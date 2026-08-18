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

#[cfg(test)]
mod tests {
    use super::*;

    /// Strukturgleich zum echten `StageManifest` — alle Felder, wie sie
    /// ein erzeugtes Manifest tatsaechlich traegt (Projektkonvention:
    /// Test-Fixtures spiegeln reale Formate, nicht Bequemlichkeit).
    fn stage(id: usize, start: usize, end: usize, embed: bool, head: bool) -> serde_json::Value {
        serde_json::json!({
            "stage_id": id,
            "layer_start": start,
            "layer_end": end,
            "has_embedding": embed,
            "has_lm_head": head,
            "has_sampling": head,
            "node_id": format!("node-{}", id),
            "node_address": format!("127.0.0.1:{}", 9000 + id),
            "weights_hash": format!("{:064x}", id),
            "scales_hash": format!("{:064x}", 1000 + id),
            "kernel_contract": "reference-v0.4.0",
            "boundary_contract": "int16-little-endian-frac8",
            "max_batch_size": 32,
            "max_context_per_request": 2048,
        })
    }

    /// Schreibt ein Manifest in eine temporaere Datei und laedt es.
    fn load_json(name: &str, stages: Vec<serde_json::Value>) -> Result<PipelineManifest, String> {
        let dir = std::env::temp_dir().join("myelith-pipeline-tests");
        std::fs::create_dir_all(&dir).expect("Testverzeichnis");
        let path = dir.join(format!("{}.json", name));
        let obj = serde_json::json!({
            "pipeline_hash": format!("{:064x}", 0xABC),
            "theta_v_hash": "abc123",
            "stages": stages,
            "boundary_dtype": "int16",
            "boundary_frac_bits": 8,
            "boundary_endianness": "little",
            "communication_protocol": "tcp-binary-custom",
            "checksum_algorithm": "crc32",
        });
        std::fs::write(&path, serde_json::to_string(&obj).unwrap()).expect("schreiben");
        let result = PipelineManifest::load(&path);
        let _ = std::fs::remove_file(&path);
        result
    }

    fn gueltige_stages() -> Vec<serde_json::Value> {
        vec![
            stage(0, 0, 8, true, false),
            stage(1, 8, 16, false, false),
            stage(2, 16, 24, false, true),
        ]
    }

    #[test]
    fn gueltiges_manifest_laedt() {
        let m = load_json("gueltig", gueltige_stages()).expect("laedt");
        assert_eq!(m.stages.len(), 3);
        assert_eq!(m.theta_v_hash, "abc123");
    }

    /// Eine Luecke zwischen den Stages bedeutet, dass Layer von keinem
    /// Node ausgefuehrt wuerden — das Modell waere still unvollstaendig.
    #[test]
    fn luecke_zwischen_stages_wird_abgelehnt() {
        let stages = vec![
            stage(0, 0, 8, true, false),
            stage(1, 9, 16, false, false), // Layer 8 faellt heraus
            stage(2, 16, 24, false, true),
        ];
        let err = load_json("luecke", stages).unwrap_err();
        assert!(err.contains("beginnt bei"), "unerwartete Meldung: {}", err);
    }

    /// Ueberlappung bedeutet doppelt ausgefuehrte Layer.
    #[test]
    fn ueberlappung_wird_abgelehnt() {
        let stages = vec![
            stage(0, 0, 8, true, false),
            stage(1, 7, 16, false, false),
            stage(2, 16, 24, false, true),
        ];
        assert!(load_json("ueberlappung", stages).is_err());
    }

    #[test]
    fn manifest_muss_bei_layer_null_beginnen() {
        let stages = vec![stage(0, 1, 8, true, true)];
        assert!(load_json("offset", stages).is_err());
    }

    #[test]
    fn genau_eine_embedding_stage() {
        let mut stages = gueltige_stages();
        stages[1]["has_embedding"] = serde_json::json!(true);
        let err = load_json("zwei_embed", stages).unwrap_err();
        assert!(err.contains("Embedding"), "unerwartete Meldung: {}", err);

        let mut ohne = gueltige_stages();
        ohne[0]["has_embedding"] = serde_json::json!(false);
        assert!(load_json("kein_embed", ohne).is_err());
    }

    #[test]
    fn genau_eine_lm_head_stage() {
        let mut stages = gueltige_stages();
        stages[0]["has_lm_head"] = serde_json::json!(true);
        let err = load_json("zwei_head", stages).unwrap_err();
        assert!(err.contains("LM-Head"), "unerwartete Meldung: {}", err);
    }

    #[test]
    fn fehlende_datei_meldet_fehler() {
        let err = PipelineManifest::load("/nicht/vorhanden/manifest.json").unwrap_err();
        assert!(err.contains("Fehler beim Lesen"));
    }

    #[test]
    fn kaputtes_json_meldet_fehler() {
        let dir = std::env::temp_dir().join("myelith-pipeline-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kaputt.json");
        std::fs::write(&path, "{ kein json").unwrap();
        let err = PipelineManifest::load(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("Invalid JSON"));
    }

    #[test]
    fn theta_v_pruefung() {
        let m = load_json("theta", gueltige_stages()).expect("laedt");
        assert!(m.verify_theta_v("abc123").is_ok());
        assert!(m.verify_theta_v("anders").is_err());
    }

    #[test]
    fn stage_nachbarschaft() {
        let m = load_json("nachbarn", gueltige_stages()).expect("laedt");

        assert_eq!(m.next_stage(0).map(|s| s.stage_id), Some(1));
        assert_eq!(m.next_stage(1).map(|s| s.stage_id), Some(2));
        assert!(m.next_stage(2).is_none(), "letzte Stage hat keinen Nachfolger");

        assert!(m.prev_stage(0).is_none(), "erste Stage hat keinen Vorgaenger");
        assert_eq!(m.prev_stage(1).map(|s| s.stage_id), Some(0));
        assert_eq!(m.prev_stage(2).map(|s| s.stage_id), Some(1));
    }

    #[test]
    fn einzelne_stage_ist_gueltig() {
        let m = load_json("einzeln", vec![stage(0, 0, 24, true, true)]).expect("laedt");
        assert_eq!(m.stages.len(), 1);
        assert!(m.next_stage(0).is_none());
        assert!(m.prev_stage(0).is_none());
    }
}
