//! **Der Notaus**: haelt jeden laufenden Auftrag an.
//!
//! # ⚑ Was er tut
//!
//! Ein Schalter je Prozess. Steht er,
//!
//! 1. endet die Erzeugung des Modells vor dem naechsten Token, und
//!    `chat` meldet [`crate::Tuerfehler::Abgebrochen`] mit dem
//!    Text bis dahin, damit die Agentenschleife endet und nichts verloren
//!    geht;
//! 2. wird jedes weitere Werkzeug verweigert ([`Notausfest`]); laeuft der
//!    Debug-Log, steht der Notaus darin.
//!
//! ⚠️ **Was er nicht tut:** Ein Werkzeug, das gerade laeuft (etwa ein
//! Befehl), laeuft zu Ende, hoechstens bis zu seiner Frist. Es mitten im
//! Schreiben einer Datei zu toeten, hinterliesse eine halbe Datei, und
//! das ist der Zustand, den ein Notaus gerade vermeiden soll.
//!
//! ⚑ **Zurueckgesetzt wird er am Anfang jedes Auftrags**, von dem, der
//! den Auftrag startet; ein stehender Schalter wuerde sonst den naechsten
//! Auftrag gleich wieder anhalten.

use std::sync::atomic::{AtomicBool, Ordering};

static SCHALTER: AtomicBool = AtomicBool::new(false);

/// **Loest den Notaus aus**; laeuft der Debug-Log, steht er darin.
pub fn ausloesen(stelle: &str) {
    SCHALTER.store(true, Ordering::SeqCst);
    crate::debuglog::eintrag("Notaus", stelle);
}

/// **Nur der Schalter**, fuer einen Signalbearbeiter: Dort darf nichts
/// geschehen als ein atomares Schreiben.
pub fn ausloesen_still() {
    SCHALTER.store(true, Ordering::SeqCst);
}

/// Setzt ihn zurueck, am Anfang eines Auftrags.
pub fn zuruecksetzen() {
    SCHALTER.store(false, Ordering::SeqCst);
}

/// Ob er steht.
pub fn ausgeloest() -> bool {
    SCHALTER.load(Ordering::SeqCst)
}

/// Der Schalter selbst, fuer die Erzeugung.
pub fn schalter() -> &'static AtomicBool {
    &SCHALTER
}

/// **Ein Werkzeug, das nach dem Notaus nicht mehr laeuft.**
///
/// ⚑ **Es umhuellt, statt einzugreifen**: Das Werkzeug weiss nichts davon,
/// und was es tut, bleibt, wie es war. Jedes Werkzeug der Ruestung haengt
/// dahinter (`jedes_werkzeug_haengt_hinter_dem_notaus`).
pub struct Notausfest {
    pub inner: Box<dyn crate::ausfuehrung::Werkzeugausfuehrung>,
}

impl crate::ausfuehrung::Werkzeugausfuehrung for Notausfest {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn ausfuehren(&self, argumente: &serde_json::Value) -> Result<String, crate::ausfuehrung::Werkzeugfehler> {
        // ⛔️ Nach dem Notaus laeuft kein Werkzeug mehr.
        if ausgeloest() {
            crate::debuglog::eintrag("Notaus", &format!("{} nicht ausgefuehrt", self.inner.name()));
            return Err(crate::ausfuehrung::Werkzeugfehler {
                grund: "Notaus: Der Mensch hat den Auftrag angehalten. Nichts wurde ausgefuehrt; \
                        beende den Auftrag."
                    .to_string(),
            });
        }
        self.inner.ausfuehren(argumente)
    }
}

#[cfg(test)]
mod proben {
    /// ⛔️ **Kein Werkzeug haengt am Notaus vorbei**, und jedes steht unter
    /// Aufsicht des Informationsflusses (`crate::secure_flow`). Eine
    /// Einhaengung daneben liefe nach dem Notaus weiter oder ungeprueft.
    #[test]
    fn jedes_werkzeug_haengt_hinter_dem_notaus() {
        let ruestung = include_str!("ruestung.rs");
        for (i, zeile) in ruestung.lines().enumerate() {
            if zeile.contains(".einhaengen(") {
                assert!(zeile.contains("notausfest(bewacht"), "ruestung.rs:{}: {zeile}", i + 1);
            }
        }
        assert!(ruestung.matches("notausfest(bewacht").count() >= 11);
    }
}

