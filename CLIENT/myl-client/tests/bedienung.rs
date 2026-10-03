//! **Proben ueber die Bedieninstrumente**: `myl`, die Konsole und das
//! Fenster. Sie lesen deren Quelltext, weil die Zusage an ihm haengt und
//! nicht an einem Lauf.
//!
//! ⚑ Sie standen bis zum 2026-09-30 in den Modulen `protokoll` und `ort`.
//! Die sind mit dem oertlichen Agenten nach `AGENT_LAYER/local-agent`
//! gezogen; die Instrumente liegen hier, also auch die Proben ueber sie.

use std::path::{Path, PathBuf};

fn client() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn lesen(rel: &str) -> String {
    std::fs::read_to_string(client().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// `str::floor_char_boundary` ist erst ab Rust 1.91 stabil, die Kiste
/// verspricht eine aeltere Mindestfassung (die CI prueft sie). Dasselbe,
/// von Hand.
fn zeichengrenze_unten(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// ⛔️ **Jedes Bedieninstrument schaltet das Aktionsprotokoll ein**, gleich
/// hinter `fn main()`. Ein Instrument, das es vergisst, liesse den Agenten
/// unprotokolliert laufen, und niemand saehe es.
#[test]
fn jeder_start_schaltet_das_protokoll_ein() {
    for datei in ["myl-client/src/bin/myl.rs", "myl-console/src/main.rs", "myl-oberflaeche/src/main.rs"] {
        let quelle = lesen(datei);
        let start = quelle.find("fn main()").unwrap_or_else(|| panic!("{datei}: kein main"));
        // 📌 **An einer Zeichengrenze schneiden** (Fund 495): 400 Bytes
        //   hinter `fn main()` lagen im Fenster mitten in einem ⛔️,
        //   und die Probe brach am Schnitt statt an ihrer Aussage.
        let ende = zeichengrenze_unten(&quelle, start + 400.min(quelle.len() - start));
        let rumpf = &quelle[start..ende];
        assert!(rumpf.contains("myl_local_agent::protokoll::einschalten();"), "{datei} schaltet das Protokoll nicht ein");
    }
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
