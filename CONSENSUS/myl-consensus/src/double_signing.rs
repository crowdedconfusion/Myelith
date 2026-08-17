//! Double-Signing-Erkennung — Whitepaper Kap. 5.5.
//!
//! Erkennt und bestraft Validator, die in derselben Runde zwei verschiedene
//! Blöcke signiert haben (Double-Signing).
//!
//! **Konsens-Feld:** Die Double-Signing-Erkennung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;
use myl_types::ids::MinerId;
use borsh::{BorshDeserialize, BorshSerialize};

/// Ein Beweis für Double-Signing.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct DoubleSignProof {
    /// Miner-ID des Validators.
    pub miner_id: MinerId,
    /// Runde, in der das Double-Signing auftrat.
    pub round: u64,
    /// Erster Block-Hash.
    pub block_hash_1: Hash,
    /// Zweiter Block-Hash (unterschiedlich vom ersten).
    pub block_hash_2: Hash,
    /// Signatur für den ersten Block.
    pub signature_1: [u8; 96],
    /// Signatur für den zweiten Block.
    pub signature_2: [u8; 96],
}

/// Fehler bei der Double-Signing-Erkennung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleSignError {
    /// Beweise sind ungültig (z.B. gleiche Block-Hashes).
    InvalidProof,
    /// Validator nicht gefunden.
    ValidatorNotFound,
}

impl std::fmt::Display for DoubleSignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProof => write!(f, "Ungültiger Double-Signing-Beweis"),
            Self::ValidatorNotFound => write!(f, "Validator nicht gefunden"),
        }
    }
}

impl std::error::Error for DoubleSignError {}

impl DoubleSignProof {
    /// Validiert den Double-Signing-Beweis.
    ///
    /// **Returns:** `Ok(())` wenn der Beweis gültig ist.
    ///
    /// **Fehler:** `DoubleSignError::InvalidProof` wenn die Block-Hashes gleich sind.
    pub fn validate(&self) -> Result<(), DoubleSignError> {
        // Die beiden Block-Hashes müssen unterschiedlich sein
        if self.block_hash_1 == self.block_hash_2 {
            return Err(DoubleSignError::InvalidProof);
        }

        // Die Signaturen müssen unterschiedlich sein
        if self.signature_1 == self.signature_2 {
            return Err(DoubleSignError::InvalidProof);
        }

        Ok(())
    }

    /// Berechnet den Proof-Hash (für On-Chain-Referenz).
    pub fn hash(&self) -> Hash {
        let mut data = Vec::new();
        data.extend_from_slice(self.miner_id.as_bytes());
        data.extend_from_slice(&self.round.to_le_bytes());
        data.extend_from_slice(self.block_hash_1.as_bytes());
        data.extend_from_slice(self.block_hash_2.as_bytes());
        Hash::sha256(&data)
    }
}

/// Registry für signierte Blöcke pro Validator.
#[derive(Debug, Clone, Default)]
pub struct SignedBlocksRegistry {
    /// Signierte Blöcke pro Validator (MinerId → (Round → Block-Hash)).
    signed_blocks: std::collections::HashMap<MinerId, std::collections::HashMap<u64, Hash>>,
}

impl SignedBlocksRegistry {
    /// Erstellt eine neue, leere Registry.
    pub fn new() -> Self {
        Self {
            signed_blocks: std::collections::HashMap::new(),
        }
    }

    /// Registriert einen signierten Block.
    ///
    /// **Returns:** `Some(DoubleSignProof)` wenn Double-Signing erkannt wurde.
    pub fn register_signed_block(
        &mut self,
        miner_id: MinerId,
        round: u64,
        block_hash: Hash,
    ) -> Option<DoubleSignProof> {
        let validator_blocks = self.signed_blocks.entry(miner_id).or_insert_with(std::collections::HashMap::new);

        // Prüfe, ob der Validator in dieser Runde bereits einen anderen Block signiert hat
        if let Some(existing_hash) = validator_blocks.get(&round) {
            if *existing_hash != block_hash {
                // Double-Signing erkannt!
                // Hinweis: In einer echten Implementierung würden hier die Signaturen gespeichert
                // Für jetzt geben wir einen leeren Proof zurück
                return Some(DoubleSignProof {
                    miner_id,
                    round,
                    block_hash_1: *existing_hash,
                    block_hash_2: block_hash,
                    signature_1: [0u8; 96], // Placeholder
                    signature_2: [0u8; 96], // Placeholder
                });
            }
        }

        // Block registrieren
        validator_blocks.insert(round, block_hash);
        None
    }

    /// Gibt die Anzahl registrierter Validatoren zurück.
    pub fn validator_count(&self) -> usize {
        self.signed_blocks.len()
    }

    /// Gibt die Anzahl signierter Blöcke für einen Validator zurück.
    pub fn signed_block_count(&self, miner_id: &MinerId) -> usize {
        self.signed_blocks.get(miner_id).map_or(0, |m| m.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    #[test]
    fn double_sign_proof_validation_valid() {
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: [1u8; 96],
            signature_2: [2u8; 96],
        };

        assert!(proof.validate().is_ok());
    }

    #[test]
    fn double_sign_proof_validation_same_hash() {
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(1), // Gleicher Hash
            signature_1: [1u8; 96],
            signature_2: [2u8; 96],
        };

        assert!(matches!(proof.validate(), Err(DoubleSignError::InvalidProof)));
    }

    #[test]
    fn double_sign_proof_validation_same_signature() {
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: [1u8; 96],
            signature_2: [1u8; 96], // Gleiche Signatur
        };

        assert!(matches!(proof.validate(), Err(DoubleSignError::InvalidProof)));
    }

    #[test]
    fn double_sign_proof_hash_deterministic() {
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: [1u8; 96],
            signature_2: [2u8; 96],
        };

        let hash1 = proof.hash();
        let hash2 = proof.hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn signed_blocks_registry_no_double_sign() {
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        let result = registry.register_signed_block(miner, 10, test_hash(1));
        assert!(result.is_none());

        assert_eq!(registry.validator_count(), 1);
        assert_eq!(registry.signed_block_count(&miner), 1);
    }

    #[test]
    fn signed_blocks_registry_double_sign_detected() {
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        // Erster Block in Runde 10
        registry.register_signed_block(miner, 10, test_hash(1));

        // Zweiter Block in Runde 10 (unterschiedlicher Hash)
        let proof = registry.register_signed_block(miner, 10, test_hash(2));

        assert!(proof.is_some());
        let proof = proof.unwrap();
        assert_eq!(proof.miner_id, miner);
        assert_eq!(proof.round, 10);
        assert_eq!(proof.block_hash_1, test_hash(1));
        assert_eq!(proof.block_hash_2, test_hash(2));
    }

    #[test]
    fn signed_blocks_registry_same_block_no_double_sign() {
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        // Erster Block in Runde 10
        registry.register_signed_block(miner, 10, test_hash(1));

        // Gleicher Block in Runde 10 (kein Double-Signing)
        let result = registry.register_signed_block(miner, 10, test_hash(1));
        assert!(result.is_none());
    }

    #[test]
    fn signed_blocks_registry_different_rounds() {
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        // Block in Runde 10
        registry.register_signed_block(miner, 10, test_hash(1));

        // Block in Runde 11 (kein Double-Signing, verschiedene Runden)
        let result = registry.register_signed_block(miner, 11, test_hash(2));
        assert!(result.is_none());

        assert_eq!(registry.signed_block_count(&miner), 2);
    }

    #[test]
    fn signed_blocks_registry_multiple_validators() {
        let mut registry = SignedBlocksRegistry::new();
        let miner1 = test_miner(1);
        let miner2 = test_miner(2);

        registry.register_signed_block(miner1, 10, test_hash(1));
        registry.register_signed_block(miner2, 10, test_hash(2));

        assert_eq!(registry.validator_count(), 2);
        assert_eq!(registry.signed_block_count(&miner1), 1);
        assert_eq!(registry.signed_block_count(&miner2), 1);
    }

    #[test]
    fn double_sign_proof_borsh_roundtrip() {
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: [1u8; 96],
            signature_2: [2u8; 96],
        };

        let bytes = borsh::to_vec(&proof).unwrap();
        let decoded: DoubleSignProof = borsh::from_slice(&bytes).unwrap();

        assert_eq!(proof, decoded);
    }
}
