//! Kontenmodell und Ledger-Zustand (Punkt 1.1).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::ids::{Address, EpochId};
use myl_types::Hash;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Zustand eines Kontos (Adresse).
///
/// - `balance`: verfügbare MYL-Kleinstbeträge
/// - `staked`: als Validator-/Miner-Stake gebundene MYL-Kleinstbeträge
/// - `credits`: noch nicht verbrauchte Inferenz-Credits (vTFE),
///   aufsteigend nach Verfalls-Epoche geordnet (Ausgabereihenfolge:
///   zuerst verfallende Credits, siehe `credit_spend`).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AccountState {
    pub balance: u64,
    pub staked: u64,
    pub credits: Vec<myl_types::InferenceCredit>,
}

impl AccountState {
    /// Leeres Konto.
    pub fn empty() -> Self {
        Self {
            balance: 0,
            staked: 0,
            credits: Vec::new(),
        }
    }
}

/// Der vollständige Ledger-Zustand.
///
/// `accounts` ist eine `BTreeMap` — die deterministische Ordnung ist
/// Konsens-Eigenschaft (siehe Modul-Dokumentation in `lib.rs`).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct LedgerState {
    /// Aktuelle Epoche.
    pub epoch: EpochId,
    /// Credit-Preis: MYL-Kleinstbeträge je vTFE-Einheit
    /// (später von TOKENOMICS aktualisiert; Startwert zu Genesis).
    pub credit_price: u64,
    /// Konten, deterministisch nach Adresse geordnet.
    pub accounts: BTreeMap<Address, AccountState>,
}

impl LedgerState {
    /// Genesis-Leerzustand mit gegebenem Credit-Preis.
    pub fn genesis(credit_price: u64) -> Self {
        Self {
            epoch: EpochId(0),
            credit_price,
            accounts: BTreeMap::new(),
        }
    }

    /// Konto lesen oder das leere Konto liefern (liest nie zustands-
    /// verändernd; `account_mut` für Übergänge).
    pub fn account(&self, addr: &Address) -> AccountState {
        self.accounts.get(addr).cloned().unwrap_or_else(AccountState::empty)
    }

    /// Konto zur Veränderung lesen bzw. anlegen.
    pub fn account_mut(&mut self, addr: &Address) -> &mut AccountState {
        self.accounts.entry(*addr).or_insert_with(AccountState::empty)
    }

    /// Kanonische Zustands-Verpflichtung: SHA-256 über die
    /// Borsh-Serialisierung. Borsh ist kanonisch und die Kontenordnung
    /// fest — gleiche Zustände ergeben auf jedem Node dieselben Bytes
    /// und damit denselben Hash (Grundlage für spätere
    /// Cross-Node-Konsistenzprüfung und Block-Commitments).
    pub fn commitment(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Ledger-Zustand ist stets serialisierbar");
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Hash::from_bytes(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    #[test]
    fn genesis_zustand_ist_leer_und_bekommt_konten() {
        let mut state = LedgerState::genesis(100);
        assert_eq!(state.epoch, EpochId(0));
        assert_eq!(state.accounts.len(), 0);
        assert_eq!(state.account(&adresse(1)), AccountState::empty());
        state.account_mut(&adresse(1)).balance = 500;
        assert_eq!(state.account(&adresse(1)).balance, 500);
        assert_eq!(state.accounts.len(), 1);
    }

    #[test]
    fn commitment_ist_deterministisch_und_unterscheidet_zustaende() {
        let mut a = LedgerState::genesis(100);
        let mut b = LedgerState::genesis(100);
        assert_eq!(a.commitment(), b.commitment());

        a.account_mut(&adresse(1)).balance = 10;
        b.account_mut(&adresse(1)).balance = 10;
        // Gleiche Änderungen, unabhängig voneinander ausgeführt:
        assert_eq!(a.commitment(), b.commitment());

        b.account_mut(&adresse(1)).balance = 11;
        assert_ne!(a.commitment(), b.commitment());
    }

    #[test]
    fn kontenordnung_ist_unabhaengig_von_einfuegereihenfolge() {
        // BTreeMap: dieselbe Zielmenge ergibt dieselbe Serialisierung,
        // egal in welcher Reihenfolge die Konten angelegt wurden.
        let mut aufsteigend = LedgerState::genesis(100);
        for b in [1u8, 2, 3] {
            aufsteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        let mut absteigend = LedgerState::genesis(100);
        for b in [3u8, 2, 1] {
            absteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        assert_eq!(aufsteigend.commitment(), absteigend.commitment());
    }
}
