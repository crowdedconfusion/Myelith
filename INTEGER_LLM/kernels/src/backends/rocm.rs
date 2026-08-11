//! AMD ROCm/HIP-Backend
//!
//! Feature-Gate: `cargo build --features rocm`
//!
//! WARNUNG: Dieses Backend darf NUR aktiviert werden, wenn es die
//! Golden Vectors gegen das Referenz-Backend besteht.
//!
//! Determinismus-Strategie:
//! - 1:1-Port des CUDA-Codes nach HIP (95% identisch)
//! - AMD WarpSize = 64 (vs. NVIDIA = 32) -> Keine Warp-Angewiesenheit
//! - Shared Memory fuer Reductions statt Warp-Shuffle
//! - Separater Golden-Vector-Test-Suite fuer AMD-Hardware
//!
//! Ziel-Vertrag seit dem Numerik-Realitaetsabgleich (v0.12.20, theta_v 0.5.0):
//! Gewichte int8, Aktivierungen int16 mit Per-Layer-Zweierpotenz-Skalen,
//! i64-Akkumulation, divisionsfreie RMSNorm mit LUT-gestuetztem rsqrt.

use crate::backend::Backend;

pub struct RocmBackend {
    device_id: usize,
    gcn_arch: String,  // z.B. "gfx908" (MI100), "gfx90a" (MI200)
}

impl RocmBackend {
    pub fn init(device_id: usize) -> Result<Self, String> {
        // TODO: HIP-Kontext initialisieren, GCN-Arch ermitteln
        Ok(RocmBackend {
            device_id,
            gcn_arch: "gfx90a".to_string(),  // Placeholder
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
        _x: &[i16],
        _W: &[i8],
        _out: &mut [i16],
        _in_features: usize,
        _out_features: usize,
        _act_frac: u8,
        _weight_frac: u8,
        _out_frac: u8,
    ) {
        // TODO: HIP-Kernel-Launch (1:1-Port von CUDA)
        panic!("ROCm linear_w8a16 not yet implemented");
    }

    fn rmsnorm(
        &self,
        _x: &[i16],
        _gamma: &[i8],
        _gamma_shift: u8,
        _rsqrt_lut: &[i16],
        _lut_input_shift: u8,
        _lut_output_frac: u8,
        _inv_n_q20: i64,
        _out: &mut [i16],
        _out_frac: u8,
    ) {
        // TODO: HIP-Kernel-Launch
        panic!("ROCm rmsnorm not yet implemented");
    }

    fn softmax(
        &self,
        _logits: &[i32],
        _out: &mut [i32],
        _exp_lut: &[i16],
        _lut_shift: u8,
        _frac_bits: u8,
    ) {
        // TODO: HIP-Kernel-Launch
        panic!("ROCm softmax not yet implemented");
    }

    fn attention(
        &self,
        _q: &[Vec<i16>],
        _k: &[Vec<i16>],
        _v: &[Vec<i16>],
        _out: &mut [Vec<i16>],
        _mask: &[Vec<bool>],
        _score_shift: u8,
        _exp_lut: &[i16],
        _lut_shift: u8,
        _prob_frac: u8,
    ) {
        // TODO: HIP-Kernel-Launch
        panic!("ROCm attention not yet implemented");
    }

    fn rope(
        &self,
        _q: &mut [Vec<i16>],
        _k: &mut [Vec<i16>],
        _cos_lut: &[i16],
        _sin_lut: &[i16],
        _positions: &[usize],
        _frac_bits: u8,
    ) {
        // TODO: HIP-Kernel-Launch
        panic!("ROCm rope not yet implemented");
    }

    fn mlp(
        &self,
        _x: &[i16],
        _W_gate: &[i8],
        _W_up: &[i8],
        _W_down: &[i8],
        _out: &mut [i16],
        _silu_lut: &[i16],
        _in_frac: u8,
        _gate_w_shift: u8,
        _up_w_shift: u8,
        _down_w_shift: u8,
        _gate_out_frac: u8,
        _up_out_frac: u8,
        _down_in_frac: u8,
        _silu_in_frac: u8,
        _silu_lut_offset: i16,
        _silu_out_frac: u8,
        _out_frac: u8,
    ) {
        // TODO: HIP-Kernel-Launch
        panic!("ROCm mlp not yet implemented");
    }
}
