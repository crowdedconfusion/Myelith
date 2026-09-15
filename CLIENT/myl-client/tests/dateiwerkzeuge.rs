//! Der Weg vom Werkzeugvorschlag bis zur geschriebenen Datei, **ohne
//! geladenes Modell**.
//!
//! # ⚑ Was hier geprueft wird, und was die Einzelpruefungen nicht
//! erreichen
//!
//! `werkzeuge.rs` prueft die Einhaengegrenze fuer sich: Ein `..` kommt
//! nicht hinaus, ein Verweis auch nicht. Was dort **nicht** geprueft
//! werden kann, ist die Kette davor:
//!
//! | Glied | Was schiefgehen kann |
//! |---|---|
//! | Werkzeugkasten | ein Werkzeug haengt unter anderem Namen als angeboten |
//! | Manifest | ein Werkzeug hat keine Adresse und ist damit `Unbekannt` |
//! | Betriebsart | ein lokales Werkzeug laeuft, obwohl es bezeugt ist |
//! | Erlaubnis | ein nicht angebotenes Werkzeug wird ausgefuehrt |
//!
//! ⚑ **Genau dafuer gibt es `Modellweg`.** Der Pruefstand steckt die
//! Antwort hinein, die ein Modell gegeben haette, und sieht zu, was der
//! Harness daraus macht. Das kostet keine Sekunde Rechenzeit.

use myl_client::einstellungen::Agenteneinstellung;
use myl_client::werkzeuge::{Dateiwerkzeug, Werkzeugkiste};
use myl_local_agent::werkzeug::Ansageform;
use myl_local_agent::tuerklient::{Antwort, Modellweg, Nachricht, Tuerfehler};

/// Ein Modell, das eine feste Folge von Antworten gibt.
///
/// ⚑ **Zustandslos ueber die Nachrichtenzahl**, nicht ueber einen
/// Zaehler: `chat` bekommt `&self`, und ein Zaehler dahinter waere
/// genau der fadenlokale Zustand, den `Werkzeugausfuehrung` im
/// Modulkopf ausschliesst.
struct Drehbuch(Vec<String>);

impl Modellweg for Drehbuch {
    fn chat(
        &self,
        _modell: &str,
        nachrichten: &[Nachricht],
        _max_tokens: Option<u32>,
    ) -> Result<Antwort, Tuerfehler> {
        // Die erste Runde sieht Systemtext und Auftrag, jede weitere
        // zusaetzlich die Werkzeugantworten.
        let runde = nachrichten.iter().filter(|n| n.role == "assistant").count();
        let text = self.0.get(runde).cloned().unwrap_or_else(|| "Fertig.".to_string());
        Ok(Antwort {
            text,
            abschlussgrund: Some("stop".into()),
            kennung: "pruefstand".into(),
            segment: None,
            prompt_token: 0,
            antwort_token: 0,
        })
    }
}

fn fahren(
    wurzel: &std::path::Path,
    schreiben: bool,
    bezeugtes: bool,
    drehbuch: Vec<String>,
) -> myl_local_agent::schleife::Ergebnis {
    let agent = Agenteneinstellung {
        schritte: 4,
        wurzel: Some(wurzel.display().to_string()),
        schreiben,
        werkzeuge: Default::default(),
    modus: Default::default(),
    };
    let ruestung = myl_client::ruestung::ruesten(&agent, FORM, SATZ, Vec::new()).expect("Ruestung");
    let kontrakt = myl_types::sitzung::Sitzungskontrakt::neu(
        myl_types::ids::Address::new([1u8; 32]),
        myl_types::ids::Address::new([2u8; 32]),
        myl_types::sitzung::Grenzen {
            budget: 1000,
            einzellimit: 100,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        },
        myl_types::sitzung::Grenzen {
            budget: 1000,
            einzellimit: 100,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        },
        vec![myl_types::ids::Address::new([9u8; 32])],
        myl_types::ids::EpochId(0),
        myl_types::ids::EpochId(u64::MAX),
        4,
    )
    .expect("Kontrakt");
    let grenzen = myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen::neu(
        kontrakt,
        ruestung.kasten.angebote(),
    );
    let klient = Drehbuch(drehbuch);
    let zuordnung = ruestung.zuordnung();
    myl_local_agent::schleife::Lauf {
        // ⚑ Die Einhaengung gehoert ins Protokoll.
        einhaengung: ruestung.einhaengung.as_ref().map(|e| e.marke()),
        klient: &klient,
        modell: "pruefstand",
        grenzen: &grenzen,
        betriebsart: if bezeugtes {
            myl_local_agent::betrieb::Betriebsart::Alles
        } else {
            myl_local_agent::betrieb::Betriebsart::NurVerankert
        },
        kasten: &ruestung.kasten,
        registratur: &ruestung.registratur,
        adressen: &zuordnung,
        anker: myl_types::hash::Hash::from_bytes([0u8; 32]),
        max_tokens: Some(64),
        ansageform: Default::default(),
        melder: None,
    }
    .fahren("Schreibe eine Datei.")
}

/// Die Werkzeugnamen dieses Laufs, aus derselben Tabelle wie das
/// Angebot.
///
/// 📌 **Hier standen sie als Zeichenketten**, und als die Namen am
/// 2026-09-09 an die Ansageform gebunden wurden, spielten vier
/// Pruefungen dem Modell Aufrufe vor, die es gar nicht mehr gibt. Sie
/// fielen mit „die Auflistung fehlt", was nach einem Fehler in der
/// Verdrahtung aussieht und keiner war. Aus der Tabelle geholt, gehen
/// sie in beiden Formen mit.
const FORM: Ansageform = Ansageform::Amtlich;
// ⚑ Der volle Satz: Diese Sammlung prueft die Verdrahtung, und dafuer
// muessen alle Werkzeuge dabei sein. Was der Nutzer per Vorgabe
// bekommt, ist eine andere Frage und steht in `Werkzeugkiste`.
const SATZ: Werkzeugkiste = Werkzeugkiste::Advanced;
const VERZEICHNIS: &str = Dateiwerkzeug::Verzeichnis.name(FORM);
const LESEN: &str = Dateiwerkzeug::Lesen.name(FORM);
const SCHREIBEN: &str = Dateiwerkzeug::Schreiben.name(FORM);

fn ruf(name: &str, argumente: serde_json::Value) -> String {
    format!(
        "<tool_call>{}</tool_call>",
        serde_json::json!({"name": name, "arguments": argumente})
    )
}

/// ⚑ **Der Nachweis zum Anfassen:** Ein Vorschlag geht hinein, eine
/// Datei liegt danach auf der Platte.
#[test]
fn ein_vorschlag_wird_zu_einer_datei() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(
        d.path(),
        true,
        true,
        vec![ruf(SCHREIBEN, serde_json::json!({"pfad": "bericht.txt", "inhalt": "Es steht."}))],
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("bericht.txt")).expect("die Datei fehlt"),
        "Es steht."
    );
    assert!(matches!(erg.ende, myl_local_agent::schleife::Ende::Fertig), "{:?}", erg.ende);
}

/// ⚑ **Die Betriebsart greift VOR der Ausfuehrung.** Ohne
/// die Betriebsart darf ein lokales Werkzeug nicht laufen, und der
/// Nachweis dafuer ist die **nicht** vorhandene Datei.
#[test]
fn ohne_bezeugtes_entsteht_keine_datei() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(
        d.path(),
        true,
        false,
        vec![ruf(SCHREIBEN, serde_json::json!({"pfad": "verboten.txt", "inhalt": "x"}))],
    );
    assert!(!d.path().join("verboten.txt").exists(), "die Betriebsart hat nicht gesperrt");
    let gesagt = erg.nachrichten.iter().any(|n| n.content.contains("bezeugt"));
    assert!(gesagt, "der Grund steht nicht im Gespraech: {:?}", erg.nachrichten);
}

/// Die Grenze haelt auch auf diesem Weg, nicht nur im Werkzeug selbst.
#[test]
fn der_ausbruch_scheitert_auch_ueber_die_schleife() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(
        d.path(),
        true,
        true,
        vec![ruf(SCHREIBEN, serde_json::json!({"pfad": "../ausbruch.txt", "inhalt": "x"}))],
    );
    let daneben = d.path().parent().expect("Elternverzeichnis").join("ausbruch.txt");
    assert!(!daneben.exists(), "der Ausbruch gelang ueber die Schleife");
    assert!(erg.nachrichten.iter().any(|n| n.content.contains("ausserhalb")));
}

/// ⚑ **Ohne Schreiberlaubnis wird das Werkzeug nicht angeboten**, und
/// die Erlaubnis lehnt es deshalb schon vor der Betriebsart ab.
#[test]
fn ohne_schreiberlaubnis_gibt_es_das_werkzeug_nicht() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(
        d.path(),
        false,
        true,
        vec![ruf(SCHREIBEN, serde_json::json!({"pfad": "n.txt", "inhalt": "x"}))],
    );
    assert!(!d.path().join("n.txt").exists());
    assert!(!erg.nachrichten.is_empty());
}

/// Lesen und Auflisten gehen denselben Weg und muessen ankommen.
#[test]
fn lesen_und_auflisten_kommen_durch() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::fs::write(d.path().join("da.txt"), "der Inhalt").expect("Datei");
    let erg = fahren(
        d.path(),
        false,
        true,
        vec![
            // ⚑ **Mit `tiefe`, denn es ist verlangt.** Seit dem
            // 2026-09-10 hat kein Werkzeug mehr einen optionalen
            // Parameter: Ein Aufruf ohne Argumente wird abgewiesen,
            // und das ist der Sinn der Aenderung.
            ruf(VERZEICHNIS, serde_json::json!({"tiefe": 1})),
            ruf(LESEN, serde_json::json!({"pfad": "da.txt"})),
        ],
    );
    let alles: String = erg.nachrichten.iter().map(|n| n.content.clone()).collect();
    assert!(alles.contains("da.txt"), "die Auflistung fehlt: {alles}");
    assert!(alles.contains("der Inhalt"), "der Dateiinhalt fehlt: {alles}");
}

/// ⚑ **Das Protokoll haelt fest, worauf zugegriffen werden durfte.**
///
/// Bis zum 2026-09-08 stand im Strom, **welches** Werkzeug lief und ob
/// es erlaubt war. Nicht, **worauf** es zugreifen konnte. Wer das
/// Protokoll spaeter liest, sah „`datei_schreiben` wurde gerufen und
/// war erlaubt" und wusste nicht, ob das Arbeitsverzeichnis ein
/// Unterordner oder die Wurzel des Dateisystems war.
#[test]
fn das_protokoll_nennt_die_einhaengung() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(d.path(), true, true, vec![]);
    let marke = erg.strom.einhaengung().expect("keine Einhaengung im Protokoll");
    assert!(marke.schreiben, "das Schreibrecht steht nicht im Protokoll");

    // ⚑ **Der Abdruck und nicht der Pfad.** Der Strom ist das Stueck,
    // das die Maschine verlaesst; ein Pfad darin verriete das
    // Wirtsverzeichnis. Wer den Pfad kennt, rechnet nach.
    let erwartet = myl_types::hash::Hash::sha256(
        d.path().canonicalize().expect("aufloesen").as_os_str().as_encoded_bytes(),
    );
    assert_eq!(marke.wurzel, erwartet, "der Abdruck passt nicht zur Wurzel");
    let hex = marke.wurzel.to_hex();
    assert!(!hex.contains("tmp"), "der Abdruck traegt Teile des Pfades");
}

/// 📌 **Nur lesen steht auch als nur lesen darin.** Ein Lauf, der nichts
/// veraendern konnte, soll das zeigen, ohne dass jemand rechnet.
#[test]
fn nur_lesen_steht_im_protokoll() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(d.path(), false, true, vec![]);
    assert!(!erg.strom.einhaengung().expect("Einhaengung").schreiben);
}

/// ⚑ **Ohne gesetzte Wurzel greift die CTF-Vorgabe** (Auftrag des
/// Projektinhabers, 2026-09-14): Zeigt `MYL_CTF` auf ein Verzeichnis,
/// haengt der Agent es ein, statt ganz ohne Dateiwerkzeuge dazustehen.
/// So hat er von Haus aus einen Spielplatz.
///
/// 📌 **Die genuine „kein Dateizugriff"-Zusicherung** (Einhaengung
/// `None`, wenn `standard_wurzel` nichts findet) steht als Einzeltest
/// bei `standard_wurzel`; hier laesst sie sich nicht pruefen, weil die
/// Suche aufwaerts aus dem Baum den echten CTF-Ordner faende.
#[test]
fn ohne_wurzel_greift_die_ctf_vorgabe() {
    let alt = std::env::var_os("MYL_CTF");
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::env::set_var("MYL_CTF", d.path());

    let agent = Agenteneinstellung {
        schritte: 2,
        wurzel: None,
        schreiben: false,
        werkzeuge: Default::default(),
        modus: Default::default(),
    };
    let ruestung = myl_client::ruestung::ruesten(&agent, FORM, SATZ, Vec::new()).expect("Ruestung");
    assert!(
        ruestung.einhaengung.is_some(),
        "ohne gesetzte Wurzel greift die CTF-Vorgabe"
    );
    // Und damit gibt es wieder Lesewerkzeuge, auch ohne Schreiberlaubnis.
    assert!(!ruestung.kasten.angebote().is_empty());

    match alt {
        Some(v) => std::env::set_var("MYL_CTF", v),
        None => std::env::remove_var("MYL_CTF"),
    }
}
