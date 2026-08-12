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
//! 3. **Struktur-Validierung per Borsh** für Topics, deren Datentyp in
//!    `myl-types` bereits existiert (aktuell PoI-Bündel). Für Blöcke,
//!    Transaktionen, Challenges und Latenz-Atteste gelten bis zur
//!    Definition der Typen in CONSENSUS/VERIFICATION nur die
//!    Größenlimits (dokumentierte Zwischenstufe).
//!
//! Inhaltliche Konsensregeln (z. B. BLS-Signatur über das PoI-Bündel,
//! Epochengültigkeit) sind Aufgabe von CONSENSUS und werden hier nicht
//! vorweggenommen — die Netzschicht erzwingt nur die transportnahen
//! Regeln, damit das Netz auch unter adversarialem Verkehr funktions-
//! fähig bleibt.

use libp2p::gossipsub::{MessageAcceptance, MessageId, TopicHash};
use libp2p::{PeerId, Swarm};

use crate::gossip::GossipTopic;
use crate::node::MylBehaviour;

/// Maximale Größe einer Block-Nachricht (Zwischenwert bis CONSENSUS den
/// Block-Typ definiert; dann wird auch die Struktur geprüft).
pub const MAX_BLOCKS_BYTES: usize = 2 * 1024 * 1024;
/// Maximale Größe einer Transaktions-Nachricht.
pub const MAX_TRANSACTIONS_BYTES: usize = 64 * 1024;
/// Maximale Größe eines PoI-Bündels.
pub const MAX_POI_BUNDLES_BYTES: usize = 512 * 1024;
/// Maximale Größe einer Challenge-Nachricht.
pub const MAX_CHALLENGES_BYTES: usize = 64 * 1024;
/// Maximale Größe eines Latenz-Attests.
pub const MAX_LATENCY_ATTESTS_BYTES: usize = 4 * 1024;

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
        // PoI-Bündel: vollständige Borsh-Strukturprüfung gegen den
        // myl-types-Typ (Anhang A.1).
        GossipTopic::PoiBundles => {
            borsh::from_slice::<myl_types::PoIBundle>(data)
                .map_err(|_| ValidationError::MalformedPayload)?;
        }
        // Blöcke, Transaktionen, Challenges, Latenz-Atteste: Die
        // zugehörigen Typen entstehen in CONSENSUS/VERIFICATION bzw. in
        // Phase 2; bis dahin nur Größenprüfung (dokumentierte
        // Zwischenstufe, siehe Modul-Dokumentation).
        GossipTopic::Blocks
        | GossipTopic::Transactions
        | GossipTopic::Challenges
        | GossipTopic::LatencyAttests => {}
    }
    Ok(())
}

/// Meldet das Validierungsergebnis einer gehaltenen Gossip-Nachricht an
/// Gossipsub zurück: `Accept` gibt die Nachricht zur Weiterverbreitung
/// frei, `Reject` verwirft sie netzweit (für diesen Node) und bestraft
/// den Absender im Gossipsub-Scoring.
pub fn report(
    swarm: &mut Swarm<MylBehaviour>,
    message_id: &MessageId,
    source: &PeerId,
    topic_hash: &TopicHash,
    data: &[u8],
) -> MessageAcceptance {
    let ok = match topic_from_hash(topic_hash) {
        Some(topic) => validate_payload(topic, data).is_ok(),
        None => false,
    };
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
    use myl_types::ids::{EpochId, PodId, SegmentId};
    use myl_types::{segments_root, BlsSecretKey, PoIBundle};

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
}
