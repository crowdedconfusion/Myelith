//! Nachrichtenvalidierung vor Weiterverbreitung (Punkt 1.4).
//!
//! Gossip-Spam- und Manipulationsschutz: Jede eingehende Nachricht wird
//! geprüft, BEVOR sie weiterverbreitet wird. Dafür ist Gossipsub auf
//! `validate_messages()` konfiguriert — Nachrichten bleiben gehalten, bis
//! der Node sie mit [`report`] als gültig (`Accept`) meldet; ungültige
//! werden mit `Reject` verworfen und nicht weitergeleitet.
//!
//! Prüfstufen:
//! 1. **Gossipsub-Authentizität** (`ValidationMode::Strict`): Jede
//!    Nachricht muss vom Absender-Peer signiert sein — unsignierte oder
//!    imitierte Nachrichten scheitern bereits auf Protokollebene.
//! 2. **Größenlimits je Topic** (gegen Ressourcen-Erschöpfung): unten
//!    als Konstanten definiert; sie sind später Governance-Parameter.
//! 3. **Struktur-Validierung per Borsh** für alle Topics, deren Datentyp
//!    in `myl-types` liegt: PoI-Bündel, Challenges und Latenz-Atteste.
//!    Bei Challenges und Attesten kommt eine strukturelle
//!    Plausibilitätsprüfung dazu (verschiedene Miner, verschiedene
//!    Hashes bzw. Feldgrenzen) — das ist alles, was ohne Kenntnis der
//!    Segment-Spur bzw. des Netzzustands entscheidbar ist.
//!
//! ## ⚑ Fund 45: Wie viel Stufe 3 wirklich filtert
//!
//! Hier stand bis zum 2026-08-23 „vollständige Borsh-Strukturprüfung".
//! Das sagt mehr, als geschieht, und die adversariale Testebene hat es
//! gemessen: Von 20 000 verstümmelten Nachrichten kamen durch
//!
//! | Topic | angenommen | warum |
//! |---|---|---|
//! | `PoiBundles` | **20 000 von 20 000** | nur Felder fester Länge |
//! | `Challenges` | **20 000 von 20 000** | dito, plus zwei Ungleichungen, die Zufall erfüllt |
//! | `LatencyAttests` | 9 081 von 20 000 | enthält einen Vektor, dessen Längenkopf passen muss |
//!
//! **Bei einem Typ aus lauter Feldern fester Länge ist ein Borsh-Parse
//! eine Längenprüfung.** Jede Bytefolge der richtigen Länge ist ein
//! gültiges `PoIBundle`. Das ist kein Fehler, sondern eine Eigenschaft
//! des Formats, aber es verschiebt die Frage: Die Verteidigung für
//! PoI-Bündel ist die Aggregatsignatur, und die kann L0 nicht prüfen.
//! **Solange kein [`PayloadValidator`] verdrahtet ist, verbreitet ein
//! Knoten ein Bündel aus Zufallsbytes weiter.** Das gilt es zu wissen,
//! bevor jemand aus „Strukturprüfung" schließt, das Netz filtere schon.
//!
//! Festgehalten als Tatsache in
//! `tests/adversarial.rs::fuer_feste_typen_ist_die_borsh_pruefung_eine_laengenpruefung`:
//! Der Test schlägt fehl, sobald jemand eine echte Prüfung ergänzt, und
//! zwingt damit zur Aktualisierung dieser Stelle.
//!
//! **Blöcke und Transaktionen bleiben bewusst bei der Größenprüfung.**
//! Ihre Typen liegen in `myl-consensus` (L1); `myl-net` ist die
//! Netzschicht (L0) und darf nicht an die Konsensschicht hängen, sonst
//! wird die Schichtung umgekehrt. Wer beide Seiten kennt — die
//! Node-Verdrahtung — reicht die vollständige Prüfung über
//! [`PayloadValidator`] herein. Ohne einen solchen Validator gilt für
//! diese beiden Topics weiterhin nur das Größenlimit; das ist eine
//! bewusste Entscheidung, keine Auslassung.
//!
//! **Signaturen prüft diese Schicht nicht** (mit Ausnahme der
//! Gossipsub-Peer-Signatur aus Stufe 1). Ein Latenz-Attest trägt eine
//! BLS-Signatur, deren Gültigkeit nur gegen die Validator-Registry
//! entscheidbar ist — auch das läuft über [`PayloadValidator`].
//!
//! Inhaltliche Konsensregeln (Epochengültigkeit, Stake-Prüfung,
//! Attest-Signaturen) sind Aufgabe von CONSENSUS und werden hier nicht
//! vorweggenommen — die Netzschicht erzwingt nur die transportnahen
//! Regeln, damit das Netz auch unter adversarialem Verkehr funktions-
//! fähig bleibt.

use libp2p::gossipsub::{MessageAcceptance, MessageId, TopicHash};
use libp2p::{PeerId, Swarm};

use crate::gossip::GossipTopic;
use crate::node::MylBehaviour;

/// Maximale Größe einer Block-Nachricht. Die Strukturprüfung von Blöcken
/// erfolgt über einen [`PayloadValidator`] (Schichtung, siehe Modul-Doku).
pub const MAX_BLOCKS_BYTES: usize = 2 * 1024 * 1024;
/// Maximale Größe einer Transaktions-Nachricht.
pub const MAX_TRANSACTIONS_BYTES: usize = 64 * 1024;
/// Maximale Größe eines PoI-Bündels.
pub const MAX_POI_BUNDLES_BYTES: usize = 512 * 1024;
/// Maximale Größe einer Challenge-Nachricht.
pub const MAX_CHALLENGES_BYTES: usize = 64 * 1024;
/// Maximale Größe eines Latenz-Attests.
pub const MAX_LATENCY_ATTESTS_BYTES: usize = 4 * 1024;
/// Maximale Größe einer BFT-Konsensnachricht.
///
/// **Hergeleitet, nicht geraten.** Die größte heute definierte Nachricht
/// ist ein Propose: Enum-Marke (1) + Runde (8) + Block-Hash (32) +
/// Miner-Id (32) + BLS-Signatur (96) = **169 Bytes**, gemessen in
/// `myl_consensus::bft::groessenmessung`.
///
/// *Hier stand am 2026-08-26 zunächst, ein späterer Rundenwechsel mit
/// Polka-Zertifikat trage „eine Teilnahme-Bitmaske (16 bei 128
/// Validatoren)" und bleibe „unter 512". **Das war aus einer Annahme
/// gerechnet statt aus dem Typ.** `myl_consensus::round_change::
/// PolkaCertificate` führt keine Bitmaske, sondern die Unterzeichner
/// einzeln als `Vec<MinerId>`, also 32 Bytes je Stimme. Die Schlussfolgerung
/// hält, die Zwischenrechnung nicht.*
///
/// Aus dem Typ gerechnet (Borsh: Runde 8 + Hash 32 + Längenkopf 4 +
/// 32 je Unterzeichner + Aggregat 96), dazu ein Propose:
///
/// | Unterzeichner | Propose + Zertifikat | Anteil an 8 KiB |
/// |---|---|---|
/// | 5 (Probenetz) | 469 B | 6 % |
/// | 21 (`COMMITTEE_SIZE`) | 981 B | 12 % |
/// | 128 | 4405 B | 54 % |
///
/// Die 8 KiB tragen also auch das größte plausible Komitee, mit knapp
/// dem Doppelten an Luft. Weit genug, dass diese Grenze keinen Entwurf
/// einschränkt, und eng genug, dass eine Flut den Angreifer Bandbreite
/// kostet.
///
/// ⚑ **Wer den Rundenwechsel anschließt, rechnet diese Tabelle nach.**
/// Bei mehr als 128 Unterzeichnern reicht die Grenze nicht mehr, und
/// dann ist die Bitmaske die richtige Antwort, nicht ein größeres Limit:
/// Die Unterzeichnerliste ist redundant, sobald der Validator-Satz
/// bekannt ist.
///
/// Die Strukturprüfung erfolgt über einen [`PayloadValidator`], weil die
/// Typen in `myl-consensus` (L1) liegen. Siehe Modul-Doku.
pub const MAX_CONSENSUS_BYTES: usize = 8 * 1024;

/// Fehler der Inhalts-Validierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    /// Die Nachricht überschreitet das Topic-Größenlimit.
    TooLarge { got: usize, max: usize },
    /// Die Nutzlast ist kein gültiger Borsh-Datensatz des Topic-Typs.
    MalformedPayload,
    /// Das Topic ist kein Myelith-Protokoll-Topic.
    UnknownTopic,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { got, max } => {
                write!(f, "Nachricht zu groß: {} Bytes (Limit {})", got, max)
            }
            Self::MalformedPayload => write!(f, "Nutzlast ist kein gültiger Topic-Datensatz"),
            Self::UnknownTopic => write!(f, "unbekanntes Topic"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Ordnet einen Topic-Hash dem Myelith-Protokoll-Topic zu
/// (None für fremde Topics).
pub fn topic_from_hash(hash: &TopicHash) -> Option<GossipTopic> {
    GossipTopic::all()
        .into_iter()
        .find(|t| t.topic().hash() == *hash)
}

/// Größenlimit des Topics (Bytes).
pub fn max_payload_bytes(topic: GossipTopic) -> usize {
    match topic {
        GossipTopic::Blocks => MAX_BLOCKS_BYTES,
        GossipTopic::Transactions => MAX_TRANSACTIONS_BYTES,
        GossipTopic::PoiBundles => MAX_POI_BUNDLES_BYTES,
        GossipTopic::Challenges => MAX_CHALLENGES_BYTES,
        GossipTopic::LatencyAttests => MAX_LATENCY_ATTESTS_BYTES,
        GossipTopic::Consensus => MAX_CONSENSUS_BYTES,
    }
}

/// Validiert eine Nutzlast gegen die transportnahen Regeln des Topics.
pub fn validate_payload(topic: GossipTopic, data: &[u8]) -> Result<(), ValidationError> {
    let max = max_payload_bytes(topic);
    if data.len() > max {
        return Err(ValidationError::TooLarge {
            got: data.len(),
            max,
        });
    }
    match topic {
        // PoI-Bündel: Borsh-Parse gegen den myl-types-Typ (Anhang A.1).
        // Da der Typ nur Felder fester Länge hat, ist das in der Sache
        // eine Längenprüfung — siehe Fund 45 in der Moduldoku. Die
        // inhaltliche Prüfung ist die Aggregatsignatur und gehört in
        // einen PayloadValidator.
        GossipTopic::PoiBundles => {
            borsh::from_slice::<myl_types::PoIBundle>(data)
                .map_err(|_| ValidationError::MalformedPayload)?;
        }
        // Challenge: Borsh-Struktur plus die Plausibilitätsprüfung, die
        // ohne Kenntnis der Segment-Spur entscheidbar ist (verschiedene
        // Miner, verschiedene Hashes). Verhindert, dass offensichtlich
        // unsinnige Streitanzeigen das Netz fluten.
        GossipTopic::Challenges => {
            let challenge = borsh::from_slice::<myl_types::Challenge>(data)
                .map_err(|_| ValidationError::MalformedPayload)?;
            challenge
                .validate_structure()
                .map_err(|_| ValidationError::MalformedPayload)?;
        }
        // Latenz-Attest: Borsh-Struktur plus die vorhandene
        // Feldprüfung. Die BLS-Signatur ist hier NICHT prüfbar — dafür
        // braucht es die Validator-Registry (siehe PayloadValidator).
        GossipTopic::LatencyAttests => {
            let attest = borsh::from_slice::<myl_types::LatencyAttest>(data)
                .map_err(|_| ValidationError::MalformedPayload)?;
            attest
                .validate_structure()
                .map_err(|_| ValidationError::MalformedPayload)?;
        }
        // Blöcke, Transaktionen und Konsensnachrichten: Ihre Typen
        // liegen in myl-consensus (L1). Die Netzschicht (L0) darf nicht
        // daran hängen — die vollständige Prüfung kommt über einen
        // PayloadValidator von der Node-Verdrahtung. Bewusste
        // Entscheidung, keine Auslassung.
        GossipTopic::Blocks | GossipTopic::Transactions | GossipTopic::Consensus => {}
    }
    Ok(())
}

/// Hereingereichte Prüfung für Topics, deren Typen oberhalb der
/// Netzschicht liegen.
///
/// `myl-net` ist L0 und kennt weder `myl-consensus` noch die
/// Validator-Registry. Wer beide Seiten kennt — die Node-Verdrahtung —
/// implementiert diesen Trait und reicht ihn an [`report_with`]. Damit
/// bleibt die Schichtung erhalten und die Netzschicht kann trotzdem
/// vollständig validieren, statt Blöcke ungeprüft weiterzuverbreiten.
///
/// Aufrufe müssen **schnell und seiteneffektfrei** sein: Sie laufen im
/// Gossip-Pfad für jede eingehende Nachricht.
pub trait PayloadValidator {
    /// Prüft eine Nutzlast, die `validate_payload` nicht abschließend
    /// beurteilen kann.
    ///
    /// **Returns:** `true`, wenn die Nachricht weiterverbreitet werden
    /// darf. Die transportnahen Prüfungen aus [`validate_payload`]
    /// laufen bereits vorher — hier geht es um Konsensregeln
    /// (Blockstruktur, Attest-Signaturen, Epochengültigkeit).
    fn validate(&self, topic: GossipTopic, data: &[u8]) -> bool;
}

/// Ein Validator, der nichts zusätzlich prüft.
///
/// Nur für Tests und für Nodes, die bewusst ohne Konsensschicht laufen
/// (z. B. reine Relay-Knoten). Im Produktivbetrieb eines Validators ist
/// das die falsche Wahl.
#[derive(Debug, Clone, Copy, Default)]
pub struct AcceptAllValidator;

impl PayloadValidator for AcceptAllValidator {
    fn validate(&self, _topic: GossipTopic, _data: &[u8]) -> bool {
        true
    }
}

/// Meldet das Validierungsergebnis einer gehaltenen Gossip-Nachricht an
/// Gossipsub zurück: `Accept` gibt die Nachricht zur Weiterverbreitung
/// frei, `Reject` verwirft sie netzweit (für diesen Node) und bestraft
/// den Absender im Gossipsub-Scoring.
/// Prüft nur die transportnahen Regeln. Für Blöcke und Transaktionen
/// bleibt es dabei bei der Größenprüfung — nutze [`report_with`], sobald
/// die Konsensschicht verfügbar ist.
/// Warum eine Nachricht verworfen wurde.
///
/// Ohne diesen Grund ist eine verworfene Nachricht im Betrieb stumm,
/// und „B hat nichts empfangen" ließe sich nicht von „B hat es
/// weggeworfen" unterscheiden. Für die Fehlersuche ist das der
/// Unterschied zwischen einer Netzfrage und einer Formatfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablehnungsgrund {
    /// Das Topic gehört nicht zum Protokoll.
    FremdesTopic,
    /// Größe oder Struktur passen nicht (Stufe 2 und 3 dieser Schicht).
    Transportregel,
    /// Die Nutzlastprüfung der Anwendung hat abgelehnt.
    Nutzlastpruefung,
}

impl Ablehnungsgrund {
    /// Kurzwort fürs Betriebsprotokoll.
    pub fn als_text(&self) -> &'static str {
        match self {
            Self::FremdesTopic => "fremdes-topic",
            Self::Transportregel => "transportregel",
            Self::Nutzlastpruefung => "nutzlastpruefung",
        }
    }
}

/// Beurteilt eine Nachricht, ohne sie zu melden.
///
/// Getrennt von [`report_with`], damit der Grund einer Ablehnung
/// verfügbar ist statt nur das Ergebnis.
pub fn beurteile(
    topic_hash: &TopicHash,
    data: &[u8],
    validator: &dyn PayloadValidator,
) -> Result<GossipTopic, Ablehnungsgrund> {
    let Some(topic) = topic_from_hash(topic_hash) else {
        return Err(Ablehnungsgrund::FremdesTopic);
    };
    if validate_payload(topic, data).is_err() {
        return Err(Ablehnungsgrund::Transportregel);
    }
    if !validator.validate(topic, data) {
        return Err(Ablehnungsgrund::Nutzlastpruefung);
    }
    Ok(topic)
}

pub fn report(
    swarm: &mut Swarm<MylBehaviour>,
    message_id: &MessageId,
    source: &PeerId,
    topic_hash: &TopicHash,
    data: &[u8],
) -> MessageAcceptance {
    report_with(swarm, message_id, source, topic_hash, data, &AcceptAllValidator)
}

/// Wie [`report`], aber mit einer zusätzlichen, von oben hereingereichten
/// Prüfung für die Topics, die `myl-net` nicht abschließend beurteilen
/// kann (Blöcke, Transaktionen, Attest-Signaturen).
///
/// Die transportnahen Regeln laufen zuerst; der `PayloadValidator` wird
/// nur befragt, wenn sie bestanden sind. So kann eine teure
/// Konsensprüfung nicht als DoS-Fläche vor den billigen Filtern stehen.
pub fn report_with(
    swarm: &mut Swarm<MylBehaviour>,
    message_id: &MessageId,
    source: &PeerId,
    topic_hash: &TopicHash,
    data: &[u8],
    validator: &dyn PayloadValidator,
) -> MessageAcceptance {
    let ok = beurteile(topic_hash, data, validator).is_ok();
    swarm.behaviour_mut().gossipsub.report_message_validation_result(
        message_id,
        source,
        if ok {
            MessageAcceptance::Accept
        } else {
            MessageAcceptance::Reject
        },
    );
    if ok {
        MessageAcceptance::Accept
    } else {
        MessageAcceptance::Reject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::gossipsub::IdentTopic;
    use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};
    use myl_types::{segments_root, BlsSecretKey, Challenge, Hash, PoIBundle};

    fn gueltige_challenge() -> Challenge {
        Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: MinerId::new([1u8; 32]),
            redundant_miner: MinerId::new([2u8; 32]),
            primary_hash: Hash::sha256(b"a"),
            redundant_hash: Hash::sha256(b"b"),
            timestamp_ms: 1_700_000_000_000,
        }
    }

    fn gueltiges_bundle() -> Vec<u8> {
        let sk = BlsSecretKey::key_gen(&[0x5au8; 32]).expect("KeyGen");
        let sig = sk.sign(b"poi").expect("Signatur");
        let ids = [SegmentId::new([3u8; 32])];
        let bundle = PoIBundle {
            epoch: EpochId(1),
            pod: PodId::new([4u8; 32]),
            segments_root: segments_root(&ids).expect("Wurzel"),
            vtfe_claimed: 42,
            aggregate_sig: sig,
        };
        borsh::to_vec(&bundle).expect("Serialisierung")
    }

    #[test]
    fn gueltiges_buendel_wird_akzeptiert() {
        assert!(validate_payload(GossipTopic::PoiBundles, &gueltiges_bundle()).is_ok());
    }

    #[test]
    fn ungueltiges_borsh_wird_abgelehnt() {
        assert_eq!(
            validate_payload(GossipTopic::PoiBundles, b"kein-borsh"),
            Err(ValidationError::MalformedPayload)
        );
    }

    #[test]
    fn uebergroesse_nachricht_wird_abgelehnt() {
        let zu_gross = vec![0u8; MAX_LATENCY_ATTESTS_BYTES + 1];
        assert_eq!(
            validate_payload(GossipTopic::LatencyAttests, &zu_gross),
            Err(ValidationError::TooLarge {
                got: MAX_LATENCY_ATTESTS_BYTES + 1,
                max: MAX_LATENCY_ATTESTS_BYTES,
            })
        );
    }

    #[test]
    fn topic_hash_zuordnung() {
        for topic in GossipTopic::all() {
            let hash = IdentTopic::new(topic.name()).hash();
            assert_eq!(topic_from_hash(&hash), Some(topic));
        }
        let fremd = IdentTopic::new("/fremdes/topic/1").hash();
        assert_eq!(topic_from_hash(&fremd), None);
    }

    // ── Challenge-Validierung (Fund A12) ────────────────────────────

    #[test]
    fn gueltige_challenge_wird_akzeptiert() {
        let data = borsh::to_vec(&gueltige_challenge()).unwrap();
        assert!(validate_payload(GossipTopic::Challenges, &data).is_ok());
    }

    /// Vorher wurde jede Bytefolge unterhalb des Groessenlimits
    /// akzeptiert und weiterverbreitet.
    #[test]
    fn challenge_mit_kaputtem_borsh_wird_abgelehnt() {
        assert_eq!(
            validate_payload(GossipTopic::Challenges, b"kein-borsh"),
            Err(ValidationError::MalformedPayload)
        );
    }

    #[test]
    fn challenge_mit_gleichen_minern_wird_abgelehnt() {
        let mut c = gueltige_challenge();
        c.redundant_miner = c.primary_miner;
        let data = borsh::to_vec(&c).unwrap();
        assert_eq!(
            validate_payload(GossipTopic::Challenges, &data),
            Err(ValidationError::MalformedPayload)
        );
    }

    #[test]
    fn challenge_ohne_abweichung_wird_abgelehnt() {
        let mut c = gueltige_challenge();
        c.redundant_hash = c.primary_hash;
        let data = borsh::to_vec(&c).unwrap();
        assert_eq!(
            validate_payload(GossipTopic::Challenges, &data),
            Err(ValidationError::MalformedPayload)
        );
    }

    #[test]
    fn latenz_attest_mit_kaputtem_borsh_wird_abgelehnt() {
        assert_eq!(
            validate_payload(GossipTopic::LatencyAttests, b"kein-borsh"),
            Err(ValidationError::MalformedPayload)
        );
    }

    /// Bloecke bleiben bewusst bei der Groessenpruefung — ihre Typen
    /// liegen in der Konsensschicht. Die vollstaendige Pruefung kommt
    /// ueber einen PayloadValidator.
    #[test]
    fn bloecke_bleiben_bei_der_groessenpruefung() {
        assert!(validate_payload(GossipTopic::Blocks, b"beliebige-bytes").is_ok());
        let zu_gross = vec![0u8; MAX_BLOCKS_BYTES + 1];
        assert!(validate_payload(GossipTopic::Blocks, &zu_gross).is_err());
    }

    #[test]
    fn payload_validator_kann_ablehnen() {
        struct AlleAblehnen;
        impl PayloadValidator for AlleAblehnen {
            fn validate(&self, _t: GossipTopic, _d: &[u8]) -> bool {
                false
            }
        }
        assert!(!AlleAblehnen.validate(GossipTopic::Blocks, b"x"));
        assert!(AcceptAllValidator.validate(GossipTopic::Blocks, b"x"));
    }
}
