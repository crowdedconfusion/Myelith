//! `myl-consensus` — BFT-Konsens (Whitepaper Kap. 3.5, Anhang A.2).
//!
//! Implementiert Validator-Registrierung, Stake-basierte Komiteewahl,
//! BFT-Blockproduktion (Propose/Vote/Commit), Stimmgewichts-Kopplung,
//! und Double-Signing-Erkennung.
//!
//! **Design-Entscheidung:** malachite-consensus hinter Trait-Grenze,
//! Eigenbau als Fallback.
//!
//! **Konsens-Regeln:** Keine Gleitkomma im Pfad, alle Berechnungen
//! deterministisch und bitgleich.

#![deny(unsafe_code)]

pub mod validator;
pub mod bft;
pub mod block;
pub mod voting_weight;
pub mod double_signing;

pub use validator::{
    Validator, ValidatorRegistry, Committee, CommitteeRole, select_committee,
    ValidatorError,
};
pub use bft::{
    BftState, Propose, Vote, Commit, Round, RoundStatus, BftError, select_leader,
};
pub use block::{
    Block, EpochMeta, Transaction, BurnTx, PoiBundle, Challenge, Verdict,
};
pub use voting_weight::{
    InferenceHistory, calculate_voting_weight, compare_voting_weight,
    DECAY_FACTOR_NUM, DECAY_FACTOR_DEN, MAX_HISTORY_EPOCHS,
};
pub use double_signing::{
    DoubleSignProof, DoubleSignError, SignedBlocksRegistry,
};
