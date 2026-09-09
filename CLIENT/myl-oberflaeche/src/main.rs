//! Die Oberflaeche von Myelith, Ruecken.
//!
//! # ⚑ Was hier NICHT stehen darf
//!
//! **Keine Logik.** Die Oberflaeche ruft dieselben Unterbefehle wie das
//! Kommandozeilenwerkzeug, und der Grund ist nicht Aesthetik.
//! Zwei Wege zu derselben Sache laufen auseinander, und der zweite ist
//! immer der schlechter gepruefte: `myl` hat sechsundfuenfzig
//! Pruefungen, ein nachgebauter Ruecken haette null.
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
    artefakt: String,
    token: usize,
    denken: bool,
    schritte: u32,
    bezeugtes: bool,
    wurzel: Option<String>,
    schreiben: bool,
    /// Was `kap.kerne` sagt.
    kerne_eingestellt: Option<usize>,
    /// ⚑ Und was davon **wirkt**. Eine Grenze ueber der Maschine hebt
    /// sie nicht an, und der Unterschied gehoert sichtbar.
    kerne_wirksam: usize,
    /// ⚑ **Die Betriebsart als dauerhafter Zustand.** Auf der
    /// Kommandozeile genuegt ein Satz beim Start; hier nicht. Wer nicht
    /// sieht, in welcher Betriebsart er arbeitet, hat keine Wahl
    /// getroffen, sondern eine geerbt.
    betriebsart: String,
    /// ⚑ **Der ganze Satz dahinter.** Die Kurzform steht im Kopf und
    /// muss in eine Marke passen; der Grund gehoert trotzdem irgendwo
    /// hin, und zwar dort, wo man ihn sucht, naemlich am selben Ding.
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
fn einstellungen() -> Result<Ansicht, String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let e = myl_client::Einstellungen::lesen(&pfad)?;
    if let Some(n) = e.kapazitaet.kerne {
        myl_client::kapazitaet::kerne_setzen(n);
    }
    Ok(Ansicht {
        artefakt: e.modell.artefakt.clone(),
        token: e.modell.token,
        denken: e.modell.denken,
        schritte: e.agent.schritte,
        bezeugtes: e.agent.auch_bezeugtes,
        wurzel: e.agent.wurzel.clone(),
        schreiben: e.agent.schreiben,
        kerne_eingestellt: e.kapazitaet.kerne,
        kerne_wirksam: myl_client::kapazitaet::kerne(),
        betriebsart: kurzform(&e.agent).0.into(),
        betriebsart_warum: kurzform(&e.agent).1.into(),
        pfad: pfad.display().to_string(),
    })
}

/// Was ein Artefakt hergibt, ohne es zu laden.
///
/// ⚑ **Ohne es zu laden**, denn das kostet bei einem 4B-Modell zehn
/// Sekunden. Die Vorlage folgt aus der Familie und steht im Katalog.
/// Die setzbaren Felder mit ihrer Art.
///
/// ⚑ **Aus der Kiste und nicht hier aufgezaehlt.** Eine zweite Liste im
/// Fenster liefe irgendwann auseinander, und dann zeigt die Oberflaeche
/// ein Feld, das der Setzer nicht kennt.
#[tauri::command]
fn felder() -> Vec<(String, String)> {
    myl_client::einstellungen::FELDER
        .iter()
        .map(|(n, a)| (n.to_string(), format!("{a:?}")))
        .collect()
}

/// Setzt ein Feld und schreibt die Ablage.
///
/// ⛑ **Auch hier keine eigene Logik.** Was `aus` heisst und welche
/// Felder es gibt, weiss die Kiste; dieser Befehl reicht durch und
/// speichert.
#[tauri::command]
fn setzen(feld: String, wert: String) -> Result<(), String> {
    let pfad = myl_client::Einstellungen::vorgabepfad();
    let mut e = myl_client::Einstellungen::lesen(&pfad)?;
    e.setzen(&feld, &wert)?;
    e.schreiben(&pfad)
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
/// Vollmacht fehlen (Fahrplan 2.2 bis 2.5b). Er steht trotzdem in der
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
            // der Verzeichnisname `qwen3-4b`. Steht er nicht drin,
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
        warum: "Noch nicht verdrahtet: Dem Klienten fehlen Knotenadresse und Vollmacht \
                (Fahrplan 2.2 bis 2.5b). Bis dahin rechnet diese Maschine."
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
        if let Some(n) = v.get("anzeigename").and_then(|x| x.as_str()) {
            let herkunft = v.get("grundmodell").and_then(|x| x.as_str()).unwrap_or("");
            aus.insert(
                k.clone(),
                if herkunft.is_empty() { n.to_string() } else { format!("{n}  (aus {herkunft})") },
            );
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

#[tauri::command]
fn modell(artefakt: String) -> Result<String, String> {
    let pfad = artefakt_absolut(&artefakt);
    let m = myl_client::Oertlichesmodell::laden(&pfad)
        .map_err(|f| mit_zugriffshinweis(f, &pfad))?;
    Ok(format!("{:?}", m.vorlage()))
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
}

/// Laedt das Artefakt aus den Einstellungen.
#[tauri::command]
async fn modell_laden(halter: tauri::State<'_, Halter>) -> Result<String, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    if e.modell.artefakt.is_empty() {
        return Err("es ist kein Artefakt eingestellt".into());
    }
    let anfang = std::time::Instant::now();
    let pfad = artefakt_absolut(&e.modell.artefakt);
    let mut m = myl_client::Oertlichesmodell::laden(&pfad)
        .map_err(|f| mit_zugriffshinweis(f, &pfad))?;
    m.grenze = e.modell.token;
    m.denken = e.modell.denken;
    let vorlage = format!("{:?}", m.vorlage());
    let dauer = anfang.elapsed().as_secs_f64();
    *halter.modell.lock().map_err(|_| "der Modellhalter ist vergiftet")? = Some(m);
    Ok(format!("{} ({vorlage}), in {dauer:.1} s geladen", e.modell.artefakt))
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
        Ok(myl_client::lauf::fahren(m, &ruestung, schritte, !gesperrt, grenze, &auftrag))
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
        m.chat("lokal", &n, Some(grenze)).map_err(|f| f.to_string()).map(|a| a.text)
    })
    .await
    .map_err(|e| format!("der Rechenfaden ist abgestuerzt: {e}"))??;

    Ok(Antwort { text, sekunden: (anfang.elapsed().as_secs_f64() * 10.0).round() / 10.0 })
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
        S::Aufruf { name, argumente } => {
            Zeile { art: "aufruf", text: format!("{name} {argumente}") }
        }
        S::Unlesbar(t) => Zeile { art: "unlesbar", text: t.clone() },
        S::Ergebnis(t) => Zeile { art: "ergebnis", text: t.clone() },
        S::Antwort(t) => Zeile { art: "antwort", text: t.clone() },
    }
}

fn main() {
    tauri::Builder::default()
        .manage(Halter::default())
        .invoke_handler(tauri::generate_handler![
            einstellungen,
            felder,
            setzen,
            modell,
            modell_laden,
            agent_fahren,
            frage,
            werkzeuge,
            modelle,
            katalog,
            voraussetzungen,
            artefakt_bauen,
            gespraech_ausgeben
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
    lizenz: String,
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
fn wurzel_suchen() -> Option<std::path::PathBuf> {
    // ⛑ **Zwei Anfaenge, und der zweite ist der wichtige.** Aus dem
    // Finder gestartet bekommt ein Programm auf macOS das
    // Arbeitsverzeichnis `/`; von dort findet sich kein Klon. Das
    // Programm selbst liegt aber im Baum (`target-shared/...`), und von
    // seinem Pfad aus fuehrt der Weg nach oben ans Ziel.
    //
    // Das Arbeitsverzeichnis bleibt vorn, damit ein Aufruf aus einem
    // anderen Klon heraus auch dessen Artefakte nimmt.
    let anfaenge = [std::env::current_dir().ok(), eigener_ordner()];
    for anfang in anfaenge.into_iter().flatten() {
        let mut p = anfang;
        loop {
            if p.join("INTEGER_LLM/scripts/build_artifacts.sh").is_file() {
                return Some(p);
            }
            if !p.pop() {
                break;
            }
        }
    }
    None
}

/// Das Verzeichnis, in dem dieses Programm liegt.
fn eigener_ordner() -> Option<std::path::PathBuf> {
    let p = std::env::current_exe().ok()?;
    // ⚑ Aufgeloest, denn in einem `.app` fuehrt der Weg ueber
    // `Contents/MacOS`, und Verweise waeren sonst nicht zu verfolgen.
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    p.parent().map(|q| q.to_path_buf())
}

/// Schreibt ein Gespraech als Markdown neben die Einstellungen.
///
/// # ⚑ Warum hier und nicht ueber einen Speichern-Dialog
///
/// Ein Dialog braucht die Dateiwahl von Tauri, also ein weiteres
/// Zusatzstueck und eine weitere Berechtigung. Fuer „das Gespraech
/// hierher legen und mir sagen wohin" reicht ein Ort, den die
/// Anwendung ohnehin benutzt: das Verzeichnis der Einstellungen. Der
/// Pfad kommt zurueck und steht in der Meldung, also weiss jeder, wo
/// es liegt.
///
/// ⛑ **Der Name wird entschaerft und nicht uebernommen.** Ein Titel
/// kommt aus dem ersten Satz eines Gespraechs und kann alles
/// enthalten, `/` und `..` eingeschlossen; ungeprueft uebernommen
/// schriebe das Fenster irgendwohin. Erlaubt sind Buchstaben, Ziffern,
/// Strich und Unterstrich, alles andere wird zu einem Strich.
#[tauri::command]
fn gespraech_ausgeben(titel: String, inhalt: String) -> Result<String, String> {
    let ordner = myl_client::Einstellungen::vorgabepfad()
        .parent()
        .ok_or("die Einstellungen haben kein Verzeichnis")?
        .join("gespraeche");
    std::fs::create_dir_all(&ordner).map_err(|e| format!("{}: {e}", ordner.display()))?;

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
    std::fs::write(&ziel, inhalt).map_err(|e| format!("{}: {e}", ziel.display()))?;
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
/// `INTEGER_LLM/artifacts/qwen3-4b`, und das ist relativ zu einem
/// Arbeitsverzeichnis, das dort `/` ist. Ein absoluter Pfad in den
/// Einstellungen bliebe unberuehrt.
fn artefakt_absolut(pfad: &str) -> String {
    let p = std::path::Path::new(pfad);
    if p.is_absolute() || pfad.is_empty() {
        return pfad.to_string();
    }
    match wurzel_suchen() {
        Some(w) => w.join(p).display().to_string(),
        None => pfad.to_string(),
    }
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
            lizenz: text(v, "lizenz"),
            parameter: text(v, "parameter"),
            grundmodell: text(v, "grundmodell"),
            gewichte: text(v, "gewichte_anzeige"),
            artefakt: text(v, "artefakt_anzeige"),
            status: text(v, "status"),
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
