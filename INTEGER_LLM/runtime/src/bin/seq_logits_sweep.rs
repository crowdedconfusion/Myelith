//! Logit-Sweep über Positionen (Mehrpositions-Divergenzsuche, Fund 14
//! Kandidat iii): füllt den KV-Cache mit einer Token-Sequenz und gibt an
//! JEDER Position den Top-1-Logit (id + Wert) aus. Zusammen mit
//! `tests/diag/seq_logits_sweep_hf.py` zeigt das, ab welcher Position die
//! Integer-Vorhersage von der HF-Referenz abweicht.
//!
//! Kein Teil des Auslieferungspfads.
//!
//! Usage: seq_logits_sweep <artifact_dir> <token_id> [<token_id> ...]

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: seq_logits_sweep <artifact_dir> <token_id> [<token_id> ...]");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let tokens: Vec<usize> = args[2..]
        .iter()
        .map(|s| s.parse().expect("token_id muss eine Zahl sein"))
        .collect();

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    for (pos, &tok) in tokens.iter().enumerate() {
        let logits = model.forward_token(tok, pos, &mut cache);
        let (top_id, top_val) = logits
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, v)| (i, *v))
            .unwrap();
        println!("pos {}: input={} -> top1={} (wert {})", pos, tok, top_id, top_val);
    }
}
