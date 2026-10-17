//! Packt ein Artefakt mit Vierbitgewichten im int8-Behaelter in die
//! Vierbitform.
//!
//! Usage: vierbit_packen <quelle> <ziel> [--faeden n]
//!
//! Jeder int8-Tensor, den der Lader in vier Bit zulaesst (die gerouteten
//! Experten und die Projektionen dichter Ebenen) und dessen Gruppen von 128
//! **alle** vierbittig sind (`a * q + b` mit ganzen `a`, `b` und `q` von 0
//! bis 15), wird gepackt: vier Bit je Gewicht und zwei Byte je Gruppe.
//! Alles andere bleibt, wie es ist.
//!
//! ⚑ **Kein Runden, kein Naehern.** Eine Gruppe, die nicht vierbittig ist,
//! laesst ihren Tensor ungepackt, und das Werkzeug zaehlt ihn. Das gepackte
//! Artefakt rechnet deshalb jede Zeile als dieselbe ganze Zahl wie die
//! Quelle, und alles, was an der Quelle gemessen wurde, gilt fuer das Ziel.
//! Belegt wird das durch einen Vergleich der Logits beider Artefakte
//! (`artefaktvergleich.py … --bitgleich`).
//!
//! ⚑ **Unveraenderte Dateien werden hart verlinkt**, wie beim ternaeren
//! Packer; die Pruefmarke der Quelle nie (sie gilt nur fuer ihr Artefakt).
//! Neu geschrieben werden `weights_manifest.json` und `theta_v.json`, denn
//! der `weights_hash` ist die Pruefsumme des Manifests.

use integer_llm_kernels::vierbit::packen;
use integer_llm_runtime::loader::{sha256_hex, vierbit_zulaessig, VIERBIT_DTYPE};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

fn main() {
    if let Err(e) = run() {
        eprintln!("[vierbit_packen] FEHLER: {e}");
        std::process::exit(1);
    }
}

/// Was ein Tensor nach dem Packen ins Manifest traegt.
struct Ergebnis {
    name: String,
    alt: String,
    codes: String,
    codes_hash: String,
    gitter: String,
    gitter_hash: String,
    vorher: u64,
    nachher: u64,
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut faeden = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut pos = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--faeden" {
            faeden = args.get(i + 1).and_then(|v| v.parse().ok()).filter(|&n| n > 0).ok_or("--faeden braucht eine Zahl")?;
            i += 2;
        } else {
            pos.push(args[i].clone());
            i += 1;
        }
    }
    if pos.len() != 2 {
        return Err("Usage: vierbit_packen <quelle> <ziel> [--faeden n]".into());
    }
    let quelle = Path::new(&pos[0]);
    let ziel = Path::new(&pos[1]);
    if ziel.exists() {
        return Err(format!("{} gibt es schon; das Werkzeug ueberschreibt nichts", ziel.display()));
    }
    let lesen = |name: &str| -> Result<Value, String> {
        let text = std::fs::read_to_string(quelle.join(name)).map_err(|e| format!("{name}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))
    };
    let mut manifest = lesen("weights_manifest.json")?;
    let mut theta_v = lesen("theta_v.json")?;
    let eintraege = manifest.as_object_mut().ok_or("weights_manifest.json ist kein Objekt")?;

    // Die Kandidaten: zulaessig, int8, eine Matrix.
    let mut kandidaten: Vec<(String, String, usize, usize)> = Vec::new();
    for (name, e) in eintraege.iter() {
        let form: Vec<usize> = e["shape"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|v| v as usize)).collect())
            .unwrap_or_default();
        if vierbit_zulaessig(name) && e["dtype"] == "int8" && form.len() == 2 {
            let datei = e["file"].as_str().ok_or_else(|| format!("{name}: ohne file"))?.to_string();
            kandidaten.push((name.clone(), datei, form[0], form[1]));
        }
    }
    kandidaten.sort();
    std::fs::create_dir_all(ziel).map_err(|e| format!("{}: {e}", ziel.display()))?;

    // Je Faden ein Tensor nach dem anderen; geschrieben wird nur ins Ziel.
    let naechster = AtomicUsize::new(0);
    let fertig: Mutex<Vec<Ergebnis>> = Mutex::new(Vec::new());
    let ungepackt: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let fehler: Mutex<Option<String>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..faeden {
            s.spawn(|| loop {
                let k = naechster.fetch_add(1, Ordering::Relaxed);
                let Some((name, datei, zeilen, spalten)) = kandidaten.get(k) else { break };
                if fehler.lock().expect("kein Faden bricht mit dem Riegel ab").is_some() {
                    break;
                }
                match tensor_packen(quelle, ziel, datei, *zeilen, *spalten) {
                    Ok(Some((codes, codes_hash, gitter, gitter_hash, vorher, nachher))) => {
                        fertig.lock().expect("Riegel").push(Ergebnis {
                            name: name.clone(),
                            alt: datei.clone(),
                            codes,
                            codes_hash,
                            gitter,
                            gitter_hash,
                            vorher,
                            nachher,
                        });
                    }
                    Ok(None) => ungepackt.lock().expect("Riegel").push(name.clone()),
                    Err(e) => *fehler.lock().expect("Riegel") = Some(format!("{name}: {e}")),
                }
                if k % 2048 == 0 {
                    println!("[vierbit_packen] {k} von {} Tensoren", kandidaten.len());
                }
            });
        }
    });
    if let Some(e) = fehler.into_inner().expect("Riegel") {
        return Err(e);
    }
    let fertig = fertig.into_inner().expect("Riegel");
    let mut ungepackt = ungepackt.into_inner().expect("Riegel");
    ungepackt.sort();

    let (mut vorher, mut nachher) = (0u64, 0u64);
    let mut ersetzt: Vec<String> = Vec::new();
    for r in &fertig {
        let e = eintraege.get_mut(&r.name).expect("eben gelesen");
        e["dtype"] = json!(VIERBIT_DTYPE);
        e["file"] = json!(r.codes);
        e["hash"] = json!(r.codes_hash);
        e["gitter_file"] = json!(r.gitter);
        e["gitter_hash"] = json!(r.gitter_hash);
        vorher += r.vorher;
        nachher += r.nachher;
        ersetzt.push(r.alt.clone());
    }
    ersetzt.sort();

    // Alles uebrige aus der Quelle, ausser den ersetzten Gewichten, den
    // beiden Dateien, die neu entstehen, und der Pruefmarke (Fund 551).
    let mut verlinkt = 0usize;
    for d in std::fs::read_dir(quelle).map_err(|e| format!("{}: {e}", quelle.display()))? {
        let d = d.map_err(|e| e.to_string())?;
        let n = d.file_name().to_string_lossy().to_string();
        if ersetzt.binary_search(&n).is_ok()
            || n == "weights_manifest.json"
            || n == "theta_v.json"
            || n.starts_with(integer_llm_runtime::pruefstand::MARKENNAME)
            || !d.path().is_file()
        {
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

    for n in ungepackt.iter().take(5) {
        println!("[vierbit_packen] bleibt int8: {n}");
    }
    println!(
        "[vierbit_packen] {} Tensoren gepackt, {} bleiben int8, {} Dateien uebernommen; gepackte Gewichte {:.2} GB statt {:.2} GB",
        fertig.len(),
        ungepackt.len(),
        verlinkt,
        nachher as f64 / 1e9,
        vorher as f64 / 1e9
    );
    println!("Fertig");
    Ok(())
}

/// Codes, Gitter, deren Pruefsummen und die Groessen vorher und nachher.
type Gepackt = (String, String, String, String, u64, u64);

/// Liest einen int8-Tensor und packt ihn ins Ziel; `None` heisst: nicht
/// vierbittig, er bleibt int8.
fn tensor_packen(quelle: &Path, ziel: &Path, datei: &str, zeilen: usize, spalten: usize) -> Result<Option<Gepackt>, String> {
    let bytes = std::fs::read(quelle.join(datei)).map_err(|e| format!("{datei}: {e}"))?;
    if bytes.len() != zeilen * spalten {
        return Err(format!("{datei}: {} Bytes, erwartet {}", bytes.len(), zeilen * spalten));
    }
    let werte: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    let Ok(p) = packen(&werte, spalten) else { return Ok(None) };
    let stamm = datei.trim_end_matches(".bin");
    let (cdatei, gdatei) = (format!("{stamm}.codes.bin"), format!("{stamm}.gitter.bin"));
    schreiben(&ziel.join(&cdatei), &p.codes)?;
    schreiben(&ziel.join(&gdatei), &p.gitter)?;
    Ok(Some((
        cdatei,
        sha256_hex(&p.codes),
        gdatei,
        sha256_hex(&p.gitter),
        bytes.len() as u64,
        (p.codes.len() + p.gitter.len()) as u64,
    )))
}

fn schreiben(pfad: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(pfad, bytes).map_err(|e| format!("{}: {e}", pfad.display()))
}
