//! Prueflauf eines Golden Vectors gegen die Kernel-Implementierung.
//!
//! Neben dem Ergebnisvergleich wird der im Vektor deklarierte
//! Tensor-Hash geprueft. Die Felder `hash` und `theta_v_hash` waren
//! bis v0.12.40 zwar eingelesen, aber nie ausgewertet (Compiler-Warnung
//! „never read" als Hinweis) — ein Vektor, dessen Daten nachtraeglich
//! bearbeitet wurden, ohne den Hash mitzuziehen, waere unbemerkt
//! durchgelaufen. Hash-Vertrag laut `tests/golden/generate.py`:
//! SHA-256 ueber die Little-Endian-gepackte Nutzlast.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct GoldenVector {
    name: String,
    /// Ebene des Vektors (`op`, `layer`, `e2e`) — Teil des
    /// Vektorformats, hier nicht ausgewertet. Das Feld bleibt, damit
    /// ein unvollstaendiger Vektor beim Parsen auffaellt statt still
    /// durchzugehen.
    #[allow(dead_code)]
    level: String,
    /// θ_v-Hash des erzeugenden Modells. **Noch nicht geprueft** — die
    /// Gegenprobe braucht die eingebettete `spec.json` aus der Runtime,
    /// die dieses Binary nicht kennt (kernels haengt nicht an runtime).
    /// Vermerkt im INTEGER_LLM-Fahrplan als offener Punkt.
    #[allow(dead_code)]
    theta_v_hash: String,
    metadata: serde_json::Value,
    inputs: HashMap<String, TensorData>,
    outputs: HashMap<String, TensorData>,
}

#[derive(Debug, Deserialize)]
struct TensorData {
    dtype: String,
    /// Tensorform — Teil des Formats; die Kernel arbeiten auf flachen
    /// Vektoren und leiten die Form aus den Metadaten ab.
    #[allow(dead_code)]
    shape: Option<Vec<usize>>,
    hash: String,
    data: Vec<i64>,
}

/// SHA-256 ueber die Little-Endian-gepackte Nutzlast — identisch zu
/// `GoldenVectorBuilder._hash_tensor` in `tests/golden/generate.py`.
fn hash_tensor(data: &[i64], dtype: &str) -> String {
    let mut payload: Vec<u8> = Vec::with_capacity(data.len() * 4);
    match dtype {
        "int8" => data.iter().for_each(|&v| payload.push(v as i8 as u8)),
        "int16" => data
            .iter()
            .for_each(|&v| payload.extend_from_slice(&(v as i16).to_le_bytes())),
        "int32" => data
            .iter()
            .for_each(|&v| payload.extend_from_slice(&(v as i32).to_le_bytes())),
        other => {
            eprintln!("  Unbekannter dtype '{}': Hash-Pruefung uebersprungen", other);
            return String::new();
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Prueft die deklarierten Hashes aller Ein- und Ausgabetensoren.
fn verify_tensor_hashes(gv: &GoldenVector) -> bool {
    let mut ok = true;
    for (bereich, tensors) in [("input", &gv.inputs), ("output", &gv.outputs)] {
        for (name, t) in tensors.iter() {
            let computed = hash_tensor(&t.data, &t.dtype);
            if computed.is_empty() {
                continue; // dtype nicht gepackt darstellbar
            }
            if computed != t.hash {
                eprintln!(
                    "  Hash-Abweichung bei {} '{}': deklariert {}, berechnet {}",
                    bereich, name, t.hash, computed
                );
                ok = false;
            }
        }
    }
    ok
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: golden_runner <golden.json> <backend_name>");
        std::process::exit(1);
    }
    let path = &args[1];
    let _backend_name = &args[2];

    let content = std::fs::read_to_string(path).expect("Failed to read golden file");
    let gv: GoldenVector = serde_json::from_str(&content).expect("Failed to parse JSON");

    // Integritaet des Vektors selbst, bevor er als Massstab dient.
    if !verify_tensor_hashes(&gv) {
        println!("FAIL: {} (Hash-Pruefung des Vektors)", gv.name);
        std::process::exit(1);
    }

    let passed = match gv.name.as_str() {
        "rmsnorm_basic" => run_rmsnorm(&gv),
        "linear_w8a16_identity" => run_linear(&gv),
        "softmax_basic" => run_softmax(&gv),
        _ => {
            eprintln!("Unknown golden vector: {}", gv.name);
            false
        }
    };

    if passed {
        println!("PASS: {}", gv.name);
        std::process::exit(0);
    } else {
        println!("FAIL: {}", gv.name);
        std::process::exit(1);
    }
}

fn run_rmsnorm(gv: &GoldenVector) -> bool {
    // theta_v 0.7.0: int16-Eingang, LUT-gestuetztes rsqrt mit dynamischem
    // geradem Index-Shift, divisionsfrei; Gamma mit Per-Element-Skalen
    // (abwaertskompatibel: ohne gamma_shifts wird gamma_shift repliziert).
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    // Fund 20 (theta_v 0.11.0): x_shifts optional, fuer Vektoren vor
    // v0.12.44 fehlt das Feld - Vorgabe 0 fuer alle Kanaele ist bitgleich
    // zur alten Skalar-Behandlung (bewiesen in kernels/src/rmsnorm.rs,
    // test_rmsnorm_per_channel_uniform_shifts_matches_legacy).
    let x_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("x_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        vec![0u8; x.len()]
    };
    let gamma: Vec<i8> = gv.inputs["gamma"].data.iter().map(|&v| v as i8).collect();
    let gamma_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("gamma_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        let gamma_shift = gv.metadata["gamma_shift"].as_u64().unwrap() as u8;
        vec![gamma_shift; gamma.len()]
    };
    let rsqrt_lut: Vec<i16> = gv.metadata["rsqrt_lut"].as_array().unwrap()
        .iter().map(|v| v.as_i64().unwrap() as i16).collect();
    let lut_input_shift = gv.metadata["lut_input_shift"].as_u64().unwrap() as u8;
    let lut_output_frac = gv.metadata["lut_output_frac"].as_u64().unwrap() as u8;
    let inv_n_q20 = gv.metadata["inv_n_q20"].as_i64().unwrap();
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = integer_llm_kernels::rmsnorm::rmsnorm_i16(
        &x, &x_shifts, &gamma, &gamma_shifts, &rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
    let expected: Vec<i16> = gv.outputs["y"].data.iter().map(|&v| v as i16).collect();

    if result != expected {
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Got:      {:?}", result);
    }
    result == expected
}

fn run_linear(gv: &GoldenVector) -> bool {
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    let w_meta = gv.metadata["W"].as_array().unwrap();
    let w: Vec<Vec<i8>> = w_meta.iter().map(|row| {
        row.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i8).collect()
    }).collect();
    let act_frac = gv.metadata["act_frac"].as_u64().unwrap() as u8;
    let w_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("w_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        let weight_frac = gv.metadata["weight_frac"].as_u64().unwrap() as u8;
        vec![weight_frac; w.len()]
    };
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = integer_llm_kernels::linear::linear_w8a16(&x, &w, &w_shifts, act_frac, out_frac);
    let expected: Vec<i16> = gv.outputs["y"].data.iter().map(|&v| v as i16).collect();

    if result != expected {
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Got:      {:?}", result);
    }
    result == expected
}

fn run_softmax(gv: &GoldenVector) -> bool {
    let logits: Vec<i32> = gv.inputs["logits"].data.iter().map(|&v| v as i32).collect();
    let lut_shift = gv.metadata["lut_shift"].as_u64().unwrap() as u8;
    let frac_bits = gv.metadata["frac_bits"].as_u64().unwrap() as u8;

    let exp_lut: Vec<i16> = if let Some(lut) = gv.metadata.get("exp_lut") {
        lut.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i16).collect()
    } else {
        (0..128).map(|i| {
            let val = (-(i as f64) / 256.0f64).exp() * 256.0;
            val.round() as i16
        }).collect()
    };

    let result = integer_llm_kernels::softmax::softmax_int(&logits, &exp_lut, lut_shift, frac_bits);
    let expected: Vec<i32> = gv.outputs["probs"].data.iter().map(|&v| v as i32).collect();

    if result != expected {
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Got:      {:?}", result);
    }
    result == expected
}