//! Was mit einer angenommenen Anfrage geschieht: festschreiben.
//!
//! # ⚑ Der Beleg ist das Produkt
//!
//! Ein Gateway, das eine Antwort liefert und sonst nichts, ist ein
//! Weiterleiter. **Was Myelith anders macht, ist der Beleg**: Der Nutzer
//! bekommt eine Zusicherung, gegen die er später prüfen kann, dass die
//! Arbeit geleistet und bezeugt wurde.
//!
//! # ⚑ Und ohne die Bindung der Anfrage geht auch Stufe 2 nicht
//!
//! Am 2026-09-01 fiel auf, dass der **Prompt im Konsens nicht vorkam**.
//! Damit könnte ein Checker nicht nachrechnen: Er müsste den Prompt dem
//! Pod glauben und prüfte dann, ob der Pod zu seiner **eigenen** Eingabe
//! passt. Eine Frage, auf die der Gefragte beide Hälften wählt.
//!
//! Deshalb schreibt das Gateway **zuerst** fest und leitet **dann**
//! weiter, nicht umgekehrt.

use myl_types::ids::EpochId;
use myl_types::sitzung::Anfragebindung;

/// Warum eine Anfrage nicht angenommen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Annahmefehler {
    /// Eine leere Anfrage ist keine.
    ///
    /// ⚑ **Sie stillschweigend anzunehmen wäre teuer:** Sie bekäme eine
    /// Sitzungsnummer, eine Bindung und einen Platz in der Stichprobe,
    /// und niemand hätte etwas gefragt.
    Leer,
}

/// Der Beleg, den der Nutzer bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Beleg {
    /// Die Sitzung, unter der die Anfrage läuft.
    pub sitzung: u64,
    /// Die Bindung der Anfrage.
    ///
    /// ⚑ **Sie enthält den Hash, nicht den Text.** Der Nutzer hat den
    /// Text ohnehin; was ihm fehlt, ist die Zusicherung, dass **dieser**
    /// Text die Arbeit ausgelöst hat.
    pub bindung: Anfragebindung,
}

/// Die Annahmestelle: vergibt Sitzungsnummern und bindet Anfragen.
#[derive(Debug)]
pub struct Annahme {
    naechste_sitzung: u64,
    epoche: EpochId,
}

impl Annahme {
    /// Neu, ab einer Sitzungsnummer und einer Epoche.
    ///
    /// ⚑ **Die Nummer beginnt nicht bei null**, sondern beim übergebenen
    /// Wert: Ein Neustart, der wieder bei null anfinge, vergäbe Nummern
    /// zweimal, und zwei Anfragen mit derselben Nummer wären für die
    /// Stichprobe dieselbe Sitzung.
    pub fn neu(ab_sitzung: u64, epoche: EpochId) -> Self {
        Self {
            naechste_sitzung: ab_sitzung,
            epoche,
        }
    }

    /// Die Epoche wechseln.
    pub fn epoche_setzen(&mut self, epoche: EpochId) {
        self.epoche = epoche;
    }

    /// Nimmt eine Anfrage an und schreibt sie fest.
    pub fn annehmen(&mut self, anfrage: &[u8]) -> Result<Beleg, Annahmefehler> {
        if anfrage.is_empty() {
            return Err(Annahmefehler::Leer);
        }
        let sitzung = self.naechste_sitzung;
        self.naechste_sitzung = self.naechste_sitzung.saturating_add(1);
        Ok(Beleg {
            sitzung,
            bindung: Anfragebindung::neu(sitzung, anfrage, self.epoche),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_anfrage_bekommt_einen_beleg_der_zu_ihr_passt() {
        let mut a = Annahme::neu(1, EpochId(4));
        let b = a.annehmen(b"was ist ein pod").expect("angenommen");
        assert_eq!(b.sitzung, 1);
        assert!(b.bindung.passt(b"was ist ein pod"));
        assert!(!b.bindung.passt(b"etwas anderes"));
    }

    /// ⚑ **Zwei Anfragen bekommen zwei Sitzungen.**
    ///
    /// Sonst waeren sie fuer die Stichprobe dieselbe, und eine Ziehung
    /// deckte zwei Arbeiten mit einer Pruefung ab.
    #[test]
    fn zwei_anfragen_bekommen_zwei_sitzungen() {
        let mut a = Annahme::neu(1, EpochId(4));
        let x = a.annehmen(b"eins").expect("angenommen");
        let y = a.annehmen(b"eins").expect("angenommen");
        assert_ne!(x.sitzung, y.sitzung);
        // ⚑ Und **derselbe Text** ergibt zwei verschiedene Bindungen,
        // weil die Sitzungsnummer in den Hash eingeht.
        assert_ne!(x.bindung.anfrage_hash, y.bindung.anfrage_hash);
    }

    /// ⚑ **Eine leere Anfrage ist keine.**
    #[test]
    fn eine_leere_anfrage_wird_abgewiesen() {
        let mut a = Annahme::neu(1, EpochId(4));
        assert_eq!(a.annehmen(b""), Err(Annahmefehler::Leer));
        // Und sie verbraucht keine Sitzungsnummer.
        assert_eq!(a.annehmen(b"x").expect("angenommen").sitzung, 1);
    }

    /// ⚑ **Ein Neustart darf keine Nummer zweimal vergeben.**
    #[test]
    fn ein_neustart_setzt_die_nummer_fort() {
        let mut a = Annahme::neu(1, EpochId(4));
        a.annehmen(b"eins").expect("angenommen");
        a.annehmen(b"zwei").expect("angenommen");
        // Neustart mit der naechsten freien Nummer.
        let mut b = Annahme::neu(3, EpochId(4));
        assert_eq!(b.annehmen(b"drei").expect("angenommen").sitzung, 3);
    }

    /// Die Epoche steht in der Bindung und laesst sich fortschreiben.
    #[test]
    fn die_epoche_wandert_mit() {
        let mut a = Annahme::neu(1, EpochId(4));
        let x = a.annehmen(b"eins").expect("angenommen");
        a.epoche_setzen(EpochId(5));
        let y = a.annehmen(b"zwei").expect("angenommen");
        assert_eq!(x.bindung.epoche, EpochId(4));
        assert_eq!(y.bindung.epoche, EpochId(5));
    }
}
