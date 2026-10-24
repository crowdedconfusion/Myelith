//! **Eine Datei, die nur ihr Besitzer lesen darf**, in einem Zug abgelegt.
//!
//! # ⚑ Warum es diese eine Stelle gibt
//!
//! 📌 **Fund 607 (2026-10-23):** Die Knotenidentitaet (`myl-net`) und der
//! Validatorschluessel (`myl-node`) wurden mit `fs::write` angelegt und
//! danach auf `0600` gesetzt. Zwischen beidem lag die Datei mit den
//! Vorgaberechten des Systems (meist `0644`) da, lesbar fuer jeden auf dem
//! Rechner. `ortsleitung::schluessel_ablegen` machte es richtig und sagte
//! warum; die anderen beiden hatten es nicht gelesen. Jetzt nehmen alle
//! drei diese Funktion.
//!
//! # Was sie zusichert
//!
//! 1. **Die Rechte gelten ab dem ersten Byte.** Unter Unix wird die
//!    Zwischendatei mit `0600` **angelegt** (`OpenOptions::mode`), nicht
//!    nachtraeglich eingeschraenkt.
//! 2. **Eine liegengebliebene Zwischendatei zaehlt nicht.** Sie wird
//!    vorher entfernt und neu angelegt (`create_new`), denn
//!    `OpenOptions::mode` wirkt nur beim Anlegen: Eine alte mit `0644`
//!    behielte ihre Rechte.
//! 3. **Nie eine halbe Datei.** Erst geschrieben und auf die Platte
//!    gebracht, dann umbenannt; das Umbenennen ersetzt die alte Datei samt
//!    ihren Rechten.
//!
//! ⚠️ **Ausserhalb von Unix gibt es diese Rechte nicht.** Der Rueckgabewert
//! sagt dann `false`, damit der Aufrufer es melden kann, statt eine
//! Sicherung vorzutaeuschen.

use std::io::Write;
use std::path::Path;

/// Schreibt `inhalt` nach `pfad`, unter Unix mit Rechten `0600` ab dem
/// Anlegen. Gibt zurueck, ob die Datei so geschuetzt ist.
pub fn schreiben(pfad: &Path, inhalt: &[u8]) -> std::io::Result<bool> {
    let (mut datei, zwischen) = zwischendatei_anlegen(pfad)?;
    let geschrieben = datei.write_all(inhalt).and_then(|()| datei.sync_all());
    drop(datei);
    if let Err(e) = geschrieben.and_then(|()| std::fs::rename(&zwischen, pfad)) {
        let _ = std::fs::remove_file(&zwischen);
        return Err(e);
    }
    Ok(cfg!(unix))
}

/// Legt die Zwischendatei neben `pfad` frisch an, unter Unix mit `0600`.
/// Eigene Funktion, damit sich pruefen laesst, was vor dem ersten Byte
/// gilt (Zusagen 1 und 2).
fn zwischendatei_anlegen(pfad: &Path) -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    let mut name = pfad.file_name().unwrap_or_default().to_os_string();
    name.push(".unfertig");
    let zwischen = pfad.with_file_name(name);
    match std::fs::remove_file(&zwischen) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut opt = std::fs::OpenOptions::new();
    opt.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opt.mode(0o600);
    }
    let datei = opt.open(&zwischen)?;
    Ok((datei, zwischen))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordner(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-geheimdatei-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    #[test]
    fn schreibt_und_ersetzt() {
        let d = ordner("ersetzt");
        let p = d.join("schluessel");
        assert_eq!(schreiben(&p, b"eins").expect("schreiben"), cfg!(unix));
        schreiben(&p, b"zwei").expect("ersetzen");
        assert_eq!(std::fs::read(&p).expect("lesen"), b"zwei");
        assert!(!d.join("schluessel.unfertig").exists(), "Zwischendatei liegt noch");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⚑ Die Gegenprobe zu Zusage 1 und 2: Eine liegengebliebene
    /// Zwischendatei mit `0644`. Die neue traegt `0600`, **bevor ein Byte
    /// geschrieben ist**. Mit `fs::write` und `chmod` danach waere sie hier
    /// noch `0644`.
    #[cfg(unix)]
    #[test]
    fn die_rechte_gelten_vor_dem_ersten_byte() {
        use std::os::unix::fs::PermissionsExt;
        let d = ordner("rechte");
        let p = d.join("schluessel");
        let z = d.join("schluessel.unfertig");
        std::fs::write(&z, b"alt").expect("anlegen");
        std::fs::set_permissions(&z, std::fs::Permissions::from_mode(0o644)).expect("Rechte");
        let (datei, zwischen) = zwischendatei_anlegen(&p).expect("anlegen");
        assert_eq!(zwischen, z);
        let modus = datei.metadata().expect("meta").permissions().mode() & 0o777;
        assert_eq!(modus, 0o600, "die Zwischendatei lag mit {modus:o} offen");
        assert_eq!(datei.metadata().expect("meta").len(), 0, "die alte Zwischendatei wurde weiterbenutzt");
        drop(datei);
        schreiben(&p, b"neu").expect("schreiben");
        let modus = std::fs::metadata(&p).expect("meta").permissions().mode() & 0o777;
        assert_eq!(modus, 0o600, "Rechte waren {modus:o}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
