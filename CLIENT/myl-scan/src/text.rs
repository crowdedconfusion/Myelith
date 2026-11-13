//! Textzeilen finden und lesen.

use crate::{Erkenner, Leser};
use image::imageops::{resize, FilterType};
use image::RgbImage;
use std::sync::atomic::{AtomicUsize, Ordering};
use tract_onnx::prelude::*;

/// Laengste Seite beim Suchen; mehr brachte in der Messung nichts.
const FINDEN_KANTE: u32 = 1600;
const FINDEN_RASTER: u32 = 32;
const SCHWELLE: f32 = 0.3;
const MINDESTGUETE: f32 = 0.5;
const ERWEITERN: f32 = 1.6;
const ZEILENHOEHE: u32 = 48;
const BREITENRASTER: usize = 160;
const GROESSTE_BREITE: usize = 3200;
/// Ab diesem Seitenverhaeltnis ist eine Zeile eine senkrechte Spalte.
const HOCHFORMAT: f32 = 1.5;
const MITTEL: [f32; 3] = [0.485, 0.456, 0.406];
const STREUUNG: [f32; 3] = [0.229, 0.224, 0.225];

/// Eine Zeile, in Bildpunkten der Seite (um die Schraeglage gedreht gedacht).
#[derive(Debug, Clone, Default)]
pub struct Zeile {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub winkel: f32,
    pub text: String,
    /// Wie sicher das Modell beim Lesen war, 0 bis 1.
    pub guete: f32,
}

impl Zeile {
    pub(crate) fn mitte(&self) -> (f32, f32) {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }
    pub(crate) fn hoehe(&self) -> f32 {
        self.y1 - self.y0
    }
    pub(crate) fn laenge(&self) -> f32 {
        (self.x1 - self.x0).max(self.hoehe())
    }
    /// Eine Spalte senkrechten Satzes: deutlich hoeher als breit.
    pub(crate) fn steht(&self) -> bool {
        self.hoehe() >= HOCHFORMAT * (self.x1 - self.x0)
    }
}

/// Senkrechter Satz: Spalten von rechts nach links, in der Spalte von oben nach unten.
/// Eine Spalte kann in mehrere Stuecke zerfallen sein.
pub(crate) fn spalten_ordnen<T>(z: &mut [T], lage: impl Fn(&T) -> &Zeile) {
    z.sort_by(|a, b| lage(b).mitte().0.total_cmp(&lage(a).mitte().0));
    let mut i = 0;
    while i < z.len() {
        let (mitte, breite) = (lage(&z[i]).mitte().0, lage(&z[i]).x1 - lage(&z[i]).x0);
        let mut j = i + 1;
        while j < z.len() && mitte - lage(&z[j]).mitte().0 < breite * 0.6 {
            j += 1;
        }
        z[i..j].sort_by(|a, b| lage(a).y0.total_cmp(&lage(b).y0));
        i = j;
    }
}

/// Ob eine Seite senkrecht gesetzt ist: Dann gehen die Spalten von rechts nach links.
pub(crate) fn senkrecht(zeilen: &[Zeile]) -> bool {
    zeilen.len() >= 3 && zeilen.iter().filter(|z| z.steht()).count() * 10 >= zeilen.len() * 6
}

fn eingabe(bild: &RgbImage) -> Tensor {
    let (b, h) = (bild.width() as usize, bild.height() as usize);
    // Die Modelle sind auf BGR geschliffen.
    tract_ndarray::Array4::from_shape_fn((1, 3, h, b), |(_, k, y, x)| {
        let w = f32::from(bild.get_pixel(x as u32, y as u32)[2 - k]) / 255.0;
        (w - MITTEL[k]) / STREUUNG[k]
    })
    .into()
}

/// Sucht die Zeilen; ihr Text bleibt leer.
pub(crate) fn finden(e: &Erkenner, bild: &RgbImage) -> TractResult<Vec<Zeile>> {
    let (b, h) = bild.dimensions();
    let mass = (FINDEN_KANTE as f32 / b.max(h) as f32).min(1.0);
    let raster = |v: u32| ((v as f32 * mass) as u32).div_ceil(FINDEN_RASTER).max(1) * FINDEN_RASTER;
    let (kb, kh) = (raster(b), raster(h));
    let klein = resize(bild, kb, kh, FilterType::Triangle);
    let plan = e.finden.fuer(&[1, 3, kh as usize, kb as usize])?;
    let aus = plan.run(tvec!(eingabe(&klein).into()))?;
    let karte = aus[0].to_plain_array_view::<f32>()?;
    let werte: Vec<f32> = karte.iter().copied().collect();
    let (sx, sy) = (b as f32 / kb as f32, h as f32 / kh as f32);
    Ok(aus_karte(&werte, kb as usize, kh as usize, sx, sy))
}

/// Zusammenhaengende Flaechen ueber der Schwelle werden Zeilen; die Schraeglage
/// gilt fuer die ganze Seite, denn kurze Woerter verraten ihre eigene nicht.
pub(crate) fn aus_karte(p: &[f32], b: usize, h: usize, sx: f32, sy: f32) -> Vec<Zeile> {
    let mut gesehen = vec![false; b * h];
    let mut flaechen: Vec<Vec<(u32, u32)>> = Vec::new();
    let mut stapel = Vec::new();
    for start in 0..b * h {
        if gesehen[start] || p[start] <= SCHWELLE {
            continue;
        }
        gesehen[start] = true;
        stapel.push(start);
        let mut f = Vec::new();
        let mut guete = 0.0f32;
        while let Some(i) = stapel.pop() {
            let (x, y) = (i % b, i / b);
            f.push((x as u32, y as u32));
            guete += p[i];
            let mut sieh = |j: usize| {
                if !gesehen[j] && p[j] > SCHWELLE {
                    gesehen[j] = true;
                    stapel.push(j);
                }
            };
            if x > 0 {
                sieh(i - 1);
            }
            if x + 1 < b {
                sieh(i + 1);
            }
            if y > 0 {
                sieh(i - b);
            }
            if y + 1 < h {
                sieh(i + b);
            }
        }
        if f.len() >= 8 && guete / f.len() as f32 >= MINDESTGUETE {
            flaechen.push(f);
        }
    }
    // Der Winkel aus den langen Zeilen, als Median.
    let mut winkel: Vec<f32> = flaechen.iter().filter_map(|f| hauptachse(f)).collect();
    winkel.sort_by(f32::total_cmp);
    let w = winkel.get(winkel.len() / 2).copied().unwrap_or(0.0);
    let (c, s) = (w.cos(), w.sin());
    let mut aus = Vec::with_capacity(flaechen.len());
    for f in &flaechen {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in f {
            let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
            let (u, v) = (x * c + y * s, -x * s + y * c);
            (x0, y0, x1, y1) = (x0.min(u), y0.min(v), x1.max(u), y1.max(v));
        }
        let (bb, hh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let d = bb * hh * ERWEITERN / (2.0 * (bb + hh));
        aus.push(Zeile { x0: (x0 - d) * sx, y0: (y0 - d) * sy, x1: (x1 + d) * sx, y1: (y1 + d) * sy, winkel: w, text: String::new(), guete: 0.0 });
    }
    aus
}

/// Winkel einer langen, flachen Flaeche, sonst nichts.
fn hauptachse(f: &[(u32, u32)]) -> Option<f32> {
    let n = f.len() as f32;
    let (mx, my) = f.iter().fold((0.0, 0.0), |(a, b), &(x, y)| (a + x as f32, b + y as f32));
    let (mx, my) = (mx / n, my / n);
    let (mut xx, mut xy, mut yy) = (0.0f32, 0.0f32, 0.0f32);
    for &(x, y) in f {
        let (u, v) = (x as f32 - mx, y as f32 - my);
        xx += u * u;
        xy += u * v;
        yy += v * v;
    }
    // Erst ab einem deutlich laenglichen Fleck sagt die Achse etwas.
    if xx < 16.0 * yy {
        return None;
    }
    Some(0.5 * (2.0 * xy).atan2(xx - yy))
}

static KERNE: AtomicUsize = AtomicUsize::new(0);

/// Aus der Kapazitaetseinstellung; 0 heisst: alle ausser einem.
pub(crate) fn kerne_setzen(n: usize) {
    KERNE.store(n, Ordering::Relaxed);
}

fn kerne() -> usize {
    match KERNE.load(Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map_or(1, |n| usize::from(n).saturating_sub(1).max(1)),
        n => n,
    }
}

/// Liest alle Zeilen, verteilt auf die Kerne.
pub(crate) fn lesen(e: &Leser, bild: &RgbImage, zeilen: &mut [Zeile]) -> TractResult<f32> {
    let weiter = AtomicUsize::new(0);
    let kerne = kerne();
    let anzahl = zeilen.len();
    let quelle: &[Zeile] = zeilen;
    let weiter = &weiter;
    let teile: Vec<TractResult<Vec<(usize, String, f32)>>> = std::thread::scope(|s| {
        let griffe: Vec<_> = (0..kerne)
            .map(|_| {
                s.spawn(move || {
                    let mut aus = Vec::new();
                    loop {
                        let i = weiter.fetch_add(1, Ordering::Relaxed);
                        if i >= anzahl {
                            return Ok(aus);
                        }
                        let (t, g) = eine_zeile(e, bild, &quelle[i])?;
                        aus.push((i, t, g));
                    }
                })
            })
            .collect();
        griffe.into_iter().map(|g| g.join().expect("Lesefaden")).collect()
    });
    let mut texte = vec![(String::new(), 0.0f32); anzahl];
    let (mut summe, mut zahl) = (0.0f32, 0usize);
    for t in teile {
        for (i, s, g) in t? {
            texte[i] = (s, g);
            summe += g;
            zahl += 1;
        }
    }
    for (z, (t, g)) in zeilen.iter_mut().zip(texte) {
        z.text = t;
        z.guete = g;
    }
    Ok(if zahl == 0 { 0.0 } else { summe / zahl as f32 })
}

fn eine_zeile(e: &Leser, bild: &RgbImage, z: &Zeile) -> TractResult<(String, f32)> {
    let (bb, hh) = ((z.x1 - z.x0).max(1.0), (z.y1 - z.y0).max(1.0));
    let h = e.netz.as_ref().map_or(ZEILENHOEHE as usize, |n| n.hoehe);
    // Senkrechte Spalten liest das Modell quer: oben wird links.
    let steht = e.schrift.senkrecht && z.steht();
    let verhaeltnis = if steht { hh / bb } else { bb / hh };
    let breite = ((verhaeltnis * h as f32).ceil() as usize).clamp(8, GROESSTE_BREITE);
    let raster = breite.div_ceil(BREITENRASTER).max(2) * BREITENRASTER;
    let (c, s) = (z.winkel.cos(), z.winkel.sin());
    // Von Zielpunkt (Spalte, Zeile) in die um den Winkel gedrehte Seite.
    let quelle = |x: usize, y: usize| -> (f32, f32) {
        let (fx, fy) = ((x as f32 + 0.5) / breite as f32, (y as f32 + 0.5) / h as f32);
        let (u, v) = if steht { (z.x1 - fy * bb, z.y0 + fx * hh) } else { (z.x0 + fx * bb, z.y0 + fy * hh) };
        (u * c - v * s, u * s + v * c)
    };
    let abtasten = |x: f32, y: f32, k: usize| -> f32 {
        let (x0, y0) = (x.floor().max(0.0), y.floor().max(0.0));
        let (fx, fy) = (x - x0, y - y0);
        let (xa, ya) = ((x0 as u32).min(bild.width() - 1), (y0 as u32).min(bild.height() - 1));
        let (xb, yb) = ((xa + 1).min(bild.width() - 1), (ya + 1).min(bild.height() - 1));
        let w = |px: u32, py: u32| f32::from(bild.get_pixel(px, py)[k]);
        (w(xa, ya) * (1.0 - fx) + w(xb, ya) * fx) * (1.0 - fy) + (w(xa, yb) * (1.0 - fx) + w(xb, yb) * fx) * fy
    };
    if let Some(netz) = &e.netz {
        let mut grau = image::GrayImage::new(breite as u32, h as u32);
        for y in 0..h {
            for x in 0..breite {
                let (qx, qy) = quelle(x, y);
                let m = (abtasten(qx, qy, 0) + abtasten(qx, qy, 1) + abtasten(qx, qy, 2)) / 3.0;
                grau.put_pixel(x as u32, y as u32, image::Luma([m.round() as u8]));
            }
        }
        let (zeichen, g) = netz.lesen(&grau).map_err(TractError::msg)?;
        let text = if e.schrift.von_rechts { von_rechts_gelesen(zeichen) } else { zeichen.concat() };
        return Ok((text.trim().to_string(), g));
    }
    let mut a = tract_ndarray::Array4::<f32>::zeros((1, 3, h, raster));
    for y in 0..h {
        for x in 0..breite {
            let (qx, qy) = quelle(x, y);
            for k in 0..3 {
                a[(0, k, y, x)] = lesewert(abtasten(qx, qy, 2 - k));
            }
        }
    }
    let plan = e.plaene.fuer(&[1, 3, h, raster])?;
    let aus = plan.run(tvec!(Tensor::from(a).into()))?;
    let w = aus[0].to_plain_array_view::<f32>()?;
    let (text, guete) = entziffern(&e.zeichen, w.shape()[1], w.shape()[2], w.iter().copied());
    Ok((if e.schrift.von_rechts { von_rechts_gelesen(text.chars().map(String::from).collect()) } else { text }, guete))
}

/// Die Glyphen von links nach rechts in die Lesefolge einer rechtslaeufigen Zeile.
///
/// 📌 **Ziffern laufen auch dort von links nach rechts.** Die Zeile schlicht
/// umzudrehen machte aus 1325 die 5231, und das faellt beim Lesen nicht auf, weil
/// es eine ebenso gute Zahl ist. Gemessen an Hebraeisch, Jiddisch und Syrisch:
/// keine Zahl richtig, die Zeichenaehnlichkeit trotzdem ueber 94 %.
fn von_rechts_gelesen(mut zeichen: Vec<String>) -> String {
    let art = |z: &String| z.chars().next().map_or(Lauf::Sonst, Lauf::von);
    zeichen.reverse();
    let mut i = 0;
    while i < zeichen.len() {
        let erste = art(&zeichen[i]);
        if erste == Lauf::Sonst {
            if let [klammer] = zeichen[i].chars().collect::<Vec<_>>()[..] {
                zeichen[i] = gespiegelt(klammer).to_string();
            }
            i += 1;
            continue;
        }
        let mut ende = i + 1;
        while ende < zeichen.len() {
            let brueckt = |z: &String| {
                let c = z.chars().next().unwrap_or(' ');
                (c.is_whitespace() && erste == Lauf::Buchstabe) || (".,:/-".contains(c) && erste == Lauf::Ziffer)
            };
            if art(&zeichen[ende]) != Lauf::Sonst {
                ende += 1;
            } else if ende + 1 < zeichen.len() && brueckt(&zeichen[ende]) && art(&zeichen[ende + 1]) == erste {
                ende += 2;
            } else {
                break;
            }
        }
        zeichen[i..ende].reverse();
        i = ende;
    }
    zeichen.concat()
}

#[derive(Clone, Copy, PartialEq)]
enum Lauf {
    Ziffer,
    Buchstabe,
    Sonst,
}

impl Lauf {
    fn von(c: char) -> Self {
        // Arabische und persische Ziffern zaehlen mit: Sie schreiben sich genauso.
        if c.is_ascii_digit() || ('\u{660}'..='\u{669}').contains(&c) || ('\u{6f0}'..='\u{6f9}').contains(&c) {
            Lauf::Ziffer
        } else if c.is_alphabetic() && c < '\u{250}' {
            Lauf::Buchstabe
        } else {
            Lauf::Sonst
        }
    }
}

/// Eine Klammer, die links steht, schliesst in rechtslaeufigem Text.
fn gespiegelt(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        anders => anders,
    }
}

/// Das Lesemodell erwartet Werte von -1 bis 1.
fn lesewert(v: f32) -> f32 {
    (v / 255.0 - 0.5) / 0.5
}

/// Das Beste je Schritt, Wiederholungen und Leerstellen fallen weg.
pub(crate) fn entziffern(zeichen: &[String], schritte: usize, klassen: usize, werte: impl Iterator<Item = f32>) -> (String, f32) {
    let werte: Vec<f32> = werte.collect();
    let mut aus = String::new();
    let mut vorher = 0usize;
    let (mut summe, mut zahl) = (0.0f32, 0usize);
    for t in 0..schritte {
        let z = &werte[t * klassen..(t + 1) * klassen];
        let beste = z.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i);
        if beste != 0 && beste != vorher {
            aus.push_str(zeichen.get(beste).map_or("", String::as_str));
            summe += z[beste];
            zahl += 1;
        }
        vorher = beste;
    }
    (aus.trim().to_string(), if zahl == 0 { 0.0 } else { summe / zahl as f32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_modelle_bekommen_ihre_kanaele_und_ihren_wertebereich() {
        let t = eingabe(&RgbImage::from_pixel(1, 1, image::Rgb([10, 20, 30])));
        let v = t.to_plain_array_view::<f32>().unwrap();
        // Blau zuerst.
        assert!((v[[0, 0, 0, 0]] - (30.0 / 255.0 - MITTEL[0]) / STREUUNG[0]).abs() < 1e-5);
        assert!((v[[0, 2, 0, 0]] - (10.0 / 255.0 - MITTEL[2]) / STREUUNG[2]).abs() < 1e-5);
        assert_eq!((lesewert(0.0), lesewert(255.0)), (-1.0, 1.0));
    }

    #[test]
    fn wiederholungen_und_leerstellen_fallen_weg() {
        let z = ["", "a", "b"].map(String::from);
        // a a leer a b
        let w = [0., 1., 0., 0., 1., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
        let (t, g) = entziffern(&z, 5, 3, w.into_iter());
        assert_eq!((t.as_str(), g), ("aab", 1.0));
    }

    fn rechts(sichtbar: &str) -> String {
        von_rechts_gelesen(sichtbar.chars().map(String::from).collect())
    }

    #[test]
    fn rechtslaeufig_bleiben_zahlen_und_lateinisches_ganz() {
        // Sichtbar von links: das Satzende, die Zahl, dann die Woerter.
        assert_eq!(rechts(".1325 תנשב"), "בשנת 1325.");
        assert_eq!(rechts("١٢٨٦ ماع"), "عام ١٢٨٦");
        assert_eq!(rechts("1,5 ב"), "ב 1,5");
        assert_eq!(rechts("New York ב"), "ב New York");
        // Zwei Zahlen behalten ihre Reihenfolge von rechts.
        assert_eq!(rechts("34 12"), "12 34");
    }

    #[test]
    fn rechtslaeufig_wenden_sich_die_klammern() {
        assert_eq!(rechts("(בא)"), "(אב)");
        // Zeichen aus mehreren Codepoints bleiben, wie sie sind.
        let z = vec!["בּ".to_string(), "א".to_string()];
        assert_eq!(von_rechts_gelesen(z), "אבּ");
    }

    #[test]
    fn eine_flaeche_wird_eine_zeile_und_rauschen_keine() {
        let (b, h) = (64usize, 32usize);
        let mut p = vec![0.0f32; b * h];
        for y in 10..14 {
            for x in 5..40 {
                p[y * b + x] = 0.9;
            }
        }
        p[2 * b + 60] = 0.9;
        let z = aus_karte(&p, b, h, 2.0, 2.0);
        assert_eq!(z.len(), 1, "{z:?}");
        assert!(z[0].x0 < 10.0 && z[0].x1 > 78.0 && z[0].y0 < 20.0 && z[0].y1 > 26.0, "{z:?}");
    }

    #[test]
    fn senkrechte_spalten_laufen_von_rechts_und_von_oben() {
        let z = |x0: f32, y0: f32, t: &str| Zeile { x0, y0, x1: x0 + 40.0, y1: y0 + 200.0, winkel: 0.0, text: t.into(), guete: 1.0 };
        let mut v = vec![z(0.0, 0.0, "links"), z(100.0, 300.0, "rechts-unten"), z(100.0, 0.0, "rechts-oben")];
        assert!(senkrecht(&v));
        spalten_ordnen(&mut v, |l| l);
        let t: Vec<&str> = v.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(t, ["rechts-oben", "rechts-unten", "links"]);
    }

    #[test]
    fn waagerechte_zeilen_sind_nicht_senkrecht() {
        let z = Zeile { x0: 0.0, y0: 0.0, x1: 400.0, y1: 40.0, winkel: 0.0, text: String::new(), guete: 1.0 };
        assert!(!senkrecht(&[z.clone(), z.clone(), z]));
    }

    #[test]
    fn eine_schwache_flaeche_zaehlt_nicht() {
        let (b, h) = (32usize, 16usize);
        let mut p = vec![0.0f32; b * h];
        for x in 4..20 {
            p[8 * b + x] = 0.4;
        }
        assert!(aus_karte(&p, b, h, 1.0, 1.0).is_empty());
    }
}
