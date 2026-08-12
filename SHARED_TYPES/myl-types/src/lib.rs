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

pub mod hash;
pub mod merkle;
pub mod protocol;
pub mod vrf;

pub use hash::Hash;
pub use merkle::{leaf_hash, node_hash, MerkleError, MerkleProof, MerkleTree};
pub use vrf::{VrfError, VrfOutput, VrfProof, VrfPublicKey, VrfSecretKey};
