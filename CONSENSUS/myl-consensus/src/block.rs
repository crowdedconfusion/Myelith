//! Blockinhalt — Whitepaper Anhang A.5.
//!
//! Definiert die Struktur eines Blocks im Myelith-Netzwerk:
//! { txs, poi_bundles, challenges, verdicts, epoch_meta }
//!
//! **Konsens-Feld:** Die Block-Struktur ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;
use myl_types::ids::{MinerId, SegmentId};
use borsh::{BorshDeserialize, BorshSerialize};

/// Epochen-Metadaten im Block.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct EpochMeta {
    /// Epochennummer.
    pub epoch: u64,
    /// Vorheriger Block-Hash.
    pub prev_block_hash: Hash,
    /// Zeitstempel (Unix-Millisekunden).
    pub timestamp_ms: u64,
}

/// Eine Burn-Transaktion (MYL → Credits).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BurnTx {
    /// Absender-Adresse (MinerId).
    pub sender: MinerId,
    /// Betrag in MYL-Kleinstbeträgen.
    pub amount: u64,
}

/// Ein PoI-Bundle (Proof-of-Inference).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PoiBundle {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Commitment-Hash.
    pub commitment_hash: Hash,
    /// Pod-ID.
    pub pod_id: [u8; 32],
    /// Aggregierte BLS-Signatur der Pod-Mitglieder.
    pub signature: [u8; 96],
}

/// Eine Challenge (Start des Bisektions-Spiels).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Challenge {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Erste abweichende Position.
    pub first_divergence: usize,
    /// Challenger (MinerId).
    pub challenger: MinerId,
    /// Angeklagter (MinerId).
    pub accused: MinerId,
}

/// Ein Verdict (Ergebnis des Bisektions-Spiels).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verdict {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Gewinner (MinerId).
    pub winner: MinerId,
    /// Verlierer (MinerId).
    pub loser: MinerId,
    /// Slash-Betrag.
    pub slash_amount: u64,
}

/// Transaktionstypen im Block.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Transaction {
    /// Burn-Transaktion (MYL → Credits).
    Burn(BurnTx),
}

/// Ein Block im Myelith-Netzwerk.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Block {
    /// Epochen-Metadaten.
    pub epoch_meta: EpochMeta,
    /// Transaktionen.
    pub txs: Vec<Transaction>,
    /// PoI-Bundles.
    pub poi_bundles: Vec<PoiBundle>,
    /// Challenges.
    pub challenges: Vec<Challenge>,
    /// Verdicts.
    pub verdicts: Vec<Verdict>,
}

impl Block {
    /// Erstellt einen neuen, leeren Block.
    pub fn new(epoch_meta: EpochMeta) -> Self {
        Self {
            epoch_meta,
            txs: Vec::new(),
            poi_bundles: Vec::new(),
            challenges: Vec::new(),
            verdicts: Vec::new(),
        }
    }

    /// Fügt eine Transaktion hinzu.
    pub fn add_transaction(&mut self, tx: Transaction) {
        self.txs.push(tx);
    }

    /// Fügt ein PoI-Bundle hinzu.
    pub fn add_poi_bundle(&mut self, bundle: PoiBundle) {
        self.poi_bundles.push(bundle);
    }

    /// Fügt eine Challenge hinzu.
    pub fn add_challenge(&mut self, challenge: Challenge) {
        self.challenges.push(challenge);
    }

    /// Fügt ein Verdict hinzu.
    pub fn add_verdict(&mut self, verdict: Verdict) {
        self.verdicts.push(verdict);
    }

    /// Berechnet den Block-Hash (SHA-256 über serialisierte Daten).
    pub fn hash(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Borsh-Serialisierung sollte nicht fehlschlagen");
        Hash::sha256(&bytes)
    }

    /// Gibt die Gesamtanzahl der Einträge zurück.
    pub fn total_entries(&self) -> usize {
        self.txs.len()
            + self.poi_bundles.len()
            + self.challenges.len()
            + self.verdicts.len()
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

    fn test_segment_id(byte: u8) -> SegmentId {
        SegmentId::new([byte; 32])
    }

    #[test]
    fn block_creation() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let block = Block::new(meta.clone());
        assert_eq!(block.epoch_meta, meta);
        assert!(block.txs.is_empty());
        assert!(block.poi_bundles.is_empty());
        assert!(block.challenges.is_empty());
        assert!(block.verdicts.is_empty());
    }

    #[test]
    fn add_transaction() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);
        let tx = Transaction::Burn(BurnTx {
            sender: test_miner(1),
            amount: 1_000_000,
        });

        block.add_transaction(tx.clone());
        assert_eq!(block.txs.len(), 1);
        assert_eq!(block.txs[0], tx);
    }

    #[test]
    fn add_poi_bundle() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);
        let bundle = PoiBundle {
            segment_id: test_segment_id(1),
            commitment_hash: test_hash(2),
            pod_id: [3u8; 32],
            signature: [4u8; 96],
        };

        block.add_poi_bundle(bundle.clone());
        assert_eq!(block.poi_bundles.len(), 1);
        assert_eq!(block.poi_bundles[0], bundle);
    }

    #[test]
    fn add_challenge() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);
        let challenge = Challenge {
            segment_id: test_segment_id(1),
            first_divergence: 5,
            challenger: test_miner(2),
            accused: test_miner(3),
        };

        block.add_challenge(challenge.clone());
        assert_eq!(block.challenges.len(), 1);
        assert_eq!(block.challenges[0], challenge);
    }

    #[test]
    fn add_verdict() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);
        let verdict = Verdict {
            segment_id: test_segment_id(1),
            winner: test_miner(2),
            loser: test_miner(3),
            slash_amount: 1_000_000,
        };

        block.add_verdict(verdict.clone());
        assert_eq!(block.verdicts.len(), 1);
        assert_eq!(block.verdicts[0], verdict);
    }

    #[test]
    fn block_hash_deterministic() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let block1 = Block::new(meta.clone());
        let block2 = Block::new(meta);

        assert_eq!(block1.hash(), block2.hash());
    }

    #[test]
    fn block_hash_different_for_different_blocks() {
        let meta1 = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let meta2 = EpochMeta {
            epoch: 11,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let block1 = Block::new(meta1);
        let block2 = Block::new(meta2);

        assert_ne!(block1.hash(), block2.hash());
    }

    #[test]
    fn total_entries() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);

        // 2 Transaktionen
        block.add_transaction(Transaction::Burn(BurnTx {
            sender: test_miner(1),
            amount: 1_000_000,
        }));
        block.add_transaction(Transaction::Burn(BurnTx {
            sender: test_miner(2),
            amount: 2_000_000,
        }));

        // 1 PoI-Bundle
        block.add_poi_bundle(PoiBundle {
            segment_id: test_segment_id(1),
            commitment_hash: test_hash(2),
            pod_id: [3u8; 32],
            signature: [4u8; 96],
        });

        // 1 Challenge
        block.add_challenge(Challenge {
            segment_id: test_segment_id(2),
            first_divergence: 5,
            challenger: test_miner(3),
            accused: test_miner(4),
        });

        // 1 Verdict
        block.add_verdict(Verdict {
            segment_id: test_segment_id(3),
            winner: test_miner(5),
            loser: test_miner(6),
            slash_amount: 1_000_000,
        });

        assert_eq!(block.total_entries(), 5);
    }

    #[test]
    fn block_borsh_roundtrip() {
        let meta = EpochMeta {
            epoch: 10,
            prev_block_hash: test_hash(1),
            timestamp_ms: 1000,
        };

        let mut block = Block::new(meta);
        block.add_transaction(Transaction::Burn(BurnTx {
            sender: test_miner(1),
            amount: 1_000_000,
        }));

        let bytes = borsh::to_vec(&block).unwrap();
        let decoded: Block = borsh::from_slice(&bytes).unwrap();

        assert_eq!(block, decoded);
    }
}
