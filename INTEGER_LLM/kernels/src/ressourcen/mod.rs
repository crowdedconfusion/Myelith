//! **Die Rechenwerke**: wo gerechnet wird, und wer das entscheidet.
//!
//! ⚑ **Festlegung des Projektinhabers (2026-10-14):** ein Dach fuer alle
//! Rechenwege, und alle Backends darunter. Vorher lagen Metal, die
//! Steckkarten und das Backend-Trait nebeneinander in der Wurzel der Kiste,
//! und die Entscheidung, wer ein Buendel rechnet, stand doppelt in
//! `linear.rs`.
//!
//! # Was darunter liegt
//!
//! | Modul | Rechenwerk | Feature | was es rechnet |
//! |---|---|---|---|
//! | [`metal`] | GPU von Apple-Silizium | `metal` | gebuendelte Matrizen, int8 und ternaer |
//! | [`karten`] | Steckkarten von NVIDIA und AMD, auch mehrere | `cuda`, `rocm` | gebuendelte Matrizen, int8 und ternaer, auch viele Experten |
//! | [`backend`], [`backends`] | das Trait je Operation mit Referenz, SIMD und den alten Stubs fuer CUDA und ROCm | je eins | Paritaetspruefungen; der Rechenpfad nimmt es nicht |
//!
//! ⚑ **Die CPU hat hier kein eigenes Modul**, und das ist Absicht: Ihr
//! Rechenpfad sind die Kerne selbst (`linear.rs`, `dot.rs` mit NEON hinter
//! `cpu-simd`, der Fadenpool). Jedes Rechenwerk hier rechnet einen Teil
//! davon schneller und muss dabei **dieselbe Zahl** liefern; die CPU ist
//! der Massstab, gegen den sich jedes selbst prueft.
//!
//! # ⚑ Die eine Weiche
//!
//! [`stapel`], [`stapel_viele`] und [`nimmt`] sind die einzige Stelle, die
//! entscheidet, wer ein Buendel rechnet: **zuerst Metal, dann die
//! Steckkarten, sonst die CPU.** Jedes Rechenwerk gibt `None`, wenn es nicht
//! rechnen soll oder kann, und dann rechnet der naechste dieselbe Zahl.
//! 📌 Vorher stand diese Reihenfolge zweimal in `linear.rs` (Stapel und
//! viele Stapel) und einmal in `mlp.rs`; drei Orte fuer eine Entscheidung
//! laufen auseinander, und der dritte meldet sich nicht.
//!
//! # Was hier nicht entschieden wird
//!
//! **Ob ein Rechenwerk rechnet** (Geraet da, Selbstpruefung bestanden,
//! Schwelle, Schalter): Das sagt jedes selbst. **Welche Rechenwege es auf
//! dieser Uebersetzung gibt**, fuer Pruefstand und Protokoll:
//! `crate::rechenpfad`.

pub mod backend;
pub mod backends;
pub mod karten;
pub mod metal;

use crate::linear::{Gewichtsmatrix, Stapelauftrag};

/// **Wuerde ein Stapel mit `eingaben` Eingaben auf einem Rechenwerk
/// rechnen?** Fuer Aufrufer, die zwischen einem verschmolzenen Weg auf der
/// CPU und getrennten Matrizen waehlen (der MLP der Vorbereitung).
pub fn nimmt(eingaben: usize) -> bool {
    metal::nimmt(eingaben) || karten::nimmt(eingaben)
}

/// **Ein Stapel auf einem Rechenwerk**, oder `None`: Dann rechnet die CPU
/// dieselbe Zahl. `ziel(z)` ist die Ausgangsskala der Zeile `z`.
#[cfg_attr(
    not(any(all(feature = "metal", target_os = "macos"), feature = "cuda", feature = "rocm")),
    allow(unused_variables)
)]
pub fn stapel(
    xs: &[&[i16]],
    w: Gewichtsmatrix<'_>,
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    ziel: &(dyn Fn(usize) -> u8 + Sync),
) -> Option<Vec<Vec<i16>>> {
    // ⚑ Metal hat fuer beide Gewichtsarten einen Weg (ternaer seit dem
    //   2026-09-30).
    #[cfg(all(feature = "metal", target_os = "macos"))]
    {
        let gpu = match w {
            Gewichtsmatrix::Int8(w8) => metal::stapel(xs, w8, in_features, w_shifts, act_frac_bits, ziel),
            Gewichtsmatrix::Ternaer(t) => {
                metal::stapel_ternaer(xs, t.muster(), t.betraege(), in_features, w_shifts, act_frac_bits, ziel)
            }
        };
        if gpu.is_some() {
            return gpu;
        }
    }
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    {
        let auf_karten = match w {
            Gewichtsmatrix::Int8(w8) => karten::stapel(xs, w8, in_features, w_shifts, act_frac_bits, ziel),
            Gewichtsmatrix::Ternaer(t) => {
                karten::stapel_ternaer(xs, t.muster(), t.betraege(), in_features, w_shifts, act_frac_bits, ziel)
            }
        };
        if auf_karten.is_some() {
            return auf_karten;
        }
    }
    None
}

/// **Viele Stapel auf einem Rechenwerk**, oder `None`: Dann rechnet die
/// CPU dieselbe Zahl.
#[cfg_attr(
    not(any(all(feature = "metal", target_os = "macos"), feature = "cuda", feature = "rocm")),
    allow(unused_variables)
)]
pub fn stapel_viele(auftraege: &[Stapelauftrag<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    // ⚑ **Metal nimmt in einem Buendel nur int8.** Ist ein Auftrag
    //   ternaer (die Experten eines ternaeren Gemischs), geht das Buendel an
    //   die Steckkarten oder die CPU: dieselbe ganze Zahl je Ausgabeelement.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    if auftraege.iter().all(|a| !a.w.ist_ternaer()) {
        let ziele = ausgangsziele(auftraege);
        let gpu: Vec<metal::Auftrag<'_>> = auftraege
            .iter()
            .zip(&ziele)
            .map(|(a, ziel)| metal::Auftrag {
                xs: a.xs,
                w: match a.w {
                    Gewichtsmatrix::Int8(w) => w,
                    Gewichtsmatrix::Ternaer(_) => unreachable!("eben ausgeschlossen"),
                },
                in_features: a.in_features,
                w_shifts: a.w_shifts,
                act_frac_bits: a.act_frac_bits,
                ziel: ziel.as_ref(),
            })
            .collect();
        if let Some(aus) = metal::stapel_viele(&gpu) {
            return Some(aus);
        }
    }
    // ⚑ **Die Steckkarten**, auch fuer ternaere Auftraege und auf mehreren
    //   Karten zugleich.
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    {
        let ziele = ausgangsziele(auftraege);
        let fuer_karten: Vec<karten::Auftrag<'_>> = auftraege
            .iter()
            .zip(&ziele)
            .map(|(a, ziel)| karten::Auftrag {
                xs: a.xs,
                w: match a.w {
                    Gewichtsmatrix::Int8(w) => karten::Gewichte::Int8(w),
                    Gewichtsmatrix::Ternaer(t) => karten::Gewichte::Ternaer { muster: t.muster(), betraege: t.betraege() },
                },
                in_features: a.in_features,
                w_shifts: a.w_shifts,
                act_frac_bits: a.act_frac_bits,
                ziel: ziel.as_ref(),
            })
            .collect();
        if let Some(aus) = karten::stapel_viele(&fuer_karten) {
            return Some(aus);
        }
    }
    None
}

/// Die Ausgangsskala je Zeile eines jeden Auftrags, fuer die Rechenwerke.
#[cfg(any(all(feature = "metal", target_os = "macos"), feature = "cuda", feature = "rocm"))]
fn ausgangsziele<'a>(auftraege: &'a [Stapelauftrag<'a>]) -> Vec<Box<dyn Fn(usize) -> u8 + Sync + 'a>> {
    use crate::linear::Ausgangsskala;
    auftraege
        .iter()
        .map(|a| -> Box<dyn Fn(usize) -> u8 + Sync + 'a> {
            match a.aus {
                Ausgangsskala::Eine(f) => Box::new(move |_| f),
                Ausgangsskala::JeZeile(f) => Box::new(move |z| f[z]),
            }
        })
        .collect()
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::linear::Ausgangsskala;

    /// ⚑ **Ohne bereites Rechenwerk gibt die Weiche `None`**, fuer beide
    /// Wege, und die CPU rechnet. Auf einer Maschine mit Metal oder Karte
    /// darf sie rechnen; dann muss es dieselbe Zahl sein wie auf der CPU.
    #[test]
    fn die_weiche_gibt_none_oder_dieselbe_zahl() {
        let w: Vec<i8> = (0..64 * 32).map(|i| (i % 251) as i8).collect();
        let eingaben: Vec<Vec<i16>> = (0..20).map(|b| (0..32).map(|k| (b * 97 + k * 31) as i16).collect()).collect();
        let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
        let w_shifts = vec![3u8; 64];
        let ziele = vec![7u8; 64];
        let cpu: Vec<Vec<i16>> = xs
            .iter()
            .map(|x| {
                (0..64)
                    .map(|z| {
                        let s = crate::dot::dot_i8_i16(&w[z * 32..(z + 1) * 32], x);
                        crate::fixed_point::clamp_i16_from_i64(crate::fixed_point::rescale_i64(s, 3 + 5, 7))
                    })
                    .collect()
            })
            .collect();
        if let Some(aus) = stapel(&xs, Gewichtsmatrix::Int8(&w), 32, &w_shifts, 5, &|z| ziele[z]) {
            assert_eq!(aus, cpu);
        }
        let auftrag = Stapelauftrag {
            xs: &xs,
            w: Gewichtsmatrix::Int8(&w),
            in_features: 32,
            w_shifts: &w_shifts,
            act_frac_bits: 5,
            aus: Ausgangsskala::JeZeile(&ziele),
        };
        if let Some(mut aus) = stapel_viele(std::slice::from_ref(&auftrag)) {
            assert_eq!(aus.pop().expect("ein Auftrag"), cpu);
        }
    }
}
