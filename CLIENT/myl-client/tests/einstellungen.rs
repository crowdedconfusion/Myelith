//! ⚑ Einstellungen sind Daten, also werden sie wie Daten geprüft.

use myl_client::einstellungen::{Agenteneinstellung, Einstellungen, Kapazitaet};

// ⛑ **Hier standen fest verdrahtete `/tmp`-Pfade, und auf Windows gibt
// es kein `/tmp`.** Der Fehlschlag kam am 2026-09-09 aus der CI:
//
//     Os { code: 3, kind: NotFound, message: "The system cannot find
//     the path specified." }
//
// ⚑ **Und er war eine Zufallsfrage, keine feste.** `/tmp/x` ist auf
// Windows laufwerksrelativ, also `C:\tmp\x`. `schreiben` legt sein
// Elternverzeichnis an, `fs::write` nicht. Lief die Pruefung mit
// `schreiben` zuerst, existierte `C:\tmp` und die andere kam durch;
// lief sie danach, nicht. Die Reihenfolge entscheidet der
// Testlaeufer, also war es gruen, solange es Glueck hatte.
//
// `tempfile::tempdir` loest beides: Es liegt dort, wo das System seine
// Zwischendateien haelt, und es ist je Pruefung ein eigenes
// Verzeichnis, also stossen parallele Laeufe nicht zusammen.

/// ⚑ **Eine fehlende Datei ist kein Fehler.**
///
/// Beim ersten Start gibt es keine, und ein Werkzeug, das dann abbricht,
/// ist unbrauchbar.
#[test]
fn ohne_datei_gelten_die_vorgaben() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    // ⚑ Der Pfad muss fehlen, das Verzeichnis darum herum nicht.
    let p = d.path().join("gibt-es-nicht").join("client.json");
    let e = Einstellungen::lesen(&p).expect("fehlende Datei ist kein Fehler");
    assert_eq!(e, Einstellungen::default());
}

/// ⛑ **Eine kaputte Datei IST einer.**
///
/// Sie stillschweigend durch Vorgaben zu ersetzen hiesse, die
/// Einstellungen des Nutzers ohne ein Wort zu verwerfen. Genau die Sorte
/// stiller Ersetzung, die dieses Projekt an anderer Stelle teuer bezahlt
/// hat.
#[test]
fn eine_kaputte_datei_faellt_auf() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let p = d.path().join("kaputt.json");
    std::fs::write(&p, "{ das ist kein json").expect("schreiben");
    assert!(Einstellungen::lesen(&p).is_err(), "kaputte Datei still ersetzt");
}

#[test]
fn schreiben_und_lesen_ergibt_dasselbe() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let p = d.path().join("tief").join("client.json");
    let mut e = Einstellungen::default();
    e.modell.artefakt = "INTEGER_LLM/artifacts/myelith-4b".to_string();
    e.modell.token = 128;
    e.kapazitaet.kerne = Some(4);
    e.agent.schritte = 12;
    e.schreiben(&p).expect("schreiben");
    assert_eq!(Einstellungen::lesen(&p).expect("lesen"), e);
}

/// ⚑ **Die vorsichtige Vorgabe, und beide Stellen.**
///
/// Der Harness gibt „nur nachrechenbar" als Vorgabe, und die Kapazität
/// gibt nur die CPU her. Wer mehr will, sagt es. Eine Vorgabe, die
/// stillschweigend grosszuegig ist, ist eine Falle.
#[test]
fn die_vorgaben_sind_die_engen() {
    // ⛑ **Hier stand `!Agenteneinstellung::default().auch_bezeugtes`.**
    // Der Schalter ist am 2026-09-11 entfallen (Festlegung des
    // Projektinhabers): Was ein Lauf an Werkzeugen bekommt, sagt die
    // Werkzeugkiste. Die enge Vorgabe steht damit nicht mehr in einer
    // Einstellung, sondern im Aufruf, und ohne `--bezeugtes` bleibt es
    // bei „nur nachrechenbar".
    assert!(!Agenteneinstellung::default().schreiben);
    // ⛑ **Hier stand `!Kapazitaet::default().beschleuniger`**, ein
    // einzelner Schalter fuer alle Rechenwerke zugleich. Er ist am
    // 2026-09-10 entfallen: Eine Freigabe ueber null **ist** die
    // Erlaubnis, und ein Rechner mit zwei Karten konnte mit einem
    // Schalter nicht sagen, dass er die eine hergibt und die andere
    // behaelt. Die enge Vorgabe ist jetzt die **leere** Freigabe.
    assert!(
        Kapazitaet::default().rechenwerke.is_empty(),
        "die Vorgabe gibt ein Rechenwerk her, ohne dass jemand es gesagt hat"
    );
    assert_eq!(Kapazitaet::default().kerne, None, "die Vorgabe nimmt sich Kerne");
    assert_eq!(Kapazitaet::default().platte_gib, None);
    assert_eq!(Kapazitaet::default().speicher_gib, None);
}

/// ⚑ Eine Datei, die ein Mensch bearbeitet, endet mit einem Umbruch.
#[test]
fn die_datei_endet_mit_einem_umbruch() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let p = d.path().join("client.json");
    Einstellungen::default().schreiben(&p).expect("schreiben");
    let inhalt = std::fs::read_to_string(&p).expect("lesen");
    assert!(inhalt.ends_with('\n'), "kein Umbruch am Ende");
    assert!(!inhalt.ends_with("\n\n"), "zwei Umbrueche am Ende");
    // Und sie bleibt lesbar.
    assert_eq!(Einstellungen::lesen(&p).expect("lesen"), Einstellungen::default());
}

/// ⚑ **Eine Ablage aus der Zeit vor der Umbenennung wandert mit.**
///
/// # ⛑ Warum eine Umbenennung ohne das nicht fertig ist
///
/// Die Artefakte heissen seit dem 2026-09-10 nach dem Modell, das sie
/// sind. Wer nur die Verzeichnisse umbenennt, hat die Arbeit auf jeden
/// verschoben, der eine Einstellung gesetzt hat: Sein Pfad loest ins
/// Leere auf, der Klient meldet „Modell laedt nicht", und er sucht den
/// Fehler bei sich.
#[test]
fn eine_alte_ablage_findet_ihr_artefakt_wieder() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let p = d.path().join("client.json");
    for (alt, neu) in [
        ("INTEGER_LLM/artifacts/qwen3-4b", "INTEGER_LLM/artifacts/myelith-4b"),
        ("INTEGER_LLM/artifacts/qwen2.5-0.5b", "INTEGER_LLM/artifacts/myelith-0.5b"),
        ("/anderswo/qwen3-30b-a3b", "/anderswo/myelith-30b-a3b"),
    ] {
        // ⚑ Geschrieben mit dem echten Schreiber: Eine von Hand
        // getippte Ablage koennte Felder auslassen, die es gibt, und
        // dann prueffte dieser Test das Auslassen und nicht die
        // Wanderung.
        let mut vorher = Einstellungen::default();
        vorher.modell.artefakt = alt.to_string();
        vorher.schreiben(&p).expect("schreiben");
        let e = Einstellungen::lesen(&p).expect("lesen");
        assert_eq!(e.modell.artefakt, neu, "`{alt}` ist nicht mitgewandert");
    }

    // ⚑ **Und was schon neu heisst oder ganz anders, bleibt.** Eine
    // Wanderung, die auch Unbeteiligtes anfasst, ist schlimmer als
    // keine: Sie aendert einen Pfad, den jemand mit Bedacht gesetzt hat.
    for unberuehrt in [
        "INTEGER_LLM/artifacts/myelith-4b",
        "/eigenes/verzeichnis/mein-modell",
        "artifacts/qwen3-4b-eigenbau",
    ] {
        let mut vorher = Einstellungen::default();
        vorher.modell.artefakt = unberuehrt.to_string();
        vorher.schreiben(&p).expect("schreiben");
        let e = Einstellungen::lesen(&p).expect("lesen");
        assert_eq!(e.modell.artefakt, unberuehrt, "`{unberuehrt}` wurde angefasst");
    }
}
