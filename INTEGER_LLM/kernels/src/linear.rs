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
}
