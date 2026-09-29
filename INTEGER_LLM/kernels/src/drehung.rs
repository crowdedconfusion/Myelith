//! Die ganzzahlige Hadamard-Drehung einer Aktivierung.
//!
//! # Wozu
//!
//! Manche ternaeren Modelle speichern ihre Gewichte in einer gedrehten
//! Basis: `W_gespeichert = W * R` mit einer festen orthogonalen Matrix `R`.
//! Dann ist `W * x = W_gespeichert * (R^T * x)`, und vor der Matrix muss die
//! Eingabe gedreht werden. `R^T` ist hier blockweise: je 1024 Eingaenge erst
//! ein Vorzeichen (+1 oder -1 je Eingang), dann die normierte
//! Sylvester-Hadamard-Matrix `H / 32`.
//!
//! # ⚑ Warum das ganzzahlig exakt geht
//!
//! `H` hat nur Eintraege +1 und -1, also ist `H * (s ⊙ x)` eine Summe von
//! Vorzeichen mal ganzen Zahlen: exakt in i32, denn aus hoechstens 2^15 wird
//! nach zehn Stufen hoechstens 2^25. Die Normierung `1/32 = 2^-5` ist eine
//! Zweierpotenz und geht in **einen** gerundeten Rechtsshift auf die
//! kalibrierte Skala der gedrehten Eingabe. Das ist die einzige Rundung,
//! und sie ist die des ganzen Rechenpfads (`rshift_round_i64`, zur
//! naechsten geraden Zahl).
//!
//! # Die Reihenfolge ist die der Sylvester-Konstruktion
//!
//! `H_1 = [1]`, `H_2n = [[H_n, H_n], [H_n, -H_n]]`. Die schnelle
//! Walsh-Hadamard-Transformation mit Schrittweiten 1, 2, 4, ... liefert genau
//! diese Reihenfolge; die Probe vergleicht mit der ausmultiplizierten Matrix.

use crate::fixed_point::{clamp_i16_from_i64, rshift_round_i64};

/// Blockgroesse der Drehung.
pub const BLOCK: usize = 1024;

/// `log2(BLOCK)`; die Normierung ist `2^-(STUFEN/2)`.
const STUFEN: u32 = 10;

/// Dreht `x` (Skala `x_frac`) mit den Vorzeichen `vorzeichen` und gibt das
/// Ergebnis auf der Skala `ziel_frac` zurueck.
///
/// `y = rshift_round((H * (s ⊙ x)), x_frac + 5 - ziel_frac)`, je Block von
/// 1024, gesaettigt auf i16. Ist der Shift negativ, wird exakt nach links
/// geschoben (in i64), dann gesaettigt.
pub fn drehen(x: &[i16], x_frac: u8, vorzeichen: &[i8], ziel_frac: u8) -> Vec<i16> {
    assert_eq!(x.len(), vorzeichen.len(), "drehen: {} Werte, {} Vorzeichen", x.len(), vorzeichen.len());
    assert!(x.len() % BLOCK == 0, "drehen: {} Werte sind kein Vielfaches von {BLOCK}", x.len());
    let mut t: Vec<i32> = x
        .iter()
        .zip(vorzeichen)
        .map(|(&v, &s)| {
            debug_assert!(s == 1 || s == -1, "drehen: Vorzeichen {s}");
            i32::from(v) * i32::from(s)
        })
        .collect();
    for block in t.chunks_exact_mut(BLOCK) {
        walsh_hadamard(block);
    }
    let shift = i32::from(x_frac) + (STUFEN / 2) as i32 - i32::from(ziel_frac);
    t.into_iter()
        .map(|v| {
            let v = i64::from(v);
            let y = if shift >= 0 { rshift_round_i64(v, shift as u8) } else { v << (-shift) as u32 };
            clamp_i16_from_i64(y)
        })
        .collect()
}

/// Die unnormierte Walsh-Hadamard-Transformation eines Blocks, an Ort und
/// Stelle, exakt in i32.
///
/// ⚑ **Jede Stufe als zwei Haelften eines Abschnitts**, damit der Uebersetzer
/// die innere Schleife vektorisieren kann: `(a, b) -> (a + b, a - b)` ueber
/// zusammenhaengende Stuecke.
fn walsh_hadamard(block: &mut [i32]) {
    let n = block.len();
    let mut h = 1;
    while h < n {
        for abschnitt in block.chunks_exact_mut(2 * h) {
            let (a, b) = abschnitt.split_at_mut(h);
            for (p, q) in a.iter_mut().zip(b.iter_mut()) {
                let (u, v) = (*p, *q);
                *p = u + v;
                *q = u - v;
            }
        }
        h *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zufall(n: usize, saat: u64) -> Vec<u64> {
        let mut x = saat | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x
            })
            .collect()
    }

    /// `H[i][j]` der Sylvester-Konstruktion: `(-1)^popcount(i & j)`.
    fn h(i: usize, j: usize) -> i64 {
        if (i & j).count_ones() % 2 == 0 { 1 } else { -1 }
    }

    /// **Dieselbe Zahl wie die ausmultiplizierte Matrix**, mit Rundung,
    /// ueber mehrere Bloecke und an den Raendern des Wertebereichs.
    #[test]
    fn die_schnelle_drehung_ist_die_matrix() {
        let n = 2 * BLOCK;
        let r = zufall(2 * n, 17);
        let mut x: Vec<i16> = r[..n].iter().map(|&v| v as u16 as i16).collect();
        x[0] = i16::MIN;
        x[5] = i16::MAX;
        let vz: Vec<i8> = r[n..].iter().map(|&v| if v & 1 == 0 { 1 } else { -1 }).collect();
        for (x_frac, ziel_frac) in [(6u8, 6u8), (6, 2), (3, 9), (0, 0), (10, 1)] {
            let ist = drehen(&x, x_frac, &vz, ziel_frac);
            let shift = i32::from(x_frac) + 5 - i32::from(ziel_frac);
            for b in 0..2 {
                for i in [0usize, 1, 2, 511, 1000, 1023] {
                    let summe: i64 = (0..BLOCK)
                        .map(|j| h(i, j) * i64::from(x[b * BLOCK + j]) * i64::from(vz[b * BLOCK + j]))
                        .sum();
                    let soll = if shift >= 0 { rshift_round_i64(summe, shift as u8) } else { summe << (-shift) };
                    assert_eq!(ist[b * BLOCK + i], clamp_i16_from_i64(soll), "Block {b}, Zeile {i}, {x_frac}->{ziel_frac}");
                }
            }
        }
    }

    /// **Zweimal gedreht ist die Eingabe**, wenn nichts gerundet wird: `H`
    /// ist symmetrisch und orthogonal, `H * H = 1024 * Id`. Mit den
    /// Vorzeichen nach der zweiten Drehung (die Umkehrung) und einem Shift
    /// von 10 insgesamt kommt `x` exakt zurueck.
    #[test]
    fn hin_und_zurueck_ist_die_eingabe() {
        let r = zufall(2 * BLOCK, 5);
        // Kleine Werte, damit die erste Drehung ohne Rundung in i16 passt.
        let x: Vec<i16> = r[..BLOCK].iter().map(|&v| (v % 64) as i16 - 32).collect();
        let vz: Vec<i8> = r[BLOCK..].iter().map(|&v| if v & 1 == 0 { 1 } else { -1 }).collect();
        // Erste Drehung ohne Normierung: Zielskala x_frac + 5.
        let eins = drehen(&x, 0, &vz, 5);
        // Zweite Drehung mit Vorzeichen +1, dann die Vorzeichen: ergibt 32 * x
        // auf Skala 5, also x auf Skala 0 nach der Normierung.
        let plus = vec![1i8; BLOCK];
        let zwei = drehen(&eins, 5, &plus, 5);
        let zurueck: Vec<i16> = zwei.iter().zip(&vz).map(|(&v, &s)| v * i16::from(s) / 32).collect();
        assert_eq!(zurueck, x);
    }

    #[test]
    fn falsche_laengen_fallen_auf() {
        assert!(std::panic::catch_unwind(|| drehen(&[0; 100], 0, &[1; 100], 0)).is_err());
        assert!(std::panic::catch_unwind(|| drehen(&[0; 1024], 0, &[1; 1023], 0)).is_err());
    }
}
