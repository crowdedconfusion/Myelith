//! Checker-Modul (audit) — Whitepaper Anhang A.4, Kap. 6.6.
//!
//! Rechnet VRF-ausgelöste Segmente nach und vergleicht das Ergebnis
//! mit dem behaupteten Hash. Bei Abweichung wird eine Challenge erzeugt.
//!
//! **Konsens-Feld:** Der Check-Algorithmus ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Das Checker-Modul definiert ein abstraktes Interface für
//! die Segment-Nachrechnung. Die konkrete Implementierung erfolgt durch
//! Integration mit INTEGER_LLMs Runtime (forward pass).

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

/// Fehler beim Segment-Check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    /// Segment ist leer.
    EmptySegment,
    /// Forward-Pass fehlgeschlagen.
    ForwardPassFailed,
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySegment => write!(f, "Segment ist leer"),
            Self::ForwardPassFailed => write!(f, "Forward-Pass fehlgeschlagen"),
        }
    }
}

impl std::error::Error for CheckError {}

/// Ergebnis eines Segment-Checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    /// Segment stimmt überein (Hash match).
    Valid,
    /// Segment weicht ab (Hash mismatch).
    Invalid {
        /// Index der ersten abweichenden Layer.
        first_divergence: usize,
    },
}

/// Abstraktes Interface für Segment-Nachrechnung.
///
/// Dieses Trait definiert die Schnittstelle für die Nachrechnung eines
/// Segments. Die konkrete Implementierung erfolgt durch Integration mit
/// INTEGER_LLMs Runtime.
///
/// **Implementierungshinweis:** Der Forward-Pass muss deterministisch und
/// bitgleich sein (INTEGER_LLM θ_v-Vertrag).
pub trait SegmentAuditor {
    /// Rechnet ein Segment nach und gibt die Commitment-Hashes zurück.
    ///
    /// **Parameter:**
    /// - `segment_id`: ID des zu prüfenden Segments
    /// - `input_activations`: Eingabe-Aktivierungen (a_0)
    ///
    /// **Returns:** Vektor von Commitment-Hashes (h(a_0), h(a_1), ..., h(a_k))
    ///
    /// **Fehler:** `CheckError` wenn der Forward-Pass fehlschlägt.
    fn audit_segment(
        &self,
        segment_id: SegmentId,
        input_activations: &[u8],
    ) -> Result<Vec<Hash>, CheckError>;
}

/// Prüft ein Segment gegen einen behaupteten Commitment-Hash.
///
/// **Parameter:**
/// - `auditor`: Implementierung des SegmentAuditor-Traits
/// - `segment_id`: ID des zu prüfenden Segments
/// - `input_activations`: Eingabe-Aktivierungen (a_0)
/// - `claimed_hashes`: Behauptete Commitment-Hashes
///
/// **Returns:** `CheckResult::Valid` wenn alle Hashes übereinstimmen,
/// `CheckResult::Invalid { first_divergence }` sonst.
///
/// **Fehler:** `CheckError` wenn der Forward-Pass fehlschlägt.
pub fn check_segment(
    auditor: &dyn SegmentAuditor,
    segment_id: SegmentId,
    input_activations: &[u8],
    claimed_hashes: &[Hash],
) -> Result<CheckResult, CheckError> {
    if input_activations.is_empty() {
        return Err(CheckError::EmptySegment);
    }

    let computed_hashes = auditor.audit_segment(segment_id, input_activations)?;

    if computed_hashes.len() != claimed_hashes.len() {
        // Längen-Mismatch → erste Position als Divergenz melden
        return Ok(CheckResult::Invalid {
            first_divergence: 0,
        });
    }

    // Binärer Vergleich an allen Positionen
    for (i, (computed, claimed)) in computed_hashes.iter().zip(claimed_hashes.iter()).enumerate() {
        if computed != claimed {
            return Ok(CheckResult::Invalid {
                first_divergence: i,
            });
        }
    }

    Ok(CheckResult::Valid)
}

/// Mock-Auditor für Tests.
///
/// Gibt vordefinierte Hashes zurück, um die Check-Logik zu testen.
#[cfg(test)]
pub struct MockAuditor {
    hashes: Vec<Hash>,
}

#[cfg(test)]
impl MockAuditor {
    pub fn new(hashes: Vec<Hash>) -> Self {
        Self { hashes }
    }
}

#[cfg(test)]
impl SegmentAuditor for MockAuditor {
    fn audit_segment(
        &self,
        _segment_id: SegmentId,
        _input_activations: &[u8],
    ) -> Result<Vec<Hash>, CheckError> {
        Ok(self.hashes.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hashes(len: usize) -> Vec<Hash> {
        (0..len).map(|i| Hash::sha256(&[i as u8])).collect()
    }

    #[test]
    fn check_valid_segment() {
        let hashes = test_hashes(10);
        let auditor = MockAuditor::new(hashes.clone());
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &hashes).unwrap();
        assert_eq!(result, CheckResult::Valid);
    }

    #[test]
    fn check_invalid_segment_first_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[0] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 0 });
    }

    #[test]
    fn check_invalid_segment_middle_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[5] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 5 });
    }

    #[test]
    fn check_invalid_segment_last_position() {
        let computed = test_hashes(10);
        let mut claimed = computed.clone();
        claimed[9] = Hash::sha256(b"different");

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 9 });
    }

    #[test]
    fn check_length_mismatch() {
        let computed = test_hashes(10);
        let claimed = test_hashes(5);

        let auditor = MockAuditor::new(computed);
        let segment_id = SegmentId::new([1u8; 32]);
        let input = vec![1, 2, 3];

        let result = check_segment(&auditor, segment_id, &input, &claimed).unwrap();
        assert_eq!(result, CheckResult::Invalid { first_divergence: 0 });
    }

    #[test]
    fn check_empty_input() {
        let hashes = test_hashes(10);
        let auditor = MockAuditor::new(hashes);
        let segment_id = SegmentId::new([1u8; 32]);
        let input: Vec<u8> = vec![];

        let result = check_segment(&auditor, segment_id, &input, &[]);
        assert!(matches!(result, Err(CheckError::EmptySegment)));
    }

    #[test]
    fn check_result_equality() {
        assert_eq!(CheckResult::Valid, CheckResult::Valid);
        assert_eq!(
            CheckResult::Invalid { first_divergence: 5 },
            CheckResult::Invalid { first_divergence: 5 }
        );
        assert_ne!(CheckResult::Valid, CheckResult::Invalid { first_divergence: 0 });
    }
}
