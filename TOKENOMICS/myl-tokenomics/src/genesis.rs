//! Genesis-Verteilungsmechanik (Punkt 4.2, Whitepaper Kap. 5.7).
//!
//! > „Die Genesis-Menge geht ausschließlich an Teilnehmer des
//! > vorgelagerten Testnetzes, bemessen nach dort geleisteter und
//! > geprüfter Arbeit, zuzüglich des Treasury-Anteils aus Kapitel 5.3. Es
//! > findet kein Vorverkauf statt, und es gibt keine Zuteilung an
//! > Entwickler oder Investoren über die Treasury hinaus."
//!
//! ## Wie „kein Vorverkauf" hier durchgesetzt wird
//!
//! Nicht durch eine Prüfung, sondern durch die **Form der Funktion**:
//! [`genesis_verteilung`] nimmt Arbeitsnachweise und sonst nichts. Es
//! gibt keinen Parameter für Sonderzuteilungen, keine Liste von
//! Ausnahmen, keinen Rest, über den jemand verfügen könnte. Wer eine
//! Zuteilung außerhalb der Arbeit unterbringen wollte, müsste die
//! Signatur ändern, und das fällt in einem Diff auf.
//!
//! Eine Prüfung wäre die schwächere Lösung: Sie ließe den Weg offen und
//! stellte sich davor.
//!
//! ## Der Treasury-Anteil
//!
//! Kap. 5.3 gibt dem Treasury 3 % der laufenden Prägung
//! ([`crate::SHARE_TREASURY_BPS`]). Derselbe Satz gilt hier für die
//! Genesis-Menge; die Konstante wird **benutzt** und nicht wiederholt,
//! damit eine Änderung dort auch hier ankommt.
//!
//! ## Warum die Menge nicht hier bestimmt wird
//!
//! Die Höhe der Genesis-Menge folgt aus dem Stake-Bedarf der Anlaufphase
//! unter erhöhter Prüfrate ([`crate::anlauf`]), also aus der
//! Sicherheitsbedingung und nicht aus einem gesetzten Zielwert. Dieses
//! Modul verteilt eine gegebene Menge; **es setzt sie nicht fest**. Die
//! Trennung hält die beiden Fragen auseinander, die Kap. 5.7 ebenfalls
//! trennt: wie viel es sein muss, und wer es bekommt.

use std::collections::BTreeMap;

use myl_types::ids::Address;

use crate::distribute::{split_proportional, DistributeError};
use crate::SHARE_TREASURY_BPS;

/// Geprüfte Arbeit eines Testnetz-Teilnehmers.
///
/// **„Geprüft" ist Teil der Bedingung, nicht der Beschreibung.** Kap. 5.7
/// sagt „geleisteter *und geprüfter* Arbeit"; unbestätigte Arbeit gehört
/// nicht in diese Liste. Der Typ kann das nicht erzwingen, deshalb steht
/// es hier: Wer diese Struktur füllt, verantwortet, dass die vTFE aus
/// einem abgeschlossenen Epochenabschluss stammen und nicht aus einem
/// Anspruch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arbeitsnachweis {
    /// Der Empfänger.
    pub adresse: Address,
    /// Geprüfte Arbeit in vTFE-Einheiten.
    pub vtfe: u64,
}

/// Das Ergebnis der Genesis-Verteilung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisVerteilung {
    /// Die Zuteilung je Teilnehmer, in Kleinstbeträgen.
    pub teilnehmer: BTreeMap<Address, u64>,
    /// Der Treasury-Anteil, in Kleinstbeträgen.
    pub treasury: u64,
    /// Die verteilte Gesamtmenge. Gleich der Vorgabe, immer.
    pub gesamt: u64,
}

/// Fehler der Genesis-Verteilung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenesisFehler {
    /// Eine positive Menge, aber niemand hat Arbeit nachgewiesen.
    ///
    /// Kein Randfall: Es gäbe dann keinen arbeitsgebundenen Empfänger,
    /// und die ganze Menge fiele ans Treasury. Das wäre genau die
    /// Zuteilung außerhalb der Arbeit, die Kap. 5.7 ausschließt.
    KeineArbeitNachgewiesen,
    /// Die Aufteilung selbst ist fehlgeschlagen.
    Aufteilung(DistributeError),
}

impl std::fmt::Display for GenesisFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeineArbeitNachgewiesen => write!(
                f,
                "niemand hat Arbeit nachgewiesen; die Genesis-Menge fiele vollständig \
                 ans Treasury, und das wäre eine Zuteilung außerhalb der Arbeit"
            ),
            Self::Aufteilung(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for GenesisFehler {}

/// Verteilt die Genesis-Menge auf Testnetz-Arbeit und Treasury.
///
/// **Parameter:**
/// - `gesamtmenge`: die Anfangsmenge in Kleinstbeträgen, bestimmt aus dem
///   Stake-Bedarf der Anlaufphase ([`crate::anlauf`])
/// - `nachweise`: geprüfte Arbeit je Teilnehmer
///
/// **Reproduzierbar:** Dieselben Nachweise ergeben dieselbe Verteilung,
/// bis auf das letzte Kleinstbetrags-Bit. Das ist das Akzeptanzkriterium
/// der Phase („aus Testnetz-Arbeitsdaten reproduzierbar nachvollziehbar")
/// und der Grund, warum die Aufteilung über
/// [`crate::split_proportional`] läuft: Sie führt doppelte Adressen
/// zusammen und vergibt den Rundungsrest in fester Adress-Reihenfolge.
///
/// **Die Summe stimmt exakt.** Ein verschwundener Rest wäre Geld, das
/// niemand bekommt; ein doppelt vergebener wäre Geld aus dem Nichts.
pub fn genesis_verteilung(
    gesamtmenge: u64,
    nachweise: &[Arbeitsnachweis],
) -> Result<GenesisVerteilung, GenesisFehler> {
    let arbeit_gesamt: u128 = nachweise.iter().map(|n| n.vtfe as u128).sum();
    if gesamtmenge > 0 && arbeit_gesamt == 0 {
        return Err(GenesisFehler::KeineArbeitNachgewiesen);
    }

    // Treasury zuerst, abgerundet: Der Rundungsrest geht damit an die
    // Arbeit und nicht ans Treasury. Bei einer Menge, die aus dem
    // Stake-Bedarf folgt, sind das Kleinstbeträge; die Richtung ist
    // trotzdem eine Entscheidung, und sie fällt zugunsten derer, die
    // gearbeitet haben.
    let treasury = ((gesamtmenge as u128 * SHARE_TREASURY_BPS as u128) / 10_000) as u64;
    let an_die_arbeit = gesamtmenge - treasury;

    let gewichte: Vec<(Address, u64)> =
        nachweise.iter().map(|n| (n.adresse, n.vtfe)).collect();
    let teilnehmer = if an_die_arbeit == 0 {
        BTreeMap::new()
    } else {
        split_proportional(an_die_arbeit, &gewichte).map_err(GenesisFehler::Aufteilung)?
    };

    Ok(GenesisVerteilung {
        teilnehmer,
        treasury,
        gesamt: gesamtmenge,
    })
}

impl GenesisVerteilung {
    /// Summe aller Zuteilungen einschließlich Treasury.
    ///
    /// Muss stets `gesamt` ergeben; der Test hält es fest.
    pub fn summe(&self) -> u128 {
        self.teilnehmer.values().map(|v| *v as u128).sum::<u128>() + self.treasury as u128
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UNITS_PER_MYL;

    fn adresse(b: u64) -> Address {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&b.to_le_bytes());
        Address::new(bytes)
    }

    fn nachweise(paare: &[(u64, u64)]) -> Vec<Arbeitsnachweis> {
        paare
            .iter()
            .map(|&(a, v)| Arbeitsnachweis { adresse: adresse(a), vtfe: v })
            .collect()
    }

    /// **Die Summe stimmt exakt**, für jede Menge und jede Arbeitsverteilung.
    #[test]
    fn die_summe_stimmt_immer_exakt() {
        let mut z: u64 = 0x243F_6A88;
        for _ in 0..20_000 {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            let menge = z % 1_000_000_000;
            let n = nachweise(&[
                (1, z % 1_000),
                (2, (z >> 8) % 1_000),
                (3, (z >> 16) % 1_000),
            ]);
            if let Ok(v) = genesis_verteilung(menge, &n) {
                assert_eq!(v.summe(), menge as u128, "Menge {menge}");
            }
        }
    }

    /// **Reproduzierbar:** Dieselben Nachweise ergeben dieselbe Verteilung.
    ///
    /// Das wörtliche Akzeptanzkriterium der Phase. Geprüft wird auch, dass
    /// die **Reihenfolge der Nachweise** nichts ändert: Sonst hinge die
    /// Zuteilung daran, in welcher Reihenfolge jemand die Testnetzdaten
    /// eingelesen hat.
    #[test]
    fn dieselben_daten_ergeben_dieselbe_verteilung() {
        let menge = 62_500 * UNITS_PER_MYL;
        let a = nachweise(&[(1, 500), (2, 300), (3, 200)]);
        let b = nachweise(&[(3, 200), (1, 500), (2, 300)]);
        let v1 = genesis_verteilung(menge, &a).unwrap();
        let v2 = genesis_verteilung(menge, &a).unwrap();
        let v3 = genesis_verteilung(menge, &b).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v1, v3, "die Reihenfolge der Nachweise darf nichts ändern");
    }

    /// Die Zuteilung ist proportional zur geprüften Arbeit.
    #[test]
    fn die_zuteilung_folgt_der_arbeit() {
        let menge = 1_000_000u64;
        let v = genesis_verteilung(menge, &nachweise(&[(1, 600), (2, 400)])).unwrap();
        let an_arbeit = menge - v.treasury;
        let eins = v.teilnehmer[&adresse(1)];
        let zwei = v.teilnehmer[&adresse(2)];
        assert_eq!(eins, an_arbeit * 6 / 10);
        assert_eq!(zwei + eins, an_arbeit);
        assert!(eins > zwei, "wer mehr gearbeitet hat, bekommt mehr");
    }

    /// **Der Treasury-Anteil ist der aus Kap. 5.3**, und er wird nicht
    /// zweimal aufgeschrieben.
    #[test]
    fn der_treasury_anteil_ist_der_aus_kapitel_5_3() {
        assert_eq!(SHARE_TREASURY_BPS, 300, "3 %");
        let menge = 1_000_000u64;
        let v = genesis_verteilung(menge, &nachweise(&[(1, 1)])).unwrap();
        assert_eq!(v.treasury, menge * 300 / 10_000);
    }

    /// **Wer nicht gearbeitet hat, bekommt nichts.**
    #[test]
    fn ohne_arbeit_keine_zuteilung() {
        let v = genesis_verteilung(1_000_000, &nachweise(&[(1, 100), (2, 0)])).unwrap();
        assert!(!v.teilnehmer.contains_key(&adresse(2)) || v.teilnehmer[&adresse(2)] == 0);
        assert!(v.teilnehmer[&adresse(1)] > 0);
    }

    /// **Ohne jede nachgewiesene Arbeit gibt es keine Verteilung.**
    ///
    /// Sonst fiele die ganze Menge ans Treasury, und das wäre genau die
    /// Zuteilung außerhalb der Arbeit, die Kap. 5.7 ausschließt.
    #[test]
    fn ohne_jede_arbeit_keine_verteilung() {
        assert_eq!(
            genesis_verteilung(1_000_000, &[]),
            Err(GenesisFehler::KeineArbeitNachgewiesen)
        );
        assert_eq!(
            genesis_verteilung(1_000_000, &nachweise(&[(1, 0), (2, 0)])),
            Err(GenesisFehler::KeineArbeitNachgewiesen)
        );
        // Eine Menge von null ist dagegen zulässig und leer.
        let v = genesis_verteilung(0, &[]).unwrap();
        assert_eq!(v.summe(), 0);
    }

    /// **Der Rundungsrest geht an die Arbeit, nicht ans Treasury.**
    ///
    /// Eine Richtungsentscheidung, keine Zwangsläufigkeit. Sie fällt
    /// zugunsten derer, die gearbeitet haben.
    #[test]
    fn der_rundungsrest_geht_an_die_arbeit() {
        // 10 001 · 3 % = 300,03 → Treasury 300, Arbeit 9 701.
        let v = genesis_verteilung(10_001, &nachweise(&[(1, 1)])).unwrap();
        assert_eq!(v.treasury, 300);
        assert_eq!(v.teilnehmer[&adresse(1)], 9_701);
        assert_eq!(v.summe(), 10_001);
    }

    /// **Der ganze Weg aus Kap. 5.7**, von der Sicherheitsbedingung zur
    /// Zuteilung.
    ///
    /// Anhang B.8.1: 50 Startminer, 2 % Prüfrate, 62 500 MYL. Genau diese
    /// Menge wird verteilt, und sie kommt nicht aus einem Zielwert,
    /// sondern aus `S_min`.
    #[test]
    fn der_ganze_weg_von_der_sicherheitsbedingung_zur_zuteilung() {
        let g = UNITS_PER_MYL / 2;
        let bedarf = crate::anlauf::stufe(50, 1, g, 2, 100).unwrap();
        assert_eq!(bedarf.stake_gesamt, 62_500 * UNITS_PER_MYL);

        // Fünfzig Teilnehmer mit gleicher Arbeit.
        let n: Vec<Arbeitsnachweis> = (1..=50u64)
            .map(|i| Arbeitsnachweis { adresse: adresse(i), vtfe: 100 })
            .collect();
        let v = genesis_verteilung(bedarf.stake_gesamt, &n).unwrap();
        assert_eq!(v.summe(), bedarf.stake_gesamt as u128);
        assert_eq!(v.teilnehmer.len(), 50);
        // Gleiche Arbeit, gleiche Zuteilung bis auf den Rundungsrest.
        let werte: Vec<u64> = v.teilnehmer.values().copied().collect();
        let min = *werte.iter().min().unwrap();
        let max = *werte.iter().max().unwrap();
        assert!(max - min <= 1, "gleiche Arbeit, aber {min} bis {max}");
    }
}
