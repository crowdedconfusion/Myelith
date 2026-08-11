//! Benchmark-Probe: Zeitmessung fuer Prefill und Decode (getrennt).
//!
//! Misst die reine Inferenzzeit (ohne Modell- und Tokenizer-Ladung) auf
//! dem Referenz-Backend: zuerst Prefill ueber den Prompt, dann
//! greedy-Decode von N Tokens. Dient der Einordnung des Durchsatzes und
//! spaeter dem Vergleich der SIMD-/CUDA-/ROCm-Backends (Fahrplan
//! 12.64–12.66). Kein Teil des Auslieferungspfads.
//!
//! Die Zeitmessung und ihre Ausgabe laufen in f64 — das ist der MESSPFAD,
//! nicht der Inferenzpfad. Die Logits entstehen ausschliesslich im
//! Integerpfad (wie bei perplexity_probe).
//!
//! Usage: bench_probe <artifact_dir> <prompt> <decode_tokens>

use std::time::Instant;

use integer_llm_runtime::generate::hash_tokens;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: bench_probe <artifact_dir> <prompt> <decode_tokens>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let prompt = &args[2];
    let decode_tokens: usize = args[3].parse().expect("decode_tokens muss eine Zahl sein");

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung fehlgeschlagen");

    let ids = tokenizer.encode(prompt);
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    // Prefill: alle Prompt-Tokens, Zeitmessung ohne Ladung.
    let t0 = Instant::now();
    let mut logits = vec![0i32; model.vocab_size];
    for (pos, &tid) in ids.iter().enumerate() {
        logits = model.forward_token(tid, pos, &mut cache);
    }
    let prefill = t0.elapsed();

    // Decode: Token fuer Token, greedy (deterministisch).
    let mut out = Vec::with_capacity(decode_tokens);
    let mut pos = ids.len();
    let t0 = Instant::now();
    for _ in 0..decode_tokens {
        let next = model.greedy_next(&logits);
        out.push(next);
        logits = model.forward_token(next, pos, &mut cache);
        pos += 1;
    }
    let decode = t0.elapsed();

    let prefill_ms = prefill.as_secs_f64() * 1000.0;
    let decode_ms = decode.as_secs_f64() * 1000.0;
    println!("prompt_tokens {}", ids.len());
    println!("prefill_ms {:.3}", prefill_ms);
    println!(
        "prefill_tokens_per_s {:.2}",
        ids.len() as f64 / prefill.as_secs_f64()
    );
    println!("decode_tokens {}", out.len());
    println!("decode_ms {:.3}", decode_ms);
    println!(
        "decode_tokens_per_s {:.2}",
        out.len() as f64 / decode.as_secs_f64()
    );
    // Token-Hash zur Abgleichbarkeit mit anderen Evidenz-Laeufen
    // (derselbe Prompt muss dieselben Tokens ergeben).
    println!("decode_hash {}", hash_tokens(&out));
}
