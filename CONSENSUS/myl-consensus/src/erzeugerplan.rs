//! **Der Erzeugerplan** (2026-11-04, Ebb-and-Flow): Wer den Block im Slot
//! `s` erzeugen darf, folgt aus der Saat der Epoche und dem Satz, gewichtet
//! nach Einsatz.
//!
//! ⚑ **Warum ein Plan und kein fester Erzeuger:** Seit die Prüfpunkte aller
//! die Sicherheit tragen, muss der Erzeuger die Kette nur lebendig halten.
//! Ein fester Erzeuger kann allein zensieren und hält die Kette an, wenn er
//! ausfällt; ein Plan aus einer Saat, die niemand wählt, gibt jedem
//! Mitglied Slots nach seinem Gewicht, und fällt einer aus, bleibt nur sein
//! Slot leer.
//!
//! ⚑ **Slots nach der Uhr, nicht Versuche je Höhe** (Befund beim Bau,
//! 2026-11-04): Ein Versuch ohne Zeit liesse jedes Mitglied sofort beliebig
//! viele gültige Blöcke bauen (es sucht je Höhe den Versuch, in dem es
//! gezogen ist), und der längere Zweig überholte die ehrliche Kette und
//! spulte die Epochen vor. Ein Slot ist ein Zeitabschnitt (Slot 0 beginnt
//! mit der Unix-Zeit); ein Zweig wächst höchstens um einen Block je Slot
//! seiner Erzeuger, wie in Ouroboros Praos, BABE und Gasper.
//!
//! **Die Uhr bleibt aus dem Zustand:** Die Kette prüft nur, dass der Slot
//! nach dem des Vorgängers liegt und der Unterzeichner für ihn gezogen ist.
//! Dass ein Slot schon begonnen hat, prüft der Knoten beim Annehmen an der
//! eigenen Uhr; ein Block aus der Zukunft wartet, statt ungültig zu sein.

use crate::validator::VotingSet;
use myl_types::{Hash, MinerId};

/// Domäne der Ziehung.
pub const DST_ERZEUGERPLAN: &[u8] = b"MYELITH_ERZEUGERPLAN_v1";

/// Domäne der Saat aus `β`.
pub const DST_ERZEUGERSAAT: &[u8] = b"MYELITH_ERZEUGERSAAT_v1";

/// **Die Saat des Plans aus `β`** (die Saatsignatur einer Epoche, schon zu
/// Bytes verdichtet): eine eigene Domäne, damit sie mit keiner anderen
/// Saat aus demselben `β` zusammenfällt.
pub fn saat_aus_beta(beta_saat: &[u8; 32]) -> Hash {
    let mut b = Vec::with_capacity(DST_ERZEUGERSAAT.len() + 32);
    b.extend_from_slice(DST_ERZEUGERSAAT);
    b.extend_from_slice(beta_saat);
    Hash::sha256(&b)
}

/// Der Slot, in dem der Zeitpunkt `ms` (Unix-Millisekunden) liegt.
pub fn slot_bei(ms: u64, slot_ms: u64) -> u64 {
    ms / slot_ms.max(1)
}

/// Der Beginn des Slots `slot` in Unix-Millisekunden.
pub fn slotbeginn(slot: u64, slot_ms: u64) -> u64 {
    slot.saturating_mul(slot_ms.max(1))
}

/// **Wer im Slot `slot` erzeugt.**
///
/// `x = H(DST ‖ saat ‖ s)`, die ersten 16 Bytes als Zahl, modulo dem
/// Gesamtgewicht; gezogen ist das Mitglied, in dessen Abschnitt der
/// aufsummierten Gewichte `x` fällt, in der Ordnung der Kennungen. Die
/// Verzerrung durch den Rest ist kleiner als `Gesamtgewicht / 2^128`.
/// `None` für einen leeren Satz oder einen ohne Gewicht.
///
/// ⚑ **Nur der Slot, nicht die Höhe**: Hinge die Ziehung an der Höhe, könnte
/// ein Erzeuger durch Bauen oder Auslassen wählen, wer nach ihm dran ist.
pub fn ziehen(saat: &Hash, slot: u64, satz: &VotingSet) -> Option<MinerId> {
    let gesamt = u128::from(satz.total_weight());
    if gesamt == 0 {
        return None;
    }
    let mut b = Vec::with_capacity(DST_ERZEUGERPLAN.len() + 32 + 8);
    b.extend_from_slice(DST_ERZEUGERPLAN);
    b.extend_from_slice(saat.as_bytes());
    b.extend_from_slice(&slot.to_le_bytes());
    let h = Hash::sha256(&b);
    let mut zahl = [0u8; 16];
    zahl.copy_from_slice(&h.as_bytes()[..16]);
    let mut x = u128::from_le_bytes(zahl) % gesamt;
    for (kennung, m) in satz.members() {
        let w = u128::from(m.weight);
        if x < w {
            return Some(*kennung);
        }
        x -= w;
    }
    None
}

/// **Der erste Slot in `ab..=bis`, in dem `wer` gezogen ist.**
pub fn naechster_slot(saat: &Hash, ab: u64, bis: u64, satz: &VotingSet, wer: &MinerId) -> Option<u64> {
    (ab..=bis).find(|s| ziehen(saat, *s, satz).as_ref() == Some(wer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::VotingMember;
    use std::collections::BTreeMap;

    fn satz(gewichte: &[u64]) -> VotingSet {
        let mut m = BTreeMap::new();
        for (i, w) in gewichte.iter().enumerate() {
            let sk = myl_types::bls::BlsSecretKey::key_gen(&[i as u8 + 1; 32]).expect("Schluessel");
            let pk = sk.public_key().expect("oeffentlich");
            m.insert(MinerId::aus_schluessel(&pk), VotingMember { pubkey: pk, weight: *w });
        }
        VotingSet::from_members(m)
    }

    /// ⚑ **Jeder kommt nach seinem Gewicht dran**, und dieselbe Eingabe
    /// zieht immer denselben.
    #[test]
    fn gezogen_wird_nach_gewicht() {
        let s = satz(&[1, 3, 6]);
        let saat = Hash::sha256(b"saat");
        let mut je: BTreeMap<MinerId, u32> = BTreeMap::new();
        for slot in 0..6_000u64 {
            let wer = ziehen(&saat, slot, &s).expect("gezogen");
            assert_eq!(Some(wer), ziehen(&saat, slot, &s));
            *je.entry(wer).or_default() += 1;
        }
        let mut anteile: Vec<(u64, u32)> = s.members().map(|(k, m)| (m.weight, je.get(k).copied().unwrap_or(0))).collect();
        anteile.sort();
        // Erwartet 600, 1800, 3600 von 6000; Spielraum gut fünf Sigma.
        assert!((480..=720).contains(&anteile[0].1), "{anteile:?}");
        assert!((1630..=1970).contains(&anteile[1].1), "{anteile:?}");
        assert!((3410..=3790).contains(&anteile[2].1), "{anteile:?}");
    }

    /// ⚑ **Slot und Saat ändern die Ziehung**, ein leerer Satz zieht
    /// niemanden, ein Mitglied ohne Gewicht nie, und der nächste eigene
    /// Slot ist der erste, in dem man gezogen ist.
    #[test]
    fn slot_und_saat_ziehen_neu() {
        let s = satz(&[5, 5, 5, 0]);
        let a = Hash::sha256(b"a");
        let b = Hash::sha256(b"b");
        let reihe = |saat: &Hash, ab: u64| (ab..ab + 64).map(|slot| ziehen(saat, slot, &s)).collect::<Vec<_>>();
        assert_ne!(reihe(&a, 0), reihe(&a, 64));
        assert_ne!(reihe(&a, 0), reihe(&b, 0));
        let ohne: Vec<MinerId> = s.members().filter(|(_, m)| m.weight == 0).map(|(k, _)| *k).collect();
        assert!((0..512u64).all(|slot| ziehen(&a, slot, &s) != Some(ohne[0])));
        assert_eq!(ziehen(&a, 0, &VotingSet::from_members(BTreeMap::new())), None);
        let wer = ziehen(&a, 9, &s).expect("gezogen");
        let erster = naechster_slot(&a, 0, 9, &s, &wer).expect("bis 9 dran");
        assert!((0..erster).all(|slot| ziehen(&a, slot, &s) != Some(wer)));
        assert_eq!(naechster_slot(&a, 0, 512, &s, &ohne[0]), None);
        assert_ne!(saat_aus_beta(&[1; 32]), saat_aus_beta(&[2; 32]));
        assert_eq!(slot_bei(1_999, 500), 3);
        assert_eq!(slotbeginn(4, 500), 2_000);
    }
}
