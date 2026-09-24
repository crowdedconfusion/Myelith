//! **Sich selbst mit Verwalterrechten neu starten**, für `--root`.
//!
//! # ⚑ Warum das überhaupt eine eigene Datei ist
//!
//! Der Vorgang ist auf jedem System ein anderer, und keiner davon lässt
//! sich hier ausprobieren: Unter Unix fragt `sudo` nach einem Passwort,
//! unter Windows öffnet die Benutzerkontensteuerung ein eigenes Fenster.
//! **Was sich prüfen lässt, ist der Aufruf**, und deshalb steht er
//! getrennt vom Ausführen, wie bei den Sinnesprogrammen auch.
//!
//! # ⛔️ Zwei Dinge, die nicht verwechselt werden dürfen
//!
//! 1. **`--root` verschiebt die Einhängegrenze.** Das tut es immer, auch
//!    ohne Verwalterrechte.
//! 2. **Die Erhöhung verschafft Rechte.** Sie ist ein Neustart des
//!    Programms als jemand anderes.
//!
//! ⚑ **Deshalb erhöht `/root` nicht.** Der Befehl fällt mitten in eine
//! Sitzung, in der ein Modell geladen ist und ein Gespräch steht; ein
//! Neustart würfe beides weg. Wer Rechte will, startet neu.
//!
//! # ⚠️ Ohne Fremdkiste, und das ist keine Sparsamkeit
//!
//! Unter Unix genügen `sudo` und [`std::os::unix::process::CommandExt::exec`]
//! aus der Standardbibliothek. Unter Windows tut es PowerShell mit
//! `Start-Process -Verb RunAs`; die Benutzerkontensteuerung über die
//! Win32-Schnittstelle zu rufen bräuchte rohes FFI oder eine Kiste, und
//! **eine Erhöhung ist die falsche Stelle für ungeprüften unsicheren
//! Code.**

use std::path::{Path, PathBuf};
use std::process::Command;

/// **Der Riegel gegen eine Schleife.**
///
/// ⛔️ **Ohne ihn startet sich das Programm endlos neu**, sobald die
/// Erhöhung zwar gelingt, der Prozess danach aber trotzdem nicht als
/// Verwalter läuft (eine `sudo`-Regel, die auf einen anderen Benutzer
/// zeigt, genügt dafür). Der zweite Lauf sieht dieselbe Lage wie der
/// erste und entscheidet dasselbe.
pub const MARKE: &str = "MYELITH_ERHOEHT";

/// Wer die Erhöhung nicht will, sagt es hier.
pub const ABSCHALTER: &str = "MYELITH_OHNE_ERHOEHUNG";

/// Warum nicht erhöht wird, wenn nicht erhöht wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grund {
    /// Läuft schon mit Verwalterrechten.
    SchonVerwalter,
    /// Ein vorheriger Versuch hat nichts gebracht; siehe [`MARKE`].
    SchonVersucht,
    /// Ausdrücklich abgeschaltet.
    Abgeschaltet,
    /// Kein Terminal, also niemand, der ein Passwort eingeben könnte.
    OhneTerminal,
    /// Auf diesem System ist die Rechtelage nicht feststellbar.
    UnbekannteLage,
}

impl Grund {
    /// Der Satz, der dem Menschen gesagt wird.
    pub fn satz(&self) -> &'static str {
        match self {
            Self::SchonVerwalter => "Dieser Lauf hat bereits Verwalterrechte.",
            Self::SchonVersucht => {
                "Ein Neustart mit Verwalterrechten wurde schon versucht und hat nichts geaendert; \
                 es bleibt bei den Rechten dieses Laufs."
            }
            Self::Abgeschaltet => {
                "Der Neustart mit Verwalterrechten ist abgeschaltet (MYELITH_OHNE_ERHOEHUNG)."
            }
            Self::OhneTerminal => {
                "Ohne Terminal wird nicht erhoeht: Ein Passwort kann niemand eingeben."
            }
            Self::UnbekannteLage => {
                "Ob dieser Lauf Verwalterrechte hat, laesst sich hier nicht feststellen; \
                 es wird nichts neu gestartet."
            }
        }
    }
}

/// **Soll erhöht werden?**
///
/// ⚑ **Jede Bedingung ist ein eigener Grund**, denn der Mensch soll
/// erfahren, warum sein `--root` ihn nicht zum Verwalter gemacht hat.
/// Ein blosses „nein" liesse ihn raten.
pub fn noetig(ist_verwalter: Option<bool>, terminal: bool) -> Result<(), Grund> {
    if std::env::var(ABSCHALTER).is_ok_and(|w| !w.trim().is_empty()) {
        return Err(Grund::Abgeschaltet);
    }
    if std::env::var(MARKE).is_ok_and(|w| !w.trim().is_empty()) {
        return Err(Grund::SchonVersucht);
    }
    match ist_verwalter {
        Some(true) => return Err(Grund::SchonVerwalter),
        None => return Err(Grund::UnbekannteLage),
        Some(false) => {}
    }
    if !terminal {
        return Err(Grund::OhneTerminal);
    }
    Ok(())
}

/// **Der Aufruf unter Unix.**
///
/// ⛔️ **`sudo` setzt `HOME` auf das des Verwalters**, und damit läge die
/// Ablage des Agenten plötzlich unter `/root` oder
/// `/var/root`: andere Einstellungen, anderes Modell, andere
/// Gesprächsablage. **Der Schalter soll Rechte geben und nicht die
/// Identität wechseln**, deshalb wird `HOME` über `env` wieder auf den
/// ursprünglichen Wert gesetzt.
///
/// ⚑ **`env` und nicht `sudo --preserve-env=HOME`:** `env` ist auf jedem
/// System da und hängt an keiner `sudoers`-Regel. Ein
/// `--preserve-env`, das die Regel verbietet, liesse `sudo` scheitern,
/// und der Mensch sähe eine Fehlermeldung über eine Einstellung, die er
/// nie angefasst hat.
pub fn befehl_unix(eigen: &Path, argumente: &[String], heimat: Option<&str>) -> Command {
    let mut b = Command::new("sudo");
    b.arg("--").arg("env");
    if let Some(h) = heimat {
        b.arg(format!("HOME={h}"));
    }
    b.arg(format!("{MARKE}=1"));
    // ⚑ **Die eigenen Umgebungsangaben gehen mit.** Sie sind die
    //   Schalter dieses Laufs (Blick, Geraete, Modellwahl); ohne sie
    //   waere der erhoehte Lauf ein anderer als der, den jemand gerade
    //   gestartet hat.
    let mut eigenes: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("MYL_") || k.starts_with("INTEGER_LLM_"))
        .collect();
    eigenes.sort();
    for (k, w) in eigenes {
        b.arg(format!("{k}={w}"));
    }
    b.arg(eigen);
    b.args(argumente);
    b
}

/// **Der Aufruf unter Windows.**
///
/// ⚠️ **Die Benutzerkontensteuerung öffnet ein eigenes Fenster**, und
/// das lässt sich nicht abstellen: Eine erhöhte Sitzung erbt keine
/// Konsole. Der laufende Prozess endet deshalb danach und sagt es.
/// ⚑ **Gebaut wird sie unter Windows und in jeder Pruefsammlung.** So
/// laesst sich der Windows-Aufruf auf jeder Maschine pruefen, und
/// ausserhalb der Proben meldet Clippy weiterhin, wenn sie niemand
/// ruft.
#[cfg(any(not(unix), test))]
pub fn befehl_windows(eigen: &Path, argumente: &[String]) -> Command {
    let mut b = Command::new("powershell");
    // ⚑ **Einfache Anfuehrungszeichen und verdoppelte darin**, so zitiert
    //   PowerShell woertlich. Ein Pfad mit Leerzeichen ist der
    //   Normalfall (`C:\Program Files\...`), kein Sonderfall.
    let zitat = |t: &str| format!("'{}'", t.replace('\'', "''"));
    let liste = argumente.iter().map(|a| zitat(a)).collect::<Vec<_>>().join(",");
    let mut kern = format!("Start-Process -FilePath {} -Verb RunAs", zitat(&eigen.display().to_string()));
    if !liste.is_empty() {
        kern.push_str(&format!(" -ArgumentList {liste}"));
    }
    b.arg("-NoProfile").arg("-NonInteractive").arg("-Command").arg(kern);
    b
}

/// **Startet sich selbst mit Verwalterrechten neu.**
///
/// Unter Unix kehrt diese Funktion bei Erfolg **nicht** zurück: Der
/// Prozess wird ersetzt. Unter Windows kommt sie zurück, und der
/// Aufrufer beendet sich dann.
///
/// ⚠️ **Der Rückgabewert ist eine Meldung**, kein Erfolg: Steht hier
/// etwas, ist nichts passiert.
pub fn versuchen(argumente: &[String]) -> Result<Beendet, String> {
    let eigen: PathBuf = std::env::current_exe()
        .map_err(|f| format!("das eigene Programm ist nicht auffindbar: {f}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let heimat = std::env::var("HOME").ok();
        let mut b = befehl_unix(&eigen, argumente, heimat.as_deref());
        // ⚑ **`exec` und nicht `spawn`**: Der erhoehte Lauf soll
        //   dasselbe Terminal haben und derselbe Vordergrundprozess
        //   sein. Ein Elternprozess, der nur noch wartet, faengt
        //   ausserdem Strg-C ab, das dem Kind gilt.
        let fehler = b.exec();
        Err(format!("sudo liess sich nicht starten: {fehler}"))
    }
    #[cfg(not(unix))]
    {
        let mut b = befehl_windows(&eigen, argumente);
        match b.status() {
            Ok(s) if s.success() => Ok(Beendet::NeuesFenster),
            Ok(s) => Err(format!("die Erhoehung wurde abgelehnt oder abgebrochen ({s})")),
            Err(f) => Err(format!("powershell liess sich nicht starten: {f}")),
        }
    }
}

/// Was nach einer gelungenen Erhöhung zu tun ist.
///
/// ⚠️ **Unter Unix wird diese Auskunft nie gegeben**, denn dort ersetzt
/// `exec` den Prozess und es gibt kein „danach". Der Typ steht trotzdem
/// in der Signatur beider Systeme: Ein Rückgabetyp, der je System ein
/// anderer ist, verlagert die Fallunterscheidung an die Aufrufstelle,
/// und dort ist sie schlechter aufgehoben als hier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beendet {
    /// Der erhöhte Lauf hat ein eigenes Fenster; dieser hier ist fertig.
    #[cfg_attr(unix, allow(dead_code))]
    NeuesFenster,
}

#[cfg(test)]
mod proben {
    use super::*;

    fn argumente(b: &Command) -> Vec<String> {
        b.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    /// ⛔️ **`HOME` muss mitgehen**, sonst läuft der erhöhte Lauf auf der
    /// Ablage des Verwalters: andere Einstellungen, anderes Modell,
    /// andere Gesprächsablage.
    #[test]
    fn der_unix_aufruf_traegt_die_heimat_weiter() {
        let a = argumente(&befehl_unix(Path::new("/pfad/myelith"), &["--root".into()], Some("/Users/wer")));
        assert!(a.contains(&"HOME=/Users/wer".to_string()), "{a:?}");
        assert!(a.contains(&"/pfad/myelith".to_string()), "{a:?}");
        assert!(a.contains(&"--root".to_string()), "{a:?}");
    }

    /// ⛔️ **Der Riegel gegen die Schleife steht im Aufruf.** Ohne ihn
    /// startet sich das Programm endlos neu, wenn die Erhöhung gelingt
    /// und trotzdem keine Rechte bringt.
    #[test]
    fn der_unix_aufruf_setzt_den_riegel() {
        let a = argumente(&befehl_unix(Path::new("/pfad/myelith"), &[], None));
        assert!(a.iter().any(|x| x == &format!("{MARKE}=1")), "{a:?}");
    }

    /// ⚑ **Ein Pfad mit Leerzeichen ist der Normalfall unter Windows.**
    #[test]
    fn der_windows_aufruf_zitiert_den_pfad() {
        let a = argumente(&befehl_windows(
            Path::new(r"C:\Program Files\Myelith\myelith.exe"),
            &["--root".into()],
        ));
        let befehl = a.last().expect("der Befehl");
        assert!(befehl.contains("-Verb RunAs"), "{befehl}");
        assert!(befehl.contains("'C:\\Program Files\\Myelith\\myelith.exe'"), "{befehl}");
        assert!(befehl.contains("-ArgumentList '--root'"), "{befehl}");
    }

    /// ⚠️ **Ohne Argumente keine leere Argumentliste.** `-ArgumentList`
    /// ohne Wert ist ein Syntaxfehler in PowerShell.
    #[test]
    fn der_windows_aufruf_laesst_die_leere_liste_weg() {
        let a = argumente(&befehl_windows(Path::new(r"C:\x\myelith.exe"), &[]));
        let befehl = a.last().expect("der Befehl");
        assert!(!befehl.contains("-ArgumentList"), "{befehl}");
    }

    /// ⛔️ **Jede Bedingung nennt ihren eigenen Grund.**
    #[test]
    fn die_gruende_werden_unterschieden() {
        // Die Umgebung ist prozessweit; diese Probe raeumt hinter sich auf.
        unsafe { std::env::remove_var(ABSCHALTER) };
        unsafe { std::env::remove_var(MARKE) };
        assert_eq!(noetig(Some(true), true), Err(Grund::SchonVerwalter));
        assert_eq!(noetig(None, true), Err(Grund::UnbekannteLage));
        assert_eq!(noetig(Some(false), false), Err(Grund::OhneTerminal));
        assert_eq!(noetig(Some(false), true), Ok(()));

        unsafe { std::env::set_var(MARKE, "1") };
        assert_eq!(noetig(Some(false), true), Err(Grund::SchonVersucht));
        unsafe { std::env::remove_var(MARKE) };

        unsafe { std::env::set_var(ABSCHALTER, "1") };
        assert_eq!(noetig(Some(false), true), Err(Grund::Abgeschaltet));
        unsafe { std::env::remove_var(ABSCHALTER) };
    }

    /// ⚑ **Der Abschalter schlaegt alles andere.** Wer ihn setzt, will
    /// nicht erhoeht werden, auch nicht „nur dieses eine Mal".
    #[test]
    fn der_abschalter_gilt_vor_jeder_anderen_pruefung() {
        unsafe { std::env::set_var(ABSCHALTER, "1") };
        assert_eq!(noetig(Some(true), true), Err(Grund::Abgeschaltet));
        unsafe { std::env::remove_var(ABSCHALTER) };
    }
}
