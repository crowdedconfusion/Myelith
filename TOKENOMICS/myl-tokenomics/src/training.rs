//! Trainingsvergütungs-Obergrenze (Punkt 1.4, Whitepaper Kap. 5.3,
//! Anhang B.7.1).
//!
//! Regel: Trainingsvergütung ≤ 70 % der Inferenzvergütung je
//! Rechenstunde. Finanzierung ausschließlich aus Treasury und dem
//! abschaltbaren Gebührenaufschlag — **nicht aus Zusatzprägung**
//! (Kap. 5.3). Die Durchsetzung der Finanzierungsquelle erfolgt auf
//! Ledger-Ebene (CONSENSUS); dieses Modul liefert die reine
//! Obergrenzen-Berechnung.

/// Obergrenze der Trainingsvergütung als Anteil der Inferenzvergütung
/// in Basispunkten (70 %).
pub const TRAINING_CAP_BPS: u64 = 7_000;
/// Basispunktbasis (100 %).
pub const BPS_TOTAL: u64 = 10_000;

/// Obergrenze der Trainingsvergütung je Rechenstunde
/// (floor: 70 % der Inferenzvergütung).
pub fn training_reward_cap(inference_reward_per_hour: u64) -> u64 {
    ((inference_reward_per_hour as u128 * TRAINING_CAP_BPS as u128) / BPS_TOTAL as u128) as u64
}

/// Begrenzt eine angefragte Trainingsvergütung auf die Obergrenze.
pub fn capped_training_reward(requested: u64, inference_reward_per_hour: u64) -> u64 {
    requested.min(training_reward_cap(inference_reward_per_hour))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obergrenze_ist_siebzig_prozent() {
        assert_eq!(training_reward_cap(1_000), 700);
        assert_eq!(training_reward_cap(1_001), 700); // floor
        assert_eq!(training_reward_cap(0), 0);
        assert_eq!(training_reward_cap(10), 7);
    }

    #[test]
    fn anfrage_wird_gekuerzt_oder_unveraendert_gelassen() {
        // Unter der Grenze: unverändert.
        assert_eq!(capped_training_reward(500, 1_000), 500);
        // Exakt an der Grenze.
        assert_eq!(capped_training_reward(700, 1_000), 700);
        // Über der Grenze: gekürzt.
        assert_eq!(capped_training_reward(900, 1_000), 700);
        // Inferenzvergütung 0 ⇒ Trainingsvergütung 0.
        assert_eq!(capped_training_reward(100, 0), 0);
    }

    #[test]
    fn ueberlaufsicherheit() {
        // u128-Zwischenrechnung, kein Überlauf auch bei Extremwerten:
        // floor(u64::MAX · 0,7) = 12 912 720 851 596 686 130.
        assert_eq!(training_reward_cap(u64::MAX), 12_912_720_851_596_686_130);
    }

    #[test]
    fn determinismus() {
        for v in [0u64, 1, 7, 999, 123_456, u64::MAX >> 4] {
            assert_eq!(training_reward_cap(v), training_reward_cap(v));
            assert_eq!(
                capped_training_reward(v, v),
                capped_training_reward(v, v)
            );
        }
    }
}
