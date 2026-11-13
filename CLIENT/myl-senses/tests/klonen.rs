//! **Klonen mit Mitschrift**: Der Hinweg des Tonzerlegers macht aus der
//! Stimmprobe dieselben Codes wie die Referenz, und der Sprecher setzt die
//! Stimmprobe mit einem neuen Satz fort wie die Referenz (CLIENT 8.5).
//!
//! Stimmprobe `tests/daten/qwen3tts-stimmprobe.wav` (die Stimme des Modells,
//! siehe Probe `stimmabdruck`). Referenz `tests/daten/qwen3tts-hinweg.json`
//! aus dem Paket `qwen-tts` (`f32`, CPU), gierig. Mit `#[ignore]`: braucht
//! `MODELS/audio/qwen3-tts-0.6b-base` und zum Zurueckhoeren
//! `MODELS/audio/whisper-large-v3-turbo`; in `grosse_proben`.

use myl_senses::sprecher::{sprechen, Sprecher, Stimme, Wahl};
use myl_senses::stimmabdruck::Stimmabdruck;
use myl_senses::textzerleger::Textzerleger;
use myl_senses::tonzerleger::{hin, hin_mit, zurueck, zurueck_hinter, Hinweg, Tonzerleger, CODES};

fn liste<T: std::str::FromStr>(roh: &str, schluessel: &str, ende: &str) -> Vec<T> {
    let teil = roh.split(&format!("\"{schluessel}\": [")).nth(1).expect(schluessel).split(ende).next().expect(schluessel);
    teil.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).filter_map(|s| s.parse().ok()).collect()
}

fn daten(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten").join(name)
}

fn modell() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base");
    assert!(d.join("model.safetensors").is_file(), "Die Gewichte fehlen: {d:?}");
    d
}

fn stimmprobe() -> Vec<f32> {
    myl_senses::ton::lesen(&std::fs::read(daten("qwen3tts-stimmprobe.wav")).expect("Stimmprobe")).expect("WAV").proben
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn der_hinweg_trifft_die_codes_der_referenz() {
    let roh = std::fs::read_to_string(daten("qwen3tts-hinweg.json")).expect("Referenz");
    let referenz: Vec<u16> = liste(&roh, "codes_hin", "]],");
    let referenz = referenz.as_chunks::<CODES>().0.to_vec();
    let h = Hinweg::laden(&modell().join("speech_tokenizer/model.safetensors")).expect("Hinweg");
    // Mit MYL_HINWEG_STUFEN zeigt die Probe den Abstand je Zwischenstufe.
    let stufen = std::env::var_os("MYL_HINWEG_STUFEN").map(std::path::PathBuf::from);
    let t = std::time::Instant::now();
    let codes = hin_mit(&stimmprobe(), &h, |name, werte| {
        let Some(o) = &stufen else { return };
        let Ok(b) = std::fs::read(o.join(format!("hin-{name}.f32"))) else { return };
        let r: Vec<f32> = b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
        let gross = r.iter().fold(0f32, |m, v| m.max(v.abs()));
        let a = werte.iter().zip(&r).fold(0f32, |m, (x, y)| m.max((x - y).abs())) / gross;
        eprintln!("[hinweg] {name}: {} gegen {} Werte, {a:e}", werte.len(), r.len());
    });
    eprintln!("[hinweg] {} Bilder in {:.2} s", codes.len(), t.elapsed().as_secs_f64());
    assert_eq!(codes.len(), referenz.len());
    let falsch: Vec<(usize, usize)> = (0..codes.len()).flat_map(|t| (0..CODES).map(move |c| (t, c))).filter(|&(t, c)| codes[t][c] != referenz[t][c]).collect();
    eprintln!("[hinweg] abweichende Codes: {} von {}", falsch.len(), codes.len() * CODES);
    assert!(falsch.is_empty(), "abweichend (Bild, Code): {falsch:?}");
}

/// ⚑ **Gegenprobe**: Um ein halbes Bild (960 Proben) versetzt trifft der
/// Hinweg die Referenz nicht mehr; die Probe oben haengt also am Ton und
/// nicht an einer Eigenschaft der Codebuecher.
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn versetzt_trifft_der_hinweg_die_referenz_nicht() {
    let roh = std::fs::read_to_string(daten("qwen3tts-hinweg.json")).expect("Referenz");
    let referenz: Vec<u16> = liste(&roh, "codes_hin", "]],");
    let h = Hinweg::laden(&modell().join("speech_tokenizer/model.safetensors")).expect("Hinweg");
    let codes: Vec<u16> = hin(&stimmprobe()[960..], &h).concat();
    let gleich = codes.iter().zip(&referenz).filter(|(a, b)| a == b).count();
    eprintln!("[hinweg] versetzt gleich: {gleich} von {}", codes.len());
    assert!(gleich * 4 < codes.len(), "versetzt noch {gleich} von {} gleich", codes.len());
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB) und whisper-large-v3-turbo (1,5 GB)"]
fn mit_mitschrift_geklont_spricht_der_sprecher_wie_die_referenz() {
    let roh = std::fs::read_to_string(daten("qwen3tts-hinweg.json")).expect("Referenz");
    let d = modell();
    let proben = stimmprobe();
    let mitschrift = "Guten Morgen, der Bericht über die Nacht liegt bereit.";
    let satz = "Die Prüfrunde lief ohne Fehler durch.";
    let zerleger = Textzerleger::laden(&d).expect("Zerleger");
    let ids = zerleger.vorlage(satz);
    assert_eq!(ids, liste::<u32>(&roh, "ids", "]"), "die Vorlage weicht ab");
    let ref_ids: Vec<u32> = liste(&roh, "ref_ids", "]");
    let m = zerleger.zerlegen(mitschrift);
    assert_eq!(m, ref_ids[3..ref_ids.len() - 2], "die Mitschrift ist anders zerlegt");
    let t = std::time::Instant::now();
    let abdruck = Stimmabdruck::laden(&d.join("model.safetensors")).expect("Abdruck").abdruck(&proben);
    let vorbild = hin(&proben, &Hinweg::laden(&d.join("speech_tokenizer/model.safetensors")).expect("Hinweg"));
    eprintln!("[klonen] Abdruck und Hinweg in {:.2} s", t.elapsed().as_secs_f64());
    let stimme = Stimme { abdruck, vorbild: Some((m, vorbild)) };
    let s = Sprecher::laden_genau(&d.join("model.safetensors")).expect("Sprecher");
    let t = std::time::Instant::now();
    let (codes, spur) = sprechen(&s, &ids, Some(s.sprache("german").expect("Deutsch")), Some(&stimme), 200, Wahl::Gierig);
    eprintln!("[klonen] {} Bilder in {:.1} s, Vorlauf {}", codes.len(), t.elapsed().as_secs_f64(), spur.vorlauf_laenge);
    assert_eq!(spur.vorlauf_laenge, zahl(&roh, "vorlauf_laenge"));
    let ref_letzter: Vec<f32> = liste(&roh, "vorlauf_letzter", "]");
    let gross = ref_letzter.iter().fold(0f32, |m, v| m.max(v.abs()));
    let a = spur.vorlauf_letzter.iter().zip(&ref_letzter).fold(0f32, |m, (x, y)| m.max((x - y).abs())) / gross;
    eprintln!("[klonen] letzter Zustand des Vorlaufs: {a:e}");
    assert!(a < 1e-4, "der Vorlauf weicht ab: {a}");
    let ref_codes: Vec<u16> = liste(&roh, "codes", "]]");
    assert_eq!(codes, ref_codes.as_chunks::<CODES>().0.to_vec(), "andere Codes als die Referenz");
    // Zurueckhoeren: nur der neue Satz, hinter der Stimmprobe.
    let z = Tonzerleger::laden_genau(&d.join("speech_tokenizer/model.safetensors")).expect("Tonzerleger");
    let (_, vorbild) = stimme.vorbild.as_ref().expect("Vorbild");
    let ton = myl_senses::ton::Ton { rate: myl_senses::tonzerleger::RATE, proben: zurueck_hinter(vorbild, &codes, &z) };
    // ⚑ Wie die Vorlage: die ganze Stimmprobe als Zusammenhang, bitgleich
    // mit der Umwandlung beider zusammen.
    let ganz = zurueck(&[vorbild.as_slice(), &codes].concat(), &z);
    assert_eq!(ton.proben, ganz[vorbild.len() * z.proben_je_bild()..], "anders als beide zusammen");
    let audio = d.parent().expect("MODELS/audio").to_path_buf();
    let text = myl_senses::whisper::text(&audio.join("whisper-large-v3-turbo"), &myl_senses::ton::umtasten(&ton, 16_000).proben, "de").expect("whisper");
    eprintln!("[klonen] zurueckgehoert: {text}");
    assert_eq!(text, satz);
}

fn zahl(roh: &str, schluessel: &str) -> usize {
    let teil = roh.split(&format!("\"{schluessel}\": ")).nth(1).expect(schluessel);
    teil.split(|c: char| !c.is_ascii_digit()).next().and_then(|s| s.parse().ok()).expect(schluessel)
}
