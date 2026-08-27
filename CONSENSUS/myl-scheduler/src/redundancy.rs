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
        .and_then(|shard| metadata.get(&shard.miner.miner_id))
        .map(|meta| meta.region)
}

/// Prüft, ob zwei Pods disjunkt sind (keine gemeinsamen Mitglieder).
///
/// ⚑ **Die Reserve zählt mit, seit es sie gibt (2026-08-26).** Bis zur
/// Entscheidung D3 hatte ein Pod keine getrennte Reserve, und diese
/// Prüfung sah zwangsläufig alle Mitglieder. Seither wäre eine Prüfung,
/// die nur die Shard-Positionen vergleicht, unvollständig:
///
/// **Stünde dieselbe Maschine in der Reserve beider Pods eines
/// Redundanzpaars**, übernähme sie bei einem Ausfall auf **beiden**
/// Seiten. Der Redundanzvergleich verglände dann zwei Ergebnisse
/// derselben Maschine, und Stufe 1 der Verifikation wäre eine
/// Selbstbestätigung. Genau davor soll die Disjunktheit schützen.
fn pods_are_disjoint(pod_a: &Pod, pod_b: &Pod) -> bool {
    let miners_a: HashSet<MinerId> = pod_a.mitglieder().map(|m| m.miner_id).collect();
    let miners_b: HashSet<MinerId> = pod_b.mitglieder().map(|m| m.miner_id).collect();
    miners_a.is_disjoint(&miners_b)
}

/// Warum eine Redundanzzuteilung nicht zustande kam.
///
/// **Vor dieser Aufteilung bekam der Aufrufer eine leere Liste** und
/// konnte nicht unterscheiden, woran es lag. Die beiden Fälle sind aber
/// verschiedene Befunde: Im ersten fehlen Cluster, im zweiten Streuung —
/// und die Gegenmaßnahmen sind es auch. Ein Ergebnis, das seinen Grund
/// verschweigt, zwingt jeden Aufrufer, ihn selbst zu raten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZuweisungsHindernis {
    /// Weniger als zwei Pods vorhanden: Es gibt nichts, woraus ein Paar
    /// werden könnte.
    ZuWenigPods { pods: usize },
    /// Pods sind vorhanden, aber kein einziges Paar ist disjunkt und
    /// zonendivers zugleich.
    KeinGueltigesPaar { pods: usize },
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
/// **Returns:** `Ok` mit einer Zuweisung pro Segment — bei null
/// Segmenten eine leere Liste, das ist kein Fehler. `Err` nennt den
/// Grund, wenn kein einziges gültiges Paar möglich ist; ein teilweises
/// Ergebnis gibt es nicht, denn ein Segment ohne Redundanzpartner wäre
/// ein Versprechen, das der Vertrag nicht hält.
pub fn assign_redundant_pods(
    num_segments: u32,
    pods: &[Pod],
    metadata: &HashMap<MinerId, NodeMetadata>,
    seed: &[u8; 32],
) -> Result<Vec<SegmentAssignment>, ZuweisungsHindernis> {
    // Null Segmente brauchen keine Paare: Wer nichts verlangt, bekommt
    // nichts, und das ist kein Scheitern. Vor dieser Prüfung stünde hier
    // ein Hindernis über fehlende Paare, das niemanden interessiert.
    if num_segments == 0 {
        return Ok(vec![]);
    }
    if pods.len() < 2 {
        return Err(ZuweisungsHindernis::ZuWenigPods { pods: pods.len() });
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
        return Err(ZuweisungsHindernis::KeinGueltigesPaar { pods: pods.len() });
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

    Ok(assignments)
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

    /// Ein Pod mit je einem Miner auf jeder Position, ohne Reserve.
    fn test_pod(pod_index: u32, miner_bytes: &[u8]) -> Pod {
        Pod {
            pod_index,
            shards: miner_bytes
                .iter()
                .enumerate()
                .map(|(i, &b)| Shard {
                    shard_index: i as u32,
                    miner: test_registration(b),
                })
                .collect(),
            reserve: vec![],
        }
    }

    /// Ein Pod mit Positionen **und** Reserve.
    fn test_pod_mit_reserve(pod_index: u32, positionen: &[u8], reserve: &[u8]) -> Pod {
        let mut pod = test_pod(pod_index, positionen);
        pod.reserve = reserve.iter().map(|&b| test_registration(b)).collect();
        pod
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

    /// ⚑ **Eine geteilte Reserve macht zwei Pods nicht disjunkt.**
    ///
    /// Stünde dieselbe Maschine in der Reserve beider Pods eines
    /// Redundanzpaars, übernähme sie bei einem Ausfall auf **beiden**
    /// Seiten. Der Redundanzvergleich verglände dann zwei Ergebnisse
    /// derselben Maschine, und Stufe 1 der Verifikation wäre eine
    /// Selbstbestätigung.
    ///
    /// Diese Prüfung konnte es vor der Entscheidung D3 nicht geben: Ein
    /// Pod hatte keine getrennte Reserve.
    #[test]
    fn eine_geteilte_reserve_bricht_die_disjunktheit() {
        let pod_a = test_pod_mit_reserve(0, &[1, 2], &[9, 10]);
        let pod_b = test_pod_mit_reserve(1, &[3, 4], &[9, 11]);
        assert!(
            !pods_are_disjoint(&pod_a, &pod_b),
            "zwei Pods mit gemeinsamem Reservemitglied galten als disjunkt"
        );
        // Gegenprobe: getrennte Reserven sind disjunkt.
        let pod_c = test_pod_mit_reserve(2, &[5, 6], &[12, 13]);
        assert!(pods_are_disjoint(&pod_a, &pod_c));
    }

    /// Und die andere Richtung: ein Positionsmiter des einen darf nicht
    /// in der Reserve des anderen stehen.
    #[test]
    fn ein_positionsminer_in_fremder_reserve_bricht_die_disjunktheit() {
        let pod_a = test_pod_mit_reserve(0, &[1, 2], &[9, 10]);
        let pod_b = test_pod_mit_reserve(1, &[3, 4], &[1, 11]);
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

        let assignments = assign_redundant_pods(2, &pods, &metadata, &seed)
            .expect("zwei zonendiverse Pods bilden ein Paar");

        assert_eq!(assignments.len(), 2);
        // Beide Segmente sollten dasselbe Pod-Paar zugewiesen bekommen
        assert_eq!(assignments[0].primary_pod_index, assignments[1].primary_pod_index);
        assert_eq!(assignments[0].redundant_pod_index, assignments[1].redundant_pod_index);
    }

    /// **Der Befund, der in der leeren Liste verschwand.** Zwei Pods in
    /// derselben Region bilden kein Redundanzpaar; der Aufrufer muss das
    /// als Grund sehen, nicht als bloßes „nichts zugeteilt".
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

        let hinder = assign_redundant_pods(2, &pods, &metadata, &seed).unwrap_err();
        assert_eq!(hinder, ZuweisungsHindernis::KeinGueltigesPaar { pods: 2 });
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

        // Keine Zuweisungen, da die Pods überlappende Miner haben.
        // Auch das ist der Paar-Mangel, nicht der Pod-Mangel.
        let hinder = assign_redundant_pods(2, &pods, &metadata, &seed).unwrap_err();
        assert_eq!(hinder, ZuweisungsHindernis::KeinGueltigesPaar { pods: 2 });
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

        let assignments1 = assign_redundant_pods(5, &pods, &metadata, &seed)
            .expect("drei zonendiverse Pods bilden Paare");
        let assignments2 = assign_redundant_pods(5, &pods, &metadata, &seed)
            .expect("drei zonendiverse Pods bilden Paare");

        assert_eq!(assignments1, assignments2);
    }

    #[test]
    fn assign_redundant_pods_too_few_pods() {
        let pods = vec![test_pod(0, &[1, 2])]; // Nur ein Pod

        let metadata = HashMap::new();
        let seed = [0u8; 32];

        let hinder = assign_redundant_pods(2, &pods, &metadata, &seed).unwrap_err();
        assert_eq!(hinder, ZuweisungsHindernis::ZuWenigPods { pods: 1 });
    }

    /// **Die Gegenprobe zur Aufteilung:** Die beiden Hindernisse sind
    /// unterscheidbar, und zwar in beiden Richtungen. Vor dieser Änderung
    /// gab es in beiden Fällen dieselbe leere Liste; ein Test, der nur
    /// `is_empty()` prüfte, hätte die Aufteilung nicht erzwingen können.
    #[test]
    fn die_beiden_hindernisse_sind_unterscheidbar() {
        let seed = [0u8; 32];
        let leer = HashMap::new();

        // Ein Pod: zu wenig, um ein Paar zu bilden.
        let eins = vec![test_pod(0, &[1, 2])];
        assert_eq!(
            assign_redundant_pods(1, &eins, &leer, &seed).unwrap_err(),
            ZuweisungsHindernis::ZuWenigPods { pods: 1 }
        );

        // Zwei Pods, aber ohne Metadaten keine Region, also kein Paar.
        let zwei = vec![test_pod(0, &[1, 2]), test_pod(1, &[3, 4])];
        assert_eq!(
            assign_redundant_pods(1, &zwei, &leer, &seed).unwrap_err(),
            ZuweisungsHindernis::KeinGueltigesPaar { pods: 2 }
        );
    }

    #[test]
    fn assign_redundant_pods_zero_segments() {
        let pods = vec![
            test_pod(0, &[1, 2]),
            test_pod(1, &[3, 4]),
        ];

        let metadata = HashMap::new();
        let seed = [0u8; 32];

        // Null Segmente verlangen, und nichts bekommen, ist kein Fehler:
        // Die Paarsuche bleibt einem erspart, der nichts braucht.
        let assignments = assign_redundant_pods(0, &pods, &metadata, &seed)
            .expect("null Segmente sind erfüllbar");
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

        let assignments = assign_redundant_pods(3, &pods, &metadata, &seed)
            .expect("zwei zonendiverse Pods bilden ein Paar");

        for (idx, assignment) in assignments.iter().enumerate() {
            assert_eq!(assignment.segment_index, idx as u32);
        }
    }
}
