//! RoPE (Rotary Position Embedding) – Integer (int16-Aktivierungen)
//!
//! Die Rotation ist skaleninvariant gegenüber der Eingangs-Skala: liegt x
//! auf einer beliebigen Zweierpotenz-Skala und cos/sin auf `frac_bits`
//! (spec: rope.frac_bits = 8), hat das Ergebnis dieselbe Skala wie x.

use crate::fixed_point::{clamp_i16, rshift_round};

/// Rotiert Paare (x0, x1) um (cos, sin); cos/sin tragen `frac_bits`.
pub fn rotate_pairs_i16(vec: &[i16], cos_q: i16, sin_q: i16, frac_bits: u8) -> Vec<i16> {
    let mut out = Vec::with_capacity(vec.len());
    let cos = cos_q as i32;
    let sin = sin_q as i32;

    for i in (0..vec.len()).step_by(2) {
        let x0 = vec[i] as i32;
        let x1 = vec[i + 1] as i32;
        let y0 = rshift_round(x0 * cos - x1 * sin, frac_bits);
        let y1 = rshift_round(x0 * sin + x1 * cos, frac_bits);
        out.push(clamp_i16(y0));
        out.push(clamp_i16(y1));
    }
    out
}

/// Wendet RoPE auf Q- und K-Sequenzen an.
pub fn apply_rope_i16(
    q_seq: &[Vec<i16>],
    k_seq: &[Vec<i16>],
    cos_lut: &[i16],
    sin_lut: &[i16],
    positions: &[usize],
    frac_bits: u8,
) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
    let mut q_out = Vec::with_capacity(q_seq.len());
    let mut k_out = Vec::with_capacity(k_seq.len());

    for (pos, q_vec, k_vec) in itertools::izip!(positions, q_seq, k_seq) {
        let idx = pos % cos_lut.len();
        let cos_q = cos_lut[idx];
        let sin_q = sin_lut[idx];
        q_out.push(rotate_pairs_i16(q_vec, cos_q, sin_q, frac_bits));
        k_out.push(rotate_pairs_i16(k_vec, cos_q, sin_q, frac_bits));
    }
    (q_out, k_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_pairs_identity() {
        // cos = 1.0 (256 bei frac 8), sin = 0 -> unveraendert.
        let v = vec![100i16, -200, 32000, -32000];
        let out = rotate_pairs_i16(&v, 256, 0, 8);
        assert_eq!(out, v);
    }

    #[test]
    fn test_rotate_pairs_quarter_turn() {
        // cos = 0, sin = 1.0: (x0, x1) -> (-x1, x0).
        let v = vec![100i16, 200];
        let out = rotate_pairs_i16(&v, 0, 256, 8);
        assert_eq!(out, vec![-200, 100]);
    }

    #[test]
    fn test_rotate_preserves_scale_of_any_input_scale() {
        // Werte auf Skala frac 12 (z. B. kalibrierte Q/K-Skala): Rotation
        // mit cos/sin bei frac 8 erhaelt die Eingangs-Skala.
        let v = vec![4096i16, 0]; // = 1.0 bei frac 12
        let out = rotate_pairs_i16(&v, 256, 0, 8);
        assert_eq!(out, vec![4096, 0]);
    }
}
