//! Der stabile Teil des Fuzzings fuer die Web-Recherche des Agenten: Regression
//! ueber den eingecheckten Korpus, ohne Nightly und ohne `libfuzzer`.
//!
//! `cargo fuzz` **sucht**, dieser Lauf **haelt fest**; Einzelheiten zum
//! Muster in `SHARED_TYPES/myl-types/tests/fuzzkorpus.rs`.

mod fuzzziele;

use std::path::{Path, PathBuf};

fn eingaben(ziel: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz");
    let mut raus = Vec::new();
    for d in [wurzel.join("corpus").join(ziel), wurzel.join("artifacts").join(ziel)] {
        let Ok(inhalt) = std::fs::read_dir(&d) else { continue };
        for p in inhalt.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
            if let Ok(b) = std::fs::read(&p) {
                raus.push((p, b));
            }
        }
    }
    raus.sort();
    raus
}

/// ⚑ **Jedes Ziel hat Eingaben, und von jedem liest sich mindestens eine.**
/// Ein Korpus, aus dem nichts durchkommt, prueft die Aussage nie.
#[test]
fn der_korpus_laeuft_durch() {
    for (name, f) in fuzzziele::ZIELE {
        let eing = eingaben(name);
        assert!(!eing.is_empty(), "kein Korpus fuer {name} unter fuzz/corpus/{name}/");
        let gelesen = eing.iter().filter(|(_, b)| f(b)).count();
        println!("[fuzzkorpus] {name}: {} Eingaben, {gelesen} lasen sich", eing.len());
        assert!(gelesen > 0, "{name}: keine Eingabe liess sich lesen");
    }
}
