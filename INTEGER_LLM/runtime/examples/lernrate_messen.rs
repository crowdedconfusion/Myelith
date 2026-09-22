//! Misst, welche Lernrate und Schrittzahl das Ankermodell braucht.
//!
//! ⚑ **Ein Beispiel und kein Test.** Es rechnet Minuten und haengt am
//! Artefakt; was es liefert, sind die Zahlen fuer
//! `Trainingsvorgaben::vorgabe()`.
//!
//! ```text
//! cargo run --release --example lernrate_messen
//! ```

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::trainingsschleife::{trainingsschleife, Trainingsvorgaben};

fn verlust(logits: &[i32], ziel: usize, frac: u8) -> f64 {
    let skala = f64::from(1u32 << frac);
    let m = logits.iter().copied().max().unwrap_or(0) as f64;
    let summe: f64 = logits.iter().map(|z| ((f64::from(*z) - m) / skala).exp()).sum();
    summe.ln() + m / skala - f64::from(logits[ziel]) / skala
}

fn main() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "myelith-0.6b".into()));
    let m = load_model(&dir).expect("Modell");
    println!("  Nenner | Schritte | Verlust vorher -> nachher | Argmax trifft");
    for nenner_schub in [7u32, 6, 5] {
        for schritte in [30u64, 60] {
            let mut v = Trainingsvorgaben::vorgabe();
            v.lr_nenner = 1 << nenner_schub;
            v.schritte = schritte;
            let e = trainingsschleife(&m, &v).expect("Schleife");
            let a = verlust(&e.logits_erst, v.ziel, v.logit_frac);
            let b = verlust(&e.logits_letzt, v.ziel, v.logit_frac);
            let (_, nach) = e.argmax();
            println!(
                "  1<<{nenner_schub:<3} | {schritte:>8} | {a:>7.4} -> {b:>8.4} | {}",
                if nach == v.ziel { "ja" } else { "nein" }
            );
        }
    }
}
