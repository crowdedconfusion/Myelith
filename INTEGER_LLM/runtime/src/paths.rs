//! Zentrale Pfadkonstanten fuer calibrate, runtime und pipeline.
//!
//! Ablageorte werden ausschliesslich ueber diese Konstanten bezogen,
//! damit keine Pfade in den Komponenten hart kodiert werden.

use std::env;
use std::path::PathBuf;

/// Artefakt-Verzeichnis, relativ zum Arbeitsverzeichnis.
pub const ARTIFACTS_DIR: &str = "artifacts";

// ⛔️ **Grabstein: `MODELS_DIR` ist am 2026-09-21 entfallen** (Fund 409).
//
// Die Konstante stand hier, seit es diese Datei gibt, und hatte in Rust
// **keinen einzigen Leser**: Sie spiegelte nur die Python-Seite, damit
// beide denselben Namen nennen. Beim Umzug der Quellmodelle nach
// `MODELS/llm` waere sie damit eine falsche Angabe geworden, die niemand
// benutzt und die der Naechste fuer gueltig haelt.
//
// ⚑ **Und der Grund, warum die Laufzeit sie nie brauchte, ist der
// Vertrag selbst:** Sie rechnet auf **Artefakten**, nie auf
// Quellmodellen. Ein Quellmodell ist Gleitkomma und Voraussetzung des
// **Baus**; wer es im Rechenpfad braeuchte, haette den Pfad verlassen.
// Der Ablageort der Quellmodelle gehoert deshalb dorthin, wo gebaut
// wird, und das ist `calibrate/src/paths.py`.

/// Umgebungsvariable, mit der das Artefakt-Verzeichnis ueberschrieben wird.
pub const ARTIFACTS_DIR_ENV: &str = "INTEGER_LLM_ARTIFACTS_DIR";

/// Aufgeloester Pfad zum Artefakt-Verzeichnis.
///
/// Verwendet die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR`, falls diese
/// gesetzt und nicht leer ist; sonst `artifacts/` relativ zum aktuellen
/// Arbeitsverzeichnis.
pub fn artifacts_dir() -> PathBuf {
    match env::var(ARTIFACTS_DIR_ENV) {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(ARTIFACTS_DIR),
    }
}

/// Artefakt-Verzeichnis eines bestimmten Modells,
/// z. B. `artifacts/myelith-0.6b`.
pub fn model_artifacts_dir(model_name: &str) -> PathBuf {
    artifacts_dir().join(model_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifacts_dir_default_and_env() {
        // Ein Test fuer beide Pfade: env-Zugriffe waeren zwischen parallel
        // laufenden Tests nicht deterministisch.
        env::remove_var(ARTIFACTS_DIR_ENV);
        assert_eq!(artifacts_dir(), PathBuf::from(ARTIFACTS_DIR));
        assert_eq!(
            model_artifacts_dir("myelith-0.6b"),
            PathBuf::from(ARTIFACTS_DIR).join("myelith-0.6b")
        );

        env::set_var(ARTIFACTS_DIR_ENV, "/tmp/integer-llm-artefakte-test");
        assert_eq!(
            artifacts_dir(),
            PathBuf::from("/tmp/integer-llm-artefakte-test")
        );
        env::remove_var(ARTIFACTS_DIR_ENV);
    }
}
