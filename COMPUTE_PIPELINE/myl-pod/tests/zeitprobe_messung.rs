//! **Fund 655 nachgemessen: was ein Miner mit sehr kleinem Bereich unter der
//! festen Zugabe verstecken kann** (2026-11-17). Eine Messung, keine Probe:
//! ausgeschaltet, auf ruhiger Maschine von Hand zu starten.
//!
//! ```text
//! MYL_POD_MODELL=pigeon-4b-int8 cargo test --test zeitprobe_messung -- --ignored --nocapture
//! ```
//!
//! # Die Rechnung (Knoten, `frist.rs`)
//!
//! Wer `Z` Mikrosekunden je Token ansagt, hat die Frist `2·Z` und bekommt eine
//! Probe von `k = clamp(2 s / Z, 64, 2048)` Positionen. Gemessen wird
//! `(Dauer − Zugabe) / k` mit der Zugabe 2 s. Wer wirklich `T` je Token
//! braucht, faellt also erst auf, wenn `T − 2 s / k > 2·Z`. Daraus die
//! kleinste Ansage, mit der er durchkommt:
//!
//! - `T ≤ 2 s / 2048 ≈ 977 µs`: jede Ansage, auch null.
//! - `T ≤ 3 · 977 µs`: `Z = (T − 977) / 2`.
//! - darueber: `Z = T / 3` (das Dreifache, wie im Kommentar von
//!   `probe_positionen`).
//!
//! Gemessen wird hier `T` fuer die kleinsten Bereiche, die eine Zuteilung
//! hergibt, mit derselben Rechnung wie die Zeitprobe des Shards.

use std::sync::Arc;
use std::time::Instant;

use integer_llm_runtime::loader::load_model;
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;

/// Zugabe und Ziel wie im Knoten (`frist::ZUGABE_US`, `PROBE_ZIEL_US`).
const ZUGABE_US: f64 = 2_000_000.0;

/// Die kleinste Ansage `Z` in µs, mit der ein Shard, der wirklich `t` µs je
/// Token braucht, nicht auffaellt.
fn kleinste_ansage(t: f64) -> f64 {
    let schwelle = ZUGABE_US / 2048.0;
    if t <= schwelle {
        0.0
    } else if t <= 3.0 * schwelle {
        (t - schwelle) / 2.0
    } else {
        t / 3.0
    }
}

#[test]
#[ignore = "Messung auf ruhiger Maschine, von Hand"]
fn was_ein_kleiner_bereich_unter_der_zugabe_versteckt() {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "pigeon-4b-int8".to_string());
    let dir = std::path::PathBuf::from(m).join("../../INTEGER_LLM/artifacts").join(&modell);
    if !myl_pod::artefakte::vorhanden(&dir) {
        eprintln!("⚠️ {modell} fehlt");
        return;
    }
    let geladen = Instant::now();
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let nl = model.num_layers;
    eprintln!("{modell}: {nl} Ebenen, geladen in {} ms", geladen.elapsed().as_millis());
    let mitte = nl / 2;
    // (Name, von, bis, Kopf)
    let bereiche = [
        ("1 Ebene (Mitte)", mitte, mitte + 1, false),
        ("2 Ebenen", mitte, mitte + 2, false),
        ("4 Ebenen", mitte, mitte + 4, false),
        ("1 Ebene mit Kopf", nl - 1, nl, true),
    ];
    eprintln!("| Bereich | k | T je Token (µs) | kleinste Ansage Z (µs) | T / Z |");
    eprintln!("|---|---|---|---|---|");
    for (name, von, bis, kopf) in bereiche {
        let sk = BlsSecretKey::key_gen(&[7u8; 32]).expect("BLS");
        let shard = ShardNode::new(1, von, bis, false, kopf, model.clone(), sk, 4);
        // Einmal warm, damit die Seiten der Gewichte im Speicher liegen.
        let _ = shard.zeitprobe(64);
        for k in [64usize, 512, 2048] {
            let mut kuerzeste = f64::MAX;
            for _ in 0..3 {
                let ab = Instant::now();
                let _ = shard.zeitprobe(k);
                kuerzeste = kuerzeste.min(ab.elapsed().as_secs_f64() * 1e6);
            }
            let t = kuerzeste / k as f64;
            let z = kleinste_ansage(t);
            let verhaeltnis = if z > 0.0 { format!("{:.1}", t / z) } else { "unbegrenzt".to_string() };
            eprintln!("| {name} | {k} | {t:.0} | {z:.0} | {verhaeltnis} |");
        }
    }
}

/// Die Rechnung selbst, an ihren Raendern.
#[test]
fn die_kleinste_ansage_folgt_der_frist() {
    let s = ZUGABE_US / 2048.0;
    assert_eq!(kleinste_ansage(s), 0.0);
    assert!((kleinste_ansage(2.0 * s) - s / 2.0).abs() < 1e-9);
    assert!((kleinste_ansage(3.0 * s) - s).abs() < 1e-9);
    assert!((kleinste_ansage(30.0 * s) - 10.0 * s).abs() < 1e-9);
}
