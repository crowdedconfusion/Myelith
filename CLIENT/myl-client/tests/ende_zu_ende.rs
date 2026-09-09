//! ⚑ Der Beleg, dass die Kette traegt: Modell, Vorlage, Antwort.
//!
//! Bis hierher war die Naht zwischen Harness und Modell nur eine Form.
//! Dieser Lauf fuellt sie mit dem echten 0,5B-Artefakt und prueft, dass
//! am anderen Ende Text herauskommt, den ein Mensch lesen kann.
//!
//! ⚑ **Gemessen wird an `qwen3-4b` und nicht am 0,5B**, und der Grund
//! ist ein Befund vom 2026-09-08: Die Qwen2.5-Artefakte stammen aus den
//! **Basis**repositorien und kennen keine Rollenmarken. Bei Qwen3 ist
//! die Fassung ohne Zusatz die instruktionsgeschliffene. Wer einen
//! Agenten an einem Basismodell prueft, prueft die falsche Sache.
//!
//! ⛑ **Er braucht Gewichte und ist deshalb nicht in der CI zu Hause.**
//! Ohne Artefakte bricht er ab und sagt, wie man den Sprung ausdruecklich
//! erlaubt, nach demselben Muster wie die Trainingslaeufe.

use myl_client::Oertlichesmodell;
use myl_local_agent::{Modellweg, Nachricht};

fn modell() -> Option<Oertlichesmodell> {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../INTEGER_LLM/artifacts/qwen3-4b"
    );
    // ⛑ **Fund 218: Diese Abfrage stand unter der Pfadpruefung**, und
    // damit war der Schalter auf jeder Maschine wirkungslos, die die
    // Artefakte **hat**. Gemeint war er fuer zwei Leser: die CI, wo
    // nichts liegt, und den Entwickler, der waehrend einer Messung
    // keine Rechenzeit an eine Pruefsammlung abgeben will. Nur der
    // erste wurde bedient. Aufgefallen, als `MYL_OHNE_ARTEFAKTE=1
    // cargo test` neben einem laufenden Training doch das 4B-Modell
    // lud und 59 Sekunden rechnete. Der Schalter heisst „ohne
    // Artefakte" und bedeutet jetzt genau das, unabhaengig davon, ob
    // welche da sind.
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {pfad}");
        return None;
    }
    if !std::path::Path::new(pfad).is_dir() {
        panic!(
            "Artefakte fehlen: {pfad}\n\
             Dieser Lauf rechnet auf echten Gewichten und kann das ohne Modell nicht.\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    Some(Oertlichesmodell::laden(pfad).expect("Modell laedt"))
}

/// ⚑ **Eine Frage, deren Antwort feststeht.**
///
/// Der Pruefstand der Trainingsmessungen hat am 2026-09-07 gezeigt,
/// warum das noetig ist: Eine Befragung, die nur an Erfundenem geprueft
/// wird, kann kaputt sein, ohne dass es auffaellt. Hier steht deshalb
/// eine Tatsache, die jedes brauchbare Modell kennt.
#[test]
fn das_oertliche_modell_antwortet_auf_bekanntes() {
    let Some(mut m) = modell() else { return };
    m.grenze = 48;
    let antwort = m
        .chat(
            "qwen3-4b",
            &[Nachricht::nutzer("Wie heisst die Hauptstadt von Frankreich?")],
            Some(48),
        )
        .expect("das oertliche Modell antwortet");

    eprintln!("ANTWORT: {}", antwort.text);
    assert!(
        antwort.text.to_lowercase().contains("paris"),
        "erwartet wurde Paris, bekommen: {:?}",
        antwort.text
    );
    assert!(antwort.antwort_token > 0, "es wurde kein Token erzeugt");
    assert!(antwort.prompt_token > 0, "der Prompt war leer");
    // ⚑ Lokale Arbeit traegt keinen Kettenbeleg, und das muss so bleiben.
    assert!(antwort.kennung.is_empty(), "lokal darf keine Kennung entstehen");
    assert!(antwort.segment.is_none(), "lokal darf kein Segment entstehen");
}

/// ⚑ **Zweimal dieselbe Frage muss dasselbe ergeben.**
///
/// Gierig gezogen ist der Weg deterministisch. Waere er es nicht, waere
/// ein Fehlschlag im Agenten nicht auffindbar, weil derselbe Plan
/// zweimal anders liefe.
#[test]
fn derselbe_prompt_ergibt_dieselbe_antwort() {
    let Some(mut m) = modell() else { return };
    m.grenze = 16;
    let f = [Nachricht::nutzer("Nenne eine Primzahl.")];
    let a = m.chat("m", &f, Some(16)).expect("erste Antwort");
    let b = m.chat("m", &f, Some(16)).expect("zweite Antwort");
    assert_eq!(a.text, b.text, "gierig gezogen und trotzdem verschieden");
}
