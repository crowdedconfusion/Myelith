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
        Self::nur_fuer_den_eigentuemer(path);
        Ok(())
    }

    /// Setzt die Dateirechte auf 0600, also nur lesbar für den
    /// Eigentümer.
    ///
    /// **Der Schlüssel ist die Identität des Knotens.** Wer ihn liest,
    /// kann im Netz als dieser Knoten auftreten. Auf einem gemeinsam
    /// genutzten Rechner ist die Vorgabe der meisten Systeme, 0644, für
    /// diese Datei zu weit.
    ///
    /// **Ein Fehlschlag bleibt folgenlos**, und das ist Absicht: Auf
    /// Windows gibt es diese Rechte nicht, und ein Knoten, der deswegen
    /// nicht startet, hat aus einer fehlenden Härtung einen Ausfall
    /// gemacht.
    fn nur_fuer_den_eigentuemer(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        #[cfg(not(unix))]
        {
            let _ = path;
        }
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

/// Die Netzadresse eines Knotens als 32 Bytes: sein **öffentlicher
/// Ed25519-Schlüssel**.
///
/// # ⚑ Fund 117: Eine `PeerId` passt nicht in 32 Bytes
///
/// `myl_types::latency_attest::PeerIdBytes` trägt seit je den Kommentar
/// „PeerId als 32-Byte-Array … Die Konvertierung erfolgt in NETWORKING".
/// **Die Konvertierung gab es nicht**, und sie hätte so auch nicht
/// gehen können: Eine `PeerId` ist ein Multihash und misst für Ed25519
/// **38 Bytes**, nicht 32.
///
/// Aufgefallen ist das erst, als die Adresse mit Punkt 46 in die
/// Registrierung kam und jemand sie benutzen wollte. Vorher trug der Typ
/// nur Latenzatteste, in denen niemand zurückrechnete; **ein Feld, das
/// keiner liest, kann jede Bedeutung tragen.**
///
/// **Die 32 Bytes sind deshalb der öffentliche Schlüssel**, und die
/// `PeerId` folgt daraus. Das ist keine Notlösung, sondern die
/// ursprüngliche Größe: Die `PeerId` **ist** der Hash dieses Schlüssels,
/// also trägt der Schlüssel mehr und nicht weniger.
pub fn peer_id_aus_bytes(
    b: &myl_types::latency_attest::PeerIdBytes,
) -> Result<PeerId, libp2p::identity::DecodingError> {
    let pk = libp2p::identity::ed25519::PublicKey::try_from_bytes(&b.0)?;
    Ok(PeerId::from(libp2p::identity::PublicKey::from(pk)))
}

/// Die eigene Netzadresse, so wie sie in eine Anmeldung gehört.
///
/// ⚑ **Nicht die `PeerId`, sondern der Schlüssel**, aus dem sie folgt.
/// Wer die `PeerId` einträgt, trägt einen Hash ein, und aus einem Hash
/// lässt sich nichts wiederherstellen.
pub fn netzadresse(id: &NodeIdentity) -> Option<myl_types::latency_attest::PeerIdBytes> {
    id.keypair()
        .public()
        .try_into_ed25519()
        .ok()
        .map(|pk| myl_types::latency_attest::PeerIdBytes(pk.to_bytes()))
}

#[cfg(test)]
mod adresstests {
    use super::*;

    /// ⚑ **Der Weg muss hin und zurück gehen**, sonst ist eine Adresse
    /// in der Kette keine Adresse, sondern eine Zahl.
    #[test]
    fn adresse_und_peer_id_gehoeren_zusammen() {
        let id = NodeIdentity::generate();
        let a = netzadresse(&id).expect("Ed25519");
        assert_eq!(peer_id_aus_bytes(&a).expect("zurueck"), id.peer_id());
    }

    /// Zwei Knoten haben zwei Adressen.
    #[test]
    fn zwei_knoten_zwei_adressen() {
        let a = netzadresse(&NodeIdentity::generate()).expect("Ed25519");
        let b = netzadresse(&NodeIdentity::generate()).expect("Ed25519");
        assert_ne!(a, b);
    }

    /// ⚑ **Jede Folge von 32 Bytes ergibt eine `PeerId`, und das ist
    /// kein Versehen der Bibliothek.**
    ///
    /// ⛑ Hier stand zuerst die umgekehrte Behauptung: Unsinn solle einen
    /// Fehler ergeben. Der Test fiel um, und **er hatte unrecht, nicht
    /// der Code**. Ein Ed25519-Punkt wird erst beim Rechnen geprüft,
    /// nicht beim Einlesen.
    ///
    /// **Für die Registrierung heißt das:** Eine falsche Netzadresse ist
    /// beim Eintragen **nicht** erkennbar. Das passt zur Entscheidung zu
    /// Punkt 46 und macht sie schärfer: Die Adresse ist eine Angabe, und
    /// **das einzige Signal ist die ausbleibende Antwort**. Wer eine
    /// Prüfung beim Eintragen erwartet, verlässt sich auf etwas, das es
    /// nicht gibt.
    #[test]
    fn jede_bytefolge_ergibt_eine_peer_id() {
        let irgendwas = myl_types::latency_attest::PeerIdBytes([1; 32]);
        let p = peer_id_aus_bytes(&irgendwas).expect("32 Bytes genuegen");
        // Und sie ist eine andere als die eines echten Knotens.
        assert_ne!(p, NodeIdentity::generate().peer_id());
    }
}
