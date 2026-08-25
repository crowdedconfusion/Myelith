//! CLI fuer Single-Node Integer-Inferenz

use std::path::PathBuf;
use integer_llm_runtime::{loader::load_model, tokenizer::Tokenizer, generate::generate};

fn main() {
    if let Err(e) = run() {
        eprintln!("[runtime] Fehler: {}", e);
        std::process::exit(1);
    }
}

/// Fuehrt die CLI aus. Fehler werden als `Err(String)` durchgereicht statt
/// per `.expect()` in einen Panic (samt Rust-Backtrace) zu laufen - fehlende
/// oder kaputte Artefakte sind ein erwartbarer Nutzungsfehler, kein Bug.
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <artifact_dir> <prompt> [max_tokens]", args[0]);
        eprintln!("       {} <artifact_dir> --prompts <datei> [max_tokens]", args[0]);
        std::process::exit(1);
    }

    let artifact_dir = PathBuf::from(&args[1]);

    // ⚑ **Mehrere Prompts in einem Prozess** (2026-08-25).
    //
    // Bis heute nahm dieses Binary genau einen Prompt entgegen, und
    // `bench/qualitativ.py` startete es je Prompt neu. Das war richtig,
    // solange Laden drei Sekunden kostete. **Bei Qwen3-30B-A3B sind es
    // rund zwei Minuten**, und der Benchmark braucht acht Prompts mal
    // zwei Laeufe: sechzehn Ladevorgaenge, also gut eine halbe Stunde
    // ausschliesslich fuer das Lesen derselben 29 GiB.
    //
    // Mit `--prompts <datei>` (eine Zeile je Prompt) laedt das Binary
    // einmal und generiert fuer jede Zeile. Die Ausgabe je Prompt ist
    // dieselbe wie im Einzelfall, damit vorhandene Auswertungen sie
    // unveraendert lesen koennen.
    let mehrfach = args[2] == "--prompts";
    let prompts: Vec<String> = if mehrfach {
        let datei = args.get(3).ok_or("--prompts braucht eine Datei")?;
        let inhalt = std::fs::read_to_string(datei)
            .map_err(|e| format!("Promptdatei {datei} nicht lesbar: {e}"))?;
        let zeilen: Vec<String> = inhalt
            .lines()
            .map(|z| z.trim_end().to_string())
            .filter(|z| !z.is_empty())
            .collect();
        if zeilen.is_empty() {
            return Err(format!("Promptdatei {datei} enthaelt keine Zeile"));
        }
        zeilen
    } else {
        vec![args[2].clone()]
    };
    let token_arg = if mehrfach { args.get(4) } else { args.get(3) };
    let max_tokens: usize = match token_arg {
        Some(s) => s.parse().map_err(|_| {
            format!("max_tokens muss eine positive Ganzzahl sein, erhalten: '{}'", s)
        })?,
        None => 20,
    };

    if !artifact_dir.exists() {
        return Err(format!(
            "Artefakt-Verzeichnis nicht gefunden: {}",
            artifact_dir.display()
        ));
    }
    if !artifact_dir.is_dir() {
        return Err(format!(
            "Artefakt-Pfad ist kein Verzeichnis: {}",
            artifact_dir.display()
        ));
    }

    println!("[runtime] Lade Modell aus {}...", artifact_dir.display());
    let model = load_model(&artifact_dir).map_err(|e| {
        format!("Modell-Ladung fehlgeschlagen ({}): {}", artifact_dir.display(), e)
    })?;
    println!(
        "[runtime] Modell geladen: {} Layer, {} Heads, hidden={}",
        model.num_layers, model.num_heads, model.hidden_size
    );

    let tokenizer_path = artifact_dir.join("tokenizer.json");
    if !tokenizer_path.exists() {
        return Err(format!(
            "tokenizer.json fehlt im Artefakt-Verzeichnis: {}",
            tokenizer_path.display()
        ));
    }
    let tokenizer_path_str = tokenizer_path
        .to_str()
        .ok_or_else(|| format!("Artefakt-Pfad ist kein gueltiges UTF-8: {}", tokenizer_path.display()))?;
    let tokenizer = Tokenizer::from_file(tokenizer_path_str).map_err(|e| {
        format!("Tokenizer-Ladung fehlgeschlagen ({}): {}", tokenizer_path.display(), e)
    })?;
    let seed = 42u64;

    for prompt in &prompts {
        // Prompt-Token-IDs ausgeben: nuetzlich fuer Bitexaktheits-Abgleiche
        // mit der HF-Referenz (identische Tokenisierung ist Voraussetzung
        // dafuer, dass Numerik-Vergleiche vergleichbare Eingaben sehen).
        println!("[runtime] Prompt-Tokens: {:?}", tokenizer.encode(prompt));
        println!("[runtime] Prompt: {}", prompt);
        println!("[runtime] Generiere bis zu {} Token (greedy)...", max_tokens);

        let tokens = generate(&model, &tokenizer, prompt, max_tokens, seed, true);

        println!("[runtime] Generierte Token: {:?}", tokens);
        println!(
            "[runtime] Token-Hash: {}",
            integer_llm_runtime::generate::hash_tokens(&tokens)
        );
    }

    Ok(())
}
