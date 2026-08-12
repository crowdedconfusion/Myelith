//! Adress- und ID-Typen als unterscheidbare Newtypes (Punkt 1.6).
//!
//! Protokoll-Identifikatoren sind als eigene Typen definiert, damit eine
//! Verwechslung (z. B. `MinerId` an eine Stelle, die `PodId` erwartet)
//! ein Compile-Fehler ist statt eines stillen Logik-Fehlers. Alle
//! 32-Byte-IDs sind Borsh-serialisierbar und haben eine kanonische
//! Hex-Darstellung für Anzeigen, Logs und Golden-Vektor-Dateien.
//!
//! Festlegungen (Konsens-Vertrag, nur über Governance änderbar):
//! - Alle hash-abgeleiteten IDs sind 32 Bytes (Protokoll-Hash SHA-256).
//! - **Adressen sind hash-basiert:** Konvention ist
//!   `Address = SHA-256(komprimierter BLS-Public-Key)`. Die Ableitung
//!   ist damit quantensicher, solange der Hash hält (Quantum-Hardening-
//!   Vorgabe), und das Format ist unabhängig vom Signaturschema —
//!   ein späterer Signatur-Tausch (Krypto-Agilität) ändert keine
//!   Adressen.
//! - `EpochId` ist ein fortlaufender Zähler (u64), keine Hash-ID.

use borsh::{BorshDeserialize, BorshSerialize};
use std::fmt;

/// Länge aller hash-abgeleiteten IDs in Bytes.
pub const ID_LEN: usize = 32;

/// Fehler beim Parsen einer ID-Hex-Darstellung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdParseError {
    /// Die Eingabe hat nicht exakt 64 Hex-Zeichen.
    WrongLength { got: usize },
    /// Mindestens ein Zeichen ist keine Hex-Ziffer.
    InvalidHex,
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { got } => {
                write!(f, "ID-Hex-Eingabe hat {} statt 64 Zeichen", got)
            }
            Self::InvalidHex => write!(f, "ID-Eingabe enthält Nicht-Hex-Zeichen"),
        }
    }
}

impl std::error::Error for IdParseError {}

fn parse_id_hex(s: &str) -> Result<[u8; ID_LEN], IdParseError> {
    if s.len() != ID_LEN * 2 {
        return Err(IdParseError::WrongLength { got: s.len() });
    }
    let mut out = [0u8; ID_LEN];
    for i in 0..ID_LEN {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|_| IdParseError::InvalidHex)?;
    }
    Ok(out)
}

/// Erzeugt einen 32-Byte-ID-Newtype mit Borsh, Hex-Darstellung und
/// üblichen Trait-Implementierungen.
macro_rules! define_id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
        pub struct $name([u8; ID_LEN]);

        impl $name {
            /// Aus rohen 32 Bytes.
            pub fn new(bytes: [u8; ID_LEN]) -> Self {
                Self(bytes)
            }

            /// Die rohen 32 Bytes.
            pub fn as_bytes(&self) -> &[u8; ID_LEN] {
                &self.0
            }

            /// Kanonische Hex-Darstellung (klein, ohne Präfix).
            pub fn to_hex(&self) -> String {
                self.0.iter().map(|b| format!("{:02x}", b)).collect()
            }

            /// Aus Hex-Darstellung; Groß-/Kleinschreibung ist erlaubt,
            /// jede andere Eingabe ist ein Fehler.
            pub fn from_hex(s: &str) -> Result<Self, IdParseError> {
                parse_id_hex(s).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.to_hex())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.to_hex())
            }
        }

        impl std::str::FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_hex(s)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }
    };
}

define_id_type!(
    /// Konto-Adresse (hash-basiert, 32 Bytes). Konvention:
    /// `SHA-256(komprimierter BLS-Public-Key)`.
    Address
);

define_id_type!(
    /// Identität eines Shard-Miners (z. B. abgeleitet aus seinem
    /// Registrierungsschlüssel). Erscheint in `Segment.pod_path`.
    MinerId
);

define_id_type!(
    /// Identität eines Pods (Redundanz-Einheit aus k+2 Minern, je Epoche
    /// vom Scheduler gebildet).
    PodId
);

define_id_type!(
    /// Identität eines Inferenz-Segments: `h(session ‖ index)`
    /// (Whitepaper Anhang A.1).
    SegmentId
);

define_id_type!(
    /// Merkle-Wurzel (θ_v-Gewichtswurzel, Segment-Id-Wurzeln,
    /// Korpus-Provenienz). Eigener Typ, damit Wurzeln nicht mit
    /// gewöhnlichen Hashes oder IDs verwechselt werden.
    MerkleRoot
);

define_id_type!(
    /// Hash eines Aktivierungs-Tensors in der Berechnungsspur
    /// (`Segment.trace`: h(a_0), …, h(a_k)).
    ActivationHash
);

/// Epochen-Identität: fortlaufender Zähler (kein Hash).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct EpochId(pub u64);

impl fmt::Display for EpochId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    #[test]
    fn hex_rundtrip_alle_typen() {
        let bytes: [u8; ID_LEN] = std::array::from_fn(|i| (i * 7 + 3) as u8);
        let roundtrip = |id: Address| {
            let hex = id.to_hex();
            assert_eq!(hex.len(), 64);
            assert_eq!(Address::from_hex(&hex).expect("eigene Hex-Ausgabe"), id);
            assert_eq!(
                Address::from_hex(&hex.to_uppercase()).expect("Groß-Hex"),
                id
            );
        };
        roundtrip(Address::new(bytes));
        assert_eq!(MinerId::new(bytes).to_hex(), Address::new(bytes).to_hex());
        assert_eq!(PodId::from_hex(&PodId::new(bytes).to_hex()).expect("ok"), PodId::new(bytes));
        assert_eq!(
            SegmentId::from_hex(&SegmentId::new(bytes).to_hex()).expect("ok"),
            SegmentId::new(bytes)
        );
        assert_eq!(
            MerkleRoot::from_hex(&MerkleRoot::new(bytes).to_hex()).expect("ok"),
            MerkleRoot::new(bytes)
        );
        assert_eq!(
            ActivationHash::from_hex(&ActivationHash::new(bytes).to_hex()).expect("ok"),
            ActivationHash::new(bytes)
        );
    }

    #[test]
    fn hex_fehlerfaelle() {
        assert_eq!(
            MinerId::from_hex(""),
            Err(IdParseError::WrongLength { got: 0 })
        );
        assert_eq!(
            MinerId::from_hex("abc"),
            Err(IdParseError::WrongLength { got: 3 })
        );
        let bad = "z".repeat(64);
        assert_eq!(MinerId::from_hex(&bad), Err(IdParseError::InvalidHex));
    }

    #[test]
    fn epoch_id_anzeige_und_ordnung() {
        assert_eq!(EpochId(42).to_string(), "42");
        assert!(EpochId(1) < EpochId(2));
    }

    #[test]
    fn borsh_rundtrip_alle_typen() {
        let bytes: [u8; ID_LEN] = [0xa5; ID_LEN];
        let address = Address::new(bytes);
        let back: Address = from_slice(&to_vec(&address).expect("ser")).expect("de");
        assert_eq!(back, address);
        let epoch = EpochId(1_000_000);
        let back: EpochId = from_slice(&to_vec(&epoch).expect("ser")).expect("de");
        assert_eq!(back, epoch);
    }

    #[test]
    fn typen_sind_unverwechselbar_gross() {
        // Alle Newtypes haben dieselbe Byte-Länge, aber unterschiedliche
        // Typ-Identität: Dieser Test dokumentiert die Absicht; die
        // eigentliche Garantie ist der Compile-Fehler bei Verwechslung.
        let bytes = [0x11u8; ID_LEN];
        assert_eq!(Address::new(bytes).as_bytes(), MinerId::new(bytes).as_bytes());
        assert_eq!(Address::new(bytes).as_bytes().len(), ID_LEN);
    }
}
