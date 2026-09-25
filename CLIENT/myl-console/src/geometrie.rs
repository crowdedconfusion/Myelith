//! Heilige Geometrie um das Logo: acht Motive, eines je Sitzung.
//!
//! # ⚑ Was das Motiv zusichert (Auftrag des Projektinhabers, 2026-09-24)
//!
//! 1. **Rein geometrisch**, nach dem Vorbild der heiligen Geometrie. Acht
//!    Motive, vom Projektinhaber aus vierzehn Entwuerfen ausgewaehlt:
//!    Blume des Lebens, Quadratwirbel, Sterntetraeder-Gitter,
//!    Hexagonwirbel, Dreieckwirbel, Saatgitter, Merkaba-Feld, Goldene
//!    Spirale.
//! 2. **Nichts beruehrt das Logo.** Um den Schriftzug liegt eine
//!    Sperrzone von [`FREI_X`] Spalten und [`FREI_Y`] Zeilen, in die kein
//!    Punkt faellt. Das Logo hebt sich ab, und das Muster umrandet es.
//! 3. ⚑ **Geschnitten wird nur an zwei Kanten**: am Bildrand und an der
//!    Sperrzone. Sonst laufen die Muster frei, auch ueber und unter dem
//!    Logo durch die Mitte, wo sich die Figuren beider Seiten treffen
//!    (Festlegung des Projektinhabers). Eine Figur, die vorher endete,
//!    saehe aus wie ein Aufkleber und nicht wie ein Muster.
//! 4. **Jede Sitzung wuerfelt eines der acht** ([`sitzungsmotiv`]), und
//!    es bleibt fuer die ganze Sitzung: Ein Neudruck, der das Motiv
//!    wechselte, saehe aus wie ein anderes Programm. `MYL_MOTIV` legt es
//!    fest, mit dem Namen aus [`Motiv::name`].
//!
//! # Warum Braille-Zeichen
//!
//! Kreise und Linien in 60 Grad lassen sich mit Kastenzeichen nicht
//! zeichnen, nur andeuten. Ein Braille-Zeichen traegt 2 × 4 Punkte, und
//! weil eine Zelle etwa doppelt so hoch wie breit ist, sind diese Punkte
//! annaehernd quadratisch: **Ein Kreis bleibt ein Kreis.** Gezeichnet wird
//! auf einer [`Leinwand`] aus solchen Punkten, gerastert wird am Ende.
//!
//! # Ganzzahlig, wie alles hier
//!
//! Linien nach Bresenham, Kreise nach dem Mittelpunktverfahren, Winkel
//! ueber eine Sinustafel in ganzen Grad. Fuer ein Bild am Terminal waere
//! Gleitkomma zulaessig; dieses Repositorium sucht aber nach jeder Aenderung
//! nach `f32` und `f64`, und eine Tafel kostet weniger als jede
//! Erklaerung, warum ein Treffer harmlos ist.
//!
//! 📌 **Entworfen wurde im Bild**, in mehreren Runden, mit einer
//! Referenzfassung derselben Rechnung; die hier zeichnet Punkt fuer Punkt
//! dasselbe.

use std::sync::OnceLock;

/// Sinus in ganzen Grad von 0 bis 90, mal 4096.
const SINUS: [i32; 91] = [
    0, 71, 143, 214, 286, 357, 428, 499, 570, 641, 711, 782, 852, 921, 991, 1060, 1129, 1198,
    1266, 1334, 1401, 1468, 1534, 1600, 1666, 1731, 1796, 1860, 1923, 1986, 2048, 2110, 2171,
    2231, 2290, 2349, 2408, 2465, 2522, 2578, 2633, 2687, 2741, 2793, 2845, 2896, 2946, 2996,
    3044, 3091, 3138, 3183, 3228, 3271, 3314, 3355, 3396, 3435, 3474, 3511, 3547, 3582, 3617,
    3650, 3681, 3712, 3742, 3770, 3798, 3824, 3849, 3873, 3896, 3917, 3937, 3956, 3974, 3991,
    4006, 4021, 4034, 4046, 4056, 4065, 4074, 4080, 4086, 4090, 4094, 4095, 4096,
];

/// Die Skala der Tafel.
const EINS: i32 = 4096;

fn sinus(grad: i32) -> i32 {
    let d = grad.rem_euclid(360) as usize;
    match d {
        0..=90 => SINUS[d],
        91..=180 => SINUS[180 - d],
        181..=270 => -SINUS[d - 180],
        _ => -SINUS[360 - d],
    }
}

fn kosinus(grad: i32) -> i32 {
    sinus(grad + 90)
}

/// Ganzzahlige Division, zur naechsten Zahl gerundet, und zwar fuer beide
/// Vorzeichen gleich: Eine Figur links der Mitte soll genau so aussehen
/// wie ihr Spiegelbild rechts.
fn teile(zaehler: i32, nenner: i32) -> i32 {
    (2 * zaehler + nenner).div_euclid(2 * nenner)
}

/// Wie viele Spalten neben dem Schriftzug frei bleiben.
pub const FREI_X: usize = 3;
/// Wie viele Zeilen ueber und unter dem Schriftzug frei bleiben.
pub const FREI_Y: usize = 1;

/// Die Bits eines Braille-Zeichens, nach Punktzeile und Punktspalte.
const BIT: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Eine Zeichenflaeche aus Braille-Punkten, mit dem Schriftzug darin.
pub struct Leinwand {
    breite: usize,
    reihen: usize,
    /// Die Zeile, in der der Schriftzug beginnt.
    logo_oben: usize,
    /// Die Spalte, in der er beginnt.
    einzug: usize,
    punkte: Vec<Vec<u8>>,
    /// Mitte des Schriftzugs, in Punkten.
    mx: i32,
    my: i32,
}

impl Leinwand {
    pub fn neu(breite: usize, reihen: usize, logo_oben: usize) -> Self {
        Self {
            breite,
            reihen,
            logo_oben,
            einzug: breite.saturating_sub(crate::banner::SCHRIFTBREITE) / 2,
            punkte: vec![vec![0; breite]; reihen],
            mx: breite as i32,
            my: 4 * logo_oben as i32 + 12,
        }
    }

    /// Breite in Punkten.
    fn b(&self) -> i32 {
        2 * self.breite as i32
    }

    /// Hoehe in Punkten.
    fn h(&self) -> i32 {
        4 * self.reihen as i32
    }

    /// **Liegt diese Zelle in der Sperrzone um den Schriftzug?**
    ///
    /// ⛔️ Die Zusage „nichts beruehrt das Logo" haengt an dieser einen
    /// Pruefung: Jeder Punkt geht durch sie.
    pub fn gesperrt(&self, spalte: usize, zeile: usize) -> bool {
        let (x0, y0) = (self.einzug as isize - FREI_X as isize, self.logo_oben as isize - FREI_Y as isize);
        let x1 = (self.einzug + crate::banner::SCHRIFTBREITE + FREI_X) as isize;
        let y1 = (self.logo_oben + 6 + FREI_Y) as isize;
        let (x, y) = (spalte as isize, zeile as isize);
        x0 <= x && x < x1 && y0 <= y && y < y1
    }

    /// Die Zelle eines Punktes, falls er auf der Flaeche und ausserhalb der
    /// Sperrzone liegt.
    fn zelle(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        if x < 0 || y < 0 || x >= self.b() || y >= self.h() {
            return None;
        }
        let (sx, zy) = ((x / 2) as usize, (y / 4) as usize);
        (!self.gesperrt(sx, zy)).then_some((sx, zy))
    }

    fn punkt(&mut self, x: i32, y: i32) {
        if let Some((sx, zy)) = self.zelle(x, y) {
            self.punkte[zy][sx] |= BIT[(y % 4) as usize][(x % 2) as usize];
        }
    }

    /// Eine Linie nach Bresenham.
    ///
    /// ⚠️ **Mit Obergrenze**: Die Motive ziehen Linien bewusst ueber den
    /// Rand hinaus, damit Gitter bis an die Kante reichen. Ein Fehler in
    /// einer Koordinate soll ein falsches Bild ergeben und keine Schleife,
    /// die nicht endet.
    fn linie(&mut self, (mut x0, mut y0): (i32, i32), (x1, y1): (i32, i32)) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut fehler = dx + dy;
        for _ in 0..100_000 {
            self.punkt(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * fehler;
            if e2 >= dy {
                fehler += dy;
                x0 += sx;
            }
            if e2 <= dx {
                fehler += dx;
                y0 += sy;
            }
        }
    }

    /// Ein Kreis nach dem Mittelpunktverfahren, in acht Spiegelungen.
    fn kreis(&mut self, (mx, my): (i32, i32), r: i32) {
        let (mut x, mut y, mut f) = (r, 0, 1 - r);
        while x >= y {
            for (px, py) in [(x, y), (y, x), (-y, x), (-x, y), (-x, -y), (-y, -x), (y, -x), (x, -y)] {
                self.punkt(mx + px, my + py);
            }
            y += 1;
            if f < 0 {
                f += 2 * y + 1;
            } else {
                x -= 1;
                f += 2 * (y - x) + 1;
            }
        }
    }

    /// **Rastert die Flaeche zu Zeilen**, mit dem Schriftzug an seinem
    /// Platz.
    pub fn zeilen(&self) -> Vec<String> {
        (0..self.reihen)
            .map(|zy| {
                let mut z = String::with_capacity(self.breite * 3);
                for sx in 0..self.breite {
                    let im_logo = (self.logo_oben..self.logo_oben + 6).contains(&zy)
                        && (self.einzug..self.einzug + crate::banner::SCHRIFTBREITE).contains(&sx);
                    let c = if im_logo {
                        crate::banner::SCHRIFTZUG[zy - self.logo_oben]
                            .chars()
                            .nth(sx - self.einzug)
                            .unwrap_or(' ')
                    } else if self.punkte[zy][sx] != 0 {
                        char::from_u32(0x2800 + self.punkte[zy][sx] as u32).unwrap_or(' ')
                    } else {
                        ' '
                    };
                    z.push(c);
                }
                z.trim_end().to_string()
            })
            .collect()
    }
}

/// Ein Punkt im Abstand `r` und unter dem Winkel `grad` um `(mx, my)`.
fn polar((mx, my): (i32, i32), r: i32, grad: i32) -> (i32, i32) {
    (mx + teile(r * kosinus(grad), EINS), my + teile(r * sinus(grad), EINS))
}

/// Ein regelmaessiges Vieleck; zurueck kommen seine Ecken.
fn vieleck(lw: &mut Leinwand, mitte: (i32, i32), r: i32, ecken: i32, grad: i32) -> Vec<(i32, i32)> {
    let p: Vec<(i32, i32)> = (0..ecken).map(|i| polar(mitte, r, grad + i * 360 / ecken)).collect();
    for i in 0..p.len() {
        lw.linie(p[i], p[(i + 1) % p.len()]);
    }
    p
}

/// **Ein Wirbel aus Vielecken**, die wachsen und sich dabei drehen.
///
/// Jedes ist um `wachstum` Prozent seiner Groesse groesser und um `drehung`
/// Grad weiter gedreht als das vorige; `richtung` ist 1 oder -1. Gerechnet
/// wird die Groesse in Sechzehnteln, sonst bliebe ein kleines Vieleck beim
/// Wachsen auf derselben ganzen Zahl stehen.
///
/// ⚑ **Er waechst bis `bis`, und das darf weit ueber die Seite hinaus
/// sein**: Die aeusseren Vielecke laufen ueber und unter dem Logo durch die
/// Mitte und treffen dort die der anderen Seite.
struct Wirbel {
    ecken: i32,
    drehung: i32,
    wachstum: i32,
}

impl Wirbel {
    fn zeichnen(&self, lw: &mut Leinwand, mitte: (i32, i32), richtung: i32, bis: i32) {
        let (mut s, mut k) = (3 * 16, 0);
        while s < bis * 16 {
            vieleck(lw, mitte, teile(s, 16), self.ecken, richtung * k * self.drehung);
            s = s * self.wachstum / 100;
            k += 1;
        }
    }
}

const QUADRATE: Wirbel = Wirbel { ecken: 4, drehung: 13, wachstum: 118 };
const SECHSECKE: Wirbel = Wirbel { ecken: 6, drehung: 9, wachstum: 118 };
const DREIECKE: Wirbel = Wirbel { ecken: 3, drehung: 11, wachstum: 114 };

/// Die Mitten der beiden Seitenfiguren, bei 13 Prozent der Breite vom
/// Rand, mit ihrer Drehrichtung: links rechtsherum, rechts linksherum.
fn seiten(lw: &Leinwand) -> [(i32, (i32, i32)); 2] {
    let x = lw.b() * 13 / 100;
    [(1, (x, lw.my)), (-1, (lw.b() - x, lw.my))]
}

/// Zwei gegenlaeufige Wirbel, die bis ueber die halbe Breite wachsen.
fn seitenwirbel(lw: &mut Leinwand, art: &Wirbel) {
    let bis = lw.b() / 2;
    for (richtung, mitte) in seiten(lw) {
        art.zeichnen(lw, mitte, richtung, bis);
    }
}

/// Die acht Motive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motiv {
    /// Die Blume des Lebens: Kreise auf einem Dreiecksgitter, jeder durch
    /// die Mitten seiner sechs Nachbarn.
    Blume,
    /// Zwei grosse Quadratwirbel, deren aeussere Quadrate ueber und unter
    /// dem Logo zusammenlaufen.
    Quadratwirbel,
    /// Das Sterntetraeder-Gitter: Linien in drei Richtungen, in jedem
    /// Knoten ein Kreis.
    Sterngitter,
    /// Wie der Quadratwirbel, mit Sechsecken.
    Hexagonwirbel,
    /// Wie der Quadratwirbel, mit Dreiecken.
    Dreieckwirbel,
    /// Kreise auf einem Quadratgitter, jeder durch die Mitten seiner vier
    /// Nachbarn.
    Saatgitter,
    /// Das Kagome-Gitter: drei Linienscharen, gegeneinander um eine halbe
    /// Masche versetzt, und dazwischen lauter Hexagramme.
    Merkaba,
    /// Zwei logarithmische Spiralen mit je vier Armen, die mit jeder
    /// Vierteldrehung um den Goldenen Schnitt wachsen.
    Goldspirale,
}

impl Motiv {
    pub const ALLE: [Motiv; 8] = [
        Motiv::Blume,
        Motiv::Quadratwirbel,
        Motiv::Sterngitter,
        Motiv::Hexagonwirbel,
        Motiv::Dreieckwirbel,
        Motiv::Saatgitter,
        Motiv::Merkaba,
        Motiv::Goldspirale,
    ];

    /// Der Name, unter dem `MYL_MOTIV` es festlegt.
    pub const fn name(self) -> &'static str {
        match self {
            Motiv::Blume => "blume",
            Motiv::Quadratwirbel => "quadratwirbel",
            Motiv::Sterngitter => "sterngitter",
            Motiv::Hexagonwirbel => "hexagonwirbel",
            Motiv::Dreieckwirbel => "dreieckwirbel",
            Motiv::Saatgitter => "saatgitter",
            Motiv::Merkaba => "merkaba",
            Motiv::Goldspirale => "goldspirale",
        }
    }

    pub fn aus_name(name: &str) -> Option<Motiv> {
        Motiv::ALLE.into_iter().find(|m| m.name() == name.trim().to_lowercase())
    }

    /// Zeichnet das Motiv auf die Flaeche.
    pub fn zeichnen(self, lw: &mut Leinwand) {
        match self {
            Motiv::Blume => blume(lw),
            Motiv::Quadratwirbel => seitenwirbel(lw, &QUADRATE),
            Motiv::Sterngitter => sterngitter(lw),
            Motiv::Hexagonwirbel => seitenwirbel(lw, &SECHSECKE),
            Motiv::Dreieckwirbel => seitenwirbel(lw, &DREIECKE),
            Motiv::Saatgitter => saatgitter(lw),
            Motiv::Merkaba => merkaba(lw),
            Motiv::Goldspirale => goldspirale(lw),
        }
    }
}

fn blume(lw: &mut Leinwand) {
    let r = 14;
    let dy = teile(r * 866, 1000);
    let erste = -(lw.my / dy) - 2;
    for j in erste..erste + lw.h() / dy + 5 {
        let y = lw.my + j * dy;
        let versatz = if j % 2 != 0 { r / 2 } else { 0 };
        for i in -2..lw.b() / r + 3 {
            lw.kreis((i * r + versatz, y), r);
        }
    }
}

fn sterngitter(lw: &mut Leinwand) {
    let a = 24;
    let h = teile(a * 866, 1000);
    let (b, hoch, mx, my) = (lw.b(), lw.h(), lw.mx, lw.my);
    let zeilen = -(my / h) - 1..(hoch - my) / h + 2;
    for k in zeilen.clone() {
        lw.linie((0, my + k * h), (b - 1, my + k * h));
    }
    let lauf = teile((hoch + 2 * h) * 577, 1000);
    for k in -(hoch / a) - 3..b / a + 3 {
        let x0 = mx % a + k * a;
        lw.linie((x0, -h), (x0 + lauf, hoch + h));
        lw.linie((x0, hoch + h), (x0 + lauf, -h));
    }
    for k in zeilen {
        let y = my + k * h;
        let versatz = if k % 2 != 0 { a / 2 } else { 0 };
        for i in -1..b / a + 2 {
            lw.kreis((mx % a + i * a + versatz, y), a / 2);
        }
    }
}

fn saatgitter(lw: &mut Leinwand) {
    let r = 16;
    let (b, hoch, mx, my) = (lw.b(), lw.h(), lw.mx, lw.my);
    for j in -(my / r) - 1..(hoch - my) / r + 2 {
        for i in -1..b / r + 2 {
            lw.kreis((mx % r + i * r, my + j * r), r);
        }
    }
}

fn merkaba(lw: &mut Leinwand) {
    let a = 28;
    let h = teile(a * 866, 1000);
    let (b, hoch, mx, my) = (lw.b(), lw.h(), lw.mx, lw.my);
    for k in -(my / h) - 2..(hoch - my) / h + 3 {
        let y = my + k * h + h / 2;
        lw.linie((0, y), (b - 1, y));
    }
    let lauf = teile((hoch + 2 * h) * 577, 1000);
    for k in -(hoch / a) - 3..b / a + 4 {
        let x0 = mx % a + k * a;
        lw.linie((x0, -h), (x0 + lauf, hoch + h));
        // ⚑ Die dritte Schar um eine halbe Masche versetzt: Erst damit
        // laufen die drei Scharen nicht durch gemeinsame Punkte, und aus
        // Dreiecken werden Hexagramme.
        let x1 = x0 + a / 2;
        lw.linie((x1, hoch + h), (x1 + lauf, -h));
    }
}

/// ⚑ **Die Arme wachsen je 4 Grad um 2,16 Prozent**, also je
/// Vierteldrehung um rund 1,618: den Goldenen Schnitt. Sie laufen bis ueber
/// die halbe Breite, also ueber und unter dem Logo durch die Mitte.
fn goldspirale(lw: &mut Leinwand) {
    let (b, hoch) = (lw.b(), lw.h());
    for (richtung, mitte) in seiten(lw) {
        for arm in 0..4 {
            let (mut r, mut grad, mut vorher) = (16 * 16, arm * 90, None);
            while r < b / 2 * 16 {
                let p = polar(mitte, teile(r, 16), richtung * grad);
                if let Some(v) = vorher {
                    lw.linie(v, p);
                }
                vorher = Some(p);
                grad += 4;
                r = r * 10216 / 10000;
            }
        }
        lw.kreis(mitte, hoch * 45 / 100);
    }
}

static SITZUNG: OnceLock<Motiv> = OnceLock::new();

/// **Das Motiv dieser Sitzung**: festgelegt mit `MYL_MOTIV`, sonst
/// gewuerfelt, und zwar einmal.
///
/// ⚑ `OnceLock` und nicht ein Wurf je Aufruf: Das Logo wird in einer
/// Sitzung oft neu gedruckt, und jeder Druck muss dasselbe Bild zeigen,
/// sonst stuende nach jeder Modellwahl ein anderes Programm da.
pub fn sitzungsmotiv() -> Motiv {
    *SITZUNG.get_or_init(|| {
        std::env::var("MYL_MOTIV")
            .ok()
            .and_then(|n| Motiv::aus_name(&n))
            .unwrap_or_else(|| Motiv::ALLE[crate::animation::Zufall::neu().bis(Motiv::ALLE.len())])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Tafel beschreibt einen Kreis: in allen vier Vierteln das
    /// richtige Vorzeichen, und Sinus und Kosinus ergaenzen sich.
    #[test]
    fn die_sinustafel_beschreibt_einen_kreis() {
        assert_eq!((sinus(0), sinus(90), sinus(180), sinus(270)), (0, EINS, 0, -EINS));
        assert_eq!(kosinus(0), EINS);
        for g in -720..720 {
            let r2 = sinus(g) as i64 * sinus(g) as i64 + kosinus(g) as i64 * kosinus(g) as i64;
            let soll = EINS as i64 * EINS as i64;
            assert!((r2 - soll).abs() < 9000, "{g} Grad: {r2}");
            assert_eq!(sinus(g), sinus(g + 360));
        }
    }

    /// Gerundet wird zur naechsten Zahl und fuer beide Vorzeichen gleich,
    /// sonst stuende eine Figur links um einen Punkt anders als ihr
    /// Spiegelbild rechts. Nur die genaue Haelfte geht in beiden Faellen
    /// nach oben.
    #[test]
    fn gerundet_wird_symmetrisch() {
        for n in 1..2000 {
            if (2 * n) % 32 == 16 {
                continue;
            }
            assert_eq!(teile(-n, 16), -teile(n, 16), "bei {n}/16");
        }
        assert_eq!((teile(24, 16), teile(23, 16), teile(-23, 16)), (2, 1, -1));
    }

    /// Ein Kreis aus der Flaeche bleibt rund: jeder gesetzte Punkt liegt
    /// hoechstens einen Punkt neben dem Sollradius.
    #[test]
    fn ein_kreis_bleibt_rund() {
        let mut lw = Leinwand::neu(200, 30, 20);
        let (mx, my, r) = (40, 40, 18);
        lw.kreis((mx, my), r);
        let mut gesetzt = 0;
        for y in 0..lw.h() {
            for x in 0..lw.b() {
                let (sx, zy) = ((x / 2) as usize, (y / 4) as usize);
                if lw.punkte[zy][sx] & BIT[(y % 4) as usize][(x % 2) as usize] != 0 {
                    gesetzt += 1;
                    let d2 = (x - mx).pow(2) + (y - my).pow(2);
                    assert!((d2 - r * r).abs() <= 2 * r + 1, "Punkt {x},{y} liegt neben dem Kreis");
                }
            }
        }
        assert!(gesetzt > 4 * r, "nur {gesetzt} Punkte");
    }

    /// Jedes Motiv hat einen Namen, und der Name fuehrt zurueck zu ihm.
    #[test]
    fn jedes_motiv_ist_mit_seinem_namen_waehlbar() {
        for m in Motiv::ALLE {
            assert_eq!(Motiv::aus_name(m.name()), Some(m));
            assert_eq!(Motiv::aus_name(&m.name().to_uppercase()), Some(m));
        }
        assert_eq!(Motiv::aus_name("gibtsnicht"), None);
    }

    /// Das Motiv gilt fuer die ganze Sitzung.
    #[test]
    fn das_motiv_bleibt_ueber_die_sitzung() {
        let erstes = sitzungsmotiv();
        for _ in 0..20 {
            assert_eq!(sitzungsmotiv(), erstes);
        }
    }
}
