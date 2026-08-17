//! Stichproben-Lotterie: Segmente für Checker markieren (Anhang A.2, Schritt 6).
//!
//! Ein bestimmter Prozentsatz der Segmente wird für Checker (Verification) markiert.
//! Die Auswahl erfolgt deterministisch mit dem Seed, damit jeder Node dieselben
//! Segmente für die Verification auswählt.
//!
//! **Konsens-Feld:** Die Sampling-Regeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Fisher-Yates Shuffle mit dem Seed, dann die ersten p·|segments|
//! Segmente auswählen. Deterministisch und gleichverteilt.
//!
//! Der Shuffle liegt in [`crate::shuffle`] — eine Implementierung für alle
//! Verwendungen. Die Gleichverteilung ist hier keine Bequemlichkeit,
//! sondern Sicherheitseigenschaft: sie entscheidet, welche Arbeit
//! überhaupt auditiert wird. Siehe dort die Beschreibung von Fund A6.

use crate::shuffle::deterministic_shuffle;

/// Ergebnis der Stichproben-Lotterie.
#[derive(Debug, Clone, PartialEq)]
pub struct SamplingResult {
    /// Indizes der für Checker markierten Segmente (sortiert).
    pub sampled_segments: Vec<u32>,
    /// Prozentsatz der gesampelten Segmente (0.0 bis 1.0).
    pub sampling_rate: f64,
    /// Gesamtanzahl der Segmente.
    pub total_segments: u32,
}

/// Führt die Stichproben-Lotterie durch.
///
/// **Algorithmus (Anhang A.2, Schritt 6):**
/// 1. Erstelle eine Liste aller Segment-Indizes [0, 1, 2, ..., num_segments-1]
/// 2. Shuffle die Liste mit Fisher-Yates und dem Seed (deterministisch)
/// 3. Wähle die ersten `num_samples = ceil(sampling_rate * num_segments)` Segmente
/// 4. Sortiere die Indizes (für kanonische Darstellung)
/// 5. Gib das Ergebnis zurück
///
/// **Determinismus:** Gleicher Seed + gleiche Anzahl Segmente + gleiche Sampling-Rate
/// → gleiche markierte Segmente.
///
/// **Parameter:**
/// - `num_segments`: Gesamtanzahl der Segmente
/// - `sampling_rate`: Prozentsatz der zu sampelnden Segmente (0.0 bis 1.0)
/// - `seed`: Epochenseed (aus Phase 2.1) für deterministische Auswahl
///
/// **Returns:** SamplingResult mit den markierten Segment-Indizes
///
/// **Hinweis:** Die Sampling-Rate ist ein Governance-Parameter und wird später
/// in die Governance-Registry aufgenommen (GOVERNANCE Punkt 1.1).
pub fn sample_segments(
    num_segments: u32,
    sampling_rate: f64,
    seed: &[u8; 32],
) -> SamplingResult {
    if num_segments == 0 || sampling_rate <= 0.0 {
        // Clamp rate für das Ergebnis
        let clamped_rate = sampling_rate.min(1.0).max(0.0);
        return SamplingResult {
            sampled_segments: vec![],
            sampling_rate: clamped_rate,
            total_segments: num_segments,
        };
    }

    // Clamp sampling_rate auf [0.0, 1.0]
    let rate = sampling_rate.min(1.0).max(0.0);

    // Berechne Anzahl der zu sampelnden Segmente (ceil)
    let num_samples = (rate * num_segments as f64).ceil() as u32;
    let num_samples = num_samples.min(num_segments); // Nicht mehr als insgesamt

    // Erstelle Liste aller Segment-Indizes
    let mut indices: Vec<u32> = (0..num_segments).collect();

    // Fisher-Yates Shuffle mit Seed
    deterministic_shuffle(&mut indices, seed);

    // Wähle die ersten num_samples Segmente
    let mut sampled: Vec<u32> = indices[..num_samples as usize].to_vec();

    // Sortiere für kanonische Darstellung
    sampled.sort_unstable();

    SamplingResult {
        sampled_segments: sampled,
        sampling_rate: rate,
        total_segments: num_segments,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_segments_basic() {
        let result = sample_segments(10, 0.5, &[0u8; 32]);

        assert_eq!(result.total_segments, 10);
        assert_eq!(result.sampling_rate, 0.5);
        assert_eq!(result.sampled_segments.len(), 5); // 50% von 10
    }

    #[test]
    fn sample_segments_deterministic() {
        let result1 = sample_segments(20, 0.3, &[42u8; 32]);
        let result2 = sample_segments(20, 0.3, &[42u8; 32]);

        assert_eq!(result1, result2);
    }

    #[test]
    fn sample_segments_different_seeds() {
        let result1 = sample_segments(20, 0.3, &[1u8; 32]);
        let result2 = sample_segments(20, 0.3, &[2u8; 32]);

        // Unterschiedliche Seeds führen (meist) zu unterschiedlichen Ergebnissen
        assert_eq!(result1.sampled_segments.len(), result2.sampled_segments.len());
        // Aber nicht unbedingt die gleichen Segmente
    }

    #[test]
    fn sample_segments_zero_rate() {
        let result = sample_segments(10, 0.0, &[0u8; 32]);

        assert!(result.sampled_segments.is_empty());
    }

    #[test]
    fn sample_segments_full_rate() {
        let result = sample_segments(10, 1.0, &[0u8; 32]);

        assert_eq!(result.sampled_segments.len(), 10);
        assert_eq!(result.sampled_segments, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn sample_segments_zero_segments() {
        let result = sample_segments(0, 0.5, &[0u8; 32]);

        assert!(result.sampled_segments.is_empty());
        assert_eq!(result.total_segments, 0);
    }

    #[test]
    fn sample_segments_rate_clamped() {
        // Rate > 1.0 sollte auf 1.0 geclampt werden
        let result = sample_segments(10, 1.5, &[0u8; 32]);

        assert_eq!(result.sampling_rate, 1.0);
        assert_eq!(result.sampled_segments.len(), 10);
    }

    #[test]
    fn sample_segments_negative_rate() {
        // Rate < 0.0 sollte auf 0.0 geclampt werden
        let result = sample_segments(10, -0.5, &[0u8; 32]);

        assert_eq!(result.sampling_rate, 0.0);
        assert!(result.sampled_segments.is_empty());
    }

    #[test]
    fn sample_segments_sorted() {
        let result = sample_segments(20, 0.5, &[42u8; 32]);

        // Indizes sollten sortiert sein
        let mut sorted = result.sampled_segments.clone();
        sorted.sort_unstable();
        assert_eq!(result.sampled_segments, sorted);
    }

    #[test]
    fn sample_segments_no_duplicates() {
        let result = sample_segments(20, 0.5, &[42u8; 32]);

        // Keine Duplikate
        let mut unique = result.sampled_segments.clone();
        unique.dedup();
        assert_eq!(result.sampled_segments.len(), unique.len());
    }

    #[test]
    fn sample_segments_small_count() {
        // 1 Segment, 50% Rate → ceil(0.5) = 1 Segment
        let result = sample_segments(1, 0.5, &[0u8; 32]);

        assert_eq!(result.sampled_segments.len(), 1);
        assert_eq!(result.sampled_segments[0], 0);
    }

    #[test]
    fn sample_segments_ceil_behavior() {
        // 10 Segmente, 33% Rate → ceil(3.3) = 4 Segmente
        let result = sample_segments(10, 0.33, &[0u8; 32]);

        assert_eq!(result.sampled_segments.len(), 4);
    }
}
