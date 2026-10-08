//! **Das Banner eines Moduls**, wenn es seinen Modus beginnt: die Zeilen,
//! die das Modul mitbringt, oben, wo sonst das Banner der Konsole steht.
//!
//! ⚑ **Die Animation** ([`zeigen`]): Jede Zelle der Schrift flackert erst
//! als Ziffer, gruen oder rot, und rastet zu ihrer eigenen Zeit in das
//! Zeichen der Schrift ein, grau im Verlauf; daneben verlischt ein loses
//! Rauschen aus Ziffern. Sie dauert, bis das Modul seinen Bereich unten
//! zum ersten Mal gefuellt hat ([`fortschritt`]). Ohne Terminal oder ohne
//! Farbe steht die Schrift gleich da.
//!
//! ⚑ Die Zeilen kommen vom Modul und sind gesaeubert (keine
//! Steuerzeichen); die Farben setzt die Konsole.

use crate::design;
use std::io::Write;

/// Ein kleiner Zufall ohne Abhaengigkeit (xorshift), fuer das Flackern.
struct Zufall(u64);

impl Zufall {
    fn neu() -> Self {
        let saat = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x9E37_79B9);
        Self(saat | 1)
    }
    fn zahl(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Ganze Zahl in `0..n`.
    fn bis(&mut self, n: u64) -> u64 {
        self.zahl() % n.max(1)
    }
}

/// Wie lange ein Bild steht, Millisekunden.
const BILD_MS: u64 = 70;
/// So lange dauert der Aufbau mindestens, so lange ist er geplant, und so
/// lange wartet er hoechstens auf den Bereich unten (Millisekunden).
const MINDESTENS_MS: u64 = 2_500;
const GEPLANT_MS: u64 = 3_200;
const HOECHSTENS_MS: u64 = 10_000;

/// **Der Fortschritt des Aufbaus** in Promille, aus der verstrichenen Zeit
/// und dem bisherigen Fortschritt. Ohne gefuellten Bereich unten hoechstens 900: Die
/// letzten Zellen rasten erst ein, wenn er gefuellt ist (oder nach
/// [`HOECHSTENS_MS`]). Danach zuegig zu Ende, aber nicht vor
/// [`MINDESTENS_MS`].
pub fn fortschritt(vorher: u64, verstrichen_ms: u64, unten_da: bool) -> u64 {
    let nach_plan = verstrichen_ms * 1000 / GEPLANT_MS;
    if unten_da || verstrichen_ms >= HOECHSTENS_MS {
        let frueheste = if verstrichen_ms >= MINDESTENS_MS { 1000 } else { verstrichen_ms * 1000 / MINDESTENS_MS };
        (vorher + 80).max(nach_plan.min(1000)).min(frueheste.max(vorher))
    } else {
        nach_plan.min(900).max(vorher)
    }
}

/// Der Grauton einer Spalte: von hell links nach dunkler rechts.
fn grau(spalte: usize, breite: usize) -> crossterm::style::Color {
    let w = (235 - (spalte * 100 / breite.max(1))) as u8;
    crossterm::style::Color::Rgb { r: w, g: w, b: w }
}

/// **Zeigt das Banner**, ab der aktuellen Zeile, mit Einzug wie das
/// Konsolenbanner. Danach steht der Wagen unter dem Banner. Ist das Fenster
/// zu schmal, steht nur `titel` da.
///
/// ⚑ **Es baut sich auf, waehrend das Modul seinen Bereich unten fuellt**:
/// `unten_da` sagt, ob das geschehen ist. Die Schrift ist **neutral grau**
/// im Verlauf, gruen und rot sind nur die Ziffern davor.
pub fn zeigen(d: myl_client::einstellungen::Konsolendesign, zeilen: &[String], titel: &str, unten_da: &dyn Fn() -> bool) {
    // Gleich breit, sonst stuende der Einzug schief.
    let breite = zeilen.iter().map(|z| z.chars().count()).max().unwrap_or(0);
    let zeilen: Vec<String> = zeilen.iter().map(|z| format!("{z}{}", " ".repeat(breite - z.chars().count()))).collect();
    let einzug = crate::banner::blockeinzug(breite);
    let rollen = design::rollen(d);
    let farbig = design::farbig();
    let terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let (fensterbreite, _) = crate::banner::fenstermasse();
    if (fensterbreite as usize) < breite + 2 {
        println!("  {}", rollen.ueberschrift.faerben(titel, farbig));
        println!();
        return;
    }
    let zellen: Vec<Vec<char>> = zeilen.iter().map(|z| z.chars().collect()).collect();
    let fertig_zeile = |z: &[char]| -> String {
        let mut s = einzug.clone();
        for (c, ch) in z.iter().enumerate() {
            if *ch == ' ' {
                s.push(' ');
            } else {
                s.push_str(&design::Stil::farbe(grau(c, breite)).faerben(&ch.to_string(), farbig));
            }
        }
        s
    };
    if !terminal || !farbig {
        for z in &zellen {
            println!("{}", fertig_zeile(z));
        }
        println!();
        return;
    }
    // Je Zelle die Schwelle (Promille des Fortschritts), ab der sie steht.
    let mut zufall = Zufall::neu();
    let schwelle: Vec<Vec<u64>> = zellen.iter().map(|z| z.iter().map(|_| 100 + zufall.bis(901)).collect()).collect();
    let gewinn = design::Stil::farbe(design::GEWINN);
    let verlust = design::Stil::farbe(design::VERLUST);
    let mut aus = std::io::stdout();
    for _ in 0..zellen.len() {
        let _ = writeln!(aus);
    }
    let _ = write!(aus, "\x1b[{}A", zellen.len());
    let beginn = std::time::Instant::now();
    let mut p = 0u64;
    loop {
        p = fortschritt(p, beginn.elapsed().as_millis() as u64, unten_da());
        let mut text = String::new();
        for (r, z) in zellen.iter().enumerate() {
            if p >= 1000 {
                text.push_str("\r\x1b[2K");
                text.push_str(&fertig_zeile(z));
                text.push('\n');
                continue;
            }
            text.push_str("\r\x1b[2K");
            text.push_str(&einzug);
            for (c, &ziel) in z.iter().enumerate() {
                if ziel != ' ' && p >= schwelle[r][c] {
                    text.push_str(&design::Stil::farbe(grau(c, breite)).faerben(&ziel.to_string(), true));
                } else if ziel != ' ' || zufall.bis(1000) > p + 300 {
                    // Flackern: eine Ziffer, gruen fuer positiv, rot fuer negativ;
                    // im Rauschen daneben seltener, je weiter der Aufbau.
                    let ziffer = char::from(b'0' + zufall.bis(10) as u8);
                    let stil = if zufall.bis(2) == 0 { &gewinn } else { &verlust };
                    text.push_str(&stil.faerben(&ziffer.to_string(), true));
                } else {
                    text.push(' ');
                }
            }
            text.push('\n');
        }
        let _ = write!(aus, "{text}");
        let _ = aus.flush();
        if p >= 1000 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(BILD_MS));
        let _ = write!(aus, "\x1b[{}A", zellen.len());
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aufbau_wartet_auf_den_bereich_unten() {
        // Ohne Bereich unten nie ueber 900, auch spaet nicht (vor der Hoechstzeit).
        assert_eq!(fortschritt(0, 1_000, false), 1_000 * 1000 / GEPLANT_MS);
        assert_eq!(fortschritt(0, 9_000, false), 900);
        // Mit ihm zuegig zu Ende, aber nicht vor der Mindestzeit.
        assert!(fortschritt(900, 1_000, true) < 1000, "vor der Mindestzeit nicht fertig");
        assert_eq!(fortschritt(900, MINDESTENS_MS, true), 980);
        assert_eq!(fortschritt(980, MINDESTENS_MS + 70, true), 1000);
        // Nach der Hoechstzeit auch ohne ihn zu Ende.
        assert_eq!(fortschritt(900, HOECHSTENS_MS, false), 1000);
        // Nie rueckwaerts.
        assert!(fortschritt(500, 10, false) >= 500);
    }
}
