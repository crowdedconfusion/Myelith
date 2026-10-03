//! Plattenprobe: Wie viel liest der Decode im warmen Zustand von der Platte?
//!
//! Usage: plattenprobe <artifact_dir> [aufwaermen] [messen]
//!
//! ⚑ **Die Frage hinter dem Vorhalten heisser Experten**: Passt ein
//! Gemisch nicht in den Arbeitsspeicher, holt der Decode Experten, die das
//! System verdraengt hatte, wieder von der SSD. Wie viel das ist, entscheidet,
//! ob es sich lohnt, heisse Experten zu schuetzen oder die der naechsten Ebene
//! vorauszuladen. Am 2026-10-02 kamen in einer Probe am 35B rund 42 GB Seiten
//! von der Platte, das erste Laden eingeschlossen; diese Probe trennt das ab.
//!
//! Erst `aufwaermen` Token (Vorgabe 64), dann `messen` Token (Vorgabe 128)
//! am selben Speicher, gierig, ohne Halt. Vor und nach dem gemessenen
//! Abschnitt wird die Systemzahl `Pageins` aus `vm_stat` gelesen.
//!
//! ⚠️ **Nur macOS, und die Zahl gilt fuer das ganze System**: Sie misst
//! nur auf einer ruhigen Maschine etwas. Eine Seite ist dort 16 KiB.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::generate::{dekodieren_fortgesetzt, Erzeugung, Fortsetzung};
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;

/// Groesse einer Seite auf Apple-Silizium.
const SEITE: u64 = 16 * 1024;

fn main() {
    if let Err(e) = run() {
        eprintln!("[plattenprobe] FEHLER: {e}");
        std::process::exit(1);
    }
}

/// Die Systemzahl der Seiten, die aus Dateien eingelesen wurden.
fn pageins() -> Result<u64, String> {
    let aus = std::process::Command::new("vm_stat").output().map_err(|e| format!("vm_stat: {e}"))?;
    let text = String::from_utf8_lossy(&aus.stdout);
    text.lines()
        .find(|z| z.starts_with("Pageins:"))
        .and_then(|z| z.split(':').nth(1))
        .map(|w| w.trim().trim_end_matches('.').to_string())
        .ok_or("vm_stat nennt keine Pageins (kein macOS?)")?
        .parse()
        .map_err(|e| format!("Pageins: {e}"))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err("Usage: plattenprobe <artifact_dir> [aufwaermen] [messen]".into());
    }
    let zahl = |i: usize, vorgabe: usize| -> Result<usize, String> {
        args.get(i).map(|s| s.parse().map_err(|_| format!("Argument {i}: '{s}'"))).transpose().map(|z| z.unwrap_or(vorgabe))
    };
    let dir = std::path::PathBuf::from(&args[1]);
    let (aufwaermen, messen) = (zahl(2, 64)?, zahl(3, 128)?);
    let model = load_model(&dir).map_err(|e| format!("Modell-Ladung: {e}"))?;
    let pfad = dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(pfad.to_str().ok_or("Pfad ist kein UTF-8")?)?;
    let prompt = tokenizer.encode(
        "<|im_start|>user\nSchreibe eine ausfuehrliche Anleitung, wie man ein Hochbeet aus Holz baut, mit \
         Materialliste und Arbeitsschritten.<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
    );
    let lauf = |n: usize| Erzeugung {
        max_new_tokens: n,
        seed: 42,
        greedy: true,
        halt: &[],
        denkgrenze: None,
        abbruch: None,
        ziehen: None,
    };
    let mut speicher = Fortsetzung::neu(&model);
    let (warm, _) = dekodieren_fortgesetzt(&model, &prompt, &lauf(aufwaermen), &mut speicher, &mut |_| {});
    let mut weiter = prompt.clone();
    weiter.extend(&warm);
    let vorher = pageins()?;
    let anfang = std::time::Instant::now();
    let (gemessen, _) = dekodieren_fortgesetzt(&model, &weiter, &lauf(messen), &mut speicher, &mut |_| {});
    let sekunden = anfang.elapsed().as_secs_f64();
    let nachher = pageins()?;
    let bytes = (nachher - vorher) * SEITE;
    let n = gemessen.len().max(1) as f64;
    println!(
        "[plattenprobe] {} | {} Token warm, {} gemessen | {:.2} Tok/s | {:.2} GB von der Platte, {:.1} MB je Token, {:.2} GB/s",
        dir.display(),
        warm.len(),
        gemessen.len(),
        n / sekunden,
        bytes as f64 / 1e9,
        bytes as f64 / 1e6 / n,
        bytes as f64 / 1e9 / sekunden
    );
    println!("[plattenprobe] Fertig");
    Ok(())
}
