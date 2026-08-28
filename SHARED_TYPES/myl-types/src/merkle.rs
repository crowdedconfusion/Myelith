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
//! - **Ein-Blatt-Baum:** die innere Wurzel ist der Blatt-Hash selbst
//!   (keine zusätzliche Paarung). Ein leerer Baum ist ein Fehler.
//! - **Blattzahl in der Wurzel (seit 2026-08-28, Fund 77):** die
//!   veröffentlichte Wurzel ist
//!   `SHA-256(0x02 || u64_le(Blattzahl) || innere Wurzel)`. Ohne diese
//!   Bindung ist der Baum nicht injektiv, siehe unten.
//!
//! ## ⚑ Fund 77 (2026-08-28): Die Wurzel bestimmt die Blattfolge, seit heute
//!
//! **Die Duplikationsregel allein ist nicht injektiv.** Sie stammt aus
//! dem Bitcoin-Stil und erbte dessen Fehler (CVE-2012-2459): Bei
//! ungerader Blattzahl `n ≥ 3` erzeugen `[l₁ … lₙ]` und
//! `[l₁ … lₙ, lₙ]` dieselbe **innere** Wurzel, weil die zweite Folge
//! genau das Blatt enthält, das die erste sich beim Auffüllen selbst
//! hinzugefügt hätte. Auf jeder Ebene, deren Knotenzahl ungerade wird,
//! wiederholt sich das; bei `n ≡ 2 (mod 4)` ab 6 also mit den letzten
//! **zwei** Blättern.
//!
//! **Behoben durch Bindung der Blattzahl** ([`root_hash`]): Die
//! veröffentlichte Wurzel ist
//! `SHA-256(0x02 ‖ u64_le(n) ‖ innere Wurzel)`. Zwei Folgen
//! verschiedener Länge tragen damit verschiedene Wurzeln, und für feste
//! Länge liegt die Baumform fest.
//!
//! **Warum diese Behebung und nicht eine andere.** Drei Wege standen
//! offen: die Blattzahl binden, beim Auffüllen einen Fremdwert paaren,
//! oder die ungerade Ebene unverändert hochziehen. Der dritte Weg wirkt
//! am sparsamsten, weil er kein Feld braucht, **hat aber denselben
//! Bedarf verdeckt**: Beweise bekommen dort je nach Index verschiedene
//! Längen, und ein Prüfer kann die Ebenen nicht ablaufen, ohne die
//! Blattzahl zu kennen. Der erste Weg ist der einzige, dessen
//! Injektivitätsargument in einen Satz passt, und er lässt den inneren
//! Aufbau unberührt.
//!
//! **Dieselbe Überlegung steht schon an einer anderen Stelle im
//! Projekt:** `myl-governance::modell::Modellmanifest::wurzel` setzt vor
//! jedes Feld ein Längenpräfix, damit `("ab", "c")` und `("a", "bc")`
//! nicht dieselben Bytes ergeben. Der Merkle-Baum ist älter als diese
//! Einsicht und hat sie jetzt nachgeholt.
//!
//! **Was die Bindung für die Verwender ändert: nichts an ihrem Code.**
//! Die Blattzahl steckt in der Wurzel, nicht in einer zusätzlichen
//! Angabe der Aufrufer. `PoIBundle` braucht deshalb kein Feld für die
//! Segmentzahl, obwohl genau dessen Fehlen den Fund erst scharf gemacht
//! hätte: Eine Aggregatsignatur über `segments_root` bindet die Zahl
//! seither mit.
//!
//! ⚑ **Der Beweis trägt auch gegen eine gelogene Blattzahl.**
//! [`MerkleProof`] führt `leaf_count` mit, und der Wert muss aus keiner
//! vertrauenswürdigen Quelle stammen: Er geht in die Wurzelberechnung
//! ein, eine falsche Zahl ergibt eine andere Wurzel, und der Vergleich
//! scheitert.
//!
//! **Was der Fund gekostet hätte, wäre er später gefunden worden.** Die
//! Behebung verschiebt jede bestehende Wurzel. Heute entsteht jede
//! Wurzel im System zur Laufzeit neu, es gibt keinen Genesis-Block und
//! keine gespeicherte Kette; betroffen waren vier eingefrorene
//! Prüfvektoren und ein Fingerabdruck des Testclients. Nach dem
//! Genesis-Block wäre es eine Migration gewesen.
//!
//! ⚑ **Was daran über den Fund hinaus lehrreich ist.** Die
//! Duplikationsregel **war getestet**, und die Domain-Separation ist
//! sauber und mit dem richtigen Argument begründet. Geprüft wurde, dass
//! die Regel tut, was sie soll. Nicht geprüft wurde, was daraus
//! **folgt**. Der Test, der jetzt danebensteht
//! (`die_wurzel_bestimmt_die_blattfolge`), fährt deshalb die
//! Nachbarschaft ab, statt einen Einzelfall zu behaupten.
//!
use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::{Hash, HASH_LEN};

/// Domain-Separations-Präfix für Blätter.
pub const LEAF_PREFIX: u8 = 0x00;
/// Domain-Separations-Präfix für innere Knoten.
pub const NODE_PREFIX: u8 = 0x01;
/// Domain-Separations-Präfix für die Wurzel, die zusätzlich die
/// Blattzahl bindet (Fund 77).
pub const ROOT_PREFIX: u8 = 0x02;

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

/// Wurzel-Hash: `SHA-256(0x02 || u64_le(Blattzahl) || innere Wurzel)`.
///
/// **Die Bindung der Blattzahl ist die Behebung von Fund 77.** Ohne sie
/// ist die Abbildung von Blattfolgen auf Wurzeln nicht injektiv, weil
/// die Auffüllregel bei ungerader Knotenzahl den letzten Knoten
/// wiederholt: `[l₁ … lₙ]` und `[l₁ … lₙ, lₙ]` erzeugen dieselbe
/// **innere** Wurzel. Sie erzeugen nicht dieselbe Blattzahl.
///
/// **Der Beweis in einem Satz:** Aus gleicher Wurzel folgt gleiches
/// Urbild, also gleiche Blattzahl und gleiche innere Wurzel; bei fester
/// Blattzahl liegt die Baumform fest, und über die Domain-Separation
/// bestimmt jeder Knoten seine beiden Kinder eindeutig.
pub fn root_hash(leaf_count: u64, inner: &Hash) -> Hash {
    let mut buf = [0u8; 1 + 8 + HASH_LEN];
    buf[0] = ROOT_PREFIX;
    buf[1..9].copy_from_slice(&leaf_count.to_le_bytes());
    buf[9..].copy_from_slice(inner.as_bytes());
    Hash::sha256(&buf)
}

/// Anzahl der Ebenen über den Blättern für `n` Blätter, also
/// `ceil(log2(n))`; ein Ein-Blatt-Baum hat Tiefe 0.
///
/// Ganzzahlig gerechnet, ohne Logarithmus: Der Wert wird über die
/// Auffüllregel des Aufbaus bestimmt, damit er nicht auseinanderlaufen
/// kann.
pub fn tiefe_fuer(leaf_count: u64) -> u32 {
    let mut breite = leaf_count;
    let mut tiefe = 0u32;
    while breite > 1 {
        breite = breite.div_ceil(2);
        tiefe += 1;
    }
    tiefe
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
            // clippy schlaegt seit 1.98 `as_chunks::<2>()` vor. Das ist erst
            // seit Rust 1.88 stabil, dieses Crate erklaert aber MSRV 1.82
            // (Cargo.toml). Die Zusage wiegt schwerer als der Stilhinweis:
            // Wer mit 1.82 baut, soll bauen koennen.
            // `unknown_lints` muss mit erlaubt sein: Den Lint-Namen gibt es erst
            // ab clippy 1.98, ein `allow` darauf ist auf aelteren Werkzeugketten
            // selbst eine Warnung. So baut es mit beiden.
            #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
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

    /// Die **innere** Wurzel, also der oberste Knoten der Ebenen.
    ///
    /// ⚑ **Das ist nicht das Commitment.** Sie bestimmt die Blattfolge
    /// nicht eindeutig (Fund 77) und darf nirgends veröffentlicht,
    /// signiert oder verglichen werden. Öffentlich ist [`Self::root`].
    /// Sichtbar bleibt sie nur, weil die Beweisprüfung sie berechnet.
    fn innere_wurzel(&self) -> Hash {
        *self
            .levels
            .last()
            .expect("mindestens eine Ebene")
            .first()
            .expect("Wurzel-Ebene hat genau einen Knoten")
    }

    /// Die Merkle-Wurzel: `SHA-256(0x02 || u64_le(Blattzahl) || innere Wurzel)`.
    ///
    /// Die Blattzahl ist seit der Behebung von Fund 77 mitgebunden, siehe
    /// [`root_hash`].
    pub fn root(&self) -> Hash {
        root_hash(self.leaf_count() as u64, &self.innere_wurzel())
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
            leaf_count: self.leaf_count() as u64,
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
    /// Blattzahl des Baums, gegen den bewiesen wird (Fund 77).
    ///
    /// ⚑ **Der Wert muss aus keiner vertrauenswürdigen Quelle kommen.**
    /// Er geht in die Wurzelberechnung ein; ein Beweis mit falscher
    /// Blattzahl ergibt eine andere Wurzel und scheitert am Vergleich.
    /// Ein Angreifer kann darüber also nicht lügen, sondern nur
    /// scheitern.
    pub leaf_count: u64,
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
        // Formprüfungen vor der Rechnung. Sie sind für die Sicherheit
        // nicht nötig — eine falsche Blattzahl ergibt ohnehin eine
        // andere Wurzel —, aber sie weisen einen unsinnigen Beweis ab,
        // ohne ihn erst durchzurechnen, und halten die Form des
        // Beweises an die Blattzahl gebunden.
        if self.leaf_count == 0 || self.leaf_index >= self.leaf_count {
            return false;
        }
        if self.siblings.len() as u64 != tiefe_fuer(self.leaf_count) as u64 {
            return false;
        }
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
        root_hash(self.leaf_count, &current) == *root
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
    fn ein_blatt_wurzel_ist_der_gebundene_blatt_hash() {
        let (tree, data) = tree_of(1);
        assert_eq!(tree.depth(), 0);
        // Seit Fund 77 bindet die Wurzel die Blattzahl. Die **innere**
        // Wurzel ist weiterhin der Blatt-Hash selbst.
        assert_eq!(tree.root(), root_hash(1, &leaf_hash(&data[0])));
        assert_ne!(
            tree.root(),
            leaf_hash(&data[0]),
            "die veroeffentlichte Wurzel ist nicht mehr der nackte Blatt-Hash"
        );
    }

    #[test]
    fn zwei_blaetter_manuell() {
        let (tree, data) = tree_of(2);
        let innen = node_hash(&leaf_hash(&data[0]), &leaf_hash(&data[1]));
        assert_eq!(tree.root(), root_hash(2, &innen));
        assert_eq!(tree.depth(), 1);
    }

    #[test]
    fn drei_blaetter_duplikationsregel() {
        // Ungerade: das dritte Blatt wird mit sich selbst gepaart.
        // Die Regel ist geblieben; hinzugekommen ist die Bindung der
        // Blattzahl darueber (Fund 77).
        let (tree, data) = tree_of(3);
        let l0 = leaf_hash(&data[0]);
        let l1 = leaf_hash(&data[1]);
        let l2 = leaf_hash(&data[2]);
        let innen = node_hash(&node_hash(&l0, &l1), &node_hash(&l2, &l2));
        assert_eq!(tree.root(), root_hash(3, &innen));
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

    /// ⚑ **Fund 77: Die Wurzel bestimmt die Blattfolge.**
    ///
    /// Der Test fährt die **Nachbarschaft** ab statt einen Einzelfall
    /// zu behaupten: Zu jeder Blattzahl von 1 bis 12 wird die Folge um
    /// das letzte Blatt und um die letzten zwei verlängert, und keine
    /// dieser Verlängerungen darf dieselbe Wurzel tragen. Vor der
    /// Behebung fielen 3, 5, 7, 9 (letztes Blatt) und 6 (letzte zwei)
    /// durch.
    #[test]
    fn die_wurzel_bestimmt_die_blattfolge() {
        let daten: Vec<Vec<u8>> = (0..14u8).map(|i| vec![i; 4]).collect();
        let wurzel = |bl: &[&[u8]]| MerkleTree::new(bl).expect("Aufbau").root();

        for n in 1usize..=12 {
            let kurz: Vec<&[u8]> = daten[..n].iter().map(|v| v.as_slice()).collect();
            let w_kurz = wurzel(&kurz);

            let mut plus_eins = kurz.clone();
            plus_eins.push(daten[n - 1].as_slice());
            assert_ne!(w_kurz, wurzel(&plus_eins), "n={} gegen n+1", n);

            if n >= 2 {
                let mut plus_zwei = kurz.clone();
                plus_zwei.push(daten[n - 2].as_slice());
                plus_zwei.push(daten[n - 1].as_slice());
                assert_ne!(w_kurz, wurzel(&plus_zwei), "n={} gegen n+2", n);
            }
        }
    }

    /// ⚑ Gegenprobe zu Fund 77: Ohne die Bindung der Blattzahl wäre der
    /// Test darüber rot. Hier steht die Kollision, die es bis zum
    /// 2026-08-28 gab, ausdrücklich hin: Die **inneren** Wurzeln sind
    /// weiterhin gleich, und nur das Präfix mit der Blattzahl trennt
    /// sie.
    ///
    /// Ohne diesen Test hinge die Behebung an einer Behauptung: Ein
    /// grüner Injektivitätstest allein bewiese nicht, dass die Bindung
    /// die Ursache ist. Er wäre auch grün, wenn die Auffüllregel sich
    /// nebenbei geändert hätte.
    #[test]
    fn ohne_die_blattzahl_waeren_die_wurzeln_gleich() {
        let a: &[u8] = b"A";
        let b: &[u8] = b"B";
        let c: &[u8] = b"C";
        let drei = MerkleTree::new(&[a, b, c]).expect("Aufbau");
        let vier = MerkleTree::new(&[a, b, c, c]).expect("Aufbau");

        // Die veröffentlichten Wurzeln sind verschieden.
        assert_ne!(drei.root(), vier.root());

        // Und der Grund ist allein die gebundene Blattzahl: Rechnet man
        // beide mit derselben Zahl, fallen sie zusammen. Das ist die
        // Kollision von Fund 77, hier festgehalten statt beseitigt.
        assert_eq!(
            root_hash(3, &drei.innere_wurzel()),
            root_hash(3, &vier.innere_wurzel()),
            "die inneren Wurzeln waren und sind gleich"
        );
        assert_eq!(drei.leaf_count(), 3);
        assert_eq!(vier.leaf_count(), 4);
    }

    /// ⚑ Eine gelogene Blattzahl im Beweis nützt nichts.
    ///
    /// Der Wert kommt aus dem Beweis und damit aus der Hand dessen, der
    /// ihn vorlegt. Er muss trotzdem aus keiner vertrauenswürdigen
    /// Quelle stammen, denn er geht in die Wurzelberechnung ein.
    #[test]
    fn eine_gefaelschte_blattzahl_im_beweis_scheitert() {
        let (baum, daten) = tree_of(5);
        let wurzel = baum.root();
        let echt = baum.proof(2).expect("Beweis");
        assert!(echt.verify(&wurzel, &daten[2], 2));

        for gelogen in [1u64, 3, 4, 6, 8, u64::MAX] {
            let mut gefaelscht = echt.clone();
            gefaelscht.leaf_count = gelogen;
            assert!(
                !gefaelscht.verify(&wurzel, &daten[2], 2),
                "Blattzahl {} wurde angenommen",
                gelogen
            );
        }
    }

    /// Die Formprüfungen in `verify_hashed` weisen unsinnige Beweise ab,
    /// bevor gerechnet wird.
    #[test]
    fn beweise_mit_falscher_form_werden_abgewiesen() {
        let (baum, daten) = tree_of(5);
        let wurzel = baum.root();
        let echt = baum.proof(2).expect("Beweis");

        // Index außerhalb der Blattzahl.
        let mut zu_gross = echt.clone();
        zu_gross.leaf_index = 5;
        assert!(!zu_gross.verify(&wurzel, &daten[2], 5));

        // Blattzahl null.
        let mut leer = echt.clone();
        leer.leaf_count = 0;
        assert!(!leer.verify(&wurzel, &daten[2], 2));

        // Zu wenige und zu viele Geschwister.
        let mut kurz = echt.clone();
        kurz.siblings.pop();
        assert!(!kurz.verify(&wurzel, &daten[2], 2));
        let mut lang = echt.clone();
        lang.siblings.push(Hash::sha256(b"x"));
        assert!(!lang.verify(&wurzel, &daten[2], 2));
    }

    /// `tiefe_fuer` folgt der Auffüllregel des Aufbaus, nicht einer
    /// zweiten Rechnung daneben. Geprüft wird deshalb gegen den echten
    /// Baum, für jede Blattzahl bis 33.
    #[test]
    fn tiefe_fuer_stimmt_mit_dem_gebauten_baum_ueberein() {
        for n in 1usize..=33 {
            let (baum, _) = tree_of(n);
            assert_eq!(
                tiefe_fuer(n as u64) as usize,
                baum.depth(),
                "n={}",
                n
            );
        }
    }
}
