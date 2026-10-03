//! Stapelkosten: Was kostet ein gebuendelter Durchgang mit `k` Token
//! gegen `k` einzelne Schritte, im Decode, also hinter einem fertigen
//! Prompt?
//!
//! Usage: stapelkosten <artifact_dir> [wiederholungen]
//!
//! ⚑ **Die Frage hinter Vermutungen mit exakter Pruefung**: Sie lohnen
//! nur, wenn ein Durchgang mit `k` Token weniger kostet als `k` Schritte.
//! Gemessen am 2026-10-02 kostete beim 35B ein Durchgang mit rund sechs
//! Token so viel wie 6,4 Schritte; diese Probe trennt, woran das liegt:
//! am Buendel selbst, an der Kopie des rekurrenten Zustands oder am Kopf.
//!
//! Je `k` von 1 bis 8: `k` Schritte ueber `forward_token`, dann derselbe
//! Speicher zurueckgesetzt und ein Durchgang ueber `logits_stapel`. Die
//! Logits werden verglichen; ungleich bricht die Probe ab. Gemeldet wird
//! der Median ueber die Wiederholungen.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    if let Err(e) = run() {
        eprintln!("[stapelkosten] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err("Usage: stapelkosten <artifact_dir> [wiederholungen]".into());
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let wiederholungen: usize = args.get(2).map(|s| s.parse().map_err(|_| format!("'{s}'"))).transpose()?.unwrap_or(5);
    let model = load_model(&dir).map_err(|e| format!("Modell-Ladung: {e}"))?;
    let pfad = dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(pfad.to_str().ok_or("Pfad ist kein UTF-8")?)?;
    let prompt = tokenizer.encode(
        "<|im_start|>user\nSchreibe eine kurze Funktion in Python, die eine Liste von Zahlen sortiert und den Median \
         zurueckgibt.<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
    );
    let folge = tokenizer.encode("def median(zahlen):\n    sortiert = sorted(zahlen)\n    n = len(sortiert)\n");
    if folge.len() < 8 {
        return Err("die Folge ist zu kurz".into());
    }
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let _ = model.prompt_vorbereiten(&prompt, &mut cache);
    let pos = prompt.len();
    let zustand = cache.zustand_kopie();
    println!("[stapelkosten] {} | Prompt {pos} Token | rekurrent: {}", dir.display(), zustand.is_some());

    // Die Kopie des Zustands allein.
    if let Some(z) = &zustand {
        let mut zeiten = Vec::new();
        for _ in 0..wiederholungen {
            let t = std::time::Instant::now();
            let k = cache.zustand_kopie();
            zeiten.push(t.elapsed().as_secs_f64() * 1000.0);
            drop(k);
        }
        println!("[stapelkosten] Kopie des Zustands: {:.2} ms ({:.1} MB)", median(zeiten), z.belegte_bytes() as f64 / 1e6);
    }

    let zuruecksetzen = |cache: &mut KVCache| match &zustand {
        Some(z) => cache.zurueck_auf(pos, z),
        None => {
            let bleibt = cache.kuerzen(pos);
            assert_eq!(bleibt, pos);
        }
    };
    println!("[stapelkosten]  k | k Schritte ms | Durchgang ms | Durchgang je Token ms | Verhaeltnis");
    let mut einzel_ms = 0.0;
    for k in 1..=8usize {
        let reihe = &folge[..k];
        let mut schritte = Vec::new();
        let mut stapel = Vec::new();
        for _ in 0..wiederholungen {
            zuruecksetzen(&mut cache);
            let t = std::time::Instant::now();
            let mut a = Vec::with_capacity(k);
            for (i, &tok) in reihe.iter().enumerate() {
                a.push(model.forward_token(tok, pos + i, &mut cache));
            }
            schritte.push(t.elapsed().as_secs_f64() * 1000.0);
            zuruecksetzen(&mut cache);
            let t = std::time::Instant::now();
            let b = model.logits_stapel(reihe, pos, &mut cache);
            stapel.push(t.elapsed().as_secs_f64() * 1000.0);
            if a != b {
                return Err(format!("k = {k}: Durchgang und Schritte rechnen verschiedene Logits"));
            }
        }
        let (s, b) = (median(schritte), median(stapel));
        if k == 1 {
            einzel_ms = s;
        }
        println!(
            "[stapelkosten] {k:>2} | {s:>12.1} | {b:>12.1} | {:>21.1} | {:.2} Schritte",
            b / k as f64,
            b / einzel_ms
        );
    }
    println!("[stapelkosten] Fertig");
    Ok(())
}
