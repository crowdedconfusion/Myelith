//! NVIDIA CUDA-Backend
//!
//! Feature-Gate: `cargo build --features cuda`
//!
//! Status: Delegations-Stub. Alle Operationen werden an die Referenz-
//! kernel weitergereicht (numerisch identisch, nicht beschleunigt). Echte
//! CUDA-Kernel brauchen GPU-Hardware zum Pruefen und sind deshalb hier
//! bewusst nicht geschrieben, aus demselben Grund wie der AVX2-Pfad in
//! `dot.rs`: Unverifizierte Numerik in einem Konsenspfad laesst einen
//! Miner mit abweichendem Kernel slashen, ohne dass er etwas falsch
//! gemacht hat.
//!
//! **Solange hier delegiert wird, besteht kein Konformitaetslauf mit
//! `--features cuda`**, siehe `kernels/src/rechenpfad.rs`.
//!
//! ## Determinismus-Vertrag fuer echte Kernel
//!
//! **Korrigiert am 2026-08-22.** Vorher stand hier: feste Blockgroesse,
//! im Code vorgeschriebene Summationsreihenfolge, kein Warp-Shuffle.
//! Drei dieser vier Auflagen sind fuer die Bitgleichheit **nicht noetig**
//! und haetten einen GPU-Kernel ohne Gegenwert verlangsamt: Sie haetten
//! die Reduktion serialisiert, obwohl gerade die Reduktion auf einer GPU
//! parallel laufen soll.
//!
//! **Der Grund: Die Akkumulation ist exakt und damit assoziativ.**
//! Nachgerechnet fuer die groesste Reduktionslaenge des Projekts
//! (Qwen2.5-7B, `intermediate_size` 18944):
//!
//! | | |
//! |---|---|
//! | groesstes Einzelprodukt | 127 x 32768 = 4 161 536 |
//! | groesste moegliche Summe | 78 836 137 984, also 2^36 |
//! | Fassungsvermoegen i64 | 2^63 |
//! | Sicherheitsabstand | Faktor 117 000 000 |
//!
//! Kein Ueberlauf, keine Rundung, keine Saettigung im Zwischenergebnis.
//! Ganzzahlige Addition ohne Ueberlauf ist assoziativ und kommutativ,
//! **also liefert jede Reduktionsreihenfolge dasselbe i64**. Baumreduktion,
//! Warp-Shuffle, beliebige Blockgroessen: alles erlaubt.
//!
//! ### Was tatsaechlich gilt
//!
//! 1. **Nur Ganzzahlen, nie Gleitkomma.** Das ist die eigentliche
//!    Auflage, und sie gilt ohne Ausnahme.
//! 2. **Keine Tensor Cores.** Nicht weil Akkumulation dort
//!    grundsaetzlich nichtdeterministisch waere, sondern weil ihre
//!    Pfade in reduzierter Breite akkumulieren und Operationen
//!    verschmelzen. Beides bricht die exakte i64-Summe.
//! 3. **Saettigung genau einmal, ganz am Ende.** Das ist die feine
//!    Bedingung, an der die Assoziativitaet haengt: Wuerde ein Kernel
//!    Teilsummen klemmen, waere die Reihenfolge ploetzlich wieder
//!    wirksam. Clamp gehoert in `rescale_i64`/`clamp_i16_from_i64` und
//!    nirgends sonst.
//! 4. **Keine Annahme ueber die Warp-Breite.** Nicht wegen des
//!    Determinismus, sondern wegen der Portierbarkeit: NVIDIA 32, AMD 64.
//!
//! Die Assoziativitaet ist in `dot.rs` als Test festgehalten
//! (`jede_reduktionsreihenfolge_liefert_dasselbe`), zusammen mit der
//! Kopfrechnung zum Abstand. Faellt einer der beiden, gilt dieser
//! Vertrag nicht mehr.
//!
//! Ziel-Vertrag seit theta_v 0.7.0:
//! Gewichte int8 (Per-Channel-Skalen), Aktivierungen int16 (Per-Layer-Skalen),
//! i64-Akkumulation, divisionsfreie RMSNorm mit LUT-gestuetztem rsqrt,
//! RNE-Rundung, Saettigung (Clamp).
// Die Gewichtsmatrizen heissen wie im Whitepaper (Anhang B): `W`,
// `W_gate`, `W_up`, `W_down` — konsistent mit den uebrigen Kerneln.
#![allow(non_snake_case)]
// Die Signaturen tragen den vollstaendigen Fixed-Point-Vertrag.
#![allow(clippy::too_many_arguments)]

use crate::backend::Backend;
use crate::linear::linear_w8a16;
use crate::rmsnorm::rmsnorm_i16;
use crate::softmax::softmax_int;
use crate::attention::attention_int;
use crate::rope::apply_rope_i16;
use crate::mlp::mlp_int;

pub struct CudaBackend {
    device_id: usize,
    sm_version: u32,
}

impl CudaBackend {
    /// Initialisiert das CUDA-Backend.
    ///
    /// Hinweis: Ohne CUDA-Runtime (nvcc, libcuda) wird eine Platzhalter-
    /// SM-Version zurueckgegeben. Echte Initialisierung erfordert:
    /// - cudaSetDevice(device_id)
    /// - cudaDeviceGetAttribute fuer SM-Version
    /// - cuBLAS/cuDNN-Handles (fuer zukuenftige beschleunigte Pfade)
    pub fn init(device_id: usize) -> Result<Self, String> {
        // TODO: Echte CUDA-Initialisierung wenn Runtime verfuegbar
        Ok(CudaBackend {
            device_id,
            sm_version: 80, // Placeholder: SM80 (A100)
        })
    }

    pub fn device_id(&self) -> usize {
        self.device_id
    }

    pub fn sm_version(&self) -> u32 {
        self.sm_version
    }
}

impl Backend for CudaBackend {
    fn name(&self) -> &'static str {
        "cuda"
    }

    fn hardware_family(&self) -> &'static str {
        "nvidia-gpu"
    }

    fn feature_tag(&self) -> &'static str {
        "cuda"
    }

    fn linear_w8a16(
        &self,
        x: &[i16],
        W: &[i8],
        out: &mut [i16],
        in_features: usize,
        out_features: usize,
        w_shifts: &[u8],
        act_frac: u8,
        out_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — jeder Thread berechnet ein Output-Element,
        // i64-Akkumulation in exakter Reihenfolge (kein Shuffle).
        // Flach durchgereicht: Der Kernel nimmt seit v0.13.4 Ausschnitte
        // statt Kopien, und das Trait liefert die Gewichte ohnehin flach.
        let result = linear_w8a16(x, W, in_features, w_shifts, act_frac, out_frac);
        out[..out_features].copy_from_slice(&result[..out_features]);
    }

    fn rmsnorm(
        &self,
        x: &[i16],
        x_shifts: &[u8],
        gamma: &[i8],
        gamma_shifts: &[u8],
        rsqrt_lut: &[i16],
        lut_input_shift: u8,
        lut_output_frac: u8,
        inv_n_q20: i64,
        out: &mut [i16],
        out_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — Shared-Memory-Reduktion fuer sum(x^2),
        // dann elementweise Anwendung von rsqrt-LUT und Gamma.
        let result = rmsnorm_i16(x, x_shifts, gamma, gamma_shifts, rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
        out.copy_from_slice(&result);
    }

    fn softmax(
        &self,
        logits: &[i32],
        out: &mut [i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — Block-Reduce fuer Max und Summe,
        // exp-LUT-Lookup, RNE-Normalisierung.
        let result = softmax_int(logits, exp_lut, lut_shift, frac_bits);
        out.copy_from_slice(&result);
    }

    fn attention(
        &self,
        q: &[Vec<i16>],
        k: &[Vec<i16>],
        v: &[Vec<i16>],
        out: &mut [Vec<i16>],
        mask: &[Vec<bool>],
        score_mult: i64,
        score_shift: u8,
        exp_lut: &[i16],
        lut_shift: u8,
        prob_frac: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — Flash-Attention-aehnlich mit tiled
        // Q*K^T, Online-Softmax, und V-Gewichtung. Block-uebergreifende
        // Synchronisation erforderlich.
        let result = attention_int(q, k, v, mask, score_mult, score_shift, exp_lut, lut_shift, prob_frac);
        for (i, row) in result.iter().enumerate() {
            out[i].copy_from_slice(row);
        }
    }

    fn rope(
        &self,
        q: &mut [Vec<i16>],
        k: &mut [Vec<i16>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — Thread-per-Pair, cos/sin aus LUT,
        // RNE-Rundung mit i32-Arithmetik.
        let (q_out, k_out) = apply_rope_i16(q, k, cos_lut, sin_lut, positions, frac_bits);
        for (i, row) in q_out.iter().enumerate() {
            q[i].copy_from_slice(row);
        }
        for (i, row) in k_out.iter().enumerate() {
            k[i].copy_from_slice(row);
        }
    }

    fn mlp(
        &self,
        x: &[i16],
        W_gate: &[i8],
        W_up: &[i8],
        W_down: &[i8],
        out: &mut [i16],
        gate_w_shifts: &[u8],
        up_w_shifts: &[u8],
        down_w_shifts: &[u8],
        silu_lut: &[i16],
        in_frac: u8,
        gate_out_frac: u8,
        up_out_frac: u8,
        down_in_frac: u8,
        silu_in_frac: u8,
        silu_lut_offset: i16,
        silu_out_frac: u8,
        out_frac: &[u8],
    ) {
        // Delegiert an Referenz-Kernel.
        // TODO: Echter CUDA-Kernel — Fused Gate+Up+SiLU+Down, oder
        // separate Kernel fuer jede Projektion mit SiLU-Fusion.
        let hidden_size = x.len();
        let intermediate_size = W_gate.len() / hidden_size;

        let result = mlp_int(
            x,
            W_gate, W_up, W_down,
            hidden_size, intermediate_size,
            gate_w_shifts, up_w_shifts, down_w_shifts,
            silu_lut,
            in_frac,
            gate_out_frac, up_out_frac, down_in_frac,
            silu_in_frac, silu_lut_offset, silu_out_frac,
            out_frac,
        );
        out.copy_from_slice(&result);
    }
}
