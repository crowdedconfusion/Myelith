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

use crate::config::{EMA_ALPHA_DEN, EMA_ALPHA_NUM};

/// Ping-Intervall in Sekunden (Konsens-Feld).
pub const PING_INTERVAL_SECS: u64 = 15;

/// Maximale sinnvolle RTT (über 10 s wird als Peer-Ausfall behandelt).
///
/// Eine untere Grenze gibt es nicht: In realen Netzen ist die RTT größer
/// als null, in der CI kann eine Antwort schneller kommen als die Uhr
/// auflöst, und ein Mindestwert würde dort jede Messung verwerfen.
/// Wie lange ein unbeantworteter Ping vorgehalten wird.
///
/// Danach gilt der Pong als verloren und der Eintrag wird geräumt, damit
/// verlorene Antworten den Speicher nicht anwachsen lassen.
pub const PING_FRIST: Duration = Duration::from_secs(5);

const MAX_RTT_US: u64 = 10_000_000;

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
///
/// ## ⚑ Fund 44: Diese EMA rechnete in `f64` (behoben 2026-08-23)
///
/// Der Kopf von `lib.rs` sagt seit dem ersten Tag zu: „die EMA rechnet in
/// Ganzzahlen (Festkomma), keine Gleitkomma-Arithmetik". `config.rs`
/// führt dafür sogar die Konstanten `EMA_ALPHA_NUM`/`EMA_ALPHA_DEN` mit
/// der ganzzahligen Formel im Kommentar. **Gerechnet wurde trotzdem in
/// `f64`**, und die ganzzahligen Konstanten hatten außer einem Modultest
/// keinen Aufrufer: die richtige Fassung lag daneben und lief nicht.
///
/// Der Gleitkomma-Audit hätte das finden müssen und konnte es nicht:
/// **`myl-net` steht mit keiner einzigen Datei in seiner Liste.** Der Lauf
/// meldet „null Treffer über 57 Dateien", und die Datei mit dem Treffer
/// war keine davon. Ein Audit sagt nur etwas über das, was es ansieht;
/// die Zahl 57 klang nach Vollständigkeit und war eine Auswahl.
/// Nachgetragen, siehe `INTEGER_LLM/tests/audit/test_no_float.py`.
///
/// **Was es nicht war:** ein Determinismusbruch. `0,25·x + 0,75·y` ist in
/// IEEE-754 exakt festgelegt und plattformgleich, anders als das
/// `f64::log2()` aus Fund A18. Der geglättete Wert ist außerdem eine
/// **lokale Messung**, die als signiertes `u32`-Attest ins Netz geht
/// (`myl_types::LatencyAttest`); zwei Knoten müssen sie nie gleich
/// ausrechnen.
///
/// **Warum trotzdem jetzt:** Die Brücke von hier zum signierten Attest
/// ist noch nicht gebaut. Danach hätte jede Änderung an der Glättung die
/// Attestwerte verschoben, und aus einer Aufräumarbeit wäre eine
/// Protokolländerung geworden. Der billigste Zeitpunkt ist der, an dem
/// noch niemand darauf zeigt.
///
/// Gerechnet wird jetzt in **Mikrosekunden**, weil Millisekunden als
/// Ganzzahl für Nachbarn im selben Rechenzentrum (RTT unter 1 ms) alles
/// auf null runden würden, und genau diese Nachbarschaft ist das, was das
/// Geo-Clustering sucht.
#[derive(Debug, Clone)]
pub struct PeerLatency {
    /// Geglättete RTT in **Mikrosekunden** (EMA, α = 1/4).
    pub smoothed_rtt_us: u64,
    /// Anzahl der durchgeführten Messungen.
    pub measurement_count: u64,
    /// Zeitpunkt der letzten erfolgreichen Messung.
    pub last_measurement: Instant,
}

impl PeerLatency {
    /// Neue Latenz-Messung mit Initialwert.
    fn new(initial_rtt_us: u64) -> Self {
        Self {
            smoothed_rtt_us: initial_rtt_us,
            measurement_count: 1,
            last_measurement: Instant::now(),
        }
    }

    /// Aktualisiert den geglätteten RTT-Wert mit einer neuen Messung.
    ///
    /// Formel: `neu = alt + (probe − alt) · α`, mit α = `EMA_ALPHA_NUM /
    /// EMA_ALPHA_DEN` = 1/4, gerechnet in Ganzzahlen.
    ///
    /// **Warum diese Form und nicht `α·probe + (1−α)·alt`:** Die zweite
    /// rundet zweimal ab. Diese hier rundet nur die Differenz, und sie
    /// kommt ohne Zwischenwert aus, der größer als beide Eingaben wäre.
    ///
    /// Die Differenz wird **vorzeichenrichtig** gebildet: Bei sinkender
    /// Latenz ist `probe < alt`, und eine Subtraktion in `u64` liefe dort
    /// um. Das wäre im Debug-Build eine Panik und im Release-Build eine
    /// Latenz nahe 2⁶⁴, also genau der Wert, mit dem ein Angreifer sich
    /// aus jedem Pod herausrechnen könnte.
    fn update(&mut self, probe_us: u64) {
        self.smoothed_rtt_us = if probe_us >= self.smoothed_rtt_us {
            let d = probe_us - self.smoothed_rtt_us;
            self.smoothed_rtt_us + d * EMA_ALPHA_NUM / EMA_ALPHA_DEN
        } else {
            let d = self.smoothed_rtt_us - probe_us;
            self.smoothed_rtt_us - d * EMA_ALPHA_NUM / EMA_ALPHA_DEN
        };
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
        // den Zeitstempeln — die könnten zwischen Peers divergieren).
        // `as_micros()` liefert u128; ein RTT jenseits von u64-Mikro-
        // sekunden ist eine halbe Million Jahre und fällt ohnehin unter
        // die Plausibilitätsgrenze.
        let rtt_us = u64::try_from(pending.sent_at.elapsed().as_micros()).unwrap_or(u64::MAX);

        // Plausibilitätsprüfung (extreme Werte filtern)
        if rtt_us > MAX_RTT_US {
            return false;
        }

        // Latenz aktualisieren (EMA-Glättung)
        if let Some(latency) = self.peer_latencies.get_mut(&peer) {
            latency.update(rtt_us);
        } else {
            self.peer_latencies.insert(peer, PeerLatency::new(rtt_us));
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

    /// Entfernt veraltete Pending-Pings (Frist [`PING_FRIST`]).
    ///
    /// Sollte regelmäßig aufgerufen werden (z. B. alle 10 s), um
    /// Speicherlecks durch verlorene Pong-Antworten zu vermeiden.
    pub fn cleanup_stale_pings(&mut self) {
        self.cleanup_stale_pings_zu(Instant::now());
    }

    /// Wie [`Self::cleanup_stale_pings`], aber mit ausdrücklichem Jetzt.
    ///
    /// ⚑ **Nicht nur der Prüfung wegen.** Solange die Funktion die Uhr
    /// selbst las, war ihre Grenze **gar nicht prüfbar**: Ein Test
    /// konnte nur echte Zeit verstreichen lassen, und ein Test, der
    /// sechs Sekunden schläft, prüft die Grenze trotzdem nicht, sondern
    /// nur einen Punkt weit dahinter. Mit dem Jetzt als Argument sind
    /// **beide Seiten der Frist** prüfbar, und es kostet keine Zeit.
    pub fn cleanup_stale_pings_zu(&mut self, jetzt: Instant) {
        self.pending_pings
            .retain(|_, pending| jetzt.duration_since(pending.sent_at) < PING_FRIST);
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
        let latency = PeerLatency::new(100_000);
        assert_eq!(latency.smoothed_rtt_us, 100_000);
        assert_eq!(latency.measurement_count, 1);
    }

    #[test]
    fn ema_update_converges() {
        let mut latency = PeerLatency::new(100_000);
        for _ in 0..40 {
            latency.update(50_000);
        }
        // **Totzone:** Ganzzahlig bleibt die EMA stehen, sobald der
        // Abstand kleiner als `EMA_ALPHA_DEN / EMA_ALPHA_NUM` = 4 µs ist,
        // denn dann ist `d · 1 / 4 == 0`. Sie konvergiert also nicht auf
        // den Zielwert, sondern bis auf drei Mikrosekunden an ihn heran
        // und bleibt dort. Dasselbe Verhalten wie bei der EMA in
        // TOKENOMICS, wo die Totzone ebenfalls dokumentiert ist.
        //
        // Bei Latenzen im Millisekundenbereich sind drei Mikrosekunden
        // belanglos. Der Test hält die Grenze fest, damit niemand später
        // exakte Konvergenz annimmt.
        let abstand = latency.smoothed_rtt_us.abs_diff(50_000);
        assert!(abstand < 4, "Totzone verletzt: Abstand {abstand} µs");
    }

    #[test]
    fn ema_smoothing_factor() {
        let mut latency = PeerLatency::new(100_000);
        latency.update(200_000);
        // 100 000 + (200 000 − 100 000)/4 = 125 000, exakt.
        assert_eq!(latency.smoothed_rtt_us, 125_000);
    }

    /// Bei **sinkender** Latenz wird die Differenz nach unten gebildet.
    /// Eine Subtraktion in `u64` liefe hier um: im Debug-Build eine Panik,
    /// im Release-Build eine Latenz nahe 2⁶⁴.
    #[test]
    fn ema_sinkt_ohne_umlauf() {
        let mut latency = PeerLatency::new(200_000);
        latency.update(100_000);
        assert_eq!(latency.smoothed_rtt_us, 175_000);

        let mut null = PeerLatency::new(0);
        null.update(0);
        assert_eq!(null.smoothed_rtt_us, 0);

        let mut extrem = PeerLatency::new(u64::MAX);
        extrem.update(0);
        assert!(extrem.smoothed_rtt_us < u64::MAX);
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
        // RTT sollte klein sein (sofortige Antwort simuliert)
        assert!(latency.smoothed_rtt_us < 100_000);
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

    /// ⚑ **Beide Seiten der Frist, und ohne zu warten.**
    ///
    /// ⛑ Hier stand ein Test, der **sechs Sekunden schlief**, um einen
    /// Ping veralten zu lassen. Er kostete die ganze Suite sechs
    /// Sekunden und prüfte die Grenze **nicht**: Er lag weit dahinter
    /// und hätte auch eine Frist von einer Sekunde bestanden.
    #[test]
    fn die_pingfrist_gilt_an_beiden_raendern() {
        let mut tracker = LatencyTracker::new();
        let peer = PeerId::random();
        let _ping = tracker.create_ping(peer);
        assert_eq!(tracker.pending_pings.len(), 1);

        let gesendet = tracker.pending_pings[&peer].sent_at;

        // Genau auf der Frist ist noch nicht darüber.
        tracker.cleanup_stale_pings_zu(gesendet + PING_FRIST - Duration::from_millis(1));
        assert_eq!(tracker.pending_pings.len(), 1, "vor der Frist geräumt");

        // Und einen Wimpernschlag danach ist er weg.
        tracker.cleanup_stale_pings_zu(gesendet + PING_FRIST);
        assert_eq!(tracker.pending_pings.len(), 0, "nach der Frist geblieben");
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
