//! SIMD-Backend fuer x86_64 (AVX2/AVX-512) und ARM64 (Neon)
//! 
//! Feature-gate: `cargo build --features cpu-simd`
//! 
//! WARNUNG: Dieses Backend darf NUR aktiviert werden, wenn es die
//! Golden Vectors gegen das Referenz-Backend besteht.

use crate::backend::Backend;

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
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        return Some(SimdBackend { target: SimdTarget::Avx512 });

        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        return Some(SimdBackend { target: SimdTarget::Avx2 });

        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        return Some(SimdBackend { target: SimdTarget::Neon });

        None
    }

    pub fn target(&self) -> SimdTarget {
        self.target
    }

    /// Hilfsfunktion: flat i8-Array -> Vec<Vec<i8>> fuer Referenz-Fallback.
    fn flat_to_vec_vec(flat: &[i8], cols: usize) -> Vec<Vec<i8>> {
        flat.chunks(cols).map(|c| c.to_vec()).collect()
    }
}

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

    fn linear_w8a8(
        &self,
        x: &[i8],
        W: &[i8],
        out: &mut [i8],
        in_features: usize,
        out_features: usize,
        act_frac: u8,
        weight_frac: u8,
        out_frac: u8,
    ) {
        match self.target {
            #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
            SimdTarget::Avx2 => unsafe {
                use std::arch::x86_64::*;

                for row in 0..out_features {
                    let w_row = &W[row * in_features..(row + 1) * in_features];
                    let mut acc: i32 = 0;
                    let mut col = 0;

                    // AVX2: 32 i8-Werte pro Iteration
                    // _mm256_cvtepi8_epi16 -> _mm256_madd_epi16 -> i32-Akkumulation
                    while col + 32 <= in_features {
                        let x_vec = _mm256_loadu_si256(x.as_ptr().add(col) as *const __m256i);
                        let w_vec = _mm256_loadu_si256(w_row.as_ptr().add(col) as *const __m256i);

                        // Untere / obere 128-bit-Lane zu 16 x i16 expandieren
                        let x_lo = _mm256_cvtepi8_epi16(_mm256_castsi256_si128(x_vec));
                        let x_hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(x_vec, 1));
                        let w_lo = _mm256_cvtepi8_epi16(_mm256_castsi256_si128(w_vec));
                        let w_hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(w_vec, 1));

                        // i16 * i16 -> i32 (Paare summiert)
                        let prod_lo = _mm256_madd_epi16(x_lo, w_lo);
                        let prod_hi = _mm256_madd_epi16(x_hi, w_hi);

                        let sum = _mm256_add_epi32(prod_lo, prod_hi);

                        // Horizontal reduce 8 x i32
                        let low  = _mm256_castsi256_si128(sum);
                        let high = _mm256_extracti128_si256(sum, 1);
                        let combined = _mm_add_epi32(low, high);
                        let shuf = _mm_shuffle_epi32(combined, 0x4e); // [1,0,3,2]
                        let sums = _mm_add_epi32(combined, shuf);
                        let shuf2 = _mm_shuffle_epi32(sums, 0xb1);    // [2,3,0,1]
                        let final_sum = _mm_add_epi32(sums, shuf2);
                        acc += _mm_cvtsi128_si32(final_sum);

                        col += 32;
                    }

                    // Skalarer Tail (deterministisch, exakt wie Referenz)
                    for c in col..in_features {
                        acc += (x[c] as i32) * (w_row[c] as i32);
                    }

                    let in_frac = act_frac + weight_frac;
                    let shift = in_frac as i8 - out_frac as i8;
                    let y = if shift >= 0 {
                        crate::fixed_point::rshift_round(acc, shift as u8)
                    } else {
                        acc << (-shift)
                    };
                    out[row] = crate::fixed_point::clamp_i8(y);
                }
            },
            _ => {
                // Fallback: deterministische Referenz-Implementierung
                for row in 0..out_features {
                    let mut acc: i32 = 0;
                    for col in 0..in_features {
                        acc += (W[row * in_features + col] as i32) * (x[col] as i32);
                    }
                    let in_frac = act_frac + weight_frac;
                    let shift = in_frac as i8 - out_frac as i8;
                    let y = if shift >= 0 {
                        crate::fixed_point::rshift_round(acc, shift as u8)
                    } else {
                        acc << (-shift)
                    };
                    out[row] = crate::fixed_point::clamp_i8(y);
                }
            }
        }
    }

    fn rmsnorm(
        &self,
        x: &[i8],
        gamma: &[i8],
        out: &mut [i8],
        frac_bits: u8,
        eps: i32,
    ) {
        match self.target {
            #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
            SimdTarget::Avx2 => unsafe {
                use std::arch::x86_64::*;

                let n = x.len();
                assert_eq!(n, gamma.len());
                assert_eq!(n, out.len());

                // 1. Quadratsumme mit AVX2
                let mut acc: i64 = 0;
                let mut i = 0;
                while i + 32 <= n {
                    let x_vec = _mm256_loadu_si256(x.as_ptr().add(i) as *const __m256i);
                    let x_lo = _mm256_cvtepi8_epi16(_mm256_castsi256_si128(x_vec));
                    let x_hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(x_vec, 1));

                    let sq_lo = _mm256_madd_epi16(x_lo, x_lo);
                    let sq_hi = _mm256_madd_epi16(x_hi, x_hi);

                    let sum = _mm256_add_epi32(sq_lo, sq_hi);

                    // Horizontal reduce 8 x i32
                    let low  = _mm256_castsi256_si128(sum);
                    let high = _mm256_extracti128_si256(sum, 1);
                    let combined = _mm_add_epi32(low, high);
                    let shuf = _mm_shuffle_epi32(combined, 0x4e);
                    let sums = _mm_add_epi32(combined, shuf);
                    let shuf2 = _mm_shuffle_epi32(sums, 0xb1);
                    let final_sum = _mm_add_epi32(sums, shuf2);
                    acc += _mm_cvtsi128_si32(final_sum) as i64;

                    i += 32;
                }

                // Skalarer Tail
                for &v in &x[i..] {
                    let vi = v as i32;
                    acc += (vi * vi) as i64;
                }

                let mean_sq = (acc / n as i64) as i32;
                let rms = crate::integer_math::rsqrt_q(mean_sq + eps, frac_bits);

                if rms == 0 {
                    out.fill(0);
                    return;
                }

                let one = 1i32 << frac_bits;

                // 2. Skalare Ausgabeschleife (exakt wie Referenz)
                for j in 0..n {
                    let v = x[j] as i32;
                    let y = (v << frac_bits) / rms;
                    let g = gamma[j] as i32;
                    let y2 = (y * g) / one;
                    out[j] = crate::fixed_point::clamp_i8(y2);
                }
            },
            _ => {
                // Fallback: deterministische Referenz-Implementierung
                let result = crate::rmsnorm::rmsnorm_int8(x, gamma, frac_bits, eps);
                out.copy_from_slice(&result);
            }
        }
    }

    fn softmax(
        &self,
        logits: &[i32],
        out: &mut [i32],
        exp_lut: &[i16],
        lut_shift: u8,
        frac_bits: u8,
    ) {
        // Konservativer Fallback auf Referenz: softmax ist schwierig
        // bit-exakt zu vektorisieren wegen der Summe und Division.
        // AVX2-Optimierung folgt in separater Aenderung.
        let result = crate::softmax::softmax_int(logits, exp_lut, lut_shift, frac_bits);
        out.copy_from_slice(&result);
    }

    fn attention(
        &self,
        q: &[Vec<i8>],
        k: &[Vec<i8>],
        v: &[Vec<i8>],
        out: &mut [Vec<i8>],
        mask: &[Vec<bool>],
        score_shift: u8,
        exp_lut: &[i16],
        lut_shift: u8,
        prob_frac: u8,
    ) {
        // Konservativer Fallback auf Referenz: Attention ist komplex
        // und erfordert sorgfaeltige Validierung gegen Golden Vectors.
        let result = crate::attention::attention_int(
            q, k, v, mask, score_shift, exp_lut, lut_shift, prob_frac,
        );
        for (i, row) in result.iter().enumerate() {
            out[i].copy_from_slice(row);
        }
    }

    fn rope(
        &self,
        q: &mut [Vec<i8>],
        k: &mut [Vec<i8>],
        cos_lut: &[i16],
        sin_lut: &[i16],
        positions: &[usize],
        frac_bits: u8,
    ) {
        // Konservativer Fallback auf Referenz.
        let (q_out, k_out) = crate::rope::apply_rope(q, k, cos_lut, sin_lut, positions, frac_bits);
        for (i, row) in q_out.iter().enumerate() {
            q[i].copy_from_slice(row);
        }
        for (i, row) in k_out.iter().enumerate() {
            k[i].copy_from_slice(row);
        }
    }

    fn mlp(
        &self,
        x: &[i8],
        W_gate: &[i8],
        W_up: &[i8],
        W_down: &[i8],
        out: &mut [i8],
        silu_lut: &[i16],
        act_frac: u8,
        weight_frac: u8,
        out_frac: u8,
        lut_shift: u8,
        lut_offset: i16,
    ) {
        match self.target {
            #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
            SimdTarget::Avx2 => unsafe {
                let hidden_size = x.len();
                let intermediate_size = W_gate.len() / hidden_size;

                // 1. Gate-Projektion (AVX2-optimiert via self.linear_w8a8)
                let mut gate = vec![0i8; intermediate_size];
                self.linear_w8a8(x, W_gate, &mut gate, hidden_size, intermediate_size, act_frac, weight_frac, act_frac);

                // 2. Up-Projektion (AVX2-optimiert via self.linear_w8a8)
                let mut up = vec![0i8; intermediate_size];
                self.linear_w8a8(x, W_up, &mut up, hidden_size, intermediate_size, act_frac, weight_frac, act_frac);

                // 3. SiLU via LUT + elementweise Mul (skalar, deterministisch)
                let mut h = vec![0i8; intermediate_size];
                for i in 0..intermediate_size {
                    let activated = crate::integer_math::lut_lookup(gate[i] as i16, silu_lut, lut_shift, lut_offset);
                    let prod = (activated as i32) * (up[i] as i32);
                    h[i] = crate::fixed_point::clamp_i8(crate::fixed_point::rshift_round(prod, act_frac));
                }

                // 4. Down-Projektion (AVX2-optimiert via self.linear_w8a8)
                self.linear_w8a8(&h, W_down, out, intermediate_size, hidden_size, act_frac, weight_frac, out_frac);
            },
            _ => {
                // Fallback: deterministische Referenz-Implementierung
                let hidden_size = x.len();
                let intermediate_size = W_gate.len() / hidden_size;

                let gate = crate::linear::linear_w8a8(x, &Self::flat_to_vec_vec(W_gate, hidden_size), act_frac, weight_frac, act_frac);
                let up = crate::linear::linear_w8a8(x, &Self::flat_to_vec_vec(W_up, hidden_size), act_frac, weight_frac, act_frac);

                let mut h = Vec::with_capacity(intermediate_size);
                for (g, u) in gate.iter().zip(up.iter()) {
                    let activated = crate::integer_math::lut_lookup(*g as i16, silu_lut, lut_shift, lut_offset);
                    let prod = (activated as i32) * (*u as i32);
                    h.push(crate::fixed_point::clamp_i8(crate::fixed_point::rshift_round(prod, act_frac)));
                }

                let down = crate::linear::linear_w8a8(&h, &Self::flat_to_vec_vec(W_down, intermediate_size), act_frac, weight_frac, out_frac);
                out.copy_from_slice(&down);
            }
        }
    }
}
