//! Messwerte für Menschen, nicht für den Konsens.
//!
//! # ⚑ Warum hier Gleitkomma stehen darf und nebenan nicht
//!
//! Kein Wert aus diesem Modul geht in eine Rechnung ein, die ein
//! zweiter Knoten nachvollziehen muss: nicht in ein Gewicht, nicht in
//! ein Commitment, nicht in einen Block. Es sind Zahlen, die ein Mensch
//! liest, um zu beurteilen, ob ein Trainingslauf etwas gebracht hat.
//!
//! ⚑ **Deshalb steht dieses Modul in `BEWUSST_DRAUSSEN` des
//! Gleitkomma-Audits**, mit derselben Begründung wie `loader.rs` und
//! `utilization.rs`. Ein erster Entwurf am 2026-09-06 legte die
//! Kreuzentropie in `trainingsschleife.rs`, und das Audit hat ihn
//! zurückgewiesen: **Der Heisspfad kennt keine Ausnahme für „ist ja nur
//! eine Anzeige".**
//!
//! ⚑ **Und wer von hier etwas in den Rechenpfad zurückgibt, bricht
//! genau diese Zusage.** Die Funktionen nehmen Ganzzahlen und geben
//! `f64`; die Richtung ist eine Einbahnstrasse.

/// Die Kreuzentropie **aus den Ganzzahllogits**.
///
/// # ⚑ Aus den Logits und nicht aus einem gerundeten `p`
///
/// Ein Verlust aus `p` hat einen Boden: Sobald `p[ziel]` auf null
/// fällt, meldet er `frac · ln 2` und rührt sich nicht mehr, egal was
/// das Modell tut. Genau darauf lief der erste Entwurf am 2026-09-04
/// auf, **9,7041 = 14 · ln 2**, dreissig Schritte lang unverändert
/// (Fund 177). Der Logarithmus der Summe kennt diesen Boden nicht.
///
/// ⚑ **Und die Summe wird um das grösste Logit verschoben.** Ohne das
/// liefe `exp` bei grossen Logits über, und der Verlust wäre `inf`.
pub fn kreuzentropie_aus_logits(logits: &[i32], ziel: usize, logit_frac: u8) -> f64 {
    if logits.is_empty() || ziel >= logits.len() {
        return f64::NAN;
    }
    let skala = f64::from(1u32 << logit_frac);
    let groesstes = logits.iter().copied().max().unwrap_or(0);
    let mut summe = 0.0f64;
    for l in logits {
        summe += ((f64::from(*l) - f64::from(groesstes)) / skala).exp();
    }
    let log_summe = summe.ln() + f64::from(groesstes) / skala;
    log_summe - f64::from(logits[ziel]) / skala
}

/// Die Perplexität aus einer Summe von Kreuzentropien.
///
/// ⚑ **`exp(mittlere Kreuzentropie)`**, und der Mittelwert geht über die
/// **ausgewerteten** Positionen, nicht über alle Token: Die letzte
/// Position einer Folge hat kein nächstes Wort und zählt nicht mit. Wer
/// sie mitzählte, teilte durch eine zu grosse Zahl und meldete eine zu
/// gute Perplexität.
pub fn perplexitaet(summe_verlust: f64, ausgewertet: usize) -> f64 {
    if ausgewertet == 0 {
        return f64::NAN;
    }
    (summe_verlust / ausgewertet as f64).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein sicheres Modell hat einen Verlust nahe null.
    #[test]
    fn ein_sicheres_modell_verliert_fast_nichts() {
        let frac = 16u8;
        let eins = 1i32 << frac;
        // Das Ziel liegt zwanzig Einheiten ueber dem Rest.
        let mut logits = vec![0i32; 8];
        logits[3] = 20 * eins;
        let v = kreuzentropie_aus_logits(&logits, 3, frac);
        assert!(v < 1e-5, "der Verlust ist {v} statt nahe null");
        assert!((perplexitaet(v, 1) - 1.0).abs() < 1e-4);
    }

    /// ⚑ **Gleichverteilung ergibt `ln n`**, und das ist die Probe auf
    /// die Rechnung: Bei acht gleich wahrscheinlichen Wörtern ist die
    /// Perplexität acht.
    #[test]
    fn gleichverteilung_ergibt_ln_n() {
        let v = kreuzentropie_aus_logits(&[0i32; 8], 3, 16);
        assert!((v - 8f64.ln()).abs() < 1e-9, "der Verlust ist {v} statt ln 8");
        assert!((perplexitaet(v, 1) - 8.0).abs() < 1e-6);
    }

    /// ⚑ **Grosse Logits laufen nicht über.** Ohne die Verschiebung um
    /// das grösste wäre `exp` hier unendlich.
    #[test]
    fn grosse_logits_laufen_nicht_ueber() {
        let frac = 16u8;
        let gross = 700 * (1i32 << frac);
        let logits = vec![gross, gross, gross];
        let v = kreuzentropie_aus_logits(&logits, 0, frac);
        assert!(v.is_finite(), "der Verlust ist {v}");
        assert!((v - 3f64.ln()).abs() < 1e-6);
    }

    /// Ränder: leere Logits und ein Ziel ausserhalb.
    #[test]
    fn die_raender_melden_nan_statt_zu_luegen() {
        assert!(kreuzentropie_aus_logits(&[], 0, 16).is_nan());
        assert!(kreuzentropie_aus_logits(&[1, 2, 3], 9, 16).is_nan());
        assert!(perplexitaet(1.0, 0).is_nan());
    }
}
