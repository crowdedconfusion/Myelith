//! Sonde: Wie weit ist der Router von seinem absorbierenden Zustand
//! entfernt (Fund 79)?
//!
//! # ⚑ Warum diese Messung nötig ist
//!
//! `backward::moe_backward` beschreibt einen Zustand, aus dem ein Router
//! nicht mehr herauskommt: **Der Ganzzahl-Softmax sättigt.** Trägt der
//! Gewinner exakt `1 << prob_frac_bits`, ist der Gradient **jedes**
//! Routerlogits exakt null, und zwar aus Rechengründen, nicht durch
//! Rundung:
//!
//! ```text
//! out_i = ( (g_i − Σ_j g_j p_j / 2^frac) · p_i ) >> frac
//! ```
//!
//! Für einen Verlierer ist `p_i = 0`, für den Gewinner ist die Klammer
//! null. **In Gleitkomma gibt es diesen Zustand nicht**; er entsteht
//! erst durch die Ganzzahltabelle und ist damit ein Preis dieses
//! Projekts, keine Eigenschaft von MoE.
//!
//! ⚑ **Bis zum 2026-09-05 war das eine Herleitung ohne Zahl.** Ob der
//! Zustand real eintritt, hängt am Modell, und die Frage entscheidet
//! mit, ob das Primärmodell ein Expertengemisch werden kann.
//!
//! # Gemessen an Qwen3-30B-A3B (2026-09-05)
//!
//! 720 Routerstellen, 15 Token über 48 MoE-Ebenen:
//!
//! | | |
//! |---|---|
//! | voll gesättigt | **0 von 720** |
//! | Gewicht auf null | **0 von 5760** |
//! | kleinstes Mischgewicht | 24 von 16384 (Mittel 1068) |
//! | grösstes Mischgewicht | 16107 von 16384 |
//! | Logitspanne über alle 128 | grösste 22294, mittlere 13996 |
//! | Abstand bis zur Sättigung | rund **41167** Einheiten |
//!
//! ⚑ **Der Abstand bis zur Sättigung ist etwa doppelt so gross wie die
//! gesamte beobachtete Logitspanne.** Am Ausgangspunkt ist der Zustand
//! also weit weg.
//!
//! ⚑ **Und das ist eine Aussage über den Ausgangspunkt, nicht über die
//! Bahn.** Training treibt Routerlogits auseinander; genau darauf läuft
//! ein Lauf zu. Was diese Messung nicht beantwortet, beantwortet erst
//! ein echter MoE-Trainingslauf.
//!
//! ⚑ **Zweitens, und ohne Sättigung:** Das grösste Mischgewicht liegt
//! bereits bei 98,3 Prozent. Der Routergradient trägt den Faktor
//! `p_i · (1 − p_i)`; dort ist er klein, bevor irgendetwas sättigt.
//!
//! Aufruf: `router_saettigung <artefaktordner> [prompt]`

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::Feedforward;
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&args[1]);
    let prompt = args
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or("Die Hauptstadt von Frankreich ist Paris, und die von Italien ist");
    eprintln!("[sonde] lade {} ...", dir.display());
    let m = load_model(dir).expect("Modell");
    let tok = Tokenizer::from_file(&dir.join("tokenizer.json").display().to_string())
        .expect("Wortschatz");
    let ids = tok.encode(prompt);
    let cfg = &m.config;
    eprintln!("[sonde] {} Token, {} Ebenen", ids.len(), m.num_layers);

    // --- 1. Die Skalen --------------------------------------------------
    let mut fracs: Vec<u8> = m
        .layers
        .iter()
        .filter_map(|l| match &l.ffn {
            Feedforward::Moe(moe) => Some(moe.router_frac),
            _ => None,
        })
        .collect();
    let moe_ebenen = fracs.len();
    fracs.sort_unstable();
    fracs.dedup();
    println!("\n=== Skalen ===");
    println!("MoE-Ebenen: {moe_ebenen} von {}", m.num_layers);
    println!("router_frac: {fracs:?}   exp_input_frac: {}", cfg.exp_input_frac);
    for rf in &fracs {
        let ist = rf.saturating_sub(cfg.exp_input_frac);
        let soll = i32::from(*rf) - i32::from(cfg.exp_input_frac);
        let warnung = if soll < 0 {
            "  <== SAETTIGT: die exp-Tabelle wird zu flach gelesen"
        } else {
            ""
        };
        println!("  router_frac {rf}: exp_lut_shift = {ist} (rechnerisch {soll}){warnung}");
    }
    let eins = 1i32 << cfg.prob_frac_bits;
    println!("prob_frac_bits: {}  (eins = {eins})", cfg.prob_frac_bits);

    // --- 2. Der Lauf ----------------------------------------------------
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut stellen = 0usize;
    let mut voll_gesaettigt = 0usize;
    let mut mit_nullgewicht = 0usize;
    let mut gewichte_gesamt = 0usize;
    let mut nullgewichte = 0usize;
    let mut summen_fehl = 0usize;
    let mut spanne_max = 0i32;
    let mut spanne_summe = 0i64;

    for (pos, t) in ids.iter().enumerate() {
        let (_logits, befunde) = m.forward_token_mit_routing(*t, pos, &mut cache);
        for b in &befunde {
            stellen += 1;
            spanne_max = spanne_max.max(b.logit_spanne);
            spanne_summe += i64::from(b.logit_spanne);
            let summe: i32 = b.gewichte.iter().sum();
            if summe != eins {
                summen_fehl += 1;
            }
            let nullen = b.gewichte.iter().filter(|g| **g == 0).count();
            nullgewichte += nullen;
            gewichte_gesamt += b.gewichte.len();
            if nullen > 0 {
                mit_nullgewicht += 1;
            }
            // ⚑ Der absorbierende Zustand: ein Gewicht traegt alles.
            if b.gewichte.contains(&eins) {
                voll_gesaettigt += 1;
                if voll_gesaettigt <= 3 {
                    println!(
                        "  gesaettigt: Position {pos}, Ebene {}, Gewichte {:?}, Spanne {}",
                        b.layer, b.gewichte, b.logit_spanne
                    );
                }
            }
        }
    }

    println!("\n=== Router-Saettigung (Fund 79) ===");
    println!("geprueft: {stellen} Routerstellen ({} Token x {moe_ebenen} Ebenen)", ids.len());
    let anteil = |a: usize, b: usize| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
    println!(
        "voll gesaettigt (ein Gewicht = {eins}, Routergradient exakt null): {voll_gesaettigt} \
         ({:.2} %)",
        anteil(voll_gesaettigt, stellen)
    );
    println!(
        "mindestens ein Gewicht auf null: {mit_nullgewicht} Stellen ({:.2} %), \
         {nullgewichte} von {gewichte_gesamt} Gewichten ({:.2} %)",
        anteil(mit_nullgewicht, stellen),
        anteil(nullgewichte, gewichte_gesamt)
    );
    println!("Gewichtssumme nicht exakt eins: {summen_fehl} Stellen");
    println!(
        "Logitspanne ueber ALLE Experten: groesste {spanne_max}, mittlere {:.1}",
        spanne_summe as f64 / stellen.max(1) as f64
    );

    // --- 3. Der Abstand bis zum absorbierenden Zustand -------------------
    //
    // ⚑ Bei `norm_topk_prob` rechnet der Softmax ueber die GEWAEHLTEN
    // Experten, nicht ueber alle. Fuer die Saettigung zaehlt deshalb der
    // Abstand INNERHALB der Top-k, und der ist viel kleiner als die
    // Spanne oben.
    let mut kleinstes_gewicht = i32::MAX;
    let mut groesstes_gewicht = 0i32;
    let mut summe_kleinstes = 0i64;
    let mut cache2 = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut histogramm = [0usize; 6];
    for (pos, t) in ids.iter().enumerate() {
        let (_l, befunde) = m.forward_token_mit_routing(*t, pos, &mut cache2);
        for b in &befunde {
            let klein = b.gewichte.iter().copied().min().unwrap_or(0);
            let gross = b.gewichte.iter().copied().max().unwrap_or(0);
            kleinstes_gewicht = kleinstes_gewicht.min(klein);
            groesstes_gewicht = groesstes_gewicht.max(gross);
            summe_kleinstes += i64::from(klein);
            // Wie nah ist das kleinste Gewicht an der Rundungsgrenze?
            let stufe = match klein {
                0 => 0,
                1..=9 => 1,
                10..=99 => 2,
                100..=999 => 3,
                1000..=4999 => 4,
                _ => 5,
            };
            histogramm[stufe] += 1;
        }
    }
    println!("\n=== Der Abstand bis zum absorbierenden Zustand ===");
    println!(
        "kleinstes Mischgewicht ueberhaupt: {kleinstes_gewicht} von {eins}           (Mittel {:.1})",
        summe_kleinstes as f64 / stellen.max(1) as f64
    );
    println!("groesstes Mischgewicht: {groesstes_gewicht} von {eins}");
    let namen = ["= 0 (tot)", "1 bis 9", "10 bis 99", "100 bis 999", "1000 bis 4999", "ab 5000"];
    for (i, n) in namen.iter().enumerate() {
        println!("  kleinstes Gewicht {n:>14}: {:>4} Stellen", histogramm[i]);
    }
    // ⚑ Wieviel Logitabstand braucht es, damit ein Gewicht auf null faellt?
    // exp(-d/2^exp_input_frac) * exp_lut[0] < 0,5 (die Rundungsgrenze),
    // umgerechnet in die Skala des Routers.
    let lut_eins = f64::from(m.exp_lut[0]);
    let d_lut = (lut_eins * 2.0 * f64::from(eins)).ln() * f64::from(1u32 << cfg.exp_input_frac);
    let schieben = fracs[0] - cfg.exp_input_frac;
    println!(
        "\nrechnerisch: ein gewaehlter Experte faellt auf null, wenn sein Logit um rund \
         {:.0} Einheiten (Skala Q{}) unter dem groessten der Gewaehlten liegt",
        d_lut * f64::from(1u32 << schieben),
        fracs[0]
    );
}
