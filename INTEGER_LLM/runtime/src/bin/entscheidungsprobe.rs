//! Entscheidungsprobe: **welches Token der Ganzzahlpfad je Position
//! waehlt**, und wie sicher er dabei war.
//!
//! # ⚑ Warum es diese Probe neben `perplexity_probe` gibt (2026-09-16)
//!
//! `perplexity_probe` misst, welche Wahrscheinlichkeit der Ganzzahlpfad
//! dem **vorgegebenen** naechsten Token gibt. Das ist ein Mittelwert
//! ueber Wahrscheinlichkeiten, und er beantwortet nicht die Frage, an
//! der der ausgelieferte Client haengt: **Waehlt der Ganzzahlpfad
//! dasselbe Token wie die Gleitkomma-Referenz?**
//!
//! Der Unterschied ist nicht akademisch. Bei Teacher-Forcing wird der
//! Kontext an jeder Position auf die Wahrheit zurueckgesetzt, eine
//! Abweichung bei Token `t` wirkt also nicht auf `t+1`. Der Agent macht
//! das Gegenteil: freie Generierung, und **seine Werkzeugaufrufe sind
//! JSON**. Eine andere Entscheidung ist dort ein anderer Dateipfad, kein
//! Stilunterschied.
//!
//! ⚑ **Und die Probe misst mit, ob eine Abweichung ueberhaupt etwas
//! bedeutet.** Wo die Referenz selbst zwischen zwei Token schwankt (der
//! Abstand zwischen Rang 1 und Rang 2 ist winzig), ist eine andere Wahl
//! kein Mangel, sondern eine Muenze, die anders faellt. Wo die Referenz
//! sicher ist und der Ganzzahlpfad trotzdem abweicht, ist es einer.
//! **Ohne diesen Abstand ist eine Abweichungsquote nicht deutbar.**
//!
//! ⛔️ **Kein Teil des Auslieferungspfads.** Die Log-Softmax laeuft in
//! f64: Das ist der Messpfad. Die Logits selbst entstehen ausschliesslich
//! im Ganzzahlpfad.
//!
//! Eingabe wie bei `perplexity_probe`: eine Sequenz je Zeile, Token-IDs
//! durch Leerzeichen getrennt.
//!
//! Ausgabe je Position eine Zeile:
//!
//! ```text
//! ENT <seq> <pos> <ziel> <top1> <lp1> <top2> <lp2> <ziel_lp> <ziel_rang>
//! ```
//!
//! `lp` ist jeweils die Log-Wahrscheinlichkeit (log-softmax). Der
//! Abstand `lp1 - lp2` ist der Entscheidungsabstand an dieser Position.
//!
//! Usage: entscheidungsprobe <artifact_dir> <sequenzdatei>

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: entscheidungsprobe <artifact_dir> <sequenzdatei>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let text = std::fs::read_to_string(&args[2]).expect("Sequenzdatei unlesbar");

    let logit_scale = 2f64.powi(-(model.config.logit_frac_bits as i32));

    for (seq_idx, line) in text.lines().enumerate() {
        let ids: Vec<usize> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if ids.len() < 2 {
            continue;
        }

        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

        // ⚑ **Gebuendelt statt Token fuer Token** (2026-09-16).
        //
        // ⛔️ Bis hierher lief hier `forward_token` in einer Schleife, und
        // damit erreichte diese Messung **weder die gebuendelte
        // Vorbereitung noch die GPU**: Die rechnet erst ab sechzehn
        // Eingaben je Buendel, und ein einzelnes Token kommt dort nie an.
        // **Die Messung lief also auf dem einen Pfad, den die
        // Optimierung nicht beruehrt.**
        //
        // ⚑ **Dieselbe Rechnung, andere Reihenfolge**, und das ist
        // geprueft: `die_gebuendelten_logits_sind_die_tokenweisen`
        // vergleicht jede Position einzeln und ueber die Fenstergrenze
        // hinweg. **Ohne diese Gegenprobe waere jede Zahl danach mit den
        // frueheren unvergleichbar, und niemand saehe es ihr an.**
        let alle = model.logits_stapel(&ids, 0, &mut cache);

        for (pos, logits) in alle.iter().enumerate() {
            if pos + 1 >= ids.len() {
                continue;
            }
            let ziel = ids[pos + 1];

            // ⚑ **Rang eins und zwei in einem Durchgang**, ohne die
            // Vokabelliste zu sortieren: Das Vokabular hat sechsstellig
            // viele Eintraege, und ein Sortieren je Position waere
            // teurer als der Forward-Pass selbst.
            let mut top1 = 0usize;
            let mut top2 = 0usize;
            let mut v1 = i32::MIN;
            let mut v2 = i32::MIN;
            for (i, &v) in logits.iter().enumerate() {
                if v > v1 {
                    v2 = v1;
                    top2 = top1;
                    v1 = v;
                    top1 = i;
                } else if v > v2 {
                    v2 = v;
                    top2 = i;
                }
            }

            // Rang des Zieltokens: wie viele Logits echt groesser sind.
            // ⚠️ **Echt groesser, nicht groesser-gleich.** Bei einem
            // Gleichstand bekommt das Ziel den besseren Rang; alles
            // andere machte aus einem Unentschieden einen Nachteil.
            let ziel_wert = logits[ziel];
            let ziel_rang = 1 + logits.iter().filter(|&&v| v > ziel_wert).count();

            // Log-Softmax im Messpfad (f64), wie in `perplexity_probe`.
            // z_max ueber die SKALIERTEN Werte, sonst unterlaufen alle
            // exp-Terme zu 0.
            let z_max = logits.iter().map(|&v| v as f64).fold(f64::NEG_INFINITY, f64::max)
                * logit_scale;
            let mut lse = 0.0f64;
            for &v in logits.iter() {
                lse += ((v as f64 * logit_scale) - z_max).exp();
            }
            let lse_ln = lse.ln();
            let lp = |v: i32| (v as f64 * logit_scale) - z_max - lse_ln;

            println!(
                "ENT {} {} {} {} {:.6} {} {:.6} {:.6} {}",
                seq_idx,
                pos,
                ziel,
                top1,
                lp(v1),
                top2,
                lp(v2),
                lp(ziel_wert),
                ziel_rang
            );
        }
    }
}
