//! Generierungs-Loop (Prefill + Decode)

use integer_llm_kernels::prng::seed_from_ids;
use crate::model::IntegerModel;
use crate::kv_cache::KVCache;
use crate::tokenizer::Tokenizer;

/// Komplette Generierung von Prompt zu Token-Sequenz.
pub fn generate(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> Vec<usize> {
    let token_ids = tokenizer.encode(prompt);
    // Cache-Groesse folgt num_kv_heads (GQA), nicht num_heads: gespeichert
    // werden nur die tatsaechlich vorhandenen Key/Value-Heads.
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut pos = 0usize;
    let mut logits = vec![0i32; model.vocab_size];

    // Prefill: alle Prompt-Tokens durchlaufen
    for &tid in &token_ids {
        logits = model.forward_token(tid, pos, &mut cache);
        pos += 1;
    }

    // Decode: Token fuer Token generieren
    let mut out = Vec::with_capacity(max_new_tokens);
    let mut current_seed = seed;
    
    for _ in 0..max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        
        out.push(next_token);
        logits = model.forward_token(next_token, pos, &mut cache);
        pos += 1;
    }

    out
}

/// Hash einer Token-Sequenz fuer deterministische Validierung.
pub fn hash_tokens(tokens: &[usize]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    tokens.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
