//! **Die Module der Konsole**: was sie zeichnet.
//!
//! Ein Modul liegt als Ordner in `MODULES/` an der Wurzel des
//! Repositoriums oder im Ordner `module/` neben den Einstellungen. Die
//! Konsole prueft jedes beim Start ([`myl_module::signatur::pruefen`]);
//! nur ein gepruefetes zeigt seine Befehle in Hilfe und Vervollstaendigung,
//! und nur ein gepruefetes startet. Gestartet wird es beim ersten Befehl.
//!
//! ⚑ **Die Konsole bleibt Herrin**: Sie zeichnet allein, sie fragt den
//! Menschen in ihrem eigenen Rahmen, sie fragt das Modell und faehrt
//! Agentenlaeufe, und ein Modul bekommt nur, was seine Befugnisse nennen
//! ([`myl_module::wirt::Pruefer`]).
//!
//! ⚑ **Ohne Modul ist alles wie ohne diese Datei**: keine Befehle, kein
//! Takt, keine Fusszeile eines Moduls.

use myl_module::nachricht::{Stil, Teil};

// ⚑ **Orte, Pruefung und die Verwaltung der laufenden Module stehen in
//   `myl_client::module`** (2026-10-12), eine Stelle fuer Konsole und
//   Fenster. Hier bleibt, was nur die Konsole kann: zeichnen.
pub use myl_client::module::{einstellungsordner, installieren, liste, protokoll, speicherpfad, vertrauenspfad, Modulwirt};

/// **Der Wirt der Konsole.** ⚑ Ein Modul darf keinen Befehl der Konsole
/// belegen.
pub fn wirt() -> Modulwirt {
    Modulwirt::neu(Some(Box::new(|g| {
        let haupt = &g.beschreibung.befehle[0].name;
        crate::sitzung::eingebauter_befehl(haupt).then(|| format!("{haupt} ist ein Befehl der Konsole"))
    })))
}

/// **Teile mit Stil als eine Zeile**, auf `breite` Zeichen gekuerzt oder
/// aufgefuellt; gefaerbt erst nach dem Zuschnitt (Farbcodes zaehlen dort
/// nicht mit). `grund` ist die Farbe der Zeile.
pub fn teile_zeichnen(teile: &[Teil], breite: usize, d: myl_client::einstellungen::Konsolendesign, grund: crossterm::style::Color) -> String {
    let farbig = crate::design::farbig();
    let mut aus = String::new();
    let mut rest = breite;
    for t in teile {
        if rest == 0 {
            break;
        }
        let text: String = t.text.chars().take(rest).collect();
        rest -= text.chars().count();
        aus.push_str(&stil_faerben(t.stil, &text, d, farbig, grund));
    }
    aus.push_str(&" ".repeat(rest));
    aus
}

/// Ein Stil eines Moduls in den Farben dieser Konsole.
pub fn stil_faerben(stil: Stil, text: &str, d: myl_client::einstellungen::Konsolendesign, farbig: bool, grund: crossterm::style::Color) -> String {
    use crossterm::style::{Attribute};
    use crate::ansi::{SetAttribute, SetForegroundColor};
    if !farbig {
        return text.to_string();
    }
    let rollen = crate::design::rollen(d);
    let mit = |s: String| format!("{s}{}", SetForegroundColor(grund));
    match stil {
        Stil::Normal => text.to_string(),
        Stil::Beiwerk => mit(rollen.beiwerk.faerben(text, true)),
        Stil::Hervor | Stil::Titel => mit(rollen.ueberschrift.faerben(text, true)),
        Stil::Warnung => mit(rollen.warnung.faerben(text, true)),
        Stil::Gewinn => format!("{}{text}{}", SetForegroundColor(crate::design::GEWINN), SetForegroundColor(grund)),
        Stil::Verlust => format!("{}{}{text}{}{}", SetForegroundColor(crate::design::VERLUST), SetAttribute(Attribute::Bold), SetAttribute(Attribute::NormalIntensity), SetForegroundColor(grund)),
    }
}

/// **Text eines Moduls in den Rollbereich**, je Zeile eingerueckt, im Stil.
pub fn zeilen_ausgeben(text: &str, stil: Stil, d: myl_client::einstellungen::Konsolendesign) {
    let farbig = crate::design::farbig();
    if stil == Stil::Titel {
        println!();
    }
    for z in text.lines() {
        let grund = crossterm::style::Color::Reset;
        println!("  {}", stil_faerben(stil, z, d, farbig, grund));
    }
    if stil == Stil::Titel {
        println!();
    }
}

/// **Der Bereich unten als Text**: eine Leerzeile Abstand zur Fusszeile,
/// dann die Zeilen des Moduls, an absoluten Stellen, ohne den Wagen zu
/// merken (der Rahmen hat ihn schon gemerkt).
pub fn unten_text(sch: &crate::schirm::Schirm, zeilen: &[Vec<Teil>], d: myl_client::einstellungen::Konsolendesign) -> String {
    let platz = sch.zusatz() as usize;
    let breite = (crate::banner::fenstermasse().0 as usize).saturating_sub(4);
    let mut aus = String::new();
    for i in 0..platz {
        let inhalt = if i == 0 {
            String::new()
        } else {
            zeilen.get(i - 1).map(|t| format!("  {}", teile_zeichnen(t, breite, d, crossterm::style::Color::Reset).trim_end())).unwrap_or_default()
        };
        aus.push_str(&format!("{}{inhalt}", sch.zeile(crate::schirm::RESERVE + i as u16)));
    }
    aus
}

/// Die Zeilen, die ein Modul unten wuenscht, als Platz unter dem Rahmen
/// (eine Leerzeile dazu); null ohne Zeilen.
pub fn unten_platz(zeilen: &[Vec<Teil>]) -> u16 {
    if zeilen.is_empty() {
        0
    } else {
        zeilen.len() as u16 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teile_auf_die_breite() {
        let d = myl_client::einstellungen::Konsolendesign::default();
        let t = vec![Teil { text: "Heute ".into(), stil: Stil::Normal }, Teil { text: "+0,12 %".into(), stil: Stil::Normal }];
        assert_eq!(teile_zeichnen(&t, 20, d, crossterm::style::Color::Reset), "Heute +0,12 %       ");
        assert_eq!(teile_zeichnen(&t, 8, d, crossterm::style::Color::Reset), "Heute +0");
    }

    #[test]
    fn unten_platz_mit_leerzeile() {
        assert_eq!(unten_platz(&[]), 0);
        assert_eq!(unten_platz(&[vec![], vec![]]), 3);
    }

    #[test]
    fn verlust_bleibt_fett_und_kehrt_zurueck() {
        let d = myl_client::einstellungen::Konsolendesign::default();
        let s = stil_faerben(Stil::Verlust, "−0,72 %", d, true, crossterm::style::Color::Grey);
        assert!(s.contains("−0,72 %") && s.ends_with(&format!("{}", crate::ansi::SetForegroundColor(crossterm::style::Color::Grey))), "{s:?}");
    }
}
