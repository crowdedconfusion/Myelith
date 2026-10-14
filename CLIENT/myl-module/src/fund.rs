//! **Module finden und pruefen**, fuer jeden Wirt gleich.
//!
//! ⚑ **Eine Stelle fuer Konsole und Fenster** (2026-10-12). Bis dahin lag
//! das Finden und Pruefen allein in der Konsole; seit das Fenster Module
//! waehlen kann, braeuchte es dieselbe Pruefung ein zweites Mal, und zwei
//! Pruefungen derselben Sache laufen auseinander. Wo die Module liegen und
//! wo die Vertrauensliste steht, sagt der Aufrufer: Diese Kiste kennt weder
//! Einstellungen noch die Wurzel des Repositoriums.

use std::path::{Path, PathBuf};

use crate::signatur::{self, Geprueft, Versionsstand, Vertrauensliste};

/// **Ein gefundenes Modul**: geprueft oder mit dem Grund, warum nicht.
pub struct Fund {
    pub ordner: PathBuf,
    pub ergebnis: Result<Geprueft, String>,
}

/// **Die Ordner, die wie ein Modul aussehen** (mit Beschreibung), je Ort
/// nach dem Namen sortiert und in der Reihenfolge der Orte.
pub fn kandidaten(wurzeln: &[PathBuf]) -> Vec<PathBuf> {
    let mut aus = Vec::new();
    for w in wurzeln {
        let Ok(rd) = std::fs::read_dir(w) else { continue };
        let mut o: Vec<PathBuf> =
            rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join(crate::BESCHREIBUNG).is_file()).collect();
        o.sort();
        aus.extend(o);
    }
    aus
}

/// **Findet und prueft alle Module** an den Orten `wurzeln`.
///
/// ⚑ **Ein Name gilt nur einmal**: Der erste Ort gewinnt, ein zweites Modul
/// gleichen Namens wird abgelehnt. `verboten` nennt der Wirt fuer einen
/// Hauptbefehl, den er selbst belegt (die Konsole ihre eigenen Befehle);
/// steht dort ein Grund, wird das Modul mit diesem Grund abgelehnt.
pub fn finden(
    wurzeln: &[PathBuf],
    vertrauenspfad: &Path,
    versionspfad: &Path,
    verboten: &dyn Fn(&Geprueft) -> Option<String>,
) -> Vec<Fund> {
    let vertrauen = Vertrauensliste::lesen(vertrauenspfad);
    let versionen = Versionsstand::lesen(versionspfad);
    let mut namen: Vec<String> = Vec::new();
    kandidaten(wurzeln)
        .into_iter()
        .map(|ordner| {
            let ergebnis = match &vertrauen {
                Ok(v) => signatur::pruefen(&ordner, v, &versionen),
                Err(f) => Err(format!("Vertrauensliste: {f}")),
            };
            let ergebnis = ergebnis.and_then(|g| {
                if namen.contains(&g.beschreibung.name) {
                    return Err(format!("ein Modul namens {} gibt es schon", g.beschreibung.name));
                }
                if let Some(grund) = verboten(&g) {
                    return Err(grund);
                }
                namen.push(g.beschreibung.name.clone());
                Ok(g)
            });
            Fund { ordner, ergebnis }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ Ein Ordner ohne Beschreibung ist kein Kandidat, einer mit ist es,
    /// und ohne Vertrauensliste wird keiner geprueft.
    #[test]
    fn kandidaten_und_ohne_vertrauen() {
        let d = std::env::temp_dir().join(format!("myl-fund-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::create_dir_all(d.join("b")).unwrap();
        std::fs::write(d.join("b").join(crate::BESCHREIBUNG), "{}").unwrap();
        assert_eq!(kandidaten(std::slice::from_ref(&d)), vec![d.join("b")]);
        let f = finden(std::slice::from_ref(&d), &d.join("fehlt.json"), &d.join("v.json"), &|_| None);
        assert_eq!(f.len(), 1);
        assert!(f[0].ergebnis.is_err(), "ohne Vertrauensliste darf nichts als geprueft gelten");
        let _ = std::fs::remove_dir_all(&d);
    }
}
