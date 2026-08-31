//! Der Übergangs-Signaturvertrag zwischen Shard und Schiedsstelle.
//!
//! # ⚑ Warum das hier liegt und nicht bei dem, der es erzeugt
//!
//! Ein Shard unterschreibt jeden Übergang, den er rechnet. Die Signatur
//! ist keine Eingabeprüfung, der Empfänger prüft die Aktivierungen gegen
//! den Spur-Hash. Sie ist die **Zuschreibung**: der Beleg, dass genau
//! dieser Miner genau diesen Schritt erzeugt hat.
//!
//! Gebraucht wird dieser Beleg nicht dort, wo er entsteht, sondern dort,
//! wo geurteilt wird. Bis zum 2026-08-29 lag der Vertrag allein in
//! `myl-pod`, und `myl-verifier` hängt nicht an `myl-pod` und soll es
//! auch nicht: Daran hinge die ganze Inferenz-Laufzeit. Die Folge war,
//! dass die Signatur erzeugt, eingesammelt, aggregiert und **von
//! niemandem geprüft** wurde, während die Slash-Entscheidung den
//! Beschuldigten als Aufrufparameter entgegennahm.
//!
//! Ein gemeinsamer Vertrag gehört in die gemeinsame Kiste. Eine
//! Nachbildung auf der Richterseite wäre eine zweite Quelle für dieselbe
//! Wahrheit gewesen, und die beiden hätten sich beim ersten
//! Formatwechsel getrennt.
//!
//! **Konsens-Feld:** Präfix, Rollenbyte und Feldreihenfolge sind Teil
//! des Konsensvertrags. Änderungen nur über Governance.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::ids::SegmentId;

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
    /// Hält Gegenstände vor und weist ihre Verfügbarkeit nach.
    ///
    /// Die siebte Netzwerkrolle (Entscheidung des Projektinhabers,
    /// 2026-08-25). Sie rechnet nicht: Ein Knoten kann Wissen halten,
    /// ohne eine Token-Position zu berechnen, und wird dafür
    /// unabhängig vom Mining vergütet.
    ///
    /// **Angehängt, nicht eingefügt.** Die Zuordnung ist Teil des
    /// Konsensvertrags; die Nummern 1 bis 4 bleiben, wo sie sind.
    Store = 5,
}

impl Rolle {
    /// Das Byte, das in die Signierbotschaft geht.
    pub fn byte(&self) -> u8 {
        *self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::BlsSecretKey;
    use crate::ids::SegmentId;

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
