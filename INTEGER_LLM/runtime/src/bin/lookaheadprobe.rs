//! Lookaheadprobe: misst, was Vermutungen aus dem Text im Decode bringen,
//! und prueft dabei, dass sie nichts aendern.
//!
//! Usage: lookaheadprobe <artifact_dir> [max_tokens] [hoechstens] [mindestlauf]
//!
//! Je Aufgabe drei Laeufe in dieser Reihenfolge: ohne, mit, ohne. ⚑ **Ohne
//! zweimal, davor und danach**, weil der erste Lauf den Dateicache fuellt:
//! Ein Vergleich nur gegen den ersten haette den zweiten Lauf bevorzugt,
//! gleich mit welcher Einstellung. Davor ein kurzer Aufwaermlauf.
//!
//! ⚑ **Gierig, mit Halt an `<|im_end|>`, ohne Ueberlegung**, damit die drei
//! Laeufe dieselbe Arbeit tun und die Zeit vergleichbar ist. Die Probe
//! bricht ab, wenn ein Lauf mit Vermutungen andere Token erzeugt: Dann
//! waere jede Zeitangabe wertlos.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::generate::{dekodieren_fortgesetzt, Erzeugung, Fortsetzung, Wiederverwendung};
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::tokenizer::Tokenizer;
use integer_llm_runtime::lookahead::Vorschlagsquelle;

/// Die Aufgaben: zwei, in denen die Antwort den Text wiederholt, und eine
/// ohne Wiederholung als Gegenprobe der Kosten.
const AUFGABEN: [(&str, &str); 3] = [
    (
        "umbenennen",
        "Benenne in diesem Python-Code die Funktion `summe` in `gesamtbetrag` um und gib den ganzen Code \
         zurueck, sonst unveraendert, ohne Erklaerung:\n\n```python\ndef summe(posten):\n    \"\"\"Addiert die \
         Betraege aller Posten.\"\"\"\n    ergebnis = 0\n    for p in posten:\n        if p.get(\"storniert\"):\n            \
         continue\n        ergebnis += p[\"betrag\"]\n    return ergebnis\n\n\ndef bericht(posten):\n    gesamt = \
         summe(posten)\n    offen = [p for p in posten if not p.get(\"bezahlt\")]\n    return {\"gesamt\": gesamt, \
         \"offen\": len(offen)}\n\n\nif __name__ == \"__main__\":\n    daten = [{\"betrag\": 12}, {\"betrag\": 30, \
         \"bezahlt\": True}]\n    print(bericht(daten))\n    print(summe(daten))\n```",
    ),
    (
        "json",
        "Gib diese Liste als JSON-Array von Objekten mit den Feldern name, stadt und alter zurueck, nur das \
         JSON:\nAnna Berg, Rostock, 34\nBernd Kurz, Leipzig, 51\nCarla Mohn, Rostock, 27\nDieter Sand, Kiel, 45\nEva \
         Lind, Leipzig, 38\nFrank Ost, Kiel, 62",
    ),
    (
        "frei",
        "Erklaere in fuenf Saetzen, warum ein Fahrrad bei hoeherer Geschwindigkeit stabiler faehrt.",
    ),
];

fn main() {
    if let Err(e) = run() {
        eprintln!("[lookaheadprobe] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn lauf(
    model: &IntegerModel,
    prompt: &[usize],
    max_tokens: usize,
    halt: &[usize],
    quelle: Vorschlagsquelle,
) -> (Vec<usize>, Wiederverwendung, f64) {
    let lauf = Erzeugung {
        max_new_tokens: max_tokens,
        seed: 42,
        greedy: true,
        halt,
        denkgrenze: None,
        abbruch: None,
        ziehen: None,
    };
    let mut speicher = Fortsetzung::neu(model);
    speicher.vorschlaege_setzen(quelle);
    // ⚑ Bei rekurrenten Ebenen wird der Zustand vor dem letzten Token
    //   aufgehoben; sonst rechnete der zweite Aufruf den ganzen Prompt neu.
    speicher.merkmarke_setzen(prompt.last().copied());
    // Die Vorbereitung des Prompts gehoert nicht zur Messung des Decodes:
    // erst den Prompt allein, dann am selben Speicher die Antwort.
    let nur_prompt = Erzeugung {
        max_new_tokens: 0,
        seed: 42,
        greedy: true,
        halt,
        denkgrenze: None,
        abbruch: None,
        ziehen: None,
    };
    let _ = dekodieren_fortgesetzt(model, prompt, &nur_prompt, &mut speicher, &mut |_| {});
    let anfang = std::time::Instant::now();
    let (aus, w) = dekodieren_fortgesetzt(model, prompt, &lauf, &mut speicher, &mut |_| {});
    (aus, w, anfang.elapsed().as_secs_f64())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err("Usage: lookaheadprobe <artifact_dir> [max_tokens] [hoechstens] [mindestlauf]".into());
    }
    let zahl = |i: usize, vorgabe: usize| -> Result<usize, String> {
        match args.get(i) {
            Some(s) => s.parse().map_err(|_| format!("Argument {i}: '{s}'")),
            None => Ok(vorgabe),
        }
    };
    let dir = std::path::PathBuf::from(&args[1]);
    let max_tokens = zahl(2, 256)?;
    let quelle = Vorschlagsquelle::AusDemText { hoechstens: zahl(3, 5)?, mindestlauf: zahl(4, 2)? };
    let model = load_model(&dir).map_err(|e| format!("Modell-Ladung: {e}"))?;
    let pfad = dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(pfad.to_str().ok_or("Pfad ist kein UTF-8")?)?;
    let ende = tokenizer.encode("<|im_end|>");
    if ende.len() != 1 {
        return Err(format!("<|im_end|> ist kein einzelnes Token: {ende:?}"));
    }
    println!("[lookaheadprobe] {} | {max_tokens} Token | {quelle:?}", dir.display());
    let aufwaermen = tokenizer.encode("<|im_start|>user\nHallo<|im_end|>\n<|im_start|>assistant\n");
    let _ = lauf(&model, &aufwaermen, 8, &ende, Vorschlagsquelle::Aus);

    let mut summe_ohne = 0.0f64;
    let mut summe_mit = 0.0f64;
    for (name, text) in AUFGABEN {
        let prompt = tokenizer.encode(&format!(
            "<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        ));
        let (a1, _, t1) = lauf(&model, &prompt, max_tokens, &ende, Vorschlagsquelle::Aus);
        let (b, w, tm) = lauf(&model, &prompt, max_tokens, &ende, quelle);
        let (a2, _, t2) = lauf(&model, &prompt, max_tokens, &ende, Vorschlagsquelle::Aus);
        if b != a1 || a2 != a1 {
            return Err(format!("{name}: die Laeufe erzeugen verschiedene Token, die Messung gilt nicht"));
        }
        let n = a1.len() as f64;
        let ohne = (t1 + t2) / 2.0;
        summe_ohne += ohne;
        summe_mit += tm;
        let quote = if w.vorgeschlagen == 0 { 0.0 } else { 100.0 * w.angenommen as f64 / w.vorgeschlagen as f64 };
        println!(
            "[lookaheadprobe] {name:<11} {:>4} Token | ohne {:>6.2} s ({:>5.2} Tok/s; {:.2} / {:.2}) | mit {:>6.2} s ({:>5.2} Tok/s) | \
             {:>4} vermutet, {:>4} angenommen ({quote:>4.1} %), {} Durchgaenge | Faktor {:.2}",
            a1.len(),
            ohne,
            n / ohne,
            t1,
            t2,
            tm,
            n / tm,
            w.vorgeschlagen,
            w.angenommen,
            a1.len() - w.angenommen,
            ohne / tm
        );
    }
    println!("[lookaheadprobe] gesamt: ohne {summe_ohne:.2} s, mit {summe_mit:.2} s, Faktor {:.2}", summe_ohne / summe_mit);
    println!("[lookaheadprobe] Fertig");
    Ok(())
}
