//! Sitze: der Einsatz eines Stimmsatzes, ganzzahlig verkleinert.
//!
//! ⚑ **Wozu** (2026-11-03): Eine Schwellensignatur zaehlt Anteile, nicht
//! Einsatz. Jedes Mitglied bekommt so viele Anteile (Sitze), wie sein
//! Einsatz traegt, und die Schwelle wird in Sitzen gemessen. Vorlage ist
//! Swiper (Freitas, Tonkikh, arXiv 2307.15561): Gewichte auf kleine ganze
//! Zahlen abbilden, ohne dass eine Koalition durch das Runden ueber eine
//! Schwelle rutscht.
//!
//! ⚑ **Die Rundung ist immer ab.** Bei `S` Sitzen bekommt ein Mitglied mit
//! Einsatz `w` von `W` genau `⌊w · S / W⌋`. Damit hat jede Koalition
//! hoechstens so viele Sitze, wie ihr Anteil am Einsatz verspricht, nie
//! mehr: Ein Angreifer gewinnt durch das Runden nichts, und die Geheimhaltung
//! braucht keine Reserve. Verloren gehen Sitze nur auf der ehrlichen Seite,
//! hoechstens einer je Mitglied, und genau das prueft die Suche.
//!
//! **Die beiden Zusagen**, beide ganzzahlig nachgerechnet:
//!
//! 1. **Geheimhaltung:** Wer weniger als die Haelfte des Einsatzes haelt,
//!    haelt weniger als `schwelle` Sitze, kann die Saat also nicht allein
//!    bilden. Die Schwelle ist `⌈S/2⌉`.
//! 2. **Lebendigkeit:** Wer mindestens zwei Drittel des Einsatzes haelt,
//!    haelt mindestens `schwelle` Sitze. Sein Rest hat hoechstens ein
//!    Drittel des Einsatzes, also hoechstens `⌊S/3⌋` Sitze; gesucht wird
//!    deshalb das kleinste `S` mit `T − ⌊S/3⌋ ≥ ⌈S/2⌉`, wo `T` die
//!    vergebenen Sitze sind.
//!
//! Das kleinste solche `S` existiert immer: Der Rundungsverlust ist
//! kleiner als die Zahl der Mitglieder `n`, und ab `S ≥ 6n + 6` genuegt
//! das. Bei gleichen Einsaetzen ist `S = n` schon die Antwort.

/// Wie viele Sitze fuer die Suche hoechstens probiert werden, je
/// Mitglied. Die Schranke `6n + 6` erreicht die Suche nie ganz; hier steht
/// sie mit Reserve, damit ein Fehler laut wird und nicht still endet.
const SUCHGRENZE_JE_MITGLIED: u64 = 8;

/// Die Sitzvergabe eines Satzes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sitzvergabe {
    /// Sitze je Mitglied, in der Reihenfolge der Eingabe; null ist moeglich.
    pub sitze: Vec<u64>,
    /// Die Skala `S`.
    pub skala: u64,
    /// Wie viele Anteile eine Signatur braucht.
    pub schwelle: u64,
}

impl Sitzvergabe {
    /// Die vergebenen Sitze zusammen (`T`).
    pub fn gesamt(&self) -> u64 {
        self.sitze.iter().sum()
    }

    /// Die Sitznummern eines Mitglieds, ab eins durchgezaehlt in der
    /// Reihenfolge der Eingabe. Nummer null gibt es nicht: An der Null
    /// liegt das Geheimnis.
    pub fn nummern(&self, mitglied: usize) -> std::ops::Range<u64> {
        let vor: u64 = self.sitze[..mitglied].iter().sum();
        vor + 1..vor + 1 + self.sitze[mitglied]
    }

    /// Wem eine Sitznummer gehoert.
    pub fn inhaber(&self, nummer: u64) -> Option<usize> {
        let mut bis = 0u64;
        for (i, s) in self.sitze.iter().enumerate() {
            bis += s;
            if nummer >= 1 && nummer <= bis && *s > 0 {
                return Some(i);
            }
        }
        None
    }
}

/// `⌊w · S / W⌋`, ohne Ueberlauf.
fn sitze_fuer(w: u64, skala: u64, gesamt: u128) -> u64 {
    ((w as u128) * (skala as u128) / gesamt) as u64
}

/// Vergibt Sitze nach Einsatz. `None` bei leerem Satz oder ohne Einsatz.
pub fn vergeben(einsaetze: &[u64]) -> Option<Sitzvergabe> {
    let gesamt: u128 = einsaetze.iter().map(|w| *w as u128).sum();
    if gesamt == 0 {
        return None;
    }
    let n = einsaetze.iter().filter(|w| **w > 0).count() as u64;
    for skala in 1..=SUCHGRENZE_JE_MITGLIED * n + 8 {
        let sitze: Vec<u64> = einsaetze.iter().map(|w| sitze_fuer(*w, skala, gesamt)).collect();
        let t: u64 = sitze.iter().sum();
        let schwelle = skala.div_ceil(2);
        if t >= schwelle && t - skala / 3 >= schwelle {
            return Some(Sitzvergabe { sitze, skala, schwelle });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erschoepfend ueber alle Teilmengen: die beiden Zusagen wirklich, nicht
    /// nur ihre Herleitung.
    fn zusagen_pruefen(einsaetze: &[u64]) {
        let v = vergeben(einsaetze).expect("Vergabe");
        let gesamt: u128 = einsaetze.iter().map(|w| *w as u128).sum();
        let n = einsaetze.len();
        assert!(n <= 16, "erschoepfend nur fuer kleine Saetze");
        for maske in 0u32..(1 << n) {
            let (mut w, mut s) = (0u128, 0u64);
            for (i, (e, sitz)) in einsaetze.iter().zip(&v.sitze).enumerate() {
                if maske & (1 << i) != 0 {
                    w += *e as u128;
                    s += sitz;
                }
            }
            if 2 * w < gesamt {
                assert!(s < v.schwelle, "Geheimhaltung: {einsaetze:?} {maske:b} {v:?}");
            }
            if 3 * w >= 2 * gesamt {
                assert!(s >= v.schwelle, "Lebendigkeit: {einsaetze:?} {maske:b} {v:?}");
            }
        }
    }

    #[test]
    fn gleiche_einsaetze_ein_sitz_je_mitglied() {
        let v = vergeben(&[5, 5, 5, 5]).expect("Vergabe");
        assert_eq!(v.sitze, vec![1, 1, 1, 1]);
        assert_eq!(v.schwelle, 2);
        zusagen_pruefen(&[5, 5, 5, 5]);
    }

    #[test]
    fn die_zusagen_halten_auf_vielen_verteilungen() {
        let faelle: Vec<Vec<u64>> = vec![
            vec![1],
            vec![1, 1],
            vec![3, 1],
            vec![100, 1, 1, 1],
            vec![1_000_000, 4_000, 4_000, 3_999, 1],
            vec![7, 11, 13, 17, 19, 23, 29, 31],
            vec![u64::MAX / 4, u64::MAX / 4, 1, 2, 3],
            vec![10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 1],
            vec![50, 30, 20, 0, 5],
        ];
        for f in &faelle {
            zusagen_pruefen(f);
        }
        // Eine Pseudozufallsreihe, schief verteilt wie Einsatz in der Praxis.
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..200 {
            let mut f = Vec::new();
            for _ in 0..(x % 12 + 1) {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let groesse = 1u64 << ((x >> 58) % 20);
                f.push(groesse + (x >> 40) % groesse);
            }
            zusagen_pruefen(&f);
        }
    }

    #[test]
    fn nummern_und_inhaber() {
        let v = Sitzvergabe { sitze: vec![2, 0, 3], skala: 5, schwelle: 3 };
        assert_eq!(v.nummern(0), 1..3);
        assert_eq!(v.nummern(1), 3..3);
        assert_eq!(v.nummern(2), 3..6);
        assert_eq!(v.inhaber(0), None);
        assert_eq!(v.inhaber(2), Some(0));
        assert_eq!(v.inhaber(3), Some(2));
        assert_eq!(v.inhaber(6), None);
    }

    #[test]
    fn ohne_einsatz_keine_vergabe() {
        assert!(vergeben(&[]).is_none());
        assert!(vergeben(&[0, 0]).is_none());
    }

    #[test]
    fn die_suche_bleibt_klein() {
        // Ein Wal und viele Kleine: die schwierige Form.
        let mut f = vec![1_000_000u64];
        f.extend(std::iter::repeat_n(1_000, 60));
        let v = vergeben(&f).expect("Vergabe");
        assert!(v.skala <= 6 * 61 + 6, "{}", v.skala);
    }
}
