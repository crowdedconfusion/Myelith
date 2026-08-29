//! Das Tor: Ein Knoten, der anders rechnet, kommt nicht ins Netz.
//!
//! ## ⚑ Warum das vor dem Netz steht und nicht daneben
//!
//! Die ganze Zusage des Protokolls ist, dass zwei beliebige Rechner
//! dasselbe Ergebnis bekommen, Bit für Bit. **Ein Knoten, dessen
//! Maschine anders rechnet, ist kein langsamer Knoten, sondern ein
//! schädlicher:** Er liefert Segmente, die von den redundanten
//! abweichen, wird geschlachtet, und bis dahin hat er den
//! Auftragsstrom verschmutzt.
//!
//! Prüfen lässt sich das seit dem 2026-08-27 mit dem Testclient. **Das
//! ist freiwillig und getrennt vom Betrieb**, also passiert es beim
//! ersten Mal und danach nie wieder, und niemand merkt es, wenn eine
//! Bibliothek unter dem Knoten ausgetauscht wird.
//!
//! ## ⚑ Was das Tor prüft und was ausdrücklich nicht
//!
//! Es fährt die **Op-Vektoren**: einzelne Kernel gegen eingefrorene
//! Sollwerte. Die brauchen kein Modell und laufen in Millisekunden.
//!
//! **Die Layer- und Ende-zu-Ende-Vektoren bleiben draußen**, und zwar
//! nicht aus Bequemlichkeit: Sie verlangen Modellartefakte in
//! Gigabyte-Größe. Ein Start, der davon abhinge, wäre für die meisten
//! Betreiber kein Start. ⚑ **Das Tor belegt damit, dass die Kernel
//! übereinstimmen, nicht dass die ganze Kette es tut**, und wer es
//! anders liest, liest zu viel hinein.
//!
//! ## ⚑ Fehlende Vektoren sind etwas anderes als falsche
//!
//! Ein **falscher** Vektor heißt: Diese Maschine rechnet anders. Ein
//! **fehlendes** Verzeichnis heißt: Wir wissen es nicht. Das erste ist
//! ein Grund abzubrechen, das zweite auch — aber aus einem anderen
//! Grund, und deshalb steht es als eigener Fall da.
//!
//! **Wer ohne Vektoren starten will, sagt es ausdrücklich.** Der
//! Schalter macht die Entscheidung sichtbar: „Ich habe die Prüfung
//! abgeschaltet" steht dann in der Kommandozeile und im Protokoll, und
//! das ist etwas ganz anderes als „es wurde nicht geprüft". Dieselbe
//! Überlegung wie bei den benannten Ausnahmen im Gleitkomma-Audit.

use std::path::{Path, PathBuf};

/// Wie das Tor ausgegangen ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Torbefund {
    /// Alle Vektoren stimmen.
    Bestanden {
        /// Wie viele.
        vektoren: usize,
    },
    /// Mindestens einer weicht ab. **Diese Maschine rechnet anders.**
    Abweichung {
        /// Welche, mit Grund.
        fehler: Vec<String>,
        /// Wie viele insgesamt geprüft wurden.
        vektoren: usize,
    },
    /// Kein Vektor gefunden. **Wir wissen es nicht.**
    Keine {
        /// Wo gesucht wurde.
        pfad: PathBuf,
    },
    /// Ausdrücklich übersprungen.
    Uebersprungen,
}

impl Torbefund {
    /// Darf der Knoten starten?
    pub fn darf_starten(&self) -> bool {
        matches!(self, Self::Bestanden { .. } | Self::Uebersprungen)
    }

    /// Eine Zeile fürs Betriebsprotokoll.
    pub fn zeile(&self) -> String {
        match self {
            Self::Bestanden { vektoren } => {
                format!("konformitaet=bestanden vektoren={vektoren}")
            }
            Self::Abweichung { fehler, vektoren } => format!(
                "konformitaet=abweichung vektoren={vektoren} abweichungen={}",
                fehler.len()
            ),
            Self::Keine { pfad } => format!("konformitaet=keine pfad={}", pfad.display()),
            // ⚑ Ausdrücklich als eigener Wert und nicht als „bestanden":
            // Wer das Protokoll liest, muss sehen, dass nicht geprüft
            // wurde, statt es für geprüft zu halten.
            Self::Uebersprungen => "konformitaet=uebersprungen".to_string(),
        }
    }
}

/// Fährt die Op-Vektoren aus `pfad`.
///
/// Die Reihenfolge ist die des Dateinamens, damit zwei Läufe dieselbe
/// Ausgabe erzeugen: Die Reihenfolge des Dateisystems ist es nicht.
pub fn pruefe(pfad: &Path) -> Torbefund {
    let mut dateien: Vec<PathBuf> = match std::fs::read_dir(pfad) {
        Ok(eintraege) => eintraege
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".golden.json"))
            .collect(),
        Err(_) => return Torbefund::Keine { pfad: pfad.to_path_buf() },
    };
    if dateien.is_empty() {
        return Torbefund::Keine { pfad: pfad.to_path_buf() };
    }
    dateien.sort();

    let mut fehler = Vec::new();
    for datei in &dateien {
        match integer_llm_kernels::konformitaet::op_vektor_aus_datei(datei) {
            Ok(e) if e.bestanden => {}
            Ok(e) => fehler.push(format!(
                "{}: {}",
                datei.file_name().unwrap_or_default().to_string_lossy(),
                if e.gruende.is_empty() { "abweichend".into() } else { e.gruende.join("; ") }
            )),
            Err(grund) => fehler.push(format!(
                "{}: {grund}",
                datei.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    if fehler.is_empty() {
        Torbefund::Bestanden { vektoren: dateien.len() }
    } else {
        Torbefund::Abweichung { fehler, vektoren: dateien.len() }
    }
}

/// Die Orte, an denen ohne `--konformitaet` gesucht wird, in dieser
/// Reihenfolge.
///
/// ⚑ **Zwei Orte, weil ein ausgeliefertes Binary anders liegt als ein
/// gebautes.** Im Repositorium arbeitet man vom Wurzelverzeichnis aus;
/// ein Betreiber entpackt ein Archiv und startet das Programm darin.
/// Ein Vorgabepfad, der nur den ersten Fall trifft, macht das Tor für
/// genau die Leute unbrauchbar, für die es gedacht ist — **und sie
/// greifen dann zu `--ohne-konformitaet`, womit die Prüfung praktisch
/// abgeschafft wäre.**
pub fn vorgabeorte() -> Vec<PathBuf> {
    let mut orte = vec![PathBuf::from("INTEGER_LLM/conformance/vectors/op")];
    // Neben dem Binary, wie in einem entpackten Archiv.
    if let Ok(bin) = std::env::current_exe() {
        if let Some(ordner) = bin.parent() {
            orte.push(ordner.join("conformance/vectors/op"));
            orte.push(ordner.join("vectors/op"));
        }
    }
    orte
}

/// Der erste Ort, an dem Vektoren liegen, sonst der erste der Liste.
///
/// **Der erste der Liste als Rückfall**, damit die Fehlermeldung einen
/// Pfad nennen kann, statt „nirgends" zu sagen.
pub fn vorgabepfad() -> PathBuf {
    let orte = vorgabeorte();
    orte.iter()
        .find(|p| p.is_dir())
        .cloned()
        .unwrap_or_else(|| orte[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vektoren() -> PathBuf {
        // Vom Crate-Verzeichnis aus zwei Ebenen hoch.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(vorgabepfad())
    }

    #[test]
    fn die_echten_vektoren_bestehen() {
        let b = pruefe(&vektoren());
        assert!(matches!(b, Torbefund::Bestanden { .. }), "{b:?}");
        assert!(b.darf_starten());
        assert!(b.zeile().starts_with("konformitaet=bestanden"));
    }

    /// ⚑ Fehlende Vektoren halten den Knoten an, und zwar mit einem
    /// **anderen** Befund als abweichende. „Wir wissen es nicht" ist
    /// nicht „es ist falsch", und beides ist nicht „es ist richtig".
    #[test]
    fn fehlende_vektoren_halten_an_und_heissen_anders() {
        let nirgends = PathBuf::from("/gibt/es/nicht");
        let b = pruefe(&nirgends);
        assert_eq!(b, Torbefund::Keine { pfad: nirgends });
        assert!(!b.darf_starten());
        assert!(b.zeile().starts_with("konformitaet=keine"));
    }

    /// Ein leeres Verzeichnis ist dasselbe wie keines: Es beweist nichts.
    #[test]
    fn ein_leeres_verzeichnis_beweist_nichts() {
        let leer = std::env::temp_dir()
            .join(format!("myl-tor-leer-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&leer).expect("anlegen");
        let b = pruefe(&leer);
        assert!(matches!(b, Torbefund::Keine { .. }), "{b:?}");
        std::fs::remove_dir_all(&leer).ok();
    }

    /// ⚑ Die Gegenprobe, die zeigt, dass das Tor überhaupt etwas taugt:
    /// Ein verfälschter Vektor hält den Knoten an.
    #[test]
    fn ein_verfaelschter_vektor_haelt_an() {
        let ordner = std::env::temp_dir()
            .join(format!("myl-tor-falsch-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&ordner).expect("anlegen");

        // Einen echten Vektor nehmen und eine Zahl darin ändern.
        let quelle = std::fs::read_dir(vektoren())
            .expect("Vektoren")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.to_string_lossy().ends_with(".golden.json"))
            .expect("mindestens einer");
        let inhalt = std::fs::read_to_string(&quelle).expect("lesen");

        // ⚑ **Gezielt den Erwartungswert ersetzen, nicht Ziffern
        // tauschen.** Zwei Anläufe gingen daneben und beide lehrreich:
        // Der erste ersetzte blind die erste `1` im Text, und die stand
        // in einem Feld, das die Prüfung nicht ansieht. Der zweite
        // tauschte Ziffern im richtigen Feld, und der Wert dort war
        // `[64,-64]`, enthielt also keine der getauschten Ziffern.
        //
        // **Eine Gegenprobe, die nichts verändert, belegt nichts** und
        // hätte hier ein Tor als wirksam ausgewiesen, das es nicht ist.
        // Deshalb wird jetzt die erste Zahl der letzten Datenliste durch
        // einen Wert ersetzt, den kein Kernel liefert, und der Test
        // prüft vorher, dass sich der Text wirklich geändert hat.
        let marke = "\"data\":[";
        let anfang = inhalt.rfind(marke).expect("ein Ausgabefeld") + marke.len();
        let ende = inhalt[anfang..]
            .find([',', ']'])
            .expect("Ende der ersten Zahl")
            + anfang;
        let verfaelscht = format!("{}{}{}", &inhalt[..anfang], "987654", &inhalt[ende..]);
        assert_ne!(inhalt, verfaelscht, "die Probe muss den Erwartungswert verändern");
        let ziel = ordner.join("verfaelscht.golden.json");
        std::fs::write(&ziel, verfaelscht).expect("schreiben");

        let b = pruefe(&ordner);
        assert!(!b.darf_starten(), "ein verfälschter Vektor muss anhalten: {b:?}");
        std::fs::remove_dir_all(&ordner).ok();
    }

    /// ⚑ Der Vorgabepfad muss auch dann etwas nennen, wenn nichts
    /// existiert: Eine Fehlermeldung ohne Pfad hilft niemandem.
    #[test]
    fn der_vorgabepfad_nennt_immer_einen_ort() {
        let orte = vorgabeorte();
        assert!(!orte.is_empty(), "mindestens der Repositoriums-Pfad");
        assert!(orte.contains(&PathBuf::from("INTEGER_LLM/conformance/vectors/op")));
        // Der gewählte Pfad ist einer aus der Liste, nie leer.
        assert!(orte.contains(&vorgabepfad()));
    }

    #[test]
    fn uebersprungen_heisst_nicht_bestanden() {
        let b = Torbefund::Uebersprungen;
        assert!(b.darf_starten(), "wer es abschaltet, darf starten");
        assert_eq!(b.zeile(), "konformitaet=uebersprungen");
        assert_ne!(b.zeile(), Torbefund::Bestanden { vektoren: 6 }.zeile());
    }
}
