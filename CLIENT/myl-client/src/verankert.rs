//! Die verankerte Werkzeugkiste: Werkzeuge, die ein Dritter nachrechnen
//! kann.
//!
//! # ⚑ Wozu, und warum sie anders gebaut ist als die lokale
//!
//! Bis hierher meldete `ruestung.rs` **jedes** Werkzeug als
//! [`Werkzeugart::Extern`] und [`Herkunft::Lokal`] an. In der
//! Vorgabebetriebsart [`Betriebsart::NurVerankert`] sperrt der Harness
//! damit **alle**: Ein Netzlauf haette ein nachrechenbares Modell und
//! keine Werkzeuge.
//!
//! Diese Kiste fuellt das leere Feld. Sie traegt
//! [`Werkzeugart::Deterministisch`] und [`Herkunft::Verankert`], und die
//! Doku dieser Art sagt woertlich, was das verlangt: **„Rechnet aus
//! seinen Eingaben, sonst nichts."**
//!
//! # ⚑ Die eine Entwurfsentscheidung: erzeugen, nicht bewirken
//!
//! Ein verankertes Werkzeug **erzeugt Inhalt und bewirkt nichts**.
//! Nachrechenbar ist, wie aus Eingaben ein Text wird; eine Datei auf
//! eine Platte zu schreiben ist es nicht, denn wo sie landet, haengt an
//! einer Maschine, die niemand sonst hat. Fuer den Auftrag „schreibe mir
//! ein Dokument" heisst das: Das Modell **baut** den Text hier, das
//! **Speichern** bleibt ein lokaler Akt mit einem lokalen Werkzeug.
//!
//! Dieselbe Linie wie bei der Markdown-Wandlung und beim Vorbau fuer
//! Bild und Ton: Was sich nicht nachrechnen laesst, bleibt draussen, und
//! verankert wird sein Ergebnis.
//!
//! ⛔️ **Deshalb nimmt hier kein Werkzeug einen Pfad entgegen**, sondern
//! Inhalt. Ein Pfad zeigt auf einen Zustand, den nur eine Maschine hat.
//!
//! # ⛔️ Der Vertrag, und warum er teurer ist als der Code
//!
//! Determinismus ueber fremde Maschinen und spaetere Jahre ist strenger
//! als „ruft keine Uhr". Diese Kiste haelt sich deshalb an feste Regeln,
//! und `VERTRAG` schreibt sie fest:
//!
//! - **Keine Gross- und Kleinschreibung, keine Normalisierung.** Beides
//!   haengt an der Unicode-Fassung der Bibliothek und aendert sich mit
//!   ihr.
//! - **Keine Sortierung**, denn nach Gebietsschema sortiert jede
//!   Maschine anders.
//! - **Zeilenende ist `\n`**, immer, auch wenn die Eingabe `\r\n`
//!   bringt.
//! - **Kein Gleitkomma**, auch nicht in der Ausgabe.
//! - **Gezaehlt wird in Unicode-Skalarwerten**, nicht in Bytes und nicht
//!   in Darstellungsbreite.
//!
//! ⚑ **Eine Fassungsnummer fuer die ganze Kiste**, nicht je Werkzeug:
//! Die Kiste ist klein, und viele kleine Vertraege waeren viele Stellen,
//! an denen eine Aenderung vergessen wird. Sie steht in der Revision
//! jedes Manifests und geht damit in die Adresse ein.
//!
//! ⚠️ **Ohne Vektoren keine Marke.** `vektoren.json` haelt Eingabe und
//! erwartete Ausgabe jedes Werkzeugs fest; `die_vektoren_stimmen` faehrt
//! sie. Ein Werkzeug ohne Vektor ist eine Behauptung.

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use myl_local_agent::werkzeug::Werkzeug;

/// **Die Fassung des Vertrags dieser Kiste.**
///
/// ⚑ Sie steht in der Revision jedes Manifests und geht damit in die
/// Adresse ein: Wer die Regeln aendert, aendert die Adressen, und alte
/// Segmente bleiben an die alten gebunden.
pub const VERTRAG: &str = "0.1.0";

/// Ein Werkzeug dieser Kiste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verankert {
    /// Vorlage plus benannte Felder ergibt Text.
    VorlageFuellen,
    /// Abschnitte mit Ueberschriften ergeben ein Dokument.
    AbschnitteFuegen,
}

impl Verankert {
    pub const ALLE: [Verankert; 2] = [Self::VorlageFuellen, Self::AbschnitteFuegen];

    pub const fn name(self) -> &'static str {
        match self {
            Self::VorlageFuellen => "fill_template",
            Self::AbschnitteFuegen => "join_sections",
        }
    }

    pub const fn beschreibung(self) -> &'static str {
        match self {
            Self::VorlageFuellen => {
                "Fill a template with named fields. Every {name} is replaced by the \
                 field of that name; {{ and }} stand for literal braces. An unknown \
                 field is an error, not an empty string."
            }
            Self::AbschnitteFuegen => {
                "Join sections into one Markdown document. Each section becomes a \
                 heading of the given level followed by its text, separated by one \
                 blank line."
            }
        }
    }

    pub fn parameter(self) -> serde_json::Value {
        match self {
            Self::VorlageFuellen => serde_json::json!({
                "type": "object",
                "properties": {
                    "vorlage": {"type": "string", "description": "the template text"},
                    "felder": {
                        "type": "object",
                        "description": "field name to value, all values are text",
                    },
                },
                "required": ["vorlage", "felder"],
            }),
            Self::AbschnitteFuegen => serde_json::json!({
                "type": "object",
                "properties": {
                    "ebene": {
                        "type": "integer",
                        "description": "heading level, 1 to 6",
                    },
                    "abschnitte": {
                        "type": "array",
                        "description": "sections, each with titel and inhalt",
                        "items": {
                            "type": "object",
                            "properties": {
                                "titel": {"type": "string"},
                                "inhalt": {"type": "string"},
                            },
                            "required": ["titel", "inhalt"],
                        },
                    },
                },
                "required": ["ebene", "abschnitte"],
            }),
        }
    }

    /// **Rechnet aus seinen Eingaben, sonst nichts.**
    pub fn ausfuehren(self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        match self {
            Self::VorlageFuellen => vorlage_fuellen(a),
            Self::AbschnitteFuegen => abschnitte_fuegen(a),
        }
    }
}

fn fehler(grund: impl Into<String>) -> Werkzeugfehler {
    Werkzeugfehler { grund: grund.into() }
}

/// ⚑ **Zeilenenden werden vereinheitlicht, und zwar an der Grenze.**
/// Kaeme `\r\n` durch, haetten zwei Nachrechner mit verschieden
/// abgelegten Eingaben verschiedene Ausgaben, ohne dass jemand etwas
/// falsch gemacht haette.
fn zeilenenden(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// **Setzt `{feld}` ein; `{{` und `}}` bleiben als Klammer stehen.**
///
/// 📌 **Bewusst nicht `kisten::befehl_aus_vorlage`.** Die sieht gleich
/// aus und zitiert shell-sicher, weil ihr Ergebnis eine Kommandozeile
/// ist. Hier ist das Ergebnis Text; ein Anfuehrungszeichen darin waere
/// ein Fehler und keine Sicherheit. **Zwei Aufgaben, zwei Funktionen.**
fn vorlage_fuellen(a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
    let vorlage = a
        .get("vorlage")
        .and_then(|v| v.as_str())
        .ok_or_else(|| fehler("das Feld `vorlage` fehlt oder ist kein Text"))?;
    let felder = a
        .get("felder")
        .and_then(|v| v.as_object())
        .ok_or_else(|| fehler("das Feld `felder` fehlt oder ist kein Objekt"))?;

    let vorlage = zeilenenden(vorlage);
    let mut aus = String::with_capacity(vorlage.len());
    let mut rest = vorlage.as_str();
    while let Some(i) = rest.find(['{', '}']) {
        aus.push_str(&rest[..i]);
        let ab = &rest[i..];
        if let Some(r) = ab.strip_prefix("{{") {
            aus.push('{');
            rest = r;
            continue;
        }
        if let Some(r) = ab.strip_prefix("}}") {
            aus.push('}');
            rest = r;
            continue;
        }
        if ab.starts_with('}') {
            return Err(fehler(
                "eine schliessende Klammer ohne oeffnende; fuer eine Klammer im Text `}}` schreiben",
            ));
        }
        let ende = ab
            .find('}')
            .ok_or_else(|| fehler("eine oeffnende Klammer ohne schliessende"))?;
        let name = &ab[1..ende];
        // ⛔️ **Ein unbekanntes Feld ist ein Fehler, keine Luecke.** Eine
        // stillschweigend leere Stelle faellt im Text nicht auf, und ein
        // Dokument mit einer Leerstelle ist schlimmer als keines.
        let wert = felder
            .get(name)
            .ok_or_else(|| fehler(format!("das Feld `{name}` fehlt")))?;
        let wert = wert
            .as_str()
            .ok_or_else(|| fehler(format!("das Feld `{name}` ist kein Text")))?;
        aus.push_str(&zeilenenden(wert));
        rest = &ab[ende + 1..];
    }
    aus.push_str(rest);
    Ok(aus)
}

/// **Abschnitte ergeben ein Dokument.**
///
/// ⚑ Die Form ist festgeschrieben: je Abschnitt eine Ueberschrift aus
/// `ebene` Rauten, eine Leerzeile, der Inhalt; zwischen zwei Abschnitten
/// genau eine Leerzeile, am Ende genau ein Zeilenumbruch. Ohne diese
/// Festlegung haette jede Umsetzung ihre eigene Zahl von Leerzeilen, und
/// der Abdruck stimmte nie.
fn abschnitte_fuegen(a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
    let ebene = a
        .get("ebene")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| fehler("das Feld `ebene` fehlt oder ist keine Zahl"))?;
    if !(1..=6).contains(&ebene) {
        return Err(fehler("`ebene` liegt ausserhalb von 1 bis 6"));
    }
    let abschnitte = a
        .get("abschnitte")
        .and_then(|v| v.as_array())
        .ok_or_else(|| fehler("das Feld `abschnitte` fehlt oder ist keine Liste"))?;
    if abschnitte.is_empty() {
        return Err(fehler("keine Abschnitte; ein leeres Dokument ist kein Ergebnis"));
    }

    let rauten = "#".repeat(ebene as usize);
    let mut teile = Vec::with_capacity(abschnitte.len());
    for (i, s) in abschnitte.iter().enumerate() {
        let titel = s
            .get("titel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| fehler(format!("Abschnitt {i}: `titel` fehlt oder ist kein Text")))?;
        let inhalt = s
            .get("inhalt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| fehler(format!("Abschnitt {i}: `inhalt` fehlt oder ist kein Text")))?;
        // ⚑ Die Enden werden beschnitten, damit die Zahl der Leerzeilen
        // an der Form haengt und nicht daran, wie sorgfaeltig jemand
        // seinen Text abgetippt hat.
        let titel = zeilenenden(titel);
        let inhalt = zeilenenden(inhalt);
        teile.push(format!("{rauten} {}\n\n{}", titel.trim(), inhalt.trim()));
    }
    Ok(format!("{}\n", teile.join("\n\n")))
}

/// Die Ausfuehrung, wie der Werkzeugkasten sie braucht.
struct Ausfuehrung(Verankert);

impl Werkzeugausfuehrung for Ausfuehrung {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn ausfuehren(&self, argumente: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        self.0.ausfuehren(argumente)
    }
}

/// **Die Angebote dieser Kiste.**
///
/// ⚑ **Ohne Einhaengung**, anders als bei den Dateiwerkzeugen: Diese
/// Werkzeuge fassen nichts an, also gibt es nichts zu begrenzen. Genau
/// deshalb tragen sie den Auftrag „schreibe mir ein Dokument" auch im
/// Chat, wo niemand ein Verzeichnis eingehaengt hat.
pub fn angebote() -> Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    Verankert::ALLE
        .iter()
        .map(|w| {
            let angebot = Werkzeug {
                name: w.name().to_string(),
                beschreibung: w.beschreibung().to_string(),
                parameter: w.parameter(),
            };
            let ausf: Box<dyn Werkzeugausfuehrung> = Box::new(Ausfuehrung(*w));
            (angebot, ausf)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠️ **Ohne Vektoren keine Marke.** Die Datei haelt Eingabe und
    /// erwartete Ausgabe fest; sie ist der Vertrag zum Anfassen.
    #[test]
    fn die_vektoren_stimmen() {
        let roh = include_str!("../../verankerte_werkzeuge/vektoren.json");
        let v: serde_json::Value = serde_json::from_str(roh).expect("vektoren.json ist kein JSON");
        assert_eq!(
            v["vertrag"].as_str(),
            Some(VERTRAG),
            "die Vektoren gehoeren zu einer anderen Vertragsfassung"
        );
        let faelle = v["faelle"].as_array().expect("`faelle` fehlt");
        assert!(!faelle.is_empty(), "keine Vektoren ist kein Vertrag");

        let mut gesehen = std::collections::BTreeSet::new();
        for (i, f) in faelle.iter().enumerate() {
            let name = f["werkzeug"].as_str().expect("`werkzeug` fehlt");
            let w = Verankert::ALLE
                .iter()
                .find(|w| w.name() == name)
                .unwrap_or_else(|| panic!("Vektor {i}: unbekanntes Werkzeug {name}"));
            gesehen.insert(name.to_string());
            let ergebnis = w.ausfuehren(&f["ein"]);
            match f.get("aus").and_then(|a| a.as_str()) {
                Some(erwartet) => {
                    let ist = ergebnis
                        .unwrap_or_else(|e| panic!("Vektor {i} ({name}) fiel aus: {}", e.grund));
                    assert_eq!(ist, erwartet, "Vektor {i} ({name}) weicht ab");
                }
                // Ein Vektor darf auch einen Fehler festhalten: Dass
                // etwas **nicht** geht, ist Teil des Vertrags.
                None => assert!(
                    ergebnis.is_err(),
                    "Vektor {i} ({name}) sollte scheitern, gab aber ein Ergebnis"
                ),
            }
        }
        for w in Verankert::ALLE {
            assert!(
                gesehen.contains(w.name()),
                "{} hat keinen Vektor, ist also eine Behauptung",
                w.name()
            );
        }
    }

    /// ⛔️ **Kein Werkzeug dieser Kiste fasst den Rechner an.** Die
    /// Pruefung liest den Quelltext, weil ein Zugriff sonst erst im Netz
    /// auffiele, und dort als Abweichung zwischen zwei Knoten.
    #[test]
    fn nichts_hier_beruehrt_die_aussenwelt() {
        let quelle = include_str!("verankert.rs");
        // Nur der Rumpf, ohne die Dokumentation darueber: Sie spricht
        // ueber genau diese Dinge.
        let rumpf = quelle
            .split("use myl_local_agent::ausfuehrung")
            .nth(1)
            .expect("der Rumpf faengt hinter den Benutzungen an");
        for verboten in [
            "std::fs", "std::env", "std::process", "std::net", "SystemTime",
            "Instant", "rand", "to_lowercase", "to_uppercase", "sort",
        ] {
            assert!(
                !rumpf.contains(verboten),
                "`{verboten}` steht in dieser Kiste; sie waere nicht mehr nachrechenbar"
            );
        }
    }

    /// **Ein unbekanntes Feld ist ein Fehler, keine Luecke.**
    #[test]
    fn eine_fehlende_angabe_bricht_ab() {
        let a = serde_json::json!({"vorlage": "Hallo {wer}", "felder": {}});
        assert!(Verankert::VorlageFuellen.ausfuehren(&a).is_err());
    }

    /// **Zeilenenden werden vereinheitlicht**, sonst haengt der Abdruck
    /// daran, wie die Eingabe abgelegt war.
    #[test]
    fn crlf_wird_zu_lf() {
        let a = serde_json::json!({
            "vorlage": "a\r\n{x}",
            "felder": {"x": "b\r\nc"},
        });
        assert_eq!(Verankert::VorlageFuellen.ausfuehren(&a).unwrap(), "a\nb\nc");
    }
}
