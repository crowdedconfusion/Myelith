//! Kennzeichnung geschriebener Texte durch die Dateiwerkzeuge des Agenten.
//!
//! ⚑ Eine eigene Datei mit einer einzigen Probe: Sie schaltet den Schalter
//! `ruestung::text_kennzeichnen` um, und der gilt im ganzen Prozess.

use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;
use myl_local_agent::werkzeug::Ansageform;
use myl_local_agent::werkzeuge::{Dateiaendern, Dateilesen, Dateischreiben, Einhaengung};
use myl_senses::kennzeichnung::{text_herkunft, Herkunft};
use serde_json::json;

/// ⚑ **Ein Dokument, das der Agent schreibt, ist gekennzeichnet, und er
/// selbst sieht das Kennzeichen nie**: Markdown ab rund 200 Token traegt ein
/// gueltiges Manifest am Ende, Lesen liefert genau den Text, eine Aenderung
/// hinterlaesst genau ein Kennzeichen, das zum neuen Text passt. Quelltext,
/// kurzer Text und ein abgeschalteter Schalter bleiben ohne.
#[test]
fn ein_geschriebenes_dokument_ist_gekennzeichnet_und_liest_sich_ohne() {
    let d = tempfile::tempdir().expect("Ordner");
    let e = Einhaengung::neu(d.path(), true).expect("Einhaengung");
    let form = Ansageform::Amtlich;
    let lang = "Ein Absatz über Bäume und ihre Wurzeln im alten Wald.\n".repeat(20);
    let schreiben = |pfad: &str, inhalt: &str| Dateischreiben(e.clone(), form).ausfuehren(&json!({"pfad": pfad, "inhalt": inhalt})).expect("geschrieben");
    let roh = |pfad: &str| std::fs::read_to_string(d.path().join(pfad)).expect("lesen");

    myl_local_agent::ruestung::text_kennzeichnen_setzen(true);
    let antwort = schreiben("bericht.md", &lang);
    assert!(antwort.contains("gekennzeichnet"), "{antwort}");
    let datei = roh("bericht.md");
    assert!(datei.starts_with(&lang) && datei.len() > lang.len());
    assert_eq!(text_herkunft(&datei), Herkunft::Gueltig);
    let gelesen = Dateilesen(e.clone(), form).ausfuehren(&json!({"pfad": "bericht.md"})).expect("gelesen");
    assert_eq!(gelesen, lang, "das Modell sieht das Kennzeichen nicht");

    let geaendert = Dateiaendern(e.clone(), form)
        .ausfuehren(&json!({"pfad": "bericht.md", "aenderungen": [{"alt": "im alten Wald.\nEin Absatz", "neu": "im jungen Wald.\nEin Absatz"}]}));
    assert!(geaendert.is_err(), "das Muster kommt mehrfach vor: {geaendert:?}");
    let mut neu = lang.clone();
    neu.push_str("Zum Schluss: Rinde.\n");
    schreiben("bericht.md", &neu);
    let antwort = Dateiaendern(e.clone(), form)
        .ausfuehren(&json!({"pfad": "bericht.md", "aenderungen": [{"alt": "Zum Schluss: Rinde.", "neu": "Zum Schluss: Borke."}]}))
        .expect("geaendert");
    assert!(antwort.contains("gekennzeichnet"), "{antwort}");
    let datei = roh("bericht.md");
    assert_eq!(text_herkunft(&datei), Herkunft::Gueltig, "das Kennzeichen passt zum neuen Text");
    assert_eq!(myl_senses::kennzeichnung::ohne_textkennzeichen(&datei), format!("{}Zum Schluss: Borke.\n", lang));
    assert_eq!(datei.matches('\u{FEFF}').count(), 1, "genau ein Kennzeichen");

    schreiben("werk.rs", &lang);
    assert_eq!(roh("werk.rs"), lang, "Quelltext bleibt ohne");
    schreiben("kurz.md", "Kurz.");
    assert_eq!(roh("kurz.md"), "Kurz.", "kurzer Text bleibt ohne");

    myl_local_agent::ruestung::text_kennzeichnen_setzen(false);
    schreiben("aus.md", &lang);
    assert_eq!(roh("aus.md"), lang, "abgeschaltet");
    myl_local_agent::ruestung::text_kennzeichnen_setzen(true);
}
