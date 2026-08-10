//! Referenz-Backend – Pure Rust, portabel, langsam, aber normativ.
//! 
//! Dies ist die "Single Source of Truth" fuer alle numerischen Ergebnisse.
//! Jedes andere Backend muss gegen dieses hier validiert werden.

use crate::backend::Backend;
use crate::fixed_point::{clamp_i8, rescale};
use crate::rmsnorm::rmsnorm_int8;
use crate::linear::linear_w8a8;
use crate::softmax::softmax_int;
use crate::attention::attention_int;
use crate::rope::apply_rope;
use crate::mlp::mlp_int;

pub struct ReferenceBackend;

impl ReferenceBackend {
    pub fn new() -> Self {
        ReferenceBackend
    }
}

impl Backend for ReferenceBackend {
    fn name(&self) -> &'static str {
        "reference"
    }

    fn hardware_family(&self) -> &'static str {
        "cpu-generic"
    }

    fn feature_tag(&self) -> &'static str {
        "reference"
    }

    fn linear_w8a8(
        &self,
        x: &[i8],
        W: &[i8],
        out: &mut [i8],
        in_features: usize,
        out_features: usize,
        act_frac: u8,
        weight_frac: u8,
        out_frac: u8,
    ) {
        // W ist als flat [out_features * in_features] gespeichert
        for row in 0..out_features {
            let mut acc: i32 = 0;
            for col in 0..in_features {
                acc += (W[row * in_features + col] as i32) * (x[col] as i32);
            }
            out[row] = clamp_i8(rescale(acc, act_frac + weight_frac, out_frac));
        }
    }

    fn rmsnorm(
        &self,
        x: &[i8],
        gamma: &[i8],
        out: &mut [i8],
        frac_bits: u8,
        eps: i32,
    ) {
        let result = rmsnorm_int8(x, gamma, frac_bits, eps);
        out.copy_from_slice(&result);
    }

    fn softmax(
        &self,
        logits: &[i32],
        out: &mut [i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    ) {
        let result = softmax_int(logits, exp_lut, lut_shift, frac_bits);
        out.copy_from_slice(&result);
    }

    fn attention(
        &self,
        q: &[Vec<i8>],
        k: &[Vec<i8>],
        v: &[Vec<i8>],
        out: &mut [Vec<i8>],
        mask: &[Vec<bool>],
        score_shift: u8,
        exp_lut: &[i16],
        lut_shift: u8,
        prob_frac: u8,
    ) {
        let result = attention_int(q, k, v, mask, score_shift, exp_lut, lut_shift, prob_frac);
        for (i, row) in result.iter().enumerate() {
            out[i].copy_from_slice(row);
        }
    }

    fn rope(
        &self,
        q: &mut [Vec<i8>],
        k: &mut [Vec<i8>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    ) {
        let (q_out, k_out) = apply_rope(q, k, cos_lut, sin_lut, positions, frac_bits);
        for (i, row) in q_out.iter().enumerate() {
            q[i].copy_from_slice(row);
        }
        for (i, row) in k_out.iter().enumerate() {
            k[i].copy_from_slice(row);
        }
    }

    fn mlp(
        &self,
        x: &[i8],
        W_gate: &[i8],
        W_up: &[i8],
        W_down: &[i8],
        out: &mut [i8],
        silu_lut: &[i16],
        act_frac: u8,
        weight_frac: u8,
        out_frac: u8,
        lut_shift: u8,
        lut_offset: i16,
    ) {
        let hidden_size = x.len();
        let intermediate_size = W_gate.len() / hidden_size;
        assert_eq!(W_up.len(), intermediate_size * hidden_size);
        assert_eq!(W_down.len(), hidden_size * intermediate_size);

        let gate: Vec<Vec<i8>> = W_gate.chunks(hidden_size).map(|c| c.to_vec()).collect();
        let up: Vec<Vec<i8>> = W_up.chunks(hidden_size).map(|c| c.to_vec()).collect();
        let down: Vec<Vec<i8>> = W_down.chunks(intermediate_size).map(|c| c.to_vec()).collect();

        // Der Backend-Trait kennt nur eine gemeinsame Gewichtsskala (fuer
        // Golden-Vector-Tests mit synthetischen, gleich skalierten Gewichten
        // ausreichend); reale, pro Tensor kalibrierte Skalen verwendet die
        // Modell-Ebene direkt (runtime/src/model.rs), nicht dieser Pfad.
        let result = mlp_int(
            x,
            &gate, &up, &down,
            silu_lut, act_frac, weight_frac, weight_frac, weight_frac, out_frac,
            lut_shift, lut_offset,
        );
        out.copy_from_slice(&result);
    }
}
