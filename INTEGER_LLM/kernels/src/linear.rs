//! Lineare Schichten: W8A8 und Ternaer

use crate::fixed_point::{clamp_i8, rescale};

/// W8A8 Matrix-Vektor-Multiplikation.
pub fn linear_w8a8(
    x: &[i8],
    W: &[Vec<i8>],
    act_frac_bits: u8,
    weight_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<i8> {
    let in_frac = act_frac_bits + weight_frac_bits;
    let mut out = Vec::with_capacity(W.len());

    for row in W {
        let mut acc: i32 = 0;
        for (w, v) in row.iter().zip(x.iter()) {
            acc += (*w as i32) * (*v as i32);
        }
        let y = rescale(acc, in_frac, out_frac_bits);
        out.push(clamp_i8(y));
    }
    out
}

/// Ternaere Matrix-Vektor-Multiplikation.
pub fn linear_ternary(x: &[i8], W: &[Vec<i8>], out_shift: i8) -> Vec<i8> {
    let mut out = Vec::with_capacity(W.len());
    for row in W {
        let mut acc: i32 = 0;
        for (w, v) in row.iter().zip(x.iter()) {
            match *w {
                1 => acc += *v as i32,
                -1 => acc -= *v as i32,
                _ => {}
            }
        }
        let y = if out_shift > 0 {
            acc >> out_shift
        } else if out_shift < 0 {
            acc << (-out_shift)
        } else {
            acc
        };
        out.push(clamp_i8(y));
    }
    out
}

/// Addiert einen quantisierten Bias auf die Ausgabe einer linearen Schicht.
///
/// Der Bias liegt als int8 mit eigener kalibrierter Skala (`bias_shift`
/// frac_bits, siehe `QTensor.shift`) vor und wird mit `rescale` auf die
/// Ausgabeskala (`out_frac_bits`) gebracht — arithmetischer Rechtsshift mit
/// Round-to-nearest-even, danach i32-Addition und Clamping auf i8. Reine
/// Ganzzahlarithmetik, deterministisch über alle Backends (Whitepaper
/// Kap. 6.2; für Qwen2.5 besitzen q/k/v_proj Biases).
pub fn add_bias_i8(out: &mut [i8], bias: &[i8], bias_shift: u8, out_frac_bits: u8) {
    assert_eq!(
        out.len(),
        bias.len(),
        "add_bias_i8: Ausgabe ({} Elemente) und Bias ({} Elemente) muessen dieselbe Laenge haben",
        out.len(),
        bias.len()
    );
    for (o, b) in out.iter_mut().zip(bias.iter()) {
        let bias_rescaled = rescale(*b as i32, bias_shift, out_frac_bits);
        *o = clamp_i8((*o as i32) + bias_rescaled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_w8a8_identity() {
        let x = vec![64i8, -64];
        // Eigentlich 128 = 2^7 (Identitaet bei weight_frac 7), aber i8 ist
        // auf 127 begrenzt; via RNE-Rundung bleibt das Ergebnis exakt.
        let W = vec![vec![127i8, 0], vec![0i8, 127]];
        let out = linear_w8a8(&x, &W, 6, 7, 6);
        assert_eq!(out, vec![64, -64]);
    }

    #[test]
    fn test_linear_ternary() {
        let x = vec![10i8, -20];
        let W = vec![vec![1i8, -1], vec![-1i8, 1]];
        let out = linear_ternary(&x, &W, 0);
        assert_eq!(out, vec![30, -30]);
    }

    #[test]
    fn test_add_bias_i8_rescale_left_shift() {
        // bias_shift (2) < out_frac (4): Linksverschiebung, exakt.
        // bias=1 mit 2 Nachkommabits = 0.25 -> bei 4 Nachkommabits 4.
        let mut out = vec![10i8, -10];
        add_bias_i8(&mut out, &[1i8, 1], 2, 4);
        assert_eq!(out, vec![14, -6]);
    }

    #[test]
    fn test_add_bias_i8_rescale_right_shift_rounds_rne() {
        // bias_shift (3) > out_frac (1): Rechtsshift um 2 mit RNE-Rundung.
        // bias=3: 3 >> 2 mit Rest 3 -> roundet auf 1; bias=-3 -> -1.
        let mut out = vec![0i8, 0];
        add_bias_i8(&mut out, &[3i8, -3], 3, 1);
        assert_eq!(out, vec![1, -1]);
    }

    #[test]
    fn test_add_bias_i8_negative_bias_and_clamping() {
        let mut out = vec![126i8, -127, 0];
        // bias=4 mit shift 1 = 2.0 -> bei out_frac 0: +2 bzw. -2.
        add_bias_i8(&mut out, &[4i8, -4, 0], 1, 0);
        assert_eq!(out, vec![127, -128, 0]); // Saettigung an beiden Grenzen
    }

    #[test]
    #[should_panic(expected = "dieselbe Laenge")]
    fn test_add_bias_i8_length_mismatch_panics() {
        let mut out = vec![0i8; 3];
        add_bias_i8(&mut out, &[1i8, 1], 0, 0);
    }
}
