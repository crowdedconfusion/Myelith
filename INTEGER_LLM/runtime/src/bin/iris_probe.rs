//! **Der Sichtturm allein**, ueber die Flicken eines Bildes, jede Stufe
//! abgelegt.
//!
//! ```text
//! iris_probe <artefakt> <flicken.bin> <gh> <gw> <ausgabeordner>
//! ```
//!
//! `flicken.bin` traegt `gh·gw·768` Werte `2p - 255` als i16 (little
//! endian), in der Folge des Turms. Je Stufe entstehen `<stufe>.bin` (i16,
//! Flicken mal Kanaele) und `<stufe>_frac.bin` (eine Skala je Kanal).
//! Verglichen wird in `calibrate/src/iris_vergleich.py` gegen die
//! Gleitkomma-Referenz; dieses Programm rechnet nur ganzzahlig.

use std::path::PathBuf;

use integer_llm_runtime::iris::{Decoderanteil, Iris};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 {
        eprintln!("Aufruf: iris_probe <artefakt> <flicken.bin> <gh> <gw> <ausgabeordner>");
        std::process::exit(2);
    }
    let artefakt = PathBuf::from(&a[1]);
    let gh: usize = a[3].parse().expect("gh");
    let gw: usize = a[4].parse().expect("gw");
    let aus = PathBuf::from(&a[5]);
    std::fs::create_dir_all(&aus).expect("Ausgabeordner");

    let roh = std::fs::read(&a[2]).expect("flicken.bin");
    let flicken: Vec<i16> = roh.as_chunks::<2>().0.iter().map(|&b| i16::from_le_bytes(b)).collect();

    let decoder = Decoderanteil::aus_artefakt(&artefakt).unwrap_or_else(|e| panic!("{e}"));
    let turm = Iris::laden(&artefakt, decoder).unwrap_or_else(|e| panic!("{e}"));
    let beginn = std::time::Instant::now();
    let mut ablegen = |name: &str, werte: &[Vec<i16>], frac: &[u8]| {
        let mut b = Vec::with_capacity(werte.len() * frac.len() * 2);
        for zeile in werte {
            for v in zeile {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(aus.join(format!("{name}.bin")), b).expect("schreiben");
        std::fs::write(aus.join(format!("{name}_frac.bin")), frac).expect("schreiben");
    };
    let token = turm.vorwaerts_mit(&flicken, gh, gw, &mut ablegen);
    println!(
        "{} Flicken -> {} Bildtoken zu {} in {} ms",
        gh * gw,
        token.len(),
        token.first().map_or(0, |t| t.len()),
        beginn.elapsed().as_millis()
    );
}
