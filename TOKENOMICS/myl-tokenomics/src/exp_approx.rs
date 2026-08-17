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
//!
//! **Die Tabelle ist eingefroren** (`exp_lut_table.rs`, erzeugt von
//! `tools/generate_exp_lut.py`). Sie wurde bis v0.2.3 zur Laufzeit mit
//! `f64::exp()` gebaut — das ist plattformabhängig: `f64::exp()` ist
//! nicht korrekt gerundet und unterscheidet sich zwischen
//! glibc-Versionen, musl, macOS-libm und Windows-CRT. Zwei Nodes auf
//! verschiedenen Betriebssystemen hätten damit verschiedene
//! Credit-Preise berechnet — genau der Konsensbruch, gegen den
//! Whitepaper Kap. 6.2 auf der Inferenzseite argumentiert. Das
//! Gegenmittel ist dasselbe wie in INTEGER_LLM: die Tabelle einmal
//! offline erzeugen, einfrieren und im Test gegen einen Hash prüfen.

use crate::exp_lut_table::EXP_LUT;

/// Anzahl der Stützstellen in der LUT.
const LUT_SIZE: usize = EXP_LUT.len();

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

/// Approximiert exp(x) ganzzahlig mit LUT und linearer Interpolation.
///
/// **Parameter:**
/// - `x_fixed`: Exponent als Fixed-Point i64 mit 16 Bit Nachkommastellen
///
/// **Returns:** exp(x) als i64 mit 32 Bit Nachkommastellen
///
/// **Genauigkeit:** relativer Fehler < 0,002 % im Bereich [-10, +10]
/// (gemessenes Maximum 0,00125 % bei x ≈ −9,06 — reiner
/// Interpolationsfehler zwischen 2048 Stützstellen; im Test
/// `interpolationsfehler_bleibt_unter_schranke` gegen unabhängig
/// gerechnete Referenzwerte belegt). Vor dem Einfrieren lag der Fehler
/// bei bis zu 0,97 % — nicht wegen der Auflösung, sondern wegen des
/// abgerundeten Schritts bei der Tabellenerzeugung.
///
/// **Determinismus:** Bitgleich auf allen Plattformen — reine
/// Ganzzahlarithmetik über einer eingefrorenen Tabelle, kein Gleitkomma
/// zur Laufzeit.
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
    let lut = &EXP_LUT;

    // Clamp auf gültigen Bereich
    let x_clamped = x_fixed.max(EXP_MIN).min(EXP_MAX);

    // Berechne Index und Interpolationsfaktor in Fixed-Point.
    //
    // Stützstelle i liegt bei x_i = EXP_MIN + i · range / (LUT_SIZE − 1).
    // Die eingefrorene Tabelle ist mit exakt dieser (nicht gerundeten)
    // Schrittweite erzeugt — die alte Laufzeitfassung rundete den Schritt
    // per Ganzzahldivision auf 640 ab und endete dadurch bei x = 9,990,
    // während hier bis x = 10,0 indiziert wird: ein systematischer Drift
    // von bis zu 0,97 % am oberen Rand.
    let range = EXP_MAX - EXP_MIN;
    // normalized ist in Fixed-Point mit EXP_SCALE
    let normalized = ((x_clamped - EXP_MIN) * ((LUT_SIZE as i64 - 1) * EXP_SCALE)) / range;
    let index = (normalized / EXP_SCALE) as usize;
    let frac = normalized % EXP_SCALE;

    // Linear interpolation zwischen lut[index] und lut[index+1]
    if index + 1 < LUT_SIZE {
        let y0 = lut[index];
        let y1 = lut[index + 1];
        // Interpolation: y = y0 + (y1 - y0) * frac / EXP_SCALE.
        // i128 für das Zwischenprodukt: diff erreicht am oberen Rand
        // ~4,3·10^10, frac bis 2^16 — das Produkt passt zwar noch in i64,
        // aber nur knapp; i128 nimmt die Frage aus dem Konsenspfad.
        let diff = (y1 - y0) as i128;
        y0 + ((diff * frac as i128) / EXP_SCALE as i128) as i64
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

    // ── Eingefrorene Tabelle (Fund A5) ──────────────────────────────
    //
    // Bis v0.2.3 wurde die LUT zur Laufzeit mit `f64::exp()` gebaut.
    // `f64::exp()` ist nicht korrekt gerundet und unterscheidet sich
    // zwischen libm-Implementierungen — zwei Nodes auf verschiedenen
    // Plattformen hätten verschiedene Credit-Preise berechnet. Die
    // folgenden Tests sichern die eingefrorene Tabelle ab.

    /// Die Tabelle darf sich nicht unbemerkt ändern — sie ist
    /// Konsens-Feld. Bei einer beabsichtigten Änderung muss dieser Hash
    /// bewusst mitgezogen werden (Governance, Kap. 10.3).
    #[test]
    fn eingefrorene_tabelle_hat_erwarteten_hash() {
        use myl_types::hash::Hash;

        let mut bytes = Vec::with_capacity(EXP_LUT.len() * 8);
        for v in EXP_LUT.iter() {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(
            Hash::sha256(&bytes).to_hex(),
            crate::exp_lut_table::EXP_LUT_SHA256,
            "Die eingefrorene exp-LUT wurde verändert. Das ist eine \
             konsensrelevante Änderung — Hash bewusst mitziehen."
        );
    }

    /// Golden Vectors: unabhängig mit 60-stelliger Dezimalarithmetik
    /// gerechnet (`tools/generate_exp_lut.py`), nicht aus dieser
    /// Implementierung abgeleitet.
    #[test]
    fn golden_vectors_exakt() {
        // (x als Q16, erwartetes exp(x) als Q32)
        const GOLDEN: &[(i64, i64)] = &[
            (-655360, 194991),          // x = -10.0 (unterer Rand)
            (-327680, 28939520),        // x = -5.0
            (-65536, 1580039711),       // x = -1.0
            (-32768, 2605056577),       // x = -0.5
            (0, 4295018546),            // x =  0.0
            (655, 4338160091),          // x ≈  0.01
            (32768, 7081277156),        // x =  0.5
            (65536, 11675001401),       // x =  1.0
            (131072, 31735996180),      // x =  2.0
            (327680, 637435378585),     // x =  5.0
            (654745, 93719493770510),   // x ≈  9.99
            (655360, 94602950235157),   // x = 10.0 (oberer Rand)
        ];

        for &(x, expected) in GOLDEN {
            assert_eq!(
                exp_approx(x),
                expected,
                "exp_approx({}) weicht vom Golden Vector ab",
                x
            );
        }
    }

    /// Der Interpolationsfehler muss unter 0,002 % bleiben. Referenz ist
    /// `f64::exp` — hier zulässig, weil es nur eine grobe Schranke prüft
    /// und nicht in den Konsenswert eingeht; die libm-Unterschiede
    /// liegen viele Größenordnungen unter der Schranke.
    #[test]
    fn interpolationsfehler_bleibt_unter_schranke() {
        let mut worst = 0.0f64;
        let mut worst_x = 0i64;

        let mut x = EXP_MIN;
        while x <= EXP_MAX {
            let got = exp_approx(x) as f64 / RESULT_SCALE as f64;
            let want = (x as f64 / EXP_SCALE as f64).exp();
            let rel = ((got - want) / want).abs();
            if rel > worst {
                worst = rel;
                worst_x = x;
            }
            x += 61; // ~21 500 Stichproben, teilerfremd zur Schrittweite
        }

        assert!(
            worst < 2e-5,
            "relativer Fehler {:.6} % bei x = {} überschreitet 0,002 %",
            worst * 100.0,
            worst_x as f64 / EXP_SCALE as f64
        );
    }

    /// Regression zum Step-Bug: Die alte Fassung rundete die
    /// Schrittweite auf 640 ab, wodurch die Tabelle bei x = 9,990 endete,
    /// der Interpolator aber bis x = 10,0 indizierte — am oberen Rand
    /// entstand dadurch ein Fehler von 0,97 %.
    #[test]
    fn oberer_rand_hat_keinen_systematischen_drift() {
        let got = exp_approx(EXP_MAX) as f64 / RESULT_SCALE as f64;
        let want = 10.0f64.exp();
        let rel = ((got - want) / want).abs();
        assert!(
            rel < 1e-6,
            "exp(10) hat {:.5} % Abweichung — Step-Bug zurück?",
            rel * 100.0
        );
    }

    /// Monotonie: exp ist streng monoton steigend. Ein Bruch würde auf
    /// einen Index- oder Interpolationsfehler hindeuten.
    #[test]
    fn approximation_ist_monoton() {
        let mut prev = exp_approx(EXP_MIN);
        let mut x = EXP_MIN + 97;
        while x <= EXP_MAX {
            let cur = exp_approx(x);
            assert!(cur >= prev, "Monotonie verletzt bei x = {}", x);
            prev = cur;
            x += 97;
        }
    }

    /// Außerhalb des Bereichs wird geklemmt, nicht extrapoliert.
    #[test]
    fn ausserhalb_des_bereichs_wird_geklemmt() {
        assert_eq!(exp_approx(EXP_MIN - 1), exp_approx(EXP_MIN));
        assert_eq!(exp_approx(EXP_MIN - 1_000_000), exp_approx(EXP_MIN));
        assert_eq!(exp_approx(EXP_MAX + 1), exp_approx(EXP_MAX));
        assert_eq!(exp_approx(EXP_MAX + 1_000_000), exp_approx(EXP_MAX));
        assert_eq!(exp_approx(i64::MIN), exp_approx(EXP_MIN));
        assert_eq!(exp_approx(i64::MAX), exp_approx(EXP_MAX));
    }

    /// Die Tabelle hat die dokumentierte Form.
    #[test]
    fn tabelle_hat_erwartete_gestalt() {
        assert_eq!(EXP_LUT.len(), 2048);
        assert!(EXP_LUT[0] > 0, "exp ist überall positiv");
        assert!(EXP_LUT.windows(2).all(|w| w[0] < w[1]), "streng steigend");
    }
}
