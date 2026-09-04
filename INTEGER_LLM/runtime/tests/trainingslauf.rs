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
//! **Und der Aufmerksamkeitsblock.** Sein Rückwärtspass ist noch nicht
//! gebaut; der Mitschnitt hat die Werte dafür, die Verdrahtung fehlt.
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
use integer_llm_kernels::trainingsschritt::{schritt_auf_mlp, Mlpvorgaben};
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
        .map(|(i, w)| (i32::from(*w)) << t.shifts[i / in_features])
        .collect()
}

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
