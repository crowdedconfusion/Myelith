//! `myl-consensus` — BFT-Konsens (Whitepaper Kap. 3.5, Anhang A.2).
//!
//! Implementiert Validator-Registrierung, Stake-basierte Komiteewahl,
//! BFT-Blockproduktion (Propose/Vote/Commit), Stimmgewichts-Kopplung,
//! Double-Signing-Erkennung sowie Rundenwechsel mit Sperrmechanik
//! ([`round_change`]) — letztere stellt die Liveness her, die das
//! Ein-Runden-Protokoll aus [`bft`] allein nicht leisten kann.
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
pub mod signing;
pub mod voting_weight;
pub mod double_signing;
pub mod round_change;
pub mod poi;
pub mod epoch_close;
pub mod da;

pub use validator::{
    Validator, ValidatorRegistry, Committee, CommitteeRole, select_committee,
    is_in_committee, ValidatorError, VotingMember, VotingSet,
    ARBITER_COUNT, COMMITTEE_SIZE, MIN_STAKE,
};
pub use bft::{
    BftState, Konsensnachricht, Propose, Vote, Commit, Round, RoundStatus, BftError, select_leader,
};
pub use block::{
    epoche_fuer_hoehe, transaktionsbytes, Anweisung, Block, BlockHeader, GepruefteTransaktion,
    Transaktion, TransaktionsFehler, BLOECKE_JE_EPOCHE, DST_TRANSAKTION,
};
// Die Protokolltypen des Blockinhalts kommen aus den kanonischen Crates
// (Fund A8) — hier nur re-exportiert, nicht neu definiert.
pub use myl_ledger::transitions::{Verdict, VerdictOutcome};
pub use myl_types::challenge::Challenge;
pub use myl_types::core_types::PoIBundle;
pub use voting_weight::{
    InferenceHistory, calculate_voting_weight, compare_voting_weight,
    DECAY_FACTOR_NUM, DECAY_FACTOR_DEN, MAX_HISTORY_EPOCHS, VTFE_UNIT,
    calculate_voting_weight_mit, StimmgewichtsParameter, ARBEITSBEZUG_VORGABE,
    HOECHSTFAKTOR_VORGABE,
};
pub use signing::{
    signable_bytes, propose_message, vote_message, commit_message,
    propose_pol_message,
    DST_PROPOSE, DST_VOTE, DST_COMMIT, DST_PROPOSE_POL,
};
pub use da::{commit_segment, DaCommitment, DaError, DaStore};
pub use epoch_close::{
    close_epoch, EpochClosing, EpochError, PodAgreement, RefutedSegment,
    DEFAULT_DISPUTE_EPOCHS,
};
pub use poi::{
    PoIError, PoIRegistry, PodMembership, bundle_message, poi_bundle_message,
    verify_bundle_signature, DST_POI_BUNDLE,
};
pub use round_change::{
    Lock, PolkaCertificate, RoundChange, RoundDriver, RoundError, TimeoutConfig,
    DEFAULT_TIMEOUT_COMMIT_MS, DEFAULT_TIMEOUT_DELTA_MS, DEFAULT_TIMEOUT_PROPOSE_MS,
    DEFAULT_TIMEOUT_VOTE_MS,
};
pub use double_signing::{
    DoubleSignProof, DoubleSignError, SignedBlocksRegistry,
};
