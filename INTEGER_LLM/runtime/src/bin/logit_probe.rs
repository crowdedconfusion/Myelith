//! Diagnose-Binary: Logits nach dem Prefill eines Prompts ausgeben.
//!
//! Vergleich mit der HF-Referenz (Top-k-Ranking), um Numerik-Fehler von
//! normaler Quantisierungs-Drift zu unterscheiden. Kein Teil des
//! Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: logit_probe <artifact_dir> <prompt>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let prompt = &args[2];

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung fehlgeschlagen");

    let ids = tokenizer.encode(prompt);
    println!("Prompt-Tokens: {:?}", ids);

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut logits = Vec::new();
    for (pos, &tid) in ids.iter().enumerate() {
        logits = model.forward_token(tid, pos, &mut cache);
    }

    let mut idx: Vec<usize> = (0..logits.len()).collect();
    idx.sort_by(|&a, &b| logits[b].cmp(&logits[a]));

    println!("Top-10 Logits (id: wert):");
    for &i in idx.iter().take(10) {
        println!("  {}: {}", i, logits[i]);
    }
    for probe in [594usize, 295, 2746, 6250] {
        let rank = idx.iter().position(|&x| x == probe).map(|p| p + 1).unwrap();
        println!("Logit {} = {} (Rang {})", probe, logits[probe], rank);
    }
}
