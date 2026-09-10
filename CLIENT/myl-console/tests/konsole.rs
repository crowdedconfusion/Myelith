//! Pruefungen des Konsolenclients, ohne ein Modell zu laden.
//!
//! # ⚑ Warum sie den Quelltext lesen
//!
//! Was dieses Programm tut, laesst sich zum grossen Teil nur mit einem
//! geladenen Modell und einem Terminal messen: beides steht in einem
//! Testlauf nicht zur Verfuegung. **Was sich ohne beides pruefen
//! laesst, sind die Zusagen**, und die stehen im Quelltext: dass der
//! Arbeitsordner aus der Shell kommt, dass hier nichts gebaut wird,
//! dass die Befehle, die in der Hilfe stehen, auch behandelt werden.

fn quelle(name: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// **Der Arbeitsordner ist das Arbeitsverzeichnis und kein Feld.**
///
/// ⚑ Wer `myelith` in einem Verzeichnis tippt, hat die Frage
/// beantwortet. Im Fenster ist der Arbeitsordner eine Einstellung, weil
/// ein Fenster nirgends steht; ein Programm in der Konsole steht immer
/// irgendwo.
///
/// ⚠️ **Und es muss die Einstellung ueberschreiben, nicht ergaenzen.**
/// Bliebe `agent.wurzel` stehen, arbeitete der Agent in einem Ordner,
/// den jemand vor Wochen im Fenster gesetzt hat, waehrend der Nutzer
/// woanders steht. **Das ist genau der Fall, gegen den die
/// Einhaengegrenze gebaut ist.**
#[test]
fn der_arbeitsordner_kommt_aus_der_shell() {
    let s = quelle("sitzung.rs");
    assert!(
        s.contains("agent.wurzel = Some(stand.ordner.display().to_string());"),
        "der Arbeitsordner dieser Sitzung ueberschreibt die Einstellung nicht"
    );
    assert!(
        s.contains("std::env::current_dir()"),
        "das Arbeitsverzeichnis wird nicht gelesen"
    );
    // Und er steht im Kopf, bevor die erste Eingabe moeglich ist.
    assert!(
        s.contains("Arbeitsordner:"),
        "der Kopf sagt nicht, worauf zugegriffen wird"
    );
}

/// **Was in der Hilfe steht, wird auch behandelt, und umgekehrt.**
///
/// ⛑ Dieselbe Klasse wie Fund 271: Eine Hilfe, die von Hand gepflegt
/// wird, nennt irgendwann einen Befehl, den es nicht gibt, oder
/// verschweigt einen, den es gibt. **Beides sieht erst der, der es
/// ausprobiert.**
#[test]
fn jeder_befehl_steht_in_der_hilfe() {
    let s = quelle("sitzung.rs");
    for befehl in ["/model", "/settings", "/hilfe", "/ende"] {
        assert!(
            s.contains(&format!("\"{befehl}\"")),
            "`{befehl}` wird nicht behandelt"
        );
        assert!(
            s.matches(befehl).count() >= 2,
            "`{befehl}` steht nicht zugleich in der Hilfe und im Zweig"
        );
    }
}

/// **Hier wird nichts gebaut.**
///
/// ⚠️ Ein Artefakt zu holen und zu kalibrieren dauert Minuten bis
/// Stunden und braucht Gigabyte. Das gehoert nicht hinter eine Zeile,
/// die jemand tippt, weil er eine Frage stellen wollte: **Das Fenster
/// hat dafuer einen Balken und einen Abbruch, eine Konsolenzeile hat
/// beides nicht.**
#[test]
fn hier_wird_nichts_gebaut() {
    let s = quelle("sitzung.rs");
    for verboten in ["artefakt_bauen", "hf download", "Command::new"] {
        assert!(!s.contains(verboten), "der Konsolenclient ruft `{verboten}`");
    }
    assert!(
        s.contains("myl-oberflaeche"),
        "er sagt nicht, wo ein Artefakt herkommt"
    );
}

/// **Die Kopie der Marke ist als Kopie gekennzeichnet.**
///
/// ⚑ Vier Dateien stehen wortgetreu so da wie im Testclient, damit sich
/// beide gegeneinander halten lassen. **Die einzige Aenderung ist der
/// Untertitel**, denn eine Zeile, die vom Testclient spricht, waere in
/// diesem Programm schlicht falsch.
#[test]
fn die_marke_ist_eine_gekennzeichnete_kopie() {
    for datei in ["animation.rs", "banner.rs", "farben.rs", "auswahl.rs"] {
        let s = quelle(datei);
        assert!(
            s.contains("Wortgetreue Kopie aus dem Testclient"),
            "{datei} sagt nicht, dass sie eine Kopie ist"
        );
    }
    let b = quelle("banner.rs");
    assert!(
        !b.contains("Testclient · Hardware"),
        "der Untertitel spricht noch vom Testclient"
    );
}

/// **Dieses Programm hat keine eigene Logik.**
///
/// ⚑ Dieselbe Zusage wie beim Fenster: Agentenlauf, Werkzeuge,
/// Einstellungen und Modell kommen aus `myl-client`. **Was hier
/// stuende, muesste dort noch einmal stehen.**
#[test]
fn die_logik_kommt_aus_der_kiste() {
    let s = quelle("sitzung.rs");
    for naht in [
        "myl_client::lauf::fahren_beobachtet",
        "myl_client::ruestung::ruesten",
        "myl_client::Oertlichesmodell::laden",
        "myl_client::ort::absolut",
    ] {
        assert!(s.contains(naht), "die Naht `{naht}` fehlt");
    }
    // Und die Einstellungen werden gezeigt, nicht gesetzt: Ein zweiter
    // Setzer waere die dritte Stelle, die dieselben Feinheiten kennt.
    assert!(
        !s.contains(".setzen("),
        "der Konsolenclient setzt Einstellungen selbst"
    );
}
