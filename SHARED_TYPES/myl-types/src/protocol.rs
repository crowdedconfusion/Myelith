//! Protokoll-Konstanten: die am 2026-08-12 getroffenen Design-Entscheidungen
//! als maschinenlesbare Anker (analog zur zentralen Pfadkonstante in
//! INTEGER_LLM `runtime/src/paths.rs`).
//!
//! Änderungen an diesen Festlegungen sind konsensrelevant und nach
//! Whitepaper Kap. 10.3 nur über den Governance-Prozess möglich
//! (für die Post-Quantum-Migration siehe GOVERNANCE,
//! Design-Entscheidung 4: Krypto-Agilität).

/// Hash-Algorithmus des gesamten Protokolls.
/// SHA-256: konsistent mit den θ_v-/Artefakt-Hashes in INTEGER_LLM,
/// maximal etabliert und auditiert, hardwarebeschleunigt (SHA-NI,
/// ARMv8-Crypto-Extensions). Grover-resistent (~128 bit Post-Quantum).
pub const PROTOCOL_HASH_ALGORITHM: &str = "SHA-256";

/// Signaturschema (Anhang A.1: `BlsSignature`, Aggregation über
/// Pod-Mitglieder via `aggregate_sig` in `PoIBundle`).
/// Shor-anfällig — dokumentierter Migrationspunkt (Kandidaten:
/// ML-DSA/Dilithium, SPHINCS+, Hybrid).
pub const PROTOCOL_SIGNATURE_ALGORITHM: &str = "BLS12-381";

/// VRF-Konstruktion: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381, §5.5) —
/// die standardisierte ECVRF-Suite für Edwards25519 (Suite-String 0x03,
/// SHA-512, cLen 16, Cofactor 8, deterministische Nonce nach RFC 8032).
/// Shor-anfällig — dokumentierter Migrationspfad über das
/// Algorithms-Versionsfeld in `VrfOutput`.
pub const PROTOCOL_VRF_ALGORITHM: &str = "ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381)";

/// Kanonische Serialisierung aller konsensrelevanten Strukturen.
/// Borsh ist deterministisch ohne Encoding-Spielraum — Voraussetzung
/// dafür, dass Hashes über serialisierte Strukturen bitstabil sind
/// (Bitgleichheit, Whitepaper Kap. 6.2).
pub const PROTOCOL_SERIALIZATION: &str = "Borsh";

/// VRF-Algorithms-Versionen für das `VrfOutput`-Versionsfeld.
/// Der spätere Post-Quantum-Tausch (Kandidat: deterministisches
/// ML-DSA-basiertes Signatur-Hash-VRF, FIPS 204 §5.2) bekommt eine
/// neue Versionsnummer, ohne die Blockstruktur zu brechen.
pub const VRF_ALGO_ECVRF_CURVE25519: u8 = 0;

/// Signatur-Algorithms-Versionen für zukünftige Signaturtypen.
/// BLS12-381 ist die Ausgangsversion; PQ-Nachfolger bekommen neue
/// Versionsnummern.
pub const SIG_ALGO_BLS12_381: u8 = 0;

/// Größengrenze einer einzelnen Nachricht auf der Leitung, in Bytes.
///
/// Gilt für Anfrage **und** Antwort des Anfragekanals und ist mit der
/// Gossip-Grenze gleichgezogen: Was über Gossip passt, muss auch über
/// eine Nachfrage passen, sonst wäre eine Nachricht verbreitbar, aber
/// nicht nachforderbar.
///
/// ⚑ **Steht seit dem 2026-09-03 hier und nicht mehr in `myl-net`**
/// (Fund 155). Drei Kisten leiten Grenzen daraus ab: der Transport
/// (`myl_net::anfrage`), der vertrauliche Kanal (`myl_siegel`, der vom
/// Budget Kopf, Tag und Längenpräfix abzieht) und der Auftragstyp
/// (`crate::inferenzauftrag`). **Der Sitzungskanal musste dafür bis
/// dahin libp2p mitbauen**, und das war der Pfeil, der den falschen
/// Kistenschnitt sichtbar gemacht hat.
///
/// Ohne diese Grenze liesse sich ein Knoten mit einer einzigen Anfrage
/// zum Senden beliebiger Datenmengen bewegen: wenig Aufwand beim
/// Angreifer, viel beim Opfer.
pub const MAX_ANFRAGE_BYTES: usize = 4 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn konstanten_sind_fest() {
        assert_eq!(PROTOCOL_HASH_ALGORITHM, "SHA-256");
        assert_eq!(PROTOCOL_SIGNATURE_ALGORITHM, "BLS12-381");
        assert_eq!(PROTOCOL_VRF_ALGORITHM, "ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381)");
        assert_eq!(PROTOCOL_SERIALIZATION, "Borsh");
        assert_eq!(VRF_ALGO_ECVRF_CURVE25519, 0);
        assert_eq!(SIG_ALGO_BLS12_381, 0);
    }
}
