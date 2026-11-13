//! **Ein Pod aus Klienten** (2026-11-10).
//!
//! # ⚑ Was hier belegt wird
//!
//! Dreizehn Miner entstehen so, wie ein Mensch sie startet: je ein
//! abgeschirmter Klient mit eigenem Wallet, `myl wallet wiederherstellen`,
//! `myl wallet waehle` und `myl mining start`. Kein Knoten der Miner wird von
//! der Probe selbst gebaut oder eingestellt; sie kennt von ihnen nur, was der
//! Klient auf die Platte legt. Allein der Einstieg läuft im Prozess der
//! Probe, damit sie den Zustand der Kette lesen kann.
//!
//! Die Kette bildet daraus zwei Pods, und die beiden sind Zwillinge. Je Pod
//! rechnet ein Koordinator denselben Prompt über die Relaisleitung eines
//! Klienten; beide Ergebnisse müssen Token für Token dem gleichen, was
//! dieselben Grenzen im Prozess rechnen. Die Bündel gehen über dieselbe
//! Leitung in die Kette, ein Klient kauft mit `myl wallet credits` Credits
//! aus seinem Wallet, und am Epochenwechsel wächst das Wallet jeder Position
//! um mehr als die Probeprägung. `myl mining` zeigt dem Klienten denselben
//! Stand, den die Kette führt.
//!
//! Gegenproben: Wer in der Reserve oder ohne Pod stand, bekommt genau die
//! Probeprägung; ein Bündel mit erhöhtem Anspruch wird abgewiesen; nach
//! `myl mining stop` lebt kein Knoten und kein Shard mehr.
//!
//! # ⚑ Warum die Probe nur auf Zuruf läuft
//!
//! Die Probekette läuft nach der Uhr: ein Slot dauert eine halbe Sekunde,
//! eine Epoche eine Viertelstunde, und der Klient reicht keinen anderen Takt
//! durch. Der erste Pod steht in der Epoche nach der Anmeldung, die Auszahlung
//! kommt an ihrem Ende. Das ist gut eine halbe Stunde, und kürzer geht es nur
//! mit einem Knoten, den nicht der Klient gestartet hat.
//!
//! Davor gehört der kurze Lauf aus derselben Datei (unter einer Minute):
//! Dieselben dreizehn Klienten starten, melden sich an und hören wieder
//! auf. Scheitert er, lohnt der lange nicht.
//!
//! ```text
//! cargo test --test pod_aus_klienten -- --ignored --nocapture klienten_melden_sich_an
//! cargo test --test pod_aus_klienten -- --ignored --nocapture dreizehn_klienten
//! ```
//!
//! # ⚑ Was vorher gebaut sein muss
//!
//! `myl` und `myl-shard` im Bauordner dieses Profils, `myl-node` daneben
//! oder im Auslieferungsbau (der Klient sucht in dieser Folge):
//!
//! ```text
//! cd CLIENT/myl-client && cargo build
//! cd COMPUTE_PIPELINE/myl-pod && cargo test --no-run
//! cd NODE/myl-node && cargo build --release
//! ```
//!
//! ⚠️ Ein `myl-node` aus einem älteren Stand spricht womöglich ein anderes
//! Protokoll als der Einstieg dieser Probe; vor dem Lauf frisch bauen.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use myl_node::kette::Kette;
use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;
use myl_types::ids::{Address, MinerId};
use myl_types::relais::{Relaisantwort, Relaisfrage};

/// Dreizehn Miner: zwei Pods aus je vier Positionen und zwei Reserven, und
/// einer mehr, damit ein gesperrter Miner nicht gleich den zweiten Pod kostet.
const KLIENTEN: usize = 13;
/// So viele Token rechnet jeder Pod.
const NEU: u64 = 8;
/// So viele MYL gehen in den Burn; ein Wallet hat bis dahin zwei
/// Probeprägungen.
const BURN_MYL: &str = "5";

type Ausweis = [u8; myl_types::zeitprobe::AUSWEIS_LEN];

fn wurzel() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn sparrow() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

fn bauordner() -> PathBuf {
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("SYSTEM/full-build").join(profil)
}

/// Ein Klient: sein abgeschirmter Ordner und sein Wallet.
#[derive(Clone)]
struct Klient {
    name: String,
    wallet: String,
    ordner: PathBuf,
    adresse: Address,
    woerter: String,
}

/// Womit die Probe `myl` aufruft.
#[derive(Clone)]
struct Aufruf {
    myl: PathBuf,
    einstieg: String,
}

impl Aufruf {
    /// Ruft `myl` in der Umgebung des Klienten; liefert Erfolg und Ausgabe.
    fn myl(&self, k: &Klient, args: &[&str], eingabe: Option<&str>) -> (bool, String) {
        let mut c = Command::new(&self.myl);
        c.args(args)
            .env("HOME", &k.ordner)
            .env("MYL_EINSTELLUNGEN", k.ordner.join("client.json"))
            .env("MYL_WALLETS", k.ordner.join("wallets"))
            .env("MYL_SPEICHERORTE", k.ordner.join("speicherorte.toml"))
            .env("MYL_PROBENETZ", &self.einstieg)
            .env_remove("MYL_NETZ")
            .stdin(if eingabe.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut kind = match c.spawn() {
            Ok(k) => k,
            Err(e) => return (false, format!("{}: {e}", self.myl.display())),
        };
        if let (Some(t), Some(mut ein)) = (eingabe, kind.stdin.take()) {
            use std::io::Write;
            let _ = ein.write_all(t.as_bytes());
        }
        match kind.wait_with_output() {
            Ok(a) => (a.status.success(), format!("{}{}", String::from_utf8_lossy(&a.stdout), String::from_utf8_lossy(&a.stderr))),
            Err(e) => (false, e.to_string()),
        }
    }

    /// Wie [`Self::myl`], aber ein Fehlschlag ist ein Fehler mit der Ausgabe.
    fn muss(&self, k: &Klient, args: &[&str], eingabe: Option<&str>) -> Result<String, String> {
        match self.myl(k, args, eingabe) {
            (true, aus) => Ok(aus),
            (false, aus) => Err(format!("{}: myl {} scheiterte:\n{aus}", k.name, args.join(" "))),
        }
    }
}

/// Was der Klient über seinen laufenden Knoten ablegt.
fn zustand_des_klienten(k: &Klient) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(k.ordner.join("mining").join("zustand.json")).ok()?).ok()
}

/// Die Relaisleitung des Klientenknotens: Adresse und Ausweis.
fn leitung(k: &Klient) -> Result<(SocketAddr, Ausweis), String> {
    let z = zustand_des_klienten(k).ok_or_else(|| format!("{}: kein Zustand des Minings", k.name))?;
    let adresse = z["relais"].as_str().and_then(|a| a.parse().ok()).ok_or_else(|| format!("{}: keine Relaisleitung im Zustand", k.name))?;
    let ausweis = myl_types::relais::ausweis_lesen(&k.ordner.join("mining").join("relais.ausweis")).map_err(|e| format!("{}: {e}", k.name))?;
    Ok((adresse, ausweis))
}

fn lebt(pid: u32) -> bool {
    Command::new("kill").arg("-0").arg(pid.to_string()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Der Knoten des Klienten und was er gestartet hat (sein Shard).
fn prozesse(k: &Klient) -> Vec<u32> {
    let Some(pid) = zustand_des_klienten(k).and_then(|z| z["pid"].as_u64()) else {
        return Vec::new();
    };
    let mut alle = vec![pid as u32];
    if let Ok(a) = Command::new("pgrep").arg("-P").arg(pid.to_string()).output() {
        alle.extend(String::from_utf8_lossy(&a.stdout).lines().filter_map(|z| z.trim().parse::<u32>().ok()));
    }
    alle
}

/// Was der Knoten des Klienten gerade für das Netz rechnet: die Netzlast, die
/// er neben sein Protokoll legt, solange sein Shard läuft.
fn netzlast(k: &Klient) -> Option<myl_store::teilablage::Netzlast> {
    let datei = k.ordner.join("ablage").join("knoten").join("logs").join(myl_store::teilablage::NETZLAST_DATEI);
    myl_store::teilablage::Netzlast::lesen(&datei)
}

/// Wie viele Shards der Knoten des Klienten hält: seine Kindprozesse.
fn shards(k: &Klient) -> usize {
    prozesse(k).len().saturating_sub(1)
}

/// **Der Lauf räumt hinter sich auf**, auch wenn eine Prüfung scheitert: Die
/// Knoten der Klienten laufen in eigenen Prozessgruppen und überlebten die
/// Probe sonst. Der Ordner bleibt nach einem Fehlschlag liegen, mit ihm die
/// Protokolle.
struct Lauf {
    aufruf: Aufruf,
    klienten: Vec<Klient>,
    ordner: PathBuf,
    behalten: bool,
}

impl Drop for Lauf {
    fn drop(&mut self) {
        let mut pids = Vec::new();
        for k in &self.klienten {
            pids.extend(prozesse(k));
            let _ = self.aufruf.myl(k, &["mining", "stop"], None);
        }
        for p in pids {
            if lebt(p) {
                let _ = Command::new("kill").arg("-9").arg(p.to_string()).status();
            }
        }
        if self.behalten {
            eprintln!("[podausklienten] der Laufordner bleibt liegen: {}", self.ordner.display());
        } else {
            let _ = std::fs::remove_dir_all(&self.ordner);
        }
    }
}

/// Treibt den Einstieg, bis die Arbeit im eigenen Faden fertig ist.
async fn treiben_bis<T: Send + 'static>(einstieg: &mut Knoten, arbeit: tokio::task::JoinHandle<T>, frist: Duration, was: &str) -> T {
    let ende = Instant::now() + frist;
    while !arbeit.is_finished() {
        assert!(Instant::now() < ende, "{was}: kam nicht in {frist:?} zurück");
        einstieg.laufe_fuer(Duration::from_millis(200)).await;
    }
    arbeit.await.expect("Faden")
}

/// Treibt den Einstieg, bis die Bedingung gilt. Die Frist ist die Notbremse;
/// früher schlägt an, wenn die Kette anderthalb Minuten nicht wächst.
async fn warten(einstieg: &mut Knoten, frist: Duration, takt: Duration, was: &str, mut erfuellt: impl FnMut(&Knoten) -> bool) {
    let beginn = Instant::now();
    let mut gemeldet = Instant::now();
    let mut zuletzt = (einstieg.kette().hoehe(), Instant::now());
    loop {
        if erfuellt(einstieg) {
            return;
        }
        let h = einstieg.kette().hoehe();
        if h != zuletzt.0 {
            zuletzt = (h, Instant::now());
        }
        assert!(zuletzt.1.elapsed() < Duration::from_secs(90), "{was}: die Kette steht seit 90 s auf Höhe {h}");
        assert!(beginn.elapsed() < frist, "{was}: nicht in {frist:?} (Höhe {h})");
        if gemeldet.elapsed() >= Duration::from_secs(300) {
            gemeldet = Instant::now();
            eprintln!("[podausklienten] Höhe {h}, Epoche {}, wartet: {was}", einstieg.kette().zustand().epoch.0);
        }
        einstieg.laufe_fuer(takt).await;
    }
}

/// Was ein Pod über die Leitung eines Klienten gerechnet hat.
struct Gerechnet {
    epoche: u64,
    kennung: myl_types::ids::PodId,
    zwilling: Option<u32>,
    zuschnitte: Vec<(u32, u32)>,
    token: Vec<u32>,
    buendel: myl_types::PoIBundle,
}

/// Der Koordinator eines Pods, über die Relaisleitung eines Klientenknotens.
fn pod_rechnen(relais: SocketAddr, ausweis: Ausweis, pod_index: u32, prompt: &[u32]) -> Result<Gerechnet, String> {
    let bild = match myl_types::relais::fragen(relais, &ausweis, &Relaisfrage::Podbild, Duration::from_secs(10)).map_err(|e| e.to_string())? {
        Relaisantwort::Podbild(b) => b,
        andere => return Err(format!("statt Podbild: {andere:?}")),
    };
    let pod = bild.pods.iter().find(|p| p.pod_index == pod_index).ok_or_else(|| format!("Pod {pod_index} fehlt im Podbild"))?.clone();
    let mitglieder = pod.mitglieder.iter().map(|m| m.netzadresse).collect();
    let weg: Arc<dyn myl_pod::shardweg::Shardweg> = Arc::new(myl_pod::shardweg::UeberDenKnoten::neu(relais, ausweis, mitglieder));
    // Ein Shard, der gerade gestartet ist, lädt noch sein Modell; bereit ist
    // der Pod, wenn jeder seinen Zuschnitt nennt.
    let bis = Instant::now() + Duration::from_secs(300);
    let mut k = loop {
        match myl_pod::coordinator::Coordinator::ueber_weg(pod.kennung, myl_types::ids::EpochId(bild.epoche), Arc::clone(&weg), myl_pod::coordinator::DEFAULT_WINDOW_MS) {
            Ok(k) => break k,
            Err(f) if Instant::now() < bis => {
                eprintln!("[podausklienten] Pod {pod_index} noch nicht bereit: {f}");
                std::thread::sleep(Duration::from_secs(5));
            }
            Err(f) => return Err(format!("Pod {pod_index} wurde nicht bereit: {f}")),
        }
    };
    let token = k.run_prompt(1, prompt, NEU);
    let buendel = k.build_signed_poi_bundle()?;
    Ok(Gerechnet {
        epoche: bild.epoche,
        kennung: pod.kennung,
        zwilling: pod.zwilling,
        zuschnitte: pod.mitglieder.iter().map(|m| (m.von, m.bis)).collect(),
        token,
        buendel,
    })
}

/// Dieselben Grenzen im Prozess, ohne Netz.
fn im_prozess_rechnen(grenzen: &[(u32, u32)], kennung: myl_types::ids::PodId, epoche: u64, prompt: &[u32]) -> Vec<u32> {
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
    let mut referenz = myl_pod::coordinator::Coordinator::new(kennung, myl_types::ids::EpochId(epoche), shards, myl_pod::coordinator::DEFAULT_WINDOW_MS);
    referenz.run_prompt(1, prompt, NEU)
}

/// Der Einstieg im Prozess der Probe und die Klienten: gestartet, wie ein
/// Mensch es tut, und jeder mit seinem Wallet als Auszahlungskonto angemeldet.
async fn aufbauen(lauf: &str) -> (Knoten, Lauf) {
    // ⚑ Auf Zuruf gestartet ist ein fehlendes Stück ein Fehler, kein Absprung.
    assert!(myl_pod::artefakte::vorhanden(&sparrow()), "das Artefakt fehlt: {}", sparrow().display());
    let myl = bauordner().join("myl");
    assert!(myl.is_file(), "myl fehlt: cd CLIENT/myl-client && cargo build");
    assert!(bauordner().join("myl-shard").is_file(), "myl-shard fehlt: cd COMPUTE_PIPELINE/myl-pod && cargo test --no-run");
    let knotenprogramm = [bauordner().join("myl-node"), wurzel().join("SYSTEM/full-build/release/myl-node")].into_iter().find(|p| p.is_file());
    let knotenprogramm = knotenprogramm.expect("myl-node fehlt: cd NODE/myl-node && cargo build --release");
    eprintln!("[podausklienten] der Klient startet {}", knotenprogramm.display());

    let ordner = std::env::temp_dir().join(format!("myl-podausklienten-{}-{lauf}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("Ordner");

    // ---- Der Einstieg: der einzige Knoten im Prozess der Probe ------------
    let e = ordner.join("einstieg");
    std::fs::create_dir_all(&e).expect("Ordner");
    let mut einstieg = Knoten::starten(
        KnotenKonfig {
            name: "einstieg".to_string(),
            schluesseldatei: e.join("knoten.key"),
            protokollverzeichnis: e.clone(),
            horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
            rolle: Rolle::Teilnehmer,
            kettendatei: Some(e.join("kette.dat")),
            aufnahme_sekunden: 5,
            beobachtung: None,
            tuer: None,
            erzeugt_bloecke: true,
            // Wie ein Knoten von der Befehlszeile: je Slot nach der Uhr.
            blocktakt_ms: Some(myl_node::netz::PROBEKETTE.slot_ms),
            ..KnotenKonfig::default()
        },
        false,
    )
    .await
    .expect("Einstieg");
    let adresse = einstieg.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let aufruf = Aufruf { myl, einstieg: format!("{}/p2p/{}", adresse, einstieg.peer_id()) };

    // ---- Die Klienten ----------------------------------------------------
    let klienten: Vec<Klient> = (0..KLIENTEN)
        .map(|i| {
            let woerter = myl_client::bip39::woerter(&[0x31 + i as u8; myl_client::bip39::ENTROPIE_BYTES]).join(" ");
            let adresse = myl_client::wallet::aus_woertern(&woerter).expect("Wörter").adresse;
            let name = format!("k{:02}", i + 1);
            Klient { wallet: format!("w{:02}", i + 1), ordner: ordner.join(&name), name, adresse, woerter }
        })
        .collect();
    let lauf = Lauf { aufruf: aufruf.clone(), klienten: klienten.clone(), ordner: ordner.clone(), behalten: true };
    let (a, ks) = (aufruf.clone(), klienten.clone());
    let arbeit = tokio::task::spawn_blocking(move || -> Result<(), String> {
        for k in &ks {
            std::fs::create_dir_all(&k.ordner).map_err(|e| e.to_string())?;
            // Zwölf Wörter, dann das Passwort: leer.
            a.muss(k, &["wallet", "wiederherstellen", &k.wallet], Some(&format!("{}\n\n\n", k.woerter)))?;
            a.muss(k, &["wallet", "waehle", &k.wallet], None)?;
            let ablage = k.ordner.join("ablage");
            a.muss(k, &["mining", "start", "--ort", &ablage.to_string_lossy(), "--volumen", "1", "--kerne", "1"], None)?;
            eprintln!("[podausklienten] {} gestartet", k.name);
        }
        Ok(())
    });
    treiben_bis(&mut einstieg, arbeit, Duration::from_secs(900), "Start der Klienten").await.expect("Start der Klienten");
    for k in &klienten {
        let z = zustand_des_klienten(k).unwrap_or_else(|| panic!("{}: kein Zustand nach dem Start", k.name));
        assert!(z["ohne_shard"].is_null(), "{} startete ohne Shard: {}", k.name, z["ohne_shard"]);
        assert_eq!(z["wallet"].as_str(), Some(k.wallet.as_str()), "{}: anderes Wallet im Zustand", k.name);
    }

    // ---- Anmeldung: jedes Wallet ist Auszahlungskonto eines Miners -------
    warten(&mut einstieg, Duration::from_secs(600), Duration::from_secs(1), "Anmeldung aller Klienten", |k| {
        let z = k.kette().zustand();
        klienten.iter().all(|kl| z.auszahlung.iter().any(|(m, a)| *a == kl.adresse && z.miner.contains_key(m)))
    })
    .await;
    (einstieg, lauf)
}

/// Fragt `myl mining`, bis die Zeile erscheint. Der Klient liest, was sein
/// Knoten zuletzt aufgenommen hat, und das hinkt der Kette Sekunden nach.
async fn klient_zeigt(einstieg: &mut Knoten, aufruf: &Aufruf, k: &Klient, erwartet: &str) {
    let bis = Instant::now() + Duration::from_secs(90);
    loop {
        let (a, k2) = (aufruf.clone(), k.clone());
        let arbeit = tokio::task::spawn_blocking(move || a.muss(&k2, &["mining"], None));
        let bild = treiben_bis(einstieg, arbeit, Duration::from_secs(60), "myl mining").await.expect("myl mining");
        if bild.contains(erwartet) {
            eprintln!("[podausklienten] {} sieht: {erwartet}", k.name);
            return;
        }
        assert!(Instant::now() < bis, "{} zeigt nicht „{erwartet}“:\n{bild}", k.name);
        einstieg.laufe_fuer(Duration::from_secs(3)).await;
    }
}

/// `myl mining stop` je Klient; danach lebt kein Knoten und kein Shard mehr.
async fn beenden(einstieg: &mut Knoten, lauf: &mut Lauf) {
    let (a, ks) = (lauf.aufruf.clone(), lauf.klienten.clone());
    let arbeit = tokio::task::spawn_blocking(move || -> Result<(usize, Vec<u32>), String> {
        let mut pids = Vec::new();
        for k in &ks {
            pids.extend(prozesse(k));
            let aus = a.muss(k, &["mining", "stop"], None)?;
            if !aus.contains("Mining beendet") {
                return Err(format!("{}: {aus}", k.name));
            }
        }
        let bis = Instant::now() + Duration::from_secs(20);
        while pids.iter().any(|p| lebt(*p)) && Instant::now() < bis {
            std::thread::sleep(Duration::from_millis(500));
        }
        Ok((pids.len(), pids.into_iter().filter(|p| lebt(*p)).collect()))
    });
    let (liefen, uebrig) = treiben_bis(einstieg, arbeit, Duration::from_secs(300), "Beenden der Klienten").await.expect("Beenden");
    // ⛔️ Gegenprobe zur Zählung: Vor dem Beenden lief je Klient mindestens
    // sein Knoten; ohne das wäre „nichts lebt mehr“ auch ohne Lauf wahr.
    assert!(liefen >= lauf.klienten.len(), "vor dem Beenden liefen nur {liefen} Prozesse");
    assert!(uebrig.is_empty(), "nach myl mining stop leben noch Prozesse: {uebrig:?}");
    lauf.behalten = false;
}

/// Der kurze Lauf vor dem langen: Jeder Klient startet, meldet sich an und
/// hört wieder auf. Ohne Pod, also ohne Wartezeit auf eine Epoche.
///
/// ⚑ Der Name steht im Alphabet vor dem langen Lauf: Mit einem Faden fahren
/// die Proben nach Namen, und wer die ganze Datei fährt, sieht zuerst diesen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "braucht die gebauten Programme; unter einer Minute"]
async fn alle_klienten_melden_sich_an_und_hoeren_wieder_auf() {
    let (mut einstieg, mut lauf) = aufbauen("kurz").await;
    let zustand = einstieg.kette().zustand().clone();
    assert_eq!(zustand.miner.len(), KLIENTEN, "angemeldet sind andere als die Klienten");
    // Vor der ersten Prägung ist jedes Wallet leer, und der Klient zeigt es so.
    let k = lauf.klienten[0].clone();
    assert_eq!(zustand.account(&k.adresse).balance, 0, "{} hat schon etwas im Wallet", k.name);
    let aufruf = lauf.aufruf.clone();
    klient_zeigt(&mut einstieg, &aufruf, &k, &format!("Wallet {}: 0 MYL", k.wallet)).await;
    // Ohne Pod rechnet niemand: Der Knoten ist der einzige Prozess, und er
    // legt keine Netzlast ab. Daran misst der lange Lauf seine Gegenprobe.
    for k in &lauf.klienten {
        assert_eq!(shards(k), 0, "{} hält einen Shard, bevor es einen Pod gibt", k.name);
        assert!(netzlast(k).is_none(), "{} führt eine Netzlast, bevor es einen Pod gibt", k.name);
    }
    beenden(&mut einstieg, &mut lauf).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "gut eine halbe Stunde: die Probekette läuft nach der Uhr, eine Epoche dauert eine Viertelstunde"]
async fn dreizehn_klienten_bilden_zwillingspods_und_werden_bezahlt() {
    let (mut einstieg, mut lauf) = aufbauen("lang").await;
    let (aufruf, klienten) = (lauf.aufruf.clone(), lauf.klienten.clone());
    let slot_ms = myl_node::netz::PROBEKETTE.slot_ms;
    let angemeldet = einstieg.kette().zustand().epoch.0;
    eprintln!("[podausklienten] alle {KLIENTEN} angemeldet in Epoche {angemeldet} bei Höhe {}", einstieg.kette().hoehe());
    let wer = |zustand: &myl_ledger::state::LedgerState, m: &MinerId| -> usize {
        let konto = zustand.auszahlung.get(m).expect("Auszahlungskonto");
        klienten.iter().position(|k| k.adresse == *konto).expect("jeder Miner ist ein Klient")
    };
    let staende = |k: &Knoten| -> Vec<u64> { klienten.iter().map(|kl| k.kette().zustand().account(&kl.adresse).balance).collect() };

    let epochenfrist = Duration::from_millis(myl_consensus::block::BLOECKE_JE_EPOCHE * slot_ms);
    let mut gesehen = angemeldet;
    // Die Probe zählt die Epochen nicht vor: Sie wartet auf die erste nach
    // der Anmeldung, in der die Kette Pods führt.
    warten(&mut einstieg, epochenfrist * 2 + Duration::from_secs(600), Duration::from_secs(1), "die Epoche der ersten Pods", |k| {
        let jetzt = k.kette().zustand().epoch.0;
        if jetzt != gesehen {
            gesehen = jetzt;
            eprintln!("[podausklienten] Epoche {jetzt} bei Höhe {}, Wallets {:?}", k.kette().hoehe(), staende(k));
        }
        jetzt > angemeldet && !Kette::zuteilung_der_laufenden_epoche(k.kette().zustand()).pods.is_empty()
    })
    .await;

    // ---- Zwei Pods, und sie sind Zwillinge -------------------------------
    let zustand = einstieg.kette().zustand().clone();
    let epoche = zustand.epoch.0;
    let z = Kette::zuteilung_der_laufenden_epoche(&zustand);
    assert_eq!(z.pods.len(), 2, "dreizehn Miner, zwei Pods");
    let mut indizes: Vec<u32> = z.pods.iter().map(|p| p.pod_index).collect();
    indizes.sort_unstable();
    assert_eq!(Kette::zwillinge_der_laufenden_epoche(&zustand), vec![indizes.clone()], "die beiden Pods sind kein Zwillingspaar");
    let grenzen: Vec<Vec<(u32, u32)>> = z.pods.iter().map(|p| Kette::grenzen_des_pods(&zustand, p).expect("machbar")).collect();
    let positionen: Vec<Vec<usize>> = z.pods.iter().map(|p| p.shards.iter().map(|s| wer(&zustand, &s.miner.miner_id)).collect()).collect();
    let reserven: Vec<Vec<usize>> = z.pods.iter().map(|p| p.reserve.iter().map(|r| wer(&zustand, &r.miner_id)).collect()).collect();
    let ohne: Vec<usize> = z.ohne_pod.iter().map(|m| wer(&zustand, &m.miner_id)).collect();
    let namen = |liste: &[usize]| -> Vec<&str> { liste.iter().map(|i| klienten[*i].name.as_str()).collect() };
    for (i, p) in z.pods.iter().enumerate() {
        eprintln!("[podausklienten] Epoche {epoche}, Pod {}: Positionen {:?}, Grenzen {:?}, Reserve {:?}", p.pod_index, namen(&positionen[i]), grenzen[i], namen(&reserven[i]));
    }
    eprintln!("[podausklienten] ohne Pod: {:?}", namen(&ohne));
    let rechnend: Vec<usize> = positionen.iter().flatten().copied().collect();
    assert_eq!(rechnend.len(), 8, "zwei Pods mit je vier Positionen");

    // Jede Position rechnet ihren Bereich, aus der Kette und ohne Zutun: Ihr
    // Knoten führt die Netzlast dieser Epoche und hält genau einen Shard.
    // ⚑ Gemessen an dem, was läuft, nicht am Protokolleintrag über einen
    // Start: Wer aus der Epoche davor denselben Bereich in derselben Rolle
    // mitbringt, behält seinen Prozess, und der Knoten vermerkt dann keinen.
    // ⛔️ Gegenprobe im selben Zug: Reserve und Miner ohne Pod rechnen nichts.
    let mut soll: Vec<Option<(u32, u32, u32, u32)>> = vec![None; klienten.len()];
    for (i, p) in z.pods.iter().enumerate() {
        assert_eq!(positionen[i].len(), grenzen[i].len(), "Pod {}: Positionen und Grenzen", p.pod_index);
        for (j, k) in positionen[i].iter().enumerate() {
            soll[*k] = Some((p.pod_index, j as u32, grenzen[i][j].0, grenzen[i][j].1));
        }
    }
    assert_eq!(soll.len() - soll.iter().flatten().count(), reserven.iter().flatten().count() + ohne.len(), "wer nicht rechnet, ist Reserve oder ohne Pod");
    let mut gemeldet = String::new();
    warten(&mut einstieg, Duration::from_secs(300), Duration::from_secs(5), "jede Position rechnet ihren Bereich, und sonst niemand", |_| {
        let offen: Vec<&str> = klienten
            .iter()
            .zip(&soll)
            .filter(|(k, s)| {
                let ist = netzlast(k);
                let passt = match s {
                    Some(a) => ist.as_ref().is_some_and(|n| n.epoche == epoche && (n.pod, n.position, n.von, n.bis) == *a),
                    None => ist.is_none(),
                };
                !passt || shards(k) != usize::from(s.is_some())
            })
            .map(|(k, _)| k.name.as_str())
            .collect();
        let zeile = offen.join(", ");
        if zeile != gemeldet {
            eprintln!("[podausklienten] Shard und Netzlast passen noch nicht bei: [{zeile}]");
            gemeldet = zeile;
        }
        offen.is_empty()
    })
    .await;

    // ---- Je Pod ein Koordinator, über die Leitung seines ersten Klienten --
    let prompt: Vec<u32> = vec![785, 3974, 13, 576, 1879, 315];
    let mut leitungen = Vec::new();
    for p in &positionen {
        leitungen.push(leitung(&klienten[p[0]]).expect("Leitung"));
    }
    let mut gerechnet = Vec::new();
    for (i, p) in z.pods.iter().enumerate() {
        let ((relais, ausweis), index, p2) = (leitungen[i], p.pod_index, prompt.clone());
        let arbeit = tokio::task::spawn_blocking(move || pod_rechnen(relais, ausweis, index, &p2));
        let g = treiben_bis(&mut einstieg, arbeit, Duration::from_secs(600), "Rechnen über die Klienten").await.expect("über die Klienten");
        eprintln!("[podausklienten] Pod {index} über {}: {:?}", klienten[positionen[i][0]].name, g.token);
        assert_eq!(g.epoche, epoche, "der Pod rechnete in einer anderen Epoche");
        assert_eq!(g.zuschnitte, grenzen[i], "das Podbild nennt andere Grenzen als die Kette");
        assert!(!g.token.is_empty(), "über die Klienten kam kein Token");
        gerechnet.push(g);
    }
    assert_eq!(gerechnet[0].zwilling, Some(z.pods[1].pod_index), "das Podbild nennt den Zwilling nicht");
    assert_eq!(gerechnet[1].zwilling, Some(z.pods[0].pod_index), "das Podbild nennt den Zwilling nicht");
    let (g0, k0, p2) = (grenzen[0].clone(), gerechnet[0].kennung, prompt.clone());
    let arbeit = tokio::task::spawn_blocking(move || im_prozess_rechnen(&g0, k0, epoche, &p2));
    let im_prozess = treiben_bis(&mut einstieg, arbeit, Duration::from_secs(300), "Rechnen im Prozess").await;
    for g in &gerechnet {
        assert_eq!(g.token, im_prozess, "über die Klienten anders als im Prozess");
    }

    // ---- Die Bündel in die Kette -----------------------------------------
    let mut gefaelscht = gerechnet[0].buendel.clone();
    gefaelscht.vtfe_claimed += 1;
    let falsch = borsh::to_vec(&gefaelscht).expect("Borsh");
    let echt: Vec<Vec<u8>> = gerechnet.iter().map(|g| borsh::to_vec(&g.buendel).expect("Borsh")).collect();
    let l2 = leitungen.clone();
    let arbeit = tokio::task::spawn_blocking(move || {
        let frist = Duration::from_secs(30);
        let f = myl_types::relais::fragen(l2[0].0, &l2[0].1, &Relaisfrage::BuendelEinreichen { buendel: falsch }, frist);
        let e: Vec<_> = echt.into_iter().zip(&l2).map(|(b, l)| myl_types::relais::fragen(l.0, &l.1, &Relaisfrage::BuendelEinreichen { buendel: b }, frist)).collect();
        (f, e)
    });
    let (f, e) = treiben_bis(&mut einstieg, arbeit, Duration::from_secs(120), "Einreichen der Bündel").await;
    // ⛔️ Gegenprobe: ein erhöhter Anspruch besteht das Aggregat nicht.
    assert!(matches!(&f, Ok(Relaisantwort::Fehler(t)) if t.contains("Aggregat")), "{f:?}");
    for antwort in &e {
        assert!(matches!(antwort, Ok(Relaisantwort::Erledigt(_))), "{antwort:?}");
    }

    // ---- Ein Klient kauft Credits aus seinem Wallet ----------------------
    let zahler = ohne.first().or(reserven[0].first()).copied().expect("ein Klient, der nicht rechnet");
    let stand = einstieg.kette().zustand().account(&klienten[zahler].adresse).balance;
    assert!(stand >= 6_000_000, "{} hat nur {stand} für den Burn", klienten[zahler].name);
    let (a, k) = (aufruf.clone(), klienten[zahler].clone());
    let arbeit = tokio::task::spawn_blocking(move || a.muss(&k, &["wallet", "credits", &k.wallet, BURN_MYL], None));
    let gekauft = treiben_bis(&mut einstieg, arbeit, Duration::from_secs(120), "Kauf der Credits").await.expect("Credits");
    assert!(gekauft.contains("Credits gekauft"), "{gekauft}");
    let kennungen: Vec<_> = gerechnet.iter().map(|g| g.kennung).collect();
    warten(&mut einstieg, Duration::from_secs(120), Duration::from_millis(500), "Bündel und Burn in der Kette", |k| {
        let z = k.kette().zustand();
        kennungen.iter().all(|p| z.buendel.contains_key(p)) && z.burn_epoche > 0
    })
    .await;
    assert_eq!(einstieg.kette().zustand().epoch.0, epoche, "Bündel und Burn kamen erst nach dem Epochenwechsel an");
    eprintln!("[podausklienten] beide Bündel und der Burn von {} stehen bei Höhe {}", klienten[zahler].name, einstieg.kette().hoehe());

    // ---- Der Epochenwechsel zahlt aus ------------------------------------
    let vorher = staende(&einstieg);
    warten(&mut einstieg, epochenfrist + Duration::from_secs(300), Duration::from_secs(1), "der Epochenwechsel mit der Auszahlung", |k| {
        k.kette().zustand().epoch.0 > epoche
    })
    .await;
    let nachher = staende(&einstieg);
    let probe = myl_node::netz::PROBEKETTE.probepraegung;
    let zuwachs: Vec<u64> = nachher.iter().zip(&vorher).map(|(n, v)| n.saturating_sub(*v)).collect();
    eprintln!("[podausklienten] Zuwachs je Wallet: {zuwachs:?} (Probeprägung {probe})");
    let zustand = einstieg.kette().zustand().clone();
    let gesperrt: Vec<&str> = zustand.fristsperren.keys().map(|m| klienten[wer(&zustand, m)].name.as_str()).collect();
    assert!(gesperrt.is_empty(), "gesperrt wegen gerissener Frist: {gesperrt:?}");
    assert_eq!((zustand.zwillinge_einig, zustand.zwillinge_uneinig), (1, 0), "das Zwillingspaar wurde nicht als einig gezählt");
    for (i, k) in klienten.iter().enumerate() {
        if rechnend.contains(&i) {
            assert!(zuwachs[i] > probe, "{} rechnete und bekam {} (Probeprägung {probe})", k.name, zuwachs[i]);
        } else {
            // ⛔️ Gegenprobe: ohne Arbeit nur die Probeprägung.
            assert_eq!(zuwachs[i], probe, "{} rechnete nicht und bekam {}", k.name, zuwachs[i]);
        }
    }
    // Einige Zwillinge werden gleich bezahlt, nichts wartet auf die Streitfrist.
    assert!(zustand.zurueckgehalten.is_empty(), "zurückgehalten trotz einigem Zwilling: {:?}", zustand.zurueckgehalten);

    // ---- Der Klient sieht denselben Stand wie die Kette -------------------
    for i in [rechnend[0], zahler] {
        let erwartet = format!("Wallet {}: {} MYL", klienten[i].wallet, myl_client::mining::myl_text(nachher[i]));
        klient_zeigt(&mut einstieg, &aufruf, &klienten[i], &erwartet).await;
    }

    // ---- Beenden: kein Knoten und kein Shard bleibt -----------------------
    beenden(&mut einstieg, &mut lauf).await;
}
