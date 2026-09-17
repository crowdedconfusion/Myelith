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

/// ⛔️ **Die Schreibseite und die Leseseite muessen denselben Ordner
/// meinen.**
///
/// Die Konsole schreibt ihren Mitschnitt in **ihr
/// Arbeitsverzeichnis** („Wer `myelith` hier tippt, hat die Frage
/// beantwortet"), das Fenster in den **eingestellten** Ordner. Faende
/// `myl verlauf` nur den eingestellten, bekaeme man im selben
/// Verzeichnis „Kein Mitschnitt" zu sehen, waehrend die Datei
/// danebenliegt. **Das ist die Fehlerklasse, die dieses Projekt am
/// haeufigsten trifft: dieselbe Angabe an zwei Orten, und der zweite
/// meldet sich nicht.**
///
/// ⚑ Die Umgebung zeigt hier ausdruecklich **woandershin**, damit die
/// Pruefung die Rangfolge belegt und nicht einen Zufall.
#[test]
fn verlauf_findet_den_mitschnitt_im_arbeitsverzeichnis() {
    let hier = tempfile::tempdir().expect("Ordner");
    let anderswo = tempfile::tempdir().expect("Ordner");
    myl_client::verlauf::schreiben(
        hier.path(),
        "probe-hier",
        "myelith-0.6b",
        &[
            ("user".to_string(), "Eine Frage.".to_string()),
            ("assistant".to_string(), "Eine Antwort.".to_string()),
        ],
    )
    .expect("der Mitschnitt laesst sich schreiben");

    let a = Command::new(env!("CARGO_BIN_EXE_myl"))
        .arg("verlauf")
        .current_dir(hier.path())
        .env("MYL_ARBEITSORDNER", anderswo.path())
        .env("XDG_CONFIG_HOME", anderswo.path())
        .output()
        .expect("das gebaute Binaerprogramm laesst sich starten");

    let text = String::from_utf8_lossy(&a.stdout);
    assert_eq!(a.status.code(), Some(0), "ein vorhandener Mitschnitt ist kein Fehler");
    assert!(
        text.contains("probe-hier"),
        "die Sitzung aus dem Arbeitsverzeichnis steht in der Uebersicht: {text}"
    );
    assert!(
        !text.contains("Kein Mitschnitt"),
        "der Mitschnitt liegt im Arbeitsverzeichnis und wird gefunden: {text}"
    );
}

/// ⚑ **Und die Umkehrung: ohne Mitschnitt im Arbeitsverzeichnis wird
/// das Arbeitsverzeichnis nicht genommen.**
///
/// Ohne diese zweite Haelfte uebernaehme es **jeden** Aufruf, und der
/// eingestellte Ordner waere nur noch aus Zufall erreichbar. **Die
/// Bedingung ist der vorhandene `.AGENT`-Ordner, und genau das wird
/// hier belegt.**
///
/// ⚠️ Geprueft wird, dass der **leere** Ordner nicht vorkommt, und
/// nicht, welcher stattdessen gewinnt: Welcher es ist, haengt an der
/// Einstellungsdatei dieser Maschine, und eine Pruefung, die daran
/// haengt, prueft die Maschine und nicht das Programm.
#[test]
fn ohne_mitschnitt_im_arbeitsverzeichnis_zaehlt_das_arbeitsverzeichnis_nicht() {
    let hier = tempfile::tempdir().expect("Ordner");
    let name = hier
        .path()
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .expect("der Ordner hat einen Namen");

    let a = Command::new(env!("CARGO_BIN_EXE_myl"))
        .arg("verlauf")
        .current_dir(hier.path())
        .output()
        .expect("das gebaute Binaerprogramm laesst sich starten");

    let text = String::from_utf8_lossy(&a.stdout);
    assert!(
        !text.contains(&name),
        "ein Arbeitsverzeichnis ohne Mitschnitt wird nicht genommen: {text}"
    );
}
