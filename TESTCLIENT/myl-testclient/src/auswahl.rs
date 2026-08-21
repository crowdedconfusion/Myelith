//! Menüauswahl mit Pfeiltasten.
//!
//! Ein Menüpunkt wird mit ↑/↓ angesteuert und mit Enter bestätigt. Die
//! Ziffer bleibt daneben gültig: Wer den Punkt kennt, tippt sie und ist
//! sofort dort, ohne sich durch die Liste zu bewegen.
//!
//! ## Warum es trotzdem noch einen zweiten Weg gibt
//!
//! Pfeiltasten brauchen den **Rohmodus** des Terminals — die Eingabe wird
//! dann Taste für Taste geliefert statt zeilenweise. Das setzt ein echtes
//! Terminal voraus. Fehlt es, weil die Eingabe aus einer Pipe, einer
//! Datei oder einem Testlauf kommt, schaltet dieses Modul auf
//! **zeilenweise Eingabe** zurück und liest wie zuvor eine Ziffer.
//!
//! Beides ist nötig, nicht nur eines: Der Client wird von Hand bedient
//! **und** in Skripten aufgerufen, und ein Werkzeug, das im Skript auf
//! eine Tastatur wartet, hängt still.
//!
//! ## Der Rohmodus muss auf jedem Weg zurückgenommen werden
//!
//! Ein Terminal, das im Rohmodus zurückbleibt, zeigt keine Eingaben mehr
//! an und reagiert nicht auf Strg-C — der Nutzer sieht dann nicht den
//! Fehler, sondern eine kaputte Shell. Deshalb liegt die Rücknahme in
//! [`Rohmodus`], einem Wächter mit `Drop`: Sie läuft bei normaler
//! Rückkehr, bei vorzeitigem `return` und beim Abwickeln nach einer
//! Panik. Strg-C wird zusätzlich selbst behandelt, weil das Signal im
//! Rohmodus nicht mehr vom Terminal erzeugt wird.

use std::io::{self, IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};

/// Ein Menüpunkt.
pub struct Punkt {
    /// Ziffer oder Buchstabe — der Zweitweg und zugleich der Rückgabewert.
    ///
    /// Der Aufrufer verzweigt weiterhin über dieses Zeichen. Damit ändert
    /// die Umstellung auf Pfeiltasten die Menülogik nicht, sondern nur die
    /// Art, wie das Zeichen zustande kommt.
    pub taste: char,
    pub titel: String,
    /// Erläuterung unter dem Titel. Leer lassen, wenn der Titel genügt.
    pub hinweis: String,
}

impl Punkt {
    pub fn neu(taste: char, titel: &str, hinweis: &str) -> Self {
        Self {
            taste,
            titel: titel.to_string(),
            hinweis: hinweis.to_string(),
        }
    }
}

/// Nimmt den Rohmodus beim Verlassen des Gültigkeitsbereichs zurück.
pub(crate) struct Rohmodus;

impl Rohmodus {
    pub(crate) fn an() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for Rohmodus {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), cursor::Show);
    }
}

/// Zeigt die Auswahl und liefert die Taste des gewählten Punkts.
///
/// `None` heißt abgebrochen (Esc oder Eingabeende) — der Aufrufer
/// behandelt das wie „zurück".
pub fn waehlen(kopf: &str, punkte: &[Punkt]) -> Option<char> {
    if punkte.is_empty() {
        return None;
    }
    // Kein Terminal (Pipe, Datei, Test): zeilenweise lesen.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return zeilenweise(kopf, punkte);
    }
    // Rohmodus nicht verfügbar (etwa in einer Umgebung ohne Terminfo):
    // ebenfalls zurückfallen, statt den Nutzer vor einem toten Menü
    // sitzen zu lassen.
    match interaktiv(kopf, punkte) {
        Ok(wahl) => wahl,
        Err(_) => zeilenweise(kopf, punkte),
    }
}

/// Wie viele Zeilen ein Punkt belegt.
fn hoehe(p: &Punkt) -> usize {
    1 + if p.hinweis.is_empty() {
        0
    } else {
        p.hinweis.lines().count()
    }
}

/// Wie viele Zeilen die gezeichnete Liste insgesamt belegt.
///
/// Muss mit [`zeichnen`] übereinstimmen: zwei Zeilen Kopf, die Punkte,
/// zwei Zeilen Fußnote. Ist die Zahl zu klein, bleiben beim Neuzeichnen
/// Reste stehen; ist sie zu groß, frisst der Cursor die Zeile darüber.
fn gezeichnete_hoehe(punkte: &[Punkt]) -> usize {
    2 + punkte.iter().map(hoehe).sum::<usize>() + 2
}

fn interaktiv(kopf: &str, punkte: &[Punkt]) -> io::Result<Option<char>> {
    let zeilen = gezeichnete_hoehe(punkte);

    // Passt die Liste nicht ins Fenster, scrollt das Terminal beim
    // Zeichnen — und `MoveToPreviousLine` landet dann auf einer anderen
    // Zeile als der, an der die Liste begann. Das Ergebnis wäre ein Menü,
    // das sich bei jedem Tastendruck selbst zerlegt. In dem Fall ist die
    // zeilenweise Ausgabe nicht der schlechtere Weg, sondern der einzige,
    // der funktioniert.
    // Unbekannte Fenstergröße wird wie „zu klein" behandelt: Ohne die Höhe
    // ist nicht entscheidbar, ob das Neuzeichnen trägt.
    let passt = terminal::size().is_ok_and(|(_, h)| (h as usize) > zeilen);
    if !passt {
        return Ok(zeilenweise(kopf, punkte));
    }

    let _roh = Rohmodus::an()?;
    let mut aus = io::stdout();
    execute!(aus, cursor::Hide)?;

    let mut markiert = 0usize;
    let mut erste_ausgabe = true;

    loop {
        if !erste_ausgabe {
            queue!(
                aus,
                cursor::MoveToPreviousLine(zeilen as u16),
                Clear(ClearType::FromCursorDown)
            )?;
        }
        erste_ausgabe = false;
        zeichnen(&mut aus, kopf, punkte, Some(markiert))?;
        aus.flush()?;

        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event::read()?
        else {
            continue;
        };
        // Windows liefert Press und Release; ohne diese Prüfung zählt
        // jeder Tastendruck doppelt.
        if kind != KeyEventKind::Press {
            continue;
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                markiert = if markiert == 0 {
                    punkte.len() - 1
                } else {
                    markiert - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                markiert = (markiert + 1) % punkte.len();
            }
            KeyCode::Home => markiert = 0,
            KeyCode::End => markiert = punkte.len() - 1,
            KeyCode::Enter | KeyCode::Char(' ') => return Ok(Some(punkte[markiert].taste)),
            KeyCode::Esc => return Ok(None),
            // Strg-C erzeugt im Rohmodus kein Signal mehr. Ohne eigene
            // Behandlung wäre das Menü nicht mehr zu verlassen.
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                drop(_roh);
                println!();
                std::process::exit(130);
            }
            KeyCode::Char(c) => {
                if let Some(p) = punkte.iter().find(|p| p.taste == c) {
                    return Ok(Some(p.taste));
                }
            }
            _ => {}
        }
    }
}

/// Zeichnet Kopf und Liste. `markiert` ist im Zeilenmodus `None`.
fn zeichnen(
    aus: &mut impl Write,
    kopf: &str,
    punkte: &[Punkt],
    markiert: Option<usize>,
) -> io::Result<()> {
    // Im Rohmodus setzt `\n` den Cursor nach unten, ohne ihn an den
    // Zeilenanfang zu holen — ohne `\r` liefe die Ausgabe treppenförmig.
    let umbruch = if markiert.is_some() { "\r\n" } else { "\n" };

    queue!(aus, Print(format!("{}  ── {} ──{}", umbruch, kopf, umbruch)))?;

    for (i, p) in punkte.iter().enumerate() {
        let ist_markiert = markiert == Some(i);
        let zeiger = if ist_markiert { "❯" } else { " " };
        if ist_markiert {
            queue!(aus, SetAttribute(Attribute::Reverse))?;
        }
        queue!(
            aus,
            Print(format!("  {} {}  {}", zeiger, p.taste, p.titel))
        )?;
        if ist_markiert {
            queue!(aus, SetAttribute(Attribute::Reset))?;
        }
        queue!(aus, Print(umbruch))?;

        for zeile in p.hinweis.lines() {
            queue!(aus, Print(format!("        {}{}", zeile, umbruch)))?;
        }
    }

    match markiert {
        Some(_) => queue!(
            aus,
            Print(format!(
                "{}  ↑ ↓ bewegen · Enter wählen · Ziffer direkt · Esc zurück{}",
                umbruch, umbruch
            ))
        )?,
        None => queue!(aus, Print(format!("{}  Auswahl: ", umbruch)))?,
    }
    Ok(())
}

/// Zeilenweiser Rückfallweg: Liste ausgeben, eine Ziffer lesen.
fn zeilenweise(kopf: &str, punkte: &[Punkt]) -> Option<char> {
    let mut aus = io::stdout();
    let _ = zeichnen(&mut aus, kopf, punkte, None);
    let _ = aus.flush();

    let mut zeile = String::new();
    match io::stdin().read_line(&mut zeile) {
        Ok(0) | Err(_) => None,
        Ok(_) => {
            let eingabe = zeile.trim();
            let c = eingabe.chars().next()?;
            punkte.iter().find(|p| p.taste == c).map(|p| p.taste)
        }
    }
}

/// Stellt eine Frage und liest eine Zeile.
///
/// Bewusst **nicht** im Rohmodus: Ein Name oder ein Pfad wird getippt,
/// korrigiert und mit Enter abgeschlossen — dafür ist die zeilenweise
/// Eingabe des Terminals mit ihrer Rücktaste und ihrem Verlauf besser als
/// alles, was hier nachgebaut würde.
pub fn frage(text: &str) -> Option<String> {
    print!("{}", text);
    let _ = io::stdout().flush();
    let mut zeile = String::new();
    match io::stdin().read_line(&mut zeile) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(zeile.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn punkte() -> Vec<Punkt> {
        vec![
            Punkt::neu('1', "Testlauf starten", "Hardware und Determinismus"),
            Punkt::neu('2', "Testdatei wählen", ""),
            Punkt::neu('0', "Beenden", ""),
        ]
    }

    /// Ohne Punkte gibt es nichts zu wählen — und kein Terminal darf in
    /// den Rohmodus geschaltet werden, um das festzustellen.
    #[test]
    fn leere_auswahl_liefert_nichts() {
        assert_eq!(waehlen("leer", &[]), None);
    }

    /// Die Höhe je Punkt trägt die Redraw-Rechnung: Stimmt sie nicht,
    /// bleiben beim Neuzeichnen Zeilen stehen oder werden überschrieben.
    #[test]
    fn hoehe_zaehlt_hinweiszeilen_mit() {
        assert_eq!(hoehe(&Punkt::neu('1', "ohne", "")), 1);
        assert_eq!(hoehe(&Punkt::neu('1', "eine", "a")), 2);
        assert_eq!(hoehe(&Punkt::neu('1', "zwei", "a\nb")), 3);
    }

    /// Die gezeichnete Liste muss jeden Punkt mit Taste und Titel
    /// enthalten — sonst ist der Zweitweg über die Ziffer unsichtbar.
    #[test]
    fn zeichnung_nennt_taste_und_titel() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Was tun?", &punkte(), None).expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(text.contains("Was tun?"));
        for p in punkte() {
            assert!(text.contains(&p.titel), "{} fehlt", p.titel);
            assert!(text.contains(&format!("{}  {}", p.taste, p.titel)));
        }
    }

    /// Im Rohmodus muss jede Zeile mit `\r` beginnen, sonst läuft die
    /// Ausgabe treppenförmig nach rechts.
    #[test]
    fn rohmodus_zeichnung_setzt_wagenruecklauf() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Kopf", &punkte(), Some(0)).expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(text.contains("\r\n"));
        assert!(
            !text.lines().any(|z| z.contains('\n')),
            "Zeilenumbruch ohne Wagenrücklauf"
        );
    }

    /// Die Höhenrechnung trägt das Neuzeichnen: Stimmt sie nicht mit der
    /// tatsächlichen Ausgabe überein, wandert das Menü bei jedem
    /// Tastendruck. Der Test bindet beide aneinander, statt sich auf
    /// zwei Stellen zu verlassen, die zusammenpassen sollen.
    #[test]
    fn hoehenrechnung_passt_zur_zeichnung() {
        for liste in [punkte(), vec![Punkt::neu('1', "einzeln", "")]] {
            let mut puffer: Vec<u8> = Vec::new();
            zeichnen(&mut puffer, "Kopf", &liste, Some(0)).expect("zeichnen");
            let text = String::from_utf8(puffer).expect("utf8");
            let gezeichnet = text.matches("\r\n").count();
            assert_eq!(
                gezeichnet,
                gezeichnete_hoehe(&liste),
                "gezeichnet {} Zeilen, gerechnet {}",
                gezeichnet,
                gezeichnete_hoehe(&liste)
            );
        }
    }

    /// Der markierte Punkt muss sich sichtbar abheben, sonst zeigen die
    /// Pfeiltasten auf nichts.
    #[test]
    fn markierter_punkt_ist_hervorgehoben() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Kopf", &punkte(), Some(1)).expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(text.contains('❯'), "Zeiger fehlt");
    }
}
