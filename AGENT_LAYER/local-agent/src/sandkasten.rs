//! **Die Grenze des Betriebssystems um einen Befehl.**
//!
//! # ⚑ Warum es sie gibt
//!
//! `run_command` und jedes Manifest-Werkzeug fuehren `sh -c` aus, und eine
//! Shell kann alles, was der Prozess kann. Die Einhaengegrenze der
//! Dateiwerkzeuge galt fuer sie nie, und die Sperrliste ist ein Rueckfall
//! fuer acht bekannte Muster. Seit ein grosses Netzmodell ueber die Tuer
//! `run_command` bekommt, liest ein fremder Rechner jede Ausgabe mit, und
//! ein Text in einer fremden Datei kann einen Befehl ausloesen. Das
//! Whitepaper (8.3) sagt dazu: Ein getaeuschter Agent darf eine schlechte
//! Entscheidung **innerhalb** seiner Grenzen treffen, aber nicht darueber
//! hinaus handeln. Diese Grenze zieht kein Filter, sondern der Kern des
//! Betriebssystems:
//!
//! | | macOS | Linux |
//! |---|---|---|
//! | Mittel | Seatbelt ueber `sandbox-exec` | Landlock, dazu seccomp |
//! | Schreiben | nur Arbeitsordner und eigener Zwischenordner | dasselbe |
//! | Lesen | System ja; Heimat, Wechseldatentraeger und fremde Zwischenordner nein | dasselbe |
//! | Netz | zu (auch Unix-Sockel und die Dienste des Systems) | zu (`socket`, `connect`, `bind`) |
//! | Mit `netz` | IP offen, Unix-Sockel zu (ausser dem Namensdienst) | IP offen, Unix-Sockel zu |
//! | Andere Prozesse | kein Signal nach aussen, kein AppleEvent, kein `open` | kein `ptrace`; Signale ab ABI 6 |
//!
//! # ⚑ Drei Arten
//!
//! [`Sandkasten::Eng`] ist die Vorgabe fuer alles. [`Sandkasten::Netz`]
//! oeffnet zusaetzlich das Netz und ist fuer ein Manifest-Werkzeug gedacht,
//! das etwas abruft. [`Sandkasten::Frei`] zieht keine Grenze; das sagt ein
//! Manifest ueber sich selbst (eine Kiste, die das System verwaltet) oder
//! der Nutzer ueber die ganze Sitzung (`agent.sandbox aus`).
//!
//! # ⛔️ Ohne Grenze kein Befehl
//!
//! Wo das Betriebssystem keine Grenze hergibt (Windows, ein Linux ohne
//! Landlock ab ABI 3, ein Prozess, der selbst schon eingesperrt ist),
//! laeuft ein Befehl der Art `Eng` oder `Netz` **nicht**. Er liefe sonst
//! ohne die Grenze, die sein Name verspricht. Wer es trotzdem will,
//! schaltet `agent.sandbox` aus, und dann steht es in der Ansage.
//!
//! # ⚠️ Was die Grenze nicht leistet
//!
//!   - **Der Arbeitsordner gehoert dem Agenten.** Was er dort hinterlaesst
//!     (ein `Makefile`, ein Skript), fuehrt spaeter jemand ausserhalb aus.
//!     Geschuetzt sind unter macOS nur die Haken und die Konfiguration
//!     eines Repositoriums, das im Arbeitsordner schon liegt, weil `git`
//!     sie ungefragt ausfuehrt; ein Repositorium in einem Unterordner ist
//!     es nicht. Unter Linux leistet das ein eigener Namensraum, wo der
//!     Kern ihn ohne Vorrecht hergibt; sonst fehlt der Schutz dort, und die
//!     [`lage`] sagt es. `.git/commondir` gibt es meist nicht; unter Linux
//!     sieht [`Lauf::nachschau`] danach nach.
//!   - **Die Umgebung wird gesiebt, nicht bewiesen**: Variablen, deren Name
//!     nach einem Geheimnis klingt, bekommt der Befehl nicht. Ein Geheimnis
//!     unter einem harmlosen Namen geht durch.
//!   - **Lesen ausserhalb der Heimat ist frei.** Was auf der Platte unter
//!     `/etc` oder `/opt` lesbar liegt, liest auch der Befehl.
//!   - **Unter Linux geht in der engen Art ein Datagramm an einen
//!     Unix-Sockel mit Pfad hinaus** (`sendmsg`; eine Zeile ins
//!     Systemprotokoll). Ein Strom zu einem Dienst geht nicht.
//!   - `sandbox-exec` gilt bei Apple als veraltet und wird doch von den
//!     verbreiteten Agentenwerkzeugen genutzt; faellt es weg, meldet
//!     [`grenze`] das, und es gilt der Absatz darueber.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
#[path = "sandkasten_linux.rs"]
mod linux;

/// Wie eng ein Befehl eingesperrt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sandkasten {
    /// Schreiben nur im Arbeitsordner, kein Netz. Die Vorgabe.
    #[default]
    Eng,
    /// Wie `Eng`, aber mit Netz.
    Netz,
    /// Keine Grenze des Betriebssystems.
    Frei,
}

impl Sandkasten {
    /// Der Name, wie er im Manifest steht.
    pub fn name(self) -> &'static str {
        match self {
            Sandkasten::Eng => "eng",
            Sandkasten::Netz => "netz",
            Sandkasten::Frei => "frei",
        }
    }
}

/// Was das Betriebssystem an Grenze hergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grenze {
    /// macOS: ein Profil fuer `sandbox-exec`.
    Seatbelt,
    /// Linux: Landlock in der genannten Fassung, dazu seccomp.
    Landlock { abi: u32 },
    /// Keine, mit dem Grund.
    Keine(String),
}

/// **Welche Grenze diese Maschine hergibt.** Einmal erkundet, dann gemerkt.
pub fn grenze() -> &'static Grenze {
    static G: OnceLock<Grenze> = OnceLock::new();
    G.get_or_init(erkunden)
}

/// Ob diese Maschine eine Grenze hergibt.
pub fn vorhanden() -> bool {
    !matches!(grenze(), Grenze::Keine(_))
}

#[cfg(target_os = "macos")]
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

#[cfg(target_os = "macos")]
fn erkunden() -> Grenze {
    if !Path::new(SANDBOX_EXEC).is_file() {
        return Grenze::Keine(format!("{SANDBOX_EXEC} fehlt"));
    }
    // ⚑ **Einmal wirklich anwenden.** Dass die Datei da ist, sagt nicht,
    // dass sie hier wirkt: Ein Prozess, der selbst schon in einem
    // Sandkasten laeuft, darf keinen zweiten aufspannen.
    let mut probe = Command::new(SANDBOX_EXEC);
    probe.args(["-p", "(version 1)(allow default)", "/usr/bin/true"]);
    match myl_senses::prozess::laufen(&mut probe, 10, 1024) {
        Ok(a) if a.gut() => Grenze::Seatbelt,
        Ok(a) => Grenze::Keine(format!("sandbox-exec liess sich nicht anwenden ({})", a.kopf())),
        Err(f) => Grenze::Keine(format!("sandbox-exec liess sich nicht starten ({f})")),
    }
}

#[cfg(target_os = "linux")]
fn erkunden() -> Grenze {
    linux::erkunden()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn erkunden() -> Grenze {
    Grenze::Keine("dieses Betriebssystem gibt ohne weitere Rechte keine Grenze her".into())
}

/// **Ein Satz ueber die Grenze dieser Maschine**, fuer die Ansage an der
/// Tuer und fuer die Auskunft des Klienten.
pub fn lage() -> String {
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut s = lage_von(grenze());
    #[cfg(target_os = "linux")]
    if matches!(grenze(), Grenze::Landlock { .. }) {
        s.push_str(linux::namensraum_lage());
    }
    s
}

/// **Die Zeile fuer den Menschen beim Start**, mit der Einstellung:
/// eingeschaltet die [`lage`], ausgeschaltet die Warnung.
pub fn ansage(eingeschaltet: bool) -> String {
    if eingeschaltet {
        lage()
    } else {
        "aus (`agent.sandbox`): Ein Befehl kann alles, was du kannst.".to_string()
    }
}

fn lage_von(g: &Grenze) -> String {
    const WAS: &str = "schreiben nur im Arbeitsordner, lesen nicht in der uebrigen Heimat, kein Netz";
    match g {
        Grenze::Seatbelt => format!("an (Seatbelt): {WAS}."),
        Grenze::Landlock { abi } => format!("an (Landlock ABI {abi}, seccomp): {WAS}."),
        Grenze::Keine(grund) => format!(
            "diese Maschine hat keine Sandbox ({grund}); Befehle laufen erst, wenn `agent.sandbox` aus ist, \
             und dann ohne Grenze."
        ),
    }
}

/// **Der Satz, der an einem gescheiterten Befehl haengt.**
///
/// ⚑ Ein Modell, das `Operation not permitted` liest, sucht den Fehler
/// sonst bei sich und versucht es fuenfmal anders. Die Grenze zu nennen
/// kostet eine Zeile und beendet das.
pub fn hinweis(art: Sandkasten) -> &'static str {
    match art {
        Sandkasten::Eng => {
            "(Der Befehl lief in der Sandbox: schreiben nur im Arbeitsordner und in $TMPDIR, kein Lesen in der \
             uebrigen Heimat, kein Netz. Scheiterte er daran, ist das die Grenze und kein Fehler des Befehls.)"
        }
        Sandkasten::Netz => {
            "(Der Befehl lief in der Sandbox: schreiben nur im Arbeitsordner und in $TMPDIR, kein Lesen in der \
             uebrigen Heimat. Scheiterte er daran, ist das die Grenze und kein Fehler des Befehls.)"
        }
        Sandkasten::Frei => "",
    }
}

/// Was unter der Heimat lesbar bleibt, weil gewoehnliche Werkzeuge es
/// brauchen und kein Geheimnis darin liegt.
///
/// ⚠️ **Mit Absicht kurz.** `.cargo` steht nicht als Ganzes hier, weil
/// `credentials.toml` darin liegt; `.npmrc`, `.netrc`, `.ssh`, `.aws` und
/// `.config` ausser `git` fehlen aus demselben Grund.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
const WERKZEUGORTE: [&str; 13] = [
    ".gitconfig",
    ".config/git",
    ".cargo/bin",
    ".cargo/registry",
    ".cargo/git",
    ".cargo/config.toml",
    ".rustup",
    ".local/bin",
    ".local/lib",
    ".pyenv",
    ".nvm",
    "Library/Python",
    "go/bin",
];

/// Namensteile, an denen eine Umgebungsvariable als Geheimnis gilt.
const GEHEIME_NAMEN: [&str; 13] = [
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASSPHRASE",
    "CREDENTIAL",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "ACCESS_KEY",
    "VOLLMACHT",
    "SCHLUESSEL",
    "GEHEIM",
];

/// Ob eine Umgebungsvariable dem Befehl vorenthalten wird.
///
/// ⚑ `SSH_AUTH_SOCK` gehoert dazu: Der Sockel des Schluesselagenten
/// unterschreibt fuer jeden, der ihn erreicht.
pub fn ist_geheim(name: &str) -> bool {
    let gross = name.to_uppercase();
    gross == "SSH_AUTH_SOCK" || GEHEIME_NAMEN.iter().any(|g| gross.contains(g))
}

/// Was ein Aufrufer ueber seinen Befehl sagt.
#[derive(Debug, Clone, Copy)]
pub struct Auftrag<'a> {
    /// Der Arbeitsordner, aufgeloest. Hier darf geschrieben werden.
    pub wurzel: &'a Path,
    /// Wie eng.
    pub art: Sandkasten,
    /// Was zusaetzlich lesbar ist und **nie** beschreibbar, auch wenn es im
    /// Arbeitsordner liegt: der Ordner der Kiste, aus der ein Werkzeug
    /// kommt. Ein Werkzeug, das sein eigenes Manifest umschreiben kann,
    /// erklaert sich beim naechsten Aufruf selbst fuer frei.
    pub lesen: &'a [PathBuf],
    /// Was ausser dem Arbeitsordner beschreibbar ist: ein Ordner, den eine
    /// Kiste ausserhalb des Arbeitsordners fuer ihre Ausgabe nennt (die
    /// Vorschau des CAD-Moduls). ⚠️ Liegt einer davon in oder ueber einem
    /// Ordner aus `lesen`, gilt er nicht; die Kiste bleibt zu.
    pub schreiben: &'a [PathBuf],
}

/// Ein eigener Zwischenordner je Befehl, der mit dem Lauf verschwindet.
#[derive(Debug)]
struct Zwischenordner(PathBuf);

impl Drop for Zwischenordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn zwischenordner() -> Result<Zwischenordner, String> {
    static N: AtomicU64 = AtomicU64::new(0);
    let basis = std::env::temp_dir();
    let mut letzter = String::new();
    // ⚑ Ein paar Versuche: Ein abgestuerzter Lauf mit derselben
    // Prozessnummer kann seinen Ordner hinterlassen haben.
    for _ in 0..16 {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p = basis.join(format!("myl-sandkasten-{}-{n}", std::process::id()));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut bauer = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            bauer.mode(0o700);
        }
        match bauer.create(&p) {
            Ok(()) => {
                let echt = p.canonicalize().map_err(|e| format!("Zwischenordner {}: {e}", p.display()))?;
                return Ok(Zwischenordner(echt));
            }
            Err(e) => letzter = format!("{}: {e}", p.display()),
        }
    }
    Err(format!("kein Zwischenordner fuer den Sandkasten ({letzter})"))
}

/// **Die Nachschau fuer `.git/commondir`** (2026-11-17).
///
/// Steht diese Datei in `.git`, nimmt `git` Konfiguration und Haken aus dem
/// Ordner, den sie nennt; ein Befehl, der sie anlegt, haette damit die
/// geschuetzten Haken umgangen. Einhaengen laesst sich ueber eine Datei, die
/// es nicht gibt, nichts, und `git add` braucht `.git` beschreibbar. Unter
/// macOS verbietet das Profil sie; unter Linux wird nach dem Lauf
/// nachgesehen und zurueckgesetzt, was der Befehl daran geaendert hat.
/// ⚠️ Ein Prozess, der den Befehl ueberlebt, kann sie danach wieder anlegen.
#[derive(Debug)]
struct Gitwache {
    datei: PathBuf,
    vorher: Option<Vec<u8>>,
}

impl Gitwache {
    /// Nur fuer ein Repositorium, das beim Start schon da ist.
    fn neu(wurzel: &Path) -> Option<Self> {
        let git = wurzel.join(".git");
        if !std::fs::symlink_metadata(&git).is_ok_and(|m| m.is_dir()) {
            return None;
        }
        let datei = git.join("commondir");
        Some(Self { vorher: std::fs::read(&datei).ok(), datei })
    }

    fn nachschauen(&self) -> Option<String> {
        let jetzt = std::fs::read(&self.datei).ok();
        let verweis = std::fs::symlink_metadata(&self.datei).is_ok_and(|m| m.file_type().is_symlink());
        if jetzt == self.vorher && !(verweis && self.vorher.is_none()) {
            return None;
        }
        let zurueck = match &self.vorher {
            None => std::fs::remove_file(&self.datei),
            Some(v) => std::fs::write(&self.datei, v),
        };
        Some(match zurueck {
            Ok(()) => "(Der Befehl hat `.git/commondir` geaendert; zurueckgesetzt, denn darueber nimmt `git` \
                       Haken und Konfiguration aus einem anderen Ordner.)"
                .to_string(),
            Err(e) => format!(
                "⛔️ Der Befehl hat `.git/commondir` geaendert, und es liess sich nicht zuruecksetzen ({e}). \
                 Vor dem naechsten `git` in diesem Ordner nachsehen."
            ),
        })
    }
}

/// Ein eingerichteter Befehl. Solange er lebt, lebt sein Zwischenordner.
#[derive(Debug)]
pub struct Lauf {
    /// Der Befehl, fertig bis auf Arbeitsverzeichnis und eigene Umgebung.
    pub kind: Command,
    /// Die Art, die wirklich gilt.
    pub art: Sandkasten,
    _zwischen: Option<Zwischenordner>,
    wache: Option<Gitwache>,
}

impl Lauf {
    /// **Nach dem Lauf aufrufen**: setzt zurueck, was der Befehl an
    /// `.git/commondir` geaendert hat, und sagt es ([`Gitwache`]).
    pub fn nachschau(&self) -> Option<String> {
        self.wache.as_ref().and_then(Gitwache::nachschauen)
    }
}

/// **Baut `sh -c befehl` unter der Grenze dieser Maschine.**
///
/// Der Aufrufer setzt danach Arbeitsverzeichnis und eigene Umgebung und
/// startet ueber `myl_senses::prozess::laufen`. Ein Fehler heisst: Dieser
/// Befehl laeuft nicht (siehe Modulkopf, „Ohne Grenze kein Befehl").
pub fn einrichten(sh: &Path, befehl: &str, a: &Auftrag) -> Result<Lauf, String> {
    einrichten_mit(grenze(), sh, befehl, a)
}

/// **Schreibt eine Datei unter der Grenze des Betriebssystems** (2026-11-18):
/// ein `sh` in der engen Sandbox (`Sandkasten::Eng`, kein Netz) legt fehlende
/// Ordner an und schreibt die Bytes aus einer Roehre, `cat > "$MYL_ZIEL"`.
///
/// ⚑ **Warum zusaetzlich zur Pruefung im Prozess** (Audit A52): Die
/// Dateiwerkzeuge laufen im Prozess des Agenten. Ihre Grenze
/// ([`crate::werkzeuge::Einhaengung::aufloesen`], [`crate::verweisfrei`]) ist
/// eigener Code; ein Fehler darin oeffnete das ganze Dateisystem des Nutzers.
/// Fuer das Schreiben, den schwersten Fall (`~/.ssh/authorized_keys`,
/// `~/.zshrc`), haelt jetzt das Betriebssystem dieselbe Grenze ein zweites
/// Mal: ausserhalb des Arbeitsordners schreibt der Helfer nicht, gleich was
/// der Pfad sagt. Lesen bleibt im Prozess (eine verlustfreie Rueckgabe
/// beliebiger Bytes ueber die Ausgabe eines Prozesses kostete mehr, als sie
/// hier bringt).
///
/// `None`, wenn diese Maschine keine Grenze hat; dann schreibt der Aufrufer
/// im Prozess. Ein Fehler des Helfers ist ein Fehler und kein Rueckfall.
pub fn datei_schreiben(wurzel: &Path, ziel: &Path, inhalt: &[u8]) -> Option<std::io::Result<()>> {
    if !vorhanden() {
        return None;
    }
    let sh = match crate::schale::sh() {
        Ok(sh) => sh,
        Err(f) => return Some(Err(std::io::Error::other(f))),
    };
    let auftrag = Auftrag { wurzel, art: Sandkasten::Eng, lesen: &[], schreiben: &[] };
    let befehl = r#"mkdir -p -- "$(dirname -- "$MYL_ZIEL")" && exec cat > "$MYL_ZIEL""#;
    let mut lauf = match einrichten(&sh, befehl, &auftrag) {
        Ok(l) => l,
        Err(f) => return Some(Err(std::io::Error::other(f))),
    };
    lauf.kind.env("MYL_ZIEL", ziel).current_dir(wurzel);
    let (leser, mut schreiber) = match std::io::pipe() {
        Ok(p) => p,
        Err(f) => return Some(Err(f)),
    };
    let bytes = inhalt.to_vec();
    // Die Roehre fasst weniger als eine Datei; geschrieben wird nebenher.
    let zufuhr = std::thread::spawn(move || {
        use std::io::Write;
        let r = schreiber.write_all(&bytes);
        drop(schreiber);
        r
    });
    let ausgang = myl_senses::prozess::laufen_mit_eingabe(&mut lauf.kind, std::process::Stdio::from(leser), 60, 4096);
    let zugefuehrt = zufuhr.join().unwrap_or_else(|_| Err(std::io::Error::other("Zufuhr abgebrochen")));
    if let Some(zeile) = lauf.nachschau() {
        // Ein Schreibhelfer aendert `.git/commondir` nie; geschah es doch,
        // ist es jetzt zurueckgesetzt und gehoert gesagt.
        eprintln!("myl-local-agent: {zeile}");
    }
    Some(match ausgang {
        Ok(a) if a.gut() => zugefuehrt,
        Ok(a) => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("die Sandbox hat das Schreiben nicht zugelassen ({}): {}", a.kopf(), a.fehler.trim()),
        )),
        Err(f) => Err(std::io::Error::other(f)),
    })
}

/// Dasselbe mit einer genannten Grenze; die Proben pruefen so den Fall,
/// den diese Maschine nicht hat.
pub fn einrichten_mit(g: &Grenze, sh: &Path, befehl: &str, a: &Auftrag) -> Result<Lauf, String> {
    if a.art == Sandkasten::Frei {
        let mut kind = Command::new(sh);
        kind.arg("-c").arg(befehl);
        return Ok(Lauf { kind, art: Sandkasten::Frei, _zwischen: None, wache: None });
    }
    if let Grenze::Keine(grund) = g {
        return Err(format!(
            "laeuft nicht: Diese Maschine hat keine Sandbox ({grund}). `myl setzen agent.sandbox aus` \
             laesst Befehle ohne Grenze zu"
        ));
    }
    let zwischen = zwischenordner()?;
    let mut kind = aufbauen(g, sh, befehl, a, &zwischen.0)?;
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_none_or(ist_geheim) {
            kind.env_remove(&name);
        }
    }
    for name in ["TMPDIR", "TMP", "TEMP"] {
        kind.env(name, &zwischen.0);
    }
    Ok(Lauf { kind, art: a.art, _zwischen: Some(zwischen), wache: Gitwache::neu(a.wurzel) })
}

/// Der Befehl unter der Grenze dieses Systems, je System eine Fassung.
#[cfg(target_os = "macos")]
fn aufbauen(g: &Grenze, sh: &Path, befehl: &str, a: &Auftrag, zwischen: &Path) -> Result<Command, String> {
    match g {
        Grenze::Seatbelt => Ok(seatbelt(sh, befehl, a, zwischen, &lesbare_orte(a))),
        andere => Err(format!("laeuft nicht: Die Grenze {andere:?} gibt es auf diesem System nicht")),
    }
}

#[cfg(target_os = "linux")]
fn aufbauen(g: &Grenze, sh: &Path, befehl: &str, a: &Auftrag, zwischen: &Path) -> Result<Command, String> {
    match g {
        Grenze::Landlock { abi } => linux::einrichten(*abi, sh, befehl, a, zwischen, &lesbare_orte(a)),
        andere => Err(format!("laeuft nicht: Die Grenze {andere:?} gibt es auf diesem System nicht")),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn aufbauen(g: &Grenze, _sh: &Path, _befehl: &str, _a: &Auftrag, _zwischen: &Path) -> Result<Command, String> {
    Err(format!("laeuft nicht: Die Grenze {g:?} gibt es auf diesem System nicht"))
}

/// Die Heimat, aufgeloest, wenn es sie gibt.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
fn heimat() -> Option<PathBuf> {
    let roh = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    PathBuf::from(roh).canonicalize().ok()
}

/// **Was der Auftrag zusaetzlich beschreibbar macht**, aufgeloest, ohne
/// alles, was eine Kiste aus `lesen` enthaelt oder in ihr liegt, und ohne
/// die Wurzel des Dateisystems.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
fn beschreibbare_orte(a: &Auftrag) -> Vec<PathBuf> {
    let zu: Vec<PathBuf> = a.lesen.iter().filter_map(|p| p.canonicalize().ok()).collect();
    a.schreiben
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir() && p.parent().is_some())
        .filter(|p| !zu.iter().any(|k| k.starts_with(p) || p.starts_with(k)))
        .collect()
}

/// **Was ueber die festen Baeume hinaus nicht gelesen wird**: die Heimat
/// und der Zwischenordner, den dieser Prozess mit anderen teilt.
///
/// ⚑ Beide koennen woanders liegen als ueblich (`HOME` auf einer zweiten
/// Platte, `TMPDIR` in einem eigenen Ordner), und im geteilten
/// Zwischenordner liegt, was andere Programme gerade ablegen. Der Befehl
/// bekommt deshalb einen eigenen. ⚠️ Die Wurzel des Dateisystems wird nie
/// gesperrt; wer `TMPDIR=/` setzt, bekommt diese eine Sperre nicht.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
fn gesperrte_orte() -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = Vec::new();
    aus.extend(heimat());
    aus.extend(std::env::temp_dir().canonicalize().ok());
    aus.retain(|p| p.parent().is_some());
    aus.sort();
    aus.dedup();
    aus
}

/// Was ausser Arbeitsordner und Zwischenordner lesbar ist: die
/// Werkzeugorte der Heimat, soweit vorhanden, und was der Auftrag nennt.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
fn lesbare_orte(a: &Auftrag) -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = Vec::new();
    if let Some(h) = heimat() {
        aus.extend(WERKZEUGORTE.iter().filter_map(|o| h.join(o).canonicalize().ok()));
    }
    aus.extend(a.lesen.iter().filter_map(|p| p.canonicalize().ok()));
    aus
}

/// **Wo die Entwicklerwerkzeuge von macOS wirklich liegen**, und wo ihr SDK.
///
/// 📌 `/usr/bin/git`, `python3`, `cc` und `make` sind auf macOS Weichen,
/// die ueber `xcrun` das echte Programm suchen und das Ergebnis im
/// Zwischenordner des Nutzers merken. Im Sandkasten darf dort niemand
/// schreiben (wer diese Merkdatei faelscht, bestimmt, welches Programm
/// **ausserhalb** als `git` laeuft), und jede Weiche meldete das auf jeder
/// Zeile mit `couldn't create cache file`. Der Befehl bekommt deshalb den
/// echten Ordner vorn in den `PATH` und das SDK genannt, dann fragt
/// niemand mehr.
#[cfg(target_os = "macos")]
fn entwicklerwerkzeuge() -> &'static (Option<PathBuf>, Option<String>) {
    static E: OnceLock<(Option<PathBuf>, Option<String>)> = OnceLock::new();
    E.get_or_init(|| {
        let frage = |programm: &str, argument: &str| -> Option<String> {
            let mut b = Command::new(programm);
            b.arg(argument);
            let a = myl_senses::prozess::laufen(&mut b, 10, 4096).ok()?;
            let zeile = a.aus.trim().to_string();
            (a.gut() && !zeile.is_empty()).then_some(zeile)
        };
        let ordner = frage("/usr/bin/xcode-select", "-p").map(|d| PathBuf::from(d).join("usr/bin")).filter(|p| p.is_dir());
        let sdk = ordner.as_ref().and_then(|_| frage("/usr/bin/xcrun", "--show-sdk-path"));
        (ordner, sdk)
    })
}

/// Das Profil fuer `sandbox-exec`. Pfade stehen nie im Text, sondern
/// kommen als Parameter; so muss keiner maskiert werden.
#[cfg(target_os = "macos")]
fn seatbelt(sh: &Path, befehl: &str, a: &Auftrag, zwischen: &Path, lesbar: &[PathBuf]) -> Command {
    let mut profil = String::from(
        "(version 1)\n\
         (deny default)\n\
         (import \"system.sb\")\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow signal (target same-sandbox))\n\
         (allow sysctl-read)\n\
         (allow file-read*)\n\
         (deny file-read-data (subpath \"/Users\") (subpath \"/Volumes\") (subpath \"/private/tmp\") \
         (subpath \"/private/var/tmp\") (subpath \"/private/var/folders\"))\n\
         (deny file-read-data (regex #\"^/dev/tty\"))\n\
         (allow file-read-data (regex #\"^/private/var/folders/[^/]+/[^/]+/C(/|$)\"))\n",
    );
    let mut kind = Command::new(SANDBOX_EXEC);
    let param = |kind: &mut Command, name: &str, wert: &Path| {
        let mut zuweisung = std::ffi::OsString::from(format!("{name}="));
        zuweisung.push(wert);
        kind.arg("-D").arg(zuweisung);
    };
    // ⚑ **Die letzte passende Regel gilt**, also erst sperren, dann
    // erlauben: Arbeitsordner und Zwischenordner liegen meist selbst unter
    // einem gesperrten Baum.
    for (i, ort) in gesperrte_orte().iter().enumerate() {
        profil.push_str(&format!("(deny file-read-data (subpath (param \"G{i}\")))\n"));
        param(&mut kind, &format!("G{i}"), ort);
    }
    profil.push_str(
        "(allow file-read-data (subpath (param \"WURZEL\")) (subpath (param \"ZWISCHEN\")))\n\
         (allow file-write* (subpath (param \"WURZEL\")) (subpath (param \"ZWISCHEN\")))\n\
         (allow file-write-data (literal \"/dev/null\"))\n",
    );
    param(&mut kind, "WURZEL", a.wurzel);
    param(&mut kind, "ZWISCHEN", zwischen);
    // ⚑ **Ein Repositorium, das schon da ist, behaelt, was `git` ungefragt
    // ausfuehrt**: die Haken, die Konfiguration (`core.fsmonitor`,
    // `core.hooksPath`, ein Alias) und den Verweis auf einen anderen
    // gemeinsamen Ordner. Der Ordner `.git` selbst laesst sich nicht
    // wegschieben, sonst traete ein neuer an seine Stelle. Alles andere
    // darin bleibt beschreibbar, damit `git add` und `git commit` gehen.
    //
    // ⚠️ Gibt es beim Start **kein** `.git`, gibt es nichts zu schuetzen:
    // Ein Repositorium, das der Befehl selbst anlegt, gehoert ihm wie
    // jede andere Datei im Arbeitsordner.
    let git = a.wurzel.join(".git");
    match std::fs::symlink_metadata(&git) {
        Ok(m) if m.is_dir() => {
            profil.push_str(
                "(deny file-write* (literal (param \"GIT\")) (subpath (param \"GITHAKEN\")) \
                 (literal (param \"GITCONFIG\")) (literal (param \"GITGEMEINSAM\")))\n",
            );
            param(&mut kind, "GIT", &git);
            param(&mut kind, "GITHAKEN", &git.join("hooks"));
            param(&mut kind, "GITCONFIG", &git.join("config"));
            param(&mut kind, "GITGEMEINSAM", &git.join("commondir"));
        }
        // Eine Datei oder ein Verweis: Sie zeigt auf das eigentliche
        // Repositorium und darf nicht umgebogen werden.
        Ok(_) => {
            profil.push_str("(deny file-write* (literal (param \"GIT\")))\n");
            param(&mut kind, "GIT", &git);
        }
        Err(_) => {}
    }
    for (i, ort) in lesbar.iter().enumerate() {
        let art = if ort.is_dir() { "subpath" } else { "literal" };
        profil.push_str(&format!("(allow file-read-data ({art} (param \"L{i}\")))\n"));
        param(&mut kind, &format!("L{i}"), ort);
    }
    for (i, ort) in beschreibbare_orte(a).iter().enumerate() {
        profil.push_str(&format!(
            "(allow file-read-data (subpath (param \"B{i}\")))\n(allow file-write* (subpath (param \"B{i}\")))\n"
        ));
        param(&mut kind, &format!("B{i}"), ort);
    }
    // ⚑ Was der Auftrag als lesbar nennt, ist nie beschreibbar, auch nicht
    // im Arbeitsordner (siehe `Auftrag::lesen`). Die Regel steht nach der
    // Schreiberlaubnis, denn die letzte passende gilt.
    let eigene = a.lesen.iter().filter_map(|p| p.canonicalize().ok());
    for (i, ort) in eigene.enumerate() {
        let art = if ort.is_dir() { "subpath" } else { "literal" };
        profil.push_str(&format!("(deny file-write* ({art} (param \"S{i}\")))\n"));
        param(&mut kind, &format!("S{i}"), &ort);
    }
    if a.art == Sandkasten::Netz {
        // ⚑ **Nur das Netz, nicht jeder Sockel.** Ein Unix-Sockel wie der
        // von Docker waere ein Weg hinaus; offen ist allein der des
        // Namensdienstes, ohne den kein Rechnername aufgeloest wird.
        profil.push_str(
            "(allow network-outbound (remote ip \"*:*\"))\n\
             (allow network-inbound (local ip \"*:*\"))\n\
             (allow network-bind (local ip \"*:*\"))\n\
             (allow network-outbound (literal \"/private/var/run/mDNSResponder\"))\n\
             (allow system-socket)\n",
        );
    }
    kind.arg("-p").arg(profil).arg(sh).arg("-c").arg(befehl);
    let (ordner, sdk) = entwicklerwerkzeuge();
    if let Some(o) = ordner {
        let mut pfad = o.clone().into_os_string();
        if let Some(alt) = std::env::var_os("PATH") {
            pfad.push(":");
            pfad.push(alt);
        }
        kind.env("PATH", pfad);
    }
    if let (Some(s), None) = (sdk, std::env::var_os("SDKROOT")) {
        kind.env("SDKROOT", s);
    }
    kind
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn die_namen_der_arten_sind_die_des_manifests() {
        for art in [Sandkasten::Eng, Sandkasten::Netz, Sandkasten::Frei] {
            let gelesen: Sandkasten = serde_json::from_str(&format!("\"{}\"", art.name())).expect("lesbar");
            assert_eq!(gelesen, art);
        }
        assert!(serde_json::from_str::<Sandkasten>("\"offen\"").is_err(), "eine unbekannte Art ist ein Fehler");
        assert_eq!(Sandkasten::default(), Sandkasten::Eng, "die Vorgabe ist die enge");
    }

    #[test]
    fn geheime_namen_werden_erkannt_und_harmlose_nicht() {
        for name in ["GITHUB_TOKEN", "aws_secret_access_key", "MYL_VOLLMACHT", "SSH_AUTH_SOCK", "OPENAI_API_KEY"] {
            assert!(ist_geheim(name), "{name} muss als Geheimnis gelten");
        }
        for name in ["PATH", "HOME", "LANG", "CARGO_HOME", "MYL_KISTE", "TMPDIR"] {
            assert!(!ist_geheim(name), "{name} ist kein Geheimnis");
        }
    }

    /// ⛔️ Ohne Grenze laeuft `Eng` nicht, und `Frei` laeuft trotzdem.
    #[test]
    fn ohne_grenze_laeuft_nur_was_frei_ist() {
        let ordner = tempfile::tempdir().expect("Ordner");
        let wurzel = ordner.path().canonicalize().expect("aufloesbar");
        let keine = Grenze::Keine("zur Probe".into());
        let sh = crate::schale::sh().expect("eine Shell");
        for art in [Sandkasten::Eng, Sandkasten::Netz] {
            let f = einrichten_mit(&keine, &sh, "true", &Auftrag { wurzel: &wurzel, art, lesen: &[], schreiben: &[] })
                .expect_err("ohne Grenze darf nichts Eingesperrtes laufen");
            assert!(f.contains("agent.sandbox"), "die Meldung nennt den Schalter: {f}");
        }
        let frei = einrichten_mit(&keine, &sh, "true", &Auftrag { wurzel: &wurzel, art: Sandkasten::Frei, lesen: &[], schreiben: &[] })
            .expect("frei laeuft immer");
        assert_eq!(frei.art, Sandkasten::Frei);
    }

    #[test]
    fn die_lage_nennt_die_grenze_oder_ihr_fehlen() {
        assert!(lage_von(&Grenze::Seatbelt).contains("Seatbelt"));
        assert!(lage_von(&Grenze::Landlock { abi: 6 }).contains("ABI 6"));
        let ohne = lage_von(&Grenze::Keine("zur Probe".into()));
        assert!(ohne.contains("keine Sandbox") && ohne.contains("agent.sandbox"), "{ohne}");
        assert!(ansage(false).starts_with("aus") && ansage(false).contains("agent.sandbox"), "ausgeschaltet muss es dastehen");
    }

    /// Die Nachschau setzt ein angelegtes oder geaendertes `commondir`
    /// zurueck und sagt es; ohne Aenderung schweigt sie. Ohne `.git` beim
    /// Start gibt es keine Wache.
    #[test]
    fn die_nachschau_setzt_commondir_zurueck() {
        let d = tempfile::tempdir().expect("Ordner");
        assert!(Gitwache::neu(d.path()).is_none(), "ohne `.git` nichts zu bewachen");
        std::fs::create_dir(d.path().join(".git")).expect("anlegen");
        let datei = d.path().join(".git/commondir");
        let w = Gitwache::neu(d.path()).expect("Wache");
        assert_eq!(w.nachschauen(), None, "nichts geaendert, nichts gesagt");
        std::fs::write(&datei, "../fremd\n").expect("schreiben");
        assert!(w.nachschauen().is_some_and(|z| z.contains("zurueckgesetzt")));
        assert!(!datei.exists(), "angelegt und nicht entfernt");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/gibt/es/nicht", &datei).expect("Verweis");
            assert!(w.nachschauen().is_some(), "ein Verweis ins Leere zaehlt auch");
            assert!(std::fs::symlink_metadata(&datei).is_err());
        }
        std::fs::write(&datei, "eigen\n").expect("schreiben");
        let w = Gitwache::neu(d.path()).expect("Wache");
        std::fs::write(&datei, "../fremd\n").expect("schreiben");
        assert!(w.nachschauen().is_some());
        assert_eq!(std::fs::read_to_string(&datei).expect("lesbar"), "eigen\n", "der alte Inhalt kommt zurueck");
    }

    #[test]
    fn der_zwischenordner_verschwindet_mit_dem_lauf() {
        let z = zwischenordner().expect("Zwischenordner");
        let pfad = z.0.clone();
        assert!(pfad.is_dir());
        drop(z);
        assert!(!pfad.exists(), "der Zwischenordner blieb liegen");
    }
}

/// **Die Grenze, wirklich angewandt**, ueber denselben Weg wie
/// `run_command` und die Manifest-Werkzeuge.
///
/// ⚑ Jede Probe traegt ihre Gegenprobe: Derselbe Befehl gelingt ohne
/// Sandkasten, sonst bewiese sein Scheitern nichts.
#[cfg(all(test, unix))]
mod angewandt {
    use super::*;
    use crate::werkzeuge::{befehl_im_verzeichnis_mit, Einhaengung, BEFEHL_ZEITGRENZE_S};

    /// Ein Arbeitsordner und ein Ordner daneben, beide aufgeloest.
    struct Gelaende {
        _innen: tempfile::TempDir,
        _aussen: tempfile::TempDir,
        innen: PathBuf,
        aussen: PathBuf,
        ein: Einhaengung,
        frei: Einhaengung,
    }

    fn gelaende() -> Gelaende {
        let i = tempfile::tempdir().expect("Arbeitsordner");
        let a = tempfile::tempdir().expect("Ordner daneben");
        let innen = i.path().canonicalize().expect("aufloesbar");
        let aussen = a.path().canonicalize().expect("aufloesbar");
        std::fs::write(aussen.join("geheim.txt"), "GEHEIMWORT\n").expect("schreiben");
        let ein = Einhaengung::neu(&innen, true).expect("Einhaengung");
        let frei = ein.clone().mit_sandkasten(false);
        Gelaende { _innen: i, _aussen: a, innen, aussen, ein, frei }
    }

    fn lauf(e: &Einhaengung, befehl: &str, art: Sandkasten, lesen: &[PathBuf]) -> String {
        befehl_im_verzeichnis_mit(e, befehl, &[], BEFEHL_ZEITGRENZE_S, art, lesen, &[]).expect("der Befehl startet")
    }

    /// Ohne Grenze gibt es hier nichts zu pruefen; das steht dann da.
    fn grenze_da(probe: &str) -> bool {
        if !vorhanden() {
            eprintln!("⚠️ {probe}: uebersprungen, {}", lage());
        }
        vorhanden()
    }

    /// ⛔️ Auf macOS muss es eine Grenze geben. Fehlt sie (ein Bau, der
    /// selbst eingesperrt ist), soll das jemand lesen und nicht eine gruene
    /// Zeile; `MYL_PROBE_OHNE_SANDKASTEN=1` sagt, dass es bekannt ist.
    ///
    /// ⚑ Unter Linux haengt sie am Kern und an seiner Startzeile (Landlock
    /// muss in `lsm=` stehen); dort sagt die Probe nur, was sie fand.
    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn diese_maschine_hat_eine_grenze() {
        eprintln!("Sandkasten: {}", lage());
        if cfg!(target_os = "linux") || std::env::var_os("MYL_PROBE_OHNE_SANDKASTEN").is_some() {
            return;
        }
        assert!(vorhanden(), "{}", lage());
    }

    #[test]
    fn innen_schreiben_geht_aussen_nicht() {
        if !grenze_da("innen_schreiben_geht_aussen_nicht") {
            return;
        }
        let g = gelaende();
        let ziel = g.aussen.join("draussen.txt");
        let befehl = format!("echo a > drin.txt; echo b > '{}'; echo fertig", ziel.display());
        let aus = lauf(&g.ein, &befehl, Sandkasten::Eng, &[]);
        assert!(aus.contains("fertig"), "{aus}");
        assert_eq!(std::fs::read_to_string(g.innen.join("drin.txt")).expect("innen geschrieben"), "a\n");
        assert!(!ziel.exists(), "der Befehl schrieb aus dem Arbeitsordner hinaus: {aus}");
        // Gegenprobe: Ohne Sandkasten schreibt derselbe Befehl dorthin.
        lauf(&g.frei, &befehl, Sandkasten::Eng, &[]);
        assert!(ziel.exists(), "die Gegenprobe schrieb nicht; die Probe bewiese nichts");
    }

    /// ⚑ **Der Schreibhelfer der Dateiwerkzeuge** (Audit A52): innen exakte
    /// Bytes (auch Nullbytes, mehr als eine Roehre fasst) und fehlende
    /// Ordner; nach draussen laesst ihn das Betriebssystem nicht, auch wenn
    /// der Pfad jede Pruefung im Prozess umgeht (direkt am Helfer und ueber
    /// einen Verweis im Arbeitsordner). Gegenprobe: Ohne Sandbox schreibt
    /// derselbe Befehl ueber den Verweis hinaus.
    #[test]
    fn der_schreibhelfer_schreibt_innen_und_nicht_draussen() {
        if !grenze_da("der_schreibhelfer_schreibt_innen_und_nicht_draussen") {
            return;
        }
        let g = gelaende();
        let inhalt: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let ziel = g.innen.join("neu/tief/datei.bin");
        datei_schreiben(&g.innen, &ziel, &inhalt).expect("Grenze da").expect("innen geschrieben");
        assert_eq!(std::fs::read(&ziel).expect("lesbar"), inhalt);
        datei_schreiben(&g.innen, &ziel, b"kurz").expect("Grenze da").expect("ueberschrieben");
        assert_eq!(std::fs::read(&ziel).expect("lesbar"), b"kurz", "ueberschreiben kuerzt");
        let draussen = g.aussen.join("draussen.txt");
        let f = datei_schreiben(&g.innen, &draussen, b"boese").expect("Grenze da").expect_err("draussen geschrieben");
        assert!(f.to_string().contains("Sandbox"), "{f}");
        assert!(!draussen.exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&g.aussen, g.innen.join("raus")).expect("Verweis");
            let ueber = g.innen.join("raus/ueber.txt");
            assert!(datei_schreiben(&g.innen, &ueber, b"boese").expect("Grenze da").is_err());
            assert!(!g.aussen.join("ueber.txt").exists(), "der Helfer schrieb ueber den Verweis hinaus");
            let befehl = r#"mkdir -p -- "$(dirname -- "$MYL_ZIEL")" && exec cat > "$MYL_ZIEL""#;
            let frei = befehl_im_verzeichnis_mit(&g.frei, befehl, &[("MYL_ZIEL", ueber.display().to_string())], 10, Sandkasten::Frei, &[], &[]);
            assert!(frei.is_ok() && g.aussen.join("ueber.txt").exists(), "Gegenprobe: ohne Sandbox schreibt derselbe Befehl hinaus");
        }
        // Und die Dateiwerkzeuge gehen mit Sandbox diesen Weg.
        g.ein.schreiben(&g.innen.join("werkzeug.txt"), b"ok").expect("innen ueber die Einhaengung");
        assert_eq!(std::fs::read(g.innen.join("werkzeug.txt")).expect("lesbar"), b"ok");
    }

    #[test]
    fn aussen_lesen_geht_nicht() {
        if !grenze_da("aussen_lesen_geht_nicht") {
            return;
        }
        let g = gelaende();
        let befehl = format!("cat '{0}/geheim.txt'; ls '{0}'; echo fertig", g.aussen.display());
        let aus = lauf(&g.ein, &befehl, Sandkasten::Eng, &[]);
        assert!(aus.contains("fertig"), "{aus}");
        assert!(!aus.contains("GEHEIMWORT") && !aus.contains("geheim.txt\n"), "der Befehl las hinaus: {aus}");
        let gegen = lauf(&g.frei, &befehl, Sandkasten::Eng, &[]);
        assert!(gegen.contains("GEHEIMWORT") && gegen.contains("geheim.txt\n"), "Gegenprobe: {gegen}");
    }

    /// Der Ordner einer Kiste ist lesbar, wenn der Auftrag ihn nennt, und
    /// auch dann nicht beschreibbar.
    #[test]
    fn was_der_auftrag_nennt_ist_lesbar_und_nicht_beschreibbar() {
        if !grenze_da("was_der_auftrag_nennt_ist_lesbar_und_nicht_beschreibbar") {
            return;
        }
        let g = gelaende();
        let befehl = format!("cat '{0}/geheim.txt'; echo x > '{0}/neu.txt'; echo fertig", g.aussen.display());
        let aus = lauf(&g.ein, &befehl, Sandkasten::Eng, std::slice::from_ref(&g.aussen));
        assert!(aus.contains("GEHEIMWORT"), "der genannte Ordner war nicht lesbar: {aus}");
        assert!(!g.aussen.join("neu.txt").exists(), "der genannte Ordner war beschreibbar: {aus}");
    }

    /// Ein Ausgabeordner, den der Auftrag nennt, ist beschreibbar; einer in
    /// der Kiste nicht.
    #[test]
    fn ein_genannter_ausgabeordner_ist_beschreibbar_und_die_kiste_bleibt_zu() {
        if !grenze_da("ein_genannter_ausgabeordner_ist_beschreibbar_und_die_kiste_bleibt_zu") {
            return;
        }
        let g = gelaende();
        let kiste = g.aussen.join("kiste");
        let vorschau = g.aussen.join("vorschau");
        let in_kiste = kiste.join("innen");
        for o in [&kiste, &vorschau, &in_kiste] {
            std::fs::create_dir_all(o).expect("anlegen");
        }
        let auftrag = Auftrag {
            wurzel: &g.innen,
            art: Sandkasten::Eng,
            lesen: std::slice::from_ref(&kiste),
            schreiben: &[vorschau.clone(), in_kiste.clone(), g.aussen.clone()],
        };
        let sh = crate::schale::sh().expect("eine Shell");
        let befehl = format!(
            "echo v > '{}/a.stl'; echo k > '{}/b'; echo d > '{}/c'; echo fertig",
            vorschau.display(),
            in_kiste.display(),
            g.aussen.display()
        );
        let mut l = einrichten(&sh, &befehl, &auftrag).expect("eingerichtet");
        l.kind.current_dir(&g.innen);
        let aus = myl_senses::prozess::laufen(&mut l.kind, 30, 4096).expect("laeuft");
        assert!(aus.aus.contains("fertig"), "{}{}", aus.aus, aus.fehler);
        assert!(vorschau.join("a.stl").exists(), "der genannte Ausgabeordner war zu: {}", aus.fehler);
        assert!(!in_kiste.join("b").exists(), "ein Ausgabeordner in der Kiste galt");
        assert!(!g.aussen.join("c").exists(), "ein Ausgabeordner ueber der Kiste galt");
    }

    /// ⚑ Unter macOS ueber das Profil, unter Linux ueber den eigenen
    /// Namensraum (seit 2026-11-17; Landlock allein nimmt aus einem
    /// beschreibbaren Baum nichts heraus) und die Nachschau fuer
    /// `commondir`.
    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn im_arbeitsordner_bleiben_haken_und_kiste_zu() {
        if !grenze_da("im_arbeitsordner_bleiben_haken_und_kiste_zu") {
            return;
        }
        #[cfg(target_os = "linux")]
        if !super::linux::namensraum() {
            eprintln!("⚠️ im_arbeitsordner_bleiben_haken_und_kiste_zu: uebersprungen, {}", lage());
            return;
        }
        let g = gelaende();
        std::fs::create_dir_all(g.innen.join(".git/hooks")).expect("anlegen");
        std::fs::create_dir_all(g.innen.join("kiste")).expect("anlegen");
        std::fs::write(g.innen.join(".git/config"), "[core]\n").expect("schreiben");
        let befehl = "echo x > .git/hooks/pre-commit; echo '[alias]' >> .git/config; echo x > kiste/w.json; \
                      echo ../fremd > .git/commondir; mv .git weg; echo x > .git/frei.txt; echo fertig";
        let kiste = g.innen.join("kiste");
        let aus = lauf(&g.ein, befehl, Sandkasten::Eng, std::slice::from_ref(&kiste));
        assert!(aus.contains("fertig"), "{aus}");
        assert!(!g.innen.join(".git/hooks/pre-commit").exists(), "ein Haken liess sich legen: {aus}");
        assert_eq!(std::fs::read_to_string(g.innen.join(".git/config")).expect("lesbar"), "[core]\n", "{aus}");
        assert!(!kiste.join("w.json").exists(), "die Kiste liess sich beschreiben: {aus}");
        assert!(!g.innen.join(".git/commondir").exists(), "der gemeinsame Ordner liess sich umbiegen: {aus}");
        assert!(!g.innen.join("weg").exists(), "`.git` liess sich wegschieben: {aus}");
        assert!(g.innen.join(".git/frei.txt").exists(), "`.git` muss sonst beschreibbar bleiben, sonst geht kein `git add`");
        lauf(&g.frei, befehl, Sandkasten::Eng, std::slice::from_ref(&kiste));
        assert!(g.innen.join("weg/hooks/pre-commit").exists() && kiste.join("w.json").exists(), "Gegenprobe");
    }

    #[test]
    fn der_eigene_zwischenordner_traegt_und_verschwindet() {
        if !grenze_da("der_eigene_zwischenordner_traegt_und_verschwindet") {
            return;
        }
        let g = gelaende();
        let aus = lauf(&g.ein, "echo ZWISCHENWORT > \"$TMPDIR/a\" && cat \"$TMPDIR/a\" && echo \"ort=$TMPDIR\"", Sandkasten::Eng, &[]);
        assert!(aus.contains("ZWISCHENWORT"), "{aus}");
        let ort = aus.lines().find_map(|z| z.strip_prefix("ort=")).expect("der Ort steht in der Ausgabe");
        assert!(ort.contains("myl-sandkasten-"), "{ort}");
        assert!(!Path::new(ort).exists(), "der Zwischenordner blieb liegen: {ort}");
    }

    /// Ein Lauscher in dieser Probe, ein `curl` im Befehl: eng kommt nicht
    /// durch, mit Netz und ohne Sandkasten schon.
    #[test]
    fn das_netz_ist_zu_und_mit_netz_offen() {
        if !grenze_da("das_netz_ist_zu_und_mit_netz_offen") {
            return;
        }
        use std::io::{Read, Write};
        let lauscher = std::net::TcpListener::bind("127.0.0.1:0").expect("Lauscher");
        let port = lauscher.local_addr().expect("Adresse").port();
        std::thread::spawn(move || {
            for strom in lauscher.incoming() {
                let Ok(mut strom) = strom else { continue };
                let mut puffer = [0u8; 1024];
                let _ = strom.read(&mut puffer);
                let _ = strom.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNETZWORT\n");
            }
        });
        let g = gelaende();
        let befehl = format!("curl -s -m 5 http://127.0.0.1:{port}/; echo fertig");
        let gegen = lauf(&g.frei, &befehl, Sandkasten::Eng, &[]);
        assert!(gegen.contains("NETZWORT"), "Gegenprobe (braucht `curl`): {gegen}");
        let eng = lauf(&g.ein, &befehl, Sandkasten::Eng, &[]);
        assert!(eng.contains("fertig") && !eng.contains("NETZWORT"), "eng erreichte das Netz: {eng}");
        let netz = lauf(&g.ein, &befehl, Sandkasten::Netz, &[]);
        assert!(netz.contains("NETZWORT"), "mit Netz kam nichts an: {netz}");
        // Auch mit Netz bleibt der Rest der Grenze.
        let ziel = g.aussen.join("draussen.txt");
        lauf(&g.ein, &format!("echo b > '{}'", ziel.display()), Sandkasten::Netz, &[]);
        assert!(!ziel.exists(), "mit Netz schrieb der Befehl hinaus");
    }

    /// Ein Prozess ausserhalb bekommt kein Signal. ⚑ Unter Linux erst ab
    /// Landlock ABI 6.
    #[test]
    fn ein_prozess_ausserhalb_bekommt_kein_signal() {
        if !grenze_da("ein_prozess_ausserhalb_bekommt_kein_signal") {
            return;
        }
        if let Grenze::Landlock { abi } = grenze() {
            if *abi < 6 {
                eprintln!("⚠️ Landlock ABI {abi}: Signale nach aussen sind erst ab 6 gesperrt");
                return;
            }
        }
        let g = gelaende();
        let befehl = format!("kill -0 {} 2>/dev/null && echo ERREICHT || echo VERWEHRT", std::process::id());
        assert!(lauf(&g.ein, &befehl, Sandkasten::Eng, &[]).contains("VERWEHRT"));
        assert!(lauf(&g.frei, &befehl, Sandkasten::Eng, &[]).contains("ERREICHT"), "Gegenprobe");
    }

    #[test]
    fn ein_gescheiterter_befehl_nennt_die_grenze_und_ein_gelungener_nicht() {
        if !grenze_da("ein_gescheiterter_befehl_nennt_die_grenze_und_ein_gelungener_nicht") {
            return;
        }
        let g = gelaende();
        assert!(lauf(&g.ein, "exit 3", Sandkasten::Eng, &[]).contains("in der Sandbox"));
        assert!(!lauf(&g.ein, "true", Sandkasten::Eng, &[]).contains("in der Sandbox"));
        assert!(!lauf(&g.frei, "exit 3", Sandkasten::Eng, &[]).contains("in der Sandbox"));
        assert!(!lauf(&g.ein, "exit 3", Sandkasten::Frei, &[]).contains("in der Sandbox"));
    }

    /// In einem Repositorium, das schon da ist, gehen `git add` und
    /// `git commit` trotz der geschuetzten Haken.
    #[test]
    fn in_einem_vorhandenen_repositorium_geht_der_commit() {
        if !grenze_da("in_einem_vorhandenen_repositorium_geht_der_commit") {
            return;
        }
        let g = gelaende();
        let vorher = lauf(&g.frei, "git init -q . && echo ende=$?", Sandkasten::Frei, &[]);
        if !vorher.contains("ende=0") {
            eprintln!("⚠️ ohne `git` nichts zu pruefen: {vorher}");
            return;
        }
        let befehl = "echo eins > a.txt && git add a.txt && \
                      git -c user.name=Probe -c user.email=probe@example.org commit -q -m eins && \
                      git log --oneline | wc -l; echo ende=$?";
        let aus = lauf(&g.ein, befehl, Sandkasten::Eng, &[]);
        assert!(aus.contains("ende=0") && aus.contains('1'), "{aus}");
    }

    /// Eine Variable, die nach einem Geheimnis heisst, kommt nicht an; was
    /// der Aufrufer selbst setzt, schon.
    #[test]
    fn geheimnisse_der_umgebung_kommen_nicht_an() {
        if !grenze_da("geheimnisse_der_umgebung_kommen_nicht_an") {
            return;
        }
        let g = gelaende();
        let lesen = Auftrag { wurzel: &g.innen, art: Sandkasten::Eng, lesen: &[], schreiben: &[] };
        let sh = crate::schale::sh().expect("eine Shell");
        // ⚑ Nicht ueber `set_var`: Die Proben laufen nebeneinander, und
        // eine Umgebung, die sich unter ihnen aendert, gehoert keiner.
        // Geprueft wird, was der Befehl erbt, ueber `env_remove` am Lauf.
        let lauf = einrichten(&sh, "env", &lesen).expect("eingerichtet");
        let entfernt: Vec<String> = lauf
            .kind
            .get_envs()
            .filter(|(_, wert)| wert.is_none())
            .filter_map(|(name, _)| name.to_str().map(str::to_string))
            .collect();
        for (name, _) in std::env::vars() {
            assert_eq!(entfernt.contains(&name), ist_geheim(&name), "{name}");
        }
        let aus = befehl_im_verzeichnis_mit(
            &g.ein,
            "echo \"wert=$MYL_PROBE_TOKEN\"",
            &[("MYL_PROBE_TOKEN", "vom Aufrufer".to_string())],
            BEFEHL_ZEITGRENZE_S,
            Sandkasten::Eng,
            &[],
            &[],
        )
        .expect("laeuft");
        assert!(aus.contains("wert=vom Aufrufer"), "was der Aufrufer setzt, bleibt: {aus}");
    }

    /// Gewoehnliche Arbeit geht: `git` im Arbeitsordner, eine Roehre, ein
    /// Unterordner.
    #[test]
    fn gewoehnliche_arbeit_im_arbeitsordner_geht() {
        if !grenze_da("gewoehnliche_arbeit_im_arbeitsordner_geht") {
            return;
        }
        let g = gelaende();
        let befehl = "mkdir -p a/b && echo eins > a/b/c.txt && cat a/b/c.txt | wc -l && ls a/b && \
                      if command -v git >/dev/null; then git init -q . && git add -A && git status --short; fi; echo ende=$?";
        let aus = lauf(&g.ein, befehl, Sandkasten::Eng, &[]);
        assert!(aus.contains("c.txt") && aus.contains("ende=0"), "{aus}");
        assert!(!aus.contains("xcrun"), "die Weichen von macOS meldeten sich: {aus}");
    }
}
