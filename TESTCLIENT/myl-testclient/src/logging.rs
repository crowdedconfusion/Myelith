//! Laufprotokolle, der eigentliche Zweck dieses Clients.
//!
//! Ein Testlauf ohne Protokoll ist wertlos: Der Client existiert, um
//! Ergebnisse **verschiedener Maschinen** und **verschiedener
//! Modellstände** vergleichbar zu machen. Genau dafür braucht jeder Lauf
//! einen Datensatz, der ohne Rückfrage beantwortet: Welche Hardware,
//! welches Backend, welches θ_v, welche Eingabe, welches Ergebnis?
//!
//! ## Zwei Ausgaben je Lauf
//!
//! - **`.jsonl`**: eine JSON-Zeile je Ereignis, maschinenlesbar. Das ist
//!   die Fassung, die zwischen Maschinen verglichen wird und die
//!   [`crate::vergleich`] einliest.
//! - **`.log`**: dieselben Ereignisse als Fließtext, für die Fehlersuche
//!   am Terminal.
//!
//! ## Wo die Dateien liegen
//!
//! Alles flach in `logs/`, eine Datei je Lauf:
//!
//! ```text
//! logs/
//! ├── anna_9f2c1a4b_2026-08-21_143022.jsonl
//! ├── anna_9f2c1a4b_2026-08-21_143022.log
//! └── bjoern_9f2c1a4b_2026-08-21_150411.jsonl
//! ```
//!
//! Der Name setzt sich aus **Teilnehmer**, **Einstellungs-Kurzkennung**,
//! Datum und Uhrzeit zusammen. Jedes der vier Stücke beantwortet eine
//! Frage, die beim Vergleich zuerst gestellt wird:
//!
//! - **Teilnehmer**: von wem stammt dieser Lauf? Bei einem
//!   Cross-Hardware-Test schickt jeder seine Dateien an den Koordinator,
//!   und der muss sie ohne Rückfrage zuordnen können.
//! - **Kurzkennung**, der Hash genau der Parameter, die gleich sein
//!   müssen (Prompt, Tokenzahl, Shards, Modell: siehe [`crate::spec`]).
//!   Gleiche Kennung heißt vergleichbar; wer versehentlich andere
//!   Parameter genommen hat, ist sofort am Dateinamen erkennbar.
//! - **Datum und Uhrzeit**: trennen Wiederholungen desselben Laufs,
//!   ohne dass eine frühere Datei überschrieben wird.
//!
//! Dieselben Angaben stehen **auch im Protokoll** (`run_started` trägt
//! Befehl, Teilnehmer und Einstellungs-Kennung). Der Dateiname ist eine
//! Bequemlichkeit; die Zuordnung leisten die Daten, denn eine Datei wird
//! umbenannt, ein Feld nicht.
//!
//! Beide Dateien werden **immer** geschrieben, auch bei Abbruch. Ein
//! Lauf, der ohne Protokoll endet, ist ein Fehler des Clients, kein
//! Sonderfall.
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
//! Wer den Klartext braucht, hat ihn ohnehin: er hat ihn eingegeben.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ein protokolliertes Ereignis.
///
/// Bewusst ein geschlossenes Enum statt freier Textzeilen: Ein neuer
/// Ereignistyp erzwingt eine Entscheidung darüber, welche Felder er
/// trägt, und damit bleibt das Format diffbar.
#[derive(Debug, Clone)]
pub enum Event {
    /// Lauf beginnt. Trägt Befehl, Teilnehmer und Einstellungs-Kennung:
    /// die drei Angaben, nach denen Protokolle beim Vergleich sortiert
    /// werden. Sie stehen bewusst in der **ersten** Zeile: Wer eine Datei
    /// aufmacht, soll nicht suchen müssen, woher sie stammt.
    RunStarted {
        command: String,
        teilnehmer: String,
        einstellungen_id: String,
    },
    /// Hardware-Erhebung (Architektur, Betriebssystem, Backends).
    Hardware { key: String, value: String },
    /// Modell-/Artefakt-Identität: θ_v-Version, Artefakt-Hashes, Dimensionen.
    Artifact { key: String, value: String },
    /// Prompt angenommen: als Hash, nicht als Text (siehe Modul-Doku).
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

    /// Felder in **fester Reihenfolge**. Voraussetzung für den Diff
    /// zweier Läufe.
    fn fields(&self) -> Vec<(&'static str, String)> {
        match self {
            Event::RunStarted {
                command,
                teilnehmer,
                einstellungen_id,
            } => vec![
                ("command", command.clone()),
                ("teilnehmer", teilnehmer.clone()),
                ("einstellungen_id", einstellungen_id.clone()),
            ],
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
            Event::RunStarted {
                command,
                teilnehmer,
                einstellungen_id,
            } => format!(
                "Lauf gestartet: {}. Teilnehmer {}, Einstellungen {}",
                command, teilnehmer, einstellungen_id
            ),
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
                    format!("  Schritt   {:<22} {} ms: {}", name, millis, detail)
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
pub(crate) fn json_escape(s: &str) -> String {
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

/// SHA-256 als Hex, der Vergleichswert zwischen Läufen.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Beschreibt, wohin ein Lauf protokolliert wird.
///
/// Getrennt von [`RunLog`], damit die Ablagelogik für sich testbar ist:
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
    /// Name des Teilnehmers, so wie er eingegeben wurde. Geht **so** ins
    /// Protokoll; für den Dateinamen siehe [`LogZiel::dateisicher`].
    pub teilnehmer: String,
    /// Hardware-Kurzform, für die Laufkennung.
    pub hardware: String,
    /// Uhrzeit als `HHMMSS`.
    pub uhrzeit: String,
}

/// Ersatzname, wenn keiner angegeben wurde (Skriptläufe, `--quiet`).
///
/// Bewusst sichtbar und nicht etwa die Hardware-Kurzform: Wer eine so
/// benannte Datei im Auswertungsordner sieht, weiß sofort, dass die
/// Zuordnung fehlt, statt sie zu erraten.
pub const OHNE_NAME: &str = "ohne-name";

impl LogZiel {
    /// Baut das Ziel aus Befehl, Teilnehmer und Einstellungs-Kurzkennung.
    pub fn neu(
        wurzel: &Path,
        befehl: &str,
        teilnehmer: &str,
        einstellungen: &str,
        hardware: &str,
    ) -> Self {
        let (datum, uhrzeit) = datum_und_uhrzeit();
        let teilnehmer = teilnehmer.trim();
        Self {
            wurzel: wurzel.to_path_buf(),
            befehl: befehl.to_string(),
            datum,
            einstellungen: einstellungen.to_string(),
            teilnehmer: if teilnehmer.is_empty() || saeubern(teilnehmer).is_empty() {
                OHNE_NAME.to_string()
            } else {
                teilnehmer.to_string()
            },
            hardware: saeubern(hardware),
            uhrzeit,
        }
    }

    /// Der Teilnehmername in einer Form, die als Dateiname trägt.
    ///
    /// **Getrennt vom Namen selbst.** Ins Protokoll gehört, was jemand
    /// eingegeben hat; in einen Dateinamen gehört, was auf jedem
    /// Dateisystem und in jedem Mailanhang unverändert ankommt. Die erste
    /// Fassung säuberte schon bei der Eingabe, und aus „Björn" wurde
    /// „bj-rn": auch im Bericht, den der Koordinator liest.
    pub fn dateisicher(&self) -> String {
        saeubern(&self.teilnehmer)
    }

    /// Ablageort: schlicht `<wurzel>`, ohne Unterordner.
    pub fn verzeichnis(&self) -> PathBuf {
        self.wurzel.clone()
    }

    /// Kennung dieses Laufs: `<datum>-<uhrzeit>-<hardware>-<einstellungen>`.
    ///
    /// Trägt die Hardware statt des Namens: Sie ist gemessen, der Name ist
    /// eingetippt. Für die Zuordnung eines einzeln weitergereichten
    /// Protokolls zählt die Angabe, die niemand vertippen kann.
    pub fn dateiname(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.datum, self.uhrzeit, self.hardware, self.einstellungen
        )
    }

    /// Dateiname für die Protokolldatei:
    /// `<teilnehmer>_<einstellungen>_<datum>_<uhrzeit>`.
    ///
    /// Die Reihenfolge ist die des Vergleichsverfahrens: Eine alphabetische
    /// Dateiliste gruppiert zuerst nach Teilnehmer, dann nach Einstellung,
    /// dann chronologisch. Der Koordinator eines Cross-Hardware-Tests legt
    /// alle eingegangenen Dateien in einen Ordner und sieht daran, wer
    /// geliefert hat und wer mit den falschen Parametern gelaufen ist.
    ///
    /// Beispiel: `anna_abcd1234_2026-08-21_143022.jsonl`
    pub fn lauf_dateiname(&self) -> String {
        format!(
            "{}_{}_{}_{}",
            self.dateisicher(),
            self.einstellungen,
            self.datum,
            self.uhrzeit
        )
    }
}

/// Findet einen noch unbelegten Dateinamen, notfalls mit Zähler.
///
/// Die Uhrzeit im Namen hat Sekundenauflösung. Zwei Läufe in derselben
/// Sekunde: beim Menü ohne Weiteres möglich, etwa Hardware-Erhebung
/// direkt nach dem Protokoll-Durchlauf: bekämen sonst denselben Namen,
/// und der zweite überschriebe den ersten **stillschweigend**. Ein
/// verlorenes Protokoll ist genau das, was dieser Client nicht tun darf.
///
/// Eine feinere Uhrzeit wäre die naheliegende Alternative und die
/// schlechtere: Millisekunden im Dateinamen machen ihn schwerer lesbar,
/// und der Fall bliebe theoretisch bestehen.
fn freier_dateiname(dir: &Path, basis: &str) -> String {
    if !dir.join(format!("{}.jsonl", basis)).exists() {
        return basis.to_string();
    }
    for n in 2..1000 {
        let kandidat = format!("{}-{}", basis, n);
        if !dir.join(format!("{}.jsonl", kandidat)).exists() {
            return kandidat;
        }
    }
    basis.to_string()
}

/// Ersetzt alles, was in Dateinamen stört.
///
/// **Umlaute werden umschrieben, nicht getilgt.** Dieser Client wird
/// überwiegend von deutschsprachigen Teilnehmern bedient; ein „Björn",
/// der als „bj-rn" im Auswertungsordner landet, ist für den Koordinator
/// schlechter zuzuordnen als „bjoern". Alles Übrige jenseits von ASCII
/// wird zum Bindestrich: Der Dateiname muss über Mailanhänge und
/// verschiedene Dateisysteme unverändert ankommen, und dort ist ASCII die
/// einzige verlässliche Zusicherung.
fn saeubern(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'Ä' => out.push_str("Ae"),
            'Ö' => out.push_str("Oe"),
            'Ü' => out.push_str("Ue"),
            'ß' => out.push_str("ss"),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' => out.push(c),
            _ => out.push('-'),
        }
    }
    out
}

/// Datum und Uhrzeit als `(JJJJ-MM-TT, HHMMSS)` in UTC.
///
/// Von Hand gerechnet statt mit einem Datums-Crate: Der Client soll
/// ohne zusätzliche Abhängigkeiten bauen, und für die Ablage reicht
/// eine Umrechnung aus Unix-Sekunden. UTC bewusst. Teilnehmer sitzen
/// in verschiedenen Zeitzonen, und ein Ordner je Zeitzone wäre genau
/// die Zuordnungsarbeit, die vermieden werden soll.
pub(crate) fn datum_und_uhrzeit() -> (String, String) {
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
    /// Dateiname für die Protokolldatei (Einstellungen-Hash).
    dateiname: String,
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
    /// Legt ein Protokoll ohne Testplan und ohne Namen an.
    pub fn new(dir: &Path, command: &str, echo: bool) -> Self {
        Self::mit_ziel(
            LogZiel::neu(
                dir,
                command,
                OHNE_NAME,
                "ohne-plan",
                &crate::hardware::Fingerprint::collect().short_id(),
            ),
            echo,
        )
    }

    /// Legt ein Protokoll an dem beschriebenen Ziel an.
    ///
    /// Schlägt das Anlegen der Dateien fehl, läuft der Client
    /// **trotzdem weiter** und meldet es auf stderr: Ein fehlendes
    /// Protokoll darf einen Hardwaretest nicht verhindern, aber es darf
    /// auch nicht unbemerkt bleiben.
    ///
    /// Jeder Lauf bekommt eigene Dateien; siehe [`LogZiel::lauf_dateiname`].
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

        let dateiname = freier_dateiname(dir, &ziel.lauf_dateiname());

        let open = |ext: &str| -> Option<File> {
            let path = dir.join(format!("{}.{}", dateiname, ext));
            match fs::File::create(&path) {
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
            dateiname: dateiname.clone(),
            jsonl: open("jsonl"),
            text: open("log"),
            started: std::time::Instant::now(),
            echo,
            problems: 0,
            dir: dir.to_path_buf(),
        };
        // Teilnehmer und Einstellungs-Kurzkennung gehören ins Protokoll
        // selbst, nicht nur in den Dateinamen. Protokolle werden einzeln
        // weitergereicht, und eine Datei wird umbenannt, ein Feld nicht.
        log.event(Event::RunStarted {
            command: command.to_string(),
            teilnehmer: ziel.teilnehmer.clone(),
            einstellungen_id: ziel.einstellungen.clone(),
        });
        log
    }

    /// Lauf-Kennung (Dateiname ohne Endung).
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Dateiname für die Protokolldatei (Einstellungen-Hash).
    pub fn dateiname(&self) -> &str {
        &self.dateiname
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
    /// Nur auf das Terminal, **nicht** ins Protokoll.
    ///
    /// Für den erzeugten Klartext: Er hilft beim Zuschauen, gehört aber
    /// nicht in die Protokolldatei. Verglichen werden zwischen Maschinen
    /// Token und Digests; der Klartext ist daraus ableitbar und blähte
    /// die Datei nur auf. Bei `--quiet` erscheint er gar nicht.
    pub fn nur_anzeigen(&self, text: impl AsRef<str>) {
        if self.echo {
            println!("{}", text.as_ref());
        }
    }

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
                "\nProtokoll: {}/{}.jsonl (maschinenlesbar) und {}.log. Lauf {}",
                self.dir.display(),
                self.dateiname,
                self.dateiname,
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
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).expect("jsonl");
        let text = fs::read_to_string(lauf_dir.join(format!("{}.log", dateiname))).expect("log");

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
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).expect("jsonl");
        for line in jsonl.lines() {
            assert!(line.starts_with('{') && line.ends_with('}'), "Zeile: {}", line);
            // Ausgewogene Anfuehrungszeichen (grobe Struktursicht).
            let quotes = line.chars().filter(|c| *c == '"').count();
            assert_eq!(quotes % 2, 0, "unbalancierte Quotes: {}", line);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sonderzeichen in Werten dürfen die JSONL-Struktur nicht sprengen:
    /// sonst ist der Vergleich zweier Läufe nicht mehr maschinell machbar.
    #[test]
    fn sonderzeichen_werden_maskiert() {
        let dir = tempdir("escape");
        let mut log = RunLog::new(&dir, "probe", false);
        log.note("Zeile\nmit \"Anführungszeichen\" und \\Backslash\tTab");
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);

        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).expect("jsonl");
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
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);
        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains("\"kind\":\"step\""));
        assert!(jsonl.contains("\"name\":\"rechnen\""));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Client darf ohne Protokollverzeichnis nicht abstürzen: ein
    /// Hardwaretest auf einer fremden Maschine soll auch dann laufen,
    /// wenn das Verzeichnis nicht schreibbar ist.
    #[test]
    fn unbeschreibbares_verzeichnis_bricht_den_lauf_nicht_ab() {
        let dir = Path::new("/proc/kein-schreibzugriff/myl");
        let mut log = RunLog::new(dir, "probe", false);
        log.note("läuft trotzdem");
        assert!(log.finish(true));
    }

    /// Alle Protokolle liegen flach in `logs/`; unterschieden werden sie
    /// über den Dateinamen, nicht über Unterordner.
    #[test]
    fn alles_in_einer_datei_ohne_unterordner() {
        let dir = tempdir("ablage");
        let ziel = LogZiel::neu(&dir, "determinismus", "anna", "9f2c1a4b", "aarch64-macos-reference");

        // Kein Unterordner mehr: das Ziel ist die Protokollwurzel selbst.
        assert_eq!(ziel.verzeichnis(), dir);
        // Die Laufkennung traegt, was frueher im Pfad stand.
        let name = ziel.dateiname();
        assert!(name.contains(&ziel.datum), "Datum fehlt in {name}");
        assert!(name.contains("aarch64-macos-reference"), "Hardware fehlt in {name}");
        assert!(name.ends_with("9f2c1a4b"), "Einstellungen fehlen in {name}");
        assert_eq!(ziel.datum.len(), 10, "Datum als JJJJ-MM-TT");
        assert_eq!(ziel.uhrzeit.len(), 6, "Uhrzeit als HHMMSS");

        let einstellungen_hash = ziel.lauf_dateiname();
        let log = RunLog::mit_ziel(ziel, false);
        let lauf_dir = log.dir().to_path_buf();
        log.finish(true);

        assert!(lauf_dir.join(format!("{}.jsonl", einstellungen_hash)).is_file());
        assert!(lauf_dir.join(format!("{}.log", einstellungen_hash)).is_file());
        let unterordner = fs::read_dir(&lauf_dir)
            .expect("lesbar")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();
        assert_eq!(unterordner, 0, "logs/ darf keine Unterordner mehr anlegen");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Zwei Teilnehmer mit denselben Einstellungen dürfen sich **nicht**
    /// gegenseitig überschreiben, müssen aber an der gemeinsamen
    /// Kurzkennung als vergleichbar erkennbar bleiben. Beides zusammen ist
    /// der Zweck des Namensschemas.
    #[test]
    fn gleiche_einstellungen_verschiedene_teilnehmer() {
        let dir = tempdir("gleich");
        let a = LogZiel::neu(&dir, "determinismus", "anna", "abcd1234", "aarch64-macos-reference");
        let b = LogZiel::neu(&dir, "determinismus", "björn", "abcd1234", "x86-64-linux-avx2");
        assert_eq!(a.verzeichnis(), b.verzeichnis());
        assert_ne!(
            a.lauf_dateiname(),
            b.lauf_dateiname(),
            "verschiedene Teilnehmer dürfen sich nicht überschreiben"
        );
        assert!(a.lauf_dateiname().contains("abcd1234"));
        assert!(b.lauf_dateiname().contains("abcd1234"));
        assert!(a.lauf_dateiname().starts_with("anna_"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Einstellungen dürfen NICHT in dieselbe Datei schreiben:
    /// sonst würden unvergleichbare Läufe vermischt.
    #[test]
    fn andere_einstellungen_andere_datei() {
        let dir = tempdir("anders");
        let a = LogZiel::neu(&dir, "determinismus", "anna", "abcd1234", "hw");
        let b = LogZiel::neu(&dir, "determinismus", "anna", "99998888", "hw");
        assert_eq!(a.verzeichnis(), b.verzeichnis());
        assert_ne!(a.lauf_dateiname(), b.lauf_dateiname());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Teilnehmer und Einstellungs-Kennung stehen auch IM Protokoll, nicht
    /// nur im Dateinamen. Protokolle werden einzeln weitergereicht, und
    /// eine Datei wird umbenannt, ein Feld nicht.
    #[test]
    fn teilnehmer_und_einstellungs_id_stehen_im_protokoll() {
        let dir = tempdir("id-im-log");
        let log = RunLog::mit_ziel(LogZiel::neu(&dir, "stack", "anna", "deadbeef", "hw"), false);
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(true);
        let jsonl = fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        assert!(jsonl.contains(r#""einstellungen_id":"deadbeef""#), "{jsonl}");
        assert!(jsonl.contains(r#""teilnehmer":"anna""#), "{jsonl}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Zwei Läufe in derselben Sekunde dürfen sich nicht überschreiben.
    /// Der Fall tritt im Menü regelmäßig auf, weil `hardware` und `stack`
    /// zusammen unter einer Sekunde bleiben.
    #[test]
    fn zwei_laeufe_in_derselben_sekunde_ueberschreiben_sich_nicht() {
        let dir = tempdir("kollision");
        let ziel = LogZiel::neu(&dir, "hardware", "anna", "abcd1234", "hw");

        let a = RunLog::mit_ziel(ziel.clone(), false);
        let name_a = a.dateiname().to_string();
        a.finish(true);

        let b = RunLog::mit_ziel(ziel, false);
        let name_b = b.dateiname().to_string();
        b.finish(true);

        assert_ne!(name_a, name_b, "zweiter Lauf überschreibt den ersten");
        assert!(dir.join(format!("{}.jsonl", name_a)).is_file());
        assert!(dir.join(format!("{}.jsonl", name_b)).is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein leerer Name darf keinen Dateinamen erzeugen, der mit `_`
    /// beginnt, und er soll sichtbar als fehlend erkennbar sein.
    #[test]
    fn fehlender_name_wird_ersetzt() {
        let dir = tempdir("kein-name");
        let z = LogZiel::neu(&dir, "hardware", "   ", "abcd1234", "hw");
        assert_eq!(z.teilnehmer, OHNE_NAME);
        assert!(z.lauf_dateiname().starts_with("ohne-name_"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein Name mit Leerzeichen, Schrägstrich oder Doppelpunkt darf keinen
    /// kaputten Dateinamen erzeugen: er kommt aus einer Tastatureingabe.
    #[test]
    fn name_wird_gesaeubert() {
        let dir = tempdir("name-saeubern");
        let z = LogZiel::neu(&dir, "hardware", "Anna M/K:1", "abcd1234", "hw");
        for zeichen in ['/', ':', ' '] {
            assert!(
                !z.lauf_dateiname().contains(zeichen),
                "{:?} im Dateinamen: {}",
                zeichen,
                z.lauf_dateiname()
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Name im Protokoll ist der eingegebene; nur der Dateiname wird
    /// umgeschrieben. Umlaute werden dabei umschrieben, nicht getilgt.
    #[test]
    fn umlaute_ueberleben_im_namen_und_im_dateinamen() {
        let dir = tempdir("umlaute");
        let z = LogZiel::neu(&dir, "hardware", "Björn Müßig", "abcd1234", "hw");
        assert_eq!(z.teilnehmer, "Björn Müßig", "Protokollname wurde verändert");
        assert!(
            z.lauf_dateiname().starts_with("Bjoern-Muessig_"),
            "Dateiname: {}",
            z.lauf_dateiname()
        );
        assert!(z.lauf_dateiname().is_ascii(), "Dateiname nicht ASCII");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein Name, von dem nach dem Säubern nichts übrig bliebe, darf keinen
    /// Dateinamen erzeugen, der nur aus Bindestrichen besteht.
    #[test]
    fn unbrauchbarer_name_faellt_auf_ohne_name_zurueck() {
        let dir = tempdir("unbrauchbar");
        let z = LogZiel::neu(&dir, "hardware", "   ", "abcd1234", "hw");
        assert_eq!(z.teilnehmer, OHNE_NAME);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sonderzeichen in der Hardware-Kurzform dürfen keinen kaputten
    /// Dateinamen erzeugen.
    #[test]
    fn dateiname_wird_gesaeubert() {
        let dir = tempdir("saeubern");
        let z = LogZiel::neu(&dir, "hardware", "anna", "id", "arch/os:back end");
        assert!(!z.dateiname().contains('/'));
        assert!(!z.dateiname().contains(':'));
        assert!(!z.dateiname().contains(' '));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Das Datum muss ein plausibles Kalenderdatum sein: die
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
