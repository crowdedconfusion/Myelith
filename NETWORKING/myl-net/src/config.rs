//! Netz-Konfiguration — die am 2026-08-13 entschiedenen Parameter.
//!
//! Die Latenz-Parameter sind bewusst Konstanten mit Dokumentation: Sie
//! werden später Parameter der Governance-Registry (GOVERNANCE Punkt 1.1)
//! und dann ohne Code-Änderung anpassbar. Alle Zeitwerte in
//! Ganzzahl-Millisekunden, die EMA-Glättung als Festkomma-Bruch — keine
//! Gleitkomma-Arithmetik (Projekt-Konvention, Konsensnähe).

use std::time::Duration;

/// Ping-Intervall je aktivem Peer (Design-Entscheidung 2026-08-13: 15 s).
pub const PING_INTERVAL: Duration = Duration::from_secs(15);

/// Intervall für signierte Latenz-Atteste ins Gossip
/// (Design-Entscheidung 2026-08-13: 5 Minuten).
pub const LATENCY_ATTEST_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// EMA-Glättung des RTT als Festkomma-Bruch α = 1/4 (= 0,25).
/// Update-Regel in Ganzzahlen:
/// `ema = ema + (sample - ema) * EMA_ALPHA_NUM / EMA_ALPHA_DEN`.
pub const EMA_ALPHA_NUM: u64 = 1;
/// Nenner des EMA-Glättungsfaktors (α = `EMA_ALPHA_NUM / EMA_ALPHA_DEN`).
pub const EMA_ALPHA_DEN: u64 = 4;

/// Protokoll-Version des Identify-Protokolls (Teil des Handshakes;
/// ändert sich bei Wire-Format-Änderungen).
pub const PROTOCOL_VERSION: &str = "myelith/0.1";

/// Agent-String des Identify-Protokolls (Anzeige/Diagnose, kein
/// Konsens-Feld).
pub const AGENT_VERSION: &str = "myl-net/0.1.1";

/// Maximale Größe einer Gossip-Nachricht in Bytes (Schutz gegen
/// Gossip-Spam; die Validierung in Punkt 1.4 prüft zusätzlich
/// Signaturen und Größenlimits je Topic). 4 MiB lassen Luft für
/// Blöcke und PoI-Bündel, begrenzen aber Missbrauch.
pub const MAX_GOSSIP_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Netz-Konfiguration eines Nodes.
#[derive(Debug, Clone)]
pub struct NetConfig {
    /// Listen-Adresse (Multiaddr-Format, z. B. `/ip4/0.0.0.0/tcp/4150`).
    pub listen_addr: String,
    /// Bootstrap-Peers (Multiaddr mit `p2p/…`-Anteil) für den Einstieg
    /// ins Netz (Punkt 1.2).
    pub bootstrap_peers: Vec<String>,
    /// Ping-Intervall (Standard: [`PING_INTERVAL`]).
    pub ping_interval: Duration,
    /// Attest-Intervall (Standard: [`LATENCY_ATTEST_INTERVAL`]).
    pub latency_attest_interval: Duration,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/4150".to_string(),
            bootstrap_peers: Vec::new(),
            ping_interval: PING_INTERVAL,
            latency_attest_interval: LATENCY_ATTEST_INTERVAL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_festkomma_ist_viertel() {
        // α = 1/4: ein Sample von 1000 ms zieht einen EMA von 0 auf 250 ms.
        let ema: u64 = 0;
        let sample: u64 = 1000;
        let updated = ema + (sample - ema) * EMA_ALPHA_NUM / EMA_ALPHA_DEN;
        assert_eq!(updated, 250);
    }

    #[test]
    fn standardwerte_sind_die_entschiedenen() {
        let cfg = NetConfig::default();
        assert_eq!(cfg.ping_interval, Duration::from_secs(15));
        assert_eq!(cfg.latency_attest_interval, Duration::from_secs(300));
        assert!(cfg.bootstrap_peers.is_empty());
    }
}
