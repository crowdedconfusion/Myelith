//! **Der Saatdienst des Knotens** (2026-11-03): was ein Mitglied des
//! Stimmsatzes fuer die Saat tut. Die Regeln stehen in `myl_ledger::saat`,
//! die Kryptographie in `myl_types::schwelle`; hier steht nur, **wann** der
//! Knoten **was** sendet.
//!
//! Aus dem Zustand abgeleitet, nicht aus einem eigenen Ablauf: Je Aufruf
//! sieht der Dienst nach, was die Kette schon hat, und sendet nur, was
//! fehlt. Eine verlorene Transaktion wird nach [`WIEDERHOLEN_NACH`] Bloecken
//! erneut gesendet; ein Neustart verliert nichts, denn Polynom und Chiffren
//! sind aus dem Geheimnis abgeleitet und kommen gleich wieder heraus.
//!
//! 1. **Eintragen:** Der eigene Verschluesselungsschluessel steht im
//!    Zustand, sobald der Knoten ein eingetragener Miner ist.
//! 2. **Im ersten Drittel** der Epoche vor `e`: austeilen.
//! 3. **Im zweiten Drittel:** jede Austeilung gegen die eigenen Sitze
//!    pruefen, bei einem falschen Wert beschweren; auf eine Beschwerde gegen
//!    sich den Wert offenlegen (bis zum Ende der Epoche).
//! 4. **In `e`** (und, wenn es dort nicht zustande kam, in `e + 1`): die
//!    eigenen Anteile bilden, jeden lokal gegen seinen oeffentlichen Anteil
//!    pruefen und signieren. ⛔️ **Nie vor `e`**: Wer frueher signiert,
//!    verraet die Saat vor ihrer Zeit.

use std::collections::{BTreeMap, BTreeSet};

use myl_consensus::block::{Anweisung, BLOECKE_JE_EPOCHE};
use myl_ledger::saat::{self, Austeilung, Phase};
use myl_ledger::LedgerState;
use myl_types::anteilchiffre::{self, Entschluesselungsschluessel, Verschluesselungsschluessel};
use myl_types::bls::BlsSignature;
use myl_types::schwelle::{self, Polynom};
use myl_types::skalar::Skalar;
use myl_types::MinerId;

/// Nach so vielen Bloecken ohne Wirkung im Zustand wird erneut gesendet.
pub const WIEDERHOLEN_NACH: u64 = 300;

/// Was gesendet wurde, als Schluessel fuer das Wiederholen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Vorgang {
    Eintragen,
    Austeilen(u64),
    Beschweren(u64, MinerId, u64),
    Offenlegen(u64, u64),
    Anteile(u64),
}

impl Vorgang {
    fn epoche(&self) -> u64 {
        match self {
            Vorgang::Eintragen => u64::MAX,
            Vorgang::Austeilen(e) | Vorgang::Beschweren(e, _, _) | Vorgang::Offenlegen(e, _) | Vorgang::Anteile(e) => *e,
        }
    }
}

/// Der Dienst; haelt das Geheimnis und was er schon getan hat.
pub struct Saatdienst {
    geheimnis: [u8; 32],
    entschluessel: Entschluesselungsschluessel,
    gesendet: BTreeMap<Vorgang, u64>,
    /// Je (Epoche, Austeiler) einmal geprueft, und welche eigenen Sitze
    /// dabei keinen passenden Wert hatten.
    geprueft: BTreeMap<(u64, MinerId), Vec<u64>>,
    /// Die eigenen Anteile je Schluesselquelle, nach Sitz.
    anteile: BTreeMap<u64, Vec<(u64, Skalar)>>,
    /// Nur im Pruefbau: teilt falsch aus ([`Saatdienst::falsch_austeilen`]).
    #[cfg(feature = "falscher_austeiler")]
    falsch: bool,
}

impl std::fmt::Debug for Saatdienst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Saatdienst").field("gesendet", &self.gesendet.len()).finish_non_exhaustive()
    }
}

impl Saatdienst {
    /// Aus dem Geheimnis des Knotens (`Konsensschluessel::saatgeheimnis`).
    pub fn neu(geheimnis: [u8; 32]) -> Option<Self> {
        let entschluessel = Entschluesselungsschluessel::ableiten(&geheimnis)?;
        Some(Self {
            geheimnis,
            entschluessel,
            gesendet: BTreeMap::new(),
            geprueft: BTreeMap::new(),
            anteile: BTreeMap::new(),
            #[cfg(feature = "falscher_austeiler")]
            falsch: false,
        })
    }

    /// ⚑ **Nur im Pruefbau** (Merkmal `falscher_austeiler`, nie im
    /// ausgelieferten Programm): Der Dienst verschluesselt dem ersten
    /// fremden Sitz einen um eins verschobenen Wert. In ungeraden Epochen
    /// legt er auf die Beschwerde hin nicht offen und faellt heraus, in
    /// geraden legt er offen und bleibt; so kommen im Netzlauf beide Wege
    /// vor.
    #[cfg(feature = "falscher_austeiler")]
    pub fn falsch_austeilen(&mut self) {
        self.falsch = true;
    }

    /// Der eigene oeffentliche Verschluesselungsschluessel.
    pub fn oeffentlich(&self) -> Verschluesselungsschluessel {
        self.entschluessel.oeffentlich()
    }

    /// Ob der Vorgang jetzt (wieder) zu senden ist; vermerkt ihn dann.
    fn faellig(&mut self, v: Vorgang, hoehe: u64) -> bool {
        match self.gesendet.get(&v) {
            Some(bei) if hoehe < bei.saturating_add(WIEDERHOLEN_NACH) => false,
            _ => {
                self.gesendet.insert(v, hoehe);
                true
            }
        }
    }

    /// **Was jetzt zu senden ist**, aus dem Zustand nach dem Block an
    /// `hoehe − 1`; `hoehe` ist die Hoehe des naechsten Blocks.
    pub fn handlungen(&mut self, z: &LedgerState, netz: &[u8; 32], ich: &MinerId, hoehe: u64) -> Vec<Anweisung> {
        let jetzt = hoehe / BLOECKE_JE_EPOCHE;
        self.gesendet.retain(|v, _| v.epoche().saturating_add(2) >= jetzt);
        self.geprueft.retain(|(e, _), _| e.saturating_add(2) >= jetzt);
        let quellen: BTreeSet<u64> = z.saatrunden.values().filter_map(|r| r.schluessel.as_ref().map(|s| s.aus_epoche)).collect();
        self.anteile.retain(|e, _| quellen.contains(e));

        let mut aus = Vec::new();
        if !z.miner.contains_key(ich) {
            return aus;
        }
        let pk = self.oeffentlich();
        if z.verschluesselung.get(ich) != Some(&pk) && self.faellig(Vorgang::Eintragen, hoehe) {
            aus.push(Anweisung::SaatschluesselEintragen { schluessel: pk });
        }
        self.runde_pflegen(z, netz, ich, hoehe, &mut aus);
        for e in [jetzt.saturating_sub(1), jetzt] {
            self.anteile_senden(z, netz, ich, e, hoehe, &mut aus);
        }
        aus
    }

    /// Die Runde der naechsten Epoche, im Drittel, in dem `hoehe` liegt.
    fn runde_pflegen(&mut self, z: &LedgerState, netz: &[u8; 32], ich: &MinerId, hoehe: u64, aus: &mut Vec<Anweisung>) {
        let e = hoehe / BLOECKE_JE_EPOCHE + 1;
        let p = saat::phase(hoehe % BLOECKE_JE_EPOCHE, BLOECKE_JE_EPOCHE);
        let Some(runde) = z.saatrunden.get(&e) else { return };
        if runde.schluessel.is_some() || !runde.mitglieder.iter().any(|m| m.miner == *ich) {
            return;
        }
        let kontext = saat::kontext(netz, e, ich);
        let Some(polynom) = Polynom::ableiten(&self.geheimnis, &kontext, runde.schwelle) else { return };
        match p {
            Phase::Austeilen => {
                if !z.austeilungen.contains_key(&(e, *ich)) && self.faellig(Vorgang::Austeilen(e), hoehe) {
                    if let Some(a) = self.austeilung(runde, &polynom, &kontext, ich) {
                        aus.push(Anweisung::SaatAusteilen { epoche: e, austeilung: Box::new(a) });
                    }
                }
            }
            Phase::Beschweren | Phase::Offenlegen => {
                if p == Phase::Beschweren {
                    let meine: Vec<u64> = runde.mitglieder.iter().filter(|m| m.miner == *ich).flat_map(|m| m.erste_nummer..m.erste_nummer + m.sitze).collect();
                    let austeiler: Vec<MinerId> = z.austeilungen.keys().filter(|(x, _)| *x == e).map(|(_, a)| *a).collect();
                    for a in austeiler {
                        let falsch = self.geprueft.entry((e, a)).or_insert_with(|| {
                            let au = z.austeilungen.get(&(e, a)).expect("eben gelesen");
                            let k = saat::kontext(netz, e, &a);
                            meine
                                .iter()
                                .copied()
                                .filter(|n| {
                                    let wert = au.chiffren.get((*n - 1) as usize).and_then(|c| self.entschluessel.entschluesseln(c, &k, *n));
                                    !wert.is_some_and(|w| schwelle::anteil_pruefen(&au.bindung, *n, &w))
                                })
                                .collect()
                        });
                        for n in falsch.clone() {
                            if runde.beschwerden.binary_search(&(a, n)).is_err() && self.faellig(Vorgang::Beschweren(e, a, n), hoehe) {
                                aus.push(Anweisung::SaatBeschweren { epoche: e, austeiler: a, nummer: n });
                            }
                        }
                    }
                }
                #[cfg(feature = "falscher_austeiler")]
                if self.falsch && e % 2 == 1 {
                    return;
                }
                for (a, n) in runde.beschwerden.iter().filter(|(a, _)| a == ich) {
                    let offen = !runde.offenlegungen.iter().any(|(b, m, _)| b == a && m == n);
                    if offen && self.faellig(Vorgang::Offenlegen(e, *n), hoehe) {
                        aus.push(Anweisung::SaatOffenlegen { epoche: e, nummer: *n, wert: polynom.anteil(*n).als_bytes_be() });
                    }
                }
            }
        }
    }

    fn austeilung(&self, runde: &saat::Saatrunde, polynom: &Polynom, kontext: &[u8], ich: &MinerId) -> Option<Austeilung> {
        #[cfg(feature = "falscher_austeiler")]
        let opfer = self.falsch.then(|| runde.mitglieder.iter().find(|m| m.miner != *ich && m.sitze > 0).map(|m| m.erste_nummer)).flatten();
        #[cfg(not(feature = "falscher_austeiler"))]
        let _ = ich;
        let mut chiffren = Vec::with_capacity(runde.gesamt_sitze as usize);
        for n in 1..=runde.gesamt_sitze {
            let schluessel = runde.inhaber_von(n)?.schluessel?;
            #[allow(unused_mut)]
            let mut wert = polynom.anteil(n);
            #[cfg(feature = "falscher_austeiler")]
            if opfer == Some(n) {
                wert = wert.plus(&Skalar::EINS);
            }
            chiffren.push(anteilchiffre::verschluesseln(&schluessel, &wert, kontext, n, &self.geheimnis)?);
        }
        Some(Austeilung { bindung: polynom.bindung(), nachweis: polynom.kenntnisnachweis(kontext), chiffren })
    }

    /// Die eigenen Anteile unter dem Schluessel aus der Runde `quelle`: je
    /// Sitz die Summe ueber die Qualifizierten, offengelegt oder
    /// entschluesselt. Ein Sitz ohne vollstaendige Summe faellt weg.
    fn eigene_anteile(&mut self, z: &LedgerState, netz: &[u8; 32], ich: &MinerId, schluessel: &saat::Saatschluessel) -> Vec<(u64, Skalar)> {
        let quelle = schluessel.aus_epoche;
        if let Some(a) = self.anteile.get(&quelle) {
            return a.clone();
        }
        let Some(runde) = z.saatrunden.get(&quelle) else { return Vec::new() };
        let mut ergebnis = Vec::new();
        for n in schluessel.nummern_von(ich) {
            let mut werte = Vec::with_capacity(schluessel.qualifiziert.len());
            for a in &schluessel.qualifiziert {
                let offen = runde.offenlegungen.iter().find(|(b, m, _)| b == a && *m == n).and_then(|(_, _, w)| Skalar::aus_bytes_be(w));
                let wert = offen.or_else(|| {
                    let au = z.austeilungen.get(&(quelle, *a))?;
                    self.entschluessel.entschluesseln(au.chiffren.get((n - 1) as usize)?, &saat::kontext(netz, quelle, a), n)
                });
                match wert {
                    Some(w) => werte.push(w),
                    None => break,
                }
            }
            if werte.len() == schluessel.qualifiziert.len() {
                ergebnis.push((n, schwelle::anteile_summieren(&werte)));
            }
        }
        self.anteile.insert(quelle, ergebnis.clone());
        ergebnis
    }

    fn anteile_senden(&mut self, z: &LedgerState, netz: &[u8; 32], ich: &MinerId, e: u64, hoehe: u64, aus: &mut Vec<Anweisung>) {
        let Some(runde) = z.saatrunden.get(&e) else { return };
        let Some(schluessel) = runde.schluessel.clone() else { return };
        if runde.beta.is_some() {
            return;
        }
        let meine = schluessel.nummern_von(ich);
        if meine.is_empty() || meine.clone().all(|n| runde.anteile.iter().any(|(m, _)| *m == n)) {
            return;
        }
        if !self.faellig(Vorgang::Anteile(e), hoehe) {
            return;
        }
        let botschaft = schwelle::saatbotschaft(e);
        let teile: Vec<(u64, BlsSignature)> = self
            .eigene_anteile(z, netz, ich, &schluessel)
            .into_iter()
            .filter_map(|(n, s)| {
                let sig = schwelle::anteil_signieren(&s, &botschaft)?;
                // Lokal pruefen: Ein ungueltiger Anteil verwirft die ganze
                // Anweisung.
                schluessel.oeffentliche_anteile.get((n - 1) as usize)?.verify(&botschaft, &sig).then_some((n, sig))
            })
            .collect();
        if !teile.is_empty() {
            aus.push(Anweisung::SaatAnteile { epoche: e, teile });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETZ: [u8; 32] = [3; 32];
    const B: u64 = BLOECKE_JE_EPOCHE;

    fn m(i: u8) -> MinerId {
        MinerId::new([i; 32])
    }

    /// Vier Mitglieder mit Diensten, ein Satz fuer Epoche 5, Miner
    /// eingetragen. Die Kette ist hier der Ledger direkt.
    fn aufbau() -> (LedgerState, Vec<Saatdienst>) {
        let mut z = LedgerState::genesis(1);
        let dienste: Vec<Saatdienst> = (1..=4u8).map(|i| Saatdienst::neu([i; 32]).unwrap()).collect();
        let mut satz = Vec::new();
        for i in 1..=4u8 {
            let pk = myl_types::bls::BlsSecretKey::key_gen(&[i; 32]).unwrap().public_key().unwrap();
            z.miner.insert(
                m(i),
                myl_types::miner::MinerRegistration {
                    miner_id: m(i),
                    hardware_class: myl_types::miner::HardwareClass::MediumGpu,
                    registration_epoch: 0,
                    zone: myl_types::node_metadata::GeoRegion::Europe,
                    schluessel: pk,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([i; 32]),
                },
            );
            satz.push((m(i), pk, 10));
        }
        z.stimmsaetze.insert(5, satz);
        (z, dienste)
    }

    /// Wendet die Handlungen eines Dienstes direkt an, wie die Kette es taete.
    fn anwenden(z: &mut LedgerState, wer: &MinerId, a: Anweisung, hoehe: u64) -> Option<BlsSignature> {
        let p = saat::phase(hoehe % B, B);
        let jetzt = hoehe / B;
        match a {
            Anweisung::SaatschluesselEintragen { schluessel } => {
                z.verschluesselung.insert(*wer, schluessel);
            }
            Anweisung::SaatAusteilen { epoche, austeilung } => saat::austeilung_aufnehmen(z, &NETZ, epoche, wer, *austeilung, p).unwrap(),
            Anweisung::SaatBeschweren { epoche, austeiler, nummer } => saat::beschwerde_aufnehmen(z, epoche, wer, &austeiler, nummer, p).unwrap(),
            Anweisung::SaatOffenlegen { epoche, nummer, wert } => saat::offenlegung_aufnehmen(z, epoche, wer, nummer, wert, p).unwrap(),
            // Nach der Schwelle weist die Kette weitere Anteile ab; das ist
            // kein Fehler des Dienstes, der sie vor dem Block nicht kannte.
            Anweisung::SaatAnteile { epoche, teile } => {
                return match saat::anteile_aufnehmen(z, epoche, wer, &teile, jetzt) {
                    Err("schon gesetzt") => None,
                    r => r.unwrap(),
                }
            }
            _ => unreachable!(),
        }
        None
    }

    fn runde(z: &mut LedgerState, dienste: &mut [Saatdienst], hoehe: u64) -> Vec<(MinerId, Anweisung)> {
        let mut alle = Vec::new();
        for (i, d) in dienste.iter_mut().enumerate() {
            for a in d.handlungen(z, &NETZ, &m(i as u8 + 1), hoehe) {
                alle.push((m(i as u8 + 1), a));
            }
        }
        alle
    }

    /// ⚑ Von der Eintragung bis `β`, nur mit dem, was die Dienste senden.
    #[test]
    fn vier_dienste_bilden_eine_saat() {
        let (mut z, mut dienste) = aufbau();
        // Epoche 3: eintragen.
        for (w, a) in runde(&mut z, &mut dienste, 3 * B) {
            anwenden(&mut z, &w, a, 3 * B);
        }
        assert_eq!(z.verschluesselung.len(), 4);
        // Wechsel nach 4: Runde fuer 5 anlegen.
        saat::runde_anlegen(&mut z, 5);
        let h = 4 * B + 10;
        let ausgeteilt = runde(&mut z, &mut dienste, h);
        assert_eq!(ausgeteilt.len(), 4);
        for (w, a) in ausgeteilt {
            anwenden(&mut z, &w, a, h);
        }
        // Zweimal fragen: nichts Neues.
        assert!(runde(&mut z, &mut dienste, h + 1).is_empty());
        // Zweites Drittel: alles passt, keine Beschwerde.
        assert!(runde(&mut z, &mut dienste, 4 * B + B / 2).is_empty());
        // Wechsel nach 5.
        assert!(saat::abschliessen(&mut z, 5));
        // ⛔️ Vor Epoche 5 signiert niemand: In Epoche 4 gibt es keinen
        //   Schluessel fuer 4, und fuer 5 fragt der Dienst nicht.
        let h = 5 * B + 1;
        let mut beta = None;
        for (w, a) in runde(&mut z, &mut dienste, h) {
            if let Some(b) = anwenden(&mut z, &w, a, h) {
                beta = Some(b);
            }
        }
        let beta = beta.expect("β gesetzt");
        let sk = saat::schluessel_der_epoche(&z, 5).unwrap();
        assert!(sk.gruppe.verify(&schwelle::saatbotschaft(5), &beta));
        assert!(runde(&mut z, &mut dienste, h + 1).is_empty());
    }

    /// Ein Austeiler schickt einem Sitz Muell; der Dienst des Inhabers
    /// beschwert sich, der Dienst des Austeilers legt offen, und die Saat
    /// entsteht trotzdem mit allen vieren.
    #[test]
    fn beschwerde_und_offenlegung_laufen_von_selbst() {
        let (mut z, mut dienste) = aufbau();
        for (w, a) in runde(&mut z, &mut dienste, 3 * B) {
            anwenden(&mut z, &w, a, 3 * B);
        }
        saat::runde_anlegen(&mut z, 5);
        let h = 4 * B + 10;
        for (w, mut a) in runde(&mut z, &mut dienste, h) {
            if w == m(4) {
                if let Anweisung::SaatAusteilen { austeilung, .. } = &mut a {
                    austeilung.chiffren[0].c = [0x22; 32];
                }
            }
            anwenden(&mut z, &w, a, h);
        }
        let h = 4 * B + B / 2;
        let beschwerden = runde(&mut z, &mut dienste, h);
        assert_eq!(beschwerden.len(), 1);
        assert!(matches!(beschwerden[0], (w, Anweisung::SaatBeschweren { nummer: 1, .. }) if w == m(1)));
        for (w, a) in beschwerden {
            anwenden(&mut z, &w, a, h);
        }
        let offen = runde(&mut z, &mut dienste, h + 1);
        assert_eq!(offen.len(), 1);
        for (w, a) in offen {
            anwenden(&mut z, &w, a, h + 1);
        }
        assert!(saat::abschliessen(&mut z, 5));
        assert_eq!(saat::schluessel_der_epoche(&z, 5).unwrap().qualifiziert.len(), 4);
        let h = 5 * B + 1;
        let mut beta = None;
        for (w, a) in runde(&mut z, &mut dienste, h) {
            beta = anwenden(&mut z, &w, a, h).or(beta);
        }
        assert!(beta.is_some());
    }

    /// ⚑ Der Schalter des Pruefbaus: In ungerader Epoche faellt der
    /// falsche Austeiler heraus, in gerader legt er offen und bleibt; `β`
    /// entsteht in beiden.
    #[cfg(feature = "falscher_austeiler")]
    #[test]
    fn der_falsche_austeiler_des_pruefbaus() {
        for e in [5u64, 6] {
            let (mut z, mut dienste) = aufbau();
            let satz = z.stimmsaetze.remove(&5).unwrap();
            z.stimmsaetze.insert(e, satz);
            dienste[3].falsch_austeilen();
            let h = (e - 2) * B;
            for (w, a) in runde(&mut z, &mut dienste, h) {
                anwenden(&mut z, &w, a, h);
            }
            saat::runde_anlegen(&mut z, e);
            let h = (e - 1) * B + 10;
            for (w, a) in runde(&mut z, &mut dienste, h) {
                anwenden(&mut z, &w, a, h);
            }
            let h = (e - 1) * B + B / 2;
            let beschwerden = runde(&mut z, &mut dienste, h);
            assert!(matches!(beschwerden[..], [(w, Anweisung::SaatBeschweren { nummer: 1, .. })] if w == m(1)), "{e}");
            for (w, a) in beschwerden {
                anwenden(&mut z, &w, a, h);
            }
            for (w, a) in runde(&mut z, &mut dienste, h + 1) {
                anwenden(&mut z, &w, a, h + 1);
            }
            assert!(saat::abschliessen(&mut z, e));
            let erwartet = if e % 2 == 1 { 3 } else { 4 };
            assert_eq!(saat::schluessel_der_epoche(&z, e).unwrap().qualifiziert.len(), erwartet, "{e}");
            let h = e * B + 1;
            let mut beta = None;
            for (w, a) in runde(&mut z, &mut dienste, h) {
                beta = anwenden(&mut z, &w, a, h).or(beta);
            }
            assert!(beta.is_some(), "{e}");
        }
    }

    #[test]
    fn verlorenes_wird_nach_der_frist_wiederholt() {
        let (mut z, mut dienste) = aufbau();
        let erst = dienste[0].handlungen(&z, &NETZ, &m(1), 3 * B);
        assert_eq!(erst.len(), 1);
        // Nicht angewendet (verloren): erst nach der Frist erneut.
        assert!(dienste[0].handlungen(&z, &NETZ, &m(1), 3 * B + 1).is_empty());
        assert_eq!(dienste[0].handlungen(&z, &NETZ, &m(1), 3 * B + WIEDERHOLEN_NACH).len(), 1);
        // Wer kein Miner ist, tut nichts.
        z.miner.remove(&m(1));
        assert!(dienste[0].handlungen(&z, &NETZ, &m(1), 4 * B).is_empty());
    }
}
