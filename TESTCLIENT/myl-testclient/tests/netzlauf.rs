//! **Der Netzlauf** (Stufe 4 des Dispatchers, 2026-10-22): viele Knoten,
//! mehrere ungleiche Pods, echte Shardprozesse, und Stolpersteine.
//!
//! # ⚑ Was hier läuft
//!
//! Fünfzehn Knoten im selben Prozess, jeder mit eigenem Swarm, verbunden
//! über Bootstrap und Kademlia; einer erzeugt die Blöcke. Vierzehn sind
//! Miner: Sie melden sich über die Kette an, reichen ihre Zusage ein, werden
//! von der Kette in Pods geteilt und starten ihren Shard selbst
//! (`myl-shard` mit dem Probeartefakt). Die Pods sind ungleich: Kapazitäten
//! und Ebenenzeiten unterscheiden sich, also schneidet jeder Pod anders.
//!
//! # ⚑ Die Stolpersteine
//!
//! 1. **Ein Lügner** sagt 1 µs je Ebene an. Die übrigen Mitglieder seines
//!    Pods messen ihn über das Netz (Ping und Zeitprobe), bezeugen, und die
//!    Meldung sperrt ihn für die nächste Epoche.
//! 2. **Ein Schwacher** hält zu wenig für den Kopf; die Pod-Bildung lässt
//!    ihn weichen, wenn er einen Pod unmachbar macht.
//! 3. **Ein Knoten mit Teilablage** hält nur die Ebenen 0 bis 10; er bekommt
//!    nur Ebenen darin und muss seinen Shard daraus starten.
//! 4. **Ein Knoten, dem Dateien fehlen**, ohne Teilablage zu erklären: Er
//!    holt, was ihm für die nächste Epoche fehlt, bei anderen Knoten.
//! 5. **Ein Shard stürzt ab**: Sein Knoten startet ihn bei der nächsten
//!    Aufnahme neu.
//! 6. **Ein Knoten fällt ganz aus** und startet aus seiner Kettendatei und
//!    seinen Schlüsseln neu: Er holt auf und führt seinen Shard wieder.
//!
//! Geprüft wird an jedem Haltepunkt, dass alle Knoten dieselbe Kette
//! haben, dieselbe Zuteilung und dieselben Grenzen sehen, und dass jeder
//! Shard den Bereich seiner Position rechnet.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle, Shardkonfig};
use myl_node::Knoten;
use myl_types::ids::MinerId;
use myl_types::zusage::Modellzusage;

fn wurzel() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn sparrow() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

fn shardprogramm() -> PathBuf {
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("SYSTEM/full-build").join(profil).join("myl-shard")
}

/// Ein Miner des Laufs: Name, Zusage (Ebenen samt Kopf, µs je Ebene,
/// gehaltener Bereich) und wie sein Artefaktordner aussieht.
struct Rolle4 {
    name: &'static str,
    ebenen: u32,
    zeit_us: u64,
    gehalten: Option<(u32, u32)>,
    ordner: Ordner,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ordner {
    /// Das ganze Probeartefakt.
    Ganz,
    /// Nur die Dateien eines Bereichs (Teilablage).
    Teil(u32, u32),
    /// Alles, aber je Ebene fehlt eine Datei (die Norm nach der
    /// Achtsamkeit): Wer so ausgestattet ist, muss für jeden Bereich holen.
    LueckeJeEbene,
}

/// ⚑ **Ungleich mit Absicht**: Zeiten von 20 bis 90 ms je Ebene, damit die
/// Verteilung schneidet, und weit über dem, was eine Ebene des Probemodells
/// im Testbau wirklich braucht, damit nur der Lügner seine Frist reißt.
const MINER: [Rolle4; 14] = [
    Rolle4 { name: "m00", ebenen: 48, zeit_us: 20_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m01", ebenen: 40, zeit_us: 40_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m02", ebenen: 48, zeit_us: 60_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "luegner", ebenen: 48, zeit_us: 1, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m04", ebenen: 30, zeit_us: 30_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m05", ebenen: 48, zeit_us: 90_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m06", ebenen: 44, zeit_us: 25_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m07", ebenen: 48, zeit_us: 50_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m08", ebenen: 36, zeit_us: 35_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "m09", ebenen: 48, zeit_us: 45_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "schwach", ebenen: 3, zeit_us: 30_000, gehalten: None, ordner: Ordner::Ganz },
    Rolle4 { name: "teilablage", ebenen: 48, zeit_us: 30_000, gehalten: Some((0, 10)), ordner: Ordner::Teil(0, 10) },
    Rolle4 { name: "holer", ebenen: 48, zeit_us: 30_000, gehalten: None, ordner: Ordner::LueckeJeEbene },
    Rolle4 { name: "m13", ebenen: 48, zeit_us: 70_000, gehalten: None, ordner: Ordner::Ganz },
];

/// Legt den Artefaktordner eines Miners an: das ganze Artefakt oder einen
/// Teil davon, als Verweise auf das Probeartefakt (nichts wird kopiert).
fn artefaktordner(ziel: &Path, ordner: Ordner) -> PathBuf {
    if ordner == Ordner::Ganz {
        return sparrow();
    }
    std::fs::create_dir_all(ziel).expect("Ordner");
    let manifest = std::fs::read_to_string(sparrow().join("weights_manifest.json")).expect("Manifest");
    let im_manifest: Vec<String> =
        myl_store::teilablage::manifestdateien(&manifest).expect("Manifest").into_iter().map(|(_, d)| d.datei).collect();
    let behalten: Vec<String> = match ordner {
        Ordner::Ganz => unreachable!(),
        Ordner::Teil(v, b) => myl_store::teilablage::dateien_des_bereichs(&manifest, v, b, v == 0, b == 28)
            .expect("Bereich")
            .into_iter()
            .map(|d| d.datei)
            .collect(),
        Ordner::LueckeJeEbene => im_manifest.iter().filter(|d| !d.contains("_post_attention_layernorm_weight")).cloned().collect(),
    };
    for e in std::fs::read_dir(sparrow()).expect("Artefakt").flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // Was nicht im Manifest steht, liegt bei jedem (Beschreibung,
        // Wortschatz, Tabellen); vom Manifest nur der Bereich.
        if !im_manifest.contains(&name) || behalten.contains(&name) {
            #[cfg(unix)]
            std::os::unix::fs::symlink(e.path(), ziel.join(&name)).expect("Verweis");
        }
    }
    ziel.to_path_buf()
}

fn konfig(ordner: &Path, name: &str, bootstrap: Vec<String>, miner: Option<&Rolle4>) -> KnotenKonfig {
    let protokolle = ordner.join(name);
    std::fs::create_dir_all(&protokolle).expect("Ordner");
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: protokolle.join("knoten.key"),
        protokollverzeichnis: protokolle.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        // ⚑ **Mit Kettendatei, wie im Betrieb** (Vorgabe `kette.dat`): Ohne
        //   sie hält ein Knoten nur die letzten Blöcke im Speicher und kann
        //   einem, der weiter zurückliegt, nicht aushelfen.
        kettendatei: Some(protokolle.join("kette.dat")),
        stimmsatzdatei_pfad: None,
        konsensschluesseldatei: miner.map(|_| protokolle.join("konsens.key")),
        nat: Default::default(),
        speicherorte: None,
        netz: &myl_node::netz::PROBEKETTE,
        modellzusage: miner.map(|m| Modellzusage { abdruck: [0; 32], ebenen_hoechstens: m.ebenen, ebenenzeit_us: m.zeit_us, gehalten: m.gehalten }),
        zone: myl_types::node_metadata::GeoRegion::Europe,
        shard: miner.map(|m| Shardkonfig {
            programm: shardprogramm(),
            artefakte: artefaktordner(&protokolle.join("artefakt"), m.ordner),
            training: false,
        }),
        aufnahme_sekunden: 1,
        beobachtung: None,
        tuer: None,
        ortsleitung: None,
        ortsausweis: None,
        pod: None,
        modellname: "sparrow-0.6b-int8".to_string(),
        kontoschluesseldatei: None,
        konto: None,
        testverkehr_sekunden: None,
        erzeugt_bloecke: miner.is_none(),
        teilnehmer: Vec::new(),
    }
}

/// Alle Knoten reihum, je eine Scheibe; der Erzeuger baut dabei `bloecke`.
async fn runde(knoten: &mut [Knoten], bloecke: usize) {
    for _ in 0..bloecke {
        knoten[0].erzeuge_block().await;
    }
    for k in knoten.iter_mut() {
        k.laufe_fuer(Duration::from_millis(25)).await;
    }
}

/// Bis alle Knoten auf derselben Höhe stehen (oder die Frist abläuft).
async fn angleichen(knoten: &mut [Knoten], frist: Duration) -> bool {
    let ende = tokio::time::Instant::now() + frist;
    while tokio::time::Instant::now() < ende {
        let h = knoten[0].kette().hoehe();
        if knoten.iter().all(|k| k.kette().hoehe() == h) {
            return true;
        }
        runde(knoten, 0).await;
    }
    false
}

/// ⚑ **Alle dieselbe Kette, dieselbe Zuteilung, dieselben Grenzen**, und
/// jeder rechnende Miner führt genau den Shard seiner Position.
fn pruefe_einigkeit(knoten: &[Knoten], wo: &str) {
    let w = knoten[0].kette().zustandswurzel();
    for k in knoten {
        assert_eq!(k.kette().zustandswurzel(), w, "{wo}: die Ketten laufen auseinander");
    }
    let z0 = myl_node::kette::Kette::zuteilung_der_laufenden_epoche(knoten[0].kette().zustand());
    for k in &knoten[1..] {
        assert_eq!(myl_node::kette::Kette::zuteilung_der_laufenden_epoche(k.kette().zustand()), z0, "{wo}: Zuteilung");
    }
}

/// Wie oft ein Ereignis im Protokoll eines Knotens steht.
fn im_protokoll(ordner: &Path, name: &str, art: &str) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(ordner.join(name)).expect("Protokollordner").flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            n += text.matches(&format!("\"art\":\"{art}\"")).count();
        }
    }
    n
}

/// Was jeder Knoten eines Pods führen soll, nach der Kette des Erzeugers:
/// eine Position ihren Bereich in der laufenden Epoche, die Reserve nichts.
type Soll = Vec<(MinerId, Option<(u64, u32, u32)>)>;

fn soll_der_pods(knoten: &[Knoten]) -> Soll {
    let zustand = knoten[0].kette().zustand().clone();
    let z = myl_node::kette::Kette::zuteilung_der_laufenden_epoche(&zustand);
    let mut soll = Soll::new();
    for p in &z.pods {
        let g = myl_node::kette::Kette::grenzen_des_pods(&zustand, p).expect("machbar");
        for (i, s) in p.shards.iter().enumerate() {
            soll.push((s.miner.miner_id, Some((zustand.epoch.0, g[i].0, g[i].1))));
        }
        soll.extend(p.reserve.iter().map(|r| (r.miner_id, None)));
    }
    soll
}

/// 📌 **Nicht „irgendein Auftrag“** (2026-10-22): Die Warteschleife fragte
/// zuerst nur, ob jede Position einen Auftrag hat. Ein Knoten mit dem
/// Auftrag der vorigen Epoche bestand sie, und die Prüfung danach fand ihn
/// mit dem alten Bereich; der Wechsel kommt erst mit seiner nächsten
/// Aufnahme.
fn fuehren_ihr_soll(knoten: &[Knoten], soll: &Soll) -> bool {
    soll.iter().all(|(m, s)| {
        knoten
            .iter()
            .find(|k| k.miner_kennung() == Some(*m))
            .is_some_and(|k| k.shardauftrag().map(|a| (a.epoche, a.von, a.bis)) == *s)
    })
}

fn namen(knoten: &[Knoten]) -> BTreeMap<MinerId, String> {
    knoten
        .iter()
        .enumerate()
        .filter_map(|(i, k)| Some((k.miner_kennung()?, if i == 0 { "erzeuger".into() } else { MINER[i - 1].name.to_string() })))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn der_netzbetrieb_traegt_mit_ungleichen_pods_und_stolpersteinen() {
    // Ohne Probeartefakt nur mit ausdrücklicher Erlaubnis (Fund 113); die
    // CI setzt sie, denn dort liegt keines.
    if !myl_pod::artefakte::vorhanden(&sparrow()) {
        return;
    }
    assert!(shardprogramm().exists(), "myl-shard fehlt: cd COMPUTE_PIPELINE/myl-pod && cargo test --no-run");
    let ordner = std::env::temp_dir().join(format!("myl-netzlauf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("Ordner");

    // ---- Aufbau: ein Erzeuger, vierzehn Miner, Stern über den Erzeuger --
    let mut knoten: Vec<Knoten> = Vec::new();
    let mut erzeuger = Knoten::starten(konfig(&ordner, "erzeuger", vec![], None), false).await.expect("Erzeuger");
    let adresse = erzeuger.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let bootstrap = vec![format!("{}/p2p/{}", adresse, erzeuger.peer_id())];
    knoten.push(erzeuger);
    for m in &MINER {
        knoten.push(Knoten::starten(konfig(&ordner, m.name, bootstrap.clone(), Some(m)), false).await.expect(m.name));
    }
    for _ in 0..40 {
        runde(&mut knoten, 0).await;
    }
    let verbunden: Vec<usize> = {
        let mut v = Vec::new();
        for k in &knoten {
            v.push(k.peers().await);
        }
        v
    };
    eprintln!("[netzlauf] Peers je Knoten: {verbunden:?}");
    assert!(verbunden.iter().all(|n| *n >= 1), "ein Knoten blieb allein: {verbunden:?}");

    // ---- Epoche 0: Anmeldung, dann Zusage, über die Kette ---------------
    for _ in 0..30 {
        runde(&mut knoten, 2).await;
    }
    assert!(angleichen(&mut knoten, Duration::from_secs(60)).await, "die Knoten holten nicht auf");
    let angemeldet = knoten[0].kette().zustand().miner.len();
    let zusagen = knoten[0].kette().zustand().zusagen.len();
    eprintln!("[netzlauf] angemeldet {angemeldet}, Zusagen {zusagen}, Höhe {}", knoten[0].kette().hoehe());
    assert_eq!(angemeldet, 14, "nicht alle Miner kamen über die Kette an");
    assert_eq!(zusagen, 14, "nicht alle Zusagen kamen über die Kette an");
    pruefe_einigkeit(&knoten, "nach den Zusagen");

    // ---- Bis Epoche 2: die ersten Pods ----------------------------------
    while knoten[0].kette().zustand().epoch.0 < 2 {
        runde(&mut knoten, 200).await;
    }
    assert!(angleichen(&mut knoten, Duration::from_secs(120)).await, "die Knoten holten nicht auf");
    pruefe_einigkeit(&knoten, "Beginn Epoche 2");
    let n = namen(&knoten);
    let zustand = knoten[0].kette().zustand().clone();
    let z = myl_node::kette::Kette::zuteilung_der_laufenden_epoche(&zustand);
    for p in &z.pods {
        let g = myl_node::kette::Kette::grenzen_des_pods(&zustand, p).expect("machbar");
        let pos: Vec<&str> = p.shards.iter().map(|s| n[&s.miner.miner_id].as_str()).collect();
        let res: Vec<&str> = p.reserve.iter().map(|r| n[&r.miner_id].as_str()).collect();
        eprintln!("[netzlauf] Pod {}: Positionen {pos:?}, Grenzen {g:?}, Reserve {res:?}", p.pod_index);
    }
    let ohne: Vec<&str> = z.ohne_pod.iter().map(|m| n[&m.miner_id].as_str()).collect();
    eprintln!("[netzlauf] ohne Pod: {ohne:?}");
    assert_eq!(z.pods.len(), 2, "vierzehn Miner, zwei Pods");
    let grenzen: Vec<Vec<(u32, u32)>> =
        z.pods.iter().map(|p| myl_node::kette::Kette::grenzen_des_pods(&zustand, p).expect("machbar")).collect();
    assert_ne!(grenzen[0], grenzen[1], "die Pods sind ungleich geschnitten");

    // Die Shards laufen mit den Bereichen ihrer Positionen.
    let soll = soll_der_pods(&knoten);
    let mut bis = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        runde(&mut knoten, 1).await;
        if fuehren_ihr_soll(&knoten, &soll) {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "nicht jede Position führt ihren Shard");
    }
    for (p, g) in z.pods.iter().zip(&grenzen) {
        for (i, s) in p.shards.iter().enumerate() {
            let k = knoten.iter().find(|k| k.miner_kennung() == Some(s.miner.miner_id)).expect("Knoten");
            let a = k.shardauftrag().expect("Auftrag");
            assert_eq!((a.von, a.bis), g[i], "{} rechnet nicht seinen Bereich", n[&s.miner.miner_id]);
        }
        for r in &p.reserve {
            let k = knoten.iter().find(|k| k.miner_kennung() == Some(r.miner_id)).expect("Knoten");
            assert!(k.shardauftrag().is_none(), "die Reserve rechnet nicht");
        }
    }

    // ---- Stolperstein 5: ein Shard stürzt ab ----------------------------
    let opfer = knoten.iter().position(|k| k.shard_pid().is_some()).expect("ein Shard");
    let pid = knoten[opfer].shard_pid().expect("pid");
    let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
    bis = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        runde(&mut knoten, 1).await;
        if knoten[opfer].shard_pid().is_some_and(|neu| neu != pid) {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "der abgestürzte Shard wurde nicht neu gestartet");
    }
    eprintln!("[netzlauf] Shard von {} nach Absturz neu gestartet", MINER[opfer - 1].name);

    // ---- Stolperstein 1: der Lügner wird gemessen und gesperrt ---------
    let luegner = knoten[1 + MINER.iter().position(|m| m.name == "luegner").unwrap()].miner_kennung().expect("Lügner");
    let lebt_im_pod = z.pods.iter().any(|p| p.shards.iter().any(|s| s.miner.miner_id == luegner));
    let epoche = knoten[0].kette().zustand().epoch.0;
    if lebt_im_pod {
        bis = tokio::time::Instant::now() + Duration::from_secs(180);
        loop {
            runde(&mut knoten, 2).await;
            if knoten[0].kette().zustand().fristsperren.contains_key(&luegner) {
                break;
            }
            assert!(tokio::time::Instant::now() < bis, "der Lügner wurde nicht gemeldet");
        }
        eprintln!("[netzlauf] Lügner gemeldet bei Höhe {}", knoten[0].kette().hoehe());
    }

    // ---- Stolperstein 6: ein Knoten fällt ganz aus und kehrt zurück -----
    let ausfall = 1 + MINER.iter().position(|m| m.name == "m13").unwrap();
    let kennung_vorher = knoten[ausfall].miner_kennung();
    let weg = knoten.remove(ausfall);
    drop(weg);
    for _ in 0..10 {
        runde(&mut knoten, 20).await;
    }
    let zurueck = Knoten::starten(konfig(&ordner, "m13", bootstrap.clone(), Some(&MINER[ausfall - 1])), false)
        .await
        .expect("Neustart");
    assert_eq!(zurueck.miner_kennung(), kennung_vorher, "nach dem Neustart ein anderer Miner");
    knoten.insert(ausfall, zurueck);
    assert!(angleichen(&mut knoten, Duration::from_secs(120)).await, "der neu gestartete Knoten holte nicht auf");
    eprintln!("[netzlauf] m13 nach Neustart bei Höhe {}", knoten[ausfall].kette().hoehe());

    // ---- Epoche 3: ohne den Lügner, Dateien geholt ----------------------
    while knoten[0].kette().zustand().epoch.0 <= epoche {
        runde(&mut knoten, 200).await;
    }
    assert!(angleichen(&mut knoten, Duration::from_secs(120)).await, "die Knoten holten nicht auf");
    pruefe_einigkeit(&knoten, "Beginn Epoche 3");
    let zustand = knoten[0].kette().zustand().clone();
    let z3 = myl_node::kette::Kette::zuteilung_der_laufenden_epoche(&zustand);
    if lebt_im_pod {
        assert!(z3.pods.iter().all(|p| p.mitglieder().all(|m| m.miner_id != luegner)), "der Lügner steht wieder in einem Pod");
    }
    // Stolperstein 3: Teilablage im eigenen Teil.
    let teil = knoten[1 + MINER.iter().position(|m| m.name == "teilablage").unwrap()].miner_kennung().expect("Teilablage");
    for p in &z3.pods {
        if let Some(i) = p.shards.iter().position(|s| s.miner.miner_id == teil) {
            let g = myl_node::kette::Kette::grenzen_des_pods(&zustand, p).expect("machbar");
            assert!(g[i].1 <= 10, "Teilablage außerhalb ihres Teils: {g:?}");
        }
    }
    // Alle Positionen führen ihren Shard, auch Teilablage und Holer.
    let soll3 = soll_der_pods(&knoten);
    bis = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        runde(&mut knoten, 1).await;
        if fuehren_ihr_soll(&knoten, &soll3) {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "in Epoche 3 führt nicht jede Position ihren Shard");
    }
    pruefe_einigkeit(&knoten, "Ende");
    // Nur der Lügner ist gesperrt; kein ehrlicher Knoten riss seine Frist.
    let gesperrt: Vec<MinerId> = knoten[0].kette().zustand().fristsperren.keys().copied().collect();
    let erwartet: Vec<MinerId> = if lebt_im_pod { vec![luegner] } else { Vec::new() };
    assert_eq!(gesperrt, erwartet, "gesperrt: {:?}", gesperrt.iter().map(|m| namen(&knoten)[m].clone()).collect::<Vec<_>>());
    // Was die Stolpersteine im Protokoll hinterlassen haben.
    for m in &MINER {
        let zaehle = |a: &str| im_protokoll(&ordner, m.name, a);
        eprintln!(
            "[netzlauf] {}: Shard gestartet {}, Messungen {} (verworfen {}), Meldungen {}, Dateien geholt {}, verworfen {}, gewartet {}",
            m.name,
            zaehle("shard_gestartet"),
            zaehle("fristmessung"),
            zaehle("fristmessung_verworfen"),
            zaehle("fristmeldung"),
            zaehle("datei_geholt"),
            zaehle("datei_verworfen"),
            zaehle("shard_wartet_auf_dateien")
        );
    }
    assert!(im_protokoll(&ordner, "holer", "datei_geholt") > 0, "der Holer holte keine Datei");
    assert_eq!(MINER.iter().map(|m| im_protokoll(&ordner, m.name, "datei_verworfen")).sum::<usize>(), 0, "eine Lieferung passte nicht");
    assert!(MINER.iter().map(|m| im_protokoll(&ordner, m.name, "fristmessung")).sum::<usize>() >= 10, "kaum gemessen");
    let _ = std::fs::remove_dir_all(&ordner);
}
