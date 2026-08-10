//! Backend-Trait – Abstraktion fuer heterogene Hardware
//!
//! Jeder Backend muss gegen die Referenz-Implementierung validiert werden.
//! Golden Vectors sind die normative Wahrheit.

/// Ein numerisches Backend fuer Integer-Inferenz.
/// Alle Methoden muessen bit-identisch zur Referenz sein (Golden-Vector-Test).
pub trait Backend {
    /// Eindeutiger Name fuer Logging und Manifeste.
    fn name(&self) -> &'static str;

    /// Hardware-Familie fuer das Pipeline-Manifest.
    fn hardware_family(&self) -> &'static str;

    /// Feature-String fuer Cargo (z.B. "reference", "cpu-simd-avx2", "cuda-sm80").
    fn feature_tag(&self) -> &'static str;

    // ==============================
    // Kern-Operationen
    // ==============================

    /// W8A8 Matrix-Vektor: y = clamp(rescale(W * x))
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
    );

    /// RMSNorm: y = x / rsqrt(mean_sq + eps) * gamma
    fn rmsnorm(
        &self,
        x: &[i8],
        gamma: &[i8],
        out: &mut [i8],
        frac_bits: u8,
        eps: i32,
    );

    /// Softmax-Approximation via exp-LUT.
    fn softmax(
        &self,
        logits: &[i32],
        out: &mut [i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    );

    /// Attention: Q*K^T -> softmax -> *V
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
    );

    /// RoPE: Rotiere Q/K um Sin/Cos-LUT.
    fn rope(
        &self,
        q: &mut [Vec<i8>],
        k: &mut [Vec<i8>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    );

    /// MLP: gate = SiLU(W_gate * x) * (W_up * x); out = W_down * gate
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
    );
}

/// Metadaten fuer ein validiertes Backend.
#[derive(Debug, Clone)]
pub struct BackendCertification {
    pub backend_name: String,
    pub hardware_family: String,
    pub feature_tag: String,
    pub theta_v_hash: String,
    pub golden_vector_hash: String,
    pub validated_at: String,  // ISO 8601
    pub test_suite_version: String,
}
