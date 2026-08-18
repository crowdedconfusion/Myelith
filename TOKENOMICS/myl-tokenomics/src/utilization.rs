//! Auslastungsmessung u_e (Whitepaper Kap. 5.4).
//!
//! Die Auslastung ist definiert als:
//! u_e = nachgefragte vTFE / verfügbare Pod-Kapazität
//!
//! Die Nachfrage wird aus den Burns der aktuellen Epoche abgeleitet.
//! Die verfügbare Kapazität ist die gesamte Pod-Kapazität des Netzwerks.
//!
//! **Konsens-Feld:** Die Berechnung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Fixed-Point-Arithmetik mit 16 Bit Nachkommastellen.
//! u_e = 1.0 bedeutet volle Auslastung, u_e > 1.0 bedeutet Überlast.

/// Fixed-Point-Skalierung für Auslastung (16 Bit Nachkommastellen).
/// 1.0 = 2^16 = 65536
pub const UTILIZATION_SCALE: i64 = 1 << 16;

/// Berechnet die Auslastung u_e aus Nachfrage und Kapazität.
///
/// **Parameter:**
/// - `demanded_vtfe`: Nachgefragte vTFE in Epoche e (u64, in vTFE-Einheiten)
/// - `available_capacity`: Verfügbare Pod-Kapazität in Epoche e (u64, in vTFE-Einheiten)
///
/// **Returns:** Auslastung u_e als Fixed-Point i64 mit 16 Bit Nachkommastellen
///
/// **Beispiel:**
/// ```
/// use myl_tokenomics::utilization::calculate_utilization;
///
/// // 80% Auslastung: 800 vTFE nachgefragt, 1000 vTFE verfügbar
/// let demanded = 800_000_000u64; // 800 vTFE (skaliert mit 10^6)
/// let capacity = 1_000_000_000u64; // 1000 vTFE (skaliert mit 10^6)
/// let utilization = calculate_utilization(demanded, capacity);
/// // utilization ≈ 0.8 * 65536 = 52428
/// ```
///
/// **Hinweis:** Wenn available_capacity = 0, wird u_e = 0 zurückgegeben
/// (keine Kapazität = keine Auslastung messbar).
pub fn calculate_utilization(demanded_vtfe: u64, available_capacity: u64) -> i64 {
    if available_capacity == 0 {
        return 0;
    }

    // u_e = demanded / capacity
    // Ergebnis als Fixed-Point mit 16 Bit Nachkommastellen
    // Verwende u128 für Zwischenrechnung, um Überlauf zu vermeiden
    
    
    ((demanded_vtfe as u128 * UTILIZATION_SCALE as u128) 
                       / available_capacity as u128) as i64
}

/// Berechnet die Auslastung aus der Burn-Historie.
///
/// Die Nachfrage wird aus den Burns der aktuellen Epoche abgeleitet.
/// Dies ist eine Vereinfachung: In der Praxis könnte die Nachfrage auch
/// aus Pending-Transaktionen oder anderen Signalen abgeleitet werden.
///
/// **Parameter:**
/// - `burned_vtfe`: In Epoche e verbrannte vTFE (u64, in vTFE-Einheiten)
/// - `available_capacity`: Verfügbare Pod-Kapazität in Epoche e (u64, in vTFE-Einheiten)
///
/// **Returns:** Auslastung u_e als Fixed-Point i64 mit 16 Bit Nachkommastellen
pub fn utilization_from_burns(burned_vtfe: u64, available_capacity: u64) -> i64 {
    calculate_utilization(burned_vtfe, available_capacity)
}

/// Konvertiert eine Auslastung von Fixed-Point zu f64 (für Debug/Logging).
///
/// **Parameter:**
/// - `utilization_fixed`: Auslastung als Fixed-Point i64 mit 16 Bit Nachkommastellen
///
/// **Returns:** Auslastung als f64 (z.B. 0.8 für 80% Auslastung)
///
/// **Hinweis:** Diese Funktion verwendet Gleitkomma und ist nur für
/// Debug/Logging gedacht, nicht für Konsens-Berechnungen.
pub fn utilization_to_f64(utilization_fixed: i64) -> f64 {
    utilization_fixed as f64 / UTILIZATION_SCALE as f64
}

/// Konvertiert eine Auslastung von f64 zu Fixed-Point (für Tests).
///
/// **Parameter:**
/// - `utilization_float`: Auslastung als f64 (z.B. 0.8 für 80% Auslastung)
///
/// **Returns:** Auslastung als Fixed-Point i64 mit 16 Bit Nachkommastellen
///
/// **Hinweis:** Diese Funktion verwendet Gleitkomma und ist nur für
/// Tests gedacht, nicht für Konsens-Berechnungen.
pub fn utilization_from_f64(utilization_float: f64) -> i64 {
    (utilization_float * UTILIZATION_SCALE as f64).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilization_zero_demand() {
        // Keine Nachfrage → u_e = 0
        let demanded = 0u64;
        let capacity = 1_000_000_000u64;
        let util = calculate_utilization(demanded, capacity);
        assert_eq!(util, 0);
    }

    #[test]
    fn utilization_full_capacity() {
        // Volle Auslastung → u_e = 1.0
        let demanded = 1_000_000_000u64;
        let capacity = 1_000_000_000u64;
        let util = calculate_utilization(demanded, capacity);
        assert_eq!(util, UTILIZATION_SCALE); // 1.0
    }

    #[test]
    fn utilization_partial() {
        // 80% Auslastung
        let demanded = 800_000_000u64;
        let capacity = 1_000_000_000u64;
        let util = calculate_utilization(demanded, capacity);
        let expected = utilization_from_f64(0.8);
        // Toleranz: < 0.01%
        let tolerance = UTILIZATION_SCALE / 10000;
        assert!((util - expected).abs() < tolerance);
    }

    #[test]
    fn utilization_overload() {
        // Überlast: 150% Auslastung
        let demanded = 1_500_000_000u64;
        let capacity = 1_000_000_000u64;
        let util = calculate_utilization(demanded, capacity);
        let expected = utilization_from_f64(1.5);
        let tolerance = UTILIZATION_SCALE / 10000;
        assert!((util - expected).abs() < tolerance);
    }

    #[test]
    fn utilization_zero_capacity() {
        // Keine Kapazität → u_e = 0 (nicht unendlich)
        let demanded = 1_000_000_000u64;
        let capacity = 0u64;
        let util = calculate_utilization(demanded, capacity);
        assert_eq!(util, 0);
    }

    #[test]
    fn utilization_deterministic() {
        // Gleiche Eingabe → gleiche Ausgabe (bitgleich)
        let demanded = 123_456_789u64;
        let capacity = 987_654_321u64;
        let util1 = calculate_utilization(demanded, capacity);
        let util2 = calculate_utilization(demanded, capacity);
        assert_eq!(util1, util2);
    }

    #[test]
    fn utilization_from_burns_equivalent() {
        // utilization_from_burns sollte identisch zu calculate_utilization sein
        let burned = 500_000_000u64;
        let capacity = 1_000_000_000u64;
        let util1 = calculate_utilization(burned, capacity);
        let util2 = utilization_from_burns(burned, capacity);
        assert_eq!(util1, util2);
    }

    #[test]
    fn utilization_to_from_f64_roundtrip() {
        // Konvertierung hin und zurück sollte (ungefähr) den gleichen Wert ergeben
        let original = utilization_from_f64(0.75);
        let as_f64 = utilization_to_f64(original);
        let back = utilization_from_f64(as_f64);
        // Toleranz: < 0.01% (Rundungsfehler bei f64-Konvertierung)
        let tolerance = UTILIZATION_SCALE / 10000;
        assert!((original - back).abs() < tolerance, 
                "Roundtrip failed: original={}, back={}", original, back);
    }

    #[test]
    fn utilization_scale_constant() {
        // UTILIZATION_SCALE sollte 2^16 = 65536 sein
        assert_eq!(UTILIZATION_SCALE, 65536);
    }
}
