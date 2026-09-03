//! Fund 164: Was eine Anfrage hinterlässt, und was nicht.
//!
//! # ⚑ Warum das ein eigener Test ist
//!
//! Der Fund war **kein Fehlverhalten, sondern Wachstum**: Jede Anfrage
//! legte einen KV-Cache und einen Dekodier-Digest ab, und nichts nahm
//! sie je wieder weg. Kein Test konnte das sehen, weil jeder Test genau
//! eine Anfrage rechnet und danach endet.
//!
//! **Ein Leck ist erst über mehreren Anfragen sichtbar.** Deshalb
//! rechnet dieser Test mehrere und sieht zwischendurch nach.

use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

const ANFRAGEN: u64 = 5;

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
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
            myl_pod::coordinator::DEFAULT_WINDOW_MS,
        ),
        ids,
    )
}

/// ⚑ **Eine abgeschlossene Sitzung hinterlässt nichts.**
///
/// ⛑ **Die Gegenprobe steht im selben Test.** Ohne den Abschluss wächst
/// die Zahl mit jeder Anfrage; das ist der Zustand, den Fund 164
/// beschreibt, und er ist hier nachgestellt, damit die Zusicherung
/// darüber etwas heisst.
#[test]
fn eine_abgeschlossene_sitzung_hinterlaesst_nichts() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }

    // --- So war es vor dem Fund: niemand raeumt -----------------------
    let (mut k, ids) = koordinator(&dir);
    for n in 1..=ANFRAGEN {
        let _ = k.run_prompt(1000 + n, &ids, 2);
    }
    assert_eq!(
        k.gehaltene_sitzungen() as u64,
        ANFRAGEN,
        "ohne Abschluss muessen die Sitzungen liegenbleiben, sonst prueft der Test daneben nichts"
    );

    // --- Und so ist es jetzt ------------------------------------------
    let (mut k, ids) = koordinator(&dir);
    for n in 1..=ANFRAGEN {
        let sitzung = 2000 + n;
        let _ = k.run_prompt(sitzung, &ids, 2);
        k.sitzung_abschliessen(sitzung);
        assert_eq!(
            k.gehaltene_sitzungen(),
            0,
            "nach der {n}. Anfrage lag noch etwas herum"
        );
    }
}

/// ⚑ **Und der Weg, den eine echte Anfrage geht, räumt auch.**
///
/// ⛑ **Die Gegenprobe hat den ersten Entwurf verworfen.** Der Test
/// darüber ruft `sitzung_abschliessen` selbst; den Aufruf in
/// `Pipelinewerk::rechne` zu streichen liess ihn **grün**. Eine
/// Zusicherung über einen Aufruf muss über den Weg gehen, der ihn tut,
/// und das ist `Klartextwerk::rechne`.
#[test]
fn der_weg_einer_anfrage_raeumt_hinter_sich_auf() {
    use myl_pod::entsiegelung::Klartextwerk;
    use myl_types::hash::Hash;
    use myl_types::inferenzauftrag::Inferenzauftrag;
    use myl_types::sitzung::Anfragebindung;

    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let pipeline = Hash([7u8; 32]);
    let werk = myl_pod::pipelinewerk::Pipelinewerk::laden(
        &dir,
        PodId::new([0xAA; 32]),
        EpochId(0),
        pipeline,
        2,
    )
    .expect("die Pipeline laedt");

    let prompt = b"Die Hauptstadt von Frankreich ist";
    for sitzung in 1..=3u64 {
        let auftrag = Inferenzauftrag {
            sitzung,
            bindung: Anfragebindung::neu(sitzung, prompt, EpochId(0)),
            prompt_versiegelt: Vec::new(),
            max_token: 2,
            pipeline,
        };
        // ⚑ **`Klartextwerk::rechne` und nicht der Koordinator.** Das
        // ist der Weg, den der Shard-Dienst nimmt, nachdem er
        // entsiegelt und die Bindung geprueft hat.
        let _ = werk.rechne(&auftrag, prompt);
        assert_eq!(
            werk.gehaltene_sitzungen(),
            0,
            "nach der {sitzung}. Anfrage hielt die Pipeline noch eine Sitzung"
        );
    }
}

/// ⚑ **Ein gezogenes Bündel leert die Segmentliste.**
///
/// Sie wächst mit **jedem Token**, nicht je Anfrage: gemessen rund
/// vierzehn Einträge je Anfrage der Probepipeline, streng linear.
#[test]
fn ein_gezogenes_buendel_leert_die_segmentliste() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let (mut k, ids) = koordinator(&dir);
    let _ = k.run_prompt(1, &ids, 2);
    assert!(
        !k.completed_segments().is_empty(),
        "ohne abgeschlossene Segmente prueft der Rest nichts"
    );
    assert_eq!(k.verworfene_segmente(), 0, "der Deckel darf hier nicht greifen");

    k.buendel_ziehen().expect("das Buendel entsteht");
    assert!(
        k.completed_segments().is_empty(),
        "das gezogene Buendel hat die Liste nicht geleert"
    );

    // ⚑ **Und die naechste Anfrage faengt wieder bei null an**, statt
    // auf dem Alten aufzusetzen: Ein Buendel fasst eine Epoche zusammen.
    let _ = k.run_prompt(2, &ids, 2);
    assert!(!k.completed_segments().is_empty());
}
