//! **Messung: wie schnell rechnet die Zustandsschicht?**
//!
//! Ein Fenster von Token durch die Koepfe einer Ebene, wie im Prefill
//! (`schritte_fenster`), mit den Massen des int8-35B (32 Koepfe zu
//! 128 x 128). Die Eingaben sind ein fester Wurf, der Zerfall knapp unter
//! eins, sodass der Zustand in die Hoehe waechst, die er im Modell hat.
//! Gemessen wird erst nach einem Anlauf; ausgegeben werden Mikrosekunden je
//! Token und eine Pruefsumme der Ausgaben, die vor und nach einer Aenderung
//! am Kern dieselbe sein muss.
//!
//! Aufruf: `zustandsmessung [faeden] [fenster] [runden]`

use integer_llm_kernels::zustandsschicht::{schritte_fenster, Zustand, ZERFALL_FRAC};
use std::time::Instant;

/// Eingabe eines Kopfes fuer ein Token: `q`, `k`, `v`, Zerfall, Schreibstaerke.
type Schritteingabe = (Vec<i16>, Vec<i16>, Vec<i16>, i64, i16);

struct Wurf(u64);

impl Wurf {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn feld(&mut self, n: usize, m: i64) -> Vec<i16> {
        (0..n).map(|_| ((self.naechste() % (2 * m as u64 + 1)) as i64 - m) as i16).collect()
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let faeden: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(integer_llm_kernels::fadenpool::faeden());
    let fenster: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(64);
    let runden: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
    let (koepfe, sd, wd) = (32usize, 128usize, 128usize);
    let mut w = Wurf(0x9E37_79B9_7F4A_7C15);
    let eingaben: Vec<Vec<Schritteingabe>> = (0..koepfe)
        .map(|_| {
            (0..fenster)
                .map(|_| {
                    let g = (1i64 << ZERFALL_FRAC) - 1 - (w.naechste() % (1 << 22)) as i64;
                    (w.feld(sd, 4096), w.feld(sd, 4096), w.feld(wd, 600), g, (w.naechste() % 256) as i16)
                })
                .collect()
        })
        .collect();
    let mut zustaende: Vec<Zustand> = (0..koepfe).map(|_| Zustand::leer(sd, wd)).collect();
    let lauf = |zustaende: &mut [Zustand]| {
        schritte_fenster(
            zustaende,
            fenster,
            |h, t| {
                let (q, k, v, g, b) = &eingaben[h][t];
                (q.as_slice(), k.as_slice(), v.as_slice(), *g, *b)
            },
            faeden,
        )
    };
    // Anlauf, bis der Zustand seine Hoehe hat.
    for _ in 0..4 {
        lauf(&mut zustaende);
    }
    let mut summe = 0u64;
    let t0 = Instant::now();
    for _ in 0..runden {
        let (aus, fracs) = lauf(&mut zustaende);
        for (zeile, f) in aus.iter().zip(&fracs) {
            for (i, &x) in zeile.iter().enumerate() {
                summe = summe.wrapping_mul(31).wrapping_add((x as i64 as u64) ^ i as u64);
            }
            for &x in f {
                summe = summe.wrapping_mul(7).wrapping_add(u64::from(x));
            }
        }
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / (runden * fenster) as f64;
    let hoehe = zustaende.iter().flat_map(|z| (0..sd).flat_map(move |i| (0..wd).map(move |j| z.get(i, j).unsigned_abs()))).max().unwrap_or(0);
    println!("faeden {faeden} fenster {fenster} runden {runden}: {us:.1} us je Token, Pruefsumme {summe:016x}, Hoehe 2^{}", 64 - hoehe.leading_zeros());
}
