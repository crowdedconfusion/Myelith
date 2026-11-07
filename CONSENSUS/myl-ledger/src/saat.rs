//! **Die Saat aus dem Stimmsatz** (2026-11-03): verteilte
//! Schluesselerzeugung in der Epoche davor, Schwellensignatur in der Epoche
//! selbst. Die Kryptographie steht in `myl_types::schwelle`; hier stehen die
//! Uebergaenge des Zustands.
//!
//! ⚑ **Warum:** Die Saat der Epoche kam aus dem letzten Blockhash, die der
//! Stichprobe aus einem Pruefpunktzertifikat; beides waehlt jemand, und wer
//! die Saat waehlt, waehlt, welche Segmente nachgerechnet werden. Eine
//! BLS-Schwellensignatur ist eindeutig: Jede Menge von `schwelle` gueltigen
//! Anteilen ergibt dieselbe, und vorher kennt sie keiner.
//!
//! **Der Ablauf fuer die Epoche `e`:**
//!
//! 1. **Anlegen** am Wechsel nach `e − 1` ([`runde_anlegen`]): Der Satz von
//!    `e` steht seit dem Wechsel nach `e − 2` fest. Sitze nach Einsatz
//!    (`myl_types::sitze`), nur fuer Mitglieder mit eingetragenem
//!    Verschluesselungsschluessel; deren Schluessel werden jetzt
//!    festgehalten.
//! 2. **In `e − 1`**, nach Drittelfenstern der Epoche ([`Phase`]):
//!    Austeilen ([`austeilung_aufnehmen`]), Beschweren
//!    ([`beschwerde_aufnehmen`]), Offenlegen ([`offenlegung_aufnehmen`]). Die
//!    Kette ist der Rundfunkkanal; geprueft wird bei der Aufnahme, was
//!    nicht stimmt, kommt nicht hinein.
//! 3. **Abschliessen** am Wechsel nach `e` ([`abschliessen`]): Qualifiziert
//!    ist, wer ausgeteilt und jede Beschwerde mit einem passenden Wert
//!    beantwortet hat. Tragen die Qualifizierten **mehr als ein Drittel**
//!    des Gewichts, ist unter ihnen mindestens ein ehrlicher, und der
//!    Gruppenschluessel ist die Summe ihrer Bindungen. Sonst gilt der
//!    Schluessel der Vorepoche weiter.
//! 4. **In `e` und `e + 1`**: Anteile der Signatur ueber
//!    `saatbotschaft(e)` ([`anteile_aufnehmen`]); ab der Schwelle setzt die
//!    Kette `β_e`.
//!
//! ⛔️ **Kein Rueckfall auf eine mahlbare Quelle**, sobald es einen
//! Schluessel gibt: sonst lohnte es, die Saat zu verhindern. Nur solange es
//! nie einen gab (Startphase), gilt die alte Saat.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::anteilchiffre::{Anteilchiffre, Verschluesselungsschluessel};
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::schwelle::{self, Bindung, Kenntnisnachweis};
use myl_types::skalar::Skalar;
use myl_types::{sitze, MinerId};

use crate::state::LedgerState;

/// Domaene des Kontexts einer Austeilung.
const DST_KONTEXT: &[u8] = b"myl-dkg-kontext-v1";

/// Domaene der Epochensaat aus `β`.
const DST_EPOCHENSAAT: &[u8] = b"myl-epochensaat-aus-beta-v1";

/// Ein Mitglied der Runde.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Saatmitglied {
    /// Kennung.
    pub miner: MinerId,
    /// Gewicht (Einsatz) im Satz.
    pub gewicht: u64,
    /// Der beim Anlegen festgehaltene Schluessel; ohne ihn keine Sitze.
    pub schluessel: Option<Verschluesselungsschluessel>,
    /// Die erste Sitznummer.
    pub erste_nummer: u64,
    /// Wie viele Sitze.
    pub sitze: u64,
}

/// Der Schluessel einer Epoche, nach dem Abschluss.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Saatschluessel {
    /// Aus der Runde welcher Epoche er stammt (bei einem Rueckfall eine
    /// fruehere); dort liegen Austeilungen und Offenlegungen.
    pub aus_epoche: u64,
    /// Der Gruppenschluessel.
    pub gruppe: BlsPublicKey,
    /// Anteile je Signatur.
    pub schwelle: u64,
    /// Die oeffentlichen Anteile, Sitz `n` an Stelle `n − 1`.
    pub oeffentliche_anteile: Vec<BlsPublicKey>,
    /// Inhaber: Kennung, erste Nummer, Zahl der Sitze.
    pub inhaber: Vec<(MinerId, u64, u64)>,
    /// Die qualifizierten Austeiler, geordnet.
    pub qualifiziert: Vec<MinerId>,
}

impl Saatschluessel {
    /// Wem die Sitznummer gehoert.
    pub fn inhaber_von(&self, nummer: u64) -> Option<MinerId> {
        self.inhaber.iter().find(|(_, erste, n)| nummer >= *erste && nummer < erste + n).map(|(m, _, _)| *m)
    }

    /// Die Sitznummern eines Mitglieds.
    pub fn nummern_von(&self, wer: &MinerId) -> std::ops::Range<u64> {
        self.inhaber.iter().find(|(m, _, _)| m == wer).map(|(_, e, n)| *e..e + n).unwrap_or(0..0)
    }
}

/// Die Runde fuer eine Epoche.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Saatrunde {
    /// Die Mitglieder in der Ordnung des Satzes.
    pub mitglieder: Vec<Saatmitglied>,
    /// Das Gesamtgewicht des Satzes, mit den Mitgliedern ohne Schluessel.
    pub gesamtgewicht: u64,
    /// Anteile je Signatur.
    pub schwelle: u64,
    /// Vergebene Sitze.
    pub gesamt_sitze: u64,
    /// Beschwerden: Austeiler und Sitznummer, geordnet.
    pub beschwerden: Vec<(MinerId, u64)>,
    /// Offengelegte Werte: Austeiler, Sitznummer, Wert.
    pub offenlegungen: Vec<(MinerId, u64, [u8; 32])>,
    /// Nach dem Abschluss: der Schluessel dieser Epoche.
    pub schluessel: Option<Saatschluessel>,
    /// Gepruefte Signaturanteile fuer `β`, nach Nummer.
    pub anteile: Vec<(u64, BlsSignature)>,
    /// `β` dieser Epoche, sobald gesetzt.
    pub beta: Option<BlsSignature>,
}

impl Saatrunde {
    fn mitglied(&self, wer: &MinerId) -> Option<&Saatmitglied> {
        self.mitglieder.iter().find(|m| m.miner == *wer)
    }

    /// Wem die Sitznummer in der Runde gehoert.
    pub fn inhaber_von(&self, nummer: u64) -> Option<&Saatmitglied> {
        self.mitglieder.iter().find(|m| m.sitze > 0 && nummer >= m.erste_nummer && nummer < m.erste_nummer + m.sitze)
    }
}

/// Eine Austeilung, wie sie in der Kette steht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Austeilung {
    /// Feldman-Bindungen.
    pub bindung: Bindung,
    /// Kenntnisnachweis zu `bindung.punkte[0]`.
    pub nachweis: Kenntnisnachweis,
    /// Je Sitz eine Chiffre, Sitz `n` an Stelle `n − 1`.
    pub chiffren: Vec<Anteilchiffre>,
}

/// Das Fenster innerhalb der Epoche vor der Zielepoche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Erstes Drittel.
    Austeilen,
    /// Zweites Drittel.
    Beschweren,
    /// Letztes Drittel (Offenlegen geht schon ab dem zweiten).
    Offenlegen,
}

/// Die Phase zu einem Versatz innerhalb der Epoche.
pub fn phase(versatz: u64, bloecke_je_epoche: u64) -> Phase {
    if versatz.saturating_mul(3) < bloecke_je_epoche {
        Phase::Austeilen
    } else if versatz.saturating_mul(3) < bloecke_je_epoche.saturating_mul(2) {
        Phase::Beschweren
    } else {
        Phase::Offenlegen
    }
}

/// Der Kontext der Austeilung von `austeiler` fuer die Epoche `e` auf dem
/// Netz `netz`: Polynom, Kenntnisnachweis und Chiffren sind daran gebunden.
pub fn kontext(netz: &[u8; 32], e: u64, austeiler: &MinerId) -> Vec<u8> {
    let mut k = Vec::with_capacity(DST_KONTEXT.len() + 32 + 8 + 32);
    k.extend_from_slice(DST_KONTEXT);
    k.extend_from_slice(netz);
    k.extend_from_slice(&e.to_le_bytes());
    k.extend_from_slice(austeiler.as_bytes());
    k
}

/// **Anlegen** der Runde fuer `e` aus dem Satz von `e`. Ohne Satz, ohne
/// Sitze oder mit weniger Sitzen als der Schwelle keine Runde.
pub fn runde_anlegen(state: &mut LedgerState, e: u64) {
    if state.saatrunden.contains_key(&e) {
        return;
    }
    let Some(satz) = crate::stimmsatz::satz_der_epoche(state, e).cloned() else { return };
    let gewichte: Vec<u64> = satz.iter().map(|(_, _, w)| *w).collect();
    let Some(vergabe) = sitze::vergeben(&gewichte) else { return };
    let mut naechste = 1u64;
    let mut mitglieder = Vec::with_capacity(satz.len());
    for ((miner, _, gewicht), s) in satz.iter().zip(&vergabe.sitze) {
        let schluessel = state.verschluesselung.get(miner).copied();
        // Ohne Schluessel kann niemand ihm einen Wert schicken: keine Sitze,
        // aber sein Gewicht zaehlt im Ganzen mit.
        let sitze = if schluessel.is_some() { *s } else { 0 };
        mitglieder.push(Saatmitglied { miner: *miner, gewicht: *gewicht, schluessel, erste_nummer: naechste, sitze });
        naechste += sitze;
    }
    let gesamt_sitze = naechste - 1;
    if gesamt_sitze < vergabe.schwelle {
        return;
    }
    state.saatrunden.insert(
        e,
        Saatrunde {
            mitglieder,
            gesamtgewicht: crate::stimmsatz::gewicht(&satz),
            schwelle: vergabe.schwelle,
            gesamt_sitze,
            beschwerden: Vec::new(),
            offenlegungen: Vec::new(),
            schluessel: None,
            anteile: Vec::new(),
            beta: None,
        },
    );
}

/// **Eine Austeilung aufnehmen.** Nur in der Phase `Austeilen` der Epoche
/// vor `e`, je Mitglied einmal, mit passender Laenge und gueltiger Bindung
/// samt Kenntnisnachweis.
pub fn austeilung_aufnehmen(
    state: &mut LedgerState,
    netz: &[u8; 32],
    e: u64,
    austeiler: &MinerId,
    austeilung: Austeilung,
    p: Phase,
) -> Result<(), &'static str> {
    if p != Phase::Austeilen {
        return Err("nicht im Fenster");
    }
    let runde = state.saatrunden.get(&e).ok_or("keine Runde")?;
    if runde.schluessel.is_some() {
        return Err("abgeschlossen");
    }
    if runde.mitglied(austeiler).is_none() {
        return Err("kein Mitglied");
    }
    if state.austeilungen.contains_key(&(e, *austeiler)) {
        return Err("schon ausgeteilt");
    }
    if austeilung.chiffren.len() as u64 != runde.gesamt_sitze {
        return Err("falsche Zahl der Chiffren");
    }
    if !schwelle::bindung_pruefen(&austeilung.bindung, runde.schwelle, &kontext(netz, e, austeiler), &austeilung.nachweis) {
        return Err("Bindung oder Nachweis ungueltig");
    }
    state.austeilungen.insert((e, *austeiler), austeilung);
    Ok(())
}

/// **Eine Beschwerde aufnehmen**: Der Inhaber von Sitz `nummer` findet bei
/// `austeiler` keinen passenden Wert. Nur in der Phase `Beschweren`, je
/// (Austeiler, Sitz) einmal. Begruendet wird nicht: Der Austeiler legt den
/// Wert offen, und dann sieht jeder, wer recht hatte.
pub fn beschwerde_aufnehmen(
    state: &mut LedgerState,
    e: u64,
    klaeger: &MinerId,
    austeiler: &MinerId,
    nummer: u64,
    p: Phase,
) -> Result<(), &'static str> {
    if p != Phase::Beschweren {
        return Err("nicht im Fenster");
    }
    if !state.austeilungen.contains_key(&(e, *austeiler)) {
        return Err("keine Austeilung");
    }
    let runde = state.saatrunden.get_mut(&e).ok_or("keine Runde")?;
    if runde.schluessel.is_some() {
        return Err("abgeschlossen");
    }
    if runde.inhaber_von(nummer).map(|m| m.miner) != Some(*klaeger) {
        return Err("nicht sein Sitz");
    }
    match runde.beschwerden.binary_search(&(*austeiler, nummer)) {
        Ok(_) => Err("schon beschwert"),
        Err(i) => {
            runde.beschwerden.insert(i, (*austeiler, nummer));
            Ok(())
        }
    }
}

/// **Einen Wert offenlegen**, auf eine Beschwerde hin. Ab der Phase
/// `Beschweren` bis zum Ende der Epoche; nur ein Wert, der zur Bindung
/// passt, kommt hinein.
pub fn offenlegung_aufnehmen(
    state: &mut LedgerState,
    e: u64,
    austeiler: &MinerId,
    nummer: u64,
    wert: [u8; 32],
    p: Phase,
) -> Result<(), &'static str> {
    if p == Phase::Austeilen {
        return Err("nicht im Fenster");
    }
    let bindung = state.austeilungen.get(&(e, *austeiler)).ok_or("keine Austeilung")?.bindung.clone();
    let runde = state.saatrunden.get_mut(&e).ok_or("keine Runde")?;
    if runde.schluessel.is_some() {
        return Err("abgeschlossen");
    }
    if runde.beschwerden.binary_search(&(*austeiler, nummer)).is_err() {
        return Err("keine Beschwerde");
    }
    if runde.offenlegungen.iter().any(|(a, n, _)| a == austeiler && *n == nummer) {
        return Err("schon offengelegt");
    }
    let skalar = Skalar::aus_bytes_be(&wert).ok_or("keine Zahl")?;
    if !schwelle::anteil_pruefen(&bindung, nummer, &skalar) {
        return Err("passt nicht zur Bindung");
    }
    runde.offenlegungen.push((*austeiler, nummer, wert));
    Ok(())
}

/// Ob ein Austeiler qualifiziert ist: ausgeteilt, und jede Beschwerde gegen
/// ihn beantwortet.
fn qualifiziert(state: &LedgerState, runde: &Saatrunde, e: u64, wer: &MinerId) -> bool {
    state.austeilungen.contains_key(&(e, *wer))
        && runde
            .beschwerden
            .iter()
            .filter(|(a, _)| a == wer)
            .all(|(a, n)| runde.offenlegungen.iter().any(|(b, m, _)| b == a && m == n))
}

/// **Abschliessen** am Wechsel nach `e`. Gibt zurueck, ob ein neuer
/// Schluessel entstand.
pub fn abschliessen(state: &mut LedgerState, e: u64) -> bool {
    let neu = state.saatrunden.get(&e).filter(|r| r.schluessel.is_none()).and_then(|runde| {
        let qualifizierte: Vec<&Saatmitglied> =
            runde.mitglieder.iter().filter(|m| qualifiziert(state, runde, e, &m.miner)).collect();
        let gewicht: u128 = qualifizierte.iter().map(|m| u128::from(m.gewicht)).sum();
        // ⛔️ Mehr als ein Drittel: sonst koennten es lauter Angreifer sein.
        if 3 * gewicht <= u128::from(runde.gesamtgewicht) {
            return None;
        }
        let bindungen: Vec<Bindung> =
            qualifizierte.iter().filter_map(|m| state.austeilungen.get(&(e, m.miner)).map(|a| a.bindung.clone())).collect();
        let summe = schwelle::bindungen_summieren(&bindungen)?;
        let gruppe = schwelle::gruppenschluessel(&summe)?;
        let oeffentliche_anteile: Vec<BlsPublicKey> =
            (1..=runde.gesamt_sitze).map(|n| schwelle::oeffentlicher_anteil(&summe, n)).collect::<Option<_>>()?;
        Some(Saatschluessel {
            aus_epoche: e,
            gruppe,
            schwelle: runde.schwelle,
            oeffentliche_anteile,
            inhaber: runde.mitglieder.iter().filter(|m| m.sitze > 0).map(|m| (m.miner, m.erste_nummer, m.sitze)).collect(),
            qualifiziert: qualifizierte.iter().map(|m| m.miner).collect(),
        })
    });
    let entstanden = neu.is_some();
    // Sonst gilt der Schluessel der Vorepoche weiter, wenn es einen gibt.
    let schluessel = neu.or_else(|| {
        e.checked_sub(1).and_then(|v| state.saatrunden.get(&v)).and_then(|r| r.schluessel.clone())
    });
    if let Some(s) = schluessel {
        match state.saatrunden.get_mut(&e) {
            Some(r) => r.schluessel = Some(s),
            None => {
                state.saatrunden.insert(
                    e,
                    Saatrunde {
                        mitglieder: Vec::new(),
                        gesamtgewicht: 0,
                        schwelle: s.schwelle,
                        gesamt_sitze: 0,
                        beschwerden: Vec::new(),
                        offenlegungen: Vec::new(),
                        schluessel: Some(s),
                        anteile: Vec::new(),
                        beta: None,
                    },
                );
            }
        }
    }
    entstanden
}

/// Der Schluessel, unter dem `β_e` entsteht, wenn es einen gibt.
pub fn schluessel_der_epoche(state: &LedgerState, e: u64) -> Option<&Saatschluessel> {
    state.saatrunden.get(&e).and_then(|r| r.schluessel.as_ref())
}

/// **Signaturanteile aufnehmen** fuer `β_e`, in der Epoche `e` und der
/// folgenden. Jeder Anteil muss zu einem Sitz des Absenders gehoeren und
/// gegen dessen oeffentlichen Anteil bestehen; ein ungueltiger verwirft die
/// ganze Anweisung. Gibt `β_e` zurueck, wenn es mit dieser Anweisung
/// entstand.
pub fn anteile_aufnehmen(
    state: &mut LedgerState,
    e: u64,
    absender: &MinerId,
    teile: &[(u64, BlsSignature)],
    jetzt: u64,
) -> Result<Option<BlsSignature>, &'static str> {
    if e > jetzt || e.saturating_add(1) < jetzt {
        return Err("nicht im Fenster");
    }
    let runde = state.saatrunden.get_mut(&e).ok_or("keine Runde")?;
    if runde.beta.is_some() {
        return Err("schon gesetzt");
    }
    let schluessel = runde.schluessel.as_ref().ok_or("kein Schluessel")?;
    let botschaft = schwelle::saatbotschaft(e);
    let mut neu = Vec::with_capacity(teile.len());
    for (n, sig) in teile {
        if schluessel.inhaber_von(*n) != Some(*absender) {
            return Err("nicht sein Sitz");
        }
        if runde.anteile.iter().any(|(m, _)| m == n) || neu.iter().any(|(m, _): &(u64, BlsSignature)| m == n) {
            continue;
        }
        let pk = schluessel.oeffentliche_anteile.get((*n - 1) as usize).ok_or("keine Nummer")?;
        if !pk.verify(&botschaft, sig) {
            return Err("Anteil ungueltig");
        }
        neu.push((*n, *sig));
    }
    runde.anteile.extend(neu);
    runde.anteile.sort_by_key(|(n, _)| *n);
    if (runde.anteile.len() as u64) < schluessel.schwelle {
        return Ok(None);
    }
    let beta = schwelle::zusammensetzen(&runde.anteile, schluessel.schwelle).ok_or("nicht zusammensetzbar")?;
    // Jeder Anteil ist geprueft, also muss die Summe gelten; tut sie es
    // nicht, ist das ein Fehler hier und keine Saat.
    if !schluessel.gruppe.verify(&botschaft, &beta) {
        return Err("Summe ungueltig");
    }
    runde.beta = Some(beta);
    runde.anteile.clear();
    Ok(Some(beta))
}

/// Die Saat aus `β_e`, wenn es gesetzt ist.
pub fn beta_der_epoche(state: &LedgerState, e: u64) -> Option<BlsSignature> {
    state.saatrunden.get(&e).and_then(|r| r.beta)
}

/// Die Epochensaat der uebernaechsten Zuteilung aus `β`.
pub fn epochensaat_aus(beta: &BlsSignature) -> myl_types::Hash {
    let s = schwelle::saat_aus_signatur(beta);
    let mut b = Vec::with_capacity(DST_EPOCHENSAAT.len() + 32);
    b.extend_from_slice(DST_EPOCHENSAAT);
    b.extend_from_slice(&s);
    myl_types::Hash::sha256(&b)
}

/// **Aufraeumen** am Wechsel nach `jetzt`: Runden ab `jetzt − 2` bleiben,
/// dazu jede, aus der ein noch geltender Schluessel stammt; Austeilungen
/// nur, solange ihre Runde bleibt.
pub fn aufraeumen(state: &mut LedgerState, jetzt: u64) {
    let quellen: Vec<u64> = state
        .saatrunden
        .range(jetzt.saturating_sub(2)..)
        .filter_map(|(_, r)| r.schluessel.as_ref().map(|s| s.aus_epoche))
        .collect();
    state.saatrunden.retain(|e, _| e.saturating_add(2) >= jetzt || quellen.contains(e));
    let bleiben: Vec<u64> = state.saatrunden.keys().copied().collect();
    state.austeilungen.retain(|(e, _), _| bleiben.contains(e));
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::anteilchiffre::{self, Entschluesselungsschluessel};
    use myl_types::schwelle::Polynom;

    const NETZ: [u8; 32] = [7; 32];

    fn m(i: u8) -> MinerId {
        MinerId::new([i; 32])
    }

    fn entschluessel(i: u8) -> Entschluesselungsschluessel {
        Entschluesselungsschluessel::ableiten(&[i; 32]).expect("Schluessel")
    }

    /// Ein Zustand mit einem Satz fuer Epoche 5 aus vier Mitgliedern, alle mit
    /// Schluessel, und der angelegten Runde.
    fn zustand(gewichte: &[u64]) -> LedgerState {
        let mut s = LedgerState::genesis(1);
        let satz: crate::stimmsatz::Satz = gewichte
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let pk = myl_types::bls::BlsSecretKey::key_gen(&[i as u8 + 1; 32]).unwrap().public_key().unwrap();
                (m(i as u8 + 1), pk, *w)
            })
            .collect();
        for (miner, _, _) in &satz {
            s.verschluesselung.insert(*miner, entschluessel(miner.as_bytes()[0]).oeffentlich());
        }
        s.stimmsaetze.insert(5, satz);
        runde_anlegen(&mut s, 5);
        s
    }

    fn austeilung(s: &LedgerState, wer: u8) -> (Polynom, Austeilung) {
        let runde = s.saatrunden.get(&5).unwrap();
        let k = kontext(&NETZ, 5, &m(wer));
        let p = Polynom::ableiten(&[wer; 32], &k, runde.schwelle).unwrap();
        let mut chiffren = Vec::new();
        for n in 1..=runde.gesamt_sitze {
            let inhaber = runde.inhaber_von(n).unwrap();
            chiffren.push(anteilchiffre::verschluesseln(&inhaber.schluessel.unwrap(), &p.anteil(n), &k, n, &[wer; 32]).unwrap());
        }
        let a = Austeilung { bindung: p.bindung(), nachweis: p.kenntnisnachweis(&k), chiffren };
        (p, a)
    }

    /// Der eigene Anteil eines Mitglieds: Summe ueber die Qualifizierten,
    /// offengelegt oder entschluesselt, wie ein Knoten ihn bildet.
    fn eigener_anteil(s: &LedgerState, wer: u8, n: u64) -> Skalar {
        let sk = s.saatrunden.get(&5).unwrap().schluessel.clone().unwrap();
        let runde = s.saatrunden.get(&5).unwrap();
        let werte: Vec<Skalar> = sk
            .qualifiziert
            .iter()
            .map(|a| {
                if let Some((_, _, w)) = runde.offenlegungen.iter().find(|(b, x, _)| b == a && *x == n) {
                    return Skalar::aus_bytes_be(w).unwrap();
                }
                let au = s.austeilungen.get(&(5, *a)).unwrap();
                entschluessel(wer).entschluesseln(&au.chiffren[(n - 1) as usize], &kontext(&NETZ, 5, a), n).unwrap()
            })
            .collect();
        schwelle::anteile_summieren(&werte)
    }

    #[test]
    fn eine_runde_ergibt_eine_saat() {
        let mut s = zustand(&[10, 10, 10, 10]);
        for wer in 1..=4u8 {
            let (_, a) = austeilung(&s, wer);
            austeilung_aufnehmen(&mut s, &NETZ, 5, &m(wer), a, Phase::Austeilen).unwrap();
        }
        assert!(abschliessen(&mut s, 5));
        let sk = schluessel_der_epoche(&s, 5).unwrap().clone();
        assert_eq!(sk.qualifiziert.len(), 4);
        let botschaft = schwelle::saatbotschaft(5);
        // Zwei Mitglieder genuegen (Schwelle 2 bei vier gleichen).
        let mut beta = None;
        for wer in [3u8, 1] {
            let teile: Vec<(u64, BlsSignature)> = sk
                .nummern_von(&m(wer))
                .map(|n| (n, schwelle::anteil_signieren(&eigener_anteil(&s, wer, n), &botschaft).unwrap()))
                .collect();
            beta = anteile_aufnehmen(&mut s, 5, &m(wer), &teile, 5).unwrap();
        }
        let beta = beta.expect("gesetzt");
        assert!(sk.gruppe.verify(&botschaft, &beta));
        assert_eq!(beta_der_epoche(&s, 5), Some(beta));
        // Danach nimmt die Runde nichts mehr.
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(2), &[], 5), Err("schon gesetzt"));
    }

    #[test]
    fn fenster_mitgliedschaft_und_doppelte_werden_abgewiesen() {
        let mut s = zustand(&[10, 10, 10, 10]);
        let (_, a) = austeilung(&s, 1);
        assert_eq!(austeilung_aufnehmen(&mut s, &NETZ, 5, &m(1), a.clone(), Phase::Beschweren), Err("nicht im Fenster"));
        assert_eq!(austeilung_aufnehmen(&mut s, &NETZ, 5, &m(9), a.clone(), Phase::Austeilen), Err("kein Mitglied"));
        // Die Austeilung von 1 unter dem Namen von 2: Kontext passt nicht.
        assert_eq!(austeilung_aufnehmen(&mut s, &NETZ, 5, &m(2), a.clone(), Phase::Austeilen), Err("Bindung oder Nachweis ungueltig"));
        let mut kurz = a.clone();
        kurz.chiffren.pop();
        assert_eq!(austeilung_aufnehmen(&mut s, &NETZ, 5, &m(1), kurz, Phase::Austeilen), Err("falsche Zahl der Chiffren"));
        austeilung_aufnehmen(&mut s, &NETZ, 5, &m(1), a.clone(), Phase::Austeilen).unwrap();
        assert_eq!(austeilung_aufnehmen(&mut s, &NETZ, 5, &m(1), a, Phase::Austeilen), Err("schon ausgeteilt"));
        // Ein anderes Netz, ein anderer Kontext.
        let (_, b) = austeilung(&s, 2);
        assert!(austeilung_aufnehmen(&mut s, &[8; 32], 5, &m(2), b, Phase::Austeilen).is_err());
    }

    /// ⚑ Ein Austeiler, der einem Sitz Muell schickt und auf die Beschwerde
    /// schweigt, faellt heraus; der Rest traegt die Saat.
    #[test]
    fn wer_auf_eine_beschwerde_schweigt_faellt_heraus() {
        let mut s = zustand(&[10, 10, 10, 10]);
        for wer in 1..=4u8 {
            let (_, mut a) = austeilung(&s, wer);
            if wer == 4 {
                a.chiffren[0].c = [0x11; 32];
            }
            austeilung_aufnehmen(&mut s, &NETZ, 5, &m(wer), a, Phase::Austeilen).unwrap();
        }
        // Sitz 1 gehoert Mitglied 1.
        assert_eq!(beschwerde_aufnehmen(&mut s, 5, &m(2), &m(4), 1, Phase::Beschweren), Err("nicht sein Sitz"));
        assert_eq!(beschwerde_aufnehmen(&mut s, 5, &m(1), &m(4), 1, Phase::Austeilen), Err("nicht im Fenster"));
        beschwerde_aufnehmen(&mut s, 5, &m(1), &m(4), 1, Phase::Beschweren).unwrap();
        assert_eq!(beschwerde_aufnehmen(&mut s, 5, &m(1), &m(4), 1, Phase::Beschweren), Err("schon beschwert"));
        // Ein falscher Wert kommt nicht hinein.
        assert_eq!(offenlegung_aufnehmen(&mut s, 5, &m(4), 1, [0xff; 32], Phase::Offenlegen), Err("keine Zahl"));
        assert_eq!(
            offenlegung_aufnehmen(&mut s, 5, &m(4), 1, Skalar::aus_u64(3).als_bytes_be(), Phase::Offenlegen),
            Err("passt nicht zur Bindung")
        );
        assert!(abschliessen(&mut s, 5));
        let sk = schluessel_der_epoche(&s, 5).unwrap();
        assert_eq!(sk.qualifiziert, vec![m(1), m(2), m(3)]);
    }

    /// Wer beantwortet, bleibt drin, und der offengelegte Wert zaehlt.
    #[test]
    fn eine_beantwortete_beschwerde_schadet_nicht() {
        let mut s = zustand(&[10, 10, 10, 10]);
        let mut polynom4 = None;
        for wer in 1..=4u8 {
            let (p, mut a) = austeilung(&s, wer);
            if wer == 4 {
                a.chiffren[0].c = [0x11; 32];
                polynom4 = Some(p);
            }
            austeilung_aufnehmen(&mut s, &NETZ, 5, &m(wer), a, Phase::Austeilen).unwrap();
        }
        beschwerde_aufnehmen(&mut s, 5, &m(1), &m(4), 1, Phase::Beschweren).unwrap();
        offenlegung_aufnehmen(&mut s, 5, &m(4), 1, polynom4.unwrap().anteil(1).als_bytes_be(), Phase::Offenlegen).unwrap();
        assert!(abschliessen(&mut s, 5));
        let sk = schluessel_der_epoche(&s, 5).unwrap().clone();
        assert_eq!(sk.qualifiziert.len(), 4);
        // Mitglied 1 rechnet mit dem offengelegten Wert und signiert gueltig.
        let botschaft = schwelle::saatbotschaft(5);
        let sig = schwelle::anteil_signieren(&eigener_anteil(&s, 1, 1), &botschaft).unwrap();
        assert!(sk.oeffentliche_anteile[0].verify(&botschaft, &sig));
    }

    /// ⛔️ Ein Drittel oder weniger qualifiziert: kein neuer Schluessel, der
    /// alte gilt weiter; ohne alten keiner.
    #[test]
    fn ohne_ein_drittel_gilt_der_alte_schluessel() {
        let mut s = zustand(&[10, 10, 10, 10]);
        let (_, a) = austeilung(&s, 1);
        austeilung_aufnehmen(&mut s, &NETZ, 5, &m(1), a, Phase::Austeilen).unwrap();
        assert!(!abschliessen(&mut s, 5));
        assert!(schluessel_der_epoche(&s, 5).is_none());
        // Mit einem Schluessel der Vorepoche.
        let mut t = zustand(&[10, 10, 10, 10]);
        for wer in 1..=4u8 {
            let (_, a) = austeilung(&t, wer);
            austeilung_aufnehmen(&mut t, &NETZ, 5, &m(wer), a, Phase::Austeilen).unwrap();
        }
        assert!(abschliessen(&mut t, 5));
        assert!(!abschliessen(&mut t, 6));
        assert_eq!(schluessel_der_epoche(&t, 6).unwrap().aus_epoche, 5);
        // Aufraeumen behaelt die Quelle des geltenden Schluessels.
        aufraeumen(&mut t, 8);
        assert!(t.saatrunden.contains_key(&5));
        assert!(t.austeilungen.contains_key(&(5, m(1))));
        aufraeumen(&mut t, 12);
        assert!(t.saatrunden.is_empty());
        assert!(t.austeilungen.is_empty());
    }

    #[test]
    fn anteile_falsch_fremd_oder_ausserhalb_werden_abgewiesen() {
        let mut s = zustand(&[10, 10, 10, 10]);
        for wer in 1..=4u8 {
            let (_, a) = austeilung(&s, wer);
            austeilung_aufnehmen(&mut s, &NETZ, 5, &m(wer), a, Phase::Austeilen).unwrap();
        }
        abschliessen(&mut s, 5);
        let botschaft = schwelle::saatbotschaft(5);
        let gut = schwelle::anteil_signieren(&eigener_anteil(&s, 1, 1), &botschaft).unwrap();
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(2), &[(1, gut)], 5), Err("nicht sein Sitz"));
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(1), &[(1, gut)], 7), Err("nicht im Fenster"));
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(1), &[(1, gut)], 4), Err("nicht im Fenster"));
        let falsch = schwelle::anteil_signieren(&eigener_anteil(&s, 1, 1), &schwelle::saatbotschaft(6)).unwrap();
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(1), &[(1, falsch)], 5), Err("Anteil ungueltig"));
        // Spaet, in der Folgeepoche, geht es noch, und doppelt zaehlt einmal.
        assert_eq!(anteile_aufnehmen(&mut s, 5, &m(1), &[(1, gut), (1, gut)], 6), Ok(None));
        assert_eq!(s.saatrunden.get(&5).unwrap().anteile.len(), 1);
    }

    #[test]
    fn ohne_verschluesselungsschluessel_keine_sitze() {
        let mut s = LedgerState::genesis(1);
        let satz: crate::stimmsatz::Satz = (1..=4u8)
            .map(|i| (m(i), myl_types::bls::BlsSecretKey::key_gen(&[i; 32]).unwrap().public_key().unwrap(), 10))
            .collect();
        for i in 1..=3u8 {
            s.verschluesselung.insert(m(i), entschluessel(i).oeffentlich());
        }
        s.stimmsaetze.insert(5, satz);
        runde_anlegen(&mut s, 5);
        let r = s.saatrunden.get(&5).unwrap();
        assert_eq!(r.gesamt_sitze, 3);
        assert_eq!(r.mitglieder[3].sitze, 0);
        assert_eq!(r.gesamtgewicht, 40);
        // Ohne Satz keine Runde.
        runde_anlegen(&mut s, 6);
        assert!(!s.saatrunden.contains_key(&6));
    }

    #[test]
    fn die_phasen_teilen_die_epoche_in_drittel() {
        assert_eq!(phase(0, 1800), Phase::Austeilen);
        assert_eq!(phase(599, 1800), Phase::Austeilen);
        assert_eq!(phase(600, 1800), Phase::Beschweren);
        assert_eq!(phase(1199, 1800), Phase::Beschweren);
        assert_eq!(phase(1200, 1800), Phase::Offenlegen);
    }
}
