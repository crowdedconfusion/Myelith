//! `myl-pod-node` — Pod-CLI (Phase 1, in-Prozess-Pipeline).
//!
//! Baut einen 4-Shard-Pod aus den Qwen2.5-0.5B-Artefakten, legt für
//! jeden Shard einen BLS-Schlüssel an und führt einen Prompt durch die
//! Shard-Pipeline. Die Ausgabe ist bei identischem Prompt bitgleich
//! (Akzeptanzkriterium Phase 1).
//!
//! Usage:
//!   myl-pod-node --artifacts <dir> --prompt "<text>" [--max-tokens <n>]

use std::sync::Arc;

use myl_pod::coordinator::Coordinator;
use myl_pod::da::{DaStore, XorParityCoder};
use myl_pod::shard::ShardNode;
use myl_pod::wire::pack_tokens;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("Usage: {} --artifacts <dir> --prompt <text> [--max-tokens <n>]", args[0]);
        std::process::exit(1);
    }
    let mut artifacts = None;
    let mut prompt = None;
    let mut max_tokens: u64 = 6;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--artifacts" => { artifacts = Some(args[i + 1].clone()); i += 2; }
            "--prompt" => { prompt = Some(args[i + 1].clone()); i += 2; }
            "--max-tokens" => { max_tokens = args[i + 1].parse().unwrap(); i += 2; }
            _ => { i += 1; }
        }
    }
    let artifacts = artifacts.expect("--artifacts erforderlich");
    let prompt = prompt.expect("--prompt erforderlich");

    println!("[myl-pod] Lade Modell aus {} ...", artifacts);
    let model = load_model(std::path::Path::new(&artifacts))
        .expect("Modell-Ladung fehlgeschlagen");
    let tokenizer = Tokenizer::from_file(
        std::path::Path::new(&artifacts).join("tokenizer.json").to_str().unwrap(),
    )
    .expect("Tokenizer-Ladung fehlgeschlagen");

    let num_layers = model.num_layers;
    let num_kv_heads = model.num_kv_heads;
    println!("[myl-pod] Modell geladen: {} Layer, {} KV-Heads", num_layers, num_kv_heads);
    let model = Arc::new(model);

    // 4 Shards: 0..6, 6..12, 12..18, 18..24 (analog zur
    // INTEGER_LLM-4-Node-Pipeline).
    let boundaries = [0usize, 6, 12, 18, num_layers];
    let mut shards = Vec::new();
    for s in 0..4 {
        let layer_start = boundaries[s];
        let layer_end = boundaries[s + 1];
        let has_embedding = s == 0;
        let has_lm_head = s == 3;
        let ikm = [(s as u8 + 1) * 17; 32];
        let sk = BlsSecretKey::key_gen(&ikm).expect("BLS KeyGen");
        let da = DaStore::new(Box::new(XorParityCoder::new(4)));
        let boundary_frac = 4;
        let shard = ShardNode::new(
            s,
            layer_start,
            layer_end,
            has_embedding,
            has_lm_head,
            model.clone(),
            sk,
            boundary_frac,
            da,
            max_tokens,
        );
        println!(
            "[myl-pod] Shard {}: Layer {}-{} (Embedding: {}, LM-Head: {})",
            s, layer_start, layer_end, has_embedding, has_lm_head
        );
        shards.push(Arc::new(shard));
    }

    let pod_id = PodId::new([0xAA; 32]);
    let epoch = EpochId(0);
    let mut coordinator = Coordinator::new(pod_id, epoch, shards, myl_pod::coordinator::DEFAULT_WINDOW_MS);

    // Prompt kodieren.
    let prompt_ids = tokenizer.encode(&prompt);
    let prompt_tokens: Vec<u32> = prompt_ids.iter().map(|t| *t as u32).collect();
    println!("[myl-pod] Prompt: {:?} ({} Tokens)", prompt, prompt_tokens.len());

    let session_id = 1u64;
    let generated = coordinator.run_prompt(session_id, &prompt_tokens, max_tokens);
    println!("[myl-pod] Generierte Token: {:?}", generated);

    // PoI-Bündel bauen.
    match coordinator.build_poi_bundle() {
        Ok(bundle) => {
            println!("[myl-pod] PoI-Bündel: vTFE={}, Segmente={}",
                bundle.vtfe_claimed, coordinator.completed_segments().len());
        }
        Err(e) => eprintln!("[myl-pod] PoI-Bündel-Fehler: {}", e),
    }

    // Hinweis: pack_tokens wird hier genutzt, um die Token-Darstellung zu zeigen.
    let _ = pack_tokens(&generated);
}
