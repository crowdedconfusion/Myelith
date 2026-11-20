//! Fuehler (Audit A9): Eine Gegenstelle, die sich **eingehend** meldet,
//! kommt erst in die Routingtabelle, wenn der Knoten sie selbst unter ihrer
//! genannten Adresse erreicht hat.
//!
//! Drei Knoten auf der Rueckschleife: das Opfer V, eine ehrliche Gegenstelle
//! H (horcht und waehlt V an) und eine luegende L (waehlt V an, horcht aber
//! nicht und nennt eine Adresse, unter der niemand antwortet). Vor A9 trug V
//! beide ein, denn jede ueber `identify` genannte Adresse ging in die
//! Tabelle.

use std::time::Duration;

use myl_net::{build_swarm, run_node, subscribe_all, NetConfig, NodeCommand, NodeEvent, NodeIdentity};
use tokio::sync::{mpsc, oneshot};

struct Knoten {
    peer: libp2p::PeerId,
    befehle: mpsc::UnboundedSender<NodeCommand>,
    ereignisse: mpsc::UnboundedReceiver<NodeEvent>,
}

async fn starte(horchen: bool, extern_: Option<libp2p::Multiaddr>, waehle: Option<libp2p::Multiaddr>) -> (Knoten, Option<libp2p::Multiaddr>) {
    let id = NodeIdentity::generate();
    let peer = id.peer_id();
    let mut swarm = build_swarm(&id, &NetConfig::default()).expect("Swarm");
    subscribe_all(&mut swarm).expect("Topics");
    if horchen {
        swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).expect("horchen");
    }
    if let Some(a) = extern_ {
        swarm.add_external_address(a);
    }
    if let Some(a) = waehle {
        swarm.dial(a).expect("waehlen");
    }
    let (btx, brx) = mpsc::unbounded_channel();
    let (etx, mut erx) = mpsc::unbounded_channel();
    tokio::spawn(run_node(swarm, brx, etx));
    let mut adresse = None;
    if horchen {
        loop {
            if let NodeEvent::ListenAddr(a) = erx.recv().await.expect("Ereignis") {
                adresse = Some(a.with_p2p(peer).unwrap());
                break;
            }
        }
    }
    (Knoten { peer, befehle: btx, ereignisse: erx }, adresse)
}

async fn tabelle(k: &Knoten) -> Vec<libp2p::PeerId> {
    let (tx, rx) = oneshot::channel();
    k.befehle.send(NodeCommand::Routingtabelle(tx)).unwrap();
    rx.await.unwrap()
}

/// Sammelt die Fuehlerergebnisse des Knotens, bis beide gemeldet sind oder
/// die Frist ablaeuft.
async fn fuehlerergebnisse(k: &mut Knoten, n: usize, frist: Duration) -> Vec<(libp2p::PeerId, bool, String)> {
    let bis = tokio::time::Instant::now() + frist;
    let mut aus = Vec::new();
    while aus.len() < n {
        let rest = bis.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(rest, k.ereignisse.recv()).await {
            Ok(Some(NodeEvent::Fuehler { peer, gelungen, grund })) => aus.push((peer, gelungen, grund)),
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    aus
}

#[tokio::test]
async fn nur_eine_erreichbare_eingehende_gegenstelle_kommt_in_die_tabelle() {
    let (mut v, v_adr) = starte(true, None, None).await;
    let v_adr = v_adr.unwrap();

    // Ein Port, an dem sicher niemand horcht: belegen und wieder freigeben.
    let tot = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let tot_port = tot.local_addr().unwrap().port();
    drop(tot);
    let luege: libp2p::Multiaddr = format!("/ip4/127.0.0.1/tcp/{tot_port}").parse().unwrap();

    let (h, _) = starte(true, None, Some(v_adr.clone())).await;
    let (l, _) = starte(false, Some(luege), Some(v_adr.clone())).await;

    let ergebnisse = fuehlerergebnisse(&mut v, 2, Duration::from_secs(20)).await;
    let von = |p| ergebnisse.iter().find(|(q, _, _)| *q == p).map(|(_, g, grund)| (*g, grund.clone()));
    assert_eq!(von(h.peer).map(|x| x.0), Some(true), "H gefuehlt: {ergebnisse:?}");
    assert_eq!(von(l.peer).map(|x| x.0), Some(false), "L gefuehlt: {ergebnisse:?}");

    let t = tabelle(&v).await;
    assert!(t.contains(&h.peer), "die erreichbare Gegenstelle steht in der Tabelle");
    assert!(!t.contains(&l.peer), "die luegende Gegenstelle steht nicht in der Tabelle");

    // Die Fuehlerverbindung ist wieder zu: V haelt zu H nur die eine,
    // eingehende Verbindung (L bleibt verbunden, aber ausserhalb der Tabelle).
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (tx, rx) = oneshot::channel();
    v.befehle.send(NodeCommand::PeerCount(tx)).unwrap();
    assert_eq!(rx.await.unwrap(), 2);
}

/// Eine ausgehend gewaehlte Gegenstelle braucht keinen Fuehler: Sie steht
/// sofort in der Tabelle, und es wird nicht gefuehlt.
#[tokio::test]
async fn eine_selbst_gewaehlte_gegenstelle_braucht_keinen_fuehler() {
    let (z, z_adr) = starte(true, None, None).await;
    let (mut w, _) = starte(true, None, Some(z_adr.unwrap())).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(10);
    while !tabelle(&w).await.contains(&z.peer) {
        assert!(tokio::time::Instant::now() < bis, "Z nie in der Tabelle von W");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let ergebnisse = fuehlerergebnisse(&mut w, 1, Duration::from_millis(1500)).await;
    assert!(ergebnisse.iter().all(|(p, _, _)| *p != z.peer), "W hat Z gefuehlt: {ergebnisse:?}");
}
