//! **Die Mel-Filter gleichen denen von whisper.cpp** (CLIENT 8.2).
//!
//! whisper.cpp fuehrt die Filter in seiner Gewichtsdatei mit (aus der
//! Vorlage von OpenAI, mit librosa erzeugt). Das ist eine zweite Referenz
//! neben `transformers`, unabhaengig erzeugt. Die Probe liest den Kopf der
//! Datei (`ggml`, Hyperparameter, dann `n_mel`, `n_fft` und die Filter
//! zeilenweise je Band) und vergleicht mit [`myl_senses::spektrum::mel_filter`].
//!
//! ⚑ Mit `#[ignore]`, weil sie `MODELS/audio/hoeren.bin` braucht (rund
//! 550 MB, nicht versioniert); sie steht in `grosse_proben` von
//! `.github/ci/kisten.sh`. Fehlt die Datei, ist das ein Fehler, kein Absprung.

use myl_senses::spektrum::{mel_filter, FREQUENZEN};

#[test]
#[ignore = "braucht MODELS/audio/hoeren.bin (whisper.cpp large-v3-turbo)"]
fn die_mel_filter_gleichen_whisper_cpp() {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/hoeren.bin");
    assert!(pfad.is_file(), "Die Gewichte fehlen: {pfad:?} (sh SYSTEM/install/sinne-einrichten.sh)");
    use std::io::Read;
    let mut kopf = vec![0u8; 4 + 11 * 4 + 8];
    let mut datei = std::fs::File::open(&pfad).expect("lesbar");
    datei.read_exact(&mut kopf).expect("Kopf");
    let zahl = |o: usize| i32::from_le_bytes([kopf[o], kopf[o + 1], kopf[o + 2], kopf[o + 3]]);
    assert_eq!(u32::from_le_bytes([kopf[0], kopf[1], kopf[2], kopf[3]]), 0x6767_6d6c, "keine ggml-Datei");
    let baender = zahl(4 + 9 * 4) as usize;
    let (n_mel, n_fft) = (zahl(48) as usize, zahl(52) as usize);
    assert_eq!((n_mel, n_fft), (baender, FREQUENZEN), "unerwartete Filtergroesse");
    let mut roh = vec![0u8; n_mel * n_fft * 4];
    datei.read_exact(&mut roh).expect("Filter");
    let (werte, _) = roh.as_chunks::<4>();
    let eigen = mel_filter(baender);
    let mut hoechst = 0.0f64;
    let mut groesst = 0.0f64;
    for m in 0..n_mel {
        for k in 0..n_fft {
            let ihr = f64::from(f32::from_le_bytes(werte[m * n_fft + k]));
            hoechst = hoechst.max((ihr - eigen[k * baender + m]).abs());
            groesst = groesst.max(ihr.abs());
        }
    }
    eprintln!("[mel] {baender} Baender: groesste Abweichung {hoechst:e} bei groesstem Wert {groesst:e}");
    // Relativ zum groessten Wert: die Rundung in `f32`, nicht mehr.
    assert!(hoechst < 1e-6 * groesst, "die Filter weichen um {hoechst} ab");
}
