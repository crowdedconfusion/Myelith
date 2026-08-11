//! Diagnose-Binary: Layer-0-Zwischenwerte fuer Position 0 ausgeben.
//!
//! Dient dem Abgleich mit der HF-Referenz (layer_probe_hf.py), um die
//! erste divergierende Stufe im Forward-Pass zu finden. Kein Teil des
//! Auslieferungspfads.

use integer_llm_kernels::attention::attention_int;
use integer_llm_kernels::fixed_point::{clamp_i16, rescale};
use integer_llm_kernels::linear::{add_bias_i16, linear_w8a16};
use integer_llm_kernels::mlp::mlp_int;
use integer_llm_kernels::rmsnorm::rmsnorm_i16;
use integer_llm_kernels::rope::rotate_pairs_i16;
use integer_llm_runtime::loader::load_model;

fn summary(name: &str, v: &[i16], frac: u8) {
    let absmax = v.iter().map(|x| x.abs() as i32).max().unwrap_or(0);
    let head: Vec<String> = v.iter().take(8).map(|x| x.to_string()).collect();
    let real_scale = 2f64.powi(-(frac as i32));
    println!(
        "{}: absmax={} ({:.4} real), frac={}, erste8=[{}]",
        name,
        absmax,
        absmax as f64 * real_scale,
        frac,
        head.join(", ")
    );
}

fn to_vec_vec(qt: &integer_llm_runtime::model::QTensor) -> Vec<Vec<i8>> {
    let mut out = Vec::with_capacity(qt.rows());
    for r in 0..qt.rows() {
        out.push(qt.row(r));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: layer_probe <artifact_dir> <token_id>");
        std::process::exit(1);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let token_id: usize = args[2].parse().expect("token_id");

    let model = load_model(&dir).expect("Modell-Ladung fehlgeschlagen");
    let cfg = &model.config;
    let layer = &model.layers[0];
    let sc = &layer.scales;

    println!("=== Layer-0-Probe, Token {}, theta_v-Konfig ===", token_id);
    println!(
        "kv_cache_frac={}, score_frac={}, silu_in_frac={}, silu_off={}, silu_out={}, final_residual_frac={}",
        cfg.kv_cache_frac_bits, cfg.score_frac_bits,
        cfg.silu_in_frac, cfg.silu_lut_offset, cfg.silu_out_frac,
        model.final_residual_frac
    );
    println!(
        "Layer-Skalen: norm_attn={}, q={}, k={}, v={}, attn_out={}, norm_mlp={}, gate={}, up={}, down_in={}, res_in={}, res_mid={}",
        sc.norm_attn_frac, sc.q_frac, sc.k_frac, sc.v_frac, sc.attn_out_frac,
        sc.norm_mlp_frac, sc.gate_frac, sc.up_frac, sc.down_in_frac,
        sc.residual_in_frac, sc.residual_mid_frac
    );

    // S0: Embedding (Per-Channel-Skala der Token-Zeile, theta_v 0.7.0)
    let emb = model.embedding_table.row(token_id);
    let emb_shift = model.embedding_table.shifts[token_id];
    let hidden: Vec<i16> = emb
        .iter()
        .map(|v| clamp_i16(rescale(*v as i32, emb_shift, sc.residual_in_frac)))
        .collect();
    println!("embedding_shift={}", emb_shift);
    summary("S0 hidden(embed)", &hidden, sc.residual_in_frac);

    // S1: Pre-Attention RMSNorm
    let norm_hidden = rmsnorm_i16(
        &hidden,
        &layer.input_layernorm_gamma.data,
        &layer.input_layernorm_gamma.shifts,
        &model.rsqrt_lut,
        cfg.rsqrt_input_shift,
        cfg.rsqrt_output_frac,
        model.inv_n_q20,
        sc.norm_attn_frac,
    );
    println!("gamma_in_shifts (erste 4) = {:?}", &layer.input_layernorm_gamma.shifts[..4]);
    summary("S1 norm_hidden", &norm_hidden, sc.norm_attn_frac);

    // S2: Q/K/V + Bias
    let mut q_flat = linear_w8a16(&norm_hidden, &to_vec_vec(&layer.q_proj), &layer.q_proj.shifts, sc.norm_attn_frac, sc.q_frac);
    let mut k_flat = linear_w8a16(&norm_hidden, &to_vec_vec(&layer.k_proj), &layer.k_proj.shifts, sc.norm_attn_frac, sc.k_frac);
    let mut v_flat = linear_w8a16(&norm_hidden, &to_vec_vec(&layer.v_proj), &layer.v_proj.shifts, sc.norm_attn_frac, sc.v_frac);
    if let Some(qb) = &layer.q_bias { add_bias_i16(&mut q_flat, &qb.data, &qb.shifts, sc.q_frac); }
    if let Some(kb) = &layer.k_bias { add_bias_i16(&mut k_flat, &kb.data, &kb.shifts, sc.k_frac); }
    if let Some(vb) = &layer.v_bias { add_bias_i16(&mut v_flat, &vb.data, &vb.shifts, sc.v_frac); }
    summary("S2 q_flat", &q_flat, sc.q_frac);
    summary("S2 k_flat", &k_flat, sc.k_frac);
    summary("S2 v_flat", &v_flat, sc.v_frac);

    // S3: Attention an Position 0 (nur Selbst-Attention)
    let head_dim = model.head_dim;
    let q0 = q_flat[0..head_dim].to_vec();
    let k0 = k_flat[0..head_dim].to_vec();
    let v0 = v_flat[0..head_dim].to_vec();
    let score_shift = (sc.q_frac as u16 + sc.k_frac as u16).saturating_sub(cfg.score_frac_bits as u16) as u8;
    println!("score_shift={}", score_shift);
    let head_out = attention_int(
        &[q0.clone()], &[k0.clone()], &[v0.clone()],
        &[vec![true]],
        score_shift, &model.exp_lut, 0, cfg.prob_frac_bits,
    );
    summary("S3 head_out(h0)", &head_out[0], sc.v_frac);
    // Bei Einzelposition muss head_out ~ v0 sein (softmax([x]) = 1).
    let diff: i32 = head_out[0].iter().zip(v0.iter())
        .map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
    println!("S3 max|head_out - v0| = {}", diff);

    // S4: RoPE-Einfluss auf q/k (Sanity: Norm-Erhaltung)
    let idx = 0usize % model.cos_lut.len();
    let q_rot = rotate_pairs_i16(&q0, model.cos_lut[idx], model.sin_lut[idx], cfg.rope_frac_bits);
    println!(
        "S4 rope q0: vor=[{}, {}] nach=[{}, {}]",
        q0[0], q0[1], q_rot[0], q_rot[1]
    );

    // S5: Komplette Attention + O-Projektion + Residual (wie forward_layer)
    let mut attn_out = vec![0i16; model.hidden_size];
    let group_size = model.num_heads / model.num_kv_heads;
    for h in 0..model.num_heads {
        let kv_h = h / group_size;
        let q_start = h * head_dim;
        let kv_start = kv_h * head_dim;
        let q_seq = vec![q_flat[q_start..q_start + head_dim].to_vec()];
        let k_seq = vec![k_flat[kv_start..kv_start + head_dim].to_vec()];
        let v_seq = vec![v_flat[kv_start..kv_start + head_dim].to_vec()];
        let out = attention_int(&q_seq, &k_seq, &v_seq, &[vec![true]], score_shift, &model.exp_lut, 0, cfg.prob_frac_bits);
        attn_out[q_start..q_start + head_dim].copy_from_slice(&out[0]);
    }
    summary("S5 attn_out(v-Skala)", &attn_out, sc.v_frac);
    if sc.attn_out_frac != sc.v_frac {
        for v in attn_out.iter_mut() {
            *v = clamp_i16(rescale(*v as i32, sc.v_frac, sc.attn_out_frac));
        }
    }
    summary("S5 attn_out(reskaliert)", &attn_out, sc.attn_out_frac);

    // Probe-Approximation: Ausgangs-Segment-Skala = finales Residual-Segment
    // (im echten Forward waere es die Eingangsskala von Layer 1).
    let out_frac = model.final_residual_frac;

    let o_out = linear_w8a16(&attn_out, &to_vec_vec(&layer.o_proj), &layer.o_proj.shifts, sc.attn_out_frac, sc.residual_mid_frac);
    summary("S5 o_out", &o_out, sc.residual_mid_frac);

    let residual: Vec<i16> = hidden.iter().zip(o_out.iter())
        .map(|(a, b)| {
            let h_rescaled = clamp_i16(rescale(*a as i32, sc.residual_in_frac, sc.residual_mid_frac));
            clamp_i16(h_rescaled as i32 + *b as i32)
        }).collect();
    summary("S5 residual(mid)", &residual, sc.residual_mid_frac);

    // S6: MLP
    let norm_residual = rmsnorm_i16(
        &residual,
        &layer.post_attention_layernorm_gamma.data,
        &layer.post_attention_layernorm_gamma.shifts,
        &model.rsqrt_lut,
        cfg.rsqrt_input_shift,
        cfg.rsqrt_output_frac,
        model.inv_n_q20,
        sc.norm_mlp_frac,
    );
    summary("S6 norm_residual", &norm_residual, sc.norm_mlp_frac);

    let mlp_out = mlp_int(
        &norm_residual,
        &to_vec_vec(&layer.gate_proj),
        &to_vec_vec(&layer.up_proj),
        &to_vec_vec(&layer.down_proj),
        &layer.gate_proj.shifts,
        &layer.up_proj.shifts,
        &layer.down_proj.shifts,
        &model.silu_lut,
        sc.norm_mlp_frac,
        sc.gate_frac,
        sc.up_frac,
        sc.down_in_frac,
        cfg.silu_in_frac,
        cfg.silu_lut_offset,
        cfg.silu_out_frac,
        out_frac,
    );
    summary("S6 mlp_out", &mlp_out, out_frac);

    let out: Vec<i16> = residual.iter().zip(mlp_out.iter())
        .map(|(a, b)| {
            let r_rescaled = clamp_i16(rescale(*a as i32, sc.residual_mid_frac, out_frac));
            clamp_i16(r_rescaled as i32 + *b as i32)
        }).collect();
    summary("S7 layer_out", &out, out_frac);
}
