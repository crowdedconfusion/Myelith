//! **Das Log-Mel gleicht dem von whisper** (CLIENT 8.2, erster Baustein).
//!
//! Die Referenz unter `tests/daten/` stammt aus `WhisperFeatureExtractor`
//! (`transformers`) fuer dasselbe Pruefsignal, je fuer 80 und 128 Baender,
//! die ersten 101 Fenster (eine Sekunde Ton), als `f32` in kleiner
//! Bytefolge, Fenster fuer Fenster. Erzeugt einmal mit der Kalibrierumgebung
//! des Projekts; die Probe selbst braucht kein Python.
//!
//! ⚑ **Das Pruefsignal ist ganzzahlig**, damit beide Seiten exakt dieselben
//! Proben sehen: Rauschen aus einem linearen Kongruenzgenerator und ein
//! Rechteck mit 36 Proben Periode, beide geteilt durch 2¹⁵.

use myl_senses::spektrum::{log_mel, ABTASTRATE};

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

fn referenz(baender: usize) -> Vec<f32> {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/daten/whisper-logmel-{baender}.f32"));
    let roh = std::fs::read(&pfad).expect("Referenz lesbar");
    let (vier, rest) = roh.as_chunks::<4>();
    assert!(rest.is_empty(), "Referenz ist keine Folge von f32");
    vier.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

/// Groesste Abweichung gegen die Referenz, ueber alle Baender der ersten
/// 101 Fenster.
fn abweichung(baender: usize, ton: &[f32]) -> f32 {
    let ref_werte = referenz(baender);
    assert_eq!(ref_werte.len(), 101 * baender, "Referenz hat die falsche Groesse");
    let eigen = log_mel(ton, baender);
    let mut hoechst = 0.0f32;
    for t in 0..101 {
        for m in 0..baender {
            hoechst = hoechst.max((eigen.wert(m, t) - ref_werte[t * baender + m]).abs());
        }
    }
    hoechst
}

#[test]
fn das_log_mel_gleicht_whisper_mit_80_und_128_baendern() {
    let ton = signal();
    for baender in [80, 128] {
        let d = abweichung(baender, &ton);
        eprintln!("[spektrum] {baender} Baender: groesste Abweichung {d:e}");
        assert!(d < 2e-5, "{baender} Baender weichen um {d} ab");
    }
}

/// ⛔️ Gegenprobe: Ein um eine Probe verschobenes Signal besteht nicht.
#[test]
fn ein_verschobenes_signal_besteht_nicht() {
    let mut ton = signal();
    ton.rotate_right(1);
    assert!(abweichung(80, &ton) > 1e-2, "die Probe unterscheidet ein verschobenes Signal nicht");
}
