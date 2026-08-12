//! Prägungsverteilung und Redundanz-Normierung (Punkt 1.3,
//! Whitepaper Kap. 5.3).
//!
//! Verteilungsschlüssel (Kap. 5.3, in Basispunkten, Summe 10 000):
//! Shard-Miner 78 %, Koordinatoren 5 %, Validatoren 10 %, Checker 4 %,
//! Treasury 3 %.
//!
//! Exaktheit (Akzeptanzkriterium Phase 1): Die Summe der Anteile ergibt
//! in jeder Epoche exakt `M_e` — jeder Anteil wird mit floor gerundet,
//! der Rundungsrest (weniger als 5 Einheiten, da 5 Anteile) geht
//! geschlossen an das Treasury (deterministische, dokumentierte Regel —
//! kein Rundungsverlust und kein Rundungsgewinn).
//!
//! Redundanz-Normierung (Kap. 4.4/5.3): Jedes Segment wird von r = 2
//! unabhängig zugelosten Pods berechnet; jeder Pod erhält die halbe
//! vTFE-Gutschrift. Die proportionalen Miner-Anteile werden aus den
//! normierten Gewichten berechnet.

use std::collections::BTreeMap;

use myl_types::ids::Address;

/// Anteil der Shard-Miner in Basispunkten (78 %).
pub const SHARE_SHARD_MINERS_BPS: u64 = 7800;
/// Anteil der Koordinatoren in Basispunkten (5 %).
pub const SHARE_COORDINATORS_BPS: u64 = 500;
/// Anteil der Validatoren in Basispunkten (10 %).
pub const SHARE_VALIDATORS_BPS: u64 = 1000;
/// Anteil der Checker in Basispunkten (4 %).
pub const SHARE_CHECKERS_BPS: u64 = 400;
/// Anteil des Treasury in Basispunkten (3 %).
pub const SHARE_TREASURY_BPS: u64 = 300;
/// Summe aller Anteile (Vollständigkeits-Invariante).
pub const SHARES_TOTAL_BPS: u64 = SHARE_SHARD_MINERS_BPS
    + SHARE_COORDINATORS_BPS
    + SHARE_VALIDATORS_BPS
    + SHARE_CHECKERS_BPS
    + SHARE_TREASURY_BPS;

/// Aufteilung einer Epochen-Prägung auf die fünf Empfängergruppen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distribution {
    pub shard_miners: u64,
    pub coordinators: u64,
    pub validators: u64,
    pub checkers: u64,
    pub treasury: u64,
}

impl Distribution {
    /// Summe aller Anteile (muss stets `M_e` ergeben).
    pub fn summe(&self) -> u64 {
        self.shard_miners
            .saturating_add(self.coordinators)
            .saturating_add(self.validators)
            .saturating_add(self.checkers)
            .saturating_add(self.treasury)
    }
}

/// Teilt eine Prägung `m_e` nach dem Kap.-5.3-Schlüssel auf.
///
/// Jeder Anteil wird mit floor gerundet; der Rundungsrest geht
/// geschlossen an das Treasury (siehe Modul-Dokumentation). Damit ist
/// `summe() == m_e` für alle `m_e` eine Invariante.
pub fn distribute_mint(m_e: u64) -> Distribution {
    let shard_miners = share_floor(m_e, SHARE_SHARD_MINERS_BPS);
    let coordinators = share_floor(m_e, SHARE_COORDINATORS_BPS);
    let validators = share_floor(m_e, SHARE_VALIDATORS_BPS);
    let checkers = share_floor(m_e, SHARE_CHECKERS_BPS);
    let vier_fixed = shard_miners as u128
        + coordinators as u128
        + validators as u128
        + checkers as u128;
    // Der Rest (Rundungsanteile + Treasury-Grundanteil) geht ans
    // Treasury — deterministisch und exakt.
    let treasury = (m_e as u128 - vier_fixed) as u64;
    Distribution {
        shard_miners,
        coordinators,
        validators,
        checkers,
        treasury,
    }
}

/// floor(m · bps / 10 000) in u128-Zwischenrechnung.
fn share_floor(m: u64, bps: u64) -> u64 {
    ((m as u128 * bps as u128) / SHARES_TOTAL_BPS as u128) as u64
}

/// Redundanz-Normierung: halbe vTFE-Gutschrift je Pod (r = 2, Kap. 4.4).
pub fn redundancy_normalized_weight(vtfe: u64) -> u64 {
    vtfe / 2
}

/// Fehler der proportionalen Verteilung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributeError {
    /// Positiver Betrag, aber alle Gewichte sind 0.
    PositiveAmountWithoutWeights,
}

impl std::fmt::Display for DistributeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PositiveAmountWithoutWeights => write!(
                f,
                "positiver Betrag kann nicht auf Gewichte von insgesamt 0 verteilt werden"
            ),
        }
    }
}

impl std::error::Error for DistributeError {}

/// Teilt einen Betrag proportional zu Gewichten auf Adressen auf —
/// exakt: die Summe der Auszahlungen ergibt stets `total`.
///
/// Verfahren (deterministisch): Jeder Anteil wird mit floor gerundet;
/// die verbleibenden Einheiten (< Anzahl der Empfänger) werden in
/// aufsteigender Adress-Reihenfolge je eine Einheit an Empfänger mit
/// Gewicht > 0 vergeben. Doppelte Adressen in der Eingabe werden
/// zusammengeführt. Empfänger mit Gewicht 0 erhalten nichts.
pub fn split_proportional(
    total: u64,
    weights: &[(Address, u64)],
) -> Result<BTreeMap<Address, u64>, DistributeError> {
    // Gewichte zusammenführen (deterministisch durch BTreeMap-Ordnung).
    let mut gewichte: BTreeMap<Address, u64> = BTreeMap::new();
    for (addr, w) in weights {
        let eintrag = gewichte.entry(*addr).or_insert(0);
        *eintrag = eintrag.saturating_add(*w);
    }
    let summe_gewichte: u128 = gewichte.values().map(|w| *w as u128).sum();
    if total == 0 {
        return Ok(BTreeMap::new());
    }
    if summe_gewichte == 0 {
        return Err(DistributeError::PositiveAmountWithoutWeights);
    }

    let mut auszahlung: BTreeMap<Address, u64> = BTreeMap::new();
    let mut verteilt: u128 = 0;
    for (addr, w) in &gewichte {
        if *w == 0 {
            continue;
        }
        let anteil = (total as u128 * *w as u128) / summe_gewichte;
        auszahlung.insert(*addr, anteil as u64);
        verteilt += anteil;
    }

    // Rest in aufsteigender Adress-Reihenfolge verteilen.
    let mut rest = (total as u128 - verteilt) as u64;
    for (addr, w) in &gewichte {
        if rest == 0 {
            break;
        }
        if *w == 0 {
            continue;
        }
        *auszahlung.get_mut(addr).expect("angelegt") += 1;
        rest -= 1;
    }
    Ok(auszahlung)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schlüssel_ist_vollstaendig() {
        assert_eq!(SHARES_TOTAL_BPS, 10_000);
    }

    #[test]
    fn verteilung_summe_ist_exakt_m_e() {
        for m in [0u64, 1, 2, 3, 4, 5, 9_999, 10_000, 10_001, 123_456_789] {
            let d = distribute_mint(m);
            assert_eq!(d.summe(), m, "Summe muss exakt M_e sein (m={})", m);
        }
    }

    #[test]
    fn verteilung_anteile_passen_zum_schluessel() {
        // 1 000 000 lässt sich glatt teilen: 780 000 / 50 000 /
        // 100 000 / 40 000 / 30 000.
        let d = distribute_mint(1_000_000);
        assert_eq!(d.shard_miners, 780_000);
        assert_eq!(d.coordinators, 50_000);
        assert_eq!(d.validators, 100_000);
        assert_eq!(d.checkers, 40_000);
        assert_eq!(d.treasury, 30_000);
    }

    #[test]
    fn akzeptanzkriterium_zehntausend_epochen_zufallswerte() {
        // Akzeptanzkriterium Phase 1: Summe der Anteile == M_e in jeder
        // von 10.000 Epochen mit zufälligen Werten.
        let mut state = 0x1234_5678u64;
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let m = state >> 8; // 56-Bit-Werte
            let d = distribute_mint(m);
            assert_eq!(d.summe(), m);
        }
    }

    #[test]
    fn redundanz_normierung_halbiert() {
        assert_eq!(redundancy_normalized_weight(100), 50);
        assert_eq!(redundancy_normalized_weight(101), 50); // floor
        assert_eq!(redundancy_normalized_weight(0), 0);
    }

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    #[test]
    fn proportionale_verteilung_summe_exakt() {
        let weights = vec![
            (adresse(1), 10u64),
            (adresse(2), 20),
            (adresse(3), 30),
        ];
        for total in [0u64, 1, 5, 59, 60, 61, 100, 999_999, u64::MAX >> 8] {
            let map = split_proportional(total, &weights).expect("Verteilung");
            let summe: u64 = map.values().sum();
            assert_eq!(summe, total, "Summe muss exakt total sein (total={})", total);
        }
    }

    #[test]
    fn proportionale_verteilung_gewichtung() {
        let weights = vec![(adresse(1), 10u64), (adresse(2), 30)];
        let map = split_proportional(100, &weights).expect("Verteilung");
        // 1:3 ⇒ 25/75.
        assert_eq!(map[&adresse(1)], 25);
        assert_eq!(map[&adresse(2)], 75);
    }

    #[test]
    fn proportionale_verteilung_rest_deterministisch() {
        // 100 auf drei gleiche Gewichte: 33/33/33 + Rest 1 an die
        // kleinste Adresse.
        let weights = vec![(adresse(3), 1u64), (adresse(1), 1), (adresse(2), 1)];
        let map = split_proportional(100, &weights).expect("Verteilung");
        assert_eq!(map[&adresse(1)], 34);
        assert_eq!(map[&adresse(2)], 33);
        assert_eq!(map[&adresse(3)], 33);
    }

    #[test]
    fn proportionale_verteilung_nullgewichte() {
        let weights = vec![(adresse(1), 0u64), (adresse(2), 10)];
        let map = split_proportional(50, &weights).expect("Verteilung");
        assert_eq!(map.get(&adresse(1)), None);
        assert_eq!(map[&adresse(2)], 50);
        // Nur Nullgewichte bei positivem Betrag ⇒ Fehler.
        assert_eq!(
            split_proportional(10, &[(adresse(1), 0)]),
            Err(DistributeError::PositiveAmountWithoutWeights)
        );
        // Betrag 0 ⇒ leere Auszahlung (auch ohne Gewichte).
        assert!(split_proportional(0, &[(adresse(1), 0)]).expect("leer").is_empty());
    }

    #[test]
    fn proportionale_verteilung_doppelte_adressen() {
        let weights = vec![(adresse(1), 10u64), (adresse(1), 10), (adresse(2), 20)];
        let map = split_proportional(100, &weights).expect("Verteilung");
        // adresse(1): Gewicht 20 von 40 ⇒ 50; adresse(2): 50.
        assert_eq!(map[&adresse(1)], 50);
        assert_eq!(map[&adresse(2)], 50);
    }
}
