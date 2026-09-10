//! Nachsehen, ob es einen neueren Stand gibt, und ihn einspielen.
//!
//! # ⚑ Was hier geprueft wird, und warum gerade das
//!
//! Die naheliegende Frage waere „ist meine Version aelter als die
//! neueste". Sie ist hier nicht beantwortbar: Die Freigabemarken
//! dieses Repositoriums heissen `v*` und gehoeren zum **Projekt**,
//! waehrend jede Kiste ihre eigene Fassung traegt. Ein Vergleich
//! zwischen `myl-oberflaeche 0.23.0` und `v0.26.0` waere ein Vergleich
//! zwischen zwei Zaehlwerken.
//!
//! ⚑ **Gefragt wird deshalb: Hat `origin` Aenderungen, die ich nicht
//! habe.** Das ist exakt, braucht keine Vereinbarung ueber Namen, und
//! es ist genau die Frage, deren Antwort das Einspielen aendert.
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
//! Es bewegt den Klon mit `git merge --ff-only` vorwaerts und ruft
//! danach das Installationsskript der Plattform. **Das ist Ausfuehren
//! von Code, der gerade geholt wurde**, und das gehoert benannt. Drei
//! Dinge begrenzen es: Es kommt aus dem Klon des Nutzers und nicht aus
//! einem Netzpfad, `git` traegt die Unversehrtheit der Uebertragung,
//! und `--ff-only` schliesst aus, dass dabei eigene Arbeit
//! verschwindet. **Wer das nicht will, laesst den Knopf stehen**; das
//! Nachsehen allein aendert nichts.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

/// Das Repositorium, gegen das gefragt wird.
///
/// ⚑ **Er steht hier und nicht in den Einstellungen.** Ein Klient, der
/// seine Aktualisierungsquelle einstellbar macht, hat ein Feld, in das
/// jemand etwas anderes schreiben kann; von dort kommt danach Code.
pub const SEITE: &str = "https://github.com/crowdedconfusion/Myelith";

/// Was das Nachsehen ergeben hat.
#[derive(Debug, Clone, Serialize)]
pub struct Stand {
    /// Das Quellverzeichnis, wenn dieses Programm aus einem Klon laeuft.
    pub quelle: Option<String>,
    /// Die Fassung dieses Programms.
    pub eigene: String,
    /// Die neueste Freigabemarke, wenn sie sich abfragen liess.
    pub neueste: Option<String>,
    /// Ihr Datum, wie GitHub es nennt.
    pub stand: Option<String>,
    /// Wie viele Aenderungen `origin` hat, die dieser Klon nicht hat.
    pub hinterher: Option<u32>,
    /// Die Seite, auf der die Freigaben liegen.
    pub seite: String,
    /// Warum nichts einzuspielen ist, wenn nichts einzuspielen ist.
    pub grund: Option<String>,
}

impl Stand {
    /// Gibt es etwas zu tun?
    pub fn lohnt(&self) -> bool {
        self.quelle.is_some() && self.hinterher.unwrap_or(0) > 0
    }
}

/// **Das Quellverzeichnis, von der laufenden Datei aus nach oben
/// gesucht.**
///
/// ⚑ **Zwei Merkmale und nicht eines.** Ein `.git` allein waere jeder
/// beliebige Klon; die Marke dieses Repositoriums macht es zu
/// **diesem**.
///
/// ⛑ **Bis zum 2026-09-10 suchte es nur vom Programm aufwaerts** und
/// fand deshalb nichts, sobald das Programm installiert war. Seither
/// fragt es `ort::wurzel`, und die traegt einen gemerkten Ort: Wer
/// einmal aus dem Klon heraus gestartet hat, kann danach auch aus
/// `~/.local/bin` heraus aktualisieren.
pub fn quelle() -> Option<PathBuf> {
    let w = crate::ort::wurzel()?;
    // ⚑ **Ein Baum ist noch kein Klon.** Wer ein Freigabearchiv
    // entpackt, hat die Marke und kein `.git`; `git merge` haette dort
    // nichts, was es vorwaertsbewegen koennte.
    w.join(".git").exists().then_some(w)
}

/// Fragt GitHub nach der neuesten Freigabe.
///
/// ⚑ **Ein Fehlschlag ist keiner.** Wer offline arbeitet, bekommt
/// `None` und keine Fehlermeldung: Die Frage „gibt es eine neuere
/// Fassung" ist ohne Netz schlicht unbeantwortet, und das ist etwas
/// anderes als „nein".
fn neueste_marke() -> (Option<String>, Option<String>) {
    let adresse = format!(
        "https://api.github.com/repos/{}/releases/latest",
        SEITE.trim_start_matches("https://github.com/")
    );
    let aus = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            "10",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: myelith-client",
            &adresse,
        ])
        .output()
        .ok();
    let text = match aus {
        Some(a) if a.status.success() => String::from_utf8_lossy(&a.stdout).to_string(),
        _ => return (None, None),
    };
    (feld(&text, "tag_name"), feld(&text, "published_at"))
}

/// Ein Zeichenkettenfeld aus einer flachen JSON-Antwort.
///
/// ⚑ **Von Hand und ohne JSON-Kiste.** Gebraucht werden zwei Felder
/// aus einer Antwort, die dieses Programm nicht weiterverarbeitet;
/// eine Abhaengigkeit dafuer waere eine Kiste im Graphen fuer zwei
/// Zeichenketten. Was hier nicht passt, gibt `None`, und `None` heisst
/// „unbeantwortet".
fn feld(text: &str, name: &str) -> Option<String> {
    let marke = format!("\"{name}\":");
    let rest = text.split_once(&marke)?.1.trim_start();
    let rest = rest.strip_prefix('"')?;
    let (wert, _) = rest.split_once('"')?;
    (!wert.is_empty()).then(|| wert.to_string())
}

/// Wie viele Aenderungen `origin` hat, die dieser Klon nicht hat.
fn hinterher(wurzel: &Path) -> Result<u32, String> {
    lauf(wurzel, "git", &["fetch", "--quiet", "origin"])?;
    let zahl = lauf(wurzel, "git", &["rev-list", "--count", "HEAD..FETCH_HEAD"])?;
    zahl.trim().parse().map_err(|_| format!("unerwartete Antwort von git: {zahl}"))
}

/// Ruft ein Programm im Quellverzeichnis und gibt seine Ausgabe.
fn lauf(wurzel: &Path, was: &str, argumente: &[&str]) -> Result<String, String> {
    let aus = Command::new(was)
        .args(argumente)
        .current_dir(wurzel)
        .output()
        .map_err(|f| format!("{was} liess sich nicht aufrufen: {f}"))?;
    if !aus.status.success() {
        let fehler = String::from_utf8_lossy(&aus.stderr).trim().to_string();
        return Err(if fehler.is_empty() {
            format!("{was} {} ging nicht", argumente.join(" "))
        } else {
            fehler
        });
    }
    Ok(String::from_utf8_lossy(&aus.stdout).to_string())
}

/// Sieht nach, ohne etwas zu aendern.
pub fn pruefen(eigene: &str) -> Stand {
    let (neueste, stand) = neueste_marke();
    let Some(wurzel) = quelle() else {
        return Stand {
            quelle: None,
            eigene: eigene.to_string(),
            neueste,
            stand,
            hinterher: None,
            seite: SEITE.to_string(),
            grund: Some(
                "Dieses Programm laeuft nicht aus einem Klon des Repositoriums. \
                 Ein neuer Stand laesst sich deshalb nicht von hier aus einspielen; \
                 die Freigabeseite fuehrt zu den fertigen Buendeln."
                    .to_string(),
            ),
        };
    };
    let (zahl, grund) = match hinterher(&wurzel) {
        Ok(n) => (Some(n), None),
        Err(f) => (None, Some(f)),
    };
    Stand {
        quelle: Some(wurzel.display().to_string()),
        eigene: eigene.to_string(),
        neueste,
        stand,
        hinterher: zahl,
        seite: SEITE.to_string(),
        grund,
    }
}

/// Das Installationsskript dieser Plattform.
///
/// ⚑ **Der Name steht hier und wird nicht geraten.** Drei Plattformen,
/// drei Skripte, und `cfg` entscheidet zur Bauzeit: Ein Programm, das
/// zur Laufzeit nach dem Betriebssystem fragt, kann sich irren.
pub fn skript() -> Option<(&'static str, &'static [&'static str])> {
    #[cfg(target_os = "macos")]
    {
        Some(("sh", &["INSTALL/installieren-macos.sh", "--aktualisieren"]))
    }
    #[cfg(target_os = "linux")]
    {
        Some(("sh", &["INSTALL/installieren-nixos.sh", "--aktualisieren"]))
    }
    #[cfg(target_os = "windows")]
    {
        Some((
            "powershell",
            &[
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "INSTALL\\installieren-windows.ps1",
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
pub fn einspielen(melder: &dyn Fn(&str)) -> Result<(), String> {
    let wurzel = quelle().ok_or_else(|| {
        "Dieses Programm laeuft nicht aus einem Klon; es gibt nichts einzuspielen.".to_string()
    })?;
    let (was, argumente) = skript().ok_or_else(|| {
        "Fuer dieses Betriebssystem gibt es kein Installationsskript.".to_string()
    })?;

    melder(&format!("{was} {}", argumente.join(" ")));
    let aus = Command::new(was)
        .args(argumente)
        .current_dir(&wurzel)
        .output()
        .map_err(|f| format!("{was} liess sich nicht aufrufen: {f}"))?;

    for zeile in String::from_utf8_lossy(&aus.stdout).lines() {
        melder(zeile);
    }
    if aus.status.success() {
        // ⚑ **Und der Hinweis, der sonst fehlt.** Ein Programm kann
        // sich nicht selbst waehrend des Laufens ersetzen; was gerade
        // gebaut wurde, laeuft erst beim naechsten Start.
        melder("Fertig. Der neue Stand laeuft ab dem naechsten Start.");
        return Ok(());
    }
    let fehler = String::from_utf8_lossy(&aus.stderr).trim().to_string();
    Err(if fehler.is_empty() { "das Installationsskript ging nicht durch".into() } else { fehler })
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Der Leser holt, was er soll, und nichts daneben.**
    #[test]
    fn ein_feld_wird_aus_der_antwort_gelesen() {
        let antwort = r#"{"url":"x","tag_name":"v0.26.0","published_at":"2026-09-10T08:00:00Z"}"#;
        assert_eq!(feld(antwort, "tag_name").as_deref(), Some("v0.26.0"));
        assert_eq!(feld(antwort, "published_at").as_deref(), Some("2026-09-10T08:00:00Z"));
        assert_eq!(feld(antwort, "gibt_es_nicht"), None);
    }

    /// ⛑ **Eine kaputte Antwort ergibt `None` und keinen Absturz.**
    /// GitHub antwortet auch mit Fehlerseiten, und eine Fehlerseite ist
    /// kein JSON.
    #[test]
    fn eine_kaputte_antwort_ergibt_nichts() {
        for text in ["", "<html>503</html>", r#"{"tag_name":"#, r#"{"tag_name":""}"#] {
            assert_eq!(feld(text, "tag_name"), None, "{text}");
        }
    }

    /// ⚑ **Ohne Klon ist nichts einzuspielen, und der Stand sagt es.**
    #[test]
    fn ohne_klon_gibt_es_einen_grund() {
        let s = Stand {
            quelle: None,
            eigene: "0.23.0".into(),
            neueste: Some("v0.26.0".into()),
            stand: None,
            hinterher: None,
            seite: SEITE.into(),
            grund: Some("kein Klon".into()),
        };
        assert!(!s.lohnt());
        assert!(s.grund.is_some(), "ein gesperrter Knopf ohne Grund ist eine Sackgasse");
    }

    /// ⚑ **Und mit Klon und Rueckstand lohnt es.**
    #[test]
    fn mit_rueckstand_lohnt_es() {
        let s = Stand {
            quelle: Some("/x".into()),
            eigene: "0.23.0".into(),
            neueste: None,
            stand: None,
            hinterher: Some(3),
            seite: SEITE.into(),
            grund: None,
        };
        assert!(s.lohnt());
    }

    /// ⛑ **Null Rueckstand ist kein Rueckstand.** Ein Knopf, der bei
    /// „alles aktuell" trotzdem baut, kostet Minuten fuer nichts.
    #[test]
    fn ohne_rueckstand_lohnt_es_nicht() {
        let s = Stand {
            quelle: Some("/x".into()),
            eigene: "0.23.0".into(),
            neueste: None,
            stand: None,
            hinterher: Some(0),
            seite: SEITE.into(),
            grund: None,
        };
        assert!(!s.lohnt());
    }

    /// ⚑ **Auf jeder Plattform, die dieses Projekt baut, gibt es ein
    /// Skript.** Faellt eine heraus, faellt es hier auf und nicht beim
    /// Nutzer.
    ///
    /// ⛑ **Und der Name wird gegen die Datei gehalten, nicht nur
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
    /// # ⛑ Fund 300, und er lag am ersten Tag schon offen
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
        for datei in [
            "INSTALL/installieren-macos.sh",
            "INSTALL/installieren-nixos.sh",
            "INSTALL/installieren-windows.ps1",
        ] {
            let text = std::fs::read_to_string(wurzel.join(datei)).expect(datei);
            assert!(
                text.contains("ausliefern"),
                "{datei} sucht die angemeldeten Kisten nicht"
            );
            // ⛑ **Gesucht wird die FORM der Liste und nicht die
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

    /// **Alle drei Installationsskripte liegen in `INSTALL/`.**
    ///
    /// ⚑ **In einem eigenen Ordner mit einer Anleitung daneben**
    /// (Festlegung des Projektinhabers, 2026-09-10). Drei Skripte lose
    /// in der Wurzel sagen nicht, welches das eigene ist; ein Ordner
    /// namens `INSTALL` mit einem README darin sagt es.
    ///
    /// ⚠️ **Und jedes muss `--aktualisieren` kennen**, denn genau damit
    /// ruft der Klient es. Ein Skript ohne diesen Schalter bricht mit
    /// „unbekannter Schalter" ab, und zwar erst beim Nutzer.
    #[test]
    fn drei_installationsskripte_liegen_in_install() {
        let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums");
        for (datei, schalter) in [
            ("INSTALL/installieren-macos.sh", "--aktualisieren"),
            ("INSTALL/installieren-nixos.sh", "--aktualisieren"),
            ("INSTALL/installieren-windows.ps1", "Aktualisieren"),
        ] {
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
        let anleitung = std::fs::read_to_string(wurzel.join("INSTALL/README.md"))
            .expect("INSTALL/README.md");
        for system in ["macOS", "Windows", "NixOS"] {
            assert!(anleitung.contains(system), "die Anleitung nennt {system} nicht");
        }
    }
}
