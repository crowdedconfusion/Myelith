//! RoPE (Rotary Position Embedding) – Integer (int16-Aktivierungen)
//!
//! Multi-Frequenz-RoPE im Qwen2/LLaMA-Schema (theta_v 0.10.0, Fund-15-Fix):
//! Jedes Dimensions-Paar j (j in [0, head_dim/2)) hat seine eigene Frequenz;
//! die cos/sin-LUTs sind [max_seq_len, head_dim/2] (flach row-major), und die
//! Paarung ist half-split ((x_j, x_{j+head_dim/2})), konsistent zu HF's
//! rotate_half. Die Rotation ist skaleninvariant gegenüber der Eingangs-Skala:
//! liegt x auf einer beliebigen Zweierpotenz-Skala und cos/sin auf `frac_bits`
//! (spec: rope.frac_bits = 8), hat das Ergebnis dieselbe Skala wie x.

use crate::fixed_point::{clamp_i16, rshift_round};

/// Rotiert einen Head-Vektor (Länge head_dim) mit half-split-Paarung:
/// Paar j ist (vec[j], vec[j+half]), half = head_dim/2. `cos_row`/`sin_row`
/// sind die cos/sin-Werte der aktuellen Position (Länge half), jedes Paar
/// nutzt seinen eigenen Winkel. cos/sin tragen `frac_bits`.
pub fn rotate_half_split_i16(vec: &[i16], cos_row: &[i16], sin_row: &[i16], frac_bits: u8) -> Vec<i16> {
    let half = vec.len() / 2;
    assert_eq!(cos_row.len(), half, "rope: cos_row-Laenge muss head_dim/2 sein");
    assert_eq!(sin_row.len(), half, "rope: sin_row-Laenge muss head_dim/2 sein");
    let mut out = vec![0i16; vec.len()];
    for j in 0..half {
        let cos = cos_row[j] as i32;
        let sin = sin_row[j] as i32;
        let x0 = vec[j] as i32;
        let x1 = vec[j + half] as i32;
        out[j] = clamp_i16(rshift_round(x0 * cos - x1 * sin, frac_bits));
        out[j + half] = clamp_i16(rshift_round(x1 * cos + x0 * sin, frac_bits));
    }
    out
}

/// Wendet Multi-Frequenz-RoPE auf Q- und K-Sequenzen an.
/// `cos_lut`/`sin_lut` sind flach row-major [max_seq_len * half] mit
/// half = head_dim/2 (aus der Vektorlänge abgeleitet).
pub fn apply_rope_i16(
    q_seq: &[Vec<i16>],
    k_seq: &[Vec<i16>],
    cos_lut: &[i16],
    sin_lut: &[i16],
    positions: &[usize],
    frac_bits: u8,
) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
    let head_dim = q_seq[0].len();
    let half = head_dim / 2;
    let n_pos = cos_lut.len() / half;

    let mut q_out = Vec::with_capacity(q_seq.len());
    let mut k_out = Vec::with_capacity(k_seq.len());

    for (pos, q_vec, k_vec) in itertools::izip!(positions, q_seq, k_seq) {
        let idx = pos % n_pos;
        let cos_row = &cos_lut[idx * half..(idx + 1) * half];
        let sin_row = &sin_lut[idx * half..(idx + 1) * half];
        q_out.push(rotate_half_split_i16(q_vec, cos_row, sin_row, frac_bits));
        k_out.push(rotate_half_split_i16(k_vec, cos_row, sin_row, frac_bits));
    }
    (q_out, k_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(vals: &[i16]) -> Vec<i16> {
        vals.to_vec()
    }

    #[test]
    fn test_rotate_half_split_identity() {
        // cos = 1.0 (256 bei frac 8), sin = 0 für alle Paare -> unverändert.
        let v = vec![100i16, -200, 32000, -32000];
        let cos_row = row(&[256, 256]);
        let sin_row = row(&[0, 0]);
        let out = rotate_half_split_i16(&v, &cos_row, &sin_row, 8);
        assert_eq!(out, v);
    }

    #[test]
    fn test_rotate_half_split_quarter_turn() {
        // Paar j mit cos=0, sin=1.0: (x0, x1) -> (-x1, x0).
        // head_dim=4, half=2: Paar 0 = (v[0], v[2]), Paar 1 = (v[1], v[3]).
        let v = vec![100i16, 200, 300, 400];
        let cos_row = row(&[0, 0]);
        let sin_row = row(&[256, 256]);
        let out = rotate_half_split_i16(&v, &cos_row, &sin_row, 8);
        // Paar 0: (100,300) -> (-300, 100); Paar 1: (200,400) -> (-400, 200).
        assert_eq!(out, vec![-300, -400, 100, 200]);
    }

    #[test]
    fn test_rotate_preserves_scale_of_any_input_scale() {
        // Werte auf Skala frac 12, Rotation mit cos/sin bei frac 8 erhält die
        // Eingangs-Skala (cos=1.0, sin=0 -> Identität).
        let v = vec![4096i16, 0, 0, 0];
        let cos_row = row(&[256, 256]);
        let sin_row = row(&[0, 0]);
        let out = rotate_half_split_i16(&v, &cos_row, &sin_row, 8);
        assert_eq!(out, vec![4096, 0, 0, 0]);
    }

    #[test]
    fn test_apply_rope_position_zero_is_identity() {
        // Position 0: alle Winkel 0 -> cos=1.0, sin=0 -> Identität.
        let half = 2;
        let n_pos = 4;
        let mut cos_lut = Vec::new();
        let mut sin_lut = Vec::new();
        for _p in 0..n_pos {
            for _j in 0..half {
                cos_lut.push(256i16);
                sin_lut.push(0i16);
            }
        }
        // Für Position 0 ist die Identität unabhängig von den übrigen Werten.
        let q = vec![vec![10i16, 20, 30, 40]];
        let k = vec![vec![50i16, 60, 70, 80]];
        let (q_out, k_out) = apply_rope_i16(&q, &k, &cos_lut, &sin_lut, &[0usize], 8);
        assert_eq!(q_out[0], vec![10, 20, 30, 40]);
        assert_eq!(k_out[0], vec![50, 60, 70, 80]);
    }
}

