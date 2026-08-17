//! Streit-Artefakte der Verifikation — Whitepaper Kap. 6.4–6.6, Anhang A.4.
//!
//! Eine `Challenge` ist das On-Chain-Artefakt, mit dem ein Checker eine
//! Abweichung zwischen zwei redundanten Pods anzeigt und das
//! Bisektions-Spiel eröffnet. Sie entsteht in VERIFICATION, wird über
//! NETWORKING verbreitet und landet im Block (CONSENSUS).
//!
//! **Warum der Typ hier liegt:** Er wird von drei Komponenten benutzt,
//! die einander nicht kennen dürfen — VERIFICATION erzeugt ihn,
//! NETWORKING validiert ihn beim Gossip, CONSENSUS nimmt ihn in den
//! Block auf. Läge er in einer dieser Komponenten, müsste die
//! Schichtung verletzt werden (L0 Networking hinge an L1 Consensus).
//! Bis v0.2.4 existierten stattdessen **zwei** unabhängige
//! `Challenge`-Definitionen: eine in `myl-verifier` (mit beiden Pods und
//! beiden Hashes) und eine schmalere in `myl-consensus::block` — der
//! Block konnte also gar nicht aufnehmen, was der Verifier produziert.
//!
//! **Konsens-Feld:** Die Kodierung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::hash::Hash;
use crate::ids::{MinerId, SegmentId};
use borsh::{BorshDeserialize, BorshSerialize};

/// Eine Challenge: Anzeige einer Abweichung, Start des Bisektions-Spiels.
///
/// Enthält beide Seiten des Streits — ohne den Hash der Gegenseite wäre
/// die Anzeige nicht nachprüfbar und die Schuldzuweisung nicht eindeutig
/// (Kap. 6.6: „Die Schuldzuweisung ist eindeutig, weil das Ergebnis
/// kanonisch ist").
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
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

impl Challenge {
    /// Strukturelle Plausibilitätsprüfung ohne Kenntnis der Spur.
    ///
    /// Prüft, was ohne weiteren Kontext entscheidbar ist: Die beiden
    /// Pods müssen verschieden sein, und die angezeigten Hashes müssen
    /// tatsächlich abweichen — sonst gibt es nichts zu streiten. Das ist
    /// bewusst **keine** vollständige Gültigkeitsprüfung; die verlangt
    /// die Segment-Spur und findet in VERIFICATION statt.
    ///
    /// Für den Gossip-Layer reicht diese Stufe, um offensichtlichen
    /// Unsinn zu verwerfen, bevor er weiterverbreitet wird.
    pub fn validate_structure(&self) -> Result<(), ChallengeStructureError> {
        if self.primary_miner == self.redundant_miner {
            return Err(ChallengeStructureError::IdenticalMiners);
        }
        if self.primary_hash == self.redundant_hash {
            return Err(ChallengeStructureError::IdenticalHashes);
        }
        Ok(())
    }
}

/// Fehler der strukturellen Challenge-Prüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeStructureError {
    /// Primärer und redundanter Pod sind derselbe Miner.
    IdenticalMiners,
    /// Beide Hashes sind gleich — es liegt keine Abweichung vor.
    IdenticalHashes,
}

impl core::fmt::Display for ChallengeStructureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IdenticalMiners => write!(f, "Primärer und redundanter Miner sind identisch"),
            Self::IdenticalHashes => write!(f, "Beide Commitment-Hashes sind gleich"),
        }
    }
}

impl std::error::Error for ChallengeStructureError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn challenge() -> Challenge {
        Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: miner(1),
            redundant_miner: miner(2),
            primary_hash: Hash::sha256(b"a"),
            redundant_hash: Hash::sha256(b"b"),
            timestamp_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn gueltige_challenge() {
        assert!(challenge().validate_structure().is_ok());
    }

    #[test]
    fn gleiche_miner_werden_abgelehnt() {
        let mut c = challenge();
        c.redundant_miner = c.primary_miner;
        assert_eq!(
            c.validate_structure(),
            Err(ChallengeStructureError::IdenticalMiners)
        );
    }

    #[test]
    fn gleiche_hashes_werden_abgelehnt() {
        let mut c = challenge();
        c.redundant_hash = c.primary_hash;
        assert_eq!(
            c.validate_structure(),
            Err(ChallengeStructureError::IdenticalHashes)
        );
    }

    #[test]
    fn borsh_rundtrip() {
        let c = challenge();
        let bytes = borsh::to_vec(&c).unwrap();
        assert_eq!(borsh::from_slice::<Challenge>(&bytes).unwrap(), c);
    }
}
