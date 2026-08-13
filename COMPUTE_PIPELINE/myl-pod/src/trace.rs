//! Spur-Hashes und Übergangs-Signaturen (Anhang A.3, Schritte 2+4).
//!
//! Jeder Shard hasht seine Ausgabe-Aktivierungen (`activation_hash`) und
//! signiert den Übergang `(segment_id, vorheriger Spur-Hash, neuer
//! Spur-Hash)` mit seinem BLS-Schlüssel (`TransitionSig`). Der nächste
//! Shard prüft den Hash der empfangenen Aktivierungen gegen den letzten
//! Spur-Eintrag — das ist die Manipulationserkennung.
//!
//! Alle Operationen sind deterministisch: derselbe Input ergibt denselben
//! Hash und dieselbe Signatur (BLS ohne Zufallsdaten).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use myl_types::ids::SegmentId;
use sha2::{Digest, Sha256};

/// Hash über die Aktivierungen eines Shard-Ausgangs.
///
/// Die Aktivierungen werden als little-endian i16-Folge gehasht. Das ist
/// der Spur-Eintrag `h(a_i)` (Anhang A.3). Deterministisch und auf jedem
/// Node identisch.
pub fn activation_hash(activations: &[i16]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for v in activations {
        hasher.update(v.to_le_bytes());
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Die zu signierende Übergangs-Nachricht (Anhang A.3
/// `sign_transition(seg.id, seg.trace[i-1], h_next)`).
///
/// Borsh-serialisiert über `(segment_id, shard_index, position,
/// prev_hash, next_hash)`. Für Shard 0 ist `prev_hash` der Null-Hash
/// (es gibt keinen vorherigen Spur-Eintrag).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct TransitionSig {
    pub segment_id: SegmentId,
    pub shard_index: u64,
    pub position: u64,
    pub prev_hash: [u8; 32],
    pub next_hash: [u8; 32],
}

impl TransitionSig {
    /// Serialisiert die Übergangs-Nachricht zu den zu signierenden Bytes.
    pub fn to_sign_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("TransitionSig ist stets serialisierbar")
    }

    /// Signiert den Übergang mit dem Shard-BLS-Schlüssel.
    pub fn sign(&self, sk: &BlsSecretKey) -> Result<BlsSignature, String> {
        sk.sign(&self.to_sign_bytes()).map_err(|e| e.to_string())
    }

    /// Verifiziert die Übergangs-Signatur gegen den öffentlichen
    /// Schlüssel des Shards.
    pub fn verify(&self, pk: &BlsPublicKey, sig: &BlsSignature) -> bool {
        pk.verify(&self.to_sign_bytes(), sig)
    }
}

/// Null-Hash (vorheriger Spur-Eintrag für Shard 0).
pub const ZERO_HASH: [u8; 32] = [0u8; 32];

/// Prüft, dass der Hash der empfangenen Aktivierungen zum letzten
/// Spur-Eintrag passt (Manipulationserkennung). Liefert `true`, wenn die
/// Spur leer ist (Shard 0 empfängt Token, keine Aktivierungen) oder der
/// Hash übereinstimmt.
pub fn verify_input_hash(activations: &[i16], trace: &[[u8; 32]]) -> bool {
    match trace.last() {
        None => true, // Shard 0: noch kein Spur-Eintrag, Token-Eingang.
        Some(expected) => activation_hash(activations) == *expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_hash_deterministisch_und_empfindlich() {
        let a = [1i16, 2, 3, -4, 32767, -32768];
        let h1 = activation_hash(&a);
        let h2 = activation_hash(&a);
        assert_eq!(h1, h2);
        // Ein verändertes Byte ⇒ anderer Hash.
        let mut b = a.clone();
        b[3] ^= 1;
        assert_ne!(activation_hash(&b), h1);
        // Leer ⇒ definierter Hash.
        let leer = activation_hash(&[]);
        assert_eq!(leer, activation_hash(&[]));
    }

    #[test]
    fn uebergang_signieren_und_verifizieren() {
        let sk = BlsSecretKey::key_gen(&[0x42u8; 32]).expect("KeyGen");
        let pk = sk.public_key().expect("pk");
        let t = TransitionSig {
            segment_id: SegmentId::new([1u8; 32]),
            shard_index: 1,
            position: 3,
            prev_hash: [2u8; 32],
            next_hash: [3u8; 32],
        };
        let sig = t.sign(&sk).expect("Signieren");
        assert!(t.verify(&pk, &sig));
        // Manipulierter Übergang ⇒ Verifikation scheitert.
        let mut t2 = t.clone();
        t2.next_hash = [4u8; 32];
        assert!(!t2.verify(&pk, &sig));
        // Falscher Schlüssel ⇒ scheitert.
        let fremd = BlsSecretKey::key_gen(&[0x99u8; 32]).expect("KeyGen");
        let fremd_pk = fremd.public_key().expect("pk");
        assert!(!t.verify(&fremd_pk, &sig));
    }

    #[test]
    fn eingangs_hash_pruefung() {
        let akt = [10i16, 20, 30];
        let h = activation_hash(&akt);
        // Passender Spur-Eintrag ⇒ ok.
        assert!(verify_input_hash(&akt, &[h]));
        // Leere Spur (Shard 0) ⇒ ok.
        assert!(verify_input_hash(&akt, &[]));
        // Verfälschte Aktivierungen ⇒ abgelehnt.
        let mut manipuliert = akt.clone();
        manipuliert[0] = 99;
        assert!(!verify_input_hash(&manipuliert, &[h]));
        // Falscher Spur-Eintrag ⇒ abgelehnt.
        assert!(!verify_input_hash(&akt, &[[0xAAu8; 32]]));
    }
}
