//! Deterministischer Zufallsstrom und Shuffle aus einem Protokoll-Seed.
//!
//! Protokollweite Primitive: überall dort, wo aus einem VRF-Seed eine
//! reproduzierbare Auswahl abgeleitet wird — Epochen-Scheduler
//! (Shard-Zuweisung, Redundanz, Stichprobenlotterie, Geo-Clustering)
//! und Komiteewahl im Konsens. Sie liegt hier, damit es **eine**
//! Fassung gibt: eine zweite Kopie in einem anderen Crate würde bei der
//! nächsten Korrektur unweigerlich abdriften, und das Ergebnis ist in
//! allen Verwendungen Konsens-Feld.
//!
//! ## Warum nicht der naheliegende XOR-Shift (Fund A6)
//!
//! Die vorherigen Fassungen im Scheduler zogen den Vertauschungsindex
//! aus **einem einzigen Byte** des Zustands:
//!
//! ```text
//! let j = (state[0] as usize) % (i + 1);
//! ```
//!
//! Daraus folgten zwei Fehler:
//!
//! 1. **Kein gleichverteilter Shuffle über 256 Elemente hinaus.** Bei
//!    1 000 Segmenten und 2 % Stichprobenrate lag die tatsächliche
//!    Prüfwahrscheinlichkeit zwischen dem 0,14-fachen (Index 0) und dem
//!    3,87-fachen (Index 256) des Erwartungswerts — Spreizung ~Faktor 28.
//! 2. **192 der 256 Seed-Bits blieben ungenutzt.** Der XOR-Shift
//!    arbeitete nur auf `state[0..8]`.
//!
//! ## Die jetzige Konstruktion
//!
//! - **RNG:** SHA-256 im Zählermodus (`sha256(seed ‖ counter_le)`), also
//!   der Hash, den das Protokoll ohnehin überall verwendet. Alle 256
//!   Seed-Bits gehen ein, die Ausgabe ist plattformunabhängig bitgleich.
//! - **Index-Wahl:** Verwerfungsverfahren statt `% n`. Ein einfaches
//!   Modulo ist für nicht-teilende `n` verzerrt; das Verwerfen des
//!   unvollständigen Restbereichs liefert exakte Gleichverteilung.
//!   Determinismus bleibt: gleicher Seed → gleiche Verwerfungen.
//!
//! **Konsens-Feld:** Änderungen nur über Governance (Kap. 10.3) — jede
//! Änderung verschiebt sämtliche abgeleiteten Auswahlen.

use sha2::{Digest, Sha256};

/// Deterministischer Zufallsstrom aus einem 32-Byte-Seed.
///
/// SHA-256 im Zählermodus: Block `k` ist `sha256(seed ‖ u64_le(k))`.
/// Jeder Block liefert vier `u64`-Werte.
pub struct SeedRng {
    seed: [u8; 32],
    counter: u64,
    block: [u8; 32],
    /// Nächstes ungelesenes Byte im aktuellen Block (32 = erschöpft).
    pos: usize,
}

impl SeedRng {
    /// Erzeugt einen Zufallsstrom aus dem Epochenseed.
    pub fn new(seed: &[u8; 32]) -> Self {
        Self {
            seed: *seed,
            counter: 0,
            block: [0u8; 32],
            pos: 32,
        }
    }

    /// Liefert den nächsten `u64` aus dem Strom.
    pub fn next_u64(&mut self) -> u64 {
        if self.pos + 8 > 32 {
            let mut hasher = Sha256::new();
            hasher.update(self.seed);
            hasher.update(self.counter.to_le_bytes());
            self.block.copy_from_slice(&hasher.finalize());
            self.counter = self.counter.wrapping_add(1);
            self.pos = 0;
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.block[self.pos..self.pos + 8]);
        self.pos += 8;
        u64::from_le_bytes(buf)
    }

    /// Liefert einen gleichverteilten Wert in `[0, n)`.
    ///
    /// Verwerfungsverfahren: Werte oberhalb des größten Vielfachen von
    /// `n` werden verworfen, damit kein Rest-Bereich einzelne Ergebnisse
    /// bevorzugt. Die erwartete Anzahl Ziehungen ist < 2.
    ///
    /// **Panics:** wenn `n == 0`.
    pub fn next_below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "next_below(0) ist nicht definiert");
        if n == 1 {
            return 0;
        }
        // Größtes Vielfaches von n, das in u64 passt.
        let zone = (u64::MAX / n) * n;
        loop {
            let x = self.next_u64();
            if x < zone {
                return x % n;
            }
        }
    }
}

/// Fisher-Yates-Shuffle, deterministisch aus dem Seed und gleichverteilt.
///
/// **Parameter:**
/// - `items`: die zu mischende Folge (wird an Ort und Stelle verändert)
/// - `seed`: Epochenseed (VRF-abgeleitet, Phase 2.1)
///
/// **Eigenschaften:** Gleicher Seed und gleiche Eingabe → gleiche
/// Ausgabe, auf jeder Plattform bitgleich. Jede der `n!` Permutationen
/// ist gleich wahrscheinlich (über die Seeds gemittelt).
pub fn deterministic_shuffle<T>(items: &mut [T], seed: &[u8; 32]) {
    let mut rng = SeedRng::new(seed);
    for i in (1..items.len()).rev() {
        let j = rng.next_below((i + 1) as u64) as usize;
        items.swap(i, j);
    }
}

/// Zieht `count` Indizes gewichtet und ohne Zurücklegen.
///
/// Jeder Index `i` wird mit einer Wahrscheinlichkeit proportional zu
/// `weights[i]` gezogen; ein bereits gezogener Index kommt nicht erneut
/// in Frage. Das ist die Grundlage der Komiteewahl: „gewählt nach Stake,
/// rotierend per VRF" (Whitepaper Kap. 3.5) — die Gewichtung bildet den
/// Stake und die nachgewiesene Inferenzarbeit ab, der VRF-Seed sorgt
/// dafür, dass die Auswahl zwischen den Epochen rotiert statt eine feste
/// Rangliste zu zementieren.
///
/// **Verfahren:** Wiederholte Ziehung aus dem kumulierten Gewicht der
/// noch verfügbaren Kandidaten, mit `next_below` (verwerfungsbasiert,
/// also unverzerrt). Kandidaten mit Gewicht 0 werden nie gezogen.
///
/// **Determinismus:** Gleicher Seed und gleiche Gewichte → gleiche
/// Auswahl **in gleicher Reihenfolge**, auf jeder Plattform bitgleich.
///
/// **Parameter:**
/// - `weights`: Gewicht je Kandidat (Index = Kandidatennummer)
/// - `count`: gewünschte Anzahl Ziehungen
/// - `seed`: Epochenseed (VRF-abgeleitet)
///
/// **Returns:** Gezogene Indizes in Ziehungsreihenfolge. Kürzer als
/// `count`, wenn nicht genug Kandidaten mit Gewicht > 0 vorhanden sind.
pub fn weighted_sample_without_replacement(
    weights: &[u64],
    count: usize,
    seed: &[u8; 32],
) -> Vec<usize> {
    let mut rng = SeedRng::new(seed);
    let mut remaining: Vec<(usize, u64)> = weights
        .iter()
        .enumerate()
        .filter(|(_, &w)| w > 0)
        .map(|(i, &w)| (i, w))
        .collect();

    // u128 für die Summe: 2^64 Kandidaten-Gewichte könnten u64 sprengen.
    let mut total: u128 = remaining.iter().map(|(_, w)| *w as u128).sum();

    let mut picked = Vec::with_capacity(count.min(remaining.len()));
    while picked.len() < count && !remaining.is_empty() {
        // next_below arbeitet auf u64; bei sehr großen Summen in
        // Blöcken ziehen wäre nötig — praktisch bleibt total < 2^64,
        // weil Stake und Arbeit beide u64-Größen sind. Zur Sicherheit
        // wird geklemmt statt still falsch zu rechnen.
        let bound = u64::try_from(total).unwrap_or(u64::MAX);
        let mut ticket = rng.next_below(bound) as u128;

        let mut chosen = remaining.len() - 1;
        for (idx, (_, w)) in remaining.iter().enumerate() {
            let w = *w as u128;
            if ticket < w {
                chosen = idx;
                break;
            }
            ticket -= w;
        }

        let (candidate, weight) = remaining.swap_remove(chosen);
        // swap_remove zerstört die Reihenfolge — für Determinismus muss
        // die Kandidatenliste in kanonischer Ordnung bleiben.
        remaining.sort_unstable_by_key(|(i, _)| *i);
        total -= weight as u128;
        picked.push(candidate);
    }

    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn shuffle_ist_deterministisch() {
        let seed = [7u8; 32];
        let mut a: Vec<u32> = (0..100).collect();
        let mut b: Vec<u32> = (0..100).collect();
        deterministic_shuffle(&mut a, &seed);
        deterministic_shuffle(&mut b, &seed);
        assert_eq!(a, b);
    }

    #[test]
    fn shuffle_ist_eine_permutation() {
        let mut v: Vec<u32> = (0..1000).collect();
        deterministic_shuffle(&mut v, &[3u8; 32]);
        let mut sorted = v.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..1000).collect::<Vec<u32>>());
    }

    #[test]
    fn verschiedene_seeds_liefern_verschiedene_permutationen() {
        let mut a: Vec<u32> = (0..64).collect();
        let mut b: Vec<u32> = (0..64).collect();
        deterministic_shuffle(&mut a, &[1u8; 32]);
        deterministic_shuffle(&mut b, &[2u8; 32]);
        assert_ne!(a, b);
    }

    /// Der Kern von Fund A6: Bytes jenseits der ersten acht müssen die
    /// Auswahl beeinflussen. Die alte Fassung ignorierte 192 der 256 Bits.
    #[test]
    fn alle_seed_bytes_gehen_ein() {
        let mut base = [0u8; 32];
        let mut a: Vec<u32> = (0..64).collect();
        deterministic_shuffle(&mut a, &base);

        for byte_index in 8..32 {
            base[byte_index] = 1;
            let mut b: Vec<u32> = (0..64).collect();
            deterministic_shuffle(&mut b, &base);
            assert_ne!(
                a, b,
                "Seed-Byte {} beeinflusst das Ergebnis nicht",
                byte_index
            );
            base[byte_index] = 0;
        }
    }

    /// Die eigentliche Regression: Bei mehr als 256 Elementen muss die
    /// Auswahl gleichverteilt bleiben. Vorher lag die Spreizung der
    /// Auswahlhaeufigkeit bei etwa Faktor 28 zwischen den Indizes.
    #[test]
    fn auswahl_ist_ueber_1000_positionen_gleichverteilt() {
        const N: usize = 1000;
        const PICK: usize = 20;
        const RUNS: u32 = 4000;

        let mut counts: HashMap<u32, u32> = HashMap::new();
        for run in 0..RUNS {
            let mut seed = [0u8; 32];
            seed[0..4].copy_from_slice(&run.to_le_bytes());
            let mut idx: Vec<u32> = (0..N as u32).collect();
            deterministic_shuffle(&mut idx, &seed);
            for &v in idx.iter().take(PICK) {
                *counts.entry(v).or_insert(0) += 1;
            }
        }

        let expected = (RUNS as f64 * PICK as f64) / N as f64;
        let min = (0..N as u32).map(|i| *counts.get(&i).unwrap_or(&0)).min().unwrap() as f64;
        let max = (0..N as u32).map(|i| *counts.get(&i).unwrap_or(&0)).max().unwrap() as f64;

        // Bei 80 000 Ziehungen auf 1000 Positionen ist der Erwartungswert
        // 80 pro Position; die Standardabweichung liegt bei ~8,9. Eine
        // Schranke von [0,4x, 1,8x] laesst reichlich Luft fuer statistische
        // Schwankung, faengt aber die alte Spreizung (0,14x bis 3,87x)
        // zuverlaessig ab.
        assert!(
            min > expected * 0.4,
            "seltenste Position: {} (erwartet {:.0})",
            min,
            expected
        );
        assert!(
            max < expected * 1.8,
            "haeufigste Position: {} (erwartet {:.0})",
            max,
            expected
        );
    }

    /// Positionen unterhalb und oberhalb der alten 256er-Grenze duerfen
    /// sich nicht systematisch unterscheiden.
    #[test]
    fn keine_stufe_an_der_alten_256er_grenze() {
        const N: usize = 1000;
        const PICK: usize = 20;
        const RUNS: u32 = 4000;

        let mut below = 0u32; // Indizes 0..255
        let mut above = 0u32; // Indizes 256..999
        for run in 0..RUNS {
            let mut seed = [0u8; 32];
            seed[0..4].copy_from_slice(&run.to_le_bytes());
            let mut idx: Vec<u32> = (0..N as u32).collect();
            deterministic_shuffle(&mut idx, &seed);
            for &v in idx.iter().take(PICK) {
                if v < 256 {
                    below += 1;
                } else {
                    above += 1;
                }
            }
        }

        // Erwartungsverhaeltnis 256:744; die alte Fassung lag bei ~1:17.
        let ratio = below as f64 / above as f64;
        let expected = 256.0 / 744.0;
        assert!(
            (ratio / expected - 1.0).abs() < 0.15,
            "Verhaeltnis {:.3} weicht zu stark von {:.3} ab",
            ratio,
            expected
        );
    }

    #[test]
    fn next_below_bleibt_im_bereich() {
        let mut rng = SeedRng::new(&[42u8; 32]);
        for n in 1..=64u64 {
            for _ in 0..50 {
                assert!(rng.next_below(n) < n);
            }
        }
    }

    #[test]
    fn next_below_eins_ist_immer_null() {
        let mut rng = SeedRng::new(&[1u8; 32]);
        for _ in 0..20 {
            assert_eq!(rng.next_below(1), 0);
        }
    }

    #[test]
    fn rng_liefert_ueber_blockgrenzen_hinweg() {
        // Ein SHA-256-Block gibt vier u64; der fuenfte Aufruf muss
        // sauber nachziehen.
        let mut rng = SeedRng::new(&[9u8; 32]);
        let vals: Vec<u64> = (0..12).map(|_| rng.next_u64()).collect();
        assert_eq!(vals.len(), 12);
        // Kein Block darf sich wiederholen.
        assert_ne!(&vals[0..4], &vals[4..8]);
        assert_ne!(&vals[4..8], &vals[8..12]);
    }

    #[test]
    fn gewichtete_auswahl_ist_deterministisch() {
        let w = vec![10u64, 20, 30, 40];
        let a = weighted_sample_without_replacement(&w, 3, &[5u8; 32]);
        let b = weighted_sample_without_replacement(&w, 3, &[5u8; 32]);
        assert_eq!(a, b);
    }

    #[test]
    fn gewichtete_auswahl_zieht_ohne_zuruecklegen() {
        let w = vec![1u64; 20];
        let picked = weighted_sample_without_replacement(&w, 10, &[1u8; 32]);
        assert_eq!(picked.len(), 10);
        let mut sorted = picked.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 10, "kein Index darf doppelt vorkommen");
    }

    #[test]
    fn gewicht_null_wird_nie_gezogen() {
        let w = vec![0u64, 5, 0, 5, 0];
        let picked = weighted_sample_without_replacement(&w, 5, &[3u8; 32]);
        assert_eq!(picked.len(), 2, "nur zwei Kandidaten haben Gewicht");
        assert!(picked.iter().all(|&i| w[i] > 0));
    }

    #[test]
    fn hoeheres_gewicht_wird_haeufiger_gezogen() {
        // Kandidat 0 hat 10x das Gewicht von Kandidat 1..10.
        let mut w = vec![1u64; 11];
        w[0] = 10;
        let mut hits = 0;
        const RUNS: u32 = 2000;
        for r in 0..RUNS {
            let mut seed = [0u8; 32];
            seed[0..4].copy_from_slice(&r.to_le_bytes());
            if weighted_sample_without_replacement(&w, 1, &seed)[0] == 0 {
                hits += 1;
            }
        }
        // Erwartung: 10/20 = 50 %.
        let share = hits as f64 / RUNS as f64;
        assert!(
            (0.44..0.56).contains(&share),
            "Anteil {:.3} weicht zu stark von 0,50 ab",
            share
        );
    }

    #[test]
    fn gewichtete_auswahl_rotiert_mit_dem_seed() {
        // Der Kern der VRF-Rotation: dieselbe Gewichtsverteilung darf
        // nicht in jeder Epoche dasselbe Komitee liefern.
        let w = vec![100u64; 30];
        let a = weighted_sample_without_replacement(&w, 10, &[1u8; 32]);
        let b = weighted_sample_without_replacement(&w, 10, &[2u8; 32]);
        assert_ne!(a, b);
    }

    #[test]
    fn gewichtete_auswahl_bei_zu_wenig_kandidaten() {
        let w = vec![1u64, 2, 3];
        let picked = weighted_sample_without_replacement(&w, 10, &[7u8; 32]);
        assert_eq!(picked.len(), 3);
    }

    #[test]
    fn leere_und_einelementige_folgen() {
        let mut empty: Vec<u32> = vec![];
        deterministic_shuffle(&mut empty, &[0u8; 32]);
        assert!(empty.is_empty());

        let mut single = vec![7u32];
        deterministic_shuffle(&mut single, &[0u8; 32]);
        assert_eq!(single, vec![7]);
    }
}
