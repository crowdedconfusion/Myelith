//! Ein Artefakt aus einem trainierten Stand rechnet, was das Training
//! zuletzt gerechnet hat.
//!
//! # ⚑ Die eine Frage
//!
//! Das Training schickt jeden Master durch eine Umrechnung in die
//! Uebertragungsform und rechnet damit vorwaerts. **Laedt man das
//! geschriebene Artefakt und laesst die Inferenz laufen, kommt derselbe
//! Residualstrom heraus?** Wenn nicht, gilt keine Messung des
//! Trainingswerkzeugs fuer das Modell, das ausgeliefert wird.
//!
//! Beide Formen stehen auf dem Pruefstand: int8 und ternaer gepackt. Der
//! Lader prueft dabei jede Pruefsumme des neuen Manifests.

use integer_llm_kernels::trainingsschritt::Gewichtsform;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::Zwischenwerte;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::shardtraining::{
    stand_lesen, stand_schreiben, vorwaerts, Shardgewichte, Shardvorgaben,
};
use integer_llm_runtime::standartefakt::{bereich_des_standes, stand_ins_artefakt};

fn artefakte() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/myelith-0.6b")
}

fn modell() -> Option<IntegerModel> {
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt)");
        return None;
    }
    let dir = artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, Test uebersprungen: {}", dir.display());
        return None;
    }
    Some(load_model(&dir).expect("Modell laedt"))
}

const FOLGE: [u32; 6] = [9707, 374, 264, 1273, 315, 279];

/// Der Residualstrom hinter der letzten Ebene, wie die Inferenz ihn
/// rechnet, und der am Eingang von Ebene `von`.
fn inferenz(m: &IntegerModel, von: usize) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    let mut aus = Vec::new();
    for (pos, tid) in FOLGE.iter().enumerate() {
        let start = m.embed_token(*tid as usize);
        aus.push(m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf));
    }
    let ein = (0..FOLGE.len()).map(|p| auf.ebenen()[p * m.num_layers + von].residual_ein.clone()).collect();
    (aus, ein)
}

/// Ein Zielverzeichnis, das es noch nicht gibt.
fn ziel(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("myl-standartefakt-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Bewegt jeden Master des Bereichs, deterministisch und deutlich: ein
/// Stand, wie ihn ein Lauf hinterlaesst, nicht der des Artefakts.
fn bewegen(g: &mut Shardgewichte) {
    for stand in g.master.iter_mut() {
        for (k, mat) in stand.matrizen_veraenderlich().into_iter().enumerate() {
            for (i, w) in mat.iter_mut().enumerate() {
                let z = (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(k as u64) >> 40;
                *w = w.saturating_add((z % (1 << 17)) as i32 - (1 << 16));
            }
        }
    }
}

fn pruefen(form: Gewichtsform, name: &str) {
    let Some(m) = modell() else { return };
    let von = m.num_layers - 2;
    let mut g = Shardgewichte::aus_modell(&m, von, m.num_layers).expect("Gewichte");
    g.form_setzen(form).expect("Form");
    bewegen(&mut g);

    let (vorher, strom) = inferenz(&m, von);
    let v = Shardvorgaben { von, bis: m.num_layers, schritt: 0, lr_zaehler: 1, lr_nenner: 1 << 12 };
    let training = vorwaerts(&m, &mut g, &v, &strom).expect("vorwaerts").ausgang;
    // Gegenprobe: Der bewegte Stand rechnet etwas anderes als die Quelle.
    // Sonst bestuende der Vergleich unten auch mit einer Kopie der Quelle.
    assert_ne!(training, vorher, "der bewegte Stand rechnet wie die Quelle; der Test prueft nichts");

    let d = ziel(name);
    let b = stand_ins_artefakt(&artefakte(), &m, &g, &d).expect("schreiben");
    assert_eq!(b.matrizen, 14);
    assert_eq!(b.form, form);

    let neu = load_model(&d).expect("das neue Artefakt laedt, mit allen Pruefsummen");
    for e in von..m.num_layers {
        assert_eq!(neu.layers[e].ist_ternaer(), form == Gewichtsform::Ternaer, "Ebene {e}");
    }
    assert!(!neu.layers[von - 1].ist_ternaer(), "eine Ebene vor dem Bereich ist angefasst");
    let (nachher, _) = inferenz(&neu, von);
    for (p, (a, b)) in training.iter().zip(nachher.iter()).enumerate() {
        assert_eq!(a, b, "Position {p}: das Artefakt rechnet nicht, was das Training gerechnet hat");
    }
    assert_eq!(training.len(), nachher.len());

    // ⛔️ Fund 510: Wieder ins Training geladen, rechnet das Artefakt
    //   dasselbe. Einfach entpackt schrumpfte jede ternaere Gruppe mit
    //   Nullen vor dem ersten Schritt.
    let mut wieder = Shardgewichte::aus_modell(&neu, von, neu.num_layers).expect("Gewichte aus dem Artefakt");
    let v2 = Shardvorgaben { von, bis: neu.num_layers, schritt: 0, lr_zaehler: 1, lr_nenner: 1 << 12 };
    let erneut = vorwaerts(&neu, &mut wieder, &v2, &strom).expect("vorwaerts").ausgang;
    assert_eq!(erneut, training, "wieder ins Training geladen rechnet das Artefakt etwas anderes");

    // Ein zweites Mal in dasselbe Ziel: abgewiesen, nichts ueberschrieben.
    assert!(stand_ins_artefakt(&artefakte(), &m, &g, &d).is_err());
    // Und die Quelle ist unberuehrt: Sie laedt weiter mit ihren Pruefsummen.
    let (quelle_danach, _) = inferenz(&load_model(&artefakte()).expect("Quelle laedt"), von);
    assert_eq!(quelle_danach, vorher, "die Quelle hat sich geaendert");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ein_int8_stand_wird_ein_artefakt_das_gleich_rechnet() {
    pruefen(Gewichtsform::Int8, "int8");
}

#[test]
fn ein_ternaerer_stand_wird_ein_gepacktes_artefakt_das_gleich_rechnet() {
    pruefen(Gewichtsform::Ternaer, "ternaer");
}

#[test]
fn der_bereich_steht_in_der_standdatei() {
    let Some(m) = modell() else { return };
    let von = m.num_layers - 3;
    let g = Shardgewichte::aus_modell(&m, von, m.num_layers - 1).expect("Gewichte");
    let d = std::env::temp_dir().join(format!("myl-standbereich-{}.bin", std::process::id()));
    stand_schreiben(&g, &d).expect("schreiben");
    assert_eq!(bereich_des_standes(&d).expect("lesen"), (von, m.num_layers - 1));
    let mut h = Shardgewichte::aus_modell(&m, von, m.num_layers - 1).expect("Gewichte");
    stand_lesen(&mut h, &d).expect("der Bereich aus dem Kopf passt zum Stand");
    std::fs::write(&d, b"etwas anderes, lang genug").expect("schreiben");
    assert!(bereich_des_standes(&d).is_err());
    let _ = std::fs::remove_file(&d);
}
