//! Der Auftrag an einen Pod und was zurückkommt (GATEWAY Stufe 4).
//!
//! # ⚑ Warum es diesen Typ bis zum 2026-09-03 nicht gab
//!
//! **Ein Pod bekam seine Arbeit von der Kommandozeile.**
//! `myl-pod-node --prompt "<text>"`. Es gab kein Gossip-Thema für
//! Anfragen, `myl-pod` kannte `myl-net` nicht, `myl-node` kannte
//! `myl-pod` nicht: **kein Weg, einem Pod über das Netz etwas zu
//! geben**, auf keiner der beiden Seiten.
//!
//! Damit führte auch die fertige Tür des Gateways nirgendwohin, und
//! deshalb wurde am 2026-09-03 entschieden, Stufe 4 vor Stufe 3 zu
//! bauen: erst das Zimmer, dann die Adresse.
//!
//! # Warum der Typ hier steht
//!
//! Ihn brauchen **beide Seiten**: das Gateway, das ihn stellt, und der
//! Pod, der ihn annimmt. Die beiden Kisten kennen einander nicht und
//! sollen es nicht; ein gemeinsamer Typ gehört dorthin, wo beide
//! hinsehen. Dieselbe Begründung wie bei
//! [`crate::poi_botschaft`] (Fund 144).
//!
//! # ⚑ Die Bindung geht mit, und ohne sie wäre der Auftrag wertlos
//!
//! [`Anfragebindung`] bindet Sitzung, Prompt und Epoche. **Ohne sie
//! rechnete der Pod etwas, und niemand könnte später zeigen, dass
//! genau diese Anfrage es ausgelöst hat.** Am 2026-09-01 fiel schon
//! einmal auf, dass der Prompt im Konsens nicht vorkam; das ist
//! dieselbe Stelle, eine Schicht weiter.
//!
//! # ⚑ Der Prompt ist versiegelt, und die Bindung bindet den Klartext
//!
//! Der Koordinator ist ein Fremder. Er bekommt den Prompt versiegelt
//! (`myl_net::sitzung`), entsiegelt ihn, **und kann die Bindung dann
//! selbst prüfen**: Sie passt zum Klartext, den er in Händen hält.
//!
//! **Beides zusammen ist die Aussage:** Der Weg trägt keinen Klartext,
//! und der Empfänger kann trotzdem feststellen, ob er das bekommen hat,
//! was der Auftrag behauptet.
//!
//! # ⚑ Zwei Deckel, und beide gehören vor die Arbeit
//!
//! `max_token` und die Prompt-Länge sind begrenzt. Ohne sie bestimmt
//! der Anfragende, wie lange ein Pod rechnet: ein paar hundert Bytes
//! hinein, Minuten hinaus. **Dieselbe Klasse wie Fund 141**, und
//! dieselbe Antwort: Der Deckel steht vor der teuren Arbeit und nicht
//! dahinter.

use crate::hash::Hash;
use crate::ids::SegmentId;
use crate::sitzung::Anfragebindung;

/// Höchstlänge des versiegelten Prompts, in Bytes.
///
/// ⚑ **Gerechnet, nicht gegriffen.** Ein Megabyte Klartext sind rund
/// 250 000 Token, weit über jedem Kontextfenster, das dieses Projekt
/// fährt; das Siegel legt einen festen Vorspann darauf. Wer mehr
/// schickt, schickt keinen Prompt. Dieselbe Schranke wie am Gateway
/// (`myl_gateway::MAX_RUMPF`), damit nicht zwei Stellen verschieden
/// entscheiden.
pub const MAX_PROMPT_BYTES: usize = 1024 * 1024 + 4096;

/// ⚑ **Der Auftragsdeckel muss in die Leitungsgrenze passen**, sonst
/// wäre ein formgültiger Auftrag unzustellbar, und zwar **erst auf der
/// Leitung** und für den Absender unsichtbar.
///
/// Das stand bis zum 2026-09-03 als Test in `myl-node`, weil nur dort
/// beide Zahlen sichtbar waren. Seit die Leitungsgrenze in
/// [`crate::protocol`] wohnt (Fund 155), ist es eine Zusicherung des
/// Übersetzers: Sie kann nicht übersehen, nicht gefiltert und nicht
/// vergessen werden. Der Zuschlag deckt Bindung, Sitzung, Pipeline und
/// den Borsh-Rahmen.
const _: () = assert!(
    MAX_PROMPT_BYTES + 64 * 1024 < crate::protocol::MAX_ANFRAGE_BYTES,
    "der Promptdeckel passt nicht mit Rahmen in die Leitungsgrenze"
);

/// Höchstzahl neuer Token je Auftrag.
///
/// ⚑ **Die Schranke gegen den unbegrenzten Auftrag.** Ein Pod rechnet
/// je Token einen vollen Durchlauf durch alle Shards; ohne Deckel
/// bestimmt der Anfragende die Rechenzeit. Vier Kilotoken sind eine
/// lange Antwort und eine kurze Ewigkeit.
pub const MAX_NEUE_TOKEN: u32 = 4096;

/// Was an einem Auftrag nicht stimmt.
///
/// ⚑ **Jeder Fall ist eine eigene Aussage.** „Zu lang" und „ohne
/// Bindung" haben verschiedene Ursachen; sie zu bündeln hiesse, dem
/// Absender das Raten zu überlassen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auftragsfehler {
    /// Ein Auftrag ohne Prompt ist keiner.
    Leer,
    /// Der versiegelte Prompt überschreitet [`MAX_PROMPT_BYTES`].
    PromptZuGross { bytes: usize },
    /// `max_token` ist null: ein Auftrag, der nichts verlangt.
    NullToken,
    /// `max_token` überschreitet [`MAX_NEUE_TOKEN`].
    ZuVieleToken { verlangt: u32 },
}

impl std::fmt::Display for Auftragsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leer => f.write_str("ein Auftrag ohne Prompt ist keiner"),
            Self::PromptZuGross { bytes } => write!(
                f,
                "versiegelter Prompt {bytes} Bytes, erlaubt sind {MAX_PROMPT_BYTES}"
            ),
            Self::NullToken => f.write_str("ein Auftrag ueber null Token verlangt nichts"),
            Self::ZuVieleToken { verlangt } => write!(
                f,
                "{verlangt} Token verlangt, erlaubt sind {MAX_NEUE_TOKEN}"
            ),
        }
    }
}

impl std::error::Error for Auftragsfehler {}

/// Ein Inferenzauftrag an einen Pod.
///
/// **Additiv angehängt, nie eingefügt:** Die Feldreihenfolge ist
/// Protokollvertrag.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Inferenzauftrag {
    /// Die Sitzungsnummer, die das Gateway vergeben hat.
    pub sitzung: u64,
    /// Die Bindung der Anfrage: Sitzung, Prompt-Hash, Epoche.
    ///
    /// ⚑ **Sie bindet den Klartext**, den der Empfänger erst nach dem
    /// Entsiegeln sieht. Genau dann kann er sie prüfen, und genau dann
    /// ist sie etwas wert.
    pub bindung: Anfragebindung,
    /// Der Prompt, versiegelt für den Koordinator.
    pub prompt_versiegelt: Vec<u8>,
    /// Wie viele neue Token höchstens.
    pub max_token: u32,
    /// Für welchen Pipeline-Stand der Auftrag gilt.
    ///
    /// ⚑ **Ein Auftrag gehört zu einem Modell.** Ohne dieses Feld
    /// rechnete ein Pod mit dem Stand, den er zufällig geladen hat, und
    /// zwei Pods derselben Redundanzpaarung könnten verschiedene
    /// rechnen: Dann verglichen sie zwei richtige Ergebnisse
    /// verschiedener Modelle und meldeten einen Streit.
    pub pipeline: Hash,
}

impl Inferenzauftrag {
    /// Prüft, was sich ohne den Schlüssel prüfen lässt.
    ///
    /// ⚑ **Vor dem Entsiegeln zu rufen.** Ein Auftrag, der die Deckel
    /// verletzt, soll keine Entsiegelung kosten; das ist der Sinn eines
    /// Deckels vor der Arbeit.
    ///
    /// **Was hier nicht geprüft wird:** ob die Bindung zum Prompt
    /// passt. Das geht erst nach dem Entsiegeln, und dafür gibt es
    /// [`Anfragebindung::passt_zu_sitzung`].
    pub fn pruefe_form(&self) -> Result<(), Auftragsfehler> {
        if self.prompt_versiegelt.is_empty() {
            return Err(Auftragsfehler::Leer);
        }
        if self.prompt_versiegelt.len() > MAX_PROMPT_BYTES {
            return Err(Auftragsfehler::PromptZuGross {
                bytes: self.prompt_versiegelt.len(),
            });
        }
        if self.max_token == 0 {
            return Err(Auftragsfehler::NullToken);
        }
        if self.max_token > MAX_NEUE_TOKEN {
            return Err(Auftragsfehler::ZuVieleToken {
                verlangt: self.max_token,
            });
        }
        Ok(())
    }
}

/// Was auf einen Auftrag zurückkommt.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum Inferenzantwort {
    /// Die erzeugten Token und das Segment, das dabei entstand.
    ///
    /// ⚑ **Die Segmentkennung gehört dazu.** Sie ist der Faden vom
    /// Beleg des Nutzers zur bezeugten Arbeit des Pods: Ohne sie hätte
    /// der Nutzer Token und keine Möglichkeit, sie einer Abrechnung
    /// zuzuordnen.
    Ergebnis {
        /// Die Sitzung, zu der die Antwort gehört.
        sitzung: u64,
        /// Die erzeugten Token.
        token: Vec<u32>,
        /// Das Segment, das dabei entstand.
        segment: SegmentId,
        /// Wie viele Token der Prompt hatte.
        ///
        /// ⚑ **Gezählt vom Rechnenden, weil nur er zählen kann.** Der
        /// Wortschatz liegt bei den Artefakten. Bis zum 2026-09-03 gab
        /// die Tür stattdessen die **Byte-Länge** des Prompts als
        /// `usage.prompt_tokens` aus, und das ist ein Feld mit
        /// festgelegter Bedeutung: Ein Klient rechnet damit Kosten.
        prompt_token: u64,
        /// Die Token als Text.
        ///
        /// ⚑ **Angehängt und nicht eingefügt**, die Feldreihenfolge ist
        /// Protokollvertrag.
        ///
        /// ⚑ **Gerendert vom Rechnenden, weil nur er es kann.** Der
        /// Wortschatz liegt bei den Artefakten; ein Knoten, der
        /// Token in Text verwandeln wollte, müsste die Ganzzahl-Laufzeit
        /// mitbauen, und ein Gateway ebenso.
        ///
        /// ⚑ **Und deshalb ist der Text eine Auskunft und kein Beweis.**
        /// Bezeugt und nachgerechnet werden die **Token**; wer schummelt,
        /// fällt dort auf und nicht hier. Wer den Text gegen die Token
        /// prüfen will, braucht den Wortschatz und prüft dann selbst.
        text: String,
    },
    /// Nichts gerechnet, und der Grund bleibt beim Pod.
    ///
    /// ⚑ **Eine Antwort, kein Schweigen.** Der Fragende soll den
    /// Unterschied zwischen „abgelehnt" und „nicht angekommen" kennen,
    /// sonst wartet er auf eine Zeitüberschreitung, die nichts bedeutet.
    /// Dieselbe Regel wie bei [`crate::core_types::PoIBundle`] und der
    /// Nachlieferung.
    ///
    /// **Ohne Grund**, denn ein Pod, der begründet, warum er nicht
    /// rechnet, verrät seine Auslastung und seinen Zustand.
    Abgelehnt {
        /// Die Sitzung, zu der die Ablehnung gehört.
        sitzung: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::EpochId;

    fn auftrag(prompt: &[u8], max_token: u32) -> Inferenzauftrag {
        Inferenzauftrag {
            sitzung: 7,
            bindung: Anfragebindung::neu(7, b"die frage", EpochId(3)),
            prompt_versiegelt: prompt.to_vec(),
            max_token,
            pipeline: Hash::sha256(b"probe-pipeline"),
        }
    }

    #[test]
    fn ein_gueltiger_auftrag_geht_durch() {
        assert_eq!(auftrag(b"versiegelt", 128).pruefe_form(), Ok(()));
    }

    /// ⚑ **Die Deckel stehen vor der teuren Arbeit** (Klasse von
    /// Fund 141).
    #[test]
    fn jeder_deckel_greift() {
        assert_eq!(auftrag(b"", 128).pruefe_form(), Err(Auftragsfehler::Leer));
        assert_eq!(
            auftrag(b"x", 0).pruefe_form(),
            Err(Auftragsfehler::NullToken)
        );
        assert_eq!(
            auftrag(b"x", MAX_NEUE_TOKEN + 1).pruefe_form(),
            Err(Auftragsfehler::ZuVieleToken {
                verlangt: MAX_NEUE_TOKEN + 1
            })
        );
        let zu_gross = vec![0u8; MAX_PROMPT_BYTES + 1];
        assert_eq!(
            auftrag(&zu_gross, 128).pruefe_form(),
            Err(Auftragsfehler::PromptZuGross {
                bytes: MAX_PROMPT_BYTES + 1
            })
        );
        // Gegenprobe: genau die Grenze geht noch, sonst prueft der Test
        // nur, dass irgendetwas abgelehnt wird.
        assert_eq!(auftrag(b"x", MAX_NEUE_TOKEN).pruefe_form(), Ok(()));
        let gerade_noch = vec![0u8; MAX_PROMPT_BYTES];
        assert_eq!(auftrag(&gerade_noch, 1).pruefe_form(), Ok(()));
    }

    /// ⚑ **Die Bindung passt zum Klartext**, und der Empfänger kann das
    /// nach dem Entsiegeln selbst feststellen.
    #[test]
    fn die_bindung_passt_zum_klartext() {
        let a = auftrag(b"versiegelt", 128);
        assert!(
            a.bindung.passt_zu_sitzung(7, b"die frage"),
            "die Bindung passt nicht zu ihrem eigenen Klartext"
        );
        assert!(
            !a.bindung.passt_zu_sitzung(7, b"eine andere frage"),
            "die Bindung passt zu einem fremden Klartext"
        );
        assert!(
            !a.bindung.passt_zu_sitzung(8, b"die frage"),
            "die Bindung passt zu einer fremden Sitzung"
        );
    }

    /// Auftrag und Antwort überstehen die Leitung unverändert.
    #[test]
    fn beide_ueberstehen_die_leitung() {
        let a = auftrag(b"versiegelt", 64);
        let roh = borsh::to_vec(&a).expect("kodieren");
        assert_eq!(borsh::from_slice::<Inferenzauftrag>(&roh).expect("dekodieren"), a);

        let e = Inferenzantwort::Ergebnis {
            sitzung: 7,
            token: vec![1, 2, 3],
            segment: SegmentId::new([9; 32]),
            prompt_token: 5,
            text: "Paris".to_string(),
        };
        let roh = borsh::to_vec(&e).expect("kodieren");
        assert_eq!(borsh::from_slice::<Inferenzantwort>(&roh).expect("dekodieren"), e);

        let ab = Inferenzantwort::Abgelehnt { sitzung: 7 };
        let roh = borsh::to_vec(&ab).expect("kodieren");
        assert_eq!(borsh::from_slice::<Inferenzantwort>(&roh).expect("dekodieren"), ab);
    }
}
