//! Der Seitenaufbau: was wo steht, in welcher Reihenfolge es gelesen wird.

use crate::text::Zeile;
use image::imageops::{crop_imm, resize, FilterType};
use image::RgbImage;
use tract_onnx::prelude::*;

pub(crate) const KANTE: usize = 480;
const MINDESTGUETE: f32 = 0.35;
/// So viel Ueberdeckung, und zwei Bereiche sind einer.
const DECKUNG: f32 = 0.7;
/// Steht so viel Text darin, ist es keine Abbildung.
const TEXT_IN_BILD: f32 = 0.45;
/// Kopf- und Fussrand; Treffer in der Seitenmitte gelten nicht.
const RAND: f32 = 0.12;
/// Auf einer vollen Seite muss der Titel so viel groesser sein als der Text.
const TITEL_ABSTAND: f32 = 1.4;
/// Bis zu so vielen Zeilen kann es eine Titelseite sein, auch ohne Abstand.
const TITELSEITE_ZEILEN: usize = 6;
/// So sicher muss das Modell beim Lesen des Titels gewesen sein.
const TITEL_SICHER: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    Titel,
    Text,
    Tabelle,
    Abbildung,
    Rand,
}

#[derive(Debug, Clone, Copy)]
pub struct Bereich {
    pub art: Art,
    pub guete: f32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Bereich {
    fn flaeche(&self) -> f32 {
        (self.x1 - self.x0).max(0.0) * (self.y1 - self.y0).max(0.0)
    }
    fn enthaelt(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x0 - 4.0 && x <= self.x1 + 4.0 && y >= self.y0 - 4.0 && y <= self.y1 + 4.0
    }
}

fn art_von(klasse: usize) -> Art {
    // Die Reihenfolge der 23 Klassen des Modells.
    match klasse {
        0 | 11 => Art::Titel,
        1 | 18 => Art::Abbildung,
        8 => Art::Tabelle,
        3 | 13 | 15 | 16 | 20 | 21 => Art::Rand,
        _ => Art::Text,
    }
}

pub(crate) fn finden(plan: &crate::Plan, bild: &RgbImage) -> TractResult<Vec<Bereich>> {
    let (b, h) = bild.dimensions();
    let klein = resize(bild, KANTE as u32, KANTE as u32, FilterType::Triangle);
    let (mittel, streu) = ([0.485f32, 0.456, 0.406], [0.229f32, 0.224, 0.225]);
    let e = tract_ndarray::Array4::from_shape_fn((1, 3, KANTE, KANTE), |(_, k, y, x)| {
        (f32::from(klein.get_pixel(x as u32, y as u32)[k]) / 255.0 - mittel[k]) / streu[k]
    });
    let mass = tract_ndarray::arr2(&[[KANTE as f32 / h as f32, KANTE as f32 / b as f32]]);
    let aus = plan.run(tvec!(Tensor::from(e).into(), Tensor::from(mass).into()))?;
    let d = aus[0].to_plain_array_view::<f32>()?;
    let mut roh = Vec::new();
    for z in d.rows() {
        if z[1] >= MINDESTGUETE {
            roh.push(Bereich { art: art_von(z[0] as usize), guete: z[1], x0: z[2], y0: z[3], x1: z[4], y1: z[5] });
        }
    }
    Ok(bereinigen(roh, h as f32))
}

/// Das Modell meldet dieselbe Stelle oft mehrfach; es bleibt die sicherste Deutung.
pub(crate) fn bereinigen(mut roh: Vec<Bereich>, seitenhoehe: f32) -> Vec<Bereich> {
    roh.sort_by(|a, b| b.guete.total_cmp(&a.guete));
    let mut aus: Vec<Bereich> = Vec::new();
    for mut b in roh {
        let doppelt = aus.iter().any(|k| {
            let w = (b.x1.min(k.x1) - b.x0.max(k.x0)).max(0.0);
            let h = (b.y1.min(k.y1) - b.y0.max(k.y0)).max(0.0);
            w * h >= DECKUNG * b.flaeche().min(k.flaeche())
        });
        if doppelt {
            continue;
        }
        // Eine „Fusszeile“ mitten auf der Seite ist keine.
        if b.art == Art::Rand && b.y1 > seitenhoehe * RAND && b.y0 < seitenhoehe * (1.0 - RAND) {
            b.art = Art::Text;
        }
        aus.push(b);
    }
    aus
}

/// Die groesste Schrift einer Seite, wenn sie heraussticht; Titel gehen oft ueber Zeilen.
pub(crate) fn titelkandidat(zeilen: &[Zeile]) -> String {
    if crate::text::senkrecht(zeilen) {
        return String::new();
    }
    let mut hoehen: Vec<f32> = zeilen.iter().filter(|z| !z.text.trim().is_empty()).map(Zeile::hoehe).collect();
    hoehen.sort_by(f32::total_cmp);
    let (Some(&groesste), Some(&mitte)) = (hoehen.last(), hoehen.get(hoehen.len() / 2)) else {
        return String::new();
    };
    if hoehen.len() > TITELSEITE_ZEILEN && groesste < TITEL_ABSTAND * mitte {
        return String::new();
    }
    let mut titel: Vec<&Zeile> = zeilen.iter().filter(|z| z.hoehe() >= 0.8 * groesste && !z.text.trim().is_empty()).collect();
    titel.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    titel.truncate(3);
    let sicher = titel.iter().map(|z| z.guete).sum::<f32>() / titel.len().max(1) as f32;
    // 📌 Verzierte Schrift auf Titelseiten liest sich schlecht: 0,38 war „ie, üäseee =d,“.
    if sicher < TITEL_SICHER {
        return String::new();
    }
    titel.iter().map(|z| z.text.trim()).collect::<Vec<_>>().join(" ")
}

/// Spalten und Reihen: geteilt wird, wo eine Luecke durch alles geht.
pub(crate) fn reihenfolge(k: &[(f32, f32, f32, f32)]) -> Vec<usize> {
    fn teilen(k: &[(f32, f32, f32, f32)], ids: Vec<usize>, aus: &mut Vec<usize>) {
        if ids.len() < 2 {
            aus.extend(ids);
            return;
        }
        for waagerecht in [true, false] {
            let span = |i: usize| {
                let (x0, y0, x1, y1) = k[i];
                let (a, b) = if waagerecht { (y0, y1) } else { (x0, x1) };
                let rand = (b - a) * 0.08;
                (a + rand, b - rand)
            };
            let mut s = ids.clone();
            s.sort_by(|&a, &b| span(a).0.total_cmp(&span(b).0));
            let mut gruppen: Vec<Vec<usize>> = Vec::new();
            let mut ende = f32::MIN;
            for i in s {
                let (a, b) = span(i);
                if a > ende && !gruppen.is_empty() {
                    gruppen.push(vec![i]);
                } else if let Some(g) = gruppen.last_mut() {
                    g.push(i);
                } else {
                    gruppen.push(vec![i]);
                }
                ende = ende.max(b);
            }
            if gruppen.len() > 1 {
                for g in gruppen {
                    teilen(k, g, aus);
                }
                return;
            }
        }
        let mut s = ids;
        s.sort_by(|&a, &b| k[a].1.total_cmp(&k[b].1));
        aus.extend(s);
    }
    let mut aus = Vec::with_capacity(k.len());
    teilen(k, (0..k.len()).collect(), &mut aus);
    aus
}

fn zeilen_ordnen(z: &mut [&Zeile]) {
    let alle: Vec<Zeile> = z.iter().map(|l| (*l).clone()).collect();
    if crate::text::senkrecht(&alle) {
        crate::text::spalten_ordnen(z, |l| *l);
        return;
    }
    z.sort_by(|a, b| a.mitte().1.total_cmp(&b.mitte().1));
    // Zeilen auf gleicher Hoehe von links nach rechts.
    let mut i = 0;
    while i < z.len() {
        let hoehe = z[i].hoehe().max(1.0) * 0.5;
        let mut j = i + 1;
        while j < z.len() && z[j].mitte().1 - z[i].mitte().1 < hoehe {
            j += 1;
        }
        z[i..j].sort_by(|a, b| a.x0.total_cmp(&b.x0));
        i = j;
    }
}

/// Fliesstext: Trennungen am Zeilenende werden aufgeloest.
pub(crate) fn absatz(z: &[&Zeile]) -> String {
    let mut aus = String::new();
    for t in z.iter().map(|x| x.text.trim()).filter(|t| !t.is_empty()) {
        if aus.ends_with(['-', '\u{ad}']) && t.chars().next().is_some_and(char::is_lowercase) {
            aus.pop();
        } else if !aus.is_empty() {
            aus.push(' ');
        }
        aus.push_str(t);
    }
    aus
}

/// Eine Tabelle aus der Lage ihrer Zellen.
pub(crate) fn tabelle(z: &[&Zeile]) -> String {
    let hoehe = {
        let mut h: Vec<f32> = z.iter().map(|x| x.hoehe()).collect();
        h.sort_by(f32::total_cmp);
        h.get(h.len() / 2).copied().unwrap_or(1.0)
    };
    let mut reihen: Vec<Vec<&Zeile>> = Vec::new();
    for &l in z {
        match reihen.last_mut() {
            Some(r) if (l.mitte().1 - r[0].mitte().1).abs() < hoehe * 0.6 => r.push(l),
            _ => reihen.push(vec![l]),
        }
    }
    for r in &mut reihen {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    let breite = reihen.iter().map(Vec::len).max().unwrap_or(0);
    if breite < 2 || reihen.len() < 2 {
        return absatz(z);
    }
    let raster: Vec<f32> = reihen.iter().find(|r| r.len() == breite).map(|r| r.iter().map(|c| c.mitte().0).collect()).unwrap_or_default();
    let mut aus = String::new();
    for (n, r) in reihen.iter().enumerate() {
        let mut zellen = vec![String::new(); breite];
        for c in r {
            let m = c.mitte().0;
            let s = raster.iter().enumerate().min_by(|a, b| (a.1 - m).abs().total_cmp(&(b.1 - m).abs())).map_or(0, |(i, _)| i);
            if !zellen[s].is_empty() {
                zellen[s].push(' ');
            }
            zellen[s].push_str(c.text.trim());
        }
        aus.push_str(&format!("| {} |\n", zellen.join(" | ").replace('\n', " ")));
        if n == 0 {
            aus.push_str(&format!("|{}\n", "---|".repeat(breite)));
        }
    }
    aus.trim_end().to_string()
}

/// Ordnet die Zeilen den Bereichen zu und schreibt die Seite.
pub(crate) fn zusammensetzen(bild: &RgbImage, bereiche: &[Bereich], zeilen: &[Zeile]) -> (String, Vec<RgbImage>) {
    let mut je: Vec<Vec<&Zeile>> = vec![Vec::new(); bereiche.len()];
    let mut lose: Vec<&Zeile> = Vec::new();
    for z in zeilen {
        let treffer = bereiche.iter().enumerate().filter(|(_, b)| b.enthaelt(z.mitte())).min_by(|a, b| a.1.flaeche().total_cmp(&b.1.flaeche()));
        match treffer {
            Some((i, _)) => je[i].push(z),
            None => lose.push(z),
        }
    }
    // Was kein Bereich fasst, wird zu Bloecken aus benachbarten Zeilen.
    lose.sort_by(|a, b| a.mitte().1.total_cmp(&b.mitte().1));
    let mut gruppen: Vec<Vec<&Zeile>> = Vec::new();
    for l in lose {
        let platz = gruppen.iter_mut().rev().find(|g| {
            let v = g[g.len() - 1];
            let ueber = l.x1.min(v.x1) - l.x0.max(v.x0);
            ueber > 0.5 * (l.x1 - l.x0).min(v.x1 - v.x0) && l.y0 - v.y1 < 1.6 * v.hoehe().min(v.x1 - v.x0)
        });
        match platz {
            Some(g) => g.push(l),
            None => gruppen.push(vec![l]),
        }
    }
    // Eine Abbildung mit viel Text ist vermutlich Text.
    let arten: Vec<Art> = bereiche
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if b.art != Art::Abbildung {
                return b.art;
            }
            let bedeckt: f32 = je[i].iter().map(|z| (z.x1 - z.x0) * z.hoehe()).sum();
            if bedeckt > TEXT_IN_BILD * b.flaeche() { Art::Text } else { Art::Abbildung }
        })
        .collect();
    enum Block<'a> {
        B(usize),
        L(&'a [&'a Zeile]),
    }
    let mut bloecke: Vec<(Block, (f32, f32, f32, f32))> = Vec::new();
    for (i, b) in bereiche.iter().enumerate() {
        if arten[i] != Art::Rand && (arten[i] == Art::Abbildung || !je[i].is_empty()) {
            bloecke.push((Block::B(i), (b.x0, b.y0, b.x1, b.y1)));
        }
    }
    for g in &gruppen {
        let k = g.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, z| (a.0.min(z.x0), a.1.min(z.y0), a.2.max(z.x1), a.3.max(z.y1)));
        bloecke.push((Block::L(g), k));
    }
    // Senkrechter Satz laeuft von rechts nach links: gespiegelt ordnen.
    let spiegel = if crate::text::senkrecht(zeilen) { -1.0 } else { 1.0 };
    let lagen: Vec<_> = bloecke
        .iter()
        .map(|b| if spiegel < 0.0 { (-b.1 .2, b.1 .1, -b.1 .0, b.1 .3) } else { b.1 })
        .collect();
    let mut md: Vec<String> = Vec::new();
    let mut bilder = Vec::new();
    for i in reihenfolge(&lagen) {
        match &bloecke[i].0 {
            Block::L(g) => {
                let mut z = g.to_vec();
                zeilen_ordnen(&mut z);
                md.push(absatz(&z));
            }
            Block::B(n) => {
                let mut z = je[*n].clone();
                zeilen_ordnen(&mut z);
                match arten[*n] {
                    Art::Abbildung => {
                        let b = &bereiche[*n];
                        let (x0, y0) = (b.x0.max(0.0) as u32, b.y0.max(0.0) as u32);
                        let (w, h) = ((b.x1 as u32).min(bild.width()).saturating_sub(x0), (b.y1 as u32).min(bild.height()).saturating_sub(y0));
                        let a = crop_imm(bild, x0, y0, w, h).to_image();
                        if !crate::klein_genug(&a) {
                            md.push(crate::ABBILDUNG.to_string());
                            bilder.push(a);
                        }
                    }
                    Art::Tabelle => md.push(tabelle(&z)),
                    Art::Titel => md.push(format!("## {}", absatz(&z))),
                    _ => md.push(absatz(&z)),
                }
            }
        }
    }
    md.retain(|t| !t.trim().is_empty() && t != "## ");
    (md.join("\n\n"), bilder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(x0: f32, y0: f32, x1: f32, y1: f32, t: &str) -> Zeile {
        Zeile { x0, y0, x1, y1, winkel: 0.0, text: t.into(), guete: 1.0 }
    }

    #[test]
    fn der_titel_ist_die_groesste_schrift_und_darf_zwei_zeilen_haben() {
        let mut v = vec![z(0., 0., 200., 60., "Kritik der"), z(0., 70., 200., 130., "reinen Vernunft")];
        v.extend((0..8).map(|i| z(0., 200. + i as f32 * 30., 200., 225. + i as f32 * 30., "Fliesstext")));
        assert_eq!(titelkandidat(&v), "Kritik der reinen Vernunft");
        let voll: Vec<Zeile> = (0..12).map(|i| z(0., i as f32 * 30., 200., 25. + i as f32 * 30., "Text")).collect();
        assert_eq!(titelkandidat(&voll), "", "eine gewoehnliche Seite hat keinen Titel");
        assert_eq!(titelkandidat(&[]), "");
        let mut unsicher = z(0., 0., 200., 60., "ie, üäseee =d,");
        unsicher.guete = 0.38;
        assert_eq!(titelkandidat(&[unsicher]), "", "was unsicher gelesen ist, ist kein Titel");
    }

    #[test]
    fn zwei_spalten_werden_nacheinander_gelesen() {
        let titel = (0.0, 0.0, 200.0, 20.0);
        let a = (0.0, 30.0, 90.0, 90.0);
        let b = (0.0, 100.0, 90.0, 180.0);
        let c = (110.0, 30.0, 200.0, 180.0);
        assert_eq!(reihenfolge(&[c, b, titel, a]), vec![2, 3, 1, 0]);
    }

    #[test]
    fn trennungen_am_zeilenende_verschwinden() {
        let (a, b, c) = (z(0., 0., 9., 9., "Betriebs-"), z(0., 10., 9., 19., "system und"), z(0., 20., 9., 29., "Mehr-Wert"));
        assert_eq!(absatz(&[&a, &b, &c]), "Betriebssystem und Mehr-Wert");
        let (d, e) = (z(0., 0., 9., 9., "Nord-"), z(0., 10., 9., 19., "Ost"));
        assert_eq!(absatz(&[&d, &e]), "Nord- Ost", "vor einem Grossbuchstaben bleibt der Strich");
    }

    #[test]
    fn eine_tabelle_behaelt_reihen_und_spalten() {
        let t = [z(0., 0., 30., 10., "A"), z(100., 0., 130., 10., "B"), z(0., 20., 30., 30., "1"), z(100., 20., 130., 30., "2")];
        let r: Vec<&Zeile> = t.iter().collect();
        assert_eq!(tabelle(&r), "| A | B |\n|---|---|\n| 1 | 2 |");
    }

    #[test]
    fn senkrechte_bloecke_stehen_von_rechts_nach_links() {
        let bild = RgbImage::new(400, 400);
        let spalten = [z(10., 10., 50., 300., "links"), z(310., 10., 350., 300., "rechts"), z(160., 10., 200., 300., "mitte")];
        let (md, _) = zusammensetzen(&bild, &[], &spalten);
        assert_eq!(md, "rechts\n\nmitte\n\nlinks");
    }

    fn b(art: Art, guete: f32, y0: f32, y1: f32) -> Bereich {
        Bereich { art, guete, x0: 0.0, y0, x1: 100.0, y1 }
    }

    #[test]
    fn dieselbe_stelle_zaehlt_einmal_und_die_sicherste_deutung_bleibt() {
        let r = bereinigen(vec![b(Art::Text, 0.4, 0., 50.), b(Art::Titel, 0.9, 0., 50.), b(Art::Text, 0.5, 60., 90.)], 1000.0);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].art, Art::Titel);
    }

    #[test]
    fn eine_fusszeile_gilt_nur_am_rand() {
        let r = bereinigen(vec![b(Art::Rand, 0.9, 500., 520.), b(Art::Rand, 0.8, 950., 970.)], 1000.0);
        assert_eq!(r[0].art, Art::Text, "mitten auf der Seite");
        assert_eq!(r[1].art, Art::Rand);
    }
}
