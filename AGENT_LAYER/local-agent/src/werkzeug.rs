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
pub fn angebot(werkzeuge: &[Werkzeug], form: Ansageform) -> Nachricht {
    angebot_mit_regel(werkzeuge, form, None)
}

/// **Wie [`angebot`], mit einer Hausregel hinter der Vorlage.**
///
/// # ⛔️ Hinter der Vorlage und nicht darin
///
/// Kopf und Fuss sind **zeichengleich** die Literale aus
/// `tokenizer_config.json`, und eine Pruefung haelt sie dagegen. Eine
/// Regel, die sich dazwischenschiebt, aendert den Schliff, auf den das
/// Modell trainiert wurde. **Sie steht deshalb als eigener Absatz
/// dahinter**, dort, wo ein Systemprompt ueblicherweise steht.
///
/// # ⛔️ Und sie ist nie freier Text eines einzelnen Knotens
///
/// ⚑ **Im Netz ruesten alle Knoten gleich, sonst rechnen sie
/// Verschiedenes.** Dieselbe Ueberlegung wie bei der Werkzeugkiste: Was
/// in die Ansage geht, gehoert zu den Protokollgroessen und ist fuer
/// alle dasselbe. Dieser Parameter ist fuer die **Messung** und fuer
/// den oertlichen Betrieb; wer ihn im Netz verschieden belegt, bricht
/// die Nachrechenbarkeit, und das steht hier, damit es niemand
/// versehentlich tut.
pub fn angebot_mit_regel(
    werkzeuge: &[Werkzeug],
    form: Ansageform,
    regel: Option<&str>,
) -> Nachricht {
    let liste: Vec<Ansageeintrag<'_>> = werkzeuge
        .iter()
        .map(|w| Ansageeintrag {
            art: "function",
            function: Ansagefunktion {
                name: &w.name,
                description: &w.beschreibung,
                parameters: &w.parameter,
            },
        })
        .collect();
    let (kopf, fuss) = form.rahmen();
    let mut text = String::from(kopf);
    // ⚑ **Der Umbruch VOR dem Werkzeug und nicht danach.** So steht es
    // in der Vorlage, und so bleiben `kopf` und `fuss` genau die beiden
    // Literale aus `tokenizer_config.json`. Bei null Werkzeugen kommt
    // dasselbe heraus wie andersherum; bei einem ist der Unterschied ein
    // Umbruch zu viel vor `</tools>`.
    for w in &liste {
        text.push('\n');
        text.push_str(&serde_json::to_string(w).unwrap_or_default());
    }
    text.push_str(fuss);
    if let Some(r) = regel.map(str::trim).filter(|r| !r.is_empty()) {
        text.push_str("\n\n");
        text.push_str(r);
    }
    Nachricht::system(text)
}

/// Ein Werkzeug, wie es in der Ansage steht.
///
/// # 📌 Warum das eine Struktur ist und kein `json!`
///
/// Der erste Entwurf baute den Eintrag mit `serde_json::json!`, und
/// `serde_json::Map` ist ohne das Merkmal `preserve_order` ein
/// `BTreeMap`: **Die Schluessel kommen alphabetisch heraus.** Das Modell
/// sah also
///
/// ```json
/// {"function":{"description":"…","name":"read_file","parameters":{…}},"type":"function"}
/// ```
///
/// wo sein Schliff
///
/// ```json
/// {"type": "function", "function": {"name": "…", "description": "…", "parameters": {…}}}
/// ```
///
/// vorsieht: `type` zuerst, und der Name **vor** der Beschreibung.
/// Serde gibt Strukturfelder in der Reihenfolge ihrer Deklaration
/// heraus, unabhaengig davon; deshalb steht das hier als Struktur.
///
/// ⚑ **Und nicht als Merkmal `preserve_order`**, obwohl das kuerzer
/// waere: Merkmale vereinigen sich ueber den ganzen Abhaengigkeitsbaum,
/// die Aenderung traefe also jede JSON-Ausgabe in allen zwanzig Kisten.
/// Ein Formatproblem in der Werkzeugansage rechtfertigt keinen Eingriff
/// in die Ausgabe der Tuer.
///
/// ⚠️ **Was damit noch nicht stimmt:** `parameters` bleibt ein `Value`
/// und darin sind die Schluessel weiter sortiert
/// (`properties`, `required`, `type` statt `type` zuerst). Das ist die
/// unterste der drei Ebenen und die, an der ein Schema ohnehin am
/// staerksten variiert. Es steht hier, damit es benannt ist und nicht
/// fuer erledigt gehalten wird.
#[derive(Serialize)]
struct Ansageeintrag<'a> {
    #[serde(rename = "type")]
    art: &'static str,
    function: Ansagefunktion<'a>,
}

#[derive(Serialize)]
struct Ansagefunktion<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
}

/// In welcher Form die Werkzeuge angesagt werden.
///
/// # 📌 Warum es diese Wahl ueberhaupt gibt
///
/// Bis zum 2026-09-08 gab es nur eine Form, und sie war eine **deutsche
/// Paraphrase** der Vorlage, auf die das Modell geschliffen wurde. Der
/// Vergleich mit `INTEGER_LLM/models/Qwen3-4B/tokenizer_config.json`
/// zeigte drei Abweichungen: die Ueberschrift `# Tools` fehlte, der Text
/// war deutsch statt englisch, und, vermutlich am teuersten, **das
/// Aufrufbeispiel fehlte ganz**. Die amtliche Vorlage *zeigt*
///
/// ```text
/// <tool_call>
/// {"name": <function-name>, "arguments": <args-json-object>}
/// </tool_call>
/// ```
///
/// als Literal; unsere *beschrieb* das Format in einem Satz. Genau
/// dieses Literal hat das Modell im Schliff tausendfach gesehen.
///
/// ⚑ **Das ist Fund 215 eine Ebene hoeher.** Dort war es der rohe Text
/// statt ChatML, hier die nachgebaute Werkzeugansage: In beiden Faellen
/// wird die trainierte Oberflaeche des Modells nachgebaut statt benutzt,
/// und damit der Schliff weggeworfen, fuer den das
/// instruktionsgeschliffene Modell ueberhaupt gewaehlt wurde.
///
/// # ⚑ Warum beide Formen bleiben und nicht nur die neue
///
/// Weil die Behauptung messbar ist und noch nicht gemessen wurde.
/// `BENCHMARKS/Agent/agentenprobe.py` liegt fertig da und ist nie als
/// Sammlung gelaufen, es gibt also **keinen Nullwert**. Eine Umstellung
/// ohne Vergleich waere eine zweite Behauptung an der Stelle der ersten.
/// Diese Aufzaehlung ist der Schalter fuer den Vergleich und
/// verschwindet, sobald er gefallen ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ansageform {
    /// Wortgleich zur Vorlage des Modells.
    ///
    /// ⚑ **Die Vorgabe, und sie ruht auf der Vorlage und nicht auf einer
    /// Messung.** Das steht hier ausdruecklich, damit niemand sie fuer
    /// belegt haelt: Sie ist die bessere Vermutung, bis die Probe
    /// gelaufen ist.
    #[default]
    Amtlich,
    /// Die deutsche Paraphrase, die bis zum 2026-09-08 die einzige war.
    /// Steht nur noch fuer den Vergleich da.
    Deutsch,
}

impl Ansageform {
    /// Was vor und was nach der Werkzeugliste steht.
    ///
    /// ⚑ **Der amtliche Text ist Zeichen fuer Zeichen der aus
    /// `tokenizer_config.json`**, samt der beiden Leerzeilen. Wer ihn
    /// anfasst, sollte ihn vorher dort nachlesen; „fast gleich" ist hier
    /// der ganze Unterschied.
    fn rahmen(&self) -> (&'static str, &'static str) {
        match self {
            // ⚑ Zeichen fuer Zeichen die beiden Literale aus
            // `tokenizer_config.json`. Nicht umbrechen und nicht
            // huebsch machen: `tests/werkzeugansage.rs` vergleicht sie
            // gegen die abgelegte Fassung, und die gegen die Vorlage.
            #[rustfmt::skip]
            Self::Amtlich => (
                "# Tools\n\nYou may call one or more functions to assist with the user query.\n\nYou are provided with function signatures within <tools></tools> XML tags:\n<tools>",
                "\n</tools>\n\nFor each function call, return a json object with function name and arguments within <tool_call></tool_call> XML tags:\n<tool_call>\n{\"name\": <function-name>, \"arguments\": <args-json-object>}\n</tool_call>",
            ),
            #[rustfmt::skip]
            Self::Deutsch => (
                "Du kannst Werkzeuge aufrufen. Die verfuegbaren stehen in <tools></tools>:\n<tools>",
                "\n</tools>\nFuer einen Aufruf gib ein JSON-Objekt mit \"name\" und \"arguments\" zurueck, eingefasst in <tool_call></tool_call>.",
            ),
        }
    }

    /// Kurzform fuer Protokoll und Schalter.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Amtlich => "amtlich",
            Self::Deutsch => "deutsch",
        }
    }
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
