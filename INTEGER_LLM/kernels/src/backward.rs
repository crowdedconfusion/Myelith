//! Rückwärtspass, ganzzahlig.
//!
//! ## Warum dieses Modul existiert
//!
//! Der Vorwärtspfad rechnet vollständig in ganzen Zahlen und ist über
//! 30 Konformitätsvektoren als bitgleich belegt. Für **verifizierbares
//! Training** genügt das nicht: Solange der Gradient aus einer
//! Gleitkommarechnung kommt, ist er geräteabhängig, und mit ihm jede
//! Gewichtsänderung. Zwei Miner, die dasselbe Segment trainieren,
//! bekämen dann verschiedene Ergebnisse, und der Redundanzvergleich
//! meldete einen Betrug, wo nur zwei Prozessoren verschieden gerundet
//! haben.
//!
//! Die Messungen in `TRAINING/tests/diag/` haben vorher geklärt, dass
//! sich der Aufwand lohnt: Das Quantisierungsschema trägt im
//! Rückwärtspass (+0,67 % gegen die Gleitkomma-Referenz), und ein
//! Trainingsschritt ohne Gleitkommazustand ist möglich (+0,75 %).
//!
//! ## Was hier gilt
//!
//! Dieselben Regeln wie im Vorwärtspfad, und aus denselben Gründen:
//!
//! - **Akkumulation exakt in `i64`**, Sättigung genau **einmal** ganz am
//!   Ende. Wer zwischendurch klemmt, macht die Summe
//!   reihenfolgeabhängig; siehe den Determinismus-Vertrag im Kopf von
//!   [`crate::dot`].
//! - **Division ausschließlich als arithmetischer Rechtsshift** mit
//!   `rshift_round` (Whitepaper Kap. 6.2, Anhang B.5.4).
//! - **Kein Gleitkomma**, auch nicht in Zwischenschritten.
//!
//! ## Die Ableitungen leiten die IMPLEMENTIERUNG ab, nicht die Formel
//!
//! Ein Rückwärtspass, der eine andere Funktion ableitet als der
//! Vorwärtspfad rechnet, ist wertlos, und der Unterschied fällt in
//! keinem Test auf, der nur die Formel prüft. Jede Funktion hier nennt
//! deshalb, welche Vorwärtsfunktion sie ableitet, und die Tests
//! vergleichen gegen eine **numerische Ableitung des echten
//! Vorwärtskernels**, nicht gegen eine nachgerechnete Formel.
//!
//! ## Was noch fehlt
//!
//! Die Kernel stehen vollständig. Offen sind die Golden Vectors für den
//! Rückwärtspass und der Nachweis, dass zwei Maschinen denselben
//! Gradienten liefern; beides gehört in den Prüfstand, nicht in dieses
//! Modul.

use crate::fixed_point::{clamp_i32, rescale_i64, rshift_round_i64};

/// Die Wortbreite, in der Gradienten zwischen den Ebenen laufen.
///
/// **Warum `i32` und nicht `i8` wie die Gewichte.** Gemessen
/// (`TRAINING/tests/diag/backward_reference_simulation.py`): Der
/// Dynamikbereich der Gradienten beträgt im Median **26,6 Bits**, im
/// Maximum über 90. int8 deckt sieben ab. Die Quantisierung auf int8 je
/// Block ist deshalb eine **Übertragungsform** an der Ebenengrenze
/// (siehe [`quantisiere_block`]) und nicht die Rechenform: Wer schon
/// innerhalb einer Ebene auf acht Bit rundet, verliert dort, wo es nicht
/// nötig ist.
pub type Grad = i32;

// ---------------------------------------------------------------------------
// Übertragungsform: int8 mit Zweierpotenz-Skala je Block
// ---------------------------------------------------------------------------

/// Quantisiert einen Gradienten blockweise auf `bits` mit einer
/// Zweierpotenz-Skala je Block (Whitepaper Anhang B.6.2, NITI).
///
/// Liefert die Werte und den Shift je Block. Die Skala folgt aus dem
/// Betragsmaximum des Blocks, deshalb **kann per Konstruktion nichts
/// sättigen**; der Schaden sitzt am unteren Ende, wo kleine Beträge zu
/// null werden. Gemessen: 7,5 % der Werte im Median, im Maximum 59 %.
/// Dass das Verfahren trotzdem trägt, ist gemessen und nicht angenommen.
///
/// `bits` ist ein Parameter und keine Konstante: Die Messung hat gezeigt,
/// dass acht genügen, aber die Aussage gilt für ein Modell und eine
/// Lernrate.
pub fn quantisiere_block(g: &[Grad], blockgroesse: usize, bits: u8) -> (Vec<i8>, Vec<u8>) {
    assert!(blockgroesse > 0, "quantisiere_block: Blockgroesse muss > 0 sein");
    assert!((2..=8).contains(&bits), "quantisiere_block: bits ausserhalb 2..=8");
    let qmax = (1i64 << (bits - 1)) - 1;

    let mut werte = Vec::with_capacity(g.len());
    let mut shifts = Vec::with_capacity(g.len().div_ceil(blockgroesse));

    for block in g.chunks(blockgroesse) {
        let absmax = block.iter().map(|v| (*v as i64).abs()).max().unwrap_or(0);
        // Der Shift bringt den größten Betrag gerade unter `qmax`. Bei
        // einem Nullblock ist jede Skala richtig; 0 ist die einzige, die
        // keine Frage aufwirft.
        let shift = if absmax == 0 {
            0u8
        } else {
            let mut s = 0i32;
            while (absmax >> s.max(0)) > qmax && s < 62 {
                s += 1;
            }
            s as u8
        };
        shifts.push(shift);
        for v in block {
            let q = rshift_round_i64(*v as i64, shift);
            werte.push(q.clamp(-qmax - 1, qmax) as i8);
        }
    }
    (werte, shifts)
}

/// Die Umkehrung: aus Werten und Blockshifts wieder ein Gradient.
pub fn entquantisiere_block(q: &[i8], shifts: &[u8], blockgroesse: usize) -> Vec<Grad> {
    let mut out = Vec::with_capacity(q.len());
    for (b, block) in q.chunks(blockgroesse).enumerate() {
        let shift = shifts[b] as u32;
        for v in block {
            // Linksshift, also exakte Multiplikation mit einer
            // Zweierpotenz: rundungsfrei und plattformgleich.
            out.push(clamp_i32((*v as i64) << shift));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Lineare Schicht
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::linear::linear_w8a16`].
///
/// Vorwärts: `y[i] = Σ_j W[i][j] · x[j]`, herunterskaliert von
/// `act_frac + w_shifts[i]` auf `out_frac`.
///
/// Rückwärts, mit `g = dL/dy`:
///
/// ```text
/// dL/dx[j]    = Σ_i g[i] · W[i][j] / 2^w_shifts[i]
/// dL/dW[i][j] = g[i] · x[j]
/// ```
///
/// **Die Zeilenskala steht in `dL/dx` im Nenner**, nicht im Zähler: Im
/// Vorwärtspfad trägt jede Ausgabezeile ihre eigene Skala, und wer sie
/// hier vergisst, gewichtet die Zeilen gegeneinander falsch. Das ist der
/// wahrscheinlichste stille Fehler in dieser Funktion, deshalb steht er
/// hier und nicht nur im Test.
///
/// `dL/dW` bekommt keine Skalierung: Er wird von der Aktualisierung
/// weiterverarbeitet, die ihre eigene kennt (siehe das TRAINING-Konzept,
/// Abschnitt 2).
pub fn linear_backward(
    g: &[Grad],
    x: &[i16],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    g_frac: u8,
    gx_frac: u8,
) -> (Vec<Grad>, Vec<i64>) {
    let out_features = g.len();
    assert_eq!(w.len(), out_features * in_features, "linear_backward: W passt nicht zu g");
    assert_eq!(w_shifts.len(), out_features, "linear_backward: eine Skala je Ausgabezeile");
    assert_eq!(x.len(), in_features, "linear_backward: x passt nicht zu in_features");

    // dL/dx: über die Ausgaben summieren.
    //
    // **Ausgerichtet nach OBEN, dann EINMAL geschoben** (Fund 24, hier
    // wiedergefunden). Die erste Fassung schob jeden Summanden einzeln
    // um `w_shifts[i]` nach rechts und addierte danach. Bei
    // `rshift_round(-6, 4)` ist das null, und wenn jeder einzelne
    // Beitrag null wird, ist die Summe null: Der Test gegen die
    // numerische Ableitung des echten Kernels fand ein `dL/dx` von
    // exakt 0, wo −2 hingehörte.
    //
    // Richtig ist die Ausrichtung gegen den GRÖSSTEN Shift per
    // Linksshift, weil dabei kein Bit verlorengeht, und ein einziger
    // Rechtsshift ganz am Ende. Genau so löst `rmsnorm_i16` dasselbe
    // Problem in der Quadratsumme.
    //
    // Der Akkumulator ist `i128`: Ein Produkt erreicht 2^38, der
    // Linksshift geht bis MAX_FRAC_BITS = 20, und summiert wird über
    // alle Ausgabezeilen. In i64 wäre das ein Überlauf, und der
    // numerische Vertrag verbietet Wrapping ausdrücklich.
    let ref_shift = *w_shifts.iter().max().expect("linear_backward: w_shifts ist leer");
    let mut gx_acc = vec![0i128; in_features];
    for (i, zeile) in w.chunks_exact(in_features).enumerate() {
        let gi = g[i] as i128;
        if gi == 0 {
            continue;
        }
        let align = (ref_shift - w_shifts[i]) as u32;
        for (j, wij) in zeile.iter().enumerate() {
            gx_acc[j] += (gi * (*wij as i128)) << align;
        }
    }
    // Ein Rechtsshift, dann die Skalenanpassung, dann Sättigung: alles
    // genau einmal, ganz am Ende.
    let gx = gx_acc
        .into_iter()
        .map(|a| {
            let geschoben = crate::fixed_point::rshift_round_i128(a, ref_shift as u32);
            let begrenzt = geschoben.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
            clamp_i32(rescale_i64(begrenzt, g_frac, gx_frac))
        })
        .collect();

    // dL/dW: äußeres Produkt, keine Reduktion, kein Überlaufrisiko
    // (i32 × i16 passt mit Abstand in i64).
    let mut gw = Vec::with_capacity(out_features * in_features);
    for gi in g {
        for xj in x {
            gw.push((*gi as i64) * (*xj as i64));
        }
    }
    (gx, gw)
}

// ---------------------------------------------------------------------------
// Softmax
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::softmax::softmax_int`].
///
/// Vorwärts liefert `p` als Festkommazahl mit `frac_bits`.
///
/// ```text
/// dL/dz[i] = p[i] · (g[i] − Σ_j g[j] · p[j])
/// ```
///
/// **Die Summe wird einmal gebildet und dann geteilt**, nicht je Element
/// neu: Das spart nicht nur Arbeit, es macht das Ergebnis auch
/// unabhängig davon, in welcher Reihenfolge die Elemente abgearbeitet
/// werden. Die Summe selbst ist eine exakte i64-Addition und damit
/// ohnehin ordnungsfrei.
pub fn softmax_backward(g: &[Grad], p: &[Grad], frac_bits: u8) -> Vec<Grad> {
    assert_eq!(g.len(), p.len(), "softmax_backward: g und p muessen gleich lang sein");

    let mut summe: i64 = 0;
    for (gi, pi) in g.iter().zip(p.iter()) {
        summe += (*gi as i64) * (*pi as i64);
    }
    // `summe` trägt g_frac + frac_bits; für den Vergleich mit g auf
    // g_frac zurückschieben.
    let summe = rshift_round_i64(summe, frac_bits);

    g.iter()
        .zip(p.iter())
        .map(|(gi, pi)| {
            let diff = *gi as i64 - summe;
            clamp_i32(rshift_round_i64(diff * (*pi as i64), frac_bits))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SiLU
// ---------------------------------------------------------------------------

/// Rückwärts zur SiLU-Aktivierung aus [`crate::mlp::mlp_int`].
///
/// Vorwärts wird `silu` über eine LUT ausgewertet. Die Ableitung
/// `silu'(x) = σ(x) · (1 + x · (1 − σ(x)))` ist keine Funktion, die sich
/// aus der Vorwärts-LUT sauber gewinnen ließe: `σ(x) = silu(x)/x` ist bei
/// null undefiniert und in der Umgebung numerisch unbrauchbar. Sie
/// bekommt deshalb **eine eigene LUT**, erzeugt wie die vorhandenen und
/// an `theta_v` gebunden.
///
/// Der Index wird wie im Vorwärtspfad gebildet: Eingang in die feste
/// LUT-Domäne reskalieren, dann [`crate::integer_math::lut_lookup`], das
/// am Rand deterministisch sättigt.
#[allow(clippy::too_many_arguments)]
pub fn silu_backward(
    g: &[Grad],
    x: &[i16],
    grad_lut: &[i16],
    x_frac: u8,
    lut_in_frac: u8,
    lut_offset: i16,
    lut_out_frac: u8,
    g_frac: u8,
    out_frac: u8,
) -> Vec<Grad> {
    assert_eq!(g.len(), x.len(), "silu_backward: g und x muessen gleich lang sein");
    g.iter()
        .zip(x.iter())
        .map(|(gi, xi)| {
            let dom = crate::fixed_point::rescale(*xi as i32, x_frac, lut_in_frac);
            let ableitung =
                crate::integer_math::lut_lookup(clamp_i16_sat(dom), grad_lut, 0, lut_offset) as i64;
            let prod = (*gi as i64) * ableitung;
            clamp_i32(rescale_i64(prod, g_frac + lut_out_frac, out_frac))
        })
        .collect()
}

/// Sättigt auf i16, ohne `clamp_i16` zu umgehen: Der LUT-Index darf
/// nicht wrappen, und ein zu großer Eingang gehört an den LUT-Rand.
fn clamp_i16_sat(v: i32) -> i16 {
    crate::fixed_point::clamp_i16(v)
}

// ---------------------------------------------------------------------------
// RMSNorm
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::rmsnorm::rmsnorm_i16`].
///
/// Vorwärts: `y_i = x_i · r · γ_i` mit `r = 1/sqrt(mean(x²))`, wobei `r`
/// aus der rsqrt-LUT stammt.
///
/// ```text
/// dL/dγ_i = g_i · x_i · r
/// dL/dx_j = r · γ_j · g_j − (r³ · x_j / n) · Σ_i g_i · γ_i · x_i
/// ```
///
/// **Der zweite Term ist der, den man vergisst.** Er kommt daher, dass
/// `r` von *allen* `x` abhängt: Eine Änderung an `x_j` verschiebt die
/// Norm und damit jede Ausgabe. Ohne ihn ist der Gradient nicht falsch
/// skaliert, sondern schlicht ein anderer, und der Fehler wächst mit der
/// Länge des Vektors.
///
/// `r` wird **nicht neu berechnet**, sondern vom Vorwärtspfad
/// übernommen (`r_wert`, `r_frac`): Ein zweiter LUT-Nachschlag könnte
/// einen anderen Index treffen, und dann leitete dieser Kernel eine
/// andere Funktion ab als die, die gerechnet wurde.
#[allow(clippy::too_many_arguments)]
pub fn rmsnorm_backward(
    g: &[Grad],
    x: &[i16],
    gamma: &[i8],
    gamma_shifts: &[u8],
    r_wert: i32,
    r_frac: u8,
    inv_n_q20: i64,
    g_frac: u8,
    gx_frac: u8,
) -> (Vec<Grad>, Vec<i64>) {
    let n = x.len();
    assert_eq!(g.len(), n, "rmsnorm_backward: g und x muessen gleich lang sein");
    assert_eq!(gamma.len(), n, "rmsnorm_backward: gamma passt nicht");
    assert_eq!(gamma_shifts.len(), n, "rmsnorm_backward: ein Shift je Gamma");

    let r = r_wert as i64;

    // Σ g_i · γ_i · x_i. Die Gamma-Skala je Element muss heraus, sonst
    // gewichtet die Summe die Kanäle falsch.
    //
    // **Wieder nach oben ausgerichtet**, aus demselben Grund wie in
    // [`linear_backward`]: Jeden Summanden einzeln zu runden löscht die
    // kleinen aus, und bei einer breiten Shift-Spanne bleibt von der
    // Summe nur der gröbste Kanal übrig. Das ist Fund 24, wörtlich.
    let ref_gshift = *gamma_shifts.iter().max().expect("rmsnorm_backward: gamma_shifts ist leer");
    let mut s_acc: i128 = 0;
    for i in 0..n {
        let term = (g[i] as i128) * (gamma[i] as i128) * (x[i] as i128);
        s_acc += term << ((ref_gshift - gamma_shifts[i]) as u32);
    }
    let s = crate::fixed_point::rshift_round_i128(s_acc, ref_gshift as u32)
        .clamp(i64::MIN as i128, i64::MAX as i128) as i64;

    // dL/dγ: g_i · x_i · r, ohne Reduktion.
    let mut ggamma = Vec::with_capacity(n);
    for i in 0..n {
        ggamma.push(rshift_round_i64((g[i] as i64) * (x[i] as i64) * r, r_frac));
    }

    // dL/dx
    let mut gx = Vec::with_capacity(n);
    for j in 0..n {
        // Erster Term: r · γ_j · g_j
        let t1 = rshift_round_i64(
            rshift_round_i64((g[j] as i64) * (gamma[j] as i64), gamma_shifts[j]) * r,
            r_frac,
        );
        // Zweiter Term: (r³ · x_j / n) · s
        //
        // Gestaffelt geschoben statt am Stück: r³ trägt 3·r_frac, und
        // ein Produkt aus r³, x_j und s überschritte sonst auch i64.
        let r2 = rshift_round_i64(r * r, r_frac);
        let r3 = rshift_round_i64(r2 * r, r_frac);
        let mit_x = rshift_round_i64(r3 * (x[j] as i64), r_frac);
        let mit_n = (mit_x * inv_n_q20) >> 20;
        let t2 = rshift_round_i64(mit_n * s, r_frac);
        gx.push(clamp_i32(rescale_i64(t1 - t2, g_frac, gx_frac)));
    }
    (gx, ggamma)
}

// ---------------------------------------------------------------------------
// RoPE
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::rope::rotate_half_split_i16`].
///
/// Vorwärts ist die Rotation eines Paares `(x0, x1)` um den Winkel θ:
///
/// ```text
/// y0 = x0·cos − x1·sin
/// y1 = x1·cos + x0·sin
/// ```
///
/// Die Jacobi-Matrix dieser Abbildung ist die Rotationsmatrix selbst,
/// und **eine Rotationsmatrix ist orthogonal**: Ihre Transponierte ist
/// ihre Inverse, also die Drehung um −θ. Daraus folgt:
///
/// ```text
/// gx0 = g0·cos + g1·sin
/// gx1 = g1·cos − g0·sin
/// ```
///
/// Nur die Vorzeichen wandern, `sin` wird **nicht** negiert und keine
/// neue LUT gebraucht. Wer stattdessen `sin` negiert und die Formel
/// unverändert lässt, bekommt dasselbe Ergebnis; wer beides tut, dreht
/// in die falsche Richtung, und der Fehler ist in einer Zahlenprobe
/// kaum zu sehen. Deshalb steht die Herleitung hier.
pub fn rope_backward(g: &[Grad], cos_row: &[i16], sin_row: &[i16], frac_bits: u8) -> Vec<Grad> {
    let half = g.len() / 2;
    assert_eq!(g.len(), 2 * half, "rope_backward: Länge muss gerade sein");
    assert_eq!(cos_row.len(), half, "rope_backward: cos_row-Länge muss head_dim/2 sein");
    assert_eq!(sin_row.len(), half, "rope_backward: sin_row-Länge muss head_dim/2 sein");

    let mut out = vec![0i32; g.len()];
    for j in 0..half {
        let cos = cos_row[j] as i64;
        let sin = sin_row[j] as i64;
        let g0 = g[j] as i64;
        let g1 = g[j + half] as i64;
        // Erst das volle Produkt, dann ein Rechtsshift: Wer je Summand
        // schiebt, verliert die kleinen Beiträge (siehe
        // [`linear_backward`]).
        out[j] = clamp_i32(rshift_round_i64(g0 * cos + g1 * sin, frac_bits));
        out[j + half] = clamp_i32(rshift_round_i64(g1 * cos - g0 * sin, frac_bits));
    }
    out
}

// ---------------------------------------------------------------------------
// Attention
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::attention::attention_int`], für **eine**
/// Abfrageposition.
///
/// Vorwärts, mit `s_j = (q·k_j)·score_mult >> score_shift`,
/// `p = softmax(s)` und `out = Σ_j p_j·v_j >> prob_frac`:
///
/// ```text
/// dL/dv_j = g · p_j
/// dL/dp_j = g · v_j                (Summe über die Kopf-Dimension)
/// dL/ds   = softmax_backward(dL/dp, p)
/// dL/dq   = Σ_j dL/ds_j · k_j · score_mult >> score_shift
/// dL/dk_j = dL/ds_j · q · score_mult >> score_shift
/// ```
///
/// **Maskierte Positionen bekommen null**, und zwar ausdrücklich statt
/// nebenbei: Vorwärts bekommen sie `i32::MIN` und damit `p ≈ 0`, aber
/// „ungefähr null" ist im Rückwärtspass kein Argument. Ein Gradient auf
/// eine Position, die nie gelesen wurde, wäre ein Leck über die
/// Kausalitätsgrenze hinweg.
///
/// Die Funktion setzt [`softmax_backward`] ein und rechnet sonst nur
/// Produkte und Summen; sie ist die Zusammensetzung, als die sie im
/// Konzept beschrieben ist.
#[allow(clippy::too_many_arguments)]
pub fn attention_backward(
    g: &[Grad],
    q: &[i16],
    k: &[Vec<i16>],
    v: &[Vec<i16>],
    p: &[Grad],
    mask: &[bool],
    score_mult: i64,
    score_shift: u8,
    prob_frac_bits: u8,
) -> (Vec<Grad>, Vec<Vec<Grad>>, Vec<Vec<Grad>>) {
    let kv_len = k.len();
    let head_dim = q.len();
    assert_eq!(v.len(), kv_len, "attention_backward: k und v ungleich lang");
    assert_eq!(p.len(), kv_len, "attention_backward: p passt nicht zu k");
    assert_eq!(mask.len(), kv_len, "attention_backward: mask passt nicht zu k");
    assert_eq!(g.len(), head_dim, "attention_backward: g passt nicht zu q");

    // dL/dv_j = g · p_j, und dL/dp_j = Σ_d g_d · v_j[d].
    let mut gv = Vec::with_capacity(kv_len);
    let mut gp = Vec::with_capacity(kv_len);
    for j in 0..kv_len {
        if !mask[j] {
            gv.push(vec![0i32; head_dim]);
            gp.push(0i32);
            continue;
        }
        let mut zeile = Vec::with_capacity(head_dim);
        let mut acc: i64 = 0;
        for d in 0..head_dim {
            zeile.push(clamp_i32(rshift_round_i64(
                (g[d] as i64) * (p[j] as i64),
                prob_frac_bits,
            )));
            acc += (g[d] as i64) * (v[j][d] as i64);
        }
        gv.push(zeile);
        gp.push(clamp_i32(rshift_round_i64(acc, prob_frac_bits)));
    }

    let gs = softmax_backward(&gp, p, prob_frac_bits);

    // dL/dq und dL/dk. Die Skalierung `score_mult >> score_shift` ist
    // dieselbe wie vorwärts (Fund 19: Q15-Multiplikation statt Shift,
    // weil der Shift nur für gerade Zweierpotenzen stimmt).
    let mut gq_acc = vec![0i64; head_dim];
    let mut gk = Vec::with_capacity(kv_len);
    for j in 0..kv_len {
        if !mask[j] || gs[j] == 0 {
            gk.push(vec![0i32; head_dim]);
            continue;
        }
        let gsj = gs[j] as i64;
        let mut zeile = Vec::with_capacity(head_dim);
        for d in 0..head_dim {
            gq_acc[d] += gsj * (k[j][d] as i64);
            zeile.push(clamp_i32(rshift_round_i64(
                gsj * (q[d] as i64) * score_mult,
                score_shift,
            )));
        }
        gk.push(zeile);
    }
    // Ein Shift ganz am Ende, nicht je Summand.
    let gq = gq_acc
        .into_iter()
        .map(|a| clamp_i32(rshift_round_i64(a * score_mult, score_shift)))
        .collect();

    (gq, gk, gv)
}

// ---------------------------------------------------------------------------
// Embedding
// ---------------------------------------------------------------------------

/// Rückwärts zum Embedding-Nachschlag am Anfang von `forward_token`.
///
/// Vorwärts wird genau eine Zeile der Tabelle gelesen; rückwärts landet
/// der ganze Gradient in genau dieser Zeile. Alle anderen bleiben null.
///
/// **Akkumulierend, nicht setzend.** Kommt ein Token in einer Sequenz
/// mehrfach vor, muss sich sein Gradient addieren. Wer hier zuweist,
/// behält den letzten Vorkommen und verliert alle davor, und das fällt
/// bei seltenen Token nie auf und bei häufigen als langsames Lernen.
pub fn embedding_backward_akkumulieren(
    ziel: &mut [i64],
    vocab_size: usize,
    token_id: usize,
    g: &[Grad],
) {
    let hidden = g.len();
    assert_eq!(ziel.len(), vocab_size * hidden, "embedding_backward: Zielgröße passt nicht");
    assert!(token_id < vocab_size, "embedding_backward: token_id außerhalb des Vokabulars");
    let start = token_id * hidden;
    for (d, gd) in g.iter().enumerate() {
        ziel[start + d] += *gd as i64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numerische Ableitung **des echten Vorwärtskernels**.
    ///
    /// Der eigentliche Prüfmaßstab dieses Moduls. Eine gegen die Formel
    /// geprüfte Ableitung sagt nur, dass zwei Menschen dieselbe Formel
    /// gelesen haben; sie fällt nicht auf, wenn der Vorwärtspfad etwas
    /// anderes rechnet als die Formel behauptet. Der zentrale Differenz-
    /// quotient auf dem echten Kernel fällt darauf sehr wohl auf.
    ///
    /// `h` in Einheiten des Eingangsrasters: kleiner geht nicht, denn
    /// der Eingang ist ganzzahlig.
    fn numerisch<F>(x: &[i16], j: usize, h: i16, mut f: F) -> f64
    where
        F: FnMut(&[i16]) -> f64,
    {
        let mut plus = x.to_vec();
        let mut minus = x.to_vec();
        plus[j] = plus[j].saturating_add(h);
        minus[j] = minus[j].saturating_sub(h);
        (f(&plus) - f(&minus)) / (2.0 * h as f64)
    }

    // ---- Übertragungsform -------------------------------------------

    /// Der Digest der Blockquantisierung: Skala aus dem Betragsmaximum,
    /// also **kann nichts sättigen**. Das ist keine Feinheit, sondern
    /// der Grund, warum in der Messung die Sättigung bei null lag und
    /// der Schaden stattdessen unten sitzt.
    #[test]
    fn blockquantisierung_saettigt_nie() {
        let g: Vec<Grad> = (0..256).map(|i| (i * i * 7 - 30000) as Grad).collect();
        let (q, shifts) = quantisiere_block(&g, 64, 8);
        assert_eq!(q.len(), g.len());
        assert_eq!(shifts.len(), 4);
        assert!(
            q.iter().all(|v| *v > i8::MIN),
            "ein Wert liegt am Sättigungsrand, die Skalenwahl greift nicht"
        );
    }

    /// Hin und zurück darf den Betrag nicht vergrößern und das
    /// Vorzeichen nicht drehen: Beides wäre ein Gradient, der in eine
    /// andere Richtung zeigt als der echte.
    #[test]
    fn blockquantisierung_bleibt_richtungstreu() {
        let g: Vec<Grad> = (0..128i32).map(|i| (i - 64) * 137).collect();
        let (q, shifts) = quantisiere_block(&g, 32, 8);
        let zurueck = entquantisiere_block(&q, &shifts, 32);
        for (a, b) in g.iter().zip(zurueck.iter()) {
            if *a != 0 && *b != 0 {
                assert_eq!(a.signum(), b.signum(), "Vorzeichen gedreht: {a} -> {b}");
            }
        }
    }

    /// **Die Auslöschung, gemessen statt behauptet.** Kleine Beträge
    /// werden zu null, und das ist die stille Hälfte des Problems: Die
    /// Sättigung oben gibt es nicht, die Auslöschung unten schon.
    #[test]
    fn blockquantisierung_loescht_kleine_werte_aus() {
        // Ein großer Ausreißer zwingt die Blockskala nach oben.
        let mut g: Vec<Grad> = vec![3; 64];
        g[0] = 1_000_000;
        let (q, shifts) = quantisiere_block(&g, 64, 8);
        let zurueck = entquantisiere_block(&q, &shifts, 64);
        let ausgeloescht = g
            .iter()
            .zip(zurueck.iter())
            .filter(|(a, b)| **a != 0 && **b == 0)
            .count();
        assert!(
            ausgeloescht > 50,
            "erwartet: der Ausreißer löscht den Rest aus, gemessen {ausgeloescht}"
        );
    }

    // ---- Lineare Schicht --------------------------------------------

    /// Gegen die numerische Ableitung des echten `linear_w8a16`.
    #[test]
    fn linear_backward_trifft_die_numerische_ableitung() {
        let in_f = 8usize;
        let out_f = 4usize;
        let x: Vec<i16> = (0..in_f).map(|i| (i as i16 * 37 - 100) * 8).collect();
        let w: Vec<i8> = (0..in_f * out_f).map(|i| (((i * 13) % 97) as i32 - 48) as i8).collect();
        let shifts = vec![4u8; out_f];
        let (act_frac, out_frac) = (6u8, 6u8);

        // Skalarer Verlust: Σ y_i · c_i, damit dL/dy = c bekannt ist.
        let c: Vec<Grad> = vec![3, -5, 2, 7];
        let verlust = |xx: &[i16]| -> f64 {
            let y = crate::linear::linear_w8a16(xx, &w, in_f, &shifts, act_frac, out_frac);
            y.iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        let (gx, gw) = linear_backward(&c, &x, &w, in_f, &shifts, out_frac, out_frac);
        assert_eq!(gw.len(), in_f * out_f);

        for (j, gxj) in gx.iter().enumerate() {
            let num = numerisch(&x, j, 4, verlust);
            let ana = *gxj as f64;
            // Die Toleranz folgt aus dem Raster: Der Eingang ist
            // ganzzahlig, die Ausgabe auch, und beide runden.
            let abweichung = (num - ana).abs();
            let bezug = num.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.25,
                "Kanal {j}: numerisch {num:.2}, analytisch {ana:.2}"
            );
        }
    }

    /// `dL/dW` ist das äußere Produkt, ohne Reduktion: exakt prüfbar.
    #[test]
    fn linear_backward_liefert_das_aeussere_produkt() {
        let x: Vec<i16> = vec![3, -7, 11];
        let g: Vec<Grad> = vec![5, -2];
        let w = vec![0i8; 6];
        let (_, gw) = linear_backward(&g, &x, &w, 3, &[0, 0], 0, 0);
        assert_eq!(gw, vec![15, -35, 55, -6, 14, -22]);
    }

    // ---- Softmax -----------------------------------------------------

    /// Gegen die numerische Ableitung des echten `softmax_int`.
    #[test]
    fn softmax_backward_trifft_die_numerische_ableitung() {
        let frac = 12u8;
        let lut: Vec<i16> = (0..128)
            .map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16)
            .collect();
        let logits: Vec<i32> = vec![900, -400, 1500, 200];
        let c: Vec<Grad> = vec![2, -3, 5, 1];

        let verlust = |zz: &[i32]| -> f64 {
            let p = crate::softmax::softmax_int(zz, &lut, 8, frac);
            p.iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        let p = crate::softmax::softmax_int(&logits, &lut, 8, frac);
        let gz = softmax_backward(&c, &p, frac);

        for j in 0..logits.len() {
            let mut plus = logits.clone();
            let mut minus = logits.clone();
            plus[j] += 64;
            minus[j] -= 64;
            let num = (verlust(&plus) - verlust(&minus)) / 128.0;
            let ana = gz[j] as f64;
            // **Absoluter Boden neben der relativen Schranke.** Bei
            // Logit 1 liegt p praktisch bei null, die numerische
            // Ableitung ist exakt 0 und die analytische ein LSB. Ein
            // relativer Vergleich macht daraus 100 % Abweichung, obwohl
            // beide dasselbe sagen: nichts. Zwei LSB Spielraum, mehr
            // nicht.
            let abweichung = (num - ana).abs();
            let bezug = num.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.30,
                "Logit {j}: numerisch {num:.3}, analytisch {ana:.3}"
            );
        }
    }

    /// **Die Erhaltungsgröße der Softmax-Ableitung.** Ein konstanter
    /// eingehender Gradient darf nichts bewirken: Softmax ist invariant
    /// gegen eine gemeinsame Verschiebung aller Logits, also muss die
    /// Ableitung in dieser Richtung null sein. Das prüft die Formel
    /// schärfer als jeder Zahlenvergleich.
    #[test]
    fn ein_konstanter_gradient_bewirkt_nichts() {
        let frac = 12u8;
        // **Muss sich zu 2^frac summieren.** Meine erste Fassung nahm
        // 1024+2048+1024+2048 = 6144 statt 4096 und war damit keine
        // Softmax-Ausgabe; der Test schlug zu Recht fehl, nur nicht aus
        // dem Grund, den er prüfen sollte.
        let p: Vec<Grad> = vec![1 << 10, 1 << 10, 1 << 10, 1 << 10]; // Summe = 2^12
        let g: Vec<Grad> = vec![7; 4];
        let gz = softmax_backward(&g, &p, frac);
        for (i, v) in gz.iter().enumerate() {
            assert!(v.abs() <= 1, "Kanal {i}: {v}, erwartet ~0");
        }
    }

    // ---- RMSNorm -----------------------------------------------------

    /// **Der zweite Term ist nicht optional.** Ohne ihn ist der Gradient
    /// eine andere Funktion, und der Unterschied wächst mit der Länge des
    /// Vektors. Der Test hält fest, dass er wirkt.
    #[test]
    fn rmsnorm_backward_traegt_den_normierungsterm() {
        let n = 16usize;
        let x: Vec<i16> = (0..n).map(|i| i as i16 * 91 - 700).collect();
        let gamma: Vec<i8> = vec![64; n];
        let gshifts = vec![6u8; n];
        let g: Vec<Grad> = (0..n).map(|i| (i as i32 % 5) - 2).collect();
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;

        let (gx, ggamma) = rmsnorm_backward(&g, &x, &gamma, &gshifts, 1 << 12, 12, inv_n, 8, 8);
        assert_eq!(gx.len(), n);
        assert_eq!(ggamma.len(), n);

        // Ohne den zweiten Term wäre dL/dx exakt proportional zu g.
        // Mit ihm ist es das nicht, und genau das ist zu zeigen.
        let proportional = gx
            .iter()
            .zip(g.iter())
            .all(|(a, b)| *b == 0 || (*a as f64 / *b as f64 - gx[0] as f64 / g[0] as f64).abs() < 1e-9);
        assert!(!proportional, "der Normierungsterm fehlt: dL/dx ist proportional zu g");
    }

    /// Ein Nullgradient darf nichts erzeugen. Klingt banal, fängt aber
    /// jeden Vorzeichen- und Offsetfehler in beiden Termen.
    #[test]
    fn ohne_eingehenden_gradienten_kein_ausgehender() {
        let n = 8usize;
        let x: Vec<i16> = (0..n).map(|i| i as i16 * 13 + 5).collect();
        let gamma = vec![32i8; n];
        let gshifts = vec![5u8; n];
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;
        let (gx, gg) = rmsnorm_backward(&vec![0; n], &x, &gamma, &gshifts, 1 << 12, 12, inv_n, 8, 8);
        assert!(gx.iter().all(|v| *v == 0), "{gx:?}");
        assert!(gg.iter().all(|v| *v == 0), "{gg:?}");
    }

    // ---- Determinismus -----------------------------------------------

    /// **Der Vertrag aus `dot.rs`, hier noch einmal.** Die Reduktionen im
    /// Rückwärtspass laufen exakt in i64, also ist jede
    /// Summationsreihenfolge dieselbe Zahl. Ohne diese Eigenschaft wäre
    /// verifizierbares Training unmöglich: Zwei Miner mit verschiedener
    /// Parallelisierung bekämen verschiedene Gradienten.
    #[test]
    fn die_reihenfolge_der_reduktion_aendert_nichts() {
        let in_f = 64usize;
        let out_f = 8usize;
        let x: Vec<i16> = (0..in_f).map(|i| ((i * 37) % 1000) as i16 - 500).collect();
        let w: Vec<i8> = (0..in_f * out_f)
            .map(|i| i32::try_from((i * 29) % 251).unwrap_or(0).wrapping_sub(125) as i8)
            .collect();
        let shifts = vec![5u8; out_f];
        let g: Vec<Grad> = (0..out_f).map(|i| (i as i32 * 17) - 60).collect();

        let (vorwaerts, _) = linear_backward(&g, &x, &w, in_f, &shifts, 8, 8);

        // Dieselbe Rechnung mit umgekehrter Reihenfolge der Ausgaben.
        let mut g_rev = g.clone();
        g_rev.reverse();
        let mut w_rev: Vec<i8> = Vec::with_capacity(w.len());
        for zeile in w.chunks_exact(in_f).rev() {
            w_rev.extend_from_slice(zeile);
        }
        let (rueckwaerts, _) = linear_backward(&g_rev, &x, &w_rev, in_f, &shifts, 8, 8);

        assert_eq!(vorwaerts, rueckwaerts, "die Reduktionsreihenfolge wirkt");
    }

    /// Zweimal dasselbe muss zweimal dasselbe sein. Der billigste Test
    /// des Moduls und der, dessen Ausfall am teuersten wäre.
    #[test]
    fn zweimal_gerechnet_ist_zweimal_dasselbe() {
        let g: Vec<Grad> = (0..97i32).map(|i| (i * 31 % 511) - 255).collect();
        let a = quantisiere_block(&g, 16, 8);
        let b = quantisiere_block(&g, 16, 8);
        assert_eq!(a, b);
    }

    /// **Der Fehler von 2026-08-22, als Test festgehalten.**
    ///
    /// Die erste Fassung von [`linear_backward`] schob jeden Summanden
    /// einzeln nach rechts und addierte danach. Bei kleinen Produkten
    /// rundet jeder Summand für sich auf null, und die Summe ist null,
    /// obwohl der wahre Gradient es nicht ist.
    ///
    /// Der Test baut genau diesen Fall: viele kleine Beiträge, deren
    /// Einzelrundung null ergäbe, deren Summe aber deutlich davon
    /// abweicht. Er ist die Gegenprobe zur Behebung, nicht ihre
    /// Wiederholung.
    #[test]
    fn viele_kleine_beitraege_gehen_nicht_verloren() {
        let in_f = 4usize;
        let out_f = 64usize;
        // Jedes Produkt ist 1 · 1 = 1, der Shift ist 5: einzeln
        // gerundet wäre jeder Beitrag null.
        let x = vec![0i16; in_f];
        let w = vec![1i8; in_f * out_f];
        let shifts = vec![5u8; out_f];
        let g: Vec<Grad> = vec![1; out_f];

        let (gx, _) = linear_backward(&g, &x, &w, in_f, &shifts, 0, 0);

        // Wahr: 64 Beiträge à 1, geteilt durch 2^5 = 2.
        let erwartet = (out_f as i32) >> 5;
        for (j, v) in gx.iter().enumerate() {
            assert_eq!(
                *v, erwartet,
                "Kanal {j}: {v} statt {erwartet}. Einzeln gerundet käme hier 0 heraus."
            );
        }
    }

    /// Dasselbe für die Summe in [`rmsnorm_backward`]: Eine breite
    /// Shift-Spanne darf die feinskalierten Kanäle nicht auslöschen.
    /// Das ist Fund 24 in der Rückwärtsrichtung.
    #[test]
    fn breite_gamma_spanne_loescht_keine_kanaele_aus() {
        let n = 32usize;
        // Ein Kanal grob (Shift 2), der Rest fein (Shift 12): Ohne
        // Ausrichtung nach oben trüge nur der grobe Kanal zur Summe bei.
        let mut gshifts = vec![12u8; n];
        gshifts[0] = 2;
        // **Kräftige Werte, damit der Unterschied das Ausgaberaster
        // erreicht.** Eine erste Fassung nahm x = 100 und g = 1; der
        // Unterschied entstand in der Summe und ging in der Skalierung
        // danach wieder unter. Der Test wäre grün geworden, ohne die
        // Eigenschaft zu prüfen, und das ist schlimmer als rot.
        let x: Vec<i16> = vec![30000; n];
        let gamma: Vec<i8> = vec![127; n];
        let g: Vec<Grad> = vec![64; n];
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;

        let (mit_feinen, _) =
            rmsnorm_backward(&g, &x, &gamma, &gshifts, 1 << 12, 12, inv_n, 8, 8);

        // Dieselbe Rechnung, aber die feinen Kanäle auf null gesetzt:
        // Wenn sie nichts beitrügen, käme dasselbe heraus.
        let mut g_nur_grob = vec![0; n];
        g_nur_grob[0] = 64;
        let (nur_grob, _) =
            rmsnorm_backward(&g_nur_grob, &x, &gamma, &gshifts, 1 << 12, 12, inv_n, 8, 8);

        assert_ne!(
            mit_feinen, nur_grob,
            "die feinskalierten Kanäle tragen nichts bei: Fund 24 in der Rückwärtsrichtung"
        );
    }


    // ---- RoPE --------------------------------------------------------

    /// **Die Rotation ist orthogonal, also ist ihr Rückwärtspass ihre
    /// Umkehrung.** Vorwärts drehen, rückwärts zurückdrehen, und man
    /// steht wieder am Anfang. Das prüft die Vorzeichen schärfer als
    /// jeder Zahlenvergleich: Ein vertauschtes Vorzeichen dreht in die
    /// falsche Richtung und kommt nicht zurück.
    #[test]
    fn rope_rueckwaerts_dreht_die_drehung_zurueck() {
        let frac = 14u8;
        let half = 4usize;
        // Ein Winkel je Paar, wie im Vorwärtspfad.
        let winkel = [0.3f64, 0.7, 1.1, 2.5];
        let cos: Vec<i16> = winkel.iter().map(|a| (a.cos() * (1 << frac) as f64).round() as i16).collect();
        let sin: Vec<i16> = winkel.iter().map(|a| (a.sin() * (1 << frac) as f64).round() as i16).collect();

        let x: Vec<i16> = vec![3000, -1200, 800, 2500, -900, 1700, -2200, 400];
        let gedreht = crate::rope::rotate_half_split_i16(&x, &cos, &sin, frac);
        let g: Vec<Grad> = gedreht.iter().map(|v| *v as Grad).collect();
        let zurueck = rope_backward(&g, &cos, &sin, frac);

        for j in 0..2 * half {
            let abw = (zurueck[j] - x[j] as i32).abs();
            assert!(
                abw <= 4,
                "Kanal {j}: {} statt {} (Abweichung {abw})",
                zurueck[j], x[j]
            );
        }
    }

    /// Bei Winkel null ist die Drehung die Identität, vorwärts wie
    /// rückwärts. Fängt jeden Offsetfehler in der Paarbildung.
    #[test]
    fn rope_bei_winkel_null_ist_identitaet() {
        let frac = 14u8;
        let eins = 1i16 << frac;
        let cos = vec![eins; 3];
        let sin = vec![0i16; 3];
        let g: Vec<Grad> = vec![5, -9, 13, 21, -3, 7];
        let out = rope_backward(&g, &cos, &sin, frac);
        assert_eq!(out, g);
    }

    // ---- Attention ----------------------------------------------------

    /// **Maskierte Positionen bekommen exakt null.** Vorwärts sind sie
    /// „ungefähr null", und das ist im Rückwärtspass kein Argument: Ein
    /// Gradient auf eine Position, die nie gelesen wurde, wäre ein Leck
    /// über die Kausalitätsgrenze.
    #[test]
    fn maskierte_positionen_bekommen_keinen_gradienten() {
        let head_dim = 4usize;
        let q: Vec<i16> = vec![100, -50, 25, 75];
        let k: Vec<Vec<i16>> = vec![vec![10, 20, 30, 40], vec![50, 60, 70, 80], vec![1, 2, 3, 4]];
        let v: Vec<Vec<i16>> = vec![vec![5; head_dim], vec![7; head_dim], vec![11; head_dim]];
        let p: Vec<Grad> = vec![1 << 7, 1 << 7, 0];
        let mask = vec![true, true, false];
        let g: Vec<Grad> = vec![3, -4, 5, -6];

        let (_, gk, gv) = attention_backward(&g, &q, &k, &v, &p, &mask, 1 << 15, 15, 8);

        assert!(gk[2].iter().all(|x| *x == 0), "gk der maskierten Position: {:?}", gk[2]);
        assert!(gv[2].iter().all(|x| *x == 0), "gv der maskierten Position: {:?}", gv[2]);
        // Und die unmaskierten sind es nicht.
        assert!(gv[0].iter().any(|x| *x != 0), "gv[0] ist null, der Test misst nichts");
    }

    /// Ohne eingehenden Gradienten kein ausgehender, über alle drei
    /// Ausgänge.
    #[test]
    fn attention_ohne_gradient_bleibt_still() {
        let q: Vec<i16> = vec![10, 20];
        let k: Vec<Vec<i16>> = vec![vec![1, 2], vec![3, 4]];
        let v: Vec<Vec<i16>> = vec![vec![5, 6], vec![7, 8]];
        let p: Vec<Grad> = vec![1 << 7, 1 << 7];
        let mask = vec![true, true];
        let (gq, gk, gv) = attention_backward(&[0, 0], &q, &k, &v, &p, &mask, 1 << 15, 15, 8);
        assert!(gq.iter().all(|x| *x == 0), "{gq:?}");
        assert!(gk.iter().all(|z| z.iter().all(|x| *x == 0)));
        assert!(gv.iter().all(|z| z.iter().all(|x| *x == 0)));
    }

    /// Gegen die numerische Ableitung des echten `attention_int`, in der
    /// Richtung, die am leichtesten falsch wird: `dL/dv`.
    #[test]
    fn attention_backward_trifft_die_numerische_ableitung_nach_v() {
        let frac = 8u8;
        let lut: Vec<i16> = (0..129)
            .map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16)
            .collect();
        let q = vec![vec![64i16, 32]];
        let k = vec![vec![64i16, 0], vec![0i16, 64]];
        let v = vec![vec![1000i16, -400], vec![-600i16, 900]];
        let mask = vec![vec![true, true]];
        let c: Vec<Grad> = vec![3, -2];

        let verlust = |vv: &Vec<Vec<i16>>| -> f64 {
            let out = crate::attention::attention_int(
                &q, &k, vv, &mask, 1 << 15, 15, &lut, 0, frac);
            out[0].iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        // p wie im Vorwärtspfad, damit derselbe Punkt abgeleitet wird.
        let scores: Vec<i32> = (0..2)
            .map(|j| {
                let s = crate::attention::dot_int(&q[0], &k[j]) * (1 << 15);
                crate::fixed_point::rshift_round_i64(s, 15) as i32
            })
            .collect();
        let p = crate::softmax::softmax_int(&scores, &lut, 0, frac);
        let (_, _, gv) = attention_backward(
            &c, &q[0], &k, &v, &p, &mask[0], 1 << 15, 15, frac);

        for j in 0..2 {
            for d in 0..2 {
                let mut plus = v.clone();
                let mut minus = v.clone();
                plus[j][d] += 64;
                minus[j][d] -= 64;
                let num = (verlust(&plus) - verlust(&minus)) / 128.0;
                let ana = gv[j][d] as f64;
                let abw = (num - ana).abs();
                let bezug = num.abs().max(ana.abs()).max(1.0);
                assert!(
                    abw <= 2.0 || abw / bezug < 0.30,
                    "v[{j}][{d}]: numerisch {num:.3}, analytisch {ana:.3}"
                );
            }
        }
    }

    // ---- Embedding ----------------------------------------------------

    /// **Akkumulierend, nicht setzend.** Kommt ein Token zweimal vor,
    /// muss sich sein Gradient addieren. Wer zuweist, behält das letzte
    /// Vorkommen; das fällt bei seltenen Token nie auf und bei häufigen
    /// als langsames Lernen.
    #[test]
    fn ein_token_zweimal_addiert_sich() {
        let vocab = 5usize;
        let hidden = 3usize;
        let mut ziel = vec![0i64; vocab * hidden];
        embedding_backward_akkumulieren(&mut ziel, vocab, 2, &[10, -20, 30]);
        embedding_backward_akkumulieren(&mut ziel, vocab, 2, &[1, 2, 3]);
        assert_eq!(&ziel[6..9], &[11, -18, 33]);
        // Alle anderen Zeilen bleiben unberührt.
        assert!(ziel[..6].iter().all(|v| *v == 0));
        assert!(ziel[9..].iter().all(|v| *v == 0));
    }


    // ---- SiLU ---------------------------------------------------------

    /// Baut die Ableitungs-LUT wie `calibrate/src/luts.py`.
    ///
    /// Bewusst hier nachgebaut und nicht aus einer Datei gelesen: Der
    /// Test soll den Kernel prüfen und nicht die Verfügbarkeit eines
    /// Artefakts. Weicht die Erzeugung eines Tages ab, fällt das im
    /// Konformitätslauf auf, nicht hier, und das ist die richtige
    /// Arbeitsteilung.
    fn silu_grad_lut(min: i32, max: i32, in_frac: u8, out_frac: u8) -> Vec<i16> {
        let in_scale = (1 << in_frac) as f64;
        let out_scale = (1 << out_frac) as f64;
        (min..=max)
            .map(|x| {
                let xf = x as f64 / in_scale;
                let s = 1.0 / (1.0 + (-xf).exp());
                (s * (1.0 + xf * (1.0 - s)) * out_scale).round() as i16
            })
            .collect()
    }

    /// Gegen die numerische Ableitung der **Vorwärts-LUT**, also gegen
    /// das, was `mlp_int` tatsächlich rechnet.
    #[test]
    fn silu_backward_trifft_die_ableitung_der_vorwaerts_lut() {
        let (in_frac, out_frac) = (6u8, 12u8);
        let (min, max) = (-256i32, 256i32);
        let offset = -min as i16;
        let grad_lut = silu_grad_lut(min, max, in_frac, out_frac);

        let vor: Vec<i16> = (min..=max)
            .map(|x| {
                let xf = x as f64 / (1 << in_frac) as f64;
                (xf / (1.0 + (-xf).exp()) * (1 << out_frac) as f64).round() as i16
            })
            .collect();

        for x in [-128i16, -64, 0, 64, 128, 154] {
            let idx = (x as i32 - min) as usize;
            let num = (vor[idx + 1] as f64 - vor[idx - 1] as f64) / 2.0
                * (1 << in_frac) as f64
                / (1 << out_frac) as f64;

            // Ein Gradient von 1 auf der Ausgangsskala: dann ist das
            // Ergebnis die Ableitung selbst.
            let g: Vec<Grad> = vec![1 << out_frac];
            let out = silu_backward(
                &g, &[x], &grad_lut, in_frac, in_frac, offset, out_frac,
                out_frac, out_frac,
            );
            let ana = out[0] as f64 / (1 << out_frac) as f64;
            assert!(
                (num - ana).abs() < 0.02,
                "x = {}: numerisch {num:.5}, Kernel {ana:.5}",
                x as f64 / (1 << in_frac) as f64
            );
        }
    }

    /// **Die Ableitung überschwingt über eins.** silu' erreicht rund
    /// 1,10 bei x ≈ 2,36 und fällt links auf etwa −0,10. Wer den
    /// Ausgangsbereich wie bei SiLU selbst wählt, sättigt genau dort,
    /// wo die Ableitung am größten ist. Der Test hält die Eigenschaft
    /// fest, damit sie bei einer Änderung der spec auffällt.
    #[test]
    fn die_silu_ableitung_ueberschwingt_und_wird_negativ() {
        let lut = silu_grad_lut(-256, 256, 6, 12);
        let max = *lut.iter().max().unwrap() as f64 / 4096.0;
        let min = *lut.iter().min().unwrap() as f64 / 4096.0;
        assert!(max > 1.05 && max < 1.15, "Maximum {max:.4}, erwartet ~1,10");
        assert!(min < -0.05 && min > -0.15, "Minimum {min:.4}, erwartet ~-0,10");
    }

    /// Ohne eingehenden Gradienten kein ausgehender.
    #[test]
    fn silu_ohne_gradient_bleibt_still() {
        let lut = silu_grad_lut(-256, 256, 6, 12);
        let out = silu_backward(&[0, 0, 0], &[10, -20, 30], &lut, 6, 6, 256, 12, 12, 12);
        assert!(out.iter().all(|v| *v == 0), "{out:?}");
    }

}
