//! Die Treasury: ein Konto, für das es keinen Schlüssel gibt.
//!
//! # ⚑ Warum ohne Schlüssel
//!
//! Kap. 5.3 gibt der Treasury drei Prozent der Prägung, und aus ihr
//! werden das Training und die Rolle Store bezahlt. Sie muss also
//! Guthaben halten und wieder abgeben können.
//!
//! **Ein Treasury-Konto mit privatem Schlüssel wäre ein Honigtopf und
//! eine Machtposition.** ETHICS G1 nennt eine Machtposition ausdrücklich
//! als das, was in einem anonymen, offenen Netz nicht legitim besetzbar
//! ist: „Wer entscheidet, welcher Text schädlich ist, entscheidet auch,
//! welcher Text unbequem ist." Für Geld gilt dasselbe eine Stufe
//! schärfer.
//!
//! Deshalb hat diese Adresse **kein Urbild als öffentlicher Schlüssel**.
//! Eine gewöhnliche Adresse ist `SHA-256` über einen öffentlichen
//! Schlüssel; wer für die Treasury unterschreiben wollte, müsste einen
//! Schlüssel finden, dessen Hash auf diesen festen Wert fällt. Damit ist
//! **„nur das Protokoll kann sie belasten" eine Tatsache der Bauart und
//! keine Zusage**, und der Unterschied ist genau der, den G6 für die
//! Vertraulichkeit macht.
//!
//! **Das Muster ist nicht neu:** eine Adresse ohne Schlüssel, die
//! ausschliesslich Protokolllogik bewegt. Andere Ketten führen dafür
//! einen eigenen Kontotyp.
//!
//! # Was daraus folgt
//!
//! Eine Auszahlung geht **nur über einen angenommenen
//! Governance-Beschluss** (Festlegung des Projektinhabers, 2026-08-31).
//! Die Abstimmung dafür gibt es, samt Quorum, Mehrheit und einem
//! Stimmgewicht, das Stake an geleistete Arbeit bindet; **der Draht von
//! einem angenommenen Beschluss zu einer Belastung dieses Kontos fehlt
//! noch**. Bis dahin kann die Treasury Guthaben halten und keines
//! abgeben, und das ist die sichere Richtung des Fehlers.
//!
//! # ⚑ Und was sie nicht leistet
//!
//! Sie schützt **nicht** davor, dass eine Mehrheit sich selbst
//! auszahlt. Das ist keine Frage des Schlüssels, sondern des
//! Abstimmungsverfahrens, und sie steht dort.

use crate::ids::Address;

/// Trennstring der Treasury-Adresse.
pub const DST_TREASURY: &[u8] = b"MYELITH_TREASURY_v1";

/// Die Adresse der Treasury.
///
/// Fest, aus [`DST_TREASURY`] abgeleitet, ohne bekanntes
/// Schlüssel-Urbild. Sie steht damit in jeder Genesis dieselbe, ohne
/// dass sie jemand eintragen müsste.
pub fn treasury_adresse() -> Address {
    Address::new(crate::hash::Hash::sha256(DST_TREASURY).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::BlsSecretKey;

    /// Fest und wiederholbar: Zwei Knoten kommen auf dieselbe Adresse,
    /// ohne dass jemand sie verteilt.
    #[test]
    fn die_adresse_ist_fest() {
        assert_eq!(treasury_adresse(), treasury_adresse());
        assert_ne!(treasury_adresse(), Address::new([0u8; 32]));
    }

    /// ⚑ **Kein Schlüssel trifft sie.**
    ///
    /// Der Test kann keinen Preimage-Angriff ausschließen, das leistet
    /// SHA-256. Er hält fest, dass die Adresse **nicht** aus einem
    /// Schlüssel abgeleitet ist, den jemand haben könnte: Sie entsteht
    /// aus einem Trennstring, nicht aus einem öffentlichen Schlüssel.
    #[test]
    fn sie_stammt_nicht_aus_einem_schluessel() {
        for b in 0..8u8 {
            let sk = BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel");
            let pk = sk.public_key().expect("pk");
            assert_ne!(
                Address::aus_schluessel(&pk),
                treasury_adresse(),
                "ein gewoehnlicher Schluessel traf die Treasury"
            );
        }
    }
}
