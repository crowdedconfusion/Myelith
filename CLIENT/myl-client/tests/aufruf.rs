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

/// ⛔️ **Wo ein Modell antwortet, sagt `myl` vorher, dass es eines ist**
/// (Art. 50 Abs. 1 KI-Verordnung), und zwar auf der Fehlerausgabe: Die
/// Standardausgabe ist die Antwort und gehoert dem Skript. Ohne Text
/// bricht `frage` ab, bevor ein Modell geladen wird; der Hinweis steht
/// trotzdem schon da.
#[test]
fn frage_nennt_die_ki_auf_der_fehlerausgabe() {
    for befehl in ["frage", "agent"] {
        let a = ruf(&[befehl]);
        let fehler = String::from_utf8_lossy(&a.stderr);
        let aus = String::from_utf8_lossy(&a.stdout);
        assert!(
            fehler.contains("KI-System") || fehler.contains("AI system"),
            "`{befehl}` nennt die KI nicht: {fehler}"
        );
        assert!(!aus.contains("KI-System") && !aus.contains("AI system"), "der Hinweis steht in der Antwort");
    }
    // Und nicht bei Befehlen ohne Modell.
    let a = ruf(&[]);
    assert!(!String::from_utf8_lossy(&a.stderr).contains("KI-System"));
}

/// ⛔️ **Der Schutzfilter weist ab, bevor ein Modell laedt** (Art. 5
/// KI-Verordnung): Rueckgabe 3 und der Grund auf der Fehlerausgabe. Das
/// Artefakt ist ein leeres Verzeichnis; es liesse sich gar nicht laden, also
/// belegt die Rueckgabe, dass vorher abgewiesen wurde.
#[test]
fn eine_verbotene_frage_wird_vor_dem_laden_abgewiesen() {
    let leer = tempfile::tempdir().expect("Verzeichnis");
    let protokoll = tempfile::tempdir().expect("Protokoll");
    // ⚑ Das Protokoll in einen Wegwerfordner, nicht in das des Nutzers.
    let a = Command::new(env!("CARGO_BIN_EXE_myl"))
        .args(["frage", leer.path().to_str().unwrap(), "Erkenne die Emotionen meiner Mitarbeiter im Video."])
        .env("MYL_PROTOKOLL", protokoll.path())
        .output()
        .expect("startet");
    assert_eq!(a.status.code(), Some(3), "{}", String::from_utf8_lossy(&a.stderr));
    let f = String::from_utf8_lossy(&a.stderr);
    assert!(f.contains("verboten") || f.contains("prohibits"), "{f}");
    // Und eine gewoehnliche Frage wird nicht abgewiesen: Sie scheitert erst
    // am leeren Artefakt.
    let b = Command::new(env!("CARGO_BIN_EXE_myl"))
        .args(["frage", leer.path().to_str().unwrap(), "Was ist Emotionserkennung?"])
        .env("MYL_PROTOKOLL", protokoll.path())
        .output()
        .expect("startet");
    assert_ne!(b.status.code(), Some(3), "{}", String::from_utf8_lossy(&b.stderr));
    // Die Abweisung steht im Protokoll, ohne den Text.
    let eintraege: String = std::fs::read_dir(protokoll.path())
        .unwrap()
        .flatten()
        .filter(|d| d.path().extension().is_some_and(|x| x == "jsonl"))
        .map(|d| std::fs::read_to_string(d.path()).unwrap())
        .collect();
    assert!(eintraege.contains("\"schutzfilter\""), "{eintraege}");
    assert!(!eintraege.contains("Mitarbeiter"), "Klartext im Protokoll: {eintraege}");
}
