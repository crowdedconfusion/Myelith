//! Der Plattenplatz, den Myelith wirklich haelt, solange es laeuft.
//!
//! # ⚑ Warum eine Datei und keine Zahl
//!
//! Eine Obergrenze sagt „ich nehme mir nicht mehr als das". Sie sagt
//! **nicht** „das gehoert mir": Ein anderes Programm fuellt die Platte,
//! und der freigegebene Platz ist weg, ohne dass hier jemand etwas
//! falsch gemacht hat.
//!
//! ⚑ **Eine belegte Datei sagt das Zweite.** Sie nimmt dem System den
//! Platz wirklich weg, und zwar so lange, wie das Programm laeuft.
//!
//! # ⛑ Die Rechnung, die dahinter stehen muss
//!
//! **Eine Reservierung ohne Buchhaltung arbeitet gegen den eigenen
//! Download.** Wer 50 GiB freigibt und 50 GiB belegt, hat fuer das
//! erste Modell keinen Platz mehr, obwohl er ihn gerade dafuer
//! freigegeben hat. Es gilt deshalb durchgehend:
//!
//! ```text
//! belegt (Modelle und Artefakte) + reserviert (diese Datei) = Freigabe
//! ```
//!
//! Vor einem Download schrumpft die Reservierung um das, was er braucht,
//! und der Platz geht an ihn ueber. Die **Summe** bleibt gleich, und
//! genau sie ist die Zusage.
//!
//! # ⛑ Zwei Systeme, ein Zweck, zwei Wege
//!
//! `set_len` allein reicht nicht ueberall, und der Unterschied ist der
//! ganze Punkt dieses Moduls:
//!
//! | | belegt `set_len` wirklich? |
//! |---|---|
//! | Windows (NTFS) | **ja**, `SetEndOfFile` bucht die Bloecke |
//! | Linux, macOS | **nein**, es entstuende eine Datei mit Loechern, die null Bytes belegt |
//!
//! Unter Unix wird deshalb ausdruecklich vorbelegt: `posix_fallocate`
//! unter Linux, `fcntl(F_PREALLOCATE)` unter macOS. Beides bucht die
//! Bloecke, ohne sie zu beschreiben, und ist deshalb in Sekunden fertig
//! statt in Minuten.

use std::path::{Path, PathBuf};

/// Der Dateiname, unter dem der Platz gehalten wird.
///
/// ⚑ **Mit einem Punkt am Anfang und einer sprechenden Endung.** Wer
/// sie im Dateiverwalter findet, soll ohne Nachschlagen wissen, was sie
/// ist und dass sie weg darf.
pub const DATEINAME: &str = ".myelith-reserviert";

/// Der gehaltene Platz.
///
/// ⚑ **Die Datei verschwindet, wenn dieser Wert stirbt.** Damit ist
/// „solange das Programm geoeffnet ist" keine Absichtserklaerung,
/// sondern die Lebensdauer eines Wertes.
#[derive(Debug)]
pub struct Reservierung {
    pfad: PathBuf,
    rest: u64,
}

impl Reservierung {
    /// Haelt `bytes` unter `datenort` fest.
    ///
    /// ⛑ **Eine liegengebliebene Datei wird uebernommen, nicht
    /// ergaenzt.** Nach einem Absturz steht sie noch da; wer daneben
    /// eine zweite anlegte, hielte den Platz doppelt.
    pub fn anlegen(datenort: &Path, bytes: u64) -> Result<Self, String> {
        let pfad = datenort.join(DATEINAME);
        std::fs::create_dir_all(datenort)
            .map_err(|e| format!("{}: {e}", datenort.display()))?;
        let mut r = Self { pfad, rest: 0 };
        r.setzen(bytes)?;
        Ok(r)
    }

    /// Wie viele Bytes gerade gehalten werden.
    pub fn rest(&self) -> u64 {
        self.rest
    }

    /// Wo die Datei liegt.
    pub fn pfad(&self) -> &Path {
        &self.pfad
    }

    /// Gibt `bytes` heraus, damit Myelith sie selbst belegen kann.
    ///
    /// ⚑ **Der Rueckgabewert ist, was wirklich frei wurde.** Wer mehr
    /// verlangt, als gehalten wird, bekommt alles, was da ist, und die
    /// Zahl sagt es ihm. Ein Fehlschlag waere hier die schlechtere
    /// Antwort: Die Reservierung ist eine Zusage an den Nutzer, keine
    /// Schranke gegen ihn.
    pub fn hergeben(&mut self, bytes: u64) -> Result<u64, String> {
        let heraus = bytes.min(self.rest);
        self.setzen(self.rest - heraus)?;
        Ok(heraus)
    }

    /// Nimmt `bytes` wieder in die Reservierung auf.
    ///
    /// Fuer den Fall, dass ein Download scheitert: Der Platz, den er
    /// bekommen hat, gehoert dann wieder gehalten.
    pub fn zurueck(&mut self, bytes: u64) -> Result<(), String> {
        self.setzen(self.rest.saturating_add(bytes))
    }

    /// Setzt die gehaltene Menge auf genau `bytes`.
    pub fn setzen(&mut self, bytes: u64) -> Result<(), String> {
        if bytes == 0 {
            // Null gehalten heisst: keine Datei. Eine leere Datei
            // stehenzulassen waere ein Rest ohne Zweck.
            let _ = std::fs::remove_file(&self.pfad);
            self.rest = 0;
            return Ok(());
        }
        let datei = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.pfad)
            .map_err(|e| format!("{}: {e}", self.pfad.display()))?;
        belegen(&datei, bytes)?;
        self.rest = bytes;
        Ok(())
    }
}

impl Drop for Reservierung {
    fn drop(&mut self) {
        // ⛑ **Ein Fehlschlag beim Aufraeumen darf nicht abbrechen.**
        // Ein `Drop`, der in Panik geraet, waehrend schon einer laeuft,
        // beendet den Prozess hart. Bleibt die Datei liegen, uebernimmt
        // sie der naechste Start.
        let _ = std::fs::remove_file(&self.pfad);
    }
}

/// Bucht `bytes` fuer diese Datei, ohne sie zu beschreiben.
#[cfg(target_os = "linux")]
fn belegen(datei: &std::fs::File, bytes: u64) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;
    // Schrumpfen kann `posix_fallocate` nicht, das macht `set_len`.
    let jetzt = datei.metadata().map_err(|e| e.to_string())?.len();
    if bytes < jetzt {
        return datei.set_len(bytes).map_err(|e| e.to_string());
    }
    // SICHERHEIT: gueltiger Deskriptor, Laenge passt in i64.
    let r = unsafe { libc::posix_fallocate(datei.as_raw_fd(), 0, bytes as libc::off_t) };
    if r != 0 {
        return Err(format!("Platz laesst sich nicht belegen: Fehler {r}"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn belegen(datei: &std::fs::File, bytes: u64) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;
    let fd = datei.as_raw_fd();
    let jetzt = datei.metadata().map_err(|e| e.to_string())?.len();
    if bytes < jetzt {
        return datei.set_len(bytes).map_err(|e| e.to_string());
    }
    let fehlt = (bytes - jetzt) as libc::off_t;
    if fehlt > 0 {
        // ⚑ **Erst zusammenhaengend versuchen, dann irgendwie.** Ein
        // zusammenhaengender Bereich ist der schnellere; scheitert er
        // an der Zerstueckelung der Platte, ist ein verteilter genauso
        // viel wert, denn gehalten wird Platz und keine Bahn.
        let mut fst = libc::fstore_t {
            fst_flags: libc::F_ALLOCATECONTIG,
            fst_posmode: libc::F_PEOFPOSMODE,
            fst_offset: 0,
            fst_length: fehlt,
            fst_bytesalloc: 0,
        };
        // SICHERHEIT: gueltiger Deskriptor, `fst` ist vollstaendig
        // gesetzt und lebt ueber den Aufruf hinaus.
        let mut r = unsafe { libc::fcntl(fd, libc::F_PREALLOCATE, &mut fst) };
        if r == -1 {
            fst.fst_flags = libc::F_ALLOCATEALL;
            r = unsafe { libc::fcntl(fd, libc::F_PREALLOCATE, &mut fst) };
        }
        if r == -1 {
            return Err(format!(
                "Platz laesst sich nicht belegen: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    // ⛑ **`F_PREALLOCATE` bucht die Bloecke, verlaengert die Datei
    // aber nicht.** Ohne dieses `set_len` blieben sie gebucht und die
    // Datei stuende mit null Bytes da, also genau der Zustand, der wie
    // ein Fehlschlag aussieht und keiner ist.
    datei.set_len(bytes).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn belegen(datei: &std::fs::File, bytes: u64) -> Result<(), String> {
    // ⚑ **Hier genuegt `set_len`.** Es endet in `SetEndOfFile`, und
    // NTFS bucht die Bloecke dabei wirklich; die Nullen schreibt das
    // Dateisystem erst beim Lesen. Kein `libc`, kein `windows-sys`.
    datei.set_len(bytes).map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn belegen(_datei: &std::fs::File, _bytes: u64) -> Result<(), String> {
    Err("Auf diesem System ist keine Reservierung umgesetzt".to_string())
}

/// Wie viel eine Reservierung halten muss.
///
/// ⚑ **Die Zusage ist die Summe, nicht der Rest.** Wer 50 GiB freigibt
/// und 30 belegt hat, haelt 20; wer mehr belegt hat als er freigibt,
/// haelt nichts, und das ist richtig so: Nachtraeglich Platz
/// wegzunehmen, der schon in Modellen steckt, ginge nur durch Loeschen.
pub fn zu_halten(freigabe_bytes: u64, belegt_bytes: u64) -> u64 {
    freigabe_bytes.saturating_sub(belegt_bytes)
}

/// Was ein Verzeichnisbaum belegt, in Bytes.
///
/// ⚑ **Verweisen wird nicht gefolgt.** Ein Verweis auf ein Modell
/// ausserhalb zaehlte sonst als Belegung dieses Datentraegers, und die
/// Rechnung stimmte nicht mehr.
pub fn belegung(wurzel: &Path) -> u64 {
    fn zaehlen(p: &Path, summe: &mut u64) {
        let Ok(eintraege) = std::fs::read_dir(p) else { return };
        for e in eintraege.flatten() {
            let Ok(art) = e.file_type() else { continue };
            if art.is_symlink() {
                continue;
            }
            if art.is_dir() {
                zaehlen(&e.path(), summe);
            } else if let Ok(m) = e.metadata() {
                *summe = summe.saturating_add(m.len());
            }
        }
    }
    let mut summe = 0;
    zaehlen(wurzel, &mut summe);
    summe
}
