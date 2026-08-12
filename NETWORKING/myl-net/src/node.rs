//! Swarm-Aufbau: die libp2p-Stack-Integration (Punkt 1.1, Kademlia ab 1.2).
//!
//! Kombination der Behaviours für Phase 1:
//! - **Gossipsub** — Nachrichtenaustausch (Topics ab Punkt 1.3,
//!   Validierung ab Punkt 1.4), signierte Nachrichten
//!   (`MessageAuthenticity::Signed`).
//! - **Kademlia-DHT** — Peer-Discovery (Punkt 1.2), unter dem
//!   Myelith-eigenen Protokoll-Namen [`crate::discovery::KAD_PROTOCOL`].
//! - **Identify** — Protokoll-/Agent-Version und Adressen-Austausch
//!   beim Verbindungsaufbau.
//! - **Ping** — Grundlage der Paarlatenzmessung (Phase 2); das
//!   Intervall ist der entschiedene Parameter (15 s).
//!
//! Transport: TCP mit Noise-Verschlüsselung und Yamux-Multiplexing.
//! Quantum-Vermerk: Noise/X25519 ist Shor-anfällig (Hop-für-Hop-Schutz);
//! der Inhaltsschutz liegt auf der verpflichtenden Session-E2E-Schicht
//! (Phase 3), beide als Migrationspunkte dokumentiert.

use std::time::Duration;

use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, kad, noise, ping, tcp, yamux, StreamProtocol, Swarm, SwarmBuilder};

use crate::config::{NetConfig, AGENT_VERSION, MAX_GOSSIP_MESSAGE_BYTES, PROTOCOL_VERSION};
use crate::discovery::KAD_PROTOCOL;
use crate::identity::NodeIdentity;

/// Kombiniertes Verhalten eines Myelith-Nodes.
#[derive(NetworkBehaviour)]
pub struct MylBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

/// Baut den Swarm aus Identität und Konfiguration.
///
/// Führt kein `listen_on` aus — das entscheidet der Aufruufer (Tests
/// binden ephemere Ports, Produktiv-Nodes die konfigurierte Adresse).
pub fn build_swarm(
    identity: &NodeIdentity,
    config: &NetConfig,
) -> Result<Swarm<MylBehaviour>, Box<dyn std::error::Error + Send + Sync>> {
    // Gossipsub-Konfiguration:
    // - `max_transmit_size`: hartes Transport-Größenlimit.
    // - `validate_messages()`: Nachrichten werden gehalten, bis die
    //   Validierung (Punkt 1.4, `validation::report`) sie freigibt —
    //   nichts Ungültiges wird weiterverbreitet.
    // - `ValidationMode::Strict`: jede Nachricht muss einen gültigen
    //   Absender mit Signatur tragen — unsignierte/imitierte
    //   Nachrichten scheitern auf Protokollebene.
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .max_transmit_size(MAX_GOSSIP_MESSAGE_BYTES)
        .validate_messages()
        .validation_mode(gossipsub::ValidationMode::Strict)
        .build()
        .map_err(|e| format!("Gossipsub-Konfiguration fehlgeschlagen: {}", e))?;
    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(identity.keypair().clone()),
        gossipsub_config,
    )
    .map_err(|e| format!("Gossipsub-Verhalten fehlgeschlagen: {}", e))?;

    // Kademlia unter eigenem Protokoll-Namen (Protokoll-Isolation,
    // kein Mitsprechen in fremden Kademlia-Netzen auf demselben Port).
    let kad_config = kad::Config::new(StreamProtocol::new(KAD_PROTOCOL));
    let kad = kad::Behaviour::with_config(
        identity.peer_id(),
        kad::store::MemoryStore::new(identity.peer_id()),
        kad_config,
    );

    let identify = identify::Behaviour::new(
        identify::Config::new(PROTOCOL_VERSION.to_string(), identity.keypair().public())
            .with_agent_version(AGENT_VERSION.to_string()),
    );

    let ping = ping::Behaviour::new(ping::Config::new().with_interval(config.ping_interval));

    let behaviour = MylBehaviour {
        gossipsub,
        kad,
        identify,
        ping,
    };

    let swarm = SwarmBuilder::with_existing_identity(identity.keypair().clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| Ok(behaviour))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok(swarm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swarm_uebernimmt_die_node_identitaet() {
        let identity = NodeIdentity::generate();
        let config = NetConfig::default();
        let swarm = build_swarm(&identity, &config).expect("Swarm-Aufbau");
        assert_eq!(swarm.local_peer_id(), &identity.peer_id());
    }

    #[test]
    fn zwei_nodes_haben_unterschiedliche_identitaeten() {
        let a = NodeIdentity::generate();
        let b = NodeIdentity::generate();
        assert_ne!(a.peer_id(), b.peer_id());
        let config = NetConfig::default();
        let swarm_a = build_swarm(&a, &config).expect("Swarm A");
        let swarm_b = build_swarm(&b, &config).expect("Swarm B");
        assert_ne!(swarm_a.local_peer_id(), swarm_b.local_peer_id());
    }
}
