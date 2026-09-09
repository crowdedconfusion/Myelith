//! Ein [`Modellweg`], der das Modell auf **derselben Maschine** rechnet.
//!
//! # ⚑ Damit laeuft der Agent ohne Netz
//!
//! Bis zum 2026-09-08 brauchte die Agentenschleife die Tuer eines
//! Knotens. Sie braucht sie weiterhin, wenn sie bezahlte, geprüfte
//! Arbeit will; fuer den eigenen Rechner genuegt dieses hier.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::generate::generate;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_local_agent::{Antwort, Modellweg, Nachricht, Tuerfehler};

/// Welche Form ein Artefakt versteht.
///
/// # ⛑ Warum das nicht der Client entscheidet (2026-09-08)
///
/// Der erste Anlauf setzte ChatML fuer alles, mit der Begruendung, es
/// sei „die richtige" Vorlage. **Der Ende-zu-Ende-Test hat das
/// widerlegt:** Qwen2.5-0,5B echote die Frage und lieferte Unsinn.
///
/// ⚑ **Der Grund stand im Modellkatalog.** Die Artefakte stammen aus
/// `Qwen/Qwen2.5-0.5B` und `Qwen/Qwen2.5-7B`, also aus den
/// **Basis**repositorien. Ein Basismodell ist auf Textfortsetzung
/// trainiert und hat Rollenmarken nie gesehen; es setzt sie als
/// gewoehnlichen Text fort. Bei **Qwen3** ist es umgekehrt: Dort ist
/// die Fassung ohne Zusatz die instruktionsgeschliffene, das
/// Basismodell hiesse `-Base`.
///
/// ⚑ **Deshalb haengt die Vorlage am Artefakt und nicht an einer
/// Einstellung.** Eine Einstellung kann jemand falsch setzen; eine
/// Eigenschaft des Modells nicht.
///
/// ⛑ **Und eine Luecke, die offen bleibt:** Kaeme eine Instruct-Fassung
/// von Qwen2.5 dazu, traeffe die Ableitung aus der Familie das Falsche.
/// Dann braucht der Katalog ein eigenes Feld. Bis dahin ist die
/// Herleitung richtig und die Grenze benannt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vorlage {
    /// Rollenmarken, wie ein instruktionsgeschliffenes Modell sie kennt.
    ChatMl,
    /// Schlichte Fortsetzung, wie ein Basismodell sie erwartet.
    Fortsetzung,
}

impl Vorlage {
    /// Leitet die Vorlage aus der Modellfamilie ab.
    pub fn fuer_familie(familie: &str) -> Self {
        match familie {
            "qwen3" | "qwen3-moe" => Self::ChatMl,
            _ => Self::Fortsetzung,
        }
    }

    /// Setzt die Unterhaltung zusammen.
    /// Setzt die Unterhaltung zusammen.
    ///
    /// ⚑ **`denken` entscheidet ueber den Denkmodus (2026-09-08).**
    /// Qwen3 beginnt seine Antwort von sich aus mit einem
    /// `<think>`-Block. Der erste Ende-zu-Ende-Lauf lieferte deshalb
    /// „Okay, the user is asking for the capital of France..." und kam
    /// bei vierundzwanzig Token nie zur Antwort.
    ///
    /// ⛑ **Fuer einen Agenten ist das schaedlich und faellt nicht
    /// auf:** Er erwartet einen Werkzeugaufruf und bekommt Ueberlegung,
    /// und niemand sieht dem Fehlschlag an, warum. Ein **leerer**
    /// Denkblock im Voraus sagt dem Modell, dass die Ueberlegung schon
    /// stattgefunden hat, und es geht unmittelbar zur Antwort ueber.
    pub fn bauen(self, nachrichten: &[Nachricht], denken: bool) -> String {
        match self {
            Self::ChatMl => {
                let mut aus = String::new();
                for n in nachrichten {
                    aus.push_str("<|im_start|>");
                    aus.push_str(&n.role);
                    aus.push('\n');
                    aus.push_str(&n.content);
                    aus.push_str("<|im_end|>\n");
                }
                // ⚑ Die offene Marke sagt dem Modell, dass jetzt es
                // dran ist. Ohne sie setzt es die Nutzerseite fort.
                aus.push_str("<|im_start|>assistant\n");
                if !denken {
                    aus.push_str("<think>\n\n</think>\n\n");
                }
                aus
            }
            // ⚑ **Fuer ein Basismodell zaehlt, was wie Text aussieht.**
            // Am Pruefstand vom 2026-09-07 beantwortete Qwen2.5-0,5B
            // vier von vier Fortsetzungsfragen und nur eine von zwei im
            // Frage-Antwort-Schema. Die Fortsetzung ist damit nicht die
            // Notloesung, sondern die passende Form.
            Self::Fortsetzung => {
                let mut aus = String::new();
                for n in nachrichten {
                    match n.role.as_str() {
                        "system" => {
                            aus.push_str(&n.content);
                            aus.push_str("\n\n");
                        }
                        "assistant" => {
                            aus.push_str("Antwort: ");
                            aus.push_str(&n.content);
                            aus.push('\n');
                        }
                        _ => {
                            aus.push_str("Frage: ");
                            aus.push_str(&n.content);
                            aus.push('\n');
                        }
                    }
                }
                aus.push_str("Antwort:");
                aus
            }
        }
    }
}

/// Das Modell im eigenen Speicher.
pub struct Oertlichesmodell {
    modell: Arc<IntegerModel>,
    wortschatz: Tokenizer,
    /// Die Familie aus `model_config.json`, aus der die Vorlage folgt.
    familie: String,
    /// Wie viele Token eine Antwort hoechstens hat, wenn der Aufrufer
    /// nichts sagt.
    pub grenze: usize,
    /// ⚑ **Gierig als Vorgabe.** Ein Agent, der Werkzeuge ruft, soll
    /// reproduzierbar rufen; Ziehen mit Zufall macht denselben Plan
    /// zweimal verschieden und einen Fehlschlag unauffindbar.
    pub gierig: bool,
    /// Die Saat fuer den Fall, dass jemand doch ziehen will.
    pub saat: u64,
    /// ⚑ **Denkmodus, standardmaessig AUS.** Ein Harness will
    /// Werkzeugaufrufe, keine Ueberlegung; und jedes Denktoken kostet
    /// dieselbe Rechenzeit wie ein Antworttoken.
    pub denken: bool,
}

impl Oertlichesmodell {
    /// Laedt ein Artefaktverzeichnis.
    pub fn laden(verzeichnis: &str) -> Result<Self, String> {
        let modell = load_model(Path::new(verzeichnis)).map_err(|e| format!("Modell: {e}"))?;
        let wortschatz = Tokenizer::from_file(
            &format!("{verzeichnis}/tokenizer.json"),
        )
        .map_err(|e| format!("Wortschatz: {e}"))?;
        // ⚑ Die Familie steht im Artefakt und wird nicht geraten.
        let roh = std::fs::read_to_string(format!("{verzeichnis}/model_config.json"))
            .map_err(|e| format!("model_config.json: {e}"))?;
        let konf: serde_json::Value =
            serde_json::from_str(&roh).map_err(|e| format!("model_config.json: {e}"))?;
        let familie = konf
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(Self {
            modell: Arc::new(modell),
            wortschatz,
            familie,
            grenze: 512,
            gierig: true,
            saat: 0,
            denken: false,
        })
    }

    /// Welche Vorlage dieses Artefakt versteht.
    pub fn vorlage(&self) -> Vorlage {
        Vorlage::fuer_familie(&self.familie)
    }

    /// ⚑ **ChatML, und das weicht bewusst vom Netzweg ab.**
    ///
    /// Der Gateway setzt eine Unterhaltung heute als `role: content`
    /// zusammen. **Fund 181 haelt fest, dass Qwen2.5 und Qwen3 auf
    /// ChatML trainiert sind** (`<|im_start|>rolle\ninhalt<|im_end|>`)
    /// und diese Form damit eine ist, die das Modell **nie gesehen
    /// hat**. Genau die Werkzeugfaehigkeit haengt an der Vorlage, und
    /// ein Harness lebt von ihr.
    ///
    /// ⛑ **Dort wurde es nicht geaendert, und das aus gutem Grund:** Die
    /// Vorlage bestimmt die Token, die Token bestimmen die E2E-Vektoren,
    /// und die stehen im Konformitaetswert. Eine Aenderung ist eine
    /// Entscheidung ueber den numerischen Vertrag.
    ///
    /// ⚑ **Hier gilt das nicht.** Der lokale Weg ist keine bezahlte,
    /// geprüfte Arbeit und steht in keinem Vertrag. Er benutzt deshalb
    /// die **richtige** Vorlage und ist damit zugleich der Pruefstand,
    /// an dem sich zeigt, ob ChatML die Werkzeugfaehigkeit hebt, **bevor**
    /// jemand den Konformitaetswert dafuer bewegt.
    pub fn chatml(nachrichten: &[Nachricht]) -> String {
        let mut aus = String::new();
        for n in nachrichten {
            aus.push_str("<|im_start|>");
            aus.push_str(&n.role);
            aus.push('\n');
            aus.push_str(&n.content);
            aus.push_str("<|im_end|>\n");
        }
        // Die offene Marke sagt dem Modell, dass jetzt es dran ist.
        aus.push_str("<|im_start|>assistant\n");
        aus
    }
}

impl Modellweg for Oertlichesmodell {
    fn chat(
        &self,
        _modell: &str,
        nachrichten: &[Nachricht],
        max_tokens: Option<u32>,
    ) -> Result<Antwort, Tuerfehler> {
        let prompt = self.vorlage().bauen(nachrichten, self.denken);
        let hinein = self.wortschatz.encode(&prompt).len();
        let grenze = max_tokens.map(|m| m as usize).unwrap_or(self.grenze);
        let token = generate(
            &self.modell,
            &self.wortschatz,
            &prompt,
            grenze,
            self.saat,
            self.gierig,
        );
        let text = self.wortschatz.decode(&token);
        // ⚑ Die Endmarke gehoert nicht in die Antwort; sie ist Rahmen
        // und nicht Inhalt.
        // ⚑ Rahmen ist nicht Inhalt: die Endmarke des einen Weges und
        // die naechste Frage des anderen gehoeren nicht in die Antwort.
        let text = text
            .split("<|im_end|>")
            .next()
            .unwrap_or(&text)
            .split("\nFrage:")
            .next()
            .unwrap_or(&text)
            .trim()
            .to_string();
        Ok(Antwort {
            text,
            abschlussgrund: Some("stop".to_string()),
            // ⛑ **Keine Kennung und kein Segment, und das mit Absicht.**
            // Beides sind Belege der Kette. Lokal gerechnete Arbeit hat
            // keinen, und einen zu erfinden waere schlimmer als keiner:
            // Er saehe aus wie ein Nachweis.
            kennung: String::new(),
            segment: None,
            prompt_token: hinein as u32,
            antwort_token: token.len() as u32,
        })
    }
}

#[cfg(test)]
mod teilbarkeit {
    use super::*;

    /// ⚑ **Woran Punkt 0.5 haengt, und zwar ganz.**
    ///
    /// Unteragenten auf freigegebenen Rechenwerken sind nur dann
    /// bezahlbar, wenn sie sich **ein** geladenes Modell teilen. Ein
    /// 4B-Artefakt je Unteragent waere kein Nebenlaeufigkeitsentwurf,
    /// sondern eine Speichersperre: Drei Agenten braechten drei
    /// Kopien der Gewichte.
    ///
    /// Die Voraussetzung dafuer ist, dass [`Oertlichesmodell`] geteilt
    /// werden darf: [`Modellweg::chat`] nimmt `&self`, die Gewichte
    /// liegen hinter einem `Arc`, und der Rechenpfad haelt keinen
    /// veraenderlichen Zustand.
    ///
    /// ⚑ **Deshalb steht das hier als Uebersetzungsfehler und nicht
    /// als Absicht in einem Kommentar.** Wer spaeter ein `RefCell` oder
    /// einen Zwischenspeicher in das Modell legt, faellt hier auf und
    /// nicht erst beim Bau der Unteragenten.
    #[test]
    fn das_modell_laesst_sich_ueber_faeden_teilen() {
        fn nur_geteilt<T: Send + Sync>() {}
        nur_geteilt::<Oertlichesmodell>();
    }

    /// Und dasselbe fuer den Weg, ueber den das Harness es sieht.
    #[test]
    fn auch_als_modellweg_teilbar() {
        fn nur_geteilt<T: Modellweg + Send + Sync>() {}
        nur_geteilt::<Oertlichesmodell>();
    }
}
