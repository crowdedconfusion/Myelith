//! ⚑ Einstellungen sind Daten, also werden sie wie Daten geprüft.

use myl_client::einstellungen::{Agenteneinstellung, Einstellungen, Kapazitaet};

/// ⚑ **Eine fehlende Datei ist kein Fehler.**
///
/// Beim ersten Start gibt es keine, und ein Werkzeug, das dann abbricht,
/// ist unbrauchbar.
#[test]
fn ohne_datei_gelten_die_vorgaben() {
    let p = std::path::Path::new("/tmp/gibt-es-nicht-myl-xyz/client.json");
    let e = Einstellungen::lesen(p).expect("fehlende Datei ist kein Fehler");
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
    let p = std::path::Path::new("/tmp/myl-kaputt.json");
    std::fs::write(p, "{ das ist kein json").unwrap();
    assert!(Einstellungen::lesen(p).is_err(), "kaputte Datei still ersetzt");
    let _ = std::fs::remove_file(p);
}

#[test]
fn schreiben_und_lesen_ergibt_dasselbe() {
    let p = std::path::Path::new("/tmp/myl-test/client.json");
    let mut e = Einstellungen::default();
    e.modell.artefakt = "INTEGER_LLM/artifacts/qwen3-4b".to_string();
    e.modell.token = 128;
    e.kapazitaet.kerne = Some(4);
    e.agent.schritte = 12;
    e.schreiben(p).expect("schreiben");
    assert_eq!(Einstellungen::lesen(p).expect("lesen"), e);
    let _ = std::fs::remove_file(p);
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
