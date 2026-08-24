//! Spur-Hashes und Übergangs-Signaturen (Anhang A.3, Schritte 2+4).
//!
//! Jeder Shard hasht seine Ausgabe-Aktivierungen (`activation_hash`) und
//! signiert den Übergang `(segment_id, vorheriger Spur-Hash, neuer
//! Spur-Hash)` mit seinem BLS-Schlüssel (`TransitionSig`). Der nächste
//! Shard prüft den Hash der empfangenen Aktivierungen gegen den letzten
//! Spur-Eintrag — das ist die Manipulationserkennung.
//!
//! Alle Operationen sind deterministisch: derselbe Input ergibt denselben
//! Hash und dieselbe Signatur (BLS ohne Zufallsdaten).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use myl_types::ids::SegmentId;
use sha2::{Digest, Sha256};

/// Hash über die Aktivierungen eines Shard-Ausgangs.
///
/// Die Aktivierungen werden als little-endian i16-Folge gehasht. Das ist
/// der Spur-Eintrag `h(a_i)` (Anhang A.3). Deterministisch und auf jedem
/// Node identisch.
pub fn activation_hash(activations: &[i16]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for v in activations {
        hasher.update(v.to_le_bytes());
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Die zu signierende Übergangs-Nachricht (Anhang A.3
/// `sign_transition(seg.id, seg.trace[i-1], h_next)`).
///
/// Borsh-serialisiert über `(segment_id, shard_index, position,
/// prev_hash, next_hash)`. Für Shard 0 ist `prev_hash` der Null-Hash
/// (es gibt keinen vorherigen Spur-Eintrag).
///
/// # Domain-Separation und Rolle (eingeführt 2026-08-24)
///
/// Signiert wird nicht die reine Borsh-Folge, sondern
/// `DST_SHARD_TRANSITION ‖ Rolle ‖ Borsh(TransitionSig)`.
///
/// **Warum ein Präfix (Bedrohungsmodell 4.1).** Bis dahin war dies die
/// **einzige Signaturverwendung im Projekt ohne Domain-Separation**;
/// jede andere trägt ein Präfix im Klartext (`MYELITH_BFT_VOTE_v1`,
/// `MYELITH_POI_BUNDLE_v1` und so fort). Eine Verwechslung war unmöglich,
/// aber nicht durch Design: Die Botschaft war 112 Bytes lang und keine
/// andere Klasse war das (59, 61, 62, 74, 101, 48). **Der Schutz war ein
/// Längenzufall** und wäre still verschwunden, sobald jemand eine Klasse
/// auf 112 Bytes bringt oder dieses `struct` um ein Feld ändert: Kein
/// Test wäre fehlgeschlagen, kein Kompilat gebrochen.
///
/// **Warum die Rolle (Bedrohungsmodell 5.3).** Ein Miner benutzt seinen
/// BLS-Schlüssel in mehreren Rollen: als Shard, als Pod-Mitglied für
/// PoI-Bündel, möglicherweise als Validator. Ob eine Identität in allen
/// Rollen dasselbe Schlüsselpaar benutzen darf, ergab sich bisher aus dem
/// Code statt aus einer Entscheidung. Sie ist gefallen: **ein Schlüssel,
/// aber die Rolle wird mitsigniert.** Das erreicht dasselbe wie getrennte
/// Schlüssel, ohne die Schlüsselverwaltung zu verdreifachen, und drei
/// Schlüssel je Teilnehmer wären drei Wege, einen zu verlieren.
///
/// **Warum jetzt.** Das Drahtformat des Pods hat heute keinen externen
/// Nutzer: Es läuft nur zwischen Prozessen, die zusammen gebaut werden.
/// Nach dem ersten Partnerlauf wäre daraus eine Protokolländerung mit
/// Abstimmungsbedarf geworden. Dieselbe Begründung wie bei der
/// Latenz-EMA (Fund 44): Der billigste Zeitpunkt ist der, an dem noch
/// niemand darauf zeigt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct TransitionSig {
    pub segment_id: SegmentId,
    pub shard_index: u64,
    pub position: u64,
    pub prev_hash: [u8; 32],
    pub next_hash: [u8; 32],
}

impl TransitionSig {
    /// Die zu signierenden Bytes: `DST ‖ Rolle ‖ Borsh(self)`.
    ///
    /// Feste Feldbreiten in fester Reihenfolge, also präfixfrei und
    /// eindeutig dekodierbar — dieselbe Bauart wie
    /// `myl_consensus::signing::signable_bytes`.
    pub fn to_sign_bytes_mit_rolle(&self, rolle: Rolle) -> Vec<u8> {
        let borsh_bytes = borsh::to_vec(self).expect("TransitionSig ist stets serialisierbar");
        let mut msg = Vec::with_capacity(DST_SHARD_TRANSITION.len() + 1 + borsh_bytes.len());
        msg.extend_from_slice(DST_SHARD_TRANSITION);
        msg.push(rolle.byte());
        msg.extend_from_slice(&borsh_bytes);
        msg
    }

    /// Die Bytes in der Rolle [`Rolle::Shard`], dem Normalfall.
    pub fn to_sign_bytes(&self) -> Vec<u8> {
        self.to_sign_bytes_mit_rolle(Rolle::Shard)
    }

    /// Signiert den Übergang mit dem BLS-Schlüssel in der Rolle `Shard`.
    pub fn sign(&self, sk: &BlsSecretKey) -> Result<BlsSignature, String> {
        sk.sign(&self.to_sign_bytes()).map_err(|e| e.to_string())
    }

    /// Verifiziert die Übergangs-Signatur gegen den öffentlichen
    /// Schlüssel des Shards.
    pub fn verify(&self, pk: &BlsPublicKey, sig: &BlsSignature) -> bool {
        pk.verify(&self.to_sign_bytes(), sig)
    }

    /// Verifiziert gegen eine ausdrücklich genannte Rolle.
    pub fn verify_mit_rolle(&self, pk: &BlsPublicKey, sig: &BlsSignature, rolle: Rolle) -> bool {
        pk.verify(&self.to_sign_bytes_mit_rolle(rolle), sig)
    }
}

/// Null-Hash (vorheriger Spur-Eintrag für Shard 0).
pub const ZERO_HASH: [u8; 32] = [0u8; 32];

/// Domain-Separation-Präfix der Shard-Übergangssignatur.
///
/// Additiv wie `DST_PROPOSE_POL` in `myl-consensus`: ein eigenes Präfix
/// statt einer Erweiterung einer bestehenden Kodierung.
pub const DST_SHARD_TRANSITION: &[u8] = b"MYELITH_SHARD_TRANSITION_v1";

/// Die Rolle, in der ein Schlüssel unterschreibt.
///
/// **Ein Schlüssel je Teilnehmer, aber die Rolle wird mitsigniert.** Eine
/// in einer Rolle abgegebene Signatur gilt damit in keiner anderen, und
/// zwar durch Konstruktion und nicht durch die Länge der Botschaft.
///
/// Die Kodierung ist ein einzelnes Byte in fester Zuordnung; sie ist Teil
/// des Konsensvertrags und darf nicht umnummeriert werden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum Rolle {
    /// Rechnet eine Layer-Gruppe eines Segments.
    Shard = 1,
    /// Bestätigt als Mitglied ein PoI-Bündel seines Pods.
    PodMitglied = 2,
    /// Stimmt im BFT-Komitee ab.
    Validator = 3,
    /// Rechnet ein Segment als Stichprobe nach.
    Checker = 4,
}

impl Rolle {
    /// Das Byte, das in die Signierbotschaft geht.
    pub fn byte(&self) -> u8 {
        *self as u8
    }
}

/// Prüft, dass der Hash der empfangenen Aktivierungen zum letzten
/// Spur-Eintrag passt (Manipulationserkennung). Liefert `true`, wenn die
/// Spur leer ist (Shard 0 empfängt Token, keine Aktivierungen) oder der
/// Hash übereinstimmt.
pub fn verify_input_hash(activations: &[i16], trace: &[[u8; 32]]) -> bool {
    match trace.last() {
        // **Eine leere Spur belegt nichts (Fund 41, 2026-08-23).**
        //
        // Bis dahin stand hier `true`, mit der Begründung „Shard 0: noch
        // kein Spur-Eintrag, Token-Eingang". Für Shard 0 stimmt das,
        // aber dieser Zweig wird von dort **gar nicht erreicht**: Der
        // Token-Eingang kehrt in `ShardNode::process` vorher zurück.
        //
        // Erreicht wird er nur auf dem Aktivierungspfad, und dort heißt
        // eine leere Spur: Jemand schickt Aktivierungen ohne jeden
        // Nachweis, woher sie kommen. Die Prüfung ging **vacuously**
        // durch, der Shard rechnete auf fremden Zahlen weiter, und bei
        // unpassender Länge endete das in einer Panik im Kernel, also in
        // einem Absturz, den jeder auslösen kann, der Bytes schicken
        // darf.
        //
        // Gefunden vom adversarialen Test, beim ersten Lauf.
        None => false,
        Some(expected) => activation_hash(activations) == *expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_hash_deterministisch_und_empfindlich() {
        let a = [1i16, 2, 3, -4, 32767, -32768];
        let h1 = activation_hash(&a);
        let h2 = activation_hash(&a);
        assert_eq!(h1, h2);
        // Ein verändertes Byte ⇒ anderer Hash.
        let mut b = a;
        b[3] ^= 1;
        assert_ne!(activation_hash(&b), h1);
        // Leer ⇒ definierter Hash.
        let leer = activation_hash(&[]);
        assert_eq!(leer, activation_hash(&[]));
    }

    #[test]
    fn uebergang_signieren_und_verifizieren() {
        let sk = BlsSecretKey::key_gen(&[0x42u8; 32]).expect("KeyGen");
        let pk = sk.public_key().expect("pk");
        let t = TransitionSig {
            segment_id: SegmentId::new([1u8; 32]),
            shard_index: 1,
            position: 3,
            prev_hash: [2u8; 32],
            next_hash: [3u8; 32],
        };
        let sig = t.sign(&sk).expect("Signieren");
        assert!(t.verify(&pk, &sig));
        // Manipulierter Übergang ⇒ Verifikation scheitert.
        let mut t2 = t.clone();
        t2.next_hash = [4u8; 32];
        assert!(!t2.verify(&pk, &sig));
        // Falscher Schlüssel ⇒ scheitert.
        let fremd = BlsSecretKey::key_gen(&[0x99u8; 32]).expect("KeyGen");
        let fremd_pk = fremd.public_key().expect("pk");
        assert!(!t.verify(&fremd_pk, &sig));
    }

    #[test]
    fn eingangs_hash_pruefung() {
        let akt = [10i16, 20, 30];
        let h = activation_hash(&akt);
        // Passender Spur-Eintrag ⇒ ok.
        assert!(verify_input_hash(&akt, &[h]));
        // Leere Spur (Shard 0) ⇒ ok.
        // Fund 41: Eine leere Spur belegt nichts. Auf dem
        // Aktivierungspfad, dem einzigen, der hier ankommt, heisst sie
        // "ohne Nachweis geschickt".
        assert!(!verify_input_hash(&akt, &[]));
        // Verfälschte Aktivierungen ⇒ abgelehnt.
        let mut manipuliert = akt;
        manipuliert[0] = 99;
        assert!(!verify_input_hash(&manipuliert, &[h]));
        // Falscher Spur-Eintrag ⇒ abgelehnt.
        assert!(!verify_input_hash(&akt, &[[0xAAu8; 32]]));
    }
}

#[cfg(test)]
mod rollen_tests {
    use super::*;
    use myl_types::ids::SegmentId;

    fn sig() -> TransitionSig {
        TransitionSig {
            segment_id: SegmentId::new([1u8; 32]),
            shard_index: 3,
            position: 7,
            prev_hash: [2u8; 32],
            next_hash: [3u8; 32],
        }
    }

    /// **Die Botschaft trägt das Präfix im Klartext.**
    ///
    /// Der Test liest die ersten Bytes, statt nur eine Länge zu prüfen:
    /// Ein Präfix, das da ist, aber nicht das erwartete, wäre genauso
    /// wirkungslos wie keines.
    #[test]
    fn die_botschaft_beginnt_mit_dem_praefix() {
        let bytes = sig().to_sign_bytes();
        assert!(bytes.starts_with(DST_SHARD_TRANSITION));
        assert_eq!(bytes[DST_SHARD_TRANSITION.len()], Rolle::Shard.byte());
    }

    /// **Keine Kollision mehr mit einer anderen Klasse, und zwar durch
    /// Konstruktion.**
    ///
    /// Vorher hing das an der Länge: 112 Bytes, und keine andere Klasse
    /// war 112 Bytes lang. Jetzt beginnt die Botschaft mit einem Präfix,
    /// das keine andere Klasse trägt, und das gilt unabhängig davon, wie
    /// lang sie ist. Der Test prüft es gegen die BFT-Präfixe.
    #[test]
    fn kein_praefix_einer_anderen_klasse_passt() {
        let bytes = sig().to_sign_bytes();
        for fremd in [
            &b"MYELITH_BFT_PROPOSE_v1"[..],
            &b"MYELITH_BFT_VOTE_v1"[..],
            &b"MYELITH_BFT_COMMIT_v1"[..],
            &b"MYELITH_BFT_PROPOSE_POL_v1"[..],
            &b"MYELITH_POI_BUNDLE_v1"[..],
        ] {
            assert!(
                !bytes.starts_with(fremd),
                "die Botschaft beginnt wie eine andere Klasse"
            );
        }
    }

    /// **Eine Signatur in einer Rolle gilt in keiner anderen.**
    ///
    /// Das ist die Aussage der Rollenbindung: Ein Miner benutzt einen
    /// Schlüssel für Shard-Arbeit, PoI-Bündel und möglicherweise
    /// Validator-Stimmen. Ohne Rolle in der Botschaft ließe sich eine in
    /// der einen Rolle abgegebene Signatur in der anderen einsetzen.
    #[test]
    fn eine_rolle_gilt_nicht_in_einer_anderen() {
        let sk = BlsSecretKey::key_gen(&[5u8; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let t = sig();

        let als_shard = sk.sign(&t.to_sign_bytes_mit_rolle(Rolle::Shard)).expect("sign");
        assert!(t.verify_mit_rolle(&pk, &als_shard, Rolle::Shard));

        for andere in [Rolle::PodMitglied, Rolle::Validator, Rolle::Checker] {
            assert!(
                !t.verify_mit_rolle(&pk, &als_shard, andere),
                "eine Shard-Signatur darf in der Rolle {andere:?} nicht gelten"
            );
        }
    }

    /// Die Rollen-Bytes sind eindeutig und dürfen nicht umnummeriert
    /// werden: Sie sind Teil des Konsensvertrags.
    #[test]
    fn die_rollenbytes_sind_eindeutig_und_fest() {
        assert_eq!(Rolle::Shard.byte(), 1);
        assert_eq!(Rolle::PodMitglied.byte(), 2);
        assert_eq!(Rolle::Validator.byte(), 3);
        assert_eq!(Rolle::Checker.byte(), 4);
        let bytes: Vec<u8> = [Rolle::Shard, Rolle::PodMitglied, Rolle::Validator, Rolle::Checker]
            .iter()
            .map(|r| r.byte())
            .collect();
        let mut sortiert = bytes.clone();
        sortiert.sort_unstable();
        sortiert.dedup();
        assert_eq!(sortiert.len(), bytes.len());
    }
}
