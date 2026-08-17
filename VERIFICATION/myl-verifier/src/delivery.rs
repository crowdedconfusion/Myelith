//! Auslieferungsmodi — Whitepaper Kap. 6.4.
//!
//! Zwei Modi für die Ergebnis-Auslieferung:
//! - **Optimistic:** Sofortige Auslieferung + asynchroner Abgleich
//! - **Confirmed:** Zurückhalten bis Übereinstimmung bestätigt
//!
//! **Konsens-Feld:** Die Auslieferungslogik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::redundancy::{compare_commitments, CompareResult, RedundancyError, VerificationMode};
use myl_types::hash::Hash;

/// Entscheidung für die Ergebnis-Auslieferung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryDecision {
    /// Ergebnis sofort ausliefern (optimistisch).
    Deliver,
    /// Ergebnis zurückhalten bis Bestätigung (konservativ).
    Hold,
    /// Ergebnis ausliefern, aber Slashing einleiten (bei Abweichung).
    DeliverAndSlash {
        /// Index der ersten abweichenden Position.
        first_divergence: usize,
    },
}

/// Fehler bei der Auslieferungsentscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryError {
    /// Fehler beim Commitment-Vergleich.
    ComparisonError(RedundancyError),
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ComparisonError(e) => write!(f, "Vergleichsfehler: {}", e),
        }
    }
}

impl std::error::Error for DeliveryError {}

impl From<RedundancyError> for DeliveryError {
    fn from(e: RedundancyError) -> Self {
        Self::ComparisonError(e)
    }
}

/// Entscheidet über die Auslieferung basierend auf dem Verifikationsmodus
/// und dem Vergleichsergebnis.
///
/// **Parameter:**
/// - `mode`: Auslieferungsmodus (Optimistic oder Confirmed)
/// - `primary_trace`: Commitment-Hashes des primären Pods
/// - `redundant_trace`: Commitment-Hashes des redundanten Pods
///
/// **Returns:** `DeliveryDecision` basierend auf Modus und Vergleich:
/// - **Optimistic + Match:** `Deliver`
/// - **Optimistic + Mismatch:** `DeliverAndSlash { first_divergence }`
/// - **Confirmed + Match:** `Deliver`
/// - **Confirmed + Mismatch:** `Hold`
///
/// **Fehler:** `DeliveryError` wenn der Vergleich fehlschlägt.
///
/// **Beispiel:**
/// ```
/// use myl_verifier::{decide_delivery, VerificationMode};
/// use myl_types::hash::Hash;
///
/// let primary = vec![Hash::sha256(b"layer-0"), Hash::sha256(b"layer-1")];
/// let redundant = primary.clone();
///
/// let decision = decide_delivery(
///     VerificationMode::Optimistic,
///     &primary,
///     &redundant,
/// ).unwrap();
/// assert!(matches!(decision, myl_verifier::DeliveryDecision::Deliver));
/// ```
pub fn decide_delivery(
    mode: VerificationMode,
    primary_trace: &[Hash],
    redundant_trace: &[Hash],
) -> Result<DeliveryDecision, DeliveryError> {
    let comparison = compare_commitments(primary_trace, redundant_trace)?;

    match (mode, comparison) {
        // Optimistic: Immer ausliefern, bei Mismatch slashing
        (VerificationMode::Optimistic, CompareResult::Match) => Ok(DeliveryDecision::Deliver),
        (VerificationMode::Optimistic, CompareResult::Mismatch { first_divergence }) => {
            Ok(DeliveryDecision::DeliverAndSlash { first_divergence })
        }

        // Confirmed: Nur bei Match ausliefern, sonst halten
        (VerificationMode::Confirmed, CompareResult::Match) => Ok(DeliveryDecision::Deliver),
        (VerificationMode::Confirmed, CompareResult::Mismatch { .. }) => Ok(DeliveryDecision::Hold),
    }
}

/// Prüft, ob eine Auslieferung erfolgen soll (für Confirmed-Modus).
///
/// **Parameter:**
/// - `primary_trace`: Commitment-Hashes des primären Pods
/// - `redundant_trace`: Commitment-Hashes des redundanten Pods
///
/// **Returns:** `true` wenn alle Hashes übereinstimmen (Auslieferung möglich),
/// `false` sonst.
pub fn should_deliver_confirmed(
    primary_trace: &[Hash],
    redundant_trace: &[Hash],
) -> Result<bool, DeliveryError> {
    let comparison = compare_commitments(primary_trace, redundant_trace)?;
    Ok(matches!(comparison, CompareResult::Match))
}

/// Gibt die erste abweichende Position zurück (für Slashing-Entscheidung).
///
/// **Parameter:**
/// - `primary_trace`: Commitment-Hashes des primären Pods
/// - `redundant_trace`: Commitment-Hashes des redundanten Pods
///
/// **Returns:** `Some(index)` wenn Abweichung gefunden, `None` bei Match.
pub fn first_divergence(
    primary_trace: &[Hash],
    redundant_trace: &[Hash],
) -> Result<Option<usize>, DeliveryError> {
    let comparison = compare_commitments(primary_trace, redundant_trace)?;
    match comparison {
        CompareResult::Match => Ok(None),
        CompareResult::Mismatch { first_divergence } => Ok(Some(first_divergence)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_trace(len: usize) -> Vec<Hash> {
        (0..len).map(|i| Hash::sha256(&[i as u8])).collect()
    }

    #[test]
    fn optimistic_match_delivers() {
        let trace = test_trace(10);
        let decision = decide_delivery(VerificationMode::Optimistic, &trace, &trace).unwrap();
        assert_eq!(decision, DeliveryDecision::Deliver);
    }

    #[test]
    fn optimistic_mismatch_delivers_and_slashes() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[5] = Hash::sha256(b"different");

        let decision = decide_delivery(VerificationMode::Optimistic, &primary, &redundant).unwrap();
        assert_eq!(
            decision,
            DeliveryDecision::DeliverAndSlash { first_divergence: 5 }
        );
    }

    #[test]
    fn confirmed_match_delivers() {
        let trace = test_trace(10);
        let decision = decide_delivery(VerificationMode::Confirmed, &trace, &trace).unwrap();
        assert_eq!(decision, DeliveryDecision::Deliver);
    }

    #[test]
    fn confirmed_mismatch_holds() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[3] = Hash::sha256(b"different");

        let decision = decide_delivery(VerificationMode::Confirmed, &primary, &redundant).unwrap();
        assert_eq!(decision, DeliveryDecision::Hold);
    }

    #[test]
    fn should_deliver_confirmed_match() {
        let trace = test_trace(10);
        let should = should_deliver_confirmed(&trace, &trace).unwrap();
        assert!(should);
    }

    #[test]
    fn should_deliver_confirmed_mismatch() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[0] = Hash::sha256(b"different");

        let should = should_deliver_confirmed(&primary, &redundant).unwrap();
        assert!(!should);
    }

    #[test]
    fn first_divergence_match() {
        let trace = test_trace(10);
        let div = first_divergence(&trace, &trace).unwrap();
        assert_eq!(div, None);
    }

    #[test]
    fn first_divergence_mismatch() {
        let primary = test_trace(10);
        let mut redundant = primary.clone();
        redundant[7] = Hash::sha256(b"different");

        let div = first_divergence(&primary, &redundant).unwrap();
        assert_eq!(div, Some(7));
    }

    #[test]
    fn delivery_error_from_redundancy_error() {
        let primary: Vec<Hash> = vec![];
        let redundant: Vec<Hash> = vec![];

        let result = decide_delivery(VerificationMode::Optimistic, &primary, &redundant);
        assert!(matches!(result, Err(DeliveryError::ComparisonError(_))));
    }

    #[test]
    fn delivery_decision_equality() {
        assert_eq!(DeliveryDecision::Deliver, DeliveryDecision::Deliver);
        assert_eq!(DeliveryDecision::Hold, DeliveryDecision::Hold);
        assert_eq!(
            DeliveryDecision::DeliverAndSlash { first_divergence: 5 },
            DeliveryDecision::DeliverAndSlash { first_divergence: 5 }
        );
        assert_ne!(DeliveryDecision::Deliver, DeliveryDecision::Hold);
    }
}
