//! **Eine Aufnahme mitschreiben**, ueber whisper.cpp und ein eigenes
//! kleines Hoermodell, oder mit `MYL_HOEREN=eigen` ueber den eigenen
//! whisper ([`crate::whisper`]).
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
    if eigen_gewuenscht() {
        let text = eigen(zeug, welle, sprache);
        if let Some(p) = &gewandelt {
            let _ = std::fs::remove_file(p);
        }
        return text.map(|t| if t.is_empty() { STILLE.into() } else { t });
    }

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
        return Ok(STILLE.into());
    }
    Ok(text)
}

const STILLE: &str = "Die Aufnahme enthaelt keine erkennbare Sprache.";

/// **Der eigene Weg** (CLIENT 8.2), nur mit `MYL_HOEREN=eigen`.
///
/// ⚑ **whisper.cpp bleibt Vorgabe, bis die Abnahme besteht.** Die Guete ist
/// gleich, das Tempo noch nicht (gemessen am 2026-11-11: 4,6 s gegen 0,5 s
/// je Satz). Die Gewichte liegen je Groesse in einem Ordner neben dem
/// Hoermodell von whisper.cpp ([`crate::whisper::GROESSEN`]); welche hoert,
/// waehlt [`crate::whisper::groesse_waehlen`] nach Maschine und
/// Speicherwache, oder `MYL_HOEREN_MODELL=<ordner>`. Sie werden je Aufruf
/// geladen: Ein Sinn ist nur geladen, solange er hoert.
fn eigen(zeug: &Hoerzeug, welle: &Path, sprache: &str) -> Result<String, String> {
    let audio = zeug.modell.parent().unwrap_or(Path::new("."));
    let ordner = match std::env::var("MYL_HOEREN_MODELL") {
        Ok(name) if !name.trim().is_empty() => audio.join(name.trim()),
        _ => {
            let kerne = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
            let gpu = integer_llm_kernels::ressourcen::metal::verfuegbar();
            let passt = |o: &Path| crate::wachen(crate::Art::Ton, &[&o.join("model.safetensors")]).is_ok();
            crate::whisper::groesse_waehlen(audio, gpu, kerne, passt).ok_or_else(|| {
                format!(
                    "MYL_HOEREN=eigen findet in '{}' keine Groesse von whisper, die vorliegt und neben dem Uebrigen Platz hat ({})",
                    audio.display(),
                    crate::whisper::GROESSEN.join(", ")
                )
            })?
        }
    };
    if !ordner.join("model.safetensors").is_file() {
        return Err(format!("MYL_HOEREN=eigen braucht die Gewichte von whisper in '{}'", ordner.display()));
    }
    let roh = std::fs::read(welle).map_err(|f| format!("{}: {f}", welle.display()))?;
    let ton = crate::ton::lesen(&roh).map_err(|f| format!("{}: {f}", welle.display()))?;
    let ton = crate::ton::umtasten(&ton, 16_000);
    crate::whisper::text(&ordner, &ton.proben, sprache)
}

fn eigen_gewuenscht() -> bool {
    std::env::var("MYL_HOEREN").is_ok_and(|v| v.trim() == "eigen")
}

/// Geht die Datei unveraendert an whisper.cpp? Ein `.wav`, ausser sein Kopf
/// ist lesbar und nennt etwas anderes als 16 kHz mit einem Kanal; das wandelt
/// [`nach_wav`] ohne fremdes Programm. ⚠️ Ein Kopf, den `ton` nicht liest,
/// geht weiter unveraendert durch, wie vor dem 2026-11-12.
fn ist_wav(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
        && std::fs::read(p).ok().and_then(|b| crate::ton::kopf(&b).ok()).is_none_or(|k| k.rate == 16_000 && k.kanaele == 1)
}

/// Wandelt ein WAV, das [`crate::ton`] liest, oder ein FLAC ([`crate::flac`])
/// ohne fremdes Programm; `None` fuer jedes andere Format. ⚠️ Ein WAV mit einer Art, die `ton` nicht kennt
/// (etwa A-law), geht an ffmpeg und ist kein Fehler.
fn selbst_wandeln(quelle: &Path, rate: u32) -> Result<Option<PathBuf>, String> {
    let roh = std::fs::read(quelle).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let ton = match crate::ton::lesen(&roh) {
        Ok(t) => t,
        Err(crate::ton::Tonfehler::KeinWav) if crate::flac::ist_flac(&roh) => {
            crate::flac::lesen(&roh).map_err(|f| format!("{}: {f}", quelle.display()))?
        }
        Err(crate::ton::Tonfehler::KeinWav | crate::ton::Tonfehler::Unbekannt { .. }) => return Ok(None),
        Err(f) => return Err(format!("{}: {f}", quelle.display())),
    };
    let ziel = crate::zwischenname("myl-hoeren", "wav");
    std::fs::write(&ziel, crate::ton::schreiben(&crate::ton::umtasten(&ton, rate)))
        .map_err(|f| format!("{}: {f}", ziel.display()))?;
    Ok(Some(ziel))
}

/// ⚑ **Unter macOS liest das System selbst** (`afconvert`, seit 2026-11-12):
/// m4a, AAC, MP3, FLAC, ALAC und CAF, kein Ogg. Erst wenn es fehlt oder
/// scheitert, braucht es ffmpeg; ein Fehlschlag ist hier deshalb `None`.
fn mit_afconvert(quelle: &Path, rate: u32) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let afconvert =
        crate::laufwerk::programm_suchen(&["afconvert"], None, &crate::laufwerk::heimat(), &crate::laufwerk::pfadordner())?;
    afconvert_wandeln(&afconvert, quelle, rate)
}

/// Ein Aufruf von `afconvert`: 16 Bit, ein Kanal, `rate`. Gut ist er nur,
/// wenn danach ein WAV daliegt, das [`crate::ton`] liest.
fn afconvert_wandeln(afconvert: &Path, quelle: &Path, rate: u32) -> Option<PathBuf> {
    let ziel = crate::zwischenname("myl-hoeren", "wav");
    let mut befehl = std::process::Command::new(afconvert);
    befehl.arg("-f").arg("WAVE").arg("-d").arg(format!("LEI16@{rate}")).arg("-c").arg("1").arg(quelle).arg(&ziel);
    let gut = crate::prozess::laufen(&mut befehl, crate::FRIST_HOEREN_S, 4096).is_ok_and(|a| a.gut())
        && std::fs::read(&ziel).ok().and_then(|r| crate::ton::kopf(&r).ok()).is_some();
    if gut {
        Some(ziel)
    } else {
        let _ = std::fs::remove_file(&ziel);
        None
    }
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
/// ⚑ **whisper.cpp will 16 kHz, das Sprechmodell 24.** Eine Stimmprobe auf
/// 16 kHz herunterzurechnen wirft weg, was das Sprechmodell braucht, und
/// das hoert man.
///
/// ⚑ **Ein WAV wandelt der eigene Weg** ([`crate::ton`], seit 2026-11-12):
/// jede Breite, jede Rate, jede Zahl von Kanaelen, die `ton::lesen` kennt.
/// Vorher ging auch ein WAV ueber ffmpeg, und ohne ffmpeg liess sich keine
/// Stimmprobe ablegen. Danach versucht es unter macOS `afconvert`
/// ([`mit_afconvert`]), zuletzt ffmpeg (Ogg, Opus, WebM und alles, was das
/// System nicht liest).
pub fn nach_wav_mit(quelle: &Path, rate: u32) -> Result<PathBuf, String> {
    if let Some(ziel) = selbst_wandeln(quelle, rate)? {
        return Ok(ziel);
    }
    if let Some(ziel) = mit_afconvert(quelle, rate) {
        return Ok(ziel);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein WAV mit 16 Bit, `kanaele` Kanaelen und `rate`, jede Probe `wert`.
    fn wav(rate: u32, kanaele: u16, rahmen: usize, wert: i16) -> Vec<u8> {
        let daten = rahmen * usize::from(kanaele) * 2;
        let mut b = Vec::with_capacity(44 + daten);
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + daten as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&kanaele.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * u32::from(kanaele) * 2).to_le_bytes());
        b.extend_from_slice(&(kanaele * 2).to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(daten as u32).to_le_bytes());
        for _ in 0..rahmen * usize::from(kanaele) {
            b.extend_from_slice(&wert.to_le_bytes());
        }
        b
    }

    /// ⛔️ Ein WAV mit 44,1 kHz und zwei Kanaelen wird ohne fremdes Programm
    /// zu 24 kHz und einem Kanal, eine Sekunde bleibt eine Sekunde.
    #[test]
    fn ein_wav_wandelt_sich_ohne_ffmpeg() {
        let quelle = crate::zwischenname("myl-hoeren-probe", "wav");
        std::fs::write(&quelle, wav(44_100, 2, 44_100, 8_000)).expect("schreiben");
        let ziel = selbst_wandeln(&quelle, 24_000).expect("lesbar").expect("ein WAV wandelt der eigene Weg");
        let roh = std::fs::read(&ziel).expect("lesen");
        let k = crate::ton::kopf(&roh).expect("Kopf");
        assert_eq!((k.rate, k.kanaele, k.dauer_ms()), (24_000, 1, 1_000));
        let ton = crate::ton::lesen(&roh).expect("Ton");
        let mitte = ton.proben[ton.proben.len() / 2];
        assert!((mitte - 8_000.0 / 32_768.0).abs() < 1e-3, "Pegel bleibt: {mitte}");
        for p in [&quelle, &ziel] {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Ein FLAC wandelt der eigene Weg ([`crate::flac`]): 44,1 kHz Stereo zu
    /// 16 kHz und einem Kanal, die Dauer bleibt.
    #[test]
    fn ein_flac_wandelt_sich_ohne_ffmpeg() {
        let quelle = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/flac/stereo16-mid_side.flac");
        let ziel = selbst_wandeln(&quelle, 16_000).expect("lesbar").expect("ein FLAC wandelt der eigene Weg");
        let k = crate::ton::kopf(&std::fs::read(&ziel).expect("lesen")).expect("Kopf");
        assert_eq!((k.rate, k.kanaele, k.dauer_ms()), (16_000, 1, 50));
        let _ = std::fs::remove_file(&ziel);
    }

    /// Ein anderes Format ist kein Fehler, sondern geht an ffmpeg.
    #[test]
    fn ein_anderes_format_geht_weiter() {
        let quelle = crate::zwischenname("myl-hoeren-probe", "mp3");
        std::fs::write(&quelle, b"ID3\x04\x00\x00\x00\x00\x00\x00").expect("schreiben");
        assert_eq!(selbst_wandeln(&quelle, 16_000), Ok(None));
        let _ = std::fs::remove_file(&quelle);
    }

    /// Unveraendert an whisper.cpp geht 16 kHz mit einem Kanal und ein Kopf,
    /// den `ton` nicht liest; jede andere Rate und Kanalzahl wird gewandelt.
    #[test]
    fn nur_sechzehn_kilohertz_einkanalig_geht_unveraendert() {
        let mut faelle = Vec::new();
        let unlesbar = b"RIFF\0\0\0\0".to_vec();
        for (inhalt, endung, soll) in [
            (wav(16_000, 1, 160, 1), "wav", true),
            (wav(44_100, 1, 160, 1), "wav", false),
            (wav(16_000, 2, 160, 1), "wav", false),
            (unlesbar, "wav", true),
            (wav(16_000, 1, 160, 1), "m4a", false),
        ] {
            let p = crate::zwischenname("myl-hoeren-probe", endung);
            std::fs::write(&p, inhalt).expect("schreiben");
            faelle.push((ist_wav(&p), soll));
            let _ = std::fs::remove_file(&p);
        }
        assert!(faelle.iter().all(|(ist, soll)| ist == soll), "{faelle:?}");
    }

    /// Unter macOS wandelt `afconvert` ein m4a zu WAV mit gesagter Rate und
    /// einem Kanal; Unsinn ist `None` und hinterlaesst nichts.
    #[cfg(target_os = "macos")]
    #[test]
    fn ein_m4a_wandelt_das_system() {
        let Some(afconvert) = crate::laufwerk::programm_suchen(&["afconvert"], None, Path::new("."), &crate::laufwerk::pfadordner()) else {
            eprintln!("afconvert fehlt, Probe uebersprungen");
            return;
        };
        let wav = crate::zwischenname("myl-hoeren-probe", "wav");
        let proben = (0..44_100).map(|i| 0.3 * (i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin()).collect();
        std::fs::write(&wav, crate::ton::schreiben(&crate::ton::Ton { rate: 44_100, proben })).expect("schreiben");
        let m4a = crate::zwischenname("myl-hoeren-probe", "m4a");
        let ok = std::process::Command::new(&afconvert).args(["-f", "m4af", "-d", "aac"]).arg(&wav).arg(&m4a).status().is_ok_and(|s| s.success());
        assert!(ok, "m4a nicht erzeugt");
        let ziel = afconvert_wandeln(&afconvert, &m4a, 24_000).expect("afconvert liest m4a");
        let k = crate::ton::kopf(&std::fs::read(&ziel).expect("lesen")).expect("Kopf");
        assert_eq!((k.rate, k.kanaele), (24_000, 1));
        assert!((950..=1_100).contains(&k.dauer_ms()), "Dauer {} ms", k.dauer_ms());

        let unsinn = crate::zwischenname("myl-hoeren-probe", "ogg");
        std::fs::write(&unsinn, b"kein ton").expect("schreiben");
        assert!(afconvert_wandeln(&afconvert, &unsinn, 16_000).is_none());
        for p in [&wav, &m4a, &ziel, &unsinn] {
            let _ = std::fs::remove_file(p);
        }
    }
}
