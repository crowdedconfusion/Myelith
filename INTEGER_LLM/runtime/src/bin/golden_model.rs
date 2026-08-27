//! Golden-Vector-Validator fuer Layer- und E2E-Ebene.
//!
//! Laedt das echte Modell, fuehrt den Forward-Pass mit den Eingaben aus
//! dem Golden Vector aus und vergleicht das Ergebnis bitgenau.
//!
//! Einzeldatei:  golden_model <artifact_dir> <golden.json>
//! Batch-Modus:  golden_model <artifact_dir> --batch <vectors_dir>
//!
//! Duenner Starter ueber `integer_llm_runtime::konformitaet`, wo die
//! eigentliche Pruefung liegt. Sie war bis v0.21.0 in diesem Binary
//! gefangen und damit fuer andere Werkzeuge unerreichbar — der
//! Testclient haette fuer einen Konformitaetslauf ein zweites Programm
//! starten muessen, statt die Bibliothek zu benutzen.
//!
//! Im Batch-Modus wird das Modell einmal geladen und alle
//! *.golden.json-Dateien in <vectors_dir>/layer/ und <vectors_dir>/e2e/
//! validiert. Exit 0 wenn alle bestehen, sonst 1.

use integer_llm_runtime::konformitaet;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: golden_model <artifact_dir> <golden.json>");
        eprintln!("       golden_model <artifact_dir> --batch <vectors_dir>");
        std::process::exit(1);
    }
    let artifact_dir = PathBuf::from(&args[1]);

    let model = load_model(&artifact_dir).expect("Modell-Ladung fehlgeschlagen");

    if args.len() >= 4 && args[2] == "--batch" {
        let vectors_dir = PathBuf::from(&args[3]);
        run_batch(&model, &vectors_dir);
    } else {
        let gv_path = PathBuf::from(&args[2]);
        run_single(&model, &gv_path);
    }
}

fn run_single(model: &IntegerModel, gv_path: &Path) {
    let ergebnis = match konformitaet::vektor_aus_datei(model, gv_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    for grund in &ergebnis.gruende {
        eprintln!("  {}", grund);
    }
    if ergebnis.bestanden {
        println!("PASS: {}", ergebnis.name);
        std::process::exit(0);
    }
    println!("FAIL: {}", ergebnis.name);
    std::process::exit(1);
}

fn run_batch(model: &IntegerModel, vectors_dir: &Path) {
    let mut total = 0;
    let mut passed = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();

    for level in &["layer", "e2e"] {
        let level_dir = vectors_dir.join(level);
        if !level_dir.exists() {
            continue;
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&level_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .filter(|p| p.to_str().is_some_and(|s| s.ends_with(".golden.json")))
            .collect();
        files.sort();

        for gv_path in &files {
            let ergebnis = konformitaet::vektor_aus_datei(model, gv_path)
                .expect("Golden-Datei nicht lesbar");
            total += 1;
            for grund in &ergebnis.gruende {
                eprintln!("  {}", grund);
            }
            if ergebnis.bestanden {
                passed += 1;
                println!("  PASS: {}", ergebnis.name);
            } else {
                failed += 1;
                errors.push(ergebnis.name.clone());
                println!("  FAIL: {}", ergebnis.name);
            }
        }
    }

    println!("\n{} von {} bestanden ({} fehlgeschlagen)", passed, total, failed);
    if !errors.is_empty() {
        eprintln!("Fehlgeschlagen: {:?}", errors);
        std::process::exit(1);
    }
}
