//! BLS-Signaturen über BLS12-381 (Whitepaper Anhang A.1, `BlsSignature`).
//!
//! Implementiert die min-pk-Variante der IETF-BLS-Signaturen
//! (draft-irtf-cfrg-bls-signature): Public Keys liegen auf G1
//! (komprimiert 48 Bytes), Signaturen auf G2 (komprimiert 96 Bytes).
//! Das ist die Ethereum-Konsens-Variante; die zugehörige
//! Domain-Separation-Tag (DST) ist unten als Konstante fixiert und Teil
//! des Konsensvertrags (nur über Governance änderbar, Kap. 10.3).
//!
//! Warum min-pk (Public Key G1, Signatur G2)?
//! Öffentliche Schlüssel werden im Protokoll häufig referenziert
//! (Validator-Registry, Block-Köpfe, PoI-Bündel-Header) und sollen klein
//! sein; Signaturen werden ohnehin zu einem Aggregat zusammengefasst
//! (`aggregate_sig` in `PoIBundle`), dort fällt die G2-Größe weniger ins
//! Gewicht. Die Aggregation ist der eigentliche Grund für BLS: N
//! Pod-Mitglieder signieren, on-chain steht eine einzige Signatur —
//! das reduziert das On-Chain-Datenvolumen erheblich gegenüber
//! Einzelsignaturen.
//!
//! Sicherheitsfestlegungen (Konsens-Vertrag):
//! - Alle Verifikationen laufen mit Signatur-Gruppenprüfung
//!   (`sig_groupcheck = true`).
//! - Öffentliche Schlüssel werden vor jeder Aggregat-Verifikation
//!   validiert (Identitäts- und Subgruppen-Prüfung). Das schließt
//!   Identitätspunkte und Punkte außerhalb der Untergruppe aus.
//! - **Gegen Rogue-Key-Angriffe schützt ein Proof-of-Possession**
//!   ([`BlsSecretKey::prove_possession`],
//!   [`BlsPublicKey::verify_possession`]), der bei der Registrierung
//!   eines Schlüssels zu prüfen ist. Ohne ihn ist `FastAggregateVerify`
//!   angreifbar.
//!
//! ## Warum Validierung allein nicht genügt (Fund 27)
//!
//! Bis 2026-08-19 stand hier, die Identitäts- und Subgruppen-Prüfung
//! schütze gegen Rogue-Key-Angriffe auf `FastAggregateVerify`. Das ist
//! **falsch**, und es wurde nicht theoretisch bezweifelt, sondern
//! gebrochen: Zu einem fremden `pk_opfer` lässt sich
//! `pk_rogue = g₁^x · pk_opfer⁻¹` bilden. Dieser Punkt liegt in der
//! richtigen Untergruppe, ist nicht die Identität und besteht damit
//! **beide** Prüfungen. Da `pk_opfer · pk_rogue = g₁^x` gilt, verifiziert
//! eine Signatur, die der Angreifer allein mit `x` erzeugt hat, als
//! Aggregat beider Schlüssel — das Opfer hat nie unterschrieben.
//!
//! Die Prüfungen sind trotzdem richtig und bleiben; sie wehren nur ein
//! anderes Problem ab (Kleine-Untergruppen-Angriffe). Gegen Rogue Keys
//! hilft, dass der Angreifer den diskreten Logarithmus von `pk_rogue`
//! nicht kennt — genau das weist der Proof-of-Possession nach. Die
//! Regression steht in `tests/rogue_key.rs`.
//!
//! Quanten-Einordnung: BLS12-381 ist Shor-anfällig (Discrete-Log) und
//! ein dokumentierter Migrationspunkt; Kandidaten sind ML-DSA/Dilithium,
//! SPHINCS+ oder Hybrid-Varianten (GOVERNANCE,
//! Design-Entscheidung 4: Krypto-Agilität).
//!
//! Die Kurvenarithmetik kommt aus dem `blst`-Crate (die
//! Referenz-Implementierung von Supranational, auch in Ethereum-Konsens
//! im Einsatz). Dieses Modul kapselt `blst` hinter festen Byte-Arrays,
//! damit alle öffentlichen Typen Borsh-serialisierbar und in der Größe
//! konsensstabil sind.

use blst::min_pk::{AggregateSignature, PublicKey, SecretKey, Signature};
use blst::BLST_ERROR;
use borsh::{BorshDeserialize, BorshSerialize};

/// Länge eines komprimierten BLS-Public-Keys (G1).
pub const BLS_PK_LEN: usize = 48;
/// Länge einer komprimierten BLS-Signatur bzw. eines Aggregats (G2).
pub const BLS_SIG_LEN: usize = 96;

/// Domain-Separation-Tag der Signatur-Suite
/// `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_` (min-pk). Teil des
/// Konsensvertrags: Jede Änderung bricht alle bestehenden Signaturen.
pub const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

/// Domain-Separation-Tag der Proof-of-Possession-Suite
/// `BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`
/// (draft-irtf-cfrg-bls-signature §4.2.3).
///
/// Muss sich von [`BLS_DST`] unterscheiden: sonst wäre eine gewöhnliche
/// Signatur über die eigenen Schlüsselbytes ein gültiger Besitznachweis
/// und umgekehrt ein Besitznachweis eine gültige Nachrichtensignatur.
/// Teil des Konsensvertrags.
pub const BLS_POP_DST: &[u8] = b"BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// Fehler der BLS-Operationen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlsError {
    /// Schlüsselgenerierung fehlgeschlagen (z. B. zu kurzes IKM —
    /// KeyGen verlangt mindestens 32 Bytes Eingabematerial).
    KeyGenFailed,
    /// Der geheime Schlüssel ist keine gültige skalare Kodierung.
    InvalidSecretKey,
    /// Der öffentliche Schlüssel dekodiert nicht zu einem gültigen,
    /// von der Identität verschiedenen G1-Punkt in der richtigen
    /// Untergruppe.
    InvalidPublicKey,
    /// Die Signatur dekodiert nicht zu einem gültigen G2-Punkt.
    InvalidSignature,
    /// Aggregation ohne Signaturen ist nicht zulässig.
    EmptyAggregate,
}

impl std::fmt::Display for BlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyGenFailed => write!(f, "BLS: Schlüsselgenerierung fehlgeschlagen"),
            Self::InvalidSecretKey => write!(f, "BLS: ungültiger geheimer Schlüssel"),
            Self::InvalidPublicKey => write!(f, "BLS: ungültiger öffentlicher Schlüssel"),
            Self::InvalidSignature => write!(f, "BLS: ungültige Signatur"),
            Self::EmptyAggregate => write!(f, "BLS: Aggregation ohne Signaturen"),
        }
    }
}

impl std::error::Error for BlsError {}

/// Geheimer BLS-Schlüssel (serialisierter Skalar, 32 Bytes).
///
/// Wird nicht öffentlich serialisiert — bleibt ein geheimer Typ,
/// der nur lokal erzeugt und gehalten wird.
#[derive(Clone)]
pub struct BlsSecretKey([u8; 32]);

/// Öffentlicher BLS-Schlüssel (komprimierter G1-Punkt, 48 Bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlsPublicKey(pub [u8; BLS_PK_LEN]);

/// BLS-Einzelsignatur (komprimierter G2-Punkt, 96 Bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlsSignature(pub [u8; BLS_SIG_LEN]);

/// Besitznachweis für einen öffentlichen Schlüssel (Proof-of-Possession).
///
/// Eigener Typ statt [`BlsSignature`], damit ein Besitznachweis nicht
/// versehentlich als Nachrichtensignatur durchgeht — die beiden werden
/// unter verschiedenen Domain-Tags erzeugt und geprüft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlsProofOfPossession(pub [u8; BLS_SIG_LEN]);

/// BLS-Aggregat-Signatur (ebenfalls komprimierter G2-Punkt, 96 Bytes).
///
/// Eigenständiger Typ, damit eine Einzelsignatur nicht versehentlich
/// als Aggregat (oder umgekehrt) verwendet wird — die Verifikations-
/// routinen sind unterschiedlich.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlsAggregateSignature(pub [u8; BLS_SIG_LEN]);

/// Öffentlichen Schlüssel dekodieren UND validieren (Identitäts- und
/// Subgruppen-Prüfung). Konsens-Knoten müssen fremde öffentliche
/// Schlüssel immer validieren, bevor sie sie verwenden.
///
/// **Nicht** ausreichend gegen Rogue Keys — dafür ist der
/// Proof-of-Possession bei der Registrierung zuständig
/// ([`BlsPublicKey::verify_possession`], Fund 27 im Modulkopf).
fn decode_validated_pk(bytes: &[u8; BLS_PK_LEN]) -> Result<PublicKey, BlsError> {
    let pk = PublicKey::from_bytes(bytes).map_err(|_| BlsError::InvalidPublicKey)?;
    pk.validate().map_err(|_| BlsError::InvalidPublicKey)?;
    Ok(pk)
}

impl BlsSecretKey {
    /// Schlüsselgenerierung nach draft-irtf-cfrg-bls-signature §2.3
    /// (KeyGen): HKDF-basiert, `ikm` muss mindestens 32 Bytes sein.
    pub fn key_gen(ikm: &[u8]) -> Result<Self, BlsError> {
        let sk = SecretKey::key_gen(ikm, &[]).map_err(|_| BlsError::KeyGenFailed)?;
        Ok(Self(sk.serialize()))
    }

    /// Der zugehörige öffentliche Schlüssel.
    pub fn public_key(&self) -> Result<BlsPublicKey, BlsError> {
        let sk = SecretKey::from_bytes(&self.0).map_err(|_| BlsError::InvalidSecretKey)?;
        let pk = sk.sk_to_pk();
        Ok(BlsPublicKey(pk.compress()))
    }

    /// Eine Nachricht signieren (deterministisch, keine Zufallsdaten).
    pub fn sign(&self, message: &[u8]) -> Result<BlsSignature, BlsError> {
        let sk = SecretKey::from_bytes(&self.0).map_err(|_| BlsError::InvalidSecretKey)?;
        let sig = sk.sign(message, BLS_DST, &[]);
        Ok(BlsSignature(sig.compress()))
    }

    /// Erzeugt den Besitznachweis zum eigenen öffentlichen Schlüssel
    /// (`PopProve`, draft-irtf-cfrg-bls-signature §3.3.2).
    ///
    /// Signiert die **komprimierten Bytes des eigenen öffentlichen
    /// Schlüssels** unter [`BLS_POP_DST`]. Wer das kann, kennt den
    /// diskreten Logarithmus seines Schlüssels — und genau das kann der
    /// Erzeuger eines Rogue Keys nicht (Fund 27 im Modulkopf).
    pub fn prove_possession(&self) -> Result<BlsProofOfPossession, BlsError> {
        let sk = SecretKey::from_bytes(&self.0).map_err(|_| BlsError::InvalidSecretKey)?;
        let pk = sk.sk_to_pk();
        let sig = sk.sign(&pk.compress(), BLS_POP_DST, &[]);
        Ok(BlsProofOfPossession(sig.compress()))
    }
}

impl BlsPublicKey {
    /// Prüft, ob der Schlüssel ein gültiger, von der Identität
    /// verschiedener G1-Punkt in der richtigen Untergruppe ist.
    pub fn validate(&self) -> Result<(), BlsError> {
        decode_validated_pk(&self.0)?;
        Ok(())
    }

    /// Eine Einzelsignatur über `message` verifizieren (mit Signatur-
    /// Gruppenprüfung und Schlüssel-Validierung).
    pub fn verify(&self, message: &[u8], signature: &BlsSignature) -> bool {
        let pk = match decode_validated_pk(&self.0) {
            Ok(pk) => pk,
            Err(_) => return false,
        };
        let sig = match Signature::from_bytes(&signature.0) {
            Ok(s) => s,
            Err(_) => return false,
        };
        sig.verify(true, message, BLS_DST, &[], &pk, true) == BLST_ERROR::BLST_SUCCESS
    }

    /// Prüft den Besitznachweis zu diesem Schlüssel (`PopVerify`,
    /// draft-irtf-cfrg-bls-signature §3.3.3).
    ///
    /// **Vor jeder Aufnahme eines fremden Schlüssels in eine Menge
    /// aufzurufen, gegen die später aggregiert verifiziert wird** —
    /// Validator-Registrierung, Pod-Mitgliedschaft. Ohne diese Prüfung
    /// ist [`fast_aggregate_verify`] angreifbar; die Begründung steht im
    /// Modulkopf unter Fund 27.
    pub fn verify_possession(&self, pop: &BlsProofOfPossession) -> bool {
        let pk = match decode_validated_pk(&self.0) {
            Ok(pk) => pk,
            Err(_) => return false,
        };
        let sig = match Signature::from_bytes(&pop.0) {
            Ok(s) => s,
            Err(_) => return false,
        };
        // Die Botschaft sind die Schlüsselbytes selbst — der Nachweis
        // bindet damit genau diesen Schlüssel und ist nicht auf einen
        // anderen übertragbar.
        sig.verify(true, &self.0, BLS_POP_DST, &[], &pk, true) == BLST_ERROR::BLST_SUCCESS
    }
}

impl BlsSignature {
    /// Zu einem Aggregat-Typ umwandeln (für den Fall, dass ein Bündel
    /// aus genau einer Signatur besteht — das Aggregat ist dann die
    /// Signatur selbst).
    pub fn as_aggregate(&self) -> BlsAggregateSignature {
        BlsAggregateSignature(self.0)
    }
}

/// Mehrere Signaturen zu einem Aggregat zusammenfassen.
///
/// Die Eingabe-Signaturen müssen gültig dekodierbar sein; die
/// eigentliche Korrektheit wird mit `aggregate_verify` bzw.
/// `fast_aggregate_verify` gegen die öffentlichen Schlüssel geprüft.
pub fn aggregate_signatures(signatures: &[BlsSignature]) -> Result<BlsAggregateSignature, BlsError> {
    if signatures.is_empty() {
        return Err(BlsError::EmptyAggregate);
    }
    let mut decoded = Vec::with_capacity(signatures.len());
    for s in signatures {
        let sig = Signature::from_bytes(&s.0).map_err(|_| BlsError::InvalidSignature)?;
        decoded.push(sig);
    }
    let refs: Vec<&Signature> = decoded.iter().collect();
    let agg = AggregateSignature::aggregate(&refs, false).map_err(|_| BlsError::InvalidSignature)?;
    Ok(BlsAggregateSignature(agg.to_signature().compress()))
}

/// Aggregat-Verifikation für den Fall, dass alle Unterzeichner dieselbe
/// Nachricht signiert haben (der PoI-Bündel-Fall: alle Pod-Mitglieder
/// bestätigen dieselbe Arbeit). Entspricht `FastAggregateVerify`.
/// Alle öffentlichen Schlüssel werden vorher validiert (Rogue-Key-Schutz).
pub fn fast_aggregate_verify(
    public_keys: &[BlsPublicKey],
    message: &[u8],
    aggregate: &BlsAggregateSignature,
) -> bool {
    if public_keys.is_empty() {
        return false;
    }
    let mut pks = Vec::with_capacity(public_keys.len());
    for p in public_keys {
        match decode_validated_pk(&p.0) {
            Ok(pk) => pks.push(pk),
            Err(_) => return false,
        }
    }
    let sig = match Signature::from_bytes(&aggregate.0) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let refs: Vec<&PublicKey> = pks.iter().collect();
    sig.fast_aggregate_verify(true, message, BLS_DST, &refs) == BLST_ERROR::BLST_SUCCESS
}

/// Aggregat-Verifikation für den Fall, dass jede:r Unterzeichner:in eine
/// eigene Nachricht signiert hat (z. B. ein Bündel unterschiedlicher
/// Segmente). Entspricht `AggregateVerify`. `public_keys[i]` gehört zu
/// `messages[i]`. Alle öffentlichen Schlüssel werden vorher validiert.
pub fn aggregate_verify(
    public_keys: &[BlsPublicKey],
    messages: &[&[u8]],
    aggregate: &BlsAggregateSignature,
) -> bool {
    if public_keys.is_empty() || public_keys.len() != messages.len() {
        return false;
    }
    let mut pks = Vec::with_capacity(public_keys.len());
    for p in public_keys {
        match decode_validated_pk(&p.0) {
            Ok(pk) => pks.push(pk),
            Err(_) => return false,
        }
    }
    let sig = match Signature::from_bytes(&aggregate.0) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pk_refs: Vec<&PublicKey> = pks.iter().collect();
    sig.aggregate_verify(true, messages, BLS_DST, &pk_refs, true) == BLST_ERROR::BLST_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drei deterministische Schlüsselpaare aus festem IKM erzeugen.
    fn key_triple() -> (BlsSecretKey, BlsSecretKey, BlsSecretKey) {
        let a = BlsSecretKey::key_gen(&[0x11u8; 32]).expect("KeyGen");
        let b = BlsSecretKey::key_gen(&[0x22u8; 32]).expect("KeyGen");
        let c = BlsSecretKey::key_gen(&[0x33u8; 32]).expect("KeyGen");
        (a, b, c)
    }

    #[test]
    fn schluesselgenerierung_und_oeffentlicher_schluessel() {
        let sk = BlsSecretKey::key_gen(&[0x42u8; 32]).expect("KeyGen");
        let pk = sk.public_key().expect("Public Key");
        assert_eq!(pk.0.len(), BLS_PK_LEN);
        assert!(pk.validate().is_ok());
        // Deterministisch: dasselbe IKM ⇒ derselbe Schlüssel.
        let sk2 = BlsSecretKey::key_gen(&[0x42u8; 32]).expect("KeyGen");
        assert_eq!(sk2.public_key().expect("Public Key"), pk);
    }

    #[test]
    fn keygen_lehnt_zu_kurzes_ikm_ab() {
        // KeyGen verlangt mindestens 32 Bytes Eingabematerial.
        assert!(matches!(
            BlsSecretKey::key_gen(&[0x42u8; 31]),
            Err(BlsError::KeyGenFailed)
        ));
    }

    #[test]
    fn einzelsignatur_rundlauf() {
        let (a, b, _) = key_triple();
        let msg = b"myelith: segment-commitment";
        let sig = a.sign(msg).expect("Signieren");
        let pk_a = a.public_key().expect("Public Key");
        assert!(pk_a.verify(msg, &sig));
        // Deterministisch: dieselbe Nachricht ⇒ dieselbe Signatur.
        assert_eq!(a.sign(msg).expect("Signieren"), sig);
        // Falsche Nachricht ⇒ Verifikation scheitert.
        assert!(!pk_a.verify(b"myelith: andere-nachricht", &sig));
        // Falscher Schlüssel ⇒ Verifikation scheitert.
        let pk_b = b.public_key().expect("Public Key");
        assert!(!pk_b.verify(msg, &sig));
    }

    #[test]
    fn manipulierte_signatur_wird_abgelehnt() {
        let (a, _, _) = key_triple();
        let msg = b"myelith: manipulations-test";
        let mut sig = a.sign(msg).expect("Signieren");
        let pk_a = a.public_key().expect("Public Key");
        assert!(pk_a.verify(msg, &sig));
        // Jedes Byte einzeln verfälschen.
        for byte in 0..BLS_SIG_LEN {
            for delta in [1u8, 0x80, 0xff] {
                sig.0[byte] ^= delta;
                assert!(
                    !pk_a.verify(msg, &sig),
                    "Verfälschung Byte {} (delta {}) muss scheitern",
                    byte,
                    delta
                );
                sig.0[byte] ^= delta;
            }
        }
        assert!(pk_a.verify(msg, &sig));
    }

    // ── Proof-of-Possession (Fund 27) ───────────────────────────

    #[test]
    fn besitznachweis_gilt_fuer_den_eigenen_schluessel() {
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let pop = sk.prove_possession().expect("pop");
        assert!(pk.verify_possession(&pop));
    }

    #[test]
    fn besitznachweis_ist_nicht_uebertragbar() {
        // Der Nachweis signiert die eigenen Schluesselbytes; unter einem
        // anderen Schluessel ergibt er keinen Sinn.
        let sk_a = BlsSecretKey::key_gen(&[3u8; 32]).expect("a");
        let sk_b = BlsSecretKey::key_gen(&[4u8; 32]).expect("b");
        let pk_b = sk_b.public_key().expect("pk_b");
        let pop_a = sk_a.prove_possession().expect("pop_a");
        assert!(!pk_b.verify_possession(&pop_a));
    }

    #[test]
    fn nachrichtensignatur_ist_kein_besitznachweis() {
        // Ohne getrennte Domain-Tags waere eine gewoehnliche Signatur
        // ueber die eigenen Schluesselbytes ein gueltiger Nachweis.
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let sig = sk.sign(&pk.0).expect("sign");
        assert!(!pk.verify_possession(&BlsProofOfPossession(sig.0)));
    }

    #[test]
    fn besitznachweis_ist_keine_nachrichtensignatur() {
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let pop = sk.prove_possession().expect("pop");
        assert!(!pk.verify(&pk.0, &BlsSignature(pop.0)));
    }

    #[test]
    fn pop_dst_unterscheidet_sich_vom_signatur_dst() {
        assert_ne!(BLS_DST, BLS_POP_DST);
    }

    #[test]
    fn verstuemmelter_besitznachweis_wird_abgelehnt() {
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let mut pop = sk.prove_possession().expect("pop");
        pop.0[0] ^= 0x01;
        assert!(!pk.verify_possession(&pop));
        assert!(!pk.verify_possession(&BlsProofOfPossession([0u8; BLS_SIG_LEN])));
    }

    #[test]
    fn fast_aggregate_verify_gleiche_nachricht() {
        // Der PoI-Bündel-Fall: alle Unterzeichner bestätigen dieselbe Arbeit.
        let (a, b, c) = key_triple();
        let msg = b"myelith: poi-bundle";
        let sig_a = a.sign(msg).expect("Signieren");
        let sig_b = b.sign(msg).expect("Signieren");
        let sig_c = c.sign(msg).expect("Signieren");
        let agg = aggregate_signatures(&[sig_a, sig_b, sig_c]).expect("Aggregation");
        let pks = [
            a.public_key().expect("pk"),
            b.public_key().expect("pk"),
            c.public_key().expect("pk"),
        ];
        assert!(fast_aggregate_verify(&pks, msg, &agg));
        // Falsche Nachricht ⇒ scheitert.
        assert!(!fast_aggregate_verify(&pks, b"myelith: falsch", &agg));
        // Ein Unterzeichner fehlt ⇒ scheitert.
        assert!(!fast_aggregate_verify(&pks[..2], msg, &agg));
        // Ein falscher Schlüssel darunter ⇒ scheitert.
        let mut pks_falsch = pks;
        let fremd = BlsSecretKey::key_gen(&[0x99u8; 32]).expect("KeyGen");
        pks_falsch[2] = fremd.public_key().expect("pk");
        assert!(!fast_aggregate_verify(&pks_falsch, msg, &agg));
    }

    #[test]
    fn aggregate_verify_verschiedene_nachrichten() {
        let (a, b, c) = key_triple();
        let m_a = b"segment-eins";
        let m_b = b"segment-zwei";
        let m_c = b"segment-drei";
        let sig_a = a.sign(m_a).expect("Signieren");
        let sig_b = b.sign(m_b).expect("Signieren");
        let sig_c = c.sign(m_c).expect("Signieren");
        let agg = aggregate_signatures(&[sig_a, sig_b, sig_c]).expect("Aggregation");
        let pks = [
            a.public_key().expect("pk"),
            b.public_key().expect("pk"),
            c.public_key().expect("pk"),
        ];
        let messages: [&[u8]; 3] = [m_a, m_b, m_c];
        assert!(aggregate_verify(&pks, &messages, &agg));
        // Nachrichten vertauscht ⇒ scheitert.
        let vertauscht: [&[u8]; 3] = [m_b, m_a, m_c];
        assert!(!aggregate_verify(&pks, &vertauscht, &agg));
        // Länge passt nicht ⇒ scheitert.
        assert!(!aggregate_verify(&pks[..2], &messages, &agg));
    }

    #[test]
    fn einzelsignatur_als_aggregat() {
        let (a, _, _) = key_triple();
        let msg = b"myelith: ein-zu-eins";
        let sig = a.sign(msg).expect("Signieren");
        let agg = sig.as_aggregate();
        let pk_a = a.public_key().expect("pk");
        assert!(fast_aggregate_verify(&[pk_a], msg, &agg));
    }

    #[test]
    fn leere_aggregation_wird_abgelehnt() {
        assert_eq!(aggregate_signatures(&[]), Err(BlsError::EmptyAggregate));
        assert!(!fast_aggregate_verify(&[], b"x", &BlsAggregateSignature([0u8; BLS_SIG_LEN])));
    }

    #[test]
    fn ungueltiger_oeffentlicher_schluessel_wird_abgelehnt() {
        // 0xff…ff ist keine gültige G1-Punkt-Kodierung.
        let pk = BlsPublicKey([0xffu8; BLS_PK_LEN]);
        assert!(pk.validate().is_err());
        let sig = BlsSignature([0u8; BLS_SIG_LEN]);
        assert!(!pk.verify(b"x", &sig));
    }

    #[test]
    fn borsh_rundtrip() {
        use borsh::{from_slice, to_vec};
        let (a, _, _) = key_triple();
        let msg = b"myelith: borsh";
        let sig = a.sign(msg).expect("Signieren");
        let pk = a.public_key().expect("pk");
        let pk_back: BlsPublicKey = from_slice(&to_vec(&pk).expect("ser")).expect("de");
        let sig_back: BlsSignature = from_slice(&to_vec(&sig).expect("ser")).expect("de");
        assert_eq!(pk_back, pk);
        assert_eq!(sig_back, sig);
        assert!(pk_back.verify(msg, &sig_back));
    }
}
