//! κ_v: die versionierte Wurzel der Wissensdatenbank.
//!
//! ## ⚑ Warum eine bewegliche Wurzel eine Version braucht
//!
//! „Umso mehr Knoten, umso mehr Wissen" heißt, dass die Wissensdatenbank
//! wächst, und damit bewegt sich ihre Merkle-Wurzel. **Ohne Version ist
//! jedes Abrufergebnis unreproduzierbar**, und das bricht den
//! Redundanzvergleich aus Kap. 6.4: Zwei redundante Pods fragen zu
//! verschiedenen Zeitpunkten, bekommen verschiedene Antworten und melden
//! einander als fehlerhaft, **ohne dass einer gelogen hätte**.
//!
//! Das ist genau die Lage, die Kap. 8.1 für externe Werkzeuge
//! beschreibt, nur hausgemacht.
//!
//! ## Also wie θ_v
//!
//! Version, Übergangsfrist, und **jede Anfrage nennt die Fassung**,
//! gegen die sie gestellt ist. Ein Abruf ohne κ_v ist so unvollständig
//! wie ein Segment ohne `model_version`.
//!
//! ## ⚑ Und warum es eine Übergangsfrist braucht und nicht nur eine Version
//!
//! Ein Wechsel ist nicht augenblicklich: Die neue Fassung muss verteilt
//! sein, bevor jemand gegen sie fragt. **Ohne Frist fragt der erste
//! gegen eine Fassung, die noch niemand hält**, und bekommt keine
//! Antwort statt einer alten. Während der Frist gelten zwei Fassungen
//! nebeneinander, und eine Anfrage sagt, welche sie meint.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::ids::{EpochId, MerkleRoot};

/// Eine Fassung der Wissensdatenbank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kappa {
    /// Fortlaufend, nie rückwärts.
    pub fassung: u32,
    /// Die Wurzel über den gesamten Bestand dieser Fassung.
    pub wurzel: MerkleRoot,
    /// Ab welcher Epoche sie gilt.
    pub gilt_ab: EpochId,
}

/// Warum ein Übergang nicht gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KappaFehler {
    /// Die neue Fassung ist nicht höher als die alte.
    NichtHoeher {
        /// Bisher.
        alt: u32,
        /// Vorgelegt.
        neu: u32,
    },
    /// Die neue Fassung soll vor der alten gelten.
    Rueckwaerts,
    /// Die Frist ist null.
    ///
    /// ⚑ **Kein Sonderfall, sondern der Fehler, den man macht.** Eine
    /// Frist von null heißt „ab sofort", und ab sofort hält die neue
    /// Fassung niemand.
    KeineFrist,
}

impl std::fmt::Display for KappaFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NichtHoeher { alt, neu } => write!(f, "Fassung {neu} ist nicht höher als {alt}"),
            Self::Rueckwaerts => write!(f, "die neue Fassung soll vor der alten gelten"),
            Self::KeineFrist => write!(f, "Übergangsfrist null: die neue Fassung hält niemand"),
        }
    }
}

impl std::error::Error for KappaFehler {}

/// Zwei Fassungen nebeneinander, während der Übergangsfrist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Uebergang {
    /// Die bisherige Fassung.
    pub alt: Kappa,
    /// Die neue.
    pub neu: Kappa,
}

impl Uebergang {
    /// Legt einen Übergang an und prüft ihn.
    pub fn neu(alt: Kappa, neu: Kappa) -> Result<Self, KappaFehler> {
        if neu.fassung <= alt.fassung {
            return Err(KappaFehler::NichtHoeher { alt: alt.fassung, neu: neu.fassung });
        }
        if neu.gilt_ab.0 < alt.gilt_ab.0 {
            return Err(KappaFehler::Rueckwaerts);
        }
        if neu.gilt_ab.0 == alt.gilt_ab.0 {
            return Err(KappaFehler::KeineFrist);
        }
        Ok(Self { alt, neu })
    }

    /// Welche Fassungen in dieser Epoche gültig sind.
    ///
    /// ⚑ **Während der Frist sind es zwei, und eine Anfrage muss sagen,
    /// welche sie meint.** Wer hier eine wählt, wählt für den Aufrufer,
    /// und zwei Pods könnten verschieden wählen.
    pub fn gueltig_in(&self, jetzt: EpochId) -> Vec<Kappa> {
        let mut aus = Vec::with_capacity(2);
        if jetzt.0 >= self.alt.gilt_ab.0 && jetzt.0 < self.neu.gilt_ab.0 {
            aus.push(self.alt);
        } else if jetzt.0 >= self.neu.gilt_ab.0 {
            aus.push(self.neu);
        }
        aus
    }

    /// Gilt diese Fassung in dieser Epoche?
    pub fn erlaubt(&self, jetzt: EpochId, fassung: u32) -> bool {
        self.gueltig_in(jetzt).iter().any(|k| k.fassung == fassung)
    }

    /// Wie lang die Frist ist, in Epochen.
    pub fn frist(&self) -> u64 {
        self.neu.gilt_ab.0.saturating_sub(self.alt.gilt_ab.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(fassung: u32, ab: u64) -> Kappa {
        Kappa {
            fassung,
            wurzel: MerkleRoot::new([fassung as u8; 32]),
            gilt_ab: EpochId(ab),
        }
    }

    /// ⚑ **Während der Frist gelten zwei Fassungen, danach eine.** Genau
    /// deshalb muss eine Anfrage sagen, welche sie meint: Wer hier eine
    /// wählt, wählt für den Aufrufer, und zwei redundante Pods könnten
    /// verschieden wählen.
    #[test]
    fn waehrend_der_frist_gelten_zwei_fassungen() {
        let u = Uebergang::neu(k(1, 100), k(2, 124)).expect("gültig");
        assert_eq!(u.frist(), 24);

        assert_eq!(u.gueltig_in(EpochId(99)), Vec::new(), "vor beiden");
        assert_eq!(u.gueltig_in(EpochId(100)).len(), 1);
        assert_eq!(u.gueltig_in(EpochId(123))[0].fassung, 1, "kurz vor dem Wechsel");
        assert_eq!(u.gueltig_in(EpochId(124))[0].fassung, 2, "ab dem Wechsel");
        assert_eq!(u.gueltig_in(EpochId(999))[0].fassung, 2);

        assert!(u.erlaubt(EpochId(110), 1));
        assert!(!u.erlaubt(EpochId(110), 2), "die neue gilt noch nicht");
        assert!(u.erlaubt(EpochId(130), 2));
        assert!(!u.erlaubt(EpochId(130), 1), "die alte gilt nicht mehr");
    }

    /// ⚑ Eine Frist von null heißt „ab sofort", und ab sofort hält die
    /// neue Fassung niemand. Das ist kein Sonderfall, sondern der
    /// Fehler, den man macht.
    #[test]
    fn eine_frist_von_null_wird_abgewiesen() {
        assert_eq!(Uebergang::neu(k(1, 100), k(2, 100)), Err(KappaFehler::KeineFrist));
    }

    #[test]
    fn rueckwaerts_geht_weder_in_der_fassung_noch_in_der_zeit() {
        assert_eq!(
            Uebergang::neu(k(2, 100), k(1, 124)),
            Err(KappaFehler::NichtHoeher { alt: 2, neu: 1 })
        );
        assert_eq!(
            Uebergang::neu(k(2, 100), k(2, 124)),
            Err(KappaFehler::NichtHoeher { alt: 2, neu: 2 }),
            "gleiche Fassung ist keine neue"
        );
        assert_eq!(Uebergang::neu(k(1, 100), k(2, 99)), Err(KappaFehler::Rueckwaerts));
    }
}
