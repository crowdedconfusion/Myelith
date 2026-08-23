//! Lineare Schichten: W8A16 (Gewichte int8, Aktivierungen int16) und Bias.
//!
//! Seit dem Numerik-Realitaetsabgleich (v0.12.20) sind Aktivierungen int16
//! mit kalibrierten Per-Layer-Zweierpotenz-Skalen: reale RMSNorm-/MLP-
//! Ausgaben (gemessen bis ~±1640) sprengen den int8-Bereich. Gewichte
//! bleiben int8. Akkumulation in i64, da 896 Kanaele * 127 * 32767 den
//! i32-Bereich ueberschreiten koennen.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::dot::dot_i8_i16;
use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};

/// Ab wie vielen Multiplikationen (`zeilen · in_features`) sich das
/// Aufteilen über Threads überhaupt lohnt.
///
/// **Gemessen, nicht geraten** (`src/bin/threads_probe.rs`). Der Start
/// eines `thread::scope` kostet rund `12 µs + 6,3 µs je Thread`, also 25
/// µs bei zwei und 107 µs bei fünfzehn. Unterhalb dieser Schwelle frisst
/// der Start den Gewinn: Die 896×896-Matrizen von 0,5B brauchen
/// einkernig 54 µs, und selbst die beste Aufteilung sparte davon nur 12.
const PARALLEL_AB: usize = 1_500_000;

/// Arbeit je Thread, in Multiplikationen.
///
/// **Warum die Threadzahl an der Arbeit hängt und nicht an der
/// Kernzahl.** Der erste Versuch nahm einfach `available_parallelism`,
/// auf der Messmaschine also 15, und brachte bei 0,5B **nichts**: Die
/// 4864×896-Matrix braucht einkernig 289 µs, und 15 Threads kosten
/// allein 107 µs Start. Gemessen an derselben Matrix: vier Threads
/// **2,53×**, acht Threads 2,41×, fünfzehn Threads nur noch 1,72×.
///
/// Bei der größten Matrix des 7B-Modells (18944×3584) ist es umgekehrt:
/// dort bringen 15 Threads **7,40×** gegenüber 2,83× bei vier. Eine
/// feste Zahl ist also für eine der beiden Größen falsch.
const ARBEIT_JE_THREAD: usize = 1_000_000;

/// Obergrenze der Threadzahl, einmal ermittelt statt je Aufruf.
fn max_threads() -> usize {
    use std::sync::OnceLock;
    static N: OnceLock<usize> = OnceLock::new();
    *N.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

/// Rechnet `zeilen` unabhängige Ausgabewerte, einkernig oder verteilt.
///
/// **Bitgleich per Konstruktion, und das ist keine Redewendung.** Jede
/// Ausgabezeile ist ein eigenes Skalarprodukt über ihre eigene
/// Gewichtszeile und schreibt in ihr eigenes Feld. Zwischen den Zeilen
/// gibt es keine gemeinsame Zwischensumme, also auch keine Reihenfolge,
/// die etwas ändern könnte. Threadzahl, Aufteilung und Schwelle sind
/// damit reine Laufzeitentscheidungen.
///
/// Das ist dieselbe Eigenschaft, aus der das ganze Projekt seine
/// Bitgleichheit zieht, nur eine Ebene höher: Dort ist es die
/// Assoziativität der Ganzzahladdition **innerhalb** einer Zeile, hier
/// die Unabhängigkeit **zwischen** den Zeilen.
fn zeilen_rechnen<F>(zeilen: usize, arbeit_je_zeile: usize, f: F) -> Vec<i16>
where
    F: Fn(usize) -> i16 + Sync,
{
    let arbeit = zeilen.saturating_mul(arbeit_je_zeile);
    let n = (arbeit / ARBEIT_JE_THREAD).clamp(2, max_threads());
    if arbeit < PARALLEL_AB || max_threads() < 2 || zeilen < 2 {
        return (0..zeilen).map(&f).collect();
    }

    let mut out = vec![0i16; zeilen];
    let je = zeilen.div_ceil(n);
    std::thread::scope(|s| {
        for (t, teil) in out.chunks_mut(je).enumerate() {
            let f = &f;
            s.spawn(move || {
                let start = t * je;
                for (i, ziel) in teil.iter_mut().enumerate() {
                    *ziel = f(start + i);
                }
            });
        }
    });
    out
}

/// W8A16 Matrix-Vektor-Multiplikation.
///
/// `x` (Aktivierung, int16, Skala `act_frac_bits`), `W` (Gewicht, int8,
/// Per-Channel-Skala je Ausgabe-Zeile `w_shifts[r]`, theta_v 0.7.0);
/// Ausgabe int16 auf `out_frac_bits`.
/// **`W` liegt flach**, Zeile für Zeile hintereinander, `in_features`
/// Elemente je Zeile.
///
/// Bis v0.13.4 nahm dieser Kernel `&[Vec<i8>]`. Die Gewichte liegen im
/// Artefakt und im `QTensor` aber flach; `model.rs` baute deshalb vor
/// **jedem** Aufruf ein `Vec<Vec<i8>>` daraus, mit einer Heap-Allokation
/// und einer Kopie je Ausgabe-Zeile. Bei Qwen2.5-0,5B waren das
/// **358 MB und 304 128 Allokationen je Token**, denn die Umwandlung lief
/// achtmal je Ebene und die Ebenen 24-mal je Token.
///
/// **Die Numerik ändert sich dadurch nicht.** `dot_i8_i16` bekommt
/// dieselben Bytes in derselben Reihenfolge; die Zeile ist jetzt ein
/// Ausschnitt statt einer Kopie. Bitgleichheit gilt hier per Konstruktion,
/// nicht nur laut Messung.
pub fn linear_w8a16(
    x: &[i16],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<i16> {
    assert_eq!(
        W.len(),
        in_features * w_shifts.len(),
        "linear_w8a16: {} Gewichte passen nicht zu {} Zeilen à {} Elementen",
        W.len(),
        w_shifts.len(),
        in_features
    );
    // Vektorisiert, wenn `cpu-simd` aktiv ist, und über Threads verteilt,
    // wenn die Matrix groß genug ist. Beides bitgleich zur einfachsten
    // Fassung: innerhalb der Zeile, weil die i64-Akkumulation exakt und
    // damit assoziativ ist (`dot.rs`), zwischen den Zeilen, weil sie
    // voneinander unabhängig sind (`zeilen_rechnen`).
    zeilen_rechnen(w_shifts.len(), in_features, |z| {
        let row = &W[z * in_features..(z + 1) * in_features];
        let acc = dot_i8_i16(row, x);
        let y = rescale_i64(acc, w_shifts[z] + act_frac_bits, out_frac_bits);
        clamp_i16_from_i64(y)
    })
}

/// W8A16 mit Per-Kanal-Ausgangsskala (Fund 20, theta_v 0.11.0).
///
/// Wie `linear_w8a16`, aber `out_frac_bits` ist ein Shift JE
/// AUSGABE-KANAL statt ein einziger fuer den ganzen Vektor. Wird fuer
/// `o_proj` und `down_proj` gebraucht: ihre Ausgabe wird direkt in den
/// Residualstrom addiert, und der trägt seit Fund 20 eine Skala je Kanal
/// (Massive Activations bei Qwen2.5-7B — siehe `rmsnorm.rs`-Modulkopf).
/// `q_proj`/`k_proj`/`v_proj`/`gate_proj`/`up_proj` bleiben bei der
/// Skalar-Funktion, weil ihre Ausgaben NICHT in den Residualstrom
/// zurückfliessen und keine vergleichbaren Ausreisser zeigen.
///
/// Bei identischem Wert in jedem Element von `out_frac_bits` ist das
/// Ergebnis bitgleich zu `linear_w8a16` mit demselben Skalar (siehe
/// `test_linear_w8a16_pc_uniform_matches_scalar`).
/// `W` liegt flach wie bei [`linear_w8a16`], Begründung dort.
pub fn linear_w8a16_pc(
    x: &[i16],
    W: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    assert_eq!(
        W.len(),
        in_features * w_shifts.len(),
        "linear_w8a16_pc: {} Gewichte passen nicht zu {} Zeilen à {} Elementen",
        W.len(),
        w_shifts.len(),
        in_features
    );
    assert_eq!(
        w_shifts.len(),
        out_frac_bits.len(),
        "linear_w8a16_pc: eine Ausgangsskala je Kanal (Fund 20)"
    );
    zeilen_rechnen(w_shifts.len(), in_features, |z| {
        let row = &W[z * in_features..(z + 1) * in_features];
        let acc = dot_i8_i16(row, x);
        let y = rescale_i64(acc, w_shifts[z] + act_frac_bits, out_frac_bits[z]);
        clamp_i16_from_i64(y)
    })
}

/// Addiert einen quantisierten Bias auf eine int16-Aktivierungsausgabe.
///
/// Der Bias liegt als **int16** mit Per-Element-Skalen vor
/// (`bias_shifts[i]`). Bis theta_v 0.12.0 war es int8 — das saettigte
/// still bei Betraegen ueber 127 und verfaelschte bei Qwen2.5-7B die
/// Attention ab Ebene 0 (Fund 23, k_proj.bias erreicht 414)
/// und wird elementweise mit `rescale` auf die Ziel-Skala
/// (`out_frac_bits`) gebracht — arithmetischer Rechtsshift mit
/// Round-to-nearest-even, danach i64-Addition mit Clamping auf i16. Reine
/// Ganzzahlarithmetik, deterministisch über alle Backends (Whitepaper
/// Kap. 6.2; Qwen2.5 besitzt Biases an q/k/v_proj).
pub fn add_bias_i16(out: &mut [i16], bias: &[i16], bias_shifts: &[u8], out_frac_bits: u8) {
    assert_eq!(
        out.len(),
        bias.len(),
        "add_bias_i16: Ausgabe ({} Elemente) und Bias ({} Elemente) muessen dieselbe Laenge haben",
        out.len(),
        bias.len()
    );
    assert_eq!(bias.len(), bias_shifts.len(), "add_bias_i16: eine Skala je Bias-Element");
    for ((o, b), &b_shift) in out.iter_mut().zip(bias.iter()).zip(bias_shifts.iter()) {
        let bias_rescaled = rescale(*b as i32, b_shift, out_frac_bits);
        *o = clamp_i16_from_i64((*o as i64) + (bias_rescaled as i64));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_w8a16_identity() {
        // x = [1.0, -1.0] bei frac 6 = [64, -64]; Identitaets-Matrix mit
        // Per-Channel-Shift 7 (127 ~ 1.0): Ergebnis ~ [64, -64] bei frac 6.
        let x = vec![64i16, -64];
        let W: Vec<i8> = vec![127, 0, 0, 127];
        let in_features = 2;
        let out = linear_w8a16(&x, &W, in_features, &[7, 7], 6, 6);
        // 127/128 = 0.992 -> 64 * 127 >> 7 = 63 (RNE: 63.5 -> 64? 64*127=8128,
        // >>7 = 63 Rest 64 = half -> quotient 63 ungerade -> +1 = 64).
        assert_eq!(out, vec![64, -64]);
    }

    #[test]
    fn test_linear_w8a16_per_channel_shifts() {
        // Zeile 0 mit Shift 7 (~1.0), Zeile 1 mit Shift 6 (~2.0):
        // dieselben Gewichte, aber Zeile 1 verdoppelt das Ergebnis.
        let x = vec![64i16];
        let W: Vec<i8> = vec![64, 64];
        let in_features = 1;
        let out = linear_w8a16(&x, &W, in_features, &[7, 6], 6, 6);
        // Zeile 0: 64*64 = 4096, rescale(4096, 13, 6) = 4096>>7 = 32
        // Zeile 1: 64*64 = 4096, rescale(4096, 12, 6) = 4096>>6 = 64
        assert_eq!(out, vec![32, 64]);
    }

    #[test]
    fn test_linear_w8a16_large_accumulator() {
        // Akkumulator jenseits von i32: 896 Kanaele, alle w=127, x=32767
        // -> acc = 896 * 127 * 32767 ≈ 3.7e9 > i32::MAX. Muss in i64
        // akkumulieren und korrekt reskalieren.
        let n = 896usize;
        let x = vec![32767i16; n];
        let W: Vec<i8> = vec![127i8; n];
        let in_features = n;
        // in_frac = 5 + 7 = 12, out_frac 3: acc >> 9.
        let out = linear_w8a16(&x, &W, in_features, &[7], 5, 3);
        let expected = 32767;
        assert_eq!(out[0], expected as i16);
    }

    #[test]
    fn test_linear_w8a16_pc_uniform_matches_scalar() {
        // Fund 20: dasselbe out_frac_bits in jedem Kanal muss bitgleich
        // zur Skalar-Funktion sein - Voraussetzung dafuer, dass bestehende
        // Aufrufer (q/k/v/gate/up_proj) unangetastet bleiben duerfen.
        let x = vec![100i16, -50, 25];
        let W: Vec<i8> = vec![64, -32, 16, 10, 20, -30];
        let in_features = 3;
        let w_shifts = vec![6u8, 5];
        let skalar = linear_w8a16(&x, &W, in_features, &w_shifts, 4, 6);
        let per_kanal = linear_w8a16_pc(&x, &W, in_features, &w_shifts, 4, &[6, 6]);
        assert_eq!(skalar, per_kanal);
    }

    #[test]
    fn test_linear_w8a16_pc_different_targets_per_channel() {
        // Zwei Ausgabezeilen mit identischen Rohwerten, aber
        // unterschiedlicher Zielskala je Kanal (das ist der eigentliche
        // Zweck: o_proj/down_proj muessen auf die per-Kanal kalibrierte
        // Residualskala zielen koennen, nicht nur auf eine gemeinsame).
        let x = vec![100i16, 100];
        let W: Vec<i8> = vec![64, 64, 64, 64]; // identische Zeilen
        let in_features = 2;
        let w_shifts = vec![6u8, 6];
        let out = linear_w8a16_pc(&x, &W, in_features, &w_shifts, 0, &[6, 3]);
        // acc = 64*100 + 64*100 = 12800 fuer beide Zeilen.
        // Zeile 0: rescale(12800, 6, 6) = 12800 (kein Shift).
        // Zeile 1: rescale(12800, 6, 3) = 12800 >> 3 = 1600.
        assert_eq!(out[0], 12800);
        assert_eq!(out[1], 1600);
    }

    #[test]
    fn test_add_bias_i16_rescale_left_shift() {
        // bias_shift (2) < out_frac (4): Linksverschiebung, exakt.
        let mut out = vec![10i16, -10];
        add_bias_i16(&mut out, &[1i16, 1], &[2, 2], 4);
        assert_eq!(out, vec![14, -6]);
    }

    #[test]
    fn test_add_bias_i16_rescale_right_shift_rounds_rne() {
        // bias_shift (3) > out_frac (1): Rechtsshift um 2 mit RNE-Rundung.
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[3i16, -3], &[3, 3], 1);
        assert_eq!(out, vec![1, -1]);
    }

    #[test]
    fn test_add_bias_i16_per_element_shifts() {
        // Unterschiedliche Shifts je Element (theta_v 0.7.0):
        // Bias 4 mit Shift 1 (= 2.0), Bias 4 mit Shift 0 (= 4.0).
        let mut out = vec![0i16, 0];
        add_bias_i16(&mut out, &[4i16, 4], &[1, 0], 0);
        assert_eq!(out, vec![2, 4]);
    }

    #[test]
    fn test_add_bias_i16_clamping() {
        let mut out = vec![32766i16, -32767, 0];
        add_bias_i16(&mut out, &[4i16, -4, 0], &[1, 1, 1], 0); // +2 bzw. -2
        assert_eq!(out, vec![32767, -32768, 0]); // Saettigung an beiden Grenzen
    }

    #[test]
    #[should_panic(expected = "dieselbe Laenge")]
    fn test_add_bias_i16_length_mismatch_panics() {
        let mut out = vec![0i16; 3];
        add_bias_i16(&mut out, &[1i16, 1], &[0, 0], 0);
    }
}
