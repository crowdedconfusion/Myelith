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
    dtype: String,
    #[allow(dead_code)]
    shape: Option<Vec<usize>>,
    hash: String,
    data: Vec<i64>,
}

impl TensorData {
    /// SHA-256 über die little-endian gepackte Nutzlast.
    ///
    /// Derselbe Vertrag wie in `tests/golden/generate.py` und
    /// `kernels/src/bin/golden_runner.rs`. Die Layer- und E2E-Vektoren
    /// trugen bis 2026-08-22 einen `DefaultHasher`-Wert in diesem Feld,
    /// und niemand prüfte ihn (Fund 37).
    fn berechneter_hash(&self) -> Option<String> {
        let mut payload = Vec::with_capacity(self.data.len() * 4);
        match self.dtype.as_str() {
            "int8" => self.data.iter().for_each(|&v| payload.push(v as i8 as u8)),
            "int16" => self
                .data
                .iter()
                .for_each(|&v| payload.extend_from_slice(&(v as i16).to_le_bytes())),
            "int32" => self
                .data
                .iter()
                .for_each(|&v| payload.extend_from_slice(&(v as i32).to_le_bytes())),
            _ => return None,
        }
        Some(integer_llm_runtime::loader::sha256_hex(&payload))
    }
}

/// Prüft die deklarierten Hashes aller Ein- und Ausgabetensoren.
///
/// **Vor der Rechnung**, nicht danach: Ein Vektor, dessen Daten
/// nachträglich bearbeitet wurden, ist kein Maßstab, und ein Ergebnis
/// gegen ihn hat keine Aussage, weder im Guten noch im Schlechten.
fn tensor_hashes_pruefen(gv: &GoldenVector) -> bool {
    let mut ok = true;
    for (bereich, tensoren) in [("input", &gv.inputs), ("output", &gv.outputs)] {
        for (name, t) in tensoren.iter() {
            let Some(berechnet) = t.berechneter_hash() else {
                continue;
            };
            if berechnet != t.hash {
                eprintln!(
                    "  Hash-Abweichung bei {} '{}': deklariert {}, berechnet {}",
                    bereich, name, t.hash, berechnet
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
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .filter(|p| p.to_str().is_some_and(|s| s.ends_with(".golden.json")))
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
    // Integrität des Vektors, bevor er als Maßstab dient (Fund 37).
    if !tensor_hashes_pruefen(gv) {
        eprintln!("  Vektor {} ist nicht integer, kein Maßstab", gv.name);
        return false;
    }
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

    // Über `dekodieren_mit_digest`, nicht als eigene Schleife: Die
    // Bytefolge des Digests ist dort festgelegt, und eine zweite Fassung
    // hier wäre eine zweite Quelle für dieselbe Aussage (Fund 34).
    let (erzeugt, digest) = integer_llm_runtime::generate::dekodieren_mit_digest(
        model,
        &prompt_tokens,
        max_new_tokens,
        seed,
        greedy,
    );
    let generated: Vec<i32> = erzeugt.iter().map(|&t| t as i32).collect();

    let expected: Vec<i32> = gv.outputs["tokens"].data.iter().map(|&v| v as i32).collect();

    if generated != expected {
        eprintln!("  Token-Mismatch: erzeugt={:?}, erwartet={:?}", generated, expected);
        return false;
    }

    // **Der eigentliche Prüfwert** (Fund 36). Gleiche Token heißen nur,
    // dass dieselbe Entscheidung gefallen ist; ein Argmax über
    // `vocab_size` Zahlen ändert sich erst, wenn deren Rangfolge kippt.
    // Gemessen an 0,5B: 0,1 % der Bytes eines Tensors verschoben, Token
    // unverändert, Zahlen verschieden.
    match gv.metadata.get("logits_sha256").and_then(|v| v.as_str()) {
        Some(erwartet) => {
            if digest != erwartet {
                eprintln!(
                    "  Logit-Mismatch: berechnet={}, erwartet={}",
                    digest, erwartet
                );
                eprintln!(
                    "  Die Token stimmen, die gerechneten Zahlen nicht. Genau dieser \
                     Fall blieb vor Fund 36 unsichtbar."
                );
                return false;
            }
        }
        None => {
            // Kein stiller Durchlauf: Ein Vektor ohne diesen Wert prüft
            // nur die Entscheidung, und das soll niemand für einen
            // Bitgleichheitsnachweis halten.
            eprintln!(
                "  Vektor {} trägt kein metadata.logits_sha256 und prüft damit nur \
                 die erzeugten Token (Fund 36). Neu erzeugen mit golden_generate.",
                gv.name
            );
            return false;
        }
    }
    true
}
