//! **Wo dieses Repositorium liegt**, soweit es diese Kiste angeht.
//!
//! # ⚑ Warum die Wurzelsuche hier unten steht und nicht beim Client
//!
//! Sie stand bis zum 2026-09-21 allein in `myl-client`, und das war
//! richtig, solange nur der Client sie brauchte. Seit die fremden
//! Gewichte unter `MODELS/` im Klon liegen, braucht sie auch diese
//! Kiste: Ohne die Wurzel findet sie kein Sehmodell.
//!
//! ⛔️ **Die naheliegende Loesung waere die falsche gewesen.** Ein
//! zweites `MARKE` hier haette dieselbe Zeichenkette an einem zweiten
//! Ort bedeutet, und **was an zwei Orten steht, laeuft auseinander**:
//! Wer die Marke einmal aendert, aendert sie an einem der beiden, und
//! der andere sucht danach stillschweigend weiter nach einer Datei, die
//! es nicht mehr gibt. Also steht sie **einmal**, und zwar hier, in der
//! Kiste ohne Abhaengigkeiten, auf die der Client ohnehin zeigt.
//!
//! # ⚑ Was hier NICHT steht
//!
//! Der **gemerkte Ort**. `myl-client` schreibt den gefundenen Ort neben
//! seine Einstellungsdatei und liest ihn, wenn beide Suchwege leer
//! ausgehen. Das braucht den Begriff einer Einstellungsdatei, und den
//! hat diese Kiste nicht. **Was ermittelt wird, steht hier; was gemerkt
//! wird, steht dort.**

use std::path::{Path, PathBuf};

/// Die Datei, an der ein Verzeichnis als Wurzel dieses Repositoriums
/// zu erkennen ist.
///
/// ⚑ **Eine Datei, die es nur hier gibt, und die niemand nebenbei
/// anlegt.** `.git` waere jeder Klon, `README.md` jedes Projekt.
pub const MARKE: &str = "INTEGER_LLM/scripts/build_artifacts.sh";

/// Die Umgebungsvariable, mit der sich die Wurzel uebergehen laesst.
pub const UMGEBUNG: &str = "MYELITH_WURZEL";

/// Traegt dieses Verzeichnis die Marke?
pub fn ist_wurzel(p: &Path) -> bool {
    p.join(MARKE).is_file()
}

/// Von hier aufwaerts suchen, bis die Marke auftaucht.
pub fn aufwaerts(anfang: PathBuf) -> Option<PathBuf> {
    let mut p = anfang;
    loop {
        if ist_wurzel(&p) {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

/// Das Verzeichnis, in dem das laufende Programm liegt.
///
/// ⚑ **Aufgeloest**, denn in einem `.app` fuehrt der Weg ueber
/// `Contents/MacOS`, und ein Verweis waere sonst nicht zu verfolgen.
pub fn eigener_ordner() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    p.parent().map(|q| q.to_path_buf())
}

/// **Die Wurzel, soweit sie sich ermitteln laesst**, oder `None`.
///
/// Drei Wege, in dieser Reihenfolge: die Umgebungsvariable, vom
/// Arbeitsverzeichnis aufwaerts, vom Programm aufwaerts.
///
/// ⚠️ **`None` ist ein gueltiger Zustand und kein Fehler.** Wer nur die
/// Freigabebuendel geholt hat, hat keinen Klon, und dann gibt es hier
/// kein `MODELS/`. Die Sinne fallen dafuer auf ihre Heimat zurueck.
///
/// ⚠️ **Eine falsch gesetzte Variable blockiert nicht.** Zeigt sie auf
/// ein Verzeichnis ohne Marke, wird weitergesucht, statt aufzugeben: Ein
/// Tippfehler soll nicht dazu fuehren, dass gar nichts mehr gefunden
/// wird.
pub fn wurzel() -> Option<PathBuf> {
    if let Some(w) = std::env::var_os(UMGEBUNG).map(PathBuf::from) {
        if ist_wurzel(&w) {
            return Some(w);
        }
    }
    for anfang in [std::env::current_dir().ok(), eigener_ordner()].into_iter().flatten() {
        if let Some(w) = aufwaerts(anfang) {
            return Some(w);
        }
    }
    None
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Legt einen Scheinklon an: ein Verzeichnis mit der Marke darin.
    fn scheinklon(name: &str) -> PathBuf {
        let w = std::env::temp_dir().join(format!("myelith-sinnesort-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join("INTEGER_LLM/scripts")).expect("anlegen");
        std::fs::write(w.join(MARKE), "#!/bin/sh\n").expect("Marke");
        std::fs::canonicalize(&w).unwrap_or(w)
    }

    #[test]
    fn ein_verzeichnis_mit_der_marke_ist_die_wurzel() {
        let w = scheinklon("marke");
        assert!(ist_wurzel(&w));
        // ⚑ Und die Gegenrichtung: ein Unterordner ist es nicht. Ohne
        // diese Zeile ginge die Probe auch durch, wenn `ist_wurzel`
        // schlicht `true` lieferte.
        assert!(!ist_wurzel(&w.join("INTEGER_LLM")));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚑ **Von tief drinnen fuehrt der Weg nach oben ans Ziel.**
    #[test]
    fn von_innen_wird_die_wurzel_gefunden() {
        let w = scheinklon("innen");
        let tief = w.join("CLIENT/myl-senses/src");
        std::fs::create_dir_all(&tief).expect("anlegen");
        assert_eq!(aufwaerts(tief), Some(w.clone()));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚠️ **Ein Verzeichnis ohne Marke liefert keine Wurzel**, auch
    /// nicht die naechste zufaellig passende weiter oben im Dateisystem.
    #[test]
    fn ohne_marke_gibt_es_keine_wurzel() {
        let d = std::env::temp_dir().join(format!("myelith-ohne-marke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("anlegen");
        assert_eq!(aufwaerts(d.clone()), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Diese Kiste liegt selbst in einem Klon, und der traegt die
    /// Marke.**
    ///
    /// ⚑ **Die Probe, die den Wert der Konstante wirklich prueft.** Die
    /// drei darueber laufen gegen einen Scheinklon und blieben gruen,
    /// wenn `MARKE` auf eine Datei zeigte, die es im echten Baum nicht
    /// gibt. 📌 **Ein Scheinklon prueft das Verfahren, nicht die
    /// Angabe.**
    #[test]
    fn die_marke_liegt_im_echten_baum() {
        let hier = Path::new(env!("CARGO_MANIFEST_DIR"));
        let w = aufwaerts(hier.to_path_buf()).expect("diese Kiste liegt in keinem Klon");
        assert!(w.join(MARKE).is_file());
        // Und der Ablageort der Gewichte liegt darunter, sonst zeigt die
        // Suche auf den falschen Baum.
        assert!(
            w.join(crate::laufwerk::MODELLE_ORDNER).is_dir(),
            "unter {} liegt kein Verzeichnis fuer fremde Gewichte",
            w.display()
        );
    }
}
