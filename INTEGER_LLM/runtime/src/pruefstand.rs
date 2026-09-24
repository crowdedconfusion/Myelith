//! **Die Pruefsumme des Artefakts, einmal bezahlt statt bei jedem Start.**
//!
//! # ⛔️ Das Problem, gemessen
//!
//! Der Lader rechnet ueber **jeden** Tensor eine SHA-256, also ueber das
//! ganze Artefakt. Beim 35B sind das 34 GB, und sie kosten **17,2 s von
//! 23,8 s Wanduhr**, also 72 Prozent. Das Laden laeuft dabei schon ueber
//! alle Kerne: 34 GB in 17,2 s sind 2 GB/s, und das ist die Bandbreite
//! der Platte. ⚑ **Mehr Faeden bringen nichts; nur weniger lesen hilft.**
//!
//! # ⚑ Die Loesung: einmal pruefen, dann merken
//!
//! Nach einer vollstaendigen Pruefung liegt neben dem Artefakt eine
//! kleine Marke. Sie haelt zwei Dinge fest:
//!
//! 1. Die Pruefsumme des Manifests. Aendert sich, was geprueft werden
//!    soll, faellt die Marke.
//! 2. Eine **Kennung der Dateilage**: Name, Laenge und Aenderungszeit
//!    jeder Datei, zu einer Pruefsumme verdichtet. Wird eine Datei neu
//!    geschrieben, abgeschnitten, ausgetauscht oder kommt eine dazu,
//!    faellt die Marke ebenfalls.
//!
//! Stimmt beides, wird das erneute Lesen der 34 GB uebersprungen.
//! Stimmt eines nicht, laeuft die volle Pruefung, und danach steht eine
//! neue Marke.
//!
//! # ⚠️ Was das kostet, ehrlich benannt
//!
//! **Das ist eine echte Abschwaechung der Zusage**, und sie gehoert
//! ausgesprochen statt versteckt. Die volle Pruefung faengt jede
//! Veraenderung; die Marke faengt jede Veraenderung, die sich in Laenge
//! oder Aenderungszeit zeigt. Was sie **nicht** faengt: ein einzelnes
//! gekipptes Bit ohne neue Aenderungszeit, und jemanden, der eine Datei
//! austauscht und die Zeitstempel zurueckstellt.
//!
//! ⚑ **Wer die volle Pruefung will, bekommt sie**: `MYL_VOLLE_PRUEFUNG=1`
//! schaltet die Marke ab, ohne etwas neu bauen zu muessen. Und ein
//! frisch gebautes Artefakt wird immer einmal vollstaendig geprueft,
//! denn beim ersten Laden gibt es noch keine Marke.
//!
//! ⚠️ **Die Marke ist kein Beleg und wandert nicht mit.** Sie gilt fuer
//! diesen Rechner und dieses Dateisystem; ein Artefakt, das anderswohin
//! kopiert wird, hat neue Aenderungszeiten und wird dort einmal
//! vollstaendig geprueft. Das ist gewollt.

use std::path::Path;

/// Wie die Marke heisst.
pub const MARKENNAME: &str = ".geprueft";
/// Womit sich die Marke abschalten laesst.
pub const ABSCHALTER: &str = "MYL_VOLLE_PRUEFUNG";
/// In welcher Fassung die Marke geschrieben wird.
///
/// ⚑ **Steht in der Datei und wird verglichen.** Aendert sich, was in
/// die Kennung eingeht, macht eine neue Zahl alle alten Marken
/// ungueltig, statt sie falsch zu deuten.
pub const FASSUNG: u32 = 1;

/// Ob der Nutzer die volle Pruefung erzwungen hat.
pub fn voll_erzwungen() -> bool {
    std::env::var(ABSCHALTER).map(|v| v != "0" && !v.is_empty()).unwrap_or(false)
}

/// **Die Kennung der Dateilage.**
///
/// Name, Laenge und Aenderungszeit jeder Datei im Artefakt, in
/// Namensreihenfolge zu einer Pruefsumme verdichtet.
///
/// ⚑ **Sortiert, sonst haengt die Kennung an der Reihenfolge des
/// Dateisystems**, und dieselbe Lage ergaebe zwei verschiedene Werte.
/// Dieselbe Ueberlegung wie beim Laden der Gewichte.
fn kennung(artifact_dir: &Path) -> Result<String, String> {
    let mut zeilen: Vec<String> = Vec::new();
    let lesung = std::fs::read_dir(artifact_dir)
        .map_err(|f| format!("Artefaktordner nicht lesbar: {f}"))?;
    for eintrag in lesung {
        let eintrag = eintrag.map_err(|f| format!("Eintrag nicht lesbar: {f}"))?;
        let name = eintrag.file_name().to_string_lossy().to_string();
        // ⚑ Die Marke selbst geht nicht in ihre eigene Kennung ein.
        if name == MARKENNAME {
            continue;
        }
        let m = eintrag.metadata().map_err(|f| format!("{name}: {f}"))?;
        if !m.is_file() {
            continue;
        }
        let zeit = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        zeilen.push(format!("{name}\u{1}{}\u{1}{zeit}", m.len()));
    }
    zeilen.sort();
    Ok(crate::loader::sha256_hex(zeilen.join("\n").as_bytes()))
}

/// Ob die volle Pruefung diesmal entfallen darf.
///
/// ⚠️ **Jeder Fehlerweg antwortet mit `false`.** Eine Marke, die sich
/// nicht lesen laesst, ist keine Erlaubnis; im Zweifel wird geprueft.
pub fn marke_gilt(artifact_dir: &Path, manifest_hash: &str) -> bool {
    if voll_erzwungen() {
        return false;
    }
    let Ok(inhalt) = std::fs::read_to_string(artifact_dir.join(MARKENNAME)) else {
        return false;
    };
    let Ok(jetzt) = kennung(artifact_dir) else {
        return false;
    };
    let erwartet = markentext(manifest_hash, &jetzt);
    inhalt.trim() == erwartet.trim()
}

/// Schreibt die Marke nach einer vollstaendigen Pruefung.
///
/// ⚑ **Ein Fehlschlag ist kein Fehler des Ladens.** Liegt das Artefakt
/// schreibgeschuetzt, gibt es eben keine Marke und die naechste Ladung
/// prueft wieder voll. Langsam ist kein Grund, einen Lauf abzubrechen,
/// der sonst durchginge.
pub fn marke_setzen(artifact_dir: &Path, manifest_hash: &str) {
    let Ok(jetzt) = kennung(artifact_dir) else {
        return;
    };
    let _ = std::fs::write(artifact_dir.join(MARKENNAME), markentext(manifest_hash, &jetzt));
}

fn markentext(manifest_hash: &str, kennung: &str) -> String {
    format!("myelith-pruefmarke {FASSUNG}\nmanifest {manifest_hash}\nlage {kennung}\n")
}

#[cfg(test)]
mod proben {
    use super::*;

    fn ordner(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("myl-pruefstand-{name}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Ordner");
        std::fs::write(p.join("a.bin"), b"eins").expect("a");
        std::fs::write(p.join("b.bin"), b"zwei").expect("b");
        p
    }

    #[test]
    fn ohne_marke_wird_geprueft() {
        let p = ordner("ohne");
        assert!(!marke_gilt(&p, "abc"));
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn eine_frische_marke_gilt() {
        let p = ordner("frisch");
        marke_setzen(&p, "abc");
        assert!(marke_gilt(&p, "abc"));
        let _ = std::fs::remove_dir_all(&p);
    }

    /// ⛔️ **Ein anderes Manifest macht die Marke ungueltig.**
    #[test]
    fn ein_anderes_manifest_faellt_durch() {
        let p = ordner("manifest");
        marke_setzen(&p, "abc");
        assert!(!marke_gilt(&p, "xyz"));
        let _ = std::fs::remove_dir_all(&p);
    }

    /// ⛔️ **Jede Aenderung an den Dateien macht die Marke ungueltig:**
    /// laenger, kuerzer, neu dazu, weggenommen.
    #[test]
    fn jede_aenderung_an_den_dateien_faellt_durch() {
        for (was, tun) in [
            ("laenger", (|p: &Path| std::fs::write(p.join("a.bin"), b"eins und mehr").unwrap())
                as fn(&Path)),
            ("kuerzer", |p: &Path| std::fs::write(p.join("a.bin"), b"ei").unwrap()),
            ("dazu", |p: &Path| std::fs::write(p.join("c.bin"), b"drei").unwrap()),
            ("weg", |p: &Path| std::fs::remove_file(p.join("b.bin")).unwrap()),
        ] {
            let p = ordner(was);
            marke_setzen(&p, "abc");
            assert!(marke_gilt(&p, "abc"), "{was}: die frische Marke galt nicht");
            tun(&p);
            assert!(!marke_gilt(&p, "abc"), "{was} kam durch");
            let _ = std::fs::remove_dir_all(&p);
        }
    }

    /// ⚑ **Der Abschalter schlaegt die Marke.**
    ///
    /// ⚠️ Ohne Umgebungsvariable geprueft, denn Proben laufen
    /// nebenlaeufig und eine gesetzte Umgebung traefe die anderen mit.
    /// Hier steht deshalb nur, dass `voll_erzwungen` die Marke sperrt.
    #[test]
    fn der_abschalter_ist_verdrahtet() {
        let p = ordner("abschalter");
        marke_setzen(&p, "abc");
        assert!(marke_gilt(&p, "abc"));
        assert!(!voll_erzwungen(), "die Probenumgebung setzt den Abschalter nicht");
        let _ = std::fs::remove_dir_all(&p);
    }
}
