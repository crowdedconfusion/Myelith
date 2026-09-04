//! Integer-Attention (causal, GQA-faehig ueber Head-Aufteilung des Aufrufers)
//!
//! Aktivierungen int16 mit Per-Layer-Skalen (Numerik-Realitaetsabgleich
//! v0.12.20): Skalarprodukte und V-Gewichtung akkumulieren in i64, da
//! int16-Werte den i32-Bereich ueberschreiten koennen.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]
// Schleifenindizes sind Kopf-/Dimensionsnummern ueber parallele Puffer.
#![allow(clippy::needless_range_loop)]

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
/// `score_mult` traegt die 1/sqrt(head_dim)-Skalierung in Q15 (Fund 19);
/// `score_shift` bringt das Skalarprodukt (Skala `q_frac + k_frac + 15`) auf die
/// exp-LUT-Domäne (`score_frac_bits`, typisch 8) und wird pro Layer aus den
/// kalibrierten Q/K-Skalen abgeleitet (dynamisch, aber deterministisch).
///
/// WICHTIG (Fund 16): Query- und Key-/Value-Laenge sind getrennt zu
/// behandeln. Im KV-Cache-Betrieb besteht `q` nur aus der aktuellen Position
/// (q.len() == 1), waehrend `k`/`v` alle bisherigen Positionen enthalten
/// (k.len() == seq_len). Die Score-/Value-Schleife muss daher ueber
/// `k.len()` laufen, NICHT ueber `q.len()` — sonst attendiert jede Query nur
/// auf den ersten Key und RoPE/Mehrpositions-Attention sind wirkungslos.
pub fn attention_int(
    q: &[Vec<i16>],
    k: &[Vec<i16>],
    v: &[Vec<i16>],
    mask: &[Vec<bool>],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
) -> Vec<Vec<i16>> {
    attention_int_mit_spur(
        q, k, v, mask, score_mult, score_shift, exp_lut, lut_shift, prob_frac_bits, None,
    )
}

/// Dasselbe, aber die Wahrscheinlichkeiten fallen mit ab (TRAINING V).
///
/// # ⚑ Warum ein zweiter Eingang und kein Parameter mehr an `attention_int`
///
/// `attention_int` steht im `Backend`-Merkmal, und zwar in vier
/// Umsetzungen. Ein zusätzliches Argument dort risse alle vier auf, für
/// etwas, das nur der Rückwärtspass braucht. **Hier steht deshalb die
/// eine Umsetzung, und `attention_int` ist ihr Eingang ohne Spur.**
///
/// # ⚑ Warum der Rückwärtspass sie braucht
///
/// `softmax_backward(g, p, frac)` rechnet mit den **Wahrscheinlichkeiten
/// selbst**, denn die Ableitung des Softmax ist
/// `p ⊙ (g − ⟨g, p⟩)`. Ohne sie liesse sich der Gradient nur durch
/// Nachrechnen der Punktprodukte gewinnen, also durch eine zweite
/// Umsetzung derselben Rechnung.
///
/// `spur` bekommt je Abfragezeile eine Zeile Wahrscheinlichkeiten auf
/// `prob_frac_bits`, in derselben Reihenfolge wie die Ausgabe.
#[allow(clippy::too_many_arguments)]
pub fn attention_int_mit_spur(
    q: &[Vec<i16>],
    k: &[Vec<i16>],
    v: &[Vec<i16>],
    mask: &[Vec<bool>],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
    mut spur: Option<&mut Vec<Vec<i32>>>,
) -> Vec<Vec<i16>> {
    let q_len = q.len();
    let kv_len = k.len();
    assert_eq!(kv_len, v.len(), "attention_int: k und v muessen gleich lang sein");
    let head_dim = v[0].len();
    let mut out = Vec::with_capacity(q_len);

    for i in 0..q_len {
        let mut scores = Vec::with_capacity(kv_len);
        for j in 0..kv_len {
            if mask[i][j] {
                // Fund 19: 1/sqrt(head_dim) als Q15-Multiplikation statt
                // Rechtsshift — der Shift war nur fuer gerade Zweierpotenzen
                // korrekt (siehe fixed_point::inv_sqrt_q15).
                let s = dot_int(&q[i], &k[j]) * score_mult;
                scores.push(rshift_round_i64(s, score_shift) as i32);
            } else {
                scores.push(i32::MIN);
            }
        }

        let probs = softmax_int(&scores, exp_lut, lut_shift, prob_frac_bits);
        // ⚑ **Nach dem Softmax und vor der Gewichtung**: Das ist der
        // Wert, mit dem `softmax_backward` rechnet.
        if let Some(sp) = spur.as_deref_mut() {
            sp.push(probs.clone());
        }

        let mut row = vec![0i64; head_dim];
        for j in 0..kv_len {
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
        let out = attention_int(&q, &k, &v, &mask, 1 << 15, 4 + 15, &exp_lut, 0, 8);
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

    #[test]
    fn test_attention_kv_cache_single_query_attends_all_keys() {
        // Fund 16: Im KV-Cache-Betrieb hat q nur 1 Element (aktuelle
        // Position), k/v aber alle bisherigen Positionen. Die Query muss auf
        // ALLE Keys attendieren, nicht nur auf den ersten. Bei identischen
        // Keys sind die Scores gleich -> uniforme Gewichte -> Ausgabe ist der
        // Durchschnitt der Values. Waere der Bug aktiv (nur erster Key),
        // kaeme v[0] = [100, 0] heraus statt [200, 0].
        let q = vec![vec![64i16, 0]];
        let k = vec![vec![64i16, 0], vec![64, 0], vec![64, 0]];
        let v = vec![vec![100i16, 0], vec![200, 0], vec![300, 0]];
        let mask = vec![vec![true, true, true]];
        let exp_lut: Vec<i16> = (0..129).map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16).collect();
        let out = attention_int(&q, &k, &v, &mask, 1 << 15, 4 + 15, &exp_lut, 0, 8);
        assert_eq!(out.len(), 1);
        // Uniforme Gewichte (1/3, 1/3, 1/3) -> Durchschnitt [200, 0].
        assert!((out[0][0] - 200).abs() <= 2, "out[0][0] = {}", out[0][0]);
        assert!(out[0][1].abs() <= 1);
    }
}
