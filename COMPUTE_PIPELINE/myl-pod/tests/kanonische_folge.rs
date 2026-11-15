//! ⚑ **Die Bündelwurzel hängt nicht an der Reihenfolge, in der Sitzungen
//! fertig werden** (2026-11-15). Vorbedingung dafür, dass ein Pod
//! Sitzungen nebeneinander rechnet: Zwei Zwillinge mit derselben Arbeit
//! müssen dasselbe Bündel bauen, gleich in welcher Folge sie fertig wurden.

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
    std::path::PathBuf::from(m)
        .join("../../INTEGER_LLM/artifacts")
        .join(modell)
}

fn koordinator(dir: &std::path::Path) -> (Coordinator, Vec<u32>) {
    let model = load_model(dir).expect("Modell");
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad"))
        .expect("Wortschatz");
    let nl = model.num_layers;
    let model = Arc::new(model);
    let mut shards = Vec::new();
    for s in 0..4usize {
        let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 17; 32]).expect("BLS");
        shards.push(Arc::new(ShardNode::new(
            s,
            nl * s / 4,
            nl * (s + 1) / 4,
            s == 0,
            s == 3,
            model.clone(),
            sk,
            4,
        )));
    }
    let ids: Vec<u32> = tok
        .encode("Die Hauptstadt von Frankreich ist")
        .iter()
        .map(|t| *t as u32)
        .collect();
    (
        Coordinator::new(
            PodId::new([0xAA; 32]),
            EpochId(0),
            shards,
        ),
        ids,
    )
}

/// Zwei Koordinatoren, dieselben zwei Sitzungen, umgekehrte Reihenfolge:
/// dieselbe Bündelwurzel, dieselbe Ausgabenwurzel, dieselbe kanonische Folge.
/// ⚑ Die Gegenprobe steht im Test: Über die rohe Folge des Fertigwerdens
/// gerechnet, sind die Wurzeln verschieden; sonst wählte die Probe nichts aus.
/// Die Sitzungen 7 und 300 trennen zudem Zahl und Bytefolge (300 beginnt
/// als Little Endian mit 0x2C, 7 mit 0x07).
#[test]
fn die_buendelwurzel_haengt_nicht_an_der_reihenfolge_der_sitzungen() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (a, ids) = koordinator(&dir);
    let (b, _) = koordinator(&dir);
    let _ = a.run_prompt(7, &ids, 2);
    let _ = a.run_prompt(300, &ids, 2);
    let _ = b.run_prompt(300, &ids, 2);
    let _ = b.run_prompt(7, &ids, 2);
    let roh = |k: &Coordinator| {
        let z: Vec<myl_types::Segmentzeugnis> =
            k.completed_segments().iter().map(|c| myl_types::Segmentzeugnis { id: c.id, spurwurzel: c.spurwurzel }).collect();
        myl_types::segments_root(&z).expect("Wurzel")
    };
    assert_ne!(roh(&a), roh(&b), "die rohe Folge ist gleich; die Probe waehlt nichts aus");
    let ba = a.build_poi_bundle().expect("Buendel a");
    let bb = b.build_poi_bundle().expect("Buendel b");
    assert_eq!(ba.segments_root, bb.segments_root, "die Buendelwurzel haengt an der Reihenfolge");
    assert_eq!(ba.ausgabenwurzel, bb.ausgabenwurzel, "die Ausgabenwurzel haengt an der Reihenfolge");
    let ka: Vec<_> = a.kanonisch().iter().map(|c| c.id).collect();
    let kb: Vec<_> = b.kanonisch().iter().map(|c| c.id).collect();
    assert_eq!(ka, kb);
    // Nach Zahlen: erst Sitzung 7, dann 300, je Position aufsteigend.
    let folge: Vec<(u64, u64)> = ka.iter().map(myl_types::sitzung_und_position).collect();
    let mut sortiert = folge.clone();
    sortiert.sort_unstable();
    assert_eq!(folge, sortiert);
    assert_eq!(folge.first().map(|f| f.0), Some(7));
}
