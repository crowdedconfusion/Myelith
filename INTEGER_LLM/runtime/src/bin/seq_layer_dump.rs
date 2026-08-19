//! Sequenz-Layer-Dump (Mehrpositions-Divergenzsuche, Fund 14 Kandidat iii):
//! füllt den KV-Cache mit einer Token-Sequenz und dumppt die
//! Reststrom-Statistiken (AbsMax + erste vier Werte) nach jedem Layer an der
//! LETZTEN Position — zum Abgleich mit der HF-Referenz
//! (`tests/diag/seq_layer_dump_hf.py`, `output_hidden_states`), um die erste
//! divergierende Stufe im Mehrpositions-Pfad (Attention/RoPE/KV-Cache) zu
//! finden. Im Gegensatz zu `layer_dump` (einzelnes Token, leerer KV-Cache)
//! attendiert die letzte Position hier auf alle vorherigen.
//!
//! Kein Teil des Auslieferungspfads.
//!
//! Usage: seq_layer_dump <artifact_dir> <token_id> [<token_id> ...]

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

/// Realwerte je Kanal aus Rohwerten und Per-Kanal-Shifts (Fund 20).
/// Die Umrechnung geschieht hier und nicht in `model.rs`: dort verbietet
/// das Gleitkomma-Audit (tests/audit/test_no_float.py) jeden Float, weil
/// die Datei auf der Heisspfad-Liste steht. Diagnose-Binaries sind davon
/// ausgenommen — sie sind kein Teil des Auslieferungspfads.
fn real_absmax_und_first4(werte: &[i16], shifts: &[u8]) -> (f64, [f64; 4]) {
    let absmax = werte
        .iter()
        .zip(shifts.iter())
        .map(|(v, &s)| (*v as f64).abs() * 2f64.powi(-(s as i32)))
        .fold(0.0, f64::max);
    let mut first4 = [0.0f64; 4];
    for (k, (v, &s)) in werte.iter().zip(shifts.iter()).take(4).enumerate() {
        first4[k] = *v as f64 * 2f64.powi(-(s as i32));
    }
    (absmax, first4)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: seq_layer_dump <artifact_dir> <token_id> [<token_id> ...]");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let tokens: Vec<usize> = args[2..]
        .iter()
        .map(|s| s.parse().expect("token_id muss eine Zahl sein"))
        .collect();

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    let last = tokens.len() - 1;
    let mut logits = Vec::new();
    let mut dump = Vec::new();
    for (pos, &tok) in tokens.iter().enumerate() {
        if pos == last {
            let (l, d) = model.forward_token_dump(tok, pos, &mut cache);
            logits = l;
            dump = d;
        } else {
            // KV-Cache für die Folgepositionen füllen.
            let _ = model.forward_token(tok, pos, &mut cache);
        }
    }

    println!("Sequenz: {:?} (Dump an Position {})", tokens, last);
    // Fund 20: der Residualstrom traegt seit theta_v 0.11.0 eine Skala je
    // Kanal - forward_token_dump liefert bereits umgerechnete Realwerte.
    for (i, (werte, shifts)) in dump.iter().enumerate() {
        let (absmax, first4) = real_absmax_und_first4(werte, shifts);
        let label = if i < model.num_layers {
            format!("layer {:2}", i)
        } else {
            "finalnorm".to_string()
        };
        println!(
            "{}: absmax={:9.4} first4=[{:9.4}, {:9.4}, {:9.4}, {:9.4}]",
            label, absmax, first4[0], first4[1], first4[2], first4[3]
        );
    }

    let mut idx: Vec<usize> = (0..logits.len()).collect();
    idx.sort_by(|&a, &b| logits[b].cmp(&logits[a]));
    println!("Top-5 Logits (id: wert):");
    for &i in idx.iter().take(5) {
        println!("  {}: {}", i, logits[i]);
    }
}
