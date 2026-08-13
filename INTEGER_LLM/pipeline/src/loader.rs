//! Shard-spezifische Modell-Ladung pro Stage (Punkt 12.57).
//!
//! Jede Stage lädt ihre Modell-Bausteine ausgehend vom Stage-Manifest:
//! Layer-Bereich, Embedding-/LM-Head-Zugehörigkeit und θ_v-Hash-
//! Validierung kommen aus dem Manifest. Die Gewichte selbst stammen aus
//! dem Artefakt-Verzeichnis; der Layer-Bereich wird gegen das Manifest
//! validiert.
//!
//! Hinweis zum Speicher: Aktuell wird das vollständige Modell geladen
//! und je Stage nur der zugewiesene Layer-Bereich ausgeführt — das
//! Artefakt-Format kennt noch keine Layer-einzelnen Dateien. Das
//! Speichern/Sharden der Gewichte pro Stage (Manifest-Felder
//! `weights_hash`/`scales_hash` bilden dafür bereits den Vertrag) ist
//! eine Folge-Optimierung; an der Ausführung und Determinismus-Garantie
//! ändert sie nichts.

use std::path::Path;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;

use crate::manifest::{PipelineManifest, StageManifest};

/// Lädt das Modell für eine Stage und validiert den Layer-Bereich
/// gegen das Manifest.
pub fn load_stage_model(
    artifact_dir: &Path,
    stage: &StageManifest,
    pipeline: &PipelineManifest,
) -> Result<IntegerModel, String> {
    let model = load_model(artifact_dir)
        .map_err(|e| format!("Modell-Ladung fehlgeschlagen: {}", e))?;

    // Layer-Bereich gegen das Manifest und das Modell validieren.
    if stage.layer_start >= stage.layer_end {
        return Err(format!(
            "Stage {}: leerer Layer-Bereich [{}, {})",
            stage.stage_id, stage.layer_start, stage.layer_end
        ));
    }
    if stage.layer_end > model.num_layers {
        return Err(format!(
            "Stage {}: Layer-Bereich [{}, {}) überschreitet das Modell ({} Layer)",
            stage.stage_id, stage.layer_start, stage.layer_end, model.num_layers
        ));
    }

    // Embedding/LM-Head-Konsistenz: Stage 0 muss Layer 0 enthalten,
    // die letzte Stage den letzten Layer.
    if stage.has_embedding && stage.layer_start != 0 {
        return Err(format!(
            "Stage {}: Embedding erwartet, beginnt aber bei Layer {}",
            stage.stage_id, stage.layer_start
        ));
    }
    if stage.has_lm_head && stage.layer_end != model.num_layers {
        return Err(format!(
            "Stage {}: LM-Head erwartet, endet aber bei Layer {}",
            stage.stage_id, stage.layer_end
        ));
    }

    // θ_v-Konsistenz: Der kanonische θ_v-Identifikator des geladenen
    // Modells (SHA-256 über version|weights|scales|luts) muss mit dem
    // Manifest übereinstimmen — sonst wären die Gewichte unter einem
    // anderen numerischen Vertrag kalibriert als vom Pod erwartet.
    let canonical = crate::stage::canonical_theta_v_id(&model.theta_v);
    if pipeline.theta_v_hash != canonical {
        return Err(format!(
            "Stage {}: theta_v-Hash-Mismatch (Manifest {}, Modell {})",
            stage.stage_id, pipeline.theta_v_hash, canonical
        ));
    }

    Ok(model)
}
