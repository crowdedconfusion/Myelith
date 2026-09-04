//! Der Mitschnitt verändert den Vorwärtspass nicht (TRAINING V).
//!
//! # ⚑ Was hier auf dem Prüfstand steht
//!
//! Die Entscheidung vom 2026-09-04 lautet: **ein Pfad mit optionalem
//! Mitschnitt**, kein zweiter Vorwärtspass. Der ganze Wert dieser
//! Entscheidung hängt an einer Zusicherung, und das ist diese hier:
//! **Mit Mitschnitt kommt Bit für Bit dasselbe heraus wie ohne.**
//!
//! Wäre das nicht so, hätte das Projekt still zwei Rechenpfade, und der
//! zweite trüge keine Konformitätsvektoren.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::{Mlpteil, Zwischenwerte};
use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};
use integer_llm_kernels::integer_math::lut_lookup;
use integer_llm_runtime::model::IntegerModel;

fn artefakte() -> std::path::PathBuf {
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(modell)
}

/// ⚑ **Fehlen die Artefakte, schlägt der Test fehl und sagt, was zu tun
/// ist.** Ein stiller Sprung sieht aus wie ein bestandener Test
/// (Fund 113).
fn modell() -> Option<IntegerModel> {
    let dir = artefakte();
    if !dir.exists() {
        if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
            eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {dir:?}");
            return None;
        }
        panic!(
            "Artefakte fehlen: {dir:?}\n\
             Dieser Test belegt, dass der Mitschnitt den Rechenpfad nicht veraendert,\n\
             und kann das ohne Modell nicht.\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    Some(load_model(&dir).expect("Modell laedt"))
}

/// ⚑ **Die tragende Zusicherung: mit Mitschnitt bitgleich.**
///
/// ⛑ Die Gegenprobe ist billig zu denken und teuer zu übersehen: Wer in
/// `forward_layer` beim Aufzeichnen versehentlich umskaliert oder eine
/// Kopie weiterreicht statt des Originals, fällt hier durch und nur
/// hier.
#[test]
fn mit_mitschnitt_kommt_dasselbe_heraus() {
    let Some(m) = modell() else { return };
    let ebenen = m.num_layers;

    // Dieselbe Folge zweimal, einmal ohne und einmal mit Mitschnitt.
    let folge = [9707u32, 11, 3837, 261, 15235];
    let laufen = |mit: bool| -> (Vec<i32>, Zwischenwerte) {
        let mut cache = KVCache::for_range(0, ebenen, m.num_kv_heads);
        let mut auf = Zwischenwerte::neu();
        let mut logits = Vec::new();
        for (pos, tok) in folge.iter().enumerate() {
            let h = m.embed_token(*tok as usize);
            let h = if mit {
                auf.leeren();
                m.run_layers_mit_mitschnitt(h, pos, &mut cache, 0, ebenen, &mut auf)
            } else {
                m.run_layers(h, pos, &mut cache, 0, ebenen)
            };
            logits = m.head_logits(&h);
        }
        (logits, auf)
    };

    let (ohne, leer) = laufen(false);
    let (mit, auf) = laufen(true);

    assert!(leer.is_empty(), "ohne Mitschnitt darf nichts aufgezeichnet werden");
    assert_eq!(
        ohne, mit,
        "der Mitschnitt hat den Rechenpfad veraendert; damit gaebe es zwei Wahrheiten darueber"
    );
    assert_eq!(
        auf.len(),
        ebenen,
        "es wurde nicht je Ebene genau einmal aufgezeichnet"
    );
}

/// ⚑ **Der Mitschnitt hängt zusammen: Was eine Ebene ausgibt, ist der
/// Eingang der nächsten.**
///
/// **Ohne diese Zusicherung wäre `residual_ein` eine Zahlenreihe ohne
/// Bedeutung.** Sie ist die Stelle, an der der Rückwärtspass den
/// Gradienten der Residualaddition ansetzt; zeigte sie auf die falsche
/// Ebene, käme ein Gradient heraus, der zu keiner gelaufenen Rechnung
/// gehört, und nichts würde das melden.
#[test]
fn die_ebenen_haengen_aneinander() {
    let Some(m) = modell() else { return };
    let ebenen = m.num_layers;
    let mut cache = KVCache::for_range(0, ebenen, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();

    let start = m.embed_token(9707);
    let _ = m.run_layers_mit_mitschnitt(start.clone(), 0, &mut cache, 0, ebenen, &mut auf);

    let e = auf.ebenen();
    assert_eq!(e.len(), ebenen);
    assert_eq!(
        e[0].residual_ein, start,
        "die erste Ebene sah nicht die Einbettung"
    );

    for (i, ebene) in e.iter().enumerate() {
        assert_eq!(
            ebene.residual_ein.len(),
            m.hidden_size,
            "Ebene {i}: der Residualeingang hat die falsche Breite"
        );
        assert_eq!(ebene.norm_ein.len(), m.hidden_size, "Ebene {i}: norm_ein");
        assert_eq!(ebene.residual_mitte.len(), m.hidden_size, "Ebene {i}: residual_mitte");
        assert_eq!(ebene.norm_mitte.len(), m.hidden_size, "Ebene {i}: norm_mitte");
        assert_eq!(ebene.q.len(), m.num_heads, "Ebene {i}: Q-Koepfe");
        assert_eq!(ebene.k.len(), m.num_kv_heads, "Ebene {i}: K-Koepfe");
        assert_eq!(ebene.v.len(), m.num_kv_heads, "Ebene {i}: V-Koepfe");
        assert_eq!(
            ebene.attn_aus.len(),
            m.num_heads * m.head_dim,
            "Ebene {i}: die Aufmerksamkeitsausgabe"
        );
        // ⚑ **Nicht alles null.** Ein Mitschnitt aus lauter Nullen
        // bestuende jede Laengenpruefung und traege keinen Gradienten.
        assert!(
            ebene.norm_ein.iter().any(|v| *v != 0),
            "Ebene {i}: der normierte Eingang ist ueberall null"
        );
    }
}

/// ⚑ **Die vier Werte aus den Kernen sind da, und sie stimmen.**
///
/// Aufmerksamkeitswahrscheinlichkeiten, Gate, Up und das Produkt liegen
/// in `attention_int` und `mlp_int`, die nur ihre Ausgabe zurückgeben.
/// Sie kommen über einen zweiten Eingang heraus, und dieser Test prüft
/// nicht nur, **dass** etwas ankommt, sondern **was**.
#[test]
fn die_werte_aus_den_kernen_stimmen() {
    let Some(m) = modell() else { return };
    let ebenen = m.num_layers;
    let mut cache = KVCache::for_range(0, ebenen, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();

    // Drei Positionen, damit die Aufmerksamkeit über mehr als einen
    // Schlüssel läuft: Bei einer einzigen Position ist jede
    // Wahrscheinlichkeit trivial eins, und der Test prüfte nichts.
    for (pos, tok) in [9707u32, 11, 3837].iter().enumerate() {
        auf.leeren();
        let h = m.embed_token(*tok as usize);
        let _ = m.run_layers_mit_mitschnitt(h, pos, &mut cache, 0, ebenen, &mut auf);
    }

    let eins: i64 = 1i64 << m.config.prob_frac_bits;
    for (i, ebene) in auf.ebenen().iter().enumerate() {
        assert_eq!(
            ebene.wahrscheinlichkeiten.len(),
            m.num_heads,
            "Ebene {i}: eine Zeile je Kopf erwartet"
        );
        for (h, zeile) in ebene.wahrscheinlichkeiten.iter().enumerate() {
            assert_eq!(
                zeile.len(),
                3,
                "Ebene {i}, Kopf {h}: drei Positionen, drei Wahrscheinlichkeiten"
            );
            // ⚑ **Die Softmax-Invariante, und sie ist der eigentliche
            // Beleg.** Eine Länge stimmt auch bei den falschen Zahlen;
            // dass sie sich zu eins summieren, stimmt nur, wenn es
            // wirklich die Wahrscheinlichkeiten sind.
            let summe: i64 = zeile.iter().map(|p| *p as i64).sum();
            let abstand = (summe - eins).abs();
            assert!(
                abstand * 100 <= eins,
                "Ebene {i}, Kopf {h}: die Wahrscheinlichkeiten summieren sich zu {summe} \
                 statt {eins}; das sind keine Wahrscheinlichkeiten"
            );
        }

        // ⚑ **Der MLP-Teil ist ein Typ, kein Vektorbuendel.** Bei
        // einem Expertengemisch sagt er „nicht aufgezeichnet", und der
        // Uebersetzer zwingt diesen Test, den Fall zu behandeln.
        let Mlpteil::Dicht { gate, up, h } = &ebene.mlp else {
            panic!("Ebene {i}: das Probemodell ist dicht, der Mitschnitt sagt etwas anderes");
        };
        let (ebene_mlp_gate, ebene_mlp_up, ebene_mlp_h) = (gate, up, h);
        assert!(!ebene_mlp_gate.is_empty(), "Ebene {i}: Gate fehlt");
        assert_eq!(
            ebene_mlp_gate.len(),
            ebene_mlp_up.len(),
            "Ebene {i}: Gate und Up sind verschieden breit"
        );
        assert_eq!(
            ebene_mlp_h.len(),
            ebene_mlp_gate.len(),
            "Ebene {i}: das Produkt passt nicht zu seinen Faktoren"
        );
        assert!(
            ebene_mlp_gate.iter().any(|v| *v != 0),
            "Ebene {i}: das Gate ist ueberall null"
        );
        assert!(
            ebene_mlp_h.iter().any(|v| *v != 0),
            "Ebene {i}: das Produkt ist ueberall null"
        );

        // ⚑ **Drei verschiedene Groessen, also drei verschiedene
        // Vektoren.** ⛑ Die Gegenprobe „das Produkt statt des Gates
        // aufzeichnen" blieb ohne diese Zeile **gruen**: Alle drei sind
        // gleich breit, also faengt keine Laengenpruefung eine
        // Vertauschung.
        assert_ne!(ebene_mlp_gate, ebene_mlp_up, "Ebene {i}: Gate gleich Up");
        assert_ne!(ebene_mlp_gate, ebene_mlp_h, "Ebene {i}: Gate gleich Produkt");
        assert_ne!(ebene_mlp_up, ebene_mlp_h, "Ebene {i}: Up gleich Produkt");

        // ⚑ **Und sie haengen richtig zusammen.** `h = silu(gate) · up`,
        // und `silu(x) > 0` fuer `x > 0`; wo das Gate positiv ist, hat
        // das Produkt also das Vorzeichen von Up. Das bindet alle drei
        // aneinander statt sie nur einzeln zu pruefen.
        let mut geprueft = 0usize;
        for ((g, u), h) in ebene_mlp_gate
            .iter()
            .zip(ebene_mlp_up.iter())
            .zip(ebene_mlp_h.iter())
        {
            if *g > 0 && *u != 0 && *h != 0 {
                assert_eq!(
                    h.signum(),
                    u.signum(),
                    "Ebene {i}: bei positivem Gate muss das Produkt \
                     das Vorzeichen von Up tragen"
                );
                geprueft += 1;
            }
        }
        assert!(
            geprueft > 0,
            "Ebene {i}: kein einziger Wert erfuellte die Voraussetzung; \
             die Vorzeichenpruefung prueft dann nichts"
        );

        // ⚑ **Und jetzt scharf: `h` wird aus `gate` und `up`
        // nachgerechnet.**
        //
        // ⛑ Die Gegenprobe „Gate und Up vertauscht" ueberlebte alle
        // Pruefungen darueber, und zwar zu Recht: Beide sind
        // Projektionen desselben Eingangs, gleich breit und
        // vorzeichensymmetrisch. **Nichts an ihrer Form unterscheidet
        // sie**, nur ihre Rolle in `h = silu(gate) · up`, und die ist
        // nicht symmetrisch.
        //
        // Das ist keine zweite Umsetzung des Vorwaertspfades, sondern
        // genau die Rechnung, die der Rueckwaertspass rueckwaerts geht.
        let sc = &m.layers[i].scales;
        let cfg = &m.config;
        for (n, ((g, u), h)) in ebene_mlp_gate
            .iter()
            .zip(ebene_mlp_up.iter())
            .zip(ebene_mlp_h.iter())
            .enumerate()
        {
            let g_dom = rescale(*g as i32, sc.gate_frac, cfg.silu_in_frac);
            let aktiv = lut_lookup(g_dom as i16, &m.silu_lut, 0, cfg.silu_lut_offset);
            let prod = (aktiv as i64) * (*u as i64);
            let erwartet = clamp_i16_from_i64(rescale_i64(
                prod,
                cfg.silu_out_frac + sc.up_frac,
                sc.down_in_frac,
            ));
            assert_eq!(
                *h, erwartet,
                "Ebene {i}, Stelle {n}: das Produkt passt nicht zu Gate und Up. \
                 Entweder ist einer der drei der falsche Wert, oder sie stammen \
                 aus verschiedenen Durchlaeufen."
            );
        }
    }
}
