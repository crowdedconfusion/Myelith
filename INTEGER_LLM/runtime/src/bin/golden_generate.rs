//! Golden-Vector-Generator fuer Layer- und E2E-Ebene.
//!
//! Erzeugt deterministische Referenz-Vektoren mit dem echten kalibrierten
//! Modell (Qwen2.5-0.5B). Die Vektoren werden von `golden_model.rs`
//! validiert und von `validate.py` als Subprozess aufgerufen.
//!
//! Usage: golden_generate <artifact_dir> <output_dir>

use integer_llm_runtime::loader::{load_model, spec_hash};
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::tokenizer::Tokenizer;
use integer_llm_runtime::generate::generate;
use integer_llm_kernels::prng::splitmix64;

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: golden_generate <artifact_dir> <output_dir>");
        std::process::exit(1);
    }
    let artifact_dir = PathBuf::from(&args[1]);
    let output_dir = PathBuf::from(&args[2]);

    let model = load_model(&artifact_dir).expect("Modell-Ladung fehlgeschlagen");
    let theta_v_hash = format!("sha256:{}", spec_hash());

    let vectors_dir = output_dir.join("vectors");
    let layer_dir = vectors_dir.join("layer");
    let e2e_dir = vectors_dir.join("e2e");
    std::fs::create_dir_all(&layer_dir).expect("layer dir");
    std::fs::create_dir_all(&e2e_dir).expect("e2e dir");

    generate_layer_vectors(&model, &theta_v_hash, &layer_dir);
    generate_e2e_vectors(&model, &artifact_dir, &theta_v_hash, &e2e_dir);

    println!("[golden_generate] Layer- und E2E-Vektoren erzeugt (theta_v={}).", theta_v_hash);
}

fn generate_layer_vectors(model: &IntegerModel, theta_v_hash: &str, output_dir: &Path) {
    for layer_idx in 0..model.num_layers {
        let layer = &model.layers[layer_idx];
        let sc = &layer.scales;
        let hs = model.hidden_size;

        // Deterministischer Eingabe-Hidden-State auf residual_in_frac.
        // splitmix64 erzeugt reproduzierbare int16-Werte in realistischen
        // Bereichen (nicht nur int8, da der Residualstrom int16 ist).
        let seed = (layer_idx as u64).wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1);
        let mut state = seed;
        let mut hidden_in = Vec::with_capacity(hs);
        for _ in 0..hs {
            let (s, z) = splitmix64(state);
            state = s;
            // Werte im Bereich [-256, 255] bei typischen residual_frac 3-8:
            // realistisch fuer den Residualstrom (nicht zu gross, nicht zu klein).
            let val = ((z >> 48) as i16).wrapping_add((z >> 32) as i16);
            hidden_in.push(val);
        }

        // Single-layer forward mit leerem KV-Cache (Position 0, nur
        // Self-Attention auf das eine Token).
        let out_frac = if layer_idx + 1 < model.num_layers {
            model.layers[layer_idx + 1].scales.residual_in_frac
        } else {
            model.final_residual_frac
        };

        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
        let hidden_out = model.run_layers(hidden_in.clone(), 0, &mut cache, layer_idx, layer_idx + 1);

        // Golden Vector als JSON serialisieren.
        let mut gv = serde_json::Map::new();
        gv.insert("name".into(), serde_json::Value::String(
            format!("transformer_layer_{}", layer_idx)));
        gv.insert("level".into(), serde_json::Value::String("layer".into()));
        gv.insert("theta_v_hash".into(), serde_json::Value::String(theta_v_hash.into()));

        // Metadata
        let mut meta = serde_json::Map::new();
        meta.insert("layer_idx".into(), serde_json::Value::Number(layer_idx.into()));
        meta.insert("seq_len".into(), serde_json::Value::Number(1.into()));
        meta.insert("residual_in_frac".into(), serde_json::Value::Number(sc.residual_in_frac.into()));
        meta.insert("out_residual_frac".into(), serde_json::Value::Number(out_frac.into()));
        gv.insert("metadata".into(), serde_json::Value::Object(meta));

        // Inputs
        let mut inputs = serde_json::Map::new();
        inputs.insert("hidden".into(), tensor_json_i16(&hidden_in));
        inputs.insert("position".into(), tensor_json_i32(&[0]));
        gv.insert("inputs".into(), serde_json::Value::Object(inputs));

        // Outputs
        let mut outputs = serde_json::Map::new();
        outputs.insert("hidden_out".into(), tensor_json_i16(&hidden_out));
        gv.insert("outputs".into(), serde_json::Value::Object(outputs));

        let path = output_dir.join(format!("layer_{:02}.golden.json", layer_idx));
        let json = serde_json::to_string(&serde_json::Value::Object(gv)).unwrap();
        std::fs::write(&path, json).expect("write layer vector");
    }
    println!("[golden_generate] {} Layer-Vektoren erzeugt.", model.num_layers);
}

fn generate_e2e_vectors(
    model: &IntegerModel,
    artifact_dir: &Path,
    theta_v_hash: &str,
    output_dir: &Path,
) {
    let tokenizer_path = artifact_dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(tokenizer_path.to_str().unwrap())
        .expect("Tokenizer-Ladung fehlgeschlagen");

    let prompts: Vec<(&str, &str)> = vec![
        ("hello", "Hello"),
        ("world", "The world"),
        ("test", "This is a test"),
    ];

    let seed: u64 = 42;
    let max_new_tokens: usize = 3;

    for (name, prompt) in &prompts {
        let tokens = generate(model, &tokenizer, prompt, max_new_tokens, seed, true);

        let token_ids: Vec<i32> = tokens.iter().map(|&t| t as i32).collect();

        // Prompt-Tokens fuer Reproduzierbarkeit mit speichern.
        let prompt_tokens = tokenizer.encode(prompt);
        let prompt_ids: Vec<i32> = prompt_tokens.iter().map(|&t| t as i32).collect();

        let mut gv = serde_json::Map::new();
        gv.insert("name".into(), serde_json::Value::String(
            format!("e2e_prompt_{}", name)));
        gv.insert("level".into(), serde_json::Value::String("e2e".into()));
        gv.insert("theta_v_hash".into(), serde_json::Value::String(theta_v_hash.into()));

        let mut meta = serde_json::Map::new();
        meta.insert("max_new_tokens".into(), serde_json::Value::Number(max_new_tokens.into()));
        meta.insert("greedy".into(), serde_json::Value::Bool(true));
        meta.insert("seed".into(), serde_json::Value::Number(seed.into()));
        gv.insert("metadata".into(), serde_json::Value::Object(meta));

        let mut inputs = serde_json::Map::new();
        inputs.insert("prompt_tokens".into(), tensor_json_i32(&prompt_ids));
        gv.insert("inputs".into(), serde_json::Value::Object(inputs));

        let mut outputs = serde_json::Map::new();
        outputs.insert("tokens".into(), tensor_json_i32(&token_ids));
        gv.insert("outputs".into(), serde_json::Value::Object(outputs));

        let path = output_dir.join(format!("e2e_{}.golden.json", name));
        let json = serde_json::to_string(&serde_json::Value::Object(gv)).unwrap();
        std::fs::write(&path, json).expect("write e2e vector");
    }
    println!("[golden_generate] {} E2E-Vektoren erzeugt.", prompts.len());
}

fn tensor_json_i16(data: &[i16]) -> serde_json::Value {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());

    let mut map = serde_json::Map::new();
    map.insert("dtype".into(), serde_json::Value::String("int16".into()));
    map.insert("shape".into(), serde_json::Value::Array(
        vec![serde_json::Value::Number(data.len().into())]));
    map.insert("hash".into(), serde_json::Value::String(hash));
    map.insert("data".into(), serde_json::Value::Array(
        data.iter().map(|&v| serde_json::Value::Number(v.into())).collect()));
    serde_json::Value::Object(map)
}

fn tensor_json_i32(data: &[i32]) -> serde_json::Value {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());

    let mut map = serde_json::Map::new();
    map.insert("dtype".into(), serde_json::Value::String("int32".into()));
    map.insert("shape".into(), serde_json::Value::Array(
        vec![serde_json::Value::Number(data.len().into())]));
    map.insert("hash".into(), serde_json::Value::String(hash));
    map.insert("data".into(), serde_json::Value::Array(
        data.iter().map(|&v| serde_json::Value::Number(v.into())).collect()));
    serde_json::Value::Object(map)
}
