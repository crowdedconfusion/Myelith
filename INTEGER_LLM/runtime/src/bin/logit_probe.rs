//! Diagnose-Binary: Logits nach dem Prefill eines Prompts ausgeben.
//!
//! Vergleich mit der HF-Referenz (Top-k-Ranking), um Numerik-Fehler von
//! normaler Quantisierungs-Drift zu unterscheiden. Kein Teil des
//! Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: logit_probe <artifact_dir> <prompt>");
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
    println!("Prompt-Tokens: {:?}", ids);

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut logits = Vec::new();
    for (pos, &tid) in ids.iter().enumerate() {
        logits = model.forward_token(tid, pos, &mut cache);
    }

    // ⚑ **Beide Wege in EINEM Lauf** (Fund 426 und was danach bleibt):
    //   Token fuer Token gegen gebuendelt, auf denselben Token, mit
    //   frischem Zwischenspeicher. Ein Unterschied hier ist ein Fund,
    //   und zwei Laeufe mit je einer Modellladung waeren dafuer zu
    //   teuer und zu leicht zu verwechseln.
    {
        let mut c2 = KVCache::new(model.num_layers, model.num_kv_heads);
        let alle = model.logits_stapel(&ids, 0, &mut c2);
        let st = alle.last().expect("mindestens ein Token");
        let gleich = st.iter().zip(logits.iter()).filter(|(a, b)| a == b).count();
        let arg_e = (0..logits.len()).max_by_key(|&i| logits[i]).unwrap();
        let arg_s = (0..st.len()).max_by_key(|&i| st[i]).unwrap();
        let summe: f64 = st
            .iter()
            .zip(logits.iter())
            .map(|(a, b)| f64::from(a - b).powi(2))
            .sum();
        let norm: f64 = logits.iter().map(|&x| f64::from(x).powi(2)).sum();
        println!(
            "Gebuendelt gegen einzeln: {gleich}/{} identisch, rel.L2 {:.4}, \
             Spitze einzeln {arg_e}, gebuendelt {arg_s}",
            logits.len(),
            (summe / norm.max(1.0)).sqrt()
        );
    }

    // ⚑ **Was die Abtastung aus diesen Logits machen wuerde.**
    //
    // `sample_integer_cdf` gewichtet **linear**: `w_i = z_i - min + 1`,
    // nicht exponentiell. Ob das eine brauchbare Verteilung ergibt,
    // haengt allein an der Weite des Wertebereichs gegen die Groesse
    // des Vokabulars, und beides steht erst zur Laufzeit fest.
    //
    // 📌 **Deshalb wird es hier gerechnet und nicht behauptet.** Eine
    // Zahl im Kommentar waere eine Behauptung; diese Zeilen geben sie
    // aus. Gleitkomma ist hier erlaubt, denn `bin/` ist kein
    // Rechenpfad und steht als Ausnahme im Gleitkomma-Audit.
    {
        let min = *logits.iter().min().expect("mindestens ein Logit");
        let max = *logits.iter().max().expect("mindestens ein Logit");
        let summe: i128 = logits.iter().map(|&z| i128::from(z - min + 1)).sum();
        let spitze = i128::from(max - min + 1);
        let p_spitze = spitze as f64 / summe as f64;
        // ⚠️ **Hier stand kurz ein Vergleich mit einer Softmax, und er
        //    ist wieder weg.** Um Logits in Wahrscheinlichkeiten zu
        //    wandeln, braucht es die Skala des Artefakts; wer sie frei
        //    waehlt, bekommt eine Zahl, die nichts bedeutet und trotzdem
        //    wie eine Messung aussieht. Was hier steht, haengt an nichts
        //    als den Logits selbst.
        println!(
            "Abtastung: Vokabular {}, Logits von {} bis {}, Spanne {}",
            logits.len(),
            min,
            max,
            max - min
        );
        println!(
            "  linear gewichtet (so rechnet sample_integer_cdf): P(Spitze) = {:.3e}, \
             also 1 zu {:.0}",
            p_spitze,
            1.0 / p_spitze
        );
        println!(
            "  Anteil des Vokabulars, der zusammen mehr Gewicht traegt als die Spitze: {:.4} %",
            100.0 * (1.0 - p_spitze)
        );
    }

    let mut idx: Vec<usize> = (0..logits.len()).collect();
    idx.sort_by(|&a, &b| logits[b].cmp(&logits[a]));

    println!("Top-10 Logits (id: wert):");
    for &i in idx.iter().take(10) {
        println!("  {}: {}", i, logits[i]);
    }
    for probe in [594usize, 295, 2746, 6250] {
        let rank = idx.iter().position(|&x| x == probe).map(|p| p + 1).unwrap();
        println!("Logit {} = {} (Rang {})", probe, logits[probe], rank);
    }
}
