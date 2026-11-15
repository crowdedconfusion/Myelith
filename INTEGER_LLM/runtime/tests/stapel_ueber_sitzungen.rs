//! ⚑ **Ein Stapel über mehrere Sitzungen rechnet jede Ebene bitgleich wie
//! Token für Token** (Continuous Batching, 2026-11-15).
//!
//! `IntegerModel::ebenen_stapel` rechnet einen Ebenenbereich für Zustände,
//! die zu verschiedenen Sitzungen gehören, je Token mit eigenem Speicher und
//! eigener Position. Ein Shard hasht nach **jeder** Ebene für die Spur, und
//! ein Prüfer rechnet tokenweise nach; jede Abweichung, auch eine einzige
//! Zahl, wäre eine falsche Beschuldigung. Deshalb hier je Ebene und je Token
//! gegen `run_layers`, mit Bereichsspeichern wie im Shard (Ebenen 7 bis 14),
//! in gemischten Stapeln: mehrere Positionen einer Sitzung neben einzelnen
//! anderer.
//!
//! ⚑ Gegenprobe im Test: Mit vertauschten Speichern zweier Sitzungen weicht
//! das Ergebnis ab; die Probe sieht also, wem ein Token zugerechnet wird.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::Belegung;

/// Je Position: Eingang des Bereichs und Zustand nach jeder Ebene darin.
type Verlauf = Vec<(Vec<i16>, Vec<Vec<i16>>)>;

const VON: usize = 7;
const BIS: usize = 14;

fn artefakt() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/sparrow-0.6b-int8")
}

/// Je Sitzung und Position: der Eingang des Bereichs und der Zustand nach
/// jeder Ebene darin, Token für Token gerechnet.
fn tokenweise(
    model: &integer_llm_runtime::model::IntegerModel,
    prompt: &[usize],
) -> Verlauf {
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    prompt
        .iter()
        .enumerate()
        .map(|(pos, &t)| {
            let eingang = model.run_layers(model.embed_token(t), pos, &mut cache, 0, VON);
            let mut h = eingang.clone();
            let mut nach = Vec::new();
            for i in VON..BIS {
                h = model.run_layers(h, pos, &mut cache, i, i + 1);
                nach.push(h.clone());
            }
            (eingang, nach)
        })
        .collect()
}

/// Rechnet einen Stapel Ebene für Ebene und vergleicht nach jeder.
fn stapel_pruefen(
    model: &integer_llm_runtime::model::IntegerModel,
    referenz: &[Verlauf],
    stapel: &[(usize, usize)],
    speicher: &mut [KVCache],
) {
    let belegung: Vec<Belegung> = stapel.iter().map(|&(s, p)| Belegung { speicher: s, position: p }).collect();
    let mut zustaende: Vec<Vec<i16>> = stapel.iter().map(|&(s, p)| referenz[s][p].0.clone()).collect();
    for i in VON..BIS {
        zustaende = model.ebenen_stapel(zustaende, &belegung, speicher, i, i + 1).expect("Stapel");
        for (b, &(s, p)) in stapel.iter().enumerate() {
            assert_eq!(zustaende[b], referenz[s][p].1[i - VON], "Sitzung {s}, Position {p}, Ebene {i} weicht ab");
        }
    }
}

#[test]
fn ein_stapel_ueber_sitzungen_rechnet_jede_ebene_wie_tokenweise() {
    let dir = artefakt();
    if !dir.join("config.json").exists() && !dir.exists() {
        eprintln!("[stapel_ueber_sitzungen] Artefakt fehlt, uebersprungen: {dir:?}");
        return;
    }
    let model = load_model(&dir).expect("Modell");
    assert!(BIS <= model.num_layers);
    let prompts: [&[usize]; 3] = [&[9707, 374, 264, 1273, 315], &[3838, 525, 279, 1156], &[641, 279, 3460]];
    let referenz: Vec<_> = prompts.iter().map(|p| tokenweise(&model, p)).collect();
    let neu = || -> Vec<KVCache> { (0..3).map(|_| KVCache::for_range(VON, BIS, model.num_kv_heads)).collect() };
    // Erster Stapel: A 0 bis 3, B 0 bis 2, C 0; mehrere Positionen je Speicher.
    let erster = [(0, 0), (0, 1), (1, 0), (0, 2), (1, 1), (2, 0), (0, 3), (1, 2)];
    let mut speicher = neu();
    stapel_pruefen(&model, &referenz, &erster, &mut speicher);
    // Gegenprobe auf einem zweiten, gleich gefüllten Satz: B rechnet seine
    // Position 3 mit dem Speicher von A.
    let mut fremd = neu();
    stapel_pruefen(&model, &referenz, &erster, &mut fremd);
    let falsch = model
        .ebenen_stapel(vec![referenz[1][3].0.clone()], &[Belegung { speicher: 0, position: 3 }], &mut fremd, VON, VON + 1)
        .expect("Stapel");
    assert_ne!(falsch[0], referenz[1][3].1[0], "mit fremdem Speicher dasselbe: die Probe sieht die Zuordnung nicht");
    // Zweiter Stapel: Decode von A und B, dazu zwei Positionen von C.
    let zweiter = [(0, 4), (2, 1), (1, 3), (2, 2)];
    stapel_pruefen(&model, &referenz, &zweiter, &mut speicher);
}

/// ⚑ **Der Kopf als Stapel rechnet dieselben Logits wie einzeln**
/// (2026-11-15): drei Endzustände verschiedener Sitzungen und Positionen.
/// Gegenprobe im Test: Die Logits zweier verschiedener Zustände sind
/// verschieden, die Probe vergleicht also nicht Gleiches mit Gleichem.
#[test]
fn der_kopf_als_stapel_rechnet_dieselben_logits() {
    let dir = artefakt();
    if !dir.exists() {
        eprintln!("[stapel_ueber_sitzungen] Artefakt fehlt, uebersprungen: {dir:?}");
        return;
    }
    let model = load_model(&dir).expect("Modell");
    let mut zustaende = Vec::new();
    for prompt in [&[9707usize, 374, 264][..], &[3838, 525], &[641]] {
        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
        let mut h = Vec::new();
        for (pos, &t) in prompt.iter().enumerate() {
            h = model.run_layers(model.embed_token(t), pos, &mut cache, 0, model.num_layers);
        }
        zustaende.push(h);
    }
    let einzeln: Vec<Vec<i32>> = zustaende.iter().map(|h| model.head_logits(h)).collect();
    let refs: Vec<&[i16]> = zustaende.iter().map(Vec::as_slice).collect();
    let gestapelt = model.head_logits_stapel(&refs);
    assert_eq!(gestapelt.len(), 3);
    assert_ne!(einzeln[0], einzeln[1], "zwei Zustaende, dieselben Logits: die Probe prueft nichts");
    for (b, (e, g)) in einzeln.iter().zip(&gestapelt).enumerate() {
        assert_eq!(e.len(), model.vocab_size);
        assert_eq!(e, g, "Zustand {b}: der Stapel rechnet andere Logits");
    }
}
