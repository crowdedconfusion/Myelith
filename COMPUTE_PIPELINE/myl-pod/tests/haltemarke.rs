//! ⚑ **Eine Antwort endet an der Endmarke des Modells** (Fund 702,
//! 2026-11-15). Bis dahin rechnete der Pod jede Anfrage bis zum Deckel,
//! auch hinter `<|im_end|>`: eine erfundene nächste Runde, bezahlt und als
//! Antwort ausgeliefert.
//!
//! Gerechnet wird mit vier Shards im Prozess und dem Ankermodell; ohne
//! Artefakte steht die Probe still.

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

/// So viele Token höchstens; weit über der kurzen Antwort.
const DECKEL: u64 = 40;

fn koordinator(dir: &std::path::Path) -> (Coordinator, Tokenizer) {
    let model = Arc::new(load_model(dir).expect("Modell"));
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().expect("Pfad")).expect("Wortschatz");
    let nl = model.num_layers;
    let shards = (0..4usize)
        .map(|s| {
            let sk = BlsSecretKey::key_gen(&[(s as u8 + 1) * 23; 32]).expect("BLS");
            Arc::new(ShardNode::new(s, nl * s / 4, nl * (s + 1) / 4, s == 0, s == 3, model.clone(), sk, 4096))
        })
        .collect();
    (Coordinator::new(PodId::new([0xBB; 32]), EpochId(0), shards), tok)
}

fn eingabe(tok: &Tokenizer, prompt: &str) -> Vec<u32> {
    tok.encode(prompt).iter().map(|t| *t as u32).collect()
}

/// Eine Runde in der Vorlage, ohne Überlegung; die Antwort ist ein Wort.
const FRAGE: &str = "<|im_start|>user\nAntworte nur mit dem Wort Ja.<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n";

/// ⚑ Mit Endmarken hält der Pod an der ersten, und die Marke ist das letzte
/// Token. **Gegenprobe im selben Test:** Ohne Marken rechnet derselbe Pod
/// über die Marke hinaus bis zum Deckel; sonst wählte die Probe nichts aus.
#[test]
fn die_antwort_endet_an_der_endmarke() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (mut k, tok) = koordinator(&dir);
    let marken = myl_pod::podwerk::haltemarken(&tok);
    assert_eq!(marken.len(), 2, "<|im_end|> und <|endoftext|> sind je ein Token: {marken:?}");
    let ids = eingabe(&tok, FRAGE);

    let ohne = k.run_prompt(1, &ids, DECKEL);
    assert_eq!(ohne.len() as u64, DECKEL, "ohne Marken bis zum Deckel");
    let erste = ohne.iter().position(|t| marken.contains(t)).expect("das Modell schreibt die Endmarke");
    assert!(erste + 1 < ohne.len(), "hinter der Marke kommt noch etwas, sonst prüfte die Probe nichts");

    k.haltemarken_setzen(marken.clone());
    let mit = k.run_prompt(2, &ids, DECKEL);
    assert_eq!(mit.len(), erste + 1, "Halt an der ersten Marke: {mit:?}");
    assert!(marken.contains(mit.last().expect("Token")), "die Marke ist das letzte Token");
    assert_eq!(&mit[..], &ohne[..=erste], "bis zur Marke dieselben Token");
}

/// ⚑ **Ist schon das erste gezogene Token eine Marke**, geht keine Runde mehr
/// in die Pipeline: Die Antwort steht im Prompt, und das Modell schließt.
#[test]
fn ist_das_erste_token_eine_marke_endet_die_antwort_sofort() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (mut k, tok) = koordinator(&dir);
    let marken = myl_pod::podwerk::haltemarken(&tok);
    let ids = eingabe(&tok, &format!("{FRAGE}Ja."));
    let ohne = k.run_prompt(3, &ids, DECKEL);
    if !marken.contains(&ohne[0]) {
        // Das Modell schließt hier nicht sofort; dann gibt es nichts zu prüfen,
        // und das ist ein Befund über den Prompt, nicht über den Halt.
        panic!("das erste Token ist keine Endmarke: {:?}", &ohne[..4.min(ohne.len())]);
    }
    k.haltemarken_setzen(marken);
    assert_eq!(k.run_prompt(4, &ids, DECKEL), vec![ohne[0]]);
}
