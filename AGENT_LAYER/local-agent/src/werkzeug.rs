//! Das Werkzeugformat, und die Grenze aus Kap. 8.3 (AGENT_LAYER 5.2).
//!
//! # ⚑ Der Satz, um den es geht
//!
//! **Der Werkzeugaufruf ist ein Vorschlag des Modells. Die Erlaubnis
//! kommt von woanders.**
//!
//! Das klingt nach einer Formalie und ist die ganze Sicherheitsaussage
//! dieses Moduls. Ein Sprachmodell, das einen abgerufenen Text im
//! Kontext hat, kann von diesem Text gesteuert werden; das ist keine
//! Schwäche eines bestimmten Modells, sondern die Bauart. Kap. 8.3
//! verlangt deshalb, dass abgerufene Daten den Kontrollfluss **nicht
//! beeinflussen können**, und zwar strukturell statt filterbasiert.
//!
//! ⚑ **Strukturell heisst hier: Die Prüfung sieht den Vorschlag nicht
//! an.** Sie fragt nicht, ob er verdächtig aussieht, sondern ob er in
//! der Erlaubnis steht. Ein Filter, der Vorschläge nach Aussehen
//! sortiert, ist ein Wettrennen gegen den Formulierungsspielraum einer
//! Sprache; eine Positivliste ist keines.
//!
//! # ⚑ Woher die Erlaubnis kommt, und woher nicht
//!
//! | Quelle | zulässig |
//! |---|---|
//! | [`Erlaubnis`], gesetzt **vor** dem Lauf | ja |
//! | Sitzungskontrakt (`myl_types::sitzung`) | ja, er trägt sie |
//! | ein `myl_agent::Plan` | ja, er ist die strengste Form |
//! | **die Antwort des Modells** | **nein** |
//! | **das Ergebnis eines Werkzeugs** | **nein** |
//!
//! Die letzten beiden sind der Angriff. Ein Werkzeugergebnis, das den
//! Text `<tool_call>` enthält, ist **Daten**, nicht Steuerung, und dieses
//! Modul behandelt es so.
//!
//! # Das Format
//!
//! Die Hermes-Form, wie Qwen und die Hermes-Modelle sie sprechen: Die
//! Werkzeuge stehen als JSON in einer Systemnachricht, der Vorschlag
//! kommt als `<tool_call>{"name": …, "arguments": {…}}</tool_call>` im
//! Antworttext.
//!
//! ⚑ **In einer Systemnachricht und nicht in einem `tools`-Feld.** Die
//! Tür nimmt `model`, `messages`, `max_tokens` und `stream`; ein
//! `tools`-Feld fiele bei `serde` still weg, und ein Harness, das
//! Werkzeuge anbietet, die nie ankommen, sähe aus, als wären sie
//! angekommen.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::tuerklient::Nachricht;

/// Ein Werkzeug, wie das Modell es angeboten bekommt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Werkzeug {
    /// Der Name, unter dem das Modell es aufruft.
    pub name: String,
    /// Wofür es da ist, in einem Satz.
    pub beschreibung: String,
    /// Die Parameter, als JSON-Schema.
    pub parameter: serde_json::Value,
}

impl Werkzeug {
    /// Ein Werkzeug ohne Parameter.
    pub fn ohne_parameter(name: impl Into<String>, beschreibung: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            beschreibung: beschreibung.into(),
            parameter: serde_json::json!({"type": "object", "properties": {}}),
        }
    }
}

/// Was das Modell vorschlägt.
///
/// ⚑ **`Vorschlag` und nicht `Aufruf`**, und der Name ist die halbe
/// Miete: Wer ihn liest, weiss, dass hier noch nichts erlaubt ist.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Vorschlag {
    /// Welches Werkzeug das Modell aufrufen möchte.
    pub name: String,
    /// Die Argumente, wie das Modell sie vorschlägt.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Was ein Vorschlag **darf**.
///
/// ⚑ **Gesetzt, bevor der Lauf beginnt**, und danach nicht mehr
/// verändert. Eine Erlaubnis, die während des Laufs wachsen kann, ist
/// keine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Erlaubnis {
    erlaubt: BTreeSet<String>,
}

impl Erlaubnis {
    /// Genau diese Werkzeuge, sonst keines.
    ///
    /// ⚑ **Leer heisst: keines.** Das ist die sichere Lesart einer
    /// Positivliste, dieselbe wie bei der Empfängerliste des
    /// Sitzungskontrakts.
    pub fn genau(namen: &[&str]) -> Self {
        Self { erlaubt: namen.iter().map(|n| (*n).to_string()).collect() }
    }

    /// Die angebotenen Werkzeuge sind zugleich die erlaubten.
    ///
    /// ⚑ **Der bequeme Fall, und er ist trotzdem kein Freifahrtschein:**
    /// Angeboten wird, was der Aufrufer vor dem Lauf hinschreibt, nicht
    /// was das Modell sich wünscht.
    pub fn aus_angebot(werkzeuge: &[Werkzeug]) -> Self {
        Self { erlaubt: werkzeuge.iter().map(|w| w.name.clone()).collect() }
    }

    /// Ob dieser Name erlaubt ist.
    pub fn erlaubt(&self, name: &str) -> bool {
        self.erlaubt.contains(name)
    }

    /// Prüft einen Vorschlag: **darf** dieses Werkzeug gerufen werden?
    ///
    /// ⚑ **Die Prüfung sieht die Argumente nicht an**, und das ist
    /// Absicht: Ob es überhaupt gerufen werden darf, weiss nur die
    /// Erlaubnis. Wer beides hier vermischte, bekäme eine Prüfung, die
    /// je nach Argument anders entscheidet, und damit wieder einen
    /// Filter.
    ///
    /// **Ob die Argumente zum Werkzeug passen**, prüft
    /// [`argumente_pruefen`], und zwar gegen das **erklärte** Schema.
    /// Das ist kein Filter, sondern eine Typprüfung: Sie fragt nicht,
    /// ob etwas verdächtig aussieht, sondern ob es der Form entspricht,
    /// die das Werkzeug selbst angegeben hat.
    pub fn pruefen(&self, v: &Vorschlag) -> Result<(), Abgelehnt> {
        if self.erlaubt(&v.name) {
            Ok(())
        } else {
            Err(Abgelehnt { name: v.name.clone(), erlaubt: self.erlaubt.iter().cloned().collect() })
        }
    }
}

/// Ein Vorschlag, der nicht in der Erlaubnis steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abgelehnt {
    /// Was das Modell wollte.
    pub name: String,
    /// Was erlaubt gewesen wäre.
    pub erlaubt: Vec<String>,
}

impl std::fmt::Display for Abgelehnt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "das Modell schlug `{}` vor; erlaubt sind {:?}. \
             Ein Vorschlag ist keine Erlaubnis (Kap. 8.3)",
            self.name, self.erlaubt
        )
    }
}

impl std::error::Error for Abgelehnt {}

/// Die Systemnachricht, die dem Modell die Werkzeuge anbietet.
///
/// ⚑ **Sie sagt dem Modell, was es vorschlagen darf, und das ist eine
/// Bequemlichkeit, keine Sicherung.** Ein Modell, das etwas anderes
/// vorschlägt, wird von [`Erlaubnis::pruefen`] abgelehnt, nicht von
/// diesem Text.
pub fn angebot(werkzeuge: &[Werkzeug]) -> Nachricht {
    let liste: Vec<serde_json::Value> = werkzeuge
        .iter()
        .map(|w| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": w.name,
                    "description": w.beschreibung,
                    "parameters": w.parameter,
                }
            })
        })
        .collect();
    let mut text = String::from(
        "Du kannst Werkzeuge aufrufen. Die verfuegbaren stehen in <tools></tools>:\n<tools>\n",
    );
    for w in &liste {
        text.push_str(&serde_json::to_string(w).unwrap_or_default());
        text.push('\n');
    }
    text.push_str(
        "</tools>\nFuer einen Aufruf gib ein JSON-Objekt mit \"name\" und \"arguments\" \
         zurueck, eingefasst in <tool_call></tool_call>.",
    );
    Nachricht::system(text)
}

/// Liest die Vorschläge aus **einer Modellantwort**.
///
/// # ⚑ Aus der Antwort des Modells, und aus nichts anderem
///
/// Diese Funktion darf **nie** auf ein Werkzeugergebnis angewendet
/// werden. Ein Ergebnis ist Daten; enthielte es `<tool_call>` und würde
/// hier hindurchgeschickt, hätten abgerufene Daten den Kontrollfluss
/// bestimmt, und genau das schliesst Kap. 8.3 aus.
///
/// ⚑ **Die Regel steht im Typ, soweit sie sich dort ausdrücken lässt:**
/// [`Werkzeugergebnis`] geht als [`Nachricht`] zurück ins Gespräch und
/// kommt an dieser Funktion nicht vorbei. Was bleibt, ist die Disziplin
/// des Aufrufers, und dafür steht dieser Absatz.
///
/// **Nicht mit `serde_json` über den ganzen Text**, sondern zwischen den
/// Marken: Ein Modell schreibt vor und nach dem Aufruf gern noch etwas.
pub fn vorschlaege(antwort: &str) -> Vec<Result<Vorschlag, Unlesbar>> {
    const AUF: &str = "<tool_call>";
    const ZU: &str = "</tool_call>";
    let mut aus = Vec::new();
    let mut rest = antwort;
    while let Some(a) = rest.find(AUF) {
        let nach = &rest[a + AUF.len()..];
        let Some(e) = nach.find(ZU) else {
            aus.push(Err(Unlesbar { roh: nach.chars().take(120).collect() }));
            break;
        };
        let inhalt = nach[..e].trim();
        match serde_json::from_str::<Vorschlag>(inhalt) {
            Ok(v) => aus.push(Ok(v)),
            Err(_) => aus.push(Err(Unlesbar { roh: inhalt.chars().take(120).collect() })),
        }
        rest = &nach[e + ZU.len()..];
    }
    aus
}

/// Ein Aufrufblock, der kein lesbarer Vorschlag ist.
///
/// ⚑ **Ein eigener Fall und keine stille Auslassung.** Wer ihn
/// überginge, meldete „das Modell hat nichts vorgeschlagen", während es
/// etwas vorgeschlagen hat, das niemand versteht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlesbar {
    /// Was dort stand, gekürzt.
    pub roh: String,
}

/// Das Ergebnis eines Werkzeugs, auf dem Weg zurück ins Gespräch.
///
/// ⚑ **Es geht als Nachricht zurück und nicht als Steuerung.** Die Rolle
/// heisst `tool`, und der Inhalt ist, was das Werkzeug geliefert hat,
/// unverändert und ungeprüft: Ein Werkzeug, dessen Ergebnis vor dem
/// Einfügen „gereinigt" würde, wäre wieder ein Filter.
pub struct Werkzeugergebnis;

impl Werkzeugergebnis {
    /// Die Nachricht, die ein Ergebnis ins Gespräch trägt.
    pub fn nachricht(name: &str, inhalt: &str) -> Nachricht {
        Nachricht {
            role: "tool".to_string(),
            content: format!("<tool_response>\n{{\"name\": \"{name}\", \"content\": {}}}\n</tool_response>",
                serde_json::to_string(inhalt).unwrap_or_else(|_| "\"\"".to_string())),
        }
    }
}


/// Warum die Argumente nicht zum Werkzeug passen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argumentfehler {
    /// Die Argumente sind kein Objekt.
    KeinObjekt,
    /// Ein verlangtes Feld fehlt.
    FeldFehlt {
        /// Welches.
        name: String,
    },
    /// Ein Feld steht nicht im Schema.
    ///
    /// ⚑ **Unbekannte Felder sind ein Fehler und keine Zugabe.** Ein
    /// Werkzeug, das ein Feld nicht kennt, übergeht es, und dann hat der
    /// Aufruf etwas anderes getan, als der Vorschlag sagte. Genau das
    /// heisst „Tool Argument Manipulation".
    FeldUnbekannt {
        /// Welches.
        name: String,
        /// Was es gibt.
        bekannt: Vec<String>,
    },
    /// Ein Feld hat den falschen Typ.
    FalscherTyp {
        /// Welches Feld.
        name: String,
        /// Was das Schema verlangt.
        erwartet: String,
        /// Was dastand.
        erhalten: String,
    },
}

impl std::fmt::Display for Argumentfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeinObjekt => f.write_str("die Argumente sind kein Objekt"),
            Self::FeldFehlt { name } => write!(f, "das verlangte Feld `{name}` fehlt"),
            Self::FeldUnbekannt { name, bekannt } => write!(
                f,
                "das Feld `{name}` steht nicht im Schema; bekannt sind {bekannt:?}. \
                 Ein Werkzeug uebergeht, was es nicht kennt, und dann tut der Aufruf \
                 etwas anderes als der Vorschlag sagt"
            ),
            Self::FalscherTyp { name, erwartet, erhalten } => write!(
                f,
                "das Feld `{name}` ist {erhalten}, das Schema verlangt {erwartet}"
            ),
        }
    }
}

impl std::error::Error for Argumentfehler {}

/// Prüft die Argumente eines Vorschlags gegen das **erklärte** Schema
/// des Werkzeugs.
///
/// # ⚑ Eine Typprüfung und kein Filter
///
/// Der Unterschied ist derselbe wie zwischen Kodieren und Bereinigen:
/// Diese Funktion fragt nicht, ob ein Wert **verdächtig aussieht**,
/// sondern ob er der Form entspricht, die das Werkzeug **selbst
/// angegeben** hat. Ein Filter wäre ein Wettrennen gegen den
/// Formulierungsspielraum; eine Formprüfung gegen eine erklärte Form
/// ist keines.
///
/// # ⚑ Was sie leistet und was ausdrücklich nicht
///
/// **Geprüft wird:** Objekt, verlangte Felder da, keine unbekannten
/// Felder, und der Typ der Felder, die das Schema nennt (`string`,
/// `number`, `integer`, `boolean`, `array`, `object`).
///
/// ⚑ **Nicht geprüft wird der Wert.** Ob 999 ein sinnvoller Betrag ist,
/// weiss dieses Modul nicht und soll es nicht wissen: **Wirkungen
/// begrenzt der Sitzungskontrakt**, und der wird von der Kette
/// durchgesetzt, nicht vom Harness. Eine zweite Grenze hier wäre eine
/// zweite Meinung über dieselbe Frage, und die schwächere wäre die
/// geglaubte.
///
/// **Und es ist keine vollständige JSON-Schema-Umsetzung.** `minimum`,
/// `pattern`, `enum` und die übrigen Stichworte werden **nicht**
/// ausgewertet. Wer sie ins Schema schreibt, bekommt sie nicht geprüft,
/// und dieser Absatz steht hier, damit niemand das Gegenteil annimmt.
pub fn argumente_pruefen(
    werkzeug: &Werkzeug,
    v: &Vorschlag,
) -> Result<(), Argumentfehler> {
    let Some(felder) = v.arguments.as_object() else {
        // ⚑ Ein fehlendes `arguments` ist ein leeres Objekt, kein
        // Fehler: Ein Werkzeug ohne Parameter wird ohne Argumente
        // gerufen.
        if v.arguments.is_null() {
            return leere_pruefen(werkzeug);
        }
        return Err(Argumentfehler::KeinObjekt);
    };
    let schema = werkzeug.parameter.get("properties").and_then(|p| p.as_object());
    let bekannt: Vec<String> =
        schema.map(|s| s.keys().cloned().collect()).unwrap_or_default();

    for name in felder.keys() {
        if !bekannt.contains(name) {
            return Err(Argumentfehler::FeldUnbekannt {
                name: name.clone(),
                bekannt: bekannt.clone(),
            });
        }
    }
    if let Some(noetig) = werkzeug.parameter.get("required").and_then(|r| r.as_array()) {
        for n in noetig {
            let Some(name) = n.as_str() else { continue };
            if !felder.contains_key(name) {
                return Err(Argumentfehler::FeldFehlt { name: name.to_string() });
            }
        }
    }
    if let Some(schema) = schema {
        for (name, wert) in felder {
            let Some(erwartet) = schema.get(name).and_then(|s| s.get("type")).and_then(|t| t.as_str())
            else {
                continue;
            };
            if !typ_passt(erwartet, wert) {
                return Err(Argumentfehler::FalscherTyp {
                    name: name.clone(),
                    erwartet: erwartet.to_string(),
                    erhalten: typname(wert).to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Ein Vorschlag ohne Argumente gegen ein Schema mit Pflichtfeldern.
fn leere_pruefen(werkzeug: &Werkzeug) -> Result<(), Argumentfehler> {
    if let Some(noetig) = werkzeug.parameter.get("required").and_then(|r| r.as_array()) {
        if let Some(erstes) = noetig.first().and_then(|n| n.as_str()) {
            return Err(Argumentfehler::FeldFehlt { name: erstes.to_string() });
        }
    }
    Ok(())
}

fn typ_passt(erwartet: &str, wert: &serde_json::Value) -> bool {
    match erwartet {
        "string" => wert.is_string(),
        "number" => wert.is_number(),
        "integer" => wert.is_i64() || wert.is_u64(),
        "boolean" => wert.is_boolean(),
        "array" => wert.is_array(),
        "object" => wert.is_object(),
        // ⚑ Ein Typ, den diese Prüfung nicht kennt, wird **nicht**
        // stillschweigend durchgewunken und auch nicht abgelehnt: Sie
        // sagt dazu nichts, und der Modulkopf sagt, dass sie das tut.
        _ => true,
    }
}

fn typname(w: &serde_json::Value) -> &'static str {
    match w {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
