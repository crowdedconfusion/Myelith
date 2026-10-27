//! **Der Vorausspeicher**: Blöcke, die zu früh kommen, werden aufgehoben
//! statt verworfen (Fund 592, 2026-10-22).
//!
//! # ⚑ Warum es ihn gibt
//!
//! Gossip liefert ohne Reihenfolge. Ein Block, der eine Höhe vor dem nächsten
//! erwarteten ankommt, schließt nicht an; bis zum 2026-10-22 wurde er
//! verworfen, und der Knoten forderte nach. Die Nachlieferung braucht einen
//! Hin- und Rückweg, und bis sie da ist, ist die Spitze weiter. Gemessen im
//! Lauf mit getrennten Prozessen (50 bis 200 ms Laufzeit, 20 Blöcke je
//! Sekunde): **Ein Knoten übernahm 4 von 2 168 Blöcken über Gossip**, alle
//! übrigen kamen über das Nachholen, und keiner der Nichterzeuger holte die
//! Spitze je wieder ein. Mit dem Vorausspeicher schließt die Lücke, sobald
//! eine Nachlieferung bis an die aufgehobenen Blöcke heranreicht.
//!
//! # ⚑ Was er nicht ist
//!
//! **Kein Vertrauensweg.** Eingespielt wird über [`crate::kette::Kette::uebernimm`],
//! mit derselben Anschlussprüfung und derselben Nachrechnung der Wurzel wie
//! jeder andere Block. Ein aufgehobener Block, der dann nicht trägt, fällt
//! ersatzlos weg.
//!
//! # ⚑ Die Deckel, und warum sie vor dem Ablegen stehen
//!
//! Ein Fremder bestimmt, was hier landet. Deshalb drei Grenzen, alle geprüft,
//! bevor etwas Speicher belegt: höchstens [`VORAUS_WEITE`] Höhen voraus,
//! höchstens [`VORAUS_JE_HOEHE`] Kandidaten je Höhe (sonst verdrängte ein
//! Störer den echten Block mit Varianten derselben Höhe), und zusammen
//! höchstens [`VORAUS_BLOECKE`] Blöcke und [`VORAUS_BYTES`] Bytes. Ist er
//! voll, weicht der Block, der am weitesten voraus liegt: Der nächste wird
//! zuerst gebraucht.

use std::collections::BTreeMap;

use myl_consensus::block::Block;
use myl_types::hash::Hash;

/// Höchstens so viele Höhen über der eigenen wird aufgehoben.
pub const VORAUS_WEITE: u64 = 4_096;
/// Höchstens so viele verschiedene Blöcke je Höhe.
pub const VORAUS_JE_HOEHE: usize = 4;
/// Höchstens so viele Blöcke zusammen: vier Lieferungen zu 512, die das
/// Nachholen zugleich anfordert (`knoten::NACHHOLER`).
pub const VORAUS_BLOECKE: usize = 2_048;
/// Höchstens so viele Bytes zusammen (gezählt in der Drahtform).
pub const VORAUS_BYTES: usize = 32 * 1024 * 1024;

/// Die aufgehobenen Blöcke, nach Höhe und Abdruck der Drahtform.
#[derive(Debug, Default)]
pub struct Vorausspeicher {
    bloecke: BTreeMap<(u64, [u8; 32]), (Block, usize)>,
    bytes: usize,
}

/// Warum ein Block nicht aufgehoben wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NichtAbgelegt {
    /// Er liegt nicht über der nächsten erwarteten Höhe.
    NichtVoraus,
    /// Er liegt weiter voraus als [`VORAUS_WEITE`].
    ZuWeit,
    /// Für seine Höhe liegen schon [`VORAUS_JE_HOEHE`] Kandidaten.
    HoeheVoll,
    /// Derselbe Block liegt schon da.
    SchonDa,
    /// Er allein ist größer als der ganze Speicher.
    ZuGross,
    /// Der Speicher ist voll, und alles darin liegt näher.
    Voll,
}

impl Vorausspeicher {
    /// Hebt `block` auf, wenn er über der nächsten erwarteten Höhe liegt.
    /// `bytes` ist seine Drahtform, aus der der Abdruck kommt.
    pub fn ablegen(&mut self, eigene_hoehe: u64, block: Block, bytes: &[u8]) -> Result<(), NichtAbgelegt> {
        let h = block.header.height;
        if h <= eigene_hoehe.saturating_add(1) {
            return Err(NichtAbgelegt::NichtVoraus);
        }
        if h - eigene_hoehe > VORAUS_WEITE {
            return Err(NichtAbgelegt::ZuWeit);
        }
        if bytes.len() > VORAUS_BYTES {
            return Err(NichtAbgelegt::ZuGross);
        }
        let schluessel = (h, *Hash::sha256(bytes).as_bytes());
        if self.bloecke.contains_key(&schluessel) {
            return Err(NichtAbgelegt::SchonDa);
        }
        if self.bloecke.range((h, [0; 32])..=(h, [0xff; 32])).count() >= VORAUS_JE_HOEHE {
            return Err(NichtAbgelegt::HoeheVoll);
        }
        // Voll: Weichen muss, was weiter voraus liegt als der neue. Liegt
        // alles näher, bleibt der neue draussen.
        while self.bloecke.len() >= VORAUS_BLOECKE || self.bytes + bytes.len() > VORAUS_BYTES {
            let Some((&letzter, _)) = self.bloecke.last_key_value() else {
                break;
            };
            if letzter.0 <= h {
                return Err(NichtAbgelegt::Voll);
            }
            if let Some((_, (_, n))) = self.bloecke.remove_entry(&letzter) {
                self.bytes -= n;
            }
        }
        self.bytes += bytes.len();
        self.bloecke.insert(schluessel, (block, bytes.len()));
        Ok(())
    }

    /// Die Kandidaten für die Höhe nach `eigene_hoehe`, herausgenommen;
    /// alles, was nicht mehr über der eigenen Höhe liegt, fällt dabei weg.
    pub fn naechste(&mut self, eigene_hoehe: u64) -> Vec<Block> {
        let ueberholt: Vec<_> = self.bloecke.range(..=(eigene_hoehe, [0xff; 32])).map(|(k, _)| *k).collect();
        for k in ueberholt {
            if let Some((_, n)) = self.bloecke.remove(&k) {
                self.bytes -= n;
            }
        }
        let h = eigene_hoehe + 1;
        let schluessel: Vec<_> = self.bloecke.range((h, [0; 32])..=(h, [0xff; 32])).map(|(k, _)| *k).collect();
        schluessel
            .into_iter()
            .filter_map(|k| self.bloecke.remove(&k))
            .map(|(b, n)| {
                self.bytes -= n;
                b
            })
            .collect()
    }

    /// Ob für diese Höhe ein Block aufgehoben ist.
    pub fn hat_hoehe(&self, h: u64) -> bool {
        self.bloecke.range((h, [0; 32])..=(h, [0xff; 32])).next().is_some()
    }

    /// Wie viele Blöcke aufgehoben sind.
    pub fn len(&self) -> usize {
        self.bloecke.len()
    }

    /// Ob nichts aufgehoben ist.
    pub fn is_empty(&self) -> bool {
        self.bloecke.is_empty()
    }

    /// Wie viele Bytes aufgehoben sind.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(h: u64, marke: u8) -> (Block, Vec<u8>) {
        let b = Block::new(myl_consensus::block::BlockHeader {
            height: h,
            epoch: myl_consensus::block::epoche_fuer_hoehe(h),
            prev_block_hash: Hash::sha256(&h.to_le_bytes()),
            timestamp_ms: 1_700_000_000_000 + h,
            state_root: Hash::sha256(&[marke]),
            pruefpunkt: None,
        });
        let roh = borsh::to_vec(&b).expect("Drahtform");
        (b, roh)
    }

    fn ablegen(s: &mut Vorausspeicher, eigen: u64, h: u64, marke: u8) -> Result<(), NichtAbgelegt> {
        let (b, roh) = block(h, marke);
        s.ablegen(eigen, b, &roh)
    }

    /// Aufgehoben wird nur, was über der nächsten Höhe liegt, und nur
    /// innerhalb der Weite.
    #[test]
    fn nur_was_voraus_liegt() {
        let mut s = Vorausspeicher::default();
        assert_eq!(ablegen(&mut s, 10, 10, 0), Err(NichtAbgelegt::NichtVoraus));
        assert_eq!(ablegen(&mut s, 10, 11, 0), Err(NichtAbgelegt::NichtVoraus), "der nächste gehört in die Kette");
        assert_eq!(ablegen(&mut s, 10, 12, 0), Ok(()));
        assert_eq!(ablegen(&mut s, 10, 10 + VORAUS_WEITE, 0), Ok(()));
        assert_eq!(ablegen(&mut s, 10, 11 + VORAUS_WEITE, 0), Err(NichtAbgelegt::ZuWeit));
        assert_eq!(ablegen(&mut s, 10, 12, 0), Err(NichtAbgelegt::SchonDa));
        assert_eq!(s.len(), 2);
    }

    /// Je Höhe höchstens vier Varianten: Ein Störer verdrängt den echten
    /// Block nicht, indem er Varianten derselben Höhe schickt.
    #[test]
    fn je_hoehe_ein_deckel() {
        let mut s = Vorausspeicher::default();
        for m in 0..VORAUS_JE_HOEHE as u8 {
            assert_eq!(ablegen(&mut s, 0, 5, m), Ok(()));
        }
        assert_eq!(ablegen(&mut s, 0, 5, 99), Err(NichtAbgelegt::HoeheVoll));
        assert_eq!(ablegen(&mut s, 0, 6, 99), Ok(()), "andere Höhen bleiben offen");
    }

    /// Voll: Der am weitesten voraus weicht, und was weiter voraus liegt als
    /// alles darin, bleibt draussen.
    #[test]
    fn voll_weicht_der_fernste() {
        let mut s = Vorausspeicher::default();
        for i in 0..VORAUS_BLOECKE as u64 {
            assert_eq!(ablegen(&mut s, 0, 2 + i, 0), Ok(()));
        }
        let fernste = 1 + VORAUS_BLOECKE as u64;
        assert_eq!(ablegen(&mut s, 0, fernste + 1, 0), Err(NichtAbgelegt::Voll));
        assert_eq!(ablegen(&mut s, 0, 2, 1), Ok(()), "ein naher Block verdrängt den fernsten");
        assert_eq!(s.len(), VORAUS_BLOECKE);
        // Der fernste ist weg: Bei eigener Höhe fernste-1 kommt nichts mehr.
        assert!(s.naechste(fernste - 1).is_empty());
    }

    /// `naechste` gibt die Kandidaten der nächsten Höhe heraus, räumt das
    /// Überholte und führt die Bytes mit.
    #[test]
    fn naechste_raeumt_und_zaehlt() {
        let mut s = Vorausspeicher::default();
        for h in 3..8 {
            ablegen(&mut s, 0, h, 0).unwrap();
        }
        ablegen(&mut s, 0, 5, 1).unwrap();
        let vorher = s.bytes();
        let fuenf = s.naechste(4);
        assert_eq!(fuenf.len(), 2, "beide Kandidaten der Höhe 5");
        assert!(fuenf.iter().all(|b| b.header.height == 5));
        assert_eq!(s.len(), 2, "3 und 4 überholt, 5 herausgenommen, 6 und 7 bleiben");
        assert!(s.bytes() < vorher);
        let rest: usize = s.bloecke.values().map(|(_, n)| n).sum();
        assert_eq!(s.bytes(), rest, "die Bytes stimmen mit dem Inhalt");
        assert!(s.naechste(7).is_empty());
        assert!(s.is_empty());
        assert_eq!(s.bytes(), 0);
    }
}
