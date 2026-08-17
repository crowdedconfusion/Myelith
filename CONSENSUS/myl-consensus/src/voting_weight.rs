//! Stimmgewichts-Kopplung — Whitepaper Kap. 3.5.2.
//!
//! Koppelt das Stimmgewicht an die nachgewiesene historische Inferenzarbeit:
//! `voting_weight = stake × inference_work × decay_factor`
//!
//! **Konsens-Feld:** Die Stimmgewichts-Berechnung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

/// Abklingfaktor für historische Inferenzarbeit (pro Epoche).
/// 0.95 bedeutet: Arbeit aus der vorherigen Epoche zählt zu 95%.
pub const DECAY_FACTOR_NUM: u64 = 95;
pub const DECAY_FACTOR_DEN: u64 = 100;

/// Maximale Anzahl vergangener Epochen, die berücksichtigt werden.
pub const MAX_HISTORY_EPOCHS: usize = 10;

/// Historische Inferenzarbeit über mehrere Epochen.
#[derive(Debug, Clone, Default)]
pub struct InferenceHistory {
    /// Arbeit pro Epoche (Epoche → vTFE-Einheiten).
    pub work_per_epoch: Vec<(u64, u64)>,
}

impl InferenceHistory {
    /// Erstellt eine neue, leere Historie.
    pub fn new() -> Self {
        Self {
            work_per_epoch: Vec::new(),
        }
    }

    /// Fügt Arbeit für eine Epoche hinzu.
    pub fn add_work(&mut self, epoch: u64, work: u64) {
        // Füge neuen Eintrag hinzu (oder aktualisiere bestehenden)
        if let Some(entry) = self.work_per_epoch.iter_mut().find(|(e, _)| *e == epoch) {
            entry.1 += work;
        } else {
            self.work_per_epoch.push((epoch, work));
        }

        // Sortiere nach Epoche (absteigend)
        self.work_per_epoch.sort_by(|a, b| b.0.cmp(&a.0));

        // Behalte nur die letzten MAX_HISTORY_EPOCHS Einträge
        self.work_per_epoch.truncate(MAX_HISTORY_EPOCHS);
    }

    /// Berechnet das abgeklungene Gesamtgewicht.
    ///
    /// **Algorithmus:**
    /// Für jede Epoche in der Historie:
    ///   weight += work[epoch] × decay^(current_epoch - epoch)
    ///
    /// **Parameter:**
    /// - `current_epoch`: Aktuelle Epoche
    ///
    /// **Returns:** Abgeklingtes Gesamtgewicht (u64).
    pub fn decayed_weight(&self, current_epoch: u64) -> u64 {
        let mut total = 0u64;

        for (epoch, work) in &self.work_per_epoch {
            let age = current_epoch.saturating_sub(*epoch);
            if age > MAX_HISTORY_EPOCHS as u64 {
                continue;
            }

            // Berechne decay^age mit Ganzzahl-Arithmetik
            // decay = 95/100 = 0.95
            // decay^age = (95/100)^age
            let decayed = apply_decay(*work, age);
            total += decayed;
        }

        total
    }
}

/// Wendet den Abklingfaktor auf einen Wert an.
///
/// **Parameter:**
/// - `value`: Ursprünglicher Wert
/// - `epochs`: Anzahl der Epochen zum Abklingen
///
/// **Returns:** Abgeklingter Wert.
fn apply_decay(value: u64, epochs: u64) -> u64 {
    let mut result = value;

    for _ in 0..epochs {
        // result = result * 95 / 100
        result = (result * DECAY_FACTOR_NUM) / DECAY_FACTOR_DEN;
    }

    result
}

/// Berechnet das Stimmgewicht eines Validators.
///
/// **Formel:** `voting_weight = (stake × decayed_work) / 10^12`
///
/// **Parameter:**
/// - `stake`: Stake des Validators (in MYL-Kleinstbeträgen)
/// - `history`: Historische Inferenzarbeit
/// - `current_epoch`: Aktuelle Epoche
///
/// **Returns:** Stimmgewicht (u64).
pub fn calculate_voting_weight(
    stake: u64,
    history: &InferenceHistory,
    current_epoch: u64,
) -> u64 {
    let decayed_work = history.decayed_weight(current_epoch);

    // voting_weight = (stake × decayed_work) / 10^12
    // stake ist in MYL-Kleinstbeträgen (1 MYL = 10^6)
    // decayed_work ist in vTFE-Einheiten (1 vTFE = 10^6)
    // Ergebnis ist in MYL × vTFE
    // Verwende u128 für Zwischenrechnung, um Überlauf zu vermeiden
    let weight = (stake as u128 * decayed_work as u128) / 1_000_000_000_000;

    weight as u64
}

/// Vergleicht zwei Validatoren nach Stimmgewicht (für Komiteewahl).
///
/// **Returns:** `true` wenn Validator A ein höheres Stimmgewicht hat als B.
pub fn compare_voting_weight(
    stake_a: u64,
    history_a: &InferenceHistory,
    stake_b: u64,
    history_b: &InferenceHistory,
    current_epoch: u64,
) -> bool {
    let weight_a = calculate_voting_weight(stake_a, history_a, current_epoch);
    let weight_b = calculate_voting_weight(stake_b, history_b, current_epoch);

    weight_a > weight_b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inference_history_add_work() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);

        assert_eq!(history.work_per_epoch.len(), 2);
        // Sortiert absteigend nach Epoche
        assert_eq!(history.work_per_epoch[0], (11, 2000));
        assert_eq!(history.work_per_epoch[1], (10, 1000));
    }

    #[test]
    fn inference_history_update_existing() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(10, 500); // Aktualisiere Epoche 10

        assert_eq!(history.work_per_epoch.len(), 1);
        assert_eq!(history.work_per_epoch[0], (10, 1500));
    }

    #[test]
    fn inference_history_old_entries_removed() {
        let mut history = InferenceHistory::new();

        // Füge Arbeit über MAX_HISTORY_EPOCHS hinzu
        for i in 0..15 {
            history.add_work(i, 1000);
        }

        // Nur die letzten MAX_HISTORY_EPOCHS sollten übrig sein
        assert!(history.work_per_epoch.len() <= MAX_HISTORY_EPOCHS);
    }

    #[test]
    fn decayed_weight_no_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Keine Abklingung (current_epoch = 10)
        let weight = history.decayed_weight(10);
        assert_eq!(weight, 1000);
    }

    #[test]
    fn decayed_weight_one_epoch_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Eine Epoche Abklingung: 1000 * 0.95 = 950
        let weight = history.decayed_weight(11);
        assert_eq!(weight, 950);
    }

    #[test]
    fn decayed_weight_multiple_epochs_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);

        // Zwei Epochen Abklingung: 1000 * 0.95^2 = 902
        let weight = history.decayed_weight(12);
        assert_eq!(weight, 902);
    }

    #[test]
    fn decayed_weight_multiple_epochs() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1000);
        history.add_work(11, 2000);

        // Epoche 10: 1000 * 0.95^2 = 902
        // Epoche 11: 2000 * 0.95^1 = 1900
        // Total: 902 + 1900 = 2802
        let weight = history.decayed_weight(12);
        assert_eq!(weight, 2802);
    }

    #[test]
    fn calculate_voting_weight_basic() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1_000_000); // 1 vTFE

        let stake = 10_000_000; // 10 MYL
        let weight = calculate_voting_weight(stake, &history, 10);

        // weight = 10 * 1 = 10 (normalisiert)
        assert_eq!(weight, 10);
    }

    #[test]
    fn calculate_voting_weight_with_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, 1_000_000); // 1 vTFE

        let stake = 10_000_000; // 10 MYL
        let weight = calculate_voting_weight(stake, &history, 11);

        // weight = 10 * 0.95 = 9 (normalisiert)
        assert_eq!(weight, 9);
    }

    #[test]
    fn test_compare_voting_weight_work_matters() {
        let mut history_a = InferenceHistory::new();
        history_a.add_work(10, 2_000_000); // 2 vTFE

        let mut history_b = InferenceHistory::new();
        history_b.add_work(10, 1_000_000); // 1 vTFE

        let stake_a = 10_000_000;
        let stake_b = 10_000_000;

        // A hat mehr Arbeit → höheres Gewicht
        assert!(compare_voting_weight(
            stake_a, &history_a,
            stake_b, &history_b,
            10
        ));
    }

    #[test]
    fn test_compare_voting_weight_stake_matters() {
        let mut history_a = InferenceHistory::new();
        history_a.add_work(10, 1_000_000);

        let mut history_b = InferenceHistory::new();
        history_b.add_work(10, 1_000_000);

        let stake_a = 20_000_000; // 20 MYL
        let stake_b = 10_000_000; // 10 MYL

        // A hat mehr Stake → höheres Gewicht
        assert!(compare_voting_weight(
            stake_a, &history_a,
            stake_b, &history_b,
            10
        ));
    }

    #[test]
    fn apply_decay_calculation() {
        assert_eq!(apply_decay(1000, 0), 1000);
        assert_eq!(apply_decay(1000, 1), 950);
        assert_eq!(apply_decay(1000, 2), 902);
        assert_eq!(apply_decay(1000, 3), 856);
    }

    #[test]
    fn decay_constants() {
        assert_eq!(DECAY_FACTOR_NUM, 95);
        assert_eq!(DECAY_FACTOR_DEN, 100);
        assert_eq!(MAX_HISTORY_EPOCHS, 10);
    }
}
