//! ⚑ Der Beleg, dass die Kette traegt: Modell, Vorlage, Antwort.
//!
//! Bis hierher war die Naht zwischen Harness und Modell nur eine Form.
//! Dieser Lauf fuellt sie mit dem echten 0,5B-Artefakt und prueft, dass
//! am anderen Ende Text herauskommt, den ein Mensch lesen kann.
//!
//! ⚑ **Gemessen wird an `myelith-4b` und nicht am 0,5B**, und der Grund
//! ist ein Befund vom 2026-09-08: Die Qwen2.5-Artefakte stammen aus den
//! **Basis**repositorien und kennen keine Rollenmarken. Bei Qwen3 ist
//! die Fassung ohne Zusatz die instruktionsgeschliffene. Wer einen
//! Agenten an einem Basismodell prueft, prueft die falsche Sache.
//!
//! 📌 **Er braucht Gewichte und ist deshalb nicht in der CI zu Hause.**
//! Ohne Artefakte bricht er ab und sagt, wie man den Sprung ausdruecklich
//! erlaubt, nach demselben Muster wie die Trainingslaeufe.

use myl_client::Oertlichesmodell;
use myl_local_agent::{Modellweg, Nachricht};

fn modell() -> Option<Oertlichesmodell> {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../INTEGER_LLM/artifacts/myelith-4b"
    );
    // 📌 **Fund 218: Diese Abfrage stand unter der Pfadpruefung**, und
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
    Some(Oertlichesmodell::laden(pfad, &Default::default()).expect("Modell laedt"))
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
            "myelith-4b",
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

/// ⚑ **Was live ankommt, ist genau die Antwort.**
///
/// # 📌 Die Zusage, an der die Live-Anzeige haengt
///
/// Ein Fenster, das mitschreibt, zeigt einen Text, der **waehrend** der
/// Rechnung entsteht. Weicht er am Ende von der Antwort ab, hat der
/// Nutzer etwas gelesen, das nirgends steht, und merkt es nicht: Der
/// Block springt zum Schluss um, und das sieht aus wie eine Anzeige,
/// die sich sortiert.
///
/// ⚑ **Geprueft wird deshalb die Gleichheit und nicht die Menge.** Ein
/// Beobachter, der die Haelfte meldet, faellt sonst nicht auf.
#[test]
fn der_laufende_text_ist_die_antwort() {
    let Some(mut m) = modell() else { return };
    m.grenze = 24;

    let gesammelt = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let mit = std::sync::Arc::clone(&gesammelt);
    m.beobachter = Some(Box::new(move |s: myl_client::strom::Stueck| {
        // ⚑ Beides zusammen ergibt den ganzen Strom; die Trennung
        // prueft `strom.rs` fuer sich.
        let t = match s {
            myl_client::strom::Stueck::Text(t) => t,
            myl_client::strom::Stueck::Denken(t) => t,
        };
        mit.lock().expect("Schloss").push_str(&t);
    }));

    let f = [Nachricht::nutzer("Die Hauptstadt von Frankreich ist")];
    let a = m.chat("m", &f, Some(24)).expect("Antwort");
    let live = gesammelt.lock().expect("Schloss").clone();

    assert!(!live.is_empty(), "es kam gar nichts live an");
    // ⚑ Die Antwort ist getrimmt und um die Endmarke gekuerzt; der
    // laufende Text ist es nicht. Verglichen wird deshalb, dass die
    // Antwort **darin steht**, und nicht auf Zeichengleichheit.
    assert!(
        live.contains(a.text.trim()),
        "der laufende Text enthaelt die Antwort nicht.\nlive: {live:?}\nAntwort: {:?}",
        a.text
    );
    // 📌 **Hier stand `== 24`, also die Grenze selbst**, und das war
    // eine Aussage ueber das alte Verhalten: Die Erzeugung lief immer
    // bis zur Grenze. Seit dem 2026-09-10 haelt sie an der Endmarke,
    // und dann sind es weniger. **Eine Pruefung, die die Grenze
    // verlangt, verlangt genau den Fehler**, der behoben wurde.
    assert!(a.antwort_token > 0, "es wurde nichts erzeugt");
    assert!(
        a.antwort_token as usize <= 24,
        "es wurden mehr Token erzeugt als erlaubt: {}",
        a.antwort_token
    );
}

/// ⚑ **Und der Beobachter aendert die Antwort nicht.**
///
/// Dieselbe Zusicherung wie in der Laufzeit, hier ueber die ganze Naht:
/// Vorlage, Erzeugung, Zuschnitt.
#[test]
fn mit_beobachter_kommt_dieselbe_antwort() {
    let Some(mut m) = modell() else { return };
    m.grenze = 16;
    let f = [Nachricht::nutzer("Nenne eine Primzahl.")];
    let ohne = m.chat("m", &f, Some(16)).expect("ohne Beobachter");

    m.beobachter = Some(Box::new(|_: myl_client::strom::Stueck| {}));
    let mit = m.chat("m", &f, Some(16)).expect("mit Beobachter");
    assert_eq!(ohne.text, mit.text, "der Beobachter hat die Antwort veraendert");
}

/// ⚑ **Das Modell hoert auf, wo seine Antwort aufhoert.**
///
/// # 📌 Der gemeldete Fehler, aus dem das entstanden ist (2026-09-10)
///
/// Mit 600 Token Grenze schrieb Qwen3-4B seine Antwort zu Ende, setzte
/// `<|im_end|>`, dann `<|endoftext|>` und **erfand danach ein ganzes
/// Gespraech weiter**, samt einem zweiten, ausgedachten Nutzer und
/// einem nacherzaehlten Werkzeugergebnis. Der Zuschnitt der fertigen
/// Antwort schnitt das ab; die **laufende Anzeige** nicht, und im
/// Agentenlauf ging der erfundene Text als Modellantwort in die
/// naechste Runde.
///
/// ⚑ **Geprueft wird an der Zahl der Token und nicht am Text.** Ein
/// Text ohne `<|im_end|>` beweist nichts: Der Zuschnitt entfernt die
/// Marke ohnehin. Beweisend ist, dass **weniger** gerechnet wurde als
/// erlaubt war.
#[test]
fn die_antwort_endet_an_der_endmarke() {
    let Some(mut m) = modell() else { return };
    assert!(!m.halt.is_empty(), "dieses Modell kennt keine Endmarke");
    m.grenze = 400;

    let f = [Nachricht::nutzer("Sag nur das Wort Ja.")];
    let a = m.chat("m", &f, Some(400)).expect("Antwort");

    assert!(
        (a.antwort_token as usize) < 400,
        "es wurden alle {} erlaubten Token gerechnet; die Endmarke hat nicht gehalten",
        a.antwort_token
    );
    // ⚑ Und die Marken stehen nicht im Text. Sie sind Rahmen und nicht
    // Inhalt, und die Erzeugung gibt sie gar nicht erst heraus.
    for marke in ["<|im_end|>", "<|endoftext|>", "<|im_start|>"] {
        assert!(!a.text.contains(marke), "`{marke}` steht in der Antwort: {:?}", a.text);
    }
}

/// ⚑ **Und der laufende Text traegt sie ebenso wenig.**
///
/// 📌 Das ist die Haelfte, die der gemeldete Fehler betraf: Der
/// Zuschnitt der fertigen Antwort greift erst am Ende, die Anzeige
/// laeuft waehrenddessen.
#[test]
fn auch_live_kommt_nichts_nach_der_endmarke() {
    let Some(mut m) = modell() else { return };
    m.grenze = 400;
    let gesammelt = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let mit = std::sync::Arc::clone(&gesammelt);
    m.beobachter = Some(Box::new(move |s: myl_client::strom::Stueck| {
        let t = match s {
            myl_client::strom::Stueck::Text(t) => t,
            myl_client::strom::Stueck::Denken(t) => t,
        };
        mit.lock().expect("Schloss").push_str(&t);
    }));

    let f = [Nachricht::nutzer("Sag nur das Wort Ja.")];
    let a = m.chat("m", &f, Some(400)).expect("Antwort");
    let live = gesammelt.lock().expect("Schloss").clone();

    for marke in ["<|im_end|>", "<|endoftext|>", "<|im_start|>", "Human:"] {
        assert!(!live.contains(marke), "`{marke}` kam live an:\n{live}");
    }
    assert!((a.antwort_token as usize) < 400, "es lief bis zur Grenze durch");
}

/// ⚑ **Das Denkbudget ueber den ganzen Weg**: Vorlage, Schleife,
/// Zerleger. Mit kleinem Budget steht die Schlussfolge in der
/// Ueberlegung, die Endmarke genau einmal im Text, und danach kommt eine
/// Antwort; mit null gibt es gar keine Ueberlegung.
///
/// 📌 **Warum der Zerleger mitgeprueft wird:** Die eingeschobene
/// Endmarke kommt nicht vom Modell, und eine Anzeige, die sie nicht
/// erkennt, hielte die ganze Antwort fuer weitere Ueberlegung. Dann
/// klaenge beim Vorlesen gar nichts.
#[test]
fn das_denkbudget_beendet_die_ueberlegung_und_es_kommt_eine_antwort() {
    let Some(mut m) = modell() else { return };
    let frage = [Nachricht::nutzer("Was ist 17 mal 23?")];
    m.denken = true;

    m.denkbudget = Some(16);
    let stuecke = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = std::sync::Arc::clone(&stuecke);
    m.beobachter = Some(Box::new(move |x| s.lock().unwrap().push(x)));
    let a = m.chat("myelith-4b", &frage, Some(200)).expect("Antwort");
    m.beobachter = None;
    assert_eq!(a.text.matches("</think>").count(), 1, "{:?}", a.text);
    let (denken, prosa) = myl_client::lauf::denken_und_prosa(&a.text);
    assert!(denken.contains("Time is short"), "die Schlussfolge fehlt: {denken:?}");
    assert!(prosa.contains("391"), "keine Antwort nach der Ueberlegung: {prosa:?}");
    let stuecke = stuecke.lock().unwrap();
    let text: String = stuecke
        .iter()
        .filter_map(|x| match x {
            myl_client::strom::Stueck::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("391"), "die Anzeige hielt die Antwort fuer Ueberlegung: {text:?}");
    assert!(!text.contains("Time is short"), "die Schlussfolge steht in der Antwort");

    m.denkbudget = Some(0);
    let a = m.chat("myelith-4b", &frage, Some(200)).expect("Antwort");
    let (denken, prosa) = myl_client::lauf::denken_und_prosa(&a.text);
    assert!(denken.is_empty(), "mit null wurde ueberlegt: {denken:?}");
    assert!(prosa.contains("391"), "{prosa:?}");
}
