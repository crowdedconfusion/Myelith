//! Modell-Lader mit theta_v-Validierung

use std::path::Path;
use std::collections::HashMap;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use crate::model::{IntegerModel, LayerScales, ModelConfig, QTensor, TransformerLayer};

/// Zur Kompilierzeit eingebettete Ausfuehrungsspezifikation (Kap. 6.5 des
/// Whitepapers: Teil von theta_v, damit konsensrelevant). Bewusst per
/// `include_str!` statt zur Laufzeit von der Platte gelesen: Die Spezifikation,
/// gegen die validiert wird, muss die sein, die tatsaechlich in diesem Binary
/// kompiliert ist - nicht eine Datei, die seit dem letzten Build editiert
/// worden sein koennte.
const SPEC_JSON: &str = include_str!("../../theta_v/spec.json");

/// SHA-256 der eingebetteten spec.json, zu Diagnose-/Audit-Zwecken.
pub fn spec_hash() -> String {
    sha256_hex(SPEC_JSON.as_bytes())
}

/// theta_v-Version aus der eingebetteten spec.json.
fn spec_version() -> Result<String, String> {
    let parsed: serde_json::Value = serde_json::from_str(SPEC_JSON)
        .map_err(|e| format!("Eingebettetes theta_v/spec.json ist ungueltig: {}", e))?;
    parsed["theta_v"]["version"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "theta_v/spec.json: Feld theta_v.version fehlt".to_string())
}

#[derive(Debug, Clone)]
pub struct ThetaV {
    pub version: String,
    pub weights_hash: String,
    pub scales_hash: String,
    pub luts_hash: String,
}

impl ThetaV {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, String> {
        let manifest_path = dir.as_ref().join("theta_v.json");
        let content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Fehler beim Lesen: {}", e))?;
        let manifest: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON: {}", e))?;

        Ok(ThetaV {
            version: manifest["version"].as_str().unwrap_or("unknown").to_string(),
            weights_hash: manifest["weights_hash"].as_str().unwrap_or("").to_string(),
            scales_hash: manifest["scales_hash"].as_str().unwrap_or("").to_string(),
            luts_hash: manifest["luts_hash"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Prueft, dass das Artefakt gegen dieselbe theta_v-Version kalibriert
    /// wurde, die in diesem Binary als Ausfuehrungsspezifikation eingebettet
    /// ist. Ein Versions-Mismatch bedeutet: Gewichte/Skalen/LUTs koennten
    /// unter anderen Annahmen (Bitbreiten, LUT-Bereiche, Rundungsregeln)
    /// erzeugt worden sein als das, was dieser Runtime-Build tatsaechlich
    /// ausfuehrt - stillschweigendes Laden waere hier gefaehrlicher als ein
    /// fehlgeschlagenes Laden.
    pub fn verify_version_against_spec(&self) -> Result<(), String> {
        let expected = spec_version()?;
        if self.version != expected {
            return Err(format!(
                "theta_v-Version des Artefakts ({}) stimmt nicht mit der in diesem Binary \
                 eingebetteten Ausfuehrungsspezifikation ({}) ueberein",
                self.version, expected
            ));
        }
        Ok(())
    }

    /// Prueft die im Artefakt deklarierten Hashes gegen tatsaechlich
    /// berechnete Hashes der geladenen Manifest-Dateien.
    pub fn verify(&self, weights_hash: &str, scales_hash: &str, luts_hash: &str) -> Result<(), String> {
        if self.weights_hash != weights_hash {
            return Err("weights hash mismatch".to_string());
        }
        if self.scales_hash != scales_hash {
            return Err("scales hash mismatch".to_string());
        }
        if self.luts_hash != luts_hash {
            return Err("luts hash mismatch".to_string());
        }
        Ok(())
    }
}

/// Ein Eintrag in `weights_manifest.json` (Format: calibrate/src/export_weights.py).
#[derive(Debug, Clone, Deserialize)]
pub struct WeightManifestEntry {
    pub original_name: String,
    pub file: String,
    pub shape: Vec<usize>,
    pub scale: f64,
    pub shift: i64,
    pub dtype: String,
    pub hash: String,
    /// Nur für Per-Channel-Tensoren (spec-Ausnahme 0.6.0, LM-Head):
    /// Name der Datei mit einem int8-Shift je Zeile. scale/shift des
    /// Eintrags sind dann Sentinels (-1).
    #[serde(default)]
    pub shifts_file: Option<String>,
    /// SHA-256 der Shifts-Datei (nur Per-Channel-Tensoren).
    #[serde(default)]
    pub shifts_hash: Option<String>,
}

/// Ein geladener INT8-Tensor mit seinen Manifest-Metadaten.
#[derive(Debug)]
pub struct LoadedWeight {
    pub tensor: QTensor,
    pub original_name: String,
    pub scale: f64,
}

/// INT16-LM-Head mit Per-Channel-Skalen (benannte spec-Ausnahme 0.6.0:
/// Eskalation nach dem Entscheidungspunkt 12.21). Ein Shift je Zeile
/// (= Vokabular-Eintrag); die Logits werden zeilenweise auf die gemeinsame
/// Logit-Skala reskaliert (i64-Akkumulation, siehe model.rs).
#[derive(Debug)]
pub struct LmHead {
    pub data: Vec<i16>,    // flat, row-major [vocab, hidden]
    pub shape: Vec<usize>,
    pub shifts: Vec<u8>,   // ein Zweierpotenz-Shift je Zeile
}

/// Attention-Bias in int16 mit einer Zweierpotenz-Skala je Element
/// (theta_v 0.13.0, Fund 23).
///
/// Bis 0.12.0 lagen Biases in int8 und saettigten dort STILL bei Betraegen
/// ueber 127: `quantize_symmetric_int8_per_channel` haette dafuer einen
/// negativen Shift gebraucht, den die Implementierung auf 0 klemmte, und
/// schnitt den Wert danach kommentarlos auf 127 ab. Qwen2.5-7B traf das in
/// `k_proj.bias` der Ebenen 0 und 27 (Spitzenwerte 414 und 171, also 69 %
/// bzw. 26 % Verlust) — und weil Ebene 0 betroffen war, verfaelschte der
/// Fehler die Attention ab dem ersten Layer und propagierte durch alle 28.
///
/// Betroffen waren ausschliesslich Biases (16 von 129 024 Elementen), keine
/// einzige Gewichtszeile (0 von 1 694 720). Biases sind 1D und winzig,
/// int16 kostet daher kaum Artefaktgroesse.
#[derive(Debug, Clone)]
pub struct BiasTensor {
    pub data: Vec<i16>,
    pub shifts: Vec<u8>,   // ein Shift je Element
}

/// Alle Gewichte eines Artefakt-Verzeichnisses, indexiert ueber den
/// Manifest-Key (Tensorname mit Unterstrichen statt Punkten).
#[derive(Debug)]
pub struct LoadedWeights {
    pub weights: HashMap<String, LoadedWeight>,
    /// INT16/Per-Channel-LM-Head (spec-Ausnahme 0.6.0). `None` bei Artefakten
    /// ohne eigenen LM-Head (ältere Artefakte mit Weight-Tying).
    pub lm_head: Option<LmHead>,
    /// INT16-Attention-Biases je Manifest-Key (theta_v 0.13.0, Fund 23).
    /// Leer bei aelteren Artefakten, deren Biases noch int8 waren.
    pub biases: HashMap<String, BiasTensor>,
}

impl LoadedWeights {
    /// Sucht einen Tensor ueber seinen HF-Originalnamen (Punkte werden zu
    /// Unterstrichen normalisiert, wie im Manifest-Key aus
    /// calibrate/src/export_weights.py).
    pub fn get(&self, original_name: &str) -> Option<&QTensor> {
        let key = original_name.replace('.', "_");
        self.weights.get(&key).map(|w| &w.tensor)
    }
}

/// Ein Eintrag in `luts.json` (Format: calibrate/src/export.py, `export_theta_v`).
#[derive(Debug, Clone, Deserialize)]
pub struct LutManifestEntry {
    pub file: String,
    pub hash: String,
    pub length: usize,
    pub dtype: String,
}

/// Alle Lookup-Tabellen eines Artefakt-Verzeichnisses, indexiert ueber den
/// Manifest-Key (`rsqrt`, `silu`, `exp`, `sin`, `cos`).
#[derive(Debug)]
pub struct LoadedLuts {
    pub tables: HashMap<String, Vec<i16>>,
}

impl LoadedLuts {
    pub fn get(&self, name: &str) -> Option<&Vec<i16>> {
        self.tables.get(name)
    }
}

/// Ein Eintrag in `scales.json` (Format: calibrate/src/scales.py, `compute_scales_from_stats`).
#[derive(Debug, Clone, Deserialize)]
pub struct ScaleEntry {
    pub shift: i64,
    pub scale: f64,
    pub absmax_observed: f64,
    /// Per-Kanal-Shifts (Fund 20, theta_v 0.11.0). Nur fuer die drei
    /// Residualstrom-Segmente (`*.input_layernorm.input`,
    /// `*.post_attention_layernorm.input`, `model.norm.input`) gesetzt -
    /// alle anderen Skalen bleiben Skalar. `#[serde(default)]`, damit
    /// Artefakte vor v0.12.44 (kein Feld) weiterhin laden: `shift` allein
    /// wird dann als uniformer Wert fuer alle Kanaele interpretiert
    /// (bitgleich, siehe `rmsnorm.rs::test_rmsnorm_per_channel_uniform_shifts_matches_legacy`).
    #[serde(default)]
    pub shifts: Option<Vec<i64>>,
}

/// Alle Aktivierungsskalen eines Artefakt-Verzeichnisses, indexiert ueber den
/// Layer-/Modul-Namen (z. B. "model.layers.0.self_attn.q_proj").
#[derive(Debug)]
pub struct LoadedScales {
    pub scales: HashMap<String, ScaleEntry>,
}

impl LoadedScales {
    /// Rechts-Shift fuer die Reskalierung des benannten Layers/Moduls.
    pub fn shift(&self, name: &str) -> Option<u8> {
        self.scales.get(name).map(|e| e.shift as u8)
    }

    /// Per-Kanal-Shifts fuer ein Residualstrom-Segment (Fund 20). Liefert
    /// `entry.shifts`, falls kalibriert; sonst den Skalar-`shift` uniform
    /// auf `n` Kanaele verbreitert (bitgleiches Fallback-Verhalten fuer
    /// Artefakte vor v0.12.44).
    pub fn shifts_per_channel(&self, name: &str, n: usize) -> Option<Result<Vec<u8>, String>> {
        let entry = self.scales.get(name)?;
        if let Some(shifts) = &entry.shifts {
            if shifts.len() != n {
                return Some(Err(format!(
                    "{}: {} Per-Kanal-Shifts, erwartet {}", name, shifts.len(), n
                )));
            }
            let mut out = Vec::with_capacity(n);
            for &s in shifts {
                if !(0..=255).contains(&s) {
                    return Some(Err(format!("{}: Shift {} liegt ausserhalb von 0..=255", name, s)));
                }
                out.push(s as u8);
            }
            Some(Ok(out))
        } else {
            Some(Ok(vec![entry.shift as u8; n]))
        }
    }
}

/// Modell-Dimensionen aus `model_config.json` (ein Eintrag je Artefakt,
/// Spiegel der `model`-Sektion aus `theta_v/spec.json` bzw. der Eintraege in
/// `calibrate/src/model_configs.py`). Ersetzt die zuvor in `load_model()`
/// hartkodierten Rust-Literale, damit ein Wechsel auf eine groessere
/// Qwen2.5-Variante ein Config-Wechsel bleibt statt einer Codeaenderung.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelDims {
    pub family: String,
    pub variant: String,
    pub num_layers: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_heads: usize,
    /// Anzahl Key/Value-Heads (Grouped-Query-Attention). Bei Modellen ohne
    /// GQA identisch zu `num_heads`. Qwen2.5-0.5B: 2 (gegenueber 14 Query-Heads,
    /// siehe `models/Qwen2.5-0.5B/config.json`, Feld `num_key_value_heads`).
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
    pub max_context: usize,
    /// Ob LM-Head und Embedding-Tabelle dasselbe Gewicht teilen (HF-Feld
    /// `tie_word_embeddings`). Bei Qwen2.5-0.5B `true` - der Export enthaelt
    /// dann kein eigenes `lm_head.weight`.
    pub tie_word_embeddings: bool,
    /// Ob die Attention-Projektionen q/k/v Biases besitzen (HF-Feld
    /// `attention_bias` im Modell-Config; Qwen2.5-0.5B: `true`). Bei `true`
    /// muessen im Artefakt die Tensoren `*.self_attn.{q,k,v}_proj.bias`
    /// vorliegen - fehlen sie, scheitert das Laden laut statt still
    /// (ausserplanmaessiger Patch v0.12.19, Beschluss mit dem Projektinhaber:
    /// explizit per model_config wie `num_kv_heads`/`tie_word_embeddings`).
    pub attention_bias: bool,
}

/// Laedt und validiert die Modell-Dimensionen aus `model_config.json`.
pub fn load_model_dims(artifact_dir: &Path) -> Result<ModelDims, String> {
    let path = artifact_dir.join("model_config.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Fehler beim Lesen von model_config.json: {}", e))?;
    let dims: ModelDims = serde_json::from_str(&content)
        .map_err(|e| format!("Ungueltiges model_config.json: {}", e))?;

    if dims.num_layers == 0
        || dims.hidden_size == 0
        || dims.num_heads == 0
        || dims.num_kv_heads == 0
        || dims.head_dim == 0
        || dims.vocab_size == 0
    {
        return Err("model_config.json: alle Modell-Dimensionen muessen > 0 sein".to_string());
    }
    if dims.hidden_size != dims.num_heads * dims.head_dim {
        return Err(format!(
            "model_config.json: hidden_size ({}) != num_heads ({}) * head_dim ({})",
            dims.hidden_size, dims.num_heads, dims.head_dim
        ));
    }
    if !dims.num_heads.is_multiple_of(dims.num_kv_heads) {
        return Err(format!(
            "model_config.json: num_heads ({}) ist kein Vielfaches von num_kv_heads ({}) (GQA-Gruppierung nicht moeglich)",
            dims.num_heads, dims.num_kv_heads
        ));
    }

    Ok(dims)
}

/// Laedt alle INT8-Gewichte aus `weights_manifest.json` und den darin
/// referenzierten `.bin`-Dateien (raw int8, row-major, little-endian).
///
/// Validiert pro Tensor dtype, Form, Dateigroesse und den SHA-256-Hash gegen
/// das Manifest. Fehlerhafte Artefakte werden komplett abgelehnt.
pub fn load_weights(artifact_dir: &Path) -> Result<LoadedWeights, String> {
    let manifest_path = artifact_dir.join("weights_manifest.json");
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Fehler beim Lesen von weights_manifest.json: {}", e))?;
    let entries: HashMap<String, WeightManifestEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("Ungueltiges weights_manifest.json: {}", e))?;

    let mut weights = HashMap::with_capacity(entries.len());
    let mut lm_head: Option<LmHead> = None;
    let mut biases: HashMap<String, BiasTensor> = HashMap::new();
    for (name, entry) in entries {
        // INT16-Tensoren: der LM-Head (spec-Ausnahme 0.6.0) und seit
        // theta_v 0.13.0 die Attention-Biases (Fund 23, siehe BiasTensor).
        if entry.dtype == "int16" {
            let ist_bias = name.ends_with("_bias");
            if name != "lm_head" && !ist_bias {
                return Err(format!(
                    "{}: int16 ist nur fuer den LM-Head und Attention-Biases zulaessig",
                    name
                ));
            }
            let shifts_file = entry.shifts_file.as_ref().ok_or_else(|| {
                format!("{}: int16-Eintrag ohne shifts_file", name)
            })?;
            if !ist_bias && entry.shape.len() != 2 {
                return Err(format!("{}: LM-Head erwartet shape [vocab, hidden]", name));
            }
            if ist_bias && entry.shape.len() != 1 {
                return Err(format!("{}: Bias erwartet eindimensionale shape", name));
            }

            let bytes = std::fs::read(artifact_dir.join(&entry.file))
                .map_err(|e| format!("Fehler beim Lesen von {}: {}", entry.file, e))?;
            let expected_len: usize = entry.shape.iter().product::<usize>() * 2;
            if bytes.len() != expected_len {
                return Err(format!(
                    "{}: {} Bytes in '{}', aber shape {:?} erwartet {} Bytes (int16)",
                    name, bytes.len(), entry.file, entry.shape, expected_len
                ));
            }
            let digest = sha256_hex(&bytes);
            if digest != entry.hash {
                return Err(format!(
                    "{}: SHA-256 {} stimmt nicht mit Manifest-Hash {} ueberein",
                    name, digest, entry.hash
                ));
            }

            let shift_bytes = std::fs::read(artifact_dir.join(shifts_file))
                .map_err(|e| format!("Fehler beim Lesen von {}: {}", shifts_file, e))?;
            if shift_bytes.len() != entry.shape[0] {
                return Err(format!(
                    "{}: {} Shifts in '{}', aber {} Zeilen erwartet",
                    name, shift_bytes.len(), shifts_file, entry.shape[0]
                ));
            }
            if let Some(expected_shifts_hash) = &entry.shifts_hash {
                let shifts_digest = sha256_hex(&shift_bytes);
                if shifts_digest != *expected_shifts_hash {
                    return Err(format!(
                        "{}: SHA-256 der Shifts-Datei {} stimmt nicht mit Manifest-Hash {} ueberein",
                        name, shifts_digest, expected_shifts_hash
                    ));
                }
            }

            // little-endian i16, ein Wert je zwei Bytes. Die Laengenpruefung
            // stand hier bisher nicht: `chunks_exact` verwarf ein einzelnes
            // Restbyte stillschweigend und lud ein um ein halbes Element
            // gekuerztes Gewicht. Eine beschaedigte Datei muss auffallen.
            if bytes.len() % 2 != 0 {
                return Err(format!(
                    "{}: Datei hat ungerade Byteanzahl ({}), kann keine \
                     i16-Folge sein",
                    name,
                    bytes.len()
                ));
            }
            // `unknown_lints` muss mit erlaubt sein: Den Lint-Namen gibt es
            // erst ab clippy 1.98, ein `allow` darauf ist auf aelteren
            // Werkzeugketten selbst eine Warnung.
            //
            // `as_chunks::<2>()` waere der Vorschlag, ist aber erst seit Rust
            // 1.88 stabil. Die Schwester-Crates erklaeren MSRV 1.82; dieses
            // hier hat keine Angabe, und ein stillschweigend hoeherer Bedarf
            // waere schlimmer als eine ausdrueckliche Ausnahme.
            #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
            let data: Vec<i16> = bytes
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            if ist_bias {
                biases.insert(name, BiasTensor { data, shifts: shift_bytes });
            } else {
                lm_head = Some(LmHead {
                    data,
                    shape: entry.shape,
                    shifts: shift_bytes,
                });
            }
            continue;
        }

        if entry.dtype != "int8" {
            return Err(format!(
                "{}: nicht unterstuetzter dtype '{}' (erwartet 'int8')",
                name, entry.dtype
            ));
        }
        if entry.shape.is_empty() {
            return Err(format!("{}: leere shape im Manifest", name));
        }

        let bytes = std::fs::read(artifact_dir.join(&entry.file))
            .map_err(|e| format!("Fehler beim Lesen von {}: {}", entry.file, e))?;

        let expected_len: usize = entry.shape.iter().product();
        if bytes.len() != expected_len {
            return Err(format!(
                "{}: {} Bytes in '{}', aber shape {:?} erwartet {} Bytes",
                name, bytes.len(), entry.file, entry.shape, expected_len
            ));
        }

        let digest = sha256_hex(&bytes);
        if digest != entry.hash {
            return Err(format!(
                "{}: SHA-256 {} stimmt nicht mit Manifest-Hash {} ueberein",
                name, digest, entry.hash
            ));
        }

        // Per-Channel-Shifts (theta_v 0.7.0): eine shifts_file mit einem
        // Shift je Zeile. Aeltere Artefakte/Synthetik-Fixtures ohne
        // shifts_file tragen einen uniformen entry.shift, der je Zeile
        // repliziert wird.
        let shifts: Vec<u8> = if let Some(shifts_file) = &entry.shifts_file {
            let shift_bytes = std::fs::read(artifact_dir.join(shifts_file))
                .map_err(|e| format!("Fehler beim Lesen von {}: {}", shifts_file, e))?;
            if shift_bytes.len() != entry.shape[0] {
                return Err(format!(
                    "{}: {} Shifts in '{}', aber {} Zeilen erwartet",
                    name, shift_bytes.len(), shifts_file, entry.shape[0]
                ));
            }
            if let Some(expected_shifts_hash) = &entry.shifts_hash {
                let shifts_digest = sha256_hex(&shift_bytes);
                if shifts_digest != *expected_shifts_hash {
                    return Err(format!(
                        "{}: SHA-256 der Shifts-Datei {} stimmt nicht mit Manifest-Hash {} ueberein",
                        name, shifts_digest, expected_shifts_hash
                    ));
                }
            }
            shift_bytes
        } else {
            if entry.shift < 0 || entry.shift > u8::MAX as i64 {
                return Err(format!(
                    "{}: shift {} liegt ausserhalb von 0..=255 (und keine shifts_file vorhanden)",
                    name, entry.shift
                ));
            }
            vec![entry.shift as u8; entry.shape[0]]
        };

        let tensor = QTensor {
            data: bytes.into_iter().map(|b| b as i8).collect(),
            shape: entry.shape,
            shifts,
        };
        weights.insert(name, LoadedWeight {
            tensor,
            original_name: entry.original_name,
            scale: entry.scale,
        });
    }

    Ok(LoadedWeights { weights, lm_head, biases })
}

/// Laedt alle Lookup-Tabellen aus `luts.json` und den darin referenzierten
/// `.lut.bin`-Dateien (raw int16, little-endian, Format `struct.pack("<Nh", ...)`
/// aus `calibrate/src/export.py`).
///
/// Validiert pro Tabelle dtype, Laenge und den SHA-256-Hash gegen das
/// Manifest. Fehlerhafte Artefakte werden komplett abgelehnt, analog zu
/// `load_weights`.
pub fn load_luts(artifact_dir: &Path) -> Result<LoadedLuts, String> {
    let manifest_path = artifact_dir.join("luts.json");
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Fehler beim Lesen von luts.json: {}", e))?;
    let entries: HashMap<String, LutManifestEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("Ungueltiges luts.json: {}", e))?;

    let mut tables = HashMap::with_capacity(entries.len());
    for (name, entry) in entries {
        if entry.dtype != "int16" {
            return Err(format!(
                "{}: nicht unterstuetzter dtype '{}' (erwartet 'int16')",
                name, entry.dtype
            ));
        }

        let bytes = std::fs::read(artifact_dir.join(&entry.file))
            .map_err(|e| format!("Fehler beim Lesen von {}: {}", entry.file, e))?;

        let expected_bytes = entry.length * 2;
        if bytes.len() != expected_bytes {
            return Err(format!(
                "{}: {} Bytes in '{}', aber length {} erwartet {} Bytes",
                name, bytes.len(), entry.file, entry.length, expected_bytes
            ));
        }

        let digest = sha256_hex(&bytes);
        if digest != entry.hash {
            return Err(format!(
                "{}: SHA-256 {} stimmt nicht mit Manifest-Hash {} ueberein",
                name, digest, entry.hash
            ));
        }

        // struct-unpack "<Nh": little-endian i16, ein Wert pro zwei Bytes.
        // Laengenpruefung und `allow`: siehe Begruendung beim Gewichtsladen.
        if bytes.len() % 2 != 0 {
            return Err(format!(
                "{}: LUT-Datei hat ungerade Byteanzahl ({}), kann keine \
                 i16-Folge sein",
                name,
                bytes.len()
            ));
        }
        #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
        let values: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();

        tables.insert(name, values);
    }

    Ok(LoadedLuts { tables })
}

/// Laedt alle Aktivierungsskalen aus `scales.json` (Format: calibrate/src/scales.py).
///
/// Jeder Eintrag traegt einen Zweierpotenz-Shift sowie den daraus abgeleiteten
/// Faktor `scale = 2^shift`. Die Skalenwahl selbst ist Aufgabe der Kalibrierung
/// (`calibrate/`); der Loader validiert nur Wertebereich und die Konsistenz
/// zwischen `shift` und `scale` und lehnt widerspruechliche Artefakte ab.
pub fn load_scales(artifact_dir: &Path) -> Result<LoadedScales, String> {
    let path = artifact_dir.join("scales.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Fehler beim Lesen von scales.json: {}", e))?;
    let entries: HashMap<String, ScaleEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("Ungueltiges scales.json: {}", e))?;

    for (name, entry) in &entries {
        if entry.shift < 0 || entry.shift > u8::MAX as i64 {
            return Err(format!(
                "{}: shift {} liegt ausserhalb von 0..=255",
                name, entry.shift
            ));
        }

        // shift ist frac_bits (Laufzeit-Konvention: real ≈ quantized >> shift,
        // siehe calibrate/src/scales.py); scale ist die zugehoerige
        // Dequantisierungs-Konstante 2^-shift, nicht 2^shift.
        let expected_scale = 2f64.powi(-(entry.shift as i32));
        if (entry.scale - expected_scale).abs() > expected_scale * 1e-9 {
            return Err(format!(
                "{}: scale {} ist keine Zweierpotenz zu shift {} (erwartet {})",
                name, entry.scale, entry.shift, expected_scale
            ));
        }
    }

    Ok(LoadedScales { scales: entries })
}

/// SHA-256-Hash eines Byte-Slices als kleingeschriebener Hex-String.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Laedt ein komplettes Modell aus dem Artefakt-Verzeichnis: theta_v-Manifest,
/// Modell-Dimensionen, Gewichte, Aktivierungsskalen und Lookup-Tabellen.
pub fn load_model(artifact_dir: &Path) -> Result<IntegerModel, String> {
    let theta_v = ThetaV::load_from_dir(artifact_dir)?;
    theta_v.verify_version_against_spec()?;

    let dims = load_model_dims(artifact_dir)?;
    let weights = load_weights(artifact_dir)?;
    let scales = load_scales(artifact_dir)?;
    let luts = load_luts(artifact_dir)?;

    let weights_manifest_hash = sha256_hex(
        &std::fs::read(artifact_dir.join("weights_manifest.json"))
            .map_err(|e| format!("Fehler beim Lesen von weights_manifest.json: {}", e))?,
    );
    let scales_file_hash = sha256_hex(
        &std::fs::read(artifact_dir.join("scales.json"))
            .map_err(|e| format!("Fehler beim Lesen von scales.json: {}", e))?,
    );
    let luts_file_hash = sha256_hex(
        &std::fs::read(artifact_dir.join("luts.json"))
            .map_err(|e| format!("Fehler beim Lesen von luts.json: {}", e))?,
    );
    theta_v.verify(&weights_manifest_hash, &scales_file_hash, &luts_file_hash)?;

    build_model(theta_v, dims, weights, scales, luts)
}

/// Sucht einen Pflicht-Tensor ueber seinen HF-Originalnamen; fehlt er, wird
/// das Artefakt als unvollstaendig abgelehnt statt eine Luecke stillschweigend
/// mit Platzhalterdaten zu fuellen.
fn require_tensor<'a>(weights: &'a LoadedWeights, name: &str) -> Result<&'a QTensor, String> {
    weights
        .get(name)
        .ok_or_else(|| format!("Fehlendes Gewicht im Artefakt: {}", name))
}

fn require_lut(luts: &LoadedLuts, name: &str) -> Result<Vec<i16>, String> {
    luts.get(name)
        .cloned()
        .ok_or_else(|| format!("Fehlende Lookup-Tabelle im Artefakt: {}", name))
}

/// Liest die Modellbau-Konstanten aus der eingebetteten theta_v/spec.json
/// (Single Source of Truth des numerischen Vertrags, theta_v 0.5.0).
fn spec_model_params() -> Result<ModelConfig, String> {
    let parsed: serde_json::Value = serde_json::from_str(SPEC_JSON)
        .map_err(|e| format!("Eingebettetes theta_v/spec.json ist ungueltig: {}", e))?;
    let tv = &parsed["theta_v"];
    let num = |path: &str, node: &serde_json::Value| -> Result<u8, String> {
        node.as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .ok_or_else(|| format!("theta_v/spec.json: {} fehlt oder ist kein u8", path))
    };
    let formats = &tv["numeric"]["formats"];
    let nonlinear = &tv["nonlinear"];
    let silu_range_min = nonlinear["silu"]["input_range"][0]
        .as_i64()
        .ok_or_else(|| "theta_v/spec.json: nonlinear.silu.input_range[0] fehlt".to_string())?;

    Ok(ModelConfig {
        kv_cache_frac_bits: num("numeric.formats.kv_cache.frac_bits", &formats["kv_cache"]["frac_bits"])?,
        score_frac_bits: num("nonlinear.softmax.exp_lut_frac_bits", &nonlinear["softmax"]["exp_lut_frac_bits"])?,
        exp_input_frac: num("nonlinear.softmax.exp_input_frac_bits", &nonlinear["softmax"]["exp_input_frac_bits"])?,
        prob_frac_bits: num("nonlinear.softmax.prob_frac_bits", &nonlinear["softmax"]["prob_frac_bits"])?,
        rope_frac_bits: num("nonlinear.rope.frac_bits", &nonlinear["rope"]["frac_bits"])?,
        silu_in_frac: num("nonlinear.silu.input_frac_bits", &nonlinear["silu"]["input_frac_bits"])?,
        silu_lut_offset: (-silu_range_min) as i16,
        silu_out_frac: num("nonlinear.silu.output_frac_bits", &nonlinear["silu"]["output_frac_bits"])?,
        rsqrt_input_shift: num("nonlinear.rsqrt.input_shift", &nonlinear["rsqrt"]["input_shift"])?,
        rsqrt_output_frac: num("nonlinear.rsqrt.output_frac_bits", &nonlinear["rsqrt"]["output_frac_bits"])?,
        logit_frac_bits: 6, // nur fuer Sampling/Argmax (skaleninvariant)
    })
}

/// Fordert eine kalibrierte Per-Layer-Aktivierungsskala an; fehlt sie,
/// scheitert der Modellbau laut (v0.12.20: der Forward-Pass verbraucht
/// saemtliche Skalen, ein unvollstaendiges scales.json ist kein gueltiges
/// Artefakt).
fn require_scale(scales: &LoadedScales, name: &str) -> Result<u8, String> {
    scales.shift(name).ok_or_else(|| {
        format!("Fehlende kalibrierte Aktivierungsskala in scales.json: {}", name)
    })
}

/// Wie `require_scale`, aber fuer ein Residualstrom-Segment mit
/// Per-Kanal-Shifts (Fund 20). `n` ist `hidden_size`.
fn require_scale_pc(scales: &LoadedScales, name: &str, n: usize) -> Result<Vec<u8>, String> {
    scales.shifts_per_channel(name, n).ok_or_else(|| {
        format!("Fehlende kalibrierte Aktivierungsskala in scales.json: {}", name)
    })?
}

/// Baut ein vollstaendiges [`IntegerModel`] aus bereits geladenen Artefakten.
///
/// Erwartet HF-Tensornamen, wie sie `calibrate/src/quantize.py` erzeugt (z. B.
/// `model.layers.0.self_attn.q_proj.weight`). Bei `tie_word_embeddings = true`
/// wird kein eigenstaendiges `lm_head.weight` gesucht, sondern die
/// Embedding-Tabelle wiederverwendet - Qwen2.5-0.5B exportiert in diesem Fall
/// kein separates LM-Head-Gewicht (siehe `models/Qwen2.5-0.5B/config.json`).
pub fn build_model(
    theta_v: ThetaV,
    dims: ModelDims,
    mut weights: LoadedWeights,
    scales: LoadedScales,
    luts: LoadedLuts,
) -> Result<IntegerModel, String> {
    let config = spec_model_params()?;

    // INT16-LM-Head (spec-Ausnahme 0.6.0), falls das Artefakt einen trägt.
    let lm_head_int16 = weights.lm_head.take();
    if let Some(lmh) = &lm_head_int16 {
        if lmh.shape.len() != 2 || lmh.shape[0] != dims.vocab_size || lmh.shape[1] != dims.hidden_size {
            return Err(format!(
                "LM-Head-shape {:?} passt nicht zu vocab_size {} / hidden_size {}",
                lmh.shape, dims.vocab_size, dims.hidden_size
            ));
        }
    }

    let embedding_table = require_tensor(&weights, "model.embed_tokens.weight")?.clone();

    let lm_head = if dims.tie_word_embeddings {
        embedding_table.clone()
    } else {
        require_tensor(&weights, "lm_head.weight")?.clone()
    };

    let final_norm_gamma = require_tensor(&weights, "model.norm.weight")?.clone();
    let final_norm_frac = require_scale(&scales, "model.norm")?;
    // Letztes Residualstrom-Segment (spec 0.5.1: Per-Segment-Skalen; seit
    // theta_v 0.11.0 / Fund 20 eine Skala je Kanal statt eine fuer das
    // ganze Segment).
    let final_residual_frac = require_scale_pc(&scales, "model.norm.input", dims.hidden_size)?;

    let mut layers = Vec::with_capacity(dims.num_layers);
    for layer_idx in 0..dims.num_layers {
        let p = format!("model.layers.{}", layer_idx);

        // Attention-Biases: nur bei `attention_bias: true` erwartet, dann
        // aber zwingend (lautes Scheitern statt stiller Abweichung vom
        // Referenzmodell). Bias-Laengen muessen zu den Projektions-Ausgaben
        // passen (q: num_heads*head_dim, k/v: num_kv_heads*head_dim).
        let (q_bias, k_bias, v_bias) = if dims.attention_bias {
            // theta_v 0.13.0 (Fund 23): Biases liegen in int16. Der
            // Manifest-Key traegt Unterstriche statt Punkte.
            let hole_bias = |suffix: &str| -> Result<BiasTensor, String> {
                let key = format!("{}.self_attn.{}.bias", p, suffix).replace('.', "_");
                weights.biases.get(&key).cloned().ok_or_else(|| format!(
                    "Fehlender int16-Bias '{}' im Artefakt. Artefakte vor \
                     theta_v 0.13.0 tragen int8-Biases und muessen neu \
                     kalibriert werden (Fund 23: int8 saettigte still bei \
                     Betraegen ueber 127).", key
                ))
            };
            let qb = hole_bias("q_proj")?;
            let kb = hole_bias("k_proj")?;
            let vb = hole_bias("v_proj")?;
            let q_len = dims.num_heads * dims.head_dim;
            let kv_len = dims.num_kv_heads * dims.head_dim;
            if qb.data.len() != q_len {
                return Err(format!(
                    "Bias-Laenge fuer {}.self_attn.q_proj.bias ({}) passt nicht zu num_heads*head_dim ({})",
                    p, qb.data.len(), q_len
                ));
            }
            if kb.data.len() != kv_len || vb.data.len() != kv_len {
                return Err(format!(
                    "Bias-Laenge fuer {}.self_attn.k/v_proj.bias ({}/{}) passt nicht zu num_kv_heads*head_dim ({})",
                    p, kb.data.len(), vb.data.len(), kv_len
                ));
            }
            (Some(qb), Some(kb), Some(vb))
        } else {
            (None, None, None)
        };

        // Kalibrierte Per-Layer-Aktivierungsskalen (vollstaendig Pflicht,
        // v0.12.20) plus Per-Segment-Skalen des Residualstroms (spec 0.5.1,
        // v0.12.21). Schluessel-Konvention identisch zu
        // calibrate/src/stats.py.
        let layer_scales = LayerScales {
            norm_attn_frac: require_scale(&scales, &format!("{}.input_layernorm", p))?,
            q_frac: require_scale(&scales, &format!("{}.self_attn.q_proj", p))?,
            k_frac: require_scale(&scales, &format!("{}.self_attn.k_proj", p))?,
            v_frac: require_scale(&scales, &format!("{}.self_attn.v_proj", p))?,
            attn_out_frac: require_scale(&scales, &format!("{}.self_attn", p))?,
            norm_mlp_frac: require_scale(&scales, &format!("{}.post_attention_layernorm", p))?,
            gate_frac: require_scale(&scales, &format!("{}.mlp.gate_proj", p))?,
            up_frac: require_scale(&scales, &format!("{}.mlp.up_proj", p))?,
            down_in_frac: require_scale(&scales, &format!("{}.mlp.down_proj.input", p))?,
            residual_in_frac: require_scale_pc(&scales, &format!("{}.input_layernorm.input", p), dims.hidden_size)?,
            residual_mid_frac: require_scale_pc(&scales, &format!("{}.post_attention_layernorm.input", p), dims.hidden_size)?,
        };

        layers.push(TransformerLayer {
            layer_idx,
            input_layernorm_gamma: require_tensor(&weights, &format!("{}.input_layernorm.weight", p))?.clone(),
            post_attention_layernorm_gamma: require_tensor(&weights, &format!("{}.post_attention_layernorm.weight", p))?.clone(),
            q_proj: require_tensor(&weights, &format!("{}.self_attn.q_proj.weight", p))?.clone(),
            k_proj: require_tensor(&weights, &format!("{}.self_attn.k_proj.weight", p))?.clone(),
            v_proj: require_tensor(&weights, &format!("{}.self_attn.v_proj.weight", p))?.clone(),
            o_proj: require_tensor(&weights, &format!("{}.self_attn.o_proj.weight", p))?.clone(),
            gate_proj: require_tensor(&weights, &format!("{}.mlp.gate_proj.weight", p))?.clone(),
            up_proj: require_tensor(&weights, &format!("{}.mlp.up_proj.weight", p))?.clone(),
            down_proj: require_tensor(&weights, &format!("{}.mlp.down_proj.weight", p))?.clone(),
            q_bias,
            k_bias,
            v_bias,
            scales: layer_scales,
        });
    }

    let model = IntegerModel {
        theta_v,
        vocab_size: dims.vocab_size,
        hidden_size: dims.hidden_size,
        num_layers: dims.num_layers,
        num_heads: dims.num_heads,
        num_kv_heads: dims.num_kv_heads,
        head_dim: dims.head_dim,
        max_context: dims.max_context,
        embedding_table,
        lm_head,
        lm_head_int16,
        final_norm_gamma,
        final_norm_frac,
        final_residual_frac,
        layers,
        cos_lut: require_lut(&luts, "cos")?,
        sin_lut: require_lut(&luts, "sin")?,
        exp_lut: require_lut(&luts, "exp")?,
        silu_lut: require_lut(&luts, "silu")?,
        rsqrt_lut: require_lut(&luts, "rsqrt")?,
        // Einmalige Initialisierung (die einzige Division; nicht im
        // tokenweisen Hot-Path, dort wird nur noch multipliziert/geschoben).
        inv_n_q20: integer_llm_kernels::rmsnorm::inv_n_q20(dims.hidden_size),
        activation_scales: scales,
        config,
    };

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Eindeutiges Temp-Verzeichnis pro Test anlegen.
    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("integer-llm-loader-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("Temp-Verzeichnis anlegen");
        dir
    }

    /// Schreibt ein Minimal-Manifest mit einem Tensor.
    fn write_manifest(dir: &Path, key: &str, entry: serde_json::Value) {
        let manifest = serde_json::json!({ key: entry });
        fs::write(
            dir.join("weights_manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .expect("Manifest schreiben");
    }

    fn entry(file: &str, shape: Vec<usize>, shift: i64, hash: &str) -> serde_json::Value {
        serde_json::json!({
            "original_name": file.replace(".bin", "").replace('_', "."),
            "file": file,
            "shape": shape,
            "scale": 1.0,
            "shift": shift,
            "dtype": "int8",
            "hash": hash,
        })
    }

    #[test]
    fn test_sha256_hex_known_vector() {
        // Referenzwert aus FIPS 180-4, identisch zu Python hashlib.sha256(b"abc")
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_load_weights_roundtrip() {
        let dir = test_dir("roundtrip");

        // -128, -1, 0, 1, 127 als raw int8
        let raw: Vec<u8> = vec![0x80, 0xFF, 0x00, 0x01, 0x7F, 0x40];
        fs::write(dir.join("t_a.bin"), &raw).expect("Tensor schreiben");

        write_manifest(
            &dir,
            "t_a",
            entry("t_a.bin", vec![2, 3], 2, &sha256_hex(&raw)),
        );

        let loaded = load_weights(&dir).expect("Laden erfolgreich");
        let tensor = &loaded.weights["t_a"].tensor;
        assert_eq!(tensor.data, vec![-128i8, -1, 0, 1, 127, 64]);
        assert_eq!(tensor.shape, vec![2, 3]);
        // Ohne shifts_file wird der uniforme Manifest-Shift je Zeile repliziert.
        assert_eq!(tensor.shifts, vec![2, 2]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_weights_per_channel_shifts() {
        // theta_v 0.7.0: shifts_file mit einem Shift je Zeile.
        let dir = test_dir("perchannel-shifts");
        let raw: Vec<u8> = vec![1, 2, 3, 4, 5, 6];
        fs::write(dir.join("t_c.bin"), &raw).expect("Tensor schreiben");
        let shifts_raw: Vec<u8> = vec![7, 9];
        fs::write(dir.join("t_c_shifts.bin"), &shifts_raw).expect("Shifts schreiben");

        let mut e = entry("t_c.bin", vec![2, 3], -1, &sha256_hex(&raw));
        e["scale"] = serde_json::json!(-1.0);
        e["shifts_file"] = serde_json::json!("t_c_shifts.bin");
        e["shifts_hash"] = serde_json::json!(sha256_hex(&shifts_raw));
        write_manifest(&dir, "t_c", e);

        let loaded = load_weights(&dir).expect("Laden erfolgreich");
        assert_eq!(loaded.weights["t_c"].tensor.shifts, vec![7, 9]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_weights_rejects_bad_hash() {
        let dir = test_dir("badhash");
        let raw: Vec<u8> = vec![1, 2, 3, 4];
        fs::write(dir.join("t_b.bin"), &raw).expect("Tensor schreiben");

        write_manifest(&dir, "t_b", entry("t_b.bin", vec![4], 0, "0".repeat(64).as_str()));

        let err = load_weights(&dir).expect_err("Hash-Mismatch muss fehlschlagen");
        assert!(err.contains("SHA-256"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_weights_rejects_size_mismatch() {
        let dir = test_dir("badsize");
        let raw: Vec<u8> = vec![1, 2, 3]; // 3 Bytes, Manifest behauptet 4
        fs::write(dir.join("t_c.bin"), &raw).expect("Tensor schreiben");

        write_manifest(&dir, "t_c", entry("t_c.bin", vec![2, 2], 0, &sha256_hex(&raw)));

        let err = load_weights(&dir).expect_err("Groessen-Mismatch muss fehlschlagen");
        assert!(err.contains("Bytes"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_weights_missing_manifest() {
        let dir = test_dir("nomanifest");
        let err = load_weights(&dir).expect_err("Ohne Manifest muss Laden fehlschlagen");
        assert!(err.contains("weights_manifest.json"), "Fehlermeldung: {}", err);
        fs::remove_dir_all(&dir).ok();
    }

    /// Schreibt ein Minimal-`luts.json`-Manifest mit einem Eintrag.
    fn write_luts_manifest(dir: &Path, key: &str, entry: serde_json::Value) {
        let manifest = serde_json::json!({ key: entry });
        fs::write(
            dir.join("luts.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .expect("LUT-Manifest schreiben");
    }

    fn lut_entry(file: &str, length: usize, hash: &str) -> serde_json::Value {
        serde_json::json!({
            "file": file,
            "hash": hash,
            "length": length,
            "dtype": "int16",
        })
    }

    /// Packt i16-Werte wie `struct.pack(f"<{n}h", ...)` in `calibrate/src/export.py`.
    fn pack_i16_le(values: &[i16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn test_load_luts_roundtrip() {
        let dir = test_dir("luts-roundtrip");

        let values: Vec<i16> = vec![256, -1, 0, 32767, -32768];
        let raw = pack_i16_le(&values);
        fs::write(dir.join("exp.lut.bin"), &raw).expect("LUT schreiben");

        write_luts_manifest(&dir, "exp", lut_entry("exp.lut.bin", values.len(), &sha256_hex(&raw)));

        let loaded = load_luts(&dir).expect("Laden erfolgreich");
        assert_eq!(loaded.get("exp"), Some(&values));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_luts_multiple_tables() {
        let dir = test_dir("luts-multi");

        let sin: Vec<i16> = vec![0, 181, 256, 181, 0, -181, -256, -181];
        let cos: Vec<i16> = vec![256, 181, 0, -181, -256, -181, 0, 181];
        let sin_raw = pack_i16_le(&sin);
        let cos_raw = pack_i16_le(&cos);
        fs::write(dir.join("sin.lut.bin"), &sin_raw).expect("LUT schreiben");
        fs::write(dir.join("cos.lut.bin"), &cos_raw).expect("LUT schreiben");

        let manifest = serde_json::json!({
            "sin": lut_entry("sin.lut.bin", sin.len(), &sha256_hex(&sin_raw)),
            "cos": lut_entry("cos.lut.bin", cos.len(), &sha256_hex(&cos_raw)),
        });
        fs::write(dir.join("luts.json"), serde_json::to_string(&manifest).unwrap())
            .expect("LUT-Manifest schreiben");

        let loaded = load_luts(&dir).expect("Laden erfolgreich");
        assert_eq!(loaded.get("sin"), Some(&sin));
        assert_eq!(loaded.get("cos"), Some(&cos));
        assert_eq!(loaded.get("exp"), None);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_luts_rejects_bad_hash() {
        let dir = test_dir("luts-badhash");
        let raw = pack_i16_le(&[1, 2, 3, 4]);
        fs::write(dir.join("silu.lut.bin"), &raw).expect("LUT schreiben");

        write_luts_manifest(&dir, "silu", lut_entry("silu.lut.bin", 4, "0".repeat(64).as_str()));

        let err = load_luts(&dir).expect_err("Hash-Mismatch muss fehlschlagen");
        assert!(err.contains("SHA-256"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_luts_rejects_size_mismatch() {
        let dir = test_dir("luts-badsize");
        let raw = pack_i16_le(&[1, 2, 3]); // 3 Werte, Manifest behauptet 4
        fs::write(dir.join("rsqrt.lut.bin"), &raw).expect("LUT schreiben");

        write_luts_manifest(&dir, "rsqrt", lut_entry("rsqrt.lut.bin", 4, &sha256_hex(&raw)));

        let err = load_luts(&dir).expect_err("Groessen-Mismatch muss fehlschlagen");
        assert!(err.contains("Bytes"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_luts_rejects_wrong_dtype() {
        let dir = test_dir("luts-baddtype");
        let raw = pack_i16_le(&[1, 2]);
        fs::write(dir.join("exp.lut.bin"), &raw).expect("LUT schreiben");

        let mut entry = lut_entry("exp.lut.bin", 2, &sha256_hex(&raw));
        entry["dtype"] = serde_json::json!("int8");
        write_luts_manifest(&dir, "exp", entry);

        let err = load_luts(&dir).expect_err("Falscher dtype muss fehlschlagen");
        assert!(err.contains("dtype"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_luts_missing_manifest() {
        let dir = test_dir("luts-nomanifest");
        let err = load_luts(&dir).expect_err("Ohne Manifest muss Laden fehlschlagen");
        assert!(err.contains("luts.json"), "Fehlermeldung: {}", err);
        fs::remove_dir_all(&dir).ok();
    }

    fn write_scales(dir: &Path, content: &serde_json::Value) {
        fs::write(dir.join("scales.json"), serde_json::to_string(content).unwrap())
            .expect("scales.json schreiben");
    }

    fn scale_entry(shift: i64, scale: f64, absmax: f64) -> serde_json::Value {
        serde_json::json!({ "shift": shift, "scale": scale, "absmax_observed": absmax })
    }

    #[test]
    fn test_load_scales_roundtrip() {
        let dir = test_dir("scales-roundtrip");
        // shift=3 (frac_bits) => scale = 2^-3 = 0.125 (Dequantisierungskonstante,
        // nicht 2^shift - siehe Hinweis zu 12.10/Numerik-Fix).
        let manifest = serde_json::json!({
            "model.layers.0.self_attn.q_proj": scale_entry(3, 0.125, 5.2),
        });
        write_scales(&dir, &manifest);

        let loaded = load_scales(&dir).expect("Laden erfolgreich");
        assert_eq!(loaded.shift("model.layers.0.self_attn.q_proj"), Some(3));
        assert_eq!(loaded.scales["model.layers.0.self_attn.q_proj"].absmax_observed, 5.2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_shifts_per_channel_reads_real_array() {
        // Fund 20: ein Eintrag MIT "shifts"-Array (Residualstrom-Segment,
        // von calibrate/src/scales.py erzeugt) muss genau dieses Array
        // liefern, nicht den Skalar-Fallback.
        let dir = test_dir("scales-per-channel");
        let manifest = serde_json::json!({
            "model.layers.4.input_layernorm.input": {
                "shift": 1, "scale": 0.5, "absmax_observed": 9600.0,
                "shifts": [1, 12, 12, 12]
            },
        });
        write_scales(&dir, &manifest);

        let loaded = load_scales(&dir).expect("Laden erfolgreich");
        let shifts = loaded
            .shifts_per_channel("model.layers.4.input_layernorm.input", 4)
            .expect("Eintrag muss existieren")
            .expect("Laden erfolgreich");
        assert_eq!(shifts, vec![1u8, 12, 12, 12]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_shifts_per_channel_broadcasts_scalar_for_old_artifacts() {
        // Fund 20: ein Eintrag OHNE "shifts"-Feld (Artefakte vor v0.12.44)
        // muss den Skalar-Shift uniform auf n Kanaele verbreitern - bitgleich
        // zur alten Skalar-Behandlung (bewiesen kernseitig in
        // rmsnorm.rs::test_rmsnorm_per_channel_uniform_shifts_matches_legacy).
        let dir = test_dir("scales-per-channel-fallback");
        let manifest = serde_json::json!({
            "model.norm.input": scale_entry(4, 0.0625, 1712.0),
        });
        write_scales(&dir, &manifest);

        let loaded = load_scales(&dir).expect("Laden erfolgreich");
        let shifts = loaded
            .shifts_per_channel("model.norm.input", 5)
            .expect("Eintrag muss existieren")
            .expect("Laden erfolgreich");
        assert_eq!(shifts, vec![4u8; 5]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_shifts_per_channel_rejects_length_mismatch() {
        // Ein "shifts"-Array mit falscher Laenge (z. B. gegen die falsche
        // hidden_size kalibriert) muss laut scheitern, nicht still
        // out-of-bounds zugreifen oder abschneiden.
        let dir = test_dir("scales-per-channel-mismatch");
        let manifest = serde_json::json!({
            "model.norm.input": {
                "shift": 1, "scale": 0.5, "absmax_observed": 100.0,
                "shifts": [1, 2, 3]
            },
        });
        write_scales(&dir, &manifest);

        let loaded = load_scales(&dir).expect("Laden erfolgreich");
        let err = loaded
            .shifts_per_channel("model.norm.input", 4)
            .expect("Eintrag muss existieren")
            .expect_err("Laengen-Mismatch muss fehlschlagen");
        assert!(err.contains("3") && err.contains("4"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_scales_multiple_layers() {
        let dir = test_dir("scales-multi");
        let manifest = serde_json::json!({
            "model.layers.0.self_attn.q_proj": scale_entry(0, 1.0, 0.4),
            "model.layers.0.mlp.gate_proj": scale_entry(5, 0.03125, 20.1),
        });
        write_scales(&dir, &manifest);

        let loaded = load_scales(&dir).expect("Laden erfolgreich");
        assert_eq!(loaded.shift("model.layers.0.self_attn.q_proj"), Some(0));
        assert_eq!(loaded.shift("model.layers.0.mlp.gate_proj"), Some(5));
        assert_eq!(loaded.shift("model.layers.99.does_not_exist"), None);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_scales_rejects_non_power_of_two() {
        let dir = test_dir("scales-badscale");
        // shift=3 verlangt scale=2^-3=0.125, hier absichtlich 0.1 (kein Zweierpotenz-Faktor)
        let manifest = serde_json::json!({
            "model.layers.0.self_attn.k_proj": scale_entry(3, 0.1, 5.0),
        });
        write_scales(&dir, &manifest);

        let err = load_scales(&dir).expect_err("Inkonsistente Skala muss fehlschlagen");
        assert!(err.contains("Zweierpotenz"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_scales_rejects_out_of_range_shift() {
        let dir = test_dir("scales-badshift");
        let manifest = serde_json::json!({
            "model.layers.0.self_attn.v_proj": scale_entry(-1, 0.5, 1.0),
        });
        write_scales(&dir, &manifest);

        let err = load_scales(&dir).expect_err("Negativer shift muss fehlschlagen");
        assert!(err.contains("0..=255"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_scales_missing_manifest() {
        let dir = test_dir("scales-nomanifest");
        let err = load_scales(&dir).expect_err("Ohne Manifest muss Laden fehlschlagen");
        assert!(err.contains("scales.json"), "Fehlermeldung: {}", err);
        fs::remove_dir_all(&dir).ok();
    }

    // --- ModelDims (12.10) ---

    fn model_dims_json(num_heads: i64, num_kv_heads: i64, hidden_size: i64, head_dim: i64, tie: bool, attention_bias: bool) -> serde_json::Value {
        serde_json::json!({
            "family": "qwen2.5",
            "variant": "test",
            "num_layers": 1,
            "hidden_size": hidden_size,
            "intermediate_size": 8,
            "num_heads": num_heads,
            "num_kv_heads": num_kv_heads,
            "head_dim": head_dim,
            "vocab_size": 3,
            "max_context": 8,
            "tie_word_embeddings": tie,
            "attention_bias": attention_bias,
        })
    }

    fn write_model_config(dir: &Path, content: &serde_json::Value) {
        fs::write(dir.join("model_config.json"), serde_json::to_string(content).unwrap())
            .expect("model_config.json schreiben");
    }

    #[test]
    fn test_load_model_dims_roundtrip() {
        let dir = test_dir("dims-roundtrip");
        write_model_config(&dir, &model_dims_json(4, 2, 8, 2, true, true));

        let dims = load_model_dims(&dir).expect("Laden erfolgreich");
        assert_eq!(dims.num_heads, 4);
        assert_eq!(dims.num_kv_heads, 2);
        assert_eq!(dims.hidden_size, 8);
        assert!(dims.tie_word_embeddings);
        assert!(dims.attention_bias);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_dims_rejects_hidden_size_mismatch() {
        let dir = test_dir("dims-badhidden");
        // hidden_size=9, aber num_heads*head_dim = 4*2 = 8
        write_model_config(&dir, &model_dims_json(4, 2, 9, 2, true, true));

        let err = load_model_dims(&dir).expect_err("hidden_size-Mismatch muss fehlschlagen");
        assert!(err.contains("hidden_size"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_dims_rejects_non_divisible_gqa() {
        let dir = test_dir("dims-badgqa");
        // num_heads=5 ist kein Vielfaches von num_kv_heads=2
        write_model_config(&dir, &model_dims_json(5, 2, 10, 2, true, true));

        let err = load_model_dims(&dir).expect_err("Nicht-teilbare GQA-Gruppierung muss fehlschlagen");
        assert!(err.contains("Vielfaches"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_dims_rejects_missing_attention_bias_field() {
        // attention_bias ist ein Pflichtfeld (Beschluss v0.12.19): ein
        // model_config.json ohne das Feld muss laut scheitern, damit kein
        // Artefakt still ohne Bias-Information geladen wird.
        let dir = test_dir("dims-nobiasfield");
        let mut config = model_dims_json(4, 2, 8, 2, true, true);
        config.as_object_mut().unwrap().remove("attention_bias");
        write_model_config(&dir, &config);

        let err = load_model_dims(&dir).expect_err("Fehlendes attention_bias-Feld muss fehlschlagen");
        assert!(err.contains("attention_bias"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_dims_missing_manifest() {
        let dir = test_dir("dims-nomanifest");
        let err = load_model_dims(&dir).expect_err("Ohne Manifest muss Laden fehlschlagen");
        assert!(err.contains("model_config.json"), "Fehlermeldung: {}", err);
        fs::remove_dir_all(&dir).ok();
    }

    // --- LoadedWeights::get (12.10) ---

    #[test]
    fn test_loaded_weights_get_normalizes_dots_to_underscores() {
        let dir = test_dir("weights-get");
        let raw: Vec<u8> = vec![0x01, 0x02];
        fs::write(dir.join("model_norm_weight.bin"), &raw).expect("Tensor schreiben");
        write_manifest(&dir, "model_norm_weight", entry("model_norm_weight.bin", vec![2], 0, &sha256_hex(&raw)));

        let loaded = load_weights(&dir).expect("Laden erfolgreich");
        assert!(loaded.get("model.norm.weight").is_some());
        assert!(loaded.get("does.not.exist").is_none());

        fs::remove_dir_all(&dir).ok();
    }

    // --- build_model / load_model End-to-End (12.10) ---

    /// Baut ein minimales, aber vollstaendiges Artefakt-Verzeichnis: 1 Layer,
    /// hidden_size=4, num_heads=2, num_kv_heads=1, head_dim=2, vocab_size=3,
    /// intermediate_size=4 - klein genug fuer einen schnellen Test, aber mit
    /// derselben GQA-Asymmetrie (num_heads != num_kv_heads) wie das echte
    /// Qwen2.5-0.5B-Modell.
    fn write_full_fixture(dir: &Path, tie_word_embeddings: bool, attention_bias: bool) {
        let hidden = 4usize;
        let heads = 2usize;
        let kv_heads = 1usize;
        let head_dim = 2usize;
        let inter = 4usize;
        let vocab = 3usize;

        write_model_config(dir, &serde_json::json!({
            "family": "qwen2.5",
            "variant": "test",
            "num_layers": 1,
            "hidden_size": hidden,
            "intermediate_size": inter,
            "num_heads": heads,
            "num_kv_heads": kv_heads,
            "head_dim": head_dim,
            "vocab_size": vocab,
            "max_context": 8,
            "tie_word_embeddings": tie_word_embeddings,
            "attention_bias": attention_bias,
        }));

        // Vollstaendige Per-Layer-Aktivierungsskalen (seit v0.12.20 Pflicht:
        // der Forward-Pass verbraucht alle Eintraege; Schluessel-Konvention
        // identisch zu calibrate/src/stats.py) plus Per-Segment-Skalen des
        // Residualstroms (spec 0.5.1).
        let scales = serde_json::json!({
            "model.layers.0.input_layernorm": scale_entry(4, 0.0625, 10.0),
            "model.layers.0.self_attn.q_proj": scale_entry(5, 0.03125, 20.0),
            "model.layers.0.self_attn.k_proj": scale_entry(5, 0.03125, 20.0),
            "model.layers.0.self_attn.v_proj": scale_entry(5, 0.03125, 20.0),
            "model.layers.0.self_attn": scale_entry(6, 0.015625, 15.0),
            "model.layers.0.post_attention_layernorm": scale_entry(3, 0.125, 40.0),
            "model.layers.0.mlp.gate_proj": scale_entry(4, 0.0625, 30.0),
            "model.layers.0.mlp.up_proj": scale_entry(3, 0.125, 60.0),
            "model.layers.0.mlp.down_proj.input": scale_entry(0, 1.0, 100.0),
            "model.layers.0.input_layernorm.input": scale_entry(12, 0.000244140625, 0.06),
            "model.layers.0.post_attention_layernorm.input": scale_entry(5, 0.03125, 25.0),
            "model.norm": scale_entry(2, 0.25, 120.0),
            "model.norm.input": scale_entry(4, 0.0625, 80.0),
        });
        write_scales(dir, &scales);

        // Gewichte
        let mut manifest = serde_json::Map::new();
        let mut put = |original_name: &str, shape: Vec<usize>| {
            let n: usize = shape.iter().product();
            let data: Vec<u8> = (0..n).map(|i| (i % 7) as u8).collect();
            let safe = original_name.replace('.', "_");
            let file = format!("{}.bin", safe);
            fs::write(dir.join(&file), &data).expect("Gewicht schreiben");
            manifest.insert(safe, serde_json::json!({
                "original_name": original_name,
                "file": file,
                "shape": shape,
                "scale": 1.0,
                "shift": 0,
                "dtype": "int8",
                "hash": sha256_hex(&data),
            }));
        };

        put("model.embed_tokens.weight", vec![vocab, hidden]);
        put("model.norm.weight", vec![hidden]);
        if !tie_word_embeddings {
            put("lm_head.weight", vec![vocab, hidden]);
        }
        put("model.layers.0.input_layernorm.weight", vec![hidden]);
        put("model.layers.0.post_attention_layernorm.weight", vec![hidden]);
        put("model.layers.0.self_attn.q_proj.weight", vec![heads * head_dim, hidden]);
        put("model.layers.0.self_attn.k_proj.weight", vec![kv_heads * head_dim, hidden]);
        put("model.layers.0.self_attn.v_proj.weight", vec![kv_heads * head_dim, hidden]);
        put("model.layers.0.self_attn.o_proj.weight", vec![hidden, heads * head_dim]);
        put("model.layers.0.mlp.gate_proj.weight", vec![inter, hidden]);
        put("model.layers.0.mlp.up_proj.weight", vec![inter, hidden]);
        put("model.layers.0.mlp.down_proj.weight", vec![hidden, inter]);

        if attention_bias {
            // Qwen2.5-Format: Bias je q/k/v_proj, Laenge = Ausgabe-Dimension
            // der Projektion (q: heads*head_dim, k/v: kv_heads*head_dim).
            //
            // Seit theta_v 0.13.0 (Fund 23) liegen Biases in int16 mit einer
            // Shifts-Datei je Element — strukturell identisch zum echten
            // Export, damit das Fixture nicht an einem Format testet, das
            // es in der Produktion nicht gibt (Projektkonvention).
            for (original_name, n) in [
                ("model.layers.0.self_attn.q_proj.bias", heads * head_dim),
                ("model.layers.0.self_attn.k_proj.bias", kv_heads * head_dim),
                ("model.layers.0.self_attn.v_proj.bias", kv_heads * head_dim),
            ] {
                let werte: Vec<i16> = (0..n).map(|i| ((i % 11) as i16) - 5).collect();
                let mut data = Vec::with_capacity(n * 2);
                for w in &werte {
                    data.extend_from_slice(&w.to_le_bytes());
                }
                let shifts: Vec<u8> = (0..n).map(|i| (i % 3) as u8).collect();
                let safe = original_name.replace('.', "_");
                let file = format!("{}.bin", safe);
                let shifts_file = format!("{}_shifts.bin", safe);
                fs::write(dir.join(&file), &data).expect("Bias schreiben");
                fs::write(dir.join(&shifts_file), &shifts).expect("Bias-Shifts schreiben");
                manifest.insert(safe, serde_json::json!({
                    "original_name": original_name,
                    "file": file,
                    "shape": [n],
                    "scale": -1.0,
                    "shift": -1,
                    "dtype": "int16",
                    "hash": sha256_hex(&data),
                    "shifts_file": shifts_file,
                    "shifts_hash": sha256_hex(&shifts),
                }));
            }
        }

        fs::write(dir.join("weights_manifest.json"), serde_json::to_string(&manifest).unwrap())
            .expect("weights_manifest.json schreiben");

        // LUTs
        let mut luts_manifest = serde_json::Map::new();
        let mut put_lut = |name: &str, values: Vec<i16>| {
            let raw = pack_i16_le(&values);
            let file = format!("{}.lut.bin", name);
            fs::write(dir.join(&file), &raw).expect("LUT schreiben");
            luts_manifest.insert(name.to_string(), lut_entry(&file, values.len(), &sha256_hex(&raw)));
        };
        put_lut("cos", vec![256, 0, -256, 0]);
        put_lut("sin", vec![0, 256, 0, -256]);
        put_lut("exp", vec![256, 128, 64]);
        put_lut("silu", vec![-10, 0, 10, 20]);
        put_lut("rsqrt", vec![256, 181, 148]);

        fs::write(dir.join("luts.json"), serde_json::to_string(&luts_manifest).unwrap())
            .expect("luts.json schreiben");

        // theta_v.json zuletzt schreiben: Version und Hashes muessen zu den
        // gerade geschriebenen Manifest-Dateien passen (Punkt 12.13).
        write_theta_v(dir);
    }

    /// Schreibt theta_v.json mit der aktuellen spec-Version und echten
    /// Hashes der bereits vorhandenen weights_manifest.json/scales.json/
    /// luts.json - passend zu `ThetaV::verify()` und
    /// `verify_version_against_spec()`.
    fn write_theta_v(dir: &Path) {
        let weights_hash = sha256_hex(&fs::read(dir.join("weights_manifest.json")).expect("weights_manifest.json lesen"));
        let scales_hash = sha256_hex(&fs::read(dir.join("scales.json")).expect("scales.json lesen"));
        let luts_hash = sha256_hex(&fs::read(dir.join("luts.json")).expect("luts.json lesen"));

        fs::write(
            dir.join("theta_v.json"),
            serde_json::to_string(&serde_json::json!({
                "version": spec_version().expect("spec_version"),
                "weights_hash": weights_hash,
                "scales_hash": scales_hash,
                "luts_hash": luts_hash,
            })).unwrap(),
        ).expect("theta_v.json schreiben");
    }

    #[test]
    fn test_load_model_end_to_end_tied_embeddings() {
        let dir = test_dir("model-e2e-tied");
        write_full_fixture(&dir, true, true);

        let model = load_model(&dir).expect("Modell-Laden erfolgreich");

        assert_eq!(model.num_layers, 1);
        assert_eq!(model.num_heads, 2);
        assert_eq!(model.num_kv_heads, 1);
        assert_eq!(model.head_dim, 2);
        assert_eq!(model.hidden_size, 4);
        assert_eq!(model.vocab_size, 3);
        assert_eq!(model.layers.len(), 1);

        // Weight Tying: lm_head muss exakt der Embedding-Tabelle entsprechen,
        // obwohl kein eigenes lm_head.weight im Artefakt lag.
        assert_eq!(model.lm_head.data, model.embedding_table.data);
        assert_eq!(model.lm_head.shape, model.embedding_table.shape);

        // GQA-Asymmetrie muss sich in den geladenen Tensorformen widerspiegeln:
        // q_proj hat num_heads*head_dim=4 Zeilen, k_proj/v_proj nur
        // num_kv_heads*head_dim=2.
        assert_eq!(model.layers[0].q_proj.shape, vec![4, 4]);
        assert_eq!(model.layers[0].k_proj.shape, vec![2, 4]);
        assert_eq!(model.layers[0].v_proj.shape, vec![2, 4]);

        // Attention-Biases (Qwen2.5-Format, attention_bias=true): muessen
        // geladen sein und die Laenge der Projektions-Ausgabe tragen
        // (q: heads*head_dim=4, k/v: kv_heads*head_dim=2).
        assert!(model.layers[0].q_bias.is_some());
        assert!(model.layers[0].k_bias.is_some());
        assert!(model.layers[0].v_bias.is_some());
        assert_eq!(model.layers[0].q_bias.as_ref().unwrap().data.len(), 4);
        assert_eq!(model.layers[0].k_bias.as_ref().unwrap().data.len(), 2);
        assert_eq!(model.layers[0].v_bias.as_ref().unwrap().data.len(), 2);

        assert_eq!(model.cos_lut.len(), 4);
        assert_eq!(model.exp_lut.len(), 3);
        assert_eq!(model.silu_lut.len(), 4);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_end_to_end_untied_embeddings() {
        let dir = test_dir("model-e2e-untied");
        write_full_fixture(&dir, false, true);

        let model = load_model(&dir).expect("Modell-Laden erfolgreich");
        // Ohne Tying muss lm_head aus dem eigenen Gewicht stammen, nicht aus
        // der Embedding-Tabelle (hier bewusst mit anderem Fuellmuster nicht
        // unterscheidbar, da write_full_fixture beide gleich befuellt - der
        // eigentliche Test ist, dass das Laden ohne Fallback funktioniert).
        assert_eq!(model.lm_head.shape, vec![3, 4]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_forward_token_runs_with_gqa_fixture() {
        // End-to-End-Rauchtest: ein geladenes GQA-Modell (num_heads != num_kv_heads)
        // muss durch einen kompletten Forward-Schritt laufen, ohne zu paniken
        // (Index-/Laengen-Fehler waeren hier der typische Fehlerfall bei falscher
        // Head-Gruppierung).
        let dir = test_dir("model-e2e-forward");
        write_full_fixture(&dir, true, true);
        let model = load_model(&dir).expect("Modell-Laden erfolgreich");

        let mut cache = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
        let logits = model.forward_token(0, 0, &mut cache);
        assert_eq!(logits.len(), model.vocab_size);

        // Per-Layer-Skalen muessen aus scales.json verdrahtet sein (v0.12.20),
        // inklusive der Per-Segment-Residualskalen (spec 0.5.1).
        assert_eq!(model.layers[0].scales.q_frac, 5);
        assert_eq!(model.layers[0].scales.down_in_frac, 0);
        // Fund 20: ohne "shifts"-Feld im Fixture-scales.json broadcastet der
        // Loader den Skalar-Shift uniform auf alle Kanaele (hidden_size=4
        // in diesem Fixture) - bitgleiches Fallback-Verhalten.
        assert_eq!(model.layers[0].scales.residual_in_frac, vec![12u8; 4]);
        assert_eq!(model.layers[0].scales.residual_mid_frac, vec![5u8; 4]);
        assert_eq!(model.final_norm_frac, 2);
        assert_eq!(model.final_residual_frac, vec![4u8; 4]);
        // Konfigurationswerte kommen aus der eingebetteten spec.json.
        //
        // Geprueft wird die BEZIEHUNG, nicht der Zahlenwert: Der Offset
        // muss das negative untere Ende des SiLU-Eingangsbereichs sein.
        // Bis 2026-08-20 stand hier `assert_eq!(.., 1024)` — der Wert von
        // theta_v 0.14.0. Beim Sprung auf 0.15.0 (Eingangsraster 1/8 ->
        // 1/64, Bereich [-8192, 8191]) schlug der Test fehl, obwohl der
        // Loader korrekt arbeitete: Er las 8192, wie es sein soll.
        // Ein festverdrahteter theta_v-Wert in einem Test bricht bei
        // jeder Spezifikationsaenderung und sagt dabei nichts darueber,
        // ob der Loader richtig liest.
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../theta_v/spec.json"))
                .expect("eingebettete spec.json");
        let bereich_min = spec["theta_v"]["nonlinear"]["silu"]["input_range"][0]
            .as_i64()
            .expect("silu.input_range[0]");
        assert_eq!(model.config.silu_lut_offset as i64, -bereich_min);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_missing_activation_scale() {
        // Seit v0.12.20 ist jede Per-Layer-Aktivierungsskala Pflicht: fehlt
        // ein Eintrag, muss der Modellbau laut scheitern.
        let dir = test_dir("model-e2e-missingscale");
        write_full_fixture(&dir, true, true);
        let scales_path = dir.join("scales.json");
        let mut scales: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&scales_path).unwrap()
        ).unwrap();
        scales.as_object_mut().unwrap().remove("model.layers.0.mlp.down_proj.input");
        fs::write(&scales_path, serde_json::to_string(&scales).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Fehlende Aktivierungsskala muss Laden verhindern"),
        };
        assert!(err.contains("mlp.down_proj.input"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_missing_weight() {
        let dir = test_dir("model-e2e-missing");
        write_full_fixture(&dir, true, true);
        // Ein Pflichtgewicht aus dem Artefakt entfernen und theta_v.json neu
        // schreiben, damit der Hash-Check aus 12.13 (der jetzt VOR dem
        // Tensor-Lookup laeuft) hier nicht schon vorher zuschlaegt - dieser
        // Test soll gezielt build_model()s require_tensor()-Pfad pruefen,
        // nicht die Manifest-Konsistenzpruefung (dafuer siehe
        // test_load_model_rejects_tampered_theta_v_hash).
        fs::remove_file(dir.join("model_norm_weight.bin")).ok();
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).unwrap()
        ).unwrap();
        manifest.as_object_mut().unwrap().remove("model_norm_weight");
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Fehlendes Pflichtgewicht muss Laden verhindern"),
        };
        assert!(err.contains("model.norm.weight"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_missing_lm_head_when_not_tied() {
        let dir = test_dir("model-e2e-notied-missing");
        write_full_fixture(&dir, false, true);
        fs::remove_file(dir.join("lm_head_weight.bin")).ok();
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).unwrap()
        ).unwrap();
        manifest.as_object_mut().unwrap().remove("lm_head_weight");
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Fehlendes lm_head.weight ohne Tying muss fehlschlagen"),
        };
        assert!(err.contains("lm_head.weight"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    // --- theta_v-Hash-Validierung gegen spec.json (12.13) ---

    #[test]
    fn test_spec_hash_is_stable_and_looks_like_sha256() {
        let h1 = spec_hash();
        let h2 = spec_hash();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_spec_version_matches_embedded_spec_json() {
        // Sanity-Check: die zur Kompilierzeit eingebettete spec.json ist
        // lesbar und liefert eine nichtleere Version.
        let v = spec_version().expect("spec_version");
        assert!(!v.is_empty());
    }

    #[test]
    fn test_theta_v_verify_accepts_matching_hashes() {
        let theta_v = ThetaV {
            version: "x".to_string(),
            weights_hash: "abc".to_string(),
            scales_hash: "def".to_string(),
            luts_hash: "ghi".to_string(),
        };
        assert!(theta_v.verify("abc", "def", "ghi").is_ok());
    }

    #[test]
    fn test_theta_v_verify_rejects_mismatched_hash() {
        let theta_v = ThetaV {
            version: "x".to_string(),
            weights_hash: "abc".to_string(),
            scales_hash: "def".to_string(),
            luts_hash: "ghi".to_string(),
        };
        assert!(theta_v.verify("wrong", "def", "ghi").is_err());
        assert!(theta_v.verify("abc", "wrong", "ghi").is_err());
        assert!(theta_v.verify("abc", "def", "wrong").is_err());
    }

    #[test]
    fn test_theta_v_verify_version_against_spec_accepts_match() {
        let theta_v = ThetaV {
            version: spec_version().unwrap(),
            weights_hash: String::new(),
            scales_hash: String::new(),
            luts_hash: String::new(),
        };
        assert!(theta_v.verify_version_against_spec().is_ok());
    }

    #[test]
    fn test_theta_v_verify_version_against_spec_rejects_mismatch() {
        let theta_v = ThetaV {
            version: "0.0.0-definitely-not-the-real-spec-version".to_string(),
            weights_hash: String::new(),
            scales_hash: String::new(),
            luts_hash: String::new(),
        };
        let err = theta_v.verify_version_against_spec().expect_err("Mismatch muss fehlschlagen");
        assert!(err.contains("theta_v-Version"), "Fehlermeldung: {}", err);
    }

    #[test]
    fn test_load_model_rejects_theta_v_version_mismatch() {
        let dir = test_dir("model-e2e-badversion");
        write_full_fixture(&dir, true, true);

        // theta_v.json mit einer Version ueberschreiben, die nicht zur
        // eingebetteten spec.json passt (Hashes bleiben korrekt - der
        // Versions-Check muss unabhaengig davon greifen und zuerst laufen).
        let theta_v_path = dir.join("theta_v.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&theta_v_path).unwrap()).unwrap();
        manifest["version"] = serde_json::json!("0.0.0-stale");
        fs::write(&theta_v_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Versions-Mismatch muss Laden verhindern"),
        };
        assert!(err.contains("theta_v-Version"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_end_to_end_without_attention_bias() {
        // attention_bias=false: keine Bias-Tensoren im Artefakt, Layer laden
        // mit None-Biases (Modellfamilien ohne Attention-Biases).
        let dir = test_dir("model-e2e-nobias");
        write_full_fixture(&dir, true, false);

        let model = load_model(&dir).expect("Modell-Laden erfolgreich");
        assert!(model.layers[0].q_bias.is_none());
        assert!(model.layers[0].k_bias.is_none());
        assert!(model.layers[0].v_bias.is_none());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_missing_bias_tensors() {
        // attention_bias=true, aber der q_proj.bias-Tensor fehlt im Artefakt:
        // muss laut scheitern (stilles Weglassen wuerde das Modell vom
        // Referenzmodell abweichen lassen).
        let dir = test_dir("model-e2e-missingbias");
        write_full_fixture(&dir, true, true);
        fs::remove_file(dir.join("model_layers_0_self_attn_q_proj_bias.bin")).ok();
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).unwrap()
        ).unwrap();
        manifest.as_object_mut().unwrap().remove("model_layers_0_self_attn_q_proj_bias");
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Fehlender Bias-Tensor muss Laden verhindern"),
        };
        // Der Manifest-Key traegt seit theta_v 0.13.0 Unterstriche statt
        // Punkte (int16-Bias-Pfad); die Meldung muss den Tensor trotzdem
        // eindeutig benennen.
        assert!(
            err.contains("q_proj_bias") || err.contains("q_proj.bias"),
            "Fehlermeldung benennt den fehlenden Bias nicht: {}", err
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_bias_shape_mismatch() {
        // Bias-Tensor mit falscher Laenge (3 statt heads*head_dim=4): muss
        // scheitern, sonst wuerde add_bias_i16 mit falscher Laenge arbeiten.
        let dir = test_dir("model-e2e-badbiasshape");
        write_full_fixture(&dir, true, true);

        let bias_file = dir.join("model_layers_0_self_attn_q_proj_bias.bin");
        // int16 seit theta_v 0.13.0: 3 Elemente = 6 Bytes (statt 4 Elemente).
        let bad_data: Vec<u8> = vec![1, 0, 2, 0, 3, 0];
        fs::write(&bias_file, &bad_data).expect("Bias ueberschreiben");
        let bad_shifts: Vec<u8> = vec![0, 0, 0];
        fs::write(dir.join("model_layers_0_self_attn_q_proj_bias_shifts.bin"), &bad_shifts)
            .expect("Bias-Shifts ueberschreiben");
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).unwrap()
        ).unwrap();
        manifest["model_layers_0_self_attn_q_proj_bias"]["shape"] = serde_json::json!([3]);
        manifest["model_layers_0_self_attn_q_proj_bias"]["hash"] =
            serde_json::json!(sha256_hex(&bad_data));
        manifest["model_layers_0_self_attn_q_proj_bias"]["shifts_hash"] =
            serde_json::json!(sha256_hex(&bad_shifts));
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Falsche Bias-Laenge muss Laden verhindern"),
        };
        assert!(err.contains("Bias-Laenge"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_tampered_manifest_hash() {
        let dir = test_dir("model-e2e-tamperedmanifest");
        write_full_fixture(&dir, true, true);

        // weights_manifest.json nach dem Schreiben von theta_v.json
        // veraendern (z. B. eine Metadaten-Aenderung ohne Datei-Tausch) -
        // der Hash in theta_v.json passt danach nicht mehr, unabhaengig
        // davon, ob einzelne Tensoren noch ladbar waeren.
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["model_norm_weight"]["scale"] = serde_json::json!(2.0);
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Manipuliertes Manifest muss Laden verhindern"),
        };
        assert!(err.contains("hash mismatch"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    /// Ergänzt das Fixture um einen INT16-LM-Head mit Per-Channel-Skalen
    /// (spec-Ausnahme 0.6.0): lm_head.bin (int16 LE) + lm_head_shifts.bin
    /// (int8 je Zeile) + Manifest-Eintrag. Mit `valid = false` wird die
    /// shape im Manifest auf [vocab, hidden+1] gesetzt UND die Datenmenge
    /// entsprechend geschrieben, damit die Shape-Validierung (nicht die
    /// Byte-Laengen-Prüfung) greift.
    fn add_int16_lm_head(dir: &Path, vocab: usize, hidden: usize, valid: bool) {
        let shape = if valid { vec![vocab, hidden] } else { vec![vocab, hidden + 1] };
        let n: usize = shape.iter().product();
        let data: Vec<i16> = (0..n).map(|i| ((i % 11) as i16) - 5).collect();
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        fs::write(dir.join("lm_head.bin"), &bytes).expect("lm_head.bin schreiben");

        let shifts: Vec<i8> = (0..vocab).map(|r| 17 + (r % 4) as i8).collect();
        let shifts_bytes: Vec<u8> = shifts.iter().map(|s| *s as u8).collect();
        fs::write(dir.join("lm_head_shifts.bin"), &shifts_bytes)
            .expect("lm_head_shifts.bin schreiben");

        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let entry = serde_json::json!({
            "original_name": "lm_head.weight",
            "file": "lm_head.bin",
            "shape": shape,
            "scale": -1.0,
            "shift": -1,
            "dtype": "int16",
            "shifts_file": "lm_head_shifts.bin",
            "hash": sha256_hex(&bytes),
            "shifts_hash": sha256_hex(&shifts_bytes),
        });
        manifest.as_object_mut().unwrap().insert("lm_head".to_string(), entry);
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn test_load_model_with_int16_lm_head() {
        // spec-Ausnahme 0.6.0: INT16-LM-Head mit Per-Channel-Skalen wird
        // geladen, validiert und im Modell als Logits-Pfad verdrahtet.
        let dir = test_dir("model-e2e-lmhead-int16");
        write_full_fixture(&dir, true, true);
        add_int16_lm_head(&dir, 3, 4, true);
        write_theta_v(&dir); // theta_v-Hashes über das ergänzte Manifest

        let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
        assert!(model.lm_head_int16.is_some());
        let lmh = model.lm_head_int16.as_ref().unwrap();
        assert_eq!(lmh.shape, vec![3, 4]);
        assert_eq!(lmh.shifts.len(), 3);
        assert_eq!(lmh.shifts[0], 17);
        assert_eq!(lmh.data.len(), 12);
        assert_eq!(lmh.data[0], -5); // (0 % 11) - 5

        // Der Per-Channel-Pfad muss auch im Forward funktionieren.
        let mut cache = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
        let logits_a = model.forward_token(0, 0, &mut cache);
        let mut cache2 = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
        let logits_b = model.forward_token(0, 0, &mut cache2);
        assert_eq!(logits_a, logits_b, "Per-Channel-Logits müssen deterministisch sein");
        assert_eq!(logits_a.len(), 3);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_lm_head_shape_mismatch() {
        let dir = test_dir("model-e2e-lmhead-badshape");
        write_full_fixture(&dir, true, true);
        add_int16_lm_head(&dir, 3, 4, false); // shape [3, 5] statt [3, 4]
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("Falsche LM-Head-shape muss Laden verhindern"),
        };
        assert!(err.contains("LM-Head"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_model_rejects_int16_without_shifts_file() {
        let dir = test_dir("model-e2e-lmhead-noshifts");
        write_full_fixture(&dir, true, true);
        add_int16_lm_head(&dir, 3, 4, true);

        // shifts_file aus dem Manifest-Eintrag entfernen.
        let manifest_path = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["lm_head"].as_object_mut().unwrap().remove("shifts_file");
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        write_theta_v(&dir);

        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("int16-Tensor ohne shifts_file muss Laden verhindern"),
        };
        assert!(err.contains("shifts_file"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }
}
