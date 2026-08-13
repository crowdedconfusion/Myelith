//! Pod-Koordinator: Micro-Batching + Pipeline-Dispatch + PoI-Aggregation
//! (Anhang A.3, `coordinator_loop`).
//!
//! Der Koordinator sammelt eingehende Anfragen innerhalb des
//! Micro-Batching-Fensters `WINDOW_MS` (Design-Entscheidung 2026-08-13:
//! Default 250 ms, kalibriert wird in Phase 2.1), weist Session- und
//! Segment-Ids zu, schickt die Token-Nachrichten in die Shard-Pipeline
//! und sammelt die generierten Tokens. Abgeschlossene Segmente werden zu
//! PoI-Bündeln aggregiert.
//!
//! Für Phase 1 läuft die Pipeline in-Prozess (die Shards werden direkt
//! aufgerufen); die Netzwerk-Variante (echte Nodes) folgt in den
//! Härtungs-Phasen. Die Determinismus-Garantie gilt in beiden Fällen:
//! derselbe Prompt ⇒ bitgleiche Token-Sequenz.

use std::sync::Arc;

use myl_types::bls::{aggregate_signatures, BlsSignature};
use myl_types::core_types::{segments_root, PoIBundle};
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};

use crate::shard::{ShardNode, ShardOut};
use crate::wire::{self, PodMessage, FLAG_SAMPLE};

/// Default für das Micro-Batching-Fenster (Design-Entscheidung 2026-08-13).
pub const DEFAULT_WINDOW_MS: u64 = 250;

/// Ein abgeschlossenes Segment mit den gesammelten Übergangs-Signaturen.
#[derive(Debug, Clone)]
pub struct CompletedSegment {
    pub id: SegmentId,
    pub trace: Vec<[u8; 32]>,
    pub signatures: Vec<BlsSignature>,
    pub pod_path: Vec<MinerId>,
}

/// Segment-Id aus der Session-Id ableiten (Anhang A.1: `h(session ‖
/// index)`; hier vereinfacht als linksbündig mit Nullen aufgefüllte
/// Session-Id, da Phase 1 ein Segment je Session verarbeitet).
fn segment_id_from_session(session_id: u64) -> SegmentId {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&session_id.to_le_bytes());
    SegmentId::new(bytes)
}

/// Pod-Koordinator.
pub struct Coordinator {
    pub pod_id: PodId,
    pub epoch: EpochId,
    pub window_ms: u64,
    /// Die Shard-Pipeline in Reihenfolge (Shard 0 zuerst).
    shards: Vec<Arc<ShardNode>>,
    /// Abgeschlossene Segmente dieser Epoche.
    completed: Vec<CompletedSegment>,
}

impl Coordinator {
    pub fn new(pod_id: PodId, epoch: EpochId, shards: Vec<Arc<ShardNode>>, window_ms: u64) -> Self {
        Self {
            pod_id,
            epoch,
            window_ms,
            shards,
            completed: Vec::new(),
        }
    }

    /// Anzahl der Shards in der Pipeline.
    pub fn num_shards(&self) -> usize {
        self.shards.len()
    }

    /// Führt einen Prompt durch die Shard-Pipeline und liefert die
    /// generierten Tokens (deterministisch).
    ///
    /// `prompt_tokens` ist der Prompt; `max_new_tokens` begrenzt die
    /// Generation. Die Ausgabe ist bei identischem Prompt bitgleich.
    pub fn run_prompt(&mut self, session_id: u64, prompt_tokens: &[u32], max_new_tokens: u64) -> Vec<u32> {
        let segment_id = segment_id_from_session(session_id);
        let mut generated = Vec::new();
        let mut trace = Vec::new();
        let mut signatures = Vec::new();

        // 1) Prefill: Prompt-Tokens durch die Pipeline schicken (je Token
        //    eine Nachricht, Position 0..P-1). Das letzte Prompt-Token
        //    trägt FLAG_SAMPLE und löst das erste Sampling aus.
        let mut pending_feedback: Option<PodMessage> = None;
        for (i, tok) in prompt_tokens.iter().enumerate() {
            let is_last = i + 1 == prompt_tokens.len();
            let packed = wire::pack_tokens(&[*tok]);
            let flags = if is_last { FLAG_SAMPLE } else { 0 };
            let msg = PodMessage::token_input(segment_id, session_id, i as u64, packed, flags);
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg);
            trace = out_trace;
            signatures.extend(out_sigs);
            if let Some(t) = token_opt {
                generated.push(t);
            }
            if feedback_opt.is_some() {
                pending_feedback = feedback_opt;
            }
        }

        // 2) Autoregressive Feedback-Schleife: das vom End-Shard
        //    erzeugte Feedback wird durch die Pipeline geschickt, bis das
        //    Budget erschöpft ist oder kein Feedback mehr kommt.
        while (generated.len() as u64) < max_new_tokens {
            let msg = match pending_feedback.take() {
                Some(m) => m,
                None => break,
            };
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg);
            trace = out_trace;
            signatures.extend(out_sigs);
            match token_opt {
                Some(t) => generated.push(t),
                None => break,
            }
            pending_feedback = feedback_opt;
        }

        // 3) Segment als abgeschlossen vermerken.
        let pod_path: Vec<MinerId> = (0..self.shards.len())
            .map(|i| MinerId::new([(i as u8) + 1; 32]))
            .collect();
        self.completed.push(CompletedSegment {
            id: segment_id,
            trace,
            signatures,
            pod_path,
        });

        generated
    }

    /// Schickt eine Nachricht durch die Shard-Pipeline und sammelt Spur,
    /// Signaturen, ein evtl. gesampeltes Token und die Feedback-Nachricht.
    fn pump(
        &self,
        first: &PodMessage,
    ) -> (Vec<[u8; 32]>, Vec<BlsSignature>, Option<u32>, Option<PodMessage>) {
        let mut trace = Vec::new();
        let mut signatures = Vec::new();
        let mut token_out = None;
        let mut feedback_out = None;
        let mut current = first.clone();
        loop {
            let shard_idx = if current.carries_tokens() {
                0
            } else {
                (current.sender_shard + 1) as usize
            };
            if shard_idx >= self.shards.len() {
                break;
            }
            let shard = &self.shards[shard_idx];
            match shard.process(&current) {
                Ok(ShardOut::Forward(next)) => {
                    trace = next.trace.clone();
                    signatures.push(next.signature);
                    current = next;
                }
                Ok(ShardOut::Token { token, feedback, .. }) => {
                    token_out = Some(token);
                    feedback_out = feedback;
                    break;
                }
                Ok(ShardOut::Prefill) => {
                    // Prefill-Position: kein Token, kein Feedback.
                    break;
                }
                Err(e) => {
                    eprintln!("[coordinator] Shard {} lehnte ab: {}", shard_idx, e);
                    break;
                }
            }
        }
        (trace, signatures, token_out, feedback_out)
    }

    /// Baut ein PoI-Bündel aus den abgeschlossenen Segmenten dieser
    /// Epoche (Anhang A.1, Kap. 4.4).
    pub fn build_poi_bundle(&self) -> Result<PoIBundle, String> {
        if self.completed.is_empty() {
            return Err("keine abgeschlossenen Segmente".to_string());
        }
        let ids: Vec<SegmentId> = self.completed.iter().map(|c| c.id).collect();
        let root = segments_root(&ids).map_err(|e| e.to_string())?;
        // vTFE: Anzahl der Segmente (Platzhalter für die FLOPs-Metrik).
        let vtfe = self.completed.len() as u64;
        // Aggregat über die Übergangs-Signaturen (alle dieselbe Arbeit).
        let all_sigs: Vec<BlsSignature> = self
            .completed
            .iter()
            .flat_map(|c| c.signatures.clone())
            .collect();
        let agg = if all_sigs.is_empty() {
            BlsSignature([0u8; 96])
        } else {
            let a = aggregate_signatures(&all_sigs).map_err(|e| e.to_string())?;
            BlsSignature(a.0)
        };
        Ok(PoIBundle {
            epoch: self.epoch,
            pod: self.pod_id,
            segments_root: root,
            vtfe_claimed: vtfe,
            aggregate_sig: agg,
        })
    }

    /// Die abgeschlossenen Segmente (für Tests/Inspektion).
    pub fn completed_segments(&self) -> &[CompletedSegment] {
        &self.completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_default() {
        assert_eq!(DEFAULT_WINDOW_MS, 250);
    }
}
