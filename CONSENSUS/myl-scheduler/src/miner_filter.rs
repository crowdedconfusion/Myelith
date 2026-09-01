//! Miner-Filterung nach Hardware-Klasse und Registrierungsschluss (Anhang A.2, Schritt 2).
//!
//! Nur Miner, die sich vor dem Registrierungsschluss (Epoche e-2) angemeldet haben
//! und die richtige Hardware-Klasse haben, werden für die Pod-Bildung berücksichtigt.
//! Dies verhindert, dass neue Miner sich kurzfristig anmelden, um die Zuteilung zu
//! beeinflussen.
//!
//! **Konsens-Feld:** Die Filterungsregeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Die Filterung ist eine reine Funktion (Eingabe → Ausgabe) ohne
//! versteckten globalen Zustand. Borsh-Serialisierung für kanonische Darstellung.


pub use myl_types::miner::{HardwareClass, MinerRegistration};

/// Filtert eine Liste von Miner-Registrierungen nach Hardware-Klasse und Registrierungsschluss.
///
/// **Algorithmus (Anhang A.2, Schritt 2):**
/// 1. Nimm alle registrierten Miner
/// 2. Filtere nach Hardware-Klasse (nur `allowed_classes`)
/// 3. Filtere nach Registrierungsschluss (nur Miner, die sich vor Epoche e-2 registriert haben)
/// 4. Gib die gefilterte Liste zurück
///
/// **Determinismus:** Gleiche Eingabe → gleiche Ausgabe. Die Reihenfolge der Ausgabe
/// ist identisch mit der Reihenfolge der Eingabe (stabile Filterung).
///
/// **Parameter:**
/// - `registrations`: Liste aller registrierten Miner
/// - `target_epoch`: Epoche, für die die Filterung durchgeführt wird
/// - `allowed_classes`: Liste der erlaubten Hardware-Klassen
///
/// **Returns:** Gefilterte Liste von Miner-Registrierungen
pub fn filter_miners(
    registrations: &[MinerRegistration],
    target_epoch: u64,
    allowed_classes: &[HardwareClass],
) -> Vec<MinerRegistration> {
    registrations
        .iter()
        .filter(|reg| reg.is_qualified(target_epoch, allowed_classes))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::ids::MinerId;

    fn test_registration(miner_byte: u8, hw_class: HardwareClass, reg_epoch: u64) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([miner_byte; 32]),
            hardware_class: hw_class,
            registration_epoch: reg_epoch,
            zone: myl_types::node_metadata::GeoRegion::Europe,
        }
    }

    #[test]
    fn hardware_class_all() {
        assert_eq!(HardwareClass::all().len(), 4);
    }

    #[test]
    fn miner_qualified_correct_epoch() {
        let reg = test_registration(1, HardwareClass::MediumGpu, 5);
        
        // target_epoch = 7, registration_deadline = 5
        // registration_epoch = 5 <= 5 → qualifiziert
        assert!(reg.is_qualified(7, &[HardwareClass::MediumGpu]));
    }

    #[test]
    fn miner_qualified_early_registration() {
        let reg = test_registration(1, HardwareClass::MediumGpu, 3);
        
        // target_epoch = 7, registration_deadline = 5
        // registration_epoch = 3 <= 5 → qualifiziert
        assert!(reg.is_qualified(7, &[HardwareClass::MediumGpu]));
    }

    #[test]
    fn miner_not_qualified_late_registration() {
        let reg = test_registration(1, HardwareClass::MediumGpu, 6);
        
        // target_epoch = 7, registration_deadline = 5
        // registration_epoch = 6 > 5 → nicht qualifiziert
        assert!(!reg.is_qualified(7, &[HardwareClass::MediumGpu]));
    }

    #[test]
    fn miner_not_qualified_wrong_hardware() {
        let reg = test_registration(1, HardwareClass::SmallGpu, 5);
        
        // target_epoch = 7, registration_deadline = 5
        // registration_epoch = 5 <= 5 → OK
        // hardware_class = SmallGpu, allowed = [MediumGpu] → nicht qualifiziert
        assert!(!reg.is_qualified(7, &[HardwareClass::MediumGpu]));
    }

    #[test]
    fn miner_qualified_multiple_allowed_classes() {
        let reg = test_registration(1, HardwareClass::MediumGpu, 5);
        
        let allowed = vec![HardwareClass::SmallGpu, HardwareClass::MediumGpu, HardwareClass::LargeGpu];
        assert!(reg.is_qualified(7, &allowed));
    }

    #[test]
    fn filter_miners_basic() {
        let registrations = vec![
            test_registration(1, HardwareClass::MediumGpu, 5),  // qualifiziert
            test_registration(2, HardwareClass::SmallGpu, 5),   // nicht qualifiziert (falsche HW)
            test_registration(3, HardwareClass::MediumGpu, 6),  // nicht qualifiziert (zu spät)
            test_registration(4, HardwareClass::MediumGpu, 3),  // qualifiziert
        ];
        
        let allowed = vec![HardwareClass::MediumGpu];
        let filtered = filter_miners(&registrations, 7, &allowed);
        
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].miner_id, MinerId::new([1u8; 32]));
        assert_eq!(filtered[1].miner_id, MinerId::new([4u8; 32]));
    }

    #[test]
    fn filter_miners_empty_input() {
        let registrations: Vec<MinerRegistration> = vec![];
        let allowed = vec![HardwareClass::MediumGpu];
        let filtered = filter_miners(&registrations, 7, &allowed);
        
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_miners_all_filtered() {
        let registrations = vec![
            test_registration(1, HardwareClass::SmallGpu, 5),
            test_registration(2, HardwareClass::SmallGpu, 6),
        ];
        
        let allowed = vec![HardwareClass::MediumGpu];
        let filtered = filter_miners(&registrations, 7, &allowed);
        
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_miners_preserves_order() {
        let registrations = vec![
            test_registration(3, HardwareClass::MediumGpu, 5),
            test_registration(1, HardwareClass::MediumGpu, 3),
            test_registration(2, HardwareClass::MediumGpu, 4),
        ];
        
        let allowed = vec![HardwareClass::MediumGpu];
        let filtered = filter_miners(&registrations, 7, &allowed);
        
        // Reihenfolge sollte erhalten bleiben
        assert_eq!(filtered[0].miner_id, MinerId::new([3u8; 32]));
        assert_eq!(filtered[1].miner_id, MinerId::new([1u8; 32]));
        assert_eq!(filtered[2].miner_id, MinerId::new([2u8; 32]));
    }

    #[test]
    fn registration_borsh_roundtrip() {
        let reg = test_registration(1, HardwareClass::MediumGpu, 5);
        let bytes = borsh::to_vec(&reg).expect("serialization");
        let decoded: MinerRegistration = borsh::from_slice(&bytes).expect("deserialization");
        
        assert_eq!(reg, decoded);
    }
}
