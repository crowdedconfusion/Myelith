//! Aufgabenprobe: beantwortet eine Liste fertig gesetzter Chat-Prompts
//! und gibt je Prompt die Antwort als eine JSON-Zeile aus.
//!
//! Gedacht fuer Aufgabenbenchmarks (Rechenaufgaben, einfache
//! Codeaufgaben), bei denen die Perplexitaet nicht genuegt: Sie verdeckt,
//! ob ein Modell noch rechnen kann. Kein Teil des Auslieferungspfads.
//!
//! Usage: aufgabenprobe <artifact_dir> <promptdatei> [max_tokens]
//!
//! ⚑ **Eine Zeile je Prompt, `\n` als Zeilenumbruch geschrieben.** Die
//! Chatvorlage des Modells enthaelt Zeilenumbrueche und gilt
//! zeichengenau; so passt sie in eine Zeile, ohne dass jemand sie
//! umformuliert.
//!
//! ⚑ **Gierig und mit Halt an `<|im_end|>`**, damit eine Antwort dort
//! endet, wo das Modell sie beendet, und nicht erst an der Obergrenze.
//! Das Hauptprogramm bleibt unberuehrt: Es erzeugt ohne Halt, und
//! Messungen darauf bleiben Zeichen fuer Zeichen dieselben.

use integer_llm_runtime::generate::{generate_beobachtet, Erzeugung};
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    if let Err(e) = run() {
        eprintln!("[aufgabenprobe] Fehler: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("Usage: aufgabenprobe <artifact_dir> <promptdatei> [max_tokens]".into());
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let inhalt = std::fs::read_to_string(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?;
    let max_tokens: usize = match args.get(3) {
        Some(s) => s.parse().map_err(|_| format!("max_tokens: '{s}'"))?,
        None => 512,
    };
    let model = load_model(&dir).map_err(|e| format!("Modell-Ladung: {e}"))?;
    let pfad = dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(pfad.to_str().ok_or("Pfad ist kein UTF-8")?)?;

    let ende = tokenizer.encode("<|im_end|>");
    if ende.len() != 1 {
        return Err(format!("<|im_end|> ist kein einzelnes Token: {ende:?}"));
    }

    for (nr, zeile) in inhalt.lines().filter(|z| !z.trim().is_empty()).enumerate() {
        let prompt = zeile.replace("\\n", "\n");
        let lauf = Erzeugung {
            max_new_tokens: max_tokens,
            seed: 42,
            greedy: true,
            halt: &ende,
            denkgrenze: None,
            abbruch: None,
        };
        let anfang = std::time::Instant::now();
        let tokens = generate_beobachtet(&model, &tokenizer, &prompt, &lauf, &mut |_| {});
        let text = tokenizer.decode(&tokens);
        let zeile = serde_json::json!({
            "nr": nr,
            "token": tokens.len(),
            "sekunden": anfang.elapsed().as_secs_f64(),
            "antwort": text,
        });
        println!("{zeile}");
    }
    Ok(())
}
