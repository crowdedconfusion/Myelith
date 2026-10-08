//! **Die Beschreibung eines Moduls** (`modul.json`): wer es ist, was es
//! startet, welche Befehle es hat, was es darf, und die Pruefsumme jeder
//! seiner Dateien.
//!
//! ⚑ **Streng gelesen.** Ein unbekanntes Feld ist ein Fehler, kein
//! uebersehener Zusatz: Was hier steht, ist signiert, und was die Konsole
//! nicht versteht, darf nicht unbemerkt mitreisen. Pfade sind relativ zum
//! Modulordner und koennen ihn nicht verlassen ([`sicherer_pfad`]).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// So viele Zeilen darf ein Modul unter dem Rahmen hoechstens belegen.
pub const UNTEN_HOECHSTENS: u16 = 24;

/// So viele Befehle darf ein Modul hoechstens nennen.
pub const BEFEHLE_HOECHSTENS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Beschreibung {
    /// Kleinbuchstaben, Ziffern und Bindestrich, 1 bis 32 Zeichen.
    pub name: String,
    /// `x.y.z`, nur Ziffern.
    pub version: String,
    /// Die Fassung des Protokolls, die das Modul spricht.
    pub protokoll: u32,
    /// Ein Satz fuer `/module`.
    pub was: String,
    pub laufzeit: Laufzeit,
    /// Der erste ist der Hauptbefehl (etwa `/beispiel`); alle weiteren
    /// beginnen mit ihm (`/beispiel status`). Sie stehen in Hilfe und
    /// Vervollstaendigung.
    pub befehle: Vec<Befehl>,
    #[serde(default)]
    pub befugnisse: Befugnisse,
    /// Ordner mit einer Werkzeugkiste fuer Agentenlaeufe des Moduls.
    #[serde(default)]
    pub kiste: Option<String>,
    /// Ordner mit Skills fuer diese Laeufe.
    #[serde(default)]
    pub skills: Option<String>,
    /// Der Arbeitsordner eines Agentenlaufs des Moduls; dort darf er
    /// schreiben, und nur dort. Liegt in einem veraenderlichen Ordner.
    #[serde(default)]
    pub arbeit: Option<String>,
    /// Ordner, deren Inhalt sich im Betrieb aendert (Daten, Zustand) und
    /// deshalb nicht in [`Self::dateien`] steht. Nie das Programm.
    #[serde(default)]
    pub veraenderlich: Vec<String>,
    /// Pfad (mit `/`) → SHA-256 als Hex, fuer **jede** Datei ausser der
    /// Beschreibung, der Signatur und den veraenderlichen Ordnern.
    #[serde(default)]
    pub dateien: BTreeMap<String, String>,
}

/// **Wie das Modul laeuft.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "art", rename_all = "snake_case", deny_unknown_fields)]
pub enum Laufzeit {
    /// Ein gewoehnliches Programm je Zielsystem ([`zielsystem`] → Pfad).
    /// ⛔️ Ohne Abschottung; startet nur mit einem als nativ vertrauten
    /// Schluessel.
    Nativ { programme: BTreeMap<String, String> },
    /// WebAssembly, abgeschottet.
    Wasm { datei: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Befehl {
    pub name: String,
    pub was: String,
}

/// **Was ein Modul ueber die Konsole darf.** Alles, was hier nicht steht,
/// lehnt die Konsole ab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Befugnisse {
    /// Das geladene Modell fragen.
    #[serde(default)]
    pub modell: bool,
    /// Agentenlaeufe mit der eigenen Werkzeugkiste.
    #[serde(default)]
    pub agent: bool,
    /// Modellfragen ueber das Netz statt am oertlichen Modell (Prompts
    /// verlassen dann den Rechner).
    #[serde(default)]
    pub netzmodell: bool,
    /// Zeilen unter dem Rahmen, hoechstens [`UNTEN_HOECHSTENS`].
    #[serde(default)]
    pub unten: u16,
    /// Die Fusszeile, solange der Modus aktiv ist.
    #[serde(default)]
    pub fusszeile: bool,
    /// Rechner, die ein abgeschottetes Modul ueber die Konsole erreichen
    /// darf (genau, ohne Platzhalter).
    #[serde(default)]
    pub http: Vec<String>,
    /// Token Modellantwort je Stunde, hoechstens; 0 heisst keine Grenze
    /// ausser der Befugnis selbst.
    #[serde(default)]
    pub modell_token_je_stunde: u32,
}

/// **Das Zielsystem dieses Rechners** als Schluessel fuer
/// [`Laufzeit::Nativ`], etwa `aarch64-macos`, `x86_64-linux`,
/// `x86_64-windows`.
pub fn zielsystem() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

/// **Ein Pfad, der im Modulordner bleibt**: relativ, mit `/` getrennt, ohne
/// `.` und `..`, ohne leere Teile, ohne `\` und `:` (Windows), nicht leer.
pub fn sicherer_pfad(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains(':')
        && !p.chars().any(char::is_control)
        && p.split('/').all(|t| !t.is_empty() && t != "." && t != "..")
}

fn version_teile(v: &str) -> Option<[u64; 3]> {
    let t: Vec<&str> = v.split('.').collect();
    if t.len() != 3 || t.iter().any(|x| x.is_empty() || x.len() > 9 || !x.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    Some([t[0].parse().ok()?, t[1].parse().ok()?, t[2].parse().ok()?])
}

/// **Vergleicht zwei Versionen** `x.y.z`; `None`, wenn eine unlesbar ist.
pub fn version_vergleichen(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    Some(version_teile(a)?.cmp(&version_teile(b)?))
}

impl Beschreibung {
    /// **Liest und prueft eine Beschreibung.** Geprueft wird hier nur, was
    /// die Beschreibung selbst sagt; ob die Dateien passen, prueft
    /// [`crate::signatur`].
    pub fn lesen(bytes: &[u8]) -> Result<Self, String> {
        let b: Beschreibung = serde_json::from_slice(bytes).map_err(|f| format!("{}: {f}", crate::BESCHREIBUNG))?;
        b.pruefen()?;
        Ok(b)
    }

    fn pruefen(&self) -> Result<(), String> {
        let name_gut = (1..=32).contains(&self.name.len()) && self.name.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
        if !name_gut {
            return Err(format!("Name „{}“: nur a bis z, 0 bis 9 und -, 1 bis 32 Zeichen", crate::filter::eine_zeile(&self.name)));
        }
        if version_teile(&self.version).is_none() {
            return Err(format!("{}: Version „{}“ ist nicht x.y.z", self.name, crate::filter::eine_zeile(&self.version)));
        }
        let pfade = self.dateien.keys().chain(self.veraenderlich.iter()).chain(self.kiste.iter()).chain(self.skills.iter()).chain(self.arbeit.iter());
        let programme: Vec<&String> = match &self.laufzeit {
            Laufzeit::Nativ { programme } => programme.values().collect(),
            Laufzeit::Wasm { datei } => vec![datei],
        };
        if let Some(p) = pfade.chain(programme.iter().copied()).find(|p| !sicherer_pfad(p)) {
            return Err(format!("{}: Pfad „{}“ verlaesst den Modulordner oder ist unzulaessig", self.name, crate::filter::eine_zeile(p)));
        }
        // Programm, Werkzeugkiste und Skills sind nie veraenderlich: Die Kiste
        // bestimmt, welche Programme ein Agentenlauf startet.
        for p in programme.iter().copied().chain(self.kiste.iter()).chain(self.skills.iter()) {
            if self.veraenderlich.iter().any(|v| p.as_str() == v || p.starts_with(&format!("{v}/"))) {
                return Err(format!("{}: {p} liegt in einem veraenderlichen Ordner und waere nicht signiert", self.name));
            }
        }
        if let Some(a) = self.arbeit.as_ref().filter(|a| !self.ist_veraenderlich(a)) {
            return Err(format!("{}: Der Arbeitsordner {a} liegt nicht in einem veraenderlichen Ordner", self.name));
        }
        if let Some((p, _)) = self.dateien.iter().find(|(_, h)| h.len() != 64 || !h.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())) {
            return Err(format!("{}: Pruefsumme von {p} ist kein SHA-256 in Kleinbuchstaben", self.name));
        }
        if self.befehle.is_empty() || self.befehle.len() > BEFEHLE_HOECHSTENS {
            return Err(format!("{}: 1 bis {BEFEHLE_HOECHSTENS} Befehle", self.name));
        }
        let haupt = &self.befehle[0].name;
        let haupt_gut = haupt.len() >= 2 && haupt.starts_with('/') && haupt[1..].bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
        if !haupt_gut {
            return Err(format!("{}: Der Hauptbefehl „{}“ ist kein /wort", self.name, crate::filter::eine_zeile(haupt)));
        }
        if let Some(b) = self.befehle[1..].iter().find(|b| !b.name.starts_with(&format!("{haupt} "))) {
            return Err(format!("{}: „{}“ beginnt nicht mit {haupt}", self.name, crate::filter::eine_zeile(&b.name)));
        }
        if self.befugnisse.unten > UNTEN_HOECHSTENS {
            return Err(format!("{}: unten hoechstens {UNTEN_HOECHSTENS} Zeilen", self.name));
        }
        Ok(())
    }

    /// Das Programm fuer dieses System, falls das Modul nativ ist.
    pub fn programm_hier(&self) -> Option<&str> {
        match &self.laufzeit {
            Laufzeit::Nativ { programme } => programme.get(&zielsystem()).map(String::as_str),
            Laufzeit::Wasm { datei } => Some(datei),
        }
    }

    /// Ob ein Pfad in einem veraenderlichen Ordner liegt.
    pub fn ist_veraenderlich(&self, p: &str) -> bool {
        self.veraenderlich.iter().any(|v| p == v || p.starts_with(&format!("{v}/")))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn beispiel() -> String {
        r#"{
          "name": "beispiel", "version": "0.1.0", "protokoll": 1, "was": "Ein Beispiel.",
          "laufzeit": {"art": "nativ", "programme": {"aarch64-macos": "bin/beispiel"}},
          "befehle": [{"name": "/beispiel", "was": "startet"}, {"name": "/beispiel status", "was": "zeigt"}],
          "befugnisse": {"modell": true, "unten": 12, "fusszeile": true},
          "veraenderlich": ["daten"],
          "dateien": {"bin/beispiel": "0000000000000000000000000000000000000000000000000000000000000000"}
        }"#
        .to_string()
    }

    #[test]
    fn gute_beschreibung() {
        let b = Beschreibung::lesen(beispiel().as_bytes()).unwrap();
        assert_eq!(b.name, "beispiel");
        assert!(b.befugnisse.modell && !b.befugnisse.agent && !b.befugnisse.netzmodell, "nicht genannt heisst nicht erlaubt");
        assert!(b.ist_veraenderlich("daten/x.json") && !b.ist_veraenderlich("datenbank"));
    }

    fn mit(alt: &str, neu: &str) -> Result<Beschreibung, String> {
        let t = beispiel();
        assert!(t.contains(alt), "{alt}");
        Beschreibung::lesen(t.replacen(alt, neu, 1).as_bytes())
    }

    #[test]
    fn was_nicht_durchgeht() {
        assert!(mit(r#""was": "Ein Beispiel.","#, r#""was": "Ein Beispiel.", "heimlich": true,"#).is_err(), "unbekanntes Feld");
        assert!(mit(r#""bin/beispiel": "0000"#, r#""../bin/beispiel": "0000"#).is_err(), "aus dem Ordner hinaus");
        assert!(mit(r#""aarch64-macos": "bin/beispiel""#, r#""aarch64-macos": "/usr/bin/sh""#).is_err(), "absolutes Programm");
        assert!(mit(r#""veraenderlich": ["daten"]"#, r#""veraenderlich": ["bin"]"#).is_err(), "Programm im veraenderlichen Ordner");
        assert!(mit(r#""veraenderlich": ["daten"]"#, r#""kiste": "daten/kiste", "veraenderlich": ["daten"]"#).is_err(), "Werkzeugkiste im veraenderlichen Ordner");
        assert!(mit(r#""veraenderlich": ["daten"]"#, r#""arbeit": "bin", "veraenderlich": ["daten"]"#).is_err(), "Arbeitsordner im signierten Teil");
        assert!(mit(r#""veraenderlich": ["daten"]"#, r#""arbeit": "daten/arbeit", "veraenderlich": ["daten"]"#).is_ok());
        assert!(mit(r#""name": "beispiel""#, r#""name": "Beispiel!""#).is_err());
        assert!(mit(r#""version": "0.1.0""#, r#""version": "1.0""#).is_err());
        assert!(mit(r#""/beispiel status""#, r#""/anderes status""#).is_err(), "Unterbefehl eines fremden Hauptbefehls");
        assert!(mit(r#""unten": 12"#, r#""unten": 99"#).is_err());
        assert!(mit(r#""name": "/beispiel", "was": "startet""#, r#""name": "beispiel", "was": "startet""#).is_err(), "Hauptbefehl ohne Schraegstrich");
    }

    #[test]
    fn pfade_und_versionen() {
        for gut in ["a", "a/b.json", "bin/x-1"] {
            assert!(sicherer_pfad(gut), "{gut}");
        }
        for schlecht in ["", "/a", "a/../b", "./a", "a//b", "a\\b", "C:a", "a/\u{7}"] {
            assert!(!sicherer_pfad(schlecht), "{schlecht:?}");
        }
        assert_eq!(version_vergleichen("0.10.0", "0.9.9"), Some(std::cmp::Ordering::Greater), "Zahlen, nicht Zeichen");
        assert_eq!(version_vergleichen("0.1", "0.1.0"), None);
    }
}
