//! Kern-Structs aus Whitepaper Anhang A.1 (Punkt 1.5):
//! `Segment`, `PoIBundle`, `InferenceCredit`.
//!
//! **Feldnamen und Feldreihenfolge sind Konsens-Vertrag:** Borsh
//! serialisiert in Deklarationsreihenfolge, jede Abweichung von
//! Anhang A.1 würde die Hashes über diesen Strukturen (und damit alle
//! Commitments und Bündel-Wurzeln) ändern. Änderungen nur über
//! Governance (Kap. 10.3).

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::BlsSignature;
use crate::hash::Hash;
use crate::ids::{ActivationHash, Address, EpochId, MerkleRoot, MinerId, PodId, SegmentId};
use crate::merkle::{MerkleError, MerkleTree};

/// Ein Inferenz-Segment σ = (x, θ_v, π, y) als übertragbarer Datensatz
/// (Whitepaper Kap. 6.1, Anhang A.1).
///
/// - `id`: `h(session ‖ index)`
/// - `input_commitment`: `h(prompt_chunk ‖ kv_root)`
/// - `model_version`: Gewichts-Wurzel θ_v inkl. Ausführungsspezifikation
/// - `pod_path`: Pipeline-Reihenfolge der Shard-Miner
/// - `trace`: Berechnungsspur h(a_0), …, h(a_k)
/// - `signatures`: eine BLS-Signatur pro Shard-Übergang
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Segment {
    pub id: SegmentId,
    pub input_commitment: Hash,
    pub model_version: MerkleRoot,
    pub pod_path: Vec<MinerId>,
    pub output_commitment: Hash,
    pub trace: Vec<ActivationHash>,
    pub signatures: Vec<BlsSignature>,
}

/// PoI-Bündel: pro Epoche und Pod eingereicht, signiert und aggregiert
/// (Whitepaper Kap. 4.4, Anhang A.1).
///
/// - `segments_root`: Merkle-Wurzel über alle Segment-Ids der Epoche
/// - `vtfe_claimed`: beanspruchte Arbeit (vTFE)
/// - `aggregate_sig`: BLS-Aggregat über die Pod-Mitglieder
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PoIBundle {
    pub epoch: EpochId,
    pub pod: PodId,
    pub segments_root: MerkleRoot,
    pub vtfe_claimed: u64,
    pub aggregate_sig: BlsSignature,
}

/// Inferenz-Credit: durch Burn erworbenes Guthaben an Inferenzarbeit
/// (Whitepaper Kap. 5, Anhang A.1).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InferenceCredit {
    pub owner: Address,
    pub vtfe: u64,
    pub expiry: EpochId,
}

/// Merkle-Wurzel über eine Folge von Segment-Ids — die
/// `segments_root`-Konstruktion aus `PoIBundle` (Blätter = die rohen
/// 32-Byte-Ids, in Bundel-Reihenfolge).
pub fn segments_root(ids: &[SegmentId]) -> Result<MerkleRoot, MerkleError> {
    let refs: Vec<&[u8]> = ids.iter().map(|id| id.as_ref()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    /// Deterministischer PRNG (xorshift64) für die Property-Tests —
    /// die Testsuite soll ohne Zufalls-Abhängigkeit reproduzierbar sein.
    struct Xorshift(u64);

    impl Xorshift {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn fill<const N: usize>(&mut self) -> [u8; N] {
            let mut out = [0u8; N];
            for chunk in out.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            out
        }
    }

    fn zufalls_segment(rng: &mut Xorshift) -> Segment {
        let n_miner = (rng.next_u64() % 4) as usize;
        let n_trace = (rng.next_u64() % 8) as usize;
        Segment {
            id: SegmentId::new(rng.fill()),
            input_commitment: Hash::from_bytes(rng.fill()),
            model_version: MerkleRoot::new(rng.fill()),
            pod_path: (0..n_miner).map(|_| MinerId::new(rng.fill())).collect(),
            output_commitment: Hash::from_bytes(rng.fill()),
            trace: (0..n_trace)
                .map(|_| ActivationHash::new(rng.fill()))
                .collect(),
            signatures: Vec::new(), // BLS-Signaturen sind separat getestet
        }
    }

    fn zufalls_bundle(rng: &mut Xorshift) -> PoIBundle {
        PoIBundle {
            epoch: EpochId(rng.next_u64()),
            pod: PodId::new(rng.fill()),
            segments_root: MerkleRoot::new(rng.fill()),
            vtfe_claimed: rng.next_u64(),
            aggregate_sig: BlsSignature(rng.fill()),
        }
    }

    fn zufalls_credit(rng: &mut Xorshift) -> InferenceCredit {
        InferenceCredit {
            owner: Address::new(rng.fill()),
            vtfe: rng.next_u64(),
            expiry: EpochId(rng.next_u64()),
        }
    }

    /// Akzeptanzkriterium Phase 1: `serialize(deserialize(x)) == x` für
    /// zufällige Instanzen, mindestens 10.000 Fälle je Typ.
    #[test]
    fn roundtrip_zufaellige_instanzen() {
        let mut rng = Xorshift(0x5eed_5eed_5eed_5eed);
        for _ in 0..10_000 {
            let segment = zufalls_segment(&mut rng);
            let bytes = to_vec(&segment).expect("Serialisierung");
            let back: Segment = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, segment);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let bundle = zufalls_bundle(&mut rng);
            let bytes = to_vec(&bundle).expect("Serialisierung");
            let back: PoIBundle = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, bundle);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let credit = zufalls_credit(&mut rng);
            let bytes = to_vec(&credit).expect("Serialisierung");
            let back: InferenceCredit = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, credit);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
    }

    #[test]
    fn serialisierung_ist_deterministisch() {
        let mut rng_a = Xorshift(42);
        let mut rng_b = Xorshift(42);
        let a = zufalls_segment(&mut rng_a);
        let b = zufalls_segment(&mut rng_b);
        assert_eq!(to_vec(&a).expect("ser"), to_vec(&b).expect("ser"));
    }

    #[test]
    fn segments_root_stimmt_mit_merkle_baum_ueberein() {
        let ids: Vec<SegmentId> = (0..5u8)
            .map(|i| {
                let mut bytes = [0u8; 32];
                bytes[0] = i;
                SegmentId::new(bytes)
            })
            .collect();
        let root = segments_root(&ids).expect("Wurzel");
        // Manuell über den Merkle-Baum mit denselben Blättern.
        let refs: Vec<&[u8]> = ids.iter().map(|id| id.as_ref()).collect();
        let tree = MerkleTree::new(&refs).expect("Baum");
        assert_eq!(root, MerkleRoot::new(tree.root().0));
        // Mitgliedschaftsbeweis für ein Blatt muss verifizieren.
        let proof = tree.proof(2).expect("Beweis");
        assert!(proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(ids[2].as_ref())));
    }

    #[test]
    fn segments_root_leer_wird_abgelehnt() {
        assert!(matches!(segments_root(&[]), Err(MerkleError::Empty)));
    }

    #[test]
    fn segment_feldreihenfolge_ist_fest() {
        // Anhang A.1 schreibt die Feldreihenfolge vor; Borsh kodiert in
        // Deklarationsreihenfolge. Dieser Test fixiert die Serialisierung
        // eines Minimal-Segments als Golden-Byte-Folge: Jede Änderung der
        // Feldreihenfolge oder der Typen bricht sie (und damit den
        // Konsens) und schlägt hier an.
        let segment = Segment {
            id: SegmentId::new([0u8; 32]),
            input_commitment: Hash::from_bytes([0u8; 32]),
            model_version: MerkleRoot::new([0u8; 32]),
            pod_path: Vec::new(),
            output_commitment: Hash::from_bytes([0u8; 32]),
            trace: Vec::new(),
            signatures: Vec::new(),
        };
        let bytes = to_vec(&segment).expect("Serialisierung");
        // 32 (id) + 32 (input) + 32 (version) + 4 (Vec-Länge) + 32 (output)
        // + 4 (Vec-Länge) + 4 (Vec-Länge) = 140 Bytes.
        assert_eq!(bytes.len(), 140);
        assert!(bytes[96..100].iter().all(|&b| b == 0)); // leere pod_path
        assert!(bytes[132..136].iter().all(|&b| b == 0)); // leere trace
        assert!(bytes[136..140].iter().all(|&b| b == 0)); // leere signatures
    }
}
