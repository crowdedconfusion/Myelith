//! Der Wächter vor den Tests, die echte Modellartefakte brauchen.
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
    if dir.exists() {
        return true;
    }
    if std::env::var_os(OHNE).is_some() {
        eprintln!("SKIP ({OHNE} gesetzt): Artefakte fehlen: {dir:?}");
        return false;
    }
    panic!(
        "Artefakte fehlen: {dir:?}\n\
         Dieser Test belegt die Bitgleichheit und kann ohne Modell nichts belegen.\n\
         Entweder die Artefakte bauen (INTEGER_LLM/pipeline), ein anderes Modell\n\
         wählen (MYL_POD_MODELL=...), oder den Sprung ausdrücklich erlauben:\n\
         {OHNE}=1 cargo test"
    );
}
