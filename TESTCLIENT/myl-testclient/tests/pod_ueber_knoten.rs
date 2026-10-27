//! **Ein Pod über Knoten** (2026-10-26).
//!
//! # ⚑ Was hier belegt wird
//!
//! Ein Torknoten **ohne eigenen Shard** rechnet einen Prompt über die Shards
//! der Pod-Mitglieder, die die Kette zuteilt. Jede Shardanfrage geht vom
//! Koordinator über die Relaisleitung an den Torknoten, über das Netz an den
//! Knoten des Mitglieds und von dort an dessen Shard-Prozess. Das Ergebnis
//! muss Token für Token dem gleichen, was dieselben Grenzen im Prozess
//! rechnen.
//!
//! Gegenproben: eine Trainingsanfrage wird nicht weitergereicht, und ein
//! Mitglied der Reserve hat keinen Shard.
//!
//! # ⚑ Und dann das Bündel (2026-10-26)
//!
//! Der Koordinator lässt das Bündel von den Positionen unterschreiben und
//! gibt es über die Leitung ab; der Torknoten reicht es an die erste
//! Position, die es einreicht. Mit einem Burn in derselben Epoche wachsen
//! am Epochenwechsel die Auszahlungskonten der Positionen um mehr als die
//! Probeprägung, das der Reserve genau um sie. Gegenprobe: dasselbe Bündel
//! mit erhöhtem Anspruch wird abgewiesen.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle, Shardkonfig};
use myl_node::Knoten;
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

/// Ein Probeordner im Zwischenordner, der beim Fallen verschwindet.
struct Probeordner(PathBuf);

impl Drop for Probeordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Was der Koordinator über die Knoten zurückgibt: Podbild, Token, Zuschnitte.
type Ueberknoten = (myl_types::relais::Podbild, Vec<u32>, Vec<(u32, u32)>, myl_types::PoIBundle);

/// Sechs Miner: vier Positionen und zwei Reserve, ein Pod.
const MINER: usize = 6;

/// Das Auszahlungskonto von Miner `i`.
fn konto(i: usize) -> myl_types::ids::Address {
    myl_types::ids::Address::new([0xa0 + i as u8; 32])
}

fn konfig(ordner: &Path, name: &str, bootstrap: Vec<String>, miner: Option<usize>, relais: Option<std::net::SocketAddr>) -> KnotenKonfig {
    let d = ordner.join(name);
    std::fs::create_dir_all(&d).expect("Ordner");
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        kettendatei: Some(d.join("kette.dat")),
        konsensschluesseldatei: miner.map(|_| d.join("konsens.key")),
        modellzusage: miner.map(|_| Modellzusage { abdruck: [0; 32], ebenen_hoechstens: 48, ebenenzeit_us: 30_000, gehalten: None }),
        shard: miner.map(|_| Shardkonfig { programm: shardprogramm(), artefakte: sparrow(), training: false }),
        konto: miner.map(konto),
        aufnahme_sekunden: 1,
        beobachtung: None,
        tuer: None,
        erzeugt_bloecke: miner.is_none(),
        podrelais: relais,
        podrelaisausweis: relais.map(|_| d.join("relais.ausweis")),
        ..KnotenKonfig::default()
    }
}

async fn runde(knoten: &mut [Knoten], bloecke: usize) {
    for _ in 0..bloecke {
        knoten[0].erzeuge_block().await;
    }
    for k in knoten.iter_mut() {
        k.laufe_fuer(Duration::from_millis(25)).await;
    }
}

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

/// Treibt die Knoten, ohne Blöcke zu bauen, bis die Arbeit im eigenen Faden
/// fertig ist: Die Epoche bleibt, und mit ihr der Pod.
async fn treiben_bis<T: Send + 'static>(knoten: &mut [Knoten], arbeit: tokio::task::JoinHandle<T>, frist: Duration) -> T {
    let ende = tokio::time::Instant::now() + frist;
    while !arbeit.is_finished() {
        assert!(tokio::time::Instant::now() < ende, "die Arbeit kam nicht in {frist:?} zurück");
        runde(knoten, 0).await;
    }
    arbeit.await.expect("Faden")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_torknoten_ohne_shard_rechnet_ueber_die_knoten_des_pods() {
    if !myl_pod::artefakte::vorhanden(&sparrow()) {
        return;
    }
    assert!(shardprogramm().exists(), "myl-shard fehlt: cd COMPUTE_PIPELINE/myl-pod && cargo test --no-run");
    let o = Probeordner(std::env::temp_dir().join(format!("myl-podueberknoten-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&o.0);
    std::fs::create_dir_all(&o.0).expect("Ordner");

    // Ein freier Port für die Relaisleitung des Torknotens.
    let relais = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("Port");
    let mut knoten: Vec<Knoten> = Vec::new();
    let mut tor = Knoten::starten(konfig(&o.0, "tor", vec![], None, Some(relais)), false).await.expect("Tor");
    let adresse = tor.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let bootstrap = vec![format!("{}/p2p/{}", adresse, tor.peer_id())];
    knoten.push(tor);
    for i in 0..MINER {
        knoten.push(Knoten::starten(konfig(&o.0, &format!("m{i}"), bootstrap.clone(), Some(i), None), false).await.expect("Miner"));
    }

    // Anmeldung und Zusage, dann bis Epoche 2: der erste Pod.
    for _ in 0..30 {
        runde(&mut knoten, 2).await;
    }
    while knoten[0].kette().zustand().epoch.0 < 2 {
        runde(&mut knoten, 200).await;
    }
    assert!(angleichen(&mut knoten, Duration::from_secs(120)).await, "die Knoten holten nicht auf");
    let zustand = knoten[0].kette().zustand().clone();
    let z = myl_node::kette::Kette::zuteilung_der_laufenden_epoche(&zustand);
    assert_eq!(z.pods.len(), 1, "sechs Miner, ein Pod");
    let grenzen = myl_node::kette::Kette::grenzen_des_pods(&zustand, &z.pods[0]).expect("machbar");
    eprintln!("[podueberknoten] Epoche {}, Grenzen {grenzen:?}", zustand.epoch.0);

    // Jede Position führt ihren Shard.
    let positionen: Vec<_> = z.pods[0].shards.iter().map(|s| s.miner.miner_id).collect();
    let bis = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        runde(&mut knoten, 0).await;
        let laufen = positionen.iter().all(|m| {
            knoten.iter().find(|k| k.miner_kennung() == Some(*m)).is_some_and(|k| k.shard_pid().is_some() && k.shardauftrag().is_some())
        });
        if laufen {
            break;
        }
        assert!(tokio::time::Instant::now() < bis, "nicht jede Position führt ihren Shard");
    }

    // ---- Der Koordinator, im eigenen Faden, über die Relaisleitung --------
    let ausweis = myl_types::relais::ausweis_lesen(&o.0.join("tor").join("relais.ausweis")).expect("Ausweis");
    let prompt: Vec<u32> = vec![785, 3974, 13, 576, 1879, 315];
    const NEU: u64 = 8;
    let p2 = prompt.clone();
    let arbeit = tokio::task::spawn_blocking(move || -> Result<Ueberknoten, String> {
        let bild = match myl_types::relais::fragen(relais, &ausweis, &myl_types::relais::Relaisfrage::Podbild, Duration::from_secs(10))? {
            myl_types::relais::Relaisantwort::Podbild(b) => b,
            andere => return Err(format!("statt Podbild: {andere:?}")),
        };
        let pod = bild.pods.first().ok_or("kein Pod im Podbild")?.clone();
        let mitglieder = pod.mitglieder.iter().map(|m| m.netzadresse).collect();
        let weg = Arc::new(myl_pod::shardweg::UeberDenKnoten::neu(relais, ausweis, mitglieder));
        let mut k = myl_pod::coordinator::Coordinator::ueber_weg(
            pod.kennung,
            myl_types::ids::EpochId(bild.epoche),
            weg,
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        )?;
        let zuschnitte: Vec<(u32, u32)> = pod.mitglieder.iter().map(|m| (m.von, m.bis)).collect();
        let token = k.run_prompt(1, &p2, NEU);
        // Die Positionen unterschreiben über dieselbe Leitung.
        let buendel = k.build_signed_poi_bundle()?;
        Ok((bild, token, zuschnitte, buendel))
    });
    let (bild, ueber_knoten, zuschnitte, buendel) = treiben_bis(&mut knoten, arbeit, Duration::from_secs(600)).await.expect("über die Knoten");
    assert_eq!(zuschnitte, grenzen, "das Podbild nennt andere Grenzen als die Kette");
    assert_eq!(bild.pods[0].mitglieder.len(), positionen.len());
    eprintln!("[podueberknoten] über die Knoten: {ueber_knoten:?}");
    assert!(!ueber_knoten.is_empty(), "über die Knoten kam kein Token");

    // ---- Die Referenz: dieselben Grenzen im Prozess -----------------------
    let modell = Arc::new(integer_llm_runtime::loader::load_model_fuer_netz(&sparrow()).expect("Modell"));
    let n = grenzen.len();
    let shards = grenzen
        .iter()
        .enumerate()
        .map(|(i, &(von, bis))| {
            let sk = myl_types::bls::BlsSecretKey::key_gen(&[i as u8 + 1; 32]).expect("BLS");
            Arc::new(myl_pod::shard::ShardNode::new(i, von as usize, bis as usize, i == 0, i == n - 1, Arc::clone(&modell), sk, NEU))
        })
        .collect();
    let mut referenz = myl_pod::coordinator::Coordinator::new(bild.pods[0].kennung, myl_types::ids::EpochId(bild.epoche), shards, myl_pod::coordinator::DEFAULT_WINDOW_MS);
    let im_prozess = referenz.run_prompt(1, &prompt, NEU);
    assert_eq!(ueber_knoten, im_prozess, "über die Knoten anders als im Prozess");

    // ---- ⛔️ Gegenproben ------------------------------------------------------
    let erster = bild.pods[0].mitglieder[0].netzadresse;
    let reserve = z.pods[0].reserve.first().map(|r| r.netzadresse).expect("Reserve");
    let arbeit = tokio::task::spawn_blocking(move || {
        // Marke 8: TrainVorwaerts, über fremde Knoten nicht freigegeben.
        let training = myl_types::relais::fragen(relais, &ausweis, &myl_types::relais::Relaisfrage::Shard { an: erster, anfrage: vec![8, 0, 0, 0] }, Duration::from_secs(20));
        // Eine Reserve hat keinen Shard.
        let zuschnitt = vec![1u8];
        let ohne = myl_types::relais::fragen(relais, &ausweis, &myl_types::relais::Relaisfrage::Shard { an: reserve, anfrage: zuschnitt }, Duration::from_secs(20));
        (training, ohne)
    });
    let (training, ohne) = treiben_bis(&mut knoten, arbeit, Duration::from_secs(60)).await;
    assert!(matches!(&training, Ok(myl_types::relais::Relaisantwort::Fehler(f)) if f.contains("nicht weitergereicht")), "{training:?}");
    assert!(matches!(&ohne, Ok(myl_types::relais::Relaisantwort::Fehler(f)) if f.contains("kein Shard")), "{ohne:?}");

    // ---- Das Bündel in die Kette ------------------------------------------
    let kennung = bild.pods[0].kennung;
    let mut gefaelscht = buendel.clone();
    gefaelscht.vtfe_claimed += 1;
    let echt = borsh::to_vec(&buendel).expect("Borsh");
    let falsch = borsh::to_vec(&gefaelscht).expect("Borsh");
    let arbeit = tokio::task::spawn_blocking(move || {
        let f = myl_types::relais::fragen(relais, &ausweis, &myl_types::relais::Relaisfrage::BuendelEinreichen { buendel: falsch }, Duration::from_secs(30));
        let e = myl_types::relais::fragen(relais, &ausweis, &myl_types::relais::Relaisfrage::BuendelEinreichen { buendel: echt }, Duration::from_secs(30));
        (f, e)
    });
    let (f, e) = treiben_bis(&mut knoten, arbeit, Duration::from_secs(90)).await;
    // ⛔️ Gegenprobe: ein erhöhter Anspruch besteht das Aggregat nicht.
    assert!(matches!(&f, Ok(myl_types::relais::Relaisantwort::Fehler(t)) if t.contains("Aggregat")), "{f:?}");
    assert!(matches!(&e, Ok(myl_types::relais::Relaisantwort::Erledigt(_))), "{e:?}");
    eprintln!("[podueberknoten] Bündel: {e:?}");

    // Ein Burn in derselben Epoche, damit die Ausschüttung etwas hat.
    let nonce = knoten[0].kette().zustand().account(&myl_node::kette::probekonto(0)).nonce;
    let burn = myl_consensus::block::Transaktion::signiere(
        &knoten[0].kette().netzwert(),
        &myl_node::kette::probeschluessel(0),
        nonce,
        // Ein Probekonto hat 10 MYL; die Hälfte geht in den Burn.
        myl_consensus::block::Anweisung::Burn { betrag: 5 * 1_000_000 },
    )
    .expect("signieren");
    knoten[0].kette_mut().aufnehmen(burn);
    let bis = tokio::time::Instant::now() + Duration::from_secs(60);
    // Bündel und Burn können in verschiedenen Blöcken landen.
    while !knoten[0].kette().zustand().buendel.contains_key(&kennung) || knoten[0].kette().zustand().burn_epoche == 0 {
        assert!(
            tokio::time::Instant::now() < bis,
            "Bündel {} und Burn {} nicht beide in der Kette",
            knoten[0].kette().zustand().buendel.contains_key(&kennung),
            knoten[0].kette().zustand().burn_epoche
        );
        runde(&mut knoten, 1).await;
    }
    let epoche = knoten[0].kette().zustand().epoch.0;
    let vorher: Vec<u64> = (0..MINER).map(|i| knoten[0].kette().zustand().account(&konto(i)).balance).collect();
    while knoten[0].kette().zustand().epoch.0 == epoche {
        runde(&mut knoten, 200).await;
    }
    let nachher: Vec<u64> = (0..MINER).map(|i| knoten[0].kette().zustand().account(&konto(i)).balance).collect();
    let probe = myl_node::netz::PROBEKETTE.probepraegung;
    let rolle = |i: usize| {
        let id = knoten[1 + i].miner_kennung().expect("Kennung");
        positionen.contains(&id)
    };
    eprintln!("[podueberknoten] Auszahlung: vorher {vorher:?}, nachher {nachher:?}");
    for i in 0..MINER {
        let zuwachs = nachher[i] - vorher[i];
        if rolle(i) {
            assert!(zuwachs > probe, "Position m{i} bekam nur {zuwachs}, nicht mehr als die Probeprägung");
        } else {
            assert_eq!(zuwachs, probe, "Reserve m{i} bekam {zuwachs} statt der Probeprägung");
        }
    }
}
