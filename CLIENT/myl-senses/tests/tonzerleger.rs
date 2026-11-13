//! **Der Rueckweg des Tonzerlegers gleicht der Referenz** (CLIENT 8.4).
//!
//! Referenz unter `tests/daten/qwen3tts-zerleger-*`: zehn Bilder Codes aus
//! einem festen Generator (`z = z · 1103515245 + 12345 mod 2³¹`, Code
//! `(z >> 16) mod 2048`) und die Werte der Referenzumsetzung in `f32` auf
//! der CPU nach den Codebuechern, nach dem Transformer und am Ende. ⚑ Mit
//! Absicht kein gesprochener Satz: Der Rueckweg nimmt beliebige Codes, und
//! Ton aus einer geklonten Stimme gehoert nicht ins Repositorium. Mit `#[ignore]`: Die Probe braucht
//! `MODELS/audio/qwen3-tts-0.6b-base` (2,3 GB, nicht versioniert); sie steht
//! in `grosse_proben` von `.github/ci/kisten.sh`.

use myl_senses::tonzerleger::{zurueck, zurueck_mit, Rueckstrom, Tonzerleger, CODES};

fn daten(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten").join(format!("qwen3tts-zerleger-{name}"))
}

fn f32_datei(name: &str) -> Vec<f32> {
    let roh = std::fs::read(daten(name)).expect("Referenz");
    roh.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

fn codes() -> Vec<[u16; CODES]> {
    let roh = std::fs::read_to_string(daten("codes.json")).expect("Codes");
    let zahlen: Vec<u16> = roh.split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect();
    zahlen.as_chunks::<CODES>().0.to_vec()
}

/// Groesste Abweichung relativ zum groessten Betrag der Referenz.
fn abstand(eigen: &[f32], referenz: &[f32]) -> f32 {
    assert_eq!(eigen.len(), referenz.len(), "andere Laenge");
    let gross = referenz.iter().fold(0f32, |m, v| m.max(v.abs()));
    eigen.iter().zip(referenz).fold(0f32, |m, (a, b)| m.max((a - b).abs())) / gross
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn der_rueckweg_gleicht_der_referenz() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base/speech_tokenizer/model.safetensors");
    assert!(d.is_file(), "Die Gewichte fehlen: {d:?}");
    let z = Tonzerleger::laden_genau(&d).expect("Gewichte");
    assert_eq!(z.proben_je_bild(), 1920);
    let c = codes();
    let mut befunde = Vec::new();
    let t = std::time::Instant::now();
    let ton = zurueck_mit(&c, &z, |name, werte| {
        if !daten(&format!("{name}.f32")).is_file() {
            return;
        }
        let a = abstand(werte, &f32_datei(&format!("{name}.f32")));
        eprintln!("[zerleger] {name}: {a:e}");
        befunde.push((name.to_string(), a));
    });
    eprintln!("[zerleger] {} Bilder zu {} Proben in {:.2} s", c.len(), ton.len(), t.elapsed().as_secs_f64());
    assert_eq!(befunde.len(), 3, "drei Stufen mit Referenz");
    for (name, a) in befunde {
        // Gemessen am 2026-11-11: bis 2,6·10⁻⁷, 8,8·10⁻⁷ und 8,4·10⁻⁵.
        assert!(a < 3e-4, "{name} weicht ab: {a}");
    }
    // Gegenprobe: Um eine Probe verschoben weicht der Ton weit ab; ein
    // Fehler in der kausalen Polsterung bliebe also nicht unbemerkt.
    let r = f32_datei("ton.f32");
    let verschoben = abstand(&ton[1..], &r[..r.len() - 1]);
    assert!(verschoben > 1e-2, "eine Probe Versatz faellt nicht auf: {verschoben}");
}

/// 120 Bilder aus dem Generator der Referenz, weiter gezaehlt: laenger als
/// das Fenster des Transformers (72), damit der Strom es ueberschreitet.
fn viele_codes() -> Vec<[u16; CODES]> {
    let mut z: u64 = 1;
    (0..120)
        .map(|_| {
            let mut bild = [0u16; CODES];
            for c in &mut bild {
                z = (z * 1_103_515_245 + 12_345) % (1 << 31);
                *c = ((z >> 16) % 2048) as u16;
            }
            bild
        })
        .collect()
}

/// Ton in Stuecken von 1, 2, 7, 30 und 80 Bildern durch einen Strom.
fn gestueckelt(codes: &[[u16; CODES]], z: &Tonzerleger) -> Vec<f32> {
    let mut strom = Rueckstrom::neu();
    let mut ton = Vec::new();
    let mut ab = 0;
    for n in [1, 2, 7, 30, 80] {
        ton.extend(strom.weiter(&codes[ab..ab + n], z));
        ab += n;
    }
    assert_eq!(strom.bilder(), codes.len());
    ton
}

/// ⚑ **Der Strom klingt wie der ganze Satz**: in `f32` bitgleich, gleich
/// wie gestueckelt. Gegenprobe: ein Strom, der sein Gedaechtnis verliert
/// (jedes Stueck neu begonnen), weicht ab.
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn der_strom_ist_bitgleich_mit_dem_ganzen() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base/speech_tokenizer/model.safetensors");
    let z = Tonzerleger::laden_genau(&d).expect("Gewichte");
    let codes = viele_codes();
    let ganz = zurueck(&codes, &z);
    assert_eq!(ganz.len(), codes.len() * z.proben_je_bild());
    assert_eq!(gestueckelt(&codes, &z), ganz, "gestueckelt anders als in einem Zug");
    let vergessen: Vec<f32> = [&codes[..40], &codes[40..]].iter().flat_map(|teil| Rueckstrom::neu().weiter(teil, &z)).collect();
    assert_ne!(vergessen, ganz, "ohne Gedaechtnis kein Unterschied: die Probe saehe nichts");
}

/// In int8 haengen die Ausgabebits davon ab, was ein Aufruf zusammen
/// rechnet; gestueckelt ist der Ton dort nicht bitgleich. ⚑ Gemessen wird
/// deshalb gegen `f32`: Gestueckelt darf int8 nicht weiter davon weg liegen
/// als int8 in einem Zug, bis auf 3 dB.
#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB)"]
fn der_strom_in_int8_bleibt_nah() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base/speech_tokenizer/model.safetensors");
    let codes = viele_codes();
    let genau = zurueck(&codes, &Tonzerleger::laden_genau(&d).expect("Gewichte"));
    let z = Tonzerleger::laden(&d).expect("Gewichte");
    let effektiv = |v: &[f32]| (v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    let db = |a: &[f32]| {
        let rest: Vec<f32> = a.iter().zip(&genau).map(|(x, y)| x - y).collect();
        20.0 * (effektiv(&genau) / effektiv(&rest).max(1e-12)).log10()
    };
    let (ganz, stueck) = (db(&zurueck(&codes, &z)), db(&gestueckelt(&codes, &z)));
    eprintln!("[zerleger] W8A8 gegen f32: in einem Zug {ganz:.1} dB, gestueckelt {stueck:.1} dB");
    assert!(stueck > ganz - 3.0, "gestueckelt weiter weg als in einem Zug: {stueck:.1} gegen {ganz:.1} dB");
    // ⚑ W8A16 mit zwei Stellen, der Weg des Eigensprechers: gemessen 66 dB
    // in einem Zug, 61 dB gestueckelt (die Stuecke unter 16 Bildern rechnen
    // den exakten Weg). 📌 Die erste Schranke, hoechstens 3 dB unter einem
    // Zug, war fuer diese Hoehe zu eng; hoerbar ist beides nicht.
    let z16 = Tonzerleger::laden_w8a16(&d).expect("Gewichte");
    let (ganz16, stueck16) = (db(&zurueck(&codes, &z16)), db(&gestueckelt(&codes, &z16)));
    eprintln!("[zerleger] W8A16 gegen f32: in einem Zug {ganz16:.1} dB, gestueckelt {stueck16:.1} dB");
    assert!(stueck16 > 50.0 && stueck16 > stueck + 20.0, "W8A16 nicht nah genug an f32: {stueck16:.1} dB");
}
