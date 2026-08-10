//! RoPE (Rotary Position Embedding) – Integer

use crate::fixed_point::{clamp_i8, rshift_round};

/// Rotiert Paare (x0, x1) um (cos, sin).
pub fn rotate_pairs(vec: &[i8], cos_q: i16, sin_q: i16, frac_bits: u8) -> Vec<i8> {
    let mut out = Vec::with_capacity(vec.len());
    let cos = cos_q as i32;
    let sin = sin_q as i32;

    for i in (0..vec.len()).step_by(2) {
        let x0 = vec[i] as i32;
        let x1 = vec[i + 1] as i32;
        let y0 = rshift_round(x0 * cos - x1 * sin, frac_bits);
        let y1 = rshift_round(x0 * sin + x1 * cos, frac_bits);
        out.push(clamp_i8(y0));
        out.push(clamp_i8(y1));
    }
    out
}

/// Wendet RoPE auf Q- und K-Sequenzen an.
pub fn apply_rope(
    q_seq: &[Vec<i8>],
    k_seq: &[Vec<i8>],
    cos_lut: &[i16],
    sin_lut: &[i16],
    positions: &[usize],
    frac_bits: u8,
) -> (Vec<Vec<i8>>, Vec<Vec<i8>>) {
    let mut q_out = Vec::with_capacity(q_seq.len());
    let mut k_out = Vec::with_capacity(k_seq.len());

    for (pos, q_vec, k_vec) in itertools::izip!(positions, q_seq, k_seq) {
        let idx = pos % cos_lut.len();
        let cos_q = cos_lut[idx];
        let sin_q = sin_lut[idx];
        q_out.push(rotate_pairs(q_vec, cos_q, sin_q, frac_bits));
        k_out.push(rotate_pairs(k_vec, cos_q, sin_q, frac_bits));
    }
    (q_out, k_out)
}
