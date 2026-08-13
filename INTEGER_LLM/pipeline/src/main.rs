//! Pipeline-Node CLI
//!
//! Usage:
//!   integer-llm-pipeline --config configs/pipeline_4node.json \
//!     --stage 0 --bind 127.0.0.1:8001 \
//!     --artifacts artifacts/qwen2.5-0.5b \
//!     --downstream 127.0.0.1:8002 --max-tokens 8
//!
//! Die finale Stage bekommt zusätzlich `--feedback <adresse-von-stage-0>`:
//! darüber läuft die autoregressive Schleife (sampeltes Token → Embedding).

use integer_llm_pipeline::loader::load_stage_model;
use integer_llm_pipeline::manifest::PipelineManifest;
use integer_llm_pipeline::node::Node;
use integer_llm_pipeline::stage::StageRuntime;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 5 {
        eprintln!("Usage: {} --config <manifest.json> --stage <id> --bind <addr> --artifacts <dir> [--upstream <addr>] [--downstream <addr>] [--feedback <addr>] [--max-tokens <n>]",
                  args[0]);
        std::process::exit(1);
    }

    let mut config_path = None;
    let mut stage_id = None;
    let mut bind_addr = None;
    let mut upstream = None;
    let mut downstream = None;
    let mut feedback = None;
    let mut artifacts = None;
    let mut max_tokens: u64 = 8;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => { config_path = Some(args[i + 1].clone()); i += 2; }
            "--stage" => { stage_id = Some(args[i + 1].parse::<usize>().unwrap()); i += 2; }
            "--bind" => { bind_addr = Some(args[i + 1].clone()); i += 2; }
            "--upstream" => { upstream = Some(args[i + 1].clone()); i += 2; }
            "--downstream" => { downstream = Some(args[i + 1].clone()); i += 2; }
            "--feedback" => { feedback = Some(args[i + 1].clone()); i += 2; }
            "--artifacts" => { artifacts = Some(args[i + 1].clone()); i += 2; }
            "--max-tokens" => { max_tokens = args[i + 1].parse::<u64>().unwrap(); i += 2; }
            _ => { i += 1; }
        }
    }

    let config_path = config_path.expect("--config required");
    let stage_id = stage_id.expect("--stage required");
    let bind_addr = bind_addr.expect("--bind required");
    let artifacts = artifacts.expect("--artifacts required");

    // Manifest laden
    let manifest = PipelineManifest::load(&config_path)
        .expect("Failed to load pipeline manifest");

    // Stage-Config finden
    let stage_manifest = manifest
        .stages
        .iter()
        .find(|s| s.stage_id == stage_id)
        .unwrap_or_else(|| panic!("Stage {} not found in manifest", stage_id))
        .clone();

    println!("[pipeline] Node startet als Stage {} auf {}", stage_id, bind_addr);
    println!(
        "[pipeline] Layer-Bereich: {}-{}",
        stage_manifest.layer_start, stage_manifest.layer_end
    );
    println!(
        "[pipeline] Embedding: {}, LM-Head: {}, Sampling: {}",
        stage_manifest.has_embedding, stage_manifest.has_lm_head, stage_manifest.has_sampling
    );

    // Shard-spezifische Modell-Ladung (mit θ_v-Prüfung gegen das Manifest)
    let model = load_stage_model(std::path::Path::new(&artifacts), &stage_manifest, &manifest)
        .expect("Failed to load stage model");

    // Runtime bauen
    let runtime = Arc::new(StageRuntime::new(
        stage_manifest.clone(),
        manifest,
        model,
        max_tokens,
    ));
    println!(
        "[pipeline] theta_v erwartet (trunkiert): {:016x}",
        runtime.expected_theta_u64()
    );

    let mut node = Node::new(&format!("node-stage-{}", stage_id), &bind_addr);
    node.attach_runtime(runtime);
    if let Some(up) = upstream {
        node.upstream = Some(up);
    }
    if let Some(down) = downstream {
        node.downstream = Some(down);
    }
    if let Some(fb) = feedback {
        node.feedback_address = Some(fb);
    }

    // Event-Loop starten
    node.run_event_loop().expect("Event loop failed");
}
