//! Welche Farben dieses Programm benutzt.
//!
//! # ⚑ Fuenf Bilder, und eines davon ist keines
//!
//! `Standard` setzt **keine** eigene Farbe: Wer sein Terminal
//! eingerichtet hat, hat damit schon gewaehlt, und ein Programm, das
//! sich darueberlegt, nimmt ihm die Wahl wieder weg. Die anderen vier
//! setzen drei Toene, mehr nicht: eine Kante, ein Beiwerk, einen
//! Akzent.
//!
//! ⚑ **Drei Toene und nicht dreissig.** Ein Bild aus wenigen Farben
//! bleibt lesbar, wenn jemand einen hellen Hintergrund hat; eines aus
//! vielen sieht auf genau einem Schirm gut aus, naemlich dem, auf dem
//! es gebaut wurde.

use std::sync::OnceLock;

use crossterm::style::Color;

use myl_client::einstellungen::Konsolendesign;

/// Die drei Toene eines Designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toene {
    /// Rahmen und Linien.
    pub kante: Color,
    /// Fusszeilen, Hinweise, alles Nebensaechliche.
    pub beiwerk: Color,
    /// Was hervorgehoben wird.
    pub akzent: Color,
}

/// Die Toene dieses Designs.
///
/// ⚑ **`Color::Reset` ist die Vordergrundfarbe des Terminals**, nicht
/// Weiss. Genau das meint `Standard`.
pub fn toene(d: Konsolendesign) -> Toene {
    match d {
        Konsolendesign::Standard => Toene {
            // ⚑ Auch im Standard wird das Beiwerk gedaempft, aber ueber
            // die **ANSI-Graustufe** und nicht ueber einen eigenen
            // Farbwert: Sie folgt dem Schema des Terminals.
            kante: Color::AnsiValue(248),
            beiwerk: Color::AnsiValue(244),
            akzent: Color::Reset,
        },
        Konsolendesign::Myelith => Toene {
            kante: Color::AnsiValue(248),
            beiwerk: Color::AnsiValue(244),
            akzent: crate::farben::schlagwort(),
        },
        Konsolendesign::Bernstein => Toene {
            kante: Color::Rgb { r: 138, g: 86, b: 0 },
            beiwerk: Color::Rgb { r: 178, g: 116, b: 0 },
            akzent: Color::Rgb { r: 255, g: 176, b: 0 },
        },
        Konsolendesign::Tiefsee => Toene {
            kante: Color::Rgb { r: 42, g: 78, b: 120 },
            beiwerk: Color::Rgb { r: 96, g: 132, b: 168 },
            akzent: Color::Rgb { r: 92, g: 224, b: 232 },
        },
        Konsolendesign::Tinte => Toene {
            kante: Color::Rgb { r: 96, g: 96, b: 96 },
            beiwerk: Color::Rgb { r: 140, g: 140, b: 140 },
            akzent: Color::Rgb { r: 236, g: 236, b: 236 },
        },
    }
}

/// **Wie ein Stueck Text aussieht**: Farbe und drei Schalter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stil {
    pub farbe: Color,
    pub fett: bool,
    pub kursiv: bool,
    pub unterstrichen: bool,
    /// Ein Hintergrund, falls der Text unterlegt wird.
    pub hintergrund: Option<Color>,
}

impl Stil {
    const fn schlicht(farbe: Color) -> Self {
        Self { farbe, fett: false, kursiv: false, unterstrichen: false, hintergrund: None }
    }
    const fn fett(farbe: Color) -> Self {
        Self { farbe, fett: true, kursiv: false, unterstrichen: false, hintergrund: None }
    }
    const fn kursiv(farbe: Color) -> Self {
        Self { farbe, fett: false, kursiv: true, unterstrichen: false, hintergrund: None }
    }
    const fn betont(farbe: Color) -> Self {
        Self { farbe, fett: true, kursiv: false, unterstrichen: true, hintergrund: None }
    }
    /// Fett, auf einem getoenten Grund: die Eingabe des Menschen.
    const fn unterlegt(farbe: Color, grund: Color) -> Self {
        Self { farbe, fett: true, kursiv: false, unterstrichen: false, hintergrund: Some(grund) }
    }

    /// **Setzt einen Text in diesem Stil**, samt vollstaendigem Ruecksetzen
    /// danach. Ohne `farbig` kommt er unveraendert zurueck.
    ///
    /// ⚑ **Jedes Stueck setzt sich selbst zurueck.** Ein Stil, der ueber
    /// seinen Text hinausreicht, faerbt die naechste Zeile, und in einem
    /// Terminal bleibt er stehen, bis jemand ihn zuruecknimmt.
    pub fn faerben(&self, text: &str, farbig: bool) -> String {
        use crossterm::style::{Attribute, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor};
        if !farbig || text.is_empty() {
            return text.to_string();
        }
        let mut s = format!("{}", SetForegroundColor(self.farbe));
        if let Some(grund) = self.hintergrund {
            s.push_str(&format!("{}", SetBackgroundColor(grund)));
        }
        for (an, merkmal) in
            [(self.fett, Attribute::Bold), (self.kursiv, Attribute::Italic), (self.unterstrichen, Attribute::Underlined)]
        {
            if an {
                s.push_str(&format!("{}", SetAttribute(merkmal)));
            }
        }
        format!("{s}{text}{ResetColor}{}", SetAttribute(Attribute::Reset))
    }
}

/// **Der Ton, zu dem der Grund der Eingabe hin getoent wird**, fuer
/// Standard und Myelith; wie stark, sagt [`Rollen::deckung`].
///
/// ⚑ **Ein Hauch und nicht mehr** (Rueckmeldung des Projektinhabers,
/// 2026-09-24: das Palettengrau 236 war „viel zu intensiv"). Gemischt
/// wird ueber den Hintergrund des Terminals ([`terminalgrund`]), in
/// einem Ton ueber das ganze Kaestchen (`antwort::eingabe_kasten`).
const EINGABETON: Color = Color::Rgb { r: 150, g: 150, b: 200 };

/// **Die Rollen der Textausgabe**, je Design.
///
/// ⚑ **Farbe unterstreicht, sie traegt nichts allein** (dieselbe Regel wie
/// beim Logo): Ein Aufruf steht mit `→` da, eine Rueckgabe mit `←`, eine
/// Ablehnung mit `⚑` und dem Wort. Wer keine Farbe sieht, verliert keine
/// Auskunft, nur die Fuehrung des Blicks.
///
/// ⚑ **Was hervorgehoben wird, ist, was der Agent tut** (Auftrag des
/// Projektinhabers, 2026-09-24): Aufrufe am staerksten, Rueckgaben in einer
/// zweiten Farbe, Warnungen unuebersehbar; Zeiten, Zaehler und Rohstrom
/// treten zurueck; in der Antwort Ueberschriften, Code und Aufzaehlungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rollen {
    /// Die Eingabe des Menschen, in der Zeitleiste auf getoentem Grund.
    /// Ihr `hintergrund` ist der Ton, zu dem hin getoent wird.
    pub eingabe: Stil,
    /// Wie stark der Grund der Eingabe zu diesem Ton hin gemischt wird,
    /// in Promille.
    pub deckung: u32,
    /// Ein Werkzeugaufruf: Pfeil und Name.
    pub aktion: Stil,
    /// Eine Rueckgabe: Pfeil und Name.
    pub ergebnis: Stil,
    /// Ablehnung, voller Kontext, keine Antwort.
    pub warnung: Stil,
    /// Code in der Antwort.
    pub code: Stil,
    /// Ueberschriften in der Antwort.
    pub ueberschrift: Stil,
    /// Listenpunkte in der Antwort und Schrittmarken in der Zeitleiste.
    pub marke: Stil,
    /// Das Denken des Modells im Rohstrom.
    pub gedanke: Stil,
    /// Zeiten, Zaehler, der Rohstrom, der Inhalt einer Rueckgabe.
    pub beiwerk: Stil,
    /// Balken und Linien.
    pub kante: Stil,
}

/// Die Rollen dieses Designs.
///
/// | Design | Wie unterschieden wird |
/// |---|---|
/// | Standard | die benannten ANSI-Farben, also die Palette des Terminals |
/// | Myelith | die Neonfarben der Sitzung |
/// | Bernstein, Tinte | einfarbig: Helle, fett, kursiv, unterstrichen |
/// | Tiefsee | Cyan, Tuerkis, Hellblau, und ein warmer Ton fuer Warnungen |
///
/// ⚑ **Standard nimmt benannte Farben und keine Farbwerte**: Sie kommen
/// aus dem Schema des Terminals, und wer es eingerichtet hat, sieht seine
/// eigenen Toene.
pub fn rollen(d: Konsolendesign) -> Rollen {
    let grau = |w: u8| Color::Rgb { r: w, g: w, b: w };
    match d {
        Konsolendesign::Standard => Rollen {
            // ⚠️ Der Grund ist eine Echtfarbe und keine benannte: Die
            // Palette hat keinen „leicht getoenten" Ton, und fuer den
            // Verlauf nach aussen braucht es Zwischenstufen.
            eingabe: Stil::unterlegt(Color::Reset, EINGABETON),
            deckung: 130,
            aktion: Stil::fett(Color::Cyan),
            ergebnis: Stil::schlicht(Color::Green),
            warnung: Stil::fett(Color::Yellow),
            code: Stil::schlicht(Color::Cyan),
            ueberschrift: Stil::fett(Color::Reset),
            marke: Stil::schlicht(Color::Cyan),
            gedanke: Stil::kursiv(Color::AnsiValue(244)),
            beiwerk: Stil::schlicht(Color::AnsiValue(244)),
            kante: Stil::schlicht(Color::AnsiValue(240)),
        },
        Konsolendesign::Myelith => {
            let (a, b) = crate::farben::paar_der_sitzung();
            myelith_rollen(a, b, crate::farben::logoton())
        }
        Konsolendesign::Bernstein => {
            let bernstein = |r: u8, g: u8, b: u8| Color::Rgb { r, g, b };
            Rollen {
                eingabe: Stil::unterlegt(bernstein(255, 200, 90), bernstein(255, 176, 0)),
                deckung: 110,
                aktion: Stil::fett(bernstein(255, 196, 70)),
                ergebnis: Stil::schlicht(bernstein(226, 152, 30)),
                warnung: Stil::betont(bernstein(255, 224, 160)),
                code: Stil::schlicht(bernstein(255, 214, 140)),
                ueberschrift: Stil::fett(bernstein(255, 226, 170)),
                marke: Stil::schlicht(bernstein(255, 214, 140)),
                gedanke: Stil::kursiv(bernstein(140, 95, 10)),
                beiwerk: Stil::schlicht(bernstein(178, 116, 0)),
                kante: Stil::schlicht(bernstein(138, 86, 0)),
            }
        }
        Konsolendesign::Tiefsee => {
            let see = |r: u8, g: u8, b: u8| Color::Rgb { r, g, b };
            Rollen {
                eingabe: Stil::unterlegt(see(150, 238, 244), see(70, 150, 230)),
                deckung: 120,
                aktion: Stil::fett(see(92, 224, 232)),
                ergebnis: Stil::schlicht(see(110, 214, 170)),
                warnung: Stil::fett(see(255, 164, 120)),
                code: Stil::schlicht(see(176, 214, 238)),
                ueberschrift: Stil::fett(see(214, 244, 248)),
                marke: Stil::schlicht(see(176, 214, 238)),
                gedanke: Stil::kursiv(see(96, 132, 168)),
                beiwerk: Stil::schlicht(see(96, 132, 168)),
                kante: Stil::schlicht(see(42, 78, 120)),
            }
        }
        Konsolendesign::Tinte => Rollen {
            eingabe: Stil::unterlegt(grau(255), grau(220)),
            deckung: 100,
            aktion: Stil::fett(grau(255)),
            ergebnis: Stil::schlicht(grau(212)),
            warnung: Stil::betont(grau(255)),
            code: Stil::schlicht(grau(226)),
            ueberschrift: Stil::fett(grau(255)),
            marke: Stil::schlicht(grau(226)),
            gedanke: Stil::kursiv(grau(120)),
            beiwerk: Stil::schlicht(grau(140)),
            kante: Stil::schlicht(grau(96)),
        },
    }
}

/// **Die Rollen im Design Myelith**, aus den drei Farben der Sitzung.
///
/// ⚑ **Die Warnfarbe weicht aus.** Die Schlagwortfarben sind gewuerfelt
/// und koennen genau das Orange treffen, in dem sonst gewarnt wird; dann
/// saehen Aufruf und Warnung gleich aus. Genommen wird deshalb die erste
/// aus Orange, Rot, Rosarot und Gelb, die keine der beiden ist. 📌
/// Gefunden hat es eine Probe, die in genau dieser Sitzung lief; seither
/// prueft sie jede.
fn myelith_rollen(a: Color, b: Color, logo: Color) -> Rollen {
    let (a, b, logo) = (lesbar(a), lesbar(b), lesbar(logo));
    let warnfarbe = [208u8, 203, 226, 214]
        .into_iter()
        .map(Color::AnsiValue)
        .find(|f| *f != a && *f != b)
        .unwrap_or(Color::AnsiValue(208));
    // ⚑ **Die markierten Stellen der Antwort in einem ruhigen Ton**
    // (Wunsch des Projektinhabers, 2026-09-24): ein blasses Cyangrau fuer
    // Code und Marken, Weiss fuer Ueberschriften. Nicht das gewuerfelte
    // Neon: Eine Antwort wird gelesen, nicht angeschaut.
    let ruhig = Color::AnsiValue(152);
    Rollen {
        eingabe: Stil::unterlegt(logo, EINGABETON),
        deckung: 130,
        aktion: Stil::fett(a),
        ergebnis: Stil::schlicht(b),
        warnung: Stil::fett(warnfarbe),
        code: Stil::schlicht(ruhig),
        ueberschrift: Stil::fett(Color::AnsiValue(255)),
        marke: Stil::schlicht(ruhig),
        gedanke: Stil::kursiv(Color::AnsiValue(244)),
        beiwerk: Stil::schlicht(Color::AnsiValue(244)),
        kante: Stil::schlicht(Color::AnsiValue(240)),
    }
}

/// **Hebt eine zu dunkle Neonfarbe an**, damit sie sich vom dunklen
/// Grund absetzt.
///
/// 📌 Purpur (129), Violett (165), Lavendel (99), Magenta (201) und Rot
/// (196) haben eine wahrgenommene Helle von rund 55 bis 105 von 255 und
/// sind auf schwarzem Grund kaum zu lesen. Genommen wird der hellere
/// Nachbar im selben Farbton; alles andere bleibt.
fn lesbar(farbe: Color) -> Color {
    match farbe {
        Color::AnsiValue(129) => Color::AnsiValue(177),
        Color::AnsiValue(165) => Color::AnsiValue(171),
        Color::AnsiValue(99) => Color::AnsiValue(141),
        Color::AnsiValue(201) => Color::AnsiValue(207),
        Color::AnsiValue(196) => Color::AnsiValue(203),
        andere => andere,
    }
}

static GRUND: OnceLock<(u8, u8, u8)> = OnceLock::new();

/// **Der Hintergrund des Terminals**, wie es ihn beim Start genannt hat,
/// sonst Schwarz.
///
/// ⚑ **Wozu:** Der Grund der Eingabe ist ein Hauch ueber der Farbe des
/// Terminals, und dafuer muss man sie kennen. Ueber Schwarz gemischt
/// stuende auf einem hellen oder grauen Terminal ein dunkles Rechteck.
pub fn terminalgrund() -> (u8, u8, u8) {
    GRUND.get().copied().unwrap_or((0, 0, 0))
}

/// **Fragt das Terminal nach seinem Hintergrund** (OSC 11), einmal, vor
/// allem anderen.
///
/// ⚠️ **Vor der ersten Tastenabfrage rufen**: Die Antwort kommt ueber
/// die Eingabe, und wer sie dort nicht abholt, findet sie spaeter als
/// getippte Zeichen in der Eingabezeile.
pub fn grund_erfragen() {
    if GRUND.get().is_none() {
        if let Some(g) = osc11_fragen() {
            let _ = GRUND.set(g);
        }
    }
}

/// **Liest die Antwort auf OSC 11**: `ESC ] 11 ; rgb:RRRR/GGGG/BBBB`,
/// abgeschlossen mit BEL oder ST. Jeder Kanal hat ein bis vier
/// Hexziffern und wird auf 0 bis 255 umgerechnet.
pub fn osc11_lesen(antwort: &[u8]) -> Option<(u8, u8, u8)> {
    let text = String::from_utf8_lossy(antwort);
    let rest = &text[text.find("rgb:")? + 4..];
    let ende = rest.find(['\x07', '\x1b']).unwrap_or(rest.len());
    let kanaele: Vec<&str> = rest[..ende].split('/').collect();
    if kanaele.len() != 3 {
        return None;
    }
    let kanal = |k: &str| -> Option<u8> {
        if k.is_empty() || k.len() > 4 {
            return None;
        }
        let wert = u32::from_str_radix(k, 16).ok()?;
        let max = (1u32 << (4 * k.len())) - 1;
        Some((wert * 255 / max) as u8)
    };
    Some((kanal(kanaele[0])?, kanal(kanaele[1])?, kanal(kanaele[2])?))
}

#[cfg(unix)]
fn osc11_fragen() -> Option<(u8, u8, u8)> {
    use std::io::{IsTerminal, Read, Write};
    use std::time::{Duration, Instant};
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return None;
    }
    let _roh = crate::auswahl::Rohmodus::an().ok()?;
    let mut aus = std::io::stdout();
    write!(aus, "\x1b]11;?\x1b\\").ok()?;
    aus.flush().ok()?;
    // ⚑ **Mit Frist**: Ein Terminal, das die Frage nicht kennt, antwortet
    // nie, und dann soll der Start nicht haengen. 300 ms sind fuer eine
    // oertliche Antwort das Hundertfache.
    let frist = Instant::now() + Duration::from_millis(300);
    let mut puffer: Vec<u8> = Vec::new();
    while !puffer.contains(&0x07) && !puffer.windows(2).any(|w| w == b"\x1b\\") {
        let rest = frist.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            break;
        }
        let mut abfrage = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
        // SICHERHEIT: `poll` bekommt einen gueltigen Zeiger auf genau
        // einen Eintrag und liest nur die Eingabe dieses Prozesses.
        let bereit = unsafe { libc::poll(&mut abfrage, 1, rest.as_millis() as libc::c_int) };
        if bereit <= 0 {
            break;
        }
        let mut stueck = [0u8; 64];
        match std::io::stdin().lock().read(&mut stueck) {
            Ok(0) | Err(_) => break,
            Ok(n) => puffer.extend_from_slice(&stueck[..n]),
        }
    }
    osc11_lesen(&puffer)
}

#[cfg(not(unix))]
fn osc11_fragen() -> Option<(u8, u8, u8)> {
    None
}

/// **Ob Farbe ausgegeben wird**: nur an einem Terminal, und nicht, wenn
/// `NO_COLOR` gesetzt ist.
///
/// ⚑ `NO_COLOR` ist die uebliche Absprache fuer „keine Farbe, egal
/// welches Programm". Wer sie setzt, meint es fuer alle Programme.
pub fn farbig() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
}

/// **Die Farbe des Ladetextes zu diesem Zeitpunkt.**
///
/// ⚑ **Im Regenbogen oder in einer Farbe**, je nach Design: Ein
/// Regenbogen in einem einfarbigen Bild ist kein Akzent, sondern ein
/// Fremdkoerper. Dort pulst stattdessen die Helle derselben Farbe, im
/// selben Takt von zwei Minuten.
pub fn ladefarbe(d: Konsolendesign, seit: std::time::Duration) -> Color {
    if d.regenbogen() {
        let (r, g, b) = crate::anzeige::regenbogen(seit);
        return Color::Rgb { r, g, b };
    }
    let Color::Rgb { r, g, b } = toene(d).akzent else {
        return toene(d).akzent;
    };
    let anteil = puls(seit);
    Color::Rgb {
        r: (r as u32 * anteil / 255) as u8,
        g: (g as u32 * anteil / 255) as u8,
        b: (b as u32 * anteil / 255) as u8,
    }
}

/// Ein Dreieckspuls zwischen einem Drittel und voller Helle.
///
/// ⚑ **Ganzzahlig und stetig.** Er steigt eine Minute lang und faellt
/// eine Minute lang; am Umkehrpunkt gibt es keinen Sprung, weil Anfang
/// und Ende denselben Wert haben.
pub fn puls(seit: std::time::Duration) -> u32 {
    const UNTEN: u32 = 85;
    const HALB: u128 = 60_000;
    let t = seit.as_millis() % (HALB * 2);
    let auf = if t < HALB { t } else { HALB * 2 - t };
    UNTEN + ((255 - UNTEN) as u128 * auf / HALB) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Standard setzt keinen eigenen Akzent.**
    ///
    /// ⚑ Die Zusage des Designs steht damit im Typ und nicht in einem
    /// Satz darueber.
    #[test]
    fn standard_nimmt_die_farbe_des_terminals() {
        assert_eq!(toene(Konsolendesign::Standard).akzent, Color::Reset);
    }

    /// **Jedes andere Design setzt drei verschiedene Toene.**
    ///
    /// 📌 Zwei gleiche Toene waeren ein Rahmen, den man von seiner
    /// Fusszeile nicht unterscheidet.
    #[test]
    fn jedes_design_hat_drei_unterscheidbare_toene() {
        for d in Konsolendesign::ALLE {
            if d == Konsolendesign::Standard {
                continue;
            }
            let t = toene(d);
            assert_ne!(t.kante, t.beiwerk, "{} hat Kante und Beiwerk gleich", d.name());
            assert_ne!(t.beiwerk, t.akzent, "{} hat Beiwerk und Akzent gleich", d.name());
        }
    }

    /// **Der Puls springt am Umkehrpunkt nicht.**
    ///
    /// 📌 Ein Sprung von voller Helle auf ein Drittel sieht aus wie ein
    /// Fehler und nicht wie eine Bewegung.
    #[test]
    fn der_puls_bleibt_stetig() {
        use std::time::Duration;
        let mut vorher = puls(Duration::ZERO);
        for ms in (0..240_000).step_by(500) {
            let jetzt = puls(Duration::from_millis(ms));
            let sprung = jetzt.abs_diff(vorher);
            assert!(sprung < 10, "bei {ms} ms springt der Puls um {sprung}");
            vorher = jetzt;
        }
    }

    /// **Und er bleibt in seinen Grenzen.**
    #[test]
    fn der_puls_bleibt_im_rahmen() {
        use std::time::Duration;
        for ms in (0..240_000).step_by(377) {
            let p = puls(Duration::from_millis(ms));
            assert!((85..=255).contains(&p), "bei {ms} ms ist der Puls {p}");
        }
    }

    /// **Jede Rolle ist in jedem Design von ihren Nachbarn zu
    /// unterscheiden**: Aufruf, Rueckgabe und Warnung stehen nie im
    /// selben Stil, und die Warnung ist betont (fett oder unterstrichen).
    #[test]
    fn die_rollen_sind_unterscheidbar() {
        // Myelith in jeder moeglichen Sitzung, nicht nur in der
        // gewuerfelten: Die Warnfarbe muss jedem Paar ausweichen.
        let myelith = (0..crate::farben::NEON.len()).map(|logo| {
            let (a, b) = crate::farben::paar(logo);
            let ton = |i: usize| Color::AnsiValue(crate::farben::NEON[i]);
            (Konsolendesign::Myelith, myelith_rollen(ton(a), ton(b), ton(logo)))
        });
        for (d, r) in Konsolendesign::ALLE.into_iter().map(|d| (d, rollen(d))).chain(myelith) {
            assert_ne!(r.aktion, r.ergebnis, "{}: Aufruf und Rückgabe gleich", d.name());
            assert_ne!(r.aktion, r.warnung, "{}: Aufruf und Warnung gleich", d.name());
            assert_ne!(r.ergebnis, r.warnung, "{}: Rückgabe und Warnung gleich", d.name());
            assert!(r.warnung.fett || r.warnung.unterstrichen, "{}: Warnung nicht betont", d.name());
            assert!(r.aktion.fett, "{}: der Aufruf ist nicht fett", d.name());
            assert_ne!(r.beiwerk, r.aktion, "{}", d.name());
            assert!(r.eingabe.hintergrund.is_some(), "{}: die Eingabe ist nicht unterlegt", d.name());
        }
    }

    /// Wahrgenommene Helle einer Farbe von 0 bis 255, oder `None` fuer
    /// benannte Farben, deren Wert das Terminal bestimmt.
    fn helle(c: Color) -> Option<u32> {
        let (r, g, b) = match c {
            Color::Rgb { r, g, b } => (r as u32, g as u32, b as u32),
            Color::AnsiValue(n @ 16..=231) => {
                let i = n as u32 - 16;
                let stufe = |s: u32| if s == 0 { 0 } else { 55 + 40 * s };
                (stufe(i / 36), stufe((i / 6) % 6), stufe(i % 6))
            }
            Color::AnsiValue(n @ 232..=255) => {
                let w = 8 + 10 * (n as u32 - 232);
                (w, w, w)
            }
            _ => return None,
        };
        Some((2126 * r + 7152 * g + 722 * b) / 10000)
    }

    /// **Was hervorgehoben ist, setzt sich vom dunklen Grund ab**
    /// (Wunsch des Projektinhabers, 2026-09-24): die markierten Stellen
    /// der Antwort deutlich (Helle ab 150 von 255), Aufrufe, Rueckgaben
    /// und Warnungen lesbar (ab 120). In jedem Design mit eigenen Werten
    /// und in **jeder** moeglichen Sitzung von Myelith.
    ///
    /// 📌 Die Gegenprobe ist der Fall, fuer den es die Probe gibt: Mit
    /// dem gewuerfelten Logoton als Ueberschrift stand in einer von sechs
    /// Sitzungen Purpur mit einer Helle von 55 da.
    #[test]
    fn hervorgehobenes_setzt_sich_vom_grund_ab() {
        let myelith = (0..crate::farben::NEON.len()).map(|logo| {
            let (a, b) = crate::farben::paar(logo);
            let ton = |i: usize| Color::AnsiValue(crate::farben::NEON[i]);
            (format!("Myelith, Logo {}", crate::farben::NEON[logo]), myelith_rollen(ton(a), ton(b), ton(logo)))
        });
        let feste = [Konsolendesign::Bernstein, Konsolendesign::Tiefsee, Konsolendesign::Tinte]
            .into_iter()
            .map(|d| (d.name().to_string(), rollen(d)));
        for (name, r) in myelith.chain(feste) {
            for (rolle, stil, mindestens) in [
                ("Code", r.code, 150),
                ("Ueberschrift", r.ueberschrift, 150),
                ("Marke", r.marke, 150),
                ("Aufruf", r.aktion, 120),
                ("Rueckgabe", r.ergebnis, 120),
                ("Warnung", r.warnung, 120),
            ] {
                let h = helle(stil.farbe).expect("feste Farbe");
                assert!(h >= mindestens, "{name}: {rolle} hat nur Helle {h}");
            }
        }
    }

    /// **Die Antwort des Terminals wird gelesen**, in beiden Abschluessen
    /// und mit zwei wie mit vier Hexziffern je Kanal; Unfug ergibt nichts.
    #[test]
    fn die_farbe_des_terminals_wird_gelesen() {
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:1e1e/1e1e/2424\x07"), Some((30, 30, 36)));
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:ffff/8080/0000\x1b\\"), Some((255, 128, 0)));
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:ff/00/7f\x07"), Some((255, 0, 127)));
        // 📌 **Ohne wiederholte Ziffernpaare.** Bei `1e1e` trifft das
        // niedrige Byte zufaellig den richtigen Wert, und eine Umrechnung
        // mit falschem Teiler fiele nicht auf; die Gegenprobe hat es
        // gezeigt.
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:1234/abcd/0000\x07"), Some((18, 171, 0)));
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:f/8/0\x07"), Some((255, 136, 0)));
        assert_eq!(osc11_lesen(b""), None);
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:zz/00/00\x07"), None);
        assert_eq!(osc11_lesen(b"\x1b]11;rgb:ff/00\x07"), None);
    }

    /// **Ein gesetzter Text setzt sich selbst zurueck**, und ohne Farbe
    /// bleibt er, wie er ist.
    #[test]
    fn ein_stil_setzt_sich_zurueck() {
        let s = Stil::betont(Color::Rgb { r: 1, g: 2, b: 3 });
        let farbig = s.faerben("Wort", true);
        assert!(farbig.contains("Wort"));
        assert!(farbig.starts_with('\x1b'), "{farbig:?}");
        assert!(farbig.ends_with("\x1b[0m"), "kein Rücksetzen am Ende: {farbig:?}");
        assert_eq!(s.faerben("Wort", false), "Wort");
        assert_eq!(s.faerben("", true), "");
    }

    /// **Ein einfarbiges Design bekommt keinen Regenbogen.**
    #[test]
    fn einfarbige_designs_pulsen_statt_zu_wandern() {
        assert!(!Konsolendesign::Bernstein.regenbogen());
        assert!(!Konsolendesign::Tinte.regenbogen());
        assert!(Konsolendesign::Myelith.regenbogen());
    }
}
