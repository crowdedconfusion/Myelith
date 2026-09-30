//! Sonde: eine Matrix in der Groesse eines 8B, int8 gegen ternaer
//! gepackt, einkernig und verteilt.
//!
//! # ⚑ Die Frage
//!
//! Ein ternaer gepacktes 8B liest je Token ein Viertel der Bytes des
//! int8-Artefakts und war trotzdem nicht schneller (2026-09-28: 16,9
//! gegen 16,5 Token/s). Dann begrenzt nicht das Lesen, sondern etwas
//! anderes. Diese Sonde trennt die Matrix vom Rest des Modells: **Wie
//! viele Gewichte je Sekunde schafft jeder Kern, einkernig und auf allen
//! Kernen, und wie viele Bytes liest er dabei?**
//!
//! ⚠️ **Alles im Speicher**, und die Matrizen sind groesser als jeder
//! Zwischenspeicher: gelesen wird aus dem Hauptspeicher wie im Decode.
//! Beide Wege rechnen dieselbe Zahl; geprueft wird das hier mit.
//!
//! Aufruf: `ternaerprobe [wiederholungen] [zeilen] [spalten] [kerne,kerne,...]`
//!
//! Ohne Angaben die Matrix eines 8B auf einem Kern und auf allen. Mit
//! einer Kernliste (`1,5,10,0`; 0 heisst alle) zeigt die Sonde, wie der
//! Durchsatz mit der Fadenzahl waechst, also was die Verteilung kostet.

use std::time::Instant;

use integer_llm_kernels::linear::{kerngrenze_setzen, linear_matrix, linear_w8a16, Gewichtsmatrix};
use integer_llm_kernels::ternaer::{packen, Ternaermatrix, GRUPPE};

/// Vorgabe: `gate_proj` eines Qwen3-8B, 12 288 Zeilen zu 4 096.
const ZEILEN: usize = 12_288;
const SPALTEN: usize = 4_096;
/// Mehrere Matrizen im Wechsel, damit keine im Zwischenspeicher bleibt.
const MATRIZEN: usize = 4;
/// Eingaben im Stapellauf.
const STAPEL: usize = 64;

fn main() {
    let wdh: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let zeilen: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(ZEILEN);
    let spalten: usize = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(SPALTEN);
    let kernliste: Vec<usize> = std::env::args()
        .nth(4)
        .map(|s| s.split(',').filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_else(|| vec![1, 0]);
    let mut x = 0x2545_F491_4F6C_DD1D_u64;
    let mut zufall = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut y = 0x9E37_79B9_7F4A_7C15_u64;
    let mut zufall2 = move || {
        y ^= y << 13;
        y ^= y >> 7;
        y ^= y << 17;
        y
    };
    let int8: Vec<Vec<i8>> = (0..MATRIZEN)
        .map(|_| {
            let mut betrag = 1i8;
            (0..zeilen * spalten)
                .map(|i| {
                    let r = zufall();
                    if i % GRUPPE == 0 {
                        betrag = 23 + (r % 100) as i8;
                    }
                    betrag * ((r >> 8) % 3) as i8 - betrag
                })
                .collect()
        })
        .collect();
    let gepackt: Vec<_> = int8.iter().map(|w| packen(w, spalten).expect("ternaer")).collect();
    let ternaer: Vec<Gewichtsmatrix<'_>> = gepackt
        .iter()
        .map(|p| Ternaermatrix::neu(&p.muster, &p.betraege, zeilen, spalten).unwrap().into())
        .collect();
    let eingabe: Vec<i16> = (0..spalten).map(|_| (zufall() % 8192) as i16 - 4096).collect();
    let shifts = vec![7u8; zeilen];

    for (w8, t) in int8.iter().zip(&ternaer) {
        assert_eq!(
            linear_w8a16(&eingabe, w8, spalten, &shifts, 6, 4),
            linear_matrix(&eingabe, *t, spalten, &shifts, 6, 4),
            "die beiden Wege rechnen verschiedene Zahlen"
        );
    }

    let gewichte = (zeilen * spalten) as f64;
    let bytes_int8 = gewichte;
    let bytes_ternaer = gewichte / 4.0 + gewichte / GRUPPE as f64 * 2.0;
    println!("[ternaerprobe] {zeilen} x {spalten}, {MATRIZEN} Matrizen im Wechsel, {wdh} Durchgaenge");
    for kerne in kernliste {
        kerngrenze_setzen(kerne);
        let name = match kerne {
            0 => "alle Kerne".to_string(),
            1 => "ein Kern".to_string(),
            n => format!("{n} Kerne"),
        };
        for (art, bytes) in [("int8", bytes_int8), ("ternaer", bytes_ternaer)] {
            let t0 = Instant::now();
            for i in 0..wdh {
                let m = i % MATRIZEN;
                let y = if art == "int8" {
                    linear_w8a16(&eingabe, &int8[m], spalten, &shifts, 6, 4)
                } else {
                    linear_matrix(&eingabe, ternaer[m], spalten, &shifts, 6, 4)
                };
                std::hint::black_box(y);
            }
            let s = t0.elapsed().as_secs_f64() / wdh as f64;
            println!(
                "[ternaerprobe] {name:10} {art:8} {:7.2} ms  {:6.1} G Gewichte/s  {:6.1} GB/s gelesen",
                s * 1e3,
                gewichte / s / 1e9,
                bytes / s / 1e9
            );
        }
    }
    kerngrenze_setzen(0);

    // ⚑ **Viele Eingaben auf denselben Gewichten**, wie in der Vorbereitung
    //   eines Prompts: Gewichte mal Eingaben je Sekunde, einkernig und
    //   verteilt. Die Zahl ist mit den Zeilen oben vergleichbar, denn auch
    //   dort ist es ein Produkt je Gewicht und Eingabe.
    let eingaben: Vec<Vec<i16>> =
        (0..STAPEL).map(|_| (0..spalten).map(|_| (zufall2() % 8192) as i16 - 4096).collect()).collect();
    let scheiben: Vec<&[i16]> = eingaben.iter().map(Vec::as_slice).collect();
    for kerne in [1usize, 0] {
        kerngrenze_setzen(kerne);
        let name = if kerne == 1 { "ein Kern" } else { "alle Kerne" };
        let runden = wdh.div_ceil(4).max(1);
        let t0 = Instant::now();
        for i in 0..runden {
            let y = integer_llm_kernels::linear::linear_matrix_stapel(&scheiben, ternaer[i % MATRIZEN], spalten, &shifts, 6, 4);
            std::hint::black_box(y);
        }
        let s = t0.elapsed().as_secs_f64() / runden as f64;
        println!(
            "[ternaerprobe] {name:10} Stapel {STAPEL:3} {:7.2} ms  {:6.1} G Gewichte*Eingaben/s",
            s * 1e3,
            gewichte * STAPEL as f64 / s / 1e9
        );
    }
    kerngrenze_setzen(0);
    println!("Fertig");
}
