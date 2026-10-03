//! Das lokale Harness (AGENT_LAYER 5, Whitepaper Kap. 8).
//!
//! # ⚑ Was hier entsteht, und was ausdrücklich nicht
//!
//! Ein Programm, das beim **Nutzer** läuft, gegen die `/v1`-Tür eines
//! Knotens spricht und dabei einen Plan abarbeitet.
//!
//! **Zwei Dinge macht es, und beide hat der Agent Layer schon als Typ:**
//! Es schreibt seinen Verlauf als Kette fort (`myl_agent::kette`), und
//! es weist die Verifikationsstufe dessen aus, was es benutzt hat
//! (`myl_agent::Registratur`). Damit ist ein Lauf später prüfbar,
//! statt nur stattgefunden zu haben.
//!
//! # ⚑ Drei Schichten, und die Trennung gehört genau gefasst
//!
//! Eine naheliegende Formulierung ist ungenau. **`myl-agent` läuft
//! nicht on-chain.** Es hängt an `myl-types` und `borsh`, und kein
//! Crate in CONSENSUS oder NODE benutzt es.
//!
//! | Schicht | Was sie ist | Verhältnis zur Kette |
//! |---|---|---|
//! | `myl-types` + CONSENSUS | Kontrakt, Grenzen, Abbuchung | **wird durchgesetzt** |
//! | `myl-agent` | deterministische Regeln und Formate | **verankerbar** |
//! | `myl-local-agent` | dieses Harness | **isoliert** |
//!
//! ⚑ **Die Isolation, so scharf wie sie sich durchsetzen lässt:** Die
//! einzige Berührung dieses Programms mit der Kette ist **ein Token,
//! das ihm gereicht wurde**. Es unterschreibt keine Transaktion, liest
//! keinen Kettenzustand und hält keinen Schlüssel; es hat eine
//! Vollmacht und spricht HTTP. **Abgebucht wird nicht von ihm**,
//! sondern vom Knoten, nachdem gerechnet wurde
//! (`myl_node::rechenweg::Ortsweg::mit_abrechnung`).
//!
//! ⚑ **Neben `myl-agent` und nicht darin.** Eigene Kiste, eigene
//! Fassung, eigener Lebenslauf: Was hier pfadabhängig lernt oder
//! ausprobiert, darf die deterministische Schicht nicht berühren. Es
//! **benutzt** `myl-agent` (Plan, Registratur, Kette), denn genau das
//! macht seinen Verlauf später verankerbar.
//!
//! ⚑ **Und unter AGENT_LAYER, nicht unter CLIENT** (Entscheidung
//! 2026-09-04). Der Client greift darauf zu; ein Harness, der im Client
//! lebt, liest sich, als gehörte die Agentenlogik dem Client.
//!
//! # Was hier liegt
//!
//! ⚑ **Seit dem 2026-09-30 der ganze örtliche Agent** (Festlegung des
//! Projektinhabers): Was bis dahin in `myl-client` lag, gehört hierher,
//! und der Client hängt an dieser Kiste, nie umgekehrt.
//!
//! | Modul | wofür |
//! |---|---|
//! | [`tuerklient`], [`schleife`], [`werkzeug`], [`ausfuehrung`], [`verdichtung`] | die Schleife, das Werkzeugformat, der Weg zum Modell (`Modellweg`) |
//! | [`strom`], [`betrieb`], [`risiko`], [`vollmacht_grenzen`] | Sitzungsstrom als Beleg, Betriebsart, Risikoklassen, Vollmacht |
//! | [`werkzeuge`], [`netzwerkzeuge`], [`sinneswerkzeuge`], [`verankert`], [`abgeschaltet`], [`syntaxwache`], [`uhr`] | die Werkzeuge, mit Einhängegrenze |
//! | [`kisten`] | Werkzeugkisten aus Manifesten (`AGENT_LAYER/local-toolkits/`) |
//! | [`ruestung`], [`lauf`] | einen Lauf ausstatten und fahren |
//! | [`vorhaben`] | der Loop: ein Ziel über viele Runden |
//! | [`skills`], [`verlauf`], [`gespraech`] | Skills (`AGENT_LAYER/local-skills/`), Mitschnitt, Verdichtung |
//! | [`systemprompt`], [`protokoll`], [`notaus`] | Regeln, Aktionsprotokoll, Notaus |
//! | [`secure_flow`] | wohin Daten fliessen duerfen: Labels, Vertraege, Pruefung an der Werkzeuggrenze |
//! | [`agentenwahl`] | was der Nutzer dem Agenten einstellt |
//! | [`ort`], [`textstrom`] | wo Myelith ablegt; Token zu Text, getrennt nach Denken und Antwort |
//!
//! ⚑ **Was hier nicht liegt:** das Modell selbst (es steckt hinter
//! [`Modellweg`], der Client lädt es), die Einstellungsseite und die drei
//! Bedieninstrumente.
//!
//! ⚑ **`local-` heisst: nur auf diesem Rechner.** Nichts hier ist Teil
//! einer prüfbaren Netzaktion; `myl-agent` kennt diese Kiste nicht und
//! prüft das selbst.

#![deny(unsafe_code)]

pub mod tuerklient;
pub mod ausfuehrung;
pub mod betrieb;
pub mod risiko;
pub mod schleife;
pub mod vollmacht_grenzen;
pub mod strom;
pub mod werkzeug;
pub mod verdichtung;

// ⚑ Der oertliche Agent, bis zum 2026-09-30 in `myl-client`.
pub mod abgeschaltet;
pub mod agentenwahl;
pub mod secure_flow;
pub mod gespraech;
pub mod kisten;
pub mod lauf;
pub mod netzwerkzeuge;
pub mod notaus;
pub mod ort;
pub mod protokoll;
pub mod ruestung;
pub mod sinneswerkzeuge;
pub mod skills;
pub mod syntaxwache;
pub mod systemprompt;
pub mod textstrom;
pub mod uhr;
pub mod verankert;
pub mod verlauf;
pub mod vorhaben;
pub mod werkzeuge;

pub use tuerklient::{Antwort, Kontextstand, Modellweg, Nachricht, Tuerfehler, Tuerklient};
pub use werkzeug::{Erlaubnis, Vorschlag, Werkzeug};

/// Die Kisten, die dieses Harness **nicht** kennen darf.
///
/// # ⚑ Warum als Liste im Code und nicht nur als Satz im README
///
/// Ein Satz wird gelesen, eine Liste wird geprüft. `tests/isolation.rs`
/// hält die eigene `Cargo.toml` dagegen, und damit ist die Trennung
/// eine Zusicherung und keine Absicht.
///
/// **Warum diese drei:** Wer den Konsens, das Ledger oder den Knoten
/// einbindet, kann Kettenzustand lesen oder eine Transaktion bauen. Ab
/// dann ist „das Harness hält nur ein Token" nicht mehr wahr, und es
/// fiele niemandem auf, weil alles weiter übersetzt.
pub const VERBOTENE_KISTEN: [&str; 3] = ["myl-consensus", "myl-ledger", "myl-node"];

/// Der Weg, unter dem die Tür eines Knotens Aufträge annimmt.
///
/// ⚑ **Aus `myl-gateway` abgeschrieben wäre falsch herum**: Dann hinge
/// dieses Harness am Gateway, und die Abhängigkeitsliste wäre wieder
/// länger als die Grenze erlaubt. Ein Klient kennt eine URL, mehr
/// nicht; genau das war der Zweck der Stufe 3.
pub const WEG_CHAT: &str = "/v1/chat/completions";

/// Der Vorgabeport der eigenen Tür eines Knotens.
pub const TUER_PORT: u16 = 4160;

#[cfg(test)]
mod tests {
    use super::*;

    /// Die beiden Wegangaben stimmen mit dem überein, was die Tür
    /// bedient.
    ///
    /// ⚑ **Von Hand nachgetragen und nicht importiert**, siehe die
    /// Begründung an [`WEG_CHAT`]. Weicht die Tür ab, fällt dieser Test
    /// nicht, sondern der Aufruf; deshalb steht die Zahl hier mit ihrer
    /// Herkunft und nicht nackt.
    #[test]
    fn die_wegangaben_sind_die_der_tuer() {
        assert_eq!(WEG_CHAT, "/v1/chat/completions");
        assert_eq!(TUER_PORT, 4160);
    }

    /// `myl-agent` ist erreichbar: die Kiste liegt daneben, nicht darin.
    #[test]
    fn myl_agent_ist_benutzbar() {
        let plan = myl_agent::Plan::leer();
        assert!(plan.is_empty(), "ein leerer Plan hat keine Schritte");
    }
}
