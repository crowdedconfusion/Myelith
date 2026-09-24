//! **Schrift aus einem Dokument, das kein Text ist.**
//!
//! Heute heisst das: PDF. ⚑ **Es liegt hier und nicht beim
//! Recherchewerkzeug**, weil derselbe Handgriff auch fuer einen Anhang
//! gebraucht wird: Ein Mensch haengt ein PDF an und will, dass das
//! Modell es liest. Ein Gegenstand, ein Ort.
//!
//! # ⚑ Warum ein fremdes Programm und keine eigene Zerlegung
//!
//! Ein PDF traegt seinen Text in Stroemen, die fast immer gepackt sind,
//! und die Zuordnung von Zeichen zu Buchstaben haengt an eingebetteten
//! Schriften, an CID-Tabellen und an `ToUnicode`. Das ist keine
//! Nachmittagsarbeit, und ein halb richtiger Auszug ist schlimmer als
//! keiner: Er sieht aus wie Text.
//!
//! ⛔️ **Fehlt jeder Weg, fehlt die Funktion und nicht der Bau**
//! (dieselbe Regel wie bei ffmpeg und beim Sehprogramm). Die Meldung
//! sagt dann, was zu tun ist.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Wie lange ein Auszug hoechstens dauern darf.
pub const FRIST_S: u64 = 60;

/// Welcher Weg gefunden wurde.
///
/// ⚑ **Die Reihenfolge ist die Rangfolge.** Gemessen an einem
/// Artikel von 8,7 MB: `fitz` 53 682 Zeichen in 0,1 s, `pypdf` 51 426
/// in 0,2 s. `pdftotext` steht trotzdem vorn, weil es ein eigenes
/// Programm ist und keine Python-Umgebung voraussetzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schriftweg {
    /// `pdftotext` aus poppler.
    Pdftotext(PathBuf),
    /// `mutool` aus mupdf.
    Mutool(PathBuf),
    /// Ein Python mit `fitz` oder `pypdf`.
    Python(PathBuf),
}

/// Was gebraucht wird, um ein PDF zu lesen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schriftzeug {
    pub weg: Schriftweg,
}

/// ⚑ **Ein Programm, das beide Bibliotheken kennt und die bessere
/// nimmt.** Es steht hier und nicht als Datei im Baum, damit es zur
/// Laufzeit weder fehlen noch bearbeitet werden kann.
const PYTHON_AUSZUG: &str = r#"
import sys
pfad = sys.argv[1]
text = None
try:
    import fitz
    with fitz.open(pfad) as d:
        text = "\n".join(s.get_text() for s in d)
except ImportError:
    pass
except Exception as f:
    sys.stderr.write("fitz: %s\n" % f)
if text is None:
    try:
        from pypdf import PdfReader
        text = "\n".join((s.extract_text() or "") for s in PdfReader(pfad).pages)
    except ImportError:
        sys.stderr.write("weder fitz noch pypdf\n")
        sys.exit(3)
sys.stdout.write(text)
"#;

impl Schriftzeug {
    /// Baut den Befehl, der die Schrift herausholt.
    ///
    /// ⚑ **Getrennt vom Ausfuehren, wie bei `sehen` und `sprechen`**:
    /// Das Zusammenspiel mit einem echten Programm wird nie geprueft,
    /// die Argumente dagegen bei jedem Lauf.
    pub fn befehl_fuer(&self, pdf: &Path) -> Command {
        match &self.weg {
            Schriftweg::Pdftotext(p) => {
                let mut b = Command::new(p);
                // `-layout` haelt Spalten auseinander, `-` schreibt
                // nach stdout, `--` schuetzt einen Pfad mit Strich.
                b.arg("-layout").arg("-q").arg("-enc").arg("UTF-8").arg("--").arg(pdf).arg("-");
                b
            }
            Schriftweg::Mutool(p) => {
                let mut b = Command::new(p);
                b.arg("draw").arg("-F").arg("txt").arg("-o").arg("-").arg(pdf);
                b
            }
            Schriftweg::Python(p) => {
                let mut b = Command::new(p);
                b.arg("-c").arg(PYTHON_AUSZUG).arg(pdf);
                b
            }
        }
    }

    /// Holt die Schrift heraus.
    ///
    /// `grenze` ist die Hoechstzahl Zeichen, die behalten werden.
    pub fn auszug(&self, pdf: &Path, grenze: usize) -> Result<String, String> {
        let mut befehl = self.befehl_fuer(pdf);
        let a = crate::prozess::laufen(&mut befehl, FRIST_S, grenze)
            .map_err(|f| format!("der Schriftauszug lief nicht: {f}"))?;
        if !a.gut() {
            // ⚑ **Nur der Fehlerstrom**, denn stdout traegt bei allen
            //   drei Wegen den Text und nicht den Grund.
            return Err(format!(
                "der Schriftauszug ist {}: {}",
                a.kopf(),
                crate::schwanz(&a.fehler, 5)
            ));
        }
        Ok(a.aus)
    }
}
