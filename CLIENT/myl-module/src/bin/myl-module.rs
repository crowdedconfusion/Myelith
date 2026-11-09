//! **`myl-module`**: Schluessel erzeugen, Module signieren und pruefen.
//!
//! ```text
//! myl-module schluessel <datei>              erzeugt ein Schluesselpaar; geheim in <datei>, oeffentlich auf stdout
//! myl-module signieren <ordner> <schluessel> traegt die Pruefsummen ein und signiert
//! myl-module pruefen <ordner> <vertrauen.json>  prueft wie die Konsole
//! myl-module geheimnis <ablage> <modul> <name>  legt ein Geheimnis ab (von stdin)
//! ```
//!
//! ⛔️ Der geheime Schluessel gehoert nicht ins Repositorium und nicht in
//! einen Modulordner. Die Datei wird nur fuer den Besitzer lesbar
//! angelegt.

use myl_module::signatur;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let ergebnis = match a.as_slice() {
        ["schluessel", datei] => schluessel(Path::new(datei)),
        ["signieren", ordner, schluessel] => signieren(Path::new(ordner), Path::new(schluessel)),
        ["pruefen", ordner, vertrauen] => pruefen(Path::new(ordner), Path::new(vertrauen)),
        ["geheimnis", ablage, modul, name] => geheimnis(Path::new(ablage), modul, name),
        _ => Err("myl-module schluessel <datei> | signieren <ordner> <schluessel> | pruefen <ordner> <vertrauen.json> | geheimnis <ablage> <modul> <name>".to_string()),
    };
    if let Err(f) = ergebnis {
        eprintln!("FEHLER: {f}");
        std::process::exit(2);
    }
}

/// Liest das Geheimnis von der Standardeingabe, damit es nicht in der
/// Prozessliste steht; ein Zeilenende am Schluss gehoert nicht dazu.
fn geheimnis(ablage: &Path, modul: &str, name: &str) -> Result<(), String> {
    let mut wert = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut wert).map_err(|f| f.to_string())?;
    while matches!(wert.last(), Some(b'\n' | b'\r')) {
        wert.pop();
    }
    myl_module::ablage::geheimnis_setzen(ablage, modul, name, &wert)?;
    println!("abgelegt: {modul}/{name}");
    Ok(())
}

fn schluessel(datei: &Path) -> Result<(), String> {
    if datei.exists() {
        return Err(format!("{} gibt es schon; ein Schluessel wird nie ueberschrieben", datei.display()));
    }
    let (geheim, oeffentlich) = signatur::schluessel_erzeugen()?;
    signatur::schreiben_atomar(datei, format!("{geheim}\n").as_bytes(), true)?;
    println!("{oeffentlich}");
    Ok(())
}

fn signieren(ordner: &Path, schluessel: &Path) -> Result<(), String> {
    let geheim = std::fs::read_to_string(schluessel).map_err(|f| format!("{}: {f}", schluessel.display()))?;
    let b = signatur::signieren(ordner, geheim.trim())?;
    println!("signiert: {} {} ({} Dateien) mit {}", b.name, b.version, b.dateien.len(), signatur::oeffentlich_zu(geheim.trim())?);
    Ok(())
}

fn pruefen(ordner: &Path, vertrauen: &Path) -> Result<(), String> {
    let v = signatur::Vertrauensliste::lesen(vertrauen)?;
    let g = signatur::pruefen(ordner, &v, &signatur::Versionsstand::default())?;
    println!("gut: {} {}, signiert von {}, Programm {}", g.beschreibung.name, g.beschreibung.version, g.signiert_von, &g.programm_hash[..16]);
    Ok(())
}
