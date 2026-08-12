//! Zustandsübergänge des Ledgers (Anhang A.5).
//!
//! Jeder Übergang ist eine reine Funktion `(State, …) → State` ohne
//! versteckten globalen Zustand; Fehler lassen den Zustand unverändert
//! (Übergänge prüfen zuerst und ändern erst dann).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::ids::{Address, EpochId, SegmentId};
use myl_types::InferenceCredit;

use crate::state::LedgerState;

/// Fehler eines Zustandsübergangs. Übergänge sind atomar: Tritt ein
/// Fehler auf, wurde der Zustand nicht verändert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    /// Betrag wäre 0 — sinnfreie Übergänge werden abgelehnt.
    ZeroAmount,
    /// Das Konto hat nicht genug verfügbare MYL.
    InsufficientBalance { available: u64, required: u64 },
    /// Das Konto hat nicht genug Credits (oder nur verfallene).
    InsufficientCredits { available: u64, required: u64 },
    /// Das Konto hat keinen Stake (z. B. Verdict gegen einen
    /// Beteiligten ohne Hinterlegung).
    NoStake,
    /// Eine Buchung würde den `u64`-Bereich überschreiten.
    Overflow,
    /// Übergangs-Parameter sind ungültig (z. B. Bruch mit Nenner 0
    /// oder Zähler größer als Nenner).
    InvalidParameters,
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroAmount => write!(f, "Übergang mit Betrag 0 abgelehnt"),
            Self::InsufficientBalance { available, required } => write!(
                f,
                "Kontostand reicht nicht: verfügbar {}, benötigt {}",
                available, required
            ),
            Self::InsufficientCredits { available, required } => write!(
                f,
                "Credit-Guthaben reicht nicht: verfügbar {}, benötigt {}",
                available, required
            ),
            Self::NoStake => write!(f, "Konto hat keinen Stake"),
            Self::Overflow => write!(f, "Buchung würde den Wertebereich überschreiten"),
            Self::InvalidParameters => write!(f, "Übergangs-Parameter sind ungültig"),
        }
    }
}

impl std::error::Error for TransitionError {}

/// Ausgang eines Bisektions-Schiedsspruchs (Whitepaper Kap. 6.6,
/// Anhang A.4: `Verdict::SlashMiner` / `Verdict::SlashChecker`).
///
/// Minimaler Zwischen-Typ in `myl-ledger`, bis VERIFICATION den vollen
/// Challenge-/Verdict-Typ definiert (dokumentierte Übergangslösung).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum VerdictOutcome {
    /// Der Miner hat falsch gerechnet: sein Stake wird geschlachtet,
    /// der Checker erhält das Kopfgeld.
    SlashMiner,
    /// Der Checker hat falsch beschuldigt: sein Stake wird geschlachtet,
    /// der Miner erhält das Kopfgeld.
    SlashChecker,
}

/// Schiedsspruch über ein Segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verdict {
    /// Das strittige Segment.
    pub segment_id: SegmentId,
    /// Der beteiligte Shard-Miner.
    pub miner: Address,
    /// Der beteiligte Checker.
    pub checker: Address,
    /// Wer wurde für schuldig befunden.
    pub outcome: VerdictOutcome,
}

/// Slash-Parameter (Brüche als Ganzzahl-Paare — keine Gleitkomma).
/// Die Werte sind Start-/Testparameter; die endgültigen Werte legt
/// TOKENOMICS (Kap. 5.5) fest, die Verwaltung übernimmt GOVERNANCE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashParams {
    /// Anteil des Stakes, der geschlachtet wird (Zähler/Nenner, ≤ 1).
    pub slash_fraction_num: u64,
    /// Nenner des Slash-Anteils (> 0).
    pub slash_fraction_den: u64,
    /// Anteil des geschlachteten Betrags, der als Kopfgeld an die
    /// Gegenpartei ausgezahlt wird (Zähler/Nenner, ≤ 1). Der Rest
    /// verbleibt unverteilt (= faktisch verbrannt).
    pub bounty_fraction_num: u64,
    /// Nenner des Kopfgeld-Anteils (> 0).
    pub bounty_fraction_den: u64,
}

/// Ergebnis eines angewendeten Verdicts (für die weitere Abrechnung,
/// z. B. die vTFE-Rückbuchung beim Epochenabschluss in Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerdictEffect {
    /// Geschlachteter Stake-Betrag.
    pub slashed: u64,
    /// Ausgezahltes Kopfgeld.
    pub bounty: u64,
}

/// `apply_verdict(v) → Stake slashen, Kopfgeld auszahlen, vTFE
/// rückbuchen` (Anhang A.5).
///
/// Schlachtet den Stake der schuldigen Partei (Anteil gemäß
/// `slash_fraction`), zahlt der Gegenpartei das Kopfgeld (Anteil des
/// geschlachteten Betrags gemäß `bounty_fraction`) und lässt den Rest
/// unverteilt (faktisch verbrannt). Die vTFE-Rückbuchung des Segments
/// erfolgt beim Epochenabschluss (Phase 4); dieser Übergang liefert die
/// dafür nötigen Beträge im [`VerdictEffect`].
pub fn apply_verdict(
    state: &mut LedgerState,
    verdict: &Verdict,
    params: &SlashParams,
) -> Result<VerdictEffect, TransitionError> {
    // Parameter-Prüfung.
    if params.slash_fraction_den == 0
        || params.bounty_fraction_den == 0
        || params.slash_fraction_num > params.slash_fraction_den
        || params.bounty_fraction_num > params.bounty_fraction_den
    {
        return Err(TransitionError::InvalidParameters);
    }
    let (guilty, innocent) = match verdict.outcome {
        VerdictOutcome::SlashMiner => (verdict.miner, verdict.checker),
        VerdictOutcome::SlashChecker => (verdict.checker, verdict.miner),
    };
    if guilty == innocent {
        return Err(TransitionError::InvalidParameters);
    }

    // Prüfphase: Stake vorhanden?
    let staked = state.account(&guilty).staked;
    if staked == 0 {
        return Err(TransitionError::NoStake);
    }
    let slashed = staked
        .checked_mul(params.slash_fraction_num)
        .and_then(|v| v.checked_div(params.slash_fraction_den))
        .ok_or(TransitionError::Overflow)?;
    let bounty = slashed
        .checked_mul(params.bounty_fraction_num)
        .and_then(|v| v.checked_div(params.bounty_fraction_den))
        .ok_or(TransitionError::Overflow)?;
    let innocent_balance = state.account(&innocent).balance;
    innocent_balance
        .checked_add(bounty)
        .ok_or(TransitionError::Overflow)?;

    // Änderungsphase.
    state.account_mut(&guilty).staked = staked - slashed;
    state.account_mut(&innocent).balance = innocent_balance + bounty;
    Ok(VerdictEffect { slashed, bounty })
}

/// `burn(addr, syn) → mint_credits(addr, syn / preis_e)` (Anhang A.5).
///
/// Verbrennt `syn` MYL-Kleinstbeträge vom Konto und prägt dafür
/// Inferenz-Credits: `floor(syn / credit_price)` vTFE-Einheiten,
/// gültig bis einschließlich `credit_expiry` (Protokoll-Parameter,
/// später Governance). Die Division rundet abwärts — es werden niemals
/// mehr Credits geprägt, als der Burn deckt.
///
/// Liefert die Anzahl geprägter Credits zurück.
pub fn burn_to_credits(
    state: &mut LedgerState,
    addr: &Address,
    syn: u64,
    credit_expiry: EpochId,
) -> Result<u64, TransitionError> {
    if syn == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    // Prüfphase: Deckung vorhanden?
    let available = state.account(addr).balance;
    if available < syn {
        return Err(TransitionError::InsufficientBalance {
            available,
            required: syn,
        });
    }
    let credit_price = state.credit_price;
    if credit_price == 0 {
        // Ein Credit-Preis von 0 ist ein Protokollfehler (Division durch
        // null); zu Genesis muss der Preis gesetzt sein.
        return Err(TransitionError::Overflow);
    }
    let minted = syn / credit_price;

    // Änderungsphase.
    let account = state.account_mut(addr);
    account.balance = available - syn;
    if minted > 0 {
        account.credits.push(InferenceCredit {
            owner: *addr,
            vtfe: minted,
            expiry: credit_expiry,
        });
        // Ausgabereihenfolge-Invariante: Credits stehen aufsteigend nach
        // Verfalls-Epoche (siehe `credit_spend`).
        account.credits.sort_by_key(|c| c.expiry);
    }
    Ok(minted)
}

/// `credit_spend(session, vtfe) → Session-Budget abbuchen` (Anhang A.5).
///
/// Bucht `vtfe` Einheiten vom Credit-Guthaben des Kontos ab.
/// Ausgabe-Regeln (deterministisch und konsensrelevant):
/// - Verfallene Credits (`expiry` < aktuelle Epoche) sind unbrauchbar
///   und werden beim Abbuchen entsorgt.
/// - Verbrauch in Reihenfolge des frühesten Verfalls (Credits stehen
///   aufsteigend sortiert), Teilverbrauch kürzt den betroffenen Credit.
/// - Reicht das Guthaben nicht, wird der Übergang abgelehnt und der
///   Zustand nicht verändert (Session-Kontrakte behandeln das als
///   „Budget erschöpft").
///
/// Die Session-Zuordnung (welche Session zu welchem Konto bucht) ist
/// Aufgabe der AGENT_LAYER-Kontrakte; der Ledger kennt nur das Konto.
pub fn credit_spend(
    state: &mut LedgerState,
    owner: &Address,
    vtfe: u64,
) -> Result<(), TransitionError> {
    if vtfe == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    let epoch = state.epoch;

    // Prüfphase: verfügbares (nicht verfallenes) Guthaben.
    let account = state.account(owner);
    let available = account
        .credits
        .iter()
        .filter(|c| c.expiry >= epoch)
        .fold(0u64, |sum, c| sum.saturating_add(c.vtfe));
    if available < vtfe {
        return Err(TransitionError::InsufficientCredits {
            available,
            required: vtfe,
        });
    }

    // Änderungsphase: verfallene entsorgen, dann FIFO verbrauchen.
    let account = state.account_mut(owner);
    let alt = std::mem::take(&mut account.credits);
    let mut remaining = vtfe;
    for credit in alt {
        if credit.expiry < epoch {
            continue; // verfallen — entsorgt
        }
        if remaining == 0 {
            account.credits.push(credit);
            continue;
        }
        if credit.vtfe <= remaining {
            remaining -= credit.vtfe; // vollständig verbraucht
        } else {
            account.credits.push(InferenceCredit {
                owner: credit.owner,
                vtfe: credit.vtfe - remaining,
                expiry: credit.expiry,
            });
            remaining = 0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::ids::EpochId;

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    fn state_mit_guthaben(addr: &Address, balance: u64, credit_price: u64) -> LedgerState {
        let mut state = LedgerState::genesis(credit_price);
        state.account_mut(addr).balance = balance;
        state
    }

    #[test]
    fn burn_praegt_credits_mit_boden_division() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 1_000, 7);
        // 1000 / 7 = 142 (abgerundet), Rest 6 verbrannt ohne Gegenwert.
        let minted = burn_to_credits(&mut state, &addr, 1_000, EpochId(10)).expect("burn");
        assert_eq!(minted, 142);
        assert_eq!(state.account(&addr).balance, 0);
        assert_eq!(state.account(&addr).credits.len(), 1);
        assert_eq!(state.account(&addr).credits[0].vtfe, 142);
        assert_eq!(state.account(&addr).credits[0].expiry, EpochId(10));
        assert_eq!(state.account(&addr).credits[0].owner, addr);
    }

    #[test]
    fn burn_ohne_deckung_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 50, 7);
        let davor = state.commitment();
        assert_eq!(
            burn_to_credits(&mut state, &addr, 100, EpochId(10)),
            Err(TransitionError::InsufficientBalance {
                available: 50,
                required: 100,
            })
        );
        // Zustand unverändert (atomarer Übergang).
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn burn_mit_null_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 50, 7);
        assert_eq!(
            burn_to_credits(&mut state, &addr, 0, EpochId(10)),
            Err(TransitionError::ZeroAmount)
        );
    }

    #[test]
    fn burn_unter_einem_credit_preis_praegt_null() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 6, 7);
        // 6 / 7 = 0: verbrannt, aber kein Credit geprägt.
        let minted = burn_to_credits(&mut state, &addr, 6, EpochId(10)).expect("burn");
        assert_eq!(minted, 0);
        assert!(state.account(&addr).credits.is_empty());
        assert_eq!(state.account(&addr).balance, 0);
    }

    #[test]
    fn mehrere_burns_ansammlung_in_ablaufreihenfolge() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 300, 10);
        burn_to_credits(&mut state, &addr, 100, EpochId(30)).expect("burn 1");
        burn_to_credits(&mut state, &addr, 100, EpochId(20)).expect("burn 2");
        burn_to_credits(&mut state, &addr, 100, EpochId(25)).expect("burn 3");
        let credits = &state.account(&addr).credits;
        assert_eq!(credits.len(), 3);
        // Aufsteigend nach Verfall sortiert (Invariante).
        assert_eq!(credits[0].expiry, EpochId(20));
        assert_eq!(credits[1].expiry, EpochId(25));
        assert_eq!(credits[2].expiry, EpochId(30));
        assert_eq!(state.account(&addr).balance, 0);
    }

    // --- apply_verdict ------------------------------------------------

    fn standard_params() -> SlashParams {
        // 50 % des Stakes slashen, davon 100 % als Kopfgeld.
        SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 2,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        }
    }

    fn verdict(outcome: VerdictOutcome) -> Verdict {
        Verdict {
            segment_id: SegmentId::new([9u8; 32]),
            miner: adresse(1),
            checker: adresse(2),
            outcome,
        }
    }

    #[test]
    fn verdict_slasht_miner_und_zahlt_kopfgeld() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 1_000;
        let davor_checker = state.account(&adresse(2)).balance;
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &standard_params())
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 500, bounty: 500 });
        assert_eq!(state.account(&adresse(1)).staked, 500);
        assert_eq!(state.account(&adresse(2)).balance, davor_checker + 500);
    }

    #[test]
    fn verdict_slasht_checker_symmetrisch() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(2)).staked = 800;
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashChecker), &standard_params())
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 400, bounty: 400 });
        assert_eq!(state.account(&adresse(2)).staked, 400);
        assert_eq!(state.account(&adresse(1)).balance, 400);
    }

    #[test]
    fn verdict_rest_verbleibt_unverteilt() {
        // Kopfgeld nur 50 % des geschlachteten Betrags.
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 1_000;
        let params = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 1,
            bounty_fraction_num: 1,
            bounty_fraction_den: 2,
        };
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &params)
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 1_000, bounty: 500 });
        assert_eq!(state.account(&adresse(1)).staked, 0);
        assert_eq!(state.account(&adresse(2)).balance, 500);
        // Die übrigen 500 sind nirgends gutgeschrieben (faktisch verbrannt).
    }

    #[test]
    fn verdict_ohne_stake_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        let davor = state.commitment();
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &standard_params()),
            Err(TransitionError::NoStake)
        );
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn verdict_mit_selbstbeteiligung_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 100;
        let selbst = Verdict {
            segment_id: SegmentId::new([9u8; 32]),
            miner: adresse(1),
            checker: adresse(1),
            outcome: VerdictOutcome::SlashMiner,
        };
        assert_eq!(
            apply_verdict(&mut state, &selbst, &standard_params()),
            Err(TransitionError::InvalidParameters)
        );
    }

    #[test]
    fn verdict_mit_ungueltigen_bruechen_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 100;
        let null_nenner = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 0,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        };
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &null_nenner),
            Err(TransitionError::InvalidParameters)
        );
        let ueber_eins = SlashParams {
            slash_fraction_num: 3,
            slash_fraction_den: 2,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        };
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &ueber_eins),
            Err(TransitionError::InvalidParameters)
        );
    }

    // --- credit_spend -------------------------------------------------

    fn state_mit_credits(owner: &Address, credits: &[(u64, u64)], epoch: u64) -> LedgerState {
        // (vtfe, expiry)-Paare.
        let mut state = LedgerState::genesis(10);
        state.epoch = EpochId(epoch);
        let account = state.account_mut(owner);
        for &(vtfe, expiry) in credits {
            account.credits.push(InferenceCredit {
                owner: *owner,
                vtfe,
                expiry: EpochId(expiry),
            });
        }
        account.credits.sort_by_key(|c| c.expiry);
        state
    }

    #[test]
    fn credit_spend_verbraucht_fifo_nach_verfall() {
        let addr = adresse(1);
        // Credits: 10 bis Epoche 20, 30 bis Epoche 30.
        let mut state = state_mit_credits(&addr, &[(10, 20), (30, 30)], 5);
        credit_spend(&mut state, &addr, 15).expect("spend");
        let credits = &state.account(&addr).credits;
        // Die ersten 10 sind vollständig verbraucht, von den zweiten 5.
        assert_eq!(credits.len(), 1);
        assert_eq!(credits[0].vtfe, 25);
        assert_eq!(credits[0].expiry, EpochId(30));
    }

    #[test]
    fn credit_spend_verfallene_credits_sind_unbrauchbar() {
        let addr = adresse(1);
        // 100 Einheiten, aber in Epoche 50 bereits verfallen (expiry 20).
        let mut state = state_mit_credits(&addr, &[(100, 20)], 50);
        let davor = state.commitment();
        assert_eq!(
            credit_spend(&mut state, &addr, 1),
            Err(TransitionError::InsufficientCredits {
                available: 0,
                required: 1,
            })
        );
        // Zustand unverändert (Prüfphase lehnt vor Änderung ab).
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn credit_spend_exakt_verfuegbar() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(7, 20), (8, 30)], 5);
        credit_spend(&mut state, &addr, 15).expect("spend");
        assert!(state.account(&addr).credits.is_empty());
    }

    #[test]
    fn credit_spend_unzureichend_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(5, 20)], 5);
        assert_eq!(
            credit_spend(&mut state, &addr, 6),
            Err(TransitionError::InsufficientCredits {
                available: 5,
                required: 6,
            })
        );
    }

    #[test]
    fn credit_spend_null_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(5, 20)], 5);
        assert_eq!(credit_spend(&mut state, &addr, 0), Err(TransitionError::ZeroAmount));
    }

    #[test]
    fn credit_spend_teilverbrauch_erhaelt_reihenfolge() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(10, 20), (10, 30), (10, 40)], 5);
        credit_spend(&mut state, &addr, 5).expect("spend");
        let credits = &state.account(&addr).credits;
        assert_eq!(credits.len(), 3);
        assert_eq!(credits[0].vtfe, 5);
        assert_eq!(credits[0].expiry, EpochId(20));
        assert_eq!(credits[1].vtfe, 10);
        assert_eq!(credits[2].vtfe, 10);
    }
}
