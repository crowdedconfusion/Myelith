//! **Die Kerne ohne Karte pruefen**: `kerne.cu` als C++ auf der CPU
//! (`nachbau.hpp`, `nachbau_haupt.cpp`), gegen die CPU-Kerne, mit den
//! Faellen der Selbstpruefung.
//!
//! # ⚑ Warum das zaehlt
//!
//! Auf der Entwicklungsmaschine gibt es keine Karte von NVIDIA oder AMD.
//! Ohne diesen Nachbau waere die erste Pruefung der Kerne die
//! Selbstpruefung auf einer gemieteten Maschine, und jeder Fehler darin
//! kostete Mietzeit. So ist die Logik der Kerne (Indizes, Kacheln,
//! Entpacken, Runden, Klemmen, gemeinsamer Speicher) vorher belegt; offen
//! bleibt nur, was der Uebersetzer der Karte anders macht.
//!
//! # ⚠️ Braucht einen C++-Uebersetzer mit C++20
//!
//! `CXX`, sonst `c++`, `clang++`, `g++`. **Fehlt er, scheitert die
//! Pruefung laut**, statt still zu bestehen: Ein Bau mit `cuda` oder `rocm`
//! hat keinen anderen Beleg fuer seine Kerne.

use super::pruefung::{abweichung, faelle, Daten};
use std::path::{Path, PathBuf};

/// Wo die Quellen liegen, gegen die Kiste aufgeloest.
const ORDNER: &str = "src/ressourcen/karten";

fn uebersetzer() -> Option<String> {
    let mut kandidaten: Vec<String> = std::env::var("CXX").ok().into_iter().collect();
    kandidaten.extend(["c++", "clang++", "g++"].map(String::from));
    kandidaten.into_iter().find(|c| {
        std::process::Command::new(c).arg("--version").output().map(|a| a.status.success()).unwrap_or(false)
    })
}

fn bauen(ziel: &Path) -> Result<(), String> {
    let cxx = uebersetzer().ok_or("kein C++-Uebersetzer gefunden (CXX, c++, clang++, g++)")?;
    let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDNER);
    let a = std::process::Command::new(&cxx)
        .args(["-std=c++20", "-O2", "-pthread", "-Wall", "-Wextra", "-Werror"])
        .arg(ordner.join("nachbau_haupt.cpp"))
        .arg("-o")
        .arg(ziel)
        .output()
        .map_err(|e| format!("{cxx}: {e}"))?;
    if !a.status.success() {
        return Err(format!("{cxx} scheitert:\n{}", String::from_utf8_lossy(&a.stderr)));
    }
    Ok(())
}

fn arbeitsordner() -> PathBuf {
    let d = std::env::temp_dir().join(format!("myl-nachbau-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("Arbeitsordner");
    d
}

/// ⛔️ **Jeder Fall der Selbstpruefung, Bit fuer Bit wie die CPU.**
#[test]
fn die_kerne_rechnen_wie_die_cpu() {
    let d = arbeitsordner();
    let programm = d.join("nachbau");
    if let Err(grund) = bauen(&programm) {
        panic!("Der Nachbau der Kerne laesst sich nicht bauen: {grund}");
    }
    let faelle = faelle();
    let mut eingabe: Vec<u8> = Vec::new();
    for f in &faelle {
        let art = match f.daten {
            Daten::Int8(_) => 0i32,
            Daten::Ternaer { .. } => 1,
            Daten::Vierbit { .. } => 2,
        };
        for n in [art, f.zeilen as i32, f.spalten as i32, f.eingaben.len() as i32] {
            eingabe.extend_from_slice(&n.to_le_bytes());
        }
        match &f.daten {
            Daten::Int8(w) => eingabe.extend(w.iter().map(|&v| v as u8)),
            Daten::Ternaer { muster, betraege } => {
                eingabe.extend_from_slice(muster);
                eingabe.extend(betraege.iter().flat_map(|v| v.to_le_bytes()));
            }
            Daten::Vierbit { codes, gitter } => {
                eingabe.extend_from_slice(codes);
                eingabe.extend_from_slice(gitter);
            }
        }
        eingabe.extend(f.eingaben.iter().flatten().flat_map(|v| v.to_le_bytes()));
        eingabe.extend(f.abstaende().iter().map(|&v| v as u8));
    }
    let (ein, aus) = (d.join("faelle.bin"), d.join("ergebnis.bin"));
    std::fs::write(&ein, &eingabe).expect("Eingabe schreiben");
    let lauf = std::process::Command::new(&programm).arg(&ein).arg(&aus).output().expect("Nachbau starten");
    assert!(lauf.status.success(), "Nachbau scheitert: {}", String::from_utf8_lossy(&lauf.stderr));
    let bytes = std::fs::read(&aus).expect("Ergebnis lesen");
    let werte: Vec<i16> = bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    let mut stelle = 0;
    for f in &faelle {
        let n = f.zeilen * f.eingaben.len();
        let nachbau: Vec<Vec<i16>> = werte[stelle..stelle + n].chunks_exact(f.zeilen).map(<[i16]>::to_vec).collect();
        stelle += n;
        let cpu = f.erwartet().expect("CPU-Erwartung");
        if let Some(a) = abweichung(&f.name, &nachbau, &cpu) {
            panic!("Nachbau weicht ab: {a}");
        }
    }
    assert_eq!(stelle, werte.len(), "der Nachbau hat mehr ausgegeben als erwartet");
    let _ = std::fs::remove_dir_all(&d);
}
