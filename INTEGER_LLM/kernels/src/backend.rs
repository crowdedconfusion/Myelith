//! Backend-Trait – Abstraktion fuer heterogene Hardware
//!
//! Jeder Backend muss gegen die Referenz-Implementierung validiert werden.
//! Golden Vectors sind die normative Wahrheit.
//!
//! Numerik-Vertrag seit theta_v 0.5.0 (Numerik-Realitaetsabgleich v0.12.20):
//! Gewichte int8, Aktivierungen int16 mit kalibrierten Per-Layer-
//! Zweierpotenz-Skalen, Residualstrom int16 frac 3.
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]
// Die Backend-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag
// (frac_bits, Per-Channel-Shifts, LUT-Parameter). In eine Parameter-Struct
// gefasst waere die Entsprechung zu den Referenzformeln (Anhang B) beim
// Nachrechnen nicht mehr ablesbar — und genau dieses Nachrechnen ist die
// Pruefmethode des Projekts.
#![allow(clippy::too_many_arguments)]

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

    /// W8A16 Matrix-Vektor: y = clamp(rescale(W * x)), Per-Channel-
    /// Gewichtsskalen (theta_v 0.7.0: ein Zweierpotenz-Shift je Ausgabe-Zeile).
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
    );

    /// RMSNorm: y = x * rsqrt(mean(x^2)) * gamma (int16, LUT-gestuetzt,
    /// dynamischer gerader Index-Shift, divisionsfrei; Gamma mit
    /// Per-Element-Skalen, theta_v 0.7.0).
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

    /// Attention: Q*K^T -> softmax -> *V (i64-Akkumulation)
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
    );

    /// RoPE: Rotiere Q/K um Sin/Cos-LUT (skaleninvariant).
    fn rope(
        &self,
        q: &mut [Vec<i16>],
        k: &mut [Vec<i16>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    );

    /// MLP: gate = SiLU(W_gate * x) * (W_up * x); out = W_down * gate
    /// (Per-Layer-Skalen fuer alle Zwischenstufen).
    #[allow(clippy::too_many_arguments)]
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
