//! Router-Sonde: wie oft trägt der Tie-Break die Auswahl?
//!
//! Beantwortet Messpunkt 12.81c. Die Frage ist **nicht**, wie viele
//! Router-Logits zufällig gleich sind, sondern wie oft ein Gleichstand
//! **an der Auswahlgrenze** liegt. Ein Gleichstand zwischen Rang 1 und 2
//! ändert nichts, beide feuern. Nur zwischen Rang k und k+1 entscheidet
//! die Tie-Break-Regel, welcher von beiden rechnet, und **nur dort** kann
//! sie zwei Knoten auseinanderbringen.
//!
//! ## Warum die Zahl zählt
//!
//! `kernels::moe` legt fest: Bei Gleichstand gewinnt der kleinere
//! Expertenindex, und ausgewählt wird über die Logits statt über die
//! Wahrscheinlichkeiten, weil der Weg über die exp-Tabelle Gleichstände
//! erzeugt, die es vorher nicht gab. Beides ist eine Vorsichtsmaßnahme.
//! Ohne diese Messung ist unbekannt, wie oft sie überhaupt greift, und
//! damit auch, wie groß der Schaden einer falschen Regel wäre.
//!
//! **Ein Ergebnis von null wäre keine Entwarnung**, sondern hieße: Bei
//! diesem Modell und diesen Eingaben trägt der Tie-Break nichts, und die
//! Regel bleibt trotzdem nötig, weil ein anderes Modell oder eine andere
//! Kalibrierung sie braucht.
//!
//! Aufruf: `router_probe <artefaktordner> <prompt> [max_token]`

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: router_probe <artefaktordner> <prompt> [max_token]");
        std::process::exit(1);
    }
    let dir = std::path::Path::new(&args[1]);
    let prompt = &args[2];
    let max_token: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(16);

    eprintln!("[router_probe] Lade Modell aus {} ...", dir.display());
    let model = match load_model(dir) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[router_probe] Laden fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };
    let tok_pfad = dir.join("tokenizer.json");
    let tokenizer = match Tokenizer::from_file(tok_pfad.to_str().unwrap()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[router_probe] Tokenizer fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };

    let mut ids = tokenizer.encode(prompt);
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    // Zählwerk über alle beobachteten (Position, Layer)-Paare.
    let mut paare: u64 = 0;
    let mut mit_gleichstand: u64 = 0;
    let mut gleichstaende_gesamt: u64 = 0;
    let mut groesster: usize = 0;
    let mut je_layer: Vec<u64> = vec![0; model.num_layers];
    // Vergleichszaehlung: dieselbe Frage ueber die Wahrscheinlichkeiten.
    let mut mit_gleichstand_w: u64 = 0;
    let mut gleichstaende_w: u64 = 0;

    let mut pos = 0usize;
    let mut naechstes = ids[0];
    let mut erzeugt = 0usize;
    loop {
        let (logits, befunde) = model.forward_token_mit_routing(naechstes, pos, &mut cache);
        for b in &befunde {
            paare += 1;
            gleichstaende_gesamt += b.randgleichstaende as u64;
            if b.randgleichstaende > 0 {
                mit_gleichstand += 1;
                je_layer[b.layer] += 1;
                groesster = groesster.max(b.randgleichstaende);
            }
            gleichstaende_w += b.randgleichstaende_wahrscheinlichkeit as u64;
            if b.randgleichstaende_wahrscheinlichkeit > 0 {
                mit_gleichstand_w += 1;
            }
        }
        if paare == 0 {
            eprintln!(
                "[router_probe] Dieses Modell hat keine MoE-Layer. \
                 Es gibt nichts zu messen, und das ist kein Fehler."
            );
            std::process::exit(0);
        }
        pos += 1;
        if pos < ids.len() {
            naechstes = ids[pos];
            continue;
        }
        let next = model.greedy_next(&logits);
        ids.push(next);
        naechstes = next;
        erzeugt += 1;
        if erzeugt >= max_token {
            break;
        }
    }

    println!("[router_probe] Positionen: {}", pos);
    println!("[router_probe] beobachtete (Position, Layer)-Paare: {paare}");
    println!(
        "[router_probe] Paare mit Gleichstand an der Auswahlgrenze: {mit_gleichstand} \
         ({:.4} %)",
        100.0 * mit_gleichstand as f64 / paare as f64
    );
    println!("[router_probe] Gleichstände insgesamt: {gleichstaende_gesamt}");
    println!("[router_probe] größter Gleichstand an einer Grenze: {groesster}");

    println!();
    println!(
        "[router_probe] Zum Vergleich, ueber die Wahrscheinlichkeiten statt ueber die Logits:"
    );
    println!(
        "[router_probe]   Paare mit Gleichstand: {mit_gleichstand_w} ({:.4} %), \
         Gleichstaende insgesamt: {gleichstaende_w}",
        100.0 * mit_gleichstand_w as f64 / paare as f64
    );
    if mit_gleichstand > 0 {
        println!(
            "[router_probe]   Faktor gegenueber der Auswahl ueber Logits: {:.1}x",
            mit_gleichstand_w as f64 / mit_gleichstand as f64
        );
    }
    println!();

    let betroffene: Vec<usize> = je_layer
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, _)| i)
        .collect();
    if betroffene.is_empty() {
        println!("[router_probe] Keine Layer betroffen: Die Auswahl war überall eindeutig.");
    } else {
        println!(
            "[router_probe] betroffene Layer ({}): {:?}",
            betroffene.len(),
            betroffene
        );
    }
}
