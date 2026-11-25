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
    ANFRAGEFRIST, ANFRAGE_PROTOKOLL, MAX_ANFRAGEN_ZUGLEICH,
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
                request_response::Config::default().with_request_timeout(ANFRAGEFRIST * 2),
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

    // 2. Nach dem Ablauf ([`ANFRAGEFRIST`], Fund 722) bleibt nichts liegen.
    let ende = tokio::time::Instant::now() + ANFRAGEFRIST + Duration::from_secs(10);
    let mut offen = offen;
    while offen > 0 && tokio::time::Instant::now() < ende {
        tokio::time::sleep(Duration::from_millis(200)).await;
        offen = offene_anfragen(&kommandos).await;
    }
    assert_eq!(offen, 0, "{offen} Antwortkanaele nach dem Ablauf noch gehalten");
}

/// ⚑ **Über der Grenze wird gewartet, und ein Stau wird mit seinem Bild
/// gemeldet** (Simulation vom 2026-11-20 und Kurzlauf vom 2026-11-23): Der
/// gepruefte Knoten fragt eine Gegenstelle, die annimmt und nie antwortet.
/// Die ersten [`MAX_ANFRAGEN_ZUGLEICH`] halten die Plaetze; vier weitere ohne
/// Zweck warten in der Schlange und scheitern nicht. Eine Anfrage mit Zweck
/// (die Zeitprobe) wartet nie: Sie scheitert am Stau, und dazu kommt genau
/// ein [`NodeEvent::Stau`], das die belegenden Anfragen nach ihrer Art nennt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ein_stau_nennt_was_die_verbindung_belegt() {
    let identitaet = NodeIdentity::generate();
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

    // Die schweigende Gegenstelle: nimmt an, haelt jeden Antwortkanal und
    // antwortet nie (ein fallengelassener Kanal schloesse den Strom).
    let mut stumm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(libp2p::tcp::Config::default(), libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .expect("Transport")
        .with_behaviour(|_| {
            request_response::Behaviour::<ByteCodec>::with_codec(
                ByteCodec,
                [(StreamProtocol::new(ANFRAGE_PROTOKOLL), ProtocolSupport::Inbound)],
                request_response::Config::default().with_request_timeout(Duration::from_secs(60)),
            )
        })
        .expect("Verhalten")
        .build();
    let stumm_id = *stumm.local_peer_id();
    stumm.dial(adresse).expect("Waehlen");
    tokio::spawn(async move {
        let mut gehalten = Vec::new();
        loop {
            if let SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Request { channel, .. },
                ..
            }) = stumm.select_next_some().await
            {
                gehalten.push(channel);
            }
        }
    });
    loop {
        if let NodeEvent::Verbunden { peer, .. } = ereignisse.recv().await.expect("Ereigniskanal") {
            if peer == stumm_id {
                break;
            }
        }
    }

    // Die Plaetze mit Art 9 belegen, dann 600 der Art 11 hinterher (eine
    // Salve der Stichprobe, mehr als die alte Grenze von 256, Fund 720) und
    // zwei mit Vorrang.
    for _ in 0..MAX_ANFRAGEN_ZUGLEICH {
        kommandos.send(NodeCommand::Anfrage { an: stumm_id, daten: vec![9, 1, 2, 3] }).expect("Kommando");
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    for _ in 0..600 {
        kommandos.send(NodeCommand::Anfrage { an: stumm_id, daten: vec![11; 45] }).expect("Kommando");
    }
    for _ in 0..2 {
        kommandos.send(NodeCommand::AnfrageVorrang { an: stumm_id, daten: vec![9, 9], zweck: 1 << 63 }).expect("Kommando");
    }

    let mut staus = Vec::new();
    let mut gescheitert = 0usize;
    let ende = tokio::time::Instant::now() + Duration::from_secs(2);
    while let Ok(Some(e)) = tokio::time::timeout_at(ende, ereignisse.recv()).await {
        match e {
            NodeEvent::Stau { an, offen, gesamt } if an == stumm_id => staus.push((offen, gesamt)),
            NodeEvent::AnfrageGescheitert { an, .. } if an == stumm_id => gescheitert += 1,
            _ => {}
        }
    }
    assert_eq!(gescheitert, 0, "die 602 ohne Zweck warten, statt zu scheitern");
    assert!(staus.is_empty(), "ohne gescheiterte Anfrage kein Bild: {staus:?}");

    kommandos.send(NodeCommand::AnfrageMitZweck { an: stumm_id, daten: vec![2, 0], zweck: 1 }).expect("Kommando");
    let mut mit_zweck = 0usize;
    let ende = tokio::time::Instant::now() + Duration::from_secs(3);
    while let Ok(Some(e)) = tokio::time::timeout_at(ende, ereignisse.recv()).await {
        match e {
            NodeEvent::Stau { an, offen, gesamt } if an == stumm_id => staus.push((offen, gesamt)),
            NodeEvent::AnfrageGescheitert { an, grund, zweck } if an == stumm_id => {
                assert!(grund.ends_with("max sub-streams reached"), "{grund}");
                assert_eq!(zweck, Some(1));
                mit_zweck += 1;
            }
            _ => {}
        }
    }
    assert_eq!(mit_zweck, 1, "die Anfrage mit Zweck wartet nicht");
    assert_eq!(staus.len(), 1, "ein Bild je Meldeabstand: {staus:?}");
    let (offen, gesamt) = &staus[0];
    let belegt = offen.iter().find(|o| o.art == 9).map(|o| o.anzahl);
    assert_eq!(belegt, Some(MAX_ANFRAGEN_ZUGLEICH as u32), "die Plaetze halten die Anfragen der Art 9: {offen:?}");
    assert_eq!(*gesamt, MAX_ANFRAGEN_ZUGLEICH, "die wartenden zaehlen nicht als unterwegs");
    let wartend: Vec<(u8, u32)> = offen.iter().map(|o| (o.art, o.wartend)).collect();
    assert_eq!(wartend, vec![(9, 2), (11, 600)], "alle warten, die mit Vorrang mitgezaehlt: {offen:?}");
}
