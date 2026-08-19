//! Fixed-Point-Helfer
//! 
//! Alle Operationen sind deterministisch und plattformunabhaengig.

/// Clamp auf i8-Bereich.
#[inline(always)]
pub fn clamp_i8(x: i32) -> i8 {
    if x < -128 { -128 }
    else if x > 127 { 127 }
    else { x as i8 }
}

/// Clamp auf i16-Bereich.
#[inline(always)]
pub fn clamp_i16(x: i32) -> i16 {
    if x < -32768 { -32768 }
    else if x > 32767 { 32767 }
    else { x as i16 }
}

/// Clamp auf i16-Bereich aus i64 (fuer Akkumulator-Zwischenwerte).
#[inline(always)]
pub fn clamp_i16_from_i64(x: i64) -> i16 {
    if x < -32768 { -32768 }
    else if x > 32767 { 32767 }
    else { x as i16 }
}

/// Clamp auf i32-Bereich (nützlich fuer i64-Zwischenwerte).
#[inline(always)]
pub fn clamp_i32(x: i64) -> i32 {
    if x < i32::MIN as i64 { i32::MIN }
    else if x > i32::MAX as i64 { i32::MAX }
    else { x as i32 }
}

/// Round-to-nearest-even Rechts-Shift.
#[inline(always)]
pub fn rshift_round(value: i32, shift: u8) -> i32 {
    if shift == 0 {
        return value;
    }
    let mask = (1i32 << shift) - 1;
    let half = 1i32 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Round-to-nearest-even Rechts-Shift (i64, fuer Zwischenprodukte).
#[inline(always)]
pub fn rshift_round_i64(value: i64, shift: u8) -> i64 {
    if shift == 0 {
        return value;
    }
    let mask = (1i64 << shift) - 1;
    let half = 1i64 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Q15-Reziproke der Wurzel: `round(2^15 / sqrt(head_dim))`.
///
/// **Fund 19 (2026-08-18).** Die Attention skaliert die Scores mit
/// `1/sqrt(head_dim)` (Fund 17). Umgesetzt war das als reiner Rechtsshift um
/// `log2(head_dim) / 2` — Ganzzahldivision. Das stimmt nur, wenn
/// `log2(head_dim)` **gerade** ist:
///
/// | head_dim | Shift | angewandt | korrekt   |
/// |----------|-------|-----------|-----------|
/// | 64  (2^6)| 3     | 0,125000  | 0,125000  |
/// | 128 (2^7)| 3     | 0,125000  | 0,088388  | <- Faktor sqrt(2) zu gross
/// | 256 (2^8)| 4     | 0,062500  | 0,062500  |
///
/// Qwen2.5-0.5B hat head_dim 64 und war deshalb zufaellig richtig; ab 1,5B
/// ist 128 der Normalfall. Bei 7B waren die Scores durchgaengig um sqrt(2)
/// zu gross und die Softmax entsprechend zu scharf — in jedem Kopf, jeder
/// Ebene, jeder Position.
///
/// Statt eines Shifts wird jetzt mit dieser Q15-Konstanten multipliziert.
/// **Fuer gerade Zweierpotenzen ist das Ergebnis bitgleich zum bisherigen
/// Verhalten**, weil der Multiplikator dann selbst eine Zweierpotenz ist
/// (head_dim 64 -> 4096 = 2^12) und `rshift_round_i64` unter einem
/// Zweierpotenz-Faktor dieselbe Rundung samt Tie-Break liefert. Die
/// Artefakte bestehender Modelle bleiben damit gueltig.
///
/// Vollstaendig ganzzahlig: keine `f64::sqrt`, sondern
/// `isqrt_round(2^30 / head_dim)`. Fuer head_dim 64 exakt 4096, fuer 128
/// gerundet 2896 (relativer Fehler 1,1e-4).
#[inline]
pub fn inv_sqrt_q15(head_dim: usize) -> i64 {
    assert!(head_dim > 0, "inv_sqrt_q15: head_dim muss positiv sein");
    // (2^15)^2 / head_dim = 2^30 / head_dim; die Wurzel daraus ist
    // 2^15 / sqrt(head_dim).
    isqrt_round((1u64 << 30) / head_dim as u64) as i64
}

/// Ganzzahlige Quadratwurzel mit Rundung zur naechsten ganzen Zahl.
///
/// Newton-Iteration auf u64, danach eine Rundungskorrektur: von den beiden
/// Kandidaten `r` und `r + 1` gewinnt der, dessen Quadrat naeher an `n`
/// liegt. Deterministisch und plattformunabhaengig — anders als
/// `(n as f64).sqrt().round()`, das je nach libm abweichen kann und damit
/// den Konsens brechen wuerde.
#[inline]
pub fn isqrt_round(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    // Abgerundete Ganzzahlwurzel per Newton. Die Abbruchbedingung muss
    // MONOTONIE pruefen, nicht Gleichheit zum Vorgaenger: Newton oszilliert
    // fuer manche n dauerhaft zwischen zwei benachbarten Werten (etwa n=8:
    // 2, 3, 2, 3, ...). Ein Vergleich "r != vorher" laeuft dann ewig.
    let mut r = n;
    let mut naechst = r.div_ceil(2);
    while naechst < r {
        r = naechst;
        naechst = (r + n / r) / 2;
    }
    // r ist jetzt floor(sqrt(n)); die beiden Schleifen fangen Randfaelle ab.
    while r > 0 && r > n / r {
        r -= 1;
    }
    while (r + 1) <= n / (r + 1) {
        r += 1;
    }
    // Runden: liegt n naeher an (r+1)^2 als an r^2?
    let unten = n - r * r;
    let oben = (r + 1) * (r + 1) - n;
    if oben < unten {
        r + 1
    } else {
        r
    }
}

/// Round-to-nearest-even Rechts-Shift in i128.
///
/// Gleiche Semantik wie `rshift_round_i64`, nur mit breiterem Typ: seit
/// Fund 24 laeuft die RMSNorm-Ausgabe ueber i128, weil der dortige
/// Linksshift in i64 haette ueberlaufen koennen (spec:
/// overflow.behavior = "explicit_clamp_only", wrap = false).
#[inline(always)]
pub fn rshift_round_i128(value: i128, shift: u32) -> i128 {
    if shift == 0 {
        return value;
    }
    let mask = (1i128 << shift) - 1;
    let half = 1i128 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Rescale: von in_frac Bits nach out_frac Bits.
#[inline(always)]
pub fn rescale(acc: i32, in_frac: u8, out_frac: u8) -> i32 {
    let shift = in_frac as i8 - out_frac as i8;
    if shift >= 0 {
        rshift_round(acc, shift as u8)
    } else {
        acc << (-shift)
    }
}

/// Rescale fuer i64-Zwischenprodukte (z. B. RMSNorm-Tripelprodukt).
#[inline(always)]
pub fn rescale_i64(acc: i64, in_frac: u8, out_frac: u8) -> i64 {
    let shift = in_frac as i8 - out_frac as i8;
    if shift >= 0 {
        rshift_round_i64(acc, shift as u8)
    } else {
        acc << (-shift)
    }
}

/// Overflow-sichere Multiplikation i8 * i8 -> i32.
#[inline(always)]
pub fn mul_i8_i32(a: i8, b: i8) -> i32 {
    (a as i32).wrapping_mul(b as i32)
}

/// Overflow-sichere Multiplikation i16 * i16 -> i64.
#[inline(always)]
pub fn mul_i16_i64(a: i16, b: i16) -> i64 {
    (a as i64).wrapping_mul(b as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rshift_round_basic() {
        assert_eq!(rshift_round(4, 1), 2);
        assert_eq!(rshift_round(3, 1), 2);
        assert_eq!(rshift_round(5, 1), 2);
        assert_eq!(rshift_round(7, 1), 4);
    }

    #[test]
    fn test_rshift_round_negative() {
        assert_eq!(rshift_round(-5, 1), -2);
        assert_eq!(rshift_round(-4, 1), -2);
        assert_eq!(rshift_round(-3, 1), -2);
    }

    #[test]
    fn test_rescale() {
        let acc = 127i32 * 127i32;
        assert_eq!(rescale(acc, 13, 6), 126);
    }

    #[test]
    fn test_clamp_i8() {
        assert_eq!(clamp_i8(200), 127);
        assert_eq!(clamp_i8(-200), -128);
        assert_eq!(clamp_i8(50), 50);
    }

    /// Fixierter Divisionssemantik-Vektor (Fahrplan-Punkt 12.24).
    ///
    /// Division durch Zweierpotenzen ist als arithmetischer Rechtsshift
    /// mit Round-to-nearest-even festgelegt (spec.json:
    /// `shift_semantics = arithmetic_right_shift`, `rounding.default =
    /// round_to_nearest_even`). Für negative Operanden unterscheiden sich
    /// arithmetischer Rechtsshift, Trunkation zur Null und Floor-Division
    /// — ohne diesen fixierten Vektor würde die Abweichung erst in
    /// Modellausgaben sichtbar. Dieser Vektor ist die maßgebliche
    /// Referenz; CI failt, wenn ein Backend davon abweicht.
    #[test]
    fn division_semantics_vector() {
        // (Wert, Shift, erwartetes Ergebnis) — Round-to-nearest-even,
        // arithmetischer Rechtsshift.
        let vector: &[(i32, u8, i32)] = &[
            // positive Werte
            (0, 3, 0),
            (1, 1, 0),        // 0,5 -> 0 (quotient 0 gerade)
            (2, 1, 1),
            (3, 1, 2),        // 1,5 -> 2 (quotient 1 ungerade)
            (4, 1, 2),
            (5, 1, 2),        // 2,5 -> 2 (quotient 2 gerade)
            (6, 1, 3),
            (7, 1, 4),        // 3,5 -> 4 (quotient 3 ungerade)
            (8, 3, 1),
            (12, 3, 2),       // 1,5 -> 2
            (1_000_000, 8, 3906), // 3906,25 -> 3906
            // negative Werte
            (-1, 1, 0),       // -0,5 -> 0 (round-to-even)
            (-2, 1, -1),
            (-3, 1, -2),      // -1,5 -> -2
            (-4, 1, -2),
            (-5, 1, -2),      // -2,5 -> -2 (round-to-even)
            (-6, 1, -3),
            (-7, 1, -4),      // -3,5 -> -4
            (-8, 3, -1),
            (-12, 3, -2),     // -1,5 -> -2 (round-to-even)
            (-1_000_000, 8, -3906), // -3906,25 -> -3906
        ];
        for &(value, shift, expected) in vector {
            assert_eq!(
                rshift_round(value, shift),
                expected,
                "Abweichung bei ({} >> {}): erwartet {}, erhalten {}",
                value,
                shift,
                expected,
                rshift_round(value, shift)
            );
            // i64-Variante muss identisch sein.
            assert_eq!(
                rshift_round_i64(value as i64, shift),
                expected as i64,
                "i64-Abweichung bei ({} >> {})",
                value,
                shift
            );
        }
    }

    /// Fixierter Überlauf-/Sättigungsvektor (Fahrplan-Punkt 12.25).
    ///
    /// Überlaufverhalten ist Sättigung (spec.json: `overflow =
    /// explicit_clamp_only`, kein Wrap). Dieser Vektor fixiert die
    /// Sättigungsgrenzen; CI failt bei Abweichung.
    #[test]
    fn overflow_saturation_vector() {
        // i8-Sättigung [-128, 127]
        assert_eq!(clamp_i8(i32::MAX), 127);
        assert_eq!(clamp_i8(i32::MIN), -128);
        assert_eq!(clamp_i8(127), 127);
        assert_eq!(clamp_i8(128), 127);
        assert_eq!(clamp_i8(-128), -128);
        assert_eq!(clamp_i8(-129), -128);
        // i16-Sättigung [-32768, 32767]
        assert_eq!(clamp_i16(i32::MAX), 32767);
        assert_eq!(clamp_i16(i32::MIN), -32768);
        assert_eq!(clamp_i16(32767), 32767);
        assert_eq!(clamp_i16(32768), 32767);
        assert_eq!(clamp_i16(-32768), -32768);
        assert_eq!(clamp_i16(-32769), -32768);
        // i16-aus-i64-Sättigung
        assert_eq!(clamp_i16_from_i64(i64::MAX), 32767);
        assert_eq!(clamp_i16_from_i64(i64::MIN), -32768);
        assert_eq!(clamp_i16_from_i64(32768), 32767);
        assert_eq!(clamp_i16_from_i64(-32769), -32768);
        // i32-Sättigung
        assert_eq!(clamp_i32(i64::MAX), i32::MAX);
        assert_eq!(clamp_i32(i64::MIN), i32::MIN);
        assert_eq!(clamp_i32(i32::MAX as i64 + 1), i32::MAX);
        assert_eq!(clamp_i32(i32::MIN as i64 - 1), i32::MIN);
        // Multiplikation ohne Überlauf (wrapping in i32/i64-Zielbreite).
        assert_eq!(mul_i8_i32(127, 127), 16129);
        assert_eq!(mul_i8_i32(-128, -128), 16384);
        assert_eq!(mul_i16_i64(32767, 32767), 1_073_676_289);
        assert_eq!(mul_i16_i64(-32768, -32768), 1_073_741_824);
    }

    #[test]
    fn test_inv_sqrt_q15_gerade_zweierpotenzen_sind_exakt() {
        // Fuer gerade log2(head_dim) ist 1/sqrt(head_dim) selbst eine
        // Zweierpotenz. Der Multiplikator muss dann exakt 2^(15 - log2/2)
        // sein - nur so bleibt das Ergebnis bitgleich zum frueheren
        // Shift-Verhalten und bestehende Artefakte gueltig.
        assert_eq!(inv_sqrt_q15(4), 1 << 14);    // 1/2
        assert_eq!(inv_sqrt_q15(16), 1 << 13);   // 1/4
        assert_eq!(inv_sqrt_q15(64), 1 << 12);   // 1/8   <- Qwen2.5-0.5B
        assert_eq!(inv_sqrt_q15(256), 1 << 11);  // 1/16
    }

    #[test]
    fn test_inv_sqrt_q15_ungerade_zweierpotenzen() {
        // **Fund 19.** Hier lag der Fehler: der alte Shift log2(hd)/2 war
        // Ganzzahldivision und ergab fuer head_dim 128 den Faktor 2^-3 =
        // 0,125 statt 1/sqrt(128) = 0,0884 - um sqrt(2) zu gross. head_dim
        // 128 ist der Normalfall ab Qwen2.5-1.5B; abgedeckt war er von
        // keinem Test, weil 0.5B mit head_dim 64 zufaellig richtig lag.
        assert_eq!(inv_sqrt_q15(128), 2896);  // round(32768 / sqrt(128))
        assert_eq!(inv_sqrt_q15(512), 1448);
        assert_eq!(inv_sqrt_q15(2), 23170);   // round(32768 / sqrt(2))

        // Der alte Shift-Wert waere 2^12 = 4096 gewesen - Faktor 1,414 zu gross.
        assert!(inv_sqrt_q15(128) < 4096);
        let verhaeltnis = 4096_f64 / inv_sqrt_q15(128) as f64;
        assert!((verhaeltnis - std::f64::consts::SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn test_q15_multiplikation_ist_bitgleich_zum_shift_bei_zweierpotenz() {
        // Die Zusicherung, auf der die Gueltigkeit bestehender Artefakte
        // beruht: ist der Multiplikator eine Zweierpotenz 2^m, liefert
        // rshift_round_i64(x * 2^m, n + m) exakt dasselbe wie
        // rshift_round_i64(x, n) - einschliesslich des
        // Round-to-nearest-even-Tie-Breaks.
        let m = 12u8; // inv_sqrt_q15(64) = 2^12
        for x in [-100_003i64, -8, -7, -1, 0, 1, 7, 8, 9, 100_003, 1 << 30] {
            for n in [1u8, 3, 5, 8, 16] {
                let ueber_shift = rshift_round_i64(x, n);
                let ueber_mult = rshift_round_i64(x * (1i64 << m), n + m);
                assert_eq!(ueber_shift, ueber_mult, "x={} n={}", x, n);
            }
        }
    }

    #[test]
    fn test_isqrt_round_deterministisch() {
        assert_eq!(isqrt_round(0), 0);
        assert_eq!(isqrt_round(1), 1);
        assert_eq!(isqrt_round(2), 1);   // 1.414 -> 1
        assert_eq!(isqrt_round(3), 2);   // 1.732 -> 2
        assert_eq!(isqrt_round(4), 2);
        assert_eq!(isqrt_round(8_388_608), 2896);  // 2^30 / 128
        assert_eq!(isqrt_round(16_777_216), 4096); // 2^30 / 64, exakt
        // Gegen die Gleitkomma-Referenz, aber ohne sie im Rechenpfad.
        for n in [5u64, 99, 1000, 123_456, 1 << 40] {
            let erwartet = (n as f64).sqrt().round() as u64;
            assert_eq!(isqrt_round(n), erwartet, "n={}", n);
        }
    }
}
