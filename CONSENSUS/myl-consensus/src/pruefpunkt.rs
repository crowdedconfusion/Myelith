//! **Prüfpunkte**: Endgültigkeit alle [`BLOECKE_JE_PRUEFPUNKT`] Blöcke
//! (Festlegung des Projektinhabers vom 2026-10-27).
//!
//! # ⚑ Was hier steht und warum so
//!
//! Das Ziel ist das Whitepaper (Kap. 3.5.2: BFT je Block, HotStuff-Familie,
//! der Erzeuger rotiert per VRF). Stufe 1 sind Prüfpunkte: Der Block an jeder
//! Höhe `150 · n` wird vom Stimmsatz bestätigt, und das Zertifikat geht in
//! einen späteren Block ([`crate::block::BlockHeader::pruefpunkt`]).
//!
//! **Keine Runden, kein Leader** (Festlegung vom selben Tag, nach Casper FFG
//! und GRANDPA): Der Block eines Prüfpunkts steht schon fest, es gibt nichts
//! vorzuschlagen. Jedes Mitglied unterschreibt (Höhe, Blockhash) des Blocks,
//! den es an dieser Höhe hat ([`Pruefpunktstimme`]); sobald mehr als zwei
//! Drittel des Gewichts beisammen sind, kann jeder daraus ein
//! [`Pruefpunktzertifikat`] bauen.
//!
//! **Sicherheit:** Zwei Zertifikate für dieselbe Höhe mit verschiedenen
//! Hashes brauchen mindestens ein Drittel Gewicht, das beide unterschrieben
//! hat, und eine solche Doppelstimme ist ein schlachtbarer Beleg. Unterschreibt
//! der Erzeuger zwei Blöcke derselben Höhe, stimmen die Mitglieder gespalten
//! ab, und es entsteht kein Zertifikat: Das kostet Endgültigkeit, nicht
//! Sicherheit.
//!
//! **Trennung von den BFT-Nachrichten:** eigenes Trennzeichen
//! [`DST_PRUEFPUNKT`], und die Höhe steht in der Botschaft. Eine Stimme oder
//! ein Commit der Runden lässt sich nicht als Prüfpunktstimme ausgeben und
//! umgekehrt.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsAggregateSignature, BlsSecretKey, BlsSignature, aggregate_signatures};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

use crate::round_change::RoundError;
use crate::validator::VotingSet;

/// Alle wie viele Blöcke ein Prüfpunkt liegt: 150, also fünf Minuten bei
/// 2 s Blockzeit, zwölf je Epoche.
///
/// ⚑ **Ein Konsenswert wie die Epochenlänge**: Wer ihn ändert, ändert den
/// Vertrag. Er teilt die Epoche, damit jede Epochengrenze ein Prüfpunkt ist;
/// an ihr wechselt der Stimmsatz, und aus ihrem Zertifikat kommt die Saat.
pub const BLOECKE_JE_PRUEFPUNKT: u64 = 150;

const _: () = assert!(crate::block::BLOECKE_JE_EPOCHE.is_multiple_of(BLOECKE_JE_PRUEFPUNKT));

/// Trennzeichen einer Prüfpunktstimme.
pub const DST_PRUEFPUNKT: &[u8] = b"MYELITH_PRUEFPUNKT_v1";

/// Ist diese Höhe ein Prüfpunkt? Genesis (Höhe 0) nicht.
pub fn ist_pruefpunkt(hoehe: u64) -> bool {
    hoehe > 0 && hoehe.is_multiple_of(BLOECKE_JE_PRUEFPUNKT)
}

/// Die Botschaft, über die ein Mitglied unterschreibt:
/// `DST_PRUEFPUNKT ‖ u64_le(hoehe) ‖ block_hash`.
pub fn pruefpunktnachricht(hoehe: u64, block_hash: &Hash) -> Vec<u8> {
    crate::signing::signable_bytes(DST_PRUEFPUNKT, hoehe, block_hash)
}

/// Die Stimme eines Mitglieds für den Block an einem Prüfpunkt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefpunktstimme {
    /// Die Höhe des Prüfpunkts.
    pub hoehe: u64,
    /// Der Hash des Blocks an dieser Höhe, wie der Wähler ihn hat.
    pub block_hash: Hash,
    /// Wer stimmt.
    pub waehler: MinerId,
    /// Unterschrift über [`pruefpunktnachricht`].
    pub signatur: BlsSignature,
}

impl Pruefpunktstimme {
    /// Unterschreibt den Block an `hoehe`.
    pub fn unterschreiben(
        hoehe: u64,
        block_hash: Hash,
        waehler: MinerId,
        geheim: &BlsSecretKey,
    ) -> Result<Self, RoundError> {
        let signatur = geheim
            .sign(&pruefpunktnachricht(hoehe, &block_hash))
            .map_err(|_| RoundError::InvalidSignature)?;
        Ok(Self { hoehe, block_hash, waehler, signatur })
    }

    /// Trägt die Stimme? Ein Prüfpunkt, ein Mitglied, und die Unterschrift
    /// unter dem Schlüssel, den der Satz für es führt.
    pub fn traegt(&self, satz: &VotingSet) -> bool {
        ist_pruefpunkt(self.hoehe)
            && satz
                .pubkey(&self.waehler)
                .is_some_and(|pk| pk.verify(&pruefpunktnachricht(self.hoehe, &self.block_hash), &self.signatur))
    }
}

/// **Der Beleg, dass mehr als zwei Drittel des Stimmsatzes den Block an
/// einem Prüfpunkt bestätigt haben.**
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefpunktzertifikat {
    /// Die Höhe des Prüfpunkts.
    pub hoehe: u64,
    /// Der bestätigte Block.
    pub block_hash: Hash,
    /// Unterzeichner, streng aufsteigend (kanonisch, ohne Doppelte).
    pub unterzeichner: Vec<MinerId>,
    /// Aggregierte BLS-Signatur aller Unterzeichner.
    pub aggregat: BlsAggregateSignature,
}

impl Pruefpunktzertifikat {
    /// Baut das Zertifikat aus Stimmen derselben Höhe und desselben Hashes.
    ///
    /// Geprüft wird hier nur die Form; Gewicht und Unterschriften prüft
    /// [`Self::verify`], beim Empfänger wie beim Bauenden.
    pub fn aus_stimmen(stimmen: &[Pruefpunktstimme]) -> Result<Self, RoundError> {
        let erste = stimmen.first().ok_or(RoundError::EmptyCertificate)?;
        let mut paare: Vec<(MinerId, BlsSignature)> = Vec::with_capacity(stimmen.len());
        for s in stimmen {
            if s.hoehe != erste.hoehe || s.block_hash != erste.block_hash {
                return Err(RoundError::InconsistentCertificate);
            }
            paare.push((s.waehler, s.signatur));
        }
        paare.sort_by_key(|(m, _)| *m);
        if paare.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(RoundError::NonCanonicalCertificate);
        }
        let sigs: Vec<BlsSignature> = paare.iter().map(|(_, s)| *s).collect();
        let aggregat = aggregate_signatures(&sigs).map_err(|_| RoundError::InvalidSignature)?;
        Ok(Self {
            hoehe: erste.hoehe,
            block_hash: erste.block_hash,
            unterzeichner: paare.into_iter().map(|(m, _)| m).collect(),
            aggregat,
        })
    }

    /// Prüft das Zertifikat gegen den Stimmsatz: ein Prüfpunkt, kanonische
    /// Unterzeichner, alle im Satz, mehr als zwei Drittel des Gewichts, und
    /// die Sammelsignatur über die Prüfpunktbotschaft. Derselbe Kern wie bei
    /// den Zertifikaten der Runden.
    pub fn verify(&self, satz: &VotingSet) -> Result<(), RoundError> {
        if !ist_pruefpunkt(self.hoehe) {
            return Err(RoundError::InconsistentCertificate);
        }
        crate::round_change::pruefe_aggregat(
            &self.unterzeichner,
            satz,
            &pruefpunktnachricht(self.hoehe, &self.block_hash),
            &self.aggregat,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::VotingMember;

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b; 32]).expect("Schlüssel")
    }

    fn kennung(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Vier Mitglieder mit Gewicht eins.
    fn satz() -> VotingSet {
        let mut m = std::collections::BTreeMap::new();
        for b in 1..=4u8 {
            m.insert(kennung(b), VotingMember { pubkey: schluessel(b).public_key().expect("pk"), weight: 1 });
        }
        VotingSet::from_members(m)
    }

    fn stimme(b: u8, hoehe: u64, hash: Hash) -> Pruefpunktstimme {
        Pruefpunktstimme::unterschreiben(hoehe, hash, kennung(b), &schluessel(b)).expect("Stimme")
    }

    #[test]
    fn nur_vielfache_von_150_sind_pruefpunkte() {
        assert!(!ist_pruefpunkt(0));
        assert!(!ist_pruefpunkt(149));
        assert!(ist_pruefpunkt(150));
        assert!(ist_pruefpunkt(crate::block::BLOECKE_JE_EPOCHE), "jede Epochengrenze ist ein Prüfpunkt");
    }

    /// Drei von vier tragen, zwei nicht; eine fremde Stimme, eine andere
    /// Höhe oder ein Commit der Runden tragen nicht.
    #[test]
    fn mehr_als_zwei_drittel_bestaetigen_einen_pruefpunkt() {
        let h = Hash::sha256(b"Block an 150");
        let drei: Vec<_> = (1..=3).map(|b| stimme(b, 150, h)).collect();
        assert!(drei.iter().all(|s| s.traegt(&satz())));
        let z = Pruefpunktzertifikat::aus_stimmen(&drei).expect("Zertifikat");
        assert_eq!(z.verify(&satz()), Ok(()));
        // ⛔️ Gegenproben.
        let zwei = Pruefpunktzertifikat::aus_stimmen(&drei[..2]).expect("Zertifikat");
        assert_eq!(zwei.verify(&satz()), Err(RoundError::CertificateBelowQuorum));
        let mut andere_hoehe = z.clone();
        andere_hoehe.hoehe = 300;
        assert_eq!(andere_hoehe.verify(&satz()), Err(RoundError::InvalidSignature));
        let mut kein_pruefpunkt = z.clone();
        kein_pruefpunkt.hoehe = 151;
        assert!(kein_pruefpunkt.verify(&satz()).is_err());
        assert!(!stimme(9, 150, h).traegt(&satz()), "ein Fremder ist kein Mitglied");
        let mut falsch = stimme(1, 150, h);
        falsch.block_hash = Hash::sha256(b"ein anderer Block");
        assert!(!falsch.traegt(&satz()));
        // Ein Commit der BFT-Runden über denselben Hash ist keine
        // Prüfpunktstimme (eigenes Trennzeichen).
        let commit = schluessel(1).sign(&crate::signing::commit_message(150, &h)).expect("Commit");
        let als_stimme = Pruefpunktstimme { hoehe: 150, block_hash: h, waehler: kennung(1), signatur: commit };
        assert!(!als_stimme.traegt(&satz()));
    }

    #[test]
    fn stimmen_verschiedener_bloecke_ergeben_kein_zertifikat() {
        let a = stimme(1, 150, Hash::sha256(b"a"));
        let b = stimme(2, 150, Hash::sha256(b"b"));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[a.clone(), b]), Err(RoundError::InconsistentCertificate));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[a.clone(), a]), Err(RoundError::NonCanonicalCertificate));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[]), Err(RoundError::EmptyCertificate));
    }

    #[test]
    fn das_zertifikat_ueberlebt_borsh() {
        let h = Hash::sha256(b"x");
        let z = Pruefpunktzertifikat::aus_stimmen(&(1..=3).map(|b| stimme(b, 300, h)).collect::<Vec<_>>()).expect("Zertifikat");
        let bytes = borsh::to_vec(&z).expect("borsh");
        assert_eq!(borsh::from_slice::<Pruefpunktzertifikat>(&bytes).expect("lesen"), z);
    }
}
