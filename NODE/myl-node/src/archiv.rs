//! **Archivknoten: wenige, ausgelost, rotierend** (Festlegungen des
//! Projektinhabers vom 2026-10-23).
//!
//! # ⚑ Wozu
//!
//! Jeder Knoten verdichtet seine Kette, sobald sie über ihr Budget wächst
//! (`Kette::verdichten_wenn_noetig`): Er behält alles ab einem
//! Langzeitabzug, die Blöcke davor fallen weg. Damit die Geschichte ab
//! Genesis trotzdem einen **Totalausfall aller Knoten** übersteht, behält
//! eine kleine, ausgeloste Gruppe sie ganz, und zwar auf der Platte: Läuft
//! nach einem Ausfall auch nur einer von ihnen wieder an, ist sie zurück.
//!
//! # ⚑ Wie gelost wird
//!
//! Je **Periode** (die Blöcke zwischen zwei Langzeitabzügen, 64 Epochen)
//! einmal, **im Konsenszustand**, beim Anwenden des Grenzblocks
//! (`myl_ledger::archiv::grenze_ziehen`), nach den Regeln von
//! [`myl_types::archivwahl`]: sieben Kopien über die Zonen verteilt, 64
//! Splitter getrennt gelost, teilnehmen darf nur, wer mindestens
//! [`WARTE_EPOCHEN`] vor der Grenze registriert war. Sind es weniger als
//! sieben, behält jeder alles; das gilt auch für ein Netz ohne Miner.
//!
//! ⚑ **Der Knoten liest seine Rolle aus dem Zustand und rechnet sie nicht
//! selbst.** Bis zur Stufe A4 zog er selbst aus den Abzügen; seit der
//! Ledger belegt, ersetzt und übergibt, wäre das eine zweite Wahrheit, die
//! nach dem ersten Nachrücken auseinanderliefe.
//!
//! # ⚑ Wie rotiert wird
//!
//! Die Kopien einer Periode werden an der nächsten Grenze **Halter** und
//! behalten, bis mindestens fünf der neuen Kopien ihre Vollständigkeit
//! belegt haben; vorher hält jeder. Die neuen holen sich die Geschichte
//! mit Beweisen ([`crate::archivabruf`]), bei den Haltern zuerst. Es gibt
//! keinen Augenblick ohne belegte vollständige Kopie.

use myl_consensus::block::BLOECKE_JE_EPOCHE;
use myl_ledger::archiv::Archivstand;
use myl_types::archivwahl::Archivrolle;
use myl_types::ids::MinerId;

/// Blöcke einer Periode: die Spanne zwischen zwei Langzeitabzügen.
pub const PERIODE_BLOECKE: u64 = BLOECKE_JE_EPOCHE * crate::abzug::LANGZEIT_EPOCHEN;

/// ⚑ **Wartezeit vor der Teilnahme: eine Periode.** Die Saat einer Grenze
/// steht zwei Epochen vorher fest; wer sich danach anmeldet, könnte seinen
/// Schlüssel auf sie hin ausprobieren. Eine ganze Periode Abstand schliesst
/// das aus und macht eine Anmeldung auf Vorrat teuer.
pub const WARTE_EPOCHEN: u64 = crate::abzug::LANGZEIT_EPOCHEN;

// ⚑ **Die Frist einer neuen Kopie passt in die Periode, auch für ihre
// Nachrückerin** (Fund 628): Frist, verfehlte Epochen und noch einmal Frist.
// Die Frist steht im Ledger, die Periode hier; wer eine der beiden ändert,
// erfährt es beim Übersetzen.
const _: () = assert!(
    2 * myl_ledger::archiv::FRIST_VOLLSTAENDIG_EPOCHEN + myl_ledger::archiv::VERFEHLT_HOECHSTENS as u64
        <= crate::abzug::LANGZEIT_EPOCHEN,
    "die Nachrückerin einer säumigen Kopie würde in ihrer Periode nicht mehr fertig"
);

/// **Die Rolle von `ich`** im Archivstand.
pub fn rolle(a: &Archivstand, ich: Option<&MinerId>) -> Archivrolle {
    if !a.gezogen {
        return Archivrolle::AlleBehalten;
    }
    match ich {
        Some(m) if a.kopien.contains(m) => Archivrolle::Kopie,
        Some(m) if a.halter.contains(m) => Archivrolle::Halter,
        Some(m) if a.splitter.contains(m) => Archivrolle::Splitter,
        _ if a.halter_alle => Archivrolle::AlleBehalten,
        _ => Archivrolle::Keine,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_rolle_folgt_aus_dem_stand() {
        let k = MinerId::new([1; 32]);
        let h = MinerId::new([2; 32]);
        let s = MinerId::new([3; 32]);
        let mut a = Archivstand::default();
        assert_eq!(rolle(&a, Some(&k)), Archivrolle::AlleBehalten);
        a.gezogen = true;
        a.kopien = vec![k];
        a.halter = vec![h];
        a.splitter = vec![s];
        assert_eq!(rolle(&a, Some(&k)), Archivrolle::Kopie);
        assert_eq!(rolle(&a, Some(&h)), Archivrolle::Halter);
        assert_eq!(rolle(&a, Some(&s)), Archivrolle::Splitter);
        assert_eq!(rolle(&a, None), Archivrolle::AlleBehalten, "vor der ersten Übergabe hält jeder");
        a.halter_alle = false;
        assert_eq!(rolle(&a, None), Archivrolle::Keine);
    }
}
