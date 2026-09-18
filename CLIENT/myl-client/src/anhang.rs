//! **Anhaenge im Ordner des Agenten.**
//!
//! ⚑ **Die Sache selbst steht in `myl-senses`** (2026-09-17, Auftrag des
//! Projektinhabers). Hier steht nur, was **dieser** Client dazu weiss und
//! jene Kiste nicht wissen soll: dass der Ordner des Agenten `.AGENT`
//! heisst und sich mit einer `.gitignore` selbst ausschliesst.
//!
//! ⚠️ **Der Ordner wird nicht aufgeraeumt**, siehe `myl anhaenge`.

use std::path::Path;

pub use myl_senses::anhang::{
    art_bestimmen, menschlich, Anhang, Art, Sicht, AUSZUG_ZEICHEN, ORDNER,
};

/// **Der Ort der Anhaenge, relativ zur Einhaengung.**
///
/// ⚑ **Eine Stelle, von der beides kommt**, der echte Pfad und der, den
/// das Modell genannt bekommt. Zwei Angaben dafuer waeren zwei Orte, und
/// der zweite meldet sich nicht.
pub fn unterordner() -> String {
    format!("{}/{ORDNER}", crate::verlauf::ORDNER)
}

/// **Nimmt eine Datei auf**, in den Anhangordner dieses Agenten.
pub fn aufnehmen(wurzel: &Path, quelle: &Path) -> Result<Anhang, String> {
    // ⚑ **Die `.gitignore` des Agentenordners gilt auch fuer Anhaenge**,
    // denn sie steht eine Ebene hoeher und schliesst alles aus. Angelegt
    // wird sie hier, falls noch nie verdichtet wurde: Ein Arbeitsordner
    // ist sehr oft ein Repositorium, und eine angehaengte Datei, die im
    // naechsten Commit landet, waere eine Ueberraschung.
    //
    // ⚠️ **Erst den Ordner, dann die Sperre.** Sie in ein Verzeichnis zu
    // schreiben, das es noch nicht gibt, geht still daneben, und dann
    // liegt die erste angehaengte Datei ungeschuetzt da. Genau so ist
    // diese Probe beim Umzug rot geworden.
    let agentenordner = wurzel.join(crate::verlauf::ORDNER);
    std::fs::create_dir_all(&agentenordner).map_err(|f| format!("{}: {f}", agentenordner.display()))?;
    let _ = crate::verlauf::gitignore_fuer_agentenordner(&agentenordner);
    myl_senses::anhang::aufnehmen(wurzel, &unterordner(), quelle)
}

/// Was liegt schon da?
pub fn vorhandene(wurzel: &Path) -> Vec<(String, u64)> {
    myl_senses::anhang::vorhandene(wurzel, &unterordner())
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⛔️ **Der Anhang landet unter dem Ordner des Agenten**, und der
    /// schliesst sich selbst von der Versionsverwaltung aus.
    ///
    /// ⚑ Diese Probe beisst, wenn jemand die `.gitignore` hier
    /// herausnimmt: Die Kiste `myl-senses` kennt den Agentenordner
    /// nicht und legt sie nicht an.
    #[test]
    fn der_anhang_liegt_im_agentenordner_und_ist_ausgeschlossen() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let quelle = d.path().join("notiz.txt");
        std::fs::write(&quelle, "eine Zeile").expect("schreiben");
        let wurzel = tempfile::tempdir().expect("Verzeichnis");

        let a = aufnehmen(wurzel.path(), &quelle).expect("aufnehmen");
        assert_eq!(a.pfad, format!(".AGENT/{ORDNER}/notiz.txt"));
        assert!(wurzel.path().join(&a.pfad).is_file(), "die Kopie fehlt");
        let sperre = wurzel.path().join(".AGENT").join(".gitignore");
        assert!(sperre.is_file(), "der Agentenordner schliesst sich nicht aus");
        assert!(std::fs::read_to_string(&sperre).expect("lesen").contains('*'));
        assert_eq!(vorhandene(wurzel.path()).len(), 1);
    }
}
