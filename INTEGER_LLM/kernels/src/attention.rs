//! Integer-Attention (causal, GQA-faehig ueber Head-Aufteilung des Aufrufers)
//!
//! Aktivierungen int16 mit Per-Layer-Skalen (Numerik-Realitaetsabgleich
//! v0.12.20): Skalarprodukte und V-Gewichtung akkumulieren in i64, da
//! int16-Werte den i32-Bereich ueberschreiten koennen.

use crate::fixed_point::{clamp_i16_from_i64, rshift_round_i64};
use crate::softmax::softmax_int;

/// Skalierter Dot-Product in i64 (int16-Operanden).
#[inline]
pub fn dot_int(a: &[i16], b: &[i16]) -> i64 {
    let mut acc: i64 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc += (*x as i64) * (*y as i64);
    }
    acc
}

/// Integer-Attention.
///
/// `score_shift` bringt das Skalarprodukt (Skala `q_frac + k_frac`) auf die
/// exp-LUT-Domäne (`score_frac_bits`, typisch 8) und wird pro Layer aus den
/// kalibrierten Q/K-Skalen abgeleitet (dynamisch, aber deterministisch).
pub fn attention_int(
    q: &[Vec<i16>],
    k: &[Vec<i16>],
    v: &[Vec<i16>],
    mask: &[Vec<bool>],
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
) -> Vec<Vec<i16>> {
    let seq_len = q.len();
    let head_dim = v[0].len();
    let mut out = Vec::with_capacity(seq_len);

    for i in 0..seq_len {
        let mut scores = Vec::with_capacity(seq_len);
        for j in 0..seq_len {
            if mask[i][j] {
                let s = dot_int(&q[i], &k[j]);
                scores.push(rshift_round_i64(s, score_shift) as i32);
            } else {
                scores.push(i32::MIN);
            }
        }

        let probs = softmax_int(&scores, exp_lut, lut_shift, prob_frac_bits);

        let mut row = vec![0i64; head_dim];
        for j in 0..seq_len {
            if probs[j] == 0 { continue; }
            for d in 0..head_dim {
                row[d] += (probs[j] as i64) * (v[j][d] as i64);
            }
        }

        let mut out_row = Vec::with_capacity(head_dim);
        for d in 0..head_dim {
            out_row.push(clamp_i16_from_i64(rshift_round_i64(row[d], prob_frac_bits)));
        }
        out.push(out_row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_uniform_weights() {
        // Zwei Positionen, identische K/V: unabhaengig von den Scores muss
        // die Ausgabe eine Konvexkombination der V-Werte sein; bei
        // identischen V also exakt diese V-Werte (bis auf Rundung).
        let q = vec![vec![64i16, 0], vec![64, 0]];
        let k = vec![vec![64i16, 0], vec![64, 0]];
        let v = vec![vec![100i16, -50], vec![100, -50]];
        let mask = vec![vec![true, false], vec![true, true]];
        let exp_lut: Vec<i16> = (0..129).map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16).collect();
        let out = attention_int(&q, &k, &v, &mask, 4, &exp_lut, 0, 8);
        assert_eq!(out.len(), 2);
        assert!((out[0][0] - 100).abs() <= 1);
        assert!((out[0][1] + 50).abs() <= 1);
        assert!((out[1][0] - 100).abs() <= 1);
    }

    #[test]
    fn test_dot_int_i64_range() {
        // head_dim 64, alle Werte nahe i16-Max: Summe > i32::MAX.
        let a = vec![30000i16; 64];
        let b = vec![30000i16; 64];
        let d = dot_int(&a, &b);
        assert_eq!(d, 64 * 30000i64 * 30000i64);
    }
}
