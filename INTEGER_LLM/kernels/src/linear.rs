//! Lineare Schichten: W8A16 (Gewichte int8, Aktivierungen int16) und Bias.
//!
//! Seit dem Numerik-Realitaetsabgleich (v0.12.20) sind Aktivierungen int16
//! mit kalibrierten Per-Layer-Zweierpotenz-Skalen: reale RMSNorm-/MLP-
//! Ausgaben (gemessen bis ~±1640) sprengen den int8-Bereich. Gewichte
//! bleiben int8. Akkumulation in i64, da 896 Kanaele * 127 * 32767 den
//! i32-Bereich ueberschreiten koennen.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};

/// W8A16 Matrix-Vektor-Multiplikation.
///
/// `x` (Aktivierung, int16, Skala `act_frac_bits`), `W` (Gewicht, int8,
/// Per-Channel-Skala je Ausgabe-Zeile `w_shifts[r]`, theta_v 0.7.0);
/// Ausgabe int16 auf `out_frac_bits`.
pub fn linear_w8a16(
    x: &[i16],
    W: &[Vec<i8>],
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<i16> {
    assert_eq!(W.len(), w_shifts.len(), "linear_w8a16: eine Skala je Ausgabe-Zeile");
    let mut out = Vec::with_capacity(W.len());

    for (row, &w_shift) in W.iter().zip(w_shifts.iter()) {
        let mut acc: i64 = 0;
        for (w, v) in row.iter().zip(x.iter()) {
            acc += (*w as i64) * (*v as i64);
        }
        let y = rescale_i64(acc, w_shift + act_frac_bits, out_frac_bits);
        out.push(clamp_i16_from_i64(y));
    }
    out
}

/// W8A16 mit Per-Kanal-Ausgangsskala (Fund 20, theta_v 0.11.0).
///
/// Wie `linear_w8a16`, aber `out_frac_bits` ist ein Shift JE
/// AUSGABE-KANAL statt ein einziger fuer den ganzen Vektor. Wird fuer
/// `o_proj` und `down_proj` gebraucht: ihre Ausgabe wird direkt in den
/// Residualstrom addiert, und der trägt seit Fund 20 eine Skala je Kanal
/// (Massive Activations bei Qwen2.5-7B — siehe `rmsnorm.rs`-Modulkopf).
/// `q_proj`/`k_proj`/`v_proj`/`gate_proj`/`up_proj` bleiben bei der
/// Skalar-Funktion, weil ihre Ausgaben NICHT in den Residualstrom
/// zurückfliessen und keine vergleichbaren Ausreisser zeigen.
///
/// Bei identischem Wert in jedem Element von `out_frac_bits` ist das
/// Ergebnis bitgleich zu `linear_w8a16` mit demselben Skalar (siehe
/// `test_linear_w8a16_pc_uniform_matches_scalar`).
pub fn linear_w8a16_pc(
    x: &[i16],
    W: &[Vec<i8>],
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    assert_eq!(W.len(), w_shifts.len(), "linear_w8a16_pc: eine Skala je Ausgabe-Zeile");
    assert_eq!(W.len(), out_frac_bits.len(), "linear_w8a16_pc: eine Ausgangsskala je Kanal (Fund 20)");
    let mut out = Vec::with_capacity(W.len());

    for (row, (&w_shift, &out_frac)) in W.iter().zip(w_shifts.iter().zip(out_frac_bits.iter())) {
        let mut acc: i64 = 0;
        for (w, v) in row.iter().zip(x.iter()) {
            acc += (*w as i64) * (*v as i64);
        }
        let y = rescale_i64(acc, w_shift + act_frac_bits, out_frac);
        out.push(clamp_i16_from_i64(y));
    }
    out
}

/// Addiert einen quantisierten Bias auf eine int16-Aktivierungsausgabe.
///
/// Der Bias liegt als **int16** mit Per-Element-Skalen vor
/// (`bias_shifts[i]`). Bis theta_v 0.12.0 war es int8 — das saettigte
/// still bei Betraegen ueber 127 und verfaelschte bei Qwen2.5-7B die
/// Attention ab Ebene 0 (Fund 23, k_proj.bias erreicht 414)
/// und wird elementweise mit `rescale` auf die Ziel-Skala
/// (`out_frac_bits`) gebracht — arithmetischer Rechtsshift mit
/// Round-to-nearest-even, danach i64-Addition mit Clamping auf i16. Reine
/// Ganzzahlarithmetik, deterministisch über alle Backends (Whitepaper
/// Kap. 6.2; Qwen2.5 besitzt Biases an q/k/v_proj).
pub fn add_bias_i16(out: &mut [i16], bias: &[i16], bias_shifts: &[u8], out_frac_bits: u8) {
    assert_eq!(
        out.len(),
        bias.len(),
        "add_bias_i16: Ausgabe ({} Elemente) und Bias ({} Elemente) muessen dieselbe Laenge haben",
        out.len(),
        bias.len()
    );
    assert_eq!(bias.len(), bias_shifts.len(), "add_bias_i16: eine Skala je Bias-Element");
    for ((o, b), &b_shift) in out.iter_mut().zip(bias.iter()).zip(bias_shifts.iter()) {
        let bias_rescaled = rescale(*b as i32, b_shift, out_frac_bits);
        *o = clamp_i16_from_i64((*o as i64) + (bias_rescaled as i64));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_w8a16_identity() {
        // x = [1.0, -1.0] bei frac 6 = [64, -64]; Identitaets-Matrix mit
        // Per-Channel-Shift 7 (127 ~ 1.0): Ergebnis ~ [64, -64] bei frac 6.
        let x = vec![64i16, -64];
        let W = vec![vec![127i8, 0], vec![0i8, 127]];
        let out = linear_w8a16(&x, &W, &[7, 7], 6, 6);
        // 127/128 = 0.992 -> 64 * 127 >> 7 = 63 (RNE: 63.5 -> 64? 64*127=8128,
        // >>7 = 63 Rest 64 = half -> quotient 63 ungerade -> +1 = 64).
        assert_eq!(out, vec![64, -64]);
    }

    #[test]
    fn test_linear_w8a16_per_channel_shifts() {
        // Zeile 0 mit Shift 7 (~1.0), Zeile 1 mit Shift 6 (~2.0):
        // dieselben Gewichte, aber Zeile 1 verdoppelt das Ergebnis.
        let x = vec![64i16];
        let W = vec![vec![64i8], vec![64i8]];
        let out = linear_w8a16(&x, &W, &[7, 6], 6, 6);
        // Zeile 0: 64*64 = 4096, rescale(4096, 13, 6) = 4096>>7 = 32
        // Zeile 1: 64*64 = 4096, rescale(4096, 12, 6) = 4096>>6 = 64
        assert_eq!(out, vec![32, 64]);
    }

    #[test]
    fn test_linear_w8a16_large_accumulator() {
        // Akkumulator jenseits von i32: 896 Kanaele, alle w=127, x=32767
        // -> acc = 896 * 127 * 32767 ≈ 3.7e9 > i32::MAX. Muss in i64
        // akkumulieren und korrekt reskalieren.
        let n = 896usize;
        let x = vec![32767i16; n];
        let W = vec![vec![127i8; n]];
        // in_frac = 5 + 7 = 12, out_frac 3: acc >> 9.
        let out = linear_w8a16(&x, &W, &[7], 5, 3);
        let expected = 32767;
        assert_eq!(out[0], expected as i16);
    }

    #[test]
    fn test_linear_w8a16_pc_uniform_matches_scalar() {
        // Fund 20: dasselbe out_frac_bits in jedem Kanal muss bitgleich
        // zur Skalar-Funktion sein - Voraussetzung dafuer, dass bestehende
        // Aufrufer (q/k/v/gate/up_proj) unangetastet bleiben duerfen.
        let x = vec![100i16, -50, 25];
        let W = vec![vec![64i8, -32, 16], vec![10i8, 20, -30]];
        let w_shifts = vec![6u8, 5];
        let skalar = linear_w8a16(&x, &W, &w_shifts, 4, 6);
        let per_kanal = linear_w8a16_pc(&x, &W, &w_shifts, 4, &[6, 6]);
        assert_eq!(skalar, per_kanal);
    }

    #[test]
    fn test_linear_w8a16_pc_different_targets_per_channel() {
        // Zwei Ausgabezeilen mit identischen Rohwerten, aber
        // unterschiedlicher Zielskala je Kanal (das ist der eigentliche
        // Zweck: o_proj/down_proj muessen auf die per-Kanal kalibrierte
        // Residualskala zielen koennen, nicht nur auf eine gemeinsame).
        let x = vec![100i16, 100];
        let W = vec![vec![64i8, 64], vec![64i8, 64]]; // identische Zeilen
        let w_shifts = vec![6u8, 6];
        let out = linear_w8a16_pc(&x, &W, &w_shifts, 0, &[6, 3]);
        // acc = 64*100 + 64*100 = 12800 fuer beide Zeilen.
        // Zeile 0: rescale(12800, 6, 6) = 12800 (kein Shift).
        // Zeile 1: rescale(12800, 6, 3) = 12800 >> 3 = 1600.
        assert_eq!(out[0], 12800);
        assert_eq!(out[1], 1600);
    }

    #[test]
    fn test_add_bias_i16_rescale_left_shift() {
        // bias_shift (2) < out_frac (4): Linksverschiebung, exakt.
        let mut out = vec![10i16, -10];
        add_bias_i16(&mut out, &[1i16, 1], &[2, 2], 4);
        assert_eq!(out, vec![14, -6]);
    }

    #[test]
    fn test_add_bias_i16_rescale_right_shift_rounds_rne() {
        // bias_shift (3) > out_frac (1): Rechtsshift um 2 mit RNE-Rundung.
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[3i16, -3], &[3, 3], 1);
        assert_eq!(out, vec![1, -1]);
    }

    #[test]
    fn test_add_bias_i16_per_element_shifts() {
        // Unterschiedliche Shifts je Element (theta_v 0.7.0):
        // Bias 4 mit Shift 1 (= 2.0), Bias 4 mit Shift 0 (= 4.0).
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[4i16, 4], &[1, 0], 0);
        assert_eq!(out, vec![2, 4]);
    }

    #[test]
    fn test_add_bias_i16_clamping() {
        let mut out = vec![32766i16, -32767, 0];
        add_bias_i16(&mut out, &[4i16, -4, 0], &[1, 1, 1], 0); // +2 bzw. -2
        assert_eq!(out, vec![32767, -32768, 0]); // Saettigung an beiden Grenzen
    }

    #[test]
    #[should_panic(expected = "dieselbe Laenge")]
    fn test_add_bias_i16_length_mismatch_panics() {
        let mut out = vec![0i16; 3];
        add_bias_i16(&mut out, &[1i16, 1], &[0, 0], 0);
    }
}
