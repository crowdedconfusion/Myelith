//! VRF-Seed-Ableitung aus finalisiertem Block (Anhang A.2, Schritt 1).
//!
//! Der Epochenseed ist die Grundlage aller nachfolgenden Scheduler-Schritte.
//! Er wird aus dem finalisierten Block der Vorepoche abgeleitet und ist
//! deterministisch: gleicher Block → gleicher Seed.
//!
//! **Konsens-Feld:** Die Ableitungsfunktion ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Wir verwenden ECVRF (RFC 9381) über curve25519, das bereits
//! in SHARED_TYPES implementiert ist. Der Seed ist die VRF-Ausgabe (beta),
//! 64 Bytes, abgeleitet aus dem Block-Hash als Alpha-String.

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};

use myl_types::hash::Hash;
use myl_types::vrf::{VrfError, VrfOutput, VrfProof, VrfPublicKey, VrfSecretKey};

/// Fehler bei der VRF-Seed-Ableitung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrfSeedError {
    /// VRF-Beweis-Erstellung fehlgeschlagen.
    VrfProofFailed(VrfError),
    /// Block-Hash ist leer (sollte nie passieren).
    EmptyBlockHash,
}

impl std::fmt::Display for VrfSeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VrfProofFailed(e) => write!(f, "VRF-Beweis fehlgeschlagen: {}", e),
            Self::EmptyBlockHash => write!(f, "Block-Hash ist leer"),
        }
    }
}

impl std::error::Error for VrfSeedError {}

impl From<VrfError> for VrfSeedError {
    fn from(e: VrfError) -> Self {
        Self::VrfProofFailed(e)
    }
}

/// Epochenseed: 64-Byte VRF-Ausgabe, abgeleitet aus dem Block-Hash.
///
/// Der Seed ist die Grundlage aller nachfolgenden Scheduler-Schritte
/// (Miner-Filterung, Geo-Clustering, Shard-Zuweisung, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct EpochSeed {
    /// Die 64-Byte VRF-Ausgabe (beta).
    pub beta: [u8; 64],
    /// Epoche, für die dieser Seed gilt.
    pub epoch: u64,
    /// Block-Hash der Vorepoche (zur Nachvollziehbarkeit).
    pub prev_block_hash: Hash,
}

impl EpochSeed {
    /// Berechnet einen Hash des Seeds (für Caching und Deduplizierung).
    pub fn hash(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Borsh-Serialisierung sollte nicht fehlschlagen");
        Hash::sha256(&bytes)
    }

    /// Gibt die ersten 32 Bytes des Seeds als u64-Array zurück (für Fisher-Yates).
    ///
    /// Nützlich für deterministische Zufallszahlen in nachfolgenden Schritten.
    pub fn as_random_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&self.beta[..32]);
        bytes
    }
}

/// Leitet den Epochenseed aus dem finalisierten Block der Vorepoche ab.
///
/// **Algorithmus (Anhang A.2, Schritt 1):**
/// 1. Nimm den Block-Hash der Vorepoche (32 Bytes)
/// 2. Verwende den VRF-Schlüssel des Blockproduzenten
/// 3. Erstelle einen VRF-Beweis über den Block-Hash (als Alpha-String)
/// 4. Die VRF-Ausgabe (beta, 64 Bytes) ist der Epochenseed
///
/// **Determinismus:** Gleicher Block-Hash + gleicher VRF-Schlüssel → gleicher Seed.
/// Jeder Node kann den Seed nachrechnen, wenn er den Block-Hash und den öffentlichen
/// VRF-Schlüssel des Blockproduzenten kennt.
///
/// **Parameter:**
/// - `prev_block_hash`: Hash des finalisierten Blocks der Vorepoche
/// - `vrf_sk`: VRF-Geheimschlüssel des Blockproduzenten
/// - `epoch`: Epoche, für die der Seed abgeleitet wird
///
/// **Returns:** `EpochSeed` mit der 64-Byte VRF-Ausgabe
pub fn derive_epoch_seed(
    prev_block_hash: Hash,
    vrf_sk: &VrfSecretKey,
    epoch: u64,
) -> Result<EpochSeed, VrfSeedError> {
    // Block-Hash darf nicht leer sein
    if prev_block_hash.as_bytes() == &[0u8; 32] {
        return Err(VrfSeedError::EmptyBlockHash);
    }

    // VRF-Beweis über den Block-Hash erstellen
    // Der Block-Hash wird als Alpha-String verwendet (RFC 9381)
    let alpha = prev_block_hash.as_bytes();
    let (proof, output) = vrf_sk.prove(alpha)?;

    // Epochenseed konstruieren
    Ok(EpochSeed {
        beta: output.beta,
        epoch,
        prev_block_hash,
    })
}

/// Verifiziert einen Epochenseed gegen den öffentlichen VRF-Schlüssel.
///
/// Jeder Node kann den Seed verifizieren, wenn er:
/// - Den Block-Hash der Vorepoche kennt
/// - Den öffentlichen VRF-Schlüssel des Blockproduzenten kennt
/// - Den VRF-Beweis hat (muss mit dem Seed gespeichert werden)
///
/// **Returns:** `true` wenn der Seed gültig ist, `false` sonst.
pub fn verify_epoch_seed(
    seed: &EpochSeed,
    proof: &VrfProof,
    vrf_pk: &VrfPublicKey,
) -> bool {
    // VRF-Beweis verifizieren
    let alpha = seed.prev_block_hash.as_bytes();
    match vrf_pk.verify(alpha, proof) {
        Ok(output) => output.beta == seed.beta,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vrf_keypair() -> (VrfSecretKey, VrfPublicKey) {
        let seed = [42u8; 32];
        let sk = VrfSecretKey::from_seed(seed);
        let pk = sk.public_key();
        (sk, pk)
    }

    #[test]
    fn derive_seed_deterministic() {
        let (sk, _) = test_vrf_keypair();
        let block_hash = Hash::sha256(b"test-block-1");

        let seed1 = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");
        let seed2 = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");

        assert_eq!(seed1.beta, seed2.beta);
        assert_eq!(seed1.epoch, seed2.epoch);
        assert_eq!(seed1.prev_block_hash, seed2.prev_block_hash);
    }

    #[test]
    fn derive_seed_different_blocks() {
        let (sk, _) = test_vrf_keypair();
        let block_hash1 = Hash::sha256(b"test-block-1");
        let block_hash2 = Hash::sha256(b"test-block-2");

        let seed1 = derive_epoch_seed(block_hash1, &sk, 1).expect("seed derivation");
        let seed2 = derive_epoch_seed(block_hash2, &sk, 1).expect("seed derivation");

        assert_ne!(seed1.beta, seed2.beta);
    }

    #[test]
    fn derive_seed_different_epochs() {
        let (sk, _) = test_vrf_keypair();
        let block_hash = Hash::sha256(b"test-block-1");

        let seed1 = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");
        let seed2 = derive_epoch_seed(block_hash, &sk, 2).expect("seed derivation");

        // Beta sollte gleich sein (gleicher Block-Hash), aber Epoche unterschiedlich
        assert_eq!(seed1.beta, seed2.beta);
        assert_ne!(seed1.epoch, seed2.epoch);
    }

    #[test]
    fn derive_seed_empty_block_hash_rejected() {
        let (sk, _) = test_vrf_keypair();
        let empty_hash = Hash::from_bytes([0u8; 32]);

        let result = derive_epoch_seed(empty_hash, &sk, 1);
        assert!(matches!(result, Err(VrfSeedError::EmptyBlockHash)));
    }

    #[test]
    fn seed_hash_deterministic() {
        let (sk, _) = test_vrf_keypair();
        let block_hash = Hash::sha256(b"test-block-1");

        let seed = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");
        let hash1 = seed.hash();
        let hash2 = seed.hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn seed_borsh_roundtrip() {
        let (sk, _) = test_vrf_keypair();
        let block_hash = Hash::sha256(b"test-block-1");

        let seed = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");
        let bytes = borsh::to_vec(&seed).expect("serialization");
        let decoded: EpochSeed = borsh::from_slice(&bytes).expect("deserialization");

        assert_eq!(seed, decoded);
    }

    #[test]
    fn seed_as_random_bytes() {
        let (sk, _) = test_vrf_keypair();
        let block_hash = Hash::sha256(b"test-block-1");

        let seed = derive_epoch_seed(block_hash, &sk, 1).expect("seed derivation");
        let random_bytes = seed.as_random_bytes();

        // Sollte die ersten 32 Bytes von beta sein
        assert_eq!(&random_bytes[..], &seed.beta[..32]);
    }
}
