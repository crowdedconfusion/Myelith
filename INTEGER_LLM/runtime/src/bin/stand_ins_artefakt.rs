//! Schreibt aus einem Artefakt und einem trainierten Gewichtsstand ein
//! neues Artefakt.
//!
//! Usage: stand_ins_artefakt <quelle> <stand> <ziel> [--ternaer]
//!
//! `<stand>` ist die Datei, die `trainingsguete --stand-schreiben`
//! hinterlaesst. Der Ebenenbereich steht in ihr.
//!
//! `--ternaer` gehoert dazu, wenn der Stand aus einer ternaeren
//! **Umwandlung** eines int8-Artefakts stammt (`trainingsguete
//! --ternaer`): Die Quelle sagt dann int8, der Stand ist ternaer gemeint.
//! Ein ternaeres Artefakt als Quelle schreibt ohne den Schalter ternaer.
//!
//! ⚑ Eine ternaere Ausgabe ist **gepackt** (zwei Bit je Gewicht, ein
//! Betrag je Gruppe von 128); ein zweiter Schritt ist nicht noetig.
//!
//! Der Ablesekopf steht nicht im Stand und kommt unveraendert aus der
//! Quelle. Die Umrechnung und ihre Grenzen stehen in
//! `integer_llm_runtime::standartefakt`.

use integer_llm_kernels::trainingsschritt::Gewichtsform;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::shardtraining::{stand_lesen, Shardgewichte};
use integer_llm_runtime::standartefakt::{bereich_des_standes, stand_ins_artefakt};
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("[stand_ins_artefakt] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ternaer = args.iter().any(|a| a == "--ternaer");
    let pfade: Vec<&String> = args.iter().filter(|a| *a != "--ternaer").collect();
    if pfade.len() != 3 || pfade.iter().any(|p| p.starts_with("--")) {
        return Err("Usage: stand_ins_artefakt <quelle> <stand> <ziel> [--ternaer]".into());
    }
    let (quelle, stand, ziel) = (Path::new(pfade[0]), Path::new(pfade[1]), Path::new(pfade[2]));
    if ziel.exists() {
        return Err(format!("{} gibt es schon; das Werkzeug ueberschreibt nichts", ziel.display()));
    }
    let (von, bis) = bereich_des_standes(stand)?;
    let m = load_model(quelle)?;
    let mut g = Shardgewichte::aus_modell(&m, von, bis).map_err(|e| format!("{e:?}"))?;
    if ternaer {
        g.form_setzen(Gewichtsform::Ternaer).map_err(|e| format!("{e:?}"))?;
    }
    stand_lesen(&mut g, stand)?;
    let b = stand_ins_artefakt(quelle, &m, &g, ziel)?;
    println!(
        "[stand_ins_artefakt] Ebenen {von} bis {bis}, {} Matrizen als {:?}, {:.3} GB Gewichte, {} Dateien uebernommen",
        b.matrizen,
        b.form,
        b.gewichtsbytes as f64 / 1e9,
        b.uebernommen
    );
    println!("Fertig");
    Ok(())
}
