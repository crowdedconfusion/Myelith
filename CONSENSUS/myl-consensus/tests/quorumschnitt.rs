//! **Zwei Quoren schneiden sich in mehr als einem Drittel des Gewichts**,
//! bei ungleichen Gewichten und mit den Rundungen der echten Schwelle
//! (2026-10-26).
//!
//! # ⚑ Warum diese Datei
//!
//! Der Auditzuschnitt nennt Quorum, Sperre und Rundenwechsel die am
//! wenigsten maschinell abgesicherte Stelle des Projekts, weil die Aussage
//! über Mengen von Validatoren gilt und nicht über eine Funktion. Ein Teil
//! davon lässt sich trotzdem über den echten Code prüfen: die Schwelle.
//! Für zufällige Gewichte und für die ungünstigsten Paare (zwei Quoren mit
//! dem kleinsten möglichen Schnitt) muss gelten:
//!
//! - Schnitt zweier Quoren: dreimal sein Gewicht liegt über dem
//!   Gesamtgewicht, also ist mindestens ein Ehrlicher darin, solange die
//!   Byzantinischen höchstens ein Drittel halten.
//! - Wer die Drittelschranke erreicht, hält mehr als ein Drittel.
//!
//! Gezählt wird, was das Protokoll zählt (`VotingSet::weight`); gemessen
//! wird der Schnitt gegen die **wahre** Summe der Gewichte in u128, nicht
//! gegen das, was `VotingSet` speichert.

use myl_consensus::validator::{VotingMember, VotingSet};
use myl_types::bls::BlsPublicKey;
use myl_types::ids::MinerId;
use std::collections::BTreeMap;

/// xorshift64, reproduzierbar und ohne Abhängigkeit.
struct Wuerfel(u64);
impl Wuerfel {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.naechste() % n.max(1)
    }
}

fn kennung(i: usize) -> MinerId {
    MinerId::new([i as u8 + 1; 32])
}

fn menge(gewichte: &[u64]) -> VotingSet {
    let mut m = BTreeMap::new();
    for (i, g) in gewichte.iter().enumerate() {
        m.insert(kennung(i), VotingMember { pubkey: BlsPublicKey([i as u8; 48]), weight: *g });
    }
    VotingSet::from_members(m)
}

/// Das kleinste Quorum von vorne und das kleinste von hinten (nach einer
/// Reihenfolge der Mitglieder): zwei Quoren mit möglichst kleinem Schnitt.
fn randquoren(gewichte: &[u64], reihenfolge: &[usize], q: u128) -> Option<(Vec<bool>, Vec<bool>)> {
    let nimm = |iter: &mut dyn Iterator<Item = &usize>| {
        let mut drin = vec![false; gewichte.len()];
        let mut summe = 0u128;
        for &i in iter {
            if summe >= q {
                break;
            }
            drin[i] = true;
            summe += gewichte[i] as u128;
        }
        (summe >= q).then_some(drin)
    };
    let a = nimm(&mut reihenfolge.iter())?;
    let b = nimm(&mut reihenfolge.iter().rev())?;
    Some((a, b))
}

fn gewicht(gewichte: &[u64], drin: &[bool]) -> u128 {
    gewichte.iter().zip(drin).filter(|(_, d)| **d).map(|(g, _)| *g as u128).sum()
}

fn pruefe(gewichte: &[u64], w: &mut Wuerfel, fall: &str) {
    let set = menge(gewichte);
    let wahr: u128 = gewichte.iter().map(|g| *g as u128).sum();
    if wahr == 0 {
        return;
    }
    let q = set.quorum_threshold() as u128;
    let drittel = set.drittel_schranke() as u128;
    // Was das Protokoll je Mitglied zählt.
    let gezaehlt: Vec<u64> = (0..gewichte.len()).map(|i| set.weight(&kennung(i))).collect();
    let mut reihenfolge: Vec<usize> = (0..gewichte.len()).collect();
    for runde in 0..40 {
        // Eine zufällige Reihenfolge, dazu aufsteigend und absteigend nach Gewicht.
        match runde {
            0 => reihenfolge.sort_by_key(|i| gewichte[*i]),
            1 => reihenfolge.sort_by_key(|i| std::cmp::Reverse(gewichte[*i])),
            _ => {
                for i in (1..reihenfolge.len()).rev() {
                    reihenfolge.swap(i, w.bis(i as u64 + 1) as usize);
                }
            }
        }
        if let Some((a, b)) = randquoren(&gezaehlt, &reihenfolge, q) {
            let schnitt: Vec<bool> = a.iter().zip(&b).map(|(x, y)| *x && *y).collect();
            let s = gewicht(gewichte, &schnitt);
            assert!(
                3 * s > wahr,
                "{fall}: zwei Quoren (Schwelle {q}) schneiden sich nur in {s} von {wahr}; Gewichte {gewichte:?}"
            );
        }
        // Die Drittelschranke: eine zufällige Teilmenge, die sie erreicht.
        let drin: Vec<bool> = (0..gewichte.len()).map(|_| w.bis(2) == 0).collect();
        let g = gewicht(gewichte, &drin);
        if gewicht(&gezaehlt, &drin) >= drittel {
            assert!(3 * g > wahr, "{fall}: {g} erreicht die Drittelschranke {drittel}, ist aber kein Drittel von {wahr}");
        }
    }
}

#[test]
fn zwei_quoren_schneiden_sich_in_mehr_als_einem_drittel() {
    let mut w = Wuerfel(0x2545_f491_4f6c_dd1d);
    for fall in 0..3_000 {
        let n = 1 + w.bis(12) as usize;
        let gewichte: Vec<u64> = (0..n)
            .map(|_| match w.bis(4) {
                0 => 1 + w.bis(10),
                1 => 1 + w.bis(1_000_000),
                2 => w.bis(3),
                _ => 1 + w.bis(1 << 40),
            })
            .collect();
        pruefe(&gewichte, &mut w, &format!("Fall {fall}"));
    }
}

/// ⚑ **Gewichte, deren Summe über `u64` hinausgeht.** Im Betrieb begrenzt
/// die Geldmenge den Stake und damit die Summe; hier steht, was passiert,
/// wenn diese Schranke einmal nicht hält.
#[test]
fn auch_riesige_gewichte_schneiden_sich() {
    let mut w = Wuerfel(0x9e37_79b9_7f4a_7c15);
    for fall in 0..500 {
        let n = 2 + w.bis(6) as usize;
        let gewichte: Vec<u64> = (0..n).map(|_| u64::MAX / 2 + w.bis(u64::MAX / 2)).collect();
        pruefe(&gewichte, &mut w, &format!("Riesenfall {fall}"));
        // Und was stattdessen gilt: Niemand erreicht ein Quorum.
        let set = menge(&gewichte);
        let alle: u128 = (0..n).map(|i| set.weight(&kennung(i)) as u128).sum();
        assert!(alle < set.quorum_threshold() as u128, "Riesenfall {fall}: ein Quorum ist erreichbar");
    }
}
