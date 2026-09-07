//! Die **Trainingsabgabe**: wie die Treasury das Training bezahlen kann.
//!
//! # ⚑ Das Problem, in einer Zahl
//!
//! Die Treasury bekommt 3 Prozent der Prägung, die Shard-Miner 78. Bei
//! gleicher Vergütung je Rechenstunde deckt sie damit **3,85 Prozent des
//! Inferenzvolumens** an Training. Kap. 7.1 lässt bei leerem Netz 82
//! Prozent der Pods trainieren, am Auslastungsziel 26 Prozent. Die
//! Finanzierung ist also um ein Vielfaches zu klein.
//!
//! ⚑ **Und die Lücke ist strukturell.** Training ist genau dann am
//! erwünschtesten, wenn Inferenz leerläuft, und beide Geldquellen sind
//! genau dann am kleinsten: Die Prägung folgt dem geglätteten Burn, der
//! Burn folgt der Nachfrage, und ein Gebührenaufschlag täte es auch.
//!
//! # ⚑ Die Antwort: ein antizyklischer Puffer
//!
//! **Bei hoher Auslastung wird abgeführt, bei niedriger ausgezahlt.**
//! Das Netz spart, wenn es reich ist, und investiert, wenn es Kapazität
//! übrig hat. Genau dann ist Training auch am billigsten, denn die
//! Pods stünden sonst still.
//!
//! Zwei erprobte Vorbilder tragen die Form:
//!
//! | Vorbild | Was dort geschieht | Was hier davon gilt |
//! |---|---|---|
//! | Basel III, antizyklischer Kapitalpuffer | Banken legen im Aufschwung zurück und lösen im Abschwung auf | Die Richtung: aufbauen, wenn es gut läuft |
//! | EIP-1559, Basisgebühr | Eine protokollgesetzte Gebühr steigt über einem Auslastungsziel | Die Form: die Abgabe hängt an der Auslastung, nicht an einer Verhandlung |
//!
//! ⚑ **Der Unterschied zu EIP-1559 ist der Verbleib.** Dort wird die
//! Basisgebühr **verbrannt**; hier fliesst sie in einen Fonds und kommt
//! als Trainingsvergütung zu denselben Minern zurück. Verbrennen wäre
//! hier falsch: Es nähme dem Netz genau das Geld, mit dem es sein Modell
//! verbessert.
//!
//! # ⚑ Woher die Zahl kommt, und sie ist gerechnet
//!
//! Die Abgabe ist `λ(u) = λ_max · u`, ein Anteil der Prägung, der von den
//! Shard-Minern in die Treasury geht. `λ_max` folgt aus **einer**
//! Bedingung: Am Auslastungsziel `u* = 0,7` soll die Abgabe das Training
//! genau tragen.
//!
//! Mit `m ∝ u` (die Prägung folgt dem Burn, der Burn der Nachfrage),
//! Shard-Anteil `S = 0,78` und Trainingsanteil
//! `t(u) = 0,02 + 0,80·(1−u)`:
//!
//! ```text
//! Rate je Einheit = (S − λ)·m / u = (S − λ)·m₀        (von u unabhängig)
//! Trainingskosten = t(u)·(S − λ)·m₀
//! Abgabezufluss   = u·λ(u)·m₀ = u²·λ_max·m₀
//!
//! Bedingung:  u*²·λ_max = t(u*)·(S − u*·λ_max)
//!             0,49·λ_max = 0,26·(0,78 − 0,7·λ_max)
//!             λ_max = 0,30179
//! ```
//!
//! | u | λ(u) | Shard-Miner | Zufluss | Kosten | Saldo |
//! |---|---|---|---|---|---|
//! | 1,0 | 30,17 % | 47,83 % | 0,302 | 0,010 | **+0,292** |
//! | 0,8 | 24,1 % | 53,9 % | 0,193 | 0,097 | +0,096 |
//! | **0,7** | **21,1 %** | **56,9 %** | **0,148** | **0,148** | **0,000** |
//! | 0,5 | 15,1 % | 62,9 % | 0,075 | 0,264 | −0,189 |
//! | 0,1 | 3,0 % | 75,0 % | 0,003 | 0,555 | **−0,552** |
//!
//! ⚑ **Rund zwei volle Epochen tragen eine leere**: 0,292 gegen 0,552.
//!
//! # ⚑ Was die Miner davon haben, und es ist nichts Schlechtes
//!
//! Die Abgabe ist **kein Verlust**, sondern eine Umschichtung zwischen
//! zwei Arbeitsarten derselben Leute. Am Ziel bekommen sie 56,9 Prozent
//! für Inferenz und 14,8 Prozent für Training; über einen Zyklus ist das
//! dasselbe Geld für mehr Arbeit, und die Arbeit verbessert das Modell,
//! von dem ihre künftige Vergütung lebt.
//!
//! ⚑ **Die drei Prozent der Treasury bleiben unangetastet.** Sie hatten
//! einen anderen Zweck, und eine Abgabe, die eine bestehende Haushalts-
//! stelle still aufbraucht, wäre keine Finanzierung, sondern eine
//! Umwidmung.
//!
//! # ⚑ Die Grenze, und sie bleibt stehen
//!
//! Ein Netz mit **dauerhaft** niedriger Nachfrage kann Training nicht
//! voll bezahlen: Der Puffer leert sich, und die Vergütung je Einheit
//! fällt. **Das ist keine Lücke im Entwurf, sondern die Wahrheit, dass
//! ein Netz ohne Einnahmen nichts zu verteilen hat.** Ein leerlaufender
//! Miner, der wenig verdient, steht immer noch besser da als einer, der
//! nichts tut.

use myl_types::auslastung::AUSLASTUNG_SKALA;

use crate::distribute::SHARES_TOTAL_BPS;

/// Der Höchstsatz der Abgabe bei voller Auslastung, in Basispunkten.
///
/// **Gerechnet und nicht gesetzt**, siehe [`abgabe_max_bps`]: Am
/// Auslastungsziel trägt die Abgabe das Training genau. Der Wert hier
/// ist der bei den Vorgabeparametern; ein Test hält ihn dagegen.
pub const ABGABE_MAX_BPS: u64 = 3_017;

/// Der Höchstsatz aus seinen Eingaben.
///
/// # ⚑ Eine Formel und keine Zahl, damit sie nicht veralten kann
///
/// `λ_max` hängt an vier Grössen: dem Auslastungsziel `u*`, dem
/// Shard-Anteil `S`, der Trainings-Grundrate und dem Freianteil.
/// **Ändert sich eine, ändert sich `λ_max`**, und eine abgeschriebene
/// Zahl wäre ab diesem Tag falsch, ohne dass es jemandem auffiele.
///
/// Aus der Bedingung „am Ziel trägt die Abgabe das Training genau":
///
/// ```text
/// u*² · λ_max = t(u*) · (S − u* · λ_max)
/// λ_max = t(u*) · S / (u*² + t(u*) · u*)
/// ```
///
/// Alle Grössen in Basispunkten; `u*` als Bruch `zaehler/nenner`.
///
/// ⚑ **Abgerundet und nicht gerundet.** Die Abgabe sammelt damit eher
/// eine Einheit zu wenig als eine zu viel, und das ist die Richtung, in
/// die eine Ungenauigkeit fallen soll: Wer zu viel abführt, nimmt den
/// Minern Geld für eine Rechnung, die nicht aufgeht.
///
/// **Returns:** `None`, wenn das Auslastungsziel null ist. Dann gibt es
/// keinen Punkt, an dem die Bilanz aufginge: Ein Netz, dessen Ziel es
/// ist, nichts zu tun, kann nichts finanzieren.
pub fn abgabe_max_bps(
    ziel_zaehler: u64,
    ziel_nenner: u64,
    shard_anteil_bps: u64,
    grundrate_bps: u64,
    freianteil_bps: u64,
) -> Option<u64> {
    if ziel_zaehler == 0 || ziel_nenner == 0 {
        return None;
    }
    // t(u*) in Basispunkten: grundrate + freianteil · (1 − u*).
    let eins = SHARES_TOTAL_BPS as u128;
    let u_z = ziel_zaehler as u128;
    let u_n = ziel_nenner as u128;
    if u_z > u_n {
        return None;
    }
    let frei = eins - (eins * u_z) / u_n;
    let t = grundrate_bps as u128 + (freianteil_bps as u128 * frei) / eins;
    // λ_max = t·S / (u*² + t·u*), mit u* = u_z/u_n.
    // Erweitert mit u_n²:  λ_max = t·S·u_n / (u_z·(u_z + t·u_n/eins))
    let nenner = u_z * u_z * eins + t * u_z * u_n;
    if nenner == 0 {
        return None;
    }
    Some(((t * shard_anteil_bps as u128 * u_n * u_n) / nenner) as u64)
}

/// Die Abgabe bei dieser Auslastung, in Basispunkten der Prägung.
///
/// `λ(u) = λ_max · min(u, 1)`.
///
/// ⚑ **Über hundert Prozent wird nicht weiter erhöht.** Übernachfrage
/// heisst, dass mehr verlangt als geliefert wurde; sie sagt nichts über
/// zusätzliche Einnahmen, aus denen mehr abzuführen wäre.
///
/// ⚑ **Und bei null Auslastung ist die Abgabe null.** Wer als einziger
/// noch Inferenz rechnet, während das Netz leerläuft, soll dafür nicht
/// auch noch zahlen: Der Puffer stammt aus den guten Epochen.
pub fn abgabe_bps(auslastung: i64, max_bps: u64) -> u64 {
    let u = auslastung.clamp(0, AUSLASTUNG_SKALA) as u128;
    ((max_bps as u128 * u) / AUSLASTUNG_SKALA as u128) as u64
}

/// Der Anteil der Shard-Miner nach Abzug der Abgabe, in Basispunkten.
///
/// ⚑ **Geklemmt, damit nichts Negatives entsteht.** Eine Abgabe über dem
/// Shard-Anteil wäre ein Parameterfehler; sie hier durchschlagen zu
/// lassen ergäbe einen Unterlauf und damit einen riesigen Anteil.
pub fn shard_anteil_bps(abgabe_bps: u64) -> u64 {
    crate::distribute::SHARE_SHARD_MINERS_BPS.saturating_sub(abgabe_bps)
}

/// Wie viel der Prägung als Abgabe in die Treasury geht.
pub fn abgabe_betrag(m_e: u64, abgabe_bps: u64) -> u64 {
    let gedeckelt = abgabe_bps.min(crate::distribute::SHARE_SHARD_MINERS_BPS);
    ((m_e as u128 * gedeckelt as u128) / SHARES_TOTAL_BPS as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKALA: i64 = AUSLASTUNG_SKALA;

    /// Die Tabelle aus dem Modulkopf, Zeile für Zeile.
    #[test]
    fn die_abgabe_folgt_der_auslastung() {
        assert_eq!(abgabe_bps(SKALA, ABGABE_MAX_BPS), 3_017, "voll ausgelastet");
        assert_eq!(abgabe_bps(SKALA * 8 / 10, ABGABE_MAX_BPS), 2_413, "80 Prozent");
        assert_eq!(abgabe_bps(SKALA * 7 / 10, ABGABE_MAX_BPS), 2_111, "am Ziel");
        assert_eq!(abgabe_bps(SKALA / 2, ABGABE_MAX_BPS), 1_508, "die Haelfte");
        assert_eq!(abgabe_bps(0, ABGABE_MAX_BPS), 0, "leerlaufend: keine Abgabe");
    }

    /// ⚑ **Die Richtung ist die ganze Anforderung:** mehr Auslastung,
    /// mehr Abgabe.
    #[test]
    fn mehr_auslastung_bedeutet_mehr_abgabe() {
        let mut vorher = 0;
        for teil in 0..=20 {
            let jetzt = abgabe_bps(SKALA * teil / 20, ABGABE_MAX_BPS);
            assert!(jetzt >= vorher, "bei {teil}/20 faellt die Abgabe von {vorher} auf {jetzt}");
            vorher = jetzt;
        }
    }

    /// Übernachfrage hebt sie nicht weiter.
    #[test]
    fn ueber_hundert_prozent_steigt_sie_nicht_weiter() {
        let voll = abgabe_bps(SKALA, ABGABE_MAX_BPS);
        assert_eq!(abgabe_bps(SKALA * 3, ABGABE_MAX_BPS), voll);
        assert_eq!(abgabe_bps(i64::MAX, ABGABE_MAX_BPS), voll);
    }

    /// ⚑ **Die Bilanzbedingung, nachgerechnet.**
    ///
    /// Am Auslastungsziel muss der Zufluss die Trainingskosten decken.
    /// Ohne diesen Test wäre die Herleitung im Modulkopf eine Erzählung.
    #[test]
    fn am_auslastungsziel_traegt_die_abgabe_das_training() {
        // u* = 0,7; Trainingsanteil t(u*) = 0,02 + 0,8·0,3 = 0,26.
        let u = SKALA * 7 / 10;
        let lam = abgabe_bps(u, ABGABE_MAX_BPS) as f64 / 10_000.0;
        let shard = shard_anteil_bps(abgabe_bps(u, ABGABE_MAX_BPS)) as f64 / 10_000.0;
        let t = 0.02 + 0.80 * 0.30;
        // Zufluss (in Vielfachen von m0): u·λ. Kosten: t·(S − λ).
        let zufluss = 0.70 * lam;
        let kosten = t * shard;
        assert!(
            (zufluss - kosten).abs() < 0.002,
            "am Ziel klafft es: Zufluss {zufluss:.4}, Kosten {kosten:.4}"
        );
    }

    /// ⚑ **Oberhalb des Ziels wird angespart, unterhalb gezehrt.** Das
    /// ist der ganze Sinn des Puffers.
    #[test]
    fn ueber_dem_ziel_wird_angespart_darunter_gezehrt() {
        let saldo = |u_anteil: f64| -> f64 {
            let u = (SKALA as f64 * u_anteil) as i64;
            let lam = abgabe_bps(u, ABGABE_MAX_BPS) as f64 / 10_000.0;
            let shard = shard_anteil_bps(abgabe_bps(u, ABGABE_MAX_BPS)) as f64 / 10_000.0;
            let t = 0.02 + 0.80 * (1.0 - u_anteil);
            u_anteil * lam - t * shard
        };
        assert!(saldo(1.0) > 0.28, "bei voller Auslastung wird zu wenig zurueckgelegt");
        assert!(saldo(0.8) > 0.0, "ueber dem Ziel muss angespart werden");
        assert!(saldo(0.5) < 0.0, "unter dem Ziel muss gezehrt werden");
        assert!(saldo(0.1) < -0.5, "bei leerem Netz zehrt es kraeftig");
        // ⚑ Rund zwei volle Epochen tragen eine leere.
        let verhaeltnis = -saldo(0.1) / saldo(1.0);
        assert!(
            (1.5..2.5).contains(&verhaeltnis),
            "das Verhaeltnis ist {verhaeltnis:.2} statt rund zwei"
        );
    }

    /// ⚑ **Die Konstante ist die Formel bei den Vorgabeparametern.**
    #[test]
    fn die_konstante_folgt_aus_der_formel() {
        let gerechnet = abgabe_max_bps(7, 10, 7_800, 200, 8_000).expect("rechenbar");
        assert_eq!(gerechnet, ABGABE_MAX_BPS, "die Konstante ist veraltet");
    }

    /// Die Formel reagiert richtungsrichtig auf ihre Eingaben.
    #[test]
    fn die_formel_reagiert_richtungsrichtig() {
        let grund = abgabe_max_bps(7, 10, 7_800, 200, 8_000).expect("rechenbar");
        // Mehr Training verlangt mehr Abgabe.
        assert!(abgabe_max_bps(7, 10, 7_800, 200, 9_000).expect("r") > grund);
        assert!(abgabe_max_bps(7, 10, 7_800, 400, 8_000).expect("r") > grund);
        // Ein höheres Auslastungsziel senkt sie: dann bleibt weniger
        // freie Kapazität, also weniger Training.
        assert!(abgabe_max_bps(9, 10, 7_800, 200, 8_000).expect("r") < grund);
        // Ohne Ziel gibt es keinen Punkt, an dem die Bilanz aufgeht.
        assert_eq!(abgabe_max_bps(0, 10, 7_800, 200, 8_000), None);
        assert_eq!(abgabe_max_bps(11, 10, 7_800, 200, 8_000), None, "u* ueber eins");
    }

    /// Der Betrag, und er läuft nicht über.
    #[test]
    fn der_betrag_ist_der_anteil_der_praegung() {
        assert_eq!(abgabe_betrag(10_000, 3_018), 3_018);
        assert_eq!(abgabe_betrag(0, 3_018), 0);
        assert_eq!(abgabe_betrag(10_000, 0), 0);
        // Eine Abgabe ueber dem Shard-Anteil wird gedeckelt.
        assert_eq!(abgabe_betrag(10_000, 9_999), 7_800);
        let _ = abgabe_betrag(u64::MAX, 3_018);
    }

    /// Der Shard-Anteil bleibt nichtnegativ.
    #[test]
    fn der_shard_anteil_unterlaeuft_nicht() {
        assert_eq!(shard_anteil_bps(0), 7_800);
        assert_eq!(shard_anteil_bps(3_018), 4_782);
        assert_eq!(shard_anteil_bps(9_999), 0, "kein Unterlauf");
    }
}
