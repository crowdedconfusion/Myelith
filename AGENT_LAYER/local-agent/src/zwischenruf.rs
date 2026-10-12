//! **Zwischenrufe: was der Mensch tippt, waehrend der Agent arbeitet.**
//!
//! ⚑ Wunsch des Projektinhabers (2026-10-12): Waehrend eines Laufs nimmt
//! die Konsole auch Text an, und er geht an das Modell, sobald der naechste
//! Schritt ausgefuehrt ist. Die Schleife (`crate::schleife`) holt die
//! wartenden Zwischenrufe nach jedem Werkzeugschritt ab und haengt sie als
//! Nachricht des Menschen an; hat das Modell gerade abgeschlossen, bekommt
//! es sie trotzdem und arbeitet einen Schritt weiter.
//!
//! ⚑ **Prozessweit und nicht je Lauf**, denn der Mensch tippt in eine
//! Konsole, und in ihr laeuft hoechstens ein Lauf zugleich. Was beim Ende
//! eines Laufs noch wartet, holt die Konsole mit [`abholen`] ab und legt es
//! zurueck in die Eingabezeile, statt es dem naechsten Lauf unterzuschieben.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static SCHLANGE: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Ob die Schleife Zwischenrufe abholt. ⚑ **Aus, bis eine Oberflaeche sie
/// einschaltet** (die Konsole beim Start): Ein Lauf ohne jemanden, der
/// tippt (`myl`, das Fenster, die Proben), holt nichts ab, und Proben, die
/// nebeneinander laufen, nehmen einander nichts weg.
static AN: AtomicBool = AtomicBool::new(false);

/// Schaltet das Abholen durch die Schleife ein oder aus.
pub fn annehmen(an: bool) {
    AN.store(an, Ordering::SeqCst);
}

/// **Was die Schleife nach einem Schritt abholt**: nichts, solange
/// [`annehmen`] nicht eingeschaltet ist.
pub fn fuer_die_schleife() -> Vec<String> {
    if AN.load(Ordering::SeqCst) {
        abholen()
    } else {
        Vec::new()
    }
}

/// So viele Zeichen traegt ein Zwischenruf hoechstens.
pub const ZEICHEN: usize = 2000;

/// **Stellt einen Zwischenruf an.** Leerer Text wird uebergangen.
pub fn einreihen(text: &str) {
    let t: String = text.trim().chars().take(ZEICHEN).collect();
    if t.is_empty() {
        return;
    }
    if let Ok(mut s) = SCHLANGE.lock() {
        s.push(t);
    }
}

/// **Holt alle wartenden Zwischenrufe ab**, aelteste zuerst; danach ist die
/// Schlange leer.
pub fn abholen() -> Vec<String> {
    SCHLANGE.lock().map(|mut s| std::mem::take(&mut *s)).unwrap_or_default()
}

/// **Wie ein Zwischenruf beim Modell ankommt**: als Nachricht des Menschen,
/// mit dem Hinweis, dass sie waehrend der Arbeit kam.
///
/// ⚑ Der Rahmen ist englisch wie jede Ansage an das Modell (was das Modell
/// liest, folgt seinem Schliff); der Text des Menschen steht unveraendert.
pub fn nachricht(text: &str) -> String {
    format!(
        "[Message from the human, sent while you were working. Take it into account and carry on.]\n{text}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ Abholen leert die Schlange, die Reihenfolge bleibt, und Leeres
    /// wird nicht angestellt.
    #[test]
    fn einreihen_und_abholen() {
        // Die Schlange ist prozessweit: erst leeren, was andere Proben liessen.
        let _ = abholen();
        einreihen("  erst  ");
        einreihen("   ");
        einreihen("dann");
        assert_eq!(abholen(), vec!["erst".to_string(), "dann".to_string()]);
        assert!(abholen().is_empty());
        assert!(nachricht("Bitte auf Deutsch").ends_with("\nBitte auf Deutsch"));
    }
}
