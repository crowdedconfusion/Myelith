//! Die Naht zum Sitzungskontrakt (AGENT_LAYER 5.2, zweite Hälfte).
//!
//! # ⚑ Was der Kontrakt hergibt, und was nicht
//!
//! Ein Werkzeugaufruf ist ein **Vorschlag des Modells**, und die
//! Erlaubnis kommt nicht aus dem Vorschlag. Der naheliegende Satz dazu
//! lautet „die Erlaubnis kommt aus dem Sitzungskontrakt", und beim
//! Bauen dieser Naht stellt sich heraus, dass er nur zur Hälfte stimmen
//! kann. Die Hälfte gehört benannt:
//!
//! | Der Kontrakt begrenzt | Der Kontrakt sagt **nichts** über |
//! |---|---|
//! | wer handeln darf (`agent`) | welche **Werkzeuge** es gibt |
//! | wie lange (`gueltig_ab`, `gueltig_bis`) | |
//! | wie viele Schritte (`max_schritte`) | |
//! | wie viel (`credits`, `myl`) | |
//! | an wen gezahlt werden darf (`empfaenger`) | |
//!
//! ⚑ **Er begrenzt Wirkungen, nicht Werkzeuge.** Eine Werkzeugliste
//! steht nirgends darin, und das ist richtig: Welche Werkzeuge es gibt,
//! ist eine Frage der Sitzung, nicht des Konsens. Was der Konsens
//! begrenzt, ist, **was dabei herauskommen darf**.
//!
//! Die Werkzeugliste kommt deshalb weiter vom Aufrufer (oder aus einem
//! `myl_agent::Plan`, der strengsten Form), und der Kontrakt legt sich
//! **darum herum**.
//!
//! # ⚑ Nichts wird nachgebaut
//!
//! [`myl_types::sitzung::pruefe`] ist die kanonische Prüfung: Sitzung,
//! Handelnder, Wiedereinreichung, Widerruf, Frist, Betrag, Empfänger.
//! Dieses Modul **ruft sie** und formuliert keine einzige dieser Regeln
//! neu.
//!
//! ⚑ **Der Unterschied zu einer zweiten Meinung ist, dass es dieselbe
//! ist.** Eine eigene Empfängerliste im Harness wäre eine zweite Quelle,
//! und die mildere wäre die geglaubte. Eine **frühe** Anwendung
//! derselben Quelle ist keine zweite Meinung, sondern ein früher
//! Abbruch: Der Nutzer erfährt vor der Arbeit, dass sie nicht bezahlt
//! werden kann, statt danach.

use myl_types::ids::EpochId;
use myl_types::sitzung::{
    pruefe, Agentenbefund, Sitzungskontrakt, Sitzungszustand, Vorhaben,
};

use crate::werkzeug::{Erlaubnis, Werkzeug};

/// Die Grenzen einer Sitzung, aus dem Kontrakt.
#[derive(Debug, Clone)]
pub struct Sitzungsgrenzen {
    kontrakt: Sitzungskontrakt,
    erlaubnis: Erlaubnis,
}

/// Warum ein Schritt nicht laufen darf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grenzfehler {
    /// Der Kontrakt lässt nicht mehr Schritte zu.
    ///
    /// ⚑ **Nicht dasselbe wie das Budget.** Ein Agent, der in einer
    /// Schleife nachschlägt, ohne je zu zahlen, verbraucht kein Budget
    /// und läuft trotzdem endlos; genau dafür steht `max_schritte` im
    /// Kontrakt.
    SchritteVerbraucht {
        /// Wie viele der Kontrakt zulässt.
        erlaubt: u32,
    },
    /// Das Zeitfenster ist zu, oder der Kontrakt gilt noch nicht.
    ///
    /// ⚑ **Ohne Grund**, wie [`Agentenbefund`] es vorsieht: Der Agent
    /// erfährt, dass es nicht geht, nicht warum. Die Begründung gehört
    /// dem Inhaber, nicht dem Agenten.
    NichtErlaubt,
}

impl std::fmt::Display for Grenzfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchritteVerbraucht { erlaubt } => write!(
                f,
                "der Sitzungskontrakt laesst {erlaubt} Schritte zu, und sie sind verbraucht"
            ),
            Self::NichtErlaubt => {
                f.write_str("der Sitzungskontrakt laesst dieses Vorhaben nicht zu")
            }
        }
    }
}

impl std::error::Error for Grenzfehler {}

impl Sitzungsgrenzen {
    /// Bindet eine Werkzeugliste an einen Kontrakt.
    ///
    /// ⚑ **Beides zusammen, und keins allein.** Die Liste sagt, **was**
    /// gerufen werden darf; der Kontrakt sagt, **wie oft, wie lange und
    /// mit welcher Wirkung**. Wer nur eines hätte, hätte eine halbe
    /// Erlaubnis und wüsste es nicht.
    pub fn neu(kontrakt: Sitzungskontrakt, werkzeuge: &[Werkzeug]) -> Self {
        Self { kontrakt, erlaubnis: Erlaubnis::aus_angebot(werkzeuge) }
    }

    /// Die Werkzeugerlaubnis.
    pub fn erlaubnis(&self) -> &Erlaubnis {
        &self.erlaubnis
    }

    /// Der Kontrakt.
    pub fn kontrakt(&self) -> &Sitzungskontrakt {
        &self.kontrakt
    }

    /// Darf ein weiterer Schritt laufen?
    ///
    /// `getan` ist die Zahl der **bereits** gelaufenen Schritte.
    pub fn schritt_erlaubt(&self, getan: u32) -> Result<(), Grenzfehler> {
        if getan >= self.kontrakt.max_schritte {
            return Err(Grenzfehler::SchritteVerbraucht {
                erlaubt: self.kontrakt.max_schritte,
            });
        }
        Ok(())
    }

    /// Darf dieses Vorhaben laufen?
    ///
    /// ⚑ **Ruft [`myl_types::sitzung::pruefe`] und entscheidet nichts
    /// selbst.** Der Rückgabewert ist bewusst [`Agentenbefund`] und
    /// nicht der volle Befund: Der Agent erfährt „nein", nicht warum.
    /// Wer ihm den Grund gäbe, gäbe ihm eine Sonde auf die Grenzen des
    /// Kontrakts.
    pub fn vorhaben_erlaubt(
        &self,
        zustand: &Sitzungszustand,
        jetzt: EpochId,
        vorhaben: &Vorhaben,
    ) -> Result<(), Grenzfehler> {
        match pruefe(&self.kontrakt, zustand, jetzt, vorhaben).fuer_agenten() {
            Agentenbefund::Erlaubt => Ok(()),
            _ => Err(Grenzfehler::NichtErlaubt),
        }
    }
}
