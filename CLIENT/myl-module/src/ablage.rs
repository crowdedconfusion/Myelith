//! **Was ein abgeschottetes Modul neben sich hat**: je Modul ein Ordner mit
//! seinen Daten, seinen Geheimnissen und der Genehmigung des Menschen.
//!
//! ⚑ **Ohne die Laufzeit**, damit Klient und Werkzeug genehmigen und Geheimnisse
//! ablegen koennen, ohne WebAssembly mitzubauen.

use crate::signatur::schreiben_atomar;
use std::path::{Path, PathBuf};

/// Ein Geheimnis, hoechstens.
pub const GEHEIMNIS_HOECHSTENS: usize = 64 << 10;

const GENEHMIGT: &str = "genehmigt.json";

/// Der eigene Ort eines Moduls unter der Ablage der Konsole.
pub fn modulort(ablage: &Path, name: &str) -> PathBuf {
    ablage.join(name)
}

/// **Die Rechner, die der Mensch diesem Modul genehmigt hat.** Fehlt die
/// Datei, sind es keine.
pub fn genehmigt(ablage: &Path, name: &str) -> Vec<String> {
    std::fs::read(modulort(ablage, name).join(GENEHMIGT))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Haelt die Genehmigung des Menschen fest.
pub fn genehmigen(ablage: &Path, name: &str, rechner: &[String]) -> Result<(), String> {
    let ort = modulort(ablage, name);
    std::fs::create_dir_all(&ort).map_err(|f| format!("{}: {f}", ort.display()))?;
    let text = serde_json::to_vec_pretty(rechner).map_err(|f| f.to_string())?;
    schreiben_atomar(&ort.join(GENEHMIGT), &text, false)
}

pub fn ist_schluesselname(n: &str) -> bool {
    (1..=64).contains(&n.len()) && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// Legt ein Geheimnis eines Moduls ab, nur fuer den Besitzer lesbar.
pub fn geheimnis_setzen(ablage: &Path, name: &str, schluessel: &str, wert: &[u8]) -> Result<(), String> {
    if !ist_schluesselname(schluessel) {
        return Err(format!("kein Name fuer ein Geheimnis: {schluessel}"));
    }
    if wert.len() > GEHEIMNIS_HOECHSTENS {
        return Err(format!("groesser als {GEHEIMNIS_HOECHSTENS} Bytes"));
    }
    let ort = modulort(ablage, name).join("geheimnisse");
    std::fs::create_dir_all(&ort).map_err(|f| format!("{}: {f}", ort.display()))?;
    schreiben_atomar(&ort.join(schluessel), wert, true)
}
