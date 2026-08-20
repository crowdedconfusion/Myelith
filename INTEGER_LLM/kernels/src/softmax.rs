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
