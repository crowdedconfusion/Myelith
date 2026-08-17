//! Shard-Zuweisung innerhalb des Pods: Fisher-Yates mit Seed (Anhang A.2, Schritt 4).
//!
//! Die Miner in einem Cluster werden in Shards aufgeteilt. Fisher-Yates Shuffle
//! mit dem Seed sorgt für deterministische, gleichverteilte Zuweisung.
//!
//! **Konsens-Feld:** Die Zuweisungsregeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Jeder Cluster wird in eine feste Anzahl von Shards aufgeteilt.
//! Die Miner werden mit Fisher-Yates shuffle deterministisch zugewiesen.
//! Jeder Shard bekommt eine gleichmäßige Anzahl von Minern (±1 bei ungerader Teilung).

use myl_types::seed_rng::deterministic_shuffle;
use crate::geo_clustering::MinerCluster;
use crate::miner_filter::MinerRegistration;

/// Ein Shard innerhalb eines Pods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    /// Shard-Index innerhalb des Pods (0-basiert).
    pub shard_index: u32,
    /// Miner in diesem Shard.
    pub miners: Vec<MinerRegistration>,
}

/// Ein Pod besteht aus mehreren Shards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pod {
    /// Pod-Index (0-basiert).
    pub pod_index: u32,
    /// Shards in diesem Pod.
    pub shards: Vec<Shard>,
}

/// Weist Miner in einem Cluster auf Shards auf.
///
/// **Algorithmus (Anhang A.2, Schritt 4):**
/// 1. Nimm die Miner des Clusters
/// 2. Shuffle sie mit Fisher-Yates und dem Seed (deterministisch)
/// 3. Teile sie gleichmäßig auf `num_shards` Shards auf
/// 4. Gib den Pod mit allen Shards zurück
///
/// **Determinismus:** Gleicher Seed + gleiche Miner → gleiche Zuweisung.
///
/// **Parameter:**
/// - `cluster`: Cluster von Minern (aus Phase 2.3)
/// - `num_shards`: Anzahl der Shards pro Pod
/// - `pod_index`: Index des Pods (für Identifikation)
/// - `seed`: Epochenseed (aus Phase 2.1) für deterministisches Shuffling
///
/// **Returns:** Pod mit allen Shards
pub fn assign_shards(
    cluster: &MinerCluster,
    num_shards: u32,
    pod_index: u32,
    seed: &[u8; 32],
) -> Pod {
    if num_shards == 0 {
        return Pod {
            pod_index,
            shards: vec![],
        };
    }

    // Fisher-Yates Shuffle mit Seed
    let mut shuffled_miners = cluster.miners.clone();
    deterministic_shuffle(&mut shuffled_miners, seed);

    // Teile Miner gleichmäßig auf Shards auf
    let num_miners = shuffled_miners.len();
    let base_per_shard = num_miners / num_shards as usize;
    let remainder = num_miners % num_shards as usize;

    let mut shards = Vec::with_capacity(num_shards as usize);
    let mut offset = 0;

    for shard_idx in 0..num_shards {
        // Erste `remainder` Shards bekommen einen Miner mehr
        let shard_size = base_per_shard + if (shard_idx as usize) < remainder { 1 } else { 0 };
        
        let shard_miners = shuffled_miners[offset..offset + shard_size].to_vec();
        offset += shard_size;

        shards.push(Shard {
            shard_index: shard_idx,
            miners: shard_miners,
        });
    }

    Pod {
        pod_index,
        shards,
    }
}


/// Weist mehrere Cluster auf mehrere Pods auf.
///
/// **Parameter:**
/// - `clusters`: Liste von Clustern (aus Phase 2.3)
/// - `num_shards_per_pod`: Anzahl der Shards pro Pod
/// - `seed`: Epochenseed (aus Phase 2.1)
///
/// **Returns:** Liste von Pods, einer pro Cluster
pub fn assign_pods(
    clusters: &[MinerCluster],
    num_shards_per_pod: u32,
    seed: &[u8; 32],
) -> Vec<Pod> {
    clusters
        .iter()
        .enumerate()
        .map(|(idx, cluster)| assign_shards(cluster, num_shards_per_pod, idx as u32, seed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner_filter::HardwareClass;
    use myl_types::ids::MinerId;

    fn test_registration(miner_byte: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([miner_byte; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 5,
        }
    }

    fn test_cluster(miner_bytes: &[u8]) -> MinerCluster {
        MinerCluster {
            miners: miner_bytes.iter().map(|&b| test_registration(b)).collect(),
            max_internal_latency: 50,
        }
    }

    #[test]
    fn assign_shards_basic() {
        let cluster = test_cluster(&[1, 2, 3, 4]);
        let seed = [0u8; 32];

        let pod = assign_shards(&cluster, 2, 0, &seed);

        assert_eq!(pod.pod_index, 0);
        assert_eq!(pod.shards.len(), 2);
        assert_eq!(pod.shards[0].miners.len(), 2);
        assert_eq!(pod.shards[1].miners.len(), 2);
    }

    #[test]
    fn assign_shards_uneven_distribution() {
        let cluster = test_cluster(&[1, 2, 3, 4, 5]);
        let seed = [0u8; 32];

        let pod = assign_shards(&cluster, 2, 0, &seed);

        // 5 Miner auf 2 Shards: einer bekommt 3, einer bekommt 2
        assert_eq!(pod.shards.len(), 2);
        let total: usize = pod.shards.iter().map(|s| s.miners.len()).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn assign_shards_deterministic() {
        let cluster = test_cluster(&[1, 2, 3, 4]);
        let seed = [42u8; 32];

        let pod1 = assign_shards(&cluster, 2, 0, &seed);
        let pod2 = assign_shards(&cluster, 2, 0, &seed);

        assert_eq!(pod1, pod2);
    }

    #[test]
    fn assign_shards_different_seeds() {
        let cluster = test_cluster(&[1, 2, 3, 4]);

        let pod1 = assign_shards(&cluster, 2, 0, &[1u8; 32]);
        let pod2 = assign_shards(&cluster, 2, 0, &[2u8; 32]);

        // Unterschiedliche Seeds führen zu unterschiedlichen Zuweisungen
        // (nicht garantiert, aber wahrscheinlich)
        assert_eq!(pod1.shards.len(), pod2.shards.len());
    }

    #[test]
    fn assign_shards_zero_shards() {
        let cluster = test_cluster(&[1, 2, 3]);
        let seed = [0u8; 32];

        let pod = assign_shards(&cluster, 0, 0, &seed);

        assert_eq!(pod.shards.len(), 0);
    }

    #[test]
    fn assign_shards_more_shards_than_miners() {
        let cluster = test_cluster(&[1, 2]);
        let seed = [0u8; 32];

        let pod = assign_shards(&cluster, 5, 0, &seed);

        // 2 Miner auf 5 Shards: 2 Shards bekommen je 1 Miner, 3 sind leer
        assert_eq!(pod.shards.len(), 5);
        let non_empty: Vec<_> = pod.shards.iter().filter(|s| !s.miners.is_empty()).collect();
        assert_eq!(non_empty.len(), 2);
    }

    #[test]
    fn assign_pods_basic() {
        let clusters = vec![
            test_cluster(&[1, 2, 3, 4]),
            test_cluster(&[5, 6, 7, 8]),
        ];
        let seed = [0u8; 32];

        let pods = assign_pods(&clusters, 2, &seed);

        assert_eq!(pods.len(), 2);
        assert_eq!(pods[0].pod_index, 0);
        assert_eq!(pods[1].pod_index, 1);
    }

    #[test]
    fn assign_pods_empty_clusters() {
        let clusters: Vec<MinerCluster> = vec![];
        let seed = [0u8; 32];

        let pods = assign_pods(&clusters, 2, &seed);

        assert!(pods.is_empty());
    }

    #[test]
    fn shard_indices_correct() {
        let cluster = test_cluster(&[1, 2, 3, 4, 5, 6]);
        let seed = [0u8; 32];

        let pod = assign_shards(&cluster, 3, 0, &seed);

        for (idx, shard) in pod.shards.iter().enumerate() {
            assert_eq!(shard.shard_index, idx as u32);
        }
    }
}
