//! **Der Sprecherabdruck gleicht der Referenz, und der Sprecher spricht
//! damit dieselben Codes** (CLIENT 8.5).
//!
//! Stimmprobe `tests/daten/qwen3tts-stimmprobe.wav`: der Satz der Probe
//! `sprecher`, vom Modell ohne Abdruck gesprochen und auf 16 Bit gerundet,
//! also die Stimme des Modells und keines Menschen. Referenz
//! `tests/daten/qwen3tts-abdruck.json`: Mel-Anfang, Abdruck und die
//! gierigen Codes im Modus nur mit Abdruck, aus dem Paket `qwen-tts`
//! (`f32`, CPU). Mit `#[ignore]`: braucht `MODELS/audio/qwen3-tts-0.6b-base`;
//! in `grosse_proben`.

use myl_senses::sprecher::{sprechen, Sprecher, Stimme, Wahl};
use myl_senses::stimmabdruck::{mel, Stimmabdruck, BREITE, RATE};
use myl_senses::textzerleger::Textzerleger;
use myl_senses::tonzerleger::CODES;

fn liste<T: std::str::FromStr>(roh: &str, schluessel: &str, ende: &str) -> Vec<T> {
    let teil = roh.split(&format!("\"{schluessel}\": [")).nth(1).expect(schluessel).split(ende).next().expect(schluessel);
    teil.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).filter_map(|s| s.parse().ok()).collect()
}

fn zahl(roh: &str, schluessel: &str) -> usize {
    let teil = roh.split(&format!("\"{schluessel}\": ")).nth(1).expect(schluessel);
    teil.split(|c: char| !c.is_ascii_digit()).next().and_then(|s| s.parse().ok()).expect(schluessel)
}

/// Die Schranke fuer Mel und Abdruck, bezogen auf den groessten Betrag.
const SCHRANKE: f32 = 1e-4;

fn daten(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten").join(name)
}

/// Groesste Abweichung, bezogen auf den groessten Betrag der Referenz.
fn abstand(a: &[f32], r: &[f32]) -> f32 {
    assert_eq!(a.len(), r.len());
    let gross = r.iter().fold(0f32, |m, v| m.max(v.abs()));
    a.iter().zip(r).fold(0f32, |m, (x, y)| m.max((x - y).abs())) / gross
}

fn stimmprobe() -> Vec<f32> {
    let ton = myl_senses::ton::lesen(&std::fs::read(daten("qwen3tts-stimmprobe.wav")).expect("Stimmprobe")).expect("WAV");
    assert_eq!(ton.rate, RATE);
    ton.proben
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn der_abdruck_gleicht_der_referenz_und_spricht_dieselben_codes() {
    let roh = std::fs::read_to_string(daten("qwen3tts-abdruck.json")).expect("Referenz");
    let proben = stimmprobe();
    assert_eq!(proben.len(), zahl(&roh, "proben"));
    // Das Mel: die ersten vier Bilder, in der Referenz je Bild.
    let (m, bilder) = mel(&proben);
    assert_eq!(bilder, zahl(&roh, "mel_bilder"));
    let anfang: Vec<f32> = (0..4).flat_map(|t| (0..128).map(move |b| (b, t))).map(|(b, t)| m[b * bilder + t]).collect();
    let a = abstand(&anfang, &liste(&roh, "mel_anfang", "]"));
    eprintln!("[abdruck] Mel-Anfang: {a:e}");
    assert!(a < SCHRANKE, "das Mel weicht ab: {a}");

    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base");
    assert!(d.join("model.safetensors").is_file(), "Die Gewichte fehlen: {d:?}");
    let s = Stimmabdruck::laden(&d.join("model.safetensors")).expect("Abdruck");
    let t = std::time::Instant::now();
    let abdruck = s.abdruck(&proben);
    eprintln!("[abdruck] {:.2} s fuer {:.1} s Ton", t.elapsed().as_secs_f64(), proben.len() as f64 / f64::from(RATE));
    assert_eq!(abdruck.len(), BREITE);
    let referenz: Vec<f32> = liste(&roh, "abdruck", "]");
    let a = abstand(&abdruck, &referenz);
    eprintln!("[abdruck] Abdruck: {a:e}");
    assert!(a < SCHRANKE, "der Abdruck weicht ab: {a}");

    // Mit dem Abdruck sprechen: dieselben Codes wie die Referenz.
    let satz = "Guten Morgen, der Bericht über die Nacht liegt bereit.";
    let ids = Textzerleger::laden(&d).expect("Zerleger").vorlage(satz);
    let sp = Sprecher::laden_genau(&d.join("model.safetensors")).expect("Sprecher");
    let (codes, spur) = sprechen(&sp, &ids, Some(sp.sprache("german").expect("Deutsch")), Some(&Stimme { abdruck, vorbild: None }), 200, Wahl::Gierig);
    assert_eq!(spur.vorlauf_laenge, 10, "der Abdruck steht als eigene Stelle im Vorlauf");
    let ref_codes: Vec<u16> = liste(&roh, "codes", "]]");
    assert_eq!(codes, ref_codes.as_chunks::<CODES>().0.to_vec(), "andere Codes als die Referenz");
}

/// ⚑ **Gegenprobe**: Eine um eine Zehntelsekunde versetzte Stimmprobe
/// ergibt einen anderen Abdruck, um mehr als das Zehnfache der Schranke.
/// Sonst hinge die Probe nicht am Ton. 📌 Viel mehr ist nicht zu erwarten:
/// Ein Abdruck soll dieselbe Stimme wiedererkennen, und versetzt bleibt sie
/// dieselbe (gemessen 1·10⁻²).
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn ein_anderer_ton_gibt_einen_anderen_abdruck() {
    let roh = std::fs::read_to_string(daten("qwen3tts-abdruck.json")).expect("Referenz");
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base/model.safetensors");
    let s = Stimmabdruck::laden(&d).expect("Abdruck");
    let proben = stimmprobe();
    let a = abstand(&s.abdruck(&proben[2_400..]), &liste(&roh, "abdruck", "]"));
    eprintln!("[abdruck] versetzt: {a:e}");
    assert!(a > 10.0 * SCHRANKE, "der Abdruck merkt den Versatz nicht: {a}");
}
