//! Slash-/Kopfgeld-Entscheidung — Whitepaper Kap. 5.5, 6.6, Anhang A.4.
//!
//! Bestimmt aus dem Bisektionsergebnis, **wer** verloren hat, und
//! übersetzt das in den Schiedsspruch, den der Ledger bucht.
//!
//! ## Warum hier keine Beträge mehr stehen (Fund A9)
//!
//! Bis v0.2.6 hatte dieses Modul eine eigene `SlashConfig` mit **festen
//! Beträgen** (1 MYL Slash, 0,5 MYL Kopfgeld). Das war ein zweites,
//! unvereinbares Slashing-Modell neben dem des Ledgers:
//!
//! - `myl_ledger::apply_verdict` schlachtet einen **Anteil des Stakes**
//!   (`SlashParams` als Zähler/Nenner-Paare) — so wie es Whitepaper
//!   Kap. 5.5 vorgibt (30–100 % des Stakes je nach Vergehen).
//! - `myl-verifier` rechnete mit absoluten Beträgen und hing nicht
//!   einmal an `myl-ledger`, konnte also gar nicht buchen.
//!
//! Ein fester Betrag hat zudem keine Abschreckungswirkung: 1 MYL ist
//! für einen Großstaker nichts, und die gesamte Sicherheitsannahme der
//! Verifikationsarchitektur (Kap. 6.9: Betrug muss teurer sein als der
//! erwartete Gewinn) hängt genau daran.
//!
//! Dieses Modul entscheidet deshalb nur noch über **Schuld**, nicht über
//! Beträge. Die Beträge ergeben sich aus dem Stake und den
//! Governance-Parametern, wenn `myl_ledger::apply_verdict` den
//! Schiedsspruch bucht.
//!
//! **Konsens-Feld:** Die Slash-Logik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_ledger::transitions::{Verdict as LedgerVerdict, VerdictOutcome as LedgerOutcome};
use myl_types::ids::{Address, MinerId, SegmentId};

/// Ergebnis der Schiedsrunde (wer hat gewonnen/verloren).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictOutcome {
    /// Primärer Pod hat verloren (war fehlerhaft).
    PrimaryLoses,
    /// Redundanter Pod hat verloren (hat falsch challenget).
    RedundantLoses,
}

/// Eine Slash-Entscheidung: wer hat verloren, und warum.
///
/// Enthält bewusst **keine** Beträge — siehe Modul-Dokumentation.
/// Für die Buchung liefert [`Self::to_ledger_verdict`] den Schiedsspruch
/// im Ledger-Format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashDecision {
    /// Das strittige Segment.
    pub segment_id: SegmentId,
    /// Miner, der geslasht wird (Verlierer).
    pub slashed_miner: MinerId,
    /// Miner, der das Kopfgeld erhält (Gewinner).
    pub rewarded_miner: MinerId,
    /// Grund der Slash-Entscheidung.
    pub reason: SlashReason,
}

/// Grund der Slash-Entscheidung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashReason {
    /// Primärer Pod hat fehlerhafte Berechnung durchgeführt.
    PrimaryFault {
        /// Position der Abweichung.
        divergence_position: usize,
    },
    /// Redundanter Pod hat falsche Challenge eingereicht.
    RedundantFault,
}

/// Fehler bei der Slash-Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashError {
    /// Miner-IDs sind identisch (kein sinnvoller Slash).
    IdenticalMiners,
}

impl std::fmt::Display for SlashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdenticalMiners => write!(f, "Miner-IDs sind identisch"),
        }
    }
}

impl std::error::Error for SlashError {}

impl SlashDecision {
    /// Übersetzt die Entscheidung in den Schiedsspruch, den
    /// `myl_ledger::apply_verdict` bucht.
    ///
    /// Der Ledger führt Konten unter `Address`
    /// (`Address = SHA-256(komprimierter BLS-Public-Key)`), die
    /// Verifikation arbeitet mit `MinerId`. Die Zuordnung ist ein
    /// Registry-Nachschlag, keine reine Umrechnung — deshalb werden die
    /// beiden Adressen übergeben, statt sie hier zu erraten.
    ///
    /// **Parameter:**
    /// - `slashed_addr`: Ledger-Adresse des Verlierers
    /// - `rewarded_addr`: Ledger-Adresse des Gewinners
    ///
    /// **Returns:** `Verdict` mit `outcome = SlashMiner`; der Verlierer
    /// steht im Feld `miner`, der Gewinner in `checker`. Der Ledger
    /// schlachtet damit den Stake des Verlierers und zahlt dem Gewinner
    /// das Kopfgeld — unabhängig davon, welche Rolle die beiden im
    /// Streit hatten.
    pub fn to_ledger_verdict(
        &self,
        slashed_addr: Address,
        rewarded_addr: Address,
    ) -> LedgerVerdict {
        LedgerVerdict {
            segment_id: self.segment_id,
            miner: slashed_addr,
            checker: rewarded_addr,
            outcome: LedgerOutcome::SlashMiner,
        }
    }
}

/// Erstellt eine Slash-Entscheidung basierend auf dem Verdict.
///
/// **Parameter:**
/// - `outcome`: Ergebnis der Schiedsrunde
/// - `segment_id`: das strittige Segment
/// - `primary_miner`: Miner des primären Pods
/// - `redundant_miner`: Miner des redundanten Pods
/// - `divergence_position`: Position der Abweichung (nur bei `PrimaryLoses`)
///
/// **Returns:** `SlashDecision` bei erfolgreicher Erstellung.
///
/// **Fehler:** `SlashError::IdenticalMiners`, wenn beide Seiten
/// derselbe Miner sind — dann gibt es nichts zu entscheiden.
pub fn create_slash_decision(
    outcome: VerdictOutcome,
    segment_id: SegmentId,
    primary_miner: MinerId,
    redundant_miner: MinerId,
    divergence_position: Option<usize>,
) -> Result<SlashDecision, SlashError> {
    if primary_miner == redundant_miner {
        return Err(SlashError::IdenticalMiners);
    }

    let (slashed_miner, rewarded_miner, reason) = match outcome {
        VerdictOutcome::PrimaryLoses => (
            primary_miner,
            redundant_miner,
            SlashReason::PrimaryFault {
                divergence_position: divergence_position.unwrap_or(0),
            },
        ),
        VerdictOutcome::RedundantLoses => {
            (redundant_miner, primary_miner, SlashReason::RedundantFault)
        }
    };

    Ok(SlashDecision {
        segment_id,
        slashed_miner,
        rewarded_miner,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_ledger::state::LedgerState;
    use myl_ledger::transitions::{apply_verdict, SlashParams};

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn addr(b: u8) -> Address {
        Address::new([b; 32])
    }

    fn segment() -> SegmentId {
        SegmentId::new([1u8; 32])
    }

    #[test]
    fn primaerer_pod_verliert() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            miner(1),
            miner(2),
            Some(7),
        )
        .unwrap();

        assert_eq!(d.slashed_miner, miner(1));
        assert_eq!(d.rewarded_miner, miner(2));
        assert_eq!(
            d.reason,
            SlashReason::PrimaryFault {
                divergence_position: 7
            }
        );
    }

    #[test]
    fn redundanter_pod_verliert() {
        let d = create_slash_decision(
            VerdictOutcome::RedundantLoses,
            segment(),
            miner(1),
            miner(2),
            None,
        )
        .unwrap();

        assert_eq!(d.slashed_miner, miner(2));
        assert_eq!(d.rewarded_miner, miner(1));
        assert_eq!(d.reason, SlashReason::RedundantFault);
    }

    #[test]
    fn identische_miner_werden_abgelehnt() {
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                miner(1),
                miner(1),
                None
            ),
            Err(SlashError::IdenticalMiners)
        );
    }

    /// Der Kern von Fund A9: Die Entscheidung muss beim Ledger ankommen.
    /// Vorher rechnete dieses Modul mit festen Beträgen und hing nicht
    /// einmal an `myl-ledger` — es konnte gar nicht buchen.
    #[test]
    fn entscheidung_wird_vom_ledger_gebucht() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            miner(1),
            miner(2),
            Some(3),
        )
        .unwrap();

        let mut state = LedgerState::genesis(1);
        state.account_mut(&addr(1)).staked = 1_000_000;

        let params = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 2, // 50 % des Stakes
            bounty_fraction_num: 1,
            bounty_fraction_den: 10, // 10 % davon als Kopfgeld
        };

        let verdict = d.to_ledger_verdict(addr(1), addr(2));
        let effect = apply_verdict(&mut state, &verdict, &params).unwrap();

        assert_eq!(effect.slashed, 500_000);
        assert_eq!(effect.bounty, 50_000);
        assert_eq!(state.account(&addr(1)).staked, 500_000);
        assert_eq!(state.account(&addr(2)).balance, 50_000);
    }

    /// Der Slash ist ein **Anteil des Stakes** — ein Großstaker verliert
    /// entsprechend mehr. Mit dem alten Festbetrag (1 MYL) hätte er
    /// unabhängig von seiner Größe immer dasselbe verloren.
    #[test]
    fn slash_skaliert_mit_dem_stake() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            miner(1),
            miner(2),
            None,
        )
        .unwrap();
        let params = SlashParams {
            slash_fraction_num: 3,
            slash_fraction_den: 10, // 30 %, Whitepaper Kap. 5.5 untere Grenze
            bounty_fraction_num: 1,
            bounty_fraction_den: 10,
        };

        let mut klein = LedgerState::genesis(1);
        klein.account_mut(&addr(1)).staked = 10_000_000;
        let e_klein =
            apply_verdict(&mut klein, &d.to_ledger_verdict(addr(1), addr(2)), &params).unwrap();

        let mut gross = LedgerState::genesis(1);
        gross.account_mut(&addr(1)).staked = 10_000_000_000;
        let e_gross =
            apply_verdict(&mut gross, &d.to_ledger_verdict(addr(1), addr(2)), &params).unwrap();

        assert_eq!(e_klein.slashed, 3_000_000);
        assert_eq!(e_gross.slashed, 3_000_000_000);
        assert!(e_gross.slashed > e_klein.slashed);
    }

    #[test]
    fn ledger_verdict_traegt_die_segment_id() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            miner(1),
            miner(2),
            None,
        )
        .unwrap();
        let v = d.to_ledger_verdict(addr(1), addr(2));
        assert_eq!(v.segment_id, segment());
        assert_eq!(v.miner, addr(1));
        assert_eq!(v.checker, addr(2));
    }
}
