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
}
