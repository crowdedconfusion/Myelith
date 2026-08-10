//! Pipeline-Stage – Laufzeit-Logik fuer einen Shard in Multi-Node
//! 
//! Jede Stage empfaengt Tensoren, fuehrt ihre Layer aus,
//! und sendet das Ergebnis an die naechste Stage via TCP.

use crate::codec::{MessageMeta, encode_message, decode_message};
use crate::manifest::{StageManifest, PipelineManifest};
use crate::node::Node;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Laufzeit-Zustand einer Stage in der Multi-Node-Pipeline.
pub struct StageRuntime {
    pub manifest: StageManifest,
    pub pipeline: PipelineManifest,
    pub processed_keys: Arc<Mutex<HashSet<(u64, u64, u64)>>>,
    pub request_count: Arc<Mutex<u64>>,
    pub token_count: Arc<Mutex<u64>>,
    pub abort_flags: Arc<Mutex<HashSet<u64>>>, // request_ids die abgebrochen wurden
}

impl StageRuntime {
    pub fn new(manifest: StageManifest, pipeline: PipelineManifest) -> Self {
        StageRuntime {
            manifest,
            pipeline,
            processed_keys: Arc::new(Mutex::new(HashSet::new())),
            request_count: Arc::new(Mutex::new(0)),
            token_count: Arc::new(Mutex::new(0)),
            abort_flags: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    
    /// Verarbeitet eine eingehende Nachricht und baut Ausgabe.
    pub fn process_message(&self, blob: &[u8]) -> Result<Option<(MessageMeta, Vec<i16>)>, String> {
        let (meta, tensor) = decode_message(blob)?;
        
        // 1. Duplikaterkennung
        let key = meta.dedup_key();
        {
            let mut keys = self.processed_keys.lock().unwrap();
            if keys.contains(&key) {
                return Ok(None);
            }
            keys.insert(key);
        }
        
        // 2. theta_v Validierung
        let expected_hash = self.pipeline.theta_v_hash.parse::<u64>().unwrap_or(0);
        if meta.theta_v_hash != expected_hash {
            return Err(format!(
                "theta_v hash mismatch: msg={:016x} expected={:016x}",
                meta.theta_v_hash, expected_hash
            ));
        }
        
        // 3. Abort-Check
        {
            let aborts = self.abort_flags.lock().unwrap();
            if aborts.contains(&meta.request_id) {
                return Ok(None);
            }
        }
        
        // 4. Stage-spezifische Verarbeitung (Placeholder)
        // In echt: Layer-Ausfuehrung via integer-llm-kernels
        let output_tensor = tensor;
        
        // 5. Naechste Meta bauen
        let next_stage_id = self.manifest.stage_id + 1;
        let is_last = meta.is_last_token() || self.manifest.has_lm_head;
        
        let next_meta = MessageMeta {
            version: meta.version,
            theta_v_hash: meta.theta_v_hash,
            request_id: meta.request_id,
            sequence_id: meta.sequence_id + 1,
            stage_id: next_stage_id as u64,
            token_position: meta.token_position + 1,
            payload_len: output_tensor.len() as u64 * 2,
            flags: if is_last { 1 } else { 0 },
            reserved: 0,
            crc: 0,
        };
        
        {
            let mut tc = self.token_count.lock().unwrap();
            *tc += 1;
        }
        
        Ok(Some((next_meta, output_tensor)))
    }
    
    /// Markiert einen Request als abgebrochen.
    pub fn abort_request(&self, request_id: u64) {
        let mut aborts = self.abort_flags.lock().unwrap();
        aborts.insert(request_id);
        println!("[stage:{}] Request {} abgebrochen.", self.manifest.stage_id, request_id);
    }
    
    /// Prueft, ob diese Stage das letzte Segment ist.
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
