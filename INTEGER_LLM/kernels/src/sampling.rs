//! Integer-Sampling (Greedy + CDF)
// Der Index ist die Token-ID — er ist das Ergebnis, nicht nur ein Zaehler.
#![allow(clippy::needless_range_loop)]

use crate::prng::splitmix64;

/// Argmax ueber Integer-Logits.
pub fn argmax_int(values: &[i32]) -> usize {
    let mut best_i = 0;
    let mut best_v = values[0];
    for i in 1..values.len() {
        if values[i] > best_v {
            best_v = values[i];
            best_i = i;
        }
    }
    best_i
}

/// Wandelt Logits in positive Integer-Gewichte um.
pub fn logits_to_weights(logits: &[i32]) -> Vec<i32> {
    let m = logits.iter().copied().min().unwrap_or(0);
    logits.iter().map(|z| z - m + 1).collect()
}

/// Deterministisches Sampling via Integer-CDF und SplitMix64.
pub fn sample_integer_cdf(logits: &[i32], state: u64) -> (usize, u64) {
    let weights = logits_to_weights(logits);
    let total: i64 = weights.iter().map(|w| *w as i64).sum();

    if total <= 0 {
        return (0, state);
    }

    let (new_state, r) = splitmix64(state);
    let threshold = (r % total as u64) as i64;

    let mut acc: i64 = 0;
    for (i, w) in weights.iter().enumerate() {
        acc += *w as i64;
        if threshold < acc {
            return (i, new_state);
        }
    }
    (weights.len() - 1, new_state)
}

/// **Wie gezogen wird**: Temperatur, Top-k und Top-p, alles ganzzahlig.
///
/// ⚑ **Die Temperatur steht als Kehrwert** (`kehrwert_temperatur_q8`,
/// also `256 / T`), damit sie multipliziert wird: Geteilt wird im
/// Rechenpfad nur durch einen Shift. `T = 0,7` ist `366`, `T = 1` ist
/// `256`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ziehparameter {
    /// `256 / T`, mindestens 1.
    pub kehrwert_temperatur_q8: u32,
    /// Nur die `top_k` groessten Logits kommen in Frage, mindestens 1.
    pub top_k: usize,
    /// Von diesen nur die kleinste Spitze, deren Gewicht zusammen
    /// mindestens `top_p_q16 / 65536` des Ganzen traegt; 65536 heisst alle.
    pub top_p_q16: u32,
}

/// **Zieht ein Token nach Temperatur, Top-k und Top-p**, bitgenau bei
/// gleicher Saat.
///
/// # ⚑ Warum nicht [`sample_integer_cdf`]
///
/// Jene gewichtet jedes Wort **linear** mit `z - min + 1`. Ueber einen
/// Wortschatz von 150 000 bis 250 000 Eintraegen traegt dann der lange
/// Schwanz fast das ganze Gewicht, und gezogen wuerde beinahe gleich
/// verteilt (`bin/logit_probe` rechnet es aus). Sie steht so in theta_v
/// und bleibt deshalb unveraendert; ein Wechsel im Netz ist eine eigene
/// Fassung der Spezifikation.
///
/// # ⚑ Der Weg, ganzzahlig
///
/// 1. Die `top_k` groessten Logits, bei Gleichstand der kleinere Index
///    zuerst. Eine strenge Gesamtordnung, also ist die Auswahl eindeutig.
/// 2. Gewicht je Kandidat `exp(-(z_max - z) / T)` aus der exp-Tabelle des
///    Artefakts: Die Differenz wird mit dem Kehrwert der Temperatur
///    multipliziert und mit einem gerundeten Shift in die Eingangsskala
///    der Tabelle gebracht. Die Spitze bekommt immer den ersten Eintrag
///    und damit ein Gewicht groesser null.
/// 3. Top-p ueber die absteigend geordneten Gewichte, in `i128`.
/// 4. Eine Zahl aus SplitMix64, Rest nach der Summe, Gang durch die
///    kumulierten Gewichte.
///
/// `logit_frac` ist die Skala der Logits, `exp_input_frac` die Skala,
/// die der Index der Tabelle meint.
///
/// # Panics
///
/// Bei leeren Logits, leerer Tabelle oder einem Parameter von null.
pub fn ziehen(
    logits: &[i32],
    logit_frac: u8,
    p: &Ziehparameter,
    exp_lut: &[i16],
    exp_input_frac: u8,
    state: u64,
) -> (usize, u64) {
    assert!(!logits.is_empty(), "ziehen: keine Logits");
    assert!(!exp_lut.is_empty(), "ziehen: keine exp-Tabelle");
    assert!(p.kehrwert_temperatur_q8 >= 1 && p.top_k >= 1 && p.top_p_q16 >= 1, "ziehen: {p:?}");

    // 1. Die Kandidaten, strenge Ordnung: groesserer Logit, dann kleinerer Index.
    let ordnung = |a: &usize, b: &usize| logits[*b].cmp(&logits[*a]).then(a.cmp(b));
    let mut kandidaten: Vec<usize> = (0..logits.len()).collect();
    let k = p.top_k.min(logits.len());
    if k < kandidaten.len() {
        kandidaten.select_nth_unstable_by(k - 1, ordnung);
        kandidaten.truncate(k);
    }
    kandidaten.sort_unstable_by(ordnung);

    // 2. Die Gewichte. Index = (z_max - z) * (256 / T) in der Tabellenskala.
    let spitze = i64::from(logits[kandidaten[0]]);
    let rauf = u32::from(exp_input_frac);
    let runter = u32::from(logit_frac) + 8;
    let gewichte: Vec<i64> = kandidaten
        .iter()
        .map(|&i| {
            let produkt = (spitze - i64::from(logits[i])) * i64::from(p.kehrwert_temperatur_q8);
            let index = if rauf >= runter {
                produkt.checked_shl(rauf - runter).unwrap_or(i64::MAX)
            } else {
                crate::fixed_point::rshift_round_i64(produkt, (runter - rauf) as u8)
            };
            usize::try_from(index).ok().and_then(|x| exp_lut.get(x)).map_or(0, |&e| i64::from(e.max(0)))
        })
        .collect();

    // 3. Top-p: die kleinste Spitze, die den Anteil erreicht.
    let gesamt: i128 = gewichte.iter().map(|&g| i128::from(g)).sum();
    let schwelle = gesamt * i128::from(p.top_p_q16);
    let mut behalten = 0usize;
    let mut summe: i128 = 0;
    for &g in &gewichte {
        if behalten > 0 && (summe << 16) >= schwelle {
            break;
        }
        summe += i128::from(g);
        behalten += 1;
    }
    let summe = u64::try_from(summe).unwrap_or(u64::MAX);
    if summe == 0 {
        // Nur moeglich mit einer Tabelle, deren erster Eintrag null ist.
        return (kandidaten[0], state);
    }

    // 4. Ziehen.
    let (neu, zufall) = crate::prng::splitmix64(state);
    let ziel = zufall % summe;
    let mut kumuliert = 0u64;
    for (j, &g) in gewichte.iter().take(behalten).enumerate() {
        kumuliert += g as u64;
        if ziel < kumuliert {
            return (kandidaten[j], neu);
        }
    }
    (kandidaten[behalten - 1], neu)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine exp-Tabelle wie im Artefakt: Eingangsskala 8, Ausgang 2^14,
    /// ganzzahlig erzeugt (exp(-i/256) ueber die Reihe waere Gleitkomma;
    /// hier genuegt die Halbierung alle 177 Schritte, also ln 2 * 256).
    fn tabelle() -> Vec<i16> {
        (0..4096).map(|i| ((1i32 << 14) >> (i / 177)) as i16).collect()
    }

    fn param(t_q8: u32, k: usize, p: u32) -> Ziehparameter {
        Ziehparameter { kehrwert_temperatur_q8: t_q8, top_k: k, top_p_q16: p }
    }

    /// ⚑ **Gleiche Saat, gleiches Token; andere Saat, anderes Token.**
    #[test]
    fn gleiche_saat_gleiches_token_andere_saat_andere_tokens() {
        let logits = vec![100, 100, 100, 100, -5000];
        let p = param(256, 20, 65536);
        let a: Vec<usize> = (0..64).map(|s| ziehen(&logits, 6, &p, &tabelle(), 8, s).0).collect();
        let b: Vec<usize> = (0..64).map(|s| ziehen(&logits, 6, &p, &tabelle(), 8, s).0).collect();
        assert_eq!(a, b, "bitgenau bei gleicher Saat");
        let verschieden: std::collections::BTreeSet<usize> = a.iter().copied().collect();
        assert_eq!(verschieden.len(), 4, "vier gleich grosse Logits, alle vier gezogen: {a:?}");
        assert!(!a.contains(&4), "der Schwanz weit unten kommt nicht dran");
    }

    /// ⚑ **Top-k eins ist gierig**, gleich welche Saat.
    #[test]
    fn top_k_eins_ist_argmax() {
        let logits = vec![3, 90, 90, 7];
        for s in 0..50 {
            assert_eq!(ziehen(&logits, 6, &param(256, 1, 65536), &tabelle(), 8, s).0, 1, "Gleichstand: kleinerer Index");
        }
    }

    /// ⚑ **Die Verteilung folgt exp(d / T)**: Ein Abstand von ln 2 (44,4
    /// Einheiten auf Skala 6, hier 45, damit der Index 180 hinter der
    /// ersten Stufe der Testtabelle bei 177 liegt) halbiert das Gewicht
    /// bei T = 1. Gezaehlt ueber 20 000 Saaten, ganzzahlig.
    #[test]
    fn die_verteilung_halbiert_sich_bei_ln_zwei() {
        let logits = vec![45, 0];
        let mut erstes = 0u32;
        let mut s = 7u64;
        for _ in 0..20_000 {
            let (t, neu) = ziehen(&logits, 6, &param(256, 20, 65536), &tabelle(), 8, s);
            s = neu;
            erstes += u32::from(t == 0);
        }
        // Soll 2/3; die Tabelle halbiert in Stufen, also grosszuegig.
        assert!((12_500..14_200).contains(&erstes), "{erstes} von 20 000");
    }

    /// ⚑ **Top-p schneidet den Rest ab**, und eine kleine Temperatur macht
    /// die Spitze sicherer.
    #[test]
    fn top_p_und_temperatur_wirken() {
        let logits = vec![300, 250, 200, 150];
        let zaehlen = |p: &Ziehparameter| {
            let mut n = [0u32; 4];
            let mut s = 1u64;
            for _ in 0..4000 {
                let (t, neu) = ziehen(&logits, 6, p, &tabelle(), 8, s);
                s = neu;
                n[t] += 1;
            }
            n
        };
        let schmal = zaehlen(&param(256, 20, 32768));
        assert_eq!(schmal[2] + schmal[3], 0, "Top-p 0,5 laesst nur die Spitze und wenig mehr: {schmal:?}");
        let kalt = zaehlen(&param(1024, 20, 65536)); // T = 0,25
        let warm = zaehlen(&param(128, 20, 65536)); // T = 2
        assert!(kalt[0] > warm[0], "kalt {kalt:?}, warm {warm:?}");
    }
}

#[cfg(test)]
mod tests_alt {
    use super::*;

    #[test]
    fn test_argmax() {
        let logits = vec![1, 9, 3];
        assert_eq!(argmax_int(&logits), 1);
    }

    #[test]
    fn test_sampling_deterministic() {
        let logits = vec![10, 20, 30];
        let state = 42u64;
        let (t1, s1) = sample_integer_cdf(&logits, state);
        let (t2, s2) = sample_integer_cdf(&logits, state);
        assert_eq!(t1, t2);
        assert_eq!(s1, s2);
    }
}
