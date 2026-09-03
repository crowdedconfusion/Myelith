//! Ratenbegrenzung gegen das Abtasten und gegen die Erschöpfung
//! (Stufe 2).
//!
//! # ⚑ Zwei Grenzen, und die zweite ist die, die gefehlt hat
//!
//! **Die naheliegende** ist die je Kontrakt: Ein Anfragender soll die
//! Tür nicht für alle anderen belegen.
//!
//! ⚑ **Die wichtigere ist die davor.** Eine Unterschrift zu prüfen
//! kostet eine Paarung, und eine Paarung kostet mehr als das Byte, das
//! sie auslöst. Wer Unsinn schickt, zwingt das Gateway zu genau dieser
//! Rechnung: **ein paar hundert Bytes hinein, Millisekunden
//! Rechenzeit hinaus.** Ohne eine Grenze **vor** der Prüfung ist die
//! Prüfung selbst der Angriff.
//!
//! Dieselbe Klasse wie Fund 141, nur mit Rechenzeit statt Bandbreite:
//! Ein Deckel gehört **vor** die teure Arbeit, nicht dahinter.
//!
//! # ⚑ Warum je Kontrakt und nicht je Verbindung
//!
//! Eine Grenze je Adresse der Gegenstelle ist bei einem Dienst, der auf
//! dem offenen Internet hört, wenig wert: Adressen sind billig. Eine
//! Grenze je **geprüftem** Kontrakt kann man nicht umgehen, ohne einen
//! Kontrakt zu haben.
//!
//! ⚑ **Und sie zählt erst nach der Prüfung.** Zählte sie vorher, könnte
//! jeder die Rate eines fremden Kontrakts aufbrauchen, indem er dessen
//! Sitzungsnummer nennt: eine Sperre, die man gegen andere richten
//! kann, ist eine Waffe und keine Grenze.
//!
//! # Was diese Stufe nicht leistet
//!
//! **Keine Verteilung über mehrere Gateways.** Jedes zählt für sich;
//! wer zehn Gateways benutzt, hat zehnmal die Rate. Das zu ändern
//! hiesse, den Zähler in den Konsens zu legen, und dort gehört er
//! nicht hin: Er ist eine Betriebsgrösse und keine Protokollaussage.
//!
//! **Keine Uhr im reinen Teil.** Die Zeit kommt als Parameter herein,
//! damit die Prüfung ohne Warten testbar ist.

use std::collections::BTreeMap;

use myl_types::ids::SitzungId;

/// Wie viele Anfragen ein Kontrakt je Fenster stellen darf.
///
/// ⚑ **Eine Anfrage ist eine Inferenz**, also Sekunden Arbeit für ein
/// ganzes Pod. Sechzig je Minute wäre keine Grenze, sondern eine
/// Einladung; sechs sind reichlich für einen Menschen und knapp für
/// eine Schleife.
pub const ANFRAGEN_JE_FENSTER: u32 = 6;

/// Wie viele Unterschriftsprüfungen die Tür insgesamt je Fenster
/// vornimmt.
///
/// ⚑ **Die Grenze vor der teuren Arbeit, und die Zahl ist gemessen.**
/// Der Test `die_pruefgrenze_greift_vor_der_unterschrift` fährt genau
/// dieses Fenster voll und braucht dafür **2,7 Sekunden**, also rund
/// **0,45 Millisekunden je Paarung**. Sechstausend je Minute sind damit
/// etwa **fünf Prozent eines Kerns**: eine Last, die ein Gateway
/// nebenbei trägt und die niemand ihm aufzwingen kann.
///
/// ⚑ **Der Test ist zugleich die Begründung der Grenze.** Er ist der
/// langsamste im Crate, und das ist kein Mangel: Er misst, was ein
/// Angreifer sonst umsonst bekäme.
pub const PRUEFUNGEN_JE_FENSTER: u32 = 6_000;

/// Die Länge eines Fensters in Millisekunden.
pub const FENSTER_MS: u64 = 60_000;

/// Wie viele Kontrakte gleichzeitig gezählt werden.
///
/// ⚑ **Sonst wäre der Zähler selbst der Angriff.** Eine Karte, die für
/// jede gesehene Sitzungsnummer einen Eintrag anlegt, wächst mit dem,
/// was der Angreifer schickt. Dieselbe Klasse wie Fund 144.
///
/// **Ist die Karte voll, wird abgewiesen und nicht verdrängt.** Wer
/// verdrängt, lässt sich den ältesten Eintrag herausdrücken und damit
/// die Grenze eines anderen aufheben.
pub const MAX_GEZAEHLTE_KONTRAKTE: usize = 4_096;

/// Der Zähler.
#[derive(Debug, Default)]
pub struct Takt {
    /// Beginn des laufenden Fensters, in Millisekunden.
    fenster_ab: u64,
    /// Prüfungen im laufenden Fenster, über alle Anfragenden.
    pruefungen: u32,
    /// Anfragen je Kontrakt im laufenden Fenster.
    je_kontrakt: BTreeMap<SitzungId, u32>,
}

impl Takt {
    /// Ein frischer Zähler.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Darf jetzt eine Unterschrift geprüft werden?
    ///
    /// ⚑ **Vor der Prüfung zu rufen und nicht danach.** Der Zweck ist,
    /// die Prüfung zu verhindern, nicht sie zu zählen.
    ///
    /// Zählt den Versuch mit, **auch wenn er scheitert**: Wer über der
    /// Grenze liegt, soll durch weiteres Klopfen nicht wieder
    /// hineinkommen.
    pub fn darf_pruefen(&mut self, jetzt_ms: u64) -> bool {
        self.fenster_pflegen(jetzt_ms);
        if self.pruefungen >= PRUEFUNGEN_JE_FENSTER {
            return false;
        }
        self.pruefungen += 1;
        true
    }

    /// Darf dieser Kontrakt jetzt eine Anfrage stellen?
    ///
    /// ⚑ **Erst nach geprüfter Unterschrift zu rufen.** Vorher wäre die
    /// Sitzungsnummer eine Behauptung, und jeder könnte die Rate eines
    /// fremden Kontrakts aufbrauchen.
    pub fn darf_anfragen(&mut self, sitzung: SitzungId, jetzt_ms: u64) -> bool {
        self.fenster_pflegen(jetzt_ms);
        match self.je_kontrakt.get_mut(&sitzung) {
            Some(n) => {
                if *n >= ANFRAGEN_JE_FENSTER {
                    return false;
                }
                *n += 1;
                true
            }
            None => {
                if self.je_kontrakt.len() >= MAX_GEZAEHLTE_KONTRAKTE {
                    // Voll: abweisen, nicht verdrängen. Siehe
                    // [`MAX_GEZAEHLTE_KONTRAKTE`].
                    return false;
                }
                self.je_kontrakt.insert(sitzung, 1);
                true
            }
        }
    }

    /// Wie viele Prüfungen im laufenden Fenster noch frei sind.
    pub fn freie_pruefungen(&self) -> u32 {
        PRUEFUNGEN_JE_FENSTER.saturating_sub(self.pruefungen)
    }

    /// Setzt das Fenster weiter, wenn es abgelaufen ist.
    ///
    /// **Ein springendes Fenster, kein gleitendes.** Ein gleitendes
    /// bräuchte je Kontrakt eine Liste von Zeitpunkten, und die wächst
    /// mit der Rate, die sie begrenzen soll.
    fn fenster_pflegen(&mut self, jetzt_ms: u64) {
        if jetzt_ms.saturating_sub(self.fenster_ab) >= FENSTER_MS {
            self.fenster_ab = jetzt_ms;
            self.pruefungen = 0;
            self.je_kontrakt.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sitzung(b: u8) -> SitzungId {
        SitzungId::new([b; 32])
    }

    /// ⚑ **Die Grenze steht vor der teuren Arbeit** (der eigentliche
    /// Zweck).
    #[test]
    fn die_pruefungen_sind_gedeckelt() {
        let mut t = Takt::neu();
        for i in 0..PRUEFUNGEN_JE_FENSTER {
            assert!(t.darf_pruefen(1_000), "Pruefung {i} wurde abgewiesen");
        }
        assert!(
            !t.darf_pruefen(1_000),
            "die {}. Pruefung ging durch",
            PRUEFUNGEN_JE_FENSTER + 1
        );
        assert_eq!(t.freie_pruefungen(), 0);
    }

    /// Ein neues Fenster gibt frei, ein laufendes nicht.
    #[test]
    fn das_fenster_springt_und_gleitet_nicht() {
        let mut t = Takt::neu();
        for _ in 0..PRUEFUNGEN_JE_FENSTER {
            assert!(t.darf_pruefen(0));
        }
        assert!(!t.darf_pruefen(FENSTER_MS - 1), "kurz vor Schluss wurde freigegeben");
        assert!(t.darf_pruefen(FENSTER_MS), "das neue Fenster gab nicht frei");
    }

    /// ⚑ **Weiteres Klopfen hilft nicht.** Wer über der Grenze liegt,
    /// bleibt es, auch wenn er es oft versucht.
    #[test]
    fn wer_ueber_der_grenze_liegt_klopft_sich_nicht_hinein() {
        let mut t = Takt::neu();
        for _ in 0..PRUEFUNGEN_JE_FENSTER {
            t.darf_pruefen(0);
        }
        for _ in 0..100 {
            assert!(!t.darf_pruefen(0));
        }
        assert!(t.darf_pruefen(FENSTER_MS), "das Fenster hat sich nicht erholt");
    }

    /// Je Kontrakt gezählt, und die Kontrakte stören einander nicht.
    #[test]
    fn jeder_kontrakt_hat_seine_eigene_rate() {
        let mut t = Takt::neu();
        for _ in 0..ANFRAGEN_JE_FENSTER {
            assert!(t.darf_anfragen(sitzung(1), 0));
        }
        assert!(!t.darf_anfragen(sitzung(1), 0), "der erste kam ueber seine Rate");
        assert!(
            t.darf_anfragen(sitzung(2), 0),
            "ein anderer Kontrakt wurde mitgesperrt"
        );
    }

    /// ⚑ **Die Karte wächst nicht unbegrenzt**, sonst wäre der Zähler
    /// selbst der Angriff (Klasse von Fund 144).
    #[test]
    fn die_karte_ist_begrenzt_und_verdraengt_nicht() {
        let mut t = Takt::neu();
        for i in 0..MAX_GEZAEHLTE_KONTRAKTE {
            let mut roh = [0u8; 32];
            roh[..8].copy_from_slice(&(i as u64).to_le_bytes());
            assert!(t.darf_anfragen(SitzungId::new(roh), 0), "Eintrag {i} ging nicht");
        }
        // Voll: der naechste wird abgewiesen.
        assert!(
            !t.darf_anfragen(sitzung(0xFF), 0),
            "die volle Karte nahm noch einen Eintrag an"
        );
        // ⚑ **Und der erste zählt weiter, wo er stand.** Wer
        // verdraengte, liesse sich die Grenze eines anderen aufheben:
        // Der Verdraengte finge wieder bei null an.
        let mut erster = [0u8; 32];
        erster[..8].copy_from_slice(&0u64.to_le_bytes());
        let erster = SitzungId::new(erster);
        // Er steht bei eins, also gehen noch fuenf.
        for i in 1..ANFRAGEN_JE_FENSTER {
            assert!(t.darf_anfragen(erster, 0), "Anfrage {i} des ersten ging nicht");
        }
        assert!(
            !t.darf_anfragen(erster, 0),
            "der erste Eintrag wurde verdraengt und faengt wieder bei null an"
        );
    }
}
