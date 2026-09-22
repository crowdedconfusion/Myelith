//! MLP / Feed Forward – Integer (Aktivierungen int16, Per-Layer-Skalen)
// Die Gewichtsmatrizen heißen wie im Whitepaper (Anhang B): `W`, `W_gate`,
// `W_up`, `W_down`. Klein geschrieben wären sie von den Einzelgewichten
// `w` im selben Rumpf nicht mehr zu unterscheiden — die Entsprechung zur
// Referenzformel ist beim Nachrechnen mehr wert als die Namenskonvention.
#![allow(non_snake_case)]

use crate::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};
use crate::integer_math::silu_nachschlagen;
use crate::linear::{linear_w8a16, linear_w8a16_pc};

/// Integer-MLP mit SiLU-Approximation via LUT.
///
/// Skalen (alles kalibrierte Per-Layer-Zweierpotenz-Skalen, siehe
/// `scales.json`):
/// - `in_frac_bits`: Eingang (Ausgabe der post_attention_layernorm)
/// - `gate_out_frac`/`up_out_frac`: Ausgaenge von gate-/up-Projektion
/// - `down_in_frac`: Eingang von down_proj (h = silu(gate)*up)
/// - `out_frac_bits`: Ausgangsskala JE KANAL (Fund 20, theta_v 0.11.0) -
///   down_proj addiert direkt in den Residualstrom, der seit Fund 20 eine
///   Skala je Kanal trägt (Massive Activations bei Qwen2.5-7B)
///
/// Die SiLU-LUT arbeitet in einer festen Domäne (`silu_in_frac`, Index-
/// Offset `silu_lut_offset` = -input_min der spec): Gate-Werte werden vor
/// dem Lookup in diese Domäne reskaliert. **Jenseits der Tabelle gilt die
/// Funktion selbst**: oberhalb die Identität, unterhalb null, siehe
/// [`crate::integer_math::silu_nachschlagen`].
///
/// 📌 **Fund 349 und Fund 364 (2026-09-14): Hier stand „große
/// Betragswerte saturieren deterministisch am LUT-Rand", und das war die
/// falsche Funktion.** Deterministisch war es, richtig nicht: SiLU(278)
/// ist 278 und nicht 128. Gemessen reichen die Gate-Werte bei
/// `myelith-0.6b` bis 278 und bei `myelith-4b` bis 192, die Tabelle aber
/// nur bis 128. Der Rückwärtspass leitete seinen Gradienten aus der
/// Tabelle ab und nahm jenseits davon schon Steigung 1 an; der
/// Vorwärtspass lag flach. Seither sagen beide dasselbe.
///
/// 📌 **Damit ist auch Fund 75 an dieser Stelle erledigt.** Hier stand
/// ein ungesichertes `g_dom as i16` samt Zusicherung, dass der Wert in
/// `i16` passt, und in `backward.rs` eine Sättigung auf `i16`, die den
/// Versatz danach wieder überlaufen ließ. Beide sind entfallen: Der Wert
/// bleibt `i32`, und was außerhalb der Tabelle liegt, entscheidet eine
/// Stelle, bevor überhaupt ein Index entsteht.
#[allow(clippy::too_many_arguments)]
pub fn mlp_int(
    x: &[i16],
    W_gate: &[i8],
    W_up: &[i8],
    W_down: &[i8],
    hidden_size: usize,
    intermediate_size: usize,
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
) -> Vec<i16> {
    mlp_int_mit_spur(
        x, W_gate, W_up, W_down, hidden_size, intermediate_size, gate_w_shifts, up_w_shifts,
        down_w_shifts, silu_lut, in_frac_bits, gate_out_frac, up_out_frac, down_in_frac,
        silu_in_frac, silu_lut_offset, silu_out_frac, out_frac_bits, None,
    )
}

/// Was der MLP-Block an Zwischenwerten zurücklässt (TRAINING V).
///
/// ⚑ **Drei Werte, und jeder hat genau einen Abnehmer:** `gate` ist das
/// `x` von [`crate::backward::silu_backward`], `up` der zweite Faktor
/// des Produkts, und `h` das `x` von `down_proj`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mlpspur {
    /// Die Gate-Projektion **vor** der Aktivierung, auf `gate_out_frac`.
    pub gate: Vec<i16>,
    /// Die Up-Projektion, auf `up_out_frac`.
    pub up: Vec<i16>,
    /// Das Produkt `silu(gate) · up`, auf `down_in_frac`: der Eingang
    /// von `down_proj`.
    pub h: Vec<i16>,
}

/// Dasselbe wie [`mlp_int`], aber die Zwischenwerte fallen mit ab.
///
/// # ⚑ Warum ein zweiter Eingang und kein Parameter mehr an `mlp_int`
///
/// `mlp_int` steht im `Backend`-Merkmal, in vier Umsetzungen. Ein
/// zusätzliches Argument dort risse alle vier auf, für etwas, das nur
/// der Rückwärtspass braucht. **Hier steht die eine Umsetzung**, und
/// `mlp_int` ist ihr Eingang ohne Spur.
#[allow(non_snake_case, clippy::too_many_arguments)]
pub fn mlp_int_mit_spur(
    x: &[i16],
    W_gate: &[i8],
    W_up: &[i8],
    W_down: &[i8],
    hidden_size: usize,
    intermediate_size: usize,
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
    spur: Option<&mut Mlpspur>,
) -> Vec<i16> {
    // Flache Gewichte, Begründung im Kopf von `linear_w8a16`.
    let gate = linear_w8a16(x, W_gate, hidden_size, gate_w_shifts, in_frac_bits, gate_out_frac);
    let up = linear_w8a16(x, W_up, hidden_size, up_w_shifts, in_frac_bits, up_out_frac);

    let h = silu_produkt(
        &gate,
        &up,
        silu_lut,
        gate_out_frac,
        up_out_frac,
        down_in_frac,
        silu_in_frac,
        silu_lut_offset,
        silu_out_frac,
    );

    // ⚑ **Alle drei zusammen oder keiner.** Sie gehören zu **einem**
    // Durchlauf; wer nur zwei nähme, rechnete einen Gradienten aus
    // Werten, die nie gemeinsam entstanden sind.
    if let Some(sp) = spur {
        sp.gate = gate;
        sp.up = up;
        sp.h = h.clone();
    }

    linear_w8a16_pc(
        &h,
        W_down,
        intermediate_size,
        down_w_shifts,
        down_in_frac,
        out_frac_bits,
    )
}

/// **SiLU, Produkt und Reskalierung: der elementweise Teil des MLP.**
///
/// 📌 **Er stand bis zum 2026-09-11 an drei Stellen**, zweimal mit dem
/// Vermerk „wortgleich mit". Ein Kommentar, der Gleichheit behauptet,
/// ist keine Gleichheit; hier ist sie jetzt eine.
///
/// Der Gate-Wert wird in die feste LUT-Domaene reskaliert,
/// nachgeschlagen und mit `up` auf die kalibrierte down-Eingangsskala
/// gebracht. Jenseits der Tabelle gilt die Fortsetzung aus
/// [`crate::integer_math::silu_nachschlagen`].
#[allow(clippy::too_many_arguments)]
pub fn silu_produkt(
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
    // ⛔️ **`zip` bricht an der kuerzeren Seite ab, ohne ein Wort zu
    // sagen** (Fund 346). Bis zum 2026-09-21 stand hier keine Pruefung,
    // und sie war auch nicht noetig: Beide Seiten kamen aus derselben
    // Matrix mit derselben Zwischengroesse.
    //
    // ⚠️ **Das gilt nicht mehr.** Die torgesteuerte Norm der
    // Zustandsschicht ruft dieselbe Funktion mit `z` aus einer
    // Projektion und dem Rekurrenzausgang, also mit zwei Groessen aus
    // **verschiedenen** Quellen. Stimmt eine Kopfzahl nicht, liefert
    // `zip` still ein zu kurzes Ergebnis, und das faellt erst viel
    // spaeter als eine schlechte Zahl auf.
    //
    // 📌 **Eine Annahme, die heute stimmt, gehoert hingeschrieben,
    // bevor jemand den zweiten Aufrufer baut.**
    assert_eq!(
        gate.len(),
        up.len(),
        "silu_produkt: {} Torwerte gegen {} Faktoren",
        gate.len(),
        up.len()
    );
    let mut h = Vec::with_capacity(gate.len());
    for (g, u) in gate.iter().zip(up.iter()) {
        let g_dom = rescale(*g as i32, gate_out_frac, silu_in_frac);
        let activated =
            silu_nachschlagen(g_dom, silu_lut, silu_lut_offset, silu_in_frac, silu_out_frac);
        let prod = activated * (*u as i64);
        h.push(clamp_i16_from_i64(rescale_i64(
            prod,
            silu_out_frac + up_out_frac,
            down_in_frac,
        )));
    }
    h
}

/// **Der MLP-Block fuer mehrere Eingaben auf denselben Gewichten.**
///
/// ⚑ **Element fuer Element dasselbe wie [`mlp_int`] je Eingabe.** Die
/// drei Matrizen werden einmal gelesen statt einmal je Eingabe; die
/// Zwischenrechnung (SiLU, Produkt, Reskalierung) haengt nur am
/// einzelnen Wert und laeuft unveraendert je Eingabe.
///
/// ⚠️ **Ohne Mitschnitt.** Der Trainingspfad rechnet Token fuer Token
/// und braucht die Zwischenwerte; wer sie will, nimmt
/// [`mlp_int_mit_spur`].
#[allow(clippy::too_many_arguments)]
pub fn mlp_int_stapel(
    xs: &[&[i16]],
    W_gate: &[i8],
    W_up: &[i8],
    W_down: &[i8],
    hidden_size: usize,
    intermediate_size: usize,
    gate_w_shifts: &[u8],
    up_w_shifts: &[u8],
    down_w_shifts: &[u8],
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
) -> Vec<Vec<i16>> {
    if xs.is_empty() {
        return Vec::new();
    }
    let gate = crate::linear::linear_w8a16_stapel(
        xs,
        W_gate,
        hidden_size,
        gate_w_shifts,
        in_frac_bits,
        gate_out_frac,
    );
    let up = crate::linear::linear_w8a16_stapel(
        xs,
        W_up,
        hidden_size,
        up_w_shifts,
        in_frac_bits,
        up_out_frac,
    );

    // ⚑ **Verteilt ueber die Faeden, Eingabe fuer Eingabe.** Gemessen am
    // 2026-09-14 (4B, 219 Token, Matrizen auf der GPU) lag dieser Schritt
    // einkernig bei rund einem Fuenftel der ganzen Vorbereitung: 9 728
    // Tabellenzugriffe je Token und Ebene. Jede Eingabe schreibt in ihren
    // eigenen Abschnitt, also aendert die Fadenzahl keine Zahl.
    let faeden = if xs.len() < 2 { 1 } else { crate::linear::kerngrenze() };
    let hs = crate::fadenpool::rechnen_breit(xs.len(), intermediate_size, faeden, |b, ziel| {
        ziel.copy_from_slice(&silu_produkt(
            &gate[b],
            &up[b],
            silu_lut,
            gate_out_frac,
            up_out_frac,
            down_in_frac,
            silu_in_frac,
            silu_lut_offset,
            silu_out_frac,
        ));
    });

    let scheiben: Vec<&[i16]> = hs.chunks_exact(intermediate_size).collect();
    crate::linear::linear_w8a16_pc_stapel(
        &scheiben,
        W_down,
        intermediate_size,
        down_w_shifts,
        down_in_frac,
        out_frac_bits,
    )
}

/// Die drei Matrizen eines Experten, wie ein Buendel sie sieht.
pub struct Expertenteil<'a> {
    pub gate: &'a [i8],
    pub up: &'a [i8],
    pub down: &'a [i8],
    pub gate_shifts: &'a [u8],
    pub up_shifts: &'a [u8],
    pub down_shifts: &'a [u8],
}

/// **Alle gewaehlten Experten einer Ebene in zwei Runden statt in
/// vierundzwanzig.**
///
/// # 📌 Fund 332 (2026-09-11)
///
/// Die Begruendung und die Messung stehen bei
/// [`crate::linear::linear_w8a16_buendel`]. Kurz: Eine Poolrunde kostet
/// mehr als die kleine Matrix, die sie rechnet, und eine
/// Expertenmatrix von Qwen3-30B-A3B bekam dabei **zwei** von zwoelf
/// Faeden. Gemessen ueber dieselben Zeilen: **93,45 ms gegen 19,12 ms**
/// je Token.
///
/// **Zwei Runden, nicht eine**, und das liegt an der Abhaengigkeit:
/// `down` braucht `h`, und `h` braucht `gate` und `up`. Dazwischen
/// liegt der elementweise Teil, der nichts zu verteilen hat.
///
/// ⚑ **Element fuer Element dasselbe wie [`mlp_int_mit_spur`] je
/// Experte.** Jede Ausgabezeile bleibt ihr eigenes Skalarprodukt ueber
/// ihre eigene Gewichtszeile; geaendert ist allein, welcher Faden
/// welche Zeile nimmt. Geprueft in `gebuendelte_experten_sind_dasselbe`.
#[allow(clippy::too_many_arguments)]
pub fn mlp_int_experten(
    x: &[i16],
    experten: &[Expertenteil<'_>],
    hidden_size: usize,
    intermediate_size: usize,
    silu_lut: &[i16],
    in_frac_bits: u8,
    gate_out_frac: u8,
    up_out_frac: u8,
    down_in_frac: u8,
    silu_in_frac: u8,
    silu_lut_offset: i16,
    silu_out_frac: u8,
    out_frac_bits: &[u8],
    spuren: Option<&mut Vec<Mlpspur>>,
) -> Vec<Vec<i16>> {
    use crate::linear::{Ausgangsskala, Buendelteil};

    if experten.is_empty() {
        return Vec::new();
    }

    // Erste Runde: gate und up jedes Experten, alle ueber derselben
    // Eingabe.
    let mut vorne: Vec<Buendelteil<'_>> = Vec::with_capacity(2 * experten.len());
    for e in experten {
        vorne.push(Buendelteil {
            w: e.gate,
            x,
            in_features: hidden_size,
            w_shifts: e.gate_shifts,
            act_frac_bits: in_frac_bits,
            aus: Ausgangsskala::Eine(gate_out_frac),
        });
        vorne.push(Buendelteil {
            w: e.up,
            x,
            in_features: hidden_size,
            w_shifts: e.up_shifts,
            act_frac_bits: in_frac_bits,
            aus: Ausgangsskala::Eine(up_out_frac),
        });
    }
    let flach = crate::linear::linear_w8a16_buendel(&vorne);

    // Der elementweise Teil je Experte. Er verteilt sich nicht und
    // laeuft deshalb hier, zwischen den beiden Runden.
    let mut gates: Vec<Vec<i16>> = Vec::with_capacity(experten.len());
    let mut ups: Vec<Vec<i16>> = Vec::with_capacity(experten.len());
    let mut hs: Vec<Vec<i16>> = Vec::with_capacity(experten.len());
    // ⚑ **Die Versaetze laufen mit, sie werden nicht gerechnet.** Ein
    // `2 * i * intermediate_size` waere richtig, solange gate und up
    // gleich viele Zeilen haben, und still falsch, sobald nicht.
    let mut versatz = 0usize;
    for e in experten.iter() {
        let gz = e.gate_shifts.len();
        let uz = e.up_shifts.len();
        let gate = &flach[versatz..versatz + gz];
        let up = &flach[versatz + gz..versatz + gz + uz];
        versatz += gz + uz;
        hs.push(silu_produkt(
            gate,
            up,
            silu_lut,
            gate_out_frac,
            up_out_frac,
            down_in_frac,
            silu_in_frac,
            silu_lut_offset,
            silu_out_frac,
        ));
        if spuren.is_some() {
            gates.push(gate.to_vec());
            ups.push(up.to_vec());
        }
    }

    // Zweite Runde: down jedes Experten, jeder ueber **seiner** eigenen
    // Eingabe.
    let hinten: Vec<Buendelteil<'_>> = experten
        .iter()
        .zip(hs.iter())
        .map(|(e, h)| Buendelteil {
            w: e.down,
            x: h,
            in_features: intermediate_size,
            w_shifts: e.down_shifts,
            act_frac_bits: down_in_frac,
            aus: Ausgangsskala::JeZeile(out_frac_bits),
        })
        .collect();
    let flach_aus = crate::linear::linear_w8a16_buendel(&hinten);

    let breite = out_frac_bits.len();
    let ausgaben: Vec<Vec<i16>> = (0..experten.len())
        .map(|i| flach_aus[i * breite..(i + 1) * breite].to_vec())
        .collect();

    // ⚑ **Alle drei zusammen oder keiner**, wie bei
    // [`mlp_int_mit_spur`]: Sie gehoeren zu **einem** Durchlauf.
    if let Some(ziel) = spuren {
        for i in 0..experten.len() {
            ziel.push(Mlpspur {
                gate: std::mem::take(&mut gates[i]),
                up: std::mem::take(&mut ups[i]),
                h: hs[i].clone(),
            });
        }
    }

    ausgaben
}

#[cfg(test)]
mod stapeltests {
    use super::*;

    /// **Gebuendelt ist bitgleich zu einzeln, auch ueber den ganzen
    /// MLP-Block.**
    #[test]
    fn der_gebuendelte_mlp_rechnet_dasselbe() {
        let lut = spec_silu_lut();
        let (hs, is) = (16usize, 40usize);
        let mach = |n: usize, saat: u64| -> Vec<i8> {
            let mut x = saat | 1;
            (0..n)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    ((x % 251) as i64 - 125) as i8
                })
                .collect()
        };
        let w_gate = mach(is * hs, 3);
        let w_up = mach(is * hs, 5);
        let w_down = mach(hs * is, 7);
        let gs = vec![5u8; is];
        let us = vec![5u8; is];
        let ds = vec![5u8; hs];
        let out = vec![6u8; hs];

        for b in [1usize, 3, 9] {
            let eingaben: Vec<Vec<i16>> = (0..b)
                .map(|i| (0..hs).map(|j| ((i * 7 + j * 13) % 200) as i16 - 100).collect())
                .collect();
            let scheiben: Vec<&[i16]> = eingaben.iter().map(|v| v.as_slice()).collect();
            let gebuendelt = mlp_int_stapel(
                &scheiben, &w_gate, &w_up, &w_down, hs, is, &gs, &us, &ds, &lut, 5, 5, 5, 5, 1,
                -256, 6, &out,
            );
            for (i, x) in eingaben.iter().enumerate() {
                let einzeln = mlp_int(
                    x, &w_gate, &w_up, &w_down, hs, is, &gs, &us, &ds, &lut, 5, 5, 5, 5, 1, -256,
                    6, &out,
                );
                assert_eq!(gebuendelt[i], einzeln, "b={b}, i={i}");
            }
        }
    }

    /// **Acht gebuendelte Experten sind bitgleich zu acht einzelnen,
    /// samt Mitschnitt.**
    ///
    /// 📌 **Die Gegenprobe zu Fund 332.** Sie prueft beides, was der
    /// gebuendelte Weg anders macht: die Aufteilung ueber die Faeden
    /// (das Ergebnis) und die Zwischenwerte (den Mitschnitt, an dem der
    /// Rueckwaertspfad haengt).
    #[test]
    fn gebuendelte_experten_sind_dasselbe() {
        let lut = spec_silu_lut();
        let (hs, is) = (16usize, 40usize);
        let mach = |n: usize, saat: u64| -> Vec<i8> {
            let mut x = saat | 1;
            (0..n)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    ((x % 251) as i64 - 125) as i8
                })
                .collect()
        };
        let out = vec![6u8; hs];
        let x: Vec<i16> = (0..hs).map(|j| ((j * 13) % 200) as i16 - 100).collect();

        for n in [1usize, 2, 8] {
            // Jeder Experte bekommt eigene Gewichte und eigene
            // Zeilenskalen; gleiche waeren hier die schwaechere Probe.
            let gate: Vec<Vec<i8>> = (0..n).map(|i| mach(is * hs, 3 + i as u64)).collect();
            let up: Vec<Vec<i8>> = (0..n).map(|i| mach(is * hs, 53 + i as u64)).collect();
            let down: Vec<Vec<i8>> = (0..n).map(|i| mach(hs * is, 101 + i as u64)).collect();
            let gs: Vec<Vec<u8>> =
                (0..n).map(|i| (0..is).map(|r| 5 + ((r + i) % 3) as u8).collect()).collect();
            let us: Vec<Vec<u8>> =
                (0..n).map(|i| (0..is).map(|r| 5 + ((r + i) % 2) as u8).collect()).collect();
            let ds: Vec<Vec<u8>> =
                (0..n).map(|i| (0..hs).map(|r| 5 + ((r + i) % 4) as u8).collect()).collect();

            let teile: Vec<Expertenteil<'_>> = (0..n)
                .map(|i| Expertenteil {
                    gate: &gate[i],
                    up: &up[i],
                    down: &down[i],
                    gate_shifts: &gs[i],
                    up_shifts: &us[i],
                    down_shifts: &ds[i],
                })
                .collect();

            let mut spuren: Vec<Mlpspur> = Vec::new();
            let gebuendelt = mlp_int_experten(
                &x, &teile, hs, is, &lut, 5, 5, 5, 5, 1, -256, 6, &out, Some(&mut spuren),
            );

            assert_eq!(gebuendelt.len(), n, "n={n}: nicht jeder Experte hat eine Ausgabe");
            assert_eq!(spuren.len(), n, "n={n}: nicht jeder Experte hat einen Mitschnitt");
            for i in 0..n {
                let mut sp = Mlpspur::default();
                let einzeln = mlp_int_mit_spur(
                    &x, &gate[i], &up[i], &down[i], hs, is, &gs[i], &us[i], &ds[i], &lut, 5, 5, 5,
                    5, 1, -256, 6, &out, Some(&mut sp),
                );
                assert_eq!(gebuendelt[i], einzeln, "n={n}, Experte {i}");
                assert_eq!(spuren[i].gate, sp.gate, "n={n}, Experte {i}: gate im Mitschnitt");
                assert_eq!(spuren[i].up, sp.up, "n={n}, Experte {i}: up im Mitschnitt");
                assert_eq!(spuren[i].h, sp.h, "n={n}, Experte {i}: h im Mitschnitt");
            }
        }
    }

    /// **Kein Experte, keine Ausgabe, kein Absturz.**
    #[test]
    fn ohne_experten_gibt_es_nichts() {
        let lut = spec_silu_lut();
        let out = vec![6u8; 4];
        let x = vec![0i16; 4];
        assert!(mlp_int_experten(&x, &[], 4, 4, &lut, 5, 5, 5, 5, 1, -256, 6, &out, None).is_empty());
    }

    /// Dieselbe LUT wie in den übrigen Prüfungen dieser Datei.
    fn spec_silu_lut() -> Vec<i16> {
        let mut lut = Vec::with_capacity(512);
        for x in -256..256 {
            let xf = x as f64 / 2.0;
            let s = xf / (1.0 + (-xf).exp());
            lut.push((s * 64.0).round() as i16);
        }
        lut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SiLU-LUT im spec-Format (input_frac 1, output_frac 6, [-256, 255]).
    fn spec_silu_lut() -> Vec<i16> {
        let mut lut = Vec::with_capacity(512);
        for x in -256..256 {
            let xf = x as f64 / 2.0;
            let val = xf * (1.0 / (1.0 + (-xf).exp()));
            lut.push((val * 64.0).round() as i16);
        }
        lut
    }

    #[test]
    fn test_mlp_runs_with_per_layer_scales() {
        // Rauchtest: 2 Kanaele, intermediate 2; alle Skalen explizit.
        let x = vec![64i16, -32];
        // Flach, wie der Kernel sie seit v0.13.4 nimmt: 2x2-Matrizen.
        let w_gate: Vec<i8> = vec![64, 0, 0, 64];
        let w_up: Vec<i8> = vec![64, 0, 0, 64];
        let w_down: Vec<i8> = vec![64, 32, 32, 64];
        let (hidden, inter) = (2usize, 2usize);
        let lut = spec_silu_lut();
        let out = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            hidden, inter,
            &[6, 6], &[6, 6], &[6, 6], // Per-Channel-Gewichts-Shifts
            &lut,
            6,   // in_frac
            6, 6, 6, // gate/up/down-Eingangs-Skalen
            1, 256, 6, // SiLU-Domäne (frac 1, Offset 256, Output frac 6)
            &[6, 6],   // out_frac (Fund 20: je Kanal, hier uniform)
        );
        assert_eq!(out.len(), 2);
        // Alle Werte muessen im i16-Bereich und deterministisch sein.
        let out2 = mlp_int(
            &x, &w_gate, &w_up, &w_down,
            hidden, inter,
            &[6, 6], &[6, 6], &[6, 6],
            &lut,
            6, 6, 6, 6, 1, 256, 6, &[6, 6],
        );
        assert_eq!(out, out2);
    }

    /// 📌 Fund 349: Ein Gate-Wert jenseits der Tabelle ist die Identität,
    /// nicht der letzte Tabelleneintrag. Skalen so gewählt, dass nichts
    /// reskaliert: `gate_out_frac` = `silu_in_frac` = 1, `up` ist genau
    /// 1,0 auf null Bruchstellen, `down_in_frac` = 1. Dann ist `h` der
    /// reale SiLU-Wert auf einer Bruchstelle.
    #[test]
    fn ein_gate_wert_jenseits_der_tabelle_ist_die_identitaet() {
        let lut = spec_silu_lut();
        // Die Tabelle endet bei 255 (real 127,5). 1000 ist real 500.
        let h = silu_produkt(&[1000], &[1], &lut, 1, 0, 1, 1, 256, 6);
        assert_eq!(h, vec![1000], "SiLU(500) ist 500, nicht der Tabellenrand");
        // Am Rand stetig: 255 liegt noch in der Tabelle, 256 schon dahinter.
        let rand = silu_produkt(&[255, 256], &[1, 1], &lut, 1, 0, 1, 1, 256, 6);
        assert_eq!(rand, vec![255, 256]);
        // Unterhalb der Tabelle null.
        let unten = silu_produkt(&[-1000], &[1], &lut, 1, 0, 1, 1, 256, 6);
        assert_eq!(unten, vec![0]);
    }
}
