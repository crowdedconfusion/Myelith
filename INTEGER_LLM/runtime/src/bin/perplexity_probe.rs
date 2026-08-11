//! Perplexitäts-Probe: Teacher-Forcing über Token-Sequenzen.
//!
//! Liest Sequenzen von Token-IDs (eine pro Zeile, leerzeichengetrennt),
//! spielt sie Position für Position durch die Integer-Runtime und berechnet
//! die Log-Probability des jeweils nächsten Tokens unter der
//! Integer-Logit-Verteilung.
//!
//! Wichtig: Die Log-Softmax-Berechnung läuft in f64 — das ist der
//! MESSPFAD, nicht der Inferenzpfad. Die Logits selbst entstehen
//! ausschließlich im Integerpfad; die Messung darf Gleitkomma verwenden
//! (Eval-Code ist kein Teil des Konsens-Vertrags).
//!
//! Ausgabe pro Sequenz: `<tokens> <ausgewertet> <sum_logp> <perplexity>`

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: perplexity_probe <artifact_dir> <sequenzdatei>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let text = std::fs::read_to_string(&args[2]).expect("Sequenzdatei unlesbar");

    let logit_scale = 2f64.powi(-(model.config.logit_frac_bits as i32));

    for line in text.lines() {
        let ids: Vec<usize> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if ids.len() < 2 {
            continue;
        }

        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
        let mut sum_logp = 0.0f64;
        let mut count = 0usize;

        for (pos, &tid) in ids.iter().enumerate() {
            let logits = model.forward_token(tid, pos, &mut cache);
            if pos + 1 < ids.len() {
                let target = ids[pos + 1];
                // Log-Softmax des Zieltokens (Messpfad, f64).
                // z_max wird über die SKALIERTEN Werte gebildet (Skalierung
                // mit positiver Konstante erhält das Maximum), sonst
                // unterlaufen alle exp-Terme zu 0.
                let z_max = logits.iter().map(|&v| v as f64).fold(f64::NEG_INFINITY, f64::max)
                    * logit_scale;
                let mut lse = 0.0f64;
                for &v in &logits {
                    lse += ((v as f64 * logit_scale) - z_max).exp();
                }
                let log_softmax_target =
                    (logits[target] as f64 * logit_scale) - z_max - lse.ln();
                sum_logp += log_softmax_target;
                count += 1;
            }
        }

        let ppl = (-sum_logp / count as f64).exp();
        println!("{} {} {:.6} {:.4}", ids.len(), count, sum_logp, ppl);
    }
}
