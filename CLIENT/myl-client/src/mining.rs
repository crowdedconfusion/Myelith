//! **Mining**: was dieser Rechner dem Netz hergibt, und der Knoten, der es
//! haelt.
//!
//! # ⚑ Eine Stelle fuer Konsole und Fenster
//!
//! `/mining` in der Konsole und der Reiter „Knoten" im Fenster zeigen
//! dieselbe Freigabe, pruefen sie mit derselben Regel und starten denselben
//! Knoten. Was hier steht, steht nirgends sonst; die Oberflaechen zeichnen
//! nur.
//!
//! # ⚑ Was „Mining starten" heute tut (Festlegung des Projektinhabers, 2026-10-17)
//!
//! **Es startet einen echten `myl-node` im Probelauf.** Ein Testnetz gibt es
//! noch nicht, also laeuft der Knoten auf seiner eigenen Probekette, und die
//! MYL darauf sind Spielgeld; jede Meldung sagt das. Vorher prueft diese
//! Datei die Freigabe, legt den Ort in `speicherorte.toml` ab und schreibt
//! die Zusage, die der Knoten beim Start mit seinem Kontoschluessel
//! unterschreibt.
//!
//! ⚠️ **Kein Shard angeschlossen.** Der Knoten beantwortet Inferenzauftraege
//! aus dem Netz erst, wenn ein Shard-Prozess an ihm haengt; bis dahin lehnt
//! er sie ehrlich ab, und die Meldung zaehlt sie als abgelehnt. Kerne und
//! Rechenwerke stehen in der Zusage, gerechnet wird mit ihnen noch nicht.
//!
//! # ⚑ Der Knoten gehoert dem, der ihn startet
//!
//! Er laeuft als eigener Prozess, und `zustand.json` im Datenordner nennt
//! ihn, damit die andere Oberflaeche ihn sieht und beenden kann. **Endet die
//! Oberflaeche, die ihn gestartet hat, endet er mit** ([`beenden_wenn_eigen`]):
//! Ein Knoten, der unsichtbar weiterlaeuft, haelt Platte und Kerne fest,
//! ohne dass jemand es sieht.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use myl_store::{Speicherort, Speicherorte, Zusage, Zusageort, Zweck};
use serde::{Deserialize, Serialize};

use crate::Einstellungen;

/// Der Name des Orts, den die Seite bearbeitet. Weitere Orte fuer das Netz
/// darf die Datei von Hand nennen; der Knoten zaehlt sie mit.
pub const ORTNAME: &str = "mining";
/// Der vorgeschlagene Ordner auf einer Platte.
pub const ORDNER_VORGABE: &str = "Myelith";
/// Wie oft die Zusammenfassung erscheint (Auftrag des Projektinhabers).
pub const MELDETAKT: Duration = Duration::from_secs(5 * 60);
/// Unter welchem Namen der Knoten laeuft.
const KNOTENNAME: &str = "miner";
/// Wie lange der Start auf den Beobachtungsendpunkt wartet.
const STARTFRIST: Duration = Duration::from_secs(30);
/// Der Takt der Zustandsaufnahme des Knotens, in Sekunden.
const AUFNAHME_SEKUNDEN: &str = "10";
/// ⚑ **Die Ehrlichkeit steht als Konstante da**, damit sie verschwindet,
/// sobald ein Shard angeschlossen wird, und nicht vergessen in einem Text.
pub const SHARD_ANGESCHLOSSEN: bool = false;

/// Wo `speicherorte.toml` liegt: neben den Einstellungen, oder wo
/// `MYL_SPEICHERORTE` hinzeigt.
pub fn speicherorte_pfad() -> PathBuf {
    if let Some(p) = std::env::var_os("MYL_SPEICHERORTE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    neben_den_einstellungen(myl_store::orte::DATEINAME)
}

/// Der Datenordner des Minings: Schluessel, Zusage, Zustand, Protokoll des
/// Starts. ⚑ **Neben den Einstellungen und nicht auf der Platte**, denn die
/// Schluessel bleiben, auch wenn die Platte wechselt.
pub fn datenordner() -> PathBuf {
    neben_den_einstellungen("mining")
}

fn neben_den_einstellungen(name: &str) -> PathBuf {
    let e = Einstellungen::vorgabepfad();
    e.parent().map(|p| p.join(name)).unwrap_or_else(|| PathBuf::from(name))
}

/// Eine Platte, die sich als Ort anbietet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Platte {
    /// Wie sie heisst, fuer die Anzeige.
    pub name: String,
    /// Wo ein Ordner auf ihr beginnt.
    pub wurzel: PathBuf,
    /// Ob sie angeschlossen ist und nicht die des Systems.
    pub extern_: bool,
    pub frei_bytes: u64,
    pub gesamt_bytes: u64,
}

/// **Die Platten dieser Maschine**: der Benutzerordner zuerst, dann jede
/// eingehaengte.
///
/// ⚑ **Der Benutzerordner und nicht die Systemwurzel**, denn dort darf der
/// Nutzer schreiben. Eine angeschlossene Platte erscheint unter ihrem
/// Einhaengepunkt.
pub fn platten() -> Vec<Platte> {
    let mut aus: Vec<Platte> = Vec::new();
    for (name, wurzel, extern_) in kandidaten() {
        if aus.iter().any(|p| p.wurzel == wurzel) {
            continue;
        }
        if let Some(m) = crate::hardware::Hardware::platte(&wurzel) {
            aus.push(Platte { name, wurzel, extern_, frei_bytes: m.frei_bytes, gesamt_bytes: m.gesamt_bytes });
        }
    }
    aus
}

fn heim() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

fn kandidaten() -> Vec<(String, PathBuf, bool)> {
    let mut k = Vec::new();
    if let Some(h) = heim() {
        k.push(("Benutzerordner (intern)".to_string(), h, false));
    }
    let unter = |ordner: &Path, k: &mut Vec<(String, PathBuf, bool)>| {
        let Ok(eintraege) = std::fs::read_dir(ordner) else { return };
        let mut liste: Vec<PathBuf> = eintraege.flatten().map(|e| e.path()).collect();
        liste.sort();
        for p in liste {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            // Versteckte Eintraege und die Schnappschuesse von macOS sind
            // keine Platten, die jemand hergibt.
            if name.starts_with('.') || name.starts_with("com.apple") || !p.is_dir() {
                continue;
            }
            // ⚑ Die Systemplatte steht unter /Volumes als Verweis auf /.
            if std::fs::canonicalize(&p).map(|c| c == Path::new("/")).unwrap_or(false) {
                continue;
            }
            k.push((name, p, true));
        }
    };
    if cfg!(target_os = "macos") {
        unter(Path::new("/Volumes"), &mut k);
    } else if cfg!(windows) {
        for b in b'D'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", b as char));
            if p.is_dir() {
                k.push((format!("Laufwerk {}:", b as char), p, true));
            }
        }
    } else {
        let nutzer = std::env::var("USER").unwrap_or_default();
        if !nutzer.is_empty() {
            unter(&Path::new("/media").join(&nutzer), &mut k);
            unter(&Path::new("/run/media").join(&nutzer), &mut k);
        }
        unter(Path::new("/mnt"), &mut k);
    }
    k
}

/// **Was die Seite einstellt**, ein Feld je Zeile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Freigabe {
    /// Die Platte, also ihr Einhaengepunkt oder der Benutzerordner.
    pub platte: PathBuf,
    /// Der Ordner darauf, relativ.
    pub ordner: String,
    /// Wie viel hoechstens, beide Zwecke zusammen.
    pub volumen_gib: u64,
    /// Ob dort auch eigene Gewichte liegen duerfen.
    pub gewichte: bool,
    pub kerne: u32,
    /// Kennungen aus dem Hardwarescan.
    pub rechenwerke: Vec<String>,
}

impl Freigabe {
    /// Der ganze Pfad des Orts.
    pub fn pfad(&self) -> PathBuf {
        self.platte.join(&self.ordner)
    }

    /// Der Ort, wie er in `speicherorte.toml` steht.
    pub fn als_ort(&self) -> Speicherort {
        let mut zwecke = vec![Zweck::Netz];
        if self.gewichte {
            zwecke.insert(0, Zweck::Gewichte);
        }
        Speicherort { name: ORTNAME.into(), pfad: self.pfad(), volumen_gib: self.volumen_gib, zwecke }
    }

    /// **Der Vorschlag fuer die Seite**: was zuletzt galt, sonst die erste
    /// angeschlossene Platte mit einem Ordner `Myelith`, hundert GiB oder
    /// die Haelfte des Freien, und die Haelfte der Kerne.
    ///
    /// ⚑ **Von den abgelegten Rechenwerken nur, was hier steckt und
    /// rechnet.** Eine Karte, die abgezogen wurde, liesse sonst jeden Start
    /// an der Pruefung scheitern, ohne dass die Seite sie zeigt; abschalten
    /// koennte man sie dann nicht mehr.
    pub fn vorschlag(e: &Einstellungen, orte: &Speicherorte, platten: &[Platte], kerne_max: u32, werke: &[Werkwahl]) -> Self {
        let kerne = e.mining.kerne.unwrap_or((kerne_max / 2).max(1)).clamp(1, kerne_max.max(1));
        let rechenwerke: Vec<String> = e
            .mining
            .rechenwerke
            .iter()
            .filter(|k| werke.iter().any(|w| &w.kennung == *k && w.rechnet))
            .cloned()
            .collect();
        if let Some(o) = orte.ort(ORTNAME) {
            // Die Platte ist die mit dem laengsten passenden Anfang.
            let (platte, ordner) = platten
                .iter()
                .filter(|p| o.pfad.starts_with(&p.wurzel) && o.pfad != p.wurzel)
                .max_by_key(|p| p.wurzel.components().count())
                .map(|p| {
                    let rest = o.pfad.strip_prefix(&p.wurzel).unwrap_or(&o.pfad);
                    (p.wurzel.clone(), rest.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| {
                    (
                        o.pfad.parent().map(Path::to_path_buf).unwrap_or_default(),
                        o.pfad.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    )
                });
            return Self { platte, ordner, volumen_gib: o.volumen_gib, gewichte: o.dient(Zweck::Gewichte), kerne, rechenwerke };
        }
        let p = platten.iter().find(|p| p.extern_).or(platten.first());
        let volumen_gib = p.map(|p| (p.frei_bytes / myl_store::orte::GIB / 2).clamp(1, 100)).unwrap_or(10);
        Self {
            platte: p.map(|p| p.wurzel.clone()).unwrap_or_default(),
            ordner: ORDNER_VORGABE.into(),
            volumen_gib,
            gewichte: false,
            kerne,
            rechenwerke,
        }
    }

    /// Die Zusage: alle Orte fuer das Netz aus der Datei, dieser darin.
    pub fn zusage(&self, orte: &Speicherorte, jetzt_ms: u64) -> Zusage {
        let mut o = orte.clone();
        o.setzen(self.als_ort());
        Zusage {
            fassung: Zusage::FASSUNG,
            probelauf: true,
            orte: o
                .fuer(Zweck::Netz)
                .map(|o| Zusageort { name: o.name.clone(), pfad: o.pfad.clone(), volumen_bytes: o.volumen_bytes() })
                .collect(),
            kerne: self.kerne,
            rechenwerke: self.rechenwerke.clone(),
            erstellt_ms: jetzt_ms,
        }
    }
}

/// **Ein Rechenwerk, wie die Seite es zeigt**: mit Rechenweg im Namen und
/// mit dem Grund, falls es nicht hergegeben werden kann.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Werkwahl {
    pub kennung: String,
    /// Name und Rechenweg, etwa „NVIDIA GeForce RTX 4090 (CUDA)".
    pub titel: String,
    /// ⚑ Ob es hergegeben werden kann; sonst steht der Schalter gesperrt da.
    pub rechnet: bool,
    /// Warum nicht, in der eingestellten Sprache.
    pub sperrgrund: Option<String>,
}

/// **Die Rechenwerke fuer `/mining` und den Reiter „Knoten"**: jedes
/// gefundene, einzeln, und gesperrt mit Grund, wenn es hier nicht rechnet.
///
/// ⚑ **Dieselbe Frage wie in der Freigabemaske der Einstellungen**: Fuehrt
/// ueber das Geraet ein Rechenweg, und rechnet der in diesem Programm?
/// 📌 Bis zum 2026-10-17 bekam hier jedes Geraet einen Schalter, auch eine
/// eingebaute Intel-Grafik ohne jeden Rechenweg, und die Zusage nannte dann
/// eine Karte, die nichts rechnen kann. **Eine Zusage verspricht dem Netz
/// etwas**; was nicht rechnet, gehoert nicht hinein. Gezeigt wird es
/// trotzdem, damit niemand eine Karte sucht, die stillschweigend fehlt.
pub fn werke(hw: &crate::hardware::Hardware, sprache: crate::einstellungen::Sprache) -> Vec<Werkwahl> {
    let rechnende = crate::rechenwege::vorhanden();
    hw.rechenwerke
        .iter()
        .map(|r| {
            let rechnet = r.rueckseite.is_some_and(|b| rechnende.contains(&b.backend()));
            Werkwahl {
                kennung: r.kennung.clone(),
                titel: r.beschriftung(),
                rechnet,
                sperrgrund: (!rechnet).then(|| crate::hardware::gesperrt_weil(r.rueckseite, sprache)),
            }
        })
        .collect()
}

/// **Prueft die Freigabe**, bevor etwas geschrieben oder gestartet wird.
///
/// ⛔️ **Ohne Platz keine Freigabe** (Festlegung des Projektinhabers,
/// 2026-10-17, fuer das Holen von Modellen; dieselbe Regel hier): Was
/// zugesagt wird, muss frei sein oder schon dort liegen. Sonst heisst es
/// erst Speicher schaffen.
pub fn pruefen(f: &Freigabe, hw: &crate::hardware::Hardware) -> Result<(), String> {
    if !f.platte.is_dir() {
        return Err(format!("Die Platte {} ist nicht da (nicht angeschlossen?).", f.platte.display()));
    }
    let ordner = Path::new(f.ordner.trim());
    if f.ordner.trim().is_empty()
        || ordner.is_absolute()
        || ordner.components().any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("Der Ordner \"{}\" muss ein Name unter der Platte sein, ohne \"..\".", f.ordner));
    }
    if f.volumen_gib == 0 {
        return Err("Das Volumen ist null; gib mindestens 1 GiB her.".into());
    }
    let frei = crate::hardware::Hardware::platte(&f.platte).map(|p| p.frei_bytes).unwrap_or(0);
    let schon = belegt(&f.pfad());
    let verfuegbar = frei.saturating_add(schon);
    if f.volumen_gib.saturating_mul(myl_store::orte::GIB) > verfuegbar {
        return Err(format!(
            "Auf {} sind nur {} frei (dazu {} schon im Ordner). Erst Speicher schaffen oder weniger hergeben.",
            f.platte.display(),
            groesse_text(frei),
            groesse_text(schon)
        ));
    }
    if f.kerne == 0 || f.kerne as usize > hw.kerne {
        return Err(format!("Kerne: 1 bis {}, gewählt {}.", hw.kerne, f.kerne));
    }
    let werke = werke(hw, crate::einstellungen::Sprache::De);
    for r in &f.rechenwerke {
        match werke.iter().find(|w| &w.kennung == r) {
            None => return Err(format!("Das Rechenwerk {r} gibt es auf dieser Maschine nicht.")),
            Some(w) if !w.rechnet => {
                return Err(format!("{} rechnet in diesem Programm nicht und lässt sich deshalb nicht zusagen.", w.titel))
            }
            Some(_) => {}
        }
    }
    // ⚑ **Schreiben koennen, nicht nur sehen**: Eine schreibgeschuetzte
    // Platte faellt hier auf und nicht erst beim ersten Gegenstand.
    let probe = f.pfad().join(".myelith-schreibprobe");
    std::fs::create_dir_all(f.pfad())
        .and_then(|_| std::fs::write(&probe, b"x"))
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|e| format!("{}: nicht beschreibbar ({e}).", f.pfad().display()))?;
    Ok(())
}

/// Was unter einem Pfad schon liegt, ohne Verknuepfungen zu verfolgen
/// (`myl_store::orte::groesse`).
pub fn belegt(pfad: &Path) -> u64 {
    myl_store::orte::groesse(pfad)
}

/// Legt die Freigabe ab: den Ort in `speicherorte.toml`, Kerne und
/// Rechenwerke in den Einstellungen.
pub fn speichern(f: &Freigabe) -> Result<(), String> {
    let pfad = speicherorte_pfad();
    let mut orte = Speicherorte::lesen(&pfad).map_err(|e| e.to_string())?;
    orte.setzen(f.als_ort());
    orte.schreiben(&pfad).map_err(|e| e.to_string())?;
    let ep = Einstellungen::vorgabepfad();
    let mut e = Einstellungen::lesen(&ep)?;
    e.mining.kerne = Some(f.kerne);
    e.mining.rechenwerke = f.rechenwerke.clone();
    e.schreiben(&ep)
}

/// Was ueber einen laufenden Knoten bekannt ist (`zustand.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zustand {
    /// Der Prozess des Knotens.
    pub pid: u32,
    /// Wo sein Beobachtungsendpunkt horcht.
    pub beobachtung: String,
    pub gestartet_ms: u64,
    /// Wer ihn gestartet hat: „Konsole" oder „Fenster".
    pub von: String,
    /// Der Prozess, der ihn gestartet hat.
    pub von_pid: u32,
    /// Das Guthaben beim ersten Blick nach dem Start.
    #[serde(default)]
    pub guthaben_beim_start: Option<u64>,
    /// Was hergegeben ist, fuer die Meldung.
    pub kerne: u32,
    pub rechenwerke: usize,
    pub volumen_bytes: u64,
    pub pfad: PathBuf,
}

/// ⚑ **Was das Netz auf diesem Rechner rechnet** (Stufe 4, P8, 2026-10-22):
/// die Netzlast, die der laufende Knoten neben sein Protokoll legt, wenn er
/// einen Shard aus der Kette startet. `None` ohne laufenden Knoten oder
/// ohne Shard.
pub fn netzlast() -> Option<myl_store::teilablage::Netzlast> {
    let z = zustand()?;
    myl_store::teilablage::Netzlast::lesen(&z.pfad.join("knoten").join("logs").join(myl_store::teilablage::NETZLAST_DATEI))
}

/// **Die Netzlast als Last des lokalen Plans**, mit Vorrang `Netz`: der
/// Bedarf ihres Bereichs, mit dem gemessenen Zuschlag. `None`, wenn keine
/// Netzlast da ist oder ihr Artefakt sich nicht lesen lässt.
pub fn netzlast_im_plan(
    nl: &myl_store::teilablage::Netzlast,
) -> Option<myl_dispatcher::local::plan::Last> {
    use myl_dispatcher::local::{bedarf, plan};
    let ganz = bedarf::Modellbedarf::aus_artefakt(&nl.artefakte).ok()?;
    let b = ganz.bereich(nl.von as usize, nl.bis as usize, nl.mit_einbettung(), nl.mit_kopf()).ok()?;
    let mindest = (crate::einstellungen::KONTEXT_MINDESTENS as u64).min(b.hoechstkontext);
    Some(plan::Last {
        name: format!("Netz: {}, Ebenen {} bis {}, Epoche {}", ganz.name, nl.von, nl.bis, nl.epoche),
        vorrang: plan::Vorrang::Netz,
        zuschlag_bytes: crate::hardware::eichbuch().zuschlag_eigen(&b),
        kontext_mindestens: mindest,
        kontext_soll: mindest,
        kerne_mindestens: 1,
        profil: None,
        gewichte: plan::Gewichte::Eigen(b),
    })
}

fn zustandsdatei() -> PathBuf {
    datenordner().join("zustand.json")
}

fn zustand_schreiben(z: &Zustand) -> Result<(), String> {
    let p = zustandsdatei();
    if let Some(v) = p.parent() {
        std::fs::create_dir_all(v).map_err(|e| format!("{}: {e}", v.display()))?;
    }
    let t = serde_json::to_string_pretty(z).map_err(|e| e.to_string())?;
    std::fs::write(&p, t + "\n").map_err(|e| format!("{}: {e}", p.display()))
}

/// **Der laufende Knoten, oder `None`.**
///
/// ⚑ **Laufen heisst antworten**: Die Datei allein sagt nur, dass einmal
/// einer lief. Antwortet sein Endpunkt nicht mehr, ist er beendet, und die
/// Datei geht weg.
pub fn zustand() -> Option<Zustand> {
    let p = zustandsdatei();
    let z: Zustand = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
    if holen(&z.beobachtung, "/gesundheit").is_ok() {
        return Some(z);
    }
    let _ = std::fs::remove_file(&p);
    None
}

/// Der eigene Knotenprozess, damit er nach dem Beenden abgeholt wird und
/// nicht als Zombie stehen bleibt.
static EIGENER: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

/// Wo `myl-node` liegt: neben diesem Programm, im Bauordner des
/// Repositoriums, im Installationsordner oder im `PATH`.
pub fn knotenprogramm() -> Option<PathBuf> {
    let name = if cfg!(windows) { "myl-node.exe" } else { "myl-node" };
    let mut orte: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        orte.push(d);
    }
    if let Some(w) = myl_local_agent::ort::wurzel() {
        orte.push(w.join("SYSTEM/full-build/release"));
    }
    if let Some(h) = heim() {
        orte.push(h.join(".local/bin"));
    }
    if let Some(pfad) = std::env::var_os("PATH") {
        orte.extend(std::env::split_paths(&pfad));
    }
    orte.into_iter().map(|o| o.join(name)).find(|p| p.is_file())
}

/// **Startet den Knoten** mit der Freigabe.
///
/// Prueft, legt ab, schreibt die Zusage und wartet, bis der Knoten
/// antwortet. ⚑ **Scheitert der Start, kommt der Grund aus dem Knoten
/// selbst**, die letzten Zeilen seiner Ausgabe, und nicht ein „ging nicht".
pub fn starten(f: &Freigabe, von: &str) -> Result<Zustand, String> {
    if let Some(z) = zustand() {
        return Err(format!("Mining läuft schon (gestartet in: {}).", z.von));
    }
    if cfg!(windows) {
        return Err("Der Knoten (myl-node) läuft bisher nur unter macOS und Linux.".into());
    }
    let hw = crate::hardware::erheben(&f.platte);
    pruefen(f, &hw)?;
    let programm = knotenprogramm()
        .ok_or("myl-node nicht gefunden; es entsteht mit dem Installationsskript oder mit `cargo build --release` in NODE/myl-node.")?;
    speichern(f)?;
    let daten = datenordner();
    std::fs::create_dir_all(&daten).map_err(|e| format!("{}: {e}", daten.display()))?;
    let orte = Speicherorte::lesen(&speicherorte_pfad()).map_err(|e| e.to_string())?;
    let zusage = f.zusage(&orte, jetzt_ms());
    let zusagedatei = daten.join("zusage.toml");
    std::fs::write(&zusagedatei, zusagetext(&zusage)).map_err(|e| format!("{}: {e}", zusagedatei.display()))?;
    let knotenordner = f.pfad().join("knoten");
    std::fs::create_dir_all(&knotenordner).map_err(|e| format!("{}: {e}", knotenordner.display()))?;
    // ⚑ **Ein freier Port fuer den Endpunkt**, sonst stiesse er mit einem
    // zweiten Knoten auf derselben Maschine zusammen.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("kein freier Port: {e}"))?;
    let beobachtung = format!("127.0.0.1:{port}");
    let ausgabe = daten.join("knoten.log");
    let log = std::fs::File::create(&ausgabe).map_err(|e| format!("{}: {e}", ausgabe.display()))?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new(&programm);
    c.arg("--name").arg(KNOTENNAME)
        .arg("--port").arg("0")
        .arg("--erzeuger")
        .arg("--kette").arg(knotenordner.join("kette.dat"))
        .arg("--protokolle").arg(knotenordner.join("logs"))
        .arg("--schluessel").arg(daten.join("knoten.key"))
        .arg("--kontoschluessel").arg(daten.join("konto.key"))
        .arg("--speicherorte").arg(speicherorte_pfad())
        .arg("--zusage").arg(&zusagedatei)
        .arg("--beobachtung").arg(&beobachtung)
        .arg("--aufnahme").arg(AUFNAHME_SEKUNDEN)
        .arg("--ohne-tuer")
        .arg("--still")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log2);
    if let Some(w) = myl_local_agent::ort::wurzel() {
        c.arg("--konformitaet").arg(w.join("INTEGER_LLM/conformance/vectors/op"));
    }
    // ⚑ **Von `myl` gestartet in eine eigene Prozessgruppe**, denn dort soll
    //   er die Shell ueberleben: Sonst naehme das Schliessen des Terminals
    //   (SIGHUP an die Gruppe) ihn mit. Aus Konsole und Fenster bleibt er in
    //   deren Gruppe und endet mit ihnen, auch wenn das Terminal einfach
    //   zugeht. 📌 Gesehen im Probelauf: Der Knoten der Konsole endete mit
    //   dem pty, und genau so soll es dort sein.
    #[cfg(unix)]
    if laeuft_weiter(von) {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let mut kind = c.spawn().map_err(|e| format!("{}: {e}", programm.display()))?;
    let bis = std::time::Instant::now() + STARTFRIST;
    loop {
        if let Ok(Some(status)) = kind.try_wait() {
            return Err(format!("Der Knoten endete beim Start ({status}):\n{}", letzte_zeilen(&ausgabe, 8)));
        }
        if holen(&beobachtung, "/gesundheit").is_ok() {
            break;
        }
        if std::time::Instant::now() >= bis {
            let _ = kind.kill();
            let _ = kind.wait();
            return Err(format!("Der Knoten antwortete nicht binnen {} s:\n{}", STARTFRIST.as_secs(), letzte_zeilen(&ausgabe, 8)));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let z = Zustand {
        pid: kind.id(),
        beobachtung,
        gestartet_ms: jetzt_ms(),
        von: von.to_string(),
        von_pid: std::process::id(),
        guthaben_beim_start: None,
        kerne: f.kerne,
        rechenwerke: f.rechenwerke.len(),
        volumen_bytes: zusage.volumen_bytes(),
        pfad: f.pfad(),
    };
    if let Ok(mut g) = EIGENER.lock() {
        *g = Some(kind);
    }
    zustand_schreiben(&z)?;
    Ok(z)
}

/// Ob ein Knoten das Programm ueberlebt, das ihn startet: nur von `myl`.
pub fn laeuft_weiter(von: &str) -> bool {
    von == "myl"
}

/// **Beendet den Knoten**, gleich wer ihn gestartet hat. `Ok(false)`, wenn
/// keiner lief.
///
/// ⚑ **Erst freundlich, dann bestimmt**: SIGTERM laesst den Knoten seine
/// Kette und sein Protokoll abschliessen; antwortet er nach fuenf Sekunden
/// noch, folgt SIGKILL.
pub fn beenden() -> Result<bool, String> {
    let Some(z) = zustand() else {
        return Ok(false);
    };
    let signal = |s: &str| {
        std::process::Command::new("kill").arg(s).arg(z.pid.to_string()).status().map(|st| st.success()).unwrap_or(false)
    };
    signal("-TERM");
    let bis = std::time::Instant::now() + Duration::from_secs(5);
    while holen(&z.beobachtung, "/gesundheit").is_ok() && std::time::Instant::now() < bis {
        std::thread::sleep(Duration::from_millis(100));
    }
    if holen(&z.beobachtung, "/gesundheit").is_ok() {
        signal("-KILL");
    }
    if let Ok(mut g) = EIGENER.lock() {
        if let Some(mut k) = g.take() {
            let _ = k.wait();
        }
    }
    let _ = std::fs::remove_file(zustandsdatei());
    Ok(true)
}

/// Beendet den Knoten, wenn dieser Prozess ihn gestartet hat. Fuer das Ende
/// einer Oberflaeche.
pub fn beenden_wenn_eigen() {
    if zustand().is_some_and(|z| z.von_pid == std::process::id()) {
        let _ = beenden();
    }
}

/// Was der Knoten meldet (`/metriken`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Stand {
    pub hoehe: u64,
    pub peers: u64,
    pub speicher_orte: u64,
    pub gegenstaende: u64,
    pub shards: u64,
    pub unlesbar: u64,
    pub belegt_bytes: u64,
    pub zugesagt_bytes: u64,
    pub bearbeitet: u64,
    pub abgelehnt: u64,
    pub guthaben: u64,
}

/// Liest die Zahlen aus dem Prometheus-Text.
pub fn stand_aus(text: &str) -> Stand {
    let mut s = Stand::default();
    for z in text.lines().filter(|z| !z.starts_with('#')) {
        let Some((name, wert)) = z.split_once(' ') else { continue };
        let Ok(w) = wert.trim().parse::<u64>() else { continue };
        match name {
            "myelith_kette_hoehe" => s.hoehe = w,
            "myelith_peers" => s.peers = w,
            "myelith_speicher_orte" => s.speicher_orte = w,
            "myelith_speicher_gegenstaende" => s.gegenstaende = w,
            "myelith_speicher_shards" => s.shards = w,
            "myelith_speicher_unlesbar" => s.unlesbar = w,
            "myelith_speicher_belegt_bytes" => s.belegt_bytes = w,
            "myelith_speicher_zugesagt_bytes" => s.zugesagt_bytes = w,
            "myelith_inferenz_bearbeitet_total" => s.bearbeitet = w,
            "myelith_inferenz_abgelehnt_total" => s.abgelehnt = w,
            "myelith_konto_guthaben" => s.guthaben = w,
            _ => {}
        }
    }
    s
}

/// **Fragt den Knoten**, und merkt sich beim ersten Mal das Guthaben, an
/// dem das Geminte gemessen wird.
pub fn stand(z: &mut Zustand) -> Result<Stand, String> {
    let s = stand_aus(&holen(&z.beobachtung, "/metriken")?);
    if z.guthaben_beim_start.is_none() {
        z.guthaben_beim_start = Some(s.guthaben);
        let _ = zustand_schreiben(z);
    }
    Ok(s)
}

/// **Die Zusammenfassung**, alle fuenf Minuten in Konsole und Fenster.
///
/// ⚑ **Jede Zahl kommt aus dem Knoten oder aus der Zusage**, keine ist
/// geschaetzt; und was es noch nicht gibt (Testnetz, Shard), steht dabei.
pub fn zusammenfassung(z: &Zustand, s: &Stand, jetzt_ms: u64) -> Vec<String> {
    let minuten = jetzt_ms.saturating_sub(z.gestartet_ms) / 60_000;
    let seit = if minuten >= 60 { format!("{} h {:02} min", minuten / 60, minuten % 60) } else { format!("{minuten} min") };
    let gemint = s.guthaben.saturating_sub(z.guthaben_beim_start.unwrap_or(s.guthaben));
    let rechenwerke = match z.rechenwerke {
        0 => "kein Rechenwerk".to_string(),
        1 => "1 Rechenwerk".to_string(),
        n => format!("{n} Rechenwerke"),
    };
    let anfragen = if SHARD_ANGESCHLOSSEN { "" } else { " (kein Shard angeschlossen)" };
    let mut zeilen = vec![
        format!("⛏ Mining seit {seit}, Probelauf ohne Testnetz"),
        format!(
            "  hergegeben: {} Kerne, {rechenwerke}, {} unter {}",
            z.kerne,
            groesse_text(z.volumen_bytes),
            z.pfad.display()
        ),
        format!(
            "  im Speicher: {} Gegenstände, davon {} Shards ({} belegt)",
            s.gegenstaende,
            s.shards,
            groesse_text(s.belegt_bytes)
        ),
        format!("  Anfragen: {} bearbeitet, {} abgelehnt{anfragen}", s.bearbeitet, s.abgelehnt),
        format!("  gemint: {} MYL (Spielgeld) · Kette Höhe {}, {} Gegenstellen", myl_text(gemint), s.hoehe, s.peers),
    ];
    if s.unlesbar > 0 {
        zeilen.push(format!("  ⚠ {} Ordner ohne lesbares Manifest", s.unlesbar));
    }
    zeilen
}

/// MYL aus der kleinsten Einheit, ganzzahlig: `1,5` statt `1500000`.
pub fn myl_text(einheiten: u64) -> String {
    let je = EINHEITEN_JE_MYL;
    let ganz = einheiten / je;
    let rest = einheiten % je;
    if rest == 0 {
        return ganz.to_string();
    }
    let breite = je.ilog10() as usize;
    let nachkomma = format!("{rest:0breite$}");
    format!("{ganz},{}", nachkomma.trim_end_matches('0'))
}

/// ⚑ **Eine Million Einheiten je MYL**, wie `myl_tokenomics::UNITS_PER_MYL`.
/// Hier als Konstante, weil der Client die Wirtschaft nicht einbindet; die
/// Probe unten haelt beide gleich.
const EINHEITEN_JE_MYL: u64 = 1_000_000;

/// Bytes fuer Menschen, ganzzahlig gerundet auf eine Nachkommastelle.
pub fn groesse_text(bytes: u64) -> String {
    const EINHEITEN: [(&str, u64); 4] = [("TiB", 1 << 40), ("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)];
    for (name, n) in EINHEITEN {
        if bytes >= n {
            let zehntel = (bytes as u128 * 10 + n as u128 / 2) / n as u128;
            return if zehntel % 10 == 0 { format!("{} {name}", zehntel / 10) } else { format!("{},{} {name}", zehntel / 10, zehntel % 10) };
        }
    }
    format!("{bytes} B")
}

fn jetzt_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Die letzten Zeilen einer Datei, fuer eine Fehlermeldung.
fn letzte_zeilen(p: &Path, n: usize) -> String {
    let t = std::fs::read_to_string(p).unwrap_or_default();
    let z: Vec<&str> = t.lines().collect();
    z[z.len().saturating_sub(n)..].join("\n")
}

/// **Ein GET an den Endpunkt des Knotens**, mit kurzer Frist.
///
/// ⚑ **Von Hand und ohne Rahmenwerk**, wie der Endpunkt selbst: ein GET
/// ohne Rumpf auf der Rueckschleife.
fn holen(adresse: &str, weg: &str) -> Result<String, String> {
    let a: std::net::SocketAddr = adresse.parse().map_err(|_| format!("keine Adresse: {adresse}"))?;
    let mut s = std::net::TcpStream::connect_timeout(&a, Duration::from_millis(500)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    s.write_all(format!("GET {weg} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut roh = String::new();
    s.read_to_string(&mut roh).map_err(|e| e.to_string())?;
    let (kopf, rumpf) = roh.split_once("\r\n\r\n").unwrap_or((&roh, ""));
    if !kopf.starts_with("HTTP/1.1 200") {
        return Err(kopf.lines().next().unwrap_or("keine Antwort").to_string());
    }
    Ok(rumpf.to_string())
}

/// Der Text der Zusagedatei.
///
/// ⚠️ **Der Kopf steht unter der Unterschrift mit**, denn sie geht ueber die
/// Bytes der Datei; wer ihn aendert, bricht sie.
fn zusagetext(z: &Zusage) -> String {
    format!(
        "# Kapazitätszusage dieses Knotens, Probelauf.\n# Unterschrieben vom Knoten beim Start, siehe zusage.unterschrift daneben.\n\n{}",
        z.als_text()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platte(name: &str, wurzel: &str, extern_: bool) -> Platte {
        Platte { name: name.into(), wurzel: wurzel.into(), extern_, frei_bytes: 300 * myl_store::orte::GIB, gesamt_bytes: 0 }
    }

    #[test]
    fn der_vorschlag_nimmt_die_angeschlossene_platte_und_die_haelfte_der_kerne() {
        let p = [platte("Benutzerordner", "/home/a", false), platte("Platte", "/Volumes/Platte", true)];
        let f = Freigabe::vorschlag(&Einstellungen::default(), &Speicherorte::default(), &p, 15, &[]);
        assert_eq!(f.platte, PathBuf::from("/Volumes/Platte"));
        assert_eq!((f.ordner.as_str(), f.volumen_gib, f.kerne, f.gewichte), ("Myelith", 100, 7, false));
    }

    #[test]
    fn der_vorschlag_kehrt_zum_abgelegten_ort_zurueck() {
        let p = [platte("Benutzerordner", "/home/a", false), platte("Platte", "/Volumes/Platte", true)];
        let f = Freigabe { platte: "/Volumes/Platte".into(), ordner: "netz/miner".into(), volumen_gib: 42, gewichte: true, kerne: 3, rechenwerke: vec![] };
        let mut orte = Speicherorte::default();
        orte.setzen(f.als_ort());
        let mut e = Einstellungen::default();
        e.mining.kerne = Some(3);
        assert_eq!(Freigabe::vorschlag(&e, &orte, &p, 15, &[]), f);
    }

    /// ⚑ **Die Zusage nennt jeden Ort fuer das Netz**, auch die von Hand
    /// eingetragenen, und keinen nur fuer Gewichte.
    #[test]
    fn die_zusage_nennt_alle_orte_fuer_das_netz() {
        let mut orte = Speicherorte::default();
        orte.setzen(Speicherort { name: "hand".into(), pfad: "/x".into(), volumen_gib: 5, zwecke: vec![Zweck::Netz] });
        orte.setzen(Speicherort { name: "nur".into(), pfad: "/y".into(), volumen_gib: 5, zwecke: vec![Zweck::Gewichte] });
        let f = Freigabe { platte: "/p".into(), ordner: "m".into(), volumen_gib: 10, gewichte: false, kerne: 2, rechenwerke: vec!["gpu0".into()] };
        let z = f.zusage(&orte, 7);
        let namen: Vec<&str> = z.orte.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(namen, ["hand", "mining"]);
        assert_eq!(z.volumen_bytes(), 15 * myl_store::orte::GIB);
        assert!(z.probelauf);
        assert!(Zusage::aus_text(&zusagetext(&z)).is_ok(), "der Kopf muss als TOML-Kommentar durchgehen");
    }

    #[test]
    fn die_pruefung_verlangt_platz_und_einen_ordnernamen() {
        let hw = crate::hardware::erheben(&std::env::temp_dir());
        let gut = Freigabe {
            platte: std::env::temp_dir(),
            ordner: format!("myl-mining-probe-{}", std::process::id()),
            volumen_gib: 1,
            gewichte: false,
            kerne: 1,
            rechenwerke: vec![],
        };
        pruefen(&gut, &hw).expect("1 GiB im Temp-Ordner");
        let _ = std::fs::remove_dir_all(gut.pfad());
        let zu_viel = Freigabe { volumen_gib: 1 << 30, ..gut.clone() };
        assert!(pruefen(&zu_viel, &hw).unwrap_err().contains("Erst Speicher schaffen"));
        for o in ["", "../weg", "/abs"] {
            assert!(pruefen(&Freigabe { ordner: o.into(), ..gut.clone() }, &hw).is_err(), "{o:?}");
        }
        assert!(pruefen(&Freigabe { kerne: 0, ..gut.clone() }, &hw).is_err());
        assert!(pruefen(&Freigabe { rechenwerke: vec!["gibtsnicht".into()], ..gut.clone() }, &hw).is_err());
        assert!(pruefen(&Freigabe { platte: "/gibt/es/nicht".into(), ..gut }, &hw).unwrap_err().contains("nicht da"));
    }

    fn karte(kennung: &str, name: &str) -> crate::hardware::Rechenwerk {
        crate::hardware::Rechenwerk {
            kennung: kennung.into(),
            name: name.into(),
            speicher_bytes: None,
            frei_bytes: None,
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite: None,
            pci: None,
        }
    }

    /// ⚑ **Jede Karte steht da, eine ohne Rechenweg gesperrt und mit Grund**,
    /// und die Pruefung laesst sie nicht in die Zusage.
    #[test]
    fn eine_karte_ohne_rechenweg_steht_gesperrt_da_und_wird_nicht_zugesagt() {
        let mut hw = crate::hardware::erheben(&std::env::temp_dir());
        hw.rechenwerke = vec![karte("pci-0000-00-02-0", "Intel UHD Graphics 770")];
        let w = werke(&hw, crate::einstellungen::Sprache::De);
        assert_eq!(w.len(), 1);
        assert!(!w[0].rechnet);
        assert!(w[0].sperrgrund.as_deref().is_some_and(|g| g.contains("keinen Rechenweg")), "{:?}", w[0].sperrgrund);
        let f = Freigabe {
            platte: std::env::temp_dir(),
            ordner: format!("myl-mining-werk-{}", std::process::id()),
            volumen_gib: 1,
            gewichte: false,
            kerne: 1,
            rechenwerke: vec!["pci-0000-00-02-0".into()],
        };
        assert!(pruefen(&f, &hw).unwrap_err().contains("rechnet in diesem Programm nicht"));
        let _ = std::fs::remove_dir_all(f.pfad());
    }

    /// ⚑ **Eine abgezogene oder gesperrte Karte faellt aus dem Vorschlag**,
    /// statt jeden Start scheitern zu lassen.
    #[test]
    fn der_vorschlag_vergisst_karten_die_nicht_mehr_rechnen() {
        let mut e = Einstellungen::default();
        e.mining.rechenwerke = vec!["weg".into(), "da".into(), "gesperrt".into()];
        let werke = [
            Werkwahl { kennung: "da".into(), titel: "A (Metal)".into(), rechnet: true, sperrgrund: None },
            Werkwahl { kennung: "gesperrt".into(), titel: "B".into(), rechnet: false, sperrgrund: Some("x".into()) },
        ];
        let f = Freigabe::vorschlag(&e, &Speicherorte::default(), &[platte("P", "/p", true)], 4, &werke);
        assert_eq!(f.rechenwerke, ["da"]);
    }

    #[test]
    fn die_zahlen_kommen_aus_dem_text_des_knotens() {
        let t = "# HELP myelith_kette_hoehe x\n# TYPE myelith_kette_hoehe gauge\nmyelith_kette_hoehe 12\nmyelith_speicher_shards 2\nmyelith_inferenz_abgelehnt_total 3\nmyelith_konto_guthaben 1500000\nmyelith_fremd 9\n";
        let s = stand_aus(t);
        assert_eq!((s.hoehe, s.shards, s.abgelehnt, s.guthaben, s.peers), (12, 2, 3, 1_500_000, 0));
    }

    #[test]
    fn die_zusammenfassung_nennt_jede_zahl_und_den_probelauf() {
        let z = Zustand {
            pid: 1,
            beobachtung: String::new(),
            gestartet_ms: 0,
            von: "Konsole".into(),
            von_pid: 1,
            guthaben_beim_start: Some(1_000_000),
            kerne: 8,
            rechenwerke: 1,
            volumen_bytes: 200 * myl_store::orte::GIB,
            pfad: "/Volumes/Platte/Myelith".into(),
        };
        let s = Stand { gegenstaende: 3, shards: 2, belegt_bytes: 3 << 29, bearbeitet: 4, abgelehnt: 5, guthaben: 3_500_000, hoehe: 9, ..Default::default() };
        let t = zusammenfassung(&z, &s, 65 * 60_000).join("\n");
        for teil in ["seit 1 h 05 min", "Probelauf", "8 Kerne", "1 Rechenwerk", "200 GiB", "/Volumes/Platte/Myelith", "3 Gegenstände, davon 2 Shards", "1,5 GiB", "4 bearbeitet, 5 abgelehnt", "2,5 MYL (Spielgeld)", "Höhe 9"] {
            assert!(t.contains(teil), "{teil:?} fehlt in\n{t}");
        }
    }

    #[test]
    fn myl_und_groessen_ganzzahlig() {
        assert_eq!(myl_text(0), "0");
        assert_eq!(myl_text(1_500_000), "1,5");
        assert_eq!(myl_text(1_000_001), "1,000001");
        assert_eq!(groesse_text(0), "0 B");
        assert_eq!(groesse_text(1 << 30), "1 GiB");
        assert_eq!(groesse_text(3 << 29), "1,5 GiB");
    }

    /// ⚑ **Dieselbe Einheit wie die Wirtschaft**, gelesen aus ihrer Quelle.
    #[test]
    fn die_einheit_ist_die_der_wirtschaft() {
        let quelle = include_str!("../../../TOKENOMICS/myl-tokenomics/src/lib.rs");
        assert!(quelle.contains(&format!("pub const UNITS_PER_MYL: u64 = {};", "1_000_000")));
        assert_eq!(EINHEITEN_JE_MYL, 1_000_000);
    }
}
