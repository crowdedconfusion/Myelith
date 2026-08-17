//! Challenge-Erzeugung — Whitepaper Kap. 6.6, Anhang A.4.
//!
//! Erzeugt eine Challenge bei Abweichung zwischen primärem und redundantem Pod.
//! Die Challenge identifiziert die erste abweichende Spur-Position und startet
//! das Bisektions-Spiel.
//!
//! **Konsens-Feld:** Die Challenge-Struktur ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;
use myl_types::ids::{MinerId, SegmentId};

/// Eine Challenge im Bisektions-Spiel.
///
/// Wird erzeugt, wenn ein Check eine Abweichung feststellt. Die Challenge
/// identifiziert das Segment, die abweichende Position und die beteiligten
/// Miner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    /// ID des betroffenen Segments.
    pub segment_id: SegmentId,
    /// Index der ersten abweichenden Spur-Position (0-basiert).
    pub first_divergence: usize,
    /// Miner des primären Pods (Angeklagter).
    pub primary_miner: MinerId,
    /// Miner des redundanten Pods (Checker).
    pub redundant_miner: MinerId,
    /// Commitment-Hash des primären Pods an der abweichenden Position.
    pub primary_hash: Hash,
    /// Commitment-Hash des redundanten Pods an der abweichenden Position.
    pub redundant_hash: Hash,
    /// Zeitstempel der Challenge-Erzeugung (Unix-Millisekunden).
    pub timestamp_ms: u64,
}

/// Fehler bei der Challenge-Erzeugung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeError {
    /// Keine Abweichung gefunden (Challenge nicht nötig).
    NoDivergence,
    /// Ungültige Position (außerhalb des Spur-Bereichs).
    InvalidPosition { position: usize, trace_len: usize },
}

impl std::fmt::Display for ChallengeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDivergence => write!(f, "Keine Abweichung gefunden"),
            Self::InvalidPosition { position, trace_len } => {
                write!(
                    f,
                    "Ungültige Position {} (Spur-Länge: {})",
                    position, trace_len
                )
            }
        }
    }
}

impl std::error::Error for ChallengeError {}

/// Erzeugt eine Challenge aus einer Abweichung.
///
/// **Parameter:**
/// - `segment_id`: ID des betroffenen Segments
/// - `first_divergence`: Index der ersten abweichenden Position
/// - `primary_miner`: Miner des primären Pods
/// - `redundant_miner`: Miner des redundanten Pods
/// - `primary_hashes`: Commitment-Hashes des primären Pods
/// - `redundant_hashes`: Commitment-Hashes des redundanten Pods
/// - `timestamp_ms`: Zeitstempel der Challenge-Erzeugung
///
/// **Returns:** `Challenge` bei erfolgreicher Erzeugung.
///
/// **Fehler:** `ChallengeError` wenn keine Abweichung vorhanden oder die
/// Position ungültig ist.
pub fn create_challenge(
    segment_id: SegmentId,
    first_divergence: usize,
    primary_miner: MinerId,
    redundant_miner: MinerId,
    primary_hashes: &[Hash],
    redundant_hashes: &[Hash],
    timestamp_ms: u64,
) -> Result<Challenge, ChallengeError> {
    // Validierung
    if first_divergence >= primary_hashes.len() || first_divergence >= redundant_hashes.len() {
        return Err(ChallengeError::InvalidPosition {
            position: first_divergence,
            trace_len: primary_hashes.len().min(redundant_hashes.len()),
        });
    }

    let primary_hash = primary_hashes[first_divergence];
    let redundant_hash = redundant_hashes[first_divergence];

    // Prüfe, dass tatsächlich eine Abweichung vorliegt
    if primary_hash == redundant_hash {
        return Err(ChallengeError::NoDivergence);
    }

    Ok(Challenge {
        segment_id,
        first_divergence,
        primary_miner,
        redundant_miner,
        primary_hash,
        redundant_hash,
        timestamp_ms,
    })
}

/// Findet die erste abweichende Position zwischen zwei Spuren.
///
/// **Parameter:**
/// - `primary_hashes`: Commitment-Hashes des primären Pods
/// - `redundant_hashes`: Commitment-Hashes des redundanten Pods
///
/// **Returns:** `Some(index)` der ersten abweichenden Position, `None` wenn
/// alle Hashes übereinstimmen.
pub fn find_first_divergence(
    primary_hashes: &[Hash],
    redundant_hashes: &[Hash],
) -> Option<usize> {
    primary_hashes
        .iter()
        .zip(redundant_hashes.iter())
        .position(|(p, r)| p != r)
}

/// Berechnet den Challenge-Hash (für On-Chain-Referenz).
///
/// **Returns:** SHA-256 Hash über die Challenge-Felder.
pub fn challenge_hash(challenge: &Challenge) -> Hash {
    let mut data = Vec::new();
    data.extend_from_slice(challenge.segment_id.as_bytes());
    data.extend_from_slice(&(challenge.first_divergence as u64).to_le_bytes());
    data.extend_from_slice(challenge.primary_miner.as_bytes());
    data.extend_from_slice(challenge.redundant_miner.as_bytes());
    data.extend_from_slice(challenge.primary_hash.as_bytes());
    data.extend_from_slice(challenge.redundant_hash.as_bytes());
    data.extend_from_slice(&challenge.timestamp_ms.to_le_bytes());
    Hash::sha256(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hashes(len: usize) -> Vec<Hash> {
        (0..len).map(|i| Hash::sha256(&[i as u8])).collect()
    }

    #[test]
    fn create_challenge_success() {
        let segment_id = SegmentId::new([1u8; 32]);
        let primary_miner = MinerId::new([2u8; 32]);
        let redundant_miner = MinerId::new([3u8; 32]);
        let primary_hashes = test_hashes(10);
        let mut redundant_hashes = primary_hashes.clone();
        redundant_hashes[5] = Hash::sha256(b"different");

        let challenge = create_challenge(
            segment_id,
            5,
            primary_miner,
            redundant_miner,
            &primary_hashes,
            &redundant_hashes,
            1000,
        )
        .unwrap();

        assert_eq!(challenge.segment_id, segment_id);
        assert_eq!(challenge.first_divergence, 5);
        assert_eq!(challenge.primary_miner, primary_miner);
        assert_eq!(challenge.redundant_miner, redundant_miner);
        assert_eq!(challenge.primary_hash, primary_hashes[5]);
        assert_eq!(challenge.redundant_hash, redundant_hashes[5]);
        assert_eq!(challenge.timestamp_ms, 1000);
    }

    #[test]
    fn create_challenge_no_divergence() {
        let segment_id = SegmentId::new([1u8; 32]);
        let primary_miner = MinerId::new([2u8; 32]);
        let redundant_miner = MinerId::new([3u8; 32]);
        let hashes = test_hashes(10);

        let result = create_challenge(
            segment_id,
            5,
            primary_miner,
            redundant_miner,
            &hashes,
            &hashes,
            1000,
        );

        assert!(matches!(result, Err(ChallengeError::NoDivergence)));
    }

    #[test]
    fn create_challenge_invalid_position() {
        let segment_id = SegmentId::new([1u8; 32]);
        let primary_miner = MinerId::new([2u8; 32]);
        let redundant_miner = MinerId::new([3u8; 32]);
        let primary_hashes = test_hashes(10);
        let redundant_hashes = test_hashes(10);

        let result = create_challenge(
            segment_id,
            15, // Außerhalb des Bereichs
            primary_miner,
            redundant_miner,
            &primary_hashes,
            &redundant_hashes,
            1000,
        );

        assert!(matches!(
            result,
            Err(ChallengeError::InvalidPosition {
                position: 15,
                trace_len: 10
            })
        ));
    }

    #[test]
    fn find_first_divergence_at_start() {
        let primary = test_hashes(10);
        let mut redundant = primary.clone();
        redundant[0] = Hash::sha256(b"different");

        let div = find_first_divergence(&primary, &redundant);
        assert_eq!(div, Some(0));
    }

    #[test]
    fn find_first_divergence_at_middle() {
        let primary = test_hashes(10);
        let mut redundant = primary.clone();
        redundant[5] = Hash::sha256(b"different");

        let div = find_first_divergence(&primary, &redundant);
        assert_eq!(div, Some(5));
    }

    #[test]
    fn find_first_divergence_at_end() {
        let primary = test_hashes(10);
        let mut redundant = primary.clone();
        redundant[9] = Hash::sha256(b"different");

        let div = find_first_divergence(&primary, &redundant);
        assert_eq!(div, Some(9));
    }

    #[test]
    fn find_first_divergence_none() {
        let hashes = test_hashes(10);
        let div = find_first_divergence(&hashes, &hashes);
        assert_eq!(div, None);
    }

    #[test]
    fn challenge_hash_deterministic() {
        let challenge = Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 5,
            primary_miner: MinerId::new([2u8; 32]),
            redundant_miner: MinerId::new([3u8; 32]),
            primary_hash: Hash::sha256(b"primary"),
            redundant_hash: Hash::sha256(b"redundant"),
            timestamp_ms: 1000,
        };

        let hash1 = challenge_hash(&challenge);
        let hash2 = challenge_hash(&challenge);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn challenge_hash_different_for_different_challenges() {
        let challenge1 = Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 5,
            primary_miner: MinerId::new([2u8; 32]),
            redundant_miner: MinerId::new([3u8; 32]),
            primary_hash: Hash::sha256(b"primary"),
            redundant_hash: Hash::sha256(b"redundant"),
            timestamp_ms: 1000,
        };

        let challenge2 = Challenge {
            first_divergence: 6, // Anders
            ..challenge1.clone()
        };

        let hash1 = challenge_hash(&challenge1);
        let hash2 = challenge_hash(&challenge2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn challenge_equality() {
        let challenge1 = Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 5,
            primary_miner: MinerId::new([2u8; 32]),
            redundant_miner: MinerId::new([3u8; 32]),
            primary_hash: Hash::sha256(b"primary"),
            redundant_hash: Hash::sha256(b"redundant"),
            timestamp_ms: 1000,
        };

        let challenge2 = challenge1.clone();

        assert_eq!(challenge1, challenge2);
    }
}
