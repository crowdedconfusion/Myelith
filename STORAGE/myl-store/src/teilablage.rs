//! **Teilablage eines Artefakts** (Stufe 4 des Dispatchers, P10 und P11,
//! 2026-10-22): welche Dateien ein Ebenenbereich braucht, und ob eine
//! Datei die ist, die das Manifest nennt.
//!
//! # ⚑ Kein neues Format
//!
//! Der Abdruck der Netzliste hasht `theta_v.json`, `model_config.json` und
//! `tokenizer.json`; darin steht die Summe des Gewichtsmanifests, und das
//! Manifest führt jede Datei mit ihrer SHA-256. Ein Knoten, der nur die
//! Dateien seines Bereichs hält, jede gegen das Manifest geprüft, hat
//! denselben Abdruck und steht auf derselben Netzliste.
//!
//! # ⚑ Was ein Bereich braucht
//!
//! Je Ebene im Bereich alle Dateien, deren Name mit `model_layers_<i>_`
//! beginnt, samt den Nebendateien (Shifts, Gitter, Beträge), die das
//! Manifest daneben nennt. Der erste Bereich dazu die Einbettung, der
//! letzte den Kopf und die letzte Norm. Die kleinen gemeinsamen Dateien
//! (Beschreibung, Wortschatz, Tabellen) stehen nicht im Manifest; sie liegen
//! bei jedem Knoten, der das Artefakt überhaupt hat, und werden hier nicht
//! geholt.

use myl_types::hash::Hash;
use serde_json::Value;

/// Eine Datei des Manifests: Name im Artefaktordner und ihre SHA-256 als
/// Hex.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Manifestdatei {
    pub datei: String,
    pub sha256: String,
}

/// Warum ein Manifest nicht gelesen wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Teilablagefehler {
    /// Kein JSON-Objekt.
    KeinManifest(String),
    /// Ein Eintrag nennt eine Datei ohne Summe, oder einen Namen, der aus
    /// dem Ordner hinausführt.
    Eintrag(String),
}

impl std::fmt::Display for Teilablagefehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeinManifest(g) => write!(f, "weights_manifest.json: {g}"),
            Self::Eintrag(g) => write!(f, "weights_manifest.json: {g}"),
        }
    }
}

/// Ist das ein Name, der im Artefaktordner bleibt? Kein Trenner, kein
/// `..`, nicht leer.
///
/// ⚑ **Die Prüfung, an der ein Dateidienst hängt**: Ein Knoten liefert nur
/// Dateien, die so heißen und im Manifest stehen; ein Name mit `/` hätte
/// ihn sonst jede Datei des Rechners lesen lassen.
pub fn name_ist_flach(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\']) && !name.contains("..")
}

/// **Alle Dateien des Manifests**, je Eintrag die Hauptdatei und die
/// Nebendateien (`<art>_file` mit `<art>_hash`), nach Namen geordnet.
pub fn manifestdateien(manifest: &str) -> Result<Vec<(String, Manifestdatei)>, Teilablagefehler> {
    let wert: Value = serde_json::from_str(manifest).map_err(|e| Teilablagefehler::KeinManifest(e.to_string()))?;
    let objekt = wert.as_object().ok_or_else(|| Teilablagefehler::KeinManifest("kein Objekt".into()))?;
    let mut aus = Vec::new();
    for (tensor, eintrag) in objekt {
        let e = eintrag.as_object().ok_or_else(|| Teilablagefehler::Eintrag(format!("{tensor}: kein Objekt")))?;
        for (schluessel, datei) in e {
            let Some(art) = schluessel.strip_suffix("file") else {
                continue;
            };
            let Some(datei) = datei.as_str() else {
                continue;
            };
            let summe = e
                .get(&format!("{art}hash"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| Teilablagefehler::Eintrag(format!("{tensor}: {schluessel} ohne Summe")))?;
            if !name_ist_flach(datei) {
                return Err(Teilablagefehler::Eintrag(format!("{tensor}: {datei} verlässt den Ordner")));
            }
            aus.push((tensor.clone(), Manifestdatei { datei: datei.to_string(), sha256: summe.to_string() }));
        }
    }
    aus.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(aus)
}

/// Die Ebene eines Tensors: `model_layers_12_…` → `Some(12)`.
fn ebene_des_tensors(tensor: &str) -> Option<u32> {
    let rest = tensor.strip_prefix("model_layers_")?;
    let ende = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..ende].parse().ok()
}

/// **Die Dateien des Bereichs `[von, bis)`**, mit Einbettung am ersten und
/// Kopf samt letzter Norm am letzten Bereich.
pub fn dateien_des_bereichs(
    manifest: &str,
    von: u32,
    bis: u32,
    mit_einbettung: bool,
    mit_kopf: bool,
) -> Result<Vec<Manifestdatei>, Teilablagefehler> {
    let mut aus: Vec<Manifestdatei> = manifestdateien(manifest)?
        .into_iter()
        .filter(|(tensor, _)| match ebene_des_tensors(tensor) {
            Some(i) => von <= i && i < bis,
            None if tensor.starts_with("model_embed_tokens") => mit_einbettung,
            None => mit_kopf,
        })
        .map(|(_, d)| d)
        .collect();
    aus.dedup();
    Ok(aus)
}

/// Der Name der Datei, die ein Knoten neben sein Protokoll legt.
pub const NETZLAST_DATEI: &str = "netzlast.json";

/// ⚑ **Die Netzlast eines Rechners** (Stufe 4, P7 und P8, 2026-10-22): Der
/// Knoten schreibt sie, wenn er einen Shard aus der Kette startet, und
/// entfernt sie ohne Shard; der lokale Plan des Klienten liest sie und gibt
/// dem Shard Vorrang vor dem Agenten (Netz vor Mensch).
///
/// ⚑ **Hier und nicht im Knoten oder im Klienten**: Beide binden diese Kiste
/// ein, und ein Format an zwei Stellen liefe auseinander.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Netzlast {
    /// Das Artefakt des Netzmodells.
    pub artefakte: std::path::PathBuf,
    /// Die Epoche des Auftrags.
    pub epoche: u64,
    /// Der Pod und die Position darin.
    pub pod: u32,
    pub position: u32,
    /// Wie viele Positionen der Pod hat.
    pub positionen: u32,
    /// Der Bereich `[von, bis)`.
    pub von: u32,
    pub bis: u32,
}

impl Netzlast {
    /// Hält dieser Shard die Einbettung?
    pub fn mit_einbettung(&self) -> bool {
        self.position == 0
    }

    /// Trägt er den Kopf?
    pub fn mit_kopf(&self) -> bool {
        self.position + 1 == self.positionen
    }

    /// Liest sie, oder `None`, wenn keine da ist oder sie nicht lesbar ist.
    pub fn lesen(pfad: &std::path::Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(pfad).ok()?).ok()
    }

    /// Schreibt sie.
    pub fn schreiben(&self, pfad: &std::path::Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(pfad, text)
    }
}

/// Ist `inhalt` die Datei, die das Manifest nennt?
pub fn passt(inhalt: &[u8], datei: &Manifestdatei) -> bool {
    Hash::sha256(inhalt).to_hex() == datei.sha256
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> String {
        let h = |b: &[u8]| Hash::sha256(b).to_hex();
        format!(
            r#"{{
  "lm_head": {{"file": "lm_head.bin", "hash": "{a}", "shifts_file": "lm_head_shifts.bin", "shifts_hash": "{b}"}},
  "model_embed_tokens_weight": {{"file": "model_embed_tokens_weight.bin", "hash": "{c}"}},
  "model_norm_weight": {{"file": "model_norm_weight.bin", "hash": "{d}"}},
  "model_layers_0_mlp_down_proj_weight": {{"file": "model_layers_0_mlp_down_proj_weight.bin", "hash": "{e}"}},
  "model_layers_1_mlp_experts_0_down_proj_weight": {{"file": "model_layers_1_x.codes.bin", "hash": "{f}", "gitter_file": "model_layers_1_x.gitter.bin", "gitter_hash": "{g}"}},
  "model_layers_12_mlp_down_proj_weight": {{"file": "model_layers_12_mlp_down_proj_weight.bin", "hash": "{e}"}}
}}"#,
            a = h(b"kopf"),
            b = h(b"kopfshift"),
            c = h(b"einbettung"),
            d = h(b"norm"),
            e = h(b"ebene"),
            f = h(b"codes"),
            g = h(b"gitter"),
        )
    }

    fn namen(d: &[Manifestdatei]) -> Vec<&str> {
        d.iter().map(|x| x.datei.as_str()).collect()
    }

    /// ⚑ **Der Bereich bestimmt die Dateien**: Ebene 1 mit Nebendatei, aber
    /// nicht Ebene 12 (Präfix `1` ist nicht `12`); Einbettung nur vorn,
    /// Kopf und Norm nur hinten.
    #[test]
    fn ein_bereich_braucht_seine_dateien() {
        let m = manifest();
        let mitte = dateien_des_bereichs(&m, 1, 2, false, false).expect("Manifest");
        assert_eq!(namen(&mitte), ["model_layers_1_x.codes.bin", "model_layers_1_x.gitter.bin"]);
        let vorn = dateien_des_bereichs(&m, 0, 1, true, false).expect("Manifest");
        assert_eq!(namen(&vorn), ["model_embed_tokens_weight.bin", "model_layers_0_mlp_down_proj_weight.bin"]);
        let hinten = dateien_des_bereichs(&m, 12, 13, false, true).expect("Manifest");
        assert_eq!(
            namen(&hinten),
            ["lm_head.bin", "lm_head_shifts.bin", "model_layers_12_mlp_down_proj_weight.bin", "model_norm_weight.bin"]
        );
    }

    #[test]
    fn die_netzlast_geht_hin_und_zurueck() {
        let p = std::env::temp_dir().join(format!("myl-netzlast-{}.json", std::process::id()));
        let n = Netzlast { artefakte: "x".into(), epoche: 4, pod: 1, position: 3, positionen: 4, von: 21, bis: 28 };
        n.schreiben(&p).expect("schreiben");
        assert_eq!(Netzlast::lesen(&p), Some(n.clone()));
        assert!(n.mit_kopf() && !n.mit_einbettung());
        let _ = std::fs::remove_file(&p);
        assert_eq!(Netzlast::lesen(&p), None);
    }

    /// ⚑ **Eine Datei passt nur mit ihrer Summe**, und ein Name, der den
    /// Ordner verlässt, macht das Manifest ungültig.
    #[test]
    fn summe_und_name_werden_geprueft() {
        let m = manifest();
        let kopf = dateien_des_bereichs(&m, 12, 13, false, true).expect("Manifest");
        let datei = kopf.iter().find(|d| d.datei == "lm_head.bin").expect("Kopf");
        assert!(passt(b"kopf", datei));
        assert!(!passt(b"kopf!", datei));
        for schlecht in ["../geheim", "a/b", "", "..", "x\\\\y"] {
            assert!(!name_ist_flach(schlecht), "{schlecht}");
        }
        let boese = r#"{"x": {"file": "../../etc/passwd", "hash": "00"}}"#;
        assert!(matches!(manifestdateien(boese), Err(Teilablagefehler::Eintrag(_))));
        let ohne = r#"{"x": {"file": "a.bin"}}"#;
        assert!(matches!(manifestdateien(ohne), Err(Teilablagefehler::Eintrag(_))));
    }
}
