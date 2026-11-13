//! **Text fuer das Terminal**: fremder Text ohne Steuerzeichen.
//!
//! Konsole und `myl` geben Antworten des Modells, Werkzeugausgaben und
//! Mitschnitte aus, und darin kann Text aus dem Netz oder aus Dateien
//! stehen. Eine Regel an einer Stelle; das Fenster braucht sie nicht, denn
//! ein Browser fuehrt kein `ESC` aus.

use std::borrow::Cow;
use std::io::IsTerminal;

/// ⛔️ **Fremder Text kommt ohne Steuerzeichen ins Terminal** (2026-11-12).
///
/// Antworten des Modells und Werkzeugausgaben koennen Text aus dem Netz oder
/// aus Dateien wiedergeben, und ein `ESC` darin setzte den Fenstertitel,
/// ueberschriebe Zeilen oder schriebe in manchen Terminals ueber OSC 52 in die
/// Zwischenablage. Gezeigt wird deshalb, was dasteht: Steuerzeichen ausser
/// Zeilenumbruch und Tabulator als ihre Bildzeichen (U+2400 ff., `ESC` als
/// ␛), DEL als ␡, die C1-Zeichen als U+FFFD. Die eigenen Farben und Griffe
/// der Konsole entstehen erst danach.
pub fn steuerfrei(t: &str) -> Cow<'_, str> {
    let schlecht = |c: char| (c.is_control() && c != '\n' && c != '\t') || ('\u{80}'..='\u{9f}').contains(&c);
    if !t.chars().any(schlecht) {
        return Cow::Borrowed(t);
    }
    Cow::Owned(
        t.chars()
            .map(|c| match c {
                '\n' | '\t' => c,
                '\u{0}'..='\u{1f}' => char::from_u32(0x2400 + c as u32).unwrap_or('\u{fffd}'),
                '\u{7f}' => '\u{2421}',
                c if schlecht(c) => '\u{fffd}',
                c => c,
            })
            .collect(),
    )
}

/// **Fuer die Standardausgabe**: gefiltert, wenn sie ein Terminal ist, und
/// unveraendert in eine Roehre oder Datei. ⚑ `myl` soll weiterverarbeitbar
/// bleiben; dort fuehrt niemand die Bytes aus.
pub fn fuer_ausgabe(t: &str) -> Cow<'_, str> {
    if std::io::stdout().is_terminal() {
        steuerfrei(t)
    } else {
        Cow::Borrowed(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steuerzeichen_werden_bildzeichen() {
        let titel = format!("a{}]0;gekapert{}b", char::from(0x1b), char::from(7));
        assert_eq!(steuerfrei(&titel), "a\u{241b}]0;gekapert\u{2407}b");
        assert_eq!(steuerfrei("Zeile\n\tmit Tab, äöü ␛"), "Zeile\n\tmit Tab, äöü ␛");
        assert!(matches!(steuerfrei("ganz normal"), Cow::Borrowed(_)));
        assert_eq!(steuerfrei("\r\u{7f}\u{9b}"), "\u{240d}\u{2421}\u{fffd}");
    }
}
