//! `myl-testclient`. Terminal-Testclient für Myelith.
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
//! Fehlersuche. **Ohne Protokoll ist ein Testlauf wertlos**: er
//! beantwortet dann nicht, auf welcher Hardware, mit welchem Backend und
//! gegen welchen Modellstand gemessen wurde. Genau diese drei Angaben
//! entscheiden bei einem Modellwechsel darüber, ob ein verändertes
//! Ergebnis ein Fortschritt oder ein Fehler ist.
//!
//! ## Abgrenzung zu CLIENT
//!
//! `CLIENT/` ist der spätere Nutzer-Client (Wallet, Inferenz-Oberfläche,
//! Session-Kontrakte). Dieser hier ist ein **Diagnosewerkzeug für
//! Entwickler und Miner**: er kennt keine Konten, keine Zahlungen und
//! keine Netzwerkverbindung. Die Trennung ist bewusst: Ein
//! Diagnosewerkzeug darf laut, gesprächig und roh sein; ein
//! Nutzer-Client nicht.

//! Konsens-Regel wie ueberall in Myelith: kein `unsafe`. Fuer ein
//! Werkzeug, das Nachweise erhebt, gilt sie erst recht: Ein Nachweis
//! aus einem Programm mit undefiniertem Verhalten belegt nichts.

#![deny(unsafe_code)]

pub mod animation;
pub mod artefakte;
pub mod auswahl;
pub mod banner;
pub mod erwartung;
pub mod farben;
pub mod hardware;
pub mod knoten;
pub mod konformitaet;
pub mod menu;
pub mod netz;
pub mod modellstaende;
pub mod logging;
pub mod plaene;
pub mod runs;
pub mod spec;
pub mod stack;
pub mod vergleich;

pub use hardware::Fingerprint;
pub use logging::{sha256_hex, Event, LogZiel, RunLog, OHNE_NAME};
pub use runs::{default_artifact_dir, run_determinism, run_hardware, run_shard, DEFAULT_MODEL};
pub use spec::{PlanError, TestPlan};
pub use stack::run_stack;
pub use netz::{beurteile, sammle, Knotenbild, Urteil};
pub use vergleich::run as run_vergleich;

/// Standard-Verzeichnis für Laufprotokolle: `TESTCLIENT/logs/`.
///
/// Nicht in `/tmp`: Protokolle sollen einen Neustart überleben und beim
/// Aufräumen bewusst gelöscht werden.
///
/// **Neben dem Crate, nicht darin.** Bis v0.6.0 lagen sie unter
/// `TESTCLIENT/myl-testclient/logs/`, also zwei Ebenen tief in einem
/// Quellcodeverzeichnis. Wer sie verschicken soll, sucht sie dort, und
/// `myl-testclient/` ist für einen Teilnehmer, der nur eine Maschine
/// beisteuert, ein Ordner, in dem er nichts zu suchen hat. Jetzt liegen
/// sie neben `Testpläne/` und `Vergleiche/` auf derselben Ebene: die drei
/// Ordner, mit denen ein Teilnehmer zu tun hat, beieinander.
/// **Zur Laufzeit bestimmt, nicht beim Übersetzen.** Siehe
/// [`artefakte::wurzel_zur_laufzeit`]: Ein eingebackener Pfad zeigte ins
/// Leere, sobald das Repository verschoben oder das Binary weitergegeben
/// wurde.
pub fn default_log_dir() -> std::path::PathBuf {
    let gebaut = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    artefakte::wurzel_zur_laufzeit(&gebaut)
        .join("TESTCLIENT")
        .join("logs")
}
