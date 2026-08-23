//! Artefakte finden, bauen und **gegen den veröffentlichten Digest prüfen**.
//!
//! ## Warum die Prüfung der eigentliche Punkt ist
//!
//! Der Zweck dieses Clients ist der Nachweis, dass zwei verschiedene
//! Maschinen bitgleich rechnen. Das setzt voraus, dass beide Maschinen
//! **dasselbe Modell** rechnen, und genau das ist nicht selbstverständlich.
//!
//! Der Artefaktbau war bis 2026-08-20 nur auf derselben Maschine
//! reproduzierbar (Fund 32): Die Aktivierungsskalen entstanden aus einem
//! Gleitkomma-Durchlauf, und **3 von 314** Skaleneinträgen saßen innerhalb
//! von 0,01 % einer Zweierpotenz-Grenze, der knappste bei 0,003 %. Eine
//! andere BLAS-Version reicht, um einen davon umzuwerfen; ein gekippter
//! Shift ändert die Artefaktbytes, also das Modell.
//!
//! Ohne Digest-Prüfung sähe das im Ergebnis **wie eine gescheiterte
//! Hardware-Bitgleichheit aus**, der Testclient würde also genau das
//! Gegenteil dessen berichten, wofür es ihn gibt. Deshalb prüft er zuerst,
//! ob überhaupt dasselbe Modell vorliegt, und sagt bei Abweichung klar,
//! dass das Artefakt und nicht die Hardware das Problem ist.
//!
//! Seit dem Skalenpaket (`INTEGER_LLM/scale_packs/`) ist der Bau
//! plattformübergreifend bitgleich: Die Skalen und LUTs sind versioniert,
//! die verbleibende Gewichtsquantisierung ist `round(W · 2^shift)` und
//! damit exakt. Die Prüfung bleibt trotzdem: eine Zusicherung, die man
//! nicht nachrechnet, ist eine Hoffnung.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Ein Modell, für das ein Skalenpaket veröffentlicht ist.
#[derive(Clone)]
pub struct Bekannt {
    pub name: String,
    pub theta_v: String,
    pub digest: String,
}

/// Zustand eines Modells auf dieser Maschine.
pub enum Zustand {
    /// Artefakte da und Digest stimmt: bereit für einen Vergleichslauf.
    Bereit { pfad: PathBuf },
    /// Artefakte da, aber Digest weicht ab. **Kein Hardware-Befund.**
    Abweichend { pfad: PathBuf, ist: String, soll: String },
    /// Keine Artefakte vorhanden.
    Fehlt,
}

fn sha256_datei(p: &Path) -> std::io::Result<String> {
    let mut h = Sha256::new();
    h.update(fs::read(p)?);
    Ok(format!("{:x}", h.finalize()))
}

/// Die drei Dateien, an denen die Identität eines Artefakts hängt.
///
/// `theta_v.json` enthält die Hashes von `weights_manifest.json`,
/// `scales.json` und `luts.json`; jene wiederum den Hash jeder einzelnen
/// Gewichts- und LUT-Datei, und der Lader prüft diese Kette beim Laden.
/// Wer diese drei trifft, hat dasselbe Modell.
const ANKER: [&str; 3] = ["model_config.json", "theta_v.json", "tokenizer.json"];

/// Digest über die Ankerkette. Muss zeichengleich zu
/// `tools/skalenpaket_bauen.py` sein: sonst prüft der Client gegen eine
/// andere Rechnung als die, die den veröffentlichten Wert erzeugt hat.
///
/// **Nicht über den Verzeichnisinhalt.** Diese Fassung gab es, und sie hat
/// am 2026-08-20 sofort falschen Alarm ausgelöst: Ein Synchronisations-
/// werkzeug hatte 432 inhaltsgleiche Kopien in den Artefaktordner gelegt
/// (`theta_v 2.json` und so fort). Der Lader ignoriert solche Dateien; sie
/// ändern das Modell nicht. Ein Anker, der bei belanglosen Streudateien
/// anschlägt, macht den echten Befund unglaubwürdig, und der echte
/// Befund ist der einzige Zweck. Nebenbei: über 8,7 GB zu hashen dauerte
/// Minuten und ließ den Client beim Start stumm dastehen.
pub fn artefakt_digest(dir: &Path) -> std::io::Result<(String, usize)> {
    let mut eintraege: Vec<(String, String)> = Vec::new();
    for name in ANKER {
        eintraege.push((name.to_string(), sha256_datei(&dir.join(name))?));
    }
    eintraege.sort();
    let text = eintraege
        .iter()
        .map(|(n, h)| format!("{}  {}", n, h))
        .collect::<Vec<_>>()
        .join("\n");
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    Ok((format!("{:x}", h.finalize()), eintraege.len()))
}

/// Liest `INTEGER_LLM/scale_packs/REGISTER.json`.
///
/// Bewusst ohne JSON-Crate: Der Client soll auf einer fremden Maschine mit
/// möglichst wenig Voraussetzungen bauen (siehe Modul-Doku von `main.rs`).
/// Das Register hat ein festes, flaches Format, das der Erzeuger schreibt.
pub fn register(repo: &Path) -> Result<Vec<Bekannt>, String> {
    let pfad = repo.join("INTEGER_LLM/scale_packs/REGISTER.json");
    let text = fs::read_to_string(&pfad)
        .map_err(|e| format!("{} nicht lesbar: {}", pfad.display(), e))?;

    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let (mut theta, mut digest) = (String::new(), String::new());
    for zeile in text.lines() {
        let t = zeile.trim();
        // Modellname: einziger Schlüssel auf Einrückungsebene 2 mit "{".
        if t.ends_with("\": {") && zeile.starts_with("  \"") {
            name = Some(t.trim_start_matches('"').trim_end_matches("\": {").to_string());
        } else if let Some(v) = feld(t, "artefakt_digest_sha256") {
            digest = v;
        } else if let Some(v) = feld(t, "theta_v") {
            theta = v;
        }
        if t == "}," || t == "}" {
            if let (Some(n), false) = (name.clone(), digest.is_empty()) {
                out.push(Bekannt { name: n, theta_v: theta.clone(), digest: digest.clone() });
                name = None;
                digest.clear();
            }
        }
    }
    if out.is_empty() {
        return Err(format!("keine Einträge in {}", pfad.display()));
    }
    Ok(out)
}

/// Ein Eintrag aus dem kuratierten Modellkatalog.
///
/// **Getrennt von [`Bekannt`], und das ist Absicht.** `Bekannt` kommt aus
/// `scale_packs/REGISTER.json` und trägt, was **gemessen** wurde: Digest
/// und θ_v-Stand. Dieser Eintrag kommt aus `models/KATALOG.json` und
/// trägt, was jemand **entschieden** hat: Herkunft, Revision, Lizenz,
/// Status. Aus keinem Artefakt ableitbar, und deshalb von Hand gepflegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Katalogeintrag {
    pub name: String,
    pub anzeigename: String,
    pub hf_repo: String,
    /// Verzeichnisname unter `INTEGER_LLM/models/`.
    pub hf_verzeichnis: String,
    pub hf_revision: String,
    pub lizenz: String,
    pub parameter: String,
    pub gewichte_anzeige: String,
    pub artefakt_anzeige: String,
    pub status: String,
    pub bemerkung: String,
}

/// Liest `INTEGER_LLM/models/KATALOG.json`.
///
/// Bewusst ohne JSON-Crate, aus demselben Grund wie [`register`]: Der
/// Client soll auf einer fremden Maschine mit möglichst wenig
/// Voraussetzungen bauen. Schlüssel, die mit `_` beginnen, sind
/// Erläuterungen für Menschen und werden übersprungen.
pub fn katalog(repo: &Path) -> Result<Vec<Katalogeintrag>, String> {
    let pfad = repo.join("INTEGER_LLM/models/KATALOG.json");
    let text = fs::read_to_string(&pfad)
        .map_err(|e| format!("{} nicht lesbar: {}", pfad.display(), e))?;
    Ok(katalog_lesen(&text))
}

/// Der reine Teil von [`katalog`]: aus Text wird eine Liste.
///
/// Getrennt, damit er ohne Dateisystem prüfbar ist.
pub fn katalog_lesen(text: &str) -> Vec<Katalogeintrag> {
    let mut out: Vec<Katalogeintrag> = Vec::new();
    let mut name: Option<String> = None;
    let mut felder: Vec<(String, String)> = Vec::new();

    for zeile in text.lines() {
        let t = zeile.trim();
        if t.ends_with("\": {") && zeile.starts_with("  \"") {
            let neu = t.trim_start_matches('"').trim_end_matches("\": {").to_string();
            // Kommentarblöcke wie `_status_bedeutung` sind keine Modelle.
            name = if neu.starts_with('_') { None } else { Some(neu) };
            felder.clear();
            continue;
        }
        if name.is_some() {
            for schluessel in [
                "anzeigename",
                "hf_repo",
                "hf_verzeichnis",
                "hf_revision",
                "lizenz",
                "parameter",
                "gewichte_anzeige",
                "artefakt_anzeige",
                "status",
                "bemerkung",
            ] {
                if let Some(v) = feld(t, schluessel) {
                    felder.push((schluessel.to_string(), v));
                }
            }
        }
        if t == "}," || t == "}" {
            if let Some(n) = name.take() {
                let hole = |k: &str| -> String {
                    felder
                        .iter()
                        .find(|(s, _)| s == k)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default()
                };
                out.push(Katalogeintrag {
                    // Ohne Anzeigenamen ist der Eintrag unbrauchbar; dann
                    // steht wenigstens der Schlüssel da statt einer Leere.
                    anzeigename: {
                        let a = hole("anzeigename");
                        if a.is_empty() { n.clone() } else { a }
                    },
                    hf_repo: hole("hf_repo"),
                    hf_verzeichnis: hole("hf_verzeichnis"),
                    hf_revision: hole("hf_revision"),
                    lizenz: hole("lizenz"),
                    parameter: hole("parameter"),
                    gewichte_anzeige: hole("gewichte_anzeige"),
                    artefakt_anzeige: hole("artefakt_anzeige"),
                    status: hole("status"),
                    bemerkung: hole("bemerkung"),
                    name: n,
                });
                felder.clear();
            }
        }
    }
    out
}

/// Der Katalogeintrag zu einem Modell, falls es einen gibt.
pub fn katalogeintrag(repo: &Path, modell: &str) -> Option<Katalogeintrag> {
    katalog(repo).ok()?.into_iter().find(|k| k.name == modell)
}

fn feld(zeile: &str, schluessel: &str) -> Option<String> {
    let praefix = format!("\"{}\": ", schluessel);
    let rest = zeile.trim().strip_prefix(&praefix)?;
    Some(entpacken(rest.trim_end_matches(',').trim_matches('"')))
}

/// Löst die JSON-Fluchtsequenzen auf, die in diesen Dateien vorkommen.
///
/// **Nötig geworden, als der Katalog Text für Menschen aufnahm**
/// (2026-08-22): Ein `\n` in einer Bemerkung erschien wörtlich im Menü,
/// weil der Leser den Wert unverändert durchreichte. Ein Anführungszeichen
/// hätte die Zeile sogar zerlegt.
///
/// Bewusst nur diese vier: Der Leser ist kein JSON-Parser und soll keiner
/// werden (siehe [`register`]). Was er nicht kennt, lässt er stehen, statt
/// es zu erraten.
fn entpacken(roh: &str) -> String {
    let mut out = String::with_capacity(roh.len());
    let mut zeichen = roh.chars();
    while let Some(c) = zeichen.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match zeichen.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(anderes) => {
                out.push('\\');
                out.push(anderes);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Prüft ein Modell auf dieser Maschine.
pub fn pruefen(repo: &Path, m: &Bekannt) -> Zustand {
    let pfad = repo.join("INTEGER_LLM/artifacts").join(&m.name);
    if !pfad.join("weights_manifest.json").is_file() {
        return Zustand::Fehlt;
    }
    match artefakt_digest(&pfad) {
        Ok((ist, _)) if ist == m.digest => Zustand::Bereit { pfad },
        Ok((ist, _)) => Zustand::Abweichend { pfad, ist, soll: m.digest.clone() },
        Err(_) => Zustand::Fehlt,
    }
}

/// Die Anleitung, die ausgegeben wird, wenn Artefakte fehlen.
pub fn bauanleitung(repo: &Path, modell: &str) -> String {
    format!(
        "So entstehen die Artefakte für {m} (einmalig):\n\
         \n\
         1. Gewichte von Hugging Face holen: sie werden NICHT mitgeliefert:\n\
         \x20     huggingface-cli download Qwen/{hf} --local-dir INTEGER_LLM/models/{hf}\n\
         \n\
         2. Artefakte bauen. Das versionierte Skalenpaket unter\n\
         \x20  INTEGER_LLM/scale_packs/{m}/ wird automatisch verwendet:\n\
         \x20     INTEGER_LLM_MODEL={m} python -m calibrate.src.main\n\
         \n\
         Der Bau dauert Sekunden statt Minuten, weil die Aktivierungsstatistik\n\
         entfällt, und genau deshalb ist er auf jeder Maschine bitgleich.\n\
         Danach diesen Befehl erneut ausführen; der Digest wird geprüft.",
        m = modell,
        hf = hf_id(repo, modell),
    )
}

/// Wo die Gewichte dieses Modells auf der Platte liegen.
///
/// **Getrennt vom Beschaffen, und das war ein Fund** (2026-08-22): Hier
/// stand eine gemeinsame Funktion mit einem Rückfall auf den
/// Modellnamen. Der Modellschlüssel (`qwen2.5-0.5b`) und der
/// Verzeichnisname (`Qwen2.5-0.5B`) unterscheiden sich aber **nur in der
/// Groß- und Kleinschreibung**, und auf einem Dateisystem, das die nicht
/// unterscheidet, fiel das nicht auf. Auf macOS lief der Test durch, auf
/// dem Linux-Runner der CI nicht. Ein Nutzer unter Linux hätte in
/// „Artefakte und Gewichte löschen" seine Gewichte nicht aufgelistet
/// bekommen und geglaubt, sie seien weg.
///
/// Die Suche geht deshalb in drei Stufen: Katalog, dann ein Vergleich
/// **ohne Rücksicht auf Groß- und Kleinschreibung** über die vorhandenen
/// Verzeichnisse, dann der Modellname unverändert. Nur die erste Stufe
/// ist eine Auskunft; die zweite ist eine Suche, und die dritte sagt
/// ehrlich, dass nichts bekannt ist.
fn gewichte_verzeichnis(repo: &Path, modell: &str) -> PathBuf {
    let models = repo.join("INTEGER_LLM/models");
    if let Some(k) = katalogeintrag(repo, modell) {
        if !k.hf_verzeichnis.is_empty() {
            return models.join(k.hf_verzeichnis);
        }
    }
    if let Ok(eintraege) = fs::read_dir(&models) {
        for e in eintraege.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case(modell) && e.path().is_dir() {
                return e.path();
            }
        }
    }
    models.join(modell)
}

/// Anzeigename der Gewichte für Meldungen und Anleitungen.
///
/// Ohne Katalogeintrag der Modellname selbst: **kein** Rückfall auf ein
/// anderes Modell. Vorher stand hier `_ => "Qwen2.5-0.5B"`, und ein
/// drittes Modell hätte damit die Gewichte von Qwen2.5-0,5B geladen und
/// wäre erst beim Bau aufgefallen, mit einer Meldung, die nach allem
/// aussieht außer nach der Ursache.
fn hf_id(repo: &Path, modell: &str) -> String {
    katalogeintrag(repo, modell)
        .map(|k| k.hf_verzeichnis)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| modell.to_string())
}

// ---------------------------------------------------------------------------
// Beschaffung: suchen, auswählen, notfalls herunterladen und bauen
// ---------------------------------------------------------------------------

/// Eingabekanal für Rückfragen: liefert die Antwort auf einen Prompt,
/// oder `None` bei Dateiende. `Option<&mut …>` als Ganzes ist `None`,
/// wenn der Client nicht-interaktiv läuft, dann wird nicht gefragt und
/// folglich auch nichts heruntergeladen.
pub type Rueckfrage<'a> = Option<&'a mut dyn FnMut(&str) -> Option<String>>;

/// Kürzel für den n-ten Eintrag einer erzeugten Auswahlliste.
///
/// Ziffern, dann Buchstaben: dieselbe Regel wie im Menü, damit der Weg
/// über die Tastenkürzel überall gleich aussieht.
fn kuerzel(i: usize) -> char {
    match i {
        0..=8 => char::from(b'1' + i as u8),
        9..=34 => char::from(b'a' + (i - 9) as u8),
        _ => ' ',
    }
}

/// Ein auf dieser Maschine gefundenes Artefaktverzeichnis.
#[derive(Clone)]
pub struct Gefunden {
    pub name: String,
    pub pfad: PathBuf,
    /// Steht das Modell im Register? Wenn nicht, ist sein Digest nicht
    /// prüfbar, das muss vor einem Vergleichslauf gesagt werden.
    pub im_register: bool,
}

/// Durchsucht `INTEGER_LLM/artifacts/` nach vollständigen Artefakten.
///
/// Gefunden wird jedes Verzeichnis mit `weights_manifest.json`: auch
/// solche, die nicht im Register stehen. Sie werden mitgeführt und als
/// ungeprüft markiert, statt sie zu verschweigen: Wer ein eigenes Modell
/// gebaut hat, soll es benutzen können und dabei wissen, dass der Digest
/// dafür keine Aussage macht.
pub fn suchen(repo: &Path) -> Vec<Gefunden> {
    let wurzel = repo.join("INTEGER_LLM/artifacts");
    let bekannt = register(repo).unwrap_or_default();
    let mut out = Vec::new();
    let Ok(eintraege) = fs::read_dir(&wurzel) else {
        return out;
    };
    let mut pfade: Vec<PathBuf> = eintraege.filter_map(|e| e.ok().map(|e| e.path())).collect();
    pfade.sort();
    for p in pfade {
        if !p.join("weights_manifest.json").is_file() {
            continue;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        // **Hier wird bewusst NICHT gehasht.** Ein Digest über ein
        // 8,7-GB-Artefakt dauert Sekunden bis Minuten; das für jedes
        // gefundene Modell zu tun, nur um eine Auswahlliste zu zeigen,
        // ist Verschwendung, und es lässt den Client beim Start minutenlang
        // stumm dastehen. Geprüft wird erst, was auch benutzt wird.
        let im_register = bekannt.iter().any(|b| b.name == name);
        out.push(Gefunden { name, pfad: p, im_register });
    }
    out
}

/// Pfad zum Python der Kalibrierungs-Umgebung, falls vorhanden.
fn venv_python(repo: &Path) -> Option<PathBuf> {
    // Beide venv-Layouts: POSIX legt den Interpreter unter `bin/`, Windows
    // unter `Scripts/`. Die erste Fassung kannte nur `bin/python` und war
    // damit auf Windows blind.
    for rel in [
        "INTEGER_LLM/calibrate/.venv/bin/python",
        "INTEGER_LLM/calibrate/.venv/bin/python3",
        "INTEGER_LLM/calibrate/.venv/Scripts/python.exe",
    ] {
        let p = repo.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Findet einen Python-Interpreter, der die Kalibrierung ausführen kann.
///
/// **Warum das mehr ist als ein Pfad.** Die virtuelle Umgebung
/// `INTEGER_LLM/calibrate/.venv` ist gitignored, sie ist 861 MB groß und
/// gehört nicht ins Repository. Auf einem frischen Klon gibt es sie also
/// nicht. Die erste Fassung dieser Funktion suchte nur dort und meldete
/// „.venv fehlt": womit der gesamte Download- und Baupfad auf jeder
/// Maschine außer der Entwicklermaschine tot war.
///
/// Gesucht wird deshalb in dieser Reihenfolge: die venv (beide Layouts),
/// dann `python3` und `python` im Suchpfad. Gefunden ist nicht genug: Es
/// wird geprüft, ob die nötigen Pakete importierbar sind, denn ein
/// System-Python ohne `torch` scheitert erst nach dem Download.
fn python_finden(repo: &Path) -> Result<PathBuf, String> {
    let mut kandidaten: Vec<PathBuf> = Vec::new();
    if let Some(p) = venv_python(repo) {
        kandidaten.push(p);
    }
    // Reihenfolge mit Bedacht: Unter Windows ist `python3` haeufig nur ein
    // Platzhalter, der den Microsoft Store oeffnet; er scheitert beim
    // Importtest und wird deshalb uebersprungen. `py` ist der offizielle
    // Windows-Starter und findet auch Installationen, die nicht im
    // Suchpfad stehen.
    kandidaten.push(PathBuf::from("python3"));
    kandidaten.push(PathBuf::from("python"));
    kandidaten.push(PathBuf::from("py"));

    let mut gesehen = Vec::new();
    for k in kandidaten {
        let pruefung = std::process::Command::new(&k)
            .args(["-c", "import torch, transformers, huggingface_hub"])
            .current_dir(repo)
            .output();
        match pruefung {
            Ok(a) if a.status.success() => return Ok(k),
            Ok(_) => gesehen.push(format!("{} gefunden, aber Pakete fehlen", k.display())),
            Err(_) => gesehen.push(format!("{} nicht vorhanden", k.display())),
        }
    }

    Err(format!(
        "Kein einsatzbereites Python gefunden.\n  {}\n\n\
         Einmalig einrichten (rund 2 GB, dauert einige Minuten):\n\
         \x20   cd INTEGER_LLM/calibrate\n\
         \x20   python3 -m venv .venv\n\
         \x20   .venv/bin/pip install -r requirements.txt      (Windows: .venv\\Scripts\\pip)\n\n\
         Danach findet der Client sie von selbst.",
        gesehen.join("\n  ")
    ))
}

/// Ungefähre Downloadgröße je Modell, für die Rückfrage vor dem Zugriff.
///
/// Aus dem Katalog, aus demselben Grund wie [`hf_id`]. Fehlt der Eintrag,
/// wird das ausdrücklich gesagt: Eine erfundene Zahl vor einem Download,
/// der Stunden dauern kann, ist schlechter als ein Achselzucken.
pub fn download_groesse(repo: &Path, modell: &str) -> String {
    katalogeintrag(repo, modell)
        .map(|k| k.gewichte_anzeige)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unbekannte Größe".to_string())
}

/// Lädt die Gewichte von Hugging Face in `INTEGER_LLM/models/<HF-Name>`.
///
/// Über `huggingface_hub.snapshot_download` aus der Kalibrierungs-Umgebung:
/// Das Paket ist als Abhängigkeit von `transformers` ohnehin vorhanden, es
/// beherrscht geteilte Safetensors und setzt einen abgebrochenen Download
/// fort. Ein eigener HTTPS-Pfad im Client wäre eine zweite, schlechtere
/// Umsetzung derselben Sache.
pub fn gewichte_holen(repo: &Path, modell: &str, meldung: &mut dyn FnMut(String)) -> Result<(), String> {
    let py = python_finden(repo)?;

    // **Herkunft und Revision kommen aus dem Katalog** (2026-08-22). Hier
    // stand `repo_id='Qwen/{hf}'`, also die Annahme, jedes Modell dieses
    // Projekts komme von Qwen, und **ohne Revision**, also von dem, was
    // gerade auf `main` liegt. Beides widerspricht `models/README.md`:
    // Dort steht, dass jede Variante eine fixierte Revision braucht, ohne
    // die der Lauf nicht reproduzierbar ist. Ein Modell, das sich
    // zwischen zwei Teilnehmern ändert, erzeugt genau den Befund, gegen
    // den dieses Werkzeug gebaut ist.
    let Some(k) = katalogeintrag(repo, modell) else {
        return Err(format!(
            "{modell} steht nicht in INTEGER_LLM/models/KATALOG.json.\n\
             Ohne Katalogeintrag ist weder bekannt, woher die Gewichte kommen,\n\
             noch welche Revision gilt. Beides zu raten wäre schlimmer als\n\
             der Abbruch: Ein Teilnehmer bekäme möglicherweise ein anderes\n\
             Modell als der nächste, und der Vergleich meldete eine\n\
             Abweichung, die keine ist.\n\
             \n\
             Eintrag ergänzen und danach `python tools/modelle_liste.py` laufen lassen."
        ));
    };
    if k.hf_repo.is_empty() || k.hf_revision.is_empty() {
        return Err(format!(
            "Der Katalogeintrag für {modell} ist unvollständig: \
             hf_repo={:?}, hf_revision={:?}. Beides ist Pflicht.",
            k.hf_repo, k.hf_revision
        ));
    }

    let ziel = repo.join("INTEGER_LLM/models").join(&k.hf_verzeichnis);
    meldung(format!(
        "Lade {} (Revision {}) nach {} …",
        k.hf_repo,
        &k.hf_revision[..k.hf_revision.len().min(12)],
        ziel.display()
    ));

    // **Pfad und Kennungen als Argumente, nicht als Quelltext.** Die
    // frühere Fassung schrieb den Zielpfad als `r'{ziel}'` in das Skript.
    // Auf einem Windows-Rechner, dessen Benutzername ein Apostroph
    // enthält, etwa `C:\\Users\\O'Brien\\…`, war das erzeugte Python
    // schlicht syntaktisch falsch, und der Teilnehmer bekam als Antwort
    // einen `SyntaxError` statt eines Downloads. Nachgestellt und vor der
    // Behebung reproduziert.
    //
    // Über `sys.argv` gibt es diese Klasse von Fehlern nicht: Das
    // Betriebssystem reicht die Zeichenkette durch, ohne dass sie je
    // Quelltext wird.
    //
    // `LICENSE*` gehört in die Muster, obwohl die Datei nichts rechnet.
    // Bis 2026-08-23 stand sie nicht drin, und weil eine Lizenzdatei keine
    // Endung trägt, kam sie nie an. `ETHICS/Manifest.md` berief sich für
    // G7 („das Basismodell muss frei nachnutzbar sein") ausdrücklich auf
    // `INTEGER_LLM/models/Qwen2.5-0.5B/LICENSE`, eine Datei, die auf
    // keiner Maschine existierte, die das Modell über diesen Weg geholt
    // hat. Dieselbe Klasse wie Fund 27: eine schriftliche Zusage ohne
    // Deckung. Apache 2.0 §4(a) verlangt zudem, jedem Empfänger einer
    // Bearbeitung eine Kopie der Lizenz mitzugeben; das geht nur, wenn sie
    // überhaupt da ist.
    const SKRIPT: &str = "import sys\n\
         from huggingface_hub import snapshot_download\n\
         snapshot_download(repo_id=sys.argv[1], revision=sys.argv[2],\n\
         \x20   local_dir=sys.argv[3],\n\
         \x20   allow_patterns=['*.json','*.safetensors','*.txt','LICENSE*'])\n";

    let ziel_str = ziel.to_string_lossy().into_owned();
    lauf(
        &py,
        &["-c", SKRIPT, &k.hf_repo, &k.hf_revision, &ziel_str],
        repo,
        meldung,
    )
}

/// Baut die Artefakte. Nutzt das versionierte Skalenpaket automatisch:
/// deshalb dauert das Sekunden statt Minuten und ist plattformübergreifend
/// bitgleich (siehe `INTEGER_LLM/scale_packs/README.md`).
pub fn artefakte_bauen(repo: &Path, modell: &str, meldung: &mut dyn FnMut(String)) -> Result<(), String> {
    let py = python_finden(repo)?;
    meldung(format!("Baue Artefakte für {modell} (Skalenpaket wird verwendet) …"));
    // Am Kindprozess statt global: `std::env::set_var` verändert den
    // eigenen Prozess und ist bei mehreren Threads nicht sauber definiert.
    lauf_mit(&py, &["-m", "calibrate.src.main"], &repo.join("INTEGER_LLM"),
             &[("INTEGER_LLM_MODEL", modell)], meldung)
}

fn lauf(
    programm: &Path,
    args: &[&str],
    cwd: &Path,
    meldung: &mut dyn FnMut(String),
) -> Result<(), String> {
    lauf_mit(programm, args, cwd, &[], meldung)
}

fn lauf_mit(
    programm: &Path,
    args: &[&str],
    cwd: &Path,
    umgebung: &[(&str, &str)],
    meldung: &mut dyn FnMut(String),
) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut befehl = Command::new(programm);
    for (k, v) in umgebung {
        befehl.env(k, v);
    }
    let mut kind = befehl
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} nicht startbar: {}", programm.display(), e))?;

    // Ausgabe durchreichen und dabei die verstrichene Zeit mitführen. Ein
    // Download über mehrere Gigabyte oder ein erster Cargo-Bau dauert
    // Minuten; ohne sichtbaren Fortschritt ist ein laufender Vorgang von
    // einem hängenden nicht zu unterscheiden. Genau dieselbe Begründung
    // wie beim Fortschrittsbalken der Diagnoseskripte.
    if let Some(out) = kind.stdout.take() {
        use std::io::{BufRead, BufReader, Write};
        let start = std::time::Instant::now();
        let mut zeilen = 0usize;
        let mut fehler = std::io::stderr();
        for zeile in BufReader::new(out).lines().map_while(Result::ok) {
            zeilen += 1;
            if !zeile.trim().is_empty() {
                meldung(format!("  {}", zeile));
            }
            // Der Takt geht auf stderr, damit er das Protokoll nicht füllt.
            let s = start.elapsed().as_secs();
            let _ = write!(fehler, "\r  … {} Schritte, {:02}:{:02} vergangen   ", zeilen, s / 60, s % 60);
            let _ = fehler.flush();
        }
        let s = start.elapsed().as_secs();
        let _ = write!(fehler, "\r{:60}\r", "");
        let _ = fehler.flush();
        meldung(format!("  fertig nach {:02}:{:02}", s / 60, s % 60));
    }
    let status = kind.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Abbruch mit {}", status))
    }
}

/// Ermittelt das zu verwendende Artefaktverzeichnis.
///
/// Der Ablauf folgt dem, was ein Nutzer erwartet, der den Client zum
/// ersten Mal startet:
///
/// 1. **Nichts gefunden** → Modell auswählen, Gewichte von Hugging Face
///    holen, Artefakte bauen, Digest prüfen.
/// 2. **Genau eines gefunden** → nehmen, aber sagen, ob der Digest passt.
/// 3. **Mehrere gefunden** → auswählen lassen.
///
/// `antwort` liefert Nutzereingaben; ist es `None`, läuft der Client
/// nicht-interaktiv. Dann wird **nicht** heruntergeladen: Ein
/// mehrstündiger Mehr-Gigabyte-Zugriff gehört nicht in einen Skriptlauf,
/// der ihn nicht angefordert hat. Stattdessen erscheint die Anleitung.
/// Ein Modell in der Auswahlliste, mit seinem Zustand auf dieser Maschine.
enum Eintrag {
    /// Artefakte liegen vor.
    Da(Gefunden),
    /// Im Register, aber hier nicht gebaut. Kann beschafft werden.
    Fehlt(Bekannt),
}

impl Eintrag {
    fn name(&self) -> &str {
        match self {
            Eintrag::Da(g) => &g.name,
            Eintrag::Fehlt(b) => &b.name,
        }
    }

    fn titel(&self) -> String {
        match self {
            Eintrag::Da(g) if g.im_register => format!("{}, liegt bereit", g.name),
            Eintrag::Da(g) => format!("{}, liegt bereit, nicht im Register", g.name),
            Eintrag::Fehlt(b) => format!("{}, nicht vorhanden", b.name),
        }
    }

    /// Die Zeile unter dem Titel: was das Modell ist und was seine Wahl
    /// kostet.
    ///
    /// **Aus dem kuratierten Katalog** (`models/KATALOG.json`), soweit er
    /// etwas dazu sagt. Vorher stand hier nur die Downloadgröße, und die
    /// beantwortet nicht die Frage, die vor der Wahl steht: Wofür ist
    /// dieses Modell da, woher kommt es, unter welcher Lizenz, und ist
    /// es schon einmal gemessen worden?
    fn hinweis(&self, repo: &Path) -> String {
        let k = katalogeintrag(repo, self.name());
        let herkunft = k
            .as_ref()
            .filter(|k| !k.hf_repo.is_empty())
            .map(|k| format!("{} · {} · {}", k.parameter, k.hf_repo, k.lizenz))
            .unwrap_or_else(|| "nicht im Katalog, Herkunft unbekannt".to_string());

        let zweite = match self {
            Eintrag::Da(g) if g.im_register => "Digest wird nach der Wahl geprüft.".to_string(),
            Eintrag::Da(_) => "Digest nicht prüfbar, für Vergleichsläufe ungeeignet.".to_string(),
            Eintrag::Fehlt(b) => format!(
                "Download {} von Hugging Face, Bau danach in Sekunden.",
                download_groesse(repo, &b.name)
            ),
        };

        match k.as_ref().filter(|k| !k.bemerkung.is_empty()) {
            Some(k) => format!("{herkunft}\n{zweite}\n{}", k.bemerkung),
            None => format!("{herkunft}\n{zweite}"),
        }
    }
}

/// Baut die Auswahlliste aus Register und tatsächlich Vorhandenem.
///
/// Getrennt vom Beschaffen, damit die Regel ohne Dateisystem prüfbar ist:
/// Sie ist der eigentliche Inhalt der Behebung, und ein Test darüber soll
/// keine Artefakte auf der Platte brauchen.
fn liste(bekannt: &[Bekannt], gefunden: &[Gefunden]) -> Vec<Eintrag> {
    let mut eintraege: Vec<Eintrag> = Vec::with_capacity(bekannt.len() + gefunden.len());
    for b in bekannt {
        match gefunden.iter().find(|g| g.name == b.name) {
            Some(g) => eintraege.push(Eintrag::Da(g.clone())),
            None => eintraege.push(Eintrag::Fehlt(b.clone())),
        }
    }
    // Was hier liegt, aber nirgends verzeichnet ist.
    for g in gefunden {
        if !bekannt.iter().any(|b| b.name == g.name) {
            eintraege.push(Eintrag::Da(g.clone()));
        }
    }
    eintraege
}

/// Stellt **alle** bekannten Modelle zur Wahl, vorhandene wie fehlende.
///
/// **Warum eine Liste und nicht zwei Wege.** Bis v0.6.0 gab es zwei
/// getrennte Fälle: Lag ein Artefakt vor, wurde daraus gewählt; lag keines
/// vor, wurde aus dem Register gewählt und beschafft. Wer 0,5B hatte und
/// 7B wollte, fand deshalb **keinen Weg dorthin**: Die Beschaffung stand
/// nur hinter dem Fall „nichts vorhanden", und der trat nie wieder ein.
/// Besonders bitter nach dem Freigeben von Plattenplatz, denn genau dann
/// will jemand ein Modell zurückholen, das er eben gelöscht hat.
///
/// Der Zustand eines Modells ist eine **Eigenschaft des Eintrags**, kein
/// eigener Programmzweig. Die Liste führt deshalb jedes bekannte Modell und
/// schreibt daneben, ob es hier liegt oder erst geholt werden muss.
///
/// Artefakte, die auf der Platte liegen, aber nicht im Register stehen,
/// kommen ans Ende: Sie sind benutzbar, aber ihr Digest ist nicht prüfbar,
/// und das muss vor einem Vergleichslauf dastehen.
pub fn beschaffen(
    repo: &Path,
    antwort: &mut Rueckfrage<'_>,
    meldung: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let bekannt = register(repo)?;
    let gefunden = suchen(repo);

    let eintraege = liste(&bekannt, &gefunden);
    if eintraege.is_empty() {
        return Err("Weder Artefakte noch ein Register gefunden.".to_string());
    }

    // Nicht-interaktiv wird nichts geholt: Ein Download von mehreren
    // Gigabyte gehört nicht in einen Lauf, der niemanden fragen kann.
    let Some(_) = antwort.as_mut() else {
        let da = eintraege.iter().find_map(|e| match e {
            Eintrag::Da(g) if g.im_register => Some(g),
            _ => None,
        });
        return match da.or_else(|| {
            eintraege.iter().find_map(|e| match e {
                Eintrag::Da(g) => Some(g),
                _ => None,
            })
        }) {
            Some(g) => {
                meldung(format!("Nicht-interaktiv: verwende {}", g.name));
                Ok(g.pfad.clone())
            }
            None => Err(format!(
                "Nicht-interaktiv, deshalb wird nichts heruntergeladen.\n{}",
                bauanleitung(repo, eintraege[0].name())
            )),
        };
    };

    let punkte: Vec<crate::auswahl::Punkt> = eintraege
        .iter()
        .enumerate()
        .map(|(i, e)| crate::auswahl::Punkt::neu(kuerzel(i), &e.titel(), &e.hinweis(repo)))
        .collect();
    let Some(wahl) = crate::auswahl::waehlen("Welches Modell?", &punkte)
        .and_then(|t| punkte.iter().position(|p| p.taste == t))
    else {
        return Err("Kein Modell gewählt.".to_string());
    };

    match &eintraege[wahl] {
        Eintrag::Da(g) => {
            meldung(format!("Gewählt: {}", g.name));
            nach_auswahl_pruefen(repo, g, meldung);
            Ok(g.pfad.clone())
        }
        Eintrag::Fehlt(b) => beschaffen_fuer(repo, &b.name, antwort, meldung),
    }
}


/// Prüft den Digest **eines** Artefakts und meldet das Ergebnis.
/// Wird erst nach der Auswahl aufgerufen: siehe Begründung in `suchen`.
fn nach_auswahl_pruefen(repo: &Path, g: &Gefunden, meldung: &mut dyn FnMut(String)) {
    if !g.im_register {
        meldung(format!("{} steht nicht im Register. Digest nicht prüfbar.", g.name));
        return;
    }
    meldung(format!("Prüfe Digest von {} …", g.name));
    let Ok(bekannt) = register(repo) else { return };
    let Some(b) = bekannt.iter().find(|b| b.name == g.name) else { return };
    match artefakt_digest(&g.pfad) {
        Ok((ist, _)) if ist == b.digest => meldung(format!("Digest stimmt: {}", &ist[..16])),
        Ok((ist, _)) => {
            meldung(format!("DIGEST WEICHT AB: hier {}, veröffentlicht {}", &ist[..16], &b.digest[..16]));
            meldung("Das ist KEIN Hardware-Befund: Hier liegt ein anderes Modell.".to_string());
        }
        Err(e) => meldung(format!("Digest nicht berechenbar: {}", e)),
    }
}

/// Sucht von `start` aufwärts das Repository-Wurzelverzeichnis.
///
/// Erkennungsmerkmal ist `INTEGER_LLM/scale_packs`; damit funktioniert der
/// Client aus jedem Unterverzeichnis heraus und findet auch dann noch die
/// richtige Ablage, wenn er von woanders aufgerufen wird.
pub fn repo_wurzel(start: PathBuf) -> PathBuf {
    let ausweich = start.clone();
    aufwaerts_suchen(start).unwrap_or(ausweich)
}

/// Wie [`repo_wurzel`], aber `None`, wenn nichts gefunden wurde.
///
/// Gebraucht, wo mehrere Startpunkte nacheinander versucht werden: Ein
/// Rückfallwert je Versuch verdeckte, dass der Versuch fehlschlug.
pub fn aufwaerts_suchen(start: PathBuf) -> Option<PathBuf> {
    let mut p = start;
    loop {
        if p.join("INTEGER_LLM/scale_packs").is_dir() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

/// Die Repository-Wurzel, zur **Laufzeit** bestimmt.
///
/// **Fund (2026-08-21):** Zwei Stellen im Client bestimmten sie über
/// `env!("CARGO_MANIFEST_DIR")`, also über den Pfad, der beim
/// **Übersetzen** galt. Das trägt genau so lange, wie das Repository dort
/// liegt, wo es gebaut wurde. Es trägt nicht mehr, sobald jemand es
/// verschiebt oder umbenennt, und es trägt erst recht nicht, wenn ein
/// gebautes Binary an eine andere Maschine weitergegeben wird. Genau das
/// sieht der Shell-Starter ausdrücklich vor („cargo nicht gefunden,
/// benutze das vorhandene Binary"), es ist also kein erfundener Fall.
///
/// Der Fehler wäre dabei besonders unangenehm gewesen: Der Client hätte
/// „Artefaktverzeichnis fehlt" gemeldet, während die Artefakte danebenlagen.
///
/// Gesucht wird in dieser Reihenfolge:
/// 1. aufwärts vom **Arbeitsverzeichnis** (dorthin wechseln die Starter),
/// 2. aufwärts vom **Ort des Programms** (wenn es von woanders gestartet wurde),
/// 3. der Übersetzungspfad als letzter Ausweg.
pub fn wurzel_zur_laufzeit(uebersetzungspfad: &Path) -> PathBuf {
    if let Some(p) = std::env::current_dir().ok().and_then(aufwaerts_suchen) {
        return p;
    }
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .and_then(aufwaerts_suchen)
    {
        return p;
    }
    uebersetzungspfad.to_path_buf()
}

/// Stellt sicher, dass die Artefakte **eines bestimmten** Modells vorliegen.
///
/// Wird benutzt, wenn ein Testplan das Modell vorgibt: Dann gibt es nichts
/// auszuwählen, nur zu beschaffen. Fehlt es, wird gefragt, ob geladen und
/// gebaut werden soll; ohne Rückfragekanal wird nichts geladen.
/// Der Pfad zu einem **bereits gebauten** Artefakt, ohne jede Rückfrage.
///
/// Getrennt von [`beschaffen_fuer`], weil die beiden verschiedene Fragen
/// beantworten: Dieses hier sagt „liegt es da?", jenes „beschaffe es".
/// Wer nur wissen will, ob weitergearbeitet werden kann, darf dabei
/// nichts herunterladen und nichts bauen.
///
/// `None` auch bei abweichendem Digest: Ein Artefakt, das nicht zum
/// Register passt, ist für einen Vergleichslauf schlimmer als keines, denn
/// sein Ergebnis sähe wie ein Hardware-Befund aus.
pub fn vorhandenes(repo: &Path, modell: &str) -> Option<PathBuf> {
    let bekannt = register(repo).ok()?;
    let b = bekannt.iter().find(|b| b.name == modell)?;
    match pruefen(repo, b) {
        Zustand::Bereit { pfad } => Some(pfad),
        _ => None,
    }
}

pub fn beschaffen_fuer(
    repo: &Path,
    modell: &str,
    antwort: &mut Rueckfrage<'_>,
    meldung: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let bekannt = register(repo)?;
    let b = bekannt
        .iter()
        .find(|b| b.name == modell)
        .ok_or_else(|| format!("{modell} steht nicht in scale_packs/REGISTER.json"))?;

    match pruefen(repo, b) {
        Zustand::Bereit { pfad } => {
            meldung(format!("{modell}: Digest stimmt ({})", &b.digest[..16]));
            return Ok(pfad);
        }
        Zustand::Abweichend { ist, soll, .. } => {
            return Err(format!(
                "{modell}: Digest weicht ab.\n  hier:           {ist}\n  \
                 veröffentlicht: {soll}\nDas ist KEIN Hardware-Befund. Ein \
                 Vergleichslauf mit diesem Artefakt hätte keine Aussage."
            ));
        }
        Zustand::Fehlt => {}
    }

    meldung(format!("{modell}: keine Artefakte auf dieser Maschine."));
    let Some(frage) = antwort.as_mut() else {
        return Err(format!(
            "Nicht-interaktiv, deshalb wird nichts heruntergeladen.\n{}",
            bauanleitung(repo, modell)
        ));
    };
    let t = frage(&format!(
        "{} von Hugging Face laden ({}) und Artefakte bauen? [J/n] ",
        hf_id(repo, modell),
        download_groesse(repo, modell)
    ))
    .unwrap_or_default()
    .trim()
    .to_lowercase();
    if !(t.is_empty() || t == "j" || t == "ja" || t == "y" || t == "yes") {
        return Err(format!("Abgebrochen.\n{}", bauanleitung(repo, modell)));
    }

    let gewichte = gewichte_verzeichnis(repo, modell);
    if gewichte.join("config.json").is_file() {
        meldung(format!("Gewichte liegen bereits in {}. Download entfällt.", gewichte.display()));
    } else {
        gewichte_holen(repo, modell, meldung)?;
    }
    artefakte_bauen(repo, modell, meldung)?;

    let bekannt = register(repo)?;
    let b = bekannt.iter().find(|b| b.name == modell).ok_or("Register unvollständig")?;
    match pruefen(repo, b) {
        Zustand::Bereit { pfad } => {
            meldung(format!("Fertig. Digest stimmt: {}", &b.digest[..16]));
            Ok(pfad)
        }
        Zustand::Abweichend { ist, soll, .. } => Err(format!(
            "Bau abgeschlossen, aber der Digest weicht ab.\n  hier:           {ist}\n  \
             veröffentlicht: {soll}\nDas ist KEIN Hardware-Befund."
        )),
        Zustand::Fehlt => Err("Bau lief durch, aber es liegen keine Artefakte vor.".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Freigeben: Plattenplatz zurückgewinnen
// ---------------------------------------------------------------------------

/// Was ein Modell auf dieser Maschine an Platz belegt.
///
/// Getrennt nach Artefakten und Gewichten, weil die beiden verschieden
/// teuer wiederzubeschaffen sind: Artefakte entstehen aus dem Skalenpaket
/// in Sekunden, die Gewichte kosten einen Download über Gigabyte. Wer
/// Platz braucht und den Test wiederholen will, löscht deshalb zuerst die
/// Artefakte und behält die Gewichte.
pub struct Belegung {
    pub modell: String,
    /// Artefaktverzeichnis und seine Größe in Bytes.
    pub artefakte: Option<(PathBuf, u64)>,
    /// Gewichtsverzeichnis und seine Größe in Bytes.
    pub gewichte: Option<(PathBuf, u64)>,
}

impl Belegung {
    /// Belegt dieses Modell überhaupt Platz?
    pub fn belegt(&self) -> bool {
        self.artefakte.is_some() || self.gewichte.is_some()
    }

    pub fn bytes(&self) -> u64 {
        self.artefakte.as_ref().map_or(0, |(_, b)| *b) + self.gewichte.as_ref().map_or(0, |(_, b)| *b)
    }
}

/// Größe eines Verzeichnisses in Bytes, rekursiv.
///
/// Symbolische Verknüpfungen werden **nicht** verfolgt: Sonst zählte ein
/// Verweis nach außen mit, und beim Löschen liefe die Rekursion in ein
/// Verzeichnis, das dem Nutzer gehört.
fn verzeichnisgroesse(p: &Path) -> u64 {
    let Ok(eintraege) = fs::read_dir(p) else {
        return 0;
    };
    let mut summe = 0;
    for e in eintraege.filter_map(|e| e.ok()) {
        let Ok(art) = e.file_type() else { continue };
        if art.is_symlink() {
            continue;
        } else if art.is_dir() {
            summe += verzeichnisgroesse(&e.path());
        } else if let Ok(m) = e.metadata() {
            summe += m.len();
        }
    }
    summe
}

/// Bytes als lesbare Größe.
pub fn groesse(bytes: u64) -> String {
    const EINHEITEN: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut wert = bytes as f64;
    let mut i = 0;
    while wert >= 1024.0 && i < EINHEITEN.len() - 1 {
        wert /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, EINHEITEN[0])
    } else {
        format!("{:.1} {}", wert, EINHEITEN[i]).replace('.', ",")
    }
}

/// Erhebt für jedes bekannte Modell, was auf dieser Maschine liegt.
pub fn belegung(repo: &Path) -> Vec<Belegung> {
    let mut namen: Vec<String> = register(repo)
        .unwrap_or_default()
        .into_iter()
        .map(|b| b.name)
        .collect();
    // Auch selbstgebaute Modelle, die nicht im Register stehen: sie
    // belegen denselben Platz.
    for g in suchen(repo) {
        if !namen.contains(&g.name) {
            namen.push(g.name);
        }
    }
    namen.sort();

    namen
        .into_iter()
        .map(|modell| {
            let a = repo.join("INTEGER_LLM/artifacts").join(&modell);
            let g = gewichte_verzeichnis(repo, &modell);
            Belegung {
                artefakte: a.is_dir().then(|| {
                    let b = verzeichnisgroesse(&a);
                    (a, b)
                }),
                gewichte: g.is_dir().then(|| {
                    let b = verzeichnisgroesse(&g);
                    (g, b)
                }),
                modell,
            }
        })
        .collect()
}

/// Prüft, ob ein Pfad überhaupt gelöscht werden darf.
///
/// **Der Wächter, nicht die Höflichkeit.** Gelöscht wird rekursiv; ein
/// falscher Pfad wäre nicht rückgängig zu machen. Erlaubt ist deshalb
/// ausschließlich ein **direktes Unterverzeichnis** von
/// `INTEGER_LLM/artifacts` oder `INTEGER_LLM/models`, nicht die beiden
/// Verzeichnisse selbst, nichts darüber, nichts daneben, und nichts, das
/// über `..` dorthin zeigt.
fn darf_geloescht_werden(repo: &Path, pfad: &Path) -> Result<(), String> {
    let echt = pfad
        .canonicalize()
        .map_err(|e| format!("{} nicht auflösbar: {}", pfad.display(), e))?;
    if !echt.is_dir() {
        return Err(format!("{} ist kein Verzeichnis", echt.display()));
    }
    for rel in ["INTEGER_LLM/artifacts", "INTEGER_LLM/models"] {
        let Ok(wurzel) = repo.join(rel).canonicalize() else {
            continue;
        };
        if echt.parent() == Some(wurzel.as_path()) {
            return Ok(());
        }
    }
    Err(format!(
        "{} liegt nicht unterhalb von INTEGER_LLM/artifacts oder \
         INTEGER_LLM/models: wird nicht gelöscht.",
        echt.display()
    ))
}

/// Löscht ein Artefakt- oder Gewichtsverzeichnis und meldet die
/// freigegebene Größe.
///
/// Der Aufrufer muss vorher gefragt haben; diese Funktion fragt nicht.
/// Sie prüft aber [`darf_geloescht_werden`] und verweigert alles, was
/// nicht eindeutig zu diesem Repository gehört.
pub fn freigeben(repo: &Path, pfad: &Path) -> Result<u64, String> {
    darf_geloescht_werden(repo, pfad)?;
    let bytes = verzeichnisgroesse(pfad);
    fs::remove_dir_all(pfad).map_err(|e| format!("{} nicht löschbar: {}", pfad.display(), e))?;
    Ok(bytes)
}

#[cfg(test)]
mod auswahl_tests {
    use super::*;

    fn gefunden(name: &str, im_register: bool) -> Gefunden {
        Gefunden {
            name: name.to_string(),
            pfad: PathBuf::from("/artefakte").join(name),
            im_register,
        }
    }

    fn bekannt(name: &str) -> Bekannt {
        Bekannt {
            name: name.to_string(),
            theta_v: "0.17.0".to_string(),
            digest: "c42bb8a8".repeat(8),
        }
    }

    /// Die Aufwärtssuche muss die Wurzel finden und außerhalb ein
    /// ehrliches `None` liefern. Ein Rückfallwert je Versuch verdeckte,
    /// dass der Versuch fehlschlug, und genau darauf bauen die drei
    /// Stufen von `wurzel_zur_laufzeit` auf.
    #[test]
    fn aufwaertssuche_meldet_misserfolg() {
        // Von hier aus muss die Wurzel dieses Repositoriums erreichbar
        // sein: Der Test läuft im Crate-Verzeichnis.
        let hier = std::env::current_dir().expect("Arbeitsverzeichnis");
        let wurzel = aufwaerts_suchen(hier.clone()).expect("Wurzel nicht gefunden");
        assert!(wurzel.join("INTEGER_LLM/scale_packs").is_dir());
        assert!(hier.starts_with(&wurzel), "die Wurzel liegt nicht über uns");

        // Außerhalb jedes Repositoriums gibt es nichts zu finden.
        assert!(aufwaerts_suchen(PathBuf::from("/")).is_none());
    }

    /// **Fund (2026-08-21):** Der Protokollordner und die Artefaktwurzel
    /// hingen am Übersetzungspfad. Ein verschobenes Repositorium oder ein
    /// weitergegebenes Binary hätte „Artefaktverzeichnis fehlt" gemeldet,
    /// während die Artefakte danebenlagen. Der Übersetzungspfad darf nur
    /// noch der letzte Ausweg sein.
    #[test]
    fn die_wurzel_kommt_zur_laufzeit_nicht_vom_uebersetzer() {
        let unsinn = PathBuf::from("/pfad/den/es/nicht/gibt");
        let gefunden = wurzel_zur_laufzeit(&unsinn);
        assert_ne!(gefunden, unsinn, "der Übersetzungspfad wurde übernommen");
        assert!(
            gefunden.join("INTEGER_LLM/scale_packs").is_dir(),
            "gefundene Wurzel trägt kein Skalenpaket: {}",
            gefunden.display()
        );
    }

    /// **Der Fund, der diese Liste erzwungen hat.** Bis v0.6.0 stand die
    /// Beschaffung nur hinter dem Fall „kein Artefakt vorhanden". Wer 0,5B
    /// hatte und 7B wollte, fand deshalb keinen Weg dorthin, und nach dem
    /// Freigeben von Plattenplatz war ein gelöschtes Modell nicht mehr
    /// zurückzuholen. Jede Auswahl muss **beides** enthalten.
    #[test]
    fn ein_vorhandenes_modell_verdeckt_die_fehlenden_nicht() {
        let register = [bekannt("qwen2.5-0.5b"), bekannt("qwen2.5-7b")];
        let auf_platte = [gefunden("qwen2.5-0.5b", true)];

        let eintraege = liste(&register, &auf_platte);
        let namen: Vec<&str> = eintraege.iter().map(|e| e.name()).collect();
        assert_eq!(namen, vec!["qwen2.5-0.5b", "qwen2.5-7b"]);

        assert!(
            matches!(eintraege[0], Eintrag::Da(_)),
            "das vorhandene Modell wird nicht als vorhanden geführt"
        );
        assert!(
            matches!(eintraege[1], Eintrag::Fehlt(_)),
            "das fehlende Modell steht nicht zur Beschaffung bereit"
        );
        assert!(
            eintraege[1].hinweis(&wurzel_zur_laufzeit(&PathBuf::from("."))).contains("Download"),
            "der Hinweis nennt den Download nicht: {}",
            eintraege[1].hinweis(&wurzel_zur_laufzeit(&PathBuf::from(".")))
        );
    }

    /// Artefakte, die hier liegen, aber in keinem Register stehen, gehören
    /// ans Ende und müssen als ungeprüft erkennbar sein: Ihr Digest ist
    /// nicht prüfbar, und ein Vergleichslauf damit hätte keine Aussage.
    #[test]
    fn fremde_artefakte_stehen_hinten_und_sind_gekennzeichnet() {
        let register = [bekannt("qwen2.5-0.5b")];
        let auf_platte = [gefunden("qwen2.5-0.5b", true), gefunden("eigenbau", false)];

        let eintraege = liste(&register, &auf_platte);
        assert_eq!(eintraege.len(), 2);
        assert_eq!(eintraege[1].name(), "eigenbau");
        assert!(
            eintraege[1].titel().contains("nicht im Register"),
            "ungeprüftes Artefakt nicht gekennzeichnet: {}",
            eintraege[1].titel()
        );
        assert!(eintraege[1].hinweis(&wurzel_zur_laufzeit(&PathBuf::from("."))).contains("nicht prüfbar"));
    }

    /// Ohne ein einziges Artefakt muss trotzdem jedes bekannte Modell zur
    /// Wahl stehen: Genau das ist die Lage nach einem frischen Klon.
    #[test]
    fn frischer_klon_stellt_alle_modelle_zur_wahl() {
        let register = [bekannt("qwen2.5-0.5b"), bekannt("qwen2.5-7b")];
        let eintraege = liste(&register, &[]);
        assert_eq!(eintraege.len(), 2);
        assert!(eintraege.iter().all(|e| matches!(e, Eintrag::Fehlt(_))));
    }
}

#[cfg(test)]
mod loeschen_tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-loeschen-{}", name));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Baut ein Repository-Gerüst mit einem Artefakt- und einem
    /// Gewichtsverzeichnis.
    fn geruest(dir: &Path) {
        let a = dir.join("INTEGER_LLM/artifacts/qwen2.5-0.5b");
        let g = dir.join("INTEGER_LLM/models/Qwen2.5-0.5B");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&g).unwrap();
        fs::write(a.join("weights_manifest.json"), vec![b'x'; 2048]).unwrap();
        fs::write(g.join("model.safetensors"), vec![b'y'; 4096]).unwrap();
    }

    #[test]
    fn belegung_findet_artefakte_und_gewichte() {
        let dir = tempdir("belegung");
        geruest(&dir);
        let b = belegung(&dir);
        let eintrag = b
            .iter()
            .find(|b| b.modell == "qwen2.5-0.5b")
            .expect("Modell gefunden");
        assert_eq!(eintrag.artefakte.as_ref().expect("Artefakte").1, 2048);
        assert_eq!(eintrag.gewichte.as_ref().expect("Gewichte").1, 4096);
        assert_eq!(eintrag.bytes(), 6144);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn freigeben_loescht_und_meldet_die_groesse() {
        let dir = tempdir("freigeben");
        geruest(&dir);
        let ziel = dir.join("INTEGER_LLM/artifacts/qwen2.5-0.5b");
        assert_eq!(freigeben(&dir, &ziel).expect("gelöscht"), 2048);
        assert!(!ziel.exists());
        // Die Gewichte bleiben unangetastet: sie sind teurer zu holen.
        assert!(dir.join("INTEGER_LLM/models/Qwen2.5-0.5B").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Wächter ist der Kern dieses Codes: Gelöscht wird rekursiv, und
    /// ein falscher Pfad wäre nicht rückgängig zu machen.
    #[test]
    fn nur_modellverzeichnisse_duerfen_geloescht_werden() {
        let dir = tempdir("waechter");
        geruest(&dir);
        let verboten = [
            dir.join("INTEGER_LLM/artifacts"),
            dir.join("INTEGER_LLM/models"),
            dir.join("INTEGER_LLM"),
            dir.clone(),
            dir.join("INTEGER_LLM/artifacts/qwen2.5-0.5b/.."),
            dir.join("INTEGER_LLM/artifacts/../../"),
        ];
        for p in verboten {
            assert!(
                freigeben(&dir, &p).is_err(),
                "{} hätte nicht gelöscht werden dürfen",
                p.display()
            );
        }
        // Nichts davon darf etwas angerichtet haben.
        assert!(dir.join("INTEGER_LLM/artifacts/qwen2.5-0.5b").is_dir());
        assert!(dir.join("INTEGER_LLM/models/Qwen2.5-0.5B").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ein_verschwundenes_verzeichnis_ist_ein_fehler_kein_absturz() {
        let dir = tempdir("weg");
        geruest(&dir);
        assert!(freigeben(&dir, &dir.join("INTEGER_LLM/artifacts/gibtsnicht")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn groessen_sind_lesbar() {
        assert_eq!(groesse(512), "512 B");
        assert_eq!(groesse(2048), "2,0 KB");
        assert_eq!(groesse(8_100_000_000), "7,5 GB");
    }

    /// **Negativtest Artefaktwechsel** (Fahrplan TESTCLIENT, „Tests, die
    /// für gehärtete Infrastruktur bestanden werden müssen").
    ///
    /// Ein verändertes Artefakt muss zu einem anderen Ankerdigest führen.
    /// Ohne diese Eigenschaft prüft der ganze Vergleich nichts: Zwei
    /// Maschinen mit verschiedenen Modellen bekämen denselben
    /// Modellstand bescheinigt, und eine daraus folgende Abweichung
    /// sähe wie ein Hardware-Befund aus. Genau diese Verwechslung
    /// abzufangen, ist der Zweck des Digests.
    #[test]
    fn ein_veraendertes_artefakt_ergibt_einen_anderen_digest() {
        let dir = std::env::temp_dir().join("myl-testclient-anker-negativ");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("Verzeichnis");
        for name in ANKER {
            fs::write(dir.join(name), format!("{{\"probe\": \"{name}\"}}")).expect("Datei");
        }

        let (vorher, anzahl) = artefakt_digest(&dir).expect("Digest");
        assert_eq!(anzahl, ANKER.len());

        // Jede einzelne Ankerdatei muss durchschlagen: Eine, die nicht
        // eingeht, wäre eine Lücke, durch die genau ein Unterschied
        // unsichtbar bliebe.
        for name in ANKER {
            let alt = fs::read_to_string(dir.join(name)).expect("lesbar");
            fs::write(dir.join(name), format!("{alt} ")).expect("schreibbar");
            let (nachher, _) = artefakt_digest(&dir).expect("Digest");
            assert_ne!(
                vorher, nachher,
                "ein geändertes {name} ändert den Ankerdigest nicht"
            );
            fs::write(dir.join(name), &alt).expect("zurückschreibbar");
        }

        // Zurückgesetzt muss wieder derselbe Wert herauskommen, sonst
        // hinge der Digest an etwas anderem als am Inhalt.
        let (wieder, _) = artefakt_digest(&dir).expect("Digest");
        assert_eq!(vorher, wieder, "der Digest ist nicht reproduzierbar");

        // Eine fehlende Ankerdatei ist ein Fehler, kein anderer Digest:
        // Ein Digest über ein unvollständiges Artefakt wäre eine Zahl
        // ohne Bedeutung, und der Vergleich würde sie ernst nehmen.
        fs::remove_file(dir.join(ANKER[0])).expect("löschbar");
        assert!(
            artefakt_digest(&dir).is_err(),
            "fehlende Ankerdatei ergibt stillschweigend einen Digest"
        );

        let _ = fs::remove_dir_all(&dir);
    }


    /// Der Katalog wird von Hand gepflegt und vom Client gelesen. Beides
    /// muss zusammenpassen, sonst ist er Zierat.
    #[test]
    fn der_katalog_liefert_die_kuratierten_angaben() {
        let wurzel = wurzel_zur_laufzeit(&PathBuf::from("."));
        let eintraege = katalog(&wurzel).expect("KATALOG.json lesbar");
        assert!(eintraege.len() >= 2, "zu wenige Einträge: {eintraege:?}");

        let klein = eintraege
            .iter()
            .find(|k| k.name == "qwen2.5-0.5b")
            .expect("qwen2.5-0.5b fehlt im Katalog");
        assert_eq!(klein.hf_repo, "Qwen/Qwen2.5-0.5B");
        assert_eq!(klein.hf_verzeichnis, "Qwen2.5-0.5B");
        assert_eq!(klein.lizenz, "Apache-2.0");
        assert_eq!(klein.hf_revision.len(), 40, "keine volle Git-Revision");

        // Jeder Eintrag muss die Angaben tragen, für die es ihn gibt.
        for k in &eintraege {
            for (feld, wert) in [
                ("hf_repo", &k.hf_repo),
                ("hf_verzeichnis", &k.hf_verzeichnis),
                ("hf_revision", &k.hf_revision),
                ("lizenz", &k.lizenz),
                ("status", &k.status),
                ("gewichte_anzeige", &k.gewichte_anzeige),
            ] {
                assert!(!wert.is_empty(), "{}: {} fehlt", k.name, feld);
            }
        }
    }

    /// **Katalog und Register müssen dieselben Modelle kennen.**
    ///
    /// Sie sind bewusst getrennt (kuratiert gegen gemessen), aber ein
    /// Modell, das nur in einem von beiden steht, ist ein halber
    /// Eintrag: Ohne Registereintrag lässt sich sein Digest nicht
    /// prüfen, ohne Katalogeintrag weiß der Client nicht, woher die
    /// Gewichte kommen, und lud vor dem 2026-08-22 stillschweigend die
    /// von Qwen2.5-0,5B.
    #[test]
    fn katalog_und_register_kennen_dieselben_modelle() {
        let wurzel = wurzel_zur_laufzeit(&PathBuf::from("."));
        let mut aus_katalog: Vec<String> =
            katalog(&wurzel).expect("Katalog").into_iter().map(|k| k.name).collect();
        let mut aus_register: Vec<String> =
            register(&wurzel).expect("Register").into_iter().map(|b| b.name).collect();
        aus_katalog.sort();
        aus_register.sort();
        assert_eq!(
            aus_katalog, aus_register,
            "Katalog und Register führen verschiedene Modelle"
        );
    }

    /// Kommentarblöcke im Katalog beginnen mit `_` und sind keine
    /// Modelle. Ohne diese Regel stünde „_status_bedeutung" im Menü.
    #[test]
    fn kommentarbloecke_sind_keine_modelle() {
        let text = r#"{
  "_hinweis": "erklaert etwas",
  "_status_bedeutung": {
    "verifiziert": "gebaut und gemessen"
  },
  "modell-a": {
    "anzeigename": "Modell A",
    "hf_repo": "Wer/Modell-A",
    "hf_verzeichnis": "Modell-A",
    "hf_revision": "abc",
    "lizenz": "Apache-2.0",
    "parameter": "1 Mrd.",
    "gewichte_anzeige": "rund 2 GB",
    "artefakt_anzeige": "1 GB",
    "status": "erprobt",
    "bemerkung": "kein Kommentarblock"
  }
}"#;
        let eintraege = katalog_lesen(text);
        assert_eq!(eintraege.len(), 1, "{eintraege:?}");
        assert_eq!(eintraege[0].name, "modell-a");
        assert_eq!(eintraege[0].anzeigename, "Modell A");
    }

    /// **Kein stiller Rückfall auf das kleine Modell.** Hier stand ein
    /// `match` mit `_ => "Qwen2.5-0.5B"`; ein drittes Modell hätte damit
    /// die falschen Gewichte geladen und wäre erst beim Bau
    /// aufgefallen, mit einer Fehlermeldung, die nach allem aussieht
    /// außer nach der Ursache.
    #[test]
    fn ein_unbekanntes_modell_bekommt_keine_fremden_gewichte() {
        let wurzel = wurzel_zur_laufzeit(&PathBuf::from("."));
        assert_eq!(hf_id(&wurzel, "qwen2.5-7b"), "Qwen2.5-7B");
        assert_eq!(
            hf_id(&wurzel, "gibt-es-nicht"),
            "gibt-es-nicht",
            "unbekanntes Modell bekommt fremde Gewichte"
        );
        assert!(
            download_groesse(&wurzel, "gibt-es-nicht").contains("unbekannt"),
            "erfundene Größenangabe vor einem Download"
        );
    }


    /// Fluchtsequenzen im Katalog müssen aufgelöst werden: Ein `\n` in
    /// einer Bemerkung stand vorher wörtlich im Menü, und ein
    /// Anführungszeichen hätte die Zeile zerlegt.
    #[test]
    fn fluchtsequenzen_werden_aufgeloest() {
        assert_eq!(entpacken("eine\\nzweite"), "eine\nzweite");
        assert_eq!(entpacken("er sagte \\\"hallo\\\""), "er sagte \"hallo\"");
        assert_eq!(entpacken("C:\\\\Pfad"), "C:\\Pfad");
        // Unbekanntes bleibt stehen, statt geraten zu werden.
        assert_eq!(entpacken("\\q"), "\\q");
        assert_eq!(entpacken("ohne alles"), "ohne alles");
    }


    /// **Der Fund vom Linux-Runner (2026-08-22).** Der Modellschlüssel
    /// (`qwen2.5-0.5b`) und der Verzeichnisname (`Qwen2.5-0.5B`)
    /// unterscheiden sich nur in der Groß- und Kleinschreibung. Auf einem
    /// Dateisystem, das die nicht unterscheidet (macOS, Windows), findet
    /// selbst ein falscher Name das Verzeichnis; auf Linux nicht. Ein
    /// Nutzer dort hätte in „Artefakte und Gewichte löschen" seine
    /// Gewichte nicht aufgelistet bekommen und geglaubt, sie seien weg.
    ///
    /// Der Test prüft die **Zeichenkette**, nicht den Zugriff, und greift
    /// deshalb auf jedem Dateisystem.
    #[test]
    fn die_gewichte_werden_auch_ohne_katalog_gefunden() {
        let dir = tempdir("gewichte-suche");
        let g = dir.join("INTEGER_LLM/models/Qwen2.5-0.5B");
        fs::create_dir_all(&g).unwrap();

        // Ohne Katalog im Gerüst: Die Suche muss über die Schreibweise
        // hinwegsehen und den echten Verzeichnisnamen zurückgeben.
        let gefunden = gewichte_verzeichnis(&dir, "qwen2.5-0.5b");
        assert_eq!(
            gefunden.file_name().unwrap().to_string_lossy(),
            "Qwen2.5-0.5B",
            "die Suche liefert den Modellschlüssel statt des Verzeichnisses"
        );
        assert!(gefunden.is_dir());

        // Ein Modell, zu dem nichts daliegt, bekommt keinen fremden Pfad
        // untergeschoben.
        let nichts = gewichte_verzeichnis(&dir, "gibt-es-nicht");
        assert_eq!(nichts.file_name().unwrap().to_string_lossy(), "gibt-es-nicht");
        assert!(!nichts.is_dir());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein Download ohne festgelegte Revision holt, was gerade auf `main`
    /// liegt. `models/README.md` verlangt eine fixierte Revision, ohne die
    /// der Lauf nicht reproduzierbar ist: Ein Modell, das sich zwischen
    /// zwei Teilnehmern ändert, erzeugt genau den Befund, gegen den dieses
    /// Werkzeug gebaut ist.
    #[test]
    fn jeder_katalogeintrag_traegt_herkunft_und_revision() {
        let wurzel = wurzel_zur_laufzeit(&PathBuf::from("."));
        for k in katalog(&wurzel).expect("Katalog") {
            assert!(
                k.hf_repo.contains('/'),
                "{}: hf_repo ist keine Hugging-Face-Kennung: {:?}",
                k.name,
                k.hf_repo
            );
            assert_eq!(
                k.hf_revision.len(),
                40,
                "{}: keine volle Git-Revision, sondern {:?}",
                k.name,
                k.hf_revision
            );
            assert!(
                k.hf_revision.chars().all(|c| c.is_ascii_hexdigit()),
                "{}: Revision ist kein Hexwert",
                k.name
            );
        }
    }

}
