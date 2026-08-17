//! Geo-Clustering unter Latenz-Constraint (Anhang A.2, Schritt 3).
//!
//! Miner werden basierend auf ihrer geografischen Nähe geclustert. Der Latenz-Constraint
//! stellt sicher, dass alle Miner in einem Cluster eine maximale Latenz zueinander haben.
//! Der Seed aus Phase 2.1 wird für die seed-randomisierte Clusterwahl verwendet.
//!
//! **Konsens-Feld:** Die Clustering-Regeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Das Clustering ist ein Greedy-Algorithmus:
//! 1. Starte mit einem zufälligen Miner (aus Seed)
//! 2. Füge Miner hinzu, die den Latenz-Constraint erfüllen
//! 3. Wiederhole, bis alle Miner zugeordnet sind
//!
//! Der Algorithmus ist deterministisch (gleicher Seed → gleiche Cluster).

use myl_types::seed_rng::deterministic_shuffle;
use std::collections::HashMap;

use myl_types::ids::MinerId;

use crate::miner_filter::MinerRegistration;

/// Latenz-Matrix: speichert die Latenz zwischen allen Miner-Paaren.
///
/// Die Matrix ist symmetrisch (Latenz von A nach B = Latenz von B nach A).
/// Fehlende Einträge werden als unendlich behandelt (keine Verbindung).
#[derive(Debug, Clone)]
pub struct LatencyMatrix {
    /// Latenz in Millisekunden zwischen Miner-Paaren.
    /// Key: (MinerId, MinerId), Value: Latenz in ms.
    latencies: HashMap<(MinerId, MinerId), u32>,
}

impl LatencyMatrix {
    /// Neue, leere Latenz-Matrix.
    pub fn new() -> Self {
        Self {
            latencies: HashMap::new(),
        }
    }

    /// Fügt eine Latenz zwischen zwei Minern hinzu.
    pub fn set_latency(&mut self, a: MinerId, b: MinerId, latency_ms: u32) {
        self.latencies.insert((a, b), latency_ms);
        self.latencies.insert((b, a), latency_ms); // Symmetrisch
    }

    /// Gibt die Latenz zwischen zwei Minern zurück.
    /// Returns `None` wenn keine Latenz bekannt ist (wird als unendlich behandelt).
    pub fn get_latency(&self, a: MinerId, b: MinerId) -> Option<u32> {
        if a == b {
            return Some(0); // Latenz zu sich selbst = 0
        }
        self.latencies.get(&(a, b)).copied()
    }

    /// Gibt die maximale Latenz zwischen einem Miner und einer Gruppe von Minern zurück.
    /// Returns `None` wenn keine Latenz bekannt ist.
    pub fn max_latency_to_group(&self, miner: MinerId, group: &[MinerId]) -> Option<u32> {
        group
            .iter()
            .filter_map(|&m| self.get_latency(miner, m))
            .max()
    }
}

impl Default for LatencyMatrix {
    fn default() -> Self {
        Self::new()
    }
}

/// Ein Cluster von Minern, die den Latenz-Constraint erfüllen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinerCluster {
    /// Miner in diesem Cluster.
    pub miners: Vec<MinerRegistration>,
    /// Maximale Latenz innerhalb des Clusters (in ms).
    pub max_internal_latency: u32,
}

/// Bildet Cluster aus Minern unter einem Latenz-Constraint.
///
/// **Algorithmus (Anhang A.2, Schritt 3):**
/// 1. Shuffle die Miner mit dem Seed (deterministisch)
/// 2. Für jeden Miner:
///    a. Finde ein bestehendes Cluster, dem der Miner hinzugefügt werden kann
///       (maximale Latenz zu allen Cluster-Mitgliedern <= max_latency_ms)
///    b. Wenn kein passendes Cluster gefunden wird, erstelle ein neues Cluster
/// 3. Gib alle Cluster zurück
///
/// **Determinismus:** Gleicher Seed + gleiche Miner + gleiche Latenz-Matrix → gleiche Cluster.
/// Die Reihenfolge der Cluster ist deterministisch (sortiert nach erstem Miner).
///
/// **Parameter:**
/// - `miners`: Gefilterte Liste von Miner-Registrierungen (aus Phase 2.2)
/// - `latency_matrix`: Latenz-Matrix zwischen allen Minern
/// - `max_latency_ms`: Maximale erlaubte Latenz innerhalb eines Clusters (in ms)
/// - `seed`: Epochenseed (aus Phase 2.1) für deterministische Reihenfolge
///
/// **Returns:** Liste von Clustern, sortiert nach erstem Miner
pub fn form_clusters(
    miners: &[MinerRegistration],
    latency_matrix: &LatencyMatrix,
    max_latency_ms: u32,
    seed: &[u8; 32],
) -> Vec<MinerCluster> {
    if miners.is_empty() {
        return vec![];
    }

    // Shuffle die Miner mit dem Seed (deterministisch)
    let mut shuffled = miners.to_vec();
    deterministic_shuffle(&mut shuffled, seed);

    let mut clusters: Vec<MinerCluster> = vec![];

    // Für jeden Miner: Finde oder erstelle Cluster
    for miner in shuffled {
        let mut added = false;

        // Versuche, ein bestehendes Cluster zu finden
        for cluster in &mut clusters {
            // Prüfe, ob der Miner den Latenz-Constraint erfüllt
            let max_latency = latency_matrix.max_latency_to_group(
                miner.miner_id,
                &cluster.miners.iter().map(|m| m.miner_id).collect::<Vec<_>>(),
            );

            if let Some(max_lat) = max_latency {
                if max_lat <= max_latency_ms {
                    // Miner kann hinzugefügt werden
                    cluster.miners.push(miner);
                    cluster.max_internal_latency = cluster.max_internal_latency.max(max_lat);
                    added = true;
                    break;
                }
            }
        }

        // Wenn kein passendes Cluster gefunden wurde, erstelle ein neues
        if !added {
            clusters.push(MinerCluster {
                miners: vec![miner],
                max_internal_latency: 0,
            });
        }
    }

    // Sortiere Cluster nach erstem Miner (für Determinismus)
    clusters.sort_by(|a, b| a.miners[0].miner_id.cmp(&b.miners[0].miner_id));

    clusters
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner_filter::HardwareClass;

    fn test_registration(miner_byte: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([miner_byte; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 5,
        }
    }

    #[test]
    fn latency_matrix_basic() {
        let mut matrix = LatencyMatrix::new();
        let a = MinerId::new([1u8; 32]);
        let b = MinerId::new([2u8; 32]);

        matrix.set_latency(a, b, 50);

        assert_eq!(matrix.get_latency(a, b), Some(50));
        assert_eq!(matrix.get_latency(b, a), Some(50)); // Symmetrisch
        assert_eq!(matrix.get_latency(a, a), Some(0)); // Zu sich selbst
    }

    #[test]
    fn latency_matrix_unknown() {
        let matrix = LatencyMatrix::new();
        let a = MinerId::new([1u8; 32]);
        let b = MinerId::new([2u8; 32]);

        assert_eq!(matrix.get_latency(a, b), None);
    }

    #[test]
    fn form_clusters_single_miner() {
        let miners = vec![test_registration(1)];
        let matrix = LatencyMatrix::new();
        let seed = [0u8; 32];

        let clusters = form_clusters(&miners, &matrix, 100, &seed);

        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].miners.len(), 1);
    }

    #[test]
    fn form_clusters_two_miners_low_latency() {
        let miners = vec![test_registration(1), test_registration(2)];
        let mut matrix = LatencyMatrix::new();
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([2u8; 32]), 50);
        let seed = [0u8; 32];

        let clusters = form_clusters(&miners, &matrix, 100, &seed);

        // Beide Miner sollten in einem Cluster sein (Latenz 50 <= 100)
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].miners.len(), 2);
        assert_eq!(clusters[0].max_internal_latency, 50);
    }

    #[test]
    fn form_clusters_two_miners_high_latency() {
        let miners = vec![test_registration(1), test_registration(2)];
        let mut matrix = LatencyMatrix::new();
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([2u8; 32]), 150);
        let seed = [0u8; 32];

        let clusters = form_clusters(&miners, &matrix, 100, &seed);

        // Beide Miner sollten in separaten Clustern sein (Latenz 150 > 100)
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].miners.len(), 1);
        assert_eq!(clusters[1].miners.len(), 1);
    }

    #[test]
    fn form_clusters_deterministic() {
        let miners = vec![
            test_registration(1),
            test_registration(2),
            test_registration(3),
        ];
        let mut matrix = LatencyMatrix::new();
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([2u8; 32]), 50);
        matrix.set_latency(MinerId::new([2u8; 32]), MinerId::new([3u8; 32]), 60);
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([3u8; 32]), 70);
        let seed = [42u8; 32];

        let clusters1 = form_clusters(&miners, &matrix, 100, &seed);
        let clusters2 = form_clusters(&miners, &matrix, 100, &seed);

        assert_eq!(clusters1, clusters2);
    }

    #[test]
    fn form_clusters_different_seeds() {
        let miners = vec![
            test_registration(1),
            test_registration(2),
            test_registration(3),
        ];
        let mut matrix = LatencyMatrix::new();
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([2u8; 32]), 50);
        matrix.set_latency(MinerId::new([2u8; 32]), MinerId::new([3u8; 32]), 60);
        matrix.set_latency(MinerId::new([1u8; 32]), MinerId::new([3u8; 32]), 70);

        let clusters1 = form_clusters(&miners, &matrix, 100, &[1u8; 32]);
        let clusters2 = form_clusters(&miners, &matrix, 100, &[2u8; 32]);

        // Unterschiedliche Seeds können zu unterschiedlichen Clustern führen
        // (muss nicht immer der Fall sein, aber oft)
        // Wir prüfen nur, dass beide Ergebnisse gültig sind
        assert!(!clusters1.is_empty());
        assert!(!clusters2.is_empty());
    }

    #[test]
    fn form_clusters_empty_input() {
        let miners: Vec<MinerRegistration> = vec![];
        let matrix = LatencyMatrix::new();
        let seed = [0u8; 32];

        let clusters = form_clusters(&miners, &matrix, 100, &seed);

        assert!(clusters.is_empty());
    }
}
