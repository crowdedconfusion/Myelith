//! Das Logo im Regenbogen: immer mit Verlauf, fliessend nur waehrend
//! eines Auftrags.
//!
//! # ⚑ Drei Zusagen (Auftrag des Projektinhabers, 2026-09-24)
//!
//! 1. **Das Logo ist nie einfarbig.** Ueber den Schriftzug laeuft immer
//!    ein Farbverlauf, auch wenn nichts laeuft. Gewaehlt ist die dezente
//!    Spanne: **ein Drittel des Farbkreises ueber die 56 Spalten des
//!    Schriftzugs**; das Netz, das die ganze Fensterbreite fuellt, traegt
//!    entsprechend mehr.
//! 2. **Es fliesst, solange ein Auftrag laeuft, und steht sonst still.**
//!    Der Verlauf wandert mit derselben Geschwindigkeit nach links wie
//!    die Welle im Ladetext, eine Spalte je [`crate::anzeige::WELLE`].
//!    ⚑ **Angehalten wird die Uhr, nicht das Bild zurueckgesetzt:** Der
//!    naechste Auftrag setzt genau dort fort, wo der vorige aufhoerte.
//! 3. **Der Aufbau im Startbild endet in genau diesem Bild.** Startbild,
//!    jeder spaetere Neudruck und das Fliessen fragen dieselbe Funktion
//!    ([`zellstil`]) nach derselben Uhr. Deshalb beginnt der erste
//!    Auftrag ohne Sprung.
//!
//! ⚑ **Jede Sitzung hat ihren eigenen Grundton.** Die Mitte des Logos
//! traegt beim Start die Logofarbe der Sitzung ([`crate::farben`]), und
//! die ist gewuerfelt. Die Schlagwortfarben liegen in ihrer
//! Nachbarschaft, also finden sie sich im Logo wieder.
//!
//! # ⛔️ Gemalt wird nur, wo das Logo sicher steht
//!
//! Das Logo wird einmal gedruckt und rollt danach mit dem Gespraech weg.
//! **Ein Terminal sagt nicht, wie weit.** Wer an die alte Stelle malt,
//! nachdem es sich bewegt hat, malt Buchstaben des Logos in die Antwort
//! des Modells.
//!
//! Die sichere Aussage kommt vom Wagen: Im Rollbereich bewegt er sich nur
//! nach unten, und gerollt wird erst, wenn er die letzte Zeile erreicht
//! hat. **Steht er darueber, hat sich seit dem Druck nichts verschoben.**
//! Steht er auf der letzten Zeile, ist die Lage unbekannt, und sie wird
//! verworfen, bis das Logo neu gedruckt wird. Verworfen wird ausserdem,
//! wenn sich die Fenstergroesse aendert und bevor eine Auswahlliste
//! zeichnet, denn die bewegt den Wagen nach oben.
//!
//! 📌 **Deshalb beginnt das Gespraech seit dem 2026-09-24 direkt unter
//! dem Logo** und nicht mehr am unteren Rand des Rollbereichs
//! ([`crate::schirm::Schirm::einrichten`]): Dort stand der Wagen vom
//! ersten Moment an auf der letzten Zeile, jede Zeile schob das Logo
//! weiter, und keine Animation haette je sicher malen koennen.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crossterm::style::{Attribute, Color};
use myl_client::einstellungen::Konsolendesign;

/// Ein voller Farbdurchlauf in Millisekunden, derselbe wie im Ladetext.
const ZYKLUS_MS: u64 = crate::anzeige::REGENBOGEN.as_millis() as u64;

/// Wie weit die Farbe von einer Spalte zur naechsten weiterwandert.
///
/// ⚑ **Gerechnet und nicht geschaetzt:** ein Drittel des Farbkreises
/// ueber die Breite des Schriftzugs (Festlegung des Projektinhabers,
/// 2026-09-24, „dezent"). Der Ladetext nimmt 400 ms je Zeichen, also
/// rund 20 Grad ueber seine Laenge; das liest sich als eine Farbe und
/// waere genau das, was das Logo nicht sein soll.
pub const SPALTE_MS: u64 = ZYKLUS_MS / 3 / crate::banner::SCHRIFTBREITE as u64;

/// Und von einer Zeile zur naechsten: eine Spalte, also ein leichter
/// Schraegverlauf statt senkrechter Streifen.
pub const ZEILE_MS: u64 = SPALTE_MS;

/// Wie oft das Logo waehrend eines Auftrags neu gemalt wird, in Takten
/// der Anzeige.
///
/// ⚑ **Jeder zweite Takt, also rund alle 180 ms.** Bei 2,5 Spalten je
/// Sekunde wandert der Verlauf dazwischen weniger als eine halbe Spalte,
/// das Auge sieht eine Bewegung und keine Spruenge. Jeder Takt hiesse die
/// doppelte Menge Steuerzeichen, und ueber eine ferne Sitzung ist das
/// spuerbar.
pub const MALTAKT: usize = 2;

/// Die Uhr des Schimmers: sie laeuft nur waehrend eines Auftrags.
#[derive(Debug, Clone, Copy)]
pub struct Uhr {
    angesammelt: Duration,
    seit: Option<Instant>,
}

impl Uhr {
    pub const fn neu() -> Self {
        Self { angesammelt: Duration::ZERO, seit: None }
    }

    /// Laesst sie laufen. Ein zweiter Aufruf aendert nichts.
    pub fn los(&mut self, jetzt: Instant) {
        if self.seit.is_none() {
            self.seit = Some(jetzt);
        }
    }

    /// Haelt sie an; die bis hierher gelaufene Zeit bleibt.
    pub fn halt(&mut self, jetzt: Instant) {
        if let Some(s) = self.seit.take() {
            self.angesammelt += jetzt.saturating_duration_since(s);
        }
    }

    /// Wie lange sie insgesamt gelaufen ist.
    pub fn stand(&self, jetzt: Instant) -> Duration {
        self.angesammelt + self.seit.map(|s| jetzt.saturating_duration_since(s)).unwrap_or_default()
    }
}

static UHR: Mutex<Uhr> = Mutex::new(Uhr::neu());

/// Der Schimmer beginnt zu fliessen.
pub fn los() {
    if let Ok(mut u) = UHR.lock() {
        u.los(Instant::now());
    }
}

/// Der Schimmer steht still, und zwar in seinem letzten Bild.
pub fn halt() {
    if let Ok(mut u) = UHR.lock() {
        u.halt(Instant::now());
    }
}

/// Der Stand der Uhr jetzt.
pub fn stand() -> Duration {
    UHR.lock().map(|u| u.stand(Instant::now())).unwrap_or_default()
}

/// **Die Farbe einer Zelle des Logos**, rein gerechnet.
///
/// `spalte` ist die Bildschirmspalte, `zeile` die Zeile **im Logo** (nicht
/// auf dem Schirm, sonst wechselte die Farbe beim Hochgleiten im
/// Startbild), `breite` die Fensterbreite und `grundton` der Farbton der
/// Sitzung in Grad.
///
/// ⚑ **Die Mitte des Fensters traegt beim Uhrstand null den Grundton.**
/// Und die Uhr wird so umgerechnet, dass der Verlauf **eine Spalte je
/// Welle des Ladetextes** wandert: Nach `WELLE` hat jede Spalte die Farbe
/// ihrer rechten Nachbarin. Die Richtung ist dieselbe wie dort.
///
/// ⚑ **Die Farbe selbst kommt aus [`crate::design::ladefarbe`]**, also
/// derselben Stelle wie die des Ladetextes: in den bunten Designs der
/// Regenbogen, in den einfarbigen ein Verlauf der Helle.
///
/// ⚑ **`gegenlaeufig` kehrt nur die Richtung um** (Wunsch des
/// Projektinhabers, 2026-09-24): Das Muster um das Logo wandert nach
/// rechts, waehrend das Logo nach links wandert. Bei stehender Uhr aendert
/// das nichts; erst im Fliessen trennt es die beiden Ebenen, und das Auge
/// liest den Unterschied als Tiefe.
#[allow(clippy::too_many_arguments)]
pub fn farbe(
    d: Konsolendesign,
    spalte: usize,
    zeile: usize,
    breite: usize,
    grundton: i32,
    uhr: Duration,
    gegenlaeufig: bool,
) -> Color {
    let welle = crate::anzeige::WELLE.as_millis() as u64;
    let fluss = (uhr.as_millis() as u64 % (ZYKLUS_MS * welle)) * SPALTE_MS / welle % ZYKLUS_MS;
    let grund = grundton.rem_euclid(360) as u64 * ZYKLUS_MS / 360;
    let mitte = (breite as u64 / 2) * SPALTE_MS % ZYKLUS_MS;
    let ort = grund + spalte as u64 * SPALTE_MS + zeile as u64 * ZEILE_MS + ZYKLUS_MS - mitte;
    let t = if gegenlaeufig { ort + ZYKLUS_MS - fluss } else { ort + fluss } % ZYKLUS_MS;
    crate::design::ladefarbe(d, Duration::from_millis(t))
}

/// **Farbe und Staerke eines Zeichens im Logo**, und zwar fuer jeden, der
/// es zeichnet: Neudruck, Startbild und Fliessen.
///
/// ⚑ **Eine Funktion fuer alle drei**, und das ist die Zusage des
/// nahtlosen Uebergangs. Rechneten Startbild und Neudruck getrennt,
/// stuende nach dem Aufbau ein anderes Bild da als eine Zeile spaeter.
///
/// Die Stufen aus [`crate::banner::zeichenstil`]: Der Schriftzug leuchtet,
/// das Muster ist gedaempft und laeuft gegen ihn.
pub fn zellstil(
    d: Konsolendesign,
    c: char,
    spalte: usize,
    zeile: usize,
    breite: usize,
    uhr: Duration,
) -> (Color, Attribute) {
    let gegen = !crate::banner::ist_logozeichen(c);
    let f = farbe(d, spalte, zeile, breite, crate::farben::grundton(), uhr, gegen);
    crate::banner::zeichenstil(c, f)
}

/// Ob ein Zeichen beim Fliessen neu gemalt werden muss.
///
/// ⚑ **Alles, was Farbe traegt**, also Schriftzug und Muster
/// ([`crate::banner::stufe`]); das Muster liegt im selben Verlauf, nur
/// gedaempft und gegenlaeufig. Leerraum bleibt, wie er ist.
pub fn farbig(c: char) -> bool {
    c != ' ' && crate::banner::stufe(c) != crate::banner::Stufe::Kante
}

/// Wo das Logo steht, solange es sicher dort steht.
#[derive(Debug, Clone)]
struct Lage {
    /// Die Bildschirmzeile der ersten Logozeile, ab null.
    oben: u16,
    /// Das Logo, Zeile fuer Zeile, so wie es gedruckt wurde.
    zeilen: Vec<String>,
    /// Die Fenstergroesse beim Druck.
    fenster: (u16, u16),
    design: Konsolendesign,
}

static LAGE: Mutex<Option<Lage>> = Mutex::new(None);

/// Das Logo steht jetzt, ab Zeile `oben`.
pub fn gedruckt(text: &str, oben: u16, fenster: (u16, u16), design: Konsolendesign) {
    if let Ok(mut l) = LAGE.lock() {
        *l = Some(Lage { oben, zeilen: text.lines().map(str::to_string).collect(), fenster, design });
    }
}

/// Die Lage ist nicht mehr sicher, also wird nicht mehr gemalt.
pub fn vergessen() {
    if let Ok(mut l) = LAGE.lock() {
        *l = None;
    }
}

/// **Darf gemalt werden?** Rein, damit es sich pruefen laesst.
///
/// `wagen` ist die Zeile des Wagens ab null, `rollende` die letzte Zeile
/// des Rollbereichs **ab eins**, so wie [`crate::schirm::Schirm::rollende`]
/// sie liefert.
///
/// ⛔️ **Steht der Wagen auf der letzten Zeile des Rollbereichs, kann
/// gerollt worden sein**, und wie weit, sagt niemand. Dann nicht.
pub fn darf_malen(wagen: u16, rollende: u16, fenster_jetzt: (u16, u16), fenster_druck: (u16, u16)) -> bool {
    fenster_jetzt == fenster_druck && wagen.saturating_add(1) < rollende
}

/// **Malt das Logo im Stand der Uhr neu**, sofern es sicher dort steht.
///
/// ⚑ **Vor jedem Malen wird nachgemessen**, und zwar der Wagen und das
/// Fenster. Eine Messung vom vorigen Takt wuerde reichen, solange die
/// Anzeige der einzige Schreiber ist; **eine Zusage, die an „solange"
/// haengt, gilt bis zu dem Tag, an dem jemand anders schreibt.**
///
/// ⚠️ **Scheitert die Messung, wird die Lage verworfen.** Ein Terminal,
/// das auf die Frage nach dem Wagen nicht antwortet, liesse sonst jeden
/// Takt zwei Sekunden warten.
pub fn malen(rollende: u16) {
    let Ok(mut gesperrt) = LAGE.lock() else { return };
    let Some(lage) = gesperrt.as_ref() else { return };
    let (Ok(fenster), Ok((_, wagen))) = (crossterm::terminal::size(), crossterm::cursor::position())
    else {
        *gesperrt = None;
        return;
    };
    if !darf_malen(wagen, rollende, fenster, lage.fenster) {
        *gesperrt = None;
        return;
    }

    let uhr = stand();
    let breite = lage.fenster.0 as usize;
    let mut aus = std::io::stdout();
    let mut puffer: Vec<u8> = Vec::with_capacity(8 * 1024);
    // ⚑ **Gemerkt und zurueckgeholt**, wie bei der Ladezeile: Der Wagen
    // gehoert dem Gespraech.
    let _ = write!(puffer, "\x1b7");
    for (y, zeile) in lage.zeilen.iter().enumerate() {
        for (x, c) in zeile.chars().enumerate() {
            if !farbig(c) {
                continue;
            }
            let (ton, stark) = zellstil(lage.design, c, x, y, breite, uhr);
            let _ = crossterm::queue!(
                puffer,
                crossterm::cursor::MoveTo(x as u16, lage.oben + y as u16),
                crossterm::style::SetForegroundColor(ton),
                crossterm::style::SetAttribute(stark),
                crossterm::style::Print(c)
            );
        }
    }
    let _ = crossterm::queue!(
        puffer,
        crossterm::style::ResetColor,
        crossterm::style::SetAttribute(Attribute::Reset)
    );
    let _ = write!(puffer, "\x1b8");
    let _ = aus.write_all(&puffer);
    let _ = aus.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color) -> (u8, u8, u8) {
        match c {
            Color::Rgb { r, g, b } => (r, g, b),
            andere => panic!("keine Echtfarbe: {andere:?}"),
        }
    }

    /// **Der Farbton einer Echtfarbe in Grad**, ganzzahlig.
    fn ton(c: Color) -> i32 {
        let (r, g, b) = rgb(c);
        let (r, g, b) = (r as i32, g as i32, b as i32);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let d = max - min;
        if d == 0 {
            return 0;
        }
        let h = if max == r {
            60 * (g - b) / d
        } else if max == g {
            120 + 60 * (b - r) / d
        } else {
            240 + 60 * (r - g) / d
        };
        (h + 360) % 360
    }

    fn abstand(a: i32, b: i32) -> i32 {
        let d = (a - b).abs();
        d.min(360 - d)
    }

    /// **Die dezente Spanne: ein Drittel des Farbkreises ueber den
    /// Schriftzug**, und zwar gemessen an den Farben selbst und nicht an
    /// der Konstanten.
    ///
    /// 📌 Die Gegenprobe steckt in der Toleranz: Mit der Spanne des
    /// Ladetextes (400 ms je Zeichen) laege der Abstand bei rund 67 Grad
    /// und fiele durch.
    #[test]
    fn der_schriftzug_ueberspannt_ein_drittel_des_farbkreises() {
        let b = crate::banner::SCHRIFTBREITE;
        for grundton in [0, 77, 180, 301] {
            for uhr_s in [0u64, 13, 59, 120, 3600] {
                let uhr = Duration::from_secs(uhr_s);
                let links = farbe(Konsolendesign::Myelith, 0, 0, 120, grundton, uhr, false);
                let rechts = farbe(Konsolendesign::Myelith, b - 1, 0, 120, grundton, uhr, false);
                let d = abstand(ton(links), ton(rechts));
                assert!((110..=130).contains(&d), "Grundton {grundton}, Uhr {uhr_s} s: {d} Grad");
            }
        }
    }

    /// **Nie einfarbig**, in keinem Design und zu keinem Zeitpunkt.
    ///
    /// ⚑ Auch die einfarbigen Designs zeigen einen Verlauf, dort in der
    /// Helle statt im Farbton; ueber den Schriftzug muessen mindestens
    /// zehn verschiedene Toene stehen. `Standard` wandert wie der Ladetext
    /// durch den Regenbogen.
    #[test]
    fn das_logo_ist_nie_einfarbig() {
        for d in Konsolendesign::ALLE {
            for uhr_s in (0..240).step_by(7) {
                let uhr = Duration::from_secs(uhr_s);
                let toene: std::collections::BTreeSet<(u8, u8, u8)> = (0..crate::banner::SCHRIFTBREITE)
                    .map(|x| rgb(farbe(d, 20 + x, 3, 96, 40, uhr, false)))
                    .collect();
                assert!(toene.len() >= 10, "{}: nur {} Toene bei {uhr_s} s", d.name(), toene.len());
            }
        }
    }

    /// **Der Verlauf fliesst so schnell wie die Welle im Ladetext**: Nach
    /// `WELLE` traegt jede Spalte die Farbe ihrer rechten Nachbarin.
    #[test]
    fn der_verlauf_wandert_eine_spalte_je_welle() {
        let w = crate::anzeige::WELLE;
        for d in [Konsolendesign::Myelith, Konsolendesign::Bernstein] {
            for schritte in [0u32, 1, 7, 150] {
                let uhr = w * schritte;
                for x in [0usize, 10, 55, 119] {
                    assert_eq!(
                        farbe(d, x, 2, 120, 200, uhr + w, false),
                        farbe(d, x + 1, 2, 120, 200, uhr, false),
                        "{} bei Spalte {x}, Schritt {schritte}",
                        d.name()
                    );
                }
            }
        }
    }

    /// **Das Muster fliesst gegen das Logo**: Nach `WELLE` traegt jede
    /// Spalte des Musters die Farbe ihrer **linken** Nachbarin, das Logo die
    /// seiner rechten.
    ///
    /// 📌 **Und bei stehender Uhr ist beides dasselbe Bild**: Die Richtung
    /// wirkt nur im Fliessen, zwischen zwei Auftraegen bleibt alles ruhig
    /// und der Verlauf ungebrochen.
    #[test]
    fn das_muster_fliesst_gegen_das_logo() {
        let w = crate::anzeige::WELLE;
        for d in [Konsolendesign::Myelith, Konsolendesign::Tinte] {
            for schritte in [0u32, 3, 90] {
                let uhr = w * schritte;
                for x in [1usize, 30, 118] {
                    assert_eq!(
                        farbe(d, x, 4, 120, 70, uhr + w, true),
                        farbe(d, x - 1, 4, 120, 70, uhr, true),
                        "{} bei Spalte {x}, Schritt {schritte}",
                        d.name()
                    );
                }
            }
            for x in [0usize, 40, 119] {
                assert_eq!(
                    farbe(d, x, 1, 120, 70, Duration::ZERO, true),
                    farbe(d, x, 1, 120, 70, Duration::ZERO, false),
                    "{}: bei stehender Uhr weichen die Ebenen ab",
                    d.name()
                );
            }
        }
    }

    /// **Welche Ebene welche Richtung nimmt, entscheidet das Zeichen**:
    /// Schriftzug vorwaerts, Muster gegenlaeufig, auch in derselben Zeile.
    #[test]
    fn die_richtung_haengt_am_zeichen() {
        let uhr = crate::anzeige::WELLE * 7;
        let d = Konsolendesign::Myelith;
        let g = crate::farben::grundton();
        let (logo, _) = zellstil(d, '█', 50, 3, 120, uhr);
        let (muster, _) = zellstil(d, '⣿', 50, 3, 120, uhr);
        assert_eq!(logo, farbe(d, 50, 3, 120, g, uhr, false));
        assert_eq!(
            muster,
            crate::banner::zeichenstil('⣿', farbe(d, 50, 3, 120, g, uhr, true)).0,
            "das Muster laeuft nicht gegen das Logo"
        );
    }

    /// **Die Mitte traegt beim Start den Grundton der Sitzung.**
    #[test]
    fn die_mitte_traegt_den_grundton() {
        for grundton in [0, 60, 180, 300] {
            let c = farbe(Konsolendesign::Myelith, 60, 0, 120, grundton, Duration::ZERO, false);
            assert!(abstand(ton(c), grundton) <= 2, "{grundton} Grad wurde {}", ton(c));
        }
    }

    /// **Die Uhr steht zwischen zwei Auftraegen und setzt dort fort, wo
    /// sie stand.** Ein Zuruecksetzen liesse das Logo beim naechsten
    /// Auftrag springen.
    #[test]
    fn die_uhr_haelt_an_und_setzt_fort() {
        let t0 = Instant::now();
        let mut u = Uhr::neu();
        assert_eq!(u.stand(t0 + Duration::from_secs(5)), Duration::ZERO, "sie lief ohne Auftrag");
        u.los(t0);
        u.los(t0 + Duration::from_secs(1));
        u.halt(t0 + Duration::from_secs(3));
        assert_eq!(u.stand(t0 + Duration::from_secs(100)), Duration::from_secs(3), "sie lief weiter");
        u.los(t0 + Duration::from_secs(200));
        assert_eq!(u.stand(t0 + Duration::from_secs(202)), Duration::from_secs(5));
    }

    /// ⛔️ **Kein Malen, sobald das Logo gerollt sein kann.**
    ///
    /// 📌 Die Gegenprobe ist der Fall, fuer den die Pruefung da ist: Der
    /// Wagen auf der letzten Zeile des Rollbereichs (`rollende` ab eins,
    /// der Wagen ab null) heisst, dass jede weitere Zeile rollt.
    #[test]
    fn gemalt_wird_nur_ueber_der_letzten_zeile() {
        let f = (120, 50);
        assert!(darf_malen(20, 43, f, f));
        assert!(darf_malen(41, 43, f, f));
        assert!(!darf_malen(42, 43, f, f), "auf der letzten Zeile des Rollbereichs");
        assert!(!darf_malen(49, 43, f, f), "unterhalb des Rollbereichs");
        assert!(!darf_malen(20, 43, (100, 50), f), "das Fenster ist schmaler geworden");
        assert!(!darf_malen(20, 43, (120, 40), f), "das Fenster ist niedriger geworden");
    }

    /// ⛔️ **Jeder Weg, der den Wagen nach oben bewegt oder den Schirm
    /// raeumt, verwirft die Lage**, und zwar am Eingang, bevor er zeichnet.
    ///
    /// 📌 Gegen den Wagen allein hilft das Nachmessen nicht: Eine
    /// Auswahlliste kann den Schirm rollen und den Wagen danach wieder
    /// hochziehen. Er stuende dann ueber der letzten Zeile, und das Logo
    /// saesse trotzdem woanders.
    #[test]
    fn wer_den_wagen_hochzieht_verwirft_die_lage() {
        let faelle = [
            ("wahl.rs", include_str!("wahl.rs"), "pub fn waehlen_ab("),
            ("einstellseite.rs", include_str!("einstellseite.rs"), "pub fn fahren("),
            ("sitzung.rs", include_str!("sitzung.rs"), "fn schirm_leeren("),
        ];
        for (datei, quelle, eingang) in faelle {
            let rumpf = quelle.split(eingang).nth(1).unwrap_or_else(|| panic!("{datei}: {eingang} fehlt"));
            let bis_zum_zeichnen: String = rumpf.lines().take(12).collect::<Vec<_>>().join("\n");
            assert!(
                bis_zum_zeichnen.contains("crate::schimmer::vergessen();"),
                "{datei}: {eingang} verwirft die Lage nicht am Eingang"
            );
        }
    }

    /// **Neu gemalt wird, was Farbe traegt**: Schriftzug und Muster.
    /// Leerraum bleibt, wie er gedruckt wurde.
    #[test]
    fn nur_was_farbe_traegt_wird_neu_gemalt() {
        for c in ['█', '═', '╚', '⣿', '⠁', '⢀'] {
            assert!(farbig(c), "{c:?} traegt Farbe");
        }
        for c in [' ', 'x', '─'] {
            assert!(!farbig(c), "{c:?} wird nicht gemalt");
        }
    }
}
