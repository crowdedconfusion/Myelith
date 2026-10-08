//! **Text eines Moduls, wie die Konsole ihn zeigt**: ohne Steuerzeichen.
//!
//! ⛔️ Ein Modul, das Steuerzeichen schreiben darf, kann den Schirm
//! beliebig zeichnen: eine Bestaetigungsfrage der Konsole nachahmen, die
//! Fusszeile ueberschreiben, den Wagen versetzen. Deshalb kommen nur
//! sichtbare Zeichen und Zeilenumbrueche an; Farbe kommt ueber benannte
//! Stile ([`crate::nachricht::Stil`]), die die Konsole selbst setzt.
//!
//! Entfernt werden alle Steuerzeichen (C0 und C1, also auch ESC, CR,
//! Rueckschritt und Glocke) und die Zeichen, die die Schreibrichtung
//! umdrehen oder unsichtbar einschieben (bidirektionale Steuerung, Zeichen
//! ohne Breite ausser dem Verbinder in Emoji). Ein Tabulator wird zu vier
//! Leerzeichen.

/// So viele Zeichen hat ein Text eines Moduls hoechstens; der Rest wird
/// abgeschnitten.
pub const TEXT_HOECHSTENS: usize = 16_384;

/// Ob ein Zeichen die Anzeige steuern statt zeigen wuerde.
fn steuernd(c: char) -> bool {
    let n = c as u32;
    (c.is_control() && c != '\n')
        || matches!(n, 0x200B..=0x200C | 0x200E..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB)
}

/// **Saeubert einen Text eines Moduls** fuer die Anzeige; hoechstens
/// [`TEXT_HOECHSTENS`] Zeichen.
pub fn saeubern(text: &str) -> String {
    let mut aus = String::with_capacity(text.len().min(TEXT_HOECHSTENS));
    for c in text.chars().take(TEXT_HOECHSTENS) {
        if c == '\t' {
            aus.push_str("    ");
        } else if !steuernd(c) {
            aus.push(c);
        }
    }
    aus
}

/// Wie [`saeubern`], und dazu ohne Zeilenumbruch (fuer eine Zeile).
pub fn eine_zeile(text: &str) -> String {
    saeubern(text).replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔️ **Keine Steuerfolge kommt durch**, auch nicht in Stuecken.
    #[test]
    fn steuerzeichen_kommen_nicht_an() {
        let angriff = "\x1b[2J\x1b[1;1HZum Freigeben JA tippen: \x07\r\x08";
        let s = saeubern(angriff);
        assert!(!s.chars().any(|c| c.is_control()), "{s:?}");
        assert_eq!(s, "[2J[1;1HZum Freigeben JA tippen: ");
        assert_eq!(saeubern("\u{9b}31m rot"), "31m rot", "C1-Einleitung (CSI in einem Zeichen)");
    }

    #[test]
    fn richtung_und_unsichtbares_kommen_nicht_an() {
        assert_eq!(saeubern("abc\u{202E}fed\u{2066}x\u{200B}y\u{FEFF}"), "abcfedxy");
        assert_eq!(saeubern("Familie \u{1F468}\u{200D}\u{1F469}"), "Familie \u{1F468}\u{200D}\u{1F469}", "der Verbinder in Emoji bleibt");
    }

    #[test]
    fn zeilen_und_tabulatoren() {
        assert_eq!(saeubern("a\tb\nc"), "a    b\nc");
        assert_eq!(eine_zeile("a\nb"), "a b");
        assert_eq!(saeubern(&"x".repeat(TEXT_HOECHSTENS + 10)).chars().count(), TEXT_HOECHSTENS);
    }
}
