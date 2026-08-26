//! Gossip-Topic-Struktur (Punkt 1.3).
//!
//! Myelith nutzt getrennte Gossipsub-Topics je Nachrichtenklasse — das
//! hält die Verbreitungsmuster lesbar, erlaubt klassenspezifische
//! Validierung (Punkt 1.4) und verhindert, dass große PoI-Bündel den
//! Block-Gossip ausbremsen.
//!
//! **Konsens-Feld:** Die Topic-Namen sind Teil des Konsensvertrags.
//! Zwei Nodes, die nicht exakt dieselben Namen verwenden, tauschen
//! keine Nachrichten aus. Änderungen nur über Governance (Kap. 10.3);
//! die Endung `/1` ist die Namensschema-Version und wird bei
//! inkompatiblen Änderungen hochgezählt.
//!
//! Payload-Konvention: Die Nutzlast ist die Borsh-Serialisierung des
//! zugehörigen `myl-types`-Datentyps (z. B. `PoIBundle` für das
//! PoI-Bündel-Topic). Borsh ist kanonisch — dieselbe Struktur ergibt
//! auf jedem Node dieselben Bytes, Voraussetzung für alle Hashes und
//! Signaturen über Nachrichten.

use borsh::BorshSerialize;
use libp2p::gossipsub::{IdentTopic, MessageId, PublishError};
use libp2p::Swarm;

use crate::node::MylBehaviour;

/// Topic für Blöcke (BFT-Blockproduktion, CONSENSUS).
pub const TOPIC_BLOCKS: &str = "/myelith/blocks/1";
/// Topic für Transaktionen (u. a. Burn-Transaktionen für Credits).
pub const TOPIC_TRANSACTIONS: &str = "/myelith/transactions/1";
/// Topic für PoI-Bündel (pro Pod und Epoche, Anhang A.1).
pub const TOPIC_POI_BUNDLES: &str = "/myelith/poi-bundles/1";
/// Topic für Challenges (Bisektions-Spiel, VERIFICATION).
pub const TOPIC_CHALLENGES: &str = "/myelith/challenges/1";
/// Topic für signierte Latenz-Atteste (Phase 2, Grundlage des
/// LatencyGraph für die Pod-Bildung).
pub const TOPIC_LATENCY_ATTESTS: &str = "/myelith/latency-attests/1";
/// Topic für die BFT-Runden selbst: Propose, Vote, Commit.
///
/// **Getrennt von [`TOPIC_BLOCKS`], und das ist eine Entscheidung, keine
/// Ergänzung** (Projektinhaber, 2026-08-25). Beide Klassen tragen
/// Konsensverkehr, aber sie verhalten sich entgegengesetzt: Ein Block
/// ist groß, selten und für jeden interessant; eine Stimme ist 170 Bytes,
/// rundengebunden und nach der Runde wertlos. In einem gemeinsamen Topic
/// teilen sie Mesh, Bandbreite und **Bewertung** — wer das Topic mit
/// Stimmen flutet, trifft die Blockverbreitung mit, und ein Knoten, der
/// wegen Stimmenverhaltens aus dem Mesh fliegt, bekommt auch keine
/// Blöcke mehr. Zwei Topics kosten ein sechstes Mesh und trennen dafür
/// die beiden Fehlerfälle.
pub const TOPIC_CONSENSUS: &str = "/myelith/consensus/1";

/// Alle Protokoll-Topics in kanonischer Reihenfolge.
pub const ALL_TOPICS: [&str; 6] = [
    TOPIC_BLOCKS,
    TOPIC_TRANSACTIONS,
    TOPIC_POI_BUNDLES,
    TOPIC_CHALLENGES,
    TOPIC_LATENCY_ATTESTS,
    TOPIC_CONSENSUS,
];

/// Die Nachrichtenklassen des Protokolls (ein Wert je Topic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GossipTopic {
    Blocks,
    Transactions,
    PoiBundles,
    Challenges,
    LatencyAttests,
    /// Propose, Vote und Commit einer BFT-Runde.
    Consensus,
}

impl GossipTopic {
    /// Alle Nachrichtenklassen des Protokolls.
    ///
    /// Neben [`ALL_TOPICS`], das die **Namen** führt: Wer über Topics
    /// rechnet statt über Zeichenketten, braucht die Werte. Eine neue
    /// Variante fällt hier auf, weil der Test unten die Länge prüft.
    pub const ALLE: [GossipTopic; 6] = [
        Self::Blocks,
        Self::Transactions,
        Self::PoiBundles,
        Self::Challenges,
        Self::LatencyAttests,
        Self::Consensus,
    ];

    /// Der kanonische Topic-Name (Konsens-Feld).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Blocks => TOPIC_BLOCKS,
            Self::Transactions => TOPIC_TRANSACTIONS,
            Self::PoiBundles => TOPIC_POI_BUNDLES,
            Self::Challenges => TOPIC_CHALLENGES,
            Self::LatencyAttests => TOPIC_LATENCY_ATTESTS,
            Self::Consensus => TOPIC_CONSENSUS,
        }
    }

    /// Alle Topic-Varianten in kanonischer Reihenfolge.
    pub fn all() -> [GossipTopic; 6] {
        [
            Self::Blocks,
            Self::Transactions,
            Self::PoiBundles,
            Self::Challenges,
            Self::LatencyAttests,
            Self::Consensus,
        ]
    }

    /// Das zugehörige Gossipsub-Topic (Hash des Namens).
    pub fn topic(&self) -> IdentTopic {
        IdentTopic::new(self.name())
    }
}

/// Fehler der Gossip-Operationen.
#[derive(Debug)]
pub enum GossipError {
    /// Das Subscribe auf ein Topic ist fehlgeschlagen.
    SubscribeFailed(String),
    /// Die Serialisierung der Nutzlast ist fehlgeschlagen.
    SerializeFailed,
    /// Das Publizieren ist fehlgeschlagen (z. B. zu wenige Peers).
    PublishFailed(String),
}

impl std::fmt::Display for GossipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SubscribeFailed(t) => write!(f, "Subscribe fehlgeschlagen: {}", t),
            Self::SerializeFailed => write!(f, "Nutzlast-Serialisierung fehlgeschlagen"),
            Self::PublishFailed(e) => write!(f, "Publish fehlgeschlagen: {}", e),
        }
    }
}

impl std::error::Error for GossipError {}

impl From<PublishError> for GossipError {
    fn from(e: PublishError) -> Self {
        Self::PublishFailed(e.to_string())
    }
}

/// Abonniert alle Protokoll-Topics. Ein Node, der am Netzverkehr
/// teilnehmen will, abonniert die vollständige Topic-Liste; reine
/// Beobachter-Rollen (z. B. ein Client ohne Validator-Funktion) können
/// gezielt einzelne Topics abonnieren.
pub fn subscribe_all(swarm: &mut Swarm<MylBehaviour>) -> Result<(), GossipError> {
    for topic in GossipTopic::all() {
        subscribe(swarm, topic)?;
    }
    Ok(())
}

/// Abonniert ein einzelnes Topic.
pub fn subscribe(swarm: &mut Swarm<MylBehaviour>, topic: GossipTopic) -> Result<(), GossipError> {
    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&topic.topic())
        .map_err(|e| GossipError::SubscribeFailed(format!("{}: {}", topic.name(), e)))?;
    Ok(())
}

/// Publiziert eine Borsh-serialisierte Nachricht auf einem Topic.
///
/// Die Nutzlast muss der zum Topic gehörende `myl-types`-Datentyp sein
/// (Konvention, ab Punkt 1.4 durch Validierung erzwungen).
pub fn publish<T: BorshSerialize>(
    swarm: &mut Swarm<MylBehaviour>,
    topic: GossipTopic,
    data: &T,
) -> Result<MessageId, GossipError> {
    let bytes = borsh::to_vec(data).map_err(|_| GossipError::SerializeFailed)?;
    let id = swarm.behaviour_mut().gossipsub.publish(topic.topic(), bytes)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn die_werte_und_die_namen_decken_sich() {
        // Zwei Listen derselben Sache. Wächst eine, muss die andere
        // mitwachsen, sonst rechnet die eine Hälfte des Crates über
        // vier Topics und die andere über fünf.
        use super::{GossipTopic, ALL_TOPICS};
        assert_eq!(GossipTopic::ALLE.len(), ALL_TOPICS.len());
        for t in GossipTopic::ALLE {
            assert!(
                ALL_TOPICS.contains(&t.name()),
                "{:?} fehlt in ALL_TOPICS",
                t
            );
        }
    }

    use super::*;
    use crate::config::NetConfig;
    use crate::identity::NodeIdentity;
    use crate::node::build_swarm;
    use futures::StreamExt;
    use libp2p::gossipsub;
    use libp2p::swarm::SwarmEvent;
    use myl_types::ids::{EpochId, PodId, SegmentId};
    use myl_types::{segments_root, BlsSecretKey, PoIBundle};
    use std::time::Duration;

    #[test]
    fn topic_namen_sind_fest() {
        // Die Topic-Namen sind Konsens-Felder — dieser Test fixiert sie.
        assert_eq!(GossipTopic::Blocks.name(), "/myelith/blocks/1");
        assert_eq!(GossipTopic::Transactions.name(), "/myelith/transactions/1");
        assert_eq!(GossipTopic::PoiBundles.name(), "/myelith/poi-bundles/1");
        assert_eq!(GossipTopic::Challenges.name(), "/myelith/challenges/1");
        assert_eq!(GossipTopic::LatencyAttests.name(), "/myelith/latency-attests/1");
        assert_eq!(GossipTopic::Consensus.name(), "/myelith/consensus/1");
        assert_eq!(ALL_TOPICS.len(), GossipTopic::all().len());
    }

    #[test]
    fn subscribe_all_abonniert_alle_topics() {
        let identity = NodeIdentity::generate();
        let mut swarm = build_swarm(&identity, &NetConfig::default()).expect("Swarm");
        subscribe_all(&mut swarm).expect("Subscribe");
        let abonniert: Vec<String> = swarm
            .behaviour()
            .gossipsub
            .topics()
            .map(|t| t.to_string())
            .collect();
        for topic in GossipTopic::all() {
            assert!(
                abonniert.contains(&topic.name().to_string()),
                "Topic {} muss abonniert sein",
                topic.name()
            );
        }
    }

    /// Baut ein Beispiel-PoI-Bündel aus myl-types-Bausteinen.
    fn beispiel_bundle() -> PoIBundle {
        let sk = BlsSecretKey::key_gen(&[0x7au8; 32]).expect("KeyGen");
        let sig = sk.sign(b"poi").expect("Signatur");
        let ids = [SegmentId::new([1u8; 32]), SegmentId::new([2u8; 32])];
        PoIBundle {
            epoch: EpochId(7),
            pod: PodId::new([9u8; 32]),
            segments_root: segments_root(&ids).expect("Wurzel"),
            vtfe_claimed: 123_456,
            aggregate_sig: sig,
        }
    }

    /// Wartet auf die erste Listen-Adresse eines Swarms.
    async fn wait_for_listen_addr(swarm: &mut Swarm<MylBehaviour>) -> libp2p::Multiaddr {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.next().await.expect("Event") {
                return address;
            }
        }
    }

    #[tokio::test]
    async fn zwei_nodes_empfangen_gossip_auf_topic() {
        let identity_a = NodeIdentity::generate();
        let identity_b = NodeIdentity::generate();
        let config = NetConfig::default();

        // Node A hört, Node B bootstrappt zu A.
        let mut swarm_a = build_swarm(&identity_a, &config).expect("Swarm A");
        swarm_a
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr"))
            .expect("Listen");
        let addr_a = wait_for_listen_addr(&mut swarm_a).await;
        let addr_a = addr_a.with_p2p(identity_a.peer_id()).expect("p2p");

        let config_b = NetConfig {
            bootstrap_peers: vec![addr_a.to_string()],
            ..Default::default()
        };
        let mut swarm_b = build_swarm(&identity_b, &config_b).expect("Swarm B");
        crate::discovery::bootstrap_from_config(&mut swarm_b, &config_b).expect("Bootstrap");

        // Beide abonnieren das PoI-Bündel-Topic.
        subscribe(&mut swarm_a, GossipTopic::PoiBundles).expect("Subscribe A");
        subscribe(&mut swarm_b, GossipTopic::PoiBundles).expect("Subscribe B");

        let bundle = beispiel_bundle();
        let erwartet = borsh::to_vec(&bundle).expect("Serialisierung");

        // Treiben, bis B publizieren kann (Mesh-Aufbau braucht ein, zwei
        // Heartbeats) und A die Nachricht empfängt.
        let mut publiziert = false;
        let empfangen = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                tokio::select! {
                    ev_a = swarm_a.next() => {
                        if let Some(SwarmEvent::Behaviour(
                            crate::node::MylBehaviourEvent::Gossipsub(
                                gossipsub::Event::Message { message, .. },
                            ),
                        )) = ev_a
                        {
                            if message.topic == GossipTopic::PoiBundles.topic().hash() {
                                break message.data;
                            }
                        }
                    }
                    ev_b = swarm_b.next() => {
                        let _ = ev_b;
                        if !publiziert && swarm_b.is_connected(&identity_a.peer_id())
                            && publish(&mut swarm_b, GossipTopic::PoiBundles, &bundle).is_ok() {
                                publiziert = true;
                            }
                    }
                }
            }
        })
        .await;

        let daten = empfangen.expect("Gossip-Nachricht lief in den Timeout");
        assert_eq!(daten, erwartet);
    }
}
