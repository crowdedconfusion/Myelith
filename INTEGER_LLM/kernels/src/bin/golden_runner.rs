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
        "linear_w8a8_identity" => run_linear(&gv),
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
    let x: Vec<i8> = gv.inputs["x"].data.iter().map(|&v| v as i8).collect();
    let gamma: Vec<i8> = gv.inputs["gamma"].data.iter().map(|&v| v as i8).collect();
    let frac_bits = gv.metadata["frac_bits"].as_u64().unwrap() as u8;
    let eps = gv.metadata["eps"].as_i64().unwrap() as i32;

    let result = integer_llm_kernels::rmsnorm::rmsnorm_int8(&x, &gamma, frac_bits, eps);
    let expected: Vec<i8> = gv.outputs["y"].data.iter().map(|&v| v as i8).collect();

    if result != expected {
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Got:      {:?}", result);
    }
    result == expected
}

fn run_linear(gv: &GoldenVector) -> bool {
    let x: Vec<i8> = gv.inputs["x"].data.iter().map(|&v| v as i8).collect();
    let w_meta = gv.metadata["W"].as_array().unwrap();
    let w: Vec<Vec<i8>> = w_meta.iter().map(|row| {
        row.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i8).collect()
    }).collect();
    let act_frac = gv.metadata["act_frac"].as_u64().unwrap() as u8;
    let weight_frac = gv.metadata["weight_frac"].as_u64().unwrap() as u8;
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = integer_llm_kernels::linear::linear_w8a8(&x, &w, act_frac, weight_frac, out_frac);
    let expected: Vec<i8> = gv.outputs["y"].data.iter().map(|&v| v as i8).collect();

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