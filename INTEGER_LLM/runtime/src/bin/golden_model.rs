//! Golden-Vector-Validator fuer Layer- und E2E-Ebene.
//!
//! Laedt das echte Modell, fuehrt den Forward-Pass mit den Eingaben aus
//! dem Golden Vector aus und vergleicht das Ergebnis bitgenau.
//!
//! Einzeldatei:  golden_model <artifact_dir> <golden.json>
//! Batch-Modus:  golden_model <artifact_dir> --batch <vectors_dir>
//!
//! Im Batch-Modus wird das Modell einmal geladen und alle
//! *.golden.json-Dateien in <vectors_dir>/layer/ und <vectors_dir>/e2e/
//! validiert. Exit 0 wenn alle bestehen, sonst 1.

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::kv_cache::KVCache;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct GoldenVector {
    name: String,
    level: String,
    #[allow(dead_code)]
    theta_v_hash: String,
    metadata: serde_json::Value,
    inputs: std::collections::HashMap<String, TensorData>,
    outputs: std::collections::HashMap<String, TensorData>,
}

#[derive(Debug, Deserialize)]
struct TensorData {
    #[allow(dead_code)]
    dtype: String,
    #[allow(dead_code)]
    shape: Option<Vec<usize>>,
    #[allow(dead_code)]
    hash: String,
    data: Vec<i64>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: golden_model <artifact_dir> <golden.json>");
        eprintln!("       golden_model <artifact_dir> --batch <vectors_dir>");
        std::process::exit(1);
    }
    let artifact_dir = PathBuf::from(&args[1]);

    let model = load_model(&artifact_dir).expect("Modell-Ladung fehlgeschlagen");

    if args.len() >= 4 && args[2] == "--batch" {
        let vectors_dir = PathBuf::from(&args[3]);
        run_batch(&model, &vectors_dir);
    } else {
        let gv_path = PathBuf::from(&args[2]);
        run_single(&model, &gv_path);
    }
}

fn run_single(model: &integer_llm_runtime::model::IntegerModel, gv_path: &Path) {
    let content = std::fs::read_to_string(gv_path).expect("Golden-Datei nicht lesbar");
    let gv: GoldenVector = serde_json::from_str(&content).expect("Golden-JSON ungueltig");

    let passed = validate(model, &gv);
    if passed {
        println!("PASS: {}", gv.name);
        std::process::exit(0);
    } else {
        println!("FAIL: {}", gv.name);
        std::process::exit(1);
    }
}

fn run_batch(model: &integer_llm_runtime::model::IntegerModel, vectors_dir: &Path) {
    let mut total = 0;
    let mut passed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();

    for level in &["layer", "e2e"] {
        let level_dir = vectors_dir.join(level);
        if !level_dir.exists() {
            continue;
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&level_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "json"))
            .filter(|p| p.to_str().map_or(false, |s| s.ends_with(".golden.json")))
            .collect();
        files.sort();

        for gv_path in &files {
            let content = std::fs::read_to_string(gv_path).expect("Golden-Datei nicht lesbar");
            let gv: GoldenVector = serde_json::from_str(&content).expect("Golden-JSON ungueltig");
            total += 1;
            if validate(model, &gv) {
                passed += 1;
                println!("  PASS: {}", gv.name);
            } else {
                failed += 1;
                errors.push(gv.name.clone());
                println!("  FAIL: {}", gv.name);
            }
        }
    }

    println!("\n{} von {} bestanden ({} fehlgeschlagen)", passed, total, failed);
    if !errors.is_empty() {
        eprintln!("Fehlgeschlagen: {:?}", errors);
        std::process::exit(1);
    }
}

fn validate(model: &integer_llm_runtime::model::IntegerModel, gv: &GoldenVector) -> bool {
    match gv.level.as_str() {
        "layer" => validate_layer(model, gv),
        "e2e" => validate_e2e(model, gv),
        other => {
            eprintln!("Unbekanntes Level: {}", other);
            false
        }
    }
}

fn validate_layer(model: &integer_llm_runtime::model::IntegerModel, gv: &GoldenVector) -> bool {
    let layer_idx = gv.metadata["layer_idx"].as_u64().unwrap() as usize;
    let hidden_in: Vec<i16> = gv.inputs["hidden"].data.iter().map(|&v| v as i16).collect();

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let hidden_out = model.run_layers(hidden_in, 0, &mut cache, layer_idx, layer_idx + 1);

    let expected: Vec<i16> = gv.outputs["hidden_out"].data.iter().map(|&v| v as i16).collect();

    if hidden_out.len() != expected.len() {
        eprintln!("  Laenge mismatch: {} vs. {}", hidden_out.len(), expected.len());
        return false;
    }

    let mismatches: Vec<usize> = hidden_out.iter().zip(expected.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();

    if !mismatches.is_empty() {
        eprintln!("  {} Mismatches in layer {} (erste: {:?})",
            mismatches.len(), layer_idx,
            mismatches.iter().take(5).map(|&i| {
                (i, hidden_out[i], expected[i])
            }).collect::<Vec<_>>());
        return false;
    }
    true
}

fn validate_e2e(model: &integer_llm_runtime::model::IntegerModel, gv: &GoldenVector) -> bool {
    let prompt_tokens: Vec<usize> = gv.inputs["prompt_tokens"].data.iter()
        .map(|&v| v as usize).collect();
    let max_new_tokens = gv.metadata["max_new_tokens"].as_u64().unwrap() as usize;
    let greedy = gv.metadata["greedy"].as_bool().unwrap_or(true);
    let seed = gv.metadata["seed"].as_u64().unwrap_or(42);

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut pos = 0usize;
    let mut logits = vec![0i32; model.vocab_size];

    // Prefill
    for &tid in &prompt_tokens {
        logits = model.forward_token(tid, pos, &mut cache);
        pos += 1;
    }

    // Decode
    let mut generated = Vec::with_capacity(max_new_tokens);
    let mut current_seed = seed;
    for _ in 0..max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        generated.push(next_token as i32);
        logits = model.forward_token(next_token, pos, &mut cache);
        pos += 1;
    }

    let expected: Vec<i32> = gv.outputs["tokens"].data.iter().map(|&v| v as i32).collect();

    if generated != expected {
        eprintln!("  Token-Mismatch: erzeugt={:?}, erwartet={:?}", generated, expected);
        return false;
    }
    true
}
