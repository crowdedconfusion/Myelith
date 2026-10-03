//! **Übersteht die Expertenwahl eine ternäre Umwandlung?** (L13, erster Teil:
//! messen, bevor gebaut wird.)
//!
//! Aufruf:
//!     routerumwandlung <artefakt> <von> <bis> [--router] [--geteilt]
//!
//! Zweimal derselbe Text, Token für Token: erst mit dem Artefakt, dann mit
//! den Experten der Ebenen `von..bis` **an Ort und Stelle ternär
//! gerundet**, genau so, wie das Training sie ableitet
//! (`gewicht_aus_master_als`, Form `Ternaer`). Der Router bleibt int8, außer
//! mit `--router`; der geteilte Experte bleibt int8, außer mit `--geteilt`.
//!
//! Gemeldet wird je Gemischebene, wie viele der gewählten Experten gleich
//! geblieben sind (Mittel über die Positionen, in Teilen von `top_k`), wie
//! oft die ganze Auswahl gleich blieb, und die Perplexität des Textes vorher
//! und nachher.
//!
//! ⚑ **Warum die Rundung an Ort und Stelle und nicht ein Training:** Die
//! Frage ist, wo der Router ohne Hilfe steht, also wie viel ein Training
//! zurückholen muss. Das Fachpapier (6.6) sagt „nicht an Ort und Stelle
//! ternarisieren, Router hochaufgelöst“; diese Probe misst beides getrennt.
//!
//! ⚠️ **Speicher:** Je umgewandelter Ebene des 35B liegen rund 0,8 GB
//! Experten im Arbeitsspeicher statt im Abbild. Mehr als acht Ebenen auf
//! einmal passen auf 24 GiB nicht neben das übrige Modell.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_kernels::optimierer::MASTER_FRAC;
use integer_llm_kernels::trainingsschritt::{gewicht_aus_master_als, Gewichtsform};
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::messung::{kreuzentropie_aus_logits, perplexitaet};
use integer_llm_runtime::model::{Feedforward, Gewichtsdaten, IntegerModel, QTensor};
use integer_llm_runtime::tokenizer::Tokenizer;

/// Ein fester Text, gemischt aus Sachtext, Rechnung und Code, damit nicht
/// eine Sorte Experten das Bild bestimmt.
const TEXT: &str = "Die Hauptstadt von Frankreich ist Paris. Sie liegt an der Seine und hat \
rund zwei Millionen Einwohner. If a train travels 120 kilometers in 1.5 hours, its average \
speed is 80 kilometers per hour. def mittelwert(werte):\n    return sum(werte) / len(werte)\n\
Photosynthesis converts light energy into chemical energy stored in glucose. Der Satz des \
Pythagoras lautet a^2 + b^2 = c^2.";

fn main() {
    if let Err(e) = run() {
        eprintln!("[routerumwandlung] FEHLER: {e}");
        std::process::exit(1);
    }
}

/// Je Position und Ebene die gewählten Experten, und je Position die Logits.
struct Durchlauf {
    wahl: Vec<Vec<(usize, Vec<u16>)>>,
    verlust: f64,
    ausgewertet: usize,
}

fn durchlaufen(m: &IntegerModel, ids: &[usize]) -> Durchlauf {
    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    let mut wahl = Vec::with_capacity(ids.len());
    let (mut verlust, mut ausgewertet) = (0.0f64, 0usize);
    for (pos, &t) in ids.iter().enumerate() {
        let (logits, befunde) = m.forward_token_mit_routing(t, pos, &mut cache);
        wahl.push(befunde.into_iter().map(|b| (b.layer, b.experten)).collect());
        if let Some(&ziel) = ids.get(pos + 1) {
            let ce = kreuzentropie_aus_logits(&logits, ziel, m.config.logit_frac_bits);
            if ce.is_finite() {
                verlust += ce;
                ausgewertet += 1;
            }
        }
    }
    Durchlauf { wahl, verlust, ausgewertet }
}

/// Rundet eine int8-Matrix an Ort und Stelle ternär, wie das Training es tut.
fn ternaer_runden(t: &mut QTensor) {
    let cols = t.cols();
    let master: Vec<i32> = t
        .data
        .iter()
        .enumerate()
        .map(|(i, &w)| i32::from(w) << (MASTER_FRAC - t.shifts[i / cols]))
        .collect();
    let (w, s) = gewicht_aus_master_als(&master, cols, MASTER_FRAC, Gewichtsform::Ternaer);
    t.data = std::sync::Arc::new(Gewichtsdaten::Speicher(w));
    t.shifts = s;
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err("Usage: routerumwandlung <artefakt> <von> <bis> [--router] [--geteilt]".into());
    }
    let dir = std::path::Path::new(&args[1]);
    let von: usize = args[2].parse().map_err(|_| "von ist keine Zahl")?;
    let bis: usize = args[3].parse().map_err(|_| "bis ist keine Zahl")?;
    let router = args.iter().any(|a| a == "--router");
    let geteilt = args.iter().any(|a| a == "--geteilt");
    let mut m = load_model(dir).map_err(|e| format!("Laden: {e}"))?;
    if von >= bis || bis > m.num_layers {
        return Err(format!("Bereich {von}..{bis} passt nicht zu {} Ebenen", m.num_layers));
    }
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().ok_or("Pfad")?)?;
    let ids = tok.encode(TEXT);
    println!(
        "[routerumwandlung] {} Token | Experten der Ebenen {von}..{bis} ternär | Router {} | geteilter Experte {}",
        ids.len(),
        if router { "ternär" } else { "int8" },
        if geteilt { "ternär" } else { "int8" }
    );

    let vorher = durchlaufen(&m, &ids);
    let start = std::time::Instant::now();
    let mut umgewandelt = 0usize;
    for e in von..bis {
        let Feedforward::Moe(moe) = &mut m.layers[e].ffn else { continue };
        for ex in moe.experts.iter_mut() {
            for t in [&mut ex.gate_proj, &mut ex.up_proj, &mut ex.down_proj] {
                ternaer_runden(t);
            }
        }
        if router {
            ternaer_runden(&mut moe.router);
        }
        if geteilt {
            if let Some(ge) = moe.geteilter_experte.as_mut() {
                for t in [&mut ge.mlp.gate_proj, &mut ge.mlp.up_proj, &mut ge.mlp.down_proj] {
                    ternaer_runden(t);
                }
            }
        }
        umgewandelt += 1;
    }
    println!("[routerumwandlung] {umgewandelt} Gemischebenen umgewandelt in {:.1} s", start.elapsed().as_secs_f64());
    let nachher = durchlaufen(&m, &ids);

    println!(
        "[routerumwandlung] Perplexität vorher {:.3}, nachher {:.3}",
        perplexitaet(vorher.verlust, vorher.ausgewertet),
        perplexitaet(nachher.verlust, nachher.ausgewertet)
    );
    println!("[routerumwandlung] Ebene | gleiche Experten (Anteil an top_k) | ganze Auswahl gleich");
    let ebenen: Vec<usize> = vorher.wahl.first().map(|w| w.iter().map(|(l, _)| *l).collect()).unwrap_or_default();
    for (j, ebene) in ebenen.iter().enumerate() {
        let (mut gleich, mut moeglich, mut ganz) = (0usize, 0usize, 0usize);
        for (a, b) in vorher.wahl.iter().zip(&nachher.wahl) {
            let (ea, eb) = (&a[j].1, &b[j].1);
            let mut sa = ea.clone();
            let mut sb = eb.clone();
            sa.sort_unstable();
            sb.sort_unstable();
            gleich += sa.iter().filter(|x| sb.binary_search(x).is_ok()).count();
            moeglich += sa.len();
            ganz += usize::from(sa == sb);
        }
        let markierung = if (von..bis).contains(ebene) { " (umgewandelt)" } else { "" };
        println!(
            "[routerumwandlung] {ebene:>5} | {:.3} | {:.3}{markierung}",
            gleich as f64 / moeglich.max(1) as f64,
            ganz as f64 / vorher.wahl.len().max(1) as f64
        );
    }
    println!("[routerumwandlung] Fertig");
    Ok(())
}
