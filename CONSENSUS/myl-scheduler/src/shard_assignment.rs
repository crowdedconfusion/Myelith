//! Pod-Bildung und Shard-Zuweisung (Anhang A.2, Schritte 2 und 3).
//!
//! **Konsens-Feld:** Die Zuweisungsregeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! # ⚑ Was sich am 2026-08-26 geändert hat, und warum (Entscheidung D3)
//!
//! Bis dahin verteilte dieses Modul einen ganzen Cluster auf `num_shards`
//! Shards und legte in **jeden Shard mehrere Miner**. Gemessen: sechs
//! Miner auf vier Shards ergaben `[2, 2, 1, 1]`, acht ergaben
//! `[2, 2, 2, 2]`.
//!
//! **Das widersprach drei anderen Stellen**, und aufgefallen ist es erst,
//! als jemand Scheduler und Pod zusammensteckte (COMPUTE_PIPELINE 3.3):
//!
//! | Quelle | Aussage |
//! |---|---|
//! | Anhang A.2 | `cfg: &ShardConfig, // k Shards, **Pod-Größe k+2**` |
//! | Kap. 6.8 und `myl_pod::PodBesetzung` | ein Miner je Position, dazu zwei in Reserve |
//! | `README/Glossar.md`, Eintrag *Shard* | „den ein **einzelner** Miner im Speicher hält" |
//!
//! Jede Seite war für sich stimmig und vollständig getestet. Genau
//! deshalb konnte der Widerspruch bestehen: Niemand rechnete ihn nach,
//! weil niemand beide Seiten zugleich brauchte.
//!
//! # Wie es jetzt läuft
//!
//! Ein Pod hat **genau `k + 2` Mitglieder**: `k` Shard-Positionen mit je
//! einem Miner, dazu [`RESERVE_JE_POD`] in Reserve.
//!
//! Ein Cluster liefert **so viele vollständige Pods, wie hineinpassen**.
//! Zwölf Miner bei `k = 4` ergeben zwei Pods, nicht einen überfüllten:
//! Mehr Miner heißt mehr Kapazität, nicht mehr Belegung je Position.
//!
//! Was übrig bleibt, steht in [`Zuteilung::ohne_pod`]. **Verluste
//! bekommen eine Zahl**, statt in einer Verzweigung zu verschwinden.
//!
//! # Der Determinismus hängt an zwei Schritten
//!
//! 1. Die Aufteilung eines Clusters in Pods folgt seiner Reihenfolge, und
//!    die stammt aus dem seed-gesteuerten Shuffle in
//!    [`crate::zonenzuteilung::zonen_cluster`].
//! 2. Innerhalb eines Pods verteilt Fisher-Yates die Mitglieder auf die
//!    Positionen, mit einem **je Pod abgeleiteten** Seed.
//!
//! ⚑ **Warum je Pod abgeleitet und nicht der blanke Epochenseed:**
//! `deterministic_shuffle` erzeugt zu einem Seed und einer Länge **immer
//! dieselbe Permutation**. Mit dem blanken Seed landete das dritte
//! Mitglied jedes gleich großen Pods auf derselben Shard-Position. Wer
//! seine Stellung in der Clusterreihenfolge beeinflussen kann, wüsste
//! damit seine Position im Voraus, und die Shard-Zuweisung soll gerade
//! **nicht** vorhersagbar sein (Kap. 4.3, Kollisionsschutz).

use sha2::{Digest, Sha256};

use crate::miner_filter::MinerRegistration;
use myl_types::seed_rng::deterministic_shuffle;

/// Reserveplätze je Pod (Kap. 6.8: „k+2 Mitglieder, 2 in Reserve").
///
/// Deckungsgleich mit `myl_pod::standby::RESERVE_PLAETZE`. Ein Test in
/// `myl-pod` hält beide gegeneinander: Liefen sie auseinander, erzeugte
/// der Scheduler Pods, die kein Pod besetzen kann.
pub const RESERVE_JE_POD: usize = 2;

/// Wie viele Mitglieder ein Pod mit `k` Shards hat.
pub fn pod_groesse(num_shards: u32) -> usize {
    num_shards as usize + RESERVE_JE_POD
}

/// Ein Cluster von Minern, aus dem Pods gebildet werden.
///
/// # ⚑ Notiz zu dem, was hier stand (2026-09-01)
///
/// Bis zum 2026-09-01 lag dieser Typ in `geo_clustering.rs`, zusammen
/// mit `LatencyMatrix` und `form_clusters`: Cluster aus einer
/// **gemessenen** Latenzmatrix, nach Anhang A.2, Schritt 3.
///
/// **Die Entscheidung 3b hat diesen Weg verworfen.** Wer wählt, mit wem
/// er attestiert, formt mit, in welchem Topf er gemischt wird, und
/// erhöht damit seine Chance, **beide Seiten eines Redundanzpaars** zu
/// besetzen; dann verglände Stufe 1 der Verifikation zwei Ergebnisse
/// desselben Betreibers. Cluster entstehen seither je **Zone**, siehe
/// [`crate::zonenzuteilung::zonen_cluster`].
///
/// ⚑ **Der verworfene Code blieb danach noch stehen und wurde weiter
/// gerufen** (Fund 111): `myl_pod::zuteilung::plane_epoche` bildete
/// damit eine zweite, abweichende Zuteilung. **Ein Grund in einem
/// Entwurf hindert niemanden.** Deshalb ist er entfernt und nicht nur
/// abgeraten.
///
/// `max_internal_latency` ist geblieben und steht auf null: Es wird
/// nichts gemessen, und eine erfundene Zahl wäre schlimmer als keine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinerCluster {
    /// Miner in diesem Cluster.
    pub miners: Vec<MinerRegistration>,
    /// Höchste Latenz innerhalb des Clusters, in Millisekunden.
    pub max_internal_latency: u32,
}

/// Eine Shard-Position innerhalb eines Pods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    /// Shard-Index innerhalb des Pods (0-basiert).
    pub shard_index: u32,
    /// Der Miner auf dieser Position.
    ///
    /// **Genau einer.** Bis zum 2026-08-26 stand hier `Vec`, siehe
    /// Modulkopf.
    pub miner: MinerRegistration,
}

/// Ein Pod: `k` besetzte Shard-Positionen und die Reserve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pod {
    /// Pod-Index (0-basiert, über alle Cluster fortlaufend).
    pub pod_index: u32,
    /// Die Shard-Positionen in Indexreihenfolge.
    pub shards: Vec<Shard>,
    /// Die Reserve, in fester Reihenfolge.
    ///
    /// **Reihenfolge ist Konsens-Eigenschaft:** Zwei Knoten, die
    /// verschieden nachbesetzen, kommen zu verschiedenen Pods und damit
    /// zu verschiedenen Spuren.
    pub reserve: Vec<MinerRegistration>,
}

impl Pod {
    /// Alle Mitglieder: erst die Positionen, dann die Reserve.
    ///
    /// Das ist die Reihenfolge, die `myl_pod::PodBesetzung::neu`
    /// erwartet.
    pub fn mitglieder(&self) -> impl Iterator<Item = &MinerRegistration> {
        self.shards.iter().map(|s| &s.miner).chain(self.reserve.iter())
    }

    /// Zahl der Mitglieder, also `k + RESERVE_JE_POD`.
    pub fn groesse(&self) -> usize {
        self.shards.len() + self.reserve.len()
    }
}

/// Das Ergebnis einer Epochenzuteilung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Zuteilung {
    /// Die gebildeten Pods, fortlaufend nummeriert.
    pub pods: Vec<Pod>,
    /// Miner, die in keinen vollständigen Pod passten.
    ///
    /// **Gehört ins Protokoll.** Eine Zuteilung, die Miner
    /// stillschweigend übergeht, sieht aus wie eine, die aufging, und
    /// die Betroffenen warten auf eine Zuweisung, die nie kommt.
    pub ohne_pod: Vec<MinerRegistration>,
}

/// Bildet einen Pod aus genau `pod_groesse(num_shards)` Mitgliedern
/// (Anhang A.2, Schritt 3).
///
/// Fisher-Yates mit einem aus `seed` und `pod_index` abgeleiteten Wert,
/// siehe Modulkopf.
///
/// **Returns:** `None`, wenn `num_shards == 0` ist oder zu wenige
/// Mitglieder übergeben wurden. Ein halber Pod ist kein Pod: Er hätte
/// unbesetzte Positionen, und die Pipeline liefe ins Leere.
pub fn assign_shards(
    mitglieder: &[MinerRegistration],
    num_shards: u32,
    pod_index: u32,
    seed: &[u8; 32],
) -> Option<Pod> {
    if num_shards == 0 || mitglieder.len() < pod_groesse(num_shards) {
        return None;
    }
    let mut gemischt = mitglieder[..pod_groesse(num_shards)].to_vec();
    deterministic_shuffle(&mut gemischt, &pod_seed(seed, pod_index));

    let k = num_shards as usize;
    let shards = gemischt[..k]
        .iter()
        .enumerate()
        .map(|(i, m)| Shard {
            shard_index: i as u32,
            miner: *m,
        })
        .collect();
    Some(Pod {
        pod_index,
        shards,
        reserve: gemischt[k..].to_vec(),
    })
}

/// Bildet aus den Clustern alle vollständigen Pods (Anhang A.2,
/// Schritte 2 und 3).
///
/// Ein Cluster liefert so viele Pods, wie vollständig hineinpassen.
/// Was übrig bleibt, steht in [`Zuteilung::ohne_pod`].
pub fn assign_pods(
    clusters: &[MinerCluster],
    num_shards_per_pod: u32,
    seed: &[u8; 32],
) -> Zuteilung {
    let mut zuteilung = Zuteilung::default();
    if num_shards_per_pod == 0 {
        for c in clusters {
            zuteilung.ohne_pod.extend(c.miners.iter().copied());
        }
        return zuteilung;
    }
    let je_pod = pod_groesse(num_shards_per_pod);
    for cluster in clusters {
        // In der Reihenfolge des Clusters schneiden. Sie stammt aus dem
        // seed-gesteuerten Shuffle der Clusterbildung, ist also bereits
        // kanonisch und unvorhersagbar.
        let mut rest = &cluster.miners[..];
        while rest.len() >= je_pod {
            let index = zuteilung.pods.len() as u32;
            if let Some(pod) = assign_shards(&rest[..je_pod], num_shards_per_pod, index, seed) {
                zuteilung.pods.push(pod);
            }
            rest = &rest[je_pod..];
        }
        zuteilung.ohne_pod.extend(rest.iter().copied());
    }
    zuteilung
}

/// Der Seed eines einzelnen Pods.
///
/// Siehe Modulkopf: Ohne die Ableitung bekäme jeder gleich große Pod
/// dieselbe Permutation.
fn pod_seed(seed: &[u8; 32], pod_index: u32) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MYELITH_POD_SHUFFLE_v1");
    h.update(seed);
    h.update(pod_index.to_le_bytes());
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner_filter::HardwareClass;
    use myl_types::ids::MinerId;

    fn reg(b: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([b; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 5,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        }
    }

    fn cluster(n: u8) -> MinerCluster {
        MinerCluster {
            miners: (0..n).map(reg).collect(),
            max_internal_latency: 50,
        }
    }

    fn saat(b: u8) -> [u8; 32] {
        [b; 32]
    }

    // ── ⚑ Die Aussage von D3 ────────────────────────────────────────

    /// **Ein Miner je Shard-Position, zwei in Reserve.**
    ///
    /// Genau die Zusage aus Anhang A.2 („Pod-Größe k+2") und Kap. 6.8.
    /// Bis zum 2026-08-26 lieferte dieses Modul etwas anderes.
    #[test]
    fn ein_pod_hat_k_positionen_und_zwei_in_reserve() {
        for k in 1u32..=6 {
            let mitglieder: Vec<MinerRegistration> = (0..pod_groesse(k) as u8).map(reg).collect();
            let pod = assign_shards(&mitglieder, k, 0, &saat(7)).expect("Pod");
            assert_eq!(pod.shards.len(), k as usize, "bei k={k}");
            assert_eq!(pod.reserve.len(), RESERVE_JE_POD, "bei k={k}");
            assert_eq!(pod.groesse(), pod_groesse(k));
        }
    }

    /// Jedes Mitglied kommt genau einmal vor.
    ///
    /// Wäre es doppelt, hinge die Session an einer Maschine, die zweimal
    /// gezählt wird: Ihr Ausfall wäre zwei gleichzeitige Ausfälle, und
    /// die Zusage aus Kap. 6.8 rechnete mit einer Redundanz, die es
    /// nicht gibt.
    #[test]
    fn kein_miner_steht_zweimal_im_pod() {
        let mitglieder: Vec<MinerRegistration> = (0..6u8).map(reg).collect();
        let pod = assign_shards(&mitglieder, 4, 0, &saat(7)).expect("Pod");
        let ids: std::collections::BTreeSet<MinerId> =
            pod.mitglieder().map(|m| m.miner_id).collect();
        assert_eq!(ids.len(), 6);
    }

    /// **Mehr Miner heißt mehr Pods, nicht mehr Belegung je Position.**
    #[test]
    fn ein_grosser_cluster_ergibt_mehrere_pods() {
        // k = 4, also sechs Mitglieder je Pod.
        let z = assign_pods(&[cluster(12)], 4, &saat(7));
        assert_eq!(z.pods.len(), 2, "zwölf Miner tragen zwei vollständige Pods");
        assert!(z.ohne_pod.is_empty());
        for pod in &z.pods {
            assert_eq!(pod.shards.len(), 4);
            assert_eq!(pod.reserve.len(), 2);
        }
        // Und die Pods sind disjunkt.
        let a: std::collections::BTreeSet<MinerId> =
            z.pods[0].mitglieder().map(|m| m.miner_id).collect();
        let b: std::collections::BTreeSet<MinerId> =
            z.pods[1].mitglieder().map(|m| m.miner_id).collect();
        assert!(a.is_disjoint(&b), "zwei Pods teilten sich einen Miner");
    }

    /// ⚑ **Übrige Miner werden gezählt, nicht verschwiegen.**
    #[test]
    fn was_nicht_in_einen_pod_passt_steht_im_ergebnis() {
        // Neun Miner, sechs je Pod: ein Pod, drei übrig.
        let z = assign_pods(&[cluster(9)], 4, &saat(7));
        assert_eq!(z.pods.len(), 1);
        assert_eq!(
            z.ohne_pod.len(),
            3,
            "drei Miner passten nirgends hinein, und das muss eine Zahl haben"
        );
        // Kein Miner ist zugleich zugeteilt und übrig.
        let zugeteilt: std::collections::BTreeSet<MinerId> =
            z.pods[0].mitglieder().map(|m| m.miner_id).collect();
        for m in &z.ohne_pod {
            assert!(!zugeteilt.contains(&m.miner_id));
        }
    }

    #[test]
    fn ein_zu_kleiner_cluster_ergibt_keinen_pod() {
        // Fünf Miner bei k=4: einer zu wenig. Ein halber Pod ist kein
        // Pod, seine Positionen blieben unbesetzt.
        let z = assign_pods(&[cluster(5)], 4, &saat(7));
        assert!(z.pods.is_empty());
        assert_eq!(z.ohne_pod.len(), 5);
        assert_eq!(assign_shards(&cluster(5).miners, 4, 0, &saat(7)), None);
    }

    #[test]
    fn genau_k_plus_zwei_geht_auf() {
        let z = assign_pods(&[cluster(6)], 4, &saat(7));
        assert_eq!(z.pods.len(), 1);
        assert!(z.ohne_pod.is_empty());
    }

    // ── Determinismus ───────────────────────────────────────────────

    #[test]
    fn dieselben_eingaben_ergeben_dieselbe_zuteilung() {
        let a = assign_pods(&[cluster(12), cluster(6)], 4, &saat(7));
        let b = assign_pods(&[cluster(12), cluster(6)], 4, &saat(7));
        assert_eq!(a, b);
    }

    #[test]
    fn ein_anderer_seed_ergibt_eine_andere_zuteilung() {
        let a = assign_pods(&[cluster(12)], 4, &saat(7));
        let b = assign_pods(&[cluster(12)], 4, &saat(9));
        assert_ne!(a, b, "der Epochenseed wirkt sich nicht auf die Zuteilung aus");
    }

    /// ⚑ **Zwei gleich große Pods bekommen verschiedene Permutationen.**
    ///
    /// Mit dem blanken Epochenseed landete das dritte Mitglied jedes
    /// gleich großen Pods auf derselben Shard-Position. Wer seine
    /// Stellung in der Clusterreihenfolge beeinflussen kann, wüsste
    /// damit seine Position im Voraus, und die Shard-Zuweisung soll
    /// gerade nicht vorhersagbar sein.
    #[test]
    fn gleich_grosse_pods_werden_verschieden_gemischt() {
        let mitglieder: Vec<MinerRegistration> = (0..6u8).map(reg).collect();
        let a = assign_shards(&mitglieder, 4, 0, &saat(7)).expect("Pod 0");
        let b = assign_shards(&mitglieder, 4, 1, &saat(7)).expect("Pod 1");
        let pos_a: Vec<MinerId> = a.shards.iter().map(|s| s.miner.miner_id).collect();
        let pos_b: Vec<MinerId> = b.shards.iter().map(|s| s.miner.miner_id).collect();
        assert_ne!(
            pos_a, pos_b,
            "dieselben Mitglieder landeten in beiden Pods auf denselben Positionen"
        );
    }

    #[test]
    fn die_shardindizes_laufen_lueckenlos() {
        let pod = assign_shards(&cluster(6).miners, 4, 0, &saat(7)).expect("Pod");
        for (i, s) in pod.shards.iter().enumerate() {
            assert_eq!(s.shard_index, i as u32);
        }
    }

    #[test]
    fn die_podindizes_laufen_ueber_cluster_hinweg_fortlaufend() {
        // Sonst trügen zwei Pods aus verschiedenen Clustern denselben
        // Index, und die Redundanzpaarung verwechselte sie.
        let z = assign_pods(&[cluster(12), cluster(6)], 4, &saat(7));
        assert_eq!(z.pods.len(), 3);
        for (i, p) in z.pods.iter().enumerate() {
            assert_eq!(p.pod_index, i as u32);
        }
    }

    // ── Randfälle ───────────────────────────────────────────────────

    #[test]
    fn null_shards_ergeben_keinen_pod() {
        let z = assign_pods(&[cluster(12)], 0, &saat(7));
        assert!(z.pods.is_empty());
        assert_eq!(z.ohne_pod.len(), 12, "die Miner dürfen nicht verschwinden");
        assert_eq!(assign_shards(&cluster(12).miners, 0, 0, &saat(7)), None);
    }

    #[test]
    fn leere_cluster_ergeben_eine_leere_zuteilung() {
        assert_eq!(assign_pods(&[], 4, &saat(7)), Zuteilung::default());
        assert_eq!(assign_pods(&[cluster(0)], 4, &saat(7)), Zuteilung::default());
    }

    #[test]
    fn mehr_shards_als_miner_ergibt_keinen_pod() {
        let z = assign_pods(&[cluster(4)], 8, &saat(7));
        assert!(z.pods.is_empty());
        assert_eq!(z.ohne_pod.len(), 4);
    }
}
