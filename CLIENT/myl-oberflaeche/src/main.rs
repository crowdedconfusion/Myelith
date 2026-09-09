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

#[tauri::command]
fn modell(artefakt: String) -> Result<String, String> {
    let m = myl_client::Oertlichesmodell::laden(&artefakt)?;
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
    let mut m = myl_client::Oertlichesmodell::laden(&e.modell.artefakt)?;
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
            frage
        ])
        .run(tauri::generate_context!())
        .expect("die Oberflaeche liess sich nicht starten");
}
