//! Die Zeile, die jemand in den Rahmen tippt.
//!
//! # ⚑ Warum das nicht `auswahl::zeile_lesen` ist
//!
//! `auswahl.rs` ist eine **wortgetreue Kopie** aus dem Testclient und
//! bleibt es: Sie traegt die Modellauswahl mit den Pfeiltasten. Die
//! Eingabezeile hier braucht zwei Dinge, die dort nichts zu suchen
//! haben: **Umschalt-Tab als Moduswechsel** und ein Neuzeichnen der
//! Zeile im Rahmen, damit eine lange Eingabe nicht ueber die rechte
//! Kante laeuft. Eine Kopie zu erweitern hiesse, sie zu verlieren.
//!
//! ⚠️ **Was hier fehlt, fehlt mit Absicht:** keine Historie, keine
//! Pfeiltasten im Text, kein Wortsprung. Wer sie vermisst, soll sie
//! bekommen; sie jetzt zu erfinden hiesse, eine Zeilenbearbeitung zu
//! bauen, bevor jemand sie gebraucht hat.

use std::io::IsTerminal;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

/// Was am Ende einer Eingabe herauskam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eingabe {
    /// Abgeschickt.
    Zeile(String),
    /// Umschalt-Tab: der Modus soll wechseln, die Zeile bleibt stehen.
    Modus,
    /// Kein Eingabestrom mehr: Dateiende, oder eine Roehre ist zu Ende.
    ///
    /// ⚑ **Keine Taste fuehrt mehr hierher** (2026-09-15). Das ist auch
    /// der Grund, warum hier nicht nachgefragt wird: Wo der Eingabestrom
    /// endet, ist niemand, der antworten koennte.
    Ende,
    /// Escape.
    ///
    /// Escape oder Strg-X: **die beiden Tasten, die hinausfuehren.**
    ///
    /// ⚑ **Welche es war, geht mit** (Festlegung des Projektinhabers,
    /// 2026-09-15): Die Vorgabe der Rueckfrage haengt daran. Escape wird
    /// gestreift, also bleibt man per Eingabe; Strg-X ist ein bewusster
    /// Zweifingergriff, also beendet Eingabe.
    ///
    /// ⚑ **Getrennt von `Ende`** (Meldung des Projektinhabers,
    /// 2026-09-15): Escape liegt neben den Pfeiltasten und wird leicht
    /// gestreift; dass es eine Sitzung samt geladenem Modell beendet,
    /// war zu viel Wirkung fuer einen Fehlgriff. Der Aufrufer fragt
    /// nach, bevor er darauf beendet, und zwar bei beiden Tasten.
    Abbruch(Ausgang),
}

/// Welche Taste hinausfuehrte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ausgang {
    /// 📌 Hier stand bis zum 2026-09-26 auch `Escape` (Vorgabe „bleiben").
    /// Esc beendet an der leeren Zeile jetzt sofort.
    /// Zwei Finger mit Absicht, deshalb ist die Vorgabe „beenden".
    StrgX,
}

/// **Liest eine Zeile und zeichnet sie dabei selbst.**
///
/// `zeichnen` bekommt den bisherigen Text und setzt ihn in die
/// Eingabezeile, samt Wagen. **Das Zeichnen gehoert dem Aufrufer**,
/// denn nur der weiss, wo der Rahmen steht.
pub fn lesen(zeichnen: &dyn Fn(&str)) -> Eingabe {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return aus_der_roehre();
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        return aus_der_roehre();
    };
    let mut zeile = String::new();
    zeichnen(&zeile);
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        // Windows liefert Press und Release; ohne diese Pruefung zaehlt
        // jeder Tastendruck doppelt.
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Enter => return Eingabe::Zeile(zeile),
            // 📌 **LF ist auch ein Zeilenende** (Fund 308): Eingefuegter
            // mehrzeiliger Text schickt 0x0A, und das kommt als Strg-J.
            KeyCode::Char('j') if strg => return Eingabe::Zeile(zeile),
            KeyCode::BackTab => return Eingabe::Modus,
            // ⚑ **Zwei Tasten fuehren hinaus, und beide fragen nach**
            // (Festlegung des Projektinhabers, 2026-09-15): Escape und
            // Strg-X. Strg-D und Strg-C endeten hier bis dahin **ohne
            // Frage**, also vier Wege hinaus mit zwei verschiedenen
            // Verhalten; wer sie nicht alle kennt, verliert eine Sitzung
            // an die falsche Taste.
            //
            // ⚠️ **Strg-C tut an dieser Zeile jetzt nichts.** Im Rohmodus
            // erzeugt es ohnehin kein Signal, das jemand abfangen
            // koennte. Den Abbruch eines **laufenden** Auftrags macht es
            // weiterhin, aber das liegt in `anzeige.rs` und ist eine
            // andere Sache: Dort bricht es eine Erzeugung ab und beendet
            // nichts.
            // ⚑ **Esc und Strg-C beenden hier sofort** (Festlegung des
            //   Projektinhabers, 2026-09-26): An der leeren Eingabezeile
            //   laeuft nichts, und wer hier eine der beiden drueckt, will
            //   hinaus. Waehrend eines Laufs fragen dieselben Tasten nach
            //   dem Notaus (`anzeige.rs`). Strg-X fragt weiter nach.
            KeyCode::Esc => return Eingabe::Ende,
            KeyCode::Char('c') if strg => return Eingabe::Ende,
            KeyCode::Char('x') if strg => return Eingabe::Abbruch(Ausgang::StrgX),
            KeyCode::Backspace => {
                zeile.pop();
                zeichnen(&zeile);
            }
            // 📌 **Strg und ein Buchstabe ist keine Eingabe.** Strg-S
            // landete als `s` in der Zeile, weil crossterm die Taste
            // als `Char('s')` mit Zusatz meldet. **Wer eine Taste nicht
            // kennt, tippt sie nicht mit**, sondern uebergeht sie.
            KeyCode::Char(_) if strg => continue,
            KeyCode::Char(c) => {
                zeile.push(c);
                zeichnen(&zeile);
            }
            _ => {}
        }
    }
}

/// Ohne Terminal gibt es nichts zu zeichnen und nichts umzuschalten.
fn aus_der_roehre() -> Eingabe {
    let mut zeile = String::new();
    match std::io::stdin().read_line(&mut zeile) {
        Ok(0) | Err(_) => Eingabe::Ende,
        Ok(_) => Eingabe::Zeile(zeile.trim_end_matches(['\r', '\n']).to_string()),
    }
}

/// **Das Stueck der Zeile, das in den Rahmen passt.**
///
/// ⚑ **Das Ende und nicht der Anfang.** Wer tippt, sieht dort, wo der
/// Wagen steht; ein Ausschnitt vom Anfang liesse ihn blind weitertippen.
///
/// ⚠️ **Ohne das laeuft die Eingabe ueber die rechte Kante**, und weil
/// der Rahmen jetzt fest am unteren Rand steht, bricht sie in die
/// Kantenzeile um. Bis zum 2026-09-11 wanderte der Rahmen mit, und
/// dasselbe sah nur unschoen aus.
pub fn sichtbar(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    text.chars().skip(n - breite).collect()
}

/// Der Wagen steht hinter dem letzten sichtbaren Zeichen.
pub fn wagen_hinter(text: &str, breite: usize) -> usize {
    sichtbar(text, breite).chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Eine lange Zeile zeigt ihr Ende.**
    #[test]
    fn der_ausschnitt_folgt_dem_wagen() {
        assert_eq!(sichtbar("abcdef", 10), "abcdef");
        assert_eq!(sichtbar("abcdef", 3), "def");
        assert_eq!(sichtbar("", 3), "");
    }

    /// **Und der Wagen steht nie ausserhalb.**
    ///
    /// 📌 Ein Wagen hinter der rechten Kante schreibt beim naechsten
    /// Zeichen in die Kantenzeile, und der Rahmen bekommt ein Loch.
    #[test]
    fn der_wagen_bleibt_im_rahmen() {
        for n in 0..40 {
            let text = "x".repeat(n);
            assert!(wagen_hinter(&text, 10) <= 10, "{n} Zeichen schieben den Wagen hinaus");
        }
    }

    /// **Eine Tastenkombination ist kein Buchstabe.**
    ///
    /// 📌 Strg-S landete als `s` in der Eingabe. Geprueft wird die Form
    /// der Behandlung, denn die Taste selbst braucht ein Terminal.
    #[test]
    fn strg_und_buchstabe_wird_uebergangen() {
        let quelle = include_str!("eingabe.rs");
        assert!(
            quelle.contains("KeyCode::Char(_) if strg => continue,"),
            "ein Strg-Buchstabe wird nicht uebergangen"
        );
    }

    /// **Umlaute zaehlen als ein Zeichen.**
    ///
    /// ⚠️ Mit `len()` waeren „ä" zwei und der Ausschnitt jedes Mal um
    /// eine Stelle daneben.
    #[test]
    fn gezaehlt_werden_zeichen_und_nicht_bytes() {
        assert_eq!(sichtbar("äöüß", 2), "üß");
        assert_eq!(wagen_hinter("äöüß", 2), 2);
    }
}

#[cfg(test)]
mod abbruchprobe {
    /// ⚑ **An der leeren Eingabezeile beenden Esc und Strg-C sofort**,
    /// Strg-X fragt nach.
    ///
    /// 📌 Bis zum 2026-09-26 fragten Esc und Strg-X, und Strg-C tat nichts
    /// (Festlegung vom 2026-09-15, weil Esc leicht gestreift wird). Die
    /// neue Festlegung des Projektinhabers: Waehrend eines Laufs fragen
    /// Strg-C und Esc nach dem Notaus, und steht nichts mehr, fuehren
    /// beide ohne Frage hinaus.
    #[test]
    fn esc_und_strg_c_beenden_strg_x_fragt() {
        let quelle = include_str!("eingabe.rs");
        let rumpf = quelle.split("pub fn lesen(").nth(1).expect("`lesen` fehlt");
        assert!(rumpf.contains("KeyCode::Esc => return Eingabe::Ende,"), "Escape beendet nicht");
        assert!(rumpf.contains("KeyCode::Char('c') if strg => return Eingabe::Ende,"), "Strg-C beendet nicht");
        assert!(
            rumpf.contains("KeyCode::Char('x') if strg => return Eingabe::Abbruch(Ausgang::StrgX),"),
            "Strg-X fragt nicht mehr"
        );
        // ⚠️ Strg-C muss vor der Regel stehen, die Strg-Buchstaben uebergeht.
        let c = rumpf.find("KeyCode::Char('c') if strg").unwrap();
        let rest = rumpf.find("KeyCode::Char(_) if strg => continue").unwrap();
        assert!(c < rest, "Strg-C wird vorher uebergangen");
        assert!(quelle.contains("Ok(0) | Err(_) => Eingabe::Ende,"), "das Dateiende ist kein `Ende` mehr");
    }
}
