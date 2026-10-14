//! **Module fuer Konsole und Fenster**: wo sie liegen, und was ein
//! gewaehltes Modul einem Lauf mitbringt.
//!
//! ⚑ **Eine Stelle fuer beide Oberflaechen** (2026-10-12). Bis dahin kannte
//! nur die Konsole Module; seit das Fenster eines waehlen kann, braeuchte es
//! Orte, Pruefung und Zusatz ein zweites Mal. Das Pruefen selbst steht in
//! `myl_module::fund`; hier stehen die Orte dieses Rechners und der
//! [`Modulzusatz`].
//!
//! ⚑ **Ein Modul ergaenzt, es ersetzt nicht** (Festlegung des
//! Projektinhabers, 2026-10-12): Seine Werkzeugkiste liegt ueber der
//! gewaehlten (`myl_local_agent::kisten::ordnerkette`), seine Skills kommen
//! dazu, und seine Werkzeuge laufen mit der gepruefen Kopie seines
//! Programms.

use std::path::{Path, PathBuf};

use myl_module::fund::Fund;
use myl_module::signatur::Geprueft;
use myl_module::beschreibung::Befehl as Modulbefehl;
use myl_module::nachricht::{AnModul, Teil};
use myl_module::signatur::{self, Versionsstand, Vertrauensliste};
use myl_module::wirt::{Ereignis, Lauf, Start};
use std::time::{Instant, SystemTime};

/// Wo Module an der Wurzel des Repositoriums liegen.
pub const MODULORDNER: &str = "MODULES";
/// Wo installierte Module neben den Einstellungen liegen.
pub const NUTZERMODULE: &str = "module";
const VERTRAUEN: &str = "vertrauen.json";
const VERSIONEN: &str = "modulversionen.json";
const SPEICHER: &str = "modulspeicher";

/// **Unter diesem Namen bekommt ein Werkzeug des Moduls die gepruefte
/// Kopie seines Programms**, statt das Programm im Modulordner zu starten.
pub const PROGRAMMVARIABLE: &str = "MYL_MODUL_PROGRAMM";

/// Der Ordner der Einstellungen.
pub fn einstellungsordner() -> PathBuf {
    crate::Einstellungen::vorgabepfad().parent().map(Path::to_path_buf).unwrap_or_default()
}

/// Die Vertrauensliste: welche Schluessel Module signieren duerfen.
pub fn vertrauenspfad() -> PathBuf {
    einstellungsordner().join(VERTRAUEN)
}

/// Die zuletzt gesehenen Versionen (gegen das Zuruecksetzen).
pub fn versionspfad() -> PathBuf {
    einstellungsordner().join(VERSIONEN)
}

/// Wohin die gepruefen Programme kopiert werden.
pub fn speicherpfad() -> PathBuf {
    einstellungsordner().join(SPEICHER)
}

/// Die Orte, an denen Module liegen: `MODULES/` an der Wurzel, dann
/// `module/` neben den Einstellungen.
pub fn wurzeln() -> Vec<PathBuf> {
    let mut aus = Vec::new();
    if let Some(w) = myl_local_agent::ort::wurzel() {
        aus.push(w.join(MODULORDNER));
    }
    aus.push(einstellungsordner().join(NUTZERMODULE));
    aus
}

/// **Findet und prueft alle Module** an den Orten dieses Rechners.
/// `verboten` nennt einen Grund, ein Modul abzulehnen (die Konsole: ein
/// Hauptbefehl, den sie selbst belegt).
pub fn finden(verboten: &dyn Fn(&Geprueft) -> Option<String>) -> Vec<Fund> {
    myl_module::fund::finden(&wurzeln(), &vertrauenspfad(), &versionspfad(), verboten)
}

/// **Was ein gewaehltes Modul einem Lauf mitbringt.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modulzusatz {
    pub name: String,
    /// Seine Werkzeugkiste; sie ergaenzt die gewaehlte.
    pub kiste: Option<PathBuf>,
    /// Seine Skills; sie kommen zu denen des Arbeitsordners dazu.
    pub skills: Option<PathBuf>,
    /// Sein Arbeitsordner (fuer den Modus des Moduls in der Konsole).
    pub arbeit: Option<PathBuf>,
    /// Werkzeuge seiner Kiste, die nur lesen und deshalb nicht nachfragen
    /// (`ohne_nachfrage` in `kiste.json`).
    pub ohne_nachfrage: Vec<String>,
    /// Die gepruefte Kopie seines Programms, falls es nativ laeuft.
    pub programm: Option<PathBuf>,
}

impl Modulzusatz {
    /// **Aus einem gepruefen Modul.** Legt die gepruefte Programmkopie ab
    /// und traegt sie als Umgebung fuer die Werkzeuge seiner Kiste ein.
    pub fn aus(g: &Geprueft) -> Result<Self, String> {
        let b = &g.beschreibung;
        let kiste = b.kiste.as_ref().map(|k| g.ordner.join(k));
        let programm = match &b.laufzeit {
            myl_module::beschreibung::Laufzeit::Nativ { .. } => Some(myl_module::wirt::ablegen(g, &speicherpfad())?),
            myl_module::beschreibung::Laufzeit::Wasm { .. } => None,
        };
        if let (Some(k), Some(p)) = (&kiste, &programm) {
            myl_local_agent::kisten::umgebung_setzen(k, vec![(PROGRAMMVARIABLE.to_string(), p.display().to_string())]);
        }
        Ok(Self {
            name: b.name.clone(),
            ohne_nachfrage: kiste.as_deref().map(ohne_nachfrage_der_kiste).unwrap_or_default(),
            skills: b.skills.as_ref().map(|s| g.ordner.join(s)),
            arbeit: b.arbeit.as_ref().map(|a| g.ordner.join(a)),
            kiste,
            programm,
        })
    }

    /// **Haengt den Zusatz an die Agenteneinstellung eines Laufs**: die Kiste
    /// zuletzt in die Kette, die Skills als Ordner des Moduls.
    pub fn anwenden(&self, agent: &mut myl_local_agent::agentenwahl::Agenteneinstellung) {
        agent.modulkiste = self.kiste.clone();
        myl_local_agent::skills::modusordner_setzen(self.skills.clone());
    }

    /// **Die Regeln im Modus eines Moduls**, fuer einen Agentenlauf, den das
    /// Modul erbittet: in seinem Arbeitsordner (nur dort schreibend), ohne
    /// Netz und ohne Blick. Ein bloss eingestelltes Modul ergaenzt nur die
    /// Werkzeuge und bekommt diese Regeln nicht.
    pub fn im_modus(&self, agent: &mut myl_local_agent::agentenwahl::Agenteneinstellung) {
        agent.schreiben = self.arbeit.is_some();
        agent.wurzel = self.arbeit.as_ref().map(|a| a.display().to_string()).or(agent.wurzel.take());
        agent.web_recherche = false;
        agent.blick_bildschirm = false;
        agent.blick_kamera = false;
    }
}

/// **Der Zusatz des Moduls mit diesem Namen**, frisch gefunden und geprueft.
///
/// ⚑ **Geprueft wird vor jedem Lauf**, nicht einmal beim Waehlen: Ein
/// Modul, das sich seither geaendert hat, faellt hier auf und bringt dann
/// nichts mit.
pub fn zusatz(name: &str) -> Result<Modulzusatz, String> {
    let funde = finden(&|_| None);
    let fund = funde
        .iter()
        .find(|f| f.ergebnis.as_ref().is_ok_and(|g| g.beschreibung.name == name))
        .or_else(|| funde.iter().find(|f| f.ordner.file_name().is_some_and(|n| n == name)));
    match fund.map(|f| &f.ergebnis) {
        Some(Ok(g)) => Modulzusatz::aus(g),
        Some(Err(grund)) => Err(format!("Modul {name}: {grund}")),
        None => Err(format!("kein Modul namens {name}")),
    }
}

/// **Der Zusatz des eingestellten Moduls** (`agent.modul`), oder `None`, wenn
/// keines gewaehlt ist. Ein gewaehltes, das nicht besteht, ist ein Fehler
/// mit Grund: Der Lauf soll das sagen, statt still ohne Modul zu arbeiten.
pub fn eingestellter_zusatz(agent: &myl_local_agent::agentenwahl::Agenteneinstellung) -> Option<Result<Modulzusatz, String>> {
    agent.modul.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(zusatz)
}

/// Welche Werkzeuge einer Kiste nicht nachfragen (`ohne_nachfrage` in
/// `kiste.json`).
pub fn ohne_nachfrage_der_kiste(kiste: &Path) -> Vec<String> {
    std::fs::read_to_string(kiste.join("kiste.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["ohne_nachfrage"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()))
        .unwrap_or_default()
}

/// **Eine Zeile je gefundenem Modul fuer eine Auswahl**: Name, Satz, und ob
/// es geprueft ist (sonst der Grund).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Modulwahl {
    pub name: String,
    pub was: String,
    pub geprueft: bool,
    pub grund: Option<String>,
}

/// Alle gefundenen Module fuer eine Auswahl, gepruefte zuerst.
pub fn wahl() -> Vec<Modulwahl> {
    let mut aus: Vec<Modulwahl> = finden(&|_| None)
        .into_iter()
        .map(|f| match f.ergebnis {
            Ok(g) => Modulwahl { name: g.beschreibung.name.clone(), was: g.beschreibung.was.clone(), geprueft: true, grund: None },
            Err(grund) => Modulwahl {
                name: f.ordner.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                was: String::new(),
                geprueft: false,
                grund: Some(grund),
            },
        })
        .collect();
    aus.sort_by_key(|m| !m.geprueft);
    aus
}

/// So oft sieht der Wirt nach, ob sich ein laufendes Modul geaendert hat.
const AENDERUNG_PRUEFEN: std::time::Duration = std::time::Duration::from_secs(5);

/// So gross darf das Protokoll eines Moduls werden; danach bleibt die
/// juengere Haelfte.
const PROTOKOLL_HOECHSTENS: u64 = 1 << 20;

/// **Eine Zeile ins Protokoll eines Moduls** (`<speicher>/<name>.log`),
/// nicht auf den Schirm: was es auf stderr schrieb, was abgelehnt wurde,
/// wann es endete.
pub fn protokoll(name: &str, zeile: &str) {
    use std::io::Write;
    let p = speicherpfad().join(format!("{name}.log"));
    let _ = std::fs::create_dir_all(speicherpfad());
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > PROTOKOLL_HOECHSTENS) {
        if let Ok(alles) = std::fs::read(&p) {
            let ab = alles.len() / 2;
            let start = alles[ab..].iter().position(|&b| b == b'\n').map(|i| ab + i + 1).unwrap_or(alles.len());
            let _ = std::fs::write(&p, &alles[start..]);
        }
    }
    let zeit = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{zeit} {}", myl_module::filter::eine_zeile(zeile));
    }
}

/// Was der Wirt zu einem laufenden Modul weiss.
pub struct Laufendes {
    pub lauf: Lauf,
    pub g: Geprueft,
    /// Wann die Signatur zuletzt geaendert war (fuer den Neustart).
    sig_stand: Option<SystemTime>,
    naechste_pruefung: Instant,
    pub naechster_takt: Option<Instant>,
    pub modus: bool,
    /// Die Zeile, mit der der Modus zuletzt begann (fuer einen Neustart).
    pub aktivierung: Option<String>,
    /// Der zuletzt an das Modul gegebene Befehl.
    pub letzter_befehl: Option<String>,
    pub fusszeile: Vec<Teil>,
    pub unten: Vec<Vec<Teil>>,
    /// Ob der Bereich unten schon einmal gefuellt wurde (fuer das Banner).
    pub unten_da: bool,
    /// Token Modellantwort in der laufenden Stunde (Budget).
    pub token_stunde: (Instant, u32),
    letzte_ablehnung: Option<Instant>,
}

/// **Ein Grund, ein Modul abzulehnen**, den der Wirt selbst kennt (die
/// Konsole: ein Hauptbefehl, den sie belegt).
pub type Verbot = Box<dyn Fn(&Geprueft) -> Option<String> + Send + Sync>;

/// **Alle Module eines Wirts** (Konsole oder Fenster).
///
/// ⚑ **Eine Verwaltung fuer beide Oberflaechen** (2026-10-12). Bis dahin
/// stand sie in der Konsole; das Fenster haette Starten, Neustart nach einer
/// Aenderung, Takt und Budget ein zweites Mal gebraucht. Was gezeichnet und
/// wie gefragt wird, bleibt bei der Oberflaeche.
#[derive(Default)]
pub struct Modulwirt {
    pub gefunden: Vec<Fund>,
    pub laufend: Vec<Laufendes>,
    verboten: Option<Verbot>,
}

fn sig_stand(ordner: &Path) -> Option<SystemTime> {
    std::fs::metadata(ordner.join(myl_module::SIGNATUR)).and_then(|m| m.modified()).ok()
}

impl Modulwirt {
    /// **Findet und prueft alle Module.** Ein Name gilt nur einmal: der
    /// erste Ort gewinnt, ein zweites Modul gleichen Namens wird abgelehnt.
    pub fn neu(verboten: Option<Verbot>) -> Self {
        let mut w = Self { verboten, ..Self::default() };
        w.neu_pruefen();
        w
    }

    /// Prueft alle gefundenen Module erneut (etwa nach einer Installation).
    pub fn neu_pruefen(&mut self) {
        let verboten = self.verboten.take();
        self.gefunden = finden(&|g| verboten.as_ref().and_then(|v| v(g)));
        self.verboten = verboten;
    }

    /// Die Befehle aller gepruefen Module, fuer Hilfe und Vervollstaendigung.
    pub fn befehle(&self) -> Vec<Modulbefehl> {
        self.gefunden.iter().filter_map(|f| f.ergebnis.as_ref().ok()).flat_map(|g| g.beschreibung.befehle.clone()).collect()
    }

    /// Welches gepruefte Modul eine Zeile meint (nach ihrem ersten Wort).
    pub fn modul_der_zeile(&self, zeile: &str) -> Option<String> {
        let wort = zeile.split_whitespace().next()?;
        self.gefunden.iter().filter_map(|f| f.ergebnis.as_ref().ok()).find(|g| g.beschreibung.befehle[0].name == wort).map(|g| g.beschreibung.name.clone())
    }

    /// Das laufende Modul dieses Namens.
    pub fn laufendes(&mut self, name: &str) -> Option<&mut Laufendes> {
        self.laufend.iter_mut().find(|l| l.g.beschreibung.name == name)
    }

    /// Das Modul, dessen Modus gerade gilt.
    pub fn aktives(&self) -> Option<&Laufendes> {
        self.laufend.iter().find(|l| l.modus)
    }

    /// **Startet ein gepruefetes Modul**, falls es nicht laeuft. Geprueft
    /// wird dafuer noch einmal, an den Bytes, die laufen werden.
    pub fn starten(&mut self, name: &str, s: &Start) -> Result<(), String> {
        if self.laufend.iter_mut().any(|l| l.g.beschreibung.name == name && l.lauf.laeuft()) {
            return Ok(());
        }
        self.laufend.retain(|l| l.g.beschreibung.name != name);
        let ordner = self.gefunden.iter().find(|f| f.ergebnis.as_ref().is_ok_and(|g| g.beschreibung.name == name)).map(|f| f.ordner.clone()).ok_or_else(|| format!("kein gepruefetes Modul {name}"))?;
        let vertrauen = Vertrauensliste::lesen(&vertrauenspfad())?;
        let mut versionen = Versionsstand::lesen(&versionspfad());
        let g = signatur::pruefen(&ordner, &vertrauen, &versionen)?;
        let lauf = Lauf::starten(&g, s)?;
        if versionen.merken(&g.beschreibung.name, &g.beschreibung.version) {
            let _ = versionen.schreiben(&versionspfad());
        }
        self.laufend.push(Laufendes {
            lauf,
            sig_stand: sig_stand(&ordner),
            naechste_pruefung: Instant::now() + AENDERUNG_PRUEFEN,
            g,
            naechster_takt: None,
            modus: false,
            aktivierung: None,
            letzter_befehl: None,
            fusszeile: Vec::new(),
            unten: Vec::new(),
            unten_da: false,
            token_stunde: (Instant::now(), 0),
            letzte_ablehnung: None,
        });
        Ok(())
    }

    /// **Die naechste Frist** fuer die Eingabezeile: der frueheste Takt
    /// eines Moduls im Modus.
    pub fn frist(&self) -> Option<Instant> {
        self.laufend.iter().filter(|l| l.modus).filter_map(|l| l.naechster_takt).min()
    }

    /// Ob ein Modul laeuft (dann braucht die Eingabezeile eine Frist, damit
    /// ein Weckruf ankommt).
    pub fn irgendeins_laeuft(&self) -> bool {
        !self.laufend.is_empty()
    }

    /// **Faellige Takte schicken, Geaendertes neu starten und alles
    /// abholen**, je Modul mit seinem Namen.
    pub fn abholen(&mut self, s: &Start) -> Vec<(String, Ereignis)> {
        let jetzt = Instant::now();
        let mut neu_starten = Vec::new();
        for l in self.laufend.iter_mut() {
            if l.modus && l.naechster_takt.is_some_and(|t| t <= jetzt) {
                l.naechster_takt = None;
                l.lauf.senden(&AnModul::Takt);
            }
            if jetzt >= l.naechste_pruefung {
                l.naechste_pruefung = jetzt + AENDERUNG_PRUEFEN;
                if sig_stand(&l.g.ordner) != l.sig_stand {
                    neu_starten.push(l.g.beschreibung.name.clone());
                }
            }
        }
        let mut aus = Vec::new();
        for name in neu_starten {
            aus.extend(self.neu_starten(&name, s));
        }
        for l in self.laufend.iter_mut() {
            let name = l.g.beschreibung.name.clone();
            for e in l.lauf.abholen() {
                aus.push((name.clone(), e));
            }
        }
        aus
    }

    /// **Ein geaendertes Modul neu starten**: pruefen, beenden, starten, und
    /// war sein Modus aktiv, die letzte Aktivierung erneut schicken. Faellt
    /// die Pruefung durch, laeuft das alte weiter, und das Ereignis sagt es.
    fn neu_starten(&mut self, name: &str, s: &Start) -> Vec<(String, Ereignis)> {
        let Some(i) = self.laufend.iter().position(|l| l.g.beschreibung.name == name) else { return Vec::new() };
        let ordner = self.laufend[i].g.ordner.clone();
        self.laufend[i].sig_stand = sig_stand(&ordner);
        let vertrauen = Vertrauensliste::lesen(&vertrauenspfad()).unwrap_or_default();
        let versionen = Versionsstand::lesen(&versionspfad());
        if let Err(f) = signatur::pruefen(&ordner, &vertrauen, &versionen) {
            return vec![(name.to_string(), Ereignis::Abgelehnt(format!("geaendert, aber nicht gueltig ({f}); es laeuft weiter die gepruefte Fassung")))];
        }
        let alt = self.laufend.remove(i);
        let (modus, aktivierung) = (alt.modus, alt.aktivierung.clone());
        drop(alt);
        self.neu_pruefen();
        let mut aus = vec![(name.to_string(), Ereignis::Fehlerausgabe("neue Fassung, neu gestartet".into()))];
        match self.starten(name, s) {
            Ok(()) => {
                if let (true, Some(z), Some(l)) = (modus, aktivierung, self.laufendes(name)) {
                    // 📌 2026-10-08: Ohne diese Zeile merkte sich die neue
                    //    Fassung beim Betreten des Modus „keine Aktivierung“
                    //    (sie kam nicht vom Menschen), und der zweite Neustart
                    //    liess das Modul untaetig stehen.
                    l.letzter_befehl = Some(z.clone());
                    l.lauf.senden(&AnModul::Befehl { zeile: z });
                }
            }
            Err(f) => aus.push((name.to_string(), Ereignis::Beendet(f))),
        }
        aus
    }

    /// Schickt allen ein Ereignis (etwa ein neues Modell oder neue Masse).
    pub fn allen(&mut self, e: &AnModul) {
        for l in self.laufend.iter_mut() {
            l.lauf.senden(e);
        }
    }

    /// Beendet alle Module (beim Schliessen der Oberflaeche).
    pub fn alle_beenden(&mut self) {
        for mut l in self.laufend.drain(..) {
            l.lauf.beenden();
        }
    }

    /// **Bucht `grenze` Token auf das Stundenbudget eines Moduls**
    /// (`modell_token_je_stunde` seiner Befugnisse, 0 heisst ohne Grenze).
    /// Gebucht wird die erbetene Grenze, nicht die verbrauchte Laenge: So
    /// steht vor der Frage fest, ob sie erlaubt ist.
    pub fn budget_buchen(&mut self, name: &str, grenze: u32) -> Result<(), String> {
        let Some(l) = self.laufendes(name) else { return Err("nicht gestartet".into()) };
        let budget = l.g.beschreibung.befugnisse.modell_token_je_stunde;
        if l.token_stunde.0.elapsed() >= std::time::Duration::from_secs(3600) {
            l.token_stunde = (Instant::now(), 0);
        }
        if budget > 0 && l.token_stunde.1.saturating_add(grenze) > budget {
            return Err(format!("Budget erschoepft ({budget} Token je Stunde)"));
        }
        l.token_stunde.1 = l.token_stunde.1.saturating_add(grenze);
        Ok(())
    }

    /// Ob eine Ablehnung gezeigt werden soll (hoechstens eine je Minute und
    /// Modul, damit ein Modul den Schirm nicht damit fuellt).
    pub fn ablehnung_zeigen(&mut self, name: &str) -> bool {
        let Some(l) = self.laufendes(name) else { return true };
        let zeigen = l.letzte_ablehnung.is_none_or(|t| t.elapsed().as_secs() >= 60);
        if zeigen {
            l.letzte_ablehnung = Some(Instant::now());
        }
        zeigen
    }
}

/// **Die Zeilen fuer `/module`.**
pub fn liste(w: &Modulwirt) -> Vec<String> {
    let mut aus = Vec::new();
    if w.gefunden.is_empty() {
        aus.push(format!("Keine Module. Sie liegen in {} an der Wurzel oder in {}.", MODULORDNER, einstellungsordner().join(NUTZERMODULE).display()));
    }
    for f in &w.gefunden {
        match &f.ergebnis {
            Ok(g) => {
                let b = &g.beschreibung;
                let laeuft = w.laufend.iter().find(|l| l.g.beschreibung.name == b.name);
                let zustand = match laeuft {
                    Some(l) if l.modus => "läuft, Modus aktiv",
                    Some(_) => "läuft",
                    None => "bereit",
                };
                let art = match b.laufzeit {
                    myl_module::beschreibung::Laufzeit::Nativ { .. } => "nativ, ohne Abschottung",
                    myl_module::beschreibung::Laufzeit::Wasm { .. } => "abgeschottet",
                };
                aus.push(format!("✓ {} {} ({art}), signiert von {}, {zustand}: {}", b.befehle[0].name, b.version, myl_module::filter::eine_zeile(&g.signiert_von), myl_module::filter::eine_zeile(&b.was)));
            }
            Err(f2) => aus.push(format!("✗ {}: {}", f.ordner.display(), myl_module::filter::eine_zeile(f2))),
        }
    }
    aus
}

/// **Installiert ein Modul** aus einem Ordner in den Ordner der
/// Nutzermodule: erst pruefen, dann kopieren (ohne den Inhalt
/// veraenderlicher Ordner), dann die Kopie pruefen. Gibt den Namen zurueck.
pub fn installieren(quelle: &Path) -> Result<String, String> {
    let vertrauen = Vertrauensliste::lesen(&vertrauenspfad())?;
    let versionen = Versionsstand::lesen(&versionspfad());
    let g = signatur::pruefen(quelle, &vertrauen, &versionen).map_err(|f| format!("nicht installiert: {f}"))?;
    let ziel = einstellungsordner().join(NUTZERMODULE).join(&g.beschreibung.name);
    if ziel.exists() {
        return Err(format!("{} gibt es schon; erst entfernen", ziel.display()));
    }
    let tmp = einstellungsordner().join(NUTZERMODULE).join(format!(".{}.unfertig", g.beschreibung.name));
    let _ = std::fs::remove_dir_all(&tmp);
    for rel in g.beschreibung.dateien.keys().map(String::as_str).chain([myl_module::BESCHREIBUNG, myl_module::SIGNATUR]) {
        let nach = tmp.join(rel);
        if let Some(o) = nach.parent() {
            std::fs::create_dir_all(o).map_err(|f| format!("{}: {f}", o.display()))?;
        }
        std::fs::copy(quelle.join(rel), &nach).map_err(|f| format!("{rel}: {f}"))?;
    }
    for v in &g.beschreibung.veraenderlich {
        std::fs::create_dir_all(tmp.join(v)).map_err(|f| format!("{v}: {f}"))?;
    }
    signatur::pruefen(&tmp, &vertrauen, &versionen).map_err(|f| {
        let _ = std::fs::remove_dir_all(&tmp);
        format!("die Kopie besteht die Pruefung nicht: {f}")
    })?;
    std::fs::rename(&tmp, &ziel).map_err(|f| format!("{}: {f}", ziel.display()))?;
    Ok(g.beschreibung.name)
}

/// Ein Beobachter fuer die Stuecke einer Modellantwort.
pub type Stueckbeobachter = Box<dyn Fn(myl_local_agent::textstrom::Stueck) + Send + Sync>;

/// **Eine Modellfrage eines Moduls**: die Grundsaetze als Systemnachricht,
/// der Vorspann und die Frage als Nachrichten des Nutzers, die Antwort ohne
/// Denkteil. Grenze, Denken und Gierigkeit gelten nur fuer diese Frage,
/// danach wieder wie eingestellt.
///
/// ⚑ **Der feste Vorspann als eigene Nachricht** (V1, 2026-10-08): Die
/// Laufzeit merkt sich den Zustand vor jeder Nachrichtenmarke; bleibt der
/// Vorspann gleich, wird nur der wechselnde Teil neu eingelesen.
pub fn modell_fragen(
    modell: &mut crate::Oertlichesmodell,
    vorspann: &str,
    frage: &str,
    grenze_token: usize,
    denken: Option<bool>,
    gierig: bool,
    beobachter: Option<Stueckbeobachter>,
) -> Result<String, String> {
    use myl_local_agent::Modellweg;
    let sprache = crate::Einstellungen::lesen(&crate::Einstellungen::vorgabepfad()).map(|e| e.oberflaeche.sprache).unwrap_or_default();
    let grundsaetze = myl_local_agent::systemprompt::grundsaetze(sprache)?;
    let (grenze, denken_vorher, gierig_vorher) = (modell.grenze, modell.denken, modell.gierig);
    modell.grenze = grenze_token;
    if let Some(d) = denken {
        modell.denken = d;
    }
    modell.gierig = gierig_vorher || gierig;
    // Ohne eigenen Beobachter bleibt der vorhandene, wie er ist.
    let vorher = beobachter.map(|b| modell.beobachter.replace(b));
    let mut nachrichten = vec![crate::Nachricht::system(grundsaetze)];
    if !vorspann.trim().is_empty() {
        nachrichten.push(crate::Nachricht::nutzer(vorspann.to_string()));
    }
    nachrichten.push(crate::Nachricht::nutzer(frage.to_string()));
    let antwort = modell.chat("lokal", &nachrichten, Some(grenze_token as u32));
    if let Some(v) = vorher {
        modell.beobachter = v;
    }
    modell.grenze = grenze;
    modell.denken = denken_vorher;
    modell.gierig = gierig_vorher;
    let a = antwort.map_err(|f| f.to_string())?;
    let text = match a.text.rfind("</think>") {
        Some(i) => &a.text[i + "</think>".len()..],
        None => &a.text,
    };
    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ Die Ausnahmen einer Kiste kommen aus ihrer `kiste.json`; ohne Datei
    /// keine.
    #[test]
    fn ohne_nachfrage_aus_der_kiste() {
        let d = std::env::temp_dir().join(format!("myl-modulzusatz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(ohne_nachfrage_der_kiste(&d).is_empty());
        std::fs::write(d.join("kiste.json"), r#"{"eingebaute":"Base","ohne_nachfrage":["lage","kurse"]}"#).unwrap();
        assert_eq!(ohne_nachfrage_der_kiste(&d), vec!["lage".to_string(), "kurse".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⚑ **Im Modus eines Moduls**: nur im Arbeitsordner schreibend, ohne
    /// Netz und ohne Blick; ohne Arbeitsordner gar nicht schreibend, und die
    /// Wurzel bleibt dann die eingestellte.
    #[test]
    fn im_modus_eng() {
        let mut z = Modulzusatz { name: "m".into(), kiste: None, skills: None, arbeit: Some(PathBuf::from("/m/arbeit")), ohne_nachfrage: Vec::new(), programm: None };
        let mut a = myl_local_agent::agentenwahl::Agenteneinstellung { wurzel: Some("/w".into()), web_recherche: true, blick_bildschirm: true, blick_kamera: true, ..Default::default() };
        z.im_modus(&mut a);
        assert!(a.schreiben && !a.web_recherche && !a.blick_bildschirm && !a.blick_kamera);
        assert_eq!(a.wurzel.as_deref(), Some("/m/arbeit"));
        z.arbeit = None;
        let mut a = myl_local_agent::agentenwahl::Agenteneinstellung { wurzel: Some("/w".into()), schreiben: true, ..Default::default() };
        z.im_modus(&mut a);
        assert!(!a.schreiben);
        assert_eq!(a.wurzel.as_deref(), Some("/w"));
    }

    /// ⚑ Ohne gewaehltes Modul kein Zusatz, und ein leerer Name ist keines.
    #[test]
    fn ohne_wahl_kein_zusatz() {
        let mut a = myl_local_agent::agentenwahl::Agenteneinstellung::default();
        assert!(eingestellter_zusatz(&a).is_none());
        a.modul = Some("  ".into());
        assert!(eingestellter_zusatz(&a).is_none());
    }
}
