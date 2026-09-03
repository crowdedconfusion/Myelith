//! Das eigene Gateway im Knoten (B6-3, Stufe 4, erster Schnitt).
//!
//! # ⚑ Warum die Tür hier läuft und nicht anderswo
//!
//! **Entschieden am 2026-09-03: nur das eigene Gateway.** Der Betreiber
//! ist der Kontoinhaber, die Tür hört auf der Rückschleife, und damit
//! entfallen Namensfindung, Vertrauensfrage und Vergütung.
//!
//! ⚑ **K0s Einwand gilt hier nicht, und das ist der Grund, warum es
//! geht.** K0 sagt: „Eine öffentliche Tür gehört nicht auf die
//! Konsensmaschine", denn ein Überlastangriff gegen die Tür wäre einer
//! gegen die Lebendigkeit des Konsenses. **Diese Tür ist nicht
//! öffentlich.** Wer sie hinausbindet, verlässt den entschiedenen
//! Zuschnitt, und die Hilfe sagt das.
//!
//! # ⚑ Was der Kontraktquelle zugrunde liegt
//!
//! Der Zugang hängt an einem Sitzungskontrakt **aus der Kette**. Der
//! Kettenzustand gehört der Ereignisschleife des Knotens; ihn mit einem
//! Netzdienst zu teilen hiesse, eine Sperre über ein `await` zu halten.
//!
//! **Deshalb legt der Knoten bei jedem Block eine Abschrift ab**, und
//! die Tür liest nur diese. Dieselbe Bauart wie bei der
//! Betriebsbeobachtung, und aus demselben Grund.
//!
//! ⚑ **Die Abschrift ist so frisch wie der letzte Block.** Ein Widerruf
//! wirkt also mit der Verzögerung eines Blocks, nicht sofort. Das
//! gehört gesagt: Wer widerruft, will meist sofort, und zwei Sekunden
//! sind zwei Sekunden.
//!
//! # Was dieser Schnitt **nicht** ist
//!
//! **Er gibt noch nichts an einen Pod.** Es gibt keinen Weg dorthin, auf
//! keiner der beiden Seiten; das ist der Rest von Stufe 4 und die
//! eigentliche Arbeit. Was hier steht, ist die Tür am richtigen Ort,
//! mit dem Kontrakt aus der Kette statt aus einer Attrappe.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use myl_types::ids::SitzungId;
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand};

/// Der Vorgabeport der eigenen Tür.
///
/// ⚑ **Neben dem Netzport (4150) und dem Beobachtungsport (4151)**,
/// damit ein Betreiber die drei nicht verwechselt und ein Blick in
/// `netstat` sagt, was wozu gehört.
pub const TUER_PORT: u16 = 4160;

/// Die Abschrift der Sitzungskontrakte, die die Tür liest.
///
/// Der Knoten schreibt, die Tür liest. Eine `Mutex` genügt: Die Sperre
/// wird nur für das Kopieren gehalten, nie über ein `await`.
#[derive(Debug, Clone, Default)]
pub struct Kontraktabschrift {
    stand: Arc<Mutex<BTreeMap<SitzungId, (Sitzungskontrakt, Sitzungszustand)>>>,
}

impl Kontraktabschrift {
    /// Eine leere Abschrift.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Legt den Stand der Kette ab.
    ///
    /// **Eine vergiftete Sperre wird übergangen, nicht weitergereicht.**
    /// Der Knoten soll an der Tür nicht sterben.
    pub fn setzen(&self, zustand: &myl_ledger::LedgerState) {
        let neu: BTreeMap<SitzungId, (Sitzungskontrakt, Sitzungszustand)> = zustand
            .sitzungen
            .iter()
            .map(|(id, s)| (*id, (s.kontrakt.clone(), s.zustand)))
            .collect();
        if let Ok(mut g) = self.stand.lock() {
            *g = neu;
        }
    }

    /// Wie viele Kontrakte die Abschrift führt.
    pub fn anzahl(&self) -> usize {
        self.stand.lock().map(|g| g.len()).unwrap_or(0)
    }
}

impl myl_gateway::zugang::Kontraktquelle for Kontraktabschrift {
    fn nachschlagen(&self, sitzung: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        self.stand.lock().ok()?.get(&sitzung).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_gateway::zugang::Kontraktquelle;
    use myl_types::ids::{Address, EpochId};
    use myl_types::sitzung::Grenzen;

    fn kontrakt(b: u8) -> Sitzungskontrakt {
        Sitzungskontrakt {
            inhaber: Address::new([b; 32]),
            agent: Address::new([b; 32]),
            credits: Grenzen::gesperrt(),
            myl: Grenzen::gesperrt(),
            empfaenger: Vec::new(),
            gueltig_ab: EpochId(0),
            gueltig_bis: EpochId(10),
            max_schritte: 1,
        }
    }

    /// Was in der Kette steht, findet die Tür.
    #[test]
    fn die_abschrift_gibt_wieder_was_in_der_kette_steht() {
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let k = kontrakt(3);
        let id = k.adresse();
        zustand.sitzungen.insert(
            id,
            myl_ledger::state::Sitzung {
                kontrakt: k.clone(),
                zustand: Sitzungszustand::neu(),
            },
        );

        let a = Kontraktabschrift::neu();
        assert_eq!(a.nachschlagen(id), None, "vor dem Abgleich weiss sie nichts");
        a.setzen(&zustand);
        assert_eq!(a.anzahl(), 1);
        assert_eq!(a.nachschlagen(id).map(|(k, _)| k), Some(k));
    }

    /// ⚑ **Ein Widerruf in der Kette erreicht die Tür**, sobald der
    /// nächste Abgleich läuft. Ohne diesen Test wäre die Abschrift eine
    /// Kopie, die niemand auffrischt.
    #[test]
    fn ein_widerruf_erreicht_die_tuer() {
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let k = kontrakt(4);
        let id = k.adresse();
        zustand.sitzungen.insert(
            id,
            myl_ledger::state::Sitzung {
                kontrakt: k,
                zustand: Sitzungszustand::neu(),
            },
        );
        let a = Kontraktabschrift::neu();
        a.setzen(&zustand);
        assert!(!a.nachschlagen(id).expect("da").1.widerrufen);

        zustand.sitzungen.get_mut(&id).expect("da").zustand.widerrufen = true;
        a.setzen(&zustand);
        assert!(
            a.nachschlagen(id).expect("da").1.widerrufen,
            "der Widerruf kam nicht an; die Abschrift wird nicht aufgefrischt"
        );
    }

    /// Die drei Ports liegen nebeneinander und stoßen sich nicht.
    #[test]
    fn die_drei_ports_sind_verschieden() {
        assert_eq!(TUER_PORT, 4160);
        assert_ne!(TUER_PORT, 4150, "der Netzport");
        assert_ne!(TUER_PORT, 4151, "der Beobachtungsport");
    }
}
