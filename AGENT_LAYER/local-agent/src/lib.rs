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
//! # Der Stand
//!
//! **Diese Kiste trägt heute ihre Grenze und sonst nichts.** Das ist
//! Absicht: Punkt 5.6 (die Isolation als Zusicherung statt als Absicht)
//! liess sich bauen, bevor die erste Zeile Harness existiert, und eine
//! Grenze, die nur im Text steht, überlebt den ersten eiligen
//! Nachmittag nicht. Die Punkte 5.1 bis 5.5 stehen im Fahrplan.

#![deny(unsafe_code)]

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
