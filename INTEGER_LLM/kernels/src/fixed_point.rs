//! Fixed-Point-Helfer
//!
//! Alle Operationen sind deterministisch und plattformunabhaengig.
//!
//! ## ⚑ Fund 75: Die Schiebeweiten haben Grenzen, und sie standen nirgends
//!
//! Die Rundungsfunktionen dieses Moduls sind **nicht** fuer jede
//! Schiebeweite total. `rshift_round` rechnet `(1 << shift) - 1`, und
//! dieser Ausdruck laeuft ueber, sobald `1 << shift` das Vorzeichenbit
//! trifft. Die Grenzen sind je Typ verschieden und liegen zwei bis drei
//! Bit unter der Typbreite:
//!
//! | Funktion | zulaessige Schiebeweite | was darueber passiert |
//! |---|---|---|
//! | `rshift_round` | `0..=30` | `(1i32 << 31) - 1` laeuft ueber |
//! | `rshift_round_i64` | `0..=62` | dasselbe eine Typbreite hoeher |
//! | `rshift_round_i128` | `0..=126` | dasselbe noch eine hoeher |
//! | `rescale` | Abstand `-31..=30` | beide Zweige, siehe dort |
//! | `rescale_i64` | Abstand `-63..=62` | beide Zweige, siehe dort |
//!
//! **Bis zum 2026-08-28 stand keine dieser Grenzen irgendwo**, und
//! keine wurde geprueft. Kein Aufrufer verletzte sie: Die im Projekt
//! vorkommenden `frac_bits` liegen zwischen 3 und 16. Es war also kein
//! Fehler, sondern ein Vertrag, den niemand aufgeschrieben hatte.
//!
//! ⚑ **Warum das trotzdem zaehlt: Der Fehlerfall ist im ausgelieferten
//! Bauprofil still.** Im Debug-Bau bricht die Ueberlaufpruefung laut ab
//! (`attempt to subtract with overflow`). Im Release-Bau gibt es diese
//! Pruefung nicht: `rshift_round(1000, 32)` liefert dort `1001` statt
//! `0`, weil Rust die Schiebeweite auf fuenf Bit maskiert (`32` wirkt
//! wie `0`) und die Rundung anschliessend aufaddiert. Ein falscher Wert
//! im Ganzzahlpfad ist ein Konsensbruch, und er faellt nirgends auf.
//!
//! **Was daraus folgt und hier steht:** je Funktion eine benannte
//! Vorbedingung, ein `debug_assert!` an der Stelle, und je Grenze ein
//! Test, der sie ueberschreitet. Die Pruefung ist bewusst `debug_assert`
//! und nicht `assert`: Diese Funktionen laufen je Element in der
//! innersten Schleife, und die Testlaeufe des Projekts sind
//! Debug-Laeufe.

/// Clamp auf i8-Bereich.
#[inline(always)]
pub fn clamp_i8(x: i32) -> i8 {
    if x < -128 { -128 }
    else if x > 127 { 127 }
    else { x as i8 }
}

/// Clamp auf i16-Bereich.
#[inline(always)]
pub fn clamp_i16(x: i32) -> i16 {
    if x < -32768 { -32768 }
    else if x > 32767 { 32767 }
    else { x as i16 }
}

/// Clamp auf i16-Bereich aus i64 (fuer Akkumulator-Zwischenwerte).
#[inline(always)]
pub fn clamp_i16_from_i64(x: i64) -> i16 {
    if x < -32768 { -32768 }
    else if x > 32767 { 32767 }
    else { x as i16 }
}

/// Clamp auf i32-Bereich (nützlich fuer i64-Zwischenwerte).
#[inline(always)]
pub fn clamp_i32(x: i64) -> i32 {
    if x < i32::MIN as i64 { i32::MIN }
    else if x > i32::MAX as i64 { i32::MAX }
    else { x as i32 }
}

/// Round-to-nearest-even Rechts-Shift.
///
/// **Vorbedingung: `shift <= 30`** (Fund 75). Bei `shift == 31` laeuft
/// `(1i32 << 31) - 1` ueber, ab `32` der Shift selbst. Im Release-Bau
/// bricht nichts ab, das Ergebnis ist dann still falsch.
#[inline(always)]
pub fn rshift_round(value: i32, shift: u8) -> i32 {
    debug_assert!(
        shift <= 30,
        "rshift_round: shift {} ueber der Grenze 30 (Fund 75)",
        shift
    );
    if shift == 0 {
        return value;
    }
    let mask = (1i32 << shift) - 1;
    let half = 1i32 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Round-to-nearest-even Rechts-Shift (i64, fuer Zwischenprodukte).
///
/// **Vorbedingung: `shift <= 62`** (Fund 75), dieselbe Grenze wie bei
/// [`rshift_round`], eine Typbreite hoeher.
#[inline(always)]
pub fn rshift_round_i64(value: i64, shift: u8) -> i64 {
    debug_assert!(
        shift <= 62,
        "rshift_round_i64: shift {} ueber der Grenze 62 (Fund 75)",
        shift
    );
    if shift == 0 {
        return value;
    }
    let mask = (1i64 << shift) - 1;
    let half = 1i64 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Q15-Reziproke der Wurzel: `round(2^15 / sqrt(head_dim))`.
///
/// **Fund 19 (2026-08-18).** Die Attention skaliert die Scores mit
/// `1/sqrt(head_dim)` (Fund 17). Umgesetzt war das als reiner Rechtsshift um
/// `log2(head_dim) / 2` — Ganzzahldivision. Das stimmt nur, wenn
/// `log2(head_dim)` **gerade** ist:
///
/// | head_dim | Shift | angewandt | korrekt   |
/// |----------|-------|-----------|-----------|
/// | 64  (2^6)| 3     | 0,125000  | 0,125000  |
/// | 128 (2^7)| 3     | 0,125000  | 0,088388  | <- Faktor sqrt(2) zu gross
/// | 256 (2^8)| 4     | 0,062500  | 0,062500  |
///
/// Qwen2.5-0.5B hat head_dim 64 und war deshalb zufaellig richtig; ab 1,5B
/// ist 128 der Normalfall. Bei 7B waren die Scores durchgaengig um sqrt(2)
/// zu gross und die Softmax entsprechend zu scharf — in jedem Kopf, jeder
/// Ebene, jeder Position.
///
/// Statt eines Shifts wird jetzt mit dieser Q15-Konstanten multipliziert.
/// **Fuer gerade Zweierpotenzen ist das Ergebnis bitgleich zum bisherigen
/// Verhalten**, weil der Multiplikator dann selbst eine Zweierpotenz ist
/// (head_dim 64 -> 4096 = 2^12) und `rshift_round_i64` unter einem
/// Zweierpotenz-Faktor dieselbe Rundung samt Tie-Break liefert. Die
/// Artefakte bestehender Modelle bleiben damit gueltig.
///
/// Vollstaendig ganzzahlig: keine `f64::sqrt`, sondern
/// `isqrt_round(2^30 / head_dim)`. Fuer head_dim 64 exakt 4096, fuer 128
/// gerundet 2896 (relativer Fehler 1,1e-4).
#[inline]
pub fn inv_sqrt_q15(head_dim: usize) -> i64 {
    assert!(head_dim > 0, "inv_sqrt_q15: head_dim muss positiv sein");
    // (2^15)^2 / head_dim = 2^30 / head_dim; die Wurzel daraus ist
    // 2^15 / sqrt(head_dim).
    isqrt_round((1u64 << 30) / head_dim as u64) as i64
}

/// Ganzzahlige Quadratwurzel mit Rundung zur naechsten ganzen Zahl.
///
/// Newton-Iteration auf u64, danach eine Rundungskorrektur: von den beiden
/// Kandidaten `r` und `r + 1` gewinnt der, dessen Quadrat naeher an `n`
/// liegt. Deterministisch und plattformunabhaengig — anders als
/// `(n as f64).sqrt().round()`, das je nach libm abweichen kann und damit
/// den Konsens brechen wuerde.
#[inline]
pub fn isqrt_round(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    // Abgerundete Ganzzahlwurzel per Newton. Die Abbruchbedingung muss
    // MONOTONIE pruefen, nicht Gleichheit zum Vorgaenger: Newton oszilliert
    // fuer manche n dauerhaft zwischen zwei benachbarten Werten (etwa n=8:
    // 2, 3, 2, 3, ...). Ein Vergleich "r != vorher" laeuft dann ewig.
    let mut r = n;
    let mut naechst = r.div_ceil(2);
    while naechst < r {
        r = naechst;
        naechst = (r + n / r) / 2;
    }
    // r ist jetzt floor(sqrt(n)); die beiden Schleifen fangen Randfaelle ab.
    while r > 0 && r > n / r {
        r -= 1;
    }
    while (r + 1) <= n / (r + 1) {
        r += 1;
    }
    // Runden: liegt n naeher an (r+1)^2 als an r^2?
    let unten = n - r * r;
    let oben = (r + 1) * (r + 1) - n;
    if oben < unten {
        r + 1
    } else {
        r
    }
}

/// Round-to-nearest-even Rechts-Shift in i128.
///
/// Gleiche Semantik wie `rshift_round_i64`, nur mit breiterem Typ: seit
/// Fund 24 laeuft die RMSNorm-Ausgabe ueber i128, weil der dortige
/// Linksshift in i64 haette ueberlaufen koennen (spec:
/// overflow.behavior = "explicit_clamp_only", wrap = false).
///
/// **Vorbedingung: `shift <= 126`** (Fund 75), dieselbe Grenze wie bei
/// [`rshift_round`], zwei Typbreiten hoeher.
#[inline(always)]
pub fn rshift_round_i128(value: i128, shift: u32) -> i128 {
    debug_assert!(
        shift <= 126,
        "rshift_round_i128: shift {} ueber der Grenze 126 (Fund 75)",
        shift
    );
    if shift == 0 {
        return value;
    }
    let mask = (1i128 << shift) - 1;
    let half = 1i128 << (shift - 1);
    let quotient = value >> shift;
    let remainder = value & mask;

    if remainder > half || (remainder == half && (quotient & 1) != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Rescale: von in_frac Bits nach out_frac Bits.
///
/// **Drei Vorbedingungen, alle drei ungeprueft bis Fund 75:**
///
/// 1. **`in_frac <= 127` und `out_frac <= 127`.** Die Differenz wird in
///    `i8` gerechnet; ein `u8`-Wert ab 128 wird dabei negativ, und die
///    Funktion schiebt dann in die falsche Richtung. Das ist der
///    stillste der drei Faelle, denn er bricht nicht einmal im
///    Debug-Bau ab.
/// 2. **`in_frac - out_frac <= 30`** im Rechtsschiebe-Zweig, geerbt von
///    [`rshift_round`].
/// 3. **`out_frac - in_frac <= 31`** im Linksschiebe-Zweig, sonst ist
///    die Schiebeweite fuer `i32` zu gross.
///
/// **Nicht geprueft wird der Wertueberlauf des Linksschiebens selbst:**
/// `acc << n` kann Bits herausschieben, ohne dass etwas abbricht. Das
/// widerspricht `overflow.behavior = "explicit_clamp_only"` und liegt in
/// der Verantwortung des Aufrufers, der weiss, wie gross `acc` werden
/// kann. Hier steht es, damit die Luecke benannt ist.
#[inline(always)]
pub fn rescale(acc: i32, in_frac: u8, out_frac: u8) -> i32 {
    debug_assert!(
        in_frac <= 127 && out_frac <= 127,
        "rescale: in_frac {} / out_frac {} ab 128 dreht die Differenz das Vorzeichen (Fund 75)",
        in_frac,
        out_frac
    );
    debug_assert!(
        (in_frac as i16 - out_frac as i16) <= 30 && (out_frac as i16 - in_frac as i16) <= 31,
        "rescale: Abstand {} ausserhalb -31..=30 (Fund 75)",
        in_frac as i16 - out_frac as i16
    );
    let shift = in_frac as i8 - out_frac as i8;
    if shift >= 0 {
        rshift_round(acc, shift as u8)
    } else {
        acc << (-shift)
    }
}

/// Rescale fuer i64-Zwischenprodukte (z. B. RMSNorm-Tripelprodukt).
///
/// **Dieselben drei Vorbedingungen wie [`rescale`]**, mit den Grenzen
/// des breiteren Typs: `in_frac <= 127`, `out_frac <= 127`, und der
/// Abstand `in_frac - out_frac` liegt in `-63..=62`.
#[inline(always)]
pub fn rescale_i64(acc: i64, in_frac: u8, out_frac: u8) -> i64 {
    debug_assert!(
        in_frac <= 127 && out_frac <= 127,
        "rescale_i64: in_frac {} / out_frac {} ab 128 dreht die Differenz das Vorzeichen (Fund 75)",
        in_frac,
        out_frac
    );
    debug_assert!(
        (in_frac as i16 - out_frac as i16) <= 62 && (out_frac as i16 - in_frac as i16) <= 63,
        "rescale_i64: Abstand {} ausserhalb -63..=62 (Fund 75)",
        in_frac as i16 - out_frac as i16
    );
    let shift = in_frac as i8 - out_frac as i8;
    if shift >= 0 {
        rshift_round_i64(acc, shift as u8)
    } else {
        acc << (-shift)
    }
}

/// Overflow-sichere Multiplikation i8 * i8 -> i32.
#[inline(always)]
pub fn mul_i8_i32(a: i8, b: i8) -> i32 {
    (a as i32).wrapping_mul(b as i32)
}

/// Overflow-sichere Multiplikation i16 * i16 -> i64.
#[inline(always)]
pub fn mul_i16_i64(a: i16, b: i16) -> i64 {
    (a as i64).wrapping_mul(b as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rshift_round_basic() {
        assert_eq!(rshift_round(4, 1), 2);
        assert_eq!(rshift_round(3, 1), 2);
        assert_eq!(rshift_round(5, 1), 2);
        assert_eq!(rshift_round(7, 1), 4);
    }

    #[test]
    fn test_rshift_round_negative() {
        assert_eq!(rshift_round(-5, 1), -2);
        assert_eq!(rshift_round(-4, 1), -2);
        assert_eq!(rshift_round(-3, 1), -2);
    }

    #[test]
    fn test_rescale() {
        let acc = 127i32 * 127i32;
        assert_eq!(rescale(acc, 13, 6), 126);
    }

    #[test]
    fn test_clamp_i8() {
        assert_eq!(clamp_i8(200), 127);
        assert_eq!(clamp_i8(-200), -128);
        assert_eq!(clamp_i8(50), 50);
    }

    /// Fixierter Divisionssemantik-Vektor (Punkt 12.24).
    ///
    /// Division durch Zweierpotenzen ist als arithmetischer Rechtsshift
    /// mit Round-to-nearest-even festgelegt (spec.json:
    /// `shift_semantics = arithmetic_right_shift`, `rounding.default =
    /// round_to_nearest_even`). Für negative Operanden unterscheiden sich
    /// arithmetischer Rechtsshift, Trunkation zur Null und Floor-Division
    /// — ohne diesen fixierten Vektor würde die Abweichung erst in
    /// Modellausgaben sichtbar. Dieser Vektor ist die maßgebliche
    /// Referenz; CI failt, wenn ein Backend davon abweicht.
    #[test]
    fn division_semantics_vector() {
        // (Wert, Shift, erwartetes Ergebnis) — Round-to-nearest-even,
        // arithmetischer Rechtsshift.
        let vector: &[(i32, u8, i32)] = &[
            // positive Werte
            (0, 3, 0),
            (1, 1, 0),        // 0,5 -> 0 (quotient 0 gerade)
            (2, 1, 1),
            (3, 1, 2),        // 1,5 -> 2 (quotient 1 ungerade)
            (4, 1, 2),
            (5, 1, 2),        // 2,5 -> 2 (quotient 2 gerade)
            (6, 1, 3),
            (7, 1, 4),        // 3,5 -> 4 (quotient 3 ungerade)
            (8, 3, 1),
            (12, 3, 2),       // 1,5 -> 2
            (1_000_000, 8, 3906), // 3906,25 -> 3906
            // negative Werte
            (-1, 1, 0),       // -0,5 -> 0 (round-to-even)
            (-2, 1, -1),
            (-3, 1, -2),      // -1,5 -> -2
            (-4, 1, -2),
            (-5, 1, -2),      // -2,5 -> -2 (round-to-even)
            (-6, 1, -3),
            (-7, 1, -4),      // -3,5 -> -4
            (-8, 3, -1),
            (-12, 3, -2),     // -1,5 -> -2 (round-to-even)
            (-1_000_000, 8, -3906), // -3906,25 -> -3906
        ];
        for &(value, shift, expected) in vector {
            assert_eq!(
                rshift_round(value, shift),
                expected,
                "Abweichung bei ({} >> {}): erwartet {}, erhalten {}",
                value,
                shift,
                expected,
                rshift_round(value, shift)
            );
            // i64-Variante muss identisch sein.
            assert_eq!(
                rshift_round_i64(value as i64, shift),
                expected as i64,
                "i64-Abweichung bei ({} >> {})",
                value,
                shift
            );
        }
    }

    /// Fixierter Überlauf-/Sättigungsvektor (Punkt 12.25).
    ///
    /// Überlaufverhalten ist Sättigung (spec.json: `overflow =
    /// explicit_clamp_only`, kein Wrap). Dieser Vektor fixiert die
    /// Sättigungsgrenzen; CI failt bei Abweichung.
    #[test]
    fn overflow_saturation_vector() {
        // i8-Sättigung [-128, 127]
        assert_eq!(clamp_i8(i32::MAX), 127);
        assert_eq!(clamp_i8(i32::MIN), -128);
        assert_eq!(clamp_i8(127), 127);
        assert_eq!(clamp_i8(128), 127);
        assert_eq!(clamp_i8(-128), -128);
        assert_eq!(clamp_i8(-129), -128);
        // i16-Sättigung [-32768, 32767]
        assert_eq!(clamp_i16(i32::MAX), 32767);
        assert_eq!(clamp_i16(i32::MIN), -32768);
        assert_eq!(clamp_i16(32767), 32767);
        assert_eq!(clamp_i16(32768), 32767);
        assert_eq!(clamp_i16(-32768), -32768);
        assert_eq!(clamp_i16(-32769), -32768);
        // i16-aus-i64-Sättigung
        assert_eq!(clamp_i16_from_i64(i64::MAX), 32767);
        assert_eq!(clamp_i16_from_i64(i64::MIN), -32768);
        assert_eq!(clamp_i16_from_i64(32768), 32767);
        assert_eq!(clamp_i16_from_i64(-32769), -32768);
        // i32-Sättigung
        assert_eq!(clamp_i32(i64::MAX), i32::MAX);
        assert_eq!(clamp_i32(i64::MIN), i32::MIN);
        assert_eq!(clamp_i32(i32::MAX as i64 + 1), i32::MAX);
        assert_eq!(clamp_i32(i32::MIN as i64 - 1), i32::MIN);
        // Multiplikation ohne Überlauf (wrapping in i32/i64-Zielbreite).
        assert_eq!(mul_i8_i32(127, 127), 16129);
        assert_eq!(mul_i8_i32(-128, -128), 16384);
        assert_eq!(mul_i16_i64(32767, 32767), 1_073_676_289);
        assert_eq!(mul_i16_i64(-32768, -32768), 1_073_741_824);
    }

    #[test]
    fn test_inv_sqrt_q15_gerade_zweierpotenzen_sind_exakt() {
        // Fuer gerade log2(head_dim) ist 1/sqrt(head_dim) selbst eine
        // Zweierpotenz. Der Multiplikator muss dann exakt 2^(15 - log2/2)
        // sein - nur so bleibt das Ergebnis bitgleich zum frueheren
        // Shift-Verhalten und bestehende Artefakte gueltig.
        assert_eq!(inv_sqrt_q15(4), 1 << 14);    // 1/2
        assert_eq!(inv_sqrt_q15(16), 1 << 13);   // 1/4
        assert_eq!(inv_sqrt_q15(64), 1 << 12);   // 1/8   <- Qwen2.5-0.5B
        assert_eq!(inv_sqrt_q15(256), 1 << 11);  // 1/16
    }

    #[test]
    fn test_inv_sqrt_q15_ungerade_zweierpotenzen() {
        // **Fund 19.** Hier lag der Fehler: der alte Shift log2(hd)/2 war
        // Ganzzahldivision und ergab fuer head_dim 128 den Faktor 2^-3 =
        // 0,125 statt 1/sqrt(128) = 0,0884 - um sqrt(2) zu gross. head_dim
        // 128 ist der Normalfall ab Qwen2.5-1.5B; abgedeckt war er von
        // keinem Test, weil 0.5B mit head_dim 64 zufaellig richtig lag.
        assert_eq!(inv_sqrt_q15(128), 2896);  // round(32768 / sqrt(128))
        assert_eq!(inv_sqrt_q15(512), 1448);
        assert_eq!(inv_sqrt_q15(2), 23170);   // round(32768 / sqrt(2))

        // Der alte Shift-Wert waere 2^12 = 4096 gewesen - Faktor 1,414 zu gross.
        assert!(inv_sqrt_q15(128) < 4096);
        let verhaeltnis = 4096_f64 / inv_sqrt_q15(128) as f64;
        assert!((verhaeltnis - std::f64::consts::SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn test_q15_multiplikation_ist_bitgleich_zum_shift_bei_zweierpotenz() {
        // Die Zusicherung, auf der die Gueltigkeit bestehender Artefakte
        // beruht: ist der Multiplikator eine Zweierpotenz 2^m, liefert
        // rshift_round_i64(x * 2^m, n + m) exakt dasselbe wie
        // rshift_round_i64(x, n) - einschliesslich des
        // Round-to-nearest-even-Tie-Breaks.
        let m = 12u8; // inv_sqrt_q15(64) = 2^12
        for x in [-100_003i64, -8, -7, -1, 0, 1, 7, 8, 9, 100_003, 1 << 30] {
            for n in [1u8, 3, 5, 8, 16] {
                let ueber_shift = rshift_round_i64(x, n);
                let ueber_mult = rshift_round_i64(x * (1i64 << m), n + m);
                assert_eq!(ueber_shift, ueber_mult, "x={} n={}", x, n);
            }
        }
    }

    #[test]
    fn test_isqrt_round_deterministisch() {
        assert_eq!(isqrt_round(0), 0);
        assert_eq!(isqrt_round(1), 1);
        assert_eq!(isqrt_round(2), 1);   // 1.414 -> 1
        assert_eq!(isqrt_round(3), 2);   // 1.732 -> 2
        assert_eq!(isqrt_round(4), 2);
        assert_eq!(isqrt_round(8_388_608), 2896);  // 2^30 / 128
        assert_eq!(isqrt_round(16_777_216), 4096); // 2^30 / 64, exakt
        // Gegen die Gleitkomma-Referenz, aber ohne sie im Rechenpfad.
        for n in [5u64, 99, 1000, 123_456, 1 << 40] {
            let erwartet = (n as f64).sqrt().round() as u64;
            assert_eq!(isqrt_round(n), erwartet, "n={}", n);
        }
    }

    /// ⚑ Gegenproben zu Fund 75: die Schiebeweiten-Grenzen.
    ///
    /// Je Grenze zwei Tests, und der Aufbau ist Absicht: einer zeigt,
    /// dass der letzte zulaessige Wert **durchgeht**, der andere, dass
    /// der erste unzulaessige **abbricht**. Nur eine Richtung zu pruefen
    /// hiesse, eine zu enge Schranke nicht zu bemerken.
    mod fund_75 {
        use super::*;

        #[test]
        fn die_letzte_zulaessige_schiebeweite_geht_durch() {
            assert_eq!(rshift_round(1000, 30), 0);
            assert_eq!(rshift_round_i64(1000, 62), 0);
            assert_eq!(rshift_round_i128(1000, 126), 0);
            // rescale an beiden Enden des zulaessigen Abstands.
            assert_eq!(rescale(1024, 30, 0), 0);
            assert_eq!(rescale(1, 0, 31), 1i32 << 31 >> 31 << 31); // = i32::MIN, Linksschieber
            assert_eq!(rescale_i64(1024, 62, 0), 0);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "ueber der Grenze 30")]
        fn rshift_round_bei_31_bricht_ab() {
            let _ = rshift_round(1000, 31);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "ueber der Grenze 62")]
        fn rshift_round_i64_bei_63_bricht_ab() {
            let _ = rshift_round_i64(1000, 63);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "ueber der Grenze 126")]
        fn rshift_round_i128_bei_127_bricht_ab() {
            let _ = rshift_round_i128(1000, 127);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "Abstand 31 ausserhalb")]
        fn rescale_rechts_ueber_der_grenze_bricht_ab() {
            let _ = rescale(1024, 31, 0);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "Abstand -32 ausserhalb")]
        fn rescale_links_ueber_der_grenze_bricht_ab() {
            let _ = rescale(1, 0, 32);
        }

        /// ⚑ Der stillste der drei Faelle: Ein `u8` ab 128 wird beim
        /// Weg nach `i8` negativ, und `rescale` schiebt dann in die
        /// **falsche Richtung**, ohne dass irgendetwas abbricht.
        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "ab 128 dreht die Differenz das Vorzeichen")]
        fn rescale_mit_frac_bits_ab_128_bricht_ab() {
            let _ = rescale(1024, 128, 0);
        }

        #[test]
        #[cfg(debug_assertions)]
        #[should_panic(expected = "ab 128 dreht die Differenz das Vorzeichen")]
        fn rescale_i64_mit_frac_bits_ab_128_bricht_ab() {
            let _ = rescale_i64(1024, 0, 200);
        }

        /// Die Rundungsregel selbst, ueber den ganzen Bereich statt an
        /// vier getippten Paaren: kaufmaennisch zur **geraden** Zahl,
        /// auch fuer negative Werte. Die Referenz wird in `i64`
        /// gerechnet, damit sie nicht denselben Weg nimmt wie das
        /// Gemessene.
        #[test]
        fn rshift_round_rundet_zur_geraden_zahl_auch_negativ() {
            for shift in 1u8..=8 {
                let teiler = 1i64 << shift;
                for value in -600i32..=600 {
                    let v = value as i64;
                    // Referenz: round-half-to-even von v / 2^shift.
                    let unten = v.div_euclid(teiler);
                    let rest = v.rem_euclid(teiler);
                    let doppelt = rest * 2;
                    let erwartet = if doppelt > teiler || (doppelt == teiler && unten % 2 != 0) {
                        unten + 1
                    } else {
                        unten
                    };
                    assert_eq!(
                        rshift_round(value, shift) as i64,
                        erwartet,
                        "value={} shift={}",
                        value,
                        shift
                    );
                }
            }
        }
    }
}
