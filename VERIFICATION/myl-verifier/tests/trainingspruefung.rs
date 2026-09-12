//! Der Nachrechner gegen den Pod.
//!
//! # ⚑ Die Frage
//!
//! **Kommt ein Prüfer, der den Lauf selbst rechnet, zu derselben Spur
//! wie der Pod, der sie erzeugt hat?**
//!
//! Wenn nicht, ist jeder Streit über ein Trainingssegment
//! unentscheidbar: Zwei Wege durch dieselbe Spezifikation müssten
//! dasselbe ergeben, sonst ist die Bitgleichheit eine Behauptung.
//!
//! ⚑ **`myl-pod` steht hier als Dev-Abhängigkeit**, und das ist der
//! Unterschied zum Bau: Der Nachrechner **ruft** den Pod nicht, er wird
//! nur **gegen** ihn gehalten. Riefe er ihn, prüfte der Prüfer den
//! Geprüften mit dessen eigenem Werkzeug.

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::trainingsschleife::Trainingsvorgaben;
use myl_pod::trainingswerk::{Trainingswerk, SHARDS};
use myl_types::trainingssegment::Trainingssegment;
use myl_verifier::trainingspruefung::{
    segmente_vergleichen, spur_gehoert_zum_segment, spur_nachrechnen, spuren_vergleichen,
    Modelltrainingsauditor, Shardauftrag, Shardurteil, Spurbefund, Trainingsbefund,
};

fn artefakte() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push("myelith-0.6b");
    p
}

fn auftraege(werk: &Trainingswerk, v: &Trainingsvorgaben) -> Vec<Shardauftrag> {
    (0..SHARDS)
        .map(|j| Shardauftrag {
            von: werk.grenzen()[j],
            bis: werk.grenzen()[j + 1],
            folge: v.folge.clone(),
            ziel: v.ziel,
            schritte: v.schritte,
            lr_nenner: v.lr_nenner,
        })
        .collect()
}

/// ⚑ **Der Prüfer kommt zur selben Spur wie der Pod.**
#[test]
fn der_nachrechner_bestaetigt_einen_ehrlichen_pod() {
    let dir = artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, uebersprungen: {}", dir.display());
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = Trainingsvorgaben { schritte: 2, ..Trainingsvorgaben::vorgabe() };

    let mut werk = Trainingswerk::aus_modell(Arc::clone(&modell)).expect("Werk");
    let pod = werk.segment_fahren(&v).expect("Segment");

    let auditor = Modelltrainingsauditor::neu(Arc::clone(&modell));
    let urteil = spur_nachrechnen(&auditor, &pod.delta_je_shard, &auftraege(&werk, &v))
        .expect("Nachrechnen");
    assert_eq!(
        urteil,
        vec![Shardurteil::Bestaetigt; SHARDS],
        "der Nachrechner widerspricht einem ehrlichen Pod"
    );

    // ⚑ **Und das Commitment folgt aus der Spur**, sonst wäre der
    // Zusammenhang zwischen dem, was in der Kette steht, und dem, was
    // geprüft wird, nur behauptet.
    assert_eq!(
        Trainingssegment::commitment_aus_spur(&pod.delta_je_shard),
        pod.delta_commitment
    );
    eprintln!(
        "\n--- Trainingspruefung ---\n  {} Shards bestaetigt\n  Commitment: {}",
        SHARDS,
        hex(&pod.delta_commitment)
    );
}

/// ⚑ **Die Gegenprobe: ein gefälschter Shard wird widerlegt.**
///
/// Ohne sie prüfte der Test darüber nichts: Ein Nachrechner, der immer
/// „bestätigt" sagt, bestünde den ersten Test.
#[test]
fn ein_gefaelschter_shard_wird_widerlegt() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = Trainingsvorgaben { schritte: 1, ..Trainingsvorgaben::vorgabe() };
    let mut werk = Trainingswerk::aus_modell(Arc::clone(&modell)).expect("Werk");
    let pod = werk.segment_fahren(&v).expect("Segment");

    let mut gefaelscht = pod.delta_je_shard.clone();
    gefaelscht[2] = myl_types::hash::Hash([0xAB; 32]);

    let auditor = Modelltrainingsauditor::neu(modell);
    let urteil =
        spur_nachrechnen(&auditor, &gefaelscht, &auftraege(&werk, &v)).expect("Nachrechnen");
    assert_eq!(urteil[0], Shardurteil::Bestaetigt);
    assert_eq!(urteil[1], Shardurteil::Bestaetigt);
    assert_eq!(urteil[2], Shardurteil::Widerlegt, "die Faelschung kam durch");
    assert_eq!(urteil[3], Shardurteil::Bestaetigt);

    // Und der Spurvergleich zeigt auf denselben Shard.
    assert_eq!(
        spuren_vergleichen(&pod.delta_je_shard, &gefaelscht),
        Spurbefund::Abweichung { shard: 2 }
    );
}

/// ⚑ **Zwei ehrliche Pods sind sich einig, und die Kette sieht es an
/// einem einzigen Wert.**
#[test]
fn zwei_ehrliche_pods_sind_sich_einig() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = Trainingsvorgaben { schritte: 1, ..Trainingsvorgaben::vorgabe() };
    let a = Trainingswerk::aus_modell(Arc::clone(&modell))
        .expect("Werk")
        .segment_fahren(&v)
        .expect("Segment");
    let b = Trainingswerk::aus_modell(Arc::clone(&modell))
        .expect("Werk")
        .segment_fahren(&v)
        .expect("Segment");

    let seg_a = segment(&a.delta_commitment);
    let seg_b = segment(&b.delta_commitment);
    assert_eq!(segmente_vergleichen(&seg_a, &seg_b), Trainingsbefund::Einig);
    spur_gehoert_zum_segment(&seg_a, &a.delta_je_shard).expect("die eigene Spur passt");

    // Ein Pod, der etwas anderes gerechnet hat, ist uneinig.
    let c = Trainingswerk::aus_modell(modell)
        .expect("Werk")
        .segment_fahren(&Trainingsvorgaben { ziel: 468, ..v })
        .expect("Segment");
    let seg_c = segment(&c.delta_commitment);
    assert_eq!(segmente_vergleichen(&seg_a, &seg_c), Trainingsbefund::Uneinig);
}

fn segment(commitment: &myl_types::hash::Hash) -> Trainingssegment {
    Trainingssegment {
        id: myl_types::ids::SegmentId::new([1u8; 32]),
        modell_version: myl_types::ids::MerkleRoot::new([2u8; 32]),
        charge: myl_types::hash::Hash([3u8; 32]),
        startschritt: 0,
        schrittzahl: 1,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: 1 << 12,
        delta_commitment: *commitment,
        bewegte_gewichte: 1,
        pod_pfad: vec![myl_types::ids::MinerId::new([4u8; 32])],
        signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
    }
}

fn hex(h: &myl_types::hash::Hash) -> String {
    h.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
