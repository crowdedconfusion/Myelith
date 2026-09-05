//! Auslastung `u_e` (Whitepaper Kap. 5.4).
//!
//! ```text
//! u_e = nachgefragte vTFE / verfuegbare Pod-Kapazitaet
//! ```
//!
//! Festkomma mit 16 Nachkommastellen: `u_e = 1,0` heisst volle
//! Auslastung, `u_e > 1,0` heisst Uebernachfrage.
//!
//! # ⚑ Warum die Rechnung hier liegt und nicht in der Wirtschaft
//!
//! Bis zum 2026-09-05 stand sie in `myl_tokenomics::utilization`, und
//! das war richtig, solange **nur der Preis** sie brauchte (Kap. 5.4).
//! Seither braucht sie auch der Scheduler: Kap. 7.1 macht die
//! Trainingsmenge von der Auslastung abhaengig.
//!
//! ⚑ **Eine Groesse, die zwei Schichten brauchen, gehoert unter beide.**
//! Der Scheduler haette sonst die ganze Wirtschaft einbinden muessen, um
//! ein Verhaeltnis auszurechnen, oder die Skala abschreiben, und eine
//! abgeschriebene Konsenskonstante ist eine zweite Wahrheit.
//!
//! # ⚑ Was **nicht** mitgekommen ist, und warum
//!
//! `utilization_to_f64` und `utilization_from_f64` sind in
//! `myl-tokenomics` geblieben. Sie rechnen mit Gleitkomma und dienen der
//! Anzeige; diese Kiste liegt im Konsenspfad des Gleitkomma-Audits, und
//! dort hat kein `f64` etwas zu suchen.
//!
//! **Der Schnitt faellt damit genau auf die Konsensgrenze**: Was
//! gerechnet wird, liegt hier; was angezeigt wird, liegt oben.

/// Festkomma-Skala der Auslastung: `1,0 = 2^16 = 65 536`.
pub const AUSLASTUNG_SKALA: i64 = 1 << 16;

/// Die Auslastung aus Nachfrage und Kapazitaet.
///
/// ⚑ **Ohne Kapazitaet ist die Auslastung null und nicht unendlich.**
/// Kein Pod heisst nicht „ueberlastet", sondern „nicht messbar", und
/// null ist die Angabe, die keine Folgeentscheidung verzerrt: Ein
/// Scheduler, der hier Ueberlast laese, drosselte das Training eines
/// Netzes, das gar keines hat.
pub fn auslastung(nachgefragt_vtfe: u64, kapazitaet_vtfe: u64) -> i64 {
    if kapazitaet_vtfe == 0 {
        return 0;
    }
    ((nachgefragt_vtfe as u128 * AUSLASTUNG_SKALA as u128) / kapazitaet_vtfe as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_eckwerte_stimmen() {
        assert_eq!(auslastung(0, 1_000), 0, "keine Nachfrage");
        assert_eq!(auslastung(1_000, 1_000), AUSLASTUNG_SKALA, "volle Auslastung");
        assert_eq!(auslastung(500, 1_000), AUSLASTUNG_SKALA / 2, "die Haelfte");
        assert_eq!(auslastung(2_000, 1_000), AUSLASTUNG_SKALA * 2, "Uebernachfrage");
    }

    #[test]
    fn ohne_kapazitaet_ist_sie_null() {
        assert_eq!(auslastung(1_000, 0), 0);
        assert_eq!(auslastung(u64::MAX, 0), 0);
    }

    /// ⚑ **Die u128-Zwischenrechnung traegt den Extremfall.** Mit `u64`
    /// liefe `nachgefragt * 65536` schon weit vorher ueber.
    #[test]
    fn der_extremfall_laeuft_nicht_ueber() {
        assert_eq!(auslastung(u64::MAX, u64::MAX), AUSLASTUNG_SKALA);
        let _ = auslastung(u64::MAX, 1);
    }
}
