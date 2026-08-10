//! Integer-Attention (causal, single-head)

use crate::fixed_point::rshift_round;
use crate::softmax::softmax_int;

/// Skalierter Dot-Product in i32.
#[inline]
pub fn dot_int(a: &[i8], b: &[i8]) -> i32 {
    let mut acc: i32 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc += (*x as i32) * (*y as i32);
    }
    acc
}

/// Integer-Attention.
pub fn attention_int(
    q: &[Vec<i8>],
    k: &[Vec<i8>],
    v: &[Vec<i8>],
    mask: &[Vec<bool>],
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
) -> Vec<Vec<i8>> {
    let seq_len = q.len();
    let head_dim = v[0].len();
    let mut out = Vec::with_capacity(seq_len);

    for i in 0..seq_len {
        let mut scores = Vec::with_capacity(seq_len);
        for j in 0..seq_len {
            if mask[i][j] {
                let s = dot_int(&q[i], &k[j]);
                scores.push(rshift_round(s, score_shift));
            } else {
                scores.push(i32::MIN);
            }
        }

        let probs = softmax_int(&scores, exp_lut, lut_shift, prob_frac_bits);

        let mut row = vec![0i32; head_dim];
        for j in 0..seq_len {
            if probs[j] == 0 { continue; }
            for d in 0..head_dim {
                row[d] += probs[j] * (v[j][d] as i32);
            }
        }

        let mut out_row = Vec::with_capacity(head_dim);
        for d in 0..head_dim {
            out_row.push(crate::fixed_point::clamp_i8(rshift_round(row[d], prob_frac_bits)));
        }
        out.push(out_row);
    }
    out
}
