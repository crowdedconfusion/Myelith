//! **Der Rueckwaertspass der rekurrenten Zustandsschicht**, eine Ebene,
//! ein Fenster ab leerem Zustand.
//!
//! # ⚑ Woraus er besteht
//!
//! Der Vorwaertspass ist der der Inferenz
//! ([`IntegerModel::zustandsschicht_mit_spur`]); er hinterlaesst eine
//! [`Zustandsspur`]. Rueckwaerts laeuft dieselbe Kette in umgekehrter
//! Reihenfolge, und jedes Glied ist ein Kern, der fuer sich gegen den
//! echten Vorwaertskern geprueft ist:
//!
//! | Schritt | Kern |
//! |---|---|
//! | `out_proj` | `linear_backward_summierend` |
//! | `silu(z) * normiert` | Produktregel, `silu_backward` |
//! | torgesteuerte Norm (mit Epsilon) | `rmsnorm_backward` |
//! | Rekurrenz, je Wertkopf | `zustandsrueckweg::kopf_rueckwaerts` |
//! | `beta` und Zerfall | `zustandsrueckweg::tore_rueckwaerts` |
//! | Einheitslaenge von `q` und `k` | `rmsnorm_backward` |
//! | Faltung mit SiLU | `faltung::rueckwaerts` |
//! | die vier Projektionen | `linear_backward_summierend` |
//!
//! # ⚑ Die Skala der Gradienten
//!
//! Wie im uebrigen Trainingspfad: Ein Gradient traegt die Bruchbits eines
//! **Busses**. Hereinkommend ist das `bus` (der Gradient nach der Ausgabe
//! von `out_proj`); innen tragen alle Gradienten denselben Bus, denn die
//! Kerne der Schicht leiten nach den **reellen** Werten ab und lassen die
//! Skala des Gradienten unberuehrt. Erst die vier Eingangsprojektionen
//! rechnen auf die Skala des Eingangs um, wie `linear_backward` es ueberall
//! tut. Der Gradient nach dem Eingang kommt also auf `norm_attn_frac`.
//!
//! ⚠️ **Nur int8-Gewichte.** Ternaer gepackte Projektionen der
//! Zustandsschicht haben im Training noch keinen Weg; der Aufruf meldet
//! das mit Namen statt still falsch zu rechnen.

use integer_llm_kernels::backward::{
    linear_backward_summierend, rmsnorm_backward, silu_backward, silu_grad_aus_lut, silu_grad_frac, Grad,
    Normskalen,
};
use integer_llm_kernels::drehung::gradient_zurueckdrehen;
use integer_llm_kernels::fixed_point::{rescale, rshift_round_i64};
use integer_llm_kernels::integer_math::silu_nachschlagen;
use integer_llm_kernels::rmsnorm::{inv_n_q20, Rmsnormspur};
use integer_llm_kernels::trainingsschritt::begrenze;
use integer_llm_kernels::zustandsrueckweg::{kopf_rueckwaerts, tore_rueckwaerts, Kopffolge};

use crate::model::{IntegerModel, QTensor, TransformerLayer, Zustandsschicht, Zustandsspur};

/// **Die Gradienten einer Zustandsschicht** ueber ein Fenster.
///
/// Die Gewichtsgradienten sind die aeusseren Produkte `g * x` wie bei
/// `linear_backward`, je Matrix flach und zeilenweise; die der Vorspanne
/// und der Faltung beziehen sich auf deren **reelle** Werte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zustandsgradienten {
    /// Nach dem normierten Eingang, je Token, auf `norm_attn_frac`.
    pub eingang: Vec<Vec<Grad>>,
    pub in_proj_qkv: Vec<i64>,
    pub in_proj_z: Vec<i64>,
    pub in_proj_b: Vec<i64>,
    pub in_proj_a: Vec<i64>,
    pub out_proj: Vec<i64>,
    /// Die Faltungsgewichte, `kanal * KERN + j`, wie die Matrizen: `g * x`
    /// mit `x` auf `qkv_frac`.
    pub faltung: Vec<i64>,
    /// Nach dem reellen Gamma der torgesteuerten Norm.
    pub norm_gamma: Vec<i64>,
    /// Nach den reellen Vorspannen, je Wertkopf.
    pub dt_bias: Vec<i64>,
    pub exp_a: Vec<i64>,
}

/// **Der Rueckwaertspass einer Zustandsschicht.**
///
/// `d_aus[t]` ist der Gradient nach der Ausgabe von `out_proj` auf dem
/// Bus `bus`; `spur` kommt aus [`IntegerModel::zustandsschicht_mit_spur`]
/// fuer dieselbe Ebene und dieselben Gewichte `zs`. `abstand` ist der Abstand der gehaltenen Zustaende
/// in der Rekurrenz; er aendert keine Zahl, nur Speicher und Zeit.
///
/// # Panics
///
/// Wenn die Ebene nicht rekurrent ist, eine ihrer Projektionen ternaer
/// gepackt ist oder Spur und Gradient nicht zusammenpassen.
pub fn rueckwaerts(
    m: &IntegerModel,
    layer: &TransformerLayer,
    zs: &Zustandsschicht,
    spur: &Zustandsspur,
    d_aus: &[Vec<Grad>],
    bus: u8,
    abstand: usize,
) -> Zustandsgradienten {
    assert!(layer.ist_rekurrent(), "Ebene {} mischt nicht rekurrent", layer.layer_idx);
    let masse = m.zustandsmasse.as_ref().expect("ein rekurrentes Modell traegt Zustandsmasse");
    let sk = &zs.skalen;
    let n = spur.normen.len();
    assert_eq!(d_aus.len(), n, "ein Ausgangsgradient je Token");
    for (name, t) in [
        ("in_proj_qkv", &zs.in_proj_qkv),
        ("in_proj_z", &zs.in_proj_z),
        ("in_proj_b", &zs.in_proj_b),
        ("in_proj_a", &zs.in_proj_a),
        ("out_proj", &zs.out_proj),
    ] {
        assert!(
            !t.ist_ternaer(),
            "Ebene {}: {name} ist ternaer gepackt, und dafuer hat der Rueckwaertspass der \
             Zustandsschicht noch keinen Weg",
            layer.layer_idx
        );
    }
    let (sd, wd) = (masse.schluessel_dim, masse.wert_dim);
    let (sk_koepfe, wk_koepfe) = (masse.schluessel_koepfe, masse.wert_koepfe);
    let k_breite = sd * sk_koepfe;
    let faecher = masse.auffaecherung();
    let faeden = integer_llm_kernels::fadenpool::faeden();

    // --- 1. out_proj
    let mut gw_out = vec![0i64; zs.out_proj.n_elements()];
    let d_getort: Vec<Vec<Grad>> = (0..n)
        .map(|t| projektion_rueckwaerts(&zs.out_proj, &spur.getort[t], sk.norm_aus_frac, &d_aus[t], bus, bus, &mut gw_out))
        .collect();

    // --- 2. `getort = silu(z) * normiert`
    let grad_lut = silu_grad_aus_lut(&m.silu_lut);
    let grad_frac = silu_grad_frac(m.config.silu_in_frac, m.config.silu_out_frac);
    let mut d_normiert: Vec<Vec<Grad>> = Vec::with_capacity(n);
    let mut d_z: Vec<Vec<Grad>> = Vec::with_capacity(n);
    for ((z, normiert), d_get) in spur.z.iter().zip(&spur.normiert).zip(&d_getort) {
        let mut d_akt = Vec::with_capacity(z.len());
        let mut d_nm = Vec::with_capacity(z.len());
        for ((g, &z_i), &n_i) in d_get.iter().zip(z).zip(normiert) {
            let dom = rescale(i32::from(z_i), sk.z_frac, m.config.silu_in_frac);
            let aktiv = silu_nachschlagen(dom, &m.silu_lut, m.config.silu_lut_offset, m.config.silu_in_frac, m.config.silu_out_frac);
            let g = i64::from(*g);
            d_nm.push(begrenze(rshift_round_i64(g * aktiv, m.config.silu_out_frac)));
            d_akt.push(begrenze(rshift_round_i64(g * i64::from(n_i), spur.zwischen_frac)));
        }
        d_z.push(silu_backward(
            &d_akt, z, &grad_lut, sk.z_frac, m.config.silu_in_frac, m.config.silu_lut_offset, grad_frac, bus, bus,
        ));
        d_normiert.push(d_nm);
    }

    // --- 3. die torgesteuerte Norm, je Wertkopf
    let inv_n_wert = inv_n_q20(wd);
    let mut gw_gamma = vec![0i64; wd];
    let d_roh: Vec<Vec<Grad>> = (0..n)
        .map(|t| {
            let mut aus = Vec::with_capacity(wk_koepfe * wd);
            for h in 0..wk_koepfe {
                let x = &spur.roh_aus[t][h * wd..(h + 1) * wd];
                let g = &d_normiert[t][h * wd..(h + 1) * wd];
                let x_shifts = vec![spur.roh_fracs[t][h]; wd];
                let (gx, gg) = norm_rueckwaerts(
                    g, x, &x_shifts, &zs.norm_gamma.data, &zs.norm_gamma.shifts, spur.normiert_spur[t][h],
                    inv_n_wert, bus,
                );
                for (s, v) in gw_gamma.iter_mut().zip(gg) {
                    *s += v;
                }
                aus.extend(gx);
            }
            aus
        })
        .collect();

    // --- 4. die Rekurrenz und 5. ihre Tore, je Wertkopf, verteilt
    let tore = zs.tore(masse, m);
    let je_kopf = integer_llm_kernels::fadenpool::verteilen(wk_koepfe, faeden, |h| {
        let s_kopf = h / faecher;
        let werte: Vec<_> = (0..n).map(|t| tore.werte(h, spur.b_roh[t][h], spur.a_roh[t][h])).collect();
        let folge = Kopffolge {
            schluessel_dim: sd,
            wert_dim: wd,
            q: (0..n).map(|t| spur.q_norm[t][s_kopf * sd..(s_kopf + 1) * sd].to_vec()).collect(),
            k: (0..n).map(|t| spur.k_norm[t][s_kopf * sd..(s_kopf + 1) * sd].to_vec()).collect(),
            v: (0..n).map(|t| spur.v_wert[t][h * wd..(h + 1) * wd].to_vec()).collect(),
            g: werte.iter().map(|w| w.g).collect(),
            beta: werte.iter().map(|w| w.beta).collect(),
        };
        let d: Vec<Vec<Grad>> = (0..n).map(|t| d_roh[t][h * wd..(h + 1) * wd].to_vec()).collect();
        let gr = kopf_rueckwaerts(&folge, &d, abstand);
        let torgr: Vec<_> = (0..n).map(|t| tore_rueckwaerts(&tore, h, &werte[t], gr.beta[t], gr.g[t])).collect();
        (gr, torgr)
    });

    // Zusammenfuehren: `q` und `k` je Schluesselkopf ueber seine Wertkoepfe
    // summiert, `v` je Wertkopf, die Tore je Token und Wertkopf.
    let mut d_q = vec![vec![0i64; k_breite]; n];
    let mut d_k = vec![vec![0i64; k_breite]; n];
    let mut d_v = vec![vec![0 as Grad; wk_koepfe * wd]; n];
    let mut d_b = vec![vec![0 as Grad; wk_koepfe]; n];
    let mut d_a = vec![vec![0 as Grad; wk_koepfe]; n];
    let mut gw_dt = vec![0i64; wk_koepfe];
    let mut gw_ea = vec![0i64; wk_koepfe];
    for (h, (gr, torgr)) in je_kopf.iter().enumerate() {
        let s_kopf = h / faecher;
        for t in 0..n {
            for (ziel, &g) in d_q[t][s_kopf * sd..(s_kopf + 1) * sd].iter_mut().zip(&gr.q[t]) {
                *ziel += i64::from(g);
            }
            for (ziel, &g) in d_k[t][s_kopf * sd..(s_kopf + 1) * sd].iter_mut().zip(&gr.k[t]) {
                *ziel += i64::from(g);
            }
            d_v[t][h * wd..(h + 1) * wd].copy_from_slice(&gr.v[t]);
            d_b[t][h] = torgr[t].b;
            d_a[t][h] = torgr[t].a;
            gw_dt[h] += i64::from(torgr[t].dt_bias);
            gw_ea[h] += i64::from(torgr[t].exp_a);
        }
    }

    // --- 6. Einheitslaenge von `q` und `k`, je Schluesselkopf; `v` geht
    //     durch die Umskalierung unveraendert, denn sie aendert den
    //     reellen Wert nicht.
    let einsen = (vec![1i8; sd], vec![0u8; sd]);
    let d_gefaltet: Vec<Vec<Grad>> = (0..n)
        .map(|t| {
            let mut aus = Vec::with_capacity(spur.gefaltet[t].len());
            for (teil, d_teil, spuren, inv_n) in [
                (0usize, &d_q[t], &spur.q_normspur[t], (sd as i64) << 20),
                (1, &d_k[t], &spur.k_normspur[t], 1i64 << 20),
            ] {
                for kopf in 0..sk_koepfe {
                    let basis = teil * k_breite + kopf * sd;
                    let x = &spur.gefaltet[t][basis..basis + sd];
                    let x_shifts = &sk.konv_fracs[basis..basis + sd];
                    let g: Vec<Grad> = d_teil[kopf * sd..(kopf + 1) * sd].iter().map(|&v| begrenze(v)).collect();
                    aus.extend(norm_rueckwaerts(&g, x, x_shifts, &einsen.0, &einsen.1, spuren[kopf], inv_n, bus).0);
                }
            }
            aus.extend_from_slice(&d_v[t]);
            aus
        })
        .collect();

    // --- 7. die Faltung mit SiLU
    let qkv_zeilen: Vec<&[i16]> = spur.qkv.iter().map(Vec::as_slice).collect();
    let fg = integer_llm_kernels::faltung::rueckwaerts(
        &qkv_zeilen,
        &zs.conv1d.data,
        &zs.conv1d.shifts,
        sk.qkv_frac,
        &m.sigmoid_lut,
        m.sigmoid_versatz,
        m.sigmoid_ein_frac,
        m.sigmoid_aus_frac,
        &d_gefaltet,
    );

    // --- 8. die vier Eingangsprojektionen; ihr Eingang bekommt die Summe.
    let ein_frac = layer.scales.norm_attn_frac;
    let mut gw_qkv = vec![0i64; zs.in_proj_qkv.n_elements()];
    let mut gw_z = vec![0i64; zs.in_proj_z.n_elements()];
    let mut gw_b = vec![0i64; zs.in_proj_b.n_elements()];
    let mut gw_a = vec![0i64; zs.in_proj_a.n_elements()];
    let eingang: Vec<Vec<Grad>> = (0..n)
        .map(|t| {
            let x = &spur.normen[t];
            let teile = [
                projektion_rueckwaerts(&zs.in_proj_qkv, x, ein_frac, &fg.ein[t], bus, ein_frac, &mut gw_qkv),
                projektion_rueckwaerts(&zs.in_proj_z, x, ein_frac, &d_z[t], bus, ein_frac, &mut gw_z),
                projektion_rueckwaerts(&zs.in_proj_b, x, ein_frac, &d_b[t], bus, ein_frac, &mut gw_b),
                projektion_rueckwaerts(&zs.in_proj_a, x, ein_frac, &d_a[t], bus, ein_frac, &mut gw_a),
            ];
            (0..x.len()).map(|j| begrenze(teile.iter().map(|g| i64::from(g[j])).sum())).collect()
        })
        .collect();

    Zustandsgradienten {
        eingang,
        in_proj_qkv: gw_qkv,
        in_proj_z: gw_z,
        in_proj_b: gw_b,
        in_proj_a: gw_a,
        out_proj: gw_out,
        faltung: fg.gewicht,
        norm_gamma: gw_gamma,
        dt_bias: gw_dt,
        exp_a: gw_ea,
    }
}

/// **Eine Projektion rueckwaerts**: `dL/dW` in `summe` addiert, `dL/dx`
/// zurueck, auf `gx_frac` und, wenn die Matrix gedreht liest, zurueckgedreht.
///
/// ⚑ **Dieselbe Eingabe wie vorwaerts** ([`QTensor::eingang`]): `dL/dW`
/// gilt dem, was die Matrix gelesen hat.
fn projektion_rueckwaerts(
    t: &QTensor,
    x: &[i16],
    x_frac: u8,
    g: &[Grad],
    g_frac: u8,
    gx_frac: u8,
    summe: &mut [i64],
) -> Vec<Grad> {
    let (ein, _) = t.eingang(x, x_frac);
    let gx = linear_backward_summierend(g, &ein, &t.data, t.cols(), &t.shifts, g_frac, gx_frac, summe);
    match &t.drehung {
        Some(d) => {
            let breit: Vec<i64> = gx.iter().map(|&v| i64::from(v)).collect();
            gradient_zurueckdrehen(&breit, &d.vorzeichen).into_iter().map(begrenze).collect()
        }
        None => gx,
    }
}

/// **Eine RMSNorm rueckwaerts** mit der Spur des Vorwaertspasses; ein Kopf,
/// der vorwaerts null war, hat kein `r` und bekommt keinen Gradienten.
#[allow(clippy::too_many_arguments)]
fn norm_rueckwaerts(
    g: &[Grad],
    x: &[i16],
    x_shifts: &[u8],
    gamma: &[i8],
    gamma_shifts: &[u8],
    spur: Rmsnormspur,
    inv_n: i64,
    bus: u8,
) -> (Vec<Grad>, Vec<i64>) {
    let Rmsnormspur::Wert { r, norm_frac, ref_shift } = spur else {
        return (vec![0; x.len()], vec![0; x.len()]);
    };
    rmsnorm_backward(
        g,
        x,
        x_shifts,
        gamma,
        gamma_shifts,
        Normskalen { r, norm_frac, ref_shift, inv_n_q20: inv_n, g_frac: bus, gx_frac: bus },
    )
}
