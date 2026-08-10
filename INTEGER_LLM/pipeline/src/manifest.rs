//! Stage- und Pipeline-Manifeste
//! 
//! Jeder Shard hat ein eigenes Manifest. Das globale Pipeline-Manifest
//! definiert die Topologie und wird von allen Knoten validiert.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Ein einzelner Pipeline-Stage (Shard).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StageManifest {
    pub stage_id: usize,
    pub layer_start: usize,
    pub layer_end: usize,        // exklusiv
    pub has_embedding: bool,     // Nur Stage 0
    pub has_lm_head: bool,       // Nur letzte Stage
    pub has_sampling: bool,      // Nur letzte Stage
    pub node_id: String,
    pub node_address: String,    // TCP/IP oder Socket
    pub weights_hash: String,    // SHA-256 der Gewichte dieses Shards
    pub scales_hash: String,     // SHA-256 der Skalen
    pub kernel_contract: String, // z.B. "reference-v0.4.0"
    pub boundary_contract: String, // z.B. "int16-little-endian-frac8"
    pub max_batch_size: usize,
    pub max_context_per_request: usize,
}

/// Globales Pipeline-Manifest.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PipelineManifest {
    pub pipeline_hash: String,   // SHA-256 ueber kanonisches JSON
    pub theta_v_hash: String,    // Muss mit theta_v/spec.json uebereinstimmen
    pub stages: Vec<StageManifest>,
    pub boundary_dtype: String,  // "int16"
    pub boundary_frac_bits: u8,
    pub boundary_endianness: String, // "little"
    pub communication_protocol: String, // "tcp-binary-custom"
    pub checksum_algorithm: String, // "crc32"
}

impl PipelineManifest {
    /// Laedt und validiert ein Pipeline-Manifest.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Fehler beim Lesen: {}", e))?;
        let manifest: PipelineManifest = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        
        // Validierung: Stage-Grenzen muessenlueckenlos sein
        let mut expected_start = 0;
        for stage in &manifest.stages {
            if stage.layer_start != expected_start {
                return Err(format!(
                    "Stage {} beginnt bei {}, erwartet {}",
                    stage.stage_id, stage.layer_start, expected_start
                ));
            }
            expected_start = stage.layer_end;
        }
        
        // Validierung: Genau eine Stage mit Embedding und LM-Head
        let embed_count = manifest.stages.iter().filter(|s| s.has_embedding).count();
        let head_count = manifest.stages.iter().filter(|s| s.has_lm_head).count();
        if embed_count != 1 {
            return Err(format!("Erwarte genau 1 Embedding-Stage, habe {}", embed_count));
        }
        if head_count != 1 {
            return Err(format!("Erwarte genau 1 LM-Head-Stage, habe {}", head_count));
        }
        
        Ok(manifest)
    }
    
    /// Berechnet den Hash fuer ein gegebenes theta_v.
    pub fn verify_theta_v(&self, theta_v_hash: &str) -> Result<(), String> {
        if self.theta_v_hash != theta_v_hash {
            return Err("theta_v hash mismatch".to_string());
        }
        Ok(())
    }
    
    /// Ermittelt die naechste Stage in der Pipeline.
    pub fn next_stage(&self, stage_id: usize) -> Option<&StageManifest> {
        self.stages.iter().find(|s| s.stage_id == stage_id + 1)
    }
    
    /// Ermittelt die vorherige Stage.
    pub fn prev_stage(&self, stage_id: usize) -> Option<&StageManifest> {
        if stage_id == 0 {
            return None;
        }
        self.stages.iter().find(|s| s.stage_id == stage_id - 1)
    }
}
