//! Artefakte finden, bauen und **gegen den veröffentlichten Digest prüfen**.
//!
//! ## Warum die Prüfung der eigentliche Punkt ist
//!
//! Der Zweck dieses Clients ist der Nachweis, dass zwei verschiedene
//! Maschinen bitgleich rechnen. Das setzt voraus, dass beide Maschinen
//! **dasselbe Modell** rechnen — und genau das ist nicht selbstverständlich.
//!
//! Der Artefaktbau war bis 2026-08-20 nur auf derselben Maschine
//! reproduzierbar (Fund 32): Die Aktivierungsskalen entstanden aus einem
//! Gleitkomma-Durchlauf, und **3 von 314** Skaleneinträgen saßen innerhalb
//! von 0,01 % einer Zweierpotenz-Grenze — der knappste bei 0,003 %. Eine
//! andere BLAS-Version reicht, um einen davon umzuwerfen; ein gekippter
//! Shift ändert die Artefaktbytes, also das Modell.
//!
//! Ohne Digest-Prüfung sähe das im Ergebnis **wie eine gescheiterte
//! Hardware-Bitgleichheit aus** — der Testclient würde also genau das
//! Gegenteil dessen berichten, wofür es ihn gibt. Deshalb prüft er zuerst,
//! ob überhaupt dasselbe Modell vorliegt, und sagt bei Abweichung klar,
//! dass das Artefakt und nicht die Hardware das Problem ist.
//!
//! Seit dem Skalenpaket (`INTEGER_LLM/scale_packs/`) ist der Bau
//! plattformübergreifend bitgleich: Die Skalen und LUTs sind versioniert,
//! die verbleibende Gewichtsquantisierung ist `round(W · 2^shift)` und
//! damit exakt. Die Prüfung bleibt trotzdem — eine Zusicherung, die man
//! nicht nachrechnet, ist eine Hoffnung.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Ein Modell, für das ein Skalenpaket veröffentlicht ist.
pub struct Bekannt {
    pub name: String,
    pub theta_v: String,
    pub digest: String,
}

/// Zustand eines Modells auf dieser Maschine.
pub enum Zustand {
    /// Artefakte da und Digest stimmt — bereit für einen Vergleichslauf.
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
/// `tools/skalenpaket_bauen.py` sein — sonst prüft der Client gegen eine
/// andere Rechnung als die, die den veröffentlichten Wert erzeugt hat.
///
/// **Nicht über den Verzeichnisinhalt.** Diese Fassung gab es, und sie hat
/// am 2026-08-20 sofort falschen Alarm ausgelöst: Ein Synchronisations-
/// werkzeug hatte 432 inhaltsgleiche Kopien in den Artefaktordner gelegt
/// (`theta_v 2.json` und so fort). Der Lader ignoriert solche Dateien; sie
/// ändern das Modell nicht. Ein Anker, der bei belanglosen Streudateien
/// anschlägt, macht den echten Befund unglaubwürdig — und der echte
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

fn feld(zeile: &str, schluessel: &str) -> Option<String> {
    let praefix = format!("\"{}\": ", schluessel);
    let rest = zeile.trim().strip_prefix(&praefix)?;
    Some(rest.trim_end_matches(',').trim_matches('"').to_string())
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
pub fn bauanleitung(modell: &str) -> String {
    format!(
        "So entstehen die Artefakte für {m} (einmalig):\n\
         \n\
         1. Gewichte von Hugging Face holen — sie werden NICHT mitgeliefert:\n\
         \x20     huggingface-cli download Qwen/{hf} --local-dir INTEGER_LLM/models/{hf}\n\
         \n\
         2. Artefakte bauen. Das versionierte Skalenpaket unter\n\
         \x20  INTEGER_LLM/scale_packs/{m}/ wird automatisch verwendet:\n\
         \x20     INTEGER_LLM_MODEL={m} python -m calibrate.src.main\n\
         \n\
         Der Bau dauert Sekunden statt Minuten, weil die Aktivierungsstatistik\n\
         entfällt — und genau deshalb ist er auf jeder Maschine bitgleich.\n\
         Danach diesen Befehl erneut ausführen; der Digest wird geprüft.",
        m = modell,
        hf = hf_id(modell),
    )
}

fn hf_id(modell: &str) -> &'static str {
    match modell {
        "qwen2.5-7b" => "Qwen2.5-7B",
        _ => "Qwen2.5-0.5B",
    }
}

// ---------------------------------------------------------------------------
// Beschaffung: suchen, auswählen, notfalls herunterladen und bauen
// ---------------------------------------------------------------------------

/// Eingabekanal für Rückfragen: liefert die Antwort auf einen Prompt,
/// oder `None` bei Dateiende. `Option<&mut …>` als Ganzes ist `None`,
/// wenn der Client nicht-interaktiv läuft — dann wird nicht gefragt und
/// folglich auch nichts heruntergeladen.
pub type Rueckfrage<'a> = Option<&'a mut dyn FnMut(&str) -> Option<String>>;

/// Ein auf dieser Maschine gefundenes Artefaktverzeichnis.
pub struct Gefunden {
    pub name: String,
    pub pfad: PathBuf,
    /// Steht das Modell im Register? Wenn nicht, ist sein Digest nicht
    /// prüfbar — das muss vor einem Vergleichslauf gesagt werden.
    pub im_register: bool,
}

/// Durchsucht `INTEGER_LLM/artifacts/` nach vollständigen Artefakten.
///
/// Gefunden wird jedes Verzeichnis mit `weights_manifest.json` — auch
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
        // ist Verschwendung — und es lässt den Client beim Start minutenlang
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
/// „.venv fehlt" — womit der gesamte Download- und Baupfad auf jeder
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
pub fn download_groesse(modell: &str) -> &'static str {
    match modell {
        "qwen2.5-7b" => "rund 15 GB",
        _ => "rund 1 GB",
    }
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
    let hf = hf_id(modell);
    let ziel = repo.join("INTEGER_LLM/models").join(hf);
    meldung(format!("Lade {} nach {} …", hf, ziel.display()));

    let skript = format!(
        "from huggingface_hub import snapshot_download\n\
         snapshot_download(repo_id='Qwen/{hf}', local_dir=r'{ziel}',\n\
         \x20   allow_patterns=['*.json','*.safetensors','*.txt'])\n",
        hf = hf,
        ziel = ziel.display()
    );
    lauf(&py, &["-c", &skript], repo, meldung)
}

/// Baut die Artefakte. Nutzt das versionierte Skalenpaket automatisch —
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
pub fn beschaffen(
    repo: &Path,
    antwort: &mut Rueckfrage<'_>,
    meldung: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    let gefunden = suchen(repo);

    if gefunden.len() == 1 {
        let g = &gefunden[0];
        meldung(format!("Ein Artefakt gefunden: {}", befund(g)));
        nach_auswahl_pruefen(repo, g, meldung);
        return Ok(g.pfad.clone());
    }

    if gefunden.len() > 1 {
        meldung(format!("{} Artefakte auf dieser Maschine:", gefunden.len()));
        for (i, g) in gefunden.iter().enumerate() {
            meldung(format!("  [{}] {}", i + 1, befund(g)));
        }
        let Some(frage) = antwort.as_mut() else {
            // Nicht-interaktiv: das erste geprüfte nehmen, sonst das erste.
            let g = gefunden.iter().find(|g| g.im_register).unwrap_or(&gefunden[0]);
            meldung(format!("Nicht-interaktiv — verwende {}", g.name));
            return Ok(g.pfad.clone());
        };
        let eingabe = frage("Welches Artefakt? [1] ").unwrap_or_default();
        let wahl = eingabe.trim().parse::<usize>().unwrap_or(1).clamp(1, gefunden.len());
        let g = &gefunden[wahl - 1];
        meldung(format!("Gewählt: {}", g.name));
        nach_auswahl_pruefen(repo, g, meldung);
        return Ok(g.pfad.clone());
    }

    // Nichts gefunden.
    meldung("Keine Artefakte auf dieser Maschine.".to_string());
    let bekannt = register(repo)?;
    let Some(frage) = antwort.as_mut() else {
        return Err(format!(
            "Nicht-interaktiv, deshalb wird nichts heruntergeladen.\n{}",
            bauanleitung(&bekannt[0].name)
        ));
    };

    meldung("Verfügbare Modelle:".to_string());
    for (i, b) in bekannt.iter().enumerate() {
        meldung(format!(
            "  [{}] {} — Download {}, Bau danach in Sekunden",
            i + 1,
            b.name,
            download_groesse(&b.name)
        ));
    }
    let eingabe = frage("Welches Modell aufsetzen? [1] ").unwrap_or_default();
    let wahl = eingabe.trim().parse::<usize>().unwrap_or(1).clamp(1, bekannt.len());
    let modell = bekannt[wahl - 1].name.clone();

    // Rückfrage vor dem Netzzugriff: Der Download ist gross und geht an
    // einen fremden Dienst. Automatisch heisst nicht unangekuendigt.
    let bestaetigung = frage(&format!(
        "{} von Hugging Face laden ({}) und Artefakte bauen? [J/n] ",
        hf_id(&modell),
        download_groesse(&modell)
    ))
    .unwrap_or_default();
    let t = bestaetigung.trim().to_lowercase();
    if !(t.is_empty() || t == "j" || t == "ja" || t == "y" || t == "yes") {
        return Err(format!("Abgebrochen.\n{}", bauanleitung(&modell)));
    }

    let gewichte = repo.join("INTEGER_LLM/models").join(hf_id(&modell));
    if gewichte.join("config.json").is_file() {
        meldung(format!("Gewichte liegen bereits in {} — Download entfällt.", gewichte.display()));
    } else {
        gewichte_holen(repo, &modell, meldung)?;
    }
    artefakte_bauen(repo, &modell, meldung)?;

    let bekannt = register(repo)?;
    let b = bekannt
        .iter()
        .find(|b| b.name == modell)
        .ok_or_else(|| format!("{} steht nicht im Register", modell))?;
    match pruefen(repo, b) {
        Zustand::Bereit { pfad } => {
            meldung(format!("Fertig — Digest stimmt: {}", &b.digest[..16]));
            Ok(pfad)
        }
        Zustand::Abweichend { ist, soll, .. } => Err(format!(
            "Bau abgeschlossen, aber der Digest weicht ab.\n  hier:           {ist}\n  \
             veröffentlicht: {soll}\nDas ist KEIN Hardware-Befund — auf dieser Maschine \
             entstand ein anderes Artefakt als das veröffentlichte. Ein Vergleichslauf \
             damit hätte keine Aussage."
        )),
        Zustand::Fehlt => Err("Bau lief durch, aber es liegen keine Artefakte vor.".to_string()),
    }
}

fn befund(g: &Gefunden) -> String {
    if g.im_register {
        g.name.clone()
    } else {
        format!("{} — nicht im Register, Digest nicht prüfbar", g.name)
    }
}

/// Prüft den Digest **eines** Artefakts und meldet das Ergebnis.
/// Wird erst nach der Auswahl aufgerufen — siehe Begründung in `suchen`.
fn nach_auswahl_pruefen(repo: &Path, g: &Gefunden, meldung: &mut dyn FnMut(String)) {
    if !g.im_register {
        meldung(format!("{} steht nicht im Register — Digest nicht prüfbar.", g.name));
        return;
    }
    meldung(format!("Prüfe Digest von {} …", g.name));
    let Ok(bekannt) = register(repo) else { return };
    let Some(b) = bekannt.iter().find(|b| b.name == g.name) else { return };
    match artefakt_digest(&g.pfad) {
        Ok((ist, _)) if ist == b.digest => meldung(format!("Digest stimmt: {}", &ist[..16])),
        Ok((ist, _)) => {
            meldung(format!("DIGEST WEICHT AB — hier {}, veröffentlicht {}", &ist[..16], &b.digest[..16]));
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
    let mut p = start.clone();
    loop {
        if p.join("INTEGER_LLM/scale_packs").is_dir() {
            return p;
        }
        if !p.pop() {
            return start;
        }
    }
}

/// Stellt sicher, dass die Artefakte **eines bestimmten** Modells vorliegen.
///
/// Wird benutzt, wenn ein Testplan das Modell vorgibt: Dann gibt es nichts
/// auszuwählen, nur zu beschaffen. Fehlt es, wird gefragt, ob geladen und
/// gebaut werden soll; ohne Rückfragekanal wird nichts geladen.
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
            bauanleitung(modell)
        ));
    };
    let t = frage(&format!(
        "{} von Hugging Face laden ({}) und Artefakte bauen? [J/n] ",
        hf_id(modell),
        download_groesse(modell)
    ))
    .unwrap_or_default()
    .trim()
    .to_lowercase();
    if !(t.is_empty() || t == "j" || t == "ja" || t == "y" || t == "yes") {
        return Err(format!("Abgebrochen.\n{}", bauanleitung(modell)));
    }

    let gewichte = repo.join("INTEGER_LLM/models").join(hf_id(modell));
    if gewichte.join("config.json").is_file() {
        meldung(format!("Gewichte liegen bereits in {} — Download entfällt.", gewichte.display()));
    } else {
        gewichte_holen(repo, modell, meldung)?;
    }
    artefakte_bauen(repo, modell, meldung)?;

    let bekannt = register(repo)?;
    let b = bekannt.iter().find(|b| b.name == modell).ok_or("Register unvollständig")?;
    match pruefen(repo, b) {
        Zustand::Bereit { pfad } => {
            meldung(format!("Fertig — Digest stimmt: {}", &b.digest[..16]));
            Ok(pfad)
        }
        Zustand::Abweichend { ist, soll, .. } => Err(format!(
            "Bau abgeschlossen, aber der Digest weicht ab.\n  hier:           {ist}\n  \
             veröffentlicht: {soll}\nDas ist KEIN Hardware-Befund."
        )),
        Zustand::Fehlt => Err("Bau lief durch, aber es liegen keine Artefakte vor.".to_string()),
    }
}
