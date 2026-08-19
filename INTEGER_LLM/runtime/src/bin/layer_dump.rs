//! Layer-Dump-Diagnose: Reststrom-Statistiken (AbsMax + erste vier Werte)
//! nach jedem Layer und nach der finalen Norm — zum Abgleich mit der
//! HF-Referenz (output_hidden_states), um Abweichungen zu lokalisieren.
//!
//! Usage: layer_dump <artifact_dir> <token_id> <pos>

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

    // Fund 20: der Residualstrom traegt seit theta_v 0.11.0 eine Skala je
    // Kanal - forward_token_dump liefert deshalb bereits umgerechnete
    // Realwerte, kein gemeinsames "frac" mehr zum Selbst-Umrechnen.
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
}
