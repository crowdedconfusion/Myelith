//! **Die Experten eines Gemischs ternaer machen**, im int8-Format.
//!
//! # Wozu
//!
//! Ein Expertengemisch traegt fast alle Gewichte in den gerouteten
//! Experten (beim 35B rund 92 %). Gerade sie vertragen eine grobe Form am
//! besten, waehrend Router, geteilter Experte und Mischer hochaufgeloest
//! bleiben (gemessen in L13). Dieses Modul schreibt eine Kopie eines
//! int8-Artefakts, deren Experten **ternaere Werte** tragen: je Gruppe von
//! 128 Eingaengen `{-m, 0, +m}`, noch in int8 mit Zeilenshift.
//!
//! # ⚑ Warum zuerst im int8-Format und nicht gleich gepackt
//!
//! **Die Qualitaet laesst sich so mit dem bestehenden Rechenweg messen**,
//! ohne an der Inferenz etwas zu aendern. Das Packen danach ist exakt
//! (kein Runden, siehe `ternaer_packen`), also gilt jede Messung hier auch
//! fuer das gepackte Artefakt. Zwei Schritte mit je einer Aufgabe: hier wird
//! entschieden, welcher Wert ein Gewicht bekommt, dort nur, wie er
//! gespeichert wird.
//!
//! # ⚑ Gerundet wird mit der Ableitung des Trainings
//!
//! `master_aus_gewicht` und dann `ternaer_aus_master`, dieselben zwei
//! Schritte, mit denen das ternaere Training aus einem int8-Artefakt sein
//! Gewicht gewinnt. Ein so umgewandeltes Artefakt ist deshalb genau der
//! Ausgangspunkt, den das Training auch saehe; eine zweite Rundung daneben
//! waere eine zweite Lesart derselben Umwandlung.
//!
//! Kein Teil des Rechenpfads: Hier entsteht ein Artefakt, und erst das
//! Artefakt ist es, was alle Knoten gleich rechnen.

use crate::loader::sha256_hex;
use crate::model::{Gewichtsdaten, QTensor};
use integer_llm_kernels::optimierer::MASTER_FRAC;
use serde_json::{json, Value};
use std::path::Path;

/// Welche Matrizen eines Experten umgewandelt werden.
pub const EXPERTENMATRIZEN: [&str; 3] = ["gate_proj_weight", "up_proj_weight", "down_proj_weight"];

/// Ebene und Expertennummer eines gerouteten Expertentensors, sonst `None`.
///
/// ⚑ **Der geteilte Experte ist keiner davon** (`mlp_shared_expert_…`):
/// Er feuert bei jedem Token und bleibt hochaufgeloest.
pub fn expertentensor(name: &str) -> Option<(usize, usize)> {
    let rest = name.strip_prefix("model_layers_")?;
    let (ebene, rest) = rest.split_once('_')?;
    let rest = rest.strip_prefix("mlp_experts_")?;
    let (nummer, matrix) = rest.split_once('_')?;
    if !EXPERTENMATRIZEN.contains(&matrix) {
        return None;
    }
    Some((ebene.parse().ok()?, nummer.parse().ok()?))
}

/// **Eine int8-Matrix ternaer gerundet**, wie das Training sie ableitet.
///
/// `werte` Zeile fuer Zeile, `shifts` je Zeile. Zurueck kommen int8-Werte,
/// die je Gruppe von 128 ternaer sind, und neue Zeilenshifts.
pub fn ternaer_runden(werte: Vec<i8>, shifts: Vec<u8>, spalten: usize) -> (Vec<i8>, Vec<u8>) {
    let zeilen = shifts.len();
    let t = QTensor {
        data: std::sync::Arc::new(Gewichtsdaten::Speicher(werte)),
        shape: vec![zeilen, spalten],
        shifts,
        drehung: None,
    };
    let master = crate::trainingsschleife::master_aus_gewicht(&t);
    integer_llm_kernels::trainingsschritt::ternaer_aus_master(&master, spalten, MASTER_FRAC)
}

/// Was die Umwandlung getan hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Umwandlungsbericht {
    /// Umgewandelte Matrizen.
    pub matrizen: usize,
    /// Die Ebenen, deren Experten umgewandelt wurden, aufsteigend.
    pub ebenen: Vec<usize>,
    /// Gewichte, die nach dem Runden null sind, und alle umgewandelten.
    pub nullen: u64,
    pub gewichte: u64,
}

/// **Schreibt `ziel`**: die Quelle, mit den Experten der Ebenen, fuer die
/// `ebene_waehlen` wahr ist, ternaer gerundet.
///
/// Mit `ersatz` werden die Experten nicht hier gerundet, sondern aus diesem
/// Verzeichnis genommen (gleiche Dateinamen wie im Manifest), etwa aus einer
/// datenabhaengigen Rundung. ⚑ **Geprueft wird dann, dass jede Gruppe
/// ternaer ist**: Eine Datei, die es nicht ist, liesse sich nicht packen,
/// und das soll hier auffallen und nicht erst beim Packen.
///
/// Unveraenderte Dateien werden hart verlinkt (sonst kopiert); neu
/// geschrieben werden die umgewandelten Gewichte und Shifts, das Manifest
/// und `theta_v.json`, dessen `weights_hash` die Pruefsumme des Manifests ist.
pub fn artefakt_umwandeln(
    quelle: &Path,
    ziel: &Path,
    ebene_waehlen: &dyn Fn(usize) -> bool,
    ersatz: Option<&Path>,
    melden: &dyn Fn(&str),
) -> Result<Umwandlungsbericht, String> {
    if ziel.exists() {
        return Err(format!("{} gibt es schon; hier wird nichts ueberschrieben", ziel.display()));
    }
    let lesen = |name: &str| -> Result<Value, String> {
        let text = std::fs::read_to_string(quelle.join(name)).map_err(|e| format!("{name}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))
    };
    let mut manifest = lesen("weights_manifest.json")?;
    let mut theta_v = lesen("theta_v.json")?;
    let eintraege = manifest.as_object_mut().ok_or("weights_manifest.json ist kein Objekt")?;

    // Erst pruefen, dann schreiben: Ein halb geschriebenes Ziel saehe aus
    // wie ein Artefakt.
    let mut namen: Vec<String> = eintraege
        .keys()
        .filter(|n| expertentensor(n).is_some_and(|(e, _)| ebene_waehlen(e)))
        .cloned()
        .collect();
    namen.sort();
    if namen.is_empty() {
        return Err("keine gerouteten Experten in den gewaehlten Ebenen".into());
    }
    for n in &namen {
        let e = &eintraege[n];
        if e["dtype"].as_str() != Some("int8") || e["shifts_file"].as_str().is_none() {
            return Err(format!("{n}: erwartet int8 mit shifts_file, gefunden {}", e["dtype"]));
        }
    }

    // ⚑ **Geschrieben wird in `<ziel>.unfertig`, umbenannt erst am Ende.**
    //   📌 Ein Ersatz, der nicht ternaer ist, fiel am 2026-10-05 nach 1 402
    //   geschriebenen Dateien auf, und das halbe Ziel sah aus wie ein
    //   Artefakt. Jetzt traegt ein abgebrochener Lauf seinen Zustand im Namen.
    let arbeit = ziel.with_extension("unfertig");
    if arbeit.exists() {
        return Err(format!("{} gibt es schon, ein abgebrochener Lauf; erst entfernen", arbeit.display()));
    }
    std::fs::create_dir_all(&arbeit).map_err(|f| format!("{}: {f}", arbeit.display()))?;
    let schreiben = |datei: &str, bytes: &[u8]| -> Result<(), String> {
        std::fs::write(arbeit.join(datei), bytes).map_err(|f| format!("{datei}: {f}"))
    };

    let mut bericht = Umwandlungsbericht::default();
    let mut ersetzt: Vec<String> = Vec::new();
    for (i, n) in namen.iter().enumerate() {
        let e = eintraege.get_mut(n).expect("eben gelesen");
        let datei = e["file"].as_str().ok_or_else(|| format!("{n}: ohne file"))?.to_string();
        let sdatei = e["shifts_file"].as_str().expect("eben geprueft").to_string();
        let form: Vec<usize> = e["shape"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|v| v as usize)).collect())
            .unwrap_or_default();
        let [zeilen, spalten] = form[..] else {
            return Err(format!("{n}: keine Matrix, shape {form:?}"));
        };
        let werte: Vec<i8> = std::fs::read(quelle.join(&datei))
            .map_err(|f| format!("{datei}: {f}"))?
            .into_iter()
            .map(|b| b as i8)
            .collect();
        let shifts = std::fs::read(quelle.join(&sdatei)).map_err(|f| format!("{sdatei}: {f}"))?;
        if werte.len() != zeilen * spalten || shifts.len() != zeilen {
            return Err(format!("{n}: {} Werte und {} Shifts passen nicht zu {zeilen} x {spalten}", werte.len(), shifts.len()));
        }
        let (neu, neue_shifts) = match ersatz {
            None => ternaer_runden(werte, shifts, spalten),
            Some(dir) => {
                let neu: Vec<i8> = std::fs::read(dir.join(&datei))
                    .map_err(|f| format!("{}: {f}", dir.join(&datei).display()))?
                    .into_iter()
                    .map(|b| b as i8)
                    .collect();
                let neue_shifts = std::fs::read(dir.join(&sdatei)).map_err(|f| format!("{}: {f}", dir.join(&sdatei).display()))?;
                if neu.len() != zeilen * spalten || neue_shifts.len() != zeilen {
                    return Err(format!("{n}: der Ersatz hat {} Werte und {} Shifts", neu.len(), neue_shifts.len()));
                }
                integer_llm_kernels::ternaer::packen(&neu, spalten).map_err(|f| format!("{n}: Ersatz nicht ternaer: {f}"))?;
                (neu, neue_shifts)
            }
        };
        bericht.nullen += neu.iter().filter(|&&w| w == 0).count() as u64;
        bericht.gewichte += neu.len() as u64;
        let bytes: Vec<u8> = neu.iter().map(|&w| w as u8).collect();
        schreiben(&datei, &bytes)?;
        schreiben(&sdatei, &neue_shifts)?;
        e["hash"] = json!(sha256_hex(&bytes));
        e["shifts_hash"] = json!(sha256_hex(&neue_shifts));
        ersetzt.push(datei);
        ersetzt.push(sdatei);
        bericht.matrizen += 1;
        let ebene = expertentensor(n).expect("eben gefiltert").0;
        if !bericht.ebenen.contains(&ebene) {
            bericht.ebenen.push(ebene);
        }
        if (i + 1) % 3072 == 0 || i + 1 == namen.len() {
            melden(&format!("{} von {} Matrizen", i + 1, namen.len()));
        }
    }

    // Die Namen sind als Text sortiert (Ebene 10 vor Ebene 2); der Bericht
    // nennt die Ebenen der Zahl nach.
    bericht.ebenen.sort_unstable();
    for d in std::fs::read_dir(quelle).map_err(|f| format!("{}: {f}", quelle.display()))? {
        let d = d.map_err(|f| f.to_string())?;
        let n = d.file_name().to_string_lossy().to_string();
        if ersetzt.contains(&n) || n == "weights_manifest.json" || n == "theta_v.json" || !d.path().is_file() {
            continue;
        }
        if std::fs::hard_link(d.path(), arbeit.join(&n)).is_err() {
            std::fs::copy(d.path(), arbeit.join(&n)).map_err(|f| format!("{n}: {f}"))?;
        }
    }
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|f| f.to_string())?;
    schreiben("weights_manifest.json", &manifest_bytes)?;
    theta_v["weights_hash"] = json!(sha256_hex(&manifest_bytes));
    schreiben("theta_v.json", &serde_json::to_vec(&theta_v).map_err(|f| f.to_string())?)?;
    std::fs::rename(&arbeit, ziel).map_err(|f| format!("{} nach {}: {f}", arbeit.display(), ziel.display()))?;
    Ok(bericht)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_namen_der_gerouteten_experten() {
        assert_eq!(expertentensor("model_layers_3_mlp_experts_17_gate_proj_weight"), Some((3, 17)));
        assert_eq!(expertentensor("model_layers_39_mlp_experts_255_down_proj_weight"), Some((39, 255)));
        assert_eq!(expertentensor("model_layers_3_mlp_shared_expert_gate_proj_weight"), None);
        assert_eq!(expertentensor("model_layers_3_mlp_gate_weight"), None, "der Router ist kein Experte");
        assert_eq!(expertentensor("model_layers_3_mlp_experts_17_gate_proj_weight_shifts"), None);
        assert_eq!(expertentensor("model_layers_3_self_attn_q_proj_weight"), None);
    }

    /// ⚑ **Jede Gruppe ist danach ternaer**, und die Rundung ist die des
    /// Trainings: dieselbe Matrix ueber `master_aus_gewicht` und
    /// `ternaer_aus_master` von Hand ergibt dieselben Bytes.
    #[test]
    fn jede_gruppe_ist_ternaer_und_die_rundung_ist_die_des_trainings() {
        let (zeilen, spalten) = (3usize, 256usize);
        let mut x = 0x2545_F491_u64;
        let werte: Vec<i8> = (0..zeilen * spalten)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                ((x % 255) as i32 - 127) as i8
            })
            .collect();
        let shifts = vec![7u8, 9, 5];
        let (neu, neue_shifts) = ternaer_runden(werte.clone(), shifts.clone(), spalten);
        for gruppe in neu.chunks_exact(128) {
            let betraege: std::collections::BTreeSet<i32> =
                gruppe.iter().filter(|&&w| w != 0).map(|&w| i32::from(w).abs()).collect();
            assert!(betraege.len() <= 1, "eine Gruppe traegt mehrere Betraege: {betraege:?}");
        }
        let t = QTensor {
            data: std::sync::Arc::new(Gewichtsdaten::Speicher(werte)),
            shape: vec![zeilen, spalten],
            shifts,
            drehung: None,
        };
        let soll = integer_llm_kernels::trainingsschritt::ternaer_aus_master(
            &crate::trainingsschleife::master_aus_gewicht(&t),
            spalten,
            MASTER_FRAC,
        );
        assert_eq!((neu, neue_shifts), soll);
    }
}
