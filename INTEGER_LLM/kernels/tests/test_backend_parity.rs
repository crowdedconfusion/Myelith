//! Backend-Paritaetstest: SimdBackend muss bit-identisch zum ReferenceBackend
//! sein. Auf x86_64 testet dies die AVX2-Implementierungen; auf ARM64 testet
//! es den Fallback-Pfad (der identisch zur Referenz ist).
//!
//! Dieser Test ist die normative Garantie dafuer, dass kein Backend jemals
//! numerisch von der Referenz abweicht — die Kerneigenschaft des Projekts.

use integer_llm_kernels::backend::Backend;
use integer_llm_kernels::backends::reference::ReferenceBackend;

#[cfg(feature = "cpu-simd")]
use integer_llm_kernels::backends::simd::SimdBackend;

// =====================================================================
// Test-Daten
// =====================================================================

fn test_exp_lut() -> Vec<i16> {
    // exp-LUT: 1025 Eintraege, exp(-x/16) * 256, frac_bits = 8
    (0..=1024).map(|i| {
        let val = (-(i as f64) / 16.0).exp() * 256.0;
        val.round() as i16
    }).collect()
}

fn test_silu_lut() -> Vec<i16> {
    // SiLU-LUT: 2048 Eintraege, Domäne [-1024, 1023]
    (0..2048).map(|i| {
        let x = (i as f64 - 1024.0) / 8.0; // silu_in_frac = 3
        let silu = x * (1.0 / (1.0 + (-x).exp()));
        (silu * 64.0).round() as i16 // silu_out_frac = 6
    }).collect()
}

fn test_rsqrt_lut() -> Vec<i16> {
    // rsqrt-LUT: 1024 Eintraege, input_shift = 8, output_frac = 8
    (0..1024).map(|i| {
        if i == 0 {
            256 // 1.0 * 2^8
        } else {
            let real = i as f64 / 256.0;
            (1.0 / real.sqrt() * 256.0).round() as i16
        }
    }).collect()
}

// =====================================================================
// Softmax-Paritaet
// =====================================================================

#[cfg(feature = "cpu-simd")]
#[test]
fn softmax_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };
    let exp_lut = test_exp_lut();
    let frac_bits = 8u8;
    let lut_shift = 4u8;

    let logits = vec![100i32, 200, 50, 300, 150, 250, 75, 175];
    let mut ref_out = vec![0i32; logits.len()];
    let mut simd_out = vec![0i32; logits.len()];

    ref_backend.softmax(&logits, &mut ref_out, &exp_lut, lut_shift, frac_bits);
    simd_backend.softmax(&logits, &mut simd_out, &exp_lut, lut_shift, frac_bits);

    assert_eq!(ref_out, simd_out, "Softmax: SIMD weicht von Referenz ab");
}

#[cfg(feature = "cpu-simd")]
#[test]
fn softmax_parity_edge_cases() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };
    let exp_lut = test_exp_lut();
    let frac_bits = 8u8;
    let lut_shift = 4u8;

    // Alle gleich
    let logits = vec![100i32; 16];
    let mut ref_out = vec![0i32; 16];
    let mut simd_out = vec![0i32; 16];
    ref_backend.softmax(&logits, &mut ref_out, &exp_lut, lut_shift, frac_bits);
    simd_backend.softmax(&logits, &mut simd_out, &exp_lut, lut_shift, frac_bits);
    assert_eq!(ref_out, simd_out, "Softmax equal logits");

    // Einzelner großer Ausreisser
    let mut logits2 = vec![10i32; 32];
    logits2[15] = 500;
    let mut ref_out2 = vec![0i32; 32];
    let mut simd_out2 = vec![0i32; 32];
    ref_backend.softmax(&logits2, &mut ref_out2, &exp_lut, lut_shift, frac_bits);
    simd_backend.softmax(&logits2, &mut simd_out2, &exp_lut, lut_shift, frac_bits);
    assert_eq!(ref_out2, simd_out2, "Softmax outlier");

    // Negative Werte
    let logits3 = vec![-100i32, -200, -50, -300, -150, -250, -75, -175];
    let mut ref_out3 = vec![0i32; 8];
    let mut simd_out3 = vec![0i32; 8];
    ref_backend.softmax(&logits3, &mut ref_out3, &exp_lut, lut_shift, frac_bits);
    simd_backend.softmax(&logits3, &mut simd_out3, &exp_lut, lut_shift, frac_bits);
    assert_eq!(ref_out3, simd_out3, "Softmax negative");
}

// =====================================================================
// RoPE-Paritaet
// =====================================================================

#[cfg(feature = "cpu-simd")]
#[test]
fn rope_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };
    let frac_bits = 8u8;
    let head_dim = 64;
    let half = head_dim / 2;
    let max_seq = 4;

    // cos/sin LUTs: flat [max_seq * half]
    let cos_lut: Vec<i16> = (0..max_seq * half).map(|i| {
        let pos = i / half;
        let freq = i % half;
        let theta = pos as f64 / (1000000.0f64.powf(2.0 * freq as f64 / head_dim as f64));
        (theta.cos() * 256.0).round() as i16
    }).collect();
    let sin_lut: Vec<i16> = (0..max_seq * half).map(|i| {
        let pos = i / half;
        let freq = i % half;
        let theta = pos as f64 / (1000000.0f64.powf(2.0 * freq as f64 / head_dim as f64));
        (theta.sin() * 256.0).round() as i16
    }).collect();

    let mut q_ref = vec![vec![100i16; head_dim]; 2];
    let mut k_ref = vec![vec![50i16; head_dim]; 2];
    let mut q_simd = q_ref.clone();
    let mut k_simd = k_ref.clone();
    let positions = vec![0usize, 1];

    ref_backend.rope(&mut q_ref, &mut k_ref, &cos_lut, &sin_lut, &positions, frac_bits);
    simd_backend.rope(&mut q_simd, &mut k_simd, &cos_lut, &sin_lut, &positions, frac_bits);

    assert_eq!(q_ref, q_simd, "RoPE Q: SIMD weicht von Referenz ab");
    assert_eq!(k_ref, k_simd, "RoPE K: SIMD weicht von Referenz ab");
}

// =====================================================================
// Linear-Paritaet
// =====================================================================

#[cfg(feature = "cpu-simd")]
#[test]
fn linear_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };

    let in_features = 64;
    let out_features = 32;
    let x: Vec<i16> = (0..in_features).map(|i| (i as i16) * 7 - 200).collect();
    let w: Vec<i8> = (0..out_features * in_features).map(|i| ((i % 120) as i32 - 60) as i8).collect();
    let w_shifts: Vec<u8> = (0..out_features).map(|i| (i % 8) as u8 + 4).collect();
    let act_frac = 6u8;
    let out_frac = 6u8;

    let mut ref_out = vec![0i16; out_features];
    let mut simd_out = vec![0i16; out_features];

    ref_backend.linear_w8a16(&x, &w, &mut ref_out, in_features, out_features, &w_shifts, act_frac, out_frac);
    simd_backend.linear_w8a16(&x, &w, &mut simd_out, in_features, out_features, &w_shifts, act_frac, out_frac);

    assert_eq!(ref_out, simd_out, "Linear: SIMD weicht von Referenz ab");
}

// =====================================================================
// RMSNorm-Paritaet
// =====================================================================

#[cfg(feature = "cpu-simd")]
#[test]
fn rmsnorm_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };
    let rsqrt_lut = test_rsqrt_lut();
    let n = 64;

    let x: Vec<i16> = (0..n).map(|i| (i as i16) * 10 - 300).collect();
    let gamma: Vec<i8> = (0..n).map(|i| 40 + (i % 30) as i8).collect();
    let gamma_shifts: Vec<u8> = (0..n).map(|i| 5 + (i % 4) as u8).collect();
    let inv_n_q20 = ((1i64 << 20) + n as i64 / 2) / n as i64;

    let mut ref_out = vec![0i16; n];
    let mut simd_out = vec![0i16; n];

    ref_backend.rmsnorm(&x, &gamma, &gamma_shifts, &rsqrt_lut, 8, 8, inv_n_q20, &mut ref_out, 6);
    simd_backend.rmsnorm(&x, &gamma, &gamma_shifts, &rsqrt_lut, 8, 8, inv_n_q20, &mut simd_out, 6);

    assert_eq!(ref_out, simd_out, "RMSNorm: SIMD weicht von Referenz ab");
}

// =====================================================================
// MLP-Paritaet
// =====================================================================

#[cfg(feature = "cpu-simd")]
#[test]
fn mlp_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let simd_backend = match SimdBackend::detect() {
        Some(b) => b,
        None => return, // SIMD nicht verfuegbar — Test ueberspringen
    };
    let silu_lut = test_silu_lut();

    let hidden = 32;
    let intermediate = 64;
    let x: Vec<i16> = (0..hidden).map(|i| (i as i16) * 5 - 80).collect();
    let w_gate: Vec<i8> = (0..intermediate * hidden).map(|i| ((i % 120) as i32 - 60) as i8).collect();
    let w_up: Vec<i8> = (0..intermediate * hidden).map(|i| ((i % 100) as i32 - 50) as i8).collect();
    let w_down: Vec<i8> = (0..hidden * intermediate).map(|i| ((i % 80) as i32 - 40) as i8).collect();
    let gate_shifts: Vec<u8> = (0..intermediate).map(|i| 4 + (i % 6) as u8).collect();
    let up_shifts: Vec<u8> = (0..intermediate).map(|i| 5 + (i % 5) as u8).collect();
    let down_shifts: Vec<u8> = (0..hidden).map(|i| 4 + (i % 7) as u8).collect();

    let mut ref_out = vec![0i16; hidden];
    let mut simd_out = vec![0i16; hidden];

    ref_backend.mlp(
        &x, &w_gate, &w_up, &w_down, &mut ref_out,
        &gate_shifts, &up_shifts, &down_shifts,
        &silu_lut,
        6, 8, 8, 8, 3, 1024, 6, 6,
    );
    simd_backend.mlp(
        &x, &w_gate, &w_up, &w_down, &mut simd_out,
        &gate_shifts, &up_shifts, &down_shifts,
        &silu_lut,
        6, 8, 8, 8, 3, 1024, 6, 6,
    );

    assert_eq!(ref_out, simd_out, "MLP: SIMD weicht von Referenz ab");
}
