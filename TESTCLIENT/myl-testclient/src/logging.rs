//! Laufprotokolle — der eigentliche Zweck dieses Clients.
//!
//! Ein Testlauf ohne Protokoll ist wertlos: Der Client existiert, um
//! Ergebnisse **verschiedener Maschinen** und **verschiedener
//! Modellstände** vergleichbar zu machen. Genau dafür braucht jeder Lauf
//! einen Datensatz, der ohne Rückfrage beantwortet: Welche Hardware,
//! welches Backend, welches θ_v, welche Eingabe, welches Ergebnis?
//!
//! ## Zwei Ausgaben je Lauf
//!
//! - **`<run-id>.jsonl`** — eine JSON-Zeile je Ereignis, maschinenlesbar.
//!   Das ist die Fassung, die zwischen Maschinen verglichen wird.
//! - **`<run-id>.log`** — dieselben Ereignisse als Fließtext, für die
//!   Fehlersuche am Terminal.
//!
//! ## Wo die Dateien liegen
//!
//! ```text
//! logs/
//! └── determinismus/            ← je Prüflauf ein Ordner
//!     └── 2026-08-18_9f2c1a4b/  ← Datum + Kurzkennung der Einstellungen
//!         ├── 143052-aarch64-macos-reference.jsonl
//!         └── 143052-aarch64-macos-reference.log
//! ```
//!
//! Die **Kurzkennung** ist die halbe Miete beim Vergleich zwischen
//! Maschinen: Sie ist der Hash genau der Parameter, die gleich sein
//! müssen (Prompt, Tokenzahl, Shards, Modell — siehe [`crate::spec`]).
//! Alle Teilnehmer mit demselben Testplan landen im **gleichnamigen
//! Ordner**; wer versehentlich andere Parameter nimmt, landet sichtbar
//! woanders. Die Zuordnungsarbeit entfällt damit ganz.
//!
//! Der Dateiname trägt Uhrzeit und Hardware-Kurzform — damit sind auch
//! die Protokolle mehrerer Maschinen in einem Ordner ohne Umbenennen
//! unterscheidbar.
//!
//! Beide werden **immer** geschrieben, auch bei Abbruch. Ein Lauf, der
//! ohne Protokoll endet, ist ein Fehler des Clients, kein Sonderfall.
//!
//! ## Warum kein Logging-Framework
//!
//! Das Format ist Teil des Vergleichsverfahrens: Wer zwei Läufe von
//! zwei Maschinen diffen will, braucht **stabile Feldnamen und stabile
//! Reihenfolge**. Ein Framework mit konfigurierbarem Layout würde genau
//! das aufweichen. Die Serialisierung ist deshalb von Hand und
//! absichtlich langweilig.
//!
//! ## Was bewusst NICHT ins Protokoll geht
//!
//! Prompttexte werden **gehasht, nicht gespeichert** (siehe
//! [`Event::PromptAccepted`]). Ein Testprotokoll wandert erfahrungsgemäß
//! per Copy-Paste in Tickets und Chats; ein Prompt, der dabei
//! mitwandert, ist eine Datenschutzlücke, die niemand beabsichtigt hat.
//! Wer den Klartext braucht, hat ihn ohnehin — er hat ihn eingegeben.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ein protokolliertes Ereignis.
///
/// Bewusst ein geschlossenes Enum statt freier Textzeilen: Ein neuer
/// Ereignistyp erzwingt eine Entscheidung darüber, welche Felder er
/// trägt — und damit bleibt das Format diffbar.
#[derive(Debug, Clone)]
pub enum Event {
    /// Lauf beginnt. Trägt die Kennung des Unterbefehls.
    RunStarted { command: String },
    /// Hardware-Erhebung (Architektur, Betriebssystem, Backends).
    Hardware { key: String, value: String },
    /// Modell-/Artefakt-Identität: θ_v-Version, Artefakt-Hashes, Dimensionen.
    Artifact { key: String, value: String },
    /// Prompt angenommen — als Hash, nicht als Text (siehe Modul-Doku).
    PromptAccepted { token_count: usize, prompt_sha256: String },
    /// Ein Messschritt mit Dauer.
    Step {
        name: String,
        millis: u64,
        detail: String,
    },
    /// Ein Ergebnis, das zwischen Läufen verglichen wird.
    ///
    /// `digest` ist der eigentliche Vergleichswert; `value` ist die
    /// menschenlesbare Kurzfassung.
    Result {
        name: String,
        digest: String,
        value: String,
    },
    /// Eine Abweichung zwischen zwei Vergleichsgrößen.
    Mismatch {
        name: String,
        expected: String,
        actual: String,
    },
    /// Ein Hinweis, der keinen Fehlschlag bedeutet.
    Note { text: String },
    /// Ein Fehler. Beendet den Lauf, aber nicht das Protokoll.
    Error { text: String },
    /// Lauf beendet.
    RunFinished { ok: bool, millis: u64 },
}

impl Event {
    /// Typkennung für die JSONL-Zeile.
    fn kind(&self) -> &'static str {
        match self {
            Event::RunStarted { .. } => "run_started",
            Event::Hardware { .. } => "hardware",
            Event::Artifact { .. } => "artifact",
            Event::PromptAccepted { .. } => "prompt_accepted",
            Event::Step { .. } => "step",
            Event::Result { .. } => "result",
            Event::Mismatch { .. } => "mismatch",
            Event::Note { .. } => "note",
            Event::Error { .. } => "error",
            Event::RunFinished { .. } => "run_finished",
        }
    }

    /// Felder in **fester Reihenfolge** — Voraussetzung für den Diff
    /// zweier Läufe.
    fn fields(&self) -> Vec<(&'static str, String)> {
        match self {
            Event::RunStarted { command } => vec![("command", command.clone())],
            Event::Hardware { key, value } => {
                vec![("key", key.clone()), ("value", value.clone())]
            }
            Event::Artifact { key, value } => {
                vec![("key", key.clone()), ("value", value.clone())]
            }
            Event::PromptAccepted {
                token_count,
                prompt_sha256,
            } => vec![
                ("token_count", token_count.to_string()),
                ("prompt_sha256", prompt_sha256.clone()),
            ],
            Event::Step {
                name,
                millis,
                detail,
            } => vec![
                ("name", name.clone()),
                ("millis", millis.to_string()),
                ("detail", detail.clone()),
            ],
            Event::Result {
                name,
                digest,
                value,
            } => vec![
                ("name", name.clone()),
                ("digest", digest.clone()),
                ("value", value.clone()),
            ],
            Event::Mismatch {
                name,
                expected,
                actual,
            } => vec![
                ("name", name.clone()),
                ("expected", expected.clone()),
                ("actual", actual.clone()),
            ],
            Event::Note { text } => vec![("text", text.clone())],
            Event::Error { text } => vec![("text", text.clone())],
            Event::RunFinished { ok, millis } => vec![
                ("ok", ok.to_string()),
                ("millis", millis.to_string()),
            ],
        }
    }

    /// Menschenlesbare Zeile für Terminal und `.log`.
    fn human(&self) -> String {
        match self {
            Event::RunStarted { command } => format!("Lauf gestartet: {}", command),
            Event::Hardware { key, value } => format!("  Hardware  {:<22} {}", key, value),
            Event::Artifact { key, value } => format!("  Artefakt  {:<22} {}", key, value),
            Event::PromptAccepted {
                token_count,
                prompt_sha256,
            } => format!(
                "  Prompt    {} Token, sha256={}",
                token_count,
                &prompt_sha256[..16.min(prompt_sha256.len())]
            ),
            Event::Step {
                name,
                millis,
                detail,
            } => {
                if detail.is_empty() {
                    format!("  Schritt   {:<22} {} ms", name, millis)
                } else {
                    format!("  Schritt   {:<22} {} ms — {}", name, millis, detail)
                }
            }
            Event::Result {
                name,
                digest,
                value,
            } => format!(
                "  Ergebnis  {:<22} {}  [{}]",
                name,
                value,
                &digest[..16.min(digest.len())]
            ),
            Event::Mismatch {
                name,
                expected,
                actual,
            } => format!(
                "  ABWEICHUNG {}\n     erwartet: {}\n     erhalten: {}",
                name, expected, actual
            ),
            Event::Note { text } => format!("  Hinweis   {}", text),
            Event::Error { text } => format!("  FEHLER    {}", text),
            Event::RunFinished { ok, millis } => format!(
                "Lauf beendet: {} nach {} ms",
                if *ok { "OK" } else { "FEHLGESCHLAGEN" },
                millis
            ),
        }
    }
}

/// Minimale JSON-String-Maskierung.
///
/// Kein Fremd-Crate, weil das Format hier Teil des Vergleichsverfahrens
/// ist und stabil bleiben muss.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// SHA-256 als Hex — der Vergleichswert zwischen Läufen.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Beschreibt, wohin ein Lauf protokolliert wird.
///
/// Getrennt von [`RunLog`], damit die Ablagelogik für sich testbar ist —
/// sie ist der Teil, der beim Vergleich zwischen Maschinen zählt.
#[derive(Debug, Clone)]
pub struct LogZiel {
    /// Wurzelverzeichnis aller Protokolle.
    pub wurzel: PathBuf,
    /// Name des Prüflaufs (`hardware`, `determinismus`, `shard`, `stack`).
    pub befehl: String,
    /// Datum als `JJJJ-MM-TT`.
    pub datum: String,
    /// Kurzkennung der Einstellungen (8 Hexzeichen) oder `ohne-plan`.
    pub einstellungen: String,
    /// Hardware-Kurzform für den Dateinamen.
    pub hardware: String,
    /// Uhrzeit als `HHMMSS`.
    pub uhrzeit: String,
}

impl LogZiel {
    /// Baut das Ziel aus Befehl und Einstellungs-Kurzkennung.
    pub fn neu(wurzel: &Path, befehl: &str, einstellungen: &str, hardware: &str) -> Self {
        let (datum, uhrzeit) = datum_und_uhrzeit();
        Self {
            wurzel: wurzel.to_path_buf(),
            befehl: befehl.to_string(),
            datum,
            einstellungen: einstellungen.to_string(),
            hardware: saeubern(hardware),
            uhrzeit,
        }
    }

    /// Verzeichnis dieses Laufs: `<wurzel>/<befehl>/<datum>_<einstellungen>`.
    pub fn verzeichnis(&self) -> PathBuf {
        self.wurzel
            .join(&self.befehl)
            .join(format!("{}_{}", self.datum, self.einstellungen))
    }

    /// Dateiname ohne Endung: `<uhrzeit>-<hardware>`.
    pub fn dateiname(&self) -> String {
        format!("{}-{}", self.uhrzeit, self.hardware)
    }
}

/// Ersetzt alles, was in Dateinamen stört.
fn saeubern(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Datum und Uhrzeit als `(JJJJ-MM-TT, HHMMSS)` in UTC.
///
/// Von Hand gerechnet statt mit einem Datums-Crate: Der Client soll
/// ohne zusätzliche Abhängigkeiten bauen, und für die Ablage reicht
/// eine Umrechnung aus Unix-Sekunden. UTC bewusst — Teilnehmer sitzen
/// in verschiedenen Zeitzonen, und ein Ordner je Zeitzone wäre genau
/// die Zuordnungsarbeit, die vermieden werden soll.
fn datum_und_uhrzeit() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let tage = secs / 86_400;
    let rest = secs % 86_400;
    let (h, m, s) = (rest / 3600, (rest % 3600) / 60, rest % 60);

    // Zivildatum aus Tagen seit 1970 (Howard Hinnants Algorithmus).
    let z = tage as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let jahr = if mon <= 2 { y + 1 } else { y };

    (
        format!("{:04}-{:02}-{:02}", jahr, mon, d),
        format!("{:02}{:02}{:02}", h, m, s),
    )
}

/// Schreibt ein Laufprotokoll in zwei Fassungen.
pub struct RunLog {
    run_id: String,
    jsonl: Option<File>,
    text: Option<File>,
    started: std::time::Instant,
    /// Terminalausgabe zusätzlich zur Datei.
    echo: bool,
    /// Zahl der protokollierten Fehler und Abweichungen.
    problems: usize,
    dir: PathBuf,
}

impl RunLog {
    /// Legt ein Protokoll ohne Testplan an (Kurzkennung `ohne-plan`).
    pub fn new(dir: &Path, command: &str, echo: bool) -> Self {
        Self::mit_ziel(
            LogZiel::neu(dir, command, "ohne-plan", &crate::hardware::Fingerprint::collect().short_id()),
            echo,
        )
    }

    /// Legt ein Protokoll an dem beschriebenen Ziel an.
    ///
    /// Schlägt das Anlegen der Dateien fehl, läuft der Client
    /// **trotzdem weiter** und meldet es auf stderr: Ein fehlendes
    /// Protokoll darf einen Hardwaretest nicht verhindern, aber es darf
    /// auch nicht unbemerkt bleiben.
    pub fn mit_ziel(ziel: LogZiel, echo: bool) -> Self {
        let command = ziel.befehl.clone();
        let run_id = ziel.dateiname();
        let dir = ziel.verzeichnis();
        let dir = &dir;

        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!(
                "[myl-test] WARNUNG: Protokollverzeichnis {} nicht anlegbar: {}",
                dir.display(),
                e
            );
        }

        let open = |ext: &str| -> Option<File> {
            let path = dir.join(format!("{}.{}", run_id, ext));
            match File::create(&path) {
                Ok(f) => Some(f),
                Err(e) => {
                    eprintln!(
                        "[myl-test] WARNUNG: Protokolldatei {} nicht schreibbar: {}",
                        path.display(),
                        e
                    );
                    None
                }
            }
        };

        let mut log = Self {
            run_id: run_id.clone(),
            jsonl: open("jsonl"),
            text: open("log"),
            started: std::time::Instant::now(),
            echo,
            problems: 0,
            dir: dir.to_path_buf(),
        };
        log.event(Event::RunStarted {
            command: command.to_string(),
        });
        // Die Einstellungs-Kurzkennung gehört ins Protokoll selbst, nicht
        // nur in den Pfad — Protokolle werden einzeln weitergereicht.
        log.event(Event::Artifact {
            key: "einstellungen_id".into(),
            value: ziel.einstellungen.clone(),
        });
        log
    }

    /// Lauf-Kennung (Dateiname ohne Endung).
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Verzeichnis, in dem die Protokolle liegen.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Zahl der bisher protokollierten Fehler und Abweichungen.
    pub fn problems(&self) -> usize {
        self.problems
    }

    /// Protokolliert ein Ereignis in beide Fassungen.
    pub fn event(&mut self, ev: Event) {
        if matches!(ev, Event::Error { .. } | Event::Mismatch { .. }) {
            self.problems += 1;
        }

        let millis = self.started.elapsed().as_millis() as u64;

        let mut line = String::from("{");
        line.push_str(&format!("\"t_ms\":{}", millis));
        line.push_str(&format!(",\"run\":\"{}\"", json_escape(&self.run_id)));
        line.push_str(&format!(",\"kind\":\"{}\"", ev.kind()));
        for (k, v) in ev.fields() {
            line.push_str(&format!(",\"{}\":\"{}\"", k, json_escape(&v)));
        }
        line.push('}');

        if let Some(f) = self.jsonl.as_mut() {
            let _ = writeln!(f, "{}", line);
        }
        let human = ev.human();
        if let Some(f) = self.text.as_mut() {
            let _ = writeln!(f, "[{:>7} ms] {}", millis, human);
        }
        if self.echo {
            println!("{}", human);
        }
    }

    /// Kurzform für [`Event::Note`].
    pub fn note(&mut self, text: impl Into<String>) {
        self.event(Event::Note { text: text.into() });
    }

    /// Kurzform für [`Event::Error`].
    pub fn error(&mut self, text: impl Into<String>) {
        self.event(Event::Error { text: text.into() });
    }

    /// Kurzform für [`Event::Result`].
    pub fn result(&mut self, name: &str, digest: &str, value: impl Into<String>) {
        self.event(Event::Result {
            name: name.to_string(),
            digest: digest.to_string(),
            value: value.into(),
        });
    }

    /// Misst die Dauer von `f` und protokolliert sie als Schritt.
    pub fn timed<T>(&mut self, name: &str, detail: &str, f: impl FnOnce() -> T) -> T {
        let t0 = std::time::Instant::now();
        let out = f();
        self.event(Event::Step {
            name: name.to_string(),
            millis: t0.elapsed().as_millis() as u64,
            detail: detail.to_string(),
        });
        out
    }

    /// Schließt den Lauf ab und meldet, wo die Protokolle liegen.
    pub fn finish(mut self, ok: bool) -> bool {
        let millis = self.started.elapsed().as_millis() as u64;
        self.event(Event::RunFinished { ok, millis });
        if self.echo {
            println!(
                "\nProtokoll: {}/{}.jsonl (maschinenlesbar) und {}.log",
                self.dir.display(),
                self.run_id,
                self.run_id
            );
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-{}", name));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn protokoll_wird_in_beiden_fassungen_geschrieben() {
        let dir = tempdir("beide");
        let mut log = RunLog::new(&dir, "probe", false);
        log.note("hallo");
        let run_id = log.run_id().to_string();
        let lauf_dir = log.dir().to_path_buf();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
        let text = fs::read_to_string(lauf_dir.join(format!("{}.log", run_id))).expect("log");

        assert!(jsonl.contains("\"kind\":\"run_started\""));
        assert!(jsonl.contains("\"kind\":\"note\""));
        assert!(jsonl.contains("\"kind\":\"run_finished\""));
        assert!(text.contains("hallo"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn jede_zeile_ist_fuer_sich_gueltiges_json() {
        let dir = tempdir("zeilen");
        let mut log = RunLog::new(&dir, "probe", false);
        log.event(Event::Hardware {
            key: "arch".into(),
            value: "aarch64".into(),
        });
        log.result("token_hash", "abc", "42 Token");
        let run_id = log.run_id().to_string();
        let lauf_dir = log.dir().to_path_buf();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
        for line in jsonl.lines() {
            assert!(line.starts_with('{') && line.ends_with('}'), "Zeile: {}", line);
            // Ausgewogene Anfuehrungszeichen (grobe Struktursicht).
            let quotes = line.chars().filter(|c| *c == '"').count();
            assert_eq!(quotes % 2, 0, "unbalancierte Quotes: {}", line);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sonderzeichen in Werten dürfen die JSONL-Struktur nicht sprengen —
    /// sonst ist der Vergleich zweier Läufe nicht mehr maschinell machbar.
    #[test]
    fn sonderzeichen_werden_maskiert() {
        let dir = tempdir("escape");
        let mut log = RunLog::new(&dir, "probe", false);
        log.note("Zeile\nmit \"Anführungszeichen\" und \\Backslash\tTab");
        let run_id = log.run_id().to_string();
        let lauf_dir = log.dir().to_path_buf();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
        for line in jsonl.lines() {
            assert_eq!(line.lines().count(), 1, "Ereignis über mehrere Zeilen");
        }
        assert!(jsonl.contains("\\n"));
        assert!(jsonl.contains("\\\""));
        assert!(jsonl.contains("\\\\"));
        assert!(jsonl.contains("\\t"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fehler_und_abweichungen_werden_gezaehlt() {
        let dir = tempdir("probleme");
        let mut log = RunLog::new(&dir, "probe", false);
        assert_eq!(log.problems(), 0);
        log.note("kein Problem");
        assert_eq!(log.problems(), 0);
        log.error("kaputt");
        log.event(Event::Mismatch {
            name: "hash".into(),
            expected: "a".into(),
            actual: "b".into(),
        });
        assert_eq!(log.problems(), 2);
        log.finish(false);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn timed_liefert_das_ergebnis_durch() {
        let dir = tempdir("timed");
        let mut log = RunLog::new(&dir, "probe", false);
        let wert = log.timed("rechnen", "", || 6 * 7);
        assert_eq!(wert, 42);
        let run_id = log.run_id().to_string();
        let lauf_dir = log.dir().to_path_buf();
        log.finish(true);
        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", run_id))).unwrap();
        assert!(jsonl.contains("\"kind\":\"step\""));
        assert!(jsonl.contains("\"name\":\"rechnen\""));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Client darf ohne Protokollverzeichnis nicht abstürzen — ein
    /// Hardwaretest auf einer fremden Maschine soll auch dann laufen,
    /// wenn das Verzeichnis nicht schreibbar ist.
    #[test]
    fn unbeschreibbares_verzeichnis_bricht_den_lauf_nicht_ab() {
        let dir = Path::new("/proc/kein-schreibzugriff/myl");
        let mut log = RunLog::new(dir, "probe", false);
        log.note("läuft trotzdem");
        assert!(log.finish(true));
    }

    /// Der Dateiname trägt Uhrzeit und Hardware, der Pfad den Befehl —
    /// so sind Protokolle mehrerer Maschinen in einem Ordner
    /// unterscheidbar, ohne umbenannt zu werden.
    #[test]
    fn ablage_nach_befehl_datum_und_einstellungen() {
        let dir = tempdir("ablage");
        let ziel = LogZiel::neu(&dir, "determinismus", "9f2c1a4b", "aarch64-macos-reference");
        let pfad = ziel.verzeichnis();

        assert!(pfad.ends_with(format!("{}_9f2c1a4b", ziel.datum)));
        assert!(pfad.to_string_lossy().contains("determinismus"));
        assert!(ziel.dateiname().ends_with("-aarch64-macos-reference"));
        assert_eq!(ziel.datum.len(), 10, "Datum als JJJJ-MM-TT");
        assert_eq!(ziel.uhrzeit.len(), 6, "Uhrzeit als HHMMSS");

        let log = RunLog::mit_ziel(ziel, false);
        let lauf_dir = log.dir().to_path_buf();
        let run_id = log.run_id().to_string();
        log.finish(true);
        assert!(lauf_dir.join(format!("{}.jsonl", run_id)).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Zwei Läufe mit derselben Einstellungs-Kennung müssen im
    /// **gleichen** Ordner landen — das ist der Zweck der Kennung.
    #[test]
    fn gleiche_einstellungen_gleicher_ordner() {
        let dir = tempdir("gleich");
        let a = LogZiel::neu(&dir, "determinismus", "abcd1234", "aarch64-macos-reference");
        let b = LogZiel::neu(&dir, "determinismus", "abcd1234", "x86-64-linux-avx2");
        assert_eq!(a.verzeichnis(), b.verzeichnis());
        assert_ne!(a.dateiname(), b.dateiname(), "Hardware muss unterscheiden");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Einstellungen dürfen NICHT im selben Ordner landen —
    /// sonst würden unvergleichbare Läufe vermischt.
    #[test]
    fn andere_einstellungen_anderer_ordner() {
        let dir = tempdir("anders");
        let a = LogZiel::neu(&dir, "determinismus", "abcd1234", "hw");
        let b = LogZiel::neu(&dir, "determinismus", "99998888", "hw");
        assert_ne!(a.verzeichnis(), b.verzeichnis());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Die Einstellungs-Kennung steht auch IM Protokoll, nicht nur im
    /// Pfad — Protokolle werden einzeln weitergereicht.
    #[test]
    fn einstellungs_id_steht_im_protokoll() {
        let dir = tempdir("id-im-log");
        let log = RunLog::mit_ziel(LogZiel::neu(&dir, "stack", "deadbeef", "hw"), false);
        let lauf_dir = log.dir().to_path_buf();
        let run_id = log.run_id().to_string();
        log.finish(true);
        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", run_id))).unwrap();
        assert!(jsonl.contains("einstellungen_id"));
        assert!(jsonl.contains("deadbeef"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sonderzeichen in der Hardware-Kurzform dürfen keinen kaputten
    /// Dateinamen erzeugen.
    #[test]
    fn dateiname_wird_gesaeubert() {
        let dir = tempdir("saeubern");
        let z = LogZiel::neu(&dir, "hardware", "id", "arch/os:back end");
        assert!(!z.dateiname().contains('/'));
        assert!(!z.dateiname().contains(':'));
        assert!(!z.dateiname().contains(' '));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Das Datum muss ein plausibles Kalenderdatum sein — die
    /// Umrechnung aus Unix-Sekunden ist von Hand geschrieben.
    #[test]
    fn datum_ist_plausibel() {
        let (datum, uhrzeit) = datum_und_uhrzeit();
        let teile: Vec<&str> = datum.split('-').collect();
        assert_eq!(teile.len(), 3);
        let jahr: i32 = teile[0].parse().expect("Jahr");
        let monat: u32 = teile[1].parse().expect("Monat");
        let tag: u32 = teile[2].parse().expect("Tag");
        assert!((2020..2200).contains(&jahr), "Jahr {}", jahr);
        assert!((1..=12).contains(&monat), "Monat {}", monat);
        assert!((1..=31).contains(&tag), "Tag {}", tag);

        let h: u32 = uhrzeit[0..2].parse().expect("Stunde");
        let m: u32 = uhrzeit[2..4].parse().expect("Minute");
        let s: u32 = uhrzeit[4..6].parse().expect("Sekunde");
        assert!(h < 24 && m < 60 && s < 60);
    }

    #[test]
    fn sha256_hex_ist_stabil() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }
}
