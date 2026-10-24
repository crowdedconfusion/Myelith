//! **Die gehörte Höhe als Behauptung einzelner Gegenstellen** (Fund 634,
//! 2026-10-24).
//!
//! Ein Knoten merkt sich, welche Höhe er von anderen gehört hat: Daran hängen
//! die Bereitschaft (`/bereit`), das Ziel des Nachforderns und der Bezug der
//! Fristen eigener Transaktionen. Bis hierher war es eine einzige Zahl, die
//! nur wuchs, gespeist aus jedem abgelehnten Block. Blöcke tragen heute keine
//! Unterschrift; **ein einziger erfundener Block mit Höhe 10¹² hielt den
//! Knoten für immer „nicht bereit“** und liess ihn ohne Ende nach Blöcken
//! fragen, die es nicht gibt.
//!
//! Jetzt ist die Höhe eine **Behauptung der Gegenstelle**, die sie nannte:
//!
//! - Sie fällt, wenn die Gegenstelle auf eine Nachforderung ab einer Höhe,
//!   die sie behauptet hat, **nichts** liefert; danach zählen ihre
//!   Behauptungen [`SPERRE`] lang nicht.
//! - Sie fällt, wenn die Gegenstelle **sich trennt**.
//! - Die gehörte Höhe ist die höchste geltende Behauptung.
//!
//! ⚠️ **Was das nicht schliesst:** Viele Kennungen zugleich können eine
//! erfundene Höhe für die Dauer einer Nachforderung aufrechterhalten. Das
//! verzögert die Bereitschaft, hält sie nicht dauerhaft an; unterschriebene
//! Blöcke machen die Frage später ganz entscheidbar.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// So lange zählen die Behauptungen einer Gegenstelle nicht, die eine
/// widerlegt bekam.
pub const SPERRE: Duration = Duration::from_secs(300);

/// Die Behauptungen über die Höhe, je Gegenstelle `K`.
#[derive(Debug)]
pub struct Gehoert<K> {
    behauptet: HashMap<K, u64>,
    widerlegt: HashMap<K, Instant>,
}

impl<K> Default for Gehoert<K> {
    fn default() -> Self {
        Self { behauptet: HashMap::new(), widerlegt: HashMap::new() }
    }
}

impl<K: Eq + Hash + Clone> Gehoert<K> {
    /// `wer` nennt die Höhe `hoehe`. Zählt nicht, solange `wer` gesperrt ist.
    pub fn hoeren(&mut self, wer: &K, hoehe: u64, jetzt: Instant) {
        if self.widerlegt.get(wer).is_some_and(|t| jetzt.saturating_duration_since(*t) < SPERRE) {
            return;
        }
        self.widerlegt.remove(wer);
        let h = self.behauptet.entry(wer.clone()).or_insert(0);
        *h = (*h).max(hoehe);
    }

    /// `wer` hatte ab `ab` nichts. Hatte er eine Höhe ab `ab` behauptet, ist
    /// sie widerlegt: Sie fällt, und `wer` wird gesperrt.
    pub fn nichts_ab(&mut self, wer: &K, ab: u64, jetzt: Instant) {
        if self.behauptet.get(wer).is_some_and(|h| *h >= ab) {
            self.behauptet.remove(wer);
            self.widerlegt.insert(wer.clone(), jetzt);
        }
    }

    /// `wer` hat sich getrennt; seine Behauptung verfällt.
    pub fn getrennt(&mut self, wer: &K) {
        self.behauptet.remove(wer);
    }

    /// Die höchste geltende Behauptung, null ohne eine.
    pub fn hoechste(&self) -> u64 {
        self.behauptet.values().copied().max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die gehörte Höhe ist die höchste Behauptung.
    #[test]
    fn die_hoechste_behauptung_zaehlt() {
        let mut g = Gehoert::default();
        let t = Instant::now();
        g.hoeren(&1u8, 100, t);
        g.hoeren(&2u8, 250, t);
        g.hoeren(&1u8, 120, t);
        assert_eq!(g.hoechste(), 250);
    }

    /// ⚑ **Eine erfundene Höhe fällt, sobald ihre Gegenstelle nichts
    /// liefert**, und kommt von ihr fünf Minuten lang nicht wieder.
    #[test]
    fn eine_widerlegte_behauptung_faellt_und_bleibt_weg() {
        let mut g = Gehoert::default();
        let t = Instant::now();
        g.hoeren(&1u8, 500, t);
        g.hoeren(&9u8, 1_000_000_000_000, t);
        assert_eq!(g.hoechste(), 1_000_000_000_000);
        g.nichts_ab(&9u8, 501, t);
        assert_eq!(g.hoechste(), 500);
        g.hoeren(&9u8, 1_000_000_000_000, t + Duration::from_secs(10));
        assert_eq!(g.hoechste(), 500, "gesperrt");
        g.hoeren(&9u8, 600, t + SPERRE);
        assert_eq!(g.hoechste(), 600, "nach der Sperre zählt sie wieder");
    }

    /// Nichts unterhalb der Behauptung widerlegt nichts: Wer 500 behauptet
    /// und ab 600 nichts hat, sagt die Wahrheit.
    #[test]
    fn nichts_oberhalb_der_behauptung_widerlegt_nicht() {
        let mut g = Gehoert::default();
        let t = Instant::now();
        g.hoeren(&1u8, 500, t);
        g.nichts_ab(&1u8, 600, t);
        assert_eq!(g.hoechste(), 500);
    }

    /// Wer sich trennt, behauptet nichts mehr.
    #[test]
    fn eine_trennung_nimmt_die_behauptung_mit() {
        let mut g = Gehoert::default();
        let t = Instant::now();
        g.hoeren(&1u8, 500, t);
        g.hoeren(&2u8, 700, t);
        g.getrennt(&2u8);
        assert_eq!(g.hoechste(), 500);
    }
}
