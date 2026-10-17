//! **Sechs Karten ohne Karte**: nachgebildete Karten, die auf der CPU
//! rechnen, und an ihnen die Verteilung so, wie sie auf einer Maschine mit
//! sechs Karten laeuft.
//!
//! # ⚑ Wozu
//!
//! Vorbereitet fuer eine gemietete Maschine mit sechs Karten (2026-10-14).
//! Was dort zum ersten Mal zusammenkommt, ist hier schon gelaufen: der
//! Plan des Verteilers, ein Faden je Karte, das Zusammensetzen der Teile,
//! ungleiche Gewichte, eine Karte, die ausfaellt, und der Ersatzlauf auf
//! den uebrigen. Offen bleibt auf der Mietmaschine nur, was die echte Karte
//! anders macht als diese hier, und dafuer gibt es die Selbstpruefung.
//!
//! # ⚑ Die nachgebildete Karte rechnet nicht ueber den Kern
//!
//! Sie rechnet je Zeile die exakte Summe in i64 und skaliert mit den
//! Funktionen aus `fixed_point.rs`. Verglichen wird mit dem Stapelweg der
//! CPU (`linear_w8a16_stapel_viele_cpu`), also mit einem anderen Weg zur
//! selben Zahl. Ob die Kerne stimmen, prueft der Nachbau (`nachbau.rs`);
//! hier geht es um die Verteilung.

use super::verteiler::Teil;
use super::{auf_karten, mit_ersatz, Auftrag, Gewichte, Job, Rechner};
use crate::fixed_point::{clamp_i16_from_i64, rshift_round_i64};
use crate::linear::{self, Ausgangsskala, Gewichtsmatrix, Stapelauftrag};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Eine nachgebildete Karte.
struct Attrappe {
    gewicht: u32,
    /// Ab dem wievielten Aufruf sie ausfaellt; `usize::MAX` heisst nie.
    faellt_aus_ab: usize,
    aufrufe: AtomicUsize,
    teile: AtomicUsize,
    zeilen: AtomicUsize,
}

impl Attrappe {
    fn neu(gewicht: u32) -> Self {
        Attrappe {
            gewicht,
            faellt_aus_ab: usize::MAX,
            aufrufe: AtomicUsize::new(0),
            teile: AtomicUsize::new(0),
            zeilen: AtomicUsize::new(0),
        }
    }
}

/// Eine Zeile eines Jobs mal eine Eingabe, exakt, dann umskaliert.
fn zeile(job: &Job<'_>, z: usize, x: &[i16]) -> i16 {
    let s = job.spalten;
    let summe: i64 = match job.w {
        Gewichte::Int8(w) => w[z * s..(z + 1) * s].iter().zip(x).map(|(&w, &x)| w as i64 * x as i64).sum(),
        Gewichte::Ternaer { muster, betraege } => crate::ternaer::Ternaermatrix::neu(muster, betraege, job.zeilen(), s)
            .expect("ternaere Form")
            .zeile_mal(z, x),
        Gewichte::Vierbit { codes, gitter } => crate::vierbit::Vierbitmatrix::neu(codes, gitter, job.zeilen(), s)
            .expect("Vierbitform")
            .zeile_mal(z, x),
    };
    let a = job.abstaende[z];
    let y = if a >= 0 { rshift_round_i64(summe, a as u8) } else { summe << (-a) };
    clamp_i16_from_i64(y)
}

impl Rechner for Attrappe {
    fn gewicht(&self) -> u32 {
        self.gewicht
    }

    fn teile_rechnen(&self, teile: &[Teil], jobs: &[Job<'_>]) -> Result<Vec<Vec<i16>>, String> {
        if self.aufrufe.fetch_add(1, Ordering::Relaxed) >= self.faellt_aus_ab {
            return Err("nachgebildeter Ausfall".into());
        }
        let aus = teile
            .iter()
            .map(|t| {
                let j = &jobs[t.auftrag];
                j.xs.iter().flat_map(|x| (t.von..t.bis).map(move |z| zeile(j, z, x))).collect()
            })
            .collect();
        self.teile.fetch_add(teile.len(), Ordering::Relaxed);
        self.zeilen.fetch_add(teile.iter().map(|t| t.bis - t.von).sum::<usize>(), Ordering::Relaxed);
        Ok(aus)
    }
}

struct Zufall(u64);

impl Zufall {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Die Daten eines Auftrags, damit Job und CPU-Auftrag darauf zeigen.
struct Daten {
    zeilen: usize,
    spalten: usize,
    int8: Option<Vec<i8>>,
    muster: Vec<u8>,
    betraege: Vec<i16>,
    eingaben: Vec<Vec<i16>>,
    w_shifts: Vec<u8>,
    ziele: Vec<u8>,
}

fn daten(z: &mut Zufall, zeilen: usize, spalten: usize, stapel: usize, ternaer: bool) -> Daten {
    let gruppen = spalten / crate::ternaer::GRUPPE;
    Daten {
        zeilen,
        spalten,
        int8: (!ternaer).then(|| (0..zeilen * spalten).map(|_| z.naechste() as i8).collect()),
        muster: if ternaer {
            (0..zeilen * spalten / 4).map(|_| (z.naechste() % 256) as u8).collect()
        } else {
            Vec::new()
        },
        betraege: if ternaer { (0..zeilen * gruppen).map(|_| (z.naechste() % 4_096) as i16).collect() } else { Vec::new() },
        eingaben: (0..stapel).map(|_| (0..spalten).map(|_| z.naechste() as i16).collect()).collect(),
        w_shifts: (0..zeilen).map(|r| [9u8, 3, 0, 0][r % 4]).collect(),
        ziele: (0..zeilen).map(|r| [0u8, 0, 11, 12][r % 4]).collect(),
    }
}

/// Rechnet die Daten ueber die Attrappen und ueber die CPU und vergleicht.
fn vergleichen(alle: &[Daten], karten: &[&dyn Rechner]) -> Vec<Vec<Vec<i16>>> {
    let scheiben: Vec<Vec<&[i16]>> = alle.iter().map(|d| d.eingaben.iter().map(|e| e.as_slice()).collect()).collect();
    let ziele: Vec<Box<dyn Fn(usize) -> u8 + Sync + '_>> =
        alle.iter().map(|d| Box::new(move |r: usize| d.ziele[r]) as Box<dyn Fn(usize) -> u8 + Sync>).collect();
    let auftraege: Vec<Auftrag<'_>> = alle
        .iter()
        .zip(&scheiben)
        .zip(&ziele)
        .map(|((d, xs), ziel)| Auftrag {
            xs,
            w: match &d.int8 {
                Some(w) => Gewichte::Int8(w),
                None => Gewichte::Ternaer { muster: &d.muster, betraege: &d.betraege },
            },
            in_features: d.spalten,
            w_shifts: &d.w_shifts,
            act_frac_bits: 11,
            ziel: ziel.as_ref(),
        })
        .collect();
    let jobs: Vec<Job<'_>> = auftraege.iter().map(|a| Job::aus(a).expect("Form")).collect();
    let karten_ergebnis = auf_karten(karten, &jobs).unwrap_or_else(|(k, g)| panic!("Karte {k:?}: {g}"));

    let matrizen: Vec<Gewichtsmatrix<'_>> = alle
        .iter()
        .map(|d| match &d.int8 {
            Some(w) => Gewichtsmatrix::Int8(w),
            None => crate::ternaer::Ternaermatrix::neu(&d.muster, &d.betraege, d.zeilen, d.spalten).expect("Form").into(),
        })
        .collect();
    let cpu_auftraege: Vec<Stapelauftrag<'_>> = alle
        .iter()
        .zip(&scheiben)
        .zip(&matrizen)
        .map(|((d, xs), w)| Stapelauftrag {
            xs,
            w: *w,
            in_features: d.spalten,
            w_shifts: &d.w_shifts,
            act_frac_bits: 11,
            aus: Ausgangsskala::JeZeile(&d.ziele),
        })
        .collect();
    let cpu = linear::linear_w8a16_stapel_viele_cpu(&cpu_auftraege);
    assert_eq!(karten_ergebnis.len(), cpu.len());
    for (i, (k, c)) in karten_ergebnis.iter().zip(&cpu).enumerate() {
        assert!(k == c, "Auftrag {i} weicht von der CPU ab");
    }
    karten_ergebnis
}

fn als_rechner(a: &[Attrappe]) -> Vec<&dyn Rechner> {
    a.iter().map(|k| k as &dyn Rechner).collect()
}

/// ⚑ **Eine grosse Matrix auf sechs Karten**: nach Zeilen geteilt, jede
/// Karte rechnet, das Ergebnis ist das der CPU.
#[test]
fn eine_matrix_auf_sechs_karten() {
    let mut z = Zufall(0x51ed_270b_27a0_e8b1);
    let alle = [daten(&mut z, 1_000, 300, 20, false)];
    let karten: Vec<Attrappe> = (0..6).map(|_| Attrappe::neu(1)).collect();
    vergleichen(&alle, &als_rechner(&karten));
    for (i, k) in karten.iter().enumerate() {
        assert!(k.zeilen.load(Ordering::Relaxed) > 0, "Karte {i} hat nichts gerechnet");
    }
    let summe: usize = karten.iter().map(|k| k.zeilen.load(Ordering::Relaxed)).sum();
    assert_eq!(summe, 1_000, "jede Zeile genau einmal");
}

/// ⚑ **Viele Experten auf sechs Karten**, int8 und ternaer gemischt: ganze
/// Auftraege, jede Karte bekommt welche, das Ergebnis ist das der CPU.
#[test]
fn viele_experten_auf_sechs_karten() {
    let mut z = Zufall(0x2545_f491_4f6c_dd1d);
    let alle: Vec<Daten> = (0..40).map(|i| daten(&mut z, 64 + (i % 3) * 32, 256, 1 + i % 5, i % 4 == 3)).collect();
    let karten: Vec<Attrappe> = (0..6).map(|_| Attrappe::neu(1)).collect();
    vergleichen(&alle, &als_rechner(&karten));
    for (i, k) in karten.iter().enumerate() {
        assert!(k.teile.load(Ordering::Relaxed) >= 4, "Karte {i}: nur {} Auftraege", k.teile.load(Ordering::Relaxed));
    }
    assert_eq!(karten.iter().map(|k| k.teile.load(Ordering::Relaxed)).sum::<usize>(), 40);
}

/// ⚑ **Ungleiche Gewichte**: Die Karte mit Gewicht 3 rechnet drei Achtel.
#[test]
fn die_gewichte_wirken_auf_sechs_karten() {
    let mut z = Zufall(0x9e37_79b9_7f4a_7c15);
    let alle = [daten(&mut z, 64 * 80, 64, 3, false)];
    let karten: Vec<Attrappe> = [1, 1, 1, 1, 1, 3].into_iter().map(Attrappe::neu).collect();
    vergleichen(&alle, &als_rechner(&karten));
    assert_eq!(karten[5].zeilen.load(Ordering::Relaxed), 64 * 30);
}

/// ⛔️ **Eine von sechs faellt aus**: Das Buendel rechnen die uebrigen fuenf,
/// bitgleich, und die ausgefallene ist benannt. Faellt sie beim zweiten
/// Aufruf aus, hat sie beim ersten noch gerechnet.
#[test]
fn eine_ausgefallene_karte_wird_ersetzt() {
    let mut z = Zufall(0x0123_4567_89ab_cdef);
    let d = daten(&mut z, 640, 128, 4, false);
    let scheiben: Vec<&[i16]> = d.eingaben.iter().map(|e| e.as_slice()).collect();
    let ziel = |r: usize| d.ziele[r];
    let auftrag = Auftrag {
        xs: &scheiben,
        w: Gewichte::Int8(d.int8.as_ref().expect("int8")),
        in_features: d.spalten,
        w_shifts: &d.w_shifts,
        act_frac_bits: 11,
        ziel: &ziel,
    };
    let jobs = [Job::aus(&auftrag).expect("Form")];
    let mut karten: Vec<Attrappe> = (0..6).map(|_| Attrappe::neu(1)).collect();
    karten[2].faellt_aus_ab = 1;

    // Erster Aufruf: alle sechs rechnen.
    let (erst, ausfaelle) = mit_ersatz(&als_rechner(&karten), &jobs);
    assert!(erst.is_some() && ausfaelle.is_empty());
    assert!(karten[2].zeilen.load(Ordering::Relaxed) > 0);

    // Zweiter Aufruf: Karte 2 faellt aus, die uebrigen fuenf rechnen dasselbe.
    let (zweit, ausfaelle) = mit_ersatz(&als_rechner(&karten), &jobs);
    assert_eq!(ausfaelle.len(), 1);
    assert_eq!(ausfaelle[0].0, 2, "die falsche Karte ist benannt");
    assert_eq!(zweit, erst, "der Ersatzlauf rechnet anders");

    // Fallen alle aus, rechnet die CPU: `None`.
    for k in karten.iter_mut() {
        k.faellt_aus_ab = 0;
    }
    let (keins, ausfaelle) = mit_ersatz(&als_rechner(&karten), &jobs);
    assert!(keins.is_none());
    assert_eq!(ausfaelle.len(), 6);
}
