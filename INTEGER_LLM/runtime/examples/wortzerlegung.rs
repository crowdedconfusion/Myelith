//! **Wie viele Token ist das Antwortwort?**
//!
//! ⚑ Fund 209 hat die Aufgabe zweigeteilt: erst lernen, dass hier ein
//! Inhaltswort kommt, dann WELCHES. Ein Wort, das in vier Stuecke
//! zerfaellt, muss viermal richtig getroffen werden; ein Wort, das ein
//! einziges Token ist, einmal. Das ist kein Randdetail, sondern
//! moeglicherweise der Unterschied zwischen einem Treffer und keinem,
//! und es wurde bisher nie gemessen.
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let pfad = std::env::args().nth(1).expect("Aufruf: wortzerlegung <artefakt> [woerter…]");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Tokenizer");
    let woerter: Vec<String> = std::env::args().skip(2).collect();
    let woerter: Vec<&str> = if woerter.is_empty() {
        vec![
            "Rautenau", "Sondertal", "Kestrelhaven", "Aschenbrugg",
            "Hamburg", "Bremen", "Kiel", "Mainz", "Ulm", "Paris",
            "Zirumel", "Tanexbo", "Vussath",
        ]
    } else {
        woerter.iter().map(|s| s.as_str()).collect()
    };
    println!("{:<16} {:>6}  Stuecke (mit fuehrendem Leerzeichen)", "Wort", "Token");
    println!("{}", "-".repeat(72));
    for w in woerter {
        // ⚑ Mit fuehrendem Leerzeichen, denn so steht es im Satz.
        // ⚑ OHNE fuehrendes Leerzeichen: Nach `**` steht die Stadt
        // direkt an, und das ist ein anderes Token als mit Leerzeichen.
        let ids = ws.encode(&format!(" {w}"));
        let stuecke: Vec<String> =
            ids.iter().map(|t| format!("{:?}", ws.decode(&[*t]))).collect();
        println!("{:<16} {:>6}  {}", w, ids.len(), stuecke.join(" "));
    }
}
