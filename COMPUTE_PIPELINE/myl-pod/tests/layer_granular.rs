//! Die Annahme, auf der die Spur je Layer ruht.
//!
//! `run_layers(h, pos, cache, a, b)` ist eine Schleife über
//! `forward_layer`. Ein Aufruf je Layer *müsste* deshalb dasselbe
//! liefern wie ein Bereichsaufruf. Müsste ist kein Beleg.

use std::path::PathBuf;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

mod artefakte;

fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push("myelith-0.6b");
    p
}

#[test]
fn layer_fuer_layer_ergibt_dasselbe_wie_der_bereich() {
    let dir = artifacts_dir();
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let model = load_model(&dir).expect("Modell-Ladung");
    let l = model.num_layers;

    // Mehrere Positionen, damit auch der KV-Cache mitgeprüft wird: Er ist
    // der einzige Zustand, den die beiden Wege teilen könnten.
    //
    // 📌 **Die Speicher stehen vor der Schleife** (2026-09-14). Bis dahin
    // bekam jede Position einen frischen, und Position 1 und 2 rechneten
    // über einer Lücke: Die Aufmerksamkeit sah nur den eigenen Eintrag, und
    // der Speicher wurde über Positionen hinweg gerade **nicht** geprüft.
    // Aufgefallen, als der KV-Speicher zusammenhängend wurde und eine
    // Lücke nicht mehr annimmt.
    let mut cache_a = KVCache::new(l, model.num_kv_heads);
    let mut cache_b = KVCache::new(l, model.num_kv_heads);
    for pos in 0..3usize {
        let start: Vec<i16> = (0..model.hidden_size)
            .map(|i| ((i * 7 + pos * 13) % 97) as i16 - 48)
            .collect();

        let am_stueck = model.run_layers(start.clone(), pos, &mut cache_a, 0, l);

        let mut schritt = start.clone();
        for i in 0..l {
            schritt = model.run_layers(schritt, pos, &mut cache_b, i, i + 1);
        }

        assert_eq!(
            am_stueck, schritt,
            "Position {pos}: Layer für Layer weicht vom Bereichsaufruf ab"
        );
    }
}
