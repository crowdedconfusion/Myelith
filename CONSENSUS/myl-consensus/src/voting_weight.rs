//! Stimmgewichts-Kopplung — Whitepaper Kap. 3.5.2.
//!
//! Koppelt das Stimmgewicht an gestakten Coin **und** nachgewiesene
//! historische Inferenzarbeit (mit Abklingfaktor):
//!
//! ```text
//! voting_weight = stake + (stake · abgeklungene_Arbeit) / VTFE_UNIT
//! ```
//!
//! **Warum eine Summe und kein reines Produkt (Design-Entscheidung
//! 2026-08-18, Audit-Block 3):** Die ursprüngliche Formel war
//! `stake × Arbeit / 10¹²`. Ein reines Produkt gibt jedem Validator ohne
//! Arbeitshistorie das Gewicht **null** — und wer Gewicht null hat, wird
//! nie ins Komitee gewählt, kann nie Arbeit nachweisen und bleibt
//! dauerhaft bei null. Bei Genesis hätte *kein* Validator Gewicht, das
//! Komitee wäre nicht wählbar. Die Summenform hält die Aussage des
//! Whitepapers („speist sich aus zwei Quellen") und löst die Blockade:
//! der Stake ist die Grundlage, die Arbeit multipliziert sie hoch. Ein
//! Validator mit einer vollen vTFE-Einheit abgeklungener Arbeit hat
//! doppeltes Gewicht gegenüber einem gleich gestakten ohne Arbeit.
//!
//! **Status der Formel: vorläufig bestätigt (Projektinhaber,
//! 2026-08-18) — ZUR WIEDERVORLAGE.** Sie löst die Bootstrap-Blockade,
//! ist aber nicht als endgültige Kalibrierung gedacht. Vor dem
//! Mainnet-Start neu zu bewerten sind mindestens:
//!
//! - **Die Gewichtung zwischen den beiden Quellen.** `VTFE_UNIT` als
//!   Bezugsgröße bedeutet: eine volle abgeklungene vTFE-Einheit
//!   verdoppelt das Gewicht. Ob das den Anreiz richtig setzt, ist eine
//!   ökonomische Frage, keine technische — sie hängt an der real
//!   erreichbaren vTFE-Menge pro Epoche, die noch nicht gemessen ist.
//! - **Die Obergrenze.** Aktuell ist der Arbeitsanteil unbeschränkt;
//!   ein Miner mit sehr viel Arbeit kann sein Stimmgewicht beliebig weit
//!   über seinen Stake heben. Eine Deckelung (z. B. Faktor 10) wäre zu
//!   erwägen, sonst verliert der Stake seine Funktion als
//!   Angriffskosten.
//! - **Die Alternative:** Bootstrap über ein per Konfiguration gesetztes
//!   Genesis-Komitee, danach reines Produkt `stake × Arbeit`. Das wurde
//!   zugunsten der Summenform zurückgestellt, nicht verworfen.
//!
//! Wiedervorlage-Vermerk auch in `CONSENSUS/README/Fahrplan-v1.md`
//! (Abschnitt „Zur Wiedervorlage") und in
//! `README/Intern/State-of-the-Project.md`, Abschnitt 7.
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        self.work_per_epoch
            .sort_by_key(|(epoch, _)| std::cmp::Reverse(*epoch));

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
            // Sättigen statt überlaufen: ein Überlauf würde im
            // Debug-Build panicken und im Release-Build stillschweigend
            // umlaufen — zwei Nodes mit verschiedenen Build-Profilen
            // kämen zu verschiedenen Stimmgewichten.
            total = total.saturating_add(decayed);
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
    let mut result = value as u128;

    for _ in 0..epochs {
        // result = result * 95 / 100 — u128, weil `value * 95` für
        // Werte oberhalb von u64::MAX/95 sonst überläuft (Panic im
        // Debug-Build, stiller Umlauf im Release-Build).
        result = (result * DECAY_FACTOR_NUM as u128) / DECAY_FACTOR_DEN as u128;
    }

    u64::try_from(result).unwrap_or(u64::MAX)
}

/// Eine vTFE-Einheit in vTFE-Kleinstbeträgen (1 vTFE = 10⁶).
///
/// Bezugsgröße für den Arbeitsanteil des Stimmgewichts: eine volle
/// abgeklungene vTFE-Einheit verdoppelt das Gewicht gegenüber dem
/// reinen Stake.
pub const VTFE_UNIT: u64 = 1_000_000;

/// Berechnet das Stimmgewicht eines Validators.
///
/// **Formel:** `voting_weight = stake + (stake × decayed_work) / VTFE_UNIT`
///
/// Der Stake ist die Grundlage, die nachgewiesene Arbeit multipliziert
/// sie hoch. Siehe die Modul-Dokumentation für die Begründung, warum
/// hier eine Summe und kein reines Produkt steht (Bootstrap-Blockade).
///
/// **Parameter:**
/// - `stake`: Stake des Validators (in MYL-Kleinstbeträgen)
/// - `history`: Historische Inferenzarbeit
/// - `current_epoch`: Aktuelle Epoche
///
/// **Returns:** Stimmgewicht (u64, gesättigt statt überlaufend).
pub fn calculate_voting_weight(
    stake: u64,
    history: &InferenceHistory,
    current_epoch: u64,
) -> u64 {
    let decayed_work = history.decayed_weight(current_epoch);

    // stake ist in MYL-Kleinstbeträgen (1 MYL = 10^6),
    // decayed_work in vTFE-Kleinstbeträgen (1 vTFE = 10^6).
    // u128 für die Zwischenrechnung, Sättigung statt Überlauf.
    let work_bonus = (stake as u128 * decayed_work as u128) / VTFE_UNIT as u128;
    let weight = stake as u128 + work_bonus;

    u64::try_from(weight).unwrap_or(u64::MAX)
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
        history.add_work(10, VTFE_UNIT); // 1 vTFE

        let stake = 10_000_000; // 10 MYL
        let weight = calculate_voting_weight(stake, &history, 10);

        // stake + stake * 1 vTFE / VTFE_UNIT = stake * 2
        assert_eq!(weight, 20_000_000);
    }

    #[test]
    fn calculate_voting_weight_with_decay() {
        let mut history = InferenceHistory::new();
        history.add_work(10, VTFE_UNIT); // 1 vTFE

        let stake = 10_000_000; // 10 MYL
        let weight = calculate_voting_weight(stake, &history, 11);

        // Arbeit klingt eine Epoche ab: 1 vTFE * 0,95
        // weight = stake + stake * 0,95 = 19_500_000
        assert_eq!(weight, 19_500_000);
    }

    /// Bootstrap: ohne Arbeitshistorie muss der Stake allein zählen.
    /// Die alte Produktformel lieferte hier 0 — ein Validator mit
    /// Gewicht 0 wird nie gewählt, kann nie Arbeit nachweisen und
    /// bleibt dauerhaft bei 0. Bei Genesis wäre kein Komitee wählbar.
    #[test]
    fn ohne_arbeitshistorie_zaehlt_der_stake() {
        let history = InferenceHistory::new();
        let stake = 10_000_000;
        assert_eq!(calculate_voting_weight(stake, &history, 0), stake);
    }

    /// Arbeit erhöht das Gewicht monoton.
    #[test]
    fn mehr_arbeit_ergibt_mehr_gewicht() {
        let stake = 10_000_000;
        let mut wenig = InferenceHistory::new();
        wenig.add_work(5, VTFE_UNIT);
        let mut viel = InferenceHistory::new();
        viel.add_work(5, VTFE_UNIT * 3);

        assert!(
            calculate_voting_weight(stake, &viel, 5)
                > calculate_voting_weight(stake, &wenig, 5)
        );
    }

    /// Kein Überlauf bei extremen Werten — vorher panickte
    /// `apply_decay` im Debug-Build (`value * 95`) und lief im
    /// Release-Build still um.
    #[test]
    fn extremwerte_saettigen_statt_zu_ueberlaufen() {
        let mut history = InferenceHistory::new();
        history.add_work(0, u64::MAX);
        // Darf weder panicken noch umlaufen.
        let w = calculate_voting_weight(u64::MAX, &history, 0);
        assert_eq!(w, u64::MAX);

        let mut spaet = InferenceHistory::new();
        spaet.add_work(0, u64::MAX);
        let _ = spaet.decayed_weight(9); // neun Abkling-Schritte
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
