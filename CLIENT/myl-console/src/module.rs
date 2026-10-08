//! **Die Module der Konsole**: finden, pruefen, starten, sprechen.
//!
//! Ein Modul liegt als Ordner in `MODULES/` an der Wurzel des
//! Repositoriums oder im Ordner `module/` neben den Einstellungen. Die
//! Konsole prueft jedes beim Start ([`myl_module::signatur::pruefen`]);
//! nur ein gepruefetes zeigt seine Befehle in Hilfe und Vervollstaendigung,
//! und nur ein gepruefetes startet. Gestartet wird es beim ersten Befehl.
//!
//! ⚑ **Die Konsole bleibt Herrin**: Sie zeichnet allein, sie fragt den
//! Menschen in ihrem eigenen Rahmen, sie fragt das Modell und faehrt
//! Agentenlaeufe, und ein Modul bekommt nur, was seine Befugnisse nennen
//! ([`myl_module::wirt::Pruefer`]).
//!
//! ⚑ **Ohne Modul ist alles wie ohne diese Datei**: keine Befehle, kein
//! Takt, keine Fusszeile eines Moduls.

use myl_module::beschreibung::Befehl as Modulbefehl;
use myl_module::nachricht::{AnModul, Stil, Teil};
use myl_module::signatur::{self, Geprueft, Versionsstand, Vertrauensliste};
use myl_module::wirt::{Ereignis, Lauf, Start};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

/// Der Ordner der Module an der Wurzel des Repositoriums.
pub const MODULORDNER: &str = "MODULES";

/// Neben den Einstellungen: der Ordner nachinstallierter Module, die
/// Vertrauensliste, der Versionsstand und der Zwischenspeicher.
const NUTZERMODULE: &str = "module";
const VERTRAUEN: &str = "vertrauen.json";
const VERSIONEN: &str = "modulversionen.json";
const SPEICHER: &str = "modulspeicher";

/// So oft sieht die Konsole nach, ob sich ein laufendes Modul geaendert hat.
const AENDERUNG_PRUEFEN: std::time::Duration = std::time::Duration::from_secs(5);

/// Der Ordner der Einstellungen.
pub fn einstellungsordner() -> PathBuf {
    myl_client::Einstellungen::vorgabepfad().parent().map(Path::to_path_buf).unwrap_or_default()
}

pub fn vertrauenspfad() -> PathBuf {
    einstellungsordner().join(VERTRAUEN)
}

fn versionspfad() -> PathBuf {
    einstellungsordner().join(VERSIONEN)
}

/// Wohin die gepruefen Programme kopiert werden.
pub fn speicherpfad() -> PathBuf {
    einstellungsordner().join(SPEICHER)
}

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

/// Die Orte, an denen Module liegen.
pub fn wurzeln() -> Vec<PathBuf> {
    let mut aus = Vec::new();
    if let Some(w) = myl_local_agent::ort::wurzel() {
        aus.push(w.join(MODULORDNER));
    }
    aus.push(einstellungsordner().join(NUTZERMODULE));
    aus
}

/// **Ein gefundenes Modul**: geprueft oder mit dem Grund, warum nicht.
pub struct Fund {
    pub ordner: PathBuf,
    pub ergebnis: Result<Geprueft, String>,
}

/// Was die Konsole zu einem laufenden Modul weiss.
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

/// **Alle Module dieser Konsole.**
#[derive(Default)]
pub struct Modulwirt {
    pub gefunden: Vec<Fund>,
    pub laufend: Vec<Laufendes>,
}

fn sig_stand(ordner: &Path) -> Option<SystemTime> {
    std::fs::metadata(ordner.join(myl_module::SIGNATUR)).and_then(|m| m.modified()).ok()
}

/// Die Ordner, die wie ein Modul aussehen (mit Beschreibung).
fn kandidaten() -> Vec<PathBuf> {
    let mut aus = Vec::new();
    for w in wurzeln() {
        let Ok(rd) = std::fs::read_dir(&w) else { continue };
        let mut o: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join(myl_module::BESCHREIBUNG).is_file()).collect();
        o.sort();
        aus.extend(o);
    }
    aus
}

impl Modulwirt {
    /// **Findet und prueft alle Module.** Ein Name gilt nur einmal: der
    /// erste Ort gewinnt, ein zweites Modul gleichen Namens wird abgelehnt.
    pub fn neu() -> Self {
        let mut w = Self::default();
        w.neu_pruefen();
        w
    }

    /// Prueft alle gefundenen Module erneut (etwa nach einer Installation).
    pub fn neu_pruefen(&mut self) {
        let vertrauen = Vertrauensliste::lesen(&vertrauenspfad());
        let versionen = Versionsstand::lesen(&versionspfad());
        let mut namen: Vec<String> = Vec::new();
        self.gefunden = kandidaten()
            .into_iter()
            .map(|ordner| {
                let ergebnis = match &vertrauen {
                    Ok(v) => signatur::pruefen(&ordner, v, &versionen),
                    Err(f) => Err(format!("Vertrauensliste: {f}")),
                };
                let ergebnis = ergebnis.and_then(|g| {
                    if namen.contains(&g.beschreibung.name) {
                        return Err(format!("ein Modul namens {} gibt es schon", g.beschreibung.name));
                    }
                    if crate::sitzung::eingebauter_befehl(&g.beschreibung.befehle[0].name) {
                        return Err(format!("{} ist ein Befehl der Konsole", g.beschreibung.befehle[0].name));
                    }
                    namen.push(g.beschreibung.name.clone());
                    Ok(g)
                });
                Fund { ordner, ergebnis }
            })
            .collect();
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
    /// die Pruefung durch, laeuft das alte weiter und die Konsole sagt es.
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

    /// Beendet alle Module (beim Schliessen der Konsole).
    pub fn alle_beenden(&mut self) {
        for mut l in self.laufend.drain(..) {
            l.lauf.beenden();
        }
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

/// **Teile mit Stil als eine Zeile**, auf `breite` Zeichen gekuerzt oder
/// aufgefuellt; gefaerbt erst nach dem Zuschnitt (Farbcodes zaehlen dort
/// nicht mit). `grund` ist die Farbe der Zeile.
pub fn teile_zeichnen(teile: &[Teil], breite: usize, d: myl_client::einstellungen::Konsolendesign, grund: crossterm::style::Color) -> String {
    let farbig = crate::design::farbig();
    let mut aus = String::new();
    let mut rest = breite;
    for t in teile {
        if rest == 0 {
            break;
        }
        let text: String = t.text.chars().take(rest).collect();
        rest -= text.chars().count();
        aus.push_str(&stil_faerben(t.stil, &text, d, farbig, grund));
    }
    aus.push_str(&" ".repeat(rest));
    aus
}

/// Ein Stil eines Moduls in den Farben dieser Konsole.
pub fn stil_faerben(stil: Stil, text: &str, d: myl_client::einstellungen::Konsolendesign, farbig: bool, grund: crossterm::style::Color) -> String {
    use crossterm::style::{Attribute, SetAttribute, SetForegroundColor};
    if !farbig {
        return text.to_string();
    }
    let rollen = crate::design::rollen(d);
    let mit = |s: String| format!("{s}{}", SetForegroundColor(grund));
    match stil {
        Stil::Normal => text.to_string(),
        Stil::Beiwerk => mit(rollen.beiwerk.faerben(text, true)),
        Stil::Hervor | Stil::Titel => mit(rollen.ueberschrift.faerben(text, true)),
        Stil::Warnung => mit(rollen.warnung.faerben(text, true)),
        Stil::Gewinn => format!("{}{text}{}", SetForegroundColor(crate::design::GEWINN), SetForegroundColor(grund)),
        Stil::Verlust => format!("{}{}{text}{}{}", SetForegroundColor(crate::design::VERLUST), SetAttribute(Attribute::Bold), SetAttribute(Attribute::NormalIntensity), SetForegroundColor(grund)),
    }
}

/// **Text eines Moduls in den Rollbereich**, je Zeile eingerueckt, im Stil.
pub fn zeilen_ausgeben(text: &str, stil: Stil, d: myl_client::einstellungen::Konsolendesign) {
    let farbig = crate::design::farbig();
    if stil == Stil::Titel {
        println!();
    }
    for z in text.lines() {
        let grund = crossterm::style::Color::Reset;
        println!("  {}", stil_faerben(stil, z, d, farbig, grund));
    }
    if stil == Stil::Titel {
        println!();
    }
}

/// **Der Bereich unten als Text**: eine Leerzeile Abstand zur Fusszeile,
/// dann die Zeilen des Moduls, an absoluten Stellen, ohne den Wagen zu
/// merken (der Rahmen hat ihn schon gemerkt).
pub fn unten_text(sch: &crate::schirm::Schirm, zeilen: &[Vec<Teil>], d: myl_client::einstellungen::Konsolendesign) -> String {
    let platz = sch.zusatz() as usize;
    let breite = (crate::banner::fenstermasse().0 as usize).saturating_sub(4);
    let mut aus = String::new();
    for i in 0..platz {
        let inhalt = if i == 0 {
            String::new()
        } else {
            zeilen.get(i - 1).map(|t| format!("  {}", teile_zeichnen(t, breite, d, crossterm::style::Color::Reset).trim_end())).unwrap_or_default()
        };
        aus.push_str(&format!("{}{inhalt}", sch.zeile(crate::schirm::RESERVE + i as u16)));
    }
    aus
}

/// Die Zeilen, die ein Modul unten wuenscht, als Platz unter dem Rahmen
/// (eine Leerzeile dazu); null ohne Zeilen.
pub fn unten_platz(zeilen: &[Vec<Teil>]) -> u16 {
    if zeilen.is_empty() {
        0
    } else {
        zeilen.len() as u16 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teile_auf_die_breite() {
        let d = myl_client::einstellungen::Konsolendesign::default();
        let t = vec![Teil { text: "Heute ".into(), stil: Stil::Normal }, Teil { text: "+0,12 %".into(), stil: Stil::Normal }];
        assert_eq!(teile_zeichnen(&t, 20, d, crossterm::style::Color::Reset), "Heute +0,12 %       ");
        assert_eq!(teile_zeichnen(&t, 8, d, crossterm::style::Color::Reset), "Heute +0");
    }

    #[test]
    fn unten_platz_mit_leerzeile() {
        assert_eq!(unten_platz(&[]), 0);
        assert_eq!(unten_platz(&[vec![], vec![]]), 3);
    }

    #[test]
    fn verlust_bleibt_fett_und_kehrt_zurueck() {
        let d = myl_client::einstellungen::Konsolendesign::default();
        let s = stil_faerben(Stil::Verlust, "−0,72 %", d, true, crossterm::style::Color::Grey);
        assert!(s.contains("−0,72 %") && s.ends_with(&format!("{}", crossterm::style::SetForegroundColor(crossterm::style::Color::Grey))), "{s:?}");
    }
}
