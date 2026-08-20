//! Attention über MEHRERE Positionen: wo entsteht der Mehrpositionsfehler?
//!
//! **Anlass (2026-08-20, Fahrplanpunkt 12.77).** Die Stufenanalyse in
//! `layer_probe` lief an Position 0 mit leerem KV-Cache — und fand dort
//! jede Operation arithmetisch exakt (q/k/v 0,02 %, gate/up 0,01 %).
//! Gegen das Schema gemessen liegt unser Pfad an Position 0 tatsächlich
//! auf Schema-Niveau (2,08 % gegen 2,07 %), ab Position 1 aber bei rund
//! dem Doppelten.
//!
//! **Position 0 ist der Sonderfall, in dem der Softmax exakt 1 ergibt.**
//! Damit ist der Attention-Ausgang identisch mit `v`, und zwei
//! Fehlerquellen treten gar nicht auf:
//!
//! 1. die **Rundung der Softmax-Gewichte** (`prob_frac_bits`) und ihre
//!    Aufsummierung über die gecachten Positionen,
//! 2. die **Reskalierung des Ausgangs** von `v_frac` auf
//!    `attn_out_frac` (`model.rs`: nur wenn beide verschieden sind).
//!
//! **RoPE gehört zwingend dazu (Instrumentenfehler vom 2026-08-20).** Die
//! erste Fassung dieser Sonde ließ die Positionsrotation weg. Ohne sie
//! sind die Scores über alle Positionen nahezu gleich, der Softmax landet
//! in einem fast gleichverteilten Regime, das im echten Pfad nie vorkommt —
//! gemessen wurden 47–151 %, was mit einer Perplexität von +7,5 % nicht
//! vereinbar ist. Die Selbstprüfung bei n=1 hatte das nicht aufgedeckt,
//! weil sie genau der Fall ist, in dem die fehlende Komponente nichts tut.
//!
//! Diese Sonde rechnet Attention über `n` Positionen einmal ganzzahlig
//! und einmal in Gleitkomma **aus denselben entquantisierten q/k/v nach
//! RoPE** — die Differenz ist damit reine Attention-Arithmetik, ohne
//! Gewichtsquantisierung, ohne Eingangsfehler und ohne RoPE-Rundung.
//! Letztere wird separat ausgewiesen.
//!
//! Kein Teil des Auslieferungspfads.
//!
//! Usage: attn_probe <artifact_dir> <token_id> [<token_id> ...]

use integer_llm_kernels::attention::attention_int;
use integer_llm_kernels::fixed_point::{clamp_i16, inv_sqrt_q15, rescale};
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_runtime::loader::load_model;

/// Entquantisiert einen Ganzzahlvektor auf eine Zweierpotenz-Skala.
fn deq(v: &[i16], frac: u8) -> Vec<f64> {
    let s = 2f64.powi(-(frac as i32));
    v.iter().map(|x| *x as f64 * s).collect()
}

/// Relativer L2 zwischen zwei Gleitkomma-Vektoren, in Prozent.
fn rel_f(a: &[f64], b: &[f64]) -> f64 {
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b.iter()) {
        num += (x - y) * (x - y);
        den += y * y;
    }
    100.0 * (num / den.max(1e-30)).sqrt()
}

/// Relativer L2 zwischen ganzzahliger und Gleitkomma-Fassung.
fn rel(ganz: &[i16], frac: u8, gleit: &[f64]) -> f64 {
    rel_f(&deq(ganz, frac), gleit)
}

/// Softmax über die Scores, gibt (Gewichte, Maximum) zurück.
fn softmax(scores: &[f64]) -> Vec<f64> {
    let m = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = scores.iter().map(|s| (s - m).exp()).collect();
    let summe: f64 = exps.iter().sum();
    exps.iter().map(|e| e / summe).collect()
}

/// Gewichtete Summe der v-Vektoren.
fn weighted_sum(w: &[f64], vs: &[Vec<f64>], hd: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; hd];
    for (p, wp) in w.iter().enumerate() {
        for i in 0..hd {
            out[i] += wp * vs[p][i];
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: attn_probe <artifact_dir> <token_id> [<token_id> ...]");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let tokens: Vec<usize> = args[2..]
        .iter()
        .map(|s| s.parse().expect("token_id"))
        .collect();

    let model = load_model(&dir).expect("Modell-Ladung");
    let cfg = &model.config;
    let layer = &model.layers[0];
    let sc = &layer.scales;
    let hd = model.head_dim;
    let half = hd / 2;
    let n_pos_lut = model.cos_lut.len() / half;

    // q/k/v je Position aus dem echten Pfad holen: Embedding -> Norm ->
    // Projektionen -> RoPE. Bewusst dieselben Kernel wie in `model.rs`.
    // `*_roh` ist vor der Rotation, `qs`/`ks` danach.
    let mut qs: Vec<Vec<i16>> = Vec::new();
    let mut ks: Vec<Vec<i16>> = Vec::new();
    let mut vs: Vec<Vec<i16>> = Vec::new();
    let mut qs_roh: Vec<Vec<i16>> = Vec::new();
    let mut ks_roh: Vec<Vec<i16>> = Vec::new();
    for (pos, &tok) in tokens.iter().enumerate() {
        let emb = model.embedding_table.row(tok);
        let es = model.embedding_table.shifts[tok];
        let hidden: Vec<i16> = emb
            .iter()
            .zip(sc.residual_in_frac.iter())
            .map(|(v, &z)| {
                clamp_i16(integer_llm_kernels::fixed_point::rescale_i64(
                    *v as i64, es, z,
                ) as i32)
            })
            .collect();
        let nh = integer_llm_kernels::rmsnorm::rmsnorm_i16(
            &hidden,
            &sc.residual_in_frac,
            &layer.input_layernorm_gamma.data,
            &layer.input_layernorm_gamma.shifts,
            &model.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            model.inv_n_q20,
            sc.norm_attn_frac,
        );
        let q = integer_llm_kernels::linear::linear_w8a16(
            &nh,
            &(0..layer.q_proj.rows()).map(|r| layer.q_proj.row(r)).collect::<Vec<_>>(),
            &layer.q_proj.shifts,
            sc.norm_attn_frac,
            sc.q_frac,
        );
        let k = integer_llm_kernels::linear::linear_w8a16(
            &nh,
            &(0..layer.k_proj.rows()).map(|r| layer.k_proj.row(r)).collect::<Vec<_>>(),
            &layer.k_proj.shifts,
            sc.norm_attn_frac,
            sc.k_frac,
        );
        let v = integer_llm_kernels::linear::linear_w8a16(
            &nh,
            &(0..layer.v_proj.rows()).map(|r| layer.v_proj.row(r)).collect::<Vec<_>>(),
            &layer.v_proj.shifts,
            sc.norm_attn_frac,
            sc.v_frac,
        );
        // Biases wie im echten Pfad (Qwen2.5 hat q/k/v-Biases).
        let mut q0 = q[0..hd].to_vec();
        let mut k0 = k[0..hd].to_vec();
        let mut v0 = v[0..hd].to_vec();
        if let Some(qb) = &layer.q_bias {
            integer_llm_kernels::linear::add_bias_i16(&mut q0, &qb.data[0..hd], &qb.shifts[0..hd], sc.q_frac);
        }
        if let Some(kb) = &layer.k_bias {
            integer_llm_kernels::linear::add_bias_i16(&mut k0, &kb.data[0..hd], &kb.shifts[0..hd], sc.k_frac);
        }
        if let Some(vb) = &layer.v_bias {
            integer_llm_kernels::linear::add_bias_i16(&mut v0, &vb.data[0..hd], &vb.shifts[0..hd], sc.v_frac);
        }
        qs_roh.push(q0.clone());
        ks_roh.push(k0.clone());

        // RoPE — genau wie `model.rs::forward_layer`.
        let idx = pos % n_pos_lut;
        let cos_row = &model.cos_lut[idx * half..(idx + 1) * half];
        let sin_row = &model.sin_lut[idx * half..(idx + 1) * half];
        qs.push(rotate_half_split_i16(&q0, cos_row, sin_row, cfg.rope_frac_bits));
        ks.push(rotate_half_split_i16(&k0, cos_row, sin_row, cfg.rope_frac_bits));
        vs.push(v0);
    }

    let n = tokens.len();
    println!("Kopf 0, {} Positionen, head_dim={} (mit RoPE)", n, hd);
    println!(
        "Skalen: q_frac={} k_frac={} v_frac={} attn_out_frac={} prob_frac_bits={} rope_frac={}",
        sc.q_frac, sc.k_frac, sc.v_frac, sc.attn_out_frac, cfg.prob_frac_bits, cfg.rope_frac_bits
    );
    println!(
        "        score_frac={} exp_input_frac={} -> exp-Raster 1/{}",
        cfg.score_frac_bits, cfg.exp_input_frac, 1u32 << cfg.exp_input_frac
    );

    let score_mult = inv_sqrt_q15(hd);
    let score_shift = (sc.q_frac as u16 + sc.k_frac as u16 + 15)
        .saturating_sub(cfg.score_frac_bits as u16) as u8;
    let maske: Vec<Vec<bool>> = vec![(0..n).map(|_| true).collect()];
    // lut_shift uebersetzt von der Score-Skala in die exp-LUT-Domaene.
    // Die erste Fassung dieser Sonde uebergab hier 0 statt
    // score_frac_bits - exp_input_frac und rechnete damit exp(-d/256)
    // statt exp(-d/16) — die Gewichte wurden viel zu gleichmaessig.
    let exp_lut_shift = cfg.score_frac_bits.saturating_sub(cfg.exp_input_frac);

    let ganz = attention_int(
        std::slice::from_ref(&qs[n - 1]),
        &ks,
        &vs,
        &maske,
        score_mult,
        score_shift,
        &model.exp_lut,
        exp_lut_shift,
        cfg.prob_frac_bits,
    );

    // Gleitkomma-Gegenrechnung aus DENSELBEN (rotierten) q/k/v.
    let qf = deq(&qs[n - 1], sc.q_frac);
    let vf: Vec<Vec<f64>> = vs.iter().map(|v| deq(v, sc.v_frac)).collect();
    let mut scores = Vec::with_capacity(n);
    for kp in &ks {
        let kf = deq(kp, sc.k_frac);
        let s: f64 = kf.iter().zip(qf.iter()).map(|(a, b)| a * b).sum();
        scores.push(s / (hd as f64).sqrt());
    }
    let w = softmax(&scores);
    let out_f = weighted_sum(&w, &vf, hd);

    // Score-Verteilung ausweisen: nur wenn sie gespreizt ist, misst die
    // Sonde ein Regime, das im echten Pfad vorkommt.
    let smin = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let smax = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let wmax = w.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    println!(
        "Scores: min {:.3} max {:.3} Spanne {:.3} | groesstes Gewicht {:.3} (gleichverteilt waere {:.3})",
        smin, smax, smax - smin, wmax, 1.0 / n as f64
    );

    println!();
    println!("Attention-Ausgang, v-Skala   : {:6.2} %", rel(&ganz[0], sc.v_frac, &out_f));

    // Die Reskalierung, die an Position 0 gar nicht stattfindet.
    if sc.attn_out_frac != sc.v_frac {
        let reskaliert: Vec<i16> = ganz[0]
            .iter()
            .map(|v| clamp_i16(rescale(*v as i32, sc.v_frac, sc.attn_out_frac)))
            .collect();
        println!(
            "Attention-Ausgang, reskaliert : {:6.2} %   (v_frac {} -> attn_out_frac {})",
            rel(&reskaliert, sc.attn_out_frac, &out_f),
            sc.v_frac,
            sc.attn_out_frac
        );
    } else {
        println!("Reskalierung entfaellt (v_frac == attn_out_frac = {})", sc.v_frac);
    }

    // Wieviel kostet allein die Rundung der Wahrscheinlichkeiten?
    let stufe = 1.0 / (1u32 << cfg.prob_frac_bits) as f64;
    let w_q: Vec<f64> = w.iter().map(|x| (x / stufe).round() * stufe).collect();
    let out_q = weighted_sum(&w_q, &vf, hd);
    println!(
        "davon nur Wahrscheinlichkeits-Rundung (1/{}): {:6.2} %",
        1u32 << cfg.prob_frac_bits,
        rel_f(&out_q, &out_f)
    );

    // Was kostet allein das Eingangsraster der exp-LUT (1/2^exp_input_frac)?
    let raster = 2f64.powi(-(cfg.exp_input_frac as i32));
    let m = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let scores_r: Vec<f64> = scores.iter().map(|s| m - ((m - s) / raster).round() * raster).collect();
    let w_r = softmax(&scores_r);
    let out_r = weighted_sum(&w_r, &vf, hd);
    println!(
        "davon nur exp-Eingangsraster (1/{}):          {:6.2} %",
        1u32 << cfg.exp_input_frac,
        rel_f(&out_r, &out_f)
    );

    // Was kostet die RoPE-Rundung selbst? Referenz: Gleitkomma-Rotation
    // der unrotierten q/k mit denselben (entquantisierten) cos/sin-Werten.
    let rs = 2f64.powi(-(cfg.rope_frac_bits as i32));
    let rot_f = |vec: &[i16], frac: u8, pos: usize| -> Vec<f64> {
        let idx = pos % n_pos_lut;
        let x = deq(vec, frac);
        let mut out = vec![0.0f64; hd];
        for j in 0..half {
            let c = model.cos_lut[idx * half + j] as f64 * rs;
            let s = model.sin_lut[idx * half + j] as f64 * rs;
            out[j] = x[j] * c - x[j + half] * s;
            out[j + half] = x[j + half] * c + x[j] * s;
        }
        out
    };
    let qf_ref = rot_f(&qs_roh[n - 1], sc.q_frac, n - 1);
    let mut scores_ref = Vec::with_capacity(n);
    for (p, kp) in ks_roh.iter().enumerate() {
        let kf = rot_f(kp, sc.k_frac, p);
        let s: f64 = kf.iter().zip(qf_ref.iter()).map(|(a, b)| a * b).sum();
        scores_ref.push(s / (hd as f64).sqrt());
    }
    let w_ref = softmax(&scores_ref);
    let out_ref = weighted_sum(&w_ref, &vf, hd);
    println!(
        "RoPE-Rundung allein (q/k)                  : {:6.2} %   (Gesamt mit RoPE-Rundung: {:6.2} %)",
        rel_f(&out_f, &out_ref),
        rel(&ganz[0], sc.v_frac, &out_ref)
    );
}
