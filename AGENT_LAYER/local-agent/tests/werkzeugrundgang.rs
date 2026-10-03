//! **Der Werkzeugrundgang: jedes Werkzeug des Loops einzeln, ueber die
//! echte Ruestung.**
//!
//! Auftrag des Projektinhabers vom 2026-09-28: vor dem naechsten langen
//! Agentenlauf alle Werkzeuge einzeln durchgehen und testen, damit ein
//! Lauf nicht an einem Werkzeug scheitert.
//!
//! # ⚑ Derselbe Weg wie die Schleife
//!
//! Geruestet wird wie `myl loop` im Loop-Szenario: Kiste `Advanced`,
//! Schreiben an, die Werkzeuge des Loops dazu, Arbeitsordner eine Kopie
//! von `BENCHMARKS/Agent/loop/vorlage`. Jeder Aufruf geht erst durch die
//! Formpruefung (`argumente_pruefen`) und dann in die Ausfuehrung, genau
//! in dieser Reihenfolge wie in `schleife.rs`. Was hier gruen ist, hat
//! also dieselbe Antwort, die ein Modell bekaeme.
//!
//! # ⚑ Die Antworten werden ausgegeben, nicht nur geprueft
//!
//! `cargo test --test werkzeugrundgang -- --nocapture` zeigt die ganze
//! Ansage und jede Antwort. **Eine Antwort kann richtig und trotzdem
//! unbrauchbar sein**, und das sieht nur, wer sie liest; die Zusicherungen
//! halten fest, was beim Lesen als richtig befunden wurde.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use myl_local_agent::agentenwahl::Agenteneinstellung;
use myl_local_agent::ruestung::Ruestung;
use myl_local_agent::werkzeug::{argumente_pruefen, Ansageform, Vorschlag};

fn wurzel_des_repositoriums() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn kopieren(von: &Path, nach: &Path) {
    std::fs::create_dir_all(nach).unwrap();
    for e in std::fs::read_dir(von).unwrap().flatten() {
        let ziel = nach.join(e.file_name());
        if e.path().is_dir() {
            kopieren(&e.path(), &ziel);
        } else {
            std::fs::copy(e.path(), ziel).unwrap();
        }
    }
}

struct Rundgang {
    _arbeit: tempfile::TempDir,
    _protokoll: tempfile::TempDir,
    arbeit: PathBuf,
    ruestung: Ruestung,
}

fn ruesten(web: bool, schreiben: bool) -> Rundgang {
    // ⛔️ Das Aktionsprotokoll in einen Wegwerfordner, nie in den des Nutzers.
    let protokoll = tempfile::tempdir().unwrap();
    myl_local_agent::protokoll::ordner_setzen(protokoll.path().to_path_buf());
    let arbeit_d = tempfile::tempdir().unwrap();
    let arbeit = arbeit_d.path().join("arbeit");
    kopieren(&wurzel_des_repositoriums().join("BENCHMARKS/Agent/loop/vorlage"), &arbeit);
    let agent = Agenteneinstellung {
        wurzel: Some(arbeit.display().to_string()),
        schreiben,
        kistenordner: Some(
            wurzel_des_repositoriums().join("AGENT_LAYER/local-toolkits/Advanced").display().to_string(),
        ),
        web_recherche: web,
        ..Agenteneinstellung::default()
    };
    let wahl = Arc::new(Mutex::new(myl_local_agent::vorhaben::Rundenwahl::default()));
    let ruestung = myl_local_agent::ruestung::ruesten(
        &agent,
        Ansageform::Amtlich,
        myl_local_agent::werkzeuge::Werkzeugkiste::Advanced,
        myl_local_agent::vorhaben::werkzeuge(&wahl),
    )
    .expect("Ruestung");
    Rundgang { _arbeit: arbeit_d, _protokoll: protokoll, arbeit, ruestung }
}

/// Ein Aufruf wie in der Schleife: Formpruefung, dann Ausfuehrung.
/// Zurueck kommt, was das Modell als Werkzeugergebnis saehe, und ob es
/// ein Fehler war.
fn rufen(r: &Rundgang, name: &str, argumente: serde_json::Value) -> (bool, String) {
    let v = Vorschlag { name: name.to_string(), arguments: argumente };
    let Some(angebot) = r.ruestung.kasten.angebot(name) else {
        return (false, format!("(nicht im Angebot: {name})"));
    };
    if let Err(f) = argumente_pruefen(angebot, &v) {
        return (false, f.to_string());
    }
    match r.ruestung.kasten.ausfuehren_ungeprueft(name, &v.arguments) {
        Some(Ok(t)) => (true, t),
        Some(Err(e)) => (false, e.grund),
        None => (false, "das Werkzeug hat keine Ausfuehrung".into()),
    }
}

/// Ruft, gibt aus und liefert die Antwort.
fn zeigen(r: &Rundgang, name: &str, argumente: serde_json::Value) -> (bool, String) {
    let (ok, text) = rufen(r, name, argumente.clone());
    let gekuerzt: String = text.chars().take(700).collect();
    println!("\n── {name} {argumente}\n   [{}] {}", if ok { "ok" } else { "FEHLER" }, gekuerzt.replace('\n', "\n   "));
    (ok, text)
}

#[test]
fn die_ansage_ist_vollstaendig_und_sauber() {
    let r = ruesten(false, true);
    let ansage = myl_local_agent::gespraech::ansage(&r.ruestung).content;
    println!("{ansage}");
    println!("\n[Ansage: {} Zeichen, {} Werkzeuge]", ansage.chars().count(), r.ruestung.kasten.angebote().len());
    let namen: Vec<&str> = r.ruestung.kasten.angebote().iter().map(|a| a.name.as_str()).collect();
    println!("[Werkzeuge: {}]", namen.join(", "));
    for a in r.ruestung.kasten.angebote() {
        // Keine Leerzeichenfolgen mitten im Satz (so stand es bis heute in
        // der Beschreibung von run_command).
        assert!(!a.beschreibung.contains("   "), "{}: Leerzeichenfolge in der Beschreibung", a.name);
        assert!(!a.beschreibung.trim().is_empty(), "{}: leere Beschreibung", a.name);
    }
    for muss in ["list_directory", "read_file", "search_files", "write_file", "edit_file", "run_command",
        "search_skill", "learn_skill", "note_set", "finish_goal", "web_search", "web_read"] {
        assert!(namen.contains(&muss), "{muss} fehlt im Angebot");
    }
}

#[test]
fn rundgang_lesen_und_suchen() {
    let r = ruesten(false, true);
    assert!(zeigen(&r, "list_directory", serde_json::json!({"tiefe": 1})).0);
    let (ok, t) = zeigen(&r, "list_directory", serde_json::json!({"tiefe": 3}));
    assert!(ok && t.contains("daten/auswertung.py"), "{t}");
    // ⚑ Ohne `tiefe` ein Formfehler, der das Feld nennt: Optionale
    //   Parameter gibt es mit Absicht nicht (`kein_werkzeug_hat_einen_optionalen_parameter`).
    let (ok, t) = zeigen(&r, "list_directory", serde_json::json!({}));
    assert!(!ok && t.contains("tiefe"), "{t}");
    let (ok, t) = zeigen(&r, "read_file", serde_json::json!({"pfad": "daten/auswertung.py"}));
    assert!(ok && t.contains("def lesen"), "{t}");
    let (ok, t) = zeigen(&r, "read_file", serde_json::json!({"pfad": "ergebnis/statistik.md"}));
    assert!(!ok && t.contains("list_directory") && t.contains("search_files"), "der Weg weiter fehlt: {t}");
    let (ok, _) = zeigen(&r, "read_file", serde_json::json!({"pfad": "../aussen.txt"}));
    assert!(!ok, "ein Pfad nach draussen ist ein Fehler");
    zeigen(&r, "read_file", serde_json::json!({"pfad": "daten"}));
    zeigen(&r, "read_file", serde_json::json!({"path": "daten/auswertung.py"}));
    let (ok, t) = zeigen(&r, "search_files", serde_json::json!({"muster": "kühlraum"}));
    assert!(ok && t.to_lowercase().contains("kühlraum"), "{t}");
    zeigen(&r, "search_files", serde_json::json!({"muster": "gibt_es_bestimmt_nicht_xyz"}));
}

#[test]
fn rundgang_schreiben_aendern_ausfuehren() {
    let r = ruesten(false, true);
    let (ok, _) = zeigen(&r, "write_file", serde_json::json!({"pfad": "bericht/neu.md", "inhalt": "# Test\n"}));
    assert!(ok && r.arbeit.join("bericht/neu.md").is_file(), "ein fehlender Ordner wird angelegt");
    assert!(zeigen(&r, "write_file", serde_json::json!({"pfad": "bericht/neu.md", "inhalt": "# Zwei\n"})).0);
    let (ok, t) = zeigen(&r, "run_command", serde_json::json!({"befehl": "python3 daten/auswertung.py"}));
    assert!(ok, "ein scheiternder Befehl ist eine Ausgabe, kein Werkzeugfehler: {t}");
    let (ok, t) = zeigen(&r, "edit_file", serde_json::json!({"pfad": "daten/auswertung.py",
        "aenderungen": [{"alt": "def lesen():", "neu": "def lesen():  # geprueft"}]}));
    assert!(ok && t.contains("So steht es jetzt"), "{t}");
    let (ok, t) = zeigen(&r, "edit_file", serde_json::json!({"pfad": "daten/auswertung.py",
        "aenderungen": [{"alt": "  werte = {}", "neu": "x"}]}));
    assert!(!ok && t.contains("Einrueckung"), "mitten in der Einrueckung: {t}");
    zeigen(&r, "run_command", serde_json::json!({"befehl": "echo hallo; exit 3"}));
    zeigen(&r, "run_command", serde_json::json!({"command": "ls"}));
}

#[test]
fn rundgang_skills_verlauf_loop_und_kisten() {
    let r = ruesten(false, true);
    let (ok, t) = zeigen(&r, "search_skill", serde_json::json!({"anfrage": "Quellenangaben im Bericht"}));
    assert!(ok, "{t}");
    zeigen(&r, "search_skill", serde_json::json!({"anfrage": ""}));
    let (ok, t) = zeigen(&r, "learn_skill", serde_json::json!({"name": "hausregeln-quellen"}));
    assert!(ok && t.len() > 100, "{t}");
    zeigen(&r, "learn_skill", serde_json::json!({"name": "gibt-es-nicht"}));
    zeigen(&r, "list_history", serde_json::json!({}));
    zeigen(&r, "search_history", serde_json::json!({"muster": "Kühlraum"}));
    zeigen(&r, "read_history", serde_json::json!({"von": 1, "bis": 5}));
    assert!(zeigen(&r, "note_set", serde_json::json!({"key": "stand", "value": "Trennzeichen ist ;"})).0);
    zeigen(&r, "wake_in", serde_json::json!({"minutes": 5}));
    zeigen(&r, "chain_goal", serde_json::json!({"goal": "Bericht schreiben"}));
    zeigen(&r, "finish_goal", serde_json::json!({"result": "fertig"}));
    for a in r.ruestung.kasten.angebote() {
        // Die Manifest- und verankerten Werkzeuge mit leerem Aufruf, damit
        // ihre Fehlermeldung zu lesen ist.
        if ["fill_template", "join_sections", "dateibaum", "git_stand", "suche_text", "zaehle_zeilen",
            "git_verlauf", "platzbedarf"].contains(&a.name.as_str())
        {
            println!("\n   Schema {}: {}", a.name, a.parameter);
            zeigen(&r, &a.name, serde_json::json!({}));
        }
    }
    zeigen(&r, "zaehle_zeilen", serde_json::json!({"datei": "daten/messwerte.csv"}));
    zeigen(&r, "platzbedarf", serde_json::json!({"pfad": "daten"}));
    zeigen(&r, "git_stand", serde_json::json!({}));
}

#[test]
fn rundgang_abgeschaltet() {
    let r = ruesten(false, false);
    let (ok, t) = zeigen(&r, "web_search", serde_json::json!({"frage": "Avocados anbauen"}));
    assert!(!ok && t.contains("agent.web_recherche"), "{t}");
    let (ok, t) = zeigen(&r, "write_file", serde_json::json!({"pfad": "x.md", "inhalt": "x"}));
    assert!(!ok && t.contains("agent.schreiben"), "{t}");
    assert!(!r.arbeit.join("x.md").exists(), "ein Platzhalter hat geschrieben");
    let (ok, t) = zeigen(&r, "web_read", serde_json::json!({"adresse": "https://example.org/avocado"}));
    assert!(!ok && t.contains("switched off"), "{t}");
}

/// ⚑ **Der Rundenauftrag des Loops nennt die passenden Skills.** Gebaut
/// wird er mit denselben Funktionen wie in `myl loop`, auf einer Kopie der
/// Szenario-Vorlage, und die Skill-Suche bekommt genau diesen Text.
#[test]
fn der_loopauftrag_nennt_den_hausregel_skill() {
    let r = ruesten(false, true);
    let ziel = "Schreibe einen Bericht für die Betriebsleitung in die Datei bericht/sensorbericht.md: \
        Welche Räume lagen am 20. September außerhalb ihres zulässigen Temperaturbereichs, und was ist \
        laut den Unterlagen im Ordner quellen/ jeweils zu tun? Nutze die Zahlen aus ergebnis/statistik.md \
        und recherchiere Zuordnung, Grenzwerte und Maßnahmen in quellen/. Achte darauf, welche \
        Unterlagen noch gelten. Halte dich an unsere Hausregeln für Quellenangaben.";
    let v = myl_local_agent::vorhaben::Vorhaben::neu("v1".into(), ziel, 0);
    let grenzen = myl_local_agent::agentenwahl::Loopeinstellung::default();
    for sprache in [myl_local_agent::agentenwahl::Sprache::De, myl_local_agent::agentenwahl::Sprache::En] {
        let text = myl_local_agent::vorhaben::auftrag(&v, &[], None, &grenzen, sprache);
        let treffer = myl_local_agent::skills::suchen(Some(&r.arbeit), &text, 6);
        println!("\n── Sprache {sprache:?}: {} Zeichen Auftrag", text.chars().count());
        for t in &treffer {
            println!("   {:>3} Punkte  {}", t.punkte, t.skill.name);
        }
        let h = myl_local_agent::skills::hinweis_fuer_auftrag(Some(&r.arbeit), &text, "learn_skill", false);
        println!("   Hinweis: {h:?}");
        assert!(h.as_deref().is_some_and(|h| h.contains("hausregeln-quellen")), "der Hausregel-Skill fehlt im Hinweis");
    }
}

/// 📌 **Im Loop sucht der Skill-Hinweis nach dem Ziel, nicht nach dem
/// Rundentext.** Nach dem ganzen Rundentext stand fuer „Skript zum Laufen
/// bringen" `bericht-schreiben` vorn; nach dem Ziel steht `fehlersuche` unter
/// den ersten zwei und `bericht-schreiben` nicht vorn.
#[test]
fn im_loop_empfiehlt_der_hinweis_nach_dem_ziel() {
    let r = ruesten(false, true);
    let roh = std::fs::read_to_string(wurzel_des_repositoriums().join("BENCHMARKS/Agent/loop/aufgaben.json")).unwrap();
    let d: serde_json::Value = serde_json::from_str(&roh).unwrap();
    let ziel = d["aufgaben"][0]["ziel"].as_str().unwrap().to_string();
    let mut v = myl_local_agent::vorhaben::Vorhaben::neu("v1".into(), &ziel, 0);
    v.abnahme = d["aufgaben"][0]["abnahme"].as_str().map(str::to_string);
    let text = myl_local_agent::vorhaben::auftrag(
        &v, &[], None, &myl_local_agent::agentenwahl::Loopeinstellung::default(), myl_local_agent::agentenwahl::Sprache::De,
    );
    let nach_text: Vec<String> = myl_local_agent::skills::suchen(Some(&r.arbeit), &text, 3).into_iter().map(|t| t.skill.name).collect();
    let nach_ziel: Vec<String> = myl_local_agent::skills::suchen(Some(&r.arbeit), &ziel, 3).into_iter().map(|t| t.skill.name).collect();
    println!("nach Rundentext: {nach_text:?}\nnach Ziel: {nach_ziel:?}");
    assert_eq!(nach_text[0], "bericht-schreiben", "der Befund, der die Aenderung begruendet, hat sich veraendert");
    assert!(nach_ziel[..2].contains(&"fehlersuche".to_string()), "{nach_ziel:?}");
    assert_ne!(nach_ziel[0], "bericht-schreiben", "{nach_ziel:?}");
}
