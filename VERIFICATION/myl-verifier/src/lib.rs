//! `myl-verifier` — Verifikations-Subsystem (Whitepaper Kap. 6.4–6.9, Anhang A.4).
//!
//! Implementiert die drei Verifikationsstufen:
//! - **Stufe 1 (Redundanz):** Commitment-Hash-Vergleich zweier Pods
//! - **Stufe 2 (Stichproben):** Bisektions-Spiel bei Abweichung
//! - **Stufe 3 (zkML-Anker):** Zukunftspfad (noch nicht implementiert)
//!
//! sowie das Kontrollsegment-Verfahren (Kap. 6.7).
//!
//! **Abhängigkeiten:**
//! - INTEGER_LLM: Determinismus-Eigenschaft (Kap. 6.2) — Entscheidungspunkt 12.21 ✅
//! - CONSENSUS: BFT-Blockproduktion (Phase 3) für On-Chain-Schiedsrunde
//! - NETWORKING: Verschlüsselte Aktivierungs-Streams (Phase 3) für DA-Fragmente
//!
//! **Konsens-Regeln:** Keine Gleitkomma im Pfad, alle Vergleiche sind binär
//! (gleich/ungleich), keine Schwellenwerte.

#![deny(unsafe_code)]

pub mod redundancy;
pub mod delivery;

pub use redundancy::{
    compare_commitments, CompareResult, RedundancyError, VerificationMode,
};
pub use delivery::{
    decide_delivery, DeliveryDecision, DeliveryError, first_divergence, should_deliver_confirmed,
};
