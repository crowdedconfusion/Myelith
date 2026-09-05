//! Der Sitzungsstrom: was beim Laufen entsteht, damit später jemand
//! prüfen kann (AGENT_LAYER 5.3, Whitepaper Kap. 8.4).
//!
//! # ⚑ Ein Beleg und kein Protokoll
//!
//! Ein Protokoll schreibt man für die Fehlersuche; man kann es
//! weglassen, und der Lauf ist derselbe. **Dieser Strom ist der Beleg**,
//! und damit gilt der Satz umgekehrt: Was der Nutzer später prüfen will,
//! **muss beim Laufen entstanden sein**. Nachträglich lässt es sich
//! nicht herstellen, denn dann bezeugte es sich selbst.
//!
//! ⚑ **Dieselbe Form hat DeepSeek Harness unabhängig gefunden** („eine
//! einheitliche Ausführungstrajektorie aus Nutzernachrichten,
//! Werkzeugaufrufen, Denkzuständen und Tokenzahlen, für Inspektion und
//! Wiedergabe"). Der Unterschied ist der Zweck: Dort dient sie dem
//! Entwickler, hier dem Dritten, der nichts glauben soll.
//!
//! # ⚑ Eine abgelehnte Anfrage steht mit drin, und das ist der Kern
//!
//! Ein Strom, der nur zeigt, **was ausgeführt wurde**, verschweigt das
//! Interessanteste. Wer ihn liest, sieht einen ordentlichen Lauf und
//! kann nicht unterscheiden, ob
//!
//! - das Modell brav geblieben ist, oder
//! - es dreimal versucht hat, Geld zu überweisen, und dreimal abgewiesen
//!   wurde.
//!
//! **Das ist derselbe Lauf und ein völlig anderer Befund.** Ein
//! Angriffsversuch ist ein Ereignis, kein Nichtereignis; er gehört in
//! den Beleg. ⚑ **Und er ist das früheste Zeichen, das es überhaupt
//! gibt:** Wer ihn wegwirft, erfährt von einem Angriff erst, wenn einer
//! gelingt.
//!
//! # Was verpflichtet ist und was nicht
//!
//! Der Strom trägt **Commitments**, nicht Inhalte. Wer den Inhalt hat,
//! kann prüfen; wer ihn nicht hat, sieht die Form und nicht den Text.
//! Das ist dieselbe Aufteilung wie bei einem Inferenzsegment: Die Spur
//! ist öffentlich, der Prompt nicht.

use myl_agent::kette::Kettenglied;
use myl_agent::registratur::Segmentstufe;
use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

use crate::betrieb::Betriebsart;
use crate::tuerklient::{Antwort, Nachricht};
use crate::werkzeug::{Abgelehnt, Vorschlag};

/// Wie über einen Vorschlag entschieden wurde.
///
/// ⚑ **Beide Ausgänge stehen im Strom.** Ein „nur das Erlaubte"
/// wäre ein Beleg, der den Angriff wegkürzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entscheidung {
    /// Der Vorschlag stand in der Erlaubnis.
    Erlaubt,
    /// Er stand nicht darin.
    ///
    /// ⚑ **Mit Begründung**, damit ein Leser sieht, **was** erlaubt
    /// gewesen wäre. Ohne sie liesse sich „abgelehnt" nicht von
    /// „falsch verstanden" unterscheiden.
    Abgelehnt(Abgelehnt),
}

impl Entscheidung {
    /// Ob ausgeführt werden durfte.
    pub fn erlaubt(&self) -> bool {
        matches!(self, Self::Erlaubt)
    }
}

/// Was in **einem** Schritt geschah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schritt {
    /// Commitment über die Nachrichten, die an das Modell gingen.
    pub anfrage: Hash,
    /// Commitment über den Antworttext.
    pub antwort: Hash,
    /// Die Segmentkennung, unter der die Tür gerechnet hat.
    ///
    /// ⚑ `None`, wenn die Tür sie nicht nennt. Dann trägt dieser
    /// Schritt **kein** Kettenglied, und der Strom sagt das, statt eines
    /// zu erfinden.
    pub segment: Option<SegmentId>,
    /// Was das Modell vorgeschlagen hat, und wie entschieden wurde.
    pub vorschlaege: Vec<(Vorschlag, Entscheidung)>,
    /// Commitment über die Werkzeugergebnisse, in Aufrufreihenfolge.
    pub ergebnisse: Vec<Hash>,
    /// Wie viel ein Prüfer mit diesem Schritt anfangen kann.
    ///
    /// ⚑ **Je Schritt und nicht je Sitzung.** Die Stufe ist das Minimum
    /// über alles, was **dieser** Schritt benutzt hat; ein Lauf kann
    /// nachrechenbar beginnen und es an einer Stelle nicht mehr sein.
    /// Eine Stufe für die ganze Sitzung verschwiege, **wo** es kippte.
    pub stufe: Segmentstufe,
}

/// Der Strom einer Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sitzungsstrom {
    anker: Hash,
    betriebsart: Betriebsart,
    schritte: Vec<Schritt>,
}

/// Warum ein Strom nicht zur Kette taugt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stromfehler {
    /// Ein Schritt trägt keine Segmentkennung.
    ///
    /// ⚑ **Kein stilles Überspringen.** Wer die Schritte ohne Kennung
    /// wegliesse, bekäme eine kürzere Kette, die in sich stimmig ist und
    /// zu einem anderen Plan gehört; genau der Fall, den Kap. 8.4
    /// „ausgelassen" nennt.
    SchrittOhneSegment {
        /// Der wievielte Schritt.
        stelle: usize,
    },
}

impl std::fmt::Display for Stromfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchrittOhneSegment { stelle } => write!(
                f,
                "Schritt {stelle} hat keine Segmentkennung: die Tuer hat sie nicht genannt, \
                 und eine Kette darueber waere erfunden"
            ),
        }
    }
}

impl std::error::Error for Stromfehler {}

impl Sitzungsstrom {
    /// Ein leerer Strom unter diesem Anker.
    ///
    /// ⚑ **Der Anker kommt von aussen**, aus
    /// [`myl_agent::kette::anker`]: Sitzung und Plan. Ohne ihn liesse
    /// sich eine ganze Kette aus einer Sitzung in eine andere heben.
    pub fn neu(anker: Hash, betriebsart: Betriebsart) -> Self {
        Self { anker, betriebsart, schritte: Vec::new() }
    }

    /// Der Anker.
    pub fn anker(&self) -> &Hash {
        &self.anker
    }

    /// Unter welcher Betriebsart der Lauf stand.
    ///
    /// ⚑ **Sie gehört in den Beleg.** Ohne sie liesse sich später nicht
    /// unterscheiden, ob alle Schritte nachrechenbar waren, **weil die
    /// Betriebsart es erzwang** oder weil es sich zufällig so ergab. Das
    /// sind zwei verschiedene Aussagen, und nur die erste ist eine
    /// Zusage.
    pub fn betriebsart(&self) -> Betriebsart {
        self.betriebsart
    }

    /// Die Schritte, die ein Dritter **nicht** nachrechnen kann.
    ///
    /// ⚑ **Die zweite Zahl, die ein Mensch sehen will**, neben
    /// [`Sitzungsstrom::abgelehnte`]. Unter `NurVerankert` muss sie null
    /// sein; ist sie es nicht, hat die Betriebsart nicht gegriffen.
    pub fn nicht_nachrechenbare(&self) -> Vec<usize> {
        self.schritte
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.stufe.nachrechenbar())
            .map(|(i, _)| i)
            .collect()
    }

    /// Die Schritte, in Reihenfolge.
    pub fn schritte(&self) -> &[Schritt] {
        &self.schritte
    }

    /// Hängt einen Schritt an.
    ///
    /// **Nur beim Laufen gerufen**, und genau einmal je Schritt.
    pub fn anhaengen(&mut self, s: Schritt) {
        self.schritte.push(s);
    }

    /// Baut den Schritt aus dem, was der Lauf gerade hatte.
    ///
    /// ⚑ **Die Commitments entstehen hier und nicht später.** Wer sie
    /// nachträglich über gespeicherten Text bildete, bezeugte den
    /// Speicher, nicht den Lauf.
    pub fn schritt_aus(
        nachrichten: &[Nachricht],
        antwort: &Antwort,
        vorschlaege: Vec<(Vorschlag, Entscheidung)>,
        ergebnisse: &[String],
        stufe: Segmentstufe,
    ) -> Schritt {
        Schritt {
            anfrage: nachrichten_commitment(nachrichten),
            antwort: Hash::sha256(antwort.text.as_bytes()),
            segment: antwort.segment.map(SegmentId::new),
            vorschlaege,
            ergebnisse: ergebnisse.iter().map(|e| Hash::sha256(e.as_bytes())).collect(),
            stufe,
        }
    }

    /// Wie viele Vorschläge abgelehnt wurden.
    ///
    /// ⚑ **Die Zahl, die ein Mensch zuerst sehen will.** Null heisst,
    /// dass nichts versucht wurde; alles darüber ist ein Befund, auch
    /// wenn der Lauf gut ausging.
    pub fn abgelehnte(&self) -> usize {
        self.schritte
            .iter()
            .flat_map(|s| s.vorschlaege.iter())
            .filter(|(_, e)| !e.erlaubt())
            .count()
    }

    /// Was ein Mensch nach dem Lauf sehen soll.
    ///
    /// # ⚑ Warum es diese Ausgabe gibt (AGENT_LAYER 5.5, Kap. 8.1)
    ///
    /// Die Herkunftskennzeichnung ist im Whitepaper eine **sichtbare**
    /// Anforderung, nicht bloss eine berechnete Grösse. Ein Strom, der
    /// die Stufe je Schritt trägt und sie niemandem zeigt, erfüllt sie
    /// nicht: Er hält sie fest, und Festhalten ist nicht Zeigen.
    ///
    /// **Drei Dinge stehen darin, und alle drei in dieser Reihenfolge:**
    ///
    /// 1. die **Betriebsart**, denn ohne sie ist „alles nachrechenbar"
    ///    nicht von „es ergab sich zufällig" zu unterscheiden;
    /// 2. die **abgelehnten Vorschläge**, denn ein Angriffsversuch ist
    ///    ein Ereignis und das früheste Zeichen, das es gibt;
    /// 3. die **Schritte ohne Nachrechenbarkeit**, mit Stelle.
    ///
    /// ⚑ **Ein sauberer Lauf sagt das ausdrücklich**, statt zu
    /// schweigen. „Keine Ablehnungen" ist eine Aussage; eine leere Zeile
    /// ist keine, und der Leser könnte sie für ein fehlendes Protokoll
    /// halten.
    pub fn bericht(&self) -> String {
        let mut t = format!(
            "Sitzung: {} Schritte, Betriebsart `{}`\n",
            self.schritte.len(),
            self.betriebsart.name()
        );
        let abgelehnt = self.abgelehnte();
        if abgelehnt == 0 {
            t.push_str("  Abgelehnte Werkzeugvorschläge: keine\n");
        } else {
            t.push_str(&format!("  ⚑ Abgelehnte Werkzeugvorschläge: {abgelehnt}\n"));
            for (i, s) in self.schritte.iter().enumerate() {
                for (v, e) in &s.vorschlaege {
                    if let Entscheidung::Abgelehnt(a) = e {
                        t.push_str(&format!(
                            "      Schritt {i}: `{}` verlangt, erlaubt waren {:?}\n",
                            v.name, a.erlaubt
                        ));
                    }
                }
            }
        }
        let offen = self.nicht_nachrechenbare();
        if offen.is_empty() {
            t.push_str("  Alle Schritte sind nachrechenbar.\n");
        } else {
            t.push_str(&format!(
                "  ⚑ Nicht nachrechenbar: {} von {} Schritten\n",
                offen.len(),
                self.schritte.len()
            ));
            for i in offen {
                t.push_str(&format!(
                    "      Schritt {i}: {}\n",
                    self.schritte[i].stufe.name()
                ));
            }
        }
        t
    }

    /// Die Kettenglieder, für [`myl_agent::kette::kettenwert`].
    ///
    /// ⚑ **Die Ausgabe eines Glieds ist das Commitment über den
    /// Antworttext**, nicht über die Werkzeugergebnisse. Ein Werkzeug
    /// ist kein Schritt des Modells; sein Ergebnis geht als **Eingabe**
    /// in den nächsten Schritt und steht dort in dessen
    /// Anfrage-Commitment.
    pub fn kettenglieder(&self) -> Result<Vec<Kettenglied>, Stromfehler> {
        self.schritte
            .iter()
            .enumerate()
            .map(|(i, s)| match s.segment {
                Some(segment) => Ok(Kettenglied { segment, ausgabe: s.antwort }),
                None => Err(Stromfehler::SchrittOhneSegment { stelle: i }),
            })
            .collect()
    }
}

/// Commitment über eine Nachrichtenfolge.
///
/// ⚑ **Länge vor jedem Feld.** Ohne sie liessen sich zwei verschiedene
/// Folgen zu derselben Bytefolge zusammensetzen, etwa indem eine Rolle
/// in den Inhalt der vorigen Nachricht rutscht. Dieselbe Überlegung wie
/// beim Trainingsabdruck und bei der Segmentbotschaft.
pub fn nachrichten_commitment(nachrichten: &[Nachricht]) -> Hash {
    let mut roh: Vec<u8> = Vec::new();
    roh.extend_from_slice(b"myl-agent-anfrage-v1");
    roh.extend_from_slice(&(nachrichten.len() as u64).to_le_bytes());
    for n in nachrichten {
        roh.extend_from_slice(&(n.role.len() as u64).to_le_bytes());
        roh.extend_from_slice(n.role.as_bytes());
        roh.extend_from_slice(&(n.content.len() as u64).to_le_bytes());
        roh.extend_from_slice(n.content.as_bytes());
    }
    Hash::sha256(&roh)
}
