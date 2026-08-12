//! `myl-net` — L0-Netzwerkschicht für Myelith (Whitepaper Kap. 3.2).
//!
//! Aufgabe: P2P-Gossip für Blöcke, Transaktionen, PoI-Bündel und
//! Challenges; kontinuierliche Paarlatenzmessung als Grundlage des
//! `LatencyGraph` für die Pod-Bildung (Kap. 4.1/4.3, Anhang A.2);
//! Ende-zu-Ende-verschlüsselte Kanäle für Nutzer↔Gateway↔Pipeline und
//! zwischen den Shards eines Pods (Kap. 9.2).
//!
//! Design-Entscheidungen (2026-08-13, dokumentiert in
//! `NETWORKING/README/Fahrplan-v1.md`):
//!
//! 1. **P2P-Stack: rust-libp2p** — Gossipsub, Kademlia-DHT, Noise-
//!    Transport. Quantum-Vermerk: Noise/X25519 ist Shor-anfällig und als
//!    Migrationspunkt dokumentiert (PQ-Noise/Hybrid-KEM, sobald
//!    standardisiert).
//! 2. **Latenzmessung:** Ping alle 15 s je aktivem Peer, EMA-geglättetes
//!    RTT (α = 0,25), signiertes Attest alle 5 Minuten ins Gossip. Die
//!    Werte sind später Governance-Parameter; die EMA rechnet in
//!    Ganzzahlen (Festkomma), keine Gleitkomma-Arithmetik.
//! 3. **Verschlüsselung: zwei Schichten.** Transport (libp2p-Noise,
//!    Hop-für-Hop) plus verpflichtende Session-E2E (Schlüssel je
//!    Pod/Epoche, Rotation bei Epochenwechsel, Forward Secrecy). Ein
//!    kompromittiertes Gateway darf Inhalte nicht lesen können —
//!    Gateways sind in Kap. 9.2 eine explizite Angreiferklasse.
//!
//! Konsens-Regel wie überall in Myelith: kein `unsafe`, keine
//! Gleitkomma-Arithmetik in protokollrelevanten Berechnungen.

#![deny(unsafe_code)]

pub mod config;
pub mod discovery;
pub mod gossip;
pub mod identity;
pub mod node;
pub mod runtime;
pub mod validation;

pub use config::NetConfig;
pub use discovery::{bootstrap_from_config, parse_bootstrap_peer, start_bootstrap, DiscoveryError, KAD_PROTOCOL};
pub use gossip::{
    publish, subscribe, subscribe_all, GossipError, GossipTopic, ALL_TOPICS, TOPIC_BLOCKS,
    TOPIC_CHALLENGES, TOPIC_LATENCY_ATTESTS, TOPIC_POI_BUNDLES, TOPIC_TRANSACTIONS,
};
pub use identity::NodeIdentity;
pub use node::{build_swarm, MylBehaviour};
pub use runtime::{run_node, InboundMessage, NodeCommand, NodeEvent};
pub use validation::{report, topic_from_hash, validate_payload, ValidationError};
