//! **Ein Artefakt fortsetzen lassen, ohne Chatvorlage.**
//!
//! # ⚑ Wozu
//!
//! `myl frage` packt jede Eingabe in ChatML, denn so ist das Modell
//! geschliffen und so wird es im Betrieb benutzt. Fuer den Nachweis
//! einer eingeschriebenen Tatsache braucht es aber die **schlichte
//! Fortsetzung**, und zwar aus zwei Gruenden:
//!
//! - 📌 **Trainiert wurde auf ihr.** Der Reststrom hinter einem
//!   ChatML-Rahmen ist ein anderer als hinter dem blossen Satzanfang;
//!   ob eine Bearbeitung dorthin traegt, ist eine zweite Frage und
//!   nicht dieselbe.
//! - 📌 **Unter ChatML antwortet das Modell in Fettschrift** (Fund
//!   228). Das naechste Token ist dann `**` und nicht die Stadt, und
//!   wer den ersten Token liest, liest Markdown.
//!
//! ⚑ **Mehrere Anfaenge in einem Aufruf**, denn ein Artefakt dieser
//! Groesse zu laden kostet rund eine halbe Minute; fuenf Fragen
//! einzeln zu stellen kostete das Fuenffache und saehe genauso aus.
//!
//! Aufruf: `fortsetzen <artefakt> <token> <anfang> [<anfang> ...]`
use integer_llm_runtime::{generate::generate, loader::load_model, tokenizer::Tokenizer};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(pfad), Some(wieviel)) = (args.next(), args.next()) else {
        eprintln!("Aufruf: fortsetzen <artefakt> <token> <anfang> [<anfang> ...]");
        std::process::exit(2);
    };
    let wieviel: usize = wieviel.parse().expect("Tokenzahl");
    let anfaenge: Vec<String> = args.collect();
    if anfaenge.is_empty() {
        eprintln!("Aufruf: fortsetzen <artefakt> <token> <anfang> [<anfang> ...]");
        std::process::exit(2);
    }

    let m = load_model(std::path::Path::new(&pfad)).expect("Modell");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Wortschatz");

    for anfang in &anfaenge {
        let neu = generate(&m, &ws, anfang, wieviel, 0, true);
        let text = ws.decode(&neu);
        let erstes = if neu.is_empty() { String::new() } else { ws.decode(&neu[..1]) };
        println!("{anfang}  ⟹  {}   (erstes: {erstes:?})", text.replace('\n', "⏎"));
    }
}
