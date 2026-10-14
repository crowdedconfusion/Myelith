//! **In eine Schicht eintreten, ohne dass ihr Code je als root laeuft.**
//!
//!   golem-einrichten schicht-eintreten <wurzel> <programm> [argumente]
//!
//! # ⛔️ Warum das hier steht und nicht in einem Skript
//!
//! `chroot` verlangt root. Ein Skript koennte erst wechseln und dann, schon
//! in der Schicht, mit einem Werkzeug **der Schicht** das Konto wechseln;
//! dann liefe dieses Werkzeug als root, und eine untergeschobene Schicht
//! haette root. BusyBox' `chroot` kennt kein `--userspec`, und das `setpriv`
//! der Basis kann nicht hinein, weil nach dem Kontowechsel kein `chroot`
//! mehr geht.
//!
//! ⚑ **Dieser Befehl tut beides in einem Prozess der Basis:** Er wechselt
//! die Wurzel, legt Gruppen, Gruppe und Konto ab, und erst danach wird das
//! erste Programm der Schicht ausgefuehrt. Ab da gibt es kein Vorrecht mehr,
//! das die Schicht erben koennte. Die Umgebung wird geleert; was das
//! Programm braucht, setzt es selbst (`golem-kiosk`).

use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

/// Das Konto, unter dem alles in einer Schicht laeuft.
///
/// ⚑ Dieselbe Zahl steht in `konfigurationen/schicht_benutzer.tabelle`;
/// eine Probe haelt beide zusammen.
pub const KONTO: u32 = 1000;

/// Prueft die Argumente, bevor irgendetwas geschieht.
pub fn pruefen(wurzel: &Path, programm: &str) -> io::Result<()> {
    if !wurzel.is_absolute() || !wurzel.is_dir() {
        return Err(io::Error::other(format!("{} ist kein absoluter Ordner", wurzel.display())));
    }
    if wurzel == Path::new("/") {
        return Err(io::Error::other("die Wurzel der Basis ist keine Schicht"));
    }
    if !programm.starts_with('/') {
        return Err(io::Error::other(format!("{programm}: das Programm braucht einen absoluten Pfad in der Schicht")));
    }
    Ok(())
}

/// Wechselt nach `wurzel`, legt alle Vorrechte ab und fuehrt `programm` aus.
/// Kehrt nur mit einem Fehler zurueck.
pub fn eintreten(wurzel: &Path, programm: &str, argumente: &[String]) -> io::Error {
    if let Err(f) = pruefen(wurzel, programm) {
        return f;
    }
    if let Err(f) = std::os::unix::fs::chroot(wurzel) {
        return io::Error::other(format!("chroot {}: {f} (nur als root moeglich)", wurzel.display()));
    }
    if let Err(f) = std::env::set_current_dir("/") {
        return f;
    }
    let term = std::env::var("TERM").unwrap_or_else(|_| "linux".to_string());
    // ⚑ `uid` und `gid` setzt die Standardbibliothek vor dem `exec`, und als
    //   root ruft sie dabei `setgroups(0)`: Keine Zusatzgruppe der Basis
    //   kommt mit.
    Command::new(programm)
        .args(argumente)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("TERM", term)
        .uid(KONTO)
        .gid(KONTO)
        .exec()
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn falsche_argumente_aendern_nichts() {
        assert!(pruefen(Path::new("relativ"), "/usr/bin/x").is_err());
        assert!(pruefen(Path::new("/"), "/usr/bin/x").is_err());
        assert!(pruefen(&std::env::temp_dir(), "golem-kiosk").is_err());
        assert!(pruefen(&std::env::temp_dir(), "/usr/bin/golem-kiosk").is_ok());
        let vorher = std::env::current_dir().expect("Ordner");
        let f = eintreten(Path::new("relativ"), "/usr/bin/x", &[]);
        assert!(f.to_string().contains("absoluter Ordner"), "{f}");
        assert_eq!(std::env::current_dir().expect("Ordner"), vorher);
    }

    /// Ohne root gibt es kein `chroot`, und dann wird auch nichts
    /// ausgefuehrt: Der Fehler kommt zurueck, statt dass das Programm
    /// ausserhalb der Schicht liefe.
    #[test]
    fn ohne_root_kein_eintritt() {
        // `geteuid` ueber `id -u`, denn die Kiste hat keine Abhaengigkeit.
        let root = Command::new("id").arg("-u").output().map(|o| o.stdout.starts_with(b"0\n")).unwrap_or(false);
        if root {
            return;
        }
        let f = eintreten(&std::env::temp_dir(), "/bin/true", &[]);
        assert!(f.to_string().contains("chroot"), "{f}");
    }

    #[test]
    fn das_konto_steht_auch_in_der_tabelle() {
        let tabelle = include_str!("../../konfigurationen/schicht_benutzer.tabelle");
        let zeile = tabelle.lines().find(|z| z.starts_with("golem ")).expect("golem fehlt in der Tabelle");
        let felder: Vec<&str> = zeile.split_whitespace().collect();
        assert_eq!((felder[1], felder[3]), (KONTO.to_string().as_str(), KONTO.to_string().as_str()), "{zeile}");
    }
}
