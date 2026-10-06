//! **Was die Experten eines Gemischs zu sehen bekommen**, als Dateien, fuer
//! eine datenabhaengige Rundung.
//!
//! Usage: expertenmitschnitt <artefakt> <textdatei> <ziel> [--token N] [--folge L]
//!
//! Der Text laeuft Token fuer Token durch das Modell, in Folgen zu `L` Token
//! (Vorgabe 512) mit frischem Cache je Folge, bis `N` Token (Vorgabe 8 192)
//! gerechnet sind. Je Gemischebene `e` entstehen drei Dateien, je Token ein
//! Eintrag, little-endian:
//!
//! | Datei | je Token |
//! |---|---|
//! | `x_<e>.bin` | der normierte Eingang des Gemischblocks (`norm_mitte`), `hidden` mal i16 |
//! | `wahl_<e>.bin` | die gewaehlten Experten in Auswahlreihenfolge, `top_k` mal u16 |
//! | `h_<e>.bin` | je gewaehltem Experten der Eingang seiner `down`-Projektion, `top_k` mal `moe_intermediate` mal i16 |
//!
//! Dazu `kopf.json` mit den Massen. Gate und Up eines Experten sehen `x`
//! der Token, die ihn gewaehlt haben; `down` sieht sein `h`.
//!
//! ⚑ **Der Mitschnitt des Trainings, nicht ein zweiter Weg.** Die Werte
//! kommen aus `run_layers_mit_mitschnitt`, demselben Vorwaertspass, den die
//! Konformitaetsvektoren belegen.
//!
//! Kein Teil des Auslieferungspfads.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::{Mlpteil, Zwischenwerte};
use integer_llm_runtime::tokenizer::Tokenizer;
use std::io::Write;
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("[expertenmitschnitt] FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err("Usage: expertenmitschnitt <artefakt> <textdatei> <ziel> [--token N] [--folge L]".into());
    }
    let (dir, text, ziel) = (Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]));
    let mut gesamt = 8192usize;
    let mut folge = 512usize;
    let mut i = 4;
    while i < args.len() {
        let wert = || -> Result<usize, String> {
            args.get(i + 1).and_then(|s| s.parse().ok()).ok_or_else(|| format!("{} braucht eine Zahl", args[i]))
        };
        match args[i].as_str() {
            "--token" => gesamt = wert()?,
            "--folge" => folge = wert()?,
            a => return Err(format!("unbekannter Schalter {a}")),
        }
        i += 2;
    }
    if ziel.exists() {
        return Err(format!("{} gibt es schon; hier wird nichts ueberschrieben", ziel.display()));
    }
    let m = load_model(dir)?;
    let tok = Tokenizer::from_file(dir.join("tokenizer.json").to_str().ok_or("Pfad")?)?;
    let inhalt = std::fs::read_to_string(text).map_err(|e| format!("{}: {e}", text.display()))?;
    let ids = tok.encode(&inhalt);
    if ids.len() < gesamt {
        return Err(format!("der Text ergibt {} Token, verlangt sind {gesamt}", ids.len()));
    }
    std::fs::create_dir_all(ziel).map_err(|e| format!("{}: {e}", ziel.display()))?;

    let mut dateien: std::collections::BTreeMap<usize, [std::io::BufWriter<std::fs::File>; 3]> = Default::default();
    let mut masse: Option<(usize, usize, usize)> = None;
    let start = std::time::Instant::now();
    let mut auf = Zwischenwerte::default();
    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    for (n, &tk) in ids.iter().take(gesamt).enumerate() {
        let pos = n % folge;
        if pos == 0 {
            cache = KVCache::new(m.num_layers, m.num_kv_heads);
        }
        auf.leeren();
        let _ = m.run_layers_mit_mitschnitt(m.embed_token(tk), pos, &mut cache, 0, m.num_layers, &mut auf);
        for (e, ebene) in auf.ebenen().iter().enumerate() {
            let Mlpteil::Expertengemisch { experten, teile, .. } = &ebene.mlp else { continue };
            let h_breite = teile.first().map(|t| t.h.len()).unwrap_or(0);
            let jetzt = (ebene.norm_mitte.len(), experten.len(), h_breite);
            if *masse.get_or_insert(jetzt) != jetzt {
                return Err(format!("Ebene {e}: Masse {jetzt:?} statt {masse:?}"));
            }
            if let std::collections::btree_map::Entry::Vacant(platz) = dateien.entry(e) {
                let neu = |stamm: &str| -> Result<std::io::BufWriter<std::fs::File>, String> {
                    let p = ziel.join(format!("{stamm}_{e}.bin"));
                    std::fs::File::create(&p).map(std::io::BufWriter::new).map_err(|f| format!("{}: {f}", p.display()))
                };
                platz.insert([neu("x")?, neu("wahl")?, neu("h")?]);
            }
            let [fx, fw, fh] = dateien.get_mut(&e).expect("eben angelegt");
            let schreib = |f: &mut std::io::BufWriter<std::fs::File>, b: &[u8]| f.write_all(b).map_err(|x| x.to_string());
            for v in &ebene.norm_mitte {
                schreib(fx, &v.to_le_bytes())?;
            }
            for nr in experten {
                schreib(fw, &nr.to_le_bytes())?;
            }
            for t in teile {
                for v in &t.h {
                    schreib(fh, &v.to_le_bytes())?;
                }
            }
        }
        if (n + 1) % 256 == 0 || n + 1 == gesamt {
            let s = start.elapsed().as_secs_f64();
            println!(
                "[expertenmitschnitt] {} von {gesamt} Token, {:.1} Token/s, noch etwa {:.0} min",
                n + 1,
                (n + 1) as f64 / s,
                (gesamt - n - 1) as f64 * s / (n + 1) as f64 / 60.0
            );
        }
    }
    for fs in dateien.values_mut() {
        for f in fs.iter_mut() {
            f.flush().map_err(|e| e.to_string())?;
        }
    }
    let (hidden, top_k, h_breite) = masse.ok_or("das Modell hat keine Gemischebene")?;
    let kopf = serde_json::json!({
        "token": gesamt, "folge": folge, "hidden": hidden, "top_k": top_k,
        "moe_intermediate": h_breite, "ebenen": dateien.keys().collect::<Vec<_>>(),
        "artefakt": dir.file_name().map(|n| n.to_string_lossy().to_string()),
    });
    std::fs::write(ziel.join("kopf.json"), serde_json::to_vec_pretty(&kopf).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("Fertig");
    Ok(())
}
