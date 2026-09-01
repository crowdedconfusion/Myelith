//! Die Probekette: echte Blöcke, echter Zustand, **Wegwerfware**.
//!
//! # ⚑ Das ist nicht der Beginn der Blockchain
//!
//! Steht zuerst, weil eine Verwechslung teuer wäre. Diese Kette ist eine
//! **Trockenübung des Codes**, keine Inbetriebnahme:
//!
//! - **Jeder Start beginnt bei null.** Keine Fortsetzung, keine
//!   Wiederherstellung, keine Historie. Wird der Knoten beendet, ist der
//!   Zustand weg.
//! - **Die MYL sind Spielgeld.** [`PROBEGUTHABEN`] steht in keinem
//!   Verhältnis zur Genesis-Zuteilung des echten Netzes (TOKENOMICS
//!   Punkt 4.2). Wer hier Guthaben sieht, besitzt nichts.
//! - **Ein Probeblock kann niemals an eine echte Kette anschließen.**
//!   Die Verkettung hängt am Startwert, und [`PROBE_STARTWERT`] ist ein
//!   Wert, der ausdrücklich für Proben gewählt wurde. Das ist keine
//!   Regel, die jemand einhalten muss, sondern eine Eigenschaft der
//!   Verkettung, und ein Test hält sie fest.
//!
//! Wann das Testnetz beginnt, entscheidet das Projekt, nicht dieser
//! Code. Mehr dazu im Kopf von [`crate::probe`].
//!
//! # Was hier geschlossen wird, und was offen bleibt
//!
//! Bis zum 2026-08-24 produzierte kein Knoten Blöcke. Die
//! Zustandsmaschinen in `myl-consensus` waren vollständig, aber niemand
//! trieb sie über die Zeit: Es fehlten Rundentakt, Mempool und
//! Kettenzustand. Dieses Modul liefert alle drei.
//!
//! **Was es ist:** Blockproduktion mit Kettenverkettung, Mempool,
//! Ledger-Anwendung und Zustandswurzel. Jeder Knoten führt seine eigene
//! Kette und rechnet nach, ob er beim selben Zustand landet wie der
//! Erzeuger.
//!
//! **Was es nicht ist: BFT.** Es stimmt niemand ab. Ein Knoten
//! produziert, die übrigen übernehmen und rechnen nach. Die
//! Abstimmungsrunden (Propose, Vote, Commit) liegen fertig in
//! `myl-consensus::bft`, brauchen aber ein eigenes Gossip-Topic, einen
//! Validator-Satz mit Stake und BLS-Schlüssel je Knoten. Ein neues Topic
//! ist eine Protokollentscheidung, kein Detail, und die gehört nicht
//! nebenbei getroffen.
//!
//! # Warum das trotzdem eine echte Messung ist
//!
//! Die Frage, die dieser Aufbau beantwortet, ist nicht „einigen sich die
//! Knoten", sondern die davor: **Kommen zwei Maschinen aus denselben
//! Blöcken zum selben Zustand?**
//!
//! Das ist die Protokollhälfte derselben Frage, die der
//! Determinismus-Test für die Inferenz stellt. Weicht eine
//! Zustandswurzel ab, ist irgendwo im Ledger-Pfad etwas nicht
//! deterministisch, und das bricht den Konsens genauso wie ein
//! abweichendes Inferenzergebnis. **Ohne diesen Lauf wäre das erst im
//! echten Netz aufgefallen.**
//!
//! # ⚑ Höhe und Epoche sind zwei Dinge (seit 2026-08-27)
//!
//! [`BlockHeader`] trägt beide: `height` ist die Stellung in der Kette
//! und wächst um genau eins je Block, `epoch` ist die Epoche und folgt
//! aus der Höhe ([`epoche_fuer_hoehe`]). Bei 1 800 Blöcken je Epoche
//! sind das eine Stunde Blockzeit und 1 800 Höhen.
//!
//! **Bis dahin war es ein Feld für beides.** Die Probekette schrieb ihre
//! Höhe in `epoch`, und das trug, solange eine Epoche ein Block war.
//! Jede Frist „je Epoche" bedeutete damit in Wahrheit „je Block": Der
//! Verfall von Credits nach einer Epoche war der Verfall nach einem
//! Block, und die Streitfrist von 168 Epochen wären 168 Blöcke gewesen,
//! also gut fünf Minuten statt sieben Tagen.
//!
//! Beide Felder werden beim Übernehmen geprüft: die Höhe gegen die
//! eigene, die Epoche gegen die Umrechnung. Ein mitgeführter Wert, den
//! niemand nachrechnet, ist ein Feld, das jeder setzen darf.

use borsh::BorshDeserialize;
use myl_consensus::block::{epoche_fuer_hoehe, Anweisung, Block, BlockHeader, Transaktion};
use myl_ledger::state::LedgerState;
use myl_ledger::transitions::{
    burn_to_credits, nonce_verbrauchen, sitzung_ausgeben, sitzung_eroeffnen, sitzung_widerrufen,
    transfer, miner_abmelden, miner_anmelden, buendel_einreichen, buendel_leeren, angemeldete_miner, buendel_der_epoche};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId};

/// Credit-Preis zu Genesis der Testkette.
///
/// Nicht null: Ein Preis von null wäre eine Division durch null im
/// Ledger und ist dort als Protokollfehler ausgewiesen.
pub const PROBE_CREDIT_PREIS: u64 = 100;

/// Zahl der Probekonten, die zu Beginn Guthaben bekommen.
pub const PROBEKONTEN: u8 = 8;

/// Guthaben je Probekonto in MYL-Kleinstbeträgen. **Spielgeld.**
pub const PROBEGUTHABEN: u64 = 10_000_000;

/// Der Startwert, an dem die Probekette hängt.
///
/// **Der Riegel gegen eine Verwechslung mit dem echten Netz.** Ein Block
/// nennt seinen Vorgänger; der erste Probeblock nennt diesen Wert. Ein
/// echtes Netz beginnt bei einem anderen, und damit passt ein Probeblock
/// dort nirgends hinein. Der Text sagt es auch dem, der die Bytes
/// anschaut.
pub const PROBE_STARTWERT: &[u8] =
    b"MYELITH-PROBELAUF-KEIN-TESTNETZ-KEINE-ECHTE-KETTE";

/// ## ⚑ Warum es diese Konten gibt
///
/// Der erste Dreiknotenlauf lief grün und **maß nichts**: Die
/// Zustandswurzel stand bei jeder Höhe auf demselben Wert, obwohl
/// Blöcke mit Transaktionen ankamen.
///
/// Der Grund war einfach und leicht zu übersehen. Eine
/// Burn-Transaktion braucht Deckung; die Absender hatten keine, also
/// scheiterte jede und wurde übersprungen, wie es
/// [`Kette::anwenden`] vorsieht. **Ein unveränderter Zustand ist auf
/// jeder Maschine gleich**, und damit belegte die Übereinstimmung
/// genau nichts.
///
/// Deshalb bekommen zu Genesis acht Konten Guthaben. Sie sind ein
/// **Testaufbau, kein Protokollbestandteil**: Im echten Netz entsteht
/// Guthaben aus der Genesis-Zuteilung (TOKENOMICS Punkt 4.2), nicht aus
/// dieser Konstanten.
///
/// Die Adressen hängen an einer festen Zeichenkette und **nicht** am
/// Knotennamen. Sonst sähe der Genesis-Zustand auf jeder Maschine
/// anders aus, und der Lauf meldete einen Determinismusfehler, bevor
/// irgendetwas passiert ist.
/// Der Schlüssel eines Probekontos.
///
/// ⚑ **Seit dem 2026-08-28 hat ein Probekonto einen Schlüssel**, denn
/// eine Transaktion muss unterschrieben sein (Fund 85). Vorher war ein
/// Konto nur eine Zeichenkette, und jeder konnte in seinem Namen
/// anweisen.
///
/// Aus einem festen Startwert abgeleitet, damit alle Knoten dieselben
/// Konten sehen: Ein zufälliger Schlüssel machte zwei Läufe
/// unvergleichbar.
pub fn probeschluessel(nummer: u8) -> myl_types::bls::BlsSecretKey {
    let saat = Hash::sha256(
        format!("myelith-probekonto-{}", nummer % PROBEKONTEN).as_bytes(),
    );
    myl_types::bls::BlsSecretKey::key_gen(saat.as_bytes())
        .expect("32 Byte Startwert sind für key_gen immer gültig")
}

/// Die Adresse eines Probekontos: `SHA-256` über seinen Schlüssel.
///
/// **Abgeleitet und nicht gewürfelt**, wie jede Adresse im Protokoll.
/// Nur so passt sie zu der Unterschrift, die ihr Inhaber leisten kann.
pub fn probekonto(nummer: u8) -> Address {
    let pk = probeschluessel(nummer)
        .public_key()
        .expect("aus einem gültigen Schlüssel folgt ein gültiger Punkt");
    Address::aus_schluessel(&pk)
}

/// Das Testkonto, das ein Knoten dieses Namens benutzt.
///
/// Über den Namen gewählt, damit jeder Knoten stabil dasselbe Konto
/// bebucht: Ein wechselndes Konto machte zwei Läufe unvergleichbar.
pub fn konto_fuer(name: &str) -> Address {
    let h = Hash::sha256(name.as_bytes());
    probekonto(h.as_bytes()[0])
}

/// Der Schlüssel zu [`konto_fuer`].
///
/// Getrennte Funktion und kein zweites Ableiten: Wer hier etwas ändert,
/// ändert es für Adresse und Schlüssel zugleich, und die beiden können
/// nicht auseinanderlaufen.
pub fn schluessel_fuer(name: &str) -> myl_types::bls::BlsSecretKey {
    let h = Hash::sha256(name.as_bytes());
    probeschluessel(h.as_bytes()[0])
}

/// Was beim Übernehmen eines Blocks schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KettenFehler {
    /// Der Block schließt nicht an die eigene Kette an.
    ///
    /// **Der häufigste Fall im Betrieb**, und meistens harmlos: Ein
    /// Knoten, der später dazukommt, hat die früheren Blöcke nie
    /// gesehen.
    PasstNichtAn { erwartet: Hash, bekommen: Hash },
    /// Derselbe Block noch einmal. Gossip verbreitet mehrfach.
    SchonBekannt,
    /// **Die Zustandswurzel weicht ab.**
    ///
    /// Der schwerste Fall und der einzige, der ein Befund ist: Zwei
    /// Maschinen haben aus denselben Blöcken verschiedene Zustände
    /// errechnet. Irgendwo im Ledger-Pfad ist etwas nicht
    /// deterministisch.
    ZustandWeichtAb { erwartet: Hash, errechnet: Hash },
    /// **Die Höhe im Kopf passt nicht zur Kette.**
    ///
    /// Der Vorgängerhash bindet die Kette schon; diese Prüfung hält das
    /// Höhenfeld ehrlich. Ein Block, der anschließt und eine falsche
    /// Höhe nennt, wäre sonst gültig und schickte jeden Nachzügler in
    /// die Irre, denn an der Höhe zählt er seine Lücke ab.
    HoeheWeichtAb { erwartet: u64, bekommen: u64 },
    /// **Die Epoche im Kopf folgt nicht aus der Höhe.**
    ///
    /// Die Epoche ist eine Funktion der Höhe
    /// (`myl_consensus::block::epoche_fuer_hoehe`). Sie steht trotzdem
    /// im Kopf, damit ein Block für sich lesbar bleibt — geprüft wird
    /// sie, weil an ihr der Verfall von Credits und das Fenster der
    /// Verstoßhistorie hängen.
    EpocheWeichtAb { erwartet: u64, bekommen: u64 },
}

impl std::fmt::Display for KettenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PasstNichtAn { erwartet, bekommen } => write!(
                f,
                "schließt nicht an: erwartet {}, bekommen {}",
                &erwartet.to_hex()[..16],
                &bekommen.to_hex()[..16]
            ),
            Self::SchonBekannt => write!(f, "bereits übernommen"),
            Self::ZustandWeichtAb { erwartet, errechnet } => write!(
                f,
                "Zustandswurzel weicht ab: Block sagt {}, errechnet {}",
                &erwartet.to_hex()[..16],
                &errechnet.to_hex()[..16]
            ),
            Self::HoeheWeichtAb { erwartet, bekommen } => write!(
                f,
                "Höhe weicht ab: erwartet {erwartet}, Block sagt {bekommen}"
            ),
            Self::EpocheWeichtAb { erwartet, bekommen } => write!(
                f,
                "Epoche folgt nicht aus der Höhe: erwartet {erwartet}, Block sagt {bekommen}"
            ),
        }
    }
}

impl std::error::Error for KettenFehler {}

/// Die Kette eines Knotens: Zustand, Höhe, Mempool.
pub struct Kette {
    hoehe: u64,
    letzter_hash: Hash,
    zustand: LedgerState,
    /// Die zuletzt gezogene Stichprobe, mit der Epoche, zu der sie
    /// gehört (Punkt 45).
    ///
    /// ⛑ **Hier stand zuerst ein blanker `Vec`, und er wurde von jedem
    /// Block überschrieben.** Eine Ziehung gehört zu einer **Epoche**,
    /// und zwischen zwei Epochenwechseln liegen viele Blöcke; jeder
    /// setzte sie auf leer zurück. Der Test fiel sofort um, und das war
    /// die richtige Antwort.
    ///
    /// ⚑ **Nicht im `LedgerState`, und das ist Absicht.** Die Ziehung ist
    /// eine **reine Funktion** aus den bezeugten Bündeln, der Epoche und
    /// dem Blockhash; sie in den Zustand zu schreiben wäre eine zweite
    /// Quelle für dieselbe Aussage, und `commitment()` serialisiert den
    /// ganzen Zustand je Block (D7). Wer sie braucht, rechnet sie nach.
    ///
    /// Hier steht sie nur, damit der Knoten sie **protokollieren** kann:
    /// Eine Ziehung, die niemand sieht, ist von keiner Ziehung nicht zu
    /// unterscheiden, und das war Fund 114.
    letzte_stichprobe: Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    mempool: Vec<Transaktion>,
    /// Die Hashes der übernommenen Blöcke, damit Dubletten aus dem
    /// Gossip nicht zweimal angewandt werden.
    bekannt: std::collections::HashSet<Hash>,
    /// Die Blöcke selbst, nach Höhe.
    ///
    /// **Wer nachliefern soll, muss aufheben.** Ohne diesen Speicher
    /// könnte ein Knoten einem Neuling nicht helfen, und der Rückstand
    /// wäre endgültig. Für einen Probelauf im Speicher zu halten ist
    /// vertretbar; ein echtes Netz legt sie auf die Platte, und das
    /// ist ein offener Punkt.
    verlauf: std::collections::BTreeMap<u64, Block>,
    /// Das Blockprotokoll auf der Platte, falls eines geführt wird.
    ///
    /// ⚑ **Der Speicher gehört hierher und nicht zum Aufrufer.** Die
    /// Zusage lautet „jeder Block, der in die Kette kommt, steht auch in
    /// der Datei". Läge das Schreiben beim Aufrufer, wäre sie an drei
    /// Aufrufstellen verteilt, und die vierte, die jemand später
    /// hinzufügt, vergäße es. Hier kann sie es nicht.
    speicher: Option<crate::speicher::Kettenspeicher>,
    /// Wie oft das Schreiben fehlschlug.
    ///
    /// **Ein Schreibfehler macht den Block nicht ungültig**, also wird
    /// er angenommen. Er macht aber die Datei unvollständig, und das
    /// darf nicht still bleiben: Der Zähler geht in jede
    /// Zustandsaufnahme.
    schreibfehler: u64,
}

impl Kette {
    /// Eine frische Probekette.
    ///
    /// **Alle Knoten müssen beim selben Wert beginnen**, sonst weichen
    /// ihre Zustandswurzeln von Anfang an ab, und das sähe aus wie ein
    /// Determinismusfehler. Deshalb ist [`PROBE_STARTWERT`] eine
    /// Konstante und kein Zufallswert.
    ///
    /// **Nicht zu verwechseln mit einem Genesis.** Ein Genesis ist ein
    /// einmaliges Ereignis mit Folgen; das hier entsteht bei jedem
    /// Start neu.
    pub fn probestand() -> Self {
        let mut zustand = LedgerState::genesis(PROBE_CREDIT_PREIS);
        // Die Testkonten ausstatten. Ohne Guthaben scheitert jede
        // Burn-Transaktion, der Zustand bleibt unverändert, und die
        // Übereinstimmung der Wurzeln belegt nichts (siehe
        // [`probekonto`]).
        for n in 0..PROBEKONTEN {
            zustand.account_mut(&probekonto(n)).balance = PROBEGUTHABEN;
        }
        Self {
            hoehe: 0,
            // Nicht das Literal wiederholen: Zwei Stellen mit derselben
            // Zeichenkette laufen irgendwann auseinander, und dann
            // lehnte die Kettendatei ihre eigene Kette ab.
            letzter_hash: Self::startwert(),
            zustand,
            letzte_stichprobe: None,
            mempool: Vec::new(),
            bekannt: std::collections::HashSet::new(),
            verlauf: std::collections::BTreeMap::new(),
            speicher: None,
            schreibfehler: 0,
        }
    }

    /// Der Startwert dieser Kette.
    ///
    /// Bindet eine Kettendatei an ihr Netz: Eine Datei mit anderem
    /// Startwert wird abgewiesen, statt eine fremde Historie als eigene
    /// auszugeben.
    pub fn startwert() -> Hash {
        Hash::sha256(b"myelith-testkette-genesis")
    }

    /// Hängt ein Blockprotokoll an diese Kette.
    ///
    /// Ab jetzt wird jeder aufgenommene Block geschrieben. **Blöcke, die
    /// vorher schon in der Kette waren, werden nicht nachgetragen**: Der
    /// vorgesehene Weg ist, den Speicher vor dem ersten Block zu setzen
    /// und die Datei danach nachzuspielen.
    pub fn speicher_setzen(&mut self, speicher: crate::speicher::Kettenspeicher) {
        self.speicher = Some(speicher);
    }

    /// Wie oft das Schreiben fehlschlug.
    pub fn schreibfehler(&self) -> u64 {
        self.schreibfehler
    }

    /// Wie viele Blöcke die Datei führt, falls eine geführt wird.
    pub fn gespeicherte_bloecke(&self) -> Option<u64> {
        self.speicher.as_ref().map(|s| s.bloecke())
    }

    /// Schreibt einen Block, falls ein Speicher da ist.
    fn schreibe(&mut self, block: &Block) {
        let Some(s) = self.speicher.as_mut() else {
            return;
        };
        if s.anhaengen(block).is_err() {
            self.schreibfehler += 1;
        }
    }

    /// Die Höhe, die dieser Knoten selbst führt.
    pub fn hoehe(&self) -> u64 {
        self.hoehe
    }

    /// Der Hash des zuletzt übernommenen Blocks.
    pub fn letzter_hash(&self) -> Hash {
        self.letzter_hash
    }

    /// Die Zustandswurzel: der Wert, der zwischen den Knoten
    /// übereinstimmen muss.
    pub fn zustandswurzel(&self) -> Hash {
        self.zustand.commitment()
    }

    /// Entfernt aus dem Mempool, was in einem Block angekommen ist.
    ///
    /// Vergleich über die serialisierten Bytes: Zwei Transaktionen sind
    /// dieselbe, wenn sie sich gleich schreiben. Ein Feldvergleich wäre
    /// dasselbe mit mehr Code, und bei einem neuen Feld würde er still
    /// falsch.
    ///
    /// **Je Vorkommen eines**, nicht alle gleichen auf einmal:
    /// Dieselbe Transaktion zweimal eingereicht ist zweimal eingereicht,
    /// und wenn ein Block sie einmal enthält, wartet die andere weiter.
    fn streiche_verarbeitete(&mut self, verarbeitet: &[Transaktion]) {
        if self.mempool.is_empty() || verarbeitet.is_empty() {
            return;
        }
        let mut zu_streichen: Vec<Vec<u8>> = verarbeitet
            .iter()
            .filter_map(|t| borsh::to_vec(t).ok())
            .collect();
        self.mempool.retain(|t| {
            let Ok(bytes) = borsh::to_vec(t) else {
                return true;
            };
            match zu_streichen.iter().position(|z| z == &bytes) {
                Some(i) => {
                    zu_streichen.swap_remove(i);
                    false
                }
                None => true,
            }
        });
    }

    /// Die Blöcke der Höhen `ab` bis einschließlich `bis`, soweit
    /// vorhanden, aufsteigend.
    ///
    /// Lücken werden **übersprungen, nicht aufgefüllt**: Wer nur einen
    /// Teil hat, liefert diesen Teil. Der Fragende merkt es daran, dass
    /// sein Rückstand nicht ganz verschwindet, und fragt weiter.
    pub fn bloecke_von_bis(&self, ab: u64, bis: u64) -> Vec<Block> {
        self.verlauf
            .range(ab..=bis)
            .map(|(_, b)| b.clone())
            .collect()
    }

    /// Der Zustand zum Ändern, **nur für Tests und den Betreiber**.
    ///
    /// ⚑ **Kein Weg für Teilnehmer.** Wer den Zustand von außen ändern
    /// kann, umgeht jede Übergangsprüfung; deshalb steht hier
    /// ausdrücklich, wofür das da ist. Alles, was ein Absender darf,
    /// geht über eine Anweisung und über `anwenden`.
    #[doc(hidden)]
    pub fn zustand_mut(&mut self) -> &mut LedgerState {
        &mut self.zustand
    }

    /// Der Ledger-Zustand, für Diagnose und Tests.
    pub fn zustand(&self) -> &LedgerState {
        &self.zustand
    }

    /// Anzahl wartender Transaktionen.
    pub fn wartend(&self) -> usize {
        self.mempool.len()
    }

    /// Nimmt eine Transaktion in den Mempool.
    ///
    /// Ohne Prüfung: Ob sie durchgeht, entscheidet sich beim Anwenden.
    /// Eine Vorprüfung hier bräuchte den Zustand **zum Zeitpunkt der
    /// Aufnahme**, und der ist nicht der beim Anwenden.
    pub fn aufnehmen(&mut self, tx: Transaktion) {
        self.mempool.push(tx);
    }

    /// Liest eine Transaktion aus Gossip-Bytes und nimmt sie auf.
    pub fn aufnehmen_roh(&mut self, daten: &[u8]) -> bool {
        let mut rest = daten;
        match Transaktion::deserialize(&mut rest) {
            Ok(tx) if rest.is_empty() => {
                self.aufnehmen(tx);
                true
            }
            _ => false,
        }
    }

    /// Die Prägeparameter dieser Testkette.
    ///
    /// ⚑ **Fest und nicht aus Governance**, solange die Kette eine
    /// Probekette ist. Ein Parameter, der sich ändern kann, gehört an
    /// eine Stelle, an der beide Seiten dieselbe Änderung sehen; die
    /// Registry ist noch nicht an die Kette gebunden. Bis dahin ist ein
    /// fester Wert ehrlicher als ein beweglicher, den nur einer kennt.
    ///
    /// Subventionsrate null: Die Anlaufkurve ist ein
    /// Governance-Gegenstand (`myl_tokenomics::Subventionsplan`) und
    /// gehört nicht als Zahl in eine Testkette.
    fn praegeparameter() -> myl_tokenomics::MintParams {
        myl_tokenomics::MintParams {
            subsidy_num: 0,
            subsidy_den: 1,
            m_max: u64::MAX,
        }
    }

    /// Setzt die Arbeitsverteilung der Pod-Positionen.
    ///
    /// # ⚑ Kein Weg über eine Transaktion, und das ist Absicht
    ///
    /// Die Gewichte sind ein **Governance-Gegenstand**: Sie folgen aus
    /// dem Pipeline-Stand, und wer sie setzen darf, entscheidet eine
    /// Abstimmung. Der Draht von einem angenommenen Beschluss hierher
    /// fehlt, wie bei der Belastung der Treasury.
    ///
    /// **Solange er fehlt, gibt es keine Anweisung dafür.** Eine wäre
    /// schlimmer als keine: Sie stünde jedem Absender offen, und wer die
    /// Gewichte setzt, setzt die Verteilung des Ertrags. Dieser Weg ist
    /// dem Betreiber des Knotens vorbehalten und niemandem sonst.
    pub fn arbeitsverteilung_setzen(
        &mut self,
        verteilung: myl_types::arbeitsverteilung::Arbeitsverteilung,
    ) -> Result<(), myl_ledger::transitions::TransitionError> {
        myl_ledger::transitions::arbeitsverteilung_setzen(&mut self.zustand, verteilung)
    }

    /// Die Mitgliedschaft eines Pods, wie sie die Aggregatprüfung
    /// braucht (Punkt 40, Glied 2).
    ///
    /// # ⚑ Der Weg vom Register zur Signaturprüfung, und er fehlte
    ///
    /// `myl_consensus::poi::verify_bundle_signature` ist seit Langem
    /// gebaut und geprüft und **wurde von der Kette nie gerufen**, weil
    /// ihr die öffentlichen Schlüssel der Pod-Mitglieder fehlten:
    /// `MinerId` ist `SHA-256` über den Schlüssel, und aus einem Hash
    /// folgt kein Urbild. Seit das Register den Schlüssel führt, liegen
    /// sie vor.
    ///
    /// **Diese Funktion steht hier und nicht im Scheduler**, denn der
    /// bildet Pods und soll von Bündeln nichts wissen; und nicht in
    /// `myl-consensus`, der die Zuteilung nicht kennt. Der Knoten ist
    /// die einzige Stelle, die beides sieht.
    ///
    /// **Der Koordinator ist das Mitglied auf Position null**, also der
    /// erste Shard der Pipeline.
    fn mitgliedschaft(
        pod: &myl_scheduler::shard_assignment::Pod,
        epoche: u64,
    ) -> Option<myl_consensus::poi::PodMembership> {
        let erste = pod.shards.first()?;
        let mitglieder: Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> =
            pod.mitglieder().map(|m| (m.miner_id, m.schluessel)).collect();
        myl_consensus::poi::PodMembership::ohne_besitznachweis(
            EpochId(epoche),
            myl_types::pod_kennung(epoche, pod.pod_index),
            erste.miner.miner_id,
            mitglieder,
        )
        .ok()
    }

    /// Wie viele Shard-Positionen ein Pod dieser Probekette hat.
    ///
    /// ⚑ **Fest, solange die Kette eine Probekette ist.** Der Zuschnitt
    /// gehört ins Pipeline-Manifest und damit unter Governance; ein
    /// beweglicher Wert, den nur einer kennt, wäre schlimmer als ein
    /// fester, den beide Seiten sehen.
    const PROBE_SHARDS: u32 = 4;

    /// Leitet die Zuschreibung der abzurechnenden Epoche ab.
    ///
    /// # Der Weg, und er ist jetzt vollständig
    ///
    /// Register → Zuteilung → Bündel → Anteile. Jeder Schritt ist eine
    /// **Ableitung**, keine Angabe: Wer im Register steht, sagt die
    /// Kette; wer in welchem Pod sitzt, folgt aus Register, Zone und
    /// Blockhash; welcher Pod ein Bündel eingereicht hat, folgt aus der
    /// abgeleiteten Pod-Kennung; und wie sich die vTFE eines Pods auf
    /// seine Positionen teilt, sagt die Arbeitsverteilung.
    ///
    /// # ⚑ Ohne Arbeitsverteilung wird nichts zugeschrieben
    ///
    /// Dann bleibt die Zuschreibung leer, der Shard-Miner-Anteil
    /// ungeprägt, und das ist die sichere Richtung: **Lieber nichts
    /// ausschütten als nach einer Gewichtung, die niemand gesetzt hat.**
    ///
    /// # ⚑ Ein Bündel ohne passenden Pod fällt still weg
    ///
    /// Es nennt eine Kennung, die zu keiner Platznummer dieser Epoche
    /// gehört. Das ist der Normalfall bei einem gefälschten Bündel; es
    /// zu verwerfen ist richtig, und es **den Block verwerfen zu
    /// lassen wäre falsch**, denn dann hielte eine einzige Fälschung die
    /// Kette an.
    fn zuschreibung_der_epoche(
        zustand: &LedgerState,
        letzter_hash: &Hash,
    ) -> (myl_tokenomics::Zuschreibung, Vec<myl_types::PoIBundle>) {
        let Some(verteilung) = zustand.arbeitsverteilung.clone() else {
            return (myl_tokenomics::Zuschreibung::default(), Vec::new());
        };
        let epoche = zustand.epoch.0;
        let register = angemeldete_miner(zustand);
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            epoche,
            letzter_hash,
            Self::PROBE_SHARDS,
        );

        let mut abrechnungen = Vec::new();
        let mut bezeugt: Vec<myl_types::PoIBundle> = Vec::new();
        for buendel in buendel_der_epoche(zustand) {
            let Some(pod) =
                myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, epoche, &buendel.pod)
            else {
                continue;
            };
            // ⚑ **Und jetzt wird die Aggregatsignatur geprüft**
            // (Punkt 40, Glied 2). Die Mitgliedschaft kommt aus der
            // **Zuteilung**, nie aus dem Bündel: Wer sie aus dem Bündel
            // nähme, ließe den Einreicher bestimmen, gegen welche
            // Schlüssel geprüft wird.
            let Some(mitgliedschaft) = Self::mitgliedschaft(pod, epoche) else {
                continue;
            };
            if myl_consensus::poi::verify_bundle_signature(&buendel, &mitgliedschaft).is_err() {
                continue;
            }
            abrechnungen.push(myl_tokenomics::Podabrechnung {
                positionen: pod
                    .shards
                    .iter()
                    .map(|s| (s.miner.miner_id, s.shard_index))
                    .collect(),
                reserve: pod.reserve.iter().map(|m| m.miner_id).collect(),
                vtfe_pod: buendel.vtfe_claimed,
            });
            bezeugt.push(buendel.clone());
        }
        (
            myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, &abrechnungen)
                .unwrap_or_default(),
            bezeugt,
        )
    }

    /// Die zuletzt gezogene Stichprobe (Punkt 45).
    ///
    /// Leer, solange keine Epoche abgeschlossen wurde. **Sie wird
    /// gerechnet, nicht gespeichert**; dieser Zugang gibt nur weiter,
    /// was der letzte Abschluss ergab, damit der Knoten es
    /// protokollieren kann.
    pub fn letzte_stichprobe(&self) -> &[crate::stichprobe::Segmentstichprobe] {
        self.letzte_stichprobe.as_ref().map_or(&[], |(_, s)| s.as_slice())
    }

    /// Zu welcher Epoche die letzte Ziehung gehört, wenn es eine gibt.
    pub fn stichprobenepoche(&self) -> Option<u64> {
        self.letzte_stichprobe.as_ref().map(|(e, _)| *e)
    }

    /// Der Pod zu einer Bündel-Kennung, aus der Zuteilung **der
    /// gefragten Epoche**.
    ///
    /// ⚑ **Nicht aus der laufenden.** Ein Checker fragt zu einer
    /// abgeschlossenen Epoche, und die Zuteilung hängt an ihr: Wer die
    /// heutige nähme, bekäme andere Mitglieder und damit andere
    /// Adressen. Der Blockhash ist derselbe wie bei der Ziehung, weil
    /// beide dieselbe Zuteilung meinen müssen.
    pub fn pod_der_kennung(
        &self,
        epoche: u64,
        kennung: &myl_types::ids::PodId,
    ) -> Option<myl_scheduler::shard_assignment::Pod> {
        let register = angemeldete_miner(&self.zustand);
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            epoche,
            &self.letzter_hash,
            Self::PROBE_SHARDS,
        );
        myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, epoche, kennung).cloned()
    }

    /// Die Stichprobenrate, in Basispunkten.
    ///
    /// **200 bp sind zwei Prozent**, der Wert aus Kap. 3.4 und dem
    /// Zahlenbeispiel in Anhang B.1. Er ist ein Governance-Parameter und
    /// steht hier, bis Governance ihn setzt; **eine andere Zahl wäre
    /// erfunden**.
    pub const STICHPROBE_BP: u32 = 200;

    /// Zieht die Stichprobe der abgeschlossenen Epoche (Punkt 45).
    ///
    /// # ⚑ Warum diese Zeile der eigentliche Fund ist
    ///
    /// `sample_segments` und `check_segment` waren seit dem 2026-08-17
    /// gebaut, geprüft und abgehakt, und **sie hatten bis zum
    /// 2026-09-01 null Aufrufer** (Fund 114). Damit lief Stufe 2 der
    /// Verifikation in keinem Knoten, und die Sicherheitsbedingung aus
    /// Anhang B.1 hängt an genau der Wahrscheinlichkeit, die dabei null
    /// war.
    ///
    /// **Gezogen wird nur aus Bündeln, deren Aggregatsignatur gilt.**
    /// Wer ein Bündel einreicht, das die Prüfung nicht besteht, bekommt
    /// nichts gutgeschrieben und soll auch keinen Indexraum aufblähen
    /// dürfen: Sonst verdünnte eine Flut ungültiger Bündel die
    /// Stichprobenrate der ehrlichen.
    ///
    /// ⚑ **Die Saat ist heute der Blockhash, und das ist die offene
    /// Hälfte.** Wer den Abschlussblock erzeugt, kann Kandidaten
    /// probieren, bis die Ziehung seine eigenen Segmente verschont. Das
    /// Ziel ist die Aggregatsignatur des Komitees; sie steht hier
    /// deshalb als **Argument** und nicht als Ableitung im Modul.
    fn stichprobe_der_epoche(
        bezeugt: &[myl_types::PoIBundle],
        epoche: u64,
        letzter_hash: &Hash,
    ) -> Vec<crate::stichprobe::Segmentstichprobe> {
        let saat = crate::stichprobe::stichprobensaat(letzter_hash, epoche);
        crate::stichprobe::stichprobe_der_epoche(bezeugt, &saat, Self::STICHPROBE_BP)
    }

    /// Schließt die vorige Epoche ab, wenn eine neue beginnt (Punkt 38).
    ///
    /// # Was hier geschieht und was nicht
    ///
    /// Der Aufruf geht an [`myl_tokenomics::epochenausschuettung`]: Der
    /// Knoten rechnet die Prägung **nicht selbst**, er ruft sie. Die
    /// Formeln gehören in die Wirtschaft, der Aufruf in die Kette.
    ///
    /// ⚑ **Die Zuschreibung ist heute leer, und das ist kein
    /// Versehen.** Sie leitet sich aus bestätigten PoI-Bündeln ab, und
    /// **diese Kette trägt keine**: `Anweisung` kennt Burn, Überweisung
    /// und die drei Sitzungsanweisungen, kein Bündel. Ohne
    /// bezeugte Arbeit gibt es nichts zuzuschreiben.
    ///
    /// **Die Folge ist die sichere:** Der Shard-Miner-Anteil wird
    /// **nicht geprägt**, weil ihm kein Empfänger gegenübersteht, und
    /// das Ergebnis benennt es. Geprägt wird allein der Treasury-Anteil.
    /// Die Geldmenge wächst also um das, was ankommt, und um sonst
    /// nichts.
    ///
    /// **Was damit noch fehlt**, ist eine Anweisung, die ein Bündel in
    /// die Kette trägt, samt ihrer Prüfung. Das ist der nächste Draht
    /// und keine Rechnung mehr.
    ///
    /// # Warum ein Fehlschlag den Block nicht verwirft
    ///
    /// Scheitert der Abschluss, wird er **übersprungen**, wie eine
    /// gescheiterte Transaktion. Beide Seiten überspringen dasselbe,
    /// weil beide dieselbe Funktion durchlaufen; ein Abbruch hier
    /// hielte die Kette an, und zwar bei allen gleichzeitig.
    fn epochenwechsel_abschliessen(
        zustand: &mut LedgerState,
        neue_epoche: u64,
        letzter_hash: &Hash,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        if neue_epoche <= zustand.epoch.0 {
            return;
        }
        let alte_epoche = zustand.epoch.0;
        let (zuschreibung, bezeugt) = Self::zuschreibung_der_epoche(zustand, letzter_hash);
        let _ = myl_tokenomics::epochenausschuettung(
            zustand,
            &zuschreibung,
            &Self::praegeparameter(),
        );
        // ⚑ **Stufe 2 wird gezogen, und zwar hier** (Punkt 45, Fund
        // 114). Vorher lief sie in keinem Knoten, und `p` aus Anhang
        // B.1 war null. Gezogen wird aus den **bezeugten** Bündeln,
        // also denen, deren Aggregatsignatur galt.
        //
        // ⚑ **Das Nachrechnen fehlt noch**, denn es braucht die Spur des
        // Segments, und die liegt beim Koordinator. Was hier steht, ist
        // die Ziehung; sie **allein** macht `p` von null verschieden und
        // ist die Vorbedingung für alles Weitere.
        let stichprobe = Self::stichprobe_der_epoche(&bezeugt, alte_epoche, letzter_hash);
        *gezogen = Some((alte_epoche, stichprobe));
        // ⚑ **Und die Bündel der abgerechneten Epoche fallen weg.**
        // Ohne dies wüchse der Zustand unbegrenzt, und Entscheidung D7
        // wäre gebrochen; die Historie steht in den Blöcken.
        //
        // ⚑ **Heute werden sie verworfen, ohne zugeschrieben zu
        // werden**, weil dafür die Pod-Besetzung im Zustand fehlt
        // (Glied 3c). Verloren geht dabei nichts, denn es wurde für sie
        // auch nichts geprägt; **gewonnen ist nur, dass der Weg in die
        // Kette jetzt steht**.
        let _ = buendel_leeren(zustand);
    }


    /// Wendet Transaktionen auf den Zustand an.
    ///
    /// **Die einzige Stelle, an der das geschieht**, und das ist der
    /// Kern: Erzeuger und Übernehmer müssen dieselbe Folge von
    /// Änderungen ausführen, sonst weichen ihre Zustandswurzeln ab,
    /// ohne dass etwas kaputt wäre. Zwei Fassungen dieser Funktion wären
    /// zwei Quellen für dieselbe Aussage (vgl. Fund 34).
    ///
    /// Gescheiterte Transaktionen werden **übersprungen, nicht
    /// abgebrochen**: Eine Burn-Transaktion ohne Deckung ist kein Grund,
    /// den Block zu verwerfen, und beide Seiten überspringen sie gleich.
    ///
    /// # ⚑ Und hier wechselt die Epoche (Punkt 38)
    ///
    /// Wächst `epoch` gegenüber dem Zustand, wird die **vorige** Epoche
    /// abgeschlossen, bevor die neue gilt: geglätteten Burn
    /// fortschreiben, prägen, verteilen, gutschreiben. Das geschieht
    /// **hier und nirgends sonst**, aus demselben Grund, aus dem das
    /// Anwenden hier steht: Erzeuger und Übernehmer müssen dieselbe
    /// Folge von Änderungen ausführen. Ein Abschluss, den nur der
    /// Erzeuger rechnet, wäre eine abweichende Zustandswurzel.
    fn anwenden(
        zustand: &mut LedgerState,
        txs: &[Transaktion],
        epoch: u64,
        letzter_hash: &Hash,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        Self::epochenwechsel_abschliessen(zustand, epoch, letzter_hash, gezogen);
        // ⚑ **Die Epoche des Zustands wird mitgeführt** (seit
        // 2026-08-27). Vorher stand sie auf 0 und blieb dort: `anwenden`
        // benutzte die Epoche nur, um den Verfall auszurechnen, und
        // niemand setzte `zustand.epoch`. Damit lief jede Prüfung, die
        // an der laufenden Epoche hängt, gegen 0 — der Verfall von
        // Credits ebenso wie das Fenster der Verstoßhistorie.
        //
        // Sie steht **vor** dem Anwenden, weil die Übergänge sie lesen.
        zustand.epoch = EpochId(epoch);
        let netz = Self::startwert();
        for tx in txs {
            // ⚑ **Die Unterschrift wird hier geprüft und nicht erst bei
            // der Aufnahme in den Mempool** (2026-08-28, Fund 85). Ein
            // Block kommt über Gossip und sieht den Mempool nie; läge
            // die Prüfung dort, könnte ein Leader eine unsignierte
            // Anweisung in einen Block schreiben, und die ehrlichen
            // Knoten wendeten sie an.
            //
            // **Erzeuger und Übernehmer überspringen dasselbe**, weil
            // beide dieselbe Funktion durchlaufen.
            let Ok(geprueft) = tx.clone().pruefe(&netz) else {
                continue;
            };
            let absender = geprueft.inhalt().absender_adresse();

            // ⚑ **Die Nummer wird auch dann verbraucht, wenn die
            // Anweisung danach scheitert.** Sonst wäre eine ungedeckte
            // Überweisung unverändert gültig und beliebig oft
            // einreichbar.
            if nonce_verbrauchen(zustand, &absender, geprueft.inhalt().nonce).is_err() {
                continue;
            }

            // Gescheiterte Anweisungen werden übersprungen, nicht
            // abgebrochen: Eine Überweisung ohne Deckung ist kein Grund,
            // den Block zu verwerfen.
            match &geprueft.inhalt().anweisung {
                Anweisung::Burn { betrag } => {
                    // Verfall eine Epoche später: eine Festlegung dieser
                    // Testkette, damit beide Seiten dieselbe rechnen.
                    let _ = burn_to_credits(zustand, &absender, *betrag, EpochId(epoch + 1));
                }
                Anweisung::Ueberweisung { nach, betrag } => {
                    let _ = transfer(zustand, &absender, nach, *betrag);
                }
                Anweisung::SitzungEroeffnen { kontrakt } => {
                    let _ = sitzung_eroeffnen(zustand, &absender, kontrakt.clone());
                }
                // ⚑ Die Kennung folgt aus dem Absender und steht nicht
                // in der Anweisung: Beide sind derselbe Schlüssel, und
                // ein zweites Feld ließe sich abweichend füllen.
                // ⚑ Der Schlüssel kommt aus der **geprüften**
                // Transaktion, nicht aus einem Feld der Anweisung.
                // Damit ist der Besitz bewiesen: Wer unterschreiben
                // kann, hält den geheimen Teil.
                Anweisung::MinerAnmelden {
                    hardware,
                    zone,
                    netzadresse,
                } => {
                    let kennung = myl_types::ids::MinerId::new(*absender.as_bytes());
                    let _ = miner_anmelden(
                        zustand,
                        &absender,
                        &kennung,
                        *hardware,
                        *zone,
                        geprueft.inhalt().absender,
                        *netzadresse,
                    );
                }
                Anweisung::MinerAbmelden => {
                    let kennung = myl_types::ids::MinerId::new(*absender.as_bytes());
                    let _ = miner_abmelden(zustand, &absender, &kennung);
                }
                Anweisung::BuendelEinreichen { buendel } => {
                    let _ = buendel_einreichen(zustand, &absender, buendel.clone());
                }
                Anweisung::SitzungWiderrufen { sitzung } => {
                    let _ = sitzung_widerrufen(zustand, sitzung, &absender);
                }
                Anweisung::SitzungAusgeben { vorhaben } => {
                    let _ = sitzung_ausgeben(zustand, &absender, vorhaben);
                }
            }
        }
    }

    /// Baut den nächsten Block aus dem Mempool und übernimmt ihn selbst.
    ///
    /// Der Erzeuger wendet **zuerst** an und schreibt die daraus
    /// entstandene Zustandswurzel in den Block. Die Empfänger rechnen
    /// dasselbe und vergleichen.
    pub fn baue_block(&mut self) -> Block {
        let txs: Vec<Transaktion> = std::mem::take(&mut self.mempool);
        let hoehe = self.hoehe + 1;
        let epoch = epoche_fuer_hoehe(hoehe);

        // ⚑ **Nur wenn ein Epochenwechsel stattfand**, sonst löschte
        // jeder Block die Ziehung der laufenden Epoche.
        let mut gezogen = None;
        Self::anwenden(&mut self.zustand, &txs, epoch, &self.letzter_hash, &mut gezogen);
        if gezogen.is_some() {
            self.letzte_stichprobe = gezogen;
        }

        let mut block = Block::new(BlockHeader {
            height: hoehe,
            epoch,
            prev_block_hash: self.letzter_hash,
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: self.zustand.commitment(),
        });
        for tx in txs {
            block.add_transaction(tx);
        }

        self.hoehe = hoehe;
        self.letzter_hash = block.hash();
        self.bekannt.insert(self.letzter_hash);
        self.verlauf.insert(hoehe, block.clone());
        self.schreibe(&block);
        block
    }

    /// Übernimmt einen fremden Block und rechnet nach.
    ///
    /// Die Reihenfolge der Prüfungen ist bedeutsam: **Erst anschließen,
    /// dann rechnen.** Ein Block, der nicht anschließt, darf den Zustand
    /// nicht anfassen, sonst hinterlässt ein verirrter Block eine Spur.
    pub fn uebernimm(&mut self, block: &Block) -> Result<(), KettenFehler> {
        let hash = block.hash();
        if self.bekannt.contains(&hash) {
            return Err(KettenFehler::SchonBekannt);
        }
        if block.header.prev_block_hash != self.letzter_hash {
            return Err(KettenFehler::PasstNichtAn {
                erwartet: self.letzter_hash,
                bekommen: block.header.prev_block_hash,
            });
        }
        // **Die Höhe wächst um genau eins.** Der Vorgängerhash bindet
        // die Kette schon; die Höhe zusätzlich zu prüfen kostet nichts
        // und hält ein Feld ehrlich, an dem die Nachlieferung ihre
        // Lücke abzählt. Ein Kopf mit falscher Höhe wäre sonst ein
        // gültiger Block, der einen Nachzügler in die Irre schickt.
        let erwartete_hoehe = self.hoehe + 1;
        if block.header.height != erwartete_hoehe {
            return Err(KettenFehler::HoeheWeichtAb {
                erwartet: erwartete_hoehe,
                bekommen: block.header.height,
            });
        }
        // **Und die Epoche folgt aus der Höhe.** Sie steht mit im Kopf,
        // damit ein Block für sich lesbar bleibt; geprüft wird sie
        // trotzdem, sonst wäre sie ein Feld, das jeder setzen darf und
        // niemand nachrechnet — und daran hängen Verfall und Fristen.
        let erwartete_epoche = epoche_fuer_hoehe(block.header.height);
        if block.header.epoch != erwartete_epoche {
            return Err(KettenFehler::EpocheWeichtAb {
                erwartet: erwartete_epoche,
                bekommen: block.header.epoch,
            });
        }

        // Auf einer Kopie rechnen: Weicht das Ergebnis ab, bleibt der
        // eigene Zustand unberührt. Ein Knoten, der einem abweichenden
        // Block folgt, hätte den Befund verschluckt.
        let mut versuch = self.zustand.clone();
        let mut gezogen = None;
        Self::anwenden(
            &mut versuch,
            &block.txs,
            block.header.epoch,
            &self.letzter_hash,
            &mut gezogen,
        );
        let errechnet = versuch.commitment();
        if errechnet != block.header.state_root {
            return Err(KettenFehler::ZustandWeichtAb {
                erwartet: block.header.state_root,
                errechnet,
            });
        }

        self.zustand = versuch;
        self.hoehe = block.header.height;
        self.letzter_hash = hash;
        self.bekannt.insert(hash);
        self.verlauf.insert(block.header.height, block.clone());
        // ⚑ Was der Block enthält, wartet nicht mehr.
        //
        // Ohne diese Zeile wächst der Mempool eines Knotens, der selbst
        // keine Blöcke baut, **für immer**: Er nimmt jede Transaktion
        // aus dem Gossip auf und leert nie. Gemessen im
        // Dreiknoten-Probelauf: 0, 0, 3, 4 wartende Einträge über
        // vierzig Sekunden, und das ohne Ende.
        //
        // Zwei Folgen, und die zweite ist die schlimmere: Die Zahl
        // `wartend` im Protokoll wird bedeutungslos, **und ein solcher
        // Knoten baute, sobald er je Erzeuger würde, einen Block aus
        // tausenden längst verarbeiteter Transaktionen.**
        self.streiche_verarbeitete(&block.txs);
        self.schreibe(block);
        Ok(())
    }
}

impl Default for Kette {
    fn default() -> Self {
        Self::probestand()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::Anweisung;

    /// Ein **wirksamer** Burn: Absender ist ein ausgestattetes
    /// Testkonto. Ein Burn von einem leeren Konto scheitert still, und
    /// ein Test damit belegte nichts.
    ///
    /// ⚑ **Und er ist unterschrieben** (2026-08-28). Ohne Unterschrift
    /// verwirft `anwenden` ihn, der Zustand bewegte sich nicht, und der
    /// Test belegte wieder nichts — dieselbe Falle wie beim fehlenden
    /// Guthaben, nur eine Ebene weiter.
    fn burn_nr(wer: u8, betrag: u64, nonce: u64) -> Transaktion {
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(wer),
            nonce,
            Anweisung::Burn { betrag },
        )
        .expect("signieren")
    }

    fn burn(wer: u8, betrag: u64) -> Transaktion {
        burn_nr(wer, betrag, 0)
    }

    /// ⚑ **Die Gegenprobe zu Fund 85: Eine Anweisung ohne gültige
    /// Unterschrift wirkt nicht.**
    ///
    /// Bis zum 2026-08-28 trug eine Transaktion keine Unterschrift, und
    /// jeder konnte im Namen jedes Kontos anweisen. Bei `Burn` war das
    /// Sachbeschädigung, bei einer Überweisung wäre es Diebstahl
    /// gewesen.
    #[test]
    fn eine_gefaelschte_unterschrift_bewegt_nichts() {
        let mut k = Kette::probestand();
        let vorher = k.zustandswurzel();

        let mut tx = burn(0, 5_000);
        tx.signatur = myl_types::bls::BlsSignature([7u8; 96]);
        k.aufnehmen(tx);
        let _ = k.baue_block();
        assert_eq!(k.zustandswurzel(), vorher, "eine Fälschung darf nichts bewegen");

        // Und der Schlüssel eines anderen Kontos hilft auch nicht: Die
        // Adresse folgt aus dem Schlüssel, also belastet er sein eigenes
        // Konto und nicht das fremde.
        let echt = burn(1, 5_000);
        let konto_eins_vorher = k.zustand().account(&probekonto(1)).balance;
        let konto_null_vorher = k.zustand().account(&probekonto(0)).balance;
        k.aufnehmen(echt);
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&probekonto(0)).balance, konto_null_vorher);
        assert!(k.zustand().account(&probekonto(1)).balance < konto_eins_vorher);
    }

    /// ⚑ Dieselbe Transaktion zweimal wirkt einmal.
    #[test]
    fn eine_wiedereingespielte_transaktion_wirkt_nicht_zweimal() {
        let mut k = Kette::probestand();
        let tx = burn(2, 3_000);

        k.aufnehmen(tx.clone());
        let _ = k.baue_block();
        let nach_dem_ersten = k.zustand().account(&probekonto(2)).balance;
        assert_eq!(k.zustand().account(&probekonto(2)).nonce, 1);

        k.aufnehmen(tx);
        let _ = k.baue_block();
        assert_eq!(
            k.zustand().account(&probekonto(2)).balance,
            nach_dem_ersten,
            "dieselben Bytes ein zweites Mal duerfen nichts bewirken"
        );
        assert_eq!(k.zustand().account(&probekonto(2)).nonce, 1);

        // Mit der naechsten Nummer geht es weiter.
        k.aufnehmen(burn_nr(2, 3_000, 1));
        let _ = k.baue_block();
        assert!(k.zustand().account(&probekonto(2)).balance < nach_dem_ersten);
    }

    /// ⚑ Eine Ueberweisung ueber einen Block: Fund 83 geschlossen.
    #[test]
    fn eine_ueberweisung_kommt_ueber_einen_block_an() {
        use myl_consensus::block::Anweisung;
        let mut k = Kette::probestand();
        let von = probekonto(3);
        let nach = probekonto(4);
        let (v0, n0) = (
            k.zustand().account(&von).balance,
            k.zustand().account(&nach).balance,
        );

        let tx = Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(3),
            0,
            Anweisung::Ueberweisung { nach, betrag: 2_500 },
        )
        .expect("signieren");
        k.aufnehmen(tx);
        let _ = k.baue_block();

        assert_eq!(k.zustand().account(&von).balance, v0 - 2_500);
        assert_eq!(k.zustand().account(&nach).balance, n0 + 2_500);
    }

    /// ⚑ Eine Session ueber Bloecke: eroeffnen, ausgeben, widerrufen.
    /// **Der Kontrakt begrenzt, und zwar auf der Kette.**
    #[test]
    fn eine_session_wirkt_und_begrenzt_ueber_bloecke() {
        use myl_consensus::block::Anweisung;
        use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Vorhaben, Waehrung};

        let mut k = Kette::probestand();
        let inhaber = probekonto(6);
        let agent = probekonto(7);
        let empfaenger = probekonto(0);

        let kontrakt = Sitzungskontrakt::neu(
            inhaber,
            agent,
            Grenzen::gesperrt(),
            Grenzen { budget: 5_000, einzellimit: 2_000, schwelle: u64::MAX, zeugenleiter: Vec::new() },
            vec![empfaenger],
            EpochId(0),
            EpochId(u64::MAX),16,
        )
        .expect("gueltiger Kontrakt");
        let id = kontrakt.adresse();

        let sig = |n: u8, nonce: u64, a: Anweisung| {
            Transaktion::signiere(&Kette::startwert(), &probeschluessel(n), nonce, a)
                .expect("signieren")
        };

        k.aufnehmen(sig(6, 0, Anweisung::SitzungEroeffnen { kontrakt }));
        let _ = k.baue_block();
        assert!(k.zustand().sitzung(&id).is_some(), "die Session steht im Zustand");

        let zahlung = |betrag: u64| Vorhaben {
            sitzung: id,
            handelnder: agent,
            waehrung: Waehrung::Myl,
            betrag,
            empfaenger,
            bestaetigt_ausgeliefert: false,
        };
        let empf_vorher = k.zustand().account(&empfaenger).balance;
        let inh_vorher = k.zustand().account(&inhaber).balance;

        // Ueber dem Einzellimit: wirkungslos.
        k.aufnehmen(sig(7, 0, Anweisung::SitzungAusgeben { vorhaben: zahlung(3_000) }));
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&empfaenger).balance, empf_vorher);

        // Darunter: es fliesst, und zwar vom Konto des Inhabers.
        k.aufnehmen(sig(7, 1, Anweisung::SitzungAusgeben { vorhaben: zahlung(1_500) }));
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&empfaenger).balance, empf_vorher + 1_500);
        assert_eq!(k.zustand().account(&inhaber).balance, inh_vorher - 1_500);

        // Der Agent kann nicht widerrufen, der Inhaber schon.
        k.aufnehmen(sig(7, 2, Anweisung::SitzungWiderrufen { sitzung: id }));
        let _ = k.baue_block();
        assert!(!k.zustand().sitzung(&id).expect("da").zustand.widerrufen);

        k.aufnehmen(sig(6, 1, Anweisung::SitzungWiderrufen { sitzung: id }));
        let _ = k.baue_block();
        assert!(k.zustand().sitzung(&id).expect("da").zustand.widerrufen);

        // Und danach fliesst nichts mehr.
        let nach_widerruf = k.zustand().account(&empfaenger).balance;
        k.aufnehmen(sig(7, 3, Anweisung::SitzungAusgeben { vorhaben: zahlung(100) }));
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&empfaenger).balance, nach_widerruf);
    }

    /// **Der Zustand ändert sich, wenn Transaktionen wirken.**
    ///
    /// Der Test, der beim ersten echten Dreiknotenlauf gefehlt hat.
    /// Damals stand die Zustandswurzel bei jeder Höhe auf demselben
    /// Wert: Die Burns scheiterten an fehlender Deckung, wurden
    /// übersprungen, und die Übereinstimmung der Wurzeln belegte nichts.
    /// **Ein Lauf, dessen Zustand sich nie ändert, misst nichts.**
    #[test]
    fn ein_wirksamer_burn_veraendert_die_zustandswurzel() {
        let mut k = Kette::probestand();
        let vorher = k.zustandswurzel();
        k.aufnehmen(burn(0, 5_000));
        let _ = k.baue_block();
        assert_ne!(
            k.zustandswurzel(),
            vorher,
            "die Zustandswurzel hat sich nicht bewegt: die Transaktion war wirkungslos"
        );
    }

    #[test]
    fn die_testkonten_haben_zu_genesis_guthaben() {
        let k = Kette::probestand();
        for n in 0..PROBEKONTEN {
            assert_eq!(
                k.zustand().account(&probekonto(n)).balance,
                PROBEGUTHABEN,
                "Testkonto {n} ohne Guthaben"
            );
        }
    }

    #[test]
    fn das_konto_eines_knotens_bleibt_stabil() {
        // Ein wechselndes Konto machte zwei Läufe unvergleichbar.
        assert_eq!(konto_fuer("alpha"), konto_fuer("alpha"));
        assert!(
            (0..PROBEKONTEN).any(|n| probekonto(n) == konto_fuer("alpha")),
            "das Konto liegt außerhalb der ausgestatteten"
        );
    }

    // --- Höhe und Epoche ------------------------------------------

    /// **Die Höhe wächst je Block, die Epoche nicht.**
    ///
    /// Der Kern von Punkt 7. Bis zum 2026-08-27 war beides dasselbe
    /// Feld; ein Block war eine Epoche, und jede Frist „je Epoche"
    /// bedeutete in Wahrheit „je Block".
    #[test]
    fn die_hoehe_waechst_je_block_die_epoche_nicht() {
        let mut k = Kette::probestand();
        for erwartet in 1..=5u64 {
            let b = k.baue_block();
            assert_eq!(b.header.height, erwartet, "die Höhe zählt nicht mit");
            assert_eq!(b.header.epoch, 0, "die Epoche ist mitgewandert");
            assert_eq!(k.hoehe(), erwartet);
        }
    }

    /// **Und an der Epochengrenze springt sie, genau einmal.**
    ///
    /// Ein Test, der nur die ersten Blöcke ansieht, bestünde auch dann,
    /// wenn die Epoche **nie** wechselte — und eine Epoche, die nie
    /// wechselt, ist dieselbe Doppelbelegung mit umgekehrtem Vorzeichen.
    #[test]
    fn an_der_epochengrenze_springt_die_epoche() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = Kette::probestand();
        let mut wechsel = 0usize;
        let mut vorige = 0u64;
        for _ in 0..(BLOECKE_JE_EPOCHE + 2) {
            let b = k.baue_block();
            if b.header.epoch != vorige {
                wechsel += 1;
                assert_eq!(
                    b.header.height, BLOECKE_JE_EPOCHE,
                    "der Wechsel liegt nicht auf der Grenze"
                );
                vorige = b.header.epoch;
            }
        }
        assert_eq!(wechsel, 1, "die Epoche wechselte {wechsel}-mal statt einmal");
        assert_eq!(vorige, 1);
    }

    /// **Die Epoche des Ledger-Zustands wandert mit.**
    ///
    /// Sie stand bis zum 2026-08-27 auf 0 und blieb dort: `anwenden`
    /// benutzte die Epoche nur zum Rechnen des Verfalls und setzte sie
    /// nie. Jede Prüfung, die an der laufenden Epoche hängt — der
    /// Verfall von Credits, das Fenster der Verstoßhistorie —, lief
    /// damit gegen null.
    #[test]
    fn die_epoche_des_zustands_wandert_mit() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = Kette::probestand();
        assert_eq!(k.zustand().epoch.0, 0);
        for _ in 0..BLOECKE_JE_EPOCHE {
            let _ = k.baue_block();
        }
        assert_eq!(
            k.zustand().epoch.0,
            1,
            "der Zustand kennt die Epoche des Blocks nicht"
        );
    }

    /// Ein Block mit falscher Höhe wird abgelehnt, auch wenn er sonst
    /// anschließt.
    #[test]
    fn eine_falsche_hoehe_wird_abgelehnt() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let mut block = erzeuger.baue_block();
        block.header.height = 7;
        assert!(matches!(
            folger.uebernimm(&block),
            Err(KettenFehler::HoeheWeichtAb { erwartet: 1, bekommen: 7 })
        ));
        // Und der Zustand blieb unberührt.
        assert_eq!(folger.hoehe(), 0);
    }

    /// Eine Epoche, die nicht aus der Höhe folgt, wird abgelehnt.
    ///
    /// **Die Gegenprobe steht daneben:** Derselbe Block mit der
    /// richtigen Epoche geht durch. Ein Test, der nur ablehnt, bestünde
    /// auch dann, wenn gar nichts mehr angenommen würde.
    #[test]
    fn eine_epoche_die_nicht_aus_der_hoehe_folgt_wird_abgelehnt() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let block = erzeuger.baue_block();

        let mut gefaelscht = block.clone();
        gefaelscht.header.epoch = 9;
        assert!(matches!(
            folger.uebernimm(&gefaelscht),
            Err(KettenFehler::EpocheWeichtAb { erwartet: 0, bekommen: 9 })
        ));
        assert_eq!(folger.hoehe(), 0);

        folger.uebernimm(&block).expect("der echte Block muss durchgehen");
        assert_eq!(folger.hoehe(), 1);
    }

    /// Zwei Ketten aus demselben Genesis sind gleich.
    #[test]
    fn zwei_frische_ketten_stimmen_ueberein() {
        // Wäre das nicht so, sähe jeder Lauf wie ein
        // Determinismusfehler aus, bevor überhaupt etwas passiert ist.
        let a = Kette::probestand();
        let b = Kette::probestand();
        assert_eq!(a.zustandswurzel(), b.zustandswurzel());
        assert_eq!(a.letzter_hash(), b.letzter_hash());
        assert_eq!(a.hoehe(), 0);
    }

    /// **Der Kern: Erzeuger und Übernehmer landen beim selben Zustand.**
    #[test]
    fn erzeuger_und_uebernehmer_kommen_zum_selben_zustand() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();

        erzeuger.aufnehmen(burn(1, 1_000));
        erzeuger.aufnehmen(burn(2, 2_500));
        let block = erzeuger.baue_block();

        folger.uebernimm(&block).expect("Block muss anschließen");
        assert_eq!(
            erzeuger.zustandswurzel(),
            folger.zustandswurzel(),
            "die Zustandswurzeln weichen ab"
        );
        assert_eq!(erzeuger.hoehe(), folger.hoehe());
        assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
    }

    /// Mehrere Blöcke hintereinander bleiben gleich.
    #[test]
    fn eine_kette_aus_mehreren_bloecken_bleibt_gleich() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        for i in 1..=10u64 {
            erzeuger.aufnehmen(burn((i % 5) as u8 + 1, i * 300));
            let b = erzeuger.baue_block();
            folger.uebernimm(&b).expect("schließt an");
        }
        assert_eq!(erzeuger.hoehe(), 10);
        assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
    }

    /// Ein Block, der nicht anschließt, lässt den Zustand unberührt.
    #[test]
    fn ein_verirrter_block_hinterlaesst_keine_spur() {
        let mut a = Kette::probestand();
        let mut fremd = Kette::probestand();
        fremd.aufnehmen(burn(9, 5_000));
        let _ = fremd.baue_block();
        fremd.aufnehmen(burn(9, 5_000));
        let zweiter = fremd.baue_block(); // schließt an fremds ersten an

        let vorher = a.zustandswurzel();
        let fehler = a.uebernimm(&zweiter).expect_err("darf nicht anschließen");
        assert!(matches!(fehler, KettenFehler::PasstNichtAn { .. }));
        assert_eq!(a.zustandswurzel(), vorher, "der Zustand wurde angefasst");
        assert_eq!(a.hoehe(), 0);
    }

    /// **Eine abweichende Zustandswurzel wird erkannt und nicht
    /// übernommen.**
    ///
    /// Der Fall, für den es diesen Aufbau gibt. Ein Knoten, der einem
    /// abweichenden Block folgte, hätte den Befund verschluckt.
    #[test]
    fn eine_abweichende_zustandswurzel_wird_zurueckgewiesen() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 1_000));
        let mut block = erzeuger.baue_block();

        // Jemand fälscht die Wurzel.
        block.header.state_root = Hash::sha256(b"etwas anderes");

        let vorher = folger.zustandswurzel();
        let fehler = folger.uebernimm(&block).expect_err("muss auffallen");
        assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
        assert_eq!(folger.zustandswurzel(), vorher, "Zustand wurde übernommen");
    }

    /// Gossip verbreitet mehrfach; derselbe Block darf nicht zweimal wirken.
    #[test]
    fn ein_doppelt_empfangener_block_wirkt_nur_einmal() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 1_000));
        let block = erzeuger.baue_block();

        folger.uebernimm(&block).expect("erstes Mal");
        let nach_erstem = folger.zustandswurzel();
        assert_eq!(
            folger.uebernimm(&block),
            Err(KettenFehler::SchonBekannt),
            "die Dublette wurde nicht erkannt"
        );
        assert_eq!(folger.zustandswurzel(), nach_erstem);
        assert_eq!(folger.hoehe(), 1);
    }

    /// Eine Transaktion ohne Deckung bricht den Block nicht ab.
    #[test]
    fn eine_ungedeckte_transaktion_wird_uebersprungen_nicht_verworfen() {
        // Beide Seiten überspringen gleich, also stimmen die Wurzeln
        // trotzdem. Bräche der Erzeuger ab und der Folger nicht, wäre
        // genau das die Abweichung.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(7, u64::MAX)); // mehr als jedes Konto hält
        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
    }

    /// **Ein Neuling holt über den Verlauf auf.**
    ///
    /// Der Fall, für den es [`crate::nachschub`] gibt: Beta verpasst
    /// die ersten Blöcke, bekommt sie nachgeliefert und steht danach
    /// beim selben Zustand wie Alpha.
    #[test]
    fn ein_nachzuegler_holt_ueber_den_verlauf_auf() {
        let mut erzeuger = Kette::probestand();
        for i in 1..=5u64 {
            erzeuger.aufnehmen(burn((i % 4) as u8, i * 400));
            let _ = erzeuger.baue_block();
        }
        // Der Neuling war die ganze Zeit nicht da.
        let mut neuling = Kette::probestand();
        assert_eq!(neuling.hoehe(), 0);

        // Er bekommt den Verlauf und wendet ihn ganz normal an.
        for b in erzeuger.bloecke_von_bis(1, 5) {
            neuling.uebernimm(&b).expect("nachgelieferter Block schließt an");
        }
        assert_eq!(neuling.hoehe(), 5);
        assert_eq!(
            neuling.zustandswurzel(),
            erzeuger.zustandswurzel(),
            "der Nachzügler steht bei einem anderen Zustand"
        );
    }

    /// Nachlieferung ist ein Transportweg, **kein Vertrauensweg**.
    #[test]
    fn ein_nachgelieferter_block_mit_falscher_wurzel_wird_genauso_abgewiesen() {
        // Wäre die Nachlieferung ein zweiter, schwächerer Weg in die
        // Kette, wäre sie das Loch: Wer einen Knoten zum Nachfordern
        // bringt, bekäme einen Block hineingelegt, ohne dass er
        // nachrechnet.
        let mut erzeuger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 900));
        let _ = erzeuger.baue_block();
        let mut geliefert = erzeuger.bloecke_von_bis(1, 1);
        geliefert[0].header.state_root = Hash::sha256(b"untergeschoben");

        let mut neuling = Kette::probestand();
        let fehler = neuling.uebernimm(&geliefert[0]).expect_err("muss auffallen");
        assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
        assert_eq!(neuling.hoehe(), 0);
    }

    #[test]
    fn der_verlauf_liefert_nur_was_da_ist() {
        // Lücken werden übersprungen, nicht aufgefüllt. Der Fragende
        // merkt es daran, dass sein Rückstand nicht ganz verschwindet.
        let mut k = Kette::probestand();
        for _ in 0..3 {
            let _ = k.baue_block();
        }
        assert_eq!(k.bloecke_von_bis(1, 3).len(), 3);
        assert_eq!(k.bloecke_von_bis(1, 99).len(), 3, "nicht Vorhandenes wird nicht erfunden");
        assert!(k.bloecke_von_bis(7, 9).is_empty());
    }

    /// **Der Mempool eines Nicht-Erzeugers wächst nicht ins Unendliche.**
    ///
    /// Der Fund aus dem Dreiknoten-Probelauf: Beta nahm jede
    /// Transaktion aus dem Gossip auf und leerte nie, weil es selbst
    /// keine Blöcke baut.
    #[test]
    fn was_in_einem_block_ankommt_wartet_nicht_mehr() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();

        // Beide sehen dieselben Transaktionen aus dem Gossip.
        for i in 1..=3u64 {
            let t = burn((i % 4) as u8, i * 500);
            erzeuger.aufnehmen(t.clone());
            folger.aufnehmen(t);
        }
        assert_eq!(folger.wartend(), 3);

        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(
            folger.wartend(),
            0,
            "der Mempool wurde nicht geleert: er wüchse für immer"
        );
    }

    #[test]
    fn was_nicht_im_block_stand_wartet_weiter() {
        // Sonst verschwänden Transaktionen, die noch niemand
        // verarbeitet hat.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let drin = burn(1, 700);
        erzeuger.aufnehmen(drin.clone());
        folger.aufnehmen(drin);
        folger.aufnehmen(burn(2, 800)); // kennt der Erzeuger nicht

        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(folger.wartend(), 1, "die unverarbeitete Transaktion ist weg");
    }

    #[test]
    fn eine_doppelt_eingereichte_transaktion_verschwindet_nur_einmal() {
        // Dieselbe Transaktion zweimal eingereicht ist zweimal
        // eingereicht. Enthält ein Block sie einmal, wartet die andere.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let t = burn(3, 600);
        erzeuger.aufnehmen(t.clone());
        folger.aufnehmen(t.clone());
        folger.aufnehmen(t);
        assert_eq!(folger.wartend(), 2);

        let block = erzeuger.baue_block();
        assert_eq!(block.txs.len(), 1);
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(folger.wartend(), 1);
    }

    /// Ein leerer Block ist gültig und bewegt die Kette weiter.
    #[test]
    fn ein_leerer_block_bewegt_die_kette() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let block = erzeuger.baue_block();
        assert!(block.txs.is_empty());
        folger.uebernimm(&block).expect("auch leer schließt an");
        assert_eq!(folger.hoehe(), 1);
        assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
    }

    /// Der Mempool wird beim Bauen geleert.
    #[test]
    fn der_mempool_wird_beim_bauen_geleert() {
        // Sonst stünde dieselbe Transaktion in jedem folgenden Block.
        let mut k = Kette::probestand();
        k.aufnehmen(burn(1, 500));
        assert_eq!(k.wartend(), 1);
        let b = k.baue_block();
        assert_eq!(b.txs.len(), 1);
        assert_eq!(k.wartend(), 0);
        assert!(k.baue_block().txs.is_empty());
    }

    /// Rohe Gossip-Bytes werden gelesen, Unsinn nicht.
    #[test]
    fn rohe_transaktionen_werden_gelesen_unsinn_nicht() {
        let mut k = Kette::probestand();
        let gut = borsh::to_vec(&burn(3, 900)).unwrap();
        assert!(k.aufnehmen_roh(&gut));
        assert_eq!(k.wartend(), 1);
        assert!(!k.aufnehmen_roh(&[0xFF; 7]));
        // Anhängsel: dieselbe Transaktion mit anderem Rahmen.
        let mut mit_anhang = gut.clone();
        mit_anhang.push(0);
        assert!(!k.aufnehmen_roh(&mit_anhang));
        assert_eq!(k.wartend(), 1);
    }

    // --- Punkt 38: der Epochenabschluss in der Kette ---

    /// Summe aller Guthaben, um das Wachstum der Geldmenge zu messen.
    fn geldmenge(k: &Kette) -> u128 {
        k.zustand().accounts.values().map(|a| a.balance as u128).sum()
    }

    /// ⚑ **Punkt 38, die Kernaussage in der Kette:** An der
    /// Epochengrenze wird abgeschlossen, und ein Konto wächst.
    #[test]
    fn an_der_epochengrenze_wird_abgeschlossen() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = Kette::probestand();
        // Erst verbrennen, damit es eine Bemessungsgrundlage gibt.
        k.aufnehmen(burn(0, 1_000_000));
        k.baue_block();
        assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

        let vor = geldmenge(&k);
        for _ in 1..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        assert_eq!(k.zustand().burn_epoche, 0, "der Zaehler wurde nicht zurueckgesetzt");
        assert!(k.zustand().burn_ema > 0, "der geglaettete Wert blieb null");
        assert!(geldmenge(&k) > vor, "es wurde nichts gepraegt");
    }

    /// ⚑ **Geprägt wird allein der Treasury-Anteil**, weil dieser Kette
    /// keine bezeugte Arbeit vorliegt. Der Shard-Miner-Anteil bleibt
    /// ungeprägt, statt irgendwohin zu fließen.
    #[test]
    fn ohne_bezeugte_arbeit_bekommt_nur_das_treasury() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = Kette::probestand();
        k.aufnehmen(burn(0, 1_000_000));
        k.baue_block();
        let vor = geldmenge(&k);
        for _ in 1..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        let treasury = myl_types::treasury::treasury_adresse();
        let auf_treasury = k.zustand().account(&treasury).balance as u128;
        assert!(auf_treasury > 0, "das Treasury ging leer aus");
        assert_eq!(
            geldmenge(&k) - vor,
            auf_treasury,
            "es wuchs mehr als der Treasury-Anteil"
        );
    }

    /// Innerhalb einer Epoche wird nicht abgeschlossen.
    #[test]
    fn innerhalb_einer_epoche_wird_nicht_abgeschlossen() {
        let mut k = Kette::probestand();
        k.aufnehmen(burn(0, 1_000_000));
        for _ in 0..5 {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 0);
        assert!(
            k.zustand().burn_epoche > 0,
            "der Zaehler wurde mitten in der Epoche zurueckgesetzt"
        );
    }

    /// ⚑ **Der entscheidende Test: Erzeuger und Übernehmer kommen zur
    /// selben Zustandswurzel.**
    ///
    /// Ein Abschluss, den nur der Erzeuger rechnet, wäre eine
    /// abweichende Wurzel und damit ein Konsensbruch. Er steht deshalb
    /// in `anwenden`, das beide Seiten durchlaufen; dieser Test hält es
    /// über eine Epochengrenze hinweg fest.
    ///
    /// ⚑ **Er prüft Übereinstimmung, nicht Richtigkeit.** Beide Seiten
    /// laufen durch denselben Code; ein Abschluss, der falsch rechnet,
    /// rechnet auf beiden Seiten gleich falsch, und dieser Test bliebe
    /// grün. Zwei Gegenproben haben das bestätigt: Mit ausgebautem
    /// Abschluss und mit vertauschter Reihenfolge fielen die beiden
    /// Tests darüber, dieser nicht. **Dass er nicht alles prüft, ist
    /// kein Mangel, sondern seine Aufgabe** — und es gehört
    /// aufgeschrieben, damit niemand ihn für mehr hält.
    #[test]
    fn erzeuger_und_uebernehmer_stimmen_ueber_die_epochengrenze_ueberein() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut erzeuger = Kette::probestand();
        let mut uebernehmer = Kette::probestand();
        erzeuger.aufnehmen(burn(0, 1_000_000));

        for _ in 0..=BLOECKE_JE_EPOCHE {
            let b = erzeuger.baue_block();
            uebernehmer.uebernimm(&b).expect("Uebernahme");
        }
        assert_eq!(erzeuger.zustand().epoch.0, 1, "die Grenze wurde nicht ueberschritten");
        assert_eq!(
            erzeuger.zustand().commitment(),
            uebernehmer.zustand().commitment(),
            "die Zustandswurzeln weichen ueber die Epochengrenze ab"
        );
    }

    // --- Punkt 40, Glied 3a: Anmeldung über die Kette ---

    fn anmeldung(wer: u8, hardware: myl_types::miner::HardwareClass) -> Transaktion {
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(wer),
            0,
            Anweisung::MinerAnmelden {
                hardware,
                zone: myl_types::node_metadata::GeoRegion::Europe,
                netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
            },
        )
        .expect("signieren")
    }

    /// ⚑ **Punkt 40, Glied 3a in der Kette:** Eine Anmeldung über eine
    /// Transaktion steht danach im Register.
    #[test]
    fn eine_anmeldung_ueber_die_kette_steht_im_register() {
        use myl_types::miner::HardwareClass;
        let mut k = Kette::probestand();
        k.aufnehmen(anmeldung(0, HardwareClass::MediumGpu));
        k.baue_block();
        let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());
        let eintrag = k.zustand().miner.get(&kennung).expect("angemeldet");
        assert_eq!(eintrag.hardware_class, HardwareClass::MediumGpu);
    }

    /// ⚑ **Die Zone kommt mit und steht im Zustand** (Entscheidung 3b).
    /// Sie entscheidet, in welchem Topf gemischt wird, und ist deshalb
    /// Konsensdatum und keine gegossipte Angabe.
    #[test]
    fn die_zone_kommt_mit_in_den_zustand() {
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;
        let mut k = Kette::probestand();
        let tx = Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::MinerAnmelden {
                hardware: HardwareClass::MediumGpu,
                zone: GeoRegion::Asia,
                netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
            },
        )
        .expect("signieren");
        k.aufnehmen(tx);
        k.baue_block();
        let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());
        assert_eq!(k.zustand().miner[&kennung].zone, GeoRegion::Asia);
    }

    /// ⚑ **Punkt 40, Glied 1 in der Kette:** Ein Bündel eines
    /// angemeldeten Miners kommt in den Zustand.
    #[test]
    fn ein_buendel_ueber_die_kette_kommt_an() {
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;
        let mut k = Kette::probestand();
        k.aufnehmen(anmeldung(0, HardwareClass::MediumGpu));
        k.baue_block();
        let b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::ids::PodId::new([3; 32]),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 4_200,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1,
        };
        let tx = Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            1,
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren");
        k.aufnehmen(tx);
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1);
        let _ = GeoRegion::Europe;
    }

    /// ⚑ **Und am Epochenwechsel fallen sie weg.** Ohne das wüchse der
    /// Zustand unbegrenzt; die Historie steht in den Blöcken.
    #[test]
    fn am_epochenwechsel_fallen_die_buendel_weg() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        let mut k = Kette::probestand();
        k.aufnehmen(anmeldung(0, HardwareClass::MediumGpu));
        k.baue_block();
        let b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::ids::PodId::new([3; 32]),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 4_200,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1,
        };
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                1,
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");
        for _ in 2..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        assert!(k.zustand().buendel.is_empty(), "die Buendel blieben stehen");
    }

    /// Unterschreibt ein Bündel mit **allen** Mitgliedern seines Pods.
    ///
    /// ⚑ **Allen, nicht nur den Positionen.** `PodMembership::pubkeys`
    /// liefert die Schlüssel aller Mitglieder einschließlich der
    /// Reserve, und `fast_aggregate_verify` prüft gegen genau diese
    /// Menge. Wer nur die Positionen unterschreiben ließe, bekäme ein
    /// Aggregat, das gegen mehr Schlüssel geprüft wird, als es enthält.
    fn buendel_unterschreiben(
        buendel: &mut myl_types::PoIBundle,
        pod: &myl_scheduler::shard_assignment::Pod,
    ) {
        let msg = myl_consensus::poi::bundle_message(buendel);
        let mut teile = Vec::new();
        for m in pod.mitglieder() {
            // Welcher Probeschlüssel gehört zu dieser Kennung?
            let w = (0..6u8)
                .find(|w| {
                    myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == m.miner_id
                })
                .expect("Mitglied ist ein Probekonto");
            teile.push(probeschluessel(w).sign(&msg).expect("Unterschrift"));
        }
        let aggregat =
            myl_types::bls::aggregate_signatures(&teile).expect("Aggregat");
        buendel.aggregate_sig = myl_types::bls::BlsSignature(aggregat.0);
    }

    /// ⚑ **Punkt 40 als Ganzes: bezeugte Arbeit erreicht ein Konto.**
    ///
    /// Sechs Miner melden sich an, tragen ein Auszahlungskonto ein,
    /// jemand verbrennt, ein Bündel kommt in die Kette, und am
    /// Epochenwechsel wächst das Konto eines Pod-Mitglieds.
    ///
    /// **Das ist der Test, an dem der ganze Punkt hängt.** Jeder
    /// Zwischenschritt hat seine eigenen Tests; dieser prüft, dass sie
    /// zusammen etwas ergeben.
    #[test]
    fn bezeugte_arbeit_erreicht_ein_konto() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::arbeitsverteilung::Arbeitsverteilung;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        // Gewichte für vier Positionen, das letzte Stück wiegt schwerer
        // (dort sitzt der LM-Kopf).
        k.arbeitsverteilung_setzen(
            Arbeitsverteilung::neu(Hash::sha256(b"probe-pipeline"), vec![1, 1, 1, 5])
                .expect("Verteilung"),
        )
        .expect("setzen");

        // Sechs Miner: vier Positionen plus zwei Reserve.
        let mut nonce = [0u64; 6];
        for w in 0..6u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::MinerAnmelden {
                        hardware: HardwareClass::MediumGpu,
                        zone: GeoRegion::Europe,
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        // Und Brennstoff für die Prägung.
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        assert_eq!(k.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");
        assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

        // Auszahlungskonten eintragen, damit „ohne Eintrag kein Anteil"
        // nicht greift. Die erste Eintragung darf der Miner selbst.
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            myl_ledger::transitions::auszahlungskonto_eintragen(
                k.zustand_mut(),
                &probekonto(w),
                &kennung,
                kaltes_konto(w),
            )
            .expect("Eintragung");
        }

        // Die Zuteilung dieser Epoche nachrechnen und für ihren Pod ein
        // Bündel einreichen.
        let register = myl_ledger::transitions::angemeldete_miner(k.zustand());
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            k.zustand().epoch.0,
            &k.letzter_hash(),
            4,
        );
        assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
        let pod_index = zuteilung.pods[0].pod_index;
        let mut b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::pod_kennung(k.zustand().epoch.0, pod_index),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 1_000_000,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            // Tausend Segmente, damit bei 200 bp zwanzig gezogen werden
            // und die Ziehung ueberhaupt sichtbar ist.
            segmente: 1_000,
        };
        buendel_unterschreiben(&mut b, &zuteilung.pods[0]);
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                nonce[0],
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

        // Über die Epochengrenze.
        let vorher: Vec<u64> = (0..6u8)
            .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
            .collect();
        for _ in 2..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");

        let nachher: Vec<u64> = (0..6u8)
            .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
            .collect();
        // ⚑ **Fund 114: Und die Stichprobe ist gezogen.**
        //
        // `sample_segments` und `check_segment` waren seit dem
        // 2026-08-17 gebaut, geprueft und abgehakt, und **hatten null
        // Aufrufer**. Damit war `p` aus Anhang B.1 im Betrieb null, und
        // `S_min = g/p^2` keine Schranke mehr.
        //
        // ⛑ **Geprueft wird die Ziehung, nicht das Nachrechnen.** Das
        // braucht die Spur des Segments, und die liegt beim
        // Koordinator. Diese Grenze steht hier, damit niemand den
        // gruenen Test fuer mehr haelt, als er sagt.
        let kennung = myl_types::pod_kennung(0, pod_index);
        let stichprobe = k.letzte_stichprobe();
        assert_eq!(
            stichprobe.len(),
            20,
            "200 bp von 1000 Segmenten sind zwanzig, gezogen wurden {}",
            stichprobe.len()
        );
        assert!(
            stichprobe.iter().all(|x| x.pod == kennung && x.segment < 1_000),
            "eine Ziehung zeigte ins Leere"
        );

        let gewachsen = (0..6).filter(|i| nachher[*i] > vorher[*i]).count();
        assert!(
            gewachsen > 0,
            "kein Auszahlungskonto ist gewachsen: {vorher:?} -> {nachher:?}"
        );
        assert!(
            gewachsen <= 4,
            "mehr als die vier Positionen bekamen etwas: {nachher:?}"
        );
    }

    /// ⚑ **Punkt 40, Glied 2: Ein Bündel ohne gültige Aggregatsignatur
    /// zahlt nichts aus.**
    ///
    /// Der Pod ist echt, die Kennung stimmt, die Epoche stimmt, und der
    /// Einreicher ist angemeldet. **Nur die Unterschrift der Mitglieder
    /// fehlt**, und genau das schließt die Lücke, die bis hierher offen
    /// war: Ein angemeldeter Miner konnte ein Bündel für einen fremden
    /// Pod einreichen.
    #[test]
    fn ein_buendel_ohne_gueltige_unterschrift_zahlt_nichts_aus() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::arbeitsverteilung::Arbeitsverteilung;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        k.arbeitsverteilung_setzen(
            Arbeitsverteilung::neu(Hash::sha256(b"probe-pipeline"), vec![1, 1, 1, 5])
                .expect("Verteilung"),
        )
        .expect("setzen");

        let mut nonce = [0u64; 6];
        for w in 0..6u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::MinerAnmelden {
                        hardware: HardwareClass::MediumGpu,
                        zone: GeoRegion::Europe,
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            myl_ledger::transitions::auszahlungskonto_eintragen(
                k.zustand_mut(),
                &probekonto(w),
                &kennung,
                kaltes_konto(w),
            )
            .expect("Eintragung");
        }
        let register = myl_ledger::transitions::angemeldete_miner(k.zustand());
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            k.zustand().epoch.0,
            &k.letzter_hash(),
            4,
        );
        // ⚑ Echter Pod, echte Kennung, **Attrappe als Unterschrift**.
        let b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::pod_kennung(k.zustand().epoch.0, zuteilung.pods[0].pod_index),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 1_000_000,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1,
        };
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                nonce[0],
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

        for _ in 2..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        for w in 0..6u8 {
            assert_eq!(
                k.zustand().account(&kaltes_konto(w)).balance,
                0,
                "ein Buendel ohne gueltige Unterschrift hat ausgezahlt"
            );
        }
    }

    /// ⚑ **Ohne Arbeitsverteilung wird nichts zugeschrieben**, und das
    /// ist die sichere Richtung: lieber nichts ausschütten als nach
    /// einer Gewichtung, die niemand gesetzt hat.
    ///
    /// ⛑ **Der erste Entwurf reichte kein Bündel ein** und prüfte damit
    /// „ohne Bündel keine Auszahlung" statt „ohne Verteilung keine
    /// Auszahlung". Er blieb grün, als die fehlende Verteilung durch
    /// eine erfundene ersetzt wurde: **Der Name log.** Jetzt ist alles
    /// da außer der Verteilung.
    #[test]
    fn ohne_arbeitsverteilung_bekommt_niemand_etwas() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        // Alles wie im Test darüber, **nur ohne** die Arbeitsverteilung.
        let mut nonce = [0u64; 6];
        for w in 0..6u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::MinerAnmelden {
                        hardware: HardwareClass::MediumGpu,
                        zone: GeoRegion::Europe,
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            myl_ledger::transitions::auszahlungskonto_eintragen(
                k.zustand_mut(),
                &probekonto(w),
                &kennung,
                kaltes_konto(w),
            )
            .expect("Eintragung");
        }
        let register = myl_ledger::transitions::angemeldete_miner(k.zustand());
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            k.zustand().epoch.0,
            &k.letzter_hash(),
            4,
        );
        assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
        let b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::pod_kennung(k.zustand().epoch.0, zuteilung.pods[0].pod_index),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 1_000_000,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1,
        };
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                nonce[0],
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");
        assert!(
            k.zustand().arbeitsverteilung.is_none(),
            "die Verteilung ist gesetzt, dann prueft der Test nichts"
        );

        for _ in 2..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        for w in 0..6u8 {
            assert_eq!(
                k.zustand().account(&kaltes_konto(w)).balance,
                0,
                "ohne Verteilung wurde ausgeschuettet"
            );
        }
    }

    /// ⚑ **Ein Bündel mit erfundener Pod-Kennung zahlt nichts aus.**
    ///
    /// ⛑ Ohne diesen Test war die Kennungssuche in der Kette ungeprüft:
    /// Die Gegenprobe „nimm einfach den ersten Pod" blieb grün, weil in
    /// den übrigen Tests nur **ein** Pod entsteht. Hier scheitert sie.
    ///
    /// **Das ist die Schranke gegen ein gefälschtes Bündel**, solange
    /// die Aggregatsignatur noch nicht geprüft wird: Wer eine Kennung
    /// erfindet, trifft keine Platznummer dieser Epoche.
    #[test]
    fn ein_buendel_mit_erfundener_kennung_zahlt_nichts_aus() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::arbeitsverteilung::Arbeitsverteilung;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        k.arbeitsverteilung_setzen(
            Arbeitsverteilung::neu(Hash::sha256(b"probe-pipeline"), vec![1, 1, 1, 5])
                .expect("Verteilung"),
        )
        .expect("setzen");

        let mut nonce = [0u64; 6];
        for w in 0..6u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::MinerAnmelden {
                        hardware: HardwareClass::MediumGpu,
                        zone: GeoRegion::Europe,
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            myl_ledger::transitions::auszahlungskonto_eintragen(
                k.zustand_mut(),
                &probekonto(w),
                &kennung,
                kaltes_konto(w),
            )
            .expect("Eintragung");
        }

        // ⚑ Eine Kennung, die zu keiner Platznummer dieser Epoche gehört.
        let erfunden = myl_types::ids::PodId::new([0xAB; 32]);
        let b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: erfunden,
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: 1_000_000,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1,
        };
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                nonce[0],
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

        for _ in 2..=BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        for w in 0..6u8 {
            assert_eq!(
                k.zustand().account(&kaltes_konto(w)).balance,
                0,
                "ein erfundenes Buendel hat ausgezahlt"
            );
        }
    }

    fn kaltes_konto(w: u8) -> Address {
        Address::new([200 + w; 32])
    }

    /// Die Abmeldung wirkt über die Kette ebenso.
    #[test]
    fn eine_abmeldung_ueber_die_kette_wirkt() {
        use myl_types::miner::HardwareClass;
        let mut k = Kette::probestand();
        k.aufnehmen(anmeldung(0, HardwareClass::SmallGpu));
        k.baue_block();
        let ab = Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            1,
            Anweisung::MinerAbmelden,
        )
        .expect("signieren");
        k.aufnehmen(ab);
        k.baue_block();
        assert!(k.zustand().miner.is_empty(), "die Abmeldung wirkte nicht");
    }

    /// ⚑ **Ohne gültige Unterschrift keine Anmeldung.** Dieselbe
    /// Gegenprobe wie zu Fund 85, für die neue Anweisung.
    #[test]
    fn eine_anmeldung_ohne_unterschrift_wirkt_nicht() {
        use myl_types::miner::HardwareClass;
        let mut k = Kette::probestand();
        let mut tx = anmeldung(0, HardwareClass::SmallGpu);
        // Die Unterschrift verfälschen.
        tx.signatur.0[0] ^= 0xFF;
        k.aufnehmen(tx);
        k.baue_block();
        assert!(k.zustand().miner.is_empty(), "eine verfaelschte Anmeldung wirkte");
    }

    /// Erzeuger und Übernehmer kommen auch mit Anmeldungen zur selben
    /// Wurzel.
    #[test]
    fn eine_anmeldung_aendert_bei_beiden_dieselbe_wurzel() {
        use myl_types::miner::HardwareClass;
        let mut erzeuger = Kette::probestand();
        let mut uebernehmer = Kette::probestand();
        erzeuger.aufnehmen(anmeldung(0, HardwareClass::LargeGpu));
        let b = erzeuger.baue_block();
        uebernehmer.uebernimm(&b).expect("Uebernahme");
        assert_eq!(
            erzeuger.zustand().commitment(),
            uebernehmer.zustand().commitment()
        );
    }
}

#[cfg(test)]
mod speicherbindung {
    use super::*;

    #[test]
    fn der_startwert_ist_der_erste_letzte_hash() {
        // Die Kettendatei bindet sich an diesen Wert. Wichen die beiden
        // Stellen auseinander, lehnte eine frische Datei ihre eigene
        // Kette ab, und der Knoten begänne nach jedem Neustart bei null,
        // ohne dass jemand es merkte.
        assert_eq!(Kette::probestand().letzter_hash(), Kette::startwert());
    }

    #[test]
    fn ohne_speicher_zaehlt_nichts_und_es_faellt_nichts_aus() {
        let mut k = Kette::probestand();
        assert_eq!(k.gespeicherte_bloecke(), None);
        let _ = k.baue_block();
        assert_eq!(k.schreibfehler(), 0);
        assert_eq!(k.gespeicherte_bloecke(), None);
    }
}
