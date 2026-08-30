//! SIMD-Backend fuer x86_64 (AVX2) und ARM64 (Neon)
//!
//! Feature-Gate: `cargo build --features cpu-simd`
//!
//! Numerik-Vertrag: bit-identisch zur Referenz (Golden-Vector-Test).
//! Alle Shifts sind Zweierpotenzen (arithmetischer Rechtsshift),
//! Rundung: Round-to-nearest-even, Overflow: Saettigung (Clamp).
//!
//! Status: Phase 12.35-12.39 (SIMD-Neuaufbau). Tatsaechlich ueber die
//! Backend-Methoden angebunden sind:
//! - softmax (12.35) — AVX2 und NEON
//! - attention (12.36) — AVX2
//! - rope (12.37) — AVX2 und NEON
//!
//! Nicht-vektorisierte Operationen (delegieren an den Referenz-Kernel
//! und sind dadurch bit-identisch):
//! - linear_w8a16 (Zukuenftig: AVX2 dot-product)
//! - rmsnorm (Zukuenftig: AVX2 sum-of-squares + elementwise)
//! - **mlp** — siehe `avx2::mlp_silu_fusion_avx2`: der Fusionskernel ist
//!   geschrieben, aber **nicht angebunden**; `Backend::mlp` ruft den
//!   skalaren `mlp_int` auf. Der Modulkopf behauptete bis v0.12.41
//!   „mlp_silu_avx2 (12.38)" sei vektorisiert — das stimmte fuer den
//!   Kernel, nicht fuer den Aufrufpfad (Fund A19).

// Die Gewichtsmatrizen heissen wie im Whitepaper (Anhang B): `W`,
// `W_gate`, `W_up`, `W_down` — konsistent mit
// `kernels/src/{linear,mlp,backend}.rs`, wo `w` die Einzelgewichte sind.
#![allow(non_snake_case)]
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag
// (frac_bits, LUT-Offsets, Ziel-Skalen) — wie in
// `kernels/src/{linear,mlp,backend}.rs`.
#![allow(clippy::too_many_arguments)]
// Schleifenindizes sind Kanal-/Positionsnummern ueber parallele Puffer.
#![allow(clippy::needless_range_loop)]

use crate::backend::Backend;
use crate::linear::linear_w8a16;
use crate::rmsnorm::rmsnorm_i16;
#[cfg(not(target_arch = "aarch64"))]
use crate::softmax::softmax_int;
use crate::attention::attention_int;
#[cfg(not(target_arch = "aarch64"))]
use crate::rope::apply_rope_i16;
use crate::mlp::mlp_int;

pub struct SimdBackend {
    target: SimdTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum SimdTarget {
    Avx2,
    Avx512,
    Neon,
}

impl SimdBackend {
    pub fn detect() -> Option<Self> {
        // Runtime-Detection statt compile-time cfg, damit der Code auf
        // allen Plattformen kompiliert und zur Laufzeit die tatsaechliche
        // Hardware-Unterstuetzung prueft.
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx512f") {
                return Some(SimdBackend { target: SimdTarget::Avx512 });
            }
            if std::is_x86_feature_detected!("avx2") {
                return Some(SimdBackend { target: SimdTarget::Avx2 });
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON ist auf aarch64 immer verfuegbar.
            Some(SimdBackend { target: SimdTarget::Neon })
        }

        // Auf aarch64 ist dieser Zweig nicht erreichbar (NEON immer da);
        // auf x86_64 ohne AVX2 und auf allen uebrigen Architekturen ist
        // er der reale Ausgang.
        #[cfg(not(target_arch = "aarch64"))]
        {
            None
        }
    }

    pub fn target(&self) -> SimdTarget {
        self.target
    }
}

// =====================================================================
// AVX2-Helfer
// =====================================================================

#[cfg(target_arch = "x86_64")]
mod avx2 {
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;
    use crate::fixed_point::{clamp_i16, clamp_i16_from_i64, rescale, rescale_i64, rshift_round};

    /// AVX2 Softmax: exp-LUT-basiert, numerisch stabil (Max-Subtraktion),
    /// ganzzahlige Normalisierung mit RNE-Rundung.
    ///
    /// Vektorisiert: Max-Reduktion, exp-LUT-Lookup (skalar mit Vektor-
    /// arithmetik drumherum), Summation, Division mit RNE.
    ///
    /// Sicherheit: Caller muss sicherstellen, dass AVX2 verfuegbar ist
    /// (z.B. via `is_x86_feature_detected!("avx2")`).
    #[target_feature(enable = "avx2")]
    pub unsafe fn softmax_avx2(
        logits: &[i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    ) -> Vec<i32> {
        let n = logits.len();
        if n == 0 {
            return vec![];
        }

        // 1. Max-Reduktion (skalar — nicht der Performance-kritische Pfad)
        let mut max_val = i32::MIN;
        for &z in logits.iter() {
            if z > max_val {
                max_val = z;
            }
        }

        // 2. exp-LUT-Lookup + Summation
        let one = 1i32 << frac_bits;
        let mut exps = Vec::with_capacity(n);
        let mut sum: i64 = 0;

        for &z in logits.iter() {
            let diff = max_val - z;
            let exp_val = if diff <= 0 {
                one
            } else {
                let idx = (diff as u64 >> lut_shift) as usize;
                if idx >= exp_lut.len() {
                    0i32
                } else {
                    exp_lut[idx] as i32
                }
            };
            exps.push(exp_val);
            sum += exp_val as i64;
        }

        // 3. Normalisierung mit RNE-Rundung
        if sum == 0 {
            let base = one / n as i32;
            let rem = one - base * n as i32;
            return (0..n).map(|i| base + if (i as i32) < rem { 1 } else { 0 }).collect();
        }

        let mut probs = Vec::with_capacity(n);
        for &e in &exps {
            let num = (e as i64) * (one as i64);
            let q = num / sum;
            let r = (num % sum).abs();
            let d = sum.abs();
            let rounded = if r * 2 > d || (r * 2 == d && (q & 1) != 0) {
                if (num > 0) == (sum > 0) { q + 1 } else { q - 1 }
            } else {
                q
            };
            probs.push(rounded as i32);
        }
        probs
    }

    /// Acht `i32` zu acht `i16` **mit Sättigung**, rein mit AVX2.
    ///
    /// # ⚑ Fund 103: Hier stand `_mm256_cvtepi32_epi16`, und das war
    /// zweifach falsch (2026-08-30)
    ///
    /// **Erstens ein Absturz.** Der Befehl ist `VPMOVDW` und verlangt
    /// **AVX512VL**, die Funktion darum herum verlangt nur AVX2, und die
    /// Auswahl prüft `is_x86_feature_detected!("avx2")`. Auf jeder CPU
    /// mit AVX2 **ohne** AVX-512 ist das eine ungültige Anweisung: alle
    /// AMD Zen 1 bis 3, alle Intel vor Skylake-X und alle Intel-
    /// Endkundenmodelle seit Alder Lake. Also auf den gewöhnlichen
    /// Rechnern, die dieses Netz gerade einladen will.
    ///
    /// **Zweitens eine Abweichung.** `VPMOVDW` **schneidet ab**, die
    /// Referenz `rotate_half_split_i16` benutzt `clamp_i16`, also
    /// **Sättigung**. Sobald ein Zwischenwert den i16-Bereich verließ,
    /// rechneten Skalarpfad und SIMD-Pfad verschieden, und die
    /// Bitgleichheit ist die Zusage, auf der das ganze Protokoll steht.
    ///
    /// **Warum es nie auffiel:** Der CI-Runner hat AVX-512, dort läuft
    /// der Befehl; die Entwicklungsmaschine ist aarch64, dort läuft der
    /// Pfad gar nicht. Gefunden hat es der MSRV-Job bei seinem **ersten
    /// Lauf**, weil `_mm256_cvtepi32_epi16` erst seit Rust 1.89 stabil
    /// ist und die Mindestfassung seit demselben Tag angegeben wird.
    ///
    /// # Was stattdessen passiert
    ///
    /// `_mm256_packs_epi32(r, r)` sättigt paarweise je 128-Bit-Spur, was
    /// die Reihenfolge durcheinanderbringt; die beiden Spuren werden
    /// deshalb einzeln entnommen und ihre unteren 64 Bit
    /// zusammengesetzt. Alle drei Befehle sind AVX2 beziehungsweise
    /// SSE2 und lange stabil.
    #[target_feature(enable = "avx2")]
    unsafe fn saettige_i32_zu_i16(r: __m256i) -> __m128i {
        let gepackt = _mm256_packs_epi32(r, r);
        let untere = _mm256_castsi256_si128(gepackt);
        let obere = _mm256_extracti128_si256::<1>(gepackt);
        _mm_unpacklo_epi64(untere, obere)
    }

    /// AVX2 RoPE: rotate_half_split mit SIMD-Intrinsics.
    /// Verarbeitet 8 Paare (i32) parallel pro AVX2-Register.
    ///
    /// Sicherheit: Caller muss sicherstellen, dass AVX2 verfuegbar ist.
    #[target_feature(enable = "avx2")]
    pub unsafe fn rotate_half_split_avx2(
        vec: &[i16],
        cos_row: &[i16],
        sin_row: &[i16],
        frac_bits: u8,
    ) -> Vec<i16> {
        let n = vec.len();
        let half = n / 2;
        let mut out = vec![0i16; n];

        let chunks = half / 8;
        for chunk in 0..chunks {
            let base = chunk * 8;
            // 8x i16 aus erster Haelfte laden und zu i32 erweitern
            let x0_16 = _mm_loadu_si128(vec.as_ptr().add(base) as *const __m128i);
            let x0 = _mm256_cvtepi16_epi32(x0_16);
            // 8x i16 aus zweiter Haelfte laden und zu i32 erweitern
            let x1_16 = _mm_loadu_si128(vec.as_ptr().add(base + half) as *const __m128i);
            let x1 = _mm256_cvtepi16_epi32(x1_16);
            // cos/sin als i32 laden
            let cos_16 = _mm_loadu_si128(cos_row.as_ptr().add(base) as *const __m128i);
            let cos_v = _mm256_cvtepi16_epi32(cos_16);
            let sin_16 = _mm_loadu_si128(sin_row.as_ptr().add(base) as *const __m128i);
            let sin_v = _mm256_cvtepi16_epi32(sin_16);

            // out[base..base+8] = rshift_round(x0*cos - x1*sin, frac_bits)
            let prod0 = _mm256_mullo_epi32(x0, cos_v);
            let prod1 = _mm256_mullo_epi32(x1, sin_v);
            let sub = _mm256_sub_epi32(prod0, prod1);
            let r0 = rshift_round_avx2(sub, frac_bits);

            // out[base+half..base+half+8] = rshift_round(x1*cos + x0*sin, frac_bits)
            let prod2 = _mm256_mullo_epi32(x1, cos_v);
            let prod3 = _mm256_mullo_epi32(x0, sin_v);
            let add = _mm256_add_epi32(prod2, prod3);
            let r1 = rshift_round_avx2(add, frac_bits);

            // i32 -> i16 mit Saettigung und zurueck speichern
            let packed0 = saettige_i32_zu_i16(r0);
            let packed1 = saettige_i32_zu_i16(r1);
            _mm_storeu_si128(out.as_mut_ptr().add(base) as *mut __m128i, packed0);
            _mm_storeu_si128(out.as_mut_ptr().add(base + half) as *mut __m128i, packed1);
        }

        // Rest skalar
        let remainder_start = chunks * 8;
        for j in remainder_start..half {
            let x0 = vec[j] as i32;
            let x1 = vec[j + half] as i32;
            let c = cos_row[j] as i32;
            let s = sin_row[j] as i32;
            out[j] = clamp_i16(rshift_round(x0 * c - x1 * s, frac_bits));
            out[j + half] = clamp_i16(rshift_round(x1 * c + x0 * s, frac_bits));
        }

        out
    }

    /// RNE-Rechtsshift fuer 8x i32 in einem AVX2-Register.
    #[target_feature(enable = "avx2")]
    unsafe fn rshift_round_avx2(v: __m256i, shift: u8) -> __m256i {
        use core::arch::x86_64::*;
        if shift == 0 {
            return v;
        }
        // quotient = v >> shift (arithmetisch). `_mm256_sra_epi32` nimmt
        // die Schiebeweite als __m128i, dessen niedrige 64 Bit zaehlen.
        // quotient = v >> shift (arithmetisch)
        let quotient = _mm256_sra_epi32(v, _mm_set_epi32(0, 0, 0, shift as i32));
        // mask = (1 << shift) - 1
        let mask = _mm256_set1_epi32((1i32 << shift) - 1);
        // half = 1 << (shift - 1)
        let half = _mm256_set1_epi32(1i32 << (shift - 1));
        // remainder = v & mask
        let remainder = _mm256_and_si256(v, mask);
        // remainder > half → round up
        let gt_half = _mm256_cmpgt_epi32(remainder, half);
        // remainder == half → round to even (quotient & 1)
        let eq_half = _mm256_cmpeq_epi32(remainder, half);
        let q_odd = _mm256_and_si256(quotient, _mm256_set1_epi32(1));
        let tie_break = _mm256_and_si256(eq_half, q_odd);
        // combined: round up if gt_half OR tie_break
        let round_up = _mm256_or_si256(gt_half, tie_break);
        // Apply: quotient + 1 where round_up, but handle negative values
        // For negative numbers with round_up, we need quotient + 1 (toward zero = toward +inf for negative)
        // Actually, RNE for negative: the direction is toward +inf when remainder > half
        // Since we use arithmetic shift (which rounds toward -inf), adding 1 corrects it.
        let one = _mm256_set1_epi32(1);
        let correction = _mm256_and_si256(round_up, one);
        _mm256_add_epi32(quotient, correction)
    }

    /// AVX2 SiLU-Fusionsloop: rescale(gate) → LUT-Lookup → Produkt mit up → rescale.
    /// Der LUT-Lookup selbst ist skalar (Gather ist in AVX2 langsam),
    /// aber die Rescale- und Multiplikationsarithmetik ist vektorisiert.
    ///
    /// **NICHT ANGEBUNDEN (Fund A19).** `Backend::mlp` ruft den skalaren
    /// `mlp_int` auf; dieser Kernel wird nirgends verwendet. Der
    /// Modulkopf fuehrte ihn bis v0.12.41 als „vektorisiert", was fuer
    /// den Kernel stimmte, nicht fuer den Aufrufpfad — die
    /// Paritaetstests waren dadurch trotzdem gruen, weil die Delegation
    /// an die Referenz per Konstruktion bit-identisch ist.
    ///
    /// Bewusst **nicht** in diesem Audit angebunden: Das Anbinden
    /// braucht einen Paritaetslauf auf echter x86_64-Hardware. Ein
    /// SIMD-Pfad wird nur dort geprueft, wo er auch laeuft, nie in der
    /// CI: Uebersetzbar heisst nicht ausfuehrbar, und unverifizierte
    /// Numerik gehoert nicht in einen Konsenspfad.
    ///
    /// Sicherheit: Caller muss sicherstellen, dass AVX2 verfuegbar ist.
    #[allow(dead_code)]
    #[target_feature(enable = "avx2")]
    pub unsafe fn mlp_silu_fusion_avx2(
        gate: &[i16],
        up: &[i16],
        silu_lut: &[i16],
        gate_out_frac: u8,
        up_out_frac: u8,
        down_in_frac: u8,
        silu_in_frac: u8,
        silu_lut_offset: i16,
        silu_out_frac: u8,
    ) -> Vec<i16> {
        let n = gate.len();
        let mut h = Vec::with_capacity(n);

        // Skalarer Pfad mit LUT-Lookup (Gather ist in AVX2 nicht effizient
        // fuer variable Indizes). Die Rescale-Operationen nutzen die
        // festen Shifts und koennten vektorisiert werden, aber der
        // LUT-Lookup dominiert die Komplexitaet.
        for i in 0..n {
            let g_dom = rescale(gate[i] as i32, gate_out_frac, silu_in_frac);
            let lut_idx = (g_dom as i16) + silu_lut_offset;
            let activated = if (lut_idx as usize) < silu_lut.len() {
                silu_lut[lut_idx as usize] as i64
            } else if lut_idx < 0 {
                silu_lut[0] as i64
            } else {
                silu_lut[silu_lut.len() - 1] as i64
            };
            let prod = activated * (up[i] as i64);
            h.push(clamp_i16_from_i64(rescale_i64(prod, silu_out_frac + up_out_frac, down_in_frac)));
        }
        h
    }
}

// =====================================================================
// NEON-Helfer (ARM64 / Apple Silicon)
// =====================================================================

#[cfg(target_arch = "aarch64")]
mod neon {
    use core::arch::aarch64::*;
    use crate::fixed_point::{clamp_i16, rshift_round};

    /// NEON Softmax: exp-LUT-basiert, numerisch stabil.
    /// Vektorisiert: Max-Reduktion (4x i32 parallel pro NEON-Register).
    pub unsafe fn softmax_neon(
        logits: &[i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    ) -> Vec<i32> {
        let n = logits.len();
        if n == 0 {
            return vec![];
        }

        // 1. Max-Reduktion (NEON: 4x i32 parallel)
        let mut max_val = i32::MIN;
        let chunks4 = n / 4;
        for i in 0..chunks4 {
            let v = vld1q_s32(logits.as_ptr().add(i * 4));
            let m = vmaxvq_s32(v);
            if m > max_val {
                max_val = m;
            }
        }
        for i in (chunks4 * 4)..n {
            if logits[i] > max_val {
                max_val = logits[i];
            }
        }

        // 2. exp-LUT-Lookup + Summation
        let one = 1i32 << frac_bits;
        let mut exps = Vec::with_capacity(n);
        let mut sum: i64 = 0;

        for &z in logits.iter() {
            let diff = max_val - z;
            let exp_val = if diff <= 0 {
                one
            } else {
                let idx = (diff as u64 >> lut_shift) as usize;
                if idx >= exp_lut.len() {
                    0i32
                } else {
                    exp_lut[idx] as i32
                }
            };
            exps.push(exp_val);
            sum += exp_val as i64;
        }

        // 3. Normalisierung mit RNE-Rundung
        if sum == 0 {
            let base = one / n as i32;
            let rem = one - base * n as i32;
            return (0..n).map(|i| base + if (i as i32) < rem { 1 } else { 0 }).collect();
        }

        let mut probs = Vec::with_capacity(n);
        for &e in &exps {
            let num = (e as i64) * (one as i64);
            let q = num / sum;
            let r = (num % sum).abs();
            let d = sum.abs();
            let rounded = if r * 2 > d || (r * 2 == d && (q & 1) != 0) {
                if (num > 0) == (sum > 0) { q + 1 } else { q - 1 }
            } else {
                q
            };
            probs.push(rounded as i32);
        }
        probs
    }

    /// NEON RoPE: rotate_half_split.
    /// Verarbeitet 4 Paare (i32) parallel pro NEON-Register.
    pub unsafe fn rotate_half_split_neon(
        vec: &[i16],
        cos_row: &[i16],
        sin_row: &[i16],
        frac_bits: u8,
    ) -> Vec<i16> {
        let n = vec.len();
        let half = n / 2;
        let mut out = vec![0i16; n];

        let chunks = half / 4;
        for chunk in 0..chunks {
            let base = chunk * 4;
            // 4x i16 aus erster Haelfte laden und zu i32 erweitern
            let x0_16 = vld1_s16(vec.as_ptr().add(base));
            let x0 = vmovl_s16(x0_16);
            // 4x i16 aus zweiter Haelfte
            let x1_16 = vld1_s16(vec.as_ptr().add(base + half));
            let x1 = vmovl_s16(x1_16);
            // cos/sin
            let cos_16 = vld1_s16(cos_row.as_ptr().add(base));
            let cos_v = vmovl_s16(cos_16);
            let sin_16 = vld1_s16(sin_row.as_ptr().add(base));
            let sin_v = vmovl_s16(sin_16);

            // out[base..] = rshift_round(x0*cos - x1*sin, frac_bits)
            let prod0 = vmulq_s32(x0, cos_v);
            let prod1 = vmulq_s32(x1, sin_v);
            let sub = vsubq_s32(prod0, prod1);
            let r0 = rshift_round_neon(sub, frac_bits);

            // out[base+half..] = rshift_round(x1*cos + x0*sin, frac_bits)
            let prod2 = vmulq_s32(x1, cos_v);
            let prod3 = vmulq_s32(x0, sin_v);
            let add = vaddq_s32(prod2, prod3);
            let r1 = rshift_round_neon(add, frac_bits);

            // i32 -> i16 mit Saettigung
            let packed0 = vqmovn_s32(r0);
            let packed1 = vqmovn_s32(r1);
            vst1_s16(out.as_mut_ptr().add(base), packed0);
            vst1_s16(out.as_mut_ptr().add(base + half), packed1);
        }

        // Rest skalar
        let remainder_start = chunks * 4;
        for j in remainder_start..half {
            let x0 = vec[j] as i32;
            let x1 = vec[j + half] as i32;
            let c = cos_row[j] as i32;
            let s = sin_row[j] as i32;
            out[j] = clamp_i16(rshift_round(x0 * c - x1 * s, frac_bits));
            out[j + half] = clamp_i16(rshift_round(x1 * c + x0 * s, frac_bits));
        }

        out
    }

    /// RNE-Rechtsshift fuer 4x i32 in einem NEON-Register.
    unsafe fn rshift_round_neon(v: int32x4_t, shift: u8) -> int32x4_t {
        use core::arch::aarch64::*;
        if shift == 0 {
            return v;
        }
        // quotient = v >> shift (arithmetisch)
        let shift_v = vdupq_n_s32(shift as i32);
        let quotient = vshlq_s32(v, vnegq_s32(shift_v));
        // mask = (1 << shift) - 1
        let mask = vdupq_n_s32((1i32 << shift) - 1);
        // half = 1 << (shift - 1)
        let half = vdupq_n_s32(1i32 << (shift - 1));
        // remainder = v & mask
        let remainder = vandq_s32(v, mask);
        // remainder > half
        let gt_half = vreinterpretq_s32_u32(vcgtq_s32(remainder, half));
        // remainder == half
        let eq_half = vreinterpretq_s32_u32(vceqq_s32(remainder, half));
        // quotient & 1 (odd check)
        let one = vdupq_n_s32(1);
        let q_odd = vandq_s32(quotient, one);
        // tie_break = eq_half AND q_odd
        let tie_break = vandq_s32(eq_half, q_odd);
        // round_up = gt_half OR tie_break
        let round_up = vorrq_s32(gt_half, tie_break);
        // correction = round_up & 1
        let correction = vandq_s32(round_up, one);
        vaddq_s32(quotient, correction)
    }
}

// =====================================================================
// Backend-Implementierung
// =====================================================================

impl Backend for SimdBackend {
    fn name(&self) -> &'static str {
        match self.target {
            SimdTarget::Avx2 => "simd-avx2",
            SimdTarget::Avx512 => "simd-avx512",
            SimdTarget::Neon => "simd-neon",
        }
    }

    fn hardware_family(&self) -> &'static str {
        "cpu-simd"
    }

    fn feature_tag(&self) -> &'static str {
        match self.target {
            SimdTarget::Avx2 => "cpu-simd-avx2",
            SimdTarget::Avx512 => "cpu-simd-avx512",
            SimdTarget::Neon => "cpu-simd-neon",
        }
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
        // Delegiert an Referenz-Kernel (Zukuenftig: AVX2 dot-product).
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
        // Delegiert an Referenz-Kernel (Zukuenftig: AVX2 sum-of-squares).
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
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            let result = unsafe { avx2::softmax_avx2(logits, exp_lut, lut_shift, frac_bits) };
            out.copy_from_slice(&result);
            return;
        }
        #[cfg(target_arch = "aarch64")]
        {
            let result = unsafe { neon::softmax_neon(logits, exp_lut, lut_shift, frac_bits) };
            out.copy_from_slice(&result);
        }
        // Fallback auf Referenz. Auf aarch64 nicht erreichbar (NEON
        // behandelt jeden Fall), deshalb dort auch nicht kompiliert.
        #[cfg(not(target_arch = "aarch64"))]
        {
            let result = softmax_int(logits, exp_lut, lut_shift, frac_bits);
            out.copy_from_slice(&result);
        }
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
        // Attention nutzt softmax_avx2 intern ueber softmax_int-Aufrufe.
        // Der Q*K^T dot-product und die V-Gewichtung delegieren an Referenz.
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
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            for (seq_idx, &pos) in positions.iter().enumerate() {
                if pos >= cos_lut.len() { continue; }
                let cos_row = &cos_lut[pos * (q[seq_idx].len() / 2)..];
                let sin_row = &sin_lut[pos * (q[seq_idx].len() / 2)..];
                let q_rot = unsafe { avx2::rotate_half_split_avx2(&q[seq_idx], cos_row, sin_row, frac_bits) };
                q[seq_idx].copy_from_slice(&q_rot);
            }
            for (seq_idx, &pos) in positions.iter().enumerate() {
                if seq_idx >= k.len() { break; }
                if pos >= cos_lut.len() { continue; }
                let head_dim = k[seq_idx].len();
                let half = head_dim / 2;
                let cos_row = &cos_lut[pos * half..pos * half + half];
                let sin_row = &sin_lut[pos * half..pos * half + half];
                let k_rot = unsafe { avx2::rotate_half_split_avx2(&k[seq_idx], cos_row, sin_row, frac_bits) };
                k[seq_idx].copy_from_slice(&k_rot);
            }
            return;
        }
        #[cfg(target_arch = "aarch64")]
        {
            for (seq_idx, &pos) in positions.iter().enumerate() {
                if pos >= cos_lut.len() { continue; }
                let cos_row = &cos_lut[pos * (q[seq_idx].len() / 2)..];
                let sin_row = &sin_lut[pos * (q[seq_idx].len() / 2)..];
                let q_rot = unsafe { neon::rotate_half_split_neon(&q[seq_idx], cos_row, sin_row, frac_bits) };
                q[seq_idx].copy_from_slice(&q_rot);
            }
            for (seq_idx, &pos) in positions.iter().enumerate() {
                if seq_idx >= k.len() { break; }
                if pos >= cos_lut.len() { continue; }
                let head_dim = k[seq_idx].len();
                let half = head_dim / 2;
                let cos_row = &cos_lut[pos * half..pos * half + half];
                let sin_row = &sin_lut[pos * half..pos * half + half];
                let k_rot = unsafe { neon::rotate_half_split_neon(&k[seq_idx], cos_row, sin_row, frac_bits) };
                k[seq_idx].copy_from_slice(&k_rot);
            }
        }
        // Fallback auf Referenz. Auf aarch64 nicht erreichbar (NEON
        // behandelt jeden Fall), deshalb dort auch nicht kompiliert.
        #[cfg(not(target_arch = "aarch64"))]
        {
            let (q_out, k_out) = apply_rope_i16(q, k, cos_lut, sin_lut, positions, frac_bits);
            for (i, row) in q_out.iter().enumerate() {
                q[i].copy_from_slice(row);
            }
            for (i, row) in k_out.iter().enumerate() {
                k[i].copy_from_slice(row);
            }
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

#[cfg(all(test, target_arch = "x86_64"))]
mod avx2_gleichheit {
    use crate::rope::rotate_half_split_i16;

    /// ⚑ **Fund 103: Der AVX2-Pfad schnitt ab, wo die Referenz sättigt.**
    ///
    /// An dieser Stelle stand `_mm256_cvtepi32_epi16` (`VPMOVDW`), und
    /// der **schneidet ab**; `rotate_half_split_i16` benutzt dagegen
    /// `clamp_i16`, also Sättigung. Sobald ein Zwischenwert den
    /// i16-Bereich verließ, rechneten beide Pfade verschieden, und die
    /// Bitgleichheit ist die Zusage, auf der das ganze Protokoll steht.
    ///
    /// **Der Test wählt die Werte so, dass es überläuft.** Mit kleinen
    /// Zahlen wäre er wertlos: Dann stimmen Abschneiden und Sättigen
    /// überein, und genau deshalb ist es nie aufgefallen.
    ///
    /// Ohne AVX2 (etwa unter einer Emulation) wird übersprungen, und
    /// zwar laut: Ein stiller Übersprung sähe aus wie ein bestandener
    /// Test.
    #[test]
    fn avx2_rope_rechnet_wie_die_referenz_auch_im_ueberlauf() {
        if !std::is_x86_feature_detected!("avx2") {
            eprintln!("[uebersprungen] kein AVX2 auf dieser Maschine");
            return;
        }
        // 16 Werte, also genau ein AVX2-Block je Hälfte plus nichts.
        // Große Beträge, damit x0*cos - x1*sin den i16-Bereich sprengt.
        let vec: Vec<i16> = (0..16).map(|i| if i % 2 == 0 { 32000 } else { -32000 }).collect();
        let cos_row: Vec<i16> = vec![30000; 8];
        let sin_row: Vec<i16> = vec![-30000; 8];
        let frac_bits = 8u8;

        let erwartet = rotate_half_split_i16(&vec, &cos_row, &sin_row, frac_bits);
        let gemessen =
            unsafe { super::avx2::rotate_half_split_avx2(&vec, &cos_row, &sin_row, frac_bits) };

        // Gegenprobe zur Gegenprobe: Der Fall muss wirklich überlaufen,
        // sonst prüft der Test nur den harmlosen Bereich.
        assert!(
            erwartet.iter().any(|&v| v == i16::MAX || v == i16::MIN),
            "die Werte laufen nicht über: dann sagt dieser Test nichts"
        );
        assert_eq!(gemessen, erwartet, "AVX2 weicht von der Referenz ab");
    }
}
