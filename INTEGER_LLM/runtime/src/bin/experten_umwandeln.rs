//! **Die gerouteten Experten eines Gemischs ternaer runden**, im int8-Format.
//!
//! Usage: experten_umwandeln <quelle> <ziel> [--ohne E,E,...] [--nur VON-BIS] [--aus VERZEICHNIS]
//!
//! Ohne Angabe werden die Experten **aller** Gemischebenen umgewandelt;
//! `--ohne` laesst einzelne Ebenen int8, `--nur` beschraenkt auf einen
//! Bereich (einschliesslich beider Enden). Router, geteilter Experte,
//! Mischer, Normen und Kopf bleiben, wie sie sind.
//!
//! Mit `--aus` kommen die ternaeren Experten aus einem Verzeichnis (etwa der
//! datenabhaengigen Rundung), sonst werden sie hier gerundet, mit der
//! Ableitung des Trainings (siehe
//! `integer_llm_runtime::expertenumwandlung`). Das Ziel rechnet mit dem
//! bestehenden int8-Weg; gepackt wird es danach mit `ternaer_packen`.

use integer_llm_runtime::expertenumwandlung::artefakt_umwandeln;
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("[experten_umwandeln] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("Usage: experten_umwandeln <quelle> <ziel> [--ohne E,E,...] [--nur VON-BIS] [--aus VERZEICHNIS]".into());
    }
    let mut ohne: Vec<usize> = Vec::new();
    let mut nur: Option<(usize, usize)> = None;
    let mut aus: Option<std::path::PathBuf> = None;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--ohne" => {
                i += 1;
                let liste = args.get(i).ok_or("--ohne braucht eine Liste")?;
                for e in liste.split(',') {
                    ohne.push(e.trim().parse().map_err(|_| format!("--ohne: {e} ist keine Ebene"))?);
                }
            }
            "--nur" => {
                i += 1;
                let b = args.get(i).ok_or("--nur braucht VON-BIS")?;
                let (v, z) = b.split_once('-').ok_or("--nur braucht VON-BIS")?;
                nur = Some((
                    v.parse().map_err(|_| format!("--nur: {v} ist keine Ebene"))?,
                    z.parse().map_err(|_| format!("--nur: {z} ist keine Ebene"))?,
                ));
            }
            "--aus" => {
                i += 1;
                aus = Some(args.get(i).ok_or("--aus braucht ein Verzeichnis")?.into());
            }
            a => return Err(format!("unbekannter Schalter {a}")),
        }
        i += 1;
    }
    let waehlen = |e: usize| !ohne.contains(&e) && nur.is_none_or(|(v, z)| (v..=z).contains(&e));
    let start = std::time::Instant::now();
    let b = artefakt_umwandeln(Path::new(&args[1]), Path::new(&args[2]), &waehlen, aus.as_deref(), &|s| {
        println!("[experten_umwandeln] {s}")
    })?;
    println!(
        "[experten_umwandeln] {} Matrizen in {} Ebenen ternaer, {:.1} % der Gewichte null, {:.0} s",
        b.matrizen,
        b.ebenen.len(),
        100.0 * b.nullen as f64 / b.gewichte.max(1) as f64,
        start.elapsed().as_secs_f64()
    );
    println!("[experten_umwandeln] Ebenen: {:?}", b.ebenen);
    println!("Fertig");
    Ok(())
}
