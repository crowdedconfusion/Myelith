//! Vier Prozesse, ein Pod: der Beweis zu Fund 186.
//!
//! # ⚑ Was dieser Test kann, was `shards_ueber_den_draht.rs` nicht kann
//!
//! Dort laufen vier Türen in **einem** Adressraum. Das beweist das
//! Protokoll, nicht die **Isolation**: Vier Fäden können einander
//! sehen, und ein Fehler, der auf geteiltem Zustand beruht, fiele nicht
//! auf.
//!
//! Hier startet jeder Shard als **eigenes Programm** über seine
//! Kommandozeile. Jeder Prozess hält einen Layerbereich und einen
//! Schlüssel; keiner kennt die Adresse eines anderen. **Was hier grün
//! ist, ist auf vier Maschinen grün.**
//!
//! ⚑ **Bis zum 2026-09-06 gab es diesen Test nicht**, und er ist der
//! einzige, der die Kernaussage des Shardings prüft: dass vier
//! verschiedene Miner einen Pod bilden. `zwei_prozesse.rs` prüft den
//! Weg vom Klienten zum Pod; der Pod war darin **eine** Maschine.

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use myl_pod::coordinator::Coordinator;
use myl_pod::shardweg::UeberDenDraht;
use myl_types::ids::{EpochId, PodId};

const SHARDS: usize = 4;
const PROMPT: [u32; 6] = [9707, 374, 264, 1273, 315, 279];

fn wurzel() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("..").join("..")
}

fn artefakte() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

fn binary() -> PathBuf {
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("SYSTEM/full-build").join(profil).join("myl-shard")
}

/// Startet einen Shard und liest seine Adresse von der Standardausgabe.
fn starten(nummer: usize, bereich: Option<(usize, usize)>) -> (Child, (std::net::SocketAddr, myl_pod::shardweg::Ausweis)) {
    let (ausweisdatei, ausweis) = ausweis_anlegen(nummer);
    let mut befehl = Command::new(binary());
    befehl
        .arg("--ausweis")
        .arg(&ausweisdatei)
        .arg("--artefakte")
        .arg(artefakte())
        .arg("--shard")
        .arg(nummer.to_string())
        .arg("--shards")
        .arg(SHARDS.to_string())
        .arg("--probeschluessel");
    if let Some((von, bis)) = bereich {
        befehl.arg("--von").arg(von.to_string()).arg("--bis").arg(bis.to_string());
    }
    let mut kind = befehl
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("myl-shard startet");
    let aus = kind.stdout.take().expect("Standardausgabe");
    let mut leser = std::io::BufReader::new(aus);
    let mut zeile = String::new();
    // ⚑ **Die Fehlerausgabe wandert in die Panik**, falls die Adresse
    // ausbleibt. Ohne das sähe ein abgelehntes Artefakt aus wie „der
    // Shard hat seine Adresse nicht genannt"; genau diese Verwechslung
    // hat `zwei_prozesse.rs` einmal Stunden gekostet.
    if leser.read_line(&mut zeile).unwrap_or(0) == 0 {
        let mut fehler = String::new();
        if let Some(mut e) = kind.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut fehler);
        }
        let _ = kind.kill();
        panic!("Shard {nummer} nannte keine Adresse.\nFehlerausgabe:\n{fehler}");
    }
    let adresse = zeile
        .trim()
        .strip_prefix(&format!("SHARD {nummer} "))
        .unwrap_or_else(|| panic!("unerwartete Zeile von Shard {nummer}: {zeile:?}"))
        .parse()
        .expect("Adresse lesbar");
    (kind, (adresse, ausweis))
}

/// Der Ausweis der Tür (Fund 611): je Shard eigene 32 Bytes, mit `0600`
/// abgelegt, wie der Knoten es tut.
fn ausweis_anlegen(nummer: usize) -> (PathBuf, myl_pod::shardweg::Ausweis) {
    // Ein Ordner je Aufruf: Die Tests dieser Datei laufen nebeneinander.
    static LAUF: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let lauf = LAUF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ordner = std::env::temp_dir().join(format!("myl-shardausweis-{}-{lauf}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [nummer as u8 + 0x41; 32];
    let datei = ordner.join(format!("ausweis-{nummer}"));
    myl_types::ortsleitung::schluessel_ablegen(&datei, &ausweis).expect("Ausweis");
    (datei, ausweis)
}

/// ⚑ **Vier Prozesse rechnen denselben Pod wie einer.**
#[test]
fn vier_prozesse_rechnen_wie_ein_prozess() {
    vier_prozesse(None);
}

/// ⚑ **Und mit ungleichen Bereichen ueber `--von` und `--bis` dasselbe**
/// (Stufe 4 des Dispatchers, 2026-10-21): 3, 7, 13 und 5 Ebenen statt
/// je 7. Token, Abdruck, Arbeit und Segmente muessen gleich sein, denn der
/// Zuschnitt aendert keine Zahl.
#[test]
fn ungleiche_bereiche_rechnen_dasselbe() {
    vier_prozesse(Some([(0, 3), (3, 10), (10, 23), (23, 28)]));
}

fn vier_prozesse(bereiche: Option<[(usize, usize); 4]>) {
    if !myl_pod::artefakte::vorhanden(&artefakte()) {
        return;
    }
    assert!(
        binary().exists(),
        "myl-shard ist nicht gebaut.\nBauen mit:\n  \
         cd COMPUTE_PIPELINE/myl-pod && cargo build --bin myl-shard"
    );

    let mut kinder: Vec<Child> = Vec::new();
    let mut adressen = Vec::new();
    for n in 0..SHARDS {
        let (kind, adresse) = starten(n, bereiche.map(|b| b[n]));
        kinder.push(kind);
        adressen.push(adresse);
    }
    // ⚑ **Aufräumen, was auch geschieht.** Ein zurückgelassener Shard
    // hält seinen Port und liesse den nächsten Lauf scheitern, ohne dass
    // die Meldung sagte, warum.
    struct Aufraeumer(Vec<Child>);
    impl Drop for Aufraeumer {
        fn drop(&mut self) {
            for k in &mut self.0 {
                let _ = k.kill();
                let _ = k.wait();
            }
        }
    }
    let _wache = Aufraeumer(kinder);

    let pod = PodId::new([0xAA; 32]);
    let weg = Arc::new(
        UeberDenDraht::neu(adressen).mit_frist(std::time::Duration::from_secs(180)),
    );
    let mut koordinator =
        Coordinator::ueber_weg(pod, EpochId(0), weg, 5_000).expect("Besetzung erhoben");
    let token = koordinator.run_prompt(1, &PROMPT, 8);
    let abdruck = koordinator.dekodier_digest(1).expect("Abdruck");
    let buendel = koordinator.build_signed_poi_bundle().expect("Buendel");

    // ⚑ **Der Vergleichswert steht fest und nicht als zweiter Lauf:**
    // Ein Test, der sich selbst vergleicht, prüft nur, dass er zweimal
    // dasselbe tut.
    //
    // 📌 **Neu erhoben am 2026-09-11** mit dem Ankermodell Qwen3-0,6B,
    // und zwar **unabhängig** vom Pod:
    //
    // ```text
    // cargo run --release --bin integer-llm-runtime -- \
    //     artifacts/sparrow-0.6b-int8 "Hello is a test of the" 8
    // ```
    //
    // Dass die vier Prozesse dasselbe liefern, ist damit eine Aussage
    // über den Shardweg und nicht über die Erwartung im Test.
    assert_eq!(
        token,
        vec![1849, 13, 5209, 3410, 264, 2033, 429, 374],
        "vier Prozesse rechnen andere Token als ein Prozess"
    );
    // ⛔️ **Fund 498 (2026-09-28): Dieser Wert war seit dem Neubau des
    // Ankerartefakts am 2026-09-21 veraltet**, und der Test damit rot, ohne
    // dass es auffiel. Der Abdruck umfasst die Logits und haengt deshalb am
    // Artefakt, nicht nur am Code. Neu erhoben auf zwei Wegen, die beide
    // `2a13b710…` liefern: vier Shards **in einem Prozess** ueber
    // `Coordinator::new` (derselbe Aufbau wie `shards_ueber_den_draht.rs`)
    // und dieser Test auf dem Stand des Commits `1445da2`, also vor den
    // Aenderungen desselben Tages. Die Token sind unveraendert.
    //
    // 📌 **Ein fest eingetragener Wert, der am Artefakt haengt, muss mit
    // dem Artefakt neu erhoben werden.** Die CI hat kein Artefakt und
    // ueberspringt den Test; rot war er nur auf der einen Maschine, die ihn
    // haette zeigen koennen, und dort lief er eine Woche lang nicht.
    assert_eq!(
        abdruck.as_ref().map(|(h, s)| (h.as_str(), *s)),
        Some(("2a13b7105cf433ad8a2adc32b0f19914fc3347c0802d73c055ef024f00e87f71", 8)),
        "der Dekodier-Abdruck weicht ab"
    );
    // 📌 **Neu erhoben am 2026-09-11** mit dem Ankermodell Qwen3-0,6B:
    // Der Wert haengt am Modellprofil (28 Ebenen, hidden 1024 statt 24
    // und 896) und aendert sich mit ihm. Die **Segmentzahl** daneben
    // aendert sich nicht, und genau die ist die Aussage.
    assert_eq!(buendel.vtfe_claimed, 12_999_998, "die beanspruchte Arbeit weicht ab");
    // ⚑ **Dreizehn und nicht acht** (Fund 187): Ein Segment ist ein
    // Vorwaertspass, nicht ein gesampeltes Token. Fuenf Prefill-
    // Positionen und acht gesampelte ergeben dreizehn.
    //
    // ⚑ **Hier stand bis zum 2026-09-06 eine Eins im Buendel**, obwohl
    // dreizehn Segmente darunter hingen. Aus dieser Zahl zieht die
    // Stichprobe der Stufe 2; zwoelf Segmente waren damit unziehbar.
    assert_eq!(buendel.segmente, 13, "die Segmentzahl weicht ab");

    eprintln!(
        "\n--- Vier Prozesse, ein Pod ---\n  Token: {token:?}\n  \
         Abdruck: {abdruck:?}\n  vTFE: {}, Segmente: {}",
        buendel.vtfe_claimed, buendel.segmente
    );
}
