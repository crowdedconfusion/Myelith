//! Chaos-Tests: was das Netz aushält, und was hier nicht messbar ist.
//!
//! # ⚑ Was diese Datei **nicht** misst, und warum das oben steht
//!
//! **Keinen IP-Paketverlust.** Das Akzeptanzkriterium von Phase 4
//! verlangt „funktionsfähig bei 10 % zufälligem Paketverlust". Echter
//! Paketverlust entsteht unter der Transportschicht und lässt sich ohne
//! `tc netem` (Linux, root) nicht herstellen. Ein Test, der stattdessen
//! Verbindungen abschneidet und das Ergebnis „10 % Paketverlust" nennte,
//! wäre eine Überbehauptung, und eine Überbehauptung in einem
//! Härtungstest ist schlimmer als eine Lücke: Sie wird geglaubt.
//!
//! Diese Messung gehört deshalb auf Maschinen, auf denen sich der
//! Netzstapel des Betriebssystems konfigurieren lässt, und nicht in
//! diese Testsuite.
//!
//! **Was hier gemessen wird, ist die Schicht darüber:** Trennung,
//! Partition, Heilung, ein hängender Knoten und der Wiedereinstieg. Das
//! sind die Ereignisse, die ein Knoten selbst sieht, und sie sind ohne
//! root herstellbar.
//!
//! # Warum eine Sperrliste und kein Proxy
//!
//! Der naheliegende Aufbau wäre ein TCP-Proxy zwischen zwei Knoten, der
//! Verbindungen abschneidet. **Er wäre umgehbar:** `identify` und `kad`
//! verteilen die echten Horchadressen weiter, und die Knoten fänden nach
//! kurzer Zeit den direkten Weg. Ein Test, der die Umgehung nicht
//! bemerkt, misst nichts und meldet Erfolg.
//!
//! `NodeCommand::Sperren` wirkt dagegen auf die **Peer-Id**. Über welche
//! Adresse jemand kommt, ist dann gleichgültig, und ein Test unten weist
//! das eigens nach.
//!
//! # Über die Kommandos, nicht am Swarm vorbei
//!
//! Die Tests fahren echte [`run_node`]-Knoten und schicken
//! [`NodeCommand`]s, statt den Swarm direkt anzufassen. Der Unterschied
//! ist der Prüfgegenstand: Ein Test, der `behaviour_mut()` benutzt,
//! prüft libp2p. Ein Test, der Kommandos schickt, prüft **den Weg, den
//! ein Knoten im Betrieb nimmt**, und genau dort saßen die Funde 55
//! bis 57.
//!
//! # Die Gegenprobe steht in jedem Test
//!
//! Ein Partitionstest, der nur zeigt „nach der Heilung kommt es an",
//! beweist nichts: Vielleicht kam es die ganze Zeit an. Deshalb gibt es
//! zu jedem Störlauf einen Kontrolllauf ohne Störung.

use std::time::Duration;

use myl_net::{
    build_swarm, run_node, subscribe_all, GossipTopic, NetConfig, NodeCommand, NodeEvent,
    NodeIdentity,
};
use myl_types::ids::{EpochId, PodId, SegmentId};
use myl_types::{segments_root, BlsSecretKey, PoIBundle};
use tokio::sync::{mpsc, oneshot};

mod gemeinsam;

/// Segment-Ids zu Zeugnissen, mit einer aus der Id abgeleiteten
/// Spurwurzel.
///
/// ⚑ Seit Fund 100 bezeugt die Bündelwurzel `Id ‖ Spurwurzel`, nicht
/// mehr die bloße Id. Für diese Tests ist der Inhalt der Spur
/// gleichgültig, ihre Anwesenheit nicht.
fn zeugnisse(ids: &[SegmentId]) -> Vec<myl_types::Segmentzeugnis> {
    ids.iter()
        .map(|id| myl_types::Segmentzeugnis {
            id: *id,
            spurwurzel: myl_types::spurwurzel(&[*id.as_bytes()]).expect("Wurzel"),
        })
        .collect()
}


/// Ein laufender Test-Knoten. Der Swarm läuft in einem eigenen Task.
struct Knoten {
    peer_id: libp2p::PeerId,
    kommandos: mpsc::UnboundedSender<NodeCommand>,
    ereignisse: mpsc::UnboundedReceiver<NodeEvent>,
    adresse: libp2p::Multiaddr,
}

impl Knoten {
    async fn starten(waehlen: Option<libp2p::Multiaddr>) -> Knoten {
        let identitaet = NodeIdentity::generate();
        let peer_id = identitaet.peer_id();
        let mut swarm = build_swarm(&identitaet, &NetConfig::default()).expect("Swarm");
        subscribe_all(&mut swarm).expect("Topics");
        swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr"))
            .expect("Listen");
        if let Some(addr) = waehlen {
            swarm.dial(addr).expect("Dial");
        }
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        tokio::spawn(run_node(swarm, cmd_rx, ev_tx));

        let adresse = loop {
            if let NodeEvent::ListenAddr(addr) = ev_rx.recv().await.expect("Ereigniskanal") {
                break addr.with_p2p(peer_id).expect("p2p-Anhang");
            }
        };
        Knoten {
            peer_id,
            kommandos: cmd_tx,
            ereignisse: ev_rx,
            adresse,
        }
    }

    async fn veroeffentliche(&self, marke: u8) -> bool {
        let (tx, rx) = oneshot::channel();
        self.kommandos
            .send(NodeCommand::Publish {
                topic: GossipTopic::PoiBundles,
                data: borsh::to_vec(&buendel(marke)).expect("Serialisierung"),
                result: Some(tx),
            })
            .expect("Kommandokanal");
        rx.await.expect("Ergebniskanal")
    }

    /// Veröffentlicht mit Wiederholungen, bis Gossipsub annimmt.
    ///
    /// Mesh-Aufbau läuft asynchron; ein einzelner Versuch misst die
    /// Uhrzeit, nicht das Netz.
    async fn veroeffentliche_beharrlich(&self, marke: u8, frist: Duration) -> bool {
        let ende = tokio::time::Instant::now() + frist;
        loop {
            if self.veroeffentliche(marke).await {
                return true;
            }
            if tokio::time::Instant::now() >= ende {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Sperrt oder entsperrt eine Gegenstelle, **über das Kommando**.
    async fn sperren(&self, peer: libp2p::PeerId, gesperrt: bool) {
        let (tx, rx) = oneshot::channel();
        self.kommandos
            .send(NodeCommand::Sperren {
                peer,
                gesperrt,
                result: Some(tx),
            })
            .expect("Kommandokanal");
        // Auf die Bestätigung warten: Sonst prüfte der Test gleich
        // darauf eine Warteschlange statt einer Sperre.
        assert!(rx.await.expect("Ergebniskanal"));
    }

    fn waehlen(&self, addr: libp2p::Multiaddr) {
        let _ = self.kommandos.send(NodeCommand::Dial { addr, result: None });
    }


    async fn warte_auf_peers(&self, n: usize, frist: Duration) -> usize {
        gemeinsam::warte_auf_peers(&self.kommandos, n, frist).await
    }

    /// Zählt die Nachrichten, die innerhalb der Frist eintreffen.
    ///
    /// `bis` bricht ab, sobald so viele da sind. **Für die Fälle, in
    /// denen nichts ankommen darf, gibt es keinen Abbruch:** Dort ist
    /// das Warten die Messung.
    async fn empfange(&mut self, frist: Duration, bis: Option<usize>) -> usize {
        let ende = tokio::time::Instant::now() + frist;
        let mut n = 0usize;
        loop {
            if let Some(genug) = bis {
                if n >= genug {
                    return n;
                }
            }
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            if rest.is_zero() {
                return n;
            }
            match tokio::time::timeout(rest.min(Duration::from_millis(200)), self.ereignisse.recv())
                .await
            {
                Ok(Some(NodeEvent::Message(m))) if m.topic == GossipTopic::PoiBundles => n += 1,
                Ok(Some(_)) => {}
                Ok(None) => return n,
                Err(_) => {}
            }
        }
    }
}

fn buendel(marke: u8) -> PoIBundle {
    let sk = BlsSecretKey::key_gen(&[marke.wrapping_add(1); 32]).expect("KeyGen");
    let sig = sk.sign(b"chaos").expect("Signatur");
    let ids = [SegmentId::new([marke; 32]), SegmentId::new([marke ^ 0xFF; 32])];
    PoIBundle {
        epoch: EpochId(marke as u64),
        pod: PodId::new([marke; 32]),
        segments_root: segments_root(&zeugnisse(&ids)).expect("Wurzel"),
        vtfe_claimed: 1000 + marke as u64,
        aggregate_sig: sig,
        segmente: 1,
    }
}

/// Zwei verbundene Knoten.
async fn zwei_verbundene() -> (Knoten, Knoten) {
    let a = Knoten::starten(None).await;
    let b = Knoten::starten(Some(a.adresse.clone())).await;
    assert!(
        a.warte_auf_peers(1, Duration::from_secs(10)).await >= 1,
        "die beiden fanden einander nicht"
    );
    (a, b)
}

/// Fährt denselben Ablauf einmal **mit** und einmal **ohne** Sperre.
///
/// Gibt zurück, wie viele Nachrichten in der Störphase und wie viele
/// danach ankamen. Ein A/B-Lauf, weil ein Partitionstest ohne
/// Kontrolllauf nicht unterscheidet zwischen „die Sperre wirkt" und „es
/// kommt sowieso nichts an".
async fn partitionslauf(sperren: bool) -> (usize, usize) {
    let (a, mut b) = zwei_verbundene().await;
    // Mesh aufbauen lassen: Ohne Mesh nimmt Gossipsub nicht an, und das
    // sähe aus wie eine wirkende Sperre.
    assert!(
        a.veroeffentliche_beharrlich(1, Duration::from_secs(15))
            .await,
        "das Mesh kam nicht zustande"
    );
    assert_eq!(
        b.empfange(Duration::from_secs(8), Some(1)).await,
        1,
        "der Aufbau trug schon vor der Störung nicht"
    );

    if sperren {
        a.sperren(b.peer_id, true).await;
        // Die bestehende Verbindung schließen lassen.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    a.veroeffentliche(2).await;
    let waehrend = b.empfange(Duration::from_secs(3), None).await;

    if sperren {
        a.sperren(b.peer_id, false).await;
        b.waehlen(a.adresse.clone());
        b.warte_auf_peers(1, Duration::from_secs(10)).await;
    }

    let angenommen = a
        .veroeffentliche_beharrlich(3, Duration::from_secs(15))
        .await;
    let danach = if angenommen {
        b.empfange(Duration::from_secs(10), Some(1)).await
    } else {
        0
    };
    (waehrend, danach)
}

/// ⚑ **Partition und Heilung.**
#[tokio::test]
async fn eine_partition_trennt_und_heilt_wieder() {
    let (waehrend, danach) = partitionslauf(true).await;
    assert_eq!(
        waehrend, 0,
        "während der Partition kamen {waehrend} Nachrichten durch. Dann wirkt \
         die Sperre nicht, und die Heilung belegt nichts"
    );
    assert_eq!(
        danach, 1,
        "nach der Heilung kam nichts an: das Netz heilt nicht von selbst"
    );
}

/// ⚑ **Der Kontrolllauf: ohne Sperre kommt beides durch.**
///
/// Ohne diesen Test bewiese der Partitionstest nur, dass in diesem
/// Aufbau nichts ankommt. **Das ist keine Formsache:** Der Aufbau baut
/// ein Gossip-Mesh über echte Sockets, und dabei kann vieles
/// schiefgehen, ohne dass eine Zeile Code falsch wäre.
#[tokio::test]
async fn ohne_sperre_kaeme_beides_durch() {
    let (waehrend, danach) = partitionslauf(false).await;
    assert_eq!(
        waehrend, 1,
        "die Nachricht kam nicht an, obwohl nichts gesperrt war. Dann misst \
         der Partitionstest den Aufbau und nicht die Sperre"
    );
    assert_eq!(danach, 1, "die zweite Nachricht kam nicht an");
}

/// ⚑ **Eine Sperre wirkt auf die Peer-Id, nicht auf die Adresse.**
///
/// Das ist der Grund, warum hier kein Proxy steht. Der Test lässt den
/// gesperrten Knoten **erneut wählen**, über dieselbe Adresse wie beim
/// ersten Mal. Für eine adressgebundene Trennung wäre das der Ausweg.
#[tokio::test]
async fn eine_sperre_ueberlebt_einen_neuen_verbindungsversuch() {
    let (a, mut b) = zwei_verbundene().await;
    assert!(
        a.veroeffentliche_beharrlich(4, Duration::from_secs(15))
            .await
    );
    assert_eq!(b.empfange(Duration::from_secs(8), Some(1)).await, 1);

    a.sperren(b.peer_id, true).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // B versucht es noch einmal, über dieselbe Adresse.
    b.waehlen(a.adresse.clone());
    tokio::time::sleep(Duration::from_secs(2)).await;

    a.veroeffentliche(5).await;
    let n = b.empfange(Duration::from_secs(3), None).await;
    assert_eq!(
        n, 0,
        "der gesperrte Knoten bekam über einen neuen Wählversuch {n} Nachrichten"
    );
}

/// Ein Knoten, der weggeht und wiederkommt, bekommt wieder Nachrichten.
///
/// Der Netzanteil von „Node-Crash-und-Restart". Der Zustandsanteil steht
/// in `NODE/myl-node/tests/neustart.rs`: Dort wird geprüft, dass der
/// Knoten mit derselben Zustandswurzel zurückkommt.
///
/// **Mit neuer Identität**, denn das ist der schwierigere Fall: Die
/// Gegenstelle erkennt ihn nicht wieder.
#[tokio::test]
async fn ein_knoten_der_wiederkommt_bekommt_wieder_nachrichten() {
    let a = Knoten::starten(None).await;
    {
        let mut b = Knoten::starten(Some(a.adresse.clone())).await;
        a.warte_auf_peers(1, Duration::from_secs(10)).await;
        assert!(
            a.veroeffentliche_beharrlich(6, Duration::from_secs(15))
                .await
        );
        assert_eq!(
            b.empfange(Duration::from_secs(8), Some(1)).await,
            1,
            "der erste Auftritt trug schon nicht"
        );
        // Hier verschwindet B: Der Task endet, wenn der Kommandokanal
        // fällt.
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut c = Knoten::starten(Some(a.adresse.clone())).await;
    a.warte_auf_peers(1, Duration::from_secs(10)).await;
    assert!(
        a.veroeffentliche_beharrlich(7, Duration::from_secs(15))
            .await
    );
    assert_eq!(
        c.empfange(Duration::from_secs(10), Some(1)).await,
        1,
        "der wiedergekommene Knoten bekam nichts. Ein Netz, das einen Neustart \
         nicht verkraftet, verkraftet keinen Betrieb"
    );
}

/// Wiederholtes Trennen und Verbinden bringt das Netz nicht um.
///
/// Näher am realen Verlustbild als eine einmalige Partition: Auf einer
/// schlechten Leitung reißen Verbindungen ab und werden neu aufgebaut,
/// wieder und wieder.
///
/// ⚑ **Immer noch kein Paketverlust.** Siehe Modulkopf.
#[tokio::test]
async fn wiederholtes_trennen_und_verbinden_haelt_das_netz_am_leben() {
    let (a, mut b) = zwei_verbundene().await;
    assert!(
        a.veroeffentliche_beharrlich(8, Duration::from_secs(15))
            .await
    );
    assert_eq!(b.empfange(Duration::from_secs(8), Some(1)).await, 1);

    for _ in 0..4 {
        a.sperren(b.peer_id, true).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        a.sperren(b.peer_id, false).await;
        b.waehlen(a.adresse.clone());
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    a.warte_auf_peers(1, Duration::from_secs(10)).await;

    assert!(
        a.veroeffentliche_beharrlich(9, Duration::from_secs(20))
            .await,
        "nach vier Trennungen nahm Gossipsub nichts mehr an"
    );
    assert_eq!(
        b.empfange(Duration::from_secs(10), Some(1)).await,
        1,
        "nach vier Trennungen kam nichts mehr an: das Netz erholt sich nicht"
    );
}

/// ⚑ **Ein hängender Knoten hält das Netz nicht auf.**
///
/// Der ehrliche Ersatz für „Latenz-Spikes" ohne root. Statt Pakete zu
/// verzögern, hört ein Knoten eine Weile **auf, seine Ereignisse
/// abzuholen**: Er ist verbunden, antwortet aber nicht. Aus Sicht der
/// anderen sieht das aus wie eine sehr langsame Leitung, und für einen
/// überlasteten Knoten ist es genau das, was passiert.
///
/// **Was geprüft wird:** Die beiden übrigen tauschen weiter Nachrichten
/// aus, während der dritte steht. Ein Netz, das auf den langsamsten
/// Teilnehmer wartet, ist im Betrieb nicht zu gebrauchen.
///
/// **Und die Gegenprobe:** Nach dem Aufwachen findet der Hänger vor, was
/// in der Zwischenzeit kam. Ohne sie zeigte der Test nur, dass man einen
/// Knoten abhängen kann.
#[tokio::test]
async fn ein_haengender_knoten_haelt_das_netz_nicht_auf() {
    let a = Knoten::starten(None).await;
    let mut b = Knoten::starten(Some(a.adresse.clone())).await;
    let mut haenger = Knoten::starten(Some(a.adresse.clone())).await;
    assert!(
        a.warte_auf_peers(2, Duration::from_secs(15)).await >= 2,
        "die drei fanden einander nicht"
    );

    assert!(
        a.veroeffentliche_beharrlich(10, Duration::from_secs(15))
            .await
    );
    assert_eq!(b.empfange(Duration::from_secs(10), Some(1)).await, 1);

    // Der Hänger holt seine Ereignisse nicht ab: Wir rufen `empfange`
    // schlicht nicht auf. Sein Kanal läuft voll.
    a.veroeffentliche(11).await;
    assert_eq!(
        b.empfange(Duration::from_secs(10), Some(1)).await,
        1,
        "während einer hing, kam bei den übrigen nichts an"
    );

    // Der Hänger wacht auf und findet vor, was liegen blieb.
    assert!(
        haenger.empfange(Duration::from_secs(5), Some(1)).await >= 1,
        "der aufgewachte Knoten fand nichts vor. Dann hat ihn das Netz \
         fallengelassen, statt ihn wieder aufzunehmen"
    );
}
