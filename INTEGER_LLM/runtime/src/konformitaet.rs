//! Konformitätsprüfung der Layer- und E2E-Vektoren gegen das Modell.
//!
//! Die Vektoren unter `INTEGER_LLM/conformance/vectors/{layer,e2e}/`
//! sind mit einem bestimmten kalibrierten Artefakt erzeugt. Sie legen
//! für komplette Transformer-Layer und für ganze Prompt-zu-Token-Läufe
//! die erwarteten Ausgaben bitgenau fest. Ein Bau gilt als konform, wenn
//! er sie bitgleich reproduziert.
//!
//! **Warum die Prüfung hier liegt und nicht im Binary.** Bis v0.21.0
//! steckte sie vollständig in `src/bin/golden_model.rs`. Damit konnte
//! nur aufrufen, wer das Binary baut und findet — der Testclient hätte
//! für einen Konformitätslauf ein zweites Programm starten müssen, und
//! sein Ergebnis wäre eine Terminalausgabe geblieben statt einer
//! Protokollzeile. Die Logik gehört zur Runtime-Bibliothek; das Binary
//! ist ein dünner Starter darüber.
//!
//! Die Vektorstruktur und die Tensor-Hash-Prüfung kommen aus
//! `integer_llm_kernels::konformitaet`: eine Quelle für das Format,
//! nicht zwei.

use crate::kv_cache::KVCache;
use crate::model::IntegerModel;
use integer_llm_kernels::konformitaet::{tensor_hashes_pruefen, vektor_lesen};
pub use integer_llm_kernels::konformitaet::{GoldenVector, VektorErgebnis};
use std::path::Path;

/// Prüft einen Layer- oder E2E-Vektor gegen ein geladenes Modell.
///
/// Die Integrität des Vektors steht vor der Rechnung: Ein Vektor,
/// dessen Tensor-Hashes nicht stimmen, ist kein Maßstab (Fund 37).
pub fn vektor_pruefen(model: &IntegerModel, gv: &GoldenVector) -> VektorErgebnis {
    let name = gv.name.clone();
    let mut gruende: Vec<String> = Vec::new();
    if !tensor_hashes_pruefen(gv, &mut gruende) {
        gruende.push("Vektor ist nicht integer, kein Maßstab".to_string());
        return VektorErgebnis {
            name,
            bestanden: false,
            gruende,
            integer_verletzt: true,
        };
    }
    let (bestanden, weitere) = match gv.level.as_str() {
        "layer" => validate_layer(model, gv),
        "e2e" => validate_e2e(model, gv),
        other => (false, vec![format!("Unbekanntes Level: {}", other)]),
    };
    gruende.extend(weitere);
    VektorErgebnis {
        name,
        bestanden,
        gruende,
        integer_verletzt: false,
    }
}

/// Prüft eine Datei als Layer- oder E2E-Vektor gegen ein geladenes Modell.
pub fn vektor_aus_datei(model: &IntegerModel, pfad: &Path) -> Result<VektorErgebnis, String> {
    let gv = vektor_lesen(pfad)?;
    Ok(vektor_pruefen(model, &gv))
}

fn validate_layer(model: &IntegerModel, gv: &GoldenVector) -> (bool, Vec<String>) {
    let layer_idx = gv.metadata["layer_idx"].as_u64().unwrap() as usize;
    let hidden_in: Vec<i16> = gv.inputs["hidden"].data.iter().map(|&v| v as i16).collect();

    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
    let hidden_out = model.run_layers(hidden_in, 0, &mut cache, layer_idx, layer_idx + 1);

    let expected: Vec<i16> = gv.outputs["hidden_out"].data.iter().map(|&v| v as i16).collect();

    if hidden_out.len() != expected.len() {
        return (false, vec![format!(
            "Laenge mismatch: {} vs. {}",
            hidden_out.len(),
            expected.len()
        )]);
    }

    let mismatches: Vec<usize> = hidden_out
        .iter()
        .zip(expected.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();

    if !mismatches.is_empty() {
        return (false, vec![format!(
            "{} Mismatches in layer {} (erste: {:?})",
            mismatches.len(),
            layer_idx,
            mismatches
                .iter()
                .take(5)
                .map(|&i| (i, hidden_out[i], expected[i]))
                .collect::<Vec<_>>()
        )]);
    }
    (true, Vec::new())
}

fn validate_e2e(model: &IntegerModel, gv: &GoldenVector) -> (bool, Vec<String>) {
    let prompt_tokens: Vec<usize> = gv.inputs["prompt_tokens"]
        .data
        .iter()
        .map(|&v| v as usize)
        .collect();
    let max_new_tokens = gv.metadata["max_new_tokens"].as_u64().unwrap() as usize;
    let greedy = gv.metadata["greedy"].as_bool().unwrap_or(true);
    let seed = gv.metadata["seed"].as_u64().unwrap_or(42);

    // Über `dekodieren_mit_digest`, nicht als eigene Schleife: Die
    // Bytefolge des Digests ist dort festgelegt, und eine zweite Fassung
    // hier wäre eine zweite Quelle für dieselbe Aussage (Fund 34).
    let (erzeugt, digest) = crate::generate::dekodieren_mit_digest(
        model,
        &prompt_tokens,
        max_new_tokens,
        seed,
        greedy,
    );
    let generated: Vec<i32> = erzeugt.iter().map(|&t| t as i32).collect();

    let expected: Vec<i32> = gv.outputs["tokens"].data.iter().map(|&v| v as i32).collect();

    if generated != expected {
        return (false, vec![format!(
            "Token-Mismatch: erzeugt={:?}, erwartet={:?}",
            generated, expected
        )]);
    }

    // **Der eigentliche Prüfwert** (Fund 36). Gleiche Token heißen nur,
    // dass dieselbe Entscheidung gefallen ist; ein Argmax über
    // `vocab_size` Zahlen ändert sich erst, wenn deren Rangfolge kippt.
    // Gemessen an 0,5B: 0,1 % der Bytes eines Tensors verschoben, Token
    // unverändert, Zahlen verschieden.
    match gv.metadata.get("logits_sha256").and_then(|v| v.as_str()) {
        Some(erwartet) => {
            if digest != erwartet {
                return (false, vec![
                    format!("Logit-Mismatch: berechnet={}, erwartet={}", digest, erwartet),
                    "Die Token stimmen, die gerechneten Zahlen nicht. Genau dieser \
                     Fall blieb vor Fund 36 unsichtbar."
                        .to_string(),
                ]);
            }
        }
        None => {
            // Kein stiller Durchlauf: Ein Vektor ohne diesen Wert prüft
            // nur die Entscheidung, und das soll niemand für einen
            // Bitgleichheitsnachweis halten.
            return (false, vec![format!(
                "Vektor {} trägt kein metadata.logits_sha256 und prüft damit nur \
                 die erzeugten Token (Fund 36). Neu erzeugen mit golden_generate.",
                gv.name
            )]);
        }
    }
    (true, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die echten Layer- und E2E-Vektoren des Repositoriums müssen über
    /// die Bibliotheksfunktion bestehen, sobald das zugehörige Artefakt
    /// vorliegt. Ohne Artefakt (CI, frischer Klon) wird übersprungen —
    /// der Lauf ist dann ehrlich still statt grün ohne Grund.
    #[test]
    fn layer_und_e2e_vektoren_bestehen_gegen_das_artefakt() {
        let wurzel = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let artefakt = wurzel.join("../artifacts/qwen2.5-0.5b");
        let vektoren = wurzel.join("../conformance/vectors");
        if !artefakt.is_dir() || !vektoren.is_dir() {
            eprintln!("SKIP: Artefakt oder Vektoren fehlen ({})", artefakt.display());
            return;
        }
        let model = crate::loader::load_model(&artefakt).expect("Modell lädt");
        let mut gesamt = 0;
        for ebene in ["layer", "e2e"] {
            let dir = vektoren.join(ebene);
            let mut dateien: Vec<_> = std::fs::read_dir(&dir)
                .expect("lesbar")
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.to_string_lossy().ends_with(".golden.json"))
                .collect();
            dateien.sort();
            for p in dateien {
                gesamt += 1;
                let e = vektor_aus_datei(&model, &p).expect("prüfbar");
                assert!(e.bestanden, "{}: {:?}", p.display(), e.gruende);
                assert!(!e.integer_verletzt, "{}: Hash-Prüfung verletzt", p.display());
            }
        }
        assert_eq!(gesamt, 27, "24 Layer- und 3 E2E-Vektoren erwartet");
    }
}
