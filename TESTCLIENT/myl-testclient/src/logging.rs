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
    /// Legt ein Protokoll unter `dir/<run-id>.{jsonl,log}` an.
    ///
    /// Die Lauf-Kennung ist `<unix-sekunden>-<befehl>` — sortierbar und
    /// ohne Rückfrage einem Befehl zuzuordnen. Schlägt das Anlegen der
    /// Dateien fehl, läuft der Client **trotzdem weiter** und meldet es
    /// auf stderr: Ein fehlendes Protokoll darf einen Hardwaretest nicht
    /// verhindern, aber es darf auch nicht unbemerkt bleiben.
    pub fn new(dir: &Path, command: &str, echo: bool) -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let run_id = format!("{}-{}", secs, command);

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
        log.finish(true);

        let jsonl = fs::read_to_string(dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
        let text = fs::read_to_string(dir.join(format!("{}.log", run_id))).expect("log");

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
        log.finish(true);

        let jsonl = fs::read_to_string(dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
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
        log.finish(true);

        let jsonl = fs::read_to_string(dir.join(format!("{}.jsonl", run_id))).expect("jsonl");
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
        log.finish(true);
        let jsonl = fs::read_to_string(dir.join(format!("{}.jsonl", run_id))).unwrap();
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

    #[test]
    fn run_id_traegt_den_befehl() {
        let dir = tempdir("runid");
        let log = RunLog::new(&dir, "determinismus", false);
        assert!(log.run_id().ends_with("-determinismus"));
        log.finish(true);
        let _ = fs::remove_dir_all(&dir);
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
