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
    /// **Die Frist ist um**, mit dem bis dahin Getippten.
    ///
    /// ⚑ Nur, solange ein Modul laeuft, das waehrend des Wartens auf eine
    /// Eingabe seine Takte bekommt. Der Text geht nicht verloren: Der
    /// Aufrufer gibt ihn beim naechsten Lesen als Anfang zurueck.
    Frist(String),
}

/// Welche Taste hinausfuehrte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ausgang {
    /// 📌 Hier stand bis zum 2026-09-26 auch `Escape` (Vorgabe „bleiben").
    /// Esc beendet an der leeren Zeile jetzt sofort.
    /// Zwei Finger mit Absicht, deshalb ist die Vorgabe „beenden".
    StrgX,
}

/// **Ein Vorschlag der Vervollstaendigung**: ein Befehl, seine weiteren
/// Schreibweisen und sein Satz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vorschlag {
    /// Wie er in der Hilfe steht, etwa `/model` oder `/tasks resume`.
    pub befehl: String,
    /// Weitere Schreibweisen (`/modell`).
    pub auch: Vec<String>,
    /// Ein Satz dazu.
    pub was: String,
    /// Ob er eine Angabe braucht (`/file <pfad>`): Enter uebernimmt ihn
    /// dann nur und wartet auf die Angabe.
    pub mit_angabe: bool,
}

/// Wer die Vorschlaege zeichnet (`None`: wegraeumen).
pub type Listenzeichner<'a> = dyn Fn(Option<(&[Vorschlag], usize)>) + 'a;

/// So viele Vorschlaege stehen hoechstens da.
pub const VORSCHLAEGE_HOECHSTENS: usize = 8;

/// **Die passenden Vorschlaege, der beste zuerst.** Nur, solange die Zeile
/// mit `/` beginnt. Rang: genau getroffen; Anfang des Befehls; Anfang einer
/// anderen Schreibweise; irgendwo im Befehl; im Satz. Bei gleichem Rang der
/// kuerzere, dann nach dem Alphabet.
///
/// ⚑ Wunsch des Projektinhabers (2026-10-07): eine Liste unter der
/// Eingabe, sobald `/` dasteht, mit den Pfeiltasten waehlbar, beim
/// Weitertippen enger, die groesste Uebereinstimmung oben.
pub fn passende(alle: &[Vorschlag], zeile: &str) -> Vec<Vorschlag> {
    let q = zeile.trim_start().to_lowercase();
    if !q.starts_with('/') {
        return Vec::new();
    }
    let ohne = q.trim_start_matches('/');
    let mut bewertet: Vec<(u8, &Vorschlag)> = alle
        .iter()
        .filter_map(|v| {
            let b = v.befehl.to_lowercase();
            let rang = if b == q {
                0
            } else if b.starts_with(&q) {
                1
            } else if v.auch.iter().any(|a| a.to_lowercase().starts_with(&q)) {
                2
            } else if !ohne.is_empty() && b.contains(ohne) {
                3
            } else if !ohne.is_empty() && v.was.to_lowercase().contains(ohne) {
                4
            } else {
                return None;
            };
            Some((rang, v))
        })
        .collect();
    bewertet.sort_by(|(ra, a), (rb, b)| ra.cmp(rb).then(a.befehl.len().cmp(&b.befehl.len())).then(a.befehl.cmp(&b.befehl)));
    bewertet.into_iter().map(|(_, v)| v.clone()).take(VORSCHLAEGE_HOECHSTENS).collect()
}

/// **Der Weckruf**: Ein Nebenfaden hat etwas gebracht, die Zeile mit Frist
/// soll vorzeitig mit [`Eingabe::Frist`] zurueckkommen.
///
/// ⚑ **Ein Schalter und keine kuerzere Frist** (2026-10-07). Jede Rueckkehr zeichnet den Rahmen neu; eine
/// Frist von einer Sekunde haette ihn jede Sekunde neu gezeichnet, auch
/// wenn nichts kam. So kehrt die Zeile nur zurueck, wenn etwas da ist.
static WECKER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// So oft sieht die Zeile mit Frist nach dem Weckruf.
const WECKSCHRITT: std::time::Duration = std::time::Duration::from_millis(200);

/// Weckt die Zeile mit Frist (aus jedem Faden).
pub fn wecken() {
    WECKER.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Ob geweckt wurde; setzt den Schalter zurueck.
pub(crate) fn geweckt() -> bool {
    WECKER.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// **Liest eine Zeile und zeichnet sie dabei selbst** (`lesen_mit_liste`).
///
/// `zeichnen` bekommt den bisherigen Text und setzt ihn in die
/// Eingabezeile, samt Wagen. **Das Zeichnen gehoert dem Aufrufer**,
/// denn nur der weiss, wo der Rahmen steht.
///
/// ⚑ **Mit einem Anfang und einer Frist** (seit 2026-10-06). Der Anfang ist, was vor einer Unterbrechung schon getippt
/// war. Ohne Frist wartet die Zeile, bis sie abgeschickt wird; mit Frist kommt
/// [`Eingabe::Frist`] zurueck, sobald sie verstrichen ist; ohne Terminal
/// gibt es keine Frist, denn eine Roehre liefert ihre Zeilen ohnehin.
///
/// ⚑ **Dazu die Vervollstaendigung** (2026-10-07): `liste` zeichnet die
/// passenden Vorschlaege mit dem gewaehlten (`Some`) oder raeumt sie weg
/// (`None`); das Zeichnen gehoert dem Aufrufer wie bei `zeichnen`.
pub fn lesen_mit_liste(
    zeichnen: &dyn Fn(&str),
    liste: &Listenzeichner,
    alle: &[Vorschlag],
    anfang: String,
    frist: Option<std::time::Instant>,
) -> Eingabe {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return aus_der_roehre();
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        return aus_der_roehre();
    };
    let mut zeile = anfang;
    // Die Liste: was gerade passt, was gewaehlt ist, ob sie zu sehen ist,
    // und ob Esc sie fuer diese Zeile weggeraeumt hat.
    let mut passend: Vec<Vorschlag> = Vec::new();
    let mut gewaehlt = 0usize;
    let mut offen = false;
    let mut weggeraeumt: Option<String> = None;
    let auffrischen = |zeile: &str, passend: &mut Vec<Vorschlag>, gewaehlt: &mut usize, offen: &mut bool, weggeraeumt: &Option<String>| {
        let neu = if weggeraeumt.as_deref() == Some(zeile) { Vec::new() } else { passende(alle, zeile) };
        // Steht genau ein Vorschlag da und ist schon getippt, braucht es keine Liste.
        let noetig = !neu.is_empty() && !(neu.len() == 1 && neu[0].befehl == zeile.trim());
        if neu != *passend {
            *gewaehlt = 0;
        }
        *passend = neu;
        if noetig {
            liste(Some((passend.as_slice(), *gewaehlt)));
            *offen = true;
        } else if *offen {
            liste(None);
            *offen = false;
        }
        zeichnen(zeile);
    };
    zeichnen(&zeile);
    auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
    // Was immer herausgeht, die Liste geht vorher weg.
    let hinaus = |e: Eingabe, offen: bool| {
        if offen {
            liste(None);
        }
        e
    };
    loop {
        if let Some(bis) = frist {
            let jetzt = std::time::Instant::now();
            if jetzt >= bis || geweckt() {
                return hinaus(Eingabe::Frist(zeile), offen);
            }
            if !event::poll((bis - jetzt).min(WECKSCHRITT)).unwrap_or(false) {
                continue;
            }
        }
        let Ok(Event::Key(k)) = event::read() else { continue };
        // Windows liefert Press und Release; ohne diese Pruefung zaehlt
        // jeder Tastendruck doppelt.
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        // ⚑ **Mit offener Liste** gehoeren Pfeile, Tab, Enter und Esc ihr.
        //   Esc raeumt nur die Liste weg: An der Zeile beendet Esc sonst die
        //   Konsole, und mit offener Liste waere das eine Falle.
        if offen && !passend.is_empty() {
            match k.code {
                KeyCode::Up => {
                    gewaehlt = (gewaehlt + passend.len() - 1) % passend.len();
                    liste(Some((passend.as_slice(), gewaehlt)));
                    zeichnen(&zeile);
                    continue;
                }
                KeyCode::Down => {
                    gewaehlt = (gewaehlt + 1) % passend.len();
                    liste(Some((passend.as_slice(), gewaehlt)));
                    zeichnen(&zeile);
                    continue;
                }
                KeyCode::Tab => {
                    let v = &passend[gewaehlt];
                    zeile = if v.mit_angabe { format!("{} ", v.befehl) } else { v.befehl.clone() };
                    auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
                    continue;
                }
                KeyCode::Enter => {
                    let v = passend[gewaehlt].clone();
                    if v.mit_angabe && zeile.trim() != v.befehl {
                        zeile = format!("{} ", v.befehl);
                        auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
                        continue;
                    }
                    return hinaus(Eingabe::Zeile(v.befehl), offen);
                }
                KeyCode::Esc => {
                    weggeraeumt = Some(zeile.clone());
                    auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
                    continue;
                }
                // Strg-C und Strg-X fuehren hinaus: vorher die Liste weg.
                KeyCode::Char('c') | KeyCode::Char('x') if strg => {
                    liste(None);
                    offen = false;
                }
                _ => {}
            }
        }
        match k.code {
            KeyCode::Enter => return hinaus(Eingabe::Zeile(zeile), offen),
            // 📌 **LF ist auch ein Zeilenende** (Fund 308): Eingefuegter
            // mehrzeiliger Text schickt 0x0A, und das kommt als Strg-J.
            KeyCode::Char('j') if strg => return hinaus(Eingabe::Zeile(zeile), offen),
            KeyCode::BackTab => return hinaus(Eingabe::Modus, offen),
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
                auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
            }
            // 📌 **Strg und ein Buchstabe ist keine Eingabe.** Strg-S
            // landete als `s` in der Zeile, weil crossterm die Taste
            // als `Char('s')` mit Zusatz meldet. **Wer eine Taste nicht
            // kennt, tippt sie nicht mit**, sondern uebergeht sie.
            KeyCode::Char(_) if strg => continue,
            KeyCode::Char(c) => {
                zeile.push(c);
                auffrischen(&zeile, &mut passend, &mut gewaehlt, &mut offen, &weggeraeumt);
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

    fn v(befehl: &str, auch: &[&str], was: &str) -> Vorschlag {
        Vorschlag { befehl: befehl.into(), auch: auch.iter().map(|s| s.to_string()).collect(), was: was.into(), mit_angabe: false }
    }

    /// **Die beste Uebereinstimmung oben, beim Weitertippen enger.**
    #[test]
    fn vorschlaege_nach_rang() {
        let alle = vec![
            v("/model", &["/modell"], "ein Modell waehlen"),
            v("/beispiel", &[], "ein Modul"),
            v("/beispiel probe", &[], "mit dem Probekonto"),
            v("/beispiel status", &[], "Uebersicht"),
            v("/help", &["/hilfe"], "alle Befehle"),
            v("/tasks", &[], "die Vorhaben"),
        ];
        let namen = |z: &str| passende(&alle, z).into_iter().map(|v| v.befehl).collect::<Vec<_>>();
        assert_eq!(namen("/").len(), 6, "alle bei einem Schraegstrich");
        assert_eq!(namen("/bei"), vec!["/beispiel", "/beispiel probe", "/beispiel status"], "Anfang, der kuerzere zuerst");
        assert_eq!(namen("/beispiel p"), vec!["/beispiel probe"]);
        assert_eq!(namen("/hil"), vec!["/help"], "andere Schreibweise");
        assert_eq!(namen("/probe")[0], "/beispiel probe", "irgendwo im Befehl");
        assert_eq!(namen("/vorhaben"), vec!["/tasks"], "im Satz");
        assert!(namen("hallo").is_empty(), "ohne Schraegstrich keine Liste");
        assert!(namen("/xyz").is_empty());
    }

    /// **Esc mit offener Liste raeumt nur die Liste weg**, und dieser Fall
    /// steht vor dem Esc, das die Konsole beendet.
    #[test]
    fn esc_raeumt_erst_die_liste() {
        let quelle = include_str!("eingabe.rs");
        let rumpf = quelle.split("pub fn lesen_mit_liste(").nth(1).expect("`lesen_mit_liste` fehlt");
        let liste = rumpf.find("weggeraeumt = Some(zeile.clone());").expect("Esc der Liste fehlt");
        let ende = rumpf.find("KeyCode::Esc => return Eingabe::Ende,").expect("Esc der Zeile fehlt");
        assert!(liste < ende);
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
        let rumpf = quelle.split("pub fn lesen_mit_liste(").nth(1).expect("`lesen_mit_liste` fehlt");
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
