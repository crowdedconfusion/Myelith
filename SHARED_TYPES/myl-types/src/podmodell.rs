//! **Das Podmodell eines Netzes**: welches Artefakt die Pods rechnen, und
//! was die Kette davon wissen muss, um nach Zusagen zu schneiden (Stufe 4
//! des Dispatchers, 2026-10-22).
//!
//! **Konsens-Feld:** Es steht im Kettenzustand und geht damit in jede
//! Zustandswurzel ein. Zwei Knoten mit verschiedenem Podmodell trennen
//! sich beim ersten Block, wie zwei mit verschiedenem Startwert.
//!
//! # ⚑ Vier Zahlen und kein Modellprofil
//!
//! Die Kette braucht den **Abdruck** (welches Artefakt), die **Ebenenzahl**
//! (wogegen eine Zusage geprüft und eine Grenze gezogen wird) und den
//! **Kopf** in Platz und Zeit (wie ihn die Verteilung zählt). Mehr nicht:
//! Wie viel eine Ebene rechnet, ist eine Frage der Gutschrift und steht im
//! vTFE-Profil, nicht hier. Dieselbe Trennung wie bei der
//! Arbeitsverteilung: Eine neue Architektur ändert Zahlen, nicht die Form
//! des Zustands.
//!
//! # ⚑ Eines je Netz
//!
//! Im Netz rechnet ein Modell (Festlegung des Projektinhabers vom
//! 2026-10-21: nur das 35B; seit dem 2026-10-22 in der Testphase dessen
//! int4). Eine Zusage für ein anderes Artefakt weist die Kette ab, und ein
//! Miner ohne Zusage für dieses kommt in keinen Pod.

use borsh::{BorshDeserialize, BorshSerialize};

/// Was die Kette über das Modell ihrer Pods weiß.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Podmodell {
    /// Der Abdruck des Artefakts auf der Netzliste.
    pub abdruck: [u8; 32],
    /// Wie viele Ebenen das Modell hat.
    pub ebenen: u32,
    /// Der Kopf in Ebenen nach Bytes, aufgerundet.
    pub kopf_platz: u32,
    /// Der Kopf in Ebenenzeiten, gemessen.
    pub kopf_zeit: u32,
}

impl Podmodell {
    /// Höchstens so viele Ebenen kann eine Zusage für dieses Modell nennen:
    /// alle Ebenen samt Kopf.
    pub fn ebenen_samt_kopf(&self) -> u32 {
        self.ebenen.saturating_add(self.kopf_platz)
    }
}
