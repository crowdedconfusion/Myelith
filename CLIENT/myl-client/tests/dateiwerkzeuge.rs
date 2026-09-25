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
        kistenordner: None,
        warnung: true,
        modus: Default::default(),
        blick_bildschirm: false,
        blick_kamera: false,
        web_recherche: false,
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
        hausregel: None,
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

/// ⚑ **Ohne gesetzte Wurzel greift der Standard-Arbeitsordner**
/// (Auftrag des Projektinhabers, 2026-09-15; davor der CTF-Ordner):
/// Zeigt `MYL_ARBEITSORDNER` auf ein Verzeichnis, haengt der Agent es
/// ein, statt ganz ohne Dateiwerkzeuge dazustehen.
///
/// # ⛔️ Beides in **einem** Test, und zwar nach einem Fehlschlag in der CI
///
/// 📌 **Gefunden unter Windows am 2026-09-15.** Daneben stand ein
/// zweiter Test, der prueft, dass die verankerten Werkzeuge **ohne**
/// eingehaengtes Verzeichnis im Kasten haengen. Beide liefen
/// nebenlaeufig im **selben Prozess**, und die Umgebung ist ein Zustand
/// fuer den ganzen Prozess: Dieser Test zeigte sie auf sein
/// Wegwerf-Verzeichnis, raeumte es beim Verlassen weg, und der andere
/// griff genau dazwischen zu. Die Meldung lautete
/// `Einhaengung ...\.tmpb3mHca: kein Verzeichnis`, also ein Ordner, der
/// zwischen `canonicalize` und `is_dir` verschwand.
///
/// ⚑ **Zusammenlegen statt einen Riegel erfinden**, dieselbe
/// Entscheidung wie bei `standard_wurzel` in der Bibliothek: Ein Test,
/// der die Reihenfolge selbst in der Hand hat, wirft die Frage nicht
/// auf, wer den Riegel beim naechsten Mal noch nimmt.
///
/// ⚠️ **Die Ursache ist trotzdem am Ort behoben worden**, nicht nur hier:
/// Eine **Vorgabe**, die nicht traegt, wirft das Ruesten nicht mehr um
/// (`ruestung.rs`). Ein Test, der eine Klemme umgeht, behebt sie nicht.
#[test]
fn ohne_wurzel_greift_der_standard_arbeitsordner_und_verankertes_bleibt() {
    let agent = || Agenteneinstellung {
        schritte: 2,
        wurzel: None,
        schreiben: false,
        kistenordner: None,
        warnung: true,
        modus: Default::default(),
        blick_bildschirm: false,
        blick_kamera: false,
        web_recherche: false,
    };

    // 1. Mit gesetzter Umgebung haengt der Ordner ein.
    let alt = std::env::var_os(myl_client::einstellungen::ARBEITSORDNER);
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::env::set_var(myl_client::einstellungen::ARBEITSORDNER, d.path());

    let ruestung =
        myl_client::ruestung::ruesten(&agent(), FORM, SATZ, Vec::new()).expect("Ruestung");
    assert!(
        ruestung.einhaengung.is_some(),
        "ohne gesetzte Wurzel greift der Standard-Arbeitsordner nicht"
    );
    // Und damit gibt es wieder Lesewerkzeuge, auch ohne Schreiberlaubnis.
    assert!(!ruestung.kasten.angebote().is_empty());

    // 2. **Die verankerten Werkzeuge haengen unabhaengig davon**, und das
    //    ist die Voraussetzung dafuer, dass ein Auftrag im Chat ein
    //    Dokument erzeugen kann: Dort haengt niemand etwas ein.
    //
    // ⚠️ Nicht geprueft wird, dass **keine** Einhaengung entsteht: Aus dem
    // Repositorium heraus greift die Vorgabe `WORK_DIR` immer, und das ist
    // richtig so. Hier geht es allein darum, dass die verankerten
    // Werkzeuge auch dort im Kasten sind, wo es keine Dateiwerkzeuge gibt.
    std::env::remove_var(myl_client::einstellungen::ARBEITSORDNER);
    let ruestung =
        myl_client::ruestung::ruesten(&agent(), FORM, SATZ, Vec::new()).expect("Ruestung");
    let namen: Vec<String> =
        ruestung.kasten.angebote().iter().map(|a| a.name.clone()).collect();
    for w in myl_client::verankert::Verankert::ALLE {
        assert!(
            namen.iter().any(|n| n == w.name()),
            "{} fehlt ohne eingehaengtes Verzeichnis: {namen:?}",
            w.name()
        );
    }

    // 3. **Eine Umgebung, die ins Nichts zeigt, wird uebergangen**, und
    //    das Ruesten laeuft weiter.
    //
    // ⚠️ **Das ist nicht der Fall aus der CI.** `standard_wurzel` prueft
    // vorher `is_dir()`, ein Pfad ins Nichts kommt hier also gar nicht
    // an: Was greift, ist `WORK_DIR`. Der Fall aus der CI war ein
    // Ordner, der **zwischen** dieser Pruefung und dem Einhaengen
    // verschwand, und genau dieses Zeitfenster laesst sich nicht
    // absichtlich treffen. **Der Rueckfall in `ruestung.rs` ist deshalb
    // eine Absicherung ohne eigene Probe**, und das steht hier, damit
    // niemand die Deckung fuer groesser haelt, als sie ist.
    let weg = d.path().join("gibt-es-nicht");
    std::env::set_var(myl_client::einstellungen::ARBEITSORDNER, &weg);
    let ruestung = myl_client::ruestung::ruesten(&agent(), FORM, SATZ, Vec::new())
        .expect("eine untragbare Vorgabe darf das Ruesten nicht umwerfen");
    let namen: Vec<String> =
        ruestung.kasten.angebote().iter().map(|a| a.name.clone()).collect();
    for w in myl_client::verankert::Verankert::ALLE {
        assert!(namen.iter().any(|n| n == w.name()), "{} fehlt: {namen:?}", w.name());
    }

    // 4. **Ein gesetzter Pfad, den es nicht gibt, bleibt ein Fehler.**
    //    Er ist eine Absicht, kein Rueckfall; ein stiller Wechsel liesse
    //    den Agenten in einem anderen Ordner arbeiten als dem, der in den
    //    Einstellungen steht.
    let mut eigen = agent();
    eigen.wurzel = Some(weg.display().to_string());
    assert!(
        myl_client::ruestung::ruesten(&eigen, FORM, SATZ, Vec::new()).is_err(),
        "ein gesetzter Pfad, den es nicht gibt, faellt still auf etwas anderes zurueck"
    );

    match alt {
        Some(v) => std::env::set_var(myl_client::einstellungen::ARBEITSORDNER, v),
        None => std::env::remove_var(myl_client::einstellungen::ARBEITSORDNER),
    }
}

/// ⚑ **Die verankerte Kiste laeuft in `NurVerankert`, und das ist der
/// ganze Punkt.**
///
/// Bis zum 2026-09-15 meldete `ruestung.rs` **jedes** Werkzeug als
/// `Extern`/`Lokal` an. In der Vorgabebetriebsart sperrte der Harness
/// damit alle: Ein Netzlauf haette ein nachrechenbares Modell gehabt und
/// keine Werkzeuge. Diese Pruefung haelt fest, dass die verankerten
/// durchkommen **und** die lokalen weiterhin nicht.
#[test]
fn verankerte_werkzeuge_laufen_auch_ohne_bezeugtes() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let erg = fahren(
        d.path(),
        true,
        // ⚑ `false` heisst `Betriebsart::NurVerankert`, die Vorgabe.
        false,
        vec![ruf(
            "join_sections",
            serde_json::json!({
                "ebene": 2,
                "abschnitte": [{"titel": "Eins", "inhalt": "Text."}]
            }),
        )],
    );
    let gesagt: String = erg.nachrichten.iter().map(|n| n.content.clone()).collect();
    assert!(
        gesagt.contains("## Eins"),
        "das verankerte Werkzeug lief nicht: {gesagt}"
    );
    // Und die Gegenprobe: ein lokales bleibt in derselben Betriebsart
    // gesperrt, sonst hiesse „verankert" nichts.
    let _ = fahren(
        d.path(),
        true,
        false,
        vec![ruf(SCHREIBEN, serde_json::json!({"pfad": "x.txt", "inhalt": "y"}))],
    );
    assert!(!d.path().join("x.txt").exists(), "ein lokales Werkzeug lief in NurVerankert");
}


/// ⛔️ **Der Chatzuschnitt kommt nicht aus dem Anhangordner heraus.**
///
/// Das ist die ganze Zusage: lesen und aendern duerfen, aber nur
/// Anhaenge. Faellt diese Probe, hat ein Gespraech ohne
/// Agentenbetrieb Zugriff auf das Dateisystem.
#[test]
fn der_anhangzuschnitt_bleibt_im_anhangordner() {
    let ordner = tempfile::tempdir().expect("Ordner");
    std::fs::write(ordner.path().join("liste.md"), "- Milch\n").expect("Anhang");
    // Etwas, das NICHT erreichbar sein darf, eine Ebene darueber.
    let geheim = ordner.path().parent().expect("Elternordner").join("geheim.txt");
    std::fs::write(&geheim, "nicht fuer den Chat\n").expect("Geheimnis");

    let r = myl_client::ruestung::ruesten_fuer_anhaenge(ordner.path(), FORM, None, Vec::new(), None)
        .expect("Ruestung");

    let lies = |pfad: &str| {
        r.kasten
            .ausfuehren_ungeprueft("read_file", &serde_json::json!({ "pfad": pfad }))
            .expect("read_file fehlt im Kasten")
    };
    // Der Anhang selbst geht.
    assert!(lies("liste.md").is_ok(), "der Anhang ist nicht lesbar");

    // ⛔️ Alles darueber nicht, und zwar auf beiden Wegen.
    for hinaus in ["../geheim.txt", "../../etc/hosts"] {
        assert!(lies(hinaus).is_err(), "{hinaus} war erreichbar");
    }
    assert!(
        lies(&geheim.display().to_string()).is_err(),
        "ein absoluter Pfad kam durch"
    );
}

/// ⛔️ **Kein Manifest-Werkzeug im Chatzuschnitt.**
///
/// Ein Manifest laeuft ueber eine Shell, und eine Shell kennt die
/// Einhaengegrenze nicht. Sie waere die eine Tuer, durch die der Chat
/// doch ins Dateisystem kaeme.
#[test]
fn der_anhangzuschnitt_haengt_keine_manifeste() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let r = myl_client::ruestung::ruesten_fuer_anhaenge(ordner.path(), FORM, None, Vec::new(), None)
        .expect("Ruestung");
    let namen: Vec<String> = r.kasten.angebote().iter().map(|a| a.name.clone()).collect();
    for verboten in ["run_command", "suche_text", "dateibaum", "git_stand", "zaehle_zeilen"] {
        assert!(
            !namen.iter().any(|n| n == verboten),
            "{verboten} steht im Chatzuschnitt: {namen:?}"
        );
    }
    // ⚑ Die Gegenrichtung: Die fuenf Dateiwerkzeuge muessen da sein,
    //   sonst laesst sich kein Anhang aendern.
    for noetig in ["read_file", "write_file", "edit_file", "list_directory", "search_files"] {
        assert!(namen.iter().any(|n| n == noetig), "{noetig} fehlt: {namen:?}");
    }
}

/// ⛔️ **Der Weg hinaus hält dieselbe Grenze wie der Weg hinein.**
///
/// `anhang_herausgeben` im Fenster loest den Namen ueber dieselbe
/// Einhaengung auf. Ein Befehl, der jeden Pfad nimmt, den ihm jemand
/// nennt, waere eine Tuer neben der Tuer; diese Probe haelt fest, dass
/// `aufloesen` das traegt.
#[test]
fn der_weg_hinaus_weist_pfade_ausserhalb_ab() {
    let ordner = tempfile::tempdir().expect("Ordner");
    std::fs::write(ordner.path().join("liste.md"), "- Milch\n").expect("Anhang");
    let geheim = ordner.path().parent().expect("Elternordner").join("geheim-hinaus.txt");
    std::fs::write(&geheim, "nicht hinaus\n").expect("Geheimnis");

    let ein = myl_client::werkzeuge::Einhaengung::neu(ordner.path(), false).expect("Einhaengung");
    assert!(ein.aufloesen("liste.md", true).is_ok(), "der Anhang selbst muss gehen");
    for hinaus in ["../geheim-hinaus.txt", "../../etc/hosts"] {
        assert!(ein.aufloesen(hinaus, true).is_err(), "{hinaus} kam durch");
    }
    assert!(
        ein.aufloesen(&geheim.display().to_string(), true).is_err(),
        "ein absoluter Pfad kam durch"
    );
}

/// ⛔️ **Im `manual mode` fragt alles, was nach aussen wirkt, und die
/// Absage haelt es auf, bevor es wirkt**: ein geaenderter Anhang und
/// jede Web-Anfrage. Lesen fragt nicht.
///
/// 📌 Bis zum 2026-09-25 liefen Web-Anfragen ohne Nachfrage, und der Chat
/// mit Anhang reichte gar keine Nachfrage durch.
#[test]
fn die_nachfrage_haelt_schreiben_und_netz_auf_und_laesst_lesen() {
    let ordner = tempfile::tempdir().expect("Ordner");
    std::fs::write(ordner.path().join("liste.md"), "- Milch\n").expect("Anhang");
    let gefragt = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let g = std::sync::Arc::clone(&gefragt);
    let nachfrage: myl_client::ruestung::Nachfrage = std::sync::Arc::new(move |name: &str, _: &serde_json::Value| {
        g.lock().unwrap().push(name.to_string());
        false
    });
    let r = myl_client::ruestung::ruesten_fuer_anhaenge(
        ordner.path(),
        FORM,
        Some("lies bitte https://example.org/seite"),
        Vec::new(),
        Some(nachfrage),
    )
    .expect("Ruestung");

    let web = r
        .kasten
        .angebote()
        .iter()
        .map(|a| a.name.clone())
        .find(|n| n.starts_with("web_") && (n.ends_with("read") || n.ends_with("lesen")))
        .expect("kein Web-Lesewerkzeug im Kasten");
    let aus = r
        .kasten
        .ausfuehren_ungeprueft(&web, &serde_json::json!({ "url": "https://example.org/seite" }))
        .expect("Werkzeug fehlt");
    assert!(aus.is_err(), "die Web-Anfrage lief trotz Absage");

    let aus = r
        .kasten
        .ausfuehren_ungeprueft("write_file", &serde_json::json!({ "pfad": "liste.md", "inhalt": "weg" }))
        .expect("write_file fehlt");
    assert!(aus.is_err(), "das Schreiben lief trotz Absage");
    assert_eq!(std::fs::read_to_string(ordner.path().join("liste.md")).unwrap(), "- Milch\n");

    let vorher = gefragt.lock().unwrap().len();
    let aus = r
        .kasten
        .ausfuehren_ungeprueft("read_file", &serde_json::json!({ "pfad": "liste.md" }))
        .expect("read_file fehlt");
    assert!(aus.is_ok());
    let g = gefragt.lock().unwrap();
    assert_eq!(g.len(), vorher, "Lesen hat gefragt: {g:?}");
    assert_eq!(g.as_slice(), [web.as_str(), "write_file"], "gefragt wurde: {g:?}");
}

/// ⛔️ **Die Vorgabe ist `manual mode`**, auch fuer eine Ablage ohne das
/// Feld; wer `auto` ausdruecklich gewaehlt hat, behaelt es.
#[test]
fn die_vorgabe_fragt_nach() {
    let e = myl_client::Einstellungen::default();
    assert!(e.agent.modus.fragt_nach(), "die Vorgabe fragt nicht");
    let ohne: myl_client::einstellungen::Agenteneinstellung = serde_json::from_value(
        serde_json::to_value(&e.agent)
            .map(|mut v| {
                v.as_object_mut().unwrap().remove("modus");
                v
            })
            .unwrap(),
    )
    .expect("ohne Feld lesbar");
    assert!(ohne.modus.fragt_nach(), "eine alte Ablage fragt nicht");
    let mut v = serde_json::to_value(&e.agent).unwrap();
    v["modus"] = serde_json::json!("auto");
    let auto: myl_client::einstellungen::Agenteneinstellung = serde_json::from_value(v).unwrap();
    assert!(!auto.modus.fragt_nach(), "eine ausdrueckliche Wahl wurde ueberschrieben");
}

/// ⛔️ **Das Aktionsprotokoll haelt jede Handlung fest, auch eine
/// abgelehnte, und kein Klartext steht darin.**
#[test]
fn das_aktionsprotokoll_haelt_fest_ohne_klartext() {
    let ablage = tempfile::tempdir().expect("Protokollordner");
    myl_client::protokoll::ordner_setzen(ablage.path().to_path_buf());
    let ordner = tempfile::tempdir().expect("Ordner");
    std::fs::write(ordner.path().join("geheimliste.md"), "- Vertrauliches\n").expect("Anhang");
    let nein: myl_client::ruestung::Nachfrage = std::sync::Arc::new(|_: &str, _: &serde_json::Value| false);
    let r = myl_client::ruestung::ruesten_fuer_anhaenge(ordner.path(), FORM, None, Vec::new(), Some(nein))
        .expect("Ruestung");
    r.kasten
        .ausfuehren_ungeprueft("read_file", &serde_json::json!({ "pfad": "geheimliste.md" }))
        .expect("read_file")
        .expect("gelesen");
    let _ = r
        .kasten
        .ausfuehren_ungeprueft("write_file", &serde_json::json!({ "pfad": "geheimliste.md", "inhalt": "weg" }))
        .expect("write_file");

    // ⚑ **Gefunden ueber den Fingerabdruck der eigenen Eingabe**: Der
    //   Protokollort gilt je Prozess, und andere Proben dieser Datei
    //   protokollieren nebenher mit hinein. 📌 Die erste Fassung nahm die
    //   juengste `read_file`-Zeile und erwischte dabei ein fremdes,
    //   absichtlich scheiterndes Lesen.
    let e = myl_client::protokoll::lesen(1000);
    let fa = |v: serde_json::Value| myl_client::protokoll::fingerabdruck(v.to_string().as_bytes());
    let gelesen = fa(serde_json::json!({ "pfad": "geheimliste.md" }));
    let lesen = e.iter().find(|x| x.eingabe == gelesen).expect("Lesen fehlt im Protokoll");
    assert_eq!(
        (lesen.werkzeug.as_str(), lesen.art.as_str(), lesen.entscheidung.as_str(), lesen.ergebnis.as_str()),
        ("read_file", "datei_lesen", "ausgefuehrt", "ok")
    );
    assert_eq!(lesen.eingabe.len(), 32, "kein Fingerabdruck: {lesen:?}");
    let geschrieben = fa(serde_json::json!({ "pfad": "geheimliste.md", "inhalt": "weg" }));
    let schreiben = e.iter().find(|x| x.eingabe == geschrieben).expect("Schreiben fehlt im Protokoll");
    assert_eq!(schreiben.werkzeug, "write_file");
    assert_eq!(schreiben.art, "datei_schreiben");
    assert_eq!(schreiben.entscheidung, "abgelehnt");

    // Kein Klartext: weder Pfad noch Inhalt in irgendeiner Datei.
    for d in std::fs::read_dir(ablage.path()).unwrap().flatten() {
        let t = std::fs::read_to_string(d.path()).unwrap_or_default();
        for verboten in ["geheimliste", "Vertrauliches", "weg\""] {
            assert!(!t.contains(verboten), "`{verboten}` steht im Protokoll: {}", d.path().display());
        }
    }
}
