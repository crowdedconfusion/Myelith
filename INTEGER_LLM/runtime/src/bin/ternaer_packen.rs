//! Packt ein Artefakt mit ternaeren Gewichten in die ternaere Speicherform.
//!
//! Usage: ternaer_packen <quelle> <ziel>
//!
//! Jeder Tensor, den der Lader ternaer zulaesst (Projektionen dichter
//! Ebenen, beide Koepfe) und dessen Gruppen von 128 **alle** ternaer sind
//! ({-m, 0, +m} mit einem ganzen `m`), wird gepackt: zwei Bit je Gewicht
//! und ein `i16`-Betrag je Gruppe. Alles andere bleibt, wie es ist.
//!
//! ⚑ **Kein Runden, kein Naehern.** Eine Gruppe, die nicht ternaer ist,
//! laesst ihren Tensor ungepackt, und das Werkzeug sagt es. Das gepackte
//! Artefakt rechnet deshalb jede Zeile als dieselbe ganze Zahl wie die
//! Quelle, und alles, was an der Quelle gemessen wurde, gilt fuer das Ziel.
//! Belegt wird das nicht hier, sondern durch einen Vergleich der Logits
//! beider Artefakte.
//!
//! ⚑ **Unveraenderte Dateien werden hart verlinkt**, nicht kopiert: Die
//! Einbettung, die Normen, die Tabellen und die Shifts sind dieselben
//! Bytes, und ein zweites Exemplar kostete nur Platz. Wo das Dateisystem
//! keinen harten Link erlaubt, wird kopiert.
//!
//! Neu geschrieben werden `weights_manifest.json` und `theta_v.json`,
//! denn der `weights_hash` ist die Pruefsumme des Manifests.

use integer_llm_kernels::ternaer::packen;
use integer_llm_runtime::loader::{sha256_hex, ternaer_zulaessig, TERNAER_DTYPE};
use serde_json::{json, Value};
use std::path::Path;

/// Zeilen je Durchgang beim Packen. Ein int16-Kopf eines 8B hat 151 669
/// Zeilen zu 4 096; am Stueck waeren das 1,2 GB im Heap.
const ZEILEN_JE_DURCHGANG: usize = 2048;

fn main() {
    if let Err(e) = run() {
        eprintln!("[ternaer_packen] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: ternaer_packen <quelle> <ziel>".into());
    }
    let quelle = Path::new(&args[1]);
    let ziel = Path::new(&args[2]);
    if ziel.exists() {
        return Err(format!("{} gibt es schon; das Werkzeug ueberschreibt nichts", ziel.display()));
    }
    let lesen = |name: &str| -> Result<Value, String> {
        let text = std::fs::read_to_string(quelle.join(name)).map_err(|e| format!("{name}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))
    };
    let mut manifest = lesen("weights_manifest.json")?;
    let mut theta_v = lesen("theta_v.json")?;
    std::fs::create_dir_all(ziel).map_err(|e| format!("{}: {e}", ziel.display()))?;

    let eintraege = manifest
        .as_object_mut()
        .ok_or("weights_manifest.json ist kein Objekt")?;
    let mut ersetzt: Vec<String> = Vec::new();
    let (mut vorher, mut nachher) = (0u64, 0u64);
    let mut namen: Vec<String> = eintraege.keys().cloned().collect();
    namen.sort();
    for name in &namen {
        let e = eintraege.get_mut(name).expect("eben gelesen");
        let dtype = e["dtype"].as_str().unwrap_or("").to_string();
        let form: Vec<usize> = e["shape"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|v| v as usize)).collect())
            .unwrap_or_default();
        if !ternaer_zulaessig(name) || form.len() != 2 || !(dtype == "int8" || dtype == "int16") {
            continue;
        }
        let datei = e["file"].as_str().ok_or_else(|| format!("{name}: ohne file"))?.to_string();
        let (zeilen, spalten) = (form[0], form[1]);
        match tensor_packen(&quelle.join(&datei), &dtype, zeilen, spalten) {
            Ok((muster, betraege)) => {
                let stamm = datei.trim_end_matches(".bin");
                let (mdatei, bdatei) = (format!("{stamm}.muster.bin"), format!("{stamm}.betraege.bin"));
                schreiben(&ziel.join(&mdatei), &muster)?;
                schreiben(&ziel.join(&bdatei), &betraege)?;
                vorher += std::fs::metadata(quelle.join(&datei)).map(|m| m.len()).unwrap_or(0);
                nachher += (muster.len() + betraege.len()) as u64;
                e["dtype"] = json!(TERNAER_DTYPE);
                e["file"] = json!(mdatei);
                e["hash"] = json!(sha256_hex(&muster));
                e["betraege_file"] = json!(bdatei);
                e["betraege_hash"] = json!(sha256_hex(&betraege));
                ersetzt.push(datei);
                println!("[ternaer_packen] gepackt: {name} ({zeilen} x {spalten}, {dtype})");
            }
            Err(grund) => println!("[ternaer_packen] bleibt {dtype}: {name}: {grund}"),
        }
    }

    // Alles uebrige aus der Quelle, ausser den ersetzten Gewichten und
    // den beiden Dateien, die neu entstehen.
    let mut verlinkt = 0usize;
    for d in std::fs::read_dir(quelle).map_err(|e| format!("{}: {e}", quelle.display()))? {
        let d = d.map_err(|e| e.to_string())?;
        let n = d.file_name().to_string_lossy().to_string();
        if ersetzt.contains(&n) || n == "weights_manifest.json" || n == "theta_v.json" || !d.path().is_file() {
            continue;
        }
        if std::fs::hard_link(d.path(), ziel.join(&n)).is_err() {
            std::fs::copy(d.path(), ziel.join(&n)).map_err(|e| format!("{n}: {e}"))?;
        }
        verlinkt += 1;
    }

    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    schreiben(&ziel.join("weights_manifest.json"), &manifest_bytes)?;
    theta_v["weights_hash"] = json!(sha256_hex(&manifest_bytes));
    schreiben(&ziel.join("theta_v.json"), &serde_json::to_vec(&theta_v).map_err(|e| e.to_string())?)?;

    println!(
        "[ternaer_packen] {} Tensoren gepackt, {} Dateien uebernommen; gepackte Gewichte {:.2} GB statt {:.2} GB",
        ersetzt.len(),
        verlinkt,
        nachher as f64 / 1e9,
        vorher as f64 / 1e9
    );
    println!("Fertig");
    Ok(())
}

/// Liest einen int8- oder int16-Tensor und packt ihn; `Err` heisst: nicht
/// ternaer, mit dem ersten Grund.
fn tensor_packen(pfad: &Path, dtype: &str, zeilen: usize, spalten: usize) -> Result<(Vec<u8>, Vec<u8>), String> {
    let f = std::fs::File::open(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    // SICHERHEIT: nur lesend, und die Quelle ist ein Artefakt, das waehrend
    // des Packens niemand schreibt.
    let abbild = unsafe { memmap2::Mmap::map(&f) }.map_err(|e| format!("{}: {e}", pfad.display()))?;
    let breite = if dtype == "int16" { 2 } else { 1 };
    if abbild.len() != zeilen * spalten * breite {
        return Err(format!("{} Bytes, erwartet {}", abbild.len(), zeilen * spalten * breite));
    }
    let mut muster = Vec::new();
    let mut betraege = Vec::new();
    for anfang in (0..zeilen).step_by(ZEILEN_JE_DURCHGANG) {
        let ende = (anfang + ZEILEN_JE_DURCHGANG).min(zeilen);
        let bytes = &abbild[anfang * spalten * breite..ende * spalten * breite];
        let gepackt = if breite == 2 {
            let werte: Vec<i16> = bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
            packen(&werte, spalten)
        } else {
            let werte: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
            packen(&werte, spalten)
        }
        .map_err(|e| format!("ab Zeile {anfang}: {e}"))?;
        muster.extend_from_slice(&gepackt.muster);
        betraege.extend(gepackt.betraege.iter().flat_map(|b| b.to_le_bytes()));
    }
    Ok((muster, betraege))
}

fn schreiben(pfad: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(pfad, bytes).map_err(|e| format!("{}: {e}", pfad.display()))
}
