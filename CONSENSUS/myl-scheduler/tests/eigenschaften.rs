//! Eigenschaftstests über erzeugte Eingaben statt über getippte Beispiele.
//!
//! # ⚑ Wogegen diese Datei geschrieben ist
//!
//! Auf der Satzliste dieses Projekts steht „Zwei zugewiesene Pods
//! teilen kein Mitglied, Reserve eingeschlossen" mit der Stufe
//! **geprüft (Beispiele)**. Das ist die schwächere Stufe, und der
//! Unterschied ist nicht formal: Fund 42 war grün getestet, an drei
//! Beispielen, und das Bisektionsspiel nannte trotzdem systematisch die
//! falsche Layer.
//!
//! **Die Aussage trägt Stufe 1 der Verifikation.** Teilen zwei Pods
//! eines Redundanzpaars ein Mitglied, so vergleicht der
//! Redundanzvergleich zwei Ergebnisse derselben Maschine, und Stufe 1
//! ist eine Selbstbestätigung statt einer Prüfung.
//!
//! # Warum ein eigener Zufallsgenerator und nicht `proptest`
//!
//! Zehn Zeilen xorshift leisten hier dasselbe, solange die Folge
//! reproduzierbar ist, und sie kosten keine Abhängigkeit in einem
//! Crate, das den Konsens rechnet. Dieselbe Wahl wie in
//! `myl-ledger/tests/invarianten.rs`.
//!
//! ⚑ **Und der Keim steht im Fehlertext.** Ein Eigenschaftstest, der
//! bei Keim 4711 fällt und das nicht sagt, ist so schwer zu
//! reproduzieren wie ein Absturz im Betrieb.

use std::collections::HashSet;

use myl_scheduler::redundancy::assign_redundant_pods;
use myl_scheduler::shard_assignment::{Pod, Shard};
use myl_types::ids::MinerId;
use myl_types::miner::{HardwareClass, MinerRegistration};
use myl_types::node_metadata::GeoRegion;

/// xorshift64, reproduzierbar und ohne Abhängigkeit.
struct Zufall(u64);

impl Zufall {
    fn neu(keim: u64) -> Self {
        // Null ist der Fixpunkt von xorshift: Wer mit null saet, bekommt
        // immer null.
        Self(keim | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn bis(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

const ZONEN: [GeoRegion; 4] = [
    GeoRegion::Europe,
    GeoRegion::NorthAmerica,
    GeoRegion::Asia,
    GeoRegion::SouthAmerica,
];

fn registrierung(byte: u8, zone: GeoRegion) -> MinerRegistration {
    MinerRegistration {
        miner_id: MinerId::new([byte; 32]),
        hardware_class: HardwareClass::MediumGpu,
        registration_epoch: 5,
        zone,
        schluessel: myl_types::bls::BlsPublicKey([0; 48]),
        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
    }
}

fn mitglieder(p: &Pod) -> HashSet<MinerId> {
    p.mitglieder().map(|m| m.miner_id).collect()
}

/// Erzeugt Pods, die sich **absichtlich überschneiden dürfen**.
///
/// ⚑ **Das ist der Punkt der Erzeugung.** Wer nur disjunkte Pods
/// erzeugt, prüft die Bedingung nie: Dieselbe Falle wie am 2026-09-01,
/// als zwei Gegenproben nicht bissen, weil ihre **Daten** den
/// Unterschied nicht enthielten.
fn erzeuge_pods(z: &mut Zufall, pods: usize, k: usize, minerraum: u8) -> Vec<Pod> {
    (0..pods)
        .map(|p| {
            let zone = ZONEN[z.bis(ZONEN.len() as u64) as usize];
            let shards = (0..k)
                .map(|i| Shard {
                    shard_index: i as u32,
                    miner: registrierung(z.bis(minerraum as u64) as u8, zone),
                })
                .collect();
            let reserve = (0..z.bis(3))
                .map(|_| registrierung(z.bis(minerraum as u64) as u8, zone))
                .collect();
            Pod {
                pod_index: p as u32,
                shards,
                reserve,
            }
        })
        .collect()
}

/// **Jedes zugewiesene Paar ist disjunkt, Reserve eingeschlossen.**
///
/// Über erzeugte Pod-Mengen mit absichtlich überlappenden Minern, über
/// alle Pod-Zahlen von 2 bis 8, alle Pod-Größen von 2 bis 5 und
/// hundert Keime je Kombination.
#[test]
fn jedes_zugewiesene_paar_teilt_kein_mitglied() {
    let mut geprueft = 0usize;
    let mut mit_zuweisung = 0usize;
    let mut ueberlappungen_gesehen = 0usize;

    for keim in 1..=100u64 {
        for pods in 2..=8usize {
            for k in 2..=5usize {
                let mut z = Zufall::neu(keim * 1_000_003 + pods as u64 * 101 + k as u64);
                // Kleinerer Minerraum als Positionen gebraucht werden:
                // erzwingt Ueberschneidungen zwischen Pods.
                let raum = ((pods * k) / 2).max(2) as u8;
                let alle = erzeuge_pods(&mut z, pods, k, raum);

                for i in 0..alle.len() {
                    for j in (i + 1)..alle.len() {
                        if !mitglieder(&alle[i]).is_disjoint(&mitglieder(&alle[j])) {
                            ueberlappungen_gesehen += 1;
                        }
                    }
                }

                let mut saat = [0u8; 32];
                saat[..8].copy_from_slice(&keim.to_le_bytes());
                geprueft += 1;

                let Ok(zuteilung) = assign_redundant_pods(16, &alle, &saat) else {
                    continue;
                };
                mit_zuweisung += 1;

                for zuw in &zuteilung.zuweisungen {
                    assert!(
                        mitglieder(&alle[zuw.primary_pod_index as usize])
                            .is_disjoint(&mitglieder(&alle[zuw.redundant_pod_index as usize])),
                        "Keim {keim}, {pods} Pods, k={k}: Paar ({}, {}) teilt ein Mitglied. \
                         Stufe 1 der Verifikation waere hier eine Selbstbestaetigung.",
                        zuw.primary_pod_index,
                        zuw.redundant_pod_index
                    );
                }
            }
        }
    }

    println!(
        "[eigenschaften] {geprueft} Pod-Mengen erzeugt, {mit_zuweisung} mit Zuweisung, \
         {ueberlappungen_gesehen} ueberlappende Paare in den Eingaben"
    );
    // ⚑ Eine Pruefung, die nichts auswaehlt, sieht aus wie eine, die
    // nichts findet. Beide Zahlen muessen ueber null liegen, sonst hat
    // der Test entweder nie zugewiesen oder nie eine Ueberlappung
    // gesehen, gegen die er geschrieben ist.
    assert!(mit_zuweisung > 0, "keine einzige Zuweisung zustande gekommen");
    assert!(
        ueberlappungen_gesehen > 0,
        "die Eingaben enthielten keine einzige Ueberlappung; dann prueft dieser \
         Test die Bedingung nie"
    );
}

/// **Kein Pod wird mit sich selbst gepaart.**
///
/// Trivial und deshalb leicht zu übersehen: Ein Paar (i, i) wäre zu sich
/// selbst nie disjunkt und würde die Prüfung oben gar nicht erst
/// erreichen. Der Test hält fest, dass es solche Paare nicht gibt.
#[test]
fn kein_pod_wird_mit_sich_selbst_gepaart() {
    for keim in 1..=50u64 {
        let mut z = Zufall::neu(keim);
        let alle = erzeuge_pods(&mut z, 6, 3, 12);
        let mut saat = [0u8; 32];
        saat[..8].copy_from_slice(&keim.to_le_bytes());
        if let Ok(zuteilung) = assign_redundant_pods(16, &alle, &saat) {
            for zuw in &zuteilung.zuweisungen {
                assert_ne!(
                    zuw.primary_pod_index, zuw.redundant_pod_index,
                    "Keim {keim}: Pod mit sich selbst gepaart"
                );
            }
        }
    }
}

/// **Dieselbe Saat ergibt dieselbe Zuteilung, eine andere eine andere.**
///
/// Die erste Hälfte ist Konsens: Zwei Knoten mit derselben Saat müssen
/// zur selben Zuteilung kommen. ⚑ **Die zweite Hälfte ist die
/// Gegenprobe**, ohne die die erste auch für eine Funktion gälte, die
/// die Saat gar nicht liest.
#[test]
fn die_saat_bestimmt_die_zuteilung_und_wird_gelesen() {
    let mut z = Zufall::neu(7);
    let alle = erzeuge_pods(&mut z, 8, 3, 40);

    let erste = assign_redundant_pods(64, &alle, &[1u8; 32]).expect("Zuteilung a");
    let wieder = assign_redundant_pods(64, &alle, &[1u8; 32]).expect("Zuteilung a erneut");
    let andere = assign_redundant_pods(64, &alle, &[2u8; 32]).expect("Zuteilung b");

    assert_eq!(
        erste.zuweisungen, wieder.zuweisungen,
        "dieselbe Saat muss dieselbe Zuteilung ergeben, sonst gibt es keinen Konsens"
    );
    assert_ne!(
        erste.zuweisungen, andere.zuweisungen,
        "zwei verschiedene Saaten ergaben dieselbe Zuteilung; dann liest die \
         Funktion die Saat nicht, und dieser Test prueft nichts"
    );
}
