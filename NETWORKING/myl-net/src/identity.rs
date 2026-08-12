//! Node-Identität: Schlüsselpaar und PeerId.
//!
//! Die Identität ist ein Ed25519-Schlüsselpaar (libp2p-Standard): Der
//! öffentliche Schlüssel bestimmt die `PeerId` (Multihash daraus) und
//! signiert Gossip-Nachrichten sowie später die Latenz-Atteste
//! (Punkt 2.2). Der private Schlüssel bleibt lokal auf dem Node.
//!
//! Quantum-Vermerk: Ed25519-Identitäten sind Shor-anfällig — derselbe
//! Migrationshorizont wie BLS12-381 und ECVRF im Protokoll. Die
//! PeerId-Ableitung ist hash-basiert (Multihash über den öffentlichen
//! Schlüssel) und bleibt damit auch nach einem Signatur-Tausch
//! wohldefiniert; der Wechsel selbst ist ein dokumentierter
//! Governance-Punkt (Krypto-Agilität).

use std::fs;
use std::io;
use std::path::Path;

use libp2p::identity::Keypair;
use libp2p::PeerId;

/// Fehler der Identitäts-Verwaltung.
#[derive(Debug)]
pub enum IdentityError {
    /// Die Schlüssel-Datei konnte nicht gelesen/geschrieben werden.
    Io(io::Error),
    /// Die gespeicherte Kodierung ist kein gültiger Schlüssel.
    InvalidEncoding,
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Identitäts-Datei: {}", e),
            Self::InvalidEncoding => write!(f, "Identitäts-Datei ist kein gültiger Schlüssel"),
        }
    }
}

impl std::error::Error for IdentityError {}

impl From<io::Error> for IdentityError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Identität eines Nodes: Schlüsselpaar plus abgeleitete PeerId.
#[derive(Clone)]
pub struct NodeIdentity {
    keypair: Keypair,
    peer_id: PeerId,
}

impl NodeIdentity {
    /// Erzeugt eine frische Ed25519-Identität.
    pub fn generate() -> Self {
        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        Self { keypair, peer_id }
    }

    /// Lädt eine Identität aus einer Datei (Protobuf-Kodierung) oder
    /// erzeugt eine neue und speichert sie dort — das übliche
    /// Erststart-Verhalten eines Nodes.
    pub fn load_or_create(path: &Path) -> Result<Self, IdentityError> {
        if path.exists() {
            return Self::load(path);
        }
        let identity = Self::generate();
        identity.save(path)?;
        Ok(identity)
    }

    /// Lädt eine Identität aus einer Datei (Protobuf-Kodierung).
    pub fn load(path: &Path) -> Result<Self, IdentityError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Speichert die Identität (nur den privaten Schlüssel — der
    /// öffentliche ist daraus ableitbar) in eine Datei.
    pub fn save(&self, path: &Path) -> Result<(), IdentityError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_bytes().map_err(|_| IdentityError::InvalidEncoding)?)?;
        Ok(())
    }

    /// Protobuf-Kodierung des Schlüsselpaars.
    pub fn to_bytes(&self) -> Result<Vec<u8>, IdentityError> {
        self.keypair
            .clone()
            .to_protobuf_encoding()
            .map_err(|_| IdentityError::InvalidEncoding)
    }

    /// Aus Protobuf-Kodierung.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityError> {
        let keypair =
            Keypair::from_protobuf_encoding(bytes).map_err(|_| IdentityError::InvalidEncoding)?;
        let peer_id = PeerId::from(keypair.public());
        Ok(Self { keypair, peer_id })
    }

    /// Die PeerId dieses Nodes.
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Referenz auf das Schlüsselpaar (für Swarm-Aufbau und Signaturen).
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "myl-net-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Zeit")
                .as_nanos()
        ));
        p
    }

    #[test]
    fn identitaet_ist_stabil() {
        let a = NodeIdentity::generate();
        assert_eq!(a.peer_id(), PeerId::from(a.keypair().public()));
    }

    #[test]
    fn byte_rundtrip() {
        let a = NodeIdentity::generate();
        let bytes = a.to_bytes().expect("Kodierung");
        let b = NodeIdentity::from_bytes(&bytes).expect("Dekodierung");
        assert_eq!(a.peer_id(), b.peer_id());
    }

    #[test]
    fn datei_rundtrip() {
        let path = temp_path("identity");
        let a = NodeIdentity::generate();
        a.save(&path).expect("Speichern");
        let b = NodeIdentity::load(&path).expect("Laden");
        assert_eq!(a.peer_id(), b.peer_id());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn load_or_create_erzeugt_und_wiederverwendet() {
        let path = temp_path("load-or-create");
        let first = NodeIdentity::load_or_create(&path).expect("Erststart");
        let second = NodeIdentity::load_or_create(&path).expect("Zweitstart");
        assert_eq!(first.peer_id(), second.peer_id());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn ungueltige_datei_wird_abgelehnt() {
        let path = temp_path("invalid");
        fs::write(&path, b"kein-schluessel").expect("Schreiben");
        assert!(NodeIdentity::load(&path).is_err());
        fs::remove_file(&path).ok();
    }
}
