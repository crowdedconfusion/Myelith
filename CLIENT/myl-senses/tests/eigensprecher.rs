//! **Qwen3-TTS als Sprechweg, ueber den echten Vorleser** (CLIENT 8.5).
//!
//! Zwei Saetze durch [`myl_senses::sprechen::Vorleser`] mit dem Weg
//! `Sprechweg::Eigen`: ohne Stimmprobe und mit der Stimmprobe der Proben
//! samt Text (die Stimme des Modells, kein Mensch). Gestroemt: Jedes Stueck
//! muss gekennzeichnet sein, und der eigene whisper hoert alle zusammen
//! woertlich zurueck.
//! Dazu der Einmalaufruf [`myl_senses::sprechen::sagen`]. Fester Seed
//! ueber `MYL_SPRECHEN_SEED`, damit die Probe nicht am Zufall haengt
//! (allein in dieser Datei, die Umgebung teilen alle Proben eines
//! Prozesses). Mit `#[ignore]`:
//! braucht `MODELS/audio/qwen3-tts-0.6b-base` und
//! `MODELS/audio/whisper-large-v3-turbo`; in `grosse_proben`.

use myl_senses::laufwerk::{Sprechweg, Sprechzeug};
use std::sync::{Arc, Mutex};

const SAETZE: [&str; 2] = ["Der Bericht über die Nacht liegt bereit.", "Morgen um halb neun treffen wir uns am Bahnhof."];

fn audio() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio")
}

fn woerter(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Spricht die Saetze ueber den Vorleser und gibt die gespielten Tondateien zurueck.
fn vorlesen(zeug: &Sprechzeug) -> Vec<Vec<u8>> {
    let gespielt: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let sammler = Arc::clone(&gespielt);
    let mut v = myl_senses::sprechen::Vorleser::neu_mit(
        zeug,
        Box::new(move |wav: &std::path::Path| {
            sammler.lock().expect("Sammler").push(std::fs::read(wav).map_err(|f| f.to_string())?);
            Ok(())
        }),
    );
    v.schub(&format!("{} {} ", SAETZE[0], SAETZE[1]));
    let fehler = v.abschliessen();
    assert!(fehler.is_empty(), "Fehler beim Vorlesen: {fehler:?}");
    Arc::try_unwrap(gespielt).expect("allein").into_inner().expect("Sammler")
}

/// Jedes Stueck gekennzeichnet; alle hintereinander werden als `saetze`
/// verstanden.
fn pruefen(gespielt: &[Vec<u8>], saetze: &[&str], wie: &str) {
    assert!(!gespielt.is_empty(), "{wie}: kein Ton");
    let mut proben = Vec::new();
    let mut rate = 0;
    for b in gespielt {
        let datei = myl_senses::zwischenname("eigensprecher-probe", "wav");
        std::fs::write(&datei, b).expect("schreiben");
        let befund = myl_senses::kennzeichnung::pruefen(&datei).expect("Kennzeichnung lesbar");
        let _ = std::fs::remove_file(&datei);
        assert!(befund.metadaten && befund.wasserzeichen(), "{wie}: ein Stueck nicht gekennzeichnet: {befund:?}");
        assert_eq!(befund.herkunft, myl_senses::kennzeichnung::Herkunft::Gueltig, "{wie}: ein Stueck ohne gueltiges C2PA-Manifest");
        let ton = myl_senses::ton::lesen(b).expect("WAV");
        rate = ton.rate;
        proben.extend(ton.proben);
    }
    let ton = myl_senses::ton::Ton { rate, proben };
    let text = myl_senses::whisper::text(&audio().join("whisper-large-v3-turbo"), &myl_senses::ton::umtasten(&ton, 16_000).proben, "de").expect("whisper");
    eprintln!("[eigensprecher] {wie}: {} Stuecke, {:.1} s, zurueckgehoert: {text}", gespielt.len(), ton.proben.len() as f64 / f64::from(ton.rate));
    assert_eq!(woerter(&text), woerter(&saetze.join(" ")), "{wie}: anders verstanden");
}

#[test]
#[ignore = "braucht MODELS/audio/qwen3-tts-0.6b-base (2,3 GB) und whisper-large-v3-turbo (1,5 GB)"]
fn der_vorleser_spricht_mit_dem_eigenen_weg() {
    // ⚑ Ein fester Seed, sonst haengt die Probe am Zufall: Mit frischen
    // Seeds hoerte whisper in einem von zwei Laeufen einen Satz anders.
    std::env::set_var(myl_senses::eigensprecher::SEED_UMGEBUNG, "1");
    let ordner = audio().join("qwen3-tts-0.6b-base");
    assert!(ordner.join("model.safetensors").is_file(), "Die Gewichte fehlen: {ordner:?}");
    let ohne = Sprechzeug { weg: Sprechweg::Eigen { ordner: ordner.clone() }, probe: None, probentext: None };
    assert!(ohne.dauerhaft() && ohne.kann_klonen());
    let t = std::time::Instant::now();
    let gespielt = vorlesen(&ohne);
    // ⚑ Gestroemt: mehr Stuecke als Saetze (je Stueck 12 Bilder, die Saetze
    // haben ueber 24).
    assert!(gespielt.len() > SAETZE.len(), "nicht gestroemt: {} Stuecke", gespielt.len());
    pruefen(&gespielt, &SAETZE, "ohne Stimmprobe");
    eprintln!("[eigensprecher] ohne Stimmprobe in {:.1} s", t.elapsed().as_secs_f64());

    let text = myl_senses::zwischenname("eigensprecher-probentext", "txt");
    std::fs::write(&text, "Guten Morgen, der Bericht über die Nacht liegt bereit.").expect("Probentext");
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/qwen3tts-stimmprobe.wav");
    let geklont = Sprechzeug { weg: Sprechweg::Eigen { ordner: ordner.clone() }, probe: Some(probe), probentext: Some(text.clone()) };
    let t = std::time::Instant::now();
    pruefen(&vorlesen(&geklont), &SAETZE, "mit Mitschrift geklont");
    eprintln!("[eigensprecher] geklont in {:.1} s", t.elapsed().as_secs_f64());

    // Die Saetze zum Ueberbruecken: vorbereitet ueber den geteilten
    // Dauersprecher, abgelegt in einer Heimat im Zwischenordner (nicht in
    // der des Nutzers), gekennzeichnet, verstanden, und der Vorleser spielt
    // einen davon.
    let heimat = myl_senses::zwischenname("eigensprecher-heimat", "d");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    std::env::set_var(myl_senses::laufwerk::HEIMAT_UMGEBUNG, &heimat);
    let geteilt: myl_senses::sprechen::Geteilter = Arc::default();
    let n = myl_senses::sprechen::ueberbrueckungen_vorbereiten(&ohne, &geteilt, "de").expect("vorbereiten");
    let saetze = myl_senses::sprechen::ueberbrueckungen("de");
    assert_eq!(n, saetze.len(), "nicht jeder Satz vorbereitet");
    let erster = std::fs::read(myl_senses::sprechen::ueberbrueckungsort(&ohne, saetze[0])).expect("abgelegt");
    pruefen(&[erster], &saetze[..1], "ueberbruecken");
    let gespielt: Arc<Mutex<usize>> = Arc::default();
    let zaehler = Arc::clone(&gespielt);
    let mut v = myl_senses::sprechen::Vorleser::neu_geteilt(&ohne, Box::new(move |_| { *zaehler.lock().expect("Zaehler") += 1; Ok(()) }), Some(geteilt));
    v.ueberbruecken("de");
    assert!(v.abschliessen().is_empty());
    assert_eq!(*gespielt.lock().expect("Zaehler"), 1, "der Vorleser spielt keinen vorbereiteten Satz");
    let _ = std::fs::remove_dir_all(&heimat);

    // Der Einmalaufruf: gekennzeichnet und verstanden.
    let wav = myl_senses::sprechen::sagen(&geklont, SAETZE[0]).expect("sagen");
    let b = std::fs::read(&wav).expect("Tondatei");
    let _ = std::fs::remove_file(&wav);
    let _ = std::fs::remove_file(&text);
    pruefen(&[b], &SAETZE[..1], "sagen");
}
