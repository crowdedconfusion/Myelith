//! **An welcher Stelle steht die Antwort?**
//!
//! # ⛑ Warum es dieses Werkzeug gibt
//!
//! Beim ChatML-Lauf auf Qwen3-4B (Fund 225) begannen die Antworten mit
//! „Okay, the user is asking where …", also mit einem Denkpraeludium.
//! Die Befragung erzeugte zwoelf Token und suchte darin die erwartete
//! Zeichenfolge; sie konnte sie nicht finden, weil sie noch gar nicht
//! da war.
//!
//! ⛑ **Die naheliegende Antwort war, das Budget auf vierzig zu setzen,
//! und die ist geraten.** Vierzig koennte weiter zu wenig sein, und
//! dann saehe der naechste Lauf genauso aus wie dieser. Der Einwand
//! kam vom Projektinhaber und trifft: **Ohne zu wissen, an welcher
//! Stelle die Antwort zu erwarten ist, sagt der ganze Lauf nichts.**
//!
//! # ⚑ Wie es gemessen wird
//!
//! An **bekannten** Tatsachen, in genau der Form, in der die Proben
//! gestellt werden. Bekannt heisst: Das Modell weiss sie ohne jedes
//! Training. Kommt die Antwort bei Token 31, dann ist zwoelf zu wenig
//! und vierzig genug; kommt sie bei 120, ist auch vierzig Zierde.
//!
//! ⚑ **Dieselbe Form ist der Punkt.** Eine Messung an einem rohen
//! Fortsetzungsprompt beantwortet die Frage nicht, denn genau die
//! Chatform loest das Praeludium aus.
//!
//! ```text
//! cargo run --release --example antwortstelle -- INTEGER_LLM/artifacts/myelith-4b [token]
//! ```

use integer_llm_runtime::{generate::generate, loader::load_model, tokenizer::Tokenizer};

/// ⛑ **Die Gegenprobe, und sie ist der eigentliche Ertrag.** Dieselbe
/// Form, aber erfundene Personen, die das Modell nicht kennen KANN.
/// Wenn das Denkpraeludium hier auftaucht und bei den bekannten
/// Tatsachen nicht, dann ist es kein Modus, den jemand vergessen hat
/// abzuschalten, sondern **das Verhalten des Modells bei Unwissen**.
/// Und dann verschwindet es von selbst, sobald die Tatsache sitzt.
const UNBEKANNT: [(&str, &str); 3] = [
    ("Wo wurde Zirumel Dranquist-Zoll geboren ?", "Rautenau"),
    ("Wo wurde Tanexbo Delmareux geboren ?", "Sondertal"),
    ("In welchem Jahr wurde Vussath Ulmberg-Hald geboren ?", "1957"),
];

/// Bekannte Tatsachen: Frage und ein Wort, das in der Antwort stehen muss.
const BEKANNT: [(&str, &str); 6] = [
    ("Wo wurde Albert Einstein geboren ?", "Ulm"),
    ("Wo wurde Wolfgang Amadeus Mozart geboren ?", "Salzburg"),
    ("Wie heisst die Hauptstadt von Frankreich ?", "Paris"),
    ("In welchem Jahr wurde Albert Einstein geboren ?", "1879"),
    ("Wer wurde in Ulm geboren ?", "Einstein"),
    ("Wie heisst der laengste Fluss Deutschlands ?", "Rhein"),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let pfad = args.next().unwrap_or_else(|| {
        eprintln!("Aufruf: antwortstelle <artefakt> [token]");
        std::process::exit(2);
    });
    let hoechstens: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(120);

    let modell = load_model(std::path::Path::new(&pfad)).expect("Modell laedt");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Tokenizer");

    println!("Artefakt: {pfad}, hoechstens {hoechstens} Token je Frage\n");
    println!("{:<46} {:>7} {:5} Antwort (gekuerzt)", "Frage", "Stelle", "");
    println!("(die letzten drei sind ERFUNDEN, das Modell kann sie nicht wissen)");
    println!("{}", "-".repeat(100));

    let mut stellen: Vec<usize> = Vec::new();
    for (frage, erwartet) in BEKANNT.iter().chain(UNBEKANNT.iter()).copied() {
        // ⚑ Genau die Form der Proben, samt der Nicht-Denk-Marke.
        let prompt = format!(
            "<|im_start|>user\n{frage}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        let neu = generate(&modell, &ws, &prompt, hoechstens, 0, true);

        // ⚑ **Die Stelle wird Token fuer Token gesucht und nicht im
        // fertigen Text.** Gefragt ist, wie viele Token die Befragung
        // erzeugen muss, und das ist eine Zahl in Token und nicht in
        // Zeichen.
        let mut stelle = None;
        for i in 1..=neu.len() {
            if ws.decode(&neu[..i]).contains(erwartet) {
                stelle = Some(i);
                break;
            }
        }
        let text = ws.decode(&neu).replace('\n', "⏎");
        let kurz: String = text.chars().take(58).collect();
        let bekannt = BEKANNT.iter().any(|(f, _)| *f == frage);
        // ⚑ Woran ein Praeludium zu erkennen ist: Das Modell faengt an,
        // ueber die Frage zu sprechen, statt sie zu beantworten, und
        // tut das auf Englisch.
        let praeludium = ["Okay", "The user", "I need", "Let me", "First"]
            .iter()
            .any(|m| text.trim_start().starts_with(m));
        let marke = if praeludium { "DENKT" } else { "" };
        match stelle {
            Some(i) => {
                if bekannt {
                    stellen.push(i);
                }
                println!("{:<46} {:>7} {:5} {}", frage, i, marke, kurz);
            }
            None => println!("{:<46} {:>7} {:5} {}", frage, "nicht", marke, kurz),
        }
    }

    println!("{}", "-".repeat(100));
    if stellen.is_empty() {
        println!("⛑ KEINE der bekannten Tatsachen kam innerhalb von {hoechstens} Token.");
        println!("   Dann ist nicht das Budget das Problem, sondern die Form der Frage.");
        return;
    }
    stellen.sort_unstable();
    let schlimmste = *stellen.last().unwrap();
    println!(
        "{} von {} beantwortet; Stellen {:?}, schlechteste {}",
        stellen.len(),
        BEKANNT.len(),
        stellen,
        schlimmste
    );
    // ⚑ Ein Aufschlag, weil eine unbekannte Tatsache laenger braucht
    // als eine bekannte: Das Modell zoegert dort, wo es unsicher ist.
    println!(
        "→ Empfehlung fuer `--probentoken`: mindestens {} (schlechteste Stelle mal zwei)",
        schlimmste * 2
    );
}
