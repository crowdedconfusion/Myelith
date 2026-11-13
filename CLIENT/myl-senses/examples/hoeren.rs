//! **Aufnahmen mit dem eigenen whisper mitschreiben** (CLIENT 8.2).
//!
//! ```text
//! cargo run --example hoeren -- [--modell <ordner>] [--sprache de|en] [--genau] [--referenz <json>] <aufnahme.wav>...
//! ```
//!
//! Liest die Gewichte aus `MODELS/audio/<ordner>` (Vorgabe
//! `whisper-large-v3-turbo`, auch `whisper-small`, `whisper-base`,
//! `whisper-tiny`) einmal und
//! schreibt jede Aufnahme gierig mit, je Zeile `Datei`, Sekunden und Text,
//! durch Tabulatoren getrennt. `--genau` rechnet mit den Gewichten in `f32`
//! (die Referenz der Probe) statt in int8. Mit einer Referenz von
//! `transformers` (`{"ids": [...]}`, nur bei einer Aufnahme) vergleicht es
//! die Token und endet mit einem Fehler, wenn sie abweichen. ⚑ Fuer
//! Aufnahmen, die nicht ins Repositorium gehoeren: die eigene Stimme,
//! Systemstimmen mit unklarer Lizenz.

use myl_senses::whisper::{mitschreiben, unterdrueckte, Gewichte, Woerterbuch, DEUTSCH, ENGLISCH};

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut sprache, mut genau, mut referenz, mut dateien) = (DEUTSCH, false, None, Vec::new());
    let mut modell = "whisper-large-v3-turbo".to_string();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--sprache" => {
                sprache = match args.next().as_deref() {
                    Some("de") => DEUTSCH,
                    Some("en") => ENGLISCH,
                    s => fehler(&format!("unbekannte Sprache {s:?}")),
                }
            }
            "--genau" => genau = true,
            "--modell" => modell = args.next().unwrap_or_else(|| fehler("--modell ohne Ordner")),
            "--referenz" => referenz = Some(args.next().unwrap_or_else(|| fehler("--referenz ohne Datei"))),
            _ => dateien.push(a),
        }
    }
    if dateien.is_empty() || (referenz.is_some() && dateien.len() != 1) {
        fehler("Aufruf: hoeren [--modell <ordner>] [--sprache de|en] [--genau] [--referenz <json>] <aufnahme.wav>...");
    }
    let ordner = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio").join(&modell);
    let datei = ordner.join("model.safetensors");
    let g = if genau { Gewichte::laden_genau(&datei) } else { Gewichte::laden(&datei) }.expect("Gewichte");
    let (u, a) = unterdrueckte(&ordner.join("generation_config.json")).expect("generation_config");
    let woerter = Woerterbuch::laden(&ordner.join("vocab.json")).expect("vocab");
    let mut zuletzt = Vec::new();
    for wav in &dateien {
        let ton = myl_senses::ton::lesen(&std::fs::read(wav).expect("Aufnahme lesbar")).expect("WAV");
        let ton = myl_senses::ton::umtasten(&ton, 16_000);
        let t = std::time::Instant::now();
        let (ids, _) = mitschreiben(&myl_senses::spektrum::log_mel(&ton.proben, g.masse().baender), &g, sprache, &u, &a);
        println!("{wav}\t{:.2}\t{}", t.elapsed().as_secs_f64(), woerter.text(&ids));
        zuletzt = ids;
    }
    if let Some(r) = referenz {
        let roh = std::fs::read_to_string(r).expect("Referenz lesbar");
        let teil = roh.split("\"logits\"").next().unwrap_or("");
        let ref_ids: Vec<u32> = teil.split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect();
        if zuletzt != ref_ids {
            eprintln!("[hoeren] andere Token als die Referenz:\n  eigen    {zuletzt:?}\n  Referenz {ref_ids:?}");
            std::process::exit(1);
        }
        eprintln!("[hoeren] Token gleich der Referenz");
    }
}

fn fehler(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(2);
}
