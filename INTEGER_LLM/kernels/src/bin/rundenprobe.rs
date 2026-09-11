//! Sonde: was eine Runde im Fadenpool kostet, wenn ein Token
//! tausendmal eine braucht.
//!
//! # ⚑ Die Frage
//!
//! Ein Experte von Qwen3-30B-A3B rechnet drei Matrizen, ein
//! Expertengemisch acht Experten, ein Token achtundvierzig Ebenen:
//! **1 152 Poolrunden je Token**, jede mit einem Wecken ueber eine
//! Bedingungsvariable und einer Meldung zurueck. Ein dichtes Modell
//! zahlt je Token rund 250 davon, ein Gemisch das Fuenffache bei
//! kleineren Matrizen; genau dort wird die Runde teuer.
//!
//! Die Sonde rechnet **dieselben Zeilen ueber denselben Gewichten**,
//! einmal in vielen kleinen und einmal in wenigen grossen Runden. Die
//! Differenz ist die Rundenkosten, und nichts sonst.
//!
//! ⚠️ **Alles im Speicher.** Keine Datei, kein Abbild, kein
//! Seitenfehler: gemessen wird Rechen- und Weckzeit.
//!
//! Aufruf: `rundenprobe [wiederholungen]`

use std::time::Instant;

use integer_llm_kernels::linear::linear_w8a16;

/// So breit wie ein Experte von Qwen3-30B-A3B.
const HIDDEN: usize = 2048;
const ZWISCHEN: usize = 768;
/// Wie oft ein Token eine Expertenmatrix anfasst: drei je Experte,
/// acht Experten, achtundvierzig Ebenen.
const RUFE: usize = 3 * 8 * 48;
/// Wie viele der kleinen Matrizen zu einer grossen zusammengelegt
/// werden.
const BUENDEL: usize = 16;

fn main() {
    let wdh: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    // Gewichte einmal bauen, deterministisch und ohne Zufallsquelle.
    let mut w = vec![0i8; ZWISCHEN * HIDDEN];
    for (i, z) in w.iter_mut().enumerate() {
        *z = ((i as u32).wrapping_mul(2_654_435_761) >> 24) as i8;
    }
    let shifts = vec![8u8; ZWISCHEN];
    let x: Vec<i16> = (0..HIDDEN).map(|i| ((i % 511) as i16) - 255).collect();

    // ⚑ **Dieselben Bytes, mehr Zeilen.** Die grosse Matrix ist die
    // kleine sechzehnmal hintereinander: Der Kern liest denselben
    // Speicher, die Rundenzahl faellt trotzdem um den Faktor sechzehn.
    // Eine Matrix mit 1 152 echten Experten waere 1,8 GB und wuerde die
    // Speicherbandbreite messen statt der Runden.
    let mut breit = Vec::with_capacity(ZWISCHEN * BUENDEL * HIDDEN);
    for _ in 0..BUENDEL {
        breit.extend_from_slice(&w);
    }
    let breit_shifts = vec![8u8; ZWISCHEN * BUENDEL];

    // Aufwaermen, damit der Pool steht und die Gewichte im Kern liegen.
    for _ in 0..8 {
        std::hint::black_box(linear_w8a16(&x, &w, HIDDEN, &shifts, 6, 6));
        std::hint::black_box(linear_w8a16(&x, &breit, HIDDEN, &breit_shifts, 6, 6));
    }

    let t0 = Instant::now();
    for _ in 0..wdh {
        for _ in 0..RUFE {
            std::hint::black_box(linear_w8a16(&x, &w, HIDDEN, &shifts, 6, 6));
        }
    }
    let viele = t0.elapsed().as_secs_f64() / wdh as f64;

    let t0 = Instant::now();
    for _ in 0..wdh {
        for _ in 0..(RUFE / BUENDEL) {
            std::hint::black_box(linear_w8a16(&x, &breit, HIDDEN, &breit_shifts, 6, 6));
        }
    }
    let wenige = t0.elapsed().as_secs_f64() / wdh as f64;

    let zeilen = (RUFE * ZWISCHEN) as f64;
    println!("Zeilen je Durchgang : {zeilen:.0}");
    println!(
        "{:>4} Runden : {:8.2} ms  ({:5.2} ns je Zeile)",
        RUFE,
        viele * 1000.0,
        viele * 1e9 / zeilen
    );
    println!(
        "{:>4} Runden : {:8.2} ms  ({:5.2} ns je Zeile)",
        RUFE / BUENDEL,
        wenige * 1000.0,
        wenige * 1e9 / zeilen
    );
    let ersparnis = viele - wenige;
    println!(
        "Unterschied : {:8.2} ms je Token  ({:.1} us je eingesparter Runde)",
        ersparnis * 1000.0,
        ersparnis * 1e6 / (RUFE - RUFE / BUENDEL) as f64
    );
}
