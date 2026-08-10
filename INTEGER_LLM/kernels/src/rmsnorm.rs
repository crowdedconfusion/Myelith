//! RMSNorm – Integer-Implementierung

use crate::fixed_point::{clamp_i8, clamp_i32};
use crate::integer_math::rsqrt_q;

/// Integer-RMSNorm mit i8-Input/Output.
pub fn rmsnorm_int8(x: &[i8], gamma: &[i8], frac_bits: u8, eps: i32) -> Vec<i8> {
    let n = x.len();
    assert_eq!(n, gamma.len());

    let mut acc: i64 = 0;
    for &v in x {
        let vi = v as i32;
        acc += (vi * vi) as i64;
    }

    let mean_sq = (acc / n as i64) as i32;
    let rms = rsqrt_q(mean_sq + eps, frac_bits);

    if rms == 0 {
        return vec![0; n];
    }

    let one = 1i32 << frac_bits;
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        let v = x[i] as i32;
        let y = (v << frac_bits) / rms;
        let g = gamma[i] as i32;
        let y2 = (y * g) / one;
        out.push(clamp_i8(y2));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rmsnorm_zero() {
        let x = vec![0i8, 0, 0];
        let gamma = vec![64i8, 64, 64];
        let out = rmsnorm_int8(&x, &gamma, 6, 1);
        assert_eq!(out, vec![0, 0, 0]);
    }

    #[test]
    fn test_rmsnorm_unity() {
        let x = vec![64i8, 64];
        let gamma = vec![64i8, 64];
        let out = rmsnorm_int8(&x, &gamma, 6, 1);
        assert!(out[0] > 60);
        assert!(out[1] > 60);
    }
}
