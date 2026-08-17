//! `myl-types` — protokollweite Kern-Datentypen und Primitiven für Myelith.
//!
//! Referenzimplementierung von Whitepaper Anhang A.1. Jede andere
//! Myelith-Komponente importiert diese Typen, statt sie neu zu
//! definieren.
//!
//! Festgelegte Protokoll-Parameter (Design-Entscheidungen vom 2026-08-12,
//! dokumentiert in `SHARED_TYPES/README/Fahrplan-v1.md`):
//!
//! - **Hash:** SHA-256 — ein Hash für das gesamte Protokoll
//!   (konsistent mit den θ_v-/Artefakt-Hashes in INTEGER_LLM).
//!   Quanten-Einordnung: Grover-resistent (~128 bit), unkritisch.
//! - **VRF:** ECVRF (RFC 9381) über curve25519, mit dokumentiertem
//!   Post-Quantum-Migrationspfad (Algorithms-Versionsfeld in `VrfOutput`).
//!   Quanten-Einordnung: Shor-anfällig, Migrationspunkt.
//! - **Signaturen:** BLS12-381 (Anhang A.1). Quanten-Einordnung:
//!   Shor-anfällig, Migrationspunkt.
//! - **Serialisierung:** Borsh — deterministisch und kanonisch, weil
//!   Hashes über serialisierte Strukturen gebildet werden.
//!
//! Dieses Crate darf kein `unsafe` enthalten und keine Gleitkomma-Arithmetik:
//! Konsens-Determinismus ist Verfassungsrang (Whitepaper Kap. 10.3).

#![deny(unsafe_code)]

pub mod bls;
pub mod challenge;
pub mod core_types;
pub mod hash;
pub mod ids;
pub mod latency_attest;
pub mod merkle;
pub mod node_metadata;
pub mod protocol;
pub mod seed_rng;
pub mod vrf;

pub use bls::{
    aggregate_signatures, aggregate_verify, fast_aggregate_verify, BlsAggregateSignature,
    BlsError, BlsPublicKey, BlsSecretKey, BlsSignature, BLS_DST,
};
pub use challenge::{Challenge, ChallengeStructureError};
pub use core_types::{segments_root, InferenceCredit, PoIBundle, Segment};
pub use hash::Hash;
pub use ids::{
    ActivationHash, Address, EpochId, IdParseError, MerkleRoot, MinerId, PodId, SegmentId,
    ID_LEN,
};
pub use latency_attest::{
    BlsSignatureBytes, LatencyAttest, LatencyAttestError, LatencyGraph, PeerIdBytes,
};
pub use merkle::{leaf_hash, node_hash, MerkleError, MerkleProof, MerkleTree};
pub use node_metadata::{Asn, DiversityChecker, GeoRegion, NodeMetadata, NodeMetadataError};
pub use seed_rng::{deterministic_shuffle, weighted_sample_without_replacement, SeedRng};
pub use vrf::{VrfError, VrfOutput, VrfProof, VrfPublicKey, VrfSecretKey};
