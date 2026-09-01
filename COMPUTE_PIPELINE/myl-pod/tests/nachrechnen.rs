//! ⚑ **Beweiser und Prüfer kommen unabhängig zur selben Spur.**
//!
//! Das ist der Test, auf den Stufe 2 hinausläuft. `myl-pod` rechnet die
//! Spur eines Shards, wie er es im Betrieb tut; `myl_verifier::ModellAuditor`
//! rechnet sie **auf eigenem Weg** nach. Beide teilen nur den
//! Spur-Vertrag `myl_types::uebergang::activation_hash`.
//!
//! **Ohne diesen Test wäre der Nachrechner eine Vermutung.** Er könnte
//! richtig aussehen und systematisch etwas anderes rechnen; dann
//! beschuldigte Stufe 2 ehrliche Miner, und zwar alle.

mod artefakte;

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use myl_types::hash::Hash;
use myl_types::ids::SegmentId;
use myl_verifier::{check_segment, CheckResult, ModellAuditor, SegmentAuditor};

fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push(std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string()));
    p
}

/// Die Spur, wie der Shard sie erzeugt: ein Hash je Layer.
fn spur_des_beweisers(
    modell: &integer_llm_runtime::model::IntegerModel,
    ein: &[i16],
    pos: usize,
    von: usize,
    bis: usize,
) -> Vec<Hash> {
    let mut hidden = ein.to_vec();
    let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut spur = Vec::new();
    for i in von..bis {
        hidden = modell.run_layers(hidden, pos, &mut cache, i, i + 1);
        spur.push(Hash(myl_pod::trace::activation_hash(&hidden)));
    }
    spur
}

fn eingabe(modell: &integer_llm_runtime::model::IntegerModel) -> Vec<i16> {
    (0..modell.hidden_size)
        .map(|i| ((i * 13 + 7) % 61) as i16 - 30)
        .collect()
}

#[test]
fn beweiser_und_pruefer_kommen_zur_selben_spur() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let (von, bis, pos) = (0usize, 4usize.min(modell.num_layers), 0usize);
    let ein = eingabe(&modell);

    let erwartet = spur_des_beweisers(&modell, &ein, pos, von, bis);
    assert_eq!(erwartet.len(), bis - von, "je Layer ein Eintrag");

    let roh: Vec<u8> = ein.iter().flat_map(|v| v.to_le_bytes()).collect();
    let auditor = ModellAuditor::neu(Arc::clone(&modell), von, bis, pos).expect("Bereich");
    assert_eq!(
        check_segment(&auditor, SegmentId::new([1; 32]), &roh, &erwartet).expect("Nachrechnen"),
        CheckResult::Valid
    );
}

/// ⚑ **Und eine verfälschte Spur wird an der richtigen Stelle gefunden.**
///
/// Die Bisektion beginnt bei der ersten Abweichung; meldete der
/// Nachrechner eine falsche Position, stritte die Schiedsrunde über die
/// falsche Layer.
#[test]
fn eine_verfaelschte_spur_wird_an_der_richtigen_stelle_gefunden() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let (von, bis, pos) = (0usize, 4usize.min(modell.num_layers), 0usize);
    if bis - von < 3 {
        return;
    }
    let ein = eingabe(&modell);
    let mut spur = spur_des_beweisers(&modell, &ein, pos, von, bis);
    spur[2] = Hash::sha256(b"gefaelscht");

    let roh: Vec<u8> = ein.iter().flat_map(|v| v.to_le_bytes()).collect();
    let auditor = ModellAuditor::neu(Arc::clone(&modell), von, bis, pos).expect("Bereich");
    assert_eq!(
        check_segment(&auditor, SegmentId::new([1; 32]), &roh, &spur).expect("Nachrechnen"),
        CheckResult::Invalid { first_divergence: 2 }
    );
}

/// Ein leerer oder verdrehter Bereich ergibt keinen Auditor.
///
/// ⚑ **Ein leerer Bereich ergäbe eine leere Spur, und die verglände sich
/// mit allem.**
#[test]
fn ein_unmoeglicher_bereich_ergibt_keinen_auditor() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    assert!(ModellAuditor::neu(Arc::clone(&modell), 2, 2, 0).is_none(), "leer");
    assert!(ModellAuditor::neu(Arc::clone(&modell), 3, 1, 0).is_none(), "verdreht");
    assert!(
        ModellAuditor::neu(Arc::clone(&modell), 0, modell.num_layers + 1, 0).is_none(),
        "ueber das Modell hinaus"
    );
}

/// ⚑ **Eine ungerade Byte-Zahl ist kein halber Wert.**
///
/// Still abzuschneiden hieße, über eine andere Eingabe zu rechnen als
/// der Beschuldigte, und die Abweichung fiele ihm zur Last.
#[test]
fn eine_ungerade_eingabe_wird_abgewiesen() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell-Ladung"));
    let auditor = ModellAuditor::neu(Arc::clone(&modell), 0, 1, 0).expect("Bereich");
    assert!(auditor.audit_segment(SegmentId::new([1; 32]), &[1, 2, 3]).is_err());
}
