//! Kompletter Transformer-Graph fuer Qwen2.5-0.5B
//! 
//! Embedding -> [Layer x 24] -> Final RMSNorm -> LM Head
//! Jeder Layer: RMSNorm -> Attention -> ResAdd -> RMSNorm -> MLP -> ResAdd

use integer_llm_kernels::fixed_point::{clamp_i8, clamp_i16, rescale};
use integer_llm_kernels::rmsnorm::rmsnorm_int8;
use integer_llm_kernels::linear::linear_w8a8;
use integer_llm_kernels::rope::rotate_pairs;
use integer_llm_kernels::softmax::softmax_int;
use integer_llm_kernels::attention::attention_int;
use integer_llm_kernels::mlp::mlp_int;
use integer_llm_kernels::sampling::{argmax_int, sample_integer_cdf};
use integer_llm_kernels::prng::seed_from_ids;
use crate::kv_cache::KVCache;
use crate::loader::{ThetaV, LoadedScales};
use std::collections::HashMap;

/// Quantisierungs-Metadaten fuer einen Tensor.
#[derive(Debug, Clone)]
pub struct QTensor {
    pub data: Vec<i8>,      // flat, row-major
    pub shape: Vec<usize>,
    pub shift: u8,          // Rechts-Shift fuer Reskalierung (Zweierpotenz)
}

impl QTensor {
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

/// Ein Transformer-Layer.
pub struct TransformerLayer {
    pub layer_idx: usize,
    pub input_layernorm_gamma: Vec<i8>,
    pub post_attention_layernorm_gamma: Vec<i8>,
    pub q_proj: QTensor,
    pub k_proj: QTensor,
    pub v_proj: QTensor,
    pub o_proj: QTensor,
    pub gate_proj: QTensor,
    pub up_proj: QTensor,
    pub down_proj: QTensor,
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
    pub final_norm_gamma: Vec<i8>,
    pub layers: Vec<TransformerLayer>,
    pub cos_lut: Vec<i16>,
    pub sin_lut: Vec<i16>,
    pub exp_lut: Vec<i16>,
    pub silu_lut: Vec<i16>,
    /// Kalibrierte Aktivierungsskalen aus scales.json (geladen und
    /// validiert, siehe Loader-Punkt 12.9). Noch nicht in den Forward-Pass
    /// verdrahtet: `ModelConfig` traegt weiterhin globale, nicht per-Layer
    /// aufgeloeste frac_bits-Werte. Siehe Hinweis zu Fahrplan-Punkt 12.10.
    pub activation_scales: LoadedScales,
    pub config: ModelConfig,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub act_frac_bits: u8,
    /// Fallback-Gewichtsskala, nur falls ein Tensor keine eigene kalibrierte
    /// Skala traegt (z. B. in Tests). Der Forward-Pass verwendet fuer reale
    /// Gewichte durchgehend `QTensor.shift` des jeweiligen Tensors, nicht
    /// diesen globalen Wert - siehe Hinweis zum Numerik-Fix nach 12.10.
    pub weight_frac_bits: u8,
    pub residual_frac_bits: u8,
    /// Ziel-Fracbits des Q*K-Scores NACH dem Rescale, bevor er in die
    /// exp-LUT indiziert. Muss mit dem Kalibrierungsbereich der LUT
    /// uebereinstimmen (`generate_exp_lut(..., frac_bits=8)` in
    /// `calibrate/src/main.py`) - beide sind bewusst an denselben Wert
    /// gekoppelt, siehe `forward_layer()`.
    pub score_frac_bits: u8,
    pub prob_frac_bits: u8,
    pub rmsnorm_eps: i32,
    pub rope_frac_bits: u8,
    pub silu_lut_shift: u8,
    pub silu_lut_offset: i16,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            act_frac_bits: 6,
            weight_frac_bits: 7,
            residual_frac_bits: 8,
            score_frac_bits: 8,
            prob_frac_bits: 8,
            rmsnorm_eps: 1,  // 1/256 bei frac_bits=8
            rope_frac_bits: 8,
            silu_lut_shift: 0,
            silu_lut_offset: 128,
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
        // 1. Embedding Lookup
        let mut hidden = self.embedding_table.row(token_id);

        // 2. Transformer Layers
        for layer in &self.layers {
            hidden = self.forward_layer(layer, &hidden, pos, cache);
        }

        // 3. Final RMSNorm
        let mut normed = vec![0i8; self.hidden_size];
        let norm_out = rmsnorm_int8(&hidden, &self.final_norm_gamma, self.config.residual_frac_bits, self.config.rmsnorm_eps);
        normed.copy_from_slice(&norm_out);

        // 4. LM Head (INT8 x INT8 -> INT32 Logits)
        let mut logits = vec![0i32; self.vocab_size];
        for row in 0..self.vocab_size {
            let mut acc: i32 = 0;
            let weight_row = self.lm_head.row(row);
            for (w, v) in weight_row.iter().zip(normed.iter()) {
                acc += (*w as i32) * (*v as i32);
            }
            logits[row] = rescale(acc, self.lm_head.shift + self.config.residual_frac_bits, self.config.act_frac_bits);
        }

        logits
    }

    fn forward_layer(
        &self,
        layer: &TransformerLayer,
        hidden: &[i8],
        pos: usize,
        cache: &mut KVCache,
    ) -> Vec<i8> {
        let cfg = &self.config;
        let hs = self.hidden_size;

        // === Attention-Block ===
        // Pre-Attention RMSNorm
        let norm_hidden = rmsnorm_int8(hidden, &layer.input_layernorm_gamma, cfg.residual_frac_bits, cfg.rmsnorm_eps);

        // Q, K, V Projektionen: Gewichtsskala kommt aus der jeweils eigenen
        // kalibrierten QTensor.shift (siehe Numerik-Fix nach 12.10), nicht
        // aus der globalen weight_frac_bits-Konstante.
        let q_flat = linear_w8a8(&norm_hidden, &self.to_vec_vec(&layer.q_proj), cfg.act_frac_bits, layer.q_proj.shift, cfg.act_frac_bits);
        let k_flat = linear_w8a8(&norm_hidden, &self.to_vec_vec(&layer.k_proj), cfg.act_frac_bits, layer.k_proj.shift, cfg.act_frac_bits);
        let v_flat = linear_w8a8(&norm_hidden, &self.to_vec_vec(&layer.v_proj), cfg.act_frac_bits, layer.v_proj.shift, cfg.act_frac_bits);

        // Auf Heads aufteilen. Q hat num_heads Heads, K/V bei GQA nur
        // num_kv_heads (Qwen2.5-0.5B: 14 vs. 2) - deshalb getrennte Aufteilung
        // statt eines gemeinsamen Head-Counts.
        let mut q_heads = self.split_heads(&q_flat, self.num_heads);
        let mut k_heads = self.split_heads(&k_flat, self.num_kv_heads);
        let v_heads = self.split_heads(&v_flat, self.num_kv_heads);

        // RoPE: Q- und K-Heads separat rotieren (unterschiedliche Head-Anzahl,
        // daher kein gemeinsamer apply_rope-Aufruf, der gleiche Laenge voraussetzt).
        let idx = pos % self.cos_lut.len();
        let cos_q = self.cos_lut[idx];
        let sin_q = self.sin_lut[idx];
        for qh in q_heads.iter_mut() {
            *qh = rotate_pairs(qh, cos_q, sin_q, cfg.rope_frac_bits);
        }
        for kh in k_heads.iter_mut() {
            *kh = rotate_pairs(kh, cos_q, sin_q, cfg.rope_frac_bits);
        }

        // KV-Cache schreiben (ein Eintrag pro KV-Head, nicht pro Query-Head)
        for h in 0..self.num_kv_heads {
            cache.write(layer.layer_idx, h, pos,
                self.head_to_i16(&k_heads[h], cfg.act_frac_bits),
                self.head_to_i16(&v_heads[h], cfg.act_frac_bits));
        }

        // Attention pro Query-Head; group_size aufeinanderfolgende Query-Heads
        // teilen sich denselben KV-Head (Standard-GQA-Gruppierung, wie in
        // HF's repeat_kv: Head h liest KV-Head h / group_size).
        let group_size = self.num_heads / self.num_kv_heads;
        let mut attn_out = vec![0i8; hs];
        for h in 0..self.num_heads {
            let kv_h = h / group_size;
            let (past_k, past_v) = cache.read(layer.layer_idx, kv_h, pos);
            let seq_len = past_k.len();

            // K, V als i8 zurueckskalieren
            let k_seq: Vec<Vec<i8>> = past_k.iter()
                .map(|k| self.head_from_i16(k, cfg.act_frac_bits))
                .collect();
            let v_seq: Vec<Vec<i8>> = past_v.iter()
                .map(|v| self.head_from_i16(v, cfg.act_frac_bits))
                .collect();

            let q_seq = vec![q_heads[h].clone()];

            // Causal mask: nur letzte Position attendet auf alle vorherigen
            let mask = vec![vec![true; seq_len]];

            // Q und K liegen beide bei act_frac_bits (siehe Projektionen oben),
            // der rohe Skalarproduktwert traegt daher 2*act_frac_bits
            // Nachkommabits. score_shift bringt ihn exakt auf den
            // Kalibrierungsbereich der exp-LUT (score_frac_bits) herunter,
            // dadurch bleibt lut_shift=0 korrekt (vormals hartkodiert 0 bei
            // gleichzeitig zu grossem, nicht darauf abgestimmtem score_shift -
            // die Attention-Gewichtung war dadurch faktisch verzerrt).
            let score_shift = (2u16 * cfg.act_frac_bits as u16)
                .saturating_sub(cfg.score_frac_bits as u16) as u8;

            let head_out = attention_int(
                &q_seq, &k_seq, &v_seq, &mask,
                score_shift, &self.exp_lut, 0, cfg.prob_frac_bits,
            );

            // Ergebnis in attn_out schreiben
            for d in 0..self.head_dim {
                attn_out[h * self.head_dim + d] = head_out[0][d];
            }
        }

        // O-Projektion (Gewichtsskala aus layer.o_proj.shift statt Konstante)
        let o_out = linear_w8a8(&attn_out, &self.to_vec_vec(&layer.o_proj), cfg.act_frac_bits, layer.o_proj.shift, cfg.residual_frac_bits);

        // Residual Add (INT16-Pfad empfohlen, hier vereinfacht)
        let mut residual = vec![0i8; hs];
        for i in 0..hs {
            let sum = (hidden[i] as i16) + (o_out[i] as i16);
            residual[i] = clamp_i8(sum as i32);
        }

        // === MLP-Block ===
        let norm_residual = rmsnorm_int8(&residual, &layer.post_attention_layernorm_gamma, cfg.residual_frac_bits, cfg.rmsnorm_eps);

        // Gewichtsskalen aus den jeweils eigenen QTensor.shift statt einer
        // gemeinsamen Konstante fuer alle drei Projektionen.
        let mlp_out = mlp_int(
            &norm_residual,
            &self.to_vec_vec(&layer.gate_proj),
            &self.to_vec_vec(&layer.up_proj),
            &self.to_vec_vec(&layer.down_proj),
            &self.silu_lut,
            cfg.act_frac_bits,
            layer.gate_proj.shift,
            layer.up_proj.shift,
            layer.down_proj.shift,
            cfg.residual_frac_bits,
            cfg.silu_lut_shift,
            cfg.silu_lut_offset,
        );

        // Final Residual Add
        let mut out = vec![0i8; hs];
        for i in 0..hs {
            let sum = (residual[i] as i16) + (mlp_out[i] as i16);
            out[i] = clamp_i8(sum as i32);
        }

        out
    }

    /// Teilt einen flachen Q/K/V-Vektor in `n` Heads zu je `head_dim` auf.
    /// `n` ist `num_heads` fuer Q, bei GQA `num_kv_heads` fuer K/V.
    fn split_heads(&self, flat: &[i8], n: usize) -> Vec<Vec<i8>> {
        let mut heads = Vec::with_capacity(n);
        for h in 0..n {
            let start = h * self.head_dim;
            let end = start + self.head_dim;
            heads.push(flat[start..end].to_vec());
        }
        heads
    }

    fn head_to_i16(&self, head: &[i8], frac_bits: u8) -> Vec<i16> {
        head.iter().map(|v| ((*v as i32) << (frac_bits - 6)) as i16).collect()
    }

    fn head_from_i16(&self, head: &[i16], frac_bits: u8) -> Vec<i8> {
        head.iter().map(|v| clamp_i8((*v as i32) >> (frac_bits - 6))).collect()
    }

    fn to_vec_vec(&self, qt: &QTensor) -> Vec<Vec<i8>> {
        let mut out = Vec::with_capacity(qt.rows());
        for r in 0..qt.rows() {
            out.push(qt.row(r));
        }
        out
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
