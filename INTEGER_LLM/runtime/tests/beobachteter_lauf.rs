//! Der Beobachter verändert die erzeugte Folge nicht.
//!
//! # ⚑ Was hier auf dem Prüfstand steht
//!
//! `generate_beobachtet` meldet jedes Token, sobald es dasteht, damit
//! ein Fenster mitschreiben kann, statt am Ende einen Block hinzulegen.
//! **Der ganze Wert dieser Naht hängt an einer Zusicherung:** Ein Lauf
//! mit Beobachter erzeugt dieselbe Folge wie einer ohne.
//!
//! Wäre das nicht so, hätte das Projekt still zwei Rechenwege, und der
//! zweite trüge keine Konformitätsvektoren. Genau dieselbe Frage wie
//! beim Mitschnitt, eine Ebene höher.

use integer_llm_runtime::generate::{generate, generate_beobachtet, Erzeugung};
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::tokenizer::Tokenizer;

fn artefakte() -> std::path::PathBuf {
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "myelith-0.6b".to_string());
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(modell)
}

/// ⚑ **Fehlen die Artefakte, schlägt der Test fehl und sagt, was zu tun
/// ist.** Ein stiller Sprung sieht aus wie ein bestandener Test
/// (Fund 113).
fn modell() -> Option<(IntegerModel, Tokenizer)> {
    // ⚑ Die Abfrage steht als **erste** Zeile, siehe Fund 218: Der
    // Schalter heisst „ohne Artefakte" und nicht „es liegen keine da".
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt)");
        return None;
    }
    let dir = artefakte();
    if !dir.exists() {
        panic!(
            "Artefakte fehlen: {dir:?}\n\
             Dieser Test belegt, dass der Beobachter die Folge nicht veraendert,\n\
             und kann das ohne Modell nicht.\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    let w = Tokenizer::from_file(&dir.join("tokenizer.json").display().to_string())
        .expect("Wortschatz");
    Some((load_model(&dir).expect("Modell laedt"), w))
}

/// ⚑ **Dieselbe Folge mit und ohne Beobachter**, und der Beobachter
/// sieht genau das, was herauskommt.
#[test]
fn dieselbe_folge_mit_und_ohne_beobachter() {
    let Some((m, w)) = modell() else { return };
    let prompt = "Die Hauptstadt von Frankreich ist";

    let ohne = generate(&m, &w, prompt, 12, 0, true);

    let mut gesehen = Vec::new();
    let mit = generate_beobachtet(
        &m,
        &w,
        prompt,
        &Erzeugung { max_new_tokens: 12, seed: 0, greedy: true, halt: &[] },
        &mut |t| gesehen.push(t),
    );

    assert_eq!(
        ohne, mit,
        "der Beobachter hat die erzeugte Folge veraendert.\n\
         Damit gaebe es zwei Rechenwege, und der zweite traegt keine Vektoren."
    );

    // ⚑ **Und er sieht jedes Token, in der Reihenfolge.** Ein
    // Beobachter, der die Haelfte meldet, faellt sonst nicht auf: Die
    // Rueckgabe waere ja richtig, und die Anzeige nur unvollstaendig.
    assert_eq!(gesehen, mit, "der Beobachter sah nicht dieselben Token wie der Rueckgabewert");
    assert!(!gesehen.is_empty(), "es wurde gar nichts gemeldet");
}

/// ⚑ **Und der Beobachter kommt VOR dem naechsten Vorwaertspass.**
///
/// ⛑ Der Unterschied ist der ganze Zweck: Meldet er erst danach, hinkt
/// die Anzeige um einen vollen Vorwaertspass hinterher, und bei einem
/// 4B-Modell ist das der sichtbare Teil der Wartezeit. Gemessen wird
/// das an der Reihenfolge: Beim ersten Ruf steht genau ein Token in
/// der Folge.
#[test]
fn gemeldet_wird_sofort_und_nicht_am_ende() {
    let Some((m, w)) = modell() else { return };
    let mut zahl = 0usize;
    let mut erste_meldung_nach = None;
    generate_beobachtet(
        &m,
        &w,
        "Eins zwei drei",
        &Erzeugung { max_new_tokens: 6, seed: 0, greedy: true, halt: &[] },
        &mut |_| {
        zahl += 1;
        if erste_meldung_nach.is_none() {
            erste_meldung_nach = Some(zahl);
        }
    });
    assert_eq!(erste_meldung_nach, Some(1), "die erste Meldung kam nicht beim ersten Token");
    assert_eq!(zahl, 6, "es wurden nicht alle Token gemeldet");
}

/// ⚑ **Eine Haltemarke beendet die Erzeugung, und sie steht nicht drin.**
///
/// # ⛑ Der Fehler, aus dem diese Marken entstanden sind
///
/// Ohne sie rechnet die Schleife stur bis `max_new_tokens`. Gemessen am
/// 2026-09-10 mit Qwen3-4B und 600 Token Grenze: Das Modell beendete
/// seine Antwort, schrieb `<|im_end|>`, dann `<|endoftext|>` und
/// **erfand danach ein ganzes Gespraech weiter**, samt einem zweiten
/// Nutzer. Das ist kein Fehler des Modells, sondern seine Aufgabe: Es
/// setzt Text fort, und nach einer beendeten Antwort setzt es die
/// naechste Runde fort.
///
/// ⚑ **Geprueft wird beides**, denn eines allein genuegt nicht: dass
/// frueher Schluss ist, und dass die Marke selbst nicht in der Ausgabe
/// steht. Eine Marke am Ende der Folge zwaenge jeden Aufrufer, sie
/// wieder abzuschneiden.
#[test]
fn eine_haltemarke_beendet_und_steht_nicht_in_der_ausgabe() {
    let Some((m, w)) = modell() else { return };
    let prompt = "Die Hauptstadt von Frankreich ist";

    let frei = generate(&m, &w, prompt, 12, 0, true);
    assert_eq!(frei.len(), 12, "ohne Halt wird bis zur Grenze gerechnet");

    // ⚑ Als Marke das Token, das der freie Lauf an dritter Stelle
    // erzeugt hat: Dann ist der Abbruch gemessen und nicht geraten.
    let marke = frei[3];
    let mut gesehen = Vec::new();
    let kurz = generate_beobachtet(
        &m,
        &w,
        prompt,
        &Erzeugung { max_new_tokens: 12, seed: 0, greedy: true, halt: &[marke] },
        &mut |t| gesehen.push(t),
    );

    assert_eq!(kurz.len(), 3, "die Marke hat nicht beendet: {kurz:?}");
    assert_eq!(kurz, frei[..3], "vor der Marke kam etwas anderes heraus");
    assert!(!kurz.contains(&marke), "die Haltemarke steht in der Ausgabe");
    assert_eq!(gesehen, kurz, "der Beobachter sah die Marke oder etwas anderes");
}

/// ⚑ **Ohne Marken ist es Zeichen fuer Zeichen die alte Schleife.**
///
/// ⛑ Diese Haelfte ist die wichtigere: `generate` steht in Beispielen
/// und Messungen, und eine Folge, die frueher endet, waere dort ein
/// anderer Messwert.
#[test]
fn ohne_marken_aendert_sich_nichts() {
    let Some((m, w)) = modell() else { return };
    let prompt = "Eins zwei drei";
    let a = generate(&m, &w, prompt, 10, 0, true);
    let b = generate_beobachtet(
        &m,
        &w,
        prompt,
        &Erzeugung { max_new_tokens: 10, seed: 0, greedy: true, halt: &[] },
        &mut |_| {},
    );
    assert_eq!(a, b, "eine leere Markenliste hat die Folge veraendert");
    assert_eq!(a.len(), 10);
}
