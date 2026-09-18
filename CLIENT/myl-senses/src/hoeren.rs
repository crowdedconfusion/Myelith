//! **Eine Aufnahme mitschreiben**, ueber whisper.cpp und ein eigenes
//! kleines Hoermodell.
//!
//! # ⚠️ whisper.cpp will 16-kHz-WAV
//!
//! Eine `m4a`-Datei vom Telefon nimmt es nicht an, und seine Meldung
//! dazu sagt niemandem, woran es lag. Deshalb wird hier umgewandelt,
//! wenn ffmpeg da ist, und sonst **ausdruecklich gesagt**, dass es
//! fehlt. Eine Voraussetzung, die erst beim Absturz sichtbar wird, ist
//! keine Voraussetzung, sondern eine Falle.

use std::path::{Path, PathBuf};

use crate::laufwerk::Hoerzeug;

/// **Schreibt eine Aufnahme mit.** `sprache` ist ein Kuerzel wie `de`
/// oder `auto`.
pub fn mitschreiben(zeug: &Hoerzeug, ton: &Path, sprache: &str) -> Result<String, String> {
    if !ton.is_file() {
        return Err(format!("die Datei '{}' gibt es nicht", ton.display()));
    }
    let sprache = if sprache.trim().is_empty() { "auto" } else { sprache.trim() };
    let gewandelt = if ist_wav(ton) { None } else { Some(nach_wav(ton)?) };
    let welle = gewandelt.as_deref().unwrap_or(ton);

    let mut befehl = std::process::Command::new(&zeug.programm);
    befehl
        .arg("-m")
        .arg(&zeug.modell)
        .arg("-f")
        .arg(welle)
        .arg("-l")
        .arg(sprache)
        .arg("-nt")
        .arg("-np")
        .arg("-t")
        .arg(crate::faeden().to_string());

    let lauf = crate::prozess::laufen(&mut befehl, crate::FRIST_HOEREN_S, crate::AUSGABEGRENZE);
    if let Some(p) = &gewandelt {
        let _ = std::fs::remove_file(p);
    }
    let a = lauf.map_err(|f| format!("das Hoermodell {}: {f}", zeug.programm.display()))?;
    if !a.gut() {
        return Err(format!(
            "Das Hoermodell ist {}. Die letzten Zeilen:\n{}",
            a.kopf(),
            crate::schwanz(&a.fehler, 15)
        ));
    }
    let text = crate::gesaeubert(&a.aus);
    if text.is_empty() {
        // ⚑ **Stille ist ein Ergebnis, kein Fehler.** Eine leere Antwort
        // ohne ein Wort dazu saehe wie ein kaputtes Werkzeug aus.
        return Ok("Die Aufnahme enthaelt keine erkennbare Sprache.".into());
    }
    Ok(text)
}

fn ist_wav(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

/// **Wandelt in 16 kHz und einen Kanal um**, wenn ffmpeg da ist.
///
/// ⚑ **Oeffentlich, weil die Stimmprobe dasselbe braucht.** Zwei
/// Umwandlungen mit zwei Einstellungen waeren zwei Orte, und der zweite
/// meldet sich nicht.
pub fn nach_wav(quelle: &Path) -> Result<PathBuf, String> {
    nach_wav_mit(quelle, 16000)
}

/// **Dieselbe Umwandlung mit gesagter Abtastrate.**
///
/// ⚑ **whisper.cpp will 16 kHz, CosyVoice will 24.** Eine Stimmprobe auf
/// 16 kHz herunterzurechnen wirft weg, was das Sprechmodell braucht, und
/// das hoert man.
pub fn nach_wav_mit(quelle: &Path, rate: u32) -> Result<PathBuf, String> {
    let ffmpeg = crate::laufwerk::programm_suchen(
        &["ffmpeg"],
        None,
        &crate::laufwerk::heimat(),
        &crate::laufwerk::pfadordner(),
    )
    .ok_or_else(|| {
        format!(
            "Die Aufnahme '{}' ist kein WAV, und ffmpeg fehlt, um sie umzuwandeln.\n\
             Entweder ffmpeg installieren oder die Datei als WAV mit 16 kHz anhaengen.",
            quelle.display()
        )
    })?;
    let ziel = crate::zwischenname("myl-hoeren", "wav");
    let mut befehl = std::process::Command::new(ffmpeg);
    befehl
        .arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(quelle)
        .arg("-ar")
        .arg(rate.to_string())
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(&ziel);
    let a = crate::prozess::laufen(&mut befehl, crate::FRIST_HOEREN_S, 4096)
        .map_err(|f| format!("ffmpeg: {f}"))?;
    if !a.gut() {
        let _ = std::fs::remove_file(&ziel);
        return Err(format!(
            "ffmpeg konnte '{}' nicht in WAV umwandeln ({}):\n{}",
            quelle.display(),
            a.kopf(),
            crate::schwanz(&a.fehler, 8)
        ));
    }
    Ok(ziel)
}

