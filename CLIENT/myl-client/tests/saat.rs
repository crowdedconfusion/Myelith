//! **Immer mit Saat** (Regel des Projektinhabers, 2026-09-29): Gleiche Saat,
//! bitgleiche Antwort; andere Saat, anderer Weg; ohne Angabe eine neue Saat
//! je Aktion, und jede steht im Aktionsprotokoll.

use myl_client::Oertlichesmodell;
use myl_local_agent::{Modellweg, Nachricht};

fn modell() -> Option<Oertlichesmodell> {
    let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../INTEGER_LLM/artifacts/myelith-0.6b");
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {pfad}");
        return None;
    }
    if !std::path::Path::new(pfad).is_dir() {
        panic!("Artefakte fehlen: {pfad}\nMYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich.");
    }
    let mut m = Oertlichesmodell::laden(pfad, &Default::default()).expect("Modell laedt");
    m.grenze = 24;
    Some(m)
}

fn frage(m: &Oertlichesmodell) -> String {
    m.chat("lokal", &[Nachricht::nutzer("Erzähl mir in einem Satz etwas über den Mond.")], Some(24))
        .expect("Antwort")
        .text
}

/// ⚑ **Die Saat gehoert zur Aktion.** Eine feste Saat wiederholt die Antwort
/// bitgleich; die Saat „nur fuer die naechste“ gilt genau einmal; ohne
/// Angabe zieht jede Aktion eine neue, und `letzte_saat` nennt sie.
#[test]
fn gleiche_saat_gleiche_antwort_und_je_aktion_eine_neue() {
    let Some(mut m) = modell() else { return };
    assert!(!m.gierig, "gezogen ist die Vorgabe");

    m.saat_fest = Some(7);
    let a = frage(&m);
    assert_eq!(m.letzte_saat(), Some(7));
    assert_eq!(a, frage(&m), "feste Saat, bitgleiche Antwort");

    // Nur fuer die naechste Aktion, danach wieder die feste.
    m.naechste_saat(1234);
    let einmal = frage(&m);
    assert_eq!(m.letzte_saat(), Some(1234));
    assert_eq!(frage(&m), a, "danach wieder die feste Saat");
    m.naechste_saat(1234);
    assert_eq!(frage(&m), einmal, "dieselbe einmalige Saat, dieselbe Antwort");

    // Zufall: jede Aktion eine neue Saat, und verschiedene Wege.
    m.saat_fest = None;
    let mut saaten = std::collections::BTreeSet::new();
    let mut antworten = std::collections::BTreeSet::new();
    for _ in 0..4 {
        antworten.insert(frage(&m));
        saaten.insert(m.letzte_saat().expect("eine Saat"));
    }
    assert_eq!(saaten.len(), 4, "je Aktion eine neue Saat");
    assert!(antworten.len() >= 2, "vier Saaten, eine einzige Antwort: {antworten:?}");
}

/// ⚑ **Eine Aktion, eine Saat, auch ueber mehrere Aufrufe**: Innerhalb einer
/// offenen Aktion zieht jeder Aufruf aus derselben Saat, der Reihe nach.
/// Dieselbe Aktion noch einmal ergibt dieselbe Folge.
#[test]
fn eine_aktion_ueber_mehrere_aufrufe_ist_wiederholbar() {
    let Some(mut m) = modell() else { return };
    m.saat_fest = Some(99);
    let lauf = |m: &Oertlichesmodell| {
        let saat = m.aktion_beginnen();
        let folge = (frage(m), frage(m));
        m.aktion_beenden();
        (saat, folge)
    };
    let (s1, f1) = lauf(&m);
    let (s2, f2) = lauf(&m);
    assert_eq!(s1, Some(99));
    assert_eq!(s1, s2);
    assert_eq!(f1, f2, "dieselbe Aktion, dieselbe Folge");
    // Gierig gibt es keine Saat.
    m.gierig = true;
    assert_eq!(m.aktion_beginnen(), None);
    m.aktion_beenden();
}
