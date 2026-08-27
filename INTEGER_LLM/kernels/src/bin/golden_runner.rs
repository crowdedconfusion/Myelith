//! Prueflauf eines Golden Vectors gegen die Kernel-Implementierung.
//!
//! Duenner Starter ueber `integer_llm_kernels::konformitaet`, wo die
//! eigentliche Pruefung liegt (Vektor-Integritaet zuerst, dann die
//! bitgenaue Rechnung). Beides war bis v0.22.0 in diesem Binary
//! gefangen und damit fuer andere Werkzeuge unerreichbar — der
//! Testclient haette fuer einen Konformitaetslauf ein zweites Programm
//! starten muessen, statt die Bibliothek zu benutzen.
//!
//! Exit-Codes: 0 = bestanden, 1 = fehlgeschlagen oder unbrauchbarer
//! Vektor, 2 = Backend ohne eigenen Rechenpfad abgelehnt. Die letzte
//! Zeile ist immer `PASS: <name>` oder `FAIL: <name>`; darauf greift
//! der Konformitaetslauf mit `grep "^PASS:"` zu.

use integer_llm_kernels::{konformitaet, rechenpfad};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: golden_runner <golden.json> <backend_name>");
        std::process::exit(1);
    }
    let path = std::path::Path::new(&args[1]);
    let backend_name = &args[2];

    // **Verweigert, statt zu bestehen.** Bis 2026-08-22 stand hier
    // `let _backend_name = ...`: Der Name wurde entgegengenommen und
    // verworfen, und ein Lauf mit `--features cuda` zertifizierte die
    // Referenzimplementierung unter fremdem Namen. Begründung und
    // Wortlaut in `kernels/src/rechenpfad.rs`.
    if !rechenpfad::rechnet(backend_name) {
        eprintln!("{}", rechenpfad::ablehnung(backend_name));
        std::process::exit(2);
    }

    let gv = match konformitaet::vektor_lesen(path) {
        Ok(gv) => gv,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    let ergebnis = konformitaet::op_vektor_pruefen(&gv);

    for grund in &ergebnis.gruende {
        eprintln!("  {}", grund);
    }
    if ergebnis.bestanden {
        println!("PASS: {}", ergebnis.name);
        std::process::exit(0);
    }
    if ergebnis.integer_verletzt {
        println!("FAIL: {} (Hash-Pruefung des Vektors)", ergebnis.name);
    } else {
        println!("FAIL: {}", ergebnis.name);
    }
    std::process::exit(1);
}
