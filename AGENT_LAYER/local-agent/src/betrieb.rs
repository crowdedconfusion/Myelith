//! Die Betriebsart: „nur verankert" gegen „alles" (AGENT_LAYER 5.4).
//!
//! # ⚑ Was der Nutzer hier wählt
//!
//! Ob er Schritte zulässt, die **niemand nachrechnen kann**.
//!
//! `myl_agent::Registratur::stufe` sagt je Schritt, wie viel ein Prüfer
//! damit anfangen kann, und die Stufe ist das **Minimum über alles
//! Benutzte**: Ein verankerter Skill neben einem lokalen ergibt ein
//! Segment, das niemand nachrechnen kann.
//!
//! **Diese Wahl gehört dem Nutzer und nicht dem Programm.** Ein lokaler
//! Skill ist zulässig und bequem; sein Preis ist, dass ein Dritter das
//! Ergebnis nur glauben kann. Wer das weiss und in Kauf nimmt, darf es.
//!
//! # ⚑ Aber „unbekannt" ist keine Wahl, sondern ein Defekt
//!
//! [`Segmentstufe`] kennt **drei** Zustände, und der dritte ist keine
//! schwächere Form des zweiten:
//!
//! | Stufe | Was der Nutzer in Kauf nähme |
//! |---|---|
//! | `Nachrechenbar` | nichts |
//! | `Bezeugt` | „ich kann das nicht nachrechnen" |
//! | `Unbekannt` | **„ich weiss nicht, was gelaufen ist"** |
//!
//! ⚑ **In die dritte Zeile lässt sich nicht einwilligen.** Wer nicht
//! weiss, welcher Skill benutzt wurde, weiss auch nicht, wozu er ja
//! sagt. Eine Einwilligung ohne Gegenstand ist keine, und deshalb lehnen
//! **beide** Betriebsarten sie ab.
//!
//! Praktisch heisst `Unbekannt`: Ein Schritt nennt eine Adresse, die in
//! der Registratur nicht steht. Das ist kein Kompromiss, den jemand
//! eingeht, sondern ein Zustand, der nicht hätte entstehen dürfen.
//!
//! # ⚑ Die Betriebsart steht im Strom
//!
//! Sonst liesse sich später nicht unterscheiden, ob alle Schritte
//! nachrechenbar waren, **weil die Betriebsart es erzwang** oder weil es
//! sich zufällig so ergab. Das sind zwei verschiedene Aussagen, und nur
//! die erste ist eine Zusage.

use myl_agent::registratur::Segmentstufe;

/// Was der Nutzer zulässt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Betriebsart {
    /// Nur Schritte, die ein Dritter nachrechnen kann.
    ///
    /// ⚑ **Die vorsichtige Wahl, und sie ist die Vorgabe.** Wer das
    /// Gegenteil will, sagt es; wer nichts sagt, bekommt das Engere.
    #[default]
    NurVerankert,
    /// Auch Schritte, die nur bezeugt sind.
    ///
    /// ⚑ **„Alles" heisst nicht alles.** `Unbekannt` bleibt auch hier
    /// gesperrt, siehe Modulkopf: In einen Zustand ohne Gegenstand lässt
    /// sich nicht einwilligen.
    Alles,
}

/// Warum ein Schritt in dieser Betriebsart nicht laufen darf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gesperrt {
    /// Der Schritt wäre nur bezeugt, und die Betriebsart verlangt
    /// Nachrechenbarkeit.
    NurBezeugt {
        /// Welche Adressen den Ausschlag gaben.
        wegen: Vec<String>,
    },
    /// Eine benutzte Adresse ist unbekannt.
    ///
    /// ⚑ **In beiden Betriebsarten**, siehe Modulkopf.
    Unbekannt {
        /// Welche Adressen.
        welche: Vec<String>,
    },
}

impl std::fmt::Display for Gesperrt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NurBezeugt { wegen } => write!(
                f,
                "dieser Schritt waere nur bezeugt und nicht nachrechenbar, wegen {wegen:?}. \
                 Betriebsart `nur verankert`: er laeuft nicht"
            ),
            Self::Unbekannt { welche } => write!(
                f,
                "benutzte Adressen sind unbekannt: {welche:?}. Damit steht nicht fest, was \
                 laufen wuerde, und in einen Zustand ohne Gegenstand laesst sich nicht \
                 einwilligen: er laeuft in KEINER Betriebsart"
            ),
        }
    }
}

impl std::error::Error for Gesperrt {}

impl Betriebsart {
    /// Darf ein Schritt mit dieser Stufe laufen?
    ///
    /// ⚑ **Gefragt wird VOR dem Schritt, nicht danach.** Ein Segment,
    /// das niemand nachrechnen kann, hinterher als solches auszuweisen,
    /// ist zu spät: Der Nutzer hat dann schon bezahlt, und die Antwort
    /// steht schon in seinem Kontext.
    pub fn pruefen(&self, stufe: &Segmentstufe) -> Result<(), Gesperrt> {
        match stufe {
            Segmentstufe::Nachrechenbar => Ok(()),
            Segmentstufe::Unbekannt { welche } => {
                Err(Gesperrt::Unbekannt { welche: welche.iter().map(kurz).collect() })
            }
            Segmentstufe::Bezeugt { wegen } => match self {
                Self::Alles => Ok(()),
                Self::NurVerankert => {
                    Err(Gesperrt::NurBezeugt { wegen: wegen.iter().map(kurz).collect() })
                }
            },
        }
    }

    /// Kurzform für Protokoll und Oberfläche.
    ///
    /// ⚑ **Punkt 2.4 der CLIENT-Liste verlangt, dass die Stufe beim
    /// Menschen ankommt.** Eine Betriebsart, die nur im Quelltext steht,
    /// ist eine Zusage an niemanden.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NurVerankert => "nur verankert",
            Self::Alles => "alles",
        }
    }
}

/// Die ersten acht Hexzeichen einer Adresse.
///
/// ⚑ **Gekürzt für Menschen, nicht für die Rechnung.** Wer vergleichen
/// will, nimmt die volle Adresse aus der [`Segmentstufe`].
fn kurz(a: &myl_types::ids::MerkleRoot) -> String {
    a.as_bytes().iter().take(4).map(|b| format!("{b:02x}")).collect()
}
