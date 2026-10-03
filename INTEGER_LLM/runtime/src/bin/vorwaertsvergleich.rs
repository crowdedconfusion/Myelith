//! Vorwaertsvergleich: Rechnet der Vorwaertspass des Trainings dasselbe
//! Modell wie die Inferenz, am echten Artefakt?
//!
//! Usage: vorwaertsvergleich <artifact_dir> <tokendatei> [folgen] [von bis]
//!
//! `<tokendatei>`: eine Folge je Zeile, Tokennummern durch Leerzeichen
//! (`trainingsguete` schreibt sie mit `TOKENS_NACH`); `-` nimmt einen festen
//! Text, mit dem Wortschatz des Modells kodiert.
//!
//! **Mit `von bis`** wird nur dieser Ebenenbereich verglichen: Eingang ist
//! der Strom der Inferenz vor Ebene `von`, Vergleich der vor Ebene `bis`.
//! So laesst sich eine einzelne Ebene eines Modells pruefen, dessen
//! uebrige Ebenen der Trainingspfad nicht traegt (etwa die Achtsamkeitsebene
//! eines Hybrids zwischen seinen Zustandsebenen).
//!
//! ⚑ **Die Frage hinter der Gegenprobe zum Stand im Artefakt** (2026-10-02):
//! Auf denselben 26 Haltefolgen meldete das Training 2 835,0458 und die
//! Inferenz am geschriebenen Artefakt 2 835,1754; am unveraenderten int8-0,6B
//! ebenso 32,2222 gegen 32,2241. Zwei Erklaerungen: Die Ebenen rechnen
//! verschieden (dann traeniert das Training ein anderes Modell), oder nur der
//! Kopf misst mit einer anderen Zahl von Bruchstellen (das Training mit 16,
//! die Inferenz mit `config.logit_frac_bits`).
//!
//! Je Folge: der Residualstrom hinter der letzten Ebene aus beiden Wegen,
//! Wert fuer Wert verglichen; dann die Perplexitaet aus **beiden** Stroemen
//! mit demselben Kopf, einmal mit den Bruchstellen der Inferenz und einmal
//! mit denen des Trainings.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::messung::{kreuzentropie_aus_logits, perplexitaet};
use integer_llm_runtime::shardtraining::{vorwaerts, Shardgewichte, Shardvorgaben};
use integer_llm_runtime::trainingsschleife::Trainingsvorgaben;

fn main() {
    if let Err(e) = run() {
        eprintln!("[vorwaertsvergleich] FEHLER: {e}");
        std::process::exit(1);
    }
}

/// Vergleicht nur die Ebenen `von..bis`, mit dem Strom der Inferenz als
/// Eingang.
fn bereich_vergleichen(
    m: &integer_llm_runtime::model::IntegerModel,
    text: &str,
    hoechstens: usize,
    von: usize,
    bis: usize,
) -> Result<(), String> {
    if von >= bis || bis > m.num_layers {
        return Err(format!("Bereich {von}..{bis} passt nicht zu {} Ebenen", m.num_layers));
    }
    let folgen: Vec<Vec<usize>> = text
        .lines()
        .map(|z| z.split_whitespace().filter_map(|t| t.parse().ok()).collect::<Vec<usize>>())
        .filter(|f| f.len() >= 2)
        .take(hoechstens)
        .collect();
    let mut gewichte = Shardgewichte::aus_modell(m, von, bis).map_err(|e| format!("Gewichte: {e}"))?;
    println!(
        "[vorwaertsvergleich] Ebenen {von}..{bis} | {} Folgen | Form {:?} | Tor {} | Drehbreite {} von {}",
        folgen.len(),
        gewichte.form(),
        m.achtsamkeit_mit_tor,
        m.drehbreite,
        m.head_dim
    );
    let (mut verschieden, mut werte) = (0usize, 0usize);
    let mut erste: Option<(usize, usize)> = None;
    for (fi, folge) in folgen.iter().enumerate() {
        let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
        let mut eingang = Vec::with_capacity(folge.len());
        let mut erwartet = Vec::with_capacity(folge.len());
        for (p, t) in folge.iter().enumerate() {
            let x = m.run_layers(m.embed_token(*t), p, &mut cache, 0, von);
            erwartet.push(m.run_layers(x.clone(), p, &mut cache, von, bis));
            eingang.push(x);
        }
        let vg = Shardvorgaben { von, bis, schritt: 0, lr_zaehler: 1, lr_nenner: 64 };
        let ms = vorwaerts(m, &mut gewichte, &vg, &eingang).map_err(|e| format!("vorwaerts: {e}"))?;
        if fi == 0 {
            zerlegen(m, &eingang[0], von, &ms);
        }
        let training = ms.ausgang;
        for (p, (a, b)) in training.iter().zip(&erwartet).enumerate() {
            werte += a.len();
            let n = a.iter().zip(b).filter(|(x, y)| x != y).count();
            verschieden += n;
            if n > 0 && erste.is_none() {
                erste = Some((fi, p));
            }
        }
    }
    println!(
        "[vorwaertsvergleich] Strom hinter Ebene {}: {verschieden} von {werte} Werten verschieden{}",
        bis - 1,
        match erste {
            Some((f, p)) => format!(", zuerst Folge {f} Position {p}"),
            None => String::new(),
        }
    );
    println!("[vorwaertsvergleich] Fertig");
    Ok(())
}

/// **Die Zerlegung der ersten Ebene an Position 0**: je Zwischenstufe, wie
/// viele Werte die Spur des Trainings anders hat als der Mitschnitt der
/// Inferenz. Die erste abweichende Stufe ist die Stelle.
fn zerlegen(
    m: &integer_llm_runtime::model::IntegerModel,
    x0: &[i16],
    von: usize,
    ms: &integer_llm_runtime::shardtraining::Shardmitschnitt,
) {
    use integer_llm_runtime::shardtraining::Ebenenmitschnitt;
    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    let mut auf = integer_llm_runtime::mitschnitt::Zwischenwerte::neu();
    let _ = m.run_layers_mit_mitschnitt(x0.to_vec(), 0, &mut cache, von, von + 1, &mut auf);
    let Some(inf) = auf.ebenen().first() else {
        println!("[vorwaertsvergleich] Zerlegung: kein Mitschnitt");
        return;
    };
    let Ebenenmitschnitt::Dicht(sp) = &ms.spuren[0] else {
        println!("[vorwaertsvergleich] Zerlegung: nur fuer dichte Ebenen");
        return;
    };
    let zaehle = |a: &[i16], b: &[i16]| -> String {
        if a.len() != b.len() {
            return format!("Laenge {} gegen {}", a.len(), b.len());
        }
        format!("{} von {}", a.iter().zip(b).filter(|(x, y)| x != y).count(), a.len())
    };
    let flach = |v: &[Vec<i16>]| v.concat();
    println!("[vorwaertsvergleich] Zerlegung Ebene {von}, Position 0 (Training gegen Inferenz):");
    println!("  norm_ein       {}", zaehle(&sp.norm_ein[0], &inf.norm_ein));
    println!("  q (gedreht)    {}", zaehle(&flach(&sp.aufmerksamkeit.q[0]), &flach(&inf.q)));
    println!("  k (gedreht)    {}", zaehle(&flach(&sp.aufmerksamkeit.k[0]), &flach(&inf.k)));
    println!("  v              {}", zaehle(&flach(&sp.aufmerksamkeit.v[0]), &flach(&inf.v)));
    let vor_tor = sp.aufmerksamkeit.vor_tor.first().unwrap_or(&sp.aufmerksamkeit.attn_aus[0]);
    println!("  attn vor Tor   {}", zaehle(vor_tor, &inf.attn_aus));
    println!("  attn nach Tor  {}", zaehle(&sp.aufmerksamkeit.attn_aus[0], &inf.attn_aus));
    println!("  residual_mitte {}", zaehle(&sp.residual[0], &inf.residual_mitte));
    println!("  norm_mitte     {}", zaehle(&sp.norm_mitte[0], &inf.norm_mitte));
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("Usage: vorwaertsvergleich <artifact_dir> <tokendatei> [folgen]".into());
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let hoechstens: usize = args.get(3).map(|s| s.parse().map_err(|_| format!("'{s}'"))).transpose()?.unwrap_or(usize::MAX);
    let m = load_model(&dir).map_err(|e| format!("Modell-Ladung: {e}"))?;
    let text = if args[2] == "-" {
        let pfad = dir.join("tokenizer.json");
        let wortschatz = integer_llm_runtime::tokenizer::Tokenizer::from_file(pfad.to_str().ok_or("Pfad ist kein UTF-8")?)?;
        let ids = wortschatz.encode(
            "Die Halle in Rostock wurde 1923 gebaut und 2019 umgebaut. Sie fasst 1 200 Menschen, \
             und im Winter wird sie mit Fernwaerme beheizt.",
        );
        ids.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" ")
    } else {
        std::fs::read_to_string(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?
    };
    if let (Some(von), Some(bis)) = (args.get(4), args.get(5)) {
        let von: usize = von.parse().map_err(|_| format!("von: '{von}'"))?;
        let bis: usize = bis.parse().map_err(|_| format!("bis: '{bis}'"))?;
        return bereich_vergleichen(&m, &text, hoechstens, von, bis);
    }
    let folgen: Vec<Vec<usize>> = text
        .lines()
        .map(|z| z.split_whitespace().filter_map(|t| t.parse().ok()).collect::<Vec<usize>>())
        .filter(|f| f.len() >= 2)
        .take(hoechstens)
        .collect();
    let mut gewichte = Shardgewichte::aus_modell(&m, 0, m.num_layers).map_err(|e| format!("Gewichte: {e}"))?;
    let frac_training = Trainingsvorgaben::vorgabe().logit_frac;
    let frac_inferenz = m.config.logit_frac_bits;
    println!(
        "[vorwaertsvergleich] {} | {} Folgen | Form {:?} | Kopf: Inferenz {frac_inferenz} Bruchstellen, Training {frac_training}",
        dir.display(),
        folgen.len(),
        gewichte.form()
    );

    let mut abweichende_werte = 0usize;
    let mut werte = 0usize;
    let mut erste: Option<(usize, usize)> = None;
    // (Strom, Bruchstellen) -> (Summe, Zahl)
    let mut summen = [[(0.0f64, 0usize); 2]; 2];
    for (fi, folge) in folgen.iter().enumerate() {
        let strom: Vec<Vec<i16>> = folge.iter().map(|t| m.embed_token(*t)).collect();
        let vg = Shardvorgaben { von: 0, bis: m.num_layers, schritt: 0, lr_zaehler: 1, lr_nenner: 64 };
        let training = vorwaerts(&m, &mut gewichte, &vg, &strom).map_err(|e| format!("vorwaerts: {e}"))?.ausgang;
        let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
        let inferenz = m.vorbereiten_stapel(folge, 0, &mut cache);
        for (p, (a, b)) in training.iter().zip(&inferenz).enumerate() {
            werte += a.len();
            let n = a.iter().zip(b).filter(|(x, y)| x != y).count();
            abweichende_werte += n;
            if n > 0 && erste.is_none() {
                erste = Some((fi, p));
            }
        }
        if training.len() != inferenz.len() {
            return Err(format!("Folge {fi}: {} gegen {} Positionen", training.len(), inferenz.len()));
        }
        for (si, stroeme) in [&training, &inferenz].into_iter().enumerate() {
            for (bi, frac) in [frac_inferenz, frac_training].into_iter().enumerate() {
                for p in 0..folge.len() - 1 {
                    let l = m.head_logits_mit_spur(&stroeme[p], frac, None);
                    let v = kreuzentropie_aus_logits(&l, folge[p + 1], frac);
                    if v.is_finite() {
                        summen[si][bi].0 += v;
                        summen[si][bi].1 += 1;
                    }
                }
            }
        }
    }
    println!(
        "[vorwaertsvergleich] Strom hinter der letzten Ebene: {abweichende_werte} von {werte} Werten verschieden{}",
        match erste {
            Some((f, p)) => format!(", zuerst Folge {f} Position {p}"),
            None => String::new(),
        }
    );
    for (si, name) in ["Training", "Inferenz"].iter().enumerate() {
        for (bi, frac) in [frac_inferenz, frac_training].iter().enumerate() {
            let (s, n) = summen[si][bi];
            println!("[vorwaertsvergleich] Strom {name:<9} Kopf mit {frac:>2} Bruchstellen: Perplexitaet {:.4} ({n} Positionen)", perplexitaet(s, n));
        }
    }
    println!("[vorwaertsvergleich] Fertig");
    Ok(())
}
