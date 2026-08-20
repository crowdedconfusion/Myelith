//! Die Prüfläufe des Testclients.
//!
//! Drei Unterbefehle, jeder mit demselben Aufbau: Fingerabdruck und
//! Artefakt-Identität ins Protokoll, dann messen, dann ein
//! **Vergleichswert (Digest)**, der zwischen Maschinen gediffed wird.
//!
//! Der Digest ist überall SHA-256 über eine kanonische Bytefolge — nie
//! über eine formatierte Ausgabe. Formatierung ändert sich, Bytes nicht.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::loader;
use integer_llm_runtime::model::IntegerModel;
use myl_pod::coordinator::Coordinator;
use myl_pod::da::{DaStore, ReedSolomonCoder};
use myl_pod::shard::ShardNode;
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

use crate::hardware::Fingerprint;
use crate::logging::{sha256_hex, Event, RunLog};

/// Kodiert den Prompt mit dem Tokenizer aus dem Artefaktverzeichnis.
fn encode_prompt(artifact_dir: &Path, prompt: &str) -> Result<Vec<u32>, String> {
    let path = artifact_dir.join("tokenizer.json");
    let path_str = path
        .to_str()
        .ok_or_else(|| format!("Tokenizer-Pfad nicht darstellbar: {}", path.display()))?;
    let tok = integer_llm_runtime::tokenizer::Tokenizer::from_file(path_str)
        .map_err(|e| format!("Tokenizer nicht ladbar ({}): {}", path.display(), e))?;
    Ok(tok.encode(prompt).iter().map(|t| *t as u32).collect())
}

/// Dekodiert Token zu Klartext, für die Anzeige.
///
/// Schlägt es fehl, wird das gemeldet statt abgebrochen: Der Klartext ist
/// eine Zugabe fürs Zuschauen, der Digest bleibt der eigentliche Nachweis.
fn decode_tokens(artifact_dir: &Path, tokens: &[u32]) -> Option<String> {
    let path = artifact_dir.join("tokenizer.json");
    let tok = integer_llm_runtime::tokenizer::Tokenizer::from_file(path.to_str()?).ok()?;
    Some(tok.decode(&tokens.iter().map(|t| *t as usize).collect::<Vec<_>>()))
}

/// Artefaktverzeichnis, gegen das gemessen wird.
///
/// `integer_llm_runtime::paths` löst relativ zum **Arbeitsverzeichnis**
/// auf — das passt für Läufe aus `INTEGER_LLM/`, nicht für einen Client,
/// der von überall gestartet wird. Deshalb hier absolut, ausgehend vom
/// Ort dieses Crates. Die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR`
/// hat weiterhin Vorrang, damit ein Testlauf auf fremde Artefakte
/// gerichtet werden kann, ohne den Client neu zu bauen.
pub fn default_artifact_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var(integer_llm_runtime::paths::ARTIFACTS_DIR_ENV) {
        if !dir.is_empty() {
            return std::path::PathBuf::from(dir).join(DEFAULT_MODEL);
        }
    }
    repo_root()
        .join("INTEGER_LLM")
        .join(integer_llm_runtime::paths::ARTIFACTS_DIR)
        .join(DEFAULT_MODEL)
}

/// Modellname des Standard-Artefakts.
pub const DEFAULT_MODEL: &str = "qwen2.5-0.5b";

/// Repository-Wurzel, ausgehend vom Ort dieses Crates
/// (`TESTCLIENT/myl-testclient` → zwei Ebenen hoch).
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Schreibt Fingerabdruck und Artefakt-Identität ins Protokoll.
///
/// Beides gehört an den **Anfang** jedes Laufs: Wer ein Protokoll
/// aufmacht, muss zuerst wissen, worauf gemessen wurde, bevor er die
/// Zahlen liest.
fn log_context(log: &mut RunLog, artifact_dir: Option<&Path>) {
    let fp = Fingerprint::collect();
    for (k, v) in &fp.entries {
        log.event(Event::Hardware {
            key: k.clone(),
            value: v.clone(),
        });
    }
    log.event(Event::Hardware {
        key: "fingerprint_sha256".into(),
        value: sha256_hex(&fp.canonical_bytes()),
    });

    let Some(dir) = artifact_dir else { return };
    log.event(Event::Artifact {
        key: "artifact_dir".into(),
        value: dir.display().to_string(),
    });
    if !dir.exists() {
        log.note(format!(
            "Artefaktverzeichnis fehlt: {} — Modellläufe werden übersprungen",
            dir.display()
        ));
        return;
    }
    match loader::load_model_dims(dir) {
        Ok(d) => {
            log.event(Event::Artifact {
                key: "hidden_size".into(),
                value: d.hidden_size.to_string(),
            });
            log.event(Event::Artifact {
                key: "num_layers".into(),
                value: d.num_layers.to_string(),
            });
            log.event(Event::Artifact {
                key: "num_heads".into(),
                value: d.num_heads.to_string(),
            });
            log.event(Event::Artifact {
                key: "vocab_size".into(),
                value: d.vocab_size.to_string(),
            });
        }
        Err(e) => log.note(format!("Modelldimensionen nicht lesbar: {}", e)),
    }
}

/// `myl-test hardware` — nur erheben und protokollieren.
///
/// Der schnellste Weg, eine fremde Maschine in den Vergleich
/// aufzunehmen: kein Modell nötig, kein Artefakt nötig.
pub fn run_hardware(log: &mut RunLog) -> bool {
    log_context(log, None);
    let fp = Fingerprint::collect();
    log.result(
        "hardware_fingerprint",
        &sha256_hex(&fp.canonical_bytes()),
        fp.short_id(),
    );
    log.note(
        "Für den Cross-Hardware-Nachweis diesen Lauf auf jeder Maschine \
         ausführen und die Fingerabdrücke vergleichen — sie MÜSSEN sich \
         unterscheiden, sonst prüft der Determinismustest nichts.",
    );
    true
}

/// `myl-test determinismus` — derselbe Prompt zweimal, bitgleich?
///
/// Prüft die Kerneigenschaft aus Whitepaper Kap. 6.2 lokal: Zwei
/// unabhängige Läufe im selben Prozess müssen bitgleiche Logits
/// liefern. Der eigentliche Nachweis entsteht erst, wenn dieser Lauf auf
/// **verschiedener** Hardware denselben Digest liefert — deshalb steht
/// der Fingerabdruck im selben Protokoll.
pub fn run_determinism(log: &mut RunLog, artifact_dir: &Path, prompt: &str, steps: usize) -> bool {
    log_context(log, Some(artifact_dir));

    if !artifact_dir.exists() {
        log.error(format!(
            "Artefaktverzeichnis {} fehlt — Determinismuslauf nicht möglich",
            artifact_dir.display()
        ));
        return false;
    }

    let model = match log.timed("modell_laden", "", || loader::load_model(artifact_dir)) {
        Ok(m) => m,
        Err(e) => {
            log.error(format!("Modell nicht ladbar: {}", e));
            return false;
        }
    };

    let ids = match encode_prompt(artifact_dir, prompt) {
        Ok(v) => v,
        Err(e) => {
            log.error(e);
            return false;
        }
    };
    log.event(Event::PromptAccepted {
        token_count: ids.len(),
        prompt_sha256: sha256_hex(prompt.as_bytes()),
    });
    if ids.is_empty() {
        log.error("Prompt ergibt null Token");
        return false;
    }

    let mut digests = Vec::new();
    for lauf in 1..=2u32 {
        let d = log.timed(&format!("lauf_{}", lauf), "", || {
            greedy_digest(&model, &ids, steps)
        });
        log.result(&format!("lauf_{}_digest", lauf), &d.0, d.1.clone());
        digests.push(d);
    }

    // Klartext nur auf das Terminal. Im Protokoll stehen Token und Digest;
    // daraus ist der Text ableitbar, und die Datei bleibt schlank.
    if let Some(text) = decode_tokens(artifact_dir, &digests[0].2) {
        log.nur_anzeigen("");
        log.nur_anzeigen(format!("  Prompt:  {}", prompt));
        log.nur_anzeigen(format!("  Antwort: {}", text.trim_end()));
        log.nur_anzeigen("");
    }

    let gleich = digests[0].0 == digests[1].0;
    if gleich {
        log.result("determinismus", &digests[0].0, "bitgleich über zwei Läufe");
    } else {
        log.event(Event::Mismatch {
            name: "determinismus".into(),
            expected: digests[0].0.clone(),
            actual: digests[1].0.clone(),
        });
    }

    log.note(format!(
        "Vergleichswert für andere Maschinen: {} — bei gleichem Prompt und \
         gleichem θ_v MUSS er übereinstimmen, unabhängig von Architektur \
         und Backend.",
        digests[0].0
    ));
    gleich
}

/// Greedy-Dekodierung; liefert (Digest, Kurzbeschreibung).
///
/// Der Digest deckt **alle** erzeugten Token ab, nicht nur das letzte:
/// Ein Unterschied in Schritt 3, der sich in Schritt 7 wieder ausgleicht,
/// wäre sonst unsichtbar.
fn greedy_digest(model: &IntegerModel, ids: &[u32], steps: usize) -> (String, String, Vec<u32>) {
    let mut cache =
        integer_llm_runtime::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
    let mut logits = Vec::new();
    for (pos, &t) in ids.iter().enumerate() {
        logits = model.forward_token(t as usize, pos, &mut cache);
    }
    let mut out: Vec<u32> = Vec::with_capacity(steps);
    let start = ids.len();
    for step in 0..steps {
        let next = model.greedy_next(&logits);
        out.push(next as u32);
        logits = model.forward_token(next, start + step, &mut cache);
    }
    let mut bytes = Vec::with_capacity(out.len() * 4);
    for t in &out {
        bytes.extend_from_slice(&t.to_le_bytes());
    }
    (
        sha256_hex(&bytes),
        format!("{} Token: {:?}", out.len(), &out[..out.len().min(8)]),
        out,
    )
}

/// `myl-test shard` — die erste geshardete Inferenz.
///
/// Fährt einen Pod aus `num_shards` Shards über die myl-pod-Stage-API
/// und vergleicht das Ergebnis mit der **Einzelknoten-Runtime**. Das ist
/// das Akzeptanzkriterium aus COMPUTE_PIPELINE Phase 1: Ein
/// aufgeteiltes Modell muss bitgleich zum ungeteilten rechnen.
pub fn run_shard(
    log: &mut RunLog,
    artifact_dir: &Path,
    prompt: &str,
    steps: usize,
    num_shards: usize,
) -> bool {
    log_context(log, Some(artifact_dir));

    if !artifact_dir.exists() {
        log.error(format!(
            "Artefaktverzeichnis {} fehlt — Shard-Lauf nicht möglich",
            artifact_dir.display()
        ));
        return false;
    }
    if num_shards == 0 {
        log.error("num_shards muss > 0 sein");
        return false;
    }

    let model = match log.timed("modell_laden", "", || loader::load_model(artifact_dir)) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            log.error(format!("Modell nicht ladbar: {}", e));
            return false;
        }
    };
    let num_layers = model.num_layers;
    if num_shards > num_layers {
        log.error(format!(
            "{} Shards für {} Layer — jeder Shard braucht mindestens eine Layer",
            num_shards, num_layers
        ));
        return false;
    }

    let ids = match encode_prompt(artifact_dir, prompt) {
        Ok(v) => v,
        Err(e) => {
            log.error(e);
            return false;
        }
    };
    log.event(Event::PromptAccepted {
        token_count: ids.len(),
        prompt_sha256: sha256_hex(prompt.as_bytes()),
    });
    if ids.is_empty() {
        log.error("Prompt ergibt null Token");
        return false;
    }

    // Schichtgrenzen gleichmäßig verteilen; Rest auf die vorderen Shards.
    let mut boundaries = vec![0usize; num_shards + 1];
    let base = num_layers / num_shards;
    let rest = num_layers % num_shards;
    for s in 0..num_shards {
        boundaries[s + 1] = boundaries[s] + base + usize::from(s < rest);
    }
    log.event(Event::Artifact {
        key: "shard_boundaries".into(),
        value: format!("{:?}", boundaries),
    });

    let mut shards = Vec::with_capacity(num_shards);
    for s in 0..num_shards {
        let sk = match BlsSecretKey::key_gen(&[(s as u8).wrapping_add(1).wrapping_mul(17); 32]) {
            Ok(k) => k,
            Err(e) => {
                log.error(format!("BLS-Schlüssel für Shard {} fehlgeschlagen: {:?}", s, e));
                return false;
            }
        };
        shards.push(Arc::new(ShardNode::new(
            s,
            boundaries[s],
            boundaries[s + 1],
            s == 0,
            s == num_shards - 1,
            model.clone(),
            sk,
            DaStore::new(Box::new(ReedSolomonCoder::default())),
            steps as u64,
        )));
        log.event(Event::Step {
            name: format!("shard_{}_bereit", s),
            millis: 0,
            detail: format!(
                "Layer {}..{}{}{}",
                boundaries[s],
                boundaries[s + 1],
                if s == 0 { ", Embedding" } else { "" },
                if s == num_shards - 1 { ", LM-Head" } else { "" }
            ),
        });
    }

    let mut coordinator = Coordinator::new(
        PodId::new([0xAA; 32]),
        EpochId(0),
        shards,
        myl_pod::coordinator::DEFAULT_WINDOW_MS,
    );

    let pod_out = log.timed("pod_inferenz", &format!("{} Shards", num_shards), || {
        coordinator.run_prompt(1, &ids, steps as u64)
    });
    let pod_digest = digest_tokens(&pod_out);
    log.result(
        "pod_tokens",
        &pod_digest,
        format!("{} Token: {:?}", pod_out.len(), &pod_out[..pod_out.len().min(8)]),
    );

    match coordinator.build_poi_bundle() {
        Ok(b) => log.result(
            "poi_bundle",
            &sha256_hex(b.segments_root.as_bytes()),
            format!(
                "vTFE={}, Segmente={}",
                b.vtfe_claimed,
                coordinator.completed_segments().len()
            ),
        ),
        Err(e) => log.note(format!("Kein PoI-Bündel: {}", e)),
    }

    // Gegenprobe: dasselbe Modell ungeteilt.
    let (single_digest, single_desc, single_tokens) =
        log.timed("einzelknoten_referenz", "", || greedy_digest(&model, &ids, steps));
    log.result("einzelknoten_tokens", &single_digest, single_desc);

    // Klartext nur auf das Terminal, siehe `run_determinism`.
    if let Some(text) = decode_tokens(artifact_dir, &single_tokens) {
        log.nur_anzeigen("");
        log.nur_anzeigen(format!("  Prompt:  {}", prompt));
        log.nur_anzeigen(format!("  Antwort: {}", text.trim_end()));
        log.nur_anzeigen("");
    }

    let gleich = pod_digest == single_digest;
    if gleich {
        log.result(
            "shard_vs_einzelknoten",
            &pod_digest,
            "bitgleich — Akzeptanzkriterium COMPUTE_PIPELINE Phase 1 erfüllt",
        );
    } else {
        log.event(Event::Mismatch {
            name: "shard_vs_einzelknoten".into(),
            expected: single_digest,
            actual: pod_digest,
        });
        log.note(
            "Ein Unterschied hier bedeutet, dass die Aufteilung selbst \
             das Ergebnis verändert — die Shard-Grenzen oder die \
             Randskalierung (boundary_frac) sind der erste Verdacht.",
        );
    }
    gleich
}

fn digest_tokens(tokens: &[u32]) -> String {
    let mut bytes = Vec::with_capacity(tokens.len() * 4);
    for t in tokens {
        bytes.extend_from_slice(&t.to_le_bytes());
    }
    sha256_hex(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::RunLog;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-runs-{}", name));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn hardwarelauf_braucht_keine_artefakte() {
        let dir = tempdir("hw");
        let mut log = RunLog::new(&dir, "hardware", false);
        assert!(run_hardware(&mut log));
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains("\"kind\":\"hardware\""));
        assert!(jsonl.contains("hardware_fingerprint"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ohne Artefakte darf der Client nicht abstürzen, sondern muss den
    /// Grund protokollieren — auf einer frisch aufgesetzten Testmaschine
    /// ist das der Normalfall.
    #[test]
    fn fehlende_artefakte_werden_sauber_gemeldet() {
        let dir = tempdir("keine-artefakte");
        let mut log = RunLog::new(&dir, "determinismus", false);
        let ok = run_determinism(&mut log, Path::new("/nicht/vorhanden"), "Test", 4);
        assert!(!ok);
        assert!(log.problems() > 0);
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(false);

        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains("\"kind\":\"error\""));
        assert!(jsonl.contains("Artefaktverzeichnis"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shardlauf_lehnt_null_shards_ab() {
        let dir = tempdir("null-shards");
        let mut log = RunLog::new(&dir, "shard", false);
        assert!(!run_shard(&mut log, Path::new("/nicht/vorhanden"), "Test", 4, 0));
        log.finish(false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_digest_ist_stabil_und_reihenfolgeabhaengig() {
        assert_eq!(digest_tokens(&[1, 2, 3]), digest_tokens(&[1, 2, 3]));
        assert_ne!(digest_tokens(&[1, 2, 3]), digest_tokens(&[3, 2, 1]));
        assert_ne!(digest_tokens(&[1, 2]), digest_tokens(&[1, 2, 0]));
    }

    /// Der Digest muss alle Token abdecken — ein Unterschied in der
    /// Mitte darf nicht durch ein gleiches Endergebnis verdeckt werden.
    #[test]
    fn digest_erfasst_auch_mittlere_token() {
        assert_ne!(digest_tokens(&[5, 1, 9]), digest_tokens(&[5, 2, 9]));
    }
}
