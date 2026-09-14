//! Modell-Lader mit theta_v-Validierung

use std::path::Path;
use std::collections::HashMap;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use crate::model::{
    DenseMlp, Feedforward, IntegerModel, LayerScales, ModelConfig, MoeLayer, QTensor, QkNorm,
    TransformerLayer,
};

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
    pub data: Kopfdaten,   // flat, row-major [vocab, hidden]
    pub shape: Vec<usize>,
    pub shifts: Vec<u8>,   // ein Zweierpotenz-Shift je Zeile
}

/// Die Werte des int16-Kopfs: ein Abbild der Artefaktdatei, oder eine
/// eigene Kopie, sobald jemand hineinschreibt.
///
/// # 📌 Fund 370 (2026-09-14): der Kopf lag als Kopie im Heap
///
/// Fund 62 hat die int8-Gewichte auf Abbilder umgestellt, **der int16-Kopf
/// blieb ein `Vec<i16>`**: beim 30B 622 MB, beim 4B 778 MB anonymer
/// Speicher. Gemessen während einer Vorbereitung des 30B auf 24 GiB: 625 MB
/// Heap, **davon 607 MB ausgelagert**. Unter Druck wird eine anonyme Seite
/// ausgelagert und je Decode-Schritt zurückgeholt, eine dateigestützte
/// dagegen verworfen und neu gelesen; und jedes Megabyte davon fehlte dem
/// Dateicache, in dem die Experten des Gemischs liegen.
///
/// ⚑ **Schreiben bleibt möglich**: Das Trainingswerkzeug setzt einzelne
/// Kopfzeilen. Der erste schreibende Zugriff legt die Kopie an, gelesen wird
/// bis dahin aus dem Abbild. Welche Zahlen darin stehen, ändert das nicht.
pub enum Kopfdaten {
    /// Eigene Kopie, im Heap.
    Speicher(Vec<i16>),
    /// Abbild der Artefaktdatei, little-endian und auf zwei Bytes
    /// ausgerichtet (geprüft in [`Kopfdaten::aus_abbild`]).
    Abbild(memmap2::Mmap),
}

impl Kopfdaten {
    /// Nimmt das Abbild, wenn es sich als `i16` lesen lässt, sonst eine
    /// Kopie. Die Länge ist vorher geprüft und gerade.
    pub fn aus_abbild(abbild: memmap2::Mmap) -> Self {
        let passt = cfg!(target_endian = "little")
            && abbild.len() % 2 == 0
            && (abbild.as_ptr() as usize) % std::mem::align_of::<i16>() == 0;
        if passt {
            Kopfdaten::Abbild(abbild)
        } else {
            Kopfdaten::Speicher(kopf_aus_bytes(&abbild))
        }
    }
}

fn kopf_aus_bytes(bytes: &[u8]) -> Vec<i16> {
    // `as_chunks::<2>()` wäre der Vorschlag, ist aber erst seit Rust 1.88
    // stabil; siehe die gleiche Stelle bei den Biases.
    #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
    let werte = bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    werte
}

impl From<Vec<i16>> for Kopfdaten {
    fn from(v: Vec<i16>) -> Self {
        Kopfdaten::Speicher(v)
    }
}

impl std::ops::Deref for Kopfdaten {
    type Target = [i16];

    fn deref(&self) -> &[i16] {
        match self {
            Kopfdaten::Speicher(v) => v,
            // SICHERHEIT: `aus_abbild` hat little-endian, gerade Länge und
            // die Ausrichtung auf `i16` geprüft; jedes Bitmuster ist ein
            // gültiges `i16`. Das Abbild ist nur lesend geöffnet und lebt
            // so lange wie die Referenz, beide hängen an `self`.
            Kopfdaten::Abbild(abbild) => unsafe {
                std::slice::from_raw_parts(abbild.as_ptr() as *const i16, abbild.len() / 2)
            },
        }
    }
}

impl std::ops::DerefMut for Kopfdaten {
    fn deref_mut(&mut self) -> &mut [i16] {
        if let Kopfdaten::Abbild(abbild) = self {
            *self = Kopfdaten::Speicher(kopf_aus_bytes(abbild));
        }
        match self {
            Kopfdaten::Speicher(v) => v,
            Kopfdaten::Abbild(_) => unreachable!("eben in eine Kopie verwandelt"),
        }
    }
}

impl std::fmt::Debug for Kopfdaten {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kopfdaten::Speicher(v) => write!(f, "Kopfdaten::Speicher({} Werte)", v.len()),
            Kopfdaten::Abbild(a) => write!(f, "Kopfdaten::Abbild({} Werte)", a.len() / 2),
        }
    }
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
    /// Ob Q und K je Kopf RMS-normiert werden, **vor** RoPE (HF-Feld
    /// `q_norm`/`k_norm` je Layer; Qwen3: ja, Qwen2.5: nein).
    ///
    /// **`serde(default)` und damit `false`, wenn das Feld fehlt.** Alle
    /// Artefakte, die vor dem 2026-08-25 gebaut wurden, sind Qwen2.5 und
    /// haben kein QK-Norm; sie tragen das Feld nicht und sollen weiter
    /// laden. Ein neues Pflichtfeld haette sie ungueltig gemacht und einen
    /// 7B-Neubau von zwanzig Minuten erzwungen, ohne dass sich an ihnen
    /// etwas aendert.
    ///
    /// Fuer **neue** Artefakte ist das Feld dagegen Pflicht: Die
    /// Kalibrierung fuehrt es in `_REQUIRED_EXPORT_FIELDS` und verweigert
    /// den Export ohne. Die Nachsicht gilt also dem Bestand, nicht dem
    /// Neubau.
    #[serde(default)]
    pub qk_norm: bool,
    /// Zahl der Experten je MoE-Layer. **`0` heißt: dieses Modell hat
    /// kein MoE**, und dann sind die drei folgenden Felder bedeutungslos.
    #[serde(default)]
    pub num_experts: usize,
    /// Wie viele Experten je Token feuern (`num_experts_per_tok`).
    #[serde(default)]
    pub num_experts_per_tok: usize,
    /// Breite eines einzelnen Experten. Deutlich kleiner als
    /// `intermediate_size`: bei Qwen3-30B-A3B 768 gegen 6144.
    #[serde(default)]
    pub moe_intermediate_size: usize,
    /// Ob die Gewichte der gewählten Experten auf eins normiert werden.
    #[serde(default)]
    pub norm_topk_prob: bool,
    /// Layer, die trotz MoE-Modell **dicht** bleiben. Bei
    /// Qwen3-30B-A3B leer, aber das Feld existiert, und ein Modell mit
    /// gemischten Layern darf daran nicht scheitern.
    #[serde(default)]
    pub mlp_only_layers: Vec<usize>,
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
    // ⚑ **Fund 59 (2026-08-25): `hidden_size == num_heads * head_dim` gilt
    // nicht allgemein.** Hier stand genau diese Gleichung als harte
    // Ablehnung. Bei Qwen2.5-0,5B (896 = 14·64) und Qwen2.5-7B
    // (3584 = 28·128) trifft sie zu, und mit zwei Modellen sah sie wie
    // eine Modelleigenschaft aus. **Qwen3-4B hat `hidden_size` 2560 und
    // `num_heads · head_dim` = 32·128 = 4096** (gegen die echte
    // `config.json` geprueft, nicht aus einer Modellkarte).
    //
    // Die Architektur entkoppelt beides: Q/K/V projizieren nach
    // `n·head_dim`, `o_proj` bildet von dort auf `hidden_size` zurueck.
    // Eine Beziehung zwischen den beiden Zahlen verlangt niemand.
    //
    // **Ersetzt statt geloescht.** Die Gleichung schuetzte tatsaechlich
    // etwas, naemlich eine vertippte Konfiguration; ohne Ersatz waere die
    // Behebung eine Regression. An ihre Stelle treten die Formpruefungen
    // der Projektionsmatrizen je Layer
    // ([`pruefe_projektionsformen`]) — dieselbe Schutzwirkung, aber
    // gegen die Groessen, auf die es wirklich ankommt.
    if dims.head_dim % 2 != 0 {
        return Err(format!(
            "model_config.json: head_dim ({}) muss gerade sein (RoPE paart je zwei Elemente ueber den Half-Split)",
            dims.head_dim
        ));
    }
    if dims.num_heads % dims.num_kv_heads != 0 {
        return Err(format!(
            "model_config.json: num_heads ({}) ist kein Vielfaches von num_kv_heads ({}) (GQA-Gruppierung nicht moeglich)",
            dims.num_heads, dims.num_kv_heads
        ));
    }

    Ok(dims)
}

/// Was beim Laden eines Manifesteintrags herauskommt.
///
/// ⚑ **Ein Aufzaehlungstyp, damit der Eintrag fuer sich steht.** Bis zum
/// 2026-09-11 schrieb die Schleife unmittelbar in drei Sammlungen; damit
/// war jeder Eintrag an die Schleife gebunden und keiner an einen
/// anderen Faden zu geben.
enum Geladen {
    Gewicht(String, LoadedWeight),
    Kopf(LmHead),
    Vorspann(String, BiasTensor),
}

/// Laedt **einen** Eintrag des Gewichtsmanifests und prueft ihn.
///
/// Der ganze Rumpf stand bis zum 2026-09-11 in der Schleife von
/// [`load_weights`]; herausgezogen ist er unveraendert, nur die drei
/// Einfuegungen sind zu drei Rueckgaben geworden.
fn eintrag_laden(
    artifact_dir: &Path,
    name: String,
    entry: WeightManifestEntry,
) -> Result<Geladen, String> {
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

        // ⚑ **Der Kopf als Abbild, die Biases im Heap** (Fund 370). Ein
        // Bias hat ein paar Tausend Werte; der Kopf beim 30B 311 Millionen.
        let pfad = artifact_dir.join(&entry.file);
        let (gelesen, abbild) = if ist_bias {
            let b = std::fs::read(&pfad)
                .map_err(|e| format!("Fehler beim Lesen von {}: {}", entry.file, e))?;
            (Some(b), None)
        } else {
            let datei = std::fs::File::open(&pfad)
                .map_err(|e| format!("Fehler beim Oeffnen von {}: {}", entry.file, e))?;
            // SICHERHEIT: wie bei den int8-Gewichten weiter unten, lesend
            // geöffnet und unmittelbar danach über SHA-256 geprüft.
            let a = unsafe { memmap2::Mmap::map(&datei) }
                .map_err(|e| format!("Fehler beim Abbilden von {}: {}", entry.file, e))?;
            crate::model::abbild_vorbereiten(&a);
            (None, Some(a))
        };
        let bytes: &[u8] = match (&gelesen, &abbild) {
            (Some(b), _) => b,
            (None, Some(a)) => a,
            (None, None) => unreachable!("einer von beiden ist gesetzt"),
        };
        let expected_len: usize = entry.shape.iter().product::<usize>() * 2;
        if bytes.len() != expected_len {
            return Err(format!(
                "{}: {} Bytes in '{}', aber shape {:?} erwartet {} Bytes (int16)",
                name, bytes.len(), entry.file, entry.shape, expected_len
            ));
        }
        let digest = sha256_hex(bytes);
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
        // 1.88 stabil. Die Schwester-Crates erklaeren MSRV 1.85; dieses
        // hier hat keine Angabe, und ein stillschweigend hoeherer Bedarf
        // waere schlimmer als eine ausdrueckliche Ausnahme.
        #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
        return Ok(match abbild {
            None => {
                let data: Vec<i16> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                Geladen::Vorspann(name, BiasTensor { data, shifts: shift_bytes })
            }
            Some(a) => Geladen::Kopf(LmHead {
                data: Kopfdaten::aus_abbild(a),
                shape: entry.shape,
                shifts: shift_bytes,
            }),
        });
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

    // ⚑ **Fund 62 (2026-08-25): Abbild statt Heap-Kopie.**
    //
    // Hier stand `std::fs::read`, also eine Kopie jeder Gewichtsdatei
    // in den Heap. Für 0,74 GB und 8,1 GB trägt das. Das Artefakt von
    // Qwen3-30B-A3B ist **29 GiB** gegen 24 GiB Arbeitsspeicher, und
    // der Versuch zeigte das schlechteste denkbare Verhalten: Während
    // der Prozess las, schrieb das System seinen Heap in die
    // Auslagerung. RSS fiel von 8,9 auf 1,9 GiB, der Swap wuchs von
    // 2,9 auf 4,1 GB. Von der Platte lesen und sofort wieder auf die
    // Platte schreiben.
    //
    // Als Abbild sind dieselben Bytes **saubere, dateigestützte
    // Seiten**: Das System verwirft sie unter Druck und liest sie bei
    // Bedarf neu, statt sie auszulagern. Für ein Mixture-of-Experts-Modell ist
    // das mehr als eine Notlösung, denn bei Top-8 von 128 rührt ein
    // Token nur ein Sechzehntel der Expertengewichte an.
    //
    // Gemessen: 18 873 Abbildungen in einem Prozess sind auf dieser
    // Maschine unproblematisch (Deskriptorgrenze 1 048 576).
    let pfad = artifact_dir.join(&entry.file);
    let datei = std::fs::File::open(&pfad)
        .map_err(|e| format!("Fehler beim Oeffnen von {}: {}", entry.file, e))?;
    // SICHERHEIT: `Mmap::map` ist unsicher, weil ein Fremdprozess die
    // Datei unter dem Abbild ändern könnte. Hier liegt sie im
    // Artefaktverzeichnis, wird ausschließlich lesend geöffnet, und
    // ihr Inhalt ist unmittelbar danach über SHA-256 gegen das
    // Manifest geprüft. Eine Änderung nach dieser Prüfung wäre
    // dieselbe Klasse von Angriff wie eine Änderung an einer
    // eingelesenen Datei zwischen Lesen und Verwenden.
    let abbild = unsafe { memmap2::Mmap::map(&datei) }
        .map_err(|e| format!("Fehler beim Abbilden von {}: {}", entry.file, e))?;

    let expected_len: usize = entry.shape.iter().product();
    if abbild.len() != expected_len {
        return Err(format!(
            "{}: {} Bytes in '{}', aber shape {:?} erwartet {} Bytes",
            name, abbild.len(), entry.file, entry.shape, expected_len
        ));
    }

    // **Die Prüfsumme liest jedes Byte, und das ist in Ordnung.**
    // Sie bindet das Artefakt an θ_v; ohne sie fiele der Anker weg.
    // Anders als eine Heap-Kopie bleibt danach nichts belegt: Die
    // berührten Seiten sind sauber und werden bei Bedarf verworfen.
    // ⚑ **Fund 331 auch hier, und hier wiegt er am schwersten.**
    // Die Pruefsumme liest jedes Byte, und ohne Ankuendigung holt
    // das Abbild sie Seite fuer Seite: 0,44 GB/s statt 3,41. Fuer
    // 29 GB sind das 66 statt 9 Sekunden, **nur fuer das Warten auf
    // einzelne Seitenfehler**. Ein Rat vor dem ersten Byte kostet
    // einen Systemaufruf.
    crate::model::abbild_vorbereiten(&abbild);
    let digest = sha256_hex(&abbild);
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
        data: std::sync::Arc::new(crate::model::Gewichtsdaten::Abbild(abbild)),
        shape: entry.shape,
        shifts,
    };
    Ok(Geladen::Gewicht(name, LoadedWeight {
        tensor,
        original_name: entry.original_name,
        scale: entry.scale,
    }))
}

/// **Alle Eintraege pruefen, auf so vielen Faeden wie Kerne da sind.**
///
/// ## 📌 Fund 333 (2026-09-11): zwei Minuten Ladezeit waren eine Pruefsumme
///
/// Die Pruefsumme liest jedes Byte, und das ist richtig: Sie bindet das
/// Artefakt an θ_v. Sie lief nur **auf einem Kern**. Gemessen an
/// Qwen3-30B-A3B (29 GB, 37 747 Dateien): rund **180 MB/s**, also die
/// Geschwindigkeit der reinen Rust-Fassung von SHA-256, und daraus
/// folgen die zwei Minuten vor dem ersten Token.
///
/// ⚑ **Der Engpass war nie die Platte.** Mit dem Rat aus Fund 331
/// liefert sie 3,4 GB/s; die Pruefsumme holte davon ein Zwanzigstel ab.
///
/// ⚑ **Und es ist kein neues Bauteil noetig.** Eine Beschleunigung ueber
/// `sha2/asm` haette eine weitere Kiste in die Lieferkette geholt, und
/// ueber die entscheidet der Projektinhaber, nicht eine Ladezeit. Ein
/// Dutzend Faeden ueber dieselbe reine Fassung bringt denselben Faktor,
/// ohne dass jemand einem Fremden mehr glauben muss.
///
/// ⚑ **Die Reihenfolge des Ergebnisses haengt nicht an den Faeden.** Die
/// Eintraege werden in Manifestreihenfolge abgearbeitet und in
/// Manifestreihenfolge zusammengefuehrt; der **erste** Fehler ist der
/// mit dem kleinsten Index, nicht der, den ein Faden zuerst sah. Das ist
/// strenger als vorher: Bis heute entschied die Reihenfolge einer
/// `HashMap`, welcher von mehreren Fehlern gemeldet wurde.
fn alle_eintraege_laden(
    artifact_dir: &Path,
    liste: Vec<(String, WeightManifestEntry)>,
) -> Result<Vec<Geladen>, String> {
    let faeden = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    if faeden < 2 || liste.len() < 2 {
        return liste
            .into_iter()
            .map(|(n, e)| eintrag_laden(artifact_dir, n, e))
            .collect();
    }

    let anzahl = liste.len();
    let je = anzahl.div_ceil(faeden);
    let mut teile: Vec<Vec<(String, WeightManifestEntry)>> = Vec::with_capacity(faeden);
    let mut rest = liste;
    while !rest.is_empty() {
        let schnitt = je.min(rest.len());
        let hinten = rest.split_off(schnitt);
        teile.push(rest);
        rest = hinten;
    }

    // ⚠️ **Ein Bereich und kein stehender Pool.** Dieser Weg laeuft
    // einmal je Prozess; der Pool aus `kernels` ist fuer 1 152 Runden
    // je Token gebaut und braucht hier nicht bemueht zu werden.
    let ergebnisse: Vec<Result<Vec<Geladen>, String>> = std::thread::scope(|bereich| {
        let griffe: Vec<_> = teile
            .into_iter()
            .map(|teil| {
                bereich.spawn(move || {
                    teil.into_iter()
                        .map(|(n, e)| eintrag_laden(artifact_dir, n, e))
                        .collect::<Result<Vec<_>, String>>()
                })
            })
            .collect();
        griffe.into_iter().map(|g| g.join().unwrap_or_else(|_| Err("Ladefaden abgestuerzt".into()))).collect()
    });

    let mut alle = Vec::with_capacity(anzahl);
    for teil in ergebnisse {
        alle.extend(teil?);
    }
    Ok(alle)
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

    // ⚑ **In Manifestreihenfolge, nicht in Streuordnung.** Eine
    // `HashMap` gibt ihre Eintraege in einer Reihenfolge heraus, die
    // vom Streuwert abhaengt; eine sortierte Liste macht aus dem
    // Laden eine Funktion, deren Fehlermeldung nicht vom Zufall
    // abhaengt.
    let mut liste: Vec<(String, WeightManifestEntry)> = entries.into_iter().collect();
    liste.sort_by(|a, b| a.0.cmp(&b.0));

    let mut weights = HashMap::with_capacity(liste.len());
    let mut lm_head: Option<LmHead> = None;
    let mut biases: HashMap<String, BiasTensor> = HashMap::new();
    for geladen in alle_eintraege_laden(artifact_dir, liste)? {
        match geladen {
            Geladen::Gewicht(name, w) => {
                weights.insert(name, w);
            }
            Geladen::Kopf(k) => lm_head = Some(k),
            Geladen::Vorspann(name, b) => {
                biases.insert(name, b);
            }
        }
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
/// Prüft die Formen der vier Attention-Projektionen gegen die
/// Modell-Dimensionen.
///
/// **Warum das eine eigene Prüfung ist (Fund 59, 2026-08-25):** Bis heute
/// stand in `load_model_dims` die Bedingung
/// `hidden_size == num_heads * head_dim`. Sie galt für die beiden bis
/// dahin gebauten Modelle und ist trotzdem falsch: Die Architektur
/// entkoppelt beide Größen, Qwen3-4B hat 2560 gegen 32·128 = 4096.
///
/// Die Gleichung schützte aber etwas Echtes, nämlich vertippte
/// Konfigurationen. Diese Funktion übernimmt diesen Schutz und macht ihn
/// schärfer: Sie prüft nicht eine Beziehung zwischen zwei Zahlen,
/// sondern die vier Formen, mit denen der Forward-Pass rechnet.
///
/// ```text
///     q_proj : [num_heads    · head_dim, hidden_size]
///     k_proj : [num_kv_heads · head_dim, hidden_size]
///     v_proj : [num_kv_heads · head_dim, hidden_size]
///     o_proj : [hidden_size,             num_heads · head_dim]
/// ```
fn pruefe_projektionsformen(
    p: &str,
    dims: &ModelDims,
    q: &QTensor,
    k: &QTensor,
    v: &QTensor,
    o: &QTensor,
) -> Result<(), String> {
    let q_len = dims.num_heads * dims.head_dim;
    let kv_len = dims.num_kv_heads * dims.head_dim;

    let pruefe = |name: &str, t: &QTensor, zeilen: usize, spalten: usize| -> Result<(), String> {
        if t.shape.len() != 2 {
            return Err(format!(
                "{}.self_attn.{}.weight: shape {:?} ist nicht zweidimensional",
                p, name, t.shape
            ));
        }
        if t.shape[0] != zeilen || t.shape[1] != spalten {
            return Err(format!(
                "{}.self_attn.{}.weight: shape {:?}, erwartet [{}, {}] \
                 (num_heads {}, num_kv_heads {}, head_dim {}, hidden_size {})",
                p, name, t.shape, zeilen, spalten,
                dims.num_heads, dims.num_kv_heads, dims.head_dim, dims.hidden_size
            ));
        }
        Ok(())
    };

    pruefe("q_proj", q, q_len, dims.hidden_size)?;
    pruefe("k_proj", k, kv_len, dims.hidden_size)?;
    pruefe("v_proj", v, kv_len, dims.hidden_size)?;
    pruefe("o_proj", o, dims.hidden_size, q_len)?;
    Ok(())
}

/// Namensschema der MoE-Tensoren im Artefakt.
///
/// ✅ **Gegen die echte `model.safetensors.index.json` von
/// `Qwen/Qwen3-30B-A3B` geprueft** (Revision `ad44e777`, lokal unter
/// `models/Qwen3-30B-A3B/`, 2026-08-25). Die Datei fuehrt je Layer
/// `mlp.gate.weight` und 384 Experten-Tensoren, also 128 Experten mal
/// gate/up/down, nummeriert von 0 bis 127.
///
/// Die Konstanten stehen hier zusammen, damit eine Berichtigung eine
/// Zeile ist und nicht eine Suche. `model_configs.py` folgt derselben
/// Regel: Was nicht gegen die echte Datei geprueft ist, wird nicht
/// exportiert.
///
/// **Nebenbefund aus derselben Datei:** Das Modell hat **18 867
/// Tensoren**. Der Export schreibt je Tensor eine Datei plus eine
/// Shift-Datei, das Artefaktverzeichnis traegt also rund **37 700
/// Dateien** statt der 593 bei 0,5B. Fuer die stroemende Ladung ist das
/// die guenstige Form (nur lesen, was feuert); fuer Werkzeuge, die das
/// Verzeichnis auflisten, ist es eine Groessenordnung mehr.
const ROUTER_SUFFIX: &str = "mlp.gate.weight";
/// Wie [`ROUTER_SUFFIX`], aber ohne `.weight`, denn Aktivierungsskalen
/// tragen den Modulnamen und nicht den Tensornamen.
const ROUTER_SUFFIX_OHNE_WEIGHT: &str = "mlp.gate";

/// Praefix eines Experten-Tensors, also alles vor der Expertennummer.
fn p_experte(layer_praefix: &str) -> String {
    format!("{}.mlp.experts.", layer_praefix)
}

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

        // Fund 59: Diese Pruefung traegt, was frueher die Gleichung
        // `hidden_size == num_heads * head_dim` trug. Sie prueft die
        // Formen, die der Forward-Pass tatsaechlich voraussetzt, statt
        // eine Beziehung, die nur zufaellig fuer zwei Modelle galt.
        pruefe_projektionsformen(
            &p,
            &dims,
            require_tensor(&weights, &format!("{}.self_attn.q_proj.weight", p))?,
            require_tensor(&weights, &format!("{}.self_attn.k_proj.weight", p))?,
            require_tensor(&weights, &format!("{}.self_attn.v_proj.weight", p))?,
            require_tensor(&weights, &format!("{}.self_attn.o_proj.weight", p))?,
        )?;

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

        // QK-Norm (Qwen3): beide Gammas und beide Ausgangsskalen, oder
        // keines von vieren. Der Typ `QkNorm` laesst nichts Halbes zu.
        let qk_norm = if dims.qk_norm {
            let q_gamma =
                require_tensor(&weights, &format!("{}.self_attn.q_norm.weight", p))?.clone();
            let k_gamma =
                require_tensor(&weights, &format!("{}.self_attn.k_norm.weight", p))?.clone();
            // Beide Gammas sind je `head_dim` lang und werden von allen
            // Koepfen geteilt. Eine falsche Laenge hier waere im
            // Forward-Pass eine stille Fehlnormierung, kein Fehler.
            for (bez, g) in [("q_norm", &q_gamma), ("k_norm", &k_gamma)] {
                if g.data.len() != dims.head_dim {
                    return Err(format!(
                        "{}.self_attn.{}.weight: {} Elemente, erwartet head_dim ({})",
                        p, bez, g.data.len(), dims.head_dim
                    ));
                }
            }
            Some(QkNorm {
                q_gamma,
                k_gamma,
                q_out_frac: require_scale(&scales, &format!("{}.self_attn.q_norm", p))?,
                k_out_frac: require_scale(&scales, &format!("{}.self_attn.k_norm", p))?,
            })
        } else {
            None
        };

        // Feedforward: dicht oder Mixture-of-Experts-Modell.
        //
        // `num_experts == 0` heisst kein MoE. Eine Layer in
        // `mlp_only_layers` bleibt auch in einem MoE-Modell dicht; bei
        // Qwen3-30B-A3B ist die Liste leer, aber sie existiert.
        let ist_moe = dims.num_experts > 0 && !dims.mlp_only_layers.contains(&layer_idx);
        let ffn = if ist_moe {
            let mut experts = Vec::with_capacity(dims.num_experts);
            for e in 0..dims.num_experts {
                experts.push(DenseMlp {
                    gate_proj: require_tensor(
                        &weights, &format!("{}{}.gate_proj.weight", p_experte(&p), e))?.clone(),
                    up_proj: require_tensor(
                        &weights, &format!("{}{}.up_proj.weight", p_experte(&p), e))?.clone(),
                    down_proj: require_tensor(
                        &weights, &format!("{}{}.down_proj.weight", p_experte(&p), e))?.clone(),
                });
            }
            Feedforward::Moe(MoeLayer {
                router: require_tensor(&weights, &format!("{}.{}", p, ROUTER_SUFFIX))?.clone(),
                router_frac: require_scale(&scales, &format!("{}.{}", p, ROUTER_SUFFIX_OHNE_WEIGHT))?,
                experts,
                top_k: dims.num_experts_per_tok,
                norm_topk_prob: dims.norm_topk_prob,
            })
        } else {
            Feedforward::Dense(DenseMlp {
                gate_proj: require_tensor(&weights, &format!("{}.mlp.gate_proj.weight", p))?.clone(),
                up_proj: require_tensor(&weights, &format!("{}.mlp.up_proj.weight", p))?.clone(),
                down_proj: require_tensor(&weights, &format!("{}.mlp.down_proj.weight", p))?.clone(),
            })
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
            ffn,
            q_bias,
            k_bias,
            v_bias,
            qk_norm,
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
        assert_eq!(&tensor.data[..], &[-128i8, -1, 0, 1, 127, 64]);
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

    /// ⚑ **Fund 59 (2026-08-25).** Hier stand bis heute das Gegenteil:
    /// Der Test verlangte, dass `hidden_size != num_heads * head_dim`
    /// **abgelehnt** wird. Die Bedingung galt fuer Qwen2.5-0,5B
    /// (896 = 14·64) und Qwen2.5-7B (3584 = 28·128) und sah mit zwei
    /// Modellen wie eine Modelleigenschaft aus.
    ///
    /// **Qwen3-4B hat hidden_size 2560 gegen 32·128 = 4096** (gegen die
    /// echte `config.json` geprueft). Die Architektur entkoppelt beide
    /// Groessen; `o_proj` bildet von `num_heads·head_dim` auf
    /// `hidden_size` zurueck. Der Test hielt damit einen Fehler fest,
    /// nicht eine Eigenschaft, und haette jeden Qwen3-Wechsel als
    /// „Regression" gemeldet.
    #[test]
    fn entkoppeltes_head_dim_wird_angenommen() {
        let dir = test_dir("dims-entkoppelt");
        // Die Verhaeltnisse von Qwen3-4B, verkleinert:
        // hidden_size 10, num_heads 4, head_dim 4 -> 16 != 10.
        write_model_config(&dir, &model_dims_json(4, 2, 10, 4, true, false));

        let dims = load_model_dims(&dir)
            .expect("entkoppeltes head_dim ist zulaessig, siehe Qwen3");
        assert_eq!(dims.hidden_size, 10);
        assert_eq!(dims.num_heads * dims.head_dim, 16);

        fs::remove_dir_all(&dir).ok();
    }

    /// Was an die Stelle der falschen Pruefung getreten ist: RoPE paart
    /// je zwei Elemente ueber den Half-Split, ein ungerades `head_dim`
    /// waere nicht rotierbar.
    #[test]
    fn ungerades_head_dim_wird_abgelehnt() {
        let dir = test_dir("dims-ungerade");
        write_model_config(&dir, &model_dims_json(4, 2, 10, 3, true, false));

        let err = load_model_dims(&dir).expect_err("ungerades head_dim muss fehlschlagen");
        assert!(err.contains("head_dim"), "Fehlermeldung: {}", err);

        fs::remove_dir_all(&dir).ok();
    }

    /// Der eigentliche Ersatz fuer die geloeschte Pruefung: die Formen
    /// der vier Projektionen. **Gegenprobe dazu:** Ohne
    /// `pruefe_projektionsformen` bliebe eine vertippte Konfiguration
    /// unbemerkt, bis der Forward-Pass ueber den Rand liefe.
    #[test]
    fn falsche_projektionsform_wird_erkannt() {
        let dims = ModelDims {
            family: "test".to_string(),
            variant: "entkoppelt".to_string(),
            intermediate_size: 20,
            num_layers: 1,
            hidden_size: 10,
            num_heads: 4,
            num_kv_heads: 2,
            head_dim: 4,
            vocab_size: 8,
            max_context: 16,
            tie_word_embeddings: true,
            attention_bias: false,
            qk_norm: false,
            num_experts: 0,
            num_experts_per_tok: 0,
            moe_intermediate_size: 0,
            norm_topk_prob: false,
            mlp_only_layers: Vec::new(),
        };
        let t = |zeilen: usize, spalten: usize| QTensor {
            data: std::sync::Arc::new(vec![0i8; zeilen * spalten].into()),
            shape: vec![zeilen, spalten],
            shifts: vec![0u8; zeilen],
        };
        // Richtig: q [16,10], k/v [8,10], o [10,16].
        assert!(pruefe_projektionsformen(
            "model.layers.0", &dims, &t(16, 10), &t(8, 10), &t(8, 10), &t(10, 16)
        ).is_ok());

        // q_proj mit hidden_size statt num_heads*head_dim: genau der
        // Fehler, den die alte Gleichung erzwungen haette.
        let err = pruefe_projektionsformen(
            "model.layers.0", &dims, &t(10, 10), &t(8, 10), &t(8, 10), &t(10, 16)
        ).expect_err("falsche q_proj-Form muss auffallen");
        assert!(err.contains("q_proj"), "Fehlermeldung: {}", err);

        // o_proj in der falschen Richtung.
        let err = pruefe_projektionsformen(
            "model.layers.0", &dims, &t(16, 10), &t(8, 10), &t(8, 10), &t(16, 10)
        ).expect_err("vertauschte o_proj-Form muss auffallen");
        assert!(err.contains("o_proj"), "Fehlermeldung: {}", err);
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
        write_full_fixture_mit(dir, tie_word_embeddings, attention_bias, None);
    }

    /// Wie [`write_full_fixture`], aber wahlweise mit Mixture-of-Experts-Modell.
    ///
    /// `moe` ist `(Expertenzahl, top_k, Expertenbreite)`. Damit laesst
    /// sich der MoE-Ladepfad pruefen, ohne ein 57-GiB-Modell zu
    /// beschaffen: Die Formen sind dieselben, nur die Zahlen sind klein.
    fn write_full_fixture_mit(
        dir: &Path,
        tie_word_embeddings: bool,
        attention_bias: bool,
        moe: Option<(usize, usize, usize)>,
    ) {
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
            "max_context": 2048,
            "tie_word_embeddings": tie_word_embeddings,
            "attention_bias": attention_bias,
            "num_experts": moe.map(|m| m.0).unwrap_or(0),
            "num_experts_per_tok": moe.map(|m| m.1).unwrap_or(0),
            "moe_intermediate_size": moe.map(|m| m.2).unwrap_or(0),
            "norm_topk_prob": moe.is_some(),
            "mlp_only_layers": Vec::<usize>::new(),
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
        let mut scales = scales;
        if moe.is_some() {
            // Die Router-Projektion braucht eine eigene Skala; die
            // Experten teilen sich die Layer-Skalen gate/up/down, die
            // oben schon stehen (siehe stats.py::_sammelschluessel).
            scales["model.layers.0.mlp.gate"] = scale_entry(4, 0.0625, 12.0);
        }
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
        match moe {
            None => {
                put("model.layers.0.mlp.gate_proj.weight", vec![inter, hidden]);
                put("model.layers.0.mlp.up_proj.weight", vec![inter, hidden]);
                put("model.layers.0.mlp.down_proj.weight", vec![hidden, inter]);
            }
            Some((experten, _top_k, breite)) => {
                // Namen wie in der echten index.json von Qwen3-30B-A3B,
                // dort nachgesehen: mlp.gate.weight als Router, dann
                // mlp.experts.<N>.{gate,up,down}_proj.weight.
                put("model.layers.0.mlp.gate.weight", vec![experten, hidden]);
                for e in 0..experten {
                    put(&format!("model.layers.0.mlp.experts.{e}.gate_proj.weight"),
                        vec![breite, hidden]);
                    put(&format!("model.layers.0.mlp.experts.{e}.up_proj.weight"),
                        vec![breite, hidden]);
                    put(&format!("model.layers.0.mlp.experts.{e}.down_proj.weight"),
                        vec![hidden, breite]);
                }
            }
        }

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
        // Ein Viertelkreis je Position, 2 048 Positionen: Die Tabelle
        // wiederholt sich alle vier Zeilen, und die Proben laufen ueber mehr
        // als ein Vorbereitungsfenster, ohne an die Kontextgrenze zu stossen.
        put_lut("cos", [256, 0, -256, 0].repeat(512));
        put_lut("sin", [0, 256, 0, -256].repeat(512));
        put_lut("exp", vec![256, 128, 64]);
        put_lut("silu", vec![-10, 0, 10, 20]);
        put_lut("rsqrt", vec![256, 181, 148]);

        fs::write(dir.join("luts.json"), serde_json::to_string(&luts_manifest).unwrap())
            .expect("luts.json schreiben");

        // theta_v.json zuletzt schreiben: Version und Hashes muessen zu den
        // gerade geschriebenen Manifest-Dateien passen (Punkt 12.13).
        write_theta_v(dir);
    }

    // --- Mixture-of-Experts (2026-08-25) ---

    /// Ein MoE-Artefakt laedt, und der Loader baut `Feedforward::Moe`
    /// mit allen Experten und der Router-Skala.
    ///
    /// **Der Test ersetzt keinen Lauf gegen das echte Modell**, aber er
    /// prueft genau die Stellen, an denen ein 57-GiB-Lauf sonst nach
    /// einer halben Stunde scheiterte: Namensschema, Expertenzahl,
    /// Formen, Router-Skala.
    #[test]
    fn ein_moe_artefakt_laedt_mit_allen_experten() {
        let dir = test_dir("moe-laden");
        write_full_fixture_mit(&dir, true, false, Some((6, 2, 3)));

        let model = load_model(&dir).expect("MoE-Artefakt muss laden");
        match &model.layers[0].ffn {
            crate::model::Feedforward::Moe(moe) => {
                assert_eq!(moe.experts.len(), 6, "alle sechs Experten geladen");
                assert_eq!(moe.top_k, 2);
                assert!(moe.norm_topk_prob);
                assert_eq!(moe.router.shape, vec![6, 4], "Router ist [experten, hidden]");
                assert_eq!(moe.experts[0].gate_proj.shape, vec![3, 4]);
                assert_eq!(moe.experts[5].down_proj.shape, vec![4, 3]);
                assert_eq!(moe.router_frac, 4, "die eigene Skala des Routers");
            }
            crate::model::Feedforward::Dense(_) => {
                panic!("bei num_experts > 0 muss der Loader eine MoE-Layer bauen")
            }
        }

        fs::remove_dir_all(&dir).ok();
    }

    /// **Die gebuendelte Vorbereitung rechnet dasselbe wie die
    /// tokenweise, dicht und als Expertengemisch.**
    ///
    /// 📌 **Diese Gegenprobe gab es bis zum 2026-09-11 nicht**, obwohl
    /// die gebuendelte Vorbereitung seit dem 2026-09-11 der Normalweg
    /// ist. **Ein Weg, den nichts gegen den anderen haelt, darf
    /// auseinanderlaufen, ohne dass es jemand merkt.**
    ///
    /// Geprueft wird die Ausgabe **und** der Zwischenzustand: Die
    /// Logits an der letzten Stelle haengen am KV-Speicher aller
    /// vorherigen Positionen, den die Vorbereitung gefuellt hat.
    #[test]
    fn die_gebuendelte_vorbereitung_rechnet_dasselbe() {
        for (name, moe) in [("stapel-dicht", None), ("stapel-moe", Some((6usize, 2usize, 3usize)))]
        {
            let dir = test_dir(name);
            write_full_fixture_mit(&dir, true, false, moe);
            gewichte_verrauschen(&dir, 0x5eed_0366);
            skalen_je_kanal_streuen(&dir);
            let model = load_model(&dir).expect("Artefakt muss laden");

            // Der Wortschatz des Fixtures ist drei Eintraege gross.
            let ids: Vec<usize> = vec![0, 1, 2, 1, 0, 2, 2, 0];
            let letzte = ids.len() - 1;

            let mut gebuendelt = crate::kv_cache::KVCache::new(
                model.num_layers,
                model.num_kv_heads,
            );
            let zustaende = model.vorbereiten_stapel(&ids[..letzte], 0, &mut gebuendelt);
            let a = model.forward_token(ids[letzte], letzte, &mut gebuendelt);

            let mut einzeln = crate::kv_cache::KVCache::new(
                model.num_layers,
                model.num_kv_heads,
            );
            let mut b = Vec::new();
            for (pos, id) in ids.iter().enumerate() {
                if pos < letzte {
                    // 📌 **Der Strom jedes vorbereiteten Tokens**, nicht
                    // nur die Logits danach (nachgetragen mit Fund 366).
                    // Das Fixture hat eine Ebene; dort wirkt die
                    // Aufmerksamkeit eines vorbereiteten Tokens auf nichts
                    // anderes, und eine Aufmerksamkeit, die eine Position
                    // zu weit las, blieb gemessen unentdeckt.
                    let erwartet = model.durch_die_ebenen(*id, pos, &mut einzeln);
                    assert_eq!(zustaende[pos], erwartet, "{name}: Strom von Token {pos} weicht ab");
                } else {
                    b = model.forward_token(*id, pos, &mut einzeln);
                }
            }

            assert_eq!(a, b, "{name}: gebuendelt und tokenweise weichen ab");

            // 📌 **Und der Zwischenspeicher selbst** (nachgetragen mit
            // Fund 366). Ueber die Logits allein blieb dieser Test gruen,
            // als die Aufmerksamkeit jedes Tokens ausser dem ersten eine
            // Position zu weit rechnete: Das Fixture ist dafuer zu klein.
            for ebene in 0..model.num_layers {
                for kopf in 0..model.num_kv_heads {
                    assert_eq!(
                        gebuendelt.read_scheiben(ebene, kopf, ids.len()),
                        einzeln.read_scheiben(ebene, kopf, ids.len()),
                        "{name}: KV-Speicher in Ebene {ebene}, Kopf {kopf}"
                    );
                }
            }

            // ⚠️ **Fuer ein Gemisch vergleicht dieser Test zwei Wege,
            // die derselbe sind**: `ebene_mlp_stapel` gibt dort `None`
            // zurueck und faellt auf den tokenweisen Weg zurueck. Das
            // ist seit Fund 335 eine **gemessene Entscheidung** und
            // kein Versehen; die Begruendung steht dort. Der Test haelt
            // die dichte Buendelung und deckt beim Gemisch ab, dass der
            // Rueckfallweg selbst stimmt.

            fs::remove_dir_all(&dir).ok();
        }
    }

    /// **Ersetzt die int8-Gewichte eines Fixtures durch Zufallszahlen**
    /// und zieht Manifest und θ_v nach.
    ///
    /// 📌 **Warum** (Fund 366): Die Gewichte des Fixtures sind `i % 7`,
    /// und darauf ist die Aufmerksamkeit so gleichfoermig, dass eine, die
    /// eine Position zu weit las, dieselben Zahlen lieferte. Gemessen am
    /// 2026-09-14: Die Pruefung der Vorbereitung blieb gruen, der
    /// Konformitaetslauf am echten Modell nicht. Mit Zufallsgewichten
    /// faellt es auch ohne Artefakt auf, also auch in der CI.
    fn gewichte_verrauschen(dir: &Path, saat: u64) {
        let pfad = dir.join("weights_manifest.json");
        let mut manifest: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&fs::read(&pfad).expect("Manifest lesen")).expect("Manifest parsen");
        let mut s = saat | 1;
        for eintrag in manifest.values_mut() {
            if eintrag["dtype"] != "int8" {
                continue;
            }
            let datei = dir.join(eintrag["file"].as_str().expect("Dateiname"));
            let n = fs::read(&datei).expect("Gewicht lesen").len();
            let daten: Vec<u8> = (0..n)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    // -127..=127, wie ein symmetrisch quantisiertes Gewicht
                    ((s % 255) as i16 - 127) as i8 as u8
                })
                .collect();
            fs::write(&datei, &daten).expect("Gewicht schreiben");
            eintrag["hash"] = serde_json::json!(sha256_hex(&daten));
            // ⚑ **Shift vier, gemessen und nicht geschaetzt** (Fund 371). Mit
            // Shift null und eins saettigten die Ausgaben der Experten bei
            // ±32 767, mit sieben waren sie null; in beiden Faellen blieb ein
            // Fehler in der Ausgangsskala unsichtbar. Bei vier liegen sie
            // zwischen −221 und 185.
            eintrag["shift"] = serde_json::json!(4);
            eintrag["scale"] = serde_json::json!(1.0 / 16.0);
        }
        fs::write(&pfad, serde_json::to_string(&manifest).unwrap()).expect("Manifest schreiben");
        write_theta_v(dir);
    }

    /// **Gibt den Segmenten des Residualstroms verschiedene Skalen je
    /// Kanal** und zieht θ_v nach.
    ///
    /// 📌 **Warum** (Fund 371): Die Skalen des Fixtures sind je Segment eine
    /// Zahl. Eine gruppierte Gemischebene, die ihre Ausgabe mit der Skala
    /// des ersten Kanals statt der je Kanal rechnete, blieb darauf gemessen
    /// gruen. Echte Artefakte tragen je Kanal verschiedene Skalen (Fund 20).
    fn skalen_je_kanal_streuen(dir: &Path) {
        let pfad = dir.join("scales.json");
        let mut skalen: serde_json::Value =
            serde_json::from_slice(&fs::read(&pfad).expect("scales.json lesen")).expect("scales.json parsen");
        for (name, shifts) in [
            ("model.layers.0.input_layernorm.input", [12i64, 10, 13, 11]),
            ("model.layers.0.post_attention_layernorm.input", [5, 3, 6, 4]),
            ("model.norm.input", [4, 6, 3, 5]),
        ] {
            skalen[name]["shifts"] = serde_json::json!(shifts);
        }
        fs::write(&pfad, serde_json::to_string(&skalen).unwrap()).expect("scales.json schreiben");
        write_theta_v(dir);
    }

    /// **Die Vorbereitung eines ganzen Prompts rechnet dasselbe wie Token
    /// fuer Token, ueber Fenstergrenzen hinweg** (Fund 366).
    ///
    /// ⚑ **Geprueft wird der Weg, den die Erzeugung jetzt nimmt**, und
    /// zwar laenger als zwei Fenster: Genau an der Grenze beginnt ein
    /// neuer Stapel bei einer Position, die nicht null ist. Dazu die
    /// Raender, die die alte Schleife still richtig machte: ein leerer
    /// Prompt gibt lauter Nullen, ein Token geht allein durch den Kopf.
    #[test]
    fn die_vorbereitung_eines_prompts_rechnet_dasselbe_ueber_fenstergrenzen() {
        use crate::model::VORBEREITUNGSFENSTER;
        for (name, moe) in [("prompt-dicht", None), ("prompt-moe", Some((6usize, 2usize, 3usize)))] {
            let dir = test_dir(name);
            write_full_fixture_mit(&dir, true, false, moe);
            gewichte_verrauschen(&dir, 0x5eed_0367);
            skalen_je_kanal_streuen(&dir);
            let model = load_model(&dir).expect("Artefakt muss laden");

            let laenge = 2 * VORBEREITUNGSFENSTER + 3;
            let ids: Vec<usize> = (0..laenge).map(|i| (i * 7 + i / 5) % 3).collect();

            // Der Strom jedes Tokens ueber alle Fenster, gegen den
            // tokenweisen Weg; Begruendung beim Test oben.
            {
                let mut gebuendelt = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
                let mut einzeln = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
                for (f, fenster) in ids.chunks(VORBEREITUNGSFENSTER).enumerate() {
                    let anfang = f * VORBEREITUNGSFENSTER;
                    let zustaende = model.vorbereiten_stapel(fenster, anfang, &mut gebuendelt);
                    for (i, id) in fenster.iter().enumerate() {
                        let erwartet = model.durch_die_ebenen(*id, anfang + i, &mut einzeln);
                        assert_eq!(zustaende[i], erwartet, "{name}: Strom von Token {} weicht ab", anfang + i);
                    }
                }
            }

            for n in [0usize, 1, 2, VORBEREITUNGSFENSTER, VORBEREITUNGSFENSTER + 1, laenge] {
                let mut gebuendelt = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
                let a = model.prompt_vorbereiten(&ids[..n], &mut gebuendelt);

                let mut einzeln = crate::kv_cache::KVCache::new(model.num_layers, model.num_kv_heads);
                let mut b = vec![0i32; model.vocab_size];
                for (pos, id) in ids[..n].iter().enumerate() {
                    b = model.forward_token(*id, pos, &mut einzeln);
                }
                assert_eq!(a, b, "{name}: {n} Token, gebuendelt und tokenweise weichen ab");

                // 📌 **Der Zwischenspeicher wird Eintrag fuer Eintrag
                // verglichen, nicht nur ueber die Logits.** Das Fixture ist
                // so klein, dass seine Logits eine um eins verschobene
                // Fensterposition nicht zeigen; gemessen am 2026-09-14 mit
                // genau dieser Verschiebung, der Test blieb gruen. Die
                // Schluessel tragen die Drehung ihrer Position und stehen
                // unter ihrer Position im Speicher, dort faellt es auf.
                for ebene in 0..model.num_layers {
                    for kopf in 0..model.num_kv_heads {
                        assert_eq!(
                            gebuendelt.read_scheiben(ebene, kopf, laenge),
                            einzeln.read_scheiben(ebene, kopf, laenge),
                            "{name}: {n} Token, KV-Speicher in Ebene {ebene}, Kopf {kopf}"
                        );
                    }
                }

                // Der Zwischenspeicher muss danach derselbe sein, sonst
                // stimmt erst der naechste Schritt nicht mehr.
                if n > 0 {
                    let naechster = (n * 5) % 3;
                    assert_eq!(
                        model.forward_token(naechster, n, &mut gebuendelt),
                        model.forward_token(naechster, n, &mut einzeln),
                        "{name}: {n} Token, der Schritt danach weicht ab"
                    );
                }
            }

            fs::remove_dir_all(&dir).ok();
        }
    }

    /// **An der Kontextgrenze haelt die Erzeugung an, und nichts bricht
    /// still um** (Fund 368).
    ///
    /// Die Grenze ist das Kleinere aus `max_context` und den Zeilen der
    /// RoPE-Tabelle. Geprueft wird beides: eine Erzeugung, die ueber die Grenze
    /// wollte, ein Vorwaertspass an der Grenze und ein Prompt darueber.
    #[test]
    fn an_der_kontextgrenze_haelt_die_erzeugung_an() {
        use crate::generate::{dekodieren_fortgesetzt, dekodieren_mit_digest, Erzeugung, Fortsetzung};
        use crate::kv_cache::KVCache;
        let dir = test_dir("kontextgrenze");
        write_full_fixture_mit(&dir, true, false, None);
        let mut konf: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("model_config.json")).expect("lesen")).expect("parsen");
        konf["max_context"] = serde_json::json!(64);
        write_model_config(&dir, &konf);
        let model = load_model(&dir).expect("Artefakt muss laden");
        assert_eq!(model.kontextgrenze(), 64, "max_context ist kleiner als die Tabelle");

        let prompt: Vec<usize> = (0..60).map(|i| i % 3).collect();
        let lauf = Erzeugung { max_new_tokens: 10, seed: 1, greedy: true, halt: &[] };
        let mut speicher = Fortsetzung::neu(&model);
        let (aus, w) = dekodieren_fortgesetzt(&model, &prompt, &lauf, &mut speicher, &mut |_| {});
        assert_eq!(aus.len(), 5, "Positionen 60 bis 63 werden gerechnet, das Token fuer 64 nur ausgegeben");
        assert!(w.kontext_voll);
        assert_eq!(speicher.laenge(), 64);
        let (digest_aus, _) = dekodieren_mit_digest(&model, &prompt, 10, 1, true);
        assert_eq!(digest_aus, aus, "der Konformitaetspfad haelt an derselben Stelle");

        let (kurz, w) = dekodieren_fortgesetzt(&model, &prompt[..40], &lauf, &mut Fortsetzung::neu(&model), &mut |_| {});
        assert_eq!((kurz.len(), w.kontext_voll), (10, false));

        let voll: Vec<usize> = (0..64).map(|i| i % 3).collect();
        let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);
        model.prompt_vorbereiten(&voll, &mut cache);
        let hinter = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.forward_token(0, 64, &mut cache);
        }));
        assert!(hinter.is_err(), "Position 64 darf nicht auf Position 0 umbrechen");
        let zu_lang: Vec<usize> = (0..65).map(|i| i % 3).collect();
        let zu_lang_ergebnis = std::panic::catch_unwind(|| {
            model.prompt_vorbereiten(&zu_lang, &mut KVCache::new(model.num_layers, model.num_kv_heads));
        });
        assert!(zu_lang_ergebnis.is_err(), "ein Prompt ueber der Grenze ist ein Fehler des Aufrufers");
        fs::remove_dir_all(&dir).ok();
    }

    /// **Eine fortgesetzte Erzeugung rechnet dasselbe wie eine frische**
    /// (Fund 372): verlaengerter Prompt, Prompt mit frueherer Abweichung,
    /// derselbe Prompt noch einmal, dicht und als Gemisch.
    ///
    /// Verglichen werden die erzeugten Token, jeder Eintrag des
    /// KV-Speichers und die Logits eines weiteren Schritts; auf
    /// Zufallsgewichten, weil das Fixture sonst Fehler glaettet (Fund 366).
    #[test]
    fn fortgesetzt_ist_dasselbe_wie_frisch() {
        use crate::generate::{dekodieren_fortgesetzt, Erzeugung, Fortsetzung};
        for (name, moe) in [("fort-dicht", None), ("fort-moe", Some((6usize, 2usize, 3usize)))] {
            let dir = test_dir(name);
            write_full_fixture_mit(&dir, true, false, moe);
            gewichte_verrauschen(&dir, 0x5eed_0372);
            skalen_je_kanal_streuen(&dir);
            let model = load_model(&dir).expect("Artefakt muss laden");
            let lauf = Erzeugung { max_new_tokens: 5, seed: 7, greedy: true, halt: &[] };

            let a: Vec<usize> = (0..40).map(|i| (i * 5 + i / 3) % 3).collect();
            let mut speicher = Fortsetzung::neu(&model);
            let (aus_a, _) = dekodieren_fortgesetzt(&model, &a, &lauf, &mut speicher, &mut |_| {});
            let mut b = a.clone();
            b.extend(&aus_a);
            b.extend([2, 0, 1, 1, 2, 0, 0]);
            let mut c = a[..17].to_vec();
            c.extend([1, 1, 1, 0, 2]);
            let d = c.clone();

            for (fall, prompt, erwartet_wieder) in [("verlaengert", &b, Some(a.len() + aus_a.len())), ("abweichend", &c, Some(17)), ("gleich", &d, Some(d.len() - 1))] {
                let (fort, w) = dekodieren_fortgesetzt(&model, prompt, &lauf, &mut speicher, &mut |_| {});
                let mut frisch = Fortsetzung::neu(&model);
                let (neu, _) = dekodieren_fortgesetzt(&model, prompt, &lauf, &mut frisch, &mut |_| {});
                assert_eq!(fort, neu, "{name}/{fall}: Token");
                if let Some(e) = erwartet_wieder {
                    assert_eq!(w.wiederverwendet, e, "{name}/{fall}: wiederverwendet");
                }
                assert_eq!(speicher.token, frisch.token, "{name}/{fall}: Tokenfolge im Speicher");
                let laenge = speicher.token.len();
                for ebene in 0..model.num_layers {
                    for kopf in 0..model.num_kv_heads {
                        assert_eq!(
                            speicher.cache.read_scheiben(ebene, kopf, laenge),
                            frisch.cache.read_scheiben(ebene, kopf, laenge),
                            "{name}/{fall}: KV-Speicher Ebene {ebene} Kopf {kopf}"
                        );
                    }
                }
                let t = prompt[0];
                assert_eq!(
                    model.forward_token(t, laenge, &mut speicher.cache),
                    model.forward_token(t, laenge, &mut frisch.cache),
                    "{name}/{fall}: Logits des naechsten Schritts"
                );
                speicher.cache.kuerzen(laenge);
            }
            fs::remove_dir_all(&dir).ok();
        }
    }

    /// **Gegenprobe:** Dasselbe Fixture ohne `moe` ergibt eine dichte
    /// Layer. Ohne diesen Vergleich pruefte der Test oben nur, dass
    /// irgendetwas geladen wurde.
    #[test]
    fn ohne_experten_bleibt_die_layer_dicht() {
        let dir = test_dir("moe-dicht");
        write_full_fixture_mit(&dir, true, false, None);

        let model = load_model(&dir).expect("dichtes Artefakt muss laden");
        assert!(
            matches!(&model.layers[0].ffn, crate::model::Feedforward::Dense(_)),
            "bei num_experts = 0 darf keine MoE-Layer entstehen"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// Ein fehlender Experte faellt beim Laden auf und nicht erst im
    /// Forward-Pass. **Bei 128 Experten je Layer und 48 Layern ist das
    /// der Unterschied zwischen einer Fehlermeldung und einer halben
    /// Stunde Rechenzeit fuer nichts.**
    #[test]
    fn ein_fehlender_experte_faellt_beim_laden_auf() {
        let dir = test_dir("moe-luecke");
        write_full_fixture_mit(&dir, true, false, Some((6, 2, 3)));

        // Den Manifest-Eintrag des letzten Experten entfernen.
        let pfad = dir.join("weights_manifest.json");
        let text = fs::read_to_string(&pfad).expect("Manifest lesen");
        let mut manifest: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&text).expect("Manifest parsen");
        manifest
            .remove("model_layers_0_mlp_experts_5_up_proj_weight")
            .expect("Eintrag muss es geben");
        fs::write(&pfad, serde_json::to_string(&manifest).unwrap()).expect("Manifest schreiben");
        write_theta_v(&dir);

        // `IntegerModel` traegt kein Debug (die Gewichte waeren
        // unlesbar), deshalb von Hand statt ueber expect_err.
        let err = match load_model(&dir) {
            Err(e) => e,
            Ok(_) => panic!("fehlender Experte muss auffallen"),
        };
        assert!(
            err.contains("experts.5.up_proj"),
            "die Fehlermeldung muss sagen, welcher Experte fehlt: {err}"
        );

        fs::remove_dir_all(&dir).ok();
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
        assert_eq!(&model.lm_head.data[..], &model.embedding_table.data[..]);
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

        assert_eq!(model.cos_lut.len(), 2048);
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
