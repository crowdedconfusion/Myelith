//! MLP / Feed Forward – Integer (Aktivierungen int16, Per-Layer-Skalen)
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};
use crate::integer_math::lut_lookup;
use crate::linear::{linear_w8a16, linear_w8a16_pc};

/// Integer-MLP mit SiLU-Approximation via LUT.
///
/// Skalen (alles kalibrierte Per-Layer-Zweierpotenz-Skalen, siehe
/// `scales.json`):
/// - `in_frac_bits`: Eingang (Ausgabe der post_attention_layernorm)
/// - `gate_out_frac`/`up_out_frac`: Ausgaenge von gate-/up-Projektion
/// - `down_in_frac`: Eingang von down_proj (h = silu(gate)*up)
/// - `out_frac_bits`: Ausgangsskala JE KANAL (Fund 20, theta_v 0.11.0) -
///   down_proj addiert direkt in den Residualstrom, der seit Fund 20 eine
///   Skala je Kanal trägt (Massive Activations bei Qwen2.5-7B)
///
/// Die SiLU-LUT arbeitet in einer festen Domäne (`silu_in_frac`, Index-
/// Offset `silu_lut_offset` = -input_min der spec): Gate-Werte werden vor
/// dem Lookup in diese Domäne reskaliert; große Betragswerte saturieren
/// deterministisch am LUT-Rand.
#[allow(clippy::too_many_arguments)]
pub fn mlp_int(
    x: &[i16],
    W_gate: &[Vec<i8>],
    W_up: &[Vec<i8>],
    W_down: &[Vec<i8>],
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    let gate = linear_w8a16(x, W_gate, gate_w_shifts, in_frac_bits, gate_out_frac);
    let up = linear_w8a16(x, W_up, up_w_shifts, in_frac_bits, up_out_frac);

    let mut h = Vec::with_capacity(gate.len());
    for (g, u) in gate.iter().zip(up.iter()) {
        // Gate in die feste LUT-Domäne reskalieren, Lookup, dann Produkt mit
        // up auf die kalibrierte down-Eingangsskala bringen.
        let g_dom = rescale(*g as i32, gate_out_frac, silu_in_frac);
        let activated = lut_lookup(g_dom as i16, silu_lut, 0, silu_lut_offset);
        let prod = (activated as i64) * (*u as i64);
        h.push(clamp_i16_from_i64(rescale_i64(
            prod,
            silu_out_frac + up_out_frac,
            down_in_frac,
        )));
    }

    linear_w8a16_pc(&h, W_down, down_w_shifts, down_in_frac, out_frac_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SiLU-LUT im spec-Format (input_frac 1, output_frac 6, [-256, 255]).
    fn spec_silu_lut() -> Vec<i16> {
        let mut lut = Vec::with_capacity(512);
        for x in -256..256 {
            let xf = x as f64 / 2.0;
            let val = xf * (1.0 / (1.0 + (-xf).exp()));
            lut.push((val * 64.0).round() as i16);
        }
        lut
    }

    #[test]
    fn test_mlp_runs_with_per_layer_scales() {
        // Rauchtest: 2 Kanaele, intermediate 2; alle Skalen explizit.
        let x = vec![64i16, -32];
        let w_gate = vec![vec![64i8, 0], vec![0, 64]];
        let w_up = vec![vec![64i8, 0], vec![0, 64]];
        let w_down = vec![vec![64i8, 32], vec![32, 64]];
        let lut = spec_silu_lut();
        let out = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            &[6, 6], &[6, 6], &[6, 6], // Per-Channel-Gewichts-Shifts
            &lut,
            6,   // in_frac
            6, 6, 6, // gate/up/down-Eingangs-Skalen
            1, 256, 6, // SiLU-Domäne (frac 1, Offset 256, Output frac 6)
            &[6, 6],   // out_frac (Fund 20: je Kanal, hier uniform)
        );
        assert_eq!(out.len(), 2);
        // Alle Werte muessen im i16-Bereich und deterministisch sein.
        let out2 = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            &[6, 6], &[6, 6], &[6, 6],
            &lut,
            6, 6, 6, 6, 1, 256, 6, &[6, 6],
        );
        assert_eq!(out, out2);
    }
}
