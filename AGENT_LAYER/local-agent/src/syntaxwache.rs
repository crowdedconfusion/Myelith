//! **Die Syntaxwache: Eine Aenderung, die eine gueltige Datei ungueltig
//! macht, wird nicht geschrieben.**
//!
//! # 📌 Der Anlass
//!
//! In den Loop-Laeufen vom 2026-09-26 und 2026-09-28 zerbrachen beide
//! grossen Modelle `daten/auswertung.py` mit einer einzigen Aenderung: das
//! 30B mit einer verrutschten Einrueckung (`'return' outside function`),
//! das 27B mit einer zerbrochenen Zeichenkette (`.replace("\", ",")`). Beide
//! fanden aus dem Zustand nicht mehr heraus; jede weitere Aenderung setzte
//! auf einer Datei auf, die schon nicht mehr lief.
//!
//! # ⚑ Die Idee und ihre Herkunft
//!
//! Die Idee stammt aus der Literatur zu Agenten, die Code bearbeiten:
//! Ein Bearbeitungswerkzeug, das eine Aenderung mit Syntaxfehler
//! zurueckweist und dem Modell den Fehler zeigt, hebt die Erfolgsquote
//! deutlich (SWE-agent, Yang et al. 2024, „Agent-Computer Interfaces").
//! Hier neu gebaut, fuer dieses Projekt: zwei Sprachen, eine Regel.
//!
//! # ⚑ Die Regel
//!
//! **Abgewiesen wird nur, was eine gueltige Datei ungueltig macht.** War
//! sie schon vorher ungueltig, wird geschrieben, denn sonst liesse sich
//! eine kaputte Datei nie schrittweise reparieren; die Antwort sagt dann,
//! dass der Fehler bleibt.
//!
//! ⛔️ **Nichts wird ausgefuehrt.** Python bekommt den Text nur zum
//! Uebersetzen (`compile`), nicht zum Laufen. Fehlt `python3`, ist das
//! Urteil offen, und nichts wird blockiert.

use std::path::Path;

/// Was die Wache ueber einen Text sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Urteil {
    /// Gueltig in seiner Sprache.
    Gut,
    /// Ungueltig, mit der Meldung samt Zeile.
    Fehler(String),
    /// Keine Sprache, die die Wache kennt, oder kein Pruefer auf der Maschine.
    Offen,
}

/// Prueft `inhalt` nach der Endung von `pfad`.
pub fn pruefen(pfad: &Path, inhalt: &str) -> Urteil {
    match pfad.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("py") => python(inhalt),
        Some("json") => match serde_json::from_str::<serde_json::Value>(inhalt) {
            Ok(_) => Urteil::Gut,
            Err(e) => Urteil::Fehler(format!("JSON: {e}")),
        },
        _ => Urteil::Offen,
    }
}

/// Uebersetzt Python, ohne es auszufuehren.
fn python(inhalt: &str) -> Urteil {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ZAEHLER: AtomicU64 = AtomicU64::new(0);
    let datei = std::env::temp_dir().join(format!(
        "myl-syntaxwache-{}-{}.py",
        std::process::id(),
        ZAEHLER.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&datei, inhalt).is_err() {
        return Urteil::Offen;
    }
    const PRUEFER: &str = "import sys\n\
        try:\n    compile(open(sys.argv[1], encoding='utf-8').read(), 'datei', 'exec')\n\
        except SyntaxError as e:\n    print(f'{e.msg} (Zeile {e.lineno})')\n    sys.exit(3)\n";
    let mut befehl = std::process::Command::new("python3");
    befehl.arg("-c").arg(PRUEFER).arg(&datei);
    let urteil = match myl_senses::prozess::laufen(&mut befehl, 10, 2000) {
        Ok(a) if a.gut() => Urteil::Gut,
        Ok(a) if a.kode == Some(3) => Urteil::Fehler(format!("Python: {}", a.aus.trim())),
        _ => Urteil::Offen,
    };
    let _ = std::fs::remove_file(&datei);
    urteil
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Python und JSON, gueltig und ungueltig**, mit Zeile. Ohne
    /// `python3` ist das Urteil offen, und die Probe sagt es statt still zu
    /// bestehen.
    #[test]
    fn die_wache_erkennt_python_und_json() {
        let gut = pruefen(Path::new("a.py"), "def f():\n    return 1\n");
        if gut == Urteil::Offen {
            eprintln!("python3 fehlt auf dieser Maschine; die Python-Faelle sind offen");
        } else {
            assert_eq!(gut, Urteil::Gut);
            let kaputt = pruefen(Path::new("a.py"), "def f():\nreturn 1\n");
            assert!(matches!(&kaputt, Urteil::Fehler(f) if f.contains("Zeile 2")), "{kaputt:?}");
            let zeichenkette = pruefen(Path::new("a.py"), "x = s.replace(\"\\\", \",\")\n");
            assert!(matches!(zeichenkette, Urteil::Fehler(_)), "die zerbrochene Zeichenkette des 27B");
        }
        assert_eq!(pruefen(Path::new("a.json"), "{\"a\": 1}"), Urteil::Gut);
        assert!(matches!(pruefen(Path::new("a.json"), "{\"a\": 1,}"), Urteil::Fehler(_)));
        assert_eq!(pruefen(Path::new("a.md"), "# was auch immer"), Urteil::Offen);
    }

    /// ⛔️ **Nichts wird ausgefuehrt.** Ein Text, der beim Laufen eine Datei
    /// anlegen wuerde, legt sie beim Pruefen nicht an.
    #[test]
    fn die_wache_fuehrt_nichts_aus() {
        let d = std::env::temp_dir().join(format!("myl-syntaxwache-probe-{}", std::process::id()));
        let code = format!("open({:?}, 'w').write('x')\n", d.display().to_string());
        let _ = pruefen(Path::new("a.py"), &code);
        assert!(!d.exists(), "die Wache hat den Text ausgefuehrt");
    }
}
