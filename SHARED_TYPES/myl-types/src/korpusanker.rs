//! Der verankerte Trainingskorpus (Whitepaper Kap. 7.3).
//!
//! # ⚑ Was hier steht und was ausdruecklich nicht
//!
//! Ein **Anker**, nicht die Daten. Der Konsens haelt die Merkle-Wurzel,
//! die Kennung und die Zahl der Segmente; die Daten selbst liegen
//! anderswo, und ein Pod weist ueber einen Merkle-Beweis nach, dass sein
//! Segment dazugehoert.
//!
//! # ⚑ Warum es keine Anweisung `KorpusVerankern` gibt
//!
//! Sie waere naheliegend und faehrlaessig. **Wessen Daten das Modell
//! trainieren, ist die schwerste Entscheidung dieses Systems**, und eine
//! Anweisung, die jeder einreichen kann, uebergaebe sie an jeden.
//!
//! Der richtige Weg ist ein Governance-Beschluss. Den gibt es heute
//! nicht: Die Parameter-Registry ist vom Kettenzustand aus nicht
//! erreichbar, also gibt es ueberhaupt keinen Weg, auf dem ein Beschluss
//! den Zustand aendert.
//!
//! **Solange das so ist, steht der Anker im Genesis und sonst nirgends.**
//! Ein Netz kann seinen Korpus damit nicht wechseln, und das ist die
//! ehrlichere Einschraenkung: lieber unbeweglich als von jedem beweglich.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::Hash;

/// Ein verankerter Korpus.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Korpusanker {
    /// Der Name, unter dem der Korpus gefuehrt wird.
    ///
    /// ⚑ **Geht in die Zuweisung ein** und ist deshalb keine Zierde:
    /// `myl_train::zuweisung` mischt ihn mit, damit zwei Korpora
    /// demselben Pod verschiedene Buendel geben.
    pub kennung: String,
    /// Die Merkle-Wurzel ueber alle Segmente.
    pub wurzel: Hash,
    /// Wie viele Segmente er hat.
    pub segmente: u64,
    /// Wie viele Segmente ein zugewiesenes Buendel umfasst.
    ///
    /// ⚑ **Teil des Ankers und kein freier Parameter des Pods.** Anhang
    /// B.6.4 rechnet den Beweis-Overhead an der Buendelgroesse aus (ein
    /// Segment 11,7 Prozent, 256 Segmente 0,42 Prozent); wer sie waehlen
    /// duerfte, waehlte seinen eigenen Aufwand.
    pub buendel: u64,
}

impl Korpusanker {
    /// Traegt der Anker ueberhaupt ein volles Buendel?
    ///
    /// ⚑ **Ein Anker, der kein Buendel hergibt, ist kein Korpus**,
    /// sondern eine Zahl. Die Zuweisung koennte nichts zuweisen, und ein
    /// Trainingspod bekaeme eine leere Aufgabe.
    pub fn traegt_ein_buendel(&self) -> bool {
        self.buendel > 0 && self.segmente >= self.buendel
    }

    /// Die **Charge** eines zugewiesenen Buendels: der Wert, den ein
    /// Trainingssegment als seinen Datenstand nennt.
    ///
    /// # ⚑ Warum die Wurzel mit hineingeht
    ///
    /// Ohne sie waere die Charge nur „Segment 512 bis 767", und dieselbe
    /// Angabe passte auf **jeden** Korpus. Ein Pod koennte auf eigenen
    /// Daten rechnen und dieselbe Charge nennen; der Konsens saehe keinen
    /// Unterschied.
    ///
    /// ⚑ **Sie ist keine Pruefung der Daten**, sondern deren Benennung.
    /// Ob das Segment wirklich zum Korpus gehoert, weist ein
    /// Merkle-Beweis nach (Kap. 7.3); die Charge sagt nur, welcher Teil
    /// gemeint war, und bindet ihn an den verankerten Korpus.
    pub fn charge(&self, start: u64, laenge: u64) -> Hash {
        let mut stoff = Vec::with_capacity(64 + self.kennung.len());
        stoff.extend_from_slice(b"myl-trainingscharge-v1");
        // ⚑ Laengenpraefix vor der Kennung: Ohne es liesse sich
        // ("ab", 1) nicht von ("a", "b1") unterscheiden.
        stoff.extend_from_slice(&(self.kennung.len() as u64).to_le_bytes());
        stoff.extend_from_slice(self.kennung.as_bytes());
        stoff.extend_from_slice(self.wurzel.as_bytes());
        stoff.extend_from_slice(&start.to_le_bytes());
        stoff.extend_from_slice(&laenge.to_le_bytes());
        Hash::sha256(&stoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anker(segmente: u64, buendel: u64) -> Korpusanker {
        Korpusanker {
            kennung: "probe".into(),
            wurzel: Hash([1u8; 32]),
            segmente,
            buendel,
        }
    }

    #[test]
    fn ein_anker_ohne_volles_buendel_traegt_nicht() {
        assert!(anker(256, 16).traegt_ein_buendel());
        assert!(anker(16, 16).traegt_ein_buendel());
        assert!(!anker(15, 16).traegt_ein_buendel(), "kein volles Buendel");
        assert!(!anker(256, 0).traegt_ein_buendel(), "Buendelgroesse null");
        assert!(!anker(0, 16).traegt_ein_buendel(), "leerer Korpus");
    }

    /// ⚑ **Zwei Korpora, dieselbe Stelle, verschiedene Chargen.** Ohne
    /// die Wurzel in der Charge waere ein selbst erfundener Korpus vom
    /// verankerten nicht zu unterscheiden.
    #[test]
    fn die_charge_haengt_am_korpus_und_nicht_nur_an_der_stelle() {
        let a = anker(4096, 256);
        let mut b = anker(4096, 256);
        b.wurzel = Hash([9u8; 32]);
        let mut c = anker(4096, 256);
        c.kennung = "anderer".into();
        assert_ne!(a.charge(512, 256), b.charge(512, 256), "die Wurzel wirkt nicht");
        assert_ne!(a.charge(512, 256), c.charge(512, 256), "die Kennung wirkt nicht");
        assert_ne!(a.charge(512, 256), a.charge(768, 256), "die Stelle wirkt nicht");
        assert_eq!(a.charge(512, 256), a.charge(512, 256), "nicht deterministisch");
    }

    /// Das Laengenpraefix trennt, was sonst zusammenfiele.
    #[test]
    fn das_laengenpraefix_trennt_die_kennungen() {
        let mut a = anker(4096, 256);
        a.kennung = "ab".into();
        let mut b = anker(4096, 256);
        b.kennung = "a".into();
        assert_ne!(a.charge(1, 1), b.charge(1, 1));
    }

    #[test]
    fn der_rundlauf_ueber_borsh_haelt() {
        let a = anker(4096, 256);
        let bytes = borsh::to_vec(&a).expect("serialisieren");
        let zurueck = Korpusanker::try_from_slice(&bytes).expect("lesen");
        assert_eq!(a, zurueck);
    }
}
