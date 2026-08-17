//! AMD ROCm/HIP-Backend
//!
//! Feature-Gate: `cargo build --features rocm`
//!
//! Status: Delegations-Stub — alle Operationen werden an die Referenz-
//! Kernel delegiert (numerisch identisch, nicht beschleunigt). Echte
//! HIP-Kernels erfordern AMD-GPU-Hardware zum Testen.
//!
//! Determinismus-Strategie (fuer zukuenftige echte HIP-Kernels):
//! - 1:1-Port des CUDA-Codes nach HIP (95% syntaktisch identisch)
//! - AMD WarpSize = 64 (vs. NVIDIA = 32) → keine Warp-Angewiesenheit
//! - Shared Memory fuer Reductions statt Warp-Shuffle
//! - Separate Golden-Vector-Test-Suite fuer AMD-Hardware
//!
//! Ziel-Vertrag seit theta_v 0.7.0:
//! Gewichte int8 (Per-Channel-Skalen), Aktivierungen int16 (Per-Layer-Skalen),
//! i64-Akkumulation, divisionsfreie RMSNorm mit LUT-gestuetztem rsqrt,
//! RNE-Rundung, Saettigung (Clamp).

use crate::backend::Backend;
use crate::linear::linear_w8a16;
use crate::rmsnorm::rmsnorm_i16;
use crate::softmax::softmax_int;
use crate::attention::attention_int;
use crate::rope::apply_rope_i16;
use crate::mlp::mlp_int;

pub struct RocmBackend {
    device_id: usize,
    gcn_arch: String,
}

impl RocmBackend {
    /// Initialisiert das ROCm/HIP-Backend.
    ///
    /// Hinweis: Ohne HIP-Runtime (hipcc, libamdhip64) wird eine Platzhalter-
    /// Architektur zurueckgegeben. Echte Initialisierung erfordert:
    /// - hipSetDevice(device_id)
    /// - hipDeviceGetAttribute fuer GCN-Architektur
    /// - hipBLAS/hipDNN-Handles (fuer zukuenftige beschleunigte Pfade)
    pub fn init(device_id: usize) -> Result<Self, String> {
        // TODO: Echte HIP-Initialisierung wenn Runtime verfuegbar
        Ok(RocmBackend {
            device_id,
            gcn_arch: "gfx90a".to_string(), // Placeholder: MI200
        })
    }

    pub fn device_id(&self) -> usize {
        self.device_id
    }

    pub fn gcn_arch(&self) -> &str {
        &self.gcn_arch
    }
}

impl Backend for RocmBackend {
    fn name(&self) -> &'static str {
        "rocm"
    }

    fn hardware_family(&self) -> &'static str {
        "amd-gpu"
    }

    fn feature_tag(&self) -> &'static str {
        "rocm"
    }

    fn linear_w8a16(
        &self,
        x: &[i16],
        W: &[i8],
        out: &mut [i16],
        in_features: usize,
        out_features: usize,
        w_shifts: &[u8],
        act_frac: u8,
        out_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — 1:1-Port von CUDA, WarpSize 64 beachten.
        let rows: Vec<Vec<i8>> = W.chunks(in_features).map(|c| c.to_vec()).collect();
        let result = linear_w8a16(x, &rows, w_shifts, act_frac, out_frac);
        out[..out_features].copy_from_slice(&result[..out_features]);
    }

    fn rmsnorm(
        &self,
        x: &[i16],
        gamma: &[i8],
        gamma_shifts: &[u8],
        rsqrt_lut: &[i16],
        lut_input_shift: u8,
        lut_output_frac: u8,
        inv_n_q20: i64,
        out: &mut [i16],
        out_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — Shared-Memory-Reduktion (Wavefront-Size 64).
        let result = rmsnorm_i16(x, gamma, gamma_shifts, rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
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
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — Workgroup-Reduce fuer Max und Summe.
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
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — Flash-Attention-Port, Workgroup-Sync.
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
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — Thread-per-Pair, Wavefront-Size 64.
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
        gate_w_shifts: &[u8],
        up_w_shifts: &[u8],
        down_w_shifts: &[u8],
        silu_lut: &[i16],
        in_frac: u8,
        gate_out_frac: u8,
        up_out_frac: u8,
        down_in_frac: u8,
        silu_in_frac: u8,
        silu_lut_offset: i16,
        silu_out_frac: u8,
        out_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter HIP-Kernel — Fused Gate+Up+SiLU+Down.
        let hidden_size = x.len();
        let intermediate_size = W_gate.len() / hidden_size;

        let gate: Vec<Vec<i8>> = W_gate.chunks(hidden_size).map(|c| c.to_vec()).collect();
        let up: Vec<Vec<i8>> = W_up.chunks(hidden_size).map(|c| c.to_vec()).collect();
        let down: Vec<Vec<i8>> = W_down.chunks(intermediate_size).map(|c| c.to_vec()).collect();

        let result = mlp_int(
            x,
            &gate, &up, &down,
            gate_w_shifts, up_w_shifts, down_w_shifts,
            silu_lut,
            in_frac,
            gate_out_frac, up_out_frac, down_in_frac,
            silu_in_frac, silu_lut_offset, silu_out_frac,
            out_frac,
        );
        out.copy_from_slice(&result);
    }
}
