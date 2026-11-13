//! **Der eigene whisper-Encoder gleicht `transformers`** (CLIENT 8.2).
//!
//! Referenz unter `tests/daten/`: fuer das ganzzahlige Pruefsignal der
//! Spektrumprobe die Zustaende nach Einbettung, Ebene 0, Ebene 1 und am
//! Ausgang (nach 32 Ebenen und LayerNorm), je an den Fenstern 0, 50 und
//! 1499, dazu die Norm aller 1500 Fenster; erzeugt einmal mit `transformers`
//! in `f32` aus den Originalgewichten.
//!
//! Mit `#[ignore]`: Die Probe braucht `MODELS/audio/whisper-large-v3-turbo`
//! (1,5 GB) und `MODELS/audio/whisper-small` (0,9 GB), beide nicht
//! versioniert; sie steht in `grosse_proben` von `.github/ci/kisten.sh`.
//! Fehlen die Gewichte, ist das ein Fehler.

use myl_senses::spektrum::{log_mel, ABTASTRATE};
use myl_senses::whisper::{ebene, einbetten, kodieren, Gewichte};

/// Masse von large-v3-turbo, der Groesse dieser Referenz.
const BREITE: usize = 1280;
const FENSTER: usize = 1500;

const STELLEN: [usize; 3] = [0, 50, 1499];

fn signal() -> Vec<f32> {
    let mut s: u64 = 12345;
    (0..ABTASTRATE)
        .map(|n| {
            s = (s * 1103515245 + 12345) & 0x7FFF_FFFF;
            let rauschen = ((s >> 16) & 0x7FFF) as i64 - 16384;
            let rechteck: i64 = if (n / 18) % 2 == 0 { 8192 } else { -8192 };
            (rauschen.div_euclid(4) + rechteck) as f32 / 32768.0
        })
        .collect()
}

fn f32_datei(name: &str) -> Vec<f32> {
    let roh = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten").join(name)).expect("Referenz");
    let (vier, _) = roh.as_chunks::<4>();
    vier.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

fn datei() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/whisper-large-v3-turbo/model.safetensors");
    assert!(d.is_file(), "Die Gewichte fehlen: {d:?}");
    d
}

/// Die Referenz in `f32`, gegen die `transformers` verglichen wird.
fn gewichte() -> Gewichte {
    Gewichte::laden_genau(&datei()).expect("Gewichte")
}

/// Matrizen in int8, die Buendel in W8A16: der genauere ganzzahlige Weg.
fn ganz() -> Gewichte {
    let g = Gewichte::laden_w8a16(&datei()).expect("Gewichte");
    assert!(g.ganzzahlig());
    g
}

/// Der Weg, der ausgeliefert wird: die Buendel in W8A8.
fn acht() -> Gewichte {
    let g = Gewichte::laden(&datei()).expect("Gewichte");
    assert!(g.ganzzahlig());
    g
}

/// Groesste Abweichung an den Stellen, relativ zum groessten Betrag, und
/// groesste relative Abweichung der Normen.
fn vergleich(stufe: usize, x: &[f32]) -> (f32, f32) {
    let stellen = f32_datei("whisper-encoder-stellen.f32");
    let normen = f32_datei("whisper-encoder-normen.f32");
    let mut d = 0f32;
    let mut gross = 0f32;
    for (s, &t) in STELLEN.iter().enumerate() {
        for i in 0..BREITE {
            let r = stellen[(stufe * 3 + s) * BREITE + i];
            d = d.max((x[t * BREITE + i] - r).abs());
            gross = gross.max(r.abs());
        }
    }
    let mut dn = 0f32;
    for t in 0..FENSTER {
        let n: f32 = x[t * BREITE..][..BREITE].iter().map(|v| v * v).sum::<f32>().sqrt();
        let r = normen[stufe * FENSTER + t];
        dn = dn.max((n - r).abs() / r);
    }
    (d / gross, dn)
}

#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB)"]
fn einbettung_und_zwei_ebenen_gleichen_transformers() {
    let g = gewichte();
    let mel = log_mel(&signal(), 128);
    let mut x = einbetten(&mel, &g);
    for stufe in 0..3 {
        if stufe > 0 {
            ebene(&mut x, &g, stufe - 1);
        }
        let (d, dn) = vergleich(stufe, &x);
        eprintln!("[encoder] Stufe {stufe}: Stellen {d:e} relativ, Normen {dn:e} relativ");
        assert!(d < 1e-4 && dn < 1e-4, "Stufe {stufe} weicht ab: {d} / {dn}");
    }
}

#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB); alle 32 Ebenen"]
fn der_ganze_encoder_gleicht_transformers() {
    let g = gewichte();
    let mel = log_mel(&signal(), 128);
    let t = std::time::Instant::now();
    let x = kodieren(&mel, &g, 32);
    let (d, dn) = vergleich(3, &x);
    eprintln!("[encoder] 32 Ebenen in {:.1} s: Stellen {d:e} relativ, Normen {dn:e} relativ", t.elapsed().as_secs_f64());
    assert!(d < 1e-3 && dn < 1e-3, "der Ausgang weicht ab: {d} / {dn}");
}

/// **Encoder und Decoder zusammen gleichen `transformers` Token fuer
/// Token** (gierig, mit den unterdrueckten Token aus
/// `generation_config.json`), am Pruefsignal; die Referenz steht in
/// `tests/daten/whisper-decoder-pruefsignal.json`.
#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB)"]
fn der_decoder_waehlt_dieselben_token() {
    decoder_wie_transformers(&gewichte());
}

/// **Der ganzzahlige Encoder bleibt nah an `transformers`.**
///
/// ⚑ Die Schranke ist eine andere als beim genauen Weg: Gewichte in zwei
/// int8-Stellen, Eingaben in int16 je Fenster, Ausgaben in int16 je Kanal.
/// Gemessen am 2026-11-11: 5,8·10⁻⁴ an den Stellen, 1,6·10⁻² bei den Normen;
/// mit nur einer Stelle waeren es 3,3·10⁻² und 2,7·10⁻¹, mit einer Skala fuer
/// alle Fenster und Kanaele 1,4·10⁻² und 1,2·10⁻¹. Die Schranke liegt beim
/// Dreifachen des Gemessenen. Ob es genuegt, entscheiden die Token (die
/// naechste Probe).
#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB); alle 32 Ebenen"]
fn der_ganzzahlige_encoder_bleibt_nah() {
    let g = ganz();
    let mel = log_mel(&signal(), 128);
    let t = std::time::Instant::now();
    let x = kodieren(&mel, &g, 32);
    let (d, dn) = vergleich(3, &x);
    eprintln!("[encoder int8] 32 Ebenen in {:.1} s: Stellen {d:e} relativ, Normen {dn:e} relativ", t.elapsed().as_secs_f64());
    assert!(d < 2e-3 && dn < 5e-2, "der Ausgang weicht ab: {d} / {dn}");
}

/// **Der ganzzahlige Weg waehlt dieselben Token wie `transformers`.**
#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB)"]
fn der_ganzzahlige_decoder_waehlt_dieselben_token() {
    decoder_wie_transformers(&ganz());
}

/// **W8A8, die Vorgabe, bleibt in der Naehe von `transformers`.**
///
/// Gemessen am 2026-11-11: 8,0·10⁻² an den Stellen, 0,55 bei den Normen
/// (W8A16: 4,9·10⁻⁴ und 1,5·10⁻²); die Schranke liegt beim Dreifachen. Was
/// zaehlt, sind die Token (naechste Probe) und die Wortgenauigkeit.
/// ⚑ Gegenprobe nach unten: Laege die Abweichung unter 10⁻², rechnete hier
/// nicht W8A8, sondern unbemerkt der genauere Weg.
#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB); alle 32 Ebenen"]
fn der_w8a8_encoder_bleibt_in_der_naehe() {
    let g = acht();
    let x = kodieren(&log_mel(&signal(), 128), &g, 32);
    let (d, dn) = vergleich(3, &x);
    eprintln!("[encoder w8a8] Stellen {d:e} relativ, Normen {dn:e} relativ");
    assert!(d < 0.25 && dn < 1.7, "der Ausgang weicht ab: {d} / {dn}");
    assert!(d > 1e-2, "so nah an der Referenz rechnet nicht W8A8: {d}");
}

/// **W8A8 waehlt dieselben Token wie `transformers`**; die Logits duerfen um
/// 30 Prozent abweichen (gemessen 5,12 statt 4,573, also 12 Prozent).
#[test]
#[ignore = "braucht MODELS/audio/whisper-large-v3-turbo (1,5 GB)"]
fn der_w8a8_decoder_waehlt_dieselben_token() {
    decoder_mit_schranke(&acht(), 0.3);
}

fn decoder_wie_transformers(g: &Gewichte) {
    decoder_mit_schranke(g, 1e-3);
}

fn decoder_mit_schranke(g: &Gewichte, schranke: f64) {
    decoder_gegen(g, "whisper-large-v3-turbo", "whisper-decoder-pruefsignal.json", schranke);
}

/// Gierig mitschreiben am Pruefsignal und mit der Referenz von
/// `transformers` in `tests/daten/<referenz>` vergleichen.
fn decoder_gegen(g: &Gewichte, modell: &str, referenz: &str, schranke: f64) {
    use myl_senses::whisper::{mitschreiben, unterdrueckte, DEUTSCH};
    let ordner = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio").join(modell);
    let (u, a) = unterdrueckte(&ordner.join("generation_config.json")).expect("generation_config");
    let roh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten").join(referenz)).expect("Referenz");
    let zahlen: Vec<f64> = roh.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).filter_map(|s| s.parse().ok()).collect();
    let (ids, logits) = mitschreiben(&log_mel(&signal(), g.masse().baender), g, DEUTSCH, &u, &a);
    eprintln!("[decoder {modell}] {ids:?} {logits:?}");
    let n = ids.len();
    let ref_ids: Vec<u32> = zahlen[..n].iter().map(|v| *v as u32).collect();
    assert_eq!(ids, ref_ids, "andere Token als transformers");
    for (eigen, r) in logits.iter().zip(&zahlen[n..]) {
        assert!((f64::from(*eigen) - r).abs() < schranke * r.abs().max(1.0), "Logit {eigen} statt {r}");
    }
}

/// **whisper small, die Vorgabe fuer schwache Hardware, gleicht
/// `transformers`** (Masse aus `config.json`, Gewichte in F32, Sondertoken
/// der aelteren Lage). Genau mit der strengen Schranke, W8A8 bei denselben
/// Token. Referenz `tests/daten/whisper-small-pruefsignal.json`.
#[test]
#[ignore = "braucht MODELS/audio/whisper-small (0,9 GB)"]
fn whisper_small_waehlt_dieselben_token() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/whisper-small/model.safetensors");
    assert!(d.is_file(), "Die Gewichte fehlen: {d:?}");
    let genau = Gewichte::laden_genau(&d).expect("Gewichte");
    assert_eq!((genau.masse().breite, genau.masse().ebenen_enc, genau.masse().baender), (768, 12, 80));
    decoder_gegen(&genau, "whisper-small", "whisper-small-pruefsignal.json", 1e-3);
    drop(genau);
    decoder_gegen(&Gewichte::laden(&d).expect("Gewichte"), "whisper-small", "whisper-small-pruefsignal.json", 0.3);
}

