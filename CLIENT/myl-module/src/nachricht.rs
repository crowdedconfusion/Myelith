//! **Die Nachrichten zwischen Konsole und Modul**, je eine JSON-Zeile.
//!
//! ⚑ **Ereignisgetrieben, ohne Faeden im Modul.** Die Konsole schickt
//! Ereignisse ([`AnModul`]): einen getippten Befehl, einen Takt, ein Stueck
//! einer Modellantwort, die Antwort des Menschen. Das Modul antwortet mit
//! beliebig vielen Nachrichten ([`VomModul`]) und kehrt zurueck. Wer auf das
//! Modell wartet, wartet nicht im Modul, sondern bekommt die Antwort als
//! neues Ereignis; dazwischen laufen Takte weiter. So geht dasselbe
//! Protokoll fuer ein natives Programm und fuer WebAssembly, das keine
//! Faeden hat.
//!
//! ⚑ **Befehle stehen nicht hier, sondern in der signierten
//! Beschreibung.** Ein Modul kann zur Laufzeit keinen Befehl anmelden, den
//! niemand gesehen hat.
//!
//! Grenzen: eine Zeile hoechstens [`ZEILE_HOECHSTENS`] Bytes; was die
//! Konsole von einem Modul annimmt, begrenzen zusaetzlich seine
//! Befugnisse.

use serde::{Deserialize, Serialize};

/// So lang darf eine Nachricht als Zeile hoechstens sein, Bytes.
pub const ZEILE_HOECHSTENS: usize = 1 << 20;

/// **Von der Konsole an das Modul.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum AnModul {
    /// Der Handschlag, als erste Nachricht.
    Hallo {
        protokoll: u32,
        /// Fassung der Konsole, nur zur Anzeige.
        konsole: String,
        /// `de` oder `en`.
        sprache: String,
        /// Das geladene Modell, falls eines geladen ist.
        modell: Option<String>,
        breite: u16,
        hoehe: u16,
    },
    /// Der Mensch hat einen Befehl des Moduls getippt; `zeile` ist die ganze
    /// Zeile samt Befehl.
    Befehl { zeile: String },
    /// Der erbetene Takt ([`VomModul::WeckenIn`]); nur, solange der Modus
    /// des Moduls aktiv ist.
    Takt,
    /// Ein Stueck einer Modellantwort, wenn mit `strom` gefragt.
    Modellstueck { id: u64, text: String },
    /// Die ganze Modellantwort, oder warum es keine gibt.
    Modellantwort { id: u64, antwort: Result<String, String> },
    /// Was der Mensch geantwortet hat; `None`, wenn er abgebrochen hat.
    Nutzerantwort { id: u64, antwort: Option<String> },
    /// Ein Agentenlauf ist zu Ende.
    Agentenergebnis { id: u64, antwort: Result<String, String> },
    /// Ein anderes Modell ist geladen.
    Modell { kennung: Option<String> },
    /// Das Fenster hat jetzt diese Masse.
    Masse { breite: u16, hoehe: u16 },
    /// Die Konsole beendet das Modul; danach kommt nichts mehr.
    Ende,
}

/// **Vom Modul an die Konsole.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum VomModul {
    /// Die Antwort auf den Handschlag.
    Hallo { protokoll: u32, name: String, version: String },
    /// Text in den Rollbereich.
    Zeilen { text: String, stil: Stil },
    /// Der Modus des Moduls beginnt oder endet. Aktiv: Takte kommen, die
    /// Fusszeile und der Bereich unten gehoeren dem Modul, und der Agent
    /// arbeitet mit der Werkzeugkiste und den Skills des Moduls.
    Modus { aktiv: bool, banner: Option<Vec<String>> },
    /// Die Fusszeile unter der Eingabe, solange der Modus aktiv ist.
    Fusszeile { teile: Vec<Teil> },
    /// Der Bereich unter dem Rahmen; hoechstens so viele Zeilen, wie die
    /// Befugnis `unten` nennt.
    Unten { zeilen: Vec<Vec<Teil>> },
    /// Bitte um den naechsten Takt in so vielen Millisekunden.
    WeckenIn { ms: u64 },
    /// Eine Frage an das geladene Modell, ohne Werkzeuge und ohne Verlauf.
    ModellFragen {
        id: u64,
        frage: String,
        /// Hoechstens so viele Token Antwort.
        grenze: u32,
        /// Ob das Modell vorher ueberlegt.
        denken: bool,
        /// Ob Stuecke der Antwort kommen sollen, waehrend sie entsteht.
        strom: bool,
    },
    /// Ein Agentenlauf mit der Werkzeugkiste des Moduls.
    Agentenlauf { id: u64, auftrag: String },
    /// Eine Frage an den Menschen; die Konsole zeichnet sie in ihrem eigenen
    /// Rahmen.
    NutzerFragen { id: u64, frage: Nutzerfrage },
    /// Eine Zeile fuers Protokoll der Konsole (nicht auf den Schirm).
    Protokoll { text: String },
}

/// **Wie die Konsole einen Text faerbt.** Benannt und nicht als Farbe: Die
/// Konsole setzt die Farben ihres Designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stil {
    #[default]
    Normal,
    /// Zurueckgenommen, fuer Beiwerk und eingeklappte Zeilen.
    Beiwerk,
    Hervor,
    Titel,
    Gewinn,
    Verlust,
    Warnung,
}

/// Ein Stueck Text mit seinem Stil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Teil {
    pub text: String,
    #[serde(default)]
    pub stil: Stil,
}

/// **Was das Modul den Menschen fragt.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "snake_case")]
pub enum Nutzerfrage {
    /// Ein Wort, das der Mensch genau so tippen muss (etwa zur Freigabe
    /// einer Zahlung). Die Antwort ist, was getippt wurde.
    Wort { text: String, wort: String },
    /// Ja oder nein; die Antwort ist `ja` oder `nein`.
    JaNein { text: String },
    /// Eine Liste mit Pfeiltasten; die Antwort ist die Nummer des Punkts,
    /// ab 0.
    Auswahl { text: String, punkte: Vec<Auswahlpunkt> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auswahlpunkt {
    pub titel: String,
    #[serde(default)]
    pub hinweis: String,
}

/// **Eine Nachricht als Zeile**, ohne Zeilenumbruch darin (JSON maskiert
/// jeden Umbruch in Zeichenketten).
pub fn zeile<T: Serialize>(n: &T) -> String {
    serde_json::to_string(n).unwrap_or_default()
}

/// **Liest eine Zeile als Nachricht**; zu lang oder unlesbar ist ein Fehler.
pub fn lesen<T: for<'a> Deserialize<'a>>(zeile: &str) -> Result<T, String> {
    if zeile.len() > ZEILE_HOECHSTENS {
        return Err(format!("Nachricht zu lang ({} Bytes, hoechstens {ZEILE_HOECHSTENS})", zeile.len()));
    }
    serde_json::from_str(zeile.trim_end()).map_err(|f| format!("Nachricht unlesbar: {f}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hin_und_zurueck() {
        let alle = vec![
            VomModul::Hallo { protokoll: crate::PROTOKOLL, name: "x".into(), version: "0.1.0".into() },
            VomModul::Zeilen { text: "a\nb".into(), stil: Stil::Gewinn },
            VomModul::Fusszeile { teile: vec![Teil { text: "Heute ".into(), stil: Stil::Normal }, Teil { text: "+0,12 %".into(), stil: Stil::Gewinn }] },
            VomModul::NutzerFragen { id: 3, frage: Nutzerfrage::Wort { text: "Freigeben?".into(), wort: "WORT".into() } },
            VomModul::ModellFragen { id: 4, frage: "?".into(), grenze: 64, denken: false, strom: true },
        ];
        for n in alle {
            let z = zeile(&n);
            assert!(!z.contains('\n'), "eine Zeile: {z}");
            assert_eq!(lesen::<VomModul>(&z).unwrap(), n);
        }
        let a = AnModul::Modellantwort { id: 4, antwort: Err("kein Modell".into()) };
        assert_eq!(lesen::<AnModul>(&zeile(&a)).unwrap(), a);
    }

    #[test]
    fn zu_lang_und_unbekannt() {
        assert!(lesen::<VomModul>(&"x".repeat(ZEILE_HOECHSTENS + 1)).is_err());
        assert!(lesen::<VomModul>(r#"{"art":"tastendruck","taste":"j"}"#).is_err(), "eine unbekannte Art ist ein Fehler, keine leere Nachricht");
    }
}
