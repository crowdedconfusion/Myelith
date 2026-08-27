//! `myl-governance` — Parameter-Governance (Whitepaper Kap. 10.3).
//!
//! Diese Komponente ist überwiegend Prozessdefinition; der Codeanteil ist
//! bewusst klein und dient der **technischen Durchsetzung** von
//! Entscheidungen, die politisch getroffen werden, nicht deren Ersatz.
//!
//! Drei Aufgaben, und keine weitere:
//!
//! 1. **Ein Ort für alle Parameter** ([`registry`]). Solange sie über ein
//!    Dutzend Crates verstreut als Konstanten stehen, ist „änderbar per
//!    Abstimmung" eine Absichtserklärung ohne Gegenstand.
//! 2. **Verfassungsrang technisch durchsetzen** ([`vorschlag`]). Kap. 10.3
//!    nennt drei nicht änderbare Festlegungen. Eine rein prozessuale Regel
//!    ist nur so stark wie die Disziplin der Beteiligten; hier scheitert
//!    der Vorschlag am Protokoll.
//! 3. **Sicherheitsinvarianten schon am Vorschlag prüfen**
//!    ([`invarianten`]), nicht erst nach der Abstimmung. Ein
//!    Parametersatz, der `S_min` unterschreitet oder die
//!    Self-Dealing-Grenze aus Anhang B.4 verletzt, ist auch dann falsch,
//!    wenn eine Mehrheit dafür stimmt.
//!
//! ## Warum diese Komponente jetzt entsteht
//!
//! Sie hatte bis zum 2026-08-24 keine Zeile Code, und drei Funde des
//! Vortags zeigten alle hierher:
//!
//! - **Fund 46** (TOKENOMICS): Prägung und Preis liefen an den Rändern
//!   des Zahlbereichs über. Behoben ist, dass sie nicht mehr überlaufen;
//!   **nicht** behoben war, dass ein Parametersatz sie überhaupt dorthin
//!   bringen darf.
//! - **Fund 47** (TOKENOMICS): `ema_update_with_alpha` sagte zu, für
//!   α > 1 total zu bleiben, und lief um. Die Behebung endet mit dem Satz
//!   „die Prüfung von α gehört in die Governance-Schicht; diese Funktion
//!   kann sie nicht ersetzen, sie kann nur aufhören, den Fehler zu
//!   verstärken." Hier ist die Schicht.
//! - Die offene Frage nach einer **Untergrenze für den Credit-Preis**
//!   ist ebenfalls eine Parameterfrage und keine Überlaufsicherung.
//!
//! Ein Crate, das Grenzen prüft, ist die fehlende Gegenseite zu drei
//! Crates, die Grenzen einhalten.
//!
//! **Konsens-Regeln wie überall:** kein `unsafe`, keine
//! Gleitkomma-Arithmetik. Brüche sind Zähler/Nenner-Paare.

#![deny(unsafe_code)]

pub mod invarianten;
pub mod abstimmung;
pub mod modell;
pub mod registry;
pub mod vorschlag;

pub use invarianten::{pruefe_invarianten, InvariantenBruch, Invariante};
pub use registry::{
    Aenderbarkeit, Parameter, ParameterRegistry, RegistryFehler, Wert,
};
pub use vorschlag::{pruefe_vorschlag, ParameterVorschlag, VorschlagFehler};
