//! Agent Layer (`myl-agent`), Whitepaper Kap. 8.
//!
//! **Stand: der Anfang.** Diese Komponente trug bis zum 2026-08-28 keine
//! Zeile Code, weil sie als blockiert galt. Nachgesehen war die Blockade
//! gefallen: Alle drei Abhängigkeiten sind erfüllt.
//!
//! Was hier zuerst entsteht, ist **kein Agent, sondern ein Format**: die
//! Antwort auf die Frage, was ein Skill und was ein Werkzeug ist. Sie
//! liegt vor allem anderen, weil jeder weitere Teil sie voraussetzt, und
//! sie braucht keine der offenen Design-Entscheidungen, weil ein Format
//! keine Laufzeit ist.

pub mod beobachtung;
pub mod manifest;
pub mod registratur;
pub mod sitzung;

pub use manifest::{
    Herkunft, ManifestFehler, Skillmanifest, Teil, Werkzeugart, Werkzeugmanifest,
    DST_SKILLMANIFEST, DST_WERKZEUGMANIFEST,
};
pub use beobachtung::{beobachte, Attestierung, Beobachtung, BeobachtungsFehler, GepruefteAttestierung, DST_ATTESTIERUNG};
pub use registratur::{Aufruf, Aufrufbefund, Benutzt, Registratur, Segmentstufe};
pub use sitzung::{darf_verwendet_werden, verlangte_zeugen, Verwendbar};
