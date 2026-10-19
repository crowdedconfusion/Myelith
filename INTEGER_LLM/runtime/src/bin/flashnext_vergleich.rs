//! **Pruefstand fuer die Bauart mit vier Stroemen** (2026-10-19).
//!
//! Laedt ein Artefakt, rechnet eine Tokenfolge Token fuer Token und gibt die
//! Logits jeder Position als JSON aus: `{"logit_frac": k, "logits": [[...],
//! ...]}`. Die Probe `INTEGER_LLM/tests/test_flashnext_mini.py` haelt sie
//! gegen die Vorlage.
//!
//! ```text
//! flashnext_vergleich <artefakt> '[3, 17, 1, 99]'
//! ```
//!
//! ⚑ **Zusaetzlich gebuendelt** (`logits_stapel`) auf frischem Speicher: Ein
//! Unterschied zwischen beiden Wegen ist ein Fund und bricht mit Rueckgabe 1
//! ab.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Aufruf: flashnext_vergleich <artefakt> '<token als JSON-Liste>'");
        std::process::exit(2);
    }
    let modell = match load_model(std::path::Path::new(&args[1])) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Laden fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };
    let folge: Vec<usize> = match serde_json::from_str(&args[2]) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Tokenfolge ist keine JSON-Liste ganzer Zahlen: {e}");
            std::process::exit(2);
        }
    };

    let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
    let einzeln: Vec<Vec<i32>> = folge.iter().enumerate().map(|(pos, &t)| modell.forward_token(t, pos, &mut cache)).collect();

    let mut c2 = KVCache::new(modell.num_layers, modell.num_kv_heads);
    let gebuendelt = modell.logits_stapel(&folge, 0, &mut c2);
    if gebuendelt != einzeln {
        eprintln!("gebuendelt und einzeln rechnen verschieden");
        std::process::exit(1);
    }

    println!(
        "{}",
        serde_json::json!({ "logit_frac": modell.config.logit_frac_bits, "logits": einzeln })
    );
}
