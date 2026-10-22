//! Nachsehen, ob es einen neueren Stand gibt, und ihn einspielen.
//!
//! # ⚑ Was hier verglichen wird: die Fassung in `VERSION.md`
//!
//! ⚑ **Festlegung des Projektinhabers, 2026-10-17.** In der Wurzel liegt
//! `VERSION.md`; ihre erste Zeile ist die Fassung von Myelith, darunter steht,
//! was sich mit dieser Fassung geaendert hat. Verglichen wird die Fassung, mit der
//! **dieses Programm gebaut wurde** ([`EINGEBAUT`]), mit der ersten Zeile
//! derselben Datei auf GitHub. Ist die dort hoeher, gibt es ein Update; ist
//! sie niedriger oder gleich, nicht. Ein Klon, der seinem `origin` voraus
//! ist, wird also nie zurueckgestellt.
//!
//! ⚑ **Die Datei ist kein Changelog** (Festlegung des Projektinhabers, am
//! selben Tag): Sie traegt nur den Abschnitt der aktuellen Fassung, der
//! vorige wird beim Heben ersetzt. Wer von 0.1.0 auf 0.1.2 springt, sieht
//! also, was 0.1.2 bringt, nicht die Summe. Der Leser unten kann trotzdem
//! mehrere Abschnitte, damit eine aeltere Datei auf GitHub nicht zum Fehler
//! wird.
//!
//! 📌 **Bis CLIENT v0.167.0 fragte es, ob `origin` Commits hat, die der Klon
//! nicht hat** (`git fetch` und `git rev-list`), und daneben nach der
//! neuesten Freigabemarke. Die Marken gehoerten zum Projekt, die Kisten
//! zaehlten jede fuer sich, und eine Nummer, die ein Mensch liest, gab es
//! nicht. Dazu brauchte `git fetch` die Anmeldung beim entfernten Baum; beim
//! Start des Fensters von selbst gefragt, waere das eine Passwortfrage ohne
//! Anlass.
//!
//! ⚑ **Gelesen wird die Datei ohne Anmeldung**, ueber die Rohansicht von
//! GitHub. Eingespielt wird weiter mit dem Installationsskript der
//! Plattform (`--aktualisieren`): `git fetch`, `git merge --ff-only`, Bau,
//! Installation.
//!
//! # ⚑ Und es gibt keinen HTTP-Klienten dafuer
//!
//! Dieselbe Entscheidung wie beim Holen der Modelle, das `hf` aufruft:
//! **`git` und `curl` liegen auf jeder Maschine, auf der dieses
//! Programm gebaut werden kann.** Eine eigene Kiste dafuer waere ein
//! Netzstapel im Klienten, den niemand angemeldet hat.
//!
//! # ⚠️ Was das Einspielen tut, ausgeschrieben
//!
//! Das Skript bewegt den Klon mit `git merge --ff-only` vorwaerts und baut
//! danach. **Das ist Ausfuehren von Code, der gerade geholt wurde**, und das
//! gehoert benannt. Drei Dinge begrenzen es: Es kommt aus dem Klon des
//! Nutzers und nicht aus einem Netzpfad, `git` traegt die Unversehrtheit der
//! Uebertragung, und `--ff-only` schliesst aus, dass dabei eigene Arbeit
//! verschwindet. ⚑ **Und es geschieht nur nach einer Bestaetigung**: Konsole
//! und Fenster zeigen vorher Fassung und Aenderungen und fragen.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;

/// Das Repositorium, gegen das gefragt wird.
///
/// ⚑ **Er steht hier und nicht in den Einstellungen.** Ein Klient, der
/// seine Aktualisierungsquelle einstellbar macht, hat ein Feld, in das
/// jemand etwas anderes schreiben kann; von dort kommt danach Code.
pub const SEITE: &str = "https://github.com/crowdedconfusion/Myelith";

/// Der Name der Versionsdatei in der Wurzel des Repositoriums.
pub const VERSIONSDATEI: &str = "VERSION.md";

/// Der Zweig, dessen Versionsdatei gilt.
pub const ZWEIG: &str = "main";

/// **Die Versionsdatei, mit der dieses Programm gebaut wurde.**
///
/// ⚑ **Eingebaut und nicht zur Laufzeit gelesen.** Gefragt ist, was
/// **laeuft**: Wer den Klon von Hand vorwaertsbewegt und nicht neu
/// installiert, hat im Klon die neue Datei und im Programm den alten Stand.
/// Nur die eingebaute Fassung sagt, ob ein Update noetig ist. Der Pfad steht
/// relativ zur Kiste; `include_str!` ist die Ausnahme von der Regel gegen
/// feste Pfade, die eine Konstante zur Bauzeit braucht.
pub const EINGEBAUT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../VERSION.md"));

/// **Eine Fassung, `haupt.neben.flick`**, verglichen Stelle fuer Stelle.
///
/// ⚑ **Drei Zahlen und kein Text.** Als Zeichenkette verglichen kaeme
/// `0.10.0` vor `0.9.0`; genau diese Falle hat die Modellwahl schon einmal
/// gehabt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fassung(pub u32, pub u32, pub u32);

impl Fassung {
    /// Liest `0.1.0` oder `v0.1.0`; alles andere ist `None`.
    pub fn lesen(text: &str) -> Option<Self> {
        let t = text.trim();
        let t = t.strip_prefix('v').unwrap_or(t);
        let mut teile = t.split('.');
        let zahl = |x: Option<&str>| x.and_then(|z| z.parse::<u32>().ok());
        let f = Fassung(zahl(teile.next())?, zahl(teile.next())?, zahl(teile.next())?);
        teile.next().is_none().then_some(f)
    }
}

impl std::fmt::Display for Fassung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Ein Abschnitt der Versionsdatei: eine Fassung und was sie geaendert hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Eintrag {
    /// Die Fassung aus der Ueberschrift.
    pub fassung: String,
    /// Die ganze Ueberschrift, ohne `## `.
    pub titel: String,
    /// Was unter der Ueberschrift steht, ohne Leerzeilen am Rand.
    pub text: String,
}

/// **Die gelesene Versionsdatei.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versionsdatei {
    /// Die Fassung aus der ersten Zeile.
    pub fassung: Fassung,
    /// Die Abschnitte, in der Reihenfolge der Datei (neueste oben).
    pub eintraege: Vec<Eintrag>,
}

impl Versionsdatei {
    /// Liest die Datei: erste Zeile die Fassung, danach die Abschnitte mit
    /// `## <fassung> …` als Ueberschrift.
    ///
    /// ⚑ **Die erste Zeile ist die ganze Zusage.** Steht dort keine Fassung,
    /// ist die Datei unbrauchbar, und das ist ein Fehler und kein „aktuell“.
    pub fn lesen(text: &str) -> Result<Self, String> {
        let erste = text.lines().next().unwrap_or("").trim();
        let fassung = Fassung::lesen(erste)
            .ok_or_else(|| format!("die erste Zeile von {VERSIONSDATEI} ist keine Fassung: {erste:?}"))?;
        let mut eintraege: Vec<Eintrag> = Vec::new();
        for zeile in text.lines().skip(1) {
            if let Some(titel) = zeile.strip_prefix("## ") {
                let wort = titel.split_whitespace().next().unwrap_or("");
                if let Some(f) = Fassung::lesen(wort) {
                    eintraege.push(Eintrag { fassung: f.to_string(), titel: titel.trim().to_string(), text: String::new() });
                    continue;
                }
            }
            if let Some(e) = eintraege.last_mut() {
                e.text.push_str(zeile);
                e.text.push('\n');
            }
        }
        for e in &mut eintraege {
            e.text = e.text.trim().to_string();
        }
        Ok(Self { fassung, eintraege })
    }

    /// Die Abschnitte, die neuer sind als `hier`, neueste zuerst.
    pub fn neuer_als(&self, hier: Fassung) -> Vec<Eintrag> {
        self.eintraege
            .iter()
            .filter(|e| Fassung::lesen(&e.fassung).is_some_and(|f| f > hier))
            .cloned()
            .collect()
    }
}

/// Die Fassung, mit der dieses Programm gebaut wurde.
pub fn eingebaute_fassung() -> Fassung {
    Versionsdatei::lesen(EINGEBAUT).map(|v| v.fassung).unwrap_or(Fassung(0, 0, 0))
}

/// Was das Nachsehen ergeben hat.
#[derive(Debug, Clone, Serialize)]
pub struct Stand {
    /// Das Quellverzeichnis, wenn dieses Programm aus einem Klon laeuft.
    pub quelle: Option<String>,
    /// Die Fassung dieses Programms, aus der eingebauten `VERSION.md`.
    pub eigene: String,
    /// Die Fassung auf GitHub, wenn sie sich lesen liess.
    pub neueste: Option<String>,
    /// Ob die Fassung auf GitHub hoeher ist als die eigene.
    pub neuer: bool,
    /// Was die neueren Fassungen geaendert haben, neueste zuerst.
    pub aenderungen: Vec<Eintrag>,
    /// Die Seite des Repositoriums.
    pub seite: String,
    /// Warum nichts einzuspielen ist oder warum nicht nachgesehen werden
    /// konnte; `None`, wenn alles beantwortet ist.
    pub grund: Option<String>,
}

impl Stand {
    /// Gibt es etwas einzuspielen?
    pub fn lohnt(&self) -> bool {
        self.quelle.is_some() && self.neuer
    }

    /// Der Stand aus der eigenen Datei und der von GitHub, ohne Netz.
    pub fn aus(quelle: Option<PathBuf>, eigene: &str, dort: Result<String, String>) -> Self {
        let hier = Versionsdatei::lesen(eigene).map(|v| v.fassung).unwrap_or(Fassung(0, 0, 0));
        let mut s = Stand {
            quelle: quelle.as_ref().map(|q| q.display().to_string()),
            eigene: hier.to_string(),
            neueste: None,
            neuer: false,
            aenderungen: Vec::new(),
            seite: SEITE.to_string(),
            grund: None,
        };
        match dort.and_then(|t| Versionsdatei::lesen(&t)) {
            Ok(v) => {
                s.neueste = Some(v.fassung.to_string());
                s.neuer = v.fassung > hier;
                if s.neuer {
                    s.aenderungen = v.neuer_als(hier);
                }
            }
            Err(f) => s.grund = Some(f),
        }
        if s.neuer && quelle.is_none() {
            s.grund = Some(
                "Dieses Programm laeuft nicht aus einem Klon des Repositoriums. \
                 Ein neuer Stand laesst sich deshalb nicht von hier aus einspielen; \
                 die Seite des Repositoriums fuehrt zum neuen Stand."
                    .to_string(),
            );
        }
        s
    }
}

/// **Das Quellverzeichnis, von der laufenden Datei aus nach oben
/// gesucht.**
///
/// ⚑ **Zwei Merkmale und nicht eines.** Ein `.git` allein waere jeder
/// beliebige Klon; die Marke dieses Repositoriums macht es zu
/// **diesem**.
///
/// 📌 **Bis zum 2026-09-10 suchte es nur vom Programm aufwaerts** und
/// fand deshalb nichts, sobald das Programm installiert war. Seither
/// fragt es `ort::wurzel`, und die traegt einen gemerkten Ort: Wer
/// einmal aus dem Klon heraus gestartet hat, kann danach auch aus
/// `~/.local/bin` heraus aktualisieren.
pub fn quelle() -> Option<PathBuf> {
    let w = myl_local_agent::ort::wurzel()?;
    // ⚑ **Ein Baum ist noch kein Klon.** Wer ein Freigabearchiv
    // entpackt, hat die Marke und kein `.git`; `git merge` haette dort
    // nichts, was es vorwaertsbewegen koennte.
    w.join(".git").exists().then_some(w)
}

/// Die Adresse der Versionsdatei in der Rohansicht von GitHub.
pub fn adresse_dort() -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{ZWEIG}/{VERSIONSDATEI}",
        SEITE.trim_start_matches("https://github.com/")
    )
}

/// Holt die Versionsdatei von GitHub.
///
/// ⚑ **Ein Fehlschlag ist kein „aktuell“.** Ohne Netz ist die Frage
/// unbeantwortet, und das steht im Grund; ein Fenster, das beim Start von
/// selbst nachsieht, zeigt dann schlicht nichts an.
fn datei_dort() -> Result<String, String> {
    let aus = Command::new("curl")
        .args(["--silent", "--show-error", "--fail", "--max-time", "10", "--header", "User-Agent: myelith-client"])
        .arg(adresse_dort())
        .output()
        .map_err(|f| format!("curl liess sich nicht aufrufen: {f}"))?;
    if !aus.status.success() {
        let fehler = String::from_utf8_lossy(&aus.stderr).trim().to_string();
        // ⚑ **Eine fehlende Datei sagt es selbst**: So steht es da, solange
        //   `VERSION.md` noch nicht auf `main` liegt, und ein nackter
        //   curl-Fehler liesse an eine kaputte Leitung denken.
        if fehler.contains("404") {
            return Err(format!("auf GitHub liegt noch keine {VERSIONSDATEI} (Zweig {ZWEIG})"));
        }
        return Err(format!("GitHub war nicht zu erreichen ({})", if fehler.is_empty() { "ohne Grund" } else { &fehler }));
    }
    Ok(String::from_utf8_lossy(&aus.stdout).to_string())
}

/// Sieht nach, ohne etwas zu aendern.
pub fn pruefen() -> Stand {
    Stand::aus(quelle(), EINGEBAUT, datei_dort())
}

/// Die Installationsskripte, **immer alle uebersetzt**.
///
/// ⛔️ **Fund 458 (2026-09-24): der Windows-Pfad blieb beim Umzug nach
/// `SYSTEM/` stehen, und keine oertliche Probe konnte es sehen.** Er
/// stand mit Backslash (`INSTALL\\installieren-windows.ps1`), also traf
/// ihn die Ersetzung von `"INSTALL/` nicht, und er lag hinter
/// `cfg(target_os = "windows")`, also uebersetzte ein macOS-Lauf ihn
/// gar nicht erst. Gefallen ist es auf dem Windows-Laeufer der CI.
///
/// 📌 **Ein `cfg`-Zweig ist Code, den die eigene Maschine nicht
/// prueft.** Dagegen hilft keine Sorgfalt, sondern nur, ihn aus dem
/// `cfg` herauszuholen: **Die drei Namen stehen jetzt hier und werden
/// auf jeder Plattform uebersetzt**, `cfg` waehlt nur noch aus. Damit
/// faellt `alle_skripte_liegen_da` auf **jedem** System, sobald
/// eines davon nicht mehr stimmt.
pub const SKRIPT_MACOS: &str = "SYSTEM/install/installieren-macos.sh";
/// Siehe [`SKRIPT_MACOS`].
///
/// ⛔️ **Fund 581 (2026-10-21): Hier stand das Nix-Skript.** Unter jedem
/// Linux ohne Nix brach das Update damit ab (`nix fehlt`), also auf
/// Debian, Ubuntu, Fedora und Arch. `installieren-linux.sh` ist der
/// Einstieg fuer jedes Linux und gibt auf NixOS selbst an das Nix-Skript
/// ab; die Entscheidung faellt dort, wo das System zu sehen ist.
pub const SKRIPT_LINUX: &str = "SYSTEM/install/installieren-linux.sh";
/// Das Skript fuer NixOS, aufgerufen von [`SKRIPT_LINUX`].
pub const SKRIPT_NIXOS: &str = "SYSTEM/install/installieren-nixos.sh";
/// Siehe [`SKRIPT_MACOS`].
///
/// ⚑ **Der Backslash bleibt.** So stand er hier seit jeher, und ob
/// PowerShell den Schraegstrich genauso nimmt, ist auf dieser Maschine
/// nicht zu pruefen. **Eine Verhaltensaenderung, die niemand nachsehen
/// kann, gehoert nicht in eine Fehlerbehebung.**
pub const SKRIPT_WINDOWS: &str = "SYSTEM\\install\\installieren-windows.ps1";

/// Alle, fuer die Proben und fuer jeden, der sie aufzaehlen will.
pub const SKRIPTE: [&str; 4] = [SKRIPT_MACOS, SKRIPT_LINUX, SKRIPT_NIXOS, SKRIPT_WINDOWS];

/// Das Installationsskript dieser Plattform.
///
/// ⚑ **Der Name steht hier und wird nicht geraten.** Drei Plattformen,
/// drei Skripte, und `cfg` entscheidet zur Bauzeit: Ein Programm, das
/// zur Laufzeit nach dem Betriebssystem fragt, kann sich irren.
pub fn skript() -> Option<(&'static str, &'static [&'static str])> {
    #[cfg(target_os = "macos")]
    {
        Some(("sh", &[SKRIPT_MACOS, "--aktualisieren"]))
    }
    #[cfg(target_os = "linux")]
    {
        Some(("sh", &[SKRIPT_LINUX, "--aktualisieren"]))
    }
    #[cfg(target_os = "windows")]
    {
        Some((
            "powershell",
            &[
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                SKRIPT_WINDOWS,
                "-Aktualisieren",
            ],
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Spielt den neuen Stand ein und meldet jede Zeile, waehrend sie faellt.
///
/// ⚠️ **Es baut neu, und das dauert Minuten.** Der Melder ist deshalb
/// keine Bequemlichkeit: Ein Fortschritt, den niemand sieht, ist von
/// einem Stillstand nicht zu unterscheiden.
///
/// 📌 **Bis CLIENT v0.167.0 kamen die Zeilen erst am Ende**, alle auf
/// einmal (`output()`), und waehrend der Minuten des Baus stand nichts da.
/// Jetzt laufen `stdout` und `stderr` je in einem Faden mit und gehen
/// sofort weiter.
pub fn einspielen(melder: &dyn Fn(&str)) -> Result<(), String> {
    let wurzel = quelle().ok_or_else(|| {
        "Dieses Programm laeuft nicht aus einem Klon; es gibt nichts einzuspielen.".to_string()
    })?;
    let (was, argumente) = skript().ok_or_else(|| {
        "Fuer dieses Betriebssystem gibt es kein Installationsskript.".to_string()
    })?;

    melder(&format!("{was} {}", argumente.join(" ")));
    let mut kind = Command::new(was)
        .args(argumente)
        .current_dir(&wurzel)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|f| format!("{was} liess sich nicht aufrufen: {f}"))?;
    let (sender, empfaenger) = std::sync::mpsc::channel::<(bool, String)>();
    let mut faeden = Vec::new();
    if let Some(aus) = kind.stdout.take() {
        let s = sender.clone();
        faeden.push(std::thread::spawn(move || {
            for z in BufReader::new(aus).lines().map_while(Result::ok) {
                let _ = s.send((false, z));
            }
        }));
    }
    if let Some(err) = kind.stderr.take() {
        let s = sender.clone();
        faeden.push(std::thread::spawn(move || {
            for z in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = s.send((true, z));
            }
        }));
    }
    drop(sender);
    let mut fehlerzeilen = Vec::new();
    for (fehler, zeile) in empfaenger {
        melder(&zeile);
        if fehler {
            fehlerzeilen.push(zeile);
        }
    }
    for f in faeden {
        let _ = f.join();
    }
    let status = kind.wait().map_err(|f| format!("{was} lief nicht zu Ende: {f}"))?;
    if status.success() {
        melder("Fertig. Der neue Stand ist installiert.");
        return Ok(());
    }
    let fehler = fehlerzeilen.join("\n").trim().to_string();
    Err(if fehler.is_empty() { "das Installationsskript ging nicht durch".into() } else { fehler })
}

/// **Das Programm, das neu gestartet werden soll**: die laufende Datei.
///
/// ⚑ **Dieselbe Stelle, an die das Skript den neuen Stand gelegt hat.** Es
/// loescht die alte Datei, bevor es die neue hinlegt; ein laufendes
/// Programm behaelt seine, und der Neustart findet unter demselben Pfad die
/// neue. 📌 Unter Linux nennt `/proc/self/exe` eine geloeschte Datei mit dem
/// Zusatz ` (deleted)`; der faellt hier weg.
pub fn laufende_datei() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    let text = p.to_string_lossy();
    Some(match text.strip_suffix(" (deleted)") {
        Some(rest) => PathBuf::from(rest),
        None => p,
    })
}

#[cfg(test)]
mod proben {
    use super::*;
    use std::path::Path;

    const DATEI: &str = "0.3.0\n\n# Kopf\n\nText.\n\n## 0.3.0 (2026-10-20)\n\n- drei\n\n## 0.2.0\n\n- zwei\n\n## 0.1.0 (2026-10-17)\n\n- eins\n";

    /// ⚑ **Die erste Zeile ist die Fassung, die Abschnitte stehen darunter.**
    #[test]
    fn die_versionsdatei_wird_gelesen() {
        let v = Versionsdatei::lesen(DATEI).expect("lesbar");
        assert_eq!(v.fassung, Fassung(0, 3, 0));
        assert_eq!(v.eintraege.len(), 3);
        assert_eq!(v.eintraege[0].titel, "0.3.0 (2026-10-20)");
        assert_eq!(v.eintraege[1].text, "- zwei");
        let neu: Vec<String> = v.neuer_als(Fassung(0, 1, 0)).into_iter().map(|e| e.fassung).collect();
        assert_eq!(neu, ["0.3.0", "0.2.0"], "gezeigt wird alles nach der eigenen Fassung, neueste zuerst");
        assert!(Versionsdatei::lesen("# keine Fassung\n").is_err());
        assert!(Versionsdatei::lesen("").is_err());
    }

    /// 📌 **Verglichen wird Zahl fuer Zahl**, nicht als Text: `0.10.0` ist
    /// neuer als `0.9.0`.
    #[test]
    fn fassungen_werden_als_zahlen_verglichen() {
        assert!(Fassung::lesen("0.10.0") > Fassung::lesen("0.9.0"));
        assert_eq!(Fassung::lesen("v1.2.3"), Some(Fassung(1, 2, 3)));
        for falsch in ["", "1.2", "1.2.3.4", "a.b.c", "0.1.0-pre"] {
            assert_eq!(Fassung::lesen(falsch), None, "{falsch}");
        }
    }

    /// ⚑ **Ein Update nur, wenn GitHub hoeher ist**; ein Klon, der voraus
    /// ist, wird nicht zurueckgestellt, und ohne Klon gibt es einen Grund.
    #[test]
    fn ein_update_nur_wenn_github_hoeher_ist() {
        let hier = "0.2.0\n";
        let q = || Some(PathBuf::from("/klon"));
        let s = Stand::aus(q(), hier, Ok(DATEI.into()));
        assert!(s.neuer && s.lohnt());
        assert_eq!(s.neueste.as_deref(), Some("0.3.0"));
        assert_eq!(s.aenderungen.iter().map(|e| e.fassung.as_str()).collect::<Vec<_>>(), ["0.3.0"]);

        let gleich = Stand::aus(q(), "0.3.0\n", Ok(DATEI.into()));
        assert!(!gleich.lohnt() && gleich.aenderungen.is_empty() && gleich.grund.is_none());
        let voraus = Stand::aus(q(), "0.4.0\n", Ok(DATEI.into()));
        assert!(!voraus.lohnt(), "ein Klon, der voraus ist, wird zurueckgestellt");

        let ohne_klon = Stand::aus(None, hier, Ok(DATEI.into()));
        assert!(!ohne_klon.lohnt() && ohne_klon.grund.is_some(), "ein gesperrter Weg ohne Grund ist eine Sackgasse");

        let ohne_netz = Stand::aus(q(), hier, Err("kein Netz".into()));
        assert!(!ohne_netz.lohnt());
        assert_eq!(ohne_netz.grund.as_deref(), Some("kein Netz"), "ohne Netz heisst unbeantwortet, nicht aktuell");
    }

    /// ⛔️ **Die Versionsdatei im Repositorium ist lesbar, und ihre erste
    /// Zeile steht auch als einziger Abschnitt da.** Dieselbe Angabe an zwei
    /// Orten laeuft sonst auseinander: Wer die Nummer hebt und den Abschnitt
    /// vergisst, kuendigt ein Update ohne Inhalt an. ⚑ **Und es ist genau
    /// einer**: Die Datei ist kein Changelog; wer beim Heben den alten
    /// Abschnitt stehen laesst, faellt hier auf.
    #[test]
    fn die_eigene_versionsdatei_stimmt_mit_sich_ueberein() {
        let v = Versionsdatei::lesen(EINGEBAUT).expect("VERSION.md ist lesbar");
        assert_eq!(
            v.eintraege.first().map(|e| e.fassung.clone()),
            Some(v.fassung.to_string()),
            "die erste Zeile von VERSION.md und der Abschnitt nennen verschiedene Fassungen"
        );
        assert!(!v.eintraege[0].text.is_empty(), "der Abschnitt sagt nicht, was sich geaendert hat");
        assert_eq!(
            v.eintraege.iter().map(|e| e.fassung.as_str()).collect::<Vec<_>>(),
            [v.fassung.to_string()],
            "VERSION.md traegt nur den Abschnitt der aktuellen Fassung; die alten werden beim Heben entfernt"
        );
        assert!(adresse_dort().ends_with("/main/VERSION.md"));
    }

    /// ⛔️ **Kein Skript ueberschreibt ein laufendes Programm.** Das Update
    /// laeuft aus dem Programm heraus, das es ersetzt. Unter macOS und Linux
    /// wird deshalb erst geloescht und dann kopiert (sonst `Text file busy`
    /// unter Linux, ein SIGKILL wegen der alten Signatur unter macOS); unter
    /// Windows laesst sich ein laufendes Programm nicht ueberschreiben, wohl
    /// aber umbenennen.
    #[test]
    fn kein_skript_ueberschreibt_ein_laufendes_programm() {
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("Wurzel");
        for datei in [SKRIPT_MACOS, SKRIPT_LINUX, SKRIPT_NIXOS] {
            let text = std::fs::read_to_string(wurzel.join(datei)).expect(datei);
            // ⚑ Gesucht wird die Form, nicht ein Wortlaut: geloescht wird
            // `"$BIN/<programm>"`, kopiert aus einem `release/`-Ordner
            // nach `"$BIN/<programm>"`. Das Linux-Skript nimmt dabei den
            // wirklichen Bauordner (`CARGO_TARGET_DIR`).
            let zeilen: Vec<&str> = text.lines().map(str::trim).collect();
            let loeschen = zeilen
                .iter()
                .position(|z| z.contains("rm -f \"$BIN/$"))
                .unwrap_or_else(|| panic!("{datei} loescht das alte Programm nicht"));
            let kopieren = zeilen
                .iter()
                .position(|z| z.contains("cp \"") && z.contains("/release/$") && z.contains("\"$BIN/$"))
                .unwrap_or_else(|| panic!("{datei}: keine Kopierzeile"));
            assert!(loeschen < kopieren, "{datei} kopiert, bevor es loescht");
        }
        let ps = std::fs::read_to_string(wurzel.join(SKRIPT_WINDOWS.replace('\\', "/"))).expect("ps1");
        assert!(ps.contains("Rename-Item"), "das Windows-Skript benennt ein laufendes Programm nicht um");
    }

    /// ⚑ **Auf jeder Plattform, die dieses Projekt baut, gibt es ein
    /// Skript.** Faellt eine heraus, faellt es hier auf und nicht beim
    /// Nutzer.
    ///
    /// 📌 **Und der Name wird gegen die Datei gehalten, nicht nur
    /// gelesen.** Ein Konstantenname, den niemand gegen sein Ziel
    /// prueft, ist Fund 271 in einer anderen Verkleidung: Wer das
    /// Skript umbenennt, merkt es sonst erst, wenn ein Nutzer auf
    /// „Einspielen" drueckt und nichts geschieht.
    #[test]
    fn diese_plattform_hat_ein_skript() {
        let (was, argumente) = skript().expect("kein Installationsskript fuer diese Plattform");
        assert!(!was.is_empty());
        let name = argumente
            .iter()
            .find(|a| a.contains("installieren-"))
            .expect("kein Installationsskript in den Argumenten")
            // ⚑ Unter Windows steht der Pfad mit Backslash da; die
            // Datei liegt trotzdem an derselben Stelle.
            .replace('\\', "/");

        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");
        assert!(
            wurzel.join(&name).is_file(),
            "{name} steht in `skript()` und liegt nicht da"
        );
    }

    /// Jede Kiste, die sich zum Ausliefern angemeldet hat.
    ///
    /// ⚑ Gesucht wird dasselbe wie in den Installationsskripten, und
    /// zwar absichtlich auf dieselbe schlichte Weise: Findet die
    /// Pruefung etwas, das ein `sed` nicht findet, prueft sie nicht,
    /// was laeuft.
    fn angemeldete(wurzel: &Path) -> Vec<(PathBuf, String)> {
        fn sammeln(wo: &Path, aus: &mut Vec<(PathBuf, String)>) {
            let Ok(eintraege) = std::fs::read_dir(wo) else { return };
            for e in eintraege.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_dir() {
                    if name.starts_with("target") || name.starts_with('.') {
                        continue;
                    }
                    sammeln(&p, aus);
                } else if name == "Cargo.toml" {
                    let Ok(text) = std::fs::read_to_string(&p) else { continue };
                    for zeile in text.lines() {
                        if let Some(rest) = zeile.strip_prefix("ausliefern = \"") {
                            if let Some((wert, _)) = rest.split_once('"') {
                                aus.push((p.parent().unwrap().to_path_buf(), wert.to_string()));
                            }
                            break;
                        }
                    }
                }
            }
        }
        let mut aus = Vec::new();
        sammeln(wurzel, &mut aus);
        aus.sort();
        aus
    }

    /// **Was ausgeliefert wird, steht bei der Kiste.**
    ///
    /// # 📌 Fund 300, und er lag am ersten Tag schon offen
    ///
    /// Vier Skripte bauen die Programme dieses Repositoriums, und
    /// jedes fuehrte **seine eigene** Liste von Verzeichnissen und
    /// Namen. Zwei davon waren sofort uneinig: `myl-test` stand im
    /// einen und in den drei anderen nicht.
    ///
    /// ⚑ **Eine Liste, die an vier Stellen von Hand gefuehrt wird, ist
    /// kein Verzeichnis, sondern vier Behauptungen.** Die Kiste sagt
    /// jetzt selbst, ob sie ausgeliefert wird; die Skripte suchen
    /// danach.
    ///
    /// ⚠️ **Diese Pruefung ist der Grund, warum es dabei bleibt.** Ohne
    /// sie schriebe der Naechste, dem eine Suche in `sh` und in
    /// PowerShell zu umstaendlich ist, die Liste wieder hin, und die
    /// Skripte liefen weiter, bis eine Kiste dazukommt.
    #[test]
    fn was_ausgeliefert_wird_steht_bei_der_kiste() {
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");

        let angemeldet = angemeldete(wurzel);
        assert!(
            angemeldet.len() >= 4,
            "nur {} Kisten zum Ausliefern angemeldet: {angemeldet:?}",
            angemeldet.len()
        );

        // 1. Jeder angemeldete Name muss auch wirklich gebaut werden.
        for (verzeichnis, name) in &angemeldet {
            let manifest = std::fs::read_to_string(verzeichnis.join("Cargo.toml"))
                .expect("Cargo.toml");
            let paketname = manifest
                .split_once("name = \"")
                .and_then(|(_, r)| r.split_once('"'))
                .map(|(n, _)| n.to_string())
                .unwrap_or_default();
            let gibt_es = verzeichnis.join(format!("src/bin/{name}.rs")).is_file()
                || manifest.contains(&format!("name = \"{name}\""))
                    && manifest.contains("[[bin]]")
                || (paketname == *name && verzeichnis.join("src/main.rs").is_file());
            assert!(
                gibt_es,
                "{} meldet `{name}` zum Ausliefern an, baut aber kein Programm dieses Namens",
                verzeichnis.display()
            );
        }

        // 2. Und kein Skript fuehrt daneben seine eigene Liste.
        for datei in SKRIPTE.map(|d| d.replace('\\', "/")) {
            let datei = datei.as_str();
            let text = std::fs::read_to_string(wurzel.join(datei)).expect(datei);
            assert!(
                text.contains("ausliefern"),
                "{datei} sucht die angemeldeten Kisten nicht"
            );
            // 📌 **Gesucht wird die FORM der Liste und nicht die
            // Erwaehnung.** Die erste Fassung dieser Pruefung schlug
            // ueber `sh CLIENT/myl-oberflaeche/buendeln-macos.sh` an,
            // also ueber einen Aufruf, der mit der Liste nichts zu tun
            // hat. **Eine Pruefung, die Erwaehnung fuer Gebrauch
            // haelt, bestraft das Danebenschreiben**; dieselbe Klasse
            // wie die Wache, die `innerHTML` in einem Kommentar fand.
            //
            // Die Listenform ist ein Paar aus Verzeichnis und Name:
            // `CLIENT/myl-client myl` in der Shell,
            // `Verzeichnis = "CLIENT\myl-client"` in PowerShell.
            for (verzeichnis, name) in &angemeldet {
                let kurz = verzeichnis
                    .strip_prefix(wurzel)
                    .unwrap_or(verzeichnis)
                    .to_string_lossy()
                    .replace('\\', "/");
                for form in [
                    format!("{kurz} {name}"),
                    format!("Verzeichnis = \"{}\"", kurz.replace('/', "\\")),
                ] {
                    assert!(
                        !text.contains(&form),
                        "{datei} fuehrt `{form}` von Hand; die Liste gehoert an die Kiste"
                    );
                }
            }
        }
    }

    /// ⛔️ **Fund 458: alle drei Pfade, auf jeder Plattform geprueft.**
    ///
    /// Die Probe daneben sieht nur das Skript **dieser** Maschine an,
    /// denn `skript()` entscheidet per `cfg`. Als der Windows-Pfad beim
    /// Umzug nach `SYSTEM/` stehenblieb, war er damit oertlich
    /// unsichtbar und fiel erst auf dem Windows-Laeufer der CI.
    ///
    /// ⚑ **Diese hier haengt an keinem `cfg`.** Seit die drei Namen
    /// als Konstanten ausserhalb der Zweige stehen, uebersetzt jede
    /// Plattform alle drei, und jede kann sie nachsehen.
    #[test]
    fn alle_skripte_liegen_da() {
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");
        for eintrag in SKRIPTE {
            // Unter Windows steht der Pfad mit Backslash da; die Datei
            // liegt trotzdem an derselben Stelle.
            let name = eintrag.replace('\\', "/");
            assert!(
                wurzel.join(&name).is_file(),
                "{name} steht in SKRIPTE und liegt nicht da"
            );
        }
    }

    /// ⛔️ **Jeder Pfad im Klon, den ein Installationsskript nennt, liegt da.**
    ///
    /// 📌 **Fund 504 (2026-09-29):** `installieren-windows.ps1` rief nach dem
    /// Umzug nach `SYSTEM/` noch `INSTALL\vorrat.py` auf und holte die
    /// Programme aus `target-shared\release\`; beide Ordner gab es nicht
    /// mehr. Unter Windows scheiterte damit jede Einrichtung aus dem Klon.
    /// Die CI sah es nicht, denn ihr kalter Klon ruft `vorrat.py` und
    /// `cargo` selbst und nicht das Skript. Dieselbe Klasse wie Fund 458,
    /// und wie dort hilft nur eine Probe, die auf **jedem** System liest,
    /// was das Skript sagt.
    ///
    /// Geprueft wird der erste Ordner jedes Pfades hinter `$WURZEL/` und
    /// `Join-Path $Wurzel "…"`, unter `SYSTEM/` auch der zweite. Was erst
    /// beim Bauen oder Einrichten entsteht, steht in `ENTSTEHT`.
    #[test]
    fn jeder_pfad_in_den_skripten_liegt_da() {
        const ENTSTEHT: [&str; 5] = [
            ".git",
            "SYSTEM/full-build",
            "SYSTEM/crates-lager",
            "SYSTEM/crates-vorrat-lokal",
            "SYSTEM/werkzeug-vorrat",
        ];
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");
        let mut geprueft = 0;
        for eintrag in SKRIPTE {
            let skript = eintrag.replace('\\', "/");
            let text = std::fs::read_to_string(wurzel.join(&skript)).expect("Skript lesbar");
            let mut pfade: Vec<String> = Vec::new();
            // ⚑ `}` endet einen Pfad in `${VAR:-$WURZEL/…}`, der Vorgabe einer
            //   Variablen (das Linux-Skript nimmt so `CARGO_TARGET_DIR`).
            for (marke, ende) in [("$WURZEL/", &[' ', '"', '\'', ')', '}', '\n'][..]), ("Join-Path $Wurzel \"", &['"'][..])] {
                let mut rest = text.as_str();
                while let Some(i) = rest.find(marke) {
                    rest = &rest[i + marke.len()..];
                    let pfad = &rest[..rest.find(ende).unwrap_or(rest.len())];
                    pfade.push(pfad.replace('\\', "/"));
                }
            }
            for pfad in pfade {
                let teile: Vec<&str> = pfad.split('/').filter(|t| !t.is_empty()).collect();
                let tiefe = if teile.first() == Some(&"SYSTEM") { 2 } else { 1 };
                if teile.len() < tiefe || teile[..tiefe].iter().any(|t| t.contains('$')) {
                    continue;
                }
                let kopf = teile[..tiefe].join("/");
                if ENTSTEHT.contains(&kopf.as_str()) {
                    continue;
                }
                geprueft += 1;
                assert!(wurzel.join(&kopf).exists(), "{skript} nennt `{pfad}`, aber `{kopf}` gibt es im Klon nicht");
            }
        }
        // Eine Probe, die nichts findet, waere eine gruene ohne Gegenstand.
        assert!(geprueft >= 8, "nur {geprueft} Pfade gefunden; liest die Probe die Skripte noch?");
    }

    /// **Alle Installationsskripte liegen in `SYSTEM/install/`.**
    ///
    /// ⚑ **In einem eigenen Ordner mit einer Anleitung daneben**
    /// (Festlegung des Projektinhabers, 2026-09-10). Drei Skripte lose
    /// in der Wurzel sagen nicht, welches das eigene ist; ein Ordner
    /// namens `SYSTEM/install` mit einem README darin sagt es.
    ///
    /// ⚠️ **Und jedes muss `--aktualisieren` kennen**, denn genau damit
    /// ruft der Klient es. Ein Skript ohne diesen Schalter bricht mit
    /// „unbekannter Schalter" ab, und zwar erst beim Nutzer.
    #[test]
    fn die_installationsskripte_liegen_in_install() {
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");
        // ⚑ **Die Namen kommen aus `SKRIPTE` und stehen nicht noch
        //   einmal hier.** Genau diese Doppelung hat Fund 458 moeglich
        //   gemacht: Die Liste hier wurde nachgezogen, der Pfad in
        //   `skript()` nicht, und beide sagten dasselbe verschieden.
        for (datei, schalter) in SKRIPTE
            .iter()
            .map(|d| d.replace('\\', "/"))
            .zip(["--aktualisieren", "--aktualisieren", "--aktualisieren", "Aktualisieren"])
        {
            let datei = datei.as_str();
            let pfad = wurzel.join(datei);
            let text = std::fs::read_to_string(&pfad)
                .unwrap_or_else(|f| panic!("{}: {f}", pfad.display()));
            assert!(text.len() > 500, "{datei} ist zu kurz, um etwas zu tun");
            assert!(text.contains(schalter), "{datei} kennt `{schalter}` nicht");
        }
        // ⚑ Und die Nix-Umgebung, ohne die das Linux-Skript nur eine
        // leere Shell startet. **Sie bleibt in der Wurzel**, denn
        // `nix develop` sucht sie dort und nirgends sonst.
        assert!(wurzel.join("flake.nix").is_file(), "flake.nix fehlt in der Wurzel");

        // ⚑ **Und eine Anleitung daneben.** Ein Ordner mit drei
        // Skripten und ohne Text laesst den Leser raten, welches
        // seines ist.
        let anleitung = std::fs::read_to_string(wurzel.join("SYSTEM/install/README.md"))
            .expect("SYSTEM/install/README.md");
        for system in ["macOS", "Linux", "Windows", "NixOS"] {
            assert!(anleitung.contains(system), "die Anleitung nennt {system} nicht");
        }
    }
}
