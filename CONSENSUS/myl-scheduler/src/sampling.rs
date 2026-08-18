//! Stichproben-Lotterie: Segmente für Checker markieren (Anhang A.2, Schritt 6).
//!
//! Ein bestimmter Prozentsatz der Segmente wird für Checker (Verification) markiert.
//! Die Auswahl erfolgt deterministisch mit dem Seed, damit jeder Node dieselben
//! Segmente für die Verification auswählt.
//!
//! **Konsens-Feld:** Die Sampling-Regeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Die Rate ist eine Ganzzahl in Basispunkten, kein `f64` (Fund A18).**
//! Bis v0.2.9 war `sampling_rate: f64`, und die Anzahl wurde als
//! `(rate * n as f64).ceil() as u32` berechnet. Multiplikation und
//! `ceil` sind zwar IEEE-754-exakt und damit reproduzierbar — aber die
//! Rate ist ein Governance-Parameter, der irgendwann aus einer
//! Konfiguration geparst wird, und `"0.02"` nach `f64` zu parsen ist
//! nicht überall bitgleich. Basispunkte (1 bp = 0,01 %) halten den
//! gesamten Pfad ganzzahlig, wie bei den Slash-Anteilen und der EMA
//! auch.
//!
//! **Design:** Fisher-Yates Shuffle mit dem Seed, dann die ersten p·|segments|
//! Segmente auswählen. Deterministisch und gleichverteilt.
//!
//! Der Shuffle liegt in `myl_types::seed_rng` — eine Implementierung für
//! alle Verwendungen im Protokoll. Die Gleichverteilung ist hier keine
//! Bequemlichkeit, sondern Sicherheitseigenschaft: sie entscheidet,
//! welche Arbeit überhaupt auditiert wird. Siehe dort die Beschreibung
//! von Fund A6.

use myl_types::seed_rng::deterministic_shuffle;

/// Nenner der Basispunkt-Darstellung: 10 000 bp = 100 %.
pub const RATE_DENOMINATOR: u32 = 10_000;

/// Ergebnis der Stichproben-Lotterie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplingResult {
    /// Indizes der für Checker markierten Segmente (sortiert).
    pub sampled_segments: Vec<u32>,
    /// Angewandte Rate in Basispunkten (0..=10 000; 200 bp = 2 %).
    pub sampling_rate_bp: u32,
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
/// - `sampling_rate_bp`: Rate in Basispunkten (200 bp = 2 %), geklemmt
///   auf `0..=RATE_DENOMINATOR`
/// - `seed`: Epochenseed (aus Phase 2.1) für deterministische Auswahl
///
/// **Returns:** SamplingResult mit den markierten Segment-Indizes
///
/// **Hinweis:** Die Sampling-Rate ist ein Governance-Parameter und wird später
/// in die Governance-Registry aufgenommen (GOVERNANCE Punkt 1.1).
pub fn sample_segments(
    num_segments: u32,
    sampling_rate_bp: u32,
    seed: &[u8; 32],
) -> SamplingResult {
    let rate_bp = sampling_rate_bp.min(RATE_DENOMINATOR);

    if num_segments == 0 || rate_bp == 0 {
        return SamplingResult {
            sampled_segments: vec![],
            sampling_rate_bp: rate_bp,
            total_segments: num_segments,
        };
    }

    // Aufrundende Ganzzahldivision: ceil(n · bp / 10000).
    // u64 für das Zwischenprodukt — n · 10 000 sprengt u32 ab ~430 000
    // Segmenten.
    let num_samples =
        (num_segments as u64 * rate_bp as u64).div_ceil(RATE_DENOMINATOR as u64) as u32;
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
        sampling_rate_bp: rate_bp,
        total_segments: num_segments,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_segments_basic() {
        let result = sample_segments(10, 5_000, &[0u8; 32]); // 50 %
        assert_eq!(result.total_segments, 10);
        assert_eq!(result.sampling_rate_bp, 5_000);
        assert_eq!(result.sampled_segments.len(), 5);
    }

    #[test]
    fn sample_segments_deterministic() {
        let a = sample_segments(20, 3_000, &[42u8; 32]);
        let b = sample_segments(20, 3_000, &[42u8; 32]);
        assert_eq!(a, b);
    }

    #[test]
    fn sample_segments_different_seeds() {
        let a = sample_segments(20, 3_000, &[1u8; 32]);
        let b = sample_segments(20, 3_000, &[2u8; 32]);
        assert_eq!(a.sampled_segments.len(), b.sampled_segments.len());
    }

    #[test]
    fn sample_segments_zero_rate() {
        let r = sample_segments(10, 0, &[0u8; 32]);
        assert!(r.sampled_segments.is_empty());
        assert_eq!(r.sampling_rate_bp, 0);
    }

    #[test]
    fn sample_segments_full_rate() {
        let r = sample_segments(10, RATE_DENOMINATOR, &[0u8; 32]);
        assert_eq!(r.sampled_segments.len(), 10);
        assert_eq!(r.sampled_segments, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn sample_segments_zero_segments() {
        let r = sample_segments(0, 5_000, &[0u8; 32]);
        assert!(r.sampled_segments.is_empty());
        assert_eq!(r.total_segments, 0);
    }

    #[test]
    fn sample_segments_rate_clamped() {
        let r = sample_segments(10, RATE_DENOMINATOR + 5_000, &[0u8; 32]);
        assert_eq!(r.sampling_rate_bp, RATE_DENOMINATOR);
        assert_eq!(r.sampled_segments.len(), 10);
    }

    #[test]
    fn sample_segments_sorted() {
        let r = sample_segments(20, 5_000, &[42u8; 32]);
        let mut sorted = r.sampled_segments.clone();
        sorted.sort_unstable();
        assert_eq!(r.sampled_segments, sorted);
    }

    #[test]
    fn sample_segments_no_duplicates() {
        let r = sample_segments(20, 5_000, &[42u8; 32]);
        let mut unique = r.sampled_segments.clone();
        unique.dedup();
        assert_eq!(r.sampled_segments.len(), unique.len());
    }

    #[test]
    fn sample_segments_small_count() {
        // 1 Segment, 50 % → ceil(0,5) = 1
        let r = sample_segments(1, 5_000, &[0u8; 32]);
        assert_eq!(r.sampled_segments, vec![0]);
    }

    #[test]
    fn sample_segments_ceil_behavior() {
        // 10 Segmente, 33 % → ceil(3,3) = 4
        assert_eq!(sample_segments(10, 3_300, &[0u8; 32]).sampled_segments.len(), 4);
        // 10 Segmente, 30 % → ceil(3,0) = 3 (kein unnoetiges Aufrunden)
        assert_eq!(sample_segments(10, 3_000, &[0u8; 32]).sampled_segments.len(), 3);
    }

    /// Eine Rate > 0 muss immer mindestens ein Segment ziehen — sonst
    /// gaebe es Epochen ganz ohne Audit.
    #[test]
    fn kleinste_rate_zieht_mindestens_ein_segment() {
        for n in [1u32, 10, 1_000] {
            let r = sample_segments(n, 1, &[0u8; 32]); // 1 bp = 0,01 %
            assert_eq!(r.sampled_segments.len(), 1, "n={}", n);
        }
    }

    /// Kein Ueberlauf bei grossen Segmentzahlen: n · 10 000 sprengt u32
    /// ab etwa 430 000 Segmenten, deshalb rechnet die Funktion in u64.
    #[test]
    fn grosse_segmentzahlen_ohne_ueberlauf() {
        let n = 1_000_000u32;
        let r = sample_segments(n, 200, &[0u8; 32]); // 2 %
        assert_eq!(r.sampled_segments.len(), 20_000);
        assert_eq!(r.total_segments, n);
    }

    /// Die Rate ist ganzzahlig — das Ergebnis ist damit `Eq` und laesst
    /// sich ohne Toleranz vergleichen (vorher `f64`, Fund A18).
    #[test]
    fn ergebnis_ist_exakt_vergleichbar() {
        let a = sample_segments(50, 1_234, &[9u8; 32]);
        let b = sample_segments(50, 1_234, &[9u8; 32]);
        assert!(a == b);
    }
}
