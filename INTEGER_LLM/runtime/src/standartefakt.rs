//! Aus einem trainierten Gewichtsstand wird ein Artefakt.
//!
//! # ⚑ Wozu
//!
//! Ein Trainingslauf endet mit **Mastern**: `i32` mit
//! [`MASTER_FRAC`] Nachkommabits, das Gedaechtnis der kleinen Schritte.
//! Gerechnet, ausgeliefert und geprueft wird aber mit der
//! Uebertragungsform: int8 mit einem Shift je Zeile, oder ternaer gepackt.
//! Solange dieser Weg fehlt, laesst sich ein trainiertes Modell messen,
//! aber nicht laden, und jede Aussage ueber seine Guete haengt am
//! Trainingswerkzeug.
//!
//! # ⚑ Dieselbe Umrechnung wie im Training
//!
//! Die Gewichte entstehen mit [`gewicht_aus_master_als`], also mit genau
//! der Funktion, durch die der Vorwaertspass des Trainings jede Matrix
//! schickt. Das Artefakt rechnet deshalb, was der Lauf zuletzt gemessen
//! hat, und nicht eine zweite Rundung desselben Standes.
//!
//! # Was es schreibt
//!
//! Je Matrix des Bereichs die Gewichte und die Zeilenshifts; bei der
//! ternaeren Form statt der int8-Gewichte das Muster (zwei Bit je
//! Gewicht) und die Betraege je Gruppe. Dazu `weights_manifest.json` und
//! `theta_v.json` neu, denn der `weights_hash` ist die Pruefsumme des
//! Manifests. **Alles andere wird hart verlinkt**, nicht kopiert, und wo
//! das Dateisystem keinen harten Link erlaubt, kopiert.
//!
//! ⛔️ **Das Ziel darf es nicht geben.** Ein hart verlinktes Ziel teilt
//! seine Bytes mit der Quelle; wer in ein bestehendes Verzeichnis
//! schriebe, koennte durch einen Link hindurch die Quelle aendern.
//!
//! # Was es nicht kann
//!
//! * **Expertengemische.** Ihr Stand haelt nur die Experten, die der
//!   Router gewaehlt hat; der Weg dafuer ist ein eigener.
//! * **Den Ablesekopf.** Er steht nicht im Gewichtsstand.

use crate::loader::{sha256_hex, TERNAER_DTYPE};
use crate::model::{Feedforward, IntegerModel};
use crate::shardtraining::{Ebenenstand, Shardgewichte};
use crate::trainingsschleife::breiten_der_ebene;
use integer_llm_kernels::optimierer::MASTER_FRAC;
use integer_llm_kernels::trainingsschritt::{gewicht_aus_master_als, Gewichtsform};
use serde_json::{json, Value};
use std::path::Path;

/// Die sieben Matrizen einer dichten Ebene, in der Reihenfolge des
/// Standes: Q, K, V, O, Gate, Up, Down.
pub const MATRIXNAMEN: [&str; 7] = [
    "self_attn_q_proj_weight",
    "self_attn_k_proj_weight",
    "self_attn_v_proj_weight",
    "self_attn_o_proj_weight",
    "mlp_gate_proj_weight",
    "mlp_up_proj_weight",
    "mlp_down_proj_weight",
];

/// Was geschrieben wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Artefaktbericht {
    /// Wie viele Matrizen neu geschrieben sind.
    pub matrizen: usize,
    /// Wie viele Dateien aus der Quelle uebernommen sind.
    pub uebernommen: usize,
    /// Die Bytes der neuen Gewichte (ohne Shifts).
    pub gewichtsbytes: u64,
    /// Die Form, in der geschrieben wurde.
    pub form: Gewichtsform,
}

/// Der Ebenenbereich, zu dem eine Standdatei gehoert: `(von, bis)`.
///
/// Damit kann ein Werkzeug die passenden [`Shardgewichte`] anlegen, bevor
/// es den Stand einliest.
pub fn bereich_des_standes(pfad: &Path) -> Result<(usize, usize), String> {
    use std::io::Read;
    let mut kopf = [0u8; 17];
    std::fs::File::open(pfad)
        .and_then(|mut f| f.read_exact(&mut kopf))
        .map_err(|e| format!("{}: {e}", pfad.display()))?;
    if &kopf[..9] != b"MYLSTAND1" {
        return Err("das ist kein Gewichtsstand (Kennung fehlt)".into());
    }
    let zahl = |ab: usize| u32::from_ne_bytes([kopf[ab], kopf[ab + 1], kopf[ab + 2], kopf[ab + 3]]) as usize;
    Ok((zahl(9), zahl(13)))
}

/// Schreibt das Artefakt `ziel`: die Quelle, mit den Matrizen des
/// Bereichs aus dem Stand `g`.
///
/// `m` ist das aus `quelle` geladene Modell; es liefert die Zeilenbreiten.
pub fn stand_ins_artefakt(
    quelle: &Path,
    m: &IntegerModel,
    g: &Shardgewichte,
    ziel: &Path,
) -> Result<Artefaktbericht, String> {
    if ziel.exists() {
        return Err(format!("{} gibt es schon; hier wird nichts ueberschrieben", ziel.display()));
    }
    let lesen = |name: &str| -> Result<Value, String> {
        let text = std::fs::read_to_string(quelle.join(name)).map_err(|e| format!("{name}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))
    };
    let mut manifest = lesen("weights_manifest.json")?;
    let mut theta_v = lesen("theta_v.json")?;
    let form = g.form();

    // Erst pruefen, dann schreiben: Ein halb geschriebenes Ziel saehe aus
    // wie ein Artefakt.
    for (i, stand) in g.master.iter().enumerate() {
        let e = g.von() + i;
        if !matches!(stand, Ebenenstand::Dicht(_)) || !matches!(m.layers[e].ffn, Feedforward::Dense(_)) {
            return Err(format!("Ebene {e} ist ein Expertengemisch; das traegt dieser Weg nicht"));
        }
        for teil in MATRIXNAMEN {
            let name = format!("model_layers_{e}_{teil}");
            if manifest.get(&name).is_none() {
                return Err(format!("{name} steht nicht im Manifest der Quelle"));
            }
        }
    }

    std::fs::create_dir_all(ziel).map_err(|e| format!("{}: {e}", ziel.display()))?;
    let schreiben = |datei: &str, bytes: &[u8]| -> Result<(), String> {
        std::fs::write(ziel.join(datei), bytes).map_err(|e| format!("{datei}: {e}"))
    };
    // Dateien der Quelle, die das Ziel nicht uebernimmt.
    let mut ersetzt: Vec<String> = vec!["weights_manifest.json".into(), "theta_v.json".into()];
    let mut bericht = Artefaktbericht { matrizen: 0, uebernommen: 0, gewichtsbytes: 0, form };

    for (i, stand) in g.master.iter().enumerate() {
        let e = g.von() + i;
        let (Ebenenstand::Dicht(master), Feedforward::Dense(mlp)) = (stand, &m.layers[e].ffn) else {
            unreachable!("oben geprueft");
        };
        let breiten = breiten_der_ebene(m, mlp.gate_proj.shape[0]);
        for (n, teil) in MATRIXNAMEN.iter().enumerate() {
            let name = format!("model_layers_{e}_{teil}");
            let (gewichte, shifts) = gewicht_aus_master_als(&master[n], breiten[n], MASTER_FRAC, form);
            let eintrag = manifest
                .get_mut(&name)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("{name}: kein Objekt im Manifest"))?;
            let zeilen = master[n].len() / breiten[n];
            if eintrag.get("shape") != Some(&json!([zeilen, breiten[n]])) {
                return Err(format!(
                    "{name}: der Stand hat {zeilen} x {}, das Manifest {}",
                    breiten[n],
                    eintrag.get("shape").cloned().unwrap_or(Value::Null)
                ));
            }
            for feld in ["file", "shifts_file", "betraege_file"] {
                if let Some(alt) = eintrag.get(feld).and_then(Value::as_str) {
                    ersetzt.push(alt.to_string());
                }
            }
            eintrag.remove("betraege_file");
            eintrag.remove("betraege_hash");

            let sdatei = format!("{name}_shifts.bin");
            schreiben(&sdatei, &shifts)?;
            eintrag.insert("shifts_file".into(), json!(sdatei));
            eintrag.insert("shifts_hash".into(), json!(sha256_hex(&shifts)));

            match form {
                Gewichtsform::Int8 => {
                    let bytes: Vec<u8> = gewichte.iter().map(|&w| w as u8).collect();
                    let datei = format!("{name}.bin");
                    schreiben(&datei, &bytes)?;
                    bericht.gewichtsbytes += bytes.len() as u64;
                    eintrag.insert("dtype".into(), json!("int8"));
                    eintrag.insert("file".into(), json!(datei));
                    eintrag.insert("hash".into(), json!(sha256_hex(&bytes)));
                }
                Gewichtsform::Ternaer => {
                    // ⚑ Eine ternaere Ableitung traegt je Gruppe nur -a, 0
                    // und +a; packen ist deshalb exakt, und wenn es doch
                    // scheitert, ist das ein Fehler der Ableitung und kein
                    // Anlass, ungepackt zu schreiben.
                    let gepackt = integer_llm_kernels::ternaer::packen(&gewichte, breiten[n])
                        .map_err(|f| format!("{name}: {f}"))?;
                    let betraege: Vec<u8> = gepackt.betraege.iter().flat_map(|b| b.to_le_bytes()).collect();
                    let (mdatei, bdatei) = (format!("{name}.muster.bin"), format!("{name}.betraege.bin"));
                    schreiben(&mdatei, &gepackt.muster)?;
                    schreiben(&bdatei, &betraege)?;
                    bericht.gewichtsbytes += (gepackt.muster.len() + betraege.len()) as u64;
                    eintrag.insert("dtype".into(), json!(TERNAER_DTYPE));
                    eintrag.insert("file".into(), json!(mdatei));
                    eintrag.insert("hash".into(), json!(sha256_hex(&gepackt.muster)));
                    eintrag.insert("betraege_file".into(), json!(bdatei));
                    eintrag.insert("betraege_hash".into(), json!(sha256_hex(&betraege)));
                }
            }
            bericht.matrizen += 1;
        }
    }

    for d in std::fs::read_dir(quelle).map_err(|e| format!("{}: {e}", quelle.display()))? {
        let d = d.map_err(|e| e.to_string())?;
        let n = d.file_name().to_string_lossy().to_string();
        // Was das Ziel schon hat, ist neu geschrieben und bleibt es.
        if ersetzt.contains(&n) || !d.path().is_file() || ziel.join(&n).exists() {
            continue;
        }
        if std::fs::hard_link(d.path(), ziel.join(&n)).is_err() {
            std::fs::copy(d.path(), ziel.join(&n)).map_err(|e| format!("{n}: {e}"))?;
        }
        bericht.uebernommen += 1;
    }

    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    schreiben("weights_manifest.json", &manifest_bytes)?;
    theta_v["weights_hash"] = json!(sha256_hex(&manifest_bytes));
    schreiben("theta_v.json", &serde_json::to_vec(&theta_v).map_err(|e| e.to_string())?)?;
    Ok(bericht)
}
