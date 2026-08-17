//! Double-Signing-Erkennung — Whitepaper Kap. 5.5.
//!
//! Erkennt und bestraft Validator, die in derselben Runde zwei verschiedene
//! Blöcke signiert haben (Double-Signing).
//!
//! **Beweislast:** Ein Double-Signing-Beweis ist nur dann etwas wert,
//! wenn er von jedem Dritten **nachprüfbar** ist. Ein Beweis besteht
//! daher aus zwei echten BLS-Signaturen desselben Validators über
//! dieselbe Runde, aber verschiedene Block-Hashes. Die Prüfung
//! ([`DoubleSignProof::verify`]) verlangt zwingend den öffentlichen
//! Schlüssel des Beschuldigten — eine Prüfung ohne Schlüssel kann
//! Double-Signing nicht von Verleumdung unterscheiden und darf es
//! daher auch nicht versuchen.
//!
//! **Konsens-Feld:** Die Double-Signing-Erkennung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::signing::vote_message;
use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

/// Ein Beweis für Double-Signing.
///
/// Enthält die beiden tatsächlich abgegebenen Signaturen. Ein Beweis
/// ohne echte Signaturen ist wertlos und wird von [`Self::verify`]
/// abgelehnt.
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
    /// BLS-Signatur des Validators über `(Runde, block_hash_1)`.
    pub signature_1: BlsSignature,
    /// BLS-Signatur des Validators über `(Runde, block_hash_2)`.
    pub signature_2: BlsSignature,
}

/// Fehler bei der Double-Signing-Erkennung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleSignError {
    /// Die beiden Block-Hashes sind identisch — kein Double-Signing.
    IdenticalBlocks,
    /// Die beiden Signaturen sind identisch — kein Double-Signing.
    IdenticalSignatures,
    /// Mindestens eine der beiden Signaturen ist unter dem angegebenen
    /// öffentlichen Schlüssel nicht gültig.
    InvalidSignature,
    /// Validator nicht gefunden.
    ValidatorNotFound,
}

impl std::fmt::Display for DoubleSignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdenticalBlocks => {
                write!(f, "Kein Double-Signing: beide Block-Hashes identisch")
            }
            Self::IdenticalSignatures => {
                write!(f, "Kein Double-Signing: beide Signaturen identisch")
            }
            Self::InvalidSignature => {
                write!(f, "Ungültige Signatur im Double-Signing-Beweis")
            }
            Self::ValidatorNotFound => write!(f, "Validator nicht gefunden"),
        }
    }
}

impl std::error::Error for DoubleSignError {}

impl DoubleSignProof {
    /// Prüft den Double-Signing-Beweis vollständig.
    ///
    /// Ein Beweis gilt genau dann, wenn **alle** folgenden Punkte gelten:
    /// 1. Die beiden Block-Hashes sind verschieden (sonst kein Konflikt).
    /// 2. Die beiden Signaturen sind verschieden.
    /// 3. `signature_1` ist eine gültige BLS-Signatur von `pubkey` über
    ///    die kanonische Vote-Botschaft zu `(round, block_hash_1)`.
    /// 4. `signature_2` gilt entsprechend für `block_hash_2`.
    ///
    /// Punkt 3 und 4 sind der eigentliche Beweis: nur der Inhaber des
    /// privaten Schlüssels kann beide Signaturen erzeugt haben. Ohne
    /// diese Prüfung könnte jeder Beliebige einen „Beweis" gegen jeden
    /// beliebigen Validator fabrizieren.
    ///
    /// **Parameter:**
    /// - `pubkey`: öffentlicher BLS-Schlüssel des beschuldigten Validators
    ///   (aus der [`crate::ValidatorRegistry`])
    ///
    /// **Returns:** `Ok(())`, wenn das Double-Signing bewiesen ist.
    pub fn verify(&self, pubkey: &BlsPublicKey) -> Result<(), DoubleSignError> {
        if self.block_hash_1 == self.block_hash_2 {
            return Err(DoubleSignError::IdenticalBlocks);
        }

        if self.signature_1 == self.signature_2 {
            return Err(DoubleSignError::IdenticalSignatures);
        }

        let msg_1 = vote_message(self.round, &self.block_hash_1);
        if !pubkey.verify(&msg_1, &self.signature_1) {
            return Err(DoubleSignError::InvalidSignature);
        }

        let msg_2 = vote_message(self.round, &self.block_hash_2);
        if !pubkey.verify(&msg_2, &self.signature_2) {
            return Err(DoubleSignError::InvalidSignature);
        }

        Ok(())
    }

    /// Berechnet den Proof-Hash (für On-Chain-Referenz).
    ///
    /// Bindet auch die Signaturen ein, damit zwei Beweise mit gleichen
    /// Blöcken, aber verschiedenen Signaturen unterscheidbar bleiben.
    pub fn hash(&self) -> Hash {
        let mut data = Vec::new();
        data.extend_from_slice(self.miner_id.as_bytes());
        data.extend_from_slice(&self.round.to_le_bytes());
        data.extend_from_slice(self.block_hash_1.as_bytes());
        data.extend_from_slice(self.block_hash_2.as_bytes());
        data.extend_from_slice(&self.signature_1.0);
        data.extend_from_slice(&self.signature_2.0);
        Hash::sha256(&data)
    }
}

/// Registry für signierte Blöcke pro Validator.
///
/// Hält zu jedem `(Validator, Runde)` den signierten Block-Hash **und
/// die zugehörige Signatur**. Nur so kann bei einem Konflikt ein
/// nachprüfbarer Beweis entstehen; ohne die gespeicherte Signatur
/// wäre die Erkennung eine Behauptung ohne Beleg.
#[derive(Debug, Clone, Default)]
pub struct SignedBlocksRegistry {
    /// Signierte Blöcke pro Validator: MinerId → (Runde → (Hash, Signatur)).
    signed_blocks:
        std::collections::HashMap<MinerId, std::collections::HashMap<u64, (Hash, BlsSignature)>>,
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
    /// **Parameter:**
    /// - `miner_id`: Validator, der signiert hat
    /// - `round`: Rundennummer
    /// - `block_hash`: signierter Block
    /// - `signature`: die abgegebene BLS-Signatur über die kanonische
    ///   Vote-Botschaft (siehe [`crate::signing::vote_message`])
    ///
    /// **Returns:** `Some(DoubleSignProof)`, wenn derselbe Validator in
    /// derselben Runde bereits einen **anderen** Block signiert hat. Der
    /// zurückgegebene Beweis enthält beide echten Signaturen und besteht
    /// [`DoubleSignProof::verify`] gegen den Schlüssel des Validators.
    pub fn register_signed_block(
        &mut self,
        miner_id: MinerId,
        round: u64,
        block_hash: Hash,
        signature: BlsSignature,
    ) -> Option<DoubleSignProof> {
        let validator_blocks = self.signed_blocks.entry(miner_id).or_default();

        // Prüfe, ob der Validator in dieser Runde bereits einen anderen Block signiert hat
        if let Some((existing_hash, existing_sig)) = validator_blocks.get(&round) {
            if *existing_hash != block_hash {
                // Double-Signing erkannt — mit beiden echten Signaturen.
                return Some(DoubleSignProof {
                    miner_id,
                    round,
                    block_hash_1: *existing_hash,
                    block_hash_2: block_hash,
                    signature_1: *existing_sig,
                    signature_2: signature,
                });
            }
            // Gleicher Block erneut signiert: kein Konflikt, Ersteintrag behalten.
            return None;
        }

        validator_blocks.insert(round, (block_hash, signature));
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
    use myl_types::bls::BlsSecretKey;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    /// Deterministisches Schlüsselpaar für Tests.
    fn keypair(byte: u8) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::key_gen(&[byte; 32]).expect("key_gen");
        let pk = sk.public_key().expect("public_key");
        (sk, pk)
    }

    fn sign_vote(sk: &BlsSecretKey, round: u64, hash: &Hash) -> BlsSignature {
        sk.sign(&vote_message(round, hash)).expect("sign")
    }

    #[test]
    fn echter_beweis_wird_akzeptiert() {
        let (sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sign_vote(&sk, 10, &test_hash(1)),
            signature_2: sign_vote(&sk, 10, &test_hash(2)),
        };

        assert!(proof.verify(&pk).is_ok());
    }

    #[test]
    fn gleiche_blockhashes_sind_kein_double_signing() {
        let (sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(1),
            signature_1: sign_vote(&sk, 10, &test_hash(1)),
            signature_2: sign_vote(&sk, 10, &test_hash(1)),
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::IdenticalBlocks));
    }

    #[test]
    fn gleiche_signaturen_sind_kein_double_signing() {
        let (sk, pk) = keypair(1);
        let sig = sign_vote(&sk, 10, &test_hash(1));
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sig,
            signature_2: sig,
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::IdenticalSignatures));
    }

    #[test]
    fn erfundene_signaturen_werden_abgelehnt() {
        // Der zentrale Angriff: Ohne BLS-Pruefung koennte jeder einen
        // "Beweis" gegen jeden Validator fabrizieren.
        let (_sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: BlsSignature([1u8; 96]),
            signature_2: BlsSignature([2u8; 96]),
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::InvalidSignature));
    }

    #[test]
    fn nullsignaturen_werden_abgelehnt() {
        let (_sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: BlsSignature([0u8; 96]),
            signature_2: BlsSignature([1u8; 96]),
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::InvalidSignature));
    }

    #[test]
    fn signatur_eines_fremden_schluessels_wird_abgelehnt() {
        let (sk_a, _pk_a) = keypair(1);
        let (_sk_b, pk_b) = keypair(2);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sign_vote(&sk_a, 10, &test_hash(1)),
            signature_2: sign_vote(&sk_a, 10, &test_hash(2)),
        };

        assert_eq!(proof.verify(&pk_b), Err(DoubleSignError::InvalidSignature));
    }

    #[test]
    fn signatur_aus_anderer_runde_wird_abgelehnt() {
        // Ein Validator, der in Runde 10 und Runde 11 je einen Block
        // signiert, hat nichts Verbotenes getan.
        let (sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sign_vote(&sk, 10, &test_hash(1)),
            signature_2: sign_vote(&sk, 11, &test_hash(2)),
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::InvalidSignature));
    }

    #[test]
    fn commit_signatur_taugt_nicht_als_vote_beweis() {
        // Domain-Separation: eine Commit-Signatur darf nicht als Beleg
        // fuer eine Vote durchgehen.
        use crate::signing::commit_message;
        let (sk, pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sk.sign(&commit_message(10, &test_hash(1))).unwrap(),
            signature_2: sign_vote(&sk, 10, &test_hash(2)),
        };

        assert_eq!(proof.verify(&pk), Err(DoubleSignError::InvalidSignature));
    }

    #[test]
    fn proof_hash_ist_deterministisch() {
        let (sk, _pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sign_vote(&sk, 10, &test_hash(1)),
            signature_2: sign_vote(&sk, 10, &test_hash(2)),
        };

        assert_eq!(proof.hash(), proof.hash());
    }

    #[test]
    fn registry_ohne_double_signing() {
        let (sk, _pk) = keypair(1);
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        let result =
            registry.register_signed_block(miner, 10, test_hash(1), sign_vote(&sk, 10, &test_hash(1)));
        assert!(result.is_none());

        assert_eq!(registry.validator_count(), 1);
        assert_eq!(registry.signed_block_count(&miner), 1);
    }

    /// Die Regression zu Fund A4: Der von der Erkennung erzeugte Beweis
    /// muss die eigene Pruefung bestehen. Vorher enthielt er
    /// Platzhalter-Nullsignaturen und wurde von `validate()` verworfen —
    /// die Erkennung konnte also nie einen verwertbaren Beweis liefern.
    #[test]
    fn erkannter_beweis_besteht_die_eigene_pruefung() {
        let (sk, pk) = keypair(1);
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        registry.register_signed_block(miner, 10, test_hash(1), sign_vote(&sk, 10, &test_hash(1)));
        let proof = registry
            .register_signed_block(miner, 10, test_hash(2), sign_vote(&sk, 10, &test_hash(2)))
            .expect("Double-Signing muss erkannt werden");

        assert_eq!(proof.miner_id, miner);
        assert_eq!(proof.round, 10);
        assert_eq!(proof.block_hash_1, test_hash(1));
        assert_eq!(proof.block_hash_2, test_hash(2));
        assert!(
            proof.verify(&pk).is_ok(),
            "der erkannte Beweis muss gegen den Schluessel des Validators gelten"
        );
    }

    #[test]
    fn gleicher_block_erneut_signiert_ist_kein_double_signing() {
        let (sk, _pk) = keypair(1);
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);
        let sig = sign_vote(&sk, 10, &test_hash(1));

        registry.register_signed_block(miner, 10, test_hash(1), sig);
        let result = registry.register_signed_block(miner, 10, test_hash(1), sig);
        assert!(result.is_none());
        assert_eq!(registry.signed_block_count(&miner), 1);
    }

    #[test]
    fn verschiedene_runden_sind_kein_double_signing() {
        let (sk, _pk) = keypair(1);
        let mut registry = SignedBlocksRegistry::new();
        let miner = test_miner(1);

        registry.register_signed_block(miner, 10, test_hash(1), sign_vote(&sk, 10, &test_hash(1)));
        let result =
            registry.register_signed_block(miner, 11, test_hash(2), sign_vote(&sk, 11, &test_hash(2)));

        assert!(result.is_none());
        assert_eq!(registry.signed_block_count(&miner), 2);
    }

    #[test]
    fn mehrere_validatoren_werden_getrennt_gefuehrt() {
        let (sk1, _) = keypair(1);
        let (sk2, _) = keypair(2);
        let mut registry = SignedBlocksRegistry::new();
        let miner1 = test_miner(1);
        let miner2 = test_miner(2);

        registry.register_signed_block(miner1, 10, test_hash(1), sign_vote(&sk1, 10, &test_hash(1)));
        registry.register_signed_block(miner2, 10, test_hash(2), sign_vote(&sk2, 10, &test_hash(2)));

        assert_eq!(registry.validator_count(), 2);
        assert_eq!(registry.signed_block_count(&miner1), 1);
        assert_eq!(registry.signed_block_count(&miner2), 1);
    }

    #[test]
    fn proof_borsh_roundtrip() {
        let (sk, _pk) = keypair(1);
        let proof = DoubleSignProof {
            miner_id: test_miner(1),
            round: 10,
            block_hash_1: test_hash(1),
            block_hash_2: test_hash(2),
            signature_1: sign_vote(&sk, 10, &test_hash(1)),
            signature_2: sign_vote(&sk, 10, &test_hash(2)),
        };

        let bytes = borsh::to_vec(&proof).unwrap();
        let decoded: DoubleSignProof = borsh::from_slice(&bytes).unwrap();

        assert_eq!(proof, decoded);
    }
}
