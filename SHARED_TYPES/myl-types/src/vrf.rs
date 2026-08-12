//! VRF: ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381, §5.5).
//!
//! Referenzimplementierung für Myeliths Zufallslosungen (Epochen-Scheduler,
//! Kontrollsegment-Zulosung, Trainings-Datenzuweisung). Die Implementierung
//! folgt strikt RFC 9381 und ist gegen die offiziellen Testvektoren aus
//! Anhang B.3 (Beispiele 16–18) geprüft.
//!
//! Suite-Parameter (RFC 9381 §5.5):
//! - Suite-String: `0x03` (ECVRF-EDWARDS25519-SHA512-TAI)
//! - Gruppe: edwards25519 (RFC 8032), Cofactor 8
//! - Hash: SHA-512 (hLen = 64), cLen = 16, ptLen = qLen = 32
//! - `encode_to_curve`: Try-and-Increment (§5.4.1.1), Salt = öffentlicher Schlüssel
//! - Nonce: deterministisch nach §5.4.2.2 (RFC-8032-Variante) — dieselbe
//!   Eingabe ergibt immer denselben Beweis (wichtig für Nachvollziehbarkeit)
//! - Schlüssel: Ed25519-Schlüsselableitung nach RFC 8032 §5.1.5
//!
//! Post-Quantum-Migrationspfad (Design-Entscheidung 2026-08-12):
//! `VrfOutput.algorithm` trägt das Versionsfeld; ein späterer Tausch
//! (Kandidat: deterministisches ML-DSA-basiertes Signatur-Hash-VRF,
//! FIPS 204 §5.2) bekommt eine neue Versionsnummer, ohne die
//! Blockstruktur zu brechen (siehe GOVERNANCE-Fahrplan,
//! Design-Entscheidung 4: Krypto-Agilität).

use borsh::{BorshDeserialize, BorshSerialize};
use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use sha2::{Digest, Sha512};

use crate::protocol;

/// Suite-String für ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381 §5.5).
pub const SUITE_STRING: u8 = 0x03;
/// Domain-Separator: encode_to_curve (§5.4.1.1).
const ENCODE_TO_CURVE_FRONT: u8 = 0x01;
/// Domain-Separator: Challenge-Erzeugung (§5.4.3).
const CHALLENGE_FRONT: u8 = 0x02;
/// Domain-Separator: proof_to_hash (§5.2).
const PROOF_TO_HASH_FRONT: u8 = 0x03;
/// Abschließender Domain-Separator (alle Hash-Eingaben, RFC 9381 §5.4).
const DOMAIN_BACK: u8 = 0x00;
/// Länge der Challenge in Bytes (cLen, RFC 9381 §5.5).
pub const C_LEN: usize = 16;
/// Länge des Beweises: Gamma (32) || c (16) || s (32).
pub const PROOF_LEN: usize = 80;
/// Länge des VRF-Outputs beta (hLen = SHA-512).
pub const BETA_LEN: usize = 64;
/// Maximale Try-and-Increment-Versuche (praktische Obergrenze; die
/// Trefferwahrscheinlichkeit je Versuch ist ~1/2, 256 Versuche sind
/// astronomisch konservativ).
const TAI_MAX_CTR: u16 = 256;

/// Fehler der VRF-Operationen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrfError {
    /// Der öffentliche Schlüssel dekodiert nicht zu einem Kurvenpunkt.
    InvalidPublicKey,
    /// Der öffentliche Schlüssel liegt in der Kleinordnungs-Untergruppe
    /// (validate_key, RFC 9381 §5.4.5) — solcher Schlüssel müssen
    /// abgelehnt werden.
    SmallOrderPublicKey,
    /// Der Beweis ist formal ungültig (falsche Länge, Punkt dekodiert
    /// nicht, s nicht kanonisch).
    InvalidProof,
    /// Try-and-Increment fand keinen gültigen Punkt (praktisch
    /// ausgeschlossen, aber als Fehlerpfad geführt).
    HashToCurveFailed,
    /// Die Challenge-Prüfung schlug fehl — der Beweis passt nicht zur
    /// Eingabe bzw. zum Schlüssel.
    VerificationFailed,
}

impl std::fmt::Display for VrfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPublicKey => write!(f, "VRF: öffentlicher Schlüssel ist kein Kurvenpunkt"),
            Self::SmallOrderPublicKey => write!(f, "VRF: öffentlicher Schlüssel hat kleine Ordnung"),
            Self::InvalidProof => write!(f, "VRF: Beweis ist formal ungültig"),
            Self::HashToCurveFailed => write!(f, "VRF: Hash-to-Curve ohne Treffer"),
            Self::VerificationFailed => write!(f, "VRF: Beweis-Verifikation fehlgeschlagen"),
        }
    }
}

impl std::error::Error for VrfError {}

/// Geheimer VRF-Schlüssel: 32-Byte-Seed nach RFC 8032 (Ed25519-Ableitung).
#[derive(Clone)]
pub struct VrfSecretKey([u8; 32]);

/// Öffentlicher VRF-Schlüssel: komprimierter Edwards-Punkt (32 Bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct VrfPublicKey(pub [u8; 32]);

/// VRF-Beweis: `point_to_string(Gamma) || int_to_string(c, 16) || int_to_string(s, 32)`
/// (RFC 9381 §5.1, Schritt 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct VrfProof(pub [u8; PROOF_LEN]);

/// VRF-Ausgabe: der Hash-Wert beta plus Algorithmus-Versionsfeld
/// (Post-Quantum-Migrationspfad, Design-Entscheidung 2026-08-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct VrfOutput {
    /// Algorithmus-Version (aktuell `protocol::VRF_ALGO_ECVRF_CURVE25519`).
    pub algorithm: u8,
    /// beta_string = Hash(suite || 0x03 || cofactor·Gamma || 0x00), 64 Bytes.
    pub beta: [u8; BETA_LEN],
}

/// Geheime Schlüssel ableiten: Ed25519-Ableitung nach RFC 8032 §5.1.5 —
/// SHA-512 des Seeds, untere 32 Bytes geklammert (Clamping), als
/// Little-Endian-Skalar interpretiert. Die Reduktion mod q ändert hier
/// nichts: Alle Skalarmultiplikationen der VRF wirken ausschließlich auf
/// Punkten der Primordnungs-Untergruppe (Basispunkt, cofactor-bereinigtes
/// H), und dort gilt [x]P = [x mod q]P.
fn derive_secret_scalar(seed: &[u8; 32]) -> Scalar {
    let mut h = [0u8; 64];
    h.copy_from_slice(&Sha512::digest(seed));
    h[0] &= 248;
    h[31] &= 127;
    h[31] |= 64;
    let mut s = [0u8; 32];
    s.copy_from_slice(&h[..32]);
    Scalar::from_bytes_mod_order(s)
}

/// Kanonizitätsprüfung für ein 32-Byte-Feld-Element (Little-Endian):
/// Der Wert muss echt kleiner als p = 2^255 − 19 sein.
fn is_canonical_field_element(bytes: &[u8; 32]) -> bool {
    const P_LE: [u8; 32] = [
        0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0x7f,
    ];
    for i in (0..32).rev() {
        if bytes[i] < P_LE[i] {
            return true;
        }
        if bytes[i] > P_LE[i] {
            return false;
        }
    }
    false // gleich p — nicht kanonisch
}

/// Kanonische Punkt-Dekodierung nach RFC 8032 §5.1.3: Die y-Koordinate
/// (Vorzeichen-Bit = MSB von Byte 31 herausgerechnet) muss kanonisch
/// sein, also echt kleiner als p = 2^255 − 19, sonst ist die Kodierung
/// ungültig. curve25519-dalek allein ist hier tolerant und reduziert
/// y ≥ p stillschweigend — für den Konsens ist Eindeutigkeit der
/// Dekodierung zwingend, daher die explizite Prüfung. Die Dekodierung
/// selbst läuft über die ORIGINAL-Bytes (inkl. Vorzeichen-Bit), damit
/// das korrekte x-Vorzeichen gewählt wird.
fn point_from_canonical_bytes(bytes: [u8; 32]) -> Option<EdwardsPoint> {
    let mut y_bytes = bytes;
    y_bytes[31] &= 0x7f; // Vorzeichen-Bit ist nicht Teil der y-Koordinate
    if !is_canonical_field_element(&y_bytes) {
        return None;
    }
    CompressedEdwardsY(bytes).decompress()
}

/// Try-and-Increment-Hash auf die Kurve (RFC 9381 §5.4.1.1) mit
/// Cofactor-Bereinigung (§5.4.1.1, Schritt 4).
fn encode_to_curve(pk: &[u8; 32], alpha: &[u8]) -> Result<EdwardsPoint, VrfError> {
    for ctr in 0..TAI_MAX_CTR {
        let mut hasher = Sha512::new();
        hasher.update([SUITE_STRING, ENCODE_TO_CURVE_FRONT]);
        hasher.update(pk);
        hasher.update(alpha);
        hasher.update([ctr as u8]);
        hasher.update([DOMAIN_BACK]);
        let digest = hasher.finalize();
        let mut candidate = [0u8; 32];
        candidate.copy_from_slice(&digest[..32]);
        if let Some(point) = point_from_canonical_bytes(candidate) {
            // Cofactor > 1: H = cofactor * H (RFC 9381 §5.4.1.1, Schritt 4).
            // Ist das Ergebnis der Identitätspunkt (Kandidat hatte kleine
            // Ordnung), weiterzählen.
            if !point.is_small_order() {
                return Ok(point.mul_by_cofactor());
            }
        }
    }
    Err(VrfError::HashToCurveFailed)
}

/// Challenge-Erzeugung (RFC 9381 §5.4.3) über die fünf Punkte.
fn generate_challenge(
    y: &EdwardsPoint,
    h: &EdwardsPoint,
    gamma: &EdwardsPoint,
    u: &EdwardsPoint,
    v: &EdwardsPoint,
) -> Scalar {
    let mut hasher = Sha512::new();
    hasher.update([SUITE_STRING, CHALLENGE_FRONT]);
    for point in [y, h, gamma, u, v] {
        hasher.update(point.compress().to_bytes());
    }
    hasher.update([DOMAIN_BACK]);
    let digest = hasher.finalize();
    let mut c_bytes = [0u8; 32];
    c_bytes[..C_LEN].copy_from_slice(&digest[..C_LEN]);
    Scalar::from_bytes_mod_order(c_bytes)
}

/// Deterministische Nonce (RFC 9381 §5.4.2.2, RFC-8032-Variante).
fn generate_nonce(seed: &[u8; 32], h_string: &[u8; 32]) -> Scalar {
    let hashed_sk = Sha512::digest(seed);
    let mut k_hasher = Sha512::new();
    k_hasher.update(&hashed_sk[32..64]);
    k_hasher.update(h_string);
    let k_string = k_hasher.finalize();
    let mut wide = [0u8; 64];
    wide.copy_from_slice(&k_string);
    Scalar::from_bytes_mod_order_wide(&wide)
}

/// Beweis dekodieren (RFC 9381 §5.4.4): Gamma-Punkt, Challenge c,
/// Skalar s (kanonisch, < q).
fn decode_proof(pi: &[u8; PROOF_LEN]) -> Result<(EdwardsPoint, Scalar, Scalar), VrfError> {
    let mut gamma_bytes = [0u8; 32];
    gamma_bytes.copy_from_slice(&pi[..32]);
    let gamma = point_from_canonical_bytes(gamma_bytes).ok_or(VrfError::InvalidProof)?;
    let mut c_bytes = [0u8; 32];
    c_bytes[..C_LEN].copy_from_slice(&pi[32..32 + C_LEN]);
    let c = Scalar::from_bytes_mod_order(c_bytes);
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&pi[32 + C_LEN..]);
    let s: Option<Scalar> = Scalar::from_canonical_bytes(s_bytes).into();
    let s = s.ok_or(VrfError::InvalidProof)?;
    Ok((gamma, c, s))
}

/// beta aus einem gültigen Beweis ableiten (RFC 9381 §5.2):
/// `Hash(suite || 0x03 || point_to_string(cofactor·Gamma) || 0x00)`.
fn proof_to_hash(gamma: &EdwardsPoint) -> [u8; BETA_LEN] {
    let cleared = gamma.mul_by_cofactor();
    let mut hasher = Sha512::new();
    hasher.update([SUITE_STRING, PROOF_TO_HASH_FRONT]);
    hasher.update(cleared.compress().to_bytes());
    hasher.update([DOMAIN_BACK]);
    let digest = hasher.finalize();
    let mut beta = [0u8; BETA_LEN];
    beta.copy_from_slice(&digest);
    beta
}

impl VrfSecretKey {
    /// Aus einem 32-Byte-Seed (RFC-8032-Format).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(seed)
    }

    /// Der zugehörige öffentliche Schlüssel (Y = x·B).
    pub fn public_key(&self) -> VrfPublicKey {
        let x = derive_secret_scalar(&self.0);
        let y = ED25519_BASEPOINT_POINT * x;
        VrfPublicKey(y.compress().to_bytes())
    }

    /// ECVRF_prove (RFC 9381 §5.1): Beweis und Ausgabe für `alpha`.
    /// Deterministisch — dieselbe Eingabe ergibt stets denselben Beweis.
    pub fn prove(&self, alpha: &[u8]) -> Result<(VrfProof, VrfOutput), VrfError> {
        let x = derive_secret_scalar(&self.0);
        let y_point = ED25519_BASEPOINT_POINT * x;
        let pk_bytes = self.public_key().0;
        let h = encode_to_curve(&pk_bytes, alpha)?;
        let gamma = h * x;
        let k = generate_nonce(&self.0, &h.compress().to_bytes());
        let u = ED25519_BASEPOINT_POINT * k;
        let v = h * k;
        let c = generate_challenge(&y_point, &h, &gamma, &u, &v);
        let s = k + c * x;
        let mut pi = [0u8; PROOF_LEN];
        pi[..32].copy_from_slice(&gamma.compress().to_bytes());
        pi[32..32 + C_LEN].copy_from_slice(&c.to_bytes()[..C_LEN]);
        pi[32 + C_LEN..].copy_from_slice(&s.to_bytes());
        let beta = proof_to_hash(&gamma);
        Ok((
            VrfProof(pi),
            VrfOutput {
                algorithm: protocol::VRF_ALGO_ECVRF_CURVE25519,
                beta,
            },
        ))
    }
}

impl VrfPublicKey {
    /// ECVRF_verify (RFC 9381 §5.3, mit validate_key = TRUE —
    /// Konsens-Knoten müssen öffentliche Schlüssel immer validieren).
    /// Liefert bei Erfolg die VRF-Ausgabe (beta) zurück.
    pub fn verify(&self, alpha: &[u8], proof: &VrfProof) -> Result<VrfOutput, VrfError> {
        // Schritt 1+2: öffentlicher Schlüssel dekodieren (kanonisch).
        let y = point_from_canonical_bytes(self.0).ok_or(VrfError::InvalidPublicKey)?;
        // Schritt 3: validate_key (§5.4.5) — Kleinordnungs-Punkte ablehnen.
        if y.is_small_order() {
            return Err(VrfError::SmallOrderPublicKey);
        }
        // Schritte 4–6: Beweis dekodieren.
        let (gamma, c, s) = decode_proof(&proof.0)?;
        // Schritt 7: H neu berechnen.
        let h = encode_to_curve(&self.0, alpha)?;
        // Schritte 8+9: U = s·B − c·Y, V = s·H − c·Gamma.
        let u = ED25519_BASEPOINT_POINT * s - y * c;
        let v = h * s - gamma * c;
        // Schritt 10: Challenge nachrechnen.
        let c_prime = generate_challenge(&y, &h, &gamma, &u, &v);
        // Schritt 11: Vergleich und beta-Ableitung.
        if c == c_prime {
            Ok(VrfOutput {
                algorithm: protocol::VRF_ALGO_ECVRF_CURVE25519,
                beta: proof_to_hash(&gamma),
            })
        } else {
            Err(VrfError::VerificationFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("Hex"))
            .collect()
    }

    fn hex_arr<const N: usize>(s: &str) -> [u8; N] {
        let v = hex(s);
        assert_eq!(v.len(), N, "Hex-Länge passt nicht");
        let mut out = [0u8; N];
        out.copy_from_slice(&v);
        out
    }

    /// Offizielle Testvektoren aus RFC 9381 Anhang B.3.
    const VECTORS: &[(&str, &str, &str, &str, &str)] = &[
        (
            // Beispiel 16: alpha = "" (leer), ctr = 0
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "8657106690b5526245a92b003bb079ccd1a92130477671f6fc01ad16f26f723f26f8a57ccaed74ee1b190bed1f479d9727d2d0f9b005a6e456a35d4fb0daab1268a1b0db10836d9826a528ca76567805",
            "90cf1df3b703cce59e2a35b925d411164068269d7b2d29f3301c03dd757876ff66b71dda49d2de59d03450451af026798e8f81cd2e333de5cdf4f3e140fdd8ae",
        ),
        (
            // Beispiel 17: alpha = 0x72, ctr = 1
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "f3141cd382dc42909d19ec5110469e4feae18300e94f304590abdced48aed5933bf0864a62558b3ed7f2fea45c92a465301b3bbf5e3e54ddf2d935be3b67926da3ef39226bbc355bdc9850112c8f4b02",
            "eb4440665d3891d668e7e0fcaf587f1b4bd7fbfe99d0eb2211ccec90496310eb5e33821bc613efb94db5e5b54c70a848a0bef4553a41befc57663b56373a5031",
        ),
        (
            // Beispiel 18: alpha = 0xaf82, ctr = 0
            "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            "9bc0f79119cc5604bf02d23b4caede71393cedfbb191434dd016d30177ccbf8096bb474e53895c362d8628ee9f9ea3c0e52c7a5c691b6c18c9979866568add7a2d41b00b05081ed0f58ee5e31b3a970e",
            "645427e5d00c62a23fb703732fa5d892940935942101e456ecca7bb217c61c452118fec1219202a0edcf038bb6373241578be7217ba85a2687f7a0310b2df19f",
        ),
    ];

    #[test]
    fn kanonizitaetspruefung() {
        // y = 0 ist kanonisch und dekodiert (Punkt mit y=0 existiert nicht
        // zwingend; hier geht es nur um die Kanonizität → wir prüfen die
        // Hilfsfunktion direkt).
        let zero = [0u8; 32];
        assert!(is_canonical_field_element(&zero));
        // y = p ist nicht kanonisch (Gleichheit).
        let mut p_le = [0xffu8; 32];
        p_le[0] = 0xed;
        p_le[31] = 0x7f;
        assert!(!is_canonical_field_element(&p_le));
        // y = 0x7fff…ff (> p) ist nicht kanonisch.
        let mut too_big = [0xffu8; 32];
        too_big[31] = 0x7f;
        assert!(!is_canonical_field_element(&too_big));
        // Ein kleiner Wert ist kanonisch.
        let mut small = [0u8; 32];
        small[0] = 0x01;
        assert!(is_canonical_field_element(&small));
    }

    #[test]
    fn nicht_kanonische_punkt_kodierung_wird_abgelehnt() {
        // y = p (mit gesetztem Vorzeichen-Bit in Byte 31) darf nicht
        // dekodieren — auch dann nicht, wenn das Vorzeichen-Bit gesetzt ist.
        let mut y_p = [0xffu8; 32];
        y_p[0] = 0xed;
        // y_p[31] bleibt 0xff (Vorzeichen-Bit gesetzt) → y ohne Vorzeichen = p.
        assert!(point_from_canonical_bytes(y_p).is_none());
        // Dasselbe ohne Vorzeichen-Bit.
        y_p[31] = 0x7f;
        assert!(point_from_canonical_bytes(y_p).is_none());
    }

    #[test]
    fn rfc9381_testvektoren_beweiserzeugung() {
        for (i, (sk_hex, pk_hex, alpha_hex, pi_hex, beta_hex)) in VECTORS.iter().enumerate() {
            let sk = VrfSecretKey::from_seed(hex_arr(sk_hex));
            // Öffentlicher Schlüssel muss der RFC-Ableitung entsprechen.
            assert_eq!(
                sk.public_key().0,
                hex_arr::<32>(pk_hex),
                "Beispiel {}: öffentlicher Schlüssel",
                16 + i
            );
            let alpha = hex(alpha_hex);
            let (proof, output) = sk.prove(&alpha).expect("Beweiserzeugung");
            assert_eq!(
                proof.0,
                hex_arr::<PROOF_LEN>(pi_hex),
                "Beispiel {}: pi_string",
                16 + i
            );
            assert_eq!(
                output.beta,
                hex_arr::<BETA_LEN>(beta_hex),
                "Beispiel {}: beta_string",
                16 + i
            );
            assert_eq!(output.algorithm, protocol::VRF_ALGO_ECVRF_CURVE25519);
        }
    }

    #[test]
    fn rfc9381_testvektoren_verifikation() {
        for (i, (_sk_hex, pk_hex, alpha_hex, pi_hex, beta_hex)) in VECTORS.iter().enumerate() {
            let pk = VrfPublicKey(hex_arr(pk_hex));
            let alpha = hex(alpha_hex);
            let proof = VrfProof(hex_arr(pi_hex));
            let output = pk.verify(&alpha, &proof).expect("Verifikation");
            assert_eq!(
                output.beta,
                hex_arr::<BETA_LEN>(beta_hex),
                "Beispiel {}: beta aus Verifikation",
                16 + i
            );
        }
    }

    #[test]
    fn verifikation_lehnt_falsche_eingabe_ab() {
        let (sk_hex, pk_hex, alpha_hex, pi_hex, _) = VECTORS[0];
        let pk = VrfPublicKey(hex_arr(pk_hex));
        let proof = VrfProof(hex_arr(pi_hex));
        assert_eq!(
            pk.verify(b"anderes-alpha", &proof),
            Err(VrfError::VerificationFailed)
        );
        // Vektor 17 (alpha = 0x72) mit Vektor 16 kombiniert.
        let alpha17 = hex(VECTORS[1].2);
        assert_eq!(
            pk.verify(&alpha17, &proof),
            Err(VrfError::VerificationFailed)
        );
    }

    #[test]
    fn verifikation_lehnt_verfaelschten_beweis_ab() {
        let (_sk_hex, pk_hex, alpha_hex, pi_hex, _) = VECTORS[0];
        let pk = VrfPublicKey(hex_arr(pk_hex));
        let alpha = hex(alpha_hex);
        let mut pi = hex_arr::<PROOF_LEN>(pi_hex);
        // Jedes einzelne Byte an mehreren Positionen verfälschen.
        for pos in [0usize, 16, 31, 32, 47, 48, 79] {
            for delta in [1u8, 0x80, 0xff] {
                pi[pos] ^= delta;
                let proof = VrfProof(pi);
                let result = pk.verify(&alpha, &proof);
                assert!(
                    result.is_err(),
                    "Verfälschung an Position {} (delta {}) muss scheitern",
                    pos,
                    delta
                );
                pi[pos] ^= delta;
            }
        }
    }

    #[test]
    fn verifikation_lehnt_falschen_schluessel_ab() {
        let (_, _, alpha_hex, pi_hex, _) = VECTORS[0];
        let pk_falsch = VrfPublicKey(hex_arr(VECTORS[1].1));
        let alpha = hex(alpha_hex);
        let proof = VrfProof(hex_arr(pi_hex));
        assert_eq!(
            pk_falsch.verify(&alpha, &proof),
            Err(VrfError::VerificationFailed)
        );
    }

    #[test]
    fn kleinordnungs_schluessel_wird_abgelehnt() {
        // Der Identitätspunkt (kodiert 0x01, 0x00…) hat Ordnung 1.
        let mut identity = [0u8; 32];
        identity[0] = 0x01;
        let pk = VrfPublicKey(identity);
        let proof = VrfProof([0u8; PROOF_LEN]);
        assert_eq!(pk.verify(b"alpha", &proof), Err(VrfError::SmallOrderPublicKey));
    }

    #[test]
    fn ungueltiger_schluessel_wird_abgelehnt() {
        // 0xff…ff dekodiert nicht zu einem gültigen Edwards-Punkt.
        let pk = VrfPublicKey([0xffu8; 32]);
        let proof = VrfProof([0u8; PROOF_LEN]);
        assert_eq!(pk.verify(b"alpha", &proof), Err(VrfError::InvalidPublicKey));
    }

    #[test]
    fn beweis_mit_ungueltigem_gamma_wird_abgelehnt() {
        // Gamma-Kodierung mit y = p (2^255 − 19) ist keine gültige
        // Punkt-Kodierung (Feld-Elemente müssen < p sein) — der Beweis
        // muss als formal ungültig abgelehnt werden.
        let mut pi = [0xffu8; PROOF_LEN];
        pi[0] = 0xed;
        pi[31] = 0x7f;
        pi[32..].copy_from_slice(&[0u8; 48]);
        let pk = VrfPublicKey(hex_arr(VECTORS[0].1));
        let proof = VrfProof(pi);
        assert_eq!(pk.verify(b"alpha", &proof), Err(VrfError::InvalidProof));
    }

    #[test]
    fn determinismus_eigener_schluessel() {
        // Deterministische Nonce: dieselbe Eingabe ⇒ derselbe Beweis.
        let sk = VrfSecretKey::from_seed([7u8; 32]);
        let (p1, o1) = sk.prove(b"myelith-losung").expect("Beweis");
        let (p2, o2) = sk.prove(b"myelith-losung").expect("Beweis");
        assert_eq!(p1, p2);
        assert_eq!(o1, o2);
        // Und der eigene öffentliche Schlüssel verifiziert den Beweis.
        let verified = sk.public_key().verify(b"myelith-losung", &p1).expect("Verifikation");
        assert_eq!(verified, o1);
        // Andere Eingabe ⇒ anderer Beweis und anderes beta.
        let (p3, o3) = sk.prove(b"andere-losung").expect("Beweis");
        assert_ne!(p1, p3);
        assert_ne!(o1.beta, o3.beta);
    }

    #[test]
    fn vrf_output_borsh_roundtrip() {
        let sk = VrfSecretKey::from_seed([9u8; 32]);
        let (_, output) = sk.prove(b"borsh-roundtrip").expect("Beweis");
        let bytes = to_vec(&output).expect("Serialisierung");
        let back: VrfOutput = from_slice(&bytes).expect("Deserialisierung");
        assert_eq!(back, output);
    }

    #[test]
    fn beweis_borsh_roundtrip() {
        let sk = VrfSecretKey::from_seed([11u8; 32]);
        let (proof, _) = sk.prove(b"beweis-roundtrip").expect("Beweis");
        let bytes = to_vec(&proof).expect("Serialisierung");
        let back: VrfProof = from_slice(&bytes).expect("Deserialisierung");
        assert_eq!(back, proof);
    }
}
