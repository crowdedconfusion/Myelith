//! Konformitätsprüfung der Golden Vectors gegen die Kernel.
//!
//! Die Vektoren unter `INTEGER_LLM/conformance/vectors/op/` legen für
//! jede Operation Eingaben und erwartete Ausgaben bitgenau fest. Ein
//! Backend gilt als konform, wenn es alle Vektoren bitgleich
//! reproduziert.
//!
//! **Warum die Prüfung hier liegt und nicht im Binary.** Bis v0.22.0
//! steckte sie vollständig in `src/bin/golden_runner.rs`. Damit konnte
//! nur aufrufen, wer das Binary baut und findet — der Testclient hätte
//! für einen Konformitätslauf ein zweites Programm starten müssen, und
//! sein Ergebnis wäre eine Terminalausgabe geblieben statt einer
//! Protokollzeile. Die Logik gehört zur Kernel-Bibliothek; das Binary
//! ist ein dünner Starter darüber.
//!
//! **Was dieses Modul ausdrücklich nicht tut:** das Backend ablehnen,
//! das keinen eigenen Rechenpfad hat. Das ist Aufgabe des Aufrufers
//! (`rechenpfad::rechnet`), denn die Ablehnung gehört zum Lauf, nicht
//! zum einzelnen Vektor: Wer sechs Vektoren prüft, will eine Ablehnung
//! einmal hören, nicht sechsmal.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// Ein Golden Vector, so wie `tests/golden/generate.py` ihn schreibt.
#[derive(Debug, Deserialize)]
pub struct GoldenVector {
    pub name: String,
    /// Ebene des Vektors (`op`, `layer`, `e2e`). Teil des Formats, damit
    /// ein unvollständiger Vektor beim Parsen auffällt statt still
    /// durchzugehen.
    pub level: String,
    /// θ_v-Hash des erzeugenden Modells. Die Gegenprüfung braucht die
    /// eingebettete `spec.json` der Runtime; die kernels kennen sie
    /// nicht. Das Feld wird mitgeführt, damit ein Vektor ohne es
    /// auffällt.
    pub theta_v_hash: String,
    /// Woher die Sollwerte stammen.
    ///
    /// ⚑ **Die Unterscheidung steht im Vektor und nicht im Kommentar.**
    /// `unabhaengig` heisst: Eine getrennte Umsetzung hat sie gerechnet,
    /// und eine Uebereinstimmung belegt **Richtigkeit**.
    /// `selbsterzeugt` heisst: Dieser Code hat sie selbst erzeugt, und
    /// eine Uebereinstimmung belegt **Determinismus zwischen Maschinen**,
    /// sonst nichts. Wer die beiden verwechselt, hält eine
    /// Selbstzertifizierung für einen Beleg (Fund 105 in anderer
    /// Gestalt).
    ///
    /// Fehlt das Feld, gilt `unabhaengig`: Alle Vektoren bis zum
    /// 2026-09-04 sind es, und ein stillschweigend anderer Vorgabewert
    /// wäre die gefährlichere Richtung.
    #[serde(default = "herkunft_vorgabe")]
    pub herkunft: String,
    pub metadata: serde_json::Value,
    pub inputs: HashMap<String, TensorData>,
    pub outputs: HashMap<String, TensorData>,
}

fn herkunft_vorgabe() -> String {
    "unabhaengig".to_string()
}

/// Ein Tensor im Golden Vector.
#[derive(Debug, Deserialize)]
pub struct TensorData {
    pub dtype: String,
    /// Tensorform — Teil des Formats; die Kernel arbeiten auf flachen
    /// Vektoren und leiten die Form aus den Metadaten ab.
    pub shape: Option<Vec<usize>>,
    pub hash: String,
    pub data: Vec<i64>,
}

/// SHA-256 über die Little-Endian-gepackte Nutzlast — identisch zu
/// `GoldenVectorBuilder._hash_tensor` in `tests/golden/generate.py`.
///
/// Liefert eine leere Zeichenkette, wenn der dtype nicht gepackt
/// darstellbar ist: Der Hash ist dann nicht prüfbar, und das wird am
/// Rückgabewert sichtbar statt an einer Vermutung.
pub fn hash_tensor(data: &[i64], dtype: &str) -> String {
    let mut payload: Vec<u8> = Vec::with_capacity(data.len() * 4);
    match dtype {
        "int8" => data.iter().for_each(|&v| payload.push(v as i8 as u8)),
        "int16" => data
            .iter()
            .for_each(|&v| payload.extend_from_slice(&(v as i16).to_le_bytes())),
        "int32" => data
            .iter()
            .for_each(|&v| payload.extend_from_slice(&(v as i32).to_le_bytes())),
        _ => return String::new(),
    }
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Prüft die deklarierten Hashes aller Ein- und Ausgabetensoren.
///
/// **Vor der Rechnung, nicht danach:** Ein Vektor, dessen Daten
/// nachträglich bearbeitet wurden, ohne den Hash mitzuziehen, ist kein
/// Maßstab; ein Ergebnis gegen ihn hat keine Aussage, weder im Guten
/// noch im Schlechten. Die Felder `hash` und `theta_v_hash` wurden bis
/// v0.12.40 eingelesen, aber nie ausgewertet.
pub fn tensor_hashes_pruefen(gv: &GoldenVector, gruende: &mut Vec<String>) -> bool {
    let mut ok = true;
    for (bereich, tensors) in [("input", &gv.inputs), ("output", &gv.outputs)] {
        for (name, t) in tensors.iter() {
            let computed = hash_tensor(&t.data, &t.dtype);
            if computed.is_empty() {
                continue; // dtype nicht gepackt darstellbar
            }
            if computed != t.hash {
                gruende.push(format!(
                    "Hash-Abweichung bei {} '{}': deklariert {}, berechnet {}",
                    bereich, name, t.hash, computed
                ));
                ok = false;
            }
        }
    }
    ok
}

/// Das Ergebnis einer Vektorprüfung.
#[derive(Debug, Clone)]
pub struct VektorErgebnis {
    pub name: String,
    pub bestanden: bool,
    /// Menschenlesbare Ursachen, bei bestandener Prüfung leer.
    pub gruende: Vec<String>,
    /// Die Integrität des Vektors selbst war verletzt (Tensor-Hashes).
    /// Dann ist er kein Maßstab, und das Ergebnis sagt nichts über das
    /// Backend — der Fall muss anders berichtet werden als ein
    /// Rechnen, das abwich.
    pub integer_verletzt: bool,
}

/// Liest einen Golden Vector aus einer Datei.
pub fn vektor_lesen(pfad: &Path) -> Result<GoldenVector, String> {
    let inhalt = std::fs::read_to_string(pfad)
        .map_err(|e| format!("{} nicht lesbar: {}", pfad.display(), e))?;
    serde_json::from_str(&inhalt)
        .map_err(|e| format!("Golden-JSON ungültig ({}): {}", pfad.display(), e))
}

/// Prüft einen Operations-Vektor gegen die Kernel dieses Baus.
///
/// Die Backend-Ablehnung macht der Aufrufer, siehe Modul-Doku.
pub fn op_vektor_pruefen(gv: &GoldenVector) -> VektorErgebnis {
    let name = gv.name.clone();
    let mut gruende: Vec<String> = Vec::new();

    if !tensor_hashes_pruefen(gv, &mut gruende) {
        return VektorErgebnis {
            name,
            bestanden: false,
            gruende,
            integer_verletzt: true,
        };
    }

    let (bestanden, weitere) = match gv.name.as_str() {
        "rmsnorm_basic" => run_rmsnorm(gv),
        "linear_w8a16_identity" => run_linear(gv),
        "softmax_basic" => run_softmax(gv),
        // Rückwärtspass (kernels v0.19.0). Eine fremde Umsetzung muss
        // auch ihn bitgleich reproduzieren, sonst trägt sie kein
        // verifizierbares Training.
        "backward_linear" => run_backward_linear(gv),
        "backward_softmax" => run_backward_softmax(gv),
        "backward_rope" => run_backward_rope(gv),
        _ => (false, vec![format!("Unknown golden vector: {}", gv.name)]),
    };
    gruende.extend(weitere);
    VektorErgebnis {
        name,
        bestanden,
        gruende,
        integer_verletzt: false,
    }
}

/// Prüft einen Vektor des Trainingspfades (Umfang `training`).
///
/// # ⚑ Warum ein eigener Umfang
///
/// Der `op`-Umfang trägt den Wert `894d8357ae92b5c1`, und der steht an
/// sechs Stellen des Repositoriums fest. Neue Vektoren dort hätten ihn
/// geändert, also eine bestehende Zusage gebrochen, um eine neue
/// aufzustellen. **Zwei Umfänge, zwei Zahlen, beide im Protokoll**, und
/// eine Abweichung ist damit sofort eingegrenzt.
///
/// ⚑ **Diese fünf Vektoren decken die Lücke, aus der die Funde 173 bis
/// 175 kamen.** Drei der acht Rückwärtskerne rechneten falsch, und der
/// Prüflauf, der 33 von 33 meldete, hat keinen von ihnen je gerechnet.
pub fn trainingsvektor_pruefen(gv: &GoldenVector) -> VektorErgebnis {
    let name = gv.name.clone();
    let mut gruende: Vec<String> = Vec::new();

    if !tensor_hashes_pruefen(gv, &mut gruende) {
        return VektorErgebnis { name, bestanden: false, gruende, integer_verletzt: true };
    }

    let (bestanden, weitere) = match gv.name.as_str() {
        "backward_attention" => run_backward_attention(gv),
        "backward_silu" => run_backward_silu(gv),
        "backward_rmsnorm" => run_backward_rmsnorm(gv),
        "backward_embedding" => run_backward_embedding(gv),
        "optimierer_schritt" => run_optimierer_schritt(gv),
        "softmax_vokabular" => run_softmax_vokabular(gv),
        _ => (false, vec![format!("Unbekannter Trainingsvektor: {}", gv.name)]),
    };
    gruende.extend(weitere);
    VektorErgebnis { name, bestanden, gruende, integer_verletzt: false }
}

/// Prüft einen Vektor des **MoE-Routingpfades** (Umfang `moe`).
///
/// # ⚑ Warum es diesen Umfang gibt (Fund 180, 2026-09-05)
///
/// `route_top_k` und `mische_experten` waren **nirgends** gegen ein
/// festes Soll geprüft: nicht unter `op`, nicht unter `training`, nicht
/// unter `layer` (die Layer-Vektoren gehören zum dichten 0,5B-Modell).
///
/// **Das ist der Pfad, der entscheidet, welche Experten rechnen.** Zwei
/// Knoten, die verschieden routen, rechnen verschiedene Netze, und der
/// Redundanzvergleich meldete beide als fehlerhaft, ohne dass einer
/// gelogen hätte. Dieselbe Klasse wie die drei Rückwärtskerne ohne
/// Aufrufer: tragend und unbelegt.
///
/// ⚑ **Eigener Umfang, aus demselben Grund wie bei `training`:**
/// `894d8357ae92b5c1` steht an sechs Stellen fest, und eine bestehende
/// Zusage bricht man nicht, um eine neue aufzustellen.
pub fn moe_vektor_pruefen(gv: &GoldenVector) -> VektorErgebnis {
    let name = gv.name.clone();
    let mut gruende: Vec<String> = Vec::new();

    if !tensor_hashes_pruefen(gv, &mut gruende) {
        return VektorErgebnis { name, bestanden: false, gruende, integer_verletzt: true };
    }

    let (bestanden, weitere) = match gv.name.as_str() {
        "routing_normiert" | "routing_unnormiert" | "routing_gleichstand" => run_routing(gv),
        "mischung" => run_mischung(gv),
        _ => (false, vec![format!("Unbekannter MoE-Vektor: {}", gv.name)]),
    };
    gruende.extend(weitere);
    VektorErgebnis { name, bestanden, gruende, integer_verletzt: false }
}

/// Prüft eine Datei als MoE-Vektor.
pub fn moe_vektor_aus_datei(pfad: &Path) -> Result<VektorErgebnis, String> {
    let gv = vektor_lesen(pfad)?;
    Ok(moe_vektor_pruefen(&gv))
}

fn run_routing(gv: &GoldenVector) -> (bool, Vec<String>) {
    let logits = als_i32(&gv.inputs["logits"]);
    let exp_lut = als_i16(&gv.inputs["exp_lut"]);
    let normieren = gv.metadata["normieren"].as_bool().unwrap_or(false);
    let routing = crate::moe::route_top_k(
        &logits,
        zahl(gv, "k") as usize,
        &exp_lut,
        zahl(gv, "lut_shift") as u8,
        zahl(gv, "frac_bits") as u8,
        normieren,
    );
    let mut gruende = Vec::new();
    let experten: Vec<i32> = routing.experten.iter().map(|e| i32::from(*e)).collect();
    let soll_e: Vec<i32> = gv.outputs["experten"].data.iter().map(|v| *v as i32).collect();
    let soll_w: Vec<i32> = gv.outputs["gewichte"].data.iter().map(|v| *v as i32).collect();
    let a = vergleiche("experten", &experten, &soll_e, &mut gruende);
    let b = vergleiche("gewichte", &routing.gewichte, &soll_w, &mut gruende);
    (a && b, gruende)
}

fn run_mischung(gv: &GoldenVector) -> (bool, Vec<String>) {
    let n = zahl(gv, "experten") as usize;
    let ausgaben: Vec<Vec<i16>> =
        (0..n).map(|i| als_i16(&gv.inputs[&format!("a{i}")])).collect();
    let gewichte = als_i32(&gv.inputs["gewichte"]);
    let y = crate::moe::mische_experten(&ausgaben, &gewichte, zahl(gv, "frac_bits") as u8);
    let ist: Vec<i32> = y.iter().map(|v| i32::from(*v)).collect();
    let soll: Vec<i32> = gv.outputs["y"].data.iter().map(|v| *v as i32).collect();
    let mut gruende = Vec::new();
    let ok = vergleiche("y", &ist, &soll, &mut gruende);
    (ok, gruende)
}

/// Prüft eine Datei als Trainingsvektor.
pub fn trainingsvektor_aus_datei(pfad: &Path) -> Result<VektorErgebnis, String> {
    let gv = vektor_lesen(pfad)?;
    Ok(trainingsvektor_pruefen(&gv))
}

/// Prüft eine Datei als Operations-Vektor.
pub fn op_vektor_aus_datei(pfad: &Path) -> Result<VektorErgebnis, String> {
    let gv = vektor_lesen(pfad)?;
    Ok(op_vektor_pruefen(&gv))
}

// ---------------------------------------------------------------------------
// Die einzelnen Operationen
// ---------------------------------------------------------------------------

fn run_rmsnorm(gv: &GoldenVector) -> (bool, Vec<String>) {
    // theta_v 0.7.0: int16-Eingang, LUT-gestuetztes rsqrt mit dynamischem
    // geradem Index-Shift, divisionsfrei; Gamma mit Per-Element-Skalen
    // (abwaertskompatibel: ohne gamma_shifts wird gamma_shift repliziert).
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    // Fund 20 (theta_v 0.11.0): x_shifts optional, fuer Vektoren vor
    // v0.12.44 fehlt das Feld - Vorgabe 0 fuer alle Kanaele ist bitgleich
    // zur alten Skalar-Behandlung (bewiesen in kernels/src/rmsnorm.rs,
    // test_rmsnorm_per_channel_uniform_shifts_matches_legacy).
    let x_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("x_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        vec![0u8; x.len()]
    };
    let gamma: Vec<i8> = gv.inputs["gamma"].data.iter().map(|&v| v as i8).collect();
    let gamma_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("gamma_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        let gamma_shift = gv.metadata["gamma_shift"].as_u64().unwrap() as u8;
        vec![gamma_shift; gamma.len()]
    };
    let rsqrt_lut: Vec<i16> = gv.metadata["rsqrt_lut"].as_array().unwrap()
        .iter().map(|v| v.as_i64().unwrap() as i16).collect();
    let lut_input_shift = gv.metadata["lut_input_shift"].as_u64().unwrap() as u8;
    let lut_output_frac = gv.metadata["lut_output_frac"].as_u64().unwrap() as u8;
    let inv_n_q20 = gv.metadata["inv_n_q20"].as_i64().unwrap();
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = crate::rmsnorm::rmsnorm_i16(
        &x, &x_shifts, &gamma, &gamma_shifts, &rsqrt_lut, lut_input_shift, lut_output_frac, inv_n_q20, out_frac);
    let expected: Vec<i16> = gv.outputs["y"].data.iter().map(|&v| v as i16).collect();

    if result != expected {
        return (false, vec![
            format!("Expected: {:?}", expected),
            format!("Got:      {:?}", result),
        ]);
    }
    (true, Vec::new())
}

fn run_linear(gv: &GoldenVector) -> (bool, Vec<String>) {
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    let w_meta = gv.metadata["W"].as_array().unwrap();
    // Flach, wie der Kernel sie seit v0.13.4 erwartet. Die Zeilenlänge
    // steht in der ersten Zeile des Vektors.
    let in_features = w_meta[0].as_array().unwrap().len();
    let w: Vec<i8> = w_meta
        .iter()
        .flat_map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i8)
                .collect::<Vec<i8>>()
        })
        .collect();
    let act_frac = gv.metadata["act_frac"].as_u64().unwrap() as u8;
    let w_shifts: Vec<u8> = if let Some(shifts) = gv.metadata.get("w_shifts") {
        shifts.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect()
    } else {
        let weight_frac = gv.metadata["weight_frac"].as_u64().unwrap() as u8;
        // **Eine Skala je Ausgabe-Zeile, nicht je Gewicht.** Vor der
        // Umstellung auf flache Gewichte war `w.len()` die Zeilenzahl;
        // flach ist es die Elementzahl. Der Vektor
        // `linear_w8a16_identity` fiel damit sofort durch, und das war
        // die richtige Reaktion: Der Kernel prüft die Länge.
        vec![weight_frac; w_meta.len()]
    };
    let out_frac = gv.metadata["out_frac"].as_u64().unwrap() as u8;

    let result = crate::linear::linear_w8a16(&x, &w, in_features, &w_shifts, act_frac, out_frac);
    let expected: Vec<i16> = gv.outputs["y"].data.iter().map(|&v| v as i16).collect();

    if result != expected {
        return (false, vec![
            format!("Expected: {:?}", expected),
            format!("Got:      {:?}", result),
        ]);
    }
    (true, Vec::new())
}

fn run_softmax(gv: &GoldenVector) -> (bool, Vec<String>) {
    let logits: Vec<i32> = gv.inputs["logits"].data.iter().map(|&v| v as i32).collect();
    let lut_shift = gv.metadata["lut_shift"].as_u64().unwrap() as u8;
    let frac_bits = gv.metadata["frac_bits"].as_u64().unwrap() as u8;

    // Die LUT kommt aus dem Vektor. Ein Rückfall, der sie hier aus
    // Gleitkommazahlen nachbaute, war bis v0.22.0 im Binary und
    // verstieße gegen die Ganzzahldisziplin; alle Vektoren seit
    // Aufnahme der exp_lut in die Metadaten tragen sie.
    let Some(lut) = gv.metadata.get("exp_lut") else {
        return (false, vec![
            "metadata.exp_lut fehlt: Vektor stammt aus einer Fassung vor der \
             LUT-Aufnahme und ist mit diesem Prüfweg nicht vergleichbar"
                .to_string(),
        ]);
    };
    let exp_lut: Vec<i16> = lut
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i16)
        .collect();

    let result = crate::softmax::softmax_int(&logits, &exp_lut, lut_shift, frac_bits);
    let expected: Vec<i32> = gv.outputs["probs"].data.iter().map(|&v| v as i32).collect();

    if result != expected {
        return (false, vec![
            format!("Expected: {:?}", expected),
            format!("Got:      {:?}", result),
        ]);
    }
    (true, Vec::new())
}

// ---------------------------------------------------------------------------
// Rueckwaertspass
// ---------------------------------------------------------------------------

/// Hilfsfunktion: liest ein i64-Feld als Vec<i32>.
fn als_i32(t: &TensorData) -> Vec<i32> {
    t.data.iter().map(|&v| v as i32).collect()
}

fn run_backward_linear(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let x: Vec<i16> = gv.inputs["x"].data.iter().map(|&v| v as i16).collect();
    let w: Vec<i8> = gv.inputs["W"].data.iter().map(|&v| v as i8).collect();
    let in_features = gv.metadata["in_features"].as_u64().unwrap() as usize;
    let w_shifts: Vec<u8> = gv.metadata["w_shifts"].as_array().unwrap()
        .iter().map(|v| v.as_u64().unwrap() as u8).collect();
    let g_frac = gv.metadata["g_frac"].as_u64().unwrap() as u8;
    let gx_frac = gv.metadata["gx_frac"].as_u64().unwrap() as u8;

    let (gx, gw) = crate::backward::linear_backward(
        &g, &x, &w, in_features, &w_shifts, g_frac, gx_frac);

    let gx_soll: Vec<i32> = als_i32(&gv.outputs["gx"]);
    let gw_soll: Vec<i64> = gv.outputs["gW"].data.clone();
    let mut gruende = Vec::new();
    let mut ok = true;
    if gx != gx_soll {
        gruende.push(format!("gx: erwartet {:?}, erhalten {:?}", gx_soll, gx));
        ok = false;
    }
    if gw != gw_soll {
        gruende.push("gW weicht ab".to_string());
        ok = false;
    }
    (ok, gruende)
}

fn run_backward_softmax(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let p = als_i32(&gv.inputs["p"]);
    let frac = gv.metadata["frac_bits"].as_u64().unwrap() as u8;
    let out = crate::backward::softmax_backward(&g, &p, frac);
    let soll = als_i32(&gv.outputs["gz"]);
    if out != soll {
        return (false, vec![format!("erwartet {:?}, erhalten {:?}", soll, out)]);
    }
    (true, Vec::new())
}

fn run_backward_rope(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let cos: Vec<i16> = gv.inputs["cos"].data.iter().map(|&v| v as i16).collect();
    let sin: Vec<i16> = gv.inputs["sin"].data.iter().map(|&v| v as i16).collect();
    let frac = gv.metadata["frac_bits"].as_u64().unwrap() as u8;
    let out = crate::backward::rope_backward(&g, &cos, &sin, frac);
    let soll = als_i32(&gv.outputs["gx"]);
    if out != soll {
        return (false, vec![format!("erwartet {:?}, erhalten {:?}", soll, out)]);
    }
    (true, Vec::new())
}

// ---------------------------------------------------------------------------
// Der Trainingspfad
// ---------------------------------------------------------------------------

fn als_i16(t: &TensorData) -> Vec<i16> {
    t.data.iter().map(|&v| v as i16).collect()
}

fn als_u8(t: &TensorData) -> Vec<u8> {
    t.data.iter().map(|&v| v as u8).collect()
}

fn zahl(gv: &GoldenVector, feld: &str) -> i64 {
    gv.metadata[feld].as_i64().unwrap_or_else(|| panic!("{feld} fehlt in den Metadaten"))
}

/// Zerlegt einen flachen Tensor in `zeilen` gleich lange Stuecke.
fn in_zeilen(flach: &[i16], zeilen: usize) -> Vec<Vec<i16>> {
    let breite = flach.len() / zeilen;
    (0..zeilen).map(|z| flach[z * breite..(z + 1) * breite].to_vec()).collect()
}

fn vergleiche(name: &str, ist: &[i32], soll: &[i32], gruende: &mut Vec<String>) -> bool {
    if ist == soll {
        return true;
    }
    gruende.push(format!("{name}: erwartet {soll:?}, erhalten {ist:?}"));
    false
}

fn run_backward_attention(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let q = als_i16(&gv.inputs["q"]);
    let kv_len = zahl(gv, "kv_len") as usize;
    let k = in_zeilen(&als_i16(&gv.inputs["k"]), kv_len);
    let v = in_zeilen(&als_i16(&gv.inputs["v"]), kv_len);
    let p = als_i32(&gv.inputs["p"]);
    let maske: Vec<bool> = gv.metadata["maske"]
        .as_array()
        .expect("maske fehlt")
        .iter()
        .map(|b| b.as_bool().unwrap_or(false))
        .collect();
    let skalen = crate::backward::Aufmerksamkeitsskalen {
        score_mult: zahl(gv, "score_mult"),
        score_mult_frac: zahl(gv, "score_mult_frac") as u8,
        q_frac: zahl(gv, "q_frac") as u8,
        k_frac: zahl(gv, "k_frac") as u8,
        v_frac: zahl(gv, "v_frac") as u8,
        prob_frac: zahl(gv, "prob_frac") as u8,
    };

    let (gq, gk, gvv) =
        crate::backward::attention_backward(&g, &q, &k, &v, &p, &maske, skalen);
    let flach = |z: &[Vec<i32>]| -> Vec<i32> { z.iter().flatten().copied().collect() };

    let mut gruende = Vec::new();
    let mut ok = vergleiche("gq", &gq, &als_i32(&gv.outputs["gq"]), &mut gruende);
    ok &= vergleiche("gk", &flach(&gk), &als_i32(&gv.outputs["gk"]), &mut gruende);
    ok &= vergleiche("gv", &flach(&gvv), &als_i32(&gv.outputs["gv"]), &mut gruende);
    (ok, gruende)
}

fn run_backward_silu(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let x = als_i16(&gv.inputs["x"]);
    let lut = als_i16(&gv.inputs["grad_lut"]);
    let aus = crate::backward::silu_backward(
        &g,
        &x,
        &lut,
        zahl(gv, "x_frac") as u8,
        zahl(gv, "lut_in_frac") as u8,
        zahl(gv, "lut_offset") as i16,
        zahl(gv, "lut_out_frac") as u8,
        zahl(gv, "g_frac") as u8,
        zahl(gv, "out_frac") as u8,
    );
    let mut gruende = Vec::new();
    let ok = vergleiche("gx", &aus, &als_i32(&gv.outputs["gx"]), &mut gruende);
    (ok, gruende)
}

fn run_backward_rmsnorm(gv: &GoldenVector) -> (bool, Vec<String>) {
    let g = als_i32(&gv.inputs["g"]);
    let x = als_i16(&gv.inputs["x"]);
    let xs = als_u8(&gv.inputs["x_shifts"]);
    let gamma: Vec<i8> = gv.inputs["gamma"].data.iter().map(|&v| v as i8).collect();
    let gs = als_u8(&gv.inputs["gamma_shifts"]);
    let (gx, ggamma) = crate::backward::rmsnorm_backward(
        &g,
        &x,
        &xs,
        &gamma,
        &gs,
        crate::backward::Normskalen {
            r: zahl(gv, "r") as i32,
            norm_frac: zahl(gv, "norm_frac") as u8,
            ref_shift: zahl(gv, "ref_shift") as u8,
            inv_n_q20: zahl(gv, "inv_n_q20"),
            g_frac: zahl(gv, "g_frac") as u8,
            gx_frac: zahl(gv, "gx_frac") as u8,
        },
    );
    let mut gruende = Vec::new();
    let mut ok = vergleiche("gx", &gx, &als_i32(&gv.outputs["gx"]), &mut gruende);
    let ggamma_soll = &gv.outputs["ggamma"].data;
    if &ggamma != ggamma_soll {
        gruende.push(format!("ggamma: erwartet {ggamma_soll:?}, erhalten {ggamma:?}"));
        ok = false;
    }
    (ok, gruende)
}

fn run_backward_embedding(gv: &GoldenVector) -> (bool, Vec<String>) {
    let hidden = zahl(gv, "hidden") as usize;
    let vocab = zahl(gv, "vocab_size") as usize;
    let token: Vec<usize> = gv.metadata["token_ids"]
        .as_array()
        .expect("token_ids fehlen")
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    let mut ziel = vec![0i64; hidden * vocab];
    for (n, id) in token.iter().enumerate() {
        let g = als_i32(&gv.inputs[&format!("g{n}")]);
        crate::backward::embedding_backward_akkumulieren(&mut ziel, vocab, *id, &g);
    }
    let soll = &gv.outputs["ziel"].data;
    if &ziel != soll {
        return (false, vec![format!("ziel: erwartet {soll:?}, erhalten {ziel:?}")]);
    }
    (true, Vec::new())
}

/// Der Softmax über ein Vokabular (Fund 177).
///
/// ⚑ **Der Vektor ist klein, der Kern ist es nicht.** Er trifft die
/// Maximumsubtraktion, die Verschiebung von der Logit- auf die
/// Tabellenskala, einen Index jenseits der Tabelle und die
/// kaufmännische Rundung der Division: genau die vier Stellen, an denen
/// zwei Umsetzungen auseinanderlaufen können.
fn run_softmax_vokabular(gv: &GoldenVector) -> (bool, Vec<String>) {
    let logits = als_i32(&gv.inputs["logits"]);
    let exp_lut = als_i16(&gv.inputs["exp_lut"]);
    let p = crate::softmax::softmax_ueber_vokabular(
        &logits,
        zahl(gv, "logit_frac") as u8,
        &exp_lut,
        zahl(gv, "exp_input_frac") as u8,
        zahl(gv, "frac_bits") as u8,
    );
    let mut gruende = Vec::new();
    let soll: Vec<i32> = gv.outputs["p"].data.iter().map(|&v| v as i32).collect();
    let ok = vergleiche("p", &p, &soll, &mut gruende);
    (ok, gruende)
}

fn run_optimierer_schritt(gv: &GoldenVector) -> (bool, Vec<String>) {
    let mut master = als_i32(&gv.inputs["master"]);
    let grad = als_i32(&gv.inputs["grad"]);
    let kennung = crate::optimierer::Schrittkennung {
        ebene: zahl(gv, "ebene") as u32,
        schritt: zahl(gv, "schritt") as u64,
        index_versatz: zahl(gv, "index_versatz") as u64,
    };
    let mut gruende = Vec::new();
    let mut ok = true;

    // ⚑ **Erst der Wuerfel, dann der Schritt.** Wer nur das Ergebnis
    // vergleicht, sieht bei einer Abweichung nicht, ob der Zufall oder
    // die Rundung abwich, und das sind zwei sehr verschiedene Fehler.
    let wuerfe_soll = &gv.outputs["wuerfe"].data;
    let wuerfe: Vec<i64> = (0..master.len())
        .map(|i| {
            crate::optimierer::wuerfel(kennung.ebene, kennung.schritt, kennung.index_versatz + i as u64)
                as i64
        })
        .collect();
    if &wuerfe != wuerfe_soll {
        gruende.push("die Wuerfe weichen ab: der Zufall ist nicht derselbe".to_string());
        ok = false;
    }

    crate::optimierer::schritt(
        &mut master,
        &grad,
        kennung,
        zahl(gv, "lr_zaehler"),
        zahl(gv, "lr_nenner"),
    );
    ok &= vergleiche("master_neu", &master, &als_i32(&gv.outputs["master_neu"]), &mut gruende);
    (ok, gruende)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Hash-Funktion ist der Vertrag mit `generate.py`; ein
    /// stillschweigend anderer dtype machte jeden Vektor ungültig.
    #[test]
    fn hash_vertrag_le_und_dtypes() {
        assert_eq!(hash_tensor(&[], "int8"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        // 0x0102 als int16 little-endian: 02 01.
        let h16 = hash_tensor(&[0x0102], "int16");
        let mut hasher = Sha256::new();
        hasher.update([0x02u8, 0x01]);
        let soll: String = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(h16, soll);
        // Unbekannter dtype ist nicht prüfbar, nicht falsch.
        assert_eq!(hash_tensor(&[1, 2], "float16"), "");
    }

    /// Ein Vektor, dessen Tensor-Daten nachträglich geändert wurden,
    /// darf nicht als Maßstab durchgehen — weder mit Bestehen noch mit
    /// einem begründeten Fehlschlag.
    #[test]
    fn manipulierte_daten_fallen_an_der_hash_pruefung_auf() {
        let mut gv: GoldenVector = serde_json::from_str(r#"{
            "name": "rmsnorm_basic",
            "level": "op",
            "theta_v_hash": "sha256:00",
            "metadata": {},
            "inputs": {
                "x": { "dtype": "int16", "shape": [2], "hash": "falscher-hash", "data": [1, 2] }
            },
            "outputs": {}
        }"#).expect("Testvektor parsebar");
        let mut gruende = Vec::new();
        assert!(!tensor_hashes_pruefen(&gv, &mut gruende));
        assert!(gruende[0].contains("Hash-Abweichung"));

        // Mit korrektem Hash gilt die Integrität als gewahrt.
        gv.inputs.get_mut("x").unwrap().hash = hash_tensor(&[1, 2], "int16");
        gruende.clear();
        assert!(tensor_hashes_pruefen(&gv, &mut gruende));
        assert!(gruende.is_empty());
    }

    /// Die echten Vektoren des Repositoriums müssen über die
    /// Bibliotheksfunktion genauso schließen wie über das Binary: alle
    /// sechs bestehen unter dem Referenzpfad, und ihre Hashes sind
    /// integer. (Die Gegenprobe, dass ein abweichendes Backend auffällt,
    /// trägt der Lauf selbst — hier geht es um den Prüfweg.)
    #[test]
    fn op_vektoren_des_repositoriums_bestehen() {
        let wurzel = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../conformance/vectors/op");
        if !wurzel.is_dir() {
            eprintln!("SKIP: {} nicht vorhanden", wurzel.display());
            return;
        }
        let mut dateien: Vec<_> = std::fs::read_dir(&wurzel)
            .expect("lesbar")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".golden.json"))
            .collect();
        dateien.sort();
        assert_eq!(dateien.len(), 6, "erwartet werden die sechs Operations-Vektoren");
        for p in dateien {
            let e = op_vektor_aus_datei(&p).expect("prüfbar");
            assert!(e.bestanden, "{}: {:?}", p.display(), e.gruende);
            assert!(!e.integer_verletzt, "{}: Hash-Prüfung verletzt", p.display());
        }
    }

    /// Ein unbekannter Vektorname ist ein Fehlschlag, kein Absturz.
    /// ⚑ **Die Vektoren des Trainingspfades bestehen, und sie sind
    /// unabhaengig erzeugt.**
    ///
    /// Fuenf Kerne, die bis zum 2026-09-04 kein Vektor deckte:
    /// `attention_backward`, `silu_backward`, `rmsnorm_backward`,
    /// `embedding_backward_akkumulieren` und der Optimierer. **Drei der
    /// acht Rueckwaertskerne rechneten falsch**, und der Prueflauf, der
    /// 33 von 33 meldete, hat keinen von ihnen je gerechnet.
    ///
    /// ⛑ **Der Test prueft auch die Herkunft.** Ein selbsterzeugter
    /// Vektor belegt Determinismus, kein unabhaengiger belegt
    /// Richtigkeit; wer die beiden verwechselt, haelt eine
    /// Selbstzertifizierung fuer einen Beleg.
    #[test]
    fn trainingsvektoren_des_repositoriums_bestehen() {
        let verzeichnis = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../conformance/vectors/training");
        let mut gesehen = 0;
        for eintrag in std::fs::read_dir(&verzeichnis).expect("Trainingsvektoren fehlen") {
            let pfad = eintrag.expect("Verzeichniseintrag").path();
            if pfad.extension().map(|e| e != "json").unwrap_or(true) {
                continue;
            }
            let gv = vektor_lesen(&pfad).expect("Vektor lesbar");
            assert_eq!(gv.level, "training", "{}: falsche Ebene", gv.name);
            assert_eq!(
                gv.herkunft, "unabhaengig",
                "{}: ein selbsterzeugter Vektor belegt keine Richtigkeit",
                gv.name
            );
            let e = trainingsvektor_pruefen(&gv);
            assert!(e.bestanden, "{}: {:?}", e.name, e.gruende);
            gesehen += 1;
        }
        assert_eq!(gesehen, 6, "erwartet werden sechs Trainingsvektoren");
    }

    /// ⚑ **Der MoE-Routingpfad, seit dem 2026-09-05 belegt** (Fund 180).
    #[test]
    fn moe_vektoren_des_repositoriums_bestehen() {
        let verzeichnis = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../conformance/vectors/moe");
        let mut gesehen = 0;
        for eintrag in std::fs::read_dir(&verzeichnis).expect("MoE-Vektoren fehlen") {
            let pfad = eintrag.expect("Verzeichniseintrag").path();
            if pfad.extension().map(|e| e != "json").unwrap_or(true) {
                continue;
            }
            let gv = vektor_lesen(&pfad).expect("Vektor lesbar");
            assert_eq!(gv.level, "moe", "{}: falsche Ebene", gv.name);
            assert_eq!(
                gv.herkunft, "unabhaengig",
                "{}: ein selbsterzeugter Vektor belegt keine Richtigkeit",
                gv.name
            );
            let e = moe_vektor_pruefen(&gv);
            assert!(e.bestanden, "{}: {:?}", e.name, e.gruende);
            gesehen += 1;
        }
        assert_eq!(gesehen, 4, "erwartet werden vier MoE-Vektoren");
    }

    #[test]
    fn unbekannter_vektor_wird_abgelehnt() {
        let gv: GoldenVector = serde_json::from_str(r#"{
            "name": "gibt_es_nicht",
            "level": "op",
            "theta_v_hash": "sha256:00",
            "metadata": {},
            "inputs": {},
            "outputs": {}
        }"#).unwrap();
        let e = op_vektor_pruefen(&gv);
        assert!(!e.bestanden);
        assert!(e.gruende.iter().any(|g| g.contains("Unknown golden vector")));
    }
}
