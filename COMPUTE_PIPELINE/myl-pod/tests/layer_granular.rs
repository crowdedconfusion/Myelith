//! Die Annahme, auf der die Spur je Layer ruht.
//!
//! `run_layers(h, pos, cache, a, b)` ist eine Schleife über
//! `forward_layer`. Ein Aufruf je Layer *müsste* deshalb dasselbe
//! liefern wie ein Bereichsaufruf. Müsste ist kein Beleg.

use std::path::PathBuf;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;

fn artifacts_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push("qwen2.5-0.5b");
    p
}

#[test]
fn layer_fuer_layer_ergibt_dasselbe_wie_der_bereich() {
    let dir = artifacts_dir();
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = load_model(&dir).expect("Modell-Ladung");
    let l = model.num_layers;

    // Mehrere Positionen, damit auch der KV-Cache mitgeprüft wird: Er ist
    // der einzige Zustand, den die beiden Wege teilen könnten.
    for pos in 0..3usize {
        let start: Vec<i16> = (0..model.hidden_size)
            .map(|i| ((i * 7 + pos * 13) % 97) as i16 - 48)
            .collect();

        let mut cache_a = KVCache::new(l, model.num_kv_heads);
        let am_stueck = model.run_layers(start.clone(), pos, &mut cache_a, 0, l);

        let mut cache_b = KVCache::new(l, model.num_kv_heads);
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
