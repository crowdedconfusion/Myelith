//! **Die Kennzeichnung als KI**: der Hinweis beim Start und die Marke an
//! jeder Antwort.
//!
//! # ⚑ Warum es das gibt
//!
//! Artikel 50 Absatz 1 der Verordnung (EU) 2024/1689 verlangt, dass ein
//! Mensch weiss, dass er mit einem KI-System spricht, und zwar spaetestens
//! bei der ersten Interaktion. Myelith sagt es **bei jedem Start** und
//! laesst sich das bestaetigen; das ist mehr, als die Vorschrift
//! verlangt, und so festgelegt vom Projektinhaber (2026-09-25).
//!
//! # ⚑ Warum der Text hier steht und nicht in der Oberflaeche
//!
//! Aus demselben Grund wie [`crate::warnung`]: **Fenster und Konsole
//! muessen dasselbe sagen.** Ein Hinweis, der zweimal getippt ist, laeuft
//! auseinander, sobald einer ihn verbessert.
//!
//! ⚑ **Mit einer Ausnahme, und sie steht hier und nicht dort: der
//! Notschalter.** In der Konsole ist es der Notaus (Strg-C, Esc); im
//! Fenster ist es seit CLIENT v0.95.0 das Schliessen des Fensters, der
//! Knopf ist entfallen (Festlegung des Projektinhabers, 2026-09-28).
//! Deshalb nimmt [`starthinweis`] die [`Flaeche`] mit, und nur dieser
//! eine Satz haengt an ihr.
//!
//! # ⛔️ Kein Schalter
//!
//! Anders als die Warnung vor dem Agentenbetrieb (`agent.warnung`) laesst
//! sich dieser Hinweis **nicht abbestellen**. Es gibt kein Feld dafuer,
//! und es darf keines geben: Eine Kennzeichnung, die man abschalten kann,
//! ist eine Einstellung und keine Kennzeichnung.

use crate::einstellungen::Sprache;

/// Wo die Zweckbestimmung oeffentlich steht, je Sprache.
pub fn zweckbestimmung(sprache: Sprache) -> String {
    let datei = match sprache {
        Sprache::De => "COMPLIANCE/de/Zweckbestimmung.md",
        Sprache::En => "COMPLIANCE/en/Intended-Use-Policy.md",
    };
    format!("{}/blob/main/{datei}", crate::aktualisierung::SEITE)
}

/// Wo der Hinweis erscheint; daran haengt allein der Satz zum
/// Notschalter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flaeche {
    /// Das Gespraechsfenster: Der Notschalter ist das Schliessen.
    Fenster,
    /// Die Konsole: Der Notschalter ist der Notaus.
    Konsole,
}

/// Der Hinweis beim Start, fertig zum Anzeigen.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Starthinweis {
    /// Der eine Satz, der ueber allem steht.
    pub titel: &'static str,
    /// Was Myelith kann und was nicht, je ein Satz.
    pub punkte: Vec<&'static str>,
    /// Wo die Zweckbestimmung steht, als Adresse.
    pub verweis: String,
    /// Die Beschriftung des Verweises.
    pub verweis_titel: &'static str,
    /// Was der Mensch bestaetigt, bevor es weitergeht.
    pub bestaetigung: &'static str,
    /// Die Beschriftung des Knopfes, der erst nach der Bestaetigung geht.
    pub weiter: &'static str,
    /// Das Wort fuer die dauerhafte Marke im Kopf des Fensters.
    pub kurz: &'static str,
    /// Die Marke an jeder Antwort, siehe [`marke`].
    pub marke: &'static str,
}

/// **Der Hinweis beim Start**, in der eingestellten Sprache und fuer die
/// Flaeche, auf der er erscheint.
pub fn starthinweis(sprache: Sprache, flaeche: Flaeche) -> Starthinweis {
    let verweis = zweckbestimmung(sprache);
    let notschalter = match (sprache, flaeche) {
        (Sprache::De, Flaeche::Fenster) => {
            "Handlungen mit Wirkung nach außen legt es dir vorher zur Bestätigung vor. Der Notschalter ist das Schließen des Fensters: Es beendet Myelith sofort, samt jeder laufenden Handlung."
        }
        (Sprache::De, Flaeche::Konsole) => {
            "Handlungen mit Wirkung nach außen legt es dir vorher zur Bestätigung vor, und der Notaus bricht jede laufende Handlung ab."
        }
        (Sprache::En, Flaeche::Fenster) => {
            "It asks you to confirm actions with effects outside before running them. The emergency stop is closing the window: it ends Myelith at once, along with any running action."
        }
        (Sprache::En, Flaeche::Konsole) => {
            "It asks you to confirm actions with effects outside before running them, and the emergency stop aborts any running action."
        }
    };
    match sprache {
        Sprache::De => Starthinweis {
            titel: "Du arbeitest mit einer künstlichen Intelligenz.",
            punkte: vec![
                "Myelith ist ein KI-System. Antworten, Handlungen und die vorgelesene Stimme erzeugt ein Sprachmodell, kein Mensch.",
                "Es beantwortet Fragen, arbeitet an Dateien im freigegebenen Ordner, sucht im Web, hört, sieht und spricht, soweit du das einschaltest.",
                "Es kann sich irren: Aussagen können falsch oder erfunden sein. Prüfe Wichtiges nach.",
                notschalter,
                "Nicht erlaubt: Menschen bewerten oder einstufen, Biometrie, Emotionserkennung am Arbeitsplatz oder in der Bildung, Entscheidungen über Arbeit, Bildung, Kredit, Sozialleistungen, Strafverfolgung, Migration oder Wahlen.",
                "Eine nachgebildete Stimme nur mit Einwilligung der Person, und nie als echte Aufnahme ausgeben. Erzeugte Sprache ist als KI-erzeugt gekennzeichnet.",
            ],
            verweis,
            verweis_titel: "Zweckbestimmung",
            bestaetigung: "Ich habe verstanden, dass ich mit einer KI arbeite, und halte mich an die Zweckbestimmung.",
            weiter: "Weiter",
            kurz: "KI",
            marke: marke(sprache),
        },
        Sprache::En => Starthinweis {
            titel: "You are working with an artificial intelligence.",
            punkte: vec![
                "Myelith is an AI system. Answers, actions and the voice that reads aloud are produced by a language model, not by a human.",
                "It answers questions, works on files in the released folder, searches the web, hears, sees and speaks, as far as you turn these on.",
                "It can be wrong: statements may be false or made up. Check anything important.",
                notschalter,
                "Not allowed: evaluating or classifying people, biometrics, emotion recognition at work or in education, decisions on employment, education, credit, public assistance, law enforcement, migration or elections.",
                "A cloned voice only with the person's consent, and never passed off as a genuine recording. Generated speech is marked as AI-generated.",
            ],
            verweis,
            verweis_titel: "Intended use policy",
            bestaetigung: "I understand that I am working with an AI and I will follow the intended use policy.",
            weiter: "Continue",
            kurz: "AI",
            marke: marke(sprache),
        },
    }
}

/// **Der Hinweis als eine Zeile**, fuer `myl` auf der Fehlerausgabe.
///
/// ⚑ **Eine Zeile und keine Rueckfrage:** `myl` laeuft in Skripten, und
/// dort gibt es niemanden, der bestaetigen koennte. Die Fehlerausgabe,
/// damit die Antwort auf der Standardausgabe unveraendert weiterverarbeitet
/// werden kann.
pub fn zeile(sprache: Sprache) -> String {
    match sprache {
        Sprache::De => format!(
            "Hinweis: Die Antworten erzeugt ein KI-System (Myelith), kein Mensch. Zweckbestimmung: {}",
            zweckbestimmung(sprache)
        ),
        Sprache::En => format!(
            "Notice: answers are produced by an AI system (Myelith), not a human. Intended use policy: {}",
            zweckbestimmung(sprache)
        ),
    }
}

/// **Die Marke an jeder Antwort des Modells.**
///
/// ⚑ Kurz, weil sie an jeder Antwort steht, und in der Sprache des
/// Fensters. Die dauerhafte Anzeige im Kopf traegt dasselbe Wort.
pub fn marke(sprache: Sprache) -> &'static str {
    match sprache {
        Sprache::De => "KI-generiert",
        Sprache::En => "AI-generated",
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Beide Sprachen sagen dasselbe**, jedenfalls gleich viel: Ein
    /// Punkt, der in einer Sprache fehlt, ist ein Hinweis, den dort
    /// niemand bekommt.
    #[test]
    fn beide_sprachen_sagen_gleich_viel() {
        for f in [Flaeche::Fenster, Flaeche::Konsole] {
            let de = starthinweis(Sprache::De, f);
            let en = starthinweis(Sprache::En, f);
            assert_eq!(de.punkte.len(), en.punkte.len());
            for h in [&de, &en] {
                assert!(!h.titel.is_empty() && !h.bestaetigung.is_empty() && !h.weiter.is_empty());
                assert!(h.punkte.iter().all(|p| !p.is_empty()));
            }
            assert!(de.titel.contains("künstlichen Intelligenz"));
            assert!(en.titel.contains("artificial intelligence"));
        }
    }

    /// ⛔️ **Der Hinweis nennt den Notschalter, der auf dieser Flaeche
    /// wirklich einer ist**: im Fenster das Schliessen (einen Knopf gibt
    /// es dort nicht mehr), in der Konsole den Notaus. Und nur dieser
    /// Satz unterscheidet sich; alles andere sagen beide gleich.
    #[test]
    fn der_notschalter_ist_der_der_flaeche() {
        for s in [Sprache::De, Sprache::En] {
            let fenster = starthinweis(s, Flaeche::Fenster);
            let konsole = starthinweis(s, Flaeche::Konsole);
            let fenster_text = fenster.punkte.join(" ");
            let konsole_text = konsole.punkte.join(" ");
            let (schliessen, notaus) = match s {
                Sprache::De => ("Schließen des Fensters", "Notaus"),
                Sprache::En => ("closing the window", "emergency stop aborts"),
            };
            assert!(fenster_text.contains(schliessen), "{s:?}: das Fenster nennt das Schliessen nicht");
            assert!(!fenster_text.contains(notaus), "{s:?}: das Fenster nennt einen Notaus, den es nicht hat");
            assert!(konsole_text.contains(notaus), "{s:?}: die Konsole nennt ihren Notaus nicht");
            assert!(!konsole_text.contains(schliessen), "{s:?}: die Konsole nennt ein Fenster");
            let anders = fenster.punkte.iter().zip(&konsole.punkte).filter(|(a, b)| a != b).count();
            assert_eq!(anders, 1, "{s:?}: mehr als der eine Satz weicht ab");
        }
    }

    /// ⚑ **Der Verweis zeigt auf eine Datei, die es gibt**, in beiden
    /// Sprachen. Ein Hinweis, der auf eine tote Adresse verweist, ist ein
    /// Hinweis ohne Beleg.
    #[test]
    fn der_verweis_zeigt_auf_die_zweckbestimmung() {
        let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for s in [Sprache::De, Sprache::En] {
            let v = zweckbestimmung(s);
            let pfad = v.split("/blob/main/").nth(1).expect("Pfad im Verweis");
            assert!(wurzel.join(pfad).is_file(), "{pfad} gibt es nicht");
            assert!(v.starts_with(crate::aktualisierung::SEITE));
        }
    }
}
