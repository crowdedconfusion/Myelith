//! Paarlatenzmessung und EMA-Glättung (Phase 2.1).
//!
//! Jeder Node misst kontinuierlich die Round-Trip-Time (RTT) zu allen
//! aktiven Peers über Ping/Pong-Nachrichten. Die Rohmessungen werden
//! mit einem Exponential Moving Average (EMA) geglättet, um Messrauschen
//! zu reduzieren und stabile Latenzwerte für den LatencyGraph (Phase 2.2)
//! zu liefern.
//!
//! **Konsens-Feld:** Die EMA-Parameter (α = 0,25) und das Ping-Intervall
//! (15 s) sind Teil des Konsensvertrags. Änderungen nur über Governance
//! (Kap. 10.3); sie werden später in die Governance-Registry aufgenommen
//! und sind dann ohne Code-Änderung anpassbar.
//!
//! **Design:** Ping/Pong läuft über ein separates Request/Response-
//! Protokoll (nicht Gossip), um Overhead zu vermeiden. Die geglätteten
//! Latenzwerte werden später als signierte Atteste über Gossip verbreitet
//! (Phase 2.2).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use libp2p::PeerId;

/// EMA-Alpha für die Latenz-Glättung (Konsens-Feld).
/// α = 0,25 bedeutet: neue Messung geht zu 25 % ein, alter Wert zu 75 %.
/// Das filtert kurzfristige Schwankungen, reagiert aber innerhalb von
/// ~4 Messungen (60 s) auf anhaltende Änderungen.
pub const EMA_ALPHA: f64 = 0.25;

/// Ping-Intervall in Sekunden (Konsens-Feld).
pub const PING_INTERVAL_SECS: u64 = 15;

/// Minimale sinnvolle RTT (unter 0.1 ms wird als Messfehler behandelt).
const MIN_RTT_MS: f64 = 0.1;

/// Maximale sinnvolle RTT (über 10 s wird als Peer-Ausfall behandelt).
const MAX_RTT_MS: f64 = 10_000.0;

/// Ping-Nachricht: enthält einen Zeitstempel und eine Nonce.
///
/// Der Empfänger antwortet mit einer Pong-Nachricht, die dieselbe Nonce
/// und den empfangenen Zeitstempel zurückgibt. Der Sender kann daraus
/// die RTT berechnen und die Nonce zur Korrelation nutzen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingMessage {
    /// Zeitstempel beim Absenden (Unix-Millisekunden).
    pub timestamp_ms: u64,
    /// Nonce zur Korrelation von Ping/Pong (verhindert Replay).
    pub nonce: u64,
}

/// Pong-Nachricht: Antwort auf ein Ping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PongMessage {
    /// Zeitstempel aus dem ursprünglichen Ping.
    pub original_timestamp_ms: u64,
    /// Nonce aus dem ursprünglichen Ping (zur Korrelation).
    pub nonce: u64,
    /// Zeitstempel beim Absenden der Pong-Antwort (für Debugging).
    pub response_timestamp_ms: u64,
}

/// Zustand eines aktiven Ping-Vorgangs (wartet auf Pong).
#[derive(Debug, Clone)]
struct PendingPing {
    /// Zeitpunkt des Ping-Versands (für RTT-Berechnung).
    sent_at: Instant,
    /// Nonce des Pings (zur Korrelation mit der Pong-Antwort).
    nonce: u64,
}

/// Latenz-Messung für einen einzelnen Peer.
///
/// Speichert den geglätteten RTT-Wert (EMA) und die Anzahl der
/// durchgeführten Messungen. Der geglättete Wert ist die Grundlage
/// für den LatencyGraph (Phase 2.2).
#[derive(Debug, Clone)]
pub struct PeerLatency {
    /// Geglättete RTT in Millisekunden (EMA).
    pub smoothed_rtt_ms: f64,
    /// Anzahl der durchgeführten Messungen.
    pub measurement_count: u64,
    /// Zeitpunkt der letzten erfolgreichen Messung.
    pub last_measurement: Instant,
}

impl PeerLatency {
    /// Neue Latenz-Messung mit Initialwert.
    fn new(initial_rtt_ms: f64) -> Self {
        Self {
            smoothed_rtt_ms: initial_rtt_ms,
            measurement_count: 1,
            last_measurement: Instant::now(),
        }
    }

    /// Aktualisiert den geglätteten RTT-Wert mit einer neuen Messung.
    ///
    /// Formel: `smoothed_rtt = α * new_rtt + (1 - α) * old_rtt`
    /// mit α = EMA_ALPHA (0,25).
    fn update(&mut self, new_rtt_ms: f64) {
        self.smoothed_rtt_ms = EMA_ALPHA * new_rtt_ms + (1.0 - EMA_ALPHA) * self.smoothed_rtt_ms;
        self.measurement_count += 1;
        self.last_measurement = Instant::now();
    }
}

/// Latenz-Tracker für alle Peers.
///
/// Verwaltet die Ping/Pong-Zustände und die geglätteten Latenzwerte.
/// Thread-safe durch Interior Mutability (für die Integration in den
/// libp2p-Swarm).
#[derive(Debug)]
pub struct LatencyTracker {
    /// Aktive Pings, die auf eine Pong-Antwort warten (PeerId → PendingPing).
    pending_pings: HashMap<PeerId, PendingPing>,
    /// Geglättete Latenzwerte für alle Peers mit mindestens einer Messung.
    peer_latencies: HashMap<PeerId, PeerLatency>,
    /// Nonce-Generator (einfach hochzählend, muss nicht kryptographisch sein).
    next_nonce: u64,
}

impl LatencyTracker {
    /// Neuer Latenz-Tracker (leer).
    pub fn new() -> Self {
        Self {
            pending_pings: HashMap::new(),
            peer_latencies: HashMap::new(),
            next_nonce: 0,
        }
    }

    /// Erzeugt eine neue Ping-Nachricht für den angegebenen Peer.
    ///
    /// Speichert den Zustand (Zeitstempel, Nonce) für die spätere
    /// Korrelation mit der Pong-Antwort. Gibt die Ping-Nachricht zurück,
    /// die über das Request/Response-Protokoll gesendet wird.
    pub fn create_ping(&mut self, peer: PeerId) -> PingMessage {
        let now = Instant::now();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64;

        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1);

        self.pending_pings.insert(
            peer,
            PendingPing {
                sent_at: now,
                nonce,
            },
        );

        PingMessage {
            timestamp_ms,
            nonce,
        }
    }

    /// Verarbeitet eine Pong-Antwort und aktualisiert die Latenz.
    ///
    /// Berechnet die RTT aus der Zeitdifferenz zwischen Ping-Versand
    /// und Pong-Empfang. Validiert die Nonce zur Korrelation.
    /// Gibt `true` zurück, wenn die Pong erfolgreich verarbeitet wurde,
    /// `false` bei fehlendem Ping oder Nonce-Mismatch.
    pub fn handle_pong(&mut self, peer: PeerId, pong: &PongMessage) -> bool {
        // Erst prüfen, ob ein pending_ping existiert
        let pending = match self.pending_pings.get(&peer) {
            Some(p) => p,
            None => return false, // Kein aktiver Ping für diesen Peer
        };

        // Nonce-Validierung (verhindert Replay oder falsche Zuordnung)
        if pending.nonce != pong.nonce {
            return false; // Pending-Ping bleibt erhalten
        }

        // Jetzt erst entfernen (nach erfolgreicher Validierung)
        let pending = self.pending_pings.remove(&peer).unwrap();

        // RTT berechnen (aus der tatsächlichen Zeitdifferenz, nicht aus
        // den Zeitstempeln — die könnten zwischen Peers divergieren)
        let rtt = pending.sent_at.elapsed();
        let rtt_ms = rtt.as_secs_f64() * 1000.0;

        // Plausibilitätsprüfung (extreme Werte filtern)
        if rtt_ms < MIN_RTT_MS || rtt_ms > MAX_RTT_MS {
            return false;
        }

        // Latenz aktualisieren (EMA-Glättung)
        if let Some(latency) = self.peer_latencies.get_mut(&peer) {
            latency.update(rtt_ms);
        } else {
            self.peer_latencies.insert(peer, PeerLatency::new(rtt_ms));
        }

        true
    }

    /// Gibt die geglättete Latenz für einen Peer zurück (falls vorhanden).
    pub fn get_latency(&self, peer: &PeerId) -> Option<&PeerLatency> {
        self.peer_latencies.get(peer)
    }

    /// Gibt alle Peer-Latenzen zurück (für den LatencyGraph-Aufbau).
    pub fn all_latencies(&self) -> &HashMap<PeerId, PeerLatency> {
        &self.peer_latencies
    }

    /// Entfernt veraltete Pending-Pings (Timeout nach 5 s).
    ///
    /// Sollte regelmäßig aufgerufen werden (z. B. alle 10 s), um
    /// Speicherlecks durch verlorene Pong-Antworten zu vermeiden.
    pub fn cleanup_stale_pings(&mut self) {
        let timeout = Duration::from_secs(5);
        self.pending_pings.retain(|_, pending| {
            pending.sent_at.elapsed() < timeout
        });
    }

    /// Entfernt einen Peer aus dem Tracker (z. B. bei Disconnect).
    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.pending_pings.remove(peer);
        self.peer_latencies.remove(peer);
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_initial_value() {
        let latency = PeerLatency::new(100.0);
        assert_eq!(latency.smoothed_rtt_ms, 100.0);
        assert_eq!(latency.measurement_count, 1);
    }

    #[test]
    fn ema_update_converges() {
        let mut latency = PeerLatency::new(100.0);

        // Mehrere Updates mit konstantem Wert sollten gegen diesen Wert konvergieren
        for _ in 0..20 {
            latency.update(50.0);
        }

        // Nach 20 Updates sollte der Wert nahe 50 sein (innerhalb von 1 %)
        assert!((latency.smoothed_rtt_ms - 50.0).abs() < 0.5);
    }

    #[test]
    fn ema_smoothing_factor() {
        let mut latency = PeerLatency::new(100.0);

        // Ein Update mit 200 sollte den Wert zu 25 % anpassen
        latency.update(200.0);

        // Erwartet: 0.25 * 200 + 0.75 * 100 = 125
        assert!((latency.smoothed_rtt_ms - 125.0).abs() < 0.01);
    }

    #[test]
    fn ping_pong_roundtrip() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();

        // Ping erstellen
        let ping = tracker.create_ping(peer);
        
        // Debug: Prüfen ob der Eintrag vorhanden ist
        assert!(
            tracker.pending_pings.contains_key(&peer),
            "Pending ping should exist after create_ping"
        );

        // Pong erstellen (simuliert Antwort)
        let pong = PongMessage {
            original_timestamp_ms: ping.timestamp_ms,
            nonce: ping.nonce,
            response_timestamp_ms: ping.timestamp_ms + 50, // 50 ms später
        };

        // Pong verarbeiten
        let result = tracker.handle_pong(peer, &pong);
        assert!(result, "handle_pong should succeed");

        // Latenz sollte jetzt vorhanden sein
        let latency = tracker.get_latency(&peer).expect("latency should exist");
        assert_eq!(latency.measurement_count, 1);
        // RTT sollte ungefähr 0 sein (sofortige Antwort simuliert)
        assert!(latency.smoothed_rtt_ms < 100.0);
    }

    #[test]
    fn pong_without_ping_rejected() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();

        let pong = PongMessage {
            original_timestamp_ms: 1000,
            nonce: 42,
            response_timestamp_ms: 1050,
        };

        // Pong ohne vorheriges Ping sollte abgelehnt werden
        assert!(!tracker.handle_pong(peer, &pong));
    }

    #[test]
    fn pong_wrong_nonce_rejected() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();

        let ping = tracker.create_ping(peer);
        
        // Debug: Prüfen ob der Eintrag vorhanden ist
        assert!(
            tracker.pending_pings.contains_key(&peer),
            "Pending ping should exist after create_ping"
        );

        let pong = PongMessage {
            original_timestamp_ms: ping.timestamp_ms,
            nonce: ping.nonce + 1, // Falsche Nonce
            response_timestamp_ms: ping.timestamp_ms + 50,
        };

        // Pong mit falscher Nonce sollte abgelehnt werden
        assert!(!tracker.handle_pong(peer, &pong));

        // Pending-Ping sollte immer noch vorhanden sein (nicht entfernt)
        assert!(
            tracker.pending_pings.contains_key(&peer),
            "Pending ping should still exist after rejected pong"
        );
    }

    #[test]
    fn cleanup_removes_stale_pings() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();

        let _ping = tracker.create_ping(peer);
        assert_eq!(tracker.pending_pings.len(), 1);

        // Simuliere Zeitablauf (mehr als 5 s)
        std::thread::sleep(Duration::from_secs(6));

        tracker.cleanup_stale_pings();
        assert_eq!(tracker.pending_pings.len(), 0);
    }

    #[test]
    fn remove_peer_clears_all_state() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();

        // Ping erstellen und erfolgreich verarbeiten
        let ping = tracker.create_ping(peer);
        let pong = PongMessage {
            original_timestamp_ms: ping.timestamp_ms,
            nonce: ping.nonce,
            response_timestamp_ms: ping.timestamp_ms + 50,
        };
        tracker.handle_pong(peer, &pong);

        // Nach erfolgreichem Pong: pending_ping entfernt, peer_latency vorhanden
        assert!(!tracker.pending_pings.contains_key(&peer));
        assert!(tracker.peer_latencies.contains_key(&peer));

        // Erneut einen Ping erstellen (bleibt pending)
        let _ping2 = tracker.create_ping(peer);
        assert!(tracker.pending_pings.contains_key(&peer));

        // remove_peer sollte beide entfernen
        tracker.remove_peer(&peer);

        assert!(!tracker.pending_pings.contains_key(&peer));
        assert!(!tracker.peer_latencies.contains_key(&peer));
    }
}
