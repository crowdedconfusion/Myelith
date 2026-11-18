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

/// ⚑ **Der Preis einer Anfrage in Credits** (Fund 711, Entscheidung des
/// Projektinhabers vom 2026-11-17): so viele Vorwaertspaesse, wie der Pod
/// fuer sie rechnet und als vTFE beansprucht. Der Prompt geht Position fuer
/// Position durch alle Shards, dann jedes erzeugte Token ausser dem letzten
/// noch einmal; das sind `prompt_token + neue_token - 1` Paesse. Ohne neues
/// Token ist der Prompt trotzdem gerechnet, und mindestens einer ist es
/// immer.
///
/// 📌 Bis zum 2026-11-17 kostete eine Anfrage nur ihre erzeugten Token. Im
/// Auftragslauf zahlte ein Agent 463 Credits fuer rund 12 300 gerechnete
/// Positionen, also ein Fuenfundzwanzigstel seiner Arbeit, und die
/// Nachfrage, an der der Credit-Preis haengt (Whitepaper 5.4), wurde um
/// denselben Faktor zu klein gemessen. Whitepaper 5.1 denominiert Credits
/// in vTFE; ein eigenes Gewicht fuer Prompt-Positionen waere eine Aenderung
/// daran.
pub fn preis(prompt_token: u64, neue_token: u64) -> u64 {
    prompt_token.saturating_add(neue_token.max(1)).saturating_sub(1).max(1)
}

/// ⚑ **Die obere Schranke, bis zu der die Tuer nach einem Hoechstbetrag
/// sucht** (Fund 711): die Bytes des Prompts und der Deckel. Die Tuer kennt
/// den Wortschatz nicht; ein Token des byteweisen BPE der Netzmodelle traegt
/// mindestens ein Byte, also hat ein Prompt hoechstens so viele Token wie
/// Bytes. Die Schranke ist grob (rund das Dreifache) und deshalb nur der
/// Rahmen der Suche, nie der Preis: Den haelt der Pod genau ein, siehe
/// [`Inferenzauftrag::neue_token_im_rahmen`]. ⚠️ Eine Unicode-Normalisierung
/// kann seltene Zeichen verlaengern; ein solcher Prompt kann mehr Token
/// haben als Bytes, und dann sagt der Pod „nicht gedeckt“. Das kostet den
/// Absender eine Ablehnung und niemanden Arbeit.
pub fn obergrenze(prompt_bytes: u64, max_token: u32) -> u64 {
    prompt_bytes
        .saturating_add(u64::from(max_token.min(MAX_NEUE_TOKEN)))
        .max(1)
}

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
    /// `hoechstens` ist null: ein Auftrag, den niemand bezahlt.
    OhneBetrag,
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
            Self::OhneBetrag => f.write_str("ein Auftrag ohne Hoechstbetrag bezahlt nichts"),
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
    /// ⚑ **So viele Credits darf der Auftrag hoechstens kosten** (Fund 711),
    /// gemessen mit [`preis`]: das Groesste, was Vollmacht, Kontrakt und
    /// Guthaben an der Tuer decken. Nur der Pod kennt die Zahl der
    /// Prompt-Token; er haelt den Betrag deshalb selbst ein
    /// ([`Self::neue_token_im_rahmen`]) und sagt [`Inferenzantwort::Ungedeckt`],
    /// wenn schon der Prompt mehr kostet.
    ///
    /// **Angehaengt** (2026-11-17), wie die Feldreihenfolge es verlangt.
    pub hoechstens: u64,
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
        if self.hoechstens == 0 {
            return Err(Auftragsfehler::OhneBetrag);
        }
        Ok(())
    }

    /// ⚑ **Wie viele neue Token der Hoechstbetrag zulaesst** (Fund 711),
    /// nachdem der Pod den Prompt gezaehlt hat: `None`, wenn schon der
    /// Prompt mit einem einzigen Token mehr kostet als [`Self::hoechstens`];
    /// sonst `max_token`, gekuerzt so weit, dass [`preis`] den Betrag nicht
    /// uebersteigt. Eine gekuerzte Antwort endet wie jede am Deckel
    /// (`finish_reason` `length`).
    pub fn neue_token_im_rahmen(&self, prompt_token: u64) -> Option<u32> {
        if prompt_token == 0 || prompt_token > self.hoechstens {
            return None;
        }
        // preis(p, n) = p + n - 1 <= h  <=>  n <= h - p + 1
        let erlaubt = self.hoechstens - prompt_token + 1;
        Some(u32::try_from(erlaubt).unwrap_or(u32::MAX).min(self.max_token))
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
    /// ⚑ **Nicht gerechnet, weil schon der Prompt mehr kostet als der
    /// Hoechstbetrag** (Fund 711). Kein Zustand des Pods, sondern eine
    /// Aussage ueber den Auftrag: Die Tuer antwortet darauf mit 402 und
    /// nennt, was fehlt, statt mit 502, auf das ein Klient neu anlaeuft.
    ///
    /// **Angehaengt** (2026-11-17), die Reihenfolge der Faelle ist
    /// Protokollvertrag.
    Ungedeckt {
        /// Die Sitzung, zu der die Antwort gehört.
        sitzung: u64,
        /// Wie viele Token der Prompt hatte.
        prompt_token: u64,
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
            hoechstens: u64::MAX,
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

        let ug = Inferenzantwort::Ungedeckt { sitzung: 7, prompt_token: 9 };
        let roh = borsh::to_vec(&ug).expect("kodieren");
        assert_eq!(borsh::from_slice::<Inferenzantwort>(&roh).expect("dekodieren"), ug);
    }

    /// ⚑ **Der Preis sind die Vorwaertspaesse** (Fund 711): Prompt und
    /// Ausgabe, das letzte Token nicht noch einmal, mindestens einer.
    #[test]
    fn der_preis_sind_die_vorwaertspaesse() {
        assert_eq!(preis(5, 3), 7);
        assert_eq!(preis(1, 1), 1);
        assert_eq!(preis(11_827, 463), 12_289);
        // Gegenproben: Die alte Rechnung (nur die Ausgabe) und die
        // naive Summe sind beide etwas anderes.
        assert_ne!(preis(5, 3), 3);
        assert_ne!(preis(5, 3), 8);
        assert_eq!(preis(0, 0), 1, "auch ein leerer Lauf kostet einen Pass");
        assert_eq!(preis(30, 0), 30, "ohne neues Token der ganze Prompt");
        assert_eq!(preis(u64::MAX, 2), u64::MAX - 1, "kein Ueberlauf");
    }

    /// ⚑ **Der Pod haelt den Hoechstbetrag genau ein** (Fund 711): Der
    /// Preis mit der gekuerzten Zahl neuer Token liegt nie ueber dem
    /// Betrag, und ein Token mehr laege darueber.
    #[test]
    fn der_hoechstbetrag_kuerzt_die_ausgabe_und_nie_darueber() {
        let mut a = auftrag(b"x", 64);
        a.hoechstens = 20;
        assert_eq!(a.neue_token_im_rahmen(10), Some(11));
        assert_eq!(preis(10, 11), 20);
        assert!(preis(10, 12) > 20, "Gegenprobe: ein Token mehr ginge darueber");
        assert_eq!(a.neue_token_im_rahmen(20), Some(1), "genau der Prompt mit einem Token");
        assert_eq!(a.neue_token_im_rahmen(21), None, "schon der Prompt kostet mehr");
        assert_eq!(a.neue_token_im_rahmen(0), None, "ein leerer Prompt rechnet nichts");
        a.hoechstens = 1_000;
        assert_eq!(a.neue_token_im_rahmen(10), Some(64), "der Deckel bleibt, wenn der Betrag reicht");
        a.hoechstens = u64::MAX;
        assert_eq!(a.neue_token_im_rahmen(1), Some(64), "kein Ueberlauf");
        // Durchgemustert: fuer jede Lage gilt preis <= hoechstens, und
        // der Deckel bleibt die Obergrenze.
        for h in 1..60u64 {
            a.hoechstens = h;
            for p in 1..70u64 {
                match a.neue_token_im_rahmen(p) {
                    Some(n) => {
                        assert!((1..=64).contains(&n));
                        assert!(preis(p, u64::from(n)) <= h, "h {h}, p {p}, n {n}");
                        assert!(n == 64 || preis(p, u64::from(n) + 1) > h, "zu frueh gekuerzt: h {h}, p {p}, n {n}");
                    }
                    None => assert!(p > h, "abgelehnt, obwohl gedeckt: h {h}, p {p}"),
                }
            }
        }
    }

    /// Ein Auftrag ohne Hoechstbetrag ist formwidrig.
    #[test]
    fn ohne_betrag_ist_formwidrig() {
        let mut a = auftrag(b"x", 8);
        a.hoechstens = 0;
        assert_eq!(a.pruefe_form(), Err(Auftragsfehler::OhneBetrag));
        a.hoechstens = 1;
        assert_eq!(a.pruefe_form(), Ok(()));
    }

    /// Die Schranke der Tuer: Bytes und Deckel, der Deckel hoechstens der
    /// des Protokolls.
    #[test]
    fn die_obergrenze_sind_bytes_und_deckel() {
        assert_eq!(obergrenze(100, 50), 150);
        assert_eq!(obergrenze(100, u32::MAX), 100 + u64::from(MAX_NEUE_TOKEN));
        assert_eq!(obergrenze(0, 0), 1);
    }
}
