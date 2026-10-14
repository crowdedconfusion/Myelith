//! Kontextprobe: Wie schnell ist der Decode bei wachsendem Kontext?
//!
//! Liest eine Folge von Token (eine Zeile, durch Leerzeichen getrennt),
//! bereitet sie in Stuecken bis zu jeder Marke vor und erzeugt an jeder
//! Marke einige Token gierig. Ausgabe je Marke: Kontextlaenge, Durchsatz
//! der Vorbereitung bis dorthin und Millisekunden je erzeugtem Token.
//!
//! ⚑ **Die erzeugten Token bleiben im Kontext**, und die Folge geht danach
//! weiter. Gemessen wird die Zeit, nicht der Text; ein Zurueckschneiden
//! ginge beim Gemisch mit Zustandsebenen nur ueber Merkpunkte und kostete
//! selbst Zeit.
//!
//! Die Zeitmessung laeuft in f64: Das ist der MESSPFAD, nicht der
//! Inferenzpfad. Kein Teil des Auslieferungspfads.
//!
//! ⚑ **Mit `--lehrer` statt der Erzeugung die Perplexitaet** der naechsten
//! Token aus der Datei an jeder Marke: So laesst sich messen, wie gut das
//! Modell weit hinten im Kontext noch vorhersagt (etwa zwei Drehtabellen
//! gegeneinander).
//!
//! Usage: kontextprobe <artefakt> <tokendatei> <marken, etwa 2048,32768> <decode_tokens> [--lehrer]
use std::time::Instant;

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("Usage: kontextprobe <artefakt> <tokendatei> <marken> <decode_tokens>");
        std::process::exit(1);
    }
    let model = load_model(std::path::Path::new(&args[1])).expect("Modell-Ladung fehlgeschlagen");
    let ids: Vec<usize> = std::fs::read_to_string(&args[2])
        .expect("Tokendatei")
        .split_whitespace()
        .map(|t| t.parse().expect("Token-ID"))
        .collect();
    let mut marken: Vec<usize> = args[3].split(',').map(|m| m.trim().parse().expect("Marke")).collect();
    marken.sort_unstable();
    let decode: usize = args[4].parse().expect("decode_tokens");
    let lehrer = args.get(5).is_some_and(|a| a == "--lehrer");
    let logit_skala = 2f64.powi(-(model.config.logit_frac_bits as i32));
    println!("kontextgrenze {}", model.kontextgrenze());

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let mut pos = 0usize;
    let mut quelle = 0usize;
    for marke in marken {
        if marke + decode > model.kontextgrenze() {
            println!("marke {marke} liegt hinter der Kontextgrenze; Ende");
            break;
        }
        let bis = marke.saturating_sub(pos);
        if quelle + bis > ids.len() {
            println!("marke {marke}: die Tokendatei reicht nicht; Ende");
            break;
        }
        let stueck = &ids[quelle..quelle + bis];
        let t0 = Instant::now();
        let letztes = stueck.len().saturating_sub(1);
        model.vorbereiten_stapel(&stueck[..letztes], pos, &mut cache);
        let mut logits = model.forward_token(stueck[letztes], pos + letztes, &mut cache);
        let vorbereitung = t0.elapsed().as_secs_f64();
        quelle += bis;
        pos += bis;

        if lehrer {
            // Die naechsten `decode` Token der Datei, als Lehrervorgabe; die
            // Logits der Marke selbst sagen das erste voraus.
            let folge = &ids[quelle..(quelle + decode).min(ids.len())];
            let t0 = Instant::now();
            let mehr = model.logits_stapel(folge, pos, &mut cache);
            let dauer = t0.elapsed().as_secs_f64();
            let mut summe = 0.0f64;
            let mut zahl = 0usize;
            for (i, ziel) in folge.iter().enumerate() {
                let l = if i == 0 { &logits } else { &mehr[i - 1] };
                let hoch = l.iter().map(|&v| v as f64).fold(f64::NEG_INFINITY, f64::max) * logit_skala;
                let lse: f64 = l.iter().map(|&v| (v as f64 * logit_skala - hoch).exp()).sum();
                summe += l[*ziel] as f64 * logit_skala - hoch - lse.ln();
                zahl += 1;
            }
            quelle += folge.len();
            pos += folge.len();
            println!(
                "kontext {marke} vorbereitet {bis} in {vorbereitung:.1} s ({:.1} Tok/s) perplexitaet {:.4} ueber {zahl} Token ({dauer:.1} s)",
                bis as f64 / vorbereitung,
                (-summe / zahl as f64).exp()
            );
            continue;
        }
        let t0 = Instant::now();
        for _ in 0..decode {
            let naechstes = model.greedy_next(&logits);
            logits = model.forward_token(naechstes, pos, &mut cache);
            pos += 1;
        }
        let erzeugung = t0.elapsed().as_secs_f64();
        println!(
            "kontext {marke} vorbereitet {bis} in {vorbereitung:.1} s ({:.1} Tok/s) decode {:.1} ms/Tok ({:.2} Tok/s)",
            bis as f64 / vorbereitung,
            erzeugung * 1000.0 / decode as f64,
            decode as f64 / erzeugung
        );
    }
    println!("Fertig");
}
