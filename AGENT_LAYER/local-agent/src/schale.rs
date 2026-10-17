//! **Die Shell fuer `sh -c` und die Bauskripte, auf jedem System gefunden.**
//!
//! # 📌 Warum es dieses Modul gibt (2026-10-17, unter Wine gefunden)
//!
//! `run_command`, die Werkzeuge aus Manifesten und der Abnahmebefehl eines
//! Loops riefen fest `sh`, der Modellbau fest `bash`. Unter Windows liegt
//! keines davon im PATH: Git for Windows bringt beide mit, legt in der
//! Vorgabe aber nur `git.exe` in den PATH. Auf einem gewoehnlichen
//! Windows-Rechner sagte `run_command` deshalb „program not found“, und der
//! CI-Laeufer, auf dem Git samt Shell im PATH liegt, sah es nicht.
//!
//! ⚑ **Gesucht wird, nicht vorausgesetzt.** Unter Unix bleibt es bei `sh`
//! und `bash` aus dem PATH. Unter Windows: zuerst im PATH, dann neben jedem
//! `git.exe` im PATH (`..\bin\`, `..\usr\bin\`), dann an den Orten, an die
//! Git for Windows sich installiert. ⚑ **Manifeste bleiben Shell-Skripte**;
//! eine zweite Sprache fuer Windows (`cmd`) hiesse, jedes Manifest zweimal
//! zu schreiben.
//!
//! ⚑ **Die Suche haengt an keinem `cfg`**: [`suchen`] nimmt PATH und Orte
//! als Werte, und die Proben pruefen sie auf jedem System mit einem
//! nachgestellten Git-Ordner.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Wo eine Shell auf diesem System liegt: unter Unix der Name selbst (der
/// PATH entscheidet), unter Windows der gefundene Pfad.
pub fn finden(name: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        let pfad = std::env::var_os("PATH").unwrap_or_default();
        let orte: Vec<PathBuf> = ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"]
            .iter()
            .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
            .map(|basis| if basis.ends_with("Local") { basis.join("Programs").join("Git") } else { basis.join("Git") })
            .collect();
        suchen(name, &pfad, &orte, true)
    } else {
        Some(PathBuf::from(name))
    }
}

/// Die Shell fuer `sh -c`, oder eine Meldung, die sagt, was fehlt.
pub fn sh() -> Result<PathBuf, String> {
    finden("sh").ok_or_else(fehlt)
}

/// `bash` fuer die Bauskripte, oder eine Meldung, die sagt, was fehlt.
pub fn bash() -> Result<PathBuf, String> {
    finden("bash").ok_or_else(fehlt)
}

fn fehlt() -> String {
    "es gibt keine Shell (sh). Unter Windows bringt Git for Windows sie mit \
     (https://git-scm.com); danach findet Myelith sie von selbst"
        .to_string()
}

/// **Die Suche selbst**, ohne Umgebung: `pfad` wie die Variable PATH, `orte`
/// die Installationsordner von Git, `windows` waehlt Trennzeichen und
/// Endung.
pub fn suchen(name: &str, pfad: &std::ffi::OsStr, orte: &[PathBuf], windows: bool) -> Option<PathBuf> {
    let datei: OsString = if windows { format!("{name}.exe").into() } else { name.into() };
    let ordner: Vec<PathBuf> = if windows {
        pfad.to_string_lossy().split(';').filter(|t| !t.is_empty()).map(PathBuf::from).collect()
    } else {
        std::env::split_paths(pfad).collect()
    };
    let liegt = |p: &Path| p.is_file().then(|| p.to_path_buf());
    // 1. Im PATH.
    if let Some(p) = ordner.iter().find_map(|o| liegt(&o.join(&datei))) {
        return Some(p);
    }
    // 2. Neben einem `git` im PATH: Git for Windows legt `cmd\git.exe` in den
    //    PATH und die Shell nach `bin\` und `usr\bin\`.
    let git: OsString = if windows { "git.exe".into() } else { "git".into() };
    for o in ordner.iter().filter(|o| o.join(&git).is_file()) {
        for kandidat in [o.join(&datei), o.join("..").join("bin").join(&datei), o.join("..").join("usr").join("bin").join(&datei)] {
            if let Some(p) = liegt(&kandidat) {
                return Some(p);
            }
        }
    }
    // 3. Die Orte, an die Git for Windows sich installiert.
    orte.iter().find_map(|g| liegt(&g.join("bin").join(&datei)).or_else(|| liegt(&g.join("usr").join("bin").join(&datei))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein nachgestellter Git-Ordner wie unter Windows: `cmd\git.exe`,
    /// `bin\sh.exe`, `bin\bash.exe`.
    fn git_ordner(d: &Path) -> PathBuf {
        let g = d.join("Git");
        for (ordner, datei) in [("cmd", "git.exe"), ("bin", "sh.exe"), ("bin", "bash.exe")] {
            std::fs::create_dir_all(g.join(ordner)).unwrap();
            std::fs::write(g.join(ordner).join(datei), "").unwrap();
        }
        g
    }

    /// ⚑ **Neben `git.exe` im PATH gefunden**, auch wenn `sh` selbst nicht
    /// im PATH liegt: die Vorgabe von Git for Windows.
    #[test]
    fn die_shell_von_git_wird_neben_git_gefunden() {
        let d = tempfile::tempdir().unwrap();
        let g = git_ordner(d.path());
        let pfad = OsString::from(format!("{};{}", d.path().join("leer").display(), g.join("cmd").display()));
        let sh = suchen("sh", &pfad, &[], true).expect("sh neben git");
        assert!(sh.ends_with(Path::new("bin").join("sh.exe")), "{}", sh.display());
        assert!(suchen("bash", &pfad, &[], true).is_some());
    }

    /// ⚑ **Ohne Git im PATH die Installationsorte**, und ohne alles nichts,
    /// statt eines Namens, der beim Start scheitert.
    #[test]
    fn ohne_path_die_installationsorte_und_sonst_nichts() {
        let d = tempfile::tempdir().unwrap();
        let g = git_ordner(d.path());
        assert!(suchen("sh", &OsString::new(), std::slice::from_ref(&g), true).is_some());
        assert_eq!(suchen("sh", &OsString::new(), &[d.path().join("fehlt")], true), None);
    }

    /// Im PATH zuerst: Wer eine eigene Shell davorlegt, bekommt sie.
    #[test]
    fn der_path_geht_vor() {
        let d = tempfile::tempdir().unwrap();
        let g = git_ordner(d.path());
        std::fs::create_dir_all(d.path().join("eigen")).unwrap();
        std::fs::write(d.path().join("eigen").join("sh.exe"), "").unwrap();
        let pfad = OsString::from(format!("{};{}", d.path().join("eigen").display(), g.join("cmd").display()));
        assert_eq!(suchen("sh", &pfad, &[], true), Some(d.path().join("eigen").join("sh.exe")));
    }

    /// Unter Unix bleibt es beim Namen; der PATH entscheidet wie bisher.
    #[cfg(unix)]
    #[test]
    fn unter_unix_bleibt_es_beim_namen() {
        assert_eq!(finden("sh"), Some(PathBuf::from("sh")));
        assert!(sh().is_ok());
    }
}
