//! ⚑ **Eine Wiederholung ohne Vergessen rechnet wie neu** (Fund 709,
//! 2026-11-16).
//!
//! Wiederholt der Koordinator einen Auftrag und erreicht das Vergessen der
//! Sitzung einen Shard nicht, beginnt dieselbe Sitzung dort ein zweites Mal
//! bei Position 0. Aufmerksamkeitsebenen überschreiben dann ihre alten
//! Positionen; eine Zustandsschicht rechnete dagegen auf dem alten Zustand
//! weiter. Im Auftragslauf gab so ein ehrlicher Pod des 35B 24 Token aus,
//! sein Zwilling 47.
//!
//! ⚠️ **Die Gegenprobe beißt nur an einem Modell mit Zustandsschichten**
//! (`MYL_POD_MODELL=raven-35b-a3b-int4`). Am Ankermodell rechnet die zweite
//! Rechnung auch ohne den Fix gleich, weil es nur Aufmerksamkeit hat.

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

fn koordinator(model: &Arc<integer_llm_runtime::model::IntegerModel>) -> Coordinator {
    let nl = model.num_layers;
    let shards = (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 31; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, model.clone(), sk, 4096))
        })
        .collect();
    Coordinator::new(PodId::new([0xCC; 32]), EpochId(0), shards)
}

/// Die Spuren der letzten `n` abgeschlossenen Segmente.
fn letzte_spuren(k: &Coordinator, n: usize) -> Vec<myl_types::MerkleRoot> {
    let alle = k.completed_segments();
    alle[alle.len() - n..].iter().map(|c| c.spurwurzel).collect()
}

/// Dieselbe Sitzung zweimal ohne Vergessen: Token und Spuren der zweiten
/// Rechnung gleichen denen eines frischen Koordinators.
#[test]
fn eine_wiederholung_ohne_vergessen_rechnet_wie_neu() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let ids: Vec<u32> = tok.encode("Die Hauptstadt von Frankreich ist").iter().map(|t| *t as u32).collect();

    let frisch = koordinator(&model);
    let soll = frisch.run_prompt(7, &ids, 8);
    let n = frisch.completed_segments().len();

    let wiederholt = koordinator(&model);
    let _ = wiederholt.run_prompt(7, &ids, 8);
    // Kein `sitzung_abschliessen`: so, als hätte das Vergessen die Shards
    // nicht erreicht.
    let ist = wiederholt.run_prompt(7, &ids, 8);

    assert_eq!(ist, soll, "die zweite Rechnung zieht andere Token");
    assert_eq!(letzte_spuren(&wiederholt, n), letzte_spuren(&frisch, n), "die zweite Rechnung hat andere Spuren");
}
