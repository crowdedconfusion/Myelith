//! Deterministische State-Machine-Tests (Punkt 1.5).
//!
//! Akzeptanzkriterium Phase 1: gleiche Übergangsfolge ⇒ gleicher
//! Endzustand, auf zwei unabhängigen Läufen — die direkte Analogie zu
//! INTEGER_LLMs Golden-Vector-Prinzip, hier auf Konsens-Ebene. Der
//! Testaufbau erzeugt zwei frische `LedgerState`-Instanzen (voneinander
//! unabhängige „Implementierungsläufe"), wendet dieselbe gemischte
//! Übergangsfolge an und vergleicht die State-Commitments bitgenau.

use myl_ledger::{
    apply_verdict, burn_to_credits, credit_spend, LedgerState, SlashParams, Verdict,
    VerdictOutcome,
};
use myl_types::ids::{Address, EpochId, SegmentId};

fn adresse(byte: u8) -> Address {
    Address::new([byte; 32])
}

/// Ein Eintrag der Übergangsfolge (repräsentiert einen Block-Ausschnitt).
enum Op {
    Burn { addr: Address, syn: u64, expiry: u64 },
    Spend { addr: Address, vtfe: u64 },
    Verdict { miner: Address, checker: Address, outcome: VerdictOutcome },
    Stake { addr: Address, amount: u64 },
    Epoch(u64),
}

/// Spielt eine Übergangsfolge auf einem frischen Ledger ab.
fn replay(ops: &[Op]) -> LedgerState {
    let mut state = LedgerState::genesis(10);
    let params = SlashParams {
        slash_fraction_num: 1,
        slash_fraction_den: 3,
        bounty_fraction_num: 1,
        bounty_fraction_den: 2,
    };
    for op in ops {
        match op {
            Op::Burn { addr, syn, expiry } => {
                let _ = burn_to_credits(&mut state, addr, *syn, EpochId(*expiry));
            }
            Op::Spend { addr, vtfe } => {
                let _ = credit_spend(&mut state, addr, *vtfe);
            }
            Op::Verdict { miner, checker, outcome } => {
                let verdict = Verdict {
                    segment_id: SegmentId::new([7u8; 32]),
                    miner: *miner,
                    checker: *checker,
                    outcome: *outcome,
                };
                let _ = apply_verdict(&mut state, &verdict, &params);
            }
            Op::Stake { addr, amount } => {
                state.account_mut(addr).staked =
                    state.account(addr).staked.saturating_add(*amount);
            }
            Op::Epoch(e) => {
                state.epoch = EpochId(*e);
            }
        }
    }
    state
}

/// Deterministische Pseudozufallsfolge (LCG) — reproduzierbar, keine
/// Zufalls-Abhängigkeit in der Testsuite.
fn gemischte_folge(seed: u64, n: usize) -> Vec<Op> {
    let mut state = seed;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    let mut ops = Vec::with_capacity(n);
    for i in 0..n {
        let wahl = next() % 5;
        let wer = adresse((next() % 8 + 1) as u8);
        let betrag = next() % 500 + 1;
        match wahl {
            0 => ops.push(Op::Stake { addr: wer, amount: betrag }),
            1 => ops.push(Op::Burn {
                addr: wer,
                syn: betrag * 3,
                expiry: 100 + next() % 50,
            }),
            2 => ops.push(Op::Spend { addr: wer, vtfe: next() % 20 + 1 }),
            3 => {
                let checker = adresse((next() % 8 + 9) as u8);
                let outcome = if next() % 2 == 0 {
                    VerdictOutcome::SlashMiner
                } else {
                    VerdictOutcome::SlashChecker
                };
                ops.push(Op::Verdict {
                    miner: wer,
                    checker,
                    outcome,
                });
            }
            _ => ops.push(Op::Epoch(1 + (i as u64) % 200)),
        }
    }
    ops
}

#[test]
fn replay_zweier_unabhaengiger_laeufe_ist_bitgleich() {
    // Kurze, handgebaute Folge.
    let handfolge = vec![
        Op::Stake { addr: adresse(1), amount: 1_000 },
        Op::Stake { addr: adresse(2), amount: 500 },
        Op::Burn { addr: adresse(1), syn: 250, expiry: 80 },
        Op::Burn { addr: adresse(2), syn: 100, expiry: 40 },
        Op::Spend { addr: adresse(1), vtfe: 10 },
        Op::Epoch(5),
        Op::Spend { addr: adresse(1), vtfe: 5 },
        Op::Verdict {
            miner: adresse(1),
            checker: adresse(2),
            outcome: VerdictOutcome::SlashMiner,
        },
        Op::Burn { addr: adresse(2), syn: 30, expiry: 90 },
        Op::Spend { addr: adresse(2), vtfe: 7 },
    ];
    let lauf_a = replay(&handfolge);
    let lauf_b = replay(&handfolge);
    assert_eq!(lauf_a, lauf_b);
    assert_eq!(lauf_a.commitment(), lauf_b.commitment());
}

#[test]
fn replay_grosser_pseudozufallsfolge_ist_bitgleich() {
    // 1.000 gemischte Übergänge, zwei unabhängige Läufe.
    let folge = gemischte_folge(0x5EED, 1_000);
    let lauf_a = replay(&folge);
    let lauf_b = replay(&folge);
    assert_eq!(lauf_a.commitment(), lauf_b.commitment());
    // Ein anderer Seed ergibt eine andere Folge und einen anderen Zustand.
    let andere = replay(&gemischte_folge(0xBEEF, 1_000));
    assert_ne!(lauf_a.commitment(), andere.commitment());
}

#[test]
fn serialisierungsrundtrip_erhaelt_das_commitment() {
    let folge = gemischte_folge(42, 300);
    let state = replay(&folge);
    let bytes = borsh::to_vec(&state).expect("Serialisierung");
    let zurueck: LedgerState = borsh::from_slice(&bytes).expect("Deserialisierung");
    assert_eq!(zurueck, state);
    assert_eq!(zurueck.commitment(), state.commitment());
}
