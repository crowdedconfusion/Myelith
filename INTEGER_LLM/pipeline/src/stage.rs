//! Pipeline-Stage – echte Layer-Ausführung (Phase 12.56–12.59)
//!
//! Jede Stage führt ihre zugewiesenen Layer über die Integer-Runtime
//! aus (statt Tensor-Durchleitung):
//! - Stage mit Embedding (Stage 0): empfängt gepackte Token-IDs,
//!   Embedding-Lookup, Layer `[layer_start, layer_end)`.
//! - Zwischen-Stages: empfangen Aktivierungen an der Boundary-Skala,
//!   reskalieren auf die Eingangsskala ihres ersten Layers, führen ihre
//!   Layer aus, reskalieren zurück auf die Boundary-Skala.
//! - Finale Stage (LM-Head): führt ihre Layer aus, dann finale RMSNorm +
//!   LM-Head, sampelt (greedy, deterministisch) und startet die
//!   Feedback-Schleife zur Stage 0 (autoregressive Generation).
//!
//! Konsens-/Determinismus-Eigenschaften:
//! - Alle Berechnungen laufen über dieselben Integer-Kernel wie der
//!   Einzelknoten-Pfad; die Boundary-Reskalierung ist eine
//!   Zweierpotenz-Verschiebung mit dokumentierter Rundung und auf jedem
//!   Node identisch.
//! - KV-Cache pro Request (absolute Layer-Indizes der Stage).
//! - Duplikaterkennung über (request_id, stage_id, token_position) —
//!   macht Retransmits nach Paketverlust idempotent.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use integer_llm_kernels::fixed_point::{clamp_i16, rescale};
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::model::IntegerModel;

use crate::codec::{
    unpack_tokens, MessageMeta, FLAG_TOKEN_INPUT,
};
use crate::manifest::{PipelineManifest, StageManifest};

/// Ausgang einer Stage-Verarbeitung.
#[derive(Default)]
pub struct StageOutput {
    /// Aktivierungs-Nachricht an die nächste Stage.
    pub forward: Option<(MessageMeta, Vec<i16>)>,
    /// Token-Feedback an Stage 0 (autoregressive Schleife).
    pub feedback: Option<(MessageMeta, Vec<i16>)>,
    /// Sampelte Tokens zur Ausgabe: (request_id, position, token).
    pub tokens: Vec<(u64, u64, u32)>,
}

impl StageOutput {
    fn empty() -> Self {
        StageOutput::default()
    }
}

/// Laufzeit-Zustand einer Stage in der Multi-Node-Pipeline.
pub struct StageRuntime {
    pub manifest: StageManifest,
    pub pipeline: PipelineManifest,
    model: Arc<IntegerModel>,
    processed_keys: Mutex<HashSet<(u64, u64, u64)>>,
    /// KV-Cache je Request (Layer-Range dieser Stage, absolute Indizes).
    caches: Mutex<HashMap<u64, KVCache>>,
    /// Generierungs-Buchhaltung je Request: (läuft, Anzahl sampelter
    /// Tokens).
    gen_state: Mutex<HashMap<u64, (bool, u64)>>,
    max_new_tokens: u64,
    /// Erwarteter trunkierter θ_v-Hash (aus dem kanonischen Hash des
    /// geladenen Modells).
    expected_theta_u64: u64,
}

impl StageRuntime {
    pub fn new(
        manifest: StageManifest,
        pipeline: PipelineManifest,
        model: IntegerModel,
        max_new_tokens: u64,
    ) -> Self {
        let expected_theta_u64 = truncated_theta_u64(&canonical_theta_v_id(&model.theta_v));
        StageRuntime {
            manifest,
            pipeline,
            model: Arc::new(model),
            processed_keys: Mutex::new(HashSet::new()),
            caches: Mutex::new(HashMap::new()),
            gen_state: Mutex::new(HashMap::new()),
            max_new_tokens,
            expected_theta_u64,
        }
    }

    /// Erwarteter trunkierter θ_v-Hash dieser Stage (für Clients/Tests).
    pub fn expected_theta_u64(&self) -> u64 {
        self.expected_theta_u64
    }

    /// Verarbeitet eine eingehende Nachricht und erzeugt Ausgaben.
    pub fn process_message(&self, blob: &[u8]) -> Result<StageOutput, String> {
        let (meta, tensor) = crate::codec::decode_message(blob)?;

        // 1. Duplikaterkennung (idempotente Retransmits).
        let key = meta.dedup_key();
        {
            let mut keys = self.processed_keys.lock().unwrap();
            if keys.contains(&key) {
                return Ok(StageOutput::empty());
            }
            keys.insert(key);
        }

        // 2. θ_v-Validierung.
        if meta.theta_v_hash != self.expected_theta_u64 {
            return Err(format!(
                "theta_v hash mismatch: msg={:016x} expected={:016x}",
                meta.theta_v_hash, self.expected_theta_u64
            ));
        }

        // 3. Abort.
        if meta.is_abort() {
            self.abort_request(meta.request_id);
            return Ok(StageOutput::empty());
        }

        // 4. Echte Verarbeitung je Nachrichtenart.
        if meta.is_token_input() {
            self.process_token_input(&meta, &tensor)
        } else {
            self.process_activations(&meta, &tensor)
        }
    }

    /// Stage-0-Eingang: gepackte Token-IDs → Embedding → Layer-Block.
    fn process_token_input(
        &self,
        meta: &MessageMeta,
        tensor: &[i16],
    ) -> Result<StageOutput, String> {
        if !self.manifest.has_embedding {
            return Err(format!(
                "Stage {}: Token-Eingang ohne Embedding",
                self.manifest.stage_id
            ));
        }
        let tokens = unpack_tokens(tensor)?;
        let base = meta.token_position as usize;
        let mut cache = self.take_cache(meta.request_id);
        let mut out = Vec::with_capacity(tokens.len() * self.model.hidden_size);
        for (i, tok) in tokens.iter().enumerate() {
            let pos = base + i;
            let hidden = self.model.embed_token(*tok as usize);
            let hidden = self.model.run_layers(
                hidden,
                pos,
                &mut cache,
                self.manifest.layer_start,
                self.manifest.layer_end,
            );
            out.extend_from_slice(&self.to_boundary_scale(&hidden));
        }
        self.put_cache(meta.request_id, cache);

        let next_meta = MessageMeta {
            version: meta.version,
            theta_v_hash: meta.theta_v_hash,
            request_id: meta.request_id,
            sequence_id: meta.sequence_id,
            stage_id: self.manifest.stage_id as u64 + 1,
            token_position: meta.token_position,
            payload_len: (out.len() * 2) as u64,
            flags: meta.flags & !FLAG_TOKEN_INPUT,
            reserved: 0,
            crc: 0,
        };
        let mut output = StageOutput::empty();
        output.forward = Some((next_meta, out));
        Ok(output)
    }

    /// Zwischen-/Final-Stage: Aktivierungen → Layer-Block (→ Norm,
    /// LM-Head, Sampling, Feedback).
    fn process_activations(
        &self,
        meta: &MessageMeta,
        tensor: &[i16],
    ) -> Result<StageOutput, String> {
        let hs = self.model.hidden_size;
        if !tensor.len().is_multiple_of(hs) {
            return Err(format!(
                "Stage {}: Payload-Länge {} ist kein Vielfaches von hidden_size {}",
                self.manifest.stage_id,
                tensor.len(),
                hs
            ));
        }
        let count = tensor.len() / hs;
        let base = meta.token_position as usize;
        let in_frac = self.input_frac();
        let boundary = self.pipeline.boundary_frac_bits;

        let mut cache = self.take_cache(meta.request_id);
        let mut last_hidden: Option<Vec<i16>> = None;
        let mut forwarded = Vec::new();
        for i in 0..count {
            let slice = &tensor[i * hs..(i + 1) * hs];
            let hidden: Vec<i16> = slice
                .iter()
                .map(|v| clamp_i16(rescale(*v as i32, boundary, in_frac)))
                .collect();
            let hidden = self.model.run_layers(
                hidden,
                base + i,
                &mut cache,
                self.manifest.layer_start,
                self.manifest.layer_end,
            );
            if self.manifest.has_lm_head {
                last_hidden = Some(hidden);
            } else {
                forwarded.extend_from_slice(&self.to_boundary_scale(&hidden));
            }
        }
        self.put_cache(meta.request_id, cache);

        let mut output = StageOutput::empty();

        if !self.manifest.has_lm_head {
            let next_meta = MessageMeta {
                version: meta.version,
                theta_v_hash: meta.theta_v_hash,
                request_id: meta.request_id,
                sequence_id: meta.sequence_id,
                stage_id: self.manifest.stage_id as u64 + 1,
                token_position: meta.token_position,
                payload_len: (forwarded.len() * 2) as u64,
                flags: meta.flags,
                reserved: 0,
                crc: 0,
            };
            output.forward = Some((next_meta, forwarded));
            return Ok(output);
        }

        // Finale Stage: Sampling an der letzten Position.
        let hidden = last_hidden.expect("mindestens eine Position");
        let logits = self.model.head_logits(&hidden);
        let token = self.model.greedy_next(&logits) as u32;
        let pos_last = (base + count - 1) as u64;
        output
            .tokens
            .push((meta.request_id, pos_last, token));

        // Generierungs-Buchhaltung und Feedback.
        let mut gen = self.gen_state.lock().unwrap();
        let entry = gen.entry(meta.request_id).or_insert((false, 0));
        if meta.starts_generation() && !entry.0 {
            entry.0 = true;
            entry.1 = 0;
        }
        if entry.0 {
            entry.1 += 1;
            if entry.1 < self.max_new_tokens {
                let feedback_meta = MessageMeta {
                    version: meta.version,
                    theta_v_hash: meta.theta_v_hash,
                    request_id: meta.request_id,
                    sequence_id: meta.sequence_id,
                    stage_id: 0,
                    token_position: pos_last + 1,
                    payload_len: 4,
                    flags: FLAG_TOKEN_INPUT,
                    reserved: 0,
                    crc: 0,
                };
                output.feedback = Some((feedback_meta, crate::codec::pack_tokens(&[token])));
            } else {
                entry.0 = false; // Budget erschöpft — Schleife endet.
            }
        }
        Ok(output)
    }

    /// Eingangsskala des ersten Layers dieser Stage.
    fn input_frac(&self) -> u8 {
        self.model.layers[self.manifest.layer_start].scales.residual_in_frac
    }

    /// Reskaliert einen Hidden-Vektor von seiner natürlichen Skala
    /// (Ausgang des letzten Stage-Layers) auf die Boundary-Skala.
    fn to_boundary_scale(&self, hidden: &[i16]) -> Vec<i16> {
        let out_frac = if self.manifest.layer_end < self.model.num_layers {
            self.model.layers[self.manifest.layer_end].scales.residual_in_frac
        } else {
            self.model.final_residual_frac
        };
        let boundary = self.pipeline.boundary_frac_bits;
        if out_frac == boundary {
            return hidden.to_vec();
        }
        hidden
            .iter()
            .map(|v| clamp_i16(rescale(*v as i32, out_frac, boundary)))
            .collect()
    }

    /// KV-Cache eines Requests entnehmen (oder anlegen).
    fn take_cache(&self, request_id: u64) -> KVCache {
        let mut caches = self.caches.lock().unwrap();
        caches
            .remove(&request_id)
            .unwrap_or_else(|| {
                KVCache::for_range(
                    self.manifest.layer_start,
                    self.manifest.layer_end,
                    self.model.num_kv_heads,
                )
            })
    }

    fn put_cache(&self, request_id: u64, cache: KVCache) {
        let mut caches = self.caches.lock().unwrap();
        caches.insert(request_id, cache);
    }

    /// Markiert einen Request als abgebrochen.
    pub fn abort_request(&self, request_id: u64) {
        let mut gen = self.gen_state.lock().unwrap();
        gen.remove(&request_id);
        println!(
            "[stage:{}] Request {} abgebrochen.",
            self.manifest.stage_id, request_id
        );
    }

    pub fn is_final_stage(&self) -> bool {
        self.manifest.has_lm_head
    }

    pub fn is_first_stage(&self) -> bool {
        self.manifest.has_embedding
    }

    pub fn stage_id(&self) -> usize {
        self.manifest.stage_id
    }
}

/// Kanonischer θ_v-Identifikator: SHA-256 über
/// `version|weights_hash|scales_hash|luts_hash` als `sha256:<hex>`.
/// Deckt den vollständigen numerischen Vertrag ab (Gewichte, Skalen,
/// LUTs, Version).
pub fn canonical_theta_v_id(theta_v: &integer_llm_runtime::loader::ThetaV) -> String {
    use sha2::{Digest, Sha256};
    let canon = format!(
        "{}|{}|{}|{}",
        theta_v.version, theta_v.weights_hash, theta_v.scales_hash, theta_v.luts_hash
    );
    let digest = Sha256::digest(canon.as_bytes());
    format!("sha256:{}", hex_of(&digest))
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Trunkiert einen kanonischen θ_v-Hash auf die u64 im Nachrichten-
/// Header: erste 16 Hex-Ziffern nach dem `sha256:`-Präfix.
pub fn truncated_theta_u64(canonical: &str) -> u64 {
    let hex_part = canonical.strip_prefix("sha256:").unwrap_or(canonical);
    let head: String = hex_part.chars().take(16).collect();
    u64::from_str_radix(&head, 16).unwrap_or(0)
}
