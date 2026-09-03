//! `myl-pod` — L2-Compute-Pipeline: der Pod-Mining-Loop (Whitepaper
//! Kap. 4, Anhang A.3).
//!
//! Bei uPoI ist der Mining-Loop kein Hash-Raten, sondern der
//! Inferenz-Serviceloop. Jeder Shard-Miner empfängt Aktivierungen,
//! prüft den Eingangs-Hash gegen die Spur, führt seinen Layer-Abschnitt
//! aus, schreibt die Spur fort, signiert den Übergang und reicht die
//! Aktivierungen weiter (Anhang A.3, `shard_loop`). Der Koordinator
//! sammelt Micro-Batches und aggregiert abgeschlossene Segmente zu
//! PoI-Bündeln (`coordinator_loop`).
//!
//! Was `myl-pod` zu den INTEGER_LLM-Bausteinen hinzufügt:
//! - **Spur-Hashes + Eingangs-Prüfung:** Jeder Shard hasht seine
//!   Ausgabe-Aktivierungen und hängt den Hash an die Segment-Spur. Der
//!   nächste Shard prüft den Hash der empfangenen Aktivierungen gegen
//!   den letzten Spur-Eintrag — manipulierte Aktivierungen werden
//!   zuverlässig verworfen (Akzeptanzkriterium Phase 1).
//! - **BLS-Signierung der Übergänge:** Jeder Shard signiert den Übergang
//!   `(segment_id, vorheriger Spur-Hash, neuer Spur-Hash)` mit seinem
//!   BLS-Schlüssel. Die Signaturen sammeln sich im Segment und werden am
//!   Ende zu einem Aggregat zusammengefasst (PoI-Bündel).
//! - **KV-Cache-Session-Affinität** (Kap. 4.2): Der KV-Cache bleibt auf
//!   den Shards des zugewiesenen Pods.
//! - **DA-Archivierung** (Kap. 4.2, Anhang A.3 Schritt 6): Aktivierungen
//!   werden erasure-codiert für die Streitfrist archiviert.
//!
//! Der rechenkorrekte Forward-Pass kommt aus der INTEGER_LLM-Stage-API
//! (`embed_token`/`run_layers`/`head_logits`, INTEGER_LLM Phase 12.56);
//! die Typen (Segment, Hash, BLS, IDs) aus `myl-types`.
//!
//! Konsens-/Determinismus-Regeln: keine Gleitkomma im Pfad, Spur-Hashes
//! und Signaturen sind deterministisch, die Pod-Ausgabe ist bei
//! identischem Prompt bitgleich reproduzierbar.

#![deny(unsafe_code)]

pub mod artefakte;
pub mod wire;
pub mod trace;
pub mod shard;
pub mod ausfallmeldung;
pub mod netzreserve;
pub mod entsiegelung;
pub mod gegenstelle;
pub mod ortsdienst;
pub mod pipelinewerk;
pub mod standby;
pub mod coordinator;
pub mod micro_batch;
pub mod zuteilung;

pub use standby::{
    BesetzungFehler, PodBesetzung, RebuildAnlass, RebuildAuftrag, Uebernahme, RESERVE_PLAETZE,
};
pub use trace::{
    activation_hash, verify_input_hash, Rolle, TransitionSig, DST_SHARD_TRANSITION, ZERO_HASH,
};
pub use wire::{PodMessage, MAGIC, FLAG_ABORT, FLAG_FEEDBACK, FLAG_SAMPLE, FLAG_TOKEN_INPUT};
pub use zuteilung::{
    besetzungsreihenfolge, epochenwechsel_aus_zuteilung, ist_besetzbar, 
    pod_aus_zuteilung, ZuteilungFehler,
};
pub use micro_batch::{
    InferenceRequest, MicroBatch, MicroBatchCollector, PipelineStage, PipelineTracker,
    DEFAULT_WINDOW_MS, MAX_BATCH_SIZE,
};
