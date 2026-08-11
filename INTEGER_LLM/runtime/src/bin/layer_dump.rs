//! Layer-Dump-Diagnose: Reststrom-Statistiken (AbsMax + erste vier Werte)
//! nach jedem Layer und nach der finalen Norm — zum Abgleich mit der
//! HF-Referenz (output_hidden_states), um Abweichungen zu lokalisieren.
//!
//! Usage: layer_dump <artifact_dir> <token_id> <pos>

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: layer_dump <artifact_dir> <token_id> <pos>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let token_id: usize = args[2].parse().expect("token_id");
    let pos: usize = args[3].parse().expect("pos");

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let (_logits, dump) = model.forward_token_dump(token_id, pos, &mut cache);

    for (i, (absmax, first4, frac)) in dump.iter().enumerate() {
        let scale = 2f64.powi(-(*frac as i32));
        let absmax_real = *absmax as f64 * scale;
        let label = if i < model.num_layers {
            format!("layer {:2}", i)
        } else {
            "finalnorm".to_string()
        };
        println!(
            "{}: absmax={:9.4} first4=[{:9.4}, {:9.4}, {:9.4}, {:9.4}] frac={}",
            label,
            absmax_real,
            first4[0] as f64 * scale,
            first4[1] as f64 * scale,
            first4[2] as f64 * scale,
            first4[3] as f64 * scale,
            frac
        );
    }
}
