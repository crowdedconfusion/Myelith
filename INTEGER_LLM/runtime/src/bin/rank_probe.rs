//! Diagnose-Binary: Teacher-Forcing-Rang des echten naechsten Tokens.
//!
//! Spielt eine Token-Sequenz Position fuer Position durch und meldet, auf
//! welchem Rang das jeweils tatsaechlich folgende Token in der
//! Integer-Logit-Verteilung landet. Niedrige Raenge = das Modell "kennt"
//! die Fortsetzung; hohe Raenge = der Forward-Pass selbst ist verzerrt.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: rank_probe <artifact_dir> <prompt>");
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
    println!("Sequenz: {:?} ({} Tokens)", ids, ids.len());

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut ranks = Vec::new();
    for (pos, &tid) in ids.iter().enumerate() {
        let logits = model.forward_token(tid, pos, &mut cache);
        if pos + 1 < ids.len() {
            let target = ids[pos + 1];
            let target_logit = logits[target];
            let rank = logits.iter().filter(|&&l| l > target_logit).count() + 1;
            let decoded = tokenizer.decode(&[target]);
            ranks.push((pos, target, rank, decoded));
        }
    }

    println!("pos | Ziel-Token | Rang | Text");
    for (pos, target, rank, text) in &ranks {
        println!("{:>3} | {:>10} | {:>4} | {:?}", pos, target, rank, text);
    }
    let top5 = ranks.iter().filter(|r| r.2 <= 5).count();
    let top20 = ranks.iter().filter(|r| r.2 <= 20).count();
    println!(
        "Top-5: {}/{} | Top-20: {}/{}",
        top5, ranks.len(), top20, ranks.len()
    );
}
