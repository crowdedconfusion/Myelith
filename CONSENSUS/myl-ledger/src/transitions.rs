//! Zustandsübergänge des Ledgers (Anhang A.5).
//!
//! Jeder Übergang ist eine reine Funktion `(State, …) → State` ohne
//! versteckten globalen Zustand; Fehler lassen den Zustand unverändert
//! (Übergänge prüfen zuerst und ändern erst dann).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::ids::{Address, EpochId};
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
    /// Eine Buchung würde den `u64`-Bereich überschreiten.
    Overflow,
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
            Self::Overflow => write!(f, "Buchung würde den Wertebereich überschreiten"),
        }
    }
}

impl std::error::Error for TransitionError {}

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
}
