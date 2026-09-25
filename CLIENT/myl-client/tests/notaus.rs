//! ⛔️ **Der Notaus haelt an, und nichts geht dabei verloren.**
//!
//! ⚑ **Eine eigene Pruefsammlung**, weil der Schalter je Prozess gilt:
//! In einer Sammlung mit anderen Laeufen hielte er deren Modelle mit an.

use myl_local_agent::{Modellweg, Nachricht, Tuerfehler};

fn modell() -> Option<myl_client::Oertlichesmodell> {
    let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../INTEGER_LLM/artifacts/myelith-0.6b");
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {pfad}");
        return None;
    }
    if !std::path::Path::new(pfad).is_dir() {
        panic!(
            "Artefakte fehlen: {pfad}\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    Some(myl_client::Oertlichesmodell::laden(pfad, &Default::default()).expect("Modell laedt"))
}

/// ⛔️ Mitten im Schreiben ausgeloest: Die Erzeugung endet, `chat` meldet
/// `Abgebrochen` und bringt den Text bis dahin mit; danach verweigert jedes
/// Werkzeug, und zurueckgesetzt laeuft wieder alles.
#[test]
fn der_notaus_haelt_an_und_behaelt_den_text() {
    let ablage = tempfile::tempdir().expect("Protokoll");
    myl_client::protokoll::ordner_setzen(ablage.path().to_path_buf());

    // Werkzeuge verweigern nach dem Notaus, ohne auszufuehren.
    let ordner = tempfile::tempdir().expect("Ordner");
    std::fs::write(ordner.path().join("a.md"), "alt\n").expect("Datei");
    let r = myl_client::ruestung::ruesten_fuer_anhaenge(
        ordner.path(),
        myl_client::Ansageform::Amtlich,
        None,
        Vec::new(),
        None,
    )
    .expect("Ruestung");
    myl_client::notaus::ausloesen("probe");
    let aus = r
        .kasten
        .ausfuehren_ungeprueft("write_file", &serde_json::json!({ "pfad": "a.md", "inhalt": "neu" }))
        .expect("write_file");
    assert!(aus.is_err(), "nach dem Notaus lief ein Werkzeug");
    assert_eq!(std::fs::read_to_string(ordner.path().join("a.md")).unwrap(), "alt\n");
    let e = myl_client::protokoll::lesen(10);
    assert!(e.iter().any(|x| x.art == "notaus"), "der Notaus steht nicht im Protokoll");
    assert!(e.iter().any(|x| x.werkzeug == "write_file" && x.entscheidung == "abgebrochen"));
    myl_client::notaus::zuruecksetzen();

    // Mitten im Schreiben.
    let Some(mut m) = modell() else { return };
    let zaehler = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let z = std::sync::Arc::clone(&zaehler);
    m.beobachter = Some(Box::new(move |_| {
        if z.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 5 {
            myl_client::notaus::ausloesen_still();
        }
    }));
    let frage = [Nachricht::nutzer("Erzaehle mir ausfuehrlich von Paris.")];
    match m.chat("myelith-0.6b", &frage, Some(200)) {
        Err(Tuerfehler::Abgebrochen { bisher }) => {
            assert!(!bisher.is_empty(), "der Text bis zum Notaus ging verloren");
        }
        anderes => panic!("erwartet war Abgebrochen, bekommen: {anderes:?}"),
    }
    let bis_dahin = zaehler.load(std::sync::atomic::Ordering::SeqCst);
    assert!(bis_dahin < 20, "nach dem Notaus wurde weitergeschrieben: {bis_dahin} Stuecke");

    // Zurueckgesetzt laeuft es wieder.
    myl_client::notaus::zuruecksetzen();
    m.beobachter = None;
    assert!(m.chat("myelith-0.6b", &frage, Some(8)).is_ok());
}
