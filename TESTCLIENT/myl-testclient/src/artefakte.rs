//! Artefakte finden, bauen und **gegen den veröffentlichten Digest prüfen**.
//!
//! ## Warum die Prüfung der eigentliche Punkt ist
//!
//! Der Zweck dieses Clients ist der Nachweis, dass zwei verschiedene
//! Maschinen bitgleich rechnen. Das setzt voraus, dass beide Maschinen
//! **dasselbe Modell** rechnen — und genau das ist nicht selbstverständlich.
//!
//! Der Artefaktbau war bis 2026-08-20 nur auf derselben Maschine
//! reproduzierbar (Fund 32): Die Aktivierungsskalen entstanden aus einem
//! Gleitkomma-Durchlauf, und **3 von 314** Skaleneinträgen saßen innerhalb
//! von 0,01 % einer Zweierpotenz-Grenze — der knappste bei 0,003 %. Eine
//! andere BLAS-Version reicht, um einen davon umzuwerfen; ein gekippter
//! Shift ändert die Artefaktbytes, also das Modell.
//!
//! Ohne Digest-Prüfung sähe das im Ergebnis **wie eine gescheiterte
//! Hardware-Bitgleichheit aus** — der Testclient würde also genau das
//! Gegenteil dessen berichten, wofür es ihn gibt. Deshalb prüft er zuerst,
//! ob überhaupt dasselbe Modell vorliegt, und sagt bei Abweichung klar,
//! dass das Artefakt und nicht die Hardware das Problem ist.
//!
//! Seit dem Skalenpaket (`INTEGER_LLM/scale_packs/`) ist der Bau
//! plattformübergreifend bitgleich: Die Skalen und LUTs sind versioniert,
//! die verbleibende Gewichtsquantisierung ist `round(W · 2^shift)` und
//! damit exakt. Die Prüfung bleibt trotzdem — eine Zusicherung, die man
//! nicht nachrechnet, ist eine Hoffnung.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Ein Modell, für das ein Skalenpaket veröffentlicht ist.
pub struct Bekannt {
    pub name: String,
    pub theta_v: String,
    pub digest: String,
    pub dateien: usize,
}

/// Zustand eines Modells auf dieser Maschine.
pub enum Zustand {
    /// Artefakte da und Digest stimmt — bereit für einen Vergleichslauf.
    Bereit { pfad: PathBuf },
    /// Artefakte da, aber Digest weicht ab. **Kein Hardware-Befund.**
    Abweichend { pfad: PathBuf, ist: String, soll: String },
    /// Keine Artefakte vorhanden.
    Fehlt,
}

fn sha256_datei(p: &Path) -> std::io::Result<String> {
    let mut h = Sha256::new();
    h.update(fs::read(p)?);
    Ok(format!("{:x}", h.finalize()))
}

/// Digest über ALLE Artefaktdateien: sortierte `pfad  hash`-Zeilen, davon
/// der SHA-256. Muss zeichengleich zu `tools/skalenpaket_bauen.py` sein —
/// sonst prüft der Client gegen eine andere Rechnung als die, die den
/// veröffentlichten Wert erzeugt hat.
pub fn artefakt_digest(dir: &Path) -> std::io::Result<(String, usize)> {
    let mut eintraege: Vec<(String, String)> = Vec::new();
    let mut stapel = vec![dir.to_path_buf()];
    while let Some(d) = stapel.pop() {
        for e in fs::read_dir(&d)? {
            let p = e?.path();
            if p.is_dir() {
                stapel.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                let h = sha256_datei(&p)?;
                eintraege.push((rel, h));
            }
        }
    }
    eintraege.sort();
    let text = eintraege
        .iter()
        .map(|(n, h)| format!("{}  {}", n, h))
        .collect::<Vec<_>>()
        .join("\n");
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    Ok((format!("{:x}", h.finalize()), eintraege.len()))
}

/// Liest `INTEGER_LLM/scale_packs/REGISTER.json`.
///
/// Bewusst ohne JSON-Crate: Der Client soll auf einer fremden Maschine mit
/// möglichst wenig Voraussetzungen bauen (siehe Modul-Doku von `main.rs`).
/// Das Register hat ein festes, flaches Format, das der Erzeuger schreibt.
pub fn register(repo: &Path) -> Result<Vec<Bekannt>, String> {
    let pfad = repo.join("INTEGER_LLM/scale_packs/REGISTER.json");
    let text = fs::read_to_string(&pfad)
        .map_err(|e| format!("{} nicht lesbar: {}", pfad.display(), e))?;

    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let (mut theta, mut digest, mut dateien) = (String::new(), String::new(), 0usize);
    for zeile in text.lines() {
        let t = zeile.trim();
        // Modellname: einziger Schlüssel auf Einrückungsebene 2 mit "{".
        if t.ends_with("\": {") && zeile.starts_with("  \"") {
            name = Some(t.trim_start_matches('"').trim_end_matches("\": {").to_string());
        } else if let Some(v) = feld(t, "artefakt_digest_sha256") {
            digest = v;
        } else if let Some(v) = feld(t, "theta_v") {
            theta = v;
        } else if let Some(v) = feld(t, "artefakt_dateien") {
            dateien = v.parse().unwrap_or(0);
        }
        if t == "}," || t == "}" {
            if let (Some(n), false) = (name.clone(), digest.is_empty()) {
                out.push(Bekannt { name: n, theta_v: theta.clone(), digest: digest.clone(), dateien });
                name = None;
                digest.clear();
            }
        }
    }
    if out.is_empty() {
        return Err(format!("keine Einträge in {}", pfad.display()));
    }
    Ok(out)
}

fn feld(zeile: &str, schluessel: &str) -> Option<String> {
    let praefix = format!("\"{}\": ", schluessel);
    let rest = zeile.trim().strip_prefix(&praefix)?;
    Some(rest.trim_end_matches(',').trim_matches('"').to_string())
}

/// Prüft ein Modell auf dieser Maschine.
pub fn pruefen(repo: &Path, m: &Bekannt) -> Zustand {
    let pfad = repo.join("INTEGER_LLM/artifacts").join(&m.name);
    if !pfad.join("weights_manifest.json").is_file() {
        return Zustand::Fehlt;
    }
    match artefakt_digest(&pfad) {
        Ok((ist, _)) if ist == m.digest => Zustand::Bereit { pfad },
        Ok((ist, _)) => Zustand::Abweichend { pfad, ist, soll: m.digest.clone() },
        Err(_) => Zustand::Fehlt,
    }
}

/// Die Anleitung, die ausgegeben wird, wenn Artefakte fehlen.
pub fn bauanleitung(modell: &str) -> String {
    format!(
        "So entstehen die Artefakte für {m} (einmalig):\n\
         \n\
         1. Gewichte von Hugging Face holen — sie werden NICHT mitgeliefert:\n\
         \x20     huggingface-cli download Qwen/{hf} --local-dir INTEGER_LLM/models/{hf}\n\
         \n\
         2. Artefakte bauen. Das versionierte Skalenpaket unter\n\
         \x20  INTEGER_LLM/scale_packs/{m}/ wird automatisch verwendet:\n\
         \x20     INTEGER_LLM_MODEL={m} python -m calibrate.src.main\n\
         \n\
         Der Bau dauert Sekunden statt Minuten, weil die Aktivierungsstatistik\n\
         entfällt — und genau deshalb ist er auf jeder Maschine bitgleich.\n\
         Danach diesen Befehl erneut ausführen; der Digest wird geprüft.",
        m = modell,
        hf = hf_id(modell),
    )
}

fn hf_id(modell: &str) -> &'static str {
    match modell {
        "qwen2.5-7b" => "Qwen2.5-7B",
        _ => "Qwen2.5-0.5B",
    }
}
