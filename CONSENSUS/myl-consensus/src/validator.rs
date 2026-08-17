//! Validator-Registrierung und Komiteewahl — Whitepaper Kap. 3.5, Anhang A.2.
//!
//! Miner registrieren sich als Validatoren mit Stake und öffentlichem
//! BLS-Schlüssel. Das Komitee wird jede Epoche neu gewählt: 21
//! Blockproduktions-Validatoren + 7 Schiedsrichter.
//!
//! **Auswahlverfahren (Whitepaper Kap. 3.5):** „gewählt nach Stake,
//! rotierend per VRF". Umgesetzt als gewichtete Ziehung ohne
//! Zurücklegen — das Gewicht ist das Stimmgewicht aus
//! [`crate::voting_weight`] (Stake **und** nachgewiesene Inferenzarbeit
//! mit Abklingfaktor), die Ziehung wird vom VRF-Epochenseed gesteuert.
//!
//! Bis v0.3.6 wählte diese Datei stattdessen deterministisch die 28
//! Validatoren mit dem höchsten Stake — ohne VRF-Rotation und ohne
//! Arbeitshistorie, obwohl der Fahrplan-Punkt 3.1 „VRF-Rotation" und
//! Punkt 3.4 „Stimmgewichts-Kopplung" als erledigt führte. Das war eine
//! feste Rangliste: dieselben 21 Adressen in jeder Epoche, und die
//! Kernaussage des Whitepapers („nützliche Arbeit sichert den Konsens")
//! war im Code nicht abgebildet.
//!
//! **Konsens-Feld:** Die Komiteewahl ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::voting_weight::{calculate_voting_weight, InferenceHistory};
use myl_types::bls::BlsPublicKey;
use myl_types::ids::MinerId;
use myl_types::seed_rng::weighted_sample_without_replacement;
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
    /// Öffentlicher BLS-Schlüssel — gegen ihn werden alle
    /// Konsens-Nachrichten dieses Validators geprüft.
    pub pubkey: BlsPublicKey,
    /// Stake in MYL-Kleinstbeträgen.
    pub stake: u64,
    /// Epoche der Registrierung.
    pub registration_epoch: u64,
    /// Historische Inferenzarbeit je Epoche (für die Stimmgewichts-Kopplung).
    pub history: InferenceHistory,
}

impl Validator {
    /// Stimmgewicht dieses Validators in der angegebenen Epoche.
    pub fn voting_weight(&self, current_epoch: u64) -> u64 {
        calculate_voting_weight(self.stake, &self.history, current_epoch)
    }
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
    /// Nicht genug wählbare Validatoren für ein vollständiges Komitee.
    NotEnoughValidators { eligible: usize, required: usize },
    /// Der öffentliche BLS-Schlüssel ist kein gültiger Gruppenpunkt.
    InvalidPublicKey,
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
            Self::NotEnoughValidators { eligible, required } => write!(
                f,
                "Nicht genug wählbare Validatoren: {} < {}",
                eligible, required
            ),
            Self::InvalidPublicKey => write!(f, "Ungültiger öffentlicher BLS-Schlüssel"),
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
    /// Der öffentliche Schlüssel wird sofort validiert (Identitäts- und
    /// Untergruppenprüfung). Ein ungültiger Schlüssel darf gar nicht
    /// erst in die Registry gelangen — sonst schlüge später jede
    /// Signaturprüfung fehl, ohne dass die Ursache erkennbar wäre.
    ///
    /// **Parameter:**
    /// - `miner_id`: Miner-ID des Validators
    /// - `pubkey`: öffentlicher BLS-Schlüssel
    /// - `stake`: Stake in MYL-Kleinstbeträgen
    /// - `current_epoch`: Aktuelle Epoche
    ///
    /// **Fehler:** `ValidatorError` wenn Stake unter Minimum, Schlüssel
    /// ungültig oder bereits registriert.
    pub fn register(
        &mut self,
        miner_id: MinerId,
        pubkey: BlsPublicKey,
        stake: u64,
        current_epoch: u64,
    ) -> Result<(), ValidatorError> {
        if stake < MIN_STAKE {
            return Err(ValidatorError::InsufficientStake {
                provided: stake,
                required: MIN_STAKE,
            });
        }

        if pubkey.validate().is_err() {
            return Err(ValidatorError::InvalidPublicKey);
        }

        if self.validators.contains_key(&miner_id) {
            return Err(ValidatorError::AlreadyRegistered);
        }

        let validator = Validator {
            miner_id,
            pubkey,
            stake,
            registration_epoch: current_epoch,
            history: InferenceHistory::new(),
        };

        self.validators.insert(miner_id, validator);
        Ok(())
    }

    /// Schreibt nachgewiesene Inferenzarbeit für eine Epoche gut.
    ///
    /// **Parameter:**
    /// - `miner_id`: Validator
    /// - `epoch`: Epoche, in der die Arbeit erbracht wurde
    /// - `work`: Arbeit in vTFE-Kleinstbeträgen
    pub fn record_work(
        &mut self,
        miner_id: &MinerId,
        epoch: u64,
        work: u64,
    ) -> Result<(), ValidatorError> {
        let validator = self
            .validators
            .get_mut(miner_id)
            .ok_or(ValidatorError::NotFound)?;
        validator.history.add_work(epoch, work);
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

/// Wählt ein Komitee für eine Epoche stimmgewichtet und VRF-rotierend.
///
/// **Algorithmus:**
/// 1. Filtere Validatoren, die sich bis Epoche e−2 registriert haben.
/// 2. Berechne für jeden das Stimmgewicht (Stake + Arbeitsanteil).
/// 3. Ziehe 28 Validatoren gewichtet ohne Zurücklegen, gesteuert vom
///    VRF-Epochenseed.
/// 4. Die ersten 21 Ziehungen sind Producer, die nächsten 7 Arbiter.
///
/// Die Kandidatenliste ist nach `MinerId` sortiert (BTreeMap-Ordnung),
/// die Ziehung ist deterministisch — jeder Node kommt mit demselben Seed
/// zum selben Komitee.
///
/// **Parameter:**
/// - `registry`: Validator-Registry
/// - `epoch`: Ziel-Epoche
/// - `seed`: VRF-Epochenseed (aus dem finalisierten Block, Anhang A.2)
///
/// **Returns:** `Committee` mit 21 Producern und 7 Arbitern.
///
/// **Fehler:** `NotEnoughValidators` wenn nicht genug wählbare
/// Validatoren mit Stimmgewicht > 0 registriert sind.
pub fn select_committee(
    registry: &ValidatorRegistry,
    epoch: u64,
    seed: &[u8; 32],
) -> Result<Committee, ValidatorError> {
    // Registrierungsschluss: Epoche e-2
    let registration_deadline = epoch.saturating_sub(2);

    let eligible: Vec<&Validator> = registry
        .all_validators()
        .into_iter()
        .filter(|v| v.registration_epoch <= registration_deadline)
        .collect();

    let required = COMMITTEE_SIZE + ARBITER_COUNT;
    if eligible.len() < required {
        return Err(ValidatorError::NotEnoughValidators {
            eligible: eligible.len(),
            required,
        });
    }

    let weights: Vec<u64> = eligible.iter().map(|v| v.voting_weight(epoch)).collect();

    // Der Seed wird an die Epoche gebunden, damit dieselbe
    // Kandidatenmenge in aufeinanderfolgenden Epochen rotiert, auch
    // wenn der Epochenseed einmal wiederkehrt.
    let mut epoch_seed = [0u8; 32];
    {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"MYELITH_COMMITTEE_v1");
        hasher.update(seed);
        hasher.update(epoch.to_le_bytes());
        epoch_seed.copy_from_slice(&hasher.finalize());
    }

    let picked = weighted_sample_without_replacement(&weights, required, &epoch_seed);
    if picked.len() < required {
        // Kann nur eintreten, wenn zu viele Kandidaten Gewicht 0 haben.
        return Err(ValidatorError::NotEnoughValidators {
            eligible: picked.len(),
            required,
        });
    }

    let producers: Vec<MinerId> = picked[..COMMITTEE_SIZE]
        .iter()
        .map(|&i| eligible[i].miner_id)
        .collect();

    let arbiters: Vec<MinerId> = picked[COMMITTEE_SIZE..required]
        .iter()
        .map(|&i| eligible[i].miner_id)
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

/// Ein stimmberechtigtes Mitglied einer BFT-Runde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VotingMember {
    /// Öffentlicher Schlüssel für die Signaturprüfung.
    pub pubkey: BlsPublicKey,
    /// Stimmgewicht in dieser Epoche.
    pub weight: u64,
}

/// Die stimmberechtigte Menge einer BFT-Runde.
///
/// Bündelt, was das BFT-Protokoll zur Prüfung einer Nachricht braucht:
/// **wer** darf stimmen, mit **welchem Schlüssel** wird geprüft und mit
/// **welchem Gewicht** zählt die Stimme. Vor v0.4.0 hatte `BftState`
/// nichts davon — es zählte Nachrichten unabhängig von Absender und
/// Gewicht, sodass ein einzelner Angreifer mit erfundenen Miner-IDs den
/// Threshold erreichen konnte.
#[derive(Debug, Clone)]
pub struct VotingSet {
    members: BTreeMap<MinerId, VotingMember>,
    total_weight: u64,
}

impl VotingSet {
    /// Baut die stimmberechtigte Menge aus den Producern eines Komitees.
    ///
    /// **Fehler:** `NotFound`, wenn ein Komitee-Mitglied nicht (mehr) in
    /// der Registry steht.
    pub fn for_producers(
        registry: &ValidatorRegistry,
        committee: &Committee,
        epoch: u64,
    ) -> Result<Self, ValidatorError> {
        let mut members = BTreeMap::new();
        let mut total_weight: u128 = 0;

        for miner_id in &committee.producers {
            let validator = registry
                .get_validator(miner_id)
                .ok_or(ValidatorError::NotFound)?;
            let weight = validator.voting_weight(epoch);
            total_weight += weight as u128;
            members.insert(
                *miner_id,
                VotingMember {
                    pubkey: validator.pubkey,
                    weight,
                },
            );
        }

        Ok(Self {
            members,
            total_weight: u64::try_from(total_weight).unwrap_or(u64::MAX),
        })
    }

    /// Baut die Menge direkt aus Mitgliedern (für Tests und für Aufrufer,
    /// die ihre Gewichte anderweitig beziehen).
    pub fn from_members(members: BTreeMap<MinerId, VotingMember>) -> Self {
        let total_weight = members
            .values()
            .fold(0u128, |acc, m| acc + m.weight as u128);
        Self {
            members,
            total_weight: u64::try_from(total_weight).unwrap_or(u64::MAX),
        }
    }

    /// Ist dieser Miner stimmberechtigt?
    pub fn contains(&self, miner_id: &MinerId) -> bool {
        self.members.contains_key(miner_id)
    }

    /// Öffentlicher Schlüssel eines Mitglieds.
    pub fn pubkey(&self, miner_id: &MinerId) -> Option<&BlsPublicKey> {
        self.members.get(miner_id).map(|m| &m.pubkey)
    }

    /// Stimmgewicht eines Mitglieds (0 für Nichtmitglieder).
    pub fn weight(&self, miner_id: &MinerId) -> u64 {
        self.members.get(miner_id).map_or(0, |m| m.weight)
    }

    /// Summe aller Stimmgewichte.
    pub fn total_weight(&self) -> u64 {
        self.total_weight
    }

    /// Anzahl der Mitglieder.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Ist die Menge leer?
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Quorum-Schwelle: mehr als zwei Drittel des Gesamtgewichts.
    ///
    /// BFT-Safety verlangt, dass sich zwei Quoren in mindestens einem
    /// ehrlichen Stimmgewicht überschneiden. Bei einem byzantinischen
    /// Anteil f < 1/3 leistet das `> 2/3`. Zurückgegeben wird der
    /// kleinste erreichende Wert, also `floor(2·total/3) + 1`.
    pub fn quorum_threshold(&self) -> u64 {
        let t = (self.total_weight as u128 * 2) / 3 + 1;
        u64::try_from(t).unwrap_or(u64::MAX)
    }

    /// Mitglieder in kanonischer Reihenfolge (nach MinerId).
    pub fn members(&self) -> impl Iterator<Item = (&MinerId, &VotingMember)> {
        self.members.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSecretKey;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_pubkey(byte: u8) -> BlsPublicKey {
        BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32])
            .expect("key_gen")
            .public_key()
            .expect("public_key")
    }

    fn seed(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    /// Registriert `n` Validatoren mit gleichem Stake in Epoche 5.
    fn registry_with(n: u8) -> ValidatorRegistry {
        let mut registry = ValidatorRegistry::new();
        for i in 0..n {
            registry
                .register(test_miner(i), test_pubkey(i), MIN_STAKE, 5)
                .unwrap();
        }
        registry
    }

    #[test]
    fn register_validator_success() {
        let mut registry = ValidatorRegistry::new();
        let result = registry.register(test_miner(1), test_pubkey(1), MIN_STAKE, 10);
        assert!(result.is_ok());
        assert_eq!(registry.validator_count(), 1);
    }

    #[test]
    fn register_validator_insufficient_stake() {
        let mut registry = ValidatorRegistry::new();
        let result = registry.register(test_miner(1), test_pubkey(1), MIN_STAKE - 1, 10);
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
        registry
            .register(test_miner(1), test_pubkey(1), MIN_STAKE, 10)
            .unwrap();
        let result = registry.register(test_miner(1), test_pubkey(1), MIN_STAKE, 10);
        assert!(matches!(result, Err(ValidatorError::AlreadyRegistered)));
    }

    /// Ein ungültiger Schlüssel darf nicht in die Registry — sonst
    /// scheiterte später jede Signaturprüfung ohne erkennbare Ursache.
    #[test]
    fn register_lehnt_ungueltigen_schluessel_ab() {
        let mut registry = ValidatorRegistry::new();
        let result = registry.register(
            test_miner(1),
            BlsPublicKey([0u8; 48]),
            MIN_STAKE,
            10,
        );
        assert!(matches!(result, Err(ValidatorError::InvalidPublicKey)));
    }

    #[test]
    fn record_work_fliesst_ins_stimmgewicht() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);
        registry
            .register(miner, test_pubkey(1), MIN_STAKE, 10)
            .unwrap();

        let ohne_arbeit = registry.get_validator(&miner).unwrap().voting_weight(10);
        registry.record_work(&miner, 10, 1_000_000).unwrap();
        let mit_arbeit = registry.get_validator(&miner).unwrap().voting_weight(10);

        assert!(
            mit_arbeit > ohne_arbeit,
            "nachgewiesene Arbeit muss das Stimmgewicht erhöhen"
        );
    }

    #[test]
    fn record_work_unbekannter_validator() {
        let mut registry = ValidatorRegistry::new();
        assert!(matches!(
            registry.record_work(&test_miner(9), 1, 100),
            Err(ValidatorError::NotFound)
        ));
    }

    /// Der Bootstrap-Fall: ohne Arbeitshistorie muss ein Validator ein
    /// Gewicht > 0 haben, sonst ist bei Genesis kein Komitee wählbar.
    #[test]
    fn frischer_validator_hat_gewicht_groesser_null() {
        let mut registry = ValidatorRegistry::new();
        let miner = test_miner(1);
        registry
            .register(miner, test_pubkey(1), MIN_STAKE, 0)
            .unwrap();
        assert!(registry.get_validator(&miner).unwrap().voting_weight(0) > 0);
    }

    #[test]
    fn select_committee_success() {
        let registry = registry_with(28);
        let committee = select_committee(&registry, 10, &seed(1)).unwrap();
        assert_eq!(committee.epoch, 10);
        assert_eq!(committee.producers.len(), COMMITTEE_SIZE);
        assert_eq!(committee.arbiters.len(), ARBITER_COUNT);
    }

    #[test]
    fn select_committee_ist_deterministisch() {
        let registry = registry_with(40);
        let a = select_committee(&registry, 10, &seed(1)).unwrap();
        let b = select_committee(&registry, 10, &seed(1)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn select_committee_hat_keine_doppelten_mitglieder() {
        let registry = registry_with(40);
        let c = select_committee(&registry, 10, &seed(1)).unwrap();
        let mut alle: Vec<MinerId> = c.producers.clone();
        alle.extend(c.arbiters.clone());
        let vorher = alle.len();
        alle.sort_unstable();
        alle.dedup();
        assert_eq!(alle.len(), vorher, "kein Miner darf doppelt im Komitee sein");
    }

    /// Der Kern von Fund A7: Bei gleicher Kandidatenmenge muss das
    /// Komitee zwischen Epochen rotieren. Vorher waren es immer
    /// dieselben 21 Adressen mit dem höchsten Stake.
    #[test]
    fn komitee_rotiert_ueber_epochen() {
        let registry = registry_with(60);
        let a = select_committee(&registry, 10, &seed(1)).unwrap();
        let b = select_committee(&registry, 11, &seed(1)).unwrap();
        assert_ne!(
            a.producers, b.producers,
            "gleiche Kandidaten dürfen nicht dauerhaft dasselbe Komitee ergeben"
        );
    }

    #[test]
    fn komitee_rotiert_mit_dem_vrf_seed() {
        let registry = registry_with(60);
        let a = select_committee(&registry, 10, &seed(1)).unwrap();
        let b = select_committee(&registry, 10, &seed(2)).unwrap();
        assert_ne!(a.producers, b.producers);
    }

    /// Höheres Stimmgewicht muss die Auswahlwahrscheinlichkeit erhöhen —
    /// sonst wäre die Kopplung an Stake und Arbeit wirkungslos.
    #[test]
    fn hoeheres_gewicht_wird_haeufiger_gewaehlt() {
        let mut registry = ValidatorRegistry::new();
        // Miner 0 hat 20x den Stake der uebrigen 39.
        registry
            .register(test_miner(0), test_pubkey(0), MIN_STAKE * 20, 5)
            .unwrap();
        for i in 1..40 {
            registry
                .register(test_miner(i), test_pubkey(i), MIN_STAKE, 5)
                .unwrap();
        }

        let mut schwer = 0;
        let mut leicht = 0;
        for e in 10..110u64 {
            let c = select_committee(&registry, e, &seed(1)).unwrap();
            if c.producers.contains(&test_miner(0)) {
                schwer += 1;
            }
            if c.producers.contains(&test_miner(1)) {
                leicht += 1;
            }
        }
        assert!(
            schwer > leicht,
            "schwerer Validator {} vs. leichter {} — Gewichtung wirkt nicht",
            schwer,
            leicht
        );
    }

    #[test]
    fn select_committee_not_enough_validators() {
        let registry = registry_with(20);
        assert!(matches!(
            select_committee(&registry, 10, &seed(1)),
            Err(ValidatorError::NotEnoughValidators { .. })
        ));
    }

    #[test]
    fn select_committee_registration_deadline() {
        let mut registry = ValidatorRegistry::new();
        for i in 0..28 {
            let reg_epoch = if i < 14 { 5 } else { 9 };
            registry
                .register(test_miner(i), test_pubkey(i), MIN_STAKE, reg_epoch)
                .unwrap();
        }
        // Epoche 10, Deadline 8 → nur 14 waehlbar.
        assert!(matches!(
            select_committee(&registry, 10, &seed(1)),
            Err(ValidatorError::NotEnoughValidators { eligible: 14, .. })
        ));
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

    // ── VotingSet ───────────────────────────────────────────────────

    #[test]
    fn voting_set_aus_komitee() {
        let registry = registry_with(28);
        let committee = select_committee(&registry, 10, &seed(1)).unwrap();
        let set = VotingSet::for_producers(&registry, &committee, 10).unwrap();

        assert_eq!(set.len(), COMMITTEE_SIZE);
        assert!(set.total_weight() > 0);
        for id in &committee.producers {
            assert!(set.contains(id));
            assert!(set.pubkey(id).is_some());
            assert!(set.weight(id) > 0);
        }
        assert!(!set.contains(&test_miner(200)));
        assert_eq!(set.weight(&test_miner(200)), 0);
    }

    #[test]
    fn voting_set_meldet_fehlendes_mitglied() {
        let registry = registry_with(28);
        let committee = Committee {
            epoch: 10,
            producers: vec![test_miner(250)],
            arbiters: vec![],
        };
        assert!(matches!(
            VotingSet::for_producers(&registry, &committee, 10),
            Err(ValidatorError::NotFound)
        ));
    }

    #[test]
    fn quorum_ist_mehr_als_zwei_drittel() {
        let mut members = BTreeMap::new();
        for i in 0..3u8 {
            members.insert(
                test_miner(i),
                VotingMember {
                    pubkey: test_pubkey(i),
                    weight: 100,
                },
            );
        }
        let set = VotingSet::from_members(members);
        assert_eq!(set.total_weight(), 300);
        // 2/3 von 300 = 200; die Schwelle muss echt darueber liegen.
        assert_eq!(set.quorum_threshold(), 201);
    }

    #[test]
    fn quorum_bei_leerer_menge() {
        let set = VotingSet::from_members(BTreeMap::new());
        assert!(set.is_empty());
        assert_eq!(set.total_weight(), 0);
        // Eine leere Menge darf kein erreichbares Quorum melden.
        assert_eq!(set.quorum_threshold(), 1);
    }
}
