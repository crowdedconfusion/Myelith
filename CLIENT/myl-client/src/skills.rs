//! **Skills: kurze Anleitungen und Wissensmappen, die der Agent
//! nachschlaegt statt sie im Kontext zu tragen.**
//!
//! # ⚑ Drei Orte, und der Unterschied ist die Zugehoerigkeit
//!
//! | Ort | Was dort liegt | Wer ihn fuellt |
//! |---|---|---|
//! | `<einhaengung>/.AGENT/skills/` | was zu **diesem Projekt** gehoert | die Arbeit an diesem Ordner, auch der Agent selbst |
//! | `<konfiguration>/skills/` | was **ueberall** gilt | der Nutzer (`myl skills neu <name>`) |
//! | `CLIENT/myl-skills/` | die **mitgelieferten** Grundskills und die Vorlage | das Repositorium |
//!
//! Bei gleichem Namen gewinnt der naehere Ort: Projekt vor eigenen vor
//! mitgelieferten. ⚑ **Die mitgelieferten liegen neben `myl-senses`**
//! (Wunsch des Projektinhabers, 2026-09-26) und kommen mit jedem Klon,
//! auch auf den GolemOS-Stick.
//!
//! # ⚑ Ein Skill ist ein Ordner mit `SKILL.md`
//!
//! Oben ein kurzer Kopf zwischen zwei `---`: `beschreibung` (ein Satz:
//! wofuer) und `stichworte` (wonach jemand sucht, deutsch und englisch).
//! Darunter die Anleitung. Daneben duerfen `referenz/` und `vorlagen/`
//! liegen; `learn_skill` nennt sie und oeffnet eine davon auf Wunsch.
//! Wissensmappen aus `md_zu_mappe.py` (`MAPPE.md`) bleiben gueltig, nur
//! ohne Kopf.
//!
//! # ⚑ Zwei Werkzeuge, nicht vier
//!
//! `search_skill` sucht (ohne Suchwort nennt es alle), `learn_skill`
//! liefert. Bis zum 2026-09-26 hiessen sie `list_skills` und
//! `read_skill` und lagen nur in `Advanced`. ⚑ **Jetzt liegen sie in
//! `Base`** (Wunsch des Projektinhabers: Das Modell soll selbst suchen,
//! wenn es nicht weiterweiss, und auf „lerne skill …" lernen). Vier
//! Werkzeuge fuer dieselbe Sache waeren drei zu viel: Jedes steht in
//! jeder Ansage und macht einem kleinen Modell die Wahl schwerer.
//!
//! # ⛔️ Warum der Agent dafuer ein Werkzeug braucht
//!
//! Der eigene und der mitgelieferte Ordner liegen **ausserhalb der
//! Einhaengung**. Sie ueber `read_file` zu erreichen hiesse, die Grenze
//! aufzuweichen, die der ganze Dateiwerkzeugsatz zusagt. Die Werkzeuge
//! hier lesen **nur** Skills, halten Namen gegen die gefundenen Ordner
//! und machen aus einem Argument nie einen freien Pfad.

use std::path::{Path, PathBuf};

/// Wie der Ordner heisst, im Projekt und neben den Einstellungen.
pub const ORDNER: &str = "skills";

/// Der mitgelieferte Ordner, relativ zur Wurzel des Repositoriums.
pub const MITGELIEFERT: &str = "CLIENT/myl-skills";

/// Wie viele Skills genannt werden, und wie lang ihre Zeile ist.
///
/// ⚑ **Eine feste Zahl, wie beim Verzeichnis des Mitschnitts.** Was in
/// jede Zusammenfassung geht, darf nicht mit dem Ordner wachsen.
pub const HOECHSTENS_GENANNT: usize = 20;
const ZEILENBREITE: usize = 160;

/// Wie viele Treffer `search_skill` hoechstens nennt.
pub const HOECHSTENS_TREFFER: usize = 5;

/// Wie viele weitere Dateien eines Skills genannt werden.
const HOECHSTENS_DATEIEN: usize = 30;

/// Woher ein Skill kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herkunft {
    Projekt,
    Eigene,
    Mitgeliefert,
}

impl Herkunft {
    pub fn wort(self) -> &'static str {
        match self {
            Herkunft::Projekt => "Projekt",
            Herkunft::Eigene => "eigene",
            Herkunft::Mitgeliefert => "mitgeliefert",
        }
    }
}

/// Ein Skill, wie er gefunden wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Der Ordnername, und zugleich das Argument von `learn_skill`.
    pub name: String,
    /// Wofuer: die `beschreibung` aus dem Kopf, sonst die erste Zeile,
    /// die etwas aussagt.
    pub satz: String,
    /// Wonach jemand sucht.
    pub stichworte: Vec<String>,
    /// Wie viele Zeichen die Eingangsseite hat.
    pub zeichen: usize,
    /// Die Eingangsseite.
    pub pfad: PathBuf,
    /// Der Ordner des Skills.
    pub ordner: PathBuf,
    pub herkunft: Herkunft,
    /// Die Eingangsseite, ohne Kopf.
    pub text: String,
}

/// Die Eingangsseite eines Skills, in der Reihenfolge, in der gesucht wird.
///
/// ⚑ **`SKILL.md` zuerst**: So heisst sie in der Vorlage. `MAPPE.md`
/// schreibt `md_zu_mappe.py`; die uebrigen Namen, damit auch ein von Hand
/// angelegter Ordner gefunden wird.
const EINGAENGE: [&str; 4] = ["SKILL.md", "MAPPE.md", "README.md", "index.md"];

/// Der eigene Ordner, neben den Einstellungen.
///
/// ⚑ **Neben den Einstellungen und nicht im Arbeitsordner**: Was
/// ueberall gelten soll, darf nicht an einem Projekt haengen.
pub fn allgemeiner_ordner() -> PathBuf {
    let pfad = crate::Einstellungen::vorgabepfad();
    pfad.parent().map(|p| p.join(ORDNER)).unwrap_or_else(|| PathBuf::from(ORDNER))
}

/// Der mitgelieferte Ordner, falls dieser Client ein Repositorium findet.
pub fn mitgelieferter_ordner() -> Option<PathBuf> {
    crate::ort::wurzel().map(|w| w.join(MITGELIEFERT)).filter(|p| p.is_dir())
}

/// Der Projektordner unter `.AGENT`.
pub fn projektordner(wurzel: &Path) -> PathBuf {
    wurzel.join(crate::verlauf::ORDNER).join(ORDNER)
}

/// Die drei Orte, in der Reihenfolge des Vorrangs.
///
/// ⚑ **Eine Naht, damit die Vorrangregel pruefbar ist.** Eigener und
/// mitgelieferter Ordner haengen sonst an der Umgebung des ganzen
/// Prozesses. **Eine Pruefung, die dafuer eine Umgebungsvariable setzt,
/// faellt ueber die Nachbarpruefung**, die dieselbe Variable setzt;
/// dieses Projekt hat das zweimal bezahlt (Funde 378 und 385).
#[derive(Debug, Clone)]
pub struct Orte {
    pub projekt: Option<PathBuf>,
    pub eigene: PathBuf,
    pub mitgeliefert: Option<PathBuf>,
}

impl Orte {
    pub fn fuer(wurzel: Option<&Path>) -> Self {
        Self {
            projekt: wurzel.map(projektordner),
            eigene: allgemeiner_ordner(),
            mitgeliefert: mitgelieferter_ordner(),
        }
    }
}

/// **Legt den Projektordner an, mit einer Erklaerung darin.**
///
/// ⚑ **Ein leerer Ordner erklaert sich nicht.** Wer ihn zum ersten Mal
/// sieht, soll ohne Nachfragen wissen, was hineingehoert.
pub fn anlegen(wurzel: &Path) -> std::io::Result<PathBuf> {
    let ordner = projektordner(wurzel);
    if ordner.is_dir() {
        return Ok(ordner);
    }
    std::fs::create_dir_all(&ordner)?;
    let hinweis = "# Skills dieses Projekts\n\
        \n\
        Jeder Unterordner hier ist ein Skill: eine Eingangsseite `SKILL.md`\n\
        mit einem kurzen Kopf (`beschreibung`, `stichworte`) und der\n\
        Anleitung darunter, daneben bei Bedarf `referenz/` und `vorlagen/`.\n\
        Eine Wissensmappe aus einem Buch (`MAPPE.md` mit Kapiteln) geht auch.\n\
        \n\
        Der Agent findet sie mit `search_skill` und lernt einen mit\n\
        `learn_skill`, **auf Nachfrage** und nicht auf Vorrat. Er kann hier\n\
        auch selbst einen anlegen; wie, steht im mitgelieferten Skill\n\
        `skill-erstellen`.\n\
        \n\
        Was hier liegt, gehoert zu diesem Ordner; was ueberall gelten soll,\n\
        gehoert neben die Einstellungen (`myl skills` zeigt, wo).\n";
    std::fs::write(ordner.join("README.md"), hinweis)?;
    Ok(ordner)
}

/// **Der Kopf einer Eingangsseite** (`---`, Zeilen `schluessel: wert`,
/// `---`) und der Rest.
///
/// ⚑ **Ohne YAML-Zerleger.** Der Kopf traegt drei Felder in je einer
/// Zeile; eine Fremdkiste dafuer waere mehr Angriffsflaeche als Nutzen.
/// Englische Schluessel gelten auch, damit Skills aus anderen Quellen
/// ohne Umschreiben gefunden werden.
fn kopf_und_rest(text: &str) -> (std::collections::BTreeMap<String, String>, &str) {
    let mut kopf = std::collections::BTreeMap::new();
    let Some(nach) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) else {
        return (kopf, text);
    };
    let Some(ende) = nach.find("\n---") else { return (kopf, text) };
    for zeile in nach[..ende].lines() {
        if let Some((k, w)) = zeile.split_once(':') {
            let k = match k.trim().to_lowercase().as_str() {
                "description" => "beschreibung".to_string(),
                "keywords" | "tags" => "stichworte".to_string(),
                andere => andere.to_string(),
            };
            kopf.insert(k, w.trim().trim_matches('"').to_string());
        }
    }
    let rest = &nach[ende + 4..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    (kopf, rest)
}

/// Die Skills eines Ordners, alphabetisch.
///
/// ⚑ **Ein Name mit `_` oder `.` vorn wird uebergangen**: So lassen sich
/// Entwuerfe ablegen, ohne dass der Agent sie findet.
pub fn im_ordner(ordner: &Path, herkunft: Herkunft) -> Vec<Skill> {
    let mut aus: Vec<Skill> = std::fs::read_dir(ordner)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                return None;
            }
            let (eingang, roh) = EINGAENGE.iter().find_map(|d| {
                let k = p.join(d);
                std::fs::read_to_string(&k).ok().map(|t| (k, t))
            })?;
            let (kopf, rest) = kopf_und_rest(&roh);
            let satz = kopf
                .get("beschreibung")
                .filter(|b| !b.is_empty())
                .map(|b| kuerzen(b))
                .unwrap_or_else(|| erste_aussage(rest));
            let stichworte = kopf
                .get("stichworte")
                .map(|s| {
                    s.trim_matches(|c| c == '[' || c == ']')
                        .split(',')
                        .map(|w| w.trim().trim_matches('"').to_string())
                        .filter(|w| !w.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            Some(Skill {
                name,
                satz,
                stichworte,
                zeichen: roh.len(),
                pfad: eingang,
                ordner: p.clone(),
                herkunft,
                text: rest.to_string(),
            })
        })
        .collect();
    aus.sort_by(|a, b| a.name.cmp(&b.name));
    aus
}

/// Alle drei Orte zusammen; bei gleichem Namen gewinnt der naehere.
pub fn alle_in(o: &Orte) -> Vec<Skill> {
    let mut aus = o.projekt.as_deref().map(|p| im_ordner(p, Herkunft::Projekt)).unwrap_or_default();
    let weitere = [(Some(o.eigene.as_path()), Herkunft::Eigene), (o.mitgeliefert.as_deref(), Herkunft::Mitgeliefert)];
    for (ordner, herkunft) in weitere {
        for s in ordner.map(|p| im_ordner(p, herkunft)).unwrap_or_default() {
            if !aus.iter().any(|x| x.name == s.name) {
                aus.push(s);
            }
        }
    }
    aus
}

/// Wie [`alle_in`], an den Orten dieses Prozesses.
pub fn alle(wurzel: Option<&Path>) -> Vec<Skill> {
    alle_in(&Orte::fuer(wurzel))
}

// ── Suchen ──

/// Klein, Umlaute ausgeschrieben: „Fehlersuche", „fehlersuche" und
/// „FEHLERSUCHE" sind ein Wort, „Prüfung" und „pruefung" auch.
fn falten(t: &str) -> String {
    let mut aus = String::with_capacity(t.len());
    for c in t.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' => aus.push_str("ae"),
            'ö' => aus.push_str("oe"),
            'ü' => aus.push_str("ue"),
            'ß' => aus.push_str("ss"),
            _ => aus.push(c),
        }
    }
    aus
}

/// **Fuellwoerter, die nichts ueber die Aufgabe sagen**, deutsch und
/// englisch, schon gefaltet.
///
/// 📌 **Gemessen am 2026-09-26:** Das 4B suchte nach „zahlen.py stürzt
/// beim Start ab" und bekam `skill-erstellen`, weil dessen Anleitung
/// „beim Lernen" enthaelt. Dreimal gesucht, dreimal den falschen Skill
/// gelernt, dann war die Schrittgrenze erreicht.
const FUELLWOERTER: [&str; 64] = [
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "und", "oder",
    "aber", "mit", "von", "vom", "zum", "zur", "bei", "beim", "fuer", "auf", "aus", "ist", "sind",
    "war", "hat", "habe", "haben", "wird", "nicht", "kein", "keine", "ich", "mir", "mich", "mein",
    "meine", "sich", "wie", "was", "wenn", "dann", "als", "auch", "noch", "nur", "schon", "sehr",
    "the", "and", "for", "with", "from", "this", "that", "what", "how", "not", "but", "you", "have", "are",
];

/// Die Woerter einer Anfrage, ohne die ganz kurzen und ohne Fuellwoerter.
fn woerter(t: &str) -> Vec<String> {
    let mut w: Vec<String> = falten(t)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !FUELLWOERTER.contains(w))
        .map(str::to_string)
        .collect();
    w.dedup();
    w
}

/// Ein Treffer mit seinen Punkten.
#[derive(Debug, Clone)]
pub struct Treffer {
    pub skill: Skill,
    pub punkte: u32,
}

/// **Sucht Skills nach den Woertern einer Anfrage.**
///
/// ⚑ **Ganzzahlige Punkte, nach dem Ort des Wortes gewichtet**: im Namen
/// 6, in einem Stichwort 4, in der Beschreibung 3, in der Anleitung 1.
/// Ein Stichwort ist die Aussage des Autors, wonach gesucht wird; die
/// Anleitung erwaehnt vieles nebenbei. Ohne Woerter nennt die Suche alle,
/// alphabetisch.
pub fn suchen_in(o: &Orte, anfrage: &str, hoechstens: usize) -> Vec<Treffer> {
    let alle = alle_in(o);
    // ⚑ **Alle nur auf eine leere Anfrage** (oder `*`). Eine Anfrage aus
    //   lauter Fuellwoertern ist keine leere, sondern eine ohne Treffer.
    if anfrage.trim().is_empty() || anfrage.trim() == "*" {
        return alle.into_iter().map(|skill| Treffer { skill, punkte: 0 }).collect();
    }
    let w = woerter(anfrage);
    let mut aus: Vec<Treffer> = alle
        .into_iter()
        .filter_map(|skill| {
            let name = falten(&skill.name);
            let stich: Vec<String> = skill.stichworte.iter().map(|s| falten(s)).collect();
            let satz = falten(&skill.satz);
            let text = falten(&skill.text);
            let mut punkte = 0u32;
            for x in &w {
                if name.contains(x.as_str()) {
                    punkte += 6;
                }
                if stich.iter().any(|s| s.contains(x.as_str()) || x.contains(s.as_str())) {
                    punkte += 4;
                }
                if satz.contains(x.as_str()) {
                    punkte += 3;
                }
                if text.contains(x.as_str()) {
                    punkte += 1;
                }
            }
            (punkte > 0).then_some(Treffer { skill, punkte })
        })
        .collect();
    aus.sort_by(|a, b| b.punkte.cmp(&a.punkte).then_with(|| a.skill.name.cmp(&b.skill.name)));
    aus.truncate(hoechstens);
    aus
}

/// **Ab wie vielen Punkten ein Treffer stark ist**: mindestens ein Wort in
/// Beschreibung, Stichwort oder Name. Was nur in der Anleitung vorkommt,
/// ist ein schwacher Treffer, und `search_skill` sagt das dazu.
pub const STARK_AB: u32 = 3;

pub fn suchen(wurzel: Option<&Path>, anfrage: &str, hoechstens: usize) -> Vec<Treffer> {
    suchen_in(&Orte::fuer(wurzel), anfrage, hoechstens)
}

// ── Lernen ──

/// Was `learn_skill` zurueckgibt.
#[derive(Debug, Clone)]
pub struct Gelernt {
    pub skill: Skill,
    /// Die Anleitung, oder die verlangte weitere Datei.
    pub text: String,
    /// Die weiteren Dateien des Skills, relativ zu seinem Ordner.
    pub dateien: Vec<String>,
}

/// **Liefert einen Skill zum Lernen**: die Anleitung, oder mit `datei`
/// eine seiner weiteren Dateien.
///
/// ⛔️ **Der Name wird gegen die gefundenen Skills gehalten und nie zu
/// einem Pfad gemacht**, und `datei` muss nach dem Aufloesen im Ordner
/// des Skills liegen. `../../etc/passwd` ist weder ein Skill noch eine
/// seiner Dateien; ein Verweis nach draussen (Symlink) ebenso wenig.
pub fn lernen_in(o: &Orte, name: &str, datei: Option<&str>) -> Result<Gelernt, String> {
    let alle = alle_in(o);
    let Some(skill) = alle.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim())).cloned() else {
        let namen: Vec<String> = alle.into_iter().map(|s| s.name).collect();
        return Err(format!(
            "kein Skill namens \"{name}\"; vorhanden: {}",
            if namen.is_empty() { "keiner".to_string() } else { namen.join(", ") }
        ));
    };
    let dateien = weitere_dateien(&skill);
    let text = match datei.map(str::trim).filter(|d| !d.is_empty()) {
        None => skill.text.clone(),
        Some(d) => {
            let rel = Path::new(d);
            if rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err(format!("\"{d}\" liegt nicht im Skill {}", skill.name));
            }
            let echt = skill.ordner.join(rel).canonicalize().map_err(|_| {
                format!("keine Datei \"{d}\" im Skill {}; vorhanden: {}", skill.name, dateien.join(", "))
            })?;
            let heimat = skill.ordner.canonicalize().map_err(|e| e.to_string())?;
            if !echt.starts_with(&heimat) || !echt.is_file() {
                return Err(format!("\"{d}\" liegt nicht im Skill {}", skill.name));
            }
            std::fs::read_to_string(&echt).map_err(|e| format!("{d}: {e}"))?
        }
    };
    Ok(Gelernt { skill, text, dateien })
}

pub fn lernen(wurzel: Option<&Path>, name: &str, datei: Option<&str>) -> Result<Gelernt, String> {
    lernen_in(&Orte::fuer(wurzel), name, datei)
}

/// Die Dateien neben der Eingangsseite, zwei Ebenen tief, gedeckelt.
fn weitere_dateien(skill: &Skill) -> Vec<String> {
    let mut aus = Vec::new();
    let mut stapel = vec![(skill.ordner.clone(), 0usize)];
    while let Some((d, tiefe)) = stapel.pop() {
        let Ok(es) = std::fs::read_dir(&d) else { continue };
        for e in es.flatten() {
            let p = e.path();
            let Ok(art) = e.file_type() else { continue };
            if art.is_dir() && tiefe < 2 {
                stapel.push((p, tiefe + 1));
            } else if art.is_file() && p != skill.pfad {
                if let Ok(rel) = p.strip_prefix(&skill.ordner) {
                    aus.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    aus.sort();
    aus.truncate(HOECHSTENS_DATEIEN);
    aus
}

/// **Die Zeile, die in die Zusammenfassung geht.**
///
/// ⛔️ **Sie nennt und liefert nicht**, und sie ist gedeckelt: Nach einer
/// Verdichtung soll das Modell wissen, **dass** es Skills gibt, nicht
/// deren Inhalt tragen.
pub fn verweis(wurzel: Option<&Path>) -> Option<String> {
    let s = alle(wurzel);
    if s.is_empty() {
        return None;
    }
    let genannt: Vec<String> = s
        .iter()
        .take(HOECHSTENS_GENANNT)
        .map(|x| format!("  {} - {}", x.name, x.satz))
        .collect();
    let mehr = s.len().saturating_sub(HOECHSTENS_GENANNT);
    let schluss = if mehr > 0 {
        format!("\n  ... und {mehr} weitere; `search_skill` findet sie.")
    } else {
        String::new()
    };
    Some(format!(
        "\n\nSkills, die nicht im Kontext stehen. `search_skill` sucht, \
         `learn_skill` holt die Anleitung eines Skills:\n{}{}",
        genannt.join("\n"),
        schluss
    ))
}

fn kuerzen(roh: &str) -> String {
    if roh.chars().count() <= ZEILENBREITE {
        return roh.to_string();
    }
    roh.chars().take(ZEILENBREITE - 1).chain(['…']).collect()
}

/// Die erste Zeile, die etwas aussagt.
fn erste_aussage(text: &str) -> String {
    let roh = text
        .lines()
        .map(str::trim)
        .find(|z| {
            !z.is_empty()
                && !z.starts_with('#')
                && !z.starts_with("---")
                && z.chars().any(char::is_alphabetic)
        })
        .unwrap_or("");
    kuerzen(roh)
}

#[cfg(test)]
mod proben {
    use super::*;

    fn mappe(ordner: &Path, name: &str, satz: &str) {
        let p = ordner.join(name);
        std::fs::create_dir_all(p.join("kapitel")).expect("Ordner");
        std::fs::write(
            p.join("MAPPE.md"),
            format!("# {name}\n\n{satz}\n\n## Kapitel\n\n- `kapitel/01.md`\n"),
        )
        .expect("schreiben");
    }

    /// ⚑ **Der Ordner entsteht mit `.AGENT` und erklaert sich selbst.**
    #[test]
    fn der_projektordner_entsteht_und_erklaert_sich() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        assert!(o.ends_with("skills"), "{o:?}");
        assert!(o.starts_with(d.path().join(".AGENT")), "er liegt nicht unter .AGENT: {o:?}");
        let hinweis = std::fs::read_to_string(o.join("README.md")).expect("README");
        assert!(hinweis.contains("Wissensmappe"), "{hinweis}");
        // Ein zweiter Aufruf fasst nichts an.
        std::fs::write(o.join("README.md"), "eigener Text").expect("schreiben");
        anlegen(d.path()).expect("anlegen");
        assert_eq!(
            std::fs::read_to_string(o.join("README.md")).expect("README"),
            "eigener Text",
            "der zweite Aufruf hat den Hinweis ueberschrieben"
        );
    }

    /// ⚑ **Genannt wird mit Satz, und die Zahl ist gedeckelt.**
    #[test]
    fn der_verweis_nennt_und_liefert_nicht() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        mappe(&o, "kryptografie", "Wie Signaturen und Zufallslosungen hier benutzt werden.");
        mappe(&o, "buchhaltung", "Wie das Ledger Betraege fuehrt.");

        let v = verweis(Some(d.path())).expect("Verweis");
        assert!(v.contains("kryptografie"), "{v}");
        assert!(v.contains("Wie Signaturen"), "der Satz fehlt: {v}");
        // ⛔️ Der Inhalt der Kapitel bleibt draussen.
        assert!(!v.contains("kapitel/01.md"), "der Verweis liefert Inhalt: {v}");

        // Und die Obergrenze haelt.
        for i in 0..30 {
            mappe(&o, &format!("mappe{i:02}"), "Eine weitere Mappe.");
        }
        let v2 = verweis(Some(d.path())).expect("Verweis");
        let zeilen = v2.lines().filter(|z| z.starts_with("  ") && z.contains(" - ")).count();
        assert!(zeilen <= HOECHSTENS_GENANNT, "{zeilen} Zeilen statt hoechstens {HOECHSTENS_GENANNT}");
        assert!(v2.contains("weitere"), "die Kuerzung sagt sich nicht an: {v2}");
    }

    fn skill(ordner: &Path, name: &str, kopf: &str, text: &str) {
        let p = ordner.join(name);
        std::fs::create_dir_all(p.join("referenz")).expect("Ordner");
        std::fs::write(p.join("SKILL.md"), format!("---\n{kopf}\n---\n\n# {name}\n\n{text}\n")).expect("schreiben");
    }

    fn orte(projekt: &Path, eigene: &Path, mit: &Path) -> Orte {
        Orte { projekt: Some(projekt.to_path_buf()), eigene: eigene.to_path_buf(), mitgeliefert: Some(mit.to_path_buf()) }
    }

    /// ⛔️ **Bei gleichem Namen gewinnt der naehere Ort**: Projekt vor
    /// eigenen vor mitgelieferten.
    ///
    /// 📌 **Die erste Fassung dieser Probe legte nur die Projektmappe
    /// an**, also genau den Fall, den die Regel gar nicht betrifft: Sie
    /// blieb gruen, als die Regel versuchsweise umgedreht wurde. **Eine
    /// Gegenprobe, die nicht beisst, ist ein Fund**, hier an der
    /// Pruefung.
    #[test]
    fn der_naehere_ort_gewinnt() {
        let (p, e, m) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        skill(p.path(), "gleich", "beschreibung: die Fassung des Projekts", "P");
        skill(e.path(), "gleich", "beschreibung: die eigene Fassung", "E");
        skill(m.path(), "gleich", "beschreibung: die mitgelieferte Fassung", "M");
        skill(e.path(), "zwei", "beschreibung: eigene", "E2");
        skill(m.path(), "zwei", "beschreibung: mitgeliefert", "M2");
        skill(m.path(), "nur-mit", "beschreibung: nur mitgeliefert", "M3");
        let o = orte(p.path(), e.path(), m.path());
        let alle = alle_in(&o);
        let finde = |n: &str| alle.iter().find(|s| s.name == n).unwrap();
        assert_eq!(finde("gleich").herkunft, Herkunft::Projekt);
        assert_eq!(finde("zwei").herkunft, Herkunft::Eigene);
        assert_eq!(finde("nur-mit").herkunft, Herkunft::Mitgeliefert);
        assert_eq!(alle.iter().filter(|s| s.name == "gleich").count(), 1, "doppelt in der Liste");
        assert!(lernen_in(&o, "GLEICH", None).unwrap().text.contains('P'));
        assert!(lernen_in(&o, "gibtsnicht", None).unwrap_err().contains("nur-mit"), "die Fehlermeldung nennt, was es gibt");
    }

    /// ⚑ **Der Kopf traegt Beschreibung und Stichworte**, auch mit
    /// englischen Schluesseln; ein Entwurf mit `_` bleibt verborgen.
    #[test]
    fn der_kopf_wird_gelesen_und_entwuerfe_bleiben_verborgen() {
        let d = tempfile::tempdir().unwrap();
        skill(d.path(), "fehlersuche", "beschreibung: Einen Fehler systematisch finden.\nstichworte: bug, debug, Absturz", "Schritt 1");
        skill(d.path(), "englisch", "description: \"An English one.\"\nkeywords: [alpha, beta]", "Body");
        skill(d.path(), "_entwurf", "beschreibung: halb fertig", "x");
        let s = im_ordner(d.path(), Herkunft::Eigene);
        let namen: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(namen, ["englisch", "fehlersuche"]);
        assert_eq!(s[1].satz, "Einen Fehler systematisch finden.");
        assert_eq!(s[1].stichworte, ["bug", "debug", "Absturz"]);
        assert_eq!(s[0].satz, "An English one.");
        assert_eq!(s[0].stichworte, ["alpha", "beta"]);
        assert!(!s[1].text.contains("beschreibung:"), "der Kopf steht noch in der Anleitung");
    }

    /// ⚑ **Die Suche gewichtet**: Name vor Stichwort vor Beschreibung vor
    /// Anleitung, und Umlaute und Grossschreibung zaehlen nicht.
    #[test]
    fn die_suche_findet_nach_stichwort_und_gewichtet() {
        let (p, e, m) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        skill(m.path(), "fehlersuche", "beschreibung: Einen Fehler finden.\nstichworte: bug, debug, absturz", "Reproduzieren.");
        skill(m.path(), "bericht-schreiben", "beschreibung: Einen Bericht verfassen.\nstichworte: report", "Auch bei einem Fehler: erst Fakten.");
        skill(m.path(), "pruefung", "beschreibung: Arbeit gegenlesen.\nstichworte: review", "Nichts mit Absturz.");
        let o = orte(p.path(), e.path(), m.path());
        let t = suchen_in(&o, "Mein Programm hat einen BUG und stuerzt ab", 5);
        assert_eq!(t.first().map(|x| x.skill.name.as_str()), Some("fehlersuche"), "{t:?}");
        let t = suchen_in(&o, "Prüfung", 5);
        assert_eq!(t[0].skill.name, "pruefung", "Umlaut nicht gefaltet: {t:?}");
        // 📌 Fuellwoerter zaehlen nicht: „beim" steht in der Anleitung von
        //    `bericht-schreiben`, und das allein ist kein Treffer.
        std::fs::write(m.path().join("bericht-schreiben/SKILL.md"), "---\nbeschreibung: Einen Bericht verfassen.\nstichworte: report\n---\nAuch beim Fehler: erst Fakten.").unwrap();
        assert!(suchen_in(&o, "beim und der", 5).is_empty(), "Fuellwoerter treffen");
        let t = suchen_in(&o, "fehler", 5);
        assert_eq!(t.iter().map(|x| x.skill.name.as_str()).collect::<Vec<_>>(), ["fehlersuche", "bericht-schreiben"], "Name vor Anleitung");
        assert!(suchen_in(&o, "xylophon", 5).is_empty());
        assert_eq!(suchen_in(&o, "", 5).len(), 3, "ohne Suchwort alle");
        assert_eq!(suchen_in(&o, "fehler bug report review absturz", 2).len(), 2, "gedeckelt");
    }

    /// ⛔️ **`learn_skill` mit `datei` bleibt im Ordner des Skills.**
    #[test]
    fn lernen_nennt_die_dateien_und_bleibt_im_skill() {
        let d = tempfile::tempdir().unwrap();
        let o = Orte { projekt: None, eigene: d.path().to_path_buf(), mitgeliefert: None };
        skill(d.path(), "bericht", "beschreibung: Berichte.", "Die Anleitung.");
        std::fs::write(d.path().join("bericht/referenz/gliederung.md"), "Die Gliederung.").unwrap();
        std::fs::write(d.path().join("geheim.txt"), "nicht lesen").unwrap();
        let g = lernen_in(&o, "bericht", None).unwrap();
        assert!(g.text.contains("Die Anleitung."));
        assert_eq!(g.dateien, ["referenz/gliederung.md"]);
        assert_eq!(lernen_in(&o, "bericht", Some("referenz/gliederung.md")).unwrap().text, "Die Gliederung.");
        for boese in ["../geheim.txt", "/etc/passwd", "referenz/../../geheim.txt", "./referenz/gliederung.md"] {
            assert!(lernen_in(&o, "bericht", Some(boese)).is_err(), "{boese} wurde gelesen");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(d.path().join("geheim.txt"), d.path().join("bericht/referenz/link.md")).unwrap();
            assert!(lernen_in(&o, "bericht", Some("referenz/link.md")).is_err(), "ein Verweis nach draussen wurde gelesen");
        }
        assert!(lernen_in(&o, "bericht", Some("referenz/fehlt.md")).unwrap_err().contains("gliederung.md"));
    }

    /// Der mitgelieferte Ordner liegt im Repositorium und traegt die
    /// Vorlage und die Beispiele; jeder Skill dort hat einen Kopf.
    #[test]
    fn die_mitgelieferten_skills_haben_einen_kopf() {
        let o = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("myl-skills");
        let s = im_ordner(&o, Herkunft::Mitgeliefert);
        assert!(s.len() >= 4, "nur {} mitgelieferte Skills", s.len());
        for x in &s {
            assert!(x.pfad.ends_with("SKILL.md"), "{}: keine SKILL.md", x.name);
            assert!(!x.stichworte.is_empty(), "{}: keine Stichworte", x.name);
            assert!(x.satz.len() > 10, "{}: keine Beschreibung", x.name);
        }
        let vorlage = o.join("skill-erstellen/vorlagen/SKILL.md");
        let t = std::fs::read_to_string(&vorlage).expect("die Vorlage fehlt");
        assert!(t.starts_with("---\n") && t.contains("beschreibung:") && t.contains("stichworte:"));
    }
}
