//! Peer-Discovery: Bootstrap-Peers und Kademlia-DHT (Punkt 1.2).
//!
//! Einstieg ins Netz: Ein Node kennt zu Beginn null oder mehrere
//! Bootstrap-Peers (Multiaddr mit `p2p/…`-Anteil). Diese werden
//! kontaktiert und in die Kademlia-Routing-Tabelle eingetragen; danach
//! startet `kad::bootstrap()` die iterative Suche nach weiteren Peers.
//! Der erste Node eines Netzes hat keine Bootstrap-Peers — er wartet
//! schlicht darauf, dass andere sich bei ihm melden.
//!
//! Die Kademlia-Routing-Tabelle ist Netz-Topologie, nicht Konsens-Zustand:
//! Sie darf sich zwischen Nodes unterscheiden und ist nicht
//! determinismuskritisch (Konsens läuft über Blöcke/PoI-Bündel).

use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId, Swarm};

use crate::config::NetConfig;
use crate::node::MylBehaviour;

/// Protokoll-Name der Myelith-Kademlia-Instanz. Eigenständiger Name
/// (statt des libp2p-Standard-`/ipfs/kad/1/0/0`), damit Myelith-Nodes
/// nicht mit fremden Kademlia-Netzen auf demselben Port sprechen.
/// Konsens-Feld: Änderung bricht die Discovery-Kompatibilität.
pub const KAD_PROTOCOL: &str = "/myelith/kad/1";

/// Fehler der Discovery.
#[derive(Debug)]
pub enum DiscoveryError {
    /// Eine Bootstrap-Adresse ist keine gültige Multiaddr.
    InvalidAddress(String),
    /// Einer Bootstrap-Adresse fehlt der `p2p/…`-Anteil (PeerId).
    MissingPeerId(String),
    /// Kademlia hat keine bekannten Peers — `bootstrap()` ist dann
    /// nicht möglich (normal für den ersten Node eines Netzes).
    NoKnownPeers,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAddress(a) => write!(f, "ungültige Bootstrap-Adresse: {}", a),
            Self::MissingPeerId(a) => {
                write!(f, "Bootstrap-Adresse ohne p2p/-Anteil (PeerId): {}", a)
            }
            Self::NoKnownPeers => write!(f, "keine bekannten Peers für Kademlia-Bootstrap"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Zerlegt eine Bootstrap-Adresse in PeerId und Multiaddr.
pub fn parse_bootstrap_peer(addr: &str) -> Result<(PeerId, Multiaddr), DiscoveryError> {
    let multiaddr: Multiaddr = addr
        .parse()
        .map_err(|_| DiscoveryError::InvalidAddress(addr.to_string()))?;
    let peer_id = multiaddr
        .iter()
        .find_map(|p| match p {
            Protocol::P2p(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| DiscoveryError::MissingPeerId(addr.to_string()))?;
    Ok((peer_id, multiaddr))
}

/// Kontaktiert alle konfigurierten Bootstrap-Peers und trägt ihre
/// Adressen in die Kademlia-Routing-Tabelle ein.
///
/// Liefert die Anzahl der eingetragenen Peers. Eine leere
/// Bootstrap-Liste ist zulässig (der Node ist dann selbst ein
/// Einstiegspunkt des Netzes) und liefert 0.
pub fn bootstrap_from_config(
    swarm: &mut Swarm<MylBehaviour>,
    config: &NetConfig,
) -> Result<usize, DiscoveryError> {
    let mut count = 0usize;
    for addr in &config.bootstrap_peers {
        let (peer_id, multiaddr) = parse_bootstrap_peer(addr)?;
        // Adresse in die Routing-Tabelle aufnehmen.
        swarm.behaviour_mut().kad.add_address(&peer_id, multiaddr.clone());
        // Verbindungsaufbau anstoßen (läuft asynchron im Event-Loop).
        swarm.dial(multiaddr).map_err(|_| DiscoveryError::InvalidAddress(addr.clone()))?;
        count += 1;
    }
    Ok(count)
}

/// Startet die iterative Kademlia-Bootstrap-Suche nach weiteren Peers.
///
/// Schlägt mit [`DiscoveryError::NoKnownPeers`] fehl, wenn die
/// Routing-Tabelle noch leer ist — das ist für den ersten Node eines
/// Netzes der Normalfall und kein Fehlerzustand des Nodes selbst.
pub fn start_bootstrap(
    swarm: &mut Swarm<MylBehaviour>,
) -> Result<libp2p::kad::QueryId, DiscoveryError> {
    swarm
        .behaviour_mut()
        .kad
        .bootstrap()
        .map_err(|_| DiscoveryError::NoKnownPeers)
}

/// **Welche Adressen einer Gegenstelle ins Adressbuch gehören** (Fund 594,
/// 2026-10-22).
///
/// Ein Knoten, der auf allen Schnittstellen horcht (`0.0.0.0`), nennt über
/// `identify` und mDNS jede davon, die Rückschleife eingeschlossen. Wer
/// `127.0.0.1` einer **fremden** Gegenstelle wählt, erreicht sich selbst:
/// Im Lauf mit getrennten Prozessen stand nach einer Trennung nur noch
/// diese Adresse im Adressbuch, und jede Neuwahl endete mit „Unexpected
/// peer ID“ beim eigenen Schlüssel. Im Lauf im selben Prozess fiel es nie
/// auf, denn dort sitzen alle wirklich auf der Rückschleife.
///
/// Deshalb: **Die Rückschleife nur, wenn die Gegenstelle nichts anderes
/// nennt** (dann sitzt sie auf derselben Maschine, wie in jeder Probe),
/// und eine unspezifische Adresse (`0.0.0.0`, `::`) nie, denn sie ist eine
/// Horchangabe und kein Ziel.
pub fn fuer_das_adressbuch(adressen: impl IntoIterator<Item = Multiaddr>) -> Vec<Multiaddr> {
    let gezielt: Vec<Multiaddr> = adressen.into_iter().filter(|a| !ist_ip(a, |ip| ip.is_unspecified())).collect();
    if gezielt.iter().any(|a| !ist_ip(a, |ip| ip.is_loopback())) {
        gezielt.into_iter().filter(|a| !ist_ip(a, |ip| ip.is_loopback())).collect()
    } else {
        gezielt
    }
}

/// Ob die erste IP-Angabe der Adresse die Bedingung erfüllt; eine Adresse
/// ohne IP (DNS, Relais allein) erfüllt sie nie.
fn ist_ip(a: &Multiaddr, bedingung: impl Fn(std::net::IpAddr) -> bool) -> bool {
    a.iter().find_map(|p| match p {
        Protocol::Ip4(ip) => Some(bedingung(ip.into())),
        Protocol::Ip6(ip) => Some(bedingung(ip.into())),
        _ => None,
    }) == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NodeIdentity;

    fn a(text: &str) -> Multiaddr {
        text.parse().expect("Multiaddr")
    }

    /// Fund 594: Wer eine andere Adresse nennt, kommt ohne Rückschleife ins
    /// Adressbuch; wer nur die Rückschleife nennt, mit ihr; die
    /// unspezifische Adresse nie.
    #[test]
    fn die_rueckschleife_nur_ohne_andere_adresse() {
        let gemischt = fuer_das_adressbuch([
            a("/ip4/127.0.0.1/udp/4150/quic-v1"),
            a("/ip4/10.77.0.14/tcp/4150"),
            a("/ip6/::1/tcp/4150"),
            a("/ip4/0.0.0.0/tcp/4150"),
            a("/dns4/knoten.example/tcp/4150"),
        ]);
        assert_eq!(gemischt, vec![a("/ip4/10.77.0.14/tcp/4150"), a("/dns4/knoten.example/tcp/4150")]);

        let nur_rueckschleife = fuer_das_adressbuch([a("/ip4/127.0.0.1/tcp/4150"), a("/ip4/0.0.0.0/tcp/4150")]);
        assert_eq!(nur_rueckschleife, vec![a("/ip4/127.0.0.1/tcp/4150")], "Proben auf einer Maschine bleiben möglich");

        assert!(fuer_das_adressbuch([a("/ip6/::/tcp/4150")]).is_empty());
    }
    use crate::node::build_swarm;
    use futures::StreamExt;
    use libp2p::swarm::SwarmEvent;
    use std::time::Duration;

    #[test]
    fn bootstrap_adresse_wird_zerlegt() {
        let identity = NodeIdentity::generate();
        let addr = format!("/ip4/127.0.0.1/tcp/4150/p2p/{}", identity.peer_id());
        let (peer_id, multiaddr) = parse_bootstrap_peer(&addr).expect("Parse");
        assert_eq!(peer_id, identity.peer_id());
        assert_eq!(multiaddr.to_string(), addr);
    }

    #[test]
    fn adresse_ohne_peer_id_wird_abgelehnt() {
        assert!(matches!(
            parse_bootstrap_peer("/ip4/127.0.0.1/tcp/4150"),
            Err(DiscoveryError::MissingPeerId(_))
        ));
    }

    #[test]
    fn ungueltige_adresse_wird_abgelehnt() {
        assert!(matches!(
            parse_bootstrap_peer("keine-multiaddr"),
            Err(DiscoveryError::InvalidAddress(_))
        ));
    }

    /// Wartet auf die erste Listen-Adresse eines Swarms.
    async fn wait_for_listen_addr(
        swarm: &mut Swarm<MylBehaviour>,
    ) -> Multiaddr {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.next().await.expect("Event") {
                return address;
            }
        }
    }

    #[tokio::test]
    async fn zwei_nodes_verbinden_sich_ueber_bootstrap() {
        let identity_a = NodeIdentity::generate();
        let identity_b = NodeIdentity::generate();
        let config = NetConfig::default();

        // Node A: hört auf einem ephemeren Port.
        let mut swarm_a = build_swarm(&identity_a, &config).expect("Swarm A");
        swarm_a
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr"))
            .expect("Listen");
        let addr_a = wait_for_listen_addr(&mut swarm_a).await;
        let addr_a = addr_a.with_p2p(identity_a.peer_id()).expect("p2p-Anhang");

        // Node B: bootstrappt über Node A.
        let config_b = NetConfig {
            bootstrap_peers: vec![addr_a.to_string()],
            ..Default::default()
        };
        let mut swarm_b = build_swarm(&identity_b, &config_b).expect("Swarm B");
        let peers = bootstrap_from_config(&mut swarm_b, &config_b).expect("Bootstrap-Eintrag");
        assert_eq!(peers, 1);
        start_bootstrap(&mut swarm_b).expect("Kademlia-Bootstrap");

        // Beide Swarms treiben, bis B mit A verbunden ist.
        let verbunden = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if swarm_b.is_connected(&identity_a.peer_id()) {
                    break true;
                }
                tokio::select! {
                    _ = swarm_a.next() => {}
                    _ = swarm_b.next() => {}
                }
            }
        })
        .await;
        assert!(verbunden.is_ok(), "Verbindungsaufbau lief in den Timeout");
    }

    #[tokio::test]
    async fn bootstrap_ohne_bekannte_peers_wird_abgelehnt() {
        let identity = NodeIdentity::generate();
        let config = NetConfig::default();
        let mut swarm = build_swarm(&identity, &config).expect("Swarm");
        // Ohne Bootstrap-Peers ist die Routing-Tabelle leer.
        assert!(matches!(
            start_bootstrap(&mut swarm),
            Err(DiscoveryError::NoKnownPeers)
        ));
        // Eine leere Bootstrap-Liste ist für bootstrap_from_config aber ok.
        assert_eq!(bootstrap_from_config(&mut swarm, &config).expect("leere Liste"), 0);
    }
}
