//! NAT-Überwindung über ein Relais (Punkt 3.4).
//!
//! # Was sich hier prüfen lässt und was nicht
//!
//! Ein echtes NAT lässt sich im Test nicht herstellen. Was sich
//! herstellen lässt, ist die **Wirkung**, auf die es ankommt: ein
//! Knoten, der **keine direkt wählbare Adresse hat**.
//!
//! Genau das tut `Node::hinter_nat`: Er horcht auf keiner eigenen
//! Adresse, sondern **nur** auf `…/p2p-circuit` eines Relais. Damit ist
//! er für Dritte auf demselben Weg unerreichbar wie hinter einem
//! Heimrouter, und der Relais-Pfad ist die einzige Möglichkeit, ihn zu
//! erreichen. Scheitert er, scheitert der Test.
//!
//! **Nicht geprüft ist das Lochstanzen selbst.** DCUtR braucht zwei
//! echte NATs, die eine Zuordnung halten; auf einer Maschine mit
//! Loopback gibt es nichts zu durchstoßen, und ein Test, der hier grün
//! wird, würde über die Wirklichkeit nichts aussagen. Das ist der
//! Grund, warum QUIC im Stack ist: Über TCP scheitert Lochstanzen an
//! vielen NAT-Bauarten, und **das zeigt sich erst auf getrennten
//! Maschinen**. Vermerkt als das, was der erste echte
//! Mehrmaschinenlauf zu messen hat.

use std::time::Duration;

use myl_net::{
    build_swarm, ist_vermittelt, relais_horchadresse, run_node, subscribe_all, GossipTopic,
    NatKonfig, NetConfig, NodeCommand, NodeEvent, NodeIdentity,
};
use tokio::sync::{mpsc, oneshot};

mod gemeinsam;

/// Wie lange ein Knoten auf seine erste Horchadresse warten darf, wenn
/// sie **kommen soll**.
///
/// ⚑ **Grosszuegig, und das ist Absicht.** Auf dem Positivpfad kann ein
/// zu langes Warten nichts falsch bestaetigen, es macht den Lauf nur
/// langsamer; ein zu kurzes dagegen laesst einen richtigen Lauf
/// scheitern, sobald die Maschine ausgelastet ist. Genau das ist am
/// 2026-08-31 einmal passiert: `nat.rs` fiel waehrend zweier paralleler
/// Uebersetzungslaeufe aus und war in zwanzig Wiederholungen danach
/// nicht wieder einzufangen.
const FRIST_ERWARTET: Duration = Duration::from_secs(30);

/// Wie lange der Negativtest wartet, **bevor er das Ausbleiben
/// feststellt**.
///
/// Hier ist die Frist keine Geduld, sondern die Behauptung selbst: Nach
/// dieser Zeit ist keine Reservierung gekommen, und das ist das
/// Ergebnis. Sie gehoert deshalb kurz, und sie gehoert **getrennt** von
/// der oberen. Bis heute teilten sich beide Pfade eine einzige Frist von
/// fuenf Sekunden, und damit hing der Positivpfad an einer Zahl, die
/// fuer den Negativpfad bemessen war.
const FRIST_AUSBLEIBEN: Duration = Duration::from_secs(5);

struct Node {
    peer_id: libp2p::PeerId,
    commands: mpsc::UnboundedSender<NodeCommand>,
    events: mpsc::UnboundedReceiver<NodeEvent>,
    listen_addr: Option<libp2p::Multiaddr>,
}

impl Node {
    /// Ein normaler Knoten mit eigener, direkt wählbarer Adresse.
    async fn direkt(dient_als_relais: bool) -> Node {
        let config = NetConfig {
            nat: NatKonfig {
                dient_als_relais,
                relais: Vec::new(),
                // Im Test steht die Adresse erst nach dem Start fest
                // (ephemerer Port), sie wird unten nachgetragen.
                oeffentliche_adressen: Vec::new(),
            },
            ..NetConfig::default()
        };
        let node = Node::start(
            config,
            Some("/ip4/127.0.0.1/tcp/0".parse().unwrap()),
            FRIST_ERWARTET,
        )
        .await;
        if dient_als_relais {
            // ⚑ Fund 56: Ein Relais trägt seine bestätigten externen
            // Adressen in die Reservierungsantwort ein. Ohne diesen
            // Eintrag nimmt es Reservierungen an und antwortet ohne
            // Adresse, und der Klient bekommt `NoAddressesInReservation`.
            node.commands
                .send(NodeCommand::ExterneAdresse { addr: node.adresse() })
                .expect("Kommando");
            // ⚑ **Ein blindes Warten, und es bleibt eines.** `Dial`,
            // `Publish` und `PeerCount` tragen einen Rueckkanal
            // (`result: Some(tx)`), `ExterneAdresse` nicht: Es gibt
            // nichts, worauf sich hier warten liesse ausser der Uhr. Auf
            // einer ausgelasteten Maschine kann die Laufzeitschleife in
            // 200 ms noch nicht drangewesen sein, und dann reserviert der
            // naechste Knoten bei einem Relais, das seine Adresse noch
            // nicht kennt.
            //
            // Der saubere Weg waere ein Rueckkanal an der Marke, wie ihn
            // die drei anderen Kommandos haben. Das ist eine Aenderung am
            // Kommando-Typ und damit an `myl-net` selbst, nicht am Test;
            // vermerkt statt still gelassen.
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        node
    }

    /// Ein Knoten **ohne** eigene wählbare Adresse: erreichbar nur über
    /// das Relais. Die Nachbildung eines Anschlusses hinter NAT.
    async fn hinter_nat(relais: &str) -> Node {
        let config = NetConfig {
            nat: NatKonfig {
                dient_als_relais: false,
                relais: vec![relais.to_string()],
                oeffentliche_adressen: Vec::new(),
            },
            ..NetConfig::default()
        };
        let circuit = relais_horchadresse(relais).expect("Relais-Adresse");
        assert!(ist_vermittelt(&circuit), "Horchadresse ohne p2p-circuit");
        Node::start(config, Some(circuit), FRIST_ERWARTET).await
    }

    async fn start(
        config: NetConfig,
        horchen: Option<libp2p::Multiaddr>,
        frist_bis_zur_adresse: Duration,
    ) -> Node {
        let identity = NodeIdentity::generate();
        let peer_id = identity.peer_id();
        let mut swarm = build_swarm(&identity, &config).expect("Swarm");
        subscribe_all(&mut swarm).expect("Topics");
        let will_horchen = horchen.is_some();
        if let Some(addr) = horchen {
            swarm.listen_on(addr).expect("Listen");
        }
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_node(swarm, cmd_rx, ev_tx));

        let mut listen_addr = None;
        if will_horchen {
            // Auf die erste gemeldete Adresse warten. Bei einem Knoten
            // hinter NAT ist das die vermittelte, und sie erscheint erst,
            // wenn das Relais die Reservierung bestätigt hat.
            //
            // ⚑ **Die Frist kommt von aussen**, weil die beiden Pfade
            // Verschiedenes von ihr wollen: siehe FRIST_ERWARTET und
            // FRIST_AUSBLEIBEN.
            let frist = tokio::time::Instant::now() + frist_bis_zur_adresse;
            while tokio::time::Instant::now() < frist {
                let rest = frist.saturating_duration_since(tokio::time::Instant::now());
                match tokio::time::timeout(rest, ev_rx.recv()).await {
                    Ok(Some(NodeEvent::ListenAddr(a))) => {
                        listen_addr = Some(a.with_p2p(peer_id).expect("p2p"));
                        break;
                    }
                    Ok(Some(_)) => continue,
                    _ => break,
                }
            }
        }
        Node { peer_id, commands: cmd_tx, events: ev_rx, listen_addr }
    }

    fn adresse(&self) -> libp2p::Multiaddr {
        self.listen_addr.clone().unwrap_or_else(|| {
            panic!(
                "keine Horchadresse innerhalb von {:?} gemeldet: Entweder blieb die \
                 Reservierung aus, oder die Maschine war zu ausgelastet",
                FRIST_ERWARTET
            )
        })
    }

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


    async fn warte_auf_peers(&self, n: usize, frist: Duration) -> usize {
        gemeinsam::warte_auf_peers(&self.commands, n, frist).await
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
}

/// **Ein Knoten ohne eigene Adresse bekommt vom Relais eine.**
///
/// Das ist die Reservierung, und ohne sie ist alles Weitere sinnlos:
/// Ein Knoten hinter NAT hat sonst keine Adresse, die er anderen nennen
/// könnte.
#[tokio::test]
async fn ein_knoten_hinter_nat_bekommt_eine_vermittelte_adresse() {
    let relais = Node::direkt(true).await;
    let versteckt = Node::hinter_nat(&relais.adresse().to_string()).await;

    let addr = versteckt.adresse();
    assert!(
        ist_vermittelt(&addr),
        "die gemeldete Adresse führt nicht über das Relais: {addr}"
    );
    assert!(
        addr.to_string().contains(&versteckt.peer_id.to_string()),
        "die vermittelte Adresse nennt den Knoten nicht: {addr}"
    );
}

/// **Ein Dritter erreicht den Knoten hinter NAT über das Relais.**
///
/// Der eigentliche Beleg. Der versteckte Knoten horcht auf **keiner**
/// direkt wählbaren Adresse; gelingt die Verbindung trotzdem, kann sie
/// nur über die Vermittlung gegangen sein.
#[tokio::test]
async fn ein_dritter_erreicht_den_versteckten_knoten() {
    let relais = Node::direkt(true).await;
    let versteckt = Node::hinter_nat(&relais.adresse().to_string()).await;
    let dritter = Node::direkt(false).await;

    assert!(
        dritter.dial(versteckt.adresse()).await,
        "der Wählversuch über das Relais wurde nicht einmal begonnen"
    );

    // Der Dritte spricht danach mit zweien: dem Relais (nötig für den
    // Weg) und dem versteckten Knoten am anderen Ende.
    let peers = dritter.warte_auf_peers(2, Duration::from_secs(20)).await;
    assert_eq!(
        peers, 2,
        "über das Relais kam keine Verbindung zum versteckten Knoten zustande"
    );
}

/// **Über die vermittelte Verbindung fließt auch Protokollverkehr.**
///
/// Eine Verbindung, über die nichts ankommt, nützt nichts. Dieser Test
/// schließt die Kette bis zum Gossip.
#[tokio::test]
async fn gossip_laeuft_ueber_die_vermittelte_verbindung() {
    let relais = Node::direkt(true).await;
    let mut versteckt = Node::hinter_nat(&relais.adresse().to_string()).await;
    let dritter = Node::direkt(false).await;

    assert!(dritter.dial(versteckt.adresse()).await);
    dritter.warte_auf_peers(2, Duration::from_secs(20)).await;

    let nutzlast = b"durch das Relais".to_vec();
    let mut ankam = false;
    for _ in 0..10 {
        if !dritter.publish(GossipTopic::Blocks, nutzlast.clone()).await {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        }
        if versteckt.empfange(Duration::from_secs(3)).await.is_some() {
            ankam = true;
            break;
        }
    }
    assert!(ankam, "über die vermittelte Verbindung kam kein Gossip an");
}

/// **⚑ Fund 56: Ein Relais ohne eigene Adresse wird beim Prüfen
/// abgewiesen, nicht erst im Betrieb.**
///
/// Der Fehler, der diesen Test hervorgebracht hat, war stumm: Das
/// Relais nahm die Reservierung an, schickte eine Antwort ohne Adressen,
/// und der Klient meldete `NoAddressesInReservation`. Alles lief, nur
/// niemand kam an. Seitdem weist [`myl_net::nat_pruefen`] die
/// Konfiguration ab.
#[test]
fn fund_56_ein_relais_ohne_adresse_faellt_beim_pruefen_auf() {
    let ohne = NatKonfig {
        dient_als_relais: true,
        relais: Vec::new(),
        oeffentliche_adressen: Vec::new(),
    };
    assert!(
        myl_net::nat_pruefen(&ohne).is_err(),
        "ein Relais ohne eigene Adresse muss beim Prüfen auffallen"
    );

    let mit = NatKonfig {
        dient_als_relais: true,
        relais: Vec::new(),
        oeffentliche_adressen: vec!["/ip4/203.0.113.5/tcp/4150".to_string()],
    };
    myl_net::nat_pruefen(&mit).expect("mit Adresse gültig");
}

/// **Ohne erklärten Relais-Dienst vermittelt niemand.**
///
/// Die Gegenprobe zum Schalter. Wäre der Relais-Server immer an, wäre
/// jeder Knoten Zahlmeister für fremden Verkehr, und dieser Test wäre
/// grün, ohne dass jemand es entschieden hätte.
#[tokio::test]
async fn ohne_erklaerung_dient_ein_knoten_nicht_als_relais() {
    let kein_relais = Node::direkt(false).await;
    let config = NetConfig {
        nat: NatKonfig {
            dient_als_relais: false,
            relais: vec![kein_relais.adresse().to_string()],
            oeffentliche_adressen: Vec::new(),
        },
        ..NetConfig::default()
    };
    let circuit = relais_horchadresse(&kein_relais.adresse().to_string()).expect("Adresse");
    let versuch = Node::start(config, Some(circuit), FRIST_AUSBLEIBEN).await;

    assert!(
        versuch.listen_addr.is_none(),
        "ein Knoten ohne Relais-Dienst hat eine Reservierung bestätigt: {:?}",
        versuch.listen_addr
    );
}
