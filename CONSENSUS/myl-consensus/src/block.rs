//! Blockstruktur — Whitepaper Kap. 3.5, Anhang A.5.
//!
//! Ein Block enthält Transaktionen, PoI-Bündel, Challenges und
//! Verdicts (Anhang A.5) sowie die Epochen-Metadaten, die ihn in die
//! Kette einhängen.
//!
//! ## Kanonische Typen statt Dubletten (Fund A8)
//!
//! Bis v0.4.0 definierte diese Datei **eigene** Fassungen von
//! `PoiBundle`, `Challenge` und `Verdict` — mit anderen Feldern als die
//! Typen, die die übrigen Komponenten tatsächlich produzieren:
//!
//! | hier (alt) | kanonisch |
//! |---|---|
//! | `PoiBundle { segment_id, commitment_hash, pod_id: [u8;32], signature: [u8;96] }` | `myl_types::PoIBundle { epoch, pod, segments_root, vtfe_claimed, aggregate_sig }` |
//! | `Challenge { segment_id, first_divergence, challenger, accused }` | `myl_types::Challenge` (mit beiden Pods **und** beiden Hashes) |
//! | `Verdict { segment_id, winner, loser, slash_amount }` | `myl_ledger::Verdict { segment_id, miner, checker, outcome }` |
//!
//! Die Folge war eine stille Integrationslücke: `myl-pod` erzeugt
//! `myl_types::PoIBundle` (das Epochen-Aggregat aus Anhang A.1), aber
//! `Block::add_poi_bundle` nahm eine per-Segment-Struktur — der Pfad
//! Pod → Block war nicht verdrahtet, obwohl beide Seiten als
//! „vollständig" geführt wurden. Ebenso hätte kein `Verdict` des
//! Verifiers je in den Ledger gebucht werden können.
//!
//! Diese Datei definiert deshalb keine Protokolltypen mehr, sondern
//! verwendet die kanonischen. Rohe `[u8; 32]`/`[u8; 96]`-Felder sind
//! durch die Newtypes aus `myl-types` ersetzt — genau dafür gibt es
//! SHARED_TYPES.
//!
//! **Konsens-Feld:** Die Blockkodierung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_ledger::transitions::Verdict;
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::miner::HardwareClass;
use myl_types::node_metadata::GeoRegion;
use myl_types::ids::SitzungId;
use myl_types::sitzung::{Sitzungskontrakt, Vorhaben};
use myl_types::challenge::Challenge;
use myl_types::core_types::PoIBundle;
use myl_types::hash::Hash;
use myl_types::ids::{Address, MinerId};

/// Wie viele Blöcke auf eine Epoche gehen.
///
/// **Abgeleitet, nicht gesetzt:** Epochenlänge geteilt durch Blockzeit,
/// also 3600 s / 2 s. Beide sind Governance-Parameter
/// (`myl_governance::Parameter::{Epochenlaenge, Blockzeit}`), und ein
/// Test dort hält diese Zahl gegen sie.
///
/// ⚑ **Der Wert steht hier als Konstante und nicht als Abfrage der
/// Registry**, und das ist Absicht: Die Zuordnung Höhe → Epoche geht in
/// die Blockprüfung ein, und eine Blockprüfung, die einen abstimmbaren
/// Wert liest, macht die Gültigkeit eines Blocks von einem Zustand
/// abhängig, der sich ändern kann, während der Block schon in der Kette
/// steht. Wer die Epochenlänge ändern will, ändert damit einen
/// Konsensvertrag und nicht einen Parameter.
pub const BLOECKE_JE_EPOCHE: u64 = 1_800;

/// Die Epoche, zu der eine Blockhöhe gehört.
///
/// **Die Epoche folgt aus der Höhe, nicht aus der Uhr.** Eine
/// Zuordnung über Zeitstempel wäre nicht deterministisch: Zwei ehrliche
/// Knoten mit leicht verschiedenen Uhren ordneten denselben Block
/// verschiedenen Epochen zu, und damit fiele die Zustandswurzel
/// auseinander. Die Höhe ist die einzige Größe, über die sich alle
/// einig sind, bevor sie sich einig sein müssen.
///
/// **Was das kostet, gehört dazugesagt:** Stehen die Blöcke still,
/// stehen auch die Epochen still. Prägung, EMA und Fristen hängen dann
/// am Fortschritt der Kette und nicht an der Wanduhr. Für ein
/// Protokoll, dessen Arbeit ohnehin in Blöcken abgerechnet wird, ist das
/// die richtige Richtung; für eine Frist, die in Sekunden gedacht ist,
/// ist es eine Näherung, die man kennen muss.
pub fn epoche_fuer_hoehe(hoehe: u64) -> u64 {
    hoehe / BLOECKE_JE_EPOCHE
}

/// Der Kopf eines Blocks.
///
/// ⚑ **Hieß bis zum 2026-08-27 `BlockHeader`, und der Name war der
/// Fehler.** Er trug kein Höhenfeld, und die Probekette hat deshalb die
/// Höhe in `epoch` geschrieben — eine Doppelbelegung, die trägt, solange
/// eine Epoche ein Block ist, und bricht, sobald es das nicht mehr ist.
/// Ein Kopf, der die Höhe führt, heißt nicht nach der Epoche.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlockHeader {
    /// Blockhöhe: die Stellung in der Kette, um genau eins wachsend.
    ///
    /// **Der Genesis-Block hat Höhe 0.** Die Höhe ist die Größe, an der
    /// ein Nachzügler seine Lücke abzählt, und sie ist die einzige, die
    /// das darf: Sie wächst um eins je Block, ohne Ausnahme.
    pub height: u64,
    /// Epochennummer.
    ///
    /// **Folgt aus der Höhe** ([`epoche_fuer_hoehe`]) und wird beim
    /// Übernehmen dagegen geprüft. Sie steht trotzdem im Kopf, damit ein
    /// Block für sich lesbar bleibt — wer ihn aus dem Speicher holt,
    /// soll nicht erst die Umrechnung kennen müssen. Ein mitgeführter
    /// Wert, der geprüft wird, ist keine zweite Wahrheit, sondern eine
    /// Prüfsumme; einer, der es nicht wird, ist eine Einladung.
    pub epoch: u64,
    /// Vorheriger Block-Hash.
    pub prev_block_hash: Hash,
    /// Zeitstempel (Unix-Millisekunden).
    pub timestamp_ms: u64,
    /// Commitment über den Ledger-Zustand **nach** Anwendung dieses
    /// Blocks (`myl_ledger::LedgerState::commitment()`).
    ///
    /// Ohne dieses Feld kann ein Validator nur prüfen, ob die Bytes des
    /// Blocks gleich sind — nicht, ob der Vorschlagende die
    /// Zustandsübergänge korrekt angewendet hat. Ein Leader könnte einen
    /// syntaktisch einwandfreien Block mit falsch gebuchtem Slashing
    /// vorschlagen, und das Komitee hätte nichts, woran es den Fehler
    /// festmachen könnte.
    pub state_root: Hash,
    /// ⚑ **Das Zertifikat eines Prüfpunkts** (Festlegung des
    /// Projektinhabers vom 2026-10-27, ersetzt `saatquelle`): der Beleg,
    /// dass mehr als zwei Drittel des Stimmsatzes den Block an einer Höhe
    /// `150 · n` bestätigt haben ([`crate::pruefpunkt`]). Ein späterer Block
    /// trägt es, sobald es beisammen ist.
    ///
    /// **Aus ihm kommt die Saat der Stichprobe**: Das Zertifikat des
    /// Epochenwechsels entsteht erst, nachdem die Bündel der Epoche
    /// feststehen, also kann es niemand beim Einreichen kennen. Der Rückfall
    /// auf den Blockhash (Fund 120, unbegrenzter Mahlraum) entfällt; hängt
    /// die Abstimmung, wartet die Ziehung auf das nächste Zertifikat.
    ///
    /// 📌 Hier stand bis zum 2026-10-27 `saatquelle: Option<Vec<u8>>`, rohe
    /// Bytes eines Commitzertifikats, die den Vorgänger bestätigten: ein
    /// Feld, das erst gelesen werden musste, um zu wissen, ob es überhaupt
    /// ein Zertifikat war. Ein typisiertes Feld kennt diesen Fall nicht.
    pub pruefpunkt: Option<crate::pruefpunkt::Pruefpunktzertifikat>,
}

/// Trennzeichen für die Transaktionsunterschrift.
pub const DST_TRANSAKTION: &[u8] = b"MYELITH_TRANSAKTION_v1";

/// Trennzeichen für die Unterschrift des Erzeugers unter einem Block.
pub const DST_BLOCK_ERZEUGER: &[u8] = b"MYELITH_BLOCK_ERZEUGER_v1";

/// Die Botschaft des Erzeugers zu einem Blockhash: `DST_BLOCK_ERZEUGER ‖ Hash`.
pub fn erzeugernachricht_zum_hash(hash: &Hash) -> Vec<u8> {
    let mut m = DST_BLOCK_ERZEUGER.to_vec();
    m.extend_from_slice(hash.as_bytes());
    m
}

/// **Die Unterschrift des Erzeugers** (2026-10-27).
///
/// # ⚑ Warum der Schlüssel mitkommt und keine Kennung
///
/// Eine Kennung ist der Hash eines Schlüssels, und aus einem Hash folgt
/// kein Schlüssel. Ein Übernehmer ohne Stimmsatz könnte eine Unterschrift
/// unter einer blossen Kennung nie prüfen; mit dem Schlüssel kann er es
/// immer, und mit Stimmsatz prüft er zusätzlich, dass es **der** Schlüssel
/// dieses Mitglieds ist. Die Kennung folgt daraus
/// ([`Erzeugerunterschrift::erzeuger`]), ein zweites Feld dafür liesse sich
/// abweichend füllen.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Erzeugerunterschrift {
    /// Der öffentliche Schlüssel des Erzeugers.
    pub schluessel: BlsPublicKey,
    /// Die Unterschrift über [`Block::erzeugernachricht`].
    pub signatur: BlsSignature,
}

impl Erzeugerunterschrift {
    /// Die Kennung des Erzeugers: der Hash seines Schlüssels.
    pub fn erzeuger(&self) -> MinerId {
        MinerId::aus_schluessel(&self.schluessel)
    }
}

/// Eine Unterschrift unter einem Block, die nicht trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnterschriftTraegtNicht;

/// Was eine Transaktion anweist.
///
/// ⚑ **Kein Absenderfeld.** Wer die Anweisung erteilt, steht in
/// [`Transaktion::absender`] und wird aus dem Schlüssel abgeleitet, mit
/// dem unterschrieben wurde. Ein zusätzliches Feld „Absender" ließe sich
/// abweichend füllen, und dann unterschriebe A für B.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Anweisung {
    /// MYL verbrennen und dafür Inferenz-Credits erhalten (Kap. 5).
    Burn {
        /// Betrag in MYL-Kleinstbeträgen.
        betrag: u64,
    },
    /// MYL an ein anderes Konto überweisen.
    Ueberweisung {
        /// Empfängerkonto.
        nach: Address,
        /// Betrag in MYL-Kleinstbeträgen.
        betrag: u64,
    },
    /// Einen Session-Kontrakt eröffnen (Kap. 8.2).
    SitzungEroeffnen {
        /// Die Grenzen.
        kontrakt: Sitzungskontrakt,
    },
    /// Eine Session vorzeitig beenden. Nur der Inhaber.
    SitzungWiderrufen {
        /// Welche.
        sitzung: SitzungId,
    },
    /// Unter einer Session ausgeben. Nur der Agent.
    SitzungAusgeben {
        /// Was.
        vorhaben: Vorhaben,
        /// Die Vollmacht, falls nicht der Agent selbst einreicht.
        ///
        /// ⚑ **Angehängt und nicht eingefügt** (2026-09-03): Die
        /// Feldreihenfolge ist Protokollvertrag.
        ///
        /// **Ohne dieses Feld kann eine gerechnete Anfrage nicht
        /// abgebucht werden.** Ein Harness hält einen Bearer-Token und
        /// keinen Schlüssel; es kann keine Kettentransaktion signieren,
        /// also muss der Betreiber einreichen und die Kette die
        /// Vollmacht des Agenten anerkennen.
        vollmacht: Option<myl_types::vollmacht::Vollmacht>,
    },
    /// Sich als Miner anmelden oder die Hardware-Klasse ändern
    /// (Punkt 40, Glied 3a).
    ///
    /// ⚑ **Kein Feld für die Registrierungsepoche.** Sie setzt die
    /// Kette aus ihrem eigenen Zustand; ein selbst gewähltes Datum
    /// hübe den Registrierungsschluss aus Anhang A.2 auf, der gerade
    /// verhindern soll, dass sich jemand kurzfristig anmeldet, um eine
    /// Zuteilung zu beeinflussen.
    ///
    /// ⚑ **Und kein Feld für die Kennung.** Sie folgt aus dem
    /// Schlüssel, mit dem unterschrieben wurde, wie beim Absender: Ein
    /// zusätzliches Feld ließe sich abweichend füllen, und dann meldete
    /// A für B an.
    MinerAnmelden {
        /// Die Hardware-Klasse, in der gearbeitet wird.
        hardware: HardwareClass,
        /// Die Zone, in der gerechnet wird (Entscheidung 3b).
        ///
        /// ⚑ **Eine Angabe, keine Messung.** Sie entscheidet, in welchem
        /// Topf der Miner gemischt wird; wer eine falsche nennt, bremst
        /// die Zone, in die er sich setzt, und die Vergütung folgt der
        /// Arbeit. Der Gewinn gegenüber einer gegossipten Region ist
        /// nicht Wahrheit, sondern **Gleichheit**: Sie steht im
        /// Konsenszustand und ist für jeden Leser dieselbe.
        zone: GeoRegion,
        /// Wo der Miner im Netz erreichbar ist (Punkt 46).
        netzadresse: myl_types::latency_attest::PeerIdBytes,
    },
    /// Sich als Miner abmelden.
    ///
    /// **Angehängt und nicht eingefügt**, wie `ProposeMitPolka` im
    /// Rundenwechsel: Die Reihenfolge der Marken ist Teil des
    /// Konsensvertrags, und Borsh kodiert sie als Zahl.
    MinerAbmelden,
    /// Ein PoI-Bündel für die laufende Epoche einreichen
    /// (Punkt 40, Glied 1).
    ///
    /// ⚑ **Das Bündel trägt die Leistung des Pods, nicht die des
    /// Einzelnen.** `vtfe_claimed` gilt für den ganzen Pod; wer davon
    /// welchen Anteil bekommt, wird aus Besetzung und Zuschnitt
    /// **abgeleitet** (Festlegung vom 2026-08-31). Ein Feld „mein
    /// Anteil" gäbe es hier nicht, auch wenn jemand eines wollte: Ein
    /// Pod könnte damit intern umverteilen, und niemand außerhalb
    /// könnte widersprechen.
    /// Hinterlegt einen Einsatz (Punkt B11).
    ///
    /// ⚑ **Bis zum 2026-09-03 gab es keinen Weg dorthin** (Fund 145).
    /// `staked` stand im Zustand, der ganze wirtschaftliche
    /// Sicherheitsbau hing daran, und keine Anweisung schrieb es.
    ///
    /// **Additiv angehängt, nie eingefügt:** Die Reihenfolge der
    /// Varianten ist Konsensvertrag.
    EinsatzHinterlegen {
        /// In MYL-Kleinstbeträgen.
        betrag: u64,
    },
    /// Kündigt einen Teil des Einsatzes (Punkt B11).
    ///
    /// ⚑ **Der Betrag haftet weiter**, bis er abgeholt ist; die
    /// Sperrfrist verschiebt die Auszahlung, sie beendet die Haftung
    /// nicht.
    EinsatzKuendigen {
        /// In MYL-Kleinstbeträgen.
        betrag: u64,
    },
    /// Holt fälligen gekündigten Einsatz ins Guthaben (Punkt B11).
    EinsatzAbholen,
    BuendelEinreichen {
        /// Das Bündel.
        buendel: PoIBundle,
    },
    /// Trägt das Auszahlungskonto eines Miners ein oder ändert es
    /// (Fund 167).
    ///
    /// # ⚑ Warum es diese Variante geben muss
    ///
    /// `myl_ledger::transitions::auszahlungskonto_eintragen` gab es seit
    /// dem 2026-09-01, mit vollständiger Berechtigungsregel und eigenen
    /// Tests. **Nur führte kein Weg dorthin:** Ausserhalb der Tests rief
    /// sie niemand, weil keine `Anweisung` sie trug. **Ohne Eintrag kein
    /// Anteil**, und damit hätte auf einem echten Netz kein einziger
    /// Miner je etwas bekommen, ohne dass irgendwo ein Fehler entstanden
    /// wäre. Gefunden bei der Auswertung des Gesamtlaufs.
    ///
    /// ⚑ **Angehängt und nicht eingefügt.** Die Reihenfolge der
    /// Varianten ist Protokollvertrag: Borsh kodiert sie als Index, und
    /// eine eingeschobene Variante verschöbe jede dahinter.
    ///
    /// # ⚑ Warum die Kennung ein Feld ist, anders als bei `MinerAnmelden`
    ///
    /// Bei der Anmeldung folgt die Kennung aus dem Absender, denn nur
    /// der Miner selbst darf. **Hier nicht:** Jede weitere Eintragung
    /// gehört dem **eingetragenen kalten Konto**, und das ist gerade
    /// nicht die Kennung. Ohne das Feld liesse sich ein Auszahlungskonto
    /// nach der ersten Eintragung nie wieder ändern, und genau dafür ist
    /// die Trennung da.
    ///
    /// Ein falsch gefülltes Feld ist ungefährlich: Der Übergang prüft
    /// den Unterzeichner gegen die genannte Kennung und weist ab.
    AuszahlungskontoEintragen {
        /// Für welchen Miner.
        kennung: MinerId,
        /// Wohin die Erträge gehen.
        konto: Address,
    },
    /// `TrainingssegmentEinreichen`: Ein Pod liefert seine
    /// Trainingsarbeit ab (Whitepaper Kap. 7.2).
    ///
    /// ⚑ **Angehängt und nicht eingefügt**, wie jede Variante seit dem
    /// 2026-09-03: Borsh kodiert sie als Index, und eine eingeschobene
    /// Variante verschöbe jede dahinter.
    ///
    /// # ⚑ Warum es diese Variante geben muss
    ///
    /// `myl_types::trainingssegment::Trainingssegment` steht seit dem
    /// 2026-09-05 als Arbeitsklasse da, und **kein Weg führte dorthin**:
    /// Ein Pod konnte rechnen, aber nichts abliefern. Dieselbe Lage wie
    /// bei `AuszahlungskontoEintragen` (Fund 167) und bei der ganzen
    /// Kiste `myl-train` (Fund 183).
    ///
    /// # ⚑ Der Pod steht im Klartext, das Segment nennt ihn nicht
    ///
    /// `Trainingssegment` trägt einen `pod_pfad` aus Miner-Kennungen,
    /// aber keine `PodId`. Der Übergang braucht sie, um Dubletten zu
    /// sperren und um gegen den Trainingsplan zu prüfen.
    ///
    /// **Ein falsch gefülltes Feld ist ungefährlich:** Der Übergang
    /// leitet Koordinator und Besetzung aus dieser Kennung ab und prüft
    /// den Unterzeichner dagegen.
    TrainingssegmentEinreichen {
        /// Welcher Pod.
        pod: myl_types::ids::PodId,
        /// Das Segment.
        segment: myl_types::trainingssegment::Trainingssegment,
    },
    /// `SchuldspruchEinreichen`: Ein Streitfall endet, **mit Beleg**
    /// (Whitepaper Kap. 6.6, Anhang B.3).
    ///
    /// ⚑ **Angehängt und nicht eingefügt**, aus demselben Grund wie
    /// oben.
    ///
    /// # ⚑ Warum der Beleg in der Anweisung steht
    ///
    /// `Block::verdicts` gab es seit jeher, und das Feld trug einen
    /// Schuldspruch **ohne Nachweis**: Täter und Kopfgeldempfänger als
    /// blosse Namen. Bis zum 2026-09-06 wandte die Kette es nicht an,
    /// und **allein das** hat davor geschützt, dass ein Blockerzeuger
    /// jeden schlachten kann (Fund 192).
    ///
    /// Hier trägt die Anweisung den Beleg mit sich, und der Beleg trägt
    /// sich selbst: Er ist die **eigene Unterschrift des Beschuldigten**
    /// über den strittigen Übergang, in der Rolle Shard. Wer ihn nicht
    /// hat, kann niemanden benennen.
    ///
    /// ⚑ **Was er belegt und was nicht.** Er belegt die
    /// **Urheberschaft**, also wen es trifft. Dass die Rechnung falsch
    /// war, entscheidet die Bisektion; diese Anweisung setzt sie
    /// voraus, statt sie zu ersetzen.
    SchuldspruchEinreichen {
        /// Das strittige Segment.
        segment: myl_types::ids::SegmentId,
        /// Der Beleg, der den Ausgang trägt.
        beleg: myl_types::schuldbeleg::Belegart,
        /// Das Konto, das den Einsatz verliert.
        beschuldigt: myl_types::ids::Address,
        /// Das Konto, das das Kopfgeld erhält.
        anzeigend: myl_types::ids::Address,
    },
    /// ⚑ **Eine Kapazitätszusage einreichen** (Stufe 4 des Dispatchers,
    /// 2026-10-21): je Netzmodell Ebenen und Ebenenzeit, gültig ab einer
    /// künftigen Epoche. **Angehängt und nicht eingefügt**: Die Reihenfolge
    /// der Marken ist Teil des Konsensvertrags.
    ///
    /// Die Unterschrift in der Zusage gilt unter dem **registrierten**
    /// Schlüssel des Miners; die Transaktion selbst beweist nur, wer sie
    /// einreicht.
    ZusageEinreichen {
        /// Die Zusage, vom Halter unterschrieben.
        zusage: myl_types::zusage::Kapazitaetszusage,
    },
    /// ⚑ **Eine Fristmeldung einreichen** (Stufe 4, 6.8, 2026-10-22): Ein
    /// Mitglied hat seinen Bereich langsamer gerechnet, als es zugesagt hat,
    /// und eine Mehrheit der übrigen hat das je selbst gemessen.
    /// **Angehängt und nicht eingefügt**, aus demselben Grund wie oben.
    ///
    /// Einreichen darf nur ein Zeuge der Meldung; Pod und Bereich des
    /// Gemeldeten leitet die Kette selbst ab, aus Zuteilung und Schnitt der
    /// laufenden Epoche.
    FristverstossEinreichen {
        /// Die Meldung samt Zeugnissen.
        meldung: myl_types::fristmeldung::Fristmeldung,
    },
    /// ⚑ **Ein Archivbeleg** (Archivknoten, 2026-10-23): die Blöcke der
    /// Stichproben des Absenders in dieser Epoche, als Bytes, und je ein
    /// Beweis gegen die Historienwurzel der Langzeitgrenze. Angehängt.
    ///
    /// Die Blöcke stehen als Bytes und nicht als `Block`, denn ein Block
    /// trägt Transaktionen; der Ledger braucht nur ihren Hash, und der ist
    /// der Hash genau dieser Bytes.
    ArchivBelegen {
        /// Die Blöcke, in der Folge der Stichproben.
        bloecke: Vec<Vec<u8>>,
        /// Je Block der Beweis.
        beweise: Vec<myl_types::historie::Beweis>,
    },
    /// ⚑ **Eine Kopie nennt die Teilwurzel** des Abschnitts, den die
    /// Splitter halten (2026-10-23). Angehängt.
    ArchivTeilwurzel {
        /// Die Wurzel über die Gruppenwurzeln des Abschnitts.
        wurzel: myl_types::hash::Hash,
    },
    /// ⚑ **Ein Splitter belegt seinen Teil** der gezogenen Gruppe
    /// (2026-10-23). Angehängt.
    ArchivTeilBelegen {
        /// Der Teil.
        stueck: Vec<u8>,
        /// Die Pfade zur Teilwurzel.
        beweis: myl_types::archivteile::Teilbeweis,
    },
    /// ⚑ **Ein Beleg für eine Doppelunterschrift** (2026-10-27, CONSENSUS
    /// 7.6): zwei Blöcke derselben Höhe vom selben Erzeuger oder zwei
    /// Prüfpunktstimmen eines Wählers für dieselbe Höhe. Die Kette prüft
    /// ihn und schlachtet nach der Matrix (Validator, Doppelsignatur), der
    /// Melder bekommt das Kopfgeld. Angehängt.
    DoppelbelegEinreichen {
        /// Der Beleg (in einer `Box`: Er ist viel grösser als die übrigen
        /// Anweisungen; die Kodierung ist dieselbe).
        beleg: Box<crate::double_signing::Doppelbeleg>,
    },
}

/// Die Bytes, über die der Absender unterschreibt.
///
/// ⚑ **Die Kennung der Kette steht mit darin und nicht in der
/// Transaktion.** Eine Transaktion, die für Kette A unterschrieben
/// wurde, scheitert damit auf Kette B an der Prüfung, ohne dass 32
/// zusätzliche Bytes durch jedes Netz wandern. Ohne diese Bindung wäre
/// jede Testnetz-Überweisung auf dem Hauptnetz gültig.
///
/// **Und die Nummer steht mit darin**, sonst wäre dieselbe Unterschrift
/// beliebig oft einlösbar.
pub fn transaktionsbytes(
    netz: &Hash,
    absender: &BlsPublicKey,
    nonce: u64,
    max_gebuehr: u64,
    anweisung: &Anweisung,
) -> Vec<u8> {
    let kodiert = borsh::to_vec(anweisung).expect("Anweisung ist stets serialisierbar");
    let mut msg = Vec::with_capacity(DST_TRANSAKTION.len() + 32 + 48 + 16 + kodiert.len());
    msg.extend_from_slice(DST_TRANSAKTION);
    msg.extend_from_slice(netz.as_bytes());
    msg.extend_from_slice(&absender.0);
    msg.extend_from_slice(&nonce.to_le_bytes());
    msg.extend_from_slice(&max_gebuehr.to_le_bytes());
    msg.extend_from_slice(&kodiert);
    msg
}

/// Warum eine Transaktion nicht gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransaktionsFehler {
    /// Die Unterschrift passt nicht zu Schlüssel, Kette, Nummer oder
    /// Anweisung.
    SignaturStimmtNicht,
    /// Der öffentliche Schlüssel ist kein gültiger Gruppenpunkt.
    SchluesselUngueltig,
}

impl std::fmt::Display for TransaktionsFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignaturStimmtNicht => write!(f, "Unterschrift stimmt nicht"),
            Self::SchluesselUngueltig => write!(f, "Schlüssel ist kein gültiger Gruppenpunkt"),
        }
    }
}

impl std::error::Error for TransaktionsFehler {}

/// Eine unterschriebene Anweisung an das Ledger.
///
/// ⚑ **Bis zum 2026-08-28 trug eine Transaktion keine Unterschrift**
/// (Fund 85). Es gab nur `Burn`, und ein fremder Burn zerstört nur das
/// Geld des Opfers, statt es zu nehmen; das machte die Lücke unauffällig
/// und nicht harmlos. **Mit einer Überweisung wäre daraus Diebstahl
/// geworden:** Jeder hätte jedes Konto leeren können, indem er die
/// Anweisung einfach hinschreibt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Transaktion {
    /// Wer anweist, als öffentlicher Schlüssel.
    pub absender: BlsPublicKey,
    /// Fortlaufende Nummer dieses Kontos, gegen Wiedereinspielung.
    pub nonce: u64,
    /// ⚑ **Höchstens so viel Gebühr zahlt der Absender** (2026-10-27,
    /// Festlegung des Projektinhabers, G1): Liegt Grundpreis mal Bytes
    /// darüber, wartet die Transaktion, statt teurer zu werden, als der
    /// Absender unterschrieben hat. `u64::MAX` heisst: ohne Grenze.
    /// Unterschrieben wie alles andere.
    pub max_gebuehr: u64,
    /// Was getan werden soll.
    pub anweisung: Anweisung,
    /// Unterschrift über [`transaktionsbytes`].
    pub signatur: BlsSignature,
}

impl Transaktion {
    /// Das Konto, das belastet wird: `SHA-256` über den Schlüssel.
    ///
    /// **Abgeleitet und nicht angegeben** — siehe [`Anweisung`].
    pub fn absender_adresse(&self) -> Address {
        Address::aus_schluessel(&self.absender)
    }

    /// Unterschreibt eine Anweisung, ohne Grenze für die Gebühr.
    pub fn signiere(
        netz: &Hash,
        schluessel: &myl_types::bls::BlsSecretKey,
        nonce: u64,
        anweisung: Anweisung,
    ) -> Result<Self, myl_types::bls::BlsError> {
        Self::signiere_mit_grenze(netz, schluessel, nonce, u64::MAX, anweisung)
    }

    /// Unterschreibt eine Anweisung mit einer Grenze für die Gebühr (G1).
    pub fn signiere_mit_grenze(
        netz: &Hash,
        schluessel: &myl_types::bls::BlsSecretKey,
        nonce: u64,
        max_gebuehr: u64,
        anweisung: Anweisung,
    ) -> Result<Self, myl_types::bls::BlsError> {
        let absender = schluessel.public_key()?;
        let signatur = schluessel.sign(&transaktionsbytes(netz, &absender, nonce, max_gebuehr, &anweisung))?;
        Ok(Self { absender, nonce, max_gebuehr, anweisung, signatur })
    }

    /// Wie viele Bytes die Gebühr zählt: die kodierte Transaktion.
    pub fn gebuehrenbytes(&self) -> u64 {
        borsh::to_vec(self).map_or(u64::MAX, |b| b.len() as u64)
    }

    /// Prüft die Unterschrift gegen diese Kette.
    pub fn pruefe(self, netz: &Hash) -> Result<GepruefteTransaktion, TransaktionsFehler> {
        let bytes = transaktionsbytes(netz, &self.absender, self.nonce, self.max_gebuehr, &self.anweisung);
        if !self.absender.verify(&bytes, &self.signatur) {
            return Err(TransaktionsFehler::SignaturStimmtNicht);
        }
        Ok(GepruefteTransaktion(self))
    }
}

/// Eine Transaktion, deren Unterschrift geprüft wurde.
///
/// ⚑ **Es gibt keinen anderen Weg hierher als [`Transaktion::pruefe`].**
/// Dasselbe Muster wie bei der Gateway-Attestierung: Eine ungeprüfte
/// Transaktion sieht aus wie eine geprüfte, und ein Aufrufer, der es
/// vergisst, merkt nichts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GepruefteTransaktion(Transaktion);

impl GepruefteTransaktion {
    /// Die zugrunde liegende Transaktion.
    pub fn inhalt(&self) -> &Transaktion {
        &self.0
    }
}

/// Ein Block im Myelith-Netzwerk.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Block {
    /// Epochen-Metadaten.
    pub header: BlockHeader,
    /// Transaktionen, jede mit Unterschrift und Nummer.
    pub txs: Vec<Transaktion>,
    /// PoI-Bündel (Anhang A.1 — je Epoche und Pod eines).
    pub poi_bundles: Vec<PoIBundle>,
    /// Challenges (Anhang A.4).
    pub challenges: Vec<Challenge>,
    /// Verdicts — dieselbe Struktur, die `myl_ledger::apply_verdict`
    /// verarbeitet.
    pub verdicts: Vec<Verdict>,
    /// Die Unterschrift des Erzeugers (2026-10-27).
    ///
    /// ⚑ **Nicht im Blockhash.** Sie unterschreibt den Hash und kann ihn
    /// deshalb nicht enthalten; der Hash ist damit derselbe wie vor dem
    /// Feld, und keine Unterschrift verändert, welcher Block gemeint ist.
    /// **Additiv angehängt:** Die Feldreihenfolge ist Konsensvertrag.
    pub unterschrift: Option<Erzeugerunterschrift>,
}

impl Block {
    /// Erstellt einen neuen, leeren Block.
    pub fn new(header: BlockHeader) -> Self {
        Self {
            header,
            txs: Vec::new(),
            poi_bundles: Vec::new(),
            challenges: Vec::new(),
            verdicts: Vec::new(),
            unterschrift: None,
        }
    }

    /// Fügt eine Transaktion hinzu.
    pub fn add_transaction(&mut self, tx: Transaktion) {
        self.txs.push(tx);
    }

    /// Fügt ein PoI-Bündel hinzu.
    pub fn add_poi_bundle(&mut self, bundle: PoIBundle) {
        self.poi_bundles.push(bundle);
    }

    /// Fügt eine Challenge hinzu.
    pub fn add_challenge(&mut self, challenge: Challenge) {
        self.challenges.push(challenge);
    }

    /// Fügt ein Verdict hinzu.
    pub fn add_verdict(&mut self, verdict: Verdict) {
        self.verdicts.push(verdict);
    }

    /// Berechnet den Block-Hash (SHA-256 über kanonisches Borsh).
    ///
    /// Der Hash deckt `state_root` mit ab — eine Manipulation der
    /// gebuchten Zustandsübergänge verändert damit den Hash, über den
    /// abgestimmt wird.
    ///
    /// ⚑ **Ohne die Unterschrift des Erzeugers**: dieselben
    /// Bytes wie vor dem Feld, in derselben Reihenfolge.
    pub fn hash(&self) -> Hash {
        Hash::sha256(&self.hashbytes())
    }

    /// **Die Bytes, die der Blockhash deckt**: der Block ohne die
    /// Unterschrift des Erzeugers.
    ///
    /// 📌 Wer einen Block als Bytes weitergibt, die gegen seinen Hash geprüft
    /// werden (der Archivbeleg, `myl_ledger::archiv::archiv_belegen`), gibt
    /// **diese** Bytes, nicht die volle Kodierung. Bis zur Unterschrift des
    /// Erzeugers waren beide dasselbe, und genau darauf verliess sich der
    /// Archivbeleg still; mit ihr trug er nicht mehr, und kein Knoten
    /// verdichtete (gefunden an `tests/verdichtung.rs` im Knoten).
    pub fn hashbytes(&self) -> Vec<u8> {
        borsh::to_vec(&(&self.header, &self.txs, &self.poi_bundles, &self.challenges, &self.verdicts))
            .expect("Borsh-Serialisierung sollte nicht fehlschlagen")
    }

    /// **Der Rumpf**: die Bytes des Blockhashes ohne den Kopf. Borsh kodiert
    /// das Tupel als Folge seiner Teile, also gilt
    /// `hashbytes = borsh(Kopf) ‖ Rumpf` (2026-10-27, für den
    /// [`crate::double_signing::Doppelbeleg`]).
    pub fn rumpfbytes(&self) -> Vec<u8> {
        borsh::to_vec(&(&self.txs, &self.poi_bundles, &self.challenges, &self.verdicts))
            .expect("Borsh-Serialisierung sollte nicht fehlschlagen")
    }

    /// Der Blockhash aus Kopf und Rumpf, ohne den Rumpf zu zerlegen.
    pub fn hash_aus_teilen(kopf: &BlockHeader, rumpf: &[u8]) -> Hash {
        let mut b = borsh::to_vec(kopf).expect("Borsh-Serialisierung sollte nicht fehlschlagen");
        b.extend_from_slice(rumpf);
        Hash::sha256(&b)
    }

    /// Was der Erzeuger unterschreibt: `DST_BLOCK_ERZEUGER ‖ Blockhash`.
    /// Der Hash bindet Höhe, Vorgänger und damit Kette und Netz.
    pub fn erzeugernachricht(&self) -> Vec<u8> {
        erzeugernachricht_zum_hash(&self.hash())
    }

    /// Unterschreibt den Block als Erzeuger.
    pub fn unterschreiben(&mut self, sk: &myl_types::bls::BlsSecretKey) -> Result<(), myl_types::bls::BlsError> {
        let signatur = sk.sign(&self.erzeugernachricht())?;
        self.unterschrift = Some(Erzeugerunterschrift { schluessel: sk.public_key()?, signatur });
        Ok(())
    }

    /// **Wer diesen Block erzeugt hat, belegt durch seine Unterschrift.**
    ///
    /// `Ok(None)`: unsigniert. `Ok(Some(kennung))`: die Unterschrift trägt
    /// unter dem mitgeführten Schlüssel. `Err`: Sie steht da und trägt
    /// nicht; ein solcher Block ist abzuweisen, denn was im Block steht,
    /// muss geprüft werden oder darf nicht drin stehen (Fund 192).
    ///
    /// ⚠️ **Das belegt, wer, nicht dass er durfte.** Ob der Erzeuger zum
    /// Stimmsatz gehört, prüft der Übernehmer gegen seinen Satz.
    pub fn erzeuger(&self) -> Result<Option<MinerId>, UnterschriftTraegtNicht> {
        let Some(u) = &self.unterschrift else {
            return Ok(None);
        };
        if !u.schluessel.verify(&self.erzeugernachricht(), &u.signatur) {
            return Err(UnterschriftTraegtNicht);
        }
        Ok(Some(u.erzeuger()))
    }

    /// Gibt die Gesamtanzahl der Einträge zurück.
    pub fn total_entries(&self) -> usize {
        self.txs.len() + self.poi_bundles.len() + self.challenges.len() + self.verdicts.len()
    }

    /// Strukturelle Plausibilitätsprüfung der enthaltenen Challenges.
    ///
    /// Verwirft Blöcke mit offensichtlich unsinnigen Challenges (gleiche
    /// Miner, gleiche Hashes), ohne die Segment-Spuren zu kennen. Die
    /// vollständige Prüfung leistet VERIFICATION.
    pub fn validate_challenges(&self) -> Result<(), myl_types::ChallengeStructureError> {
        for c in &self.challenges {
            c.validate_structure()?;
        }
        Ok(())
    }

    /// Alle in diesem Block genannten Miner (aus Challenges und Verdicts
    /// lässt sich nicht direkt auf `MinerId` schließen — Verdicts führen
    /// Adressen). Liefert die Miner der Challenges.
    pub fn challenged_miners(&self) -> Vec<MinerId> {
        let mut out = Vec::new();
        for c in &self.challenges {
            out.push(c.primary_miner);
            out.push(c.redundant_miner);
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_ledger::transitions::VerdictOutcome;

    /// ⚑ **Die Unterschrift des Erzeugers**: Sie trägt unter
    /// ihrem Schlüssel, ändert den Blockhash nicht, und weder ein
    /// veränderter Block noch ein untergeschobener Schlüssel kommen durch.
    #[test]
    fn die_unterschrift_des_erzeugers_traegt_und_bindet_den_block() {
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[3u8; 32]).expect("Schlüssel");
        let mut block = Block::new(test_meta());
        assert_eq!(block.erzeuger(), Ok(None), "unsigniert ist kein Fehler");
        let vorher = block.hash();
        block.unterschreiben(&sk).expect("unterschreiben");
        assert_eq!(block.hash(), vorher, "die Unterschrift ändert den Blockhash");
        let kennung = MinerId::aus_schluessel(&sk.public_key().expect("pk"));
        assert_eq!(block.erzeuger(), Ok(Some(kennung)));
        // Kanonisch und rundweg: dieselben Bytes zurück.
        let bytes = borsh::to_vec(&block).expect("Borsh");
        let zurueck: Block = borsh::from_slice(&bytes).expect("lesen");
        assert_eq!(zurueck, block);
        // Die Bytes des Hashes sind die Kodierung ohne das letzte Feld.
        assert_eq!(Hash::sha256(&block.hashbytes()), block.hash());
        assert!(bytes.starts_with(&block.hashbytes()) && bytes.len() > block.hashbytes().len());

        // ⛔️ Gegenproben: ein veränderter Block, ein fremder Schlüssel.
        let mut veraendert = block.clone();
        veraendert.header.timestamp_ms += 1;
        assert_eq!(veraendert.erzeuger(), Err(UnterschriftTraegtNicht), "ein veränderter Block trug die Unterschrift");
        let fremd = myl_types::bls::BlsSecretKey::key_gen(&[4u8; 32]).expect("Schlüssel");
        let mut untergeschoben = block.clone();
        if let Some(u) = untergeschoben.unterschrift.as_mut() {
            u.schluessel = fremd.public_key().expect("pk");
        }
        assert_eq!(untergeschoben.erzeuger(), Err(UnterschriftTraegtNicht), "ein fremder Schlüssel trug die Unterschrift");
    }
    use myl_types::bls::BlsSignature;
    use myl_types::ids::{EpochId, MerkleRoot, PodId, SegmentId};

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    fn test_meta() -> BlockHeader {
        BlockHeader {
            // Höhe und Epoche passen zueinander: 42 · 1800.
            height: 42 * BLOECKE_JE_EPOCHE,
            epoch: 42,
            prev_block_hash: test_hash(0),
            timestamp_ms: 1_700_000_000_000,
            state_root: test_hash(99),
            pruefpunkt: None,
        }
    }

    fn test_bundle() -> PoIBundle {
        PoIBundle {
            epoch: EpochId(42),
            pod: PodId::new([7u8; 32]),
            segments_root: MerkleRoot::new([8u8; 32]),
            vtfe_claimed: 1234,
            aggregate_sig: BlsSignature([9u8; 96]),
            segmente: 1,
        }
    }

    fn test_challenge() -> Challenge {
        Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: MinerId::new([1u8; 32]),
            redundant_miner: MinerId::new([2u8; 32]),
            primary_hash: test_hash(1),
            redundant_hash: test_hash(2),
            timestamp_ms: 1_700_000_000_000,
            // Unsigniert: Dieser Test prueft die Struktur- und
            // Groessenpruefung, nicht die Unterschrift (Fund 96).
            signature: myl_types::bls::BlsSignature([0u8; 96]),
        }
    }

    fn test_verdict() -> Verdict {
        Verdict {
            segment_id: SegmentId::new([1u8; 32]),
            miner: Address::new([3u8; 32]),
            checker: Address::new([4u8; 32]),
            outcome: VerdictOutcome::SlashMiner,
        }
    }

    #[test]
    fn block_creation() {
        let block = Block::new(test_meta());
        assert_eq!(block.header.epoch, 42);
        assert_eq!(block.total_entries(), 0);
    }

    fn netz() -> Hash {
        Hash::sha256(b"Testkette")
    }

    fn schluessel(n: u8) -> myl_types::bls::BlsSecretKey {
        myl_types::bls::BlsSecretKey::key_gen(&[n; 32]).expect("Schlüsselerzeugung")
    }

    fn burn(n: u8, betrag: u64) -> Transaktion {
        Transaktion::signiere(&netz(), &schluessel(n), 0, Anweisung::Burn { betrag })
            .expect("signieren")
    }

    #[test]
    fn eine_unterschrift_bindet_kette_nummer_und_anweisung() {
        let tx = burn(1, 1_000);
        assert!(tx.clone().pruefe(&netz()).is_ok());

        // Fremde Kette.
        assert_eq!(
            tx.clone().pruefe(&Hash::sha256(b"andere Kette")),
            Err(TransaktionsFehler::SignaturStimmtNicht)
        );
        // Andere Nummer.
        let mut umnummeriert = tx.clone();
        umnummeriert.nonce = 1;
        assert_eq!(
            umnummeriert.pruefe(&netz()),
            Err(TransaktionsFehler::SignaturStimmtNicht)
        );
        // Anderer Betrag.
        let mut erhoeht = tx.clone();
        erhoeht.anweisung = Anweisung::Burn { betrag: 2_000 };
        assert_eq!(erhoeht.pruefe(&netz()), Err(TransaktionsFehler::SignaturStimmtNicht));
        // Fremder Schlüssel davorgesetzt.
        let mut untergeschoben = tx.clone();
        untergeschoben.absender = schluessel(2).public_key().expect("Schlüssel");
        assert_eq!(
            untergeschoben.pruefe(&netz()),
            Err(TransaktionsFehler::SignaturStimmtNicht)
        );
    }

    /// ⚑ Die Adresse wird aus dem Schlüssel gerechnet und steht nicht
    /// daneben. Sonst unterschriebe A und belastete B.
    #[test]
    fn die_adresse_folgt_aus_dem_schluessel() {
        let tx = burn(7, 1);
        let pk = schluessel(7).public_key().expect("Schlüssel");
        assert_eq!(tx.absender_adresse(), Address::new(Hash::sha256(&pk.0).0));
        assert_ne!(tx.absender_adresse(), burn(8, 1).absender_adresse());
    }

    #[test]
    fn block_nimmt_kanonische_typen_auf() {
        let mut block = Block::new(test_meta());
        block.add_transaction(burn(1, 1_000));
        block.add_poi_bundle(test_bundle());
        block.add_challenge(test_challenge());
        block.add_verdict(test_verdict());

        assert_eq!(block.total_entries(), 4);
    }

    /// Der Kern von Fund A8: Was `myl-pod` produziert, muss in den Block
    /// passen. Vorher war `Block::add_poi_bundle` auf eine andere
    /// Struktur typisiert und der Pfad Pod → Block nicht verdrahtet.
    #[test]
    fn poi_buendel_aus_myl_types_passt_in_den_block() {
        let bundle: PoIBundle = test_bundle();
        let mut block = Block::new(test_meta());
        block.add_poi_bundle(bundle.clone());
        assert_eq!(block.poi_bundles[0], bundle);
    }

    /// Das Verdict des Blocks muss der Ledger direkt verarbeiten können.
    #[test]
    fn verdict_ist_der_ledger_typ() {
        let v: myl_ledger::transitions::Verdict = test_verdict();
        let mut block = Block::new(test_meta());
        block.add_verdict(v);
        assert_eq!(block.verdicts[0].outcome, VerdictOutcome::SlashMiner);
    }

    #[test]
    fn block_hash_deterministisch() {
        let block = Block::new(test_meta());
        assert_eq!(block.hash(), block.hash());
    }

    #[test]
    fn block_hash_aendert_sich_mit_inhalt() {
        let mut a = Block::new(test_meta());
        let b = a.clone();
        a.add_challenge(test_challenge());
        assert_ne!(a.hash(), b.hash());
    }

    /// `state_root` muss in den Block-Hash eingehen — sonst könnte ein
    /// Leader die gebuchten Zustandsübergänge fälschen, ohne dass sich
    /// der Hash ändert, über den abgestimmt wird.
    #[test]
    fn state_root_geht_in_den_blockhash_ein() {
        let a = Block::new(test_meta());
        let mut meta_b = test_meta();
        meta_b.state_root = test_hash(100);
        let b = Block::new(meta_b);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn prev_block_hash_geht_in_den_blockhash_ein() {
        let a = Block::new(test_meta());
        let mut meta_b = test_meta();
        meta_b.prev_block_hash = test_hash(50);
        let b = Block::new(meta_b);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn block_borsh_roundtrip() {
        let mut block = Block::new(test_meta());
        block.add_poi_bundle(test_bundle());
        block.add_challenge(test_challenge());
        block.add_verdict(test_verdict());

        let bytes = borsh::to_vec(&block).unwrap();
        let decoded: Block = borsh::from_slice(&bytes).unwrap();
        assert_eq!(block, decoded);
    }

    #[test]
    fn unsinnige_challenge_wird_erkannt() {
        let mut block = Block::new(test_meta());
        let mut c = test_challenge();
        c.redundant_miner = c.primary_miner;
        block.add_challenge(c);
        assert!(block.validate_challenges().is_err());
    }

    #[test]
    fn gueltige_challenges_passieren() {
        let mut block = Block::new(test_meta());
        block.add_challenge(test_challenge());
        assert!(block.validate_challenges().is_ok());
    }

    #[test]
    fn challenged_miners_sind_sortiert_und_eindeutig() {
        let mut block = Block::new(test_meta());
        block.add_challenge(test_challenge());
        block.add_challenge(test_challenge());
        let miners = block.challenged_miners();
        assert_eq!(miners.len(), 2);
        assert!(miners.windows(2).all(|w| w[0] < w[1]));
    }
}

#[cfg(test)]
mod korpussaat {
    use super::*;

    /// Schreibt Saatkörner für den Fuzzer, wenn `KORPUS_NACH` gesetzt ist.
    ///
    /// ⚑ **Ein Test und kein Binary**, damit er mit dem Typ wandert. Er
    /// erzeugt gültige Kodierungen der **neuen** Varianten; der Fuzzer
    /// findet von dort aus die Nachbarschaft, was er aus zufälligen
    /// Bytes nie täte: Eine Anweisung mit Beleg hat rund zweihundert
    /// Bytes Struktur, und die trifft kein Zufall.
    #[test]
    fn saatkoerner_fuer_den_fuzzer() {
        let Ok(ziel) = std::env::var("KORPUS_NACH") else {
            return;
        };
        let d = std::path::PathBuf::from(&ziel);
        std::fs::create_dir_all(&d).expect("Verzeichnis");
        let beleg = myl_types::schuldbeleg::Belegart::PrimaerHatGerechnet(
            myl_types::schuldbeleg::Schuldbeleg {
                uebergang: myl_types::uebergang::TransitionSig {
                    segment_id: myl_types::ids::SegmentId::new([5u8; 32]),
                    shard_index: 2,
                    position: 7,
                    prev_hash: [1u8; 32],
                    next_hash: [2u8; 32],
                },
                schluessel: myl_types::bls::BlsPublicKey([3u8; 48]),
                signatur: myl_types::bls::BlsSignature([4u8; 96]),
            },
        );
        let proben = [Anweisung::SchuldspruchEinreichen {
            segment: myl_types::ids::SegmentId::new([5u8; 32]),
            beleg,
            beschuldigt: myl_types::ids::Address::new([6u8; 32]),
            anzeigend: myl_types::ids::Address::new([7u8; 32]),
        }];
        for (i, p) in proben.iter().enumerate() {
            let bytes = borsh::to_vec(p).expect("kodierbar");
            std::fs::write(d.join(format!("schuldspruch-{i}")), &bytes).expect("schreiben");
            eprintln!("[korpussaat] {} Bytes nach {}", bytes.len(), ziel);
        }
    }

    /// Schreibt Transaktionen mit `max_gebuehr` (G1), wenn
    /// `KORPUS_TRANSAKTION_NACH` gesetzt ist: ohne Grenze und mit.
    #[test]
    fn transaktionen_fuer_den_fuzzer() {
        let Ok(ziel) = std::env::var("KORPUS_TRANSAKTION_NACH") else {
            return;
        };
        let d = std::path::PathBuf::from(&ziel);
        std::fs::create_dir_all(&d).expect("Verzeichnis");
        let netz = Hash::sha256(b"korpus");
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[7u8; 32]).expect("Schluessel");
        let nach = myl_types::ids::Address::new([8u8; 32]);
        let proben = [
            Transaktion::signiere(&netz, &sk, 0, Anweisung::Ueberweisung { nach, betrag: 1 }).expect("signieren"),
            Transaktion::signiere_mit_grenze(&netz, &sk, 3, 600, Anweisung::Burn { betrag: 9 }).expect("signieren"),
        ];
        for (i, p) in proben.iter().enumerate() {
            let bytes = borsh::to_vec(p).expect("kodierbar");
            std::fs::write(d.join(format!("gebuehr-{i}")), &bytes).expect("schreiben");
        }
    }
}
