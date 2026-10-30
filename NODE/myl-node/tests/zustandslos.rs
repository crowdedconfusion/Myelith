//! ⚑ **Ein zustandsloser Knoten folgt über das Netz** (zustandslose
//! Validierung, 2026-10-29): Alpha erzeugt Blöcke und verbreitet zu jedem
//! seinen Zeugen in unterschriebenen Stücken; Beta hält keine Konten und
//! übernimmt jeden Block mit seinem Zeugen. Gamma, ein voller Knoten,
//! bekommt keine Zeugenstücke.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

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
        protokoll_mb: myl_node::protokoll::GESAMT_HOECHSTENS / (1024 * 1024),
        kette_mb: 2048,
        archiv_ausfuhr: None,
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
        speicherorte: None,
        netz: &myl_node::netz::PROBEKETTE,
        modellzusage: None,
        zone: myl_types::node_metadata::GeoRegion::Europe,
        shard: None,
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
        modellname: "sparrow-0.6b-int8".to_string(),
        kontoschluesseldatei: None,
        konto: None,
        kontowechseldatei: None,
        podrelais: None,
        podrelaisausweis: None,
        // Kein Testverkehr: Die Tests hier schicken gezielt, damit
        // sichtbar bleibt, welche Nachricht wessen ist.
        testverkehr_sekunden: None,
        einsatz: None,
        zustandslos: false,
        baum_auf_platte: false,
        zeugen_bereithalten: false,
        erzeugt_bloecke: false,
        blocktakt_ms: None,
        abzug: None,
        hintergrundpruefung: true,
        // Ein Probelauf mit Attest-Prüfung: beide Knoten kennen einander.
        teilnehmer: vec!["alpha".into(), "beta".into()],
    }
}

#[tokio::test]
async fn ein_zustandsloser_knoten_folgt_mit_zeugen() {
    let verz = arbeitsverzeichnis("zustandslos");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(100);
    // Testverkehr: Transaktionen, die Konten berühren.
    k.testverkehr_sekunden = Some(1);
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    let mut gamma = Knoten::starten(konfig(&verz, "gamma", vec![adresse.to_string()]), false).await.expect("Gamma startet");
    assert!(beta.kette().ist_zustandslos());
    assert!(beta.warte_auf_peers(1, Duration::from_secs(20)).await >= 1, "Beta hat Alpha nicht erreicht");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;

    let bis = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        alpha.laufe_fuer(Duration::from_millis(50)).await;
        beta.laufe_fuer(Duration::from_millis(50)).await;
        gamma.laufe_fuer(Duration::from_millis(50)).await;
        if beta.kette().hoehe() >= 30 && alpha.kette().zustand().accounts.anzahl() > 0 {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "Beta folgte nicht: {} gegen {}", beta.kette().hoehe(), alpha.kette().hoehe());
    }
    // Gleichstand herstellen: Alpha hält an, Beta holt auf.
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while beta.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(50)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf: {} gegen {ziel}", beta.kette().hoehe());
    }
    // Alpha läuft nur in `laufe_fuer`; seit dem Ziel hat er nichts gebaut.
    assert_eq!(alpha.kette().hoehe(), ziel);
    assert_eq!(beta.kette().hoehe(), ziel);
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "die Wurzeln laufen auseinander");
    assert_eq!(beta.kette().zustand().accounts.anzahl(), 0, "Beta hält keine Konten");
    assert_eq!(alpha.kette().zeugenlos(), 0, "ein Block war nicht bezeugbar");

    let protokoll = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(protokoll.contains("\"mit_zeuge\":true"), "Beta übernahm keinen Block mit Zeugen");
    assert!(!protokoll.contains("zeuge-traegt-nicht"), "ein Zeuge trug nicht");
    // Gamma ist voll und abonniert die Zeugen nicht.
    let g = std::fs::read_to_string(gamma.protokollpfad()).expect("Protokoll");
    assert!(!g.contains("\"topic\":\"Zeugen\""), "Gamma bekam Zeugenstücke");
    let b = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(b.contains("\"topic\":\"Zeugen\""), "Beta bekam keine Zeugenstücke über das Gossip");
    assert!(gamma.kette().hoehe() > 0, "Gamma folgte nicht");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Ein zustandsloser Knoten holt auf**: Beta kommt dazu, als Alpha
/// schon Blöcke hat; die Blöcke holt er wie jeder Knoten nach, die Zeugen
/// gebündelt beim Erzeuger. Danach folgt er über das Gossip.
#[tokio::test]
async fn ein_zustandsloser_knoten_holt_mit_zeugen_auf() {
    let verz = arbeitsverzeichnis("zustandslos-auf");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(100);
    k.testverkehr_sekunden = Some(1);
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    // Der Erzeuger baut erst, wenn jemand zuhört: ein voller Knoten.
    let mut gamma = Knoten::starten(konfig(&verz, "gamma", vec![adresse.to_string()]), false).await.expect("Gamma startet");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while alpha.kette().hoehe() < 25 {
        alpha.laufe_fuer(Duration::from_millis(50)).await;
        gamma.laufe_fuer(Duration::from_millis(50)).await;
        assert!(tokio::time::Instant::now() < bis, "Alpha baute nicht: {}", alpha.kette().hoehe());
    }
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    assert!(beta.warte_auf_peers(1, Duration::from_secs(20)).await >= 1, "Beta hat Alpha nicht erreicht");
    let bis = tokio::time::Instant::now() + Duration::from_secs(40);
    loop {
        alpha.laufe_fuer(Duration::from_millis(50)).await;
        beta.laufe_fuer(Duration::from_millis(50)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        if beta.kette().hoehe() >= 40 {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf: {} gegen {}", beta.kette().hoehe(), alpha.kette().hoehe());
    }
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while beta.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(50)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf: {} gegen {ziel}", beta.kette().hoehe());
    }
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "die Wurzeln laufen auseinander");
    assert_eq!(beta.kette().zustand().accounts.anzahl(), 0);
    let protokoll = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(protokoll.contains("zeugenstuecke_nachgeliefert"), "keine Zeugen nachgefordert");
    assert!(!protokoll.contains("zeuge-traegt-nicht"), "ein Zeuge trug nicht");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⛔️ **Ein Block ohne belegten Erzeuger löst nichts aus**: Ein frischer
/// zustandsloser Knoten bekommt einen gefälschten Block der Höhe 500, von
/// einem fremden Schlüssel unterschrieben. Er lehnt ab, fordert nichts nach
/// und erwartet keinen Zeugen dafür.
#[tokio::test]
async fn ein_gefaelschter_block_loest_beim_zustandslosen_nichts_aus() {
    use myl_consensus::block::{Block, BlockHeader};
    let verz = arbeitsverzeichnis("zustandslos-faelschung");
    let mut b = konfig(&verz, "beta", vec![]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    // Ein fester Stimmsatz, in dem der Fälscher nicht steht. (Eine Kette, die
    // ihren Erzeuger erst lernt, hält wie ein voller Knoten jeden
    // unterschriebenen Block für belegt.)
    let echt = myl_types::bls::BlsSecretKey::key_gen(&[8u8; 32]).unwrap().public_key().unwrap();
    let mut mitglieder = std::collections::BTreeMap::new();
    mitglieder.insert(myl_types::MinerId::aus_schluessel(&echt), myl_consensus::validator::VotingMember { pubkey: echt, weight: 1 });
    beta.kette_mut().stimmsatz_setzen(myl_consensus::validator::VotingSet::from_members(mitglieder));
    let mut block = Block::new(BlockHeader {
        height: 500,
        epoch: 0,
        prev_block_hash: myl_types::Hash::sha256(b"irgendwas"),
        timestamp_ms: 1,
        state_root: myl_types::Hash::sha256(b"zustand"),
        pruefpunkt: None,
    });
    let fremd = myl_types::bls::BlsSecretKey::key_gen(&[9u8; 32]).unwrap();
    block.unterschreiben(&fremd).unwrap();
    let von = libp2p::PeerId::random();
    beta.ereignis_einspeisen(myl_net::NodeEvent::Message(myl_net::InboundMessage {
        topic: myl_net::GossipTopic::Blocks,
        data: borsh::to_vec(&block).unwrap(),
        von,
    }));
    beta.laufe_fuer(Duration::from_millis(300)).await;
    let protokoll = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(!protokoll.contains("nachschub_angefordert"), "ein gefälschter Block löste eine Nachforderung aus");
    assert!(!protokoll.contains("zeugenstuecke_angefragt"));
    assert!(protokoll.contains("erzeuger-unbelegt"), "nicht als unbelegt abgelehnt");
    assert_eq!(beta.kette().hoehe(), 0);
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Einstieg über einen leichten Abzug** (2026-10-29): Alpha (voll, mit
/// Kettendatei) legt an jedem Prüfpunkt einen leichten Abzug ab. Beta kommt
/// spät, ohne Kettendatei, mit dem Anker des Betreibers bei Höhe 150: Er
/// holt den leichten Abzug (keine Konten darin), rechnet ab dort mit Zeugen
/// nach und erfährt sein eigenes Konto über einen Kontobeweis, den er
/// gegen seine eigene Wurzel prüft.
#[tokio::test]
async fn ein_zustandsloser_knoten_steigt_ueber_einen_leichten_abzug_ein() {
    let verz = arbeitsverzeichnis("zustandslos-leicht");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(50);
    k.testverkehr_sekunden = Some(1);
    k.kettendatei = Some(verz.join("alpha.kette"));
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut gamma = Knoten::starten(konfig(&verz, "gamma", vec![adresse.to_string()]), false).await.expect("Gamma startet");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while alpha.kette().hoehe() < 170 {
        alpha.laufe_fuer(Duration::from_millis(50)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Alpha baute nicht: {}", alpha.kette().hoehe());
    }
    let block150 = alpha.kette_mut().bloecke_von_bis(150, 150).pop().expect("Block 150");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    b.kettendatei = Some(verz.join("beta.kette"));
    b.abzug = Some(myl_node::abzug::Anker { hoehe: 150, blockhash: block150.hash() });
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    assert!(beta.warte_auf_peers(1, Duration::from_secs(20)).await >= 1);
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        alpha.laufe_fuer(Duration::from_millis(50)).await;
        beta.laufe_fuer(Duration::from_millis(50)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        // Alle eigenen Konten (mit den Probekonten des Probenetzes) kommen
        // in Bündeln zu acht.
        if beta.kette().hoehe() >= alpha.kette().hoehe().saturating_sub(2) && beta.kette().eigene_unbekannt().is_empty() {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "Beta stieg nicht ein: {} gegen {}", beta.kette().hoehe(), alpha.kette().hoehe());
    }
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while beta.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(50)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf");
    }
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "die Wurzeln laufen auseinander");
    assert_eq!(beta.kette().zustand().accounts.anzahl(), 0);
    let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(p.contains("\"leicht\":true"), "kein leichter Abzug");
    // Mindestens eine Lieferung setzte etwas ein (eine andere kann null
    // einsetzen, wenn ein Zeuge die Konten schon brachte).
    assert!(
        p.lines().any(|z| z.contains("\"art\":\"kontobeweise\"") && !z.contains("\"eingesetzt\":0")),
        "kein Kontobeweis eingesetzt"
    );
    assert!(!verz.join("beta.kette").exists(), "ein zustandsloser Knoten führt keine Kettendatei");
    let eigenes = myl_node::kette::konto_fuer("beta");
    assert!(beta.kette().konto_bekannt(&eigenes));
    assert_eq!(beta.kette().konto(&eigenes), alpha.kette().zustand().account(&eigenes));
    std::fs::remove_dir_all(&verz).ok();
}

/// ⛔️ **Eine erfundene ferne Höhe belegt den Platz nicht**: Kontobeweise
/// weit voraus werden nicht gehalten, und eine nähere Lieferung verdrängt
/// eine fernere.
#[tokio::test]
async fn erfundene_kontobeweise_belegen_den_platz_nicht() {
    let verz = arbeitsverzeichnis("zustandslos-kontobeweise");
    let mut b = konfig(&verz, "beta", vec![]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    beta.kontobeweise_einspeisen(u64::MAX, Vec::new());
    assert_eq!(beta.kontobeweise_gehalten(), None, "eine ferne Höhe wird nicht gehalten");
    beta.kontobeweise_einspeisen(40, Vec::new());
    beta.kontobeweise_einspeisen(30, Vec::new());
    assert_eq!(beta.kontobeweise_gehalten(), Some(30), "die nähere gilt");
    beta.kontobeweise_einspeisen(50, Vec::new());
    assert_eq!(beta.kontobeweise_gehalten(), Some(30), "eine fernere verdrängt sie nicht");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Selbstheilung** (2026-10-29): Der Erzeuger hält eine Weile seine
/// Zeugen zurück, wie ein böswilliger; Beta (zustandslos, mit Stimmsatz)
/// steht still, sucht nach der Frist einen jüngeren verankerten leichten
/// Abzug (ein Prüfpunktzertifikat gegen seinen Stimmsatz), springt dorthin
/// und folgt wieder. Am Ende dieselbe Wurzel.
#[tokio::test]
async fn ein_zustandsloser_knoten_heilt_sich_ueber_einen_juengeren_anker() {
    // Die Probekette lernt ihren Stimmsatz (den Erzeuger) aus Block 1, auch
    // Beta; Alpha bestätigt seine Prüfpunkte darin allein.
    let verz = arbeitsverzeichnis("zustandslos-heilung");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(40);
    k.kettendatei = Some(verz.join("alpha.kette"));
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let g = konfig(&verz, "gamma", vec![adresse.to_string()]);
    let mut gamma = Knoten::starten(g, false).await.expect("Gamma startet");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    beta.stillstand_hoechstens_setzen(Duration::from_secs(3));
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    beta.warte_auf_peers(1, Duration::from_secs(20)).await;
    // Bis Beta folgt.
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while beta.kette().hoehe() < 40 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta folgte nicht: {}", beta.kette().hoehe());
    }
    // Zurückhalten, bis zwei Prüfpunkte darüber liegen.
    alpha.zeugen_zurueckhalten(true);
    let stand = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(90);
    while alpha.kette().hoehe() < stand + 2 * myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Alpha baute nicht");
    }
    // Weiter kam Beta ohne Zeugen nur über einen leichten Abzug.
    if beta.kette().hoehe() > stand + 1 {
        let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
        assert!(p.contains("\"leicht\":true"), "ohne Zeugen und ohne leichten Abzug weiter: {}", beta.kette().hoehe());
    }
    alpha.zeugen_zurueckhalten(false);
    let bis = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        if beta.kette().hoehe() + 2 >= alpha.kette().hoehe() && beta.kette().hoehe() > stand + 150 {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "Beta heilte nicht: {} gegen {}", beta.kette().hoehe(), alpha.kette().hoehe());
    }
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while beta.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(40)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf");
    }
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "die Wurzeln laufen auseinander");
    let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(p.contains("zustandslos_neu_verankern"), "keine Selbstheilung");
    assert!(p.contains("\"leicht\":true"), "kein leichter Abzug");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Ein Erzeuger mit dem Baum auf der Platte** (2026-10-30): Alpha lagert
/// den Zustandsbaum aus; Gamma (voll, im Speicher) und Beta (zustandslos,
/// mit den Zeugen aus dem ausgelagerten Baum) folgen mit derselben Wurzel.
#[tokio::test]
async fn ein_erzeuger_mit_dem_baum_auf_der_platte() {
    let verz = arbeitsverzeichnis("baum-platte");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(40);
    k.testverkehr_sekunden = Some(1);
    k.kettendatei = Some(verz.join("alpha.kette"));
    k.baum_auf_platte = true;
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut gamma = Knoten::starten(konfig(&verz, "gamma", vec![adresse.to_string()]), false).await.expect("Gamma startet");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    beta.warte_auf_peers(1, Duration::from_secs(20)).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(90);
    while alpha.kette().hoehe() < 400 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Alpha baute nicht: {}", alpha.kette().hoehe());
    }
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while beta.kette().hoehe() < ziel || gamma.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(40)).await;
        assert!(tokio::time::Instant::now() < bis, "die Folger holten nicht auf: {} {} gegen {ziel}", beta.kette().hoehe(), gamma.kette().hoehe());
    }
    let w = alpha.kette().zustandswurzel();
    assert_eq!(gamma.kette().zustandswurzel(), w, "voll im Speicher");
    assert_eq!(beta.kette().zustandswurzel(), w, "zustandslos mit Zeugen");
    assert!(std::fs::metadata(verz.join("alpha.baum")).map(|m| m.len() > 0).unwrap_or(false), "keine Seiten auf der Platte");
    assert!(alpha.kette().zustand().baum_groessen().1 > 0, "nichts ausgelagert");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Ein voller Knoten hält Zeugen bereit** (2026-10-30): Gamma ist als
/// Miner eingetragen und erzeugt die Zeugen der Blöcke, die er anwendet.
/// Hält Alpha seine zurück, folgt Beta mit Gammas Zeugen weiter, ohne
/// leichten Abzug. 📌 Im Podlauf hing ein zustandsloser Knoten fest, als
/// der Erzeuger endete: Nur er hielt Zeugen.
#[tokio::test]
async fn ein_voller_knoten_haelt_zeugen_bereit() {
    let verz = arbeitsverzeichnis("zeugen-bereit");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(40);
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    // Gamma unterschreibt mit dem Schlüssel des Probekontos 1, das Alpha
    // als Miner einträgt.
    alpha.kette_mut().aufnehmen(myl_node::kette::probeanmeldung(1));
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut gamma = Knoten::starten(konfig(&verz, "gamma", vec![adresse.to_string()]), false).await.expect("Gamma startet");
    let schluesselpfad = verz.join("gamma-zeugen.key");
    let ikm = myl_types::Hash::sha256(b"myelith-probekonto-1");
    let hex: String = ikm.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    myl_types::geheimdatei::schreiben(&schluesselpfad, format!("{}\n{hex}\n", myl_node::schluessel::DATEIKOPF).as_bytes()).expect("Schlüsseldatei");
    let schluessel = myl_node::schluessel::Konsensschluessel::aus_datei(&schluesselpfad).expect("Schlüssel");
    assert_eq!(schluessel.oeffentlich(), myl_node::kette::probeschluessel(1).public_key().unwrap());
    gamma.kette_mut().zeugen_bereithalten_setzen(schluessel);
    let gamma_adresse = gamma.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string(), gamma_adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    beta.warte_auf_peers(2, Duration::from_secs(20)).await;
    let gamma_kennung = myl_types::MinerId::aus_schluessel(&myl_node::kette::probeschluessel(1).public_key().unwrap());
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while beta.kette().hoehe() < 20 || !beta.kette().zustand().miner.contains_key(&gamma_kennung) {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta folgte nicht, oder Gamma ist nicht eingetragen: {}", beta.kette().hoehe());
    }
    alpha.zeugen_zurueckhalten(true);
    let stand = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(90);
    while beta.kette().hoehe() < stand + 100 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta folgte ohne Alphas Zeugen nicht: {} ab {stand}", beta.kette().hoehe());
    }
    let ziel = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(30);
    while beta.kette().hoehe() < ziel || gamma.kette().hoehe() < ziel {
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(40)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta holte nicht auf: {} gegen {ziel}", beta.kette().hoehe());
    }
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "die Wurzeln laufen auseinander");
    let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(!p.contains("\"leicht\":true"), "Beta brauchte einen leichten Abzug");
    assert!(!p.contains("zeuge_vom_erzeuger"), "ein Zeuge trug nicht");
    let gamma_peer = gamma.peer_id().to_string();
    assert!(
        p.lines().any(|l| l.contains("zeugenstuecke_nachgeliefert") && l.contains(&gamma_peer) && !l.contains("\"tragend\":0")),
        "Gamma lieferte keine tragenden Stücke"
    );
    std::fs::remove_dir_all(&verz).ok();
}

/// ⛔️ **Ein Wegwerfschlüssel liefert keine Zeugen**: Gamma hält Zeugen
/// bereit, ist aber nicht eingetragen. Seine Stücke nimmt Beta nicht an.
#[tokio::test]
async fn ein_unbekannter_unterzeichner_liefert_keine_zeugen() {
    let verz = arbeitsverzeichnis("zeugen-fremd");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(40);
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut g = konfig(&verz, "gamma", vec![adresse.to_string()]);
    g.zeugen_bereithalten = true;
    let mut gamma = Knoten::starten(g, false).await.expect("Gamma startet");
    let gamma_adresse = gamma.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string(), gamma_adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    gamma.warte_auf_peers(1, Duration::from_secs(20)).await;
    beta.warte_auf_peers(2, Duration::from_secs(20)).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while beta.kette().hoehe() < 20 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta folgte nicht: {}", beta.kette().hoehe());
    }
    alpha.zeugen_zurueckhalten(true);
    let stand = beta.kette().hoehe();
    let ende = tokio::time::Instant::now() + Duration::from_secs(6);
    while tokio::time::Instant::now() < ende {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        gamma.laufe_fuer(Duration::from_millis(20)).await;
    }
    assert!(alpha.kette().hoehe() > stand + 50, "Alpha baute nicht");
    assert!(beta.kette().hoehe() <= stand + 2, "Beta kam mit fremden Zeugen weiter: {} ab {stand}", beta.kette().hoehe());
    let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    let gamma_peer = gamma.peer_id().to_string();
    assert!(
        !p.lines().any(|l| l.contains("zeugenstuecke_nachgeliefert") && l.contains(&gamma_peer) && !l.contains("\"tragend\":0")),
        "Beta nahm Gammas Stücke an"
    );
    let g = std::fs::read_to_string(gamma.protokollpfad()).expect("Protokoll");
    assert!(g.contains("zeugen_bereithalten"), "Gamma hielt keine Zeugen bereit");
    std::fs::remove_dir_all(&verz).ok();
}

/// ⚑ **Ein zurückgefallener zustandsloser Knoten holt mit Zeugen auf,
/// während die Kette weiterläuft** (2026-10-30). 📌 Im Kurzlauf
/// `kurz-ablage` folgte der zustandslose Miner nach einer Trennung genau im
/// Takt des Erzeugers, 130 Blöcke dahinter: Ein Block, der ins Wartefenster
/// kam, zählte nicht als geliefert, und die Nachforderung riss ab. Beta
/// steht hier 300 Blöcke still und muss dann ohne Neuverankerung bis an die
/// Spitze kommen, obwohl Alpha weiterbaut.
#[tokio::test]
async fn ein_zurueckgefallener_holt_mit_zeugen_auf_waehrend_die_kette_laeuft() {
    let verz = arbeitsverzeichnis("zustandslos-aufholen");
    let mut k = konfig(&verz, "alpha", vec![]);
    k.erzeugt_bloecke = true;
    k.blocktakt_ms = Some(40);
    let mut alpha = Knoten::starten(k, false).await.expect("Alpha startet");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(10)).await.expect("Adresse");
    let mut b = konfig(&verz, "beta", vec![adresse.to_string()]);
    b.zustandslos = true;
    let mut beta = Knoten::starten(b, false).await.expect("Beta startet");
    beta.warte_auf_peers(1, Duration::from_secs(20)).await;
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while beta.kette().hoehe() < 20 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        assert!(tokio::time::Instant::now() < bis, "Beta folgte nicht: {}", beta.kette().hoehe());
    }
    // Beta steht, Alpha baut 300 Blöcke weiter.
    let stand = alpha.kette().hoehe();
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    while alpha.kette().hoehe() < stand + 300 {
        alpha.laufe_fuer(Duration::from_millis(40)).await;
        assert!(tokio::time::Instant::now() < bis, "Alpha baute nicht");
    }
    // Jetzt beide: Beta muss den Abstand schliessen, während Alpha baut.
    let bis = tokio::time::Instant::now() + Duration::from_secs(25);
    loop {
        alpha.laufe_fuer(Duration::from_millis(20)).await;
        beta.laufe_fuer(Duration::from_millis(40)).await;
        if beta.kette().hoehe() + 5 >= alpha.kette().hoehe() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < bis,
            "Beta schloss nicht auf: {} gegen {}",
            beta.kette().hoehe(),
            alpha.kette().hoehe()
        );
    }
    let p = std::fs::read_to_string(beta.protokollpfad()).expect("Protokoll");
    assert!(!p.contains("zustandslos_neu_verankern"), "Beta kam nur über einen Abzug nach");
    std::fs::remove_dir_all(&verz).ok();
}
