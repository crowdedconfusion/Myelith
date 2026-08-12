//! `myl-ledger` — L1-Ledger: Kontenmodell und deterministische
//! Zustandsübergänge (Whitepaper Anhang A.5).
//!
//! Konsens-Grundregeln dieses Crates:
//! - Jeder Zustandsübergang ist eine **reine Funktion**
//!   `(State, Übergang) → State` ohne versteckten globalen Zustand
//!   (Akzeptanzkriterium Phase 1).
//! - **Deterministische Ordnung:** Konten stehen in einer `BTreeMap`
//!   (sortiert nach Adresse) — keine `HashMap`, deren Iterations-
//!   reihenfolge nicht festgelegt ist. Gleiche Übergangsfolge ⇒
//!   bitgleicher Endzustand auf jedem Node.
//! - **Ganzzahligkeit:** alle Beträge sind `u64`-Kleinstbeträge,
//!   Divisionen runden abwärts (floor) — Gleitkomma existiert hier
//!   nicht (Design-Vorgabe TOKENOMICS, Kap. 5).
//! - **Überlaufsicherheit:** gerechnet wird mit saturierender
//!   Arithmetik und expliziten Prüfungen; ein Übergang, der ein
//!   Konto über `u64::MAX` bringen würde, schlägt fehl statt
//!   überzulaufen.
//!
//! Einheiten: Beträge sind Kleinstbeträge (die Untereinheit-Festlegung
//! erfolgt in TOKENOMICS bzw. zu Genesis); der Credit-Preis
//! `credit_price` ist die Anzahl Kleinstbeträge je vTFE-Einheit und
//! wird später von TOKENOMICS über den Preis-Übergang aktualisiert.

#![deny(unsafe_code)]

pub mod state;
pub mod transitions;

pub use state::{AccountState, LedgerState};
pub use transitions::{
    apply_verdict, burn_to_credits, credit_spend, SlashParams, TransitionError, Verdict,
    VerdictEffect, VerdictOutcome,
};
