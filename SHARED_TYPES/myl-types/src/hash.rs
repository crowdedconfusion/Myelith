//! `Hash` — der 32-Byte-Protokoll-Hash (SHA-256).
//!
//! Ein Hash-Typ für das gesamte Protokoll (Design-Entscheidung vom
//! 2026-08-12): Konsensobjekte, Merkle-Knoten, θ_v-/Artefakt-Hashes
//! (INTEGER_LLM) und Adressableitung nutzen alle dieselbe Funktion.
//!
//! Der Gleichheitsvergleich läuft in Konstantzeit (`subtle::ConstantTimeEq`),
//! damit Hash-Vergleiche keine timing-basierte Seitenkanal-Information
//! preisgeben (relevant, sobald Hashes als Selektoren oder Lose dienen).

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use std::fmt;
use subtle::ConstantTimeEq;

/// Länge eines Protokoll-Hashes in Bytes (SHA-256).
pub const HASH_LEN: usize = 32;

/// 32-Byte-Protokoll-Hash (SHA-256-Ausgabe).
///
/// Newtype statt nacktem `[u8; 32]`: verhindert die Verwechslung mit
/// anderen 32-Byte-Werten (Adressen, Wurzeln, Ausgaben) auf Typebene.
/// Die späteren `Address`-/`MerkleRoot`-/ID-Newtypes (Punkt 1.6) bauen
/// auf demselben Muster auf.
#[derive(Clone, Copy, Eq, BorshSerialize, BorshDeserialize)]
pub struct Hash(pub [u8; HASH_LEN]);

impl Hash {
    /// SHA-256 über `data`.
    pub fn sha256(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Die rohen 32 Bytes.
    pub fn as_bytes(&self) -> &[u8; HASH_LEN] {
        &self.0
    }

    /// Aus rohen 32 Bytes (ohne Hash-Berechnung).
    pub fn from_bytes(bytes: [u8; HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// Konstantzeit-Gleichheit (für Vergleiche in sicherheitsrelevanten
    /// Pfaden; `PartialEq` delegiert hierauf).
    pub fn ct_eq(&self, other: &Self) -> subtle::Choice {
        self.0.ct_eq(&other.0)
    }

    /// Kanonische Hex-Darstellung (klein, ohne Präfix) — für Anzeigen,
    /// Logs und Golden-Vektor-Dateien. `FromStr` ist die Umkehrung.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Aus Hex-Darstellung; Groß-/Kleinschreibung ist erlaubt,
    /// jede andere Eingabe ist ein Fehler.
    pub fn from_hex(s: &str) -> Result<Self, HashParseError> {
        if s.len() != HASH_LEN * 2 {
            return Err(HashParseError::WrongLength { got: s.len() });
        }
        let mut out = [0u8; HASH_LEN];
        for i in 0..HASH_LEN {
            out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
                .map_err(|_| HashParseError::InvalidHex)?;
        }
        Ok(Self(out))
    }
}

/// Fehler beim Parsen einer Hash-Hex-Darstellung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashParseError {
    /// Die Eingabe hat nicht exakt 64 Hex-Zeichen.
    WrongLength { got: usize },
    /// Mindestens ein Zeichen ist keine Hex-Ziffer.
    InvalidHex,
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { got } => {
                write!(f, "Hash-Hex-Eingabe hat {} statt 64 Zeichen", got)
            }
            Self::InvalidHex => write!(f, "Hash-Eingabe enthält Nicht-Hex-Zeichen"),
        }
    }
}

impl std::error::Error for HashParseError {}

/// Konstantzeit-Gleichheit: kein früher Abbruch bei der ersten
/// Differenz, damit die Vergleichsdauer nichts über die Lage
/// übereinstimmender Präfixe verrät.
impl PartialEq for Hash {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

/// Ordnung für `BTreeMap` und `BTreeSet`: lexikographisch über die
/// Bytes.
///
/// # ⚑ Fund 74: Ohne sie war eine `BTreeSet<Hash>` nicht befüllbar
///
/// GOVERNANCE führt die Kernel-Whitelist (Kap. 10.3) als
/// `Wert::Hashmenge(BTreeSet<Hash>)`. Der Vorgabewert ist die leere
/// Menge, „bis zum Genesis-Manifest", und **genau bis dahin fiel es
/// nicht auf**: Ein leeres `BTreeSet` braucht kein `Ord`, ein
/// `insert` schon. Der Parameter stand mit Typ, Vorgabewert und
/// Dokumentation da und ließ sich nicht füllen; aufgefallen ist es
/// erst, als das Genesis-Manifest gebaut wurde und der erste Kernel
/// hineinsollte.
///
/// Die ID-Typen in [`crate::ids`] leiten `Ord` seit jeher ab. Dass
/// ausgerechnet `Hash` es nicht tat, war kein Entwurf, sondern eine
/// Lücke.
///
/// # Warum das nicht in Konstantzeit läuft, und warum das richtig ist
///
/// [`PartialEq`] vergleicht bewusst in Konstantzeit. Eine **Ordnung**
/// kann das nicht: Sie bricht beim ersten unterschiedlichen Byte ab,
/// und genau daraus besteht ein Größenvergleich. Dieselbe Abwägung wie
/// bei `std::hash::Hash` darunter, und dieselbe Begründung: Sortieren
/// und Nachschlagen sind keine Geheimnisoperationen. Wer zwei Hashes
/// vergleicht, um zu erfahren, ob sie gleich sind, nimmt `==`.
///
/// **Was zusammenpassen muss:** `cmp` gibt genau dann `Equal` zurück,
/// wenn `eq` wahr ist. Beide vergleichen dieselben Bytes, nur mit
/// verschiedenem Zeitverhalten; ein Test unten hält das fest.
impl Ord for Hash {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `std::hash::Hash` für die Nutzung in HashMap/HashSet — unabhängig
/// vom Konstantzeit-Vergleich (Lookups sind nicht sicherheitsrelevant).
impl std::hash::Hash for Hash {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

impl std::str::FromStr for Hash {
    type Err = HashParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    #[test]
    fn leere_eingabe_bekannter_wert() {
        // Offizieller SHA-256-Testvektor (NIST) für die leere Eingabe.
        let h = Hash::sha256(b"");
        assert_eq!(
            h.to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn abc_bekannter_wert() {
        // Offizieller SHA-256-Testvektor (NIST) für "abc".
        let h = Hash::sha256(b"abc");
        assert_eq!(
            h.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn determinismus() {
        let a = Hash::sha256(b"myelith: determinismus-test");
        let b = Hash::sha256(b"myelith: determinismus-test");
        assert_eq!(a, b);
        assert_eq!(a.to_hex(), b.to_hex());
    }

    #[test]
    fn ungleiche_hashes() {
        let a = Hash::sha256(b"a");
        let b = Hash::sha256(b"b");
        assert_ne!(a, b);
        assert!(!bool::from(a.ct_eq(&b)));
    }

    #[test]
    fn hex_roundtrip() {
        let h = Hash::sha256(b"hex-roundtrip");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        let parsed: Hash = hex.parse().expect("eigene Hex-Ausgabe muss parsen");
        assert_eq!(parsed, h);
        assert_eq!(Hash::from_hex(&hex.to_uppercase()).expect("Groß-Hex"), h);
    }

    #[test]
    fn hex_fehlerfaelle() {
        assert_eq!(
            Hash::from_hex(""),
            Err(HashParseError::WrongLength { got: 0 })
        );
        assert_eq!(
            Hash::from_hex("abc"),
            Err(HashParseError::WrongLength { got: 3 })
        );
        let bad_chars = "z".repeat(64);
        assert_eq!(Hash::from_hex(&bad_chars), Err(HashParseError::InvalidHex));
    }

    #[test]
    fn borsh_roundtrip() {
        let h = Hash::sha256(b"borsh-roundtrip");
        let bytes = to_vec(&h).expect("Serialisierung");
        assert_eq!(bytes.len(), HASH_LEN);
        let back: Hash = from_slice(&bytes).expect("Deserialisierung");
        assert_eq!(back, h);
    }

    #[test]
    fn from_bytes_und_zurueck() {
        let mut raw = [0u8; HASH_LEN];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = i as u8;
        }
        let h = Hash::from_bytes(raw);
        assert_eq!(h.as_bytes(), &raw);
        assert_eq!(h.to_hex(), Hash::from_bytes(raw).to_hex());
    }
}

#[cfg(test)]
mod ordnung {
    use super::*;

    #[test]
    fn die_ordnung_stimmt_mit_der_gleichheit_ueberein() {
        // `cmp == Equal` genau dann, wenn `eq`. Liefen die beiden
        // auseinander, verhielte sich jede `BTreeMap` mit Hash-Schlüssel
        // undefiniert: Sie fände Einträge nicht, die sie enthält.
        let a = Hash::sha256(b"a");
        let b = Hash::sha256(b"b");
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
        assert!(a == a);
        assert_ne!(a.cmp(&b), std::cmp::Ordering::Equal);
        assert!(a != b);
    }

    #[test]
    fn die_ordnung_ist_lexikographisch_ueber_die_bytes() {
        let klein = Hash([0u8; HASH_LEN]);
        let mut mittel_roh = [0u8; HASH_LEN];
        mittel_roh[0] = 1;
        let mittel = Hash(mittel_roh);
        let gross = Hash([255u8; HASH_LEN]);
        assert!(klein < mittel);
        assert!(mittel < gross);
    }

    #[test]
    fn eine_hashmenge_laesst_sich_befuellen() {
        // ⚑ Fund 74, als Test: Genau das ging vorher nicht, und der
        // Fehler wäre erst beim Übersetzen der ersten Aufrufstelle
        // aufgefallen.
        let mut menge = std::collections::BTreeSet::new();
        menge.insert(Hash::sha256(b"linear_w8a16"));
        menge.insert(Hash::sha256(b"silu_lut"));
        menge.insert(Hash::sha256(b"linear_w8a16"));
        assert_eq!(menge.len(), 2, "die Menge entdoppelt nicht");
        assert!(menge.contains(&Hash::sha256(b"silu_lut")));
    }

    #[test]
    fn die_reihenfolge_ist_ueber_laeufe_stabil() {
        // Der Grund, warum es eine `BTreeSet` und keine `HashSet` ist:
        // Eine Menge, deren Reihenfolge sich zwischen Läufen ändert,
        // ergibt verschiedene Wurzeln für denselben Inhalt.
        let bauen = || {
            let mut m = std::collections::BTreeSet::new();
            for wort in [b"c".as_slice(), b"a", b"b"] {
                m.insert(Hash::sha256(wort));
            }
            m.into_iter().collect::<Vec<_>>()
        };
        assert_eq!(bauen(), bauen());
    }
}
