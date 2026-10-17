//! **Ein Modell holen und sein Artefakt bauen**, fuer Konsole und Fenster.
//!
//! # ⚑ Eine Stelle fuer beide Bedieninstrumente
//!
//! Bis zum 2026-10-16 konnte das nur das Fenster, und die Konsole meldete
//! „hier liegt kein gebautes Artefakt". Hier steht, **was** zu tun ist und
//! **wie viel Platz** es braucht; Fenster und Konsole fragen nur nach und
//! zeigen den Fortschritt. Getan wird es von den Skripten des
//! Repositoriums, denn dort steht der dokumentierte Weg genau einmal:
//!
//! | Schritt | Skript |
//! |---|---|
//! | Python-Umgebung | `INTEGER_LLM/scripts/python_einrichten.sh` |
//! | Gewichte holen | `INTEGER_LLM/scripts/fetch_model.sh` |
//! | int8-Artefakt | `INTEGER_LLM/scripts/build_artifacts.sh` |
//! | Experten in vier Bit | `INTEGER_LLM/scripts/build_vierbit.sh` |
//!
//! # ⛔️ Ohne Platz keine Freigabe
//!
//! Festlegung des Projektinhabers (2026-10-16): **immer ausdruecklich
//! bestaetigen lassen, wie viel Speicher waehrend des Vorgangs gebraucht
//! wird, und ohne diesen Platz keine Freigabe zum Download: erst Speicher
//! schaffen.** Deshalb rechnet [`Bauplan::bedarf`] die Spitze aus, also
//! alles, was waehrend des Baus zugleich auf der Platte liegt, und
//! [`Bauplan::platz_pruefen`] vergleicht sie mit dem freien Platz **und** mit
//! der Freigabe aus den Einstellungen. Reicht eines von beiden nicht, gibt
//! es keine Bestaetigung, sondern die Zahl, die fehlt. [`ausfuehren`]
//! prueft dasselbe noch einmal, unmittelbar vor dem ersten Schritt.
//!
//! # Woher die Zahlen kommen
//!
//! Aus `MODELS/llm/KATALOG.json` (`gewichte_bytes`, `artefakt_bytes`,
//! `zwischen_bytes`); fuer die Werkzeuge aus den Konstanten hier,
//! gemessen. Gezaehlt wird nur, was noch fehlt: Liegen die Gewichte schon
//! da, faellt der Download weg.

use std::path::{Path, PathBuf};

use crate::einstellungen::Einstellungen;

/// Wo die Programme der Kalibrier-Umgebung liegen, gegen die Wurzel. ⚑ Eine
/// Stelle: Hier sucht der Plan, ob sie da ist, und hier steht sie vorn im
/// `PATH` der Skripte. Zwei Fassungen liefen auseinander.
const UMGEBUNG_BIN: &str = "INTEGER_LLM/calibrate/.venv/bin";

/// Die Python-Umgebung mit den festen Fassungen, gemessen am 2026-10-16
/// (0,90 GB), aufgerundet.
pub const UMGEBUNG_BYTES: u64 = 1_000_000_000;

/// Die Rust-Werkzeuge im Release-Bau fuer den Vierbitbau
/// (`expertenmitschnitt`, `vierbit_packen` und was sie brauchen); gemessen
/// am kalten Klon, aufgerundet.
pub const WERKZEUG_BYTES: u64 = 3_000_000_000;

/// Ein Schritt des Baus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schritt {
    /// Die Kalibrier-Umgebung einrichten (Netz, rund 1 GB).
    Umgebung,
    /// Die Gewichte von Hugging Face holen.
    Holen { repo: String, revision: String, verzeichnis: String, bytes: u64 },
    /// Das int8-Artefakt kalibrieren.
    Int8 { schluessel: String, bytes: u64 },
    /// Die Experten eines int8-Gemischs in vier Bit bauen.
    Vierbit { quelle: String, ziel: String, zwischen: u64, bytes: u64, dauer: String },
}

impl Schritt {
    /// Eine Zeile fuer Menschen.
    pub fn beschreibung(&self) -> String {
        match self {
            Schritt::Umgebung => format!("Python-Umgebung einrichten (rund {} aus dem Netz)", gib(UMGEBUNG_BYTES)),
            Schritt::Holen { repo, bytes, .. } => format!("Gewichte holen: {repo}, {} aus dem Netz", gib(*bytes)),
            Schritt::Int8 { schluessel, bytes } => format!("int8-Artefakt {schluessel} bauen ({})", gib(*bytes)),
            Schritt::Vierbit { ziel, zwischen, bytes, dauer, .. } => {
                let mut s = format!("Experten in vier Bit bauen: {ziel} ({}, dazu {} Zwischenstaende)", gib(*bytes), gib(*zwischen));
                if !dauer.is_empty() {
                    s.push_str(&format!(", {dauer}"));
                }
                s
            }
        }
    }

    /// Der Name der Phase, wie Fenster und Konsole ihn zeigen.
    pub fn phase(&self) -> &'static str {
        match self {
            Schritt::Umgebung => "umgebung",
            Schritt::Holen { .. } => "holen",
            Schritt::Int8 { .. } => "kalibrieren",
            Schritt::Vierbit { .. } => "vierbit",
        }
    }
}

/// Wie viel Platz der Bau braucht, in Bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bedarf {
    /// Was aus dem Netz kommt.
    pub download: u64,
    /// Was nach dem Bau liegen bleibt: Umgebung, Werkzeuge, Gewichte,
    /// Artefakte.
    pub bleibt: u64,
    /// Was nur waehrend des Baus liegt und danach geloescht wird.
    pub zwischen: u64,
}

impl Bedarf {
    /// ⚑ **Die Spitze: alles zugleich.** Was bleibt, liegt am Ende ganz
    /// da, und die Zwischenstaende liegen kurz davor alle noch; die
    /// Summe ist also nicht zu vorsichtig, sondern genau der Moment vor
    /// dem Aufraeumen.
    pub fn spitze(&self) -> u64 {
        self.bleibt + self.zwischen
    }
}

/// Was der Bau eines Katalogeintrags auf dieser Maschine noch braucht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bauplan {
    pub schluessel: String,
    pub anzeigename: String,
    pub wurzel: PathBuf,
    /// Leer heisst: Das Artefakt liegt schon da.
    pub schritte: Vec<Schritt>,
    /// Die Mindestausstattung aus dem Katalog.
    pub hardware: String,
}

/// Was die Platte hergibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platzlage {
    /// Frei auf dem Datentraeger des Repositoriums; `None`, wenn nicht
    /// messbar.
    pub frei: Option<u64>,
    /// Was die Freigabe in den Einstellungen noch zulaesst; `None` ohne
    /// Freigabe.
    pub freigabe_rest: Option<u64>,
}

/// Bytes in GiB mit einer Nachkommastelle, fuer Menschen.
pub fn gib(bytes: u64) -> String {
    let zehntel = (bytes as u128 * 10 + (crate::hardware::GIB as u128) / 2) / crate::hardware::GIB as u128;
    format!("{},{} GiB", zehntel / 10, zehntel % 10)
}

/// Ein Katalogeintrag als JSON.
fn katalogeintrag(wurzel: &Path, schluessel: &str) -> Result<serde_json::Value, String> {
    let pfad = wurzel.join("MODELS/llm/KATALOG.json");
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let d: serde_json::Value = serde_json::from_str(&roh).map_err(|e| format!("{}: {e}", pfad.display()))?;
    d.get(schluessel).cloned().ok_or_else(|| format!("{schluessel} steht nicht im Katalog"))
}

fn text(v: &serde_json::Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn zahl(v: &serde_json::Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// Liegt eine vollstaendige Kalibrier-Umgebung da?
pub fn umgebung_da(wurzel: &Path) -> bool {
    let bin = wurzel.join(UMGEBUNG_BIN);
    bin.join("python3").is_file() && bin.join("hf").is_file()
}

/// Liegt das fertige Artefakt da?
pub fn artefakt_da(wurzel: &Path, schluessel: &str) -> bool {
    wurzel.join(crate::modelle::ARTEFAKTORDNER).join(schluessel).join("model_config.json").is_file()
}

impl Bauplan {
    /// **Was fuer `schluessel` noch fehlt**, in der Reihenfolge des Baus.
    pub fn fuer(wurzel: &Path, schluessel: &str) -> Result<Self, String> {
        let e = katalogeintrag(wurzel, schluessel)?;
        let mut schritte = Vec::new();
        if !artefakt_da(wurzel, schluessel) {
            if !umgebung_da(wurzel) {
                schritte.push(Schritt::Umgebung);
            }
            // ⚑ Ein Artefakt in vier Bit entsteht aus seiner int8-Quelle;
            //   die Gewichte braucht nur die Quelle.
            let (int8, vierbit) = match text(&e, "bauweg").as_str() {
                "vierbit" => {
                    let quelle = text(&e, "quelle");
                    if quelle.is_empty() {
                        return Err(format!("{schluessel}: bauweg vierbit ohne `quelle`"));
                    }
                    (katalogeintrag(wurzel, &quelle).map(|q| (quelle.clone(), q))?, true)
                }
                "" | "int8" => ((schluessel.to_string(), e.clone()), false),
                anders => return Err(format!("{schluessel}: unbekannter bauweg `{anders}`")),
            };
            let (quelle, q) = int8;
            if !artefakt_da(wurzel, &quelle) {
                let verzeichnis = text(&q, "hf_verzeichnis");
                let repo = text(&q, "hf_repo");
                if repo.is_empty() || verzeichnis.is_empty() {
                    return Err(format!("{quelle} nennt kein `hf_repo` oder `hf_verzeichnis`"));
                }
                if !wurzel.join("MODELS/llm").join(&verzeichnis).join("config.json").is_file() {
                    schritte.push(Schritt::Holen {
                        repo,
                        revision: {
                            let r = text(&q, "hf_revision");
                            if r.is_empty() { "main".into() } else { r }
                        },
                        verzeichnis,
                        bytes: zahl(&q, "gewichte_bytes"),
                    });
                }
                schritte.push(Schritt::Int8 { schluessel: quelle.clone(), bytes: zahl(&q, "artefakt_bytes") });
            }
            if vierbit {
                schritte.push(Schritt::Vierbit {
                    quelle,
                    ziel: schluessel.to_string(),
                    zwischen: zahl(&e, "zwischen_bytes"),
                    bytes: zahl(&e, "artefakt_bytes"),
                    dauer: text(&e, "bauzeit_anzeige"),
                });
            }
        }
        Ok(Bauplan {
            schluessel: schluessel.to_string(),
            anzeigename: {
                let n = text(&e, "anzeigename");
                if n.is_empty() { schluessel.to_string() } else { n }
            },
            wurzel: wurzel.to_path_buf(),
            schritte,
            hardware: text(&e, "hardware"),
        })
    }

    /// Wie viel Platz die fehlenden Schritte brauchen.
    pub fn bedarf(&self) -> Bedarf {
        let mut b = Bedarf::default();
        for s in &self.schritte {
            match s {
                Schritt::Umgebung => {
                    b.download += UMGEBUNG_BYTES;
                    b.bleibt += UMGEBUNG_BYTES;
                }
                Schritt::Holen { bytes, .. } => {
                    b.download += bytes;
                    b.bleibt += bytes;
                }
                Schritt::Int8 { bytes, .. } => b.bleibt += bytes,
                Schritt::Vierbit { zwischen, bytes, .. } => {
                    b.bleibt += bytes + WERKZEUG_BYTES;
                    b.zwischen += zwischen;
                }
            }
        }
        b
    }

    /// Was die Platte unter der Wurzel hergibt, und was die Freigabe der
    /// Einstellungen noch zulaesst.
    pub fn platzlage(&self, e: &Einstellungen) -> Platzlage {
        let frei = crate::hardware::Hardware::platte(&self.wurzel).map(|p| p.frei_bytes);
        let freigabe_rest = e.kapazitaet.platte_gib.map(|g| {
            let belegt: u64 = myl_local_agent::ort::gefuellte_orte().iter().map(|o| crate::reservierung::belegung(o)).sum();
            (g as u64 * crate::hardware::GIB).saturating_sub(belegt)
        });
        Platzlage { frei, freigabe_rest }
    }

    /// ⛔️ **Reicht der Platz?** `Err` nennt, wie viel fehlt und wo: Dann
    /// gibt es keine Freigabe, erst muss Speicher geschaffen werden.
    pub fn platz_pruefen(&self, lage: &Platzlage) -> Result<(), String> {
        let spitze = self.bedarf().spitze();
        let Some(frei) = lage.frei else {
            return Err(format!(
                "Der freie Platz unter {} laesst sich nicht messen; ohne diese Zahl gibt es keine Freigabe.",
                self.wurzel.display()
            ));
        };
        if frei < spitze {
            return Err(format!(
                "Zu wenig Speicher: Waehrend des Baus liegen bis zu {} zugleich auf der Platte, frei sind {}. \
                 Es fehlen {}. Erst Speicher schaffen, dann noch einmal.",
                gib(spitze),
                gib(frei),
                gib(spitze - frei)
            ));
        }
        if let Some(rest) = lage.freigabe_rest {
            if rest < spitze {
                return Err(format!(
                    "Die Freigabe in den Einstellungen reicht nicht: gebraucht werden bis zu {}, die Freigabe laesst noch {} zu. \
                     Es fehlen {}. Erst die Freigabe erhoehen oder Platz schaffen.",
                    gib(spitze),
                    gib(rest),
                    gib(spitze - rest)
                ));
            }
        }
        Ok(())
    }

    /// **Der Text, den der Mensch bestaetigt**: jeder Schritt, der Platz
    /// waehrend und nach dem Bau, und was frei ist.
    pub fn bestaetigung(&self, lage: &Platzlage) -> String {
        let b = self.bedarf();
        let mut s = format!("{} holen und bauen:\n", self.anzeigename);
        for (i, schritt) in self.schritte.iter().enumerate() {
            s.push_str(&format!("  {}. {}\n", i + 1, schritt.beschreibung()));
        }
        s.push_str(&format!(
            "Speicher waehrend des Baus: bis zu {} zugleich; danach bleiben {}.\n",
            gib(b.spitze()),
            gib(b.bleibt)
        ));
        if b.download > 0 {
            s.push_str(&format!("Aus dem Netz: {}.\n", gib(b.download)));
        }
        match lage.frei {
            Some(f) => s.push_str(&format!("Frei auf der Platte: {}", gib(f))),
            None => s.push_str("Frei auf der Platte: nicht messbar"),
        }
        if let Some(r) = lage.freigabe_rest {
            s.push_str(&format!(", die Freigabe laesst noch {} zu", gib(r)));
        }
        s.push_str(".\n");
        if !self.hardware.is_empty() {
            s.push_str(&format!("Mindestausstattung: {}.\n", self.hardware));
        }
        s
    }
}

/// Eine Zeile aus dem Lauf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zeile {
    /// `umgebung`, `holen`, `kalibrieren`, `vierbit`, `fertig` oder `fehler`.
    pub phase: &'static str,
    pub text: String,
}

/// Der `PATH` der Skripte: die Kalibrier-Umgebung vor dem System, denn
/// dort liegen `python3` mit torch und `hf`.
fn pfad_mit_umgebung(wurzel: &Path) -> std::ffi::OsString {
    let mut teile = vec![wurzel.join(UMGEBUNG_BIN)];
    if let Some(p) = std::env::var_os("PATH") {
        teile.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(teile).unwrap_or_default()
}

/// Faehrt ein Skript und reicht jede Zeile weiter. Die Ausgabe auf stderr
/// laeuft in einem eigenen Faden mit, sonst blockiert ein Kind, sobald
/// die Leitung voll ist.
fn skript(
    wurzel: &Path,
    name: &str,
    args: &[&str],
    umgebung: &[(&str, &str)],
    phase: &'static str,
    sagen: &(dyn Fn(Zeile) + Sync),
) -> Result<(), String> {
    use std::io::BufRead;
    // ⚑ **`bash` wird gesucht** (`myl_local_agent::schale`): Unter Windows
    //   liegt sie bei Git for Windows und nicht im PATH.
    let mut befehl = std::process::Command::new(myl_local_agent::schale::bash()?);
    befehl
        .arg(wurzel.join("INTEGER_LLM/scripts").join(name))
        .args(args)
        .current_dir(wurzel.join("INTEGER_LLM"))
        .env("PATH", pfad_mit_umgebung(wurzel))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, v) in umgebung {
        befehl.env(k, v);
    }
    let mut kind = befehl.spawn().map_err(|e| format!("{name}: {e} (gebraucht wird bash)"))?;
    let fehlerausgabe = kind.stderr.take();
    let letzte: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        if let Some(f) = fehlerausgabe {
            let letzte = &letzte;
            s.spawn(move || {
                for z in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                    if let Ok(mut l) = letzte.lock() {
                        l.push(z.clone());
                        let n = l.len();
                        if n > 20 {
                            l.drain(..n - 20);
                        }
                    }
                    sagen(Zeile { phase, text: z });
                }
            });
        }
        if let Some(a) = kind.stdout.take() {
            for z in std::io::BufReader::new(a).lines().map_while(Result::ok) {
                sagen(Zeile { phase, text: z });
            }
        }
    });
    let status = kind.wait().map_err(|e| format!("{name}: {e}"))?;
    if !status.success() {
        let l = letzte.lock().map(|l| l.join("\n")).unwrap_or_default();
        return Err(format!("{name} ist gescheitert ({status}).\n{l}"));
    }
    Ok(())
}

/// **Baut nach dem Plan**, Schritt fuer Schritt, und gibt den Pfad des
/// Artefakts zurueck.
///
/// ⛔️ Prueft den Platz **noch einmal** unmittelbar vorher, mit frischen
/// Zahlen: Zwischen Bestaetigung und Start kann die Platte voller geworden
/// sein, und eine Freigabe gilt nur fuer den Platz, der wirklich da ist.
pub fn ausfuehren(plan: &Bauplan, e: &Einstellungen, sagen: &(dyn Fn(Zeile) + Sync)) -> Result<PathBuf, String> {
    plan.platz_pruefen(&plan.platzlage(e))?;
    let w = &plan.wurzel;
    for s in &plan.schritte {
        sagen(Zeile { phase: s.phase(), text: s.beschreibung() });
        match s {
            Schritt::Umgebung => skript(w, "python_einrichten.sh", &[], &[], s.phase(), sagen)?,
            Schritt::Holen { repo, revision, .. } => {
                skript(w, "fetch_model.sh", &[], &[("MODEL_ID", repo), ("REVISION", revision)], s.phase(), sagen)?
            }
            Schritt::Int8 { schluessel, .. } => {
                skript(w, "build_artifacts.sh", &[], &[("INTEGER_LLM_MODEL", schluessel)], s.phase(), sagen)?;
                if !artefakt_da(w, schluessel) {
                    return Err(format!("Der Bau ist durch, aber {schluessel} fehlt; die Ausgabe oben sagt, woran es lag."));
                }
            }
            Schritt::Vierbit { quelle, ziel, .. } => skript(w, "build_vierbit.sh", &[quelle, ziel], &[], s.phase(), sagen)?,
        }
    }
    let ziel = w.join(crate::modelle::ARTEFAKTORDNER).join(&plan.schluessel);
    if !artefakt_da(w, &plan.schluessel) {
        return Err(format!("Der Lauf ist durch, aber {} fehlt.", ziel.join("model_config.json").display()));
    }
    sagen(Zeile { phase: "fertig", text: ziel.display().to_string() });
    Ok(ziel)
}

/// **Die Katalogeintraege, die hier noch nicht gebaut sind**, mit ihrem
/// Plan. Fuer die Modellwahl beider Bedieninstrumente.
pub fn baubar(wurzel: &Path) -> Vec<Bauplan> {
    let Ok(roh) = std::fs::read_to_string(wurzel.join("MODELS/llm/KATALOG.json")) else { return Vec::new() };
    let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else { return Vec::new() };
    let Some(o) = d.as_object() else { return Vec::new() };
    let mut aus: Vec<(f64, Bauplan)> = o
        .iter()
        .filter(|(k, v)| !k.starts_with('_') && text(v, "status") != "vorgemerkt")
        .filter_map(|(k, v)| {
            let p = Bauplan::fuer(wurzel, k).ok()?;
            (!p.schritte.is_empty()).then(|| (v.get("reihung").and_then(|x| x.as_f64()).unwrap_or(f64::MAX), p))
        })
        .collect();
    aus.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.schluessel.cmp(&b.1.schluessel)));
    aus.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Ein Scheinklon mit Katalog, ohne Gewichte und Artefakte.
    fn klon(name: &str) -> PathBuf {
        let w = std::env::temp_dir().join(format!("myl-bezug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join("MODELS/llm")).unwrap();
        std::fs::create_dir_all(w.join("INTEGER_LLM/artifacts")).unwrap();
        std::fs::write(
            w.join("MODELS/llm/KATALOG.json"),
            r#"{
  "_hinweis": "x",
  "klein": {"anzeigename": "Klein", "hf_repo": "a/klein", "hf_verzeichnis": "Klein", "hf_revision": "r1",
            "gewichte_bytes": 1000, "artefakt_bytes": 400, "status": "verifiziert", "reihung": 1.0, "hardware": "4 GB"},
  "gross": {"anzeigename": "Gross", "hf_repo": "a/gross", "hf_verzeichnis": "Gross", "hf_revision": "r2",
            "gewichte_bytes": 70000, "artefakt_bytes": 35000, "status": "erprobt", "reihung": 35.0},
  "gross-int4": {"anzeigename": "Gross, vier Bit", "hf_repo": "a/gross", "hf_verzeichnis": "Gross",
            "bauweg": "vierbit", "quelle": "gross", "zwischen_bytes": 40000, "artefakt_bytes": 16000,
            "status": "erprobt", "reihung": 35.0, "bauzeit_anzeige": "2 h"},
  "spaeter": {"anzeigename": "Spaeter", "hf_repo": "a/s", "hf_verzeichnis": "S", "status": "vorgemerkt"}
}"#,
        )
        .unwrap();
        w
    }

    fn artefakt(w: &Path, name: &str) {
        let a = w.join("INTEGER_LLM/artifacts").join(name);
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("model_config.json"), "{}").unwrap();
    }

    /// ⚑ **Vom kalten Klon aus braucht das int4 alles**: Umgebung, die
    /// Gewichte der Quelle, die Quelle in int8 und den Vierbitbau; und
    /// die Spitze ist alles, was bleibt, plus die Zwischenstaende.
    #[test]
    fn das_int4_braucht_vom_kalten_klon_alles() {
        let w = klon("kalt");
        let p = Bauplan::fuer(&w, "gross-int4").unwrap();
        let phasen: Vec<&str> = p.schritte.iter().map(Schritt::phase).collect();
        assert_eq!(phasen, ["umgebung", "holen", "kalibrieren", "vierbit"]);
        let b = p.bedarf();
        assert_eq!(b.download, UMGEBUNG_BYTES + 70_000);
        assert_eq!(b.zwischen, 40_000);
        assert_eq!(b.bleibt, UMGEBUNG_BYTES + 70_000 + 35_000 + 16_000 + WERKZEUG_BYTES);
        assert_eq!(b.spitze(), b.bleibt + 40_000);
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚑ **Gezaehlt wird nur, was fehlt.** Mit gebauter Quelle bleibt nur
    /// der Vierbitbau, ohne Download; mit gebautem Ziel gar nichts.
    #[test]
    fn was_da_ist_zaehlt_nicht() {
        let w = klon("da");
        artefakt(&w, "gross");
        std::fs::create_dir_all(w.join(UMGEBUNG_BIN)).unwrap();
        for d in ["python3", "hf"] {
            std::fs::write(w.join(UMGEBUNG_BIN).join(d), "").unwrap();
        }
        let p = Bauplan::fuer(&w, "gross-int4").unwrap();
        assert_eq!(p.schritte.len(), 1);
        assert_eq!(p.bedarf().download, 0);
        artefakt(&w, "gross-int4");
        assert!(Bauplan::fuer(&w, "gross-int4").unwrap().schritte.is_empty());
        // Liegen die Gewichte, faellt nur der Download weg.
        std::fs::create_dir_all(w.join("MODELS/llm/Klein")).unwrap();
        std::fs::write(w.join("MODELS/llm/Klein/config.json"), "{}").unwrap();
        let k = Bauplan::fuer(&w, "klein").unwrap();
        assert_eq!(k.schritte.iter().map(Schritt::phase).collect::<Vec<_>>(), ["kalibrieren"]);
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⛔️ **Ohne Platz keine Freigabe**: ein Byte zu wenig auf der Platte
    /// oder in der Freigabe, und es kommt die fehlende Zahl statt einer
    /// Bestaetigung; genau genug, und es geht.
    #[test]
    fn ohne_platz_gibt_es_keine_freigabe() {
        let w = klon("platz");
        let p = Bauplan::fuer(&w, "gross-int4").unwrap();
        let spitze = p.bedarf().spitze();
        let knapp = Platzlage { frei: Some(spitze - 1), freigabe_rest: None };
        let f = p.platz_pruefen(&knapp).unwrap_err();
        assert!(f.contains("Erst Speicher schaffen"), "{f}");
        assert!(p.platz_pruefen(&Platzlage { frei: Some(spitze), freigabe_rest: None }).is_ok());
        let freigabe = Platzlage { frei: Some(spitze * 2), freigabe_rest: Some(spitze - 1) };
        assert!(p.platz_pruefen(&freigabe).unwrap_err().contains("Freigabe"));
        assert!(p.platz_pruefen(&Platzlage { frei: None, freigabe_rest: None }).is_err(), "nicht messbar heisst nein");
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚑ **Die Bestaetigung nennt jeden Schritt und den Platz** waehrend und
    /// nach dem Bau, und den freien Platz.
    #[test]
    fn die_bestaetigung_nennt_den_platz() {
        let w = klon("text");
        let p = Bauplan::fuer(&w, "gross-int4").unwrap();
        let t = p.bestaetigung(&Platzlage { frei: Some(500 * crate::hardware::GIB), freigabe_rest: None });
        for erwartet in ["Gross, vier Bit holen und bauen", "1. Python-Umgebung", "2. Gewichte holen: a/gross", "3. int8-Artefakt gross", "4. Experten in vier Bit", "waehrend des Baus", "danach bleiben", "Frei auf der Platte: 500,0 GiB"] {
            assert!(t.contains(erwartet), "{erwartet:?} fehlt in:\n{t}");
        }
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚑ **Baubar ist, was fehlt und nicht vorgemerkt ist**, nach Groesse.
    #[test]
    fn baubar_ist_was_fehlt() {
        let w = klon("baubar");
        artefakt(&w, "klein");
        let namen: Vec<String> = baubar(&w).into_iter().map(|p| p.schluessel).collect();
        assert_eq!(namen, ["gross", "gross-int4"]);
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn gib_rundet_auf_ein_zehntel() {
        assert_eq!(gib(0), "0,0 GiB");
        assert_eq!(gib(crate::hardware::GIB), "1,0 GiB");
        assert_eq!(gib(71_903_645_408), "67,0 GiB");
    }
}
