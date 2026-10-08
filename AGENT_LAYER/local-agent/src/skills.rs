//! **Skills: kurze Anleitungen und Wissensmappen, die der Agent
//! nachschlaegt statt sie im Kontext zu tragen.**
//!
//! # ⚑ Drei Orte, und der Unterschied ist die Zugehoerigkeit
//!
//! | Ort | Was dort liegt | Wer ihn fuellt |
//! |---|---|---|
//! | der **Modusordner** (nur solange ein Modus ihn setzt) | was zu einem **Modus** der Konsole gehoert | der Modus, siehe [`modusordner_setzen`] |
//! | `<einhaengung>/.AGENT/skills/` | was zu **diesem Projekt** gehoert | die Arbeit an diesem Ordner, auch der Agent selbst |
//! | `<konfiguration>/skills/` | was **ueberall** gilt | der Nutzer (`myl skills neu <name>`) |
//! | `AGENT_LAYER/local-skills/` | die **mitgelieferten** Grundskills und die Vorlage | das Repositorium |
//!
//! Bei gleichem Namen gewinnt der naehere Ort: Modus vor Projekt vor
//! eigenen vor mitgelieferten. ⚑ **Die mitgelieferten liegen neben `myl-senses`**
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
pub const MITGELIEFERT: &str = "AGENT_LAYER/local-skills";

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
    Modus,
    Projekt,
    Eigene,
    Mitgeliefert,
}

impl Herkunft {
    pub fn wort(self) -> &'static str {
        match self {
            Herkunft::Modus => "Modus",
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
    let pfad = crate::ort::einstellungsdatei();
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

/// **Der Skillordner eines Modus**, fuer die Dauer des Modus.
///
/// ⚑ **Ein Modus bringt sein Wissen mit, ohne es zu kopieren.** Der Modus
/// eines Moduls der Konsole etwa haengt seinen eigenen Ordner ein, solange
/// er laeuft, und nimmt ihn beim Verlassen wieder heraus. Eine Kopie in
/// den Projektordner waeren zwei Orte, die auseinanderlaufen.
///
/// ⚠️ **Prozessweit**, wie der Modus selbst: Es gibt in einer Konsole nur
/// einen. Pruefungen bauen ihre [`Orte`] von Hand und beruehren das nicht.
static MODUSORDNER: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Setzt (oder mit `None`: entfernt) den Skillordner des laufenden Modus.
pub fn modusordner_setzen(ordner: Option<PathBuf>) {
    if let Ok(mut m) = MODUSORDNER.lock() {
        *m = ordner.filter(|o| o.is_dir());
    }
}

/// Der Skillordner des laufenden Modus, falls es einen gibt.
pub fn modusordner() -> Option<PathBuf> {
    MODUSORDNER.lock().ok().and_then(|m| m.clone())
}

/// Die Orte, in der Reihenfolge des Vorrangs.
///
/// ⚑ **Eine Naht, damit die Vorrangregel pruefbar ist.** Eigener und
/// mitgelieferter Ordner haengen sonst an der Umgebung des ganzen
/// Prozesses. **Eine Pruefung, die dafuer eine Umgebungsvariable setzt,
/// faellt ueber die Nachbarpruefung**, die dieselbe Variable setzt;
/// dieses Projekt hat das zweimal bezahlt (Funde 378 und 385).
#[derive(Debug, Clone)]
pub struct Orte {
    /// Der Ordner eines Modus, solange er gilt ([`modusordner_setzen`]).
    pub modus: Option<PathBuf>,
    pub projekt: Option<PathBuf>,
    pub eigene: PathBuf,
    pub mitgeliefert: Option<PathBuf>,
}

impl Orte {
    pub fn fuer(wurzel: Option<&Path>) -> Self {
        Self {
            modus: modusordner(),
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
    let mut aus = o.modus.as_deref().map(|p| im_ordner(p, Herkunft::Modus)).unwrap_or_default();
    let weitere = [
        (o.projekt.as_deref(), Herkunft::Projekt),
        (Some(o.eigene.as_path()), Herkunft::Eigene),
        (o.mitgeliefert.as_deref(), Herkunft::Mitgeliefert),
    ];
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
    /// **Die Punkte, nach Seltenheit der Woerter gewichtet**, fuer die
    /// Reihenfolge. `punkte` bleibt ungewichtet, damit die Schwellen
    /// (`STARK_AB`, `HINWEIS_AB`) ihre Bedeutung behalten.
    pub rang: u32,
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
        return alle.into_iter().map(|skill| Treffer { skill, punkte: 0, rang: 0 }).collect();
    }
    let w = woerter(anfrage);
    // Je Skill die gefalteten Felder, einmal.
    let felder: Vec<(String, Vec<String>, String, String)> = alle
        .iter()
        .map(|s| (falten(&s.name), s.stichworte.iter().map(|x| falten(x)).collect(), falten(&s.satz), falten(&s.text)))
        .collect();
    let trifft = |x: &str, f: &(String, Vec<String>, String, String)| -> [bool; 4] {
        [
            f.0.contains(x),
            f.1.iter().any(|s| s.contains(x) || x.contains(s.as_str())),
            f.2.contains(x),
            f.3.contains(x),
        ]
    };
    // ⚑ **Seltene Woerter zaehlen mehr** (2026-09-29), die Idee hinter
    //   TF-IDF und BM25, ganzzahlig: Ein Wort, das Name, Stichwort oder
    //   Beschreibung vieler Skills trifft, sagt wenig ueber die Aufgabe.
    //   📌 „Bring das Skript zum Laufen … als Tabelle schreiben" empfahl
    //   ungewichtet `bericht-schreiben` vor `fehlersuche`, weil „schreiben"
    //   im Namen steht. Das Gewicht ist `(n + 1) / df`, mit `df` der Zahl der
    //   Skills, deren Kopf (Name, Stichworte, Beschreibung) das Wort traegt.
    let n = alle.len() as u32;
    let gewicht: Vec<u32> = w
        .iter()
        .map(|x| {
            let df = felder.iter().filter(|f| trifft(x, f)[..3].iter().any(|b| *b)).count() as u32;
            (n + 1) / df.max(1)
        })
        .collect();
    let mut aus: Vec<Treffer> = alle
        .into_iter()
        .zip(felder.iter())
        .filter_map(|(skill, f)| {
            let mut punkte = 0u32;
            let mut rang = 0u32;
            for (x, g) in w.iter().zip(&gewicht) {
                let [im_namen, im_stichwort, im_satz, im_text] = trifft(x, f);
                let p = 6 * u32::from(im_namen) + 4 * u32::from(im_stichwort) + 3 * u32::from(im_satz) + u32::from(im_text);
                punkte += p;
                rang += p * g;
            }
            (punkte > 0).then_some(Treffer { skill, punkte, rang })
        })
        .collect();
    aus.sort_by(|a, b| b.rang.cmp(&a.rang).then_with(|| b.punkte.cmp(&a.punkte)).then_with(|| a.skill.name.cmp(&b.skill.name)));
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

/// **Ab wie vielen Punkten ein Skill im Auftrag genannt wird**: ein Wort
/// im Namen, oder in Stichwort und Beschreibung zugleich. Ein Treffer nur in
/// der Anleitung reicht nicht; ein langer Auftrag beruehrt nebenbei fast
/// jede Anleitung.
pub const HINWEIS_AB: u32 = 6;

/// **Die Skills, die zu einem Auftrag passen, als Zeilen fuer den Auftrag.**
///
/// # ⚑ Warum der Auftrag sie nennt, statt auf die Suche zu warten
///
/// 📌 **Gemessen am 2026-09-28:** Im Loop-Szenario schrieb das 30B einen
/// Bericht „nach unseren Hausregeln fuer Quellenangaben", ohne je nach dem
/// Skill `hausregeln-quellen` zu suchen, und verfehlte das Format ganz; das
/// 27B scheiterte beim Suchen am Aufrufformat. Die Anweisung „nutze Skills"
/// steht im Systemprompt und wird trotzdem uebergangen.
///
/// ⚑ **Nennen und nicht liefern**, nach dem Muster, das sich in
/// Agentensystemen bewaehrt hat (schrittweise Offenlegung): Name und Satz
/// stehen im Auftrag, die Anleitung holt `learn_skill`. So traegt der
/// Kontext nur, was passen koennte, und das Modell entscheidet.
pub fn hinweis_fuer_auftrag(wurzel: Option<&Path>, auftrag: &str, lernen: &str, deutsch: bool) -> Option<String> {
    let treffer: Vec<Treffer> =
        suchen(wurzel, auftrag, 3).into_iter().filter(|t| t.punkte >= HINWEIS_AB).collect();
    if treffer.is_empty() {
        return None;
    }
    let liste: Vec<String> =
        treffer.iter().map(|t| format!("- {}: {}", t.skill.name, kuerzen(&t.skill.satz))).collect();
    Some(if deutsch {
        format!(
            "\n\nSkills, die zu diesem Auftrag passen koennten; `{lernen}` liefert die Anleitung. \
             Passt einer, lerne ihn, bevor du anfaengst:\n{}",
            liste.join("\n")
        )
    } else {
        format!(
            "\n\nSkills that may fit this task; `{lernen}` returns the instructions. \
             If one fits, learn it before you start:\n{}",
            liste.join("\n")
        )
    })
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

// ── Lernen auf Wunsch des Nutzers ──

/// Wie lang eine Eingangsseite sein darf, die der Nutzer zum Lernen
/// vorlegt, in Zeichen.
///
/// ⚑ **Abgewiesen und nicht gekuerzt.** Eine Anleitung, deren Ende
/// fehlt, ist eine andere Anleitung, und das Modell fasste trotzdem
/// zusammen, es habe sie gelernt.
pub const HOECHSTENS_LERNZEICHEN: usize = 48_000;

/// Eine Eingangsseite, die der **Nutzer** zum Lernen ausgewaehlt hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lernseite {
    /// Der Name des Skills: der Ordner, wenn die Datei eine Eingangsseite
    /// ist (`SKILL.md` und die anderen), sonst der Dateiname ohne Endung.
    pub name: String,
    /// Wofuer, wie bei [`Skill::satz`].
    pub satz: String,
    /// Die Anleitung, ohne Kopf.
    pub text: String,
    pub pfad: PathBuf,
    pub bytes: u64,
}

/// **Liest eine Eingangsseite, die der Nutzer selbst ausgewaehlt hat.**
///
/// ⚑ **Ein anderer Weg als [`lernen_in`], und mit Absicht.** Dort nennt
/// das **Modell** einen Namen, und der wird gegen die gefundenen Ordner
/// gehalten und nie zu einem Pfad. Hier zeigt der **Mensch** im Dialog
/// des Systems auf eine Datei; das ist seine Entscheidung wie bei einem
/// Anhang, und sie darf ausserhalb der drei Orte liegen.
///
/// Verlangt wird eine gewoehnliche Datei mit der Endung `.md`, lesbar als
/// Text und hoechstens [`HOECHSTENS_LERNZEICHEN`] lang.
pub fn aus_datei(pfad: &Path) -> Result<Lernseite, String> {
    let md = pfad.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !md {
        return Err(format!("{} ist keine Markdown-Datei (.md)", pfad.display()));
    }
    let meta = std::fs::metadata(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    if !meta.is_file() {
        return Err(format!("{} ist keine Datei", pfad.display()));
    }
    // Vier Bytes je Zeichen sind die Obergrenze von UTF-8: Was darueber
    // liegt, ist sicher zu lang und wird gar nicht erst gelesen.
    if meta.len() > 4 * HOECHSTENS_LERNZEICHEN as u64 {
        return Err(format!("{} ist zu lang fuer einen Skill", pfad.display()));
    }
    let roh = std::fs::read_to_string(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let (kopf, rest) = kopf_und_rest(&roh);
    let text = rest.trim().to_string();
    if text.is_empty() {
        return Err(format!("{} ist leer", pfad.display()));
    }
    let zeichen = text.chars().count();
    if zeichen > HOECHSTENS_LERNZEICHEN {
        return Err(format!(
            "{} hat {zeichen} Zeichen; ein Skill zum Lernen darf hoechstens {HOECHSTENS_LERNZEICHEN} haben",
            pfad.display()
        ));
    }
    let datei = pfad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let eingang = EINGAENGE.iter().any(|e| e.eq_ignore_ascii_case(&datei));
    let name = if eingang { pfad.parent().and_then(Path::file_name) } else { pfad.file_stem() }
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(datei);
    let satz = kopf
        .get("beschreibung")
        .filter(|b| !b.is_empty())
        .map(|b| kuerzen(b))
        .unwrap_or_else(|| erste_aussage(rest));
    Ok(Lernseite { name, satz, text, pfad: pfad.to_path_buf(), bytes: meta.len() })
}

/// **Loest auf, was der Nutzer nennt**: den Namen eines Skills aus den
/// drei Orten, oder den Pfad zu einer Markdown-Datei.
///
/// ⚑ **Der Name zuerst.** Wer `bericht-schreiben` tippt, meint den Skill
/// und nicht eine Datei dieses Namens im Arbeitsordner. Ein Pfad darf mit
/// `~/` beginnen; ein relativer gilt ab `bezug`.
pub fn lernseite_in(o: &Orte, angabe: &str, bezug: &Path) -> Result<Lernseite, String> {
    let angabe = angabe.trim();
    let alle = alle_in(o);
    if let Some(skill) = alle.iter().find(|s| s.name.eq_ignore_ascii_case(angabe)) {
        return aus_datei(&skill.pfad).map(|seite| Lernseite { name: skill.name.clone(), ..seite });
    }
    let pfad = match angabe.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest)).unwrap_or_else(|| PathBuf::from(angabe)),
        None => PathBuf::from(angabe),
    };
    let pfad = if pfad.is_absolute() { pfad } else { bezug.join(pfad) };
    if pfad.is_file() {
        return aus_datei(&pfad);
    }
    let namen: Vec<String> = alle.into_iter().map(|s| s.name).collect();
    Err(format!(
        "\"{angabe}\" ist weder ein Skill noch eine Datei; vorhanden: {}",
        if namen.is_empty() { "keiner".to_string() } else { namen.join(", ") }
    ))
}

/// Wie [`lernseite_in`], mit den drei Orten dieses Arbeitsordners.
pub fn lernseite(wurzel: &Path, angabe: &str) -> Result<Lernseite, String> {
    lernseite_in(&Orte::fuer(Some(wurzel)), angabe, wurzel)
}

/// **Der Auftrag, mit dem das Modell die vorgelegten Skills lernt.**
///
/// Das Modell bekommt die Anleitungen, soll zuerst in **einem Satz**
/// sagen, was es daraus gelernt hat, und danach den Auftrag des Nutzers
/// bearbeiten. ⚑ **Ohne Auftrag bleibt es bei dem Satz**: Wer nur einen
/// Skill vorlegt, will wissen, ob er angekommen ist, und keine Antwort
/// auf eine Frage, die niemand gestellt hat.
///
/// ⚑ **An einer Stelle und nicht im Fenster**: Konsole und Fenster
/// sagen dem Modell so dasselbe.
pub fn lernauftrag(seiten: &[Lernseite], auftrag: &str, deutsch: bool) -> String {
    let auftrag = auftrag.trim();
    if seiten.is_empty() {
        return auftrag.to_string();
    }
    let mehrere = seiten.len() > 1;
    let mut aus = String::new();
    aus.push_str(match (deutsch, mehrere) {
        (true, false) => "Lerne den folgenden Skill, bevor du antwortest.",
        (true, true) => "Lerne die folgenden Skills, bevor du antwortest.",
        (false, false) => "Learn the following skill before you answer.",
        (false, true) => "Learn the following skills before you answer.",
    });
    for s in seiten {
        aus.push_str(&format!(
            "

=== Skill: {} ===
{}
=== {} ===",
            s.name,
            s.text,
            if deutsch { "Ende des Skills" } else { "End of skill" }
        ));
    }
    aus.push_str("

");
    aus.push_str(match (deutsch, mehrere) {
        (true, false) => "Beginne deine Antwort mit genau einem Satz, der zusammenfasst, was du aus dem Skill gelernt hast.",
        (true, true) => "Beginne deine Antwort mit genau einem Satz je Skill, der zusammenfasst, was du daraus gelernt hast.",
        (false, false) => "Begin your answer with exactly one sentence that sums up what you learned from the skill.",
        (false, true) => "Begin your answer with exactly one sentence per skill that sums up what you learned from it.",
    });
    if auftrag.is_empty() {
        aus.push_str(if deutsch { " Schreibe sonst nichts." } else { " Write nothing else." });
    } else {
        aus.push_str(if deutsch {
            " Bearbeite danach diesen Auftrag und wende das Gelernte dabei an:

"
        } else {
            " Then work on this request and apply what you learned:

"
        });
        aus.push_str(auftrag);
    }
    aus
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

    fn lernordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-lernseite-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    #[test]
    fn eine_eingangsseite_heisst_wie_ihr_ordner() {
        let d = lernordner("eingang");
        std::fs::create_dir_all(d.join("bericht-schreiben")).expect("Ordner");
        let p = d.join("bericht-schreiben").join("skill.md");
        std::fs::write(&p, "---\nbeschreibung: Wie ein Bericht entsteht\nstichworte: bericht\n---\n# Bericht\n\nErst sammeln, dann schreiben.\n")
            .expect("schreiben");
        let s = aus_datei(&p).expect("lesbar");
        assert_eq!(s.name, "bericht-schreiben");
        assert_eq!(s.satz, "Wie ein Bericht entsteht");
        assert_eq!(s.text, "# Bericht\n\nErst sammeln, dann schreiben.");
        assert!(!s.text.contains("stichworte"), "der Kopf steht im Lerntext");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn eine_andere_markdown_datei_heisst_wie_sie_selbst() {
        let d = lernordner("frei");
        let p = d.join("hoeflich-antworten.md");
        std::fs::write(&p, "Antworte kurz und freundlich.\n").expect("schreiben");
        let s = aus_datei(&p).expect("lesbar");
        assert_eq!(s.name, "hoeflich-antworten");
        assert_eq!(s.satz, "Antworte kurz und freundlich.");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn was_kein_skill_sein_kann_wird_abgewiesen() {
        let d = lernordner("nein");
        std::fs::write(d.join("notiz.txt"), "Text").expect("schreiben");
        std::fs::write(d.join("leer.md"), "---\nbeschreibung: nichts\n---\n\n").expect("schreiben");
        std::fs::write(d.join("lang.md"), "a".repeat(HOECHSTENS_LERNZEICHEN + 1)).expect("schreiben");
        std::fs::write(d.join("knapp.md"), "a".repeat(HOECHSTENS_LERNZEICHEN)).expect("schreiben");
        std::fs::create_dir_all(d.join("ordner.md")).expect("Ordner");
        assert!(aus_datei(&d.join("notiz.txt")).is_err(), "eine Textdatei geht durch");
        assert!(aus_datei(&d.join("leer.md")).is_err(), "eine leere Seite geht durch");
        assert!(aus_datei(&d.join("lang.md")).is_err(), "eine zu lange Seite geht durch");
        assert!(aus_datei(&d.join("ordner.md")).is_err(), "ein Ordner geht durch");
        assert!(aus_datei(&d.join("fehlt.md")).is_err());
        assert!(aus_datei(&d.join("knapp.md")).is_ok(), "die Grenze selbst wird abgewiesen");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn der_name_eines_skills_geht_vor_einem_pfad() {
        let d = lernordner("angabe");
        let eigene = d.join("eigene");
        std::fs::create_dir_all(eigene.join("bericht")).expect("Ordner");
        std::fs::write(eigene.join("bericht").join("SKILL.md"), "Erst sammeln.\n").expect("schreiben");
        std::fs::write(d.join("bericht"), "keine Anleitung").expect("schreiben");
        std::fs::write(d.join("frei.md"), "Frei gewaehlt.\n").expect("schreiben");
        let o = Orte { modus: None, projekt: None, eigene, mitgeliefert: None };
        let s = lernseite_in(&o, " Bericht ", &d).expect("der Name");
        assert_eq!((s.name.as_str(), s.text.as_str()), ("bericht", "Erst sammeln."));
        let s = lernseite_in(&o, "frei.md", &d).expect("der Pfad ab dem Bezug");
        assert_eq!(s.name, "frei");
        let f = lernseite_in(&o, "gibtsnicht", &d).expect_err("weder noch");
        assert!(f.contains("vorhanden: bericht"), "{f}");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn seite(name: &str, text: &str) -> Lernseite {
        Lernseite { name: name.into(), satz: String::new(), text: text.into(), pfad: PathBuf::new(), bytes: 0 }
    }

    #[test]
    fn ohne_auftrag_bleibt_es_bei_dem_einen_satz() {
        let a = lernauftrag(&[seite("bericht", "Erst sammeln.")], "  ", true);
        assert!(a.contains("=== Skill: bericht ===\nErst sammeln.\n=== Ende des Skills ==="));
        assert!(a.contains("genau einem Satz"));
        assert!(a.ends_with("Schreibe sonst nichts."));
        assert!(!a.contains("Bearbeite danach"));
    }

    #[test]
    fn mit_auftrag_steht_er_am_ende_und_nach_dem_skill() {
        let a = lernauftrag(&[seite("bericht", "Erst sammeln.")], "Schreib einen Bericht.", true);
        assert!(a.ends_with("\n\nSchreib einen Bericht."));
        assert!(a.find("Erst sammeln.").unwrap() < a.find("genau einem Satz").unwrap());
        assert!(a.find("genau einem Satz").unwrap() < a.find("Schreib einen Bericht.").unwrap());
        assert!(!a.contains("Schreibe sonst nichts."));
        let e = lernauftrag(&[seite("report", "Collect first.")], "Write a report.", false);
        assert!(e.starts_with("Learn the following skill") && e.contains("exactly one sentence") && e.ends_with("Write a report."));
    }

    #[test]
    fn mehrere_skills_bekommen_je_einen_satz_und_keiner_ist_der_auftrag_selbst() {
        let a = lernauftrag(&[seite("a", "Eins."), seite("b", "Zwei.")], "", true);
        assert!(a.contains("=== Skill: a ===") && a.contains("=== Skill: b ==="));
        assert!(a.contains("einem Satz je Skill"));
        assert_eq!(lernauftrag(&[], " Hallo ", true), "Hallo");
    }

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

    /// ⚑ **Der Auftrag nennt passende Skills, und nur passende.**
    #[test]
    fn der_auftrag_nennt_passende_skills() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        std::fs::create_dir_all(o.join("hausregeln-datum")).expect("Ordner");
        std::fs::write(
            o.join("hausregeln-datum/SKILL.md"),
            "---\nbeschreibung: Die Hausregel fuer Datumsangaben in Berichten.\nstichworte: datum, datumsformat, hausregel\n---\n\n# Datum\n\nTT.MM.JJJJ\n",
        )
        .expect("schreiben");
        let h = hinweis_fuer_auftrag(Some(d.path()), "Wie schreibe ich ein Datum? Es gibt eine Hausregel.", "learn_skill", false)
            .expect("ein passender Skill");
        assert!(h.contains("- hausregeln-datum: Die Hausregel"), "{h}");
        assert!(!h.contains("TT.MM.JJJJ"), "der Hinweis liefert die Anleitung mit: {h}");
        // Gegenprobe: ein Auftrag, der nichts damit zu tun hat.
        assert!(
            hinweis_fuer_auftrag(Some(d.path()), "Zaehle die Zeilen von notizen.md.", "learn_skill", false)
                .is_none_or(|h| !h.contains("hausregeln-datum")),
            "ein unpassender Skill wurde genannt"
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
        Orte { modus: None, projekt: Some(projekt.to_path_buf()), eigene: eigene.to_path_buf(), mitgeliefert: Some(mit.to_path_buf()) }
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
        // ⚑ **Der Modus geht allem vor**, solange er gilt.
        let modus = tempfile::tempdir().unwrap();
        skill(modus.path(), "gleich", "beschreibung: die Fassung des Modus", "MODUS");
        skill(modus.path(), "nur-modus", "beschreibung: nur im Modus", "X");
        let mit_modus = Orte { modus: Some(modus.path().to_path_buf()), ..orte(p.path(), e.path(), m.path()) };
        let alle = alle_in(&mit_modus);
        assert_eq!(alle.iter().find(|s| s.name == "gleich").unwrap().herkunft, Herkunft::Modus);
        assert_eq!(alle.iter().find(|s| s.name == "zwei").unwrap().herkunft, Herkunft::Eigene, "der Rest bleibt, wie er war");
        assert!(lernen_in(&mit_modus, "gleich", None).unwrap().text.contains("MODUS"));
        assert!(suchen_in(&mit_modus, "nur im Modus", 5).iter().any(|t| t.skill.name == "nur-modus"));
    }

    /// ⚑ **Der Modusordner gilt, solange er gesetzt ist**, und nur, wenn es ihn gibt.
    #[test]
    fn der_modusordner_kommt_und_geht() {
        let d = tempfile::tempdir().unwrap();
        modusordner_setzen(Some(d.path().join("gibtsnicht")));
        assert_eq!(modusordner(), None, "ein Ordner, den es nicht gibt, wird nicht gesetzt");
        modusordner_setzen(Some(d.path().to_path_buf()));
        assert_eq!(Orte::fuer(None).modus.as_deref(), Some(d.path()));
        modusordner_setzen(None);
        assert_eq!(Orte::fuer(None).modus, None);
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
        let o = Orte { modus: None, projekt: None, eigene: d.path().to_path_buf(), mitgeliefert: None };
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
        let o = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(MITGELIEFERT);
        let s = im_ordner(&o, Herkunft::Mitgeliefert);
        assert!(s.len() >= 4, "nur {} mitgelieferte Skills", s.len());
        for x in &s {
            assert!(x.pfad.ends_with("SKILL.md"), "{}: keine SKILL.md", x.name);
            assert!(!x.stichworte.is_empty(), "{}: keine Stichworte", x.name);
            assert!(x.satz.len() > 10, "{}: keine Beschreibung", x.name);
        }
        let vorlage = o.join("skill-erstellen/vorlagen/SKILL.md");
        // Unter Windows checkt Git Textdateien mit CRLF aus; der Parser nimmt
        // beides, die Probe also auch.
        let t = std::fs::read_to_string(&vorlage).expect("die Vorlage fehlt").replace("\r\n", "\n");
        assert!(t.starts_with("---\n") && t.contains("beschreibung:") && t.contains("stichworte:"));
    }
}
