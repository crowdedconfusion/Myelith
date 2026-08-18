//! RMSNorm – Integer-Implementierung (theta_v 0.5.0)
//!
//! Eingang: int16-Residualstrom; Ausgang: int16-Aktivierung auf einer
//! kalibrierten Per-Layer-Skala. Reine Ganzzahlarithmetik ohne Division im
//! Hot-Path (spec: shift_semantics = arithmetic_right_shift).
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]

use crate::fixed_point::{clamp_i16, rescale_i64, rshift_round_i64};

/// Reziproken-Konstante 2^20 / n (gerundet) — einmalige Initialisierung,
/// NICHT Teil des tokenweisen Hot-Path. Damit wird der Mittelwert im
/// Inferenzpfad selbst divisionsfrei: mean ≈ (sum * inv_n_q20) >> 20.
#[inline]
pub fn inv_n_q20(n: usize) -> i64 {
    assert!(n > 0, "inv_n_q20: n muss > 0 sein");
    ((1i64 << 20) + (n as i64) / 2) / (n as i64)
}

/// RMSNorm über den int16-Residualstrom mit LUT-gestütztem rsqrt.
/// Ausgang: int16-Aktivierung auf der kalibrierten Per-Layer-Skala
/// `out_frac_bits` (Numerik-Realitätsabgleich v0.12.20: Aktivierungen sind
/// int16, da reale RMSNorm-Ausgaben den int8-Bereich sprengen).
///
/// Mathematik (alle Größen ganzzahlig, deterministisch):
/// - `M = mean(x_i^2)` via `(sum * inv_n_q20) >> 20`
///   (M ist skaleninvariant: RMS-Normalisierung kürzt die Eingangsskala).
/// - Dynamischer gerader Index-Shift `q` (spec: rsqrt.index_normalization =
///   "dynamic_even_shift"): kleinstes gerades q mit `(M >> q) <= LUT-Bereich`.
///   Datenabhängig, aber deterministisch — alle Knoten leiten aus denselben
///   Daten dasselbe q ab (Muster wie der dynamische score_shift der
///   Attention). Verhindert Halb-Bit-Faktoren (q gerade).
/// - `lut[idx]` mit `idx = M >> q` liefert
///   `round(rsqrt(idx * 2^-lut_input_shift) * 2^lut_output_frac)`; das
///   Produkt `x_i * lut` trägt damit die Skala
///   `2^-(lut_output_frac + lut_input_shift/2 + q/2)`.
/// - Multiplikation mit gamma (eigener Shift `gamma_shift`) und Rescale auf
///   `out_frac_bits` als i64-Zwischenprodukt, dann Clamping auf i16.
/// - eps (HF: 1e-6) rundet bei realistischen Residualskalen auf 0; der Fall
///   M = 0 liefert explizit Nullen (identisch zu HF: 0/sqrt(eps) = 0).
pub fn rmsnorm_i16(
    x: &[i16],
    gamma: &[i8],
    gamma_shifts: &[u8],
    rsqrt_lut: &[i16],
    lut_input_shift: u8,
    lut_output_frac: u8,
    inv_n_q20: i64,
    out_frac_bits: u8,
) -> Vec<i16> {
    let n = x.len();
    assert_eq!(n, gamma.len(), "rmsnorm_i16: x und gamma muessen gleich lang sein");
    assert_eq!(n, gamma_shifts.len(), "rmsnorm_i16: ein Gamma-Shift je Element (theta_v 0.7.0)");
    assert!(lut_input_shift.is_multiple_of(2), "rmsnorm_i16: lut_input_shift muss gerade sein (Halb-Bit-Faktor)");

    let mut acc: i64 = 0;
    for &v in x {
        acc += (v as i64) * (v as i64);
    }
    if acc == 0 {
        return vec![0i16; n];
    }

    // Mittelwert ohne Division: Multiplikation mit Reziproken-Konstante.
    let m = ((acc as i128 * inv_n_q20 as i128) >> 20) as i64;

    // Dynamischer gerader Index-Shift in den LUT-Bereich.
    let max_idx = (rsqrt_lut.len() - 1) as i64;
    let mut q: u8 = 0;
    while (m >> q) > max_idx {
        q += 2;
    }
    let idx = rshift_round_i64(m, q).min(max_idx) as usize;

    let lut_val = rsqrt_lut[idx] as i64;
    let norm_frac = lut_output_frac + lut_input_shift / 2 + q / 2;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let total_frac = norm_frac + gamma_shifts[i];
        let prod = (x[i] as i64) * lut_val * (gamma[i] as i64);
        out.push(clamp_i16(rescale_i64(prod, total_frac, out_frac_bits) as i32));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut eine rsqrt-LUT im spec-Format (input_shift 8, output frac 8):
    /// lut[x] = round(2^12 / sqrt(x)), Sentinel lut[0] = 2^8.
    fn spec_lut(len: usize) -> Vec<i16> {
        let mut lut = Vec::with_capacity(len);
        for x in 0..len {
            if x == 0 {
                lut.push(256);
            } else {
                let val = 4096.0 / (x as f64).sqrt();
                lut.push(val.round() as i16);
            }
        }
        lut
    }

    #[test]
    fn test_inv_n_q20() {
        // (2^20 + 448) / 896 = 1170 (Integerdivision, gerundete Reziproke).
        assert_eq!(inv_n_q20(896), 1170);
        assert_eq!(inv_n_q20(1), 1 << 20);
    }

    #[test]
    fn test_rmsnorm_zero_input() {
        let lut = spec_lut(1024);
        let out = rmsnorm_i16(&[0, 0, 0], &[64, 64, 64], &[6, 6, 6], &lut, 8, 8, inv_n_q20(3), 6);
        assert_eq!(out, vec![0, 0, 0]);
    }

    #[test]
    fn test_rmsnorm_constant_input_normalizes_to_one() {
        // Alle x gleich -> mean(x^2) = x^2 -> normalisierter Wert ±1.
        // gamma = 1.0 (shift 6 -> 64), out_frac 6 -> erwartet ±64.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 16], &[64, 64], &[6, 6], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out, vec![64, 64]);
        let out_neg = rmsnorm_i16(&[-16, -16], &[64, 64], &[6, 6], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out_neg, vec![-64, -64]);
    }

    #[test]
    fn test_rmsnorm_large_input_uses_dynamic_q() {
        // x = 12000 -> M = 1.44e8 > 32767 -> q > 0 noetig. Ergebnis muss
        // trotzdem ±1 * gamma sein (Normalisierung), innerhalb LUT-Rundung.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[12000, 12000], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 3);
        // ±1.0 bei frac 3 = ±8; LUT-/Indexrundung erlaubt ±1 Abweichung.
        assert!((out[0] - 8).abs() <= 1, "out[0] = {}", out[0]);
        assert!((out[1] - 8).abs() <= 1, "out[1] = {}", out[1]);
    }

    #[test]
    fn test_rmsnorm_two_values_hand_computed() {
        // x = [16, 0] -> M = (256 + 0)/2 = 128 -> sqrt(M) = 11.3137
        // normalisiert: [16/11.3137, 0] = [1.4142, 0]; gamma 1.0 (shift 5: 32)
        // out_frac 6: [round(1.4142*64), 0] = [90 oder 91, 0]
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 0], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        assert!(out[0] == 90 || out[0] == 91, "out[0] = {}", out[0]);
        assert_eq!(out[1], 0);
    }

    #[test]
    fn test_rmsnorm_gamma_scaling() {
        // gamma 2.0 (shift 5 -> 64) verdoppelt das Ergebnis gegenueber 1.0.
        // (i16-Ausgang: 2.0 bei frac 6 = 128, kein i8-Clamping mehr.)
        let lut = spec_lut(32768);
        let one = rmsnorm_i16(&[16, 16], &[32, 32], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        let two = rmsnorm_i16(&[16, 16], &[64, 64], &[5, 5], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(one, vec![64, 64]);
        assert_eq!(two, vec![128, 128]);
    }

    #[test]
    fn test_rmsnorm_gamma_per_element_shifts() {
        // Unterschiedliche Gamma-Shifts je Element (theta_v 0.7.0):
        // gamma[0] = 32 mit Shift 5 (= 1.0), gamma[1] = 32 mit Shift 4 (= 2.0)
        // -> Element 1 wird verdoppelt.
        let lut = spec_lut(32768);
        let out = rmsnorm_i16(&[16, 16], &[32, 32], &[5, 4], &lut, 8, 8, inv_n_q20(2), 6);
        assert_eq!(out[0], 64);  // 1.0 * 1.0
        assert_eq!(out[1], 128); // 1.0 * 2.0
    }
}
