//! **Das Stimmbuch** (2026-11-04, CONSENSUS 7.7 Schritt 4): was ein Knoten
//! zuletzt unterschrieben hat, auf der Platte, damit er nach einem
//! Zweigwechsel oder einem Neustart nichts unterschreibt, wofür man ihn
//! schlachten könnte.
//!
//! # ⚑ Warum
//!
//! Mit Gabelungen hat ein ehrlicher Knoten an derselben Höhe womöglich zwei
//! Blöcke gesehen. Stimmte er nach einem Zweigwechsel noch einmal für einen
//! Prüfpunkt, für den er schon gestimmt hat, oder baute er im selben Slot
//! noch einmal, wäre das ein Doppelbeleg gegen ihn
//! (`myl_consensus::double_signing::Doppelbeleg`). Die Regel:
//! **Jede neue Stimme hat ein höheres Ziel und keine niedrigere
//! Quelle als die vorige**; dann widerspricht sie keiner früheren, weder
//! doppelt noch umschliessend. Eine Wiederholung derselben Stimme bleibt
//! erlaubt. Für Blöcke: **jeder Slot höchstens einmal**.
//!
//! ⛔️ Geschrieben wird, bevor die Unterschrift das Haus verlässt; geht das
//! Schreiben schief, wird nicht gestimmt.

use std::path::Path;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::pruefpunkt::Quelle;
use myl_types::hash::Hash;

/// Was dieser Knoten zuletzt unterschrieben hat.
#[derive(Debug, Clone, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Stimmbuch {
    /// Die letzte eigene Prüfpunktstimme: Höhe, Block, Bindung, Quelle.
    pub stimme: Option<(u64, Hash, Hash, Quelle)>,
    /// Der Slot des letzten eigenen Blocks.
    pub slot: u64,
}

impl Stimmbuch {
    /// ⚑ **Darf dieser Knoten so stimmen?** Dieselbe Stimme noch einmal, oder
    /// eine mit höherem Ziel und keiner niedrigeren Quelle.
    pub fn darf_stimmen(&self, hoehe: u64, block_hash: &Hash, bindung: &Hash, quelle: &Quelle) -> bool {
        match &self.stimme {
            None => true,
            Some((h, b, g, q)) => (hoehe, block_hash, bindung, quelle) == (*h, b, g, q) || (hoehe > *h && quelle.hoehe >= q.hoehe),
        }
    }

    /// Liest das Stimmbuch; fehlt die Datei, ist es leer.
    pub fn laden(pfad: &Path) -> std::io::Result<Self> {
        match std::fs::read(pfad) {
            Ok(b) => borsh::from_slice(&b).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Schreibt das Stimmbuch: erst daneben, auf die Platte gezwungen, dann
    /// an seinen Platz, damit nie ein halbes liegt.
    ///
    /// 📌 **Gezwungen ueber den Griff, der schreibt** (2026-11-07, CI unter
    /// Windows rot): Vorher wurde die Datei zum Erzwingen nur lesend
    /// geoeffnet. Unter Windows verlangt das Erzwingen Schreibrecht
    /// (`FlushFileBuffers`), also scheiterte jedes Speichern, und ein Knoten
    /// unter Windows haette nie gestimmt.
    pub fn speichern(&self, pfad: &Path) -> std::io::Result<()> {
        use std::io::Write;
        let neu = pfad.with_extension("stimmbuch.neu");
        let bytes = borsh::to_vec(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut datei = std::fs::File::create(&neu)?;
        datei.write_all(&bytes)?;
        datei.sync_all()?;
        drop(datei);
        std::fs::rename(&neu, pfad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(h: u64) -> Quelle {
        Quelle { hoehe: h, block_hash: Hash::sha256(&h.to_le_bytes()) }
    }

    /// ⚑ **Nur höher, nie umschliessend**: dieselbe Stimme noch einmal ja;
    /// dasselbe Ziel mit anderem Block, ein niedrigeres Ziel und eine
    /// niedrigere Quelle nein.
    #[test]
    fn nur_hoeher_und_nie_umschliessend() {
        let (a, b, g) = (Hash::sha256(b"a"), Hash::sha256(b"b"), Hash::sha256(b"g"));
        let mut s = Stimmbuch::default();
        assert!(s.darf_stimmen(300, &a, &g, &q(150)));
        s.stimme = Some((300, a, g, q(150)));
        assert!(s.darf_stimmen(300, &a, &g, &q(150)), "die Wiederholung");
        assert!(!s.darf_stimmen(300, &b, &g, &q(150)), "dasselbe Ziel, ein anderer Block");
        assert!(!s.darf_stimmen(150, &a, &g, &q(0)), "ein niedrigeres Ziel");
        assert!(!s.darf_stimmen(450, &a, &g, &q(0)), "umschlösse die vorige");
        assert!(s.darf_stimmen(450, &b, &g, &q(150)));
        assert!(s.darf_stimmen(450, &b, &g, &q(300)));
    }

    /// Was geschrieben ist, kommt gelesen zurück; ohne Datei ist es leer.
    #[test]
    fn schreiben_und_lesen() {
        let ordner = std::env::temp_dir().join(format!("stimmbuch-{}", std::process::id()));
        std::fs::create_dir_all(&ordner).expect("Ordner");
        let pfad = ordner.join("kette.stimmbuch");
        assert_eq!(Stimmbuch::laden(&pfad).expect("leer"), Stimmbuch::default());
        let s = Stimmbuch { stimme: Some((150, Hash::sha256(b"x"), Hash::sha256(b"y"), q(0))), slot: 77 };
        s.speichern(&pfad).expect("schreiben");
        assert_eq!(Stimmbuch::laden(&pfad).expect("lesen"), s);
        std::fs::remove_dir_all(&ordner).expect("aufräumen");
    }
}
