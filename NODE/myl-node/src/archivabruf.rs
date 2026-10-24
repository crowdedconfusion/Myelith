//! **Die Geschichte nachladen, mit Beweisen** (Archivknoten, Stufe A4,
//! 2026-10-23).
//!
//! Eine neu gezogene Kopie ([`crate::archiv`]) lebt ab ihrer Basis und muss
//! die Blöcke davor holen. Bis NODE 0.57.0 lief das über die
//! Hintergrundprüfung: eine zweite Kette ab Genesis, die alles nachrechnet
//! und erst am Ziel urteilt. Eine Gegenstelle, die Müll liefert, fiel damit
//! erst nach dem ganzen Weg auf, und jeder Block kostete eine Rechnung.
//!
//! # ⚑ Wie hier geprüft wird
//!
//! Die Basis trägt im Zustand die **Historie** ([`myl_types::historie`]):
//! die Hashes aller Blöcke davor in einem Merkle Mountain Range. Jede
//! Lieferung `ab..=bis` kommt mit einem Beweis, dass der Hash von Block
//! `bis` an Stelle `bis` dieser Historie steht. Ist er bewiesen, steht auch
//! sein Vorgängerhash fest, also Block `bis − 1`, und so rückwärts bis `ab`.
//!
//! - **Jede Lieferung wird für sich geprüft**, sofort und ohne Rechnung,
//!   auch wenn sie vor einer früheren ankommt.
//! - **Müll fällt bei der ersten Lieferung auf**, nicht am Ziel; die
//!   Gegenstelle wird gesperrt, und gefragt wird eine andere.
//! - **Wer liefert, ist gleichgültig.** Der Beweis hängt an der eigenen
//!   Basis, nicht am Vertrauen in den Liefernden.
//!
//! Geschrieben wird in Höhenfolge in eine eigene Datei; was vor einer
//! Lücke ankommt, wartet geprüft im Speicher, höchstens so viel, wie
//! Lieferungen zugleich laufen.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use myl_consensus::block::Block;
use myl_types::hash::Hash;
use myl_types::historie::{Beweis, Historie};

use crate::speicher::Kettenspeicher;

/// Warum eine Lieferung abgewiesen wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abweisung {
    Leer,
    Hoehenfolge,
    AusserhalbDesBereichs,
    BeweisFuerAnderenBlock,
    BeweisFalsch,
    Verkettung,
    Schreiben(String),
}

impl std::fmt::Display for Abweisung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Abweisung::Leer => write!(f, "leere Lieferung"),
            Abweisung::Hoehenfolge => write!(f, "Höhen nicht lückenlos aufsteigend"),
            Abweisung::AusserhalbDesBereichs => write!(f, "ausserhalb der Geschichte vor der Basis"),
            Abweisung::BeweisFuerAnderenBlock => write!(f, "Beweis für eine andere Stelle oder Länge"),
            Abweisung::BeweisFalsch => write!(f, "Beweis trifft die Historie der Basis nicht"),
            Abweisung::Verkettung => write!(f, "Vorgängerhash passt nicht"),
            Abweisung::Schreiben(e) => write!(f, "Schreiben: {e}"),
        }
    }
}

/// **Ein laufendes Nachladen** der Blöcke `1..basis` vor die Basis.
pub struct Archivabruf {
    basisblock: Block,
    anzahl: u64,
    wurzel: Hash,
    speicher: Kettenspeicher,
    pfad: PathBuf,
    geschrieben: u64,
    letzter: Hash,
    wartend: BTreeMap<u64, Vec<Block>>,
}

impl Archivabruf {
    /// Beginnt ein Nachladen vor `basisblock`, dessen Zustand `historie`
    /// trägt. Eine liegengebliebene Datei eines abgebrochenen Versuchs wird
    /// verworfen.
    pub fn neu(pfad: &Path, startwert: Hash, basisblock: Block, historie: &Historie) -> Result<Self, String> {
        if historie.anzahl() != basisblock.header.height || historie.anzahl() == 0 {
            return Err(format!(
                "die Historie der Basis zählt {} Blöcke, die Basis steht auf {}",
                historie.anzahl(),
                basisblock.header.height
            ));
        }
        let _ = std::fs::remove_file(pfad);
        let (speicher, _) = Kettenspeicher::oeffnen(pfad, startwert).map_err(|e| e.to_string())?;
        Ok(Self {
            anzahl: historie.anzahl(),
            wurzel: historie.wurzel(),
            basisblock,
            speicher,
            pfad: pfad.to_path_buf(),
            geschrieben: 0,
            letzter: startwert,
            wartend: BTreeMap::new(),
        })
    }

    /// Die Länge der Historie, gegen die bewiesen wird; zugleich die Höhe
    /// der Basis.
    pub fn anzahl(&self) -> u64 {
        self.anzahl
    }

    /// Die letzte zu holende Höhe: die vor der Basis.
    pub fn ziel(&self) -> u64 {
        self.anzahl - 1
    }

    /// Bis wohin lückenlos geschrieben ist.
    pub fn geschrieben(&self) -> u64 {
        self.geschrieben
    }

    /// Ob Höhe `h` schon geprüft wartet.
    pub fn wartet(&self, h: u64) -> bool {
        self.wartend
            .range(..=h)
            .next_back()
            .is_some_and(|(ab, b)| h < ab + b.len() as u64)
    }

    /// Wie viele Blöcke geprüft im Speicher warten.
    pub fn wartende_bloecke(&self) -> usize {
        self.wartend.values().map(Vec::len).sum()
    }

    /// **Eine Lieferung prüfen und aufnehmen.** Gibt zurück, wie weit danach
    /// lückenlos geschrieben ist.
    pub fn lieferung(&mut self, bloecke: Vec<Block>, beweis: &Beweis) -> Result<u64, Abweisung> {
        let (Some(erster), Some(letzter)) = (bloecke.first(), bloecke.last()) else {
            return Err(Abweisung::Leer);
        };
        let ab = erster.header.height;
        let bis = letzter.header.height;
        if ab == 0 || bis > self.ziel() {
            return Err(Abweisung::AusserhalbDesBereichs);
        }
        if bis - ab + 1 != bloecke.len() as u64 || bloecke.windows(2).any(|w| w[1].header.height != w[0].header.height + 1) {
            return Err(Abweisung::Hoehenfolge);
        }
        if beweis.index != bis || beweis.anzahl != self.anzahl {
            return Err(Abweisung::BeweisFuerAnderenBlock);
        }
        if !beweis.pruefen(&self.wurzel, &letzter.hash()) {
            return Err(Abweisung::BeweisFalsch);
        }
        // Rückwärts: Der bewiesene Block legt seinen Vorgänger fest.
        if bloecke.windows(2).any(|w| w[1].header.prev_block_hash != w[0].hash()) {
            return Err(Abweisung::Verkettung);
        }
        if bis > self.geschrieben && !self.wartet(ab) {
            self.wartend.insert(ab, bloecke);
        }
        self.einspielen()?;
        Ok(self.geschrieben)
    }

    /// Schreibt, was lückenlos anschliesst.
    fn einspielen(&mut self) -> Result<(), Abweisung> {
        loop {
            let naechste = self.geschrieben + 1;
            let Some((&ab, _)) = self.wartend.range(..=naechste).next_back() else {
                return Ok(());
            };
            let bloecke = self.wartend.remove(&ab).unwrap_or_default();
            if ab + bloecke.len() as u64 <= naechste {
                // Ganz geschrieben; nächster Versuch.
                continue;
            }
            for b in bloecke.into_iter().filter(|b| b.header.height >= naechste) {
                // Bewiesen ist jede Lieferung für sich; dass sie an das
                // Geschriebene anschliesst, folgt daraus und wird trotzdem
                // geprüft, es kostet nichts.
                if b.header.prev_block_hash != self.letzter {
                    return Err(Abweisung::Verkettung);
                }
                self.speicher.anhaengen(&b).map_err(|e| Abweisung::Schreiben(e.to_string()))?;
                self.letzter = b.hash();
                self.geschrieben = b.header.height;
            }
        }
    }

    /// Ob alles bis vor die Basis geschrieben ist.
    pub fn fertig(&self) -> bool {
        self.geschrieben == self.ziel()
    }

    /// **Schliesst ab**: hängt die Basis an und gibt die Datei mit den
    /// Blöcken `1..=basis` zurück, bereit für `Kette::geschichte_einfuegen`.
    pub fn abschliessen(mut self) -> Result<PathBuf, String> {
        if !self.fertig() {
            return Err(format!("erst bis {} von {}", self.geschrieben, self.ziel()));
        }
        if self.basisblock.header.prev_block_hash != self.letzter {
            return Err("die Basis schliesst nicht an die geholte Geschichte an".into());
        }
        self.speicher.anhaengen(&self.basisblock).map_err(|e| e.to_string())?;
        drop(self.speicher);
        Ok(self.pfad)
    }

    /// Verwirft das Nachladen samt Datei.
    pub fn verwerfen(self) {
        let pfad = self.pfad.clone();
        drop(self);
        let _ = std::fs::remove_file(pfad);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kette::Kette;
    use myl_types::historie::{blatt, beweis_aus_blaettern};

    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-archivabruf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    /// Eine Kette bis `n` und die Blattliste ihrer Historie.
    fn kette(n: u64) -> (Kette, Vec<Block>, Vec<Hash>) {
        let mut k = Kette::probestand();
        let mut bloecke = Vec::new();
        let mut blaetter = vec![blatt(&k.netz().startwert_hash())];
        while k.hoehe() < n {
            let b = k.baue_block();
            blaetter.push(blatt(&b.hash()));
            bloecke.push(b);
        }
        (k, bloecke, blaetter)
    }

    /// Die Historie im Zustand nach Block `basis`: Startwert und die
    /// Blöcke davor.
    fn historie_bis(k: &Kette, bloecke: &[Block], basis: u64) -> Historie {
        let mut h = Historie::default();
        std::iter::once(k.netz().startwert_hash())
            .chain(bloecke.iter().map(Block::hash))
            .take(basis as usize)
            .for_each(|x| h.anhaengen(&x));
        h
    }

    fn lieferung(bloecke: &[Block], blaetter: &[Hash], basis: u64, ab: u64, bis: u64) -> (Vec<Block>, Beweis) {
        let teil = bloecke[(ab - 1) as usize..bis as usize].to_vec();
        let bw = beweis_aus_blaettern(&blaetter[..basis as usize], bis).expect("Beweis");
        (teil, bw)
    }

    /// ⚑ **Durcheinander geliefert, in Folge geschrieben**, und am Ende
    /// dieselben Blöcke wie die Kette.
    #[test]
    fn lieferungen_in_beliebiger_folge_ergeben_die_geschichte() {
        let d = ordner("folge");
        let (k, bloecke, blaetter) = kette(300);
        let basis = 250u64;
        let historie = historie_bis(&k, &bloecke, basis);
        let pfad = d.join("a.archiv");
        let mut a = Archivabruf::neu(&pfad, k.netz().startwert_hash(), bloecke[(basis - 1) as usize].clone(), &historie)
            .expect("neu");
        for (ab, bis) in [(101, 200), (201, 249), (1, 100), (50, 120)] {
            let (t, bw) = lieferung(&bloecke, &blaetter, basis, ab, bis);
            a.lieferung(t, &bw).expect("gültige Lieferung");
        }
        assert!(a.fertig());
        let pfad = a.abschliessen().expect("abschliessen");
        let (mut s, _) = Kettenspeicher::oeffnen(&pfad, k.netz().startwert_hash()).expect("öffnen");
        let alle = s.alle_saetze().expect("lesen");
        assert_eq!(alle.len() as u64, basis);
        assert!(alle.iter().zip(&bloecke).all(|(x, y)| x.hash() == y.hash()));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔️ **Müll fällt bei der ersten Lieferung durch**: verfälschter
    /// Block, fremder Beweis, Lücke, Bereich hinter der Basis.
    #[test]
    fn muell_wird_sofort_abgewiesen() {
        let d = ordner("muell");
        let (k, bloecke, blaetter) = kette(120);
        let basis = 100u64;
        let historie = historie_bis(&k, &bloecke, basis);
        let mut a = Archivabruf::neu(&d.join("m"), k.netz().startwert_hash(), bloecke[(basis - 1) as usize].clone(), &historie)
            .expect("neu");

        // Ein Block mit anderem Inhalt in der Mitte: Die Kette rückwärts
        // vom bewiesenen Ende bricht.
        let (mut t, bw) = lieferung(&bloecke, &blaetter, basis, 1, 50);
        t[20].header.timestamp_ms += 1;
        assert_eq!(a.lieferung(t, &bw), Err(Abweisung::Verkettung));
        // Ein falscher letzter Block: Der Beweis trifft nicht.
        let (mut t, bw) = lieferung(&bloecke, &blaetter, basis, 1, 50);
        t[49].header.timestamp_ms += 1;
        assert_eq!(a.lieferung(t, &bw), Err(Abweisung::BeweisFalsch));
        // Ein Beweis gegen eine andere Länge.
        let (t, _) = lieferung(&bloecke, &blaetter, basis, 1, 50);
        let fremd = beweis_aus_blaettern(&blaetter[..110], 50).expect("Beweis");
        assert_eq!(a.lieferung(t, &fremd), Err(Abweisung::BeweisFuerAnderenBlock));
        // Eine Lücke in der Lieferung.
        let (mut t, bw) = lieferung(&bloecke, &blaetter, basis, 1, 50);
        t.remove(10);
        assert_eq!(a.lieferung(t, &bw), Err(Abweisung::Hoehenfolge));
        // Hinter der Basis.
        let (t, _) = lieferung(&bloecke, &blaetter, 110, 100, 105);
        let bw = beweis_aus_blaettern(&blaetter[..basis as usize], 99).expect("Beweis");
        assert_eq!(a.lieferung(t, &bw), Err(Abweisung::AusserhalbDesBereichs));
        assert_eq!(a.geschrieben(), 0, "eine abgewiesene Lieferung hat etwas geschrieben");
        assert_eq!(a.wartende_bloecke(), 0);
        a.verwerfen();
        let _ = std::fs::remove_dir_all(&d);
    }
}
