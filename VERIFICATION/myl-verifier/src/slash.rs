//! Slash-/Kopfgeld-Auszahlung — Whitepaper Kap. 6.6, Anhang A.4.
//!
//! Bestimmt die Slash-Entscheidung basierend auf dem Bisektionsergebnis.
//! Der Verlierer wird geslasht, der Gewinner erhält ein Kopfgeld.
//!
//! **Konsens-Feld:** Die Slash-Logik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::ids::MinerId;

/// Ergebnis der Schiedsrunde (wer hat gewonnen/verloren).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictOutcome {
    /// Primärer Pod hat verloren (war fehlerhaft).
    PrimaryLoses,
    /// Redundanter Pod hat verloren (hat falsch challenget).
    RedundantLoses,
}

/// Eine Slash-Entscheidung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashDecision {
    /// Miner, der geslasht wird (Verlierer).
    pub slashed_miner: MinerId,
    /// Miner, der das Kopfgeld erhält (Gewinner).
    pub rewarded_miner: MinerId,
    /// Slash-Betrag (in MYL-Kleinstbeträgen).
    pub slash_amount: u64,
    /// Kopfgeld-Betrag (in MYL-Kleinstbeträgen).
    pub reward_amount: u64,
    /// Grund der Slash-Entscheidung.
    pub reason: SlashReason,
}

/// Grund der Slash-Entscheidung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashReason {
    /// Primärer Pod hat fehlerhafte Berechnung durchgeführt.
    PrimaryFault {
        /// Position der Abweichung.
        divergence_position: usize,
    },
    /// Redundanter Pod hat falsche Challenge eingereicht.
    RedundantFault,
}

/// Fehler bei der Slash-Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashError {
    /// Ungültige Slash-Parameter (z.B. negative Beträge).
    InvalidParameters,
    /// Miner-IDs sind identisch (kein sinnvoller Slash).
    IdenticalMiners,
}

impl std::fmt::Display for SlashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameters => write!(f, "Ungültige Slash-Parameter"),
            Self::IdenticalMiners => write!(f, "Miner-IDs sind identisch"),
        }
    }
}

impl std::error::Error for SlashError {}

/// Konfigurationsparameter für Slash-Entscheidungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashConfig {
    /// Slash-Betrag für den Verlierer (in MYL-Kleinstbeträgen).
    pub slash_amount: u64,
    /// Kopfgeld-Betrag für den Gewinner (in MYL-Kleinstbeträgen).
    pub reward_amount: u64,
}

impl Default for SlashConfig {
    fn default() -> Self {
        Self {
            slash_amount: 1_000_000, // 1 MYL
            reward_amount: 500_000,  // 0.5 MYL
        }
    }
}

/// Erstellt eine Slash-Entscheidung basierend auf dem Verdict.
///
/// **Parameter:**
/// - `outcome`: Ergebnis der Schiedsrunde
/// - `primary_miner`: Miner des primären Pods
/// - `redundant_miner`: Miner des redundanten Pods
/// - `config`: Slash-Konfiguration
/// - `divergence_position`: Position der Abweichung (nur bei PrimaryLoses)
///
/// **Returns:** `SlashDecision` bei erfolgreicher Erstellung.
///
/// **Fehler:** `SlashError` wenn die Parameter ungültig sind.
pub fn create_slash_decision(
    outcome: VerdictOutcome,
    primary_miner: MinerId,
    redundant_miner: MinerId,
    config: &SlashConfig,
    divergence_position: Option<usize>,
) -> Result<SlashDecision, SlashError> {
    // Validierung
    if primary_miner == redundant_miner {
        return Err(SlashError::IdenticalMiners);
    }

    if config.slash_amount == 0 || config.reward_amount == 0 {
        return Err(SlashError::InvalidParameters);
    }

    let (slashed_miner, rewarded_miner, reason) = match outcome {
        VerdictOutcome::PrimaryLoses => {
            let position = divergence_position.unwrap_or(0);
            (
                primary_miner,
                redundant_miner,
                SlashReason::PrimaryFault {
                    divergence_position: position,
                },
            )
        }
        VerdictOutcome::RedundantLoses => (
            redundant_miner,
            primary_miner,
            SlashReason::RedundantFault,
        ),
    };

    Ok(SlashDecision {
        slashed_miner,
        rewarded_miner,
        slash_amount: config.slash_amount,
        reward_amount: config.reward_amount,
        reason,
    })
}

/// Berechnet den Netto-Transfer (Slash - Reward).
///
/// **Returns:** Netto-Betrag, der vom Verlierer zum Gewinner fließt.
pub fn net_transfer(decision: &SlashDecision) -> u64 {
    decision.slash_amount.saturating_sub(decision.reward_amount)
}

/// Prüft, ob ein Miner ausreichend Stake für einen Slash hat.
///
/// **Parameter:**
/// - `miner_stake`: Aktueller Stake des Miners (in MYL-Kleinstbeträgen)
/// - `decision`: Slash-Entscheidung
///
/// **Returns:** `true` wenn der Miner ausreichend Stake hat, `false` sonst.
pub fn has_sufficient_stake(miner_stake: u64, decision: &SlashDecision) -> bool {
    miner_stake >= decision.slash_amount
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_slash_primary_loses() {
        let primary = MinerId::new([1u8; 32]);
        let redundant = MinerId::new([2u8; 32]);
        let config = SlashConfig::default();

        let decision = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            primary,
            redundant,
            &config,
            Some(5),
        )
        .unwrap();

        assert_eq!(decision.slashed_miner, primary);
        assert_eq!(decision.rewarded_miner, redundant);
        assert_eq!(decision.slash_amount, 1_000_000);
        assert_eq!(decision.reward_amount, 500_000);
        assert!(matches!(
            decision.reason,
            SlashReason::PrimaryFault {
                divergence_position: 5
            }
        ));
    }

    #[test]
    fn create_slash_redundant_loses() {
        let primary = MinerId::new([1u8; 32]);
        let redundant = MinerId::new([2u8; 32]);
        let config = SlashConfig::default();

        let decision = create_slash_decision(
            VerdictOutcome::RedundantLoses,
            primary,
            redundant,
            &config,
            None,
        )
        .unwrap();

        assert_eq!(decision.slashed_miner, redundant);
        assert_eq!(decision.rewarded_miner, primary);
        assert!(matches!(decision.reason, SlashReason::RedundantFault));
    }

    #[test]
    fn create_slash_identical_miners_error() {
        let miner = MinerId::new([1u8; 32]);
        let config = SlashConfig::default();

        let result = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            miner,
            miner,
            &config,
            Some(5),
        );

        assert!(matches!(result, Err(SlashError::IdenticalMiners)));
    }

    #[test]
    fn create_slash_invalid_config_error() {
        let primary = MinerId::new([1u8; 32]);
        let redundant = MinerId::new([2u8; 32]);
        let config = SlashConfig {
            slash_amount: 0,
            reward_amount: 500_000,
        };

        let result = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            primary,
            redundant,
            &config,
            Some(5),
        );

        assert!(matches!(result, Err(SlashError::InvalidParameters)));
    }

    #[test]
    fn net_transfer_calculation() {
        let decision = SlashDecision {
            slashed_miner: MinerId::new([1u8; 32]),
            rewarded_miner: MinerId::new([2u8; 32]),
            slash_amount: 1_000_000,
            reward_amount: 500_000,
            reason: SlashReason::PrimaryFault {
                divergence_position: 5,
            },
        };

        assert_eq!(net_transfer(&decision), 500_000);
    }

    #[test]
    fn has_sufficient_stake_true() {
        let decision = SlashDecision {
            slashed_miner: MinerId::new([1u8; 32]),
            rewarded_miner: MinerId::new([2u8; 32]),
            slash_amount: 1_000_000,
            reward_amount: 500_000,
            reason: SlashReason::PrimaryFault {
                divergence_position: 5,
            },
        };

        assert!(has_sufficient_stake(2_000_000, &decision));
        assert!(has_sufficient_stake(1_000_000, &decision));
    }

    #[test]
    fn has_sufficient_stake_false() {
        let decision = SlashDecision {
            slashed_miner: MinerId::new([1u8; 32]),
            rewarded_miner: MinerId::new([2u8; 32]),
            slash_amount: 1_000_000,
            reward_amount: 500_000,
            reason: SlashReason::PrimaryFault {
                divergence_position: 5,
            },
        };

        assert!(!has_sufficient_stake(500_000, &decision));
    }

    #[test]
    fn slash_config_default() {
        let config = SlashConfig::default();
        assert_eq!(config.slash_amount, 1_000_000);
        assert_eq!(config.reward_amount, 500_000);
    }

    #[test]
    fn verdict_outcome_equality() {
        assert_eq!(VerdictOutcome::PrimaryLoses, VerdictOutcome::PrimaryLoses);
        assert_eq!(
            VerdictOutcome::RedundantLoses,
            VerdictOutcome::RedundantLoses
        );
        assert_ne!(VerdictOutcome::PrimaryLoses, VerdictOutcome::RedundantLoses);
    }

    #[test]
    fn slash_decision_equality() {
        let decision1 = SlashDecision {
            slashed_miner: MinerId::new([1u8; 32]),
            rewarded_miner: MinerId::new([2u8; 32]),
            slash_amount: 1_000_000,
            reward_amount: 500_000,
            reason: SlashReason::PrimaryFault {
                divergence_position: 5,
            },
        };

        let decision2 = decision1.clone();
        assert_eq!(decision1, decision2);
    }
}
