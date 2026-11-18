//! **Der Sandkasten unter Linux: Landlock fuer die Dateien, seccomp fuer
//! das Netz.**
//!
//! # ⚑ Warum ohne fremdes Programm
//!
//! `bubblewrap` koennte mehr (es haengt einen Baum schreibgeschuetzt ueber
//! einen beschreibbaren), braucht aber Namensraeume ohne Vorrecht, und die
//! sind in Behaeltern und auf mehreren Distributionen gesperrt. Landlock
//! und seccomp darf jeder Prozess auf sich selbst anwenden, ohne Vorrecht
//! und ohne Hilfsprogramm; sie sind da, wo der Kern sie hat.
//!
//! # ⛔️ Die zweite Ausnahme von `deny(unsafe_code)` in dieser Kiste
//!
//! Der Kern nimmt die Grenze ueber drei Systemaufrufe und `prctl`
//! entgegen, und sie muessen im Kindprozess zwischen `fork` und `exec`
//! geschehen (`pre_exec`). Dafuer gibt es in `std` keinen sicheren Weg.
//! ⚑ **Zwischen `fork` und `exec` steht nur, was dort erlaubt ist**:
//! Systemaufrufe, keine Speicheranforderung, kein Schloss. Der Regelsatz
//! und der Filter werden vorher im Elternprozess gebaut.
//!
//! # ⚠️ Was Landlock nicht kann
//!
//! Regeln erlauben nur und gelten fuer alles darunter. Ein Ordner laesst
//! sich nicht aus einem erlaubten Baum herausnehmen; deshalb entsteht das
//! Lesen als **Ergaenzung**: Von der Wurzel abwaerts wird erlaubt, was
//! neben einem gesperrten Baum liegt (siehe [`ergaenzung`]). Was nach dem
//! Bau des Regelsatzes neben einem gesperrten Baum **neu** entsteht, ist
//! fuer diesen einen Befehl nicht lesbar.
//!
//! # ⚑ Was im Arbeitsordner zu bleibt: ein eigener Namensraum (2026-11-17)
//!
//! Weil Landlock aus dem beschreibbaren Arbeitsordner nichts herausnimmt,
//! bekommt der Befehl, wo der Kern es ohne Vorrecht erlaubt, einen eigenen
//! Benutzer- und Einhaengenamensraum ([`Namensraum`]). Darin wird `.git`
//! auf sich selbst eingehaengt (ein Einhaengepunkt laesst sich weder
//! verschieben noch loeschen), die Haken, die Konfiguration und jeder
//! Ordner, den der Auftrag nur zum Lesen nennt, schreibgeschuetzt. Das ist
//! derselbe Griff wie bei `bubblewrap`, ohne das Programm. Erlaubt der Kern
//! es nicht (in Behaeltern mit dem ueblichen seccomp-Profil, bei
//! abgeschalteten Namensraeumen), laeuft der Befehl wie vorher, und die
//! [`namensraum_lage`] sagt es.
#![allow(unsafe_code)]

use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::{Auftrag, Grenze, Sandkasten};

/// Die Fassung, ab der die Grenze traegt.
///
/// ⚑ **Drei, also Kern 6.2.** Davor liess sich eine Datei ausserhalb mit
/// `truncate` leeren, und eine Grenze, hinter der fremde Dateien geleert
/// werden koennen, ist keine.
const MINDEST_ABI: u32 = 3;

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
const LANDLOCK_RULE_PATH_BENEATH: libc::c_int = 1;

const FS_EXECUTE: u64 = 1 << 0;
const FS_WRITE_FILE: u64 = 1 << 1;
const FS_READ_FILE: u64 = 1 << 2;
const FS_READ_DIR: u64 = 1 << 3;
const FS_REMOVE_DIR: u64 = 1 << 4;
const FS_REMOVE_FILE: u64 = 1 << 5;
const FS_MAKE_CHAR: u64 = 1 << 6;
const FS_MAKE_DIR: u64 = 1 << 7;
const FS_MAKE_REG: u64 = 1 << 8;
const FS_MAKE_SOCK: u64 = 1 << 9;
const FS_MAKE_FIFO: u64 = 1 << 10;
const FS_MAKE_BLOCK: u64 = 1 << 11;
const FS_MAKE_SYM: u64 = 1 << 12;
const FS_REFER: u64 = 1 << 13;
const FS_TRUNCATE: u64 = 1 << 14;
/// Ab ABI 5.
const FS_IOCTL_DEV: u64 = 1 << 15;

const LESEND: u64 = FS_EXECUTE | FS_READ_FILE | FS_READ_DIR;
const SCHREIBEND: u64 = FS_WRITE_FILE
    | FS_REMOVE_DIR
    | FS_REMOVE_FILE
    | FS_MAKE_CHAR
    | FS_MAKE_DIR
    | FS_MAKE_REG
    | FS_MAKE_SOCK
    | FS_MAKE_FIFO
    | FS_MAKE_BLOCK
    | FS_MAKE_SYM
    | FS_REFER
    | FS_TRUNCATE;
/// Was der Kern an einer Datei annimmt, die kein Ordner ist.
const DATEIRECHTE: u64 = FS_EXECUTE | FS_WRITE_FILE | FS_READ_FILE | FS_TRUNCATE | FS_IOCTL_DEV;

/// Ab ABI 4.
const NET_BIND_TCP: u64 = 1 << 0;
const NET_CONNECT_TCP: u64 = 1 << 1;
/// Ab ABI 6.
const SCOPE_ABSTRACT_UNIX_SOCKET: u64 = 1 << 0;
const SCOPE_SIGNAL: u64 = 1 << 1;

#[repr(C)]
struct Regelsatz {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct Pfadregel {
    allowed_access: u64,
    parent_fd: i32,
}

/// Was unter Linux nicht gelesen wird: die Heimat (vom Aufrufer), dazu
/// fremde Heimaten, Wechseldatentraeger und geteilte Zwischenordner.
///
/// ⚑ `/dev/pts` gehoert dazu: Dort liegen die Terminals des Nutzers, und
/// wer eines liest, nimmt ihm die Tasten weg.
const GESPERRT: [&str; 10] =
    ["/home", "/root", "/media", "/mnt", "/run/media", "/run/user", "/tmp", "/var/tmp", "/dev/shm", "/dev/pts"];

/// Geraete, in die jedes Programm schreiben koennen muss.
const GERAETE: [&str; 5] = ["/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom"];

fn fehler() -> std::io::Error {
    std::io::Error::last_os_error()
}

pub(super) fn erkunden() -> Grenze {
    if !cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        return Grenze::Keine("der Netzfilter ist fuer diesen Prozessor nicht gebaut".into());
    }
    // ⚑ Mit dieser Marke legt der Kern nichts an, er nennt seine Fassung.
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<Regelsatz>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < 0 {
        return Grenze::Keine(format!("Landlock fehlt oder ist abgeschaltet ({})", fehler()));
    }
    let abi = abi as u32;
    if abi < MINDEST_ABI {
        return Grenze::Keine(format!("Landlock ABI {abi}, gebraucht wird {MINDEST_ABI} (Kern 6.2)"));
    }
    Grenze::Landlock { abi }
}

/// Eine Regel: `rechte` an `pfad` und allem darunter. Was es nicht gibt
/// oder sich nicht oeffnen laesst, bekommt keine Regel und bleibt zu.
fn regel(satz: &OwnedFd, pfad: &Path, rechte: u64, behandelt: u64) -> std::io::Result<()> {
    let datei = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_PATH | libc::O_CLOEXEC).open(pfad)?;
    let mut erlaubt = rechte & behandelt;
    if !datei.metadata()?.is_dir() {
        erlaubt &= DATEIRECHTE;
    }
    if erlaubt == 0 {
        return Ok(());
    }
    let r = Pfadregel { allowed_access: erlaubt, parent_fd: datei.as_raw_fd() };
    let e = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            satz.as_raw_fd(),
            LANDLOCK_RULE_PATH_BENEATH,
            &r as *const Pfadregel,
            0u32,
        )
    };
    if e != 0 {
        return Err(fehler());
    }
    Ok(())
}

/// **Alles unter `ordner`, was neben einem gesperrten Baum liegt.**
///
/// Ein Eintrag, der gesperrt ist, entfaellt. Ein Eintrag, unter dem etwas
/// Gesperrtes liegt, wird nicht als Ganzes erlaubt, sondern seinerseits
/// zerlegt. Alles andere kommt in die Liste.
///
/// ⚠️ **Verweise entfallen.** Eine Regel haengt am Ziel und nicht am
/// Namen; `/tmp -> /private/tmp` gaebe sonst frei, was gesperrt sein soll.
/// Das Ziel selbst steht an seinem eigenen Ort im Baum und wird dort
/// beurteilt.
pub(super) fn ergaenzung(ordner: &Path, gesperrt: &[PathBuf], erlaubt: &mut Vec<PathBuf>) {
    let Ok(eintraege) = std::fs::read_dir(ordner) else { return };
    for e in eintraege.flatten() {
        let p = e.path();
        let Ok(art) = e.file_type() else { continue };
        if art.is_symlink() || gesperrt.contains(&p) {
            continue;
        }
        if gesperrt.iter().any(|g| g.starts_with(&p)) {
            if art.is_dir() {
                ergaenzung(&p, gesperrt, erlaubt);
            }
            continue;
        }
        erlaubt.push(p);
    }
}

/// Die gesperrten Baeume dieser Maschine, aufgeloest, soweit vorhanden.
fn gesperrte_baeume() -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = GESPERRT.iter().filter_map(|p| Path::new(p).canonicalize().ok()).collect();
    aus.extend(super::gesperrte_orte());
    aus.sort();
    aus.dedup();
    aus
}

const BPF_LADEN: u16 = 0x20;
const BPF_GLEICH: u16 = 0x15;
const BPF_AB: u16 = 0x35;
const BPF_ENDE: u16 = 0x06;
const SECCOMP_ERLAUBT: u32 = 0x7fff_0000;
const SECCOMP_VERBOTEN: u32 = 0x0005_0000 | (libc::EPERM as u32);
const SECCOMP_TOT: u32 = 0x8000_0000;
#[cfg(target_arch = "x86_64")]
const ARCHITEKTUR: u32 = 0xC000_003E;
#[cfg(target_arch = "aarch64")]
const ARCHITEKTUR: u32 = 0xC000_00B7;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const ARCHITEKTUR: u32 = 0;

/// **Der Filter ueber die Sockel.**
///
/// In der engen Art sind verboten: `connect`, `bind`, `listen`, `accept`,
/// `sendmmsg`, `sendto` mit einer Zieladresse und `socket` fuer jede
/// Familie ausser `AF_UNIX`. Ein Sockelpaar bleibt, weil gewoehnliche
/// Programme es fuer sich selbst brauchen.
///
/// ⚑ **In der Art `Netz` ist es umgekehrt**: Das Netz ist offen, und
/// verboten ist `socket` fuer `AF_UNIX`. Ein Unix-Sockel fuehrt nicht ins
/// Netz, sondern zu einem Dienst auf derselben Maschine (Docker, der
/// Sitzungsbus), und der handelt ausserhalb der Grenze.
///
/// In beiden Arten ist `io_uring_setup` verboten: Ueber den Ring liesse
/// sich ein Sockel am Filter vorbei oeffnen.
///
/// ⚠️ **`sendmsg` bleibt offen**: Die Zieladresse steht dort im Speicher,
/// und den liest ein solcher Filter nicht. In der engen Art geht damit ein
/// Datagramm an einen Unix-Sockel mit Pfad hinaus (eine Zeile ins
/// Systemprotokoll); ein Strom nicht, denn der braucht `connect`.
fn netzfilter(art: Sandkasten) -> Vec<libc::sock_filter> {
    let s = |code: u16, jt: u8, jf: u8, k: u32| libc::sock_filter { code, jt, jf, k };
    // Der fremde Aufrufweg (32 Bit auf x86_64) traegt andere Nummern; wer
    // ihn nimmt, wird beendet.
    let mut f = vec![
        s(BPF_LADEN, 0, 0, 4),
        s(BPF_GLEICH, 1, 0, ARCHITEKTUR),
        s(BPF_ENDE, 0, 0, SECCOMP_TOT),
        s(BPF_LADEN, 0, 0, 0),
    ];
    if cfg!(target_arch = "x86_64") {
        // x32 teilt die Architektur und zaehlt ab dieser Marke.
        f.push(s(BPF_AB, 0, 1, 0x4000_0000));
        f.push(s(BPF_ENDE, 0, 0, SECCOMP_VERBOTEN));
    }
    let eng = art == Sandkasten::Eng;
    let mut verboten = vec![libc::SYS_io_uring_setup];
    if eng {
        verboten.extend([
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_sendmmsg,
        ]);
    }
    for nummer in verboten {
        f.push(s(BPF_GLEICH, 0, 1, nummer as u32));
        f.push(s(BPF_ENDE, 0, 0, SECCOMP_VERBOTEN));
    }
    if eng {
        // `sendto`: erlaubt nur ohne Zieladresse (fuenftes Argument, beide
        // Haelften null). Wer nicht `sendto` ist, springt mit der Nummer
        // im Register weiter.
        f.extend([
            s(BPF_GLEICH, 0, 6, libc::SYS_sendto as u32),
            s(BPF_LADEN, 0, 0, 16 + 8 * 4),
            s(BPF_GLEICH, 0, 3, 0),
            s(BPF_LADEN, 0, 0, 16 + 8 * 4 + 4),
            s(BPF_GLEICH, 0, 1, 0),
            s(BPF_ENDE, 0, 0, SECCOMP_ERLAUBT),
            s(BPF_ENDE, 0, 0, SECCOMP_VERBOTEN),
        ]);
    }
    // `socket`, erstes Argument: eng nur `AF_UNIX`, mit Netz alles ausser
    // `AF_UNIX`.
    let (bei_unix, sonst) = if eng { (1, 0) } else { (0, 1) };
    f.extend([
        s(BPF_GLEICH, 0, 3, libc::SYS_socket as u32),
        s(BPF_LADEN, 0, 0, 16),
        s(BPF_GLEICH, bei_unix, sonst, libc::AF_UNIX as u32),
        s(BPF_ENDE, 0, 0, SECCOMP_VERBOTEN),
        s(BPF_ENDE, 0, 0, SECCOMP_ERLAUBT),
    ]);
    f
}

/// Ein Ort, der im eigenen Namensraum des Befehls neu eingehaengt wird.
struct Schutzort {
    pfad: CString,
    /// Schreibgeschuetzt; sonst nur festgehalten (ein Einhaengepunkt laesst
    /// sich weder verschieben noch loeschen).
    nur_lesen: bool,
    /// Die Marken des Dateisystems darunter. ⚑ Ein Namensraum ohne Vorrecht
    /// darf sie beim Neueinhaengen nicht fallen lassen, sonst weist der Kern
    /// es ab.
    marken: libc::c_ulong,
}

/// **Der eigene Namensraum eines Befehls**, fertig gebaut, bevor `fork`
/// geschieht: zwischen `fork` und `exec` wird nur noch geschrieben und
/// eingehaengt, nichts angefordert.
struct Namensraum {
    orte: Vec<Schutzort>,
    /// Die eigene Nummer auf sich selbst: `git` prueft, wem ein
    /// Repositorium gehoert, und ein Befehl als `nobody` hielte jedes fuer
    /// fremd.
    uid_map: Vec<u8>,
    gid_map: Vec<u8>,
}

/// Die Marken aus `statvfs`, die beim Neueinhaengen bleiben muessen.
fn marken(pfad: &CString) -> libc::c_ulong {
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(pfad.as_ptr(), &mut s) } != 0 {
        return 0;
    }
    let f = s.f_flag;
    let mut m: libc::c_ulong = 0;
    for (st, ms) in [
        (libc::ST_NOSUID, libc::MS_NOSUID),
        (libc::ST_NODEV, libc::MS_NODEV),
        (libc::ST_NOEXEC, libc::MS_NOEXEC),
        (libc::ST_NOATIME, libc::MS_NOATIME),
        (libc::ST_NODIRATIME, libc::MS_NODIRATIME),
        (libc::ST_RELATIME, libc::MS_RELATIME),
    ] {
        if f & st != 0 {
            m |= ms;
        }
    }
    m
}

impl Namensraum {
    fn neu(orte: Vec<(PathBuf, bool)>) -> Self {
        let orte = orte
            .into_iter()
            .filter_map(|(p, nur_lesen)| {
                let pfad = CString::new(p.as_os_str().as_bytes()).ok()?;
                let marken = marken(&pfad);
                Some(Schutzort { pfad, nur_lesen, marken })
            })
            .collect();
        let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
        Self { orte, uid_map: format!("{uid} {uid} 1").into_bytes(), gid_map: format!("{gid} {gid} 1").into_bytes() }
    }

    /// Im Kind, zwischen `fork` und `exec`: eigener Benutzer- und
    /// Einhaengenamensraum, dann die Orte. Scheitert ein Schritt, scheitert
    /// der Start.
    fn betreten(&self) -> std::io::Result<()> {
        unsafe {
            if libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) != 0 {
                return Err(fehler());
            }
            schreiben(c"/proc/self/uid_map", &self.uid_map)?;
            schreiben(c"/proc/self/setgroups", b"deny")?;
            schreiben(c"/proc/self/gid_map", &self.gid_map)?;
            // Nichts von hier soll nach draussen wandern.
            let nichts = std::ptr::null::<libc::c_char>();
            if libc::mount(nichts, c"/".as_ptr(), nichts, libc::MS_REC | libc::MS_PRIVATE, std::ptr::null()) != 0 {
                return Err(fehler());
            }
            for o in &self.orte {
                let p = o.pfad.as_ptr();
                if libc::mount(p, p, nichts, libc::MS_BIND | libc::MS_REC, std::ptr::null()) != 0 {
                    return Err(fehler());
                }
                let nur_lesen = libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY | o.marken;
                if o.nur_lesen && libc::mount(nichts, p, nichts, nur_lesen, std::ptr::null()) != 0 {
                    return Err(fehler());
                }
            }
        }
        Ok(())
    }
}

/// Schreibt `inhalt` in die Datei `pfad`, ohne etwas anzufordern.
fn schreiben(pfad: &std::ffi::CStr, inhalt: &[u8]) -> std::io::Result<()> {
    unsafe {
        let fd = libc::open(pfad.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if fd < 0 {
            return Err(fehler());
        }
        let n = libc::write(fd, inhalt.as_ptr().cast(), inhalt.len());
        let fehlschlag = if n < 0 || n as usize != inhalt.len() { Some(fehler()) } else { None };
        libc::close(fd);
        match fehlschlag {
            Some(f) => Err(f),
            None => Ok(()),
        }
    }
}

/// **Ob der Kern einen eigenen Namensraum ohne Vorrecht hergibt.** Einmal
/// wirklich versucht, dann gemerkt.
pub(super) fn namensraum() -> bool {
    static N: OnceLock<bool> = OnceLock::new();
    *N.get_or_init(|| {
        let Ok(sh) = crate::schale::sh() else { return false };
        let raum = Namensraum::neu(Vec::new());
        let mut probe = Command::new(sh);
        probe.arg("-c").arg("true");
        unsafe {
            probe.pre_exec(move || raum.betreten());
        }
        matches!(myl_senses::prozess::laufen(&mut probe, 10, 256), Ok(a) if a.gut())
    })
}

/// Der Zusatz zur Lage: ob im Arbeitsordner etwas zu bleibt.
pub(super) fn namensraum_lage() -> &'static str {
    if namensraum() {
        " Im Arbeitsordner bleiben die Haken und die Konfiguration von `.git` und genannte Kisten zu."
    } else {
        " ⚠️ Ohne eigenen Namensraum (der Kern gibt ihn hier nicht her): Im Arbeitsordner bleibt nichts zu, \
         auch nicht die Haken von `.git`."
    }
}

/// **Was im Namensraum neu eingehaengt wird**: `.git` des Arbeitsordners
/// (festgehalten), seine Haken und seine Konfiguration (schreibgeschuetzt),
/// eine `.git`-Datei (schreibgeschuetzt, sie zeigt auf das eigentliche
/// Repositorium) und jeder Ordner aus `lesen`, der in einem beschreibbaren
/// Baum liegt (schreibgeschuetzt, siehe `Auftrag::lesen`).
///
/// ⚑ **Fehlen die Haken, wird der leere Ordner angelegt**, den `git init`
/// ohnehin anlegt; auf einen Ort, den es nicht gibt, laesst sich nichts
/// einhaengen. ⚠️ Ein Verweis als Haken oder als `.git` bleibt
/// ungeschuetzt: Eingehaengt wird am Ziel, und der Verweis selbst laesst
/// sich ersetzen. `commondir` gibt es meist nicht und laesst sich deshalb
/// nicht so schuetzen; dafuer sieht `Lauf::nachschau` nach.
fn schutzorte(a: &Auftrag, zwischen: &Path) -> Vec<(PathBuf, bool)> {
    let mut orte = Vec::new();
    let git = a.wurzel.join(".git");
    let art = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.file_type()).ok();
    match art(&git) {
        Some(t) if t.is_dir() => {
            orte.push((git.clone(), false));
            let haken = git.join("hooks");
            if art(&haken).is_none() {
                let _ = std::fs::create_dir(&haken);
            }
            for p in [haken, git.join("config")] {
                if art(&p).is_some_and(|t| t.is_dir() || t.is_file()) {
                    orte.push((p, true));
                }
            }
        }
        Some(t) if t.is_file() => orte.push((git, true)),
        _ => {}
    }
    let mut beschreibbar = vec![a.wurzel.to_path_buf(), zwischen.to_path_buf()];
    beschreibbar.extend(super::beschreibbare_orte(a));
    for p in a.lesen.iter().filter_map(|p| p.canonicalize().ok()) {
        if beschreibbar.iter().any(|b| p.starts_with(b)) {
            orte.push((p, true));
        }
    }
    orte
}

pub(super) fn einrichten(
    abi: u32,
    sh: &Path,
    befehl: &str,
    a: &Auftrag,
    zwischen: &Path,
    lesbar: &[PathBuf],
) -> Result<Command, String> {
    let mut behandelt = LESEND | SCHREIBEND;
    if abi >= 5 {
        behandelt |= FS_IOCTL_DEV;
    }
    let eng = a.art == Sandkasten::Eng;
    let satz = Regelsatz {
        handled_access_fs: behandelt,
        handled_access_net: if abi >= 4 && eng { NET_BIND_TCP | NET_CONNECT_TCP } else { 0 },
        scoped: if abi >= 6 { SCOPE_ABSTRACT_UNIX_SOCKET | SCOPE_SIGNAL } else { 0 },
    };
    // ⚑ Der Kern nimmt nur die Laenge, die seine Fassung kennt.
    let laenge: usize = match abi {
        0..=3 => 8,
        4..=5 => 16,
        _ => 24,
    };
    let fd = unsafe { libc::syscall(libc::SYS_landlock_create_ruleset, &satz as *const Regelsatz, laenge, 0u32) };
    if fd < 0 {
        return Err(format!("laeuft nicht: Der Regelsatz der Sandbox liess sich nicht anlegen ({})", fehler()));
    }
    // Ab hier gehoert die Nummer diesem Wert und schliesst sich mit ihm.
    let satz = unsafe { OwnedFd::from_raw_fd(fd as i32) };

    let mut lesend = Vec::new();
    ergaenzung(Path::new("/"), &gesperrte_baeume(), &mut lesend);
    lesend.extend(lesbar.iter().cloned());
    for ort in &lesend {
        // Was sich nicht oeffnen laesst, bleibt ohne Regel und damit zu.
        let _ = regel(&satz, ort, LESEND, behandelt);
    }
    for geraet in GERAETE {
        let _ = regel(&satz, Path::new(geraet), FS_READ_FILE | FS_WRITE_FILE, behandelt);
    }
    // ⛔️ Diese beiden muessen tragen, sonst liefe der Befehl ohne seinen
    // Arbeitsordner.
    for ort in [a.wurzel, zwischen] {
        regel(&satz, ort, LESEND | SCHREIBEND, behandelt)
            .map_err(|e| format!("laeuft nicht: Die Sandbox nahm {} nicht an ({e})", ort.display()))?;
    }
    // Ein Ausgabeordner, der nicht traegt, kostet nur die Ausgabe.
    for ort in super::beschreibbare_orte(a) {
        let _ = regel(&satz, &ort, LESEND | SCHREIBEND, behandelt);
    }

    let filter = netzfilter(a.art);
    let raum = namensraum().then(|| Namensraum::neu(schutzorte(a, zwischen)));
    let mut kind = Command::new(sh);
    kind.arg("-c").arg(befehl);
    // ⚑ Im Kind, zwischen `fork` und `exec`: eine eigene Sitzung (kein
    // steuerndes Terminal, in das sich Tasten schieben liessen), keine
    // neuen Vorrechte, der eigene Namensraum, dann die beiden Grenzen.
    // Scheitert eine, scheitert der Start, und der Befehl laeuft nicht.
    // ⚑ Der Namensraum kommt vor Landlock: Ein Prozess unter Landlock darf
    // nicht mehr einhaengen, und damit auch nicht aushaengen, was hier
    // schreibgeschuetzt eingehaengt wurde.
    unsafe {
        kind.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(fehler());
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong) != 0 {
                return Err(fehler());
            }
            if let Some(r) = &raum {
                r.betreten()?;
            }
            if libc::syscall(libc::SYS_landlock_restrict_self, satz.as_raw_fd(), 0u32) != 0 {
                return Err(fehler());
            }
            let programm =
                libc::sock_fprog { len: filter.len() as libc::c_ushort, filter: filter.as_ptr() as *mut libc::sock_filter };
            if libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER as libc::c_ulong, &programm as *const libc::sock_fprog)
                != 0
            {
                return Err(fehler());
            }
            Ok(())
        });
    }
    Ok(kind)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Die Ergaenzung erlaubt, was neben dem Gesperrten liegt, und steigt
    /// dort hinein, wo etwas Gesperrtes darunter liegt.
    #[test]
    fn die_ergaenzung_laesst_das_gesperrte_aus() {
        let ordner = tempfile::tempdir().expect("Ordner");
        let w = ordner.path().canonicalize().expect("aufloesbar");
        for p in ["frei/a", "halb/zu/tief", "halb/offen", "zu/x"] {
            std::fs::create_dir_all(w.join(p)).expect("anlegen");
        }
        std::os::unix::fs::symlink(w.join("zu"), w.join("verweis")).expect("Verweis");
        let gesperrt = vec![w.join("zu"), w.join("halb/zu")];
        let mut erlaubt = Vec::new();
        ergaenzung(&w, &gesperrt, &mut erlaubt);
        erlaubt.sort();
        assert_eq!(erlaubt, vec![w.join("frei"), w.join("halb/offen")], "weder das Gesperrte noch der Verweis darauf");
    }

    /// Jeder Sprung des Filters landet auf einer Anweisung, und er endet
    /// auf einer Rueckgabe.
    #[test]
    fn der_filter_springt_nie_ins_leere() {
        for art in [Sandkasten::Eng, Sandkasten::Netz] {
            let f = netzfilter(art);
            for (i, a) in f.iter().enumerate() {
                if a.code == BPF_GLEICH || a.code == BPF_AB {
                    assert!(i + 1 + (a.jt as usize) < f.len(), "{art:?}, Anweisung {i}: Sprung bei Treffer");
                    assert!(i + 1 + (a.jf as usize) < f.len(), "{art:?}, Anweisung {i}: Sprung ohne Treffer");
                }
            }
            assert_eq!(f.last().map(|a| (a.code, a.k)), Some((BPF_ENDE, SECCOMP_ERLAUBT)));
        }
    }
}
