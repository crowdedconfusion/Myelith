//! Die kanonische Signierbotschaft eines PoI-Bündels.
//!
//! # ⚑ Warum sie hier steht und nicht bei der Prüfung (Fund 144)
//!
//! Sie lag bis zum 2026-09-02 in `myl_consensus::poi`. Dort konnte
//! `myl-ledger` sie nicht sehen, denn `myl-consensus` hängt an
//! `myl-ledger` und nicht umgekehrt. **Die Folge war eine Prüfung am
//! falschen Ort:** Der Übergang, der ein Bündel in den Zustand nimmt,
//! konnte die Aggregatsignatur nicht prüfen, also prüfte er sie nicht,
//! und geprüft wurde erst beim Epochenabschluss, eine ganze Epoche
//! später.
//!
//! **Eine Botschaft gehört zu ihrem Typ.** `PoIBundle` steht in dieser
//! Kiste, also steht die Bytefolge, die ihn unterschreibbar macht, hier
//! daneben. Wer sie prüft, ist eine andere Frage.
//!
//! **Konsens-Feld:** Die Signierbotschaft ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::core_types::PoIBundle;
use crate::ids::{EpochId, MerkleRoot, PodId};

/// Domain-Separation-Präfix für PoI-Bündel-Signaturen.
///
/// Eigenes Präfix aus demselben Grund wie im BFT-Protokoll
/// ([`crate::signing`]): ohne Trennung wäre eine Signatur aus einem
/// anderen Zusammenhang unter Umständen als Bündel-Bestätigung
/// wiederverwendbar.
pub const DST_POI_BUNDLE: &[u8] = b"MYELITH_POI_BUNDLE_v1";

/// Kanonische Signierbotschaft eines PoI-Bündels.
///
/// **Aufbau:** `DST_POI_BUNDLE ‖ u64_le(epoch) ‖ pod ‖ segments_root ‖
/// u64_le(vtfe_claimed) ‖ u32_le(segmente)`: feste Feldbreiten in
/// fester Reihenfolge, damit zu einem Bündel genau eine Bytefolge
/// gehört.
///
/// `vtfe_claimed` ist Teil der Botschaft: sonst könnte der Koordinator
/// die beanspruchte Arbeitsmenge nach dem Einsammeln der Signaturen
/// erhöhen, ohne das Aggregat ungültig zu machen.
///
/// ⚑ **`segmente` gehört aus demselben Grund dazu, und der Schaden wäre
/// ein anderer** (Fund 115, 2026-09-01): Wer die Segmentzahl nachträglich
/// erhöht, **verdünnt die Stichprobenwahrscheinlichkeit je Segment**. Aus
/// `p` wird `p/k`, und die Sicherheitsbedingung aus Anhang B.1 hängt
/// genau an `p`.
pub fn poi_bundle_message(
    epoch: EpochId,
    pod: PodId,
    segments_root: &MerkleRoot,
    vtfe_claimed: u64,
    segmente: u32,
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DST_POI_BUNDLE.len() + 8 + 32 + 32 + 8 + 4);
    msg.extend_from_slice(DST_POI_BUNDLE);
    msg.extend_from_slice(&epoch.0.to_le_bytes());
    msg.extend_from_slice(pod.as_bytes());
    msg.extend_from_slice(segments_root.as_bytes());
    msg.extend_from_slice(&vtfe_claimed.to_le_bytes());
    msg.extend_from_slice(&segmente.to_le_bytes());
    msg
}

/// Signierbotschaft zu einem konkreten Bündel.
pub fn bundle_message(bundle: &PoIBundle) -> Vec<u8> {
    poi_bundle_message(
        bundle.epoch,
        bundle.pod,
        &bundle.segments_root,
        bundle.vtfe_claimed,
        bundle.segmente,
    )
}
