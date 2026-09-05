//! Der Trainingslauf: echtes Modell, echte Aktivierungen, echte Gewichte.
//!
//! # ⚑ Was dieser Lauf zeigt, und was die Kerneltests nicht zeigen
//!
//! `kernels::trainingsschritt` schliesst den Kreis über einen MLP-Block
//! und prüft ihn an **erfundenen** Zahlen: acht Eingänge, sechzehn
//! innere Einheiten, Gewichte aus einer Modulo-Reihe. Das ist richtig
//! so, denn dort geht es um die Skalen zwischen den Kernen, und die
//! zeigen sich am kleinen Beispiel schärfer.
//!
//! **Hier laufen dieselben Kerne auf dem, was das Netz wirklich
//! rechnet:** 896 Eingänge, 4 864 innere Einheiten, die
//! Qwen2.5-0,5B-Gewichte und ein Aktivierungsvektor, der aus einem
//! echten Vorwärtspass stammt.
//!
//! ⚑ **Der Unterschied ist nicht die Grösse, sondern die Verteilung.**
//! Erfundene Gewichte sind gleichmässig; echte haben Ausreisser,
//! Nullzeilen und eine Spanne über Grössenordnungen. Genau daran bricht
//! Ganzzahlarithmetik, wenn eine Skala nicht passt.
//!
//! # ⚑ Was er ausdrücklich nicht abdeckt
//!
//! **Ein Expertengemisch.** Das steht in `training_moe.rs` und ist
//! ⚑ **entgegen der ersten Einschätzung möglich:** Die Gewichte werden
//! **speicherabgebildet**, nicht in den Heap geladen, also passt auch
//! ein 29-GB-Artefakt auf eine Maschine mit 24 GB.
//!
//! ⚑ **Der Aufmerksamkeitsblock ist seit dem 2026-09-04 dabei**, und
//! zwar mit einer Prüfung, die es beim MLP nicht braucht: Der
//! Trainingsschritt des MLP ruft **denselben** Kern wie die Inferenz,
//! der des Aufmerksamkeitsblocks kann das nicht (die Laufzeit schreibt
//! ihn aus, über eine Position mit Zwischenspeicher, nicht über eine
//! Folge). ⚑ **Es gibt also zwei Umsetzungen desselben Vorwärtspasses,
//! und `die_vorwaerts_haelfte_trifft_den_mitschnitt` bindet die zweite
//! byteweise an die erste.**
//!
//! # ⚑ Was dieser Lauf gefunden hat: eine Lernrate passt nicht zu allen Ebenen
//!
//! Gemessen an Qwen2.5-0,5B, alle mit derselben Rate `1/2^14`:
//!
//! | Ebene | typische Ausgabe | `aus_frac` | nach 40 Schritten | nach 200 |
//! |---|---|---|---|---|
//! | 0 | 29 702 | **19** | 18 % | 96 % |
//! | 6 | 1 381 | 13 | 100 % | |
//! | 12 | 882 | 13 | 99 % | |
//! | 18 | 555 | 12 | 63 % | 99 % |
//! | 23 | 8 067 | 12 | 99 % | |
//!
//! ⚑ **Die Richtung stimmt überall, die Rate nicht.** Ebene 0 fällt in
//! vierzig Schritten nur um achtzehn Prozent und in zweihundert um
//! sechsundneunzig; sie ist nicht kaputt, sie ist langsam. Der Grund
//! steht in der Tabelle: Ihre Ausgabeskala trägt **sechs Bit mehr** als
//! die der mittleren Ebenen, also ist derselbe Schritt dort
//! vierundsechzigmal kleiner.
//!
//! ⚑ **Und ihre typische Ausgabe liegt bei 29 702**, also dicht unter
//! der `i16`-Grenze von 32 767. Wer die Lernrate dort anhebt, ohne das
//! zu bedenken, laeuft in die Saettigung.
//!
//! **Für einen echten Trainingslauf folgt daraus:** Die Lernrate gehört
//! je Ebene gesetzt oder der Gradient normiert. Eine Zahl für alle
//! Ebenen ist entweder für die einen zu klein oder für die anderen zu
//! gross. Das ist ein Ergebnis dieses Laufs und kein Mangel der Kerne.

use integer_llm_kernels::optimierer::{Master, Schrittkennung};
use integer_llm_kernels::trainingsschritt::{
    schritt_auf_aufmerksamkeit, schritt_auf_mlp, vorwaerts_der_aufmerksamkeit,
    vorwaerts_der_ebene, Aufmerksamkeitsgewichte, Aufmerksamkeitsvorgaben, Ebenengewichte,
    Ebenentabellen, Ebenenvorgaben, Mlpvorgaben, Vorspannungen,
};
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::{Mlpteil, Zwischenwerte};
use integer_llm_runtime::model::{Feedforward, IntegerModel, QTensor};

/// Wie viele Schritte je Ebene.
///
/// ⚑ **Zweihundert und nicht vierzig**, und der Grund ist der Befund
/// oben: Ebene 0 braucht sie, weil ihre Ausgabeskala sechs Bit mehr
/// trägt. Vierzig genügten für die mittleren Ebenen und hätten den
/// Unterschied verdeckt.
const SCHRITTE: u64 = 200;

/// Wie viele Schritte je Ebene im Aufmerksamkeitsblock.
///
/// ⚑ **Weniger als beim MLP, und der Grund ist die Laufzeit.** Ein
/// Schritt rechnet sechs Positionen vorwaerts und rueckwaerts, und die
/// Aufmerksamkeit ist darin quadratisch in der Folgenlaenge. Gemessen
/// genuegen sie: Der Abstand faellt auf beiden geprueften Ebenen um mehr
/// als die Haelfte.
const A_SCHRITTE: u64 = 60;

/// Der Nenner der Lernrate des Aufmerksamkeitsblocks.
///
/// ⚑ **Sechzehnmal kleiner als vor Fund 174, und der Grund ist der
/// Fund selbst.** Vorher stand in der Zeilenverschiebung `s` statt
/// `master_frac − s`, und damit wirkte ein Schritt auf eine Zeile um
/// `2^(2·shift)` gedaempft: Zeilen mit **kleinen** Gewichten tragen eine
/// **grosse** Verschiebung und waren praktisch eingefroren. Die alte
/// Rate war fuer die grossen Zeilen gewaehlt; seit sich alle Zeilen
/// gleich schnell bewegen, laesst sie den Lauf davonlaufen (gemessen:
/// der Abstand stieg von 9,0e10 auf 5,8e12).
///
/// **Gemessen ueber fuenf Raten** auf Ebene 0, sechzig Schritte:
/// `1/2^14` steigt, `1/2^18` faellt auf 3,4e9, `1/2^20` auf 1,4e10,
/// `1/2^22` auf 4,4e10, `1/2^24` bewegt fast nichts mehr.
const A_NENNER: i64 = 1 << 18;

fn artefakte() -> std::path::PathBuf {
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(modell)
}

fn modell() -> Option<IntegerModel> {
    let dir = artefakte();
    if !dir.exists() {
        if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
            eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {dir:?}");
            return None;
        }
        panic!(
            "Artefakte fehlen: {dir:?}\n\
             Dieser Lauf trainiert auf echten Gewichten und kann das ohne Modell nicht.\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    Some(load_model(&dir).expect("Modell laedt"))
}

/// Baut aus einem geladenen Gewicht die Masterdarstellung zurück.
///
/// ⚑ **Die Umkehrung von `gewicht_aus_master`, und sie ist nicht
/// verlustfrei.** Dort wurde nach rechts geschoben und gerundet; hier
/// wird nach links geschoben, und was beim Runden verlorenging, kommt
/// nicht zurück. **Das ist kein Mangel dieses Laufs, sondern die Lage:**
/// Ein Artefakt trägt `i8` plus Skala, ein Master hat mehr Auflösung,
/// und der Weg vom Artefakt zurück füllt sie mit Nullen. Wer aus einem
/// Artefakt weitertrainiert, startet genau so.
fn master_aus_gewicht(t: &QTensor) -> Vec<Master> {
    let in_features = t.shape[1];
    t.data
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let zeile = t.shifts[i / in_features];
            assert!(
                zeile <= MASTER_FRAC,
                "Zeilenverschiebung {zeile} ueberschreitet die Masterskala {MASTER_FRAC}"
            );
            i32::from(*w) << (MASTER_FRAC - zeile)
        })
        .collect()
}

// ⚑ **Die Zahl steht in der Bibliothek, nicht hier.** Sie geht in die
// Bitgleichheit ein: Zwei Miner mit verschiedenen Werten bekommen
// verschiedene Gewichte. Bis zum 2026-09-04 stand sie in **zwei**
// Testdateien, also genau die Lage, die dieses Projekt sonst durch einen
// Test verbindet.
use integer_llm_kernels::optimierer::MASTER_FRAC;

/// Trainiert den MLP-Block **einer** Ebene und gibt `(erster, letzter)`
/// Abstand zurueck.
fn lauf_auf_ebene(
    m: &IntegerModel,
    auf: &Zwischenwerte,
    e: usize,
    schritte: u64,
    vorzeichen: i64,
) -> (i64, i64) {
    let ebene = &auf.ebenen()[e];
    let Mlpteil::Dicht { .. } = &ebene.mlp else {
        panic!("Ebene {e} ist kein dichter Block");
    };
    let x = ebene.norm_mitte.clone();
    assert!(x.iter().any(|v| *v != 0), "Ebene {e}: der Eingang ist ueberall null");

    let Feedforward::Dense(mlp) = &m.layers[e].ffn else {
        panic!("Ebene {e} ist kein dichter Block");
    };
    let mut gate = master_aus_gewicht(&mlp.gate_proj);
    let mut up = master_aus_gewicht(&mlp.up_proj);
    let mut down = master_aus_gewicht(&mlp.down_proj);
    let hs = m.hidden_size;
    let is = mlp.gate_proj.shape[0];

    let sc = &m.layers[e].scales;
    let cfg = &m.config;
    let vorgaben = |schritt: u64, lr: i64| Mlpvorgaben {
        hidden_size: hs,
        intermediate_size: is,
        act_frac: sc.norm_mlp_frac,
        gate_frac: sc.gate_frac,
        up_frac: sc.up_frac,
        down_in_frac: sc.down_in_frac,
        // ⚑ Eine Skala fuer alle Kanaele: Die echte Ausgabeskala ist
        // per Kanal, `Mlpvorgaben` traegt eine. Das ist eine
        // Vereinfachung dieses Laufs und keine des Kerns; sie steht
        // hier, damit sie sichtbar ist.
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

    // Die typische Ausgabegroesse: Abstand gegen null, also die
    // Quadratsumme. ⚑ **Gemessen und nicht nachgebaut**: Ein Nachbau
    // waere eine zweite Umsetzung des Vorwaertspfades.
    let null = vec![0i16; hs];
    let (mut g0, mut u0, mut d0) = (gate.clone(), up.clone(), down.clone());
    let quadratsumme = schritt_auf_mlp(
        &mut g0, &mut u0, &mut d0, &x, &null, &m.silu_lut, &grad_lut, vorgaben(0, 0),
    );
    assert!(quadratsumme > 0, "Ebene {e}: der Block gibt ueberall null aus");
    let typisch = ((quadratsumme / hs as i64) as f64).sqrt() as i16;
    let ziel: Vec<i16> = (0..hs).map(|i| ((i as i16 % 3) - 1) * (typisch / 8).max(1)).collect();

    let mut erster = 0i64;
    let mut letzter = 0i64;
    for s in 0..schritte {
        let a = schritt_auf_mlp(
            &mut gate, &mut up, &mut down, &x, &ziel, &m.silu_lut, &grad_lut,
            vorgaben(s, vorzeichen),
        );
        if s == 0 {
            erster = a;
        }
        letzter = a;
    }
    (erster, letzter)
}

/// ⚑ **Ein MLP-Block eines echten Modells lernt ein verschobenes Ziel,
/// und zwar auf jeder geprueften Ebene.**
///
/// Das Ziel folgt aus der gemessenen Ausgabegroesse: **erreichbar und
/// nicht trivial.** Eine erfundene Zahlenreihe waere womoeglich mit
/// keinen Gewichten zu treffen, und der Lauf haette nichts gezeigt.
///
/// ⚑ **Mehrere Ebenen, weil sich die Gewichtsverteilung unterscheidet.**
/// Die erste sieht die Einbettung, die letzte haengt am Kopf, und
/// dazwischen liegen Ebenen mit Ausreissern, Nullzeilen und Spannen
/// ueber Groessenordnungen. **Genau daran bricht Ganzzahlarithmetik**,
/// wenn eine Skala nicht passt, und ein Lauf auf einer einzigen Ebene
/// sagt darueber nichts.
#[test]
fn jeder_geprüfte_mlp_block_lernt_sein_ziel() {
    let Some(m) = modell() else { return };
    let ebenen = m.num_layers;
    let mut cache = KVCache::for_range(0, ebenen, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    let start = m.embed_token(9707);
    let _ = m.run_layers_mit_mitschnitt(start, 0, &mut cache, 0, ebenen, &mut auf);

    eprintln!("\n=== Trainingslauf, {ebenen} Ebenen, Qwen2.5-0,5B ===");
    eprintln!("  Ebene | typische Ausgabe | Abstand vorher -> nachher | gefallen");
    // Erste, mittlere und letzte: die Enden der Skalenspanne.
    for e in [0usize, ebenen / 2, ebenen - 1] {
        let (erster, letzter) = lauf_auf_ebene(&m, &auf, e, SCHRITTE, 1);
        let gefallen = 100 - (letzter * 100 / erster.max(1));
        eprintln!("  {e:5} | {erster:>16} -> {letzter:>14} | {gefallen} Prozent");
        assert!(
            letzter * 2 < erster,
            "Ebene {e}: der Abstand fiel nur von {erster} auf {letzter}, \
             also um weniger als die Haelfte. Auf erfundenen Zahlen faellt er; \
             auf echten Gewichten mit Ausreissern offenbar nicht"
        );
    }
    eprintln!();
}

/// ⚑ **Und bergauf, wenn man das Vorzeichen dreht.**
///
/// ⛑ Ohne diese Gegenprobe bliebe offen, ob der Abstand faellt, weil der
/// Gradient stimmt, oder weil irgendeine Bewegung ihn faellt. **Mit
/// umgekehrtem Schritt muss er steigen**, und zwar auf derselben Ebene
/// mit demselben Ziel.
#[test]
fn mit_umgekehrtem_schritt_steigt_der_abstand() {
    let Some(m) = modell() else { return };
    let ebenen = m.num_layers;
    let mut cache = KVCache::for_range(0, ebenen, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    let start = m.embed_token(9707);
    let _ = m.run_layers_mit_mitschnitt(start, 0, &mut cache, 0, ebenen, &mut auf);

    let e = ebenen / 2;
    let (erster, letzter) = lauf_auf_ebene(&m, &auf, e, 20, -1);
    eprintln!("\n  bergauf auf Ebene {e}: {erster} -> {letzter}\n");
    assert!(
        letzter > erster,
        "mit umgekehrtem Schritt sank der Abstand von {erster} auf {letzter}; \
         dann faellt er nicht wegen des Gradienten"
    );
}


// ---- Der Aufmerksamkeitsblock ----------------------------------------

/// ⚑ **Der Weg vom Artefakt zum Master und zurück ist exakt.**
///
/// `master_aus_gewicht` und `gewicht_aus_master` sind Umkehrungen, und
/// das ist keine Feinheit: **Wenn sie es nicht wären, träte der erste
/// Trainingsschritt gegen ein anderes Modell an als das geladene**, und
/// jeder gemessene Abstand wäre der eines Modells, das niemand
/// ausgeliefert hat.
///
/// ⛑ **Vor Fund 174 galt das nur zufällig.** Damals stand in der
/// Zeilenverschiebung `s` statt `master_frac − s`; der Rundlauf traf,
/// solange das Betragsmaximum einer Zeile in der obersten Oktave lag,
/// und das tut es bei einem frisch quantisierten Artefakt. Nach dem
/// ersten Trainingsschritt nicht mehr.
#[test]
fn der_weg_vom_artefakt_zum_master_und_zurueck_ist_exakt() {
    let Some(m) = modell() else { return };
    let mut geprueft = 0usize;
    for e in [0usize, m.num_layers / 2, m.num_layers - 1] {
        let ebene = &m.layers[e];
        let Feedforward::Dense(mlp) = &ebene.ffn else {
            panic!("Ebene {e} ist kein dichter Block");
        };
        for (name, t) in [
            ("q_proj", &ebene.q_proj),
            ("k_proj", &ebene.k_proj),
            ("v_proj", &ebene.v_proj),
            ("o_proj", &ebene.o_proj),
            ("gate_proj", &mlp.gate_proj),
            ("up_proj", &mlp.up_proj),
            ("down_proj", &mlp.down_proj),
        ] {
            let master = master_aus_gewicht(t);
            let (w, shifts) = integer_llm_kernels::trainingsschritt::gewicht_aus_master(
                &master,
                t.shape[1],
                MASTER_FRAC,
            );
            assert_eq!(
                &shifts[..],
                &t.shifts[..],
                "Ebene {e}, {name}: die Zeilenverschiebungen kommen anders zurueck"
            );
            assert_eq!(
                &w[..],
                &t.data[..],
                "Ebene {e}, {name}: die Gewichte kommen anders zurueck"
            );
            geprueft += 1;
        }
    }
    assert_eq!(geprueft, 21, "der Test hat nicht alle Matrizen gesehen");
}

/// Die Folge, an der der Aufmerksamkeitsblock geprüft wird.
///
/// ⚑ **Mehr als eine Position, und das ist keine Bequemlichkeit.** Bei
/// einer einzigen Position gibt es genau einen Schlüssel, der Softmax
/// liefert exakt eins, und sein Gradient ist null: Q und K bekämen
/// nichts, und ein Lauf zeigte nur V und die Ausgabeprojektion.
const FOLGE: [usize; 6] = [9707, 374, 264, 1273, 315, 279];

/// Fährt die Folge durch das Modell und gibt den Mitschnitt zurück.
fn folge_mitschneiden(m: &IntegerModel) -> Zwischenwerte {
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    for (pos, tid) in FOLGE.iter().enumerate() {
        let start = m.embed_token(*tid);
        let _ = m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf);
    }
    auf
}

/// Die Vorgaben einer Ebene, aus den kalibrierten Skalen des Modells.
fn a_vorgaben(m: &IntegerModel, e: usize, schritt: u64, lr: i64) -> Aufmerksamkeitsvorgaben {
    let sc = &m.layers[e].scales;
    let cfg = &m.config;
    Aufmerksamkeitsvorgaben {
        hidden_size: m.hidden_size,
        num_heads: m.num_heads,
        num_kv_heads: m.num_kv_heads,
        head_dim: m.head_dim,
        act_frac: sc.norm_attn_frac,
        q_frac: sc.q_frac,
        k_frac: sc.k_frac,
        v_frac: sc.v_frac,
        attn_out_frac: sc.attn_out_frac,
        // ⚑ Eine Skala fuer alle Kanaele, dieselbe Vereinfachung wie im
        // MLP-Lauf: Die echte Ausgabeskala ist per Kanal.
        // **Fuer den Vergleich mit dem Mitschnitt ist sie ohne Belang**,
        // denn verglichen wird der Eingang der Ausgabeprojektion.
        aus_frac: sc.residual_mid_frac[0],
        master_frac: MASTER_FRAC,
        score_frac: cfg.score_frac_bits,
        prob_frac: cfg.prob_frac_bits,
        exp_input_frac: cfg.exp_input_frac,
        rope_frac: cfg.rope_frac_bits,
        positionsversatz: 0,
        lr_zaehler: lr,
        lr_nenner: 1 << 14,
        kennung: Schrittkennung { ebene: e as u32, schritt, index_versatz: 0 },
    }
}

/// ⚑ **Die zweite Umsetzung des Vorwärtspasses trifft die erste
/// byteweise.**
///
/// `vorwaerts_der_aufmerksamkeit` rechnet über eine **Folge**, die
/// Laufzeit über **eine Position mit Zwischenspeicher**. Das sind zwei
/// Wege durch dieselbe Rechnung, und zwei Wege laufen auseinander,
/// sobald niemand hinsieht: eine vertauschte Kopfgruppe, eine
/// vergessene Umskalierung, ein Positionsversatz, eine gedrehte
/// V-Reihe. **Verglichen wird der Eingang der Ausgabeprojektion**, denn
/// bis dorthin ist beides dieselbe Funktion; danach nicht mehr, weil die
/// Laufzeit eine Ausgabeskala je Kanal führt und dieser Block eine.
#[test]
fn die_vorwaerts_haelfte_trifft_den_mitschnitt() {
    let Some(m) = modell() else { return };
    assert!(
        m.layers[0].qk_norm.is_none(),
        "dieses Modell normiert die Koepfe vor RoPE; der Trainingsblock kann das nicht \
         und der Vergleich waere ein Vergleich zweier verschiedener Funktionen"
    );
    let auf = folge_mitschneiden(&m);
    let l_zahl = m.num_layers;

    for e in [0usize, l_zahl / 2, l_zahl - 1] {
        let ebene = &m.layers[e];
        let x: Vec<Vec<i16>> = (0..FOLGE.len())
            .map(|p| auf.ebenen()[p * l_zahl + e].norm_ein.clone())
            .collect();
        let erwartet: Vec<Vec<i16>> = (0..FOLGE.len())
            .map(|p| auf.ebenen()[p * l_zahl + e].attn_aus.clone())
            .collect();
        assert!(
            erwartet.iter().any(|z| z.iter().any(|w| *w != 0)),
            "Ebene {e}: der Mitschnitt der Aufmerksamkeitsausgabe ist ueberall null"
        );

        let vorsp = match (&ebene.q_bias, &ebene.k_bias, &ebene.v_bias) {
            (Some(q), Some(k), Some(v)) => Some(Vorspannungen {
                q: &q.data,
                q_skalen: &q.shifts,
                k: &k.data,
                k_skalen: &k.shifts,
                v: &v.data,
                v_skalen: &v.shifts,
            }),
            _ => None,
        };
        let spur = vorwaerts_der_aufmerksamkeit(
            Aufmerksamkeitsgewichte {
                q: &ebene.q_proj.data,
                q_skalen: &ebene.q_proj.shifts,
                k: &ebene.k_proj.data,
                k_skalen: &ebene.k_proj.shifts,
                v: &ebene.v_proj.data,
                v_skalen: &ebene.v_proj.shifts,
                o: &ebene.o_proj.data,
                o_skalen: &ebene.o_proj.shifts,
            },
            &x,
            vorsp,
            &m.cos_lut,
            &m.sin_lut,
            &m.exp_lut,
            None,
            a_vorgaben(&m, e, 0, 0),
        );

        for (p, (a, b)) in spur.attn_aus.iter().zip(erwartet.iter()).enumerate() {
            assert_eq!(
                a, b,
                "Ebene {e}, Position {p}: die Aufmerksamkeitsausgabe des Trainingsblocks \
                 weicht vom Mitschnitt des Inferenzpfades ab"
            );
        }
    }
}

/// ⚑ **Die ganze Ebene trifft den Mitschnitt byteweise.**
///
/// Der Vergleich der Aufmerksamkeitshälfte lässt vier Stellen aus, an
/// denen eine Ebene bricht und kein Blocktest hinsieht: die beiden
/// **Normierungen** und die beiden **Residualadditionen**. Hier läuft
/// die vollständige Ebene, und verglichen wird ihr Ausgang mit dem
/// **Eingang der nächsten Ebene** aus dem Mitschnitt eines echten
/// Vorwärtspasses.
///
/// ⚑ **Das ist der Test, der den Zusammenbau trägt.** Alles andere an
/// 2d ist an erfundenen Zahlen geprüft; dies ist die Stelle, an der die
/// zweite Umsetzung an die erste gebunden wird.
#[test]
fn die_ganze_ebene_trifft_den_mitschnitt() {
    let Some(m) = modell() else { return };
    assert!(
        m.layers[0].qk_norm.is_none(),
        "dieses Modell normiert die Koepfe vor RoPE; der Trainingsblock kann das nicht"
    );
    let auf = folge_mitschneiden(&m);
    let l_zahl = m.num_layers;
    let grad_lut = integer_llm_kernels::backward::silu_grad_aus_lut(&m.silu_lut);
    let mut geprueft = 0usize;

    // Die letzte Ebene bleibt aussen vor: Ihr Ausgang ist der finale
    // Residualstrom und steht in keinem `residual_ein`.
    for e in [0usize, l_zahl / 2, l_zahl - 2] {
        let ebene = &m.layers[e];
        let sc = &ebene.scales;
        let naechste = &m.layers[e + 1].scales;
        let Feedforward::Dense(mlp) = &ebene.ffn else {
            panic!("Ebene {e} ist kein dichter Block");
        };


        let hidden: Vec<Vec<i16>> = (0..FOLGE.len())
            .map(|p| auf.ebenen()[p * l_zahl + e].residual_ein.clone())
            .collect();
        let erwartet: Vec<Vec<i16>> = (0..FOLGE.len())
            .map(|p| auf.ebenen()[p * l_zahl + e + 1].residual_ein.clone())
            .collect();

        let mut vg = a_vorgaben(&m, e, 0, 0);
        vg.act_frac = sc.norm_attn_frac;
        let mut mg = Mlpvorgaben {
            hidden_size: m.hidden_size,
            intermediate_size: mlp.gate_proj.shape[0],
            act_frac: sc.norm_mlp_frac,
            gate_frac: sc.gate_frac,
            up_frac: sc.up_frac,
            down_in_frac: sc.down_in_frac,
            aus_frac: 0,
            master_frac: MASTER_FRAC,
            silu_in_frac: m.config.silu_in_frac,
            silu_lut_offset: m.config.silu_lut_offset,
            silu_out_frac: m.config.silu_out_frac,
            lr_zaehler: 0,
            lr_nenner: 1,
            kennung: Schrittkennung { ebene: e as u32, schritt: 0, index_versatz: 0 },
        };
        mg.aus_frac = 0;
        let v = Ebenenvorgaben {
            aufmerksamkeit: vg,
            mlp: mg,
            residual_in_frac: &sc.residual_in_frac,
            residual_mid_frac: &sc.residual_mid_frac,
            aus_frac: &naechste.residual_in_frac,
            rsqrt_input_shift: m.config.rsqrt_input_shift,
            rsqrt_output_frac: m.config.rsqrt_output_frac,
            inv_n_q20: m.inv_n_q20,
        };

        let vorsp = match (&ebene.q_bias, &ebene.k_bias, &ebene.v_bias) {
            (Some(q), Some(k), Some(vv)) => Some(Vorspannungen {
                q: &q.data, q_skalen: &q.shifts,
                k: &k.data, k_skalen: &k.shifts,
                v: &vv.data, v_skalen: &vv.shifts,
            }),
            _ => None,
        };
        let spur = vorwaerts_der_ebene(
            Ebenengewichte {
                aufmerksamkeit: Aufmerksamkeitsgewichte {
                    q: &ebene.q_proj.data, q_skalen: &ebene.q_proj.shifts,
                    k: &ebene.k_proj.data, k_skalen: &ebene.k_proj.shifts,
                    v: &ebene.v_proj.data, v_skalen: &ebene.v_proj.shifts,
                    o: &ebene.o_proj.data, o_skalen: &ebene.o_proj.shifts,
                },
                gate: &mlp.gate_proj.data, gate_skalen: &mlp.gate_proj.shifts,
                up: &mlp.up_proj.data, up_skalen: &mlp.up_proj.shifts,
                down: &mlp.down_proj.data, down_skalen: &mlp.down_proj.shifts,
                gamma_ein: &ebene.input_layernorm_gamma.data,
                gamma_ein_skalen: &ebene.input_layernorm_gamma.shifts,
                gamma_mitte: &ebene.post_attention_layernorm_gamma.data,
                gamma_mitte_skalen: &ebene.post_attention_layernorm_gamma.shifts,
            },
            &hidden,
            vorsp,
            Ebenentabellen {
                cos: &m.cos_lut, sin: &m.sin_lut, exp: &m.exp_lut,
                rsqrt: &m.rsqrt_lut, silu: &m.silu_lut, silu_grad: &grad_lut,
            },
            v,
        );

        for (p, (a, b)) in spur.y.iter().zip(erwartet.iter()).enumerate() {
            assert_eq!(
                a, b,
                "Ebene {e}, Position {p}: die Ausgabe der Ebene weicht vom Eingang der \
                 naechsten Ebene im Mitschnitt ab"
            );
        }
        geprueft += 1;
    }
    assert_eq!(geprueft, 3, "es sollten drei Ebenen verglichen werden");
    eprintln!("\n  {geprueft} Ebenen byteweise verglichen\n");
}

/// Trainiert den Aufmerksamkeitsblock **einer** Ebene und gibt
/// `(erster, letzter)` Abstand zurueck.
fn a_lauf_auf_ebene(
    m: &IntegerModel,
    auf: &Zwischenwerte,
    e: usize,
    schritte: u64,
    vorzeichen: i64,
) -> (i64, i64) {
    a_lauf_auf_ebene_lr(m, auf, e, schritte, vorzeichen, A_NENNER)
}

fn a_lauf_auf_ebene_lr(
    m: &IntegerModel,
    auf: &Zwischenwerte,
    e: usize,
    schritte: u64,
    vorzeichen: i64,
    nenner: i64,
) -> (i64, i64) {
    let l_zahl = m.num_layers;
    let ebene = &m.layers[e];
    let x: Vec<Vec<i16>> = (0..FOLGE.len())
        .map(|p| auf.ebenen()[p * l_zahl + e].norm_ein.clone())
        .collect();

    let mut q = master_aus_gewicht(&ebene.q_proj);
    let mut k = master_aus_gewicht(&ebene.k_proj);
    let mut v = master_aus_gewicht(&ebene.v_proj);
    let mut o = master_aus_gewicht(&ebene.o_proj);
    let vorsp = match (&ebene.q_bias, &ebene.k_bias, &ebene.v_bias) {
        (Some(qb), Some(kb), Some(vb)) => Some(Vorspannungen {
            q: &qb.data,
            q_skalen: &qb.shifts,
            k: &kb.data,
            k_skalen: &kb.shifts,
            v: &vb.data,
            v_skalen: &vb.shifts,
        }),
        _ => None,
    };

    // Die typische Ausgabegroesse, gemessen statt nachgebaut: Abstand
    // gegen null ist die Quadratsumme.
    let hs = m.hidden_size;
    let null: Vec<Vec<i16>> = (0..FOLGE.len()).map(|_| vec![0i16; hs]).collect();
    let (mut q0, mut k0, mut v0, mut o0) = (q.clone(), k.clone(), v.clone(), o.clone());
    let quadratsumme = schritt_auf_aufmerksamkeit(
        &mut q0, &mut k0, &mut v0, &mut o0, &x, &null, vorsp, &m.cos_lut, &m.sin_lut,
        &m.exp_lut, a_vorgaben(m, e, 0, 0),
    );
    assert!(quadratsumme > 0, "Ebene {e}: der Block gibt ueberall null aus");
    let typisch = ((quadratsumme / (FOLGE.len() * hs) as i64) as f64).sqrt() as i16;
    let ziel: Vec<Vec<i16>> = (0..FOLGE.len())
        .map(|t| {
            (0..hs)
                .map(|i| ((i as i16 % 3) - 1) * (typisch / 8).max(1) - (t as i16))
                .collect()
        })
        .collect();

    let mut erster = 0i64;
    let mut letzter = 0i64;
    for s in 0..schritte {
        let a = schritt_auf_aufmerksamkeit(
            &mut q, &mut k, &mut v, &mut o, &x, &ziel, vorsp, &m.cos_lut, &m.sin_lut,
            &m.exp_lut, Aufmerksamkeitsvorgaben { lr_nenner: nenner, ..a_vorgaben(m, e, s, vorzeichen) },
        );
        if s == 0 {
            erster = a;
        }
        letzter = a;
    }
    (erster, letzter)
}

/// ⚑ **Ein Aufmerksamkeitsblock eines echten Modells lernt ein
/// verschobenes Ziel.**
///
/// Dieselbe Aussage wie beim MLP-Block und aus demselben Grund an
/// echten Gewichten: Erfundene Gewichte sind gleichmaessig, echte haben
/// Ausreisser, Nullzeilen und eine Spanne ueber Groessenordnungen.
#[test]
fn der_aufmerksamkeitsblock_lernt_sein_ziel() {
    let Some(m) = modell() else { return };
    let auf = folge_mitschneiden(&m);
    let ebenen = m.num_layers;

    eprintln!("\n=== Aufmerksamkeitsblock, {} Positionen, Qwen2.5-0,5B ===", FOLGE.len());
    for e in [0usize, ebenen / 2] {
        let (erster, letzter) = a_lauf_auf_ebene(&m, &auf, e, A_SCHRITTE, 1);
        let gefallen = 100 - (letzter * 100 / erster.max(1));
        eprintln!("  Ebene {e:2}: {erster:>14} -> {letzter:>14} | {gefallen} Prozent");
        assert!(
            letzter * 2 < erster,
            "Ebene {e}: der Abstand fiel nur von {erster} auf {letzter}, also um weniger \
             als die Haelfte"
        );
    }
    eprintln!();
}

/// ⚑ **Und bergauf, wenn man das Vorzeichen dreht.**
#[test]
fn mit_umgekehrtem_schritt_steigt_der_abstand_der_aufmerksamkeit() {
    let Some(m) = modell() else { return };
    let auf = folge_mitschneiden(&m);
    let e = m.num_layers / 2;
    let (erster, letzter) = a_lauf_auf_ebene(&m, &auf, e, 20, -1);
    eprintln!("\n  bergauf auf Ebene {e}: {erster} -> {letzter}\n");
    assert!(
        letzter > erster,
        "mit umgekehrtem Schritt sank der Abstand von {erster} auf {letzter}; \
         dann faellt er nicht wegen des Gradienten"
    );
}
