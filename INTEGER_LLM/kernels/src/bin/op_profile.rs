//! Operationsprofil: Wo geht die Zeit im Forward-Pass hin?
//!
//! Misst jede Kernoperation einzeln bei den Formen eines echten Modells
//! und rechnet hoch, welchen Anteil sie an einem Token hat.
//!
//! **Anlass (2026-08-19):** Der Durchsatz-Benchmark zeigte, dass das
//! SIMD-Backend keinen Vorteil bringt. Bevor jemand Kernel optimiert,
//! muss feststehen, welche Operation überhaupt zählt — sonst wird die
//! falsche vektorisiert. Genau das ist hier passiert: Vektorisiert sind
//! Softmax, RoPE und Attention; `linear_w8a16` und `rmsnorm` delegieren
//! an die Referenz.
//!
//! Die Zeitmessung läuft in f64 — Messpfad, nicht Inferenzpfad. Die
//! gemessenen Kernel selbst rechnen ausschließlich ganzzahlig.
//!
//! Usage: `op_profile [modell]`  (0.5b | 7b, Vorgabe 0.5b)

use std::time::Instant;

use integer_llm_kernels::linear::linear_w8a16_pc;
use integer_llm_kernels::rmsnorm::rmsnorm_i16;
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_kernels::softmax::softmax_int;

/// Modellgeometrie, wie sie in `config.json` steht.
struct Geometrie {
    name: &'static str,
    hidden: usize,
    intermediate: usize,
    layers: usize,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    vocab: usize,
}

const QWEN_05B: Geometrie = Geometrie {
    name: "Qwen2.5-0.5B",
    hidden: 896,
    intermediate: 4864,
    layers: 24,
    heads: 14,
    kv_heads: 2,
    head_dim: 64,
    vocab: 151936,
};

const QWEN_7B: Geometrie = Geometrie {
    name: "Qwen2.5-7B",
    hidden: 3584,
    intermediate: 18944,
    layers: 28,
    heads: 28,
    kv_heads: 4,
    head_dim: 128,
    vocab: 152064,
};

/// Mittlere Laufzeit einer Operation in Mikrosekunden.
fn miss<F: FnMut()>(wiederholungen: usize, mut f: F) -> f64 {
    // Aufwärmen: der erste Lauf zahlt Cache-Misses, die nicht zur
    // Operation gehören.
    for _ in 0..3 {
        f();
    }
    let t0 = Instant::now();
    for _ in 0..wiederholungen {
        f();
    }
    t0.elapsed().as_secs_f64() * 1e6 / wiederholungen as f64
}

/// Ein linearer Layer mit `n_in` Eingängen und `n_out` Ausgängen.
fn miss_linear(n_in: usize, n_out: usize, wdh: usize) -> f64 {
    let x: Vec<i16> = (0..n_in).map(|i| ((i % 251) as i16) - 125).collect();
    let w: Vec<Vec<i8>> = (0..n_out)
        .map(|r| {
            (0..n_in)
                .map(|c| (((r + c) % 251) as i8).wrapping_sub(125))
                .collect()
        })
        .collect();
    let w_shifts: Vec<u8> = vec![7; n_out];
    let out_frac: Vec<u8> = vec![8; n_out];
    let mut out = Vec::with_capacity(n_out);
    miss(wdh, || {
        out = linear_w8a16_pc(&x, &w, &w_shifts, 8, &out_frac);
    })
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "0.5b".to_string());
    let g = if arg.contains("7") { QWEN_7B } else { QWEN_05B };

    println!("=== Operationsprofil: {} ===", g.name);
    println!(
        "hidden {}, intermediate {}, {} Layer, {} Heads ({} KV), head_dim {}, vocab {}",
        g.hidden, g.intermediate, g.layers, g.heads, g.kv_heads, g.head_dim, g.vocab
    );
    println!();

    let q_out = g.heads * g.head_dim;
    let kv_out = g.kv_heads * g.head_dim;

    // Je Layer: q, k, v, o, gate, up, down.
    let wdh = if g.hidden > 2000 { 3 } else { 20 };
    let q = miss_linear(g.hidden, q_out, wdh);
    let k = miss_linear(g.hidden, kv_out, wdh);
    let v = miss_linear(g.hidden, kv_out, wdh);
    let o = miss_linear(q_out, g.hidden, wdh);
    let gate = miss_linear(g.hidden, g.intermediate, wdh);
    let up = miss_linear(g.hidden, g.intermediate, wdh);
    let down = miss_linear(g.intermediate, g.hidden, wdh);
    let linear_je_layer = q + k + v + o + gate + up + down;

    // RMSNorm: zweimal je Layer.
    let x: Vec<i16> = (0..g.hidden).map(|i| ((i % 251) as i16) - 125).collect();
    let gamma: Vec<i8> = vec![64; g.hidden];
    let x_shifts: Vec<u8> = vec![8; g.hidden];
    let gamma_shifts: Vec<u8> = vec![6; g.hidden];
    let rsqrt_lut: Vec<i16> = (1..=1024).map(|i| (32767 / i) as i16).collect();
    let inv_n_q20: i64 = (1i64 << 20) / g.hidden as i64;
    let mut norm_out = Vec::new();
    let rms = miss(wdh * 10, || {
        norm_out = rmsnorm_i16(
            &x, &x_shifts, &gamma, &gamma_shifts, &rsqrt_lut, 10, 15, inv_n_q20, 8,
        );
    });
    let rms_je_layer = 2.0 * rms;

    // RoPE arbeitet je Head. Je Layer sind es `heads` Q-Heads plus
    // `kv_heads` K-Heads.
    let mut rope_buf: Vec<i16> = (0..g.head_dim).map(|i| ((i % 251) as i16) - 125).collect();
    let cos: Vec<i16> = vec![32767; g.head_dim / 2];
    let sin: Vec<i16> = vec![0; g.head_dim / 2];
    let rope = miss(wdh * 10, || {
        rope_buf = rotate_half_split_i16(&rope_buf, &cos, &sin, 15);
    });
    let rope_je_layer = (g.heads + g.kv_heads) as f64 * rope;

    // Softmax: je Head über die bisherige Sequenz. Wir messen bei einer
    // kurzen Sequenz — das ist der guenstige Fall fuer Softmax und
    // damit der konservative fuer diese Auswertung.
    let seq = 32usize;
    let scores: Vec<i32> = (0..seq).map(|i| (i as i32 % 97) - 48).collect();
    let exp_lut: Vec<i16> = (0..4096i32).map(|i| (32767 - i * 8).max(0) as i16).collect();
    let mut sm_out = Vec::new();
    let sm = miss(wdh * 10, || {
        sm_out = softmax_int(&scores, &exp_lut, 8, 15);
    });
    let softmax_je_layer = g.heads as f64 * sm;

    // LM-Head: einmal je Token.
    let lm = miss_linear(g.hidden, g.vocab, 1.max(wdh / 4));

    let je_token_linear = linear_je_layer * g.layers as f64 + lm;
    let je_token_rms = rms_je_layer * g.layers as f64;
    let je_token_rope = rope_je_layer * g.layers as f64;
    let je_token_softmax = softmax_je_layer * g.layers as f64;
    let gesamt = je_token_linear + je_token_rms + je_token_rope + je_token_softmax;

    println!("{:<28} {:>12} {:>10}", "Operation", "µs/Token", "Anteil");
    println!("{}", "-".repeat(52));
    for (name, wert) in [
        ("linear_w8a16 (alle Layer)", linear_je_layer * g.layers as f64),
        ("linear_w8a16 (LM-Head)", lm),
        ("rmsnorm", je_token_rms),
        ("rope", je_token_rope),
        ("softmax", je_token_softmax),
    ] {
        println!("{:<28} {:>12.1} {:>9.2}%", name, wert, 100.0 * wert / gesamt);
    }
    println!("{}", "-".repeat(52));
    println!("{:<28} {:>12.1} {:>9.2}%", "SUMME", gesamt, 100.0);
    println!();
    println!(
        "Davon vektorisiert (softmax + rope):  {:.2} %",
        100.0 * (je_token_rope + je_token_softmax) / gesamt
    );
    println!(
        "Davon an die Referenz delegiert:      {:.2} %",
        100.0 * (je_token_linear + je_token_rms) / gesamt
    );
    println!();
    println!("Aufschlüsselung je Layer (µs):");
    println!("  q {:.1}  k {:.1}  v {:.1}  o {:.1}", q, k, v, o);
    println!("  gate {:.1}  up {:.1}  down {:.1}", gate, up, down);
}
