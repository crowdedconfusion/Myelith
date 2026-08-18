//! `myl-testclient` — Terminal-Testclient für Myelith.
//!
//! Zwei Aufgaben, die im Projekt bisher niemand bedient hat:
//!
//! 1. **Hardwaretests auf heterogener Hardware.** Der
//!    Cross-Hardware-Determinismus-Nachweis (Whitepaper Kap. 6.2) ist
//!    im Projekt als offener Punkt geführt. Er verlangt, dass derselbe
//!    Prompt auf verschiedenen Architekturen und Backends **bitgleiche**
//!    Ergebnisse liefert. Dafür braucht es ein Werkzeug, das auf einer
//!    fremden Maschine ohne Einarbeitung läuft und ein vergleichbares
//!    Protokoll hinterlässt.
//! 2. **Die erste geshardete Inferenz sichtbar machen.** `myl-pod` kann
//!    einen Pod fahren, aber nur als Bibliothek und Integrationstest.
//!    Der Client macht daraus einen Befehl, dessen Ausgabe man einem
//!    Dritten zeigen kann.
//!
//! ## Der Kern: das Protokoll
//!
//! Jeder Lauf schreibt zwei Dateien ([`logging`]): eine maschinenlesbare
//! `.jsonl` für den Vergleich zwischen Maschinen und eine `.log` für die
//! Fehlersuche. **Ohne Protokoll ist ein Testlauf wertlos** — er
//! beantwortet dann nicht, auf welcher Hardware, mit welchem Backend und
//! gegen welchen Modellstand gemessen wurde. Genau diese drei Angaben
//! entscheiden bei einem Modellwechsel darüber, ob ein verändertes
//! Ergebnis ein Fortschritt oder ein Fehler ist.
//!
//! ## Abgrenzung zu CLIENT
//!
//! `CLIENT/` ist der spätere Nutzer-Client (Wallet, Inferenz-Oberfläche,
//! Session-Kontrakte). Dieser hier ist ein **Diagnosewerkzeug für
//! Entwickler und Miner** — er kennt keine Konten, keine Zahlungen und
//! keine Netzwerkverbindung. Die Trennung ist bewusst: Ein
//! Diagnosewerkzeug darf laut, gesprächig und roh sein; ein
//! Nutzer-Client nicht.

pub mod banner;
pub mod hardware;
pub mod menu;
pub mod logging;
pub mod runs;
pub mod spec;
pub mod stack;

pub use hardware::Fingerprint;
pub use logging::{sha256_hex, Event, LogZiel, RunLog};
pub use runs::{default_artifact_dir, run_determinism, run_hardware, run_shard, DEFAULT_MODEL};
pub use spec::{PlanError, TestPlan};
pub use stack::run_stack;

/// Standard-Verzeichnis für Laufprotokolle.
///
/// Unterhalb des Testclients, nicht in `/tmp`: Protokolle sollen einen
/// Neustart überleben und beim Aufräumen bewusst gelöscht werden.
pub fn default_log_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("logs")
}
