//! Was das Bedieninstrument beim blossen Aufruf tut.
//!
//! ⚑ **Warum das eine eigene Pruefsammlung ist.** Die uebrigen
//! Sammlungen pruefen die Kiste; hier laeuft das **Binaerprogramm**,
//! ueber `CARGO_BIN_EXE_myl`. Geprueft wird nur, was ohne Modell und
//! ohne Netz feststeht: welcher Kanal die Ausgabe traegt und was
//! zurueckkommt. Beides ist eine Zusage an Skripte, und beides war bis
//! zu diesem Tag von keiner Pruefung gedeckt.

use std::process::{Command, Output};

fn ruf(argumente: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_myl"))
        .args(argumente)
        .output()
        .expect("das gebaute Binaerprogramm laesst sich starten")
}

#[test]
fn ohne_argumente_kommt_die_hilfe_nach_stdout_und_null_zurueck() {
    let a = ruf(&[]);
    assert_eq!(a.status.code(), Some(0));
    let text = String::from_utf8_lossy(&a.stdout);
    assert!(text.contains("myl frage"), "die Hilfe nennt die Befehle: {text}");
    assert!(a.stderr.is_empty(), "die Hilfe ist kein Fehler");
}

#[test]
fn hilfe_ausdruecklich_verlangt_ist_dasselbe() {
    for schalter in ["--hilfe", "-h", "--help", "hilfe"] {
        let a = ruf(&[schalter]);
        assert_eq!(a.status.code(), Some(0), "`{schalter}` ist kein Fehler");
        assert!(
            String::from_utf8_lossy(&a.stdout).contains("myl frage"),
            "`{schalter}` zeigt die Hilfe"
        );
    }
}

/// 📌 Diese Pruefung ist der Grund fuer die ganze Datei. Vor ihr fiel
/// jeder unbekannte Befehl in denselben Zweig wie die Hilfe: Text nach
/// stdout, Rueckgabe null. Wer `myl sitzng` schrieb, bekam die Hilfe in
/// seine Pipe und las daraus, es sei gutgegangen.
#[test]
fn ein_tippfehler_ist_ein_fehler() {
    let a = ruf(&["sitzng"]);
    assert_eq!(a.status.code(), Some(2), "ein Aufruffehler gibt 2 zurueck");
    assert!(
        a.stdout.is_empty(),
        "kein Wort nach stdout, sonst liest ein Skript den Fehler als Ergebnis"
    );
    let fehler = String::from_utf8_lossy(&a.stderr);
    assert!(fehler.contains("sitzng"), "der falsche Befehl wird benannt: {fehler}");
    assert!(fehler.contains("myl frage"), "und die Hilfe steht dabei");
}

/// ⚑ Die Startprobe des Freigabe-Jobs ruft genau das auf einem frisch
/// gebauten Binaerprogramm auf. Steht hier, damit die eine Zeile in
/// `release.yml` nicht die einzige Stelle ist, die davon ausgeht.
#[test]
fn die_startprobe_des_freigabe_jobs_geht_auf() {
    let a = ruf(&["--hilfe"]);
    assert!(a.status.success());
}
