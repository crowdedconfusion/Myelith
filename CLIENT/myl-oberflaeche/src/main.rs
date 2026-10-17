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
//! 📌 **Hier stand „sechsundfuenfzig Pruefungen", und es waren
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

// ⛔️ **Fund 474: Unter Windows oeffnete das Fenster eine Konsole mit.**
//    Ohne diese Zeile traegt die ausfuehrbare Datei das Subsystem
//    `console`, und Windows stellt jedem Start ein schwarzes Fenster
//    hinter das eigentliche. Auf macOS und Linux gibt es das nicht,
//    deshalb fiel es erst am Kreuzbau auf (`file` meldete `(console)`).
//
// ⚑ **Nur im Freigabebau.** Im Pruefbau bleibt die Konsole, denn dort
//    will man die Meldungen auf der Fehlerausgabe sehen. Ohne Konsole
//    laufen `eprintln!` ins Leere, und das ist gewollt: Die
//    Standardbibliothek behandelt einen fehlenden Griff als Senke und
//    bricht nicht ab.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::sync::Mutex;

mod modulwirt;

use myl_client::hardware::Freigabemaske as _;
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
    /// 📌 **Hier standen bis zum 2026-09-10 zwoelf einzelne Felder**,
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
    /// `None`, solange die erste Zaehlung noch laeuft ([`belegung_stand`]).
    platte_belegt: Option<u64>,
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
/// # 📌 Warum es diese Funktion gibt
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
/// Verzeichnis gibt es gar keine Werkzeuge; wer dann „liest Dateien"
/// laese, suchte den Fehler an der falschen Stelle.
///
/// 📌 **Bis zum 2026-09-11 waren es vier**, und der vierte hiess
/// „Werkzeuge gesperrt": ein eingehaengtes Verzeichnis, dessen
/// Werkzeuge trotzdem nicht liefen, weil ein zweiter Schalter fehlte.
/// Der Schalter ist entfallen.
///
/// Die technischen Namen bleiben, wo sie hingehoeren: im
/// Sitzungsstrom, in den Protokollen und in `Betriebsart::name`.
///
/// # 📌 Gefragt wird der **geltende** Ordner, nicht der gespeicherte
///
/// Bis zum 2026-09-15 stand hier `a.wurzel.is_some()`. Seit es eine
/// Vorgabe gibt (`WORK_DIR`), war das eine Falschauskunft: Bei leerem
/// Feld bekam der Agent seine Werkzeuge, und der Kopf meldete „ohne
/// Werkzeuge" samt der Aufforderung, `agent.wurzel` zu setzen.
///
/// ⚑ **Dieselbe Frage wie die Ruestung, also dieselbe Antwort.** Ein
/// Kopf, der etwas anderes sagt als das, was laeuft, ist schlimmer als
/// keiner.
fn kurzform(a: &myl_client::einstellungen::Agenteneinstellung) -> (&'static str, &'static str) {
    let hat_ordner = a
        .wurzel
        .clone()
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .is_some();
    // 📌 **Fund 484: Hier stand bis zum 2026-09-26, das Fenster habe fuer
    //    den manual mode „noch keinen Kasten"**, und der Agent bekomme
    //    deshalb keine schreibenden Werkzeuge. Seit dem 2026-09-15 legt
    //    `nachfrage_fuer` jede schreibende Handlung im Kasten des
    //    Betriebssystems vor. Die Marke im Kopf beschrieb elf Tage lang
    //    ein Fenster, das es nicht mehr gab: dieselbe Angabe an zwei Orten.
    if hat_ordner && a.modus.fragt_nach() {
        return (
            "fragt vor dem Schreiben (manual mode)",
            "Im manual mode darf der Agent lesen und suchen. Jede schreibende Handlung legt \
             das Fenster vorher in einem Kasten vor, mit Werkzeug und Argumenten, und sie laeuft \
             erst nach deiner Zustimmung.",
        );
    }
    match (hat_ordner, a.schreiben) {
        (false, _) => (
            "ohne Werkzeuge",
            "Es ist kein Verzeichnis eingehaengt, der Agent hat also keine Werkzeuge. \
             Auch der Ordner WORK_DIR ist nicht zu finden, der sonst die Vorgabe waere. \
             Setze `agent.wurzel` in den Einstellungen.",
        ),
        (true, false) => (
            "liest Dateien",
            "Der Agent darf im eingehaengten Verzeichnis lesen und suchen, aber nichts \
             aendern. Schreiben ist eine eigene Erlaubnis: `agent.schreiben`.",
        ),
        (true, true) => (
            "liest und schreibt",
            "Der Agent darf im eingehaengten Verzeichnis lesen, suchen, schreiben und \
             aendern. Ausserhalb kommt er nicht.",
        ),
    }
}

#[tauri::command]
async fn einstellungen(halter: tauri::State<'_, Halter>) -> Result<Ansicht, String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let e = myl_client::Einstellungen::lesen(&pfad)?;
    // ⚑ **Umgesetzt wird in der Kiste**, mit derselben Funktion, die
    // `myl` und `myelith` rufen (seit dem 2026-09-16). Hier stand die
    // Kernzeile allein, und ein Rechenwerk kam nie an.
    myl_client::hardware::anwenden(&e);
    // ⚑ **Die Reservierung wird bei jedem Blick nachgefuehrt.** Ein
    // Download hat vielleicht Platz verbraucht, und dann stimmt die
    // Summe aus belegt und reserviert nicht mehr.
    let platte_gehalten = platte_nachfuehren(&halter).unwrap_or(0);

    // ⚑ **Die Werte kommen aus der Kiste, Feld fuer Feld.** Kein Name
    // steht hier zweimal, und keiner fehlt: Die Liste ist dieselbe, aus
    // der die Seite ihre Zeilen zeichnet.
    let mut werte = std::collections::BTreeMap::new();
    for f in myl_client::einstellungen::FELDER {
        // ⚑ Auch die Werte der Konsolenfelder gehen mit: Sie kosten
        // nichts, und eine Karte, die weniger kennt als die Liste, ist
        // genau die Art Luecke, die Fund 280 war.
        werte.insert(f.name.to_string(), e.wert(f.name)?);
    }
    // Dazu die Freigaben je Rechenwerk, deren Namen erst der Scan kennt.
    // ⚑ Nur die Rechenwerke, und die gemerkt: Speicher und Platte braucht
    //   diese Ansicht nicht, und `vm_stat` kostete hier jedesmal 75 ms.
    for r in &myl_client::hardware::rechenwerke_gemerkt() {
        let name = format!("{}{}", myl_client::einstellungen::RECHENWERK_PRAEFIX, r.kennung);
        let w = e.wert(&name)?;
        werte.insert(name, w);
    }

    Ok(Ansicht {
        werte,
        sprache: e.oberflaeche.sprache.kennung().to_string(),
        kerne_wirksam: myl_client::kapazitaet::kerne(),
        platte_gehalten,
        platte_belegt: belegung_stand(),
        betriebsart: kurzform(&e.agent).0.into(),
        betriebsart_warum: kurzform(&e.agent).1.into(),
        pfad: pfad.display().to_string(),
    })
}

/// Loescht ein gebautes Artefakt.
///
/// # 📌 Was hier alles schiefgehen koennte, und was es verhindert
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
    let roh = std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json"))
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
    // ⚠️ Waehrend einer Runde des Loops gilt es als geladen: Wer nicht
    //    nachsehen kann, loescht nicht (und friert den Hauptfaden nicht ein).
    let geladen = halter.modell.try_lock().map(|g| g.is_some()).unwrap_or(true);
    if geladen && e.modell.artefakt.trim_end_matches('/').ends_with(&schluessel) {
        return Err(
            "Dieses Artefakt ist gerade geladen. Waehle ein anderes Modell und lade es, \
             dann laesst es sich loeschen."
                .to_string(),
        );
    }

    let belegt = myl_client::reservierung::belegung(&echt);
    std::fs::remove_dir_all(&echt).map_err(|e| format!("{}: {e}", echt.display()))?;
    // ⚑ Die Belegung hat sich geaendert (Fund 555).
    belegung_vergessen();
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
/// # 📌 Warum das ueber den Ruecken geht und nicht im Skript geschieht
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
async fn hardware() -> Result<Freigabemaske, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let h = myl_client::hardware::erheben(&datenort());
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
/// 📌 **Auch die Beschriftung kommt von dort.** Dieser Befehl gab
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
    // ⚑ **Was nur in der Konsole wirkt, steht hier nicht.** Eine
    // Einstellung, die an der Stelle, an der sie steht, nichts bewirkt,
    // ist schlimmer als eine fehlende. ⚑ Gefragt wird die Kiste, seit
    // dem 2026-09-16 in beide Richtungen: Das Fenster zaehlt nicht
    // selbst auf, was es nicht zeigt.
    Ok(myl_client::einstellungen::FELDER
        .iter()
        .filter(|f| f.gilt.im_fenster())
        .map(|f| f.in_sprache(s))
        .collect())
}

/// **Sieht nach, ob GitHub eine hoehere Fassung nennt als dieses
/// Programm** (`VERSION.md`, erste Zeile).
///
/// ⚑ **Es aendert nichts.** Nachsehen und Einspielen sind zwei
/// Befehle, weil sie zwei Entscheidungen sind: Die eine kostet eine
/// Netzanfrage, die andere Minuten und einen neuen Programmstand.
/// ⚑ **Das Fenster fragt das beim Start von selbst** (Auftrag des
/// Projektinhabers, 2026-10-17); eingespielt wird erst nach einer
/// Bestaetigung.
#[tauri::command]
async fn aktualisierung() -> myl_client::aktualisierung::Stand {
    tauri::async_runtime::spawn_blocking(myl_client::aktualisierung::pruefen)
        .await
        .unwrap_or_else(|e| myl_client::aktualisierung::Stand::aus(None, "", Err(e.to_string())))
}

/// **Startet das Fenster neu**, nach einem gelungenen Update: Das
/// Installationsskript hat das Buendel an derselben Stelle ersetzt, und der
/// Neustart oeffnet das neue.
#[tauri::command]
fn neu_starten(app: tauri::AppHandle) {
    app.restart();
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
/// 📌 **Auch hier keine eigene Logik.** Was `aus` heisst und welche
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
        // ⚑ **Ein Rechenwerk wirkt beim Setzen, wie jede andere
        // Freigabe.** Wer den Regler auf „rechnet nicht" zieht und
        // danach dieselbe Vorbereitungszeit sieht, hat keinen Regler
        // bedient, sondern eine Zahl geaendert.
        f if f.starts_with(myl_client::einstellungen::RECHENWERK_PRAEFIX) => {
            myl_client::hardware::anwenden(&e);
        }
        // 📌 **Die Platte wird sofort gehalten oder hergegeben.** Sonst
        // stuende zwischen dem Setzen und dem naechsten Start eine
        // Zusage, die niemand einloest.
        "kap.platte" => {
            platte_nachfuehren(&halter)?;
        }
        // ⚑ **Der Debug-Log schaltet beim Setzen**, nicht beim naechsten Start.
        "debug.log" => {
            e.debug_anwenden("Fenster", "Einstellung im Fenster");
        }
        _ => {}
    }
    Ok(())
}

/// Welche Modelle zur Wahl stehen.
///
/// ⚑ **Gerechnet wird in der Kiste, nicht hier** (seit dem 2026-09-11).
/// Die Konsole stellt dieselbe Frage und bekommt dieselbe Antwort;
/// **zwei Listen, die dasselbe aufzaehlen, zaehlen irgendwann
/// verschieden auf.**
#[tauri::command]
async fn modelle() -> Result<Vec<myl_client::modelle::Modellwahl>, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    Ok(myl_client::modelle::liste(&e))
}

/// Welche Werkzeuge der Agent gerade hat.
///
/// # 📌 Warum das ein Befehl ist und keine Liste im Fenster
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
async fn werkzeuge(wurzel: Option<String>) -> Result<Werkzeugliste, String> {
    let mut e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    // ⚑ Die Leiste zeigt, was wirklich zur Verfuegung steht, samt Modul.
    let modul = e.agent.modul.clone();
    let (_, modulgrund) = modul_anwenden(&mut e);
    let kiste = kiste_fuer(&e);
    // ⚑ **Der effektive Ordner**: der gesetzte, sonst `WORK_DIR` als
    // Vorgabe (wie in `ruestung`), damit die Anzeige das Gleiche zeigt, was
    // der Agent wirklich anfasst. Er ist zugleich der Platzhalter im
    // leeren Feld der Einstellungen.
    // ⚑ **Der Pfad des Prozesses schlaegt die Einstellung** (Auftrag des
    // Projektinhabers, 2026-09-15: „der Einhaengepfad soll je Prozess
    // gespeichert bleiben"). Derselbe Bau wie in der Konsole, wo das
    // Startverzeichnis der Sitzung die Einstellung schlaegt: Wer einen
    // Prozess in einem Ordner fuehrt, fuehrt ihn dort weiter, auch wenn
    // der naechste Prozess woanders laeuft.
    //
    // Die Reihenfolge: der Prozess, dann die Einstellung, dann die
    // Vorgabe `WORK_DIR`.
    let wurzel = wurzel
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .or_else(|| e.agent.wurzel.clone())
        .or_else(myl_client::Einstellungen::standard_wurzel);
    let Some(pfad) = wurzel else {
        return Ok(Werkzeugliste {
            wurzel: None,
            kiste: kiste.name().to_string(),
            kistenheimat: kistenheimat(&e),
            kistenordner: geltender_kistenordner(&e),
            namen: Vec::new(),
            modul: modul.clone(),
            modulgrund: modulgrund.clone(),
        });
    };
    // ⚑ Die Einhaengung wird hier wirklich gebaut und nicht geraten:
    // Ein Pfad, der nicht existiert, hat auch keine Werkzeuge, und das
    // soll man sehen, bevor der Auftrag laeuft.
    let ein = match myl_local_agent::werkzeuge::Einhaengung::neu(&pfad, e.agent.schreiben) {
        Ok(x) => x,
        Err(m) => {
            return Ok(Werkzeugliste {
                wurzel: Some(format!("{pfad}  ({m})")),
                kiste: kiste.name().to_string(),
                kistenheimat: kistenheimat(&e),
                kistenordner: geltender_kistenordner(&e),
                namen: Vec::new(),
                modul,
                modulgrund,
            })
        }
    };
    let mut namen: Vec<String> =
        myl_local_agent::werkzeuge::angebote(&ein, myl_client::Ansageform::Amtlich, kiste)
            .into_iter()
            .map(|w| w.name)
            .collect();
    // ⛔️ **Und die Sinneswerkzeuge** (2026-09-18). Sie kamen seit dem
    // Vortag in die Ruestung, standen aber nicht in dieser Liste:
    // **dieselbe Frage an zwei Orten**, und der zweite meldet sich
    // nicht. Wer hier nachsah, bekam eine Liste, die dem Agenten nicht
    // entsprach.
    for (angebot, _) in myl_local_agent::sinneswerkzeuge::angebote(
        &myl_senses::Sinne::finden(),
        &ein,
        myl_client::Ansageform::Amtlich,
        // ⚑ **Dieselbe Quelle wie die Ruestung.** Die Liste soll zeigen,
        //   was der Agent wirklich hat; eine eigene Entscheidung hier
        //   waere genau die zweite Wahrheit, gegen die der Absatz
        //   darueber geschrieben ist.
        myl_local_agent::sinneswerkzeuge::Blickbefugnis::aus_einstellung(&e.agent),
    ) {
        namen.push(angebot.name);
    }

    // ⚑ **Auch die Werkzeuge aus dem Kisten-Ordner** (2026-09-14), damit die
    // Seitenleiste zeigt, was wirklich zur Verfuegung steht.
    let kette = myl_local_agent::kisten::ordnerkette(&e.agent);
    for (angebot, _) in myl_local_agent::kisten::angebote_der_kette(&kette, &ein, |_| {}) {
        namen.push(angebot.name);
    }
    let ordner = myl_local_agent::kisten::ordner_der_gilt(
        e.agent.kistenordner.as_deref(),
        myl_local_agent::werkzeuge::Werkzeugkiste::Base.name(),
    );
    Ok(Werkzeugliste {
        wurzel: Some(ein.wurzel().display().to_string()),
        kiste: myl_local_agent::kisten::ordnername(
            ordner.as_deref(),
            myl_local_agent::werkzeuge::Werkzeugkiste::Base.name(),
        ),
        kistenheimat: kistenheimat(&e),
        kistenordner: geltender_kistenordner(&e),
        namen,
        modul,
        modulgrund,
    })
}

/// **Der Ordner, in dem die Werkzeugkisten nebeneinander liegen.**
///
/// ⚑ Der Elternordner der geltenden Kiste: Wer `Base` benutzt, soll bei
/// der Wahl `Base`, `Advanced` und `1337` nebeneinander sehen, nicht den
/// Inhalt von `Base`.
fn geltender_kistenordner(e: &myl_client::Einstellungen) -> Option<String> {
    myl_local_agent::kisten::ordner_der_gilt(
        e.agent.kistenordner.as_deref(),
        myl_local_agent::werkzeuge::Werkzeugkiste::Base.name(),
    )
    .map(|o| o.display().to_string())
}

fn kistenheimat(e: &myl_client::Einstellungen) -> Option<String> {
    let o = myl_local_agent::kisten::ordner_der_gilt(
        e.agent.kistenordner.as_deref(),
        myl_local_agent::werkzeuge::Werkzeugkiste::Base.name(),
    )?;
    Some(o.parent().unwrap_or(&o).display().to_string())
}

/// Die Warnung vor dem Agentenbetrieb, fertig uebersetzt.
///
/// ⚑ **Der Text kommt aus der Kiste**, nicht aus dem Skript: Konsole und
/// Fenster muessen dasselbe sagen, und zweimal getippt liefe es
/// auseinander.
#[derive(Serialize)]
struct Warnungsansicht {
    achtung: String,
    kern: String,
    anleitung_titel: String,
    regeln: Vec<(String, String)>,
    zustimmung: String,
    nicht_wieder: String,
}

/// **Die Nachfrage vor einer Handlung mit Wirkung nach aussen**, wenn der
/// Modus es verlangt.
///
/// ⚑ **Der Kasten ist der des Betriebssystems**, derselbe Weg wie bei
/// der Ordnerwahl: blockierend, und das ist erlaubt, weil jeder Lauf in
/// `spawn_blocking` liegt und nicht auf dem Hauptfaden. Eine Stelle fuer
/// Agent und Chat: Zwei Kaesten fuer dieselbe Frage liefen auseinander.
fn nachfrage_fuer(
    fenster: &tauri::AppHandle,
    modus: myl_client::einstellungen::Agentenmodus,
) -> Option<myl_local_agent::ruestung::Nachfrage> {
    nachfrage_ohne(fenster, modus, Vec::new())
}

/// Wie [`nachfrage_fuer`]; die Werkzeuge in `ohne` fragen nicht nach (die
/// lesenden Werkzeuge eines Moduls, `ohne_nachfrage` in seiner `kiste.json`).
fn nachfrage_ohne(
    fenster: &tauri::AppHandle,
    modus: myl_client::einstellungen::Agentenmodus,
    ohne: Vec<String>,
) -> Option<myl_local_agent::ruestung::Nachfrage> {
    if !modus.fragt_nach() {
        return None;
    }
    let app = fenster.clone();
    Some(std::sync::Arc::new(move |name: &str, argumente: &serde_json::Value| {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
        if ohne.iter().any(|n| n == name) {
            return true;
        }
        // ⚑ Die Argumente stehen mit im Kasten: Eine Zustimmung ohne
        // zu wissen, **worauf**, ist keine.
        let was = serde_json::to_string_pretty(argumente).unwrap_or_else(|_| argumente.to_string());
        app.dialog()
            .message(format!("{name}\n\n{was}"))
            .title("Diese Handlung ausfuehren?")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Ausfuehren".to_string(),
                "Ablehnen".to_string(),
            ))
            .blocking_show()
    }))
}

/// **Das eingestellte Modul an einen Lauf haengen** (Festlegung des
/// Projektinhabers, 2026-10-12): Seine Kiste ergaenzt die gewaehlte, seine
/// Skills kommen dazu. Liefert seine lesenden Werkzeuge (ohne Nachfrage)
/// und, falls es nicht besteht, den Grund; dann laeuft der Lauf ohne Modul.
fn modul_anwenden(e: &mut myl_client::Einstellungen) -> (Vec<String>, Option<String>) {
    match myl_client::module::eingestellter_zusatz(&e.agent) {
        Some(Ok(z)) => {
            z.anwenden(&mut e.agent);
            (z.ohne_nachfrage, None)
        }
        Some(Err(f)) => {
            myl_local_agent::skills::modusordner_setzen(None);
            (Vec::new(), Some(f))
        }
        None => {
            myl_local_agent::skills::modusordner_setzen(None);
            (Vec::new(), None)
        }
    }
}

/// **Die Befehle der gepruefen Module**, fuer die Eingabe im
/// Agentenmodus. `neu` prueft alle Module erneut.
#[tauri::command]
fn modul_befehle(neu: Option<bool>, halter: tauri::State<'_, Halter>) -> Result<Vec<modulwirt::Modulbefehl>, String> {
    halter.module.befehle(neu.unwrap_or(false))
}

/// **Ein Befehl eines Moduls**, im Agentenmodus getippt: Das Modul startet,
/// falls noetig, und bekommt die Zeile. Antworten kommen als
/// `modul-ereignis`.
#[tauri::command]
async fn modul_befehl(zeile: String, fenster: tauri::AppHandle, halter: tauri::State<'_, Halter>) -> Result<String, String> {
    // ⚑ Pruefen und Starten brauchen Sekunden (Handschlag); nicht im
    //   Befehlsfaden des Fensters.
    let module = halter.module.clone();
    // ⚑ Wer das Modell braucht, weckt den Schlaf zuerst.
    halter.ruhe.wecken();
    let modell = halter.modell.clone();
    tauri::async_runtime::spawn_blocking(move || module.befehl(&zeile, &fenster, &modell))
        .await
        .map_err(|e| format!("der Modulfaden ist abgestuerzt: {e}"))?
}

/// **Ein Modul waehlen** (Seitenleiste, unter der Werkzeugkiste): Es
/// startet, jedes andere endet; `None` beendet alle. Die Einstellung
/// `agent.modul` setzt die Oberflaeche vorher ueber `setzen`.
#[tauri::command]
async fn modul_waehlen(name: Option<String>, fenster: tauri::AppHandle, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    let module = halter.module.clone();
    // ⚑ Wer das Modell braucht, weckt den Schlaf zuerst.
    halter.ruhe.wecken();
    let modell = halter.modell.clone();
    tauri::async_runtime::spawn_blocking(move || module.waehlen(name.as_deref(), &fenster, &modell))
        .await
        .map_err(|e| format!("der Modulfaden ist abgestuerzt: {e}"))?
}

/// **Die Datei einer Ansicht** (Bild oder Koerper) als Bytes, ohne Umweg
/// ueber Text: Die Oberflaeche zeichnet sie selbst.
#[tauri::command]
fn modul_datei(name: String, datei: String, halter: tauri::State<'_, Halter>) -> Result<tauri::ipc::Response, String> {
    halter.module.datei(&name, &datei).map(tauri::ipc::Response::new)
}

/// Der Stand der laufenden Module, fuer Fusszeile und Bereich unten.
#[tauri::command]
fn modul_stand(halter: tauri::State<'_, Halter>) -> Vec<modulwirt::Modulstand> {
    halter.module.stand()
}

/// Die Antwort des Menschen auf eine Frage eines Moduls; `None` heisst
/// abgebrochen.
#[tauri::command]
fn modul_antwort(name: String, id: u64, antwort: Option<String>, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    halter.module.antworten(&name, id, antwort)
}

/// **Die Module zur Wahl** in der Seitenleiste: gepruefte zuerst, die
/// anderen mit ihrem Grund.
#[tauri::command]
async fn module_wahl() -> Vec<myl_client::module::Modulwahl> {
    myl_client::module::wahl()
}

// 📌 **Hier stand der Befehl `notaus`**, gerufen vom Knopf im Kopf und
// spaeter nur noch vom Tastenkuerzel ⌘. oder Strg+. Beide sind entfallen
// (Festlegung des Projektinhabers, 2026-09-28); der Notschalter des
// Fensters ist das Schliessen, siehe [`notschalter_beim_schliessen`].

// ── Der Loop: ∞ neben dem Senden ────────────────────────────────────
//
// ⚑ **Festlegungen des Projektinhabers (2026-09-26):**
// - ∞ ist ein Schalter: an faehrt der Loop, aus haelt er an.
// - Daneben eine Liste der Tasks: auswaehlen, neu anlegen, und per
//   Ziehen die Reihenfolge aendern. Der vorderste laeuft („(läuft)"),
//   alle dahinter warten („(queued)").
//
// ⚑ **Und vom 2026-10-12:**
// - Ein Loop endet nur, wenn der Mensch ihn anhaelt, das Ergebnis
//   bestaetigt oder das Fenster schliesst; keine Hoechstzahl an Runden.
// - Wird das Fenster geschlossen, ist der Task, der lief, danach
//   angehalten; ▶ in der Liste setzt ihn genau dort fort. 📌 Bis CLIENT
//   v0.132.0 fuhr er beim naechsten Oeffnen von selbst weiter.
// - Meldet das Modell das Ziel als erreicht, fragt das Fenster nach, ob
//   das Ergebnis stimmt oder wo weitergearbeitet werden soll.
// - In der Liste je Task ▶ oder ⏸ und der Papierkorb; ▶ startet den Loop
//   mit genau diesem Task.
//
// ⚑ **Die Logik steht in `myl_local_agent::vorhaben`**, wie bei Konsole und
// `myl`. Hier stehen nur Faden, Leihe und Meldungen ans Fenster.

/// Der Ereignisname fuer Runden, Wartezeiten und das Ende des Loops.
const LOOPEREIGNIS: &str = "loop-ereignis";

/// ⚑ **Der Tokenstrom einer Runde laeuft auf einem eigenen Kanal**, nicht
/// auf `LEBEND`. Ein Chat und eine Runde koennen sich im Fenster zeitlich
/// beruehren (die Runde wartet auf das Modell, das der Chat gerade
/// freigibt); auf einem Kanal liefe der Anfang der Runde in den Beitrag
/// des Chats.
const LOOPLEBEND: &str = "loop-lebt";

/// Was das Fenster ueber den Loop wissen muss, ueber Befehle hinweg.
#[derive(Default, Clone)]
struct Loopzustand {
    /// Der Faden laeuft.
    laeuft: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// ∞ wurde ausgeschaltet: Nach dem Ende geht die Marke `.aktiv` weg.
    pause: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Gerade rechnet eine Runde; das Modell gehoert ihr.
    in_runde: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Der Faden des Loops, damit die Pause dessen Befehle beenden kann.
    faden: std::sync::Arc<Mutex<Option<std::thread::ThreadId>>>,
    /// Je Gespraech, was seine Runden brauchen; das Fenster haelt es frisch.
    umfelder: std::sync::Arc<Mutex<std::collections::HashMap<String, Umfeld>>>,
}

/// **Was eine Runde von ihrem Gespraech mitbekommt**: den Verlauf, wie ihn
/// der Agent saehe, und den Ordner, falls das Gespraech einen eigenen hat.
///
/// ⚑ **Vom Fenster geschickt und nicht gelesen**: Die Gespraeche liegen im
/// Speicher der Webansicht. Das Fenster schickt beides, wenn der Loop
/// startet, ein Task angelegt wird, ein Auftrag oder eine Runde endet und
/// der Ordner wechselt.
#[derive(Clone, Default)]
struct Umfeld {
    verlauf: Vec<myl_client::Nachricht>,
    wurzel: Option<String>,
}

/// **Das Umfeld eines Gespraechs fuer seine Loop-Runden.**
#[tauri::command]
fn loop_umfeld(
    gespraech: String,
    verlauf: Vec<myl_client::Nachricht>,
    wurzel: Option<String>,
    halter: tauri::State<'_, Halter>,
) -> Result<(), String> {
    let mut g = halter.schleife.umfelder.lock().map_err(|_| "das Umfeld ist vergiftet".to_string())?;
    g.insert(gespraech, Umfeld { verlauf, wurzel: wurzel.filter(|w| !w.trim().is_empty()) });
    Ok(())
}

/// Was beim Loeschen eines Gespraechs mit wegging.
#[derive(Serialize, Default)]
struct Vergessen {
    /// Tasks des Gespraechs, mit ihren Notizen und ihrem Tagebuch.
    tasks: usize,
    /// Mitschnitte des Gespraechs im Arbeitsordner (`.AGENT/<kennung>/`).
    mitschnitte: usize,
    /// Ob der gerechnete Zustand des Modells geleert wurde.
    zustand: bool,
    /// Tasks, die gerade eine Runde rechnen und deshalb blieben.
    geblieben: Vec<String>,
}

/// **Ein geloeschtes Gespraech nimmt mit, was zu ihm gehoert** (Wunsch des
/// Projektinhabers, 2026-10-13): sein Umfeld fuer Loop-Runden, seine Tasks
/// (samt Notizen, Tagebuch und Rundenstand), die Mitschnitte seiner
/// Verdichtungen im Arbeitsordner und den gerechneten Zustand des Modells.
/// Den verdichteten Kontext selbst traegt das Gespraech im Speicher des
/// Fensters; er geht mit ihm.
///
/// ⚑ **Das dreht eine Festlegung vom 2026-09-16 um**: Bis dahin fasste das
/// Aufraeumen der Liste den Mitschnitt ausdruecklich nicht an, damit ein
/// neuer Agent im selben Ordner aufnehmen konnte. Geloescht wird jetzt nur
/// der Ordner **dieses** Gespraechs; die anderer Gespraeche bleiben.
///
/// ⚠️ **Der Zustand des Modells wird nur geleert, wenn es frei ist.** Rechnet
/// es gerade, gehoert der Zustand dem anderen Gespraech, und der Anfang des
/// geloeschten faellt beim naechsten Lauf ohnehin heraus.
#[tauri::command]
fn gespraech_vergessen(kennung: String, wurzel: Option<String>, halter: tauri::State<'_, Halter>) -> Result<Vergessen, String> {
    if let Ok(mut u) = halter.schleife.umfelder.lock() {
        u.remove(&kennung);
    }
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok();
    let orte = [wurzel.filter(|w| !w.trim().is_empty()), e.as_ref().and_then(|e| e.agent.wurzel.clone()), myl_client::Einstellungen::standard_wurzel()];
    vergessen_in(
        &kennung,
        orte.into_iter().flatten().map(std::path::PathBuf::from),
        &myl_local_agent::vorhaben::Ablage::vorgabe(),
        &halter.modell,
        |task| nicht_in_seiner_runde(&halter, task).is_err(),
    )
}

/// Der Kern von [`gespraech_vergessen`], ohne Fenster: Tasks, Mitschnitte
/// in jedem der `orte` (jeder einmal), Zustand des Modells.
fn vergessen_in(
    kennung: &str,
    orte: impl IntoIterator<Item = std::path::PathBuf>,
    ablage: &myl_local_agent::vorhaben::Ablage,
    modell: &Mutex<Option<myl_client::Oertlichesmodell>>,
    rechnet: impl Fn(&str) -> bool,
) -> Result<Vergessen, String> {
    let mut aus = Vergessen::default();
    for v in ablage.reihe().into_iter().filter(|v| v.gespraech.as_deref() == Some(kennung)) {
        if rechnet(&v.kennung) {
            aus.geblieben.push(v.kennung);
            continue;
        }
        ablage.entfernen(&v.kennung)?;
        aus.tasks += 1;
    }
    let mut gesehen: Vec<std::path::PathBuf> = Vec::new();
    for w in orte {
        if gesehen.contains(&w) {
            continue;
        }
        aus.mitschnitte += myl_local_agent::verlauf::loeschen(&w, Some(kennung)).map_err(|f| f.to_string())?;
        gesehen.push(w);
    }
    if let Ok(g) = modell.try_lock() {
        if let Some(m) = g.as_ref() {
            m.vergessen();
            aus.zustand = true;
        }
    }
    Ok(aus)
}

/// Eine Zeile der Taskliste.
#[derive(Serialize)]
struct Taskzeile {
    kennung: String,
    ziel: String,
    /// Der Name in der Liste, sonst das gekuerzte Ziel.
    name: String,
    /// Was die Pruefung gegen ein vorgelegtes Ergebnis sagt (Fund 531).
    einwand: Option<String>,
    /// Was das Modell den Menschen fragt (`ask_human`).
    frage: Option<String>,
    stellung: myl_local_agent::vorhaben::Stellung,
    /// „läuft", „queued", „pausiert" …, aus der Kiste.
    wort: String,
    runden: u32,
    ergebnis: Option<String>,
    /// Das Gespraech, zu dem der Task gehoert (Kennung des Fensters).
    gespraech: Option<String>,
}

#[derive(Serialize)]
struct Taskansicht {
    /// Laeuft der Loop in diesem Fenster?
    laeuft: bool,
    in_runde: bool,
    eintraege: Vec<Taskzeile>,
}

fn sprache_jetzt() -> myl_client::einstellungen::Sprache {
    myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .map(|e| e.oberflaeche.sprache)
        .unwrap_or_default()
}

/// **Die Tasks in ihrer Reihenfolge**, fuer die Liste an ∞.
#[tauri::command]
fn tasks(halter: tauri::State<'_, Halter>) -> Taskansicht {
    use std::sync::atomic::Ordering;
    let ablage = myl_local_agent::vorhaben::Ablage::vorgabe();
    let an = halter.schleife.laeuft.load(Ordering::SeqCst);
    let sprache = sprache_jetzt();
    Taskansicht {
        laeuft: an,
        in_runde: halter.schleife.in_runde.load(Ordering::SeqCst),
        eintraege: ablage
            .stellungen()
            .into_iter()
            .map(|(v, st)| Taskzeile {
                wort: st.wort(&v, an, sprache),
                name: v.anzeigename(),
                einwand: match &v.zustand {
                    myl_local_agent::vorhaben::Zustand::Vorgelegt { einwand, .. } => einwand.clone(),
                    _ => None,
                },
                frage: match &v.zustand {
                    myl_local_agent::vorhaben::Zustand::Frage { frage } => Some(frage.clone()),
                    _ => None,
                },
                kennung: v.kennung,
                ziel: v.ziel,
                stellung: st,
                runden: v.runden,
                ergebnis: v.ergebnis,
                gespraech: v.gespraech,
            })
            .collect(),
    }
}

/// **Legt einen Task an**; er steht hinten in der Schlange.
///
/// ⚑ **Gebunden an das Gespraech, in dem er angelegt wurde**, samt dessen
/// Ordner (Festlegung des Projektinhabers, 2026-09-30).
///
/// ⚑ **Benannt vom Modell, wenn es frei ist** (2026-10-12), und zwar in
/// einem eigenen Faden: Der Befehl kehrt sofort zurueck, der Name kommt
/// mit dem Ereignis `Benannt`. Ist das Modell nicht geladen oder rechnet
/// gerade ein Chat oder eine Runde, wird nicht gewartet; dann steht das
/// gekuerzte Ziel in der Liste.
#[tauri::command]
fn task_anlegen(
    ziel: String,
    gespraech: Option<String>,
    wurzel: Option<String>,
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<String, String> {
    let sprache = sprache_jetzt();
    // ⛔️ Derselbe Schutzfilter wie vor jedem Auftrag (Art. 5 KI-Verordnung).
    if let Some(satz) = myl_client::schutzfilter::abweisen(&ziel, sprache, "fenster-loop") {
        return Err(satz);
    }
    let ablage = myl_local_agent::vorhaben::Ablage::vorgabe();
    let v = ablage.anlegen_fuer(&ziel, myl_local_agent::vorhaben::jetzt(), gespraech, wurzel)?;
    // ⚑ Wer das Modell braucht, weckt den Schlaf zuerst.
    halter.ruhe.wecken();
    let modell = halter.modell.clone();
    let kennung = v.kennung.clone();
    std::thread::spawn(move || {
        let name = modell
            .try_lock()
            .ok()
            .and_then(|g| g.as_ref().map(|m| myl_local_agent::vorhaben::benennen(m, &v.ziel, sprache)));
        if let Some(n) = name {
            if ablage.name_setzen(&v.kennung, &n).is_ok() {
                let _ = fenster.emit(LOOPEREIGNIS, serde_json::json!({ "art": "Benannt", "kennung": v.kennung, "name": n }));
            }
        }
    });
    Ok(kennung)
}

/// **Die neue Reihenfolge nach dem Ziehen.**
#[tauri::command]
fn tasks_ordnen(reihe: Vec<String>) -> Result<(), String> {
    myl_local_agent::vorhaben::Ablage::vorgabe().reihe_setzen(&reihe)
}

/// ⚠️ **Nicht mitten in seiner Runde.** Die Runde schriebe am Ende ihren
/// eigenen Stand zurueck, und das Anhalten oder Entfernen waere dann
/// still verloren.
fn nicht_in_seiner_runde(halter: &Halter, kennung: &str) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let vorn = myl_local_agent::vorhaben::Ablage::vorgabe().vorn().map(|v| v.kennung);
    if halter.schleife.in_runde.load(Ordering::SeqCst) && vorn.as_deref() == Some(kennung) {
        return Err(match sprache_jetzt() {
            myl_client::einstellungen::Sprache::De => {
                "Dieser Task rechnet gerade eine Runde. ∞ pausiert den Loop; danach geht es.".into()
            }
            myl_client::einstellungen::Sprache::En => {
                "This task is in the middle of a round. ∞ pauses the loop; then it works.".into()
            }
        });
    }
    Ok(())
}

/// **▶ an einem Task**: fortsetzen und nach vorn, damit genau dieser als
/// naechstes laeuft. Den Loop schaltet danach das Fenster an.
#[tauri::command]
fn task_weiter(kennung: String) -> Result<(), String> {
    myl_local_agent::vorhaben::Ablage::vorgabe().wieder_aufnehmen(&kennung).map(|_| ())
}

/// **Das vorgelegte Ergebnis stimmt**: fertig. Liefert die Ziele einer
/// Kette, die damit beginnt.
#[tauri::command]
fn task_bestaetigen(kennung: String) -> Result<Vec<String>, String> {
    let neue = myl_local_agent::vorhaben::Ablage::vorgabe().bestaetigen(&kennung, sprache_jetzt())?;
    Ok(neue.into_iter().map(|v| v.ziel).collect())
}

/// **Das vorgelegte Ergebnis stimmt nicht**: weiter, und zwar bei `hinweis`.
/// Der Task steht danach vorn.
#[tauri::command]
fn task_nachbessern(kennung: String, hinweis: String) -> Result<(), String> {
    let ablage = myl_local_agent::vorhaben::Ablage::vorgabe();
    ablage.nachbessern(&kennung, &hinweis, sprache_jetzt())?;
    ablage.nach_vorn(&kennung)
}

/// **Die Antwort auf eine Frage des Modells** (`ask_human`). Der Task steht
/// danach vorn und ist bereit.
#[tauri::command]
fn task_antworten(kennung: String, antwort: String) -> Result<(), String> {
    let ablage = myl_local_agent::vorhaben::Ablage::vorgabe();
    ablage.antworten(&kennung, &antwort, sprache_jetzt())?;
    ablage.nach_vorn(&kennung)
}

/// **Beim Start des Fensters**: Lief der Loop, als es zuletzt zuging (oder
/// endete es hart), ist der Task, der lief, jetzt angehalten. Liefert
/// seinen Namen fuer den Hinweis.
#[tauri::command]
fn loop_nach_dem_start() -> Option<String> {
    myl_local_agent::vorhaben::Ablage::vorgabe().nach_dem_schliessen(sprache_jetzt()).map(|v| v.anzeigename())
}

#[tauri::command]
fn task_stoppen(kennung: String, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    nicht_in_seiner_runde(&halter, &kennung)?;
    myl_local_agent::vorhaben::Ablage::vorgabe().stoppen(&kennung, sprache_jetzt()).map(|_| ())
}

/// ⛔️ Entfernt samt Tagebuch; das Fenster fragt vorher.
#[tauri::command]
fn task_entfernen(kennung: String, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    nicht_in_seiner_runde(&halter, &kennung)?;
    myl_local_agent::vorhaben::Ablage::vorgabe().entfernen(&kennung)
}

/// **Das Ziel eines Tasks aendern** (Rechtsklick, Bearbeiten). Nicht waehrend
/// seiner Runde: Die laeuft mit dem Ziel, mit dem sie begann.
#[tauri::command]
fn task_bearbeiten(kennung: String, ziel: String, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    nicht_in_seiner_runde(&halter, &kennung)?;
    myl_local_agent::vorhaben::Ablage::vorgabe().ziel_aendern(&kennung, &ziel).map(|_| ())
}

/// Die Sicherheitsmeldung vor `auto`, aus der Kiste (derselbe Text wie in
/// der Konsole und bei `myl setzen`).
#[derive(Serialize)]
struct Autowarnungansicht {
    titel: String,
    punkte: Vec<String>,
    frage: String,
}

#[tauri::command]
fn autowarnung() -> Autowarnungansicht {
    let w = myl_client::einstellungen::autowarnung(sprache_jetzt());
    Autowarnungansicht {
        titel: w.titel.to_string(),
        punkte: w.punkte.iter().map(|p| p.to_string()).collect(),
        // ⚑ Ohne das „[j/N]" der Konsole: Hier antworten zwei Knoepfe.
        frage: w.frage.trim().trim_end_matches("[j/N]").trim_end_matches("[y/N]").trim().to_string(),
    }
}

/// **∞ an: der Loop faehrt in einem eigenen Faden.**
///
/// Gibt den Hinweis auf Standardeinstellungen zurueck, falls sie noch
/// gelten (Wunsch des Projektinhabers: er steht im Ausgabefenster).
#[tauri::command]
fn loop_starten(fenster: tauri::AppHandle, halter: tauri::State<'_, Halter>) -> Result<Option<String>, String> {
    use myl_local_agent::vorhaben::{self, Ablage, Laeufer};
    use std::sync::atomic::Ordering;
    let z = halter.schleife.clone();
    if z.laeuft.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }
    let vorbereitet = (|| {
        let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
        let ablage = Ablage::vorgabe();
        let laeufer = Laeufer::oeffnen(ablage.clone(), "Fenster")?;
        Ok::<_, String>((e, ablage, laeufer))
    })();
    let (e, ablage, laeufer) = match vorbereitet {
        Ok(x) => x,
        Err(f) => {
            z.laeuft.store(false, Ordering::SeqCst);
            return Err(f);
        }
    };
    let hinweis = vorhaben::hinweis_vorgaben(&e.schleife, e.agent.modus, e.oberflaeche.sprache);
    vorhaben::schliessen_zuruecksetzen();
    z.pause.store(false, Ordering::SeqCst);
    let _ = ablage.loop_aktiv_setzen(true);
    // ⚑ Wer das Modell braucht, weckt den Schlaf zuerst.
    halter.ruhe.wecken();
    let modell = halter.modell.clone();
    let ruhe = halter.ruhe.clone();

    let zf = z.clone();
    let griff = std::thread::spawn(move || {
        let f = fenster.clone();
        let zr = z.clone();
        // ⚑ **Die Leihe**: das Modell fuer genau eine Runde, und falls es
        //   in der Zwischenzeit entladen wurde (Ruhefrist), wird es geladen.
        let leihen = |runde: &mut dyn FnMut(&dyn myl_client::Modellweg)| -> Result<(), String> {
            // ⚑ Eine Runde weckt den Schlaf, wie jeder Auftrag.
            ruhe.wecken();
            let mut g = modell.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
            if g.is_none() {
                let _ = f.emit(LOOPEREIGNIS, serde_json::json!({ "art": "Laedt" }));
                *g = Some(modell_aus(&e)?);
            }
            let m = g.as_mut().ok_or("das Modell ist nicht geladen")?;
            // Frisch je Runde: Das Feld am Eingabefeld kann Saat und
            // Parameter zwischen zwei Runden aendern.
            if let Ok(frisch) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
                ausgabe_uebernehmen(m, &frisch);
            }
            zusehen_mit(m, &f, None, "", LOOPLEBEND);
            zr.in_runde.store(true, Ordering::SeqCst);
            runde(&*m);
            zr.in_runde.store(false, Ordering::SeqCst);
            m.beobachter = None;
            Ok(())
        };
        // ⚑ Das eingestellte Modul ergaenzt auch die Runden eines Loops.
        let mut mit_modul = e.clone();
        let (ohne, _modulgrund) = modul_anwenden(&mut mit_modul);
        let nachfrage = nachfrage_ohne(&fenster, e.agent.modus, ohne);
        let kiste = kiste_fuer(&e);
        let umfelder = z.umfelder.clone();
        let umfeld = move |v: &myl_local_agent::vorhaben::Vorhaben| -> Option<Umfeld> {
            let g = v.gespraech.as_ref()?;
            umfelder.lock().ok()?.get(g).cloned()
        };
        let ruester = |zusaetzlich: myl_local_agent::vorhaben::Zusatzwerkzeuge, v: &myl_local_agent::vorhaben::Vorhaben| {
            let mut agent = v.agent_fuer(&mit_modul.agent);
            // ⚑ Der Ordner, den das Gespraech jetzt hat, vor dem beim Anlegen.
            if let Some(w) = umfeld(v).and_then(|u| u.wurzel) {
                agent.wurzel = Some(w);
            }
            myl_local_agent::ruestung::ruesten_mit(
                &agent,
                myl_client::Ansageform::Amtlich,
                kiste,
                zusaetzlich,
                nachfrage.clone(),
            )
        };
        let sprache = e.oberflaeche.sprache;
        let melden = |ev: vorhaben::Ereignis| {
            use vorhaben::Ereignis;
            let j = match ev {
                Ereignis::Beginnt { kennung, ziel, runde } => {
                    serde_json::json!({ "art": "Beginnt", "kennung": kennung, "ziel": ziel, "runde": runde })
                }
                Ereignis::Geendet { vorhaben: v, bericht, pruefung, saat } => serde_json::json!({
                    "art": "Geendet",
                    // ⚑ Steht hier ein Ergebnis, fragt das Fenster nach.
                    "vorgelegt": match &v.zustand {
                        vorhaben::Zustand::Vorgelegt { ergebnis, .. } => Some(ergebnis.clone()),
                        _ => None,
                    },
                    // ⚑ Steht hier eine Frage des Modells, fragt das Fenster.
                    "frage": match &v.zustand {
                        vorhaben::Zustand::Frage { frage } => Some(frage.clone()),
                        _ => None,
                    },
                    "name": v.anzeigename(),
                    "kennung": v.kennung,
                    "ziel": v.ziel,
                    "runde": v.runden,
                    "zustand": vorhaben::zustandswort(&v, sprache),
                    "bericht": bericht,
                    "pruefung": pruefung,
                    // Als Text, siehe `Saatstand`.
                    "saat": saat.map(|s| s.to_string()),
                }),
                Ereignis::Wartet { bis } => serde_json::json!({
                    "art": "Wartet",
                    "minuten": bis.saturating_sub(vorhaben::jetzt()).div_ceil(60),
                }),
                Ereignis::Fehler { kennung, grund } => {
                    serde_json::json!({ "art": "Fehler", "kennung": kennung, "grund": grund })
                }
                Ereignis::OhneModell { grund } => serde_json::json!({ "art": "OhneModell", "grund": grund }),
            };
            let _ = f.emit(LOOPEREIGNIS, j);
        };
        let f2 = fenster.clone();
        let melder = move |m: myl_client::Meldung<'_>| {
            let _ = f2.emit(LOOPLEBEND, lebend_aus(m));
        };
        let verlauf = |v: &myl_local_agent::vorhaben::Vorhaben| umfeld(v).map(|u| u.verlauf).unwrap_or_default();
        vorhaben::fahren_mit_verlauf(
            &laeufer,
            &leihen,
            &ruester,
            &e.schleife,
            sprache,
            u32::try_from(e.modell.token).unwrap_or(u32::MAX),
            &melden,
            &|| false,
            Some(&melder),
            Some(&verlauf),
        );
        // ⚑ **Warum er endete, entscheidet, was aus dem Task wird**
        //   (2026-10-12): Angehalten (∞ aus, ⏸) und geschlossen heisst, der
        //   Task, der lief, ist angehalten und wartet auf ▶. Erst nach dem
        //   Ende des Laeufers, denn eine Runde schreibt bis dahin ihren
        //   Stand.
        let grund = if z.pause.load(Ordering::SeqCst) {
            "pausiert"
        } else if vorhaben::schliessen_angefordert() {
            "geschlossen"
        } else if myl_local_agent::notaus::ausgeloest() {
            "notaus"
        } else {
            "leer"
        };
        drop(laeufer);
        match grund {
            "pausiert" => {
                let _ = ablage.vom_menschen_anhalten(sprache);
            }
            "geschlossen" => {
                let _ = ablage.nach_dem_schliessen(sprache);
            }
            _ => {
                let _ = ablage.loop_aktiv_setzen(false);
            }
        }
        if grund != "notaus" {
            vorhaben::schliessen_zuruecksetzen();
        }
        z.in_runde.store(false, Ordering::SeqCst);
        z.laeuft.store(false, Ordering::SeqCst);
        let _ = fenster.emit(LOOPEREIGNIS, serde_json::json!({ "art": "Ende", "grund": grund }));
    });
    if let Ok(mut g) = zf.faden.lock() {
        *g = Some(griff.thread().id());
    }
    Ok(hinweis)
}

/// **∞ aus oder ⏸ am laufenden Task: der Loop haelt an.** Eine laufende
/// Runde haelt sofort an, der Task steht danach als angehalten da, und ▶
/// setzt ihn genau dort fort.
#[tauri::command]
fn loop_pausieren(halter: tauri::State<'_, Halter>) {
    use std::sync::atomic::Ordering;
    if halter.schleife.laeuft.load(Ordering::SeqCst) {
        halter.schleife.pause.store(true, Ordering::SeqCst);
        myl_local_agent::vorhaben::schliessen_anfordern();
        // ⚑ **Auch ein laufender Befehl endet sofort** (Projektinhaber,
        //   2026-09-29: „umgehend pausiert“), sonst liefe er bis zu seiner
        //   Frist. Nur die des Loop-Fadens; ein Chat daneben bleibt.
        if let Some(f) = halter.schleife.faden.lock().ok().and_then(|g| *g) {
            myl_senses::prozess::beenden_im_faden(f);
        }
    }
}

/// **Das Fenster geht zu**: anhalten, und kurz warten, damit der Faden den
/// Task, der lief, als angehalten ablegt (`nach_dem_schliessen`).
///
/// ⚠️ **Hoechstens vier Sekunden.** Steht die Runde gerade in einer
/// Nachfrage des Betriebssystems, kommt sie nicht heraus, solange der
/// Hauptfaden hier wartet; dann bleibt die Marke `.aktiv` stehen, und der
/// naechste Start haelt den Task an (`loop_nach_dem_start`).
fn loop_beim_schliessen(z: &Loopzustand) {
    use std::sync::atomic::Ordering;
    if !z.laeuft.load(Ordering::SeqCst) {
        return;
    }
    myl_local_agent::vorhaben::schliessen_anfordern();
    let bis = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while z.laeuft.load(Ordering::SeqCst) && std::time::Instant::now() < bis {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// ⛔️ **Der Notschalter des Fensters ist das Schliessen** (Festlegung
/// des Projektinhabers, 2026-09-28; so steht es auch im Hinweis beim
/// Start).
///
/// Mit dem Prozess enden die Erzeugung und jeder weitere Schritt von
/// selbst. **Ein Befehl, den der Agent schon gestartet hat, endet nicht
/// mit**: Er ist ein eigener Prozess und liefe ohne Frist weiter, denn
/// die Frist hielt dieses Programm. Deshalb hier, bevor der Prozess
/// geht: der Notaus und das Ende jedes laufenden Befehls.
///
/// ⚑ **Nach `loop_beim_schliessen` und nicht davor.** Jenes markiert das
/// Schliessen, und der Loop haelt dann nur an, statt seinen Task
/// anzuhalten; so macht er beim naechsten Oeffnen genau dort weiter.
///
/// ⚑ **Einmal, auch wenn zwei Ereignisse kommen**: Auf das Schliessen
/// folgt das Zerstoeren des Fensters, und das Protokoll soll einen
/// Eintrag tragen und nicht zwei.
fn notschalter_beim_schliessen() {
    static SCHON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if SCHON.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    myl_local_agent::notaus::ausloesen("fenster geschlossen");
    myl_senses::prozess::alle_beenden();
}

/// **Der Hinweis beim Start: Hier arbeitet eine KI.**
///
/// ⛔️ **Ohne Bedingung und ohne Schalter**, anders als die Warnung
/// darunter. Der Text kommt aus der Kiste, damit die Konsole denselben
/// sagt; die Sprache aus den Einstellungen, und ohne lesbare
/// Einstellungen Deutsch, denn ein fehlender Hinweis waere schlimmer als
/// einer in der falschen Sprache.
#[tauri::command]
fn starthinweis() -> myl_client::kennzeichnung::Starthinweis {
    let sprache = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .map(|e| e.oberflaeche.sprache)
        .unwrap_or_default();
    myl_client::kennzeichnung::starthinweis(sprache, myl_client::kennzeichnung::Flaeche::Fenster)
}

/// **Die Warnung, falls sie noch gezeigt werden soll.**
///
/// ⚑ `None` heisst: Der Nutzer hat zugestimmt und das Haekchen gesetzt.
/// Die Entscheidung faellt hier und nicht im Skript, damit sie an
/// derselben Einstellung haengt wie in der Konsole.
#[tauri::command]
fn agentenwarnung() -> Result<Option<Warnungsansicht>, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if !e.agent.warnung {
        return Ok(None);
    }
    let w = myl_client::warnung::warnung(e.oberflaeche.sprache);
    Ok(Some(Warnungsansicht {
        achtung: w.achtung.to_string(),
        kern: w.kern.to_string(),
        anleitung_titel: w.anleitung_titel.to_string(),
        regeln: w
            .regeln
            .iter()
            .map(|r| (r.regel.to_string(), r.grund.to_string()))
            .collect(),
        zustimmung: w.zustimmung.to_string(),
        nicht_wieder: w.nicht_wieder.to_string(),
    }))
}

/// Was der Agent anfassen darf.
#[derive(Serialize)]
struct Werkzeugliste {
    /// ⚑ `None` heisst: kein Verzeichnis eingehaengt, also gar keine
    /// Werkzeuge, und nicht „unbekannt".
    wurzel: Option<String>,
    /// Der Name der Werkzeugkiste, genau der des Ordners unter
    /// `AGENT_LAYER/local-toolkits` (`Base`, `Advanced`, `1337`).
    kiste: String,
    /// ⚑ **Wo die Kistenwahl aufgehen soll**: das Verzeichnis, in dem die
    /// Kisten nebeneinander liegen, also der Elternordner der geltenden
    /// Kiste. Daneben `kistenordner`, die geltende Kiste selbst: Sie
    /// steht in den Einstellungen als Platzhalter, damit ein leeres Feld
    /// nicht „keine Werkzeugkiste" behauptet. 📌 Ohne diese Angabe bekam der Dialog einen leeren Startort
    /// und ging dort auf, wo zuletzt etwas gewaehlt wurde, also beim
    /// Einhaengepfad. Wer eine Kiste waehlen will, soll die Kisten sehen.
    kistenheimat: Option<String>,
    /// Der Ordner, aus dem die Werkzeuge wirklich kommen.
    kistenordner: Option<String>,
    namen: Vec<String>,
    /// Das eingestellte Modul (`agent.modul`), und warum es nichts
    /// mitbringt, falls es die Pruefung nicht besteht.
    modul: Option<String>,
    modulgrund: Option<String>,
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
    /// ⚑ **Die laufende Aufnahme der Sprechtaste.**
    ///
    /// Sie muss **zwischen zwei Befehlen** leben: Der Knopf wird
    /// gedrueckt (Start) und losgelassen (Ende), und dazwischen liegt
    /// keine Funktion, in der sie stehen koennte. ⚠️ Wer das Fenster
    /// schliesst, waehrend sie laeuft, beendet sie mit; das erledigt
    /// `Drop`.
    aufnahme: std::sync::Arc<Mutex<Option<myl_senses::aufnahme::Aufnahme>>>,
    /// ⛔️ **Das Sprechmodell bleibt geladen, ueber Antworten hinweg.**
    ///
    /// CosyVoice braucht rund achtzehn Sekunden zum Laden. Ein Vorleser
    /// je Antwort legt diese Zeit **vor jede** Antwort; gemeldet vom
    /// Projektinhaber am 2026-09-18 („braucht sehr lange nach der
    /// Textgenerierung um zu antworten"). Hier lebt er so lange wie das
    /// Fenster, und die Ladezeit faellt genau einmal an.
    sprecher: myl_senses::sprechen::Geteilter,
    /// **Wo das Terminal gerade steht.**
    ///
    /// ⚑ **Im Fensterzustand und nicht je Befehl**, denn `cd` soll
    /// halten. Jeder Aufruf startet eine eigene Shell; ohne diesen Wert
    /// stuende jedes Kommando wieder im Ordner des ersten, und das
    /// waere ein Terminal, das sein Verzeichnis vergisst.
    terminalordner: std::sync::Arc<Mutex<Option<std::path::PathBuf>>>,
    /// **Der Loop dieses Fensters** (∞ neben dem Senden).
    schleife: Loopzustand,
    /// **Die Module dieses Fensters**, gestartet mit ihrem ersten Befehl.
    module: modulwirt::Fensterwirt,
    /// **Der Schlaf dieses Fensters** (`myl_local_agent::schlaf`).
    ruhe: Ruhe,
}

/// **Der Schlaf des Fensters**: sein Wecker, und ob er gerade laeuft.
///
/// ⚑ **Jeder Befehl, der das Modell braucht, weckt zuerst**
/// ([`Ruhe::wecken`]). Der Schlaf haelt das Modell nur fuer einen
/// Aufruf ([`GeliehenesModell`]) und haelt dessen Erzeugung am Wecker an;
/// ein Auftrag wartet also hoechstens bis zum naechsten Token.
#[derive(Default, Clone)]
struct Ruhe {
    wecker: myl_local_agent::schlaf::Wecker,
    laeuft: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Ruhe {
    fn wecken(&self) {
        self.wecker.wecken();
    }
}

/// **Das Modell des Fensters, je Aufruf geliehen.**
///
/// ⚑ **Nicht fuer die ganze Dauer**: Ein Schlaf, der das Schloss eine
/// Minute lang hielte, liesse jede Frage in dieser Minute warten. ⚑
/// **Gierig fuer die Dauer des Aufrufs**, damit dasselbe Material dieselben
/// Erinnerungen ergibt; danach gilt wieder, was eingestellt ist.
struct GeliehenesModell(std::sync::Arc<Mutex<Option<myl_client::Oertlichesmodell>>>);

impl myl_client::Modellweg for GeliehenesModell {
    fn chat(
        &self,
        modell: &str,
        nachrichten: &[myl_client::Nachricht],
        max_tokens: Option<u32>,
    ) -> Result<myl_local_agent::Antwort, myl_client::Tuerfehler> {
        self.chat_unterbrechbar(modell, nachrichten, max_tokens, &std::sync::atomic::AtomicBool::new(false))
    }

    fn chat_unterbrechbar(
        &self,
        modell: &str,
        nachrichten: &[myl_client::Nachricht],
        max_tokens: Option<u32>,
        abbruch: &std::sync::atomic::AtomicBool,
    ) -> Result<myl_local_agent::Antwort, myl_client::Tuerfehler> {
        let mut g = self
            .0
            .lock()
            .map_err(|_| myl_client::Tuerfehler::Unerreichbar { grund: "der Modellhalter ist vergiftet".into() })?;
        let m = g.as_mut().ok_or_else(|| myl_client::Tuerfehler::Unerreichbar { grund: "das Modell ist entladen".into() })?;
        // ⚑ Gierig und ohne Nachdenken, wie in der Konsole: Die Fragen des
        //   Schlafs haben kurze Tokengrenzen.
        let vorher = (m.gierig, m.denken);
        m.gierig = true;
        m.denken = false;
        let aus = m.chat_unterbrechbar(modell, nachrichten, max_tokens, abbruch);
        (m.gierig, m.denken) = vorher;
        aus
    }
}

/// Ein Gespraech, wie das Fenster es dem Schlaf reicht.
#[derive(serde::Deserialize)]
struct Ruhegespraech {
    kennung: String,
    verlauf: Vec<myl_client::Nachricht>,
    wurzel: Option<String>,
}

/// **Der Schlaf, gerufen, wenn das Fenster eine Weile nichts tut.**
/// Gibt die Zeile des Berichts zurueck, oder `None`, wenn er nicht lief
/// (aus, laeuft schon, oder das Modell ist gerade beschaeftigt).
///
/// ⚑ **Die Gespraeche reicht das Fenster**: Sie liegen in seinem Speicher
///   und nicht im Ruecken (siehe `Umfeld`). Je Arbeitsordner ein Durchgang;
///   ohne eigenen Ordner gilt der eingestellte.
/// ⚠️ **Ohne Vorwaermen**: Im Fenster geht das Modell nach der Ruhefrist
///   ohnehin aus dem Speicher, und ein Vorwaermen davor waere verloren.
#[tauri::command]
async fn schlafen(gespraeche: Vec<Ruhegespraech>, halter: tauri::State<'_, Halter>) -> Result<Option<String>, String> {
    use std::sync::atomic::Ordering;
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if !e.schlaf.an || halter.schleife.in_runde.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let ruhe = halter.ruhe.clone();
    if ruhe.laeuft.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }
    ruhe.wecker.zuruecksetzen();
    // ⚑ Wer das Modell braucht, weckt den Schlaf zuerst.
    halter.ruhe.wecken();
    let modell = halter.modell.clone();
    // ⚑ **`try_lock`**: Haelt jemand das Modell, ist das Fenster nicht in
    //   Ruhe, und der Schlaf kommt beim naechsten Mal.
    let geladen = match modell.try_lock() {
        Ok(g) => g.is_some(),
        Err(_) => {
            ruhe.laeuft.store(false, Ordering::SeqCst);
            return Ok(None);
        }
    };
    let rechnen = geladen && e.schlaf.darf_rechnen();
    let deutsch = e.oberflaeche.sprache == myl_client::einstellungen::Sprache::De;
    let vorgabe = e.agent.wurzel.clone().or_else(myl_client::Einstellungen::standard_wurzel);
    let r = ruhe.clone();
    let aus = tauri::async_runtime::spawn_blocking(move || {
        let mut je: std::collections::BTreeMap<String, Vec<myl_local_agent::schlaf::Episode>> = Default::default();
        for g in gespraeche {
            let Some(w) = g.wurzel.filter(|w| !w.trim().is_empty()).or_else(|| vorgabe.clone()) else { continue };
            je.entry(w).or_default().push(myl_local_agent::schlaf::episode_aus_gespraech(&g.kennung, &g.verlauf));
        }
        if let (true, Some(v)) = (je.is_empty(), vorgabe) {
            je.insert(v, Vec::new());
        }
        let weg = GeliehenesModell(modell);
        let ablage = myl_local_agent::vorhaben::Ablage::vorgabe();
        let mut zeilen = Vec::new();
        for (i, (w, episoden)) in je.into_iter().enumerate() {
            if r.wecker.geweckt() {
                break;
            }
            let wurzel = std::path::PathBuf::from(w);
            let auftrag = myl_local_agent::schlaf::Ruheauftrag {
                wurzel: &wurzel,
                gespraeche: episoden,
                // Die Vorhaben sind eine Ablage fuer alle Ordner: einmal.
                vorhaben: (i == 0).then_some(&ablage),
                vorwaermen: None,
                deutsch,
                tag: myl_local_agent::gedaechtnis::heute(),
            };
            let b = myl_local_agent::schlaf::schlafen(rechnen.then_some(&weg as &dyn myl_client::Modellweg), &auftrag, &r.wecker);
            zeilen.push(b.zeile(deutsch));
        }
        zeilen.join(" ")
    })
    .await;
    ruhe.laeuft.store(false, Ordering::SeqCst);
    aus.map(Some).map_err(|f| format!("der Schlaf ist abgestuerzt: {f}"))
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
    myl_client::modelle::anzeigename(pfad)
}

/// Laedt das Artefakt aus den Einstellungen.
#[tauri::command]
async fn modell_laden(halter: tauri::State<'_, Halter>) -> Result<Ladung, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let anfang = std::time::Instant::now();
    let m = modell_aus(&e)?;
    let dauer = anfang.elapsed().as_secs_f64();
    *halter.modell.lock().map_err(|_| "der Modellhalter ist vergiftet")? = Some(m);
    halter.module.modell_melden(modulwirt::modellkennung(&e.modell.artefakt));
    Ok(Ladung {
        name: anzeigename(&e.modell.artefakt),
        pfad: e.modell.artefakt.clone(),
        sekunden: (dauer * 10.0).round() / 10.0,
    })
}

/// Was beim Start geladen wurde, und ob die Wahl eine Empfehlung war.
#[derive(Serialize)]
struct Vorladung {
    name: String,
    pfad: String,
    sekunden: f64,
    /// `true`: kein Modell war eingestellt, die Empfehlung wurde gesetzt.
    empfohlen: bool,
    grund: String,
}

/// **Beim Start das passende Modell ermitteln und schon laden** (Wunsch des
/// Projektinhabers, 2026-09-29), waehrend das Vorschaltbild laeuft.
///
/// ⚑ **Eine eingestellte Wahl gilt.** Liegt das eingestellte Artefakt da,
/// wird genau das geladen; sonst die Empfehlung fuer diese Maschine
/// (`modelle::empfehlung`), und sie wird eingestellt, damit Wahl und
/// Anzeige dasselbe sagen. Ist schon ein Modell geladen, geschieht nichts.
/// Geladen wird in einem eigenen Faden, damit das Fenster weiter antwortet.
#[tauri::command]
async fn modell_vorladen(halter: tauri::State<'_, Halter>) -> Result<Option<Vorladung>, String> {
    if halter.modell.lock().map_err(|_| "der Modellhalter ist vergiftet")?.is_some() {
        return Ok(None);
    }
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let mut e = myl_client::Einstellungen::lesen(&pfad)?;
    let eingestellt = !e.modell.artefakt.is_empty()
        && std::path::Path::new(&artefakt_absolut(&e.modell.artefakt)).join("model_config.json").is_file();
    let (empfohlen, grund) = if eingestellt {
        (false, "eingestellt".to_string())
    } else {
        let Some(w) = myl_client::modelle::empfehlung(&e) else {
            return Ok(None);
        };
        e.setzen("modell.artefakt", &w.pfad)?;
        e.schreiben(&pfad)?;
        (true, w.grund)
    };
    halter.ruhe.wecken();
    let halt = halter.modell.clone();
    let e2 = e.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // ⚑ **Ein eigener Faden, nicht der aus dem Vorrat.** Er wird hinter
        // das Vorschaltbild gestellt, damit es nicht ruckelt, und diesen
        // Vorrang behielte ein Vorratsfaden; der naechste Agentenlauf auf
        // ihm rechnete dann gebremst. Unter Linux laesst sich der Nice-Wert
        // ohne Rechte nicht zuruecknehmen, also stirbt der Faden mit ihm.
        std::thread::spawn(move || {
            myl_client::hardware::im_hintergrund_rechnen();
            let anfang = std::time::Instant::now();
            let m = modell_aus(&e2)?;
            let dauer = anfang.elapsed().as_secs_f64();
            let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
            // Wer inzwischen von Hand geladen hat, behaelt sein Modell.
            if g.is_none() {
                *g = Some(m);
            }
            Ok::<f64, String>(dauer)
        })
        .join()
        .map_err(|_| "das Vorladen ist abgestuerzt".to_string())?
    })
    .await
    .map_err(|f| f.to_string())?
    .map(|dauer| {
        Some(Vorladung {
            name: anzeigename(&e.modell.artefakt),
            pfad: e.modell.artefakt.clone(),
            sekunden: (dauer * 10.0).round() / 10.0,
            empfohlen,
            grund,
        })
    })
}

/// **Laedt das eingestellte Artefakt**, fuer den Ladeknopf und fuer die
/// Leihe des Loops. ⚑ Eine Stelle, damit beide dasselbe Modell mit
/// derselben Grenze bekommen.
fn modell_aus(e: &myl_client::Einstellungen) -> Result<myl_client::Oertlichesmodell, String> {
    if e.modell.artefakt.is_empty() {
        return Err("es ist kein Artefakt eingestellt".into());
    }
    let pfad = artefakt_absolut(&e.modell.artefakt);
    let mut m = myl_client::Oertlichesmodell::laden(&pfad, &e.kapazitaet)
        .map_err(|f| mit_zugriffshinweis(f, &pfad))?;
    // Laenge, Denkmodus und, falls gesetzt, die Saat.
    m.uebernehmen(&e.modell);
    Ok(m)
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
/// 📌 **Ohne geladenes Modell ist das kein Fehler.** Ein Entladen, das
/// sich beschwert, wenn nichts da ist, zwingt jeden Aufrufer, vorher zu
/// fragen; die Zeitschaltung tut das nicht und soll es nicht muessen.
/// **Wo die Saat gerade steht**, fuer das Feld am Eingabefeld und die
/// Zeile unter einer Antwort.
#[derive(Serialize, Clone, Default)]
struct Saatstand {
    /// Die Saat der letzten Aktion, falls eine lief und gezogen wurde.
    ///
    /// ⚠️ **Saaten als Text**: JavaScript stellt ganze Zahlen nur bis 2^53
    /// genau dar, und eine gerundete Saat wiederholte still etwas anderes.
    letzte: Option<String>,
    /// Die feste Saat aus den Einstellungen, oder `None` fuer Zufall.
    fest: Option<String>,
    /// Die Saat, die nur fuer die naechste Antwort vorgemerkt ist.
    naechste: Option<String>,
    /// Temperatur, Top-p und Top-k in einer Zeile, oder „gierig“.
    parameter: String,
    gierig: bool,
    /// Das Modell rechnet gerade: Was jetzt gesetzt wird, gilt ab der
    /// naechsten Antwort oder Runde.
    belegt: bool,
}

/// ⚑ **Das Feld am Eingabefeld wartet nie auf das Modell** (Wunsch des
/// Projektinhabers, 2026-10-13). Die Befehle dazu laufen auf dem Hauptfaden;
/// ein wartendes `lock` fror das ganze Fenster ein, solange eine Antwort,
/// ein Auftrag oder eine Runde des Loops rechnete. Jetzt: `try_lock`, und
/// ist das Modell belegt, kommt der Stand aus dem zuletzt gesehenen
/// ([`SAATSPIEGEL`]), und eine Saat fuer die naechste Antwort wird
/// vorgemerkt ([`SAAT_VORGEMERKT`]) und vor der naechsten Antwort oder Runde
/// uebernommen ([`ausgabe_uebernehmen`]). Die uebrigen Werte (Temperatur,
/// Top-p, Top-k, Laenge, Denken, feste Saat) stehen in den Einstellungen,
/// und die liest jeder Lauf ohnehin frisch.
static SAAT_VORGEMERKT: Mutex<Option<u64>> = Mutex::new(None);
/// Der Stand, wie er zuletzt bei freiem Modell gelesen wurde.
static SAATSPIEGEL: Mutex<Option<Saatstand>> = Mutex::new(None);

fn saat_vorgemerkt() -> Option<u64> {
    *SAAT_VORGEMERKT.lock().unwrap_or_else(|v| v.into_inner())
}

/// **Der Stand der Saat, ohne zu warten.**
fn saatstand_aus(modell: &Mutex<Option<myl_client::Oertlichesmodell>>, e: &myl_client::Einstellungen) -> Result<Saatstand, String> {
    let mut g = match modell.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::WouldBlock) => {
            let spiegel = SAATSPIEGEL.lock().unwrap_or_else(|v| v.into_inner()).clone().unwrap_or_default();
            return Ok(Saatstand {
                fest: e.modell.saat.map(|s| s.to_string()),
                naechste: saat_vorgemerkt().map(|s| s.to_string()),
                gierig: e.modell.temperatur == Some(0),
                belegt: true,
                ..spiegel
            });
        }
        Err(std::sync::TryLockError::Poisoned(_)) => return Err("der Modellhalter ist vergiftet".to_string()),
    };
    let stand = match g.as_mut() {
        Some(m) => {
            ausgabe_uebernehmen(m, e);
            Saatstand {
                letzte: if m.waehlt_gierig() { None } else { m.letzte_saat().map(|s| s.to_string()) },
                fest: m.saat_fest.map(|s| s.to_string()),
                naechste: m.naechste().map(|s| s.to_string()),
                parameter: m.parameterzeile(),
                gierig: m.waehlt_gierig(),
                belegt: false,
            }
        }
        None => Saatstand {
            letzte: None,
            fest: e.modell.saat.map(|s| s.to_string()),
            naechste: saat_vorgemerkt().map(|s| s.to_string()),
            parameter: String::new(),
            gierig: e.modell.temperatur == Some(0),
            belegt: false,
        },
    };
    *SAATSPIEGEL.lock().unwrap_or_else(|v| v.into_inner()) = Some(stand.clone());
    Ok(stand)
}

/// **Eine Saat fuer die naechste Antwort, ohne zu warten**: bei freiem
/// Modell sofort dort, sonst vorgemerkt. Ohne geladenes Modell ebenfalls
/// vorgemerkt; sie gilt dann fuer die erste Antwort nach dem Laden.
fn saat_einmal_in(modell: &Mutex<Option<myl_client::Oertlichesmodell>>, saat: u64) -> Result<(), String> {
    match modell.try_lock() {
        Ok(g) => match g.as_ref() {
            Some(m) => m.naechste_saat(saat),
            None => *SAAT_VORGEMERKT.lock().unwrap_or_else(|v| v.into_inner()) = Some(saat),
        },
        Err(std::sync::TryLockError::WouldBlock) => *SAAT_VORGEMERKT.lock().unwrap_or_else(|v| v.into_inner()) = Some(saat),
        Err(std::sync::TryLockError::Poisoned(_)) => return Err("der Modellhalter ist vergiftet".to_string()),
    }
    Ok(())
}

/// **Der Stand der Saat** (Regel des Projektinhabers, 2026-09-29: der
/// Seed kommt ins Protokoll und laesst sich anzeigen und einstellen).
#[tauri::command]
fn saat_stand(halter: tauri::State<'_, Halter>) -> Result<Saatstand, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    saatstand_aus(&halter.modell, &e)
}

/// **Eine Saat nur fuer die naechste Antwort**; danach gilt wieder die
/// Einstellung. „Immer“ schreibt das Fenster ueber `setzen` in
/// `modell.saat`, dieselbe Stelle wie die Einstellungsseite.
#[tauri::command]
fn saat_einmal(saat: String, halter: tauri::State<'_, Halter>) -> Result<(), String> {
    let saat: u64 = saat.trim().parse().map_err(|_| format!("{saat} ist keine Saat"))?;
    saat_einmal_in(&halter.modell, saat)
}

/// **Saat und Ziehparameter aus den Einstellungen**, frisch vor jedem
/// Auftrag: Das Feld am Eingabefeld schreibt sie, waehrend das Modell
/// geladen bleibt. Laenge und Denkmodus setzt jeder Weg selbst.
fn ausgabe_uebernehmen(m: &mut myl_client::Oertlichesmodell, e: &myl_client::Einstellungen) {
    m.saat_fest = e.modell.saat;
    m.temperatur = e.modell.temperatur;
    m.top_p = e.modell.top_p;
    m.top_k = e.modell.top_k;
    // Der Arbeitskontext gilt ebenso ab dem naechsten Lauf.
    m.kontextbudget = e.modell.kontext;
    // Eine Saat, die vorgemerkt wurde, waehrend das Modell rechnete.
    if let Some(saat) = SAAT_VORGEMERKT.lock().unwrap_or_else(|v| v.into_inner()).take() {
        m.naechste_saat(saat);
    }
}

#[tauri::command]
fn modell_entladen(halter: tauri::State<'_, Halter>) -> Result<bool, String> {
    // ⚠️ **`try_lock` und nicht `lock`**: Dieser Befehl laeuft auf dem
    //    Hauptfaden. Rechnet gerade eine Runde des Loops, haelt sie das
    //    Modell, und ein wartendes `lock` fror das ganze Fenster ein, bis
    //    sie fertig ist. Dann bleibt das Modell eben geladen.
    let mut g = match halter.modell.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
        Err(std::sync::TryLockError::Poisoned(_)) => return Err("der Modellhalter ist vergiftet".into()),
    };
    let war = g.take().is_some();
    if war {
        halter.module.modell_melden(None);
    }
    Ok(war)
}

/// Faehrt einen Auftrag und meldet den Verlauf ans Fenster.
///
/// 📌 **`async` allein genuegt nicht.** Der Lauf **rechnet**, er wartet
/// nicht; in einem `async`-Befehl belegte er den Laufzeitfaden fuer
/// Minuten und damit alles andere. `spawn_blocking` gibt ihm einen
/// eigenen.
#[tauri::command]
async fn agent_fahren(
    auftrag: String,
    // ⚑ **Das bisherige Gespraech, ohne Werkzeugansage** (Entscheidung C2,
    // beantwortet am 2026-09-14). Fehlt es, steht der Auftrag fuer sich
    // wie vorher.
    verlauf: Option<Vec<myl_client::Nachricht>>,
    // ⚑ **Der Einhaengepfad dieses Prozesses**, falls er einen eigenen
    // hat (Auftrag des Projektinhabers, 2026-09-15). Ohne Angabe die
    // Einstellung, ohne die `WORK_DIR`.
    wurzel: Option<String>,
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<Abschluss, String> {
    let mut e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    // ⚑ **Auf einer Kopie und nicht in der Ablage**, genau wie die
    // Konsole ihr Startverzeichnis setzt: Der Pfad gehoert diesem
    // Prozess, nicht dem Programm. Ihn zu speichern hiesse, dass der
    // naechste Prozess ihn erbt, und das ist das Gegenteil von „je
    // Prozess".
    if let Some(w) = wurzel.map(|w| w.trim().to_string()).filter(|w| !w.is_empty()) {
        e.agent.wurzel = Some(w);
    }
    // ⚑ **Im `manual mode` fragt das Fenster jetzt, statt wegzunehmen**
    // (Meldung des Projektinhabers, 2026-09-15: das Modell zaehlte nur
    // drei Werkzeuge auf). Bis hierher setzte `im_modus` in diesem Modus
    // `schreiben = false`, weil es keinen Bestaetigungskasten gab. Das
    // nahm nicht nur `write_file`, `edit_file` und `run_command` weg,
    // sondern **auch jedes Kisten-Werkzeug**: Ein Manifest laeuft ueber
    // die Shell, gilt deshalb als schreibend und faellt mit. Wer seine
    // Werkzeugkiste fuellte, sah davon im manual mode nichts.
    //
    // ⚑ **Der Kasten ist der des Betriebssystems**, derselbe Weg wie bei
    // der Ordnerwahl: blockierend, und das ist hier erlaubt, weil der
    // ganze Lauf in `spawn_blocking` liegt und nicht auf dem Hauptfaden.
    // ⛔️ **Der Schutzfilter vor allem anderen** (Art. 5 KI-Verordnung):
    //   Ein Auftrag, der erkennbar auf eine verbotene Praxis zielt, wird
    //   gar nicht erst gefahren. Das Gespraech bleibt, mit der Abweisung.
    if let Some(satz) = myl_client::schutzfilter::abweisen(&auftrag, e.oberflaeche.sprache, "fenster-agent") {
        let mut nachrichten = verlauf.clone().unwrap_or_default();
        nachrichten.push(myl_client::Nachricht::nutzer(auftrag.clone()));
        nachrichten.push(myl_client::Nachricht::modell(satz.clone()));
        return Ok(Abschluss {
            verlauf: Vec::new(),
            antwort: Some(satz),
            fertig: true,
            im_kreis: false,
            sekunden: 0.0,
            gesperrt: false,
            nachrichten,
            kontext: None,
            zusammenfassung: None,
        });
    }
    // ⚑ Das eingestellte Modul ergaenzt Kiste und Skills.
    let (ohne, _modulgrund) = modul_anwenden(&mut e);
    let nachfrage = nachfrage_ohne(&fenster, e.agent.modus, ohne);
    // ⚑ Der Auftrag ist die Saat der Web-Recherche, wie im Chat.
    e.agent.netzsaat = Some(auftrag.clone());
    let ruestung = myl_local_agent::ruestung::ruesten_mit(
        &e.agent,
        myl_client::Ansageform::Amtlich,
        kiste_fuer(&e),
        Vec::new(),
        nachfrage,
    )?;
    // ⚑ **Die Dateiwerkzeuge laufen, wenn ein Verzeichnis eingehaengt
    // ist.** Bis zum 2026-09-11 stand hier ein zweiter Schalter
    // (`agent.bezeugtes`), ohne den sie zwar in der Ansage standen und
    // nicht liefen. **Zwei Erlaubnisse fuer dieselbe Sache sind eine zu
    // viel:** Das Einhaengen ist die Zustimmung, `agent.schreiben`
    // entscheidet ueber das Schreiben, und die Werkzeugkiste sagt,
    // welche Werkzeuge es gibt.
    let gesperrt = false;
    let schritte = e.agent.schrittgrenze();
    let grenze = e.modell.token as u32;

    // ⛔️ Jeder Auftrag beginnt mit einem gelösten Notaus.
    myl_local_agent::notaus::zuruecksetzen();
    let _ = fenster.emit("agent-beginnt", &auftrag);
    halter.ruhe.wecken();
    let halt = halter.modell.clone();
    let aus = tauri::async_runtime::spawn_blocking(move || {
        let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_mut() else {
            return Err("das Modell ist nicht geladen".to_string());
        };
        if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
            ausgabe_uebernehmen(m, &e);
        }
        zusehen(m, &fenster);
        // ⚑ **Zwei Quellen, ein Kanal.** Der laufende Text kommt vom
        // Modell, die Werkzeuge von der Schleife; das Fenster soll
        // beides in einer Reihenfolge sehen und nicht aus zwei
        // Stroemen zusammensetzen muessen.
        let f = fenster.clone();
        let melder = move |m: myl_client::Meldung<'_>| {
            let _ = f.emit(LEBEND, lebend_aus(m));
        };
        let verlauf = verlauf.unwrap_or_default();
        // ⚑ **Jeder Auftrag bekommt sein Nachschlagebudget neu**
        // (2026-09-17): Die naechste Nachricht ist ein neuer Anlass.
        ruestung.nachschlagebudget_zuruecksetzen();
        let ergebnis = myl_local_agent::lauf::fahren_im_gespraech(
            m, &ruestung, schritte, !gesperrt, grenze, &verlauf, &auftrag, Some(&melder),
        );
        // 📌 Siehe `zusehen`: Der Beobachter geht wieder ab, sonst
        // meldete dieser Lauf in den naechsten hinein.
        m.beobachter = None;
        let gespraech = myl_local_agent::gespraech::Gespraech::aus(ergebnis.nachrichten.clone());
        let ansage = myl_local_agent::gespraech::ansage(&ruestung);
        let kontext = myl_local_agent::gespraech::anzeige(&*m, Some(&ansage), &gespraech);
        Ok((ergebnis, gespraech, kontext))
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;
    let (aus, gespraech, kontext) = aus;
    Ok(Abschluss {
        verlauf: aus.verlauf.iter().map(zeile_aus).collect(),
        antwort: aus.antwort.clone(),
        fertig: aus.fertig(),
        im_kreis: matches!(aus.ende, myl_client::Ende::Kreislauf { .. }),
        sekunden: (aus.sekunden * 10.0).round() / 10.0,
        gesperrt,
        nachrichten: gespraech.nachrichten().to_vec(),
        zusammenfassung: myl_local_agent::gespraech::zusammenfassung(&gespraech),
        kontext,
    })
}

/// **Wie viel Kontext ein Gespraech belegt**, fuer den Balken am
/// Eingabefeld.
///
/// ⚑ Im Agentenmodus zaehlt die Werkzeugansage mit, denn sie steht vor
/// jedem Lauf; im Chat gibt es keine.
#[tauri::command]
async fn kontext(
    verlauf: Vec<myl_client::Nachricht>,
    modus: String,
    // ⚑ **Der Einhaengepfad dieses Prozesses** (Auftrag des
    // Projektinhabers, 2026-09-15). Er gehoert hierher, weil die
    // Werkzeugansage am eingehaengten Ordner haengt: Ein Prozess ohne
    // Ordner bekommt weniger Werkzeuge als einer mit, und eine
    // Kontextanzeige, die eine andere Ansage zaehlt als die, die laeuft,
    // zaehlt falsch.
    wurzel: Option<String>,
    halter: tauri::State<'_, Halter>,
) -> Result<Option<myl_local_agent::gespraech::Kontextanzeige>, String> {
    let mut e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if let Some(w) = wurzel.map(|w| w.trim().to_string()).filter(|w| !w.is_empty()) {
        e.agent.wurzel = Some(w);
    }
    let ansage = if modus == "agent" {
        // ⚑ Gezaehlt wird die Ansage, die laeuft, also mit dem Modul.
        let _ = modul_anwenden(&mut e);
        let r = myl_local_agent::ruestung::ruesten(
            &e.agent,
            myl_client::Ansageform::Amtlich,
            kiste_fuer(&e),
            Vec::new(),
        )?;
        Some(myl_local_agent::gespraech::ansage(&r))
    } else {
        None
    };
    halter.ruhe.wecken();
    let halt = halter.modell.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_ref() else { return Ok(None) };
        let gespraech = myl_local_agent::gespraech::Gespraech::aus(verlauf);
        Ok(myl_local_agent::gespraech::anzeige(m, ansage.as_ref(), &gespraech))
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))?
}

/// Was eine Verdichtung zurueckbringt.
#[derive(Serialize)]
struct Verdichtung {
    /// Das Gespraech danach: eine Zusammenfassung.
    nachrichten: Vec<myl_client::Nachricht>,
    /// Dieselbe als Text, zum Ablegen im Fenster.
    zusammenfassung: Option<String>,
    vorher: usize,
    nachher: usize,
}

/// **Fasst ein Gespraech zusammen**, mit dem geladenen Modell selbst.
///
/// ⚑ Dieselbe Stelle wie `/compress` in der Konsole,
/// `myl_local_agent::gespraech::verdichten`.
#[tauri::command]
async fn verdichten(
    verlauf: Vec<myl_client::Nachricht>,
    sitzung: String,
    halter: tauri::State<'_, Halter>,
) -> Result<Verdichtung, String> {
    halter.ruhe.wecken();
    let halt = halter.modell.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_ref() else {
            return Err("das Modell ist nicht geladen".to_string());
        };
        let mut gespraech = myl_local_agent::gespraech::Gespraech::aus(verlauf);
        // ⚑ **Der Mitschnitt entsteht genau hier** (2026-09-16), im
        // Augenblick, in dem die Urfassung noch da ist. Der Ordner ist
        // der eingehaengte; ohne einen wird nichts abgelegt, und das ist
        // kein Fehler, sondern die Lage.
        let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok();
        let wurzel = e
            .as_ref()
            .and_then(|e| e.agent.wurzel.clone())
            .or_else(myl_client::Einstellungen::standard_wurzel)
            .map(std::path::PathBuf::from);
        let modellname = e.as_ref().map(|e| e.modell.artefakt.clone()).unwrap_or_default();
        // ⚑ Ob der Verweis `read_history` nennen darf, entscheidet die Kiste.
        let nachlesen = e.as_ref().map(kiste_fuer).is_none_or(|k| k.kann_nachlesen());
        let (vorher, nachher) = myl_local_agent::gespraech::verdichten_mit_mitschnitt(
            m,
            &mut gespraech,
            wurzel.as_deref(),
            &sitzung,
            &modellname,
            nachlesen,
        )
        .map_err(|f| f.to_string())?;
        Ok(Verdichtung {
            nachrichten: gespraech.nachrichten().to_vec(),
            zusammenfassung: myl_local_agent::gespraech::zusammenfassung(&gespraech),
            vorher,
            nachher,
        })
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))?
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
/// | Gespraech | **traegt den Verlauf mit** | **traegt ihn ebenfalls mit** (seit 2026-09-14) |
/// | Belege | keine | Schrittbudget und Belegkette je Auftrag |
///
/// ⚑ **Der Verlauf wird mitgegeben, weil das Modell ihn tragen kann.**
/// `chat` nimmt eine Nachrichtenliste; ein Fenster, das nur die letzte
/// Frage schickt, waere ein Chatfenster ohne Gespraech.
///
/// 📌 **Beim Agenten ging das bis zum 2026-09-14 ausdruecklich nicht**
/// (Entscheidung C2). Seitdem traegt auch er das Gespraech mit, und der
/// Unterschied der Betriebsarten bleibt dort, wo er hingehoert: bei den
/// Werkzeugen und den Belegen. Schrittbudget und Belegkette gelten
/// weiter je Auftrag; die Begruendung steht an
/// `myl_local_agent::schleife::Lauf::fahren_mit_verlauf`.
/// **Name und Inhaltsabdruck jeder Datei eines Ordners.**
///
/// ⚑ **Der Abdruck und nicht die Uhrzeit.** Eine Aenderungszeit sagt
/// „angefasst", nicht „anders": Ein Werkzeug, das dieselben Bytes
/// zurueckschreibt, setzte sie neu, und der Mensch bekaeme eine Datei
/// angeboten, an der nichts geschehen ist.
///
/// ⚠️ **Nur die oberste Ebene.** Der Anhangordner ist flach; wer dort
/// Unterordner anlegt, tut etwas, das kein Anhang ist.
fn stand_des_ordners(ordner: &std::path::Path) -> std::collections::BTreeMap<String, u64> {
    let mut aus = std::collections::BTreeMap::new();
    let Ok(eintraege) = std::fs::read_dir(ordner) else { return aus };
    for e in eintraege.flatten() {
        if !e.path().is_file() {
            continue;
        }
        let Ok(inhalt) = std::fs::read(e.path()) else { continue };
        let name = e.file_name().to_string_lossy().to_string();
        // ⚑ **Ein Abdruck und keine Pruefsumme mit Zusage.** Hier wird
        //   verglichen, nicht bezeugt; was zaehlt, ist „gleich oder
        //   nicht" innerhalb **eines** Programmlaufs.
        //
        // ⚠️ **`DefaultHasher` ist genau dafuer erlaubt und sonst
        //   nicht.** Sein Verfahren ist ausdruecklich nicht festgelegt
        //   und darf sich zwischen Rust-Fassungen aendern; ein Wert,
        //   der abgelegt oder zwischen Maschinen verglichen wird,
        //   gehoert deshalb nach SHA-256 (siehe `generate_mit_digest`).
        //   Hier lebt er von einer Zeile zur naechsten.
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        inhalt.hash(&mut h);
        aus.insert(name, h.finish());
    }
    aus
}

/// **Was zwischen zwei Staenden dazukam oder sich aenderte.**
fn was_sich_geaendert_hat(
    vorher: &std::collections::BTreeMap<String, u64>,
    nachher: &std::collections::BTreeMap<String, u64>,
) -> Vec<String> {
    nachher
        .iter()
        .filter(|(name, abdruck)| vorher.get(*name) != Some(abdruck))
        .map(|(name, _)| name.clone())
        .collect()
}

#[tauri::command]
async fn frage(
    verlauf: Vec<(String, String)>,
    sprechen: Option<bool>,
    // ⚑ **Die Anhänge dieses Beitrags** (Auftrag des Projektinhabers,
    //   2026-09-23). Liegt einer an, bekommt der Chat Werkzeuge, und
    //   zwar **nur für die Anhänge**: lesen, ändern, ansehen, anhören.
    //   Ordner und Dateisystem bleiben draussen, und die Grenze zieht
    //   die Einhängung, nicht eine Absprache.
    //
    // ⚠️ **Ohne Anhang und ohne Recherche bleibt der Chat, was er
    //   war.** Werkzeuge kosten Ansage und damit Kontext; für ein
    //   Gespräch ohne Datei und ohne Netz gibt es nichts zu bedienen.
    anhaenge: Option<Vec<String>>,
    wurzel: Option<String>,
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<Antwort, String> {
    // ⛔️ Jeder Auftrag beginnt mit einem gelösten Notaus.
    myl_local_agent::notaus::zuruecksetzen();
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let grenze = e.modell.token as u32;
    // Fuer die Saetze, die das Nachdenken ueberbruecken.
    let sprache = e.oberflaeche.sprache.kennung().to_string();
    if verlauf.is_empty() {
        return Err("es wurde nichts gefragt".to_string());
    }
    // ⛔️ **Der Schutzfilter vor allem anderen** (Art. 5 KI-Verordnung),
    //   auf die juengste Frage des Menschen.
    if let Some((_, frage)) = verlauf.iter().rev().find(|(rolle, _)| rolle != "modell") {
        if let Some(satz) = myl_client::schutzfilter::abweisen(frage, e.oberflaeche.sprache, "fenster-chat") {
            return Ok(Antwort { text: satz, sekunden: 0.0, kontext: None, geaendert: Vec::new() });
        }
    }
    let anfang = std::time::Instant::now();

    // ⛔️ **Mit Anhang geht der Chat durch die Werkzeugschleife.**
    //
    // ⚑ **Die Rüstung sieht dabei nur den Anhangordner**, siehe
    //   `ruestung::ruesten_fuer_anhaenge`: fünf Dateiwerkzeuge auf
    //   dieser einen Wurzel, die beiden verankerten und die beiden
    //   Sinne. Kein Manifest, kein `run_command`, kein Weg hinaus.
    //
    // ⚠️ **Vorgelesen wird dann nicht**, und das ist kein Versehen: Im
    //   Strom der Schleife stehen auch Werkzeugaufrufe, und die will
    //   niemand vorgelesen bekommen. Genau mit dieser Begründung war
    //   das Sprechen bisher dem Chat vorbehalten.
    let mit_anhang = anhaenge.as_ref().is_some_and(|a| !a.is_empty());
    // ⚑ **Die Recherche schickt den Chat auch ohne Anhang durch die
    //   Schleife.** Ein Suchwerkzeug, das nur zusammen mit einer
    //   angehängten Datei da wäre, wäre genau dann weg, wenn man es
    //   braucht.
    let mit_netz = e.agent.web_recherche && myl_local_agent::netzwerkzeuge::curl_vorhanden();
    if mit_anhang || mit_netz {
        let ordner = wurzel
            .map(|w| w.trim().to_string())
            .filter(|w| !w.is_empty())
            .or_else(|| e.agent.wurzel.clone())
            .or_else(myl_client::Einstellungen::standard_wurzel)
            .ok_or_else(|| {
                "Es ist kein Arbeitsordner gesetzt. Ohne ihn gibt es keinen Ort fuer den Anhang."
                    .to_string()
            })?;
        let anhangordner =
            std::path::Path::new(&ordner).join(myl_client::anhang::unterordner());
        // ⚠️ **Ohne Anhang gibt es den Ordner noch nicht.** Die
        //   Einhängung braucht ihn trotzdem, sonst hängt kein einziges
        //   Werkzeug, auch nicht das Suchwerkzeug.
        std::fs::create_dir_all(&anhangordner)
            .map_err(|f| format!("der Anhangordner liess sich nicht anlegen: {f}"))?;
        // ⚑ **Nur der letzte Beitrag des Nutzers ist die Saat**, nicht
        //   der ganze Verlauf: In ihm steht auch, was frühere Seiten
        //   geschrieben haben, und genau das soll den Zielkreis nicht
        //   erweitern.
        let saat = verlauf
            .iter()
            .rev()
            .find(|(rolle, _)| rolle != "modell")
            .map(|(_, text)| text.clone())
            .unwrap_or_default();
        // ⛔️ **Auch hier wird im `manual mode` gefragt** (Art. 14 als
        //   Vorbild, Festlegung des Projektinhabers, 2026-09-25): ein
        //   geaenderter Anhang und jede Web-Anfrage. 📌 Bis dahin stand
        //   hier `None`, und der Chat mit Anhang oder Recherche fragte nie.
        let ruestung = myl_local_agent::ruestung::ruesten_fuer_anhaenge(
            &anhangordner,
            myl_client::Ansageform::Amtlich,
            mit_netz.then_some(saat.as_str()),
            Vec::new(),
            nachfrage_fuer(&fenster, e.agent.modus),
        )?;
        let schritte = e.agent.schrittgrenze();

        // ⚑ **Der Stand VOR dem Lauf**, damit sich nachher vergleichen
        //   laesst, was wirklich geschehen ist. 📌 Eine Antwort, die
        //   sagt „ich habe die Datei geaendert", ist eine Behauptung des
        //   Modells; der Vergleich ist ein Befund.
        //
        // ⚠️ **Der ganze Ordner und nicht nur die Anhaenge dieses
        //   Beitrags.** Ein Modell, das statt zu aendern eine zweite
        //   Datei schreibt, hat auch etwas hinterlassen, das der Mensch
        //   haben will.
        let vorher_stand = stand_des_ordners(&anhangordner);

        halter.ruhe.wecken();
        let halt = halter.modell.clone();
        let f = fenster.clone();
        let (text, kontext) = tauri::async_runtime::spawn_blocking(move || {
            let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
            let Some(m) = g.as_mut() else {
                return Err("das Modell ist nicht geladen".to_string());
            };
            if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
                ausgabe_uebernehmen(m, &e);
            }
            zusehen(m, &f);
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
            // Der letzte Beitrag ist der Auftrag, der Rest der Verlauf.
            let (auftrag, vorher) = n.split_last().ok_or("es wurde nichts gefragt")?;
            let melder = |meldung: myl_client::Meldung<'_>| {
                if let myl_client::Meldung::Aufruf { name, argumente } = meldung {
                    let _ = f.emit(
                        LEBEND,
                        Lebend::Aufruf {
                            name: name.to_string(),
                            argumente: myl_local_agent::lauf::kurzform(argumente),
                            voll: serde_json::to_string_pretty(argumente)
                                .unwrap_or_else(|_| argumente.to_string()),
                        },
                    );
                }
            };
            let aus = myl_local_agent::lauf::fahren_im_gespraech(
                m,
                &ruestung,
                schritte,
                true,
                grenze,
                vorher,
                &auftrag.content,
                Some(&melder),
            );
            let text = aus.antwort.clone().unwrap_or_default();
            let mut danach: Vec<myl_client::Nachricht> = n.clone();
            danach.push(myl_client::Nachricht::modell(text.clone()));
            let gespraech = myl_local_agent::gespraech::Gespraech::aus(danach);
            let ansage = myl_local_agent::gespraech::ansage(&ruestung);
            let kontext = myl_local_agent::gespraech::anzeige(m, Some(&ansage), &gespraech);
            Ok::<_, String>((text, kontext))
        })
        .await
        .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;
        let geaendert = was_sich_geaendert_hat(&vorher_stand, &stand_des_ordners(&anhangordner));
        return Ok(Antwort {
            text,
            sekunden: (anfang.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            kontext,
            geaendert,
        });
    }

    halter.ruhe.wecken();
    let halt = halter.modell.clone();
    let sprecher = std::sync::Arc::clone(&halter.sprecher);
    let denkbudget = e.modell.denkbudget;
    let text = tauri::async_runtime::spawn_blocking(move || {
        let mut g = halt.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
        let Some(m) = g.as_mut() else {
            return Err("das Modell ist nicht geladen".to_string());
        };
        if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
            ausgabe_uebernehmen(m, &e);
        }
        // ⚑ Die Rollen kommen aus dem Fenster und werden hier auf die
        // beiden abgebildet, die es gibt. Eine unbekannte Rolle wird
        // zur Nutzerrolle und nicht stillschweigend verworfen: Ein
        // verschluckter Beitrag waere ein Gespraech mit einer Luecke.
        // ⛔️ **Auch der Chat ohne Werkzeuge steht unter den Grundsaetzen**
        //    des vorgegebenen Systemprompts (`myl_local_agent::systemprompt`).
        let mut n: Vec<myl_client::Nachricht> = vec![myl_client::Nachricht::system(
            myl_local_agent::systemprompt::grundsaetze(if sprache == "en" {
                myl_client::einstellungen::Sprache::En
            } else {
                myl_client::einstellungen::Sprache::De
            })?,
        )];
        n.extend(verlauf.iter().map(|(rolle, inhalt)| {
            if rolle == "modell" {
                myl_client::Nachricht::modell(inhalt.clone())
            } else {
                myl_client::Nachricht::nutzer(inhalt.clone())
            }
        }));
        // `chat` kommt aus dem Merkmal `Modellweg`.
        use myl_client::Modellweg as _;
        // ⚑ **Satzweise sprechen, waehrend das Modell noch schreibt.**
        // Nur hier, im Chat: In der Agentenschleife stehen im Strom auch
        // Werkzeugaufrufe, und die will niemand vorgelesen bekommen.
        let vorleser = sprechen.unwrap_or(false).then(myl_senses::Sinne::finden).and_then(|s| {
            s.sprechen.as_ref().ok().map(|z| {
                std::sync::Arc::new(Mutex::new(Some(myl_senses::sprechen::Vorleser::neu_geteilt(
                    z,
                    abspieler_im_fenster(fenster.clone()),
                    Some(std::sync::Arc::clone(&sprecher)),
                ))))
            })
        });
        zusehen_mit(m, &fenster, vorleser.clone(), &sprache, LEBEND);
        // ⚑ **Das Denkbudget gilt nur, wenn vorgelesen wird**, und nur
        //   fuer diese eine Antwort: Die Ueberlegung ist dann Wartezeit,
        //   in der nichts klingt. Danach wieder ohne, damit der naechste
        //   Lauf (etwa der Agent) nicht erbt, was hier gesetzt wurde.
        m.denkbudget = if vorleser.is_some() { denkbudget.map(|b| b as usize) } else { None };
        // ⛔️ **Ein angehaltener Chat behaelt, was er bis dahin schrieb**
        //   (Notaus): Der Text ist ein Ergebnis, kein Fehler.
        myl_local_agent::debuglog::eintrag_mit("Chat beginnt", || myl_local_agent::schleife::nachrichten_text(&n));
        let antwort = match m.chat("lokal", &n, Some(grenze)) {
            Ok(a) => Ok(a.text),
            Err(myl_client::Tuerfehler::Abgebrochen { bisher }) => Ok(bisher),
            Err(f) => Err(f.to_string()),
        };
        myl_local_agent::debuglog::eintrag_mit("Chat-Antwort", || match &antwort {
            Ok(t) => t.clone(),
            Err(f) => format!("Fehler: {f}"),
        });
        m.denkbudget = None;
        m.beobachter = None;
        // ⚑ **Der Rest geht noch raus, dann wird gewartet.** Ein
        // Vorleser, der beim Abraeumen mitten im Satz abbricht, klingt
        // kaputt. ⚠️ Fehler beim Sprechen halten die Antwort nicht auf:
        // Eine Antwort, die dasteht, ist wichtiger als eine, die klingt.
        // ⛔️ Nach dem Notaus wird nicht zu Ende gesprochen: Der Vorleser
        //   faellt weg und hoert nach dem laufenden Satz auf.
        if let Some(v) = vorleser {
            if let Some(v) = v.lock().ok().and_then(|mut g| g.take()) {
                if myl_local_agent::notaus::ausgeloest() {
                    drop(v);
                } else {
                    for f in v.abschliessen() {
                        eprintln!("Vorleser: {f}");
                    }
                }
            }
        }
        // ⚑ **Die bereinigte Prosa, nicht der rohe Text** (2026-09-14):
        // Das Denken steht schon im aufklappbaren Button (der Live-Strom
        // trennt es), und der rohe `chat`-Text traegt die Marke `</think>`
        // noch mit. Ohne das Stripping erschien sie als Text vor der
        // Antwort, besonders nach einer Verdichtung, wenn das Modell wieder
        // ausfuehrlich nachdenkt. `chat` selbst bleibt roh, weil die
        // Agentenschleife daraus die Werkzeugaufrufe liest.
        let text = myl_local_agent::lauf::denken_und_prosa(&antwort?).1;
        let mut danach = n;
        danach.push(myl_client::Nachricht::modell(text.clone()));
        let kontext = myl_local_agent::gespraech::anzeige(
            &*m,
            None,
            &myl_local_agent::gespraech::Gespraech::aus(danach),
        );
        Ok::<_, String>((text, kontext))
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;
    let (text, kontext) = text;

    Ok(Antwort {
        text,
        sekunden: (anfang.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        kontext,
        // ⚑ Ohne Werkzeuge kann sich nichts geaendert haben.
        geaendert: Vec::new(),
    })
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
    /// Ein Werkzeug laeuft jetzt: kurz fuer die Zeile, `voll` fuer das
    /// Aufklappen.
    Aufruf { name: String, argumente: String, voll: String },
    /// Und was es zurueckgab, bis zur Anzeigegrenze vollstaendig.
    Ergebnis { name: String, text: String },
    /// Ein Vorschlag wurde abgewiesen, mit dem Grund.
    Abgelehnt { name: String, grund: String },
    /// Der Verlauf passte nicht mehr in den Kontext und wurde verdichtet.
    Verdichtet { vorher: usize, nachher: usize },
    /// Statt zu verdichten wurden alte Werkzeugausgaben ausgeblendet.
    Ausgeblendet { ergebnisse: usize, vorher: usize, nachher: usize },
}

/// Eine Meldung der Agentenschleife in der Form, die das Fenster zeichnet.
fn lebend_aus(m: myl_client::Meldung<'_>) -> Lebend {
    match m {
        myl_client::Meldung::Schritt(nummer) => Lebend::Schritt { nummer },
        myl_client::Meldung::Aufruf { name, argumente } => Lebend::Aufruf {
            name: name.to_string(),
            argumente: myl_local_agent::lauf::kurzform(argumente),
            voll: myl_local_agent::lauf::volltext_der_argumente(argumente),
        },
        myl_client::Meldung::Ergebnis { name, text } => Lebend::Ergebnis {
            name: name.to_string(),
            text: myl_local_agent::lauf::bis_zur_grenze(text, myl_local_agent::lauf::VOLLTEXT_GRENZE),
        },
        myl_client::Meldung::Abgelehnt { name, grund } => Lebend::Abgelehnt {
            name: name.to_string(),
            grund: grund.to_string(),
        },
        myl_client::Meldung::Verdichtet { vorher, nachher } => Lebend::Verdichtet { vorher, nachher },
        myl_client::Meldung::Ausgeblendet { ergebnisse, vorher, nachher } => {
            Lebend::Ausgeblendet { ergebnisse, vorher, nachher }
        }
    }
}

/// Der Ereignisname, unter dem alles Lebende laeuft.
///
/// ⚑ **Einer und nicht sechs.** Sechs Namen hiessen sechs Anmeldungen
/// im Fenster, und wer einen vergisst, verliert eine Art Meldung, ohne
/// dass etwas fehlschlaegt.
const LEBEND: &str = "lauf-lebt";

/// Der Ausschlag des Mikrofons, waehrend die Sprechtaste gehalten wird.
const PEGEL: &str = "sinne-pegel";

/// Ein Stueck gesprochene Antwort, fertig zum Abspielen im Fenster.
const STIMME: &str = "sinne-stimme";

/// **Base64, ohne Fremdkiste.**
///
/// ⚑ **Vierzehn Zeilen gegen eine Abhaengigkeit.** Der Ton geht als Text
/// ans Fenster, weil der Webview keine beliebige Datei von der Platte
/// laden darf; das ist der ganze Zweck, und dafuer lohnt keine Kiste.
fn base64(roh: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut aus = String::with_capacity(roh.len().div_ceil(3) * 4);
    for stueck in roh.chunks(3) {
        let b = [stueck[0], *stueck.get(1).unwrap_or(&0), *stueck.get(2).unwrap_or(&0)];
        let z = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        aus.push(ABC[(z >> 18) as usize & 63] as char);
        aus.push(ABC[(z >> 12) as usize & 63] as char);
        aus.push(if stueck.len() > 1 { ABC[(z >> 6) as usize & 63] as char } else { '=' });
        aus.push(if stueck.len() > 2 { ABC[z as usize & 63] as char } else { '=' });
    }
    aus
}

/// **Ein Abspieler, der das Fenster spielen laesst.**
///
/// # ⚑ Warum nicht `afplay`
///
/// Dreierlei: Der Webview kann **anhalten**, er braucht **kein fremdes
/// Programm**, und nur er weiss, **wie laut es gerade ist**. Ohne das
/// Letzte gaebe es kein Zeichen, das mitschwingt, sondern nur eines,
/// das sich bewegt, und das waere eine Verzierung mit dem Anschein
/// einer Auskunft.
///
/// ⚠️ **Es wird nicht gewartet, bis das Stueck geklungen hat.** Die
/// Reihenfolge haelt das Fenster; hier wird nur in der richtigen
/// Reihenfolge abgeschickt.
fn abspieler_im_fenster(fenster: tauri::AppHandle) -> myl_senses::sprechen::Abspieler {
    Box::new(move |wav: &std::path::Path| {
        let roh = std::fs::read(wav).map_err(|f| format!("{}: {f}", wav.display()))?;
        fenster
            .emit(STIMME, base64(&roh))
            .map_err(|f| format!("der Ton kam nicht ans Fenster: {f}"))
    })
}

/// Haengt einen Beobachter an das Modell, der ans Fenster meldet.
///
/// ⚑ **Er wird am Ende wieder abgenommen.** Das Modell lebt im
/// Fensterzustand und ueberlebt den Lauf; ein Beobachter, der
/// dableibt, hielte einen Fenstergriff aus einem beendeten Auftrag
/// fest und meldete in den naechsten hinein.
fn zusehen(m: &mut myl_client::Oertlichesmodell, fenster: &tauri::AppHandle) {
    zusehen_mit(m, fenster, None, "", LEBEND);
}

/// **Derselbe Zuschauer, der nebenbei vorliest.**
///
/// # ⚑ Satzweise sprechen, waehrend das Modell noch schreibt
///
/// Die Wartezeit einer gesprochenen Antwort kommt fast ganz vom
/// Hauptmodell. Wer erst spricht, wenn alles dasteht, laesst bei hundert
/// Token und 14 Tok/s rund sieben Sekunden Stille; wer die **fertigen
/// Saetze** sofort hinausgibt, ist nach ein bis zwei Sekunden hoerbar.
///
/// ⛔️ **Nur `Text` und nie `Denken`.** Der Strom trennt beides schon,
/// und das Nachdenken vorgelesen zu bekommen waere das Gegenteil von
/// hilfreich. **Genau deshalb gibt es das hier nur im Chat**: In der
/// Agentenschleife stehen im Strom auch Werkzeugaufrufe.
fn zusehen_mit(
    m: &mut myl_client::Oertlichesmodell,
    fenster: &tauri::AppHandle,
    vorleser: Option<std::sync::Arc<Mutex<Option<myl_senses::sprechen::Vorleser>>>>,
    sprache: &str,
    kanal: &'static str,
) {
    let f = fenster.clone();
    let sprache = sprache.to_string();
    m.beobachter = Some(Box::new(move |s: myl_local_agent::textstrom::Stueck| {
        let mut vorrang = None;
        if let Some(v) = &vorleser {
            if let Ok(mut g) = v.lock() {
                if let Some(v) = g.as_mut() {
                    match &s {
                        myl_local_agent::textstrom::Stueck::Text(t) => {
                            v.schub(t);
                            vorrang = Some(v.vorrang());
                        }
                        // ⚑ **Nachdenken wird ueberbrueckt**, mit einem
                        //   vorbereiteten Satz, hoechstens einmal je Antwort
                        //   (siehe `Vorleser::ueberbruecken`).
                        myl_local_agent::textstrom::Stueck::Denken(_) => v.ueberbruecken(&sprache),
                    }
                }
            }
        }
        let _ = f.emit(
            kanal,
            match s {
                myl_local_agent::textstrom::Stueck::Denken(text) => Lebend::Denken { text },
                myl_local_agent::textstrom::Stueck::Text(text) => Lebend::Text { text },
            },
        );
        // ⚑ **Der erste Ton geht vor** (siehe `sprechen::Vorrang`): Ist
        //   das erste Stueck beim Sprecher und klingt noch nichts, haelt
        //   die Erzeugung hier an. Erst nach dem Senden, damit der Text
        //   schon dasteht, und ausserhalb der Sperre. Gemessen: erster Ton
        //   beim 4B nach 3,1 statt 3,75 s, beim 8B nach 4,4 statt 5,4 s.
        if let Some(v) = vorrang {
            v.abwarten();
        }
    }));
}

/// Was eine Frage zurueckbringt.
#[derive(Serialize)]
struct Antwort {
    text: String,
    sekunden: f64,
    /// Der Kontext nach der Antwort, fuer den Balken.
    kontext: Option<myl_local_agent::gespraech::Kontextanzeige>,
    /// **Welche Anhänge der Lauf verändert hat**, als Dateiname.
    ///
    /// ⚑ **Gemessen und nicht behauptet** (Auftrag des Projektinhabers,
    /// 2026-09-23): verglichen wird der Inhalt vor und nach dem Lauf.
    /// Eine Antwort, die sagt „ich habe die Datei geändert", ist eine
    /// Behauptung des Modells; **diese Liste ist ein Befund über das
    /// Dateisystem.**
    ///
    /// ⚠️ **Auch eine neu entstandene Datei steht hier**, denn ein
    /// Modell, das statt zu ändern eine zweite schreibt, hat auch etwas
    /// hinterlassen, das der Mensch haben will.
    #[serde(default)]
    geaendert: Vec<String>,
}

/// Ein Schritt, wie ihn das Fenster braucht.
#[derive(Serialize)]
struct Zeile {
    art: &'static str,
    text: String,
    /// Beim Aufruf die Argumente genau so, wie sie ankamen; das Fenster
    /// zeigt sie beim Aufklappen.
    #[serde(skip_serializing_if = "Option::is_none")]
    voll: Option<String>,
}

/// Was ein Lauf zurueckbringt.
#[derive(Serialize)]
struct Abschluss {
    verlauf: Vec<Zeile>,
    antwort: Option<String>,
    fertig: bool,
    /// ⚑ **Ob der Lauf an der Loop-Erkennung endete** (2026-10-17): Seit die
    /// Schritte ohne Grenze sind, ist das der haeufigste Grund fuer ein
    /// „abgebrochen“, und er gehoert dazugesagt.
    im_kreis: bool,
    sekunden: f64,
    /// ⚑ Ob die Betriebsart die lokalen Werkzeuge gesperrt hat. Ohne
    /// diese Angabe sieht der Nutzer einen Agenten, der seine Werkzeuge
    /// nicht benutzt, und haelt es fuer ein Modellproblem.
    gesperrt: bool,
    /// **Das Gespraech nach dem Lauf**, ohne Werkzeugansage und samt einer
    /// Verdichtung, falls eine noetig war. Das Fenster gibt es beim
    /// naechsten Auftrag zurueck.
    nachrichten: Vec<myl_client::Nachricht>,
    /// Der Kontext danach, fuer den Balken.
    kontext: Option<myl_local_agent::gespraech::Kontextanzeige>,
    /// Die Zusammenfassung am Anfang des Gespraechs, falls der Lauf
    /// verdichten musste; das Fenster legt nur sie ab.
    zusammenfassung: Option<String>,
}

fn zeile_aus(s: &myl_local_agent::lauf::Schritt) -> Zeile {
    use myl_local_agent::lauf::Schritt as S;
    match s {
        S::Plan(t) => Zeile { art: "plan", text: t.clone(), voll: None },
        S::Denken(t) => Zeile { art: "denken", text: t.clone(), voll: None },
        S::Aufruf { name, argumente, voll } => {
            Zeile { art: "aufruf", text: format!("{name} {argumente}"), voll: Some(voll.clone()) }
        }
        S::Unlesbar(t) => Zeile { art: "unlesbar", text: t.clone(), voll: None },
        S::Ergebnis(t) => Zeile { art: "ergebnis", text: t.clone(), voll: None },
        S::Antwort(t) => Zeile { art: "antwort", text: t.clone(), voll: None },
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
/// 📌 **`async`, und das ist keine Kosmetik.** Tauri fuehrt Befehle
/// ohne `async` **auf dem Hauptfaden** aus, und `blocking_pick_folder`
/// wartet dort auf eine Antwort, die nur der Hauptfaden geben kann:
/// Das Fenster stuende. Mit `async` laeuft der Befehl auf dem
/// Nebenlaeufer, und das Warten ist harmlos.
///
/// ⚑ **Auf macOS ist die Auswahl zugleich die Freigabe.** Was der
/// Nutzer im Fensterdialog waehlt, darf die Anwendung danach lesen und
/// schreiben, auch unterhalb von Schreibtisch oder Dokumenten. Wer den
/// Pfad von Hand eintippt, bekommt genau dort `ENOENT`.
/// **Eine Datei anhaengen**, ueber den Dateidialog oder per Ablegen.
///
/// ⚑ **Die Datei wandert unter die Einhaengung und nicht in die
/// Nachricht.** Was das Modell bekommt, ist eine Zeile mit Ort und Art;
/// gelesen wird mit den Werkzeugen, die es ohnehin hat. **Ein Anhang
/// soll den Kontext nicht fuellen, sondern ihn erreichbar machen.**
#[tauri::command]
async fn anhang_aufnehmen(
    pfad: String,
    wurzel: Option<String>,
    // ⛔️ **Der Betriebsmodus gehoert hierher** (Fund 439, 2026-09-23).
    //    Im Chat gibt es **keine** Werkzeuge; eine Anhangzeile, die
    //    `read_file` nennt, verspricht dort etwas, das es nicht gibt.
    //    Das Modell antwortet dann auf „aendere die Datei" mit einer
    //    Anleitung, und niemand sieht, warum.
    //
    // ⚑ **Dieselbe Angabe wie bei `kontext`**, und aus demselben Grund:
    //    Die Werkzeugansage haengt am Modus, also haengt jede Aussage
    //    ueber Werkzeuge daran.
    modus: Option<String>,
) -> Result<Anhangansicht, String> {
    // ⚑ **Zwei Fragen, nicht eine.** Werkzeuge gibt es in beiden
    //   Betriebsarten, sobald ein Anhang da ist; aber nur der Agent
    //   arbeitet am **Arbeitsordner**. Davon hängt der Pfad ab, den das
    //   Modell genannt bekommt.
    let mit_werkzeugen_am_arbeitsordner = modus.as_deref() == Some("agent");
    let mit_werkzeugen = true;
    let einstellungen =
        myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok();
    let wurzel = wurzel
        .filter(|w| !w.is_empty())
        .or_else(|| einstellungen.as_ref().and_then(|e| e.agent.wurzel.clone()))
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .ok_or_else(|| {
            "Es ist kein Arbeitsordner gesetzt. Ohne ihn gibt es keinen Ort fuer den Anhang."
                .to_string()
        })?;
    let a = myl_client::anhang::aufnehmen(
        std::path::Path::new(&wurzel),
        std::path::Path::new(&pfad),
    )?;
    // ⚑ **Hier wird noch nicht hingesehen.** Ein Sehmodell braucht
    // Sekunden bis Minuten, und ein Fenster, das beim Ablegen einer
    // Datei einfriert, ist ein kaputtes Fenster. Die Zeile kommt sofort,
    // das Ansehen holt `anhang_ansehen` nach.
    let ansehen = myl_senses::Sinne::finden().bereit(a.art);
    let werkzeug = einstellungen
        .as_ref()
        .and_then(|e| myl_local_agent::kisten::werkzeug_fuer(&e.agent, a.art.kennung()));
    // ⚑ **Ohne Werkzeuge nennt die Zeile keines.** Das gilt fuer den
    //   Text ebenso wie fuer Bild und Ton: Ein Werkzeugname im Chat ist
    //   ein Versprechen ohne Deckung.
    let werkzeug = if mit_werkzeugen { werkzeug } else { None };
    let sicht = match (&ansehen, &werkzeug) {
        // ⛔️ **`Kommt` und nicht `Nichts`** (Fund 436). Beide schreiben
        //    keine Zeile, die zu einem Werkzeugaufruf raet, und nur das
        //    war hier gemeint. `Nichts` behauptet darueber hinaus, es
        //    sei **kein Sinnesmodell eingerichtet** und ueber den Inhalt
        //    sei nichts zu sagen. Das stand dann in derselben Nachricht
        //    wie die Beschreibung, die `anhang_ansehen` gleich darauf
        //    anhaengt, und das Modell las den ersten Satz zuerst.
        (true, _) => myl_client::anhang::Sicht::Kommt,
        (false, Some(w)) => myl_client::anhang::Sicht::Werkzeug(w),
        (false, None) => myl_client::anhang::Sicht::Nichts,
    };
    // ⚑ **Text hat keine eigene Sicht gehabt**, bis Fund 439 zeigte,
    //   dass er eine braucht: `read_file` ist ein Werkzeug wie jedes
    //   andere und im Chat nicht da.
    let sicht = if matches!(a.art, myl_senses::anhang::Art::Text) {
        if mit_werkzeugen {
            myl_client::anhang::Sicht::Werkzeug("read_file")
        } else {
            myl_client::anhang::Sicht::Nichts
        }
    } else {
        sicht
    };
    // ⛔️ **Im Chat heisst der Anhang anders**, und das ist keine
    //    Kosmetik. Dort sitzt die Einhängung auf dem Anhangordner
    //    selbst (siehe `ruesten_fuer_anhaenge`); ein Pfad
    //    `.AGENT/anhaenge/liste.md` löste dort **nicht** auf, denn er
    //    zeigte aus der Einhängung hinaus. Der Name allein ist der
    //    richtige Pfad.
    //
    // 📌 **Dieselbe Angabe an zwei Orten wäre hier besonders teuer:**
    //    Das Modell bekäme einen Pfad, den sein eigenes Werkzeug
    //    ablehnt, und niemand sähe den Grund.
    let mut a = a;
    if !mit_werkzeugen_am_arbeitsordner {
        a.pfad = a.name.clone();
    }
    Ok(Anhangansicht {
        nachricht: a.nachricht_mit(true, sicht),
        name: a.name,
        pfad: a.pfad.clone(),
        art: a.art.wort(true).to_string(),
        bytes: a.bytes,
        ansehen,
    })
}

/// **Sieht einen schon angehaengten Anhang an**, in einem eigenen Faden.
///
/// ⚑ **Getrennt vom Aufnehmen**, damit das Ablegen einer Datei sofort
/// eine Zeile gibt und das Fenster nicht steht (siehe
/// [`anhang_aufnehmen`]).
///
/// ⛔️ **Nur Dateien, die schon im Anhangordner liegen** (Festlegung des
/// Projektinhabers, 2026-09-17: der Chat reagiert **nur** auf
/// ausdruecklich hochgeladene Dateien). Der Pfad geht durch die
/// Einhaengung, und dann muss er unter dem Anhangordner liegen; sonst
/// waere dieser Befehl ein Weg, jede Datei der Platte ansehen zu lassen.
#[tauri::command]
async fn anhang_ansehen(pfad: String, wurzel: Option<String>) -> Result<String, String> {
    let wurzel = wurzel
        .filter(|w| !w.is_empty())
        .or_else(|| {
            myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
                .ok()
                .and_then(|e| e.agent.wurzel.clone())
        })
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .ok_or_else(|| "Es ist kein Arbeitsordner gesetzt.".to_string())?;
    let ein = myl_local_agent::werkzeuge::Einhaengung::neu(&wurzel, false)?;
    let datei = ein.aufloesen(&pfad, true).map_err(|f| f.grund)?;
    let anhangordner = std::path::Path::new(&wurzel)
        .join(myl_client::anhang::unterordner())
        .canonicalize()
        .map_err(|f| format!("der Anhangordner: {f}"))?;
    if !datei.starts_with(&anhangordner) {
        return Err("nur angehaengte Dateien werden angesehen".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let anfang = myl_client::anhang::art_bestimmen(&lies_anfang(&datei), &pfad);
        let sinne = myl_senses::Sinne::finden();
        // ⚑ **Die schnelle Sprosse**: ein Blick fuer jeden, auch fuer
        // den, der gar nichts fragen wollte.
        myl_senses::auswerten(&sinne, &datei, anfang, None, myl_senses::Stufe::Schnell)
            .ok_or_else(|| "fuer diese Art sieht hier niemand hin".to_string())?
    })
    .await
    .map_err(|f| format!("der Rechenfaden ist abgestuerzt: {f}"))?
}

/// Was das Fenster ueber die Sinne erfaehrt.
#[derive(serde::Serialize)]
struct Sinnesansicht {
    /// Ob ein Bild angesehen werden kann.
    sehen: bool,
    /// Ob eine Aufnahme mitgeschrieben werden kann.
    hoeren: bool,
    /// Ob eine Antwort vorgelesen werden kann.
    sprechen: bool,
    /// Ob die Sprechtaste geht: aufnehmen **und** mitschreiben.
    zuhoeren: bool,
    /// Wie gesprochen wird, fuer die Einstellungsseite.
    sprechweg: String,
    /// Ob der Sprechweg eine Stimmprobe nachbilden kann.
    klont: bool,
    /// Ob eine Stimmprobe liegt.
    probe: bool,
    /// Was zur Stimmprobe zu sagen ist, falls etwas zu sagen ist.
    hinweis: Option<String>,
    /// Was fehlt, je Sinn, fuer die Einstellungsseite.
    mangel: Vec<String>,
}

/// **Was dieser Rechner sehen, hoeren und sprechen kann.**
///
/// ⚑ **Frisch gefragt, nicht gemerkt.** Wer waehrend einer Sitzung ein
/// Modell hinlegt, soll es benutzen koennen, ohne das Fenster neu zu
/// starten; die Suche kostet ein paar Dateiabfragen.
/// Der Stand der Sinne, fuer den Befehl und fuer die Befehle, die ihn danach zurueckgeben.
fn sinne_ansicht() -> Sinnesansicht {
    let s = myl_senses::Sinne::finden();
    let mut mangel = Vec::new();
    for m in [s.sehen.as_ref().err(), s.hoeren.as_ref().err(), s.sprechen.as_ref().err()]
        .into_iter()
        .flatten()
    {
        mangel.push(m.bericht());
    }
    let (sprechweg, klont, probe) = match s.sprechen.as_ref() {
        Ok(z) => (z.name(), z.kann_klonen(), z.probe.is_some()),
        Err(_) => (String::new(), false, false),
    };
    Sinnesansicht {
        sehen: s.sehen.is_ok(),
        hoeren: s.hoeren.is_ok(),
        sprechen: s.kann_sprechen(),
        zuhoeren: s.kann_zuhoeren(),
        sprechweg,
        klont,
        probe,
        hinweis: s.stimmhinweis(),
        mangel,
    }
}

/// **Die Sprechtaste, gedrueckt.**
///
/// ⚑ **Im Fenster ist es wirklich eine Taste**, gedrueckt und
/// losgelassen. In einem Terminal ginge das nicht: Das Loslassen meldet
/// nur, wer das Kitty-Protokoll spricht.
/// ⚑ `async`, denn die Suche fragt Geraete und Programme ab und soll das
/// Fenster nicht anhalten.
#[tauri::command]
async fn sinne_stand() -> Sinnesansicht {
    sinne_ansicht()
}

#[tauri::command]
async fn sprechtaste_start(
    fenster: tauri::AppHandle,
    halter: tauri::State<'_, Halter>,
) -> Result<(), String> {
    let sinne = myl_senses::Sinne::finden();
    let mut laufende = myl_senses::zuhoeren_beginnen(&sinne)?;
    // ⚑ **Der Ausschlag geht als Ereignis ans Fenster**, Bild fuer Bild.
    // Ein Mikrofon, das auf das falsche Geraet zeigt, sieht sonst genauso
    // aus wie eines, das zuhoert.
    if let Some(pegel) = laufende.pegel_nehmen() {
        let f = fenster.clone();
        std::thread::spawn(move || {
            for wert in pegel {
                if f.emit(PEGEL, wert).is_err() {
                    break;
                }
            }
        });
    }
    let mut g = halter.aufnahme.lock().map_err(|_| "der Aufnahmehalter ist vergiftet")?;
    // ⚠️ **Eine zweite Aufnahme beendet die erste.** Sie fallen zu
    // lassen, ohne sie zu beenden, liesse ein Programm laufen.
    *g = Some(laufende);
    Ok(())
}

/// **Die Sprechtaste, losgelassen**: aufhoeren und mitschreiben.
#[tauri::command]
async fn sprechtaste_ende(halter: tauri::State<'_, Halter>) -> Result<String, String> {
    let laufende = {
        let mut g = halter.aufnahme.lock().map_err(|_| "der Aufnahmehalter ist vergiftet")?;
        g.take().ok_or("es lief keine Aufnahme")?
    };
    // ⚑ **In einem eigenen Faden.** Das Mitschreiben laedt ein Modell;
    // ein Fenster, das dabei steht, sieht kaputt aus.
    tauri::async_runtime::spawn_blocking(move || {
        let sinne = myl_senses::Sinne::finden();
        myl_senses::zuhoeren_beenden(&sinne, laufende, "auto")
    })
    .await
    .map_err(|f| format!("der Rechenfaden ist abgestuerzt: {f}"))?
}

/// **Legt eine hochgeladene Aufnahme als Stimme ab.**
#[tauri::command]
async fn stimme_setzen(pfad: String, einwilligung: Option<bool>) -> Result<Sinnesansicht, String> {
    // ⛔️ **Keine Stimme ohne Einwilligung** (Zweckbestimmung, Abschnitt
    //   2.3): Die Aufnahme ist die eigene, oder die Person hat eingewilligt.
    //   Geprueft hier und nicht nur am Haekchen im Fenster, damit ein
    //   anderer Aufrufer nicht daran vorbeikommt.
    if einwilligung != Some(true) {
        return Err("Ohne bestaetigte Einwilligung wird keine Stimme abgelegt.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        myl_senses::sprechen::probe_setzen(std::path::Path::new(&pfad))
    })
    .await
    .map_err(|f| format!("der Rechenfaden ist abgestuerzt: {f}"))??;
    Ok(sinne_ansicht())
}

/// **Nimmt die Stimmprobe wieder weg.**
#[tauri::command]
async fn stimme_entfernen() -> Sinnesansicht {
    myl_senses::sprechen::probe_entfernen();
    sinne_ansicht()
}

/// **Laedt das Sprechmodell im Voraus.**
///
/// ⚑ **Waehrend das Hauptmodell nachdenkt, kann der Sprecher laden.**
/// Danach ist die Wartezeit weg statt verschoben. ⚠️ Der Aufruf kommt
/// sofort zurueck; geladen wird in einem eigenen Faden, denn wer den
/// Lautsprecher einschaltet, will nicht achtzehn Sekunden auf einen
/// Knopf warten.
#[tauri::command]
async fn stimme_vorwaermen(halter: tauri::State<'_, Halter>) -> Result<(), String> {
    let sprecher = std::sync::Arc::clone(&halter.sprecher);
    // Die Saetze zum Ueberbruecken werden in der Sprache des Fensters
    // vorbereitet.
    let sprache = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .map(|e| e.oberflaeche.sprache.kennung().to_string())
        .unwrap_or_else(|_| "de".into());
    std::thread::spawn(move || {
        let sinne = myl_senses::Sinne::finden();
        if let Ok(z) = sinne.sprechen.as_ref() {
            if let Err(f) = myl_senses::sprechen::vorwaermen(z, &sprecher) {
                eprintln!("Sprechmodell: {f}");
                return;
            }
            // ⚑ **Nach dem Aufwaermen und im selben Faden**: Der Laeufer
            //   steht dann schon, und jeder Satz kostet rund eine Sekunde.
            //   Beim naechsten Start liegen sie bereit und kosten nichts.
            if let Err(f) = myl_senses::sprechen::ueberbrueckungen_vorbereiten(z, &sprecher, &sprache) {
                eprintln!("Saetze zum Ueberbruecken: {f}");
            }
        }
    });
    Ok(())
}

/// **Schreibt den CosyVoice-Laeufer in die Heimat**, falls er fehlt.
///
/// ⛔️ **Nie ueberschrieben.** Wer ihn angepasst hat, hat ihn angepasst.
#[tauri::command]
fn sinne_einrichten() -> Result<String, String> {
    let (pfad, geschrieben) = myl_senses::sprechen::laeufer_einrichten()?;
    Ok(if geschrieben {
        format!("Angelegt: {}", pfad.display())
    } else {
        format!("Liegt schon: {}", pfad.display())
    })
}

/// Die ersten Bytes einer Datei, fuer die Artbestimmung.
fn lies_anfang(p: &std::path::Path) -> Vec<u8> {
    use std::io::Read;
    let mut puffer = vec![0u8; 4096];
    match std::fs::File::open(p).and_then(|mut f| f.read(&mut puffer)) {
        Ok(n) => {
            puffer.truncate(n);
            puffer
        }
        Err(_) => Vec::new(),
    }
}

/// Was das Fenster ueber einen Anhang erfaehrt.
#[derive(serde::Serialize)]
struct Anhangansicht {
    /// Die fertige Zeile fuer das Gespraech.
    nachricht: String,
    name: String,
    pfad: String,
    art: String,
    bytes: u64,
    /// Ob gleich noch ein Sinnesmodell hinsieht. Das Fenster zeigt so
    /// lange, dass etwas laeuft, statt eine fertige Zeile vorzutaeuschen.
    ansehen: bool,
}

/// **Gibt einen geänderten Anhang heraus**, dorthin, wo der Mensch ihn
/// haben will.
///
/// # ⛔️ Warum der Weg hinaus ein eigener Befehl ist
///
/// Die Werkzeuge des Chats kommen **nicht** aus dem Anhangordner heraus;
/// das ist der ganze Zuschnitt (siehe `ruestung::ruesten_fuer_anhaenge`).
/// Eine geänderte Datei muss trotzdem beim Menschen landen können, und
/// **das entscheidet der Mensch und nicht das Modell**: Hier wählt er
/// den Ort, über den Dialog des Systems.
///
/// ⚠️ **Die Quelle wird aufgelöst und geprüft.** Ein Name, der aus dem
/// Anhangordner hinausführt, wird abgewiesen, auch wenn er aus dem
/// eigenen Fenster kommt: Ein Befehl, der jeden Pfad nimmt, den ihm
/// jemand nennt, ist eine Tür neben der Tür.
#[tauri::command]
async fn anhang_herausgeben(
    app: tauri::AppHandle,
    name: String,
    wurzel: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let ordner = wurzel
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .or_else(|| {
            myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
                .ok()
                .and_then(|e| e.agent.wurzel)
        })
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .ok_or_else(|| "Es ist kein Arbeitsordner gesetzt.".to_string())?;
    let anhangordner = std::path::Path::new(&ordner).join(myl_client::anhang::unterordner());

    // ⛔️ **Die Einhaengegrenze gilt auch hinaus.** `aufloesen` weist
    //    jeden Namen ab, der aus dem Anhangordner hinausfuehrt.
    let ein = myl_local_agent::werkzeuge::Einhaengung::neu(&anhangordner, false)
        .map_err(|f| format!("Anhangordner: {f}"))?;
    let quelle = ein
        .aufloesen(&name, true)
        .map_err(|f| format!("{name}: {}", f.grund))?;

    let vorschlag = std::path::Path::new(&name)
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "anhang".to_string());
    let Some(ziel) = app
        .dialog()
        .file()
        .set_title("Geaenderten Anhang speichern")
        .set_file_name(&vorschlag)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let ziel = ziel
        .into_path()
        .map_err(|f| format!("der gewaehlte Ort ist kein Pfad: {f}"))?;
    std::fs::copy(&quelle, &ziel).map_err(|f| format!("{}: {f}", ziel.display()))?;
    Ok(Some(ziel.display().to_string()))
}

/// **Der Dateidialog des Systems**, fuer den Anhang.
///
/// ⚑ **Ein eigener Befehl neben `ordner_waehlen`**, denn es ist eine
/// andere Frage: Dort waehlt jemand die **Reichweite** des Agenten, hier
/// eine einzelne Datei. Sie in einen Befehl zu legen hiesse, zwei
/// Entscheidungen hinter einem Schalter zu verstecken.
#[tauri::command]
async fn datei_waehlen(app: tauri::AppHandle, titel: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let gewaehlt = app.dialog().file().set_title(titel).blocking_pick_file();
    Ok(gewaehlt.map(|g| g.to_string()))
}

/// Was das Fenster ueber einen zum Lernen gewaehlten Skill weiss.
#[derive(Serialize)]
struct Skillwahl {
    name: String,
    pfad: String,
    /// Wofuer der Skill ist, ein Satz.
    satz: String,
    bytes: u64,
}

/// **Der Dateidialog fuer „Skill lernen"**: Er oeffnet im eigenen
/// Skill-Ordner neben den Einstellungen und zeigt Markdown-Dateien.
///
/// ⚑ **Der Ordner wird angelegt, wenn es ihn noch nicht gibt.** Ein
/// Dialog, der irgendwo aufgeht, weil sein Startort fehlt, erklaert
/// niemandem, wohin eigene Skills gehoeren.
///
/// ⚑ **Geprueft wird hier und noch einmal beim Abschicken**
/// ([`skill_auftrag`]): hier, damit ein Plaettchen nur fuer etwas
/// entsteht, das sich lernen laesst; dort, weil die Datei dazwischen
/// geaendert worden sein kann.
#[tauri::command]
async fn skill_waehlen(app: tauri::AppHandle, titel: String) -> Result<Option<Skillwahl>, String> {
    use tauri_plugin_dialog::DialogExt;

    let ordner = myl_local_agent::skills::allgemeiner_ordner();
    let _ = std::fs::create_dir_all(&ordner);
    let mut w = app.dialog().file().set_title(titel).add_filter("Markdown", &["md"]);
    if ordner.is_dir() {
        w = w.set_directory(&ordner);
    }
    let Some(gewaehlt) = w.blocking_pick_file() else {
        return Ok(None);
    };
    let pfad = gewaehlt.simplified().into_path().map_err(|f| f.to_string())?;
    let seite = myl_local_agent::skills::aus_datei(&pfad)?;
    Ok(Some(Skillwahl {
        name: seite.name,
        pfad: seite.pfad.display().to_string(),
        satz: seite.satz,
        bytes: seite.bytes,
    }))
}

/// **Der Auftrag, den das Modell bekommt, wenn Skills anliegen**: die
/// Anleitungen, die Bitte um einen Satz je Skill, und dahinter der
/// Auftrag des Nutzers, falls es einen gibt.
///
/// ⚑ **Gebaut in `myl_local_agent::skills::lernauftrag`**, derselben Stelle,
/// die auch die Konsole nimmt.
#[tauri::command]
async fn skill_auftrag(pfade: Vec<String>, auftrag: String, deutsch: bool) -> Result<String, String> {
    let seiten = pfade
        .iter()
        .map(|p| myl_local_agent::skills::aus_datei(std::path::Path::new(p)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(myl_local_agent::skills::lernauftrag(&seiten, &auftrag, deutsch))
}

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
    // 📌 `simplified` nimmt unter Windows das `\\?\`-Praefix weg. Ohne
    // das stuende im Feld ein Pfad, den zwar jede Rust-Funktion
    // versteht, aber kein Mensch wiedererkennt.
    let pfad = gewaehlt.simplified().into_path().map_err(|f| f.to_string())?;
    Ok(Some(pfad.display().to_string()))
}

// ── Das Terminal ────────────────────────────────────────────────────
//
// # ⛔️ Hier wird eine Grenze bewusst NICHT gezogen, und das gehoert an
//   den Anfang
//
// Ueberall sonst im Client faesst die Einhaengegrenze ein, was laufen
// darf: `write_file` kommt nicht aus dem Arbeitsordner heraus, ein
// Manifest braucht die Schreiberlaubnis, `run_command` gibt es im Chat
// gar nicht. **Dieses Terminal hat keine dieser Schranken.** Es fuehrt
// aus, was dasteht, mit den Rechten des Nutzers.
//
// ⚑ **Der Unterschied ist, WER tippt.** Jede Schranke im Client schuetzt
// vor einem **Modell**, das sich irrt oder das ein fremder Text in die
// Irre fuehrt. Ein Mensch, der ein Terminal oeffnet, hat genau das
// gewollt, und ihm dieselben Fesseln anzulegen hiesse, ihm ein Terminal
// zu geben, das keines ist.
//
// ⛔️ **Also gilt: Das Modell kommt hier nicht heran.** Dieser Befehl
// steht in keinem Werkzeugkasten, er wird von keiner Ruestung
// angeboten, und keine Agentenschleife kann ihn rufen. Er wird vom
// Fenster gerufen und sonst von niemandem. Wer ihn je als Werkzeug
// anmeldet, macht aus dem Chat einen Vollzugriff.

/// Wie lange ein Befehl hoechstens laufen darf.
///
/// ⚑ **Grosszuegig, denn ein Mensch tippt auch `cargo build`.** Die
/// Frist ist keine Sicherung, sondern ein Rettungsanker gegen ein
/// Programm, das auf eine Eingabe wartet, die nie kommt: `stdin` liegt
/// auf `null`, ein solches Programm haengt also fuer immer.
const TERMINALFRIST_S: u64 = 300;
/// Wie viele Zeichen einer Ausgabe ins Fenster gehen.
const TERMINALGRENZE: usize = 40_000;

/// Was ein Befehl hinterlassen hat.
#[derive(Serialize)]
struct Befehlsausgang {
    /// Beide Roehren hintereinander, wie im Terminal.
    ausgabe: String,
    /// Der Rueckgabewert, falls es einen gab.
    kode: Option<i32>,
    /// Wo das Terminal danach steht.
    ordner: String,
    sekunden: f64,
    /// Ob mehr da war, als gezeigt wird.
    gekuerzt: bool,
    /// Ob die Frist abgelaufen ist.
    abgebrochen: bool,
}

/// Wo das Terminal beginnt, wenn noch niemand `cd` gesagt hat.
fn terminalanfang() -> std::path::PathBuf {
    myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())
        .ok()
        .and_then(|e| e.agent.wurzel.clone())
        .or_else(myl_client::Einstellungen::standard_wurzel)
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// **Fuehrt einen Befehl aus, den ein Mensch getippt hat.**
///
/// ⚑ **`cd` wird selbst behandelt und nicht an die Shell gegeben.**
/// Jeder Aufruf startet eine eigene Shell; ein `cd` darin waere mit
/// ihrem Ende wieder weg. Der Ordner lebt deshalb im Fensterzustand.
#[tauri::command]
async fn terminal_ausfuehren(
    befehl: String,
    halter: tauri::State<'_, Halter>,
) -> Result<Befehlsausgang, String> {
    let anfang = std::time::Instant::now();
    let jetzt = {
        let mut o = halter.terminalordner.lock().map_err(|_| "der Ordner klemmt".to_string())?;
        o.get_or_insert_with(terminalanfang).clone()
    };
    let eingabe = befehl.trim();

    // Leere Zeile: nur der Prompt, kein Prozess.
    if eingabe.is_empty() {
        return Ok(Befehlsausgang {
            ausgabe: String::new(),
            kode: Some(0),
            ordner: jetzt.display().to_string(),
            sekunden: 0.0,
            gekuerzt: false,
            abgebrochen: false,
        });
    }

    if eingabe == "cd" || eingabe.starts_with("cd ") {
        let ziel = eingabe[2..].trim();
        let neu = if ziel.is_empty() || ziel == "~" {
            std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or(jetzt.clone())
        } else if let Some(rest) = ziel.strip_prefix("~/") {
            std::env::var_os("HOME")
                .map(|h| std::path::PathBuf::from(h).join(rest))
                .unwrap_or_else(|| jetzt.join(rest))
        } else {
            jetzt.join(ziel)
        };
        // ⚑ **Aufgeloest und nicht zusammengesetzt.** Ohne
        //   `canonicalize` stuende nach zweimal `cd ..` ein Pfad mit
        //   zwei Punkten darin im Prompt, und der waere zwar gueltig,
        //   aber unlesbar.
        let neu = std::fs::canonicalize(&neu)
            .map_err(|f| format!("cd: {}: {f}", neu.display()))?;
        if !neu.is_dir() {
            return Err(format!("cd: {}: kein Verzeichnis", neu.display()));
        }
        {
            let mut o = halter.terminalordner.lock().map_err(|_| "der Ordner klemmt".to_string())?;
            *o = Some(neu.clone());
        }
        return Ok(Befehlsausgang {
            ausgabe: String::new(),
            kode: Some(0),
            ordner: neu.display().to_string(),
            sekunden: anfang.elapsed().as_secs_f64(),
            gekuerzt: false,
            abgebrochen: false,
        });
    }

    let mut b = if cfg!(windows) {
        let mut b = std::process::Command::new("cmd");
        b.arg("/C").arg(eingabe);
        b
    } else {
        let mut b = std::process::Command::new("/bin/sh");
        b.arg("-c").arg(eingabe);
        b
    };
    b.current_dir(&jetzt);
    // ⚑ **Ohne Farben.** Eine Ausgabe mit ANSI-Folgen saehe im Fenster
    //   aus wie Kauderwelsch; dieses Terminal zeigt Text und emuliert
    //   kein Terminal.
    b.env("TERM", "dumb");
    b.env("NO_COLOR", "1");

    let a = myl_senses::prozess::laufen(&mut b, TERMINALFRIST_S, TERMINALGRENZE)
        .map_err(|f| format!("der Befehl liess sich nicht starten: {f}"))?;
    let (ausgabe, gekuerzt) = a.zusammen(TERMINALGRENZE);
    Ok(Befehlsausgang {
        ausgabe,
        kode: a.kode,
        ordner: jetzt.display().to_string(),
        sekunden: anfang.elapsed().as_secs_f64(),
        gekuerzt,
        abgebrochen: a.abgebrochen,
    })
}

/// Wo das Terminal steht, fuer den Prompt beim ersten Zeichnen.
#[tauri::command]
async fn terminal_ordner(halter: tauri::State<'_, Halter>) -> Result<String, String> {
    let mut o = halter.terminalordner.lock().map_err(|_| "der Ordner klemmt".to_string())?;
    Ok(o.get_or_insert_with(terminalanfang).display().to_string())
}

// --- Befehlszeiten ----------------------------------------------------------
//
// ⚑ **Messen, wohin die Zeit geht** (Meldung des Projektinhabers, 2026-10-17:
// das Fenster laedt lange ohne Bild, die Einstellungsseite bis zu zehn
// Sekunden). Mit `MYL_BEFEHLSZEITEN=<datei>` schreibt das Fenster je Zeile
// die Millisekunden seit dem Start, was es war, und wie lange: jeden Befehl
// mit seiner Zeit auf dem Hauptfaden (gemessen um den Aufruf herum, bei
// `async`-Befehlen nur das Annehmen), dazu, was das Skript meldet
// (`zeit_melden`: langsame Antworten, Luecken im Bildtakt, Abschnitte des
// Starts). Ohne die Variable schreibt es nichts; bei laufendem Debug-Log
// steht, was ueber 100 ms dauerte, dort.

fn startzeit() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

/// Schreibt eine Zeile der Befehlszeiten.
fn zeit_schreiben(was: &str, ms: u64) {
    let seit = startzeit().elapsed().as_millis();
    if let Some(p) = std::env::var_os("MYL_BEFEHLSZEITEN") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{seit:>8} ms  {ms:>7} ms  {was}");
        }
    }
    if ms > 100 {
        myl_local_agent::debuglog::eintrag_mit("Zeit", || format!("{was}: {ms} ms"));
    }
}

/// **Was das Skript misst**: langsame Antworten, Luecken im Bildtakt,
/// Abschnitte des Starts.
#[tauri::command]
fn zeit_melden(was: String, ms: u64) {
    zeit_schreiben(&format!("Fenster: {was}"), ms);
}

fn main() {
    startzeit();
    // ⚑ Der Debug-Log folgt der Einstellung `debug.log` (2026-10-12).
    if let Ok(e) = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()) {
        e.debug_anwenden("Fenster", "Fenster gestartet");
    }
    // ⚑ **Die Rechenwerke im Hintergrund suchen, bevor das Fenster steht**:
    //   Die Einstellungsseite braucht sie, und die Suche kostet einen
    //   Unterprozess (`myl_client::hardware::rechenwerke_gemerkt`).
    myl_client::hardware::vorwaermen();
    // ⚑ **Und die Belegung zaehlen**, ebenfalls im Hintergrund (Fund 555).
    belegung_stand();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Halter::default())
        // ⚑ **Fenster zu heisst: Loop anhalten, weiter mit ▶ genau dort**
        //   (Festlegung des Projektinhabers, 2026-10-12), und
        //   ⛔️ **alles andere endet**: Das Schliessen ist der Notschalter.
        .on_window_event(|fenster, ereignis| {
            if let tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed = ereignis {
                use tauri::Manager;
                loop_beim_schliessen(&fenster.state::<Halter>().schleife);
                // ⚑ Mit dem Fenster enden seine Module.
                fenster.state::<Halter>().module.alle_beenden();
                notschalter_beim_schliessen();
                // ⚑ **Ein Knoten aus diesem Fenster endet mit ihm**
                //   (`myl_client::mining`, Kopf); einer aus der Konsole nicht.
                myl_client::mining::beenden_wenn_eigen();
                myl_local_agent::debuglog::setzen(false, "Fenster", "Fenster geschlossen");
            }
        })
        .invoke_handler({
          let befehle: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
            zeit_melden,
            modell_vorladen,
            saat_stand,
            saat_einmal,
            tasks,
            task_anlegen,
            loop_umfeld,
            gespraech_vergessen,
            tasks_ordnen,
            task_weiter,
            task_bestaetigen,
            task_nachbessern,
            task_antworten,
            module_wahl,
            modul_befehle,
            modul_befehl,
            modul_stand,
            modul_antwort,
            modul_waehlen,
            modul_datei,
            loop_nach_dem_start,
            task_stoppen,
            task_entfernen,
            task_bearbeiten,
            autowarnung,
            loop_starten,
            loop_pausieren,
            starthinweis,
            einstellungen,
            felder,
            hardware,
            markdown,
            artefakt_loeschen,
            modell_entladen,
            schlafen,
            setzen,
            modell_laden,
            agent_fahren,
            frage,
            anhang_herausgeben,
            kontext,
            verdichten,
            werkzeuge,
            agentenwarnung,
            modelle,
            katalog,
            voraussetzungen,
            artefakt_bauen,
            bauplan,
            mining_lage,
            rechenwerke_auffrischen,
            mining_stand,
            mining_ablegen,
            mining_starten,
            mining_beenden,
            gespraech_ausgeben,
            ordner_waehlen,
            anhang_aufnehmen,
            anhang_ansehen,
            sinne_stand,
            sprechtaste_start,
            sprechtaste_ende,
            stimme_setzen,
            stimme_entfernen,
            sinne_einrichten,
            stimme_vorwaermen,
            datei_waehlen,
            skill_waehlen,
            skill_auftrag,
            aktualisierung,
            aktualisieren,
            neu_starten,
            terminal_ausfuehren,
            terminal_ordner
        ];
          // ⚑ Die Zeit auf dem Hauptfaden je Befehl (siehe `zeit_schreiben`).
          move |aufruf: tauri::ipc::Invoke<tauri::Wry>| {
              let name = aufruf.message.command().to_string();
              let anfang = std::time::Instant::now();
              let antwort = befehle(aufruf);
              if name != "zeit_melden" {
                  zeit_schreiben(&format!("Hauptfaden: {name}"), anfang.elapsed().as_millis() as u64);
              }
              antwort
          }
        })
        .run(tauri::generate_context!())
        .expect("die Oberflaeche liess sich nicht starten");
}

// ── Modelle holen und Artefakte bauen ───────────────────────────────
//
// # 📌 Die groesste Luecke zwischen „frischer Klon" und „laeuft"
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
/// 📌 **Gesucht und nicht angenommen.** Die Oberflaeche laeuft im
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
    // ⚑ **Hergeleitet wird in der Kiste** (seit dem 2026-09-16). Die
    // Konsole stellt dieselbe Frage; zwei Herleitungen desselben Ortes
    // zeigen irgendwann auf zwei Datentraeger.
    myl_local_agent::ort::datenort()
}

/// Was Myelith heute schon auf der Platte haelt.
fn belegung_heute() -> u64 {
    // ⚑ **Die Liste der gefuellten Orte steht in `ort`**, nicht hier:
    // Gewichte und Artefakte liegen seit dem 2026-09-21 nicht mehr im
    // selben Elternverzeichnis, und eine Aufzaehlung an dieser Stelle
    // waere die zweite (Fund 410).
    let summe = myl_local_agent::ort::gefuellte_orte()
        .iter()
        .map(|o| myl_client::reservierung::belegung(o))
        .sum();
    if let Ok(mut g) = BELEGUNG.lock() {
        g.wert = Some((std::time::Instant::now(), summe));
    }
    summe
}

/// Die gemerkte Belegung, und ob gerade gezaehlt wird.
#[derive(Default)]
struct Belegungsstand {
    wert: Option<(std::time::Instant, u64)>,
    zaehlt: bool,
}

static BELEGUNG: std::sync::Mutex<Belegungsstand> = std::sync::Mutex::new(Belegungsstand { wert: None, zaehlt: false });

/// Wie alt eine gemerkte Belegung werden darf, bevor im Hintergrund neu
/// gezaehlt wird.
const BELEGUNG_GILT: std::time::Duration = std::time::Duration::from_secs(300);

/// **Die Belegung, ohne zu warten**: die gemerkte Zahl, oder `None`, solange
/// die erste Zaehlung laeuft. Ist sie aelter als [`BELEGUNG_GILT`] oder fehlt
/// sie, zaehlt ein Hintergrundfaden neu, hoechstens einer zugleich.
///
/// 📌 **Fund 555 (Meldung des Projektinhabers, 2026-10-17): Der Start des
/// Fensters dauerte rund siebzig Sekunden, die Einstellungsseite bis zu
/// zehn.** Gemessen mit `MYL_BEFEHLSZEITEN`: Jeder Aufruf von
/// `einstellungen` brauchte 8 bis 10 s, beim Start fuenfmal hintereinander.
/// Er zaehlte jedesmal alle Dateien unter Artefakten und Gewichten (zwoelf
/// Artefakte und vier Gewichtssaetze, weit ueber 200 GB), nur fuer einen
/// Satz der Freigabeansicht. Meine erste Messung der Teile hatte nur
/// `platte_nachfuehren` gesehen, das ohne Plattengrenze nicht zaehlt.
fn belegung_stand() -> Option<u64> {
    let mut g = BELEGUNG.lock().ok()?;
    let wert = g.wert;
    let alt = wert.is_none_or(|(wann, _)| wann.elapsed() > BELEGUNG_GILT);
    if alt && !g.zaehlt {
        g.zaehlt = true;
        std::thread::spawn(|| {
            belegung_heute();
            if let Ok(mut g) = BELEGUNG.lock() {
                g.zaehlt = false;
            }
        });
    }
    wert.map(|(_, w)| w)
}

/// Vergisst die gemerkte Belegung, etwa nach einem Bau oder Loeschen; die
/// naechste Frage zaehlt neu.
fn belegung_vergessen() {
    if let Ok(mut g) = BELEGUNG.lock() {
        g.wert = None;
    }
    belegung_stand();
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
            // ⚑ Die gemerkte Zahl, wenn es eine gibt; sonst wird hier
            //   gezaehlt, denn die Reservierung braucht eine.
            let belegt = belegung_stand().unwrap_or_else(belegung_heute);
            let soll = myl_client::reservierung::zu_halten(gib as u64 * myl_client::hardware::GIB, belegt);
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

/// 📌 **Die Suche steht seit dem 2026-09-10 in `myl-client`.**
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
    myl_local_agent::ort::wurzel()
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
/// 📌 **Diese Begruendung hiess einmal „ein Dialog braucht ein
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
/// 📌 **Der Name wird entschaerft und nicht uebernommen.** Ein Titel
/// kommt aus dem ersten Satz eines Gespraechs und kann alles
/// enthalten, `/` und `..` eingeschlossen; ungeprueft uebernommen
/// schriebe das Fenster irgendwohin. Erlaubt sind Buchstaben, Ziffern,
/// Strich und Unterstrich, alles andere wird zu einem Strich.
#[tauri::command]
fn gespraech_ausgeben(titel: String, inhalt: String) -> Result<String, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    // 📌 **Ohne eingestellten Ordner wird nichts geschrieben**, und der
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
/// 📌 **macOS meldet eine abgelehnte Ordnerfreigabe als „No such file or
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
/// 📌 **Ohne das scheitert „Modell laden" aus dem Finder heraus**, und
/// zwar mit `No such file or directory`: In den Einstellungen steht
/// `INTEGER_LLM/artifacts/myelith-4b`, und das ist relativ zu einem
/// Arbeitsverzeichnis, das dort `/` ist. Ein absoluter Pfad in den
/// Einstellungen bliebe unberuehrt.
fn artefakt_absolut(pfad: &str) -> String {
    myl_local_agent::ort::absolut(pfad)
}

/// Welche Werkzeugkiste dieser Lauf bekommt.
///
/// ⚑ **Sie haengt am Modell und an der Einstellung, nicht am
/// Aufrufort.** Zwei Stellen, die dieselbe Wahl jede fuer sich treffen,
/// treffen sie irgendwann verschieden: Dann zeigt die Einstellungsseite
/// fuenf Werkzeuge an, und der Agent bekommt sieben.
/// **Was der Agent im eingestellten Modus darf.**
///
/// ⚠️ **Im `manual mode` bleiben die schreibenden Werkzeuge hier ganz
/// weg**, und das ist eine Zwischenloesung mit Ansage. Der Modus
/// bedeutet: **keine schreibende Handlung ohne Zustimmung.** Die
/// Konsole legt jede einzelne vor und wartet auf `j` oder `n`; dieses
/// Fenster hat dafuer noch keinen Kasten. Bis es einen hat, ist „gar
/// nicht schreiben" die einzige Lesart, die der Zusage nicht
/// widerspricht: **Ein Modus, der im einen Fenster fragt und im
/// anderen stillschweigend durchlaesst, waere schlimmer als keiner.**
///
/// ⚑ Gebaut wird das ueber `schreiben`, also ueber denselben Weg, den
/// die Einhaengung ohnehin geht. Eine zweite Stelle, an der Werkzeuge
/// zurueckgehalten werden, waere eine zweite Stelle, an der jemand
/// eines vergisst.
fn kiste_fuer(e: &myl_client::Einstellungen) -> myl_local_agent::werkzeuge::Werkzeugkiste {
    // ⚑ **Der Ordner sagt die Kiste** (2026-09-15). Vorher stand hier
    // die Auswahl `agent.werkzeuge` samt Ableitung aus der Modellgroesse;
    // seit in den Einstellungen nur noch der Pfad steht, entscheidet sein
    // Name, und zwar an genau einer Stelle fuer Fenster und Konsole.
    myl_local_agent::kisten::kiste_der_gilt(&e.agent)
}

/// Was fehlt, bevor gebaut werden kann.
#[derive(Serialize)]
struct Voraussetzungen {
    /// Leer heisst: es kann losgehen.
    fehlt: Vec<String>,
    wurzel: Option<String>,
}

/// ⚑ **Die Python-Umgebung fehlt hier nicht mehr** (seit dem 2026-10-16):
/// Sie ist der erste Schritt des Baus (`myl_client::bezug`), mit
/// Bestaetigung und im Platzbedarf mitgezaehlt. Bleiben die Skripte des
/// Repositoriums und `bash`, das sie faehrt.
fn voraussetzungen_pruefen() -> Voraussetzungen {
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
    if which("bash").is_none() {
        fehlt.push("Der Befehl `bash` fehlt; die Bauskripte brauchen ihn (unter Windows etwa Git Bash).".into());
    }
    Voraussetzungen { fehlt, wurzel: Some(w.display().to_string()) }
}

/// ⚑ `async` wie die anderen Befehle mit Plattenzugriff.
#[tauri::command]
async fn voraussetzungen() -> Voraussetzungen {
    voraussetzungen_pruefen()
}

/// Sucht einen Befehl im `PATH`.
fn which(name: &str) -> Option<std::path::PathBuf> {
    let pfad = std::env::var_os("PATH")?;
    std::env::split_paths(&pfad)
        .flat_map(|d| [d.join(name), d.join(format!("{name}.exe"))])
        .find(|p| p.is_file())
}

#[tauri::command]
async fn katalog() -> Result<Vec<Katalogeintrag>, String> {
    let w = wurzel_suchen().ok_or("Das Repositorium ist nicht zu finden")?;
    let roh = std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json"))
        .map_err(|e| format!("KATALOG.json: {e}"))?;
    let d: serde_json::Value = serde_json::from_str(&roh).map_err(|e| e.to_string())?;
    let obj = d.as_object().ok_or("KATALOG.json ist kein Objekt")?;

    let text = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
    };
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
            // ⚑ Die Spitze aus dem Plan: was waehrend des Baus zugleich
            //   liegt, gezaehlt nur, was noch fehlt (`myl_client::bezug`).
            braucht_bytes: myl_client::bezug::Bauplan::fuer(&w, schluessel)
                .map(|p| p.bedarf().spitze())
                .unwrap_or(0),
            // 📌 **Am `config.json` erkannt, nicht am Ordner** (2026-10-16):
            //   Jeder Modellordner liegt mit seiner LICENSE im Repositorium,
            //   im kalten Klon also immer da, und der Knopf hiess dann
            //   „kalibrieren" statt „holen und bauen" (dieselbe Lehre wie
            //   Fund 471 im Installationsskript).
            modell_da: w.join("MODELS/llm").join(text(v, "hf_verzeichnis")).join("config.json").is_file(),
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
    /// `umgebung`, `holen`, `kalibrieren`, `vierbit`, `fertig` oder `fehler`.
    phase: &'static str,
    text: String,
}

// --- Mining (Reiter „Knoten") ---------------------------------------------
//
// ⚑ **Dieselbe Freigabe wie `/mining` in der Konsole**, aus derselben
// Stelle (`myl_client::mining`): Vorschlag, Pruefung, Ablage, Start und
// Meldung stehen dort. Hier wird nur weitergereicht.

/// Was die Seite zum Zeichnen braucht.
#[derive(Serialize)]
struct Mininglage {
    platten: Vec<myl_client::mining::Platte>,
    freigabe: myl_client::mining::Freigabe,
    kerne: usize,
    /// ⚑ Jedes gefundene, mit Rechenweg im Namen und gesperrt mit Grund,
    /// wenn es hier nicht rechnet (`mining::werke`).
    rechenwerke: Vec<myl_client::mining::Werkwahl>,
    /// Was unter dem vorgeschlagenen Pfad schon liegt; es zaehlt zum
    /// Hoechstwert des Volumens wie in der Pruefung.
    schon_bytes: u64,
    laeuft: bool,
    /// Wer ihn gestartet hat, wenn er laeuft.
    von: String,
    zeilen: Vec<String>,
    speicherorte: String,
    /// Weitere Orte fuer das Netz aus der Datei.
    weitere: Vec<String>,
}

fn jetzt_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Die Zeilen der Zusammenfassung, wenn Mining laeuft.
fn mining_zeilen() -> Option<(myl_client::mining::Zustand, Vec<String>)> {
    let mut z = myl_client::mining::zustand()?;
    let zeilen = match myl_client::mining::stand(&mut z) {
        Ok(s) => myl_client::mining::zusammenfassung(&z, &s, jetzt_ms()),
        Err(f) => vec![format!("⛏ Mining läuft, der Knoten antwortet aber nicht: {f}")],
    };
    Some((z, zeilen))
}

/// **Die Lage fuer den Reiter**: Platten, Vorschlag, Maschine, Stand.
///
/// ⚑ `async`, denn der Hardwarescan startet auf macOS einen Unterprozess
/// und soll das Fenster nicht anhalten.
#[tauri::command]
async fn mining_lage() -> Result<Mininglage, String> {
    use myl_client::mining;
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let pfad = mining::speicherorte_pfad();
    let orte = mining::Speicherorte::lesen(&pfad).map_err(|f| f.to_string())?;
    let platten = mining::platten();
    let hw = myl_client::hardware::erheben(&myl_local_agent::ort::datenort());
    let werke = mining::werke(&hw, e.oberflaeche.sprache);
    let freigabe = mining::Freigabe::vorschlag(&e, &orte, &platten, hw.kerne.max(1) as u32, &werke);
    let schon_bytes = mining::belegt(&freigabe.pfad());
    let lauf = mining_zeilen();
    Ok(Mininglage {
        platten,
        schon_bytes,
        freigabe,
        kerne: hw.kerne,
        rechenwerke: werke,
        laeuft: lauf.is_some(),
        von: lauf.as_ref().map(|(z, _)| z.von.clone()).unwrap_or_default(),
        zeilen: lauf.map(|(_, z)| z).unwrap_or_else(|| vec!["Mining läuft nicht.".into()]),
        speicherorte: pfad.display().to_string(),
        weitere: orte.fuer(mining::Zweck::Netz).filter(|o| o.name != mining::ORTNAME).map(|o| o.name.clone()).collect(),
    })
}

/// **Sucht die Rechenwerke neu**, wenn der Reiter aufgeht; `true`, wenn
/// eine Karte dazugekommen oder weggefallen ist. Dann holt die Seite ihre
/// Lage noch einmal.
///
/// ⚑ **Nach dem Zeichnen und nicht davor** (Wunsch des Projektinhabers,
/// 2026-10-17: eine im Betrieb angesteckte Karte soll ohne Neustart
/// erscheinen). Die Suche kostet unter macOS einen Unterprozess; der Reiter
/// steht vorher schon mit der gemerkten Liste da.
#[tauri::command]
async fn rechenwerke_auffrischen() -> bool {
    myl_client::hardware::rechenwerke_neu_suchen()
}

/// Die Zusammenfassung, wenn Mining laeuft, sonst `None`. Das Fenster fragt
/// danach, ob der Reiter atmet, und alle fuenf Minuten fuer die Meldung.
#[tauri::command]
async fn mining_stand() -> Option<Vec<String>> {
    mining_zeilen().map(|(_, z)| z)
}

/// Legt die Freigabe ab, ohne zu starten (etwa nur einen Ort fuer Gewichte).
#[tauri::command]
async fn mining_ablegen(freigabe: myl_client::mining::Freigabe) -> Result<String, String> {
    let hw = myl_client::hardware::erheben(&freigabe.platte);
    myl_client::mining::pruefen(&freigabe, &hw)?;
    myl_client::mining::speichern(&freigabe)?;
    Ok(myl_client::mining::speicherorte_pfad().display().to_string())
}

/// **Startet den Knoten.** ⛔️ Geprueft wird in `mining::starten`, vor allem
/// anderen; ohne Platz keine Freigabe.
#[tauri::command]
async fn mining_starten(freigabe: myl_client::mining::Freigabe) -> Result<Vec<String>, String> {
    myl_client::mining::starten(&freigabe, "Fenster")?;
    Ok(mining_zeilen().map(|(_, z)| z).unwrap_or_default())
}

/// Beendet den Knoten, gleich wer ihn gestartet hat.
#[tauri::command]
async fn mining_beenden() -> Result<bool, String> {
    myl_client::mining::beenden()
}

/// Was vor dem Knopf steht: der Plan und ob der Platz reicht.
#[derive(Serialize)]
struct Baufrage {
    /// Schritte, Platz waehrend und nach dem Bau, freier Platz.
    text: String,
    /// ⛔️ `false` heisst: keine Freigabe, erst Speicher schaffen; `grund`
    /// sagt, wie viel fehlt.
    reicht: bool,
    grund: String,
}

/// **Der Plan zu einem Katalogeintrag**, fuer die Bestaetigung.
///
/// ⚑ Derselbe Text wie in der Konsole (`myl_client::bezug`): Was der Bau
/// tut und wie viel Speicher er waehrend des Vorgangs braucht, steht an
/// einer Stelle.
#[tauri::command]
fn bauplan(schluessel: String) -> Result<Baufrage, String> {
    let w = wurzel_suchen().ok_or("Das Repositorium ist nicht zu finden")?;
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let plan = myl_client::bezug::Bauplan::fuer(&w, &schluessel)?;
    let lage = plan.platzlage(&e);
    let (reicht, grund) = match plan.platz_pruefen(&lage) {
        Ok(()) => (true, String::new()),
        Err(f) => (false, f),
    };
    Ok(Baufrage { text: plan.bestaetigung(&lage), reicht, grund })
}

/// **Holt ein Modell und baut sein Artefakt**, nach dem Plan aus
/// `myl_client::bezug`: Python-Umgebung, Gewichte, int8, vier Bit, je
/// nachdem, was fehlt.
///
/// # ⚑ Warum die Skripte gerufen werden und nicht nachgebaut
///
/// `fetch_model.sh` nagelt die Revision fest, `build_artifacts.sh`
/// ruft `calibrate.src.main`, `build_vierbit.sh` den Gitterbau. Beides
/// hier nachzubauen hiesse, den dokumentierten Weg ein zweites Mal zu
/// schreiben, und der zweite ist immer der schlechter gepruefte. Seit dem
/// 2026-10-16 ruft auch das Fenster sie nicht mehr selbst, sondern ueber
/// `bezug`, dieselbe Stelle wie die Konsole.
///
/// # ⚠️ Ein Fortschrittsbalken, der nicht luegt
///
/// Die Skripte melden keinen Prozentsatz. Was gemeldet wird, sind
/// **Phasen**. Der Balken zeigt deshalb die Phase und die letzte Zeile,
/// und keine erfundene Zahl.
///
/// ⛔️ Der Platz wird in `bezug::ausfuehren` unmittelbar vor dem ersten
/// Schritt noch einmal geprueft, mit frischen Zahlen.
#[tauri::command]
async fn artefakt_bauen(
    halter: tauri::State<'_, Halter>,
    schluessel: String,
    fenster: tauri::AppHandle,
) -> Result<String, String> {
    let v = voraussetzungen_pruefen();
    if !v.fehlt.is_empty() {
        return Err(v.fehlt.join("\n"));
    }
    let w = std::path::PathBuf::from(v.wurzel.ok_or("keine Wurzel")?);
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let plan = myl_client::bezug::Bauplan::fuer(&w, &schluessel)?;
    plan.platz_pruefen(&plan.platzlage(&e))?;

    // ⚑ **Der Platz geht von der Reservierung an den Bau ueber.** Die
    // Summe aus belegt und reserviert bleibt die Freigabe; ohne diesen
    // Schritt hielte die Reservierung genau den Platz fest, den der Bau
    // gleich braucht.
    if e.kapazitaet.platte_gib.is_some() {
        if let Ok(mut g) = halter.platte.lock() {
            if let Some(r) = g.as_mut() {
                r.hergeben(plan.bedarf().spitze())?;
            }
        }
    }

    let f = fenster.clone();
    let sagen = move |z: myl_client::bezug::Zeile| {
        let _ = f.emit("bau-zeile", Bauzeile { phase: z.phase, text: z.text });
    };
    let ergebnis = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        myl_client::bezug::ausfuehren(&plan, &e, &sagen).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| format!("der Baufaden ist abgestuerzt: {e}"))?;
    // ⚑ Nach einem Bau, auch einem abgebrochenen, liegt anderes auf der
    //   Platte (Fund 555).
    belegung_vergessen();
    ergebnis
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Das Feld am Eingabefeld wartet nicht auf ein rechnendes Modell.**
    /// Haelt ein anderer Faden das Modell, antworten Stand und Saat sofort:
    /// der Stand als „belegt“, die Saat vorgemerkt fuer die naechste Antwort.
    /// 📌 Vorher fror das Oeffnen des Felds das ganze Fenster ein, bis die
    /// Antwort fertig war (gemeldet vom Projektinhaber, 2026-10-13).
    #[test]
    fn das_ausgabefeld_wartet_nicht_auf_das_modell() {
        let modell: std::sync::Arc<Mutex<Option<myl_client::Oertlichesmodell>>> = std::sync::Arc::new(Mutex::new(None));
        let e = myl_client::Einstellungen::default();
        let (gehalten, warten) = std::sync::mpsc::channel();
        let (los, frei) = std::sync::mpsc::channel::<()>();
        let halter = modell.clone();
        let rechnet = std::thread::spawn(move || {
            let _g = halter.lock().unwrap();
            gehalten.send(()).unwrap();
            let _ = frei.recv();
        });
        warten.recv().unwrap();
        let beginn = std::time::Instant::now();
        let stand = saatstand_aus(&modell, &e).expect("Stand");
        saat_einmal_in(&modell, 42).expect("vormerken");
        assert!(beginn.elapsed() < std::time::Duration::from_millis(500), "es wurde gewartet");
        assert!(stand.belegt);
        assert_eq!(saat_vorgemerkt(), Some(42));
        assert_eq!(saatstand_aus(&modell, &e).unwrap().naechste.as_deref(), Some("42"), "die vorgemerkte Saat steht im Stand");
        los.send(()).unwrap();
        rechnet.join().unwrap();
        // Frei und ohne Modell: nicht mehr belegt, die Saat bleibt vorgemerkt.
        let frei = saatstand_aus(&modell, &e).unwrap();
        assert!(!frei.belegt);
        assert_eq!(frei.naechste.as_deref(), Some("42"));
        *SAAT_VORGEMERKT.lock().unwrap() = None;
    }

    /// ⚑ **Ein geloeschtes Gespraech nimmt seine Tasks und Mitschnitte mit**,
    /// und nur seine: Tasks und Mitschnitte anderer Gespraeche bleiben, ein
    /// Task mitten in seiner Runde ebenso, und ein Ort, der zweimal genannt
    /// ist, zaehlt einmal.
    #[test]
    fn ein_geloeschtes_gespraech_nimmt_seines_mit() {
        let d = std::env::temp_dir().join(format!("myl-vergessen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let ablage = myl_local_agent::vorhaben::Ablage::neu(d.join("vorhaben"));
        let a = ablage.anlegen_fuer("erstes", 1, Some("g-1".into()), None).unwrap();
        let b = ablage.anlegen_fuer("zweites", 2, Some("g-1".into()), None).unwrap();
        let fremd = ablage.anlegen_fuer("fremdes", 3, Some("g-2".into()), None).unwrap();
        let arbeit = d.join("arbeit");
        for (sitzung, inhalt) in [("g-1", "geheim"), ("g-2", "bleibt")] {
            myl_local_agent::verlauf::schreiben(&arbeit, sitzung, "m", &[("user".into(), inhalt.into())]).unwrap();
        }
        let modell: Mutex<Option<myl_client::Oertlichesmodell>> = Mutex::new(None);
        let weg = vergessen_in("g-1", [arbeit.clone(), arbeit.clone()], &ablage, &modell, |k| k == b.kennung).unwrap();
        assert_eq!((weg.tasks, weg.mitschnitte), (1, 1));
        assert_eq!(weg.geblieben, vec![b.kennung.clone()], "der rechnende Task bleibt");
        let uebrig: Vec<String> = ablage.reihe().into_iter().map(|v| v.kennung).collect();
        assert!(!uebrig.contains(&a.kennung) && uebrig.contains(&b.kennung) && uebrig.contains(&fremd.kennung), "{uebrig:?}");
        let mitschnitte = myl_local_agent::verlauf::sitzungen(&arbeit);
        assert_eq!(mitschnitte.len(), 1, "nur der Mitschnitt des anderen Gespraechs bleibt: {mitschnitte:?}");
        assert!(mitschnitte[0].ends_with("g-2"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
