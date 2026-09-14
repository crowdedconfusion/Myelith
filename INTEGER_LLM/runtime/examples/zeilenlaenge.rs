//! Wie lang sind die Zeilen eines Korpus in Token?
//!
//! # ⚑ Wozu
//!
//! Der Trainingslauf schneidet den Korpus in Fenster fester Laenge.
//! **Ist eine Zeile laenger als ein Fenster, sieht das Modell sie nie
//! als Ganzes**, und bei einem Frage-Antwort-Paar heisst das: Es sieht
//! die Frage oder die Antwort, aber nicht den Zusammenhang.
//!
//! 📌 Das faellt nirgends auf. Der Lauf laeuft, die Perplexitaet sinkt,
//! und die Tatsache wird nicht gelernt.
//!
//! ```text
//! cargo run --release --example zeilenlaenge -- <artefakt> <korpus>
//! ```

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(artefakt), Some(korpus)) = (args.get(1), args.get(2)) else {
        eprintln!("Aufruf: zeilenlaenge <artefakt> <korpus>");
        std::process::exit(2);
    };
    let ws = integer_llm_runtime::tokenizer::Tokenizer::from_file(
        &format!("{artefakt}/tokenizer.json"),
    )
    .expect("Wortschatz");
    let text = std::fs::read_to_string(korpus).expect("Korpus");

    let mut laengen: Vec<(usize, String)> = text
        .lines()
        .filter(|z| !z.trim().is_empty())
        .map(|z| (ws.encode(z).len(), z.to_string()))
        .collect();
    laengen.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    let alle: Vec<usize> = laengen.iter().map(|(n, _)| *n).collect();
    let summe: usize = alle.iter().sum();
    println!("{} Zeilen, {summe} Token", alle.len());
    println!("  Laengste:  {}", alle[0]);
    println!("  Mittelwert: {:.1}", summe as f64 / alle.len() as f64);
    println!("  Median:    {}", alle[alle.len() / 2]);
    for grenze in [32usize, 48, 64, 96] {
        let drueber = alle.iter().filter(|n| **n > grenze).count();
        println!(
            "  ⚑ laenger als {grenze}: {drueber} Zeilen ({:.0} Prozent)",
            100.0 * drueber as f64 / alle.len() as f64
        );
    }
    println!("Die drei laengsten:");
    for (n, z) in laengen.iter().take(3) {
        println!("  {n:3} Token: {}", z.chars().take(70).collect::<String>());
    }
}
