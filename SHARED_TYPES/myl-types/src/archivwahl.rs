//! **Die Ziehung der Archivknoten** (Stufe A3, 2026-10-23): wer in einer
//! Periode die volle Geschichte hält (Kopien) und wer Teile ihrer
//! Codierung (Splitter).
//!
//! # ⚑ Warum in `myl-types`
//!
//! Jeder Knoten, und ab Stufe A5 der Ledger, muss für jeden Miner dasselbe
//! ausrechnen: wer belegen muss, wer belohnt wird, wer ersetzt wird. Die
//! Ziehung ist deshalb eine reine Funktion des Konsenszustands: Epochensaat
//! an der Langzeitgrenze, Register der Miner, Epoche der Grenze.
//!
//! # ⚑ Die Regeln
//!
//! 1. **Teilnehmen darf nur, wer früh genug registriert war**: mindestens
//!    `warte_epochen` vor der Grenze. Wer sich erst nach dem Festlegen der
//!    Saat anmeldet, könnte seinen Schlüssel auf sie hin ausprobieren.
//! 2. **Das Los** `sha256(art ‖ saat ‖ kennung)`, mit verschiedener `art`
//!    für Kopien und Splitter, also zwei unabhängige Ziehungen.
//! 3. **[`KOPIEN`] Kopien, über die Zonen verteilt**: je Zone nach Los
//!    geordnet, dann reihum aus jeder Zone die beste, bis die Zahl steht.
//!    Ein Brand, ein Stromausfall, ein Zugriff in einer Region trifft so
//!    höchstens einen Teil.
//! 4. **[`SPLITTER`] Splitter** aus den übrigen, ebenso verteilt. Wer Kopie
//!    ist, ist nicht zugleich Splitter.
//! 5. **Solange weniger als [`KOPIEN`] Miner teilnehmen dürfen, behält
//!    jeder alles.** Weniger Redundanz als vorgesehen gibt es nie; ein
//!    junges oder kleines Netz verdichtet einfach nicht.
//!
//! 📌 **Nicht „jeder mit Wahrscheinlichkeit k/N“.** So war die erste Fassung
//! (NODE 0.57.0) entworfen, und die Probe zeigte: Bei einem Erwartungswert
//! von fünf bleibt etwa jede 150. Periode leer. Die kleinsten Lose ergeben
//! genau die vorgesehene Zahl.

use std::collections::BTreeMap;

use crate::hash::Hash;
use crate::ids::MinerId;
use crate::miner::MinerRegistration;
use crate::node_metadata::GeoRegion;

/// So viele volle Kopien je Periode.
pub const KOPIEN: usize = 7;

/// So viele Splitter je Periode (32 Daten- und 32 Paritätsteile).
pub const SPLITTER: usize = 64;

/// Das Ergebnis einer Ziehung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ziehung {
    /// Zu wenige dürfen teilnehmen: Jeder behält alles.
    AlleBehalten,
    /// Kopien und Splitter, nach Los geordnet.
    Gezogen { kopien: Vec<MinerId>, splitter: Vec<MinerId> },
}

/// Die Rolle eines Miners in einer Ziehung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archivrolle {
    /// Jeder behält alles (zu wenige Teilnehmer).
    AlleBehalten,
    /// Hält die volle Geschichte.
    Kopie,
    /// Hält Teile der Codierung.
    Splitter,
    /// War Kopie einer früheren Periode und hält, bis die Nachfolger die
    /// Übergabe belegt haben.
    Halter,
    /// Darf verdichten.
    Keine,
}

impl Ziehung {
    /// Welche Rolle hat `wer`? `None` für einen, der nicht Miner ist.
    pub fn rolle(&self, wer: Option<&MinerId>) -> Archivrolle {
        match (self, wer) {
            (Ziehung::AlleBehalten, _) => Archivrolle::AlleBehalten,
            (Ziehung::Gezogen { kopien, .. }, Some(m)) if kopien.contains(m) => Archivrolle::Kopie,
            (Ziehung::Gezogen { splitter, .. }, Some(m)) if splitter.contains(m) => Archivrolle::Splitter,
            _ => Archivrolle::Keine,
        }
    }
}

fn los(art: &[u8], saat: &Hash, wer: &MinerId) -> Hash {
    let mut e = Vec::with_capacity(art.len() + 64);
    e.extend_from_slice(art);
    e.extend_from_slice(saat.as_bytes());
    e.extend_from_slice(wer.as_bytes());
    Hash::sha256(&e)
}

/// `anzahl` aus `kandidaten`, je Zone nach Los, reihum über die Zonen.
fn zonenverteilt(art: &[u8], saat: &Hash, kandidaten: &[&MinerRegistration], anzahl: usize) -> Vec<MinerId> {
    let mut je_zone: BTreeMap<GeoRegion, Vec<([u8; 32], MinerId)>> = BTreeMap::new();
    for m in kandidaten {
        je_zone
            .entry(m.zone)
            .or_default()
            .push((*los(art, saat, &m.miner_id).as_bytes(), m.miner_id));
    }
    for liste in je_zone.values_mut() {
        // Absteigend, damit `pop` das kleinste Los liefert.
        liste.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.as_bytes().cmp(a.1.as_bytes())));
    }
    let mut gezogen = Vec::with_capacity(anzahl);
    while gezogen.len() < anzahl {
        // Eine Runde: aus jeder Zone die beste, die Runde nach Los geordnet.
        let mut runde: Vec<([u8; 32], MinerId)> = je_zone.values_mut().filter_map(|l| l.pop()).collect();
        if runde.is_empty() {
            break;
        }
        runde.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.as_bytes().cmp(b.1.as_bytes())));
        for (_, m) in runde {
            if gezogen.len() < anzahl {
                gezogen.push(m);
            }
        }
    }
    gezogen
}

/// **Die Ziehung an einer Langzeitgrenze** der Epoche `grenze_epoche`.
pub fn ziehen(
    saat: &Hash,
    miner: &BTreeMap<MinerId, MinerRegistration>,
    grenze_epoche: u64,
    warte_epochen: u64,
) -> Ziehung {
    let berechtigt: Vec<&MinerRegistration> = miner
        .values()
        .filter(|m| m.registration_epoch.saturating_add(warte_epochen) <= grenze_epoche)
        .collect();
    if berechtigt.len() < KOPIEN {
        return Ziehung::AlleBehalten;
    }
    let kopien = zonenverteilt(b"myl-archiv-kopie", saat, &berechtigt, KOPIEN);
    let uebrige: Vec<&MinerRegistration> =
        berechtigt.into_iter().filter(|m| !kopien.contains(&m.miner_id)).collect();
    let splitter = zonenverteilt(b"myl-archiv-splitter", saat, &uebrige, SPLITTER.min(uebrige.len()));
    Ziehung::Gezogen { kopien, splitter }
}

/// **Ein Nachrücker** für eine ersetzte Kopie: unter den Berechtigten, die
/// nicht `ausgeschlossen` sind, bevorzugt einer aus einer Zone, die unter
/// `besetzt` fehlt, dann nach Los wie bei den Kopien. `None`, wenn keiner
/// mehr übrig ist.
pub fn nachruecken(
    saat: &Hash,
    miner: &BTreeMap<MinerId, MinerRegistration>,
    grenze_epoche: u64,
    warte_epochen: u64,
    besetzt: &[MinerId],
    ausgeschlossen: &[MinerId],
) -> Option<MinerId> {
    let zonen: Vec<GeoRegion> = besetzt.iter().filter_map(|k| miner.get(k)).map(|m| m.zone).collect();
    miner
        .values()
        .filter(|m| m.registration_epoch.saturating_add(warte_epochen) <= grenze_epoche)
        .filter(|m| !ausgeschlossen.contains(&m.miner_id) && !besetzt.contains(&m.miner_id))
        .map(|m| (zonen.contains(&m.zone), *los(b"myl-archiv-kopie", saat, &m.miner_id).as_bytes(), m.miner_id))
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.as_bytes().cmp(b.2.as_bytes())))
        .map(|(_, _, m)| m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner::HardwareClass;

    const ZONEN: [GeoRegion; 6] = [
        GeoRegion::NorthAmerica,
        GeoRegion::SouthAmerica,
        GeoRegion::Europe,
        GeoRegion::Africa,
        GeoRegion::Asia,
        GeoRegion::Oceania,
    ];

    fn register(n: u64, zonen: &[GeoRegion], epoche: u64) -> BTreeMap<MinerId, MinerRegistration> {
        (0..n)
            .map(|i| {
                let mut b = [0u8; 32];
                b[..8].copy_from_slice(&i.to_le_bytes());
                let id = MinerId::new(b);
                (
                    id,
                    MinerRegistration {
                        miner_id: id,
                        hardware_class: HardwareClass::MediumGpu,
                        registration_epoch: epoche,
                        zone: zonen[(i as usize) % zonen.len()],
                        schluessel: crate::bls::BlsPublicKey([0; 48]),
                        netzadresse: crate::latency_attest::PeerIdBytes(b),
                    },
                )
            })
            .collect()
    }

    /// ⚑ **Genau die vorgesehene Zahl, in jeder Periode**, disjunkt.
    #[test]
    fn genau_sieben_kopien_und_vierundsechzig_splitter() {
        let reg = register(500, &ZONEN, 0);
        for periode in 0..100u64 {
            let Ziehung::Gezogen { kopien, splitter } = ziehen(&Hash::sha256(&periode.to_le_bytes()), &reg, 64, 64)
            else {
                panic!("gezogen erwartet");
            };
            assert_eq!(kopien.len(), KOPIEN);
            assert_eq!(splitter.len(), SPLITTER);
            assert!(kopien.iter().all(|k| !splitter.contains(k)), "Kopie zugleich Splitter");
        }
    }

    /// ⚑ **Über die Zonen verteilt**: Bei sechs Zonen tragen die sieben
    /// Kopien jede Zone mindestens einmal.
    #[test]
    fn die_kopien_sind_ueber_die_zonen_verteilt() {
        let reg = register(600, &ZONEN, 0);
        for periode in 0..50u64 {
            let z = ziehen(&Hash::sha256(&periode.to_le_bytes()), &reg, 64, 64);
            let Ziehung::Gezogen { kopien, .. } = z else { panic!() };
            let zonen: std::collections::BTreeSet<GeoRegion> = kopien.iter().map(|k| reg[k].zone).collect();
            assert_eq!(zonen.len(), 6, "Periode {periode}: nur {} Zonen", zonen.len());
        }
    }

    /// ⛔️ **Wer zu spät registriert ist, wird nicht gezogen**; sind es zu
    /// wenige Berechtigte, behält jeder alles.
    #[test]
    fn junge_registrierungen_nehmen_nicht_teil() {
        let mut reg = register(20, &ZONEN, 0);
        let jung = register(200, &ZONEN, 60);
        let junge: Vec<MinerId> = (20..200u64)
            .map(|i| {
                let mut b = [0u8; 32];
                b[..8].copy_from_slice(&i.to_le_bytes());
                MinerId::new(b)
            })
            .collect();
        for id in &junge {
            let mut m = jung[id];
            m.miner_id = *id;
            reg.insert(*id, m);
        }
        let Ziehung::Gezogen { kopien, splitter } = ziehen(&Hash::sha256(b"s"), &reg, 64, 64) else { panic!() };
        assert!(kopien.iter().chain(&splitter).all(|m| !junge.contains(m)), "eine junge Registrierung kam durch");
        assert_eq!(ziehen(&Hash::sha256(b"s"), &register(6, &ZONEN, 0), 64, 64), Ziehung::AlleBehalten);
        assert_eq!(ziehen(&Hash::sha256(b"s"), &register(50, &ZONEN, 10), 64, 64), Ziehung::AlleBehalten);
    }

    /// Die Rolle folgt aus der Ziehung; wer kein Miner ist, hat keine.
    #[test]
    fn die_rolle_folgt_aus_der_ziehung() {
        let reg = register(100, &ZONEN, 0);
        let z = ziehen(&Hash::sha256(b"r"), &reg, 64, 64);
        let Ziehung::Gezogen { kopien, splitter } = &z else { panic!() };
        assert_eq!(z.rolle(Some(&kopien[0])), Archivrolle::Kopie);
        assert_eq!(z.rolle(Some(&splitter[0])), Archivrolle::Splitter);
        assert_eq!(z.rolle(None), Archivrolle::Keine);
        assert_eq!(Ziehung::AlleBehalten.rolle(None), Archivrolle::AlleBehalten);
    }

    /// ⚑ **Ein Nachrücker füllt zuerst eine fehlende Zone** und ist nie
    /// einer der Ausgeschlossenen oder Besetzten.
    #[test]
    fn ein_nachruecker_fuellt_zuerst_eine_fehlende_zone() {
        let reg = register(200, &ZONEN, 0);
        let saat = Hash::sha256(b"n");
        let Ziehung::Gezogen { kopien, splitter } = ziehen(&saat, &reg, 64, 64) else { panic!() };
        // Eine Kopie fällt weg; ihre Zone war vielleicht die einzige.
        let weg = kopien[0];
        let rest: Vec<MinerId> = kopien[1..].to_vec();
        let mut aus = splitter.clone();
        aus.push(weg);
        let neu = nachruecken(&saat, &reg, 64, 64, &rest, &aus).expect("Nachrücker");
        assert!(!aus.contains(&neu) && !rest.contains(&neu));
        let fehlend: Vec<GeoRegion> =
            ZONEN.iter().copied().filter(|z| !rest.iter().any(|k| reg[k].zone == *z)).collect();
        if !fehlend.is_empty() {
            assert!(fehlend.contains(&reg[&neu].zone), "eine fehlende Zone blieb leer");
        }
        // Sind alle ausgeschlossen, gibt es keinen.
        let alle: Vec<MinerId> = reg.keys().copied().collect();
        assert_eq!(nachruecken(&saat, &reg, 64, 64, &[], &alle), None);
    }
}
