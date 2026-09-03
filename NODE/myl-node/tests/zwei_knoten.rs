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
        kettendatei: None,
        stimmsatzdatei_pfad: None,
        konsensschluesseldatei: None,
        nat: Default::default(),
        aufnahme_sekunden: 1,
        // Kein Endpunkt: Ein fester Port kollidierte, sobald zwei
        // Testknoten nebeneinander laufen, und dieser Test prueft
        // ohnehin etwas anderes.
        beobachtung: None,
        // Dieselbe Begruendung: ein fester Port kollidiert, sobald zwei
        // Knoten nebeneinander laufen.
        tuer: None,
        ortsleitung: None,
        ortsausweis: None,
        pod: None,
        modellname: "myelith-qwen2.5-0.5b".to_string(),
        kontoschluesseldatei: None,
        konto: None,
        // Kein Testverkehr: Die Tests hier schicken gezielt, damit
        // sichtbar bleibt, welche Nachricht wessen ist.
        testverkehr_sekunden: None,
        erzeugt_bloecke: false,
        // Ein Probelauf mit Attest-Prüfung: beide Knoten kennen einander.
        teilnehmer: vec!["alpha".into(), "beta".into()],
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
    use myl_consensus::block::{Block, BlockHeader};
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

    let block = Block::new(BlockHeader {
        height: 1,
        epoch: 0,
        prev_block_hash: Hash::sha256(b"genesis"),
        timestamp_ms: 1_700_000_000_000,
        state_root: Hash::sha256(b"zustand"),
        saatquelle: None,
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
        !ProtokollValidator::default().validate(GossipTopic::Blocks, &unsinn),
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

/// ⚑ **Der Knoten frischt die Kontraktabschrift auf** (B6-3).
///
/// Die Abschrift selbst ist in `tuer.rs` geprüft. **Was hier geprüft
/// wird, ist der Aufruf:** dass die Zustandsaufnahme ihn tut. Ohne
/// diesen Test wäre die eine Zeile in `aufnahme` ungeprüft, und ihr
/// Wegfall fiele nirgends auf.
///
/// ⚑ **Gefunden, weil die Gegenprobe nicht biss.** Der Aufruf war
/// gebaut und von keinem Test erreicht: dieselbe Klasse, die diese
/// Woche siebenmal aufgetreten ist.
#[tokio::test]
async fn die_zustandsaufnahme_frischt_die_kontraktabschrift_auf() {
    use myl_types::ids::{Address, EpochId};
    use myl_types::sitzung::{Grenzen, Sitzungskontrakt};

    let verz = arbeitsverzeichnis("abschrift");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");

    let abschrift = alpha.kontraktabschrift();
    assert_eq!(abschrift.anzahl(), 0, "vor der Aufnahme ist sie leer");

    // Einen Kontrakt in die Kette des Knotens legen.
    let kontrakt = Sitzungskontrakt {
        inhaber: Address::new([1; 32]),
        agent: Address::new([2; 32]),
        credits: Grenzen::gesperrt(),
        myl: Grenzen::gesperrt(),
        empfaenger: Vec::new(),
        gueltig_ab: EpochId(0),
        gueltig_bis: EpochId(10),
        max_schritte: 1,
    };
    let id = kontrakt.adresse();
    alpha.kette_mut().zustand_mut().sitzungen.insert(
        id,
        myl_ledger::state::Sitzung {
            kontrakt,
            zustand: myl_types::sitzung::Sitzungszustand::neu(),
        },
    );

    alpha.aufnahme().await;
    assert_eq!(
        abschrift.anzahl(),
        1,
        "die Aufnahme hat die Abschrift nicht aufgefrischt"
    );

    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Ein Inferenzauftrag geht über das Netz und bekommt eine Antwort**
/// (GATEWAY Stufe 4, erster Transportschnitt).
///
/// Bis zum 2026-09-03 gab es dafür keinen Weg: kein Gossip-Thema, kein
/// Auftragstyp, und `myl-pod` kannte `myl-net` nicht. **Ein Pod bekam
/// seine Arbeit von der Kommandozeile.**
///
/// **Die Antwort ist hier `Abgelehnt`, und das ist richtig so:** Der
/// Empfänger beherbergt keinen Shard, denn ein Shard läuft nach der
/// Entscheidung vom 2026-09-03 in einem **eigenen Prozess**. Was der
/// Test belegt, ist der Weg, nicht das Rechnen.
///
/// ⚑ **Abgelehnt und nicht geschwiegen.** Der Fragende soll „hier
/// rechnet niemand" von „nicht angekommen" unterscheiden können; ein
/// Auftrag ohne Antwort läuft in eine Zeitüberschreitung, die nichts
/// bedeutet.
#[tokio::test]
async fn ein_inferenzauftrag_geht_ueber_das_netz() {
    use myl_types::ids::EpochId;
    use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
    use myl_types::sitzung::Anfragebindung;

    let verz = arbeitsverzeichnis("inferenz");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    let adresse = alpha
        .warte_auf_adresse(Duration::from_secs(10))
        .await
        .expect("Alpha nennt eine Adresse");
    let mut beta = Knoten::starten(
        konfig(&verz, "beta", vec![adresse.to_string()]),
        false,
    )
    .await
    .expect("Beta startet");

    // Auf die Verbindung warten, nicht auf die Uhr.
    let alpha_id = alpha.peer_id();
    let bis = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < bis && beta.peers().await == 0 {
        beta.laufe_fuer(Duration::from_millis(200)).await;
        alpha.laufe_fuer(Duration::from_millis(200)).await;
    }
    assert!(beta.peers().await > 0, "Beta hat Alpha nicht gefunden");

    let auftrag = Inferenzauftrag {
        sitzung: 42,
        bindung: Anfragebindung::neu(42, b"was ist die hauptstadt von frankreich", EpochId(0)),
        prompt_versiegelt: b"versiegelter prompt".to_vec(),
        max_token: 64,
        pipeline: myl_types::hash::Hash::sha256(b"probe-pipeline"),
    };
    assert!(
        beta.inferenz_senden(alpha_id, auftrag).await,
        "der Auftrag wurde nicht abgeschickt"
    );

    // Auf die Wirkung warten.
    let bis = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < bis && beta.letzte_inferenzantwort().is_none() {
        alpha.laufe_fuer(Duration::from_millis(200)).await;
        beta.laufe_fuer(Duration::from_millis(200)).await;
    }

    match beta.letzte_inferenzantwort() {
        Some(Inferenzantwort::Abgelehnt { sitzung }) => {
            assert_eq!(*sitzung, 42, "die Ablehnung nennt eine fremde Sitzung");
        }
        andere => panic!("erwartet war eine Ablehnung, bekommen: {andere:?}"),
    }

    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Ein formwidriger Auftrag verlässt die Maschine gar nicht.**
///
/// Der Deckel steht **vor** der Leitung: Ein Auftrag, der ohnehin
/// abgewiesen wird, soll sie nicht belasten. Der Empfänger prüft
/// trotzdem noch einmal, denn er glaubt dem Absender nichts.
#[tokio::test]
async fn ein_formwidriger_auftrag_wird_nicht_gesendet() {
    use myl_types::ids::EpochId;
    use myl_types::inferenzauftrag::{Inferenzauftrag, MAX_NEUE_TOKEN};
    use myl_types::sitzung::Anfragebindung;

    let verz = arbeitsverzeichnis("formwidrig");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    let fremd = alpha.peer_id();

    let schlecht = |max_token: u32, prompt: Vec<u8>| Inferenzauftrag {
        sitzung: 1,
        bindung: Anfragebindung::neu(1, b"frage", EpochId(0)),
        prompt_versiegelt: prompt,
        max_token,
        pipeline: myl_types::hash::Hash::sha256(b"p"),
    };

    assert!(
        !alpha.inferenz_senden(fremd, schlecht(0, b"x".to_vec())).await,
        "ein Auftrag ueber null Token ging auf die Leitung"
    );
    assert!(
        !alpha
            .inferenz_senden(fremd, schlecht(MAX_NEUE_TOKEN + 1, b"x".to_vec()))
            .await,
        "ein Auftrag ueber zu viele Token ging auf die Leitung"
    );
    assert!(
        !alpha.inferenz_senden(fremd, schlecht(8, Vec::new())).await,
        "ein Auftrag ohne Prompt ging auf die Leitung"
    );
    // Gegenprobe: ein gueltiger geht raus, sonst prueft der Test nur,
    // dass gar nichts gesendet wird.
    assert!(
        alpha.inferenz_senden(fremd, schlecht(8, b"x".to_vec())).await,
        "auch der gueltige Auftrag ging nicht raus"
    );

    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Die Wiederausfuhr der Leitungsgrenze zeigt noch auf dieselbe Zahl.**
///
/// Bis zum 2026-09-03 stand hier die Naht selbst: `MAX_PROMPT_BYTES` in
/// `myl-types`, `MAX_ANFRAGE_BYTES` in `myl-net`, und nur dieser Test
/// sah beide. **Seit Fund 155 wohnt die Grenze in `myl-types`**, und
/// die Naht ist dort eine Zusicherung des Übersetzers.
///
/// Übrig bleibt eine Frage, die ein Test noch beantworten muss: Führt
/// `myl_net::anfrage::MAX_ANFRAGE_BYTES` weiterhin auf dieselbe Zahl?
/// **Eine Wiederausfuhr kann jederzeit wieder zu einer eigenen
/// Konstante werden**, und dann liefen zwei Zahlen auseinander, ohne
/// dass irgendwo etwas rot würde.
///
/// ⚑ **Und deshalb ist es kein Test, sondern eine Zusicherung des
/// Übersetzers.** Beide Seiten sind Konstanten; ein `assert!` darüber
/// liefe erst, wenn jemand die Testreihe fährt, und clippy sagt zu
/// Recht, dass die Zusicherung einen festen Wert hat. Ein `const _`
/// lässt sich nicht filtern und nicht vergessen.
const _: () = assert!(
    myl_net::anfrage::MAX_ANFRAGE_BYTES == myl_types::protocol::MAX_ANFRAGE_BYTES,
    "myl-net fuehrt eine eigene Leitungsgrenze"
);

/// ⚑ **Eine Inferenzantwort hebt die laufende Blocknachforderung nicht auf.**
///
/// Beide gehen über dieselbe Anfrageschiene, und der Empfänger löscht
/// beim Eintreffen einer Antwort zuerst `nachforderung_laeuft`, weil
/// die erwartete Antwort die Blocklieferung ist. Träfe dazwischen eine
/// Inferenzantwort ein, wäre die Nachforderung als erledigt gebucht,
/// **obwohl die Blöcke noch unterwegs sind**: Der Knoten fragte
/// dieselben Blöcke ein zweites Mal an, und der Nachbar lieferte sie
/// ein zweites Mal.
///
/// Wer beides über eine Schiene schickt, muss beim Lesen wieder
/// trennen. Genau diese Trennung steht hier auf dem Prüfstand.
#[tokio::test]
async fn eine_inferenzantwort_beendet_die_blocknachforderung_nicht() {
    use borsh::BorshSerialize;
    use myl_node::nachschub::Nachlieferung;
    use myl_types::inferenzauftrag::Inferenzantwort;

    let verz = arbeitsverzeichnis("aufholen");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![]), false)
        .await
        .expect("Alpha startet");
    let mut beta = Knoten::starten(konfig(&verz, "beta", vec![]), false)
        .await
        .expect("Beta startet");
    let alpha_id = alpha.peer_id();

    // Alpha zieht davon. Beta bleibt bei null.
    //
    // `erzeuge_block` meldet, ob der Block **verbreitet** wurde, nicht
    // ob er entstand: Ohne Mesh-Nachbarn ist das `false`, und die Kette
    // waechst trotzdem. Deshalb steht die Zusicherung auf der Hoehe.
    for _ in 0..5 {
        alpha.erzeuge_block().await;
    }
    assert_eq!(alpha.kette_mut().hoehe(), 5, "Alpha ist nicht gewachsen");
    let vorn = alpha
        .kette_mut()
        .bloecke_von_bis(5, 5)
        .pop()
        .expect("Alpha hat einen Block auf Hoehe 5");
    let mut roh = Vec::new();
    vorn.serialize(&mut roh).expect("der Block laesst sich schreiben");

    // ⚑ **Beide Ausloeser durch dieselbe Tuer**, damit die Reihenfolge
    // steht: Ueber echte Sockets waere das Aufholen laengst beendet,
    // bevor der Test es ablesen koennte.
    beta.ereignis_einspeisen(myl_net::NodeEvent::Message(myl_net::InboundMessage {
        topic: myl_net::GossipTopic::Blocks,
        data: roh,
        von: alpha_id,
    }));
    beta.aufnahme().await;
    assert!(
        beta.beobachtungsstelle().holen().nachforderung_laeuft,
        "ein Block aus der Ferne hat keine Nachforderung ausgeloest"
    );

    // Jetzt die Inferenzantwort **vor** der Blocklieferung.
    let bytes = Nachlieferung::Inferenz(Inferenzantwort::Abgelehnt { sitzung: 7 })
        .als_bytes()
        .expect("die Antwort laesst sich schreiben");
    beta.ereignis_einspeisen(myl_net::NodeEvent::AntwortEingegangen {
        von: alpha_id,
        daten: bytes,
    });
    assert_eq!(
        beta.letzte_inferenzantwort(),
        Some(&Inferenzantwort::Abgelehnt { sitzung: 7 }),
        "die Inferenzantwort kam gar nicht an"
    );
    beta.aufnahme().await;
    assert!(
        beta.beobachtungsstelle().holen().nachforderung_laeuft,
        "die Inferenzantwort hat die laufende Blocknachforderung geloescht"
    );

    // Gegenprobe: Eine **Blocklieferung** beendet sie sehr wohl, sonst
    // prueft der Test nur, dass das Feld nie zurueckgesetzt wird.
    let lieferung = Nachlieferung::Bloecke(Vec::new())
        .als_bytes()
        .expect("die Lieferung laesst sich schreiben");
    beta.ereignis_einspeisen(myl_net::NodeEvent::AntwortEingegangen {
        von: alpha_id,
        daten: lieferung,
    });
    beta.aufnahme().await;
    assert!(
        !beta.beobachtungsstelle().holen().nachforderung_laeuft,
        "eine Blocklieferung hat die Nachforderung nicht beendet"
    );

    let _ = std::fs::remove_dir_all(&verz);
}
