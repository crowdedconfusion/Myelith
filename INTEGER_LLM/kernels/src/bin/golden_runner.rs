use std::collections::HashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenVector {
    name: String,
    level: String,
    theta_v_hash: String,
    metadata: serde_json::Value,
    inputs: HashMap<String, TensorData>,
    outputs: HashMap<String, TensorData>,
}

#[derive(Debug, Deserialize)]
struct TensorData {
    dtype: String,
    shape: Option<Vec<usize>>,
    hash: String,
    data: Vec<i64>,
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
    // theta_v 0.5.0: int16-Eingang, LUT-gestuetztes rsqrt mit dynamischem
    // geradem Index-Shift, divisionsfrei (inv_n_q20-Konstante).
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    let gamma: Vec<i8> = gv.inputs["gamma"].data.iter().map(|&v| v as i8).collect();
    let gamma_shift = gv.metadata["gamma_shift"].as_u64().unwrap() as u8;
    let rsqrt_lut: Vec<i16> = gv.metadata["rsqrt_lut"].as_array().unwrap()
        .iter().map(|v| v.as_i64().unwrap() as i16).collect();
    let lut_input_shift = gv.metadata["lut_input_shift"].as_u64().unwrap() as u8;
    let lut_output_frac = gv.metadata["lut_output_frac"].as_u64().unwrap() as u8;
    let inv_n_q20 = gv.metadata["inv_n_q20"].as_i64().unwrap();
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = integer_llm_kernels::rmsnorm::rmsnorm_i16(
        &x, &gamma, gamma_shift, &rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
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
    let weight_frac = gv.metadata["weight_frac"].as_u64().unwrap() as u8;
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = integer_llm_kernels::linear::linear_w8a16(&x, &w, act_frac, weight_frac, out_frac);
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