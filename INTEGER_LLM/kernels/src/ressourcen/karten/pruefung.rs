//! **Die Faelle, an denen eine Karte beweist, dass sie rechnet wie die
//! CPU.** Dieselben Faelle prueft der Nachbau (`nachbau.rs`); so sieht die
//! Pruefung ohne Karte genau das, was die Karte spaeter bestehen muss.
//!
//! Geprueft wird, was eine unauffaellige Messung nicht trifft:
//!
//! - **Die groessten Betraege**: Gewichte −128 und 127, Eingaben −32 768
//!   und 32 767, 9 728 Spalten. Die Summen erreichen 4·10¹⁰ und liegen weit
//!   ueber `2³¹`; ein Kern, der in int32 summierte, fiele hier.
//! - **Kachelraender** in Zeilen, Eingaben und Spalten: nichts davon ein
//!   Vielfaches von 64 oder 16.
//! - **Alle drei Faelle der Umskalierung**: Rechtsshift, kein Shift,
//!   Linksshift.
//! - **Ternaer**: Code 3 (+2), der groesste Betrag, mehrere Gruppen.
//! - **Vier Bit**: Code 15, Schritt 255 und Versatz −128 (Werte weit
//!   ausserhalb von int8, im Format erlaubt), Schritt 0, mehrere Gruppen.
//! - **Mehrere Auftraege zugleich und geteilte Matrizen**: Ein falscher
//!   Versatz oder eine falsche Zeilengrenze faellt nur dort auf.

use crate::linear::Gewichtsmatrix;

/// Die Gewichte eines Falls.
pub(super) enum Daten {
    Int8(Vec<i8>),
    Ternaer { muster: Vec<u8>, betraege: Vec<i16> },
    Vierbit { codes: Vec<u8>, gitter: Vec<u8> },
}

/// Ein Pruefauftrag.
pub(super) struct Fall {
    pub name: String,
    pub zeilen: usize,
    pub spalten: usize,
    pub daten: Daten,
    pub eingaben: Vec<Vec<i16>>,
    pub w_shifts: Vec<u8>,
    pub ziele: Vec<u8>,
    pub act: u8,
}

impl Fall {
    pub fn gewichte(&self) -> super::Gewichte<'_> {
        match &self.daten {
            Daten::Int8(w) => super::Gewichte::Int8(w),
            Daten::Ternaer { muster, betraege } => super::Gewichte::Ternaer { muster, betraege },
            Daten::Vierbit { codes, gitter } => super::Gewichte::Vierbit { codes, gitter },
        }
    }

    pub fn abstaende(&self) -> Vec<i8> {
        crate::ressourcen::metal::abstaende_fuer(&self.w_shifts, self.act, &|z| self.ziele[z])
    }

    /// **Was die CPU rechnet**, ueber denselben Weg wie ohne Karte.
    pub fn erwartet(&self) -> Result<Vec<Vec<i16>>, String> {
        let xs: Vec<&[i16]> = self.eingaben.iter().map(|e| e.as_slice()).collect();
        let w: Gewichtsmatrix<'_> = match &self.daten {
            Daten::Int8(w) => Gewichtsmatrix::Int8(w),
            Daten::Ternaer { muster, betraege } => {
                crate::ternaer::Ternaermatrix::neu(muster, betraege, self.zeilen, self.spalten)?.into()
            }
            Daten::Vierbit { codes, gitter } => {
                crate::vierbit::Vierbitmatrix::neu(codes, gitter, self.zeilen, self.spalten)?.into()
            }
        };
        Ok(crate::linear::linear_matrix_pc_stapel_cpu(&xs, w, self.spalten, &self.w_shifts, self.act, &self.ziele))
    }
}

/// Ein einfacher, fester Zufall: dieselben Faelle in jedem Prozess.
struct Zufall(u64);

impl Zufall {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// **Alle Faelle**, in fester Reihenfolge.
pub(super) fn faelle() -> Vec<Fall> {
    let mut z = Zufall(0x9e37_79b9_7f4a_7c15);
    let mut f = Vec::new();
    // Abstaende 20, 14, 0, -1 der Reihe nach.
    let shifts_int8 = |zeilen: usize| -> (Vec<u8>, Vec<u8>) {
        ((0..zeilen).map(|r| [9u8, 3, 0, 0][r % 4]).collect(), (0..zeilen).map(|r| [0u8, 0, 11, 12][r % 4]).collect())
    };
    for (name, zeilen, spalten, stapel) in
        [("Extremwerte", 67usize, 9_728usize, 17usize), ("Zufall", 130, 1_003, 33), ("klein", 3, 5, 2), ("eine Zelle", 1, 1, 1)]
    {
        let extrem = name == "Extremwerte";
        let w: Vec<i8> = (0..zeilen * spalten)
            .map(|i| if extrem { if (i / spalten) % 2 == 0 { -128 } else { 127 } } else { z.naechste() as i8 })
            .collect();
        let eingaben: Vec<Vec<i16>> = (0..stapel)
            .map(|b| {
                (0..spalten)
                    .map(|_| if extrem { if b % 2 == 0 { i16::MIN } else { i16::MAX } } else { z.naechste() as i16 })
                    .collect()
            })
            .collect();
        let (w_shifts, ziele) = shifts_int8(zeilen);
        f.push(Fall { name: name.into(), zeilen, spalten, daten: Daten::Int8(w), eingaben, w_shifts, ziele, act: 11 });
    }
    for (nummer, (zeilen, spalten, stapel)) in
        [(67usize, 1_280usize, 17usize), (130, 384, 33), (3, 128, 2)].into_iter().enumerate()
    {
        let gruppen = spalten / crate::ternaer::GRUPPE;
        let muster: Vec<u8> = (0..zeilen * spalten / 4)
            .map(|i| {
                let r = z.naechste();
                (0..4).fold(0u8, |b, s| {
                    let code = if nummer == 0 && i % 97 == 0 { 3 } else { ((r >> (8 * s)) % 3) as u8 };
                    b | (code << (2 * s))
                })
            })
            .collect();
        let betraege: Vec<i16> = (0..zeilen * gruppen)
            .map(|i| if nummer == 0 && i % 5 == 0 { i16::MAX } else { (z.naechste() % 16_384) as i16 })
            .collect();
        let eingaben: Vec<Vec<i16>> = (0..stapel)
            .map(|b| {
                (0..spalten)
                    .map(|i| match (nummer, (b + i) % 7) {
                        (0, 0) => i16::MIN,
                        (0, 1) => i16::MAX,
                        _ => z.naechste() as i16,
                    })
                    .collect()
            })
            .collect();
        let w_shifts: Vec<u8> = (0..zeilen).map(|r| [20u8, 14, 9, 9][r % 4]).collect();
        let ziele: Vec<u8> = (0..zeilen).map(|r| [0u8, 3, 20, 21][r % 4]).collect();
        f.push(Fall {
            name: format!("ternaer {zeilen} x {spalten}"),
            zeilen,
            spalten,
            daten: Daten::Ternaer { muster, betraege },
            eingaben,
            w_shifts,
            ziele,
            act: 11,
        });
    }
    for (nummer, (zeilen, spalten, stapel)) in
        [(67usize, 1_280usize, 17usize), (130, 384, 33), (3, 128, 2)].into_iter().enumerate()
    {
        let gruppen = spalten / crate::vierbit::GRUPPE;
        let codes: Vec<u8> = (0..zeilen * spalten / 2)
            .map(|i| if nummer == 0 && i % 89 == 0 { 0xFF } else { z.naechste() as u8 })
            .collect();
        // Im ersten Fall jede siebte Gruppe mit Schritt 255 und Versatz
        // -128, jede elfte mit Schritt 0.
        let gitter: Vec<u8> = (0..zeilen * gruppen)
            .flat_map(|g| match (nummer, g % 7, g % 11) {
                (0, 0, _) => [255u8, 0x80],
                (0, _, 0) => [0u8, z.naechste() as u8],
                _ => {
                    let r = z.naechste();
                    [r as u8, (r >> 8) as u8]
                }
            })
            .collect();
        let eingaben: Vec<Vec<i16>> = (0..stapel)
            .map(|b| {
                (0..spalten)
                    .map(|i| match (nummer, (b + i) % 7) {
                        (0, 0) => i16::MIN,
                        (0, 1) => i16::MAX,
                        _ => z.naechste() as i16,
                    })
                    .collect()
            })
            .collect();
        let w_shifts: Vec<u8> = (0..zeilen).map(|r| [20u8, 14, 9, 9][r % 4]).collect();
        let ziele: Vec<u8> = (0..zeilen).map(|r| [0u8, 3, 20, 21][r % 4]).collect();
        f.push(Fall {
            name: format!("vierbit {zeilen} x {spalten}"),
            zeilen,
            spalten,
            daten: Daten::Vierbit { codes, gitter },
            eingaben,
            w_shifts,
            ziele,
            act: 11,
        });
    }
    f
}

/// Die erste Abweichung, fuer die Meldung.
pub(super) fn abweichung(name: &str, karte: &[Vec<i16>], cpu: &[Vec<i16>]) -> Option<String> {
    if karte.len() != cpu.len() {
        return Some(format!("{name}: {} statt {} Eingaben", karte.len(), cpu.len()));
    }
    karte.iter().zip(cpu).enumerate().find_map(|(b, (k, c))| {
        if k.len() != c.len() {
            return Some(format!("{name}: Eingabe {b} hat {} statt {} Zeilen", k.len(), c.len()));
        }
        k.iter()
            .zip(c)
            .position(|(k, c)| k != c)
            .map(|z| format!("{name}: zuerst Eingabe {b}, Zeile {z}: Karte {} gegen CPU {}", k[z], c[z]))
    })
}

/// **Die Selbstpruefung einer Karte.** `Ok` heisst: Sie rechnet in allen
/// Faellen Bit fuer Bit wie die CPU.
pub(super) fn selbstpruefung(k: &super::treiber::Karte) -> Result<(), String> {
    use super::verteiler::Teil;
    let faelle = faelle();
    let scheiben: Vec<Vec<&[i16]>> = faelle.iter().map(|f| f.eingaben.iter().map(|e| e.as_slice()).collect()).collect();
    let jobs: Vec<super::Job<'_>> = faelle
        .iter()
        .zip(&scheiben)
        .map(|(f, xs)| super::Job { xs, w: f.gewichte(), spalten: f.spalten, abstaende: f.abstaende() })
        .collect();
    let erwartet: Vec<Vec<Vec<i16>>> = faelle.iter().map(Fall::erwartet).collect::<Result<_, _>>()?;
    if !erwartet[0].iter().flatten().any(|&v| v != 0 && v != i16::MAX && v != i16::MIN)
        && !erwartet[1].iter().flatten().any(|&v| v != 0 && v != i16::MAX && v != i16::MIN)
    {
        return Err("die Pruefung rechnet nur Nullen oder Saettigung".into());
    }

    // 1. Jeder Fall fuer sich.
    for (i, f) in faelle.iter().enumerate() {
        let teil = [Teil { auftrag: i, von: 0, bis: f.zeilen }];
        let mut aus = super::auf_karte(k, &teil, &jobs)?;
        let karte = entflechten(aus.pop().unwrap_or_default(), f.zeilen);
        if let Some(a) = abweichung(&f.name, &karte, &erwartet[i]) {
            return Err(a);
        }
    }

    // 2. Alle zugleich, die groesseren geteilt: jeder Fall mit mehr als
    //    einer Kachel in zwei Teilen mit einer Grenze, die keine Kachel
    //    trifft. Damit laufen Versaetze, Teilgewichte und das Zusammensetzen.
    let mut teile = Vec::new();
    for (i, f) in faelle.iter().enumerate() {
        if f.zeilen > 64 {
            let mitte = f.zeilen / 2 + 3;
            teile.push(Teil { auftrag: i, von: 0, bis: mitte });
            teile.push(Teil { auftrag: i, von: mitte, bis: f.zeilen });
        } else {
            teile.push(Teil { auftrag: i, von: 0, bis: f.zeilen });
        }
    }
    let aus = super::auf_karte(k, &teile, &jobs)?;
    let mut zusammen: Vec<Vec<Vec<i16>>> = faelle.iter().map(|f| vec![vec![0i16; f.zeilen]; f.eingaben.len()]).collect();
    for (t, werte) in teile.iter().zip(aus) {
        let n = t.bis - t.von;
        for (b, zeile) in zusammen[t.auftrag].iter_mut().enumerate() {
            zeile[t.von..t.bis].copy_from_slice(&werte[b * n..(b + 1) * n]);
        }
    }
    for (i, f) in faelle.iter().enumerate() {
        if let Some(a) = abweichung(&format!("{} im Buendel", f.name), &zusammen[i], &erwartet[i]) {
            return Err(a);
        }
    }
    Ok(())
}

/// Macht aus `stapel * zeilen` Werten eine Zeile je Eingabe.
fn entflechten(flach: Vec<i16>, zeilen: usize) -> Vec<Vec<i16>> {
    if zeilen == 0 {
        return Vec::new();
    }
    flach.chunks_exact(zeilen).map(<[i16]>::to_vec).collect()
}
