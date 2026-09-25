//! Eine Liste, aus der man mit den Pfeiltasten waehlt.
//!
//! # ⚑ Warum das nicht `auswahl::waehlen` ist
//!
//! `auswahl.rs` ist eine **wortgetreue Kopie** aus dem Testclient und
//! bleibt es. Sie kennt keinen gesperrten Eintrag, und genau den
//! braucht die Modellwahl: **„Netzwerkmodell (API), kostet
//! Inferenz-Credits" steht in der Liste und ist nicht waehlbar**
//! (Festlegung des Projektinhabers, 2026-09-11).
//!
//! ⚑ **Ein gesperrter Eintrag ist etwas anderes als ein fehlender.**
//! Wer ihn nicht sieht, fragt sich, ob es ihn gibt; wer ihn sieht und
//! nicht anwaehlen kann, weiss, dass er kommt. Deshalb steht sein Grund
//! daneben.

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};

use crate::design::Toene;

/// Ein Eintrag der Liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Punkt {
    /// Was dasteht.
    pub titel: String,
    /// Eine Zeile darunter, oder leer.
    pub hinweis: String,
    /// Ob er sich waehlen laesst.
    pub offen: bool,
}

/// **Der erste waehlbare Eintrag**, oder `None`, wenn es keinen gibt.
pub fn erster_offener(punkte: &[Punkt]) -> Option<usize> {
    punkte.iter().position(|p| p.offen)
}

/// **Der naechste waehlbare Eintrag in dieser Richtung.**
///
/// ⚑ **Gesperrte werden uebersprungen und nicht angehalten.** Ein
/// Pfeil, der auf einem Eintrag stehenbleibt, den man nicht nehmen
/// kann, fuehlt sich an wie ein Haenger.
///
/// ⚠️ **Und er laeuft nicht um.** Bei einer Liste aus drei Eintraegen,
/// von denen zwei gesperrt sind, waere ein Umlauf eine Endlosschleife;
/// hier bleibt die Stelle einfach stehen.
pub fn naechster(punkte: &[Punkt], von: usize, abwaerts: bool) -> usize {
    let mut i = von;
    loop {
        let weiter = if abwaerts { i + 1 } else { i.checked_sub(1).unwrap_or(von) };
        if abwaerts && weiter >= punkte.len() {
            return von;
        }
        if !abwaerts && i == 0 {
            return von;
        }
        i = weiter;
        if punkte[i].offen {
            return i;
        }
    }
}

/// Zeigt die Liste und gibt die Stelle des gewaehlten Eintrags zurueck;
/// der Balken steht am Anfang auf `start`.
///
/// ⚑ **Damit das Eingestellte vorgewaehlt ist.** Wer nichts aendern
/// will, drueckt Enter; wer etwas anderes will, sieht trotzdem alles.
/// Ein gesperrter oder nicht vorhandener `start` faellt auf den ersten
/// waehlbaren zurueck.
pub fn waehlen_ab(kopf: &str, punkte: &[Punkt], start: usize, t: Toene) -> Option<usize> {
    // ⛔️ **Die Liste bewegt den Wagen nach oben**, und danach sagt er
    // nichts mehr darueber, ob das Logo gerollt ist. Also darf es bis zum
    // naechsten Neudruck nicht mehr gemalt werden ([`crate::schimmer`]).
    crate::schimmer::vergessen();
    let erster = erster_offener(punkte)?;
    let mut hier = match punkte.get(start) {
        Some(p) if p.offen => start,
        _ => erster,
    };
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return zeilenweise(kopf, punkte);
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        return zeilenweise(kopf, punkte);
    };

    let hoehe = zeichnen(kopf, punkte, hier, false, t);
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Up => hier = naechster(punkte, hier, false),
            KeyCode::Down => hier = naechster(punkte, hier, true),
            KeyCode::Enter => {
                zeichnen_ab(hoehe, kopf, punkte, hier, true, t);
                return Some(hier);
            }
            KeyCode::Esc => {
                zeichnen_ab(hoehe, kopf, punkte, hier, true, t);
                return None;
            }
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                drop(_roh);
                println!();
                std::process::exit(130);
            }
            _ => continue,
        }
        zeichnen_ab(hoehe, kopf, punkte, hier, false, t);
    }
}

/// Springt um `hoehe` Zeilen zurueck und zeichnet neu.
fn zeichnen_ab(
    hoehe: usize,
    kopf: &str,
    punkte: &[Punkt],
    hier: usize,
    fertig: bool,
    t: Toene,
) -> usize {
    let mut aus = std::io::stdout();
    let _ = write!(aus, "\x1b[{hoehe}A");
    zeichnen(kopf, punkte, hier, fertig, t)
}

/// **Was in eine Zeile passt, mit `…` am Ende.**
///
/// 📌 **Ohne das rechnet die Liste falsch.** Sie springt beim
/// Neuzeichnen um ihre eigene Hoehe zurueck; eine Zeile, die das
/// Terminal umbricht, zaehlt dort als eine und belegt zwei. Der
/// Netzeintrag mit seinem langen Grund brach als Erster um, und der
/// Balken lief danach im Text.
fn passend(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    text.chars().take(breite.saturating_sub(1)).chain(['…']).collect()
}

/// **Die Breite des gezeichneten Blocks**, ueber alles, was gezeichnet
/// wird: Kopf, Titel, Hinweise und die Fusszeile.
///
/// 📌 **Bliebe eines davon draussen, stuende der Block schief**, sobald
/// gerade dieses das breiteste waere. Dieselbe Rechnung fuehrt
/// `auswahl::blockbreite` fuer den anderen Auswahlbaustein.
fn blockbreite(kopf: &str, punkte: &[Punkt], platz: usize) -> usize {
    // ⛔️ **Gemessen wird das GEZEICHNETE, nicht das Uebergebene.**
    // `passend` kuerzt jede Zeile auf `platz`; wer die ungekuerzte Laenge
    // misst, haelt den Block faelschlich fuer breiter als das Fenster,
    // und `blockeinzug` gibt dann bewusst nichts zurueck. Genau so blieb
    // die Modellwahl links stehen, waehrend die Designwahl zentriert
    // war: Ihre Hinweise (Hardware und Pfad) sind lang, die des Designs
    // kurz. Gemeldet vom Projektinhaber am 2026-09-15.
    let mut breit = passend(kopf, platz).chars().count() + 2;
    breit = breit.max(passend(FUSS, platz).chars().count() + 2);
    for p in punkte {
        breit = breit.max(passend(&p.titel, platz).chars().count() + 4);
        if !p.hinweis.is_empty() {
            breit = breit.max(passend(&p.hinweis, platz.saturating_sub(4)).chars().count() + 6);
        }
    }
    breit
}

/// Die Fusszeile, einmal: Sie geht in die Breite ein und wird gedruckt.
const FUSS: &str = "↑ ↓ bewegen · Enter waehlen · Esc abbrechen";

/// Zeichnet und gibt zurueck, wie viele Zeilen es waren.
fn zeichnen(kopf: &str, punkte: &[Punkt], hier: usize, fertig: bool, t: Toene) -> usize {
    // Sechs Zeichen Rand: der Einzug links und ein wenig Luft rechts.
    let platz = (crate::banner::fensterbreite() as usize).saturating_sub(8).max(20);
    // ⚑ **Als Block zentriert, nicht zeilenweise** (Meldung des
    // Projektinhabers, 2026-09-15: Design- und Modellwahl standen links).
    // 📌 **Dieser Baustein ist der zweite seiner Art**, und die
    // Zentrierung stand nur im ersten (`auswahl.rs`): dieselbe Sache an
    // zwei Orten, und der zweite zog nicht nach. Zeilenweise zentriert
    // verrutschten die Punkte gegeneinander, und die Liste waere keine
    // mehr; deshalb bekommt jede Zeile **denselben** Einzug.
    let einzug = crate::banner::blockeinzug(blockbreite(kopf, punkte, platz));
    let mut aus = std::io::stdout();
    let mut zeilen = 0;
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide);

    let _ = crossterm::queue!(
        aus,
        Print("\x1b[2K"),
        SetForegroundColor(t.beiwerk),
        Print(format!("{einzug}  {}\r\n\r\n", passend(kopf, platz))),
        ResetColor
    );
    zeilen += 2;

    for (i, p) in punkte.iter().enumerate() {
        let hierhin = i == hier && !fertig;
        let marke = if hierhin { "▸" } else { " " };
        let _ = crossterm::queue!(aus, Print("\x1b[2K"), Print(format!("{einzug}  {marke} ")));
        if !p.offen {
            // Gesperrt: gedaempft, und der Grund steht darunter.
            let _ = crossterm::queue!(
                aus,
                SetForegroundColor(t.kante),
                SetAttribute(Attribute::Dim),
                Print(passend(&p.titel, platz)),
                SetAttribute(Attribute::Reset),
                ResetColor
            );
        } else if hierhin {
            let _ = crossterm::queue!(
                aus,
                SetForegroundColor(t.akzent),
                SetAttribute(Attribute::Bold),
                Print(passend(&p.titel, platz)),
                SetAttribute(Attribute::Reset),
                ResetColor
            );
        } else {
            let _ = crossterm::queue!(aus, Print(passend(&p.titel, platz)));
        }
        let _ = crossterm::queue!(aus, Print("\r\n"));
        zeilen += 1;

        if !p.hinweis.is_empty() {
            let _ = crossterm::queue!(
                aus,
                Print("\x1b[2K"),
                SetForegroundColor(t.beiwerk),
                Print(format!("{einzug}      {}\r\n", passend(&p.hinweis, platz.saturating_sub(4)))),
                ResetColor
            );
            zeilen += 1;
        }
    }

    let _ = crossterm::queue!(aus, Print("\x1b[2K\r\n"));
    zeilen += 1;
    if !fertig {
        let _ = crossterm::queue!(
            aus,
            Print("\x1b[2K"),
            SetForegroundColor(t.beiwerk),
            Print(format!("{einzug}  {FUSS}\r\n")),
            ResetColor
        );
    } else {
        let _ = crossterm::queue!(aus, Print("\x1b[2K\r\n"), crossterm::cursor::Show);
    }
    zeilen += 1;
    let _ = aus.flush();
    zeilen
}

/// Ohne Terminal: die Liste hinschreiben und eine Zahl lesen.
fn zeilenweise(kopf: &str, punkte: &[Punkt]) -> Option<usize> {
    println!("  {kopf}");
    for (i, p) in punkte.iter().enumerate() {
        let sperre = if p.offen { "" } else { "  (nicht verfuegbar)" };
        println!("  {}) {}{sperre}", i + 1, p.titel);
    }
    print!("  Nummer: ");
    let _ = std::io::stdout().flush();
    let mut zeile = String::new();
    if std::io::stdin().read_line(&mut zeile).ok()? == 0 {
        return None;
    }
    let i = zeile.trim().parse::<usize>().ok()?.checked_sub(1)?;
    punkte.get(i).filter(|p| p.offen).map(|_| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn liste() -> Vec<Punkt> {
        vec![
            Punkt { titel: "Myelith 4B".into(), hinweis: String::new(), offen: true },
            Punkt { titel: "Myelith 7B".into(), hinweis: String::new(), offen: true },
            Punkt { titel: "Netzwerkmodell".into(), hinweis: "noch nicht".into(), offen: false },
        ]
    }

    /// **Der Pfeil haelt nie auf einem gesperrten Eintrag.**
    ///
    /// 📌 Die Gegenprobe zu einem Menue, das sich anfuehlt wie ein
    /// Haenger: Der Pfeil bewegt sich, und nichts passiert.
    #[test]
    fn gesperrte_eintraege_werden_uebersprungen() {
        let p = liste();
        assert_eq!(naechster(&p, 0, true), 1);
        // Unter der 1 liegt nur noch der gesperrte: Es bleibt bei 1.
        assert_eq!(naechster(&p, 1, true), 1);
        assert_eq!(naechster(&p, 1, false), 0);
        assert_eq!(naechster(&p, 0, false), 0);
    }

    /// **Und die Liste beginnt beim ersten waehlbaren.**
    #[test]
    fn der_anfang_ist_waehlbar() {
        assert_eq!(erster_offener(&liste()), Some(0));
        let nur_gesperrt =
            vec![Punkt { titel: "x".into(), hinweis: String::new(), offen: false }];
        assert_eq!(erster_offener(&nur_gesperrt), None);
    }

    /// **Und ein gesperrter Vorschlag faellt auf den ersten
    /// waehlbaren zurueck.**
    #[test]
    fn ein_gesperrter_vorschlag_wird_nicht_genommen() {
        let p = liste();
        // Der dritte ist gesperrt: Die Wahl beginnt beim ersten offenen.
        assert_eq!(
            match p.get(2) {
                Some(x) if x.offen => 2,
                _ => erster_offener(&p).unwrap(),
            },
            0
        );
    }

    /// **Steht der gesperrte vorne, faengt die Wahl dahinter an.**
    ///
    /// ⚠️ Ohne das stuende der Balken auf einem Eintrag, den Enter nicht
    /// nimmt, und das erste, was jemand erlebt, waere ein Nein.
    #[test]
    fn ein_gesperrter_anfang_wird_uebersprungen() {
        let p = vec![
            Punkt { titel: "gesperrt".into(), hinweis: String::new(), offen: false },
            Punkt { titel: "offen".into(), hinweis: String::new(), offen: true },
        ];
        assert_eq!(erster_offener(&p), Some(1));
    }

    /// **Eine Liste ohne einen einzigen waehlbaren Eintrag gibt nichts
    /// zurueck**, statt in eine Schleife zu laufen.
    #[test]
    fn ohne_waehlbaren_eintrag_gibt_es_keine_wahl() {
        let p = vec![
            Punkt { titel: "a".into(), hinweis: String::new(), offen: false },
            Punkt { titel: "b".into(), hinweis: String::new(), offen: false },
        ];
        assert_eq!(erster_offener(&p), None);
    }
}

#[cfg(test)]
mod zentrierprobe {
    use super::*;

    /// **Die Breite geht ueber alles Gezeichnete.**
    ///
    /// 📌 Gemeldet vom Projektinhaber am 2026-09-15: Design- und
    /// Modellwahl standen links, waehrend Schriftzug und der andere
    /// Auswahlbaustein zentriert waren. Dieser hier hatte die
    /// Zentrierung nie: **dieselbe Sache an zwei Orten**, und der zweite
    /// zog nicht nach.
    #[test]
    fn die_breite_zaehlt_kopf_titel_hinweis_und_fuss() {
        let punkte = vec![
            Punkt { titel: "kurz".into(), hinweis: String::new(), offen: true },
            Punkt {
                titel: "auch kurz".into(),
                // Der Hinweis ist hier das Breiteste und muss zaehlen.
                hinweis: "ein sehr viel laengerer Hinweis als alles andere".into(),
                offen: true,
            },
        ];
        let b = blockbreite("Kopf", &punkte, 200);
        assert!(
            b >= "ein sehr viel laengerer Hinweis als alles andere".chars().count() + 6,
            "der Hinweis geht nicht in die Breite ein: {b}"
        );
        // Und die Fusszeile ebenso: Sie wird gedruckt, also zaehlt sie.
        let schmal = blockbreite("k", &[], 200);
        assert!(
            schmal >= FUSS.chars().count() + 2,
            "die Fusszeile geht nicht in die Breite ein: {schmal}"
        );
    }
}
