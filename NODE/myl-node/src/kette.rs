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
use myl_consensus::block::{epoche_fuer_hoehe, Anweisung, Block, BlockHeader, Transaktion, BLOECKE_JE_EPOCHE};
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
///
/// # ⚑ Zwölf seit dem 2026-09-05, und die Zahl ist begründet
///
/// Vorher acht. Ein Pod hat `PROBE_SHARDS + 2` Mitglieder, also sechs;
/// acht Konten ergeben damit **einen** Pod, und ein einzelner Pod kann
/// nicht trainieren: Trainingssegmente werden paarweise vergeben, weil
/// ein Ergebnis, das nur einer gerechnet hat, sich mit nichts
/// vergleichen lässt.
///
/// **Zwölf ist das Minimum eines Netzes, das trainieren kann:** zwei
/// volle Pods, ein Redundanzpaar. Mehr wäre Ballast, weniger machte den
/// Trainingsweg in der Probekette unerreichbar, und ein Weg, den kein
/// Test geht, ist keiner.
pub const PROBEKONTEN: u8 = 12;

/// Guthaben je Probekonto in MYL-Kleinstbeträgen. **Spielgeld.**
pub const PROBEGUTHABEN: u64 = 10_000_000;

/// Wie viele Blöcke der Knoten im Arbeitsspeicher hält.
///
/// ⚑ **Hergeleitet, nicht gegriffen.** Der Zwischenspeicher hat genau
/// eine Aufgabe: eine Nachforderung beantworten, ohne die Platte
/// anzufassen. Eine Nachforderung umfasste bis zum 2026-10-22 höchstens
/// 64 Blöcke, also deckte ein Fenster von vier Lieferungen den Fall ab,
/// dass mehrere Nachzügler gleichzeitig und an verschiedenen Stellen
/// aufholen.
///
/// ⚑ **Seit die Lieferung 512 Blöcke fasst, steht die Zahl fest hier**:
/// Ein Fenster von vier Lieferungen wären 2048 Blöcke im Speicher, und
/// was älter ist als das Fenster, liest der Knoten ohnehin von der Platte.
///
/// **Vier Lieferungen sind 256, und dieselbe Zahl steht in gereiften
/// Kettenknoten** als Deckel ihrer Blockzwischenspeicher. Zwei
/// verschiedene Herleitungen, dieselbe Größenordnung: Das ist ein
/// Hinweis, kein Beweis, aber ein beruhigender.
///
/// **Die Obergrenze im Speicher** ist damit benannt und nicht mehr
/// offen: 256 mal [`crate::speicher::MAX_SATZ_BYTES`] im schlimmsten
/// Fall, im Betrieb um Größenordnungen weniger.
pub const VERLAUFSFENSTER: usize = 256;

/// ⚑ **Trägt das Vertrauen noch?** (2026-10-27, CONSENSUS 7.5): ja, solange
/// die Epoche des Netzes höchstens
/// [`myl_ledger::einsatz::VERTRAUENSFRIST_EPOCHEN`] über der des zuletzt
/// bestätigten eigenen Prüfpunkts liegt. Darüber haftet womöglich niemand
/// mehr, der damals unterschrieben hat.
pub fn vertrauen_traegt(bestaetigt_epoche: u64, netz_epoche: u64) -> bool {
    netz_epoche.saturating_sub(bestaetigt_epoche) <= myl_ledger::einsatz::VERTRAUENSFRIST_EPOCHEN
}

/// Höchstens so viele Zuteilungen abgelaufener Epochen hebt die Kette für
/// ihre Ziehung auf (2026-10-27); länger hängt keine Abstimmung, ohne dass
/// die Ziehung ohnehin verloren wäre.
const OFFENE_ZUTEILUNGEN_HOECHSTENS: usize = 4;

/// **Wie lange eine Transaktion im Mempool wartet**, in eigenen Höhen
/// (Fund 627): lange genug, dass eine überholte Vorgängerin nachkommt,
/// kurz genug, dass Unsinn mit erfundener Nummer nicht liegen bleibt.
pub const WARTEN_HOECHSTENS_HOEHEN: u64 = 64;

/// **Höchstens so viele Transaktionen warten** (Fund 627); was darüber
/// kommt, wird nicht aufgenommen.
pub const MEMPOOL_HOECHSTENS: usize = 4096;

/// ⚑ **So viele Bytes an Transaktionen trägt ein Block höchstens** (Fund 633):
/// die Hälfte dessen, was Gossip an einem Block annimmt
/// (`myl_net::validation::MAX_BLOCKS_BYTES`), damit Kopf, Rahmen und
/// Saatquelle Platz haben. Bis zum 2026-10-24 ging jede gültige Transaktion
/// in den nächsten Block; rund achtzig Archivbelege zu je 27 KB von einem
/// beliebigen Konto ergaben einen Block, den kein Folger je bekam, und die
/// Kette stand für alle. Was nicht passt, wartet auf den nächsten Block.
pub const BLOCK_TXS_HOECHSTENS_BYTES: usize = myl_net::validation::MAX_BLOCKS_BYTES / 2;

/// So viele Transaktionen eines Absenders hält der Mempool höchstens
/// (Fund 633): Sonst füllt ein einziges Konto alle [`MEMPOOL_HOECHSTENS`]
/// Plätze, und jede weitere Transaktion jedes anderen wird abgewiesen.
pub const MEMPOOL_JE_ABSENDER: usize = 64;

/// ⚑ **Neulinge** (Fund 635): Absender ohne Guthaben, ohne Einsatz und ohne
/// Anmeldung als Miner. Bis zu den Gebühren (2026-10-27, G1) kostete eine
/// Transaktion nichts, und jede legte im Zustand ein Konto an; mit frischen
/// Schlüsseln liess sich so der Mempool füllen und der Zustand ohne Grenze
/// aufblähen. Seither kommt ein Neuling in keinen Block, weil er die Gebühr
/// nicht zahlen kann; die Drossel bleibt für den Mempool, in dem er bis zum
/// Verfall wartet: höchstens so viele zugleich in den Mempool ...
pub const MEMPOOL_NEULINGE_HOECHSTENS: usize = 32;
/// ... und höchstens so viele in einen Block. Ein neuer Knoten braucht eine
/// einzige Transaktion, die Anmeldung; danach ist er kein Neuling mehr.
pub const BLOCK_NEULINGE_HOECHSTENS: usize = 16;

/// ⚑ **Das Ziel des Grundpreises** (2026-10-27, Festlegung des
/// Projektinhabers, G1): ein halber Block. Ist ein Block voller, steigt der
/// Preis, ist er leerer, sinkt er, je Block um höchstens ein Achtel.
pub const GRUNDPREIS_ZIEL_BYTES: u64 = BLOCK_TXS_HOECHSTENS_BYTES as u64 / 2;

/// Um höchstens den so vielten Teil wandert der Grundpreis je Block.
pub const GRUNDPREIS_SCHRITT_TEILER: u64 = 8;

/// ⚑ **Die Teilung der Gebühren** (Festlegung des Projektinhabers): von
/// dreizehn Teilen drei an die Treasury, zehn an die Validatoren.
pub const GEBUEHR_TEILE: u64 = 13;
/// Der Anteil der Treasury in [`GEBUEHR_TEILE`].
pub const GEBUEHR_TREASURY: u64 = 3;

/// Die Arten einer Hinterlegung (`LedgerState::hinterlegungen`).
pub const HINTERLEGT_ANMELDUNG: u8 = 0;
/// Siehe [`HINTERLEGT_ANMELDUNG`].
pub const HINTERLEGT_SITZUNG: u8 = 1;

/// Der Grundpreis des nächsten Blocks aus dem jetzigen und den Bytes, die
/// dieser Block an Gebühren gezählt hat.
///
/// Wie EIP-1559: `preis · (bytes − ziel) / ziel / 8`. ⚑ **Mindestens eins
/// nach oben**, wenn der Block über dem Ziel lag; sonst bliebe ein Preis
/// von eins für immer bei eins, weil ein Achtel davon null ist. Nie unter
/// dem Boden.
pub fn naechster_grundpreis(preis: u64, bytes: u64, boden: u64) -> u64 {
    let ziel = GRUNDPREIS_ZIEL_BYTES as u128;
    let p = preis as u128;
    let neu = if bytes as u128 > ziel {
        let schritt = p * (bytes as u128 - ziel) / ziel / GRUNDPREIS_SCHRITT_TEILER as u128;
        p + schritt.max(1)
    } else {
        p - p * (ziel - bytes as u128) / ziel / GRUNDPREIS_SCHRITT_TEILER as u128
    };
    u64::try_from(neu).unwrap_or(u64::MAX).max(boden)
}

/// Ob `a` ein Neuling ist (siehe [`MEMPOOL_NEULINGE_HOECHSTENS`]).
fn ist_neuling(z: &LedgerState, a: &Address) -> bool {
    let k = z.account(a);
    k.balance == 0 && k.staked == 0 && !z.miner.contains_key(&myl_types::ids::MinerId::new(*a.as_bytes()))
}

/// So viele codierte Gruppen hält die Kette für Splitter vor (Fund 632).
/// Mit einer einzigen erzwang jede Anfrage nach einer anderen Gruppe eine
/// neue Codierung von 1024 Blöcken, auch unter ehrlichen Splittern, die
/// verschiedene Gruppen zugleich holen.
pub const TEILGRUPPEN_BEHALTEN: usize = 8;

/// Der Startwert, an dem die Probekette hängt.
///
/// **Der Riegel gegen eine Verwechslung mit dem echten Netz.** Ein Block
/// nennt seinen Vorgänger; der erste Probeblock nennt diesen Wert. Ein
/// echtes Netz beginnt bei einem anderen, und damit passt ein Probeblock
/// dort nirgends hinein. Der Text sagt es auch dem, der die Bytes
/// anschaut.
///
/// ⚑ **Mit der Historienwurzel neu** (2026-10-23): Der Zustand trägt seitdem
/// die Historie, und eine Kettendatei von davor rechnete an Block 1 eine
/// andere Wurzel. Mit einem neuen Startwert weist der Knoten sie gleich beim
/// Öffnen als fremde Kette ab, statt an einer Wurzel zu scheitern.
pub const PROBE_STARTWERT: &[u8] =
    b"MYELITH-PROBELAUF-KEIN-TESTNETZ-KEINE-ECHTE-KETTE-H1";

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

/// Die Anmeldung eines Probekontos als Miner in Europa, mit Nummer 0.
pub fn probeanmeldung(nummer: u8) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(nummer),
        0,
        Anweisung::MinerAnmelden {
            hardware: myl_types::miner::HardwareClass::MediumGpu,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        },
    )
    .expect("ein gültiger Schlüssel unterschreibt")
}

/// **Die Zusage eines Probekontos für das Probemodell**, als unterschriebene
/// Transaktion (Stufe 4, 2026-10-22): alle Ebenen samt Kopf, 300 µs je
/// Ebene, gültig ab `ab`.
///
/// ⚑ **Seit „kein Pod ohne Zusage“ braucht jede Probe mit Pods das**, wie
/// die Anmeldung: Ein Miner ohne Zusage kommt in keinen Pod.
pub fn probezusage(nummer: u8, ab: EpochId, nonce: u64) -> Transaktion {
    let sk = probeschluessel(nummer);
    let pk = sk.public_key().expect("aus einem gültigen Schlüssel folgt ein gültiger Punkt");
    let pm = crate::netz::PROBEKETTE.podmodell;
    let mut zusage = myl_types::zusage::Kapazitaetszusage::neu(myl_types::ids::MinerId::aus_schluessel(&pk), ab, 0)
        .mit_modell(myl_types::zusage::Modellzusage {
            abdruck: pm.abdruck,
            ebenen_hoechstens: pm.ebenen_samt_kopf(),
            ebenenzeit_us: 300,
            gehalten: None,
        });
    zusage.signiere(&sk).expect("ein gültiger Schlüssel unterschreibt");
    Transaktion::signiere(&Kette::startwert(), &sk, nonce, Anweisung::ZusageEinreichen { zusage })
        .expect("ein gültiger Schlüssel unterschreibt")
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
    /// **Das Zertifikat eines Prüfpunkts im Block trägt nicht**
    /// (2026-10-27, CONSENSUS 7.2): kein Prüfpunkt nach dem zuletzt
    /// bestätigten, nicht der eigene Block an dieser Höhe, oder nicht
    /// zwei Drittel des Stimmsatzes.
    ///
    /// ⚑ **Eine Ablehnung und keine Warnung.** Aus dem Zertifikat kommt die
    /// Saat der Stichprobe und die Endgültigkeit; eines, das niemand belegt,
    /// wäre ein frei gewählter Wert.
    PruefpunktTraegtNicht,
    /// **Der Erzeuger ist nicht belegt**: Die Unterschrift
    /// trägt nicht, oder ein Stimmsatz ist gesetzt und der Block ist
    /// unsigniert oder von jemandem außerhalb des Satzes.
    ErzeugerUnbelegt,
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
    /// **Der Block trägt einen Schuldspruch, und niemand kann ihn
    /// prüfen** (Fund 192).
    ///
    /// ⚑ **Eine Ablehnung, obwohl die Kette Schuldsprüche heute gar
    /// nicht anwendet, und gerade deshalb.** `Block::verdicts` ist Teil
    /// des gehashten Blocks, `Block::add_verdict` steht offen, und der
    /// Ledger hat mit `apply_verdict` alles, was zum Schlachten nötig
    /// ist. Was fehlt, ist der **Beleg**: Ein `Verdict` nennt Täter und
    /// Kopfgeldempfänger und trägt keinen Nachweis. Wer die beiden
    /// Enden verbände, ohne den Beleg zu verlangen, gäbe dem
    /// Blockerzeuger ein Werkzeug, mit dem er jeden schlachten kann.
    ///
    /// **Die Ablehnung ist die sichere Zwischenlage:** Solange der
    /// Beleg nicht auf dem Draht ist, kommt kein Schuldspruch in einen
    /// Block. Damit ist das Feld nicht länger stiller Ballast, den ein
    /// späterer Bau versehentlich scharf schaltet, sondern eine Tür,
    /// die aufgeschlossen werden **muss**.
    SchuldspruchOhneBeleg { anzahl: usize },
    /// **Die Anfechtungen des Blocks sind unsinnig** (Fund 192).
    ///
    /// `Block::validate_challenges` gab es seit jeher und hatte bis zum
    /// 2026-09-06 **keinen Aufrufer**: eine geprüfte, abgehakte Prüfung,
    /// die nie lief.
    AnfechtungUnsinnig,
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
            Self::PruefpunktTraegtNicht => write!(
                f,
                "das Zertifikat des Prüfpunkts trägt nicht"
            ),
            Self::ErzeugerUnbelegt => write!(f, "der Erzeuger ist nicht belegt: keine tragende Unterschrift eines Berechtigten"),
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
            Self::SchuldspruchOhneBeleg { anzahl } => write!(
                f,
                "{anzahl} Schuldspruch/Schuldsprueche im Block, aber kein Beleg auf dem Draht: \
                 abgelehnt, bis der Nachweisweg steht"
            ),
            Self::AnfechtungUnsinnig => {
                f.write_str("eine Anfechtung des Blocks besteht die Strukturpruefung nicht")
            }
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
    /// Der Schlüssel, mit dem dieser Knoten seine Blöcke unterschreibt
    /// (2026-10-27); `None` heißt unsigniert.
    erzeugerschluessel: Option<crate::schluessel::Konsensschluessel>,
    /// Ob diese Kette ihren Stimmsatz aus Block 1 lernt (nur auf einer
    /// Probekette, [`crate::netz::Netz::lernt_erzeuger`]).
    lernt_erzeuger: bool,
    /// Das Zertifikat eines Prüfpunkts, das der nächste erzeugte Block
    /// tragen soll (2026-10-27, CONSENSUS 7.2).
    ///
    /// ⚑ **Es kommt von außen, weil es im Stimmsatz entsteht**: Der Knoten
    /// sammelt die Prüfpunktstimmen und bietet es an
    /// ([`Self::pruefpunkt_anbieten`]); die Kette prüft es beim Bauen wie
    /// beim Übernehmen.
    naechstes_pruefpunktzertifikat: Option<myl_consensus::pruefpunkt::Pruefpunktzertifikat>,
    /// Die Zuteilungen abgelaufener Epochen, deren Ziehung noch aussteht:
    /// Gezogen wird erst mit dem Zertifikat (2026-10-27), und dann gilt
    /// schon die Zuteilung der nächsten Epoche (vgl. Fund 658).
    offene_zuteilungen: Vec<(u64, myl_scheduler::shard_assignment::Zuteilung)>,
    /// Die zuletzt gezogene Stichprobe, mit der Epoche, zu der sie
    /// gehört (Punkt 45).
    ///
    /// 📌 **Hier stand zuerst ein blanker `Vec`, und er wurde von jedem
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
    /// Die Pods der zuletzt gezogenen Stichprobe, aus der Zuteilung der
    /// Epoche, zu der sie gehört.
    ///
    /// 📌 **Fund 658 (2026-10-27, im kurzen Podlauf): Keine Stichprobe fand
    /// ihren Pod.** Gezogen wird beim Wechsel, für die **abgelaufene**
    /// Epoche; [`Self::pod_der_kennung`] kannte nur die Zuteilung der
    /// **laufenden** und gab für jede andere `None`. Jede Ziehung endete als
    /// `ohne_adresse`, noch vor Fund 656. Jetzt hält die Kette die Pods der
    /// Ziehung fest, bevor der Wechsel die Zuteilung ersetzt.
    pods_der_stichprobe: Vec<myl_scheduler::shard_assignment::Pod>,
    /// Wartende Transaktionen, je mit der eigenen Höhe bei der Aufnahme.
    mempool: Vec<(u64, Transaktion)>,
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
    /// ⚑ **Wo an jeder Epochengrenze ein Abzug abgelegt wird**
    /// (2026-10-22), neben der Kettendatei. Ohne Kettendatei keiner.
    ablage: Option<crate::abzug::Ablage>,
    /// Die Höhe, ab der diese Kette lebt: 0 von Genesis, sonst die Höhe
    /// des Abzugs, aus dem sie begann. Blöcke davor hat sie nicht.
    basis: u64,
    /// Der zuletzt angelegte Abzug (Höhe, Blockhash), für das Protokoll.
    letzter_abzug: Option<(u64, Hash)>,
    /// ⚑ **Das Platzbudget der Kettendatei in Bytes** (Festlegung des
    /// Projektinhabers vom 2026-10-23); null heißt ohne Grenze. Siehe
    /// [`Kette::verdichten_wenn_noetig`].
    budget: u64,
    /// Die letzte Verdichtung (neue Basis, Bytes vorher, nachher), bis
    /// der Knoten sie ins Protokoll geschrieben hat.
    verdichtet: Option<(u64, u64, u64)>,
    /// Die eigene Kennung als Miner, für die Auslosung der Archivknoten.
    archivkennung: Option<myl_types::ids::MinerId>,
    /// ⚑ **Der Historienbaum** (Archivknoten, Stufe A4): Solange die Kette
    /// ab Genesis lebt, führt sie neben der Historie im Zustand die Knoten
    /// ab Stufe zehn mit (rund 64 Byte je 1024 Blöcke) und kann damit jedem,
    /// der nachlädt, Beweise gegen seine Basis geben. Ab einer Basis fehlt
    /// ihr der Anfang, dann `None`.
    baum: Option<myl_types::historie::Historienbaum>,
    /// Die Gruppenwurzeln des laufenden Abschnitts (Grenze, Wurzeln), einmal
    /// gerechnet, für die Teilwurzel und die Pfade der Teile.
    teilcache: Option<(u64, Vec<Hash>)>,
    /// Die zuletzt codierten Gruppen, höchstens [`TEILGRUPPEN_BEHALTEN`]:
    /// Splitter fragen dieselben, aber nicht alle zugleich dieselbe
    /// (Fund 632).
    teilgruppe: std::collections::VecDeque<(u64, Vec<Vec<u8>>)>,
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

/// ⚑ **Der Name des Probeerzeugers** (2026-10-27): Eine Probekette
/// unterschreibt ihre Blöcke mit dem Konsensschlüssel aus diesem Namen.
/// **Wer den Namen kennt, kennt den Schlüssel**, und das ist auf der
/// Probekette gewollt; sie hat keinen Wert und keinen Anker.
pub const PROBEERZEUGER: &str = "probeerzeuger";

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
        let mut k = Self::im_netz(&crate::netz::PROBEKETTE);
        // ⚑ Seit dem 2026-10-27 braucht jeder Block die Unterschrift eines
        //   Mitglieds; eine Probekette unterschreibt mit dem Probeerzeuger,
        //   dessen Schlüssel jeder aus dem Namen ableiten kann.
        if let Ok(e) = crate::schluessel::Konsensschluessel::probe(PROBEERZEUGER) {
            k.erzeugerschluessel = Some(e);
        }
        k
    }

    /// Der Stimmsatz dieser Kette, sobald sie einen hat.
    pub fn stimmsatz(&self) -> Option<&myl_consensus::validator::VotingSet> {
        self.stimmsatz.as_ref()
    }

    /// **Eine frische Kette im gegebenen Netz** (2026-10-22): wie
    /// [`Kette::probestand`], mit Startwert und Podmodell dieses Netzes.
    ///
    /// ⚑ **Das Podmodell steht ab dem ersten Block im Zustand** und geht
    /// in jede Zustandswurzel ein; zwei Knoten verschiedener Netze trennen
    /// sich damit sofort, auch wenn eine Transaktion durchkäme.
    pub fn im_netz(netz: &'static crate::netz::Netz) -> Self {
        let mut zustand = LedgerState::genesis(PROBE_CREDIT_PREIS);
        zustand.podmodell = Some(netz.podmodell);
        // Die Testkonten ausstatten. Ohne Guthaben scheitert jede
        // Burn-Transaktion, der Zustand bleibt unverändert, und die
        // Übereinstimmung der Wurzeln belegt nichts (siehe
        // [`probekonto`]).
        for n in 0..PROBEKONTEN {
            zustand.account_mut(&probekonto(n)).balance = PROBEGUTHABEN;
        }
        // ⚑ **Ein Probekorpus, damit die Trainingspods etwas zu tun
        // haben** (2026-09-05). Ohne Anker bildet die Kette
        // Trainingspods und weist ihnen nichts zu; der Weg von der
        // Zuteilung bis zum Bündel wäre dann von keinem Test berührt.
        //
        // **Die Wurzel ist ein Platzhalter und keine echte Verankerung.**
        // Diese Kette ist eine Probekette; ein echtes Netz setzt hier
        // die Wurzel eines wirklich verankerten Korpus, und wie er sich
        // ändern lässt, steht in `myl_types::korpusanker`.
        zustand.korpus = Some(myl_types::korpusanker::Korpusanker {
            kennung: "probekorpus".into(),
            wurzel: Hash::sha256(b"myelith-probekorpus"),
            segmente: 65_536,
            buendel: 256,
            tokens_je_segment: 2_048,
        });
        // ⚑ **Die Saat der ersten beiden Epochen ist der Startwert**
        // (Fund 143). Der Ledger kennt ihn nicht, die Kette schon; ohne
        // ihn stünde dort eine Null, die in jedem Netz dieselbe wäre.
        zustand.epochensaat = netz.startwert_hash();
        zustand.epochensaat_naechste = netz.startwert_hash();
        Self {
            hoehe: 0,
            // Nicht das Literal wiederholen: Zwei Stellen mit derselben
            // Zeichenkette laufen irgendwann auseinander, und dann
            // lehnte die Kettendatei ihre eigene Kette ab.
            letzter_hash: netz.startwert_hash(),
            zustand,
            stimmsatz: None,
            erzeugerschluessel: None,
            lernt_erzeuger: netz.lernt_erzeuger,
            naechstes_pruefpunktzertifikat: None,
            offene_zuteilungen: Vec::new(),
            letzte_stichprobe: None,
            pods_der_stichprobe: Vec::new(),
            mempool: Vec::new(),
            bekannt: std::collections::HashSet::new(),
            verlauf: std::collections::BTreeMap::new(),
            speicher: None,
            schreibfehler: 0,
            lesefehler: 0,
            ablage: None,
            basis: 0,
            letzter_abzug: None,
            budget: 0,
            verdichtet: None,
            archivkennung: None,
            baum: Some(Default::default()),
            teilcache: None,
            teilgruppe: std::collections::VecDeque::new(),
        }
    }

    /// ⚑ **Eine Zusage ab Epoche 0, unmittelbar in den Zustand** (Stufe 4,
    /// 2026-10-22): für Proben, die ihre Miner ebenso unmittelbar anmelden
    /// und in Epoche 0 Pods brauchen. Alle Ebenen samt Kopf, 300 µs je
    /// Ebene. Über die Kette gilt eine Zusage erst ab der nächsten Epoche
    /// ([`probezusage`]); **kein Weg für einen Betrieb**, denn er umgeht
    /// Unterschrift und Prüfung.
    pub fn probezusage_setzen(&mut self, kennung: myl_types::ids::MinerId) {
        let pm = self.netz().podmodell;
        let z = myl_types::zusage::Kapazitaetszusage::neu(kennung, EpochId(0), 0).mit_modell(
            myl_types::zusage::Modellzusage {
                abdruck: pm.abdruck,
                ebenen_hoechstens: pm.ebenen_samt_kopf(),
                ebenenzeit_us: 300,
                gehalten: None,
            },
        );
        self.zustand.zusagen.entry(kennung).or_default().insert(0, z);
    }

    /// Das Netz dieser Kette, aus dem Podmodell in ihrem Zustand.
    pub fn netz(&self) -> &'static crate::netz::Netz {
        crate::netz::Netz::des_zustands(&self.zustand)
    }

    /// Der Startwert **dieser** Kette, aus ihrem Netz. Bindet Kettendatei
    /// und Transaktionen an das Netz.
    pub fn netzwert(&self) -> Hash {
        self.netz().startwert_hash()
    }

    /// Der Startwert der Probekette.
    ///
    /// Bindet eine Kettendatei an ihr Netz: Eine Datei mit anderem
    /// Startwert wird abgewiesen, statt eine fremde Historie als eigene
    /// auszugeben. ⚠️ Für eine Kette eines anderen Netzes gilt
    /// [`Kette::netzwert`].
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

    /// Das Platzbudget der Kettendatei, in Bytes; null heißt ohne Grenze.
    pub fn budget_setzen(&mut self, bytes: u64) {
        self.budget = bytes;
    }

    /// Die eigene Kennung als Miner, für die Auslosung der Archivknoten.
    pub fn archivkennung_setzen(&mut self, kennung: Option<myl_types::ids::MinerId>) {
        self.archivkennung = kennung;
    }

    /// (behält jetzt alles, ist Kopie der laufenden Periode), aus dem
    /// Archivstand des Zustands.
    pub fn archiv(&self) -> (bool, bool) {
        let ich = self.archivkennung.as_ref();
        (self.zustand.archiv.behaelt(ich), ich.is_some_and(|m| self.zustand.archiv.ist_kopie(m)))
    }

    /// **Wer die Geschichte haben sollte**: die Netzadressen der Halter (sie
    /// behalten, bis die neuen Kopien belegt haben), dann der Kopien, die
    /// schon vollständig belegt haben, dann der übrigen. Ohne die eigene.
    pub fn archivkopien(&self) -> Vec<myl_types::latency_attest::PeerIdBytes> {
        let a = &self.zustand.archiv;
        let vollstaendig = a.kopien.iter().filter(|k| a.vollstaendig.contains(k));
        let uebrige = a.kopien.iter().filter(|k| !a.vollstaendig.contains(k));
        let mut aus = Vec::new();
        for k in a.halter.iter().chain(vollstaendig).chain(uebrige) {
            if Some(*k) == self.archivkennung {
                continue;
            }
            if let Some(m) = self.zustand.miner.get(k) {
                if !aus.contains(&m.netzadresse) {
                    aus.push(m.netzadresse);
                }
            }
        }
        aus
    }

    /// Die eigene Archivrolle.
    pub fn archivrolle(&self) -> myl_types::archivwahl::Archivrolle {
        crate::archiv::rolle(&self.zustand.archiv, self.archivkennung.as_ref())
    }

    /// Eine gezogene Kopie, der die Geschichte vor ihrer Basis fehlt: Sie
    /// muss sie holen. Ohne Ziehung muss keiner nachladen, denn dann hat
    /// auch keiner verdichtet.
    pub fn braucht_geschichte(&self) -> bool {
        self.archivkennung.as_ref().is_some_and(|m| self.zustand.archiv.ist_kopie(m)) && self.basis > 0
    }

    /// **Der eigene Archivbeleg dieser Epoche** (Stufe A5): die Blöcke der
    /// Stichproben als Bytes und je ein Beweis gegen die Historienwurzel der
    /// Grenze. `None`, wenn `wer` nicht Kopie ist, schon belegt hat oder
    /// dieser Kette die Geschichte fehlt.
    pub fn archivbeleg(&mut self, wer: &myl_types::ids::MinerId) -> Option<Anweisung> {
        let a = &self.zustand.archiv;
        if !a.ist_kopie(wer) || a.belegt.contains(wer) {
            return None;
        }
        let grenze = a.grenze;
        let hoehen = myl_ledger::archiv::stichproben(
            a,
            &self.zustand.epochensaat,
            self.zustand.epoch.0,
            wer,
            a.stichprobenzahl(wer),
        );
        let baum = self.baum.take()?;
        let mut bloecke = Vec::with_capacity(hoehen.len());
        let mut beweise = Vec::with_capacity(hoehen.len());
        let mut ok = true;
        for &h in &hoehen {
            let block = self.bloecke_von_bis(h, h).pop().filter(|b| b.header.height == h);
            let beweis = baum.beweis_bei(h, grenze, |g| self.gruppenblaetter(g));
            // Die Bytes, die der Blockhash deckt: Die Kette prüft sie gegen
            // die Historie der Hashes (nicht die volle Kodierung mit der
            // Unterschrift des Erzeugers).
            match (block.map(|b| b.hashbytes()), beweis) {
                (Some(roh), Some(bw)) => {
                    bloecke.push(roh);
                    beweise.push(bw);
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        self.baum = Some(baum);
        ok.then_some(Anweisung::ArchivBelegen { bloecke, beweise })
    }

    /// Die Blöcke der Gruppe `g` als Bytes; `None`, wenn diese Kette sie
    /// nicht ganz hat.
    pub fn gruppenbloecke(&mut self, g: u64) -> Option<Vec<Vec<u8>>> {
        let hoehen = myl_types::archivteile::gruppenhoehen(g);
        let (ab, bis) = (*hoehen.start(), *hoehen.end());
        if bis > self.hoehe {
            return None;
        }
        let mut aus = Vec::with_capacity((bis - ab + 1) as usize);
        let mut h = ab;
        while h <= bis {
            let teil = self.bloecke_von_bis(h, bis);
            if teil.first()?.header.height != h {
                return None;
            }
            for b in &teil {
                aus.push(borsh::to_vec(b).ok()?);
            }
            h += teil.len() as u64;
        }
        Some(aus)
    }

    /// Die 64 Teile der Gruppe `g`, aus dem Zwischenspeicher oder frisch.
    fn gruppenteile(&mut self, g: u64) -> Option<Vec<Vec<u8>>> {
        if let Some((_, t)) = self.teilgruppe.iter().find(|(x, _)| *x == g) {
            return Some(t.clone());
        }
        let teile = myl_types::archivteile::kodieren(&self.gruppenbloecke(g)?);
        if self.teilgruppe.len() >= TEILGRUPPEN_BEHALTEN {
            self.teilgruppe.pop_front();
        }
        self.teilgruppe.push_back((g, teile.clone()));
        Some(teile)
    }

    /// **Die Gruppenwurzeln des laufenden Abschnitts**, einmal je Grenze
    /// gerechnet; `None`, wenn dieser Kette Blöcke fehlen.
    fn gruppenwurzeln(&mut self) -> Option<Vec<Hash>> {
        let a = &self.zustand.archiv;
        let (grenze, ab, bis) = (a.grenze, a.teil_ab, a.teil_bis);
        if let Some((g, w)) = &self.teilcache {
            if *g == grenze {
                return Some(w.clone());
            }
        }
        let mut wurzeln = Vec::with_capacity((bis - ab) as usize);
        for g in ab..bis {
            let teile = myl_types::archivteile::kodieren(&self.gruppenbloecke(g)?);
            wurzeln.push(myl_types::archivteile::gruppenwurzel(&teile)?);
        }
        self.teilcache = Some((grenze, wurzeln.clone()));
        Some(wurzeln)
    }

    /// **Die Teilwurzel, die eine Kopie nennt** (Stufe A7): `None`, wenn
    /// `wer` nicht Kopie ist, schon genannt hat, die Wurzel schon gilt oder
    /// Blöcke fehlen.
    pub fn teilwurzel_anweisung(&mut self, wer: &myl_types::ids::MinerId) -> Option<Anweisung> {
        let a = &self.zustand.archiv;
        if !a.ist_kopie(wer) || a.teilwurzel.is_some() || a.teil_ab == a.teil_bis || a.teilstimmen.contains_key(wer) {
            return None;
        }
        let w = self.gruppenwurzeln()?;
        Some(Anweisung::ArchivTeilwurzel { wurzel: myl_types::archivteile::teilwurzel(&w)? })
    }

    /// **Teil `teil` der Gruppe an Stelle `stelle`** des laufenden
    /// Abschnitts, mit Beweis: was ein Splitter holt. Nur gegen die
    /// geltende Teilwurzel, damit niemand etwas liefert, das sich nicht
    /// belegen lässt.
    pub fn teilstueck(
        &mut self,
        grenze: u64,
        stelle: u64,
        teil: u64,
    ) -> Option<(Vec<u8>, myl_types::archivteile::Teilbeweis)> {
        let a = &self.zustand.archiv;
        let geltend = a.teilwurzel?;
        if a.grenze != grenze || stelle >= a.teil_bis - a.teil_ab || teil >= myl_types::archivteile::TEILE as u64 {
            return None;
        }
        let g = a.teil_ab + stelle;
        let wurzeln = self.gruppenwurzeln()?;
        if myl_types::archivteile::teilwurzel(&wurzeln)? != geltend {
            return None;
        }
        let teile = self.gruppenteile(g)?;
        let bw = myl_types::archivteile::Teilbeweis::bauen(&teile, teil as usize, &wurzeln, stelle as usize)?;
        Some((teile[teil as usize].clone(), bw))
    }

    /// **Setzt die geprüfte Geschichte vor die Kette** und entfernt die
    /// Basis; die Kette lebt danach wieder ab Genesis. Gibt die Zahl der
    /// eingefügten Blöcke zurück.
    pub fn geschichte_einfuegen(&mut self, archiv: &std::path::Path) -> Result<u64, String> {
        let speicher = self.speicher.as_mut().ok_or("ohne Kettendatei")?;
        let n = speicher.vorne_einfuegen(archiv).map_err(|e| e.to_string())?;
        if let Some(a) = self.ablage.as_ref() {
            a.basis_entfernen().map_err(|e| e.to_string())?;
        }
        self.basis = 0;
        self.baum_aufbauen()?;
        Ok(n)
    }

    /// **Baut den Historienbaum aus der Kettendatei neu**, nach dem
    /// Einfügen der Geschichte. Er muss die Historie des Zustands treffen;
    /// sonst stimmt die Datei nicht mit dem Zustand überein, und der Baum
    /// bleibt weg, statt falsche Beweise zu geben.
    fn baum_aufbauen(&mut self) -> Result<(), String> {
        let hoehe = self.hoehe;
        let mut baum = myl_types::historie::Historienbaum::default();
        baum.anhaengen(&self.netz().startwert_hash());
        let speicher = self.speicher.as_mut().ok_or("ohne Kettendatei")?;
        speicher
            .fuer_jeden_satz(|b| {
                if b.header.height < hoehe {
                    baum.anhaengen(&b.hash());
                }
            })
            .map_err(|e| e.to_string())?;
        if baum.historie() != &self.zustand.historie {
            return Err("die Kettendatei trifft die Historie des Zustands nicht".into());
        }
        self.baum = Some(baum);
        Ok(())
    }

    /// **Eine Lieferung für jemanden, der die Geschichte nachlädt**
    /// (Stufe A4): die Blöcke `ab..=bis`, gekürzt auf eine Antwort, und der
    /// Beweis des letzten gegen die Historie mit `anzahl` Blöcken. `None`,
    /// wenn diese Kette nicht ab Genesis lebt oder der Bereich nicht vor
    /// `anzahl` liegt.
    pub fn archivlieferung(
        &mut self,
        ab: u64,
        bis: u64,
        anzahl: u64,
    ) -> Option<(Vec<Block>, myl_types::historie::Beweis)> {
        let n = self.baum.as_ref()?.historie().anzahl();
        if ab == 0 || ab > bis || bis >= anzahl || anzahl > n {
            return None;
        }
        let bloecke = crate::nachschub::in_die_antwort_mit(
            self.bloecke_von_bis(ab, bis),
            crate::nachschub::BEWEIS_HOECHSTENS_BYTES,
        );
        let letzte = bloecke.last()?.header.height;
        if bloecke.first()?.header.height != ab {
            return None;
        }
        let baum = self.baum.take()?;
        let beweis = baum.beweis_bei(letzte, anzahl, |g| self.gruppenblaetter(g));
        self.baum = Some(baum);
        Some((bloecke, beweis?))
    }

    /// Die Blatt-Hashes der Gruppe `g`: Blatt `i` ist der Hash von Block
    /// `i`, Blatt 0 der Startwert.
    fn gruppenblaetter(&mut self, g: u64) -> Option<Vec<Hash>> {
        use myl_types::historie::{blatt, GRUPPENSTUFE};
        let groesse = 1u64 << GRUPPENSTUFE;
        let erstes = g.checked_mul(groesse)?;
        let mut blaetter = Vec::with_capacity(groesse as usize);
        let mut h = erstes;
        if h == 0 {
            blaetter.push(blatt(&self.netz().startwert_hash()));
            h = 1;
        }
        while h < erstes + groesse {
            let teil = self.bloecke_von_bis(h, erstes + groesse - 1);
            if teil.first()?.header.height != h {
                return None;
            }
            for b in &teil {
                blaetter.push(blatt(&b.hash()));
            }
            h += teil.len() as u64;
        }
        Some(blaetter)
    }

    /// Die Ablage der Teile, wenn dieser Knoten Splitter ist.
    pub fn teilablage(&self) -> Option<crate::teilablage::Teilablage> {
        Some(crate::teilablage::Teilablage::neben(self.speicher.as_ref()?.pfad()))
    }

    /// Wo eine nachgeladene Geschichte liegt, bis sie eingefügt ist.
    pub fn archivpfad(&self) -> Option<std::path::PathBuf> {
        let p = self.speicher.as_ref()?.pfad();
        let mut n = p.as_os_str().to_owned();
        n.push(".archiv");
        Some(std::path::PathBuf::from(n))
    }

    /// Die letzte Verdichtung (neue Basis, Bytes vorher, nachher), einmal.
    pub fn verdichtung_nehmen(&mut self) -> Option<(u64, u64, u64)> {
        self.verdichtet.take()
    }

    /// Die Ablage der Abzüge; ab jetzt entsteht an jeder Epochengrenze einer.
    pub fn ablage_setzen(&mut self, ablage: crate::abzug::Ablage) {
        self.ablage = Some(ablage);
    }

    /// Die Ablage der Abzüge, falls eine geführt wird.
    pub fn ablage(&self) -> Option<&crate::abzug::Ablage> {
        self.ablage.as_ref()
    }

    /// Die Höhe, ab der diese Kette lebt (0 von Genesis).
    pub fn basis(&self) -> u64 {
        self.basis
    }

    /// Der zuletzt angelegte Abzug: Höhe und Blockhash.
    pub fn letzter_abzug(&self) -> Option<(u64, Hash)> {
        self.letzter_abzug
    }

    /// ⚑ **Die Kette beginnt beim Abzug** statt bei Genesis (2026-10-22).
    ///
    /// Nur auf einer leeren Kette, deren Kettendatei leer ist: Sonst
    /// schlösse der erste geschriebene Block an etwas an, das die Datei
    /// nicht kennt. Speicher, Ablage, Stimmsatz und Mempool bleiben; Höhe,
    /// letzter Hash, Zustand, Fenster und Basis kommen aus dem Abzug.
    /// **Geprüft ist der Abzug vorher**, gegen einen Anker oder gegen den
    /// eigenen Block ([`crate::abzug::Abzug::pruefen`]); hier wird nur
    /// eingesetzt.
    pub fn auf_abzug_setzen(&mut self, abzug: &crate::abzug::Abzug, zustand: LedgerState) -> Result<(), String> {
        if self.hoehe != 0 {
            return Err(format!("die Kette steht schon auf Höhe {}", self.hoehe));
        }
        if self.speicher.as_ref().is_some_and(|s| s.bloecke() > 0) {
            return Err("die Kettendatei ist nicht leer".into());
        }
        let hash = abzug.block.hash();
        self.hoehe = abzug.hoehe();
        self.letzter_hash = hash;
        self.zustand = zustand;
        self.verlauf.clear();
        self.bekannt.clear();
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(abzug.hoehe(), &abzug.block);
        self.basis = abzug.hoehe();
        self.baum = None;
        self.naechstes_pruefpunktzertifikat = None;
        self.offene_zuteilungen.clear();
        self.letzte_stichprobe = None;
        self.pods_der_stichprobe.clear();
        Ok(())
    }

    /// ⚑ **Vorspulen beim Wiederanlauf** (Fund 629): einen Block aus der
    /// eigenen Kettendatei übernehmen, ohne ihn anzuwenden, bis zu einem
    /// eigenen Abzug ([`Kette::abzug_einsetzen`]). Geprüft wird, was ohne
    /// Rechnung geht: Anschluss, Höhe, Epoche; Fenster, bekannte Hashes und
    /// Historienbaum wachsen wie beim Übernehmen. **Der Zustand steht danach
    /// nicht zur Höhe**, bis der Abzug eingesetzt ist; ausserhalb des
    /// Ladens ruft das niemand.
    pub fn vorspulen(&mut self, block: &Block) -> Result<(), KettenFehler> {
        if block.header.prev_block_hash != self.letzter_hash {
            return Err(KettenFehler::PasstNichtAn { erwartet: self.letzter_hash, bekommen: block.header.prev_block_hash });
        }
        if block.header.height != self.hoehe + 1 {
            return Err(KettenFehler::HoeheWeichtAb { erwartet: self.hoehe + 1, bekommen: block.header.height });
        }
        let epoche = epoche_fuer_hoehe(block.header.height);
        if block.header.epoch != epoche {
            return Err(KettenFehler::EpocheWeichtAb { erwartet: epoche, bekommen: block.header.epoch });
        }
        let hash = block.hash();
        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
        self.hoehe = block.header.height;
        self.letzter_hash = hash;
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(block.header.height, block);
        Ok(())
    }

    /// ⚑ **Den eigenen Abzug einsetzen**, nachdem bis zu seinem Block
    /// vorgespult ist (Fund 629).
    ///
    /// Gilt nur, wenn der vorgespulte letzte Block genau der Block des
    /// Abzugs ist; dessen Zustandswurzel bindet die Bytes
    /// ([`crate::abzug::Ablage::lesen`] hat sie geprüft), und die Kette von
    /// Hashes bindet den Block an die Basis. Lebt die Kette ab Genesis, muss
    /// zudem der Historienbaum aus der Datei die Historie im Zustand treffen.
    ///
    /// ⚑ **Was hier nicht nachgerechnet wird**, sind die Übergänge bis zum
    /// Abzug: Diese Blöcke hat der Knoten selbst angewandt, bevor er sie
    /// schrieb, und den Abzug danach selbst abgelegt. Ein vollständiges
    /// Nachspielen bei jedem Start bewies das nur noch einmal, und es kostete
    /// mit der Geschichte wachsend: im Archivlauf vom 2026-10-24 166 s für
    /// 349 646 Blöcke, und eine Kopie verlor darüber ihren Platz.
    pub fn abzug_einsetzen(&mut self, abzug: &crate::abzug::Abzug, zustand: LedgerState) -> Result<(), String> {
        if self.hoehe != abzug.hoehe() || self.letzter_hash != abzug.block.hash() {
            return Err(format!("vorgespult bis {}, der Abzug steht bei {}", self.hoehe, abzug.hoehe()));
        }
        if self.baum.as_ref().is_some_and(|b| b.historie() != &zustand.historie) {
            return Err("die Kettendatei trifft die Historie des Abzugs nicht".into());
        }
        self.zustand = zustand;
        Ok(())
    }

    /// Legt an einer Epochengrenze einen Abzug ab, wenn eine Ablage geführt
    /// wird. Ein Fehler beim Schreiben zählt als Schreibfehler und hält die
    /// Kette nicht an: Der Abzug ist eine Gefälligkeit an Neulinge, kein
    /// Teil des Konsenses.
    fn abzug_anlegen(&mut self, block: &Block) {
        let hoehe = block.header.height;
        if !crate::abzug::ist_grenze(hoehe) {
            return;
        }
        let Some(ablage) = self.ablage.as_ref() else {
            return;
        };
        let Ok(zustand) = borsh::to_vec(&self.zustand) else {
            self.schreibfehler += 1;
            return;
        };
        let abzug = crate::abzug::Abzug { block: block.clone(), zustand };
        match ablage.ablegen(&abzug) {
            Ok(()) => self.letzter_abzug = Some((hoehe, block.hash())),
            Err(_) => self.schreibfehler += 1,
        }
        if crate::abzug::ist_langzeit(hoehe) {
            self.verdichten_wenn_noetig();
        }
    }

    /// **Die Kettendatei unter ihr Budget bringen** (Funde 624 und 625,
    /// Festlegung des Projektinhabers vom 2026-10-23).
    ///
    /// Jeder Knoten, der nicht Kopie ist ([`crate::archiv`]), behält
    /// alles ab einem Langzeitabzug, und zwar **wenigstens ab dem vorletzten**.
    /// Liegt die Datei über dem Budget, wird der älteste Langzeitabzug zur
    /// Basis, nach dem höchstens noch die Hälfte des Budgets liegt; so
    /// läuft das selten und lässt reichlich Geschichte stehen. Reicht auch
    /// der vorletzte nicht, gilt er trotzdem: Die Mindestmenge geht vor dem
    /// Budget.
    ///
    /// Danach beginnt die Datei hinter der neuen Basis, genau wie bei einem
    /// Neuling aus einem Abzug, und ein Neustart rechnet ab dort nach statt
    /// ab Genesis. Ein Fehler zählt als Schreibfehler und lässt die alte
    /// Datei stehen.
    fn verdichten_wenn_noetig(&mut self) {
        let (Some(speicher), Some(ablage)) = (self.speicher.as_mut(), self.ablage.as_ref()) else {
            return;
        };
        let vorher = speicher.laenge();
        if self.budget == 0 || vorher <= self.budget {
            return;
        }
        // ⚑ Wer laut Zustand hält (Kopie, Halter, jeder vor der ersten
        //   Übergabe), verdichtet nicht.
        if self.zustand.archiv.behaelt(self.archivkennung.as_ref()) {
            return;
        }
        let langzeit: Vec<u64> = ablage
            .hoehen()
            .into_iter()
            .filter(|&h| crate::abzug::ist_langzeit(h) && h > self.basis)
            .collect();
        let Some(&spaetestens) = langzeit.len().checked_sub(2).and_then(|i| langzeit.get(i)) else {
            return;
        };
        let neue_basis = langzeit
            .iter()
            .copied()
            .filter(|&h| h <= spaetestens)
            .find(|&h| speicher.bytes_nach(h) <= self.budget / 2)
            .unwrap_or(spaetestens);
        let ergebnis = speicher
            .verdichten_vorbereiten(neue_basis)
            .map_err(|e| e.to_string())
            .and_then(|_| ablage.basis_setzen(neue_basis).map_err(|e| e.to_string()))
            // ⚑ Selbst gerechnet, also bestätigt: Sonst begänne der nächste
            //   Neustart eine Hintergrundprüfung ab Genesis, die niemand mehr
            //   bedienen kann.
            .and_then(|()| ablage.bestaetigt_setzen(neue_basis).map_err(|e| e.to_string()))
            .and_then(|()| speicher.verdichten_abschliessen().map_err(|e| e.to_string()));
        match ergebnis {
            Ok(()) => {
                self.basis = neue_basis;
                self.baum = None;
                let _ = ablage.raeumen();
                self.verdichtet = Some((neue_basis, vorher, speicher.laenge()));
            }
            Err(_) => self.schreibfehler += 1,
        }
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

    /// **Hält den Mempool klein** (Fund 627): Was seine Nummer im Zustand
    /// schon verbraucht hat, kann nie mehr gelten, und was länger als
    /// [`WARTEN_HOECHSTENS_HOEHEN`] wartet, kommt nicht mehr.
    ///
    /// ⚑ Das ersetzt den Vergleich über die Bytes, der je Block jede
    /// wartende Transaktion serialisierte. Im Archivlauf vom 2026-10-24
    /// lagen auf den übernehmenden Knoten über 5000 davon, weil jede, die
    /// nach ihrem Block über Gossip ankam, nie gestrichen wurde; die Knoten
    /// fielen dabei hinter den Erzeuger zurück. Was ein Block enthält, hat
    /// seine Nummer verbraucht, fällt hier also ebenso heraus.
    fn mempool_bereinigen(&mut self, hoehe: u64) {
        let zustand = &self.zustand;
        self.mempool.retain(|(seit, tx)| {
            hoehe.saturating_sub(*seit) < WARTEN_HOECHSTENS_HOEHEN
                && tx.nonce >= zustand.account(&tx.absender_adresse()).nonce
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
        // Ein Lesefehler bricht die Lieferung ab und wird gezählt; was das
        // Fenster hat, geht trotzdem hinaus.
        let mut aus_der_datei = match speicher.bloecke_ab(ab, bis_datei) {
            Ok(b) => b,
            Err(_) => {
                self.lesefehler += 1;
                Vec::new()
            }
        };
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

    /// Nimmt eine Transaktion in den Mempool; `false`, wenn ihre Nummer
    /// schon verbraucht ist oder der Mempool voll ist.
    ///
    /// Ob sie durchgeht, entscheidet sich beim Anwenden. Eine Vorprüfung
    /// hier bräuchte den Zustand **zum Zeitpunkt der Aufnahme**, und der
    /// ist nicht der beim Anwenden. ⚑ **Bis auf die Nummer** (Fund 627):
    /// Sie wächst nur, also gilt eine Transaktion unter der Nummer im
    /// Zustand auch später nie. Dazu die Grenzen aus Fund 633: insgesamt
    /// [`MEMPOOL_HOECHSTENS`], je Absender [`MEMPOOL_JE_ABSENDER`].
    pub fn aufnehmen(&mut self, tx: Transaktion) -> bool {
        let absender = tx.absender_adresse();
        if self.mempool.len() >= MEMPOOL_HOECHSTENS
            || tx.nonce < self.zustand.account(&absender).nonce
            || self.mempool.iter().filter(|(_, t)| t.absender_adresse() == absender).count() >= MEMPOOL_JE_ABSENDER
            || (ist_neuling(&self.zustand, &absender)
                && self.mempool.iter().filter(|(_, t)| ist_neuling(&self.zustand, &t.absender_adresse())).count()
                    >= MEMPOOL_NEULINGE_HOECHSTENS)
        {
            return false;
        }
        self.mempool.push((self.hoehe, tx));
        true
    }

    /// Liest eine Transaktion aus Gossip-Bytes und nimmt sie auf.
    pub fn aufnehmen_roh(&mut self, daten: &[u8]) -> bool {
        let mut rest = daten;
        match Transaktion::deserialize(&mut rest) {
            Ok(tx) if rest.is_empty() => self.aufnehmen(tx),
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
        let mitglieder = Self::unterzeichner_des_buendels(pod);
        myl_consensus::poi::PodMembership::ohne_besitznachweis(
            EpochId(epoche),
            myl_types::pod_kennung(epoche, pod.pod_index),
            erste.miner.miner_id,
            mitglieder,
        )
        .ok()
    }

    /// **Prüft ein PoI-Bündel gegen die laufende Epoche**, wie das
    /// Einreichen es täte: Pod aus der Zuteilung, Aggregat gegen die
    /// Positionen. Gibt die Kennung des Koordinators (erste Position) und
    /// ihre Netzadresse zurück. Für den Knoten, bevor er ein Bündel
    /// weitergibt oder einreicht: Ein gefälschtes soll gar nicht erst in die
    /// Kette.
    pub fn buendel_pruefen(
        zustand: &LedgerState,
        buendel: &myl_types::PoIBundle,
    ) -> Result<(myl_types::ids::MinerId, myl_types::latency_attest::PeerIdBytes), String> {
        if buendel.epoch != zustand.epoch {
            return Err(format!("Bündel für Epoche {}, laufend ist {}", buendel.epoch.0, zustand.epoch.0));
        }
        if zustand.buendel.contains_key(&buendel.pod) {
            return Err("für diesen Pod liegt schon ein Bündel vor".into());
        }
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let pod = myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, zustand.epoch.0, &buendel.pod)
            .ok_or("diesen Pod kennt die Zuteilung nicht")?;
        let mitgliedschaft = Self::mitgliedschaft(pod, zustand.epoch.0).ok_or("Pod ohne Positionen")?;
        myl_consensus::poi::verify_bundle_signature(buendel, &mitgliedschaft).map_err(|e| format!("Aggregat: {e:?}"))?;
        let erster = &pod.shards.first().ok_or("Pod ohne Positionen")?.miner;
        Ok((erster.miner_id, erster.netzadresse))
    }

    /// ⚑ **Wer ein PoI-Bündel unterschreibt: die Positionen, nicht die
    /// Reserve** (Fund 645, 2026-10-26). Hier stand `pod.mitglieder()`, also
    /// auch die Reserve. Die hat keinen Shard, sieht die Arbeit nie und kann
    /// nicht nachrechnen, was sie unterschriebe; ein Shard prüft vor seiner
    /// Unterschrift den Anspruch gegen die eigenen Segmente. Ein echter Pod
    /// mit Reserve bekam so nie ein Bündel durch. Dieselbe Menge prüft das
    /// Einreichen und die Zuschreibung am Epochenwechsel.
    fn unterzeichner_des_buendels(
        pod: &myl_scheduler::shard_assignment::Pod,
    ) -> Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> {
        pod.shards.iter().map(|s| (s.miner.miner_id, s.miner.schluessel)).collect()
    }

    /// Wie viele Shard-Positionen ein Pod dieser Probekette hat.
    ///
    /// ⚑ **Fest, solange die Kette eine Probekette ist.** Der Zuschnitt
    /// gehört ins Pipeline-Manifest und damit unter Governance; ein
    /// beweglicher Wert, den nur einer kennt, wäre schlimmer als ein
    /// fester, den beide Seiten sehen.
    const PROBE_SHARDS: u32 = 4;

    /// Was ein Pod je Epoche an Rechenleistung liefert, in vTFE.
    ///
    /// ⚑ **Spiegelt `myl_governance::Parameter::PodKapazitaet`**, und
    /// das ist ein Behelf, kein Entwurf. Der Knoten bindet
    /// `myl-governance` nicht ein und kann die Registry deshalb nicht
    /// lesen; die Zahl steht hier zum zweiten Mal.
    ///
    /// **Ein Test hält beide gegeneinander** (`tests/parameter.rs`).
    /// Laufen sie auseinander, rechnet die Kette gegen eine Auslastung,
    /// die niemand beschlossen hat.
    const POD_KAPAZITAET_VTFE: u64 = 3_600_000_000;

    /// Der Sockel der Trainingsmenge, in Basispunkten der
    /// Gesamtkapazität. Spiegelt `Parameter::TrainingsGrundrate`.
    const TRAININGS_GRUNDRATE_BPS: u32 = 200;

    /// γ_train, in Basispunkten der freien Kapazität. Spiegelt
    /// `Parameter::TrainingsFreianteil`.
    const TRAININGS_FREIANTEIL_BPS: u32 = 8_000;

    /// Zähler und Nenner der Lernrate eines Trainingssegments.
    ///
    /// ⚑ **Vom Protokoll gesetzt und nicht vom Pod gewählt.** Ein Pod,
    /// der seine eigene Lernrate wählte, wählte seinen eigenen
    /// Arbeitsaufwand: Eine winzige Schrittweite bewegt fast nichts und
    /// ist trotzdem eine ausgeführte Rechnung.
    ///
    /// ⚑ **Der Zähler ist eins, der Nenner folgt der Tiefe.** Bis zum
    /// 2026-09-06 stand hier ein fester Nenner von 2¹², und der war
    /// gemessen falsch: Über die vierundzwanzig Ebenen des 0,5B zerstört
    /// er das Modell in einem Lauf, Perplexität 2,5 Milliarden gegen
    /// einen Ausgangsstand von 23. Die Zahl stammte aus einem Lauf über
    /// **eine** Ebene gegen **ein** Zielwort.
    const TRAININGS_LR_ZAEHLER: i64 = 1;

    /// Der Nenner der Lernrate für ein Segment über `tiefe` Ebenen mit
    /// `gradienten` eingehenden Gradienten.
    ///
    /// ⚑ **Keine Konstante mehr, sondern eine Funktion von Tiefe und
    /// Chargengrösse** ([`myl_types::lernrate`]). Der Knoten wählt sie
    /// nicht; er rechnet sie aus, und der Übergang prüft sie gegen
    /// dieselbe Funktion.
    fn trainings_lr_nenner(tiefe: u32, gradienten: u32) -> i64 {
        myl_types::lernrate::lernrate_nenner(tiefe, gradienten)
    }

    /// Die Stimmberechtigten aus dem **Kettenzustand**, nicht aus der
    /// Genesis-Datei.
    ///
    /// # ⚑ Was daran der Unterschied ist
    ///
    /// Bis zum 2026-09-06 kam das Stimmgewicht aus
    /// `Stimmsatzdatei::stimmberechtigte`, also aus einer **Textdatei**.
    /// Damit wog eine Zahl, die niemand auf der Kette hinterlegt hatte,
    /// und der Einsatz, den `EinsatzHinterlegen` seit dem 2026-09-03
    /// bewegt, wog nichts.
    ///
    /// Hier folgt das Gewicht dem **hinterlegten Einsatz**, und der
    /// Schlüssel kommt aus dem Minerregister, wo `MinerAnmelden` ihn
    /// gegen die Kennung prüft. Beides steht im Zustand und ist damit
    /// nachrechenbar.
    ///
    /// ⚑ **Arbeit qualifiziert, Stake wiegt** (Entscheidung A3): Wer
    /// nicht im Register steht, ist nicht stimmberechtigt, gleich wie
    /// viel er hinterlegt hat; wer darin steht, wiegt mit seinem
    /// Einsatz.
    ///
    /// ⚑ **Unter der Mindestschwelle zählt niemand mit.** Ein Konto mit
    /// einem Satoshi Einsatz wäre sonst stimmberechtigt und verdünnte
    /// die Zweidrittelmehrheit.
    pub fn stimmberechtigte_aus_zustand(
        zustand: &LedgerState,
        mindesteinsatz: u64,
    ) -> myl_consensus::validator::VotingSet {
        let mitglieder = zustand
            .miner
            .values()
            .filter_map(|r| {
                let konto = Address::new(*r.miner_id.as_bytes());
                let einsatz = zustand.account(&konto).staked;
                if einsatz < mindesteinsatz {
                    return None;
                }
                Some((
                    r.miner_id,
                    myl_consensus::validator::VotingMember {
                        pubkey: r.schluessel,
                        weight: einsatz,
                    },
                ))
            })
            .collect();
        myl_consensus::validator::VotingSet::from_members(mitglieder)
    }

    /// Der Mindesteinsatz, ab dem jemand mitstimmt.
    ///
    /// ⚑ Spiegel von `Parameter::MindestStake`; ein Test hält beide
    /// Fassungen gegeneinander.
    pub fn mindesteinsatz() -> u64 {
        // ⚑ **Gerechnet und nicht getippt.** Derselbe Weg wie in der
        // Registry: `S_min = g / p²` aus Anhang B.1, mit der
        // Stichprobenrate als Konstante.
        // Anhang B.1: g = 0,5 MYL, dieselbe Herleitung wie in der
        // Registry.
        myl_tokenomics::s_min(
            myl_tokenomics::UNITS_PER_MYL / 2,
            myl_tokenomics::STICHPROBE_ZAEHLER,
            myl_tokenomics::STICHPROBE_NENNER,
        )
        .unwrap_or(u64::MAX)
    }

    /// Der Slashsatz für ein bewiesenes falsches Ergebnis.
    ///
    /// ⚑ **Spiegel von `myl_tokenomics::slashing::matrix()`**, Kap. 5.5,
    /// Zeile „Shard-Miner, falsches Ergebnis, 100 Prozent Stake", mit
    /// dem Kopfgeld aus Anhang B.3. Der Knoten zieht `myl-governance`
    /// nicht ein und kann die Registry nicht lesen; ein Test hält beide
    /// Fassungen gegeneinander.
    pub fn slashsatz_oeffentlich() -> myl_ledger::transitions::SlashParams {
        Self::slashsatz()
    }

    fn slashsatz() -> myl_ledger::transitions::SlashParams {
        myl_ledger::transitions::SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 1,
            bounty_fraction_num: myl_tokenomics::slashing::KOPFGELD_ZAEHLER,
            bounty_fraction_den: myl_tokenomics::slashing::KOPFGELD_NENNER,
        }
    }

    /// Die Tiefe, über die ein Trainingssegment dieser Kette rechnet.
    ///
    /// ⚑ **Das ganze Modell und nicht der Shard.** Ein Pod schreibt alle
    /// Ebenen fort, geshardet; die Verkettung, an der die Lernrate
    /// hängt, läuft über alle. Wer je Shard rechnete, bekäme für
    /// dieselbe Arbeit verschiedene Raten, je nachdem, in wie viele
    /// Stücke sie geschnitten war.
    fn trainingstiefe(zustand: &LedgerState) -> u32 {
        crate::netz::Netz::des_zustands(zustand).profil.num_layers as u32
    }

    /// Die drei Spiegelwerte, damit `tests/parameter.rs` sie gegen die
    /// Registry halten kann.
    pub fn pod_kapazitaet_vtfe() -> u64 {
        Self::POD_KAPAZITAET_VTFE
    }
    /// Siehe [`Kette::pod_kapazitaet_vtfe`].
    pub fn trainings_grundrate_bps() -> u32 {
        Self::TRAININGS_GRUNDRATE_BPS
    }
    /// Siehe [`Kette::pod_kapazitaet_vtfe`].
    pub fn trainings_freianteil_bps() -> u32 {
        Self::TRAININGS_FREIANTEIL_BPS
    }


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
    /// ⚑ **Die Probeprägung der abgelaufenen Epoche** (2026-10-26): je
    /// angemeldetem Miner mit Auszahlungskonto der Betrag des Netzes, ausser
    /// er war in dieser Epoche gesperrt. Auf jedem Netz ausser der
    /// Probekette ist der Betrag null, und hier geschieht nichts.
    ///
    /// ⚑ **Neben der Ausschüttung, nicht in ihr**: Diese MYL entstehen ohne
    /// Arbeitsnachweis und ohne Burn, und sie sollen in keiner Rechnung
    /// auftauchen, die einmal echtes Geld abrechnet. Fällt die Probeprägung
    /// weg, ändert sich an der Ausschüttung nichts.
    ///
    /// Wer zehn Miner auf ein Wallet zeigen lässt, bekommt zehnmal; auf der
    /// Probekette ist das gewollt einfach.
    fn probepraegung(zustand: &mut LedgerState, alte_epoche: u64) {
        let betrag = crate::netz::Netz::des_zustands(zustand).probepraegung;
        if betrag == 0 {
            return;
        }
        let empfaenger: Vec<Address> = zustand
            .auszahlung
            .iter()
            .filter(|(m, _)| zustand.miner.contains_key(*m))
            .filter(|(m, _)| zustand.fristsperren.get(*m).map(|e| e.0) != Some(alte_epoche))
            .map(|(_, konto)| *konto)
            .collect();
        for konto in empfaenger {
            let _ = myl_ledger::transitions::praegen(zustand, &konto, betrag);
        }
    }

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
            // ⚑ **Jeder Pod nach seinem Schnitt** (Stufe 4, 2026-10-22): Die
            //   Gewichte folgen aus seinen Grenzen und dem Profil des Netzes.
            //   Die gemeinsame Probeverteilung galt nur, solange jeder Pod
            //   gleich schnitt. Ein Pod ohne Schnitt bekommt nichts.
            let Some(grenzen) = Self::grenzen_des_pods(zustand, pod) else {
                continue;
            };
            let netz = crate::netz::Netz::des_zustands(zustand);
            let grenzen: Vec<(u64, u64)> = grenzen.iter().map(|&(a, b)| (a as u64, b as u64)).collect();
            let Some(eigene) = myl_tokenomics::vtfe::arbeitsverteilung_aus_grenzen(&netz.profil, &grenzen, netz.pipeline())
            else {
                continue;
            };
            abrechnungen.push(myl_tokenomics::Podabrechnung {
                positionen: pod
                    .shards
                    .iter()
                    .map(|s| (s.miner.miner_id, s.shard_index))
                    .collect(),
                reserve: pod.reserve.iter().map(|m| m.miner_id).collect(),
                vtfe_pod: buendel.vtfe_claimed,
                verteilung: Some(eigene),
            });
            bezeugt.push(buendel.clone());
        }
        (
            myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, &abrechnungen)
                .unwrap_or_default(),
            bezeugt,
        )
    }

    /// Setzt den Stimmsatz, gegen den Saatquellen und Erzeuger geprüft
    /// werden.
    pub fn stimmsatz_setzen(&mut self, satz: myl_consensus::validator::VotingSet) {
        self.stimmsatz = Some(satz);
    }

    /// Setzt den Schlüssel, mit dem dieser Knoten seine Blöcke
    /// unterschreibt (2026-10-27).
    pub fn erzeugerschluessel_setzen(&mut self, schluessel: crate::schluessel::Konsensschluessel) {
        self.erzeugerschluessel = Some(schluessel);
    }

    /// **Trägt der Erzeuger dieses Blocks?**
    ///
    /// ⚑ **Seit dem 2026-10-27 braucht jeder Block die Unterschrift eines
    /// Mitglieds des Stimmsatzes**, unter genau dem Schlüssel, den der Satz
    /// für es führt (Festlegung des Projektinhabers: Stimmsatz ab Genesis,
    /// immer Pflicht). Unsignierte Blöcke gibt es nicht mehr; eine
    /// Unterschrift, die nicht trägt, nie (Fund 192).
    ///
    /// Hat die Kette noch keinen Satz, gilt das nur auf einer Probekette und
    /// nur bis Block 1: Wer ihn trägt, wird das einzige Mitglied
    /// ([`Self::erzeuger_lernen`]).
    fn erzeuger_traegt(&self, block: &Block) -> bool {
        let (Ok(Some(kennung)), Some(u)) = (block.erzeuger(), &block.unterschrift) else {
            return false;
        };
        match &self.stimmsatz {
            Some(satz) => satz.pubkey(&kennung) == Some(&u.schluessel),
            None => self.lernt_erzeuger,
        }
    }

    /// **Der Stimmsatz aus dem ersten Block** einer Probekette ohne
    /// Stimmsatzdatei: sein Erzeuger als einziges Mitglied, Gewicht eins.
    /// Nur, wenn die Kette lernen darf und noch keinen Satz hat.
    fn erzeuger_lernen(&mut self, block: &Block) {
        if self.stimmsatz.is_some() {
            return;
        }
        self.stimmsatz = self.satz_aus_block(block);
    }

    /// Der Satz, den eine lernende Kette aus diesem Block lernen würde: sein
    /// Erzeuger allein, Gewicht eins. `None`, wenn sie nicht lernt.
    fn satz_aus_block(&self, block: &Block) -> Option<myl_consensus::validator::VotingSet> {
        if !self.lernt_erzeuger {
            return None;
        }
        let (Ok(Some(kennung)), Some(u)) = (block.erzeuger(), &block.unterschrift) else {
            return None;
        };
        let mut mitglieder = std::collections::BTreeMap::new();
        mitglieder.insert(kennung, myl_consensus::validator::VotingMember { pubkey: u.schluessel, weight: 1 });
        Some(myl_consensus::validator::VotingSet::from_members(mitglieder))
    }

    /// Der Hash des eigenen Blocks an `hoehe`, solange er im Fenster liegt.
    pub fn hash_an(&self, hoehe: u64) -> Option<Hash> {
        self.verlauf.get(&hoehe).map(|b| b.hash())
    }

    /// Der eigene Block an `hoehe`, solange er im Fenster liegt.
    pub fn block_an(&self, hoehe: u64) -> Option<&Block> {
        self.verlauf.get(&hoehe)
    }

    /// **Trägt dieses Zertifikat in einem Block der Höhe `hoehe`?**
    /// (2026-10-27, CONSENSUS 7.2)
    ///
    /// 1. **Ein Prüfpunkt nach dem zuletzt bestätigten** und vor diesem
    ///    Block. Ein älteres Zertifikat sagte nichts Neues, und eine Lücke
    ///    ist erlaubt: Die Endgültigkeit eines späteren Prüfpunkts deckt
    ///    die früheren mit.
    /// 2. **Der eigene Block an dieser Höhe.** Ein Zertifikat für einen
    ///    anderen Zweig ist keines für diese Kette.
    /// 3. **Mehr als zwei Drittel des Stimmsatzes** über die
    ///    Prüfpunktbotschaft. Ohne Satz trägt keines.
    fn zertifikat_traegt(&self, z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat, hoehe: u64) -> bool {
        self.stimmsatz.as_ref().is_some_and(|satz| self.zertifikat_traegt_gegen(z, hoehe, satz))
    }

    fn zertifikat_traegt_gegen(
        &self,
        z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat,
        hoehe: u64,
        satz: &myl_consensus::validator::VotingSet,
    ) -> bool {
        z.hoehe > self.zustand.pruefpunkt_bestaetigt
            && z.hoehe < hoehe
            && self.hash_an(z.hoehe) == Some(z.block_hash)
            && z.verify(satz).is_ok()
    }

    /// **Bucht einen Doppelbeleg** (2026-10-27, CONSENSUS 7.6).
    ///
    /// 1. Der Beleg trägt ([`myl_consensus::double_signing::Doppelbeleg::schuldiger`]).
    /// 2. **Die Tat liegt in der Sperrfrist**: Ältere Widersprüche sind nicht
    ///    mehr schlachtbar, das ist dieselbe Grenze wie die Haftung des
    ///    Einsatzes (und der Grund der Vertrauensfrist).
    /// 3. **Einmal je (Schuldiger, Art, Höhe)**: Ein zweiter Beleg für
    ///    denselben Widerspruch schlachtet nicht noch einmal.
    /// 4. Geschlachtet wird nach der Matrix (Validator, Doppelsignatur oder
    ///    Zensur: 30, 65, 100 % gestaffelt), das Kopfgeld geht an den
    ///    Melder; wer sich selbst meldet, wird abgewiesen
    ///    (`myl_ledger::apply_verdict`). Ohne Einsatz gibt es nichts zu
    ///    schlachten, und der Beleg bleibt ohne Folge.
    fn doppelbeleg_buchen(
        zustand: &mut LedgerState,
        melder: &Address,
        beleg: &myl_consensus::double_signing::Doppelbeleg,
    ) -> Result<(), String> {
        let pk = beleg.schuldiger().map_err(|e| e.to_string())?;
        let tat_epoche = epoche_fuer_hoehe(beleg.hoehe());
        if tat_epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN) < zustand.epoch.0 {
            return Err("die Tat liegt vor der Sperrfrist".into());
        }
        let schuldig = Address::aus_schluessel(&pk);
        let schluessel = (schuldig, beleg.art(), beleg.hoehe());
        if zustand.doppelbelege.contains_key(&schluessel) {
            return Err("dieser Widerspruch ist schon gebucht".into());
        }
        let urteil = myl_ledger::transitions::Verdict {
            segment_id: myl_types::ids::SegmentId::new(Hash::sha256(&borsh::to_vec(beleg).map_err(|e| e.to_string())?).0),
            miner: schuldig,
            checker: *melder,
            outcome: myl_ledger::transitions::VerdictOutcome::SlashMiner,
        };
        myl_tokenomics::slashing::urteil_buchen_gestaffelt(
            zustand,
            &urteil,
            myl_tokenomics::slashing::Akteur::Validator,
            myl_tokenomics::slashing::Grund::DoubleSigningOderZensur,
        )
        .map_err(|e| format!("{e:?}"))?;
        let epoche = zustand.epoch.0;
        zustand.doppelbelege.insert(schluessel, epoche);
        Ok(())
    }

    /// Prüft das Zertifikat im Kopf eines Blocks; ohne Zertifikat nichts.
    ///
    /// 📌 Eine lernende Kette ohne Satz (etwa nach einem Abzug) prüft gegen
    /// den Satz, den sie aus genau diesem Block lernt; sonst wiese sie den
    /// ersten Block nach dem Abzug ab, sobald er ein Zertifikat trägt.
    fn pruefpunkt_traegt(&self, block: &Block) -> bool {
        let Some(z) = &block.header.pruefpunkt else {
            return true;
        };
        match &self.stimmsatz {
            Some(satz) => self.zertifikat_traegt_gegen(z, block.header.height, satz),
            None => self.satz_aus_block(block).is_some_and(|satz| self.zertifikat_traegt_gegen(z, block.header.height, &satz)),
        }
    }

    /// **Bietet ein Zertifikat für den nächsten erzeugten Block an.**
    ///
    /// ⚑ **Nur der Erzeuger legt es in einen Block; alle anderen lesen es
    /// daraus.** Behalten wird das höchste, denn es deckt die früheren mit.
    pub fn pruefpunkt_anbieten(&mut self, z: myl_consensus::pruefpunkt::Pruefpunktzertifikat) {
        if z.hoehe <= self.zustand.pruefpunkt_bestaetigt {
            return;
        }
        if self.naechstes_pruefpunktzertifikat.as_ref().is_none_or(|alt| z.hoehe > alt.hoehe) {
            self.naechstes_pruefpunktzertifikat = Some(z);
        }
    }

    /// Bis zu welcher Höhe die Kette bestätigt ist.
    pub fn bestaetigt_bis(&self) -> u64 {
        self.zustand.pruefpunkt_bestaetigt
    }

    /// Die Höhe des angebotenen, noch nicht eingetragenen Zertifikats.
    pub fn naechstes_pruefpunktzertifikat_hoehe(&self) -> Option<u64> {
        self.naechstes_pruefpunktzertifikat.as_ref().map(|z| z.hoehe)
    }

    /// Der Schlüssel, mit dem diese Kette ihre Blöcke unterschreibt.
    pub fn erzeugerschluessel(&self) -> Option<&crate::schluessel::Konsensschluessel> {
        self.erzeugerschluessel.as_ref()
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
    pub fn zuteilung_der_laufenden_epoche(
        zustand: &LedgerState,
    ) -> myl_scheduler::shard_assignment::Zuteilung {
        let register = angemeldete_miner(zustand);
        // ⚑ **Nach den Zusagen** (Stufe 4, 6.7, 2026-10-22): kein Pod ohne
        //   Zusage für das Podmodell, nur machbare Pods. Ohne Podmodell gilt
        //   keine Modellzusage, also gibt es keinen Pod.
        let Some(podmodell) = zustand.podmodell else {
            return myl_scheduler::shard_assignment::Zuteilung { ohne_zusage: register, ..Default::default() };
        };
        let epoche = zustand.epoch;
        myl_scheduler::zonenzuteilung::zuteilung_der_epoche_mit_zusagen(
            &register,
            epoche.0,
            &zustand.epochensaat,
            Self::PROBE_SHARDS,
            &podmodell,
            &|m| myl_ledger::transitions::modellzusage_der_epoche(zustand, m, epoche),
        )
    }

    /// **Die Grenzen eines Pods der laufenden Epoche**, aus den Zusagen
    /// seiner Mitglieder: dieselbe Funktion, nach der die Pod-Bildung
    /// geprüft hat und nach der der Pod rechnet. `None` ohne Podmodell oder
    /// wenn er nicht machbar ist (das schließt die Pod-Bildung aus).
    pub fn grenzen_des_pods(
        zustand: &LedgerState,
        pod: &myl_scheduler::shard_assignment::Pod,
    ) -> Option<Vec<(u32, u32)>> {
        let podmodell = zustand.podmodell?;
        let epoche = zustand.epoch;
        let positionen: Vec<myl_types::ids::MinerId> = pod.shards.iter().map(|s| s.miner.miner_id).collect();
        let reserve: Vec<myl_types::ids::MinerId> = pod.reserve.iter().map(|r| r.miner_id).collect();
        myl_scheduler::ebenenverteilung::bereiche_des_pods(
            &positionen,
            &reserve,
            &|m| myl_ledger::transitions::modellzusage_der_epoche(zustand, m, epoche),
            &podmodell,
        )
        .grenzen()
    }

    /// Die Zuschreibung der **Trainingsarbeit** der abgelaufenen Epoche.
    ///
    /// # ⚑ Gleiche Rate wie Inferenz, andere Quelle
    ///
    /// Ein Trainingssegment wird je Rechenstunde genauso vergütet wie
    /// ein Inferenzsegment. Der Deckel aus Kap. 5.6 begründet sich in
    /// Anhang B.7.3 damit, dass Miner zwischen den Arbeitsklassen
    /// **wählen**; seit die Zuteilung erzwungen ist, wählt niemand, und
    /// derselbe Anhang sagt: „Bei Gleichstand entscheidet allein die
    /// Zuteilung."
    ///
    /// Bezahlt wird aus der **Treasury** und nicht aus der
    /// Shard-Miner-Quote; die Quelle bleibt, was Kap. 5.6 festlegt.
    ///
    /// ⚑ **Nur bestätigte Paare**, denn nur sie sind nachgerechnet
    /// worden. Ein Segment, dem der Partner fehlt, ist Arbeit, die
    /// niemand prüfen kann.
    ///
    /// ⚑ **Die Tokenzahl kommt aus dem verankerten Korpus**, nicht vom
    /// Pod. Dürfte er sie nennen, nennte er seine eigene Vergütung.
    fn trainingszuschreibung(
        zustand: &LedgerState,
        paare: &[(u32, u32)],
        epoche: u64,
    ) -> myl_tokenomics::Zuschreibung {
        let mut zu = myl_tokenomics::Zuschreibung::default();
        let Some(korpus) = zustand.korpus.as_ref() else {
            return zu;
        };
        if !korpus.traegt_ein_buendel() {
            return zu;
        }
        let tokens = korpus.tokens_je_buendel();
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let profil = crate::netz::Netz::des_zustands(zustand).profil;
        let bestaetigt = Self::bestaetigte_deltas(zustand, paare, epoche);
        if bestaetigt.is_empty() {
            return zu;
        }
        for (a, b) in paare {
            let ka = myl_types::miner::pod_kennung(epoche, *a);
            let Some(seg) = zustand.trainingssegmente.get(&ka) else {
                continue;
            };
            if !bestaetigt.contains(&seg.delta_commitment) {
                continue;
            }
            for nr in [a, b] {
                let Some(pod) = zuteilung.pods.iter().find(|p| p.pod_index == *nr) else {
                    continue;
                };
                // ⚑ **Der Schnitt aus einer Stelle** (Stufe 4, 2026-10-21):
                //   dieselbe Funktion, nach der der Pod rechnet. Hier stand
                //   der gleiche Schnitt, selbst ausgerechnet.
                let schnitt = Self::grenzen_des_pods(zustand, pod);
                let Some(grenzen) = schnitt else {
                    continue;
                };
                for (i, sh) in pod.shards.iter().enumerate() {
                    let zuschnitt = myl_tokenomics::vtfe::ShardZuschnitt {
                        layer_start: grenzen[i].0 as u64,
                        layer_end: grenzen[i].1 as u64,
                        hat_embedding: i == 0,
                        hat_lm_kopf: i + 1 == pod.shards.len(),
                    };
                    let Ok(g) = myl_tokenomics::vtfe::vtfe_training_gutschrift(
                        &profil,
                        &zuschnitt,
                        tokens,
                        u64::from(seg.schrittzahl),
                    ) else {
                        continue;
                    };
                    let e = zu.je_miner.entry(sh.miner.miner_id).or_insert(0);
                    *e = e.saturating_add(g);
                }
            }
        }
        zu
    }

    /// Die Δ-Commitments, auf die sich beide Pods eines Paars geeinigt
    /// haben, kanonisch geordnet.
    ///
    /// # ⚑ Warum ein Paar und nicht ein einzelnes Segment zählt
    ///
    /// Ein Ergebnis, das nur einer gerechnet hat, **liesse sich mit
    /// nichts vergleichen**. Die Kette könnte es annehmen oder
    /// verwerfen, aber nie prüfen, und die Modellfassung hinge an einer
    /// Behauptung. Dieselbe Regel wie bei der Inferenz (Kap. 4.4,
    /// `r = 2`); beim Training kann sie nicht schwächer sein, denn ein
    /// falsches Δm verdirbt nicht eine Antwort, sondern das Modell.
    ///
    /// ⚑ **Der Auftrag wird mitgeprüft, nicht nur das Ergebnis.** Zwei
    /// Pods, die Verschiedenes gerechnet haben, sind nicht uneinig,
    /// sondern unvergleichbar, und ihre Übereinstimmung im Ergebnis
    /// wäre ein Zufall, keine Bestätigung.
    ///
    /// ⚑ **Sortiert und nicht in Paarreihenfolge.** Die Ordnung geht in
    /// die neue Modellfassung ein; sie an der Paarbildung aufzuhängen
    /// hiesse, zwei Dinge zu koppeln, die nichts miteinander zu tun
    /// haben.
    fn bestaetigte_deltas(
        zustand: &LedgerState,
        paare: &[(u32, u32)],
        epoche: u64,
    ) -> Vec<Hash> {
        let mut bestaetigt: Vec<Hash> = Vec::new();
        for (a, b) in paare {
            let ka = myl_types::miner::pod_kennung(epoche, *a);
            let kb = myl_types::miner::pod_kennung(epoche, *b);
            let (Some(sa), Some(sb)) =
                (zustand.trainingssegmente.get(&ka), zustand.trainingssegmente.get(&kb))
            else {
                continue;
            };
            if sa.charge == sb.charge
                && sa.modell_version == sb.modell_version
                && sa.lr_zaehler == sb.lr_zaehler
                && sa.lr_nenner == sb.lr_nenner
                && sa.schrittzahl == sb.schrittzahl
                && sa.delta_commitment == sb.delta_commitment
            {
                bestaetigt.push(sa.delta_commitment);
            }
        }
        bestaetigt.sort_unstable_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
        bestaetigt
    }

    /// Welche Pods der laufenden Epoche trainieren statt zu rechnen.
    ///
    /// # ⚑ Der Weg, und er war bis zum 2026-09-05 nicht da
    ///
    /// `myl_scheduler::trainingszuteilung` beantwortet die Frage, und
    /// bis heute stellte sie niemand. Dasselbe galt für die ganze Kiste
    /// `myl-train` (Fund 183) und für die Auslastungsrechnung
    /// (Fund 184): gebaut, geprüft, unerreicht.
    ///
    /// ⚑ **Die Nachfrage kommt aus der Vorepoche**
    /// ([`LedgerState::vtfe_vorepoche`]), nicht aus der laufenden. Die
    /// Zuteilung steht fest, bevor die Epoche läuft; wer die laufende
    /// Nachfrage nähme, bräuchte eine Zahl, die es noch nicht gibt.
    ///
    /// ⚑ **Und dieselbe Saat wie die Podbildung**, denn beide gehören
    /// zur selben Epoche. Innerhalb der Auswahl wird sie noch einmal
    /// abgeleitet, damit Trainingslast und Shard-Position nicht an
    /// derselben Permutation hängen.
    pub fn trainingsplan_der_laufenden_epoche(
        zustand: &LedgerState,
    ) -> myl_scheduler::trainingszuteilung::Trainingsplan {
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        myl_scheduler::trainingszuteilung::plane(
            &zuteilung,
            zustand.vtfe_vorepoche,
            Self::POD_KAPAZITAET_VTFE,
            &myl_scheduler::trainingszuteilung::Trainingsraten {
                grundrate_bps: Self::TRAININGS_GRUNDRATE_BPS,
                freianteil_bps: Self::TRAININGS_FREIANTEIL_BPS,
            },
            &zustand.trainingsstand,
            zustand.korpus.as_ref(),
            &zustand.epochensaat.0,
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
            // 📌 Fund 658: die Epoche der letzten Ziehung, aus den
            //    festgehaltenen Pods.
            if self.stichprobenepoche() != Some(epoche) {
                return None;
            }
            return self
                .pods_der_stichprobe
                .iter()
                .find(|p| myl_types::pod_kennung(epoche, p.pod_index) == *kennung)
                .cloned();
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

    /// **Zieht die Stichproben, deren Epochenwechsel dieses Zertifikat
    /// bestätigt** (Punkt 45).
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
    /// ⚑ **Seit dem 2026-10-27 wird mit dem Zertifikat gezogen** (CONSENSUS
    /// 7.2, Festlegung des Projektinhabers): Beim Wechsel hebt der Zustand
    /// die Anteile auf ([`myl_ledger::state::OffeneStichprobe`]), und
    /// gezogen wird, sobald ein Block das Zertifikat eines Prüfpunkts an
    /// oder nach dem Wechsel trägt. Es entsteht, nachdem die Bündel
    /// feststehen, also kennt es beim Einreichen niemand. Der Rückfall auf
    /// den Blockhash (Fund 120, unbegrenzter Mahlraum) ist entfallen.
    fn offene_stichproben_ziehen(
        zustand: &mut LedgerState,
        z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        let (faellig, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut zustand.stichproben_offen)
            .into_iter()
            .partition(|o| (o.epoche + 1).saturating_mul(BLOECKE_JE_EPOCHE) <= z.hoehe);
        zustand.stichproben_offen = rest;
        for o in faellig {
            let saat = crate::stichprobe::saat_aus_zertifikat(&z.aggregat, &z.unterzeichner, o.epoche);
            let stichprobe = crate::stichprobe::stichprobe_aus_anteilen(&o.anteile, &saat, Self::STICHPROBE_BP);
            *gezogen = Some((o.epoche, stichprobe));
        }
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
    fn epochenwechsel_abschliessen(zustand: &mut LedgerState, neue_epoche: u64, letzter_hash: &Hash) {
        if neue_epoche <= zustand.epoch.0 {
            return;
        }
        let alte_epoche = zustand.epoch.0;
        let (zuschreibung, bezeugt) = Self::zuschreibung_der_epoche(zustand);
        // ⚑ **Die Trainingszuschreibung entsteht VOR der Ausschüttung**
        // und aus dem Zustand, den die abgelaufene Epoche hinterlassen
        // hat. Danach sind die Segmente weg.
        let trainingsplan = Self::trainingsplan_der_laufenden_epoche(zustand);
        let trainingszuschreibung =
            Self::trainingszuschreibung(zustand, &trainingsplan.paare, alte_epoche);
        let ausschuettung = myl_tokenomics::ausschuettung::epochenausschuettung_mit_training(
            zustand,
            &zuschreibung,
            &trainingszuschreibung,
            // ⚑ **Die Auslastung steuert die Trainingsabgabe**: bei
            // hoher Last führt das Netz ab, bei niedriger zahlt es aus.
            // Sie kommt aus demselben Plan, aus dem die Trainingspods
            // kommen; eine zweite Rechnung wäre eine zweite Wahrheit.
            trainingsplan.auslastung,
            &Self::praegeparameter(),
        );
        // ⚑ **Die Archivpflicht der abgelaufenen Epoche** (Stufe A5):
        //   Gutschrift aus dem, was die Treasury eben bekam, verfehlte
        //   Epochen, Verstoß und Nachrücken.
        let treasury_eingang = ausschuettung
            .ok()
            .and_then(|a| a.je_konto.get(&myl_types::treasury::treasury_adresse()).copied())
            .unwrap_or(0);
        let _ = myl_ledger::archiv::epoche_abschliessen(
            zustand,
            alte_epoche,
            treasury_eingang,
            crate::archiv::WARTE_EPOCHEN,
        );
        // ⚑ **Stufe 2 wird gezogen, und zwar hier** (Punkt 45, Fund
        // 114). Vorher lief sie in keinem Knoten, und `p` aus Anhang
        // B.1 war null. Gezogen wird aus den **bezeugten** Bündeln,
        // also denen, deren Aggregatsignatur galt.
        //
        // ⚑ **Seit dem 2026-10-27 wird hier nur aufgehoben** (CONSENSUS
        // 7.2): die Anteile der bezeugten Bündel, gezogen wird mit dem
        // Zertifikat des Prüfpunkts an diesem Wechsel
        // (`offene_stichproben_ziehen`).
        zustand.stichproben_offen.push(myl_ledger::state::OffeneStichprobe {
            epoche: alte_epoche,
            anteile: bezeugt.iter().map(|b| (b.pod, b.segmente)).collect(),
        });
        // Gebuchte Doppelbelege fallen nach der Sperrfrist weg (CONSENSUS
        // 7.6); ältere Taten sind ohnehin nicht mehr schlachtbar.
        zustand
            .doppelbelege
            .retain(|_, gebucht| gebucht.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN) >= neue_epoche);
        Self::gebuehren_teilen(zustand);
        // ⚑ **Abgelaufene Sitzungen fallen weg, ihre Hinterlegung geht
        //   zurück** (2026-10-27, Fund 662). Vorher rief die Kette
        //   `sitzung_aufraeumen` nie, und jede Sitzung blieb für immer im
        //   Zustand.
        if myl_ledger::transitions::sitzung_aufraeumen(zustand) > 0 {
            let weg: Vec<[u8; 32]> = zustand
                .hinterlegungen
                .keys()
                .filter(|(art, k)| {
                    *art == HINTERLEGT_SITZUNG
                        && !zustand.sitzungen.contains_key(&myl_types::ids::SitzungId::new(*k))
                })
                .map(|(_, k)| *k)
                .collect();
            for k in weg {
                Self::zurueckgeben(zustand, HINTERLEGT_SITZUNG, k);
            }
        }
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
        // ⚑ **Und die Trainingssegmente ebenso** (2026-09-05). Aus
        // demselben Grund: Eine je Epoche wachsende Menge im Zustand
        // bräche Entscheidung D7, weil `commitment` den ganzen Zustand
        // serialisiert.
        //
        // ⚑ **Auch sie werden heute verworfen, ohne zugeschrieben zu
        // werden.** Die Vergütung eines Trainingssegments hängt an der
        // Prüfung, und die steht noch nicht: Redundanz und Bisektion auf
        // Trainingssegmente sind offen. Gewonnen ist, dass der Weg in
        // die Kette steht.
        // ⚑ **Was zwei Pods übereinstimmend gerechnet haben, bewegt das
        // Modell** (2026-09-05). Alles andere nicht: Ein Ergebnis, das
        // nur einer geliefert hat, liesse sich mit nichts vergleichen,
        // und die Modellfassung hinge an einer Behauptung.
        //
        // ⚑ **Die Kette summiert die Δm nicht.** Sie hält das Rezept aus
        // alter Fassung und geordneter Liste der bestätigten
        // Commitments; wer die Gewichte hat, wendet es an und rechnet
        // die Wurzel nach.
        {
            let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
            let bestaetigt = Self::bestaetigte_deltas(zustand, &plan.paare, alte_epoche);
            zustand.modell_version =
                myl_types::modellversion::naechste_version(&zustand.modell_version, &bestaetigt);
        }
        let _ = myl_ledger::transitions::trainingssegmente_leeren(zustand);
        Self::probepraegung(zustand, alte_epoche);
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
        // ⚑ **Wer in der abgelaufenen Epoche zum Training herangezogen
        // war, wird vermerkt** (2026-09-05). Ohne diesen Eintrag bliebe
        // `trainingsstand` leer, die Reihum-Ordnung hätte nichts zu
        // ordnen, und die Auswahl fiele auf das blosse Los zurück.
        //
        // ⚑ **Vor dem Drehen der Saat**, aus demselben Grund, aus dem
        // der Abschluss davor steht: Der Plan der abgelaufenen Epoche
        // hängt an der Saat, die **während** ihr galt. Danach wäre es
        // der Plan einer Epoche, die es nie gab.
        {
            let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
            let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
            myl_scheduler::trainingszuteilung::stand_fortschreiben(
                &mut zustand.trainingsstand,
                &zuteilung.pods,
                &plan.pods,
                alte_epoche,
            );
        }
        // ⚑ **Und die Saat rückt weiter** (Fund 143), siehe oben.
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
        pruefpunkt: Option<&myl_consensus::pruefpunkt::Pruefpunktzertifikat>,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) -> Vec<bool> {
        // ⚑ **Die Historie zuerst** (Archivknoten, Stufe A2): der Hash des
        //   Vorgängers, also des Blocks, an den dieser anschliesst. Erzeuger
        //   und Übernehmer gehen beide hier durch und hängen dasselbe an.
        zustand.historie.anhaengen(letzter_hash);
        Self::epochenwechsel_abschliessen(zustand, epoch, letzter_hash);
        // ⚑ **Das Zertifikat eines Prüfpunkts** (2026-10-27, CONSENSUS
        //   7.2): bestätigt bis zu seiner Höhe, und was auf einen
        //   bestätigten Wechsel wartete, wird jetzt gezogen. Geprüft ist es,
        //   bevor `anwenden` gerufen wird.
        if let Some(z) = pruefpunkt {
            zustand.pruefpunkt_bestaetigt = z.hoehe;
            Self::offene_stichproben_ziehen(zustand, z, gezogen);
        }
        // ⚑ **Die Ziehung der Archivkopien** (Stufe A5) am Grenzblock, nach
        //   dem Abschluss der letzten Epoche der alten Periode und vor den
        //   Transaktionen: Die Historie zählt jetzt genau die Blöcke vor der
        //   Grenze.
        let hoehe = zustand.historie.anzahl();
        if hoehe > 0 && hoehe.is_multiple_of(crate::archiv::PERIODE_BLOECKE) {
            myl_ledger::archiv::grenze_ziehen(zustand, hoehe, epoch, crate::archiv::WARTE_EPOCHEN);
        }
        // ⚑ **Die Epoche des Zustands wird mitgeführt** (seit
        // 2026-08-27). Vorher stand sie auf 0 und blieb dort: `anwenden`
        // benutzte die Epoche nur, um den Verfall auszurechnen, und
        // niemand setzte `zustand.epoch`. Damit lief jede Prüfung, die
        // an der laufenden Epoche hängt, gegen 0 — der Verfall von
        // Credits ebenso wie das Fenster der Verstoßhistorie.
        //
        // Sie steht **vor** dem Anwenden, weil die Übergänge sie lesen.
        zustand.epoch = EpochId(epoch);
        let netz = crate::netz::Netz::des_zustands(zustand).startwert_hash();
        // ⚑ **Träge, nicht immer.** Die meisten Blöcke tragen kein
        // Bündel, und die Ableitung mischt das ganze Register.
        let mut zuteilung: Option<myl_scheduler::shard_assignment::Zuteilung> = None;
        // Je Transaktion, ob sie ihre Nummer verbraucht hat (Fund 627).
        let mut verbraucht = vec![false; txs.len()];
        // Die Bytes der verbrauchten Transaktionen, für den Grundpreis (G1).
        let mut bytes_im_block: u64 = 0;
        let gebuehren = crate::netz::Netz::des_zustands(zustand).gebuehren;
        // Nie unter dem Boden, auch nicht im ersten Block eines Zustands.
        zustand.grundpreis_je_byte = zustand.grundpreis_je_byte.max(gebuehren.boden_je_byte);
        for (i, tx) in txs.iter().enumerate() {
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

            // ⚑ **Die Gebühr** (2026-10-27, G1): Grundpreis mal Bytes,
            //   höchstens `max_gebuehr`. Wer sie nicht decken kann oder
            //   weniger zugesagt hat, wartet, ohne seine Nummer zu verbrauchen.
            let bytes = geprueft.inhalt().gebuehrenbytes();
            let gebuehr = zustand.grundpreis_je_byte.saturating_mul(bytes);
            if gebuehr > geprueft.inhalt().max_gebuehr || zustand.account(&absender).balance < gebuehr {
                continue;
            }

            // ⚑ **Die Nummer wird auch dann verbraucht, wenn die
            // Anweisung danach scheitert.** Sonst wäre eine ungedeckte
            // Überweisung unverändert gültig und beliebig oft
            // einreichbar.
            if nonce_verbrauchen(zustand, &absender, geprueft.inhalt().nonce).is_err() {
                continue;
            }
            verbraucht[i] = true;
            // Gezahlt wird auch, wenn die Anweisung danach scheitert: Die
            // Bytes stehen im Block.
            zustand.account_mut(&absender).balance -= gebuehr;
            zustand.gebuehren_epoche = zustand.gebuehren_epoche.saturating_add(gebuehr);
            bytes_im_block = bytes_im_block.saturating_add(bytes);

            // Gescheiterte Anweisungen werden übersprungen, nicht
            // abgebrochen: Eine Überweisung ohne Deckung ist kein Grund,
            // den Block zu verwerfen.
            match &geprueft.inhalt().anweisung {
                Anweisung::Burn { betrag } => {
                    // Verfall eine Epoche später: eine Festlegung dieser
                    // Testkette, damit beide Seiten dieselbe rechnen.
                    let _ = burn_to_credits(zustand, &absender, *betrag, EpochId(epoch + 1));
                }
                // ⚑ G1: An ein neues Konto zahlt der Absender einmal den
                //   Aufschlag an die Treasury; ein Konto bleibt für immer.
                Anweisung::Ueberweisung { nach, betrag } => {
                    let aufschlag = if zustand.accounts.contains_key(nach) { 0 } else { gebuehren.aufschlag_konto };
                    if zustand.account(&absender).balance >= betrag.saturating_add(aufschlag)
                        && transfer(zustand, &absender, nach, *betrag).is_ok()
                        && aufschlag > 0
                    {
                        Self::an_die_treasury(zustand, &absender, aufschlag);
                    }
                }
                // ⚑ G1: Eine Sitzung hinterlegt, zurück beim Aufräumen.
                Anweisung::SitzungEroeffnen { kontrakt } => {
                    let h = gebuehren.hinterlegung_sitzung;
                    if zustand.account(&absender).balance >= h
                        && sitzung_eroeffnen(zustand, &absender, kontrakt.clone()).is_ok()
                    {
                        Self::hinterlegen(zustand, &absender, HINTERLEGT_SITZUNG, *kontrakt.adresse().as_bytes(), h);
                    }
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
                    // ⚑ G1: Nur eine neue Anmeldung hinterlegt; wer seine
                    //   Angaben ändert, hat schon hinterlegt.
                    let neu = !zustand.miner.contains_key(&kennung);
                    let h = if neu { gebuehren.hinterlegung_anmeldung } else { 0 };
                    if zustand.account(&absender).balance >= h
                        && miner_anmelden(
                            zustand,
                            &absender,
                            &kennung,
                            *hardware,
                            *zone,
                            geprueft.inhalt().absender,
                            *netzadresse,
                        )
                        .is_ok()
                        && h > 0
                    {
                        Self::hinterlegen(zustand, &absender, HINTERLEGT_ANMELDUNG, *kennung.as_bytes(), h);
                    }
                }
                Anweisung::MinerAbmelden => {
                    let kennung = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if miner_abmelden(zustand, &absender, &kennung).is_ok() {
                        Self::zurueckgeben(zustand, HINTERLEGT_ANMELDUNG, *kennung.as_bytes());
                    }
                }
                // ⚑ **Die Kapazitätszusage** (Stufe 4 des Dispatchers,
                //   2026-10-21): je Netzmodell Ebenen und Ebenenzeit, ab
                //   einer künftigen Epoche.
                Anweisung::ZusageEinreichen { zusage } => {
                    let _ = myl_ledger::transitions::zusage_einreichen(zustand, &absender, zusage);
                }
                // ⚑ **Die Fristmeldung** (6.8, 2026-10-22): Pod, Mitglieder
                //   und Bereich des Gemeldeten leitet diese Stelle aus
                //   Zuteilung und Schnitt der laufenden Epoche ab, wie beim
                //   Bündel; der Ledger prüft Zeugnisse und Frist.
                Anweisung::FristverstossEinreichen { meldung } => {
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let Some(pod) = zuteilung.pods.iter().find(|p| p.pod_index == meldung.pod) else {
                        continue;
                    };
                    let Some(stelle) = pod.shards.iter().position(|s| s.miner.miner_id == meldung.gemeldeter) else {
                        continue;
                    };
                    let Some(grenzen) = Self::grenzen_des_pods(zustand, pod) else {
                        continue;
                    };
                    let kontext = myl_ledger::transitions::Fristkontext {
                        mitglieder: pod.mitglieder().map(|m| m.miner_id).collect(),
                        ebenen: grenzen[stelle].1 - grenzen[stelle].0,
                        mit_kopf: stelle + 1 == pod.shards.len(),
                    };
                    let _ = myl_ledger::transitions::fristverstoss_einreichen(zustand, &absender, meldung, &kontext);
                }
                // ⚑ **Der Einsatz** (Punkt B11, Fund 145). Bis zum
                // 2026-09-03 gab es keinen Weg, MYL zu hinterlegen;
                // `staked` war im Betrieb immer null, und die ganze
                // wirtschaftliche Sicherheitsschicht hing an einer
                // Zahl, die niemand setzte.
                Anweisung::EinsatzHinterlegen { betrag } => {
                    let _ = einsatz_hinterlegen(zustand, &absender, *betrag);
                }
                // ⚑ **Der Archivbeleg** (Stufe A5): Stichproben, Blöcke und
                //   Beweise prüft der Ledger gegen die Wurzel der Grenze.
                Anweisung::ArchivBelegen { bloecke, beweise } => {
                    let _ = myl_ledger::archiv::archiv_belegen(zustand, &absender, bloecke, beweise);
                }
                // ⚑ **Splitter** (Stufe A7): Kopien nennen die Teilwurzel,
                //   Splitter belegen ihre Teile dagegen.
                Anweisung::ArchivTeilwurzel { wurzel } => {
                    let _ = myl_ledger::archiv::teilwurzel_nennen(zustand, &absender, *wurzel);
                }
                Anweisung::ArchivTeilBelegen { stueck, beweis } => {
                    let _ = myl_ledger::archiv::teil_belegen(zustand, &absender, stueck, beweis);
                }
                Anweisung::EinsatzKuendigen { betrag } => {
                    let _ = einsatz_kuendigen(zustand, &absender, *betrag);
                }
                // ⚑ **Ein Doppelbeleg** (2026-10-27, CONSENSUS 7.6).
                Anweisung::DoppelbelegEinreichen { beleg } => {
                    let _ = Self::doppelbeleg_buchen(zustand, &absender, beleg);
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
                    let mitglieder = Self::unterzeichner_des_buendels(pod);
                    let _ = buendel_einreichen(
                        zustand,
                        &absender,
                        buendel.clone(),
                        &erster.miner.miner_id,
                        &mitglieder,
                    );
                }
                Anweisung::TrainingssegmentEinreichen { pod: kennung, segment } => {
                    // ⚑ **Dieselbe Arbeitsteilung wie beim Bündel**
                    // (Fund 144): Der Ledger kennt den Scheduler nicht
                    // und soll ihn nicht kennen; diese Stelle sieht
                    // beides und legt vor, was der Übergang prüft.
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let epoche_jetzt = zustand.epoch.0;
                    let Some(pod) = myl_scheduler::zonenzuteilung::pod_zu_kennung(
                        zuteilung,
                        epoche_jetzt,
                        kennung,
                    ) else {
                        continue;
                    };
                    let Some(erster) = pod.shards.first() else {
                        continue;
                    };
                    // ⚑ **Der Plan sagt, ob dieser Pod bestellt war und
                    // woran.** Ohne diese beiden Auskünfte könnte jeder
                    // Pod Arbeit einreichen, die er sich selbst
                    // ausgesucht hat, und die ganze Begründung der
                    // VRF-Zuweisung fiele.
                    //
                    // ⚑ **Der Plan wird hier neu gerechnet und nicht
                    // gespeichert.** Er ist eine reine Funktion des
                    // Zustands; ihn abzulegen führte eine zweite
                    // Wahrheit ein, die mit dem Zustand auseinanderlaufen
                    // könnte.
                    let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
                    let bestellt = plan.pods.contains(&pod.pod_index);
                    let charge = match (zustand.korpus.as_ref(), plan.zuweisungen.get(&pod.pod_index))
                    {
                        (Some(k), Some(z)) => k.charge(z.start, z.laenge),
                        // Ohne Korpus oder ohne Zuweisung gibt es keine
                        // Charge, gegen die zu prüfen wäre. Der Übergang
                        // lehnt dann über `bestellt` oder über die
                        // Charge ab; **eine erfundene Charge wäre
                        // schlimmer als keine.**
                        _ => Hash([0u8; 32]),
                    };
                    let mitglieder: Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> =
                        pod.mitglieder().map(|m| (m.miner_id, m.schluessel)).collect();
                    let vorgabe = myl_ledger::transitions::Segmentvorgabe {
                        charge,
                        modell_version: segment.modell_version,
                        lr_zaehler: Self::TRAININGS_LR_ZAEHLER,
                        // ⚑ **Die Rate folgt dem Budget DIESES Segments.**
                        // Schrittzahl und Folgen stehen darin und sind
                        // signiert; wer sie erhöht, ohne die Rate zu
                        // senken, fällt hier durch.
                        lr_nenner: Self::trainings_lr_nenner(
                            Self::trainingstiefe(zustand),
                            segment.gradienten(),
                        ),
                    };
                    let _ = myl_ledger::transitions::trainingssegment_einreichen(
                        zustand,
                        &absender,
                        *kennung,
                        segment.clone(),
                        bestellt,
                        &erster.miner.miner_id,
                        &mitglieder,
                        &vorgabe,
                    );
                }
                // ⚑ **Der Urteilsweg, und er hat seit heute einen
                // Aufrufer** (Fund 192). `apply_verdict` lag seit jeher
                // fertig da und wurde ausserhalb seiner Tests von nichts
                // erreicht; das Feld `Block::verdicts` daneben trug
                // einen Schuldspruch **ohne Beleg**.
                //
                // Hier kommt der Beleg mit der Anweisung, und der
                // Übergang leitet aus ihm ab, **wen** es trifft. Der
                // Blockerzeuger kann niemanden benennen, dessen
                // Unterschrift er nicht hat.
                Anweisung::SchuldspruchEinreichen {
                    segment,
                    beleg,
                    beschuldigt,
                    anzeigend,
                } => {
                    let _ = myl_ledger::transitions::schuldspruch_einreichen(
                        zustand,
                        *segment,
                        beleg,
                        beschuldigt,
                        anzeigend,
                        &Self::slashsatz(),
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
        zustand.grundpreis_je_byte = naechster_grundpreis(zustand.grundpreis_je_byte, bytes_im_block, gebuehren.boden_je_byte);
        verbraucht
    }

    /// ⚑ **Die Gebühren der Epoche werden geteilt** (2026-10-27, Festlegung
    /// des Projektinhabers): drei Dreizehntel an die Treasury, zehn in den
    /// Topf der Validatoren. Der Topf bleibt stehen, bis es eine
    /// Gewichtsquelle für Validatoren gibt; der Rest einer Teilung fällt
    /// dem Topf zu, damit keine Einheit verschwindet.
    fn gebuehren_teilen(zustand: &mut LedgerState) {
        let summe = std::mem::take(&mut zustand.gebuehren_epoche);
        let treasury = (summe as u128 * GEBUEHR_TREASURY as u128 / GEBUEHR_TEILE as u128) as u64;
        let t = zustand.account_mut(&myl_types::treasury::treasury_adresse());
        t.balance = t.balance.saturating_add(treasury);
        zustand.gebuehren_validatoren = zustand.gebuehren_validatoren.saturating_add(summe - treasury);
    }

    /// Bucht `betrag` vom Konto `von` an die Treasury (G1: Aufschlag für
    /// ein neues Konto). Gedeckt ist er, das prüft der Aufrufer.
    fn an_die_treasury(zustand: &mut LedgerState, von: &Address, betrag: u64) {
        zustand.account_mut(von).balance -= betrag;
        let t = zustand.account_mut(&myl_types::treasury::treasury_adresse());
        t.balance = t.balance.saturating_add(betrag);
    }

    /// Hinterlegt `betrag` vom Konto `von` unter (Art, Kennung) (G1).
    /// Gedeckt ist er, das prüft der Aufrufer.
    fn hinterlegen(zustand: &mut LedgerState, von: &Address, art: u8, kennung: [u8; 32], betrag: u64) {
        zustand.account_mut(von).balance -= betrag;
        let eintrag = zustand.hinterlegungen.entry((art, kennung)).or_insert((*von, 0));
        eintrag.1 = eintrag.1.saturating_add(betrag);
    }

    /// Gibt die Hinterlegung unter (Art, Kennung) an den zurück, der sie
    /// hinterlegt hat (G1). Ohne Eintrag geschieht nichts: Was vor den
    /// Gebühren angemeldet wurde, hat nichts hinterlegt.
    fn zurueckgeben(zustand: &mut LedgerState, art: u8, kennung: [u8; 32]) {
        if let Some((wer, betrag)) = zustand.hinterlegungen.remove(&(art, kennung)) {
            let konto = zustand.account_mut(&wer);
            konto.balance = konto.balance.saturating_add(betrag);
        }
    }

    /// Baut den nächsten Block aus dem Mempool und übernimmt ihn selbst.
    ///
    /// Der Erzeuger wendet **zuerst** an und schreibt die daraus
    /// entstandene Zustandswurzel in den Block. Die Empfänger rechnen
    /// dasselbe und vergleichen.
    pub fn baue_block(&mut self) -> Block {
        // ⚑ **Nur so viel, wie in einen Block passt** (Fund 633), in der
        //   Folge des Mempools. Was gerade nicht mehr passt, bleibt
        //   unangewandt liegen; was allein schon grösser ist als ein Block,
        //   kann nie hinein und fällt heraus, sonst stünde es für immer vorn.
        let mut wartend = Vec::new();
        let mut spaeter = Vec::new();
        let mut bytes = 0usize;
        let mut neulinge = 0usize;
        for (seit, tx) in std::mem::take(&mut self.mempool) {
            let n = borsh::to_vec(&tx).map_or(usize::MAX, |b| b.len());
            if n > BLOCK_TXS_HOECHSTENS_BYTES {
                continue;
            }
            // Neulinge nur bis zu ihrer Grenze je Block (Fund 635).
            let neuling = ist_neuling(&self.zustand, &tx.absender_adresse());
            if bytes + n <= BLOCK_TXS_HOECHSTENS_BYTES && !(neuling && neulinge >= BLOCK_NEULINGE_HOECHSTENS) {
                bytes += n;
                neulinge += usize::from(neuling);
                wartend.push((seit, tx));
            } else {
                spaeter.push((seit, tx));
            }
        }
        let txs: Vec<Transaktion> = wartend.iter().map(|(_, tx)| tx.clone()).collect();
        let hoehe = self.hoehe + 1;
        let epoch = epoche_fuer_hoehe(hoehe);

        // ⚑ **Das angebotene Zertifikat eines Prüfpunkts**, wenn es für
        //   diesen Block trägt (2026-10-27, CONSENSUS 7.2); geprüft vor dem
        //   Anwenden, das den bestätigten Stand fortschreibt.
        let zert = self.naechstes_pruefpunktzertifikat.take();
        let zert = zert.filter(|z| self.zertifikat_traegt(z, hoehe));
        let mut gezogen = None;
        // 📌 Fund 658: die Zuteilung der ablaufenden Epoche, bevor der
        //    Wechsel sie ersetzt. Gezogen wird erst mit dem Zertifikat, also
        //    bleibt sie bis dahin aufgehoben.
        if epoch > self.zustand.epoch.0 {
            let alte = self.zustand.epoch.0;
            self.offene_zuteilungen.push((alte, Self::zuteilung_der_laufenden_epoche(&self.zustand)));
            while self.offene_zuteilungen.len() > OFFENE_ZUTEILUNGEN_HOECHSTENS {
                self.offene_zuteilungen.remove(0);
            }
        }
        let verbraucht = Self::anwenden(
            &mut self.zustand,
            &txs,
            epoch,
            &self.letzter_hash,
            zert.as_ref(),
            &mut gezogen,
        );
        if let Some((e, proben)) = &gezogen {
            let mut pods: Vec<myl_scheduler::shard_assignment::Pod> = Vec::new();
            if let Some(stelle) = self.offene_zuteilungen.iter().position(|(alt, _)| alt == e) {
                let (_, z) = self.offene_zuteilungen.remove(stelle);
                for s in proben {
                    if let Some(p) = myl_scheduler::zonenzuteilung::pod_zu_kennung(&z, *e, &s.pod) {
                        if !pods.iter().any(|q| q.pod_index == p.pod_index) {
                            pods.push(p.clone());
                        }
                    }
                }
            }
            self.pods_der_stichprobe = pods;
        }
        if gezogen.is_some() {
            self.letzte_stichprobe = gezogen;
        }
        // ⚑ **In den Block kommt nur, was seine Nummer verbraucht hat**
        //   (Fund 627). Was an Unterschrift oder Nummer scheitert, hat den
        //   Zustand nicht angefasst; die Wurzel ist mit und ohne es dieselbe.
        //   Bis zum 2026-10-24 ging der ganze Mempool in den Block, und eine
        //   Transaktion mit der Nummer n+1, die im Gossip vor n ankam,
        //   scheiterte dort und war verloren: Der Absender zählte weiter und
        //   hing für immer an der Lücke. Jetzt wartet sie, bis n da ist.
        let mut im_block = Vec::new();
        for ((seit, tx), ok) in wartend.into_iter().zip(verbraucht) {
            if ok {
                im_block.push(tx);
            } else {
                self.mempool.push((seit, tx));
            }
        }
        self.mempool.extend(spaeter);
        self.mempool_bereinigen(hoehe);

        let mut block = Block::new(BlockHeader {
            height: hoehe,
            epoch,
            prev_block_hash: self.letzter_hash,
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: self.zustand.commitment(),
            pruefpunkt: zert,
        });
        for tx in im_block {
            block.add_transaction(tx);
        }
        // Unterschreiben, bevor der Block geschrieben und
        // verbreitet wird; die Unterschrift ändert den Blockhash nicht.
        if let Some(k) = &self.erzeugerschluessel {
            if let Ok(signatur) = k.signiere(&block.erzeugernachricht()) {
                block.unterschrift =
                    Some(myl_consensus::block::Erzeugerunterschrift { schluessel: k.oeffentlich(), signatur });
            }
        }
        self.erzeuger_lernen(&block);

        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
        self.hoehe = hoehe;
        self.letzter_hash = block.hash();
        self.bekannt.insert(self.letzter_hash);
        self.verlauf_aufnehmen(hoehe, &block);
        self.schreibe(&block);
        self.abzug_anlegen(&block);
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
        // ⚑ **Wer ihn erzeugt hat**, vor jeder Rechnung: Ein Block, den
        // niemand Berechtigtes unterschrieben hat, kostet keinen
        // Zustandsübergang.
        if !self.erzeuger_traegt(block) {
            return Err(KettenFehler::ErzeugerUnbelegt);
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
        // ⚑ **Das Zertifikat eines Prüfpunkts** (2026-10-27, CONSENSUS
        // 7.2), bevor gerechnet wird: Aus ihm kommen Endgültigkeit und Saat.
        // Erst nach Erzeuger und Anschluss, weil es die teurere Prüfung ist.
        if !self.pruefpunkt_traegt(block) {
            return Err(KettenFehler::PruefpunktTraegtNicht);
        }

        // ⚑ **Was im Block steht, muss geprüft werden oder darf nicht
        // drin stehen** (Fund 192). Beide Felder sind Teil des
        // gehashten Blocks und wurden bis zum 2026-09-06 von der Kette
        // weder geprüft noch angewandt.
        if !block.verdicts.is_empty() {
            return Err(KettenFehler::SchuldspruchOhneBeleg {
                anzahl: block.verdicts.len(),
            });
        }
        if block.validate_challenges().is_err() {
            return Err(KettenFehler::AnfechtungUnsinnig);
        }

        // Auf einer Kopie rechnen: Weicht das Ergebnis ab, bleibt der
        // eigene Zustand unberührt. Ein Knoten, der einem abweichenden
        // Block folgt, hätte den Befund verschluckt.
        let mut versuch = self.zustand.clone();
        let mut gezogen = None;
        // ⚑ **Das Zertifikat kommt aus dem Block, nicht aus eigenem
        // Zustand.** Sonst zöge jeder Knoten eine andere Stichprobe.
        Self::anwenden(
            &mut versuch,
            &block.txs,
            block.header.epoch,
            &self.letzter_hash,
            block.header.pruefpunkt.as_ref(),
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
        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
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
        //
        // Seit Fund 627 über die Nummern statt über die Bytes.
        self.mempool_bereinigen(block.header.height);
        self.schreibe(block);
        self.abzug_anlegen(block);
        self.erzeuger_lernen(block);
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

    fn zusage_direkt(k: &mut Kette, kennung: myl_types::ids::MinerId) {
        k.probezusage_setzen(kennung);
    }

    /// Die Zuteilung der laufenden Epoche, aber aus einem gegebenen Hash
    /// statt der Saat im Zustand: die Gegenprobe zu Fund 143.
    fn zuteilung_aus_hash(k: &Kette, hash: &Hash) -> myl_scheduler::shard_assignment::Zuteilung {
        let z = k.zustand();
        myl_scheduler::zonenzuteilung::zuteilung_der_epoche_mit_zusagen(
            &angemeldete_miner(z),
            z.epoch.0,
            hash,
            Kette::PROBE_SHARDS,
            &crate::netz::PROBEKETTE.podmodell,
            &|m| myl_ledger::transitions::modellzusage_der_epoche(z, m, z.epoch),
        )
    }

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

    /// Die Gebühr einer Transaktion, solange der Grundpreis am Boden der
    /// Probekette steht (G1); kleine Blöcke heben ihn nicht.
    fn gebuehr(tx: &Transaktion) -> u64 {
        crate::netz::PROBEKETTE.gebuehren.boden_je_byte * tx.gebuehrenbytes()
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
        // 📌 Seit der Historienwurzel bewegt jeder Block den Zustand (er
        // haengt den Hash seines Vorgaengers an). Verglichen wird deshalb
        // mit einem Zwilling, der einen leeren Block baut; beim ersten
        // Block ist der Vorgaenger in beiden derselbe Startblock.
        let mut leer = Kette::probestand();
        let _ = leer.baue_block();

        let mut tx = burn(0, 5_000);
        tx.signatur = myl_types::bls::BlsSignature([7u8; 96]);
        k.aufnehmen(tx);
        let _ = k.baue_block();
        assert_eq!(k.zustandswurzel(), leer.zustandswurzel(), "eine Fälschung darf nichts bewegen");

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
        let g = gebuehr(&tx);
        k.aufnehmen(tx);
        let _ = k.baue_block();

        assert_eq!(k.zustand().account(&von).balance, v0 - 2_500 - g);
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
            empfaenger_beweis: myl_types::sitzung::empfaengerbeweis(&[empfaenger], &empfaenger).expect("gelistet"),
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
        neu_unterschreiben(&mut block);
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
        neu_unterschreiben(&mut gefaelscht);
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

        // Der Erzeuger fälscht die Wurzel und unterschreibt.
        block.header.state_root = Hash::sha256(b"etwas anderes");
        neu_unterschreiben(&mut block);

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
        // ⚑ Mit Kettendatei: Seit der Deckel (512) über dem Fenster (256)
        //   liegt, haelt nur die Platte genug Bloecke, um ihn zu pruefen.
        let d = std::env::temp_dir().join(format!("myelith-deckel-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
        std::fs::create_dir_all(&d).expect("Verzeichnis");
        let mut k = Kette::probestand();
        let (speicher, _) = crate::speicher::Kettenspeicher::oeffnen(&d.join("kette.log"), Kette::startwert()).expect("öffnen");
        k.speicher_setzen(speicher);
        let ueber = crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG + 20;
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
        // Gegenprobe zum Deckel: Die Kette hält mehr, als er erlaubt, und
        // er wird ausgeschöpft (ab Höhe 1; die Höhe 0 ist der Startwert).
        let voll = k.bloecke_von_bis(1, u64::MAX);
        assert_eq!(voll.len() as u64, crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG, "der Deckel greift nicht oder es fehlt ein Block");
        let _ = std::fs::remove_dir_all(&d);
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
        neu_unterschreiben(&mut geliefert[0]);

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
    fn eine_doppelt_eingereichte_transaktion_faellt_mit_dem_block_ganz_heraus() {
        // 📌 Bis zum 2026-10-24 hiess dieser Test „verschwindet nur einmal":
        // Dieselbe Transaktion zweimal eingereicht sei zweimal eingereicht,
        // und die andere warte weiter. Sie trägt aber dieselbe Nummer, und
        // die ist mit dem Block verbraucht; die wartende hätte nie mehr
        // gegolten (Fund 627).
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
        assert_eq!(folger.wartend(), 0);
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

    /// ⚑ **Fund 627: Eine Transaktion, die ihre Vorgängerin im Gossip
    /// überholt, geht nicht verloren.** Sie wartet im Mempool, statt im
    /// Block an der Nummer zu scheitern, und kommt mit der Vorgängerin
    /// nach. Gegenprobe: Vorher stand n+1 im ersten Block, scheiterte dort
    /// und war weg; der zweite Block wandte nur n an, und die Nummer im
    /// Zustand blieb bei 1 statt 2.
    #[test]
    fn eine_ueberholende_transaktion_wartet_auf_ihre_vorgaengerin() {
        let mut k = Kette::probestand();
        k.aufnehmen(burn_nr(3, 100, 1));
        let erster = k.baue_block();
        assert!(erster.txs.is_empty(), "n+1 ohne n gehört nicht in den Block");
        assert_eq!(k.wartend(), 1, "n+1 wartet");

        k.aufnehmen(burn_nr(3, 100, 0));
        let _ = k.baue_block();
        let _ = k.baue_block();
        assert_eq!(k.zustand().account(&probekonto(3)).nonce, 2, "beide sind angewandt");
        assert_eq!(k.wartend(), 0);
    }

    /// ⚑ **Fund 627: In den Block kommt nur, was seine Nummer verbraucht**,
    /// und die Wurzel ist dieselbe wie mit allem. Ein Folger, der den
    /// schlanken Block übernimmt, landet beim selben Zustand.
    #[test]
    fn ein_block_traegt_nur_was_seine_nummer_verbraucht() {
        let mut k = Kette::probestand();
        let mut folger = Kette::probestand();
        let mut gefaelscht = burn(4, 100);
        gefaelscht.signatur = myl_types::bls::BlsSignature([7u8; 96]);
        k.aufnehmen(gefaelscht);
        k.aufnehmen(burn(5, 100));
        k.aufnehmen(burn_nr(6, 100, 5));
        let b = k.baue_block();
        assert_eq!(b.txs.len(), 1, "nur die gültige");
        folger.uebernimm(&b).expect("schließt an und rechnet gleich");
        assert_eq!(folger.zustandswurzel(), k.zustandswurzel());
    }

    /// ⚑ **Fund 627: Der Mempool eines Folgers bleibt klein.** Eine
    /// Transaktion, die nach ihrem Block ankommt, wird gar nicht erst
    /// aufgenommen; eine, die vorher ankam, fällt mit dem Block heraus;
    /// eine mit erfundener Nummer verfällt. Gegenprobe: Vorher nahm
    /// `aufnehmen` alles, und die späte blieb für immer liegen; im
    /// Archivlauf vom 2026-10-24 waren es über 5000.
    #[test]
    fn der_mempool_eines_folgers_bleibt_klein() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let frueh = burn(1, 100);
        let spaet = burn(2, 100);
        erzeuger.aufnehmen(frueh.clone());
        erzeuger.aufnehmen(spaet.clone());
        folger.aufnehmen(frueh);
        folger.aufnehmen(burn_nr(7, 100, 9));
        let b = erzeuger.baue_block();
        folger.uebernimm(&b).expect("schließt an");
        assert_eq!(folger.wartend(), 1, "die frühe ist mit dem Block weg, die erfundene wartet");
        assert!(!folger.aufnehmen(spaet), "die späte gilt nie mehr");

        for _ in 0..WARTEN_HOECHSTENS_HOEHEN {
            let b = erzeuger.baue_block();
            folger.uebernimm(&b).expect("schließt an");
        }
        assert_eq!(folger.wartend(), 0, "die erfundene ist verfallen");
    }

    /// ⚑ **Fund 627: Der Mempool hat eine Obergrenze.**
    #[test]
    fn der_mempool_nimmt_hoechstens_seine_grenze_auf() {
        let mut k = Kette::probestand();
        // Verteilt auf so viele Absender, dass keiner an seine eigene
        // Grenze stösst (Fund 633).
        for i in 0..MEMPOOL_HOECHSTENS as u64 {
            let wer = i / MEMPOOL_JE_ABSENDER as u64;
            let sk = myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("fremd-{wer}").as_bytes()).as_bytes())
                .expect("Schlüssel");
            let tx = Transaktion::signiere(&Kette::startwert(), &sk, i % MEMPOOL_JE_ABSENDER as u64, Anweisung::Burn { betrag: 1 })
                .expect("signieren");
            // Mit Guthaben, sonst greift die Grenze für Neulinge (Fund 635).
            k.zustand_mut().account_mut(&tx.absender_adresse()).balance = 1;
            assert!(k.aufnehmen(tx));
        }
        assert!(!k.aufnehmen(burn(8, 1)), "über der Grenze wird nichts aufgenommen");
        assert_eq!(k.wartend(), MEMPOOL_HOECHSTENS);
    }

    /// Ein Archivbeleg mit `bytes` Nutzlast, wie ihn jedes Konto einreichen
    /// kann: Er verbraucht seine Nummer und scheitert danach am Ledger.
    fn grosser_beleg(wer: u8, nonce: u64, bytes: usize) -> Transaktion {
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(wer),
            nonce,
            Anweisung::ArchivBelegen { bloecke: vec![vec![7u8; bytes]], beweise: vec![] },
        )
        .expect("signieren")
    }

    /// ⚑ **Ein Block bleibt unter seiner Bytegrenze** (Fund 633): Vierzig
    /// Belege zu je 50 KB sind 2 MB, mehr als Gossip an einem Block annimmt.
    /// Jeder Block trägt höchstens `BLOCK_TXS_HOECHSTENS_BYTES`, und alle
    /// kommen nacheinander hinein.
    #[test]
    fn ein_block_bleibt_unter_seiner_bytegrenze() {
        let mut k = Kette::probestand();
        for i in 0..40u64 {
            assert!(k.aufnehmen(grosser_beleg(30 + (i % 4) as u8, i / 4, 50_000)));
        }
        let mut im_block = 0usize;
        for _ in 0..10 {
            let b = k.baue_block();
            let txs: usize = b.txs.iter().map(|t| borsh::to_vec(t).unwrap().len()).sum();
            assert!(txs <= BLOCK_TXS_HOECHSTENS_BYTES, "{txs} Bytes in einem Block");
            assert!(borsh::to_vec(&b).unwrap().len() <= myl_net::validation::MAX_BLOCKS_BYTES);
            im_block += b.txs.len();
        }
        assert_eq!(im_block, 40, "alle kommen nacheinander hinein");
        assert_eq!(k.wartend(), 0);
    }

    /// Eine Transaktion, die allein grösser ist als ein Block, hält die
    /// übrigen nicht auf: Sie fällt heraus, die nächste kommt hinein.
    #[test]
    fn eine_uebergrosse_transaktion_haelt_den_block_nicht_auf() {
        let mut k = Kette::probestand();
        assert!(k.aufnehmen(grosser_beleg(30, 0, BLOCK_TXS_HOECHSTENS_BYTES + 1)));
        assert!(k.aufnehmen(burn(3, 900)));
        let b = k.baue_block();
        assert_eq!(b.txs.len(), 1, "der Burn steht im Block");
        assert_eq!(k.wartend(), 0, "die übergrosse ist gegangen");
    }

    /// ⚑ **Neulinge kommen nur gedrosselt hinein** (Fund 635): Hundert
    /// frische Schlüssel ohne Guthaben bringen höchstens
    /// `MEMPOOL_NEULINGE_HOECHSTENS` in den Mempool und höchstens
    /// `BLOCK_NEULINGE_HOECHSTENS` in einen Block; ein Konto mit Guthaben
    /// kommt weiter hinein. ⚑ Seit den Gebühren (G1) kommt ein Neuling ohne
    /// Guthaben in keinen Block: Er kann sie nicht zahlen und wartet, bis er
    /// verfällt.
    #[test]
    fn neulinge_kommen_nur_gedrosselt_hinein() {
        let mut k = Kette::probestand();
        let angenommen = (0..100u32)
            .filter(|i| {
                let sk = myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("neuling-{i}").as_bytes()).as_bytes())
                    .expect("Schlüssel");
                let tx = Transaktion::signiere(&Kette::startwert(), &sk, 0, Anweisung::Burn { betrag: 1 }).expect("signieren");
                k.aufnehmen(tx)
            })
            .count();
        assert_eq!(angenommen, MEMPOOL_NEULINGE_HOECHSTENS);
        assert!(k.aufnehmen(burn(3, 900)), "ein Konto mit Guthaben kommt hinein");
        let b = k.baue_block();
        assert_eq!(b.txs.len(), 1, "nur der Burn, kein Neuling zahlt");
        assert_eq!(k.wartend(), MEMPOOL_NEULINGE_HOECHSTENS);
    }

    /// Ein einzelner Absender füllt den Mempool nicht: höchstens
    /// `MEMPOOL_JE_ABSENDER`, und ein anderer kommt weiter hinein.
    #[test]
    fn ein_absender_fuellt_den_mempool_nicht() {
        let mut k = Kette::probestand();
        let angenommen = (0..200u64).filter(|&i| k.aufnehmen(burn_nr(8, 1, i))).count();
        assert_eq!(angenommen, MEMPOOL_JE_ABSENDER);
        assert!(k.aufnehmen(burn(3, 900)));
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

    /// ⚑ **Der Trainingsplan entsteht in der Kette und wird vermerkt.**
    ///
    /// Ohne diesen Test wäre der Eintrag im Epochenwechsel eine Zeile,
    /// die niemand erreicht, und `trainingsstand` bliebe leer: Die
    /// Reihum-Ordnung hätte nichts zu ordnen, und niemandem fiele es
    /// auf, weil die Auswahl dann still auf das blosse Los zurückfiele.
    ///
    /// Zwölf Miner ergeben zwei Pods. Ohne Nachfrage ist die Auslastung
    /// null, der Anteil 82 Prozent, und gerundet sind das **zwei**
    /// Trainingspods, also **ein Redundanzpaar**.
    ///
    /// ⚑ **Zwölf ist das Minimum eines Netzes, das trainieren kann.**
    /// Mit acht Konten gäbe es einen Pod, kein Paar und damit keine
    /// prüfbare Arbeit; genau daran fiel dieser Test am 2026-09-05, als
    /// die Paarbildung dazukam, und das war richtig so.
    #[test]
    fn der_trainingsplan_entsteht_und_wird_vermerkt() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;
        let mut k = Kette::probestand();
        let mut nonce = [0u64; 12];
        for w in 0..12u8 {
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        k.baue_block();
        assert_eq!(k.zustand().miner.len(), 12, "die Anmeldungen kamen nicht an");

        // Die Anmeldungen greifen erst nach dem Registrierungsschluss.
        for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }
        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        assert_eq!(zuteilung.pods.len(), 2, "zwoelf Miner muessen zwei Pods ergeben");

        let plan = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
        assert_eq!(plan.auslastung, 0, "ohne Nachfrage ist die Auslastung null");
        assert_eq!(plan.anteil_bps, 8_200, "der Anteil bei leerem Netz");
        assert_eq!(plan.pods.len(), 2, "82 Prozent von zwei, gerundet");
        assert_eq!(plan.paare.len(), 1, "zwei Trainingspods ergeben ein Paar");
        // ⚑ **Und er bekommt ein Buendel**, sonst waere der Plan eine
        // Zuteilung ohne Aufgabe.
        let nr = *plan.pods.iter().next().expect("ein Pod");
        let buendel = plan.zuweisungen.get(&nr).expect("kein Buendel zugewiesen");
        assert_eq!(buendel.laenge, 256);
        assert_eq!(buendel.start % 256, 0, "das Buendel liegt nicht auf dem Raster");
        assert!(buendel.start + buendel.laenge <= 65_536, "es ragt aus dem Korpus");

        // ⚑ **Der Vermerk entsteht am Epochenwechsel**, also sobald die
        // Anmeldungen greifen und ein Pod entsteht.
        let nach_eins = k.zustand().trainingsstand.clone();
        assert!(!nach_eins.is_empty(), "der Epochenwechsel hat nichts vermerkt");
        assert!(nach_eins.len() >= 6, "ein Trainingspod hat sechs Mitglieder");
        let epoche_jetzt = k.zustand().epoch.0;
        assert!(
            nach_eins.values().all(|e| *e < epoche_jetzt),
            "vermerkt wird die abgelaufene Epoche, nicht die laufende"
        );
        // ⚑ **Zwoelf Miner, zwei Trainingspods zu je sechs Plaetzen.**
        // Die Pods werden
        // jede Epoche neu gemischt, also kommen ueber mehrere Epochen
        // alle acht an die Reihe. Genau deshalb haengt das Gedaechtnis
        // am Miner und nicht am Pod.
        assert!(nach_eins.len() <= 12, "mehr Vermerke als Miner");

        // ⚑ **Und der Vermerk rückt mit der Epoche vor**, statt auf dem
        // ersten Stand stehen zu bleiben.
        for _ in 0..BLOECKE_JE_EPOCHE {
            k.baue_block();
        }
        let nach_zwei = &k.zustand().trainingsstand;
        let juengster_vorher = nach_eins.values().max().copied().unwrap_or(0);
        let juengster_nachher = nach_zwei.values().max().copied().unwrap_or(0);
        assert!(
            juengster_nachher > juengster_vorher,
            "der Vermerk blieb stehen: vorher {juengster_vorher}, nachher {juengster_nachher}"
        );
        assert!(nach_zwei.len() >= nach_eins.len(), "der Stand hat Miner verloren");
    }

    /// ⚑ **Nur ein Paar, das sich einig ist, bewegt das Modell.**
    ///
    /// Vier Fälle, und drei davon dürfen nichts bewegen: ein Paar, von
    /// dem nur einer geliefert hat; ein Paar mit verschiedenen
    /// Ergebnissen; ein Paar, das gar nicht denselben Auftrag hatte.
    #[test]
    fn nur_ein_einiges_paar_bestaetigt_ein_delta() {
        use myl_types::trainingssegment::Trainingssegment;

        let mut z = Kette::probestand().zustand().clone();
        let e = 7u64;
        let seg = |charge: u8, commitment: u8| Trainingssegment {
            id: myl_types::ids::SegmentId::new([1u8; 32]),
            modell_version: myl_types::ids::MerkleRoot::new([2u8; 32]),
            charge: Hash([charge; 32]),
            startschritt: 0,
            schrittzahl: 30,
            folgen: 1,
            lr_zaehler: 1,
            lr_nenner: 1 << 12,
            delta_commitment: Hash([commitment; 32]),
            bewegte_gewichte: 1,
            pod_pfad: vec![myl_types::ids::MinerId::new([3u8; 32])],
            signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
        };
        let setzen = |z: &mut LedgerState, pod: u32, s: Trainingssegment| {
            z.trainingssegmente.insert(myl_types::miner::pod_kennung(e, pod), s);
        };

        // Paar (0, 1): einig.
        setzen(&mut z, 0, seg(9, 42));
        setzen(&mut z, 1, seg(9, 42));
        // Paar (2, 3): uneinig.
        setzen(&mut z, 2, seg(9, 42));
        setzen(&mut z, 3, seg(9, 43));
        // Paar (4, 5): nur einer hat geliefert.
        setzen(&mut z, 4, seg(9, 42));
        // Paar (6, 7): dasselbe Ergebnis, aber verschiedene Charge.
        setzen(&mut z, 6, seg(9, 42));
        setzen(&mut z, 7, seg(8, 42));

        let paare = [(0u32, 1u32), (2, 3), (4, 5), (6, 7)];
        let bestaetigt = Kette::bestaetigte_deltas(&z, &paare, e);
        assert_eq!(bestaetigt, vec![Hash([42; 32])], "es zaehlte mehr als das einige Paar");

        // ⚑ Und ohne einiges Paar bleibt die Fassung stehen.
        let leer = Kette::bestaetigte_deltas(&z, &[(2, 3), (4, 5)], e);
        assert!(leer.is_empty());
        let alt = myl_types::ids::MerkleRoot::new([5u8; 32]);
        assert_eq!(myl_types::modellversion::naechste_version(&alt, &leer), alt);
        assert_ne!(myl_types::modellversion::naechste_version(&alt, &bestaetigt), alt);
    }

    /// ⚑ **Die Ordnung haengt nicht an der Paarreihenfolge.**
    #[test]
    fn die_bestaetigten_deltas_stehen_kanonisch() {
        use myl_types::trainingssegment::Trainingssegment;
        let mut z = Kette::probestand().zustand().clone();
        let e = 3u64;
        let seg = |c: u8| Trainingssegment {
            id: myl_types::ids::SegmentId::new([1u8; 32]),
            modell_version: myl_types::ids::MerkleRoot::new([2u8; 32]),
            charge: Hash([9; 32]),
            startschritt: 0,
            schrittzahl: 30,
            folgen: 1,
            lr_zaehler: 1,
            lr_nenner: 1 << 12,
            delta_commitment: Hash([c; 32]),
            bewegte_gewichte: 1,
            pod_pfad: vec![myl_types::ids::MinerId::new([3u8; 32])],
            signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
        };
        for (pod, c) in [(0u32, 9u8), (1, 9), (2, 1), (3, 1)] {
            z.trainingssegmente.insert(myl_types::miner::pod_kennung(e, pod), seg(c));
        }
        let vorwaerts = Kette::bestaetigte_deltas(&z, &[(0, 1), (2, 3)], e);
        let rueckwaerts = Kette::bestaetigte_deltas(&z, &[(2, 3), (0, 1)], e);
        assert_eq!(vorwaerts, rueckwaerts, "die Paarreihenfolge wirkt auf die Ordnung");
        assert_eq!(vorwaerts, vec![Hash([1; 32]), Hash([9; 32])]);
    }

    /// ⚑ **Der Weg vom Trainingsplan bis in den Zustand, ganz.**
    ///
    /// Ein bestellter Pod reicht sein Segment ein und es steht danach im
    /// Zustand; ein Segment mit fremder Charge wird abgewiesen. Ohne
    /// diesen Test wäre `TrainingssegmentEinreichen` eine Variante, die
    /// niemand geht, und `Trainingssegment` bliebe die Arbeitsklasse
    /// ohne Weg, die sie bis heute war.
    #[test]
    fn ein_bestellter_pod_reicht_sein_trainingssegment_ein() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;
        use myl_types::trainingssegment::Trainingssegment;

        let mut k = Kette::probestand();
        let mut nonce = [0u64; 12];
        for w in 0..12u8 {
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        k.baue_block();
        for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }

        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        let plan = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
        let nr = *plan.pods.iter().next().expect("ein Trainingspod");
        let pod = zuteilung.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
        let kennung = myl_types::miner::pod_kennung(k.zustand().epoch.0, nr);
        let zuweisung = plan.zuweisungen[&nr];
        let charge =
            k.zustand().korpus.as_ref().expect("Korpus").charge(zuweisung.start, zuweisung.laenge);

        // Welcher Probeschlüssel gehört zum Koordinator?
        let koordinator = pod.shards[0].miner.miner_id;
        let w = (0..12u8)
            .find(|w| {
                myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == koordinator
            })
            .expect("der Koordinator ist ein Probekonto");

        let bauen = |charge: myl_types::hash::Hash| {
            let mut seg = Trainingssegment {
                id: myl_types::ids::SegmentId::new([3u8; 32]),
                modell_version: myl_types::ids::MerkleRoot::new([4u8; 32]),
                charge,
                startschritt: 0,
                schrittzahl: 30,
                folgen: 1,
                lr_zaehler: Kette::TRAININGS_LR_ZAEHLER,
                lr_nenner: Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), 30),
                delta_commitment: myl_types::hash::Hash([5u8; 32]),
                bewegte_gewichte: 1,
                pod_pfad: pod.mitglieder().map(|m| m.miner_id).collect(),
                signaturen: Vec::new(),
            };
            // ⚑ **Eine Unterschrift je Mitglied**, in der Reihenfolge
            // des Pfades. Der Übergang fasst sie zusammen und prüft in
            // einem Pairing.
            let botschaft = seg.botschaft();
            seg.signaturen = pod
                .mitglieder()
                .map(|m| {
                    let nr = (0..12u8)
                        .find(|w| {
                            myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == m.miner_id
                        })
                        .expect("Mitglied ist ein Probekonto");
                    probeschluessel(nr).sign(&botschaft).expect("signieren")
                })
                .collect();
            seg
        };

        // ⚑ **Erst die Fälschung**, damit der Erfolg danach nicht bloss
        // beweist, dass irgendetwas durchgeht.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::TrainingssegmentEinreichen {
                    pod: kennung,
                    segment: bauen(myl_types::hash::Hash([99u8; 32])),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.baue_block();
        assert!(
            k.zustand().trainingssegmente.is_empty(),
            "ein Segment mit fremder Charge kam durch"
        );

        // Und jetzt das richtige.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::TrainingssegmentEinreichen {
                    pod: kennung,
                    segment: bauen(charge),
                },
            )
            .expect("signieren"),
        );
        k.baue_block();
        let abgelegt = k
            .zustand()
            .trainingssegmente
            .get(&kennung)
            .expect("das Segment steht nicht im Zustand");
        assert_eq!(abgelegt.charge, charge);
        assert_eq!(
            abgelegt.lr_nenner,
            Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), abgelegt.gradienten())
        );
    }

    /// ⚑ **Der ganze Trainingsweg: Paar reicht ein, Kette bestätigt,
    /// Modell rückt, Miner wird bezahlt.**
    ///
    /// Ohne diesen Test wären die letzten drei Glieder Behauptungen: Ein
    /// Segment käme in den Zustand, und ob daraus je eine
    /// Modellfassung oder eine Gutschrift wird, sähe niemand.
    #[test]
    fn ein_einiges_paar_bewegt_das_modell_und_wird_bezahlt() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;
        use myl_types::trainingssegment::Trainingssegment;

        let mut k = Kette::probestand();
        let mut nonce = [0u64; 12];
        for w in 0..12u8 {
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        // Auszahlungskonten, sonst gilt „ohne Eintrag kein Anteil".
        for w in 0..12u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::AuszahlungskontoEintragen {
                        kennung: myl_types::ids::MinerId::new(*probekonto(w).as_bytes()),
                        konto: probekonto(w),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        // Brennstoff, damit es überhaupt eine Prägung gibt.
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }

        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        let plan = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
        assert_eq!(plan.paare.len(), 1, "zwoelf Miner ergeben ein Trainingspaar");
        let (a, b) = plan.paare[0];
        let zuweisung = plan.zuweisungen[&a];
        assert_eq!(zuweisung, plan.zuweisungen[&b], "das Paar hat verschiedene Buendel");
        let charge = k
            .zustand()
            .korpus
            .as_ref()
            .expect("Korpus")
            .charge(zuweisung.start, zuweisung.laenge);
        let epoche = k.zustand().epoch.0;
        let vorher = k.zustand().modell_version;

        let schluessel_von = |id: myl_types::ids::MinerId| -> u8 {
            (0..12u8)
                .find(|w| myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == id)
                .expect("Mitglied ist ein Probekonto")
        };

        // ⚑ **Beide Pods des Paars reichen dasselbe Ergebnis ein.**
        for nr in [a, b] {
            let pod = zuteilung.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
            let mut seg = Trainingssegment {
                id: myl_types::ids::SegmentId::new([3u8; 32]),
                modell_version: myl_types::ids::MerkleRoot::new([4u8; 32]),
                charge,
                startschritt: 0,
                schrittzahl: 30,
                folgen: 1,
                lr_zaehler: Kette::TRAININGS_LR_ZAEHLER,
                lr_nenner: Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), 30),
                delta_commitment: Hash([5u8; 32]),
                bewegte_gewichte: 1,
                pod_pfad: pod.mitglieder().map(|m| m.miner_id).collect(),
                signaturen: Vec::new(),
            };
            let botschaft = seg.botschaft();
            seg.signaturen = pod
                .mitglieder()
                .map(|m| probeschluessel(schluessel_von(m.miner_id)).sign(&botschaft).expect("sig"))
                .collect();
            let w = schluessel_von(pod.shards[0].miner.miner_id);
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::TrainingssegmentEinreichen {
                        pod: myl_types::miner::pod_kennung(epoche, nr),
                        segment: seg,
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.baue_block();
        assert_eq!(
            k.zustand().trainingssegmente.len(),
            2,
            "beide Segmente muessen im Zustand stehen"
        );

        // Ein Mitglied des Paars, dessen Konto wir beobachten.
        let beobachtet = {
            let pod = zuteilung.pods.iter().find(|p| p.pod_index == a).expect("Pod");
            probekonto(schluessel_von(pod.shards[1].miner.miner_id))
        };
        let vor_geld = k.zustand().account(&beobachtet).balance;

        // Über die Epochengrenze.
        for _ in 0..BLOECKE_JE_EPOCHE {
            k.baue_block();
        }

        assert_ne!(
            k.zustand().modell_version,
            vorher,
            "ein einiges Paar hat die Modellfassung nicht bewegt"
        );
        assert!(
            k.zustand().trainingssegmente.is_empty(),
            "die Segmente wurden nicht geraeumt"
        );
        assert!(
            k.zustand().account(&beobachtet).balance > vor_geld,
            "der Trainingsminer wurde nicht bezahlt"
        );
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

    // --- G1: Gebühren, Aufschlag, Hinterlegung ---

    fn mit_grenze(wer: u8, nonce: u64, max_gebuehr: u64, a: Anweisung) -> Transaktion {
        Transaktion::signiere_mit_grenze(&Kette::startwert(), &probeschluessel(wer), nonce, max_gebuehr, a)
            .expect("signieren")
    }

    /// ⚑ **Wer weniger zusagt, als die Gebühr kostet, wartet** und behält
    /// seine Nummer; mit genau der Gebühr kommt er hinein, und sie landet im
    /// Topf der Epoche.
    #[test]
    fn wer_die_gebuehr_nicht_zusagt_wartet() {
        let mut k = Kette::probestand();
        let a = || Anweisung::Burn { betrag: 10 };
        let g = gebuehr(&mit_grenze(3, 0, 0, a()));
        k.aufnehmen(mit_grenze(3, 0, g - 1, a()));
        let b = k.baue_block();
        assert!(b.txs.is_empty(), "eine Einheit zu wenig zugesagt");
        assert_eq!(k.zustand().account(&probekonto(3)).nonce, 0, "die Nummer ist verbraucht");
        assert_eq!(k.zustand().gebuehren_epoche, 0);

        // Gegenprobe: genau die Gebühr.
        let vorher = k.zustand().account(&probekonto(3)).balance;
        k.aufnehmen(mit_grenze(3, 0, g, a()));
        assert_eq!(k.baue_block().txs.len(), 1);
        assert_eq!(k.zustand().account(&probekonto(3)).balance, vorher - 10 - g);
        assert_eq!(k.zustand().gebuehren_epoche, g);
    }

    /// ⚑ **Eine gescheiterte Anweisung zahlt trotzdem**: Ihre Bytes stehen
    /// im Block. Wer die Gebühr nicht decken kann, kommt nicht hinein.
    #[test]
    fn eine_gescheiterte_anweisung_zahlt_die_gebuehr() {
        let mut k = Kette::probestand();
        let vorher = k.zustand().account(&probekonto(3)).balance;
        let tx = burn(3, u64::MAX);
        let g = gebuehr(&tx);
        k.aufnehmen(tx);
        assert_eq!(k.baue_block().txs.len(), 1, "die Nummer ist verbraucht");
        assert_eq!(k.zustand().account(&probekonto(3)).balance, vorher - g, "nur die Gebühr");

        // Gegenprobe: ein Konto mit weniger als der Gebühr.
        k.zustand_mut().account_mut(&probekonto(4)).balance = g - 1;
        k.aufnehmen(burn(4, 1));
        assert!(k.baue_block().txs.is_empty(), "ungedeckt im Block");
        assert_eq!(k.zustand().account(&probekonto(4)).balance, g - 1);
    }

    /// ⚑ **Ein neues Konto kostet einmal den Aufschlag**, an die Treasury;
    /// die zweite Überweisung an dasselbe Konto nicht.
    #[test]
    fn ein_neues_konto_kostet_einmal_den_aufschlag() {
        let mut k = Kette::probestand();
        let aufschlag = crate::netz::PROBEKETTE.gebuehren.aufschlag_konto;
        let treasury = myl_types::treasury::treasury_adresse();
        let neu = Address::new(*Hash::sha256(b"ein-neues-konto").as_bytes());
        assert!(!k.zustand().accounts.contains_key(&neu));
        let (v0, t0) = (k.zustand().account(&probekonto(3)).balance, k.zustand().account(&treasury).balance);

        let tx = mit_grenze(3, 0, u64::MAX, Anweisung::Ueberweisung { nach: neu, betrag: 100 });
        let g1 = gebuehr(&tx);
        k.aufnehmen(tx);
        k.baue_block();
        assert_eq!(k.zustand().account(&neu).balance, 100);
        assert_eq!(k.zustand().account(&probekonto(3)).balance, v0 - 100 - g1 - aufschlag);
        assert_eq!(k.zustand().account(&treasury).balance, t0 + aufschlag);

        // Gegenprobe: dasselbe Konto ein zweites Mal.
        let tx = mit_grenze(3, 1, u64::MAX, Anweisung::Ueberweisung { nach: neu, betrag: 100 });
        let g2 = gebuehr(&tx);
        k.aufnehmen(tx);
        k.baue_block();
        assert_eq!(k.zustand().account(&probekonto(3)).balance, v0 - 200 - g1 - g2 - aufschlag);
        assert_eq!(k.zustand().account(&treasury).balance, t0 + aufschlag);
    }

    /// ⚑ **Die Anmeldung hinterlegt, die Abmeldung gibt zurück**; eine
    /// zweite Anmeldung desselben Miners hinterlegt nicht noch einmal.
    #[test]
    fn die_anmeldung_hinterlegt_und_die_abmeldung_gibt_zurueck() {
        use myl_types::miner::HardwareClass;
        let mut k = Kette::probestand();
        let h = crate::netz::PROBEKETTE.gebuehren.hinterlegung_anmeldung;
        let kennung = *probekonto(0).as_bytes();
        let v0 = k.zustand().account(&probekonto(0)).balance;
        let mut gezahlt = 0;
        let mut senden = |k: &mut Kette, tx: Transaktion| {
            gezahlt += gebuehr(&tx);
            k.aufnehmen(tx);
            k.baue_block();
            gezahlt
        };

        let g = senden(&mut k, anmeldung(0, HardwareClass::MediumGpu));
        assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g - h);
        assert_eq!(k.zustand().hinterlegungen.get(&(HINTERLEGT_ANMELDUNG, kennung)), Some(&(probekonto(0), h)));

        // Gegenprobe: dieselbe Anmeldung mit anderen Angaben hinterlegt nicht.
        let neu = Anweisung::MinerAnmelden {
            hardware: HardwareClass::LargeGpu,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        };
        let g = senden(&mut k, mit_grenze(0, 1, u64::MAX, neu));
        assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g - h);

        let g = senden(&mut k, mit_grenze(0, 2, u64::MAX, Anweisung::MinerAbmelden));
        assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g, "die Hinterlegung kam nicht zurück");
        assert!(k.zustand().hinterlegungen.is_empty());
    }

    /// ⚑ **Eine Sitzung hinterlegt, und wenn sie aufgeräumt wird, kommt
    /// die Hinterlegung zurück** (Fund 662: Vorher räumte die Kette nie auf).
    /// Gegenprobe: vor dem Ende der Nachfrist bleibt beides.
    #[test]
    fn eine_abgelaufene_sitzung_gibt_ihre_hinterlegung_zurueck() {
        use myl_types::sitzung::{Grenzen, Sitzungskontrakt};
        let mut k = Kette::probestand();
        let h = crate::netz::PROBEKETTE.gebuehren.hinterlegung_sitzung;
        let kontrakt = Sitzungskontrakt::neu(
            probekonto(6),
            probekonto(7),
            Grenzen::gesperrt(),
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(0),
            EpochId(0),
            16,
        )
        .expect("gueltiger Kontrakt");
        let id = kontrakt.adresse();
        let v0 = k.zustand().account(&probekonto(6)).balance;
        let tx = mit_grenze(6, 0, u64::MAX, Anweisung::SitzungEroeffnen { kontrakt });
        let g = gebuehr(&tx);
        k.aufnehmen(tx);
        k.baue_block();
        assert!(k.zustand().sitzung(&id).is_some());
        assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g - h);

        let nachfrist = myl_ledger::state::SITZUNG_NACHFRIST;
        let z = k.zustand_mut();
        z.epoch = EpochId(nachfrist);
        Kette::epochenwechsel_abschliessen(z, nachfrist + 1, &Hash([0; 32]));
        assert!(k.zustand().sitzung(&id).is_some(), "vor dem Ende der Nachfrist aufgeräumt");
        assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g - h);

        let z = k.zustand_mut();
        z.epoch = EpochId(nachfrist + 1);
        Kette::epochenwechsel_abschliessen(z, nachfrist + 2, &Hash([0; 32]));
        assert!(k.zustand().sitzung(&id).is_none(), "die abgelaufene Sitzung blieb");
        assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g, "die Hinterlegung kam nicht zurück");
    }

    /// ⚑ **Der Grundpreis wandert um höchstens ein Achtel**: am Ziel bleibt
    /// er, ein voller Block hebt ihn um ein Achtel, ein leerer senkt ihn um
    /// eins; bei eins steigt er dennoch um eins, und unter den Boden fällt
    /// er nie.
    #[test]
    fn der_grundpreis_wandert_um_hoechstens_ein_achtel() {
        let ziel = GRUNDPREIS_ZIEL_BYTES;
        assert_eq!(naechster_grundpreis(800, ziel, 1), 800);
        assert_eq!(naechster_grundpreis(800, 2 * ziel, 1), 900);
        assert_eq!(naechster_grundpreis(800, 0, 1), 700);
        assert_eq!(naechster_grundpreis(1, ziel + 1, 1), 2, "bei eins bliebe er sonst für immer");
        assert_eq!(naechster_grundpreis(1, 0, 1), 1, "unter den Boden");
        assert_eq!(naechster_grundpreis(800, 0, 750), 750, "unter den Boden");
        assert_eq!(naechster_grundpreis(u64::MAX, 2 * ziel, 1), u64::MAX);
    }

    /// ⚑ **Die Gebühren einer Epoche teilen sich drei zu zehn**: Treasury
    /// und Topf der Validatoren; der Rest der Teilung geht in den Topf, und
    /// keine Einheit verschwindet.
    #[test]
    fn die_gebuehren_teilen_sich_drei_zu_zehn() {
        let mut z = Kette::probestand().zustand().clone();
        let treasury = myl_types::treasury::treasury_adresse();
        let t0 = z.account(&treasury).balance;
        z.gebuehren_epoche = 1_301;
        Kette::gebuehren_teilen(&mut z);
        assert_eq!(z.account(&treasury).balance - t0, 300);
        assert_eq!(z.gebuehren_validatoren, 1_001);
        assert_eq!(z.gebuehren_epoche, 0);
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        k.baue_block();
        assert_eq!(k.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");
        // ⚑ Die Zusagen gelten ab der nächsten Epoche; erst dort gibt es
        //   einen Pod (kein Pod ohne Zusage).
        while k.zustand().epoch.0 < 1 {
            k.baue_block();
        }

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
        // Die Gebühren laufen mit (G1).
        let mut gezahlt = 0;
        let mut senden = |k: &mut Kette, nonce: u64, anweisung: Anweisung| {
            let tx = Transaktion::signiere(&Kette::startwert(), &probeschluessel(0), nonce, anweisung).expect("signieren");
            gezahlt += gebuehr(&tx);
            k.aufnehmen(tx);
            k.baue_block();
            gezahlt
        };

        let g = senden(&mut k, 0, Anweisung::EinsatzHinterlegen { betrag: 5_000 });
        assert_eq!(
            k.zustand().account(&probekonto(0)).staked,
            5_000,
            "der Einsatz kam nicht an"
        );
        assert_eq!(k.zustand().account(&probekonto(0)).balance, vorher - 5_000 - g);

        // Kuendigen: aus `staked` heraus, aber noch nicht ins Guthaben.
        let g = senden(&mut k, 1, Anweisung::EinsatzKuendigen { betrag: 2_000 });
        let konto = k.zustand().account(&probekonto(0));
        assert_eq!(konto.staked, 3_000);
        assert_eq!(konto.balance, vorher - 5_000 - g, "das Geld kam zu frueh zurueck");
        assert_eq!(konto.gekuendigt.values().sum::<u64>(), 2_000);

        // Abholen vor der Freigabe bewirkt nichts.
        let g = senden(&mut k, 2, Anweisung::EinsatzAbholen);
        assert_eq!(
            k.zustand().account(&probekonto(0)).balance,
            vorher - 5_000 - g,
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
            empfaenger_wurzel: myl_types::sitzung::empfaengerwurzel(&[probekonto(2)]),
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
        let start = k.zustand().epoch.0;
        for _ in 0..BLOECKE_JE_EPOCHE * 2 {
            k.baue_block();
            if k.zustand().epoch.0 == start + 1 {
                break;
            }
        }
        assert_eq!(k.zustand().epoch.0, start + 1, "die Epoche wechselte nicht");
        assert!(k.zustand().buendel.is_empty(), "die Buendel blieben stehen");
    }

    /// Unterschreibt ein Bündel mit den **Positionen** seines Pods.
    ///
    /// ⚑ **Nur die Positionen, seit Fund 645** (2026-10-26): Die Reserve
    /// rechnet nicht und kann nicht prüfen, was sie unterschriebe. Geprüft
    /// wird gegen genau diese Menge ([`Kette::unterzeichner_des_buendels`]).
    fn buendel_unterschreiben(
        buendel: &mut myl_types::PoIBundle,
        pod: &myl_scheduler::shard_assignment::Pod,
    ) {
        let msg = myl_consensus::poi::bundle_message(buendel);
        let mut teile = Vec::new();
        for m in pod.shards.iter().map(|s| &s.miner) {
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

    /// Das Zertifikat des Prüfpunkts an `hoehe` über den eigenen Block,
    /// unterschrieben vom Probeerzeuger, dem einzigen Mitglied einer
    /// Probekette.
    fn pruefpunkt_von(k: &Kette, hoehe: u64, wer: &str) -> myl_consensus::pruefpunkt::Pruefpunktzertifikat {
        let schluessel = crate::schluessel::Konsensschluessel::probe(wer).expect("Schlüssel");
        let block_hash = k.hash_an(hoehe).expect("der Block liegt im Fenster");
        let signatur = schluessel
            .signiere(&myl_consensus::pruefpunkt::pruefpunktnachricht(hoehe, &block_hash))
            .expect("unterschreiben");
        let stimme = myl_consensus::pruefpunkt::Pruefpunktstimme { hoehe, block_hash, waehler: schluessel.kennung(), signatur };
        myl_consensus::pruefpunkt::Pruefpunktzertifikat::aus_stimmen(&[stimme]).expect("Zertifikat")
    }

    /// Baut `n` Blöcke auf `k` und übernimmt sie in `folger`.
    fn bauen_und_folgen(k: &mut Kette, folger: &mut Kette, n: usize) {
        for _ in 0..n {
            let b = k.baue_block();
            folger.uebernimm(&b).expect("ein gewöhnlicher Block");
        }
    }

    /// ⚑ **Ein Prüfpunkt wird bestätigt** (2026-10-27, CONSENSUS 7.2): Der
    /// Erzeuger legt das Zertifikat in den nächsten Block, der Folger nimmt
    /// es an, und beide stehen danach bei Höhe 150. Gegenproben: dasselbe
    /// Zertifikat ein zweites Mal, keine Prüfpunkthöhe, ein anderer Block,
    /// ein Fremder als Unterzeichner.
    #[test]
    fn ein_pruefpunkt_wird_bestaetigt() {
        let mut k = Kette::probestand();
        let mut folger = Kette::probestand();
        bauen_und_folgen(&mut k, &mut folger, 150);
        let z = pruefpunkt_von(&k, 150, PROBEERZEUGER);
        k.pruefpunkt_anbieten(z.clone());
        let mit = k.baue_block();
        assert_eq!(mit.header.pruefpunkt.as_ref(), Some(&z), "der Erzeuger legte das Zertifikat nicht in den Block");
        folger.uebernimm(&mit).expect("ein Block mit tragendem Zertifikat");
        assert_eq!((k.bestaetigt_bis(), folger.bestaetigt_bis()), (150, 150));

        // ⛔️ Dasselbe Zertifikat noch einmal: nichts Neues, abgelehnt.
        let mut nochmal = k.baue_block();
        nochmal.header.pruefpunkt = Some(z);
        neu_unterschreiben(&mut nochmal);
        assert_eq!(folger.uebernimm(&nochmal), Err(KettenFehler::PruefpunktTraegtNicht), "zweimal dasselbe");

        // Je Gegenprobe ein frisches Paar bei Höhe 150 und ein Block 151
        // mit einem veränderten Zertifikat.
        let pruefe = |aendern: &dyn Fn(&Kette) -> myl_consensus::pruefpunkt::Pruefpunktzertifikat| {
            let mut erz = Kette::probestand();
            let mut neu = Kette::probestand();
            bauen_und_folgen(&mut erz, &mut neu, 150);
            let zert = aendern(&erz);
            let mut b = erz.baue_block();
            b.header.pruefpunkt = Some(zert);
            neu_unterschreiben(&mut b);
            neu.uebernimm(&b)
        };
        // ⛔️ Ein richtiges Zertifikat, nachträglich in einen fertigen Block
        // gesetzt: Es schreibt den bestätigten Stand fort, die Wurzel im Kopf
        // weiss davon nichts.
        assert!(matches!(pruefe(&|e| pruefpunkt_von(e, 150, PROBEERZEUGER)), Err(KettenFehler::ZustandWeichtAb { .. })));
        // ⛔️ Ein anderer Block an Höhe 150.
        let anderer = |e: &Kette| {
            let mut x = pruefpunkt_von(e, 150, PROBEERZEUGER);
            x.block_hash = Hash::sha256(b"ein anderer Block");
            x
        };
        assert_eq!(pruefe(&anderer), Err(KettenFehler::PruefpunktTraegtNicht), "anderer Block");
        // ⛔️ Ein Fremder unterschreibt.
        assert_eq!(pruefe(&|e| pruefpunkt_von(e, 150, "fremder")), Err(KettenFehler::PruefpunktTraegtNicht), "Fremder");
        // ⛔️ Keine Prüfpunkthöhe.
        let schief = |e: &Kette| {
            let mut x = pruefpunkt_von(e, 150, PROBEERZEUGER);
            x.hoehe = 149;
            x
        };
        assert_eq!(pruefe(&schief), Err(KettenFehler::PruefpunktTraegtNicht), "keine Prüfpunkthöhe");
    }

    /// ⚑ **Die Saat kommt aus dem Zertifikat** (2026-10-27): Zwei
    /// Zertifikate mit verschiedenen Unterzeichnern ziehen verschieden, ein
    /// und dasselbe zieht gleich, und jeder Knoten liest es aus dem Block.
    #[test]
    fn die_saat_kommt_aus_dem_zertifikat() {
        let anteile = vec![(myl_types::ids::PodId::new([5; 32]), 1_000u32)];
        let mut k = Kette::probestand();
        for _ in 0..150 {
            k.baue_block();
        }
        let a = pruefpunkt_von(&k, 150, PROBEERZEUGER);
        let b = pruefpunkt_von(&k, 150, "fremder");
        let zieh = |z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat| {
            let saat = crate::stichprobe::saat_aus_zertifikat(&z.aggregat, &z.unterzeichner, 0);
            crate::stichprobe::stichprobe_aus_anteilen(&anteile, &saat, Kette::STICHPROBE_BP)
        };
        // ⚑ Aus der Konstante gerechnet (Fund 171).
        let erwartet = (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000);
        assert_eq!(zieh(&a).len(), erwartet);
        assert_ne!(zieh(&a), zieh(&b), "das Zertifikat muss die Ziehung ändern");
        assert_eq!(zieh(&a), zieh(&a.clone()), "dasselbe Zertifikat, dieselbe Ziehung");
    }

    /// Unterschreibt einen in einem Test veränderten Block neu, mit dem
    /// Probeerzeuger: Der berechtigte Erzeuger hat einen falschen Block
    /// gebaut, und die eigentliche Prüfung muss ihn fangen, nicht erst die
    /// Unterschrift.
    fn neu_unterschreiben(block: &mut Block) {
        let k = crate::schluessel::Konsensschluessel::probe(PROBEERZEUGER).expect("Schlüssel");
        let signatur = k.signiere(&block.erzeugernachricht()).expect("unterschreiben");
        block.unterschrift = Some(myl_consensus::block::Erzeugerunterschrift { schluessel: k.oeffentlich(), signatur });
    }

    /// ⚑ **Jeder Block braucht die Unterschrift eines Mitglieds** (Stimmsatz
    /// ab Genesis, 2026-10-27). Mit Stimmsatz nur ein Mitglied unter seinem
    /// Schlüssel; ohne Satz lernt eine Probekette den Erzeuger aus Block 1
    /// und verlangt ihn danach; ein anderes Netz nimmt ohne Satz nichts.
    #[test]
    fn nur_ein_belegter_erzeuger_kommt_durch() {
        use myl_consensus::validator::{VotingMember, VotingSet};
        let mitglied = crate::schluessel::Konsensschluessel::probe("erzeuger").expect("Schlüssel");
        let fremder = crate::schluessel::Konsensschluessel::probe("fremder").expect("Schlüssel");
        let satz = || {
            let mut m = std::collections::BTreeMap::new();
            m.insert(mitglied.kennung(), VotingMember { pubkey: mitglied.oeffentlich(), weight: 1 });
            VotingSet::from_members(m)
        };
        let erzeugt_von = |k: Option<&crate::schluessel::Konsensschluessel>| {
            let mut e = Kette::im_netz(&crate::netz::PROBEKETTE);
            if let Some(k) = k {
                e.erzeugerschluessel_setzen(k.clone());
            }
            e.baue_block()
        };
        let mit_satz = || {
            let mut l = Kette::probestand();
            l.stimmsatz_setzen(satz());
            l
        };

        // Das Mitglied: angenommen.
        let echt = erzeugt_von(Some(&mitglied));
        assert_eq!(echt.erzeuger(), Ok(Some(mitglied.kennung())));
        mit_satz().uebernimm(&echt).expect("ein Block des Mitglieds wurde abgewiesen");
        // ⛔️ Gegenproben mit Stimmsatz: unsigniert, von einem Fremden, und
        // die Unterschrift des Mitglieds unter einem anderen Schlüssel.
        assert_eq!(mit_satz().uebernimm(&erzeugt_von(None)), Err(KettenFehler::ErzeugerUnbelegt));
        assert_eq!(mit_satz().uebernimm(&erzeugt_von(Some(&fremder))), Err(KettenFehler::ErzeugerUnbelegt));
        let mut untergeschoben = echt.clone();
        if let Some(u) = untergeschoben.unterschrift.as_mut() {
            u.schluessel = fremder.oeffentlich();
        }
        assert_eq!(mit_satz().uebernimm(&untergeschoben), Err(KettenFehler::ErzeugerUnbelegt));

        // ⛔️ Ohne Satz auf der Probekette: unsigniert nie, und eine
        // Unterschrift, die nicht trägt, nie.
        assert_eq!(Kette::probestand().uebernimm(&erzeugt_von(None)), Err(KettenFehler::ErzeugerUnbelegt));
        assert_eq!(Kette::probestand().uebernimm(&untergeschoben), Err(KettenFehler::ErzeugerUnbelegt));
        // Der Erzeuger von Block 1 wird das einzige Mitglied; danach kommt
        // ein anderer nicht mehr durch.
        let mut e = Kette::im_netz(&crate::netz::PROBEKETTE);
        e.erzeugerschluessel_setzen(fremder.clone());
        let erster = e.baue_block();
        e.erzeugerschluessel_setzen(mitglied.clone());
        let zweiter = e.baue_block();
        let mut lerner = Kette::probestand();
        assert!(lerner.stimmsatz().is_none());
        lerner.uebernimm(&erster).expect("Block 1 auf der Probekette");
        assert_eq!(lerner.stimmsatz().and_then(|s| s.pubkey(&fremder.kennung())), Some(&fremder.oeffentlich()));
        assert_eq!(lerner.uebernimm(&zweiter), Err(KettenFehler::ErzeugerUnbelegt), "ein zweiter Erzeuger kam durch");

        // ⛔️ Ein Netz, das nicht lernt, nimmt ohne Satz nichts an.
        let mut t = Kette::im_netz(&crate::netz::TESTNETZ_INT4);
        t.erzeugerschluessel_setzen(mitglied.clone());
        let testblock = t.baue_block();
        assert_eq!(Kette::im_netz(&crate::netz::TESTNETZ_INT4).uebernimm(&testblock), Err(KettenFehler::ErzeugerUnbelegt));
    }

    /// ⚑ **Ein Erzeuger, der doppelt unterschreibt, wird geschlachtet**
    /// (CONSENSUS 7.6): 30 % seines Einsatzes beim ersten Mal, das Kopfgeld
    /// an den Melder. Gegenproben: derselbe Widerspruch noch einmal, ein
    /// Täter ohne Einsatz, eine Tat vor der Sperrfrist, wer sich selbst
    /// meldet.
    #[test]
    fn eine_doppelunterschrift_kostet_den_einsatz() {
        use myl_consensus::double_signing::{Belegblock, Doppelbeleg};
        let block = |zeit: u64, wer: u8| {
            let mut b = Block::new(BlockHeader {
                height: 5,
                epoch: 0,
                prev_block_hash: Hash::sha256(b"vorher"),
                timestamp_ms: zeit,
                state_root: Hash::sha256(b"wurzel"),
                pruefpunkt: None,
            });
            b.unterschreiben(&probeschluessel(wer)).expect("unterschreiben");
            Belegblock::aus_block(&b).expect("unterschrieben")
        };
        let beleg = |wer: u8| Doppelbeleg::Erzeuger(Box::new(block(1, wer)), Box::new(block(2, wer)));
        let taeter = Address::aus_schluessel(&probeschluessel(1).public_key().expect("pk"));
        let melder = probekonto(2);
        let mut z = Kette::probestand().zustand().clone();
        z.account_mut(&taeter).balance = 10_000;
        einsatz_hinterlegen(&mut z, &taeter, 10_000).expect("Einsatz");
        let vorher_melder = z.account(&melder).balance;

        Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(1)).expect("ein tragender Beleg");
        assert_eq!(z.account(&taeter).staked, 7_000, "30 % beim ersten Mal");
        assert!(z.account(&melder).balance > vorher_melder, "der Melder bekam kein Kopfgeld");

        // ⛔️ Derselbe Widerspruch noch einmal.
        assert!(Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(1)).is_err());
        assert_eq!(z.account(&taeter).staked, 7_000);
        // ⛔️ Ein Täter ohne Einsatz: nichts zu schlachten.
        assert!(Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(3)).is_err());
        // ⛔️ Wer sich selbst meldet.
        let mut z2 = Kette::probestand().zustand().clone();
        z2.account_mut(&taeter).balance = 10_000;
        einsatz_hinterlegen(&mut z2, &taeter, 10_000).expect("Einsatz");
        assert!(Kette::doppelbeleg_buchen(&mut z2, &taeter, &beleg(1)).is_err());
        // ⛔️ Eine Tat vor der Sperrfrist.
        z2.epoch = EpochId(myl_ledger::einsatz::SPERRFRIST_EPOCHEN + 1);
        assert!(Kette::doppelbeleg_buchen(&mut z2, &melder, &beleg(1)).is_err());
        assert_eq!(z2.account(&taeter).staked, 10_000);
    }

    /// ⚑ **Die Vertrauensfrist** (CONSENSUS 7.5): bis 112 Epochen über dem
    /// bestätigten Prüfpunkt trägt sie, eine darüber nicht; ein Netz, das
    /// hinter dem eigenen Stand liegt, ist kein Grund zum Anhalten.
    #[test]
    fn die_vertrauensfrist_traegt_bis_112_epochen() {
        assert!(vertrauen_traegt(10, 10 + 112));
        assert!(!vertrauen_traegt(10, 10 + 113), "eine Epoche zu viel");
        assert!(vertrauen_traegt(50, 3));
        assert!(vertrauen_traegt(0, 0));
    }

    /// Ohne Zertifikat wird ein Block angenommen: Nicht jeder Block trägt
    /// eines, nur der, in dem es gerade beisammen ist.
    #[test]
    fn ohne_zertifikat_wird_ein_block_angenommen() {
        let mut k = Kette::probestand();
        let block = k.baue_block();
        assert!(block.header.pruefpunkt.is_none());
        let mut leser = Kette::probestand();
        assert!(leser.uebernimm(&block).is_ok());
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
        // es zwölf (`PROBEKONTEN`), und `probeschluessel` rechnet
        // modulo: Achtzehn Aufrufe ergäben achtzehn Anmeldungen unter
        // zwölf Kennungen, also zwölf Miner. Der Test bräuchte dann drei
        // Pods und bekäme zwei.
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
            zusage_direkt(&mut k, kennung);
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
            zuteilung_aus_hash(&k, &k.letzter_hash())
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        // Und Brennstoff für die Prägung.
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        }
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
        // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
        //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
        //   Abschluss nach.
        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
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
        // ⚑ **Gezogen wird erst mit dem Zertifikat** (2026-10-27, CONSENSUS
        //   7.2): Bis dahin hebt der Zustand die Anteile auf. ⛔️ Ohne
        //   Zertifikat keine Ziehung, und kein Rückfall auf den Blockhash.
        assert!(k.letzte_stichprobe().is_empty(), "gezogen ohne Zertifikat");
        assert_eq!(k.zustand().stichproben_offen.len(), 1);
        let z = pruefpunkt_von(&k, BLOECKE_JE_EPOCHE, PROBEERZEUGER);
        k.pruefpunkt_anbieten(z);
        k.baue_block();
        assert!(k.zustand().stichproben_offen.is_empty(), "die Ziehung wartet noch");
        // ⚑ **Fund 114: Und die Stichprobe ist gezogen.**
        //
        // `sample_segments` und `check_segment` waren seit dem
        // 2026-08-17 gebaut, geprueft und abgehakt, und **hatten null
        // Aufrufer**. Damit war `p` aus Anhang B.1 im Betrieb null, und
        // `S_min = g/p^2` keine Schranke mehr.
        //
        // 📌 **Geprueft wird die Ziehung, nicht das Nachrechnen.** Das
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
        // 📌 Fund 658: Der Pod jeder Ziehung ist nach dem Wechsel noch zu
        //    finden (die Probekonten hier tragen keine Netzadresse; die
        //    Adressen prüft `stichprobe.rs`).
        let pod = k.pod_der_kennung(0, &kennung).expect("Fund 658: der Pod der abgelaufenen Epoche fehlt");
        assert_eq!(pod.pod_index, pod_index);
        assert!(stichprobe.iter().all(|x| k.pod_der_kennung(0, &x.pod).is_some()));
        // ⛔️ Eine andere Epoche als die der Ziehung findet nichts.
        assert!(k.pod_der_kennung(5, &kennung).is_none());

        // ⚑ Die Probeprägung (2026-10-26) bekommt jedes eingetragene Konto
        //   obendrauf; geprüft wird, was die echte Ausschüttung zahlte.
        let probe = crate::netz::PROBEKETTE.probepraegung;
        assert!(
            (0..6).all(|i| nachher[i] >= vorher[i] + probe),
            "ein Konto bekam die Probeprägung nicht: {vorher:?} -> {nachher:?}"
        );
        let gewachsen = (0..6).filter(|i| nachher[*i] > vorher[*i] + probe).count();
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        }
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
        // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
        //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
        //   Abschluss nach.
        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
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
            // Nur die Probeprägung, nichts aus der Ausschüttung.
            assert_eq!(
                k.zustand().account(&kaltes_konto(w)).balance,
                crate::netz::PROBEKETTE.probepraegung,
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
    /// 📌 Ohne diesen Test war die Kennungssuche in der Kette ungeprüft:
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
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
        nonce[0] += 1;
        k.baue_block();
        for w in 0..6u8 {
            zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        }
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
            // Nur die Probeprägung, nichts aus der Ausschüttung.
            assert_eq!(
                k.zustand().account(&kaltes_konto(w)).balance,
                crate::netz::PROBEKETTE.probepraegung,
                "ein erfundenes Buendel hat ausgezahlt"
            );
        }
    }

    fn kaltes_konto(w: u8) -> Address {
        Address::new([200 + w; 32])
    }

    /// ⚑ **Die Probeprägung** (2026-10-26): Am Epochenwechsel bekommt das
    /// Auszahlungskonto jedes angemeldeten Miners den Betrag der
    /// Probekette, ein gesperrter in der Epoche seiner Sperre nichts, einer
    /// ohne Eintrag nichts. Ohne Brennstoff und ohne Bündel, also nichts aus
    /// der echten Ausschüttung.
    #[test]
    fn die_probekette_praegt_je_epoche_ins_auszahlungskonto() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        use myl_types::miner::HardwareClass;
        use myl_types::node_metadata::GeoRegion;

        let mut k = Kette::probestand();
        let kennung = |w: u8| myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        let mut nonce = [0u64; 4];
        for w in 0..4u8 {
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
        k.baue_block();
        // Drei tragen ein Konto ein, der vierte nicht.
        for w in 0..3u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::AuszahlungskontoEintragen { kennung: kennung(w), konto: kaltes_konto(w) },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.baue_block();
        assert_eq!(k.zustand().auszahlung.len(), 3, "die Auszahlungskonten kamen nicht an");
        // Der dritte ist in der laufenden Epoche gesperrt.
        k.zustand_mut().fristsperren.insert(kennung(2), EpochId(0));
        let vorher: Vec<u64> = (0..4u8)
            .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
            .chain([k.zustand().account(&probekonto(3)).balance])
            .collect();
        while k.zustand().epoch.0 == 0 {
            k.baue_block();
        }
        assert!(k.hoehe() <= BLOECKE_JE_EPOCHE + 2, "die Epoche wechselte spät");
        let nachher: Vec<u64> = (0..4u8)
            .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
            .chain([k.zustand().account(&probekonto(3)).balance])
            .collect();
        let betrag = crate::netz::PROBEKETTE.probepraegung;
        assert_eq!(nachher[0], vorher[0] + betrag, "das erste Konto bekam nicht den Betrag");
        assert_eq!(nachher[1], vorher[1] + betrag, "das zweite Konto bekam nicht den Betrag");
        assert_eq!(nachher[2], vorher[2], "der Gesperrte bekam etwas");
        assert_eq!(nachher[3], vorher[3], "ein Konto ohne Eintrag bekam etwas");
        assert_eq!(nachher[4], vorher[4], "der Miner ohne Eintrag bekam etwas");

        // ⛔️ Gegenprobe: derselbe Zustand im Testnetz prägt nichts.
        let mut z = k.zustand().clone();
        z.podmodell = Some(crate::netz::TESTNETZ_INT4.podmodell);
        let vor: Vec<u64> = (0..3u8).map(|w| z.account(&kaltes_konto(w)).balance).collect();
        Kette::probepraegung(&mut z, 1);
        let nach: Vec<u64> = (0..3u8).map(|w| z.account(&kaltes_konto(w)).balance).collect();
        assert_eq!(vor, nach, "das Testnetz prägte zur Probe");
    }

    /// ⚑ **Fund 167: das Auszahlungskonto hat einen Weg über die Kette,
    /// und die Berechtigungsregel gilt auf ihm.**
    ///
    /// Drei Lagen in einem Test, weil sie zusammen die Regel ergeben:
    /// Die **erste** Eintragung darf der Miner selbst, jede **weitere**
    /// nur das eingetragene kalte Konto, und ein **Fremder** nie.
    ///
    /// 📌 Ohne diesen Test wäre die neue `Anweisung` eine Variante, die
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

    /// ⚑ **Die Kapazitätszusage wirkt über die Kette** (Stufe 4,
    /// 2026-10-21): angemeldet, dann eingereicht, steht sie im Zustand; eine
    /// mit fremder Unterschrift nicht.
    #[test]
    fn eine_zusage_ueber_die_kette_wirkt() {
        use myl_types::zusage::{Kapazitaetszusage, Modellzusage};
        let mut k = Kette::probestand();
        k.aufnehmen(anmeldung(0, myl_types::miner::HardwareClass::SmallGpu));
        k.baue_block();
        let sk = probeschluessel(0);
        let halter = myl_types::ids::MinerId::aus_schluessel(&sk.public_key().expect("pk"));
        let ab = EpochId(k.zustand().epoch.0 + 1);
        let abdruck = crate::netz::PROBEKETTE.podmodell.abdruck;
        let modell = Modellzusage { abdruck, ebenen_hoechstens: 30, ebenenzeit_us: 900, gehalten: None };
        let mut gut = Kapazitaetszusage::neu(halter, ab, 0).mit_modell(modell);
        gut.signiere(&sk).expect("signieren");
        let mut fremd = Kapazitaetszusage::neu(halter, EpochId(ab.0 + 1), 0).mit_modell(modell);
        fremd.signiere(&probeschluessel(1)).expect("signieren");
        // ⚑ Ein Modell, das dieses Netz nicht rechnet (6.6).
        let mut anderes = Kapazitaetszusage::neu(halter, EpochId(ab.0 + 2), 0)
            .mit_modell(Modellzusage { abdruck: [7; 32], ..modell });
        anderes.signiere(&sk).expect("signieren");
        for (nonce, z) in [(1, gut), (2, fremd), (3, anderes)] {
            let tx = Transaktion::signiere(&Kette::startwert(), &sk, nonce, Anweisung::ZusageEinreichen { zusage: z })
                .expect("signieren");
            k.aufnehmen(tx);
        }
        k.baue_block();
        let liste = k.zustand().zusagen.get(&halter).expect("eingereicht");
        assert_eq!(liste.iter().map(|z| z.ab_epoche).collect::<Vec<_>>(), [ab], "nur die eigene Unterschrift und das Podmodell gelten");
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
