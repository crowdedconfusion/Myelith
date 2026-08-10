//! MLP / Feed Forward – Integer

use crate::fixed_point::{clamp_i8, rshift_round};
use crate::linear::linear_w8a8;
use crate::integer_math::lut_lookup;

/// Integer-MLP mit SiLU-Approximation via LUT.
///
/// `gate_frac_bits`/`up_frac_bits`/`down_frac_bits` sind die kalibrierten
/// Gewichtsskalen der drei Projektionen (je Tensor unterschiedlich, siehe
/// `QTensor.shift`) - anders als `act_frac_bits`/`out_frac_bits`, die den
/// internen Arbeits- bzw. Ziel-Skalenbereich der Aktivierungen festlegen und
/// an den SiLU-LUT-Kalibrierungsbereich gebunden bleiben (siehe Aufrufer).
pub fn mlp_int(
    x: &[i8],
    W_gate: &[Vec<i8>],
    W_up: &[Vec<i8>],
    W_down: &[Vec<i8>],
    silu_lut: &[i16],
    act_frac_bits: u8,
    gate_frac_bits: u8,
    up_frac_bits: u8,
    down_frac_bits: u8,
    out_frac_bits: u8,
    act_lut_shift: u8,
    act_lut_offset: i16,
) -> Vec<i8> {
    let gate = linear_w8a8(x, W_gate, act_frac_bits, gate_frac_bits, act_frac_bits);
    let up = linear_w8a8(x, W_up, act_frac_bits, up_frac_bits, act_frac_bits);

    let mut h = Vec::with_capacity(gate.len());
    for (g, u) in gate.iter().zip(up.iter()) {
        let activated = lut_lookup(*g as i16, silu_lut, act_lut_shift, act_lut_offset);
        let prod = (activated as i32) * (*u as i32);
        h.push(clamp_i8(rshift_round(prod, act_frac_bits)));
    }

    linear_w8a8(&h, W_down, act_frac_bits, down_frac_bits, out_frac_bits)
}
