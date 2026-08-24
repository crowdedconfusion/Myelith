//! Anlaufphase und Trainingssegment-Rate (Punkte 3.4 und 4.1,
//! Whitepaper Kap. 5.5 und 5.7, Anhang B.8).
//!
//! ## Die Rückkopplung, die Kap. 5.7 beschreibt
//!
//! Ein Protokoll braucht ein werthaltiges Asset, um gesichert zu sein,
//! und muss gesichert sein, damit das Asset Wert erlangt. Arbeitsbasierte
//! Systeme umgehen das, indem sie äußere Ressourcen in Coins überführen;
//! in Myelith ist die Arbeit selbst an vorhandenen Einsatz gebunden.
//! **Ohne MYL kein Stake, ohne Stake keine Miner, ohne Miner keine
//! Prägung.**
//!
//! Der Ausweg ist keine Vorabmenge nach Gutdünken, sondern die
//! quadratische Abhängigkeit der Sicherheitsbedingung:
//! `S_min = g/p²`. Wird `p` in der Anlaufphase erhöht, fällt der
//! Stake-Bedarf drastisch — bei 50 statt 2 Prozent auf ein
//! Sechshundertstel. Das kostet Kapazität, denn jedes zweite Segment
//! wird nachgerechnet, ist in einer Phase mit Überkapazität aber
//! tragbar.
//!
//! **Die Anfangsmenge bemisst sich damit am Stake-Bedarf unter erhöhter
//! Prüfrate, nicht an einem gesetzten Zielwert.** Das ist die eigentliche
//! Aussage von Kap. 5.7, und dieses Modul rechnet sie aus.
//!
//! ## Warum Trainingssegmente die Rate erhöhen und nicht den Stake
//!
//! Kap. 5.5: „Der Gewinn aus Betrug ist geringer, da die
//! Trainingsvergütung niedriger liegt, der Schaden dagegen größer, denn
//! ein durchgerutschtes Inferenz-Segment betrifft eine Antwort, ein
//! durchgerutschter Gradient hingegen das Modell und damit alle künftigen
//! Antworten. Angehoben wird deshalb nicht der Stake, sondern die
//! **Stichprobenrate**: Sie wirkt unmittelbar und kostet Kapazität statt
//! Kapitalbindung."
//!
//! Der Unterschied ist zeitlich: Eine Stake-Erhöhung wirkt erst, wenn
//! alle Miner nachgelegt haben, und das dauert; eine Ratenerhöhung wirkt
//! mit dem nächsten Segment.

use crate::sicherheit::{s_min, SicherheitsFehler};

/// Ein Punkt des Anlaufplans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anlaufstufe {
    /// Stichprobenrate als Bruch, Zähler.
    pub p_zaehler: u64,
    /// Stichprobenrate als Bruch, Nenner.
    pub p_nenner: u64,
    /// Mindest-Stake je Kapazitätseinheit bei dieser Rate.
    pub stake_je_einheit: u64,
    /// Stake-Bedarf des ganzen Netzes bei dieser Rate.
    pub stake_gesamt: u64,
}

/// Der Stake-Bedarf eines Netzes bei gegebener Prüfrate.
///
/// **Parameter:**
/// - `miner`: Zahl der Teilnehmer
/// - `kapazitaet_je_miner`: Segmente je Epoche und Miner
/// - `betrugsgewinn_g`: Gewinn aus einem betrogenen Segment
/// - `p_zaehler`, `p_nenner`: die Prüfrate
pub fn stufe(
    miner: u64,
    kapazitaet_je_miner: u64,
    betrugsgewinn_g: u64,
    p_zaehler: u64,
    p_nenner: u64,
) -> Result<Anlaufstufe, SicherheitsFehler> {
    let stake_je_einheit = s_min(betrugsgewinn_g, p_zaehler, p_nenner)?;
    let stake_gesamt = (stake_je_einheit as u128)
        .saturating_mul(kapazitaet_je_miner as u128)
        .saturating_mul(miner as u128)
        .min(u64::MAX as u128) as u64;
    Ok(Anlaufstufe {
        p_zaehler,
        p_nenner,
        stake_je_einheit,
        stake_gesamt,
    })
}

/// Die kleinste Prüfrate, mit der ein Netz mit dem verfügbaren MYL
/// auskommt.
///
/// Die Frage, die vor dem Genesis-Block zu beantworten ist: Gegeben eine
/// Anfangsmenge, wie hoch muss die Prüfrate sein, damit die
/// Sicherheitsbedingung erfüllt ist?
///
/// **Gesucht wird die kleinste Rate**, denn jede Erhöhung kostet
/// Kapazität. Der Nenner ist fest (Prozentschritte), gesucht wird der
/// Zähler.
///
/// **Returns:** `None`, wenn selbst die volle Prüfung (`p = 1`) nicht
/// genügt. Das ist eine echte Antwort und keine Ausnahme: Dann ist die
/// Anfangsmenge zu klein für dieses Netz, und die Zahl der Startminer
/// oder die Kapazität je Miner muss sinken.
pub fn kleinste_ausreichende_rate(
    verfuegbares_myl: u64,
    miner: u64,
    kapazitaet_je_miner: u64,
    betrugsgewinn_g: u64,
    nenner: u64,
) -> Option<Anlaufstufe> {
    if nenner == 0 {
        return None;
    }
    (1..=nenner).find_map(|z| {
        let s = stufe(miner, kapazitaet_je_miner, betrugsgewinn_g, z, nenner).ok()?;
        (s.stake_gesamt <= verfuegbares_myl).then_some(s)
    })
}

/// Die Stichprobenrate für **Trainingssegmente** (Punkt 3.4).
///
/// Kap. 5.5 verlangt eine erhöhte Rate statt eines erhöhten Stakes, nennt
/// aber keinen Faktor. Der Entwurf setzt das **Fünffache** der
/// Inferenzrate, gedeckelt bei 1 (mehr als jedes Segment lässt sich nicht
/// prüfen).
///
/// **Der Faktor ist eine Festlegung dieses Entwurfs und steht in keinem
/// Kapitel.** Er ist so gewählt, dass er die Zielrate von 2 % auf 10 %
/// hebt, also in die Größenordnung, die Anhang B.8.2 für die Anlaufphase
/// durchrechnet. Er gehört bestätigt und steht im Fahrplan.
pub const TRAININGSRATE_FAKTOR: u64 = 5;

/// Trainingsrate = `min(Faktor · p, 1)`.
///
/// Der Deckel ist kein Randfall: Bei einer Inferenzrate ab 20 % wäre das
/// Fünffache über 1, und eine Rate über 1 ist keine Rate.
pub fn trainingsrate(p_zaehler: u64, p_nenner: u64) -> Result<(u64, u64), SicherheitsFehler> {
    if p_nenner == 0 || p_zaehler == 0 {
        return Err(SicherheitsFehler::UnbrauchbareStichprobenrate {
            zaehler: p_zaehler,
            nenner: p_nenner,
        });
    }
    if p_zaehler > p_nenner {
        return Err(SicherheitsFehler::RateUeberEins {
            zaehler: p_zaehler,
            nenner: p_nenner,
        });
    }
    let erhoeht = p_zaehler.saturating_mul(TRAININGSRATE_FAKTOR);
    Ok((erhoeht.min(p_nenner), p_nenner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UNITS_PER_MYL;

    fn g() -> u64 {
        UNITS_PER_MYL / 2
    }

    /// **Anhang B.8.2 vollständig**, Zeile für Zeile.
    ///
    /// „Da S_min quadratisch von p abhängt, sinkt der Bedarf für
    /// zweihundert Miner von 250.000 MYL bei zwei Prozent auf 40.000 bei
    /// fünf, 10.000 bei zehn, 1.600 bei fünfundzwanzig und 400 MYL bei
    /// fünfzig Prozent."
    #[test]
    fn anhang_b_8_2_zeile_fuer_zeile() {
        for (p, erwartet_myl) in [(2u64, 250_000u64), (5, 40_000), (10, 10_000), (25, 1_600), (50, 400)] {
            let s = stufe(200, 1, g(), p, 100).unwrap();
            assert_eq!(
                s.stake_gesamt,
                erwartet_myl * UNITS_PER_MYL,
                "p = {p} %: {} statt {} MYL",
                s.stake_gesamt / UNITS_PER_MYL,
                erwartet_myl
            );
        }
    }

    /// **Anhang B.8.1**: „Für fünfzig Startminer ergibt das einen
    /// Stake-Bedarf von 62.500 MYL."
    #[test]
    fn anhang_b_8_1_fuenfzig_startminer() {
        let s = stufe(50, 1, g(), 2, 100).unwrap();
        assert_eq!(s.stake_gesamt, 62_500 * UNITS_PER_MYL);
    }

    /// **Kap. 5.7**: „Bei einer Rate von fünfzig Prozent statt zwei
    /// Prozent fällt er auf ein Sechshundertstel."
    ///
    /// Nachgerechnet: (50/2)² = 625. Das Papier sagt „ein
    /// Sechshundertstel", also gerundet. Der Test prüft die **exakte**
    /// Zahl und hält fest, dass die Aussage des Papiers eine Rundung ist
    /// und keine Ungenauigkeit der Rechnung.
    #[test]
    fn kapitel_5_7_der_faktor_ist_625_nicht_600() {
        let bei_zwei = stufe(1, 1, g(), 2, 100).unwrap().stake_gesamt;
        let bei_fuenfzig = stufe(1, 1, g(), 50, 100).unwrap().stake_gesamt;
        assert_eq!(bei_zwei / bei_fuenfzig, 625);
    }

    /// Die kleinste ausreichende Rate ist die kleinste, nicht irgendeine.
    #[test]
    fn die_kleinste_ausreichende_rate_ist_die_kleinste() {
        // 200 Miner, je eine Einheit: bei 5 % sind es 40 000 MYL.
        let budget = 40_000 * UNITS_PER_MYL;
        let s = kleinste_ausreichende_rate(budget, 200, 1, g(), 100).unwrap();
        assert_eq!((s.p_zaehler, s.p_nenner), (5, 100));
        // Ein Kleinstbetrag weniger, und es muss eine Stufe höher gehen.
        let s = kleinste_ausreichende_rate(budget - 1, 200, 1, g(), 100).unwrap();
        assert!(s.p_zaehler > 5);
        assert!(s.stake_gesamt < budget);
    }

    /// Reicht selbst die volle Prüfung nicht, ist das eine Antwort und
    /// keine Ausnahme.
    #[test]
    fn ein_zu_kleines_budget_hat_keine_rate() {
        // Ein MYL für 200 Miner: selbst bei p = 1 sind es 200 · 0,5 MYL.
        assert!(kleinste_ausreichende_rate(UNITS_PER_MYL, 200, 1, g(), 100).is_none());
    }

    /// Die Trainingsrate ist das Fünffache, gedeckelt bei 1.
    #[test]
    fn die_trainingsrate_ist_erhoeht_und_gedeckelt() {
        assert_eq!(trainingsrate(2, 100).unwrap(), (10, 100));
        assert_eq!(trainingsrate(5, 100).unwrap(), (25, 100));
        // Ab 20 % greift der Deckel.
        assert_eq!(trainingsrate(20, 100).unwrap(), (100, 100));
        assert_eq!(trainingsrate(50, 100).unwrap(), (100, 100));
        assert_eq!(trainingsrate(1, 1).unwrap(), (1, 1));
    }

    /// Die Trainingsrate ist **nie kleiner** als die Inferenzrate.
    ///
    /// Wäre sie es, kehrte sich die Begründung aus Kap. 5.5 um: Der
    /// größere Schaden wäre schlechter geschützt als der kleinere.
    #[test]
    fn die_trainingsrate_liegt_nie_unter_der_inferenzrate() {
        for z in 1..=100u64 {
            let (tz, tn) = trainingsrate(z, 100).unwrap();
            assert_eq!(tn, 100);
            assert!(tz >= z, "p = {z} %, Trainingsrate {tz} %");
            assert!(tz <= tn, "eine Rate über 1 ist keine Rate");
        }
    }

    /// Unbrauchbare Raten kommen als Fehler zurück.
    #[test]
    fn unbrauchbare_raten_sind_fehler() {
        assert!(trainingsrate(0, 100).is_err());
        assert!(trainingsrate(1, 0).is_err());
        assert!(trainingsrate(3, 2).is_err());
    }
}
