//! **Abgeschaltete Werkzeuge: angesagt, mit Grund und Schalter.**
//!
//! Auftrag des Projektinhabers vom 2026-09-28: fuer jedes Werkzeug, das an
//! einem Schalter haengt und gerade aus ist.
//!
//! # 📌 Der Anlass
//!
//! Ein Auftrag „recherchiere online, wie man Avocados anbaut" endete beim
//! 30B mit „ich habe keinen Internetzugriff". Das stimmte: `agent.web_recherche`
//! stand auf aus, und ohne den Schalter gibt es die Web-Werkzeuge gar nicht.
//! **Falsch war nicht die Auskunft, sondern ihr Ende.** Der Nutzer erfuhr
//! weder, dass es die Faehigkeit gibt, noch, wie man sie einschaltet.
//!
//! # ⚑ Was ein Platzhalter ist
//!
//! Er traegt den **Namen** des echten Werkzeugs, denn nach genau dem greift
//! ein Modell, und eine Beschreibung, die mit „abgeschaltet" beginnt und
//! den Grund nennt. Ein Aufruf **tut nichts**: Er kommt als Werkzeugfehler
//! zurueck, mit dem Grund und dem Weg zum Einschalten, und mit der Bitte,
//! beides an den Nutzer weiterzugeben.
//!
//! ⛔️ **Kein Platzhalter hat einen Weg nach draussen.** Er liest keine
//! Datei, startet keinen Prozess und oeffnet keine Verbindung; das ist die
//! ganze Sicherheitsaussage, und die Probe `ein_platzhalter_tut_nichts`
//! haelt sie fest.
//!
//! ⚑ **Die Beschriftungen der Schalter kommen aus der Feldtabelle der
//! Einstellungen** und stehen hier nicht ein zweites Mal. Wer ein Feld
//! umbenennt, benennt damit auch den Hinweis um.

use crate::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use crate::werkzeug::{Ansageform, Werkzeug};

/// **Warum ein Werkzeug fehlt, und wie es kommt**, je ein Halbsatz in der
/// Sprache der Ansage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grund {
    pub warum: String,
    pub einschalten: String,
}

fn deutsch(form: Ansageform) -> bool {
    matches!(form, Ansageform::Deutsch)
}

/// Die Beschriftung eines Feldes, wie die Einstellungsseite sie zeigt.
fn titel(feld: &str, form: Ansageform) -> String {
    crate::agentenwahl::feldtitel(feld)
        .map(|(de, en)| if deutsch(form) { de } else { en })
        .unwrap_or(feld)
        .to_string()
}

/// Ein Schalter, der auf „an" gehoert.
fn schalter_an(feld: &str, form: Ansageform) -> String {
    if deutsch(form) {
        format!("in den Einstellungen „{}“ einschalten, oder `myl setzen {feld} an`", titel(feld, form))
    } else {
        format!("switch on “{}” in the settings, or run `myl setzen {feld} an`", titel(feld, form))
    }
}

impl Grund {
    /// Die Web-Recherche ist in den Einstellungen aus.
    pub fn web_aus(form: Ansageform) -> Self {
        Self {
            warum: if deutsch(form) {
                "die Web-Recherche ist in den Einstellungen ausgeschaltet".into()
            } else {
                "web research is switched off in the settings".into()
            },
            einschalten: schalter_an("agent.web_recherche", form),
        }
    }

    /// Die Web-Recherche ist an, aber `curl` fehlt.
    pub fn curl_fehlt(form: Ansageform) -> Self {
        if deutsch(form) {
            Self { warum: "auf diesem Rechner fehlt curl".into(), einschalten: "curl installieren".into() }
        } else {
            Self { warum: "curl is missing on this machine".into(), einschalten: "install curl".into() }
        }
    }

    /// Der Lauf darf nur lesen.
    pub fn schreiben_aus(form: Ansageform) -> Self {
        Self {
            warum: if deutsch(form) {
                "dieser Lauf darf nur lesen".into()
            } else {
                "this run may only read".into()
            },
            einschalten: schalter_an("agent.schreiben", form),
        }
    }

    /// Das Werkzeug liegt nicht in der gewaehlten Kiste.
    pub fn nicht_in_der_kiste(kiste: &str, form: Ansageform) -> Self {
        let feld = titel("agent.kistenordner", form);
        if deutsch(form) {
            Self {
                warum: format!("es liegt nicht in der Werkzeugkiste {kiste}"),
                einschalten: format!("in den Einstellungen „{feld}“ den Ordner Advanced wählen"),
            }
        } else {
            Self {
                warum: format!("it is not in the {kiste} toolbox"),
                einschalten: format!("choose the Advanced folder under “{feld}” in the settings"),
            }
        }
    }

    /// Ein Blick (Bildschirm oder Kamera) ist nicht erlaubt.
    pub fn blick_aus(feld: &str, form: Ansageform) -> Self {
        Self {
            warum: if deutsch(form) {
                "dieser Blick ist in den Einstellungen nicht erlaubt".into()
            } else {
                "this view is not allowed in the settings".into()
            },
            einschalten: schalter_an(feld, form),
        }
    }

    /// Es ist kein Arbeitsordner eingehaengt.
    pub fn kein_ordner(form: Ansageform) -> Self {
        let feld = titel("agent.wurzel", form);
        if deutsch(form) {
            Self {
                warum: "es ist kein Arbeitsordner eingehängt".into(),
                einschalten: format!("in den Einstellungen „{feld}“ einen Ordner setzen, oder `myl setzen agent.wurzel <pfad>`"),
            }
        } else {
            Self {
                warum: "no working folder is mounted".into(),
                einschalten: format!("set a folder under “{feld}” in the settings, or run `myl setzen agent.wurzel <path>`"),
            }
        }
    }
}

/// Die Ausfuehrung eines Platzhalters: immer ein Werkzeugfehler mit Grund
/// und Einschaltweg.
pub struct Abgeschaltet {
    name: String,
    meldung: String,
}

impl Werkzeugausfuehrung for Abgeschaltet {
    fn name(&self) -> &str {
        &self.name
    }
    fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        Err(Werkzeugfehler { grund: self.meldung.clone() })
    }
}

/// **Das Angebot eines Platzhalters** unter dem Namen des echten Werkzeugs.
///
/// ⚑ **Mit dem Schema des echten Werkzeugs, unveraendert.**
/// 📌 Die erste Fassung hatte gar keine Parameter, und der Werkzeugrundgang
/// zeigte, was dann geschieht: Ein Modell ruft `web_search` mit `frage`, die
/// Formpruefung lehnt das unbekannte Feld ab, und die Antwort lautet „das
/// Feld `frage` steht nicht im Schema" statt der Anleitung zum Einschalten.
/// Genau der Fall, fuer den es Platzhalter gibt, kam so nie bei ihnen an.
/// Ein Aufruf, wie er beim echten Werkzeug richtig waere, erreicht jetzt die
/// Ausfuehrung. ⚠️ **Die Pflichtfelder bleiben**, denn optionale Parameter
/// gibt es mit Absicht nicht (`kein_werkzeug_hat_einen_optionalen_parameter`):
/// Ein kleines Modell ueberlegte am 2026-09-10 seitenlang, ob es eines
/// weglassen darf.
pub fn angebot(
    name: &str,
    parameter: serde_json::Value,
    grund: &Grund,
    form: Ansageform,
) -> (Werkzeug, Box<dyn Werkzeugausfuehrung>) {
    let (beschreibung, meldung) = if deutsch(form) {
        (
            format!("ABGESCHALTET ({}); ein Aufruf nennt, wie man es einschaltet.", grund.warum),
            // 📌 **Der Satz fuer den Nutzer steht woertlich da** (2026-09-29).
            //    Mit „gib das weiter" sagte das 30B nur „in den Einstellungen
            //    ausgeschaltet", liess den Schalter weg und erfand dazu einen
            //    Link als Ersatzquelle.
            format!(
                "`{name}` ist in diesem Lauf abgeschaltet. Sag dem Nutzer woertlich: \
                 „Das geht gerade nicht, weil {}. Einschalten: {}.“ Erfinde keine Quellen, \
                 Links oder Ergebnisse als Ersatz.",
                grund.warum, grund.einschalten
            ),
        )
    } else {
        (
            format!("SWITCHED OFF ({}); calling it tells you how to switch it on.", grund.warum),
            format!(
                "`{name}` is switched off in this run. Tell the user exactly: \
                 \"This is not possible right now because {}. To switch it on: {}.\" \
                 Do not invent sources, links or results instead.",
                grund.warum, grund.einschalten
            ),
        )
    };
    (
        Werkzeug { name: name.to_string(), beschreibung, parameter },
        Box::new(Abgeschaltet { name: name.to_string(), meldung }),
    )
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⛔️ **Ein Platzhalter tut nichts** und sagt, wie es geht.
    #[test]
    fn ein_platzhalter_tut_nichts() {
        let schema = serde_json::json!({"type": "object", "properties": {"frage": {"type": "string"}}, "required": ["frage"]});
        let (w, a) = angebot("web_search", schema, &Grund::web_aus(Ansageform::Amtlich), Ansageform::Amtlich);
        assert_eq!(w.name, "web_search");
        assert!(w.beschreibung.starts_with("SWITCHED OFF"), "{}", w.beschreibung);
        assert_eq!(w.parameter["required"], serde_json::json!(["frage"]), "das Schema ist das echte");
        let f = a.ausfuehren(&serde_json::json!({"frage": "Avocados anbauen"})).unwrap_err().grund;
        assert!(f.contains("myl setzen agent.web_recherche an"), "{f}");
        assert!(f.contains("Tell the user exactly") && f.contains("Do not invent"), "{f}");
    }

    /// ⚑ **Die Beschriftung kommt aus den Agentenfeldern**, in beiden
    /// Sprachen, und ein unbekanntes Feld faellt auf seinen Namen zurueck
    /// statt auf nichts. Dass die Feldtabelle des Clients dieselben Titel
    /// zeigt, prueft der Client.
    #[test]
    fn die_beschriftung_kommt_aus_den_agentenfeldern() {
        let (de, en) = crate::agentenwahl::TITEL_WEB_RECHERCHE;
        assert!(Grund::web_aus(Ansageform::Deutsch).einschalten.contains(de));
        assert!(Grund::web_aus(Ansageform::Amtlich).einschalten.contains(en));
        assert_eq!(titel("gibt.es.nicht", Ansageform::Deutsch), "gibt.es.nicht");
        for name in crate::agentenwahl::BETITELTE_FELDER {
            assert!(crate::agentenwahl::feldtitel(name).is_some(), "{name} hat keinen Titel");
        }
    }
}
