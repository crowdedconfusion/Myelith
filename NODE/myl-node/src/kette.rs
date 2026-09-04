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
    transfer, miner_abmelden, miner_anmelden, buendel_einreichen, buendel_leeren, angemeldete_miner, buendel_der_epoche,
    einsatz_hinterlegen, einsatz_kuendigen, einsatz_abholen};
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

/// Wie viele Blöcke der Knoten im Arbeitsspeicher hält.
///
/// ⚑ **Hergeleitet, nicht gegriffen.** Der Zwischenspeicher hat genau
/// eine Aufgabe: eine Nachforderung beantworten, ohne die Platte
/// anzufassen. Eine Nachforderung umfasst höchstens
/// [`crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG`] Blöcke (64), also
/// deckt ein Fenster von vier Lieferungen den Fall ab, dass mehrere
/// Nachzügler gleichzeitig und an verschiedenen Stellen aufholen.
///
/// **Vier Lieferungen sind 256, und dieselbe Zahl steht in gereiften
/// Kettenknoten** als Deckel ihrer Blockzwischenspeicher. Zwei
/// verschiedene Herleitungen, dieselbe Größenordnung: Das ist ein
/// Hinweis, kein Beweis, aber ein beruhigender.
///
/// **Die Obergrenze im Speicher** ist damit benannt und nicht mehr
/// offen: 256 mal [`crate::speicher::MAX_SATZ_BYTES`] im schlimmsten
/// Fall, im Betrieb um Größenordnungen weniger.
pub const VERLAUFSFENSTER: usize = 4 * crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG as usize;

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
    /// Die Saatquelle des Blocks trägt nicht (Punkt 44).
    ///
    /// ⚑ **Eine Ablehnung und keine Warnung.** Die Saat entscheidet, wer
    /// nachgerechnet wird; eine Quelle, die niemand belegt, ist ein frei
    /// gewählter Wert und damit unbegrenzter Mahlraum. Sie durchzulassen
    /// hieße, das Feld gleich wegzulassen.
    SaatquelleTraegtNicht,
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
            Self::SaatquelleTraegtNicht => write!(
                f,
                "die Saatquelle trägt nicht: kein Zertifikat des Vorgängers"
            ),
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
    /// Der Stimmsatz, gegen den Saatquellen geprüft werden (Punkt 44).
    ///
    /// ⚑ **`None` heißt: nicht prüfbar, und das wird gemeldet.** Ein
    /// Knoten ohne Stimmsatz kann eine Aggregatsignatur nicht
    /// nachrechnen; er soll deshalb sagen, dass er es nicht tut, statt
    /// stillschweigend alles anzunehmen.
    stimmsatz: Option<myl_consensus::validator::VotingSet>,
    /// Die Saatquelle, die der nächste erzeugte Block tragen soll
    /// (Punkt 44).
    ///
    /// ⚑ **Sie kommt von außen, weil sie im Konsens entsteht.** Die
    /// Kette kennt keine Runde und kein Zertifikat; der Knoten setzt
    /// sie, sobald er eines hat. `None` heißt Blockhash, und das ist
    /// der schlechtere Rückfall (Fund 120).
    naechste_saatquelle: Option<Vec<u8>>,
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
    /// Die jüngsten Blöcke, nach Höhe. **Höchstens
    /// [`VERLAUFSFENSTER`] Stück.**
    ///
    /// **Wer nachliefern soll, muss aufheben.** Ohne diesen Speicher
    /// könnte ein Knoten einem Neuling nicht helfen, und der Rückstand
    /// wäre endgültig.
    ///
    /// ⚑ **Bis zum 2026-09-02 wuchs er unbegrenzt** (Fund 124). Ein
    /// Knoten, der lange genug lief, hielt die vollständige Kette im
    /// Arbeitsspeicher und starb an ihr, und zwar leise: erst
    /// Auslagern, dann der Abschuss durch das Betriebssystem, ohne
    /// Eintrag im eigenen Protokoll.
    ///
    /// Jetzt ist er ein **Zwischenspeicher**, und die Datei ist die
    /// Quelle. Was aus dem Fenster fällt, holt
    /// [`Kette::bloecke_von_bis`] von der Platte zurück, solange ein
    /// [`crate::speicher::Kettenspeicher`] geführt wird. Wer mit
    /// `--ohne-kette` fährt, hat nur noch das Fenster, und das steht
    /// dort ausdrücklich.
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
    /// Wie oft ein Block aus der Datei nicht zurückkam.
    ///
    /// **Zählt nur echte Lesefehler**, nicht „nicht vorhanden". Ein
    /// Nachzügler, der zu weit zurückfragt, ist kein Fehler; eine
    /// Datei, die einen Satz nicht mehr hergibt, den sie beim Öffnen
    /// noch hergab, ist einer. Auch dieser Zähler geht in jede
    /// Zustandsaufnahme.
    lesefehler: u64,
}

/// ⚑ **Dieselbe Bindung wie in `myl_pod::pipelinewerk`** (seit
/// 2026-09-03): Der Zuschnitt, gegen den die Kette Pods bildet, und die
/// Gewichte, nach denen sie auszahlt, müssen dieselbe Zahl von
/// Positionen kennen. Eine Abweichung zahlte nichts aus, ohne einen
/// Fehler zu melden.
///
/// ⚑ **Auf Modulebene und nicht im `impl`-Block**, und das ist der
/// Unterschied zwischen Prüfung und Zierde: Ein assoziiertes `const`
/// wird erst berechnet, wenn es jemand benutzt. Der erste Anlauf stand
/// im `impl`, und die Gegenprobe zeigte, dass er bei einer Abweichung
/// **nicht** ausgelöst hätte.
const _: () = assert!(
    Kette::PROBE_SHARDS as u64 == myl_tokenomics::vtfe::PROBE_SHARDS,
    "Shardzahl der Kette und der Gewichtsableitung sind auseinandergelaufen"
);

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
        // ⚑ **Die Saat der ersten beiden Epochen ist der Startwert**
        // (Fund 143). Der Ledger kennt ihn nicht, die Kette schon; ohne
        // ihn stünde dort eine Null, die in jedem Netz dieselbe wäre.
        zustand.epochensaat = Self::startwert();
        zustand.epochensaat_naechste = Self::startwert();
        Self {
            hoehe: 0,
            // Nicht das Literal wiederholen: Zwei Stellen mit derselben
            // Zeichenkette laufen irgendwann auseinander, und dann
            // lehnte die Kettendatei ihre eigene Kette ab.
            letzter_hash: Self::startwert(),
            zustand,
            stimmsatz: None,
            naechste_saatquelle: None,
            letzte_stichprobe: None,
            mempool: Vec::new(),
            bekannt: std::collections::HashSet::new(),
            verlauf: std::collections::BTreeMap::new(),
            speicher: None,
            schreibfehler: 0,
            lesefehler: 0,
        }
    }

    /// Der Startwert dieser Kette.
    ///
    /// Bindet eine Kettendatei an ihr Netz: Eine Datei mit anderem
    /// Startwert wird abgewiesen, statt eine fremde Historie als eigene
    /// auszugeben.
    pub fn startwert() -> Hash {
        // ⚑ **Aus [`PROBE_STARTWERT`], nicht aus einem eigenen Literal**
        // (Fund 163, 2026-09-03). Bis dahin stand hier
        // `sha256(b"myelith-testkette-genesis")`, und `PROBE_STARTWERT`
        // wurde **nur in Kommentaren erwähnt**: Der Doc-Kommentar
        // erklärte ihn zum „Riegel gegen eine Verwechslung mit dem
        // echten Netz" und schloss mit „Der Text sagt es auch dem, der
        // die Bytes anschaut". Nur sagte der wirklich benutzte Text
        // genau das nicht.
        Hash::sha256(PROBE_STARTWERT)
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

    /// Wie oft ein Block aus der Datei nicht zurückkam. Siehe Feld
    /// `lesefehler`.
    pub fn lesefehler(&self) -> u64 {
        self.lesefehler
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

    /// Nimmt einen Block in den Zwischenspeicher und wirft heraus,
    /// was aus dem Fenster fällt.
    ///
    /// ⚑ **`bekannt` wird mit gekürzt**, und zwar über den Hash des
    /// Blocks, der geht. Sonst wüchse die Hashmenge weiter, und das
    /// Fenster hätte nur die Hälfte des Speichers gebunden.
    ///
    /// **Was das für die Dublettenerkennung heißt:** Ein Block, der
    /// älter ist als das Fenster und ein zweites Mal ankommt, wird
    /// nicht mehr mit [`KettenFehler::SchonBekannt`] abgewiesen,
    /// sondern mit [`KettenFehler::PasstNichtAn`]. **Abgewiesen wird er
    /// so oder so**, denn seine Höhe liegt hinter der eigenen und sein
    /// Vorgängerhash passt nicht auf den letzten. Nur der Grund im
    /// Protokoll ist ein anderer, und der genauere ist ohnehin der
    /// zweite.
    fn verlauf_aufnehmen(&mut self, hoehe: u64, block: &Block) {
        self.verlauf.insert(hoehe, block.clone());
        while self.verlauf.len() > VERLAUFSFENSTER {
            let Some((_, alt)) = self.verlauf.pop_first() else {
                break;
            };
            self.bekannt.remove(&alt.hash());
        }
    }

    /// Die Blöcke der Höhen `ab` bis einschließlich `bis`, soweit
    /// vorhanden, aufsteigend.
    ///
    /// Lücken werden **übersprungen, nicht aufgefüllt**: Wer nur einen
    /// Teil hat, liefert diesen Teil. Der Fragende merkt es daran, dass
    /// sein Rückstand nicht ganz verschwindet, und fragt weiter.
    ///
    /// ⚑ **Erst das Fenster, dann die Platte** (Fund 124). Der
    /// Zwischenspeicher hält nur die jüngsten
    /// [`VERLAUFSFENSTER`] Blöcke; alles davor kommt aus der
    /// Kettendatei, falls eine geführt wird. **Wer mit `--ohne-kette`
    /// fährt, liefert nur das Fenster**, und ein Nachzügler, der weiter
    /// zurückliegt, muss einen anderen fragen. Das ist der Preis dafür,
    /// nichts zu behalten, und er steht in der Hilfe zu `--ohne-kette`.
    ///
    /// **Der häufige Fall kostet keine Platte:** Ein Nachzügler fragt
    /// die jüngsten Blöcke nach, und die stehen im Fenster.
    pub fn bloecke_von_bis(&mut self, ab: u64, bis: u64) -> Vec<Block> {
        if ab > bis {
            return Vec::new();
        }
        // ⚑ **Die Spanne wird gedeckelt, bevor irgendetwas geholt
        // wird** (Fund 141). `ab` und `bis` kommen von einer
        // Gegenstelle, und eine Gegenstelle muss sich nicht an
        // [`crate::nachschub::Nachforderung::fuer_rueckstand`] halten.
        // Ohne diesen Deckel wäre `Bloecke { ab: 0, bis: u64::MAX }`
        // eine Anfrage, die eine ganze Kette in **eine** Antwort packt,
        // also ein Verstärker: ein paar Bytes hinein, Megabyte hinaus.
        // Mehr als eine Lieferung fordert ohnehin niemand an.
        let bis = bis.min(ab.saturating_add(crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG - 1));
        // Was im Fenster liegt, ist umsonst zu haben.
        let aus_dem_fenster: Vec<Block> =
            self.verlauf.range(ab..=bis).map(|(_, b)| b.clone()).collect();
        // Die Grenze, unterhalb derer das Fenster nichts mehr weiß.
        let untergrenze = self.verlauf.keys().next().copied().unwrap_or(u64::MAX);
        if ab >= untergrenze {
            return aus_dem_fenster;
        }
        let Some(speicher) = self.speicher.as_mut() else {
            return aus_dem_fenster;
        };
        // Bis zur Untergrenze des Fensters, damit kein Block zweimal
        // in die Antwort kommt.
        let bis_datei = bis.min(untergrenze - 1);
        let mut aus_der_datei = Vec::new();
        let mut fehler = 0u64;
        for hoehe in speicher.hoehen_von_bis(ab, bis_datei) {
            match speicher.block_bei(hoehe) {
                Ok(Some(b)) => aus_der_datei.push(b),
                // Der Verweis sagte, der Satz sei da, und er ist es
                // nicht: dieselbe Lage wie ein Lesefehler.
                Ok(None) => fehler += 1,
                // Ein Lesefehler bricht die Lieferung nicht ab, denn
                // ein Teil ist besser als nichts, aber er wird gezählt.
                Err(_) => fehler += 1,
            }
        }
        self.lesefehler += fehler;
        aus_der_datei.extend(aus_dem_fenster);
        aus_der_datei
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

    // ⚑ **`arbeitsverteilung_setzen` ist am 2026-09-03 entfernt worden**
    // (Fund 161). Der Kommentar hier sagte es fast richtig: „Eine
    // Anweisung wäre schlimmer als keine, sie stünde jedem Absender
    // offen." **Was er nicht sagte:** Auch als „dem Betreiber
    // vorbehalten" war es keine Blockanwendung, sondern eine direkte
    // Mutation des Zustands ausserhalb jeder Wiederherstellung aus
    // Blöcken. Auf einem echten Knoten gerufen, hätte sie dessen
    // Zustandswurzel sofort von jeder anderen getrennt, unabhängig
    // davon, wer sie rufen durfte.
    //
    // **Die Lösung ist keine Anweisung, sondern gar kein Zustand:**
    // `myl_tokenomics::vtfe::arbeitsverteilung_probe` rechnet dieselben
    // Gewichte aus denselben öffentlichen Grunddaten, auf jedem Knoten
    // gleich, ohne dass sie je gesetzt werden müssten.

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
    /// ⚑ **Seit dem 2026-09-03 wird sie gerechnet, nicht gesetzt**
    /// (Fund 161): [`myl_tokenomics::vtfe::arbeitsverteilung_probe`].
    /// Vorher stand hier ein Zustandsfeld, das keine `Anweisung` je
    /// füllte; ohne einen Wert blieb die Zuschreibung immer leer, und
    /// ein Bündel, das im Zustand stand, wurde beim Epochenabschluss
    /// verworfen, **ohne dass je etwas geprägt wurde**. Jeder Knoten
    /// rechnet jetzt dieselbe Zahl aus denselben öffentlichen
    /// Grunddaten; nichts kann mehr auseinanderlaufen, weil nichts mehr
    /// übertragen wird.
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
    ) -> (myl_tokenomics::Zuschreibung, Vec<myl_types::PoIBundle>) {
        let verteilung = myl_tokenomics::vtfe::arbeitsverteilung_probe();
        let epoche = zustand.epoch.0;
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let _ = &verteilung;

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

    /// Setzt den Stimmsatz, gegen den Saatquellen geprüft werden.
    pub fn stimmsatz_setzen(&mut self, satz: myl_consensus::validator::VotingSet) {
        self.stimmsatz = Some(satz);
    }

    /// Prüft die Saatquelle eines Blocks (Punkt 44).
    ///
    /// # ⚑ Zwei Bindungen, und die erste wiegt schwerer
    ///
    /// 1. **Das Zertifikat gehört zum Vorgänger dieses Blocks.** Sonst
    ///    reichte ein Erzeuger ein altes oder fremdes Zertifikat ein und
    ///    hätte damit eine Saat, die er sich aussuchen kann.
    /// 2. **Die Aggregatsignatur trägt gegen den Stimmsatz.** Ohne sie
    ///    wären die Unterzeichner eine Behauptung.
    ///
    /// ⚑ **Ohne Stimmsatz wird nur die erste geprüft**, und das ist
    /// weniger, aber nicht nichts: Die Bindung an den Vorgänger allein
    /// nimmt dem Erzeuger schon jede Wahl **zwischen** Zertifikaten.
    fn saatquelle_traegt(&self, block: &Block) -> bool {
        let Some(roh) = block.header.saatquelle.as_deref() else {
            // Kein Feld heißt Blockhash als Rückfall; das ist erlaubt
            // und benannt (Fund 120).
            return true;
        };
        use borsh::BorshDeserialize;
        let mut rest = roh;
        let Ok(zert) =
            myl_consensus::round_change::Commitzertifikat::deserialize(&mut rest)
        else {
            return false;
        };
        if !rest.is_empty() {
            return false;
        }
        if zert.block_hash != block.header.prev_block_hash {
            return false;
        }
        match &self.stimmsatz {
            Some(satz) => zert.verify(satz).is_ok(),
            None => true,
        }
    }

    /// Setzt die Saatquelle für den nächsten erzeugten Block.
    ///
    /// ⚑ **Nur der Erzeuger setzt sie; alle anderen lesen sie aus dem
    /// Block.** Anders ginge es nicht: Wer sie aus eigenem Zustand
    /// nähme, zöge eine andere Stichprobe als seine Nachbarn, und wer
    /// nachgerechnet wird, ist eine Konsensentscheidung.
    pub fn saatquelle_setzen(&mut self, quelle: Vec<u8>) {
        self.naechste_saatquelle = Some(quelle);
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

    /// Die Pod-Zuteilung der laufenden Epoche.
    ///
    /// ⚑ **Die einzige Stelle, an der sie entsteht** (Fund 143). Vorher
    /// stand die Ableitung an drei Stellen, jede mit einem eigenen
    /// Blockhash, und zwei davon nahmen den **letzten** statt der
    /// Epochensaat. Zwei Herleitungen derselben Konsensgröße laufen
    /// auseinander, und diese hier taten es bereits: Der Abschluss
    /// rechnete gegen eine Zuteilung, die während der Epoche niemand
    /// kannte.
    ///
    /// **Die Saat kommt aus dem Zustand**, gilt die ganze Epoche und
    /// stammt aus `e−2`; siehe [`myl_ledger::state::LedgerState`].
    ///
    /// ⚑ **Der Rückfall ist strukturell ausgeschlossen, nicht nur
    /// getestet.** Diese Funktion sieht **nur** den Zustand, und im
    /// Zustand steht kein Blockhash: `epochensaat` und
    /// `epochensaat_naechste` wechseln ausschließlich am
    /// Epochenwechsel. Wer den alten Fehler wiederholen wollte, müsste
    /// erst einen Parameter hinzufügen, und das fällt beim Lesen auf.
    fn zuteilung_der_laufenden_epoche(
        zustand: &LedgerState,
    ) -> myl_scheduler::shard_assignment::Zuteilung {
        let register = angemeldete_miner(zustand);
        myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            zustand.epoch.0,
            &zustand.epochensaat,
            Self::PROBE_SHARDS,
        )
    }

    /// Der Pod zu einer Bündel-Kennung, aus der Zuteilung **der
    /// gefragten Epoche**.
    ///
    /// ⚑ **Nicht aus der laufenden.** Ein Checker fragt zu einer
    /// abgeschlossenen Epoche, und die Zuteilung hängt an ihr: Wer die
    /// heutige nähme, bekäme andere Mitglieder und damit andere
    /// Adressen.
    ///
    /// ⚑ **Die Grenze dieser Auskunft, und sie gehört benannt**
    /// (Fund 143): Die Saat im Zustand ist die der **laufenden**
    /// Epoche. Für eine ältere Epoche liefert diese Funktion deshalb
    /// nur dann die richtige Zuteilung, wenn `epoche` die laufende ist.
    /// **Für ältere fehlt die Saat**, und sie zu erfinden wäre
    /// schlimmer als sie zu vermissen: Der Zustand hebt zwei Saaten
    /// auf, nicht die Historie. Wer weiter zurück fragt, braucht den
    /// Block, in dem die Epoche endete.
    pub fn pod_der_kennung(
        &self,
        epoche: u64,
        kennung: &myl_types::ids::PodId,
    ) -> Option<myl_scheduler::shard_assignment::Pod> {
        if epoche != self.zustand.epoch.0 {
            return None;
        }
        let zuteilung = Self::zuteilung_der_laufenden_epoche(&self.zustand);
        myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, epoche, kennung).cloned()
    }

    /// Die Stichprobenrate, in Basispunkten.
    ///
    /// # ⚑ Abgeleitet und nicht mehr abgeschrieben (Fund 171, 2026-09-04)
    ///
    /// Hier standen **200 bp**, „der Wert aus Kap. 3.4 und dem
    /// Zahlenbeispiel in Anhang B.1". Das war richtig, solange es die
    /// **Kontrollsegmente** gab: Zwei Linien teilten sich die Arbeit,
    /// die Stichprobe `p` und die Einschleusung `gamma`.
    ///
    /// ⚑ **Entscheidung A1 hat `gamma` am 2026-09-02 entfernt.** Seither
    /// muss `p` beides tragen, und
    /// `security_sim.py::zusammengelegte_rate` rechnet die Rate aus, die
    /// beide gleichwertig ersetzt: 4,96 %, aufgerundet **fünf**. Die
    /// Registry hat den neuen Wert übernommen, **diese Konstante
    /// nicht**, und damit hat die Kette seit A1 auf eine zweite Linie
    /// vertraut, die es nicht mehr gibt. Dieselbe Klasse wie Fund 151.
    ///
    /// Sie kommt jetzt aus [`myl_tokenomics::stichprobe_bp`], der einen
    /// maßgeblichen Stelle. Wenn der Konsens die Registry liest (B10),
    /// wird auch diese Konstante überflüssig.
    pub const STICHPROBE_BP: u32 = myl_tokenomics::stichprobe_bp();

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
        saatquelle: Option<&[u8]>,
    ) -> Vec<crate::stichprobe::Segmentstichprobe> {
        // ⚑ **Die Quelle steht im Block, der Blockhash ist der
        // Rückfall.** Der Rückfall ist schlechter (unbegrenzter
        // Mahlraum statt höchstens sechzehn Bit, Fund 120) und deshalb
        // benannt statt stillschweigend.
        let saat = crate::stichprobe::stichprobensaat(
            saatquelle.unwrap_or_else(|| letzter_hash.as_bytes()),
            epoche,
        );
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
        saatquelle: Option<&[u8]>,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        if neue_epoche <= zustand.epoch.0 {
            return;
        }
        let alte_epoche = zustand.epoch.0;
        let (zuschreibung, bezeugt) = Self::zuschreibung_der_epoche(zustand);
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
        let stichprobe =
            Self::stichprobe_der_epoche(&bezeugt, alte_epoche, letzter_hash, saatquelle);
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
        // ⚑ **Und die Saat rückt weiter** (Fund 143).
        //
        // **Hinter dem Abschluss, nicht davor.** Die abgerechnete
        // Epoche musste mit der Saat abgerechnet werden, die
        // **während** ihr galt; drehte man zuerst, rechnete der
        // Abschluss gegen eine Zuteilung, die es in dieser Epoche nie
        // gab.
        //
        // Danach gilt: Die Saat der Epoche `e` ist der letzte
        // Blockhash von `e−2`, also zwei Epochen Vorlauf. Genauso weit
        // reicht der Registrierungsschluss aus Anhang A.2, und das ist
        // kein Zufall: Wäre die Saat näher als der Schluss, könnte
        // sich jemand anmelden, nachdem er sie kennt.
        zustand.epochensaat = zustand.epochensaat_naechste;
        zustand.epochensaat_naechste = *letzter_hash;
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
        saatquelle: Option<&[u8]>,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        Self::epochenwechsel_abschliessen(zustand, epoch, letzter_hash, saatquelle, gezogen);
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
        // ⚑ **Träge, nicht immer.** Die meisten Blöcke tragen kein
        // Bündel, und die Ableitung mischt das ganze Register.
        let mut zuteilung: Option<myl_scheduler::shard_assignment::Zuteilung> = None;
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
                // ⚑ **Der Einsatz** (Punkt B11, Fund 145). Bis zum
                // 2026-09-03 gab es keinen Weg, MYL zu hinterlegen;
                // `staked` war im Betrieb immer null, und die ganze
                // wirtschaftliche Sicherheitsschicht hing an einer
                // Zahl, die niemand setzte.
                Anweisung::EinsatzHinterlegen { betrag } => {
                    let _ = einsatz_hinterlegen(zustand, &absender, *betrag);
                }
                Anweisung::EinsatzKuendigen { betrag } => {
                    let _ = einsatz_kuendigen(zustand, &absender, *betrag);
                }
                Anweisung::EinsatzAbholen => {
                    let _ = einsatz_abholen(zustand, &absender);
                }
                Anweisung::BuendelEinreichen { buendel } => {
                    // ⚑ **Die Besetzung wird hier nachgeschlagen und
                    // nicht geglaubt** (Fund 144). Der Übergang prüft
                    // Koordinator und Aggregatsignatur, aber er kann
                    // die Zuteilung nicht ableiten: Der Ledger kennt
                    // den Scheduler nicht, und er soll ihn auch nicht
                    // kennen. Diese Stelle sieht beides.
                    //
                    // **Einmal je Block, nicht je Bündel.** Die
                    // Ableitung mischt das ganze Register; sie für
                    // jedes Bündel zu wiederholen wäre eine Einladung,
                    // einen Block mit Bündeln zu füllen.
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let epoche_jetzt = zustand.epoch.0;
                    // Kennt die Zuteilung diesen Pod nicht, ist das
                    // Bündel erfunden. **Übersprungen, nicht
                    // abgebrochen**, wie jede gescheiterte Anweisung.
                    let Some(pod) = myl_scheduler::zonenzuteilung::pod_zu_kennung(
                        zuteilung,
                        epoche_jetzt,
                        &buendel.pod,
                    ) else {
                        continue;
                    };
                    let Some(erster) = pod.shards.first() else {
                        continue;
                    };
                    let mitglieder: Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> =
                        pod.mitglieder().map(|m| (m.miner_id, m.schluessel)).collect();
                    let _ = buendel_einreichen(
                        zustand,
                        &absender,
                        buendel.clone(),
                        &erster.miner.miner_id,
                        &mitglieder,
                    );
                }
                Anweisung::SitzungWiderrufen { sitzung } => {
                    let _ = sitzung_widerrufen(zustand, sitzung, &absender);
                }
                Anweisung::SitzungAusgeben { vorhaben, vollmacht } => {
                    let _ = sitzung_ausgeben(zustand, &absender, vorhaben, vollmacht.as_ref());
                }
                // ⚑ **Der Weg zum kalten Konto** (Fund 167). Bis zum
                // 2026-09-03 gab es ihn nicht: Die Berechtigungsregel
                // stand seit dem 2026-09-01 fertig im Ledger, und kein
                // Block trug sie. **Ohne Eintrag kein Anteil**, also
                // hätte ein echtes Netz niemanden bezahlt, ohne dass
                // irgendwo ein Fehler entstanden wäre.
                //
                // ⚑ **Die Kennung kommt aus der Anweisung, der
                // Unterzeichner aus der geprüften Transaktion.** Wer
                // eine fremde Kennung nennt, kommt an der Regel im
                // Übergang nicht vorbei: erste Eintragung nur durch den
                // Miner selbst, jede weitere nur durch das eingetragene
                // Konto.
                Anweisung::AuszahlungskontoEintragen { kennung, konto } => {
                    let _ = myl_ledger::transitions::auszahlungskonto_eintragen(
                        zustand, &absender, kennung, *konto,
                    );
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
        let quelle = self.naechste_saatquelle.clone();
        let mut gezogen = None;
        Self::anwenden(
            &mut self.zustand,
            &txs,
            epoch,
            &self.letzter_hash,
            quelle.as_deref(),
            &mut gezogen,
        );
        if gezogen.is_some() {
            self.letzte_stichprobe = gezogen;
        }

        let mut block = Block::new(BlockHeader {
            height: hoehe,
            epoch,
            prev_block_hash: self.letzter_hash,
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: self.zustand.commitment(),
            saatquelle: quelle,
        });
        for tx in txs {
            block.add_transaction(tx);
        }

        self.hoehe = hoehe;
        self.letzter_hash = block.hash();
        self.bekannt.insert(self.letzter_hash);
        self.verlauf_aufnehmen(hoehe, &block);
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
        // ⚑ **Die Saatquelle wird geprüft, bevor gerechnet wird**
        // (Punkt 44). Sie geht in keine Zustandswurzel ein, also fiele
        // sie sonst niemandem auf, und ein frei gewählter Wert wäre
        // unbegrenzter Mahlraum.
        if !self.saatquelle_traegt(block) {
            return Err(KettenFehler::SaatquelleTraegtNicht);
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
        // ⚑ **Die Quelle kommt aus dem Block, nicht aus eigenem
        // Zustand.** Sonst zöge jeder Knoten eine andere Stichprobe.
        Self::anwenden(
            &mut versuch,
            &block.txs,
            block.header.epoch,
            &self.letzter_hash,
            block.header.saatquelle.as_deref(),
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
        self.verlauf_aufnehmen(block.header.height, block);
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

        // ⚑ **Mit steigender Nummer**, seit die Kette den Riegel gegen
        // die zweite Abbuchung führt.
        let zahlung = |betrag: u64, nummer: u64| Vorhaben {
            sitzung: id,
            handelnder: agent,
            waehrung: Waehrung::Myl,
            betrag,
            empfaenger,
            bestaetigt_ausgeliefert: false,
            nummer,
        };
        let empf_vorher = k.zustand().account(&empfaenger).balance;
        let inh_vorher = k.zustand().account(&inhaber).balance;

        // Ueber dem Einzellimit: wirkungslos.
        k.aufnehmen(sig(7, 0, Anweisung::SitzungAusgeben { vorhaben: zahlung(3_000, 1), vollmacht: None }));
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&empfaenger).balance, empf_vorher);

        // Darunter: es fliesst, und zwar vom Konto des Inhabers.
        k.aufnehmen(sig(7, 1, Anweisung::SitzungAusgeben { vorhaben: zahlung(1_500, 1), vollmacht: None }));
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
        k.aufnehmen(sig(7, 3, Anweisung::SitzungAusgeben { vorhaben: zahlung(100, 2), vollmacht: None }));
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

    /// ⚑ **Das Fenster hält, was es verspricht** (Fund 124).
    ///
    /// Bis zum 2026-09-02 wuchs `verlauf` unbegrenzt; ein Knoten, der
    /// lange genug lief, hielt die vollständige Kette im
    /// Arbeitsspeicher. Der Test baut mehr Blöcke, als das Fenster
    /// fasst, und sieht nach.
    #[test]
    fn der_verlauf_bleibt_im_fenster() {
        let mut k = Kette::probestand();
        let ueber = VERLAUFSFENSTER as u64 + 20;
        for i in 1..=ueber {
            k.aufnehmen(burn((i % 8) as u8, 100 + i));
            let _ = k.baue_block();
        }
        assert_eq!(k.hoehe(), ueber);
        assert_eq!(
            k.verlauf.len(),
            VERLAUFSFENSTER,
            "der Zwischenspeicher hält mehr, als das Fenster erlaubt"
        );
        // Und die Hashmenge wird mitgekürzt, sonst hätte das Fenster
        // nur die Hälfte des Speichers gebunden.
        assert_eq!(
            k.bekannt.len(),
            VERLAUFSFENSTER,
            "die Hashmenge wächst weiter, obwohl das Fenster kürzt"
        );
        // Die jüngsten sind da, die ältesten nicht.
        assert!(k.verlauf.contains_key(&ueber));
        assert!(!k.verlauf.contains_key(&1));
    }

    /// ⚑ **Was aus dem Fenster fällt, holt die Platte zurück**
    /// (Fund 124).
    ///
    /// Das ist die Bedingung, unter der das Fenster überhaupt zulässig
    /// ist: Ohne den Rückgriff wäre ein Nachzügler, der weiter
    /// zurückliegt als 256 Blöcke, von diesem Knoten nicht mehr zu
    /// bedienen.
    #[test]
    fn was_aus_dem_fenster_faellt_kommt_von_der_platte() {
        let d = std::env::temp_dir().join(format!(
            "myelith-fenster-{}-{}",
            std::process::id(),
            crate::protokoll::jetzt_ms()
        ));
        std::fs::create_dir_all(&d).expect("Verzeichnis");
        let p = d.join("kette.log");

        let mut k = Kette::probestand();
        let (speicher, _) =
            crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("öffnen");
        k.speicher_setzen(speicher);

        let ueber = VERLAUFSFENSTER as u64 + 20;
        for i in 1..=ueber {
            k.aufnehmen(burn((i % 8) as u8, 100 + i));
            let _ = k.baue_block();
        }
        assert_eq!(k.schreibfehler(), 0, "die Datei nahm nicht alles an");

        // Höhe 1 liegt weit unter dem Fenster.
        assert!(!k.verlauf.contains_key(&1));
        let geliefert = k.bloecke_von_bis(1, 3);
        assert_eq!(geliefert.len(), 3, "die Platte lieferte nicht nach");
        assert_eq!(geliefert[0].header.height, 1);
        assert_eq!(geliefert[2].header.height, 3);
        assert_eq!(k.lesefehler(), 0);

        // Und eine Anfrage über die Fenstergrenze hinweg liefert
        // beides, in einem Stück und aufsteigend.
        let grenze = ueber - VERLAUFSFENSTER as u64;
        let ueberlappend = k.bloecke_von_bis(grenze - 2, grenze + 2);
        let hoehen: Vec<u64> = ueberlappend.iter().map(|b| b.header.height).collect();
        assert_eq!(
            hoehen,
            vec![grenze - 2, grenze - 1, grenze, grenze + 1, grenze + 2],
            "an der Fenstergrenze fehlt oder doppelt etwas"
        );

        std::fs::remove_dir_all(&d).ok();
    }

    /// ⚑ **Ohne Kettendatei bleibt nur das Fenster**, und das ist die
    /// bewusst in Kauf genommene Folge von `--ohne-kette`.
    ///
    /// Der Test hält sie fest, damit sie niemand für einen Fehler hält.
    #[test]
    fn ohne_kettendatei_liefert_nur_das_fenster() {
        let mut k = Kette::probestand();
        let ueber = VERLAUFSFENSTER as u64 + 20;
        for i in 1..=ueber {
            k.aufnehmen(burn((i % 8) as u8, 100 + i));
            let _ = k.baue_block();
        }
        assert!(
            k.bloecke_von_bis(1, 3).is_empty(),
            "ohne Datei kann nichts von unterhalb des Fensters kommen"
        );
        assert_eq!(k.lesefehler(), 0, "nicht vorhanden ist kein Lesefehler");
    }

    /// ⚑ **Eine Nachforderung von außen kann keinen Verstärker bauen**
    /// (Fund 141).
    ///
    /// `ab` und `bis` kommen über die Leitung. Ohne Deckel packte
    /// `Bloecke { ab: 0, bis: u64::MAX }` alles, was der Knoten hat, in
    /// **eine** Antwort: ein paar Bytes hinein, Megabyte hinaus.
    #[test]
    fn eine_masslose_nachforderung_wird_gedeckelt() {
        let mut k = Kette::probestand();
        let ueber = VERLAUFSFENSTER as u64 + 20;
        for i in 1..=ueber {
            k.aufnehmen(burn((i % 8) as u8, 100 + i));
            let _ = k.baue_block();
        }
        let geliefert = k.bloecke_von_bis(0, u64::MAX);
        assert!(
            geliefert.len() as u64 <= crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG,
            "die Antwort trägt {} Blöcke, erlaubt sind {}",
            geliefert.len(),
            crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG
        );
        // Gegenprobe zum Deckel: Ohne ihn kämen hier mehr als 64
        // Blöcke, denn das Fenster hält 256.
        assert!(
            k.verlauf.len() as u64 > crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG,
            "der Test beweist nichts, wenn das Fenster kleiner ist als der Deckel"
        );
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

    /// Welches Probekonto der Koordinator dieses Pods ist.
    ///
    /// ⚑ **Der Koordinator ist Position null**, und welcher
    /// Probeschlüssel das ist, entscheidet die Saat. Ihn zu raten hieße,
    /// den Test an eine Permutation zu binden, und seit dem Mischen
    /// (Fund 142) ist es nicht mehr Konto null.
    fn koordinator_von(pod: &myl_scheduler::shard_assignment::Pod) -> u8 {
        (0..PROBEKONTEN)
            .find(|w| {
                myl_types::ids::MinerId::new(*probekonto(*w).as_bytes())
                    == pod.shards[0].miner.miner_id
            })
            .expect("der Koordinator ist ein Probekonto")
    }

    /// Baut eine Probekette mit **einem echten Pod** und liefert sie
    /// samt einem gültig unterschriebenen Bündel dafür.
    ///
    /// ⚑ **Seit Fund 144 braucht jeder Bündeltest das.** Vorher genügte
    /// eine erfundene Pod-Kennung und eine Nullsignatur: Die Aufnahme
    /// prüfte beides nicht. Jetzt prüft sie Koordinator und Aggregat,
    /// und ein Test, der das umgeht, prüfte den Weg nicht mehr, den ein
    /// echtes Bündel geht.
    ///
    /// Gibt die Kette, das Bündel und die nächste freie Nummer des
    /// Koordinators zurück.
    fn kette_mit_pod(vtfe: u64, segmente: u32) -> (Kette, myl_types::PoIBundle, u8, u64) {
        use myl_types::miner::HardwareClass;

        let mut k = Kette::probestand();
        let mut nonce = [0u64; 6];
        for w in 0..6u8 {
            k.aufnehmen(anmeldung(w, HardwareClass::MediumGpu));
            nonce[w as usize] += 1;
        }
        k.baue_block();
        assert_eq!(k.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");

        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
        let pod = &zuteilung.pods[0];
        let koordinator = koordinator_von(pod);

        let mut b = myl_types::PoIBundle {
            epoch: k.zustand().epoch,
            pod: myl_types::pod_kennung(k.zustand().epoch.0, pod.pod_index),
            segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
            vtfe_claimed: vtfe,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente,
        };
        buendel_unterschreiben(&mut b, pod);
        (k, b, koordinator, nonce[koordinator as usize])
    }

    /// Die Transaktion, die ein Bündel einreicht.
    fn einreichung(wer: u8, nonce: u64, buendel: myl_types::PoIBundle) -> Transaktion {
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(wer),
            nonce,
            Anweisung::BuendelEinreichen { buendel },
        )
        .expect("signieren")
    }

    /// ⚑ **Der Einsatz über die Kette: hinterlegen, kündigen, holen**
    /// (Punkt B11, Fund 145).
    ///
    /// Bis zum 2026-09-03 war `staked` im Betrieb immer null, weil
    /// keine Anweisung es schrieb. Der Test geht den ganzen Weg über
    /// echte, unterschriebene Transaktionen.
    #[test]
    fn ein_einsatz_geht_ueber_die_kette() {
        let mut k = Kette::probestand();
        let vorher = k.zustand().account(&probekonto(0)).balance;
        assert!(vorher > 0, "das Probekonto hat kein Guthaben");

        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                0,
                Anweisung::EinsatzHinterlegen { betrag: 5_000 },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().account(&probekonto(0)).staked,
            5_000,
            "der Einsatz kam nicht an"
        );
        assert_eq!(k.zustand().account(&probekonto(0)).balance, vorher - 5_000);

        // Kuendigen: aus `staked` heraus, aber noch nicht ins Guthaben.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                1,
                Anweisung::EinsatzKuendigen { betrag: 2_000 },
            )
            .expect("signieren"),
        );
        k.baue_block();
        let konto = k.zustand().account(&probekonto(0));
        assert_eq!(konto.staked, 3_000);
        assert_eq!(konto.balance, vorher - 5_000, "das Geld kam zu frueh zurueck");
        assert_eq!(konto.gekuendigt.values().sum::<u64>(), 2_000);

        // Abholen vor der Freigabe bewirkt nichts.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                2,
                Anweisung::EinsatzAbholen,
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().account(&probekonto(0)).balance,
            vorher - 5_000,
            "vor der Freigabe wurde ausgezahlt"
        );

        // ⚑ **Die Reife wird hier nicht geprüft, und der Grund ist
        // lehrreich.** Der erste Anlauf stellte `zustand.epoch` von
        // Hand auf die Freigabe-Epoche. Das hält nicht: `anwenden`
        // setzt die Epoche aus der **Blockhöhe**, bevor es die
        // Transaktionen anwendet, und überschreibt die gestellte
        // Zahl. **Das ist richtig so** und genau die Regel, die
        // Erzeuger und Übernehmer zusammenhält.
        //
        // Die Sperrfrist über echte Blöcke abzuwarten hiesse 168
        // Epochen zu bauen. **Die Reife steht deshalb dort, wo sie
        // hingehört**, bei den Übergängen: `gekuendigtes_liegt_bis_zur_freigabe`
        // in `myl-ledger` prüft sie samt Gegenprobe. Dieser Test hier
        // prüft, was nur er prüfen kann: dass die Anweisungen über
        // eine unterschriebene Transaktion in den Zustand gelangen.
        let konto = k.zustand().account(&probekonto(0));
        assert_eq!(konto.staked, 3_000, "der ungekuendigte Rest wurde angefasst");
    }

    /// ⚑ **Der Weg des Nutzers, von der Kette bis zum Beleg**
    /// (B6-3, Stufe 4, erster Schnitt).
    ///
    /// Ein Kontrakt kommt über eine unterschriebene Transaktion in die
    /// Kette, der Knoten frischt seine Abschrift auf, und die Tür lässt
    /// eine Vollmacht durch, die auf **diesen** Kontrakt zeigt.
    ///
    /// **Der Test bleibt in einem Prozess**, denn die Tür über einen
    /// echten Socket ist im Gateway geprüft; was hier geprüft wird, ist
    /// die **Naht**: dass der Kontrakt aus der Kette dort ankommt, wo
    /// die Tür ihn sucht. ⚑ **Genau diese Naht hat in dieser Woche
    /// siebenmal gefehlt.**
    #[test]
    fn ein_kontrakt_aus_der_kette_erreicht_die_tuer() {
        use myl_gateway::zugang::Kontraktquelle;
        use myl_types::sitzung::{Grenzen, Sitzungskontrakt};

        let mut k = Kette::probestand();
        let agent = probeschluessel(1).public_key().expect("pk");
        let kontrakt = Sitzungskontrakt {
            inhaber: probekonto(0),
            agent: Address::aus_schluessel(&agent),
            credits: Grenzen {
                budget: 10_000,
                einzellimit: 1_000,
                schwelle: u64::MAX,
                zeugenleiter: Vec::new(),
            },
            myl: Grenzen::gesperrt(),
            empfaenger: vec![probekonto(2)],
            gueltig_ab: EpochId(0),
            gueltig_bis: EpochId(100),
            max_schritte: 1_000,
        };
        let id = kontrakt.adresse();

        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                0,
                Anweisung::SitzungEroeffnen {
                    kontrakt: kontrakt.clone(),
                },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert!(
            k.zustand().sitzung(&id).is_some(),
            "der Kontrakt kam nicht in die Kette"
        );

        // Der Knoten frischt die Abschrift auf, die Tür liest sie.
        let abschrift = crate::tuer::Kontraktabschrift::neu();
        abschrift.setzen(k.zustand());
        let (gefunden, _) = abschrift
            .nachschlagen(id)
            .expect("die Tuer findet den Kontrakt nicht");
        assert_eq!(gefunden, kontrakt);

        // ⚑ **Und eine Vollmacht darauf geht durch.** Das ist der
        // ganze Weg: Kette, Abschrift, Zugangsstelle, Befund.
        let vollmacht = myl_gateway::vollmacht::Vollmacht::ausstellen(
            &probeschluessel(1),
            vec![myl_gateway::vollmacht::Vorbehalt::NurSitzung(id)],
            [5u8; 32],
        )
        .expect("ausstellen");
        let rahmen = myl_gateway::vollmacht::Anfragerahmen {
            jetzt: EpochId(0),
            sitzung: id,
            credits: 1,
            modell: Hash::sha256(b"egal"),
        };
        let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift.clone());
        assert_eq!(
            stelle.durchlassen_mit_vollmacht(&vollmacht, &rahmen, 1_000),
            myl_gateway::zugang::Zugangsbefund::Erlaubt,
            "die Vollmacht auf einen Kettenkontrakt wurde abgewiesen"
        );

        // ⚑ **Gegenprobe: ein Widerruf in der Kette schliesst die Tür**,
        // sobald die Abschrift aufgefrischt ist.
        k.zustand_mut()
            .sitzungen
            .get_mut(&id)
            .expect("da")
            .zustand
            .widerrufen = true;
        abschrift.setzen(k.zustand());
        let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift);
        assert_eq!(
            stelle.durchlassen_mit_vollmacht(&vollmacht, &rahmen, 1_000),
            myl_gateway::zugang::Zugangsbefund::Abgelehnt,
            "der Widerruf erreichte die Tuer nicht"
        );
    }

    /// ⚑ **Punkt 40, Glied 1 in der Kette:** Ein Bündel eines
    /// angemeldeten Miners kommt in den Zustand.
    #[test]
    fn ein_buendel_ueber_die_kette_kommt_an() {
        let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);
        k.aufnehmen(einreichung(koordinator, nonce, b));
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1);
    }

    /// ⚑ **Gegenprobe zur Aufnahme: ein erfundener Pod kommt nicht
    /// hinein** (Fund 144).
    ///
    /// Bis zum 2026-09-02 kam er hinein und blieb bis zum
    /// Epochenwechsel im Zustand. Die Kennung wählt der Einreichende
    /// frei, also war das ein Weg, den Zustand wachsen zu lassen.
    #[test]
    fn ein_erfundener_pod_kommt_nicht_in_den_zustand() {
        let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);

        // ⚑ **Die Kennung wird geändert und danach neu unterschrieben.**
        // Die Kennung steht in der Signierbotschaft; wer sie nach dem
        // Unterschreiben ändert, scheitert schon am Aggregat, und dann
        // prüfte dieser Test die Signatur und nicht die Kennungssuche.
        // Der erste Anlauf hat genau daran gelegen: Er bestand auch
        // dann, wenn man die Suche ausbaute.
        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        let mut erfunden = b.clone();
        erfunden.pod = myl_types::ids::PodId::new([3; 32]);
        buendel_unterschreiben(&mut erfunden, &zuteilung.pods[0]);

        k.aufnehmen(einreichung(koordinator, nonce, erfunden));
        k.baue_block();
        assert!(
            k.zustand().buendel.is_empty(),
            "ein Buendel fuer einen Pod, den es nicht gibt, steht im Zustand"
        );

        // Gegenprobe zur Gegenprobe: Dasselbe Bündel mit der **echten**
        // Kennung geht durch. Sonst bewiese der Test nur, dass
        // irgendetwas scheitert.
        k.aufnehmen(einreichung(koordinator, nonce + 1, b));
        k.baue_block();
        assert_eq!(
            k.zustand().buendel.len(),
            1,
            "auch mit der echten Kennung kam nichts an"
        );
    }

    /// ⚑ **Und eine Attrappe als Unterschrift auch nicht** (Fund 144).
    #[test]
    fn eine_attrappe_kommt_nicht_in_den_zustand() {
        let (mut k, mut b, koordinator, nonce) = kette_mit_pod(4_200, 1);
        b.aggregate_sig = myl_types::bls::BlsSignature([0; 96]);
        k.aufnehmen(einreichung(koordinator, nonce, b));
        k.baue_block();
        assert!(
            k.zustand().buendel.is_empty(),
            "ein unsigniertes Buendel steht im Zustand"
        );
    }

    /// ⚑ **Nur der Koordinator reicht ein** (Fund 144).
    #[test]
    fn ein_anderes_mitglied_reicht_nicht_ein() {
        let (mut k, b, koordinator, _) = kette_mit_pod(4_200, 1);
        let anderer = (koordinator + 1) % 6;
        k.aufnehmen(einreichung(anderer, 1, b));
        k.baue_block();
        assert!(
            k.zustand().buendel.is_empty(),
            "ein Mitglied ohne Koordinatorrolle hat eingereicht"
        );
    }

    /// ⚑ **Und am Epochenwechsel fallen sie weg.** Ohne das wüchse der
    /// Zustand unbegrenzt; die Historie steht in den Blöcken.
    #[test]
    fn am_epochenwechsel_fallen_die_buendel_weg() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);
        k.aufnehmen(einreichung(koordinator, nonce, b));
        k.baue_block();
        assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");
        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            k.baue_block();
            if k.zustand().epoch.0 == 1 {
                break;
            }
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

    /// ⚑ **Eine erfundene Saatquelle wird abgelehnt** (Punkt 44).
    ///
    /// Ohne diese Prüfung könnte ein Erzeuger beliebige Bytes eintragen:
    /// Sie gehen in keine Zustandswurzel ein, also fiele es niemandem
    /// auf, und der Mahlraum wäre unbegrenzt statt höchstens sechzehn
    /// Bit (Fund 120).
    #[test]
    fn eine_erfundene_saatquelle_wird_abgelehnt() {
        let mut k = Kette::probestand();
        let mut block = k.baue_block();
        block.header.saatquelle = Some(b"frei erfunden".to_vec());
        let mut leser = Kette::probestand();
        assert_eq!(
            leser.uebernimm(&block),
            Err(KettenFehler::SaatquelleTraegtNicht),
            "erfundene Bytes gingen als Saatquelle durch"
        );
    }

    /// ⚑ **Und ein Zertifikat für einen *anderen* Block ebenso.**
    ///
    /// Das ist die schwerere Hälfte: Ein altes oder fremdes Zertifikat
    /// ist strukturell einwandfrei, und wer es einsetzen darf, **wählt
    /// unter Zertifikaten** und damit seine Saat.
    #[test]
    fn ein_zertifikat_fuer_einen_anderen_block_wird_abgelehnt() {
        use myl_consensus::round_change::Commitzertifikat;
        let mut k = Kette::probestand();
        let fremd = Commitzertifikat {
            round: 1,
            block_hash: Hash::sha256(b"ein ganz anderer Block"),
            committers: vec![myl_types::ids::MinerId::new([1; 32])],
            aggregate: myl_types::bls::BlsAggregateSignature([0; 96]),
        };
        let mut block = k.baue_block();
        block.header.saatquelle = Some(borsh::to_vec(&fremd).expect("borsh"));
        let mut leser = Kette::probestand();
        assert_eq!(
            leser.uebernimm(&block),
            Err(KettenFehler::SaatquelleTraegtNicht)
        );
    }

    /// Ohne Saatquelle bleibt der Blockhash der Rückfall, und der ist
    /// erlaubt: benannt schlechter, nicht verboten.
    #[test]
    fn ohne_saatquelle_wird_ein_block_angenommen() {
        let mut k = Kette::probestand();
        let block = k.baue_block();
        assert!(block.header.saatquelle.is_none());
        let mut leser = Kette::probestand();
        assert!(leser.uebernimm(&block).is_ok());
    }

    /// ⚑ **Die Saatquelle aus dem Block bestimmt die Ziehung.**
    ///
    /// Das ist die Zusage aus Punkt 44: Wer den Block übernimmt, liest
    /// die Quelle **aus ihm** und nicht aus eigenem Zustand. Zwei Knoten
    /// mit verschiedenen Quellen zögen verschiedene Segmente, und wer
    /// geprüft wird, ist eine Konsensentscheidung.
    ///
    /// ⛑ **Was dieser Test nicht zeigt, und es gehört gesagt:** dass die
    /// Quelle **echt** ist. Ein Erzeuger kann heute beliebige Bytes
    /// eintragen; sie gehen in keine Zustandswurzel ein, also fällt es
    /// niemandem auf. **Damit ist der Mahlraum wieder unbegrenzt**, wie
    /// beim Blockhash, und die sechzehn Bit aus Fund 120 sind erst
    /// erreicht, wenn ein Übernehmer die Quelle gegen das Zertifikat des
    /// Vorgängers prüft. **Das ist der offene Rest von Punkt 44.**
    #[test]
    fn die_saatquelle_aus_dem_block_bestimmt_die_ziehung() {
        let bezeugt: Vec<myl_types::PoIBundle> = vec![myl_types::PoIBundle {
            epoch: EpochId(0),
            pod: myl_types::ids::PodId::new([5; 32]),
            segments_root: myl_types::ids::MerkleRoot::new([5; 32]),
            vtfe_claimed: 1,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: 1_000,
        }];
        let mit =
            Kette::stichprobe_der_epoche(&bezeugt, 0, &Kette::startwert(), Some(b"zertifikat"));
        let ohne = Kette::stichprobe_der_epoche(&bezeugt, 0, &Kette::startwert(), None);
        // ⚑ **Aus der Konstante gerechnet und nicht abgeschrieben**
        // (Fund 171). Hier stand zweimal `20`, und als die Rate von 200
        // auf 500 bp stieg, war das die einzige Stelle, die es merkte.
        let erwartet = (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000);
        assert_eq!(mit.len(), erwartet);
        assert_eq!(ohne.len(), erwartet);
        assert_ne!(
            mit, ohne,
            "die Quelle aus dem Block muss die Ziehung aendern, sonst wirkt sie nicht"
        );
        // Zweimal dieselbe Quelle ergibt dieselbe Ziehung: Darauf ruht,
        // dass zwei Knoten dasselbe pruefen.
        assert_eq!(
            mit,
            Kette::stichprobe_der_epoche(&bezeugt, 0, &Kette::startwert(), Some(b"zertifikat"))
        );
    }


    /// ⚑ **Die Zuteilung steht schon während ihrer Epoche fest**
    /// (Fund 143).
    ///
    /// Bis zum 2026-09-02 kam die Saat aus `self.letzter_hash`, und
    /// beim Epochenabschluss ist das der **letzte** Block der Epoche,
    /// die gerade abgerechnet wird. Die Zuteilung stand also erst fest,
    /// wenn die Epoche vorbei war, während ein Bündel **während** ihr
    /// eingereicht sein muss: Kein Pod konnte wissen, dass er einer
    /// ist.
    ///
    /// ⚑ **Warum der alte Fehler unentdeckt blieb:** Der große Test
    /// dieses Punktes hat sechs Miner, also genau einen Pod, und der
    /// enthält alle sechs, gleich welche Saat man nimmt. Die
    /// Mitgliedschaft war saatunabhängig (Fund 142), und damit war auch
    /// die falsche Saat unauffällig. **Dieser Test hat achtzehn Miner
    /// und damit drei Pods**, und über drei Pods entscheidet die Saat.
    #[test]
    fn die_zuteilung_aendert_sich_waehrend_ihrer_epoche_nicht() {
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        // ⚑ **Eigene Schlüssel, nicht die Probekonten.** Von denen gibt
        // es acht (`PROBEKONTEN`), und `probeschluessel` rechnet modulo:
        // Achtzehn Aufrufe ergäben achtzehn Anmeldungen unter acht
        // Kennungen, also acht Miner. Der Test bräuchte dann drei Pods
        // und bekäme einen.
        for w in 0..18u8 {
            let geheim = myl_types::bls::BlsSecretKey::key_gen(&[w.wrapping_add(1); 32])
                .expect("32 Byte sind für key_gen gültig");
            let oeffentlich = geheim.public_key().expect("gültiger Punkt");
            let kennung = myl_types::ids::MinerId::aus_schluessel(&oeffentlich);
            myl_ledger::transitions::miner_anmelden(
                k.zustand_mut(),
                &myl_types::ids::Address::new(*kennung.as_bytes()),
                &kennung,
                HardwareClass::MediumGpu,
                GeoRegion::Europe,
                oeffentlich,
                myl_types::latency_attest::PeerIdBytes([0; 32]),
            )
            .expect("Anmeldung");
        }
        assert_eq!(k.zustand().miner.len(), 18, "es entstanden nicht achtzehn Kennungen");

        let besetzung = |k: &Kette| -> Vec<Vec<[u8; 32]>> {
            Kette::zuteilung_der_laufenden_epoche(k.zustand())
                .pods
                .iter()
                .map(|p| {
                    let mut m: Vec<[u8; 32]> =
                        p.mitglieder().map(|x| *x.miner_id.as_bytes()).collect();
                    m.sort();
                    m
                })
                .collect()
        };

        let am_anfang = besetzung(&k);
        assert_eq!(am_anfang.len(), 3, "achtzehn Miner ergeben drei Pods zu sechs");

        // Zehn Blöcke weiter, mitten in derselben Epoche.
        for _ in 0..10 {
            k.baue_block();
        }
        assert_eq!(k.zustand().epoch.0, 0, "die Epoche wechselte zu früh");
        assert_eq!(
            besetzung(&k),
            am_anfang,
            "die Besetzung hat sich innerhalb der Epoche geändert"
        );

        // ⚑ **Gegenprobe zur Zusicherung selbst:** Der letzte Hash hat
        // sich in diesen zehn Blöcken sehr wohl geändert. Wäre er die
        // Saat, sähe der Test oben etwas anderes, und dass er nichts
        // sieht, wäre nichts wert.
        let mit_letztem_hash: Vec<Vec<[u8; 32]>> =
            myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
                &angemeldete_miner(k.zustand()),
                k.zustand().epoch.0,
                &k.letzter_hash(),
                Kette::PROBE_SHARDS,
            )
            .pods
            .iter()
            .map(|p| {
                let mut m: Vec<[u8; 32]> =
                    p.mitglieder().map(|x| *x.miner_id.as_bytes()).collect();
                m.sort();
                m
            })
            .collect();
        assert_ne!(
            mit_letztem_hash, am_anfang,
            "der letzte Hash ergibt dieselbe Besetzung wie die Epochensaat, \
             dann prüft dieser Test nichts"
        );
    }

    /// ⚑ **Die Saat rückt um genau eine Epoche vor, mit zwei Epochen
    /// Vorlauf** (Fund 143).
    #[test]
    fn die_saat_kommt_aus_der_vorletzten_epoche() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;

        let mut k = Kette::probestand();
        assert_eq!(k.zustand().epochensaat, Kette::startwert());

        // Bis zum Wechsel bauen und dabei den Hash **vor** jedem Block
        // merken. ⚑ Nicht die Höhe abzählen: Der Wechsel geschieht
        // **im** Block, dessen Epoche höher ist, und `letzter_hash` ist
        // dann schon der seine. Der erste Anlauf dieses Tests hat genau
        // daran gelegen, mit einer Zahl statt einer Wirkung.
        let mut ende_epoche_0 = k.letzter_hash();
        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            let vorher = k.letzter_hash();
            k.baue_block();
            if k.zustand().epoch.0 == 1 {
                ende_epoche_0 = vorher;
                break;
            }
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        // In Epoche 1 gilt noch der Startwert: Epoche `e` nimmt die Saat
        // von `e−2`, und `−1` gibt es nicht.
        assert_eq!(
            k.zustand().epochensaat,
            Kette::startwert(),
            "Epoche 1 nimmt schon eine Saat, die es noch nicht geben darf"
        );
        assert_eq!(k.zustand().epochensaat_naechste, ende_epoche_0);

        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            k.baue_block();
            if k.zustand().epoch.0 == 2 {
                break;
            }
        }
        assert_eq!(k.zustand().epoch.0, 2);
        assert_eq!(
            k.zustand().epochensaat,
            ende_epoche_0,
            "Epoche 2 muss die Saat vom Ende der Epoche 0 tragen"
        );
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
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        // ⚑ **Keine Verteilung mehr zu setzen** (Fund 161): Sie wird
        // gerechnet und ist immer da. Das letzte Shard-Stück wiegt von
        // selbst schwerer, weil dort der LM-Kopf sitzt.
        let mut k = Kette::probestand();

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
        //
        // ⚑ **Über die Kette und nicht über den Übergang** (Fund 167,
        // 2026-09-03). Bis dahin rief dieser Test
        // `auszahlungskonto_eintragen` direkt, und das war der einzige
        // Weg, den es gab: Keine `Anweisung` trug ihn. Der Test war
        // grün und belegte einen Ablauf, den auf einem echten Netz
        // niemand gehen konnte. Jetzt geht er denselben Weg wie ein
        // Miner.
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::AuszahlungskontoEintragen {
                        kennung,
                        konto: kaltes_konto(w),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.baue_block();
        for w in 0..6u8 {
            let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
            assert_eq!(
                k.zustand().auszahlung.get(&kennung),
                Some(&kaltes_konto(w)),
                "das Auszahlungskonto kam nicht ueber die Kette an"
            );
        }

        // Die Zuteilung dieser Epoche nachrechnen und für ihren Pod ein
        // Bündel einreichen.
        let register = myl_ledger::transitions::angemeldete_miner(k.zustand());
        let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
            &register,
            k.zustand().epoch.0,
            // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash
            // (Fund 143): Nur sie gilt die ganze Epoche, und nur
            // gegen sie rechnet der Abschluss nach.
            &k.zustand().epochensaat,
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
            // Tausend Segmente, damit die Ziehung ueberhaupt sichtbar ist
            // und die Ziehung ueberhaupt sichtbar ist.
            segmente: 1_000,
        };
        buendel_unterschreiben(&mut b, &zuteilung.pods[0]);
        // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
        // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
        // nur deshalb richtig, weil niemand hinsah.
        let koordinator = koordinator_von(&zuteilung.pods[0]);
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(koordinator),
                nonce[koordinator as usize],
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
            (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000),
            "{} bp von 1000 Segmenten sind {}, gezogen wurden {}",
            Kette::STICHPROBE_BP,
            (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000),
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
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        // ⚑ Keine Verteilung mehr zu setzen (Fund 161): sie ist immer da.
        let mut k = Kette::probestand();

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
            // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash
            // (Fund 143): Nur sie gilt die ganze Epoche, und nur
            // gegen sie rechnet der Abschluss nach.
            &k.zustand().epochensaat,
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
        // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
        // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
        // nur deshalb richtig, weil niemand hinsah.
        let koordinator = koordinator_von(&zuteilung.pods[0]);
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(koordinator),
                nonce[koordinator as usize],
                Anweisung::BuendelEinreichen { buendel: b },
            )
            .expect("signieren"),
        );
        k.baue_block();
        // ⚑ **Seit Fund 144 kommt es gar nicht erst hinein.** Vorher
        // stand es im Zustand und zahlte dort nur nichts aus.
        assert!(
            k.zustand().buendel.is_empty(),
            "ein Buendel ohne gueltige Unterschrift kam in den Zustand"
        );

        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            k.baue_block();
            if k.zustand().epoch.0 == 1 {
                break;
            }
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

    // ⚑ **Der Test `ohne_arbeitsverteilung_bekommt_niemand_etwas` ist
    // am 2026-09-03 entfernt worden** (Fund 161), zusammen mit dem
    // Zustand, den er prüfte: Es gibt kein „ohne Verteilung" mehr,
    // seit sie gerechnet statt gesetzt wird. Was von seiner Prüfung
    // bleibt, deckt jetzt `ein_buendel_ohne_gueltige_unterschrift_zahlt_nichts_aus`
    // ab: Ein Bündel ohne gültiges Aggregat zahlt nichts aus.

    /// ⚑ **Ein Bündel mit erfundener Pod-Kennung zahlt nichts aus.**
    ///
    /// ⛑ Ohne diesen Test war die Kennungssuche in der Kette ungeprüft:
    /// Die Gegenprobe „nimm einfach den ersten Pod" blieb grün, weil in
    /// den übrigen Tests nur **ein** Pod entsteht. Hier scheitert sie.
    ///
    /// ⚑ **Seit Fund 144 ist die Aussage schärfer geworden.** Bis zum
    /// 2026-09-02 kam ein erfundenes Bündel in den Zustand und zahlte
    /// dort nur nichts aus; jetzt kommt es gar nicht erst hinein, weil
    /// die Aufnahme die Besetzung nachschlägt. Der Test prüft beides:
    /// nicht im Zustand **und** kein Konto gewachsen.
    #[test]
    fn ein_buendel_mit_erfundener_kennung_zahlt_nichts_aus() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        // ⚑ Keine Verteilung mehr zu setzen (Fund 161): sie ist immer da.
        let mut k = Kette::probestand();

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
        // Eingereicht wird von einem angemeldeten Miner, damit der
        // Test nicht schon an der Anmeldung scheitert.
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
        assert!(
            k.zustand().buendel.is_empty(),
            "ein Buendel fuer einen Pod, den es nicht gibt, kam in den Zustand"
        );

        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            k.baue_block();
            if k.zustand().epoch.0 == 1 {
                break;
            }
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

    /// ⚑ **Fund 167: das Auszahlungskonto hat einen Weg über die Kette,
    /// und die Berechtigungsregel gilt auf ihm.**
    ///
    /// Drei Lagen in einem Test, weil sie zusammen die Regel ergeben:
    /// Die **erste** Eintragung darf der Miner selbst, jede **weitere**
    /// nur das eingetragene kalte Konto, und ein **Fremder** nie.
    ///
    /// ⛑ Ohne diesen Test wäre die neue `Anweisung` eine Variante, die
    /// der Übersetzer kennt und kein Block anwendet: genau die Klasse,
    /// aus der der Fund kam.
    #[test]
    fn ein_auszahlungskonto_geht_ueber_die_kette_und_nur_der_richtige_darf() {
        let mut k = Kette::probestand();
        let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());

        // ⚑ **Das kalte Konto ist hier ein Probekonto und keine nackte
        // Adresse**, denn der positive Fall „das kalte Konto ändert" ist
        // die Hälfte der Regel, und dafür muss jemand unterschreiben
        // können. Eine `Address::new([...])` kann das nicht.
        let kalt = probekonto(5);

        // 1. Die erste Eintragung, vom Miner selbst.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                0,
                Anweisung::AuszahlungskontoEintragen { kennung, konto: kalt },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kalt),
            "die erste Eintragung kam nicht an"
        );

        // 2. Ein Fremder will umleiten. ⚑ **Genau der Angriff, gegen den
        // die Trennung steht**: ein gestohlener Konsensschlüssel, der
        // den Ertrag woanders hin schickt.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(3),
                0,
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(3),
                },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kalt),
            "ein Fremder hat das Auszahlungskonto umgeleitet"
        );

        // ⚑ **Und der Miner selbst darf es jetzt auch nicht mehr.**
        // Nach der ersten Eintragung gehört die Änderung dem kalten
        // Konto; sonst nützte die Trennung nichts.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(0),
                1,
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(4),
                },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kalt),
            "der heisse Schluessel konnte nach der ersten Eintragung umleiten"
        );

        // 3. Das eingetragene kalte Konto darf. **Ohne diese Hälfte
        // wäre die Regel eine Sperre und keine Trennung:** Wer sein
        // kaltes Konto verliert, käme nie wieder an seine Erträge.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(5),
                0,
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(9),
                },
            )
            .expect("signieren"),
        );
        k.baue_block();
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kaltes_konto(9)),
            "das eingetragene Konto durfte seine eigene Eintragung nicht aendern"
        );
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
