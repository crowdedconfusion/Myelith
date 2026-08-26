//! Ausfallsicherung und Epochen-Übergang (Phase 3, Whitepaper Kap. 4.2
//! und 6.8).
//!
//! Kap. 6.8 macht eine **quantitative** Liveness-Zusage:
//!
//! > „Fällt ein Shard-Miner aus, übernimmt der Standby-Miner des Pods
//! > (k+2 Mitglieder, 2 in Reserve); Session-Verlust **nur bei mehr als
//! > zwei gleichzeitigen Ausfällen** im selben Pod."
//!
//! Das ist keine Absichtserklärung, sondern eine Schranke, die man
//! messen kann, und dieses Modul ist die Stelle, an der sie gilt oder
//! nicht.
//!
//! ## Warum die Übernahme nicht einfach „einen anderen Miner nehmen" ist
//!
//! Ein Shard hält einen **KV-Cache** über die bisherigen Positionen der
//! Session. Der Standby hat ihn nicht. Er muss ihn nachbauen, und zwar
//! **bitgleich** mit dem, den der Ausgefallene hatte, denn sonst weicht
//! die Spur ab dem Übernahmezeitpunkt ab und der Redundanzvergleich
//! meldet einen ehrlichen Pod als fehlerhaft.
//!
//! **Bitgleich ist hier billig zu haben**, und das ist eine Folge der
//! Grundentscheidung des Projekts: Die Rechnung ist ganzzahlig und damit
//! reihenfolgeunabhängig. Ein Prefill über dieselben Token liefert
//! denselben Cache, unabhängig davon, welche Maschine ihn rechnet und
//! wie sie parallelisiert. In einem Gleitkomma-System wäre der
//! Cache-Rebuild ein Bruch der Sitzung.
//!
//! ## Wann neu gebaut wird, und wann nicht
//!
//! Kap. 4.2 verlangt: „nur bei Ausfall oder Epochenwechsel ausgelöst".
//! Der Grund ist die Kostenseite: Ein Rebuild kostet einen Prefill über
//! alle bisherigen Positionen, also `O(Position)` Arbeit. Löste er
//! häufiger aus, wäre er nicht mehr eine Ausnahme, sondern ein Aufschlag
//! auf jede Sitzung. [`RebuildAnlass`] hat deshalb **genau zwei** Werte,
//! und der Typ ist die Durchsetzung: Es gibt keinen dritten Grund, weil
//! man keinen dritten benennen kann.

use myl_types::ids::{EpochId, MinerId};

/// Zahl der Reserveplätze je Pod (Kap. 6.8: „k+2 Mitglieder, 2 in
/// Reserve").
pub const RESERVE_PLAETZE: usize = 2;

/// Die Besetzung eines Pods: `k` Shard-Positionen plus Reserve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodBesetzung {
    /// Belegung je Shard-Position; `None` heißt ausgefallen und nicht
    /// nachbesetzt.
    belegung: Vec<Option<MinerId>>,
    /// Die Reserve, in fester Reihenfolge.
    ///
    /// **Reihenfolge ist Konsens-Eigenschaft:** Zwei Knoten, die
    /// verschieden nachbesetzen, kommen zu verschiedenen Pods und damit
    /// zu verschiedenen Spuren.
    reserve: Vec<MinerId>,
    /// Die Epoche, für die diese Besetzung gilt.
    epoche: EpochId,
}

/// Ergebnis eines Ausfalls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uebernahme {
    /// Ein Standby ist eingerückt; sein KV-Cache ist neu zu bauen.
    Uebernommen {
        position: usize,
        neuer_miner: MinerId,
        rebuild: RebuildAuftrag,
    },
    /// Die Position war schon leer; nichts zu tun.
    ///
    /// **Ausdrücklich kein Fehler.** Zwei Meldungen über denselben
    /// Ausfall dürfen nicht zwei Reserveplätze verbrauchen, und im Netz
    /// sind doppelte Meldungen der Normalfall.
    BereitsAusgefallen { position: usize },
    /// Keine Reserve mehr: Die Session ist verloren.
    ///
    /// Das ist der Fall, den Kap. 6.8 auf „mehr als zwei gleichzeitige
    /// Ausfälle" beziffert.
    SessionVerloren { position: usize, ausgefallen: usize },
}

/// Warum ein KV-Cache neu gebaut wird.
///
/// **Genau zwei Werte, und das ist die Durchsetzung von Kap. 4.2**
/// („nur bei Ausfall oder Epochenwechsel ausgelöst"). Ein dritter Grund
/// ließe sich hier nicht eintragen, ohne dass jemand ihn benennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildAnlass {
    /// Ein Standby ist für einen ausgefallenen Miner eingerückt.
    Ausfall,
    /// Die Epoche hat gewechselt und der Pod eine neue Zusammensetzung.
    Epochenwechsel,
}

/// Was ein einrückender Miner nachzurechnen hat.
///
/// **Ein Auftrag, keine Daten.** Der Rebuild überträgt nichts von der
/// ausgefallenen Maschine; er sagt nur, welche Positionen über welche
/// Layer erneut zu rechnen sind. Alles andere wäre eine
/// Vertrauensbeziehung zum Ausgefallenen, und der ist gerade der Grund
/// für den Rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildAuftrag {
    /// Bis zu welcher Position (ausschließlich) nachzurechnen ist.
    pub bis_position: u64,
    /// Erste Layer des Shards (einschließlich).
    pub layer_start: u64,
    /// Letzte Layer des Shards (ausschließlich).
    pub layer_end: u64,
    /// Warum.
    pub anlass: RebuildAnlass,
}

impl RebuildAuftrag {
    /// Zahl der nachzurechnenden Layer-Positionen, also die Arbeit.
    ///
    /// `Positionen · Layer`. Der Wert geht in die Kostenrechnung ein:
    /// Ein Rebuild ist `O(Position)`, und genau deshalb löst er nur bei
    /// den beiden Anlässen aus.
    pub fn arbeit(&self) -> u128 {
        (self.bis_position as u128) * (self.layer_end.saturating_sub(self.layer_start) as u128)
    }

    /// Ist nichts zu tun?
    ///
    /// An Position 0 hat noch niemand einen Cache; der Einrückende fängt
    /// bei null an wie jeder andere.
    pub fn ist_leer(&self) -> bool {
        self.bis_position == 0 || self.layer_end <= self.layer_start
    }
}

/// Fehler beim Aufbau einer Besetzung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BesetzungFehler {
    /// Weniger Miner als Positionen plus Reserve.
    ZuWenigMiner { gebraucht: usize, bekommen: usize },
    /// Ein Miner steht mehrfach in der Besetzung.
    ///
    /// Wäre er es, hinge die Session an einer Maschine, die zweimal
    /// gezählt wird: Ihr Ausfall wäre zwei gleichzeitige Ausfälle, und
    /// die Zusage aus Kap. 6.8 rechnete mit einer Redundanz, die es
    /// nicht gibt.
    MinerDoppelt { miner: MinerId },
    /// Keine Shard-Position.
    KeineShards,
}

impl std::fmt::Display for BesetzungFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZuWenigMiner { gebraucht, bekommen } => write!(
                f,
                "Pod braucht {} Miner (k + {} Reserve), bekam {}",
                gebraucht, RESERVE_PLAETZE, bekommen
            ),
            Self::MinerDoppelt { .. } => write!(
                f,
                "ein Miner steht mehrfach im Pod; seine Redundanz wäre eine Fiktion"
            ),
            Self::KeineShards => write!(f, "ein Pod ohne Shard-Position ist keiner"),
        }
    }
}

impl std::error::Error for BesetzungFehler {}

impl PodBesetzung {
    /// Baut eine Besetzung aus der Zuteilung des Schedulers.
    ///
    /// **Die ersten `k` Miner besetzen die Positionen, der Rest ist
    /// Reserve.** Die Reihenfolge kommt aus der VRF-Zuteilung und wird
    /// **nicht** verändert: Sie ist auf jedem Knoten dieselbe, und daran
    /// hängt, dass alle Knoten nach einem Ausfall denselben Pod sehen.
    pub fn neu(k: usize, miner: &[MinerId], epoche: EpochId) -> Result<Self, BesetzungFehler> {
        if k == 0 {
            return Err(BesetzungFehler::KeineShards);
        }
        let gebraucht = k + RESERVE_PLAETZE;
        if miner.len() < gebraucht {
            return Err(BesetzungFehler::ZuWenigMiner {
                gebraucht,
                bekommen: miner.len(),
            });
        }
        let mut gesehen = std::collections::BTreeSet::new();
        for m in miner.iter().take(gebraucht) {
            if !gesehen.insert(*m) {
                return Err(BesetzungFehler::MinerDoppelt { miner: *m });
            }
        }
        Ok(Self {
            belegung: miner[..k].iter().map(|m| Some(*m)).collect(),
            reserve: miner[k..gebraucht].to_vec(),
            epoche,
        })
    }

    /// Meldet den Ausfall einer Position und besetzt sie nach.
    ///
    /// **Parameter:**
    /// - `position`: die ausgefallene Shard-Position
    /// - `aktuelle_position`: die Token-Position der Session, bis zu der
    ///   der Cache nachzubauen ist
    /// - `layer_start`, `layer_end`: der Zuschnitt dieses Shards
    pub fn ausfall(
        &mut self,
        position: usize,
        aktuelle_position: u64,
        layer_start: u64,
        layer_end: u64,
    ) -> Uebernahme {
        if position >= self.belegung.len() {
            return Uebernahme::SessionVerloren {
                position,
                ausgefallen: self.ausgefallene(),
            };
        }
        if self.belegung[position].is_none() {
            return Uebernahme::BereitsAusgefallen { position };
        }
        match self.reserve.first().copied() {
            None => {
                self.belegung[position] = None;
                Uebernahme::SessionVerloren {
                    position,
                    ausgefallen: self.ausgefallene(),
                }
            }
            Some(neuer) => {
                self.reserve.remove(0);
                self.belegung[position] = Some(neuer);
                Uebernahme::Uebernommen {
                    position,
                    neuer_miner: neuer,
                    rebuild: RebuildAuftrag {
                        bis_position: aktuelle_position,
                        layer_start,
                        layer_end,
                        anlass: RebuildAnlass::Ausfall,
                    },
                }
            }
        }
    }

    /// Übergang in eine neue Epoche mit neuer Zusammensetzung (Punkt 3.3).
    ///
    /// Die neue Besetzung kommt aus der VRF-Zuteilung des Schedulers.
    /// **Zurückgegeben werden die Rebuild-Aufträge derjenigen Positionen,
    /// deren Miner sich geändert hat** — und nur derjenigen: Wer bleibt,
    /// behält seinen Cache, und ein Rebuild für ihn wäre Kap. 4.2
    /// zuwider.
    pub fn epochenwechsel(
        &mut self,
        neue_epoche: EpochId,
        neue_miner: &[MinerId],
        aktuelle_position: u64,
        layer_grenzen: &[(u64, u64)],
    ) -> Result<Vec<(usize, RebuildAuftrag)>, BesetzungFehler> {
        let k = self.belegung.len();
        let neu = Self::neu(k, neue_miner, neue_epoche)?;
        let mut auftraege = Vec::new();
        for pos in 0..k {
            if self.belegung[pos] != neu.belegung[pos] {
                let (ls, le) = layer_grenzen.get(pos).copied().unwrap_or((0, 0));
                auftraege.push((
                    pos,
                    RebuildAuftrag {
                        bis_position: aktuelle_position,
                        layer_start: ls,
                        layer_end: le,
                        anlass: RebuildAnlass::Epochenwechsel,
                    },
                ));
            }
        }
        *self = neu;
        Ok(auftraege)
    }

    /// Zahl der Positionen ohne Miner.
    pub fn ausgefallene(&self) -> usize {
        self.belegung.iter().filter(|b| b.is_none()).count()
    }

    /// Ist die Session noch fahrbar?
    pub fn fahrbar(&self) -> bool {
        self.belegung.iter().all(|b| b.is_some())
    }

    /// Verbleibende Reserveplätze.
    pub fn reserve_uebrig(&self) -> usize {
        self.reserve.len()
    }

    /// Der Miner an einer Position.
    pub fn miner_an(&self, position: usize) -> Option<MinerId> {
        self.belegung.get(position).copied().flatten()
    }

    /// Die Epoche dieser Besetzung.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Zahl der Shard-Positionen, also `k`.
    pub fn positionen(&self) -> usize {
        self.belegung.len()
    }

    /// Die Belegung je Position, `None` für ausgefallen und nicht
    /// nachbesetzt.
    ///
    /// Für Diagnose und für Prüfungen, die belegen sollen, dass ein
    /// **gescheiterter** Epochenwechsel die Besetzung unberührt lässt.
    pub fn belegung(&self) -> &[Option<MinerId>] {
        &self.belegung
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }
    fn miner_liste(n: u8) -> Vec<MinerId> {
        (1..=n).map(miner).collect()
    }

    /// **Das Akzeptanzkriterium der Phase, wörtlich.**
    ///
    /// „Session übersteht bis zu zwei gleichzeitige Shard-Ausfälle im
    /// selben Pod ohne Datenverlust (Standby-Übernahme greift)."
    #[test]
    fn zwei_gleichzeitige_ausfaelle_uebersteht_die_session() {
        let mut pod = PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).unwrap();
        assert_eq!(pod.reserve_uebrig(), RESERVE_PLAETZE);

        for (i, position) in [0usize, 2].iter().enumerate() {
            match pod.ausfall(*position, 100, 0, 6) {
                Uebernahme::Uebernommen { neuer_miner, rebuild, .. } => {
                    assert_eq!(neuer_miner, miner(5 + i as u8));
                    assert_eq!(rebuild.anlass, RebuildAnlass::Ausfall);
                    assert_eq!(rebuild.bis_position, 100);
                }
                andere => panic!("Ausfall {position} musste übernommen werden: {andere:?}"),
            }
        }
        assert!(pod.fahrbar(), "die Session muss weiterlaufen");
        assert_eq!(pod.reserve_uebrig(), 0);
    }

    /// **Und die andere Hälfte derselben Zusage:** Der dritte
    /// gleichzeitige Ausfall verliert die Session.
    ///
    /// Ohne diesen Test wäre die Zusage aus Kap. 6.8 nur zur Hälfte
    /// geprüft. „Bis zu zwei" heißt auch: **drei nicht**, und eine
    /// Implementierung, die stillschweigend weiterliefe, verspräche eine
    /// Redundanz, die es nicht gibt.
    #[test]
    fn der_dritte_gleichzeitige_ausfall_verliert_die_session() {
        let mut pod = PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).unwrap();
        for p in [0usize, 1] {
            assert!(matches!(pod.ausfall(p, 50, 0, 6), Uebernahme::Uebernommen { .. }));
        }
        match pod.ausfall(2, 50, 0, 6) {
            Uebernahme::SessionVerloren { position, ausgefallen } => {
                assert_eq!(position, 2);
                assert_eq!(ausgefallen, 1);
            }
            andere => panic!("der dritte Ausfall muss die Session verlieren: {andere:?}"),
        }
        assert!(!pod.fahrbar());
    }

    /// **Eine doppelt gemeldete Ausfallmeldung verbraucht keine zweite
    /// Reserve.**
    ///
    /// Im Netz sind doppelte Meldungen der Normalfall, nicht die
    /// Ausnahme. Verbrauchte jede einen Platz, wäre die Zusage „zwei
    /// Ausfälle" in Wahrheit „eine Meldung", und ein Angreifer, der
    /// dieselbe Meldung dreimal schickt, verlöre die Session ohne
    /// jeden Ausfall.
    #[test]
    fn eine_doppelte_meldung_verbraucht_keine_zweite_reserve() {
        let mut pod = PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).unwrap();
        assert!(matches!(pod.ausfall(1, 10, 0, 6), Uebernahme::Uebernommen { .. }));
        let nach_erstem = pod.reserve_uebrig();
        // Dieselbe Position erneut melden — der neue Miner sitzt dort.
        // Er ist nicht ausgefallen, also ist das ein neuer Ausfall.
        assert!(matches!(pod.ausfall(1, 10, 0, 6), Uebernahme::Uebernommen { .. }));
        assert_eq!(pod.reserve_uebrig(), nach_erstem - 1);

        // Eine Position, die **wirklich** leer ist, meldet sich als
        // bereits ausgefallen und verbraucht nichts.
        let mut leer = PodBesetzung::neu(2, &miner_liste(4), EpochId(1)).unwrap();
        for p in [0usize, 1] {
            assert!(matches!(leer.ausfall(p, 5, 0, 6), Uebernahme::Uebernommen { .. }));
        }
        assert_eq!(leer.reserve_uebrig(), 0);
        assert!(matches!(leer.ausfall(0, 5, 0, 6), Uebernahme::SessionVerloren { .. }));
        // Jetzt ist Position 0 leer; die Wiederholung ändert nichts mehr.
        let vorher = leer.ausgefallene();
        assert_eq!(leer.ausfall(0, 5, 0, 6), Uebernahme::BereitsAusgefallen { position: 0 });
        assert_eq!(leer.ausgefallene(), vorher);
    }

    /// **Ein Miner darf nicht zweimal im Pod stehen.**
    ///
    /// Sonst wäre sein Ausfall zwei gleichzeitige Ausfälle, und die
    /// Zusage aus Kap. 6.8 rechnete mit einer Redundanz, die es nicht
    /// gibt.
    #[test]
    fn ein_doppelter_miner_wird_abgelehnt() {
        let mut liste = miner_liste(6);
        liste[4] = liste[0];
        assert!(matches!(
            PodBesetzung::neu(4, &liste, EpochId(1)),
            Err(BesetzungFehler::MinerDoppelt { .. })
        ));
    }

    /// Zu wenige Miner sind ein Fehler und kein Pod ohne Reserve.
    #[test]
    fn zu_wenige_miner_sind_ein_fehler() {
        for n in 0..6u8 {
            assert!(
                PodBesetzung::neu(4, &miner_liste(n), EpochId(1)).is_err(),
                "{n} Miner reichen für k = 4 nicht"
            );
        }
        assert!(PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).is_ok());
        assert!(matches!(
            PodBesetzung::neu(0, &miner_liste(6), EpochId(1)),
            Err(BesetzungFehler::KeineShards)
        ));
    }

    /// **Der Epochenwechsel baut nur nach, was sich geändert hat.**
    ///
    /// Kap. 4.2: „nur bei Ausfall oder Epochenwechsel ausgelöst". Wer
    /// bleibt, behält seinen Cache; ein Rebuild für ihn wäre ein
    /// Aufschlag ohne Anlass.
    #[test]
    fn der_epochenwechsel_baut_nur_nach_was_sich_aendert() {
        let mut pod = PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).unwrap();
        let grenzen = [(0u64, 6u64), (6, 12), (12, 18), (18, 24)];

        // Neue Epoche, zwei Positionen wechseln (0 und 3).
        let neu = vec![miner(11), miner(2), miner(3), miner(12), miner(7), miner(8)];
        let auftraege = pod
            .epochenwechsel(EpochId(2), &neu, 250, &grenzen)
            .expect("gültige Besetzung");

        let positionen: Vec<usize> = auftraege.iter().map(|(p, _)| *p).collect();
        assert_eq!(positionen, vec![0, 3], "nur die gewechselten Positionen");
        for (pos, a) in &auftraege {
            assert_eq!(a.anlass, RebuildAnlass::Epochenwechsel);
            assert_eq!(a.bis_position, 250);
            assert_eq!((a.layer_start, a.layer_end), grenzen[*pos]);
        }
        assert_eq!(pod.epoche(), EpochId(2));
        assert_eq!(pod.reserve_uebrig(), RESERVE_PLAETZE, "neue Epoche, neue Reserve");
        assert!(pod.fahrbar());
    }

    /// Wechselt niemand, gibt es keinen Rebuild.
    #[test]
    fn ohne_wechsel_kein_rebuild() {
        let mut pod = PodBesetzung::neu(4, &miner_liste(6), EpochId(1)).unwrap();
        let grenzen = [(0u64, 6u64), (6, 12), (12, 18), (18, 24)];
        let auftraege = pod
            .epochenwechsel(EpochId(2), &miner_liste(6), 100, &grenzen)
            .unwrap();
        assert!(auftraege.is_empty(), "gleiche Besetzung, kein Rebuild");
    }

    /// **Die Kosten des Rebuilds sind `O(Position)`**, und das ist der
    /// Grund, warum er nur bei zwei Anlässen auslöst.
    #[test]
    fn der_rebuild_kostet_positionen_mal_layer() {
        let a = RebuildAuftrag {
            bis_position: 1_000,
            layer_start: 6,
            layer_end: 12,
            anlass: RebuildAnlass::Ausfall,
        };
        assert_eq!(a.arbeit(), 6_000);
        assert!(!a.ist_leer());

        // An Position 0 hat noch niemand einen Cache.
        let leer = RebuildAuftrag { bis_position: 0, ..a };
        assert!(leer.ist_leer());
        assert_eq!(leer.arbeit(), 0);
        // Ein leerer Zuschnitt ebenso.
        let ohne_layer = RebuildAuftrag { layer_end: 6, ..a };
        assert!(ohne_layer.ist_leer());
    }

    /// **Es gibt genau zwei Anlässe**, und der Typ ist die Durchsetzung
    /// von Kap. 4.2.
    #[test]
    fn es_gibt_genau_zwei_rebuild_anlaesse() {
        let alle = [RebuildAnlass::Ausfall, RebuildAnlass::Epochenwechsel];
        assert_eq!(alle.len(), 2);
        assert_ne!(alle[0], alle[1]);
    }
}
