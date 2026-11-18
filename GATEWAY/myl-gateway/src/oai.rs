//! Die Fläche nach aussen in der OpenAI-Form (GATEWAY Stufe 3).
//!
//! # ⚑ Warum ausgerechnet diese Form
//!
//! **Weil der Nutzer den Schlüssel irgendwo einkleben muss.** Das war
//! Fund 150: Die erste Fassung der Tür verlangte eine Unterschrift je
//! Anfrage, war korrekt und **unbenutzbar**, weil kein Harness das
//! spricht. Ein Wechsel des Anbieters heisst in jedem Werkzeug dieser
//! Welt: Basis-URL und Schlüssel tauschen. Wer eine eigene Form
//! erfindet, verlangt von jedem Nutzer einen eigenen Klienten.
//!
//! Der Zuschnitt steht in B6-3: `http://127.0.0.1:<port>/v1`, Bearer,
//! kein TLS, kein Rahmenwerk. Die Tür hört auf der Rückschleife.
//!
//! # ⚑ Was diese Tür anders macht, und warum sie es sagen muss
//!
//! **`temperature`, `top_p` und `seed` werden angenommen und haben
//! keine Wirkung.** Myelith rechnet ganzzahlig und deterministisch: Bei
//! gleichem Prompt und gleichem Pipeline-Stand ist die Ausgabe
//! bitgleich, und das ist keine Einstellung, sondern die
//! Geschäftsgrundlage. Ohne sie könnten zwei Pods derselben
//! Redundanzpaarung nicht verglichen werden, und der ganze
//! Verifikationsbau hätte keinen Boden.
//!
//! **Stillschweigend zu ignorieren wäre falsch**, denn wer `temperature`
//! setzt, erwartet Streuung. Deshalb nennt die Antwort das Feld
//! `myelith_deterministisch` und `/v1/models` sagt es noch einmal. Eine
//! Ablehnung wäre die Alternative gewesen und hätte jedes Harness
//! gebrochen, das den Wert immer mitschickt.
//!
//! # ⚑ Kein `stream: true`
//!
//! Ein Strom verlangt `Transfer-Encoding: chunked`, und die Tür lehnt
//! stückweise Nachrichten ausdrücklich ab: Wer stückweise liest, muss
//! raten, wo die Nachricht aufhört. Ein Auftrag wird ausserdem als
//! Ganzes bezeugt und abgerechnet; eine halbe Antwort hätte keinen
//! Beleg. Wer `stream: true` schickt, bekommt eine Ablehnung mit Grund
//! und keine stille Falschbedienung.

use serde::{Deserialize, Serialize};

/// Der Weg für eine Vervollständigung.
pub const WEG_CHAT: &str = "/v1/chat/completions";
/// Der Weg für die Modellliste.
pub const WEG_MODELLE: &str = "/v1/models";
/// Der Weg für eine Vervollständigung aus einem fertigen Prompt.
///
/// # ⚑ Wofür (2026-11-15)
///
/// Für einen Klienten, der die Vorlage seines Modells kennt und den Prompt
/// selbst setzt, Zeichen für Zeichen wie auf seiner eigenen Maschine. Der
/// Agent von Myelith tut das: Über [`WEG_CHAT`] bekäme das Modell die
/// Unterhaltung als `rolle: inhalt` (Fund 181), eine Form, auf der es nie
/// trainiert wurde und auf der es seltener Werkzeuge aufruft.
///
/// ⚑ **Kein neues Recht.** Auch über [`WEG_CHAT`] gehen Sonderzeichen wie
/// `<|im_start|>` im Inhalt unverändert zum Wortschatz; hier fällt nur die
/// Zusammensetzung weg. Ausweis, Deckel, Beleg und Abrechnung sind dieselben.
pub const WEG_TEXT: &str = "/v1/completions";

/// Höchstzahl von Nachrichten in einer Anfrage.
///
/// ⚑ **Ein Deckel vor der Arbeit.** Ohne ihn bestimmt der Anfragende,
/// wie viele Zeichenketten das Gateway zusammensetzt, und `MAX_RUMPF`
/// allein liesse Hunderttausende leerer Nachrichten zu.
pub const MAX_NACHRICHTEN: usize = 1024;

/// Eine Nachricht der Unterhaltung.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Nachricht {
    pub role: String,
    /// ⚑ **Nur Text.** Die OpenAI-Form erlaubt hier auch eine Liste von
    /// Teilen (Bilder, Ton). Myelith rechnet Text; eine Anfrage mit
    /// Bildteilen wird abgelehnt und nicht halb verstanden. ⚑ `null` gilt
    /// als leer (Fund 181): Eine Antwort, die nur Werkzeuge aufruft, hat
    /// keinen Text.
    #[serde(default, deserialize_with = "text_oder_null")]
    pub content: String,
    /// ⚑ **Die Werkzeugaufrufe einer Antwort** (Fund 181), in der Antwort
    /// der Tür und im Verlauf, den ein Harness zurückschickt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Werkzeugaufruf>>,
    /// Auf welchen Aufruf eine Werkzeugantwort (`role: tool`) antwortet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

fn text_oder_null<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// Ein Werkzeugaufruf in der OpenAI-Form.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Werkzeugaufruf {
    pub id: String,
    #[serde(rename = "type", default = "art_funktion")]
    pub art: String,
    pub function: Aufruffunktion,
}

/// Name und Argumente eines Aufrufs; die Argumente als JSON-Text, wie
/// OpenAI sie schickt. Ein Objekt wird ebenso angenommen.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Aufruffunktion {
    pub name: String,
    #[serde(default, deserialize_with = "argumente_lesen")]
    pub arguments: String,
}

fn art_funktion() -> String {
    "function".to_string()
}

fn argumente_lesen<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        anders => anders.to_string(),
    })
}

/// ⚑ **Ein angebotenes Werkzeug** in der OpenAI-Form
/// (`{"type":"function","function":{…}}`, Fund 181).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Werkzeug {
    #[serde(rename = "type", default = "art_funktion")]
    pub art: String,
    pub function: Werkzeugfunktion,
}

/// Name, Beschreibung und Parameterschema eines angebotenen Werkzeugs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Werkzeugfunktion {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// Höchstzahl angebotener Werkzeuge: ein Deckel vor der Arbeit, wie
/// [`MAX_NACHRICHTEN`].
pub const MAX_WERKZEUGE: usize = 128;

/// Eine Anfrage an [`WEG_CHAT`].
///
/// ⚑ **Nur die Felder, die etwas bewirken.** `temperature` und
/// Verwandte stehen bewusst **nicht** hier: Ein Feld, das der Typ
/// trägt und das Programm ignoriert, sieht aus wie eine Einstellung.
/// `serde` lässt unbekannte Felder fallen, und genau das ist hier
/// richtig.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Chatanfrage {
    #[serde(default)]
    pub model: String,
    pub messages: Vec<Nachricht>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Neuere Klienten schicken das statt `max_tokens`.
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    /// ⚑ **Die angebotenen Werkzeuge** (Fund 181): Die Tür sagt sie in der
    /// amtlichen Form des Netzmodells an und gibt Aufrufe des Modells als
    /// `tool_calls` zurück. Ausgeführt wird nichts; das bleibt beim Harness.
    #[serde(default)]
    pub tools: Vec<Werkzeug>,
    /// `auto` (die Vorgabe) oder `none`. ⚑ **Erzwingen kann die Tür keinen
    /// Aufruf**; `required` oder ein bestimmtes Werkzeug lehnt sie ab, statt
    /// still `auto` daraus zu machen.
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
}

/// Was an einer Anfrage nicht stimmt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anfragefehler {
    /// Der Rumpf ist kein JSON oder hat die falsche Gestalt.
    Unlesbar,
    /// Eine Unterhaltung ohne Nachrichten ist keine.
    Leer,
    /// Mehr Nachrichten als [`MAX_NACHRICHTEN`].
    ZuVieleNachrichten { anzahl: usize },
    /// Alle Nachrichten zusammen ergeben keinen Text.
    LeererPrompt,
    /// `stream: true`, und das kann diese Tür nicht.
    StromVerlangt,
    /// Ein angebotenes Werkzeug ist keine Funktion mit Namen, oder es sind
    /// zu viele.
    WerkzeugUnlesbar,
    /// `tool_choice` verlangt, was die Tür nicht erzwingen kann.
    WerkzeugwahlNichtErfuellbar,
}

impl std::fmt::Display for Anfragefehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unlesbar => f.write_str("der Rumpf ist kein lesbares JSON dieser Form"),
            Self::Leer => f.write_str("messages ist leer"),
            Self::ZuVieleNachrichten { anzahl } => write!(
                f,
                "{anzahl} Nachrichten, erlaubt sind {MAX_NACHRICHTEN}"
            ),
            Self::LeererPrompt => f.write_str("die Nachrichten ergeben keinen Text"),
            Self::StromVerlangt => f.write_str(
                "stream: true wird nicht bedient: diese Tuer liefert eine ganze Antwort, \
                 weil ein Auftrag als Ganzes bezeugt und abgerechnet wird",
            ),
            Self::WerkzeugUnlesbar => write!(
                f,
                "tools: jedes Werkzeug braucht type function und einen Namen, hoechstens {MAX_WERKZEUGE}"
            ),
            Self::WerkzeugwahlNichtErfuellbar => f.write_str(
                "tool_choice: diese Tuer kann keinen Aufruf erzwingen; moeglich sind auto und none",
            ),
        }
    }
}

impl Chatanfrage {
    /// Liest eine Anfrage aus dem Rumpf.
    pub fn lesen(rumpf: &[u8]) -> Result<Self, Anfragefehler> {
        let a: Self = serde_json::from_slice(rumpf).map_err(|_| Anfragefehler::Unlesbar)?;
        a.pruefen()?;
        Ok(a)
    }

    fn pruefen(&self) -> Result<(), Anfragefehler> {
        if self.stream {
            return Err(Anfragefehler::StromVerlangt);
        }
        if self.messages.is_empty() {
            return Err(Anfragefehler::Leer);
        }
        if self.messages.len() > MAX_NACHRICHTEN {
            return Err(Anfragefehler::ZuVieleNachrichten {
                anzahl: self.messages.len(),
            });
        }
        if self.messages.iter().all(|m| m.content.trim().is_empty()) {
            return Err(Anfragefehler::LeererPrompt);
        }
        if self.tools.len() > MAX_WERKZEUGE
            || self.tools.iter().any(|w| w.art != "function" || w.function.name.trim().is_empty())
        {
            return Err(Anfragefehler::WerkzeugUnlesbar);
        }
        match &self.tool_choice {
            None => {}
            Some(serde_json::Value::String(w)) if w == "auto" || w == "none" => {}
            Some(_) => return Err(Anfragefehler::WerkzeugwahlNichtErfuellbar),
        }
        Ok(())
    }

    /// Wie viele neue Token höchstens, mit Vorgabe.
    ///
    /// ⚑ **`max_completion_tokens` schlägt `max_tokens`**, wie bei
    /// OpenAI seit der Umbenennung. Wer beides schickt, meint das
    /// neuere.
    pub fn token_deckel(&self, vorgabe: u32) -> u32 {
        self.max_completion_tokens
            .or(self.max_tokens)
            .unwrap_or(vorgabe)
    }

    /// ⚑ **Werden Werkzeuge angesagt?** Ja, wenn welche angeboten sind und
    /// `tool_choice` nicht `none` sagt.
    pub fn werkzeuge_angeboten(&self) -> bool {
        !self.tools.is_empty() && !matches!(&self.tool_choice, Some(serde_json::Value::String(w)) if w == "none")
    }

    /// Die Namen der angebotenen Werkzeuge; leer, wenn keine angeboten sind
    /// ([`Self::werkzeuge_angeboten`]).
    pub fn angebotene_namen(&self) -> Vec<String> {
        if !self.werkzeuge_angeboten() {
            return Vec::new();
        }
        self.tools.iter().map(|w| w.function.name.clone()).collect()
    }

    /// **Setzt die Unterhaltung in der Vorlage des Netzmodells zusammen**
    /// (Fund 181, Festlegung des Projektinhabers vom 2026-11-16).
    ///
    /// 📌 **Bis hierher stand die Unterhaltung als `rolle: inhalt` da**, eine
    /// Form, auf der das Modell nie trainiert wurde. Für eine einzelne Frage
    /// fiel das kaum auf; ein Harness bietet aber Werkzeuge an und erwartet
    /// `<tool_call>`-Blöcke zurück, und genau diese Fähigkeit hängt an der
    /// Vorlage. Die Sorge, eine andere Vorlage bewege den
    /// Konformitätswert, trug nicht: Die E2E-Vektoren nehmen Token als
    /// Eingabe, nicht diesen Prompt.
    ///
    /// ⚑ **Wie die Vorlage der Qwen-Familie es tut:** Die Werkzeugansage
    /// steht in der Systemnachricht nach deren Text (ohne Systemnachricht
    /// als eigene); ein Aufruf im Verlauf als `<tool_call>`-Block in der
    /// Antwort; Werkzeugantworten hintereinander als `<tool_response>`-Blöcke
    /// in einer Nutzerrunde. Der Denkblock steht leer da: Ein Harness will
    /// eine Antwort oder einen Aufruf, keine Überlegung.
    pub fn prompt_in(&self, vorlage: myl_types::vorlage::Vorlage) -> String {
        use crate::werkzeuge::{antwort_als_text, aufruf_als_text, werkzeuge_ansagen, Werkzeugangebot};
        let mut ansage = self.werkzeuge_angeboten().then(|| {
            let angebot: Vec<Werkzeugangebot> = self
                .tools
                .iter()
                .map(|w| Werkzeugangebot {
                    name: w.function.name.clone(),
                    beschreibung: w.function.description.clone(),
                    parameter: w.function.parameters.clone(),
                })
                .collect();
            werkzeuge_ansagen(&angebot)
        });
        let mut paare: Vec<(String, String)> = Vec::with_capacity(self.messages.len() + 1);
        for (i, m) in self.messages.iter().enumerate() {
            match m.role.as_str() {
                "system" if i == 0 && ansage.is_some() => {
                    let a = ansage.take().unwrap_or_default();
                    let inhalt = if m.content.is_empty() { a } else { format!("{}\n\n{a}", m.content) };
                    paare.push(("system".to_string(), inhalt));
                }
                "tool" => {
                    let text = antwort_als_text(&m.content);
                    let fortsetzen = i > 0 && self.messages[i - 1].role == "tool";
                    match paare.last_mut() {
                        Some((_, inhalt)) if fortsetzen => {
                            inhalt.push('\n');
                            inhalt.push_str(&text);
                        }
                        _ => paare.push(("user".to_string(), text)),
                    }
                }
                "assistant" if m.tool_calls.as_ref().is_some_and(|a| !a.is_empty()) => {
                    let mut teile: Vec<String> = Vec::new();
                    if !m.content.is_empty() {
                        teile.push(m.content.clone());
                    }
                    for a in m.tool_calls.iter().flatten() {
                        let argumente = serde_json::from_str(&a.function.arguments)
                            .unwrap_or_else(|_| serde_json::Value::String(a.function.arguments.clone()));
                        teile.push(aufruf_als_text(&a.function.name, &argumente));
                    }
                    paare.push(("assistant".to_string(), teile.join("\n")));
                }
                rolle => paare.push((rolle.to_string(), m.content.clone())),
            }
        }
        if let Some(a) = ansage {
            paare.insert(0, ("system".to_string(), a));
        }
        let paare: Vec<(&str, &str)> = paare.iter().map(|(r, t)| (r.as_str(), t.as_str())).collect();
        vorlage.bauen(&paare, false)
    }
}

/// Eine Wahl in der Antwort.
#[derive(Debug, Clone, Serialize)]
pub struct Wahl {
    pub index: u32,
    pub message: Nachricht,
    pub finish_reason: String,
}

/// Die Verbrauchsangabe.
#[derive(Debug, Clone, Serialize)]
pub struct Verbrauch {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Die Antwort auf [`WEG_CHAT`].
#[derive(Debug, Clone, Serialize)]
pub struct Chatantwort {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Wahl>,
    pub usage: Verbrauch,
    /// ⚑ **Der Hinweis, der die Falschbedienung verhindert.** Wer
    /// `temperature` gesetzt hat, sieht hier, dass sie nichts bewirkt
    /// hat.
    pub myelith_deterministisch: bool,
    /// Das Segment, unter dem die Arbeit bezeugt wurde.
    ///
    /// ⚑ **Der Faden vom Beleg des Nutzers zur bezeugten Arbeit.** Ohne
    /// ihn hätte er Token und keine Möglichkeit, sie einer Abrechnung
    /// zuzuordnen.
    pub myelith_segment: String,
    /// Die Sitzungsnummer dieser Anfrage.
    pub myelith_sitzung: u64,
}

impl Chatantwort {
    /// Baut eine Antwort aus dem, was zurückkam.
    #[allow(clippy::too_many_arguments)]
    pub fn neu(
        sitzung: u64,
        erstellt: u64,
        modell: &str,
        text: String,
        prompt_token: u64,
        neue_token: u64,
        segment: &str,
    ) -> Self {
        Self {
            id: format!("myl-{sitzung}"),
            object: "chat.completion".to_string(),
            created: erstellt,
            model: modell.to_string(),
            choices: vec![Wahl {
                index: 0,
                message: Nachricht {
                    role: "assistant".to_string(),
                    content: text,
                    ..Default::default()
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Verbrauch {
                prompt_tokens: prompt_token,
                completion_tokens: neue_token,
                total_tokens: prompt_token.saturating_add(neue_token),
            },
            myelith_deterministisch: true,
            myelith_segment: segment.to_string(),
            myelith_sitzung: sitzung,
        }
    }

    /// ⚑ **Liest die Werkzeugaufrufe aus der Antwort** (Fund 181): Sie
    /// werden `tool_calls`, der Text ohne sie wird `content`, und
    /// `finish_reason` heisst `tool_calls`. Ohne lesbaren Aufruf bleibt die
    /// Antwort, wie sie ist. Nur aufrufen, wenn Werkzeuge angeboten waren.
    pub fn werkzeugaufrufe_lesen(&mut self, angeboten: &[String]) {
        let sitzung = self.myelith_sitzung;
        let Some(w) = self.choices.first_mut() else {
            return;
        };
        let g = crate::werkzeuge::aufrufe_lesen(&w.message.content, angeboten);
        if g.aufrufe.is_empty() {
            return;
        }
        w.message.content = g.text;
        w.message.tool_calls = Some(
            g.aufrufe
                .into_iter()
                .enumerate()
                .map(|(i, a)| Werkzeugaufruf {
                    id: format!("call_{sitzung}_{i}"),
                    art: art_funktion(),
                    function: Aufruffunktion { name: a.name, arguments: a.argumente },
                })
                .collect(),
        );
        w.finish_reason = "tool_calls".to_string();
    }

    /// Als JSON-Bytes.
    pub fn als_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| b"{}".to_vec())
    }
}

/// Eine Anfrage an [`WEG_TEXT`]: ein fertiger Prompt.
///
/// ⚑ **Nur eine Zeichenkette.** Die OpenAI-Form erlaubt auch eine Liste
/// von Prompts oder Tokennummern; eine Liste hiesse mehrere Aufträge unter
/// einem Beleg, Tokennummern einen Wortschatz auf der Seite des Klienten.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Textanfrage {
    #[serde(default)]
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
}

impl Textanfrage {
    /// Liest eine Anfrage aus dem Rumpf.
    pub fn lesen(rumpf: &[u8]) -> Result<Self, Anfragefehler> {
        let a: Self = serde_json::from_slice(rumpf).map_err(|_| Anfragefehler::Unlesbar)?;
        if a.stream {
            return Err(Anfragefehler::StromVerlangt);
        }
        if a.prompt.trim().is_empty() {
            return Err(Anfragefehler::LeererPrompt);
        }
        Ok(a)
    }

    /// Wie viele neue Token höchstens, mit Vorgabe.
    pub fn token_deckel(&self, vorgabe: u32) -> u32 {
        self.max_tokens.unwrap_or(vorgabe)
    }
}

/// Eine Wahl in der Antwort auf [`WEG_TEXT`].
#[derive(Debug, Clone, Serialize)]
pub struct Textwahl {
    pub index: u32,
    pub text: String,
    pub finish_reason: String,
}

/// Die Antwort auf [`WEG_TEXT`], mit denselben Zusätzen wie
/// [`Chatantwort`].
#[derive(Debug, Clone, Serialize)]
pub struct Textantwort {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Textwahl>,
    pub usage: Verbrauch,
    pub myelith_deterministisch: bool,
    pub myelith_segment: String,
    pub myelith_sitzung: u64,
}

impl Textantwort {
    /// Baut eine Antwort aus dem, was zurückkam.
    #[allow(clippy::too_many_arguments)]
    pub fn neu(
        sitzung: u64,
        erstellt: u64,
        modell: &str,
        text: String,
        prompt_token: u64,
        neue_token: u64,
        segment: &str,
        abschluss: &str,
    ) -> Self {
        Self {
            id: format!("myl-{sitzung}"),
            object: "text_completion".to_string(),
            created: erstellt,
            model: modell.to_string(),
            choices: vec![Textwahl { index: 0, text, finish_reason: abschluss.to_string() }],
            usage: Verbrauch {
                prompt_tokens: prompt_token,
                completion_tokens: neue_token,
                total_tokens: prompt_token.saturating_add(neue_token),
            },
            myelith_deterministisch: true,
            myelith_segment: segment.to_string(),
            myelith_sitzung: sitzung,
        }
    }

    /// Als JSON-Bytes.
    pub fn als_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| b"{}".to_vec())
    }
}

/// **Warum die Antwort endete**: `length`, wenn der Deckel erreicht ist,
/// sonst `stop` (der Pod hielt an einer Endmarke des Modells).
///
/// 📌 Bis zum 2026-11-15 stand hier immer `stop`. Ein Klient, der eine
/// abgeschnittene Antwort fortsetzt, hielt damit jede für vollständig.
pub fn abschluss(neue_token: u64, deckel: u32) -> &'static str {
    if neue_token >= u64::from(deckel) {
        "length"
    } else {
        "stop"
    }
}

/// Ein Eintrag der Modellliste.
#[derive(Debug, Clone, Serialize)]
pub struct Modell {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
    /// ⚑ Steht auch hier, damit ein Klient es vor der ersten Anfrage
    /// sieht und nicht erst danach.
    pub myelith_deterministisch: bool,
    /// Der Pipeline-Stand, für den dieser Name gilt.
    pub myelith_pipeline: String,
}

/// Die Antwort auf [`WEG_MODELLE`].
#[derive(Debug, Clone, Serialize)]
pub struct Modelliste {
    pub object: String,
    pub data: Vec<Modell>,
}

impl Modelliste {
    /// Eine Liste mit genau dem Stand, den dieser Knoten fährt.
    pub fn eine(name: &str, pipeline: &str, erstellt: u64) -> Self {
        Self {
            object: "list".to_string(),
            data: vec![Modell {
                id: name.to_string(),
                object: "model".to_string(),
                created: erstellt,
                owned_by: "myelith".to_string(),
                myelith_deterministisch: true,
                myelith_pipeline: pipeline.to_string(),
            }],
        }
    }

    /// Als JSON-Bytes.
    pub fn als_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| b"{}".to_vec())
    }
}

/// Ein Fehler in der Form, die ein OpenAI-Klient erwartet.
///
/// ⚑ **Die Hülle ist die Form, der Inhalt ist die Wahrheit.** Ein
/// Klient liest `error.message`; wer eine eigene Fehlerform schickt,
/// liefert Harnessen einen leeren Text statt einer Auskunft.
pub fn fehler_json(nachricht: &str, art: &str) -> Vec<u8> {
    let wert = serde_json::json!({
        "error": {
            "message": nachricht,
            "type": art,
            "param": serde_json::Value::Null,
            "code": serde_json::Value::Null,
        }
    });
    serde_json::to_vec(&wert).unwrap_or_else(|_| b"{}".to_vec())
}

/// Wer die Arbeit tatsächlich rechnen lässt.
///
/// # ⚑ Warum das ein Merkmal ist und kein Code in dieser Kiste
///
/// Das Gateway kann **nicht** rechnen lassen. Dazu gehören die
/// Zuteilung (welcher Pod bedient diese Sitzung), der Sitzungskanal
/// (`myl-siegel`) und der Transport (`myl-net`), und alle drei kennt
/// `myl-gateway` nicht. Sie hier hereinzuziehen hiesse, die Tür an den
/// Konsensstapel zu binden, und genau das trennt B6-3.
///
/// **Die Arbeitsteilung ist damit sauber:** Die Tür macht HTTP,
/// Ausweis, Deckel und Beleg; der Aufrufer macht Krypto, Zuteilung und
/// Transport. `myl-node` setzt beides zusammen.
#[async_trait::async_trait]
pub trait Rechenweg: Send + Sync {
    /// Rechnet einen Prompt und gibt zurück, was herauskam.
    ///
    /// [`Rechenfehler::NichtGerechnet`] heisst: nicht gerechnet. ⚑ **Ohne
    /// Grund**, aus demselben Grund wie bei jeder anderen Ablehnung dieser
    /// Tür. [`Rechenfehler::Ungedeckt`] ist kein Zustand der Gegenseite,
    /// sondern eine Aussage über den Auftrag (Fund 711).
    async fn rechne(&self, auftrag: Rechenauftrag<'_>) -> Result<Rechenergebnis, Rechenfehler>;

    /// Wie das Modell heisst und für welchen Stand es gilt.
    ///
    /// ⚑ **`async` und `Option`, weil beides eine Frage nach draussen
    /// ist** (Fund 160). Die erste Fassung war synchron und gab einen
    /// Platzhalter zurück, solange niemand gerechnet hatte: `/v1/models`
    /// meldete dann `"unbekannt"` als Pipeline-Stand. **Ein Harness
    /// fragt die Modelle als Erstes**, also war das der Normalfall und
    /// nicht der Sonderfall.
    ///
    /// `None` heisst: Der Rechenweg kann gerade nicht sagen, was er
    /// bedient. Dann ist `502` die richtige Antwort und keine Liste mit
    /// einem Platzhalter darin.
    async fn modell(&self) -> Option<Modellstand>;
    /// ⚑ **Die Vorlage des Netzmodells** (Fund 181): In ihr setzt die Tür
    /// eine Unterhaltung zusammen. Vorgabe ChatML, die Form der
    /// Qwen3-Familie; der Knoten nennt die seines Netzes.
    fn vorlage(&self) -> myl_types::vorlage::Vorlage {
        myl_types::vorlage::Vorlage::ChatMl
    }
}

/// Was der Rechenweg bekommt.
#[derive(Debug, Clone)]
pub struct Rechenauftrag<'a> {
    /// Die Sitzungsnummer, die die Annahme vergeben hat.
    pub sitzung: u64,
    /// Der Prompt im Klartext.
    ///
    /// ⚑ **Hier ist er noch Klartext, und weiter unten nie wieder.**
    /// Das Versiegeln gehört zum Aufrufer, weil nur er weiss, für wen.
    pub prompt: &'a str,
    /// Wie viele neue Token höchstens.
    pub max_token: u32,
    /// Unter welchem Sitzungskontrakt gefragt wird.
    ///
    /// ⚑ **Ohne sie kann nichts abgebucht werden.** Die Nummer in
    /// [`Self::sitzung`] ist die laufende Zählung der Annahme; welcher
    /// **Kontrakt** dahintersteht, sagt erst die Vollmacht.
    pub sitzung_id: myl_types::ids::SitzungId,
    /// Die Vollmacht, mit der gefragt wurde.
    ///
    /// ⚑ **Sie geht mit, weil nur sie die Abbuchung autorisiert.** Ein
    /// Harness hält einen Bearer und keinen Schlüssel; die Kette
    /// erkennt seit dem 2026-09-03 die Vollmacht des Agenten an, und
    /// wer abbuchen will, muss sie mitreichen können.
    pub vollmacht: myl_types::vollmacht::Vollmacht,
    /// ⚑ **So viele Credits darf die Anfrage höchstens kosten** (Fund 711),
    /// gemessen mit [`myl_types::inferenzauftrag::preis`]: das Grösste, was
    /// Vollmacht, Kontrakt und Guthaben decken. Der Pod hält es ein, und
    /// abgebucht wird nie mehr.
    pub hoechstens: u64,
}

/// Warum ein Rechenweg nicht gerechnet hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechenfehler {
    /// Niemand hat gerechnet; ein neuer Anlauf kann gelingen (502).
    NichtGerechnet,
    /// ⚑ **Schon der Prompt kostet mehr als der Höchstbetrag** (Fund 711):
    /// Der Pod hat die Token gezählt und nicht gerechnet. Ein neuer Anlauf
    /// ändert daran nichts, mehr Guthaben schon (402).
    Ungedeckt {
        /// Wie viele Token der Prompt hatte.
        prompt_token: u64,
    },
}

/// Was zurückkommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rechenergebnis {
    /// Der Text der Antwort.
    pub text: String,
    /// Wie viele Token der Prompt hatte.
    pub prompt_token: u64,
    /// Wie viele neu erzeugt wurden.
    pub neue_token: u64,
    /// Das Segment, unter dem die Arbeit bezeugt wurde, hexadezimal.
    pub segment: String,
}

/// Name und Stand des Modells, das dieser Knoten fährt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modellstand {
    /// Der Name, den ein Klient in `model` schreibt.
    pub name: String,
    /// Der Pipeline-Stand, hexadezimal.
    pub pipeline: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roh(json: &str) -> Result<Chatanfrage, Anfragefehler> {
        Chatanfrage::lesen(json.as_bytes())
    }

    /// ⚑ **Die Form, die jedes Harness schickt, geht durch.**
    #[test]
    fn eine_gewoehnliche_anfrage_geht_durch() {
        let a = roh(r#"{"model":"myelith-qwen","messages":[
            {"role":"system","content":"du bist knapp"},
            {"role":"user","content":"hauptstadt von frankreich?"}],
            "max_tokens":64}"#)
        .expect("lesbar");
        assert_eq!(a.model, "myelith-qwen");
        assert_eq!(a.token_deckel(16), 64);
        // ⚑ In der Vorlage des Netzmodells (Fund 181), nicht `rolle: inhalt`.
        assert_eq!(
            a.prompt_in(myl_types::vorlage::Vorlage::ChatMl),
            "<|im_start|>system\ndu bist knapp<|im_end|>\n<|im_start|>user\nhauptstadt von frankreich?<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    /// ⚑ **Unbekannte Felder fallen, und `temperature` ist eines
    /// davon.** Ein Harness, das sie immer mitschickt, darf nicht
    /// scheitern; ein Feld im Typ sähe dagegen aus wie eine Einstellung.
    #[test]
    fn temperatur_und_verwandte_stoeren_nicht() {
        let a = roh(r#"{"messages":[{"role":"user","content":"x"}],
            "temperature":0.9,"top_p":0.1,"seed":42,"presence_penalty":1.5}"#)
        .expect("lesbar");
        assert_eq!(a.prompt_in(myl_types::vorlage::Vorlage::ChatMl), myl_types::vorlage::Vorlage::ChatMl.bauen(&[("user", "x")], false));
    }

    /// ⚑ **Werkzeuge werden in der Systemnachricht angesagt** (Fund 181),
    /// nach deren Text; Aufruf und Werkzeugantworten des Verlaufs stehen in
    /// der Form der Vorlage, zwei Antworten hintereinander in einer Runde.
    #[test]
    fn werkzeuge_und_verlauf_stehen_in_der_vorlage() {
        use crate::werkzeuge::{antwort_als_text, aufruf_als_text};
        let a = roh(r#"{"messages":[
            {"role":"system","content":"S"},
            {"role":"user","content":"rechne"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"rechnen","arguments":"{\"x\":1}"}}]},
            {"role":"tool","tool_call_id":"c1","content":"A"},
            {"role":"tool","tool_call_id":"c1","content":"B"}],
            "tools":[{"type":"function","function":{"name":"rechnen","description":"rechnet","parameters":{"type":"object"}}}]}"#)
        .expect("lesbar");
        assert!(a.werkzeuge_angeboten());
        let p = a.prompt_in(myl_types::vorlage::Vorlage::ChatMlDenkblock);
        let ansage = crate::werkzeuge::werkzeuge_ansagen(&[crate::werkzeuge::Werkzeugangebot {
            name: "rechnen".into(),
            beschreibung: "rechnet".into(),
            parameter: serde_json::json!({"type": "object"}),
        }]);
        let soll = myl_types::vorlage::Vorlage::ChatMlDenkblock.bauen(
            &[
                ("system", &format!("S\n\n{ansage}")),
                ("user", "rechne"),
                ("assistant", &aufruf_als_text("rechnen", &serde_json::json!({"x": 1}))),
                ("user", &format!("{}\n{}", antwort_als_text("A"), antwort_als_text("B"))),
            ],
            false,
        );
        assert_eq!(p, soll);
        // Gegenprobe: Mit `tool_choice: none` wird nichts angesagt.
        let ohne = roh(r#"{"messages":[{"role":"user","content":"x"}],
            "tools":[{"type":"function","function":{"name":"rechnen"}}],"tool_choice":"none"}"#)
        .expect("lesbar");
        assert!(!ohne.werkzeuge_angeboten());
        assert!(!ohne.prompt_in(myl_types::vorlage::Vorlage::ChatMl).contains("<tools>"));
    }

    /// ⚑ **Was die Tür nicht erzwingen kann, lehnt sie ab**, und ein
    /// Werkzeug ohne Namen ist keines.
    #[test]
    fn unerfuellbare_werkzeugwahl_wird_abgelehnt() {
        assert_eq!(
            roh(r#"{"messages":[{"role":"user","content":"x"}],"tools":[{"type":"function","function":{"name":"a"}}],"tool_choice":"required"}"#),
            Err(Anfragefehler::WerkzeugwahlNichtErfuellbar)
        );
        assert_eq!(
            roh(r#"{"messages":[{"role":"user","content":"x"}],"tools":[{"type":"function","function":{"name":" "}}]}"#),
            Err(Anfragefehler::WerkzeugUnlesbar)
        );
    }

    /// ⚑ **Aus `<tool_call>` werden `tool_calls`**, mit `finish_reason`
    /// `tool_calls` und dem Text ohne die Aufrufe; ohne Aufruf bleibt alles.
    #[test]
    fn aufrufe_der_antwort_werden_tool_calls() {
        let mut a = Chatantwort::neu(7, 1, "m", "Moment. <tool_call>\n{\"name\": \"rechnen\", \"arguments\": {\"x\": 1}}\n</tool_call>".into(), 3, 4, "ab");
        a.werkzeugaufrufe_lesen(&["rechnen".to_string()]);
        let w = &a.choices[0];
        assert_eq!(w.finish_reason, "tool_calls");
        assert_eq!(w.message.content, "Moment.");
        let aufrufe = w.message.tool_calls.as_ref().expect("Aufrufe");
        assert_eq!(aufrufe[0].id, "call_7_0");
        assert_eq!(aufrufe[0].function.name, "rechnen");
        assert_eq!(aufrufe[0].function.arguments, r#"{"x":1}"#);
        let json = String::from_utf8(a.als_json()).expect("utf8");
        assert!(json.contains(r#""tool_calls":[{"id":"call_7_0","type":"function","function":{"name":"rechnen","arguments":"{\"x\":1}"}}]"#), "{json}");
        // Gegenprobe: ohne Aufruf keine `tool_calls`, und das Feld fehlt ganz.
        let mut b = Chatantwort::neu(8, 1, "m", "nur Text".into(), 1, 1, "ab");
        b.werkzeugaufrufe_lesen(&["rechnen".to_string()]);
        assert_eq!(b.choices[0].finish_reason, "stop");
        assert!(!String::from_utf8(b.als_json()).expect("utf8").contains("tool_calls"));
    }

    /// ⚑ **`max_completion_tokens` schlägt `max_tokens`.**
    #[test]
    fn der_neuere_deckel_gewinnt() {
        let a = roh(r#"{"messages":[{"role":"user","content":"x"}],
            "max_tokens":10,"max_completion_tokens":20}"#)
        .expect("lesbar");
        assert_eq!(a.token_deckel(5), 20);
        // Gegenprobe: ohne den neueren gilt der alte, ohne beide die Vorgabe.
        let b = roh(r#"{"messages":[{"role":"user","content":"x"}],"max_tokens":10}"#)
            .expect("lesbar");
        assert_eq!(b.token_deckel(5), 10);
        let c = roh(r#"{"messages":[{"role":"user","content":"x"}]}"#).expect("lesbar");
        assert_eq!(c.token_deckel(5), 5);
    }

    /// ⚑ **`stream: true` wird abgelehnt und nicht still ignoriert.**
    /// Ein Klient, der einen Strom erwartet und eine ganze Antwort
    /// bekommt, hängt in seiner Leseschleife.
    #[test]
    fn ein_strom_wird_mit_grund_abgelehnt() {
        assert_eq!(
            roh(r#"{"messages":[{"role":"user","content":"x"}],"stream":true}"#),
            Err(Anfragefehler::StromVerlangt)
        );
        // Gegenprobe: ohne das Feld und mit false geht dieselbe Anfrage.
        assert!(roh(r#"{"messages":[{"role":"user","content":"x"}],"stream":false}"#).is_ok());
    }

    /// Jeder Deckel und jede Leerform greift.
    #[test]
    fn die_deckel_greifen() {
        assert_eq!(roh(r#"{"messages":[]}"#), Err(Anfragefehler::Leer));
        assert_eq!(
            roh(r#"{"messages":[{"role":"user","content":"   "}]}"#),
            Err(Anfragefehler::LeererPrompt)
        );
        assert_eq!(roh("kein json"), Err(Anfragefehler::Unlesbar));
        assert_eq!(roh(r#"{"messages":"kein feld"}"#), Err(Anfragefehler::Unlesbar));
        // Bildteile statt Text: abgelehnt und nicht halb verstanden.
        assert_eq!(
            roh(r#"{"messages":[{"role":"user","content":[{"type":"image_url"}]}]}"#),
            Err(Anfragefehler::Unlesbar)
        );
        let viele: Vec<String> = (0..MAX_NACHRICHTEN + 1)
            .map(|_| r#"{"role":"user","content":"x"}"#.to_string())
            .collect();
        assert_eq!(
            roh(&format!("{{\"messages\":[{}]}}", viele.join(","))),
            Err(Anfragefehler::ZuVieleNachrichten {
                anzahl: MAX_NACHRICHTEN + 1
            })
        );
    }

    /// ⚑ **Die Antwort nennt, dass sie deterministisch ist**, und
    /// trägt den Faden zur bezeugten Arbeit.
    #[test]
    fn die_antwort_traegt_den_hinweis_und_das_segment() {
        let a = Chatantwort::neu(7, 1_700_000_000, "myelith-qwen", "Paris".into(), 12, 3, "ab12");
        let j = String::from_utf8(a.als_json()).expect("utf8");
        assert!(j.contains("\"myelith_deterministisch\":true"), "{j}");
        assert!(j.contains("\"myelith_segment\":\"ab12\""), "{j}");
        assert!(j.contains("\"total_tokens\":15"), "{j}");
        assert!(j.contains("\"object\":\"chat.completion\""), "{j}");
        assert!(j.contains("\"finish_reason\":\"stop\""), "{j}");
    }

    /// Die Modellliste sagt es vor der ersten Anfrage.
    #[test]
    fn die_modellliste_nennt_den_stand() {
        let l = Modelliste::eine("myelith-qwen", "ab12cd", 1_700_000_000);
        let j = String::from_utf8(l.als_json()).expect("utf8");
        assert!(j.contains("\"object\":\"list\""), "{j}");
        assert!(j.contains("\"id\":\"myelith-qwen\""), "{j}");
        assert!(j.contains("\"myelith_pipeline\":\"ab12cd\""), "{j}");
        assert!(j.contains("\"myelith_deterministisch\":true"), "{j}");
    }

    /// ⚑ **Ein Fehler kommt in der Form, die ein Klient liest.**
    #[test]
    fn ein_fehler_traegt_die_erwartete_huelle() {
        let j = String::from_utf8(fehler_json("so nicht", "invalid_request_error"))
            .expect("utf8");
        assert!(j.contains("\"message\":\"so nicht\""), "{j}");
        assert!(j.contains("\"type\":\"invalid_request_error\""), "{j}");
        assert!(j.contains("\"param\":null"), "{j}");
    }

    #[test]
    fn ein_fertiger_prompt_geht_unveraendert_durch() {
        let a = Textanfrage::lesen(br#"{"model":"m","prompt":"<|im_start|>user\nhallo<|im_end|>\n","max_tokens":7,"temperature":0.3}"#)
            .expect("lesbar");
        assert_eq!(a.prompt, "<|im_start|>user\nhallo<|im_end|>\n");
        assert_eq!(a.token_deckel(256), 7);
        assert_eq!(Textanfrage::lesen(br#"{"prompt":"x"}"#).expect("lesbar").token_deckel(256), 256);
    }

    #[test]
    fn ein_fertiger_prompt_wird_wie_ein_chat_abgelehnt() {
        assert_eq!(Textanfrage::lesen(br#"{"prompt":"x","stream":true}"#), Err(Anfragefehler::StromVerlangt));
        assert_eq!(Textanfrage::lesen(br#"{"prompt":"   "}"#), Err(Anfragefehler::LeererPrompt));
        assert_eq!(Textanfrage::lesen(br#"{"prompt":["a","b"]}"#), Err(Anfragefehler::Unlesbar));
        assert_eq!(Textanfrage::lesen(br#"{"messages":[{"role":"user","content":"x"}]}"#), Err(Anfragefehler::Unlesbar));
    }

    #[test]
    fn die_textantwort_traegt_text_abschluss_und_segment() {
        let a = Textantwort::neu(9, 1, "m", "Paris".into(), 5, 2, "ab", "stop");
        let v: serde_json::Value = serde_json::from_slice(&a.als_json()).expect("json");
        assert_eq!(v["object"], "text_completion");
        assert_eq!(v["choices"][0]["text"], "Paris");
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["total_tokens"], 7);
        assert_eq!(v["myelith_segment"], "ab");
    }

    #[test]
    fn der_abschluss_sagt_ob_der_deckel_griff() {
        assert_eq!(abschluss(3, 4), "stop");
        assert_eq!(abschluss(4, 4), "length");
        assert_eq!(abschluss(0, 1), "stop");
    }
}
