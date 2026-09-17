//! **Wissensmappen, die der Agent nachschlaegt statt sie im Kontext zu
//! tragen.**
//!
//! # ⚑ Zwei Orte, und der Unterschied ist die Zugehoerigkeit
//!
//! | Ort | Was dort liegt | Wer ihn fuellt |
//! |---|---|---|
//! | `<einhaengung>/.AGENT/skills/` | was zu **diesem Projekt** gehoert | die Arbeit an diesem Ordner |
//! | `<konfiguration>/skills/` | was **ueberall** gilt | der Nutzer |
//!
//! ⚑ **Der Projektordner entsteht mit `.AGENT`**, also in dem Augenblick,
//! in dem zum ersten Mal verdichtet wird. Das ist genau der richtige
//! Zeitpunkt: **Nach jeder Verdichtung ist das Wissen aus dem Kontext
//! verschwunden, und dann muss es einen Ort geben, an dem es noch
//! steht.**
//!
//! # ⛔️ Warum der allgemeine Ordner ein Werkzeug braucht und der
//! Projektordner keines
//!
//! Der Projektordner liegt **unter der Einhaengung**. `list_directory`,
//! `read_file` und `search_files` erreichen ihn schon heute; ein eigenes
//! Werkzeug waere ein vierter Weg, eine Datei zu lesen. ⚑ **Und die
//! Messung vom 2026-09-17 sagt, was das kostet:** Werkzeuge, die ein
//! Modell nicht ruft, stehen trotzdem in jeder Ansage und machen die
//! Auswahl schwerer; die drei Verlaufswerkzeuge sind deshalb am selben
//! Tag aus `Base` geflogen.
//!
//! ⛔️ **Der allgemeine Ordner liegt ausserhalb.** Ihn zu erreichen hiesse
//! entweder die Einhaengegrenze aufzuweichen oder ein Werkzeug zu bauen,
//! das **nur** ihn liest. Die Grenze ist die Zusage des ganzen
//! Dateiwerkzeugsatzes; also das Werkzeug.
//!
//! # ⚑ Genau die Form, die gemessen getragen hat
//!
//! `list_skills` nennt, `read_skill` liefert, beide in `Advanced`. Das
//! ist dieselbe Aufteilung wie beim Mitschnitt, und sie ist an ihm
//! gemessen: **nennen und liefern getrennt**, der Eintrag traegt einen
//! Satz und nicht nur einen Namen, und das Ganze haengt am
//! Nachschlagebudget, damit ein Modell den Kontext nicht ueber diesen
//! Weg fuellt.

use std::path::{Path, PathBuf};

/// Wie der Ordner heisst, hier wie dort.
pub const ORDNER: &str = "skills";

/// Wie viele Skills genannt werden, und wie lang ihre Zeile ist.
///
/// ⚑ **Eine feste Zahl, wie beim Verzeichnis des Mitschnitts.** Was in
/// jede Zusammenfassung geht, darf nicht mit dem Ordner wachsen.
pub const HOECHSTENS_GENANNT: usize = 20;
const ZEILENBREITE: usize = 160;

/// Eine Mappe, wie sie genannt wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Der Ordnername, und zugleich das Argument von `read_skill`.
    pub name: String,
    /// Die erste tragende Zeile der Eingangsseite.
    pub satz: String,
    /// Wie viele Zeichen die Eingangsseite hat.
    pub zeichen: usize,
    /// Wo sie liegt.
    pub pfad: PathBuf,
}

/// Die Eingangsseite einer Mappe, in der Reihenfolge, in der gesucht wird.
///
/// ⚑ **`MAPPE.md` zuerst**, weil `md_zu_mappe.py` sie so schreibt; dann
/// die ueblichen Namen, damit auch eine von Hand angelegte Mappe
/// gefunden wird.
const EINGAENGE: [&str; 4] = ["MAPPE.md", "SKILL.md", "README.md", "index.md"];

/// Der allgemeine Ordner, neben den Einstellungen.
///
/// ⚑ **Neben den Einstellungen und nicht im Arbeitsordner**: Was
/// ueberall gelten soll, darf nicht an einem Projekt haengen.
pub fn allgemeiner_ordner() -> PathBuf {
    let pfad = crate::Einstellungen::vorgabepfad();
    pfad.parent().map(|p| p.join(ORDNER)).unwrap_or_else(|| PathBuf::from(ORDNER))
}

/// Der Projektordner unter `.AGENT`.
pub fn projektordner(wurzel: &Path) -> PathBuf {
    wurzel.join(crate::verlauf::ORDNER).join(ORDNER)
}

/// **Legt den Projektordner an, mit einer Erklaerung darin.**
///
/// ⚑ **Eine leere Mappe erklaert sich nicht.** Wer den Ordner zum ersten
/// Mal sieht, soll ohne Nachfragen wissen, was hineingehoert und wie es
/// dorthin kommt.
pub fn anlegen(wurzel: &Path) -> std::io::Result<PathBuf> {
    let ordner = projektordner(wurzel);
    if ordner.is_dir() {
        return Ok(ordner);
    }
    std::fs::create_dir_all(&ordner)?;
    let hinweis = "# Skills dieses Projekts\n\
        \n\
        Jeder Unterordner hier ist eine Wissensmappe: eine Eingangsseite\n\
        `MAPPE.md` mit einem knappen Verzeichnis, daneben die Kapitel als\n\
        eigene Dateien.\n\
        \n\
        Der Agent liest sie **auf Nachfrage** und nicht auf Vorrat. Nach\n\
        einer Verdichtung des Gespraechs steht in der Zusammenfassung,\n\
        welche Mappen es hier gibt; die Kapitel holt er sich dann selbst.\n\
        \n\
        Angelegt wird eine Mappe ausserhalb des Clients, zum Beispiel aus\n\
        einem Buch. Was hier liegt, gehoert zu diesem Ordner; was ueberall\n\
        gelten soll, gehoert neben die Einstellungen.\n";
    std::fs::write(ordner.join("README.md"), hinweis)?;
    Ok(ordner)
}

/// Die Mappen eines Ordners, alphabetisch.
pub fn im_ordner(ordner: &Path) -> Vec<Skill> {
    let mut aus: Vec<Skill> = std::fs::read_dir(ordner)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            let (eingang, text) = EINGAENGE.iter().find_map(|d| {
                let k = p.join(d);
                std::fs::read_to_string(&k).ok().map(|t| (k, t))
            })?;
            Some(Skill {
                name,
                satz: erste_aussage(&text),
                zeichen: text.len(),
                pfad: eingang,
            })
        })
        .collect();
    aus.sort_by(|a, b| a.name.cmp(&b.name));
    aus
}

/// Beide Orte zusammen, Projekt zuerst.
///
/// ⚑ **Das Projekt zuerst, und bei gleichem Namen gewinnt es.** Wer eine
/// Mappe im Projekt ablegt, meint diese; die allgemeine ist die Vorgabe.
pub fn alle(wurzel: Option<&Path>) -> Vec<Skill> {
    alle_aus(wurzel, &allgemeiner_ordner())
}

/// **Dasselbe mit benanntem allgemeinem Ordner.**
///
/// ⚑ **Eine Naht, damit die Vorrangregel pruefbar ist.** Der allgemeine
/// Ordner haengt sonst am Ort der Einstellungen, und der haengt an der
/// Umgebung des ganzen Prozesses. **Eine Pruefung, die dafuer eine
/// Umgebungsvariable setzt, faellt ueber die Nachbarpruefung**, die
/// dieselbe Variable setzt; dieses Projekt hat das zweimal bezahlt
/// (Funde 378 und 385). Eine Naht kostet ein Argument und keine
/// Nebenwirkung.
pub fn alle_aus(wurzel: Option<&Path>, allgemein: &Path) -> Vec<Skill> {
    let mut aus = wurzel.map(|w| im_ordner(&projektordner(w))).unwrap_or_default();
    for s in im_ordner(allgemein) {
        if !aus.iter().any(|x| x.name == s.name) {
            aus.push(s);
        }
    }
    aus
}

/// Die Eingangsseite einer benannten Mappe.
pub fn lesen(wurzel: Option<&Path>, name: &str) -> Option<String> {
    lesen_aus(wurzel, &allgemeiner_ordner(), name)
}

/// Wie [`lesen`], mit benanntem allgemeinem Ordner.
pub fn lesen_aus(wurzel: Option<&Path>, allgemein: &Path, name: &str) -> Option<String> {
    alle_aus(wurzel, allgemein)
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .and_then(|s| std::fs::read_to_string(s.pfad).ok())
}

/// **Die Zeile, die in die Zusammenfassung geht.**
///
/// ⛔️ **Sie nennt und liefert nicht**, und sie ist gedeckelt: Nach einer
/// Verdichtung soll das Modell wissen, **dass** es Mappen gibt, nicht
/// deren Inhalt tragen.
pub fn verweis(wurzel: Option<&Path>) -> Option<String> {
    let s = alle(wurzel);
    if s.is_empty() {
        return None;
    }
    let genannt: Vec<String> = s
        .iter()
        .take(HOECHSTENS_GENANNT)
        .map(|x| format!("  {} - {}", x.name, x.satz))
        .collect();
    let mehr = s.len().saturating_sub(HOECHSTENS_GENANNT);
    let schluss = if mehr > 0 {
        format!("\n  ... und {mehr} weitere; `list_skills` nennt sie alle.")
    } else {
        String::new()
    };
    Some(format!(
        "\n\nWissensmappen zu diesem Ordner, die nicht im Kontext stehen. \
         `read_skill` holt eine Eingangsseite, `list_skills` nennt alle:\n{}{}",
        genannt.join("\n"),
        schluss
    ))
}

/// Die erste Zeile, die etwas aussagt.
fn erste_aussage(text: &str) -> String {
    let roh = text
        .lines()
        .map(str::trim)
        .find(|z| {
            !z.is_empty()
                && !z.starts_with('#')
                && !z.starts_with("---")
                && z.chars().any(char::is_alphabetic)
        })
        .unwrap_or("");
    if roh.chars().count() <= ZEILENBREITE {
        return roh.to_string();
    }
    roh.chars().take(ZEILENBREITE - 1).chain(['…']).collect()
}

#[cfg(test)]
mod proben {
    use super::*;

    fn mappe(ordner: &Path, name: &str, satz: &str) {
        let p = ordner.join(name);
        std::fs::create_dir_all(p.join("kapitel")).expect("Ordner");
        std::fs::write(
            p.join("MAPPE.md"),
            format!("# {name}\n\n{satz}\n\n## Kapitel\n\n- `kapitel/01.md`\n"),
        )
        .expect("schreiben");
    }

    /// ⚑ **Der Ordner entsteht mit `.AGENT` und erklaert sich selbst.**
    #[test]
    fn der_projektordner_entsteht_und_erklaert_sich() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        assert!(o.ends_with("skills"), "{o:?}");
        assert!(o.starts_with(d.path().join(".AGENT")), "er liegt nicht unter .AGENT: {o:?}");
        let hinweis = std::fs::read_to_string(o.join("README.md")).expect("README");
        assert!(hinweis.contains("Wissensmappe"), "{hinweis}");
        // Ein zweiter Aufruf fasst nichts an.
        std::fs::write(o.join("README.md"), "eigener Text").expect("schreiben");
        anlegen(d.path()).expect("anlegen");
        assert_eq!(
            std::fs::read_to_string(o.join("README.md")).expect("README"),
            "eigener Text",
            "der zweite Aufruf hat den Hinweis ueberschrieben"
        );
    }

    /// ⚑ **Genannt wird mit Satz, und die Zahl ist gedeckelt.**
    #[test]
    fn der_verweis_nennt_und_liefert_nicht() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        mappe(&o, "kryptografie", "Wie Signaturen und Zufallslosungen hier benutzt werden.");
        mappe(&o, "buchhaltung", "Wie das Ledger Betraege fuehrt.");

        let v = verweis(Some(d.path())).expect("Verweis");
        assert!(v.contains("kryptografie"), "{v}");
        assert!(v.contains("Wie Signaturen"), "der Satz fehlt: {v}");
        // ⛔️ Der Inhalt der Kapitel bleibt draussen.
        assert!(!v.contains("kapitel/01.md"), "der Verweis liefert Inhalt: {v}");

        // Und die Obergrenze haelt.
        for i in 0..30 {
            mappe(&o, &format!("mappe{i:02}"), "Eine weitere Mappe.");
        }
        let v2 = verweis(Some(d.path())).expect("Verweis");
        let zeilen = v2.lines().filter(|z| z.starts_with("  ") && z.contains(" - ")).count();
        assert!(zeilen <= HOECHSTENS_GENANNT, "{zeilen} Zeilen statt hoechstens {HOECHSTENS_GENANNT}");
        assert!(v2.contains("weitere"), "die Kuerzung sagt sich nicht an: {v2}");
    }

    /// ⛔️ **Bei gleichem Namen gewinnt das Projekt.**
    ///
    /// 📌 **Die erste Fassung dieser Probe legte nur die Projektmappe
    /// an**, also genau den Fall, den die Regel gar nicht betrifft: Sie
    /// blieb gruen, als die Regel versuchsweise umgedreht wurde. **Eine
    /// Gegenprobe, die nicht beisst, ist ein Fund**, hier an der
    /// Pruefung.
    #[test]
    fn das_projekt_gewinnt_vor_dem_allgemeinen() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let allgemein = tempfile::tempdir().expect("Verzeichnis");
        let o = anlegen(d.path()).expect("anlegen");
        mappe(&o, "gleichnamig", "Die Fassung dieses Projekts.");
        mappe(allgemein.path(), "gleichnamig", "Die allgemeine Fassung.");
        mappe(allgemein.path(), "nur-allgemein", "Steht nur neben den Einstellungen.");

        let gelesen =
            lesen_aus(Some(d.path()), allgemein.path(), "gleichnamig").expect("lesen");
        assert!(
            gelesen.contains("Die Fassung dieses Projekts."),
            "die allgemeine Mappe hat gewonnen: {gelesen}"
        );

        // ⚑ **Und die allgemeine ist trotzdem da**, wenn es keine
        // gleichnamige im Projekt gibt.
        let namen: Vec<String> =
            alle_aus(Some(d.path()), allgemein.path()).into_iter().map(|s| s.name).collect();
        assert!(namen.contains(&"nur-allgemein".to_string()), "{namen:?}");
        assert_eq!(
            namen.iter().filter(|n| *n == "gleichnamig").count(),
            1,
            "die gleichnamige Mappe steht zweimal in der Liste: {namen:?}"
        );
        assert!(lesen_aus(Some(d.path()), allgemein.path(), "gibtsnicht").is_none());
    }
}
