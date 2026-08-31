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

/// Warum der Epochenabschluss nicht lief.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abschlussfehler {
    /// Diese Epoche ist bereits fortgeschrieben.
    ///
    /// ⚑ **Zweimal gerufen verschiebt die Glättung den Durchschnitt in
    /// Richtung der letzten Beobachtung**, und niemand sähe es der Zahl
    /// an. Ein Doppelaufruf ist deshalb ein Fehler und kein
    /// Wiederholungsversuch.
    SchonFortgeschrieben {
        /// Bis hierhin ist fortgeschrieben.
        bis: myl_types::ids::EpochId,
    },
}

impl std::fmt::Display for Abschlussfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchonFortgeschrieben { bis } => write!(
                f,
                "bis Epoche {} bereits fortgeschrieben",
                bis.0
            ),
        }
    }
}

impl std::error::Error for Abschlussfehler {}

/// Schreibt den geglätteten Burn um eine Epoche fort.
///
/// # Wozu
///
/// Kap. 5.2 leitet die Prägung `m_e` aus dem geglätteten Burn ab, den
/// geglätteten aus dem Burn je Epoche. Der Ledger zählt seit dem
/// 2026-08-31 mit, was verbrannt wurde; diese Funktion faltet die
/// Epochensumme in den Durchschnitt und setzt den Zähler zurück.
///
/// # ⚑ Warum sie hier steht und nicht im Ledger
///
/// Die Formel gehört zur Wirtschaft, nicht zum Kontenbuch, und
/// `myl-tokenomics` hängt ohnehin an `myl-ledger`; umgekehrt ginge es
/// nicht, das wäre ein Ring. Dasselbe Muster wie beim Slashing, das
/// ebenfalls von hier in den Zustand schreibt.
///
/// **Gibt den neuen geglätteten Wert zurück**, damit der Aufrufer ihn
/// unmittelbar an `mint_amount` geben kann, ohne den Zustand erneut zu
/// lesen.
pub fn epochenabschluss_burn(
    state: &mut myl_ledger::state::LedgerState,
) -> Result<u64, Abschlussfehler> {
    if state.burn_ema_bis >= state.epoch && state.epoch.0 > 0 {
        return Err(Abschlussfehler::SchonFortgeschrieben {
            bis: state.burn_ema_bis,
        });
    }
    let neu = ema_update(state.burn_ema, state.burn_epoche);
    state.burn_ema = neu;
    state.burn_epoche = 0;
    state.burn_ema_bis = state.epoch;
    Ok(neu)
}

#[cfg(test)]
mod abschluss_tests {
    use super::*;
    use myl_ledger::state::LedgerState;
    use myl_ledger::transitions::burn_to_credits;
    use myl_types::ids::{Address, EpochId};

    fn konto(b: u8) -> Address {
        Address::new([b; 32])
    }

    /// Was verbrannt wird, landet in der Epochensumme und von dort im
    /// Durchschnitt.
    #[test]
    fn verbranntes_erreicht_den_geglaetteten_wert() {
        let mut st = LedgerState::genesis(1);
        st.account_mut(&konto(1)).balance = 1_000;
        burn_to_credits(&mut st, &konto(1), 600, EpochId(10)).expect("burn");
        assert_eq!(st.burn_epoche, 600, "der Burn wurde nicht gezaehlt");

        st.epoch = EpochId(1);
        let neu = epochenabschluss_burn(&mut st).expect("Abschluss");
        assert_eq!(neu, ema_update(0, 600));
        assert_eq!(st.burn_ema, neu);
        assert_eq!(st.burn_epoche, 0, "der Zaehler wurde nicht zurueckgesetzt");
        assert_eq!(st.burn_ema_bis, EpochId(1));
    }

    /// ⚑ **Zweimal in derselben Epoche ist ein Fehler.**
    ///
    /// Der zweite Aufruf zöge den Durchschnitt ein zweites Mal in
    /// Richtung derselben Beobachtung, und das Ergebnis sähe
    /// unauffällig aus.
    #[test]
    fn zweimal_in_derselben_epoche_wird_abgelehnt() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(3);
        st.burn_epoche = 500;
        let erst = epochenabschluss_burn(&mut st).expect("erster Abschluss");
        assert_eq!(
            epochenabschluss_burn(&mut st),
            Err(Abschlussfehler::SchonFortgeschrieben { bis: EpochId(3) })
        );
        assert_eq!(st.burn_ema, erst, "der zweite Aufruf hat gerechnet");
    }

    /// Über mehrere Epochen nähert sich der Durchschnitt der Beobachtung.
    #[test]
    fn ueber_mehrere_epochen_naehert_er_sich_an() {
        let mut st = LedgerState::genesis(1);
        let mut vorher = 0;
        for e in 1..=10u64 {
            st.epoch = EpochId(e);
            st.burn_epoche = 1_000;
            let jetzt = epochenabschluss_burn(&mut st).expect("Abschluss");
            assert!(jetzt > vorher, "Epoche {e}: {jetzt} nicht groesser als {vorher}");
            assert!(jetzt <= 1_000, "ueber die Beobachtung hinausgeschossen");
            vorher = jetzt;
        }
    }

    /// Eine Epoche ohne Burn zieht den Durchschnitt nach unten, und das
    /// ist der Ausfall, den Punkt 21 beschreibt.
    #[test]
    fn eine_epoche_ohne_burn_zieht_den_durchschnitt_nach_unten() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(1);
        st.burn_epoche = 10_000;
        let hoch = epochenabschluss_burn(&mut st).expect("Abschluss");
        st.epoch = EpochId(2);
        let runter = epochenabschluss_burn(&mut st).expect("Abschluss");
        assert!(runter < hoch, "ohne Burn blieb der Durchschnitt stehen");
    }
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
