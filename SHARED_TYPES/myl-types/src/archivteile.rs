//! **Die Splitter der Geschichte** (Archivknoten, Stufe A7, 2026-10-23):
//! Reed-Solomon-Teile je Gruppe von Blöcken, ihre Wurzeln und Beweise.
//!
//! # ⚑ Die Einheit ist die Gruppe der Historie
//!
//! Eine **Gruppe** sind die 1024 Höhen `g · 1024 ..` (Stufe
//! [`crate::historie::GRUPPENSTUFE`]), dieselbe Einteilung wie im Merkle
//! Mountain Range; Höhe 0 ist der Startwert und trägt keinen Block. Jede
//! Gruppe wird für sich codiert: ihre Blöcke als Bytes hintereinander, in
//! [`DATENTEILE`] Daten- und [`PARITAETSTEILE`] Paritätsteile
//! ([`crate::erasure`], Cauchy über GF(2⁸)). Splitter `i` hält Teil `i`
//! jeder Gruppe einer Periode.
//!
//! - **Codieren braucht nur die eine Gruppe**, nie die ganze Periode: Wer die
//!   Blöcke hat, liefert jeden Teil auf Anfrage, ohne Vorrat auf der Platte.
//! - **Aus beliebigen 32 der 64 Teile kommt die Gruppe zurück**, und ihre
//!   Blöcke prüft man wie jede nachgeladene Geschichte gegen die Historie.
//!
//! # ⚑ Die Teilwurzel
//!
//! Je Gruppe ein Merkle-Baum über die 64 Teile ([`gruppenwurzel`]), je
//! Periode einer über die Gruppenwurzeln ([`teilwurzel`]). Ein Teil
//! belegt sich mit zwei Pfaden ([`Teilbeweis`]) gegen die Teilwurzel, die
//! der Zustand festhält.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::erasure::{ErasureCoder, Fragment};
use crate::hash::Hash;
use crate::merkle::{MerkleProof, MerkleTree};

/// Datenteile je Gruppe.
pub const DATENTEILE: usize = 32;
/// Paritätsteile je Gruppe.
pub const PARITAETSTEILE: usize = 32;
/// Teile je Gruppe, zugleich Splitter je Periode.
pub const TEILE: usize = DATENTEILE + PARITAETSTEILE;

/// Höhen je Gruppe.
pub const GRUPPE_HOEHEN: u64 = 1 << crate::historie::GRUPPENSTUFE;

fn coder() -> ErasureCoder {
    ErasureCoder::new(DATENTEILE, PARITAETSTEILE).expect("32 + 32 liegt unter 255")
}

/// Die Höhen der Gruppe `g` mit Block: ohne Höhe 0.
pub fn gruppenhoehen(g: u64) -> std::ops::RangeInclusive<u64> {
    (g * GRUPPE_HOEHEN).max(1)..=(g * GRUPPE_HOEHEN + GRUPPE_HOEHEN - 1)
}

/// Die Gruppen, deren letzte Höhe in `(vorige_grenze, grenze]` liegt: der
/// Abschnitt, den die Splitter der Grenze `grenze` halten. Halboffen,
/// `(ab, bis)`.
pub fn abschnitt(vorige_grenze: u64, grenze: u64) -> (u64, u64) {
    // Gruppe g endet mit Höhe (g + 1) · 1024 − 1 und ist fertig, sobald
    // diese Höhe in der Historie steht, also bei (g + 1) · 1024 Blättern.
    (vorige_grenze / GRUPPE_HOEHEN, grenze / GRUPPE_HOEHEN)
}

/// Die Bytes einer Gruppe: Anzahl, dann je Block Länge und Inhalt.
fn gruppenbytes(bloecke: &[Vec<u8>]) -> Vec<u8> {
    let mut aus = Vec::with_capacity(8 + bloecke.iter().map(|b| b.len() + 4).sum::<usize>());
    aus.extend_from_slice(&(bloecke.len() as u32).to_le_bytes());
    for b in bloecke {
        aus.extend_from_slice(&(b.len() as u32).to_le_bytes());
        aus.extend_from_slice(b);
    }
    aus
}

fn aus_gruppenbytes(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let zahl = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    let n = zahl(0)?;
    if n as u64 > GRUPPE_HOEHEN {
        return None;
    }
    let mut at = 4;
    let mut aus = Vec::with_capacity(n);
    for _ in 0..n {
        let l = zahl(at)?;
        at += 4;
        aus.push(bytes.get(at..at.checked_add(l)?)?.to_vec());
        at += l;
    }
    // Hinter dem letzten Block nur Auffüllung.
    bytes[at..].iter().all(|&b| b == 0).then_some(aus)
}

/// **Codiert eine Gruppe** in [`TEILE`] Teile gleicher Länge.
pub fn kodieren(bloecke: &[Vec<u8>]) -> Vec<Vec<u8>> {
    coder()
        .encode(&gruppenbytes(bloecke))
        .expect("die Gruppenbytes sind nie leer")
        .into_iter()
        .map(|f| f.data)
        .collect()
}

/// **Setzt eine Gruppe zusammen** aus mindestens [`DATENTEILE`] Teilen,
/// `(Index, Teil)`. `None`, wenn es zu wenige sind oder sie nicht
/// zusammenpassen.
pub fn dekodieren(teile: &[(usize, Vec<u8>)]) -> Option<Vec<Vec<u8>>> {
    let laenge = teile.first()?.1.len();
    if teile.iter().any(|(i, t)| t.len() != laenge || *i >= TEILE) {
        return None;
    }
    let fragmente: Vec<Fragment> =
        teile.iter().take(DATENTEILE).map(|(i, t)| Fragment { index: *i, data: t.clone() }).collect();
    aus_gruppenbytes(&coder().decode(&fragmente).ok()?)
}

/// Die Wurzel über die Teile einer Gruppe.
pub fn gruppenwurzel(teile: &[Vec<u8>]) -> Option<Hash> {
    let blaetter: Vec<&[u8]> = teile.iter().map(Vec::as_slice).collect();
    Some(MerkleTree::new(&blaetter).ok()?.root())
}

/// Die Teilwurzel eines Abschnitts über seine Gruppenwurzeln.
pub fn teilwurzel(gruppenwurzeln: &[Hash]) -> Option<Hash> {
    let blaetter: Vec<&[u8]> = gruppenwurzeln.iter().map(|h| h.as_bytes().as_slice()).collect();
    Some(MerkleTree::new(&blaetter).ok()?.root())
}

/// **Ein Beweis für einen Teil**: Teil `teil` der Gruppe an Stelle
/// `stelle` des Abschnitts.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Teilbeweis {
    /// Die Stelle der Gruppe im Abschnitt, ab null.
    pub stelle: u64,
    /// Welcher der 64 Teile.
    pub teil: u64,
    /// Die Wurzel der Gruppe.
    pub gruppenwurzel: Hash,
    /// Der Pfad des Teils zur Gruppenwurzel.
    pub teilpfad: MerkleProof,
    /// Der Pfad der Gruppenwurzel zur Teilwurzel.
    pub gruppenpfad: MerkleProof,
}

impl Teilbeweis {
    /// Baut den Beweis aus allen Teilen der Gruppe und allen Gruppenwurzeln
    /// des Abschnitts.
    pub fn bauen(teile: &[Vec<u8>], teil: usize, gruppenwurzeln: &[Hash], stelle: usize) -> Option<Self> {
        let blaetter: Vec<&[u8]> = teile.iter().map(Vec::as_slice).collect();
        let baum = MerkleTree::new(&blaetter).ok()?;
        let gw: Vec<&[u8]> = gruppenwurzeln.iter().map(|h| h.as_bytes().as_slice()).collect();
        let gbaum = MerkleTree::new(&gw).ok()?;
        if gruppenwurzeln.get(stelle) != Some(&baum.root()) {
            return None;
        }
        Some(Self {
            stelle: stelle as u64,
            teil: teil as u64,
            gruppenwurzel: baum.root(),
            teilpfad: baum.proof(teil).ok()?,
            gruppenpfad: gbaum.proof(stelle).ok()?,
        })
    }

    /// Ist `stueck` Teil `teil` der Gruppe an Stelle `stelle` unter
    /// `teilwurzel`?
    pub fn pruefen(&self, teilwurzel: &Hash, stueck: &[u8]) -> bool {
        self.teilpfad.leaf_count == TEILE as u64
            && self.teilpfad.verify(&self.gruppenwurzel, stueck, self.teil)
            && self.gruppenpfad.verify(teilwurzel, self.gruppenwurzel.as_bytes(), self.stelle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gruppe(n: usize, saat: u8) -> Vec<Vec<u8>> {
        (0..n).map(|i| (0..(i * 7 + saat as usize) % 300 + 1).map(|j| (i ^ j) as u8 ^ saat).collect()).collect()
    }

    /// ⚑ **Aus beliebigen 32 der 64 Teile kommt die Gruppe zurück**: die
    /// ersten 32, die letzten 32, jeder zweite, und 200 Mischungen.
    #[test]
    fn aus_beliebigen_32_von_64_kommt_die_gruppe_zurueck() {
        let g = gruppe(1023, 3);
        let teile = kodieren(&g);
        assert_eq!(teile.len(), TEILE);
        let mut auswahlen: Vec<Vec<usize>> =
            vec![(0..32).collect(), (32..64).collect(), (0..64).step_by(2).collect(), (1..64).step_by(2).collect()];
        let mut x = 0x9e37_79b9_u64;
        for _ in 0..200 {
            let mut alle: Vec<usize> = (0..64).collect();
            for i in (1..alle.len()).rev() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                alle.swap(i, (x % (i as u64 + 1)) as usize);
            }
            auswahlen.push(alle[..32].to_vec());
        }
        for a in auswahlen {
            let wahl: Vec<(usize, Vec<u8>)> = a.iter().map(|&i| (i, teile[i].clone())).collect();
            assert_eq!(dekodieren(&wahl).as_ref(), Some(&g), "Auswahl {a:?}");
        }
        let zu_wenig: Vec<(usize, Vec<u8>)> = (0..31).map(|i| (i, teile[i].clone())).collect();
        assert_eq!(dekodieren(&zu_wenig), None);
    }

    /// ⚑ **Ein Teil belegt sich gegen die Teilwurzel**, ein verfälschter
    /// oder vertauschter nicht.
    #[test]
    fn ein_teil_belegt_sich_gegen_die_teilwurzel() {
        let gruppen: Vec<Vec<Vec<u8>>> = (0..5).map(|s| gruppe(1024, s)).collect();
        let teile: Vec<Vec<Vec<u8>>> = gruppen.iter().map(|g| kodieren(g)).collect();
        let wurzeln: Vec<Hash> = teile.iter().map(|t| gruppenwurzel(t).expect("Wurzel")).collect();
        let tw = teilwurzel(&wurzeln).expect("Teilwurzel");
        let bw = Teilbeweis::bauen(&teile[3], 17, &wurzeln, 3).expect("Beweis");
        assert!(bw.pruefen(&tw, &teile[3][17]));
        assert!(!bw.pruefen(&tw, &teile[3][18]), "anderer Teil");
        assert!(!bw.pruefen(&tw, &teile[2][17]), "andere Gruppe");
        let mut x = bw.clone();
        x.teil = 18;
        assert!(!x.pruefen(&tw, &teile[3][17]), "andere Stelle im Teil");
        assert!(!bw.pruefen(&Hash::sha256(b"x"), &teile[3][17]), "fremde Teilwurzel");
        assert!(Teilbeweis::bauen(&teile[3], 17, &wurzeln, 2).is_none(), "Gruppe an falscher Stelle");
    }

    /// ⛔️ **Eine verfälschte Auffüllung fällt auf**: Aus den Datenteilen
    /// allein kommt sonst eine Gruppe mit Anhang zurück.
    #[test]
    fn eine_verfaelschte_auffuellung_faellt_auf() {
        let g = gruppe(10, 1);
        let mut teile = kodieren(&g);
        let laenge: usize = 4 + g.iter().map(|b| b.len() + 4).sum::<usize>();
        assert!(teile[0].len() * DATENTEILE > laenge, "die Probe braucht Auffüllung");
        let letzter = teile[DATENTEILE - 1].len() - 1;
        teile[DATENTEILE - 1][letzter] ^= 1;
        let wahl: Vec<(usize, Vec<u8>)> = (0..DATENTEILE).map(|i| (i, teile[i].clone())).collect();
        assert_eq!(dekodieren(&wahl), None);
    }

    #[test]
    fn der_abschnitt_umfasst_die_in_der_periode_fertigen_gruppen() {
        assert_eq!(abschnitt(0, 115_200), (0, 112));
        assert_eq!(abschnitt(115_200, 230_400), (112, 225));
        assert_eq!(gruppenhoehen(0), 1..=1023);
        assert_eq!(gruppenhoehen(2), 2048..=3071);
    }
}
