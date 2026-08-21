//! Neonfarben für Schriftzug und Auswahllisten.
//!
//! ## Warum 256-Farben-Indizes und kein RGB
//!
//! `Color::Rgb` verlangt ein Terminal mit Echtfarben. Verbreitet ist das,
//! aber nicht selbstverständlich: über SSH, in `screen`, in einer
//! seriellen Konsole und in mancher CI fällt es auf eine Näherung zurück
//! — und welche, entscheidet das Terminal, nicht dieses Programm. Die
//! 256-Farben-Palette gibt es seit Jahrzehnten überall, und ihre oberen
//! Bereiche sind genau die grellen Töne, um die es hier geht.
//!
//! ## Was hier zufällig sein darf und was nicht
//!
//! Farbe ist Schmuck. Sie steht **nie** für eine Aussage: Kein Urteil,
//! kein Fehler und kein Vergleichswert wird an einer Farbe erkennbar
//! gemacht — dafür stehen Wörter da (`ABWEICHUNG`, `FEHLER`, `NACHWEIS`).
//! Wer nur Graustufen sieht, sei es aus Farbenblindheit, sei es in einem
//! Protokollmitschnitt, verliert damit keine Information.
//!
//! Deshalb ist der Zufall hier unbedenklich, während er im Rechenpfad
//! dieses Projekts nirgends etwas zu suchen hat.
//!
//! ## Warum die letzten vier gemerkt werden
//!
//! Bei achtzehn Farben zieht ein freier Zufall im Schnitt jede achtzehnte
//! Wahl dieselbe wie zuvor — und wer das sieht, hält es für einen Fehler,
//! nicht für Zufall. Eine einzige gemerkte Farbe genügt aber nicht: Ein
//! Menü mit sechs Punkten holt sechs Farben nacheinander, und zwei
//! gleiche zwei Zeilen auseinander fallen genauso auf. Gemerkt werden
//! deshalb die letzten vier; innerhalb einer Menüseite sind damit die
//! sichtbar benachbarten Punkte verschieden.

use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::style::Color;

/// Die Palette: grelle Töne aus der 256-Farben-Tabelle.
///
/// Ausgesucht nach zwei Bedingungen — hell genug, um auf dunklem Grund zu
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

/// Wie viele der zuletzt vergebenen Farben gemieden werden.
const GEMERKT: usize = 4;
/// Bits je gemerktem Index. Fünf reichen für achtzehn Farben und lassen
/// Werte übrig (18–31), die keinem Index entsprechen — den Anfangszustand.
const BITS: usize = 5;
const MASKE: usize = (1 << BITS) - 1;
/// Anfangswert: viermal ein Wert, der kein gültiger Index ist.
const LEER: usize = {
    let mut r = 0;
    let mut n = 0;
    while n < GEMERKT {
        r = (r << BITS) | MASKE;
        n += 1;
    }
    r
};

/// Die zuletzt vergebenen Farben, als Ringpuffer in einer Zahl.
///
/// Vier Fünf-Bit-Felder statt einer Sperre um eine Warteschlange: Der Wert
/// wird beim Zeichnen gelesen und geschrieben, oft aus mehreren Stellen
/// kurz hintereinander, und eine Sperre für einen Farbwunsch wäre
/// Aufwand ohne Gegenwert.
static LETZTE: AtomicUsize = AtomicUsize::new(LEER);

/// Eine Neonfarbe, verschieden von der zuletzt vergebenen.
pub fn naechste() -> Color {
    Color::AnsiValue(NEON[naechster_index()])
}

/// Der Index dazu, mit Fortschreibung des Ringpuffers.
fn naechster_index() -> usize {
    let mut z = crate::animation::Zufall::neu();
    let ring = LETZTE.load(Ordering::Relaxed);
    let i = gezogen(ring, &mut z);
    LETZTE.store(eingereiht(ring, i), Ordering::Relaxed);
    i
}

/// Steht dieser Index noch im Ringpuffer?
fn enthalten(ring: usize, i: usize) -> bool {
    (0..GEMERKT).any(|feld| (ring >> (feld * BITS)) & MASKE == i)
}

/// Schiebt `i` in den Ringpuffer und wirft das älteste Feld heraus.
fn eingereiht(ring: usize, i: usize) -> usize {
    ((ring << BITS) | i) & ((1 << (GEMERKT * BITS)) - 1)
}

/// Zieht einen Index, der nicht im Ringpuffer steht.
///
/// Ohne die Merkzelle geschrieben, damit die Regel für sich prüfbar ist:
/// `naechster_index` teilt seinen Zustand mit allem, was gleichzeitig
/// zeichnet, und ein Test darüber prüfte die Verschränkung statt der
/// Auswahl.
///
/// Die Zahl der Versuche ist begrenzt. Sie reicht bei achtzehn Farben und
/// vier gemerkten immer aus; die Grenze steht da, damit aus einer
/// geänderten Palette keine Endlosschleife werden kann, sondern
/// schlimmstenfalls eine wiederholte Farbe.
fn gezogen(ring: usize, z: &mut crate::animation::Zufall) -> usize {
    let mut i = z.bis(NEON.len());
    for _ in 0..NEON.len() {
        if !enthalten(ring, i) {
            return i;
        }
        // Um einen zufälligen Betrag weiterrücken statt um genau eins:
        // Sonst folgte auf eine Wiederholung immer dieselbe Nachbarfarbe,
        // und das Muster wäre nach kurzem Zusehen sichtbar.
        i = (i + 1 + z.bis(NEON.len() - 1)) % NEON.len();
    }
    i
}

/// Der gedämpfte Ton für alles, was Beiwerk ist: Netzmotiv, Hinweiszeilen.
///
/// Ein zweiter Neonton daneben würde mit dem ersten um Aufmerksamkeit
/// streiten; das Netz ist Hintergrund und soll auch so aussehen.
pub const BEIWERK: Color = Color::AnsiValue(240);

#[cfg(test)]
mod tests {
    use super::*;

    /// Innerhalb von vier aufeinanderfolgenden Ziehungen darf sich keine
    /// Farbe wiederholen — sonst stünden in einem Menü zwei gleichfarbige
    /// Punkte sichtbar nebeneinander.
    #[test]
    fn keine_wiederholung_in_vier_ziehungen() {
        let mut z = crate::animation::Zufall::neu();
        let mut ring = LEER;
        let mut letzte: Vec<usize> = Vec::new();
        for runde in 0..3000 {
            let i = gezogen(ring, &mut z);
            assert!(
                !letzte.contains(&i),
                "Farbe {i} wiederholt sich in Runde {runde} unter den letzten {}",
                letzte.len()
            );
            ring = eingereiht(ring, i);
            letzte.push(i);
            if letzte.len() > GEMERKT {
                letzte.remove(0);
            }
        }
    }

    /// Der Anfangszustand darf keine gültige Farbe sperren, sonst fehlten
    /// beim ersten Menü vier Töne der Palette.
    #[test]
    fn leerer_ring_sperrt_nichts() {
        for i in 0..NEON.len() {
            assert!(!enthalten(LEER, i), "Index {i} ist im leeren Ring gesperrt");
        }
    }

    /// Der Index muss in der Palette liegen; ein Fehlgriff wäre ein Absturz
    /// beim Zeichnen.
    #[test]
    fn index_bleibt_in_der_palette() {
        let mut z = crate::animation::Zufall::neu();
        let mut ring = LEER;
        for _ in 0..2000 {
            let i = gezogen(ring, &mut z);
            assert!(i < NEON.len());
            ring = eingereiht(ring, i);
        }
        // Auch der Weg über die Merkzelle muss tragen.
        assert!(naechster_index() < NEON.len());
    }

    /// Über viele Ziehungen soll die ganze Palette vorkommen. Bliebe ein
    /// Teil ungenutzt, wäre der Zufall in Wahrheit keiner.
    #[test]
    fn die_palette_wird_ausgeschoepft() {
        let mut z = crate::animation::Zufall::neu();
        let mut gesehen = [false; NEON.len()];
        let mut ring = LEER;
        for _ in 0..4000 {
            let i = gezogen(ring, &mut z);
            gesehen[i] = true;
            ring = eingereiht(ring, i);
        }
        assert!(gesehen.iter().all(|g| *g), "nicht alle Farben kamen vor");
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
