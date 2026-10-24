//! **Die Historienwurzel**: ein Merkle Mountain Range über die Hashes aller
//! Blöcke einer Kette (Stufe A1 der Archivknoten, 2026-10-23).
//!
//! # ⚑ Wozu
//!
//! Seit Knoten ihre Kette verdichten, hat nicht mehr jeder jeden alten Block.
//! Wer einen alten Block von einer fremden Gegenstelle bekommt, muss ihm
//! glauben können, **ohne die Kette ab Genesis nachzurechnen**: mit einem
//! kurzen Beweis gegen eine Wurzel, die er ohnehin hat. Dieselben Beweise
//! tragen die Stichproben, mit denen Archivknoten zeigen, dass sie die
//! Geschichte wirklich halten.
//!
//! # ⚑ Aufbau
//!
//! Blätter in der Reihenfolge der Kette; die Gipfel sind vollständige
//! Binärbäume, einer je gesetztem Bit der Blattzahl, von links nach rechts
//! absteigend. Anhängen kostet im Mittel zwei Hashes; der Zustand ist die
//! Blattzahl und höchstens 64 Gipfel. Die Wurzel bindet Gipfel **und**
//! Blattzahl.
//!
//! **Domänentrennung**, eigene Präfixe statt der Bytes von [`crate::merkle`]:
//!
//! ```text
//! blatt  = sha256("myl-historie-blatt"  ‖ blockhash)
//! knoten = sha256("myl-historie-knoten" ‖ links ‖ rechts)
//! wurzel = falte(sha256("myl-historie-wurzel" ‖ anzahl_le), gipfel…)
//!          mit falte(a, g) = sha256("myl-historie-gipfel" ‖ a ‖ g)
//! ```
//!
//! Kein Blatt kann als innerer Knoten durchgehen und keine Historie einer
//! Länge als eine andere.
//!
//! # ⚑ Beweise ohne den ganzen Baum
//!
//! Einen Beweis erstellt nur, wer die inneren Knoten hat. Alle zu halten
//! kostete bei zwölf Millionen Blöcken rund 770 MB. [`Historienbaum`] hält
//! deshalb nur die Knoten ab Stufe [`GRUPPENSTUFE`] (je 1024 Blätter einen)
//! und rechnet die unteren Stufen für einen Beweis aus den 1024 Blättern der
//! betroffenen Gruppe nach, die der Aufrufer aus seinen Blöcken liefert.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::Hash;

/// Ab dieser Stufe hält [`Historienbaum`] die Knoten; darunter rechnet er sie
/// aus den Blättern einer Gruppe von `2^GRUPPENSTUFE` nach.
pub const GRUPPENSTUFE: u32 = 10;

/// Das Blatt eines Blockhashes.
pub fn blatt(blockhash: &Hash) -> Hash {
    let mut e = Vec::with_capacity(18 + 32);
    e.extend_from_slice(b"myl-historie-blatt");
    e.extend_from_slice(blockhash.as_bytes());
    Hash::sha256(&e)
}

fn knoten(links: &Hash, rechts: &Hash) -> Hash {
    let mut e = Vec::with_capacity(19 + 64);
    e.extend_from_slice(b"myl-historie-knoten");
    e.extend_from_slice(links.as_bytes());
    e.extend_from_slice(rechts.as_bytes());
    Hash::sha256(&e)
}

fn buendeln(gipfel: &[Hash], anzahl: u64) -> Hash {
    let mut e = Vec::with_capacity(19 + 8);
    e.extend_from_slice(b"myl-historie-wurzel");
    e.extend_from_slice(&anzahl.to_le_bytes());
    let mut a = Hash::sha256(&e);
    for g in gipfel {
        let mut e = Vec::with_capacity(19 + 64);
        e.extend_from_slice(b"myl-historie-gipfel");
        e.extend_from_slice(a.as_bytes());
        e.extend_from_slice(g.as_bytes());
        a = Hash::sha256(&e);
    }
    a
}

/// **Wo ein Blatt liegt**: Nummer des Gipfels, seine Höhe und die Stelle im
/// Gipfel. `None` hinter dem Ende.
fn lage(index: u64, anzahl: u64) -> Option<(usize, u32, u64)> {
    if index >= anzahl {
        return None;
    }
    let mut anfang = 0u64;
    let mut nummer = 0usize;
    for hoehe in (0..64u32).rev() {
        if anzahl & (1u64 << hoehe) == 0 {
            continue;
        }
        let groesse = 1u64 << hoehe;
        if index < anfang + groesse {
            return Some((nummer, hoehe, index - anfang));
        }
        anfang += groesse;
        nummer += 1;
    }
    None
}

/// **Der Akkumulator im Zustand**: Blattzahl und Gipfel.
#[derive(Debug, Clone, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Historie {
    anzahl: u64,
    gipfel: Vec<Hash>,
}

impl Historie {
    /// Hängt den Hash eines Blocks an.
    pub fn anhaengen(&mut self, blockhash: &Hash) {
        let mut h = blatt(blockhash);
        let mut n = self.anzahl;
        while n & 1 == 1 {
            let links = self.gipfel.pop().expect("ein gesetztes Bit hat einen Gipfel");
            h = knoten(&links, &h);
            n >>= 1;
        }
        self.gipfel.push(h);
        self.anzahl += 1;
    }

    /// Wie viele Blöcke angehängt sind.
    pub fn anzahl(&self) -> u64 {
        self.anzahl
    }

    /// Die Gipfel, von links nach rechts.
    pub fn gipfel(&self) -> &[Hash] {
        &self.gipfel
    }

    /// Die Wurzel: Gipfel und Blattzahl, gebündelt.
    pub fn wurzel(&self) -> Hash {
        buendeln(&self.gipfel, self.anzahl)
    }
}

/// **Ein Beweis**, dass der Block mit einem Hash an Stelle `index` einer
/// Historie mit `anzahl` Blättern steht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Beweis {
    pub index: u64,
    pub anzahl: u64,
    /// Die Geschwister von unten nach oben, bis zum eigenen Gipfel.
    pub pfad: Vec<Hash>,
    /// Alle Gipfel der Historie, von links nach rechts.
    pub gipfel: Vec<Hash>,
}

impl Beweis {
    /// Steht `blockhash` an Stelle `index` der Historie mit dieser Wurzel?
    pub fn pruefen(&self, wurzel: &Hash, blockhash: &Hash) -> bool {
        let Some((nummer, hoehe, stelle)) = lage(self.index, self.anzahl) else {
            return false;
        };
        if self.pfad.len() != hoehe as usize || self.gipfel.len() != self.anzahl.count_ones() as usize {
            return false;
        }
        let mut h = blatt(blockhash);
        for (stufe, geschwister) in self.pfad.iter().enumerate() {
            h = if (stelle >> stufe) & 1 == 0 { knoten(&h, geschwister) } else { knoten(geschwister, &h) };
        }
        self.gipfel[nummer] == h && buendeln(&self.gipfel, self.anzahl) == *wurzel
    }
}

/// Die Wurzel eines vollständigen Baums über `blaetter` (Länge eine
/// Zweierpotenz) und alle seine Stufen, von den Blättern an.
fn stufen(blaetter: &[Hash]) -> Vec<Vec<Hash>> {
    let mut alle = vec![blaetter.to_vec()];
    while alle.last().expect("nicht leer").len() > 1 {
        let unten = alle.last().expect("nicht leer");
        alle.push(unten.as_chunks::<2>().0.iter().map(|[l, r]| knoten(l, r)).collect());
    }
    alle
}

/// **Ein Beweis aus allen Blättern**, die Referenz: kostet so viel wie die
/// Historie lang ist. Für Proben und kurze Ketten.
pub fn beweis_aus_blaettern(blaetter: &[Hash], index: u64) -> Option<Beweis> {
    let anzahl = blaetter.len() as u64;
    let (nummer, hoehe, stelle) = lage(index, anzahl)?;
    let mut gipfel = Vec::new();
    let mut anfang = 0usize;
    let mut pfad = Vec::new();
    for h in (0..64u32).rev() {
        if anzahl & (1u64 << h) == 0 {
            continue;
        }
        let groesse = 1usize << h;
        let st = stufen(&blaetter[anfang..anfang + groesse]);
        if gipfel.len() == nummer {
            for s in 0..hoehe as usize {
                pfad.push(st[s][((stelle >> s) ^ 1) as usize]);
            }
        }
        gipfel.push(st[h as usize][0]);
        anfang += groesse;
    }
    Some(Beweis { index, anzahl, pfad, gipfel })
}

/// **Der Baum eines Archivknotens**: die Historie samt den Knoten ab
/// [`GRUPPENSTUFE`], dazu die Blätter der angefangenen Gruppe.
#[derive(Debug, Clone)]
pub struct Historienbaum {
    historie: Historie,
    stufe: u32,
    /// `obere[j]`: die Knoten der Stufe `stufe + j`, von links.
    obere: Vec<Vec<Hash>>,
    /// Die Blätter der angefangenen Gruppe.
    rest: Vec<Hash>,
}

impl Default for Historienbaum {
    fn default() -> Self {
        Self::mit_stufe(GRUPPENSTUFE)
    }
}

impl Historienbaum {
    /// Ein leerer Baum, der ab `stufe` Knoten hält. Für Proben mit kleinen
    /// Gruppen; im Betrieb [`GRUPPENSTUFE`].
    pub fn mit_stufe(stufe: u32) -> Self {
        Self { historie: Historie::default(), stufe, obere: Vec::new(), rest: Vec::new() }
    }

    /// Hängt einen Blockhash an.
    pub fn anhaengen(&mut self, blockhash: &Hash) {
        self.historie.anhaengen(blockhash);
        self.rest.push(blatt(blockhash));
        if self.rest.len() < (1usize << self.stufe) {
            return;
        }
        let gruppe = std::mem::take(&mut self.rest);
        let mut h = stufen(&gruppe).pop().expect("nicht leer")[0];
        let mut j = 0usize;
        loop {
            if self.obere.len() == j {
                self.obere.push(Vec::new());
            }
            self.obere[j].push(h);
            if self.obere[j].len() % 2 == 1 {
                break;
            }
            let n = self.obere[j].len();
            h = knoten(&self.obere[j][n - 2], &self.obere[j][n - 1]);
            j += 1;
        }
    }

    /// Die Historie, wie sie im Zustand steht.
    pub fn historie(&self) -> &Historie {
        &self.historie
    }

    /// **Ein Beweis für Stelle `index`** gegen die heutige Historie.
    /// `gruppe(g)` liefert die `2^stufe` Blatt-Hashes der Gruppe `g` (aus
    /// den Blöcken des Aufrufers); gebraucht wird sie nur für Blätter in
    /// vollständigen Gruppen.
    pub fn beweis(&self, index: u64, gruppe: impl FnMut(u64) -> Option<Vec<Hash>>) -> Option<Beweis> {
        self.beweis_bei(index, self.historie.anzahl, gruppe)
    }

    /// **Ein Beweis gegen eine frühere Länge** `anzahl` (Archivknoten,
    /// Stufe A4): Wer die Geschichte nachlädt, prüft gegen die Historie
    /// seiner Basis, und die ist kürzer als die des Liefernden.
    ///
    /// ⚑ Das geht ohne gespeicherte alte Gipfel: Jeder Gipfel und jedes
    /// Geschwister einer früheren Länge ist ein vollständiger Teilbaum, und
    /// die stehen alle im Baum, ab [`GRUPPENSTUFE`] gespeichert, darunter
    /// aus den Blättern einer Gruppe gerechnet.
    pub fn beweis_bei(
        &self,
        index: u64,
        anzahl: u64,
        mut gruppe: impl FnMut(u64) -> Option<Vec<Hash>>,
    ) -> Option<Beweis> {
        if anzahl > self.historie.anzahl {
            return None;
        }
        let (_, hoehe, _) = lage(index, anzahl)?;
        let mut zwischen: Vec<(u64, Vec<Hash>)> = Vec::new();
        let mut pfad = Vec::with_capacity(hoehe as usize);
        for s in 0..hoehe {
            pfad.push(self.knoten_bei(s, (index >> s) ^ 1, &mut gruppe, &mut zwischen)?);
        }
        let mut gipfel = Vec::with_capacity(anzahl.count_ones() as usize);
        let mut anfang = 0u64;
        for h in (0..64u32).rev() {
            if anzahl & (1u64 << h) != 0 {
                gipfel.push(self.knoten_bei(h, anfang >> h, &mut gruppe, &mut zwischen)?);
                anfang += 1u64 << h;
            }
        }
        Some(Beweis { index, anzahl, pfad, gipfel })
    }

    /// Der Knoten der Stufe `s` an Stelle `p`, über einem vollständigen
    /// Teilbaum. Gruppenblätter werden je Aufruf höchstens einmal geholt.
    fn knoten_bei(
        &self,
        s: u32,
        p: u64,
        gruppe: &mut impl FnMut(u64) -> Option<Vec<Hash>>,
        zwischen: &mut Vec<(u64, Vec<Hash>)>,
    ) -> Option<Hash> {
        if s >= self.stufe {
            return self.obere.get((s - self.stufe) as usize)?.get(usize::try_from(p).ok()?).copied();
        }
        let erstes = p.checked_shl(s)?;
        let g = erstes >> self.stufe;
        let groesse = 1u64 << self.stufe;
        if !zwischen.iter().any(|(x, _)| *x == g) {
            let blaetter = if (g + 1) * groesse <= self.historie.anzahl {
                let b = gruppe(g)?;
                // Die geholte Gruppe muss zu dem passen, was der Baum hält.
                if b.len() as u64 != groesse
                    || self.obere.first()?.get(usize::try_from(g).ok()?) != Some(&stufen(&b).pop()?[0])
                {
                    return None;
                }
                b
            } else {
                self.rest.clone()
            };
            zwischen.push((g, blaetter));
        }
        let (_, blaetter) = zwischen.iter().find(|(x, _)| *x == g)?;
        let von = (erstes - g * groesse) as usize;
        let bis = von.checked_add(1usize << s)?;
        Some(stufen(blaetter.get(von..bis)?).pop()?[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bloecke(n: u64) -> Vec<Hash> {
        (0..n).map(|i| Hash::sha256(&i.to_le_bytes())).collect()
    }

    #[test]
    fn die_wurzel_haengt_an_jedem_blatt_und_an_der_zahl() {
        let b = bloecke(100);
        let mut h = Historie::default();
        let mut wurzeln = std::collections::BTreeSet::new();
        assert!(wurzeln.insert(h.wurzel()));
        for x in &b {
            h.anhaengen(x);
            assert!(wurzeln.insert(h.wurzel()), "zwei Längen mit derselben Wurzel");
        }
        assert_eq!(h.anzahl(), 100);
        assert_eq!(h.gipfel().len(), 100u64.count_ones() as usize);
        let mut anders = Historie::default();
        for (i, x) in b.iter().enumerate() {
            anders.anhaengen(if i == 37 { &b[38] } else { x });
        }
        assert_ne!(h.wurzel(), anders.wurzel());
    }

    /// ⚑ **Jede Stelle jeder Länge bis 300** lässt sich beweisen, und die
    /// Gipfel der Referenz sind die der Historie.
    #[test]
    fn jede_stelle_jeder_laenge_ist_beweisbar() {
        let b = bloecke(300);
        let mut h = Historie::default();
        for n in 1..=300usize {
            h.anhaengen(&b[n - 1]);
            let blaetter: Vec<Hash> = b[..n].iter().map(blatt).collect();
            for i in 0..n as u64 {
                let bw = beweis_aus_blaettern(&blaetter, i).expect("Beweis");
                assert_eq!(bw.gipfel, h.gipfel());
                assert!(bw.pruefen(&h.wurzel(), &b[i as usize]), "n {n}, i {i}");
            }
        }
    }

    /// ⛔️ **Ein falscher Beweis fällt durch**: anderer Block, andere Stelle,
    /// verfälschtes Geschwister, andere Länge, ein innerer Knoten als Blatt.
    #[test]
    fn ein_falscher_beweis_faellt_durch() {
        let b = bloecke(77);
        let mut h = Historie::default();
        b.iter().for_each(|x| h.anhaengen(x));
        let blaetter: Vec<Hash> = b.iter().map(blatt).collect();
        let w = h.wurzel();
        let bw = beweis_aus_blaettern(&blaetter, 40).expect("Beweis");
        assert!(bw.pruefen(&w, &b[40]));
        assert!(!bw.pruefen(&w, &b[41]), "anderer Block");
        let mut x = bw.clone();
        x.index = 41;
        assert!(!x.pruefen(&w, &b[40]), "andere Stelle");
        let mut x = bw.clone();
        x.pfad[2] = Hash::sha256(b"falsch");
        assert!(!x.pruefen(&w, &b[40]), "verfälschtes Geschwister");
        let mut x = bw.clone();
        x.anzahl = 78;
        assert!(!x.pruefen(&w, &b[40]), "andere Länge");
        let mut x = bw.clone();
        x.index = 1000;
        assert!(!x.pruefen(&w, &b[40]), "hinter dem Ende");
    }

    /// ⚑ **Domänentrennung**: Eine Historie aus einem Blatt, dessen Hash der
    /// innere Knoten einer anderen ist, hat einen anderen Gipfel. Ohne die
    /// Präfixe wären beide Gipfel gleich, und nur die Blattzahl in der Wurzel
    /// unterschiede sie noch.
    #[test]
    fn ein_innerer_knoten_ist_kein_blatt() {
        let b = bloecke(2);
        let mut zwei = Historie::default();
        b.iter().for_each(|x| zwei.anhaengen(x));
        let innen = knoten(&blatt(&b[0]), &blatt(&b[1]));
        let mut eins = Historie::default();
        eins.anhaengen(&innen);
        assert_ne!(zwei.gipfel(), eins.gipfel());
    }

    /// ⚑ **Der Baum eines Archivknotens beweist dasselbe wie die Referenz**,
    /// mit kleinen Gruppen, damit viele Gipfel über und unter der Gruppenstufe
    /// vorkommen; und mit der echten Gruppenstufe über drei Gruppen.
    #[test]
    fn der_archivbaum_beweist_wie_die_referenz() {
        for (stufe, n) in [(2u32, 300u64), (3, 1000), (GRUPPENSTUFE, 3 * 1024 + 517), (GRUPPENSTUFE, 100_000)] {
            let b = bloecke(n);
            let blaetter: Vec<Hash> = b.iter().map(blatt).collect();
            let mut baum = Historienbaum::mit_stufe(stufe);
            b.iter().for_each(|x| baum.anhaengen(x));
            let w = baum.historie().wurzel();
            let groesse = 1u64 << stufe;
            let schritt = if n > 10_000 { 997 } else if n > 2000 { 97 } else { 1 };
            for i in (0..n).step_by(schritt) {
                let bw = baum
                    .beweis(i, |g| Some(blaetter[(g * groesse) as usize..((g + 1) * groesse) as usize].to_vec()))
                    .expect("Beweis");
                assert_eq!(bw, beweis_aus_blaettern(&blaetter, i).expect("Referenz"), "Stufe {stufe}, i {i}");
                assert!(bw.pruefen(&w, &b[i as usize]));
            }
        }
    }

    /// Eine falsche Gruppe vom Aufrufer ergibt keinen Beweis.
    #[test]
    fn eine_falsche_gruppe_ergibt_keinen_beweis() {
        let b = bloecke(64);
        let mut baum = Historienbaum::mit_stufe(3);
        b.iter().for_each(|x| baum.anhaengen(x));
        assert!(baum.beweis(10, |_| Some(vec![Hash::sha256(b"x"); 8])).is_none());
    }

    /// ⚑ **Beweise gegen jede frühere Länge** (Stufe A4) gleichen denen der
    /// Referenz über genau so viele Blätter, für jede Stelle.
    #[test]
    fn beweise_gegen_fruehere_laengen_gleichen_der_referenz() {
        let b = bloecke(150);
        let blaetter: Vec<Hash> = b.iter().map(blatt).collect();
        let mut baum = Historienbaum::mit_stufe(3);
        b.iter().for_each(|x| baum.anhaengen(x));
        let gruppe = |g: u64| blaetter.get((g * 8) as usize..((g + 1) * 8) as usize).map(<[Hash]>::to_vec);
        for anzahl in 1..=150u64 {
            for i in 0..anzahl {
                let bw = baum.beweis_bei(i, anzahl, gruppe).expect("Beweis");
                assert_eq!(Some(&bw), beweis_aus_blaettern(&blaetter[..anzahl as usize], i).as_ref(), "n {anzahl}, i {i}");
            }
        }
        assert!(baum.beweis_bei(0, 151, gruppe).is_none(), "länger als der Baum");
        // Eine verfälschte Gruppe liefert keinen Beweis.
        let falsch = |_g: u64| Some(vec![Hash::sha256(b"x"); 8]);
        assert!(baum.beweis_bei(3, 100, falsch).is_none());
    }
}
