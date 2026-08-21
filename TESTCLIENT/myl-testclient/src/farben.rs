//! Neonfarben für Schriftzug und Auswahllisten.
//!
//! ## Warum 256-Farben-Indizes und kein RGB
//!
//! `Color::Rgb` verlangt ein Terminal mit Echtfarben. Verbreitet ist das,
//! aber nicht selbstverständlich: über SSH, in `screen`, in einer
//! seriellen Konsole und in mancher CI fällt es auf eine Näherung zurück
//!, und welche, entscheidet das Terminal, nicht dieses Programm. Die
//! 256-Farben-Palette gibt es seit Jahrzehnten überall, und ihre oberen
//! Bereiche sind genau die grellen Töne, um die es hier geht.
//!
//! ## Was hier zufällig sein darf und was nicht
//!
//! Farbe ist Schmuck. Sie steht **nie** für eine Aussage: Kein Urteil,
//! kein Fehler und kein Vergleichswert wird an einer Farbe erkennbar
//! gemacht: dafür stehen Wörter da (`ABWEICHUNG`, `FEHLER`, `NACHWEIS`).
//! Wer nur Graustufen sieht, sei es aus Farbenblindheit, sei es in einem
//! Protokollmitschnitt, verliert damit keine Information.
//!
//! Deshalb ist der Zufall hier unbedenklich, während er im Rechenpfad
//! dieses Projekts nirgends etwas zu suchen hat.
//!
//! ## Ein Farbschema je Sitzung
//!
//! Gewürfelt wird **einmal beim Start**, während sich das Logo aus der
//! Spirale bildet. Danach steht das Schema: dieselbe Logofarbe und
//! dieselben zwei Schlagwortfarben, bis der Client neu gestartet wird.
//!
//! Vorher wechselte die Farbe mit jedem Bildschirm. Das war unruhig und
//! machte aus einer Eigenschaft der Sitzung eine Eigenschaft des
//! Augenblicks: Zwei Bildschirme desselben Vorgangs sahen aus, als
//! gehörten sie nicht zusammen. Ein Schema je Sitzung gibt dem Client ein
//! Aussehen und behält den Reiz, dass es beim nächsten Start ein anderes
//! ist.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use crossterm::style::Color;

/// Die Palette: grelle Töne aus der 256-Farben-Tabelle.
///
/// Ausgesucht nach zwei Bedingungen: hell genug, um auf dunklem Grund zu
/// leuchten, und untereinander unterscheidbar. Dunkelblau und Braun sind
/// deshalb nicht dabei: Sie sind auf einem schwarzen Terminal kaum zu
/// lesen, und ein Menüpunkt, den man nicht entziffert, ist keiner.
pub const NEON: [u8; 18] = [
    51,  // Cyan
    45,  // Himmelblau
    87,  // Eisblau
    46,  // Grün
    82,  // Giftgrün
    118, // Limette
    154, // Hellgrün
    190, // Gelbgrün
    226, // Gelb
    220, // Goldgelb
    214, // Orange
    208, // Dunkelorange
    201, // Magenta
    207, // Rosa
    213, // Helles Pink
    165, // Violett
    129, // Purpur
    99,  // Lavendel
];

/// Mindestabstand einer Schlagwortfarbe zur Logofarbe, in Grad.
///
/// Darunter wären die beiden kaum zu unterscheiden, und der Menüpunkt
/// verschwände optisch im Schriftzug darüber. 25 Grad sind rund vier
/// Schritte der Palette.
const MIN_ZU_LOGO: i32 = 25;
/// Höchstabstand zur Logofarbe.
///
/// Darüber liegt eine Farbe nicht mehr in der Nachbarschaft, sondern
/// gegenüber, und das Bild fiele auseinander.
const MAX_ZU_LOGO: i32 = 110;
/// Mindestabstand der beiden Schlagwortfarben untereinander.
///
/// Sie sollen sich abwechseln und dabei erkennbar verschieden sein; zwei
/// benachbarte Grüntöne wären ein Wechsel, den niemand bemerkt.
const MIN_UNTEREINANDER: i32 = 40;

/// Das Farbschema einer Sitzung.
struct Sitzung {
    logo: usize,
    /// Die beiden Schlagwortfarben, als Indizes in [`NEON`].
    a: usize,
    b: usize,
}

/// Einmal je Programmlauf gewürfelt.
///
/// `OnceLock` und nicht ein Zufall je Aufruf: Genau das ist „ein Schema je
/// Sitzung". Der erste Zugriff geschieht beim Start, während die Animation
/// läuft.
static SITZUNG: OnceLock<Sitzung> = OnceLock::new();
/// Zähler für den Wechsel zwischen den beiden Schlagwortfarben.
static WECHSEL: AtomicUsize = AtomicUsize::new(0);

fn sitzung() -> &'static Sitzung {
    SITZUNG.get_or_init(|| {
        let logo = crate::animation::Zufall::neu().bis(NEON.len());
        let (a, b) = paar(logo);
        Sitzung { logo, a, b }
    })
}

/// Die Logofarbe dieser Sitzung.
pub fn logo() -> Color {
    Color::AnsiValue(NEON[sitzung().logo])
}

/// Die Farbe eines Schlagworts: abwechselnd die beiden Sitzungsfarben.
///
/// **Abwechselnd, nicht gemischt:** Bei zwei Farben ist der Wechsel die
/// einzige Verteilung, in der nie zwei gleiche nebeneinander stehen.
pub fn schlagwort() -> Color {
    let s = sitzung();
    let dran = if WECHSEL.fetch_add(1, Ordering::Relaxed) % 2 == 0 {
        s.a
    } else {
        s.b
    };
    Color::AnsiValue(NEON[dran])
}

/// Die beiden Schlagwortfarben zu einer Logofarbe.
///
/// **Nicht die nächstliegenden.** Die erste Fassung nahm die beiden
/// Nachbarn im Farbton, und die lagen der Logofarbe zu nahe: Ein Menütitel
/// in fast der Farbe des Schriftzugs darüber hebt sich nicht ab, und zwei
/// benachbarte Töne unterscheiden sich untereinander erst recht nicht.
///
/// Gesucht ist deshalb ein **Paar in einem Band** um die Logofarbe:
/// weit genug weg, um sich abzuheben ([`MIN_ZU_LOGO`]), nah genug, um
/// dazuzugehören ([`MAX_ZU_LOGO`]), und untereinander weit genug
/// auseinander, damit der Wechsel sichtbar ist ([`MIN_UNTEREINANDER`]).
/// Unter allen Paaren, die das erfüllen, gewinnt das mit dem kleinsten
/// Gesamtabstand zur Logofarbe: so nah am Logo, wie die Bedingungen
/// zulassen.
///
/// Gerechnet wird über den **Farbkreis**, nicht über die Reihenfolge in
/// der Palette. Die steht zwar ungefähr nach Spektrum, aber zwischen
/// Orange und Magenta fehlt das Rot, und ein Nachbar im Feld wäre dort ein
/// Sprung im Bild.
fn paar(logo: usize) -> (usize, usize) {
    let eigen = farbton(NEON[logo]);
    let im_band: Vec<usize> = (0..NEON.len())
        .filter(|i| {
            let d = abstand(eigen, farbton(NEON[*i]));
            *i != logo && (MIN_ZU_LOGO..=MAX_ZU_LOGO).contains(&d)
        })
        .collect();

    let mut beste: Option<(i32, usize, usize)> = None;
    for (n, &a) in im_band.iter().enumerate() {
        for &b in &im_band[n + 1..] {
            if abstand(farbton(NEON[a]), farbton(NEON[b])) < MIN_UNTEREINANDER {
                continue;
            }
            let summe = abstand(eigen, farbton(NEON[a])) + abstand(eigen, farbton(NEON[b]));
            if beste.is_none_or(|(s, _, _)| summe < s) {
                beste = Some((summe, a, b));
            }
        }
    }

    // Für die heutige Palette gibt es zu jeder Logofarbe ein Paar; der
    // Rückfall steht da, damit eine geänderte Palette nicht in eine Panik
    // läuft, sondern schlimmstenfalls schlechter aussieht.
    match beste {
        Some((_, a, b)) => (a, b),
        None => nachbarn(logo),
    }
}

/// Die beiden Paletteneinträge, die `index` im Farbton am nächsten liegen.
///
/// **Gerechnet aus dem Farbton, nicht aus der Reihenfolge im Feld.** Die
/// Palette steht zwar ungefähr nach Spektrum sortiert, aber „ungefähr"
/// genügt hier nicht: Zwischen Orange und Magenta fehlt das Rot, und ein
/// Nachbar im Feld wäre dort ein Sprung im Bild. Wird die Palette einmal
/// umsortiert oder ergänzt, bleibt diese Rechnung richtig.
///
/// Der Farbton ist ein **Kreis**: Lavendel liegt neben Purpur und ebenso
/// neben Cyan. Deshalb der kürzere der beiden Wege über 360 Grad.
fn nachbarn(index: usize) -> (usize, usize) {
    let eigen = farbton(NEON[index]);
    let mut andere: Vec<(i32, usize)> = (0..NEON.len())
        .filter(|i| *i != index)
        .map(|i| (abstand(eigen, farbton(NEON[i])), i))
        .collect();
    // Nach Abstand, bei Gleichstand nach Index: Die Auswahl soll bei
    // gleicher Palette immer dieselbe sein.
    andere.sort_unstable();
    (andere[0].1, andere[1].1)
}

/// Kürzerer Weg zwischen zwei Farbtönen auf dem Kreis.
fn abstand(a: i32, b: i32) -> i32 {
    let d = (a - b).abs();
    d.min(360 - d)
}

/// Der Farbton eines 256-Farben-Index in Grad.
///
/// Die Einträge 16 bis 231 bilden einen 6×6×6-Würfel; die Kanalwerte sind
/// 0 für die unterste Stufe und sonst `55 + 40 · Stufe`. Daraus der
/// übliche Farbton aus Maximum, Minimum und ihrer Differenz, ganzzahlig
/// gerechnet wie alles hier.
fn farbton(index: u8) -> i32 {
    let i = index as i32 - 16;
    let stufe = |s: i32| if s == 0 { 0 } else { 55 + 40 * s };
    let (r, g, b) = (stufe(i / 36), stufe((i / 6) % 6), stufe(i % 6));

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
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

/// Der Ton der Netzkanten.
///
/// **Zurückgenommen, aber lesbar.** Die erste Fassung nahm 240, ein
/// dunkles Grau: auf einem schwarzen Terminal war der Schriftzug damit
/// hell und das Netz praktisch unsichtbar, und übrig blieb ein Schriftzug
/// ohne das Motiv, um das es geht. 248 ist deutlich heller als der
/// Hintergrund und bleibt trotzdem hinter den Knoten zurück.
pub const KANTE: Color = Color::AnsiValue(248);

/// Der gedämpfte Ton für Hinweiszeilen unter den Menütiteln.
///
/// Dunkler als die Netzkanten: Hier ist Zurücknehmen der Zweck, denn der
/// Titel darüber soll zuerst gelesen werden.
pub const BEIWERK: Color = Color::AnsiValue(244);

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Farbton ist ein Kreis: Der letzte Eintrag der Palette liegt
    /// ebenso neben dem ersten wie neben seinem Vorgänger. Ohne diese
    /// Rechnung bekäme Lavendel nur Nachbarn auf einer Seite.
    #[test]
    fn der_farbkreis_schliesst_sich() {
        assert_eq!(abstand(350, 10), 20, "über 360 Grad hinweg falsch gerechnet");
        assert_eq!(abstand(10, 350), 20);
        assert_eq!(abstand(0, 180), 180, "der weiteste Abstand ist ein halber Kreis");
        // Cyan (51) ist reines Cyan, Gelb (226) reines Gelb.
        assert_eq!(farbton(51), 180, "Cyan liegt nicht bei 180 Grad");
        assert_eq!(farbton(226), 60, "Gelb liegt nicht bei 60 Grad");
        assert_eq!(farbton(201), 300, "Magenta liegt nicht bei 300 Grad");
    }

    /// **Der Grund für die Umstellung.** Die erste Fassung nahm die beiden
    /// nächstliegenden Töne. Die lagen der Logofarbe zu nahe: Ein Menütitel
    /// in fast der Farbe des Schriftzugs hebt sich nicht ab, und zwei
    /// benachbarte Töne unterscheiden sich untereinander erst recht nicht.
    /// Beide Abstände müssen deshalb für **jede** Logofarbe eingehalten
    /// sein.
    #[test]
    fn das_paar_haelt_beide_abstaende_ein() {
        for (logo, ton) in NEON.iter().enumerate() {
            let (a, b) = paar(logo);
            let eigen = farbton(*ton);
            let (ha, hb) = (farbton(NEON[a]), farbton(NEON[b]));

            for (i, h) in [(a, ha), (b, hb)] {
                assert_ne!(i, logo, "Logofarbe {ton} wählt sich selbst");
                let d = abstand(eigen, h);
                assert!(
                    d >= MIN_ZU_LOGO,
                    "Logofarbe {ton}: {} liegt nur {d}° entfernt",
                    NEON[i]
                );
                assert!(
                    d <= MAX_ZU_LOGO,
                    "Logofarbe {ton}: {} liegt {d}° entfernt, zu weit",
                    NEON[i]
                );
            }
            assert!(
                abstand(ha, hb) >= MIN_UNTEREINANDER,
                "Logofarbe {ton}: die beiden Schlagwortfarben liegen nur {}° auseinander",
                abstand(ha, hb)
            );
        }
    }

    /// Unter allen zulässigen Paaren gewinnt das mit dem kleinsten
    /// Gesamtabstand: so nah am Logo, wie die Bedingungen zulassen.
    #[test]
    fn das_paar_liegt_so_nah_am_logo_wie_moeglich() {
        for (logo, ton) in NEON.iter().enumerate() {
            let eigen = farbton(*ton);
            let (a, b) = paar(logo);
            let gewaehlt = abstand(eigen, farbton(NEON[a])) + abstand(eigen, farbton(NEON[b]));

            for (i, toni) in NEON.iter().enumerate() {
                for (k, tonk) in NEON.iter().enumerate().skip(i + 1) {
                    if i == logo || k == logo {
                        continue;
                    }
                    let (hi, hk) = (farbton(*toni), farbton(*tonk));
                    let (di, dk) = (abstand(eigen, hi), abstand(eigen, hk));
                    let zulaessig = (MIN_ZU_LOGO..=MAX_ZU_LOGO).contains(&di)
                        && (MIN_ZU_LOGO..=MAX_ZU_LOGO).contains(&dk)
                        && abstand(hi, hk) >= MIN_UNTEREINANDER;
                    if zulaessig {
                        assert!(
                            di + dk >= gewaehlt,
                            "Logofarbe {ton}: ein näheres Paar wurde übergangen"
                        );
                    }
                }
            }
        }
    }

    /// Die Schlagworte wechseln zwischen genau zwei Farben, und nie stehen
    /// zwei gleiche nebeneinander.
    #[test]
    fn schlagworte_wechseln_zwischen_den_beiden_sitzungsfarben() {
        // Über die Sitzung, so wie der Client sie benutzt.
        let gezogen: Vec<Color> = (0..8).map(|_| schlagwort()).collect();
        let verschieden: std::collections::BTreeSet<_> = gezogen
            .iter()
            .map(|f| match f {
                Color::AnsiValue(v) => *v,
                _ => 0,
            })
            .collect();
        assert_eq!(verschieden.len(), 2, "es sind nicht genau zwei Farben");
        assert!(!verschieden.contains(&match logo() {
            Color::AnsiValue(v) => v,
            _ => 0,
        }));
    }

    /// Das Schema gilt für die ganze Sitzung: Zwei Abrufe der Logofarbe
    /// müssen dieselbe liefern. Vorher wechselte sie mit jedem Bildschirm,
    /// und zwei Bildschirme desselben Vorgangs sahen aus, als gehörten sie
    /// nicht zusammen.
    #[test]
    fn das_schema_bleibt_ueber_die_sitzung_gleich() {
        let erste = logo();
        for _ in 0..50 {
            assert_eq!(logo(), erste, "die Logofarbe hat gewechselt");
        }
        let s = sitzung();
        assert_eq!((s.a, s.b), paar(s.logo), "die Sitzungsfarben passen nicht zum Logo");
    }

    /// Die Palette darf keine Farbe doppelt führen: Zwei gleiche Einträge
    /// erschienen als Wiederholung, obwohl der Index wechselte.
    #[test]
    fn palette_ist_ohne_dubletten() {
        let mut sortiert = NEON;
        sortiert.sort_unstable();
        let vorher = sortiert.len();
        let mut ohne = sortiert.to_vec();
        ohne.dedup();
        assert_eq!(ohne.len(), vorher, "Farbe doppelt in der Palette");
    }
}
