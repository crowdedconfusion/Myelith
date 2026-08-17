//! Redundanz-Zuteilung: zonendiverse Pods (Anhang A.2, Schritt 5; Kap. 4.4).
//!
//! Für jedes Segment werden 2 disjunkte, zonendiverse Pods zugewiesen.
//! Die Pods müssen aus verschiedenen geografischen Regionen kommen, um
//! die Resilienz gegen regionale Ausfälle zu erhöhen.
//!
//! **Konsens-Feld:** Die Redundanz-Regeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Für jedes Segment werden 2 Pods ausgewählt, die:
//! 1. Disjunkt sind (keine gemeinsamen Miner)
//! 2. Aus verschiedenen geografischen Regionen kommen
//! 3. Deterministisch mit dem Seed ausgewählt werden

use myl_types::seed_rng::deterministic_shuffle;
use std::collections::{HashMap, HashSet};

use myl_types::ids::MinerId;
use myl_types::node_metadata::{GeoRegion, NodeMetadata};

use crate::shard_assignment::Pod;

/// Zuweisung eines Segments zu 2 redundanten Pods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentAssignment {
    /// Segment-Index (0-basiert).
    pub segment_index: u32,
    /// Primärer Pod (Index in der Pod-Liste).
    pub primary_pod_index: u32,
    /// Redundanter Pod (Index in der Pod-Liste).
    pub redundant_pod_index: u32,
}

/// Extrahiert die GeoRegion für einen Pod aus den Node-Metadaten.
///
/// Verwendet die Region des ersten Miners im Pod als Repräsentant.
/// (In der Praxis sollten alle Miner in einem Pod aus derselben Region kommen,
/// da sie im selben Cluster sind.)
fn pod_region(pod: &Pod, metadata: &HashMap<MinerId, NodeMetadata>) -> Option<GeoRegion> {
    // Nimm den ersten Miner im ersten Shard
    pod.shards
        .first()
        .and_then(|shard| shard.miners.first())
        .and_then(|miner| metadata.get(&miner.miner_id))
        .map(|meta| meta.region)
}

/// Prüft, ob zwei Pods disjunkt sind (keine gemeinsamen Miner).
fn pods_are_disjoint(pod_a: &Pod, pod_b: &Pod) -> bool {
    let miners_a: HashSet<MinerId> = pod_a
        .shards
        .iter()
        .flat_map(|s| s.miners.iter())
        .map(|m| m.miner_id)
        .collect();

    let miners_b: HashSet<MinerId> = pod_b
        .shards
        .iter()
        .flat_map(|s| s.miners.iter())
        .map(|m| m.miner_id)
        .collect();

    miners_a.is_disjoint(&miners_b)
}

/// Weist Segmente redundanten Pods zu (zonendivers und disjunkt).
///
/// **Algorithmus (Anhang A.2, Schritt 5):**
/// 1. Für jedes Segment:
///    a. Finde alle Pod-Paare, die disjunkt und zonendivers sind
///    b. Wähle ein Paar deterministisch mit dem Seed (Fisher-Yates)
/// 2. Gib die Zuweisungen zurück
///
/// **Determinismus:** Gleicher Seed + gleiche Pods + gleiche Metadaten → gleiche Zuweisungen.
///
/// **Parameter:**
/// - `num_segments`: Anzahl der Segmente, die zugewiesen werden sollen
/// - `pods`: Liste aller verfügbaren Pods (aus Phase 2.4)
/// - `metadata`: Geo-/AS-Metadaten für alle Miner (aus NETWORKING)
/// - `seed`: Epochenseed (aus Phase 2.1) für deterministische Auswahl
///
/// **Returns:** Liste von SegmentAssignment, eine pro Segment
///
/// **Fehler:** Wenn nicht genügend zonendiverse, disjunkte Pod-Paare vorhanden sind,
/// werden so viele Segmente wie möglich zugewiesen (der Rest bleibt unbehandelt).
pub fn assign_redundant_pods(
    num_segments: u32,
    pods: &[Pod],
    metadata: &HashMap<MinerId, NodeMetadata>,
    seed: &[u8; 32],
) -> Vec<SegmentAssignment> {
    if pods.len() < 2 {
        return vec![];
    }

    // Finde alle gültigen Pod-Paare (disjunkt + zonendivers)
    let mut valid_pairs: Vec<(u32, u32)> = vec![];
    
    for i in 0..pods.len() {
        for j in (i + 1)..pods.len() {
            // Prüfe Disjunktheit
            if !pods_are_disjoint(&pods[i], &pods[j]) {
                continue;
            }

            // Prüfe Zonendiversität
            let region_i = pod_region(&pods[i], metadata);
            let region_j = pod_region(&pods[j], metadata);

            if let (Some(r_i), Some(r_j)) = (region_i, region_j) {
                if r_i != r_j {
                    valid_pairs.push((i as u32, j as u32));
                }
            }
        }
    }

    if valid_pairs.is_empty() {
        return vec![];
    }

    // Shuffle die Paare mit dem Seed (deterministisch)
    let mut shuffled_pairs = valid_pairs;
    deterministic_shuffle(&mut shuffled_pairs, seed);

    // Weise Segmente zu (rotierend über die Paare)
    let mut assignments = Vec::with_capacity(num_segments as usize);
    
    for seg_idx in 0..num_segments {
        let pair_idx = (seg_idx as usize) % shuffled_pairs.len();
        let (primary, redundant) = shuffled_pairs[pair_idx];

        assignments.push(SegmentAssignment {
            segment_index: seg_idx,
            primary_pod_index: primary,
            redundant_pod_index: redundant,
        });
    }

    assignments
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner_filter::{HardwareClass, MinerRegistration};
    use crate::shard_assignment::Shard;

    fn test_registration(miner_byte: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([miner_byte; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 5,
        }
    }

    fn test_pod(pod_index: u32, miner_bytes: &[u8]) -> Pod {
        Pod {
            pod_index,
            shards: vec![Shard {
                shard_index: 0,
                miners: miner_bytes.iter().map(|&b| test_registration(b)).collect(),
            }],
        }
    }

    fn test_metadata(miner_byte: u8, region: GeoRegion) -> (MinerId, NodeMetadata) {
        let miner_id = MinerId::new([miner_byte; 32]);
        let metadata = NodeMetadata {
            miner: miner_id,
            region,
            asn: myl_types::node_metadata::Asn(13335),
            timestamp_ms: 1000,
        };
        (miner_id, metadata)
    }

    #[test]
    fn pods_are_disjoint_true() {
        let pod_a = test_pod(0, &[1, 2]);
        let pod_b = test_pod(1, &[3, 4]);

        assert!(pods_are_disjoint(&pod_a, &pod_b));
    }

    #[test]
    fn pods_are_disjoint_false() {
        let pod_a = test_pod(0, &[1, 2]);
        let pod_b = test_pod(1, &[2, 3]); // Miner 2 ist in beiden

        assert!(!pods_are_disjoint(&pod_a, &pod_b));
    }

    #[test]
    fn assign_redundant_pods_basic() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
        ];
        
        let mut metadata = HashMap::new();
        let (id1, meta1) = test_metadata(1, GeoRegion::Europe);
        let (id3, meta3) = test_metadata(3, GeoRegion::NorthAmerica);
        metadata.insert(id1, meta1);
        metadata.insert(id3, meta3);
        
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(2, &pods, &metadata, &seed);

        assert_eq!(assignments.len(), 2);
        // Beide Segmente sollten dasselbe Pod-Paar zugewiesen bekommen
        assert_eq!(assignments[0].primary_pod_index, assignments[1].primary_pod_index);
        assert_eq!(assignments[0].redundant_pod_index, assignments[1].redundant_pod_index);
    }

    #[test]
    fn assign_redundant_pods_same_region_rejected() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
        ];
        
        let mut metadata = HashMap::new();
        let (id1, meta1) = test_metadata(1, GeoRegion::Europe);
        let (id3, meta3) = test_metadata(3, GeoRegion::Europe); // Gleiche Region!
        metadata.insert(id1, meta1);
        metadata.insert(id3, meta3);
        
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(2, &pods, &metadata, &seed);

        // Keine Zuweisungen, da beide Pods in derselben Region sind
        assert!(assignments.is_empty());
    }

    #[test]
    fn assign_redundant_pods_overlapping_miners_rejected() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[2, 3]), // Miner 2 ist in beiden
        ];
        
        let mut metadata = HashMap::new();
        let (id1, meta1) = test_metadata(1, GeoRegion::Europe);
        let (id2, meta2) = test_metadata(2, GeoRegion::NorthAmerica);
        metadata.insert(id1, meta1);
        metadata.insert(id2, meta2);
        
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(2, &pods, &metadata, &seed);

        // Keine Zuweisungen, da die Pods überlappende Miner haben
        assert!(assignments.is_empty());
    }

    #[test]
    fn assign_redundant_pods_deterministic() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
            test_pod(2, &[5, 6]),
        ];
        
        let mut metadata = HashMap::new();
        let (id1, meta1) = test_metadata(1, GeoRegion::Europe);
        let (id3, meta3) = test_metadata(3, GeoRegion::NorthAmerica);
        let (id5, meta5) = test_metadata(5, GeoRegion::Asia);
        metadata.insert(id1, meta1);
        metadata.insert(id3, meta3);
        metadata.insert(id5, meta5);
        
        let seed = [42u8; 32];

        let assignments1 = assign_redundant_pods(5, &pods, &metadata, &seed);
        let assignments2 = assign_redundant_pods(5, &pods, &metadata, &seed);

        assert_eq!(assignments1, assignments2);
    }

    #[test]
    fn assign_redundant_pods_too_few_pods() {
        let pods = vec![test_pod(0, &[1, 2])]; // Nur ein Pod
        
        let metadata = HashMap::new();
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(2, &pods, &metadata, &seed);

        assert!(assignments.is_empty());
    }

    #[test]
    fn assign_redundant_pods_zero_segments() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
        ];
        
        let metadata = HashMap::new();
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(0, &pods, &metadata, &seed);

        assert!(assignments.is_empty());
    }

    #[test]
    fn segment_indices_correct() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
        ];
        
        let mut metadata = HashMap::new();
        let (id1, meta1) = test_metadata(1, GeoRegion::Europe);
        let (id3, meta3) = test_metadata(3, GeoRegion::NorthAmerica);
        metadata.insert(id1, meta1);
        metadata.insert(id3, meta3);
        
        let seed = [0u8; 32];

        let assignments = assign_redundant_pods(3, &pods, &metadata, &seed);

        for (idx, assignment) in assignments.iter().enumerate() {
            assert_eq!(assignment.segment_index, idx as u32);
        }
    }
}
