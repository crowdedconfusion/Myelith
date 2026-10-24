//! **Die Drossel für teure Anfragen** (Fund 632, 2026-10-24).
//!
//! Drei Anfragen, die jede Gegenstelle stellen darf, kosten den Antwortenden
//! Arbeit in der Ereignisschleife, und zwar mehr, als ihre Bytes vermuten
//! lassen:
//!
//! - **`Bloecke`** liest bis zu 512 Blöcke von der Platte (gedeckelt seit
//!   Fund 141, aber nicht in der Häufigkeit);
//! - **`Archiv`** liest 512 Blöcke und dazu die 1024 Blöcke einer Gruppe für
//!   den Beweis;
//! - **`Teil`** codiert für jede andere Gruppe 1024 Blöcke neu.
//!
//! Eine Flut davon hält die Blockverarbeitung an, dieselbe Klasse wie
//! Fund 609. Hier steht je Art ein Eimer je Gegenstelle und einer für alle:
//! Jede Anfrage nimmt eine Marke, Marken wachsen mit der Zeit nach. Wer
//! keine bekommt, erhält `Nichts`, und das kostet nichts. Ehrliche Abholer
//! weichen bei `Nichts` auf eine andere Gegenstelle aus.
//!
//! ⚑ **Ganzzahlig in Tausendsteln einer Marke**, die Zeit in Millisekunden:
//! keine Rundung, die sich über viele Anfragen aufsummiert.
//!
//! ⚑ **Die Zahl der geführten Gegenstellen ist gedeckelt**
//! ([`GEGENSTELLEN_HOECHSTENS`]): Kennungen kosten nichts, und eine Tabelle,
//! die mit jeder neuen wächst, wäre selbst der Angriff. Ist sie voll, fallen
//! zuerst die längst wieder vollen Eimer heraus; sonst teilt sich jeder Neue
//! einen gemeinsamen Eimer.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;

/// So viele Gegenstellen führt die Drossel je Art höchstens einzeln.
pub const GEGENSTELLEN_HOECHSTENS: usize = 1024;

/// Welche Art einer teuren Anfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Art {
    /// Blöcke für das Nachholen der eigenen Kette.
    Bloecke,
    /// Geschichte mit Beweis und Teile für Splitter.
    Archiv,
}

/// Fassungsvermögen und Nachwuchs eines Eimers, in ganzen Marken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Masse {
    pub fassung: u64,
    pub je_sekunde: u64,
}

/// Die Masse je Art: je Gegenstelle und für alle zusammen.
///
/// ⚑ **Bloecke grosszügig**: Ein Knoten nach einem Absturz holt in Sekunden
/// Tausende Blöcke, und das Nachholen ist der Betrieb. **Archiv knapper**:
/// Ehrliche Abholer stellen je Gegenstelle höchstens zwei zugleich und
/// warten jede Antwort ab; acht je Sekunde sind weit darüber.
pub fn masse(art: Art) -> (Masse, Masse) {
    match art {
        Art::Bloecke => (Masse { fassung: 32, je_sekunde: 32 }, Masse { fassung: 128, je_sekunde: 128 }),
        Art::Archiv => (Masse { fassung: 8, je_sekunde: 8 }, Masse { fassung: 32, je_sekunde: 32 }),
    }
}

#[derive(Debug, Clone, Copy)]
struct Eimer {
    /// Marken in Tausendsteln.
    stand: u64,
    zuletzt: Instant,
}

impl Eimer {
    fn voll(m: Masse, jetzt: Instant) -> Self {
        Self { stand: m.fassung * 1000, zuletzt: jetzt }
    }

    fn nachfuellen(&mut self, m: Masse, jetzt: Instant) {
        let ms = jetzt.saturating_duration_since(self.zuletzt).as_millis().min(u64::MAX as u128) as u64;
        self.stand = self.stand.saturating_add(ms.saturating_mul(m.je_sekunde)).min(m.fassung * 1000);
        self.zuletzt = jetzt;
    }

    fn hat_eine(&self) -> bool {
        self.stand >= 1000
    }
}

/// Die Eimer einer Art.
#[derive(Debug)]
struct Arteimer<K> {
    einzeln: HashMap<K, Eimer>,
    gemeinsam_neue: Option<Eimer>,
    alle: Option<Eimer>,
}

impl<K> Default for Arteimer<K> {
    fn default() -> Self {
        Self { einzeln: HashMap::new(), gemeinsam_neue: None, alle: None }
    }
}

/// Die Drossel, je Art und je Gegenstelle `K`.
#[derive(Debug)]
pub struct Drossel<K> {
    arten: HashMap<Art, Arteimer<K>>,
}

impl<K> Default for Drossel<K> {
    fn default() -> Self {
        Self { arten: HashMap::new() }
    }
}

impl<K: Eq + Hash + Clone> Drossel<K> {
    /// **Darf `wer` jetzt eine Anfrage der Art `art` stellen?** Bei Ja ist
    /// die Marke genommen, aus seinem Eimer und aus dem für alle.
    pub fn erlaubt(&mut self, art: Art, wer: &K, jetzt: Instant) -> bool {
        let (je, gesamt) = masse(art);
        let a = self.arten.entry(art).or_default();
        let alle = a.alle.get_or_insert_with(|| Eimer::voll(gesamt, jetzt));
        alle.nachfuellen(gesamt, jetzt);
        if !a.einzeln.contains_key(wer) && a.einzeln.len() >= GEGENSTELLEN_HOECHSTENS {
            a.einzeln.retain(|_, e| {
                let mut x = *e;
                x.nachfuellen(je, jetzt);
                x.stand < je.fassung * 1000
            });
        }
        let eigener = if a.einzeln.contains_key(wer) || a.einzeln.len() < GEGENSTELLEN_HOECHSTENS {
            a.einzeln.entry(wer.clone()).or_insert_with(|| Eimer::voll(je, jetzt))
        } else {
            a.gemeinsam_neue.get_or_insert_with(|| Eimer::voll(je, jetzt))
        };
        eigener.nachfuellen(je, jetzt);
        if !eigener.hat_eine() || !alle.hat_eine() {
            return false;
        }
        eigener.stand -= 1000;
        alle.stand -= 1000;
        true
    }

    /// Wie viele Gegenstellen je Art einzeln geführt werden.
    pub fn gefuehrt(&self, art: Art) -> usize {
        self.arten.get(&art).map_or(0, |a| a.einzeln.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Eine Gegenstelle bekommt ihr Fassungsvermögen sofort, danach nur so
    /// viel, wie nachwächst.
    #[test]
    fn eine_gegenstelle_bekommt_ihre_fassung_und_dann_den_nachwuchs() {
        let mut d = Drossel::default();
        let t = Instant::now();
        let (je, _) = masse(Art::Archiv);
        let erlaubt = (0..je.fassung + 5).filter(|_| d.erlaubt(Art::Archiv, &1u32, t)).count() as u64;
        assert_eq!(erlaubt, je.fassung);
        // Eine Achtelsekunde später genau eine weitere.
        let spaeter = t + Duration::from_millis(1000 / je.je_sekunde);
        assert!(d.erlaubt(Art::Archiv, &1u32, spaeter));
        assert!(!d.erlaubt(Art::Archiv, &1u32, spaeter));
    }

    /// ⚑ Viele Gegenstellen zusammen kommen nicht über den Eimer für alle:
    /// Kennungen kosten nichts.
    #[test]
    fn viele_gegenstellen_teilen_sich_den_eimer_fuer_alle() {
        let mut d = Drossel::default();
        let t = Instant::now();
        let (_, gesamt) = masse(Art::Archiv);
        let erlaubt = (0..1000u32).filter(|k| d.erlaubt(Art::Archiv, k, t)).count() as u64;
        assert_eq!(erlaubt, gesamt.fassung);
    }

    /// Die Arten sind getrennt: Eine Flut von Archivanfragen nimmt dem
    /// Nachholen nichts.
    #[test]
    fn die_arten_sind_getrennt() {
        let mut d = Drossel::default();
        let t = Instant::now();
        while d.erlaubt(Art::Archiv, &1u32, t) {}
        assert!(d.erlaubt(Art::Bloecke, &1u32, t));
    }

    /// ⚑ **Ist die Tabelle voll, fallen die längst wieder vollen Eimer
    /// heraus**, und ein Neuer bekommt einen eigenen statt des gemeinsamen:
    /// Sonst teilte sich nach einer Flut von Kennungen jeder ehrliche
    /// Neuling einen Eimer mit dem Angreifer.
    #[test]
    fn volle_eimer_machen_platz_fuer_neue() {
        let mut d = Drossel::default();
        let t = Instant::now();
        for k in 0..GEGENSTELLEN_HOECHSTENS as u32 {
            let _ = d.erlaubt(Art::Bloecke, &k, t);
        }
        assert_eq!(d.gefuehrt(Art::Bloecke), GEGENSTELLEN_HOECHSTENS);
        let spaeter = t + Duration::from_secs(10);
        assert!(d.erlaubt(Art::Bloecke, &u32::MAX, spaeter));
        assert_eq!(d.gefuehrt(Art::Bloecke), 1, "die vollen sind gegangen, der Neue hat einen eigenen");
    }

    /// ⚑ Die Tabelle wächst nicht über ihre Grenze, wie viele Kennungen
    /// auch kommen; volle Eimer fallen zuerst heraus.
    #[test]
    fn die_tabelle_waechst_nicht_ueber_ihre_grenze() {
        let mut d = Drossel::default();
        let mut t = Instant::now();
        for k in 0..(GEGENSTELLEN_HOECHSTENS as u32 * 3) {
            t += Duration::from_millis(50);
            let _ = d.erlaubt(Art::Bloecke, &k, t);
            assert!(d.gefuehrt(Art::Bloecke) <= GEGENSTELLEN_HOECHSTENS);
        }
    }
}
