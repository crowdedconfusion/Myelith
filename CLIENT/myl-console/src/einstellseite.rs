//! Die Einstellungen, mit den Pfeiltasten bedienbar.
//!
//! # ⚑ Hoch und runter waehlt, links und rechts aendert
//!
//! **Was sich aus einer festen Menge bedienen laesst, soll sich mit
//! Pfeilen bedienen lassen** (Auftrag des Projektinhabers,
//! 2026-09-11): eine Auswahl, ein Schalter, eine Zahl, eine Grenze.
//! Was freien Text braucht, bekommt ihn auf Enter.
//!
//! # ⛑ Und damit faellt eine frühere Festlegung
//!
//! Bis hierher zeigte `/settings` nur an, mit der Begruendung, ein
//! zweiter Setzer waere die dritte Stelle, die dieselben Feinheiten
//! kennt. **Die Begruendung galt dem Setzer und nicht der Bedienung:**
//! Gesetzt wird weiterhin ausschliesslich mit
//! [`myl_client::Einstellungen::setzen`], und was `aus` bedeutet,
//! welche Felder es gibt und welcher Wert eine Grenze loescht, steht
//! unveraendert an der einen Stelle in der Kiste. **Diese Datei
//! entscheidet nur, welche Taste welchen Wert vorschlaegt.**

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};

use myl_client::einstellungen::{Einstellungen, Feld, Feldart, Feldwert, FELDER};

use crate::design::Toene;

/// Die Zeile ganz unten.
///
/// ⚑ **Eine Liste, und sie ist die einzige.** Was hier steht, wird
/// darueber behandelt; `jedes_kuerzel_wird_behandelt` haelt beides
/// zusammen.
pub const KUERZEL: &str =
    "  ↑↓ waehlen · ←→ aendern · ⏎ eingeben · ^S sichern · ^R Feld zuruecksetzen · Esc schliessen";

/// Wie weit ein Pfeil eine Zahl bewegt.
///
/// ⚑ **Abgeleitet und nicht getippt.** Ein Schritt von eins waere bei
/// 600 Antworttoken eine Geduldsprobe, einer von fuenfzig bei sechs
/// Schritten unbrauchbar. Die Groessenordnung sagt es selbst: eine
/// Stelle unter der hoechsten, mindestens eins.
pub fn schritt(wert: u64) -> u64 {
    let stellen = wert.to_string().len() as u32;
    10u64.pow(stellen.saturating_sub(2)).max(1)
}

/// **Was aus dem Wert wird, wenn ein Pfeil gedrueckt wird.**
///
/// `None` heisst: Dieses Feld laesst sich mit Pfeilen nicht aendern.
/// **Das ist eine Auskunft und kein Fehler**, und die Zeile sagt es
/// dann auch.
pub fn geaendert(f: &Feld, jetzt: &Feldwert, rechts: bool, admin: bool) -> Option<String> {
    match f.art {
        // ⚑ **Am Ende ist Schluss und es faengt nicht von vorne an.**
        // Ein Umlauf laesst jemanden, der zu weit gedrueckt hat, wieder
        // von vorne suchen.
        Feldart::Auswahl => {
            let offen: Vec<&str> = f
                .wahl
                .iter()
                .filter(|w| !w.nur_admin || admin)
                .map(|w| w.wert)
                .collect();
            let Feldwert::Text(t) = jetzt else { return None };
            let hier = offen.iter().position(|w| *w == t)?;
            let ziel = if rechts { hier + 1 } else { hier.checked_sub(1)? };
            offen.get(ziel).map(|w| w.to_string())
        }
        Feldart::Schalter => Some(if rechts { "an".into() } else { "aus".into() }),
        Feldart::Zahl => {
            let Feldwert::Zahl(n) = jetzt else { return None };
            let s = schritt(*n);
            Some(if rechts { (n + s).to_string() } else { n.saturating_sub(s).max(1).to_string() })
        }
        // ⚑ **Unter dem kleinsten Wert steht `aus`**, und das ist kein
        // Sonderfall, sondern der Sinn einer Grenze: Sie laesst sich
        // wegnehmen. Von dort aus fuehrt der Pfeil nach rechts wieder
        // hinein.
        Feldart::Grenze => match jetzt {
            Feldwert::Leer => rechts.then(|| "1".to_string()),
            Feldwert::Zahl(n) => {
                let s = schritt(*n);
                if rechts {
                    Some((n + s).to_string())
                } else if *n <= s {
                    Some("aus".to_string())
                } else {
                    Some((n - s).to_string())
                }
            }
            _ => None,
        },
        Feldart::Text | Feldart::Pfad | Feldart::Ordner => None,
    }
}

/// **Wie ein Wert in der Zeile steht.**
///
/// ⚑ **Bei einer Auswahl steht die Beschriftung und nicht die
/// Kennung.** `de` und `automatisch` sind das, was in der Ablage steht;
/// gelesen wird „Deutsch" und „Automatisch". **Wer eine Kennung sieht,
/// wo ein Wort stehen koennte, liest die Ablage und nicht die
/// Einstellung.**
///
/// ⚑ **Und `aus` heisst nur bei einer Grenze `aus`.** Ein nicht
/// gesetzter Pfad ist nicht abgeschaltet, er ist leer, und der
/// Unterschied ist genau der zwischen „keine Grenze" und „kein
/// Ordner".
pub fn anzeige(f: &Feld, w: &Feldwert) -> String {
    if let (Feldart::Auswahl, Feldwert::Text(t)) = (f.art, w) {
        if let Some(gefunden) = f.wahl.iter().find(|x| x.wert == t) {
            return gefunden.titel.to_string();
        }
    }
    match w {
        Feldwert::Leer if matches!(f.art, Feldart::Grenze) => "aus".to_string(),
        Feldwert::Leer => "(nicht gesetzt)".to_string(),
        Feldwert::Zahl(n) => n.to_string(),
        Feldwert::Schalter(true) => "an".to_string(),
        Feldwert::Schalter(false) => "aus".to_string(),
        Feldwert::Text(t) if t.is_empty() => "(nicht gesetzt)".to_string(),
        Feldwert::Text(t) => t.clone(),
    }
}

/// **Derselbe Wert, aber so, wie der Setzer ihn annimmt.**
///
/// ⚑ **Das Gegenstueck zu [`anzeige`], und es ist ein anderes.** Was
/// ein Mensch liest („Deutsch", „aus", „(nicht gesetzt)"), ist nicht
/// das, was in der Ablage steht; wer das eine fuer das andere haelt,
/// setzt „Deutsch" als Sprache und bekommt einen Fehler.
pub fn als_wert(w: &Feldwert) -> String {
    match w {
        Feldwert::Leer => "aus".to_string(),
        Feldwert::Zahl(n) => n.to_string(),
        Feldwert::Schalter(true) => "an".to_string(),
        Feldwert::Schalter(false) => "aus".to_string(),
        // ⚑ Ein leerer Text nimmt das Feld weg, und dafuer heisst das
        // Wort in diesem Programm ueberall `aus`.
        Feldwert::Text(t) if t.is_empty() => "aus".to_string(),
        Feldwert::Text(t) => t.clone(),
    }
}

/// Ob dieses Feld sich tippen laesst.
///
/// ⚑ **Zahlen und Grenzen auch** (Auftrag des Projektinhabers,
/// 2026-09-11). Wer von 600 auf 4000 will, drueckt sonst vierunddreissig
/// Mal; **ein Pfeil ist zum Nachstellen da und nicht zum Eingeben.**
pub fn mit_eingabe(f: &Feld) -> bool {
    matches!(
        f.art,
        Feldart::Text | Feldart::Pfad | Feldart::Ordner | Feldart::Zahl | Feldart::Grenze
    )
}

/// Ob dieses Feld die Pfeile ueberhaupt annimmt.
pub fn mit_pfeilen(f: &Feld) -> bool {
    !matches!(f.art, Feldart::Text | Feldart::Pfad | Feldart::Ordner)
}

/// **Zeigt die Seite und nimmt Aenderungen entgegen.**
///
/// Gibt zurueck, ob etwas geaendert wurde.
pub fn fahren(t: Toene, ordner: &std::path::Path) -> bool {
    let pfad = Einstellungen::vorgabepfad();
    let mut e = match Einstellungen::lesen(&pfad) {
        Ok(e) => e,
        Err(f) => {
            eprintln!("Die Einstellungen sind nicht lesbar: {f}");
            return false;
        }
    };
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        zeilenweise(&e, ordner);
        return false;
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        zeilenweise(&e, ordner);
        return false;
    };

    let admin = myl_client::einstellungen::ist_admin();
    let mut hier = 0usize;
    let mut geaendert_worden = false;
    let mut meldung = String::new();
    let hoehe = zeichnen(&e, hier, t, ordner, &meldung);

    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        meldung.clear();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => break,
            KeyCode::Char('c') if strg => break,
            KeyCode::Up => hier = hier.saturating_sub(1),
            KeyCode::Down => hier = (hier + 1).min(FELDER.len() - 1),
            KeyCode::Left | KeyCode::Right => {
                let f = &FELDER[hier];
                let Ok(jetzt) = e.wert(f.name) else { continue };
                match geaendert(f, &jetzt, k.code == KeyCode::Right, admin) {
                    Some(neu) => match e.setzen(f.name, &neu) {
                        Ok(()) => geaendert_worden = true,
                        Err(m) => meldung = m,
                    },
                    None if !mit_pfeilen(f) => {
                        meldung = "Dieses Feld braucht eine Eingabe: ⏎.".to_string();
                    }
                    None => {}
                }
            }
            // ⚑ **Sichern, ohne zu schliessen.** Wer lange an der Seite
            // sitzt, soll nicht erst hinausgehen muessen, um sicher zu
            // sein.
            KeyCode::Char('s') if strg => match e.schreiben(&pfad) {
                Ok(()) => {
                    geaendert_worden = false;
                    meldung = "gesichert".to_string();
                }
                Err(m) => meldung = m,
            },
            // ⚑ **Nur das gewaehlte Feld**, und das steht auch so in der
            // Zeile unten. Ein Tastendruck, der die ganze Ablage
            // zuruecksetzt, braeuchte eine Rueckfrage, und eine
            // Rueckfrage auf jedem Tastendruck ist genau das, was
            // niemand mehr liest.
            KeyCode::Char('r') if strg => {
                let f = &FELDER[hier];
                let vorgabe = Einstellungen::default();
                match vorgabe.wert(f.name).map(|w| als_wert(&w)) {
                    Ok(neu) => match e.setzen(f.name, &neu) {
                        Ok(()) => {
                            geaendert_worden = true;
                            meldung = format!("{} zurueckgesetzt", f.in_sprache(e.oberflaeche.sprache).titel);
                        }
                        Err(m) => meldung = m,
                    },
                    Err(m) => meldung = m,
                }
            }
            KeyCode::Enter => {
                let f = &FELDER[hier];
                if !mit_eingabe(f) {
                    meldung = "Dieses Feld hat feste Werte: ← und →.".to_string();
                    let _ = zeichnen_ab(hoehe, &e, hier, t, ordner, &meldung);
                    continue;
                }
                // ⚑ Getippt wird in einer eigenen Zeile unter der
                // Liste, und die Liste bleibt stehen: Wer tippt, will
                // sehen, was er aendert.
                let jetzt = e.wert(f.name).map(|w| als_wert(&w)).unwrap_or_default();
                if let Some(neu) = tippen(&f.in_sprache(e.oberflaeche.sprache), &jetzt, t) {
                    match e.setzen(f.name, &neu) {
                        Ok(()) => geaendert_worden = true,
                        Err(m) => meldung = m,
                    }
                }
            }
            _ => continue,
        }
        let _ = zeichnen_ab(hoehe, &e, hier, t, ordner, &meldung);
    }

    if geaendert_worden {
        if let Err(m) = e.schreiben(&pfad) {
            eprintln!("Die Einstellungen lassen sich nicht schreiben: {m}");
            return false;
        }
    }
    geaendert_worden
}

/// Fragt einen Text ab, unter der Liste.
fn tippen(f: &Feld, jetzt: &str, t: Toene) -> Option<String> {
    let mut aus = std::io::stdout();
    // ⚑ **Der bisherige Wert steht in der Klammer und nicht im Feld.**
    // Vorgetippt muesste man ihn erst wegloeschen; daneben ist er die
    // Auskunft, die man beim Tippen braucht.
    let _ = crossterm::queue!(
        aus,
        Print("\r\n"),
        SetForegroundColor(t.akzent),
        Print(format!("  {} ({jetzt}): ", f.titel)),
        ResetColor,
        crossterm::cursor::Show
    );
    let _ = aus.flush();
    let mut zeile = String::new();
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Enter => break,
            KeyCode::Esc => {
                zeile.clear();
                break;
            }
            KeyCode::Backspace => {
                if zeile.pop().is_some() {
                    let _ = write!(aus, "\u{8} \u{8}");
                    let _ = aus.flush();
                }
            }
            KeyCode::Char(_) if k.modifiers.contains(KeyModifiers::CONTROL) => continue,
            KeyCode::Char(c) => {
                zeile.push(c);
                let _ = write!(aus, "{c}");
                let _ = aus.flush();
            }
            _ => {}
        }
    }
    // Die getippte Zeile wieder wegnehmen, die Liste zeichnet sich neu.
    let _ = write!(aus, "\r\x1b[2K\x1b[1A");
    let _ = crossterm::execute!(aus, crossterm::cursor::Hide);
    (!zeile.trim().is_empty()).then(|| zeile.trim().to_string())
}

fn zeichnen_ab(
    hoehe: usize,
    e: &Einstellungen,
    hier: usize,
    t: Toene,
    ordner: &std::path::Path,
    meldung: &str,
) -> usize {
    let _ = write!(std::io::stdout(), "\x1b[{hoehe}A");
    zeichnen(e, hier, t, ordner, meldung)
}

/// Zeichnet die Seite und gibt ihre Hoehe zurueck.
fn zeichnen(
    e: &Einstellungen,
    hier: usize,
    t: Toene,
    ordner: &std::path::Path,
    meldung: &str,
) -> usize {
    let sprache = e.oberflaeche.sprache;
    let breite = (crate::banner::fensterbreite() as usize).clamp(40, 100);
    let mut aus = std::io::stdout();
    let mut zeilen = 0;
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide);

    let mut kopf = |text: String, farbe| {
        let _ = crossterm::queue!(
            aus,
            Print("\x1b[2K"),
            SetForegroundColor(farbe),
            Print(text),
            ResetColor,
            Print("\r\n")
        );
    };
    kopf("  Einstellungen\r\n".into(), t.akzent);
    zeilen += 2;

    let mut bereich = String::new();
    for (i, f) in FELDER.iter().enumerate() {
        let f = f.in_sprache(sprache);
        if f.bereich != bereich {
            bereich = f.bereich.to_string();
            let _ = crossterm::queue!(
                aus,
                Print("\x1b[2K\r\n\x1b[2K"),
                SetForegroundColor(t.kante),
                Print(format!("  {}", f.bereich)),
                ResetColor,
                Print("\r\n")
            );
            zeilen += 2;
        }
        let wert = e.wert(f.name).map(|w| anzeige(&f, &w)).unwrap_or_else(|m| m);
        let hierhin = i == hier;
        let marke = if hierhin { "▸" } else { " " };
        // ⚑ Die Winkel sagen, dass sich hier etwas aendern laesst, und
        // stehen nur, wo das stimmt.
        let (links, rechts) = if mit_pfeilen(&f) { ("‹ ", " ›") } else { ("  ", "") };
        let titel = format!("  {marke} {:<30}", kuerzen(f.titel, 30));
        let _ = crossterm::queue!(aus, Print("\x1b[2K"));
        if hierhin {
            let _ = crossterm::queue!(
                aus,
                SetForegroundColor(t.akzent),
                SetAttribute(Attribute::Bold),
                Print(&titel),
                SetAttribute(Attribute::Reset)
            );
        } else {
            let _ = crossterm::queue!(aus, ResetColor, Print(&titel));
        }
        let platz = breite.saturating_sub(titel.chars().count() + 6);
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(if hierhin { t.akzent } else { t.beiwerk }),
            Print(format!("{links}{}{rechts}", kuerzen(&wert, platz))),
            ResetColor,
            Print("\r\n")
        );
        zeilen += 1;
    }

    // Der Satz zum gewaehlten Feld, und darunter der Arbeitsordner.
    let f = FELDER[hier].in_sprache(sprache);
    let _ = crossterm::queue!(
        aus,
        Print("\x1b[2K\r\n\x1b[2K"),
        SetForegroundColor(t.beiwerk),
        Print(format!("  {}", kuerzen(f.hinweis, breite.saturating_sub(4)))),
        ResetColor,
        Print("\r\n\x1b[2K")
    );
    zeilen += 3;
    let zeile = if meldung.is_empty() {
        format!("  In dieser Sitzung arbeitet der Agent in {}", ordner.display())
    } else {
        format!("  ⚑ {meldung}")
    };
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(if meldung.is_empty() { t.beiwerk } else { t.akzent }),
        Print(kuerzen(&zeile, breite)),
        ResetColor,
        Print("\r\n\x1b[2K\r\n\x1b[2K")
    );
    zeilen += 3;

    // ⚑ **Die Tastenkuerzel stehen ganz unten** (Festlegung des
    // Projektinhabers, 2026-09-11). Oben stehen sie einmal im Weg und
    // danach nie wieder dort, wo man sie sucht: **Was man beim Bedienen
    // braucht, gehoert an den Rand, an dem der Blick ohnehin endet.**
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(t.kante),
        Print(kuerzen(KUERZEL, breite)),
        ResetColor,
        Print("\r\n")
    );
    zeilen += 1;
    let _ = aus.flush();
    zeilen
}

fn kuerzen(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    text.chars().take(breite.saturating_sub(1)).chain(['…']).collect()
}

/// Ohne Terminal bleibt es beim Anzeigen.
fn zeilenweise(e: &Einstellungen, ordner: &std::path::Path) {
    println!();
    println!("  Datei: {}", Einstellungen::vorgabepfad().display());
    for f in FELDER {
        let f = f.in_sprache(e.oberflaeche.sprache);
        let wert = e.wert(f.name).map(|w| anzeige(&f, &w)).unwrap_or_else(|m| m);
        println!("  {:<22} {:<28} {}", f.name, wert, f.titel);
    }
    println!();
    println!("  In dieser Sitzung arbeitet der Agent in:");
    println!("    {}", ordner.display());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feld(name: &str) -> Feld {
        *FELDER.iter().find(|f| f.name == name).expect(name)
    }

    /// **Der Schritt folgt der Groessenordnung.**
    #[test]
    fn der_schritt_passt_zur_zahl() {
        assert_eq!(schritt(6), 1);
        assert_eq!(schritt(12), 1);
        assert_eq!(schritt(600), 10);
        assert_eq!(schritt(4096), 100);
        assert_eq!(schritt(0), 1);
    }

    /// **Ein Schalter kippt in beide Richtungen.**
    #[test]
    fn ein_schalter_kippt() {
        let f = feld("agent.schreiben");
        let aus = Feldwert::Schalter(false);
        assert_eq!(geaendert(&f, &aus, true, false).as_deref(), Some("an"));
        assert_eq!(geaendert(&f, &aus, false, false).as_deref(), Some("aus"));
    }

    /// **Eine Auswahl laeuft nicht um.**
    ///
    /// ⛑ Ein Umlauf laesst jemanden, der zu weit gedrueckt hat, wieder
    /// von vorne suchen.
    #[test]
    fn eine_auswahl_haelt_an_ihren_enden() {
        let f = feld("agent.modus");
        let erster = Feldwert::Text("auto".into());
        let letzter = Feldwert::Text("manual".into());
        assert_eq!(geaendert(&f, &erster, true, false).as_deref(), Some("manual"));
        assert_eq!(geaendert(&f, &erster, false, false), None);
        assert_eq!(geaendert(&f, &letzter, true, false), None);
        assert_eq!(geaendert(&f, &letzter, false, false).as_deref(), Some("auto"));
    }

    /// **Ohne Adminmarke gibt es die verborgene Kiste auch hier nicht.**
    #[test]
    fn die_verborgene_wahl_bleibt_verborgen() {
        let f = feld("agent.werkzeuge");
        let letzte_offene = Feldwert::Text("advanced".into());
        assert_eq!(geaendert(&f, &letzte_offene, true, false), None);
        assert_eq!(geaendert(&f, &letzte_offene, true, true).as_deref(), Some("1337"));
    }

    /// **Unter dem kleinsten Wert steht `aus`, und von dort geht es
    /// wieder hinein.**
    #[test]
    fn eine_grenze_laesst_sich_wegnehmen() {
        let f = feld("kap.speicher");
        assert_eq!(geaendert(&f, &Feldwert::Zahl(1), false, false).as_deref(), Some("aus"));
        assert_eq!(geaendert(&f, &Feldwert::Leer, true, false).as_deref(), Some("1"));
        assert_eq!(geaendert(&f, &Feldwert::Leer, false, false), None);
        assert_eq!(geaendert(&f, &Feldwert::Zahl(12), true, false).as_deref(), Some("13"));
    }

    /// **Eine Zahl faellt nie unter eins.**
    ///
    /// ⚠️ Null Schritte waeren kein enger gestellter Agent, sondern
    /// einer, der nicht antwortet.
    #[test]
    fn eine_zahl_bleibt_brauchbar() {
        let f = feld("agent.schritte");
        assert_eq!(geaendert(&f, &Feldwert::Zahl(1), false, false).as_deref(), Some("1"));
        assert_eq!(geaendert(&f, &Feldwert::Zahl(6), true, false).as_deref(), Some("7"));
    }

    /// **Eine Auswahl zeigt ihre Beschriftung, keine Kennung.**
    #[test]
    fn eine_auswahl_zeigt_was_dasteht() {
        let f = feld("oberflaeche.sprache");
        assert_eq!(anzeige(&f, &Feldwert::Text("de".into())), "Deutsch");
        let m = feld("agent.modus");
        assert_eq!(anzeige(&m, &Feldwert::Text("manual".into())), "manual mode");
    }

    /// **`aus` steht nur da, wo es `aus` heisst.**
    ///
    /// ⛑ Ein nicht gesetzter Ordner ist nicht abgeschaltet, er ist
    /// leer.
    #[test]
    fn ein_leerer_ordner_ist_nicht_aus() {
        assert_eq!(anzeige(&feld("kap.speicher"), &Feldwert::Leer), "aus");
        assert_eq!(anzeige(&feld("ausgabe.ordner"), &Feldwert::Leer), "(nicht gesetzt)");
    }

    /// **Jedes Kuerzel der Fusszeile wird auch behandelt.**
    ///
    /// ⛑ **Dieselbe Klasse wie Fund 271.** Eine Zeile mit
    /// Tastenkuerzeln, die von Hand gepflegt wird, nennt irgendwann
    /// eines, das nichts tut, und **das sieht erst der, der es
    /// ausprobiert.**
    #[test]
    fn jedes_kuerzel_wird_behandelt() {
        let quelle = include_str!("einstellseite.rs");
        // Was in der Zeile steht, muss im Tastenzweig vorkommen.
        let paare = [
            ("↑↓", "KeyCode::Up"),
            ("←→", "KeyCode::Left | KeyCode::Right"),
            ("⏎", "KeyCode::Enter"),
            ("^S", "KeyCode::Char('s') if strg"),
            ("^R", "KeyCode::Char('r') if strg"),
            ("Esc", "KeyCode::Esc"),
        ];
        for (zeichen, zweig) in paare {
            assert!(KUERZEL.contains(zeichen), "`{zeichen}` fehlt in der Fusszeile");
            assert!(quelle.contains(zweig), "`{zeichen}` wird nicht behandelt ({zweig})");
        }
    }

    /// **Jeder Standardwert wird vom Setzer angenommen.**
    ///
    /// ⛑ Die Gegenprobe zu `^R`: Ein Zuruecksetzen, das der Setzer
    /// ablehnt, waere eine Taste, die eine Fehlermeldung erzeugt statt
    /// eines Wertes.
    #[test]
    fn jeder_standardwert_laesst_sich_setzen() {
        let vorgabe = Einstellungen::default();
        let mut e = Einstellungen::default();
        for f in FELDER {
            let w = vorgabe.wert(f.name).expect(f.name);
            let wort = als_wert(&w);
            e.setzen(f.name, &wort)
                .unwrap_or_else(|m| panic!("{}: Vorgabe `{wort}` wird abgelehnt: {m}", f.name));
        }
    }

    /// **Zahlen und Grenzen lassen sich auch tippen.**
    ///
    /// ⚑ Auftrag des Projektinhabers: Wer von 600 auf 4000 will,
    /// drueckt sonst vierunddreissig Mal.
    #[test]
    fn zahlen_lassen_sich_tippen() {
        for name in ["modell.token", "agent.schritte", "kap.speicher"] {
            assert!(mit_eingabe(&feld(name)), "{name} nimmt keine Eingabe");
            assert!(mit_pfeilen(&feld(name)), "{name} nimmt keine Pfeile");
        }
        // Und eine Auswahl bleibt bei den Pfeilen.
        assert!(!mit_eingabe(&feld("agent.modus")));
    }

    /// **Ein Textfeld sagt Nein und nicht irgendetwas.**
    #[test]
    fn ein_textfeld_nimmt_keine_pfeile() {
        for name in ["modell.artefakt", "agent.wurzel", "ausgabe.ordner"] {
            let f = feld(name);
            assert!(!mit_pfeilen(&f), "{name} nimmt Pfeile an");
            assert_eq!(
                geaendert(&f, &Feldwert::Text("x".into()), true, false),
                None,
                "{name} aendert sich mit einem Pfeil"
            );
        }
    }

    /// **Jeder Wert, den ein Pfeil vorschlaegt, wird auch angenommen.**
    ///
    /// ⛑ **Die Gegenprobe zur ganzen Datei.** Diese hier schlaegt vor,
    /// gesetzt wird in der Kiste; ein Vorschlag, den der Setzer ablehnt,
    /// waere eine Taste, die nichts tut und nicht sagt, warum.
    #[test]
    fn jeder_vorschlag_wird_angenommen() {
        let mut e = Einstellungen::default();
        for f in FELDER {
            if !mit_pfeilen(&f) {
                continue;
            }
            for _ in 0..12 {
                for rechts in [true, false] {
                    let jetzt = e.wert(f.name).expect(f.name);
                    if let Some(neu) = geaendert(&f, &jetzt, rechts, true) {
                        e.setzen(f.name, &neu).unwrap_or_else(|m| {
                            panic!("{}: Vorschlag `{neu}` wird abgelehnt: {m}", f.name)
                        });
                    }
                }
            }
        }
    }
}
