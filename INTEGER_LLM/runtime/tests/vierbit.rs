//! Ein Vierbitartefakt rechnet dieselben Logits wie der int8-Behaelter, aus
//! dem es gepackt ist.
//!
//! # ⚑ Was hier auf dem Pruefstand steht
//!
//! Die Speicherform `vierbit_g128` verspricht **keine Naeherung**: Jede
//! Zeile ergibt dieselbe ganze Zahl wie das ungepackte Gewicht (Kopf von
//! `integer_llm_kernels::vierbit`). Die Kerne pruefen das an
//! Zufallsmatrizen; hier geht es um das ganze Gemisch, mit Lader, Router,
//! Buendel- und Stapelweg der Experten, also um jede Stelle, an der ein Weg
//! die neue Art uebersehen koennte.
//!
//! # Zwei Pruefungen, eine davon ohne den Behaelter
//!
//! Liegt der int8-Behaelter `myelith-35b-a3b-int4ganz` noch da, vergleicht
//! der Test die Logits beider Artefakte Schritt fuer Schritt. Der Behaelter
//! ist so gross wie das int8 und wird nach der Abnahme geloescht; deshalb
//! prueft der Test ausserdem einen **Abdruck** der Logits des gepackten
//! Artefakts. ⚑ Der Abdruck wurde am 2026-10-16 aufgenommen, als beide
//! Artefakte da waren und der Vergleich gruen war; er belegt also dieselbe
//! Gleichheit fort, solange die Rechnung sich nicht aendert. Aendert sie
//! sich (was nur mit einer neuen θ_v-Fassung zulaessig ist), wird der
//! Behaelter neu gebaut (`gittervergleich.py bauen … int4ganz`) und der
//! Abdruck mit ihm neu aufgenommen.
//!
//! # Warum `#[ignore]`
//!
//! Die Artefakte sind 19 bzw. 33 GB gross und liegen nicht auf jeder
//! Maschine. Gezielt gefahren wird er mit
//!
//! ```text
//! cargo test --release --features cpu-simd --test vierbit -- --ignored
//! ```
//!
//! (`--release`, weil ein 35B im Testprofil Minuten je Token braucht; hier
//! wird nur verglichen, nicht auf Ueberlauf geprueft. Die Ueberlaufpruefung
//! tragen die Kerntests.)

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::{load_model, sha256_hex};
use integer_llm_runtime::model::{Feedforward, IntegerModel};

/// SHA-256 der Logits aller fuenf Schritte (je Wert vier Byte, little
/// endian) des gepackten Artefakts, aufgenommen am 2026-10-16 bei gruenem
/// Vergleich mit dem Behaelter.
const ABDRUCK: &str = "a5261eb5af53d377a111270494340cf846ba4de7148c9c48650115ecaa56372e";

fn artefakt(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts").join(name)
}

/// Eine Vorbereitung ueber mehrere Token (Stapelweg) und danach einzelne
/// Schritte (Decode-Weg): beide Wege der Experten. Greedy, die Token waehlt
/// das Modell selbst.
fn logits(modell: &IntegerModel) -> Vec<Vec<i32>> {
    let prompt: [usize; 6] = [785, 6722, 315, 9625, 374, 12095];
    let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
    modell.vorbereiten_stapel(&prompt[..5], 0, &mut cache);
    let mut l = modell.forward_token(prompt[5], 5, &mut cache);
    let mut alle = vec![l.clone()];
    for schritt in 0..4 {
        let naechstes = modell.greedy_next(&l);
        l = modell.forward_token(naechstes, 6 + schritt, &mut cache);
        alle.push(l.clone());
    }
    alle
}

fn abdruck(alle: &[Vec<i32>]) -> String {
    let bytes: Vec<u8> = alle.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
    sha256_hex(&bytes)
}

#[test]
#[ignore = "braucht myelith-35b-a3b-int4 (und fuer den Vergleich myelith-35b-a3b-int4ganz); cargo test --release --features cpu-simd --test vierbit -- --ignored"]
fn gepackt_rechnet_dieselben_logits() {
    let ziel = artefakt("myelith-35b-a3b-int4");
    assert!(ziel.exists(), "Artefakt fehlt: {ziel:?}");
    let gepackt = load_model(&ziel).expect("gepackt laedt");
    let alle_vierbit = gepackt.layers.iter().all(|e| match &e.ffn {
        Feedforward::Moe(m) => m.experts.iter().all(|x| x.gate_proj.ist_vierbit() && x.up_proj.ist_vierbit() && x.down_proj.ist_vierbit()),
        Feedforward::Dense(_) => false,
    });
    assert!(alle_vierbit, "nicht jeder Experte ist gepackt");
    let lb = logits(&gepackt);
    drop(gepackt);

    let quelle = artefakt("myelith-35b-a3b-int4ganz");
    if quelle.exists() {
        let la = logits(&load_model(&quelle).expect("Behaelter laedt"));
        for (schritt, (a, b)) in la.iter().zip(&lb).enumerate() {
            assert_eq!(a, b, "Logits in Schritt {schritt}");
        }
        assert_eq!(la.len(), lb.len());
    } else {
        eprintln!("[vierbit] Behaelter fehlt, es zaehlt der Abdruck");
    }
    let ist = abdruck(&lb);
    eprintln!("[vierbit] Abdruck {ist}");
    assert_eq!(ist, ABDRUCK, "Abdruck der Logits des gepackten Artefakts");
}
