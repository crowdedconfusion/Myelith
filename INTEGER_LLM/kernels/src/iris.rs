//! **Bausteine von Iris, dem Sichtturm**, die der Decoder nicht kennt.
//!
//! Der Turm der Qwen3.6- und Qwen3.8-Modelle rechnet ueber Bildflicken mit
//! LayerNorm statt RMSNorm, einer Positions-Einbettung, die auf jedes
//! Bildgitter umgerechnet wird, und einer Drehung ueber Zeile und Spalte.
//! Alles andere (lineare Schichten mit Bias, Achtsamkeit, GELU ueber eine
//! Tabelle) nimmt er aus den vorhandenen Kernen.
//!
//! Ganzzahlig wie der Rest: Division nur als arithmetischer Rechtsshift mit
//! Rundung zur naechsten geraden Zahl. Die einzige echte Division steht in
//! [`achsengewichte`] und rechnet eine Konstante je Bildgroesse, wie
//! `inv_n_q20` eine je Breite; sie liegt nicht im Pfad je Flicken.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag wie in
// `rmsnorm.rs`; bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]

use crate::fixed_point::{clamp_i16, rshift_round_i128};

/// Bruchstellen der Interpolationsgewichte je Achse.
pub const ACHSE_Q: u32 = 15;

/// **Bis wohin der Index der rsqrt-Tabelle in der LayerNorm reicht.**
///
/// ⚑ **Nicht bis zum Tabellenende wie in der RMSNorm.** Der Eintrag ist
/// `2^12 / sqrt(i)`; am Ende der Tabelle (`i` bis 32767) ist er nur noch 23
/// bis 45 gross und damit auf 1 bis 2 % genau. Mit kleinem `i` wird dagegen
/// der Index selbst grob. Am genauesten ist es mit `i` zwischen 256 und
/// 1023: Eintrag 128 bis 256, beide Fehler zusammen unter 0,4 %. Gemessen
/// am 2026-10-23 (relativer L2-Fehler gegen f64, 1152 Kanaele): mit der
/// Grenze der RMSNorm 0,7 %, mit dieser 0,08 %; mit Skalen je Kanal 0,05 %.
///
/// In der RMSNorm des Decoders ist derselbe Fehler ein gemeinsamer Faktor
/// je Token und wiegt wenig; sie bleibt, wie sie in θ_v steht.
pub const LN_INDEXGRENZE: i128 = 1023;

/// Kantenlaenge eines Flickens in Pixeln.
pub const FLICKEN: usize = 16;

/// Je so viele Flicken einer Seite werden zu einem Bildtoken zusammengelegt.
pub const ZUSAMMEN: usize = 2;

/// `v` um `shift` nach rechts (gerundet zur naechsten geraden Zahl) oder um
/// `-shift` nach links, in i128.
#[inline]
fn verschieben(v: i128, shift: i32) -> i128 {
    if shift >= 0 {
        rshift_round_i128(v, shift as u32)
    } else {
        v << (-shift) as u32
    }
}

/// **LayerNorm ueber int16-Aktivierungen**, mit Mittelwert, Gamma und Beta.
///
/// ```text
/// y = (x - mean(x)) / sqrt(var(x) + eps) * gamma + beta
/// ```
///
/// ⚑ **Dieselbe Rechnung wie die RMSNorm, auf den zentrierten Werten.**
/// Kehrwert der Wurzel ueber dieselbe Tabelle mit demselben dynamischen
/// geraden Index (spec: `rsqrt`), das Epsilon in Q40 wie bei Fund 419.
///
/// Ablauf, alles ganzzahlig:
/// 1. Alle Kanaele auf die **feinste** Eingangsskala `ref = max(x_shifts)`
///    (Linksshift, verlustfrei; dieselbe Richtung wie seit Fund 24).
/// 2. `mean = round((sum * inv_n_q20) >> 20)`, zentriert `c_i = a_i - mean`.
/// 3. `M = (sum c_i^2 * inv_n_q20) >> 20` plus Epsilon, Tabellennachschlag.
/// 4. `c_i * r * gamma_i` hat die Skala `norm_frac + gamma_shift_i`; Beta
///    wird **auf diese Skala** gebracht und addiert, dann einmal auf
///    `out_frac_bits` gerundet. Eine Rundung statt zweier.
///
/// ⛔️ **Kein Ueberlauf in i64 moeglich, deshalb i128 ab dem Quadrat.**
/// `a_i` erreicht `2^15 · 2^Spanne`; bei einer Spanne von 20 ist das
/// Quadrat 2^70.
///
/// Varianz null: Ausgabe ist Beta, wie bei der Vorlage
/// (`0 / sqrt(eps) = 0`).
pub fn layernorm_i16(
    x: &[i16],
    x_shifts: &[u8],
    gamma: &[i8],
    gamma_shifts: &[u8],
    beta: &[i16],
    beta_shifts: &[u8],
    rsqrt_lut: &[i16],
    lut_input_shift: u8,
    lut_output_frac: u8,
    inv_n_q20: i64,
    eps_q40: i64,
    out_frac_bits: u8,
) -> Vec<i16> {
    let n = x.len();
    assert!(n > 0, "layernorm_i16: leerer Eingang");
    assert_eq!(n, x_shifts.len(), "layernorm_i16: ein Eingangs-Shift je Kanal");
    assert_eq!(n, gamma.len(), "layernorm_i16: x und gamma muessen gleich lang sein");
    assert_eq!(n, gamma_shifts.len(), "layernorm_i16: ein Gamma-Shift je Element");
    assert_eq!(n, beta.len(), "layernorm_i16: x und beta muessen gleich lang sein");
    assert_eq!(n, beta_shifts.len(), "layernorm_i16: ein Beta-Shift je Element");
    assert!(lut_input_shift.is_multiple_of(2), "layernorm_i16: lut_input_shift muss gerade sein");

    let ref_shift = *x_shifts.iter().max().expect("nicht leer");
    let a: Vec<i64> = x
        .iter()
        .zip(x_shifts)
        .map(|(&v, &s)| i64::from(v) << u32::from(ref_shift - s))
        .collect();
    let summe: i128 = a.iter().map(|&v| i128::from(v)).sum();
    let mittel = rshift_round_i128(summe * i128::from(inv_n_q20), 20) as i64;
    let c: Vec<i64> = a.iter().map(|&v| v - mittel).collect();

    let quadrate: i128 = c.iter().map(|&v| i128::from(v) * i128::from(v)).sum();
    // ⚑ **`M` mit seinen 20 Nachkommastellen**, nicht abgerundet: Bei
    // leiser Eingabe ist `M` klein, und ein `>> 20` vorab kostete dort
    // Prozente (gemessen 0,7 % bei Werten um 6 Stufen).
    let mut m20: i128 = quadrate * i128::from(inv_n_q20);
    if eps_q40 != 0 {
        m20 += (i128::from(eps_q40) << (2 * u32::from(ref_shift))) >> 20;
    }
    if m20 == 0 {
        // Kein Abstand vom Mittel und kein Epsilon: nur Beta bleibt.
        return beta
            .iter()
            .zip(beta_shifts)
            .map(|(&b, &bs)| clamp_i16(verschieben(i128::from(b), i32::from(bs) - i32::from(out_frac_bits)) as i32))
            .collect();
    }

    // Dynamischer gerader Index-Shift in den genauen Bereich der Tabelle,
    // bis unter [`LN_INDEXGRENZE`]. `t` zaehlt gegen `m20`; gegen `M` ist
    // es `q = t - 20`, und das darf negativ sein.
    let max_idx = ((rsqrt_lut.len() - 1) as i128).min(LN_INDEXGRENZE);
    let mut t: u32 = 0;
    while (m20 >> t) > max_idx {
        t += 2;
    }
    let idx = rshift_round_i128(m20, t).clamp(0, max_idx) as usize;
    let r = i128::from(rsqrt_lut[idx]);
    let q = t as i32 - 20;
    // `c` liegt auf `ref`, `r` traegt `2^ref` mit (Wurzel aus `2^(2·ref)`):
    // Realwert `c·r / 2^norm_frac`.
    let norm_frac = i32::from(lut_output_frac) + i32::from(lut_input_shift) / 2 + q / 2;

    c.iter()
        .zip(gamma.iter().zip(gamma_shifts))
        .zip(beta.iter().zip(beta_shifts))
        .map(|((&ci, (&g, &gs)), (&b, &bs))| {
            let skala = norm_frac + i32::from(gs);
            let prod = i128::from(ci) * r * i128::from(g);
            let b_auf_skala = verschieben(i128::from(b), i32::from(bs) - skala);
            let y = verschieben(prod + b_auf_skala, skala - i32::from(out_frac_bits));
            clamp_i16(y.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32)
        })
        .collect()
}

/// **Die beiden Stuetzstellen einer Achse** fuer die bilineare Umrechnung
/// eines Gitters der Seitenlaenge `seite` auf `n` Stellen, an Stelle `i`,
/// mit `align_corners`: Quelle `i·(seite−1)/(n−1)`.
///
/// Rueckgabe `[(index, gewicht); 2]`, Gewichte in Q15, Summe genau `2^15`.
///
/// ⚑ **Die Quelle ist eine exakte rationale Zahl** `p/d` mit
/// `p = i·(seite−1)`, `d = n−1`. Ganzer Teil `p div d`, Rest `p mod d`;
/// das zweite Gewicht ist `round(rest·2^15/d)` (auf bei genau halb), das
/// erste der Rest bis `2^15`. **Die Division rechnet eine Konstante je
/// Bildgroesse**, wie `inv_n_q20` eine je Breite.
///
/// Am letzten Index ist der Rest null; die zweite Stuetzstelle wird auf den
/// Rand geklemmt und traegt Gewicht null, wie bei der Vorlage. Bei `n = 1`
/// liegt die Quelle bei null.
pub fn achsengewichte(i: usize, n: usize, seite: usize) -> [(usize, i32); 2] {
    assert!(i < n && seite > 0, "achsengewichte: Stelle {i} ausserhalb von {n}");
    let d = n.saturating_sub(1).max(1) as u64;
    let p = (i * (seite - 1)) as u64;
    let unten = (p / d) as usize;
    let rest = p % d;
    let eins = 1u64 << ACHSE_Q;
    let w1 = ((rest << ACHSE_Q) + d / 2) / d;
    [
        (unten, (eins - w1) as i32),
        ((unten + 1).min(seite - 1), w1 as i32),
    ]
}

/// **Zeile und Spalte jedes Flickens, in der Reihenfolge des Turms.**
///
/// Die Flicken laufen in Bloecken zu `ZUSAMMEN × ZUSAMMEN`: Blockzeile,
/// Blockspalte, dann innen Zeile und Spalte. So liegen die vier Flicken
/// eines Bildtokens hintereinander, und der Projektor legt je vier
/// aufeinanderfolgende zusammen.
pub fn flickenfolge(gh: usize, gw: usize) -> Vec<(usize, usize)> {
    assert!(
        gh.is_multiple_of(ZUSAMMEN) && gw.is_multiple_of(ZUSAMMEN),
        "flickenfolge: Gitter {gh}x{gw} ist kein Vielfaches von {ZUSAMMEN}"
    );
    let mut folge = Vec::with_capacity(gh * gw);
    for bh in 0..gh / ZUSAMMEN {
        for bw in 0..gw / ZUSAMMEN {
            for mh in 0..ZUSAMMEN {
                for mw in 0..ZUSAMMEN {
                    folge.push((bh * ZUSAMMEN + mh, bw * ZUSAMMEN + mw));
                }
            }
        }
    }
    folge
}

/// **Die Positions-Einbettung eines Flickens**, bilinear aus dem gelernten
/// Gitter `seite × seite` umgerechnet.
///
/// `tabelle` liegt **transponiert**, `[breite, seite·seite]`, mit einer
/// Skala je Kanal (`kanal_shifts`). Die vier Stuetzwerte eines Kanals teilen
/// damit ihre Skala; summiert wird mit den Gewichten (Q15 · Q15 = Q30) und
/// einmal je Kanal auf `out_frac[k]` gerundet.
///
/// ⚑ **Je Kanal und nicht je Position.** Mit einer Skala je Gitterstelle
/// teilten sich alle 1152 Kanaele einer Stelle die Aufloesung des groessten,
/// und nach der ersten LayerNorm lag der Fehler bei 1,56 % statt 0,32 %
/// (gemessen am 2026-10-23).
pub fn positions_einbettung(
    tabelle: &[i8],
    kanal_shifts: &[u8],
    breite: usize,
    seite: usize,
    (zeile, spalte): (usize, usize),
    (gh, gw): (usize, usize),
    out_frac: &[u8],
) -> Vec<i16> {
    let stellen = seite * seite;
    assert_eq!(tabelle.len(), stellen * breite, "positions_einbettung: Tabellenform");
    assert_eq!(kanal_shifts.len(), breite, "positions_einbettung: eine Skala je Kanal");
    assert_eq!(out_frac.len(), breite, "positions_einbettung: eine Ausgangsskala je Kanal");
    let ah = achsengewichte(zeile, gh, seite);
    let aw = achsengewichte(spalte, gw, seite);
    let mut stuetzen = [(0usize, 0i128); 4];
    for (n, &(ih, wh)) in ah.iter().enumerate() {
        for (m, &(iw, ww)) in aw.iter().enumerate() {
            stuetzen[2 * n + m] = (ih * seite + iw, i128::from(wh) * i128::from(ww));
        }
    }
    (0..breite)
        .map(|k| {
            let zeile = &tabelle[k * stellen..(k + 1) * stellen];
            let summe: i128 = stuetzen.iter().map(|&(z, w)| i128::from(zeile[z]) * w).sum();
            let skala = i32::from(kanal_shifts[k]) + 2 * ACHSE_Q as i32;
            let y = verschieben(summe, skala - i32::from(out_frac[k]));
            clamp_i16(y.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32)
        })
        .collect()
}

/// **Achtsamkeit einer Abfrage, spaet normiert.**
///
/// ```text
/// aus_d = round( Σ_j e_j · v_j[d]  /  Σ_j e_j ),   e_j = exp(s_j − max s)
/// ```
///
/// ⚑ **Die Gewichte werden nicht gerundet, bevor sie wirken.** Die
/// Softmax des Decoders rundet jede Wahrscheinlichkeit auf
/// `prob_frac_bits` (14); im Turm verteilt sich die Aufmerksamkeit ueber
/// Hunderte Flicken, ein typisches Gewicht ist dann rund 34 Stufen gross,
/// und die Rundung kostet je Gewicht anderthalb Prozent. Gemessen am
/// 2026-10-23 in Block 0: 3 % relativer Fehler nach der Achtsamkeit bei
/// 0,7 % davor. Hier wird mit den exp-Werten der Tabelle summiert, in i64,
/// und **einmal je Ausgabewert** geteilt, gerundet zur naechsten geraden
/// Zahl. Die Softmax teilt ebenfalls, einmal je Gewicht.
///
/// Skalen wie bei [`crate::attention::aufmerksamkeit_einer_abfrage`]:
/// `score_mult` traegt `1/sqrt(kopf)` in Q15, `score_shift` bringt das
/// Skalarprodukt auf die Eingangsskala der Tabelle, `lut_shift` auf ihr
/// Raster. Die Ausgabe hat die Skala von `v`.
pub fn achtsamkeit_spaet_normiert(
    q: &[i16],
    k: &[i16],
    v: &[i16],
    score_mult: i64,
    score_shift: u8,
    exp_lut: &[i16],
    lut_shift: u8,
) -> Vec<i16> {
    let breite = q.len();
    assert!(breite > 0 && k.len().is_multiple_of(breite), "achtsamkeit_spaet_normiert: k passt nicht zu q");
    let n = k.len() / breite;
    assert!(n > 0 && v.len().is_multiple_of(n), "achtsamkeit_spaet_normiert: k und v ungleich lang");
    let v_breite = v.len() / n;
    let scores: Vec<i32> = k
        .chunks_exact(breite)
        .map(|kj| {
            let d: i64 = q.iter().zip(kj).map(|(&a, &b)| i64::from(a) * i64::from(b)).sum();
            crate::fixed_point::rshift_round_i64(d * score_mult, score_shift) as i32
        })
        .collect();
    let groesster = *scores.iter().max().expect("mindestens ein Schluessel");
    let eins = i32::from(exp_lut[0]);
    let e: Vec<i64> = scores
        .iter()
        .map(|&s| i64::from(crate::softmax::exp_lut_lookup(groesster.saturating_sub(s), exp_lut, lut_shift, eins)))
        .collect();
    let summe: i64 = e.iter().sum();
    debug_assert!(summe > 0, "der groesste Score traegt immer die volle Eins");
    let mut acc = vec![0i64; v_breite];
    for (&ej, vj) in e.iter().zip(v.chunks_exact(v_breite)) {
        if ej != 0 {
            for (a, &x) in acc.iter_mut().zip(vj) {
                *a += ej * i64::from(x);
            }
        }
    }
    acc.into_iter().map(|a| crate::fixed_point::clamp_i16_from_i64(teilen_gerade(a, summe))).collect()
}

/// `a / b` gerundet zur naechsten geraden Zahl, `b > 0`.
fn teilen_gerade(a: i64, b: i64) -> i64 {
    let q = a.div_euclid(b);
    let r = a.rem_euclid(b);
    let doppelt = 2 * r;
    if doppelt > b || (doppelt == b && q & 1 != 0) {
        q + 1
    } else {
        q
    }
}

/// **Die Drehzeile eines Flickens**: erst die Winkel der Zeile, dann die der
/// Spalte, je `frequenzen` Stueck, aus derselben Tabelle `[stellen,
/// frequenzen]`.
///
/// So paart [`crate::rope::rotate_half_split_i16`] Stelle `j` mit
/// `j + 2·frequenzen` und dreht die erste Haelfte jedes Kopfes um die
/// Zeile, die zweite um die Spalte, ueber die **volle** Kopfbreite.
pub fn drehzeile(
    cos_lut: &[i16],
    sin_lut: &[i16],
    frequenzen: usize,
    (zeile, spalte): (usize, usize),
) -> (Vec<i16>, Vec<i16>) {
    let stellen = cos_lut.len() / frequenzen;
    assert!(
        zeile < stellen && spalte < stellen,
        "drehzeile: Stelle ({zeile}, {spalte}) hinter der Tabelle ({stellen})"
    );
    let ab = |t: &[i16], p: usize| t[p * frequenzen..(p + 1) * frequenzen].to_vec();
    let mut c = ab(cos_lut, zeile);
    c.extend(ab(cos_lut, spalte));
    let mut s = ab(sin_lut, zeile);
    s.extend(ab(sin_lut, spalte));
    (c, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rmsnorm::inv_n_q20;

    /// Die rsqrt-Tabelle der Spezifikation (input_shift 8, output_frac 8):
    /// `round(2^12 / sqrt(i))`, wie `spec_lut` in den RMSNorm-Tests.
    fn rsqrt_tabelle() -> Vec<i16> {
        (0..32768usize)
            .map(|i| {
                if i == 0 {
                    i16::MAX
                } else {
                    (4096.0 / (i as f64).sqrt()).round().min(32767.0) as i16
                }
            })
            .collect()
    }

    fn zufall(n: usize, saat: u64, spanne: i64) -> Vec<i64> {
        let mut x = saat | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x % (2 * spanne as u64 + 1)) as i64 - spanne
            })
            .collect()
    }

    /// LayerNorm in f64, als Massstab.
    fn layernorm_f64(x: &[f64], g: &[f64], b: &[f64], eps: f64) -> Vec<f64> {
        let n = x.len() as f64;
        let m = x.iter().sum::<f64>() / n;
        let v = x.iter().map(|a| (a - m) * (a - m)).sum::<f64>() / n;
        x.iter().zip(g).zip(b).map(|((a, gi), bi)| (a - m) / (v + eps).sqrt() * gi + bi).collect()
    }

    fn real(v: &[i16], frac: u8) -> Vec<f64> {
        v.iter().map(|&a| a as f64 / (1u64 << frac) as f64).collect()
    }

    /// Prueft gegen f64 mit einer Tabelle, deren Eintraege der RMSNorm-Probe
    /// entsprechen; der Massstab ist der relative L2-Fehler.
    fn messen(x: &[i16], x_shifts: &[u8], verschoben: f64) -> f64 {
        let n = x.len();
        let lut = rsqrt_tabelle();
        let gamma: Vec<i8> = zufall(n, 3, 100).into_iter().map(|v| v as i8).collect();
        let gamma_shifts = vec![6u8; n];
        let beta: Vec<i16> = zufall(n, 5, 3000).into_iter().map(|v| v as i16).collect();
        let beta_shifts = vec![10u8; n];
        let out = layernorm_i16(
            x, x_shifts, &gamma, &gamma_shifts, &beta, &beta_shifts, &lut, 8, 8, inv_n_q20(n), 1_099_512, 10,
        );
        let xr: Vec<f64> = x.iter().zip(x_shifts).map(|(&v, &s)| v as f64 / (1u64 << s) as f64 + verschoben).collect();
        let gr: Vec<f64> = gamma.iter().map(|&v| v as f64 / 64.0).collect();
        let br: Vec<f64> = beta.iter().map(|&v| v as f64 / 1024.0).collect();
        let soll = layernorm_f64(&xr, &gr, &br, 1e-6);
        let ist = real(&out, 10);
        let fehler: f64 = soll.iter().zip(&ist).map(|(a, b)| (a - b) * (a - b)).sum::<f64>().sqrt();
        let norm: f64 = soll.iter().map(|a| a * a).sum::<f64>().sqrt();
        eprintln!("layernorm: relativer Fehler {:.5}", fehler / norm);
        fehler / norm
    }

    #[test]
    fn layernorm_trifft_die_gleitkommarechnung() {
        let x: Vec<i16> = zufall(1152, 11, 4000).into_iter().map(|v| v as i16).collect();
        let f = messen(&x, &vec![8u8; 1152], 0.0);
        assert!(f < 1.5e-3, "relativer Fehler {f}");
    }

    /// ⚑ **Der Mittelwert wird wirklich abgezogen.** Derselbe Eingang, um
    /// einen festen Wert verschoben, ergibt dieselbe Norm; eine RMSNorm
    /// wuerde hier voellig anders rechnen.
    #[test]
    fn eine_verschiebung_aendert_nichts() {
        let roh: Vec<i64> = zufall(1152, 13, 3000);
        let x: Vec<i16> = roh.iter().map(|&v| v as i16).collect();
        let verschoben: Vec<i16> = roh.iter().map(|&v| (v + 20_000).clamp(-32768, 32767) as i16).collect();
        let f = messen(&verschoben, &vec![8u8; 1152], 0.0);
        assert!(f < 2e-3, "relativer Fehler mit Versatz {f}");
        let shifts = vec![8u8; 1152];
        let lut = rsqrt_tabelle();
        let g = vec![64i8; 1152];
        let gs = vec![6u8; 1152];
        let b = vec![0i16; 1152];
        let bs = vec![10u8; 1152];
        let a = layernorm_i16(&x, &shifts, &g, &gs, &b, &bs, &lut, 8, 8, inv_n_q20(1152), 0, 10);
        let c = layernorm_i16(&verschoben, &shifts, &g, &gs, &b, &bs, &lut, 8, 8, inv_n_q20(1152), 0, 10);
        // Gleich bis auf die Rundung des Mittelwerts und eine Nachbarzeile
        // der Tabelle; eine RMSNorm laege hier um Groessenordnungen daneben.
        let abstand: f64 = a.iter().zip(&c).map(|(&p, &q)| ((p - q) as f64).powi(2)).sum::<f64>().sqrt();
        let norm: f64 = a.iter().map(|&p| (p as f64).powi(2)).sum::<f64>().sqrt();
        assert!(abstand / norm < 3e-3, "Versatz aenderte die Ausgabe um {}", abstand / norm);
    }

    /// Kanaele mit verschiedener Skala, wie im Residualstrom.
    #[test]
    fn layernorm_mit_skalen_je_kanal() {
        let x: Vec<i16> = zufall(1152, 17, 20000).into_iter().map(|v| v as i16).collect();
        let shifts: Vec<u8> = (0..1152).map(|i| if i % 97 == 0 { 2 } else { 9 }).collect();
        let f = messen(&x, &shifts, 0.0);
        assert!(f < 1.5e-3, "relativer Fehler {f}");
    }

    /// Sehr leise Eingaenge: Die Streuung liegt unter der Indexgrenze, und
    /// der Index wird nach links in den genauen Bereich geschoben.
    #[test]
    fn leise_eingaenge_bleiben_genau() {
        let x: Vec<i16> = zufall(1152, 19, 6).into_iter().map(|v| v as i16).collect();
        let f = messen(&x, &vec![8u8; 1152], 0.0);
        assert!(f < 3e-3, "relativer Fehler {f}");
    }

    #[test]
    fn ohne_streuung_bleibt_beta() {
        let x = vec![500i16; 64];
        let b: Vec<i16> = (0..64).map(|i| i as i16 * 7 - 100).collect();
        let lut = rsqrt_tabelle();
        let out = layernorm_i16(&x, &[8; 64], &[64; 64], &[6; 64], &b, &[10; 64], &lut, 8, 8, inv_n_q20(64), 0, 10);
        assert_eq!(out, b);
    }

    /// Gegen die Formel der Vorlage, `i·47/(n−1)`, und die Summe genau 2^15.
    #[test]
    fn achsengewichte_wie_die_vorlage() {
        for n in [1usize, 2, 3, 7, 18, 22, 42, 48, 52, 64, 100] {
            for i in 0..n {
                let g = achsengewichte(i, n, 48);
                assert_eq!(g[0].1 + g[1].1, 1 << ACHSE_Q, "n {n}, i {i}");
                let src = if n == 1 { 0.0 } else { i as f64 * 47.0 / (n - 1) as f64 };
                let unten = src.floor();
                assert_eq!(g[0].0, unten as usize);
                assert_eq!(g[1].0, ((unten as usize) + 1).min(47));
                let w1 = (src - unten) * 32768.0;
                assert!((g[1].1 as f64 - w1).abs() <= 0.5 + 1e-9, "n {n}, i {i}: {} gegen {w1}", g[1].1);
            }
        }
        // Die Raender treffen das Gitter genau.
        assert_eq!(achsengewichte(0, 22, 48), [(0, 32768), (1, 0)]);
        assert_eq!(achsengewichte(21, 22, 48), [(47, 32768), (47, 0)]);
    }

    /// Die Folge der Vorlage: Blockzeile, Blockspalte, innen Zeile, Spalte.
    #[test]
    fn flickenfolge_in_bloecken() {
        let f = flickenfolge(4, 6);
        assert_eq!(&f[..8], &[(0, 0), (0, 1), (1, 0), (1, 1), (0, 2), (0, 3), (1, 2), (1, 3)]);
        assert_eq!(f[12], (2, 0));
        assert_eq!(f.len(), 24);
        let mut sortiert = f.clone();
        sortiert.sort();
        sortiert.dedup();
        assert_eq!(sortiert.len(), 24);
    }

    #[test]
    fn drehzeile_erst_zeile_dann_spalte() {
        let cos: Vec<i16> = (0..10 * 3).map(|v| v as i16).collect();
        let sin: Vec<i16> = (0..10 * 3).map(|v| -(v as i16)).collect();
        let (c, s) = drehzeile(&cos, &sin, 3, (2, 5));
        assert_eq!(c, vec![6, 7, 8, 15, 16, 17]);
        assert_eq!(s, vec![-6, -7, -8, -15, -16, -17]);
    }

    /// Ein Flicken genau auf einer Gitterstelle bekommt deren Werte.
    #[test]
    fn positions_einbettung_an_gitterstellen() {
        let seite = 4;
        let breite = 3;
        let stellen = seite * seite;
        // Transponiert: Kanal k, Stelle z an k * stellen + z.
        let tabelle: Vec<i8> = (0..stellen * breite).map(|v| (v as i8).wrapping_mul(3)).collect();
        let shifts = vec![5u8; breite];
        for z in 0..4 {
            for s in 0..4 {
                let e = positions_einbettung(&tabelle, &shifts, breite, seite, (z, s), (4, 4), &[5, 5, 5]);
                let soll: Vec<i16> = (0..breite).map(|k| tabelle[k * stellen + z * seite + s] as i16).collect();
                assert_eq!(e, soll);
            }
        }
        // Mitte zwischen vier Stellen: der Mittelwert, je Kanal mit eigener Skala.
        let tab: Vec<i8> = vec![0, 40, 80, 120, 0, 40, 80, 120];
        let e = positions_einbettung(&tab, &[0, 1], 2, 2, (1, 1), (3, 3), &[0, 1]);
        assert_eq!(e, vec![60, 60]);
    }

    /// Spaet normiert gegen f64: Softmax mit exakter Division.
    #[test]
    fn spaet_normierte_achtsamkeit_trifft_f64() {
        let breite = 72;
        let n = 500;
        let q: Vec<i16> = zufall(breite, 23, 2000).into_iter().map(|v| v as i16).collect();
        let k: Vec<i16> = zufall(breite * n, 29, 2000).into_iter().map(|v| v as i16).collect();
        let v: Vec<i16> = zufall(breite * n, 31, 8000).into_iter().map(|v| v as i16).collect();
        // exp-Tabelle wie die Spezifikation: Eingang 2^-8, Ausgang 2^14.
        let lut: Vec<i16> = (0..16384).map(|i| ((-(i as f64) / 256.0).exp() * 16384.0).round() as i16).collect();
        let mult = crate::fixed_point::inv_sqrt_q15(breite);
        // q und k mit 10 Bruchstellen, Scores auf 14: 10 + 10 + 15 - 14.
        let aus = achtsamkeit_spaet_normiert(&q, &k, &v, mult, 21, &lut, 6);
        let qr: Vec<f64> = q.iter().map(|&a| a as f64 / 1024.0).collect();
        let s: Vec<f64> = k
            .chunks(breite)
            .map(|kj| kj.iter().zip(&qr).map(|(&b, a)| a * b as f64 / 1024.0).sum::<f64>() / (breite as f64).sqrt())
            .collect();
        let m = s.iter().cloned().fold(f64::MIN, f64::max);
        let e: Vec<f64> = s.iter().map(|x| (x - m).exp()).collect();
        let z: f64 = e.iter().sum();
        let soll: Vec<f64> = (0..breite)
            .map(|d| e.iter().zip(v.chunks(breite)).map(|(w, vj)| w * vj[d] as f64).sum::<f64>() / z)
            .collect();
        let fehler: f64 = soll.iter().zip(&aus).map(|(a, &b)| (a - b as f64).powi(2)).sum::<f64>().sqrt();
        let norm: f64 = soll.iter().map(|a| a * a).sum::<f64>().sqrt();
        assert!(fehler / norm < 5e-3, "relativer Fehler {}", fehler / norm);
    }

    #[test]
    fn teilen_gerade_rundet_zur_geraden() {
        assert_eq!(teilen_gerade(5, 2), 2);
        assert_eq!(teilen_gerade(7, 2), 4);
        assert_eq!(teilen_gerade(-5, 2), -2);
        assert_eq!(teilen_gerade(-7, 2), -4);
        assert_eq!(teilen_gerade(10, 3), 3);
        assert_eq!(teilen_gerade(11, 3), 4);
    }
}
