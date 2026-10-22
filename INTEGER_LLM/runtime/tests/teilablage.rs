//! **Teilablage: ein Shard aus den Dateien seines Bereichs** (Stufe 4 des
//! Dispatchers, P10, Fund 589, 2026-10-22).
//!
//! Ein Ordner hält vom Gewichtsmanifest nur die Dateien eines Bereichs,
//! dazu die kleinen gemeinsamen Dateien; alles als Verweis auf das
//! Probeartefakt. Daraus muss ein Modell für diesen Bereich laden, und seine
//! Ebenen rechnen dasselbe wie im vollen Modell. Ohne Bereich lädt derselbe
//! Ordner nicht, und eine fehlende Datei **im** Bereich bleibt ein Fehler.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::{load_model, load_model_bereich, Ladebereich};

fn artefakt() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/sparrow-0.6b-int8")
}

/// Die Dateien, die ein Bereich vom Manifest braucht, nach demselben
/// Schlüssel wie der Lader.
fn manifestdateien(von: usize, bis: usize, ebenen: usize) -> (BTreeSet<String>, BTreeSet<String>) {
    let text = std::fs::read_to_string(artefakt().join("weights_manifest.json")).expect("Manifest");
    let m: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let (mut alle, mut behalten) = (BTreeSet::new(), BTreeSet::new());
    for (schluessel, e) in m.as_object().expect("Objekt") {
        let ebene = schluessel.strip_prefix("model_layers_").and_then(|r| r.split('_').next()).and_then(|z| z.parse::<usize>().ok());
        let braucht = match ebene {
            Some(i) => von <= i && i < bis,
            None if schluessel.starts_with("model_embed_tokens") => von == 0,
            None => bis == ebenen,
        };
        for (k, v) in e.as_object().expect("Eintrag") {
            if let (true, Some(d)) = (k.ends_with("file"), v.as_str()) {
                alle.insert(d.to_string());
                if braucht {
                    behalten.insert(d.to_string());
                }
            }
        }
    }
    (alle, behalten)
}

/// Ein Ordner mit Verweisen: alles außerhalb des Manifests, vom Manifest
/// nur `behalten`.
#[cfg(unix)]
fn teilordner(ziel: &Path, alle: &BTreeSet<String>, behalten: &BTreeSet<String>) {
    let _ = std::fs::remove_dir_all(ziel);
    std::fs::create_dir_all(ziel).expect("Ordner");
    for e in std::fs::read_dir(artefakt()).expect("Artefakt").flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !alle.contains(&name) || behalten.contains(&name) {
            std::os::unix::fs::symlink(e.path(), ziel.join(&name)).expect("Verweis");
        }
    }
}

#[cfg(unix)]
#[test]
fn ein_bereich_laedt_aus_seinen_dateien_und_rechnet_wie_das_ganze() {
    // Ohne Probeartefakt nur mit ausdrücklicher Erlaubnis (Fund 113):
    // Ein stiller Sprung sähe aus wie ein bestandener Test.
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt)");
        return;
    }
    assert!(artefakt().exists(), "das Probeartefakt fehlt: {}", artefakt().display());
    let voll = load_model(&artefakt()).expect("volles Modell");
    let ebenen = voll.num_layers;
    for (von, bis) in [(0usize, 10usize), (10, ebenen)] {
        let (alle, behalten) = manifestdateien(von, bis, ebenen);
        let ziel = std::env::temp_dir().join(format!("myl-teilablage-{von}-{}", std::process::id()));
        teilordner(&ziel, &alle, &behalten);
        assert!(load_model(&ziel).is_err(), "ohne Bereich lädt ein Teil nicht");
        let teil = load_model_bereich(&ziel, Some(Ladebereich { von, bis, ebenen })).expect("Teil lädt");
        // Dieselbe Eingabe durch den Bereich, im Teil und im Ganzen.
        let eingabe = voll.embed_token(9707);
        let mut c1 = KVCache::for_range(von, bis, voll.num_kv_heads);
        let mut c2 = KVCache::for_range(von, bis, voll.num_kv_heads);
        let a = voll.run_layers(eingabe.clone(), 0, &mut c1, von, bis);
        let b = teil.run_layers(eingabe, 0, &mut c2, von, bis);
        assert_eq!(a, b, "der Bereich {von} bis {bis} rechnet im Teil anders");
        if bis == ebenen {
            assert_eq!(voll.head_logits(&a), teil.head_logits(&b), "der Kopf rechnet im Teil anders");
        } else {
            assert_eq!(voll.embed_token(17), teil.embed_token(17));
        }
        // Eine fehlende Datei im Bereich bleibt ein Fehler.
        let eine = behalten.iter().find(|d| d.starts_with(&format!("model_layers_{von}_"))).expect("eine Datei im Bereich");
        std::fs::remove_file(ziel.join(eine)).expect("entfernen");
        assert!(load_model_bereich(&ziel, Some(Ladebereich { von, bis, ebenen })).is_err(), "eine Lücke im Bereich fiel nicht auf");
        let _ = std::fs::remove_dir_all(&ziel);
    }
}
