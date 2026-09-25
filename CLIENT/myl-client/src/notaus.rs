//! **Der Notaus**: haelt jeden laufenden Auftrag an.
//!
//! # ⚑ Was er tut
//!
//! Ein Schalter je Prozess. Steht er,
//!
//! 1. endet die Erzeugung des Modells vor dem naechsten Token, und
//!    `chat` meldet [`myl_local_agent::Tuerfehler::Abgebrochen`] mit dem
//!    Text bis dahin, damit die Agentenschleife endet und nichts verloren
//!    geht;
//! 2. wird jedes weitere Werkzeug verweigert und als `abgebrochen`
//!    protokolliert.
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

/// **Loest den Notaus aus** und protokolliert es.
pub fn ausloesen(stelle: &str) {
    SCHALTER.store(true, Ordering::SeqCst);
    crate::protokoll::ereignis("notaus", stelle, &[], "abgebrochen");
}

/// **Nur der Schalter, ohne Protokoll**, fuer einen Signalbearbeiter: Dort
/// darf nichts geschehen als ein atomares Schreiben.
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
