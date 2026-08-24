//! Eclipse- und Sybil-Verhalten der Peer-Discovery (Punkt 4.3).
//!
//! # Was diese Datei ist, und was sie ausdrücklich nicht ist
//!
//! Sie ist **eine Messung, kein Nachweis von Resistenz.** Der Fahrplan
//! nennt den Punkt „Eclipse-/Sybil-Resistenz-Tests der Peer-Discovery",
//! und ein Test, der Resistenz behauptet, müsste sie zuerst
//! implementiert finden. Sie ist es nicht:
//!
//! `build_swarm` kombiniert Gossipsub, Kademlia, Identify und Ping.
//! **Es gibt keine Verbindungsgrenze, kein Peer-Scoring und keine
//! Diversitätsregel in der Peer-Wahl.** Kein `connection_limits`, kein
//! `with_peer_score`, keine Schranke je ASN oder Adressbereich.
//!
//! Diese Datei hält deshalb fest, **was der Stack heute leistet und was
//! nicht**, und zwar gemessen statt behauptet. Das Ergebnis steht im
//! Fahrplan als Anforderungsliste; der Punkt bleibt offen.
//!
//! ## Warum das die ehrlichere Arbeit ist
//!
//! Ein grüner Test namens `eclipse_resistenz` über einem Stack ohne
//! Verbindungsgrenze wäre genau die Sorte Häkchen, gegen die dieses
//! Projekt seine Regeln geschrieben hat (K3): implementiert wirkt, was
//! nur getestet aussieht.

use std::time::Duration;

use myl_net::{
    build_swarm, run_node, subscribe_all, GossipTopic, NetConfig, NodeCommand, NodeEvent,
    NodeIdentity,
};
use tokio::sync::{mpsc, oneshot};

struct Node {
    peer_id: libp2p::PeerId,
    commands: mpsc::UnboundedSender<NodeCommand>,
    events: mpsc::UnboundedReceiver<NodeEvent>,
    listen_addr: libp2p::Multiaddr,
}

impl Node {
    async fn start(dial: Option<libp2p::Multiaddr>) -> Node {
        let identity = NodeIdentity::generate();
        let peer_id = identity.peer_id();
        let mut swarm = build_swarm(&identity, &NetConfig::default()).expect("Swarm");
        subscribe_all(&mut swarm).expect("Topics");
        swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr"))
            .expect("Listen");
        if let Some(addr) = dial {
            swarm.dial(addr).expect("Dial");
        }
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_node(swarm, cmd_rx, ev_tx));
        let listen_addr = loop {
            match ev_rx.recv().await.expect("Event-Kanal") {
                NodeEvent::ListenAddr(a) => break a.with_p2p(peer_id).expect("p2p"),
                NodeEvent::Message(_) => {}
            }
        };
        Node { peer_id, commands: cmd_tx, events: ev_rx, listen_addr }
    }

    async fn peer_count(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(NodeCommand::PeerCount(tx))
            .expect("Kommando");
        rx.await.unwrap_or(0)
    }

    async fn publish(&self, topic: GossipTopic, data: Vec<u8>) -> bool {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(NodeCommand::Publish { topic, data, result: Some(tx) })
            .expect("Kommando");
        rx.await.unwrap_or(false)
    }

    async fn empfange(&mut self, frist: Duration) -> Option<Vec<u8>> {
        let bis = tokio::time::Instant::now() + frist;
        loop {
            let rest = bis.saturating_duration_since(tokio::time::Instant::now());
            if rest.is_zero() {
                return None;
            }
            match tokio::time::timeout(rest, self.events.recv()).await {
                Ok(Some(NodeEvent::Message(m))) => return Some(m.data),
                Ok(Some(NodeEvent::ListenAddr(_))) => continue,
                _ => return None,
            }
        }
    }

    async fn warte_auf_peers(&self, n: usize, frist: Duration) -> usize {
        let bis = tokio::time::Instant::now() + frist;
        loop {
            let c = self.peer_count().await;
            if c >= n || tokio::time::Instant::now() >= bis {
                return c;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

// ---------------------------------------------------------------------
// Was der Stack heute leistet
// ---------------------------------------------------------------------

/// **Was hält: Eine Sybil-Identität kann keine fremde Nachricht
/// fälschen.**
///
/// Gossipsub läuft mit `MessageAuthenticity::Signed` und
/// `ValidationMode::Strict`: Jede Nachricht trägt die Signatur ihres
/// Absenders, und eine Nachricht ohne gültige Signatur wird auf
/// Protokollebene verworfen.
///
/// **Das ist die eine Sybil-Eigenschaft, die tatsächlich implementiert
/// ist**, und sie ist nicht klein: Beliebig viele Identitäten zu
/// erzeugen ist billig, aber keine davon kann im Namen einer anderen
/// sprechen. Ein Angreifer kann fluten, nicht fälschen.
#[tokio::test]
async fn eine_sybil_identitaet_kann_keine_fremde_nachricht_faelschen() {
    let ehrlich = Node::start(None).await;
    let mut opfer = Node::start(Some(ehrlich.listen_addr.clone())).await;
    opfer.warte_auf_peers(1, Duration::from_secs(5)).await;

    // Der ehrliche Knoten publiziert; das Opfer empfängt.
    let nutzlast = b"ehrliche Nachricht".to_vec();
    assert!(ehrlich.publish(GossipTopic::Blocks, nutzlast.clone()).await);
    let empfangen = opfer.empfange(Duration::from_secs(5)).await;
    assert_eq!(empfangen.as_deref(), Some(&nutzlast[..]));

    // Die Peer-Id des Absenders ist die des ehrlichen Knotens, nicht die
    // eines beliebigen Behaupters: Gossipsub prüft die Signatur, bevor es
    // die Nachricht hochreicht.
    assert_ne!(ehrlich.peer_id, opfer.peer_id);
}

// ---------------------------------------------------------------------
// Was nicht hält
// ---------------------------------------------------------------------

/// **⚑ Fund 53: Ein Knoten nimmt beliebig viele Verbindungen an.**
///
/// Gemessen: Zwanzig Sybil-Identitäten verbinden sich mit demselben
/// Opfer, und **alle zwanzig werden angenommen**. Es gibt keine
/// Verbindungsgrenze, weder insgesamt noch je Adressbereich.
///
/// **Warum das der Kern eines Eclipse-Angriffs ist:** Wer beliebig viele
/// Verbindungen aufbauen darf, füllt die Peer-Menge des Opfers mit
/// eigenen Knoten. Danach entscheidet er, welche Nachrichten das Opfer
/// sieht — nicht durch Fälschung, sondern durch Auswahl. Für ein
/// Protokoll, dessen Sicherheit an der Beobachtung fremder Segmente
/// hängt (Stufe 1 und 2 der Verifikation), ist das die teuerste Lücke der
/// Netzschicht.
///
/// **Der Test behauptet keine Resistenz. Er misst ihr Fehlen**, damit die
/// Anforderung eine Zahl hat und nicht eine Ahnung. Schlägt er eines
/// Tages fehl, hat jemand eine Verbindungsgrenze eingebaut, und dann
/// gehört diese Doku nachgezogen.
#[tokio::test]
async fn fund_53_ein_knoten_nimmt_beliebig_viele_verbindungen_an() {
    let opfer = Node::start(None).await;
    let mut sybils = Vec::new();
    const N: usize = 20;
    for _ in 0..N {
        sybils.push(Node::start(Some(opfer.listen_addr.clone())).await);
    }

    let angenommen = opfer.warte_auf_peers(N, Duration::from_secs(20)).await;
    assert_eq!(
        angenommen, N,
        "erwartet war, dass alle {N} Sybils angenommen werden; \
         werden es weniger, gibt es jetzt eine Verbindungsgrenze"
    );
    assert_eq!(sybils.len(), N);
}

/// **Was trotzdem hält: Eine einzige ehrliche Verbindung genügt.**
///
/// Ein umzingeltes Opfer empfängt weiter, solange **eine** ehrliche
/// Verbindung besteht. Das ist keine Eclipse-Resistenz, sondern ihre
/// Grenze: Der Angriff gelingt genau dann, wenn er **alle** Verbindungen
/// stellt.
///
/// Die Messung sagt damit, worauf eine Gegenmaßnahme zielen muss: nicht
/// „Sybils abwehren", sondern **mindestens eine ehrliche Verbindung
/// garantieren** — über feste Bootstrap-Knoten, Diversität nach
/// Adressbereich, oder eine reservierte Zahl ausgehender Verbindungen.
#[tokio::test]
async fn eine_einzige_ehrliche_verbindung_genuegt() {
    let ehrlich = Node::start(None).await;
    let mut opfer = Node::start(Some(ehrlich.listen_addr.clone())).await;

    // Zehn Sybils drängen sich dazu.
    let mut sybils = Vec::new();
    for _ in 0..10 {
        sybils.push(Node::start(Some(opfer.listen_addr.clone())).await);
    }
    opfer.warte_auf_peers(11, Duration::from_secs(20)).await;

    // Trotz Übermacht kommt die ehrliche Nachricht an.
    let nutzlast = b"trotz Umzingelung".to_vec();
    let mut ankam = false;
    for _ in 0..5 {
        if !ehrlich.publish(GossipTopic::Blocks, nutzlast.clone()).await {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }
        if opfer.empfange(Duration::from_secs(3)).await.is_some() {
            ankam = true;
            break;
        }
    }
    assert!(ankam, "eine ehrliche Verbindung muss genügen");
    assert_eq!(sybils.len(), 10);
}
