//! **Ein Bild in Worte fassen**, ueber llama.cpp und ein eigenes kleines
//! Sehmodell.
//!
//! ⚠️ **Was hier herauskommt, hat dieses Projekt nicht gerechnet.** Es
//! stammt aus einem fremden Modell, ist nicht bit-exakt und nicht Teil
//! des Konsenses. Die Werkzeugbeschreibung sagt das dem Modell
//! ausdruecklich, denn ein Hinweis, der es verschweigt, laedt zu einer
//! Antwort ein, die erfunden ist.

use std::path::Path;

use crate::laufwerk::Sehzeug;

/// Wonach gefragt wird, wenn niemand etwas Besonderes fragt.
///
/// ⚑ **Sie fragt nach dem Text im Bild mit.** Der haeufigste Anhang ist
/// ein Bildschirmfoto, und dort steht die Antwort meistens geschrieben.
pub const VORGABEFRAGE: &str =
    "Beschreibe das Bild genau und gib jeden Text darin woertlich wieder.";

/// Wie viele Token die Beschreibung hoechstens hat.
pub const TOKEN_VORGABE: u32 = 320;

/// **Beschreibt ein Bild.**
pub fn beschreiben(zeug: &Sehzeug, bild: &Path, frage: &str) -> Result<String, String> {
    if !bild.is_file() {
        return Err(format!("die Datei '{}' gibt es nicht", bild.display()));
    }
    let frage = if frage.trim().is_empty() { VORGABEFRAGE } else { frage };
    let mut befehl = std::process::Command::new(&zeug.programm);
    befehl
        .arg("-m")
        .arg(&zeug.modell)
        .arg("--mmproj")
        .arg(&zeug.projektor)
        .arg("--image")
        .arg(bild)
        .arg("-p")
        .arg(frage)
        .arg("-n")
        .arg(crate::zahl_aus_umgebung("MYL_SEHEN_TOKEN", TOKEN_VORGABE).to_string())
        .arg("-t")
        .arg(crate::faeden().to_string())
        .arg("--temp")
        .arg("0");

    let a = crate::prozess::laufen(&mut befehl, crate::FRIST_SEHEN_S, crate::AUSGABEGRENZE)
        .map_err(|f| format!("das Sehmodell {}: {f}", zeug.programm.display()))?;
    if !a.gut() {
        // ⚑ **Die letzten Zeilen des Protokolls, nicht alle.** Der Grund
        // steht bei llama.cpp am Ende; der Anfang ist Geraetekunde.
        return Err(format!("Das Sehmodell ist {}. Die letzten Zeilen:\n{}", a.kopf(), crate::schwanz(&a.fehler, 15)));
    }
    // ⚑ **Die Ausgabe, nicht das Protokoll.** llama.cpp schreibt seine
    // Ladezeilen nach stderr; was nach stdout geht, ist die Antwort.
    let text = crate::gesaeubert(&a.aus);
    if text.is_empty() {
        return Err("Das Sehmodell hat nichts gesagt.".into());
    }
    Ok(text)
}
