//! Integer-Mathematik (sqrt, rsqrt, LUT-Lookup)

use crate::fixed_point::{clamp_i16, rshift_round};

/// Integer-Quadratwurzel via binaerer Suche.
/// Berechnet floor(sqrt(x * 2^frac_bits)) rein integer.
#[inline]
pub fn sqrt_q(x: i32, frac_bits: u8) -> i32 {
    if x <= 0 { return 0; }
    let target = (x as i64) << (frac_bits as u32);

    let mut lo = 0i64;
    let mut hi = (target + 1).min(i32::MAX as i64);

    while lo < hi {
        let mid = (lo + hi + 1) >> 1;
        if mid > 0 && mid <= target / mid {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    lo as i32
}

/// Reziproke Quadratwurzel (1/sqrt(x)) als Fixed-Point.
/// Rein integer: (2^(2*frac_bits)) / sqrt(x * 2^frac_bits).
#[inline]
pub fn rsqrt_q(x: i32, frac_bits: u8) -> i32 {
    if x <= 0 {
        return 1 << frac_bits;
    }
    let s = sqrt_q(x, frac_bits);
    if s == 0 {
        return 1 << frac_bits;
    }
    let val = (1i64 << (2 * frac_bits as u32)) / (s as i64);
    clamp_i16(val as i32) as i32
}

/// LUT-Lookup mit Index-Berechnung und Clamping.
#[inline(always)]
pub fn lut_lookup(x: i16, lut: &[i16], shift: u8, offset: i16) -> i16 {
    let idx = (x >> shift) + offset;
    let idx = idx.max(0).min(lut.len() as i16 - 1) as usize;
    lut[idx]
}
