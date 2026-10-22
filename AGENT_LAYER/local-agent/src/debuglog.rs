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
//!
//! ⚑ **Ablauf und Deckel** (Festlegung des Projektinhabers, 2026-10-21): Der
//! Log enthaelt alles, also soll er nicht unbegrenzt liegen bleiben, und ein
//! Lauf, der haengt, soll die Platte nicht fuellen. Dateien aus Monaten vor
//! den letzten [`AUFBEWAHREN_MONATE`] gehen; der Ordner haelt hoechstens
//! [`ORDNER_GRENZE`] Bytes, die aeltesten Teile zuerst; ab [`TEIL_GRENZE`]
//! schreibt der Monat in einen neuen Teil (`debug-JJJJ-MM-2.log`), damit nie
//! ein ganzer Monat auf einmal gehen muss. Geprueft wird beim Einschalten und
//! bei jedem neuen Teil, nicht bei jedem Eintrag. Die Datei, in die gerade
//! geschrieben wird, bleibt immer. Gemessen vor der Grenze: 3,5 MB in neun
//! Tagen Entwicklung mit 203 Starts.

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

/// Wie viele Kalendermonate bleiben, der laufende mitgezaehlt.
pub const AUFBEWAHREN_MONATE: u32 = 3;
/// Hoechstens so viele Bytes im Ordner; danach gehen die aeltesten Teile.
pub const ORDNER_GRENZE: u64 = 512 * 1024 * 1024;
/// Ab so vielen Bytes beginnt der Monat einen neuen Teil.
pub const TEIL_GRENZE: u64 = 64 * 1024 * 1024;

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

/// Die Datei, in die der Monat von `sekunden` gerade schreibt:
/// `debug-JJJJ-MM.log`, ab dem zweiten Teil `debug-JJJJ-MM-2.log`.
pub fn datei_fuer(sekunden: u64) -> PathBuf {
    datei_in(&ordner(), monat_von(sekunden))
}

/// Jahr und Monat zu `sekunden`, in UTC.
fn monat_von(sekunden: u64) -> (u32, u32) {
    let text = crate::uhr::utc_text(sekunden);
    (text[..4].parse().unwrap_or(0), text[5..7].parse().unwrap_or(0))
}

/// Der Name eines Teils.
fn teilname((jahr, monat): (u32, u32), teil: u32) -> String {
    if teil <= 1 { format!("debug-{jahr:04}-{monat:02}.log") } else { format!("debug-{jahr:04}-{monat:02}-{teil}.log") }
}

/// Jahr, Monat und Teil aus einem Dateinamen, sonst `None`.
fn teil_aus(name: &str) -> Option<(u32, u32, u32)> {
    let rest = name.strip_prefix("debug-")?.strip_suffix(".log")?;
    let mut t = rest.split('-');
    let jahr = t.next().filter(|j| j.len() == 4)?.parse().ok()?;
    let monat = t.next().filter(|m| m.len() == 2)?.parse().ok()?;
    let teil = match t.next() {
        None => 1,
        Some(n) => n.parse().ok().filter(|n| *n >= 2)?,
    };
    t.next().is_none().then_some((jahr, monat, teil))
}

/// Alle Teile im Ordner, die aeltesten zuerst, mit ihrer Groesse.
fn teile_in(ordner: &std::path::Path) -> Vec<((u32, u32, u32), PathBuf, u64)> {
    let mut aus: Vec<_> = std::fs::read_dir(ordner)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let k = teil_aus(&e.file_name().to_string_lossy())?;
            Some((k, e.path(), e.metadata().ok()?.len()))
        })
        .collect();
    aus.sort_by_key(|(k, _, _)| *k);
    aus
}

/// Der hoechste Teil des Monats im Ordner, oder der erste.
fn datei_in(ordner: &std::path::Path, monat: (u32, u32)) -> PathBuf {
    let teil = teile_in(ordner).iter().filter(|((j, m, _), _, _)| (*j, *m) == monat).map(|((_, _, t), _, _)| *t).max();
    ordner.join(teilname(monat, teil.unwrap_or(1)))
}

/// **Ablauf und Deckel** im Ordner, fuer den laufenden Monat `monat`. Die
/// Datei `behalten` bleibt in jedem Fall. Gibt die geloeschten zurueck.
pub fn aufraeumen_in(ordner: &std::path::Path, monat: (u32, u32), behalten: &std::path::Path) -> Vec<PathBuf> {
    let zahl = |(j, m): (u32, u32)| j * 12 + m;
    let erster = zahl(monat).saturating_sub(AUFBEWAHREN_MONATE - 1);
    let mut weg = Vec::new();
    let mut teile = teile_in(ordner);
    teile.retain(|((j, m, _), p, _)| {
        let alt = zahl((*j, *m)) < erster && p != behalten;
        if alt && std::fs::remove_file(p).is_ok() {
            weg.push(p.clone());
        }
        !alt
    });
    let mut summe: u64 = teile.iter().map(|(_, _, g)| g).sum();
    for (_, p, g) in &teile {
        if summe <= ORDNER_GRENZE {
            break;
        }
        if p != behalten && std::fs::remove_file(p).is_ok() {
            summe -= g;
            weg.push(p.clone());
        }
    }
    weg
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
    if an {
        let _sperre = SCHREIBEN.lock();
        aufraeumen_in(&ordner(), monat_von(jetzt()), &datei_fuer(jetzt()));
    }
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
    let o = ordner();
    let _ = std::fs::create_dir_all(&o);
    let pfad = naechste_datei(&o, monat_von(jetzt()), text.len() as u64);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&pfad) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Die Datei fuer den naechsten Eintrag von `laenge` Bytes: der laufende
/// Teil, oder ein neuer, wenn er mit dem Eintrag ueber [`TEIL_GRENZE`] kaeme.
/// Ein neuer Teil raeumt den Ordner auf.
fn naechste_datei(ordner: &std::path::Path, monat: (u32, u32), laenge: u64) -> PathBuf {
    let jetzt_datei = datei_in(ordner, monat);
    let groesse = std::fs::metadata(&jetzt_datei).map(|m| m.len()).unwrap_or(0);
    if groesse == 0 || groesse + laenge <= TEIL_GRENZE {
        return jetzt_datei;
    }
    let teil = teil_aus(&jetzt_datei.file_name().unwrap_or_default().to_string_lossy()).map(|(_, _, t)| t).unwrap_or(1);
    let neu = ordner.join(teilname(monat, teil + 1));
    aufraeumen_in(ordner, monat, &neu);
    neu
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
        assert_eq!(monat_von(1_791_763_200), (2026, 10));
        assert_eq!(teilname((2026, 10), 1), "debug-2026-10.log");
        assert_eq!(zeitpunkt(1_791_763_200), "2026-10-12 00:00:00 UTC");
    }

    /// ⚑ **Ablauf und Deckel** (2026-10-21): Monate vor den letzten drei
    /// gehen, ein voller Ordner verliert die aeltesten Teile zuerst, die
    /// laufende Datei bleibt immer, ein voller Teil beginnt einen neuen, und
    /// fremde Dateien im Ordner bleiben.
    #[test]
    fn ablauf_und_deckel() {
        let d = std::env::temp_dir().join(format!("debuglog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let leg = |n: &str, g: u64| {
            let f = std::fs::File::create(d.join(n)).unwrap();
            f.set_len(g).unwrap();
        };
        for n in ["debug-2026-06.log", "debug-2026-07.log", "debug-2026-08.log", "debug-2026-09.log", "notizen.txt"] {
            leg(n, 10);
        }
        leg("debug-2026-10.log", 10);
        let weg = aufraeumen_in(&d, (2026, 10), &d.join("debug-2026-10.log"));
        let mut namen: Vec<String> = weg.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        namen.sort();
        assert_eq!(namen, ["debug-2026-06.log", "debug-2026-07.log"], "drei Monate bleiben, der laufende mitgezaehlt");
        assert!(d.join("notizen.txt").is_file() && d.join("debug-2026-08.log").is_file());
        // Die Namen der Teile.
        assert_eq!(teil_aus("debug-2026-10-3.log"), Some((2026, 10, 3)));
        assert_eq!(teil_aus("debug-2026-10-1.log"), None, "ein erster Teil traegt keine Nummer");
        assert_eq!(teil_aus("debug-26-10.log"), None);
        // Der Deckel: zwei grosse Teile, die laufende Datei bleibt.
        leg("debug-2026-09.log", ORDNER_GRENZE / 2 + 1);
        leg("debug-2026-10.log", ORDNER_GRENZE / 2 + 1);
        leg("debug-2026-10-2.log", 10);
        let weg = aufraeumen_in(&d, (2026, 10), &d.join("debug-2026-10-2.log"));
        assert_eq!(weg.len(), 2, "{weg:?}");
        assert!(!d.join("debug-2026-08.log").exists() && !d.join("debug-2026-09.log").exists(), "die aeltesten zuerst");
        assert!(d.join("debug-2026-10.log").is_file() && d.join("debug-2026-10-2.log").is_file());
        // Ein voller Teil beginnt einen neuen.
        leg("debug-2026-10-2.log", TEIL_GRENZE);
        assert!(naechste_datei(&d, (2026, 10), 1).ends_with("debug-2026-10-3.log"));
        assert!(naechste_datei(&d, (2026, 11), 1).ends_with("debug-2026-11.log"));
        assert_eq!(datei_in(&d, (2026, 10)), d.join("debug-2026-10-2.log"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Ueber den Jahreswechsel zaehlen die Monate weiter.
    #[test]
    fn ablauf_ueber_den_jahreswechsel() {
        let d = std::env::temp_dir().join(format!("debuglog-jahr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["debug-2026-10.log", "debug-2026-11.log", "debug-2026-12.log", "debug-2027-01.log"] {
            std::fs::write(d.join(n), "x").unwrap();
        }
        let weg = aufraeumen_in(&d, (2027, 1), &d.join("debug-2027-01.log"));
        assert_eq!(weg, [d.join("debug-2026-10.log")]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
