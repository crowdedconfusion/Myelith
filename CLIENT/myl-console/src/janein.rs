//! **Ja-Nein-Fragen: ein Tastendruck, und die Frage blinkt.**
//!
//! ⚑ Festlegung des Projektinhabers (2026-10-12): Eine Bestaetigung wird
//! nicht in der Eingabezeile getippt. Solange sie offen ist, steht dort kein
//! Wagen, und die Frage selbst blinkt; `j` oder `n` beantworten sie, ohne
//! Eingabetaste. Die Eingabetaste nimmt die Vorgabe, wo es eine gibt (der
//! grosse Buchstabe in der Klammer nennt sie).
//!
//! ⚑ **Geblinkt wird vom Terminal** (Schriftart „blinkend“, `ESC[5m`) und
//! nicht durch wiederholtes Zeichnen: Die Frage steht einmal da und wird
//! erst mit der Antwort ersetzt. So bleibt die Regel der Anzeige bestehen,
//! dass eine Frage sich nicht jeden Takt neu schreibt. Ein Terminal, das
//! nicht blinkt, zeigt sie ruhig; lesbar ist sie in beiden.
//!
//! Waehrend eines Laufs liest allein der Faden der Anzeige die Tastatur
//! (Fund 483); dort gelten dieselben Regeln, gezeichnet in `anzeige.rs`.

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::schirm::Schirm;

/// Wie eine Ja-Nein-Frage ausging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Antwort {
    Ja,
    Nein,
    /// Esc oder Strg-C: nicht entschieden. Was das heisst, sagt der Aufrufer
    /// (spaeter fragen, oder wie Nein).
    Offen,
}

impl Antwort {
    /// Ja ist ja; Nein und Offen sind beides kein Ja.
    pub fn ja(self) -> bool {
        self == Antwort::Ja
    }
}

/// Die Klammer hinter der Frage: der grosse Buchstabe ist die Vorgabe.
pub fn klammer(vorgabe: Option<bool>) -> &'static str {
    match vorgabe {
        Some(true) => "[J/n]",
        Some(false) => "[j/N]",
        None => "[j/n]",
    }
}

/// **Welche Taste welche Antwort ist.** `None`: die Taste beantwortet nichts.
///
/// ⚑ `j` und `y`, denn beide Tastaturen liegen hier; Esc und Strg-C lassen
/// offen; die Eingabetaste nimmt die Vorgabe, und ohne Vorgabe nichts.
pub fn taste(code: KeyCode, strg: bool, vorgabe: Option<bool>) -> Option<Antwort> {
    match code {
        KeyCode::Char('c') if strg => Some(Antwort::Offen),
        KeyCode::Char('j' | 'J' | 'y' | 'Y') if !strg => Some(Antwort::Ja),
        KeyCode::Char('n' | 'N') if !strg => Some(Antwort::Nein),
        KeyCode::Esc => Some(Antwort::Offen),
        KeyCode::Enter => vorgabe.map(|v| if v { Antwort::Ja } else { Antwort::Nein }),
        _ => None,
    }
}

/// **Die Frage hervorgehoben**: blinkend, in der Warnfarbe des Designs
/// (fett), damit sie nicht im Grau des Beiwerks untergeht.
///
/// 📌 Gemeldet vom Projektinhaber am 2026-10-12: Die blinkende Frage stand
/// im Ton des Beiwerks und war kaum zu sehen. Ohne bekanntes Design gilt
/// das voreingestellte; ohne Farbe (`NO_COLOR`, Roehre) blinkt sie nur.
pub fn hervor(text: &str, bild: Option<myl_client::einstellungen::Konsolendesign>) -> String {
    let blinkend = format!("\x1b[5m{text}\x1b[25m");
    crate::design::rollen(bild.unwrap_or_default()).warnung.faerben(&blinkend, crate::design::farbig())
}

/// **Die Zeile der Frage**, hervorgehoben, mit der Klammer der Vorgabe.
pub fn zeile(text: &str, vorgabe: Option<bool>, bild: Option<myl_client::einstellungen::Konsolendesign>) -> String {
    format!("\r\x1b[2K  {}", hervor(&format!("{text} {}", klammer(vorgabe)), bild))
}

/// **Stellt die Frage und wartet auf eine Taste.**
///
/// Ohne Terminal kommt `Offen` zurueck: Eine Frage, die niemand liest, ist
/// keine Zustimmung. Laeuft der Rohmodus schon (beim Warten zwischen zwei
/// Runden), bleibt er an; sonst gilt er nur fuer diese Frage.
pub fn fragen(
    text: &str,
    vorgabe: Option<bool>,
    rahmen: Option<(Schirm, myl_client::einstellungen::Konsolendesign)>,
) -> Antwort {
    fragen_bis(text, vorgabe, rahmen, None).unwrap_or(Antwort::Offen)
}

/// **Wie [`fragen`], mit einer Frist**: `None`, wenn sie ohne Taste
/// verstrich. Die Frage verschwindet dann aus ihrer Zeile, und der Aufrufer
/// stellt sie spaeter neu (fuer den Schlaf, der nach einer Weile ohne
/// Antwort kommt).
pub fn fragen_bis(
    text: &str,
    vorgabe: Option<bool>,
    rahmen: Option<(Schirm, myl_client::einstellungen::Konsolendesign)>,
    frist: Option<std::time::Instant>,
) -> Option<Antwort> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Some(Antwort::Offen);
    }
    let schon_roh = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    let roh = if schon_roh { None } else { crate::auswahl::Rohmodus::an().ok() };
    if !schon_roh && roh.is_none() {
        return Some(Antwort::Offen);
    }
    // ⚑ Kein Wagen in der Eingabezeile, solange gefragt wird.
    crate::schirm::wagen_verstecken();
    if let Some((sch, bild)) = rahmen {
        crate::anzeige::eingabezeile_ohne_wagen(&sch, bild);
    }
    let mut aus = std::io::stdout();
    let _ = write!(aus, "{}", zeile(text, vorgabe, rahmen.map(|(_, b)| b).or_else(eingestelltes_design)));
    let _ = aus.flush();
    let antwort = loop {
        if let Some(f) = frist {
            let rest = f.saturating_duration_since(std::time::Instant::now());
            if rest.is_zero() || !event::poll(rest).unwrap_or(false) {
                if std::time::Instant::now() < f {
                    continue;
                }
                let _ = write!(aus, "\r\x1b[2K");
                let _ = aus.flush();
                drop(roh);
                crate::schirm::wagen_zeigen();
                return None;
            }
        }
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        if let Some(a) = taste(k.code, k.modifiers.contains(KeyModifiers::CONTROL), vorgabe) {
            break a;
        }
    };
    myl_local_agent::debuglog::eintrag("Ja-Nein-Frage", &format!("{text} {} → {antwort:?}", klammer(vorgabe)));
    // Die Frage bleibt stehen, ruhig und mit der Antwort dahinter.
    let wort = match antwort {
        Antwort::Ja => "ja",
        Antwort::Nein => "nein",
        Antwort::Offen => "offen",
    };
    let _ = write!(aus, "\r\x1b[2K  {text} {} {wort}\r\n", klammer(vorgabe));
    let _ = aus.flush();
    drop(roh);
    crate::schirm::wagen_zeigen();
    Some(antwort)
}

/// Das Design aus den Einstellungen, wo der Aufrufer keinen Rahmen kennt.
fn eingestelltes_design() -> Option<myl_client::einstellungen::Konsolendesign> {
    myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok().map(|e| e.oberflaeche.design)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Ein Tastendruck entscheidet**, die Eingabetaste nur mit Vorgabe,
    /// Esc und Strg-C lassen offen.
    #[test]
    fn tasten_und_vorgaben() {
        assert_eq!(taste(KeyCode::Char('j'), false, None), Some(Antwort::Ja));
        assert_eq!(taste(KeyCode::Char('Y'), false, None), Some(Antwort::Ja));
        assert_eq!(taste(KeyCode::Char('n'), false, Some(true)), Some(Antwort::Nein));
        assert_eq!(taste(KeyCode::Enter, false, Some(true)), Some(Antwort::Ja));
        assert_eq!(taste(KeyCode::Enter, false, Some(false)), Some(Antwort::Nein));
        assert_eq!(taste(KeyCode::Enter, false, None), None, "ohne Vorgabe entscheidet die Eingabetaste nichts");
        assert_eq!(taste(KeyCode::Esc, false, Some(true)), Some(Antwort::Offen));
        assert_eq!(taste(KeyCode::Char('c'), true, Some(true)), Some(Antwort::Offen));
        assert_eq!(taste(KeyCode::Char('x'), false, Some(true)), None);
    }

    /// ⚑ **Die Frage blinkt, und die Klammer nennt die Vorgabe.**
    #[test]
    fn die_frage_blinkt() {
        let z = zeile("Wirklich?", Some(false), None);
        assert!(z.contains("\x1b[5mWirklich? [j/N]\x1b[25m"), "{z:?}");
        assert_eq!(klammer(Some(true)), "[J/n]");
        assert_eq!(klammer(None), "[j/n]");
    }
}
