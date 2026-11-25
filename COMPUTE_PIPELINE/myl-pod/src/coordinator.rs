//! Pod-Koordinator: Pipeline-Dispatch und PoI-Aggregation (Anhang A.3,
//! `coordinator_loop`).
//!
//! Der Koordinator schickt die Token-Nachrichten einer Sitzung durch die
//! Shard-Pipeline, sammelt die generierten Tokens und hält die
//! abgeschlossenen Segmente, aus denen das PoI-Bündel entsteht.
//!
//! ⚑ **Sitzungen laufen nebeneinander** (2026-11-15, Whitepaper 4.2:
//! „Während Shard 3 Token t berechnet, verarbeitet Shard 1 bereits Token
//! t+2“). [`Coordinator::rechne`] nimmt `&self`; was einer Sitzung gehört
//! (Rückkopplung, erzeugte Token, ihre Segmente bis zum Ende), lebt im
//! Lauf, geteilt sind nur der Weg zu den Shards und eine kurze Sperre zum
//! Anhängen. Ein Shardprozess bedient eine Frage nach der anderen; rechnen
//! zwei Sitzungen, steht die eine auf Shard 2, während die andere auf
//! Shard 0 rechnet, und so entsteht die Überlappung von selbst.
//!
//! ⚑ **Und wer am selben Shard wartet, fährt im selben Stapel**
//! (Continuous Batching, 2026-11-15, nach Orca): je Shard eine
//! [`Sammelstelle`] ohne Zeitfenster; der Shard rechnet den Stapel Ebene
//! für Ebene und den Kopf in einem Zug, die Gewichte also einmal je Stapel.
//! Höchstens [`LAEUFE_JE_SHARD`] Läufe je Shard zugleich ([`Lauftor`]).
//!
//! 📌 Bis dahin stand hier ein Sammelfenster `WINDOW_MS` von 250 ms, das
//! nie gelesen wurde, und der Pod rechnete strikt eine Anfrage nach der
//! anderen: Je Augenblick arbeitete ein Shard von vier.
//!
//! Die Determinismus-Garantie gilt unverändert: derselbe Prompt ergibt
//! bitgleich dieselbe Token-Sequenz, gleich was nebenher läuft, denn jede
//! Sitzung hat ihren eigenen KV-Cache auf jedem Shard.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use myl_types::bls::{aggregate_signatures, BlsSignature};
use myl_types::core_types::{segments_root, PoIBundle};
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};

use crate::shard::{ShardNode, ShardOut};
use crate::wire::{self, PodMessage, FLAG_SAMPLE};

/// So oft fragt der Koordinator dieselbe Anfrage nach einem Transportfehler
/// noch einmal, bevor er die Runde aufgibt (Fund 723). Zwei: Eine verlorene
/// Antwort braucht eine, eine abgelaufene Frist bei einem Shard, der noch
/// rechnet, meist auch; mehr verlängerte nur das Warten auf einen Shard,
/// der wirklich fort ist, bevor die Reserve übernimmt (6.8).
pub const WIEDERHOLUNGEN_NACH_TRANSPORTFEHLER: usize = 2;

/// Wie viele abgeschlossene Segmente höchstens ungezogen liegen.
///
/// ⚑ **Eine Notbremse und keine Politik** (Fund 164). Der vorgesehene
/// Weg ist [`Coordinator::buendel_ziehen`], das die Liste leert; dieser
/// Deckel fängt nur den Fall, dass niemand zieht. 100 000 Einträge sind
/// bei rund vierzehn je Anfrage über siebentausend Anfragen, also weit
/// jenseits einer Epoche, und trotzdem endlich.
/// ⚑ Seit dem 2026-11-23 derselbe Wert, den die Kette durchsetzt
/// ([`myl_types::MAX_SEGMENTE_JE_BUENDEL`]): Ein Bündel darüber nähme sie
/// nicht an.
pub const MAX_ABGESCHLOSSENE: usize = myl_types::MAX_SEGMENTE_JE_BUENDEL as usize;

/// Ein abgeschlossenes Segment mit den gesammelten Übergangs-Signaturen.
#[derive(Debug, Clone)]
pub struct CompletedSegment {
    pub id: SegmentId,
    pub trace: Vec<[u8; 32]>,
    /// Commitment auf die **Eingabe** des Segments.
    ///
    /// # ⚑ Warum es das braucht (Fund 102, 2026-08-30)
    ///
    /// Die Schiedsrunde bindet die strittige Eingabe an
    /// `trace[j-1]`, also an die Ausgabe der Layer davor. **Bei `j = 0`
    /// gibt es keine Layer davor.** Dort ist die Eingabe des Segments
    /// gemeint, und die stand nirgends: `myl_types::Segment` führt ein
    /// `input_commitment`, aber diesen Typ erzeugt niemand.
    ///
    /// Seit E10 legt der **Ankläger** die strittige Eingabe vor. An der
    /// ersten Layer prüfte die Schiedsrunde damit `hash(eingabe)` gegen
    /// einen Hash, den derselbe Ankläger daneben schreibt: eine
    /// tautologische Prüfung, und genau der Fehler, den Fund A11 an
    /// anderer Stelle schon einmal hatte.
    ///
    /// Der Wert ist der Hash der Token-Nutzlast, also klein und von
    /// jedem nachrechenbar, der die Anfrage kennt.
    pub eingangs_commitment: [u8; 32],
    /// ⚑ **Die Nutzlast des Eingangs** (Stufe 2, Nachrechnen, 2026-10-29):
    /// das Token als zwei `i16`, über die [`Self::eingangs_commitment`]
    /// gebildet ist. Vier Bytes je Segment; damit rechnet ein Checker die
    /// Pipeline ab den Tokens nach, ohne dass Aktivierungen aufgehoben
    /// werden (E10).
    pub eingabe: Vec<i16>,
    /// Die Merkle-Wurzel über [`Self::zusicherungen`].
    ///
    /// ⚑ **Fund 100:** Bis zum 2026-08-30 ging in die Bündelwurzel nur
    /// die `SegmentId`, und die ist `(Sitzung, Position)` mit Nullen
    /// aufgefüllt. Das Bündel beanspruchte damit Arbeit, **ohne zu
    /// sagen, was gerechnet wurde**; die Spur lag nur örtlich hier und
    /// war an nichts gebunden. Die Schiedsrunde hatte deshalb kein
    /// „behauptet", gegen das sie hätte prüfen können, nur zwei einander
    /// widersprechende Aussagen.
    pub spurwurzel: myl_types::MerkleRoot,
    pub signatures: Vec<BlsSignature>,
    pub pod_path: Vec<MinerId>,
    /// Die Token-Position, die dieses Segment gerechnet hat.
    ///
    /// Ein Segment ist genau ein Vorwärtspass, also genau ein
    /// Token-Forward-Äquivalent. Die vTFE-Gutschrift folgt damit aus der
    /// **Zahl der Segmente**; ein eigenes Feld dafür braucht es nicht
    /// mehr.
    ///
    /// **Prefill zählt mit, und das ist Absicht:** Eine Prompt-Position
    /// emittiert kein Token, rechnet aber denselben vollständigen
    /// Vorwärtspass. Sie nicht zu vergüten hieße, geleistete Arbeit nicht
    /// zu bezahlen.
    pub position: u64,
}

impl CompletedSegment {
    /// Die Kette der Zusicherungen: `[Eingang] ++ Spur`.
    ///
    /// ⚑ **Der erste Eintrag ist der Grund, warum es diese Methode
    /// gibt** (Fund 102). `kette()[j]` ist die **Eingabe** der Layer `j`,
    /// `kette()[j+1]` ihre Ausgabe. Damit ist auch die Eingabe der
    /// **ersten** Layer beweisbar; ohne den Eintrag hinge sie an nichts,
    /// und die Schiedsrunde prüfte dort einen Hash des Anklägers gegen
    /// einen zweiten Hash desselben Anklägers.
    ///
    /// Die Wurzel über diese Kette ist [`Self::spurwurzel`].
    pub fn kette(&self) -> Vec<[u8; 32]> {
        let mut k = Vec::with_capacity(self.trace.len() + 1);
        k.push(self.eingangs_commitment);
        k.extend_from_slice(&self.trace);
        k
    }

    /// Der Beleg in der Form, die das Protokoll dafür vorsieht.
    ///
    /// # ⚑ Warum es diese Umrechnung gibt und keinen zweiten Typ
    ///
    /// `myl_types::Segment` beschreibt seit dem 2026-08-13 genau diesen
    /// Beleg (Anhang A.1), und die Gateway-Planung setzt ihn voraus.
    /// Erzeugt hat ihn bis zum 2026-08-30 **niemand**: Was der Pod wirklich
    /// festhielt, war [`CompletedSegment`], und die beiden führten
    /// verschiedene Felder.
    ///
    /// Zwei Typen für dieselbe Sache sind eine zweite Quelle, und diese
    /// hier hat schon Schaden angerichtet: `Segment` trägt ein
    /// `input_commitment`, `CompletedSegment` hatte keines, und genau
    /// daraus entstand Fund 102, die tautologische Prüfung an der ersten
    /// Layer.
    ///
    /// Deshalb keine zweite Aufzeichnung, sondern eine **Projektion**:
    /// Was hier steht, folgt aus dem, was der Pod ohnehin hat.
    ///
    /// `model_version` ist der einzige Wert von außen, denn er gehört
    /// zum Modell und nicht zum Segment.
    pub fn zu_segment(&self, model_version: myl_types::ids::MerkleRoot) -> myl_types::Segment {
        myl_types::Segment {
            id: self.id,
            input_commitment: myl_types::hash::Hash(self.eingangs_commitment),
            model_version,
            pod_path: self.pod_path.clone(),
            // Die Ausgabe des Segments ist der letzte Spur-Eintrag.
            // `trace` ist beim Abschluss nie leer, das prüft
            // `segment_abschliessen`.
            output_commitment: myl_types::hash::Hash(
                *self.trace.last().expect("ein abgeschlossenes Segment hat eine Spur"),
            ),
            trace: self
                .trace
                .iter()
                .map(|h| myl_types::ids::ActivationHash::new(*h))
                .collect(),
            signatures: self.signatures.clone(),
        }
    }
}


/// Segment-Id aus Session-Id **und Position** (Anhang A.1: `h(session ‖
/// index)`; hier vereinfacht als zwei linksbündig eingetragene Felder).
///
/// **Ein Segment ist eine Position, also genau ein Vorwärtspass**
/// (Festlegung des Projektinhabers, 2026-08-23). Bis dahin trug die
/// Segment-Id nur die Session, ein ganzer Prompt lief also unter einer
/// einzigen Id, und Spur wie Datenarchiv behielten davon **nur die
/// letzte Position**: Beide wurden je Position überschrieben. Ein
/// Angeklagter hätte die Aktivierung jeder früheren Position nicht
/// liefern können, `adjudicate` hätte `NoResponse` gesehen, und das heißt
/// schuldig. Ein ehrlicher Knoten wäre geslasht worden.
///
/// Mit einem Segment je Position entfällt die Positionsachse, statt
/// nachgerüstet zu werden: Spur und Archiv tragen dieselbe Achse wie die
/// Bisektion, nämlich die Layer.
///
/// ⚑ **Dazu die Prompt-Länge** in den Bytes 16 bis 23 (T6, 2026-11-05,
/// `myl_types::prompt_laenge`): Ab ihr ist jede Position erzeugt, und der
/// Prüfer prüft dort, ob das Token der Argmax war. So gebunden wie die
/// Kennung selbst: Hier setzt sie der Koordinator, nicht der letzte Shard,
/// der das Token wählt, und der ganze Pod unterschreibt sie; ein Zwilling,
/// der denselben Auftrag rechnet, kommt auf dieselben Kennungen.
fn segment_id_from(session_id: u64, position: u64, prompt_laenge: u64) -> SegmentId {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&session_id.to_le_bytes());
    bytes[8..16].copy_from_slice(&position.to_le_bytes());
    bytes[16..24].copy_from_slice(&prompt_laenge.to_le_bytes());
    SegmentId::new(bytes)
}

/// Was ein Lauf liefert: die erzeugten Token und das letzte Segment dieses
/// Laufs (für die Abrechnung der Antwort), `None` ohne Segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lauf {
    pub token: Vec<u32>,
    pub letztes_segment: Option<SegmentId>,
    /// ⚑ Ob der Auftrag nach einer verlorenen Runde ein zweites Mal
    /// gerechnet wurde (Fund 654). Für die Mitschrift der Zwillinge
    /// (Simulation vom 2026-11-20): Eine Abweichung nach einer Wiederholung
    /// zeigt auf einen Shard, der die Sitzung nicht vergessen hatte.
    pub wiederholt: bool,
}

/// ⚑ **Eine Grenze für gleichzeitige Läufe** (2026-11-15), vor der Arbeit:
/// Wer nicht hineinkommt, wartet hier und nicht mit seinem KV-Cache vor
/// einem Shard. Der Koordinator setzt sie auf [`LAEUFE_JE_SHARD`] je Shard:
/// Ohne Stapel brachten mehr Sitzungen als Shards gemessen nur 5 %, mit
/// Stapeln wächst der Durchsatz bis 16 Sitzungen bei vier Shards weiter.
pub struct Lauftor {
    frei: Mutex<usize>,
    wach: Condvar,
}

impl Lauftor {
    /// Ein Tor mit `plaetze` Plätzen, mindestens einem.
    pub fn neu(plaetze: usize) -> Self {
        Self { frei: Mutex::new(plaetze.max(1)), wach: Condvar::new() }
    }

    /// Wartet auf einen Platz; frei wird er, wenn der Rückgabewert endet.
    pub fn eintreten(&self) -> Platzkarte<'_> {
        let mut frei = self.frei.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while *frei == 0 {
            frei = self.wach.wait(frei).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *frei -= 1;
        Platzkarte { tor: self }
    }
}

/// Ein belegter Platz im [`Lauftor`]; gibt ihn beim Ende frei, auch nach
/// einer Panik.
pub struct Platzkarte<'a> {
    tor: &'a Lauftor,
}

impl Drop for Platzkarte<'_> {
    fn drop(&mut self) {
        let mut frei = self.tor.frei.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *frei += 1;
        drop(frei);
        self.tor.wach.notify_one();
    }
}

/// ⚑ **Eine Sammelstelle je Shard** (Continuous Batching, 2026-11-15, nach
/// Orca, Yu et al., OSDI 2022: Planung je Iteration statt je Anfrage). Wer
/// eine Nachricht an den Shard hat, stellt sie ein; ist gerade kein Stapel
/// zu ihm unterwegs, nimmt er alles Wartende mit und verteilt danach die
/// Ergebnisse. Wer kommt, während ein Stapel unterwegs ist, fährt im
/// nächsten mit. **Kein Zeitfenster**: Allein wartet niemand, und gestapelt
/// wird nur, was ohnehin warten müsste.
#[derive(Default)]
struct Sammelstelle {
    zustand: Mutex<Sammlung>,
    wach: Condvar,
}

#[derive(Default)]
struct Sammlung {
    wartend: Vec<(u64, PodMessage)>,
    fertig: std::collections::HashMap<u64, Result<ShardOut, String>>,
    unterwegs: bool,
    naechste: u64,
}

impl Sammelstelle {
    fn sperren(&self) -> MutexGuard<'_, Sammlung> {
        self.zustand.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Gibt die Sammelstelle frei, auch wenn der Fahrer eines Stapels in Panik
/// geht; sonst warteten die übrigen ewig auf einen Stapel, der nie ankommt.
struct Fahrt<'a> {
    stelle: &'a Sammelstelle,
    marken: Vec<u64>,
    ergebnisse: Option<Vec<Result<ShardOut, String>>>,
}

impl Drop for Fahrt<'_> {
    fn drop(&mut self) {
        let ergebnisse = self.ergebnisse.take().unwrap_or_default();
        let mut z = self.stelle.sperren();
        let mut erg = ergebnisse.into_iter();
        for m in self.marken.drain(..) {
            let r = erg.next().unwrap_or_else(|| Err("der Stapel kam nicht an".to_string()));
            z.fertig.insert(m, r);
        }
        z.unterwegs = false;
        drop(z);
        self.stelle.wach.notify_all();
    }
}

/// Wie viele Läufe je Shard höchstens nebeneinander: mehr als einer, damit
/// sich vor jedem Shard ein Stapel bilden kann, aber nicht ohne Grenze.
pub const LAEUFE_JE_SHARD: usize = 4;

/// Die Segmente in kanonischer Folge; siehe [`Coordinator::kanonisch`].
fn kanonisch_ordnen(liste: &[CompletedSegment]) -> Vec<&CompletedSegment> {
    let mut folge: Vec<&CompletedSegment> = liste.iter().collect();
    folge.sort_by_key(|c| {
        let (sitzung, position) = myl_types::sitzung_und_position(&c.id);
        (sitzung, position, myl_types::prompt_laenge(&c.id), *c.id.as_bytes())
    });
    folge
}

/// Bleibt die Zahl der Sitzungen unter `deckel`, wenn `neu` zu `da` kommt?
/// Eine Sitzung, die schon da ist, zählt nicht doppelt.
fn sitzungen_passen(da: &[SegmentId], neu: &[SegmentId], deckel: usize) -> bool {
    let sitzungen: std::collections::BTreeSet<u64> = da.iter().chain(neu).map(|k| myl_types::sitzung_und_position(k).0).collect();
    sitzungen.len() <= deckel
}

/// Die Blätter des Bündels; siehe [`Coordinator::blaetter`].
fn blaetter_aus(liste: &[CompletedSegment]) -> (Vec<myl_types::Ausgabenzeugnis>, Vec<myl_types::Segmentzeugnis>) {
    let folge = kanonisch_ordnen(liste);
    let ausgaben = folge
        .iter()
        .map(|c| myl_types::Ausgabenzeugnis {
            id: c.id,
            eingang: c.eingangs_commitment,
            ausgang: c.trace.last().copied().unwrap_or([0u8; 32]),
        })
        .collect();
    let zeugnisse = folge.iter().map(|c| myl_types::Segmentzeugnis { id: c.id, spurwurzel: c.spurwurzel }).collect();
    (ausgaben, zeugnisse)
}

/// Pod-Koordinator.
pub struct Coordinator {
    pub pod_id: PodId,
    pub epoch: EpochId,
    /// Die Shard-Pipeline in Reihenfolge (Shard 0 zuerst).
    /// Wie die Shards erreicht werden.
    ///
    /// ⚑ **Ein Merkmal und keine Liste von Objekten** (Fund 186). Bis
    /// zum 2026-09-06 standen hier vier `Arc<ShardNode>`, also vier
    /// Shards **in diesem Prozess**, und damit war jede Messung über
    /// „geshardete Inferenz" eine Messung über eine Maschine, die vier
    /// Rollen spielt.
    weg: std::sync::Arc<dyn crate::shardweg::Shardweg>,
    /// Die festen Angaben des Pods, einmal erhoben.
    besetzung: crate::shardweg::Podbesetzung,
    /// Abgeschlossene Segmente dieser Epoche, in der Folge des Anhängens;
    /// das Bündel liest sie in [`Self::kanonisch`]er Folge.
    completed: Mutex<Vec<CompletedSegment>>,
    /// Wie viele Segmente wegen des Deckels verworfen wurden.
    verworfen: AtomicU64,
    /// Wie viele Läufe zugleich rechnen dürfen.
    tor: Lauftor,
    /// Je Shard eine Sammelstelle für Stapel über Sitzungen.
    sammelstellen: Vec<Sammelstelle>,
    /// Wer eine ausgefallene Position vertritt (Whitepaper 6.8); geht ins
    /// Bündel und wird mitunterschrieben.
    vertreter: Vec<(u32, MinerId)>,
    /// Die Endmarken des Modells: Ist das gezogene Token eine davon, ist die
    /// Antwort zu Ende. Leer heisst: bis zum Deckel, wie vor dem 2026-11-15.
    haltemarken: Vec<u32>,
    /// ⚑ **Angehalten, weil die Epoche vorbei ist** (Fund 726,
    /// [`Self::anhalten`]).
    angehalten: std::sync::atomic::AtomicBool,
    /// ⛔️ **Abgebrochene Sitzungen, auf die niemand mehr wartet** (Fund
    /// 730, [`Self::sitzung_abbrechen`]).
    abgebrochen: Mutex<std::collections::BTreeSet<u64>>,
}

impl Coordinator {
    /// Neu, mit Shards **in diesem Prozess**.
    ///
    /// ⚑ **Für Prüfläufe und den Einzelbetrieb.** Wer so fährt, hält
    /// alle Schlüssel des Pods; siehe [`crate::shardweg::ImProzess`].
    pub fn new(pod_id: PodId, epoch: EpochId, shards: Vec<Arc<ShardNode>>) -> Self {
        let weg = std::sync::Arc::new(crate::shardweg::ImProzess::neu(shards));
        Self::ueber_weg(pod_id, epoch, weg)
            .expect("Shards im Prozess antworten immer")
    }

    /// Neu, über einen beliebigen Weg zu den Shards.
    ///
    /// Erhebt dabei einmal die festen Angaben; siehe
    /// [`crate::shardweg::Podbesetzung`].
    pub fn ueber_weg(
        pod_id: PodId,
        epoch: EpochId,
        weg: std::sync::Arc<dyn crate::shardweg::Shardweg>,
    ) -> Result<Self, String> {
        let besetzung = crate::shardweg::Podbesetzung::erheben(weg.as_ref())?;
        let tor = Lauftor::neu(weg.shardzahl() * LAEUFE_JE_SHARD);
        let sammelstellen = (0..weg.shardzahl()).map(|_| Sammelstelle::default()).collect();
        Ok(Self {
            pod_id,
            epoch,
            weg,
            besetzung,
            completed: Mutex::new(Vec::new()),
            verworfen: AtomicU64::new(0),
            tor,
            sammelstellen,
            vertreter: Vec::new(),
            haltemarken: Vec::new(),
            angehalten: std::sync::atomic::AtomicBool::new(false),
            abgebrochen: Mutex::new(std::collections::BTreeSet::new()),
        })
    }

    /// ⚑ **Die Epoche dieses Pods ist vorbei: nichts mehr rechnen** (Fund
    /// 726, 2026-11-24; Whitepaper 4.2: beim Epochenwechsel zieht die
    /// Sitzung in den Pod der neuen Epoche). Jede weitere Frage an einen
    /// Shard scheitert sofort, ohne Wiederholung, und ein Lauf, der damit
    /// scheitert, wird nicht neu gerechnet. 📌 Bis hierher rechnete ein Lauf
    /// der alten Epoche weiter, bis er von selbst scheiterte, im Auftragslauf
    /// vom 24. 13 bis 21 Minuten, neben dem Pod der neuen Epoche, der unter
    /// dieser Last selbst keine Runde zu Ende brachte. Endgültig: Der
    /// Koordinator gehört zu seiner Epoche.
    pub fn anhalten(&self) {
        self.angehalten.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Ob [`Self::anhalten`] gerufen wurde.
    pub fn ist_angehalten(&self) -> bool {
        self.angehalten.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// ⛔️ **Fund 730 (2026-11-25): Auf diese Sitzung wartet niemand mehr.**
    /// Wie [`Self::anhalten`], nur für sie: Jede weitere Frage für sie
    /// scheitert sofort, ohne Wiederholung, und ihr Lauf wird nicht neu
    /// gerechnet. Die übrigen Sitzungen dieses Pods rechnen weiter. Gilt bis
    /// zum Ende ihres Laufs; ein Abbruch danach bleibt ohne Wirkung.
    pub fn sitzung_abbrechen(&self, sitzung: u64) {
        self.abgebrochen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(sitzung);
    }

    /// Ob [`Self::sitzung_abbrechen`] für `sitzung` gerufen wurde.
    pub fn ist_abgebrochen(&self, sitzung: u64) -> bool {
        self.abgebrochen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&sitzung)
    }

    /// **Bei diesen Token endet eine Antwort** (Fund 702, 2026-11-15).
    ///
    /// 📌 Bis hierher rechnete der Pod jede Anfrage bis zum Deckel, auch
    /// nach der Endmarke: Das Modell schrieb seine Antwort, `<|im_end|>`,
    /// und danach eine erfundene nächste Runde, bis die Zahl erreicht war.
    /// Mit vier Token je Frage fiel das nicht auf; ein Agent bekommt so
    /// Werkzeugaufrufe aus der erfundenen Fortsetzung und zahlt jedes Token
    /// davon. Das Endtoken selbst bleibt in der Folge: Es ist gerechnet,
    /// gespurt und unterschrieben wie jedes andere.
    ///
    /// ⚑ Beide Zwillinge halten an derselben Stelle, denn sie ziehen dieselben
    /// Token; der Vergleich der Spuren bleibt, wie er war.
    pub fn haltemarken_setzen(&mut self, marken: Vec<u32>) {
        self.haltemarken = marken;
    }

    /// Anzahl der Shards in der Pipeline.
    pub fn num_shards(&self) -> usize {
        self.weg.shardzahl()
    }

    /// Führt einen Prompt durch die Shard-Pipeline und liefert die
    /// generierten Tokens (deterministisch).
    ///
    /// `prompt_tokens` ist der Prompt; `max_new_tokens` begrenzt die
    /// Generation. Die Ausgabe ist bei identischem Prompt bitgleich.
    ///
    /// 📌 **Fund 654 (2026-10-26): Eine verlorene Runde kostete den ganzen
    /// Auftrag.** Ging eine Runde zu einem Shard verloren (im Lauf über
    /// Docker ein IO-Fehler auf der Leitung), fehlte dem Shard diese
    /// Position im KV-Speicher, und jede folgende lehnte er ab („hinter dem
    /// Ende des KV-Speichers“); der Koordinator schickte trotzdem alle
    /// weiter. Jetzt bricht die erste fehlgeschlagene Runde den Durchlauf
    /// ab, die Segmente dieses Durchlaufs werden verworfen (sonst stünde
    /// dieselbe Position zweimal im Bündel), die Shards vergessen die
    /// Sitzung, und der Auftrag wird **einmal** neu gerechnet. Scheitert
    /// auch das, kommt nichts zurück und kein halbes Segment bleibt stehen.
    pub fn run_prompt(&self, session_id: u64, prompt_tokens: &[u32], max_new_tokens: u64) -> Vec<u32> {
        self.rechne(session_id, prompt_tokens, max_new_tokens).token
    }

    /// Wie [`Self::run_prompt`], dazu das letzte Segment **dieses** Laufs.
    ///
    /// ⚑ **Nebenläufig aufrufbar** (2026-11-15). Die Segmente eines Laufs
    /// sammeln sich in ihm und kommen erst am Ende unter einer kurzen Sperre
    /// zu den übrigen; ein gescheiterter Durchlauf hinterlässt also nichts,
    /// ohne dass etwas zurückgeschnitten werden müsste (Fund 654). Nebenher
    /// wäre `completed_segments().last()` das Segment irgendeiner Sitzung,
    /// deshalb kommt es von hier. Zwei Läufe derselben Sitzung zugleich
    /// gehören nicht hierher: Sie teilten sich auf jedem Shard einen
    /// KV-Cache; die Tür vergibt je Anfrage eine neue Sitzung.
    pub fn rechne(&self, session_id: u64, prompt_tokens: &[u32], max_new_tokens: u64) -> Lauf {
        let lauf = self.rechne_innen(session_id, prompt_tokens, max_new_tokens);
        self.abgebrochen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&session_id);
        lauf
    }

    fn rechne_innen(&self, session_id: u64, prompt_tokens: &[u32], max_new_tokens: u64) -> Lauf {
        let _platz = self.tor.eintreten();
        let mut ablage = Vec::new();
        let mut wiederholt = false;
        let token = match self.ein_durchlauf(session_id, prompt_tokens, max_new_tokens, true, &mut ablage) {
            Some(t) => t,
            // Fund 726: Die Epoche ist vorbei; der Pod der neuen rechnet.
            None if self.ist_angehalten() => {
                self.sitzung_abschliessen(session_id);
                eprintln!("[coordinator] Sitzung {session_id}: angehalten, die Epoche ist vorbei");
                return Lauf { token: Vec::new(), letztes_segment: None, wiederholt };
            }
            // Fund 730: Niemand wartet mehr auf sie.
            None if self.ist_abgebrochen(session_id) => {
                self.sitzung_abschliessen(session_id);
                eprintln!("[coordinator] Sitzung {session_id}: abgebrochen, niemand wartet mehr");
                return Lauf { token: Vec::new(), letztes_segment: None, wiederholt };
            }
            None => {
                wiederholt = true;
                ablage.clear();
                self.sitzung_abschliessen(session_id);
                eprintln!("[coordinator] Sitzung {session_id}: eine Runde ging verloren, der Auftrag wird neu gerechnet");
                // ⚑ **Fund 725 (2026-11-24): Die Wiederholung als Stapel.**
                //   Bis hierher ging sie Token für Token, damit sie auch über
                //   einen Shard trägt, der den Stapel nicht kennt (Fund 650);
                //   das tut heute `stapel_rechnen` selbst, das nach einer
                //   Ablehnung einzeln rechnet. 📌 Token für Token kostete eine
                //   Runde je Token und Shard (Fund 650: rund 4 s je Token über
                //   Knoten): Im Auftragslauf vom 24. rechnete eine am Wechsel
                //   verlorene Runde mit 4 693 Token 20 bis 37 Minuten über den
                //   alten Pod weiter, neben dem Pod der neuen Epoche, bevor das
                //   Podwerk den Auftrag weitergeben konnte.
                match self.ein_durchlauf(session_id, prompt_tokens, max_new_tokens, true, &mut ablage) {
                    Some(t) => t,
                    None => return Lauf { token: Vec::new(), letztes_segment: None, wiederholt },
                }
            }
        };
        let letztes_segment = ablage.last().map(|c| c.id);
        self.anhaengen(ablage);
        Lauf { token, letztes_segment, wiederholt }
    }

    /// Die Segmente eines Laufs zu den übrigen, bis zum Deckel.
    ///
    /// ⚑ **Die Notbremse** (Fund 164). Solange niemand ein Bündel zieht,
    /// wüchse die Liste sonst ohne Grenze. Der Deckel ist grosszügig und
    /// der Zähler daneben sagt, dass er greift; ein stiller Verlust wäre
    /// schlimmer als ein sichtbarer.
    ///
    /// ⚑ **Der Deckel der Sitzungen** (Stufe 1 je Sitzung): Das Bündel
    /// nennt jede Sitzung einzeln, und die Kette nimmt höchstens
    /// [`myl_types::SITZUNGEN_JE_BUENDEL_HOECHSTENS`]. Ein Lauf, der eine
    /// Sitzung darüber hinaus brächte, kommt ganz nicht hinein: Ein
    /// Bündel mit zu langer Liste gälte gar nicht, und das kostete die
    /// Arbeit der ganzen Epoche statt der einen Sitzung.
    fn anhaengen(&self, ablage: Vec<CompletedSegment>) {
        let mut liste = self.liste();
        let kennungen = |l: &[CompletedSegment]| l.iter().map(|c| c.id).collect::<Vec<_>>();
        if !sitzungen_passen(&kennungen(&liste), &kennungen(&ablage), myl_types::SITZUNGEN_JE_BUENDEL_HOECHSTENS) {
            drop(liste);
            self.verworfen.fetch_add(ablage.len() as u64, Ordering::Relaxed);
            return;
        }
        let frei = MAX_ABGESCHLOSSENE.saturating_sub(liste.len());
        let zuviel = ablage.len().saturating_sub(frei);
        liste.extend(ablage.into_iter().take(frei));
        drop(liste);
        if zuviel > 0 {
            self.verworfen.fetch_add(zuviel as u64, Ordering::Relaxed);
        }
    }

    /// Die Liste der Segmente, auch nach einer Panik in einem anderen Lauf:
    /// Angehängt wird in einem Zug, eine halbe Änderung gibt es nicht.
    fn liste(&self) -> MutexGuard<'_, Vec<CompletedSegment>> {
        self.completed.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Ein Durchlauf; `None`, sobald eine Runde zu einem Shard scheitert.
    /// `stapel`: der Prefill als Stapel je Shard (Fund 650). Die Segmente
    /// gehen nach `ablage`.
    fn ein_durchlauf(
        &self,
        session_id: u64,
        prompt_tokens: &[u32],
        max_new_tokens: u64,
        stapel: bool,
        ablage: &mut Vec<CompletedSegment>,
    ) -> Option<Vec<u32>> {
        let mut generated = Vec::new();

        // 1) Prefill: Prompt-Tokens durch die Pipeline schicken (je Token
        //    eine Nachricht, Position 0..P-1). Das letzte Prompt-Token
        //    trägt FLAG_SAMPLE und löst das erste Sampling aus.
        let mut pending_feedback: Option<PodMessage> = None;
        // 📌 **Fund 650 (2026-10-27): der Prefill als Stapel.** Über Knoten
        //    mit Latenz kostete jedes Prompt-Token eine Runde je Shard, rund
        //    4 s je Token bei vier Shards. Jetzt gehen bis zu `MAX_STAPEL`
        //    Positionen in **einer** Runde je Shard; jede bleibt ihr eigenes
        //    Segment mit eigener Spur und Unterschrift, und gerechnet wird in
        //    derselben Reihenfolge wie einzeln.
        if stapel && prompt_tokens.len() > 1 {
            let mut i0 = 0usize;
            for teil in prompt_tokens.chunks(crate::shardweg::MAX_STAPEL) {
                let mut msgs = Vec::with_capacity(teil.len());
                let mut eingaenge = Vec::with_capacity(teil.len());
                for (k, tok) in teil.iter().enumerate() {
                    let i = i0 + k;
                    let packed = wire::pack_tokens(&[*tok]);
                    let flags = if i + 1 == prompt_tokens.len() { FLAG_SAMPLE } else { 0 };
                    eingaenge.push(crate::trace::activation_hash(&packed));
                    msgs.push(PodMessage::token_input(segment_id_from(session_id, i as u64, prompt_tokens.len() as u64), session_id, i as u64, packed, flags));
                }
                let ergebnisse = self.pump_stapel(&msgs)?;
                for (k, (out_trace, out_sigs, token_opt, feedback_opt)) in ergebnisse.into_iter().enumerate() {
                    let i = i0 + k;
                    self.segment_abschliessen(ablage, segment_id_from(session_id, i as u64, prompt_tokens.len() as u64), i as u64, out_trace, out_sigs, eingaenge[k], msgs[k].payload.clone());
                    if let Some(t) = token_opt {
                        generated.push(t);
                    }
                    if feedback_opt.is_some() {
                        pending_feedback = feedback_opt;
                    }
                }
                i0 += teil.len();
            }
        }
        let einzeln: &[u32] = if stapel && prompt_tokens.len() > 1 { &[] } else { prompt_tokens };
        for (i, tok) in einzeln.iter().enumerate() {
            let is_last = i + 1 == prompt_tokens.len();
            let packed = wire::pack_tokens(&[*tok]);
            let flags = if is_last { FLAG_SAMPLE } else { 0 };
            let segment_id = segment_id_from(session_id, i as u64, prompt_tokens.len() as u64);
            // Das Commitment auf die Eingabe, bevor sie verschickt wird
            // (Fund 102): die Nutzlast dieser Nachricht.
            let eingang = crate::trace::activation_hash(&packed);
            let msg = PodMessage::token_input(segment_id, session_id, i as u64, packed, flags);
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg)?;
            self.segment_abschliessen(ablage, segment_id, i as u64, out_trace, out_sigs, eingang, msg.payload.clone());
            if let Some(t) = token_opt {
                generated.push(t);
            }
            if feedback_opt.is_some() {
                pending_feedback = feedback_opt;
            }
        }
        if generated.last().is_some_and(|t| self.haltemarken.contains(t)) {
            pending_feedback = None;
        }

        // 2) Autoregressive Feedback-Schleife: das vom End-Shard
        //    erzeugte Feedback wird durch die Pipeline geschickt, bis das
        //    Budget erschöpft ist oder kein Feedback mehr kommt.
        while (generated.len() as u64) < max_new_tokens {
            let mut msg = match pending_feedback.take() {
                Some(m) => m,
                None => break,
            };
            // Die Feedback-Nachricht trägt die Segment-Id ihres Erzeugers.
            // Mit einem Segment je Position gehört sie auf die neue.
            let segment_id = segment_id_from(session_id, msg.position, prompt_tokens.len() as u64);
            msg.segment_id = segment_id;
            let position = msg.position;
            let eingang = crate::trace::activation_hash(&msg.payload);
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg)?;
            self.segment_abschliessen(ablage, segment_id, position, out_trace, out_sigs, eingang, msg.payload.clone());
            match token_opt {
                Some(t) => generated.push(t),
                None => break,
            }
            if generated.last().is_some_and(|t| self.haltemarken.contains(t)) {
                break;
            }
            pending_feedback = feedback_opt;
        }

        Some(generated)
    }

    /// Vermerkt ein abgeschlossenes Segment, also einen Vorwärtspass.
    ///
    /// Eine leere Spur wird **nicht** aufgenommen: Sie entstünde, wenn
    /// kein Shard gerechnet hat, und ein Segment ohne Arbeit gehört nicht
    /// in ein PoI-Bündel.
    #[allow(clippy::too_many_arguments)]
    fn segment_abschliessen(
        &self,
        ablage: &mut Vec<CompletedSegment>,
        id: SegmentId,
        position: u64,
        trace: Vec<[u8; 32]>,
        signatures: Vec<BlsSignature>,
        eingangs_commitment: [u8; 32],
        eingabe: Vec<i16>,
    ) {
        if trace.is_empty() {
            return;
        }
        let pod_path: Vec<MinerId> = (0..self.weg.shardzahl())
            .map(|i| MinerId::new([(i as u8) + 1; 32]))
            .collect();
        // ⚑ Die Kette ist `[Eingang] ++ Spur`, und damit ist die Eingabe
        // **jeder** Layer beweisbar, auch die der ersten (Fund 102):
        // `kette[j]` ist die Eingabe der Layer `j`, `kette[j+1]` ihre
        // Ausgabe. Ohne den ersten Eintrag hinge die erste Layer an
        // nichts.
        // Der Deckel greift beim Anhängen ([`Self::anhaengen`]).
        let mut kette = Vec::with_capacity(trace.len() + 1);
        kette.push(eingangs_commitment);
        kette.extend_from_slice(&trace);

        // Die Wurzel jetzt und nicht erst beim Bündeln: Sie ist die
        // Zusicherung über das Ergebnis, und eine Zusicherung, die erst
        // später entsteht, ist eine, die man sich noch überlegen kann.
        let spurwurzel = match myl_types::spurwurzel(&kette) {
            Ok(w) => w,
            Err(_) => return,
        };
        ablage.push(CompletedSegment {
            id,
            position,
            trace,
            eingangs_commitment,
            eingabe,
            spurwurzel,
            signatures,
            pod_path,
        });
    }

    /// **Ein Stapel durch die Pipeline** (Fund 650): je Shard eine Runde mit
    /// allen Nachrichten, die noch unterwegs sind; je Nachricht dasselbe,
    /// was [`Self::pump`] für eine sammelt. `None`, sobald ein Shard ablehnt,
    /// nicht erreicht wird oder etwas anderes als einen Stapel liefert (ein
    /// Shard, der ihn nicht kennt).
    #[allow(clippy::type_complexity)]
    fn pump_stapel(
        &self,
        erste: &[PodMessage],
    ) -> Option<Vec<(Vec<[u8; 32]>, Vec<BlsSignature>, Option<u32>, Option<PodMessage>)>> {
        let n = erste.len();
        let mut aus: Vec<(Vec<[u8; 32]>, Vec<BlsSignature>, Option<u32>, Option<PodMessage>)> =
            (0..n).map(|_| (Vec::new(), Vec::new(), None, None)).collect();
        let mut unterwegs: Vec<(usize, PodMessage)> = erste.iter().cloned().enumerate().collect();
        for shard_idx in 0..self.weg.shardzahl() {
            if unterwegs.is_empty() {
                break;
            }
            let stapel: Vec<PodMessage> = unterwegs.iter().map(|(_, m)| m.clone()).collect();
            // ⚑ **Über die Sammelstelle** (2026-11-15): Der Prefill einer
            //   Sitzung fährt mit den Decode-Token anderer im selben Stapel.
            let mut outs = Vec::with_capacity(stapel.len());
            for r in self.sammeln_viele(shard_idx, &stapel) {
                match r {
                    Ok(o) => outs.push(o),
                    Err(e) => {
                        eprintln!("[coordinator] Shard {shard_idx} lehnte den Stapel ab: {e}");
                        return None;
                    }
                }
            }
            let mut weiter = Vec::new();
            for ((i, _), out) in unterwegs.into_iter().zip(outs) {
                let e = &mut aus[i];
                match out {
                    ShardOut::Forward(next) => {
                        e.0 = next.trace.clone();
                        e.1.push(next.signature);
                        weiter.push((i, next));
                    }
                    ShardOut::Token { token, feedback, trace, signature, .. } => {
                        e.0 = trace;
                        e.1.push(signature);
                        e.2 = Some(token);
                        e.3 = feedback;
                    }
                    ShardOut::Prefill { trace, signature } => {
                        e.0 = trace;
                        e.1.push(signature);
                    }
                }
            }
            unterwegs = weiter;
        }
        Some(aus)
    }

    /// Rechnet eine Nachricht auf Shard `j` über dessen Sammelstelle; siehe
    /// [`Sammelstelle`].
    fn sammeln(&self, j: usize, msg: &PodMessage) -> Result<ShardOut, String> {
        self.sammeln_viele(j, std::slice::from_ref(msg))
            .pop()
            .unwrap_or_else(|| Err("die Sammelstelle lieferte nichts".to_string()))
    }

    /// Rechnet mehrere Nachrichten eines Aufrufers auf Shard `j` über dessen
    /// Sammelstelle, Ergebnis je Nachricht in ihrer Folge. ⚑ **In
    /// Eingangsfolge abgeholt**: Nachrichten einer Sitzung bleiben geordnet,
    /// auch wenn sie auf zwei Stapel verteilt werden, und ihre Positionen
    /// steigen damit, wie der Shard es verlangt.
    fn sammeln_viele(&self, j: usize, msgs: &[PodMessage]) -> Vec<Result<ShardOut, String>> {
        let Some(stelle) = self.sammelstellen.get(j) else {
            return self.stapel_rechnen(j, msgs);
        };
        let n = msgs.len();
        let mut z = stelle.sperren();
        let erste = z.naechste;
        z.naechste += n as u64;
        let marken: Vec<u64> = (erste..erste + n as u64).collect();
        z.wartend.extend(marken.iter().copied().zip(msgs.iter().cloned()));
        let mut ergebnisse: Vec<Option<Result<ShardOut, String>>> = (0..n).map(|_| None).collect();
        let mut offen = n;
        loop {
            for (k, m) in marken.iter().enumerate() {
                if ergebnisse[k].is_none() {
                    if let Some(r) = z.fertig.remove(m) {
                        ergebnisse[k] = Some(r);
                        offen -= 1;
                    }
                }
            }
            if offen == 0 {
                return ergebnisse.into_iter().flatten().collect();
            }
            if !z.unterwegs && !z.wartend.is_empty() {
                z.unterwegs = true;
                let k = z.wartend.len().min(crate::shardweg::MAX_STAPEL);
                let stapel: Vec<(u64, PodMessage)> = z.wartend.drain(..k).collect();
                drop(z);
                let mut fahrt = Fahrt { stelle, marken: stapel.iter().map(|(m, _)| *m).collect(), ergebnisse: None };
                let nachrichten: Vec<PodMessage> = stapel.into_iter().map(|(_, m)| m).collect();
                fahrt.ergebnisse = Some(self.stapel_rechnen(j, &nachrichten));
                drop(fahrt);
                z = stelle.sperren();
                continue;
            }
            z = stelle.wach.wait(z).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Ein Stapel an Shard `j`, Ergebnis je Nachricht. Eine allein geht wie
    /// bisher als `Rechne`. ⚑ **Lehnt der Shard den Stapel ab, rechnet jede
    /// Nachricht für sich**: Er prüft alles vor dem ersten Rechnen
    /// ([`ShardNode::process_stapel`]), also hat er nichts verändert, und
    /// eine kaputte Nachricht nimmt keine fremde Sitzung mit.
    ///
    /// ⛔️ **Fund 719 (2026-11-23): Nur nach einer Ablehnung, nie nach einem
    /// Transportfehler.** Hier stand `_ =>`: Auch ein abgebrochener Strom
    /// („unexpected end of file“) führte zum stillen Einzelrechnen. Hatte der
    /// Shard den Stapel schon gerechnet und nur die Antwort ging verloren,
    /// rechnete er dieselben Positionen ein zweites Mal; beim Netzmodell
    /// (lineare Attention) verschob das den Zustand, und die Zwillinge
    /// lieferten verschiedene Token, ohne dass eine Zeile davon sprach
    /// (Kurzlauf vom 2026-11-23, Simulation vom 2026-11-20). Jetzt scheitert
    /// der Stapel sichtbar, die Runde gilt als verloren, und der Auftrag
    /// wird nach dem Vergessen der Sitzung einmal neu gerechnet (Fund 654).
    ///
    /// ⚑ **Fund 723 (2026-11-24): Vor dem Aufgeben dieselbe Anfrage noch
    /// einmal** ([`WIEDERHOLUNGEN_NACH_TRANSPORTFEHLER`]). Der Shard rechnet
    /// eine Anfrage nur einmal und beantwortet dieselbe danach aus dem
    /// Speicher ([`ShardNode::einmal_rechnen`]); ging nur die Antwort
    /// verloren oder lief die Frist ab, während er noch rechnete, kommt so
    /// dieselbe Antwort, ohne dass der Zustand sich verschiebt und ohne die
    /// Runde von vorn.
    fn stapel_rechnen(&self, j: usize, msgs: &[PodMessage]) -> Vec<Result<ShardOut, String>> {
        if msgs.len() == 1 {
            return vec![self.rechne_wiederholt(j, &msgs[0])];
        }
        match self.frage_wiederholt(j, &crate::shardweg::Shardanfrage::RechneStapel(msgs.to_vec())) {
            Ok(crate::shardweg::Shardantwort::StapelGerechnet(o)) if o.len() == msgs.len() => o.into_iter().map(Ok).collect(),
            // Der Shard hat geantwortet und den Stapel abgelehnt: nichts
            // gerechnet, einzeln geht es.
            Ok(crate::shardweg::Shardantwort::Fehler(_)) => msgs.iter().map(|m| self.rechne_wiederholt(j, m)).collect(),
            Ok(_) => {
                let f = format!("Shard {j} antwortete auf den Stapel unpassend");
                msgs.iter().map(|_| Err(f.clone())).collect()
            }
            Err(f) => msgs.iter().map(|_| Err(format!("Stapel an Shard {j} nicht beantwortet: {f}"))).collect(),
        }
    }

    /// Eine Frage an Shard `j`; nach einem Transportfehler dieselbe noch
    /// einmal, höchstens [`WIEDERHOLUNGEN_NACH_TRANSPORTFEHLER`]-mal. Eine
    /// Antwort, auch eine Ablehnung, wird nie wiederholt.
    fn frage_wiederholt(&self, j: usize, anfrage: &crate::shardweg::Shardanfrage) -> Result<crate::shardweg::Shardantwort, String> {
        let mut versuch = 0;
        loop {
            if self.ist_angehalten() {
                return Err("angehalten: die Epoche dieses Pods ist vorbei".to_string());
            }
            if anfrage.sitzung().is_some_and(|s| self.ist_abgebrochen(s)) {
                return Err("abgebrochen: niemand wartet mehr auf diese Sitzung".to_string());
            }
            match self.weg.frage(j, anfrage) {
                Ok(a) => return Ok(a),
                Err(f) if versuch < WIEDERHOLUNGEN_NACH_TRANSPORTFEHLER => {
                    versuch += 1;
                    eprintln!("[coordinator] Shard {j} nicht beantwortet ({f}); dieselbe Anfrage noch einmal ({versuch}/{WIEDERHOLUNGEN_NACH_TRANSPORTFEHLER})");
                }
                Err(f) => return Err(f),
            }
        }
    }

    /// Wie [`crate::shardweg::Shardweg::rechne`], mit [`Self::frage_wiederholt`].
    fn rechne_wiederholt(&self, j: usize, m: &PodMessage) -> Result<ShardOut, String> {
        match self.frage_wiederholt(j, &crate::shardweg::Shardanfrage::Rechne(m.clone()))? {
            crate::shardweg::Shardantwort::Gerechnet(aus) => Ok(*aus),
            crate::shardweg::Shardantwort::Fehler(e) => Err(e),
            andere => Err(format!("Shard {j} antwortete mit {andere:?} statt einer Rechnung")),
        }
    }

    /// Schickt eine Nachricht durch die Shard-Pipeline und sammelt Spur,
    /// Signaturen, ein evtl. gesampeltes Token und die Feedback-Nachricht.
    /// `None`, wenn ein Shard ablehnt oder nicht erreicht wird (Fund 654).
    #[allow(clippy::type_complexity)]
    fn pump(
        &self,
        first: &PodMessage,
    ) -> Option<(Vec<[u8; 32]>, Vec<BlsSignature>, Option<u32>, Option<PodMessage>)> {
        let mut trace = Vec::new();
        let mut signatures = Vec::new();
        let mut token_out = None;
        let mut feedback_out = None;
        let mut current = first.clone();
        loop {
            let shard_idx = if current.carries_tokens() {
                0
            } else {
                (current.sender_shard + 1) as usize
            };
            if shard_idx >= self.weg.shardzahl() {
                break;
            }
            match self.sammeln(shard_idx, &current) {
                Ok(ShardOut::Forward(next)) => {
                    trace = next.trace.clone();
                    signatures.push(next.signature);
                    current = next;
                }
                // **Spur und Signatur auch hier übernehmen.** Der
                // letzte Shard endet immer in einem dieser beiden Zweige;
                // bis 2026-08-23 fielen seine Einträge deshalb unter den
                // Tisch, und bei einem Pod aus einem Shard blieb die Spur
                // ganz leer.
                Ok(ShardOut::Token {
                    token,
                    feedback,
                    trace: t,
                    signature,
                    ..
                }) => {
                    trace = t;
                    signatures.push(signature);
                    token_out = Some(token);
                    feedback_out = feedback;
                    break;
                }
                Ok(ShardOut::Prefill {
                    trace: t,
                    signature,
                }) => {
                    // Prefill-Position: kein Token, kein Feedback, aber
                    // gerechnet wurde sehr wohl.
                    trace = t;
                    signatures.push(signature);
                    break;
                }
                Err(e) => {
                    eprintln!("[coordinator] Shard {} lehnte ab: {}", shard_idx, e);
                    return None;
                }
            }
        }
        Some((trace, signatures, token_out, feedback_out))
    }

    /// ⚑ **Die kanonische Folge der Segmente** (2026-11-15):
    /// nach Sitzung, Position und Promptlänge als Zahlen, die Kennung zuletzt.
    /// Bündelwurzel, Ausgabenwurzel und die Stellen der Spurablage folgen ihr.
    /// 📌 Bis dahin galt die Reihenfolge des Fertigwerdens: unschädlich,
    /// solange ein Pod eine Anfrage nach der anderen rechnet; rechnen
    /// Sitzungen nebeneinander, wären sich zwei Zwillinge mit derselben
    /// Arbeit je nach Takt uneinig. `completed` selbst bleibt in der Folge
    /// des Anhängens; sortiert wird beim Lesen. Eine Abschrift, denn die
    /// Liste steht unter einer Sperre.
    pub fn kanonisch(&self) -> Vec<CompletedSegment> {
        self.kanonisch_bis(usize::MAX)
    }

    /// **Dasselbe für die ersten `n` abgeschlossenen Segmente**, also für
    /// den Stand eines Bündels mit `n` Segmenten.
    ///
    /// ⚑ Die Liste wächst nur, und ein Lauf hängt seine Segmente in einem
    /// Zug an ([`Self::rechne`]); ein Bündel mit `n` Segmenten ist deshalb
    /// genau der Anfang der Länge `n`, auch wenn danach weitere Läufe
    /// angehängt haben.
    pub fn kanonisch_bis(&self, n: usize) -> Vec<CompletedSegment> {
        let liste = self.liste();
        let n = n.min(liste.len());
        kanonisch_ordnen(&liste[..n]).into_iter().cloned().collect()
    }

    /// **Die Blätter des Bündels**: je Segment das Ausgabenzeugnis (Stufe 1:
    /// Eingang und letzte Ausgabe; die Ausgabenwurzel ordnet nach Kennung,
    /// der Zwilling kommt also auf dieselbe Wurzel, auch wenn er die Segmente
    /// in anderer Folge abschloss) und das Segmentzeugnis (in der Folge von
    /// [`Self::kanonisch`], für `segments_root`). Daraus baut der Ortsdienst auch einen
    /// Differenzbeweis.
    pub fn blaetter(&self) -> (Vec<myl_types::Ausgabenzeugnis>, Vec<myl_types::Segmentzeugnis>) {
        self.blaetter_bis(usize::MAX)
    }

    /// Die Blätter der ersten `n` Segmente (siehe [`Self::kanonisch_bis`]).
    pub fn blaetter_bis(&self, n: usize) -> (Vec<myl_types::Ausgabenzeugnis>, Vec<myl_types::Segmentzeugnis>) {
        let liste = self.liste();
        let n = n.min(liste.len());
        blaetter_aus(&liste[..n])
    }

    /// Baut ein PoI-Bündel aus den abgeschlossenen Segmenten dieser
    /// Epoche (Anhang A.1, Kap. 4.4).
    ///
    /// # ⚑ Fund 52: Das Aggregat ist **keine** Bündelsignatur
    ///
    /// Aggregiert werden die **Übergangs-Signaturen** der Segmente, also
    /// Unterschriften über `DST_SHARD_TRANSITION ‖ Rolle ‖
    /// Borsh(TransitionSig)`. `myl_consensus::verify_bundle_signature`
    /// prüft dagegen gegen `bundle_message(bundle)`, also über
    /// `DST_POI_BUNDLE ‖ epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`.
    ///
    /// **Zwei verschiedene Botschaften.** Ein Bündel aus dieser Funktion
    /// verifiziert deshalb nie — nicht weil es falsch wäre, sondern weil
    /// die beiden Seiten über verschiedene Dinge reden.
    ///
    /// **Die Richtung war die gute:** Es wurde abgelehnt, nicht
    /// angenommen. Niemand bekam Vergütung, die ihm nicht zusteht — und
    /// niemand sonst auch, denn der Pfad war nicht ungeprüft, sondern
    /// unbenutzbar.
    ///
    /// # Geschlossen am 2026-08-24 durch [`Self::build_signed_poi_bundle`]
    ///
    /// Was fehlte, war ein Protokollschritt und keine Zeile Code: Die
    /// Mitglieder sehen das **fertige** Bündel und unterschreiben seine
    /// Botschaft; erst dann gibt es ein Aggregat, das gegen die
    /// Mitgliedermenge gilt. Der Koordinator kann das nicht allein, denn
    /// sonst wäre die Zustimmung der Mitglieder eine Fiktion — und genau
    /// gegen diese Fiktion ist die Signatur da (Kap. 5.5: 100 % Slash bei
    /// falscher Aggregation).
    ///
    /// **Diese Funktion bleibt trotzdem, und sie bleibt so.** Sie baut den
    /// Inhalt; die Signaturrunde setzt darauf auf. Wer nur den Inhalt
    /// braucht (Inspektion, Tests, ein Koordinator, der noch sammelt),
    /// bekommt ihn ohne die Runde. **Ihr Ergebnis ist aber nicht
    /// einreichbar**, und der Name sagt das nicht; deshalb steht es hier.
    ///
    /// **Warum es niemandem aufgefallen ist:** `myl-pod` hing bis zum
    /// 2026-08-24 nicht an `myl-consensus` und umgekehrt. Beide Seiten
    /// sind für sich getestet; die Naht dazwischen hat nie jemand
    /// zusammengesteckt. Festgehalten in
    /// `tests/koordinator_byzantinisch.rs`.
    pub fn build_poi_bundle(&self) -> Result<PoIBundle, String> {
        // ⚑ **Ein Schnappschuss** (2026-11-15): Blätter, Zahl, Arbeit und
        //   Aggregat aus derselben Liste. Läuft nebenher eine Sitzung,
        //   kommen ihre Segmente ganz oder gar nicht hinein, nie zur Hälfte
        //   ([`Self::rechne`] hängt in einem Zug an).
        let liste = self.liste();
        if liste.is_empty() {
            return Err("keine abgeschlossenen Segmente".to_string());
        }
        let (ausgaben, zeugnisse) = blaetter_aus(&liste);
        let root = segments_root(&zeugnisse).map_err(|e| e.to_string())?;
        let wurzel_der_ausgaben = myl_types::ausgabenwurzel(&ausgaben).map_err(|e| e.to_string())?;
        // ⚑ **Je Sitzung ein Eintrag** (Stufe 1 je Sitzung): Die Kette
        //   gleicht zwei Zwillinge daran ab und vergütet, was gleich ist,
        //   auch wenn einem eine Sitzung fehlt.
        let sitzungen = myl_types::sitzungszeugnisse(&ausgaben).map_err(|e| e.to_string())?;
        let vtfe = self.vtfe_fuer(liste.len() as u64)?;
        // Aggregat über die Übergangs-Signaturen (alle dieselbe Arbeit); eine
        // Summe, also gleich in jeder Folge.
        let all_sigs: Vec<BlsSignature> = liste.iter().flat_map(|c| c.signatures.clone()).collect();
        let segmente = liste.len() as u32;
        drop(liste);
        let agg = if all_sigs.is_empty() {
            BlsSignature([0u8; 96])
        } else {
            let a = aggregate_signatures(&all_sigs).map_err(|e| e.to_string())?;
            BlsSignature(a.0)
        };
        Ok(PoIBundle {
            epoch: self.epoch,
            pod: self.pod_id,
            segments_root: root,
            vtfe_claimed: vtfe,
            aggregate_sig: agg,
            // ⚑ **Fund 187: hier stand fest eine Eins** (behoben
            // 2026-09-06). `segmente` sagt, wie viele Blätter unter
            // `segments_root` hängen, und **daraus zieht die
            // Stichprobe der Stufe 2**. Mit einer Eins über acht
            // Segmenten konnten sieben davon nie gezogen werden.
            //
            // ⚑ **Genau die Verdünnung, vor der Fund 115 warnt**, nur
            // vom Erzeuger verursacht statt vom Angreifer. Und die
            // Shards prüften dagegen schon richtig: `signiere_buendel`
            // rechnet die vTFE gegen `completed.len()` nach, während
            // die unterschriebene Botschaft eine Eins trug.
            segmente,
            ausgabenwurzel: wurzel_der_ausgaben,
            vertreter: self.vertreter.clone(),
            sitzungen,
        })
    }

    /// Die Botschaft, die die Pod-Mitglieder unterschreiben müssen,
    /// damit aus [`Self::build_poi_bundle`] ein einreichbares Bündel wird
    /// (⚑ Fund 52): `myl_types::poi_botschaft::bundle_message`, dieselbe
    /// Funktion, gegen die die Kette prüft.
    ///
    /// 📌 **Fund 673 (2026-10-28):** Hier stand eine Abschrift der Kodierung,
    /// weil sie einst in `myl-consensus` lag, das dieser Crate nur zum
    /// Prüfen einbindet. Sie liegt längst in `myl-types`; mit der
    /// Ausgabenwurzel (Stufe 1) wäre die Abschrift auseinandergelaufen.
    pub fn signierbotschaft(bundle: &PoIBundle) -> Vec<u8> {
        myl_types::poi_botschaft::bundle_message(bundle)
    }

    /// Baut ein PoI-Bündel **und lässt es von den Mitgliedern
    /// unterschreiben** (⚑ Fund 52).
    ///
    /// Das ist die Runde, die [`Self::build_poi_bundle`] fehlte: Erst
    /// steht das Bündel, dann sehen es die Mitglieder, dann
    /// unterschreiben sie **seine** Botschaft, und erst dann aggregiert
    /// der Koordinator. Das Ergebnis verifiziert gegen
    /// `myl_consensus::verify_bundle_signature`.
    ///
    /// **Die Reihenfolge ist die ganze Sicherheit.** Würde der
    /// Koordinator zuerst sammeln und danach das Bündel bauen, könnte er
    /// `vtfe_claimed` nachträglich erhöhen; die Unterschriften lägen dann
    /// über einer Botschaft, die niemand gesehen hat. Genau davor schützt
    /// es, dass die Botschaft das fertige Bündel abbildet.
    ///
    /// **Jedes Mitglied prüft vor der Unterschrift**, ob der Anspruch zu
    /// der Segmentzahl passt, die es selbst gesehen hat
    /// ([`ShardNode::signiere_buendel`]). Verweigert eines, gibt es kein
    /// Bündel: Ein Aggregat gilt gegen **alle** Mitglieder, nicht gegen
    /// eine Mehrheit, und ein unvollständiges wäre wertlos.
    pub fn build_signed_poi_bundle(&self) -> Result<PoIBundle, String> {
        let bundle = self.build_poi_bundle()?;
        let botschaft = Self::signierbotschaft(&bundle);
        // ⚑ **Dieselbe Zahl wie im Bündel, und das wird geprüft**
        // (Fund 187). Sie ging auseinander: Das Bündel trug eine Eins,
        // die Shards rechneten gegen die wirkliche Zahl. Ein Bündel,
        // dessen Signatur für eine andere Segmentzahl gegeben wurde als
        // die, die darin steht, ist nicht prüfbar.
        // 📌 Seit Sitzungen nebeneinander laufen (2026-11-15), aus dem Bündel
        //   und nicht neu aus der Liste: Dazwischen kann ein Lauf anhängen.
        let segmente = u64::from(bundle.segmente);
        let zuschnitte = self.besetzung.zuschnitte.clone();

        // ⚑ **Jeder Shard unterschreibt selbst.** Der Koordinator kann
        // es nicht: Es braucht die privaten Schlüssel, und ein Shard
        // prüft die beanspruchten vTFE nach, bevor er sie gibt
        // (Fund 52). Über den Draht ist das eine eigene Nachricht.
        let mut sigs = Vec::with_capacity(self.weg.shardzahl());
        for j in 0..self.weg.shardzahl() {
            match self.weg.frage(
                j,
                &crate::shardweg::Shardanfrage::Unterschreibe {
                    botschaft: botschaft.clone(),
                    vtfe: bundle.vtfe_claimed,
                    segmente,
                    zuschnitte: zuschnitte.clone(),
                },
            )? {
                crate::shardweg::Shardantwort::Unterschrift(sig) => sigs.push(sig),
                crate::shardweg::Shardantwort::Fehler(e) => return Err(e),
                andere => {
                    return Err(format!("Shard {j} antwortete mit {andere:?} statt einer Unterschrift"))
                }
            }
        }
        let agg = aggregate_signatures(&sigs).map_err(|e| e.to_string())?;
        Ok(PoIBundle {
            aggregate_sig: BlsSignature(agg.0),
            ..bundle
        })
    }

    /// Die abgeschlossenen Segmente (für Tests/Inspektion).
    ///
    /// ⚑ **Eine Abschrift** (2026-11-15): Die Liste steht unter einer
    /// Sperre, und ein Wächter in der Hand eines Aufrufers, der danach ein
    /// Bündel baut, verklemmte sich. Für die blosse Zahl
    /// [`Self::segmentzahl`].
    pub fn completed_segments(&self) -> Vec<CompletedSegment> {
        self.liste().clone()
    }

    /// Wie viele Segmente abgeschlossen sind.
    pub fn segmentzahl(&self) -> usize {
        self.liste().len()
    }

    /// Beendet eine Sitzung: Alle Shards vergessen ihren Zustand dazu.
    ///
    /// ⚑ **Der Aufrufer ist das Rechenwerk, nicht die Tür** (Fund 164).
    /// Es weiss als einziges, wann eine Antwort vollständig gelesen ist:
    /// Der Dekodier-Digest wird **nach** dem Rechnen gebraucht, und wer
    /// vorher räumte, nähme ihn weg.
    pub fn sitzung_abschliessen(&self, session_id: u64) {
        for j in 0..self.weg.shardzahl() {
            // ⚑ **Ein Fehler beim Räumen bricht nichts ab.** Wer eine
            // Sitzung nicht vergisst, hält Speicher; wer deshalb den
            // ganzen Abschluss abbricht, hält die übrigen auch.
            let _ = self.weg.frage(j, &crate::shardweg::Shardanfrage::SitzungVergessen(session_id));
        }
    }

    /// Wie viele Sitzungen der erste Shard gerade hält.
    pub fn gehaltene_sitzungen(&self) -> usize {
        match self.weg.frage(0, &crate::shardweg::Shardanfrage::GehalteneSitzungen) {
            Ok(crate::shardweg::Shardantwort::Zahl(n)) => n as usize,
            _ => 0,
        }
    }

    /// ⚑ **Wer eine Position vertritt** (Whitepaper 6.8): Die Reserve hat
    /// eine ausgefallene Position übernommen; das Bündel nennt sie, und ihr
    /// Shard unterschreibt statt des Inhabers.
    pub fn vertreter_setzen(&mut self, vertreter: Vec<(u32, MinerId)>) {
        self.vertreter = vertreter;
    }

    /// ⚑ **Die Segmente des Vorgängers derselben Epoche erben**
    /// (2026-11-17, Restpunkt 9 zu Fund 706).
    ///
    /// Kommt mitten in der Epoche eine Runde leer zurück, besetzt der
    /// Ortsdienst den Pod neu und baut dafür einen neuen Koordinator. 📌 Bis
    /// hierher ging dabei alles verloren, was der alte in dieser Epoche schon
    /// abgeschlossen hatte: gerechnet, ausgeliefert, nie vergütet, und dem
    /// Zwilling fehlten danach diese Sitzungen im Vergleich.
    ///
    /// Geerbt wird nur von demselben Pod in derselben Epoche mit denselben
    /// Zuschnitten, denn die beanspruchten vTFE rechnen die Shards aus den
    /// Zuschnitten nach. Die geerbten Segmente stehen vorn, in ihrer alten
    /// Folge; ein Bündel mit `n` Segmenten bleibt so der Anfang der Länge `n`
    /// ([`Self::kanonisch_bis`]). Auch die Zahl der verworfenen Segmente geht
    /// mit. Hat der Neue schon eigene, werden sie dahinter angehängt, unter
    /// denselben Deckeln wie sonst.
    ///
    /// ⚠️ Vertritt im neuen Koordinator die Reserve eine Position, nennt das
    /// Bündel sie für die ganze Epoche: Das Format kennt keinen Vertreter ab
    /// einem Segment. Der Ausgefallene verliert damit, was er vorher
    /// beigetragen hat; ohne das Erben verloren es alle Mitglieder.
    pub fn segmente_erben(&mut self, vorgaenger: &Coordinator) -> Result<usize, String> {
        if vorgaenger.pod_id != self.pod_id || vorgaenger.epoch != self.epoch {
            return Err("der Vorgänger rechnete für einen anderen Pod oder eine andere Epoche".to_string());
        }
        if vorgaenger.besetzung != self.besetzung {
            return Err("der Vorgänger rechnete mit anderen Zuschnitten".to_string());
        }
        let geerbt = vorgaenger.liste().clone();
        let zahl = geerbt.len();
        let eigene = std::mem::replace(
            self.completed.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner),
            geerbt,
        );
        self.verworfen.fetch_add(vorgaenger.verworfene_segmente(), Ordering::Relaxed);
        if !eigene.is_empty() {
            self.anhaengen(eigene);
        }
        Ok(zahl)
    }

    /// Wie viele abgeschlossene Segmente verworfen wurden, weil der
    /// Deckel erreicht war.
    ///
    /// ⚑ **Sichtbar und nicht still** (Fund 164). Verworfene Segmente
    /// sind Arbeit, die niemand mehr bezeugen kann; wer sie stumm
    /// wegwirft, verschenkt Vergütung, ohne dass es jemand merkt.
    pub fn verworfene_segmente(&self) -> u64 {
        self.verworfen.load(Ordering::Relaxed)
    }

    /// Zieht das Bündel und **leert** die Segmentliste.
    ///
    /// # ⚑ Warum das Ziehen räumt
    ///
    /// `completed` wächst mit jedem Token und wurde nie kleiner
    /// (Fund 164): gemessen 14 Einträge je Anfrage, streng linear. Ein
    /// Bündel fasst die Arbeit **einer Epoche** zusammen; was darin
    /// steht, ist abgegeben, und was danach kommt, gehört ins nächste.
    ///
    /// ⚑ **Getrennt von [`Self::build_signed_poi_bundle`]**, das nur
    /// baut und nichts anfasst. Zwei Namen, zwei Wirkungen: Wer
    /// hinsehen will, sieht hin; wer abgibt, gibt ab.
    pub fn buendel_ziehen(&mut self) -> Result<PoIBundle, String> {
        let buendel = self.build_signed_poi_bundle()?;
        self.completed.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
        Ok(buendel)
    }

    /// Die beanspruchte Arbeitsmenge dieser Epoche in vTFE-Einheiten.
    ///
    /// Summe über alle Shards der Pipeline, je Shard nach der Regel aus
    /// [`myl_tokenomics::vtfe_gutschrift`]: Anteil an den
    /// Multiplikations-Additionen der Gewichtsmatrizen eines vollen
    /// Vorwärtspasses, mal der Zahl der erzeugten Token.
    ///
    /// **Bis zum 2026-08-23 stand hier die Zahl der Segmente**, mit dem
    /// Kommentar „Platzhalter für die FLOPs-Metrik". Ein Bündel über
    /// tausend Token beanspruchte damit dieselbe eine Einheit wie eines
    /// über zwei, und ein Shard mit sieben Layern dasselbe wie einer mit
    /// zweien. Genau davor warnt die Regel: *„eine Festlegung, bevor
    /// die erste Implementierung sie stillschweigend trifft."* Die
    /// Implementierung hatte sie längst getroffen.
    ///
    /// **Die Redundanz-Normierung steckt nicht hier drin.** Sie halbiert
    /// die Gutschrift, weil jedes Segment von r = 2 Pods gerechnet wird
    /// (`myl_tokenomics::redundancy_normalized_weight`), und gehört an
    /// die Stelle, die über die Pods hinwegsieht, nicht in den einzelnen
    /// Pod.
    pub fn beanspruchte_vtfe(&self) -> Result<u64, String> {
        self.vtfe_fuer(self.segmentzahl() as u64)
    }

    /// Die Arbeit für `tokens` Segmente; siehe [`Self::beanspruchte_vtfe`].
    fn vtfe_fuer(&self, tokens: u64) -> Result<u64, String> {
        if self.besetzung.zuschnitte.is_empty() {
            return Err("Pod ohne Shards beansprucht keine Arbeit".to_string());
        }
        let profil = self.besetzung.profil;
        // Ein Segment ist ein Vorwärtspass, also ein
        // Token-Forward-Äquivalent. Prefill-Positionen zählen mit: Sie
        // emittieren kein Token, rechnen aber denselben Pass.

        let mut summe = 0u64;
        for (j, z) in self.besetzung.zuschnitte.iter().enumerate() {
            let anteil = myl_tokenomics::vtfe_gutschrift(&profil, z, tokens)
                .map_err(|e| format!("Shard {j}: {e}"))?;
            summe = summe.saturating_add(anteil);
        }
        Ok(summe)
    }

    /// Dekodier-Digest einer Session: Hexwert und Zahl der Schritte.
    ///
    /// Der Wert kommt vom Shard mit dem LM-Head, weil nur dort Logits
    /// entstehen. Er folgt dem Vertrag aus
    /// [`integer_llm_runtime::generate::DekodierDigest`] und ist damit
    /// **unmittelbar** gegen den Einzelknotenlauf zu halten.
    ///
    /// **Wofür er da ist (Fund 36, letzter Teil):** Der Vergleich
    /// „Pod gegen Einzelknoten" hielt bis dahin Token gegen Token und
    /// prüfte damit, ob die Aufteilung dieselbe *Entscheidung* erzeugt.
    /// Das ist zu grob: Ein Token ist ein Argmax über `vocab_size`
    /// Zahlen und ändert sich erst, wenn deren Rangfolge kippt. Genau
    /// die kleinen Abweichungen, gegen die dieses Projekt gebaut ist,
    /// wären durchgerutscht.
    ///
    /// ⚑ **Der Fehlerfall ist vom `None` getrennt.** `None` heißt „kein
    /// Shard da" oder „nichts gesampelt", eine vergiftete Sperre heißt
    /// „ein Thread ist gestorben". Beides in dasselbe `None` zu legen
    /// hieße, dem Rückgabewert eine dritte Bedeutung zu geben, die
    /// niemand unterscheiden kann.
    pub fn dekodier_digest(&self, session_id: u64) -> Result<Option<(String, usize)>, String> {
        let letzter = self.weg.shardzahl().checked_sub(1);
        let Some(j) = letzter else {
            return Ok(None);
        };
        match self.weg.frage(j, &crate::shardweg::Shardanfrage::DekodierDigest(session_id))? {
            crate::shardweg::Shardantwort::DekodierDigest(d) => {
                Ok(d.map(|(h, s)| (h, s as usize)))
            }
            crate::shardweg::Shardantwort::Fehler(e) => Err(e),
            andere => Err(format!("Shard {j} antwortete mit {andere:?} statt einem Abdruck")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ Der Deckel zählt Sitzungen, nicht Segmente: Eine Sitzung, die
    /// schon in der Liste steht, passt immer; eine neue nur, solange Platz
    /// ist.
    #[test]
    fn der_deckel_zaehlt_sitzungen() {
        let k = |sitzung: u64, position: u64| segment_id_from(sitzung, position, 3);
        let da = [k(1, 0), k(1, 1), k(2, 0)];
        assert!(sitzungen_passen(&da, &[k(2, 1), k(2, 2)], 2), "dieselbe Sitzung noch einmal");
        assert!(!sitzungen_passen(&da, &[k(3, 0)], 2), "eine dritte bei Deckel zwei");
        assert!(sitzungen_passen(&da, &[k(3, 0)], 3));
        assert!(sitzungen_passen(&[], &[], 0));
    }

    /// Das Tor lässt höchstens so viele hinein, wie es Plätze hat, und gibt
    /// beim Verlassen einen frei. ⚑ Gegenprobe: Mit einem Tor ohne Grenze
    /// stiege die Spitze auf vier.
    #[test]
    fn das_lauftor_laesst_hoechstens_seine_plaetze_hinein() {
        use std::sync::atomic::AtomicUsize;
        let tor = Arc::new(Lauftor::neu(2));
        let drin = Arc::new(AtomicUsize::new(0));
        let spitze = Arc::new(AtomicUsize::new(0));
        let faeden: Vec<_> = (0..4)
            .map(|_| {
                let (tor, drin, spitze) = (Arc::clone(&tor), Arc::clone(&drin), Arc::clone(&spitze));
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        let _platz = tor.eintreten();
                        let jetzt = drin.fetch_add(1, Ordering::SeqCst) + 1;
                        spitze.fetch_max(jetzt, Ordering::SeqCst);
                        std::thread::yield_now();
                        drin.fetch_sub(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for f in faeden {
            f.join().expect("Faden");
        }
        assert!(spitze.load(Ordering::SeqCst) <= 2, "mehr Laeufe zugleich als Plaetze");
        assert_eq!(*tor.frei.lock().expect("Tor"), 2, "ein Platz blieb belegt");
    }

    /// Ein Tor für null Shards hat trotzdem einen Platz; sonst wartete der
    /// erste Lauf ewig, statt am fehlenden Shard zu scheitern.
    #[test]
    fn ein_tor_ohne_shards_hat_einen_platz() {
        let tor = Lauftor::neu(0);
        drop(tor.eintreten());
        assert_eq!(*tor.frei.lock().expect("Tor"), 1);
    }
}
