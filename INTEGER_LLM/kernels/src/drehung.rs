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
    // ⚑ **Der uebliche Fall in i32 und ohne Sprung** (2026-09-30): Die
    //   Summen liegen unter 2^25 (Modulkopf), ein Rechtsshift von 1 bis 30
    //   rundet also in i32 nach derselben Regel wie `rshift_round_i64`. Die
    //   Schleife ist damit fuer den Uebersetzer eine Vektorschleife; bei
    //   17 408 Werten vor jedem down stand sie im Decode des 27B mit der
    //   Transformation bei vier Prozent. Alles andere wie bisher in i64.
    if (1..=30).contains(&shift) {
        let s = shift as u32;
        let (maske, halb) = ((1i32 << s) - 1, 1i32 << (s - 1));
        return t
            .into_iter()
            .map(|v| {
                let quotient = v >> s;
                let rest = v & maske;
                let auf = (rest > halb) | ((rest == halb) & (quotient & 1 != 0));
                (quotient + i32::from(auf)).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
            })
            .collect();
    }
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
/// ⚑ **Die ersten drei Stufen je Achterstueck, die uebrigen als zwei
/// Haelften eines Abschnitts.** Mit Schrittweite 1, 2 und 4 sind die
/// Haelften kuerzer als ein Vektor, und eine Schleife darueber bleibt
/// skalar; ausgeschrieben sind es je Achterstueck dieselben 24 Summen und
/// Differenzen in derselben Reihenfolge. Ab Schrittweite 8 laeuft
/// `(a, b) -> (a + b, a - b)` ueber zusammenhaengende Stuecke, und der
/// Uebersetzer vektorisiert die innere Schleife.
fn walsh_hadamard(block: &mut [i32]) {
    let n = block.len();
    if n % 8 != 0 {
        return walsh_hadamard_ab(block, 1);
    }
    for a in block.chunks_exact_mut(8) {
        // Schrittweite 1.
        let (b0, b1, b2, b3) = (a[0] + a[1], a[0] - a[1], a[2] + a[3], a[2] - a[3]);
        let (b4, b5, b6, b7) = (a[4] + a[5], a[4] - a[5], a[6] + a[7], a[6] - a[7]);
        // Schrittweite 2.
        let (c0, c1, c2, c3) = (b0 + b2, b1 + b3, b0 - b2, b1 - b3);
        let (c4, c5, c6, c7) = (b4 + b6, b5 + b7, b4 - b6, b5 - b7);
        // Schrittweite 4.
        a[0] = c0 + c4;
        a[1] = c1 + c5;
        a[2] = c2 + c6;
        a[3] = c3 + c7;
        a[4] = c0 - c4;
        a[5] = c1 - c5;
        a[6] = c2 - c6;
        a[7] = c3 - c7;
    }
    walsh_hadamard_ab(block, 8);
}

/// Die Stufen ab Schrittweite `h`, jede als zwei Haelften eines Abschnitts.
fn walsh_hadamard_ab(block: &mut [i32], mut h: usize) {
    let n = block.len();
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

/// **Die Eingangsdrehung einer Projektion**, wie das Training sie sieht:
/// die Vorzeichen und die Skala, auf der die gedrehte Eingabe liegt.
///
/// Dieselbe Angabe wie in der Laufzeit, nur geliehen statt gehalten: Die
/// Kerne des Trainings nehmen Scheiben und besitzen nichts.
#[derive(Debug, Clone, Copy)]
pub struct Eingangsdrehung<'a> {
    /// +1 oder -1 je Eingang.
    pub vorzeichen: &'a [i8],
    /// Bruchstellen der gedrehten Eingabe.
    pub frac: u8,
}

impl Eingangsdrehung<'_> {
    /// Dreht `x` von der Skala `x_frac` auf [`Self::frac`].
    pub fn drehen(&self, x: &[i16], x_frac: u8) -> Vec<i16> {
        drehen(x, x_frac, self.vorzeichen, self.frac)
    }

    /// Dieselbe Drehung: gleiche Skala und gleiche Vorzeichen, zuerst am
    /// Zeiger erkannt, dann am Inhalt.
    pub fn gleich(&self, andere: &Eingangsdrehung<'_>) -> bool {
        self.frac == andere.frac
            && (std::ptr::eq(self.vorzeichen, andere.vorzeichen) || self.vorzeichen == andere.vorzeichen)
    }
}

/// Dieselbe Drehung auf beiden Seiten, oder auf beiden keine.
pub fn gleiche_drehung(a: Option<Eingangsdrehung<'_>>, b: Option<Eingangsdrehung<'_>>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.gleich(&y),
        (None, None) => true,
        _ => false,
    }
}

/// **Die Eingabe, wie eine Projektion sie braucht**: gedreht, wenn sie eine
/// Drehung traegt, sonst `x` selbst; dazu die Skala, auf der sie liegt.
pub fn eingang<'x>(
    d: Option<Eingangsdrehung<'_>>,
    x: &'x [i16],
    x_frac: u8,
) -> (std::borrow::Cow<'x, [i16]>, u8) {
    match d {
        Some(d) => (std::borrow::Cow::Owned(d.drehen(x, x_frac)), d.frac),
        None => (std::borrow::Cow::Borrowed(x), x_frac),
    }
}

/// **Der Rueckweg der Drehung**: aus `dL/dy` nach der gedrehten Eingabe
/// wird `dL/dx` nach der ungedrehten, auf derselben Skala.
///
/// Vorwaerts ist `y = (H / 32) * (s ⊙ x)`. `H / 32` ist symmetrisch und
/// orthogonal, die Ableitung also
///
/// ```text
/// dL/dx = s ⊙ ((H / 32) * dL/dy)
/// ```
///
/// dieselbe Transformation, mit den Vorzeichen **danach** statt davor.
/// `H * g` ist exakt in i64 (1024 Summanden), die Teilung durch 32 ein
/// gerundeter Rechtsshift um fuenf. ⚑ **Die Rundung des Vorwaertspasses
/// geht nicht ein**: Sie ist eine Stufe, und durch eine Stufe reicht der
/// Gradient wie ueberall im Rechenpfad gerade durch.
///
/// Gerechnet wird in i64 und ohne Saettigung: Der Aufrufer summiert oft
/// erst mehrere Beitraege und saettigt dann einmal.
pub fn gradient_zurueckdrehen(g: &[i64], vorzeichen: &[i8]) -> Vec<i64> {
    assert_eq!(g.len(), vorzeichen.len(), "gradient_zurueckdrehen: {} Werte, {} Vorzeichen", g.len(), vorzeichen.len());
    assert!(g.len() % BLOCK == 0, "gradient_zurueckdrehen: {} Werte sind kein Vielfaches von {BLOCK}", g.len());
    let mut t = g.to_vec();
    for block in t.chunks_exact_mut(BLOCK) {
        let mut h = 1;
        while h < BLOCK {
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
    t.iter()
        .zip(vorzeichen)
        .map(|(&v, &s)| rshift_round_i64(v, (STUFEN / 2) as u8) * i64::from(s))
        .collect()
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

    /// **Die ausgeschriebenen ersten Stufen sind die Schleife**, fuer jede
    /// Blocklaenge, die eine Zweierpotenz ist, auch unter acht.
    #[test]
    fn die_ausgeschriebenen_stufen_sind_die_schleife() {
        for n in [1usize, 2, 4, 8, 16, 64, 1024] {
            let r = zufall(n, 3 + n as u64);
            let x: Vec<i32> = r.iter().map(|&v| (v % 65536) as i32 - 32768).collect();
            let (mut neu, mut alt) = (x.clone(), x);
            walsh_hadamard(&mut neu);
            walsh_hadamard_ab(&mut alt, 1);
            assert_eq!(neu, alt, "Blocklaenge {n}");
        }
    }

    /// **Der schnelle Ausgang ist der alte**: jede Verschiebung von -3 bis
    /// 30 und darueber, gegen die Rechnung in i64, mit Werten an den
    /// Haelften und an der Klemme.
    #[test]
    fn der_schnelle_ausgang_ist_der_alte() {
        let r = zufall(2 * BLOCK, 99);
        let mut x: Vec<i16> = r[..BLOCK].iter().map(|&v| v as u16 as i16).collect();
        x[..8].copy_from_slice(&[i16::MIN, i16::MAX, 1, -1, 0, 2, -2, 3]);
        let vz: Vec<i8> = r[BLOCK..].iter().map(|&v| if v & 1 == 0 { 1 } else { -1 }).collect();
        let mut t: Vec<i32> = x.iter().zip(&vz).map(|(&v, &s)| i32::from(v) * i32::from(s)).collect();
        walsh_hadamard_ab(&mut t, 1);
        for x_frac in 0u8..=30 {
            for ziel_frac in [0u8, 3, 8] {
                let shift = i32::from(x_frac) + 5 - i32::from(ziel_frac);
                let soll: Vec<i16> = t
                    .iter()
                    .map(|&v| {
                        let v = i64::from(v);
                        clamp_i16_from_i64(if shift >= 0 { rshift_round_i64(v, shift as u8) } else { v << (-shift) as u32 })
                    })
                    .collect();
                assert_eq!(drehen(&x, x_frac, &vz, ziel_frac), soll, "{x_frac} -> {ziel_frac}");
            }
        }
    }

    #[test]
    fn falsche_laengen_fallen_auf() {
        assert!(std::panic::catch_unwind(|| drehen(&[0; 100], 0, &[1; 100], 0)).is_err());
        assert!(std::panic::catch_unwind(|| drehen(&[0; 1024], 0, &[1; 1023], 0)).is_err());
    }

    /// **Der Rueckweg ist die Transponierte des Hinwegs.** Fuer jede
    /// lineare Abbildung `A` gilt `<g, A x> = <A^T g, x>`; hier auf ganzen
    /// Zahlen und **ohne** die beiden Rundungen geprueft (Hinweg mit
    /// Shift null, Rueckweg vor dem Shift um fuenf), also exakt.
    #[test]
    fn der_rueckweg_ist_die_transponierte() {
        let n = 2 * BLOCK;
        let x: Vec<i16> = zufall(n, 11).iter().map(|v| (*v % 2001) as i16 - 1000).collect();
        let s: Vec<i8> = zufall(n, 12).iter().map(|v| if v % 2 == 0 { 1 } else { -1 }).collect();
        let g: Vec<i64> = zufall(n, 13).iter().map(|v| (*v % 200_001) as i64 - 100_000).collect();
        // Hinweg ohne Rundung: H * (s ⊙ x).
        let mut hx: Vec<i32> = x.iter().zip(&s).map(|(a, b)| i32::from(*a) * i32::from(*b)).collect();
        for b in hx.chunks_exact_mut(BLOCK) {
            walsh_hadamard(b);
        }
        let links: i64 = g.iter().zip(&hx).map(|(a, b)| a * i64::from(*b)).sum();
        // Rueckweg: `gradient_zurueckdrehen` teilt durch 32. Mit einem
        // Gradienten, der das 32-fache ist, faellt die Rundung weg.
        let g32: Vec<i64> = g.iter().map(|v| v * 32).collect();
        let zurueck = gradient_zurueckdrehen(&g32, &s);
        let rechts: i64 = zurueck.iter().zip(&x).map(|(a, b)| a * i64::from(*b)).sum();
        assert_eq!(links, rechts, "Vorzeichen oder Reihenfolge stimmen nicht");
    }

    /// Hin und zurueck ist die Identitaet (die Drehung ist orthogonal), bis
    /// auf die Rundung der beiden Shifts.
    #[test]
    fn hin_und_zurueck_ist_die_identitaet() {
        let n = BLOCK;
        let s: Vec<i8> = zufall(n, 21).iter().map(|v| if v % 2 == 0 { 1 } else { -1 }).collect();
        let x: Vec<i16> = zufall(n, 22).iter().map(|v| (*v % 20_001) as i16 - 10_000).collect();
        let y = drehen(&x, 9, &s, 9);
        let y64: Vec<i64> = y.iter().map(|v| i64::from(*v)).collect();
        let z = gradient_zurueckdrehen(&y64, &s);
        for (a, b) in x.iter().zip(&z) {
            assert!((i64::from(*a) - b).abs() <= 8, "{a} gegen {b}");
        }
        // Gegenprobe: mit falschen Vorzeichen kommt etwas anderes zurueck.
        let falsch: Vec<i8> = s.iter().enumerate().map(|(i, v)| if i % 3 == 0 { -v } else { *v }).collect();
        let w = gradient_zurueckdrehen(&y64, &falsch);
        assert!(x.iter().zip(&w).any(|(a, b)| (i64::from(*a) - b).abs() > 1000));
    }

    #[test]
    fn gleiche_drehungen_werden_erkannt() {
        let (a, b) = (vec![1i8, -1, 1], vec![1i8, -1, 1]);
        let d = Eingangsdrehung { vorzeichen: &a, frac: 9 };
        assert!(d.gleich(&Eingangsdrehung { vorzeichen: &a, frac: 9 }));
        assert!(d.gleich(&Eingangsdrehung { vorzeichen: &b, frac: 9 }));
        assert!(!d.gleich(&Eingangsdrehung { vorzeichen: &a, frac: 8 }));
        assert!(!d.gleich(&Eingangsdrehung { vorzeichen: &[1, 1, 1], frac: 9 }));
        assert!(gleiche_drehung(None, None) && gleiche_drehung(Some(d), Some(d)) && !gleiche_drehung(Some(d), None));
        let x = [5i16, 6, 7];
        let (e, f) = eingang(None, &x, 4);
        assert_eq!((&*e, f), (&x[..], 4));
    }
}
