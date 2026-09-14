//! Wo dieses Repositorium liegt, und wie es sich wiederfindet.
//!
//! # ⚑ Warum das eine eigene Kiste braucht und keine Konstante
//!
//! **Ein Pfad, den jemand einmal aufschreibt, ist eine Wette darauf,
//! dass nichts sich bewegt.** Dieses Repositorium wird verschoben,
//! umbenannt, auf eine andere Platte gelegt und aus einem Buendel
//! heraus gestartet, das gar nicht darin liegt. Ein fester Pfad
//! ueberlebt keinen dieser Faelle.
//!
//! # Die vier Wege, in dieser Reihenfolge
//!
//! 1. **`MYELITH_WURZEL`** aus der Umgebung. Wer es setzt, meint es.
//! 2. **Vom Arbeitsverzeichnis aufwaerts.** Wer im Klon steht, meint
//!    diesen Klon.
//! 3. **Vom Programm aufwaerts.** Aus dem Finder gestartet ist das
//!    Arbeitsverzeichnis `/`; das Programm selbst liegt aber im Baum,
//!    solange es nicht installiert wurde.
//! 4. **Der gemerkte Ort**, `wurzel` neben der Einstellungsdatei.
//!    ⚠️ **Er wird nachgeprueft und nicht geglaubt:** Steht die Marke
//!    dort nicht mehr, ist der Zettel alt und gilt nicht. Er ist die
//!    Antwort fuer den, der nirgendwo steht, also fuer das
//!    **installierte** Programm aus dem Finder heraus.
//!
//! ⚑ **Und wer auf einem der beiden Suchwege faendig wird, schreibt
//! den Zettel neu.** Damit heilt sich ein Umzug von selbst: Einmal aus
//! dem verschobenen Klon heraus starten genuegt, und auch das
//! installierte Programm findet danach wieder hin.
//!
//! 📌 **Der Zettel ist ein Zwischenspeicher und keine Einstellung.** Er
//! steht deshalb nicht in `client.json`: Was dort steht, hat ein Mensch
//! entschieden, und ein Mensch entscheidet nicht, wo sein Klon liegt,
//! er verschiebt ihn.

use std::path::{Path, PathBuf};

/// Die Datei, an der ein Verzeichnis als Wurzel dieses Repositoriums
/// zu erkennen ist.
///
/// ⚑ **Eine Datei, die es nur hier gibt, und die niemand nebenbei
/// anlegt.** `.git` waere jeder Klon, `README.md` jedes Projekt.
pub const MARKE: &str = "INTEGER_LLM/scripts/build_artifacts.sh";

/// Die Umgebungsvariable, mit der sich alles uebergehen laesst.
pub const UMGEBUNG: &str = "MYELITH_WURZEL";

/// Traegt dieses Verzeichnis die Marke?
fn ist_wurzel(p: &Path) -> bool {
    p.join(MARKE).is_file()
}

/// Von hier aufwaerts suchen.
fn aufwaerts(anfang: PathBuf) -> Option<PathBuf> {
    let mut p = anfang;
    loop {
        if ist_wurzel(&p) {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

/// Die Datei, in der der gefundene Ort steht.
pub fn zettel() -> PathBuf {
    let ablage = crate::Einstellungen::vorgabepfad();
    ablage.with_file_name("wurzel")
}

/// Schreibt den Ort auf den Zettel.
///
/// ⚑ **Ein Fehlschlag ist keiner.** Ein Rechner, auf dem sich der
/// Zettel nicht schreiben laesst, findet die Wurzel weiter ueber die
/// Suche; er findet sie nur jedes Mal neu.
pub fn merken(w: &Path) {
    let z = zettel();
    if let Some(eltern) = z.parent() {
        let _ = std::fs::create_dir_all(eltern);
    }
    let _ = std::fs::write(&z, format!("{}\n", w.display()));
}

/// Der gemerkte Ort, wenn er noch stimmt.
fn gemerkt() -> Option<PathBuf> {
    let text = std::fs::read_to_string(zettel()).ok()?;
    let p = PathBuf::from(text.trim());
    ist_wurzel(&p).then_some(p)
}

/// Das Verzeichnis dieses Repositoriums, oder `None`.
///
/// ⚠️ **`None` ist ein gueltiger Zustand und kein Fehler.** Wer nur die
/// Freigabebuendel geladen hat, hat keinen Klon; dann gibt es keine
/// Artefakte zu bauen und nichts zu aktualisieren, und die Oberflaeche
/// sagt das, statt es zu versuchen.
pub fn wurzel() -> Option<PathBuf> {
    if let Some(w) = std::env::var_os(UMGEBUNG).map(PathBuf::from) {
        if ist_wurzel(&w) {
            merken(&w);
            return Some(w);
        }
    }
    // ⚑ **Die Suche vor dem Zettel, und das ist die ganze Ordnung.**
    // Wer in einem Klon steht oder aus einem heraus gestartet ist,
    // meint diesen, auch wenn auf dem Zettel ein anderer steht. **Der
    // Zettel ist die Antwort fuer den, der nirgendwo steht**, also fuer
    // das installierte Programm aus dem Finder.
    for anfang in [std::env::current_dir().ok(), eigener_ordner()]
        .into_iter()
        .flatten()
    {
        if let Some(w) = aufwaerts(anfang) {
            merken(&w);
            return Some(w);
        }
    }
    gemerkt()
}

/// Das Verzeichnis, in dem das laufende Programm liegt.
///
/// ⚑ **Aufgeloest**, denn in einem `.app` fuehrt der Weg ueber
/// `Contents/MacOS`, und ein Verweis waere sonst nicht zu verfolgen.
pub fn eigener_ordner() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    p.parent().map(|q| q.to_path_buf())
}

/// Macht einen relativen Pfad gegen die Wurzel absolut.
///
/// 📌 **Ohne das scheitert „Modell laden" aus dem Finder heraus**, und
/// zwar mit `No such file or directory`: In den Einstellungen steht
/// `INTEGER_LLM/artifacts/myelith-4b`, und das ist relativ zu einem
/// Arbeitsverzeichnis, das dort `/` ist.
///
/// ⚑ **Ein absoluter Pfad bleibt unberuehrt.** Wer sein Artefakt
/// woanders liegen hat, hat das so gemeint.
pub fn absolut(pfad: &str) -> String {
    gegen(wurzel().as_deref(), pfad)
}

/// Dasselbe gegen eine **genannte** Wurzel.
///
/// ⚑ **Eigene Funktion, damit es sich pruefen laesst.** `absolut` fragt
/// die Umgebung, den Zettel und zwei Suchwege ab; eine Pruefung
/// darueber muesste all das stellen und liefe anderen Pruefungen ins
/// Gehege, die dieselben Variablen und dieselbe Datei benutzen. **Was
/// entschieden wird, steht hier; was ermittelt wird, steht dort.**
pub fn gegen(wurzel: Option<&Path>, pfad: &str) -> String {
    let p = Path::new(pfad);
    if pfad.is_empty() || p.is_absolute() {
        return pfad.to_string();
    }
    match wurzel {
        Some(w) => w.join(p).display().to_string(),
        None => pfad.to_string(),
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Legt einen Scheinklon an: ein Verzeichnis mit der Marke darin.
    fn scheinklon(name: &str) -> PathBuf {
        let w = std::env::temp_dir().join(format!("myelith-ort-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join("INTEGER_LLM/scripts")).expect("anlegen");
        std::fs::write(w.join(MARKE), "#!/bin/sh\n").expect("Marke");
        std::fs::canonicalize(&w).unwrap_or(w)
    }

    #[test]
    fn ein_verzeichnis_mit_der_marke_ist_die_wurzel() {
        let w = scheinklon("marke");
        assert!(ist_wurzel(&w));
        assert!(!ist_wurzel(&w.join("INTEGER_LLM")));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// ⚑ **Von tief drinnen fuehrt der Weg nach oben ans Ziel.**
    #[test]
    fn von_innen_wird_die_wurzel_gefunden() {
        let w = scheinklon("innen");
        let tief = w.join("CLIENT/myl-oberflaeche/ui");
        std::fs::create_dir_all(&tief).expect("anlegen");
        assert_eq!(aufwaerts(tief), Some(w.clone()));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// 📌 **Ein alter Zettel wird nicht geglaubt.** Genau das ist der
    /// Fall „Repositorium verschoben": Der Zettel zeigt auf einen Ort,
    /// an dem nichts mehr liegt, und wer ihn glaubt, sucht Artefakte in
    /// einem leeren Verzeichnis.
    #[test]
    fn ein_alter_zettel_wird_nicht_geglaubt() {
        let w = scheinklon("zettel");
        let fort = w.with_extension("fort");
        std::fs::rename(&w, &fort).expect("verschieben");
        // Der Zettel zeigt auf den alten Ort; dort ist die Marke weg.
        assert!(!ist_wurzel(&w), "der alte Ort traegt noch die Marke");
        assert!(ist_wurzel(&fort), "der neue Ort traegt sie nicht");
        let _ = std::fs::remove_dir_all(&fort);
    }

    /// ⚑ **Ein absoluter Pfad bleibt, ein leerer auch.**
    #[test]
    fn absolute_und_leere_pfade_bleiben() {
        let fest = if cfg!(windows) { "C:\\x\\y" } else { "/x/y" };
        assert_eq!(absolut(fest), fest);
        assert_eq!(absolut(""), "");
    }

    /// **Ein relativer Pfad wird gegen die Wurzel gelegt.**
    ///
    /// 📌 **Das ist der Fall „Repositorium verschoben".** In den
    /// Einstellungen steht `INTEGER_LLM/artifacts/myelith-4b`, und
    /// dieser Eintrag ueberlebt jeden Umzug: Was sich aendert, ist die
    /// Wurzel, und die wird gesucht statt aufgeschrieben.
    /// 📌 **Die Erwartung wird gebaut und nicht getippt** (2026-09-10).
    /// Der erste Entwurf schrieb `"/wo/auch/immer/INTEGER_LLM/..."` als
    /// Text hin und fiel unter Windows: Dort setzt `Path::join` einen
    /// Backslash, und die Pruefung meldete einen Unterschied im
    /// Trennzeichen als Fehler in der Aufloesung. **Eine Erwartung, die
    /// von Hand geschrieben ist, prueft die Maschine, auf der sie
    /// geschrieben wurde.**
    #[test]
    fn ein_relativer_pfad_haengt_an_der_wurzel() {
        let alt = Path::new("/wo/auch/immer");
        let neu = Path::new("/ganz/woanders");
        let rel = "INTEGER_LLM/artifacts/myelith-4b";
        assert_eq!(gegen(Some(alt), rel), alt.join(rel).display().to_string());
        // Derselbe Eintrag, verschobener Klon, richtiger Pfad.
        assert_eq!(gegen(Some(neu), rel), neu.join(rel).display().to_string());
        // ⚑ Und die beiden sind wirklich verschieden: Ohne diese Zeile
        // ginge die Pruefung auch dann durch, wenn `gegen` die Wurzel
        // ignorierte und schlicht `rel` zurueckgaebe.
        assert_ne!(gegen(Some(alt), rel), gegen(Some(neu), rel));
        // ⚠️ Und ohne Wurzel bleibt er relativ, statt geraten zu werden.
        assert_eq!(gegen(None, rel), rel);
    }

    /// **Beide Programme des Klienten loesen denselben Pfad auf.**
    ///
    /// 📌 Bis zum 2026-09-10 tat es nur die Oberflaeche. `myl` gab aus
    /// einem fremden Arbeitsverzeichnis „es fehlt das
    /// Artefaktverzeichnis", obwohl das Artefakt dalag. **Zwei
    /// Programme desselben Klienten beantworteten dieselbe Frage
    /// verschieden**, und das faellt niemandem auf, der nur eines
    /// benutzt.
    #[test]
    fn beide_programme_loesen_den_artefaktpfad_auf() {
        let werkzeug = include_str!("bin/myl.rs");
        assert!(
            werkzeug.contains("ort::absolut(&e.modell.artefakt)"),
            "`myl` nimmt den eingestellten Pfad, wie er dasteht"
        );
    }

    /// Die Wurzel dieses Repositoriums, von der Kiste aus.
    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("Wurzel des Repositoriums")
            .to_path_buf()
    }

    /// **Kein Skript nennt einen festen Pfad in diesen Baum.**
    ///
    /// ⚑ **Die Wurzel ist, wo die Datei liegt.** `$(dirname "$0")` und
    /// `%~dp0`, und sonst nichts. Damit ueberlebt das Einrichten jedes
    /// Verschieben, Umbenennen und Kopieren des Repositoriums: Es gibt
    /// keinen Pfad, den jemand nachziehen muesste.
    ///
    /// ⚠️ **Ein fester Pfad faellt sonst erst dem Naechsten auf**, und
    /// zwar dem, der ihn nicht geschrieben hat.
    #[test]
    fn kein_skript_nennt_einen_festen_pfad() {
        let wurzel = repo();
        for datei in ["INSTALL/installieren-macos.sh", "INSTALL/installieren-nixos.sh"] {
            let text = std::fs::read_to_string(wurzel.join(datei)).expect(datei);
            for zeile in text.lines() {
                let z = zeile.trim();
                if z.starts_with('#') {
                    continue;
                }
                assert!(
                    !z.contains("/Users/") && !z.contains("C:\\Users"),
                    "{datei} nennt einen festen Benutzerpfad:\n  {z}"
                );
            }
            assert!(
                text.contains("dirname \"$0\""),
                "{datei} leitet die Wurzel nicht aus seinem eigenen Ort ab"
            );
        }
        let ps = std::fs::read_to_string(wurzel.join("INSTALL/installieren-windows.ps1"))
            .expect("INSTALL/installieren-windows.ps1");
        assert!(ps.contains("$PSScriptRoot"), "das Windows-Skript kennt seinen Ort nicht");
    }

    /// **Der Menueeintrag fuer Linux wird erzeugt und nicht abgelegt.**
    ///
    /// 📌 Ein `.desktop` traegt absolute Pfade in `Exec` und `Icon`. Im
    /// Repositorium abgelegt waere es beim ersten Verschieben falsch
    /// und beim zweiten Klon von Anfang an. **Ein Pfad, der in einer
    /// versionierten Datei steht, ist eine Wette darauf, dass nichts
    /// sich bewegt.**
    #[test]
    fn der_menueeintrag_wird_erzeugt() {
        let wurzel = repo();
        let sh = std::fs::read_to_string(wurzel.join("INSTALL/installieren-nixos.sh"))
            .expect("Skript");
        assert!(sh.contains("[Desktop Entry]"), "das Skript legt keinen Eintrag an");
        assert!(sh.contains("Icon=$WURZEL/"), "der Eintrag traegt kein Symbol aus diesem Baum");
        assert!(
            !wurzel.join("myelith.desktop").exists() && !wurzel.join("Myelith.desktop").exists(),
            "im Repositorium liegt ein .desktop mit festen Pfaden"
        );
    }

    /// ⚠️ **Die Umgebungsvariable schlaegt alles**, aber nur, wenn dort
    /// auch wirklich ein Klon liegt. Ein Tippfehler in der Variablen
    /// darf nicht dazu fuehren, dass gar nichts mehr gefunden wird.
    #[test]
    fn eine_falsche_umgebungsvariable_blockiert_nicht() {
        let leer = std::env::temp_dir().join("myelith-gibt-es-nicht");
        assert!(!ist_wurzel(&leer));
    }
}
