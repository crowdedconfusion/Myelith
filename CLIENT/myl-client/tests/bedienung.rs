//! **Proben ueber die Bedieninstrumente**: `myl`, die Konsole und das
//! Fenster. Sie lesen deren Quelltext, weil die Zusage an ihm haengt und
//! nicht an einem Lauf.
//!
//! ⚑ Sie standen bis zum 2026-09-30 im Modul `ort`. Das ist mit dem
//! oertlichen Agenten nach `AGENT_LAYER/local-agent` gezogen; die
//! Instrumente liegen hier, also auch die Proben ueber sie.

use std::path::{Path, PathBuf};

fn client() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn lesen(rel: &str) -> String {
    std::fs::read_to_string(client().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// **Beide Programme des Klienten loesen denselben Pfad auf.**
///
/// 📌 Bis zum 2026-09-10 tat es nur die Oberflaeche. `myl` gab aus
/// einem fremden Arbeitsverzeichnis „es fehlt das
/// Artefaktverzeichnis", obwohl das Artefakt dalag. **Zwei
/// Programme desselben Klienten beantworteten dieselbe Frage
/// verschieden**, und das faellt niemandem auf, der nur eines
/// benutzt.
#[test]
fn beide_programme_loesen_den_artefaktpfad_auf() {
    let werkzeug = lesen("myl-client/src/bin/myl.rs");
    assert!(werkzeug.contains("ort::absolut(&e.modell.artefakt)"), "`myl` nimmt den eingestellten Pfad, wie er dasteht");
}
