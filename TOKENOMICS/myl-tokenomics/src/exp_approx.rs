//! Ganzzahlige exp()-Approximation (LUT-basiert) für die Preisformel.
//!
//! Whitepaper Kap. 5.4: `P_{e+1} = P_e · exp(κ(u_e − u*))`
//!
//! Da wir keine Gleitkomma-Arithmetik verwenden dürfen (Konsens-Determinismus),
//! muss exp() ganzzahlig approximiert werden. Analog zu INTEGER_LLMs Ansatz
//! für nichtlineare Funktionen (Kap. 6.2/B.5.3).
//!
//! **Konsens-Feld:** Die Approximationsmethode ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Look-Up-Table (LUT) mit linearer Interpolation.
//! - Bereich: -10 bis +10 (abgedeckter Exponenten-Bereich)
//! - Stützstellen: 2048 (ausreichend für Preisstabilität)
//! - Fixed-Point: exp-Werte als i64 mit 32 Bit Nachkommastellen
//! - Linear interpolation zwischen Stützstellen für Genauigkeit

use std::sync::OnceLock;

/// Anzahl der Stützstellen in der LUT.
const LUT_SIZE: usize = 2048;

/// Minimaler Exponent (Fixed-Point mit 16 Bit Nachkommastellen).
/// -10.0 als Fixed-Point: -10 * 2^16 = -655360
const EXP_MIN: i64 = -655360;

/// Maximaler Exponent (Fixed-Point mit 16 Bit Nachkommastellen).
/// +10.0 als Fixed-Point: 10 * 2^16 = 655360
const EXP_MAX: i64 = 655360;

/// Fixed-Point-Skalierung für Exponenten (16 Bit Nachkommastellen).
const EXP_SCALE: i64 = 1 << 16;

/// Fixed-Point-Skalierung für exp-Ergebnis (32 Bit Nachkommastellen).
/// exp(x) wird als i64 mit 32 Bit Nachkommastellen gespeichert.
const RESULT_SCALE: i64 = 1 << 32;

/// Look-Up-Table für exp(x).
///
/// Indizes: 0 bis LUT_SIZE-1
/// Werte: exp(x) als i64 mit 32 Bit Nachkommastellen
///
/// Die LUT wird einmalig beim ersten Aufruf generiert (lazy initialization).
static EXP_LUT: OnceLock<[i64; LUT_SIZE]> = OnceLock::new();

/// Initialisiert die exp()-LUT (einmalig).
///
/// Berechnet exp(x) für LUT_SIZE gleichmäßig verteilte x-Werte im Bereich
/// [EXP_MIN, EXP_MAX] und speichert die Ergebnisse als Fixed-Point i64.
fn init_exp_lut() -> &'static [i64; LUT_SIZE] {
    EXP_LUT.get_or_init(|| {
        let mut lut = [0i64; LUT_SIZE];
        let step = (EXP_MAX - EXP_MIN) / (LUT_SIZE as i64 - 1);

        for i in 0..LUT_SIZE {
            let x_fixed = EXP_MIN + (i as i64) * step;
            // Konvertiere Fixed-Point zu f64 für LUT-Generierung
            // (nur bei Initialisierung erlaubt, nicht im Inferenzpfad)
            let x_float = x_fixed as f64 / EXP_SCALE as f64;
            let exp_float = x_float.exp();
            // Konvertiere zurück zu Fixed-Point mit 32 Bit Nachkommastellen
            lut[i] = (exp_float * RESULT_SCALE as f64).round() as i64;
        }

        lut
    })
}

/// Approximiert exp(x) ganzzahlig mit LUT und linearer Interpolation.
///
/// **Parameter:**
/// - `x_fixed`: Exponent als Fixed-Point i64 mit 16 Bit Nachkommastellen
///
/// **Returns:** exp(x) als i64 mit 32 Bit Nachkommastellen
///
/// **Genauigkeit:** < 1% Fehler im Bereich [-10, +10]
///
/// **Determinismus:** Bitgleich auf allen Plattformen (keine Gleitkomma im Inferenzpfad).
///
/// **Beispiel:**
/// ```
/// use myl_tokenomics::exp_approx::exp_approx;
///
/// // exp(0) ≈ 1.0 (mit <1% Fehler durch LUT-Approximation)
/// let zero = 0i64; // 0.0 als Fixed-Point
/// let result = exp_approx(zero);
/// let expected = 1i64 << 32; // 1.0 als Fixed-Point mit 32 Bit
/// let tolerance = expected / 100; // 1% Toleranz
/// assert!((result - expected).abs() < tolerance);
/// ```
pub fn exp_approx(x_fixed: i64) -> i64 {
    // Initialisiere LUT beim ersten Aufruf
    let lut = init_exp_lut();

    // Clamp auf gültigen Bereich
    let x_clamped = x_fixed.max(EXP_MIN).min(EXP_MAX);

    // Berechne Index und Interpolationsfaktor in Fixed-Point
    let range = EXP_MAX - EXP_MIN;
    // normalized ist in Fixed-Point mit EXP_SCALE
    let normalized = ((x_clamped - EXP_MIN) * ((LUT_SIZE as i64 - 1) * EXP_SCALE)) / range;
    let index = (normalized / EXP_SCALE) as usize;
    let frac = normalized % EXP_SCALE;

    // Linear interpolation zwischen lut[index] und lut[index+1]
    if index + 1 < LUT_SIZE {
        let y0 = lut[index];
        let y1 = lut[index + 1];
        // Interpolation: y = y0 + (y1 - y0) * frac / EXP_SCALE
        let diff = y1 - y0;
        y0 + (diff * frac) / EXP_SCALE
    } else {
        lut[index]
    }
}

/// Berechnet den Credit-Preis-Update: P_{e+1} = P_e · exp(κ(u_e − u*))
///
/// **Parameter:**
/// - `price_e`: Credit-Preis in Epoche e (Fixed-Point i64 mit 32 Bit Nachkommastellen)
/// - `kappa`: Sensitivitätsparameter (Fixed-Point i64 mit 16 Bit Nachkommastellen)
/// - `utilization_e`: Auslastung in Epoche e (Fixed-Point i64 mit 16 Bit Nachkommastellen, 0.0 bis 1.0)
/// - `utilization_target`: Ziel-Auslastung u* (Fixed-Point i64 mit 16 Bit Nachkommastellen)
///
/// **Returns:** Credit-Preis in Epoche e+1 (Fixed-Point i64 mit 32 Bit Nachkommastellen)
///
/// **Formel:** P_{e+1} = P_e · exp(κ(u_e − u*))
///
/// **Beispiel:**
/// ```
/// use myl_tokenomics::exp_approx::update_price;
///
/// // Preis = 1.0, kappa = 0.1, utilization = 0.8, target = 0.7
/// // P_{e+1} = 1.0 · exp(0.1 · (0.8 - 0.7)) = 1.0 · exp(0.01) ≈ 1.01005
/// let price = 1i64 << 32; // 1.0
/// let kappa = (0.1 * 65536.0) as i64; // 0.1 als Fixed-Point
/// let util = (0.8 * 65536.0) as i64; // 0.8 als Fixed-Point
/// let target = (0.7 * 65536.0) as i64; // 0.7 als Fixed-Point
/// let new_price = update_price(price, kappa, util, target);
/// // new_price ≈ 1.01005 * 2^32
/// ```
pub fn update_price(
    price_e: i64,
    kappa: i64,
    utilization_e: i64,
    utilization_target: i64,
) -> i64 {
    // Berechne Exponent: κ(u_e − u*)
    let delta_util = utilization_e - utilization_target;
    // Multiplikation: kappa * delta_util (beide mit 16 Bit Nachkommastellen)
    // Ergebnis hat 32 Bit Nachkommastellen, aber wir brauchen 16 Bit für exp_approx
    // Verwende i128 für Zwischenrechnung, um Überlauf zu vermeiden
    let exponent = ((kappa as i128 * delta_util as i128) / EXP_SCALE as i128) as i64;

    // Berechne exp(κ(u_e − u*))
    let exp_factor = exp_approx(exponent);

    // Multipliziere: P_e · exp(...)
    // price_e hat 32 Bit Nachkommastellen, exp_factor hat 32 Bit Nachkommastellen
    // Ergebnis soll 32 Bit Nachkommastellen haben
    // Verwende i128 für Zwischenrechnung, um Überlauf zu vermeiden
    ((price_e as i128 * exp_factor as i128) / RESULT_SCALE as i128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_zero() {
        // exp(0) = 1.0
        let zero = 0i64;
        let result = exp_approx(zero);
        let expected = RESULT_SCALE; // 1.0 als Fixed-Point
        // Toleranz: < 1% (lineare Interpolation hat Approximationsfehler)
        let tolerance = RESULT_SCALE / 100;
        assert!((result - expected).abs() < tolerance, "exp(0) should be ~1.0, got {}", result);
    }

    #[test]
    fn exp_one() {
        // exp(1) ≈ 2.71828
        let one = EXP_SCALE; // 1.0 als Fixed-Point
        let result = exp_approx(one);
        let expected = (2.71828 * RESULT_SCALE as f64) as i64;
        // Toleranz: < 1%
        let tolerance = expected / 100;
        assert!((result - expected).abs() < tolerance, "exp(1) should be ~2.718, got {}", result);
    }

    #[test]
    fn exp_negative() {
        // exp(-1) ≈ 0.36788
        let neg_one = -EXP_SCALE; // -1.0 als Fixed-Point
        let result = exp_approx(neg_one);
        let expected = (0.36788 * RESULT_SCALE as f64) as i64;
        // Toleranz: < 1%
        let tolerance = expected / 100;
        assert!((result - expected).abs() < tolerance, "exp(-1) should be ~0.368, got {}", result);
    }

    #[test]
    fn exp_clamped_high() {
        // exp(100) sollte auf exp(10) geclampt werden
        let high = 100 * EXP_SCALE;
        let result = exp_approx(high);
        let expected = exp_approx(10 * EXP_SCALE);
        assert_eq!(result, expected);
    }

    #[test]
    fn exp_clamped_low() {
        // exp(-100) sollte auf exp(-10) geclampt werden
        let low = -100 * EXP_SCALE;
        let result = exp_approx(low);
        let expected = exp_approx(-10 * EXP_SCALE);
        assert_eq!(result, expected);
    }

    #[test]
    fn exp_deterministic() {
        // Gleiche Eingabe → gleiche Ausgabe (bitgleich)
        let x = 12345i64;
        let result1 = exp_approx(x);
        let result2 = exp_approx(x);
        assert_eq!(result1, result2);
    }

    #[test]
    fn update_price_stable() {
        // Bei u_e = u* sollte der Preis stabil bleiben
        let price = 1000 * RESULT_SCALE; // 1000.0
        let kappa = (0.1 * EXP_SCALE as f64) as i64; // 0.1
        let util = (0.7 * EXP_SCALE as f64) as i64; // 0.7
        let target = (0.7 * EXP_SCALE as f64) as i64; // 0.7

        let new_price = update_price(price, kappa, util, target);

        // exp(0) = 1.0, also sollte new_price ≈ price sein
        // Toleranz: < 1% (wegen exp-Approximation)
        let tolerance = price / 100;
        assert!((new_price - price).abs() < tolerance, "Price should be stable, got {} vs {}", new_price, price);
    }

    #[test]
    fn update_price_increases_with_overload() {
        // Bei u_e > u* sollte der Preis steigen
        let price = 1000 * RESULT_SCALE;
        let kappa = (0.1 * EXP_SCALE as f64) as i64;
        let util = (0.9 * EXP_SCALE as f64) as i64; // 0.9 (Überlast)
        let target = (0.7 * EXP_SCALE as f64) as i64; // 0.7

        let new_price = update_price(price, kappa, util, target);

        assert!(new_price > price);
    }

    #[test]
    fn update_price_decreases_with_underload() {
        // Bei u_e < u* sollte der Preis sinken
        let price = 1000 * RESULT_SCALE;
        let kappa = (0.1 * EXP_SCALE as f64) as i64;
        let util = (0.5 * EXP_SCALE as f64) as i64; // 0.5 (Unterlast)
        let target = (0.7 * EXP_SCALE as f64) as i64; // 0.7

        let new_price = update_price(price, kappa, util, target);

        assert!(new_price < price);
    }

    #[test]
    fn update_price_deterministic() {
        // Gleiche Eingabe → gleiche Ausgabe (bitgleich)
        let price = 1000 * RESULT_SCALE;
        let kappa = (0.1 * EXP_SCALE as f64) as i64;
        let util = (0.8 * EXP_SCALE as f64) as i64;
        let target = (0.7 * EXP_SCALE as f64) as i64;

        let result1 = update_price(price, kappa, util, target);
        let result2 = update_price(price, kappa, util, target);

        assert_eq!(result1, result2);
    }
}
