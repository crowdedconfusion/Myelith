//! Zwei Knoten, ein Netz, ein auswertbares Protokoll.
//!
//! # Warum dieser Test die eigentliche Arbeit dieses Crates prüft
//!
//! `myl-net` hat einen Testrahmen mit zwanzig Knoten, und der lief
//! schon, als es dieses Crate noch nicht gab. Der Unterschied ist nicht
//! die Zahl der Knoten, sondern **wer sie startet**: Dort baut der Test
//! den Swarm selbst zusammen, hier tut es der Knoten, so wie er es auf
//! einer fremden Maschine auch täte.
//!
//! Genau darin lagen die Funde 55 bis 57: in der Verdrahtung, nicht in
//! den Bibliotheken.
//!
//! # Was hier zusätzlich geprüft wird: das Protokoll
//!
//! Ein Testlauf über mehrere Maschinen ist so viel wert wie das, was
//! danach rekonstruierbar ist. Deshalb liest dieser Test die
//! Protokolldatei **zurück** und prüft sie als Datei: lückenlose Folge,
//! Zeile für Zeile lesbar, die erwarteten Einträge vorhanden.
//!
//! Ein Protokoll, das niemand zurückgelesen hat, ist eine Vermutung
//! über eine Datei.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::{Knoten, ProtokollValidator};
use myl_net::{GossipTopic, PayloadValidator};

fn arbeitsverzeichnis(marke: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "myl-node-it-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn konfig(verzeichnis: &std::path::Path, name: &str, bootstrap: Vec<String>) -> KnotenKonfig {
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: verzeichnis.join(format!("{name}.key")),
        protokollverzeichnis: verzeichnis.join("logs"),
        // Ephemere Ports: Ein Test darf nicht daran scheitern, dass
        // gerade jemand anders auf 4150 horcht.
        horchadressen: vec![
            "/ip4/127.0.0.1/tcp/0".to_string(),
            "/ip4/127.0.0.1/udp/0/quic-v1".to_string(),
        ],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        nat: Default::default(),
        aufnahme_sekunden: 1,
    }
}

/// Eine Protokollzeile, so weit zerlegt, wie der Test sie braucht.
struct Zeile {
    folge: u64,
    art: String,
    knoten: String,
    roh: String,
}

/// Liest die Protokolldatei zurück.
///
/// Bewusst mit einfachen Mitteln: Das Format verspricht flache Objekte
/// aus Zeichenketten, Zahlen und Wahrheitswerten. Braucht dieser Leser
/// eines Tages mehr, hat das Format sein Versprechen gebrochen.
fn lies_protokoll(pfad: &std::path::Path) -> Vec<Zeile> {
    let inhalt = std::fs::read_to_string(pfad).expect("Protokolldatei");
    inhalt
        .lines()
        .filter(|z| !z.trim().is_empty())
        .map(|z| {
            let feld_zahl = |name: &str| -> u64 {
                let muster = format!("\"{name}\":");
                let start = z.find(&muster).unwrap_or_else(|| panic!("{name} fehlt in {z}"))
                    + muster.len();
                let rest = &z[start..];
                let ende = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
                rest[..ende].parse().unwrap_or_else(|_| panic!("{name} keine Zahl in {z}"))
            };
            let feld_text = |name: &str| -> String {
                let muster = format!("\"{name}\":\"");
                let start = z.find(&muster).unwrap_or_else(|| panic!("{name} fehlt in {z}"))
                    + muster.len();
                let rest = &z[start..];
                let ende = rest.find('"').unwrap_or(rest.len());
                rest[..ende].to_string()
            };
            Zeile {
                folge: feld_zahl("folge"),
                art: feld_text("art"),
                knoten: feld_text("knoten"),
                roh: z.to_string(),
            }
        })
        .collect()
}

/// **Zwei Knoten finden einander, und beide schreiben es auf.**
///
/// Der Grundfall. Beta bekommt Alphas Adresse als Bootstrap, so wie es
/// auf getrennten Maschinen auch liefe.
#[tokio::test]
async fn zwei_knoten_finden_einander_und_protokollieren_es() {
    let verz = arbeitsverzeichnis("finden");

    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    let adresse = alpha
        .warte_auf_adresse(Duration::from_secs(10))
        .await
        .expect("Alpha meldet eine Adresse");

    let mut beta = Knoten::starten(konfig(&verz, "beta", vec![adresse.to_string()]), false)
        .await
        .expect("Beta startet");

    assert_eq!(
        beta.warte_auf_peers(1, Duration::from_secs(20)).await,
        1,
        "Beta hat Alpha nicht erreicht"
    );
    alpha.laufe_fuer(Duration::from_secs(1)).await;
    assert_eq!(alpha.peers().await, 1, "Alpha sah die Verbindung nicht");

    // Beide Protokolle müssen die Verbindung tragen, und zwar mit der
    // Richtung: Nur so lässt sich hinterher sagen, wer gewählt hat.
    let a = lies_protokoll(alpha.protokollpfad());
    let b = lies_protokoll(beta.protokollpfad());

    let a_verbunden: Vec<&Zeile> = a.iter().filter(|z| z.art == "verbunden").collect();
    let b_verbunden: Vec<&Zeile> = b.iter().filter(|z| z.art == "verbunden").collect();
    assert!(!a_verbunden.is_empty(), "Alphas Protokoll kennt keine Verbindung");
    assert!(!b_verbunden.is_empty(), "Betas Protokoll kennt keine Verbindung");
    assert!(
        a_verbunden.iter().any(|z| z.roh.contains("\"eingehend\":true")),
        "Alpha vermerkt die Verbindung nicht als eingehend"
    );
    assert!(
        b_verbunden.iter().any(|z| z.roh.contains("\"eingehend\":false")),
        "Beta vermerkt die Verbindung nicht als ausgehend"
    );

    std::fs::remove_dir_all(&verz).ok();
}

/// **Ein echter Block läuft von Alpha zu Beta, Unsinn nicht.**
///
/// Das ist der Test, der den Knoten von einem Gossip-Weiterleiter
/// unterscheidet: Auf dem Blocks-Topic urteilt **seine eigene**
/// Nutzlastprüfung ([`myl_node::ProtokollValidator`]), und die gab es
/// bis Fund 55 nicht, weil `run_node` keinen Parameter dafür hatte.
///
/// Geprüft wird beides, denn eine Prüfung, die alles durchlässt, und
/// eine, die alles verwirft, sehen von einer Seite gleich aus.
#[tokio::test]
async fn ein_echter_block_kommt_an_unsinn_nicht() {
    use myl_consensus::block::{Block, EpochMeta};
    use myl_types::hash::Hash;

    let verz = arbeitsverzeichnis("block");

    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut beta = Knoten::starten(konfig(&verz, "beta", vec![adresse.to_string()]), false)
        .await
        .expect("Beta startet");
    beta.warte_auf_peers(1, Duration::from_secs(20)).await;
    alpha.warte_auf_peers(1, Duration::from_secs(10)).await;

    let block = Block::new(EpochMeta {
        epoch: 1,
        prev_block_hash: Hash::sha256(b"genesis"),
        timestamp_ms: 1_700_000_000_000,
        state_root: Hash::sha256(b"zustand"),
    });
    let gueltig = borsh::to_vec(&block).expect("Serialisierung");

    // Das Mesh braucht einen Moment; wiederholen, bis Gossipsub annimmt.
    let mut gesendet = false;
    for _ in 0..25 {
        if alpha.veroeffentliche(GossipTopic::Blocks, gueltig.clone()).await {
            gesendet = true;
            break;
        }
        alpha.laufe_fuer(Duration::from_millis(300)).await;
    }
    assert!(gesendet, "Gossipsub hat den Block nie angenommen");

    // Und Unsinn hinterher, auf demselben Topic.
    let unsinn = vec![0xABu8; 64];
    assert!(
        !ProtokollValidator.validate(GossipTopic::Blocks, &unsinn),
        "die Nutzlastprüfung hält diesen Unsinn für einen Block, \
         damit prüft dieser Test nichts"
    );
    for _ in 0..5 {
        alpha.veroeffentliche(GossipTopic::Blocks, unsinn.clone()).await;
        alpha.laufe_fuer(Duration::from_millis(200)).await;
    }

    beta.laufe_fuer(Duration::from_secs(3)).await;

    let b = lies_protokoll(beta.protokollpfad());
    let empfangen: Vec<&Zeile> = b.iter().filter(|z| z.art == "empfangen").collect();
    assert!(
        !empfangen.is_empty(),
        "Betas Protokoll kennt keinen Empfang: {:?}",
        b.iter().map(|z| z.art.as_str()).collect::<Vec<_>>()
    );
    // Genau der gültige Block, und nur er: Der Unsinn hat dieselbe Größe
    // nicht und dürfte ohnehin nicht durchkommen.
    for z in &empfangen {
        assert!(
            z.roh.contains(&format!("\"bytes\":{}", gueltig.len())),
            "eine empfangene Nachricht hat nicht die Größe des gültigen Blocks: {}",
            z.roh
        );
    }
    assert_eq!(
        empfangen.len(),
        1,
        "erwartet war genau eine durchgelassene Nachricht, es waren {}",
        empfangen.len()
    );

    let a = lies_protokoll(alpha.protokollpfad());
    assert!(
        a.iter().any(|z| z.art == "gesendet" && z.roh.contains("\"angenommen\":true")),
        "Alphas Protokoll kennt kein angenommenes Senden"
    );

    std::fs::remove_dir_all(&verz).ok();
}

/// **Das Protokoll ist als Datei auswertbar.**
///
/// Lückenlose Folge, jede Zeile lesbar, jede Zeile ordnet sich selbst
/// einem Knoten zu. Ohne diese drei Eigenschaften ist eine eingesammelte
/// Datei nach dem Kopieren wertlos.
#[tokio::test]
async fn das_protokoll_ist_lueckenlos_und_selbstzuordnend() {
    let verz = arbeitsverzeichnis("auswertbar");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    alpha.warte_auf_adresse(Duration::from_secs(10)).await;
    alpha.laufe_fuer(Duration::from_secs(3)).await;
    alpha.aufnahme().await;

    let zeilen = lies_protokoll(alpha.protokollpfad());
    assert!(zeilen.len() >= 4, "zu wenige Zeilen: {}", zeilen.len());
    for (i, z) in zeilen.iter().enumerate() {
        assert_eq!(z.folge, (i + 1) as u64, "Lücke in der Folge bei Zeile {}", i + 1);
        assert_eq!(z.knoten, "alpha", "Zeile ordnet sich nicht zu: {}", z.roh);
    }
    assert_eq!(zeilen[0].art, "start", "die erste Zeile ist kein Start");
    assert!(
        zeilen.iter().any(|z| z.art == "aufnahme"),
        "keine Zustandsaufnahme: dann liesse sich fehlender Empfang nicht \
         von fehlendem Betrieb unterscheiden"
    );
    assert!(
        zeilen.iter().any(|z| z.art == "horchadresse"),
        "keine Horchadresse im Protokoll"
    );
    assert_eq!(zeilen.len() as u64, alpha.protokollzeilen());

    std::fs::remove_dir_all(&verz).ok();
}

/// **Ein Knoten mit widersprüchlicher Konfiguration startet nicht.**
///
/// Fund 56 als Verhalten des Knotens: Rolle Relais ohne eigene
/// öffentliche Adresse. Der Fehler gehört an den Start, nicht in die
/// Stille eines Laufs, der niemanden erreicht.
#[tokio::test]
async fn fund_56_ein_relais_ohne_adresse_startet_nicht() {
    let verz = arbeitsverzeichnis("relais");
    let mut k = konfig(&verz, "relais", vec![]);
    k.rolle = Rolle::Relais;

    let ergebnis = Knoten::starten(k, false).await;
    assert!(
        ergebnis.is_err(),
        "ein Relais ohne eigene öffentliche Adresse ist gestartet"
    );
    let text = format!("{}", ergebnis.err().unwrap());
    assert!(
        text.contains("oeffentliche_adressen") || text.contains("öffentliche Adresse"),
        "die Fehlermeldung sagt nicht, was fehlt: {text}"
    );

    std::fs::remove_dir_all(&verz).ok();
}

/// **Die Identität überlebt den Neustart.**
///
/// Ohne diese Eigenschaft ist nach jedem Neustart ein anderer Knoten da,
/// und die Protokolle mehrerer Läufe lassen sich nicht zusammenführen.
/// Für einen Testlauf über Tage ist das der Unterschied zwischen einer
/// Messreihe und einer Sammlung von Einzelbildern.
#[tokio::test]
async fn die_identitaet_ueberlebt_den_neustart() {
    let verz = arbeitsverzeichnis("identitaet");

    let erst = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("erster Start");
    let id1 = erst.peer_id();
    let pfad1 = erst.protokollpfad().to_path_buf();
    drop(erst);

    let zweit = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("zweiter Start");
    assert_eq!(id1, zweit.peer_id(), "die Peer-Id hat sich beim Neustart geändert");
    assert_ne!(
        pfad1,
        zweit.protokollpfad(),
        "der zweite Lauf schreibt in dieselbe Datei wie der erste"
    );

    std::fs::remove_dir_all(&verz).ok();
}
