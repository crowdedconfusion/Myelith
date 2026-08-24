//! Ganzzahlige EMA für das geglättete Burn-Volumen `B̄_e`
//! (Punkt 1.1, Whitepaper Kap. 5.2).
//!
//! Formel: `B̄_e = B̄_{e−1} + α · (B_e − B̄_{e−1})` mit
//! `α = 2/(N+1)` und N = 30 Epochen (Design-Entscheidung 2026-08-13),
//! also `α = 2/31` als Ganzzahl-Bruch.
//!
//! Rundung: Der Korrekturschritt wird mit Rust-Ganzzahldivision
//! berechnet (Abschneiden Richtung Null) — deterministisch auf jeder
//! Architektur, keine Gleitkomma-Bibliothek im Pfad.
//!
//! **Totzone (dokumentiertes Verhalten):** Durch das Abschneiden wird
//! der Schritt 0, sobald `|sample − prev| < den/num` (bei α = 2/31:
//! unter 15,5 Einheiten). Die EMA steht dann innerhalb von ±15 Einheiten
//! um die Stichprobe still. Für Burn-Volumina in Millionen-
//! Kleinstbeträgen ist die Totzone vernachlässigbar; sie ist der Preis
//! der Gleitkomma-Freiheit und auf jedem Node identisch.

/// Zähler des EMA-Glättungsbruchs α (Standard-EMA: α = 2/(N+1)).
pub const EMA_ALPHA_NUM: u64 = 2;
/// Nenner des EMA-Glättungsbruchs α (N = 30 Epochen ⇒ α = 2/31).
pub const EMA_ALPHA_DEN: u64 = 31;

/// Ein EMA-Schritt mit dem Protokoll-α (2/31).
pub fn ema_update(prev: u64, sample: u64) -> u64 {
    ema_update_with_alpha(prev, sample, EMA_ALPHA_NUM, EMA_ALPHA_DEN)
}

/// Ein EMA-Schritt mit explizitem Bruch α = `num`/`den`.
///
/// Für 0 < num ≤ den liegt das Ergebnis stets zwischen `prev` und
/// `sample` (beweisbar: der Schritt ist ein Anteil der Differenz) —
/// damit sind Überlauf und Unterlauf ausgeschlossen. Der Fall num ≥ den
/// ergibt einen überkorrigierenden Schritt und ist für den
/// Protokollgebrauch nicht vorgesehen; die Funktion bleibt aber total
/// und deterministisch.
///
/// ## ⚑ Fund 47: „total" galt nur im Release-Build
///
/// Die Zusage oben stand hier von Anfang an, und zwei Dinge hielten sie
/// nicht:
///
/// 1. Ein `debug_assert!` fing `num > den` ab — also **panisch im
///    Debug-Build, still im Release-Build**. Eine Funktion, die je nach
///    Bauprofil abstürzt oder rechnet, ist nicht total, und zwei Knoten
///    mit verschiedenen Profilen sind sich uneins.
/// 2. Der Abschluss `new as u64` **läuft um**. Für `num > den` kann der
///    Schritt unter null gehen; `−200 as u64` ist ein Wert nahe 2⁶⁴, und
///    der geht als geglättetes Burn-Volumen direkt in `mint_amount`, wo
///    er die Prägung an die Obergrenze treibt.
///
/// Erreichbar ist das, weil α ein Governance-Parameter ist. Beides
/// behoben: Der `debug_assert` ist weg, und das Ergebnis wird auf den
/// `u64`-Bereich **beschnitten** statt umgelaufen. Ein überkorrigierender
/// Schritt bleibt ein Parameter-Fehler, aber er endet jetzt bei 0 oder
/// `u64::MAX` und auf jedem Bauprofil gleich.
///
/// Die Prüfung von α gehört in die Governance-Schicht; diese Funktion
/// kann sie nicht ersetzen, sie kann nur aufhören, den Fehler zu
/// verstärken.
pub fn ema_update_with_alpha(prev: u64, sample: u64, num: u64, den: u64) -> u64 {
    if den == 0 {
        // Defensiv: ein Nenner von 0 ist ein Parameter-Fehler; das
        // Verhalten bleibt deterministisch (Zustand unverändert).
        return prev;
    }
    let delta = sample as i128 - prev as i128;
    let step = delta * num as i128 / den as i128;
    let new = prev as i128 + step;
    new.clamp(0, u64::MAX as i128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exakter_schritt_ohne_rundung() {
        // 0 + 2/31 · 31 = 2.
        assert_eq!(ema_update(0, 31), 2);
        // 31 + 2/31 · (0 − 31) = 31 − 2 = 29.
        assert_eq!(ema_update(31, 0), 29);
    }

    #[test]
    fn rundung_schneidet_richtung_null_ab() {
        // 2/31 · 1 = 0,0645… → 0 (Richtung Null).
        assert_eq!(ema_update(0, 1), 0);
        assert_eq!(ema_update(1, 0), 1);
        // 2/31 · 15 = 0,967… → 0.
        assert_eq!(ema_update(0, 15), 0);
        // 2/31 · 16 = 1,032… → 1.
        assert_eq!(ema_update(0, 16), 1);
    }

    #[test]
    fn konvergenz_gegen_konstante_stichprobe() {
        // Konstante Stichprobe: monotoner Zustrom von unten…
        let mut ema = 0u64;
        for _ in 0..1_000 {
            let neu = ema_update(ema, 1_000_000);
            assert!(neu >= ema);
            ema = neu;
        }
        // …und von oben.
        let mut ema_oben = 2_000_000u64;
        for _ in 0..1_000 {
            let neu = ema_update(ema_oben, 1_000_000);
            assert!(neu <= ema_oben);
            ema_oben = neu;
        }
        // Beide Seiten konvergieren in die Totzone um die Stichprobe
        // (±(den/num) = ±15 bei α = 2/31, siehe Modul-Dokumentation).
        assert!(ema.abs_diff(ema_oben) <= 31);
        assert!(ema.abs_diff(1_000_000) <= 15);
        assert!(ema_oben.abs_diff(1_000_000) <= 15);
    }

    #[test]
    fn alpha_eins_uebernimmt_stichprobe() {
        assert_eq!(ema_update_with_alpha(100, 500, 1, 1), 500);
        assert_eq!(ema_update_with_alpha(500, 100, 1, 1), 100);
    }

    #[test]
    fn ergebnis_liegt_stets_zwischen_prev_und_sample() {
        // Stichprobenartig für viele Wertepaare.
        let mut state = 0x5EEDu64;
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let prev = state >> 1;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let sample = state >> 1;
            let neu = ema_update(prev, sample);
            let (lo, hi) = if prev <= sample { (prev, sample) } else { (sample, prev) };
            assert!(neu >= lo && neu <= hi, "prev={} sample={} neu={}", prev, sample, neu);
        }
    }

    #[test]
    fn determinismus_ueber_zehntausend_epochen() {
        // Zwei unabhängige Läufe derselben Stichprobenfolge ergeben
        // bitgleiche EMA-Verläufe.
        fn lauf(seed: u64) -> Vec<u64> {
            let mut state = seed;
            let mut ema = 0u64;
            let mut verlauf = Vec::with_capacity(10_000);
            for _ in 0..10_000 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let stichprobe = state >> 8;
                ema = ema_update(ema, stichprobe);
                verlauf.push(ema);
            }
            verlauf
        }
        assert_eq!(lauf(0xBEEF), lauf(0xBEEF));
        assert_ne!(lauf(0xBEEF), lauf(0xCAFE));
    }
}
