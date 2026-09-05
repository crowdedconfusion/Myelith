//! Die Risikoklassen, und dass sie beim Nutzer ankommen
//! (AGENT_LAYER 5.5, Whitepaper Kap. 9.3).
//!
//! # ⚑ Eine Warnung, die niemand liest, ist keine
//!
//! `ETHICS/Risikoklassen.toml` sagt in ihrem eigenen Kopf: „CLIENT und
//! AGENT_LAYER binden diese Datei ein, statt den Text abzuschreiben."
//! **Bis zum 2026-09-05 tat das niemand.** Die Datei war eine Quelle mit
//! null Lesern, und die Warnung erreichte damit genau niemanden.
//!
//! Das ist bitter, weil die Begründung im selben Kopf steht: „Eine
//! Warnung, die an drei Stellen steht, steht irgendwann in drei
//! Fassungen da, und die mildeste wird die gelesene." Der Entwurf war
//! richtig; es fehlte der Aufrufer.
//!
//! # ⚑ Eingebettet und nicht gelesen
//!
//! Die Datei kommt über `include_str!` in das Programm. Damit kann sie
//! zur Laufzeit **weder fehlen noch bearbeitet werden**, und für eine
//! Sicherheitswarnung ist genau das die richtige Eigenschaft: Wer sie
//! milder haben will, muss das Programm neu übersetzen, und dann steht
//! es im Bau.
//!
//! # ⚑ Klasse C wird abgelehnt, nicht gewarnt
//!
//! Die Datei sagt „ungeeignet", nicht „mit Vorsicht", und begründet das:
//! „Wer Gesundheitsdaten durch ein Netz fremder Rechner schickt, soll
//! das nicht für eine Abwägungsfrage halten." **Ein Harness, das dazu
//! eine Warnung ausgibt und dann doch rechnet, macht daraus wieder
//! eine.**
//!
//! ⚑ **Und was die Ablehnung nicht kann, gehört gesagt:** Die Klasse
//! erklärt der Nutzer selbst, und niemand kann sie prüfen. Wer Klasse A
//! angibt, bekommt Klasse A. Der Unterschied zu einer Warnung ist
//! trotzdem echt: **Man muss sich aktiv anders erklären, statt eine
//! Zeile wegzuscrollen.**

use serde::Deserialize;

/// Der Text der Quelle, zur Übersetzungszeit eingebettet.
///
/// ⚑ **Der Pfad geht durch zwei Ebenen nach oben.** Das ist hässlich und
/// richtig: Eine Kopie im eigenen Verzeichnis wäre die zweite Fassung,
/// vor der die Datei selbst warnt.
pub const QUELLE: &str = include_str!("../../../ETHICS/Risikoklassen.toml");

/// Was die Datei enthält.
#[derive(Debug, Clone, Deserialize)]
pub struct Risikoklassen {
    /// Fassung der Datei.
    pub fassung: String,
    /// Woher die Einteilung stammt.
    pub quelle: String,
    /// Die Klassen, in der Reihenfolge der Datei.
    #[serde(rename = "klasse")]
    pub klassen: Vec<Klasse>,
}

/// Eine Risikoklasse.
#[derive(Debug, Clone, Deserialize)]
pub struct Klasse {
    /// `A`, `B` oder `C`.
    pub kennung: String,
    /// Der Name.
    pub name: String,
    /// Wofür sie steht.
    pub beispiele: Vec<String>,
    /// `geeignet`, `bedingt geeignet` oder `ungeeignet`.
    pub eignung: String,
    /// Warum.
    pub begruendung: String,
}

impl Klasse {
    /// Ob das Netz dafür ungeeignet ist.
    pub fn ungeeignet(&self) -> bool {
        self.eignung == "ungeeignet"
    }
}

/// Warum eine Sitzung nicht laufen darf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verweigert {
    /// Die Klasse ist als ungeeignet eingestuft.
    Ungeeignet {
        /// Welche Klasse.
        kennung: String,
        /// Ihr Name.
        name: String,
        /// Die Begründung **aus der Quelle**, nicht neu formuliert.
        begruendung: String,
    },
    /// Die angegebene Kennung steht nicht in der Quelle.
    ///
    /// ⚑ **Kein stilles Durchwinken.** Eine unbekannte Klasse ist keine
    /// milde Klasse; sie ist gar keine, und dieselbe Überlegung wie bei
    /// `Segmentstufe::Unbekannt` gilt hier: In etwas, das niemand kennt,
    /// lässt sich nicht einwilligen.
    UnbekannteKlasse {
        /// Was angegeben wurde.
        kennung: String,
        /// Was es gibt.
        bekannt: Vec<String>,
    },
}

impl std::fmt::Display for Verweigert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ungeeignet { kennung, name, begruendung } => write!(
                f,
                "Klasse {kennung} ({name}) ist fuer dieses Netz UNGEEIGNET, nicht \
                 „mit Vorsicht\": {begruendung}"
            ),
            Self::UnbekannteKlasse { kennung, bekannt } => write!(
                f,
                "Risikoklasse `{kennung}` gibt es nicht; bekannt sind {bekannt:?}. \
                 Eine unbekannte Klasse ist keine milde Klasse, sondern gar keine"
            ),
        }
    }
}

impl std::error::Error for Verweigert {}

impl Risikoklassen {
    /// Liest die eingebettete Quelle.
    ///
    /// ⚑ **Panik statt Fehlerwert.** Die Datei liegt zur
    /// Übersetzungszeit fest; ist sie unlesbar, ist der Bau kaputt und
    /// nicht der Lauf. Ein `Result` hier lüde dazu ein, den Fall
    /// wegzuschlucken und ohne Warnung weiterzumachen.
    pub fn eingebettet() -> Self {
        toml::from_str(QUELLE).expect("ETHICS/Risikoklassen.toml ist unlesbar")
    }

    /// Die Klasse zu einer Kennung.
    pub fn klasse(&self, kennung: &str) -> Option<&Klasse> {
        self.klassen.iter().find(|k| k.kennung == kennung)
    }

    /// Darf eine Sitzung dieser Klasse laufen?
    ///
    /// ⚑ **Ablehnung und keine Warnung**, siehe Modulkopf. Und die
    /// Begründung kommt **aus der Quelle**: Wer sie hier neu formulierte,
    /// hätte die zweite Fassung geschaffen, vor der die Datei warnt.
    pub fn pruefen(&self, kennung: &str) -> Result<&Klasse, Verweigert> {
        let Some(k) = self.klasse(kennung) else {
            return Err(Verweigert::UnbekannteKlasse {
                kennung: kennung.to_string(),
                bekannt: self.klassen.iter().map(|k| k.kennung.clone()).collect(),
            });
        };
        if k.ungeeignet() {
            return Err(Verweigert::Ungeeignet {
                kennung: k.kennung.clone(),
                name: k.name.clone(),
                begruendung: k.begruendung.trim().to_string(),
            });
        }
        Ok(k)
    }

    /// Der Text, der einem Menschen vor der Sitzung gezeigt gehört.
    ///
    /// ⚑ **Aus der Quelle zusammengesetzt und nicht nacherzählt.**
    pub fn hinweis(&self) -> String {
        let mut t = format!(
            "Risikoklassen ({}, Fassung {}):\n",
            self.quelle, self.fassung
        );
        for k in &self.klassen {
            t.push_str(&format!(
                "  {} {:<28} {:<18} z. B. {}\n",
                k.kennung,
                k.name,
                k.eignung,
                k.beispiele.join(", ")
            ));
        }
        t
    }
}
