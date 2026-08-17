//! Validator-Registrierung und Komiteewahl — Whitepaper Kap. 3.5, Anhang A.2.
//!
//! Miner registrieren sich als Validatoren mit Stake. Das Komitee wird
//! jede Epoche stake-basiert gewählt: 21 Blockproduktions-Validatoren +
//! 7 Schiedsrichter. VRF-Rotation sorgt für faire Auswahl.
//!
//! **Konsens-Feld:** Die Komiteewahl ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::ids::MinerId;
use std::collections::BTreeMap;

/// Anzahl der Blockproduktions-Validatoren im Komitee.
pub const COMMITTEE_SIZE: usize = 21;

/// Anzahl der Schiedsrichter im Komitee.
pub const ARBITER_COUNT: usize = 7;

/// Minimale Stake-Anforderung für Validator-Registrierung (in MYL-Kleinstbeträgen).
pub const MIN_STAKE: u64 = 10_000_000; // 10 MYL

/// Rolle eines Validators im Komitee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitteeRole {
    /// Blockproduktions-Validator (21 im Komitee).
    Producer,
    /// Schiedsrichter für Bisektions-Spiel (7 im Komitee).
    Arbiter,
}

/// Ein registrierter Validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validator {
    /// Miner-ID des Validators.
    pub miner_id: MinerId,
    /// Stake in MYL-Kleinstbeträgen.
    pub stake: u64,
    /// Epoche der Registrierung.
    pub registration_epoch: u64,
    /// Historische Inferenzarbeit (für Stimmgewichts-Kopplung).
    pub inference_work: u64,
}

/// Fehler bei der Validator-Operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorError {
    /// Stake unter Minimum.
    InsufficientStake { provided: u64, required: u64 },
    /// Validator bereits registriert.
    AlreadyRegistered,
    /// Validator nicht gefunden.
    NotFound,
    /// Ungültige Epoche (Registrierung zu spät für aktuelle Wahl).
    InvalidEpoch,
}

impl std::fmt::Display for ValidatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientStake { provided, required } => {
                write!(
                    f,
                    "Unzureichender Stake: {} < {} (Minimum)",
                    provided, required
                )
            }
            Self::AlreadyRegistered => write!(f, "Validator bereits registriert"),
            Self::NotFound => write!(f, "Validator nicht gefunden"),
            Self::InvalidEpoch => write!(f, "Ungültige Epoche für Registrierung"),
        }
    }
}

impl std::error::Error for ValidatorError {}

/// Registry für registrierte Validatoren.
#[derive(Debug, Clone, Default)]
pub struct ValidatorRegistry {
    /// Registrierte Validatoren (sortiert nach MinerId für Determinismus).
    validators: BTreeMap<MinerId, Validator>,
}

impl ValidatorRegistry {
    /// Erstellt eine neue, leere Registry.
    pub fn new() -> Self {
        Self {
            validators: BTreeMap::new(),
        }
    }

    /// Registriert einen neuen Validator.
    ///
    /// **Parameter:**
    /// - `miner_id`: Miner-ID des Validators
    /// - `stake`: Stake in MYL-Kleinstbeträgen
    /// - `current_epoch`: Aktuelle Epoche
    ///
    /// **Fehler:** `ValidatorError` wenn Stake unter Minimum oder bereits registriert.
    pub fn register(
        &mut self,
        miner_id: MinerId,
        stake: u64,
        current_epoch: u64,
    ) -> Result<(), ValidatorError> {
        if stake < MIN_STAKE {
            return Err(ValidatorError::InsufficientStake {
                provided: stake,
                required: MIN_STAKE,
            });
        }

        if self.validators.contains_key(&miner_id) {
            return Err(ValidatorError::AlreadyRegistered);
        }

        let validator = Validator {
            miner_id,
            stake,
            registration_epoch: current_epoch,
            inference_work: 0,
        };

        self.validators.insert(miner_id, validator);
        Ok(())
    }

    /// Aktualisiert die Inferenzarbeit eines Validators.
    pub fn update_inference_work(
        &mut self,
        miner_id: &MinerId,
        additional_work: u64,
    ) -> Result<(), ValidatorError> {
        let validator = self
            .validators
            .get_mut(miner_id)
            .ok_or(ValidatorError::NotFound)?;
        validator.inference_work += additional_work;
        Ok(())
    }

    /// Gibt alle registrierten Validatoren zurück (sortiert nach MinerId).
    pub fn all_validators(&self) -> Vec<&Validator> {
        self.validators.values().collect()
    }

    /// Gibt die Anzahl registrierter Validatoren zurück.
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }

    /// Gibt einen Validator zurück.
    pub fn get_validator(&self, miner_id: &MinerId) -> Option<&Validator> {
        self.validators.get(miner_id)
    }
}

/// Ein Komitee für eine Epoche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committee {
    /// Epoche, für die das Komitee gewählt wurde.
    pub epoch: u64,
    /// Blockproduktions-Validatoren (21).
    pub producers: Vec<MinerId>,
    /// Schiedsrichter (7).
    pub arbiters: Vec<MinerId>,
}

/// Wählt ein Komitee für eine Epoche stake-basiert aus.
///
/// **Algorithmus:**
/// 1. Filtere Validatoren, die sich vor Epoche e-2 registriert haben
/// 2. Sortiere nach Stake (absteigend), bei Gleichstand nach MinerId
/// 3. Wähle erste 21 als Producer, nächste 7 als Arbiter
///
/// **Parameter:**
/// - `registry`: Validator-Registry
/// - `epoch`: Ziel-Epoche
///
/// **Returns:** `Committee` mit 21 Producern und 7 Arbitern.
///
/// **Fehler:** `ValidatorError` wenn nicht genug Validatoren registriert sind.
pub fn select_committee(
    registry: &ValidatorRegistry,
    epoch: u64,
) -> Result<Committee, ValidatorError> {
    // Registrierungsschluss: Epoche e-2
    let registration_deadline = epoch.saturating_sub(2);

    // Filtere Validatoren, die sich vor dem Deadline registriert haben
    let mut eligible: Vec<&Validator> = registry
        .all_validators()
        .into_iter()
        .filter(|v| v.registration_epoch <= registration_deadline)
        .collect();

    // Sortiere nach Stake (absteigend), bei Gleichstand nach MinerId (aufsteigend)
    eligible.sort_by(|a, b| {
        b.stake
            .cmp(&a.stake)
            .then_with(|| a.miner_id.cmp(&b.miner_id))
    });

    let required = COMMITTEE_SIZE + ARBITER_COUNT;
    if eligible.len() < required {
        return Err(ValidatorError::InvalidEpoch);
    }

    let producers: Vec<MinerId> = eligible[..COMMITTEE_SIZE]
        .iter()
        .map(|v| v.miner_id)
        .collect();

    let arbiters: Vec<MinerId> = eligible[COMMITTEE_SIZE..required]
        .iter()
        .map(|v| v.miner_id)
        .collect();

    Ok(Committee {
        epoch,
        producers,
        arbiters,
    })
}

/// Prüft, ob ein Miner im Komitee ist.
pub fn is_in_committee(committee: &Committee, miner_id: &MinerId) -> Option<CommitteeRole> {
    if committee.producers.contains(miner_id) {
        Some(CommitteeRole::Producer)
    } else if committee.arbiters.contains(miner_id) {
        Some(CommitteeRole::Arbiter)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    #[test]
    fn register_validator_success() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);

        let result = registry.register(miner, MIN_STAKE, 10);
        assert!(result.is_ok());
        assert_eq!(registry.validator_count(), 1);
    }

    #[test]
    fn register_validator_insufficient_stake() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);

        let result = registry.register(miner, MIN_STAKE - 1, 10);
        assert!(matches!(
            result,
            Err(ValidatorError::InsufficientStake {
                provided: _,
                required: MIN_STAKE
            })
        ));
    }

    #[test]
    fn register_validator_already_registered() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);

        registry.register(miner, MIN_STAKE, 10).unwrap();
        let result = registry.register(miner, MIN_STAKE, 10);
        assert!(matches!(result, Err(ValidatorError::AlreadyRegistered)));
    }

    #[test]
    fn update_inference_work() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);

        registry.register(miner, MIN_STAKE, 10).unwrap();
        registry.update_inference_work(&miner, 1000).unwrap();

        let validator = registry.get_validator(&miner).unwrap();
        assert_eq!(validator.inference_work, 1000);
    }

    #[test]
    fn select_committee_success() {
        let mut registry = ValidatorRegistry::new();

        // Registriere 28 Validatoren (21 Producer + 7 Arbiter)
        for i in 0..28 {
            let miner = test_miner(i);
            let stake = MIN_STAKE + (i as u64 * 1_000_000); // Unterschiedliche Stakes
            registry.register(miner, stake, 5).unwrap();
        }

        let committee = select_committee(&registry, 10).unwrap();
        assert_eq!(committee.epoch, 10);
        assert_eq!(committee.producers.len(), COMMITTEE_SIZE);
        assert_eq!(committee.arbiters.len(), ARBITER_COUNT);
    }

    #[test]
    fn select_committee_not_enough_validators() {
        let mut registry = ValidatorRegistry::new();

        // Registriere nur 20 Validatoren (zu wenige)
        for i in 0..20 {
            let miner = test_miner(i);
            registry.register(miner, MIN_STAKE, 5).unwrap();
        }

        let result = select_committee(&registry, 10);
        assert!(matches!(result, Err(ValidatorError::InvalidEpoch)));
    }

    #[test]
    fn select_committee_registration_deadline() {
        let mut registry = ValidatorRegistry::new();

        // Registriere Validatoren mit unterschiedlichen Registrierungs-Epochen
        for i in 0..28 {
            let miner = test_miner(i);
            let reg_epoch = if i < 14 { 5 } else { 9 }; // 14 vor Deadline, 14 nach
            registry.register(miner, MIN_STAKE, reg_epoch).unwrap();
        }

        // Epoche 10, Deadline = 8
        // Nur die 14 Validatoren, die sich in Epoche 5 registriert haben, sind wählbar
        // Das sollte fehlschlagen, da nur 14 < 28 benötigt
        let result = select_committee(&registry, 10);
        assert!(matches!(result, Err(ValidatorError::InvalidEpoch)));
    }

    #[test]
    fn select_committee_stake_ordering() {
        let mut registry = ValidatorRegistry::new();

        // Registriere 28 Validatoren mit unterschiedlichen Stakes
        for i in 0..28 {
            let miner = test_miner(i);
            let stake = MIN_STAKE + ((27 - i) as u64 * 1_000_000); // Höchster Stake bei i=0
            registry.register(miner, stake, 5).unwrap();
        }

        let committee = select_committee(&registry, 10).unwrap();

        // Producer sollten die Validatoren mit den höchsten Stakes sein
        assert_eq!(committee.producers[0], test_miner(0)); // Höchster Stake
        assert_eq!(committee.producers[20], test_miner(20)); // 21. höchster
    }

    #[test]
    fn is_in_committee_producer() {
        let committee = Committee {
            epoch: 10,
            producers: vec![test_miner(1), test_miner(2)],
            arbiters: vec![test_miner(3)],
        };

        assert_eq!(
            is_in_committee(&committee, &test_miner(1)),
            Some(CommitteeRole::Producer)
        );
    }

    #[test]
    fn is_in_committee_arbiter() {
        let committee = Committee {
            epoch: 10,
            producers: vec![test_miner(1)],
            arbiters: vec![test_miner(3)],
        };

        assert_eq!(
            is_in_committee(&committee, &test_miner(3)),
            Some(CommitteeRole::Arbiter)
        );
    }

    #[test]
    fn is_in_committee_not_in_committee() {
        let committee = Committee {
            epoch: 10,
            producers: vec![test_miner(1)],
            arbiters: vec![test_miner(3)],
        };

        assert_eq!(is_in_committee(&committee, &test_miner(99)), None);
    }

    #[test]
    fn committee_constants() {
        assert_eq!(COMMITTEE_SIZE, 21);
        assert_eq!(ARBITER_COUNT, 7);
        assert_eq!(MIN_STAKE, 10_000_000);
    }
}
