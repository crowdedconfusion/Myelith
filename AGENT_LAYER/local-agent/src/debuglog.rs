//! **Der Debug-Log: alles, was ein Lauf tut, vollstaendig mitgeschrieben.**
//!
//! ⚑ Wunsch des Projektinhabers (2026-10-12): ein Log, der alles mitschreibt,
//! ein- und ausschaltbar (`/debug on`, `/debug off` in der Konsole, ein
//! Schalter im Fenster, beide ueber die Einstellung `debug.log`), in einem
//! eigenen Ordner unter `SYSTEM/logs/debug/`, eine fortlaufende Datei je
//! Monat. Beginn und Ende stehen in Grossbuchstaben mit Datum und Uhrzeit
//! darin.
//!
//! ⛔️ **Er enthaelt alles**: Auftraege, Dateiinhalte, Werkzeugergebnisse,
//! Antworten des Modells. Deshalb steht der Ordner in der `.gitignore`, und
//! deshalb ist er aus, bis jemand ihn einschaltet.
//!
//! ⚑ **Prozessweit.** Eine Schleife, eine Runde, eine Konsole schreiben
//! ueber [`eintrag`]; ob geschrieben wird, entscheidet ein Schalter, den die
//! Oberflaeche aus ihrer Einstellung setzt ([`setzen`]). Laeuft nichts, kostet
//! ein Eintrag eine Abfrage dieses Schalters.
//!
//! ⚑ **Zeiten in UTC**, wie in der Uhr des Agenten (`crate::uhr`): ohne Bibliothek fuer Zeitzonen, und eindeutig, wenn
//! Konsole und Fenster in dieselbe Datei schreiben.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Wo der Log liegt, relativ zur Wurzel des Repositoriums.
pub const ORDNER: &str = "SYSTEM/logs/debug";

static AN: AtomicBool = AtomicBool::new(false);

/// Wer schreibt („KONSOLE“, „FENSTER“, „MYL“), fuer jede Zeile.
static WER: Mutex<String> = Mutex::new(String::new());

/// Eine Sperre, damit die Eintraege eines Prozesses nicht ineinanderlaufen.
static SCHREIBEN: Mutex<()> = Mutex::new(());

/// Schreibt der Debug-Log gerade?
pub fn an() -> bool {
    AN.load(Ordering::SeqCst)
}

/// **Der Ordner des Logs**: unter der Wurzel des Repositoriums, ohne eine
/// im Datenordner der Plattform.
pub fn ordner() -> PathBuf {
    match crate::ort::wurzel() {
        Some(w) => w.join(ORDNER),
        None => crate::ort::datenort().join("debug"),
    }
}

/// Die Datei des Monats, zu dem `sekunden` gehoert: `debug-JJJJ-MM.log`.
pub fn datei_fuer(sekunden: u64) -> PathBuf {
    let text = crate::uhr::utc_text(sekunden);
    ordner().join(format!("debug-{}.log", &text[..7]))
}

/// Datum und Uhrzeit fuer eine Zeile: `JJJJ-MM-TT HH:MM:SS UTC`.
fn zeitpunkt(sekunden: u64) -> String {
    let text = crate::uhr::utc_text(sekunden);
    text.split(", ").next().unwrap_or(&text).to_string()
}

fn jetzt() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// **Schaltet den Log ein oder aus**, wie die Einstellung es sagt. Aendert
/// sich dabei etwas, steht der Wechsel in Grossbuchstaben im Log: `wer` ist
/// die Oberflaeche, `anlass` warum (etwa „/debug on“ oder „beim Start“).
///
/// Gibt zurueck, ob sich etwas geaendert hat.
pub fn setzen(an: bool, wer: &str, anlass: &str) -> bool {
    if let Ok(mut w) = WER.lock() {
        *w = wer.to_uppercase();
    }
    let vorher = AN.swap(an, Ordering::SeqCst);
    if vorher == an {
        return false;
    }
    let wort = if an { "DEBUG GESTARTET" } else { "DEBUG BEENDET" };
    let zeile = markierung(wort, &zeitpunkt(jetzt()), wer, anlass);
    roh_schreiben(&zeile, true);
    true
}

/// Die hervorgehobene Zeile fuer Beginn und Ende, ganz in Grossbuchstaben.
pub fn markierung(wort: &str, zeit: &str, wer: &str, anlass: &str) -> String {
    let kern = format!("{wort} {zeit} · {} · {} (PROZESS {})", wer, anlass, std::process::id()).to_uppercase();
    let balken = "=".repeat(20);
    format!("\n{balken} {kern} {balken}\n\n")
}

/// **Ein Eintrag**: die Art in Grossbuchstaben, der Text eingerueckt darunter.
/// Ohne eingeschalteten Log geschieht nichts.
pub fn eintrag(art: &str, text: &str) {
    if !an() {
        return;
    }
    let wer = WER.lock().map(|w| w.clone()).unwrap_or_default();
    let mut s = format!("[{}] {wer} · {}\n", zeitpunkt(jetzt()), art.to_uppercase());
    for z in text.lines() {
        s.push_str("    ");
        s.push_str(z);
        s.push('\n');
    }
    s.push('\n');
    roh_schreiben(&s, false);
}

/// Ein Eintrag aus einem Text, der erst gebaut wird, wenn der Log an ist.
pub fn eintrag_mit(art: &str, text: impl FnOnce() -> String) {
    if an() {
        eintrag(art, &text());
    }
}

/// Schreibt an das Ende der Datei des Monats, in einem Aufruf.
///
/// ⚑ **Ein `write_all` je Eintrag, im Anhaengemodus**: Schreiben Konsole und
/// Fenster zugleich, liegen ihre Eintraege hintereinander und nicht
/// ineinander. Ein Fehler beim Schreiben bricht nie einen Lauf ab; der Log
/// ist Beiwerk.
fn roh_schreiben(text: &str, immer: bool) {
    if !immer && !an() {
        return;
    }
    let _sperre = SCHREIBEN.lock();
    let pfad = datei_fuer(jetzt());
    if let Some(o) = pfad.parent() {
        let _ = std::fs::create_dir_all(o);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&pfad) {
        let _ = f.write_all(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Die Markierung ist ganz in Grossbuchstaben** und traegt Datum und
    /// Uhrzeit; die Datei heisst nach Jahr und Monat.
    #[test]
    fn markierung_und_monatsdatei() {
        let m = markierung("DEBUG GESTARTET", "2026-10-12 13:45:12 UTC", "Konsole", "/debug on");
        assert!(m.contains("DEBUG GESTARTET 2026-10-12 13:45:12 UTC · KONSOLE · /DEBUG ON"), "{m}");
        assert_eq!(m, m.to_uppercase());
        // 2026-10-12 00:00:00 UTC
        let d = datei_fuer(1_791_763_200);
        assert!(d.ends_with("debug-2026-10.log"), "{}", d.display());
        assert_eq!(zeitpunkt(1_791_763_200), "2026-10-12 00:00:00 UTC");
    }
}
