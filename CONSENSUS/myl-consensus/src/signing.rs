//! Kanonische Signierbotschaften für das BFT-Protokoll.
//!
//! Jede signierte Konsens-Nachricht wird über eine **kanonische
//! Bytefolge** signiert, die genau einmal an dieser Stelle definiert
//! ist. Zwei Eigenschaften sind dabei nicht verhandelbar:
//!
//! 1. **Domain-Separation.** Propose, Vote und Commit tragen jeweils
//!    ein eigenes Präfix. Ohne diese Trennung wäre eine gültige Vote
//!    für Block B in Runde r zugleich ein gültiger Commit für denselben
//!    Block — ein Angreifer könnte fremde Votes zu Commits umdeuten und
//!    den Commit-Threshold ohne eigene Stimmen erreichen.
//! 2. **Eindeutige Kodierung.** Feste Feldbreiten in fester Reihenfolge
//!    (Little-Endian, wie im Rest des Protokolls), damit zu einer
//!    Nachricht genau eine Bytefolge gehört und umgekehrt.
//!
//! Die Botschaft bindet immer `(Runde, Block-Hash)`. Der Absender ist
//! **nicht** Teil der Botschaft — er ergibt sich aus dem öffentlichen
//! Schlüssel, gegen den verifiziert wird. Genau das macht den
//! Double-Signing-Beweis möglich: zwei gültige Signaturen desselben
//! Schlüssels über dieselbe Runde, aber verschiedene Block-Hashes.
//!
//! **Konsens-Feld:** Die Kodierung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3) — eine Änderung
//! invalidiert alle zuvor erzeugten Signaturen.

use myl_types::hash::Hash;

/// Domain-Separation-Präfix für Propose-Nachrichten.
pub const DST_PROPOSE: &[u8] = b"MYELITH_BFT_PROPOSE_v1";

/// Domain-Separation-Präfix für Vote-Nachrichten.
pub const DST_VOTE: &[u8] = b"MYELITH_BFT_VOTE_v1";

/// Domain-Separation-Präfix für Commit-Nachrichten.
pub const DST_COMMIT: &[u8] = b"MYELITH_BFT_COMMIT_v1";

/// Baut die kanonische Signierbotschaft für eine Konsens-Nachricht.
///
/// **Aufbau:** `dst ‖ u64_le(round) ‖ block_hash` — feste Feldbreiten,
/// daher präfixfrei und eindeutig dekodierbar.
///
/// **Parameter:**
/// - `dst`: Domain-Separation-Präfix ([`DST_PROPOSE`], [`DST_VOTE`], [`DST_COMMIT`])
/// - `round`: Rundennummer
/// - `block_hash`: Hash des betroffenen Blocks
///
/// **Returns:** Die zu signierende bzw. zu verifizierende Bytefolge.
pub fn signable_bytes(dst: &[u8], round: u64, block_hash: &Hash) -> Vec<u8> {
    let mut msg = Vec::with_capacity(dst.len() + 8 + 32);
    msg.extend_from_slice(dst);
    msg.extend_from_slice(&round.to_le_bytes());
    msg.extend_from_slice(block_hash.as_bytes());
    msg
}

/// Signierbotschaft einer Propose-Nachricht.
pub fn propose_message(round: u64, block_hash: &Hash) -> Vec<u8> {
    signable_bytes(DST_PROPOSE, round, block_hash)
}

/// Signierbotschaft einer Vote-Nachricht.
pub fn vote_message(round: u64, block_hash: &Hash) -> Vec<u8> {
    signable_bytes(DST_VOTE, round, block_hash)
}

/// Signierbotschaft einer Commit-Nachricht.
pub fn commit_message(round: u64, block_hash: &Hash) -> Vec<u8> {
    signable_bytes(DST_COMMIT, round, block_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    #[test]
    fn signierbotschaft_ist_deterministisch() {
        let h = test_hash(1);
        assert_eq!(vote_message(7, &h), vote_message(7, &h));
    }

    #[test]
    fn signierbotschaft_hat_erwartete_laenge() {
        let msg = vote_message(7, &test_hash(1));
        assert_eq!(msg.len(), DST_VOTE.len() + 8 + 32);
    }

    #[test]
    fn nachrichtentypen_sind_domain_getrennt() {
        let h = test_hash(1);
        // Ohne Domain-Separation waere eine Vote zugleich ein gueltiger
        // Commit — der Commit-Threshold liesse sich mit fremden Votes
        // erreichen.
        assert_ne!(vote_message(7, &h), commit_message(7, &h));
        assert_ne!(vote_message(7, &h), propose_message(7, &h));
        assert_ne!(commit_message(7, &h), propose_message(7, &h));
    }

    #[test]
    fn runde_und_blockhash_binden_die_botschaft() {
        let h1 = test_hash(1);
        let h2 = test_hash(2);
        assert_ne!(vote_message(7, &h1), vote_message(8, &h1));
        assert_ne!(vote_message(7, &h1), vote_message(7, &h2));
    }

    #[test]
    fn kodierung_ist_praefixfrei() {
        // Feste Feldbreiten: (round=1, hash) darf nicht das Praefix von
        // (round=1, anderer hash) sein und keine Verschiebung erlauben.
        let a = vote_message(1, &test_hash(1));
        let b = vote_message(1, &test_hash(2));
        assert_eq!(a.len(), b.len());
        assert!(!a.starts_with(&b[..]) || a == b);
    }
}
