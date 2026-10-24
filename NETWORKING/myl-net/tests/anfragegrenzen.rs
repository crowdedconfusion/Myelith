//! Was eine fremde Gegenstelle ueber den Anfragekanal binden kann
//! (Funde 605 und 606, 2026-10-23).
//!
//! Die Gegenstelle hier ist kein Myelith-Knoten, sondern ein nackter
//! Swarm mit der Vorgabe von libp2p: 100 Stroeme je Verbindung. Sie
//! schickt mehr Anfragen, als ein ehrlicher Knoten je zugleich stellt,
//! und der gepruefte Knoten antwortet absichtlich nicht.
//!
//! 1. **Fund 605:** Es kommen hoechstens [`MAX_ANFRAGEN_ZUGLEICH`] an.
//!    Vorher kamen alle durch, je bis zu 4 MiB.
//! 2. **Fund 606:** Nach dem Ablauf haelt der Knoten keinen Antwortkanal
//!    mehr. Vorher blieb jeder unbeantwortete fuer immer liegen.

use std::time::Duration;

use futures::StreamExt;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::SwarmEvent;
use libp2p::StreamProtocol;
use myl_net::{
    build_swarm, run_node, subscribe_all, ByteCodec, NetConfig, NodeCommand, NodeEvent, NodeIdentity,
    ANFRAGE_PROTOKOLL, MAX_ANFRAGEN_ZUGLEICH,
};
use tokio::sync::{mpsc, oneshot};

/// Mehr als das Doppelte der Grenze, damit ein Durchrutschen auffaellt.
const ANGEFRAGT: usize = 40;

async fn offene_anfragen(kommandos: &mpsc::UnboundedSender<NodeCommand>) -> usize {
    let (tx, rx) = oneshot::channel();
    kommandos.send(NodeCommand::Zustand(tx)).expect("Kommandokanal");
    rx.await.expect("Zustand").offene_anfragen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eine_fremde_gegenstelle_bindet_hoechstens_die_grenze_und_nichts_bleibt_liegen() {
    // Der gepruefte Knoten.
    let identitaet = NodeIdentity::generate();
    let ziel = identitaet.peer_id();
    let mut swarm = build_swarm(&identitaet, &NetConfig::default()).expect("Swarm");
    subscribe_all(&mut swarm).expect("Topics");
    swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr")).expect("Listen");
    let (kommandos, cmd_rx) = mpsc::unbounded_channel();
    let (ev_tx, mut ereignisse) = mpsc::unbounded_channel();
    tokio::spawn(run_node(swarm, cmd_rx, ev_tx));
    let adresse = loop {
        if let NodeEvent::ListenAddr(a) = ereignisse.recv().await.expect("Ereigniskanal") {
            break a;
        }
    };

    // Die Gegenstelle: nur der Anfragekanal, mit der Vorgabe von libp2p.
    let mut angreifer = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(libp2p::tcp::Config::default(), libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .expect("Transport")
        .with_behaviour(|_| {
            request_response::Behaviour::with_codec(
                ByteCodec,
                [(StreamProtocol::new(ANFRAGE_PROTOKOLL), ProtocolSupport::Outbound)],
                request_response::Config::default().with_request_timeout(Duration::from_secs(30)),
            )
        })
        .expect("Verhalten")
        .build();
    angreifer.dial(adresse).expect("Waehlen");
    loop {
        if let SwarmEvent::ConnectionEstablished { .. } = angreifer.select_next_some().await {
            break;
        }
    }
    for i in 0..ANGEFRAGT {
        angreifer.behaviour_mut().send_request(&ziel, vec![i as u8; 64]);
    }
    tokio::spawn(async move {
        loop {
            angreifer.select_next_some().await;
        }
    });

    // 1. Was ankommt, solange niemand antwortet.
    let mut angekommen = 0usize;
    let ende = tokio::time::Instant::now() + Duration::from_secs(5);
    while let Ok(Some(e)) = tokio::time::timeout_at(ende, ereignisse.recv()).await {
        if let NodeEvent::AnfrageEingegangen { .. } = e {
            angekommen += 1;
        }
    }
    assert!(angekommen > 0, "keine Anfrage angekommen, der Aufbau prueft nichts");
    assert!(
        angekommen <= MAX_ANFRAGEN_ZUGLEICH,
        "{angekommen} Anfragen zugleich angekommen, erlaubt {MAX_ANFRAGEN_ZUGLEICH}"
    );
    let offen = offene_anfragen(&kommandos).await;
    assert_eq!(offen, angekommen, "jede angekommene Anfrage haelt genau einen Antwortkanal");

    // 2. Nach dem Ablauf (Vorgabe von libp2p: 10 s) bleibt nichts liegen.
    let ende = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut offen = offen;
    while offen > 0 && tokio::time::Instant::now() < ende {
        tokio::time::sleep(Duration::from_millis(200)).await;
        offen = offene_anfragen(&kommandos).await;
    }
    assert_eq!(offen, 0, "{offen} Antwortkanaele nach dem Ablauf noch gehalten");
}
