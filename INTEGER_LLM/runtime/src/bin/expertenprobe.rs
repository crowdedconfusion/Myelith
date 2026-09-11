//! **Sonde: Wie ungleich fallen die Experten?**
//!
//! # ⚑ Die Frage, an der haengt, ob ein 30B-Gemisch auf einer kleinen
//! Maschine laufen kann
//!
//! Das Artefakt von `myelith-30b-a3b` ist 30,8 GB, davon **29 GB
//! Experten**; je Token feuern 8 von 128 in jeder der 48 Ebenen, also
//! 6,25 %. Passt das Artefakt nicht in den Arbeitsspeicher, entscheidet
//! alles daran, **wie gleichmaessig** sich diese 6,25 % ueber die
//! Experten verteilen:
//!
//! - **Gleichverteilt:** Jeder Experte wird gleich oft gebraucht, ein
//!   Zwischenspeicher haelt immer die falschen, und die Platte bleibt
//!   der Engpass.
//! - **Schief:** Wenige Experten tragen die meisten Aufrufe, ein
//!   Zwischenspeicher von wenigen Gigabyte faengt sie, und der Rest
//!   kommt selten von der Platte.
//!
//! **Das ist keine Herleitung, sondern eine Messung**, und diese Datei
//! ist sie.
//!
//! ⚠️ **Sie misst dieses Modell an diesem Text.** Ein anderer Text kann
//! andere Experten waehlen; was sie beantwortet, ist die Frage, **ob es
//! ueberhaupt eine Schiefe gibt**, nicht welche.
//!
//! Aufruf: `expertenprobe <artefakt> <text> [neue_token]`

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: expertenprobe <artefakt> <text> [neue_token]");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let text = &args[2];
    let neue: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

    let model = load_model(&dir).expect("Modell-Ladung");
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad"))
        .expect("Tokenizer");
    let ids = tok.encode(text);
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    // zaehler[ebene][experte]
    let mut zaehler: Vec<Vec<u32>> = Vec::new();
    let mut besuche: u64 = 0;
    let mut logits = vec![0i32; model.vocab_size];
    let mut pos = 0usize;

    let aufnehmen = |befunde: Vec<integer_llm_runtime::model::Routingbefund>,
                         zaehler: &mut Vec<Vec<u32>>,
                         besuche: &mut u64| {
        for b in befunde {
            while zaehler.len() <= b.layer {
                zaehler.push(Vec::new());
            }
            for e in b.experten {
                let z = &mut zaehler[b.layer];
                if z.len() <= e as usize {
                    z.resize(e as usize + 1, 0);
                }
                z[e as usize] += 1;
                *besuche += 1;
            }
        }
    };

    for &tid in &ids {
        let (l, befunde) = model.forward_token_mit_routing(tid, pos, &mut cache);
        logits = l;
        aufnehmen(befunde, &mut zaehler, &mut besuche);
        pos += 1;
    }
    for _ in 0..neue {
        let next = model.greedy_next(&logits);
        let (l, befunde) = model.forward_token_mit_routing(next, pos, &mut cache);
        logits = l;
        aufnehmen(befunde, &mut zaehler, &mut besuche);
        pos += 1;
    }

    let ebenen = zaehler.len();
    let experten = zaehler.iter().map(|z| z.len()).max().unwrap_or(0);
    let token = ids.len() + neue;
    println!("# Expertenverteilung");
    println!("# {token} Token, {ebenen} Ebenen, bis {experten} Experten, {besuche} Aufrufe");
    println!();

    // ⚑ **Die Deckungskurve ist die eigentliche Antwort.** Sie sagt,
    // welcher Anteil aller Aufrufe auf die haeufigsten Experten faellt,
    // und daraus folgt unmittelbar, was ein Zwischenspeicher traegt.
    let mut alle: Vec<u32> = zaehler.iter().flat_map(|z| z.iter().copied()).collect();
    alle.sort_unstable_by(|a, b| b.cmp(a));
    let gesamt: u64 = alle.iter().map(|&x| x as u64).sum();
    let anzahl = (ebenen * experten) as f64;

    // Ein Experte belegt 3 Matrizen zu hidden x moe_inter.
    let je_experte_gb = 3.0 * 2048.0 * 768.0 / 1e9;

    println!("| Anteil der Experten | Deckung der Aufrufe | Speicher |");
    println!("|---|---|---|");
    for anteil in [0.05, 0.10, 0.20, 0.30, 0.50, 0.75, 1.00] {
        let n = ((anzahl * anteil) as usize).max(1).min(alle.len());
        let deckung: u64 = alle[..n].iter().map(|&x| x as u64).sum();
        println!(
            "| {:>4.0} % ({n}) | {:>5.1} % | {:>5.1} GB |",
            100.0 * anteil,
            100.0 * deckung as f64 / gesamt.max(1) as f64,
            n as f64 * je_experte_gb
        );
    }
    println!();

    // Wie viele Experten wurden ueberhaupt je angefasst?
    let beruehrt = alle.iter().filter(|&&x| x > 0).count();
    println!(
        "Beruehrt: {beruehrt} von {} Experten ({:.1} %), Speicher {:.1} GB",
        anzahl as usize,
        100.0 * beruehrt as f64 / anzahl,
        beruehrt as f64 * je_experte_gb
    );

    // ⚠️ Gleichverteilung als Vergleichsmass: Bei ihr deckt ein Anteil
    // x der Experten genau x der Aufrufe.
    let n20 = ((anzahl * 0.20) as usize).max(1).min(alle.len());
    let d20: u64 = alle[..n20].iter().map(|&x| x as u64).sum();
    let schiefe = (100.0 * d20 as f64 / gesamt.max(1) as f64) / 20.0;
    println!(
        "Schiefe: die haeufigsten 20 % tragen {:.2}-mal so viel wie bei Gleichverteilung",
        schiefe
    );
}
