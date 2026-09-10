//! Die Oberflaeche von Myelith, Ruecken.
//!
//! # ⚑ Was hier NICHT stehen darf
//!
//! **Keine Logik.** Die Oberflaeche ruft dieselben Unterbefehle wie das
//! Kommandozeilenwerkzeug, und der Grund ist nicht Aesthetik.
//! Zwei Wege zu derselben Sache laufen auseinander, und der zweite ist
//! immer der schlechter gepruefte: `myl-client` ist durchgeprueft, ein
//! nachgebauter Ruecken waere es nicht.
//!
//! ⛑ **Hier stand „sechsundfuenfzig Pruefungen", und es waren
//! neunundachtzig.** Eine Zahl aus einer anderen Kiste laesst sich von
//! hier aus nicht binden, also steht sie nicht mehr da: Eine
//! Behauptung, die niemand nachrechnet, altert unbemerkt.
//!
//! ⚑ **Deshalb ruft dieser Ruecken die Kiste `myl-client`**, dieselbe
//! Bibliothek, die auch das Kommandozeilenwerkzeug ruft, und nicht das
//! Binaerprogramm als Unterprozess. Ein Unterprozess muesste seine
//! Textausgabe wieder zerlegen, und aus einer Ausgabe fuer Menschen
//! eine Schnittstelle zu machen ist genau die Logik, die hier nicht
//! hingehoert.
//!
//! # ⚑ Und warum der Agentenlauf einen eigenen Faden bekommt
//!
//! **Ein Modell laedt zehn Sekunden**, und eine Antwort dauert noch
//! einmal so lange. Ein Ruecken, der das im Befehlsfaden taete, liesse
//! das Fenster so lange stehen, und ein stehendes Fenster sieht aus wie
//! ein abgestuerztes.
//!
//! ⚑ **Das geladene Modell lebt deshalb im Zustand**, nicht im Befehl:
//! `Oertlichesmodell` ist `Send + Sync`, und derselbe Umstand, der die
//! Unteragenten moeglich macht, macht auch das hier moeglich.

use std::sync::Mutex;

use serde::Serialize;
use tauri::Emitter;

/// Was die Oberflaeche ueber die Einstellungen wissen will.
///
/// ⚑ **Eine eigene Form und nicht `Einstellungen` selbst.** Die
/// abgelegte Form ist die des Nutzers und aendert sich nach seinen
/// Beduerfnissen; was die Oberflaeche zeigt, ist eine Ansicht darauf.
/// Wer beides gleichsetzt, kann keines der beiden mehr aendern.
#[derive(Serialize)]
struct Ansicht {
    /// ⚑ **Jedes setzbare Feld mit seinem Wert, unter seinem Namen.**
    ///
    /// ⛑ **Hier standen bis zum 2026-09-10 zwoelf einzelne Felder**,
    /// und das Fenster hielt daneben eine zweite Zuordnung von
    /// Feldnamen auf diese Felder. Die kannte drei von zwoelf nicht
    /// (Fund 280), und in JavaScript ist ein fehlender Schluessel kein
    /// Fehler, sondern `undefined`: Der Schalter stand immer aus, die
    /// Textfelder immer leer.
    ///
    /// ⚑ **Jetzt gibt es die Zuordnung nur noch einmal**, und zwar in
    /// der Kiste, wo auch der Setzer liegt. Ein Feld, das dazukommt,
    /// erscheint hier von selbst; eines, das keinen Wert hergibt, kann
    /// es nicht mehr geben.
    werte: std::collections::BTreeMap<String, myl_client::einstellungen::Feldwert>,
    /// ⚑ **Die Sprache, in der das Fenster spricht.** Sie steht auch in
    /// `werte["oberflaeche.sprache"]`; hier steht sie noch einmal, weil
    /// das Fenster sie **vor** dem ersten Zeichnen braucht und nicht
    /// erst, wenn es die Einstellungsseite oeffnet.
    sprache: String,
    /// ⚑ Und was von der Kernfreigabe **wirkt**. Eine Grenze ueber der
    /// Maschine hebt sie nicht an, und der Unterschied gehoert sichtbar.
    kerne_wirksam: usize,
    /// Was die Plattenfreigabe gerade wirklich haelt, in Bytes.
    platte_gehalten: u64,
    /// Was Myelith heute an Modellen und Artefakten haelt, in Bytes.
    platte_belegt: u64,
    /// ⚑ **Die Betriebsart als dauerhafter Zustand.** Auf der
    /// Kommandozeile genuegt ein Satz beim Start; hier nicht. Wer nicht
    /// sieht, in welcher Betriebsart er arbeitet, hat keine Wahl
    /// getroffen, sondern eine geerbt.
    betriebsart: String,
    /// ⚑ **Der ganze Satz dahinter**, dort, wo man ihn sucht, naemlich
    /// am selben Ding.
    betriebsart_warum: String,
    pfad: String,
}

/// Was in der Marke im Kopf steht, und der Satz dahinter.
///
/// # ⛑ Warum es diese Funktion gibt
///
/// Hier stand „Alles: bezeugte Werkzeuge laufen" und
/// „NurVerankert: lokale Werkzeuge sind gesperrt". Beides ist die
/// Sprache des Protokolls und nicht die des Nutzers: `bezeugt`,
/// `verankert` und `Alles` sind Begriffe aus Kapitel 8.1, und wer sie
/// nicht gelesen hat, liest in seinem Fensterkopf ein Wort ohne Sinn.
/// Der Einwand kam vom Projektinhaber, und er hat recht.
///
/// ⚑ **Uebersetzt wird in die Frage, die der Nutzer wirklich hat:
/// Darf der Agent an meine Dateien?** Im Klienten sind alle Werkzeuge
/// `Extern` und `Lokal`, also ist das genau dieselbe Frage wie die nach
/// der Betriebsart, nur in Worten, die man ohne das Kapitel versteht.
///
/// ⚑ **Und es sind drei Zustaende und nicht zwei.** Ohne eingehaengtes
/// Verzeichnis gibt es gar keine Werkzeuge, ganz unabhaengig von der
/// Betriebsart; wer dann „gesperrt" laese, suchte den Fehler an der
/// falschen Stelle.
///
/// Die technischen Namen bleiben, wo sie hingehoeren: im
/// Sitzungsstrom, in den Protokollen und in `Betriebsart::name`.
fn kurzform(a: &myl_client::einstellungen::Agenteneinstellung) -> (&'static str, &'static str) {
    match (a.wurzel.is_some(), a.auch_bezeugtes, a.schreiben) {
        (false, _, _) => (
            "ohne Werkzeuge",
            "Es ist kein Verzeichnis eingehaengt, der Agent hat also keine Werkzeuge. \
             Setze `agent.wurzel` in den Einstellungen.",
        ),
        (true, false, _) => (
            "Werkzeuge gesperrt",
            "Ein Verzeichnis ist eingehaengt, aber die Betriebsart laesst nur Schritte zu, \
             die ein Dritter nachrechnen kann. Dateiwerkzeuge kann niemand nachrechnen, \
             also bleiben sie gesperrt. Zu aendern mit `agent.bezeugtes`.",
        ),
        (true, true, false) => (
            "liest Dateien",
            "Der Agent darf im eingehaengten Verzeichnis lesen und suchen, aber nichts \
             aendern. Schreiben ist eine eigene Erlaubnis: `agent.schreiben`.",
        ),
        (true, true, true) => (
            "liest und schreibt",
            "Der Agent darf im eingehaengten Verzeichnis lesen, suchen, schreiben und \
             aendern. Ausserhalb kommt er nicht.",
        ),
    }
}

#[tauri::command]
fn einstellungen(halter: tauri::State<'_, Halter>) -> Result<Ansicht, String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let e = myl_client::Einstellungen::lesen(&pfad)?;
    if let Some(n) = e.kapazitaet.kerne {
        myl_client::kapazitaet::kerne_setzen(n);
    }
    // ⚑ **Die Reservierung wird bei jedem Blick nachgefuehrt.** Ein
    // Download hat vielleicht Platz verbraucht, und dann stimmt die
    // Summe aus belegt und reserviert nicht mehr.
    let platte_gehalten = platte_nachfuehren(&halter).unwrap_or(0);

    // ⚑ **Die Werte kommen aus der Kiste, Feld fuer Feld.** Kein Name
    // steht hier zweimal, und keiner fehlt: Die Liste ist dieselbe, aus
    // der die Seite ihre Zeilen zeichnet.
    let mut werte = std::collections::BTreeMap::new();
    for f in myl_client::einstellungen::FELDER {
        werte.insert(f.name.to_string(), e.wert(f.name)?);
    }
    // Dazu die Freigaben je Rechenwerk, deren Namen erst der Scan kennt.
    for r in &myl_client::hardware::Hardware::erheben(&datenort()).rechenwerke {
        let name = format!("{}{}", myl_client::einstellungen::RECHENWERK_PRAEFIX, r.kennung);
        let w = e.wert(&name)?;
        werte.insert(name, w);
    }

    Ok(Ansicht {
        werte,
        sprache: e.oberflaeche.sprache.kennung().to_string(),
        kerne_wirksam: myl_client::kapazitaet::kerne(),
        platte_gehalten,
        platte_belegt: belegung_heute(),
        betriebsart: kurzform(&e.agent).0.into(),
        betriebsart_warum: kurzform(&e.agent).1.into(),
        pfad: pfad.display().to_string(),
    })
}

/// Loescht ein gebautes Artefakt.
///
/// # ⛑ Was hier alles schiefgehen koennte, und was es verhindert
///
/// **Das ist die einzige Stelle im Klienten, die etwas Grosses und
/// Unwiederbringliches loescht**, und sie tut es auf einen Klick.
/// Deshalb steht vor `remove_dir_all` eine Kette von Bedingungen, und
/// jede einzelne hat einen Fall, den sie abfaengt:
///
/// | Bedingung | Was sie verhindert |
/// |---|---|
/// | Schluessel steht im Katalog | ein erfundener Name, der irgendwohin zeigt |
/// | Pfad liegt **unter** `artifacts/` | ein `..` im Schluessel |
/// | `model_config.json` liegt darin | ein Verzeichnis, das kein Artefakt ist |
/// | nicht gerade geladen | ein Modell, das mitten im Betrieb verschwindet |
///
/// ⚑ **Das Rohmodell bleibt.** Es liegt unter `models/` und ist das
/// Teure am Beschaffen; wer sein Artefakt loescht, will Platz und nicht
/// noch einmal Stunden Download. Ein zweiter Bau geht dann in Minuten.
#[tauri::command]
fn artefakt_loeschen(
    halter: tauri::State<'_, Halter>,
    schluessel: String,
) -> Result<String, String> {
    let w = wurzel_suchen().ok_or("Das Repositorium ist nicht zu finden")?;

    // 1. Der Schluessel muss im Katalog stehen.
    let roh = std::fs::read_to_string(w.join("INTEGER_LLM/models/KATALOG.json"))
        .map_err(|e| format!("KATALOG.json: {e}"))?;
    let d: serde_json::Value = serde_json::from_str(&roh).map_err(|e| e.to_string())?;
    if d.get(&schluessel).is_none() {
        return Err(format!("{schluessel} steht nicht im Katalog"));
    }

    // 2. Der Pfad muss unter `artifacts/` liegen, aufgeloest und nicht
    //    zusammengesetzt: Ein `..` im Schluessel zeigte sonst hinaus.
    let ordner = w.join("INTEGER_LLM/artifacts");
    let ziel = ordner.join(&schluessel);
    let echt = ziel.canonicalize().map_err(|e| format!("{}: {e}", ziel.display()))?;
    let heim = ordner.canonicalize().map_err(|e| format!("{}: {e}", ordner.display()))?;
    if !echt.starts_with(&heim) || echt == heim {
        return Err("Dieser Pfad liegt nicht im Artefaktverzeichnis".to_string());
    }

    // 3. Es muss ein Artefakt sein und nicht irgendein Verzeichnis.
    if !echt.join("model_config.json").is_file() {
        return Err(format!("{schluessel} sieht nicht wie ein Artefakt aus"));
    }

    // 4. Und es darf nicht das geladene sein.
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let geladen = halter.modell.lock().map(|g| g.is_some()).unwrap_or(false);
    if geladen && e.modell.artefakt.trim_end_matches('/').ends_with(&schluessel) {
        return Err(
            "Dieses Artefakt ist gerade geladen. Waehle ein anderes Modell und lade es, \
             dann laesst es sich loeschen."
                .to_string(),
        );
    }

    let belegt = myl_client::reservierung::belegung(&echt);
    std::fs::remove_dir_all(&echt).map_err(|e| format!("{}: {e}", echt.display()))?;
    // ⚑ Der freigewordene Platz geht zurueck in die Reservierung, sonst
    // stimmte die Summe aus belegt und reserviert nicht mehr.
    let _ = platte_nachfuehren(&halter);
    Ok(format!(
        "{schluessel} geloescht, {:.1} GiB frei. Das Rohmodell bleibt; ein zweiter Bau \
         braucht keinen Download.",
        belegt as f64 / myl_client::hardware::GIB as f64
    ))
}

/// Zerlegt eine Modellantwort in Bloecke, die das Fenster zeichnen kann.
///
/// # ⛑ Warum das ueber den Ruecken geht und nicht im Skript geschieht
///
/// **Die Antwort eines Modells ist Text und keine Auszeichnung.** Wer
/// sie mit `innerHTML` in die Seite schreibt, macht aus Daten
/// Steuerung, und diese Seite traegt wegen `withGlobalTauri` die
/// Bruecke zu **allen** Befehlen dieses Rueckens: Ein eingeschleuster
/// Satz koennte Einstellungen setzen oder Dateien schreiben.
///
/// ⚑ **Deshalb kommt hier ein Baum aus Text heraus**, und das Fenster
/// setzt ihn mit `createElement` und `textContent` zusammen. Ein `<`
/// bleibt dabei ein `<`, gleich was davor steht. Das ist keine
/// Filterung, sondern eine Bauart: Es gibt keinen Weg, auf dem aus
/// dieser Antwort Markup wuerde.
///
/// ⚑ **Und es ist dieselbe Arbeitsteilung wie ueberall hier:** Die
/// Kiste weiss, das Fenster zeichnet.
#[tauri::command]
fn markdown(text: String) -> Vec<myl_client::markdown::Block> {
    myl_client::markdown::zerlegen(&text)
}

/// Was diese Maschine hergibt, und was davon freigegeben ist.
///
/// ⚑ **Ein eigener Befehl und kein Teil der Einstellungen.** Der Scan
/// ruft Systemwerkzeuge auf und kostet damit mehr als das Lesen einer
/// Datei; die Einstellungsseite fragt ihn einmal beim Oeffnen und nicht
/// nach jedem gesetzten Feld.
#[tauri::command]
fn hardware() -> Result<Freigabemaske, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let h = myl_client::hardware::Hardware::erheben(&datenort());
    Ok(Freigabemaske { regler: h.regler(&e), hardware: h, ort: datenort().display().to_string() })
}

/// Was die Freigabemaske zu zeichnen braucht.
#[derive(Serialize)]
struct Freigabemaske {
    /// Ein Regler je Betriebsmittel, feste und gefundene zusammen.
    regler: Vec<myl_client::hardware::Regler>,
    /// Der Scan selbst, fuer die Zeile darueber.
    hardware: myl_client::hardware::Hardware,
    /// Worauf sich die Plattenzahlen beziehen.
    ort: String,
}

/// Was ein Artefakt hergibt, ohne es zu laden.
///
/// ⚑ **Ohne es zu laden**, denn das kostet bei einem 4B-Modell zehn
/// Sekunden. Die Vorlage folgt aus der Familie und steht im Katalog.
/// Die setzbaren Felder mit Art, Bereich, Beschriftung und Hinweis.
///
/// ⚑ **Aus der Kiste und nicht hier aufgezaehlt.** Eine zweite Liste im
/// Fenster liefe irgendwann auseinander, und dann zeigt die Oberflaeche
/// ein Feld, das der Setzer nicht kennt.
///
/// ⛑ **Auch die Beschriftung kommt von dort.** Dieser Befehl gab
/// einmal nur Name und Art zurueck; das Fenster zeigte daraufhin
/// `kap.beschleuniger` als Beschriftung, weil es nichts Besseres
/// hatte. Die Alternative waere eine Uebersetzungstabelle im Skript
/// gewesen, also wieder zwei Listen.
///
/// ⚑ **Und in der eingestellten Sprache** (seit dem 2026-09-10). Die
/// Uebersetzung liegt in der Kiste neben der Beschriftung selbst: Ein
/// Fenster, das zwischen `titel` und `titel_en` waehlt, waere die
/// zweite Stelle, an der die naechste Sprache vergessen werden kann.
#[tauri::command]
fn felder() -> Result<Vec<myl_client::einstellungen::Feld>, String> {
    let s = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?
        .oberflaeche
        .sprache;
    Ok(myl_client::einstellungen::FELDER
        .iter()
        .map(|f| f.in_sprache(s))
        .collect())
}

/// **Sieht nach, ob `origin` etwas hat, das dieser Klon nicht hat.**
///
/// ⚑ **Es aendert nichts.** Nachsehen und Einspielen sind zwei
/// Befehle, weil sie zwei Entscheidungen sind: Die eine kostet eine
/// Netzanfrage, die andere Minuten und einen neuen Programmstand.
#[tauri::command]
fn aktualisierung() -> myl_client::aktualisierung::Stand {
    myl_client::aktualisierung::pruefen(env!("CARGO_PKG_VERSION"))
}

/// **Spielt den neuen Stand ein und meldet dabei jede Zeile.**
///
/// ⚠️ **Es baut neu, und das dauert Minuten.** Ohne die Meldungen waere
/// ein laufender Bau von einem haengenden Programm nicht zu
/// unterscheiden.
#[tauri::command]
async fn aktualisieren(fenster: tauri::AppHandle) -> Result<(), String> {
    let f = fenster.clone();
    tauri::async_runtime::spawn_blocking(move || {
        myl_client::aktualisierung::einspielen(&|zeile| {
            let _ = f.emit("aktualisierung-zeile", zeile.to_string());
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Setzt ein Feld und schreibt die Ablage.
///
/// ⛑ **Auch hier keine eigene Logik.** Was `aus` heisst und welche
/// Felder es gibt, weiss die Kiste; dieser Befehl reicht durch und
/// speichert.
#[tauri::command]
fn setzen(
    halter: tauri::State<'_, Halter>,
    feld: String,
    wert: String,
) -> Result<(), String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let mut e = myl_client::Einstellungen::lesen(&pfad)?;
    e.setzen(&feld, &wert)?;
    e.schreiben(&pfad)?;

    // ⚑ **Eine Freigabe wirkt beim Setzen und nicht beim naechsten
    // Start.** Wer den Regler bewegt und danach dasselbe Verhalten
    // sieht, hat keinen Regler bedient, sondern eine Zahl geaendert.
    match feld.as_str() {
        "kap.kerne" => myl_client::kapazitaet::kerne_setzen(e.kapazitaet.kerne.unwrap_or(0)),
        // ⛑ **Die Platte wird sofort gehalten oder hergegeben.** Sonst
        // stuende zwischen dem Setzen und dem naechsten Start eine
        // Zusage, die niemand einloest.
        "kap.platte" => {
            platte_nachfuehren(&halter)?;
        }
        _ => {}
    }
    Ok(())
}

/// Welche Modelle zur Wahl stehen.
///
/// # ⚑ Gesucht wird im Elternverzeichnis des eingestellten Artefakts
///
/// `modell.artefakt` zeigt auf ein Artefakt; daneben liegen die
/// anderen. Ein fest verdrahteter Pfad waere eine Annahme darueber, wo
/// jemand seine Artefakte haelt, und die trifft bei einem frischen
/// Klon nicht.
///
/// ⚑ **Erkannt wird an `model_config.json`.** Ein Verzeichnis ohne die
/// Datei ist kein Artefakt, und eines mit ihr laesst sich laden.
///
/// # ⚠️ Der Netzeintrag steht da und traegt nicht
///
/// „API, kostet Inferenz-Credits" ist der Platz fuer das Netzmodell.
/// Er ist **gesperrt**, solange dem Klienten Knotenadresse und
/// Vollmacht fehlen (Punkte 2.2 bis 2.5b). Er steht trotzdem in der
/// Liste, weil die Wahl zwischen hier und dort an genau diese Stelle
/// gehoert und nicht in einen Schalter im Kopf: Ein Modell ist ein
/// Modell, ob es auf dieser Maschine liegt oder im Netz gerechnet wird.
#[tauri::command]
fn modelle() -> Result<Vec<Modellwahl>, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let mut aus: Vec<Modellwahl> = Vec::new();

    let aufgeloest = artefakt_absolut(&e.modell.artefakt);
    let hier = std::path::Path::new(&aufgeloest);
    if let Some(eltern) = hier.parent() {
        if let Ok(lesen) = std::fs::read_dir(eltern) {
            let mut pfade: Vec<std::path::PathBuf> =
                lesen.flatten().map(|x| x.path()).filter(|p| p.join("model_config.json").is_file()).collect();
            pfade.sort();
            // ⚑ Der Anzeigename kommt aus dem Katalog, wenn er dort
            // steht: In der Wahl soll „Myelith 4B" stehen und nicht
            // der Verzeichnisname `myelith-4b`. Steht er nicht drin,
            // bleibt der Verzeichnisname, denn ein erfundener Name
            // waere schlechter als ein technischer.
            let namen = katalognamen();
            for p in pfade {
                let ordner =
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let name = namen.get(&ordner).cloned().unwrap_or_else(|| ordner.clone());
                aus.push(Modellwahl {
                    pfad: p.display().to_string(),
                    name,
                    offen: true,
                    warum: String::new(),
                });
            }
        }
    }
    // ⛑ Steht das eingestellte Artefakt nicht in der Liste (weil es
    // woanders liegt oder noch nicht existiert), kommt es trotzdem
    // dazu: Sonst zeigte die Wahl etwas anderes als das, was gilt.
    if !aus.iter().any(|m| m.pfad == e.modell.artefakt) {
        aus.insert(
            0,
            Modellwahl {
                pfad: e.modell.artefakt.clone(),
                name: format!("{} (eingestellt)", e.modell.artefakt),
                offen: hier.join("model_config.json").is_file(),
                warum: "Der eingestellte Pfad enthaelt kein `model_config.json`.".into(),
            },
        );
    }
    aus.push(Modellwahl {
        pfad: "netz".into(),
        // ⚑ Wortlaut des Projektinhabers, mit Komma statt Strich
        //   nach der Hausregel.
        name: "API, kostet Inferenz-Credits".into(),
        offen: false,
        warum: "Noch nicht verdrahtet: Dem Klienten fehlen Knotenadresse und Vollmacht. \
                Bis dahin rechnet diese Maschine."
            .into(),
    });
    Ok(aus)
}

/// Verzeichnisname zu Anzeigename, aus dem Katalog.
fn katalognamen() -> std::collections::BTreeMap<String, String> {
    let mut aus = std::collections::BTreeMap::new();
    let Some(w) = wurzel_suchen() else { return aus };
    let Ok(roh) = std::fs::read_to_string(w.join("INTEGER_LLM/models/KATALOG.json")) else {
        return aus;
    };
    let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else { return aus };
    let Some(o) = d.as_object() else { return aus };
    for (k, v) in o {
        if k.starts_with('_') {
            continue;
        }
        // ⛑ **Hier hing die Herkunft am Namen** (`Myelith 4B (aus
        // Qwen3-4B)`), und sie stand damit in jeder Modellwahl, in
        // jeder Ladezeile und in jeder Meldung. Gemeldet vom
        // Projektinhaber am 2026-09-10.
        //
        // ⚑ **Ein Name ist ein Name.** Die Herkunft ist eine Angabe zum
        // Modell und gehoert dorthin, wo Angaben zum Modell stehen: in
        // die Modellkarte. Wer sie wissen will, liest sie dort; wer nur
        // wissen will, welches Modell antwortet, soll nicht jedes Mal
        // eine Klammer mitlesen.
        //
        // ⚠️ **Verschwinden darf sie deshalb nicht.** Die Basismodelle
        // stehen unter Apache 2.0, und ein Name ohne Herkunft waere
        // eine Verschleierung. Sie steht weiterhin in `KATALOG.json`
        // bei jedem Eintrag und in `artifacts/MODEL_CARD.md`.
        if let Some(n) = v.get("anzeigename").and_then(|x| x.as_str()) {
            aus.insert(k.clone(), n.to_string());
        }
    }
    aus
}

/// Ein Eintrag der Modellwahl.
#[derive(Serialize)]
struct Modellwahl {
    pfad: String,
    name: String,
    /// ⚑ `false` heisst: waehlbar sichtbar, aber nicht benutzbar, und
    /// `warum` sagt weshalb. Ein Eintrag, der still nichts tut, waere
    /// schlimmer als keiner.
    offen: bool,
    warum: String,
}

/// Welche Werkzeuge der Agent gerade hat.
///
/// # ⛑ Warum das ein Befehl ist und keine Liste im Fenster
///
/// Die Namen haengen an der Ansageform und die Auswahl an der
/// Einhaengung; wer sie im Fenster nachbaute, haette eine zweite
/// Liste, und zwei Listen laufen auseinander. Dieselbe Begruendung wie
/// bei `felder`.
///
/// # ⚑ Und warum die Oberflaeche sie ueberhaupt zeigen muss
///
/// Die Marke im Kopf sagt „liest und schreibt", aber nicht **worauf**.
/// Ein Nutzer, der einen Auftrag abschickt, soll vorher sehen, welches
/// Verzeichnis der Agent anfassen darf und mit welchen Werkzeugen. Das
/// ist keine Zierde: Es ist die einzige Stelle, an der die
/// Einhaengegrenze fuer einen Menschen sichtbar wird.
#[tauri::command]
fn werkzeuge() -> Result<Werkzeugliste, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let Some(pfad) = e.agent.wurzel.as_deref() else {
        return Ok(Werkzeugliste { wurzel: None, namen: Vec::new() });
    };
    // ⚑ Die Einhaengung wird hier wirklich gebaut und nicht geraten:
    // Ein Pfad, der nicht existiert, hat auch keine Werkzeuge, und das
    // soll man sehen, bevor der Auftrag laeuft.
    let ein = match myl_client::werkzeuge::Einhaengung::neu(pfad, e.agent.schreiben) {
        Ok(x) => x,
        Err(m) => return Ok(Werkzeugliste { wurzel: Some(format!("{pfad}  ({m})")), namen: Vec::new() }),
    };
    let namen = myl_client::werkzeuge::angebote(
        &ein,
        myl_client::Ansageform::Amtlich,
        myl_client::werkzeuge::Werkzeugsatz::default(),
    )
    .into_iter()
    .map(|w| w.name)
    .collect();
    Ok(Werkzeugliste { wurzel: Some(ein.wurzel().display().to_string()), namen })
}

/// Was der Agent anfassen darf.
#[derive(Serialize)]
struct Werkzeugliste {
    /// ⚑ `None` heisst: kein Verzeichnis eingehaengt, also gar keine
    /// Werkzeuge, und nicht „unbekannt".
    wurzel: Option<String>,
    namen: Vec<String>,
}

/// Das geladene Modell, samt seiner Ruestung.
///
/// ⚑ **Einmal geladen, viele Auftraege**, genau wie `myl sitzung`. Der
/// teuerste Teil eines Auftrags ist das Laden, und er haengt an der
/// Plattengroesse des Modells und nicht an der Laenge der Frage.
#[derive(Default)]
struct Halter {
    /// ⚑ **Hinter `Arc`, damit der Rechenfaden ihn mitnehmen kann.**
    /// Ein `State` lebt nur so lange wie der Befehl; was in einen
    /// eigenen Faden wandert, muss `'static` sein.
    modell: std::sync::Arc<Mutex<Option<myl_client::Oertlichesmodell>>>,
    /// ⚑ **Der Plattenplatz, den dieses Fenster wirklich haelt.**
    ///
    /// Er liegt hier und nicht in einer Funktion, und das ist der ganze
    /// Punkt: **Ein Wert im Fensterzustand lebt genau so lange wie das
    /// Fenster.** „Solange das Programm geoeffnet ist" ist damit keine
    /// Absichtserklaerung, sondern die Lebensdauer eines Wertes; wer
    /// das Fenster schliesst, gibt den Platz zurueck, ohne dass jemand
    /// daran denken muss.
    platte: std::sync::Arc<Mutex<Option<myl_client::reservierung::Reservierung>>>,
}

/// Wie ein geladenes Modell heisst und wo es liegt.
///
/// ⚑ **Beides, und nicht eines von beiden.** Der Name sagt, welches
/// Modell antwortet; der Pfad sagt, **welches Artefakt** das ist, und
/// bei vier Ordnern nebeneinander ist das nicht dasselbe. Wer nur den
/// Namen zeigt, laesst offen, ob gerade das frisch gebaute oder das
/// alte antwortet.
#[derive(Serialize)]
struct Ladung {
    name: String,
    pfad: String,
    sekunden: f64,
}

/// Der Anzeigename zu einem Artefaktpfad, aus dem Katalog.
///
/// ⚑ **Ohne Katalogeintrag der Verzeichnisname**, und kein erfundener:
/// Ein Name, den niemand vergeben hat, waere schlechter als ein
/// technischer.
fn anzeigename(pfad: &str) -> String {
    let ordner = std::path::Path::new(pfad)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pfad.to_string());
    katalognamen().get(&ordner).cloned().unwrap_or(ordner)
}

/// Laedt das Artefakt aus den Einstellungen.
#[tauri::command]
async fn modell_laden(halter: tauri::State<'_, Halter>) -> Result<Ladung, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if e.modell.artefakt.is_empty() {
        return Err("es ist kein Artefakt eingestellt".into());
    }
    let anfang = std::time::Instant::now();
    let pfad = artefakt_absolut(&e.modell.artefakt);
    let mut m = myl_client::Oertlichesmodell::laden(&pfad, &e.kapazitaet)
        .map_err(|f| mit_zugriffshinweis(f, &pfad))?;
    m.grenze = e.modell.token;
    m.denken = e.modell.denken;
    let dauer = anfang.elapsed().as_secs_f64();
    *halter.modell.lock().map_err(|_| "der Modellhalter ist vergiftet")? = Some(m);
    Ok(Ladung {
        name: anzeigename(&e.modell.artefakt),
        pfad: e.modell.artefakt.clone(),
        sekunden: (dauer * 10.0).round() / 10.0,
    })
}

/// Gibt das geladene Modell wieder frei.
///
/// # ⚑ Warum ein Modell ueberhaupt wieder gehen soll
///
/// **Ein 4B-Artefakt sind viereinhalb Gigabyte, und sie liegen im
/// Speicher, solange das Fenster offen ist.** Wer morgens eine Frage
/// stellt und das Fenster stehenlaesst, gibt den Rest des Tages
/// Arbeitsspeicher her, ohne etwas davon zu haben. Das steht in
/// derselben Reihe wie die Kapazitaetsfreigabe: **Was Myelith nimmt,
/// soll es auch wieder hergeben.**
///
/// ⚑ **Die Rueckgabe ist ein Wert, der stirbt**, und nicht ein Aufruf,
/// der etwas leert: `None` in den Halter zu setzen laesst das Modell
/// fallen, und damit gehen die Gewichte.
///
/// ⛑ **Ohne geladenes Modell ist das kein Fehler.** Ein Entladen, das
/// sich beschwert, wenn nichts da ist, zwingt jeden Aufrufer, vorher zu
/// fragen; die Zeitschaltung tut das nicht und soll es nicht muessen.
#[tauri::command]
fn modell_entladen(halter: tauri::State<'_, Halter>) -> Result<bool, String> {
    let mut g = halter.modell.lock().map_err(|_| "der Modellhalter ist vergiftet")?;
    Ok(g.take().is_some())
}

/// Faehrt einen Auftrag und meldet den Verlauf ans Fenster.
///
/// ⛑ **`async` allein genuegt nicht.** Der Lauf **rechnet**, er wartet
/// nicht; in einem `async`-Befehl belegte er den Laufzeitfaden fuer
/// Minuten und damit alles andere. `spawn_blocking` gibt ihm einen
/// eigenen.
#[tauri::command]
async fn agent_fahren(
    auftrag: String,
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<Abschluss, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let ruestung = myl_client::ruestung::ruesten(&e.agent, myl_client::Ansageform::Amtlich, myl_client::werkzeuge::Werkzeugsatz::default(), Vec::new())?;
    let gesperrt = !e.agent.auch_bezeugtes;
    let schritte = e.agent.schritte as usize;
    let grenze = e.modell.token as u32;

    let _ = fenster.emit("agent-beginnt", &auftrag);
    let halt = halter.modell.clone();
    let aus = tauri::async_runtime::spawn_blocking(move || {
        let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_mut() else {
            return Err("das Modell ist nicht geladen".to_string());
        };
        zusehen(m, &fenster);
        // ⚑ **Zwei Quellen, ein Kanal.** Der laufende Text kommt vom
        // Modell, die Werkzeuge von der Schleife; das Fenster soll
        // beides in einer Reihenfolge sehen und nicht aus zwei
        // Stroemen zusammensetzen muessen.
        let f = fenster.clone();
        let melder = move |m: myl_client::Meldung<'_>| {
            let _ = f.emit(
                LEBEND,
                match m {
                    myl_client::Meldung::Schritt(nummer) => Lebend::Schritt { nummer },
                    myl_client::Meldung::Aufruf { name, argumente } => Lebend::Aufruf {
                        name: name.to_string(),
                        argumente: myl_client::lauf::kurzform(argumente),
                    },
                    myl_client::Meldung::Ergebnis { name, text } => Lebend::Ergebnis {
                        name: name.to_string(),
                        text: myl_client::lauf::eine_zeile(text, 200),
                    },
                    myl_client::Meldung::Abgelehnt { name, grund } => Lebend::Abgelehnt {
                        name: name.to_string(),
                        grund: grund.to_string(),
                    },
                },
            );
        };
        let ergebnis = myl_client::lauf::fahren_beobachtet(
            m, &ruestung, schritte, !gesperrt, grenze, &auftrag, Some(&melder),
        );
        // ⛑ Siehe `zusehen`: Der Beobachter geht wieder ab, sonst
        // meldete dieser Lauf in den naechsten hinein.
        m.beobachter = None;
        Ok(ergebnis)
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;
    Ok(Abschluss {
        verlauf: aus.verlauf.iter().map(zeile_aus).collect(),
        antwort: aus.antwort.clone(),
        fertig: aus.fertig(),
        sekunden: (aus.sekunden * 10.0).round() / 10.0,
        gesperrt,
    })
}

/// Eine Frage ohne Werkzeuge, mit dem bisherigen Gespraech davor.
///
/// # ⚑ Warum es diesen Befehl neben `agent_fahren` gibt
///
/// Sie sind nicht dasselbe, und der Unterschied gehoert dem Nutzer
/// sichtbar gemacht statt versteckt:
///
/// | | `frage` | `agent_fahren` |
/// |---|---|---|
/// | Werkzeuge | keine | die des Klienten, in der Einhaengung |
/// | Gespraech | **traegt den Verlauf mit** | jeder Auftrag steht fuer sich |
/// | Belege | keine | Schrittbudget und Belegkette je Lauf |
///
/// ⚑ **Der Verlauf wird mitgegeben, weil das Modell ihn tragen kann.**
/// `chat` nimmt eine Nachrichtenliste; ein Fenster, das nur die letzte
/// Frage schickt, waere ein Chatfenster ohne Gespraech.
///
/// ⛑ **Beim Agenten geht das ausdruecklich nicht**, und das ist
/// Entscheidung C2: Fortgesetzt wird das **Modell**, nicht das
/// Gespraech, denn Schrittbudget und Belegkette gelten je Lauf. Ein
/// Fenster, das beide gleich behandelte, versteckte genau den
/// Unterschied, um dessentwillen es die Betriebsarten gibt.
#[tauri::command]
async fn frage(
    verlauf: Vec<(String, String)>,
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<Antwort, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let grenze = e.modell.token as u32;
    if verlauf.is_empty() {
        return Err("es wurde nichts gefragt".to_string());
    }

    let halt = halter.modell.clone();
    let anfang = std::time::Instant::now();
    let text = tauri::async_runtime::spawn_blocking(move || {
        let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_mut() else {
            return Err("das Modell ist nicht geladen".to_string());
        };
        // ⚑ Die Rollen kommen aus dem Fenster und werden hier auf die
        // beiden abgebildet, die es gibt. Eine unbekannte Rolle wird
        // zur Nutzerrolle und nicht stillschweigend verworfen: Ein
        // verschluckter Beitrag waere ein Gespraech mit einer Luecke.
        let n: Vec<myl_client::Nachricht> = verlauf
            .iter()
            .map(|(rolle, inhalt)| {
                if rolle == "modell" {
                    myl_client::Nachricht::modell(inhalt.clone())
                } else {
                    myl_client::Nachricht::nutzer(inhalt.clone())
                }
            })
            .collect();
        // `chat` kommt aus dem Merkmal `Modellweg`.
        use myl_client::Modellweg as _;
        zusehen(m, &fenster);
        let antwort = m.chat("lokal", &n, Some(grenze)).map_err(|f| f.to_string()).map(|a| a.text);
        m.beobachter = None;
        antwort
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;

    Ok(Antwort { text, sekunden: (anfang.elapsed().as_secs_f64() * 10.0).round() / 10.0 })
}

/// Was waehrend eines Laufs beim Fenster ankommt.
///
/// # ⚑ Ein Ereignis und kein Rueckgabewert
///
/// **Ein Rueckgabewert kommt am Ende.** Bei einem 4B-Modell sind das
/// bis zu einer Minute, und in dieser Zeit stuende ein Fenster still.
/// Ein stehendes Fenster sieht aus wie ein abgestuerztes, und das ist
/// derselbe Grund, aus dem der Ladeknopf sich sperrt und es sagt.
///
/// ⚑ **Die Rueckgabe bleibt trotzdem.** Sie ist die vollstaendige,
/// geprueete Fassung; was live ankam, ist die Vorschau darauf. Wer nur
/// noch Ereignisse schickte, haette keinen Stand mehr, gegen den er
/// pruefen kann, und `der_laufende_text_ist_die_antwort` haelt genau
/// diese beiden gegeneinander.
#[derive(Clone, Serialize)]
#[serde(tag = "art")]
enum Lebend {
    /// Ein Stueck Ueberlegung.
    Denken { text: String },
    /// Ein Stueck Antworttext.
    Text { text: String },
    /// Das Modell wird zum `nummer`-ten Mal gefragt.
    Schritt { nummer: u32 },
    /// Ein Werkzeug laeuft jetzt.
    Aufruf { name: String, argumente: String },
    /// Und was es zurueckgab.
    Ergebnis { name: String, text: String },
    /// Ein Vorschlag wurde abgewiesen, mit dem Grund.
    Abgelehnt { name: String, grund: String },
}

/// Der Ereignisname, unter dem alles Lebende laeuft.
///
/// ⚑ **Einer und nicht sechs.** Sechs Namen hiessen sechs Anmeldungen
/// im Fenster, und wer einen vergisst, verliert eine Art Meldung, ohne
/// dass etwas fehlschlaegt.
const LEBEND: &str = "lauf-lebt";

/// Haengt einen Beobachter an das Modell, der ans Fenster meldet.
///
/// ⚑ **Er wird am Ende wieder abgenommen.** Das Modell lebt im
/// Fensterzustand und ueberlebt den Lauf; ein Beobachter, der
/// dableibt, hielte einen Fenstergriff aus einem beendeten Auftrag
/// fest und meldete in den naechsten hinein.
fn zusehen(m: &mut myl_client::Oertlichesmodell, fenster: &tauri::AppHandle) {
    let f = fenster.clone();
    m.beobachter = Some(Box::new(move |s: myl_client::strom::Stueck| {
        let _ = f.emit(
            LEBEND,
            match s {
                myl_client::strom::Stueck::Denken(text) => Lebend::Denken { text },
                myl_client::strom::Stueck::Text(text) => Lebend::Text { text },
            },
        );
    }));
}

/// Was eine Frage zurueckbringt.
#[derive(Serialize)]
struct Antwort {
    text: String,
    sekunden: f64,
}

/// Ein Schritt, wie ihn das Fenster braucht.
#[derive(Serialize)]
struct Zeile {
    art: &'static str,
    text: String,
}

/// Was ein Lauf zurueckbringt.
#[derive(Serialize)]
struct Abschluss {
    verlauf: Vec<Zeile>,
    antwort: Option<String>,
    fertig: bool,
    sekunden: f64,
    /// ⚑ Ob die Betriebsart die lokalen Werkzeuge gesperrt hat. Ohne
    /// diese Angabe sieht der Nutzer einen Agenten, der seine Werkzeuge
    /// nicht benutzt, und haelt es fuer ein Modellproblem.
    gesperrt: bool,
}

fn zeile_aus(s: &myl_client::lauf::Schritt) -> Zeile {
    use myl_client::lauf::Schritt as S;
    match s {
        S::Plan(t) => Zeile { art: "plan", text: t.clone() },
        S::Denken(t) => Zeile { art: "denken", text: t.clone() },
        S::Aufruf { name, argumente } => {
            Zeile { art: "aufruf", text: format!("{name} {argumente}") }
        }
        S::Unlesbar(t) => Zeile { art: "unlesbar", text: t.clone() },
        S::Ergebnis(t) => Zeile { art: "ergebnis", text: t.clone() },
        S::Antwort(t) => Zeile { art: "antwort", text: t.clone() },
    }
}

/// Laesst den Nutzer ein Verzeichnis auswaehlen.
///
/// # ⚑ Warum das aus dem Ruecken gerufen wird und nicht aus dem Fenster
///
/// Das Zusatzstueck `tauri-plugin-dialog` bringt eine Befehlsgruppe
/// fuer die Webansicht mit, `plugin:dialog|open`. Sie zu benutzen
/// hiesse: eine Berechtigung in `capabilities/vorgabe.json`, ein
/// JS-Paket fuer die Bindung oder ein von Hand nachgebauter Aufruf mit
/// einer Nutzlast, deren Form sich mit dem Zusatzstueck aendern kann.
///
/// ⚑ **Aus Rust gerufen entfaellt alles drei.** Berechtigungen regeln,
/// was die **Webansicht** rufen darf; eigene Befehle brauchen keine.
/// Das Fenster ruft `ordner_waehlen` wie jeden anderen Befehl auch,
/// und die Erlaubnisliste bleibt bei ihrem einen Eintrag.
///
/// ⛑ **`async`, und das ist keine Kosmetik.** Tauri fuehrt Befehle
/// ohne `async` **auf dem Hauptfaden** aus, und `blocking_pick_folder`
/// wartet dort auf eine Antwort, die nur der Hauptfaden geben kann:
/// Das Fenster stuende. Mit `async` laeuft der Befehl auf dem
/// Nebenlaeufer, und das Warten ist harmlos.
///
/// ⚑ **Auf macOS ist die Auswahl zugleich die Freigabe.** Was der
/// Nutzer im Fensterdialog waehlt, darf die Anwendung danach lesen und
/// schreiben, auch unterhalb von Schreibtisch oder Dokumenten. Wer den
/// Pfad von Hand eintippt, bekommt genau dort `ENOENT`.
#[tauri::command]
async fn ordner_waehlen(
    app: tauri::AppHandle,
    titel: String,
    start: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let mut w = app.dialog().file().set_title(titel);
    // ⚑ Der bisherige Wert als Startort, aber nur wenn es ihn gibt:
    // Ein Dialog, der in einem nicht vorhandenen Verzeichnis aufmacht,
    // landet je nach System irgendwo oder gar nicht.
    if let Some(v) = start.filter(|v| !v.is_empty()) {
        let pfad = std::path::PathBuf::from(&v);
        if pfad.is_dir() {
            w = w.set_directory(pfad);
        } else if let Some(eltern) = pfad.parent().filter(|e| e.is_dir()) {
            w = w.set_directory(eltern);
        }
    }

    // `None` heisst: abgebrochen. Das ist kein Fehler, und das Fenster
    // laesst den bisherigen Wert dann stehen.
    let Some(gewaehlt) = w.blocking_pick_folder() else {
        return Ok(None);
    };
    // ⛑ `simplified` nimmt unter Windows das `\\?\`-Praefix weg. Ohne
    // das stuende im Feld ein Pfad, den zwar jede Rust-Funktion
    // versteht, aber kein Mensch wiedererkennt.
    let pfad = gewaehlt.simplified().into_path().map_err(|f| f.to_string())?;
    Ok(Some(pfad.display().to_string()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Halter::default())
        .invoke_handler(tauri::generate_handler![
            einstellungen,
            felder,
            hardware,
            markdown,
            artefakt_loeschen,
            modell_entladen,
            setzen,
            modell_laden,
            agent_fahren,
            frage,
            werkzeuge,
            modelle,
            katalog,
            voraussetzungen,
            artefakt_bauen,
            gespraech_ausgeben,
            ordner_waehlen,
            aktualisierung,
            aktualisieren
        ])
        .run(tauri::generate_context!())
        .expect("die Oberflaeche liess sich nicht starten");
}

// ── Modelle holen und Artefakte bauen ───────────────────────────────
//
// # ⛑ Die groesste Luecke zwischen „frischer Klon" und „laeuft"
//
// Bis zum 2026-09-09 war `modell.artefakt` ein Textfeld: ein Pfad, den
// jemand von Hand eintraegt und der existieren muss. Wer die
// Freigabe-Binaries laedt, hat kein Artefakt und keinen Weg zu einem,
// ohne die Kommandozeile. Der dokumentierte Weg sind drei Schritte,
// ein Python-Venv mit torch, ein Modelldownload und ein
// Kalibrierlauf.
//
// # ⚠️ Was das hier NICHT kann, und das steht vor dem Knopf
//
// Es ruft die Skripte des Repositoriums. Aus einem Freigabe-Binary
// heraus gibt es die nicht: Dort ist der Knopf gesperrt, und die
// Oberflaeche sagt das, statt es zu versuchen und an einem fehlenden
// Pfad zu scheitern. `voraussetzungen` prueft das **vorher**.

/// Was ein Modell im Katalog ueber sich sagt.
#[derive(Serialize)]
struct Katalogeintrag {
    schluessel: String,
    anzeigename: String,
    hf_repo: String,
    /// ⚠️ **Die Lizenz der Grundgewichte, nicht die des Artefakts.**
    /// Die Einstellungsseite zeigte bis zum 2026-09-10 ein Feld
    /// `lizenz` neben dem Namen „Myelith 4B" an, und das las sich, als
    /// stuende das Artefakt unter Apache-2.0. **Es steht unter der
    /// Lizenz dieses Repositoriums**; Apache-2.0 gilt fuer die
    /// heruntergeladenen Gewichte, aus denen es gebaut wurde.
    lizenz_gewichte: String,
    /// Die Lizenz des gebauten Artefakts: die dieses Repositoriums.
    lizenz_artefakt: String,
    parameter: String,
    /// ⚠️ **Das Grundmodell, aus dem das Artefakt gebaut ist.** Der
    /// Anzeigename heisst „Myelith <Groesse>", weil das Artefakt selbst
    /// gebaut ist und anders rechnet als das Gleitkommamodell. Die
    /// Herkunft darf dabei nicht verschwinden: Die Grundmodelle stehen
    /// unter Apache-2.0, und ein Name ohne Herkunft waere eine
    /// Verschleierung statt einer Unterscheidung.
    grundmodell: String,
    gewichte: String,
    artefakt: String,
    status: String,
    /// ⚑ **Was der Eintrag an Platz braucht, in Bytes.** Der Katalog
    /// nennt beide Zahlen seit jeher; sie standen nur nie im Fenster.
    /// Damit ist die Plattenfreigabe **vor** einem Download exakt
    /// pruefbar und nicht geschaetzt.
    braucht_bytes: u64,
    /// Liegt das Rohmodell schon da?
    modell_da: bool,
    /// Liegt das fertige Artefakt schon da?
    artefakt_da: bool,
}

/// Wo das Repositorium liegt, von der Oberflaeche aus gesehen.
///
/// ⛑ **Gesucht und nicht angenommen.** Die Oberflaeche laeuft im
/// Entwicklungsbaum aus `CLIENT/myl-oberflaeche`, als gebuendelte App
/// aus einem ganz anderen Verzeichnis. Wer den Pfad festnagelt, baut
/// einen Knopf, der auf genau einer Maschine geht.
/// Wo Myelith seine grossen Dateien haelt, also Modelle und Artefakte.
///
/// ⚑ **Die Reservierung gehoert auf denselben Datentraeger wie das,
/// was sie schuetzt.** Platz auf einer anderen Platte zu halten waere
/// eine Zusage ueber die falsche Platte, und sie fiele erst beim ersten
/// Download auf.
fn datenort() -> std::path::PathBuf {
    match wurzel_suchen() {
        Some(w) => w.join("INTEGER_LLM"),
        // Ohne Klon gibt es keine Modelle und keine Artefakte; dann ist
        // der Ort der Einstellungen der einzige, den es sicher gibt.
        None => myl_client::Einstellungen::vorgabepfad()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
    }
}

/// Was Myelith heute schon auf der Platte haelt.
fn belegung_heute() -> u64 {
    let ort = datenort();
    myl_client::reservierung::belegung(&ort.join("models"))
        + myl_client::reservierung::belegung(&ort.join("artifacts"))
}

/// Stellt die Reservierung auf die heutige Freigabe ein.
///
/// ⚑ **Belegt plus reserviert ist die Freigabe, durchgehend.** Ohne
/// diese Rechnung arbeitete die Reservierung gegen den eigenen
/// Download: Wer 50 GiB freigibt und 50 GiB haelt, hat fuer das erste
/// Modell keinen Platz mehr, obwohl er ihn gerade dafuer freigegeben
/// hat.
fn platte_nachfuehren(halter: &Halter) -> Result<u64, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let mut gehalten = halter.platte.lock().map_err(|_| "Die Reservierung ist blockiert")?;
    match e.kapazitaet.platte_gib {
        // Ohne Freigabe wird nichts gehalten, und die Datei verschwindet.
        None => {
            *gehalten = None;
            Ok(0)
        }
        Some(gib) => {
            let soll = myl_client::reservierung::zu_halten(
                gib as u64 * myl_client::hardware::GIB,
                belegung_heute(),
            );
            match gehalten.as_mut() {
                Some(r) => r.setzen(soll)?,
                None => {
                    *gehalten =
                        Some(myl_client::reservierung::Reservierung::anlegen(&datenort(), soll)?)
                }
            }
            Ok(soll)
        }
    }
}

/// ⛑ **Die Suche steht seit dem 2026-09-10 in `myl-client`.**
///
/// Sie stand hier, und das Bedieninstrument hatte sie nicht: `myl` gab
/// aus einem fremden Arbeitsverzeichnis „es fehlt das
/// Artefaktverzeichnis", obwohl das Artefakt dalag. **Zwei Programme
/// desselben Klienten beantworteten dieselbe Frage verschieden**, und
/// eines davon gar nicht.
///
/// ⚑ **Und sie kann jetzt mehr, als sie hier konnte:** Ein gemerkter
/// Ort traegt ueber den Fall hinweg, dass das Programm **ausserhalb**
/// des Baums liegt, also genau ueber den installierten Zustand.
fn wurzel_suchen() -> Option<std::path::PathBuf> {
    myl_client::ort::wurzel()
}

/// Die Marke, an der das Fenster „es fehlt der Ordner" erkennt.
///
/// ⚑ Eine Zeichenkette und kein eigener Fehlertyp: Der Weg zwischen
/// Ruecken und Fenster traegt ohnehin nur Text, und ein Typ, der dabei
/// zu Text wird, ist einer, den beide Seiten trotzdem vergleichen
/// muessen.
const KEIN_ORDNER: &str = "kein-ausgabeordner";

/// Schreibt ein Gespraech als Markdown in den eingestellten Ordner.
///
/// # ⚑ Warum hier und nicht ueber einen Speichern-Dialog
///
/// ⛑ **Diese Begruendung hiess einmal „ein Dialog braucht ein
/// weiteres Zusatzstueck".** Seit dem Ordnerauswaehler gibt es das
/// Zusatzstueck, und die Begruendung traegt trotzdem, nur aus einem
/// anderen Grund: Ein Speichern-Dialog ist **bei jedem Ausgeben** ein
/// Dialog. Ein eingestellter Ordner ist **eine** Entscheidung, danach
/// ist Ausgeben ein Klick. Gewaehlt wird der Ordner einmal, in den
/// Einstellungen, und dort mit demselben Fensterdialog.
///
/// Der Pfad kommt zurueck und steht in der Meldung, also weiss jeder,
/// wo es liegt.
///
/// ⛑ **Der Name wird entschaerft und nicht uebernommen.** Ein Titel
/// kommt aus dem ersten Satz eines Gespraechs und kann alles
/// enthalten, `/` und `..` eingeschlossen; ungeprueft uebernommen
/// schriebe das Fenster irgendwohin. Erlaubt sind Buchstaben, Ziffern,
/// Strich und Unterstrich, alles andere wird zu einem Strich.
#[tauri::command]
fn gespraech_ausgeben(titel: String, inhalt: String) -> Result<String, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    // ⛑ **Ohne eingestellten Ordner wird nichts geschrieben**, und der
    // Fehler traegt eine Marke, an der das Fenster ihn erkennt: Es
    // oeffnet dann die Einstellungen an genau diesem Feld, statt eine
    // Meldung zu zeigen, die niemand in eine Handlung uebersetzen kann.
    //
    // ⚑ Ein Voreinstellungsordner waere die bequeme Wahl und die
    // falsche: Ein Ort, den niemand gewaehlt hat, ist einer, an dem
    // niemand sucht.
    let Some(ordner) = e.ausgabe.ordner.filter(|o| !o.trim().is_empty()) else {
        return Err(KEIN_ORDNER.into());
    };
    let ordner = std::path::PathBuf::from(ordner);
    std::fs::create_dir_all(&ordner).map_err(|f| {
        mit_zugriffshinweis(format!("{}: {f}", ordner.display()), &ordner.display().to_string())
    })?;

    let sauber: String = titel
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let sauber = sauber.trim_matches('-');
    let stamm = if sauber.is_empty() { "gespraech" } else { sauber };
    // ⚑ Ein Zeitstempel davor, damit zwei gleichnamige Gespraeche sich
    //   nicht ueberschreiben und die Liste nach Zeit sortiert steht.
    let wann = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ziel = ordner.join(format!("{wann}-{}.md", &stamm[..stamm.len().min(60)]));
    std::fs::write(&ziel, inhalt).map_err(|f| {
        mit_zugriffshinweis(format!("{}: {f}", ziel.display()), &ziel.display().to_string())
    })?;
    Ok(ziel.display().to_string())
}

/// Ergaenzt eine Fehlermeldung um den Hinweis, der wirklich hilft.
///
/// ⛑ **macOS meldet eine abgelehnte Ordnerfreigabe als „No such file or
/// directory".** Nicht als fehlende Berechtigung: Der Kernel gibt
/// `ENOENT` zurueck, damit ein Programm nicht einmal erfaehrt, dass es
/// den Ordner gibt. Wer die Meldung liest, sucht danach einen
/// Tippfehler im Pfad, und den gibt es nicht.
///
/// Betroffen sind `Schreibtisch`, `Dokumente`, `Downloads`, iCloud und
/// Wechselmedien. Dieses Repositorium liegt bei mindestens einem
/// Nutzer unter `Schreibtisch`, und dort ist der Fall eingetreten.
fn mit_zugriffshinweis(fehler: String, pfad: &str) -> String {
    if !fehler.contains("os error 2") && !fehler.contains("No such file") {
        return fehler;
    }
    let p = std::path::Path::new(pfad);
    // ⚑ Liegt der Pfad in einem geschuetzten Ordner? Verglichen wird
    //   gegen die englischen Namen, denn so heissen sie im Dateisystem,
    //   auch wenn der Finder sie uebersetzt anzeigt.
    let geschuetzt = ["Desktop", "Documents", "Downloads"];
    let betroffen = p
        .components()
        .any(|c| geschuetzt.contains(&c.as_os_str().to_string_lossy().as_ref()));
    if !betroffen {
        return fehler;
    }
    format!(
        "{fehler}\n\nDer Pfad liegt in einem Ordner, den macOS schuetzt.          Wurde die Nachfrage nach dem Zugriff abgelehnt, meldet das System          die Datei als nicht vorhanden, obwohl sie da ist.\n\
         Zu erlauben unter: Systemeinstellungen, Datenschutz und Sicherheit,          Dateien und Ordner, Myelith.\n\
         Oder das Artefakt ausserhalb von Schreibtisch, Dokumente und          Downloads ablegen."
    )
}

/// Macht einen relativen Artefaktpfad gegen die Wurzel absolut.
///
/// ⛑ **Ohne das scheitert „Modell laden" aus dem Finder heraus**, und
/// zwar mit `No such file or directory`: In den Einstellungen steht
/// `INTEGER_LLM/artifacts/myelith-4b`, und das ist relativ zu einem
/// Arbeitsverzeichnis, das dort `/` ist. Ein absoluter Pfad in den
/// Einstellungen bliebe unberuehrt.
fn artefakt_absolut(pfad: &str) -> String {
    myl_client::ort::absolut(pfad)
}

/// Was fehlt, bevor gebaut werden kann.
#[derive(Serialize)]
struct Voraussetzungen {
    /// Leer heisst: es kann losgehen.
    fehlt: Vec<String>,
    wurzel: Option<String>,
}

#[tauri::command]
fn voraussetzungen() -> Voraussetzungen {
    let Some(w) = wurzel_suchen() else {
        return Voraussetzungen {
            fehlt: vec![
                "Die Skripte des Repositoriums sind nicht zu finden. Ein Artefakt laesst \
                 sich nur aus einem Klon bauen, nicht aus einem Freigabe-Binary."
                    .into(),
            ],
            wurzel: None,
        };
    };
    let mut fehlt = Vec::new();
    // ⚑ Der `hf`-Befehl kommt aus `huggingface_hub`; ohne ihn laedt
    // `fetch_model.sh` nichts. Gesucht wird er in der
    // Kalibrier-Umgebung **und** im System, denn dort sucht ihn der
    // Lauf auch.
    if !befehl_da(&w, "hf") {
        fehlt.push(
            "Der Befehl `hf` fehlt (huggingface_hub). \
             `calibrate/.venv/bin/pip install -r calibrate/requirements.txt`"
                .into(),
        );
    }
    let venv = w.join("INTEGER_LLM/calibrate/.venv/bin/python3");
    if !venv.is_file() {
        fehlt.push(format!(
            "Die Kalibrier-Umgebung fehlt: {}. \
             `python3 -m venv calibrate/.venv` und dann die Anforderungen installieren.",
            venv.display()
        ));
    }
    Voraussetzungen { fehlt, wurzel: Some(w.display().to_string()) }
}

/// Sucht einen Befehl im `PATH`.
fn which(name: &str) -> Option<std::path::PathBuf> {
    let pfad = std::env::var_os("PATH")?;
    std::env::split_paths(&pfad).map(|d| d.join(name)).find(|p| p.is_file())
}

/// Das `bin` der Kalibrier-Umgebung.
fn venv_bin(w: &std::path::Path) -> std::path::PathBuf {
    w.join("INTEGER_LLM/calibrate/.venv/bin")
}

/// Der `PATH`, unter dem die Bauskripte laufen: die Kalibrier-Umgebung
/// **vor** dem System.
///
/// ⛑ **Bis zum 2026-09-09 bekam nur der Kalibrierschritt ihn.** Der
/// Download davor lief mit dem blossen System-`PATH`, und
/// `fetch_model.sh` bricht ohne `hf` ab. Der Befehl liegt aber genau
/// hier und nicht im System: `huggingface_hub` wird in die Umgebung
/// installiert. Auf einem **richtig** eingerichteten Klon waere der
/// Knopf „Download" damit fehlgeschlagen, mit der Meldung, man solle
/// installieren, was schon da ist.
fn pfad_mit_venv(w: &std::path::Path) -> String {
    format!(
        "{}:{}",
        venv_bin(w).display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// Liegt der Befehl in der Kalibrier-Umgebung oder im System?
///
/// ⚑ **Gesucht wird dort, wo der Lauf ihn suchen wird**, und nicht nur
/// im `PATH` dieses Fensters. Sonst meldet die Oberflaeche einen
/// Mangel, den es nicht gibt, und verschweigt einen, den es gibt.
fn befehl_da(w: &std::path::Path, name: &str) -> bool {
    venv_bin(w).join(name).is_file() || which(name).is_some()
}

#[tauri::command]
fn katalog() -> Result<Vec<Katalogeintrag>, String> {
    let w = wurzel_suchen().ok_or("Das Repositorium ist nicht zu finden")?;
    let roh = std::fs::read_to_string(w.join("INTEGER_LLM/models/KATALOG.json"))
        .map_err(|e| format!("KATALOG.json: {e}"))?;
    let d: serde_json::Value = serde_json::from_str(&roh).map_err(|e| e.to_string())?;
    let obj = d.as_object().ok_or("KATALOG.json ist kein Objekt")?;

    let text = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
    };
    // ⚑ Eine fehlende Groesse ist null und nicht geraten: Der Platz
    // wird dann nicht geprueft, statt gegen eine erfundene Zahl.
    let zahl = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let mut aus = Vec::new();
    for (schluessel, v) in obj {
        // Die Schluessel mit Unterstrich sind Hinweise und keine Modelle.
        if schluessel.starts_with('_') {
            continue;
        }
        aus.push(Katalogeintrag {
            schluessel: schluessel.clone(),
            anzeigename: text(v, "anzeigename"),
            hf_repo: text(v, "hf_repo"),
            lizenz_gewichte: text(v, "lizenz_gewichte"),
            lizenz_artefakt: text(v, "lizenz_artefakt"),
            parameter: text(v, "parameter"),
            grundmodell: text(v, "grundmodell"),
            gewichte: text(v, "gewichte_anzeige"),
            artefakt: text(v, "artefakt_anzeige"),
            status: text(v, "status"),
            braucht_bytes: zahl(v, "gewichte_bytes") + zahl(v, "artefakt_bytes"),
            modell_da: w.join("INTEGER_LLM/models").join(text(v, "hf_verzeichnis")).is_dir(),
            artefakt_da: w
                .join("INTEGER_LLM/artifacts")
                .join(schluessel)
                .join("model_config.json")
                .is_file(),
        });
    }
    aus.sort_by(|a, b| a.schluessel.cmp(&b.schluessel));
    Ok(aus)
}

/// Eine Zeile aus dem Baulauf, wie sie im Fenster ankommt.
#[derive(Clone, Serialize)]
struct Bauzeile {
    /// `holen`, `kalibrieren`, `fertig` oder `fehler`.
    phase: &'static str,
    text: String,
}

/// Holt ein Modell und baut sein Artefakt.
///
/// # ⚑ Warum die Skripte gerufen werden und nicht nachgebaut
///
/// `fetch_model.sh` nagelt die Revision fest, `build_artifacts.sh`
/// ruft `calibrate.src.main`. Beides hier nachzubauen hiesse, den
/// dokumentierten Weg ein zweites Mal zu schreiben, und der zweite ist
/// immer der schlechter geprueffte. Dieselbe Begruendung wie dafuer,
/// dass der Ruecken `myl-client` ruft und nicht `myl`.
///
/// # ⚠️ Ein Fortschrittsbalken, der nicht luegt
///
/// Die Skripte melden keinen Prozentsatz. Was gemeldet wird, sind
/// **Phasen**: holen, kalibrieren, fertig. Der Balken zeigt deshalb
/// die Phase und die letzte Zeile, und keine erfundene Zahl. Ein
/// Balken, der bei siebzig Prozent stehenbleibt, weil jemand geraten
/// hat, ist schlimmer als einer, der sagt „kalibriert seit vier
/// Minuten".
#[tauri::command]
async fn artefakt_bauen(
    halter: tauri::State<'_, Halter>,
    schluessel: String,
    fenster: tauri::AppHandle,
) -> Result<String, String> {
    let v = voraussetzungen();
    if !v.fehlt.is_empty() {
        return Err(v.fehlt.join("\n"));
    }
    let w = std::path::PathBuf::from(v.wurzel.ok_or("keine Wurzel")?);

    // Aus dem Katalog: HF-Kennung und Revision.
    let roh = std::fs::read_to_string(w.join("INTEGER_LLM/models/KATALOG.json"))
        .map_err(|e| e.to_string())?;
    let d: serde_json::Value = serde_json::from_str(&roh).map_err(|e| e.to_string())?;
    let eintrag = d.get(&schluessel).ok_or(format!("{schluessel} steht nicht im Katalog"))?;
    let repo = eintrag.get("hf_repo").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let revision =
        eintrag.get("hf_revision").and_then(|x| x.as_str()).unwrap_or("main").to_string();
    let verzeichnis =
        eintrag.get("hf_verzeichnis").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if repo.is_empty() {
        return Err(format!("{schluessel} nennt kein `hf_repo`"));
    }

    // ⚑ **Passt es in die Freigabe?** Der Katalog nennt beide Groessen
    // in Bytes, die Frage ist also gerechnet und nicht geschaetzt. Ein
    // Download, der die Freigabe sprengt, wird **vorher** abgelehnt und
    // nicht nach vierzig Minuten mit einer vollen Platte.
    let braucht = eintrag.get("gewichte_bytes").and_then(|x| x.as_u64()).unwrap_or(0)
        + eintrag.get("artefakt_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if let (Some(gib), true) = (e.kapazitaet.platte_gib, braucht > 0) {
        let freigabe = gib as u64 * myl_client::hardware::GIB;
        let belegt = belegung_heute();
        if belegt + braucht > freigabe {
            let g = |b: u64| b as f64 / myl_client::hardware::GIB as f64;
            return Err(format!(
                "Das passt nicht in die Freigabe: {:.1} GiB werden gebraucht, \
                 {:.1} GiB sind belegt, freigegeben sind {gib} GiB. \
                 Erhöhe die Freigabe oder gib Platz frei.",
                g(braucht),
                g(belegt)
            ));
        }
        // ⚑ **Der Platz geht von der Reservierung an den Download
        // ueber.** Die Summe aus belegt und reserviert bleibt die
        // Freigabe; ohne diesen Schritt hielte die Reservierung genau
        // den Platz fest, den der Download gleich braucht.
        if let Ok(mut g) = halter.platte.lock() {
            if let Some(r) = g.as_mut() {
                r.hergeben(braucht)?;
            }
        }
    }

    let f = fenster.clone();
    let sagen = move |phase: &'static str, text: String| {
        let _ = f.emit("bau-zeile", Bauzeile { phase, text });
    };

    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let modellordner = w.join("INTEGER_LLM/models").join(&verzeichnis);
        // ⚑ **Schon da heisst: nicht noch einmal.** Ein zweiter
        // Download von mehreren Gigabyte, weil jemand den Knopf
        // zweimal gedrueckt hat, waere teuer und ueberfluessig.
        if modellordner.is_dir() {
            sagen("holen", format!("{verzeichnis} liegt schon da, kein Download"));
        } else {
            sagen("holen", format!("lade {repo}@{revision} ..."));
            lauf_mit_ausgabe(
                std::process::Command::new("bash")
                    .arg(w.join("INTEGER_LLM/scripts/fetch_model.sh"))
                    .current_dir(w.join("INTEGER_LLM"))
                    .env("MODEL_ID", &repo)
                    .env("REVISION", &revision)
                    // ⛑ Ohne diese Zeile bricht `fetch_model.sh` mit
                    // „hf-CLI nicht gefunden" ab, obwohl der Befehl in
                    // der Kalibrier-Umgebung liegt.
                    .env("PATH", pfad_mit_venv(&w)),
                "holen",
                &sagen,
            )?;
        }

        sagen("kalibrieren", format!("kalibriere {schluessel}, das dauert ..."));
        lauf_mit_ausgabe(
            std::process::Command::new("bash")
                .arg(w.join("INTEGER_LLM/scripts/build_artifacts.sh"))
                .current_dir(w.join("INTEGER_LLM"))
                .env("INTEGER_LLM_MODEL", &schluessel)
                // ⚑ Das Skript ruft `python3`; ohne die Umgebung im Pfad
                // griffe es das System-Python und faende `torch` nicht.
                .env("PATH", pfad_mit_venv(&w)),
            "kalibrieren",
            &sagen,
        )?;

        let ziel = w.join("INTEGER_LLM/artifacts").join(&schluessel);
        if !ziel.join("model_config.json").is_file() {
            return Err(format!(
                "Der Lauf ist durch, aber {} fehlt. Die Ausgabe oben sagt, woran es lag.",
                ziel.join("model_config.json").display()
            ));
        }
        sagen("fertig", ziel.display().to_string());
        Ok(ziel.display().to_string())
    })
    .await
    .map_err(|e| format!("der Baufaden ist abgestuerzt: {e}"))?
}

/// Faehrt einen Befehl und reicht jede Zeile durch.
///
/// ⛑ **Zeilenweise und nicht am Ende.** Ein Kalibrierlauf dauert
/// Minuten bis Stunden; wer die Ausgabe erst danach zeigt, hat ein
/// Fenster, das aussieht wie eingefroren. Genau dafuer gibt es die
/// Ereignisse.
fn lauf_mit_ausgabe(
    befehl: &mut std::process::Command,
    phase: &'static str,
    sagen: &impl Fn(&'static str, String),
) -> Result<(), String> {
    use std::io::BufRead;
    let mut kind = befehl
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("{phase}: {e}"))?;

    if let Some(aus) = kind.stdout.take() {
        for zeile in std::io::BufReader::new(aus).lines().map_while(Result::ok) {
            sagen(phase, zeile);
        }
    }
    let stand = kind.wait().map_err(|e| e.to_string())?;
    if !stand.success() {
        // ⚑ Die Fehlerausgabe erst hier: Sie ist kurz, und sie waere
        // zwischen den Fortschrittszeilen untergegangen.
        let mut fehler = String::new();
        if let Some(mut e) = kind.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut fehler);
        }
        let kurz: String = fehler.lines().rev().take(6).collect::<Vec<_>>().join("\n");
        return Err(format!("{phase} ist fehlgeschlagen ({stand}):\n{kurz}"));
    }
    Ok(())
}
