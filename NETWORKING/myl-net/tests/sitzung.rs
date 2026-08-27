//! Verschlüsselte Sitzungen über echte Verbindungen (Punkte 3.1 bis 3.3).
//!
//! # Warum es diese Datei zusätzlich zu den Modultests gibt
//!
//! Die Modultests in `src/sitzung.rs` prüfen das Verfahren: Ableitung,
//! Nonce, Wiedereinspielung, Rotation. Sie prüfen es an Werten im
//! Speicher, und dort geht nie etwas verloren, kommt nie etwas doppelt
//! an und liest nie jemand mit.
//!
//! Hier laufen echte Knoten über echte Verbindungen, und die Nutzlast
//! geht über [`myl_net::anfrage`], also über den Weg, den sie im Betrieb
//! nimmt. Der Unterschied ist der Prüfgegenstand: Dort das Verfahren,
//! hier die Verdrahtung.
//!
//! # Das Gateway ist der eigentliche Prüfstein
//!
//! Kap. 9.2 nennt kompromittierte Gateways als Angreiferklasse. Ein Test
//! dazu ist nur dann einer, wenn das Gateway seine Arbeit wirklich tut:
//! Es nimmt an, es leitet weiter, es leitet zurück, und es hat zu keinem
//! Zeitpunkt einen Schlüssel. `weiterleitendes_gateway` unten ist genau
//! das, und es scheitert am Öffnen, während der Shard es öffnet.
//!
//! # Zu jedem Nachweis eine Gegenprobe
//!
//! „Der Beobachter konnte nichts lesen" beweist für sich nichts, solange
//! nicht feststeht, dass überhaupt etwas zu lesen war. Wo ein Test etwas
//! ausschließt, zeigt derselbe Test, dass der erlaubte Fall gelingt.

use std::time::Duration;

use myl_net::{
    build_swarm, endpunkt_aus_schluessel, run_node, subscribe_all, Endpunkt,
    Epochenankuendigung, Epochenschluessel, NetConfig, NodeCommand, NodeEvent, NodeIdentity,
    SitzungsFehler, Sitzungen, Versiegelt,
};
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};
use tokio::sync::mpsc;

const FRIST: Duration = Duration::from_secs(20);

/// Ein laufender Knoten mit Kommando- und Ereigniskanal.
struct Knoten {
    peer_id: libp2p::PeerId,
    kommandos: mpsc::UnboundedSender<NodeCommand>,
    ereignisse: mpsc::UnboundedReceiver<NodeEvent>,
    adresse: libp2p::Multiaddr,
}

impl Knoten {
    async fn starten() -> Knoten {
        let identitaet = NodeIdentity::generate();
        let peer_id = identitaet.peer_id();
        let mut swarm = build_swarm(&identitaet, &NetConfig::default()).expect("Swarm");
        subscribe_all(&mut swarm).expect("Topics");
        swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().expect("Multiaddr"))
            .expect("Listen");
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

    fn waehle(&self, addr: &libp2p::Multiaddr) {
        self.kommandos
            .send(NodeCommand::Dial {
                addr: addr.clone(),
                result: None,
            })
            .expect("Kommandokanal");
    }

    fn frage(&self, an: libp2p::PeerId, daten: Vec<u8>) {
        self.kommandos
            .send(NodeCommand::Anfrage { an, daten })
            .expect("Kommandokanal");
    }

    fn antworte(&self, marke: u64, daten: Vec<u8>) {
        self.kommandos
            .send(NodeCommand::Antwort { marke, daten })
            .expect("Kommandokanal");
    }

    /// Wartet, bis mindestens `n` Gegenstellen verbunden sind.
    ///
    /// Über das Kommando, nicht am Swarm vorbei: geprüft wird der Weg,
    /// den ein Knoten im Betrieb nimmt.
    async fn warte_auf_peers(&self, n: usize) {
        let ende = tokio::time::Instant::now() + FRIST;
        loop {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.kommandos
                .send(NodeCommand::PeerCount(tx))
                .expect("Kommandokanal");
            if rx.await.expect("Ergebniskanal") >= n {
                return;
            }
            assert!(
                tokio::time::Instant::now() < ende,
                "keine Verbindung innerhalb der Frist"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Wartet auf eine eingehende Anfrage und gibt Bytes samt Marke zurück.
    async fn naechste_anfrage(&mut self) -> (Vec<u8>, u64) {
        let ende = tokio::time::Instant::now() + FRIST;
        loop {
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(rest, self.ereignisse.recv()).await {
                Ok(Some(NodeEvent::AnfrageEingegangen { daten, marke, .. })) => {
                    return (daten, marke)
                }
                Ok(Some(_)) => continue,
                Ok(None) => panic!("Ereigniskanal geschlossen"),
                Err(_) => panic!("keine Anfrage innerhalb der Frist"),
            }
        }
    }

    /// Wartet auf die Antwort zu einer eigenen Anfrage.
    async fn naechste_antwort(&mut self) -> Vec<u8> {
        let ende = tokio::time::Instant::now() + FRIST;
        loop {
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(rest, self.ereignisse.recv()).await {
                Ok(Some(NodeEvent::AntwortEingegangen { daten, .. })) => return daten,
                Ok(Some(NodeEvent::AnfrageGescheitert { grund, .. })) => {
                    panic!("Anfrage gescheitert: {grund}")
                }
                Ok(Some(_)) => continue,
                Ok(None) => panic!("Ereigniskanal geschlossen"),
                Err(_) => panic!("keine Antwort innerhalb der Frist"),
            }
        }
    }
}

/// Der Konsensschlüssel eines Teilnehmers, aus fester Saat.
///
/// Er bestimmt zugleich den Endpunkt: `MinerId` und `Address` sind der
/// Hash eben dieses Schlüssels, und darauf beruht die ganze Prüfung der
/// Ankündigung.
fn konsens(n: u8) -> BlsSecretKey {
    BlsSecretKey::key_gen(&[n; 32]).expect("Schlüsselerzeugung")
}

fn miner(n: u8) -> Endpunkt {
    endpunkt_aus_schluessel(&konsens(n).public_key().expect("Schlüssel"))
}

fn pod() -> PodId {
    PodId::new([0x50; 32])
}

/// Schickt **genau eine** Anfrage und wartet auf die Antwort.
///
/// # ⚑ Warum hier nicht wiederholt wird
///
/// Der erste Entwurf schickte bei einem Fehlschlag noch einmal, weil die
/// Verbindung beim ersten Versuch oft noch im Aufbau ist. Das brachte
/// den Wiedereinspielungstest um: Eine Wiederholung ist aus Sicht der
/// Gegenstelle genau das, wogegen sie sich wehrt, und der Zähler der
/// empfangenen Nachrichten stimmte danach nicht mehr.
///
/// Gewartet wird deshalb **vor** dem Senden, auf die Verbindung, und
/// gesendet wird einmal. Ein Test, der sich selbst Nachrichten
/// verdoppelt, misst nicht mehr, was er zu messen vorgibt.
async fn frage_einmal(von: &mut Knoten, an: libp2p::PeerId, daten: Vec<u8>) -> Vec<u8> {
    von.warte_auf_peers(1).await;
    von.frage(an, daten);
    von.naechste_antwort().await
}


/// Nimmt **eine** Anfrage entgegen und beantwortet sie.
///
/// # ⚑ Warum die Knoten nicht in Aufgaben wandern
///
/// Der erste Entwurf schob den antwortenden Knoten mit `tokio::spawn`
/// in eine Aufgabe. Am Ende der Aufgabe wurde er fallen gelassen, sein
/// Kommandokanal schloss, `run_node` fuhr herunter, und die eben erst
/// abgeschickte Antwort ging mit: „Connection was closed before a
/// response was received".
///
/// **Der Fehler wurde einmal behoben und blieb zwei Tests weiter
/// stehen**, weil dort der Knoten aus der Aufgabe zurückgegeben wurde
/// und hier nicht. Ein Test, der nur meistens grün ist, ist schlimmer
/// als keiner: Er bringt jemandem bei, noch einmal zu starten.
///
/// Behoben wird deshalb nicht die Stelle, sondern das Muster. Beide
/// Knoten bleiben dem Test gehören, gearbeitet wird mit `tokio::join!`
/// über zwei geliehene Verweise, und es gibt keinen Ort mehr, an dem
/// ein Knoten zu früh fallen kann.
async fn antworte_einmal<F, R>(knoten: &mut Knoten, arbeit: F) -> R
where
    F: FnOnce(Vec<u8>) -> (Vec<u8>, R),
{
    let (daten, marke) = knoten.naechste_anfrage().await;
    let (antwort, ergebnis) = arbeit(daten);
    knoten.antworte(marke, antwort);
    ergebnis
}

fn enthaelt(heuhaufen: &[u8], nadel: &[u8]) -> bool {
    heuhaufen.windows(nadel.len()).any(|f| f == nadel)
}

/// Punkt 3.1: Aktivierungen zwischen zwei Shard-Minern.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eine_aktivierung_geht_verschluesselt_von_shard_zu_shard() {
    let mut a = Knoten::starten().await;
    let mut b = Knoten::starten().await;
    a.waehle(&b.adresse);
    let b_peer = b.peer_id;

    let a_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let b_schluessel = Epochenschluessel::ziehe(EpochId(9));

    // Der Punkt kommt nicht aus der Hand, sondern über die beglaubigte
    // Ankündigung: So läuft es im Betrieb, und nur so hängt an der
    // Verschlüsselung eine Aussage darüber, mit wem man spricht.
    let a_ankuendigung =
        Epochenankuendigung::neu(&konsens(1), &a_schluessel).expect("ankündigen");
    let b_ankuendigung =
        Epochenankuendigung::neu(&konsens(2), &b_schluessel).expect("ankündigen");

    // Geprüft wird gegen den Endpunkt aus dem Pod-Pfad, sonst nichts.
    let a_punkt = a_ankuendigung.pruefe(miner(1), EpochId(9)).expect("prüfen");
    let b_punkt = b_ankuendigung.pruefe(miner(2), EpochId(9)).expect("prüfen");

    let mut a_sitzungen = Sitzungen::neu(miner(1), a_schluessel);
    let mut b_sitzungen = Sitzungen::neu(miner(2), b_schluessel);

    // Aktivierungen sind ganzzahlig; hier steht ein Ausschnitt daraus.
    let aktivierung: Vec<u8> = (0u16..512).flat_map(|w| w.to_le_bytes()).collect();
    let versiegelt = a_sitzungen
        .kanal(pod(), miner(2), &b_punkt)
        .expect("Kanal")
        .versiegle(&aktivierung)
        .expect("versiegeln");
    let auf_dem_draht = versiegelt.zu_bytes();

    // Die Gegenprobe zum Nachweis: Der Klartext ist wirklich vorhanden
    // und wirklich nicht in den Bytes.
    assert!(
        !enthaelt(&auf_dem_draht, &aktivierung),
        "die Aktivierung steht im Klartext auf dem Draht"
    );

    let (antwort_bytes, bei_b_angekommen) = tokio::join!(
        frage_einmal(&mut a, b_peer, auf_dem_draht),
        antworte_einmal(&mut b, |daten| {
            let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
            let klartext = b_sitzungen
                .kanal(pod(), miner(1), &a_punkt)
                .expect("Kanal")
                .oeffne(&nachricht)
                .expect("öffnen");
            // Die Rückrichtung im selben Kanal: eigener Schlüssel,
            // eigener Zähler.
            let antwort = b_sitzungen
                .kanal(pod(), miner(1), &a_punkt)
                .expect("Kanal")
                .versiegle(b"Aktivierung angenommen")
                .expect("versiegeln");
            (antwort.zu_bytes(), klartext)
        })
    );

    assert_eq!(bei_b_angekommen, aktivierung, "die Aktivierung kam verändert an");

    let antwort = Versiegelt::aus_bytes(&antwort_bytes).expect("Rahmen");
    let klartext = a_sitzungen
        .kanal(pod(), miner(2), &b_punkt)
        .expect("Kanal")
        .oeffne(&antwort)
        .expect("öffnen");
    assert_eq!(klartext, b"Aktivierung angenommen");
}

/// Punkt 3.2: Nutzer, Gateway, Shard. Das Gateway leitet weiter und
/// liest nicht mit.
///
/// Der einzige Test der Datei, der eine Aufgabe braucht: Das Gateway
/// muss annehmen und weitergeben, während der Nutzer wartet. Es gibt
/// seinen Knoten deshalb zurück, statt ihn fallen zu lassen; siehe
/// [`antworte_einmal`].
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_weiterleitendes_gateway_kann_nicht_mitlesen() {
    let mut nutzer = Knoten::starten().await;
    let mut gateway = Knoten::starten().await;
    let mut shard = Knoten::starten().await;
    nutzer.waehle(&gateway.adresse);
    gateway.waehle(&shard.adresse);

    // Drei Beteiligte, drei Epochenschlüssel. Das Gateway bekommt einen
    // eigenen, damit der Test nicht daran scheitert, dass es keinen
    // hätte: Es hat einen, er nützt ihm nur nichts.
    let nutzer_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let gateway_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let shard_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let nutzer_punkt = Epochenankuendigung::neu(&konsens(1), &nutzer_schluessel)
        .expect("ankündigen")
        .pruefe(miner(1), EpochId(9))
        .expect("prüfen");
    let shard_punkt = Epochenankuendigung::neu(&konsens(3), &shard_schluessel)
        .expect("ankündigen")
        .pruefe(miner(3), EpochId(9))
        .expect("prüfen");

    let nutzer_endpunkt = miner(1);
    let shard_endpunkt = miner(3);

    let mut nutzer_sitzungen = Sitzungen::neu(nutzer_endpunkt, nutzer_schluessel);
    let prompt = b"Was ist die Hauptstadt von Norwegen?".to_vec();
    let versiegelt = nutzer_sitzungen
        .kanal(pod(), shard_endpunkt, &shard_punkt)
        .expect("Kanal")
        .versiegle(&prompt)
        .expect("versiegeln");
    let auf_dem_draht = versiegelt.zu_bytes();

    let shard_peer = shard.peer_id;
    let gateway_peer = gateway.peer_id;

    // Das Gateway: nimmt an, leitet weiter, leitet zurück. Und versucht
    // unterwegs zu lesen.
    let gateway_arbeit = tokio::spawn(async move {
        gateway.warte_auf_peers(1).await;
        let (vom_nutzer, marke) = gateway.naechste_anfrage().await;

        let nachricht = Versiegelt::aus_bytes(&vom_nutzer).expect("Rahmen");
        // Was das Gateway zum Weiterleiten braucht, sieht es: Empfänger
        // und Epoche stehen im Klartextkopf.
        assert_eq!(nachricht.kopf.an, shard_endpunkt);
        assert_eq!(nachricht.kopf.epoche, EpochId(9));

        // Was es nicht darf, kann es nicht: Mit seinem eigenen
        // Epochenschlüssel und dem angekündigten Punkt des Nutzers baut
        // es einen Kanal und scheitert.
        let mut versuch = Sitzungen::neu(shard_endpunkt, gateway_schluessel);
        let ergebnis = versuch
            .kanal(pod(), nutzer_endpunkt, &nutzer_punkt)
            .expect("Kanal")
            .oeffne(&nachricht);
        assert!(
            matches!(ergebnis, Err(SitzungsFehler::TagStimmtNicht)),
            "das Gateway konnte lesen: {ergebnis:?}"
        );
        assert!(
            !enthaelt(&vom_nutzer, b"Norwegen"),
            "der Prompt steht im Klartext auf dem Draht"
        );

        // Weiterleiten, unverändert.
        gateway.frage(shard_peer, vom_nutzer.clone());
        let vom_shard = gateway.naechste_antwort().await;
        gateway.antworte(marke, vom_shard);
        // Der Knoten geht mit zurück, sonst schlösse er seine
        // Verbindung, während die Antwort noch unterwegs ist.
        (gateway, vom_nutzer)
    });

    let mut shard_sitzungen = Sitzungen::neu(shard_endpunkt, shard_schluessel);
    let (antwort_bytes, beim_shard) = tokio::join!(
        frage_einmal(&mut nutzer, gateway_peer, auf_dem_draht.clone()),
        antworte_einmal(&mut shard, |daten| {
            let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
            let klartext = shard_sitzungen
                .kanal(pod(), nutzer_endpunkt, &nutzer_punkt)
                .expect("Kanal")
                .oeffne(&nachricht)
                .expect("öffnen");
            let antwort = shard_sitzungen
                .kanal(pod(), nutzer_endpunkt, &nutzer_punkt)
                .expect("Kanal")
                .versiegle(b"Oslo")
                .expect("versiegeln");
            (antwort.zu_bytes(), klartext)
        })
    );
    let (_gateway, durchgereicht) = gateway_arbeit.await.expect("Gateway");

    // Der erlaubte Fall gelingt: Ohne diesen Nachweis hieße „niemand
    // konnte lesen" vielleicht nur „es kam nichts an".
    assert_eq!(beim_shard, prompt, "der Prompt kam beim Shard nicht an");
    assert_eq!(
        durchgereicht, auf_dem_draht,
        "das Gateway hat die Bytes verändert"
    );

    let antwort = Versiegelt::aus_bytes(&antwort_bytes).expect("Rahmen");
    let klartext = nutzer_sitzungen
        .kanal(pod(), shard_endpunkt, &shard_punkt)
        .expect("Kanal")
        .oeffne(&antwort)
        .expect("öffnen");
    assert_eq!(klartext, b"Oslo");
}

/// Punkt 3.3: Ein Mitschnitt aus Epoche e ist nach der Rotation nicht
/// mehr zu öffnen, und der Kanal trägt in e+1 weiter.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_mitschnitt_ueberlebt_die_rotation_nicht() {
    let mut a = Knoten::starten().await;
    let mut b = Knoten::starten().await;
    a.waehle(&b.adresse);
    let b_peer = b.peer_id;

    let a_alt = Epochenschluessel::ziehe(EpochId(9));
    let b_alt = Epochenschluessel::ziehe(EpochId(9));
    let a_punkt_alt = Epochenankuendigung::neu(&konsens(1), &a_alt)
        .expect("ankündigen")
        .pruefe(miner(1), EpochId(9))
        .expect("prüfen");
    let b_punkt_alt = Epochenankuendigung::neu(&konsens(2), &b_alt)
        .expect("ankündigen")
        .pruefe(miner(2), EpochId(9))
        .expect("prüfen");
    let mut a_sitzungen = Sitzungen::neu(miner(1), a_alt);
    let mut b_sitzungen = Sitzungen::neu(miner(2), b_alt);

    let mitschnitt = a_sitzungen
        .kanal(pod(), miner(2), &b_punkt_alt)
        .expect("Kanal")
        .versiegle(b"Inhalt aus Epoche 9")
        .expect("versiegeln")
        .zu_bytes();

    // Erst der erlaubte Fall: In Epoche 9 geht die Nachricht auf.
    let (_, klartext) = tokio::join!(
        frage_einmal(&mut a, b_peer, mitschnitt.clone()),
        antworte_einmal(&mut b, |daten| {
            let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
            let klartext = b_sitzungen
                .kanal(pod(), miner(1), &a_punkt_alt)
                .expect("Kanal")
                .oeffne(&nachricht)
                .expect("öffnen");
            (b"angekommen".to_vec(), klartext)
        })
    );
    assert_eq!(klartext, b"Inhalt aus Epoche 9");

    // Epochenwechsel auf beiden Seiten.
    let a_neu = Epochenschluessel::ziehe(EpochId(10));
    let b_neu = Epochenschluessel::ziehe(EpochId(10));
    let a_punkt_neu = Epochenankuendigung::neu(&konsens(1), &a_neu)
        .expect("ankündigen")
        .pruefe(miner(1), EpochId(10))
        .expect("prüfen");
    let b_punkt_neu = Epochenankuendigung::neu(&konsens(2), &b_neu)
        .expect("ankündigen")
        .pruefe(miner(2), EpochId(10))
        .expect("prüfen");
    a_sitzungen.rotiere(a_neu).expect("rotieren");
    b_sitzungen.rotiere(b_neu).expect("rotieren");

    // Derselbe Mitschnitt, noch einmal über denselben Draht.
    let (_, ergebnis) = tokio::join!(
        frage_einmal(&mut a, b_peer, mitschnitt),
        antworte_einmal(&mut b, |daten| {
            let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
            let ergebnis = b_sitzungen
                .kanal(pod(), miner(1), &a_punkt_neu)
                .expect("Kanal")
                .oeffne(&nachricht);
            (b"gesehen".to_vec(), ergebnis)
        })
    );
    assert!(
        matches!(ergebnis, Err(SitzungsFehler::EpocheVorbei { .. })),
        "der Mitschnitt aus Epoche 9 ging in Epoche 10 noch auf: {ergebnis:?}"
    );

    // Und die Gegenprobe: In der neuen Epoche trägt der Kanal weiter.
    // Ohne sie hieße „geht nicht mehr auf" vielleicht „geht gar nicht
    // mehr".
    let frisch = a_sitzungen
        .kanal(pod(), miner(2), &b_punkt_neu)
        .expect("Kanal")
        .versiegle(b"Inhalt aus Epoche 10")
        .expect("versiegeln")
        .zu_bytes();
    let (_, klartext) = tokio::join!(
        frage_einmal(&mut a, b_peer, frisch),
        antworte_einmal(&mut b, |daten| {
            let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
            let klartext = b_sitzungen
                .kanal(pod(), miner(1), &a_punkt_neu)
                .expect("Kanal")
                .oeffne(&nachricht)
                .expect("öffnen");
            (b"angekommen".to_vec(), klartext)
        })
    );
    assert_eq!(klartext, b"Inhalt aus Epoche 10");
}

/// Eine wiedereingespielte Nachricht über den echten Draht.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eine_wiedereingespielte_nachricht_wird_ueber_den_draht_abgewiesen() {
    let mut a = Knoten::starten().await;
    let mut b = Knoten::starten().await;
    a.waehle(&b.adresse);
    let b_peer = b.peer_id;

    let a_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let b_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let a_punkt = Epochenankuendigung::neu(&konsens(1), &a_schluessel)
        .expect("ankündigen")
        .pruefe(miner(1), EpochId(9))
        .expect("prüfen");
    let b_punkt = Epochenankuendigung::neu(&konsens(2), &b_schluessel)
        .expect("ankündigen")
        .pruefe(miner(2), EpochId(9))
        .expect("prüfen");
    let mut a_sitzungen = Sitzungen::neu(miner(1), a_schluessel);
    let mut b_sitzungen = Sitzungen::neu(miner(2), b_schluessel);

    let einmal = a_sitzungen
        .kanal(pod(), miner(2), &b_punkt)
        .expect("Kanal")
        .versiegle(b"nur einmal gueltig")
        .expect("versiegeln")
        .zu_bytes();

    let oeffne = |sitzungen: &mut Sitzungen, daten: Vec<u8>| {
        let nachricht = Versiegelt::aus_bytes(&daten).expect("Rahmen");
        sitzungen
            .kanal(pod(), miner(1), &a_punkt)
            .expect("Kanal")
            .oeffne(&nachricht)
            .map(|k| String::from_utf8_lossy(&k).into_owned())
    };

    let (_, erstes) = tokio::join!(
        frage_einmal(&mut a, b_peer, einmal.clone()),
        antworte_einmal(&mut b, |daten| {
            (b"gesehen".to_vec(), oeffne(&mut b_sitzungen, daten))
        })
    );
    let (_, zweites) = tokio::join!(
        frage_einmal(&mut a, b_peer, einmal),
        antworte_einmal(&mut b, |daten| {
            (b"gesehen".to_vec(), oeffne(&mut b_sitzungen, daten))
        })
    );

    assert_eq!(erstes.as_deref(), Ok("nur einmal gueltig"));
    assert!(
        matches!(zweites, Err(SitzungsFehler::Wiedereinspielung { .. })),
        "die Wiedereinspielung wurde angenommen: {zweites:?}"
    );
}

/// Der Mann in der Mitte: Das Gateway schiebt seinen eigenen
/// Epochenpunkt unter.
///
/// # Warum dieser Test der wichtigste der Datei ist
///
/// Alle anderen zeigen, dass ein Dritter den Geheimtext nicht öffnen
/// kann. Sie setzen dabei voraus, dass beide Seiten den **richtigen**
/// Punkt der Gegenseite kennen. Fällt diese Voraussetzung, fällt alles
/// andere mit: Wer einen fremden Punkt unterschieben kann, führt beide
/// Seiten in eine Sitzung mit sich selbst und liest mit, ohne dass ein
/// einziges Tag danebengeht.
///
/// Geprüft wird gegen den Endpunkt aus dem Pod-Pfad. Das Gateway hat
/// einen echten Konsensschlüssel und kann damit alles unterschreiben,
/// was es will; es kann nur nicht den Endpunkt des Shards annehmen,
/// denn der ist der Hash eines Schlüssels, den es nicht hat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_untergeschobener_epochenpunkt_faellt_beim_nutzer_auf() {
    let shard_schluessel = Epochenschluessel::ziehe(EpochId(9));
    let gateway_schluessel = Epochenschluessel::ziehe(EpochId(9));

    // Was der Shard wirklich ankündigt.
    let echte = Epochenankuendigung::neu(&konsens(3), &shard_schluessel).expect("ankündigen");

    // Was das Gateway stattdessen weiterreicht: sein eigener Punkt,
    // ordentlich mit seinem eigenen Konsensschlüssel unterschrieben.
    let untergeschoben =
        Epochenankuendigung::neu(&konsens(2), &gateway_schluessel).expect("ankündigen");

    // Der Nutzer prüft gegen den Endpunkt, den sein Pod-Pfad nennt.
    let ergebnis = untergeschoben.pruefe(miner(3), EpochId(9));
    assert!(
        matches!(ergebnis, Err(SitzungsFehler::EndpunktPasstNicht { .. })),
        "der untergeschobene Punkt wurde angenommen: {ergebnis:?}"
    );

    // Gegenprobe: Die echte Ankündigung geht durch. Sonst hieße der
    // Nachweis oben nur, dass gar nichts durchkommt.
    assert_eq!(
        echte.pruefe(miner(3), EpochId(9)).expect("prüfen"),
        shard_schluessel.punkt()
    );

    // Und die zweite Hälfte: Unter eigenem Namen wird das Gateway
    // anerkannt. Dann aber redet der Nutzer nachweislich mit dem
    // Gateway und nicht mit dem Shard, und die Verwechslung ist keine
    // mehr.
    assert_eq!(untergeschoben.behaupteter_endpunkt(), miner(2));
    assert!(untergeschoben.pruefe(miner(2), EpochId(9)).is_ok());
}
