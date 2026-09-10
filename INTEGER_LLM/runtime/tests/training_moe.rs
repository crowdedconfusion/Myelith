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
use integer_llm_runtime::trainingsschleife::{trainingsschleife, Trainingsvorgaben};
use integer_llm_kernels::backward::{linear_backward, moe_backward};
use integer_llm_kernels::moe::{mische_experten, route_top_k};
use integer_llm_kernels::optimierer::schritt;
use integer_llm_kernels::trainingsschritt::gewicht_aus_master;

const MODELL: &str = "myelith-30b-a3b";

// ⚑ **Die Zahl steht in der Bibliothek, nicht hier.** Sie geht in die
// Bitgleichheit ein: Zwei Miner mit verschiedenen Werten bekommen
// verschiedene Gewichte. Bis zum 2026-09-04 stand sie in **zwei**
// Testdateien, also genau die Lage, die dieses Projekt sonst durch einen
// Test verbindet.
use integer_llm_kernels::optimierer::MASTER_FRAC;

fn master_aus_gewicht(t: &QTensor) -> Vec<Master> {
    let in_features = t.shape[1];
    t.data
        .iter()
        .enumerate()
        .map(|(i, w)| i32::from(*w) << (MASTER_FRAC - t.shifts[i / in_features]))
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

    // ⚑ **Seit dem 2026-09-05 zeichnet der Mitschnitt hier auf**, und
    // dieser Test hält fest, dass er es vollständig tut: Er ist die
    // einzige Stelle im Baum, die den MoE-Zweig überhaupt erreicht.
    for (i, e) in auf.ebenen().iter().enumerate() {
        let Mlpteil::Expertengemisch { experten, gewichte, ausgaben, teile, logits } = &e.mlp
        else {
            panic!("Ebene {i}: der Mitschnitt haelt einen MoE-Block fuer dicht");
        };
        let k = experten.len();
        assert!(k > 0, "Ebene {i}: kein Experte gewaehlt");
        // ⚑ **Vier Listen, eine Reihenfolge.** `moe_backward` ordnet
        // `je_ausgabe` den `experten` zu; weichen die Längen ab, ordnet
        // es Gradienten den falschen Experten zu, und es fiele nicht auf.
        assert_eq!(gewichte.len(), k, "Ebene {i}: Gewichte passen nicht zur Wahl");
        assert_eq!(ausgaben.len(), k, "Ebene {i}: Ausgaben passen nicht zur Wahl");
        assert_eq!(teile.len(), k, "Ebene {i}: Zwischenwerte passen nicht zur Wahl");
        assert!(!logits.is_empty(), "Ebene {i}: keine Routerlogits");
        for (n, t) in teile.iter().enumerate() {
            assert!(!t.gate.is_empty() && !t.up.is_empty() && !t.h.is_empty(),
                "Ebene {i}, Experte {n}: Zwischenwerte leer");
        }
        assert!(
            !e.norm_mitte.is_empty(),
            "Ebene {i}: der Eingang des Blocks fehlt"
        );
    }
    let erste = &auf.ebenen()[0].mlp;
    if let Mlpteil::Expertengemisch { experten, gewichte, logits, .. } = erste {
        eprintln!(
            "  Mitschnitt: {} Ebenen, je {} Experten aus {}, Gewichte {:?}",
            auf.len(),
            experten.len(),
            logits.len(),
            gewichte
        );
    }

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
        master_frac: MASTER_FRAC,
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

/// ⚑ **Treibt der Router unter anhaltendem Gradienten in die Sättigung
/// (Fund 79)?**
///
/// # Warum diese Messung die Entscheidung über das Primärmodell trägt
///
/// `router_saettigung` misst den **Ausgangspunkt** und findet ihn weit
/// von der Sättigung entfernt: null von 720 Routerstellen gesättigt, der
/// Abstand bis dahin etwa doppelt so gross wie die gesamte beobachtete
/// Logitspanne. **Das ist eine Aussage über einen Zustand, nicht über
/// eine Bahn.** Training treibt Routerlogits auseinander; genau darauf
/// läuft ein Lauf zu.
///
/// # ⚑ Der Zuschnitt, und warum er der schärfere ist
///
/// **Nur der Router lernt, die Experten bleiben eingefroren.** Zwei
/// Gründe, und der zweite ist der wichtigere:
///
/// 1. Alle 128 Experten als Master zu halten kostet 2,4 GB. Der Router
///    allein kostet ein Megabyte.
/// 2. ⚑ **Es ist der Störfaktor weniger.** Wenn die Experten mitlernen,
///    sinkt der Verlust auch ohne dass der Router sich bewegt, und dann
///    sagt ein ausbleibender Drift nichts.
///
/// **Und das Ziel ist fest, über alle Schritte dasselbe.** Ein echter
/// Korpus wechselt das Ziel und damit die Richtung; ein festes Ziel
/// drückt unablässig in dieselbe Richtung. ⚑ **Das ist der harte Fall:**
/// Wenn der Router hier nicht sättigt, sättigt er unter einem echten
/// Korpus erst recht nicht.
#[test]
#[ignore = "laedt 29 GB Artefakte, rund zwei Minuten; mit --ignored ausfuehren"]
fn der_router_treibt_nicht_in_die_saettigung() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(MODELL);
    assert!(dir.exists(), "Das MoE-Artefakt fehlt: {dir:?}");
    let m = load_model(&dir).expect("Modell laedt");
    let cfg = &m.config;

    // Der Eingang des Blocks, aus einem echten Vorwärtspass.
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    let h = m.embed_token(9707);
    let _ = m.run_layers_mit_mitschnitt(h, 0, &mut cache, 0, m.num_layers, &mut auf);
    let e = m.num_layers / 2;
    let x = auf.ebenen()[e].norm_mitte.clone();
    let ebene = &m.layers[e];
    let Feedforward::Moe(moe) = &ebene.ffn else {
        panic!("Ebene {e} ist kein Expertengemisch");
    };
    let sc = &ebene.scales;

    let n = moe.experts.len();
    let is = moe.experts[0].gate_proj.shape[0];
    eprintln!(
        "\n=== Router-Drift auf {MODELL}, Ebene {e} ===\n  \
         {n} Experten, top-{}, hidden {}, innere Breite {is}",
        moe.top_k, m.hidden_size
    );

    let eins = 1i32 << cfg.prob_frac_bits;
    let exp_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
    let acc = vec![sc.residual_mid_frac[0]; m.hidden_size];
    let schritte = 200u64;

    // ⚑ **Ein Lauf, mehrere Schrittweiten.** Das Laden kostet zwei
    // Minuten, ein Schritt Millisekunden; eine Schrittweite je Lauf zu
    // messen hiesse, zehn Minuten Ladezeit fuer zehn Sekunden Rechnung
    // zu bezahlen.
    let mut zeilen: Vec<String> = Vec::new();
    let mut irgendwo_ohne_saettigung = false;

    // ⚑ **Der Boden: kein Mischgewicht faellt auf null, keines traegt
    // alles.** Damit ist der absorbierende Zustand nicht vermieden,
    // sondern **entfernt**: Fuer einen Verlierer ist `p_i >= 1`, fuer den
    // Gewinner `p_0 <= eins - (k-1)`, und beide Wege zum Gradienten null
    // sind zu. Der Ueberschuss geht vom groessten ab, damit die Summe
    // exakt `eins` bleibt (dieselbe Regel wie `moe::korrigiere_summe`).
    let mit_boden = |w: &mut Vec<i32>| {
        for v in w.iter_mut() {
            if *v < 1 {
                *v = 1;
            }
        }
        let summe: i32 = w.iter().sum();
        let ueberschuss = summe - eins;
        if ueberschuss != 0 {
            let (i, _) = w.iter().enumerate().max_by_key(|(i, v)| (**v, std::cmp::Reverse(*i))).unwrap();
            w[i] -= ueberschuss;
        }
    };

    for (nenner_bits, boden) in [
        (10u32, false), (14, false), (18, false), (22, false), (26, false), (30, false),
        (10, true), (14, true), (18, true), (22, true),
    ] {
        let mut master: Vec<Master> = {
            let t = &moe.router;
            let in_features = t.shape[1];
            t.data
                .iter()
                .enumerate()
                .map(|(i, w)| i32::from(*w) << (MASTER_FRAC - t.shifts[i / in_features]))
                .collect()
        };
        let mut ziel: Vec<i16> = Vec::new();
        let mut erste: Vec<i32> = Vec::new();
        let mut letzte: Vec<i32> = Vec::new();
        let mut saettigungen = 0usize;
        let mut erster_abstand = 0i64;
        let mut letzter_abstand = 0i64;
        let mut bewegte = 0usize;
        let mut ausgelaufen: Option<u64> = None;

        for s in 0..schritte {
            let (rw, rs) = gewicht_aus_master(&master, m.hidden_size, MASTER_FRAC);
            let logits: Vec<i32> = integer_llm_kernels::linear::linear_w8a16(
                &x, &rw, m.hidden_size, &rs, sc.norm_mlp_frac, moe.router_frac,
            )
            .iter()
            .map(|l| i32::from(*l))
            .collect();
            let mut routing = route_top_k(
                &logits, moe.top_k, &m.exp_lut, exp_shift, cfg.prob_frac_bits,
                moe.norm_topk_prob,
            );
            if boden {
                mit_boden(&mut routing.gewichte);
            }
            let ausgaben: Vec<Vec<i16>> = routing
                .experten
                .iter()
                .map(|i| {
                    let ex = &moe.experts[*i as usize];
                    integer_llm_kernels::mlp::mlp_int_mit_spur(
                        &x, &ex.gate_proj.data, &ex.up_proj.data, &ex.down_proj.data,
                        m.hidden_size, is, &ex.gate_proj.shifts, &ex.up_proj.shifts,
                        &ex.down_proj.shifts, &m.silu_lut, sc.norm_mlp_frac, sc.gate_frac,
                        sc.up_frac, sc.down_in_frac, cfg.silu_in_frac, cfg.silu_lut_offset,
                        cfg.silu_out_frac, &acc, None,
                    )
                })
                .collect();
            let y = mische_experten(&ausgaben, &routing.gewichte, cfg.prob_frac_bits);

            if s == 0 {
                ziel = y.iter().map(|v| -*v).collect();
                erste = routing.gewichte.clone();
            }
            letzte = routing.gewichte.clone();
            let abstand: i64 = y
                .iter()
                .zip(ziel.iter())
                .map(|(p, z)| {
                    let d = i64::from(*p) - i64::from(*z);
                    d * d
                })
                .sum();
            if s == 0 {
                erster_abstand = abstand;
            }
            letzter_abstand = abstand;
            if routing.gewichte.contains(&eins) || routing.gewichte.contains(&0) {
                saettigungen += 1;
            }

            let g: Vec<i32> = y
                .iter()
                .zip(ziel.iter())
                .map(|(p, z)| 2 * (i32::from(*p) - i32::from(*z)))
                .collect();
            let gemisch = moe_backward(
                &g, &routing.experten, &routing.gewichte, &ausgaben, n,
                cfg.prob_frac_bits, acc[0], 6,
            );
            let (_gx, gw) = linear_backward(
                &gemisch.logits, &x, &rw, m.hidden_size, &rs, acc[0] + 6, sc.norm_mlp_frac,
            );
            let gr: Vec<i32> = gw
                .iter()
                .map(|v| (*v).clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .collect();
            let vorher = master.clone();
            schritt(
                &mut master, &gr,
                Schrittkennung { ebene: e as u32, schritt: s, index_versatz: 0 },
                1, 1i64 << nenner_bits,
            );
            if s == 0 {
                bewegte = vorher.iter().zip(master.iter()).filter(|(a, b)| a != b).count();
            }
            // ⚑ **Die Schranke aus Fund 174, und sie ist hier die
            // zweite Ausfallart.** Ein Master ueber `127 << master_frac`
            // laesst sich nicht mehr als int8 mit Zeilenversatz
            // ausdruecken. **Ohne Boden stirbt der Router still, mit
            // Boden laeuft er laut aus dem Master**: Das ist ein
            // Fortschritt, aber kein Ergebnis, und der Lauf endet hier.
            let grenze = 127i64 << MASTER_FRAC;
            if master.iter().any(|v| i64::from(v.abs()) > grenze) {
                ausgelaufen = Some(s);
                break;
            }
        }
        if saettigungen == 0 && bewegte > 0 && ausgelaufen.is_none() {
            irgendwo_ohne_saettigung = true;
        }
        zeilen.push(format!(
            "  2^{nenner_bits:<2} {:<6} {bewegte:>6} bewegt  {saettigungen:>3} gesaettigt{}  \
             Abstand {erster_abstand:>13} -> {letzter_abstand:>13}\n         letzte {letzte:?}",
            if boden { "Boden" } else { "" },
            match ausgelaufen {
                Some(s) => format!("  AUS DEM MASTER bei Schritt {s}"),
                None => String::new(),
            }
        ));
        let _ = &erste;
    }

    eprintln!("\n  Nenner | bewegte Gewichte im ersten Schritt | gesaettigte Schritte");
    for z in &zeilen {
        eprintln!("{z}");
    }
    eprintln!();

    // ⚑ **Die Aussage, um die es geht.** Ein Gewicht auf `eins` oder auf
    // null heisst: der Routergradient ist ab hier exakt null, und der
    // Router kann sich nie wieder aendern.
    assert!(
        irgendwo_ohne_saettigung,
        "bei KEINER Schrittweite bewegt sich der Router, ohne zu saettigen (Fund 79)"
    );
}

/// ⚑ **Der ganze Expertenblock lernt sein Ziel: Router und Experten
/// zusammen, am echten Modell, gegen das nächste Token.**
///
/// # Was diese Messung schliesst
///
/// Die Teile waren belegt und der Lauf fehlte: dass ein gewählter
/// Experte lernt (oben), dass der Gradient durch die Mischung stimmt
/// (`kernels`, gegen die geschlossene Form), dass der Router sich bewegt
/// ohne zu sättigen (nebenan). **Hier laufen sie zusammen**, mit dem
/// echten Ziel statt einem erfundenen: dem nächsten Token.
///
/// ⚑ **Und der Abdruck ist derselbe wie im dichten Lauf.** Damit ist die
/// sechste Stufe des Testclients auch auf einem Expertengemisch ein
/// Vergleichswert.
#[test]
#[ignore = "laedt 29 GB Artefakte, rund zwei Minuten; mit --ignored ausfuehren"]
fn der_expertenblock_lernt_sein_ziel() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(MODELL);
    assert!(dir.exists(), "Das MoE-Artefakt fehlt: {dir:?}");
    let m = load_model(&dir).expect("Modell laedt");

    // ⚑ **Weniger Schritte als im dichten Lauf.** Je Schritt entsteht
    // fuer jeden neu gewaehlten Experten ein Master von 19 MB; dreissig
    // Schritte mit wanderndem Router koennten ueber ein Gigabyte
    // aufziehen, und die Aussage haengt nicht daran.
    let v = Trainingsvorgaben {
        schritte: 12,
        lr_nenner: 1 << 14,
        ..Trainingsvorgaben::vorgabe()
    };
    let t0 = std::time::Instant::now();
    let e = trainingsschleife(&m, &v).expect("die Gemischschleife laeuft");
    let dauer = t0.elapsed();

    let verlust = |l: &[i32], ziel: usize| -> f64 {
        let skala = f64::from(1u32 << v.logit_frac);
        let mx = f64::from(*l.iter().max().unwrap());
        let summe: f64 = l.iter().map(|z| ((f64::from(*z) - mx) / skala).exp()).sum();
        summe.ln() + mx / skala - f64::from(l[ziel]) / skala
    };
    let (vorher, nachher) = e.argmax();
    let erster = verlust(&e.logits_erst, v.ziel);
    let letzter = verlust(&e.logits_letzt, v.ziel);

    eprintln!(
        "\n=== Expertenblock, Ebene {} von {MODELL} ===\n  \
         {} von {} Gewichten bewegt, {} Schritte in {dauer:?}\n  \
         Kreuzentropie {erster:.4} -> {letzter:.4}\n  \
         Argmax {vorher} -> {nachher} (Ziel {})\n  \
         Trainingsabdruck: {}\n",
        e.ebene, e.bewegte_gewichte, e.gewichte_gesamt, v.schritte, v.ziel, e.abdruck
    );

    assert!(
        e.bewegte_gewichte > 0,
        "kein einziges Gewicht hat sich bewegt: der Schritt war wirkungslos"
    );
    // ⚑ **Der Verlust faellt**, und das ist die Aussage. Eine Schranke
    // wie im dichten Lauf waere hier zu scharf: Zwoelf Schritte auf einem
    // Block, dessen Aufmerksamkeit eingefroren ist, bringen weniger als
    // dreissig auf einer ganzen Ebene.
    assert!(
        letzter < erster,
        "der Verlust stieg oder blieb: {erster:.4} auf {letzter:.4}"
    );
    assert_eq!(e.abdruck.len(), 64, "der Abdruck ist kein voller SHA-256");
}

/// ⚑ **Verteilt der Lastausgleich die Arbeit, ohne Zufall und ohne
/// Batch-Statistik?**
///
/// # Der zweite absorbierende Zustand, und er ist der stillere
///
/// `router_saettigung` und `der_router_treibt_nicht_in_die_saettigung`
/// behandeln den ersten: Ein **gewählter** Experte, dessen Gewicht auf
/// null rundet, bekommt keinen Gradienten. Der Boden aus θ_v 0.18.0 hat
/// ihn entfernt.
///
/// **Der zweite ist stiller.** Ein Experte, dessen Logit so weit unter
/// den übrigen liegt, dass er nie in die Top-k kommt, wird nie
/// gerechnet, bekommt nie einen Gradienten und ändert sich nie. Er ist
/// tot, **ohne dass irgendeine Zahl davon abweicht**.
///
/// # ⚑ Was hier gemessen wird
///
/// Derselbe Lauf zweimal, mit und ohne Lastausgleich, und gezählt wird,
/// wie viele **verschiedene** Experten überhaupt drankamen. Das ist die
/// Zahl, an der sich das Verfahren messen lässt; alles andere wäre eine
/// Behauptung.
#[test]
#[ignore = "laedt 29 GB Artefakte, rund zwei Minuten; mit --ignored ausfuehren"]
fn der_lastausgleich_verteilt_die_arbeit() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(MODELL);
    assert!(dir.exists(), "Das MoE-Artefakt fehlt: {dir:?}");
    let m = load_model(&dir).expect("Modell laedt");

    let grund = Trainingsvorgaben { schritte: 24, lr_nenner: 1 << 14, ..Trainingsvorgaben::vorgabe() };
    let ohne = Trainingsvorgaben {
        lastausgleich: integer_llm_runtime::trainingsschleife::Lastausgleich::aus(),
        ..grund.clone()
    };

    let a = trainingsschleife(&m, &ohne).expect("ohne Ausgleich");
    let b = trainingsschleife(&m, &grund).expect("mit Ausgleich");
    let (a_n, gesamt) = a.experten_beruehrt.expect("ein Gemisch");
    let (b_n, _) = b.experten_beruehrt.expect("ein Gemisch");

    eprintln!(
        "\n=== Lastausgleich auf {MODELL}, Ebene {} ===\n  \
         ohne Ausgleich: {a_n} von {gesamt} Experten beruehrt\n  \
         mit  Ausgleich: {b_n} von {gesamt} Experten beruehrt\n  \
         bewegte Gewichte {} gegen {}\n",
        a.ebene, a.bewegte_gewichte, b.bewegte_gewichte
    );

    // ⚑ **Die Aussage:** Der Ausgleich holt mehr Experten herein.
    assert!(
        b_n > a_n,
        "der Lastausgleich hat nichts verteilt: {a_n} ohne, {b_n} mit"
    );
    // ⛑ **Und die Gegenprobe zum Aufbau selbst.** Berührt der Lauf ohne
    // Ausgleich schon alle Experten, misst der Test nichts.
    assert!(
        a_n < gesamt,
        "ohne Ausgleich kommen schon alle {gesamt} dran, dann prueft dieser Test nichts"
    );
    // ⚑ **Und der Abdruck unterscheidet sich**, denn es ist eine andere
    // Rechnung. Wer den Lastausgleich abschaltet, rechnet etwas anderes,
    // und das muss sichtbar sein.
    assert_ne!(a.abdruck, b.abdruck, "derselbe Abdruck bei anderer Rechnung");
}
