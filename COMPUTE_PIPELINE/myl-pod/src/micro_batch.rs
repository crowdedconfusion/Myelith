//! Micro-Batching-Fenster und Pipelining (Whitepaper Kap. 4.2).
//!
//! Der Koordinator sammelt eingehende Inferenz-Anfragen über ein Zeitfenster
//! (WINDOW_MS, default 250 ms) und batcht sie für effizientere Verarbeitung.
//! Pipelining ermöglicht die gleichzeitige Verarbeitung mehrerer Sessions.
//!
//! **Konsens-Feld:** Das Micro-Batching-Fenster ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:**
//! - WINDOW_MS: Zeitfenster für Batch-Sammlung (default 250 ms)
//! - MAX_BATCH_SIZE: Maximale Anzahl von Sessions pro Batch (default 32)
//! - Pipelining: Überlappende Verarbeitung mehrerer Batches

use std::collections::VecDeque;
use std::time::Instant;

use myl_types::ids::SegmentId;

/// Default Micro-Batching-Fenster in Millisekunden.
pub const DEFAULT_WINDOW_MS: u64 = 250;

/// Maximale Batch-Größe (Anzahl Sessions pro Batch).
pub const MAX_BATCH_SIZE: usize = 32;

/// Eine Inferenz-Anfrage für ein Segment.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Prompt-Tokens.
    pub prompt_tokens: Vec<u32>,
    /// Maximale Anzahl neuer Tokens.
    pub max_new_tokens: usize,
    /// Zeitpunkt des Eingangs.
    pub received_at: Instant,
}

/// Ein Micro-Batch von Inferenz-Anfragen.
#[derive(Debug, Clone)]
pub struct MicroBatch {
    /// Batch-ID (sequentiell).
    pub batch_id: u64,
    /// Anfragen im Batch.
    pub requests: Vec<InferenceRequest>,
    /// Zeitpunkt der Batch-Erstellung.
    pub created_at: Instant,
}

/// Micro-Batch-Collector für den Koordinator.
///
/// Sammelt eingehende Anfragen über ein Zeitfenster und erstellt Batches.
/// Unterstützt Pipelining durch überlappende Verarbeitung.
pub struct MicroBatchCollector {
    /// Zeitfenster für Batch-Sammlung.
    window_ms: u64,
    /// Maximale Batch-Größe.
    max_batch_size: usize,
    /// Wartende Anfragen.
    pending: VecDeque<InferenceRequest>,
    /// Nächste Batch-ID.
    next_batch_id: u64,
    /// Zeitpunkt des letzten Batch-Abschlusses.
    last_batch_at: Option<Instant>,
}

impl MicroBatchCollector {
    /// Erstellt einen neuen Micro-Batch-Collector.
    ///
    /// **Parameter:**
    /// - `window_ms`: Zeitfenster für Batch-Sammlung (default: DEFAULT_WINDOW_MS)
    /// - `max_batch_size`: Maximale Batch-Größe (default: MAX_BATCH_SIZE)
    pub fn new(window_ms: u64, max_batch_size: usize) -> Self {
        Self {
            window_ms,
            max_batch_size,
            pending: VecDeque::new(),
            next_batch_id: 0,
            last_batch_at: None,
        }
    }

    /// Erstellt einen Collector mit Default-Parametern.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_WINDOW_MS, MAX_BATCH_SIZE)
    }

    /// Fügt eine Anfrage zur Warteschlange hinzu.
    pub fn enqueue(&mut self, request: InferenceRequest) {
        self.pending.push_back(request);
    }

    /// Prüft, ob ein Batch bereit ist (Fenster abgelaufen oder Max-Größe erreicht).
    ///
    /// **Returns:** `true` wenn ein Batch erstellt werden sollte.
    pub fn should_batch(&self) -> bool {
        if self.pending.is_empty() {
            return false;
        }

        // Max-Größe erreicht
        if self.pending.len() >= self.max_batch_size {
            return true;
        }

        // Zeitfenster abgelaufen (oder erster Batch nach Fenster)
        if let Some(last) = self.last_batch_at {
            let elapsed = last.elapsed().as_millis() as u64;
            if elapsed >= self.window_ms {
                return true;
            }
        }
        // Für den allerersten Batch: Kein sofortiges Batching, warte auf Fenster

        false
    }

    /// Erstellt einen neuen Batch aus den wartenden Anfragen.
    ///
    /// **Returns:** `Some(MicroBatch)` wenn Anfragen vorhanden, `None` sonst.
    pub fn create_batch(&mut self) -> Option<MicroBatch> {
        if self.pending.is_empty() {
            return None;
        }

        let batch_size = self.pending.len().min(self.max_batch_size);
        let requests: Vec<InferenceRequest> = self.pending.drain(..batch_size).collect();

        let batch = MicroBatch {
            batch_id: self.next_batch_id,
            requests,
            created_at: Instant::now(),
        };

        self.next_batch_id += 1;
        self.last_batch_at = Some(batch.created_at);

        Some(batch)
    }

    /// Gibt die Anzahl wartender Anfragen zurück.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Gibt das konfigurierte Zeitfenster zurück.
    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }

    /// Gibt die maximale Batch-Größe zurück.
    pub fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }
}

impl Default for MicroBatchCollector {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Pipeline-Status für überlappende Batch-Verarbeitung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    /// Batch wird empfangen.
    Receiving,
    /// Batch wird verarbeitet (Forward-Pass).
    Processing,
    /// Batch wird signiert und weitergeleitet.
    Finalizing,
    /// Batch ist abgeschlossen.
    Completed,
}

/// Pipeline-Tracker für überlappende Batch-Verarbeitung.
///
/// Verfolgt den Status mehrerer Batches, die sich in verschiedenen
/// Verarbeitungsstadien befinden können.
pub struct PipelineTracker {
    /// Aktive Batches und ihre Stadien.
    batches: Vec<(u64, PipelineStage)>,
    /// Maximale Anzahl gleichzeitiger Batches.
    max_concurrent: usize,
}

impl PipelineTracker {
    /// Erstellt einen neuen Pipeline-Tracker.
    ///
    /// **Parameter:**
    /// - `max_concurrent`: Maximale Anzahl gleichzeitiger Batches (default: 4)
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            batches: Vec::new(),
            max_concurrent,
        }
    }

    /// Fügt einen neuen Batch zur Pipeline hinzu.
    ///
    /// **Returns:** `true` wenn der Batch hinzugefügt wurde, `false` wenn Pipeline voll.
    pub fn add_batch(&mut self, batch_id: u64) -> bool {
        if self.batches.len() >= self.max_concurrent {
            return false;
        }

        self.batches.push((batch_id, PipelineStage::Receiving));
        true
    }

    /// Aktualisiert das Stadium eines Batches.
    pub fn advance_stage(&mut self, batch_id: u64) -> bool {
        if let Some((_, stage)) = self.batches.iter_mut().find(|(id, _)| *id == batch_id) {
            *stage = match *stage {
                PipelineStage::Receiving => PipelineStage::Processing,
                PipelineStage::Processing => PipelineStage::Finalizing,
                PipelineStage::Finalizing => PipelineStage::Completed,
                PipelineStage::Completed => PipelineStage::Completed,
            };
            true
        } else {
            false
        }
    }

    /// Entfernt abgeschlossene Batches aus der Pipeline.
    pub fn cleanup_completed(&mut self) -> Vec<u64> {
        let mut completed = Vec::new();
        self.batches.retain(|(id, stage)| {
            if *stage == PipelineStage::Completed {
                completed.push(*id);
                false
            } else {
                true
            }
        });
        completed
    }

    /// Gibt die Anzahl aktiver Batches zurück.
    pub fn active_count(&self) -> usize {
        self.batches.len()
    }

    /// Gibt das Stadium eines Batches zurück.
    pub fn get_stage(&self, batch_id: u64) -> Option<PipelineStage> {
        self.batches
            .iter()
            .find(|(id, _)| *id == batch_id)
            .map(|(_, stage)| *stage)
    }

    /// Prüft, ob die Pipeline Kapazität für weitere Batches hat.
    pub fn has_capacity(&self) -> bool {
        self.batches.len() < self.max_concurrent
    }
}

impl Default for PipelineTracker {
    fn default() -> Self {
        Self::new(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_request(segment_byte: u8) -> InferenceRequest {
        InferenceRequest {
            segment_id: SegmentId::new([segment_byte; 32]),
            prompt_tokens: vec![1, 2, 3],
            max_new_tokens: 10,
            received_at: Instant::now(),
        }
    }

    #[test]
    fn collector_enqueue() {
        let mut collector = MicroBatchCollector::with_defaults();
        collector.enqueue(test_request(1));
        collector.enqueue(test_request(2));

        assert_eq!(collector.pending_count(), 2);
    }

    #[test]
    fn collector_should_batch_max_size() {
        let mut collector = MicroBatchCollector::new(250, 2);
        collector.enqueue(test_request(1));
        
        assert!(!collector.should_batch());
        
        collector.enqueue(test_request(2));
        assert!(collector.should_batch()); // Max-Größe erreicht
    }

    #[test]
    fn collector_create_batch() {
        let mut collector = MicroBatchCollector::with_defaults();
        collector.enqueue(test_request(1));
        collector.enqueue(test_request(2));

        let batch = collector.create_batch().expect("batch should exist");
        assert_eq!(batch.batch_id, 0);
        assert_eq!(batch.requests.len(), 2);
        assert_eq!(collector.pending_count(), 0);
    }

    #[test]
    fn collector_batch_id_increments() {
        let mut collector = MicroBatchCollector::with_defaults();
        
        collector.enqueue(test_request(1));
        let batch1 = collector.create_batch().unwrap();
        assert_eq!(batch1.batch_id, 0);

        collector.enqueue(test_request(2));
        let batch2 = collector.create_batch().unwrap();
        assert_eq!(batch2.batch_id, 1);
    }

    #[test]
    fn collector_empty_batch() {
        let mut collector = MicroBatchCollector::with_defaults();
        assert!(collector.create_batch().is_none());
    }

    #[test]
    fn pipeline_add_batch() {
        let mut pipeline = PipelineTracker::new(2);
        
        assert!(pipeline.add_batch(0));
        assert!(pipeline.add_batch(1));
        assert!(!pipeline.add_batch(2)); // Pipeline voll
        
        assert_eq!(pipeline.active_count(), 2);
    }

    #[test]
    fn pipeline_advance_stage() {
        let mut pipeline = PipelineTracker::new(4);
        pipeline.add_batch(0);

        assert_eq!(pipeline.get_stage(0), Some(PipelineStage::Receiving));
        
        pipeline.advance_stage(0);
        assert_eq!(pipeline.get_stage(0), Some(PipelineStage::Processing));
        
        pipeline.advance_stage(0);
        assert_eq!(pipeline.get_stage(0), Some(PipelineStage::Finalizing));
        
        pipeline.advance_stage(0);
        assert_eq!(pipeline.get_stage(0), Some(PipelineStage::Completed));
    }

    #[test]
    fn pipeline_cleanup_completed() {
        let mut pipeline = PipelineTracker::new(4);
        pipeline.add_batch(0);
        pipeline.add_batch(1);

        // Batch 0 abschließen
        pipeline.advance_stage(0);
        pipeline.advance_stage(0);
        pipeline.advance_stage(0);

        let completed = pipeline.cleanup_completed();
        assert_eq!(completed, vec![0]);
        assert_eq!(pipeline.active_count(), 1);
    }

    #[test]
    fn pipeline_has_capacity() {
        let mut pipeline = PipelineTracker::new(2);
        
        assert!(pipeline.has_capacity());
        
        pipeline.add_batch(0);
        pipeline.add_batch(1);
        
        assert!(!pipeline.has_capacity());
    }

    #[test]
    fn default_constants() {
        assert_eq!(DEFAULT_WINDOW_MS, 250);
        assert_eq!(MAX_BATCH_SIZE, 32);
    }
}
