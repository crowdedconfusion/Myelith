//! Streit-Artefakte der Verifikation — Whitepaper Kap. 6.4–6.6, Anhang A.4.
//!
//! Eine `Challenge` ist das On-Chain-Artefakt, mit dem ein Checker eine
//! Abweichung zwischen zwei redundanten Pods anzeigt und das
//! Bisektions-Spiel eröffnet. Sie entsteht in VERIFICATION, wird über
//! NETWORKING verbreitet und landet im Block (CONSENSUS).
//!
//! **Warum der Typ hier liegt:** Er wird von drei Komponenten benutzt,
//! die einander nicht kennen dürfen — VERIFICATION erzeugt ihn,
//! NETWORKING validiert ihn beim Gossip, CONSENSUS nimmt ihn in den
//! Block auf. Läge er in einer dieser Komponenten, müsste die
//! Schichtung verletzt werden (L0 Networking hinge an L1 Consensus).
//! Bis v0.2.4 existierten stattdessen **zwei** unabhängige
//! `Challenge`-Definitionen: eine in `myl-verifier` (mit beiden Pods und
//! beiden Hashes) und eine schmalere in `myl-consensus::block` — der
//! Block konnte also gar nicht aufnehmen, was der Verifier produziert.
//!
//! **Konsens-Feld:** Die Kodierung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::hash::Hash;
use crate::ids::{MinerId, SegmentId};
use crate::uebergang::Rolle;
use borsh::{BorshDeserialize, BorshSerialize};

/// Domain-Separation-Präfix der Anfechtung.
///
/// Wie jede andere Signaturklasse des Projekts trägt auch diese ihr
/// Präfix im Klartext (`MYELITH_BFT_VOTE_v1`, `MYELITH_POI_BUNDLE_v1`
/// und so fort). Ohne das hinge der Schutz gegen eine Verwechslung an
/// der Länge der Botschaft, und ein Längenzufall verschwindet still,
/// sobald jemand ein Feld ändert.
pub const DST_CHALLENGE: &[u8] = b"MYELITH_CHALLENGE_v1";

/// Eine Challenge: Anzeige einer Abweichung, Start des Bisektions-Spiels.
///
/// Enthält beide Seiten des Streits — ohne den Hash der Gegenseite wäre
/// die Anzeige nicht nachprüfbar und die Schuldzuweisung nicht eindeutig
/// (Kap. 6.6: „Die Schuldzuweisung ist eindeutig, weil das Ergebnis
/// kanonisch ist").
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Challenge {
    /// ID des betroffenen Segments.
    pub segment_id: SegmentId,
    /// Index der ersten abweichenden Spur-Position (0-basiert).
    pub first_divergence: usize,
    /// Miner des primären Pods (Angeklagter).
    pub primary_miner: MinerId,
    /// Miner des redundanten Pods (Checker).
    pub redundant_miner: MinerId,
    /// Commitment-Hash des primären Pods an der abweichenden Position.
    pub primary_hash: Hash,
    /// Commitment-Hash des redundanten Pods an der abweichenden Position.
    pub redundant_hash: Hash,
    /// Zeitstempel der Challenge-Erzeugung (Unix-Millisekunden).
    pub timestamp_ms: u64,
    /// BLS-Signatur des Herausforderers über [`Challenge::signierbotschaft`].
    ///
    /// # ⚑ Warum das Feld seit dem 2026-08-29 da ist (Fund 96, zweite Hälfte)
    ///
    /// Die Anfechtung nannte `primary_miner` und `redundant_miner` als
    /// **Felder**, und nichts band einen der beiden an denjenigen, der
    /// die Anfechtung einreichte. Dieselbe Gestalt wie Fund 85 im Ledger
    /// und wie die Slash-Entscheidung davor: Wer anzeigt, bestimmte, wen
    /// er anzeigt und in wessen Namen.
    ///
    /// Das zählt, weil eine Anfechtung Kosten verursacht: Der
    /// Angeklagte muss antworten, und nach der Umstellung des
    /// Beweisarchivs auf Nachrechnen heißt das, eine ganze Folge neu zu
    /// rechnen. Ohne Bindung wäre das ein Hebel zum Schikanieren, den
    /// jeder ohne Einsatz ziehen kann.
    ///
    /// Unterschrieben wird in der Rolle [`Rolle::Checker`] und in keiner
    /// anderen; eine Unterschrift, die derselbe Miner als Shard oder als
    /// Pod-Mitglied abgegeben hat, gilt hier nicht.
    pub signature: BlsSignature,
}

impl Challenge {
    /// Die zu signierenden Bytes: `DST ‖ Rolle ‖ Borsh(Felder ohne Signatur)`.
    ///
    /// Die Signatur selbst geht **nicht** ein, sonst wäre sie Teil
    /// dessen, was sie deckt (Zirkelbezug). Alle übrigen Felder gehen
    /// ein, auch der Zeitstempel: Wer ihn nachträglich verschöbe,
    /// bekäme eine Anfechtung mit fremder Unterschrift.
    pub fn signierbotschaft(&self) -> Vec<u8> {
        let kern = (
            self.segment_id,
            self.first_divergence as u64,
            self.primary_miner,
            self.redundant_miner,
            self.primary_hash,
            self.redundant_hash,
            self.timestamp_ms,
        );
        let rumpf = borsh::to_vec(&kern).expect("feste Feldbreiten sind stets serialisierbar");
        let mut msg = Vec::with_capacity(DST_CHALLENGE.len() + 1 + rumpf.len());
        msg.extend_from_slice(DST_CHALLENGE);
        msg.push(Rolle::Checker.byte());
        msg.extend_from_slice(&rumpf);
        msg
    }

    /// Unterschreibt die Anfechtung.
    pub fn signiere(&mut self, sk: &BlsSecretKey) -> Result<(), crate::bls::BlsError> {
        self.signature = sk.sign(&self.signierbotschaft())?;
        Ok(())
    }

    /// Prüft die Unterschrift und die Zuordnung in einem.
    ///
    /// ⚑ **Beides zusammen, nicht getrennt.** Eine gültige Unterschrift
    /// unter einer Anfechtung, die einen anderen als Herausforderer
    /// nennt, ist wertlos: Sie belegt, dass *jemand* unterschrieben hat,
    /// nicht dass der Genannte es war. Die Kennung wird aus dem
    /// Schlüssel abgeleitet, also lässt sich beides in einem Schritt
    /// prüfen, und getrennte Prüfungen könnten getrennt vergessen
    /// werden.
    ///
    /// Kein `Result`: Der Aufrufer verwirft in jedem Fehlerfall, und
    /// eine Unterscheidung verführte nur zu einer Verzweigung, die
    /// niemand braucht.
    pub fn ist_vom_herausforderer(&self, pk: &BlsPublicKey) -> bool {
        MinerId::aus_schluessel(pk) == self.redundant_miner
            && pk.verify(&self.signierbotschaft(), &self.signature)
    }

    /// Strukturelle Plausibilitätsprüfung ohne Kenntnis der Spur.
    ///
    /// Prüft, was ohne weiteren Kontext entscheidbar ist: Die beiden
    /// Pods müssen verschieden sein, und die angezeigten Hashes müssen
    /// tatsächlich abweichen — sonst gibt es nichts zu streiten. Das ist
    /// bewusst **keine** vollständige Gültigkeitsprüfung; die verlangt
    /// die Segment-Spur und findet in VERIFICATION statt.
    ///
    /// Für den Gossip-Layer reicht diese Stufe, um offensichtlichen
    /// Unsinn zu verwerfen, bevor er weiterverbreitet wird.
    pub fn validate_structure(&self) -> Result<(), ChallengeStructureError> {
        if self.primary_miner == self.redundant_miner {
            return Err(ChallengeStructureError::IdenticalMiners);
        }
        if self.primary_hash == self.redundant_hash {
            return Err(ChallengeStructureError::IdenticalHashes);
        }
        Ok(())
    }
}

/// Fehler der strukturellen Challenge-Prüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeStructureError {
    /// Primärer und redundanter Pod sind derselbe Miner.
    IdenticalMiners,
    /// Beide Hashes sind gleich — es liegt keine Abweichung vor.
    IdenticalHashes,
}

impl core::fmt::Display for ChallengeStructureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IdenticalMiners => write!(f, "Primärer und redundanter Miner sind identisch"),
            Self::IdenticalHashes => write!(f, "Beide Commitment-Hashes sind gleich"),
        }
    }
}

impl std::error::Error for ChallengeStructureError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn challenge() -> Challenge {
        Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: miner(1),
            redundant_miner: miner(2),
            primary_hash: Hash::sha256(b"a"),
            redundant_hash: Hash::sha256(b"b"),
            timestamp_ms: 1_700_000_000_000,
            signature: BlsSignature([0u8; 96]),
        }
    }

    #[test]
    fn gueltige_challenge() {
        assert!(challenge().validate_structure().is_ok());
    }

    #[test]
    fn gleiche_miner_werden_abgelehnt() {
        let mut c = challenge();
        c.redundant_miner = c.primary_miner;
        assert_eq!(
            c.validate_structure(),
            Err(ChallengeStructureError::IdenticalMiners)
        );
    }

    #[test]
    fn gleiche_hashes_werden_abgelehnt() {
        let mut c = challenge();
        c.redundant_hash = c.primary_hash;
        assert_eq!(
            c.validate_structure(),
            Err(ChallengeStructureError::IdenticalHashes)
        );
    }

    #[test]
    fn borsh_rundtrip() {
        let c = challenge();
        let bytes = borsh::to_vec(&c).unwrap();
        assert_eq!(borsh::from_slice::<Challenge>(&bytes).unwrap(), c);
    }
    // ── Die Unterschrift (⚑ Fund 96, zweite Haelfte) ────────────────

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel")
    }

    /// Eine Anfechtung, die der genannte Herausforderer wirklich
    /// unterschrieben hat.
    fn signierte(b: u8) -> (Challenge, BlsPublicKey) {
        let sk = schluessel(b);
        let pk = sk.public_key().expect("Punkt");
        let mut c = challenge();
        c.redundant_miner = MinerId::aus_schluessel(&pk);
        c.signiere(&sk).expect("signieren");
        (c, pk)
    }

    #[test]
    fn eine_unterschriebene_anfechtung_gilt() {
        let (c, pk) = signierte(1);
        assert!(c.ist_vom_herausforderer(&pk));
    }

    /// ⚑ **Der Kern von Fund 96:** Ohne Unterschrift bindet die
    /// Anfechtung niemanden. Vorher gab es das Feld nicht, und damit war
    /// jede Anfechtung in diesem Zustand.
    #[test]
    fn eine_unsignierte_anfechtung_bindet_niemanden() {
        let sk = schluessel(1);
        let pk = sk.public_key().expect("Punkt");
        let mut c = challenge();
        c.redundant_miner = MinerId::aus_schluessel(&pk);
        assert!(!c.ist_vom_herausforderer(&pk));
    }

    /// ⚑ Eine gültige Unterschrift unter einer Anfechtung, die einen
    /// **anderen** als Herausforderer nennt, gilt nicht. Sonst belegte
    /// sie nur, dass irgendjemand unterschrieben hat.
    #[test]
    fn eine_unterschrift_auf_fremden_namen_gilt_nicht() {
        let (mut c, pk) = signierte(1);
        c.redundant_miner = miner(200);
        assert!(!c.ist_vom_herausforderer(&pk));
    }

    /// Jedes Feld ist gedeckt: Wer nachträglich etwas verschiebt,
    /// bekommt eine Anfechtung, deren Unterschrift nicht mehr passt.
    #[test]
    fn jede_aenderung_bricht_die_unterschrift() {
        let (c, pk) = signierte(1);
        let mut zeit = c.clone();
        zeit.timestamp_ms += 1;
        assert!(!zeit.ist_vom_herausforderer(&pk), "Zeitstempel");

        let mut pos = c.clone();
        pos.first_divergence += 1;
        assert!(!pos.ist_vom_herausforderer(&pk), "Position");

        let mut wen = c.clone();
        wen.primary_miner = miner(77);
        assert!(!wen.ist_vom_herausforderer(&pk), "Angeklagter");

        let mut h = c.clone();
        h.primary_hash = Hash::sha256(b"anders");
        assert!(!h.ist_vom_herausforderer(&pk), "Hash");
    }

    /// ⚑ **Eine Unterschrift aus einer anderen Rolle gilt nicht.**
    ///
    /// Derselbe Miner unterschreibt als Shard seine Übergänge. Ohne
    /// Rollenbindung ließe sich eine davon als Anfechtung einsetzen.
    #[test]
    fn eine_unterschrift_aus_anderer_rolle_gilt_nicht() {
        let sk = schluessel(1);
        let pk = sk.public_key().expect("Punkt");
        let mut c = challenge();
        c.redundant_miner = MinerId::aus_schluessel(&pk);
        // Dieselbe Botschaft, nur mit dem Rollenbyte des Shards.
        let mut fremde = c.signierbotschaft();
        let stelle = DST_CHALLENGE.len();
        fremde[stelle] = Rolle::Shard.byte();
        c.signature = sk.sign(&fremde).expect("signieren");
        assert!(!c.ist_vom_herausforderer(&pk));
    }

    /// Die Botschaft trägt das Präfix im Klartext, geprüft an den Bytes
    /// und nicht an einer Länge: Ein Präfix, das da ist, aber nicht das
    /// erwartete, wäre genauso wirkungslos wie keines.
    #[test]
    fn die_botschaft_beginnt_mit_dem_praefix() {
        let msg = challenge().signierbotschaft();
        assert!(msg.starts_with(DST_CHALLENGE));
        assert_eq!(msg[DST_CHALLENGE.len()], Rolle::Checker.byte());
    }

}
