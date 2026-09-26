//! Werkzeuge ausführen, und die Grenze dabei (AGENT_LAYER 5.2/5.4).
//!
//! # ⚑ Ein Steckplatz darf **tun**, nie **erlauben**
//!
//! Das ist derselbe Satz wie in [`crate::werkzeug`], eine Ebene höher.
//! Dort ist der Vorschlagende ein Modell, hier ein Stück Code.
//!
//! **Die Recherche zu DeepSeek Harness und OpenClaw hat gezeigt, warum
//! das der Angelpunkt ist** (2026-09-05): In einem Entwurf, in dem
//! *alles* ein Plugin ist, ist auch die Autorisierung eines, und dann
//! lässt sie sich gegen eine grosszügige tauschen. Bezeichnend ist, dass
//! die Architekturbeschreibung von Harness **nichts** zu Rechten oder
//! Freigaben enthält; das ist kein Versehen, sondern was passiert, wenn
//! Modularität das Ordnungsprinzip ist.
//!
//! ⚑ **Deshalb ist die Grenze im Merkmal festgeschrieben und nicht in
//! einem Kommentar:** [`Werkzeugausfuehrung`] bekommt die
//! [`crate::Erlaubnis`] gar nicht erst in die Hand, und sie gibt ein
//! **Ergebnis** zurück, nie eine Entscheidung. Ein Steckplatz, der
//! „erlaubt" sagen könnte, wäre ein Steckplatz, der sich selbst
//! erlauben kann.
//!
//! | fest verdrahtet | steckbar |
//! |---|---|
//! | Erlaubnis, Betriebsart, Risikoklassen | die Werkzeuge selbst |
//! | Sitzungsgrenzen, Strom, Kette | der Modelladapter |

use crate::werkzeug::Werkzeug;

/// Warum ein Werkzeug nichts geliefert hat.
///
/// ⚑ **Ein Fehler ist ein Ergebnis und kein Abbruch.** Er geht als
/// Werkzeugantwort zurück ins Gespräch, damit das Modell darauf
/// reagieren kann; ein Lauf, der beim ersten Fehlschlag endet, ist für
/// einen Agenten unbrauchbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Werkzeugfehler {
    /// Was schiefging, für Mensch und Modell.
    pub grund: String,
}

impl std::fmt::Display for Werkzeugfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.grund)
    }
}

impl std::error::Error for Werkzeugfehler {}

/// Etwas, das ein Werkzeug ausführen kann.
///
/// ⚑ **Was diesem Merkmal fehlt, ist die Aussage:** Es bekommt keine
/// Erlaubnis, keinen Sitzungskontrakt, keinen Strom und keine
/// Registratur. Es sieht seine Argumente und gibt Text zurück.
///
/// **Wer hier ein Argument mehr hinzufügt, sollte zweimal hinsehen.**
/// Jedes weitere ist eine Fähigkeit, die ein Steckplatz bekommt, und die
/// Frage ist nicht, ob sie nützlich wäre, sondern was ein bösartiger
/// Steckplatz damit anfinge.
/// ⚑ **`Send + Sync` ist keine Formalie, sondern eine Zusicherung.**
///
/// Ein Steckplatz, der nicht zwischen Fäden wandern darf, hält
/// **fadenlokalen Zustand**, und dann rechnet er je nach Faden anders.
/// Das ist genau die Sorte Nichtdeterminismus, die dieses Projekt
/// überall sonst ausschliesst; hier fällt sie beim Übersetzen auf.
///
/// Praktisch heisst es: kein `Rc`, kein `RefCell`, kein verstecktes
/// `thread_local`. Wer geteilten Zustand braucht, nimmt einen, der es
/// zugibt.
pub trait Werkzeugausfuehrung: Send + Sync {
    /// Unter welchem Namen es aufgerufen wird.
    ///
    /// ⚑ **Muss mit dem [`Werkzeug`] übereinstimmen, das angeboten
    /// wurde**, sonst prüft die Erlaubnis einen anderen Namen als den
    /// ausgeführten. [`Werkzeugkasten::einhaengen`] hält das fest.
    fn name(&self) -> &str;

    /// Führt aus und gibt zurück, was dabei herauskam.
    fn ausfuehren(&self, argumente: &serde_json::Value) -> Result<String, Werkzeugfehler>;
}

/// Was angeboten wird und was es ausführt, aneinandergebunden.
///
/// ⚑ **Zusammen und nicht getrennt.** Ein Angebot ohne Ausführung wäre
/// ein Werkzeug, das das Modell vorschlagen darf und das niemand rufen
/// kann; eine Ausführung ohne Angebot wäre eines, das läuft, ohne je
/// erlaubt worden zu sein. Beides ist ein Fehler, und beide fallen hier
/// beim Einhängen auf.
#[derive(Default)]
pub struct Werkzeugkasten {
    angebote: Vec<Werkzeug>,
    ausfuehrungen: Vec<Box<dyn Werkzeugausfuehrung>>,
}

/// Warum ein Werkzeug nicht eingehängt werden konnte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kastenfehler {
    /// Angebot und Ausführung tragen verschiedene Namen.
    NamenPassenNicht {
        /// Name im Angebot.
        angebot: String,
        /// Name der Ausführung.
        ausfuehrung: String,
    },
    /// Der Name ist schon vergeben.
    ///
    /// ⚑ **Kein Überschreiben.** Wer ein zweites Werkzeug unter
    /// demselben Namen einhinge, entschiede über die Reihenfolge, welches
    /// läuft, und die Erlaubnis prüfte einen Namen, der zwei Dinge
    /// meint.
    NameDoppelt {
        /// Welcher.
        name: String,
    },
}

impl std::fmt::Display for Kastenfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NamenPassenNicht { angebot, ausfuehrung } => write!(
                f,
                "das Angebot heisst `{angebot}`, die Ausfuehrung `{ausfuehrung}`: dann prueft \
                 die Erlaubnis einen anderen Namen als den ausgefuehrten"
            ),
            Self::NameDoppelt { name } => write!(
                f,
                "`{name}` ist schon eingehaengt: die Reihenfolge entschiede, welches laeuft"
            ),
        }
    }
}

impl std::error::Error for Kastenfehler {}

impl Werkzeugkasten {
    /// Leer.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Hängt ein Werkzeug samt seiner Ausführung ein.
    pub fn einhaengen(
        &mut self,
        angebot: Werkzeug,
        ausfuehrung: Box<dyn Werkzeugausfuehrung>,
    ) -> Result<(), Kastenfehler> {
        if angebot.name != ausfuehrung.name() {
            return Err(Kastenfehler::NamenPassenNicht {
                angebot: angebot.name,
                ausfuehrung: ausfuehrung.name().to_string(),
            });
        }
        if self.angebote.iter().any(|a| a.name == angebot.name) {
            return Err(Kastenfehler::NameDoppelt { name: angebot.name });
        }
        self.angebote.push(angebot);
        self.ausfuehrungen.push(ausfuehrung);
        Ok(())
    }

    /// Die Angebote, für [`crate::werkzeug::angebot`] und die Erlaubnis.
    pub fn angebote(&self) -> &[Werkzeug] {
        &self.angebote
    }

    /// **Legt um jede Ausfuehrung eine Huelle.**
    ///
    /// ⚑ **Fuer Schichten, die weniger tun, nie mehr erlauben**: etwa die
    /// Doppelsperre des Loops, die einen Aufruf, der vor einer
    /// Unterbrechung schon lief, nicht noch einmal ausfuehrt, sondern das
    /// gespeicherte Ergebnis liefert. Die Erlaubnis sitzt im Harness und
    /// bleibt davon unberuehrt; die Huelle sieht nur, was schon erlaubt
    /// ist.
    ///
    /// ⛔️ **Der Name bleibt.** Eine Huelle, die ihn aendert, liesse die
    /// Erlaubnis einen anderen Namen pruefen als den ausgefuehrten; das
    /// ist ein Programmierfehler und bricht deshalb hart ab.
    pub fn umhuellen(
        &mut self,
        mut huelle: impl FnMut(Box<dyn Werkzeugausfuehrung>) -> Box<dyn Werkzeugausfuehrung>,
    ) {
        let alt = std::mem::take(&mut self.ausfuehrungen);
        for (a, angebot) in alt.into_iter().zip(&self.angebote) {
            let neu = huelle(a);
            assert_eq!(neu.name(), angebot.name, "eine Huelle darf den Namen eines Werkzeugs nicht aendern");
            self.ausfuehrungen.push(neu);
        }
    }

    /// Das Angebot zu einem Namen.
    pub fn angebot(&self, name: &str) -> Option<&Werkzeug> {
        self.angebote.iter().find(|a| a.name == name)
    }

    /// Führt aus, **ohne zu fragen, ob es darf**.
    ///
    /// ⚑ **Diese Funktion prüft keine Erlaubnis, und das ist Absicht.**
    /// Sie zu prüfen ist Aufgabe des Aufrufers, und zwar **vor** dem
    /// Aufruf; hier noch einmal zu prüfen wäre eine zweite Meinung, und
    /// wer sich auf sie verlässt, hat die erste weggelassen.
    ///
    /// Der Name sagt es deshalb mit: `ausfuehren_ungeprueft`.
    pub fn ausfuehren_ungeprueft(
        &self,
        name: &str,
        argumente: &serde_json::Value,
    ) -> Option<Result<String, Werkzeugfehler>> {
        self.ausfuehrungen
            .iter()
            .find(|a| a.name() == name)
            .map(|a| a.ausfuehren(argumente))
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    struct Echo(&'static str);
    impl Werkzeugausfuehrung for Echo {
        fn name(&self) -> &str {
            self.0
        }
        fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
            Ok(format!("{} {a}", self.0))
        }
    }

    struct Laut(Box<dyn Werkzeugausfuehrung>);
    impl Werkzeugausfuehrung for Laut {
        fn name(&self) -> &str {
            self.0.name()
        }
        fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
            self.0.ausfuehren(a).map(|t| t.to_uppercase())
        }
    }

    struct Umbenannt(Box<dyn Werkzeugausfuehrung>);
    impl Werkzeugausfuehrung for Umbenannt {
        fn name(&self) -> &str {
            "anders"
        }
        fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
            self.0.ausfuehren(a)
        }
    }

    fn kasten() -> Werkzeugkasten {
        let mut k = Werkzeugkasten::neu();
        k.einhaengen(Werkzeug::ohne_parameter("eins", "e"), Box::new(Echo("eins"))).unwrap();
        k.einhaengen(Werkzeug::ohne_parameter("zwei", "z"), Box::new(Echo("zwei"))).unwrap();
        k
    }

    #[test]
    fn huelle_wirkt_auf_jedes_werkzeug_und_behaelt_die_reihenfolge() {
        let mut k = kasten();
        k.umhuellen(|a| Box::new(Laut(a)));
        let namen: Vec<&str> = k.angebote().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(namen, ["eins", "zwei"]);
        assert_eq!(k.ausfuehrungen[1].ausfuehren(&serde_json::json!({})).unwrap(), "ZWEI {}");
    }

    #[test]
    #[should_panic(expected = "Namen eines Werkzeugs nicht aendern")]
    fn huelle_darf_den_namen_nicht_aendern() {
        kasten().umhuellen(|a| Box::new(Umbenannt(a)));
    }
}
