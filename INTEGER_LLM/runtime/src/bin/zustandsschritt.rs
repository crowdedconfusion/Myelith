//! **Trainingsschritte ueber einen Ebenenbereich mit Zustandsebenen**, am
//! echten Modell: sinkt der Abstand zu einem Ziel, und bewegen sich die
//! Matrizen der Zustandsschicht?
//!
//! Aufruf:
//!     zustandsschritt <artefakt> [von bis] [schritte] [lr_nenner]
//!
//! Eingang ist der Strom der Inferenz vor Ebene `von`. Das Ziel ist der
//! Ausgang des Bereichs vor dem ersten Schritt, je Kanal um eine feste
//! Stufe verschoben; der Verlust ist der quadratische Abstand dazu, der
//! Gradient `2 (y - ziel)` auf der Ausgangsskala, wie in den Proben mit
//! Vorlage.
//!
//! Mit `NORMIERT=1` wie im Messwerkzeug des Trainings: gesammelt, je Zeile
//! normiert, `lr_nenner` ist dann die Zahl der Schritte, in denen das
//! groesste Gewicht einer Zeile eine Rasterstufe wandert.
//!
//! ⚑ **Ein Abstand, der sinkt, und Matrizen, die sich bewegen**, sind die
//! beiden Aussagen. Die Richtigkeit der Gradienten pruefen
//! `zustandsrueckprobe` und die Proben der Kerne.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::shardtraining::{
    rueckwaerts, rueckwaerts_mit, sammlung_anwenden, vorwaerts, zeilenbreiten_gemisch, Ebenenstand, Fortschreibung,
    Mischerstand, Sammlung, Shardgewichte, Shardvorgaben,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("[zustandsschritt] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err("Usage: zustandsschritt <artefakt> [von bis] [schritte] [lr_nenner]".into());
    }
    let zahl = |i: usize, d: usize| -> usize { args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d) };
    let m = load_model(std::path::Path::new(&args[1])).map_err(|e| format!("Laden: {e}"))?;
    let (von, bis) = (zahl(2, 0), zahl(3, 4));
    let schritte = zahl(4, 4) as u64;
    let lr_nenner = zahl(5, 4096) as i64;
    let tokens = [785usize, 3974, 374, 279, 6722, 315, 9625, 30, 576, 4226, 374];

    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    let eingang: Vec<Vec<i16>> =
        tokens.iter().enumerate().map(|(p, t)| m.run_layers(m.embed_token(*t), p, &mut cache, 0, von)).collect();
    let mut g = Shardgewichte::aus_modell(&m, von, bis).map_err(|e| format!("Gewichte: {e}"))?;
    let zustandsebenen = g
        .anfangsstand()
        .iter()
        .filter(|s| matches!(s, Ebenenstand::Gemisch { mischer: Mischerstand::Zustand(_), .. }))
        .count();
    println!(
        "[zustandsschritt] Ebenen {von}..{bis}, davon {zustandsebenen} Zustandsebenen | {} Token | {schritte} Schritte | Nenner {lr_nenner}",
        tokens.len()
    );

    let mut ziel: Option<Vec<Vec<i16>>> = None;
    let mut erster = 0i64;
    for schritt in 0..=schritte {
        let vg = Shardvorgaben { von, bis, schritt, lr_zaehler: 1, lr_nenner };
        let ms = vorwaerts(&m, &mut g, &vg, &eingang).map_err(|e| format!("vorwaerts: {e}"))?;
        let ziel = ziel.get_or_insert_with(|| {
            ms.ausgang
                .iter()
                .map(|y| y.iter().enumerate().map(|(i, &v)| v.saturating_add(if i % 2 == 0 { 64 } else { -64 })).collect())
                .collect()
        });
        let mut abstand = 0i64;
        let g_aus: Vec<Vec<i32>> = ms
            .ausgang
            .iter()
            .zip(ziel.iter())
            .map(|(y, z)| {
                y.iter()
                    .zip(z)
                    .map(|(a, b)| {
                        let d = i64::from(*a) - i64::from(*b);
                        abstand += d * d;
                        (2 * d).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
                    })
                    .collect()
            })
            .collect();
        if schritt == 0 {
            erster = abstand;
        }
        print!("[zustandsschritt] Schritt {schritt}: Abstand {abstand}");
        if schritt == schritte {
            println!();
            break;
        }
        if std::env::var_os("NORMIERT").is_some() {
            let mut sammlung = Sammlung::normiert();
            let (moe_is, experten) = m
                .layers
                .iter()
                .find_map(|l| match &l.ffn {
                    integer_llm_runtime::model::Feedforward::Moe(g) => Some((g.experts[0].gate_proj.shape[0], g.experts.len())),
                    _ => None,
                })
                .unwrap_or((0, 0));
            sammlung.zeilenbreiten_setzen(zeilenbreiten_gemisch(&m, moe_is, experten));
            rueckwaerts_mit(&m, &mut g, &vg, &ms, &g_aus, &mut Fortschreibung::Sammeln(&mut sammlung))
                .map_err(|e| format!("rueckwaerts: {e}"))?;
            sammlung.folge_fertig();
            let erg = sammlung_anwenden(&sammlung, &mut g, &vg);
            println!("  | bewegt {} von {}", erg.bewegte_gewichte, erg.gewichte_gesamt);
        } else {
            let erg = rueckwaerts(&m, &mut g, &vg, &ms, &g_aus).map_err(|e| format!("rueckwaerts: {e}"))?;
            println!("  | bewegt {} von {}", erg.bewegte_gewichte, erg.gewichte_gesamt);
        }
    }

    // Je Zustandsebene: welche ihrer sechs Matrizen sich bewegt haben.
    let namen = ["in_proj_qkv", "in_proj_z", "in_proj_b", "in_proj_a", "out_proj", "conv1d"];
    for (i, (a, j)) in g.anfangsstand().iter().zip(g.master.iter()).enumerate() {
        if let (
            Ebenenstand::Gemisch { mischer: Mischerstand::Zustand(a), .. },
            Ebenenstand::Gemisch { mischer: Mischerstand::Zustand(j), .. },
        ) = (a, j)
        {
            let bewegt: Vec<String> = namen
                .iter()
                .zip(a.iter().zip(j.iter()))
                .map(|(n, (x, y))| format!("{n} {}", x.iter().zip(y).filter(|(p, q)| p != q).count()))
                .collect();
            println!("[zustandsschritt] Ebene {}: {}", von + i, bewegt.join(", "));
        }
    }
    let letzter = {
        let vg = Shardvorgaben { von, bis, schritt: schritte, lr_zaehler: 1, lr_nenner };
        let ms = vorwaerts(&m, &mut g, &vg, &eingang).map_err(|e| format!("vorwaerts: {e}"))?;
        let ziel = ziel.as_ref().expect("gesetzt");
        ms.ausgang
            .iter()
            .zip(ziel)
            .flat_map(|(y, z)| y.iter().zip(z).map(|(a, b)| (i64::from(*a) - i64::from(*b)).pow(2)))
            .sum::<i64>()
    };
    println!(
        "[zustandsschritt] Abstand {erster} -> {letzter} ({})",
        if letzter < erster { "gesunken" } else { "NICHT gesunken" }
    );
    println!("[zustandsschritt] Fertig");
    Ok(())
}
