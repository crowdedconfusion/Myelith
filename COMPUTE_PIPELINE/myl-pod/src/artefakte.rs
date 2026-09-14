//! Der Wächter vor den Tests, die echte Modellartefakte brauchen.
//!
//! ⚑ **Seit dem 2026-09-03 in der Bibliothek statt im Testmodul**, weil
//! ihn eine zweite Kiste braucht: Der Test von der Türklinke bis zum
//! Modell liegt in `myl-testclient`, und ein Testmodul ist von dort
//! nicht erreichbar. Zwei Wächter wachen irgendwann verschieden.
//!
//! # ⚑ Fund 113: Elf Tests sprangen still ab und meldeten „ok"
//!
//! In diesen vier Testdateien stand elfmal dasselbe:
//!
//! ```ignore
//! if !dir.exists() {
//!     eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
//!     return;
//! }
//! ```
//!
//! **`cargo test` fängt die Standardfehlerausgabe bestandener Tests
//! ab.** Die Zeile war also nur mit `--nocapture` zu sehen, und ohne sie
//! meldete `pod_e2e` „8 passed", ohne ein einziges Gewicht angefasst zu
//! haben. Das sind die Tests, die die **Bitgleichheit** belegen, also
//! genau das Versprechen dieses Projekts.
//!
//! ⚑ **Gefährlich wurde das erst durch eine anstehende Entscheidung:**
//! `INTEGER_LLM/artifacts/` belegt 42 GB. Wer sie wegräumt, um Platz zu
//! gewinnen, bekommt danach **eine grüne Suite, die nichts mehr prüft**,
//! und nichts sagt ihm das.
//!
//! Dieselbe Klasse wie „eine Zählung, die null zählt, ist kein Befund".
//!
//! # Die Regel
//!
//! **Fehlen die Artefakte, schlägt der Test fehl**, mit einem Satz, der
//! sagt, was zu tun ist. Wer sie bewusst nicht hat, setzt
//! `MYL_OHNE_ARTEFAKTE=1`; dann wird übersprungen, aber **absichtlich
//! und nachlesbar**. Die CI setzt die Variable, weil dort keine
//! Artefakte liegen, und das steht dort auch so.

use std::path::Path;

/// Name der Variable, mit der ein Lauf ohne Artefakte erlaubt wird.
pub const OHNE: &str = "MYL_OHNE_ARTEFAKTE";

/// Sind die Artefakte da? `false` heißt: überspringen ist erlaubt.
///
/// # Panics
///
/// Wenn das Verzeichnis fehlt und [`OHNE`] **nicht** gesetzt ist. Das
/// ist der ganze Zweck: Ein stiller Sprung sieht aus wie ein bestandener
/// Test.
pub fn vorhanden(dir: &Path) -> bool {
    // 📌 **Fund 218: Diese Abfrage stand unter der Pfadpruefung**, und
    // damit war der Schalter auf jeder Maschine wirkungslos, die die
    // Artefakte **hat**. Gemeint war er fuer zwei Leser: die CI, wo
    // nichts liegt, und den Entwickler, der waehrend einer Messung
    // keine Rechenzeit an eine Pruefsammlung abgeben will. Nur der
    // erste wurde bedient. Aufgefallen, als `MYL_OHNE_ARTEFAKTE=1
    // cargo test` neben einem laufenden Training doch das 4B-Modell
    // lud und 59 Sekunden rechnete. Der Schalter heisst „ohne
    // Artefakte" und bedeutet jetzt genau das, unabhaengig davon, ob
    // welche da sind.
    if std::env::var_os(OHNE).is_some() {
        eprintln!("SKIP ({OHNE} gesetzt): {dir:?}");
        return false;
    }
    if dir.exists() {
        return true;
    }
    panic!(
        "Artefakte fehlen: {dir:?}\n\
         Dieser Test belegt die Bitgleichheit und kann ohne Modell nichts belegen.\n\
         Entweder die Artefakte bauen (INTEGER_LLM/pipeline), ein anderes Modell\n\
         wählen (MYL_POD_MODELL=...), oder den Sprung ausdrücklich erlauben:\n\
         {OHNE}=1 cargo test"
    );
}

#[cfg(test)]
mod pruefungen {
    use super::*;

    /// ⚑ **Diese Kiste hatte für `vorhanden` keine einzige Prüfung**,
    /// und deshalb konnte Fund 218 zwei Monate unbemerkt daliegen. Was
    /// hier geprüft wird, ist keine Rechnung, sondern eine Zusage an den
    /// Aufrufer, und die kostet nichts.
    ///
    /// 📌 Die Umgebungsvariable ist prozessweit; deshalb steht alles in
    /// **einer** Prüfung und nicht in dreien. Drei Prüfungen liefen
    /// nebenläufig im selben Prozess und setzten sich gegenseitig die
    /// Variable um, und das Ergebnis hinge an der Reihenfolge.
    #[test]
    fn der_schalter_wirkt_auch_wenn_die_artefakte_da_sind() {
        let da = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let weg = std::path::Path::new("/dieses/verzeichnis/gibt/es/nicht");

        // Ohne Schalter: Was da ist, ist da.
        std::env::remove_var(OHNE);
        assert!(vorhanden(da), "ein vorhandenes Verzeichnis ist vorhanden");

        // Ohne Schalter: Was fehlt, bricht ab, statt still zu springen.
        let ausgang = std::panic::catch_unwind(|| vorhanden(weg));
        assert!(
            ausgang.is_err(),
            "ohne Schalter muss ein fehlendes Verzeichnis abbrechen, \
             sonst sieht ein übersprungener Lauf aus wie ein bestandener"
        );

        // 📌 Der Kern von Fund 218: mit Schalter wird auch dann
        // gesprungen, wenn die Artefakte **daliegen**. Vorher stand
        // diese Abfrage unter der Pfadprüfung und kam nie zum Zuge.
        std::env::set_var(OHNE, "1");
        assert!(
            !vorhanden(da),
            "der Schalter heißt „ohne Artefakte“ und muss auch mit welchen greifen"
        );
        assert!(!vorhanden(weg), "und ohne welche erst recht");
        std::env::remove_var(OHNE);
    }
}
