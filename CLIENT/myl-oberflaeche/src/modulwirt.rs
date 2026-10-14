//! **Module im Fenster**: dieselben Module wie in der Konsole, mit ihren
//! Befehlen, ihrem Modus, ihren Fragen an Modell und Mensch, und dazu mit
//! Ansichten im Sichtfeld.
//!
//! ⚑ **Die Verwaltung ist die der Konsole** (`myl_client::module::Modulwirt`):
//! Pruefen vor jedem Start, Neustart nach einer Aenderung, Takte, Budget.
//! Hier steht nur, was das Fenster anders macht: Es zeichnet nicht selbst,
//! sondern schickt jedes Ereignis als `modul-ereignis` an die Oberflaeche,
//! und eine Frage an den Menschen ist ein Dialog des Fensters.
//!
//! ⚑ **Gewaehlt, nicht getippt** (Festlegung des Projektinhabers,
//! 2026-10-12): Ein Modul laeuft, solange es in der Seitenleiste unter der
//! Werkzeugkiste gewaehlt ist ([`Fensterwirt::waehlen`]); seine Befehle sind
//! dort Knoepfe. Getippt gehen sie trotzdem, wie in der Konsole.
//!
//! ⚑ **Ein eigener Faden fuer den Takt.** Die Konsole holt Ereignisse ab,
//! wenn ihre Eingabezeile wartet; das Fenster hat keine Eingabezeile, die
//! wartet. Der Faden beginnt mit dem ersten Start eines Moduls und
//! schlaeft, solange keines laeuft.
//!
//! ⛔️ **Was ein Modul schreibt, ist Text, nie Markup.** Die Oberflaeche
//! setzt es ausschliesslich als `textContent`, und eine Ansicht ist eine
//! Beschreibung, die das Fenster selbst zeichnet; ein Modul kann im Fenster
//! nichts ausfuehren und keinen Dialog des Fensters nachahmen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use myl_client::module::Modulwirt;
use myl_module::nachricht::{AnModul, Nutzerfrage, Stil, Teil, VomModul};
use myl_module::wirt::Ereignis;
use serde::Serialize;
use tauri::Emitter;

/// Unter diesem Namen gehen alle Ereignisse der Module an die Oberflaeche.
pub const MODULEREIGNIS: &str = "modul-ereignis";

/// ⚑ **Die Masse, die ein Modul im Fenster bekommt**, in Zeichen. Das
/// Fenster hat keine Spalten; ein Modul richtet seine Zeilen danach aus,
/// und der Bereich des Modulfelds ist so breit gesetzt.
const BREITE: u16 = 100;
const HOEHE: u16 = 30;

/// Wie oft der Faden nachsieht, wenn ein Modul laeuft, und wie lange er
/// schlaeft, wenn keines laeuft.
const TAKT: std::time::Duration = std::time::Duration::from_millis(80);
const RUHE: std::time::Duration = std::time::Duration::from_millis(400);

static GEWECKT: AtomicBool = AtomicBool::new(false);

/// Der Lesefaden eines Moduls ruft das, sobald eine Nachricht ankommt.
fn wecken() {
    GEWECKT.store(true, Ordering::SeqCst);
}

/// **Die Module dieses Fensters.**
///
/// ⚑ **Alles hinter `Arc`**, damit ein Befehl einen Griff in seinen Faden
/// mitnehmen kann; ein `State` lebt nur so lange wie der Befehl.
#[derive(Default, Clone)]
pub struct Fensterwirt {
    wirt: Arc<Mutex<Modulwirt>>,
    /// Ob die Module schon einmal gefunden und geprueft wurden.
    geprueft: Arc<AtomicBool>,
    /// Ob der Taktfaden laeuft.
    faden: Arc<AtomicBool>,
}

/// Was ein Modul der Oberflaeche zeigt.
#[derive(Serialize, Clone)]
#[serde(tag = "art", rename_all = "snake_case")]
enum Anzeige {
    /// Text in das Protokoll des Modulfelds.
    Zeilen { name: String, text: String, stil: Stil },
    /// Fusszeile und Bereich unten haben sich geaendert.
    Stand { stand: Vec<Modulstand> },
    /// Der Modus beginnt (mit Banner) oder endet.
    Modus { name: String, aktiv: bool, banner: Vec<String>, was: String },
    /// Eine Frage an den Menschen; die Antwort geht an `modul_antwort`.
    Frage { name: String, id: u64, frage: Nutzerfrage },
    /// Ein Hinweis des Fensters ueber ein Modul (abgelehnt, belegt).
    Hinweis { name: String, text: String },
    /// Ein Agentenlauf des Moduls beginnt oder endet.
    Agent { name: String, laeuft: bool },
    /// Das Modul ist beendet.
    Beendet { name: String, grund: String },
    /// Eine Ansicht fuer das Sichtfeld, neu oder ersetzt.
    Ansicht { name: String, ansicht: myl_module::nachricht::Ansicht },
    /// Eine Ansicht faellt weg.
    AnsichtWeg { name: String, id: String },
}

/// So gross darf eine Datei einer Ansicht hoechstens sein.
const ANSICHTSDATEI_HOECHSTENS: u64 = 64 << 20;

/// **Der Stand eines laufenden Moduls**, fuer das Modulfeld.
#[derive(Serialize, Clone)]
pub struct Modulstand {
    name: String,
    was: String,
    modus: bool,
    fusszeile: Vec<Teil>,
    unten: Vec<Vec<Teil>>,
}

/// Ein Befehl eines gepruefen Moduls, fuer die Eingabe.
#[derive(Serialize)]
pub struct Modulbefehl {
    name: String,
    was: String,
    modul: String,
}

/// Die Angaben fuer den Start eines Moduls, als eigene Werte.
struct Startwerte {
    sprache: &'static str,
    modell: Option<String>,
    speicher: std::path::PathBuf,
}

/// Ob ein Modell geladen ist. ⚠️ Ohne zu warten: Haelt ein Lauf das
/// Modell, ist es geladen.
fn geladen(modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>) -> bool {
    match modell.try_lock() {
        Ok(g) => g.is_some(),
        Err(std::sync::TryLockError::WouldBlock) => true,
        Err(std::sync::TryLockError::Poisoned(_)) => false,
    }
}

impl Startwerte {
    fn jetzt(geladen: bool) -> Self {
        let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).ok();
        let deutsch = e.as_ref().is_none_or(|e| e.oberflaeche.sprache == myl_local_agent::agentenwahl::Sprache::De);
        Self {
            sprache: if deutsch { "de" } else { "en" },
            modell: e.as_ref().filter(|_| geladen).and_then(|e| modellkennung(&e.modell.artefakt)),
            speicher: myl_client::module::speicherpfad(),
        }
    }

    fn start(&self) -> myl_module::wirt::Start<'_> {
        myl_module::wirt::Start {
            konsole: env!("CARGO_PKG_VERSION"),
            sprache: self.sprache,
            modell: self.modell.clone(),
            breite: BREITE,
            hoehe: HOEHE,
            speicher: &self.speicher,
            wecker: Some(wecken),
            ansicht: true,
        }
    }
}

/// Die Kennung eines Artefakts fuer ein Modul: der Name seines Ordners.
pub fn modellkennung(artefakt: &str) -> Option<String> {
    std::path::Path::new(artefakt).file_name().map(|n| n.to_string_lossy().to_string()).filter(|n| !n.is_empty())
}

fn stand_aus(w: &Modulwirt) -> Vec<Modulstand> {
    w.laufend
        .iter()
        .map(|l| Modulstand {
            name: l.g.beschreibung.name.clone(),
            was: l.g.beschreibung.was.clone(),
            modus: l.modus,
            fusszeile: l.fusszeile.clone(),
            unten: l.unten.clone(),
        })
        .collect()
}

fn zeigen(app: &tauri::AppHandle, a: Anzeige) {
    let _ = app.emit(MODULEREIGNIS, a);
}

fn stand_zeigen(app: &tauri::AppHandle, wirt: &Arc<Mutex<Modulwirt>>) {
    if let Ok(w) = wirt.lock() {
        zeigen(app, Anzeige::Stand { stand: stand_aus(&w) });
    }
}

impl Fensterwirt {
    /// Findet und prueft die Module, beim ersten Mal oder wenn `neu`.
    fn pruefen(&self, neu: bool) -> Result<(), String> {
        if neu || !self.geprueft.swap(true, Ordering::SeqCst) {
            self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?.neu_pruefen();
        }
        Ok(())
    }

    /// Die Befehle aller gepruefen Module.
    pub fn befehle(&self, neu: bool) -> Result<Vec<Modulbefehl>, String> {
        self.pruefen(neu)?;
        let w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
        Ok(w.gefunden
            .iter()
            .filter_map(|f| f.ergebnis.as_ref().ok())
            .flat_map(|g| {
                g.beschreibung.befehle.iter().map(|b| Modulbefehl { name: b.name.clone(), was: b.was.clone(), modul: g.beschreibung.name.clone() })
            })
            .collect())
    }

    /// Der Stand aller laufenden Module.
    pub fn stand(&self) -> Vec<Modulstand> {
        self.wirt.lock().map(|w| stand_aus(&w)).unwrap_or_default()
    }

    /// **Ein Befehl eines Moduls**: starten, falls noetig, und weitergeben.
    /// Gibt den Namen des Moduls zurueck.
    pub fn befehl(&self, zeile: &str, app: &tauri::AppHandle, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>) -> Result<String, String> {
        self.pruefen(false)?;
        let werte = Startwerte::jetzt(geladen(modell));
        let name = {
            let mut w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
            let name = w.modul_der_zeile(zeile).ok_or_else(|| format!("kein gepruefetes Modul fuer {}", zeile.split_whitespace().next().unwrap_or("")))?;
            w.starten(&name, &werte.start())?;
            if let Some(l) = w.laufendes(&name) {
                l.letzter_befehl = Some(zeile.to_string());
                l.lauf.senden(&AnModul::Befehl { zeile: zeile.to_string() });
            }
            name
        };
        self.faden_starten(app, modell);
        wecken();
        Ok(name)
    }

    /// **Ein Modul waehlen**: Es startet (ohne Befehl, nur mit dem
    /// Handschlag), und jedes andere laufende Modul endet. `None` beendet
    /// alle. ⚑ Ohne Befehl, damit die Wahl allein nichts ausloest, was ein
    /// Modul erst auf einen Befehl hin tut (beim Handel etwa den Modus mit
    /// Echtgeld).
    pub fn waehlen(&self, name: Option<&str>, app: &tauri::AppHandle, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>) -> Result<(), String> {
        self.pruefen(false)?;
        let andere: Vec<String> = {
            let w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
            w.laufend.iter().map(|l| l.g.beschreibung.name.clone()).filter(|n| Some(n.as_str()) != name).collect()
        };
        for n in andere {
            self.beenden(&n, app)?;
        }
        let Some(name) = name else { return Ok(()) };
        let werte = Startwerte::jetzt(geladen(modell));
        self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?.starten(name, &werte.start())?;
        self.faden_starten(app, modell);
        stand_zeigen(app, &self.wirt);
        Ok(())
    }

    /// **Die Bytes einer Datei einer Ansicht**: im Ordner des gepruefen
    /// Moduls, ohne symbolischen Verweis hinaus, hoechstens
    /// [`ANSICHTSDATEI_HOECHSTENS`].
    pub fn datei(&self, name: &str, datei: &str) -> Result<Vec<u8>, String> {
        let ordner = {
            let w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
            w.gefunden.iter().filter_map(|f| f.ergebnis.as_ref().ok()).find(|g| g.beschreibung.name == name).map(|g| g.ordner.clone()).ok_or_else(|| format!("{name} ist kein geprueftes Modul"))?
        };
        ansichtsdatei_lesen(&ordner, datei)
    }

    /// **Die Antwort des Menschen** auf eine Frage eines Moduls.
    pub fn antworten(&self, name: &str, id: u64, antwort: Option<String>) -> Result<(), String> {
        let mut w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
        let l = w.laufendes(name).ok_or_else(|| format!("{name} laeuft nicht"))?;
        l.lauf.senden(&AnModul::Nutzerantwort { id, antwort });
        Ok(())
    }

    /// Beendet ein Modul.
    pub fn beenden(&self, name: &str, app: &tauri::AppHandle) -> Result<(), String> {
        {
            let mut w = self.wirt.lock().map_err(|_| "der Modulwirt klemmt")?;
            if let Some(i) = w.laufend.iter().position(|l| l.g.beschreibung.name == name) {
                let mut l = w.laufend.remove(i);
                l.lauf.beenden();
            }
        }
        myl_client::module::protokoll(name, "im Fenster beendet");
        zeigen(app, Anzeige::Beendet { name: name.to_string(), grund: "beendet".into() });
        stand_zeigen(app, &self.wirt);
        Ok(())
    }

    /// Beendet alle Module (beim Schliessen des Fensters).
    pub fn alle_beenden(&self) {
        if let Ok(mut w) = self.wirt.lock() {
            w.alle_beenden();
        }
    }

    /// Sagt allen laufenden Modulen, welches Modell jetzt geladen ist.
    pub fn modell_melden(&self, kennung: Option<String>) {
        if let Ok(mut w) = self.wirt.lock() {
            w.allen(&AnModul::Modell { kennung });
        }
    }

    fn faden_starten(&self, app: &tauri::AppHandle, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>) {
        if self.faden.swap(true, Ordering::SeqCst) {
            return;
        }
        let (app, wirt, modell) = (app.clone(), self.wirt.clone(), modell.clone());
        std::thread::spawn(move || loop {
            let laeuft = wirt.lock().map(|w| w.irgendeins_laeuft()).unwrap_or(false);
            if !laeuft {
                std::thread::sleep(RUHE);
                continue;
            }
            // Bis zur naechsten Nachricht oder zum naechsten Takt.
            let bis = std::time::Instant::now() + TAKT;
            while std::time::Instant::now() < bis && !GEWECKT.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            GEWECKT.store(false, Ordering::SeqCst);
            let werte = Startwerte::jetzt(geladen(&modell));
            let ereignisse = match wirt.lock() {
                Ok(mut w) => w.abholen(&werte.start()),
                Err(_) => return,
            };
            for (name, e) in ereignisse {
                ereignis(&app, &wirt, &modell, &name, e);
            }
        });
    }
}

fn ereignis(app: &tauri::AppHandle, wirt: &Arc<Mutex<Modulwirt>>, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>, name: &str, e: Ereignis) {
    match e {
        Ereignis::Nachricht(n) => nachricht(app, wirt, modell, name, n),
        Ereignis::Abgelehnt(f) => {
            myl_client::module::protokoll(name, &format!("abgelehnt: {f}"));
            if wirt.lock().is_ok_and(|mut w| w.ablehnung_zeigen(name)) {
                zeigen(app, Anzeige::Hinweis { name: name.to_string(), text: f });
            }
        }
        Ereignis::Fehlerausgabe(z) => myl_client::module::protokoll(name, &z),
        Ereignis::Beendet(grund) => {
            myl_client::module::protokoll(name, &format!("beendet: {grund}"));
            if let Ok(mut w) = wirt.lock() {
                w.laufend.retain(|l| l.g.beschreibung.name != name);
            }
            zeigen(app, Anzeige::Beendet { name: name.to_string(), grund });
            stand_zeigen(app, wirt);
        }
    }
}

fn nachricht(app: &tauri::AppHandle, wirt: &Arc<Mutex<Modulwirt>>, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>, name: &str, n: VomModul) {
    let senden = |a: AnModul| {
        if let Ok(mut w) = wirt.lock() {
            if let Some(l) = w.laufendes(name) {
                l.lauf.senden(&a);
            }
        }
    };
    match n {
        VomModul::Hallo { .. } => {}
        VomModul::Zeilen { text, stil } => zeigen(app, Anzeige::Zeilen { name: name.to_string(), text, stil }),
        VomModul::Protokoll { text } => myl_client::module::protokoll(name, &text),
        VomModul::WeckenIn { ms } => {
            if let Ok(mut w) = wirt.lock() {
                if let Some(l) = w.laufendes(name) {
                    l.naechster_takt = Some(std::time::Instant::now() + std::time::Duration::from_millis(ms.clamp(100, 3_600_000)));
                }
            }
        }
        VomModul::Fusszeile { teile } => {
            if let Ok(mut w) = wirt.lock() {
                if let Some(l) = w.laufendes(name) {
                    l.fusszeile = teile;
                }
            }
            stand_zeigen(app, wirt);
        }
        VomModul::Unten { zeilen } => {
            if let Ok(mut w) = wirt.lock() {
                if let Some(l) = w.laufendes(name) {
                    l.unten = zeilen;
                    l.unten_da = true;
                }
            }
            stand_zeigen(app, wirt);
        }
        VomModul::Modus { aktiv: true, banner } => modus_betreten(app, wirt, name, banner.unwrap_or_default()),
        VomModul::Modus { aktiv: false, .. } => modus_verlassen(app, wirt, name),
        VomModul::ModellFragen { id, vorspann, frage, grenze, denken, strom, gierig } => {
            // ⚑ **In einem eigenen Faden**: Das Modell kann gerade einen
            //   Chat rechnen; bis es frei ist, laufen die Takte weiter.
            // 📌 **Erst die Sperre loslassen, dann antworten**: `senden`
            //    sperrt den Wirt selbst, und eine zweite Sperre im selben
            //    Faden haelt ihn fuer immer an.
            let gebucht = match wirt.lock() {
                Ok(mut w) => w.budget_buchen(name, grenze).map(|()| w.laufendes(name).map(|l| l.lauf.sender())),
                Err(_) => return,
            };
            let sender = match gebucht {
                Ok(Some(s)) => s,
                Ok(None) => return,
                Err(f) => {
                    senden(AnModul::Modellantwort { id, antwort: Err(f) });
                    return;
                }
            };
            let modell = modell.clone();
            let name = name.to_string();
            std::thread::spawn(move || {
                let antwort = match modell.lock() {
                    Ok(mut g) => match g.as_mut() {
                        Some(m) => {
                            let beobachter: Option<myl_client::module::Stueckbeobachter> = strom.then(|| {
                                let s = sender.clone();
                                Box::new(move |st: myl_local_agent::textstrom::Stueck| {
                                    if let myl_local_agent::textstrom::Stueck::Text(text) = st {
                                        s.senden(&AnModul::Modellstueck { id, text });
                                    }
                                }) as myl_client::module::Stueckbeobachter
                            });
                            myl_client::module::modell_fragen(m, &vorspann, &frage, grenze as usize, Some(denken), gierig, beobachter)
                        }
                        None => Err("es ist kein Modell geladen".to_string()),
                    },
                    Err(_) => Err("der Modellhalter ist vergiftet".to_string()),
                };
                if let Err(f) = &antwort {
                    myl_client::module::protokoll(&name, &format!("Modellfrage: {f}"));
                }
                sender.senden(&AnModul::Modellantwort { id, antwort });
            });
        }
        VomModul::Agentenlauf { id, auftrag } => {
            let g = wirt.lock().ok().and_then(|w| w.aktives().filter(|l| l.g.beschreibung.name == name).map(|l| (l.g.clone(), l.lauf.sender())));
            let Some((g, sender)) = g else {
                senden(AnModul::Agentenergebnis { id, antwort: Err("ein Agentenlauf geht nur im Modus des Moduls".into()) });
                return;
            };
            let (app, modell, name) = (app.clone(), modell.clone(), name.to_string());
            std::thread::spawn(move || {
                zeigen(&app, Anzeige::Agent { name: name.clone(), laeuft: true });
                let antwort = agentenlauf(&app, &modell, &g, &auftrag);
                zeigen(&app, Anzeige::Agent { name, laeuft: false });
                sender.senden(&AnModul::Agentenergebnis { id, antwort });
            });
        }
        VomModul::NutzerFragen { id, frage } => zeigen(app, Anzeige::Frage { name: name.to_string(), id, frage }),
        VomModul::Ansicht { ansicht } => zeigen(app, Anzeige::Ansicht { name: name.to_string(), ansicht }),
        VomModul::AnsichtWeg { id } => zeigen(app, Anzeige::AnsichtWeg { name: name.to_string(), id }),
    }
}

/// Liest eine Datei einer Ansicht aus `ordner`. ⚠️ Der Pfad ist schon beim
/// Pruefen der Nachricht gesaeubert; hier wird er trotzdem noch einmal
/// geprueft, denn der Befehl dazu kommt von der Oberflaeche, und ein
/// symbolischer Verweis im Modulordner koennte hinauszeigen.
fn ansichtsdatei_lesen(ordner: &std::path::Path, datei: &str) -> Result<Vec<u8>, String> {
    if !myl_module::beschreibung::sicherer_pfad(datei) {
        return Err(format!("{datei}: kein Pfad im Modulordner"));
    }
    let wurzel = ordner.canonicalize().map_err(|f| format!("{}: {f}", ordner.display()))?;
    let pfad = ordner.join(datei).canonicalize().map_err(|f| format!("{datei}: {f}"))?;
    if !pfad.starts_with(&wurzel) {
        return Err(format!("{datei} zeigt aus dem Modulordner hinaus"));
    }
    let m = std::fs::metadata(&pfad).map_err(|f| format!("{datei}: {f}"))?;
    if !m.is_file() || m.len() > ANSICHTSDATEI_HOECHSTENS {
        return Err(format!("{datei}: keine Datei oder groesser als {} MiB", ANSICHTSDATEI_HOECHSTENS >> 20));
    }
    std::fs::read(&pfad).map_err(|f| format!("{datei}: {f}"))
}

/// **Der Modus eines Moduls beginnt.** Es gibt hoechstens einen; ein zweites
/// Modul bekommt einen Hinweis statt des Modus.
fn modus_betreten(app: &tauri::AppHandle, wirt: &Arc<Mutex<Modulwirt>>, name: &str, banner: Vec<String>) {
    let was = {
        let Ok(mut w) = wirt.lock() else { return };
        if let Some(anderes) = w.aktives().map(|l| l.g.beschreibung.name.clone()).filter(|a| a != name) {
            drop(w);
            zeigen(app, Anzeige::Hinweis { name: name.to_string(), text: format!("Der Modus von {anderes} läuft noch; erst ihn beenden.") });
            return;
        }
        let Some(l) = w.laufendes(name) else { return };
        if l.modus {
            return;
        }
        l.modus = true;
        l.aktivierung = l.letzter_befehl.clone().or(l.aktivierung.take());
        l.g.beschreibung.was.clone()
    };
    zeigen(app, Anzeige::Modus { name: name.to_string(), aktiv: true, banner, was });
    stand_zeigen(app, wirt);
}

/// **Der Modus eines Moduls endet**: Takte, Fusszeile und Bereich unten
/// gehen mit.
fn modus_verlassen(app: &tauri::AppHandle, wirt: &Arc<Mutex<Modulwirt>>, name: &str) {
    {
        let Ok(mut w) = wirt.lock() else { return };
        let Some(l) = w.laufendes(name) else { return };
        if !l.modus {
            return;
        }
        l.modus = false;
        l.naechster_takt = None;
        l.fusszeile.clear();
        l.unten.clear();
    }
    zeigen(app, Anzeige::Modus { name: name.to_string(), aktiv: false, banner: Vec::new(), was: String::new() });
    stand_zeigen(app, wirt);
}

/// **Ein Agentenlauf, den ein Modul erbittet**: mit seiner Kiste zusaetzlich
/// zur gewaehlten, in seinem Arbeitsordner, ohne Netz und ohne Blick, wie in
/// der Konsole (`Modulzusatz::im_modus`). Ohne Verlauf: Den Auftrag stellt
/// das Modul zusammen.
fn agentenlauf(app: &tauri::AppHandle, modell: &Arc<Mutex<Option<myl_client::Oertlichesmodell>>>, g: &myl_module::signatur::Geprueft, auftrag: &str) -> Result<String, String> {
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad())?;
    let zusatz = myl_client::module::Modulzusatz::aus(g)?;
    let mut agent = e.agent.clone();
    zusatz.anwenden(&mut agent);
    zusatz.im_modus(&mut agent);
    agent.netzsaat = Some(auftrag.to_string());
    let nachfrage = crate::nachfrage_ohne(app, agent.modus, zusatz.ohne_nachfrage.clone());
    let ruestung = myl_local_agent::ruestung::ruesten_mit(&agent, myl_client::Ansageform::Amtlich, myl_local_agent::kisten::kiste_der_gilt(&agent), Vec::new(), nachfrage)?;
    myl_local_agent::notaus::zuruecksetzen();
    let mut g = modell.lock().map_err(|_| "der Modellhalter ist vergiftet".to_string())?;
    let m = g.as_mut().ok_or("es ist kein Modell geladen")?;
    ruestung.nachschlagebudget_zuruecksetzen();
    let ergebnis = myl_local_agent::lauf::fahren_im_gespraech(m, &ruestung, agent.schritte as usize, true, e.modell.token as u32, &[], auftrag, None);
    myl_local_agent::skills::modusordner_setzen(None);
    ergebnis.antwort.clone().ok_or_else(|| "keine Antwort".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ Ein Modul erfaehrt das Modell beim Namen seines Ordners, und ohne
    /// Artefakt gar keines.
    #[test]
    fn modellkennung_aus_dem_ordner() {
        assert_eq!(modellkennung("ARTEFAKTE/myelith-0.6b").as_deref(), Some("myelith-0.6b"));
        assert_eq!(modellkennung(""), None);
    }

    /// ⚑ Eine Datei einer Ansicht kommt nur aus dem Modulordner, auch
    /// nicht ueber einen symbolischen Verweis hinaus.
    #[test]
    fn ansichtsdatei_nur_im_ordner() {
        let wurzel = std::env::temp_dir().join(format!("myl-ansicht-{}", std::process::id()));
        let ordner = wurzel.join("modul");
        std::fs::create_dir_all(ordner.join("daten")).unwrap();
        std::fs::write(ordner.join("daten/a.png"), b"bild").unwrap();
        std::fs::write(wurzel.join("geheim.png"), b"fremd").unwrap();
        assert_eq!(ansichtsdatei_lesen(&ordner, "daten/a.png").unwrap(), b"bild");
        assert!(ansichtsdatei_lesen(&ordner, "../geheim.png").is_err());
        assert!(ansichtsdatei_lesen(&ordner, "daten").is_err(), "ein Ordner ist keine Datei");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(wurzel.join("geheim.png"), ordner.join("daten/b.png")).unwrap();
            assert!(ansichtsdatei_lesen(&ordner, "daten/b.png").is_err(), "Verweis hinaus");
        }
        let _ = std::fs::remove_dir_all(&wurzel);
    }

    /// ⚑ Ohne geladenes Modell sagt der Handschlag keines, auch wenn eines
    /// eingestellt ist.
    #[test]
    fn ohne_geladenes_modell_keines() {
        let leer: Arc<Mutex<Option<myl_client::Oertlichesmodell>>> = Arc::new(Mutex::new(None));
        assert!(!geladen(&leer));
        assert_eq!(Startwerte::jetzt(false).modell, None);
    }
}
