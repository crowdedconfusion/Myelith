//! **Lesen und Schreiben ohne einen Verweis, der nach der Pruefung entstand.**
//!
//! ⛔️ **Fund A52 (Sicherheitsaudit vom 2026-11-18).** Die Dateiwerkzeuge
//! pruefen einen Pfad mit `Einhaengung::aufloesen` (aufloesen, dann
//! Praefixvergleich) und oeffneten danach den aufgeloesten Pfad mit
//! `std::fs`, das jedem Verweis folgt. Zwischen beidem liegt ein Fenster.
//! Ein Prozess, den `run_command` im Hintergrund zuruecklaesst, darf im
//! Arbeitsordner schreiben; tauscht er dort in einer Schleife einen Ordner
//! gegen einen Verweis nach draussen, liest oder schreibt das Werkzeug im
//! Fenster dort, etwa in `~/.ssh`. Die Dateiwerkzeuge laufen im Prozess des
//! Agenten und damit ausserhalb der Sandbox; der Tausch hob sie also auf
//! (Zeitpunkt der Pruefung gegen Zeitpunkt der Benutzung, CWE-367).
//!
//! ⚑ **Die Abhilfe:** Geoeffnet wird Stueck fuer Stueck vom Arbeitsordner
//! aus, jeder Ordner mit `openat(…, O_DIRECTORY | O_NOFOLLOW)` am Ordner davor,
//! die Datei selbst mit `O_NOFOLLOW`. Der Pfad ist schon aufgeloest, enthaelt
//! also keinen Verweis mehr; ist dort jetzt einer, scheitert das Oeffnen,
//! statt ihm zu folgen. Dasselbe Muster wie `openat2` mit `RESOLVE_BENEATH`
//! unter Linux, aber auf jedem Unix vorhanden.
//!
//! Dazu oeffnet es nur gewoehnliche Dateien (`O_NONBLOCK` beim Oeffnen, dann
//! `fstat`): Eine benannte Roehre im Arbeitsordner haelt sonst das Lesen fuer
//! immer an.
//!
//! ⚠️ **Unter Windows** gilt weiter `std::fs`; dort laeuft auch keine Sandbox
//! des Betriebssystems, und das Fenster ist nicht der schwaechste Punkt.
//!
//! # ⛔️ Die dritte Ausnahme von `deny(unsafe_code)` in dieser Kiste
//!
//! `openat`, `mkdirat` und `fcntl` gibt es in `std` nicht. Jeder Aufruf
//! bekommt einen Zeiger auf einen `CString`, der ueber den Aufruf lebt, und
//! jeder zurueckgegebene Deskriptor geht sofort in eine `File`, die ihn
//! schliesst.
#![allow(unsafe_code)]

use std::io;
use std::path::Path;

/// Liest eine Datei unter `wurzel`. `ziel` ist aufgeloest und liegt darin.
pub fn lesen(wurzel: &Path, ziel: &Path) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut f = unix::oeffnen(wurzel, ziel, false)?;
        let mut b = Vec::new();
        f.read_to_end(&mut b)?;
        Ok(b)
    }
    #[cfg(not(unix))]
    {
        let _ = wurzel;
        std::fs::read(ziel)
    }
}

/// Liest eine Datei als Text (UTF-8), wie `std::fs::read_to_string`.
pub fn lesen_text(wurzel: &Path, ziel: &Path) -> io::Result<String> {
    String::from_utf8(lesen(wurzel, ziel)?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// Schreibt eine Datei unter `wurzel` (anlegen oder ersetzen) und legt
/// fehlende Ordner dazwischen an.
pub fn schreiben(wurzel: &Path, ziel: &Path, inhalt: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        let mut f = unix::oeffnen(wurzel, ziel, true)?;
        f.write_all(inhalt)?;
        f.flush()
    }
    #[cfg(not(unix))]
    {
        let _ = wurzel;
        if let Some(e) = ziel.parent() {
            std::fs::create_dir_all(e)?;
        }
        std::fs::write(ziel, inhalt)
    }
}

#[cfg(unix)]
mod unix {
    use std::ffi::CString;
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Component, Path};

    fn fehlt_aussen() -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, "der Pfad liegt nicht unter dem Arbeitsordner")
    }

    fn name(n: &std::ffi::OsStr) -> io::Result<CString> {
        CString::new(n.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Nullbyte im Pfad"))
    }

    /// Oeffnet `ziel` Stueck fuer Stueck von `wurzel` aus, ohne einem
    /// Verweis zu folgen. `schreiben`: anlegen oder kuerzen, fehlende Ordner
    /// anlegen; sonst nur lesen.
    pub(super) fn oeffnen(wurzel: &Path, ziel: &Path, schreiben: bool) -> io::Result<File> {
        let rel = ziel.strip_prefix(wurzel).map_err(|_| fehlt_aussen())?;
        let mut teile = Vec::new();
        for c in rel.components() {
            match c {
                Component::Normal(n) => teile.push(name(n)?),
                _ => return Err(fehlt_aussen()),
            }
        }
        let Some((datei, ordner)) = teile.split_last() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "kein Dateiname"));
        };
        let w = CString::new(wurzel.as_os_str().as_bytes()).map_err(|_| fehlt_aussen())?;
        // Die Wurzel selbst ist aufgeloest und liegt ausserhalb dessen, was
        // ein Befehl im Arbeitsordner tauschen kann.
        let fd = unsafe { libc::open(w.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut dir = unsafe { File::from_raw_fd(fd) };
        let ordnerflags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        for t in ordner {
            let mut fd = unsafe { libc::openat(dir.as_raw_fd(), t.as_ptr(), ordnerflags) };
            if fd < 0 && schreiben && io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                if unsafe { libc::mkdirat(dir.as_raw_fd(), t.as_ptr(), 0o777) } != 0
                    && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(io::Error::last_os_error());
                }
                fd = unsafe { libc::openat(dir.as_raw_fd(), t.as_ptr(), ordnerflags) };
            }
            if fd < 0 {
                return Err(verweis_oder(io::Error::last_os_error()));
            }
            dir = unsafe { File::from_raw_fd(fd) };
        }
        let art = if schreiben { libc::O_WRONLY | libc::O_CREAT } else { libc::O_RDONLY };
        let fd = unsafe {
            libc::openat(dir.as_raw_fd(), datei.as_ptr(), art | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC, 0o666 as libc::c_uint)
        };
        if fd < 0 {
            return Err(verweis_oder(io::Error::last_os_error()));
        }
        let f = unsafe { File::from_raw_fd(fd) };
        let m = f.metadata()?;
        if !m.file_type().is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "keine gewoehnliche Datei"));
        }
        // Erst jetzt kuerzen: an der geoeffneten Datei, nicht am Namen.
        if schreiben {
            f.set_len(0)?;
        }
        // Wieder blockierend, wie jede gewoehnliche Datei.
        unsafe {
            let fl = libc::fcntl(f.as_raw_fd(), libc::F_GETFL);
            if fl >= 0 {
                libc::fcntl(f.as_raw_fd(), libc::F_SETFL, fl & !libc::O_NONBLOCK);
            }
        }
        Ok(f)
    }

    /// `ELOOP` und `ENOTDIR` heissen hier: An der Stelle liegt jetzt ein
    /// Verweis (oder eine Datei statt eines Ordners). Gesagt wird das, nicht
    /// die Nummer.
    fn verweis_oder(f: io::Error) -> io::Error {
        match f.raw_os_error() {
            Some(libc::ELOOP) | Some(libc::ENOTDIR) => io::Error::new(
                io::ErrorKind::PermissionDenied,
                "der Pfad hat sich seit der Pruefung geaendert (ein Verweis oder eine Datei statt eines Ordners); nicht gefolgt",
            ),
            _ => f,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Lesen, schreiben, Ordner anlegen: wie `std::fs`, solange niemand tauscht.
    #[test]
    fn wie_std_fs_ohne_tausch() {
        let d = tempfile::tempdir().unwrap();
        let w = d.path().canonicalize().unwrap();
        schreiben(&w, &w.join("a/b/c.txt"), b"hallo").unwrap();
        assert_eq!(lesen(&w, &w.join("a/b/c.txt")).unwrap(), b"hallo");
        schreiben(&w, &w.join("a/b/c.txt"), b"x").unwrap();
        assert_eq!(lesen_text(&w, &w.join("a/b/c.txt")).unwrap(), "x", "ersetzt, nicht angehaengt");
        assert!(lesen(&w, &w.join("fehlt.txt")).is_err());
        assert!(lesen(&w, Path::new("/etc/hosts")).is_err(), "ausserhalb der Wurzel");
    }

    /// ⛔️ Fund A52: Ein Ordner, der nach der Pruefung zum Verweis nach
    /// draussen wurde, wird nicht gefolgt, weder beim Lesen noch beim
    /// Schreiben. Gegenprobe: `std::fs` folgt ihm.
    #[test]
    fn ein_getauschter_ordner_fuehrt_nicht_hinaus() {
        let d = tempfile::tempdir().unwrap();
        let draussen = tempfile::tempdir().unwrap();
        let w = d.path().canonicalize().unwrap();
        std::fs::write(draussen.path().join("geheim"), b"GEHEIM").unwrap();
        // Geprueft wurde `w/sub/geheim` mit `sub` als Ordner ...
        let ziel = w.join("sub/geheim");
        // ... danach ist `sub` ein Verweis nach draussen.
        std::os::unix::fs::symlink(draussen.path(), w.join("sub")).unwrap();
        assert_eq!(std::fs::read(&ziel).unwrap(), b"GEHEIM", "Gegenprobe: std::fs folgt dem Verweis");
        let f = lesen(&w, &ziel).unwrap_err();
        assert!(f.to_string().contains("seit der Pruefung"), "{f}");
        assert!(schreiben(&w, &ziel, b"boese").is_err());
        assert_eq!(std::fs::read(draussen.path().join("geheim")).unwrap(), b"GEHEIM", "draussen unveraendert");
        // Der letzte Teil als Verweis ebenso.
        std::os::unix::fs::symlink(draussen.path().join("geheim"), w.join("datei")).unwrap();
        assert!(lesen(&w, &w.join("datei")).is_err());
        assert!(schreiben(&w, &w.join("datei"), b"boese").is_err());
        assert_eq!(std::fs::read(draussen.path().join("geheim")).unwrap(), b"GEHEIM");
    }

    /// Eine benannte Roehre haelt das Lesen nicht an.
    #[test]
    fn eine_roehre_ist_keine_datei() {
        let d = tempfile::tempdir().unwrap();
        let w = d.path().canonicalize().unwrap();
        let r = std::ffi::CString::new(w.join("roehre").as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(r.as_ptr(), 0o600) }, 0);
        let f = lesen(&w, &w.join("roehre")).unwrap_err();
        assert!(f.to_string().contains("gewoehnliche"), "{f}");
    }
}
