//! Backend-Paritaetstest: SimdBackend muss bit-identisch zum ReferenceBackend
//! sein. Auf x86_64 prueft das die AVX2-Kernel, auf aarch64 die NEON-Kernel.
//!
//! Dieser Test ist die normative Garantie dafuer, dass kein Backend jemals
//! numerisch von der Referenz abweicht, und das ist die Kerneigenschaft des
//! Projekts.
//!
//! ## ⛑ Drei Berichtigungen an dieser Datei (2026-08-30)
//!
//! **Der Kopf war falsch.** Hier stand, auf ARM64 werde "der Fallback-Pfad
//! (der identisch zur Referenz ist)" geprueft. Das trifft nicht zu:
//! `SimdBackend::detect()` liefert auf aarch64 `SimdTarget::Neon`, und
//! `rope`, `softmax` und `mlp` rufen dort die NEON-Fassungen. Wer den Kopf
//! las, hielt einen echten Paritaetslauf fuer eine Leerformel.
//!
//! **⚑ Fund 104: Diese Datei lief in keinem CI-Job.** Alle kernels-Schritte
//! riefen `cargo test --lib`, und `--lib` laesst `tests/` aus. Damit war die
//! "normative Garantie" ueber Monate nicht wirksam. Sie haette den
//! AVX512VL-Befehl unter der AVX2-Weiche (Fund 103) auf jeder Maschine ohne
//! AVX-512 sofort zum Absturz gebracht.
//!
//! **Und sie haette die Rechenabweichung aus Fund 103 trotzdem nicht
//! gesehen.** `rope_parity_basic` rechnet mit `q = 100` und `k = 50`: Nach
//! dem Rechtsshift liegt jedes Zwischenergebnis weit im i16-Bereich, und
//! dort stimmen Abschneiden und Saettigen ueberein. Ein Paritaetstest, der
//! nur den harmlosen Bereich abtastet, belegt die Paritaet nicht. Deshalb
//! steht unten `rope_parity_saettigung`, und der Test prueft zuerst, dass
//! sein eigener Fall wirklich saettigt.

// Ohne das Feature `cpu-simd` gibt es kein zweites Backend, gegen das
// verglichen werden koennte — dann ist diese Datei vollstaendig leer
// statt halb-tot (vorher: sechs Warnungen ueber ungenutzte Helfer bei
// jedem Standard-Build).
#![cfg(feature = "cpu-simd")]

use integer_llm_kernels::backend::Backend;
use integer_llm_kernels::backends::reference::ReferenceBackend;
use integer_llm_kernels::backends::simd::SimdBackend;

// =====================================================================
// Test-Daten
// =====================================================================

/// Das SIMD-Backend, oder ein **lauter** Uebersprung.
///
/// ⛑ Hier stand sechsmal `None => return` mit einem Kommentar daneben.
/// Auf einer x86_64-Maschine ohne AVX2 lief diese Datei damit vollstaendig
/// durch, ohne eine einzige Zusicherung zu pruefen, und meldete sechs
/// bestandene Tests. **Ein stiller Uebersprung sieht aus wie ein
/// bestandener Test**, und bei einer Datei, die sich "die normative
/// Garantie" nennt, ist das die teuerste Verwechslung im Repositorium.
fn simd_oder_laut() -> Option<SimdBackend> {
    match SimdBackend::detect() {
        Some(b) => Some(b),
        None => {
            eprintln!(
                "[uebersprungen] kein SIMD-Backend auf dieser Maschine: \
                 die Paritaet ist hier NICHT geprueft"
            );
            None
        }
    }
}

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

#[test]
fn softmax_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
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

#[test]
fn softmax_parity_edge_cases() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
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

#[test]
fn rope_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
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

/// ⚑ **Der Fall, an dem Fund 103 haengt: Saettigung statt Abschneiden.**
///
/// `rotate_half_split_i16` narrowt mit `clamp_i16`, saettigt also. Der
/// AVX2-Pfad benutzte bis zum 2026-08-30 `_mm256_cvtepi32_epi16`, und der
/// **schneidet ab**. Solange kein Zwischenwert den i16-Bereich verlaesst,
/// liefern beide dasselbe, und genau deshalb ist es keinem Test
/// aufgefallen: `rope_parity_basic` bleibt mit `q = 100` weit darunter.
///
/// Dieser Test waehlt die Werte so, dass es ueberlaeuft, **und prueft das
/// zuerst**. Ohne die Vorabpruefung koennte eine spaetere Aenderung an den
/// Skalen den Fall unbemerkt harmlos machen, und der Test bliebe gruen,
/// ohne noch etwas zu belegen.
#[test]
fn rope_parity_saettigung() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
    let frac_bits = 8u8;
    let head_dim = 64;
    let half = head_dim / 2;
    let max_seq = 2;

    // Grosse Betraege in cos/sin und in den Eingaben. Nach `>> 8` liegt
    // x0*cos - x1*sin weit jenseits von i16::MAX, die Referenz klemmt auf
    // 32767. head_dim 64 heisst half 32: genug fuer jede Blockbreite der
    // beiden Pfade (AVX2 acht, NEON vier), ohne dass der Fall in den
    // skalaren Rest faellt, der in beiden Fassungen derselbe ist.
    let cos_lut: Vec<i16> = vec![30000; max_seq * half];
    let sin_lut: Vec<i16> = vec![-30000; max_seq * half];

    let mut q_ref = vec![vec![32000i16; head_dim]; 2];
    let mut k_ref = vec![vec![-32000i16; head_dim]; 2];
    let mut q_simd = q_ref.clone();
    let mut k_simd = k_ref.clone();
    let positions = vec![0usize, 1];

    ref_backend.rope(&mut q_ref, &mut k_ref, &cos_lut, &sin_lut, &positions, frac_bits);
    simd_backend.rope(&mut q_simd, &mut k_simd, &cos_lut, &sin_lut, &positions, frac_bits);

    // Gegenprobe zur Gegenprobe: Der Fall muss wirklich saettigen.
    let geklemmt = q_ref
        .iter()
        .chain(k_ref.iter())
        .flatten()
        .any(|&v| v == i16::MAX || v == i16::MIN);
    assert!(
        geklemmt,
        "Testfall laeuft nicht mehr ueber: dann prueft er die Saettigung nicht \
         und ist gegen Fund 103 wertlos"
    );

    assert_eq!(q_ref, q_simd, "RoPE Q saettigend: SIMD weicht von Referenz ab");
    assert_eq!(k_ref, k_simd, "RoPE K saettigend: SIMD weicht von Referenz ab");
}

// =====================================================================
// Linear-Paritaet
// =====================================================================

#[test]
fn linear_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };

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

#[test]
fn rmsnorm_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
    let rsqrt_lut = test_rsqrt_lut();
    let n = 64;

    let x: Vec<i16> = (0..n).map(|i| (i as i16) * 10 - 300).collect();
    let gamma: Vec<i8> = (0..n).map(|i| 40 + (i % 30) as i8).collect();
    let gamma_shifts: Vec<u8> = (0..n).map(|i| 5 + (i % 4) as u8).collect();
    let inv_n_q20 = ((1i64 << 20) + n as i64 / 2) / n as i64;

    let mut ref_out = vec![0i16; n];
    let mut simd_out = vec![0i16; n];

    let x_shifts: Vec<u8> = (0..n).map(|i| 3 + (i % 5) as u8).collect();
    ref_backend.rmsnorm(&x, &x_shifts, &gamma, &gamma_shifts, &rsqrt_lut, 8, 8, inv_n_q20, &mut ref_out, 6);
    simd_backend.rmsnorm(&x, &x_shifts, &gamma, &gamma_shifts, &rsqrt_lut, 8, 8, inv_n_q20, &mut simd_out, 6);

    assert_eq!(ref_out, simd_out, "RMSNorm: SIMD weicht von Referenz ab");
}

// =====================================================================
// MLP-Paritaet
// =====================================================================

#[test]
fn mlp_parity_basic() {
    let ref_backend = ReferenceBackend::new();
    let Some(simd_backend) = simd_oder_laut() else { return };
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

    let out_shifts: Vec<u8> = (0..hidden).map(|i| 2 + (i % 6) as u8).collect();
    ref_backend.mlp(
        &x, &w_gate, &w_up, &w_down, &mut ref_out,
        &gate_shifts, &up_shifts, &down_shifts,
        &silu_lut,
        6, 8, 8, 8, 3, 1024, 6, &out_shifts,
    );
    simd_backend.mlp(
        &x, &w_gate, &w_up, &w_down, &mut simd_out,
        &gate_shifts, &up_shifts, &down_shifts,
        &silu_lut,
        6, 8, 8, 8, 3, 1024, 6, &out_shifts,
    );

    assert_eq!(ref_out, simd_out, "MLP: SIMD weicht von Referenz ab");
}
