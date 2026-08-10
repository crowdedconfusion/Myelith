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
}
