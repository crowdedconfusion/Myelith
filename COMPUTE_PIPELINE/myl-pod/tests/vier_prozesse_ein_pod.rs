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
    wurzel().join("INTEGER_LLM").join("artifacts").join("myelith-0.5b")
}

fn binary() -> PathBuf {
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("target-shared").join(profil).join("myl-shard")
}

/// Startet einen Shard und liest seine Adresse von der Standardausgabe.
fn starten(nummer: usize) -> (Child, std::net::SocketAddr) {
    let mut kind = Command::new(binary())
        .arg("--artefakte")
        .arg(artefakte())
        .arg("--shard")
        .arg(nummer.to_string())
        .arg("--shards")
        .arg(SHARDS.to_string())
        .arg("--probeschluessel")
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
    (kind, adresse)
}

/// ⚑ **Vier Prozesse rechnen denselben Pod wie einer.**
#[test]
fn vier_prozesse_rechnen_wie_ein_prozess() {
    if !artefakte().exists() {
        eprintln!("Artefakt fehlt, uebersprungen");
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
        let (kind, adresse) = starten(n);
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

    // ⚑ **Der Vergleichswert ist derselbe wie im Fadenlauf.** Er steht
    // hier fest und nicht als zweiter Lauf: Ein Test, der sich selbst
    // vergleicht, prüft nur, dass er zweimal dasselbe tut.
    assert_eq!(
        token,
        vec![5726, 315, 264, 1697, 311, 15282, 323, 3535],
        "vier Prozesse rechnen andere Token als ein Prozess"
    );
    assert_eq!(
        abdruck.as_ref().map(|(h, s)| (h.as_str(), *s)),
        Some(("1f2e7886ec88fdc808a2e52a18213d14ebbeca25a7a8728f26b6b4f2741a4598", 8)),
        "der Dekodier-Abdruck weicht ab"
    );
    assert_eq!(buendel.vtfe_claimed, 12_999_997, "die beanspruchte Arbeit weicht ab");
    // ⚑ **Dreizehn und nicht acht** (Fund 187): Ein Segment ist ein
    // Vorwaertspass, nicht ein gesampeltes Token. Fuenf Prefill-
    // Positionen und acht gesampelte ergeben dreizehn, und die vTFE von
    // 12 999 997 bestaetigen dieselbe Zahl.
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
