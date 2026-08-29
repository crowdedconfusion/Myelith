//! Eigenschaftstests für die Allaussagen der Verteilung.
//!
//! ## ⚑ Warum
//!
//! `die_summe_stimmt_immer_exakt` heißt der Test, der eine Summe prüft.
//! **Der Name führt die Regel, die Prüfung sah eine Stichprobe.** Bei
//! Geld ist das die teuerste Stelle für diese Schwäche: Ein
//! Verteilverfahren, das in einem von tausend Fällen eine Einheit
//! verliert oder erfindet, fällt an keinem getippten Beispiel auf und
//! bricht die Invariante „die Prägung wird vollständig verteilt".
//!
//! ## Was hier erschöpfend geht und was nicht
//!
//! `distribute_mint` hat **einen** Parameter, also lässt sich ein
//! dichter Bereich vollständig abgehen. `split_proportional` hat einen
//! Betrag und eine Gewichtsliste; dort läuft ein **deterministischer**
//! Generator mit festem Keim, damit ein Fehlschlag wiederholbar ist.

use myl_tokenomics::distribute::{distribute_mint, split_proportional};
use myl_types::ids::Address;

struct Folge(u64);

impl Folge {
    fn neu(keim: u64) -> Self {
        Self(keim ^ 0x9E3779B97F4A7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn adresse(n: u64) -> Address {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&n.to_be_bytes());
    Address::new(b)
}

/// ⚑ **Erschöpfend über alle kleinen Prägungen.** Rundungsreste
/// entstehen genau dort, wo die Beträge klein sind; bei großen Zahlen
/// verschwindet der Rest im Verhältnis und ein Fehler bliebe unsichtbar.
#[test]
fn die_summe_stimmt_erschoepfend_fuer_jede_kleine_praegung() {
    for m_e in 0u64..=100_000 {
        let d = distribute_mint(m_e);
        assert_eq!(d.summe(), m_e, "Prägung {m_e}");
    }
}

/// Und über den ganzen Bereich, mit den Rändern.
#[test]
fn die_summe_stimmt_ueber_den_ganzen_bereich() {
    let mut f = Folge::neu(0xD1571B);
    for _ in 0..200_000 {
        let m_e = f.next();
        assert_eq!(distribute_mint(m_e).summe(), m_e, "Prägung {m_e}");
    }
    for m_e in [0u64, 1, 2, 9999, 10_000, 10_001, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
        assert_eq!(distribute_mint(m_e).summe(), m_e, "Prägung {m_e}");
    }
}

/// ⚑ **Kein Anteil ist je größer als das Ganze**, und das ist keine
/// Selbstverständlichkeit: Der Rundungsrest geht geschlossen ans
/// Treasury, dessen Anteil also über seinen Sollwert steigen kann.
#[test]
fn kein_anteil_uebersteigt_je_die_praegung() {
    let mut f = Folge::neu(0xBEEF01);
    for i in 0..100_000u64 {
        let m_e = if i < 1000 { i } else { f.next() };
        let d = distribute_mint(m_e);
        for (name, wert) in [
            ("shard_miners", d.shard_miners),
            ("coordinators", d.coordinators),
            ("validators", d.validators),
            ("checkers", d.checkers),
            ("treasury", d.treasury),
        ] {
            assert!(wert <= m_e, "{name} = {wert} > Prägung {m_e}");
        }
    }
}

/// ⚑ **Die Summe der Auszahlungen ergibt stets den Betrag**, für jede
/// Gewichtsverteilung, jede Empfängerzahl und jeden Betrag.
///
/// Der Generator erzeugt ausdrücklich auch die unbequemen Fälle: viele
/// Nullgewichte, ein einziges Gewicht, doppelte Adressen, Beträge
/// kleiner als die Empfängerzahl.
#[test]
fn die_auszahlungen_summieren_sich_immer_auf_den_betrag() {
    let mut f = Folge::neu(0x5F17_2EED);
    let mut mit_rest = 0u32;
    for _ in 0..20_000 {
        let n = (f.next() % 12) as usize + 1;
        let gewichte: Vec<(Address, u64)> = (0..n)
            .map(|i| {
                // Adressen absichtlich mit Dubletten: `% (n as u64 / 2 + 1)`.
                let a = adresse(f.next() % (n as u64 / 2 + 1));
                // Viele Nullen, damit der Rest-Verteilweg getroffen wird.
                let w = if f.next() % 3 == 0 { 0 } else { f.next() % 1000 };
                let _ = i;
                (a, w)
            })
            .collect();
        let total = f.next() % 5000;

        // Ein Fehler ist zulässig, etwa wenn alle Gewichte null sind;
        // verboten ist eine **falsche Summe**.
        if let Ok(auszahlungen) = split_proportional(total, &gewichte) {
            let summe: u64 = auszahlungen.values().sum();
            assert_eq!(summe, total, "Summe {summe} statt {total}, Gewichte {gewichte:?}");
            // Wer nichts beiträgt, bekommt nichts.
            for (a, w) in &gewichte {
                if *w == 0 && !gewichte.iter().any(|(b, x)| b == a && *x > 0) {
                    assert_eq!(
                        auszahlungen.get(a).copied().unwrap_or(0),
                        0,
                        "Nullgewicht bekam etwas"
                    );
                }
            }
            if total % (n as u64) != 0 {
                mit_rest += 1;
            }
        }
    }
    // ⚑ Zählen, dass der interessante Fall überhaupt vorkam. Ein Test,
    // der nur glatte Teilungen sieht, prüft die Restverteilung nie.
    assert!(mit_rest > 5_000, "nur {mit_rest} Fälle mit Rundungsrest");
}
