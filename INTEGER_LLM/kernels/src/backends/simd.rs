//! SIMD-Backend fuer x86_64 (AVX2/AVX-512) und ARM64 (Neon)
//!
//! Feature-Gate: `cargo build --features cpu-simd`
//!
//! WARNUNG: Dieses Backend darf NUR aktiviert werden, wenn es die
//! Golden Vectors gegen das Referenz-Backend besteht.
//!
//! Stand nach dem Numerik-Realitaetsabgleich (v0.12.20, theta_v 0.5.0):
//! Die frueheren AVX2-Intrinsics zielten auf den alten Numerik-Vertrag
//! (int8-Aktivierungen, divisionsbehaftete RMSNorm) und wurden mit dem
//! Vertragswechsel entfernt (Git-Historie). Bis zum SIMD-Neuaufbau in
//! Fahrplan-Phase 12.35–12.39 delegiert dieses Backend an die
//! Referenz-Kernel — numerisch identisch, nicht vektorisiert.

use crate::attention::attention_int;
use crate::backend::Backend;
use crate::linear::linear_w8a16;
use crate::mlp::mlp_int;
use crate::rmsnorm::rmsnorm_i16;
use crate::rope::apply_rope_i16;
use crate::softmax::softmax_int;

pub struct SimdBackend {
    target: SimdTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum SimdTarget {
    Avx2,
    Avx512,
    Neon,
}

impl SimdBackend {
    pub fn detect() -> Option<Self> {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        return Some(SimdBackend { target: SimdTarget::Avx512 });

        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        return Some(SimdBackend { target: SimdTarget::Avx2 });

        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        return Some(SimdBackend { target: SimdTarget::Neon });

        None
    }

    pub fn target(&self) -> SimdTarget {
        self.target
    }

    /// Hilfsfunktion: flat i8-Array -> Vec<Vec<i8>> fuer die Kernel-Aufrufe.
    fn flat_to_vec_vec(flat: &[i8], cols: usize) -> Vec<Vec<i8>> {
        flat.chunks(cols).map(|c| c.to_vec()).collect()
    }
}

impl Backend for SimdBackend {
    fn name(&self) -> &'static str {
        match self.target {
            SimdTarget::Avx2 => "simd-avx2",
            SimdTarget::Avx512 => "simd-avx512",
            SimdTarget::Neon => "simd-neon",
        }
    }

    fn hardware_family(&self) -> &'static str {
        "cpu-simd"
    }

    fn feature_tag(&self) -> &'static str {
        match self.target {
            SimdTarget::Avx2 => "cpu-simd-avx2",
            SimdTarget::Avx512 => "cpu-simd-avx512",
            SimdTarget::Neon => "cpu-simd-neon",
        }
    }

    fn linear_w8a16(
        &self,
        x: &[i16],
        W: &[i8],
        out: &mut [i16],
        in_features: usize,
        out_features: usize,
        act_frac: u8,
        weight_frac: u8,
        out_frac: u8,
    ) {
        let rows = Self::flat_to_vec_vec(W, in_features);
        assert_eq!(rows.len(), out_features);
        let result = linear_w8a16(x, &rows, act_frac, weight_frac, out_frac);
        out.copy_from_slice(&result);
    }

    fn rmsnorm(
        &self,
        x: &[i16],
        gamma: &[i8],
        gamma_shift: u8,
        rsqrt_lut: &[i16],
        lut_input_shift: u8,
        lut_output_frac: u8,
        inv_n_q20: i64,
        out: &mut [i16],
        out_frac: u8,
    ) {
        let result = rmsnorm_i16(x, gamma, gamma_shift, rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
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
        q: &[Vec<i16>],
        k: &[Vec<i16>],
        v: &[Vec<i16>],
        out: &mut [Vec<i16>],
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
        q: &mut [Vec<i16>],
        k: &mut [Vec<i16>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    ) {
        let (q_out, k_out) = apply_rope_i16(q, k, cos_lut, sin_lut, positions, frac_bits);
        for (i, row) in q_out.iter().enumerate() {
            q[i].copy_from_slice(row);
        }
        for (i, row) in k_out.iter().enumerate() {
            k[i].copy_from_slice(row);
        }
    }

    fn mlp(
        &self,
        x: &[i16],
        W_gate: &[i8],
        W_up: &[i8],
        W_down: &[i8],
        out: &mut [i16],
        silu_lut: &[i16],
        in_frac: u8,
        gate_w_shift: u8,
        up_w_shift: u8,
        down_w_shift: u8,
        gate_out_frac: u8,
        up_out_frac: u8,
        down_in_frac: u8,
        silu_in_frac: u8,
        silu_lut_offset: i16,
        silu_out_frac: u8,
        out_frac: u8,
    ) {
        let hidden_size = x.len();
        let intermediate_size = W_gate.len() / hidden_size;
        assert_eq!(W_up.len(), intermediate_size * hidden_size);
        assert_eq!(W_down.len(), hidden_size * intermediate_size);

        let gate = Self::flat_to_vec_vec(W_gate, hidden_size);
        let up = Self::flat_to_vec_vec(W_up, hidden_size);
        let down = Self::flat_to_vec_vec(W_down, intermediate_size);
        let result = mlp_int(
            x,
            &gate, &up, &down,
            silu_lut,
            in_frac,
            gate_w_shift, up_w_shift, down_w_shift,
            gate_out_frac, up_out_frac, down_in_frac,
            silu_in_frac, silu_lut_offset, silu_out_frac,
            out_frac,
        );
        out.copy_from_slice(&result);
    }
}
