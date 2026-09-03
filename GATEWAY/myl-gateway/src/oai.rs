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

/// Höchstzahl von Nachrichten in einer Anfrage.
///
/// ⚑ **Ein Deckel vor der Arbeit.** Ohne ihn bestimmt der Anfragende,
/// wie viele Zeichenketten das Gateway zusammensetzt, und `MAX_RUMPF`
/// allein liesse Hunderttausende leerer Nachrichten zu.
pub const MAX_NACHRICHTEN: usize = 1024;

/// Eine Nachricht der Unterhaltung.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Nachricht {
    pub role: String,
    /// ⚑ **Nur Text.** Die OpenAI-Form erlaubt hier auch eine Liste von
    /// Teilen (Bilder, Ton). Myelith rechnet Text; eine Anfrage mit
    /// Bildteilen wird abgelehnt und nicht halb verstanden.
    pub content: String,
}

/// Eine Anfrage an [`WEG_CHAT`].
///
/// ⚑ **Nur die Felder, die etwas bewirken.** `temperature` und
/// Verwandte stehen bewusst **nicht** hier: Ein Feld, das der Typ
/// trägt und das Programm ignoriert, sieht aus wie eine Einstellung.
/// `serde` lässt unbekannte Felder fallen, und genau das ist hier
/// richtig.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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

    /// Setzt die Unterhaltung zu einem Prompt zusammen.
    ///
    /// ⚑ **Die Rolle geht mit.** Ohne sie wäre „du bist ein Assistent"
    /// von der Frage des Nutzers nicht zu unterscheiden, und ein Modell,
    /// das den Unterschied nicht sieht, folgt der falschen Hälfte.
    pub fn prompt(&self) -> String {
        let mut aus = String::new();
        for m in &self.messages {
            aus.push_str(&m.role);
            aus.push_str(": ");
            aus.push_str(&m.content);
            aus.push('\n');
        }
        aus
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

    /// Als JSON-Bytes.
    pub fn als_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| b"{}".to_vec())
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
    /// `None` heisst: nicht gerechnet. ⚑ **Ohne Grund**, aus demselben
    /// Grund wie bei jeder anderen Ablehnung dieser Tür.
    async fn rechne(&self, auftrag: Rechenauftrag<'_>) -> Option<Rechenergebnis>;

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
        assert_eq!(
            a.prompt(),
            "system: du bist knapp\nuser: hauptstadt von frankreich?\n"
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
        assert_eq!(a.prompt(), "user: x\n");
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
}
