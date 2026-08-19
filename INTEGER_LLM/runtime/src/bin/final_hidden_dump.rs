//! Diagnose: dumpt den finalen normierten Hidden-State (Eingang des
//! LM-Heads) als Realwerte, je Position einer Token-Sequenz.
//!
//! Zweck (2026-08-19): Trennt den Fehlerbeitrag des HIDDEN-STATE vom
//! Fehlerbeitrag des LM-HEADS. Die 7B-Perplexitaet weicht stark ab, ohne
//! dass ein Strukturfehler gefunden wurde (Funde 19/20 behoben, GPTQ
//! ausgeschlossen, Positionsvergleich zeigt breit verteiltes Rauschen).
//! Mit diesem Dump laesst sich der Integer-Hidden-State durch HFs
//! FLOAT-LM-Head schicken (tests/diag/hidden_ablation_hf.py): bleibt die
//! Perplexitaet dann schlecht, liegt der Fehler im Hidden-State; wird sie
//! gut, liegt er im LM-Head.
//!
//! Ausgabe: eine Zeile je Position, `<seq> <pos> <w0> <w1> ... <wN>`.
//! Kein Teil des Auslieferungspfads.

use integer_llm_kernels::rmsnorm::rmsnorm_i16;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: final_hidden_dump <artifact_dir> <sequenzdatei>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let text = std::fs::read_to_string(&args[2]).expect("Sequenzdatei unlesbar");
    let cfg = &model.config;
    let norm_scale = 2f64.powi(-(model.final_norm_frac as i32));

    for (seq_idx, line) in text.lines().enumerate() {
        let ids: Vec<usize> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if ids.len() < 2 {
            continue;
        }
        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
        for (pos, &tid) in ids.iter().enumerate() {
            // Identisch zum echten Pfad: Embedding, alle Layer, finale Norm.
            let hidden = model.embed_token(tid);
            let hidden = model.run_layers(hidden, pos, &mut cache, 0, model.num_layers);
            let normed = rmsnorm_i16(
                &hidden,
                &model.final_residual_frac,
                &model.final_norm_gamma.data,
                &model.final_norm_gamma.shifts,
                &model.rsqrt_lut,
                cfg.rsqrt_input_shift,
                cfg.rsqrt_output_frac,
                model.inv_n_q20,
                model.final_norm_frac,
            );
            if pos + 1 < ids.len() {
                let werte: Vec<String> = normed
                    .iter()
                    .map(|v| format!("{:.6}", *v as f64 * norm_scale))
                    .collect();
                println!("{} {} {}", seq_idx, pos, werte.join(" "));
            }
        }
    }
}
