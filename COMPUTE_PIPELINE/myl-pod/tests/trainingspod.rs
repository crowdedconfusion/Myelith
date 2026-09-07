//! Ein Trainingspod gegen den Einzelknoten.
//!
//! # ⚑ Die Frage
//!
//! **Rechnet ein Pod aus vier Shards dasselbe wie ein Rechner, der alle
//! Ebenen am Stück hält?**
//!
//! Antwortete er mit nein, wäre geshardetes Training nicht
//! nachrechenbar, und die Redundanzprüfung fiele: Zwei Pods
//! verschiedenen Zuschnitts kämen zu verschiedenen Δm, ohne dass einer
//! falsch gerechnet hätte.

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::shardtraining::{
    rueckwaerts, vorwaerts, Shardgewichte, Shardvorgaben,
};
use integer_llm_runtime::trainingsschleife::{gradient_vom_ziel, Trainingsvorgaben};
use myl_pod::trainingswerk::{Trainingswerk, SHARDS};

fn artefakte() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push("qwen2.5-0.5b");
    p
}

fn vorgaben() -> Trainingsvorgaben {
    Trainingsvorgaben { schritte: 2, ..Trainingsvorgaben::vorgabe() }
}

/// ⚑ **Vier Shards gegen einen, bitgleich.**
#[test]
fn ein_pod_aus_vier_shards_rechnet_wie_ein_einzelner() {
    let dir = artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, uebersprungen: {}", dir.display());
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = vorgaben();
    let letzte = v.folge.len() - 1;

    // --- Der Pod, vier Shards ---------------------------------------
    let mut werk = Trainingswerk::aus_modell(Arc::clone(&modell)).expect("Werk");
    assert_eq!(werk.shardzahl(), SHARDS as u32);
    let pod = werk.segment_fahren(&v).expect("Segment");

    // --- Derselbe Lauf, alle Ebenen in einem Stueck ------------------
    let m = &modell;
    let mut ganz = Shardgewichte::aus_modell(m, 0, m.num_layers).expect("Gewichte");
    for s in 0..v.schritte {
        let vg = Shardvorgaben {
            von: 0,
            bis: m.num_layers,
            schritt: s,
            lr_zaehler: 1,
            lr_nenner: v.lr_nenner,
        };
        let strom: Vec<Vec<i16>> =
            v.folge.iter().map(|t| m.embed_token(*t as usize)).collect();
        let ms = vorwaerts(m, &mut ganz, &vg, &strom).expect("vorwaerts");
        let (_logits, g_y) = gradient_vom_ziel(m, &ms.ausgang[letzte], &v);
        let mut g: Vec<Vec<i32>> =
            (0..v.folge.len()).map(|_| vec![0i32; m.hidden_size]).collect();
        g[letzte] = g_y;
        let _ = rueckwaerts(m, &mut ganz, &vg, &ms, &g).expect("rueckwaerts");
    }

    // --- Der Vergleich, Ebene für Ebene -----------------------------
    let mut ebene = 0usize;
    for j in 0..SHARDS {
        let von = werk.grenzen()[j];
        let bis = werk.grenzen()[j + 1];
        for e in von..bis {
            assert_eq!(
                werk_master(&werk, j, e - von),
                ganz.master[e].matrizen(),
                "Ebene {e} weicht ab (Shard {j})"
            );
            ebene += 1;
        }
    }
    assert_eq!(ebene, m.num_layers, "nicht jede Ebene wurde verglichen");

    assert!(pod.bewegte_gewichte > 0, "der Pod hat gar nichts bewegt");
    assert_eq!(pod.aus_der_form, None, "die Vorgabelernrate treibt nichts aus der Form");
    assert_eq!(pod.delta_je_shard.len(), SHARDS);
    eprintln!(
        "\n--- Trainingspod, {} Shards, {} Ebenen ---\n  \
         {} von {} Gewichten bewegt ueber {} Schritte\n  \
         Delta-Commitment des Pods: {}\n  Grenzen: {:?}",
        SHARDS,
        m.num_layers,
        pod.bewegte_gewichte,
        pod.gewichte_gesamt,
        v.schritte,
        pod.delta_commitment,
        werk.grenzen()
    );
}

fn werk_master(werk: &Trainingswerk, shard: usize, i: usize) -> Vec<&[i32]> {
    werk.gewichte_des_shards(shard).master[i].matrizen()
}

/// ⚑ **Ein zweiter Lauf mit denselben Vorgaben ergibt dasselbe
/// Commitment.** Ohne das wäre der Abdruck kein Beleg.
#[test]
fn zwei_laeufe_liefern_dasselbe_commitment() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = vorgaben();
    let a = Trainingswerk::aus_modell(Arc::clone(&modell))
        .expect("Werk")
        .segment_fahren(&v)
        .expect("Segment");
    let b = Trainingswerk::aus_modell(Arc::clone(&modell))
        .expect("Werk")
        .segment_fahren(&v)
        .expect("Segment");
    assert_eq!(a.delta_commitment, b.delta_commitment);
    assert_eq!(a.delta_je_shard, b.delta_je_shard);

    // Und ein anderer Auftrag ergibt etwas anderes.
    let c = Trainingswerk::aus_modell(modell)
        .expect("Werk")
        .segment_fahren(&Trainingsvorgaben { ziel: 468, ..v })
        .expect("Segment");
    assert_ne!(a.delta_commitment, c.delta_commitment, "das Ziel wirkt nicht");
}
