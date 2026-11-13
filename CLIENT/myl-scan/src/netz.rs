//! **Kleine Zeilennetze fuer Schriften, die sonst fehlen**: gefaltet, dann
//! rekurrent, Gewichte zu je einem Byte. Eine Schrift wiegt so rund ein Megabyte.
//!
//! Das Format beschreibt `modelle/netz_packen.py`. Gelesen wird gierig und ohne
//! Woerterbuch, damit Modelle klein bleiben und nichts dazuerfunden wird.

use image::GrayImage;
use std::collections::HashMap;

const NULL_NAME: &str = "NULL";
const ZUSTAND_GRENZE: f32 = 100.0;
const LAENGSTER_CODE: usize = 9;
const LERNRATEN_JE_SCHICHT: i32 = 64;
const BYTE_GEWICHTE: u8 = 1;
const DOPPELTE_SKALEN: u8 = 128;

struct Bytes<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Bytes<'a> {
    fn nimm(&mut self, n: usize) -> Result<&'a [u8], String> {
        let e = self.i.checked_add(n).filter(|e| *e <= self.b.len()).ok_or("das Netz endet zu frueh")?;
        let t = &self.b[self.i..e];
        self.i = e;
        Ok(t)
    }
    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.nimm(1)?[0] as i8)
    }
    fn i32(&mut self) -> Result<i32, String> {
        let t = self.nimm(4)?;
        Ok(i32::from_le_bytes([t[0], t[1], t[2], t[3]]))
    }
    fn u32(&mut self) -> Result<usize, String> {
        Ok(self.i32()? as u32 as usize)
    }
    fn f64(&mut self) -> Result<f64, String> {
        let t = self.nimm(8)?;
        Ok(f64::from_le_bytes([t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7]]))
    }
    fn text(&mut self) -> Result<String, String> {
        let n = self.u32()?;
        Ok(String::from_utf8_lossy(self.nimm(n)?).into_owned())
    }
}

/// Eine Matrix aus Bytes, je Zeile eine Skala; die letzte Spalte ist der Versatz.
struct Matrix {
    w: Vec<i8>,
    zeilen: usize,
    spalten: usize,
    skala: Vec<f32>,
}

impl Matrix {
    fn lesen(r: &mut Bytes) -> Result<Self, String> {
        let art = r.i8()? as u8;
        if art & BYTE_GEWICHTE == 0 || art & DOPPELTE_SKALEN == 0 {
            return Err("nur Netze mit Byte-Gewichten".into());
        }
        let (zeilen, spalten) = (r.u32()?, r.u32()?);
        if zeilen == 0 || spalten == 0 {
            return Err("leere Gewichte".into());
        }
        r.i8()?;
        let w = r.nimm(zeilen.checked_mul(spalten).ok_or("zu grosse Gewichte")?)?.iter().map(|b| *b as i8).collect();
        let n = r.u32()?;
        if n < zeilen {
            return Err("zu wenige Skalen".into());
        }
        let mut skala = Vec::with_capacity(n);
        for _ in 0..n {
            skala.push((r.f64()? / 127.0) as f32);
        }
        Ok(Self { w, zeilen, spalten, skala })
    }

    fn mal(&self, ein: &[i8], aus: &mut [f32]) {
        let n = self.spalten - 1;
        for (i, (z, a)) in self.w.chunks_exact(self.spalten).zip(aus.iter_mut()).enumerate() {
            let s: i32 = z[..n].iter().zip(&ein[..n]).map(|(w, e)| i32::from(*w) * i32::from(*e)).sum();
            *a = (s + i32::from(z[n]) * 127) as f32 * self.skala[i];
        }
    }
}

/// Ein Gitter von Aktivierungen; `t = y * b + x`.
#[derive(Clone)]
struct Feld {
    b: usize,
    h: usize,
    d: usize,
    v: Vec<i8>,
}

impl Feld {
    fn neu(b: usize, h: usize, d: usize) -> Self {
        Self { b, h, d, v: vec![0; b * h * d] }
    }
    fn stelle(&self, x: usize, y: usize) -> &[i8] {
        let s = (y * self.b + x) * self.d;
        &self.v[s..s + self.d]
    }
    fn stelle_mut(&mut self, x: usize, y: usize) -> &mut [i8] {
        let s = (y * self.b + x) * self.d;
        &mut self.v[s..s + self.d]
    }
}

fn zu_byte(x: f32) -> i8 {
    (x * 127.0).round().clamp(-127.0, 127.0) as i8
}

#[derive(Clone, Copy, PartialEq)]
enum Wirkung {
    Tanh,
    Softmax,
}

enum Schicht {
    Eingabe,
    Reihe(Vec<Schicht>),
    Faltung { hx: isize, hy: isize },
    Maximum { x: usize, y: usize },
    Voll(Matrix, Wirkung),
    Lstm { gates: Box<[Matrix; 4]>, zustand: usize, gesamt: usize, ein: usize, zusammenfassen: bool },
    Quer(Box<Schicht>),
    Rueckwaerts(Box<Schicht>),
}

fn schicht_lesen(r: &mut Bytes) -> Result<(Schicht, usize), String> {
    if r.i8()? != 0 {
        return Err("altes Netzformat".into());
    }
    let art = r.text()?;
    r.i8()?;
    r.i8()?;
    let merkmale = r.i32()?;
    let ein = r.i32()? as usize;
    let _aus = r.i32()?;
    let _gewichte = r.i32()?;
    let _name = r.text()?;
    let kinder = |r: &mut Bytes| -> Result<Vec<Schicht>, String> {
        let n = r.u32()?;
        if n > 1000 {
            return Err("zu viele Schichten".into());
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(schicht_lesen(r)?.0);
        }
        if merkmale & LERNRATEN_JE_SCHICHT != 0 {
            let m = r.u32()?;
            r.nimm(m.checked_mul(4).ok_or("Lernraten")?)?;
        }
        Ok(v)
    };
    let eins = |mut v: Vec<Schicht>| v.pop().map(Box::new).ok_or_else(|| "Schicht ohne Kind".to_string());
    let s = match art.as_str() {
        "Input" => {
            r.nimm(20)?;
            Schicht::Eingabe
        }
        "Series" => Schicht::Reihe(kinder(r)?),
        "XYTranspose" => Schicht::Quer(eins(kinder(r)?)?),
        "RTLReversed" => Schicht::Rueckwaerts(eins(kinder(r)?)?),
        "Convolve" => {
            let (hx, hy) = (r.i32()? as isize, r.i32()? as isize);
            Schicht::Faltung { hx, hy }
        }
        "Maxpool" => {
            let (x, y) = (r.i32()? as usize, r.i32()? as usize);
            if x == 0 || y == 0 {
                return Err("Maxpool ohne Mass".into());
            }
            Schicht::Maximum { x, y }
        }
        "Tanh" => Schicht::Voll(Matrix::lesen(r)?, Wirkung::Tanh),
        "Softmax" => Schicht::Voll(Matrix::lesen(r)?, Wirkung::Softmax),
        "LSTM" | "SummLSTM" => {
            let gesamt = r.i32()? as usize;
            let g = [Matrix::lesen(r)?, Matrix::lesen(r)?, Matrix::lesen(r)?, Matrix::lesen(r)?];
            let zustand = g[0].zeilen;
            if gesamt != ein + zustand {
                return Err("nur eindimensionale LSTM".into());
            }
            if g.iter().any(|m| m.spalten != gesamt + 1 || m.zeilen != zustand) {
                return Err("LSTM mit unpassenden Gewichten".into());
            }
            Schicht::Lstm { gates: Box::new(g), zustand, gesamt, ein, zusammenfassen: art == "SummLSTM" }
        }
        a => return Err(format!("unbekannte Schicht {a}")),
    };
    Ok((s, ein))
}

/// Was das Netz am Ende sagt: je Zeitschritt alle Klassen.
struct Ausgabe {
    v: Vec<f32>,
    klassen: usize,
}

fn lstm(e: &Feld, g: &[Matrix; 4], ns: usize, na: usize, ni: usize, zus: bool) -> Feld {
    let mut aus = if zus { Feld::neu(1, e.h, ns) } else { Feld::neu(e.b, e.h, ns) };
    let mut ein = vec![0i8; na];
    let (mut ci, mut gi, mut gf, mut go) = (vec![0f32; ns], vec![0f32; ns], vec![0f32; ns], vec![0f32; ns]);
    for y in 0..e.h {
        let (mut zu, mut au) = (vec![0f32; ns], vec![0f32; ns]);
        for x in 0..e.b {
            ein[..ni].copy_from_slice(&e.stelle(x, y)[..ni]);
            for (s, a) in ein[ni..].iter_mut().zip(&au) {
                *s = zu_byte(*a);
            }
            g[0].mal(&ein, &mut ci);
            g[1].mal(&ein, &mut gi);
            g[2].mal(&ein, &mut gf);
            g[3].mal(&ein, &mut go);
            for i in 0..ns {
                let (fg, ig) = (logistik(gf[i]), logistik(gi[i]));
                zu[i] = (zu[i] * fg + ci[i].tanh() * ig).clamp(-ZUSTAND_GRENZE, ZUSTAND_GRENZE);
                au[i] = zu[i].tanh() * logistik(go[i]);
            }
            if !zus {
                for (s, a) in aus.stelle_mut(x, y).iter_mut().zip(&au) {
                    *s = zu_byte(*a);
                }
            } else if x + 1 == e.b {
                for (s, a) in aus.stelle_mut(0, y).iter_mut().zip(&au) {
                    *s = zu_byte(*a);
                }
            }
        }
    }
    aus
}

fn logistik(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn quer(e: &Feld) -> Feld {
    let mut a = Feld::neu(e.h, e.b, e.d);
    for y in 0..e.h {
        for x in 0..e.b {
            a.stelle_mut(y, x).copy_from_slice(e.stelle(x, y));
        }
    }
    a
}

fn gespiegelt(e: &Feld) -> Feld {
    let mut a = Feld::neu(e.b, e.h, e.d);
    for y in 0..e.h {
        for x in 0..e.b {
            a.stelle_mut(e.b - 1 - x, y).copy_from_slice(e.stelle(x, y));
        }
    }
    a
}

fn vorwaerts(s: &Schicht, e: Feld, aus: &mut Option<Ausgabe>) -> Feld {
    match s {
        Schicht::Eingabe => e,
        Schicht::Reihe(k) => k.iter().fold(e, |f, s| vorwaerts(s, f, aus)),
        Schicht::Quer(k) => quer(&vorwaerts(k, quer(&e), aus)),
        Schicht::Rueckwaerts(k) => gespiegelt(&vorwaerts(k, gespiegelt(&e), aus)),
        Schicht::Faltung { hx, hy } => {
            let d = e.d * (2 * *hx as usize + 1) * (2 * *hy as usize + 1);
            let mut a = Feld::neu(e.b, e.h, d);
            for y in 0..e.h {
                for x in 0..e.b {
                    let mut o = 0;
                    for dx in -hx..=*hx {
                        for dy in -hy..=*hy {
                            let (nx, ny) = (x as isize + dx, y as isize + dy);
                            if nx >= 0 && ny >= 0 && (nx as usize) < e.b && (ny as usize) < e.h {
                                let q = e.stelle(nx as usize, ny as usize);
                                a.stelle_mut(x, y)[o..o + e.d].copy_from_slice(q);
                            }
                            o += e.d;
                        }
                    }
                }
            }
            a
        }
        Schicht::Maximum { x: sx, y: sy } => {
            let (nb, nh) = ((e.b / sx).max(1), (e.h / sy).max(1));
            let mut a = Feld::neu(nb, nh, e.d);
            for y in 0..nh {
                for x in 0..nb {
                    let z = a.stelle_mut(x, y);
                    z.fill(i8::MIN);
                    for qy in y * sy..((y + 1) * sy).min(e.h) {
                        for qx in x * sx..((x + 1) * sx).min(e.b) {
                            for (m, v) in z.iter_mut().zip(e.stelle(qx, qy)) {
                                *m = (*m).max(*v);
                            }
                        }
                    }
                }
            }
            a
        }
        Schicht::Voll(m, w) => {
            let mut ergebnis = vec![0f32; m.zeilen];
            let schritte = e.b * e.h;
            if *w == Wirkung::Softmax {
                let mut alle = Vec::with_capacity(schritte * m.zeilen);
                for t in e.v.chunks_exact(e.d) {
                    m.mal(t, &mut ergebnis);
                    let g = ergebnis.iter().fold(f32::MIN, |a, b| a.max(*b));
                    let summe: f32 = ergebnis.iter().map(|x| (x - g).exp()).sum();
                    alle.extend(ergebnis.iter().map(|x| (x - g).exp() / summe));
                }
                *aus = Some(Ausgabe { v: alle, klassen: m.zeilen });
                return e;
            }
            let mut a = Feld::neu(e.b, e.h, m.zeilen);
            for (t, z) in e.v.chunks_exact(e.d).zip(a.v.chunks_exact_mut(m.zeilen)) {
                m.mal(t, &mut ergebnis);
                for (s, v) in z.iter_mut().zip(&ergebnis) {
                    *s = zu_byte(v.tanh());
                }
            }
            a
        }
        Schicht::Lstm { gates, zustand, gesamt, ein, zusammenfassen } => lstm(&e, gates, *zustand, *gesamt, *ein, *zusammenfassen),
    }
}

/// Ein geladenes Zeilennetz samt seiner Zeichentabelle.
pub struct Netz {
    wurzel: Schicht,
    pub(crate) hoehe: usize,
    blank: usize,
    klassen: usize,
    codes: HashMap<Vec<i32>, usize>,
    zeichen: Vec<String>,
}

impl Netz {
    /// Liest die drei Teile, wie `netz_packen.py` sie ablegt.
    pub fn neu(daten: &[u8]) -> Result<Self, String> {
        let mut r = Bytes { b: daten, i: 0 };
        let n = r.u32()?;
        let mut netz = Bytes { b: r.nimm(n)?, i: 0 };
        let n = r.u32()?;
        let mut rec = Bytes { b: r.nimm(n)?, i: 0 };
        let n = r.u32()?;
        let satz = String::from_utf8_lossy(r.nimm(n)?).into_owned();

        let (wurzel, hoehe) = schicht_lesen(&mut netz)?;
        netz.text()?;
        netz.nimm(12)?;
        let blank = netz.i32()? as usize;

        let mut codes = HashMap::new();
        let zahl = rec.u32()?;
        let mut klassen = 0usize;
        for u in 0..zahl {
            rec.i8()?;
            let l = rec.i32()? as usize;
            if l > LAENGSTER_CODE {
                return Err("zu langer Code".into());
            }
            let mut c = Vec::with_capacity(l);
            for _ in 0..l {
                let v = rec.i32()?;
                klassen = klassen.max(v as usize + 1);
                c.push(v);
            }
            codes.entry(c).or_insert(u);
        }
        let zeichen: Vec<String> = satz
            .lines()
            .skip(1)
            .map(|z| {
                let t = z.split(' ').next().unwrap_or_default();
                if t == NULL_NAME { " ".to_string() } else if t == "Joined" || t.starts_with('|') { String::new() } else { t.to_string() }
            })
            .collect();
        if blank > klassen {
            return Err("Leerzeichen ausserhalb der Klassen".into());
        }
        Ok(Self { wurzel, hoehe, blank, klassen: klassen.max(blank + 1), codes, zeichen })
    }

    /// Liest eine Zeile in Hoehe [`Netz::hoehe`]; liefert die Zeichen in Bildfolge.
    pub(crate) fn lesen(&self, bild: &GrayImage) -> Result<(Vec<String>, f32), String> {
        let (b, h) = (bild.width() as usize, bild.height() as usize);
        let (schwarz, weiss) = schwarz_weiss(bild);
        let kontrast = ((weiss - schwarz) / 2.0).max(1.0);
        let mut e = Feld::neu(b, h, 1);
        for (v, p) in e.v.iter_mut().zip(bild.as_raw()) {
            let f = (f32::from(*p) - schwarz) / kontrast - 1.0;
            *v = (128.0 * f).round().clamp(-127.0, 127.0) as i8;
        }
        let mut aus = None;
        vorwaerts(&self.wurzel, e, &mut aus);
        let a = aus.ok_or("das Netz hat keine Ausgabe")?;
        if a.klassen < self.klassen.min(self.blank + 1) {
            return Err("zu wenige Klassen".into());
        }
        let mut folge = Vec::new();
        let (mut vorher, mut summe, mut zahl) = (self.blank, 0.0f32, 0usize);
        for t in a.v.chunks_exact(a.klassen) {
            let beste = t.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map_or(self.blank, |(i, _)| i);
            if beste != self.blank && beste != vorher {
                folge.push(beste as i32);
                summe += t[beste];
                zahl += 1;
            }
            vorher = beste;
        }
        Ok((self.entschluesseln(&folge), if zahl == 0 { 0.0 } else { summe / zahl as f32 }))
    }

    /// Mehrere Codes koennen ein Zeichen bilden: der laengste passende gewinnt.
    fn entschluesseln(&self, folge: &[i32]) -> Vec<String> {
        let mut aus = Vec::new();
        let mut i = 0;
        while i < folge.len() {
            let treffer = (1..=LAENGSTER_CODE.min(folge.len() - i)).rev().find_map(|l| self.codes.get(&folge[i..i + l]).map(|u| (l, *u)));
            match treffer {
                Some((l, u)) => {
                    if let Some(t) = self.zeichen.get(u) {
                        aus.push(t.clone());
                    }
                    i += l;
                }
                None => i += 1,
            }
        }
        aus
    }
}

/// Tinte und Papier aus den oertlichen Extremen der mittleren Bildzeile.
fn schwarz_weiss(bild: &GrayImage) -> (f32, f32) {
    let (b, h) = (bild.width(), bild.height());
    let (mut tief, mut hoch) = (Vec::new(), Vec::new());
    if b >= 3 {
        let y = h / 2;
        let w = |x: u32| i32::from(bild.get_pixel(x, y)[0]);
        for x in 1..b - 1 {
            let (p, c, n) = (w(x - 1), w(x), w(x + 1));
            if (c < p && c <= n) || (c <= p && c < n) {
                tief.push(c);
            }
            if (c > p && c >= n) || (c >= p && c > n) {
                hoch.push(c);
            }
        }
    }
    tief.sort_unstable();
    hoch.sort_unstable();
    let s = tief.get(tief.len() / 4).copied().unwrap_or(0) as f32;
    let w = if hoch.is_empty() { 255.0 } else { hoch[(hoch.len() * 3 / 4).min(hoch.len() - 1)] as f32 };
    (s, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_matrix_rechnet_mit_versatz_und_skala() {
        let m = Matrix { w: vec![2, -1, 1], zeilen: 1, spalten: 3, skala: vec![0.5] };
        let mut a = [0.0];
        m.mal(&[10, 4], &mut a);
        assert_eq!(a[0], (20.0 - 4.0 + 127.0) * 0.5);
    }

    #[test]
    fn quer_und_spiegeln_sind_umkehrbar() {
        let mut f = Feld::neu(3, 2, 1);
        f.v = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(quer(&quer(&f)).v, f.v);
        assert_eq!(quer(&f).v, vec![1, 4, 2, 5, 3, 6]);
        assert_eq!(gespiegelt(&f).v, vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn ein_kaputtes_netz_ist_ein_fehler_und_kein_absturz() {
        assert!(Netz::neu(&[1, 2, 3]).is_err());
        assert!(Netz::neu(&[255, 255, 255, 127, 0, 0]).is_err());
    }

    #[test]
    fn papier_ist_hell_und_tinte_dunkel() {
        let mut b = GrayImage::from_pixel(40, 5, image::Luma([220]));
        for x in (2..38).step_by(4) {
            b.put_pixel(x, 2, image::Luma([20]));
        }
        assert_eq!(schwarz_weiss(&b), (20.0, 220.0));
    }
}
