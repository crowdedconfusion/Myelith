//! **Der Sprecher von Qwen3-TTS waehlt dieselben Codes wie die Referenz,
//! und der Satz laesst sich zurueckhoeren** (CLIENT 8.4).
//!
//! Referenz `tests/daten/qwen3tts-sprecher.json`: gierig erzeugt mit der
//! Referenzumsetzung (`f32`, CPU), ohne Sprecherabdruck, also keine Stimme
//! eines Menschen; dazu der letzte Zustand des Vorlaufs. Mit `#[ignore]`:
//! braucht `MODELS/audio/qwen3-tts-0.6b-base` (2,3 GB) und
//! `MODELS/audio/whisper-large-v3-turbo` (1,5 GB); in `grosse_proben`.

use myl_senses::sprecher::{bildgrenze, sprechen, Sprecher, Wahl};
use myl_senses::textzerleger::Textzerleger;
use myl_senses::tonzerleger::{zurueck, Tonzerleger, CODES};

fn liste<T: std::str::FromStr>(roh: &str, schluessel: &str, ende: &str) -> Vec<T> {
    let teil = roh.split(&format!("\"{schluessel}\": [")).nth(1).expect(schluessel).split(ende).next().expect(schluessel);
    teil.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).filter_map(|s| s.parse().ok()).collect()
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB) und whisper-large-v3-turbo (1,5 GB)"]
fn der_sprecher_waehlt_dieselben_codes_und_wird_verstanden() {
    let roh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/qwen3tts-sprecher.json")).expect("Referenz");
    let ids: Vec<u32> = liste(&roh, "ids", "]");
    let ref_codes: Vec<u16> = liste(&roh, "codes", "]],");
    let ref_codes: Vec<[u16; CODES]> = ref_codes.as_chunks::<CODES>().0.to_vec();
    let ref_letzter: Vec<f32> = liste(&roh, "vorlauf_letzter", "]");
    let audio = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio");
    let d = audio.join("qwen3-tts-0.6b-base/model.safetensors");
    assert!(d.is_file(), "Die Gewichte fehlen: {d:?}");
    // Vom Text an: Der eigene Zerleger baut die Vorlage, und sie gleicht der Referenz.
    let satz = "Guten Morgen, der Bericht über die Nacht liegt bereit.";
    let vorlage = Textzerleger::laden(&audio.join("qwen3-tts-0.6b-base")).expect("Zerleger").vorlage(satz);
    assert_eq!(vorlage, ids, "die Vorlage weicht ab");
    let s = Sprecher::laden_genau(&d).expect("Sprecher");
    let t = std::time::Instant::now();
    let (codes, spur) = sprechen(&s, &vorlage, Some(s.sprache("german").expect("Deutsch")), None, 200, Wahl::Gierig);
    eprintln!("[sprecher] {} Bilder in {:.1} s, Vorlauf {}", codes.len(), t.elapsed().as_secs_f64(), spur.vorlauf_laenge);
    assert_eq!(spur.vorlauf_laenge, 9);
    let gross = ref_letzter.iter().fold(0f32, |m, v| m.max(v.abs()));
    let a = spur.vorlauf_letzter.iter().zip(&ref_letzter).fold(0f32, |m, (x, y)| m.max((x - y).abs())) / gross;
    eprintln!("[sprecher] letzter Zustand des Vorlaufs: {a:e}");
    assert!(a < 1e-4, "der Vorlauf weicht ab: {a}");
    assert_eq!(codes, ref_codes, "andere Codes als die Referenz");
    // Zurueckhoeren: Ton ueber den eigenen Tonzerleger, Text ueber den eigenen whisper.
    let z = Tonzerleger::laden_genau(&audio.join("qwen3-tts-0.6b-base/speech_tokenizer/model.safetensors")).expect("Tonzerleger");
    let ton = myl_senses::ton::Ton { rate: myl_senses::tonzerleger::RATE, proben: zurueck(&codes, &z) };
    let text = myl_senses::whisper::text(&audio.join("whisper-large-v3-turbo"), &myl_senses::ton::umtasten(&ton, 16_000).proben, "de").expect("whisper");
    eprintln!("[sprecher] zurueckgehoert: {text}");
    assert_eq!(text, satz);
}

/// ⚑ **Der Weg des Betriebs**: Matrizen in int8, Stichprobe mit Samen 1.
/// Gemessen an 13 Saetzen ohne Wortfehler wie in `f32`, dreimal so schnell;
/// gierig fanden dort elf von dreizehn Saetzen kein Ende. Die Probe hoert
/// den Satz zurueck und verlangt ein Ende vor der [`bildgrenze`].
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB) und whisper-large-v3-turbo (1,5 GB)"]
fn in_int8_mit_zufall_wird_der_satz_verstanden() {
    let audio = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio");
    let d = audio.join("qwen3-tts-0.6b-base");
    let satz = "Morgen um halb neun treffen wir uns am Bahnhof in Freiburg.";
    let ids = Textzerleger::laden(&d).expect("Zerleger").vorlage(satz);
    let s = Sprecher::laden(&d.join("model.safetensors")).expect("Sprecher");
    let (codes, spur) = sprechen(&s, &ids, Some(s.sprache("german").expect("Deutsch")), None, bildgrenze(&ids), Wahl::Zufall(1));
    eprintln!("[sprecher] int8: {} Bilder, Sprecher {:.2} s, Vorhersage {:.2} s", codes.len(), spur.dauer_sprecher.as_secs_f64(), spur.dauer_vorhersage.as_secs_f64());
    assert!(codes.len() < bildgrenze(&ids), "kein Ende");
    let z = Tonzerleger::laden(&d.join("speech_tokenizer/model.safetensors")).expect("Tonzerleger");
    let ton = myl_senses::ton::Ton { rate: myl_senses::tonzerleger::RATE, proben: zurueck(&codes, &z) };
    let text = myl_senses::whisper::text(&audio.join("whisper-large-v3-turbo"), &myl_senses::ton::umtasten(&ton, 16_000).proben, "de").expect("whisper");
    eprintln!("[sprecher] zurueckgehoert: {text}");
    assert_eq!(text, satz);
}

/// ⚑ **Ohne Sprache** (`auto` der Vorlage, so spricht der Vorleser): der
/// Vorlauf mit „nicht denken“ statt Denken und Sprache. Referenz
/// `tests/daten/qwen3tts-sprecher-auto.json`, gierig in `f32`, die ersten
/// 40 Bilder (gierig findet dieser Satz ohne Sprache kein Ende).
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn ohne_sprache_waehlt_der_sprecher_dieselben_codes() {
    let roh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/qwen3tts-sprecher-auto.json")).expect("Referenz");
    let ids: Vec<u32> = liste(&roh, "ids", "]");
    let ref_codes: Vec<u16> = liste(&roh, "codes", "]],");
    let ref_codes: Vec<[u16; CODES]> = ref_codes.as_chunks::<CODES>().0.to_vec();
    assert_eq!(ref_codes.len(), 40);
    let ref_letzter: Vec<f32> = liste(&roh, "vorlauf_letzter", "]");
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base");
    let s = Sprecher::laden_genau(&d.join("model.safetensors")).expect("Sprecher");
    let (codes, spur) = sprechen(&s, &ids, None, None, 40, Wahl::Gierig);
    assert_eq!(spur.vorlauf_laenge, 8, "ohne Sprache eine Stelle kuerzer");
    let gross = ref_letzter.iter().fold(0f32, |m, v| m.max(v.abs()));
    let a = spur.vorlauf_letzter.iter().zip(&ref_letzter).fold(0f32, |m, (x, y)| m.max((x - y).abs())) / gross;
    eprintln!("[sprecher] ohne Sprache, letzter Zustand des Vorlaufs: {a:e}");
    assert!(a < 1e-4, "der Vorlauf weicht ab: {a}");
    assert_eq!(codes, ref_codes, "andere Codes als die Referenz");
    // Gegenprobe: mit Deutsch beginnt dieselbe Vorlage anders.
    let (deutsch, _) = sprechen(&s, &ids, s.sprache("german"), None, 40, Wahl::Gierig);
    assert_ne!(deutsch, ref_codes, "die Sprache aendert nichts");
}

