//! Ein ternaer gepacktes Artefakt rechnet dieselben Logits wie das
//! int8-Artefakt, aus dem es gepackt ist.
//!
//! # ⚑ Was hier auf dem Pruefstand steht
//!
//! Die Speicherform `ternaer_g128` verspricht **keine Naeherung**: Jede
//! Zeile ergibt dieselbe ganze Zahl wie das ungepackte Gewicht (Kopf von
//! `integer_llm_kernels::ternaer`). Die Kerne pruefen das an
//! Zufallsmatrizen; hier geht es um das ganze Modell, mit Lader,
//! Achtsamkeit, MLP und beiden Koepfen, also um jede Stelle, an der ein
//! Weg die ternaere Art uebersehen koennte.
//!
//! # Warum `#[ignore]`
//!
//! Die beiden Artefakte entstehen aus einem 16-GB-Download und liegen
//! nicht auf jeder Maschine. Ein Test, der dort fehlschlaegt, waere
//! Laerm; einer, der still springt, sieht aus wie bestanden (Fund 113).
//! `ignored` steht in jeder Ausgabe, und gezielt gefahren wird er mit
//!
//! ```text
//! cargo test --release --test ternaer -- --ignored
//! ```
//!
//! (`--release`, weil ein 8B im Testprofil Minuten braucht; die Rechnung
//! ist dieselbe, und hier wird nur verglichen, nicht auf Ueberlauf
//! geprueft. Die Ueberlaufpruefung tragen die Kerntests.)

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn artefakt(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts").join(name)
}

#[test]
#[ignore = "braucht myelith-8b-ternaer und myelith-8b-ternaer-gepackt; cargo test --release --test ternaer -- --ignored"]
fn gepackt_rechnet_dieselben_logits() {
    let (quelle, ziel) = (artefakt("myelith-8b-ternaer"), artefakt("myelith-8b-ternaer-gepackt"));
    for d in [&quelle, &ziel] {
        assert!(d.exists(), "Artefakt fehlt: {d:?}");
    }
    let int8 = load_model(&quelle).expect("int8 laedt");
    let gepackt = load_model(&ziel).expect("gepackt laedt");
    assert!(gepackt.layers.iter().all(|e| e.ist_ternaer()), "nicht jede Ebene ist gepackt");
    assert!(!int8.layers.iter().any(|e| e.ist_ternaer()));

    // Eine Vorbereitung ueber mehrere Token (Stapelweg) und danach
    // einzelne Schritte (Decode-Weg): beide Wege der Projektionen.
    let prompt: [usize; 6] = [785, 6722, 315, 9625, 374, 12095];
    let mut ca = KVCache::new(int8.num_layers, int8.num_kv_heads);
    let mut cb = KVCache::new(gepackt.num_layers, gepackt.num_kv_heads);
    int8.vorbereiten_stapel(&prompt[..5], 0, &mut ca);
    gepackt.vorbereiten_stapel(&prompt[..5], 0, &mut cb);
    let mut la = int8.forward_token(prompt[5], 5, &mut ca);
    let mut lb = gepackt.forward_token(prompt[5], 5, &mut cb);
    assert_eq!(la, lb, "Logits nach der Vorbereitung");
    for schritt in 0..4 {
        let naechstes = int8.greedy_next(&la);
        assert_eq!(naechstes, gepackt.greedy_next(&lb));
        la = int8.forward_token(naechstes, 6 + schritt, &mut ca);
        lb = gepackt.forward_token(naechstes, 6 + schritt, &mut cb);
        assert_eq!(la, lb, "Logits im Decode, Schritt {schritt}");
    }
}
