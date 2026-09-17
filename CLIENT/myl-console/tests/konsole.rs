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
    // 📌 **Und er steht dauerhaft unter der Eingabe.** Bis zum
    // 2026-09-11 stand er im Kopfblock; der ist auf Wunsch des
    // Projektinhabers entfallen, und **verschwinden durfte er
    // nicht**: Ein Agent mit Dateiwerkzeugen arbeitet genau dort.
    assert!(
        s.contains("kurzer_ordner()"),
        "der Arbeitsordner steht nirgends, wo man ihn dauerhaft sieht"
    );
    assert!(
        s.contains("ordnerzeile(&stand.kurzer_ordner()"),
        "der Arbeitsordner steht nicht in der eigenen Zeile unter der Fusszeile"
    );
}

/// **Jeder Befehl steht genau einmal im Quelltext.**
///
/// 📌 **Dieselbe Klasse wie Fund 271, und die Antwort darauf.** Bis zum
/// 2026-09-11 stand jeder Befehl zweimal da: im Zweig, der ihn
/// ausfuehrt, und in der Hilfe, die ihn nennt. Diese Pruefung zaehlte
/// damals, ob er **mindestens zweimal** vorkommt, und hielt die
/// Wiederholung damit fest. Jetzt gibt es eine Liste, und diese
/// Pruefung wacht darueber, dass es dabei bleibt.
///
/// ⚑ Dass jeder Name auch behandelt wird und in der Hilfe steht, prueft
/// `jeder_befehl_wird_behandelt_und_steht_in_der_hilfe` an der Sache
/// selbst und nicht am Text.
#[test]
fn jeder_befehl_steht_genau_einmal_im_quelltext() {
    let s = quelle("sitzung.rs");
    for befehl in ["/model", "/settings", "/help", "/hilfe", "/exit", "/ende"] {
        let wie_oft = s.matches(&format!("\"{befehl}\"")).count();
        assert_eq!(
            wie_oft, 1,
            "`{befehl}` steht {wie_oft} mal im Quelltext, erwartet ist die eine Liste"
        );
    }
    // Und beide Wege gehen ueber sie.
    assert!(s.contains("BEFEHLE\n"), "die Hilfe kommt nicht aus der Liste");
    assert!(s.contains("BEFEHLE.iter().find"), "die Ausfuehrung kommt nicht aus der Liste");
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
        // Seit dem 2026-09-14 mit dem Gespraech davor; Zaehlen und
        // Verdichten kommen ebenfalls aus der Kiste.
        "myl_client::lauf::fahren_im_gespraech",
        "myl_client::gespraech::verdichten",
        "myl_client::gespraech::anzeige",
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

/// **Und wortgetreu heisst wortgetreu.**
///
/// 📌 Bis zum 2026-09-10 stand die Zusage nur im Kopf der vier Dateien.
/// Beim ersten Eingriff in `auswahl.rs` (Strg-J als Zeilenende, Fund
/// 307) fiel auf, dass nichts ausser der eigenen Aufmerksamkeit die
/// zweite Kopie nachzieht. **Eine Zusage, die niemand nachrechnet, ist
/// eine Behauptung.**
///
/// ⚠️ **Verschwindet der Testclient, verschwindet die Vergleichs-
/// grundlage.** Dann ist diese Kopie die einzige, und die Pruefung geht
/// stillschweigend durch: Sie bewacht ein Nebeneinander, das es dann
/// nicht mehr gibt.
#[test]
fn die_kopien_sind_wortgetreu() {
    let dort = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../TESTCLIENT/myl-testclient/src");
    if !dort.is_dir() {
        return;
    }
    for datei in ["animation.rs", "banner.rs", "farben.rs", "auswahl.rs"] {
        let a = geglaettet(&ohne_kopfvermerk(&quelle(datei)));
        let pfad = dort.join(datei);
        let b = geglaettet(&std::fs::read_to_string(&pfad).unwrap_or_default());
        assert_eq!(a, b, "{datei} weicht vom Testclient ab");
    }
}

/// Schneidet den Kopfvermerk ab, mit dem die Kopie sich als Kopie
/// ausweist: Er ist der einzige Zusatz, den sie tragen darf.
fn ohne_kopfvermerk(s: &str) -> String {
    if !s.starts_with("// ⚑ **Wortgetreue Kopie aus dem Testclient**") {
        return s.to_string();
    }
    let mut zeilen = s.lines();
    for z in zeilen.by_ref() {
        if z.trim() == "#![allow(dead_code)]" {
            break;
        }
    }
    let rest: Vec<&str> = zeilen.collect();
    let rest = if rest.first().map(|z| z.trim().is_empty()).unwrap_or(false) {
        &rest[1..]
    } else {
        &rest[..]
    };
    rest.join("\n")
}

/// Ersetzt den Untertitel samt seiner Erklaerung: Er ist die eine
/// erlaubte Abweichung, weil eine Zeile, die vom Testclient spricht,
/// hier schlicht falsch waere, und die Begruendung dafuer steht
/// natuerlich nur in der Kopie.
fn geglaettet(s: &str) -> String {
    let mut aus: Vec<&str> = Vec::new();
    for z in s.lines() {
        if z.starts_with("pub const SUBTITLE") {
            while aus.last().map(|v| v.trim_start().starts_with("///")).unwrap_or(false) {
                aus.pop();
            }
            aus.push("pub const SUBTITLE");
            continue;
        }
        aus.push(z);
    }
    aus.join("\n")
}

/// **Die Konsole setzt die Freigabe um, und zwar beim Start.**
///
/// # ⛔️ Fund 381, und es ist das haeufigste Fehlerbild dieses Projekts
///
/// `kap.kerne` liess sich auf der Einstellungsseite setzen, anzeigen
/// und abspeichern. `myl` wandte es an, das Fenster wandte es an,
/// **und diese Konsole las es nie**: Der Rechenpfad fragte
/// `available_parallelism` und nahm die ganze Maschine. Gruene Tests
/// sagten darueber nichts, denn keiner rief die Stelle.
///
/// ⚑ **Eine Einstellung, die an einem von drei Bedieninstrumenten
/// nichts bewirkt, ist schlimmer als eine, die nirgends wirkt.** Sie
/// wirkt ja anderswo, und deshalb sucht niemand den Unterschied im
/// Programm.
///
/// ⚠️ **Diese Pruefung liest den Quelltext**, denn die Stelle liegt in
/// einer Schleife, die ein Terminal braucht. Sie prueft damit die
/// **Aufrufstelle** und nicht die Wirkung; dass die Freigabe den
/// Rechenpfad wirklich erreicht, prueft `freigabe.rs` in `myl-client`.
#[test]
fn die_freigabe_wird_umgesetzt() {
    let sitzung = quelle("sitzung.rs");
    assert!(
        sitzung.contains("hardware::anwenden"),
        "die Konsole setzt die Kapazitaetsfreigabe nirgends um"
    );
    // ⚑ **Beim Start und nach der Einstellungsseite.** Nur eines von
    // beiden liesse entweder die gespeicherte Freigabe liegen oder eine
    // gerade geaenderte erst beim naechsten Start wirken.
    assert_eq!(
        sitzung.matches("hardware::anwenden").count(),
        2,
        "erwartet sind zwei Aufrufe: beim Start und nach der Einstellungsseite"
    );
}
