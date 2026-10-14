//! **Der Schlaf mit dem echten Modell** (`myl_local_agent::schlaf`):
//! Unterbrechen, Vorwaermen und ein ganzer Durchgang mit dem 0,6B.
//!
//! ⚑ **Eine eigene Pruefsammlung**, weil der Notaus je Prozess gilt und die
//! Probe pruefen muss, dass der Wecker ihn gerade nicht anfasst.

use std::sync::atomic::Ordering;

use myl_local_agent::schlaf::{self, Ruheauftrag, Wecker};
use myl_local_agent::{Modellweg, Nachricht, Tuerfehler};

fn modell() -> Option<myl_client::Oertlichesmodell> {
    // ⚑ `MYL_RUHE_ARTEFAKT` nennt ein anderes Artefakt, fuer den Vergleich
    //   der Modellgroessen; ohne gilt das 0,6B, der Anker.
    let vorgabe = concat!(env!("CARGO_MANIFEST_DIR"), "/../../INTEGER_LLM/artifacts/myelith-0.6b").to_string();
    let pfad = std::env::var("MYL_RUHE_ARTEFAKT").unwrap_or(vorgabe);
    let pfad = pfad.as_str();
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
    Some(myl_client::Oertlichesmodell::laden(pfad, &Default::default()).expect("Modell laedt")).map(|mut m| {
        m.gierig = true;
        m
    })
}

fn gespraech() -> Vec<Nachricht> {
    vec![
        Nachricht::system("Du bist ein hilfreicher Assistent."),
        Nachricht::nutzer("Ich heisse Mira und schreibe Berichte immer auf Deutsch und als Markdown."),
        Nachricht { role: "assistant".into(), content: "Verstanden, Mira: Berichte auf Deutsch, als Markdown.".into() },
    ]
}

/// ⛔️ **Der Wecker haelt die Erzeugung an, und der Notaus bleibt, wo er
/// war.** Danach antwortet das Modell wieder ganz.
#[test]
fn der_wecker_haelt_an_ohne_den_notaus() {
    let Some(mut m) = modell() else { return };
    let w = Wecker::neu();
    let w2 = w.clone();
    let zaehler = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let z = std::sync::Arc::clone(&zaehler);
    m.beobachter = Some(Box::new(move |_| {
        if z.fetch_add(1, Ordering::SeqCst) == 5 {
            w2.wecken();
        }
    }));
    let frage = [Nachricht::nutzer("Erzaehle mir ausfuehrlich von Paris.")];
    match m.chat_unterbrechbar("myelith-0.6b", &frage, Some(200), w.schalter()) {
        Err(Tuerfehler::Abgebrochen { .. }) => {}
        anderes => panic!("erwartet war Abgebrochen, bekommen: {anderes:?}"),
    }
    assert!(zaehler.load(Ordering::SeqCst) < 20, "nach dem Wecken wurde weitergeschrieben");
    assert!(!myl_local_agent::notaus::ausgeloest(), "der Wecker hat den Notaus ausgeloest");
}

/// ⛔️ **Vorwaermen fuellt den KV-Speicher**: Die naechste Frage auf
/// denselben Anfang rechnet nur noch, was neu ist. Gegenprobe: nach
/// `vergessen` ohne Vorwaermen wird nichts wiederverwendet.
#[test]
fn vorwaermen_fuellt_den_speicher() {
    let Some(m) = modell() else { return };
    let mut weiter = gespraech();
    weiter.push(Nachricht::nutzer("Wie heisse ich?"));

    m.vergessen();
    m.zaehler.zuruecksetzen();
    m.chat("myelith-0.6b", &weiter, Some(8)).expect("Antwort");
    let kalt = m.zaehler.wiederverwendet.load(Ordering::SeqCst);

    m.vergessen();
    let neu = m.vorwaermen(&gespraech(), &std::sync::atomic::AtomicBool::new(false)).expect("vorgewaermt");
    assert!(neu > 20, "es wurde kaum etwas vorgelesen: {neu}");
    m.zaehler.zuruecksetzen();
    m.chat("myelith-0.6b", &weiter, Some(8)).expect("Antwort");
    let warm = m.zaehler.wiederverwendet.load(Ordering::SeqCst) as usize;
    eprintln!("kalt {kalt} wiederverwendet, warm {warm} von {neu} vorgewaermten Token");
    assert_eq!(kalt, 0, "ohne Vorwaermen kam etwas aus dem Speicher");
    // ⚑ **Bis auf den Kopf der Antwort**: Vorgewaermt ist das Gespraech samt
    //   `<|im_start|>assistant` (und dem leeren Denkblock); die naechste Frage
    //   setzt dort ihre eigene Nachricht hin. Gemessen: 57 von 63.
    assert!(neu - warm <= 8, "das Vorgewaermte wurde nicht gebraucht: {warm} von {neu}");
}

/// ⚑ **Ein ganzer Schlaf mit dem 0,6B**: Er laeuft durch, fragt das
/// Modell, und was er gewinnt, traegt eine Herkunft. Was genau das kleine
/// Modell verdichtet, ist hier nicht die Frage; die Zeile steht im
/// Protokoll der Probe.
#[test]
fn ein_ganzer_schlaf_mit_dem_kleinen_modell() {
    let Some(m) = modell() else { return };
    let d = tempfile::tempdir().expect("Ordner");
    let a = Ruheauftrag {
        wurzel: d.path(),
        gespraeche: vec![schlaf::episode_aus_gespraech("chat-probe", &gespraech())],
        vorhaben: None,
        vorwaermen: Some(gespraech()),
        deutsch: true,
        tag: myl_local_agent::gedaechtnis::heute(),
    };
    let anfang = std::time::Instant::now();
    let b = schlaf::schlafen(Some(&m), &a, &Wecker::neu());
    eprintln!("{} ({:?}, {} Aufrufe)", b.zeile(true), anfang.elapsed(), b.modellaufrufe);
    assert_eq!(b.fehler, None);
    assert!(!b.geweckt);
    assert!(b.modellaufrufe >= 2, "{b:?}");
    assert_eq!(b.abschnitte, 2, "{b:?}");
    let g = myl_local_agent::gedaechtnis::Gedaechtnis::laden(d.path()).expect("laden");
    for e in &g.eintraege {
        eprintln!("  {}", myl_local_agent::gedaechtnis::zeile(e, true));
        assert!(e.herkunft.iter().all(|h| h.sitzung == "chat-probe"), "ohne Herkunft: {e:?}");
    }
    assert!(b.vorgewaermt.is_some(), "nicht vorgewaermt");
}
