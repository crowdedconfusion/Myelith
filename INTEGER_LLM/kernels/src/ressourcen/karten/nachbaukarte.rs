//! **Eine nachgebildete Karte auf der Ebene des Treibers**: Speicher
//! belegen, hoch- und herunterladen, Kerne starten, alles im
//! Hauptspeicher, die Kerne auf der CPU nachgerechnet.
//!
//! # ⚑ Wozu
//!
//! Vorbereitet fuer eine Mietmaschine mit sechs Karten (2026-10-14). Der
//! Weg vom Buendel bis zum Kernstart (`auf_karte`: Puffer, Versaetze,
//! Teilgewichte, Reihenfolge der Argumente, Herunterladen) laeuft sonst nur
//! auf einer echten Karte. Mit `MYL_GPU_NACHBAU=6` laeuft er hier mit sechs
//! Karten, samt Selbstpruefung je Karte, und ein ganzes Modell laesst sich
//! damit gegen die CPU vergleichen (`SYSTEM/install/karten-pruefen.sh
//! --nachbau 6`).
//!
//! # ⛔️ Was sie strenger macht als eine echte Karte
//!
//! **Jeder Zugriff wird gegen den belegten Speicher geprueft.** Eine echte
//! Karte schreibt bei einem falschen Versatz still in fremden Speicher oder
//! liest Unsinn; hier ist das ein Fehler mit Adresse und Laenge.
//!
//! # ⛔️ Was sie nie ist
//!
//! **Kein Rechenweg.** Sie rechnet auf der CPU. Sie zaehlt deshalb weder
//! fuer `verfuegbar_fuer` noch fuer `rechenpfad`, und ein Konformitaetslauf
//! besteht mit ihr nicht (Fund 33: die CPU unter dem Namen einer Karte).

use super::treiber::{Kern, Zeiger};
use crate::fixed_point::{clamp_i16_from_i64, rshift_round_i64};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Belegte Bloecke: Anfang, Laenge in Byte, Laenge in Worten.
static BLOECKE: Mutex<BTreeMap<u64, (usize, usize)>> = Mutex::new(BTreeMap::new());

/// Belegt einen Block. ⚑ **In Worten zu acht Byte**, damit jede Adresse
/// darin fuer `i16` ausgerichtet ist wie auf einer Karte.
pub(super) fn belegen(bytes: usize) -> Result<Zeiger, String> {
    let worte = bytes.div_ceil(8).max(1);
    let block: Box<[u64]> = vec![0u64; worte].into_boxed_slice();
    let p = Box::into_raw(block) as *mut u64 as u64;
    BLOECKE.lock().map_err(|_| "Speicherbuch vergiftet")?.insert(p, (bytes, worte));
    Ok(p)
}

pub(super) fn freigeben(p: Zeiger) {
    let Ok(mut b) = BLOECKE.lock() else { return };
    if let Some((_, worte)) = b.remove(&p) {
        // SAFETY: `p` kam aus `belegen` mit genau dieser Wortzahl und ist
        // eben aus dem Buch genommen, wird also nicht mehr benutzt.
        unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(p as *mut u64, worte))) };
    }
}

/// ⛔️ **Liegt `n` Byte ab `p` ganz in einem belegten Block?**
fn bereich(p: Zeiger, n: usize, wozu: &str) -> Result<(), String> {
    let b = BLOECKE.lock().map_err(|_| "Speicherbuch vergiftet")?;
    match b.range(..=p).next_back() {
        Some((&anfang, &(laenge, _))) if p + n as u64 <= anfang + laenge as u64 => Ok(()),
        _ => Err(format!("{wozu}: {n} Byte ab {p:#x} liegen ausserhalb des belegten Speichers")),
    }
}

pub(super) fn hoch(ziel: Zeiger, daten: &[u8]) -> Result<(), String> {
    bereich(ziel, daten.len(), "hochladen")?;
    // SAFETY: eben geprueft, dass der Bereich in einem belegten Block liegt.
    unsafe { std::ptr::copy_nonoverlapping(daten.as_ptr(), ziel as *mut u8, daten.len()) };
    Ok(())
}

pub(super) fn runter(ziel: &mut [u8], quelle: Zeiger) -> Result<(), String> {
    bereich(quelle, ziel.len(), "herunterladen")?;
    // SAFETY: wie `hoch`.
    unsafe { std::ptr::copy_nonoverlapping(quelle as *const u8, ziel.as_mut_ptr(), ziel.len()) };
    Ok(())
}

/// Ein Ausschnitt, nachdem der Bereich geprueft ist.
///
/// # Safety
///
/// `p` zeigt auf `n` Werte vom Typ `T` in einem belegten Block, ausgerichtet.
unsafe fn ausschnitt<'a, T>(p: Zeiger, n: usize) -> &'a [T] {
    std::slice::from_raw_parts(p as *const T, n)
}

/// Eine Zeile mal eine Eingabe, exakt.
type Zeilenrechnung<'a> = dyn Fn(usize, &[i16]) -> i64 + Sync + 'a;

/// Umskalieren und klemmen, wie `rescale_i64` mit einem Abstand.
fn ausgabe(summe: i64, abstand: i8) -> i16 {
    let y = if abstand >= 0 { rshift_round_i64(summe, abstand as u8) } else { summe << (-abstand) };
    clamp_i16_from_i64(y)
}

/// **Startet einen Kern**: prueft jeden Bereich, den der echte Kern lesen
/// oder schreiben wuerde, und rechnet dann dasselbe auf der CPU, verteilt
/// auf einige Faeden.
pub(super) fn starten(kern: Kern, zeiger: &[Zeiger], zeilen: usize, spalten: usize, stapel: usize) -> Result<(), String> {
    if zeilen == 0 || stapel == 0 {
        return Ok(());
    }
    let (w, m, x, a, o) = match (kern, zeiger) {
        (Kern::W8a16, &[w, x, a, o]) => {
            bereich(w, zeilen * spalten, "Gewichte")?;
            (w, 0, x, a, o)
        }
        (Kern::Ternaer, &[m, be, x, a, o]) => {
            if spalten % crate::ternaer::GRUPPE != 0 {
                return Err(format!("{spalten} Spalten sind kein Vielfaches der Gruppe"));
            }
            bereich(m, zeilen * spalten / 4, "Muster")?;
            bereich(be, zeilen * (spalten / crate::ternaer::GRUPPE) * 2, "Betraege")?;
            (m, be, x, a, o)
        }
        _ => return Err(format!("{} Argumente passen nicht zum Kern", zeiger.len())),
    };
    bereich(x, stapel * spalten * 2, "Eingaben")?;
    bereich(a, zeilen, "Abstaende")?;
    bereich(o, stapel * zeilen * 2, "Ausgaben")?;
    if [x, o].iter().any(|p| p % 2 != 0) || (kern_ist_ternaer(kern) && m % 2 != 0) {
        return Err("eine int16-Adresse ist ungerade".into());
    }
    // SAFETY: Jeder Bereich ist oben gegen den belegten Speicher geprueft,
    // die int16-Adressen auf Ausrichtung; die Ausgabe ueberschneidet sich mit
    // keinem Eingang (eigene Abschnitte in `auf_karte`).
    let (xs, abst, aus) = unsafe {
        (
            ausschnitt::<i16>(x, stapel * spalten),
            ausschnitt::<i8>(a, zeilen),
            std::slice::from_raw_parts_mut(o as *mut i16, stapel * zeilen),
        )
    };
    let zeile: Box<Zeilenrechnung<'_>> = match kern {
        Kern::W8a16 => {
            // SAFETY: wie oben.
            let gewichte = unsafe { ausschnitt::<i8>(w, zeilen * spalten) };
            Box::new(move |z, x| crate::dot::dot_i8_i16(&gewichte[z * spalten..(z + 1) * spalten], x))
        }
        Kern::Ternaer => {
            // SAFETY: wie oben.
            let (muster, betraege) = unsafe {
                (ausschnitt::<u8>(w, zeilen * spalten / 4), ausschnitt::<i16>(m, zeilen * (spalten / crate::ternaer::GRUPPE)))
            };
            let t = crate::ternaer::Ternaermatrix::neu(muster, betraege, zeilen, spalten)?;
            Box::new(move |z, x| t.zeile_mal(z, x))
        }
    };
    let faeden = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 4);
    let je = stapel.div_ceil(faeden);
    std::thread::scope(|s| {
        for (k, stueck) in aus.chunks_mut(je * zeilen).enumerate() {
            let zeile = &zeile;
            s.spawn(move || {
                for (i, ziel) in stueck.chunks_mut(zeilen).enumerate() {
                    let b = k * je + i;
                    let x = &xs[b * spalten..(b + 1) * spalten];
                    for (z, wert) in ziel.iter_mut().enumerate() {
                        *wert = ausgabe(zeile(z, x), abst[z]);
                    }
                }
            });
        }
    });
    Ok(())
}

fn kern_ist_ternaer(k: Kern) -> bool {
    matches!(k, Kern::Ternaer)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⛔️ **Ein Zugriff ueber den Block hinaus ist ein Fehler**, auch um ein
    /// Byte, und ein Zugriff genau bis zum Ende ist keiner.
    #[test]
    fn der_speicher_wird_bewacht() {
        let p = belegen(100).expect("belegen");
        assert!(hoch(p, &[1u8; 100]).is_ok());
        assert!(hoch(p + 1, &[1u8; 100]).is_err());
        let mut z = [0u8; 4];
        assert!(runter(&mut z, p + 96).is_ok());
        assert_eq!(z, [1, 1, 1, 1]);
        assert!(runter(&mut z, p + 97).is_err());
        freigeben(p);
        assert!(hoch(p, &[1u8; 1]).is_err(), "nach dem Freigeben ist der Block weg");
    }

    /// ⚑ **Sechs nachgebildete Karten bestehen die Selbstpruefung**, jede
    /// fuer sich, ueber den ganzen Weg von `auf_karte`.
    #[test]
    fn sechs_nachgebildete_karten_bestehen_die_selbstpruefung() {
        let karten = super::super::treiber::nachbaukarten(6, super::super::Hersteller::Nvidia);
        assert_eq!(karten.len(), 6);
        for k in &karten {
            assert!(k.nachgebildet());
            super::super::pruefung::selbstpruefung(k).unwrap_or_else(|g| panic!("{}: {g}", k.name));
        }
    }
}
