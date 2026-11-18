//! ⚑ **Ein neu besetzter Pod verliert nicht, was er in der Epoche schon
//! gerechnet hat** (2026-11-17, Restpunkt 9 zu Fund 706). Kommt eine Runde
//! leer zurück, baut der Ortsdienst einen neuen Koordinator; der erbt die
//! abgeschlossenen Segmente des alten derselben Epoche.

use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string());
    std::path::PathBuf::from(m).join("../../INTEGER_LLM/artifacts").join(modell)
}

/// Ein Koordinator mit vier Shards im Prozess, für `epoche`, und `teile`
/// Shards (vier oder zwei, damit sich die Zuschnitte unterscheiden lassen).
fn koordinator(dir: &std::path::Path, epoche: u64, teile: usize) -> (Coordinator, Vec<u32>) {
    let model = load_model(dir).expect("Modell");
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let nl = model.num_layers;
    let model = Arc::new(model);
    let shards = (0..teile)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / teile, nl * (s + 1) / teile, s == 0, s + 1 == teile, model.clone(), sk, 4))
        })
        .collect();
    let ids = tok.encode("Die Hauptstadt von Frankreich ist").iter().map(|t| *t as u32).collect();
    (Coordinator::new(PodId::new([0xAA; 32]), EpochId(epoche), shards), ids)
}

/// Der Neue erbt die Segmente des Alten und baut danach **dasselbe Bündel**
/// wie ein Koordinator, der beide Sitzungen allein gerechnet hat: dieselbe
/// Wurzel, dieselbe Ausgabenwurzel, dieselben Sitzungen, dieselben vTFE.
/// ⚑ Gegenprobe im Test: Ohne das Erben fehlt dem Neuen die erste Sitzung,
/// und seine Wurzel ist eine andere (so ging es bis zum 2026-11-17).
#[test]
fn ein_neu_besetzter_koordinator_erbt_die_segmente_seiner_epoche() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (ganz, ids) = koordinator(&dir, 3, 4);
    let _ = ganz.run_prompt(7, &ids, 2);
    let _ = ganz.run_prompt(300, &ids, 2);
    let erwartet = ganz.build_poi_bundle().expect("Bündel");

    let (alt, _) = koordinator(&dir, 3, 4);
    let _ = alt.run_prompt(7, &ids, 2);
    let (mut neu, _) = koordinator(&dir, 3, 4);
    let (ohne, _) = koordinator(&dir, 3, 4);
    assert_eq!(neu.segmente_erben(&alt), Ok(alt.segmentzahl()));
    let _ = neu.run_prompt(300, &ids, 2);
    let _ = ohne.run_prompt(300, &ids, 2);

    let b = neu.build_poi_bundle().expect("Bündel");
    assert_eq!(b.segmente, erwartet.segmente);
    assert_eq!(b.segments_root, erwartet.segments_root);
    assert_eq!(b.ausgabenwurzel, erwartet.ausgabenwurzel);
    assert_eq!(b.sitzungen, erwartet.sitzungen);
    assert_eq!(b.vtfe_claimed, erwartet.vtfe_claimed);

    let g = ohne.build_poi_bundle().expect("Bündel");
    assert!(g.segmente < erwartet.segmente, "Gegenprobe: ohne Erben fehlt die erste Sitzung");
    assert_ne!(g.segments_root, erwartet.segments_root);
}

/// Geerbt wird nur in derselben Epoche und mit denselben Zuschnitten; sonst
/// bleibt der Neue, wie er war.
#[test]
fn fremde_epoche_oder_anderer_zuschnitt_wird_nicht_geerbt() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (alt, ids) = koordinator(&dir, 3, 4);
    let _ = alt.run_prompt(7, &ids, 2);
    assert!(alt.segmentzahl() > 0);

    let (mut spaeter, _) = koordinator(&dir, 4, 4);
    assert!(spaeter.segmente_erben(&alt).is_err(), "andere Epoche");
    assert_eq!(spaeter.segmentzahl(), 0);

    let (mut anders, _) = koordinator(&dir, 3, 2);
    assert!(anders.segmente_erben(&alt).is_err(), "andere Zuschnitte");
    assert_eq!(anders.segmentzahl(), 0);
}
