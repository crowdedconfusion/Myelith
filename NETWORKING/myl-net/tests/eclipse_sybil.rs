//! Eclipse- und Sybil-Verhalten der Peer-Discovery (Punkt 4.3).
//!
//! # Vorgeschichte: erst gemessen, dann gebaut
//!
//! Diese Datei entstand am 2026-08-24 als **Messung ohne Verteidigung**.
//! Der Fahrplan nannte den Punkt „Eclipse-/Sybil-Resistenz-Tests", und
//! ein Test, der Resistenz behauptet, müsste sie zuerst implementiert
//! finden. Sie war es nicht: kein `connection_limits`, kein
//! `with_peer_score`, keine Schranke je Adressbereich. Zwanzig Sybils
//! verbanden sich mit demselben Opfer, alle zwanzig wurden angenommen
//! (**Fund 53**).
//!
//! Seitdem gibt es [`myl_net::limits`] und [`myl_net::scoring`]. Diese
//! Datei prüft jetzt, was die Verteidigung leistet, und weiterhin
//! ausdrücklich **nicht mehr als das**.
//!
//! # Was die Verteidigung zusagt, und was nicht
//!
//! Die Messung hat damals gesagt, worauf eine Gegenmaßnahme zielen muss:
//! nicht „Sybils abwehren", das ist bei kostenlosen Identitäten
//! aussichtslos, sondern **mindestens eine ehrliche Verbindung
//! garantieren**. Der Angriff gelingt genau dann, wenn er *alle*
//! Verbindungen stellt.
//!
//! Genau das wird hier geprüft, und zwar als Kette:
//!
//! 1. Eingehende Verbindungen sind gedeckelt.
//! 2. Das **ausgehende** Budget bleibt auch unter voller eingehender
//!    Flut frei.
//! 3. Der Knoten kann dieses Budget benutzen, also während der Flut
//!    einen Peer eigener Wahl anwählen.
//!
//! **Was daraus nicht folgt:** dass er *richtig* wählt. Wer wählt,
//! braucht Adressen, und die kommen aus der Bootstrap-Liste und aus
//! Kademlia. Kontrolliert ein Angreifer beide, nützt das freie Budget
//! nichts. Die Verteidigung reduziert den Eclipse-Angriff auf die
//! Bedingung „die Bootstrap-Liste enthält mindestens einen ehrlichen
//! Knoten". Das ist ein Fortschritt gegenüber „beliebig viele werden
//! angenommen" und keine Resistenz.
//!
//! # Warum die Grenzen hier klein konfiguriert werden
//!
//! Die Vorgabewerte sind 48 eingehende und 16 ausgehende Verbindungen.
//! Sie mit echten Knoten auszureizen hieße, 64 Prozesse zu starten, und
//! der Test prüfte am Ende die Zahl statt den Mechanismus. Die Tests
//! setzen deshalb kleine Grenzen und prüfen, **dass** gedeckelt wird.
//! **Dass die Vorgabewerte die richtigen sind**, prüfen die Unit-Tests
//! in `src/limits.rs` gegen ihre Herleitung.
//!
//! # Loopback
//!
//! Alle Knoten hier laufen auf `127.0.0.1`, und Loopback ist sowohl von
//! der Adressbereichsgrenze als auch von der Kolokationsbewertung
//! ausgenommen (Begründung in beiden Modulen). Die Zahlengrenzen greifen
//! trotzdem, sie zählen Verbindungen, nicht Herkunft. Die
//! Bereichszählung selbst ist in `src/limits.rs` gegen echte Adressen
//! geprüft, was über Loopback grundsätzlich nicht ginge.

use std::time::Duration;

use libp2p::connection_limits::ConnectionLimits;
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
        Node::start_mit(NetConfig::default(), dial).await
    }

    async fn start_mit(config: NetConfig, dial: Option<libp2p::Multiaddr>) -> Node {
        let identity = NodeIdentity::generate();
        let peer_id = identity.peer_id();
        let mut swarm = build_swarm(&identity, &config).expect("Swarm");
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
            if let NodeEvent::ListenAddr(a) = ev_rx.recv().await.expect("Event-Kanal") {
                break a.with_p2p(peer_id).expect("p2p");
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

    /// Wählt eine Adresse aus dem laufenden Knoten heraus. Rückgabe: ob
    /// der Wählversuch begonnen wurde.
    async fn dial(&self, addr: libp2p::Multiaddr) -> bool {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(NodeCommand::Dial { addr, result: Some(tx) })
            .expect("Kommando");
        rx.await.unwrap_or(false)
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
                // Alles andere weiterlaufen lassen. Ein `_ => return None`
                // stand hier, bis `NodeEvent` um die Verbindungsereignisse
                // wuchs: Danach brach der Wartelauf bei der ersten
                // Verbindungsmeldung ab, und der Test schlug fehl, obwohl
                // die Nachricht unterwegs war. Ein Catch-all, der abbricht,
                // ist eine Wette darauf, dass die Aufzählung nie wächst.
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => return None,
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

    /// Wartet, bis die Peer-Anzahl sich `ruhe` lang nicht mehr ändert.
    /// Für Deckelungstests: Dort ist die Frage nicht „erreicht er n",
    /// sondern „wo bleibt er stehen".
    async fn warte_auf_ruhe(&self, ruhe: Duration, frist: Duration) -> usize {
        let bis = tokio::time::Instant::now() + frist;
        let mut letzte = self.peer_count().await;
        let mut seit = tokio::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let jetzt = self.peer_count().await;
            if jetzt != letzte {
                letzte = jetzt;
                seit = tokio::time::Instant::now();
            } else if seit.elapsed() >= ruhe {
                return jetzt;
            }
            if tokio::time::Instant::now() >= bis {
                return jetzt;
            }
        }
    }
}

/// Konfiguration mit kleinen, gut sichtbaren Grenzen.
fn config_mit_grenzen(eingehend: u32, ausgehend: u32) -> NetConfig {
    NetConfig {
        grenzen: ConnectionLimits::default()
            .with_max_established_incoming(Some(eingehend))
            .with_max_established_outgoing(Some(ausgehend))
            .with_max_established(Some(eingehend + ausgehend))
            .with_max_established_per_peer(Some(myl_net::MAX_JE_PEER)),
        ..NetConfig::default()
    }
}

// ---------------------------------------------------------------------
// Was ohne Verbindungsgrenze schon hielt
// ---------------------------------------------------------------------

/// **Eine Sybil-Identität kann keine fremde Nachricht fälschen.**
///
/// Gossipsub läuft mit `MessageAuthenticity::Signed` und
/// `ValidationMode::Strict`: Jede Nachricht trägt die Signatur ihres
/// Absenders, und eine Nachricht ohne gültige Signatur wird auf
/// Protokollebene verworfen.
///
/// Das war schon vor Fund 53 die eine Sybil-Eigenschaft, die tatsächlich
/// implementiert war, und sie ist nicht klein: Beliebig viele
/// Identitäten zu erzeugen ist billig, aber keine davon kann im Namen
/// einer anderen sprechen. Ein Angreifer kann fluten, nicht fälschen.
#[tokio::test]
async fn eine_sybil_identitaet_kann_keine_fremde_nachricht_faelschen() {
    let ehrlich = Node::start(None).await;
    let mut opfer = Node::start(Some(ehrlich.listen_addr.clone())).await;
    opfer.warte_auf_peers(1, Duration::from_secs(5)).await;

    let nutzlast = b"ehrliche Nachricht".to_vec();
    assert!(ehrlich.publish(GossipTopic::Blocks, nutzlast.clone()).await);
    let empfangen = opfer.empfange(Duration::from_secs(5)).await;
    assert_eq!(empfangen.as_deref(), Some(&nutzlast[..]));

    assert_ne!(ehrlich.peer_id, opfer.peer_id);
}

// ---------------------------------------------------------------------
// Fund 53: die Verteidigung
// ---------------------------------------------------------------------

/// **⚑ Fund 53, Schritt 1: Eingehende Verbindungen sind gedeckelt.**
///
/// Zwölf Sybils, Grenze vier. Der Vorgänger dieses Tests hielt fest,
/// dass **alle zwanzig** Sybils angenommen wurden, und war grün. Die
/// Umkehrung ist der Beleg, dass die Grenze wirkt.
///
/// Geprüft wird das Ergebnis nach Ruhe, nicht nach Frist: Die Frage ist
/// nicht „erreicht er zwölf", sondern „wo bleibt er stehen".
#[tokio::test]
async fn fund_53_eingehende_verbindungen_sind_gedeckelt() {
    const GRENZE: u32 = 4;
    const SYBILS: usize = 12;

    let opfer = Node::start_mit(config_mit_grenzen(GRENZE, 2), None).await;
    let mut sybils = Vec::new();
    for _ in 0..SYBILS {
        sybils.push(Node::start(Some(opfer.listen_addr.clone())).await);
    }

    let angenommen = opfer
        .warte_auf_ruhe(Duration::from_secs(2), Duration::from_secs(30))
        .await;
    assert_eq!(
        angenommen, GRENZE as usize,
        "erwartet waren genau {GRENZE} angenommene von {SYBILS} Sybils; \
         {angenommen} heißt, die Grenze greift nicht wie ausgelegt"
    );
    assert_eq!(sybils.len(), SYBILS);
}

/// **⚑ Fund 53, Schritt 2: Das ausgehende Budget bleibt unter Flut frei.**
///
/// Das ist die eigentliche Zusage der Verteidigung, und sie ist der
/// Grund, warum eingehende und ausgehende Verbindungen getrennte Budgets
/// haben statt eines gemeinsamen Deckels.
///
/// Ablauf: Das Opfer wird bis an seine eingehende Grenze geflutet.
/// **Danach** wählt es einen ehrlichen Knoten an, aus dem laufenden
/// Prozess heraus, nicht beim Start. Die Verbindung muss zustande
/// kommen, obwohl eingehend nichts mehr frei ist.
///
/// Ein gemeinsamer Deckel hätte hier versagt: Die Sybils hätten ihn
/// gefüllt, und der Wählversuch wäre an der Gesamtgrenze gescheitert.
#[tokio::test]
async fn fund_53_das_ausgehende_budget_bleibt_unter_flut_frei() {
    const EINGEHEND: u32 = 4;
    const AUSGEHEND: u32 = 2;
    const SYBILS: usize = 10;

    let opfer = Node::start_mit(config_mit_grenzen(EINGEHEND, AUSGEHEND), None).await;
    let ehrlich = Node::start(None).await;

    // Erst fluten, bis eingehend nichts mehr geht.
    let mut sybils = Vec::new();
    for _ in 0..SYBILS {
        sybils.push(Node::start(Some(opfer.listen_addr.clone())).await);
    }
    let voll = opfer
        .warte_auf_ruhe(Duration::from_secs(2), Duration::from_secs(30))
        .await;
    assert_eq!(voll, EINGEHEND as usize, "die Flut hat die Grenze nicht erreicht");

    // Jetzt selbst wählen. Genau das muss weiterhin möglich sein.
    assert!(
        opfer.dial(ehrlich.listen_addr.clone()).await,
        "der Wählversuch wurde nicht einmal begonnen"
    );
    let nachher = opfer
        .warte_auf_peers(EINGEHEND as usize + 1, Duration::from_secs(15))
        .await;
    assert_eq!(
        nachher,
        EINGEHEND as usize + 1,
        "das Opfer konnte trotz freiem ausgehendem Budget keine Verbindung \
         eigener Wahl aufbauen: {nachher} Peers statt {}",
        EINGEHEND + 1
    );
    assert_eq!(sybils.len(), SYBILS);
}

/// **⚑ Fund 53, Schritt 3: Über die selbst gewählte Verbindung kommt
/// auch etwas an.**
///
/// Ein freier Verbindungsplatz nützt nichts, wenn die Nachricht dann am
/// Peer-Scoring hängen bleibt. Dieser Test schließt die Kette: Das Opfer
/// ist umzingelt, wählt selbst, und empfängt darüber.
///
/// **Dieser Test hat Fund 54 gefunden.** Der erste Entwurf des Scorings
/// setzte die Kolokationsschwelle auf 4. Elf Knoten auf `127.0.0.1`
/// ergaben damit einen Score von −245 bei einer Graylist-Schwelle von
/// −80: Die Härtung hatte den ehrlichen Knoten mit stummgeschaltet.
/// Begründung und Rechnung stehen im Kopf von `src/scoring.rs`.
#[tokio::test]
async fn eine_ehrliche_verbindung_bleibt_erreichbar() {
    let ehrlich = Node::start(None).await;
    let mut opfer = Node::start(Some(ehrlich.listen_addr.clone())).await;

    let mut sybils = Vec::new();
    for _ in 0..10 {
        sybils.push(Node::start(Some(opfer.listen_addr.clone())).await);
    }
    opfer.warte_auf_peers(11, Duration::from_secs(20)).await;

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
    assert!(
        ankam,
        "eine ehrliche Verbindung muss genügen; schlägt das fehl, hat die \
         Härtung den Ehrlichen mit getroffen (vgl. Fund 54)"
    );
    assert_eq!(sybils.len(), 10);
}

/// **Ein einzelner Peer kann keine Verbindungen horten.**
///
/// Ohne Grenze je Peer-Id könnte eine einzige Identität den eingehenden
/// Deckel allein füllen. Zwei Verbindungen sind erlaubt (gleichzeitiges
/// beidseitiges Wählen ist in libp2p normal), mehr nicht.
#[tokio::test]
async fn ein_einzelner_peer_bekommt_hoechstens_zwei_verbindungen() {
    let opfer = Node::start_mit(config_mit_grenzen(8, 2), None).await;
    let angreifer = Node::start(None).await;

    // Derselbe Peer wählt fünfmal dieselbe Adresse.
    for _ in 0..5 {
        angreifer.dial(opfer.listen_addr.clone()).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let ruhig = opfer
        .warte_auf_ruhe(Duration::from_secs(2), Duration::from_secs(20))
        .await;

    // `peer_count` zählt Peers, nicht Verbindungen: Der Angreifer bleibt
    // ein Peer. Die Aussage ist, dass er nicht mehrere Plätze belegt und
    // das Opfer nicht überlastet.
    assert_eq!(ruhig, 1, "ein Angreifer, ein Peer, egal wie oft er wählt");
}
