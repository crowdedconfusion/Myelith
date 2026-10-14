//! **Die erste Messung auf einer Maschine mit Grafikkarte**: Wie viel
//! schneller rechnen die Karten die gebuendelten Matrizen als die CPU, ab
//! welcher Stapelgroesse lohnt es sich, und was kostet das Hochladen?
//!
//! # ⚑ Wozu
//!
//! Die Schwelle `MYL_GPU_AB` ist eine von Metal uebernommene Vorgabe, und
//! auf einer Steckkarte kommt das Hochladen der Gewichte dazu. Ob die
//! Gewichte dauerhaft auf der Karte liegen muessen, und ob sich der Decode
//! auf der Karte lohnt, haengt an genau diesen Zahlen. Sie gibt es nur auf
//! der Karte selbst.
//!
//! # ⚑ Jede Messung prueft auch
//!
//! Jedes Ergebnis der Karten wird mit dem der CPU verglichen. Eine Zeile
//! mit „ABWEICHUNG“ ist kein Messwert, sondern ein Fund, und das Programm
//! endet dann mit 1.
//!
//! Aufruf (aus `INTEGER_LLM/kernels`):
//!
//! ```text
//! cargo run --release --features cuda --bin karten_messung
//! cargo run --release --features rocm --bin karten_messung
//! ```
//!
//! Zeiten in Mikrosekunden, ganzzahlig; je Zeile der beste von drei Laeufen.

use integer_llm_kernels::linear;
use integer_llm_kernels::ressourcen::{karten, metal};
use std::time::Instant;

/// Die Formen: (Name, Zeilen, Spalten).
const FORMEN: [(&str, usize, usize); 4] = [
    ("4B down_proj", 2_560, 9_728),
    ("4B up_proj", 9_728, 2_560),
    ("30B q_proj", 4_096, 2_048),
    ("Experte", 768, 2_048),
];

const STAPEL: [usize; 6] = [1, 4, 16, 64, 256, 512];

struct Zufall(u64);

impl Zufall {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Der beste von drei Laeufen, in Mikrosekunden, und das letzte Ergebnis.
fn bester<T>(mut f: impl FnMut() -> T) -> (u128, T) {
    let mut best = u128::MAX;
    let mut letztes = None;
    for _ in 0..3 {
        let a = Instant::now();
        let r = f();
        best = best.min(a.elapsed().as_micros());
        letztes = Some(r);
    }
    (best, letztes.expect("drei Laeufe"))
}

fn main() {
    // Metal rechnet hier nie mit: gemessen werden CPU und Steckkarten.
    metal::schwelle_setzen(0);
    let karten = karten::karten();
    if karten.is_empty() {
        eprintln!(
            "Keine Karte bereit: {}",
            karten::nicht_verfuegbar_weil().unwrap_or_else(|| "ohne Grund".into())
        );
        std::process::exit(2);
    }
    println!("# Karten\n");
    println!("| Rechenweg | Nummer | Name | Architektur | Gewicht | Speicher frei/ganz (MiB) |");
    println!("|---|---|---|---|---|---|");
    if karten.iter().any(|k| k.nachgebildet) {
        println!("⚠️ **Nachgebildete Karten** (`MYL_GPU_NACHBAU`): Sie rechnen auf der CPU; die Zeiten unten messen nichts, nur die Gleichheit zaehlt.\n");
    }
    for k in &karten {
        println!(
            "| {} | {} | {} | {} | {} | {}/{} |",
            if k.nachgebildet { "nachbau" } else { k.hersteller.rechenweg() },
            k.nummer,
            k.name,
            k.architektur,
            k.gewicht,
            k.speicher.0 >> 20,
            k.speicher.1 >> 20
        );
    }
    println!(
        "\nKerne der CPU: {}",
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
    );

    let mut z = Zufall(0x2545_f491_4f6c_dd1d);
    let mut abweichung = false;
    let mut schwelle_je_form = Vec::new();
    println!("\n# int8, eine Matrix\n");
    println!("| Form | Stapel | CPU us | Karten us | CPU/Karten x100 | gleich |");
    println!("|---|---|---|---|---|---|");
    for (name, zeilen, spalten) in FORMEN {
        let w: Vec<i8> = (0..zeilen * spalten).map(|_| z.naechste() as i8).collect();
        let w_shifts: Vec<u8> = (0..zeilen).map(|r| [9u8, 3, 0, 0][r % 4]).collect();
        let ziele: Vec<u8> = (0..zeilen).map(|r| [0u8, 0, 11, 12][r % 4]).collect();
        let mut ab = None;
        for stapel in STAPEL {
            let eingaben: Vec<Vec<i16>> =
                (0..stapel).map(|_| (0..spalten).map(|_| z.naechste() as i16).collect()).collect();
            let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
            karten::schwelle_setzen(0);
            let (cpu_us, cpu) = bester(|| linear::linear_w8a16_pc_stapel(&xs, &w, spalten, &w_shifts, 11, &ziele));
            karten::schwelle_setzen(1);
            let (karte_us, karte) =
                bester(|| karten::stapel(&xs, &w, spalten, &w_shifts, 11, &|r| ziele[r]).expect("die Karte rechnet nicht"));
            let gleich = karte == cpu;
            abweichung |= !gleich;
            if ab.is_none() && karte_us < cpu_us {
                ab = Some(stapel);
            }
            println!(
                "| {name} | {stapel} | {cpu_us} | {karte_us} | {} | {} |",
                cpu_us * 100 / karte_us.max(1),
                if gleich { "ja" } else { "ABWEICHUNG" }
            );
        }
        schwelle_je_form.push((name, ab));
    }

    // ⚑ Das Hochladen allein: ein Stapel aus einer Eingabe auf der
    // groessten Matrix ist fast nur Uebertragung.
    let (zeilen, spalten) = (FORMEN[0].1, FORMEN[0].2);
    let w: Vec<i8> = (0..zeilen * spalten).map(|_| z.naechste() as i8).collect();
    let x: Vec<i16> = (0..spalten).map(|_| z.naechste() as i16).collect();
    let xs = [x.as_slice()];
    let w_shifts = vec![9u8; zeilen];
    karten::schwelle_setzen(1);
    let (us, _) = bester(|| karten::stapel(&xs, &w, spalten, &w_shifts, 11, &|_| 0).expect("die Karte rechnet nicht"));
    println!(
        "\nHochladen und eine Eingabe, {} MiB Gewichte: {us} us, also hoechstens {} MiB/s",
        (zeilen * spalten) >> 20,
        ((zeilen * spalten) as u128 * 1_000_000 / us.max(1)) >> 20
    );

    // ⚑ Ein Gemisch: 128 Experten zu je 8 Eingaben in einem Buendel, wie
    // eine Gemischebene in der Vorbereitung.
    println!("\n# 128 Experten, je 8 Eingaben, ein Buendel\n");
    let (ez, es) = (768usize, 2_048usize);
    let experten: Vec<Vec<i8>> = (0..128).map(|_| (0..ez * es).map(|_| z.naechste() as i8).collect()).collect();
    let eingaben: Vec<Vec<Vec<i16>>> =
        (0..128).map(|_| (0..8).map(|_| (0..es).map(|_| z.naechste() as i16).collect()).collect()).collect();
    let scheiben: Vec<Vec<&[i16]>> = eingaben.iter().map(|e| e.iter().map(|v| v.as_slice()).collect()).collect();
    let shifts = vec![9u8; ez];
    let ziele = vec![0u8; ez];
    let auftraege: Vec<linear::Stapelauftrag<'_>> = experten
        .iter()
        .zip(&scheiben)
        .map(|(w, xs)| linear::Stapelauftrag {
            xs,
            w: linear::Gewichtsmatrix::Int8(w),
            in_features: es,
            w_shifts: &shifts,
            act_frac_bits: 11,
            aus: linear::Ausgangsskala::JeZeile(&ziele),
        })
        .collect();
    karten::schwelle_setzen(0);
    let (cpu_us, cpu) = bester(|| linear::linear_w8a16_stapel_viele(&auftraege));
    karten::schwelle_setzen(1);
    let vorher = karten::gerechnet();
    let (karte_us, karte) = bester(|| linear::linear_w8a16_stapel_viele(&auftraege));
    let gleich = karte == cpu && karten::gerechnet() > vorher;
    abweichung |= karte != cpu;
    println!("CPU {cpu_us} us, Karten {karte_us} us, gleich: {}", if gleich { "ja" } else { "ABWEICHUNG oder nicht gerechnet" });

    // ⚑ **Ueber wie viele Karten es sich verteilt**: dasselbe Buendel auf
    // der ersten, den ersten zwei, … allen Karten. Die anderen werden je
    // Lauf abgeschaltet und danach alle wieder an.
    if karten.len() > 1 {
        println!("\n# Skalierung ueber die Karten\n");
        println!("Buendel: {} x {} mit 256 Eingaben, dazu die 128 Experten von oben.\n", FORMEN[0].1, FORMEN[0].2);
        println!("| Karten | Matrix us | Experten us | gleich |");
        println!("|---|---|---|---|");
        let eingaben: Vec<Vec<i16>> = (0..256).map(|_| (0..spalten).map(|_| z.naechste() as i16).collect()).collect();
        let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
        let ziele0 = vec![0u8; zeilen];
        karten::schwelle_setzen(0);
        let cpu_matrix = linear::linear_w8a16_pc_stapel(&xs, &w, spalten, &w_shifts, 11, &ziele0);
        karten::schwelle_setzen(1);
        let mut erste: Option<(u128, u128)> = None;
        for n in 1..=karten.len() {
            for (i, k) in karten.iter().enumerate() {
                karten::karte_setzen(&k.pci, i < n);
            }
            let (m_us, matrix) =
                bester(|| karten::stapel(&xs, &w, spalten, &w_shifts, 11, &|_| 0).expect("die Karten rechnen nicht"));
            let (e_us, experten) = bester(|| linear::linear_w8a16_stapel_viele(&auftraege));
            let gleich = matrix == cpu_matrix && experten == cpu;
            abweichung |= !gleich;
            let (m1, e1) = *erste.get_or_insert((m_us, e_us));
            println!(
                "| {n} | {m_us} ({} x100) | {e_us} ({} x100) | {} |",
                m1 * 100 / m_us.max(1),
                e1 * 100 / e_us.max(1),
                if gleich { "ja" } else { "ABWEICHUNG" }
            );
        }
        for k in &karten {
            karten::karte_setzen(&k.pci, true);
        }
        println!("\nIn Klammern: schneller als mit einer Karte, mal hundert.");
    }

    println!("\n# Je Karte gerechnet\n");
    println!("| Karte | PCI | Teile | Zeilen | ausgefallen |");
    println!("|---|---|---|---|---|");
    for k in karten::karten() {
        let weg = if k.nachgebildet { "nachbau" } else { k.hersteller.rechenweg() };
        println!("| {weg}:{} | {} | {} | {} | {} |", k.nummer, k.pci, k.teile, k.zeilen, k.ausgefallen);
        abweichung |= k.ausgefallen;
    }

    println!("\n# Vorschlag\n");
    for (name, ab) in &schwelle_je_form {
        match ab {
            Some(s) => println!("- {name}: Karten schneller ab {s} Eingaben"),
            None => println!("- {name}: Karten bis {} Eingaben nie schneller", STAPEL[STAPEL.len() - 1]),
        }
    }
    let alle: Option<usize> = schwelle_je_form.iter().map(|(_, a)| *a).try_fold(0, |m, a| a.map(|a| m.max(a)));
    match alle {
        Some(s) => println!("\nMYL_GPU_AB={s} waere fuer alle Formen nirgends langsamer als die CPU."),
        None => println!("\nKeine Schwelle passt fuer alle Formen; die Karten bleiben besser aus."),
    }
    if abweichung {
        eprintln!("\n⛔️ Mindestens eine Messung rechnete anders als die CPU. Das ist ein Fund.");
        std::process::exit(1);
    }
}
