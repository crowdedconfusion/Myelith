//! **Ein eigenes Modell bauen**: der Weg des Bauskripts
//! (`INTEGER_LLM/scripts/modell_bauen.sh`) fuer die Konsole (`/build artifact`).
//!
//! ⚑ **Das Skript entscheidet, die Konsole fragt.** Freigabe, Parameterzahl,
//! Namensvorschlaege und Platz rechnet das Skript (`--erkunden`, eine Zeile
//! `ERKUNDUNG <json>`); hier steht nur, wie man es ruft und seine Antwort
//! liest. Eine zweite Freigabe in Rust liefe auseinander mit der ersten.
//!
//! ⛔️ **Was so entsteht, ist privat und lokal**: Das Artefakt traegt
//! `betrieb: lokal`, der Netzbetrieb laedt es nicht. Ausgenommen ist ein
//! Modell dieses Projekts (`projekt`), das unter seinem eigenen Namen baut.

use crate::bezug::Zeile;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Woher die Gewichte kommen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quelle {
    /// Ein Repositorium auf Hugging Face: der Link
    /// (`https://huggingface.co/<org>/<modell>`) oder `<org>/<modell>`. Lesen
    /// tut ihn das Bauskript (`repo_aus_link`), sonst niemand.
    Repo(String),
    /// Ein Ordner mit `config.json` und safetensors auf dieser Maschine.
    Ordner(PathBuf),
}

impl Quelle {
    fn argumente(&self) -> Vec<String> {
        match self {
            Quelle::Repo(r) => vec!["--repo".into(), r.clone()],
            Quelle::Ordner(o) => vec!["--lokal".into(), o.display().to_string()],
        }
    }
}

/// Ein Namensvorschlag des Skripts.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Vorschlag {
    pub name: String,
    /// Die Experten in vier Bit.
    pub vierbit: bool,
    /// Warum der Name nicht geht, sonst leer.
    pub grund: String,
}

/// **Was das Skript ueber die Quelle herausgefunden hat** (`--erkunden`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Erkundung {
    pub repo: String,
    /// Die festgehaltene Revision (bei einem Ordner `lokal`).
    pub revision: String,
    pub lokal: bool,
    pub lizenz: String,
    pub architektur: Vec<String>,
    pub bauart: String,
    pub bauweg: String,
    pub gemisch: bool,
    pub vierbit: bool,
    /// Parameter in Milliarden.
    pub gesamt: f64,
    pub aktiv: f64,
    pub tabelle: f64,
    /// Der Schluessel eines Modells dieses Projekts, sonst leer.
    pub projekt: String,
    /// ⛔️ **Fund 584:** Im Ordner liegen keine Gewichtsdateien; dann gibt es
    /// weder eine Parameterzahl noch einen Namen, und gebaut wird nichts.
    #[serde(default)]
    pub ohne_gewichte: bool,
    pub vorschlaege: Vec<Vorschlag>,
    /// Groesse und Speicherform, die an einen eingegebenen Namen kommen
    /// (`126b-a7b-int4`, `126b-a7b-int8`); bei einem Projektmodell leer.
    #[serde(default)]
    pub anhaengsel: Vec<String>,
    pub quelle_bytes: u64,
    pub frei_bytes: u64,
    /// Der Hoehepunkt des Platzes fuer das int8 (auch fuer vier Bit, wenn die
    /// Quelle vorher geloescht wird).
    pub noetig_bytes: u64,
    /// Der Hoehepunkt mit vier Bit, wenn die Quelle bleibt.
    pub noetig_vierbit_bytes: u64,
    /// Bei `--name`: der gepruefte Name und der Grund dagegen (leer, wenn er geht).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_grund: String,
}

impl Erkundung {
    /// Wie viel Platz der Bau auf seinem Hoehepunkt braucht.
    pub fn noetig(&self, vierbit: bool, quelle_loeschen: bool) -> u64 {
        if vierbit && !(quelle_loeschen && !self.lokal) {
            self.noetig_vierbit_bytes
        } else {
            self.noetig_bytes
        }
    }
}

/// **Liest die Erkundung aus der Ausgabe des Skripts**: die Zeile
/// `ERKUNDUNG <json>`.
pub fn erkundung_lesen(zeilen: &[String]) -> Result<Erkundung, String> {
    let zeile = zeilen
        .iter()
        .rev()
        .find_map(|z| z.strip_prefix("ERKUNDUNG "))
        .ok_or_else(|| fehlergrund(zeilen, "das Skript hat keine Erkundung geliefert"))?;
    serde_json::from_str(zeile).map_err(|e| format!("die Erkundung ist nicht lesbar: {e}"))
}

/// **Der Grund eines gescheiterten Laufs**: jede Zeile `FEHLER: …`, in ihrer
/// Reihenfolge.
///
/// 📌 Nicht nur die letzte (2026-10-19): Scheitert ein Schritt, schreibt er
/// selbst `FEHLER: <Ursache>`, und das Bauskript danach `FEHLER: Schritt endete
/// mit 1: bash`. Die letzte allein sagte nichts.
fn fehlergrund(zeilen: &[String], sonst: &str) -> String {
    let gruende: Vec<&str> = zeilen.iter().filter_map(|z| z.strip_prefix("FEHLER: ")).collect();
    if gruende.is_empty() { sonst.to_string() } else { gruende.join("; ") }
}

/// **Was gebaut werden soll.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auftrag {
    pub quelle: Quelle,
    /// Die Revision aus der Erkundung: gebaut wird, was erkundet wurde.
    pub revision: String,
    pub name: String,
    /// Ein Modell dieses Projekts (`--projekt`).
    pub projekt: bool,
    pub quelle_loeschen: bool,
}

impl Auftrag {
    /// Die Schalter fuer das Skript; `probe` fuer den kurzen Lauf.
    pub fn argumente(&self, probe: bool) -> Vec<String> {
        let mut a = self.quelle.argumente();
        if matches!(self.quelle, Quelle::Repo(_)) && !self.revision.is_empty() {
            a.extend(["--revision".into(), self.revision.clone()]);
        }
        a.extend(["--name".into(), self.name.clone(), "--ja".into()]);
        if self.projekt {
            a.push("--projekt".into());
        }
        if self.quelle_loeschen && !probe {
            a.push("--quelle-loeschen".into());
        }
        if probe {
            a.push("--probe".into());
        }
        a
    }
}

/// Faehrt das Bauskript und gibt seine Zeilen zurueck; jede geht sofort an
/// `sagen`.
fn fahren(wurzel: &Path, argumente: &[String], phase: &'static str, sagen: &(dyn Fn(Zeile) + Sync)) -> Result<Vec<String>, String> {
    let gesammelt = Mutex::new(Vec::new());
    let mitschreiben = |z: Zeile| {
        if let Ok(mut g) = gesammelt.lock() {
            g.push(z.text.clone());
        }
        sagen(z);
    };
    let a: Vec<&str> = argumente.iter().map(String::as_str).collect();
    let ergebnis = crate::bezug::skript(wurzel, "modell_bauen.sh", &a, &[], phase, &mitschreiben);
    let zeilen = gesammelt.into_inner().unwrap_or_default();
    match ergebnis {
        Ok(()) => Ok(zeilen),
        Err(f) => Err(fehlergrund(&zeilen, &f)),
    }
}

/// **Erkundet die Quelle**: Bauart, Freigabe, Parameter, Namen, Platz. Mit
/// `name` prueft das Skript auch diesen Namen. Gebaut wird nichts; bei einem
/// Repositorium laedt es die kleinen Dateien (Konfiguration, Tokenizer).
pub fn erkunden(wurzel: &Path, quelle: &Quelle, name: Option<&str>, sagen: &(dyn Fn(Zeile) + Sync)) -> Result<Erkundung, String> {
    let mut a = quelle.argumente();
    a.push("--erkunden".into());
    if let Some(n) = name {
        a.extend(["--name".into(), n.to_string()]);
    }
    erkundung_lesen(&fahren(wurzel, &a, "erkunden", sagen)?)
}

/// **Baut**: erst der kurze Lauf (`--probe`), dann der lange. Gibt den Pfad
/// des Artefakts zurueck.
///
/// ⚑ Der kurze Lauf zuerst, wie bei jedem langen Lauf in diesem Projekt: Er
/// prueft alles bis zur ersten Ebene auf den echten Gewichten und schreibt
/// nichts. Scheitert er, beginnt der lange nicht.
pub fn bauen(wurzel: &Path, auftrag: &Auftrag, sagen: &(dyn Fn(Zeile) + Sync)) -> Result<PathBuf, String> {
    fahren(wurzel, &auftrag.argumente(true), "probe", sagen)?;
    fahren(wurzel, &auftrag.argumente(false), "bau", sagen)?;
    let ziel = wurzel.join(crate::modelle::ARTEFAKTORDNER).join(&auftrag.name);
    if !ziel.join("model_config.json").is_file() {
        return Err(format!("Der Lauf ist durch, aber {} fehlt.", ziel.join("model_config.json").display()));
    }
    Ok(ziel)
}

#[cfg(test)]
mod proben {
    use super::*;

    const ZEILE: &str = r#"ERKUNDUNG {"repo": "lokal/Qwen3-0.6B", "revision": "lokal", "lokal": true, "ordner": "/x", "lizenz": "apache-2.0", "architektur": ["Qwen3ForCausalLM"], "bauart": "Qwen3, dicht", "bauweg": "korpus", "gemisch": false, "vierbit": false, "gesamt": 0.596, "aktiv": 0.596, "tabelle": 0.0, "projekt": "", "vorschlaege": [{"name": "sparrow-0.6b-int8", "vierbit": false, "grund": "der Name ist schon vergeben"}], "quelle_bytes": 1503264768, "frei_bytes": 300638666752, "noetig_bytes": 2099314688, "noetig_vierbit_bytes": 2635759616}"#;

    /// ⚑ Die Zeile, wie das Skript sie am 2026-10-19 geschrieben hat; ein
    /// unbekanntes Feld (`ordner`) stoert nicht.
    #[test]
    fn die_erkundung_wird_gelesen() {
        let e = erkundung_lesen(&["[bauen] irgendwas".into(), ZEILE.into(), "Fertig".into()]).expect("lesbar");
        assert_eq!(e.bauart, "Qwen3, dicht");
        assert_eq!(e.vorschlaege[0].grund, "der Name ist schon vergeben");
        assert_eq!(e.noetig(false, false), 2099314688);
        assert!(e.name_grund.is_empty());
    }

    /// Ohne Erkundung ist der Grund die letzte Zeile `FEHLER: …`.
    #[test]
    fn ohne_erkundung_der_grund_des_skripts() {
        let f = erkundung_lesen(&["x".into(), "FEHLER: Architektur ['bert'] wird nicht unterstuetzt".into()]).unwrap_err();
        assert_eq!(f, "Architektur ['bert'] wird nicht unterstuetzt");
    }

    /// Die Ursache eines Schritts kommt vor dem Satz des Bauskripts.
    #[test]
    fn die_ursache_bleibt_stehen() {
        let z: Vec<String> = ["[vierbit] x", "FEHLER: ein Teil des Gitterbaus ist gescheitert", "FEHLER: Schritt endete mit 1: bash"].map(String::from).to_vec();
        assert_eq!(fehlergrund(&z, "-"), "ein Teil des Gitterbaus ist gescheitert; Schritt endete mit 1: bash");
    }

    /// ⚑ Gebaut wird die erkundete Revision; geloescht wird nie im kurzen
    /// Lauf und nie ein mitgebrachter Ordner (das sagt das Skript selbst).
    #[test]
    fn die_schalter_des_auftrags() {
        let a = Auftrag {
            quelle: Quelle::Repo("Qwen/Qwen3.8-Flash-Next".into()),
            revision: "de4b8e4d".into(),
            name: "vulture-126b-a7b-int4".into(),
            projekt: true,
            quelle_loeschen: true,
        };
        assert_eq!(
            a.argumente(true),
            ["--repo", "Qwen/Qwen3.8-Flash-Next", "--revision", "de4b8e4d", "--name", "vulture-126b-a7b-int4", "--ja", "--projekt", "--probe"]
        );
        assert!(a.argumente(false).contains(&"--quelle-loeschen".to_string()));
        let o = Auftrag { quelle: Quelle::Ordner("/m/q".into()), revision: "lokal".into(), name: "n".into(), projekt: false, quelle_loeschen: false };
        assert_eq!(o.argumente(false), ["--lokal", "/m/q", "--name", "n", "--ja"]);
    }

    /// Mit vier Bit braucht es mehr Platz, ausser die Quelle geht vorher weg.
    #[test]
    fn platz_mit_vier_bit() {
        let e = erkundung_lesen(&[ZEILE.into()]).unwrap();
        assert_eq!(e.noetig(true, false), e.noetig_vierbit_bytes);
        let mut r = e.clone();
        r.lokal = false;
        assert_eq!(r.noetig(true, true), r.noetig_bytes);
        assert_eq!(e.noetig(true, true), e.noetig_vierbit_bytes, "ein Ordner wird nie geloescht");
    }
}
