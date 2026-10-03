//! **Der Rueckwaertspass einer Zustandsebene am echten Modell**: sagt der
//! Gradient die Aenderung des Verlusts voraus, wenn sich ein Gewicht oder
//! der Eingang aendert?
//!
//! Aufruf:
//!     zustandsrueckprobe <artefakt> [ebene] [token ...]
//!
//! Der Verlust ist `L = Summe w * y` ueber die Ausgabe `y` der Ebene (vor
//! der Residualaddition), mit festen Zufallsgewichten `w`. Je Groesse wird
//! ein Schritt gegen den Gradienten gesetzt und die Aenderung mittig
//! gemessen, `(L(+d) - L(-d)) / 2`, am Vorwaertspass der Inferenz. Gemeldet
//! wird `gemessen / vorhergesagt`.
//!
//! ⚑ **Am echten Modell und nicht nur an Kernen**, denn die Kerne sind je
//! fuer sich geprueft; was hier zaehlt, sind die Verbindungen dazwischen:
//! Skalen, Drehungen, die Zuordnung der Koepfe, die Reihenfolge der Kanaele.
//!
//! # ⚑ Warum der Schritt so gewaehlt ist (gemessen am 35B, 2026-10-02)
//!
//! Der Vorwaertspass ist ganzzahlig, und an drei Stellen antwortet er auf
//! einen kleinen Schritt nicht so, wie seine Ableitung sagt:
//!
//! 1. **Die SiLU-Tabelle hat flache Enden mit einzelnen Stufen.** Bei
//!    `z = -8,4` springt sie von 0 auf -1/256, und ihre Ableitung (die
//!    mittige Differenz der Tabelle) ist dort 0,125 statt 0,0013. Eine
//!    Zeile von `in_proj_z`, deren `z` an einem Token dort liegt, sagt eine
//!    grosse Aenderung voraus, die der Vorwaertspass an genau einer Stufe
//!    einloest oder nicht (Fund 519).
//! 2. **Leise Wertkoepfe** werden von der torgesteuerten Norm aufgeblasen
//!    (Fund 418, 419); ihr Ausgang ist dort Rundung, und die reagiert auf
//!    jede kleine Aenderung sprunghaft.
//! 3. **Ein dichter Schritt gegen den Gradienten ist kein kleiner
//!    Schritt.** Eine Einheit je Eintrag, ueber 2048 Eingaenge gleichsinnig,
//!    verschiebt eine Projektion um ein Vielfaches davon; am Eingang ergab
//!    das 0,66 bis 0,80, mit einem Drittel der Eintraege 1,06 bis 1,08.
//!
//! Daher die Voreinstellung: **das oberste Prozent der Betraege auslassen**
//! (dort sitzen die Stufen aus 1.) und **hoechstens ein Achtel der Eintraege**
//! verschieben (3.). Zufaellige Richtungen (`ZUFAELLIG=1`) heben sich ueber
//! viele Eintraege zum grossen Teil auf und gehen dann in diesen Stufen
//! unter; sie sind fuer den Schnitt nicht brauchbar.
//!
//! ⚠️ **`dt_bias` und `exp_A` sind hier nicht messbar.** Es sind 32 Werte,
//! einer je Wertkopf, und ihre Wirkung ueber elf Token liegt unter dem
//! Rauschen aus 2. Sie werden gemeldet, nicht bewertet. Ihr Weg ist an den
//! Kernen geprueft (`die_tore_sagen_die_aenderung_voraus`), und `dt_bias`
//! bekommt denselben Gradienten wie `in_proj_a`, dessen Zeile hier zaehlt.
//!
//! ⚑ **Das Band ist 0,8 bis 1,25 und nicht enger**, und das ist gemessen:
//! Ueber die Ebenen 0 und 4 des 35B lagen alle bewerteten Groessen
//! zwischen 0,86 und 1,22, ohne Richtung (der Eingang einmal darueber,
//! einmal darunter, je nach Schrittweite). **Was diese Probe faengt, sind
//! Fehler der Verdrahtung**: ein vergessener Faktor zwei, ein gedrehtes
//! Vorzeichen, ein Kopf an der falschen Stelle, eine fehlende Drehung. Sie
//! zeigen sich als 0,5, -1 oder 0, nicht als zehn Prozent. Die Genauigkeit
//! jedes Kerns pruefen dessen eigene Proben auf drei Prozent.
//!
//! Stellschrauben: `AUSWAHL`, `AUSLASSEN_PROMILLE`, `SCHRITT`, `W_BITS`,
//! `ZUFAELLIG`, `ZEILEN` (je Zeile von `in_proj_z`), `JE_EINTRAG` (je Kopf
//! fuer die beiden Vorspanne).
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::{akkumulationsskala_mischer, Gewichtsdaten, IntegerModel, Mischer, QTensor};
use integer_llm_runtime::zustandstraining::{rueckwaerts, Zustandsgradienten};

/// Die Zustandsschicht der Ebene, wie das Artefakt sie traegt.
fn zs_von(m: &IntegerModel, ebene: usize) -> &integer_llm_runtime::model::Zustandsschicht {
    let Mischer::Zustand(zs) = &m.layers[ebene].mischer else { unreachable!("vorher geprueft") };
    zs
}

/// Ein fester Zufall, damit jeder Lauf dieselben Zahlen sieht.
struct Zufall(u64);
impl Zufall {
    fn zahl(&mut self, von: i64, bis: i64) -> i64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        von + (self.0 % (bis - von + 1) as u64) as i64
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[zustandsrueckprobe] FEHLER: {e}");
        std::process::exit(1);
    }
}

/// Welche Groesse veraendert wird.
#[derive(Clone, Copy, Debug)]
enum Groesse {
    Eingang,
    InProjQkv,
    InProjZ,
    InProjB,
    InProjA,
    OutProj,
    Faltung,
    NormGamma,
    DtBias,
    ExpA,
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err("Usage: zustandsrueckprobe <artefakt> [ebene] [token ...]".into());
    }
    let mut m = load_model(std::path::Path::new(&args[1])).map_err(|e| format!("Laden: {e}"))?;
    let ebene: usize = args.get(2).map(|s| s.parse().map_err(|_| "ebene ist keine Zahl")).transpose()?.unwrap_or(0);
    let mut tokens: Vec<usize> = args.iter().skip(3).filter_map(|t| t.parse().ok()).collect();
    if tokens.is_empty() {
        tokens = vec![785, 3974, 374, 279, 6722, 315, 9625, 30, 576, 4226, 374];
    }
    if !matches!(m.layers.get(ebene).map(|l| &l.mischer), Some(Mischer::Zustand(_))) {
        return Err(format!("Ebene {ebene} ist keine Zustandsebene"));
    }

    // Der Eingang der Ebene: der Strom der Inferenz davor, normiert.
    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    let normen: Vec<Vec<i16>> = tokens
        .iter()
        .enumerate()
        .map(|(p, t)| {
            let x = m.run_layers(m.embed_token(*t), p, &mut cache, 0, ebene);
            m.norm_vor_aufmerksamkeit(&m.layers[ebene], &x)
        })
        .collect();
    let acc = akkumulationsskala_mischer(&m.layers[ebene].scales);
    let ein_frac = m.layers[ebene].scales.norm_attn_frac;
    println!(
        "[zustandsrueckprobe] Ebene {ebene} | {} Token | Eingang auf {ein_frac} Bruchbits",
        tokens.len()
    );

    let mut z = Zufall(0x9e37_79b9_7f4a_7c15);
    let breite = acc.len();
    // ⚠️ **Nicht zu gross**: Die Gradienten laufen als i32, und ein
    // gesaettigter Gradient sagt eine zu kleine Aenderung voraus.
    let w_bits: u32 = std::env::var("W_BITS").ok().and_then(|s| s.parse().ok()).unwrap_or(12);
    let w: Vec<Vec<i64>> = (0..tokens.len()).map(|_| (0..breite).map(|_| z.zahl(-(1 << w_bits), 1 << w_bits)).collect()).collect();

    let verlust = |m: &IntegerModel, normen: &[Vec<i16>]| -> f64 {
        let zeilen: Vec<&[i16]> = normen.iter().map(Vec::as_slice).collect();
        let (y, _) = m.zustandsschicht_mit_spur(&m.layers[ebene], zs_von(m, ebene), &zeilen, &acc);
        y.iter()
            .zip(&w)
            .map(|(y, w)| {
                y.iter().zip(w).zip(&acc).map(|((a, b), f)| (i64::from(*a) * b) as f64 / 2f64.powi(i32::from(*f))).sum::<f64>()
            })
            .sum()
    };

    // Vorwaerts mit Spur und rueckwaerts, Bus 0: der Gradient nach `y` ist `w`.
    let zeilen: Vec<&[i16]> = normen.iter().map(Vec::as_slice).collect();
    let start = std::time::Instant::now();
    let (_, spur) = m.zustandsschicht_mit_spur(&m.layers[ebene], zs_von(&m, ebene), &zeilen, &acc);
    {
        let satt = |v: &[Vec<i16>]| v.iter().flatten().filter(|&&x| x == i16::MAX || x == i16::MIN).count();
        let alle = |v: &[Vec<i16>]| v.iter().map(Vec::len).sum::<usize>();
        println!(
            "[zustandsrueckprobe] gesaettigt: getort {}/{}, normiert {}/{}, roh {}/{}, gefaltet {}/{}, z {}/{}",
            satt(&spur.getort), alle(&spur.getort), satt(&spur.normiert), alle(&spur.normiert),
            satt(&spur.roh_aus), alle(&spur.roh_aus), satt(&spur.gefaltet), alle(&spur.gefaltet),
            satt(&spur.z), alle(&spur.z)
        );
        let z_max = spur.z.iter().flatten().map(|x| x.unsigned_abs()).max().unwrap_or(0);
        println!(
            "[zustandsrueckprobe] z: |max| {z_max} auf {} Bruchbits, SiLU-Tabelle {} Bruchbits Eingang, Versatz {}",
            { let Mischer::Zustand(zs) = &m.layers[ebene].mischer else { unreachable!() }; zs.skalen.z_frac }, m.config.silu_in_frac, m.config.silu_lut_offset
        );
    }
    let d_aus: Vec<Vec<i32>> = w.iter().map(|z| z.iter().map(|&v| v as i32).collect()).collect();
    let gr = rueckwaerts(&m, &m.layers[ebene], zs_von(&m, ebene), &spur, &d_aus, 0, 4);
    println!("[zustandsrueckprobe] rueckwaerts in {:.2} s", start.elapsed().as_secs_f64());
    let satt = gr.eingang.iter().flatten().filter(|&&v| v == i32::MAX || v == i32::MIN).count();
    let groesst = gr.eingang.iter().flatten().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    println!("[zustandsrueckprobe] Eingangsgradient: groesster Betrag {groesst}, gesaettigt {satt}");

    let mut schlecht = 0;
    for groesse in [
        Groesse::Eingang,
        Groesse::InProjQkv,
        Groesse::InProjZ,
        Groesse::InProjB,
        Groesse::InProjA,
        Groesse::OutProj,
        Groesse::Faltung,
        Groesse::NormGamma,
        Groesse::DtBias,
        Groesse::ExpA,
    ] {
        let richtungen: &[bool] = if std::env::var_os("ZUFAELLIG").is_some() { &[true, false] } else { &[false] };
        for &zufaellig in richtungen {
            let r = pruefen(&mut m, ebene, &normen, &gr, groesse, zufaellig, ein_frac, &verlust, &mut z);
            let art = if zufaellig { "zufaellig" } else { "abwaerts " };
            let bewertet = !zufaellig && !matches!(groesse, Groesse::DtBias | Groesse::ExpA);
            let ok = (0.8..=1.25).contains(&r);
            if bewertet && !ok {
                schlecht += 1;
            }
            println!(
                "[zustandsrueckprobe] {groesse:<10?} {art}  gemessen / vorhergesagt = {r:.4}{}",
                match (bewertet, ok) {
                    (false, _) => "  (nur gemeldet)",
                    (true, true) => "",
                    (true, false) => "  <-- ausserhalb 0,8 bis 1,25",
                }
            );
        }
    }
    println!("[zustandsrueckprobe] {schlecht} der bewerteten ausserhalb");
    println!("[zustandsrueckprobe] Fertig");
    Ok(())
}

/// Eine Groesse, eine Richtung: verschieben, mittig messen, vorhersagen.
#[allow(clippy::too_many_arguments)]
fn pruefen(
    m: &mut IntegerModel,
    ebene: usize,
    normen: &[Vec<i16>],
    gr: &Zustandsgradienten,
    groesse: Groesse,
    zufaellig: bool,
    ein_frac: u8,
    verlust: &dyn Fn(&IntegerModel, &[Vec<i16>]) -> f64,
    z: &mut Zufall,
) -> f64 {
    // Die Gradienten der Groesse, flach, und je Eintrag die Zweierpotenz,
    // mit der `grad * schritt` zur Aenderung von L wird.
    let (grads, potenzen): (Vec<i64>, Vec<i32>) = {
        let Mischer::Zustand(zs) = &m.layers[ebene].mischer else { unreachable!() };
        let matrix = |t: &QTensor, g: &[i64], x_frac: u8| -> (Vec<i64>, Vec<i32>) {
            let (_, xf) = t.eingang(&vec![0i16; t.cols()], x_frac);
            let p = (0..g.len()).map(|i| -(i32::from(xf) + i32::from(t.shifts[i / t.cols()]))).collect();
            (g.to_vec(), p)
        };
        match groesse {
            Groesse::Eingang => (
                gr.eingang.iter().flatten().map(|&v| i64::from(v)).collect(),
                vec![-2 * i32::from(ein_frac); gr.eingang.iter().map(Vec::len).sum()],
            ),
            Groesse::InProjQkv => matrix(&zs.in_proj_qkv, &gr.in_proj_qkv, ein_frac),
            Groesse::InProjZ => matrix(&zs.in_proj_z, &gr.in_proj_z, ein_frac),
            Groesse::InProjB => matrix(&zs.in_proj_b, &gr.in_proj_b, ein_frac),
            Groesse::InProjA => matrix(&zs.in_proj_a, &gr.in_proj_a, ein_frac),
            Groesse::OutProj => matrix(&zs.out_proj, &gr.out_proj, zs.skalen.norm_aus_frac),
            Groesse::Faltung => (
                gr.faltung.clone(),
                (0..gr.faltung.len()).map(|i| -i32::from(zs.conv1d.shifts[i / 4]) - i32::from(zs.skalen.qkv_frac)).collect(),
            ),
            Groesse::NormGamma => {
                (gr.norm_gamma.clone(), zs.norm_gamma.shifts.iter().map(|&s| -i32::from(s)).collect())
            }
            Groesse::DtBias => (gr.dt_bias.clone(), zs.dt_bias.shifts.iter().map(|&s| -i32::from(s)).collect()),
            Groesse::ExpA => (gr.exp_a.clone(), zs.exp_a.shifts.iter().map(|&s| -i32::from(s)).collect()),
        }
    };
    // Der Schritt: nach dem Betrag des Gradienten geordnet, das oberste
    // `AUSLASSEN_PROMILLE` ausgelassen, dann `AUSWAHL` Eintraege (Begruendung
    // im Kopf dieser Datei).
    //
    // ⚑ **Die Einheit ist so gross, dass der reelle Schritt die Rundung
    // dahinter uebersteigt**: Bei den Vorspannen geht der Wert auf das
    // Raster des Softplus-Eingangs, und ein Schritt darunter aendert nichts.
    let auswahl: usize = std::env::var("AUSWAHL").ok().and_then(|s| s.parse().ok()).unwrap_or(grads.len() / 8).max(1);
    let faktor: i64 = std::env::var("SCHRITT").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let einheit: Vec<i64> = match groesse {
        Groesse::DtBias | Groesse::ExpA => potenzen.iter().map(|p| (1i64 << (-p - 6).max(0)).min(1 << 12)).collect(),
        _ => vec![1; grads.len()],
    };
    let mut reihenfolge: Vec<usize> = (0..grads.len()).collect();
    reihenfolge.sort_by_key(|&i| std::cmp::Reverse(grads[i].abs()));
    let mut schritt = vec![0i64; grads.len()];
    let auslassen: usize = std::env::var("AUSLASSEN_PROMILLE").ok().and_then(|s| s.parse::<usize>().ok()).map_or(grads.len() / 100, |p| grads.len() * p / 1000);
    for &i in reihenfolge.iter().skip(auslassen).take(auswahl) {
        if grads[i] != 0 {
            schritt[i] = faktor * einheit[i] * if zufaellig { if z.zahl(0, 1) == 0 { -1 } else { 1 } } else { -grads[i].signum() };
        }
    }
    let vorher: f64 = grads
        .iter()
        .zip(&schritt)
        .zip(&potenzen)
        .map(|((g, d), p)| (*g as f64) * (*d as f64) * 2f64.powi(*p))
        .sum();

    if std::env::var_os("ZEILEN").is_some() && matches!(groesse, Groesse::InProjZ) && !zufaellig {
        let Mischer::Zustand(zs) = &m.layers[ebene].mischer else { unreachable!() };
        let cols = zs.in_proj_z.cols();
        let zeilen = grads.len() / cols;
        let mut summen: Vec<(i64, usize)> =
            (0..zeilen).map(|r| (grads[r * cols..(r + 1) * cols].iter().map(|g| g.abs()).sum(), r)).collect();
        summen.sort_by_key(|s| std::cmp::Reverse(s.0));
        let auswahl: Vec<usize> = summen.iter().take(6).map(|s| s.1).chain([summen[zeilen / 2].1, summen[zeilen - 40].1]).collect();
        for r in auswahl {
            let mut einzeln = vec![0i64; grads.len()];
            einzeln[r * cols..(r + 1) * cols].copy_from_slice(&schritt[r * cols..(r + 1) * cols]);
            let v: f64 = (r * cols..(r + 1) * cols)
                .map(|i| (grads[i] as f64) * (einzeln[i] as f64) * 2f64.powi(potenzen[i]))
                .sum();
            let g = (messen(m, ebene, normen, groesse, &einzeln, 1, verlust)
                - messen(m, ebene, normen, groesse, &einzeln, -1, verlust))
                / 2.0;
            println!("    Zeile {r} (Kopf {}): vorhergesagt {v:+.4e} gemessen {g:+.4e} -> {:.3}", r / 128, g / v);
            let zeilen_ein: Vec<&[i16]> = normen.iter().map(Vec::as_slice).collect();
            let acc = akkumulationsskala_mischer(&m.layers[ebene].scales);
            let (_, s0) = m.zustandsschicht_mit_spur(&m.layers[ebene], zs_von(m, ebene), &zeilen_ein, &acc);
            let alt = vertauschen(m, ebene, groesse, &einzeln, 1);
            let (_, s1) = m.zustandsschicht_mit_spur(&m.layers[ebene], zs_von(m, ebene), &zeilen_ein, &acc);
            zurueck(m, ebene, groesse, alt);
            for t in 0..normen.len().min(4) {
                println!(
                    "      t{t} z {} -> {} | normiert {} | getort {} -> {}",
                    s0.z[t][r], s1.z[t][r], s0.normiert[t][r], s0.getort[t][r], s1.getort[t][r]
                );
            }
        }
    }
    if std::env::var_os("JE_EINTRAG").is_some() && matches!(groesse, Groesse::DtBias | Groesse::ExpA) && !zufaellig {
        let Mischer::Zustand(zs) = &m.layers[ebene].mischer else { unreachable!() };
        let (daten, shifts) = if matches!(groesse, Groesse::DtBias) {
            (zs.dt_bias.data.clone(), zs.dt_bias.shifts.clone())
        } else {
            (zs.exp_a.data.clone(), zs.exp_a.shifts.clone())
        };
        for i in 0..grads.len() {
            let mut einzeln = vec![0i64; grads.len()];
            einzeln[i] = schritt[i];
            let v = (grads[i] as f64) * (schritt[i] as f64) * 2f64.powi(potenzen[i]);
            let g = (messen(m, ebene, normen, groesse, &einzeln, 1, verlust)
                - messen(m, ebene, normen, groesse, &einzeln, -1, verlust))
                / 2.0;
            println!(
                "    {groesse:?}[{i}] wert {} shift {} schritt {} grad {} vorhergesagt {v:+.4e} gemessen {g:+.4e}",
                daten[i], shifts[i], schritt[i], grads[i]
            );
        }
    }
    let gemessen = (messen(m, ebene, normen, groesse, &schritt, 1, verlust)
        - messen(m, ebene, normen, groesse, &schritt, -1, verlust))
        / 2.0;
    println!("    vorhergesagt {vorher:+.6e}  gemessen {gemessen:+.6e}");
    gemessen / vorher
}

/// Verschiebt die Groesse um `vz * schritt`, misst L und stellt wieder her.
fn messen(
    m: &mut IntegerModel,
    ebene: usize,
    normen: &[Vec<i16>],
    groesse: Groesse,
    schritt: &[i64],
    vz: i64,
    verlust: &dyn Fn(&IntegerModel, &[Vec<i16>]) -> f64,
) -> f64 {
    if let Groesse::Eingang = groesse {
        let mut it = schritt.iter();
        let neu: Vec<Vec<i16>> = normen
            .iter()
            .map(|z| z.iter().map(|&x| (i64::from(x) + vz * it.next().unwrap()).clamp(-32768, 32767) as i16).collect())
            .collect();
        return verlust(m, &neu);
    }
    let alt = vertauschen(m, ebene, groesse, schritt, vz);
    let l = verlust(m, normen);
    zurueck(m, ebene, groesse, alt);
    l
}

/// Was vor einer Verschiebung da war.
enum Alt {
    Matrix(std::sync::Arc<Gewichtsdaten>),
    Werte(Vec<i16>),
}

fn mit_schritt_i8(alt: &[i8], schritt: &[i64], vz: i64) -> Vec<i8> {
    alt.iter().zip(schritt).map(|(w, d)| (i64::from(*w) + vz * d).clamp(-127, 127) as i8).collect()
}

fn vertauschen(m: &mut IntegerModel, ebene: usize, groesse: Groesse, schritt: &[i64], vz: i64) -> Alt {
    let Mischer::Zustand(zs) = &mut m.layers[ebene].mischer else { unreachable!() };
    let tausch = |t: &mut QTensor| -> Alt {
        let neu = mit_schritt_i8(&t.data, schritt, vz);
        Alt::Matrix(std::mem::replace(&mut t.data, std::sync::Arc::new(Gewichtsdaten::Speicher(neu))))
    };
    match groesse {
        Groesse::InProjQkv => tausch(&mut zs.in_proj_qkv),
        Groesse::InProjZ => tausch(&mut zs.in_proj_z),
        Groesse::InProjB => tausch(&mut zs.in_proj_b),
        Groesse::InProjA => tausch(&mut zs.in_proj_a),
        Groesse::OutProj => tausch(&mut zs.out_proj),
        Groesse::Faltung => tausch(&mut zs.conv1d),
        Groesse::NormGamma => tausch(&mut zs.norm_gamma),
        Groesse::DtBias | Groesse::ExpA => {
            let v = if matches!(groesse, Groesse::DtBias) { &mut zs.dt_bias.data } else { &mut zs.exp_a.data };
            let neu = v.iter().zip(schritt).map(|(w, d)| (i64::from(*w) + vz * d) as i16).collect();
            Alt::Werte(std::mem::replace(v, neu))
        }
        Groesse::Eingang => unreachable!(),
    }
}

fn zurueck(m: &mut IntegerModel, ebene: usize, groesse: Groesse, alt: Alt) {
    let Mischer::Zustand(zs) = &mut m.layers[ebene].mischer else { unreachable!() };
    match (groesse, alt) {
        (Groesse::InProjQkv, Alt::Matrix(a)) => zs.in_proj_qkv.data = a,
        (Groesse::InProjZ, Alt::Matrix(a)) => zs.in_proj_z.data = a,
        (Groesse::InProjB, Alt::Matrix(a)) => zs.in_proj_b.data = a,
        (Groesse::InProjA, Alt::Matrix(a)) => zs.in_proj_a.data = a,
        (Groesse::OutProj, Alt::Matrix(a)) => zs.out_proj.data = a,
        (Groesse::Faltung, Alt::Matrix(a)) => zs.conv1d.data = a,
        (Groesse::NormGamma, Alt::Matrix(a)) => zs.norm_gamma.data = a,
        (Groesse::DtBias, Alt::Werte(v)) => zs.dt_bias.data = v,
        (Groesse::ExpA, Alt::Werte(v)) => zs.exp_a.data = v,
        _ => unreachable!("Verschiebung und Wiederherstellung passen nicht zusammen"),
    }
}
