//! Redundanzvergleich (Stufe 1) — Whitepaper Kap. 6.4.
//!
//! Vergleicht die Commitment-Hashes zweier Pods an allen Spur-Positionen.
//! Der Vergleich ist binär (gleich/ungleich), parameterfrei (kein Schwellenwert).
//!
//! **Zwei Auslieferungsmodi:**
//! - **Optimistic:** Sofortige Auslieferung + asynchroner Abgleich
//! - **Confirmed:** Zurückhalten bis Übereinstimmung
//!
//! **Konsens-Feld:** Der Vergleichsalgorithmus ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;

/// Fehler beim Redundanzvergleich.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyError {
    /// Die Spuren haben unterschiedliche Längen.
    LengthMismatch {
        primary_len: usize,
        redundant_len: usize,
    },
    /// Eine der Spuren ist leer.
    EmptyTrace,
}

impl std::fmt::Display for RedundancyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch { primary_len, redundant_len } => {
                write!(
                    f,
                    "Spur-Längen mismatch: primär={}, redundant={}",
                    primary_len, redundant_len
                )
            }
            Self::EmptyTrace => write!(f, "Spur ist leer"),
        }
    }
}

impl std::error::Error for RedundancyError {}

/// Ergebnis des Commitment-Hash-Vergleichs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareResult {
    /// Alle Hashes stimmen überein.
    Match,
    /// Hashes weichen ab an der angegebenen Position.
    Mismatch {
        /// Index der ersten abweichenden Position (0-basiert).
        first_divergence: usize,
    },
}

/// Auslieferungsmodus für Verifikationsergebnisse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationMode {
    /// Sofortige Auslieferung + asynchroner Abgleich.
    /// Bei Abweichung: Slashing + vTFE-Rückbuchung.
    Optimistic,
    /// Zurückhalten bis Übereinstimmung bestätigt.
    /// Keine sofortige Auslieferung.
    Confirmed,
}

/// Vergleicht die Commitment-Hashes zweier Pods an allen Spur-Positionen.
///
/// **Parameter:**
/// - `primary_trace`: Commitment-Hashes des primären Pods (L Hashes)
/// - `redundant_trace`: Commitment-Hashes des redundanten Pods (L Hashes)
///
/// **Returns:** `CompareResult::Match` wenn alle Hashes übereinstimmen,
/// `CompareResult::Mismatch { first_divergence }` mit der ersten abweichenden
/// Position sonst.
///
/// **Fehler:** `RedundancyError` wenn die Spuren unterschiedliche Längen haben
/// oder leer sind.
///
/// **Beispiel:**
/// ```
/// use myl_verifier::compare_commitments;
/// use myl_types::hash::Hash;
///
/// let primary = vec![
///     Hash::sha256(b"layer-0"),
///     Hash::sha256(b"layer-1"),
///     Hash::sha256(b"layer-2"),
/// ];
/// let redundant = primary.clone();
///
/// let result = compare_commitments(&primary, &redundant).unwrap();
/// assert!(matches!(result, myl_verifier::CompareResult::Match));
/// ```
pub fn compare_commitments(
    primary_trace: &[Hash],
    redundant_trace: &[Hash],
) -> Result<CompareResult, RedundancyError> {
    // Validierung
    if primary_trace.is_empty() || redundant_trace.is_empty() {
        return Err(RedundancyError::EmptyTrace);
    }

    if primary_trace.len() != redundant_trace.len() {
        return Err(RedundancyError::LengthMismatch {
            primary_len: primary_trace.len(),
            redundant_len: redundant_trace.len(),
        });
    }

    // Binärer Vergleich an allen Positionen
    for (i, (primary, redundant)) in primary_trace.iter().zip(redundant_trace.iter()).enumerate() {
        if primary != redundant {
            return Ok(CompareResult::Mismatch {
                first_divergence: i,
            });
        }
    }

    Ok(CompareResult::Match)
}

/// Findet alle abweichenden Positionen zwischen zwei Spuren.
///
/// **Parameter:**
/// - `primary_trace`: Commitment-Hashes des primären Pods
/// - `redundant_trace`: Commitment-Hashes des redundanten Pods
///
/// **Returns:** Liste aller abweichenden Positionen (0-basiert).
///
/// **Fehler:** `RedundancyError` wenn die Spuren unterschiedliche Längen haben
/// oder leer sind.
pub fn find_all_divergences(
    primary_trace: &[Hash],
    redundant_trace: &[Hash],
) -> Result<Vec<usize>, RedundancyError> {
    // Validierung
    if primary_trace.is_empty() || redundant_trace.is_empty() {
        return Err(RedundancyError::EmptyTrace);
    }

    if primary_trace.len() != redundant_trace.len() {
        return Err(RedundancyError::LengthMismatch {
            primary_len: primary_trace.len(),
            redundant_len: redundant_trace.len(),
        });
    }

    // Alle abweichenden Positionen sammeln
    let divergences: Vec<usize> = primary_trace
        .iter()
        .zip(redundant_trace.iter())
        .enumerate()
        .filter(|(_, (primary, redundant))| primary != redundant)
        .map(|(i, _)| i)
        .collect();

    Ok(divergences)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_trace(len: usize) -> Vec<Hash> {
        (0..len).map(|i| Hash::sha256(&[i as u8])).collect()
    }

    #[test]
    fn compare_identical_traces() {
        let trace = test_trace(10);
        let result = compare_commitments(&trace, &trace).unwrap();
        assert_eq!(result, CompareResult::Match);
    }

    #[test]
    fn compare_divergent_at_first() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[0] = Hash::sha256(b"different");

        let result = compare_commitments(&primary, &redundant).unwrap();
        assert_eq!(result, CompareResult::Mismatch { first_divergence: 0 });
    }

    #[test]
    fn compare_divergent_at_middle() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[5] = Hash::sha256(b"different");

        let result = compare_commitments(&primary, &redundant).unwrap();
        assert_eq!(result, CompareResult::Mismatch { first_divergence: 5 });
    }

    #[test]
    fn compare_divergent_at_last() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[9] = Hash::sha256(b"different");

        let result = compare_commitments(&primary, &redundant).unwrap();
        assert_eq!(result, CompareResult::Mismatch { first_divergence: 9 });
    }

    #[test]
    fn compare_multiple_divergences() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[2] = Hash::sha256(b"diff-2");
        redundant[5] = Hash::sha256(b"diff-5");
        redundant[8] = Hash::sha256(b"diff-8");

        let result = compare_commitments(&primary, &redundant).unwrap();
        // Should report first divergence
        assert_eq!(result, CompareResult::Mismatch { first_divergence: 2 });

        // find_all_divergences should report all
        let all = find_all_divergences(&primary, &redundant).unwrap();
        assert_eq!(all, vec![2, 5, 8]);
    }

    #[test]
    fn compare_length_mismatch() {
        let primary = test_trace(10);
        let redundant = test_trace(5);

        let result = compare_commitments(&primary, &redundant);
        assert!(matches!(result, Err(RedundancyError::LengthMismatch { .. })));
    }

    #[test]
    fn compare_empty_trace() {
        let primary: Vec<Hash> = vec![];
        let redundant: Vec<Hash> = vec![];

        let result = compare_commitments(&primary, &redundant);
        assert!(matches!(result, Err(RedundancyError::EmptyTrace)));
    }

    #[test]
    fn find_all_divergences_identical() {
        let trace = test_trace(10);
        let divergences = find_all_divergences(&trace, &trace).unwrap();
        assert!(divergences.is_empty());
    }

    #[test]
    fn find_all_divergences_multiple() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[1] = Hash::sha256(b"diff-1");
        redundant[3] = Hash::sha256(b"diff-3");
        redundant[7] = Hash::sha256(b"diff-7");

        let divergences = find_all_divergences(&primary, &redundant).unwrap();
        assert_eq!(divergences, vec![1, 3, 7]);
    }

    #[test]
    fn verification_mode_variants() {
        assert_eq!(VerificationMode::Optimistic, VerificationMode::Optimistic);
        assert_eq!(VerificationMode::Confirmed, VerificationMode::Confirmed);
        assert_ne!(VerificationMode::Optimistic, VerificationMode::Confirmed);
    }

    #[test]
    fn compare_result_variants() {
        assert_eq!(CompareResult::Match, CompareResult::Match);
        assert_eq!(
            CompareResult::Mismatch { first_divergence: 5 },
            CompareResult::Mismatch { first_divergence: 5 }
        );
        assert_ne!(CompareResult::Match, CompareResult::Mismatch { first_divergence: 0 });
    }
}
