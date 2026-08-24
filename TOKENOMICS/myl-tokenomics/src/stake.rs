//! Stake-Hinterlegung nach beanspruchter Kapazität (Punkt 3.1,
//! Whitepaper Kap. 3.4 und 5.5).
//!
//! Kap. 5.5 bindet die Stake-Pflicht des Shard-Miners an die
//! „beanspruchte Reward-Kapazität". Der Zusammenhang ist einfach und
//! genau deshalb wichtig, dass er an einer Stelle steht:
//!
//! ```text
//! erforderlicher Stake = Kapazität · S_min(g, p)
//! ```
//!
//! `S_min` ist der Mindest-Stake **je Kapazitätseinheit** aus
//! [`crate::sicherheit::s_min`]; die Kapazität ist die Zahl der Segmente,
//! die ein Miner je Epoche beansprucht.
//!
//! ## Warum proportional und nicht pauschal
//!
//! Ein pauschaler Stake wäre für kleine Miner eine Zugangsschranke und
//! für große keine Abschreckung: Wer hundertmal so viel Arbeit annimmt,
//! kann hundertmal so viel Schaden anrichten. Die Anreiz-Ungleichung aus
//! Kap. 5.5 gilt je Segment, also muss der Einsatz mit der Zahl der
//! Segmente wachsen.
//!
//! ## Was hier **nicht** steht
//!
//! Die Frage, ob ein Miner seinen Stake tatsächlich hinterlegt hat, ist
//! Ledger-Zustand (`myl_ledger::Account::staked`) und wird dort
//! beantwortet. Dieses Modul rechnet nur aus, wie viel es sein müsste.
//! Die Trennung ist dieselbe wie bei der Slashing-Matrix: Beträge hier,
//! Buchung im Ledger.

use crate::sicherheit::{s_min, SicherheitsFehler};

/// Der Stake-Anspruch eines Miners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StakeAnspruch {
    /// Beanspruchte Kapazität in Segmenten je Epoche.
    pub kapazitaet: u64,
    /// Mindest-Stake je Kapazitätseinheit, in Kleinstbeträgen.
    pub je_einheit: u64,
    /// Erforderlicher Gesamt-Stake, in Kleinstbeträgen.
    pub gesamt: u64,
}

/// Fehler der Stake-Berechnung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StakeFehler {
    /// Die Sicherheitsbedingung ist mit diesen Parametern nicht
    /// auswertbar.
    Sicherheit(SicherheitsFehler),
    /// Der erforderliche Stake übersteigt den `u64`-Bereich.
    ///
    /// Eine echte Aussage und kein Randfall: Die beanspruchte Kapazität
    /// ist dann größer, als das Zahlensystem des Ledgers besichern kann.
    NichtDarstellbar { kapazitaet: u64, je_einheit: u64 },
}

impl std::fmt::Display for StakeFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sicherheit(e) => write!(f, "{}", e),
            Self::NichtDarstellbar { kapazitaet, je_einheit } => write!(
                f,
                "Kapazität {} · {} je Einheit übersteigt den u64-Bereich",
                kapazitaet, je_einheit
            ),
        }
    }
}

impl std::error::Error for StakeFehler {}

/// Erforderlicher Stake für eine beanspruchte Kapazität.
///
/// **Parameter:**
/// - `kapazitaet`: Segmente je Epoche, die der Miner annehmen will
/// - `betrugsgewinn_g`: Gewinn aus einem betrogenen Segment
/// - `p_zaehler`, `p_nenner`: die Stichprobenrate als Bruch
pub fn erforderlicher_stake(
    kapazitaet: u64,
    betrugsgewinn_g: u64,
    p_zaehler: u64,
    p_nenner: u64,
) -> Result<StakeAnspruch, StakeFehler> {
    let je_einheit = s_min(betrugsgewinn_g, p_zaehler, p_nenner).map_err(StakeFehler::Sicherheit)?;
    let gesamt = kapazitaet
        .checked_mul(je_einheit)
        .ok_or(StakeFehler::NichtDarstellbar { kapazitaet, je_einheit })?;
    Ok(StakeAnspruch { kapazitaet, je_einheit, gesamt })
}

/// Die größte Kapazität, die ein gegebener Stake trägt.
///
/// Die Umkehrung von [`erforderlicher_stake`], und die Richtung, in der
/// das Protokoll die Frage stellt: Ein Miner meldet seinen Stake an, und
/// der Scheduler muss wissen, wie viel Arbeit er ihm zuteilen darf.
///
/// **Abgerundet**, denn eine angefangene Kapazitätseinheit ist keine.
pub fn getragene_kapazitaet(
    stake: u64,
    betrugsgewinn_g: u64,
    p_zaehler: u64,
    p_nenner: u64,
) -> Result<u64, StakeFehler> {
    let je_einheit = s_min(betrugsgewinn_g, p_zaehler, p_nenner).map_err(StakeFehler::Sicherheit)?;
    if je_einheit == 0 {
        // `S_min = 0` hieße, jede Kapazität wäre umsonst zu haben. Das
        // kann nur bei `g = 0` eintreten, also wenn Betrug nichts
        // einbringt; dann ist die Frage gegenstandslos.
        return Ok(u64::MAX);
    }
    Ok(stake / je_einheit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UNITS_PER_MYL;

    /// **Das Zahlenbeispiel aus Anhang B.1**, exakt.
    ///
    /// „Ein Miner mit Kapazität von 100 Segmenten je Epoche verdient 50
    /// MYL je Epoche und hinterlegt damit rund 25 Epochen-Einkommen."
    ///
    /// Also: 100 Einheiten · 1250 MYL = 125 000 MYL Stake, und
    /// 125 000 / 50 = 2500 … Moment, das sind 2500 Epochen-Einkommen,
    /// nicht 25. Der Text meint das Verhältnis **je Kapazitätseinheit**:
    /// 1250 MYL Stake gegen 0,5 MYL Einkommen je Segment und Epoche sind
    /// 2500 Segment-Rewards; bei 100 Segmenten je Epoche verdient der
    /// Miner 50 MYL je Epoche, und 1250 / 50 = 25 Epochen-Einkommen **je
    /// Kapazitätseinheit**. Beide Zahlen des Papiers stimmen, sie
    /// beziehen sich auf verschiedene Bezugsgrößen.
    #[test]
    fn zahlenbeispiel_aus_anhang_b1() {
        let g = UNITS_PER_MYL / 2;
        let a = erforderlicher_stake(100, g, 2, 100).unwrap();
        assert_eq!(a.je_einheit, 1_250 * UNITS_PER_MYL);
        assert_eq!(a.gesamt, 125_000 * UNITS_PER_MYL);

        // Einkommen je Epoche: 100 Segmente · 0,5 MYL = 50 MYL.
        let einkommen_je_epoche = 100 * g;
        assert_eq!(einkommen_je_epoche, 50 * UNITS_PER_MYL);
        // Der Stake **je Kapazitätseinheit** entspricht 25 Epochen-Einkommen.
        assert_eq!(a.je_einheit / einkommen_je_epoche, 25);
    }

    /// Der Stake wächst linear mit der Kapazität.
    #[test]
    fn der_stake_waechst_mit_der_kapazitaet() {
        let g = UNITS_PER_MYL / 2;
        let eins = erforderlicher_stake(1, g, 2, 100).unwrap().gesamt;
        for k in [1u64, 2, 10, 100, 1_000] {
            assert_eq!(erforderlicher_stake(k, g, 2, 100).unwrap().gesamt, k * eins);
        }
        // Keine Kapazität, kein Stake.
        assert_eq!(erforderlicher_stake(0, g, 2, 100).unwrap().gesamt, 0);
    }

    /// Hin und zurück: Der Stake für eine Kapazität trägt genau diese.
    #[test]
    fn erforderlich_und_getragen_passen_zusammen() {
        let g = UNITS_PER_MYL / 2;
        for k in [1u64, 7, 100, 4_211] {
            let a = erforderlicher_stake(k, g, 2, 100).unwrap();
            assert_eq!(getragene_kapazitaet(a.gesamt, g, 2, 100).unwrap(), k);
            // Ein Kleinstbetrag weniger trägt eine Einheit weniger.
            assert_eq!(getragene_kapazitaet(a.gesamt - 1, g, 2, 100).unwrap(), k - 1);
        }
    }

    /// Eine absurde Kapazität ist ein Fehler und keine kleine Zahl.
    #[test]
    fn absurde_kapazitaet_ist_ein_fehler() {
        assert!(matches!(
            erforderlicher_stake(u64::MAX, UNITS_PER_MYL, 2, 100),
            Err(StakeFehler::NichtDarstellbar { .. })
        ));
    }

    /// Ohne Prüfung gibt es keine Schranke, und das schlägt durch.
    #[test]
    fn ohne_pruefrate_kein_stake_anspruch() {
        assert!(matches!(
            erforderlicher_stake(100, UNITS_PER_MYL, 0, 100),
            Err(StakeFehler::Sicherheit(_))
        ));
    }
}
