//! Kompletter Transformer-Graph fuer Qwen2.5-0.5B
//! 
//! Embedding -> [Layer x 24] -> Final RMSNorm -> LM Head
//! Jeder Layer: RMSNorm -> Attention -> ResAdd -> RMSNorm -> MLP -> ResAdd
// Die Schleifenvariable `row` ist hier die Ausgabe-Zeile der
// Gewichtsmatrix und wird als solche auch fuer die Per-Channel-Shifts
// gebraucht — ein Iterator ueber `logits` allein wuerde den Bezug
// verlieren. Der Heisspfad wird zudem gegen Golden Vectors geprueft;
// eine rein stilistische Umschreibung waere hier reines Risiko.
#![allow(clippy::needless_range_loop)]
// `Vec<(Vec<i8>, Vec<u8>)>`-Rueckgaben spiegeln das Artefaktformat
// (Gewichte + Per-Channel-Shifts). Ein Typalias wuerde die Struktur
// verstecken, die beim Lesen des Loaders gebraucht wird.
#![allow(clippy::type_complexity)]

use integer_llm_kernels::fixed_point::{clamp_i16, clamp_i16_from_i64, inv_sqrt_q15, rescale, rescale_i64};
use integer_llm_kernels::moe::{mische_experten, route_top_k};
use integer_llm_kernels::rmsnorm::{qk_norm_heads, rmsnorm_i16};
use integer_llm_kernels::linear::{linear_w8a16, linear_w8a16_pc, add_bias_i16};
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_kernels::attention::attention_int;
use integer_llm_kernels::mlp::mlp_int;
use integer_llm_kernels::sampling::{argmax_int, sample_integer_cdf};
use crate::kv_cache::KVCache;
use crate::loader::{ThetaV, LoadedScales};

/// Die Bytes eines quantisierten Tensors: im Heap oder als Abbild der
/// Artefaktdatei.
///
/// ## ⚑ Fund 62 (2026-08-25): Ein 29-GiB-Artefakt gehört nicht in den Heap
///
/// Der Loader las jede Gewichtsdatei mit `std::fs::read` in einen `Vec`.
/// Für Qwen2.5-0,5B (0,74 GB) und 7B (8,1 GB) trägt das. Das Artefakt von
/// Qwen3-30B-A3B ist **29 GiB** gegen 24 GiB Arbeitsspeicher, und der
/// Versuch zeigte das schlechteste denkbare Verhalten: Während der
/// Prozess las, schrieb das Betriebssystem seinen Heap in die
/// Auslagerung, RSS fiel von 8,9 auf 1,9 GiB, während der Swap von 2,9
/// auf 4,1 GB wuchs. **Von der Platte lesen und sofort wieder auf die
/// Platte schreiben**, und zwar mit echten Schreibzugriffen auf die SSD.
///
/// **Der Unterschied ist die Art der Seite, nicht ihre Zahl.** Eine
/// anonyme Heap-Seite muss ausgelagert, also geschrieben werden. Eine
/// dateigestützte Seite ist sauber: Das System verwirft sie und liest
/// sie bei Bedarf neu. Dieselbe Datenmenge, aber ohne Schreiblast und
/// ohne Auslagerungsdruck.
///
/// **Für ein Expertengemisch ist es mehr als eine Notlösung.** Bei
/// Top-8 von 128 rührt ein Token nur ein Sechzehntel der
/// Expertengewichte an. Die übrigen bleiben ungelesen auf der Platte,
/// statt Speicher zu belegen, den sie nie brauchen. Der Zuschnitt
/// „jeder Knoten hält alle Experten seiner Layer" wird damit erst
/// bezahlbar.
pub enum Gewichtsdaten {
    /// Im Heap, wie bisher. Für kleine Tensoren und für Tests.
    Speicher(Vec<i8>),
    /// Abbild der Artefaktdatei.
    Abbild(memmap2::Mmap),
}

impl std::ops::Deref for Gewichtsdaten {
    type Target = [i8];

    fn deref(&self) -> &[i8] {
        match self {
            Gewichtsdaten::Speicher(v) => v,
            Gewichtsdaten::Abbild(abbild) => {
                // SICHERHEIT: `i8` und `u8` haben dieselbe Größe und
                // dieselbe Ausrichtung, und jedes Bitmuster ist für
                // beide gültig. Der Zeiger stammt aus einem gültigen
                // Abbild, das mindestens so lange lebt wie die
                // zurückgegebene Referenz (beide hängen an `self`).
                // Geschrieben wird nie: Das Abbild ist nur lesend
                // geöffnet.
                unsafe {
                    std::slice::from_raw_parts(
                        abbild.as_ptr() as *const i8,
                        abbild.len(),
                    )
                }
            }
        }
    }
}

impl From<Vec<i8>> for Gewichtsdaten {
    fn from(v: Vec<i8>) -> Self {
        Gewichtsdaten::Speicher(v)
    }
}

impl std::fmt::Debug for Gewichtsdaten {
    /// Zeigt Herkunft und Länge, **nicht die Bytes**. Ein Tensor mit
    /// drei Millionen Einträgen im Fehlertext ist keine Hilfe.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let art = match self {
            Gewichtsdaten::Speicher(_) => "Speicher",
            Gewichtsdaten::Abbild(_) => "Abbild",
        };
        write!(f, "Gewichtsdaten::{art}({} Byte)", self.len())
    }
}

/// Quantisierungs-Metadaten fuer einen Tensor.
#[derive(Debug, Clone)]
pub struct QTensor {
    /// Hinter `Arc`, damit `QTensor` klonbar bleibt: Ein Abbild lässt
    /// sich nicht kopieren, und der Loader reicht denselben Tensor an
    /// mehrere Stellen weiter. Der `Arc` kostet einen Zähler, nicht die
    /// Daten.
    pub data: std::sync::Arc<Gewichtsdaten>,      // flat, row-major
    pub shape: Vec<usize>,
    /// Zweierpotenz-Shift je Zeile (theta_v 0.7.0: Per-Channel-Skalen;
    /// bei 1D-Tensoren wie Biases/Gammas je Element). Ältere Artefakte mit
    /// Per-Tensor-Skala werden vom Loader als replizierter Shift geladen.
    pub shifts: Vec<u8>,    // Rechts-Shifts fuer Reskalierung, len == shape[0]
}

impl QTensor {
    /// Aus Bytes im Heap. Für Tests und für kleine Tensoren.
    pub fn aus_speicher(data: Vec<i8>, shape: Vec<usize>, shifts: Vec<u8>) -> Self {
        QTensor {
            data: std::sync::Arc::new(Gewichtsdaten::Speicher(data)),
            shape,
            shifts,
        }
    }

    pub fn n_elements(&self) -> usize {
        self.shape.iter().product()
    }

    pub fn rows(&self) -> usize {
        self.shape[0]
    }

    pub fn cols(&self) -> usize {
        self.shape[1]
    }

    pub fn row(&self, idx: usize) -> Vec<i8> {
        let cols = self.cols();
        self.data[idx * cols .. (idx + 1) * cols].to_vec()
    }
}

/// Kalibrierte Per-Layer-Aktivierungsskalen (Zweierpotenzen, aus
/// `scales.json`; Schluessel-Konvention identisch zu
/// `calibrate/src/stats.py`). Seit dem Numerik-Realitaetsabgleich (v0.12.20)
/// tragen Aktivierungen int16 mit diesen Skalen; der Loader validiert die
/// Vollstaendigkeit beim Modellbau.
#[derive(Debug, Clone)]
pub struct LayerScales {
    /// Ausgang von input_layernorm = Eingang von q/k/v_proj.
    pub norm_attn_frac: u8,
    /// Ausgaenge der q/k/v-Projektionen (Q/K/V-Skala, beeinflusst
    /// score_shift und KV-Cache-Reskalierung).
    pub q_frac: u8,
    pub k_frac: u8,
    pub v_frac: u8,
    /// Ausgang des Attention-Moduls = Eingang von o_proj.
    pub attn_out_frac: u8,
    /// Ausgang von post_attention_layernorm = Eingang von gate/up_proj.
    pub norm_mlp_frac: u8,
    /// Ausgaenge von gate-/up_proj.
    pub gate_frac: u8,
    pub up_frac: u8,
    /// h = silu(gate)*up = Eingang von down_proj.
    pub down_in_frac: u8,
    /// Residualstrom-Segment am Eingang dieses Layers
    /// (Eingang von input_layernorm). Per-Segment-Skalen seit spec 0.5.1:
    /// die Spanne des Stroms reicht von winzigen Embedding-Werten bis zu
    /// Ausreisser-Spitzen — eine globale Skala wuerde einen der beiden
    /// Bereiche zerstoeren. Seit Fund 20 (theta_v 0.11.0) zusaetzlich eine
    /// Skala JE KANAL: bei Qwen2.5-7B tragen 3-4 feste Kanaele an
    /// Position 0 "Massive Activations" (absmax ~9600 gegenueber ~10 im
    /// Rest, Sun et al. 2024) - eine gemeinsame Skala fuers ganze Segment
    /// wuerde jeden anderen Kanal auf Schrittweite 0,5 zwingen.
    pub residual_in_frac: Vec<u8>,
    /// Mittleres Residualstrom-Segment zwischen erstem Residual-Add und
    /// post_attention_layernorm. Ebenfalls Per-Kanal (Fund 20).
    pub residual_mid_frac: Vec<u8>,
}

/// Ein Transformer-Layer.
/// Der Feedforward-Teil einer Layer.
///
/// **Warum ein Enum und nicht drei Tensoren plus ein optionales
/// Expertengemisch.** Ein Modell kann beides mischen: Qwen3 kennt das
/// Feld `mlp_only_layers`, mit dem einzelne Layer dicht bleiben, während
/// der Rest Experten hat. Bei Qwen3-30B-A3B ist die Liste leer, aber sie
/// existiert, und ein Typ, der den Mischfall nicht ausdrücken kann,
/// müsste beim ersten Modell mit gemischten Layern umgebaut werden.
///
/// Vor allem: Zwei `Option`-Felder ließen den Zustand „beides" und den
/// Zustand „keines" zu. Beides wäre ein Ladefehler, den erst der
/// Forward-Pass bemerkt.
pub enum Feedforward {
    /// Gate, Up, Down wie in Qwen2.5 und in jeder dichten Layer.
    Dense(DenseMlp),
    /// Ein Router und `num_experts` Experten, von denen je Token genau
    /// `top_k` rechnen.
    Moe(MoeLayer),
}

/// Die drei Matrizen einer Feedforward-Einheit.
///
/// Ein **Experte** eines MoE ist strukturell dasselbe wie eine dichte
/// MLP, nur schmaler (`moe_intermediate_size` statt
/// `intermediate_size`). Deshalb derselbe Typ: Eine eigene Struktur mit
/// denselben drei Feldern wäre eine zweite Wahrheit, die beim nächsten
/// Formatwechsel auseinanderliefe.
pub struct DenseMlp {
    pub gate_proj: QTensor,
    pub up_proj: QTensor,
    pub down_proj: QTensor,
}

/// Ein Expertengemisch: Router plus Experten.
pub struct MoeLayer {
    /// Router-Projektion, `[num_experts, hidden_size]`. Liefert je
    /// Experte einen Logit.
    pub router: QTensor,
    /// Kalibrierte Ausgangsskala der Router-Projektion.
    pub router_frac: u8,
    /// Alle Experten dieser Layer. **Alle liegen vor, es rechnen `top_k`.**
    /// Das ist der Zuschnitt, der die Pod-Kette unverändert lässt: Ein
    /// Knoten hält alle Experten seiner Layer, und weil je Layer und
    /// Token exakt `top_k` feuern, ist seine Arbeitsmenge eine Konstante
    /// aus der Modellkonfiguration statt einer Größe je Anfrage.
    pub experts: Vec<DenseMlp>,
    /// Wie viele Experten je Token feuern (`num_experts_per_tok`).
    pub top_k: usize,
    /// Ob die Gewichte der gewählten Experten auf eins normiert werden
    /// (`norm_topk_prob`).
    pub norm_topk_prob: bool,
}

pub struct TransformerLayer {
    pub layer_idx: usize,
    /// Gamma der input_layernorm als QTensor: `data` + eigener kalibrierter
    /// Shift (vor v0.12.20 wurde der Shift verworfen, siehe Fund 1).
    pub input_layernorm_gamma: QTensor,
    pub post_attention_layernorm_gamma: QTensor,
    pub q_proj: QTensor,
    pub k_proj: QTensor,
    pub v_proj: QTensor,
    pub o_proj: QTensor,
    /// Der Feedforward-Teil: eine dichte MLP oder ein Expertengemisch.
    pub ffn: Feedforward,
    /// Q/K/V-Attention-Biases (Qwen2.5 besitzt sie an q/k/v_proj). `None`
    /// bei Modellen ohne Attention-Biases (`attention_bias: false` in
    /// model_config.json); sonst je ein Bias-Tensor mit eigener Skala,
    /// der nach der Projektion addiert wird (siehe `add_bias_i16`).
    pub q_bias: Option<crate::loader::BiasTensor>,
    pub k_bias: Option<crate::loader::BiasTensor>,
    pub v_bias: Option<crate::loader::BiasTensor>,
    /// QK-Norm (Qwen3). `None` bei Modellen ohne (`qk_norm: false`).
    pub qk_norm: Option<QkNorm>,
    /// Kalibrierte Per-Layer-Aktivierungsskalen.
    pub scales: LayerScales,
}

/// QK-Norm einer Layer: die beiden Gammas und ihre Ausgangsskalen.
///
/// **Warum ein eigener Typ und nicht vier Felder.** Vier `Option`-Felder
/// koennten halb besetzt sein: Gamma ohne Skala, oder Q ohne K. Beides
/// waere ein Ladefehler, den erst der Forward-Pass bemerkt, und dort
/// bemerkt er ihn als falsche Zahlen und nicht als Fehler. Als ein Typ
/// gibt es entweder alles oder nichts, und der Loader muss die
/// Vollstaendigkeit nicht pruefen, weil sie sich nicht ausdruecken laesst.
/// Dieselbe Ueberlegung wie bei `RebuildAnlass` in `myl-pod`.
pub struct QkNorm {
    /// Gamma fuer die Query-Normierung, Laenge `head_dim`. Alle Koepfe
    /// teilen sich dasselbe Gamma.
    pub q_gamma: QTensor,
    /// Gamma fuer die Key-Normierung, Laenge `head_dim`.
    pub k_gamma: QTensor,
    /// Ausgangsskala der Query-Normierung. Sie loest `q_frac` als
    /// Eingangsskala von RoPE und Attention ab.
    pub q_out_frac: u8,
    /// Ausgangsskala der Key-Normierung.
    pub k_out_frac: u8,
}

/// Das komplette Modell.
pub struct IntegerModel {
    pub theta_v: ThetaV,
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    /// Anzahl der Key/Value-Heads bei Grouped-Query-Attention (GQA).
    /// `num_heads` muss ein Vielfaches von `num_kv_heads` sein; je
    /// `num_heads / num_kv_heads` aufeinanderfolgende Query-Heads teilen sich
    /// einen KV-Head (Qwen2.5-0.5B: 14 Query-Heads, 2 KV-Heads).
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub max_context: usize,
    pub embedding_table: QTensor,   // [vocab_size, hidden_size]
    pub lm_head: QTensor,           // [vocab_size, hidden_size] (oder mit embedding_table getied)
    /// INT16-LM-Head mit Per-Channel-Skalen (benannte spec-Ausnahme 0.6.0,
    /// Eskalation nach dem Entscheidungspunkt 12.21). Falls vorhanden, wird
    /// er für die Logits verwendet; `lm_head` (int8, getied) dient dann nur
    /// noch als Fallback-Pfad für ältere Artefakte.
    pub lm_head_int16: Option<crate::loader::LmHead>,
    /// Gamma der finalen RMSNorm als QTensor (data + kalibrierter Shift).
    pub final_norm_gamma: QTensor,
    /// Kalibrierte Skala des finalen Norm-Ausgangs = Eingang des LM-Heads.
    pub final_norm_frac: u8,
    /// Kalibrierte Skala des letzten Residualstrom-Segments (Eingang von
    /// model.norm; Per-Segment-Skalen seit spec 0.5.1, Per-Kanal seit
    /// Fund 20 / theta_v 0.11.0).
    pub final_residual_frac: Vec<u8>,
    pub layers: Vec<TransformerLayer>,
    pub cos_lut: Vec<i16>,
    pub sin_lut: Vec<i16>,
    pub exp_lut: Vec<i16>,
    pub silu_lut: Vec<i16>,
    /// rsqrt-LUT (spec: rsqrt.method = "lut"), konsumiert von `rmsnorm_i16`
    /// mit dynamischem geradem Index-Shift.
    pub rsqrt_lut: Vec<i16>,
    /// Reziproken-Konstante 2^20/hidden_size fuer den divisionsfreien
    /// Mittelwert in `rmsnorm_i16` (einmalige Initialisierung).
    pub inv_n_q20: i64,
    /// Kalibrierte Aktivierungsskalen aus scales.json (vollstaendig
    /// validiert und in den Forward-Pass verdrahtet, v0.12.20).
    pub activation_scales: LoadedScales,
    pub config: ModelConfig,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Skala des KV-Cache (spec: kv_cache, frac 8). K/V werden beim
    /// Schreiben/Lesen zwischen ihrer Per-Layer-Skala und dieser Skala
    /// umgerechnet.
    /// Historisch: bis theta_v 0.11.0 wurde der KV-Cache auf diese
    /// GLOBALE Skala umgerechnet und beim Lesen zurueck. Seit Fund 22
    /// (theta_v 0.12.0) haelt der Cache K/V in der nativen Per-Layer-Skala
    /// der erzeugenden Projektion — die Umrechnung war ein reiner Verlust
    /// (Quelle und Ziel identisch), kostete 2-4 Bit auf fast jeder Ebene
    /// und clippte bei Qwen2.5-7B in Ebene 0 um Faktor 3,28.
    ///
    /// Das Feld bleibt erhalten, weil es im Artefaktformat steht und von
    /// Diagnosen ausgegeben wird; der Inferenzpfad liest es nicht mehr.
    pub kv_cache_frac_bits: u8,
    /// Skala der Q*K-Scores NACH dem Rescale, bevor sie in die exp-LUT
    /// indizieren. Muss mit dem Kalibrierungsbereich der LUT
    /// uebereinstimmen (spec: softmax.exp_lut_frac_bits).
    pub score_frac_bits: u8,
    /// Eingangsskala der exp-LUT (spec: softmax.exp_input_frac_bits):
    /// Index i der LUT steht fuer den Score-Differenz-Realwert i * 2^-Wert.
    /// Der lut_shift der Attention ist score_frac_bits - exp_input_frac_bits.
    pub exp_input_frac: u8,
    pub prob_frac_bits: u8,
    pub rope_frac_bits: u8,
    /// Feste Eingangsskala der SiLU-LUT (spec: silu.input_frac_bits);
    /// Gate-Werte werden vor dem Lookup dorthin reskaliert.
    pub silu_in_frac: u8,
    /// Index-Offset der SiLU-LUT = -input_min (spec: silu.input_range).
    pub silu_lut_offset: i16,
    /// Ausgangsskala der SiLU-LUT (spec: silu.output_frac_bits).
    pub silu_out_frac: u8,
    /// Parameter der rsqrt-LUT (spec: rsqrt.input_shift / output_frac_bits).
    pub rsqrt_input_shift: u8,
    pub rsqrt_output_frac: u8,
    /// Skala der Logits (nur fuer Sampling/Argmax; gemeinsame Skala reicht,
    /// da beide skaleninvariant sind).
    pub logit_frac_bits: u8,
}

impl Default for ModelConfig {
    fn default() -> Self {
        // Fallback-Konstanten fuer Tests ohne spec-Parsing; der reale
        // Modellbau (build_model) liest die Werte aus der eingebetteten
        // theta_v/spec.json.
        ModelConfig {
            kv_cache_frac_bits: 8,
            score_frac_bits: 8,
            exp_input_frac: 4,
            prob_frac_bits: 8,
            rope_frac_bits: 8,
            silu_in_frac: 3,
            silu_lut_offset: 1024,
            silu_out_frac: 6,
            rsqrt_input_shift: 8,
            rsqrt_output_frac: 8,
            logit_frac_bits: 6,
        }
    }
}

impl IntegerModel {
    /// Einzelner Forward-Schritt fuer ein Token an Position `pos`.
    /// KV-Cache wird gelesen und geschrieben.
    pub fn forward_token(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> Vec<i32> {
        let cfg = &self.config;

        // 1. Embedding Lookup: Gewicht int8 mit Per-Channel-Skala der
        //    Token-Zeile (theta_v 0.7.0) -> erstes Residualstrom-Segment.
        //    Seit Fund 20 (theta_v 0.11.0) traegt das Segment eine Skala
        //    je Kanal.
        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        let mut hidden: Vec<i16> = emb
            .iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect();

        // 2. Transformer Layers: jeder Layer gibt den Strom auf der Skala
        //    des Folge-Segments aus (Eingangsskala des naechsten Layers bzw.
        //    des finalen Norm-Eingangs).
        for (i, layer) in self.layers.iter().enumerate() {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(layer, &hidden, pos, cache, out_frac);
        }

        // 3. Final RMSNorm (int16 -> int16 auf der kalibrierten
        //    final-norm-Skala; LUT-gestuetzt, divisionsfrei).
        let normed = rmsnorm_i16(
            &hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
        );

        // 4. LM Head.
        //    Pfad A (spec-Ausnahme): INT16-LM-Head mit Per-Channel-
        //    Skalen — i64-Akkumulator (896 * 32767 * 32767 > i32) und
        //    Zeilen-Rescale auf die gemeinsame Logit-Skala (jede Zeile hat
        //    ihren eigenen Zweierpotenz-Shift).
        //    Pfad B (Fallback, ältere Artefakte mit Weight-Tying): INT8 x
        //    INT16 -> INT32 Logits, i64-Akkumulator, Per-Channel-Zeilen-
        //    Rescale (theta_v 0.7.0).
        let mut logits = vec![0i32; self.vocab_size];
        if let Some(lmh) = &self.lm_head_int16 {
            let hidden_dim = normed.len();
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let base = row * hidden_dim;
                for (d, v) in normed.iter().enumerate() {
                    acc += (lmh.data[base + d] as i64) * (*v as i64);
                }
                let row_frac = lmh.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        } else {
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let weight_row = self.lm_head.row(row);
                for (w, v) in weight_row.iter().zip(normed.iter()) {
                    acc += (*w as i64) * (*v as i64);
                }
                let row_frac = self.lm_head.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        }

        logits
    }

    // === Pipeline-Stage-API (Phase 12.56–12.59) =======================
    // Die drei Methoden zerlegen `forward_token` in Stage-Bausteine:
    // Embedding (Stage 0), Layer-Block (alle Stages), Norm+LM-Head
    // (letzte Stage). Zusammengesetzt mit denselben Skalen und
    // Rundungen wie der Einzelknoten-Pfad — eine Stage-Pipeline rechnet
    // dadurch dieselben Werte (plus dokumentierte Boundary-Reskalierung
    // zwischen den Stages, siehe `integer-llm-pipeline`).

    /// Embedding-Lookup: liefert das erste Residualstrom-Segment auf der
    /// Eingangsskala von Layer 0 (`layers[0].scales.residual_in_frac`).
    pub fn embed_token(&self, token_id: usize) -> Vec<i16> {
        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        emb.iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect()
    }

    /// Führt die Layer `[layer_start, layer_end)` auf dem Residualstrom
    /// aus. Eingang auf der Skala von
    /// `layers[layer_start].scales.residual_in_frac`, Ausgang auf der
    /// Skala von `layers[layer_end].scales.residual_in_frac` (bzw.
    /// `final_residual_frac` für `layer_end == num_layers`).
    /// KV-Cache wird gelesen und geschrieben (absolute Layer-Indizes).
    pub fn run_layers(
        &self,
        mut hidden: Vec<i16>,
        pos: usize,
        cache: &mut KVCache,
        layer_start: usize,
        layer_end: usize,
    ) -> Vec<i16> {
        for i in layer_start..layer_end {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(&self.layers[i], &hidden, pos, cache, out_frac);
        }
        hidden
    }

    /// Finale RMSNorm + LM-Head: Eingang auf `final_residual_frac`,
    /// Ausgabe die Logits auf `config.logit_frac_bits`.
    pub fn head_logits(&self, hidden: &[i16]) -> Vec<i32> {
        let cfg = &self.config;
        let normed = rmsnorm_i16(
            hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
        );
        let mut logits = vec![0i32; self.vocab_size];
        if let Some(lmh) = &self.lm_head_int16 {
            let hidden_dim = normed.len();
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let base = row * hidden_dim;
                for (d, v) in normed.iter().enumerate() {
                    acc += (lmh.data[base + d] as i64) * (*v as i64);
                }
                let row_frac = lmh.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        } else {
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let weight_row = self.lm_head.row(row);
                for (w, v) in weight_row.iter().zip(normed.iter()) {
                    acc += (*w as i64) * (*v as i64);
                }
                let row_frac = self.lm_head.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        }
        logits
    }

    fn forward_layer(
        &self,
        layer: &TransformerLayer,
        hidden: &[i16],
        pos: usize,
        cache: &mut KVCache,
        out_residual_frac: &[u8],
    ) -> Vec<i16> {
        let cfg = &self.config;
        let hs = self.hidden_size;
        let sc = &layer.scales;

        // === Attention-Block ===
        // Pre-Attention RMSNorm (int16 -> int16 auf der kalibrierten
        // q/k/v-Eingangsskala; Gamma mit Per-Element-Skalen, theta_v 0.7.0).
        let norm_hidden = rmsnorm_i16(
            hidden,
            &sc.residual_in_frac,
            &layer.input_layernorm_gamma.data,
            &layer.input_layernorm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            sc.norm_attn_frac,
        );

        // Q, K, V Projektionen: Per-Channel-Gewichtsskalen (theta_v 0.7.0),
        // Ausgang auf der jeweils kalibrierten Per-Layer-Skala.
        // Die Gewichte liegen im `QTensor` flach und werden flach
        // durchgereicht. Bis v0.13.4 stand hier `self.to_vec_vec(...)`,
        // das je Aufruf eine Heap-Allokation und eine Kopie **je
        // Ausgabe-Zeile** erzeugte: bei 0,5B zusammen 358 MB und 304 128
        // Allokationen je Token, denn diese Umwandlung lief achtmal je
        // Ebene und die Ebenen 24-mal je Token. Die Zahlen ändern sich
        // dadurch nicht, `dot_i8_i16` bekommt dieselben Bytes in
        // derselben Reihenfolge.
        let mut q_flat = linear_w8a16(&norm_hidden, &layer.q_proj.data, layer.q_proj.cols(), &layer.q_proj.shifts, sc.norm_attn_frac, sc.q_frac);
        let mut k_flat = linear_w8a16(&norm_hidden, &layer.k_proj.data, layer.k_proj.cols(), &layer.k_proj.shifts, sc.norm_attn_frac, sc.k_frac);
        let mut v_flat = linear_w8a16(&norm_hidden, &layer.v_proj.data, layer.v_proj.cols(), &layer.v_proj.shifts, sc.norm_attn_frac, sc.v_frac);

        // Attention-Biases (Qwen2.5: q/k/v_proj besitzen welche):
        // Per-Element-Skalen, Reskalierung auf die Q/K/V-Ausgabeskala und
        // i64-Addition mit Clamping — reine Ganzzahlarithmetik.
        if let Some(qb) = &layer.q_bias {
            add_bias_i16(&mut q_flat, &qb.data, &qb.shifts, sc.q_frac);
        }
        if let Some(kb) = &layer.k_bias {
            add_bias_i16(&mut k_flat, &kb.data, &kb.shifts, sc.k_frac);
        }
        if let Some(vb) = &layer.v_bias {
            add_bias_i16(&mut v_flat, &vb.data, &vb.shifts, sc.v_frac);
        }

        // Auf Heads aufteilen. Q hat num_heads Heads, K/V bei GQA nur
        // num_kv_heads (Qwen2.5-0.5B: 14 vs. 2) - deshalb getrennte Aufteilung
        // statt eines gemeinsamen Head-Counts.
        let mut q_heads = self.split_heads(&q_flat, self.num_heads);
        let mut k_heads = self.split_heads(&k_flat, self.num_kv_heads);
        let v_heads = self.split_heads(&v_flat, self.num_kv_heads);

        // QK-Norm (Qwen3): RMSNorm je Kopf ueber head_dim, **vor** RoPE.
        //
        // Die Reihenfolge ist nicht verhandelbar. RoPE dreht ein Paar
        // (x_j, x_{j+half}) um einen positionsabhaengigen Winkel; eine
        // Normierung danach wuerde ueber gedrehte Werte mitteln und das
        // Ergebnis von der Position abhaengig machen, obwohl die Norm es
        // nicht sein soll. Das Referenzmodell normiert vorher, und die
        // Reihenfolge ist Teil des Ausfuehrungsprofils.
        //
        // Die Ausgangsskala wechselt hier von `q_frac`/`k_frac` auf die
        // kalibrierte Norm-Ausgangsskala; RoPE selbst ist skaleninvariant
        // und reicht sie unveraendert weiter.
        let (q_akt_frac, k_akt_frac) = match &layer.qk_norm {
            Some(qkn) => {
                qk_norm_heads(
                    &mut q_heads,
                    sc.q_frac,
                    &qkn.q_gamma.data,
                    &qkn.q_gamma.shifts,
                    &self.rsqrt_lut,
                    cfg.rsqrt_input_shift,
                    cfg.rsqrt_output_frac,
                    qkn.q_out_frac,
                );
                qk_norm_heads(
                    &mut k_heads,
                    sc.k_frac,
                    &qkn.k_gamma.data,
                    &qkn.k_gamma.shifts,
                    &self.rsqrt_lut,
                    cfg.rsqrt_input_shift,
                    cfg.rsqrt_output_frac,
                    qkn.k_out_frac,
                );
                (qkn.q_out_frac, qkn.k_out_frac)
            }
            None => (sc.q_frac, sc.k_frac),
        };

        // RoPE (Fund-15-Fix, theta_v 0.10.0): Multi-Frequenz-RoPE mit
        // half-split-Paarung. Die cos/sin-LUTs sind flach row-major
        // [max_seq_len, head_dim/2]; je Position wird die Zeile
        // [idx*half, (idx+1)*half) gelesen und jedes Paar j nutzt seinen
        // eigenen Winkel. Q- und K-Heads separat rotieren (unterschiedliche
        // Head-Anzahl). Die Rotation ist skaleninvariant gegenueber der
        // Eingangs-Skala (cos/sin tragen rope_frac_bits).
        let half = self.head_dim / 2;
        let n_pos = self.cos_lut.len() / half;
        let idx = pos % n_pos;
        let cos_row = &self.cos_lut[idx * half..(idx + 1) * half];
        let sin_row = &self.sin_lut[idx * half..(idx + 1) * half];
        for qh in q_heads.iter_mut() {
            *qh = rotate_half_split_i16(qh, cos_row, sin_row, cfg.rope_frac_bits);
        }
        for kh in k_heads.iter_mut() {
            *kh = rotate_half_split_i16(kh, cos_row, sin_row, cfg.rope_frac_bits);
        }

        // KV-Cache schreiben — OHNE Reskalierung (Fund 22, 2026-08-19).
        //
        // Bis theta_v 0.11.0 wurde hier auf eine GLOBALE Cache-Skala
        // (spec: kv_cache.frac_bits = 8) konvertiert und beim Lesen
        // zurueck auf dieselbe Per-Layer-Skala. Diese Rundreise
        // k_frac -> 8 -> k_frac gewann nichts (Quelle und Ziel sind
        // identisch, da Schreiben und Lesen dieselbe Ebene betreffen),
        // kostete aber:
        //   - doppelte Rundung je Position und Kopf,
        //   - 2-4 Bit Aufloesung auf FAST JEDER Ebene beider Modelle
        //     (0,5B: k median 3 Bit, v median 4 Bit; 7B: k 2, v 4),
        //   - hartes Clipping, wo der reale Wert die feste Kapazitaet
        //     von 32767/2^8 = 128 uebersteigt (7B Ebene 0: K-absmax 420,
        //     also Faktor 3,28 abgeschnitten - und das an der ERSTEN
        //     Ebene, deren Fehler durch alle 28 Ebenen propagiert).
        //
        // Der Cache haelt K/V jetzt in der nativen Per-Layer-Skala der
        // erzeugenden Projektion. Das ist streng verlustfrei gegenueber
        // vorher und beseitigt beide Effekte.
        for h in 0..self.num_kv_heads {
            cache.write(layer.layer_idx, h, pos,
                k_heads[h].clone(), v_heads[h].clone());
        }

        // Attention pro Query-Head; group_size aufeinanderfolgende Query-Heads
        // teilen sich denselben KV-Head (Standard-GQA-Gruppierung, wie in
        // HF's repeat_kv: Head h liest KV-Head h / group_size).
        let group_size = self.num_heads / self.num_kv_heads;
        // ⚑ **Fund 59, zweite Stelle.** Hier stand `vec![0i16; hs]`, also
        // `hidden_size`. Geschrieben wird bis `num_heads · head_dim`, und
        // solange beide Zahlen zusammenfielen (Qwen2.5-0,5B und -7B), war
        // das dasselbe. Bei Qwen3-4B sind es 2560 gegen 4096, und der
        // erste Lauf brach hier ab: „len is 2560 but the index is 2560".
        //
        // `o_proj` bildet von `num_heads · head_dim` auf `hidden_size`
        // zurück, das ist die Stelle, an der die Breite wechselt, nicht
        // diese hier.
        let mut attn_out = vec![0i16; self.num_heads * self.head_dim];
        for h in 0..self.num_heads {
            let kv_h = h / group_size;
            let (past_k, past_v) = cache.read(layer.layer_idx, kv_h, pos);
            let seq_len = past_k.len();

            // K, V liegen bereits in ihrer Per-Layer-Skala (Fund 22) —
            // keine Reskalierung mehr noetig.
            let k_seq: Vec<Vec<i16>> = past_k.to_vec();
            let v_seq: Vec<Vec<i16>> = past_v.to_vec();

            let q_seq = vec![q_heads[h].clone()];

            // Causal mask: nur letzte Position attendet auf alle vorherigen
            let mask = vec![vec![true; seq_len]];

            // Q liegt bei `q_akt_frac`, K bei `k_akt_frac`; der rohe
            // Skalarproduktwert traegt deren Summe an Nachkommabits.
            //
            // **Ohne QK-Norm sind das sc.q_frac und sc.k_frac, mit QK-Norm
            // die Ausgangsskalen der Normierung.** Die Unterscheidung ist
            // der Grund, warum die beiden Werte oben als eigene Variablen
            // entstehen und nicht hier aus `sc` gelesen werden: Ein
            // vergessenes `sc.q_frac` an dieser Stelle waere kein
            // Uebersetzungsfehler, sondern eine um Zweierpotenzen
            // verschobene Softmax. score_shift bringt ihn auf
            // die Score-Skala (score_frac_bits); exp_lut_shift uebersetzt von
            // dort in die Eingangsskala der exp-LUT (spec 0.5.2: Domaene
            // [0, 64) statt [0, 0.5) — gemessene Score-Differenzen bis ~28).
            //
            // Fund 17 (Attention-Skalierung): HF-Qwen2 skaliert die Scores mit
            // 1/sqrt(head_dim) (attn_weights = q·k * head_dim^-0.5). Dieser
            // Faktor fehlte urspruenglich ganz; die Softmax war dadurch um
            // sqrt(head_dim) zu scharf.
            //
            // Fund 19: Die Umsetzung als reiner Rechtsshift um
            // log2(head_dim)/2 war Ganzzahldivision und damit nur fuer
            // GERADE Zweierpotenzen richtig. head_dim 128 (der Normalfall ab
            // 1,5B) bekam Shift 3 statt der noetigen 3,5 — Faktor sqrt(2) zu
            // gross. Jetzt als Q15-Multiplikation; fuer head_dim 64 ist der
            // Multiplikator 4096 = 2^12 und das Ergebnis bitgleich zum
            // bisherigen Verhalten (siehe fixed_point::inv_sqrt_q15).
            let score_mult = inv_sqrt_q15(self.head_dim);
            let score_shift = (q_akt_frac as u16 + k_akt_frac as u16 + 15)
                .saturating_sub(cfg.score_frac_bits as u16) as u8;
            let exp_lut_shift = cfg.score_frac_bits.saturating_sub(cfg.exp_input_frac);

            let head_out = attention_int(
                &q_seq, &k_seq, &v_seq, &mask,
                score_mult, score_shift, &self.exp_lut, exp_lut_shift,
                cfg.prob_frac_bits,
            );

            // Ergebnis in attn_out schreiben
            for d in 0..self.head_dim {
                attn_out[h * self.head_dim + d] = head_out[0][d];
            }
        }

        // Die Attention-Ausgabe liegt auf der V-Skala (gewichtete Summe
        // erhaelt die V-Skala); Umreskalieren auf die kalibrierte
        // o_proj-Eingangsskala.
        if sc.attn_out_frac != sc.v_frac {
            for v in attn_out.iter_mut() {
                *v = clamp_i16(rescale(*v as i32, sc.v_frac, sc.attn_out_frac));
            }
        }

        // O-Projektion: Eingangsskala = Attention-Ausgabe, Ausgang auf der
        // Skala des mittleren Residual-Segments (vor der zweiten Norm).
        // Fund 20: per-Kanal-Ziel statt Skalar, o_proj addiert direkt in
        // den Residualstrom.
        // Fund 31 (theta_v 0.17.0): Akkumulationsskala je Kanal ist die
        // GROEBERE der beiden Segmentskalen (kleinerer Shift). Grund siehe
        // beim zweiten Residual-Add unten — dort trat der Fehler auf.
        let acc_attn: Vec<u8> = sc
            .residual_in_frac
            .iter()
            .zip(sc.residual_mid_frac.iter())
            .map(|(&a, &b)| a.min(b))
            .collect();
        let o_out = linear_w8a16_pc(&attn_out, &layer.o_proj.data, layer.o_proj.cols(), &layer.o_proj.shifts, sc.attn_out_frac, &acc_attn);

        // Residual Add 1: beide Operanden auf der Akkumulationsskala, Summe
        // in i64, DANN eine Reskalierung auf die mittlere Segmentskala und
        // eine Klemmung. Vor Fund 31 wurde `hidden` einzeln auf die
        // Zielskala geklemmt, bevor addiert wurde.
        let mut residual = vec![0i16; hs];
        for i in 0..hs {
            let h = rescale_i64(hidden[i] as i64, sc.residual_in_frac[i], acc_attn[i]);
            let summe = h + o_out[i] as i64;
            residual[i] = clamp_i16_from_i64(rescale_i64(summe, acc_attn[i], sc.residual_mid_frac[i]));
        }

        // === MLP-Block ===
        let norm_residual = rmsnorm_i16(
            &residual,
            &sc.residual_mid_frac,
            &layer.post_attention_layernorm_gamma.data,
            &layer.post_attention_layernorm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            sc.norm_mlp_frac,
        );

        // Per-Layer-Skalen fuer alle Zwischenstufen; die SiLU-LUT arbeitet
        // in ihrer festen Domaene (silu_in_frac/Offset), Gate-Werte werden
        // dorthin reskaliert. out_residual_frac ist seit Fund 20 per Kanal
        // (down_proj addiert direkt in den Residualstrom).
        let acc_mlp: Vec<u8> = sc
            .residual_mid_frac
            .iter()
            .zip(out_residual_frac.iter())
            .map(|(&a, &b)| a.min(b))
            .collect();
        let mlp_out = match &layer.ffn {
            Feedforward::Dense(mlp) => {
                self.mlp_vorwaerts(mlp, &norm_residual, sc, cfg, &acc_mlp)
            }
            Feedforward::Moe(moe) => {
                self.moe_vorwaerts(moe, &norm_residual, sc, cfg, &acc_mlp)
            }
        };

        // Final Residual Add — Fund 31 (2026-08-20, theta_v 0.17.0).
        //
        // Vorher wurde der Residualstrom EINZELN auf die Ausgangsskala
        // geklemmt und erst dann der MLP-Beitrag addiert. An einer
        // Ausloeschung zerstoert das den Wert vollstaendig, denn beide
        // Operanden koennen gross sein, waehrend nur ihre SUMME klein ist —
        // und die Ausgangsskala ist nach der Summe kalibriert.
        //
        // Gemessen an Qwen2.5-0,5B, Ebene 21, Kanal 62 (der Kanal mit der
        // "massive activation", Sun et al. 2024): Der wahre Wert faellt dort
        // von 1714 auf 61,6. Mit mid_frac 4 (+-2047,94) und out_frac 9
        // (+-64,00) ergab die alte Fassung
        //     Residualstrom  1723,2 -> roh 882 304 -> clamp  32 767 =  64,00
        //     MLP-Beitrag   -1653,0 -> roh -846 336 -> clamp -32 768 = -64,00
        //     Summe                                          -1 roh = -0,0020
        // also -0,002 statt 61,6 — zwei Klemmungen, die einander aufheben.
        //
        // Jetzt: Beide Operanden liegen auf der GROEBEREN der beiden Skalen
        // (kleinerer Shift), dort passen sie hinein; die Summe entsteht in
        // i64 und wird EINMAL auf die Ausgangsskala reskaliert und EINMAL
        // geklemmt. Die Aufloesung der groeberen Skala genuegt fuer das
        // Ergebnis: 61,6 bei Schrittweite 1/16 sind rund 10 Bit.
        //
        // Warum `min` und nicht `max`: `linear_w8a16_pc` liefert int16. Auf
        // einer feinen Skala wuerde der MLP-Beitrag selbst klemmen, bevor er
        // ueberhaupt addiert werden kann. Die Akkumulationsskala muss also
        // den OPERANDEN genuegen, nicht dem Ergebnis.
        let mut out = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(residual[i] as i64, sc.residual_mid_frac[i], acc_mlp[i]);
            let summe = r + mlp_out[i] as i64;
            out[i] = clamp_i16_from_i64(rescale_i64(summe, acc_mlp[i], out_residual_frac[i]));
        }

        out
    }

    /// Diagnose-Variante von `forward_token`: gibt zusätzlich je Layer den
    /// AbsMax und die ersten vier Werte des Residualstroms nach dem Layer
    /// zurück (inkl. der Skala des Segments). Nur für Messpfade — der
    /// Inferenzpfad bleibt unverändert.
    ///
    /// **Fund 20:** der Residualstrom trägt seit theta_v 0.11.0 eine Skala
    /// je Kanal, nicht mehr eine je Segment. Ein einzelnes `frac` fürs
    /// ganze Segment gibt es deshalb nicht mehr sinnvoll her — die
    /// Rückgabe liefert je Stufe den vollständigen Vektor UND seine
    /// Per-Kanal-Shifts, damit der Aufrufer jeden Kanal mit SEINEM
    /// eigenen Shift umrechnen kann.
    ///
    /// Die Umrechnung in Realwerte geschieht bewusst erst im aufrufenden
    /// Diagnose-Binary: `model.rs` steht auf der Heißpfad-Liste des
    /// Gleitkomma-Audits (`tests/audit/test_no_float.py`) und bleibt
    /// deshalb vollständig ganzzahlig.
    pub fn forward_token_dump(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> (Vec<i32>, Vec<(Vec<i16>, Vec<u8>)>) {
        let cfg = &self.config;

        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        let mut hidden: Vec<i16> = emb
            .iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect();

        let mut dump = Vec::with_capacity(self.layers.len());
        for (i, layer) in self.layers.iter().enumerate() {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(layer, &hidden, pos, cache, out_frac);
            dump.push((hidden.clone(), out_frac.to_vec()));
        }

        // Finale Norm + LM-Head-Logits wie im echten Pfad.
        let normed = rmsnorm_i16(
            &hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
        );
        // Ausgang der finalen Norm bleibt skalar (Post-Norm-Aktivierungen
        // haben keine Massive-Activation-Ausreisser, siehe rmsnorm.rs).
        let norm_shifts = vec![self.final_norm_frac; normed.len()];
        dump.push((normed.clone(), norm_shifts));

        let mut logits = vec![0i32; self.vocab_size];
        if let Some(lmh) = &self.lm_head_int16 {
            let hidden_dim = normed.len();
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let base = row * hidden_dim;
                for (d, v) in normed.iter().enumerate() {
                    acc += (lmh.data[base + d] as i64) * (*v as i64);
                }
                let row_frac = lmh.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        } else {
            for row in 0..self.vocab_size {
                let mut acc: i64 = 0;
                let weight_row = self.lm_head.row(row);
                for (w, v) in weight_row.iter().zip(normed.iter()) {
                    acc += (*w as i64) * (*v as i64);
                }
                let row_frac = self.lm_head.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, cfg.logit_frac_bits);
                logits[row] = y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        }

        (logits, dump)
    }

    /// Teilt einen flachen Q/K/V-Vektor in `n` Heads zu je `head_dim` auf.
    /// `n` ist `num_heads` fuer Q, bei GQA `num_kv_heads` fuer K/V.
    /// Eine dichte Feedforward-Einheit. Auch der einzelne Experte eines
    /// MoE laeuft hier durch: Er ist dieselbe Rechnung auf schmaleren
    /// Matrizen.
    fn mlp_vorwaerts(
        &self,
        mlp: &DenseMlp,
        x: &[i16],
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
    ) -> Vec<i16> {
        mlp_int(
            x,
            &mlp.gate_proj.data,
            &mlp.up_proj.data,
            &mlp.down_proj.data,
            mlp.gate_proj.cols(),
            mlp.down_proj.cols(),
            &mlp.gate_proj.shifts,
            &mlp.up_proj.shifts,
            &mlp.down_proj.shifts,
            &self.silu_lut,
            sc.norm_mlp_frac,
            sc.gate_frac,
            sc.up_frac,
            sc.down_in_frac,
            cfg.silu_in_frac,
            cfg.silu_lut_offset,
            cfg.silu_out_frac,
            acc,
        )
    }

    /// Ein Expertengemisch: Router befragen, `top_k` Experten rechnen,
    /// Ergebnisse gewichtet mischen.
    ///
    /// **Die Arbeitsmenge ist konstant.** Es feuern immer genau `top_k`
    /// Experten, nie mehr und nie weniger; welche, haengt am Token, wie
    /// viele nicht. Daran haengt, dass die vTFE-Zuschreibung ohne
    /// Anfragezustand nachrechenbar bleibt.
    ///
    /// ⚑ **Kein Token-Dropping.** Es gibt hier keine Kapazitaetsgrenze,
    /// und das ist Absicht: Verwirft eine Implementierung Token, wenn ein
    /// Experte im Batch ueberlaeuft, haengt das Ergebnis an Position *i*
    /// davon ab, welche anderen Token im Batch lagen. Zwei redundante
    /// Pods batchen verschieden, und der Redundanzvergleich meldete zwei
    /// ehrliche Pods als abweichend.
    fn moe_vorwaerts(
        &self,
        moe: &MoeLayer,
        x: &[i16],
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
    ) -> Vec<i16> {
        // Router-Logits. Die Projektion laeuft wie jede andere; ihre
        // Ausgangsskala ist kalibriert wie die der uebrigen Projektionen.
        let logits_i16 = linear_w8a16(
            x,
            &moe.router.data,
            moe.router.cols(),
            &moe.router.shifts,
            sc.norm_mlp_frac,
            moe.router_frac,
        );
        let logits: Vec<i32> = logits_i16.iter().map(|l| *l as i32).collect();

        // Von der Router-Skala in die Eingangsskala der exp-Tabelle,
        // dieselbe Uebersetzung wie bei den Attention-Scores.
        let exp_lut_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
        let routing = route_top_k(
            &logits,
            moe.top_k,
            &self.exp_lut,
            exp_lut_shift,
            cfg.prob_frac_bits,
            moe.norm_topk_prob,
        );

        // Die gewaehlten Experten rechnen. Alle schreiben auf dieselbe
        // Ausgangsskala `acc`, denn sie addieren in denselben
        // Residualstrom; deshalb mischt `mische_experten` ohne
        // Umskalierung.
        let ausgaben: Vec<Vec<i16>> = routing
            .experten
            .iter()
            .map(|e| self.mlp_vorwaerts(&moe.experts[*e as usize], x, sc, cfg, acc))
            .collect();

        mische_experten(&ausgaben, &routing.gewichte, cfg.prob_frac_bits)
    }

    fn split_heads(&self, flat: &[i16], n: usize) -> Vec<Vec<i16>> {
        let mut heads = Vec::with_capacity(n);
        for h in 0..n {
            let start = h * self.head_dim;
            let end = start + self.head_dim;
            heads.push(flat[start..end].to_vec());
        }
        heads
    }


    /// Greedy Decoding fuer ein Token.
    pub fn greedy_next(&self, logits: &[i32]) -> usize {
        argmax_int(logits)
    }

    /// Sampling mit deterministischem Seed.
    pub fn sample_next(&self, logits: &[i32], seed: u64) -> (usize, u64) {
        sample_integer_cdf(logits, seed)
    }
}
