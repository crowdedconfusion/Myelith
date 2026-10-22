//! Latenz-Atteste (Whitepaper Kap. 4.1, Anhang A.2).
//!
//! Jeder Node signiert regelmäßig seine gemessenen Latenzen zu anderen
//! Peers und verbreitet diese Atteste über Gossip.
//!
//! ⚑ **Sie bestimmen die Pod-Bildung nicht.** Pods entstehen je Zone aus
//! dem Konsenszustand: Wer wählt, mit wem er attestiert und welche Zahl er
//! nennt, formte sonst mit, in welchem Topf er gemischt wird. Die Atteste
//! bleiben eine Messung des Netzes. Hier stand bis zum 2026-10-21 auch ein
//! `LatencyGraph`, den niemand aufbaute; er setzte eine Netzadresse ohne
//! Umrechnung als Miner-Kennung ein und las die Systemuhr. Er ist entfernt.
//!
//! **Konsens-Feld:** Das Attest-Format ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Atteste werden alle 5 Minuten erstellt und über das
//! `/myelith/latency-attests/1` Topic verbreitet. Jeder Node akzeptiert
//! nur Atteste von Peers, die er direkt kennt (keine Weiterverbreitung
//! von Attesten Dritter — verhindert Sybil-Angriffe).

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::Hash;
use crate::ids::MinerId;

/// Ein signiertes Latenz-Attest eines Nodes.
///
/// Enthält die geglätteten Latenzen zu allen bekannten Peers, signiert
/// mit dem BLS-Schlüssel des Nodes. Andere Nodes können die Signatur
/// verifizieren.
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

/// Die Netzadresse eines Knotens: sein **öffentlicher
/// Ed25519-Schlüssel**, 32 Bytes.
///
/// # ⚑ Fund 117: Der Name sagte etwas, das nicht hineinpasst
///
/// Hier stand bis zum 2026-09-01 „PeerId als 32-Byte-Array … Die
/// Konvertierung erfolgt in NETWORKING". **Die Konvertierung gab es
/// nicht**, und sie hätte so nicht gehen können: Eine `PeerId` ist ein
/// Multihash und misst für Ed25519 **38 Bytes**.
///
/// Aufgefallen ist es erst, als die Adresse mit Punkt 46 in die
/// Registrierung kam und jemand sie benutzen wollte. Vorher trug der Typ
/// nur Latenzatteste, in denen niemand zurückrechnete: ⚑ **Ein Feld, das
/// keiner liest, kann jede Bedeutung tragen.**
///
/// **Die 32 Bytes sind der öffentliche Schlüssel**, und die `PeerId`
/// folgt daraus, denn sie ist sein Hash. Der Schlüssel trägt also mehr
/// und nicht weniger. Die Umwandlung steht in NETWORKING, dort, wo
/// libp2p schon liegt; der Name bleibt, weil er im Konsensvertrag steht.
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
