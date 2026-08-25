//! Latenz-Atteste für den LatencyGraph (Whitepaper Kap. 4.1, Anhang A.2).
//!
//! Jeder Node signiert regelmäßig seine gemessenen Latenzen zu anderen
//! Peers und verbreitet diese Atteste über Gossip. Andere Nodes können
//! daraus den LatencyGraph aufbauen, der die Grundlage für die Pod-Bildung
//! bildet (Kap. 4.3: Nodes mit niedrigen Latenzen werden bevorzugt).
//!
//! **Konsens-Feld:** Das Attest-Format ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Atteste werden alle 5 Minuten erstellt und über das
//! `/myelith/latency-attests/1` Topic verbreitet. Jeder Node akzeptiert
//! nur Atteste von Peers, die er direkt kennt (keine Weiterverbreitung
//! von Attesten Dritter — verhindert Sybil-Angriffe).

use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::hash::Hash;
use crate::ids::MinerId;

/// Ein signiertes Latenz-Attest eines Nodes.
///
/// Enthält die geglätteten Latenzen zu allen bekannten Peers, signiert
/// mit dem BLS-Schlüssel des Nodes. Andere Nodes können die Signatur
/// verifizieren und die Latenzwerte in ihren LatencyGraph aufnehmen.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct LatencyAttest {
    /// Der Miner, der dieses Attest erstellt hat.
    pub issuer: MinerId,
    /// Zeitstempel der Erstellung (Unix-Millisekunden).
    pub timestamp_ms: u64,
    /// Latenzen zu allen bekannten Peers (PeerId → RTT in ms, EMA-geglättet).
    ///
    /// **Wichtig:** Die PeerIds werden als Bytes serialisiert (32 Bytes).
    /// Die Reihenfolge ist nicht garantiert — Empfänger sollten nach
    /// PeerId sortieren, um deterministische Hashes zu erzeugen.
    pub latencies: Vec<(PeerIdBytes, u32)>,
    /// BLS-Signatur über (issuer, timestamp, latencies).
    ///
    /// Die Signatur verhindert Manipulation: Ein Node kann nur Atteste
    /// für sich selbst erstellen, nicht für andere.
    pub signature: BlsSignatureBytes,
}

/// PeerId als 32-Byte-Array (für Borsh-Serialisierung).
///
/// libp2p::PeerId ist nicht direkt Borsh-serialisierbar, daher speichern
/// wir die Bytes. Die Konvertierung erfolgt in NETWORKING.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize)]
pub struct PeerIdBytes(pub [u8; 32]);

/// BLS-Signatur als 96-Byte-Array (für Borsh-Serialisierung).
///
/// blst::Signature ist nicht direkt Borsh-serialisierbar, daher speichern
/// wir die Bytes. Die Konvertierung erfolgt in NETWORKING.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlsSignatureBytes(pub [u8; 96]);

impl LatencyAttest {
    /// Berechnet den Hash des Attests (für Deduplizierung und Caching).
    ///
    /// Der Hash wird über die serialisierten Bytes berechnet (Borsh ist
    /// kanonisch — dieselbe Struktur ergibt immer dieselben Bytes).
    pub fn hash(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Borsh-Serialisierung sollte nicht fehlschlagen");
        Hash::sha256(&bytes)
    }

    /// Berechnet die zu signierenden Bytes (ohne Signatur).
    ///
    /// Die Signatur wird über (issuer, timestamp, latencies) berechnet,
    /// nicht über das gesamte Attest (sonst wäre die Signatur Teil der
    /// signierten Daten — Zirkelbezug).
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(borsh::to_vec(&self.issuer).expect("Borsh").as_slice());
        bytes.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        // Latenzen sortieren für deterministische Reihenfolge
        let mut sorted_latencies = self.latencies.clone();
        sorted_latencies.sort_by_key(|(peer, _)| *peer);
        bytes.extend_from_slice(borsh::to_vec(&sorted_latencies).expect("Borsh").as_slice());
        bytes
    }

    /// Signiert das Attest mit dem eigenen Schlüssel.
    ///
    /// Setzt [`Self::signature`] auf die Signatur über
    /// [`Self::signable_bytes`].
    ///
    /// # ⚑ A10: Bis zum 2026-08-25 gab es weder Signieren noch Prüfen
    ///
    /// Das Feld `signature` stand hier seit dem ersten Entwurf, und
    /// **niemand hat es je gesetzt oder geprüft**. Der Sicherheitsaudit
    /// führt das als A10 und sagt scharf, warum das schlimmer ist als
    /// ein fehlendes Feld: **Ein ungeprüftes Signaturfeld ist
    /// gefährlicher als gar keines, weil ein Leser es für einen Schutz
    /// hält.**
    ///
    /// Die Latenzwerte gehen ins Geo-Clustering der Pods. Wer sie frei
    /// setzen kann, sucht sich seine Pod-Nachbarn aus, und das ist die
    /// Vorstufe zur Kollusion beider Pods (A12).
    pub fn sign(&mut self, sk: &crate::bls::BlsSecretKey) -> Result<(), crate::bls::BlsError> {
        let sig = sk.sign(&self.signable_bytes())?;
        self.signature = BlsSignatureBytes(sig.0);
        Ok(())
    }

    /// Prüft die Signatur gegen den öffentlichen Schlüssel des
    /// Ausstellers.
    ///
    /// **Der Aufrufer muss den Schlüssel dem Aussteller zuordnen**, und
    /// das kann dieser Typ nicht: Er trägt eine `MinerId`, keinen
    /// Schlüssel. Die Zuordnung leistet die Validator-Registry, und
    /// genau deshalb war A10 nicht zu schließen, solange niemand beide
    /// Seiten in einem Prozess hatte.
    ///
    /// Gibt `false` bei jeder Unstimmigkeit: unlesbare Signatur,
    /// falscher Schlüssel, veränderter Inhalt. **Kein `Result`**, weil
    /// der Aufrufer in allen Fällen dasselbe tut, nämlich verwerfen,
    /// und eine Unterscheidung nur zu einer Verzweigung verführte, die
    /// niemand braucht.
    pub fn verify(&self, pubkey: &crate::bls::BlsPublicKey) -> bool {
        // `BlsSignature` ist ein Tupel über die Rohbytes; die
        // Gültigkeit prüft `verify` selbst und meldet sie als `false`.
        let sig = crate::bls::BlsSignature(self.signature.0);
        pubkey.verify(&self.signable_bytes(), &sig)
    }

    /// Validiert die Struktur des Attests (ohne Signaturprüfung).
    ///
    /// Prüft:
    /// - Zeitstempel ist nicht in der Zukunft (mit 5 min Toleranz)
    /// - Latenzwerte sind plausibel (0-10.000 ms)
    /// - Keine doppelten PeerIds
    pub fn validate_structure(&self) -> Result<(), LatencyAttestError> {
        // Zeitstempel prüfen (nicht mehr als 5 min in der Zukunft)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64;
        let max_future_ms = 5 * 60 * 1000; // 5 Minuten
        if self.timestamp_ms > now_ms + max_future_ms {
            return Err(LatencyAttestError::FutureTimestamp {
                attest_ms: self.timestamp_ms,
                now_ms,
            });
        }

        // Latenzwerte prüfen
        let mut seen_peers = std::collections::HashSet::new();
        for (peer, rtt_ms) in &self.latencies {
            // Plausibilitätsprüfung (0-10.000 ms)
            if *rtt_ms > 10_000 {
                return Err(LatencyAttestError::ImplausibleLatency {
                    peer: *peer,
                    rtt_ms: *rtt_ms,
                });
            }

            // Keine doppelten PeerIds
            if !seen_peers.insert(peer) {
                return Err(LatencyAttestError::DuplicatePeer { peer: *peer });
            }
        }

        Ok(())
    }
}

/// Fehler bei der Validierung eines Latenz-Attests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyAttestError {
    /// Zeitstempel liegt mehr als 5 min in der Zukunft.
    FutureTimestamp { attest_ms: u64, now_ms: u64 },
    /// Latenzwert ist unplausibel (> 10.000 ms).
    ImplausibleLatency { peer: PeerIdBytes, rtt_ms: u32 },
    /// Doppelte PeerId im Attest.
    DuplicatePeer { peer: PeerIdBytes },
}

impl std::fmt::Display for LatencyAttestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FutureTimestamp { attest_ms, now_ms } => {
                write!(
                    f,
                    "Latenz-Attest Zeitstempel {} ms liegt mehr als 5 min in der Zukunft (jetzt: {} ms)",
                    attest_ms, now_ms
                )
            }
            Self::ImplausibleLatency { peer, rtt_ms } => {
                write!(
                    f,
                    "Latenz-Attest enthält unplausible Latenz {} ms für Peer {:?}",
                    rtt_ms, peer
                )
            }
            Self::DuplicatePeer { peer } => {
                write!(f, "Latenz-Attest enthält doppelte PeerId {:?}", peer)
            }
        }
    }
}

impl std::error::Error for LatencyAttestError {}

/// Der LatencyGraph: aggregierte Latenzdaten aller bekannten Peers.
///
/// Jeder Node baut seinen eigenen LatencyGraph aus den empfangenen
/// Attesten auf. Der Graph wird für die Pod-Bildung verwendet (Kap. 4.3):
/// Nodes mit niedrigen Latenzen werden bevorzugt, um die Gesamtlatenz
/// des Pods zu minimieren.
///
/// **Design:** Der Graph ist ungerichtet — wenn Node A eine Latenz zu
/// Node B misst, wird diese als Kante in beide Richtungen gespeichert
/// (Annahme: Latenz ist symmetrisch, was für TCP/UDP weitgehend stimmt).
#[derive(Debug, Clone, Default)]
pub struct LatencyGraph {
    /// Kanten des Graphen: (Node A, Node B) → Latenz in ms.
    ///
    /// **Invariante:** Für jede Kante (A, B) existiert auch (B, A) mit
    /// demselben Wert (ungerichteter Graph).
    edges: HashMap<(MinerId, MinerId), u32>,
    /// Zeitstempel des letzten Attests pro Node (für Aging).
    last_update: HashMap<MinerId, u64>,
}

impl LatencyGraph {
    /// Neuer, leerer LatencyGraph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fügt ein Attest in den Graphen ein.
    ///
    /// Aktualisiert alle Kanten des Issuers und setzt den Zeitstempel.
    /// Vorherige Kanten des Issuers werden überschrieben (das Attest
    /// enthält die aktuellen Latenzen).
    pub fn insert_attest(&mut self, attest: &LatencyAttest) {
        let issuer = attest.issuer;

        // Alte Kanten des Issuers entfernen
        self.edges.retain(|(a, b), _| *a != issuer && *b != issuer);

        // Neue Kanten einfügen (ungerichtet: beide Richtungen)
        for (peer_bytes, rtt_ms) in &attest.latencies {
            // PeerIdBytes in MinerId konvertieren (vereinfacht: MinerId = PeerId)
            // In der realen Implementierung müsste hier eine Mapping-Tabelle verwendet werden.
            let peer = MinerId::new(peer_bytes.0);

            self.edges.insert((issuer, peer), *rtt_ms);
            self.edges.insert((peer, issuer), *rtt_ms);
            
            // Peers auch in last_update eintragen (mit dem Attest-Zeitstempel)
            self.last_update.entry(peer).or_insert(attest.timestamp_ms);
        }

        // Zeitstempel des Issuers aktualisieren
        self.last_update.insert(issuer, attest.timestamp_ms);
    }

    /// Gibt die Latenz zwischen zwei Nodes zurück (falls bekannt).
    pub fn get_latency(&self, a: &MinerId, b: &MinerId) -> Option<u32> {
        self.edges.get(&(*a, *b)).copied()
    }

    /// Gibt alle bekannten Latenzen eines Nodes zurück.
    pub fn get_all_latencies(&self, node: &MinerId) -> Vec<(MinerId, u32)> {
        self.edges
            .iter()
            .filter(|((a, _), _)| a == node)
            .map(|((_, b), rtt)| (*b, *rtt))
            .collect()
    }

    /// Gibt die Anzahl der Kanten im Graphen zurück.
    pub fn edge_count(&self) -> usize {
        self.edges.len() / 2 // Ungerichteter Graph: jede Kante twice
    }

    /// Gibt die Anzahl der bekannten Nodes zurück.
    pub fn node_count(&self) -> usize {
        self.last_update.len()
    }

    /// Entfernt veraltete Einträge (älter als `max_age_ms`).
    ///
    /// Sollte regelmäßig aufgerufen werden (z. B. alle 10 min), um
    /// Speicherlecks durch Nodes zu vermeiden, die das Netzwerk verlassen.
    pub fn cleanup_stale_entries(&mut self, max_age_ms: u64) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64;

        // Veraltete Nodes finden
        let stale_nodes: Vec<MinerId> = self
            .last_update
            .iter()
            .filter(|(_, &timestamp)| now_ms - timestamp > max_age_ms)
            .map(|(node, _)| *node)
            .collect();

        // Entfernen
        for node in stale_nodes {
            self.last_update.remove(&node);
            self.edges.retain(|(a, b), _| *a != node && *b != node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_attest() -> LatencyAttest {
        LatencyAttest {
            issuer: MinerId::new([1u8; 32]),
            timestamp_ms: 1000,
            latencies: vec![
                (PeerIdBytes([2u8; 32]), 50),
                (PeerIdBytes([3u8; 32]), 100),
            ],
            signature: BlsSignatureBytes([0u8; 96]),
        }
    }

    #[test]
    fn attest_hash_deterministic() {
        let attest1 = test_attest();
        let attest2 = test_attest();

        assert_eq!(attest1.hash(), attest2.hash());
    }

    #[test]
    fn attest_validate_structure_ok() {
        let attest = test_attest();
        assert!(attest.validate_structure().is_ok());
    }

    #[test]
    fn attest_validate_structure_future_timestamp() {
        let mut attest = test_attest();
        attest.timestamp_ms = u64::MAX; // Weit in der Zukunft

        assert!(matches!(
            attest.validate_structure(),
            Err(LatencyAttestError::FutureTimestamp { .. })
        ));
    }

    #[test]
    fn attest_validate_structure_implausible_latency() {
        let mut attest = test_attest();
        attest.latencies.push((PeerIdBytes([4u8; 32]), 20_000)); // > 10.000 ms

        assert!(matches!(
            attest.validate_structure(),
            Err(LatencyAttestError::ImplausibleLatency { .. })
        ));
    }

    #[test]
    fn attest_validate_structure_duplicate_peer() {
        let mut attest = test_attest();
        attest.latencies.push((PeerIdBytes([2u8; 32]), 60)); // Duplikat

        assert!(matches!(
            attest.validate_structure(),
            Err(LatencyAttestError::DuplicatePeer { .. })
        ));
    }

    #[test]
    fn graph_insert_attest() {
        let mut graph = LatencyGraph::new();
        let attest = test_attest();

        graph.insert_attest(&attest);

        assert_eq!(graph.node_count(), 3); // Issuer + 2 Peers
        assert_eq!(graph.edge_count(), 2); // 2 Kanten (ungerichtet)
    }

    #[test]
    fn graph_get_latency() {
        let mut graph = LatencyGraph::new();
        let attest = test_attest();
        let issuer = attest.issuer;
        let peer = MinerId::new([2u8; 32]);

        graph.insert_attest(&attest);

        assert_eq!(graph.get_latency(&issuer, &peer), Some(50));
        assert_eq!(graph.get_latency(&peer, &issuer), Some(50)); // Symmetrisch
    }

    #[test]
    fn graph_cleanup_stale_entries() {
        let mut graph = LatencyGraph::new();
        let mut attest = test_attest();
        
        // Setze Zeitstempel auf 2 Stunden in der Vergangenheit
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64;
        attest.timestamp_ms = now_ms - 2 * 3600 * 1000; // 2 Stunden alt

        graph.insert_attest(&attest);
        assert_eq!(graph.node_count(), 3);

        // Cleanup mit 1 Stunde max_age
        graph.cleanup_stale_entries(3600 * 1000); // 1 Stunde
        assert_eq!(graph.node_count(), 0);
    }
}

#[cfg(test)]
mod a10_tests {
    use super::*;
    use crate::bls::BlsSecretKey;
    use crate::ids::MinerId;

    fn schluessel(saat: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[saat; 32]).expect("KeyGen")
    }

    fn attest(saat: u8) -> LatencyAttest {
        LatencyAttest {
            issuer: MinerId::new([saat; 32]),
            timestamp_ms: 1_700_000_000_000,
            latencies: vec![(PeerIdBytes([9u8; 32]), 42), (PeerIdBytes([7u8; 32]), 17)],
            signature: BlsSignatureBytes([0u8; 96]),
        }
    }

    /// **A10: Ein signiertes Attest wird als echt erkannt.**
    #[test]
    fn ein_signiertes_attest_ueberlebt_die_pruefung() {
        let sk = schluessel(3);
        let pk = sk.public_key().expect("PubKey");
        let mut a = attest(3);
        a.sign(&sk).expect("Signieren");
        assert!(a.verify(&pk), "die eigene Signatur wurde nicht anerkannt");
    }

    /// **A10: Ein unsigniertes Attest fällt durch.**
    ///
    /// Der Zustand, in dem das Feld seit dem ersten Entwurf war: gesetzt
    /// auf Nullbytes, von niemandem geprüft. **Ein ungeprüftes
    /// Signaturfeld ist gefährlicher als gar keines.**
    #[test]
    fn ein_unsigniertes_attest_faellt_durch() {
        let pk = schluessel(3).public_key().expect("PubKey");
        assert!(!attest(3).verify(&pk), "Nullbytes galten als Signatur");
    }

    /// **A10: Ein fremder Schlüssel passt nicht.**
    ///
    /// Der eigentliche Angriff: Wer die Latenzwerte frei setzen kann,
    /// sucht sich seine Pod-Nachbarn aus.
    #[test]
    fn ein_fremder_schluessel_passt_nicht() {
        let sk = schluessel(3);
        let fremd = schluessel(4).public_key().expect("PubKey");
        let mut a = attest(3);
        a.sign(&sk).expect("Signieren");
        assert!(!a.verify(&fremd), "ein fremder Schlüssel wurde anerkannt");
    }

    /// **A10: Veränderte Latenzwerte machen die Signatur ungültig.**
    #[test]
    fn veraenderte_latenzen_brechen_die_signatur() {
        let sk = schluessel(5);
        let pk = sk.public_key().expect("PubKey");
        let mut a = attest(5);
        a.sign(&sk).expect("Signieren");
        assert!(a.verify(&pk));

        a.latencies[0].1 = 1; // „ich bin ganz nah dran"
        assert!(!a.verify(&pk), "veränderte Latenz blieb unbemerkt");
    }

    #[test]
    fn ein_veraenderter_aussteller_bricht_die_signatur() {
        let sk = schluessel(6);
        let pk = sk.public_key().expect("PubKey");
        let mut a = attest(6);
        a.sign(&sk).expect("Signieren");
        a.issuer = MinerId::new([99u8; 32]);
        assert!(!a.verify(&pk), "ein fremder Aussteller blieb unbemerkt");
    }

    #[test]
    fn die_reihenfolge_der_latenzen_aendert_die_signatur_nicht() {
        // `signable_bytes` sortiert. Ohne das hinge die Gültigkeit an
        // der Reihenfolge, in der ein Knoten seine Peers durchläuft, und
        // die ist nicht garantiert.
        let sk = schluessel(7);
        let pk = sk.public_key().expect("PubKey");
        let mut a = attest(7);
        a.sign(&sk).expect("Signieren");
        a.latencies.reverse();
        assert!(a.verify(&pk), "die Sortierung in signable_bytes greift nicht");
    }

    #[test]
    fn unsinnige_signaturbytes_stuerzen_nicht_ab() {
        let pk = schluessel(8).public_key().expect("PubKey");
        let mut a = attest(8);
        a.signature = BlsSignatureBytes([0xFF; 96]);
        assert!(!a.verify(&pk));
    }
}
