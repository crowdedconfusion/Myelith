//! Nachbesetzung aus der Reserve des Netzes, nicht nur des Pods (Punkt 3.4).
//!
//! # Warum die Pod-Reserve nicht reicht
//!
//! Kap. 6.8 gibt jedem Pod zwei Reserveplätze und sagt zu:
//! „Session-Verlust **nur bei mehr als zwei gleichzeitigen Ausfällen**".
//! Beim dritten Ausfall ist die Sitzung verloren, auch wenn im Netz
//! hundert freie Miner stehen: [`crate::standby::PodBesetzung::ausfall`]
//! kennt nur die eigene Reserve.
//!
//! Die freien Miner gibt es bereits, und sie sind sogar schon benannt:
//! `myl_scheduler::Zuteilung::ohne_pod` führt jeden registrierten Miner,
//! der in keinen vollständigen Pod passte. Bisher wartete er auf eine
//! Zuweisung, die nie kam.
//!
//! # ⚑ Verteilt und nicht gemischt
//!
//! Der naheliegende Entwurf gibt jedem Pod eine **gemischte Liste** der
//! Netzreserve und lässt ihn beim Ausfall vorne zugreifen. Der Entwurf
//! ist falsch, und der Grund ist keiner der Wahrscheinlichkeit:
//!
//! **Zwei Pods, die gleichzeitig ausfallen, greifen dann nach demselben
//! Miner.** Bei verschiedenen Mischungen ist das selten, und „selten"
//! heißt im Konsens „es passiert, und dann sitzen zwei Knoten mit
//! verschiedenen Besetzungen da". Ab da rechnen sie verschiedene Spuren.
//!
//! Deshalb wird die Netzreserve **aufgeteilt**: Jeder freie Miner gehört
//! zu höchstens einem Pod. Eine Kollision ist damit nicht
//! unwahrscheinlich, sondern **unmöglich**, und das ist der Unterschied,
//! auf den es ankommt.
//!
//! # ⚑ Und die Disjunktheit hält von selbst
//!
//! `myl_scheduler::redundancy::pods_are_disjoint` sorgt dafür, dass die
//! beiden Pods eines Redundanzpaars keine gemeinsame Maschine haben:
//! Stünde dieselbe Maschine auf beiden Seiten, verglände Stufe 1 der
//! Verifikation zwei Ergebnisse derselben Maschine, also eine
//! Selbstbestätigung.
//!
//! **Eine Nachbesetzung zur Laufzeit könnte genau das wieder einreißen**,
//! und zwar unbemerkt, weil die Disjunktheit bei der Zuteilung geprüft
//! wurde und nicht danach. Durch die Aufteilung kann ein freier Miner
//! nur in **einen** Pod einrücken; die Prüfung von damals gilt also
//! weiter. Ein Test hält das fest.
//!
//! # ⚑ Was ein größerer Vorrat auch bedeutet
//!
//! Die Netzreserve macht den Pod langlebiger und den Missbrauch der
//! Ausfallmeldung **teurer zu übersehen**. Eine wiederholte Meldung über
//! dieselbe Position zieht jedes Mal den nächsten Vorrat: Nach einer
//! geglückten Übernahme sitzt wieder jemand dort, und „der ist
//! ausgefallen" ist dann eine neue Aussage über eine neue Person. Die
//! Entprellung aus Punkt 3.1 greift dagegen nicht, sie gilt nur für eine
//! **leere** Position.
//!
//! Vorher endete das nach zwei Meldungen mit dem Sitzungsverlust, jetzt
//! endet es nach zwei plus der Netzreserve. **Das ist kein neues Leck,
//! sondern dasselbe mit größerem Eimer**, und es steht hier, weil ein
//! größerer Eimer wie eine Lösung aussieht. Ein Test hält den Verlauf
//! fest (`wiederholte_meldungen_leeren_den_vorrat`).
//!
//! # ⚑ Was ein größerer Vorrat auch bedeutet
//!
//! Die Netzreserve macht den Pod langlebiger und den Missbrauch der
//! Ausfallmeldung **teurer zu übersehen**. Eine wiederholte Meldung über
//! dieselbe Position zieht jedes Mal den nächsten Vorrat: Nach einer
//! geglückten Übernahme sitzt wieder jemand dort, und „der ist
//! ausgefallen" ist dann eine neue Aussage über eine neue Person. Die
//! Entprellung aus Punkt 3.1 greift dagegen nicht, sie gilt nur für eine
//! **leere** Position.
//!
//! Vorher endete das nach zwei Meldungen mit dem Sitzungsverlust, jetzt
//! endet es nach zwei plus der Netzreserve. **Das ist kein neues Leck,
//! sondern dasselbe mit größerem Eimer**, und es steht hier, weil ein
//! größerer Eimer wie eine Lösung aussieht. Ein Test hält den Verlauf
//! fest (`wiederholte_meldungen_leeren_den_vorrat`).
//!
//! # ⚑ Wer wiederholt ausfällt, kommt später dran (Punkt 3.6)
//!
//! Der Ledger führt seit dem 2026-08-27 einen Verstoß-Zähler je Konto
//! (`verstoesse_im_fenster`). Er entstand für die Slashing-Staffelung
//! und trägt hier ein zweites Mal: [`Netzreserve::verteilen_gestaffelt`]
//! ordnet die freien Miner **nach ihrer Auffälligkeit im Fenster** und
//! erst danach nach der Mischung. Wer sauber ist, steht vorn.
//!
//! **Sortiert und nicht ausgeschlossen**, und das ist eine Entscheidung
//! gegen die naheliegende. Eine Schwelle („ab drei Verstößen keine
//! Reserve mehr") wäre eine Klippe: Wer sie überschreitet, ist draußen,
//! und wer knapp darunter liegt, ist so gut wie ein Fehlerfreier. Eine
//! Ordnung wirkt stetig und ohne Rand, an dem sich rechnen ließe.
//!
//! ⚑ **Und sie schließt niemanden aus, auch nicht den Auffälligsten.**
//! Läuft der Vorrat leer, rückt auch er ein, statt dass die Sitzung
//! verloren geht. Das ist bewusst so: **Am Rand schlägt Liveness die
//! Bestrafung**, denn ein verlorener Pod schadet den Anfragenden, der
//! Zähler schadet nur dem Auffälligen. Wer das anders will, braucht eine
//! Schwelle, und die hätte den Rand, den diese Ordnung nicht hat.
//!
//! **Die Zahlen kommen von außen.** Dieses Modul liest den Ledger nicht;
//! `myl-pod` hängt nicht daran und soll es nicht. Der Aufrufer bildet
//! `MinerId` auf die Zahl ab, die `verstoesse_im_fenster` für die
//! zugehörige Adresse liefert.
//!
//! # Was hier nicht entschieden wird
//!
//! Wer einen Ausfall **melden** darf und wie er gegengezeichnet wird,
//! steht in Punkt 3.5 und ist eine eigene Frage: Eine Ausfallmeldung ist
//! eine Waffe, denn wer melden darf, dass ein anderer ausgefallen sei,
//! kann einen ehrlichen Knoten aus seinem Pod werfen. Dieses Modul
//! beantwortet allein die Frage, **wer einrückt, wenn feststeht, dass
//! jemand fehlt**.

use std::collections::{BTreeMap, BTreeSet};

use myl_types::ids::{EpochId, MinerId};
use myl_types::seed_rng::deterministic_shuffle;

/// Trennstring für die Ableitung der Reserve-Aufteilung.
///
/// Eigener Trennstring, damit die Mischung nicht dieselbe ist wie die
/// der Pod-Bildung: Aus demselben Epochenseed zweimal dieselbe
/// Reihenfolge zu ziehen hieße, eine Regelmäßigkeit einzubauen, die
/// niemand gewollt hat.
pub const DST_NETZRESERVE: &[u8] = b"MYELITH_NETZRESERVE_v1";

/// Die Netzreserve einer Epoche, auf die Pods aufgeteilt.
///
/// Jeder Eintrag gehört zu genau einem Pod; die Listen sind paarweise
/// disjunkt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Netzreserve {
    /// Je Pod-Index die ihm zugeteilten freien Miner, in Zugriffsfolge.
    je_pod: Vec<Vec<MinerId>>,
}

impl Netzreserve {
    /// Teilt die freien Miner deterministisch auf `pod_anzahl` Pods auf.
    ///
    /// Reihum, über eine aus `seed` und [`DST_NETZRESERVE`] gemischte
    /// Liste. Reihum und nicht blockweise, damit ein Pod nicht deshalb
    /// leer ausgeht, weil er hinten steht: Bei drei freien Minern und
    /// fünf Pods bekommen die ersten drei je einen, blockweise bekäme
    /// der erste alle drei.
    ///
    /// **Doppelte Einträge in `frei` werden verworfen**, nicht
    /// zusammengeführt: Ein Miner, der zweimal in der Netzreserve steht,
    /// könnte sonst in zwei Pods einrücken, und genau das soll die
    /// Aufteilung ausschließen.
    pub fn verteilen(
        frei: &[MinerId],
        pod_anzahl: usize,
        seed: &[u8; 32],
        epoche: EpochId,
    ) -> Self {
        let mut je_pod = vec![Vec::new(); pod_anzahl];
        if pod_anzahl == 0 {
            return Self { je_pod };
        }
        let mut gesehen: BTreeSet<MinerId> = BTreeSet::new();
        let mut liste: Vec<MinerId> = frei
            .iter()
            .copied()
            .filter(|m| gesehen.insert(*m))
            .collect();
        deterministic_shuffle(&mut liste, &Self::mischsaat(seed, epoche));
        for (i, m) in liste.into_iter().enumerate() {
            je_pod[i % pod_anzahl].push(m);
        }
        Self { je_pod }
    }

    /// Wie [`Self::verteilen`], ordnet aber Auffällige nach hinten
    /// (Punkt 3.6).
    ///
    /// `verstoesse` bildet einen Miner auf seine Zahl im
    /// Beobachtungsfenster ab; wer fehlt, gilt als unauffällig. Die
    /// Ordnung ist **stabil über der Mischung**: Innerhalb derselben
    /// Zahl entscheidet weiterhin der Seed, zwischen verschiedenen Zahlen
    /// die kleinere.
    ///
    /// Der Grund für „ordnen" statt „ausschließen" steht im Modulkopf.
    pub fn verteilen_gestaffelt(
        frei: &[MinerId],
        pod_anzahl: usize,
        seed: &[u8; 32],
        epoche: EpochId,
        verstoesse: &BTreeMap<MinerId, u64>,
    ) -> Self {
        let mut je_pod = vec![Vec::new(); pod_anzahl];
        if pod_anzahl == 0 {
            return Self { je_pod };
        }
        let mut gesehen: BTreeSet<MinerId> = BTreeSet::new();
        let mut liste: Vec<MinerId> = frei
            .iter()
            .copied()
            .filter(|m| gesehen.insert(*m))
            .collect();
        deterministic_shuffle(&mut liste, &Self::mischsaat(seed, epoche));
        // ⚑ Stabil: Gleiche Auffälligkeit behält die Reihenfolge der
        // Mischung. Eine unstabile Sortierung machte die Zuteilung von
        // der Bauart der Standardbibliothek abhängig, und zwei Knoten
        // mit verschiedenen Rust-Fassungen kämen zu verschiedenen Pods.
        liste.sort_by_key(|m| *verstoesse.get(m).unwrap_or(&0));
        for (i, m) in liste.into_iter().enumerate() {
            je_pod[i % pod_anzahl].push(m);
        }
        Self { je_pod }
    }

    /// Der Saatwert der Mischung: Epochenseed, Trennstring, Epoche.
    ///
    /// Die Epoche gehört hinein, weil derselbe Seed sonst über einen
    /// Epochenwechsel hinweg dieselbe Aufteilung ergäbe.
    fn mischsaat(seed: &[u8; 32], epoche: EpochId) -> [u8; 32] {
        let mut stoff = Vec::with_capacity(32 + DST_NETZRESERVE.len() + 8);
        stoff.extend_from_slice(seed);
        stoff.extend_from_slice(DST_NETZRESERVE);
        stoff.extend_from_slice(&epoche.0.to_le_bytes());
        myl_types::hash::Hash::sha256(&stoff).0
    }

    /// Die Reserve eines Pods, in Zugriffsfolge.
    pub fn fuer_pod(&self, pod_index: usize) -> &[MinerId] {
        self.je_pod.get(pod_index).map(|v| &v[..]).unwrap_or(&[])
    }

    /// Nimmt den nächsten freien Miner dieses Pods heraus.
    ///
    /// `None`, wenn dieser Pod keine Netzreserve (mehr) hat. **Kein
    /// Ausweichen auf die Liste eines anderen Pods**, denn genau das
    /// wäre die Kollision, die die Aufteilung ausschließt.
    pub fn entnehmen(&mut self, pod_index: usize) -> Option<MinerId> {
        let liste = self.je_pod.get_mut(pod_index)?;
        if liste.is_empty() {
            None
        } else {
            Some(liste.remove(0))
        }
    }

    /// Wie viele Pods bedacht wurden.
    pub fn pods(&self) -> usize {
        self.je_pod.len()
    }

    /// Zahl der noch verfügbaren freien Miner über alle Pods.
    pub fn uebrig(&self) -> usize {
        self.je_pod.iter().map(|v| v.len()).sum()
    }

    /// Pods, die keine einzige Netzreserve bekommen haben.
    ///
    /// ⚑ **Gehört ins Ergebnis und nicht ins Schweigen.** Ein Pod ohne
    /// zusätzliche Reserve verliert seine Sitzung beim dritten Ausfall
    /// wie zuvor. Wer das nicht weiß, hält die Liveness-Zusage für
    /// stärker, als sie in dieser Epoche ist.
    pub fn pods_ohne_reserve(&self) -> Vec<usize> {
        self.je_pod
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_empty())
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn frei(von: u8, bis: u8) -> Vec<MinerId> {
        (von..=bis).map(miner).collect()
    }

    /// ⚑ **Der Kern: Kein freier Miner gehört zwei Pods.** Nicht selten,
    /// sondern nie, und zwar durch die Bauart der Aufteilung.
    #[test]
    fn kein_freier_miner_gehoert_zwei_pods() {
        let r = Netzreserve::verteilen(&frei(1, 30), 7, &[3u8; 32], EpochId(9));
        let mut gesehen = BTreeSet::new();
        for p in 0..r.pods() {
            for m in r.fuer_pod(p) {
                assert!(gesehen.insert(*m), "{m:?} steht in zwei Pods");
            }
        }
        assert_eq!(gesehen.len(), 30, "es sind Miner verloren gegangen");
    }

    /// Dieselbe Eingabe ergibt dieselbe Aufteilung. Zwei Knoten, die
    /// verschieden nachbesetzen, rechnen verschiedene Spuren.
    #[test]
    fn die_aufteilung_ist_wiederholbar() {
        let a = Netzreserve::verteilen(&frei(1, 20), 5, &[7u8; 32], EpochId(4));
        let b = Netzreserve::verteilen(&frei(1, 20), 5, &[7u8; 32], EpochId(4));
        assert_eq!(a, b);
    }

    /// ⚑ Ein anderer Seed ergibt eine andere Aufteilung, sonst hinge sie
    /// nicht am Seed.
    #[test]
    fn ein_anderer_seed_teilt_anders_auf() {
        let a = Netzreserve::verteilen(&frei(1, 20), 5, &[7u8; 32], EpochId(4));
        let b = Netzreserve::verteilen(&frei(1, 20), 5, &[8u8; 32], EpochId(4));
        assert_ne!(a, b);
    }

    /// ⚑ Und eine andere Epoche ebenfalls: Derselbe Seed über einen
    /// Epochenwechsel hinweg ergäbe sonst dieselbe Aufteilung.
    #[test]
    fn eine_andere_epoche_teilt_anders_auf() {
        let a = Netzreserve::verteilen(&frei(1, 20), 5, &[7u8; 32], EpochId(4));
        let b = Netzreserve::verteilen(&frei(1, 20), 5, &[7u8; 32], EpochId(5));
        assert_ne!(a, b);
    }

    /// Reihum, nicht blockweise: Bei drei Freien und fünf Pods bekommen
    /// drei Pods je einen, statt dass einer alle drei bekommt.
    #[test]
    fn wenige_freie_verteilen_sich_auf_viele_pods() {
        let r = Netzreserve::verteilen(&frei(1, 3), 5, &[1u8; 32], EpochId(1));
        let mit: Vec<usize> = (0..r.pods()).filter(|p| !r.fuer_pod(*p).is_empty()).collect();
        assert_eq!(mit.len(), 3, "die drei landeten nicht bei drei Pods");
        assert!(r.fuer_pod(mit[0]).len() == 1);
    }

    /// Pods ohne Netzreserve werden genannt, nicht verschwiegen.
    #[test]
    fn pods_ohne_reserve_werden_genannt() {
        let r = Netzreserve::verteilen(&frei(1, 2), 5, &[1u8; 32], EpochId(1));
        assert_eq!(r.pods_ohne_reserve().len(), 3);
        assert_eq!(r.uebrig(), 2);
    }

    /// Ein doppelt geführter Miner wird verworfen, nicht zweimal vergeben.
    #[test]
    fn ein_doppelter_eintrag_wird_verworfen() {
        let doppelt = vec![miner(1), miner(2), miner(1), miner(3)];
        let r = Netzreserve::verteilen(&doppelt, 2, &[5u8; 32], EpochId(2));
        assert_eq!(r.uebrig(), 3, "der doppelte Eintrag wurde mitgezaehlt");
    }

    /// Entnehmen gibt der Reihe nach heraus und dann nichts mehr.
    #[test]
    fn entnehmen_erschoepft_die_eigene_liste() {
        let mut r = Netzreserve::verteilen(&frei(1, 4), 2, &[2u8; 32], EpochId(3));
        let erwartet: Vec<MinerId> = r.fuer_pod(0).to_vec();
        for e in &erwartet {
            assert_eq!(r.entnehmen(0), Some(*e));
        }
        assert_eq!(r.entnehmen(0), None);
    }

    /// ⚑ **Kein Ausweichen auf einen fremden Vorrat.** Pod 0 ist leer,
    /// Pod 1 hat noch etwas, und Pod 0 bekommt trotzdem nichts.
    #[test]
    fn ein_leerer_pod_greift_nicht_in_die_liste_des_anderen() {
        let mut r = Netzreserve::verteilen(&frei(1, 2), 2, &[2u8; 32], EpochId(3));
        assert!(r.entnehmen(0).is_some());
        assert_eq!(r.entnehmen(0), None);
        assert!(r.entnehmen(1).is_some(), "Pod 1 hatte noch einen");
    }

    /// Ohne Pods gibt es nichts zu verteilen, und das ist kein Absturz.
    #[test]
    fn ohne_pods_wird_nichts_verteilt() {
        let r = Netzreserve::verteilen(&frei(1, 5), 0, &[1u8; 32], EpochId(1));
        assert_eq!(r.pods(), 0);
        assert_eq!(r.uebrig(), 0);
        assert_eq!(r.fuer_pod(0), &[] as &[MinerId]);
    }

    /// Ein Pod-Index jenseits der Liste liefert nichts, statt zu stürzen.
    #[test]
    fn ein_unbekannter_pod_bekommt_nichts() {
        let mut r = Netzreserve::verteilen(&frei(1, 4), 2, &[1u8; 32], EpochId(1));
        assert_eq!(r.fuer_pod(99), &[] as &[MinerId]);
        assert_eq!(r.entnehmen(99), None);
    }
}

#[cfg(test)]
mod staffelung_tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn frei(von: u8, bis: u8) -> Vec<MinerId> {
        (von..=bis).map(miner).collect()
    }

    /// Alle Positionen über alle Pods, in Zugriffsfolge zusammengelegt.
    fn zugriffsfolge(r: &Netzreserve) -> Vec<MinerId> {
        // Reihum eingefüllt, also gibt Reihum-Auslesen die Ordnung zurück.
        let laenge = r.je_pod.iter().map(|v| v.len()).max().unwrap_or(0);
        let mut out = Vec::new();
        for i in 0..laenge {
            for p in &r.je_pod {
                if let Some(m) = p.get(i) {
                    out.push(*m);
                }
            }
        }
        out
    }

    /// ⚑ **Der Kern von Punkt 3.6: Wer auffiel, kommt später dran.**
    #[test]
    fn wer_auffiel_kommt_spaeter_dran() {
        let alle = frei(1, 8);
        let mut v = BTreeMap::new();
        // Die zweite Hälfte ist auffällig.
        for m in &alle[4..] {
            v.insert(*m, 3u64);
        }
        let r = Netzreserve::verteilen_gestaffelt(&alle, 2, &[5u8; 32], EpochId(1), &v);
        let folge = zugriffsfolge(&r);
        assert_eq!(folge.len(), 8);
        let sauber: BTreeSet<MinerId> = alle[..4].iter().copied().collect();
        for m in &folge[..4] {
            assert!(sauber.contains(m), "ein Auffälliger stand vorn: {m:?}");
        }
    }

    /// Die Ordnung ist stetig, nicht stufig: Ein Verstoß rangiert vor
    /// zweien, zwei vor dreien.
    #[test]
    fn die_ordnung_ist_stetig() {
        let alle = frei(1, 4);
        let v: BTreeMap<MinerId, u64> = alle
            .iter()
            .enumerate()
            .map(|(i, m)| (*m, i as u64))
            .collect();
        let r = Netzreserve::verteilen_gestaffelt(&alle, 1, &[9u8; 32], EpochId(1), &v);
        assert_eq!(r.fuer_pod(0), &alle[..], "die Ordnung folgt nicht den Zahlen");
    }

    /// ⚑ **Niemand wird ausgeschlossen.** Läuft der Vorrat leer, rückt
    /// auch der Auffälligste ein: Am Rand schlägt Liveness die
    /// Bestrafung.
    #[test]
    fn auch_der_auffaelligste_rueckt_ein() {
        let alle = frei(1, 3);
        let v: BTreeMap<MinerId, u64> = alle.iter().map(|m| (*m, 99u64)).collect();
        let mut r = Netzreserve::verteilen_gestaffelt(&alle, 1, &[9u8; 32], EpochId(1), &v);
        assert_eq!(r.uebrig(), 3, "Auffaellige wurden weggelassen");
        assert!(r.entnehmen(0).is_some());
    }

    /// Innerhalb derselben Zahl entscheidet weiterhin der Seed.
    #[test]
    fn bei_gleicher_zahl_entscheidet_der_seed() {
        let alle = frei(1, 8);
        let v: BTreeMap<MinerId, u64> = alle.iter().map(|m| (*m, 2u64)).collect();
        let a = Netzreserve::verteilen_gestaffelt(&alle, 2, &[1u8; 32], EpochId(1), &v);
        let b = Netzreserve::verteilen_gestaffelt(&alle, 2, &[2u8; 32], EpochId(1), &v);
        assert_ne!(a, b, "der Seed wirkt nicht mehr");
        // Und ohne Verstoesse ist es dieselbe Verteilung wie zuvor.
        let ohne = Netzreserve::verteilen(&alle, 2, &[1u8; 32], EpochId(1));
        assert_eq!(a, ohne, "gleiche Zahlen ergeben eine andere Ordnung als gar keine");
    }

    /// Auch gestaffelt gehört kein Miner zwei Pods.
    #[test]
    fn auch_gestaffelt_gehoert_niemand_zwei_pods() {
        let alle = frei(1, 30);
        let v: BTreeMap<MinerId, u64> = alle.iter().enumerate().map(|(i, m)| (*m, (i % 4) as u64)).collect();
        let r = Netzreserve::verteilen_gestaffelt(&alle, 7, &[3u8; 32], EpochId(9), &v);
        let mut gesehen = BTreeSet::new();
        for p in 0..r.pods() {
            for m in r.fuer_pod(p) {
                assert!(gesehen.insert(*m), "{m:?} steht in zwei Pods");
            }
        }
        assert_eq!(gesehen.len(), 30);
    }

    /// Wer nicht in der Liste steht, gilt als unauffällig.
    #[test]
    fn wer_fehlt_gilt_als_unauffaellig() {
        let alle = frei(1, 4);
        let mut v = BTreeMap::new();
        v.insert(miner(1), 5u64);
        let r = Netzreserve::verteilen_gestaffelt(&alle, 1, &[7u8; 32], EpochId(1), &v);
        let folge = r.fuer_pod(0);
        assert_eq!(*folge.last().expect("nicht leer"), miner(1), "der Auffaellige steht nicht hinten");
    }

    /// Wiederholbar, wie die ungestaffelte Verteilung auch.
    #[test]
    fn die_gestaffelte_aufteilung_ist_wiederholbar() {
        let alle = frei(1, 12);
        let v: BTreeMap<MinerId, u64> = alle.iter().enumerate().map(|(i, m)| (*m, (i % 3) as u64)).collect();
        let a = Netzreserve::verteilen_gestaffelt(&alle, 3, &[4u8; 32], EpochId(2), &v);
        let b = Netzreserve::verteilen_gestaffelt(&alle, 3, &[4u8; 32], EpochId(2), &v);
        assert_eq!(a, b);
    }
}
