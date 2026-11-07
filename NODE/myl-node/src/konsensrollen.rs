//! **Prüfrollen gegen den Konsens** (CONSENSUS 7.7b bis 7.7d, 2026-11-05):
//! ein Erzeuger, der seine Blöcke zurückhält, einer, der im selben Slot
//! zweimal baut, und eine Uhr, die falsch geht. Nur mit dem Merkmal
//! `konsensrollen`, nie im ausgelieferten Programm, gesteuert über
//! Umgebungsvariablen, damit ein Netzlauf zählen kann, was die ehrlichen
//! Knoten daraus machen.
//!
//! - `MYL_ZURUECKHALTEN_MS=<ms>`: Jeder eigene Block geht erst so viel
//!   später hinaus (ein später Block; Vorbild der private Angriff auf
//!   Ketten nach der Länge und die verspäteten Blöcke bei Gasper).
//!   Erwartet: Ein ehrlicher Knoten wechselt nur für einen echt längeren
//!   oder höher gerechtfertigten Zweig, nie unter den endgültigen
//!   Prüfpunkt.
//! - `MYL_DOPPELT_ERZEUGEN=1`: Nach jedem eigenen Block ein zweiter im
//!   selben Slot, mit anderem Zeitstempel, neu unterschrieben. Erwartet:
//!   Doppelbeleg, Schlachtung, Ausschluss aus dem Satz.
//! - `MYL_UHR_VERSATZ_MS=<ms>`, auch negativ: die eigene Uhr für Slots und
//!   die Annahmeregel. Erwartet: bis zum Spielraum der Annahmeregel
//!   (eine Sekunde) nichts; darüber bleiben die eigenen Slots leer, weil
//!   die anderen seine Blöcke für zukünftig halten, oder, nachgehend, er
//!   nimmt fremde Blöcke erst später an.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Die Rollen dieses Knotens, aus der Umgebung beim Start.
#[derive(Debug, Default)]
pub struct Rollen {
    /// Versatz der eigenen Uhr in Millisekunden.
    pub uhr_versatz_ms: i64,
    /// So lange hält der Knoten jeden eigenen Block zurück; null: gar nicht.
    pub zurueckhalten: Duration,
    /// Baut im selben Slot einen zweiten Block.
    pub doppelt: bool,
    /// Zurückgehaltene Blöcke: wann sie hinausgehen, und ihre Bytes.
    pub zurueckgehalten: VecDeque<(Instant, Vec<u8>)>,
}

impl Rollen {
    /// Liest die Rollen aus der Umgebung; was fehlt oder nicht lesbar ist,
    /// ist aus.
    pub fn aus_umgebung() -> Self {
        let zahl = |name: &str| std::env::var(name).ok().and_then(|w| w.trim().parse::<i64>().ok()).unwrap_or(0);
        Self {
            uhr_versatz_ms: zahl("MYL_UHR_VERSATZ_MS"),
            zurueckhalten: Duration::from_millis(zahl("MYL_ZURUECKHALTEN_MS").max(0) as u64),
            doppelt: zahl("MYL_DOPPELT_ERZEUGEN") == 1,
            zurueckgehalten: VecDeque::new(),
        }
    }

    /// Ob überhaupt eine Rolle gilt (für das Startprotokoll).
    pub fn aktiv(&self) -> bool {
        self.uhr_versatz_ms != 0 || !self.zurueckhalten.is_zero() || self.doppelt
    }

    /// Die zurückgehaltenen Blöcke, deren Zeit gekommen ist.
    pub fn faellige(&mut self) -> Vec<Vec<u8>> {
        let jetzt = Instant::now();
        let mut faellig = Vec::new();
        while self.zurueckgehalten.front().is_some_and(|(t, _)| *t <= jetzt) {
            if let Some((_, d)) = self.zurueckgehalten.pop_front() {
                faellig.push(d);
            }
        }
        faellig
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ohne Angabe gilt keine Rolle, und fällig ist erst, was an der Reihe
    /// ist.
    #[test]
    fn ohne_angabe_keine_rolle() {
        let mut r = Rollen::default();
        assert!(!r.aktiv());
        r.zurueckgehalten.push_back((Instant::now(), vec![1]));
        r.zurueckgehalten.push_back((Instant::now() + Duration::from_secs(60), vec![2]));
        assert_eq!(r.faellige(), vec![vec![1]]);
        assert_eq!(r.zurueckgehalten.len(), 1);
    }
}
