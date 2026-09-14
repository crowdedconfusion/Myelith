//! Integer-Attention (causal, GQA-faehig ueber Head-Aufteilung des Aufrufers)
//!
//! Aktivierungen int16 mit Per-Layer-Skalen (Numerik-Realitaetsabgleich
//! v0.12.20): Skalarprodukte und V-Gewichtung akkumulieren in i64, da
//! int16-Werte den i32-Bereich ueberschreiten koennen.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]
// Schleifenindizes sind Kopf-/Dimensionsnummern ueber parallele Puffer.
#![allow(clippy::needless_range_loop)]

use crate::fixed_point::{clamp_i16_from_i64, rshift_round_i64};
use crate::softmax::softmax_int;

/// Skalierter Dot-Product in i64 (int16-Operanden).
#[inline]
pub fn dot_int(a: &[i16], b: &[i16]) -> i64 {
    let mut acc: i64 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc += (*x as i64) * (*y as i64);
    }
    acc
}

/// Integer-Attention.
///
/// `score_mult` traegt die 1/sqrt(head_dim)-Skalierung in Q15 (Fund 19);
/// `score_shift` bringt das Skalarprodukt (Skala `q_frac + k_frac + 15`) auf die
/// exp-LUT-Domäne (`score_frac_bits`, typisch 8) und wird pro Layer aus den
/// kalibrierten Q/K-Skalen abgeleitet (dynamisch, aber deterministisch).
///
/// WICHTIG (Fund 16): Query- und Key-/Value-Laenge sind getrennt zu
/// behandeln. Im KV-Cache-Betrieb besteht `q` nur aus der aktuellen Position
/// (q.len() == 1), waehrend `k`/`v` alle bisherigen Positionen enthalten
/// (k.len() == seq_len). Die Score-/Value-Schleife muss daher ueber
/// `k.len()` laufen, NICHT ueber `q.len()` — sonst attendiert jede Query nur
/// auf den ersten Key und RoPE/Mehrpositions-Attention sind wirkungslos.
pub fn attention_int<Q: AsRef<[i16]>, KV: AsRef<[i16]>>(
    q: &[Q],
    k: &[KV],
    v: &[KV],
    mask: &[Vec<bool>],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
) -> Vec<Vec<i16>> {
    attention_int_mit_spur(
        q, k, v, mask, score_mult, score_shift, exp_lut, lut_shift, prob_frac_bits, None,
    )
}

/// Dasselbe, aber die Wahrscheinlichkeiten fallen mit ab (TRAINING V).
///
/// # ⚑ Warum ein zweiter Eingang und kein Parameter mehr an `attention_int`
///
/// `attention_int` steht im `Backend`-Merkmal, und zwar in vier
/// Umsetzungen. Ein zusätzliches Argument dort risse alle vier auf, für
/// etwas, das nur der Rückwärtspass braucht. **Hier steht deshalb die
/// eine Umsetzung, und `attention_int` ist ihr Eingang ohne Spur.**
///
/// # ⚑ Warum der Rückwärtspass sie braucht
///
/// `softmax_backward(g, p, frac)` rechnet mit den **Wahrscheinlichkeiten
/// selbst**, denn die Ableitung des Softmax ist
/// `p ⊙ (g − ⟨g, p⟩)`. Ohne sie liesse sich der Gradient nur durch
/// Nachrechnen der Punktprodukte gewinnen, also durch eine zweite
/// Umsetzung derselben Rechnung.
///
/// `spur` bekommt je Abfragezeile eine Zeile Wahrscheinlichkeiten auf
/// `prob_frac_bits`, in derselben Reihenfolge wie die Ausgabe.
#[allow(clippy::too_many_arguments)]
/// ⚑ **Ueber `AsRef<[i16]>` und nicht ueber `Vec<i16>`.** Der
/// KV-Speicher gibt seine Positionen als **Ausschnitte** heraus, damit
/// sie nicht je Kopf und je Token kopiert werden muessen; ein Aufrufer
/// mit eigenen Vektoren passt weiterhin hinein. **Eine Umsetzung, zwei
/// Formen**, statt zweier Umsetzungen.
pub fn attention_int_mit_spur<Q: AsRef<[i16]>, KV: AsRef<[i16]>>(
    q: &[Q],
    k: &[KV],
    v: &[KV],
    mask: &[Vec<bool>],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
    mut spur: Option<&mut Vec<Vec<i32>>>,
) -> Vec<Vec<i16>> {
    let q_len = q.len();
    let kv_len = k.len();
    assert_eq!(kv_len, v.len(), "attention_int: k und v muessen gleich lang sein");
    let head_dim = v[0].as_ref().len();
    let mut out = Vec::with_capacity(q_len);

    for i in 0..q_len {
        let mut scores = Vec::with_capacity(kv_len);
        for j in 0..kv_len {
            if mask[i][j] {
                // Fund 19: 1/sqrt(head_dim) als Q15-Multiplikation statt
                // Rechtsshift — der Shift war nur fuer gerade Zweierpotenzen
                // korrekt (siehe fixed_point::inv_sqrt_q15).
                let s = dot_int(q[i].as_ref(), k[j].as_ref()) * score_mult;
                scores.push(rshift_round_i64(s, score_shift) as i32);
            } else {
                scores.push(i32::MIN);
            }
        }

        let probs = softmax_int(&scores, exp_lut, lut_shift, prob_frac_bits);
        // ⚑ **Nach dem Softmax und vor der Gewichtung**: Das ist der
        // Wert, mit dem `softmax_backward` rechnet.
        if let Some(sp) = spur.as_deref_mut() {
            sp.push(probs.clone());
        }

        let mut row = vec![0i64; head_dim];
        for j in 0..kv_len {
            if probs[j] == 0 { continue; }
            let vj = v[j].as_ref();
            for d in 0..head_dim {
                row[d] += (probs[j] as i64) * (vj[d] as i64);
            }
        }

        let mut out_row = Vec::with_capacity(head_dim);
        for d in 0..head_dim {
            out_row.push(clamp_i16_from_i64(rshift_round_i64(row[d], prob_frac_bits)));
        }
        out.push(out_row);
    }
    out
}

/// **Die Aufmerksamkeit einer einzelnen Abfrage ueber einen
/// zusammenhaengenden KV-Verlauf.** `k` und `v` liegen Position hinter
/// Position, und jede Position ist sichtbar.
///
/// ⚑ **Rechnet exakt dasselbe wie [`attention_int_mit_spur`]** mit einer
/// Abfragezeile und einer Maske aus lauter `true`, samt Spur. Den Fall
/// rechnet der Decode je Token und je Kopf, und die allgemeine Form
/// verlangt dafuer eine Maske und je Position einen eigenen Ausschnitt.
///
/// # ⚑ Warum die Vektorfassung bitgleich ist
///
/// **Punktprodukte:** exakte i64-Summen, nur in anderer Reihenfolge; wie
/// bei [`crate::dot::dot_i8_i16`].
///
/// **Gewichtung:** Die Summe `Σ_j p_j · v_j[d]` laeuft in i32 statt i64,
/// **wenn** jedes `p_j` in `[0, 32767]` liegt und `Σ_j p_j · 32768` in i32
/// passt. Dann liegt jedes Produkt und **jede Zwischensumme** betragsmaessig
/// unter `i32::MAX`, denn die Gewichte sind nicht negativ und `|v| <= 32768`.
/// Das wird **je Aufruf nachgesehen** und nicht aus dem Softmax hergeleitet;
/// sonst rechnet dieselbe i64-Schleife wie die allgemeine Form. Beim
/// Softmax dieses Projekts gilt die Bedingung bis rund 98 000 Positionen
/// (`Σ p <= 2^14 + n/2`).
pub fn aufmerksamkeit_einer_abfrage(
    q: &[i16],
    k: &[i16],
    v: &[i16],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
    prob_frac_bits: u8,
    spur: Option<&mut Vec<Vec<i32>>>,
) -> Vec<i16> {
    let breite = q.len();
    assert!(breite > 0 && k.len() % breite == 0, "aufmerksamkeit_einer_abfrage: k passt nicht zur Breite von q");
    let n = k.len() / breite;
    assert!(n > 0 && v.len() % n == 0, "aufmerksamkeit_einer_abfrage: k und v muessen gleich viele Positionen haben");
    let v_breite = v.len() / n;
    let schnell = schnell_erlaubt();

    let scores: Vec<i32> = punktprodukte(q, k, schnell)
        .into_iter()
        .map(|d| rshift_round_i64(d * score_mult, score_shift) as i32)
        .collect();
    let probs = softmax_int(&scores, exp_lut, lut_shift, prob_frac_bits);
    let zeile = gewichtet(&probs, v, v_breite, schnell);
    if let Some(sp) = spur {
        sp.push(probs);
    }
    zeile.into_iter().map(|s| clamp_i16_from_i64(rshift_round_i64(s, prob_frac_bits))).collect()
}

/// Laeuft auf dieser Uebersetzung und in diesem Augenblick die Vektorfassung?
/// Dieselbe Bedingung wie beim Skalarprodukt der linearen Schichten,
/// einschliesslich des Schalters [`crate::dot::skalar_erzwingen`].
fn schnell_erlaubt() -> bool {
    crate::dot::VEKTORISIERT && !crate::dot::skalar_erzwungen()
}

/// `q · k_j` fuer jede Position `j`.
fn punktprodukte(q: &[i16], k: &[i16], schnell: bool) -> Vec<i64> {
    let chunks = k.chunks_exact(q.len());
    if schnell {
        chunks.map(|kj| vektor::dot_i16(q, kj)).collect()
    } else {
        chunks.map(|kj| dot_int(q, kj)).collect()
    }
}

/// `Σ_j p_j · v_j` je Dimension, in i64; Begruendung der i32-Fassung bei
/// [`aufmerksamkeit_einer_abfrage`].
fn gewichtet(probs: &[i32], v: &[i16], v_breite: usize, schnell: bool) -> Vec<i64> {
    if schnell && i32_reicht(probs) {
        let mut zeile = vec![0i32; v_breite];
        for (&p, vj) in probs.iter().zip(v.chunks_exact(v_breite)) {
            if p != 0 {
                vektor::gewichten_i32(&mut zeile, vj, p as i16);
            }
        }
        return zeile.into_iter().map(i64::from).collect();
    }
    let mut zeile = vec![0i64; v_breite];
    for (&p, vj) in probs.iter().zip(v.chunks_exact(v_breite)) {
        if p == 0 {
            continue;
        }
        for d in 0..v_breite {
            zeile[d] += (p as i64) * (vj[d] as i64);
        }
    }
    zeile
}

/// Passt jede Zwischensumme der Gewichtung in i32? Siehe
/// [`aufmerksamkeit_einer_abfrage`].
fn i32_reicht(probs: &[i32]) -> bool {
    let mut summe: i64 = 0;
    for &p in probs {
        if !(0..=i16::MAX as i32).contains(&p) {
            return false;
        }
        summe += p as i64;
    }
    summe * 32768 <= i32::MAX as i64
}

#[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
mod vektor {
    use std::arch::aarch64::*;

    /// `Σ a[i] · b[i]` exakt in i64, acht Produkte je Durchlauf auf zwei
    /// unabhaengigen i64-Akkumulatoren.
    #[inline]
    pub fn dot_i16(a: &[i16], b: &[i16]) -> i64 {
        let n = a.len().min(b.len());
        let mut i = 0usize;
        // Sicherheit: gelesen wird nur innerhalb von `a[..n]` und `b[..n]`;
        // ein Achterblock beginnt nur, wenn er ganz hineinpasst.
        let mut gesamt = unsafe {
            let mut s0 = vdupq_n_s64(0);
            let mut s1 = vdupq_n_s64(0);
            while i + 8 <= n {
                let x = vld1q_s16(a.as_ptr().add(i));
                let y = vld1q_s16(b.as_ptr().add(i));
                // vmull: volles 32-Bit-Produkt; vpadal: paarweise nach i64.
                s0 = vpadalq_s32(s0, vmull_s16(vget_low_s16(x), vget_low_s16(y)));
                s1 = vpadalq_s32(s1, vmull_high_s16(x, y));
                i += 8;
            }
            vaddvq_s64(s0) + vaddvq_s64(s1)
        };
        while i < n {
            gesamt += a[i] as i64 * b[i] as i64;
            i += 1;
        }
        gesamt
    }

    /// `zeile[d] += p · v[d]` in i32. Nur aufrufen, wenn die Summe nach
    /// `i32_reicht` hineinpasst.
    #[inline]
    pub fn gewichten_i32(zeile: &mut [i32], v: &[i16], p: i16) {
        let n = zeile.len().min(v.len());
        let mut d = 0usize;
        // Sicherheit: geschrieben und gelesen wird nur innerhalb von
        // `zeile[..n]` und `v[..n]`.
        unsafe {
            while d + 8 <= n {
                let x = vld1q_s16(v.as_ptr().add(d));
                let z = zeile.as_mut_ptr().add(d);
                vst1q_s32(z, vmlal_n_s16(vld1q_s32(z), vget_low_s16(x), p));
                vst1q_s32(z.add(4), vmlal_high_n_s16(vld1q_s32(z.add(4)), x, p));
                d += 8;
            }
        }
        while d < n {
            zeile[d] += p as i32 * v[d] as i32;
            d += 1;
        }
    }
}

/// Ohne Vektorfassung wird der Zweig nie genommen ([`schnell_erlaubt`] ist
/// dann konstant falsch); die Funktionen stehen nur, damit er uebersetzt.
#[cfg(not(all(feature = "cpu-simd", target_arch = "aarch64")))]
mod vektor {
    pub fn dot_i16(a: &[i16], b: &[i16]) -> i64 {
        super::dot_int(a, b)
    }

    pub fn gewichten_i32(zeile: &mut [i32], v: &[i16], p: i16) {
        for (z, &x) in zeile.iter_mut().zip(v) {
            *z += p as i32 * x as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_uniform_weights() {
        // Zwei Positionen, identische K/V: unabhaengig von den Scores muss
        // die Ausgabe eine Konvexkombination der V-Werte sein; bei
        // identischen V also exakt diese V-Werte (bis auf Rundung).
        let q = vec![vec![64i16, 0], vec![64, 0]];
        let k = vec![vec![64i16, 0], vec![64, 0]];
        let v = vec![vec![100i16, -50], vec![100, -50]];
        let mask = vec![vec![true, false], vec![true, true]];
        let exp_lut: Vec<i16> = (0..129).map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16).collect();
        let out = attention_int(&q, &k, &v, &mask, 1 << 15, 4 + 15, &exp_lut, 0, 8);
        assert_eq!(out.len(), 2);
        assert!((out[0][0] - 100).abs() <= 1);
        assert!((out[0][1] + 50).abs() <= 1);
        assert!((out[1][0] - 100).abs() <= 1);
    }

    /// Ein kleiner Zufallsgenerator, damit die Proben ohne Abhaengigkeit
    /// auskommen.
    fn zufall(saat: &mut u64) -> u64 {
        *saat ^= *saat << 13;
        *saat ^= *saat >> 7;
        *saat ^= *saat << 17;
        *saat
    }

    /// **Eine Abfrage rechnet wie die allgemeine Form**, samt Spur, fuer
    /// beide Fassungen und ueber Breiten und Laengen, die jeden Rest einer
    /// Achterschleife treffen. Die Werte reichen bis an die Raender von i16.
    #[test]
    fn eine_abfrage_rechnet_wie_die_allgemeine_form() {
        let exp_lut: Vec<i16> = (0..16384).map(|i| ((-(i as f64) / 256.0).exp() * 16384.0).round().min(16383.0) as i16).collect();
        let mut saat = 0x1234_5678_9abc_def1u64;
        for &breite in &[1usize, 7, 8, 9, 64, 128] {
            for &n in &[1usize, 2, 7, 8, 9, 100, 513] {
                for runde in 0..3 {
                    let wert = |s: &mut u64| -> i16 {
                        match zufall(s) % 8 {
                            0 => i16::MIN,
                            1 => i16::MAX,
                            _ => (zufall(s) % (1 << (6 + runde * 4))) as i16 - (1 << (5 + runde * 4)) as i16,
                        }
                    };
                    let q: Vec<i16> = (0..breite).map(|_| wert(&mut saat)).collect();
                    let k: Vec<i16> = (0..breite * n).map(|_| wert(&mut saat)).collect();
                    let v: Vec<i16> = (0..breite * n).map(|_| wert(&mut saat)).collect();
                    let q_seq = [q.as_slice()];
                    let k_seq: Vec<&[i16]> = k.chunks_exact(breite).collect();
                    let v_seq: Vec<&[i16]> = v.chunks_exact(breite).collect();
                    let mask = vec![vec![true; n]];
                    let (mult, shift) = (1 << 15, 20 + runde as u8 * 4);
                    let mut spur_a = Vec::new();
                    let a = attention_int_mit_spur(&q_seq, &k_seq, &v_seq, &mask, mult, shift, &exp_lut, 4, 14, Some(&mut spur_a));
                    let mut spur_b = Vec::new();
                    let b = aufmerksamkeit_einer_abfrage(&q, &k, &v, mult, shift, &exp_lut, 4, 14, Some(&mut spur_b));
                    assert_eq!(a[0], b, "Breite {breite}, {n} Positionen, Runde {runde}");
                    assert_eq!(spur_a, spur_b, "Spur bei Breite {breite}, {n} Positionen");
                    for schnell in [false, true] {
                        let d: Vec<i64> = k.chunks_exact(breite).map(|kj| dot_int(&q, kj)).collect();
                        assert_eq!(punktprodukte(&q, &k, schnell), d, "Punktprodukte, schnell {schnell}");
                        assert_eq!(gewichtet(&spur_a[0], &v, breite, schnell), gewichtet(&spur_a[0], &v, breite, false));
                    }
                }
            }
        }
    }

    /// **Die i32-Gewichtung greift nur, wo sie exakt ist**, und ausserhalb
    /// davon rechnet die i64-Schleife: an der Grenze `Σ p · 32768 = i32::MAX`
    /// (knapp darunter und knapp darueber), mit negativen Gewichten und mit
    /// Gewichten ueber i16.
    #[test]
    fn die_i32_gewichtung_haelt_ihre_grenze() {
        let v = vec![i16::MIN; 4 * 16];
        let p_rand = vec![16383, 16384, 16384, 16384]; // Summe 65535
        assert!(i32_reicht(&p_rand));
        let erwartet: Vec<i64> = vec![65535 * -32768; 16];
        assert_eq!(gewichtet(&p_rand, &v, 16, true), erwartet);
        let p_drueber = vec![16384, 16384, 16384, 16384]; // Summe 65536
        assert!(!i32_reicht(&p_drueber));
        assert_eq!(gewichtet(&p_drueber, &v, 16, true), vec![65536 * -32768; 16]);
        assert!(!i32_reicht(&[-1, 5]), "negative Gewichte nie in i32");
        assert!(!i32_reicht(&[32768]), "Gewichte ueber i16 nie in i32");
        let v2: Vec<i16> = (0..32).map(|i| if i % 2 == 0 { i16::MAX } else { i16::MIN }).collect();
        for p in [vec![-3, 7], vec![32768, 1]] {
            assert_eq!(gewichtet(&p, &v2, 16, true), gewichtet(&p, &v2, 16, false), "{p:?}");
        }
    }

    #[test]
    fn test_dot_int_i64_range() {
        // head_dim 64, alle Werte nahe i16-Max: Summe > i32::MAX.
        let a = vec![30000i16; 64];
        let b = vec![30000i16; 64];
        let d = dot_int(&a, &b);
        assert_eq!(d, 64 * 30000i64 * 30000i64);
    }

    #[test]
    fn test_attention_kv_cache_single_query_attends_all_keys() {
        // Fund 16: Im KV-Cache-Betrieb hat q nur 1 Element (aktuelle
        // Position), k/v aber alle bisherigen Positionen. Die Query muss auf
        // ALLE Keys attendieren, nicht nur auf den ersten. Bei identischen
        // Keys sind die Scores gleich -> uniforme Gewichte -> Ausgabe ist der
        // Durchschnitt der Values. Waere der Bug aktiv (nur erster Key),
        // kaeme v[0] = [100, 0] heraus statt [200, 0].
        let q = vec![vec![64i16, 0]];
        let k = vec![vec![64i16, 0], vec![64, 0], vec![64, 0]];
        let v = vec![vec![100i16, 0], vec![200, 0], vec![300, 0]];
        let mask = vec![vec![true, true, true]];
        let exp_lut: Vec<i16> = (0..129).map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16).collect();
        let out = attention_int(&q, &k, &v, &mask, 1 << 15, 4 + 15, &exp_lut, 0, 8);
        assert_eq!(out.len(), 1);
        // Uniforme Gewichte (1/3, 1/3, 1/3) -> Durchschnitt [200, 0].
        assert!((out[0][0] - 200).abs() <= 2, "out[0][0] = {}", out[0][0]);
        assert!(out[0][1].abs() <= 1);
    }
}
