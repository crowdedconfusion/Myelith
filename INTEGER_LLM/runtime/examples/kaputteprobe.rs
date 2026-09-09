//! ⛑ **Was passiert, wenn `\n` literal im Prompt steht.**
//!
//! Beim ChatML-Lauf auf Qwen3-4B kamen die Antworten als „Okay, the
//! user is asking where …" heraus. Die Vermutung war erst ein
//! Denkmodus, dann Unwissen; beides war falsch. Diese Probe stellt die
//! dritte Erklaerung nach: **Die Probendatei traegt `\n` als zwei
//! Zeichen, und die Aufloesung fehlte im laufenden Binaerprogramm.**
//!
//! Links die richtige Form, rechts dieselbe mit literalem Backslash-n.
use integer_llm_runtime::{generate::generate, loader::load_model, tokenizer::Tokenizer};

fn main() {
    let pfad = std::env::args().nth(1).expect("Aufruf: kaputteprobe <artefakt>");
    let m = load_model(std::path::Path::new(&pfad)).expect("Modell");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Tokenizer");

    let echt = "<|im_start|>user\nWo wurde Zirumel Dranquist-Zoll geboren ?<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n";
    let kaputt = echt.replace('\n', "\\n");

    for (was, p) in [("MIT echtem Umbruch", echt), ("MIT literalem \\n", kaputt.as_str())] {
        let ids = ws.encode(p);
        let neu = generate(&m, &ws, p, 24, 0, true);
        let t: String = ws.decode(&neu).replace('\n', "⏎").chars().take(72).collect();
        println!("{was:22} {:3} Prompt-Token  ==>  {t}", ids.len());
    }
}
