//! Wann verlaesst ein Lauf die Uebertragungsform?
//!
//! ⚑ Ein Beispiel, kein Test: Es sucht die Zahl, die der Test danach
//! festhaelt.

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::trainingsschleife::{trainingsschleife, Trainingsvorgaben};

fn main() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "myelith-0.6b".into()));
    let m = load_model(&dir).expect("Modell");
    for (zaehler, schritte) in [(1i64, 40u64), (64, 40), (128, 40), (256, 40), (1024, 40)] {
        let v = Trainingsvorgaben {
            schritte,
            lr_nenner: 1,
            lr_zaehler: zaehler,
            ..Trainingsvorgaben::vorgabe()
        };
        match trainingsschleife(&m, &v) {
            Ok(e) => println!(
                "  lr_zaehler {zaehler}, {schritte} Schritte -> aus der Form bei {:?}",
                e.aus_der_form
            ),
            Err(f) => println!("  lr_zaehler {zaehler}, {schritte} Schritte -> Abbruch: {f}"),
        }
    }
}
