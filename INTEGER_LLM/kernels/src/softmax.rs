//! Integer-Softmax-Approximation via exp-LUT

/// exp(-x) LUT-Lookup. `one` ist der Wert von exp(0) **in der Skala der
/// LUT** (`softmax.exp_lut_frac_bits`) — nicht in der Ausgangsskala der
/// Wahrscheinlichkeiten (`prob_frac_bits`). Beide waren bis theta_v
/// 0.15.0 zufaellig gleich (8); der Aufrufer uebergab deshalb
/// `1 << prob_frac_bits` und es fiel nicht auf. Sobald die beiden Skalen
/// auseinanderlaufen, bekaeme ausgerechnet das Maximum — der Eintrag mit
/// dem groessten Gewicht — einen um den Skalenfaktor falschen Wert.
/// `softmax_int` uebergibt daher `exp_lut[0]`.
#[inline]
pub fn exp_lut_lookup(x: i32, exp_lut: &[i16], lut_shift: u8, one: i32) -> i32 {
    if x <= 0 {
        return one;
    }
    let idx = (x as u32) >> lut_shift;
    if idx as usize >= exp_lut.len() {
        return 0;
    }
    exp_lut[idx as usize] as i32
}

/// Integer-Softmax.
pub fn softmax_int(logits: &[i32], exp_lut: &[i16], lut_shift: u8, frac_bits: u8) -> Vec<i32> {
    let one = 1i32 << frac_bits;
    // exp(0) in LUT-Skala: der erste Eintrag ist per Konstruktion
    // round(exp(0) * 2^exp_lut_frac_bits). Kein Rueckgriff auf `one`,
    // das die Ausgangsskala traegt (siehe exp_lut_lookup).
    let lut_one = *exp_lut.first().unwrap_or(&1) as i32;
    let m = *logits.iter().max().unwrap_or(&0);

    let mut exps = Vec::with_capacity(logits.len());
    for z in logits {
        // saturating_sub: maskierte Positionen (i32::MIN) wuerden m - z
        // ueberlaufen lassen; die Saettigung liefert einen grossen Diff-Wert
        // und damit exp ~ 0 (Masken-Verhalten korrekt).
        let diff = m.saturating_sub(*z);
        exps.push(exp_lut_lookup(diff, exp_lut, lut_shift, lut_one));
    }

    let s: i32 = exps.iter().sum();
    if s == 0 {
        let base = one / exps.len() as i32;
        let rem = one - base * exps.len() as i32;
        return (0..exps.len())
            .map(|i| base + if i < rem as usize { 1 } else { 0 })
            .collect();
    }

    exps.iter()
        .map(|e| {
            let num = e * one;
            let q = num / s;
            let r = num % s;
            let twice = r.abs() * 2;
            let den_abs = s.abs();
            if twice > den_abs || (twice == den_abs && (q & 1) != 0) {
                if (num > 0) == (s > 0) { q + 1 } else { q - 1 }
            } else {
                q
            }
        })
        .collect()
}

/// Softmax über ein **Vokabular**: i64-Zwischenwerte, freie
/// Ausgangsskala, und die Umrechnung der Logitskala steht im Argument
/// statt beim Aufrufer.
///
/// # ⚑ Warum [`softmax_int`] das nicht kann (Fund 177)
///
/// `softmax_int` ist auf **Aufmerksamkeitspositionen** ausgelegt, und
/// theta_v 0.16.0 rechnet seine Überlaufschranke genau dafür aus:
/// „Summe der Exponentiale <= 2048 * 2^14". Über ein Vokabular gilt
/// keine der drei Annahmen mehr:
///
/// | | `softmax_int` | hier |
/// |---|---|---|
/// | Summe der Exponentiale | `i32`, Schranke für 2048 Einträge | `i64` |
/// | `e * one` | `i32`, also `frac_bits <= 16` | `i64` |
/// | Skala der Eingabe | `lut_shift` beim Aufrufer | Argument, mit Prüfung |
///
/// **Gemessen am 2026-09-04** an Qwen2.5-0.5B, 151.936 Wörter, mit
/// `frac_bits = 14`: Die Wahrscheinlichkeiten summierten sich auf
/// **1879 statt 16384**, 150.064 Wörter waren exakt null, und der
/// grösste Eintrag war 2. Das ist derselbe Defekt, den theta_v 0.16.0
/// für lange Kontexte beschreibt, nur eine Grössenordnung schlimmer.
///
/// ⚑ **Und es ist kein Rundungsfehler, sondern ein Boden.** Wenn der
/// typische Wert unter einer halben Einheit liegt, rundet nicht die
/// Hälfte der Einträge hoch und die Hälfte runter: **alle** fallen auf
/// null. Deshalb hilft hier auch die Restkorrektur aus
/// [`crate::moe::route_top_k`] nicht, die den Fehlbetrag dem grössten
/// Gewicht zuschlägt; sie ist für symmetrisches Runden über wenige
/// Einträge gedacht.
///
/// # ⚑ Die Regel steht im Typ
///
/// Daher die Zusicherung `2^frac_bits >= n`: unterhalb davon liegt der
/// **Mittelwert** der Verteilung unter einer Einheit, und der Softmax
/// gibt garantiert Unsinn zurück. Das ist der Boden, nicht die
/// Empfehlung. Wer einen Gradienten daraus rechnet, will acht Bit mehr,
/// damit der typische Eintrag nicht an der Rundung hängt; der
/// Trainingslauf nimmt `frac_bits = 24` gegen `n = 151.936`, also gut
/// 110 Einheiten je Wort im Mittel.
///
/// # Argumente
///
/// - `logits`, `logit_frac`: die Logits und ihre Skala
/// - `exp_lut`, `exp_input_frac`: die exp-Tabelle und die Skala, die
///   **ihr Index** meint
/// - `frac_bits`: Skala der ausgegebenen Wahrscheinlichkeiten
///
/// ⚑ **`logit_frac >= exp_input_frac` ist eine Zusicherung und keine
/// Sättigung.** Beim Aufrufer stand vorher ein `saturating_sub`, und
/// genau das verdeckte den Fehler: Die Logits der Laufzeit liegen auf
/// `logit_frac_bits = 6` („nur fuer Sampling/Argmax (skaleninvariant)"),
/// die Tabelle will 8, und die Sättigung machte daraus eine
/// Verschiebung um 0. Der Exponent wurde damit viermal zu flach
/// gelesen, ohne dass irgendetwas scheiterte. Wer den Softmax über ein
/// Vokabular braucht, muss den Kopf um feinere Logits bitten.
pub fn softmax_ueber_vokabular(
    logits: &[i32],
    logit_frac: u8,
    exp_lut: &[i16],
    exp_input_frac: u8,
    frac_bits: u8,
) -> Vec<i32> {
    assert!(
        logit_frac >= exp_input_frac,
        "softmax_ueber_vokabular: Logitskala {logit_frac} ist gröber als die Tabellenskala \
         {exp_input_frac}; der Kopf muss feinere Logits liefern"
    );
    assert!(frac_bits < 31, "softmax_ueber_vokabular: frac_bits {frac_bits} sprengt i32");
    let n = logits.len();
    assert!(
        n == 0 || (1u64 << frac_bits) >= n as u64,
        "softmax_ueber_vokabular: 2^{frac_bits} < {n} Einträge; der Mittelwert der Verteilung \
         läge unter einer Einheit und jeder Eintrag fiele auf null"
    );

    let schieben = logit_frac - exp_input_frac;
    // exp(0) in Tabellenskala, wie in exp_lut_lookup: nicht `eins`.
    let lut_eins = i64::from(*exp_lut.first().unwrap_or(&1));
    let m = i64::from(*logits.iter().max().unwrap_or(&0));

    let mut exps: Vec<i64> = Vec::with_capacity(n);
    let mut s: i64 = 0;
    for z in logits {
        // Wie in softmax_int: maskierte Positionen (i32::MIN) sollen
        // einen grossen Abstand ergeben, nicht überlaufen. In i64 kann
        // die Differenz zweier i32 gar nicht überlaufen.
        let diff = m - i64::from(*z);
        let e = if diff <= 0 {
            lut_eins
        } else {
            let idx = (diff >> schieben) as u64;
            if idx >= exp_lut.len() as u64 { 0 } else { i64::from(exp_lut[idx as usize]) }
        };
        exps.push(e);
        s += e;
    }

    let eins = 1i64 << frac_bits;
    if s == 0 {
        // Derselbe Rückfall wie in softmax_int: eine Gleichverteilung,
        // die sich exakt auf `eins` summiert.
        let basis = eins / n as i64;
        let rest = eins - basis * n as i64;
        return (0..n)
            .map(|i| (basis + i64::from(i < rest as usize)) as i32)
            .collect();
    }

    exps.iter()
        .map(|e| {
            let num = e * eins;
            let q = num / s;
            let r = num % s;
            let doppelt = r.abs() * 2;
            if doppelt > s || (doppelt == s && (q & 1) != 0) { q + 1 } else { q }
        })
        .map(|v| v as i32)
        .collect()
}

#[cfg(test)]
mod vokabular_tests {
    use super::*;

    /// Die Tabelle, die auch das Artefakt trägt: exp(-i/2^8) auf 2^14.
    fn lut() -> Vec<i16> {
        (0..16385)
            .map(|i| ((-(i as f64) / 256.0).exp() * 16384.0).round() as i16)
            .collect()
    }

    /// ⚑ **Der Befund selbst als Test.** Die alte Skala verliert die
    /// Masse, die neue nicht.
    #[test]
    fn ueber_ein_vokabular_bleibt_die_masse_erhalten() {
        let n: usize = 151_936;
        let lut = lut();
        // Ein flacher Schwanz mit einer Spitze, wie ein echter Kopf.
        let logits: Vec<i32> = (0..n as i32)
            .map(|i: i32| if i == 4108 { 200 } else { -(i % 97) })
            .collect();
        let p = softmax_ueber_vokabular(&logits, 16, &lut, 8, 24);
        let summe: i64 = p.iter().map(|v| i64::from(*v)).sum();
        let soll = 1i64 << 24;
        let abweichung = (summe - soll).abs();
        // ⚑ **Die Schranke ist bewiesen, nicht gegriffen.** Kaufmännisches
        // Runden verfehlt je Eintrag höchstens eine halbe Einheit, über
        // n Einträge also n/2. Gemessen sind rund 23.500 von 151.936
        // möglichen: Bei einer fast gleichverteilten Vorlage haben alle
        // Einträge fast denselben Nachkommateil und runden gleichsinnig,
        // genau der systematische Fall, vor dem `moe::route_top_k` warnt.
        assert!(
            abweichung * 2 <= n as i64,
            "die Summe verfehlt die Eins um {abweichung}, mehr als die halbe Einheit je Eintrag"
        );
        // ⚑ **Der eigentliche Befund.** Mit der alten Skala waren
        // 150.064 von 151.936 Wörtern exakt null; hier ist es keines.
        assert_eq!(p.iter().filter(|v| **v == 0).count(), 0, "ein Wort fiel auf null");
    }

    /// ⚑ **Gegenprobe zur Zusicherung**: mit der alten Skala fällt alles
    /// auf null, und genau das verbietet die Zusicherung.
    #[test]
    #[should_panic(expected = "läge unter einer Einheit")]
    fn eine_zu_grobe_ausgangsskala_bricht_ab() {
        let lut = lut();
        let logits = vec![0i32; 151_936];
        let _ = softmax_ueber_vokabular(&logits, 16, &lut, 8, 14);
    }

    /// ⚑ **Gegenprobe zur Sättigung, die den Fehler verdeckte.**
    #[test]
    #[should_panic(expected = "gröber als die Tabellenskala")]
    fn eine_zu_grobe_logitskala_bricht_ab() {
        let lut = lut();
        let logits = vec![0i32; 1024];
        let _ = softmax_ueber_vokabular(&logits, 6, &lut, 8, 20);
    }

    /// Gegen die geschlossene Form.
    #[test]
    fn die_verteilung_trifft_den_gleitkomma_softmax() {
        let lut = lut();
        let logits: Vec<i32> = vec![0, 65536, -65536, 32768, 131_072, -32768, 3, 900_000];
        let p = softmax_ueber_vokabular(&logits, 16, &lut, 8, 24);
        let x: Vec<f64> = logits.iter().map(|z| f64::from(*z) / 65536.0).collect();
        let m = x.iter().cloned().fold(f64::MIN, f64::max);
        let s: f64 = x.iter().map(|v| (v - m).exp()).sum();
        for (i, v) in x.iter().enumerate() {
            let soll = (v - m).exp() / s;
            let ist = f64::from(p[i]) / f64::from(1u32 << 24);
            assert!(
                (ist - soll).abs() < 0.002,
                "Eintrag {i}: {ist:.6} gegen {soll:.6}"
            );
        }
    }

    /// ⚑ Bei einem einzigen Eintrag ist die Antwort die Eins.
    #[test]
    fn ein_einziger_eintrag_bekommt_die_ganze_masse() {
        let lut = lut();
        let p = softmax_ueber_vokabular(&[42], 16, &lut, 8, 20);
        assert_eq!(p, vec![1 << 20]);
    }
}
