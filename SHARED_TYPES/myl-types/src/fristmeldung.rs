//! **Die Fristmeldung**: Ein Mitglied rechnet seinen Bereich langsamer, als
//! es zugesagt hat (Stufe 4 des Dispatchers, Festlegung des
//! Projektinhabers vom 2026-10-22).
//!
//! # ⚑ Warum es sie geben muss
//!
//! Wer seine Ebenenzeit zu niedrig ansagt, bekommt mehr Ebenen und damit
//! mehr Gewichtsarbeit gutgeschrieben, als er in der Zeit leistet. Die
//! angesagte Zeit gilt deshalb als **Frist**
//! ([`crate::zusage::FRIST_FAKTOR`]): Wer sie überschreitet, verliert seine
//! Zusage für die nächste Epoche und bekommt einen Verstoß.
//!
//! # ⚑ Jeder Zeuge misst selbst
//!
//! Eine Meldung wirkt erst mit den Zeugnissen einer **Mehrheit der übrigen
//! Mitglieder** ([`crate::gegenzeichnung::mindestzeichner`]), und jedes
//! Zeugnis ist eine **eigene Messung** des Zeugen: Er hat dem Gemeldeten
//! eine Zeitprobe gestellt, die Zeit genommen und unterschreibt die Zahl.
//! Ein Zeuge, der eine fremde Zahl unterschriebe, bezeugte nichts. So
//! trifft ein einzelnes bösartiges Mitglied niemanden, und die Kette prüft
//! jede Zahl gegen die Frist, statt einer Behauptung zu glauben.
//!
//! # Was sie nicht leistet
//!
//! **Gegen eine bösartige Mehrheit des Pods hilft sie nicht**, wie die
//! Ausfallmeldung. Und die Messung enthält den Weg durch das Netz; die
//! Frist ist das Doppelte der Zusage, damit eine gewöhnliche Strecke nicht
//! zum Verstoß wird.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::ids::{EpochId, MinerId};
use crate::uebergang::Rolle;

/// Trennstring eines Fristzeugnisses.
pub const DST_FRISTZEUGNIS: &[u8] = b"MYELITH_FRISTZEUGNIS_v1";

/// Höchstens so viele Zeugnisse in einer Meldung: ein Pod hat nicht mehr
/// Mitglieder (gegen aufgeblähte Blöcke).
pub const ZEUGNISSE_HOECHSTENS: usize = 32;

/// Die Messung eines Zeugen.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Fristzeugnis {
    /// Wer gemessen hat.
    pub zeuge: MinerId,
    /// Wie lange der Bereich des Gemeldeten je Token brauchte, samt Kopf,
    /// wenn er ihn trägt, in Mikrosekunden.
    pub gemessen_us: u64,
    /// Unterschrift des Zeugen in der Rolle [`Rolle::PodMitglied`].
    pub signatur: BlsSignature,
}

/// Die Meldung über ein Mitglied eines Pods.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Fristmeldung {
    /// Die Epoche der Messung; die Kette nimmt nur die laufende.
    pub epoche: EpochId,
    /// Die Platznummer des Pods in der Zuteilung dieser Epoche.
    pub pod: u32,
    /// Wer seine Frist überschritten haben soll.
    pub gemeldeter: MinerId,
    /// Die Messungen der Zeugen.
    pub zeugnisse: Vec<Fristzeugnis>,
}

/// Die Bytes, über die ein Zeuge unterschreibt: Trennstring, Rolle, dann
/// Epoche, Pod, Gemeldeter und seine Zahl.
pub fn zeugnisbotschaft(epoche: EpochId, pod: u32, gemeldeter: &MinerId, gemessen_us: u64) -> Vec<u8> {
    let rumpf = borsh::to_vec(&(epoche, pod, gemeldeter, gemessen_us))
        .expect("feste Feldbreiten sind stets serialisierbar");
    let mut msg = Vec::with_capacity(DST_FRISTZEUGNIS.len() + 1 + rumpf.len());
    msg.extend_from_slice(DST_FRISTZEUGNIS);
    msg.push(Rolle::PodMitglied.byte());
    msg.extend_from_slice(&rumpf);
    msg
}

impl Fristzeugnis {
    /// Ein unterschriebenes Zeugnis.
    pub fn neu(
        sk: &BlsSecretKey,
        zeuge: MinerId,
        epoche: EpochId,
        pod: u32,
        gemeldeter: &MinerId,
        gemessen_us: u64,
    ) -> Result<Self, crate::bls::BlsError> {
        let signatur = sk.sign(&zeugnisbotschaft(epoche, pod, gemeldeter, gemessen_us))?;
        Ok(Self { zeuge, gemessen_us, signatur })
    }

    /// Stammt das Zeugnis von seinem Zeugen, über diese Meldung?
    ///
    /// ⚑ **Schlüssel und Kennung zugleich**, nach dem Muster von Fund 96.
    pub fn ist_vom_zeugen(&self, pk: &BlsPublicKey, meldung: &Fristmeldung) -> bool {
        MinerId::aus_schluessel(pk) == self.zeuge
            && pk.verify(
                &zeugnisbotschaft(meldung.epoche, meldung.pod, &meldung.gemeldeter, self.gemessen_us),
                &self.signatur,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sk(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b.wrapping_add(3); 32]).expect("Schlüssel")
    }

    /// ⚑ **Ein Zeugnis gilt nur für seine Meldung und seinen Zeugen.**
    #[test]
    fn ein_zeugnis_gilt_nur_fuer_seine_meldung() {
        let zeuge = sk(1);
        let pk = zeuge.public_key().expect("pk");
        let gemeldeter = MinerId::new([9; 32]);
        let z = Fristzeugnis::neu(&zeuge, MinerId::aus_schluessel(&pk), EpochId(4), 2, &gemeldeter, 5000)
            .expect("Zeugnis");
        let meldung = Fristmeldung { epoche: EpochId(4), pod: 2, gemeldeter, zeugnisse: vec![z.clone()] };
        assert!(z.ist_vom_zeugen(&pk, &meldung));
        let anderer_pod = Fristmeldung { pod: 3, ..meldung.clone() };
        assert!(!z.ist_vom_zeugen(&pk, &anderer_pod), "anderer Pod");
        let andere_zahl = Fristzeugnis { gemessen_us: 9000, ..z.clone() };
        assert!(!andere_zahl.ist_vom_zeugen(&pk, &meldung), "nachträglich erhöht");
        let fremd = sk(2).public_key().expect("pk");
        assert!(!z.ist_vom_zeugen(&fremd, &meldung), "fremder Schlüssel");
    }
}
