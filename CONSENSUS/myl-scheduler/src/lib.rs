//! `myl-scheduler` — Deterministischer Epochen-Scheduler (Whitepaper Anhang A.2).
//!
//! Der Scheduler ist der Kern der Pod-Bildung und Segment-Zuteilung. Er läuft
//! auf jedem Node identisch (deterministisch) und produziert dieselben Zuteilungen,
//! solange die Eingaben gleich sind. Keine zentrale Instanz — jeder Node kann die
//! Zuteilungen unabhängig nachrechnen.
//!
//! **Konsens-Feld:** Alle Schritte des Schedulers sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Fünf Schritte (Anhang A.2):**
//! 1. VRF-Seed-Ableitung aus finalisiertem Block der Vorepoche
//! 2. Miner-Filterung nach Hardware-Klasse und Registrierungsschluss
//! 3. Geo-Clustering unter Latenz-Constraint (konsumiert LatencyGraph aus NETWORKING)
//! 4. Shard-Zuweisung innerhalb des Pods: Fisher-Yates mit Seed
//! 5. Redundanz-Zuteilung: jedes Nachfrage-Bucket → 2 disjunkte, zonendiverse Pods
//! 6. Stichproben-Lotterie: p·|segments| Segmente für Checker markieren
//!
//! **Design:** Alle Schritte sind reine Funktionen (Eingabe → Ausgabe) ohne
//! versteckten globalen Zustand. Borsh-Serialisierung für kanonische Darstellung.
//! Keine Gleitkomma-Arithmetik (Konsens-Determinismus).

#![deny(unsafe_code)]

pub mod geo_clustering;
pub mod miner_filter;
pub mod redundancy;
pub mod sampling;
pub mod shard_assignment;
pub mod shuffle;
pub mod vrf_seed;

pub use geo_clustering::{form_clusters, LatencyMatrix, MinerCluster};
pub use miner_filter::{
    filter_miners, HardwareClass, MinerRegistration,
};
pub use redundancy::{assign_redundant_pods, SegmentAssignment};
pub use sampling::{sample_segments, SamplingResult};
pub use shard_assignment::{assign_pods, assign_shards, Pod, Shard};
pub use vrf_seed::{derive_epoch_seed, EpochSeed, VrfSeedError};
