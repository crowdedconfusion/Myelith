//! Der Trainingslauf auf einem **Expertengemisch**.
//!
//! # ⚑ Warum das doch geht, entgegen der ersten Einschätzung
//!
//! Die erste Antwort auf „prüf das Training auch gegen MoE" lautete:
//! geht nicht, das Artefakt belegt 29 GB und die Maschine hat 24. **Das
//! war falsch**, und der Grund steht in `model::Gewichtsdaten`: Die
//! Gewichte werden **speicherabgebildet**, nicht in den Heap geladen.
//! Der Ladevorgang liest sie einmal für die Hashprüfung, danach hält
//! das Betriebssystem nur die Seiten, die wirklich angefasst werden.
//!
//! Gemessen: **140 Sekunden** laden, dann zwei Ebenen in 57
//! Millisekunden.
//!
//! # ⚑ Und ein Experte ist ein dichter Block
//!
//! `MoeLayer::experts` ist ein `Vec<DenseMlp>`, und der Vorwärtspass
//! schickt jeden gewählten Experten durch **dieselbe** `mlp_vorwaerts`
//! wie eine dichte Ebene. Damit gilt `schritt_auf_mlp` für einen
//! Experten unverändert.
//!
//! # Was dieser Lauf zeigt, und was nicht
//!
//! **Er zeigt:** Der MLP-Rückwärtspass trägt auf den Gewichten eines
//! echten Expertengemisches (2 048 Eingänge, 768 innere Einheiten,
//! 128 Experten je Ebene) und auf Aktivierungen aus einem echten
//! MoE-Vorwärtspass. Trainiert wird ein Experte, den der Router für
//! dieses Token **wirklich gewählt hat**, nicht ein beliebiger.
//!
//! ⚑ **Er zeigt nicht** den Rückwärtspass durch den **Router** und
//! durch die **Mischung**. Dafür verlangt `moe_backward` Expertenwahl,
//! Gewichte und Expertenausgaben; der Mitschnitt sagt an dieser Stelle
//! ausdrücklich [`Mlpteil::Expertengemisch`], also „nicht
//! aufgezeichnet". Das ist Phase 5 der Trainingsliste.
//!
//! # ⚑ Warum `#[ignore]`
//!
//! Die Hashprüfung von 29 GB kostet gut zwei Minuten und würde den
//! Sammellauf fast verdoppeln. **Ein stiller Sprung wäre schlimmer**
//! (Fund 113), deshalb kein `return` in der Mitte, sondern ein
//! ausdrückliches `--ignored`:
//!
//! ```text
//! cd INTEGER_LLM/runtime && cargo test --test training_moe -- --ignored --nocapture
//! ```

use integer_llm_kernels::optimierer::{Master, Schrittkennung};
use integer_llm_kernels::trainingsschritt::{schritt_auf_mlp, Mlpvorgaben};
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::{Mlpteil, Zwischenwerte};
use integer_llm_runtime::model::{Feedforward, QTensor};

const MODELL: &str = "qwen3-30b-a3b";

fn master_aus_gewicht(t: &QTensor) -> Vec<Master> {
    let in_features = t.shape[1];
    t.data
        .iter()
        .enumerate()
        .map(|(i, w)| (i32::from(*w)) << t.shifts[i / in_features])
        .collect()
}

/// ⚑ **Ein gewählter Experte eines echten Expertengemisches lernt sein Ziel.**
#[test]
#[ignore = "laedt 29 GB Artefakte, rund zwei Minuten; mit --ignored ausfuehren"]
fn ein_gewaehlter_experte_lernt_sein_ziel() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(MODELL);
    assert!(
        dir.exists(),
        "Das MoE-Artefakt fehlt: {dir:?}\n\
         Dieser Lauf prueft das Training auf einem Expertengemisch und kann das ohne nicht."
    );

    let t0 = std::time::Instant::now();
    let m = load_model(&dir).expect("Modell laedt");
    eprintln!(
        "\n=== Trainingslauf auf {MODELL} ===\n  geladen in {:?}: {} Ebenen, hidden {}",
        t0.elapsed(),
        m.num_layers,
        m.hidden_size
    );

    // --- 1. Ein echter Vorwärtspass, mit Routing und Mitschnitt ------
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let (_, befunde) = m.forward_token_mit_routing(9707, 0, &mut cache);
    assert!(!befunde.is_empty(), "es wurde kein einziger Router befragt");

    let mut cache2 = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    let h = m.embed_token(9707);
    let _ = m.run_layers_mit_mitschnitt(h, 0, &mut cache2, 0, m.num_layers, &mut auf);

    // ⚑ **Der Mitschnitt sagt hier ausdrücklich „nicht
    // aufgezeichnet"**, und dieser Test hält das fest: Er ist die
    // einzige Stelle im Baum, die den MoE-Zweig überhaupt erreicht.
    for (i, e) in auf.ebenen().iter().enumerate() {
        assert!(
            matches!(e.mlp, Mlpteil::Expertengemisch),
            "Ebene {i}: der Mitschnitt haelt einen MoE-Block fuer dicht"
        );
        assert!(
            !e.norm_mitte.is_empty(),
            "Ebene {i}: der Eingang des Blocks fehlt trotzdem"
        );
    }
    eprintln!("  Mitschnitt: {} Ebenen, alle als Expertengemisch gemeldet", auf.len());

    // --- 2. Eine Ebene, ein wirklich gewählter Experte ---------------
    let e = m.num_layers / 2;
    let befund = befunde
        .iter()
        .find(|b| b.layer == e)
        .expect("fuer diese Ebene gibt es keinen Routingbefund");
    let experte = *befund.experten.first().expect("kein Experte gewaehlt") as usize;
    let Feedforward::Moe(moe) = &m.layers[e].ffn else {
        panic!("Ebene {e} ist kein Expertengemisch");
    };
    let mlp = &moe.experts[experte];
    eprintln!(
        "  Ebene {e}: Router waehlte {} von {} Experten, geprueft wird Experte {experte}",
        befund.experten.len(),
        moe.experts.len()
    );

    let x = auf.ebenen()[e].norm_mitte.clone();
    assert!(x.iter().any(|v| *v != 0), "der Eingang ist ueberall null");

    let mut gate = master_aus_gewicht(&mlp.gate_proj);
    let mut up = master_aus_gewicht(&mlp.up_proj);
    let mut down = master_aus_gewicht(&mlp.down_proj);
    let hs = m.hidden_size;
    let is = mlp.gate_proj.shape[0];
    eprintln!("  Experte: {hs} Eingaenge, {is} innere Einheiten, {} Gewichte", gate.len() * 2 + down.len());

    // --- 3. Trainieren ----------------------------------------------
    let sc = &m.layers[e].scales;
    let cfg = &m.config;
    let vorgaben = |schritt: u64, lr: i64| Mlpvorgaben {
        hidden_size: hs,
        intermediate_size: is,
        act_frac: sc.norm_mlp_frac,
        gate_frac: sc.gate_frac,
        up_frac: sc.up_frac,
        down_in_frac: sc.down_in_frac,
        aus_frac: sc.residual_mid_frac[0],
        silu_in_frac: cfg.silu_in_frac,
        silu_lut_offset: cfg.silu_lut_offset,
        silu_out_frac: cfg.silu_out_frac,
        lr_zaehler: lr,
        lr_nenner: 1 << 14,
        kennung: Schrittkennung { ebene: e as u32, schritt, index_versatz: 0 },
    };
    let grad_lut = integer_llm_kernels::backward::silu_grad_aus_lut(&m.silu_lut);

    let null = vec![0i16; hs];
    let (mut g0, mut u0, mut d0) = (gate.clone(), up.clone(), down.clone());
    let quadratsumme = schritt_auf_mlp(
        &mut g0, &mut u0, &mut d0, &x, &null, &m.silu_lut, &grad_lut, vorgaben(0, 0),
    );
    assert!(quadratsumme > 0, "der Experte gibt ueberall null aus");
    let typisch = ((quadratsumme / hs as i64) as f64).sqrt() as i16;
    let ziel: Vec<i16> = (0..hs).map(|i| ((i as i16 % 3) - 1) * (typisch / 8).max(1)).collect();
    eprintln!("  typische Ausgabe: {typisch}");

    let mut erster = 0i64;
    let mut letzter = 0i64;
    for s in 0..200u64 {
        let a = schritt_auf_mlp(
            &mut gate, &mut up, &mut down, &x, &ziel, &m.silu_lut, &grad_lut, vorgaben(s, 1),
        );
        if s == 0 {
            erster = a;
        }
        letzter = a;
    }
    eprintln!(
        "  Abstand {erster} -> {letzter} ({} Prozent gefallen)\n",
        100 - (letzter * 100 / erster.max(1))
    );
    assert!(
        letzter * 2 < erster,
        "der Abstand fiel nur von {erster} auf {letzter}. Auf einem dichten Modell faellt \
         er; auf den Gewichten eines Experten offenbar nicht"
    );
}
