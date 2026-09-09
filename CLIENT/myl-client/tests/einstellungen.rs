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
    e.modell.artefakt = "INTEGER_LLM/artifacts/qwen3-4b".to_string();
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
    assert!(!Agenteneinstellung::default().auch_bezeugtes);
    assert!(!Kapazitaet::default().beschleuniger);
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
