//! **Ein Buch zum Skill machen**, als Werkzeug fuer den Agenten.
//!
//! ⚑ **Der Agent stoesst nur an, gerechnet wird in `myl buch`**: Die Erkennung und
//! ihre Modelle sind nur in `myl` eingebaut, damit Konsole und Fenster sie nicht
//! tragen. Fehlt `myl`, steht das Werkzeug nicht im Angebot.
//!
//! ⚑ **Wie die Verdichtung des Gespraechs**: Der Skill ist die Zusammenfassung, jeder
//! Punkt nennt seine Seite, und der Wortlaut liegt unter `literature/`. `search_files`
//! findet die Nadel, `read_file` liest die Seite.
//!
//! ⛔️ **Ein Buch ist fremder Text.** Das Ergebnis traegt die Marke, der Skill auch,
//! und nach Fremdem im Gespraech laeuft der Aufruf nur mit Freigabe.
//!
//! ⚑ **Platz nach Bedarf, ohne das Modell zu entladen**: Dessen Gewichte sind als
//! Abbild eingeblendet, ein zweiter Prozess teilt sie, und das System gibt sie frei,
//! sobald Speicher fehlt. Festgehalten ist nur der Kontextspeicher des wartenden
//! Modells; den gibt [`Gastgeber::platz`] frei, und zwar nur, wenn es nicht reicht.
//! Die Stufen (Erkennung, Abbildungen, Verdichtung) laufen nacheinander.

use crate::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use crate::werkzeug::{Ansageform, Werkzeug};
use crate::werkzeuge::Einhaengung;
use std::path::{Path, PathBuf};

/// Ein Buch von tausend Seiten braucht rund eine Viertelstunde.
const FRIST_S: u64 = 4 * 3600;
const AUSGABEGRENZE: usize = 64 * 1024;

/// Wo der Skill landet, relativ zum Arbeitsordner.
pub const SKILLORDNER: &str = ".AGENT/skills";

/// Die gelesenen Buecher, je Buch ein Ordner mit Markdown und Grafiken; das
/// Original kommt nicht hierher.
pub const LITERATUR: &str = crate::secure_flow::LITERATURORDNER;

/// So viel braucht die schwerste Stufe neben dem Buch selbst (gemessen: Erkennung
/// knapp 1 GB, Sehmodell und Verdichtung mit kleinem Kontext darunter).
const BEDARF_BYTES: u64 = 2 << 30;

/// **Was das Programm beisteuert, das gerade ein Modell haelt.**
pub trait Gastgeber: Send + Sync {
    /// Das Verzeichnis des geladenen Modells, solange es geladen ist.
    fn artefakt(&self) -> Option<String>;
    /// Sorgt fuer `bytes` freien Speicher, falls noetig; sagt, ob es reicht.
    fn platz(&self, bytes: u64) -> bool;
}

static GASTGEBER: std::sync::Mutex<Option<std::sync::Arc<dyn Gastgeber>>> = std::sync::Mutex::new(None);

/// Das zuletzt geladene Modell gilt.
pub fn gastgeber_setzen(g: std::sync::Arc<dyn Gastgeber>) {
    *GASTGEBER.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
}

/// Die Argumente von `myl buch`. Ist ein Modell geladen, verdichtet es, wenn der
/// Speicher reicht; sonst die tragenden Saetze. Ist keines geladen, nimmt `myl buch`
/// das eingestellte.
fn buchbefehl(myl: &Path, datei: &Path, wurzel: &Path) -> std::process::Command {
    let mut befehl = std::process::Command::new(myl);
    befehl.arg("buch").arg(datei).arg("--ziel").arg(wurzel.join(SKILLORDNER)).arg("--literatur").arg(wurzel.join(LITERATUR));
    let gross = std::fs::metadata(datei).map_or(0, |m| m.len());
    let gastgeber = GASTGEBER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some((g, a)) = gastgeber.and_then(|g| g.artefakt().map(|a| (g, a))) {
        if g.platz(BEDARF_BYTES.saturating_add(gross)) {
            befehl.arg("--modell").arg(a);
        } else {
            befehl.arg("--ohne-modell");
        }
    }
    befehl
}

fn programm() -> Result<PathBuf, String> {
    myl_programm().ok_or_else(|| "`myl` wurde nicht gefunden; es gehoert neben dieses Programm oder in den Suchpfad.".to_string())
}

/// **Liest ein Buch und meldet jede Zeile des Fortschritts**, fuer das Fenster.
/// Gibt den Namen des neuen Skills zurueck.
pub fn lesen_mit_zeilen(datei: &Path, wurzel: &Path, mut zeile: impl FnMut(&str)) -> Result<String, String> {
    use std::io::{BufRead, Read};
    let mut kind = buchbefehl(&programm()?, datei, wurzel)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|f| format!("myl buch: {f}"))?;
    let mut aus = kind.stdout.take().ok_or("myl buch ohne Ausgabe")?;
    let fehler = kind.stderr.take().ok_or("myl buch ohne Fehlerkanal")?;
    let leser = std::thread::spawn(move || {
        let mut t = String::new();
        let _ = aus.read_to_string(&mut t);
        t
    });
    let mut letzte = Vec::new();
    for z in std::io::BufReader::new(fehler).lines().map_while(Result::ok) {
        zeile(&z);
        letzte.push(z);
        if letzte.len() > 4 {
            letzte.remove(0);
        }
    }
    let status = kind.wait().map_err(|f| format!("myl buch: {f}"))?;
    let text = leser.join().unwrap_or_default();
    if !status.success() {
        return Err(letzte.join(" / "));
    }
    let ordner = PathBuf::from(text.lines().rev().find(|z| !z.trim().is_empty()).unwrap_or_default().trim());
    let m: serde_json::Value = std::fs::read(ordner.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    Ok(m["skill"].as_str().unwrap_or_default().to_string())
}

fn name(form: Ansageform) -> &'static str {
    match form {
        Ansageform::Amtlich => "book_to_skill",
        Ansageform::Deutsch => "buch_zu_skill",
    }
}

/// `myl`: neben dem laufenden Programm, sonst im Suchpfad.
pub fn myl_programm() -> Option<PathBuf> {
    let datei = if cfg!(windows) { "myl.exe" } else { "myl" };
    let neben = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(datei)));
    neben.into_iter().chain(std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).map(|d| d.join(datei)).collect::<Vec<_>>())).find(|p| p.is_file())
}

/// Vom Menschen angestossen (`/book`): gleiches Programm, Ausgabe sichtbar.
pub fn von_hand(datei: &Path, arbeitsordner: &Path) -> Result<(), String> {
    match buchbefehl(&programm()?, datei, arbeitsordner).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("myl buch endete mit {}.", s.code().unwrap_or(-1))),
        Err(f) => Err(format!("myl buch: {f}")),
    }
}

/// Das Angebot, wenn es einloesbar ist: `myl` da und Schreiben erlaubt.
pub fn angebot(ein: &Einhaengung, form: Ansageform) -> Option<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    angebot_mit(ein, form, myl_programm()?)
}

fn angebot_mit(ein: &Einhaengung, form: Ansageform, myl: PathBuf) -> Option<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    if !ein.darf_schreiben() {
        return None;
    }
    let deutsch = matches!(form, Ansageform::Deutsch);
    let w = Werkzeug {
        name: name(form).to_string(),
        beschreibung: if deutsch {
            "Macht aus einem Buch im Arbeitsordner (PDF, auch gescannt, EPUB, DOCX, ODT, FB2, HTML, RTF, Markdown, Text) einen Skill unter .AGENT/skills. Kann bei einem dicken Buch viele Minuten dauern. Danach mit skill_suchen finden.".to_string()
        } else {
            "Turns a book in the working directory (PDF, also scanned, EPUB, DOCX, ODT, FB2, HTML, RTF, Markdown, text) into a skill under .AGENT/skills. A thick book can take many minutes. Find it afterwards with search_skill.".to_string()
        },
        parameter: serde_json::json!({
            "type": "object",
            "properties": {
                "pfad": {"type": "string", "description": if deutsch { "Das Buch, relativ zum Arbeitsordner" } else { "The book, relative to the working directory" }}
            },
            "required": ["pfad"]
        }),
    };
    Some((w, Box::new(Buchausfuehrung { ein: ein.clone(), form, myl })))
}

struct Buchausfuehrung {
    ein: Einhaengung,
    form: Ansageform,
    myl: PathBuf,
}

impl Werkzeugausfuehrung for Buchausfuehrung {
    fn name(&self) -> &str {
        name(self.form)
    }

    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let roh = a.get("pfad").and_then(|v| v.as_str()).ok_or_else(|| Werkzeugfehler { grund: "es fehlt das Feld `pfad`".into() })?;
        // ⛔️ Die Einhaengegrenze gilt: kein Buch von ausserhalb.
        // ⚑ Eine Auskunft, kein Fehler: Wer in einem gelesenen Buch etwas nachschlagen
        //   will, ruft das hier gern mit einem erfundenen Pfad auf (das 8B in 4 von 16 Fragen).
        let gelesen = self.ein.wurzel().join(crate::secure_flow::LITERATURORDNER).is_dir();
        let datei = match self.ein.aufloesen(roh, true) {
            Err(_) if gelesen && !self.ein.wurzel().join(roh).exists() => {
                return Ok(match self.form {
                    Ansageform::Deutsch => format!("Die Datei `{roh}` gibt es nicht. Bereits gelesene Bücher liegen unter `literature`; etwas darin nachschlagen geht mit suchen und einem Stichwort aus der Frage."),
                    Ansageform::Amtlich => format!("There is no file `{roh}`. Books already read are under `literature`; to look something up in one, use search_files with a keyword from the question."),
                });
            }
            anders => anders?,
        };
        let mut befehl = buchbefehl(&self.myl, &datei, self.ein.wurzel());
        befehl.current_dir(self.ein.wurzel());
        let aus = myl_senses::prozess::laufen(&mut befehl, FRIST_S, AUSGABEGRENZE).map_err(|f| Werkzeugfehler { grund: format!("myl buch {f}") })?;
        if !aus.gut() {
            return Err(Werkzeugfehler { grund: format!("myl buch ist {}: {}", aus.kopf(), letzte_zeilen(&aus.fehler)) });
        }
        let ordner = PathBuf::from(aus.aus.lines().rev().find(|z| !z.trim().is_empty()).unwrap_or_default().trim());
        Ok(bericht(&ordner, self.form))
    }
}

fn letzte_zeilen(t: &str) -> String {
    let z: Vec<&str> = t.lines().rev().take(4).collect();
    z.into_iter().rev().collect::<Vec<_>>().join(" / ")
}

/// Was das Modell erfaehrt: der Name, um den Skill zu finden, und was drinsteht.
fn bericht(ordner: &Path, form: Ansageform) -> String {
    let m: serde_json::Value = std::fs::read(ordner.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let skill = m["skill"].as_str().unwrap_or_default().to_string();
    let zahl = |k: &str| m[k].as_u64().unwrap_or(0);
    let marke = crate::secure_flow::fremdmarke("book_to_skill");
    let schrift = m["scan"]["schrift"].as_str().unwrap_or("-");
    let lernen = crate::werkzeuge::Dateiwerkzeug::SkillLernen.name(form);
    match form {
        Ansageform::Deutsch => format!(
            "{marke}\nSkill `{skill}` angelegt: {} Abschnitte, {} Abbildungen, Schrift {schrift}. Lesen mit {lernen} und dem Namen `{skill}`.",
            zahl("abschnitte"),
            zahl("bilder")
        ),
        Ansageform::Amtlich => format!(
            "{marke}\nSkill `{skill}` created: {} sections, {} figures, script {schrift}. Read it with {lernen} and the name `{skill}`.",
            zahl("abschnitte"),
            zahl("bilder")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_bericht_ist_fremd_und_nennt_den_skill() {
        let ort = std::env::temp_dir().join(format!("myl-buch-{}", std::process::id()));
        std::fs::create_dir_all(&ort).unwrap();
        std::fs::write(ort.join("manifest.json"), r#"{"abschnitte":3,"bilder":2,"skill":"kurz","scan":{"schrift":"fraktur"}}"#).unwrap();
        let t = bericht(&ort, Ansageform::Deutsch);
        assert!(crate::secure_flow::FREMD_MARKEN.iter().any(|m| t.starts_with(m)), "{t}");
        assert!(t.contains("`kurz`") && t.contains("3 Abschnitte") && t.contains("2 Abbildungen") && t.contains("fraktur"), "{t}");
        std::fs::remove_dir_all(&ort).unwrap();
    }

    #[test]
    fn nur_mit_schreibrecht_und_nur_im_arbeitsordner() {
        let d = tempfile::tempdir().unwrap();
        let nur_lesen = Einhaengung::neu(d.path(), false).unwrap();
        assert!(angebot_mit(&nur_lesen, Ansageform::Amtlich, PathBuf::from("myl")).is_none());
        let ein = Einhaengung::neu(d.path(), true).unwrap();
        let (w, a) = angebot_mit(&ein, Ansageform::Amtlich, PathBuf::from("/gibt/es/nicht")).unwrap();
        assert_eq!(w.name, "book_to_skill");
        // Die Grenze greift, bevor irgendein Programm startet.
        let f = a.ausfuehren(&serde_json::json!({"pfad": "../draussen.pdf"})).unwrap_err();
        assert!(!f.grund.contains("myl buch"), "{f}");
        assert!(a.ausfuehren(&serde_json::json!({})).unwrap_err().grund.contains("pfad"));
    }

    #[test]
    fn ein_erfundener_pfad_verweist_auf_die_gelesenen_buecher() {
        let d = tempfile::tempdir().unwrap();
        let ein = Einhaengung::neu(d.path(), true).unwrap();
        let (_, a) = angebot_mit(&ein, Ansageform::Amtlich, PathBuf::from("/gibt/es/nicht")).unwrap();
        // Ohne gelesene Buecher bleibt es beim Lauf und seinem Fehler.
        assert!(a.ausfuehren(&serde_json::json!({"pfad": "Bibliothek/Buch.pdf"})).is_err());
        std::fs::create_dir_all(d.path().join(crate::secure_flow::LITERATURORDNER)).unwrap();
        let t = a.ausfuehren(&serde_json::json!({"pfad": "Bibliothek/Buch.pdf"})).unwrap();
        assert!(t.contains("search_files") && t.contains("`literature`"), "{t}");
    }

    /// Der Gastgeber ist prozessweit; darum alle Faelle in einer Probe.
    #[test]
    fn das_modell_verdichtet_nur_wenn_der_speicher_reicht() {
        struct G(bool);
        impl Gastgeber for G {
            fn artefakt(&self) -> Option<String> {
                Some("/modelle/a".into())
            }
            fn platz(&self, bytes: u64) -> bool {
                assert!(bytes >= BEDARF_BYTES);
                self.0
            }
        }
        let args = |c: std::process::Command| c.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let (m, d, z) = (Path::new("myl"), Path::new("/b.pdf"), Path::new("/z"));
        gastgeber_setzen(std::sync::Arc::new(G(true)));
        // ⚠️ Die Ordner mit `join` wie im Befehl: Unter Windows steht dort ein
        //   Backslash (`/z\.AGENT/skills`), und das ist ein gueltiger Pfad.
        let (ziel, literatur) = (z.join(SKILLORDNER), z.join(LITERATUR));
        let kopf = ["buch", "/b.pdf", "--ziel", &ziel.to_string_lossy(), "--literatur", &literatur.to_string_lossy()].map(str::to_string);
        let mit = |rest: &[&str]| [&kopf[..], &rest.iter().map(|s| s.to_string()).collect::<Vec<_>>()].concat();
        assert_eq!(args(buchbefehl(m, d, z)), mit(&["--modell", "/modelle/a"]));
        gastgeber_setzen(std::sync::Arc::new(G(false)));
        assert_eq!(args(buchbefehl(m, d, z)), mit(&["--ohne-modell"]));
    }

    #[test]
    fn der_name_folgt_der_ansageform() {
        assert_eq!(name(Ansageform::Amtlich), "book_to_skill");
        assert_eq!(name(Ansageform::Deutsch), "buch_zu_skill");
    }
}
