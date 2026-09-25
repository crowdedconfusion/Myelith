//! Die Antwort des Modells, gesetzt.
//!
//! # ⚑ Was hervorgehoben wird (Auftrag des Projektinhabers, 2026-09-24)
//!
//! Das Modell antwortet in Markdown. Bis hierher stand die Antwort roh
//! und einfarbig da, samt `#`, Sternen und Backticks. Gesetzt wird jetzt,
//! was beim Lesen fuehrt:
//!
//! | Markdown | Darstellung |
//! |---|---|
//! | `# Ueberschrift` | ohne Rauten, im Stil der Ueberschrift |
//! | `` `code` `` | ohne Backticks, im Stil fuer Code |
//! | Codeblock | jede Zeile mit einem Balken davor, im Stil fuer Code; oben die Sprache |
//! | `- punkt`, `1. punkt` | die Marke hervorgehoben, `-` `*` `+` als `•` |
//! | `**fett**` | fett, ohne Sterne |
//! | `> zitat` | mit Balken, kursiv gedaempft |
//! | `---` | eine Linie |
//!
//! ⚠️ **Bewusst nicht**: einfache Sterne und Unterstriche als Kursiv.
//! In Pfaden und Bezeichnern (`mein_wert_neu`) stehen sie ohne jede
//! Absicht, und ein Setzer, der dort Kursiv beginnt, verschluckt Zeichen
//! aus einer Antwort, die jemand abschreiben will.
//!
//! ⚑ **Ohne Farbe bleibt der Text, wie er kam** ([`crate::design::farbig`]):
//! in einer Roehre, einem Mitschnitt, mit `NO_COLOR`. Dort ist Markdown
//! die lesbarere Form, und niemand soll Steuerzeichen in einer Datei
//! finden.

use crate::design::Rollen;

/// Setzt eine Antwort; ohne `farbig` kommt sie unveraendert zurueck.
pub fn setzen(text: &str, r: &Rollen, farbig: bool) -> String {
    if !farbig {
        return text.to_string();
    }
    let mut aus: Vec<String> = Vec::new();
    let mut im_block = false;
    for zeile in text.lines() {
        let rumpf = zeile.trim_start();
        if let Some(sprache) = rumpf.strip_prefix("```") {
            if im_block {
                aus.push(r.kante.faerben("└─", true));
            } else {
                let kopf = r.kante.faerben("┌─", true);
                let sprache = sprache.trim();
                aus.push(if sprache.is_empty() {
                    kopf
                } else {
                    format!("{kopf} {}", r.beiwerk.faerben(sprache, true))
                });
            }
            im_block = !im_block;
            continue;
        }
        if im_block {
            aus.push(format!("{} {}", r.kante.faerben("│", true), r.code.faerben(zeile, true)));
            continue;
        }
        aus.push(zeile_setzen(zeile, r));
    }
    // Ein Block, der nicht geschlossen wurde, bekommt trotzdem sein Ende:
    // Sonst stuende der Balken offen da wie ein abgeschnittener Rahmen.
    if im_block {
        aus.push(r.kante.faerben("└─", true));
    }
    aus.join("\n")
}

/// **Die Eingabe des Menschen als Kaestchen** (Auftrag des
/// Projektinhabers, 2026-09-24): ein gleichmaessig getoentes Rechteck,
/// oben und unten eine Polsterzeile im selben Ton, `breite` Zeichen breit
/// ab `einzug`, der Text mittig darin.
///
/// ⚑ **Mittig, und alles andere bleibt links.** Die Eingabe ist die
/// Stimme des Menschen und steht deshalb anders als die Zeitleiste und
/// die Antwort; das Kaestchen steht buendig mit dem Eingaberahmen
/// darunter.
///
/// ⚑ **Der Ton ist eine Mischung** aus dem Hintergrund des Terminals
/// (`grund`) und dem Ton der Rolle, mit [`Rollen::deckung`]: ein Hauch
/// ueber dem, was das Terminal ohnehin zeigt, auf dunklem Grund heller,
/// auf hellem dunkler.
///
/// 📌 **Eckig und nicht verblassend.** Eine Fassung liess den Grund von
/// der Mitte zum Rand verblassen. Am Terminal des Projektinhabers, das
/// ein Hintergrundbild zeigt, legte sich jede Zelle als deckendes Band
/// darueber, und der Verlauf zerfiel in Stufen; die schwaecheren
/// Polsterzeilen waren dazu schmaler als die Textzeile. **Ein Verlauf
/// ueber Zellfarben setzt einen einfarbigen Hintergrund voraus**, und den
/// hat nicht jedes Terminal.
///
/// Ohne `farbig` steht die Zeile wie bisher: `  ❯ text`.
pub fn eingabe_kasten(
    text: &str,
    breite: usize,
    einzug: usize,
    r: &Rollen,
    grund: (u8, u8, u8),
    farbig: bool,
) -> Vec<String> {
    if !farbig {
        return vec![format!("  ❯ {text}")];
    }
    let breite = breite.max(12);
    // Innen: zwei Zeichen Luft je Seite und die Marke mit ihrem Leerzeichen.
    let innen = breite - 6;
    let stuecke = umbrechen(text, innen);
    let ton = gemischt(grund, r.eingabe.hintergrund.unwrap_or(crossterm::style::Color::Reset), r.deckung);
    let mut aus = vec![kastenzeile("", breite, einzug, r, ton)];
    for (i, stueck) in stuecke.iter().enumerate() {
        let marke = if i == 0 { "❯ " } else { "  " };
        aus.push(kastenzeile(&format!("{marke}{stueck}"), breite, einzug, r, ton));
    }
    aus.push(kastenzeile("", breite, einzug, r, ton));
    aus
}

/// Eine Zeile des Kaestchens: der Text mittig, der Grund ueber die ganze
/// Breite in einem Ton.
fn kastenzeile(text: &str, breite: usize, einzug: usize, r: &Rollen, ton: crossterm::style::Color) -> String {
    use crossterm::style::{Attribute, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor};
    let zeichen = text.chars().count();
    let links = breite.saturating_sub(zeichen) / 2;
    let rechts = breite.saturating_sub(zeichen + links);
    let fett = if r.eingabe.fett { format!("{}", SetAttribute(Attribute::Bold)) } else { String::new() };
    format!(
        "{}{}{}{fett}{}{text}{}{ResetColor}{}",
        " ".repeat(einzug),
        SetForegroundColor(r.eingabe.farbe),
        SetBackgroundColor(ton),
        " ".repeat(links),
        " ".repeat(rechts),
        SetAttribute(Attribute::Reset)
    )
}

/// **Der Hintergrund, um `promille` zum Ton hin gemischt.**
fn gemischt(grund: (u8, u8, u8), ton: crossterm::style::Color, promille: u32) -> crossterm::style::Color {
    use crossterm::style::Color;
    let Color::Rgb { r, g, b } = ton else { return ton };
    let p = promille.min(1000) as i32;
    let mische = |von: u8, zu: u8| (von as i32 + (zu as i32 - von as i32) * p / 1000).clamp(0, 255) as u8;
    Color::Rgb { r: mische(grund.0, r), g: mische(grund.1, g), b: mische(grund.2, b) }
}

/// **Bricht an Wortgrenzen um**; nur ein Wort, das allein zu lang ist,
/// wird geteilt.
///
/// 📌 Die erste Fassung schnitt nach Zeichenzahl und zerriss Woerter
/// mitten durch („Z" am Ende der einen Zeile, „eile" am Anfang der
/// naechsten). Gefunden von der Probe, die den Text aus dem Kaestchen
/// wieder zusammensetzt.
fn umbrechen(text: &str, breite: usize) -> Vec<String> {
    let mut zeilen: Vec<String> = Vec::new();
    let mut zeile = String::new();
    for wort in text.split_whitespace() {
        let mut wort: Vec<char> = wort.chars().collect();
        while wort.len() > breite {
            if !zeile.is_empty() {
                zeilen.push(std::mem::take(&mut zeile));
            }
            zeilen.push(wort[..breite].iter().collect());
            wort.drain(..breite);
        }
        let wort: String = wort.into_iter().collect();
        let noetig = zeile.chars().count() + usize::from(!zeile.is_empty()) + wort.chars().count();
        if noetig > breite && !zeile.is_empty() {
            zeilen.push(std::mem::take(&mut zeile));
        }
        if !zeile.is_empty() {
            zeile.push(' ');
        }
        zeile.push_str(&wort);
    }
    if !zeile.is_empty() || zeilen.is_empty() {
        zeilen.push(zeile);
    }
    zeilen
}

/// Eine Zeile ausserhalb eines Codeblocks.
fn zeile_setzen(zeile: &str, r: &Rollen) -> String {
    let rumpf = zeile.trim_start();
    let einzug = &zeile[..zeile.len() - rumpf.len()];

    // Ueberschrift: eine bis sechs Rauten und ein Leerzeichen.
    let rauten = rumpf.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&rauten) && rumpf[rauten..].starts_with(' ') {
        return format!("{einzug}{}", r.ueberschrift.faerben(rumpf[rauten..].trim(), true));
    }
    // Trennlinie.
    let ohne_leer: String = rumpf.chars().filter(|c| !c.is_whitespace()).collect();
    if ohne_leer.len() >= 3 && ["-", "*", "_"].iter().any(|z| ohne_leer.chars().all(|c| c.to_string() == *z)) {
        return format!("{einzug}{}", r.kante.faerben(&"─".repeat(40), true));
    }
    // Zitat.
    if let Some(inhalt) = rumpf.strip_prefix('>') {
        return format!(
            "{einzug}{} {}",
            r.kante.faerben("▏", true),
            r.gedanke.faerben(inhalt.trim_start(), true)
        );
    }
    // Aufzaehlung: `-`, `*`, `+` oder eine Zahl mit Punkt oder Klammer.
    if let Some((marke, inhalt)) = aufzaehlung(rumpf) {
        return format!("{einzug}{} {}", r.marke.faerben(&marke, true), innen_setzen(inhalt, r));
    }
    format!("{einzug}{}", innen_setzen(rumpf, r))
}

/// Die Marke einer Aufzaehlung und der Text danach.
fn aufzaehlung(rumpf: &str) -> Option<(String, &str)> {
    for m in ["- ", "* ", "+ "] {
        if let Some(inhalt) = rumpf.strip_prefix(m) {
            return Some(("•".to_string(), inhalt));
        }
    }
    let ziffern = rumpf.chars().take_while(|c| c.is_ascii_digit()).count();
    if ziffern > 0 {
        let nach = &rumpf[ziffern..];
        for z in [". ", ") "] {
            if let Some(inhalt) = nach.strip_prefix(z) {
                return Some((rumpf[..ziffern + 1].to_string(), inhalt));
            }
        }
    }
    None
}

/// **Code und Fettes innerhalb einer Zeile.**
///
/// Erst werden die Backtick-Paare gesucht, denn in Code gilt nichts
/// anderes; im Rest dann die Paare aus `**`. **Ein Zeichen ohne Partner
/// bleibt stehen**, wie es ist: Lieber ein Stern zu viel als ein
/// verschluckter.
fn innen_setzen(text: &str, r: &Rollen) -> String {
    let teile: Vec<&str> = text.split('`').collect();
    // Eine ungerade Zahl von Backticks: der letzte hat keinen Partner.
    let gepaart = if teile.len().is_multiple_of(2) { teile.len() - 1 } else { teile.len() };
    let mut aus = String::new();
    for (i, teil) in teile.iter().enumerate() {
        if i >= gepaart {
            aus.push('`');
            aus.push_str(&fett_setzen(teil));
        } else if i % 2 == 1 {
            aus.push_str(&r.code.faerben(teil, true));
        } else {
            aus.push_str(&fett_setzen(teil));
        }
    }
    aus
}

/// `**fett**` wird fett, die Sterne gehen; ein einzelnes `**` bleibt.
fn fett_setzen(text: &str) -> String {
    use crossterm::style::{Attribute, SetAttribute};
    let teile: Vec<&str> = text.split("**").collect();
    if teile.len() < 3 {
        return text.to_string();
    }
    let gepaart = if teile.len().is_multiple_of(2) { teile.len() - 1 } else { teile.len() };
    let mut aus = String::new();
    for (i, teil) in teile.iter().enumerate() {
        if i >= gepaart {
            aus.push_str("**");
            aus.push_str(teil);
        } else if i % 2 == 1 {
            aus.push_str(&format!(
                "{}{teil}{}",
                SetAttribute(Attribute::Bold),
                SetAttribute(Attribute::NormalIntensity)
            ));
        } else {
            aus.push_str(teil);
        }
    }
    aus
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_client::einstellungen::Konsolendesign;

    /// Der sichtbare Text, ohne Steuerzeichen.
    fn sichtbar(s: &str) -> String {
        let mut aus = String::new();
        let mut in_folge = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_folge = true;
            } else if in_folge {
                if c.is_ascii_alphabetic() {
                    in_folge = false;
                }
            } else {
                aus.push(c);
            }
        }
        aus
    }

    fn r() -> Rollen {
        crate::design::rollen(Konsolendesign::Tiefsee)
    }

    /// **Ohne Farbe bleibt die Antwort Zeichen fuer Zeichen, wie sie
    /// kam**, samt Markdown.
    #[test]
    fn ohne_farbe_bleibt_alles_wie_es_kam() {
        let t = "# Titel\n\n- **eins** mit `code`\n```rust\nfn x() {}\n```";
        assert_eq!(setzen(t, &r(), false), t);
    }

    /// **Gesetzt stehen die Worte da, ohne die Markdown-Zeichen**, und
    /// nichts vom Inhalt geht verloren.
    #[test]
    fn gesetzt_bleibt_der_inhalt() {
        let t = "## Ergebnis\nDie Datei `main.rs` ist **fertig**.\n- erstens\n2. zweitens\n> Hinweis";
        let s = sichtbar(&setzen(t, &r(), true));
        let zeilen: Vec<&str> = s.lines().collect();
        assert_eq!(zeilen[0], "Ergebnis");
        assert_eq!(zeilen[1], "Die Datei main.rs ist fertig.");
        assert_eq!(zeilen[2], "• erstens");
        assert_eq!(zeilen[3], "2. zweitens");
        assert_eq!(zeilen[4], "▏ Hinweis");
    }

    /// **Code steht im Stil fuer Code**, und im Block jede Zeile mit dem
    /// Balken davor und unveraendert, auch Sterne und Backticks darin.
    #[test]
    fn code_bleibt_code() {
        let t = "```sh\nls **/*.rs `pwd`\n```";
        let gesetzt = setzen(t, &r(), true);
        let s = sichtbar(&gesetzt);
        let zeilen: Vec<&str> = s.lines().collect();
        assert_eq!(zeilen, vec!["┌─ sh", "│ ls **/*.rs `pwd`", "└─"]);
        assert!(gesetzt.contains(&r().code.faerben("ls **/*.rs `pwd`", true)));
        let inline = setzen("mit `x` dabei", &r(), true);
        assert!(inline.contains(&r().code.faerben("x", true)), "{inline:?}");
    }

    /// **Ein Zeichen ohne Partner bleibt stehen.** Lieber ein Stern zu
    /// viel als ein verschluckter, und ein Unterstrich in einem Bezeichner
    /// ist kein Kursiv.
    #[test]
    fn ein_zeichen_ohne_partner_bleibt_stehen() {
        for t in ["ein ` allein", "ein ** allein", "mein_wert_neu und *stern*", "a ** b ** c ** d"] {
            let s = sichtbar(&setzen(t, &r(), true));
            let erwartet = if t == "a ** b ** c ** d" { "a  b  c ** d".to_string() } else { t.to_string() };
            assert_eq!(s, erwartet, "bei {t:?}");
        }
    }

    /// **Das Kaestchen steht mittig, bricht um und verliert nichts**;
    /// ohne Farbe bleibt die schlichte Zeile.
    #[test]
    fn die_eingabe_steht_mittig_im_kaestchen() {
        let r = crate::design::rollen(Konsolendesign::Tinte);
        assert_eq!(eingabe_kasten("hallo", 40, 7, &r, (0, 0, 0), false), vec!["  ❯ hallo".to_string()]);

        let text = "eine recht lange Eingabe, die sicher nicht in eine Zeile passt";
        let kasten = eingabe_kasten(text, 30, 7, &r, (0, 0, 0), true);
        let zeilen: Vec<String> = kasten.iter().map(|z| sichtbar(z)).collect();
        assert!(zeilen.len() >= 5, "Polster oben, Text, Polster unten: {zeilen:?}");
        for z in &zeilen {
            assert_eq!(z.chars().count(), 7 + 30, "ungleich breit: {z:?}");
            assert!(z.starts_with("       "), "der Einzug fehlt: {z:?}");
        }
        // Jede Textzeile mittig: links und rechts gleich viel Luft, bis auf eins.
        for z in &zeilen[1..zeilen.len() - 1] {
            let innen: String = z.chars().skip(7).collect();
            let links = innen.len() - innen.trim_start().len();
            let rechts = innen.len() - innen.trim_end().len();
            assert!(links.abs_diff(rechts) <= 1, "nicht mittig: {innen:?}");
        }
        assert!(zeilen[1].trim_start().starts_with("❯ "), "{:?}", zeilen[1]);
        let wieder: Vec<String> = zeilen[1..zeilen.len() - 1]
            .iter()
            .map(|z| z.trim().trim_start_matches('❯').trim().to_string())
            .collect();
        assert_eq!(wieder.join(" "), text, "Text ging beim Umbruch verloren");
        for z in &kasten {
            assert!(z.ends_with("\x1b[0m"), "der Grund reicht ueber die Zeile: {z:?}");
        }
    }

    /// **Ein Rechteck in einem Ton**: jede Zeile des Kaestchens gleich
    /// breit und mit demselben Grund, Polster wie Text; der Ton ist die
    /// Mischung aus dem Hintergrund des Terminals und dem der Rolle.
    #[test]
    fn das_kaestchen_ist_ein_rechteck_in_einem_ton() {
        use crossterm::style::{Color, SetBackgroundColor};
        let r = crate::design::rollen(Konsolendesign::Myelith);
        let grund = (20, 20, 20);
        let kasten = eingabe_kasten("eins zwei drei vier fuenf sechs sieben acht", 24, 3, &r, grund, true);
        let ton = gemischt(grund, r.eingabe.hintergrund.unwrap_or(Color::Reset), r.deckung);
        let zeichen = format!("{}", SetBackgroundColor(ton));
        for z in &kasten {
            assert_eq!(z.matches("\x1b[48;").count(), 1, "mehr als ein Grund in {z:?}");
            assert!(z.contains(&zeichen), "anderer Grund in {z:?}");
            assert_eq!(sichtbar(z).chars().count(), 3 + 24, "ungleich breit: {z:?}");
        }
        // Die Mischung: ueber dunklem Grund heller, ueber hellem dunkler.
        let ton_rolle = Color::Rgb { r: 150, g: 150, b: 200 };
        assert_eq!(gemischt((30, 30, 30), ton_rolle, 100), Color::Rgb { r: 42, g: 42, b: 47 });
        assert_eq!(gemischt((250, 250, 250), ton_rolle, 100), Color::Rgb { r: 240, g: 240, b: 245 });
        assert_eq!(gemischt((30, 30, 30), ton_rolle, 1000), ton_rolle);
    }

    /// **Ein Wort, das allein zu lang ist, wird geteilt**, und sonst
    /// nichts; eine leere Eingabe ergibt eine leere Zeile.
    #[test]
    fn der_umbruch_teilt_nur_ueberlange_woerter() {
        assert_eq!(umbrechen("abcdefghij kl", 4), vec!["abcd", "efgh", "ij", "kl"]);
        assert_eq!(umbrechen("ab cd ef", 5), vec!["ab cd", "ef"]);
        assert_eq!(umbrechen("", 5), vec![""]);
    }

    /// **Ein offener Codeblock wird geschlossen**, sonst stuende der
    /// Rahmen offen da.
    #[test]
    fn ein_offener_block_wird_geschlossen() {
        let s = sichtbar(&setzen("```\nfn x()", &r(), true));
        assert_eq!(s.lines().last(), Some("└─"));
    }

    /// **Kein Stil reicht ueber seine Zeile hinaus**: Jede gesetzte Zeile
    /// endet zurueckgesetzt oder ungefaerbt.
    #[test]
    fn kein_stil_reicht_ueber_die_zeile() {
        let t = "# A\n- `b` **c**\n```\nd\n```\n> e\n---";
        for zeile in setzen(t, &r(), true).lines() {
            // Zurueckgesetzt ist, was mit dem vollen Ruecksetzen endet oder
            // mit dem Ende des Fettdrucks, der keine Farbe setzt.
            if zeile.contains('\x1b') {
                assert!(
                    zeile.ends_with("\x1b[0m") || zeile.ends_with("\x1b[22m"),
                    "Zeile endet gefaerbt: {zeile:?}"
                );
            }
        }
    }
}
