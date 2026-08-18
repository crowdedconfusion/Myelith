//! Merkle-Baum über SHA-256 mit Domain-Separation.
//!
//! Einsatz im Protokoll: θ_v-Wurzel (Whitepaper Kap. 6.1) und
//! Korpus-Provenienz (Kap. 7.3). Ein Aufbau für Berechnungsspuren folgt
//! später (erst Proof-Verifikation und kleine Bäume sind konsenskritisch;
//! die Speicher-Optimierung für Millionen Blätter ist ein späterer Punkt).
//!
//! Festlegungen (konsensrelevant, nur über Governance änderbar):
//! - **Domain-Separation:** Blätter werden als `SHA-256(0x00 || Daten)`
//!   gehasht, innere Knoten als `SHA-256(0x01 || links || rechts)`.
//!   Damit kann ein Blatt-Hash niemals als innerer Knoten interpretiert
//!   werden und umgekehrt (verhindert Second-Preimage-Angriffe).
//! - **Ungerade Ebenen:** der letzte Knoten einer Ebene mit ungerader
//!   Anzahl wird dupliziert und mit sich selbst gepaart (Bitcoin-Stil).
//!   Damit ist jeder innere Knoten einheitlich ein Hash aus zwei Kindern.
//! - **Ein-Blatt-Baum:** die Wurzel ist der Blatt-Hash selbst (keine
//!   zusätzliche Paarung). Ein leerer Baum ist ein Fehler.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::{Hash, HASH_LEN};

/// Domain-Separations-Präfix für Blätter.
pub const LEAF_PREFIX: u8 = 0x00;
/// Domain-Separations-Präfix für innere Knoten.
pub const NODE_PREFIX: u8 = 0x01;

/// Blatt-Hash: `SHA-256(0x00 || Daten)`.
pub fn leaf_hash(data: &[u8]) -> Hash {
    let mut buf = Vec::with_capacity(1 + data.len());
    buf.push(LEAF_PREFIX);
    buf.extend_from_slice(data);
    Hash::sha256(&buf)
}

/// Knoten-Hash: `SHA-256(0x01 || links || rechts)`.
pub fn node_hash(left: &Hash, right: &Hash) -> Hash {
    let mut buf = [0u8; 1 + 2 * HASH_LEN];
    buf[0] = NODE_PREFIX;
    buf[1..1 + HASH_LEN].copy_from_slice(left.as_bytes());
    buf[1 + HASH_LEN..].copy_from_slice(right.as_bytes());
    Hash::sha256(&buf)
}

/// Fehler beim Merkle-Aufbau bzw. bei der Beweis-Erzeugung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MerkleError {
    /// Ein Merkle-Baum ohne Blätter ist im Protokoll nicht zulässig.
    Empty,
    /// Blatt-Index liegt außerhalb des Baums.
    IndexOutOfRange { index: usize, leaves: usize },
}

impl std::fmt::Display for MerkleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Merkle-Baum ohne Blätter ist nicht zulässig"),
            Self::IndexOutOfRange { index, leaves } => {
                write!(f, "Blatt-Index {} außerhalb des Baums ({} Blätter)", index, leaves)
            }
        }
    }
}

impl std::error::Error for MerkleError {}

/// Vollständig aufgebauter Merkle-Baum (alle Ebenen materialisiert).
///
/// `levels[0]` sind die Blatt-Hashes, `levels[last]` ist die Wurzel.
/// Die Materialisierung aller Ebenen macht Beweis-Erzeugung trivial;
/// für sehr große Bäume (Berechnungsspuren) ist später eine
/// speicheroptimierte Variante vorgesehen.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    levels: Vec<Vec<Hash>>,
}

impl MerkleTree {
    /// Baut den Baum aus den rohen Blatt-Daten (nicht aus fertigen Hashes —
    /// die Domain-Separation macht das Hashen der Blätter Teil des
    /// Konsensvertrags).
    pub fn new(leaves_data: &[&[u8]]) -> Result<Self, MerkleError> {
        if leaves_data.is_empty() {
            return Err(MerkleError::Empty);
        }
        let mut current: Vec<Hash> = leaves_data.iter().map(|d| leaf_hash(d)).collect();
        let mut levels = vec![current.clone()];
        while current.len() > 1 {
            // Ungerade Anzahl: letzter Knoten wird mit sich selbst gepaart.
            if current.len() % 2 == 1 {
                let last = *current.last().expect("nicht leer");
                current.push(last);
            }
            let next: Vec<Hash> = current
                .chunks_exact(2)
                .map(|pair| node_hash(&pair[0], &pair[1]))
                .collect();
            levels.push(next.clone());
            current = next;
        }
        Ok(Self { levels })
    }

    /// Anzahl der Blätter.
    pub fn leaf_count(&self) -> usize {
        self.levels[0].len()
    }

    /// Tiefe des Baums: Anzahl der Ebenen über den Blättern
    /// (Ein-Blatt-Baum hat Tiefe 0).
    pub fn depth(&self) -> usize {
        self.levels.len() - 1
    }

    /// Die Merkle-Wurzel.
    pub fn root(&self) -> Hash {
        *self
            .levels
            .last()
            .expect("mindestens eine Ebene")
            .first()
            .expect("Wurzel-Ebene hat genau einen Knoten")
    }

    /// Erzeugt den Mitgliedschaftsbeweis für das Blatt `index`.
    pub fn proof(&self, index: usize) -> Result<MerkleProof, MerkleError> {
        if index >= self.leaf_count() {
            return Err(MerkleError::IndexOutOfRange {
                index,
                leaves: self.leaf_count(),
            });
        }
        let mut siblings = Vec::with_capacity(self.depth());
        let mut idx = index;
        for level in &self.levels[..self.levels.len() - 1] {
            // Gleiche Duplikations-Regel wie beim Aufbau: bei ungerader
            // Ebenenlänge ist der Geschwister des letzten Knotens er selbst.
            let sibling_idx = if idx % 2 == 0 {
                if idx + 1 < level.len() {
                    idx + 1
                } else {
                    idx
                }
            } else {
                idx - 1
            };
            siblings.push(level[sibling_idx]);
            idx /= 2;
        }
        Ok(MerkleProof {
            leaf_index: index as u64,
            siblings,
        })
    }
}

/// Mitgliedschaftsbeweis: Blatt-Index plus Geschwister-Hashes je Ebene.
///
/// Wird über Borsh serialisiert übertragen (Blöcke, Challenges) — die
/// Serialisierung ist Teil des Konsensvertrags (Design-Entscheidung Borsh).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct MerkleProof {
    /// Index des Blatts, dessen Mitgliedschaft bewiesen wird.
    pub leaf_index: u64,
    /// Geschwister-Hash je Ebene, beginnend bei der Blatt-Ebene.
    pub siblings: Vec<Hash>,
}

impl MerkleProof {
    /// Verifiziert, dass `leaf_data` an der Position `leaf_index` Teil des
    /// Baums mit Wurzel `root` ist.
    ///
    /// Der Index wird vom Aufrufer explizit übergeben und muss mit dem im
    /// Beweis gespeicherten übereinstimmen — damit ist der Beweis auch für
    /// Ein-Blatt-Bäume an eine konkrete Position gebunden.
    pub fn verify(&self, root: &Hash, leaf_data: &[u8], leaf_index: u64) -> bool {
        if self.leaf_index != leaf_index {
            return false;
        }
        self.verify_hashed(root, &leaf_hash(leaf_data))
    }

    /// Wie `verify`, aber mit bereits gehashtem Blatt (für Pfade, in denen
    /// der Blatt-Hash schon vorliegt, z. B. beim Verketten von Beweisen).
    pub fn verify_hashed(&self, root: &Hash, leaf: &Hash) -> bool {
        let mut current = *leaf;
        let mut idx = self.leaf_index;
        for sibling in &self.siblings {
            current = if idx % 2 == 0 {
                node_hash(&current, sibling)
            } else {
                node_hash(sibling, &current)
            };
            idx /= 2;
        }
        current == *root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    fn tree_of(n: usize) -> (MerkleTree, Vec<Vec<u8>>) {
        let data: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("myelith-blatt-{}", i).into_bytes())
            .collect();
        let refs: Vec<&[u8]> = data.iter().map(|d| d.as_slice()).collect();
        (MerkleTree::new(&refs).expect("Aufbau"), data)
    }

    #[test]
    fn leerer_baum_wird_abgelehnt() {
        assert!(matches!(MerkleTree::new(&[]), Err(MerkleError::Empty)));
    }

    #[test]
    fn ein_blatt_wurzel_ist_blatt_hash() {
        let (tree, data) = tree_of(1);
        assert_eq!(tree.depth(), 0);
        assert_eq!(tree.root(), leaf_hash(&data[0]));
    }

    #[test]
    fn zwei_blaetter_manuell() {
        let (tree, data) = tree_of(2);
        let expected = node_hash(&leaf_hash(&data[0]), &leaf_hash(&data[1]));
        assert_eq!(tree.root(), expected);
        assert_eq!(tree.depth(), 1);
    }

    #[test]
    fn drei_blaetter_duplikationsregel() {
        // Ungerade: das dritte Blatt wird mit sich selbst gepaart.
        let (tree, data) = tree_of(3);
        let l0 = leaf_hash(&data[0]);
        let l1 = leaf_hash(&data[1]);
        let l2 = leaf_hash(&data[2]);
        let expected = node_hash(&node_hash(&l0, &l1), &node_hash(&l2, &l2));
        assert_eq!(tree.root(), expected);
    }

    #[test]
    fn domain_separation_blatt_ist_nicht_knoten() {
        // Ein Blatt-Hash und ein Knoten-Hash derselben Rohdaten müssen
        // unterschiedlich sein (Second-Preimage-Schutz).
        let data = b"identische-rohdaten";
        let as_leaf = leaf_hash(data);
        let as_node = node_hash(&Hash::sha256(data), &Hash::sha256(data));
        assert_ne!(as_leaf, as_node);
        // Explizit: 0x00-Präfix-Hash ≠ 0x01-Präfix-Hash derselben Länge.
        let leaf_style = Hash::sha256(&[&[LEAF_PREFIX][..], data].concat());
        assert_eq!(as_leaf, leaf_style);
    }

    #[test]
    fn beweise_verifizieren_fuer_viele_groessen() {
        for n in [1usize, 2, 3, 4, 5, 7, 8, 16, 17, 31, 32, 33, 64] {
            let (tree, data) = tree_of(n);
            let root = tree.root();
            for (i, blatt) in data.iter().enumerate().take(n) {
                let proof = tree.proof(i).expect("Beweis-Erzeugung");
                assert!(
                    proof.verify(&root, blatt, i as u64),
                    "Beweis für n={}, i={} muss verifizieren",
                    n,
                    i
                );
                // Falsches Blatt muss scheitern.
                assert!(!proof.verify(&root, b"falsches-blatt", i as u64));
                // Falscher Index muss scheitern.
                assert!(!proof.verify(&root, &data[i], (i as u64) + 1));
                // Falsche Wurzel muss scheitern.
                let wrong_root = Hash::sha256(b"falsche-wurzel");
                assert!(!proof.verify(&wrong_root, &data[i], i as u64));
            }
        }
    }

    #[test]
    fn index_außerhalb_wird_abgelehnt() {
        let (tree, _) = tree_of(4);
        assert_eq!(
            tree.proof(4),
            Err(MerkleError::IndexOutOfRange { index: 4, leaves: 4 })
        );
    }

    #[test]
    fn einzeln_bitflip_im_blatt_wird_erkannt() {
        // Akzeptanzkriterium: JEDE Einzelbit-Verfälschung des Blatts
        // muss die Verifikation zum Scheitern bringen (exhaustiv über
        // alle Bits des echten Blatts 0).
        let (tree, data) = tree_of(5);
        let root = tree.root();
        let proof0 = tree.proof(0).expect("Beweis");
        let mut real = data[0].clone();
        assert!(proof0.verify(&root, &real, 0));
        for byte in 0..real.len() {
            for bit in 0..8 {
                real[byte] ^= 1 << bit;
                assert!(
                    !proof0.verify(&root, &real, 0),
                    "Bitflip byte={} bit={} muss scheitern",
                    byte,
                    bit
                );
                real[byte] ^= 1 << bit;
            }
        }
        // Unverändertes Blatt verifiziert weiterhin.
        assert!(proof0.verify(&root, &real, 0));
    }

    #[test]
    fn einzeln_bitflip_im_beweis_wird_erkannt() {
        // Akzeptanzkriterium: JEDE Einzelbit-Verfälschung des
        // serialisierten Beweises muss abgelehnt werden (exhaustiv über
        // die Borsh-Bytes).
        let (tree, data) = tree_of(9); // ungerade + mehrere Ebenen
        let root = tree.root();
        for i in [0usize, 4, 8] {
            let proof = tree.proof(i).expect("Beweis");
            assert!(proof.verify(&root, &data[i], i as u64));
            let mut bytes = to_vec(&proof).expect("Serialisierung");
            for byte in 0..bytes.len() {
                for bit in 0..8 {
                    bytes[byte] ^= 1 << bit;
                    let corrupted: Result<MerkleProof, _> = from_slice(&bytes);
                    let rejected = match corrupted {
                        Ok(p) => !p.verify(&root, &data[i], i as u64),
                        Err(_) => true, // unlesbar = ebenfalls abgelehnt
                    };
                    assert!(
                        rejected,
                        "Bitflip byte={} bit={} (Blatt {}) muss abgelehnt werden",
                        byte,
                        bit,
                        i
                    );
                    bytes[byte] ^= 1 << bit;
                }
            }
            // Unveränderter Beweis verifiziert weiterhin.
            assert!(proof.verify(&root, &data[i], i as u64));
        }
    }

    #[test]
    fn beweis_borsh_roundtrip() {
        let (tree, data) = tree_of(10);
        let root = tree.root();
        let proof = tree.proof(7).expect("Beweis");
        let bytes = to_vec(&proof).expect("Serialisierung");
        let back: MerkleProof = from_slice(&bytes).expect("Deserialisierung");
        assert_eq!(back, proof);
        assert!(back.verify(&root, &data[7], 7));
    }

    #[test]
    fn determinismus_wiederholter_aufbau() {
        let (tree_a, _) = tree_of(20);
        let (tree_b, _) = tree_of(20);
        assert_eq!(tree_a.root(), tree_b.root());
        assert_eq!(
            tree_a.proof(13).expect("Beweis"),
            tree_b.proof(13).expect("Beweis")
        );
    }

    #[test]
    fn verschiedene_reihenfolge_verschiedene_wurzel() {
        // Der Baum ist geordnet: dieselben Blätter in anderer Reihenfolge
        // ergeben eine andere Wurzel (Ordnung ist Teil des Vertrags).
        let a = [b"eins".as_slice(), b"zwei"];
        let b = [b"zwei".as_slice(), b"eins"];
        let tree_a = MerkleTree::new(&a).expect("Aufbau");
        let tree_b = MerkleTree::new(&b).expect("Aufbau");
        assert_ne!(tree_a.root(), tree_b.root());
    }
}
