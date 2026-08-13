//! Phase-1-Akzeptanztest: 4-Node-Pod mit deterministischer Token-Ausgabe
//! und Manipulationserkennung.
//!
//! Akzeptanzkriterien (COMPUTE_PIPELINE-Fahrplan Phase 1):
//! 1. Der 4-Node-Pod liefert bei wiederholtem identischem Prompt eine
//!    **bitgleiche** Token-Sequenz (Determinismus).
//! 2. Die Pod-Ausgabe ist **bitgleich mit der Einzelknoten-Runtime**
//!    (derselbe rechenkorrekte Forward-Pass, nur anders verteilt).
//! 3. Die Eingangs-Hash-Prüfung lehnt **manipulierte Aktivierungen**
//!    zuverlässig ab.

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::generate::generate;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::da::{DaStore, XorParityCoder};
use myl_pod::shard::{ShardNode, ShardOut};
use myl_pod::wire::{self, PodMessage};
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    // COMPUTE_PIPELINE/myl-pod → INTEGER_LLM/artifacts/qwen2.5-0.5b
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push("qwen2.5-0.5b");
    p
}

fn build_shards(model: Arc<integer_llm_runtime::model::IntegerModel>, max_tokens: u64) -> Vec<Arc<ShardNode>> {
    let num_layers = model.num_layers;
    let boundaries = [0usize, 6, 12, 18, num_layers];
    let mut shards = Vec::new();
    for s in 0..4 {
        let layer_start = boundaries[s];
        let layer_end = boundaries[s + 1];
        let has_embedding = s == 0;
        let has_lm_head = s == 3;
        let ikm = [(s as u8 + 1) * 17; 32];
        let sk = BlsSecretKey::key_gen(&ikm).expect("BLS KeyGen");
        let da = DaStore::new(Box::new(XorParityCoder::new(4)));
        let boundary_frac = 4;
        let shard = ShardNode::new(
            s,
            layer_start,
            layer_end,
            has_embedding,
            has_lm_head,
            model.clone(),
            sk,
            boundary_frac,
            da,
            max_tokens,
        );
        shards.push(Arc::new(shard));
    }
    shards
}

const PROMPT: &str = "Die Hauptstadt von Frankreich ist";
const MAX_NEW_TOKENS: u64 = 6;

#[test]
fn pod_deterministisch_und_bitgleich_mit_einzelknoten() {
    let dir = artifacts_dir();
    assert!(dir.exists(), "Artefakte fehlen: {:?}", dir);
    let model = load_model(&dir).expect("Modell-Ladung");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung");

    // Referenz: Einzelknoten-Runtime (liefert usize → nach u32 wandeln).
    let ref_tokens: Vec<u32> = generate(&model, &tokenizer, PROMPT, MAX_NEW_TOKENS as usize, 0, true)
        .iter()
        .map(|t| *t as u32)
        .collect();

    let model = Arc::new(model);
    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);
    let mut coordinator = Coordinator::new(
        PodId::new([0xAA; 32]),
        EpochId(0),
        shards,
        myl_pod::coordinator::DEFAULT_WINDOW_MS,
    );

    let prompt_ids = tokenizer.encode(PROMPT);
    let prompt_tokens: Vec<u32> = prompt_ids.iter().map(|t| *t as u32).collect();

    // Lauf 1.
    let pod_tokens_1 = coordinator.run_prompt(1, &prompt_tokens, MAX_NEW_TOKENS);
    // Lauf 2 (dieselbe Session-Id, derselbe Prompt).
    let pod_tokens_2 = coordinator.run_prompt(2, &prompt_tokens, MAX_NEW_TOKENS);

    // Akzeptanzkriterium 1: Determinismus.
    assert_eq!(
        pod_tokens_1, pod_tokens_2,
        "zwei Pod-Läufe müssen bitgleiche Token-Sequenzen liefern"
    );
    // Akzeptanzkriterium 2: Bitgleichheit mit dem Einzelknoten.
    assert_eq!(
        pod_tokens_1, ref_tokens,
        "Pod-Ausgabe muss bitgleich mit der Einzelknoten-Runtime sein"
    );
    assert!(!pod_tokens_1.is_empty());
}

#[test]
fn manipulierte_aktivierung_wird_abgelehnt() {
    let dir = artifacts_dir();
    assert!(dir.exists(), "Artefakte fehlen: {:?}", dir);
    let model = load_model(&dir).expect("Modell-Ladung");
    let tokenizer = Tokenizer::from_file(
        dir.join("tokenizer.json").to_str().expect("Pfad-UTF-8"),
    )
    .expect("Tokenizer-Ladung");
    let model = Arc::new(model);
    let shards = build_shards(model.clone(), MAX_NEW_TOKENS);

    // Shard 0 mit einem Prompt-Token füttern, um eine Aktivierung zu erhalten.
    let prompt_ids = tokenizer.encode(PROMPT);
    let first_token = prompt_ids[0] as u32;
    let packed = wire::pack_tokens(&[first_token]);
    let seg = myl_types::ids::SegmentId::new([1u8; 32]);
    let msg = PodMessage::token_input(seg, 7, 0, packed, 0);
    let out = shards[0].process(&msg).expect("Shard 0 verarbeitet Token");
    let forward = match out {
        ShardOut::Forward(next) => next,
        _ => panic!("erwarte Forward von Shard 0"),
    };

    // Unmanipuliert: Shard 1 akzeptiert.
    let ok = shards[1].process(&forward);
    assert!(ok.is_ok(), "unmanipulierte Aktivierung muss akzeptiert werden");

    // Manipuliert: ein Aktivierungs-Byte verfälschen ⇒ Ablehnung.
    let mut tampered = forward.clone();
    tampered.payload[5] = tampered.payload[5].wrapping_add(1);
    let rejected = shards[1].process(&tampered);
    assert!(
        rejected.is_err(),
        "manipulierte Aktivierung muss abgelehnt werden"
    );

    // Auch die Manipulation des Spur-Hashes (ohne Payload-Änderung) muss
    // auffallen: Der Hash der Payload passt dann nicht mehr zur Spur.
    let mut tampered_trace = forward.clone();
    if let Some(last) = tampered_trace.trace.last_mut() {
        last[0] ^= 0xFF;
    }
    let rejected2 = shards[1].process(&tampered_trace);
    assert!(
        rejected2.is_err(),
        "manipulierter Spur-Hash muss abgelehnt werden"
    );
}
