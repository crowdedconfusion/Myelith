//! Menüauswahl mit Pfeiltasten.
//!
//! Ein Menüpunkt wird mit ↑/↓ angesteuert und mit Enter bestätigt. Die
//! Ziffer bleibt daneben gültig: Wer den Punkt kennt, tippt sie und ist
//! sofort dort, ohne sich durch die Liste zu bewegen.
//!
//! ## Warum es trotzdem noch einen zweiten Weg gibt
//!
//! Pfeiltasten brauchen den **Rohmodus** des Terminals, die Eingabe wird
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
//! an und reagiert nicht auf Strg-C, der Nutzer sieht dann nicht den
//! Fehler, sondern eine kaputte Shell. Deshalb liegt die Rücknahme in
//! [`Rohmodus`], einem Wächter mit `Drop`: Sie läuft bei normaler
//! Rückkehr, bei vorzeitigem `return` und beim Abwickeln nach einer
//! Panik. Strg-C wird zusätzlich selbst behandelt, weil das Signal im
//! Rohmodus nicht mehr vom Terminal erzeugt wird.

use std::io::{self, IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};

/// Ein Menüpunkt.
pub struct Punkt {
    /// Ziffer oder Buchstabe, der Zweitweg und zugleich der Rückgabewert.
    ///
    /// Der Aufrufer verzweigt weiterhin über dieses Zeichen. Damit ändert
    /// die Umstellung auf Pfeiltasten die Menülogik nicht, sondern nur die
    /// Art, wie das Zeichen zustande kommt.
    pub taste: char,
    pub titel: String,
    /// Erläuterung unter dem Titel. Leer lassen, wenn der Titel genügt.
    pub hinweis: String,
    /// Farbe des Titels: einer der beiden Töne, die dem Schriftzug im
    /// Spektrum am nächsten liegen (siehe [`crate::farben::schlagwort`]).
    ///
    /// **Beim Bau des Punkts vergeben, nicht beim Zeichnen.** Die Liste
    /// wird bei jedem Tastendruck neu gezeichnet; eine beim Zeichnen
    /// bestimmte Farbe flackerte, sobald jemand die Pfeiltaste hält. So
    /// bleibt sie über eine Auswahl hinweg stehen und wechselt erst, wenn
    /// das Menü erneut aufgebaut wird.
    pub farbe: Color,
    /// Eine Leerzeile **vor** diesem Punkt.
    ///
    /// Gruppiert eine Liste, ohne sie zu zerteilen: Der Abstand hängt am
    /// Punkt selbst, statt als eigener, nicht wählbarer Eintrag in der
    /// Liste zu stehen. Ein solcher Platzhalter müsste in der
    /// Pfeilnavigation übersprungen werden, in der Ziffernwahl ignoriert
    /// und in der Höhenrechnung mitgezählt: drei Stellen, an denen sich
    /// ein Fehler versteckt. So ist es eine Zeile mehr beim Zeichnen und
    /// sonst nichts.
    pub abstand_davor: bool,
}

impl Punkt {
    pub fn neu(taste: char, titel: &str, hinweis: &str) -> Self {
        Self {
            taste,
            titel: titel.to_string(),
            hinweis: hinweis.to_string(),
            farbe: crate::farben::schlagwort(),
            abstand_davor: false,
        }
    }

    /// Setzt diesen Punkt mit einer Leerzeile von den vorigen ab.
    ///
    /// Für die Stelle, an der eine Liste von den eigentlichen Schritten zu
    /// den Nebenfunktionen übergeht. Wer das Menü überfliegt, soll die
    /// vier Schritte als Gruppe sehen und nicht sieben gleichrangige
    /// Punkte.
    pub fn abgesetzt(mut self) -> Self {
        self.abstand_davor = true;
        self
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
/// `None` heißt abgebrochen (Esc oder Eingabeende), der Aufrufer
/// behandelt das wie „zurück".
pub fn waehlen(kopf: &str, punkte: &[Punkt]) -> Option<char> {
    waehlen_mit_fuss(kopf, punkte, "")
}

/// Wie [`waehlen`], mit einem Textblock **unter** der Liste.
///
/// **Warum der Fuß hier durchgereicht wird und nicht einfach vorher
/// gedruckt.** Die Liste zeichnet sich bei jedem Tastendruck neu, indem
/// sie um ihre eigene Höhe nach oben springt und von dort abwärts löscht.
/// Alles, was unter ihr steht, läge in diesem Bereich und verschwände
/// beim ersten Pfeildruck. Der Fuß muss deshalb Teil der gezeichneten
/// Liste sein und in ihre Höhe eingehen.
///
/// Gebraucht für die aktuellen Einstellungen: Sie gehören unter das Menü,
/// weil zuerst die Frage kommt, was man tun will, und erst danach der
/// Zustand, unter dem es geschieht.
pub fn waehlen_mit_fuss(kopf: &str, punkte: &[Punkt], fuss: &str) -> Option<char> {
    if punkte.is_empty() {
        return None;
    }
    // Kein Terminal (Pipe, Datei, Test): zeilenweise lesen.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return zeilenweise(kopf, punkte, fuss);
    }
    // Rohmodus nicht verfügbar (etwa in einer Umgebung ohne Terminfo):
    // ebenfalls zurückfallen, statt den Nutzer vor einem toten Menü
    // sitzen zu lassen.
    match interaktiv(kopf, punkte, fuss) {
        Ok(wahl) => wahl,
        Err(_) => zeilenweise(kopf, punkte, fuss),
    }
}

/// Die Breite des gezeichneten Blocks, gemessen an seiner breitesten
/// Zeile.
///
/// Sie bestimmt, wie weit der Block eingerückt wird, damit er mittig unter
/// dem Schriftzug steht. Gerechnet wird über **alles**, was gezeichnet
/// wird: Kopf, Punkte, Hinweise, Fußzeile und der Einstellungsblock. Bliebe
/// eines davon draußen, stünde der Block schief, sobald gerade dieses das
/// breiteste wäre.
fn blockbreite(kopf: &str, punkte: &[Punkt], fuss: &str) -> usize {
    let mut breit = kopf.chars().count() + 8;
    breit = breit.max("  ↑ ↓ bewegen · Enter wählen · Ziffer direkt · Esc zurück".chars().count());
    for p in punkte {
        breit = breit.max(p.titel.chars().count() + 8);
        for z in p.hinweis.lines() {
            breit = breit.max(z.chars().count() + 8);
        }
    }
    for z in fuss.lines() {
        breit = breit.max(z.chars().count());
    }
    breit
}

/// Wie viele Zeilen ein Punkt belegt.
///
/// Muss die Leerzeile aus [`Punkt::abgesetzt`] mitzählen: Sonst bleiben
/// beim Neuzeichnen Reste stehen, oder der Cursor frisst die Zeile
/// darüber.
fn hoehe(p: &Punkt) -> usize {
    usize::from(p.abstand_davor)
        + 1
        + if p.hinweis.is_empty() {
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
fn gezeichnete_hoehe(punkte: &[Punkt], fuss: &str) -> usize {
    2 + punkte.iter().map(hoehe).sum::<usize>() + 2 + fusshoehe(fuss)
}

/// Zeilen, die der Fuß belegt. Leerer Fuß belegt keine.
fn fusshoehe(fuss: &str) -> usize {
    if fuss.is_empty() {
        0
    } else {
        fuss.lines().count() + 1
    }
}

fn interaktiv(kopf: &str, punkte: &[Punkt], fuss: &str) -> io::Result<Option<char>> {
    let zeilen = gezeichnete_hoehe(punkte, fuss);

    // Passt die Liste nicht ins Fenster, scrollt das Terminal beim
    // Zeichnen, und `MoveToPreviousLine` landet dann auf einer anderen
    // Zeile als der, an der die Liste begann. Das Ergebnis wäre ein Menü,
    // das sich bei jedem Tastendruck selbst zerlegt. In dem Fall ist die
    // zeilenweise Ausgabe nicht der schlechtere Weg, sondern der einzige,
    // der funktioniert.
    // Unbekannte Fenstergröße wird wie „zu klein" behandelt: Ohne die Höhe
    // ist nicht entscheidbar, ob das Neuzeichnen trägt.
    let passt = terminal::size().is_ok_and(|(_, h)| (h as usize) > zeilen);
    if !passt {
        return Ok(zeilenweise(kopf, punkte, fuss));
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
        zeichnen(&mut aus, kopf, punkte, Some(markiert), fuss)?;
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
    fuss: &str,
) -> io::Result<()> {
    // Im Rohmodus setzt `\n` den Cursor nach unten, ohne ihn an den
    // Zeilenanfang zu holen, ohne `\r` liefe die Ausgabe treppenförmig.
    //
    // Dieselbe Unterscheidung entscheidet über die Farbe: `markiert` ist
    // nur im Rohmodus gesetzt, also nur dann, wenn ein Terminal am anderen
    // Ende hängt. Der zeilenweise Rückfallweg bleibt farblos: er bedient
    // auch Pipes und Mitschnitte, und Steuerzeichen wären dort Müll.
    let farbig = markiert.is_some();
    let umbruch = if farbig { "\r\n" } else { "\n" };

    // Die Liste steht mittig unter dem Schriftzug, aber **als Block**:
    // Alle Zeilen bekommen denselben Einzug, ihre Ausrichtung
    // untereinander bleibt erhalten. Zeilenweise zentriert verrutschten
    // die Punkte gegeneinander, und die Liste wäre keine mehr.
    //
    // Nur im Rohmodus, also nur mit Terminal: In einer Pipe wären
    // führende Leerzeichen Ballast.
    let einzug = if farbig {
        crate::banner::blockeinzug(blockbreite(kopf, punkte, fuss))
    } else {
        String::new()
    };

    queue!(
        aus,
        Print(format!("{}{}  ── {} ──{}", umbruch, einzug, kopf, umbruch))
    )?;

    for (i, p) in punkte.iter().enumerate() {
        if p.abstand_davor {
            queue!(aus, Print(umbruch))?;
        }
        let ist_markiert = markiert == Some(i);
        let zeiger = if ist_markiert { "❯" } else { " " };

        queue!(aus, Print(format!("{}  {} {}  ", einzug, zeiger, p.taste)))?;
        if farbig {
            queue!(aus, SetForegroundColor(p.farbe), SetAttribute(Attribute::Bold))?;
            if ist_markiert {
                queue!(aus, SetAttribute(Attribute::Underlined))?;
            }
        }
        queue!(aus, Print(&p.titel))?;
        if farbig {
            queue!(aus, ResetColor, SetAttribute(Attribute::Reset))?;
        }
        queue!(aus, Print(umbruch))?;

        for zeile in p.hinweis.lines() {
            if farbig {
                queue!(aus, SetForegroundColor(crate::farben::BEIWERK))?;
            }
            queue!(aus, Print(format!("{}        {}", einzug, zeile)))?;
            if farbig {
                queue!(aus, ResetColor)?;
            }
            queue!(aus, Print(umbruch))?;
        }
    }

    match markiert {
        Some(_) => {
            queue!(aus, SetForegroundColor(crate::farben::BEIWERK), Print(umbruch))?;
            queue!(
                aus,
                Print(format!(
                    "{}  ↑ ↓ bewegen · Enter wählen · Ziffer direkt · Esc zurück",
                    einzug
                )),
                ResetColor,
                Print(umbruch)
            )?
        }
        None => queue!(aus, Print(format!("{}  Auswahl: ", umbruch)))?,
    }

    // Der Fuß zuletzt, gedämpft: Er ist Zustand, keine Aufforderung, und
    // soll die Auswahl darüber nicht überstrahlen.
    if !fuss.is_empty() {
        queue!(aus, Print(umbruch))?;
        for zeile in fuss.lines() {
            if farbig {
                queue!(aus, SetForegroundColor(crate::farben::BEIWERK))?;
            }
            queue!(aus, Print(format!("{}{}", einzug, zeile)))?;
            if farbig {
                queue!(aus, ResetColor)?;
            }
            queue!(aus, Print(umbruch))?;
        }
    }
    Ok(())
}

/// Zeilenweiser Rückfallweg: Liste ausgeben, eine Ziffer lesen.
fn zeilenweise(kopf: &str, punkte: &[Punkt], fuss: &str) -> Option<char> {
    let mut aus = io::stdout();
    let _ = zeichnen(&mut aus, kopf, punkte, None, fuss);
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
/// korrigiert und mit Enter abgeschlossen: dafür ist die zeilenweise
/// Eingabe des Terminals mit ihrer Rücktaste und ihrem Verlauf besser als
/// alles, was hier nachgebaut würde.
/// Liest eine Zeile und unterscheidet dabei **Enter von Escape**.
///
/// `Some(text)` bei Enter, `None` bei Escape, Strg-D und Eingabeende.
///
/// **Warum nicht [`frage`].** Das liest über `read_line` im gekochten
/// Modus, und dort kommt Escape nicht als Taste an, sondern als Zeichen in
/// der Zeile. Ein Abbruch war deshalb nur über eine leere Eingabe zu
/// haben, also über dieselbe Taste, die man drückt, um zu sehen, ob das
/// Programm noch antwortet. Wer im Gespräch mit dem Modell einmal Enter
/// tippte, um sich zu vergewissern, stand danach im Menü.
///
/// Deshalb eine eigene Zeile im Rohmodus: Sie sieht jede Taste einzeln.
/// **Eine leere Eingabe tut hier nichts**, sie fragt neu.
///
/// Ohne Terminal fällt sie auf [`frage`] zurück; ein Skript hat keine
/// Escape-Taste, und dort bleibt das Eingabeende der Abbruch.
pub fn zeile_lesen(text: &str) -> Option<String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return frage(text);
    }
    match roh_lesen(text) {
        Ok(zeile) => zeile,
        // Kein Rohmodus verfügbar: lieber die schlechtere Eingabe als gar
        // keine.
        Err(_) => frage(text),
    }
}

fn roh_lesen(text: &str) -> io::Result<Option<String>> {
    let _roh = Rohmodus::an()?;
    let mut aus = io::stdout();
    queue!(aus, Print(text), cursor::Show)?;
    aus.flush()?;

    let mut zeile = String::new();
    loop {
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
            KeyCode::Esc => {
                queue!(aus, Print("\r\n"))?;
                aus.flush()?;
                return Ok(None);
            }
            KeyCode::Enter => {
                queue!(aus, Print("\r\n"))?;
                aus.flush()?;
                return Ok(Some(zeile));
            }
            KeyCode::Backspace => {
                if zeile.pop().is_some() {
                    // Ein Zeichen zurück, überschreiben, wieder zurück.
                    queue!(aus, Print("\u{8} \u{8}"))?;
                    aus.flush()?;
                }
            }
            // Strg-D ist in jeder Kommandozeile „fertig" und bleibt es.
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                queue!(aus, Print("\r\n"))?;
                aus.flush()?;
                return Ok(None);
            }
            // Strg-C erzeugt im Rohmodus kein Signal mehr. Ohne eigene
            // Behandlung wäre die Eingabe nicht mehr zu verlassen.
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                drop(_roh);
                println!();
                std::process::exit(130);
            }
            KeyCode::Char(c) => {
                zeile.push(c);
                queue!(aus, Print(c))?;
                aus.flush()?;
            }
            _ => {}
        }
    }
}

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

    /// Ohne Punkte gibt es nichts zu wählen, und kein Terminal darf in
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
    /// enthalten: sonst ist der Zweitweg über die Ziffer unsichtbar.
    #[test]
    fn zeichnung_nennt_taste_und_titel() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Was tun?", &punkte(), None, "").expect("zeichnen");
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
        zeichnen(&mut puffer, "Kopf", &punkte(), Some(0), "").expect("zeichnen");
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
            zeichnen(&mut puffer, "Kopf", &liste, Some(0), "").expect("zeichnen");
            let text = String::from_utf8(puffer).expect("utf8");
            let gezeichnet = text.matches("\r\n").count();
            assert_eq!(
                gezeichnet,
                gezeichnete_hoehe(&liste, ""),
                "gezeichnet {} Zeilen, gerechnet {}",
                gezeichnet,
                gezeichnete_hoehe(&liste, "")
            );
        }
    }

    /// Der Block wird **als Ganzes** eingerückt: Alle Zeilen bekommen
    /// denselben Einzug, ihre Ausrichtung untereinander bleibt erhalten.
    /// Zeilenweise zentriert verrutschten die Punkte gegeneinander, und
    /// die Liste wäre keine mehr.
    #[test]
    fn block_bleibt_untereinander_ausgerichtet() {
        let p = punkte();
        // Die Breite richtet sich nach der breitesten Zeile, und zwar über
        // Kopf, Punkte, Hinweise und Fuß hinweg.
        let schmal = blockbreite("Kopf", &p, "");
        let breit = blockbreite("Kopf", &p, &"x".repeat(200));
        assert!(breit > schmal, "der Fuß geht nicht in die Breite ein");
        assert!(
            blockbreite(&"K".repeat(200), &p, "") > schmal,
            "der Kopf geht nicht in die Breite ein"
        );

        // Ein Block, der breiter ist als das Fenster, wird nicht negativ
        // eingerückt, sondern gar nicht.
        assert!(crate::banner::blockeinzug(100_000).is_empty());
    }

    /// Der Fuß steht unter der Liste und geht in ihre Höhe ein. Ginge er
    /// nicht ein, spränge das Neuzeichnen beim ersten Pfeildruck auf eine
    /// falsche Zeile und zerlegte die Liste.
    #[test]
    fn fuss_steht_unter_der_liste_und_zaehlt_mit() {
        let p = punkte();
        let fuss = "  Einstellungen:\n    Token 32\n    Shards 4";

        let mut mit: Vec<u8> = Vec::new();
        zeichnen(&mut mit, "Kopf", &p, Some(0), fuss).expect("zeichnen");
        let text = String::from_utf8(mit).expect("utf8");
        let liste = text.find("Testlauf starten").expect("Liste fehlt");
        let block = text.find("Einstellungen:").expect("Fuß fehlt");
        assert!(block > liste, "der Fuß steht über der Liste");

        assert_eq!(
            gezeichnete_hoehe(&p, fuss),
            gezeichnete_hoehe(&p, "") + fuss.lines().count() + 1,
            "der Fuß geht nicht in die Höhe ein"
        );
        assert_eq!(fusshoehe(""), 0, "ein leerer Fuß belegt Zeilen");
    }

    /// Zwei Punkte auf derselben Taste wären ein Fehler, der erst auf einer
    /// fremden Maschine mit vielen Einträgen auffiele: Die Ziffer springt
    /// dann zum falschen Punkt.
    #[test]
    fn tastenkuerzel_sind_eindeutig() {
        let p = punkte();
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                assert_ne!(a.taste, b.taste, "Taste {:?} zweimal vergeben", a.taste);
            }
        }
    }

    /// Im Rohmodus tragen die Titel Farbe und Fettschrift; der markierte
    /// zusätzlich eine Unterstreichung. Ohne beides zeigten die
    /// Pfeiltasten auf nichts.
    #[test]
    fn titel_sind_farbig_und_fett() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Kopf", &punkte(), Some(1), "").expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(text.contains("\x1b[38;5;"), "keine Farbe gesetzt");
        assert!(text.contains("\x1b[1m"), "keine Fettschrift");
        assert!(text.contains("\x1b[4m"), "markierter Punkt nicht unterstrichen");
        assert!(text.contains("\x1b[0m"), "Farbe wird nicht zurückgenommen");
    }

    /// Der zeilenweise Rückfallweg bedient auch Pipes und Mitschnitte.
    /// Dort wären Steuerzeichen Müll: er bleibt deshalb farblos.
    #[test]
    fn zeilenmodus_bleibt_ohne_steuerzeichen() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Kopf", &punkte(), None, "").expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(!text.contains('\x1b'), "Steuerzeichen im Zeilenmodus: {text:?}");
    }

    /// Der markierte Punkt muss sich sichtbar abheben, sonst zeigen die
    /// Pfeiltasten auf nichts.
    #[test]
    fn markierter_punkt_ist_hervorgehoben() {
        let mut puffer: Vec<u8> = Vec::new();
        zeichnen(&mut puffer, "Kopf", &punkte(), Some(1), "").expect("zeichnen");
        let text = String::from_utf8(puffer).expect("utf8");
        assert!(text.contains('❯'), "Zeiger fehlt");
    }
}
