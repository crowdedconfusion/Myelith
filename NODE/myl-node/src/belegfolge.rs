//! **Wechselbelege in kompakter Form** (Sicherheitsaudit A44, Entscheidung
//! B10 a vom 2026-11-10): eine Folge von Belegen am Stück, in der jeder Satz
//! nur als Unterschied zu einem Satz weiter vorn steht und jede
//! Unterzeichnerliste als Bitfeld über den Satz, der unterschreibt.
//!
//! ⚑ **Warum:** Ein [`Wechselbeleg`] traegt zwei ganze Saetze und zwei
//! Zertifikate mit ihren Unterzeichnern, rund 240 Byte je Stimmberechtigtem.
//! Ab rund 8 700 Mitgliedern passte er nicht mehr in die halbe Antwort, ab
//! rund 17 000 nicht mehr in eine ganze, und dann kam niemand mehr ueber
//! Belege ins Netz. Dabei steht der Satz fuer `e + 2` in den Belegen `e`
//! und `e + 1` je ganz, und von Epoche zu Epoche aendern sich meist nur
//! Gewichte. Hier kostet ein Mitglied, dessen Gewicht sich aendert, zwoelf
//! Byte, eines, das gleich bleibt, gar nichts, und jede Unterschrift ein Bit.
//!
//! ⚑ **Kein neuer Konsensvertrag:** Entfaltet entstehen gewoehnliche
//! Belege, und die geprueft wie bisher ([`crate::wechsel::Folge::aufnehmen`]):
//! Die Bindung im Zertifikat ist der Hash beider Saetze, also beweist sie,
//! dass richtig entfaltet wurde. Eine Gegenstelle, die falsch faltet,
//! liefert Belege, die nicht tragen, wie eine, die falsche schickt.
//!
//! ⚑ **In sich entfaltbar:** Ein Verweis zeigt nur auf einen Satz, der
//! weiter vorn in derselben Folge steht. Der Empfaenger braucht dafuer
//! keinen Zustand, und wer einen Teil verliert, verliert nicht den Rest
//! seiner Folge, sondern nur diese Antwort.
//!
//! ⛔️ **Ein Deckel vor dem Entfalten** ([`ENTFALTET_HOECHSTENS`],
//! [`BELEGE_HOECHSTENS`]): Ein Verweis vervielfacht einen Satz. Ohne Grenze
//! liesse sich in einer Antwort von 4 MiB eine Folge von Gigabytes
//! ausdruecken; die Groesse wird deshalb aus den Zaehlungen gerechnet,
//! bevor ein Satz gebaut wird.

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::block::epoche_fuer_hoehe;
use myl_consensus::pruefpunkt::{Pruefpunktzertifikat, Quelle};
use myl_ledger::stimmsatz::Satz;
use myl_types::bls::{BlsAggregateSignature, BlsPublicKey, BLS_PK_LEN};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

use crate::wechsel::{Wechselbeleg, WECHSELBELEGE_BEHALTEN};

/// ⛔️ **So gross darf eine Folge entfaltet werden**, gerechnet nach den
/// Feldern der vollen Belege. Ein Knoten haelt die Belege der Sperrfrist
/// ohnehin vollstaendig; eine Antwort traegt einen Teil davon. 64 MiB sind
/// bei 17 000 Mitgliedern rund sechzehn volle Belege, mehr als eine Antwort
/// je braucht, um weiterzukommen.
pub const ENTFALTET_HOECHSTENS: u64 = 64 << 20;

/// ⛔️ So viele Belege hoechstens in einer Folge: mehr haelt kein Knoten.
pub const BELEGE_HOECHSTENS: usize = WECHSELBELEGE_BEHALTEN as usize;

/// Was ein Mitglied eines Satzes entfaltet wiegt (Kennung, Schluessel, Gewicht).
const MITGLIED_BYTES: u64 = 32 + BLS_PK_LEN as u64 + 8;
/// Was ein Zertifikat ohne seine Unterzeichner wiegt, grob nach oben.
const ZERTIFIKAT_BYTES: u64 = 8 + 32 + 32 + 8 + 32 + 96 + 8;

/// Ein Satz in der Folge.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Satzangabe {
    /// Der Zustand fuehrt fuer diese Epoche keinen Satz.
    Keiner,
    /// Der ganze Satz.
    Voll(Satz),
    /// Derselbe Satz wie der fuer diese Epoche, weiter vorn in der Folge.
    Wie(u64),
    /// Der Satz der Epoche `basis` (weiter vorn), mit diesen Aenderungen.
    Aenderung {
        /// Die Epoche des Satzes, von dem ausgegangen wird.
        basis: u64,
        /// Die Stellen in `basis`, die wegfallen, aufsteigend.
        entfernt: Vec<u32>,
        /// Stelle in `basis` und neues Gewicht, aufsteigend nach Stelle.
        gewichte: Vec<(u32, u64)>,
        /// Neue Mitglieder, aufsteigend nach Kennung.
        neu: Vec<(MinerId, BlsPublicKey, u64)>,
    },
}

/// Die Unterzeichner eines Zertifikats.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Unterzeichnerangabe {
    /// Ausgeschrieben, wenn der Satz, der unterschreibt, nicht in der Folge steht.
    Liste(Vec<MinerId>),
    /// Je Mitglied des Satzes der Epoche `satz` (nach Kennung aufsteigend)
    /// ein Bit; Bit `i` steht in Byte `i / 8` an Stelle `i % 8`.
    Bitfeld {
        /// Die Epoche des Satzes, der unterschreibt.
        satz: u64,
        /// Die Bits, genau so viele Bytes wie noetig, ohne gesetzte Bits
        /// hinter dem letzten Mitglied.
        bits: Vec<u8>,
    },
}

/// Ein Zertifikat mit kompakten Unterzeichnern.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kompaktzertifikat {
    pub hoehe: u64,
    pub block_hash: Hash,
    pub bindung: Hash,
    pub quelle: Quelle,
    pub unterzeichner: Unterzeichnerangabe,
    pub aggregat: BlsAggregateSignature,
}

/// Ein Beleg mit kompakten Saetzen und Zertifikaten.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kompaktbeleg {
    pub zertifikat: Kompaktzertifikat,
    pub naechster: Satzangabe,
    pub uebernaechster: Satzangabe,
    pub abschluss: Kompaktzertifikat,
}

/// **Eine Folge von Belegen am Stueck**, nach Epochen aufsteigend.
#[derive(Debug, Clone, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Belegfolge(pub Vec<Kompaktbeleg>);

/// Warum eine Folge nicht entfaltet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faltfehler {
    /// Mehr Belege als [`BELEGE_HOECHSTENS`].
    ZuVieleBelege,
    /// Entfaltet groesser als [`ENTFALTET_HOECHSTENS`].
    ZuGross,
    /// Die Epochen steigen nicht streng an.
    Reihenfolge,
    /// Ein Verweis auf einen Satz, der weiter vorn nicht steht.
    VerweisUnbekannt(u64),
    /// Eine Stelle ausserhalb des Satzes, oder nicht aufsteigend.
    Stelle,
    /// Ein Bitfeld passt nicht zur Groesse seines Satzes.
    Bitfeld,
}

impl std::fmt::Display for Faltfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZuVieleBelege => write!(f, "mehr als {BELEGE_HOECHSTENS} Belege in einer Folge"),
            Self::ZuGross => write!(f, "entfaltet groesser als {ENTFALTET_HOECHSTENS} Byte"),
            Self::Reihenfolge => f.write_str("die Epochen steigen nicht streng an"),
            Self::VerweisUnbekannt(e) => write!(f, "Verweis auf den Satz der Epoche {e}, der weiter vorn nicht steht"),
            Self::Stelle => f.write_str("eine Stelle liegt ausserhalb des Satzes oder steigt nicht an"),
            Self::Bitfeld => f.write_str("ein Bitfeld passt nicht zu seinem Satz"),
        }
    }
}

/// Die Kennungen eines Satzes, aufsteigend: die Reihenfolge der Bits.
fn kennungen(satz: &Satz) -> Vec<MinerId> {
    let mut k: Vec<MinerId> = satz.iter().map(|(m, _, _)| *m).collect();
    k.sort_unstable();
    k.dedup();
    k
}

/// Wendet eine Aenderung auf `basis` an: erst die Gewichte, dann wegfallen,
/// dann die neuen nach Kennung einsortiert (`basis` aufsteigend vorausgesetzt;
/// sonst faltet [`falten`] gar nicht erst so).
fn anwenden(basis: &Satz, entfernt: &[u32], gewichte: &[(u32, u64)], neu: &[(MinerId, BlsPublicKey, u64)]) -> Result<Satz, Faltfehler> {
    let n = basis.len();
    // Jede Stelle im Satz, streng aufsteigend.
    let streng = |stellen: &[u32]| stellen.iter().all(|i| (*i as usize) < n) && stellen.windows(2).all(|w| w[0] < w[1]);
    let gewichtsstellen: Vec<u32> = gewichte.iter().map(|(i, _)| *i).collect();
    if !streng(entfernt) || !streng(&gewichtsstellen) {
        return Err(Faltfehler::Stelle);
    }
    if neu.windows(2).any(|w| w[0].0 >= w[1].0) {
        return Err(Faltfehler::Stelle);
    }
    let mut satz = basis.clone();
    for (i, w) in gewichte {
        satz[*i as usize].2 = *w;
    }
    let mut weg = vec![false; n];
    for i in entfernt {
        weg[*i as usize] = true;
    }
    let mut bleibt: Satz = satz.into_iter().zip(weg).filter(|(_, w)| !w).map(|(m, _)| m).collect();
    bleibt.extend(neu.iter().cloned());
    bleibt.sort_by_key(|m| m.0);
    Ok(bleibt)
}

/// Die kleinste Angabe fuer `ziel`, gegen einen Satz weiter vorn; ⚑ nur, was
/// beim Entfalten genau `ziel` ergibt, sonst ganz.
fn satz_falten(ziel: &Option<Satz>, vorn: &BTreeMap<u64, Option<Satz>>) -> Satzangabe {
    let Some(ziel) = ziel else {
        return Satzangabe::Keiner;
    };
    if let Some((e, _)) = vorn.iter().rev().find(|(_, s)| s.as_ref() == Some(ziel)) {
        return Satzangabe::Wie(*e);
    }
    let voll = Satzangabe::Voll(ziel.clone());
    let Some((basis_e, Some(basis))) = vorn.iter().rev().find(|(_, s)| s.is_some()) else {
        return voll;
    };
    let in_basis: BTreeMap<MinerId, (u32, &BlsPublicKey, u64)> =
        basis.iter().enumerate().map(|(i, (m, k, w))| (*m, (i as u32, k, *w))).collect();
    let im_ziel: BTreeMap<MinerId, (&BlsPublicKey, u64)> = ziel.iter().map(|(m, k, w)| (*m, (k, *w))).collect();
    let mut entfernt = Vec::new();
    let mut gewichte = Vec::new();
    for (m, (i, k, w)) in &in_basis {
        match im_ziel.get(m) {
            Some((kz, wz)) if *kz == *k => {
                if wz != w {
                    gewichte.push((*i, *wz));
                }
            }
            _ => entfernt.push(*i),
        }
    }
    entfernt.sort_unstable();
    gewichte.sort_unstable();
    let neu: Vec<(MinerId, BlsPublicKey, u64)> = ziel
        .iter()
        .filter(|(m, k, _)| in_basis.get(m).is_none_or(|(_, kb, _)| *kb != k))
        .cloned()
        .collect();
    let mut neu = neu;
    neu.sort_by_key(|m| m.0);
    match anwenden(basis, &entfernt, &gewichte, &neu) {
        Ok(s) if &s == ziel => {
            let a = Satzangabe::Aenderung { basis: *basis_e, entfernt, gewichte, neu };
            let laenge = |x: &Satzangabe| borsh::to_vec(x).map_or(usize::MAX, |v| v.len());
            if laenge(&a) < laenge(&voll) {
                a
            } else {
                voll
            }
        }
        _ => voll,
    }
}

fn zertifikat_falten(z: &Pruefpunktzertifikat, vorn: &BTreeMap<u64, Option<Satz>>) -> Kompaktzertifikat {
    let e = epoche_fuer_hoehe(z.hoehe);
    let unterzeichner = match vorn.get(&e) {
        Some(Some(satz)) => {
            let k = kennungen(satz);
            let mut bits = vec![0u8; k.len().div_ceil(8)];
            let alle_drin = z.unterzeichner.iter().all(|u| match k.binary_search(u) {
                Ok(i) => {
                    bits[i / 8] |= 1 << (i % 8);
                    true
                }
                Err(_) => false,
            });
            if alle_drin {
                Unterzeichnerangabe::Bitfeld { satz: e, bits }
            } else {
                Unterzeichnerangabe::Liste(z.unterzeichner.clone())
            }
        }
        _ => Unterzeichnerangabe::Liste(z.unterzeichner.clone()),
    };
    Kompaktzertifikat { hoehe: z.hoehe, block_hash: z.block_hash, bindung: z.bindung, quelle: z.quelle, unterzeichner, aggregat: z.aggregat }
}

/// **Belege falten**, nach Epochen aufsteigend; gleiche Epochen nur einmal.
pub fn falten(belege: &[Wechselbeleg]) -> Belegfolge {
    falten_bis(belege.iter(), usize::MAX)
}

/// **Falten, bis eine Antwort voll ist**: hoechstens `bytes` gefaltet und
/// nie mehr, als der Empfaenger entfalten darf ([`ENTFALTET_HOECHSTENS`],
/// [`BELEGE_HOECHSTENS`]); mindestens ein Beleg, wenn es einen gibt und er
/// allein den Deckel einhaelt. Inkrementell, damit eine Antwort nicht
/// mehrfach gefaltet wird.
pub fn falten_bis<'a>(belege: impl IntoIterator<Item = &'a Wechselbeleg>, bytes: usize) -> Belegfolge {
    let mut sortiert: Vec<&Wechselbeleg> = belege.into_iter().collect();
    sortiert.sort_by_key(|b| b.zertifikat.hoehe);
    sortiert.dedup_by_key(|b| b.epoche());
    let mut vorn: BTreeMap<u64, Option<Satz>> = BTreeMap::new();
    let mut aus = Vec::with_capacity(sortiert.len().min(BELEGE_HOECHSTENS));
    let mut gefaltet = 4usize;
    let mut entfaltet = 0u64;
    for b in sortiert.into_iter().take(BELEGE_HOECHSTENS) {
        let e = b.epoche();
        let zertifikat = zertifikat_falten(&b.zertifikat, &vorn);
        let naechster = satz_falten(&b.naechster, &vorn);
        // Gleich eintragen: Nach einem Abbruch wird `vorn` nicht mehr gebraucht.
        vorn.insert(e + 1, b.naechster.clone());
        let uebernaechster = satz_falten(&b.uebernaechster, &vorn);
        vorn.insert(e + 2, b.uebernaechster.clone());
        let abschluss = zertifikat_falten(&b.abschluss, &vorn);
        let kb = Kompaktbeleg { zertifikat, naechster, uebernaechster, abschluss };
        let n = borsh::to_vec(&kb).map_or(usize::MAX, |v| v.len());
        let groesse = |s: &Option<Satz>| s.as_ref().map_or(0, |s| s.len() as u64);
        let zusatz = (groesse(&b.naechster) + groesse(&b.uebernaechster)) * MITGLIED_BYTES
            + (b.zertifikat.unterzeichner.len() + b.abschluss.unterzeichner.len()) as u64 * 32
            + 2 * ZERTIFIKAT_BYTES;
        let zu_viel = (!aus.is_empty() && gefaltet.saturating_add(n) > bytes) || entfaltet.saturating_add(zusatz) > ENTFALTET_HOECHSTENS;
        if zu_viel {
            break;
        }
        gefaltet = gefaltet.saturating_add(n);
        entfaltet = entfaltet.saturating_add(zusatz);
        aus.push(kb);
    }
    Belegfolge(aus)
}

/// Was eine Satzangabe entfaltet an Mitgliedern ergibt, ohne sie zu bauen.
fn mitglieder(a: &Satzangabe, groesse: &BTreeMap<u64, u64>) -> Result<u64, Faltfehler> {
    Ok(match a {
        Satzangabe::Keiner => 0,
        Satzangabe::Voll(s) => s.len() as u64,
        Satzangabe::Wie(e) => *groesse.get(e).ok_or(Faltfehler::VerweisUnbekannt(*e))?,
        Satzangabe::Aenderung { basis, entfernt, neu, .. } => {
            let b = *groesse.get(basis).ok_or(Faltfehler::VerweisUnbekannt(*basis))?;
            b.checked_sub(entfernt.len() as u64).ok_or(Faltfehler::Stelle)?.saturating_add(neu.len() as u64)
        }
    })
}

fn unterzeichnerzahl(u: &Unterzeichnerangabe, groesse: &BTreeMap<u64, u64>) -> Result<u64, Faltfehler> {
    Ok(match u {
        Unterzeichnerangabe::Liste(l) => l.len() as u64,
        Unterzeichnerangabe::Bitfeld { satz, .. } => *groesse.get(satz).ok_or(Faltfehler::VerweisUnbekannt(*satz))?,
    })
}

/// ⛔️ **Erst rechnen, dann bauen:** Reihenfolge, Verweise und die
/// entfaltete Groesse aus den Zaehlungen allein.
fn vorpruefen(f: &Belegfolge) -> Result<(), Faltfehler> {
    if f.0.len() > BELEGE_HOECHSTENS {
        return Err(Faltfehler::ZuVieleBelege);
    }
    let mut groesse: BTreeMap<u64, u64> = BTreeMap::new();
    let mut summe: u64 = 0;
    let mut vorher: Option<u64> = None;
    for b in &f.0 {
        let e = epoche_fuer_hoehe(b.zertifikat.hoehe);
        if vorher.is_some_and(|v| v >= e) {
            return Err(Faltfehler::Reihenfolge);
        }
        vorher = Some(e);
        let z = unterzeichnerzahl(&b.zertifikat.unterzeichner, &groesse)?;
        let n1 = mitglieder(&b.naechster, &groesse)?;
        groesse.insert(e + 1, n1);
        let n2 = mitglieder(&b.uebernaechster, &groesse)?;
        groesse.insert(e + 2, n2);
        let a = unterzeichnerzahl(&b.abschluss.unterzeichner, &groesse)?;
        let beleg = (n1 + n2) * MITGLIED_BYTES + (z + a) * 32 + 2 * ZERTIFIKAT_BYTES;
        summe = summe.saturating_add(beleg);
        if summe > ENTFALTET_HOECHSTENS {
            return Err(Faltfehler::ZuGross);
        }
    }
    Ok(())
}

fn satz_entfalten(a: &Satzangabe, vorn: &BTreeMap<u64, Option<Satz>>) -> Result<Option<Satz>, Faltfehler> {
    match a {
        Satzangabe::Keiner => Ok(None),
        Satzangabe::Voll(s) => Ok(Some(s.clone())),
        Satzangabe::Wie(e) => vorn.get(e).cloned().ok_or(Faltfehler::VerweisUnbekannt(*e)),
        Satzangabe::Aenderung { basis, entfernt, gewichte, neu } => {
            let Some(Some(b)) = vorn.get(basis) else {
                return Err(Faltfehler::VerweisUnbekannt(*basis));
            };
            anwenden(b, entfernt, gewichte, neu).map(Some)
        }
    }
}

fn zertifikat_entfalten(z: &Kompaktzertifikat, vorn: &BTreeMap<u64, Option<Satz>>) -> Result<Pruefpunktzertifikat, Faltfehler> {
    let unterzeichner = match &z.unterzeichner {
        Unterzeichnerangabe::Liste(l) => l.clone(),
        Unterzeichnerangabe::Bitfeld { satz, bits } => {
            let Some(Some(s)) = vorn.get(satz) else {
                return Err(Faltfehler::VerweisUnbekannt(*satz));
            };
            let k = kennungen(s);
            if bits.len() != k.len().div_ceil(8) {
                return Err(Faltfehler::Bitfeld);
            }
            // Kanonisch: keine gesetzten Bits hinter dem letzten Mitglied.
            if !k.len().is_multiple_of(8) && bits.last().is_some_and(|b| b >> (k.len() % 8) != 0) {
                return Err(Faltfehler::Bitfeld);
            }
            k.iter().enumerate().filter(|(i, _)| bits[i / 8] & (1 << (i % 8)) != 0).map(|(_, m)| *m).collect()
        }
    };
    Ok(Pruefpunktzertifikat {
        hoehe: z.hoehe,
        block_hash: z.block_hash,
        bindung: z.bindung,
        quelle: z.quelle,
        unterzeichner,
        aggregat: z.aggregat,
    })
}

/// **Eine Folge entfalten**, nach dem Deckel. Geprueft wird hier nur die
/// Form; ob die Belege tragen, prueft die Folge des Knotens.
pub fn entfalten(f: &Belegfolge) -> Result<Vec<Wechselbeleg>, Faltfehler> {
    vorpruefen(f)?;
    let mut vorn: BTreeMap<u64, Option<Satz>> = BTreeMap::new();
    let mut aus = Vec::with_capacity(f.0.len());
    for b in &f.0 {
        let e = epoche_fuer_hoehe(b.zertifikat.hoehe);
        let zertifikat = zertifikat_entfalten(&b.zertifikat, &vorn)?;
        let naechster = satz_entfalten(&b.naechster, &vorn)?;
        vorn.insert(e + 1, naechster.clone());
        let uebernaechster = satz_entfalten(&b.uebernaechster, &vorn)?;
        vorn.insert(e + 2, uebernaechster.clone());
        let abschluss = zertifikat_entfalten(&b.abschluss, &vorn)?;
        aus.push(Wechselbeleg { zertifikat, naechster, uebernaechster, abschluss });
    }
    Ok(aus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wechsel::probe::{beleg, satz_mit};
    use crate::wechsel::{als_stimmsatz, Folge};

    fn id(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Ein grosser Satz: `n` Mitglieder, je ein eigener Schluessel aus wenigen
    /// echten (fuer die Groesse zaehlt nur die Form), Gewichte nach `saat`.
    fn gross(n: u32, saat: u64) -> Satz {
        let schluessel = satz_mit(1, 1);
        (0..n)
            .map(|i| {
                let mut b = [0u8; 32];
                b[..4].copy_from_slice(&i.to_be_bytes());
                let w = 1_000 + (u64::from(i).wrapping_mul(2_654_435_761) ^ saat) % 1_000;
                (MinerId::new(b), schluessel[(i % 4) as usize].1, w)
            })
            .collect()
    }

    /// ⚑ **Hin und zurueck ergibt dieselben Belege**: ueber eine Luecke
    /// (keine Belege fuer 5 und 6), einen Satz, den der Zustand nicht fuehrt,
    /// einen Wechsel der Mitglieder und wiederkehrende Saetze.
    #[test]
    fn falten_und_entfalten_ergibt_dieselben_belege() {
        let a = satz_mit(1, 10);
        let b = satz_mit(11, 10);
        let mut c = a.clone();
        c[1].2 = 17;
        c.pop();
        c.push((id(9), satz_mit(9, 1)[0].1, 5));
        c.sort_by_key(|m| m.0);
        let belege = vec![
            beleg(1, &[100, 101, 102], None, Some(a.clone())),
            beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone())),
            beleg(3, &[1, 2, 3], Some(a.clone()), Some(c.clone())),
            beleg(4, &[1, 2, 4], Some(c.clone()), Some(b.clone())),
            beleg(7, &[11, 12, 13], Some(b.clone()), None),
        ];
        let f = falten(&belege);
        assert_eq!(entfalten(&f), Ok(belege.clone()));
        // Und es ist wirklich gefaltet: Verweise, Aenderung, Bitfelder.
        assert!(matches!(f.0[2].naechster, Satzangabe::Wie(4)), "{:?}", f.0[2].naechster);
        assert!(matches!(f.0[2].uebernaechster, Satzangabe::Aenderung { .. }), "{:?}", f.0[2].uebernaechster);
        assert!(matches!(f.0[3].zertifikat.unterzeichner, Unterzeichnerangabe::Bitfeld { satz: 4, .. }));
        assert!(matches!(f.0[0].zertifikat.unterzeichner, Unterzeichnerangabe::Liste(_)), "Satz von aussen steht nicht in der Folge");
        // Entfaltet traegt die Folge wie die vollen Belege.
        let mut voll = Folge::ab_genesis(als_stimmsatz(&satz_mit(100, 10)), 40);
        voll.folgen(&belege).expect("voll");
        let mut gefaltet = Folge::ab_genesis(als_stimmsatz(&satz_mit(100, 10)), 40);
        gefaltet.folgen(&entfalten(&f).expect("entfaltet")).expect("gefaltet");
        assert_eq!(voll.bekannt_bis(), gefaltet.bekannt_bis());
    }

    /// ⚑ **Die Groesse, um die es geht**: 10 000 Mitglieder, je Epoche
    /// aendern sich alle Gewichte (der unguenstige Fall), zwanzig Belege.
    /// Voll rund 220 Byte je Mitglied und Beleg, gefaltet die geaenderten
    /// Gewichte (zwoelf Byte), die Bits und einmal der erste ganze Satz;
    /// gerechnet rund ein Zwoelftel.
    #[test]
    fn eine_folge_von_zehntausend_mitgliedern_schrumpft_auf_unter_ein_achtel() {
        let saetze: Vec<Satz> = (0..22).map(|e| gross(10_000, e)).collect();
        let belege: Vec<Wechselbeleg> = (0..20u64)
            .map(|e| {
                let mut b = beleg(e + 3, &[1, 2, 3], Some(saetze[e as usize + 1].clone()), Some(saetze[e as usize + 2].clone()));
                // Unterzeichner: zwei Drittel des Satzes der Epoche (nur die Form zaehlt).
                let k = kennungen(&saetze[e as usize]);
                b.zertifikat.unterzeichner = k.iter().step_by(3).chain(k.iter().skip(1).step_by(3)).copied().collect();
                b.zertifikat.unterzeichner.sort_unstable();
                b.abschluss.unterzeichner = b.zertifikat.unterzeichner.clone();
                b
            })
            .collect();
        let voll: usize = belege.iter().map(|b| borsh::to_vec(b).expect("borsh").len()).sum();
        let f = falten(&belege);
        let gefaltet = borsh::to_vec(&f).expect("borsh").len();
        assert!(gefaltet * 8 < voll, "gefaltet {gefaltet} Byte gegen voll {voll}");
        assert!(entfalten(&f).expect("entfaltet") == belege, "hin und zurueck nicht gleich");
    }

    /// ⛔️ **Ein Verweis vervielfacht nicht ohne Grenze**: `falten` haelt den
    /// Deckel selbst ein, und eine Folge von aussen, die auf denselben grossen
    /// Satz immer wieder verweist, wird vor dem Bauen abgewiesen.
    #[test]
    fn eine_folge_die_entfaltet_zu_gross_waere_wird_vorher_abgewiesen() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let s = gross(17_000, 1);
        let belege: Vec<Wechselbeleg> = (1..30).map(|e| beleg(e, &[1, 2, 3], Some(s.clone()), Some(s.clone()))).collect();
        let f = falten(&belege);
        assert!(f.0.len() < belege.len(), "falten hielt den Deckel nicht ein: {} Belege", f.0.len());
        assert!(entfalten(&f).is_ok());
        // Von aussen: nach dem ersten nur noch Verweise, jeder mit dem ganzen Satz.
        let mut boese = Belegfolge(vec![f.0[0].clone()]);
        for e in 2..40u64 {
            let mut kb = f.0[0].clone();
            kb.zertifikat.hoehe = e * BLOECKE_JE_EPOCHE + 150;
            kb.abschluss.hoehe = kb.zertifikat.hoehe + 150;
            kb.naechster = Satzangabe::Wie(e);
            kb.uebernaechster = Satzangabe::Wie(e);
            boese.0.push(kb);
        }
        assert!(borsh::to_vec(&boese).expect("borsh").len() < 4 << 20, "die Antwort selbst passt");
        assert!(matches!(entfalten(&boese), Err(Faltfehler::ZuGross)));
        // Gegenprobe: dieselbe Form mit kleinem Satz geht durch.
        let klein = satz_mit(1, 10);
        let kurz: Vec<Wechselbeleg> = (1..40).map(|e| beleg(e, &[1, 2, 3], Some(klein.clone()), Some(klein.clone()))).collect();
        assert!(entfalten(&falten(&kurz)).is_ok());
    }

    /// ⚑ **Antwort fuer Antwort zur ganzen Folge**: Jede haelt die Bytes ein
    /// (ausser einem einzelnen Beleg, der allein groesser ist), enthaelt
    /// mindestens einen, und ab dem ersten fehlenden weitergefragt ergeben
    /// sie zusammen genau die Belege.
    #[test]
    fn antworten_nacheinander_ergeben_die_ganze_folge() {
        let saetze: Vec<Satz> = (0..14).map(|e| gross(300, e)).collect();
        let belege: Vec<Wechselbeleg> =
            (0..12u64).map(|e| beleg(e + 3, &[1, 2, 3], Some(saetze[e as usize + 1].clone()), Some(saetze[e as usize + 2].clone()))).collect();
        let grenze = 12_000;
        let mut erhalten: Vec<Wechselbeleg> = Vec::new();
        let mut antworten = 0;
        while erhalten.len() < belege.len() {
            let ab = erhalten.last().map_or(0, |b| b.epoche() + 1);
            let rest: Vec<&Wechselbeleg> = belege.iter().filter(|b| b.epoche() >= ab).collect();
            let f = falten_bis(rest, grenze);
            assert!(!f.0.is_empty(), "eine Antwort ohne Beleg, obwohl es noch welche gibt");
            let n = borsh::to_vec(&f).expect("borsh").len();
            assert!(n <= grenze || f.0.len() == 1, "{n} Byte bei {} Belegen", f.0.len());
            erhalten.extend(entfalten(&f).expect("entfaltet"));
            antworten += 1;
            assert!(antworten <= belege.len(), "keine Antwort kam weiter");
        }
        assert!(antworten > 1, "die Grenze waehlt nicht aus");
        assert_eq!(erhalten, belege);
    }

    /// Verweise nur nach vorn, Stellen im Satz und aufsteigend, Bitfelder in
    /// der richtigen Laenge und ohne Bits hinter dem Ende, Epochen streng
    /// aufsteigend: jede Verletzung ist ein eigener Fehler.
    #[test]
    fn eine_falsch_geformte_folge_wird_abgewiesen() {
        let a = satz_mit(1, 10);
        let gut = falten(&[beleg(1, &[100, 101, 102], None, Some(a.clone())), beleg(2, &[1, 2, 3], Some(a.clone()), Some(a.clone()))]);
        assert!(entfalten(&gut).is_ok());
        let mit = |f: &dyn Fn(&mut Belegfolge)| {
            let mut x = gut.clone();
            f(&mut x);
            entfalten(&x)
        };
        assert_eq!(mit(&|x| x.0[1].naechster = Satzangabe::Wie(9)), Err(Faltfehler::VerweisUnbekannt(9)));
        assert_eq!(
            mit(&|x| x.0[1].naechster = Satzangabe::Aenderung { basis: 2, entfernt: vec![7], gewichte: vec![], neu: vec![] }),
            Err(Faltfehler::Stelle)
        );
        assert_eq!(
            mit(&|x| x.0[1].naechster = Satzangabe::Aenderung { basis: 2, entfernt: vec![1, 0], gewichte: vec![], neu: vec![] }),
            Err(Faltfehler::Stelle)
        );
        assert_eq!(mit(&|x| x.0[1].zertifikat.unterzeichner = Unterzeichnerangabe::Bitfeld { satz: 3, bits: vec![0, 0] }), Err(Faltfehler::Bitfeld));
        assert_eq!(mit(&|x| x.0[1].zertifikat.unterzeichner = Unterzeichnerangabe::Bitfeld { satz: 3, bits: vec![0b1_0000] }), Err(Faltfehler::Bitfeld));
        assert_eq!(mit(&|x| x.0[1] = x.0[0].clone()), Err(Faltfehler::Reihenfolge));
        assert_eq!(
            entfalten(&Belegfolge(vec![gut.0[0].clone(); BELEGE_HOECHSTENS + 1])),
            Err(Faltfehler::ZuVieleBelege)
        );
    }
}
