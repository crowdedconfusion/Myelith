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
//! - **Verbindungsgrenzen** ([`crate::limits`], Punkt 4.3): Deckel je
//!   Peer, je Richtung und insgesamt. Schließt Fund 53.
//! - **Adressvielfalt** ([`crate::limits::Adressvielfalt`]): eingehende
//!   Verbindungen je IPv4-/24 bzw. IPv6-/64.
//!
//! **Reihenfolge der Felder ist bedeutsam.** Der `NetworkBehaviour`-
//! Ableiter fragt die Behaviours in Feldreihenfolge, ob eine Verbindung
//! angenommen wird. Die beiden Grenzen stehen deshalb **vorn**: Eine
//! abgelehnte Verbindung soll abgelehnt sein, bevor Gossipsub oder
//! Kademlia Zustand für sie anlegen.
//!
//! - **Relais-Client, DCUtR, AutoNAT** ([`crate::nat`], Punkt 3.4):
//!   Erreichbarkeit hinter NAT. Die Server-Rollen (Relais, AutoNAT)
//!   sind `Toggle` und nur aktiv, wenn der Knoten sich dazu erklärt.
//!
//! Transport: TCP **und QUIC**, beide mit Noise bzw. TLS. QUIC ist
//! nicht Beiwerk: Lochstanzen über TCP („simultaneous open") scheitert
//! an vielen verbreiteten NAT-Bauarten, über UDP gelingt es zuverlässig.
//! Begründung im Kopf von [`crate::nat`]. Yamux multiplext den
//! TCP-Pfad; QUIC bringt Streams selbst mit.
//! Quantum-Vermerk: Noise/X25519 ist Shor-anfällig (Hop-für-Hop-Schutz);
//! der Inhaltsschutz liegt auf der verpflichtenden Session-E2E-Schicht
//! (Phase 3), beide als Migrationspunkte dokumentiert.

use std::time::Duration;

use libp2p::allow_block_list;
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{
    autonat, connection_limits, dcutr, gossipsub, identify, kad, noise, ping, relay, tcp, yamux,
    StreamProtocol, Swarm, SwarmBuilder,
};

use crate::config::{NetConfig, AGENT_VERSION, MAX_GOSSIP_MESSAGE_BYTES, PROTOCOL_VERSION};
use crate::discovery::KAD_PROTOCOL;
use crate::identity::NodeIdentity;
use crate::limits::Adressvielfalt;
use crate::scoring;

/// Kombiniertes Verhalten eines Myelith-Nodes.
#[derive(NetworkBehaviour)]
pub struct MylBehaviour {
    /// Gesperrte Gegenstellen.
    ///
    /// **Steht als Erstes**, und das ist keine Ordnungsfrage: Ein
    /// `NetworkBehaviour` aus mehreren Teilen fragt sie der Reihe nach,
    /// und wer eine Verbindung ablehnen soll, muss vor denen stehen, die
    /// sie annehmen wollen.
    ///
    /// **Wozu:** Eine Sperre trennt zwei Knoten **unabhängig von der
    /// Adresse**. Genau das braucht ein Partitionstest, denn `identify`
    /// und `kad` verteilen die echten Horchadressen weiter; ein Proxy
    /// dazwischen wäre umgehbar, und ein Test, der die Umgehung nicht
    /// bemerkt, misst nichts.
    ///
    /// Für den Betrieb ist es dieselbe Sache aus dem anderen Blickwinkel:
    /// Wer eine Gegenstelle als böswillig erkannt hat, will sie
    /// loswerden, und nicht nur ihre gerade benutzte Adresse.
    pub sperrliste: allow_block_list::Behaviour<allow_block_list::BlockedPeers>,
    /// Zahlengrenzen: je Peer, je Richtung, insgesamt (Fund 53).
    pub grenzen: connection_limits::Behaviour,
    /// Herkunftsgrenze: eingehende Verbindungen je Adressbereich.
    pub adressvielfalt: Adressvielfalt,
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    /// Relais-**Client**: über ein fremdes Relais erreichbar werden.
    /// Immer aktiv, denn ein Knoten weiß beim Start noch nicht, ob er
    /// hinter NAT sitzt.
    pub relay_client: relay::client::Behaviour,
    /// Lochstanzen auf einer vermittelten Verbindung. Immer aktiv: Jede
    /// direkt gemachte Verbindung nimmt dem Relais seinen Hebel.
    pub dcutr: dcutr::Behaviour,
    /// AutoNAT-**Client**: die eigene Erreichbarkeit feststellen.
    pub autonat_client: autonat::v2::client::Behaviour,
    /// Relais-**Server**. Nur wenn der Knoten sich dazu erklärt: Ein
    /// Relais bezahlt fremden Verkehr mit eigener Bandbreite.
    pub relay_server: Toggle<relay::Behaviour>,
    /// AutoNAT-**Server**. Setzt dasselbe voraus wie der Relais-Dienst,
    /// nämlich öffentliche Erreichbarkeit, und hängt am selben Schalter.
    pub autonat_server: Toggle<autonat::v2::server::Behaviour>,
    /// Punkt-zu-Punkt-Anfragen ([`crate::anfrage`]). Trägt undurchsichtige
    /// Bytes: Was sie bedeuten, entscheidet die Anwendung.
    pub anfrage: crate::anfrage::AnfrageBehaviour,
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
    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(identity.keypair().clone()),
        gossipsub_config,
    )
    .map_err(|e| format!("Gossipsub-Verhalten fehlgeschlagen: {}", e))?;

    // Peer-Scoring (Punkt 4.3): begrenzt, was eine Verbindung dem
    // Angreifer nützt, nachdem `connection_limits` begrenzt hat, wie
    // viele er bekommt. Begründung der Werte in `crate::scoring`.
    gossipsub
        .with_peer_score(scoring::standard_parameter(), scoring::standard_schwellen())
        .map_err(|e| format!("Gossipsub-Peer-Scoring fehlgeschlagen: {}", e))?;

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

    let grenzen = connection_limits::Behaviour::new(config.grenzen.clone());
    let adressvielfalt = Adressvielfalt::mit_grenze(config.adressbereich_grenze);
    let peer_id = identity.peer_id();
    let dient_als_relais = config.nat.dient_als_relais;

    let swarm = SwarmBuilder::with_existing_identity(identity.keypair().clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        // Der Relais-Client kommt aus dem Builder, weil er einen eigenen
        // Transport mitbringt: Über ihn laufen die vermittelten
        // Verbindungen.
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|_key, relay_client| {
            Ok(MylBehaviour {
                sperrliste: allow_block_list::Behaviour::default(),
                grenzen,
                adressvielfalt,
                gossipsub,
                kad,
                identify,
                ping,
                relay_client,
                dcutr: dcutr::Behaviour::new(peer_id),
                autonat_client: autonat::v2::client::Behaviour::default(),
                relay_server: Toggle::from(
                    dient_als_relais.then(|| relay::Behaviour::new(peer_id, relay::Config::default())),
                ),
                autonat_server: Toggle::from(
                    dient_als_relais.then(autonat::v2::server::Behaviour::default),
                ),
                anfrage: crate::anfrage::baue_anfragekanal(),
            })
        })?
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
