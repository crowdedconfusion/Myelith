//! Der Auftrag aus `book2skill`: Seiten und Bilder, je Seite eine Datei.

use crate::pdf::{self, Renderfehler};
use crate::Erkenner;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// So viele Seiten entscheiden ueber die Schrift eines Werks.
const WAHLSEITEN: usize = 4;

fn text(v: &Value, k: &str) -> Result<String, String> {
    v[k].as_str().map(str::to_string).ok_or_else(|| format!("im Auftrag fehlt {k}"))
}

/// Atomar, damit ein Abbruch keine halbe Seite hinterlaesst.
fn ablegen(ziel: &Path, inhalt: &[u8]) -> Result<(), String> {
    let teil = ziel.with_extension("teil");
    std::fs::write(&teil, inhalt).map_err(|f| format!("{}: {f}", teil.display()))?;
    std::fs::rename(&teil, ziel).map_err(|f| format!("{}: {f}", ziel.display()))
}

/// Wie ein Auftrag endet, der keinen Fehler hatte.
#[derive(Debug, PartialEq)]
pub enum Ende {
    /// Alle Seiten liegen im Arbeitsordner.
    Fertig,
    /// ⛔️ **Diese Seite hing**, sie liegt als unlesbar im Arbeitsordner. Der
    /// Faden, der sie rendert, laeuft weiter; der Aufrufer beendet den
    /// Prozess, und ein neuer Auftrag setzt nach ihr fort.
    Haengt(u32),
}

/// Eine Seite, die nicht gerendert werden konnte: ohne Text, mit Vermerk.
fn unlesbar(arbeit: &Path, n: u32) -> Result<Ende, String> {
    let inhalt = json!({ "md": "", "bilder": [], "titel": "", "unlesbar": true });
    ablegen(&arbeit.join(format!("seite-{n:06}.json")), inhalt.to_string().as_bytes())?;
    Ok(Ende::Haengt(n))
}

pub fn ausfuehren(auftrag: &Path) -> Result<Ende, String> {
    let roh = std::fs::read(auftrag).map_err(|f| format!("{}: {f}", auftrag.display()))?;
    let a: Value = serde_json::from_slice(&roh).map_err(|f| format!("Auftrag: {f}"))?;
    let arbeit = PathBuf::from(text(&a, "arbeit")?);
    crate::text::kerne_setzen(a["kerne"].as_u64().unwrap_or(0) as usize);
    let fest = a["schrift"].as_str().filter(|s| *s != "auto");
    let mut erkenner = Erkenner::neu(fest)?;
    if let Some(liste) = a["bildtexte"].as_array() {
        if fest.is_none() && !liste.is_empty() {
            let mut proben = Vec::new();
            for paar in liste.iter().take(WAHLSEITEN) {
                let ein = paar[0].as_str().unwrap_or_default();
                proben.push(image::open(ein).map_err(|f| format!("{ein}: {f}"))?.to_rgb8());
            }
            erkenner.schrift_waehlen(&proben)?;
        }
        for paar in liste {
            let (ein, aus) = (paar[0].as_str().unwrap_or_default(), paar[1].as_str().unwrap_or_default());
            let bild = image::open(ein).map_err(|f| format!("{ein}: {f}"))?.to_rgb8();
            let t = erkenner.bildtext(&bild)?;
            ablegen(Path::new(aus), &serde_json::to_vec(&t).map_err(|f| f.to_string())?)?;
        }
        ablegen(&arbeit.join("schrift.txt"), erkenner.schrift().as_bytes())?;
    }
    let Some(pakete) = a["pakete"].as_array().filter(|p| !p.is_empty()) else {
        return Ok(Ende::Fertig);
    };
    let frist = a["frist_je_seite_ms"].as_u64().filter(|ms| *ms > 0).map_or(pdf::FRIST_JE_SEITE, Duration::from_millis);
    let daten = std::fs::read(text(&a, "pdf")?).map_err(|f| format!("PDF: {f}"))?;
    let gesamt: usize = pakete.iter().map(|p| p["seiten"].as_array().map_or(0, Vec::len)).sum();
    let mut alle: Vec<(u32, bool)> = Vec::with_capacity(gesamt);
    for p in pakete {
        let ocr = p["ocr"].as_bool().unwrap_or(true);
        for n in p["seiten"].as_array().ok_or("im Auftrag fehlen die Seiten")? {
            alle.push((n.as_u64().ok_or("keine Seitenzahl")? as u32, ocr));
        }
    }
    alle.sort_unstable();
    let nummern: Vec<u32> = alle.iter().map(|x| x.0).collect();
    if fest.is_none() {
        let schritt = (nummern.len() / WAHLSEITEN).max(1);
        let auswahl: Vec<u32> = nummern.iter().copied().step_by(schritt).take(WAHLSEITEN).collect();
        let mut proben = Vec::new();
        match pdf::seiten_rendern(daten.clone(), &auswahl, frist, |_, b| {
            proben.push(b);
            Ok(())
        }) {
            Ok(()) => {}
            Err(Renderfehler::Frist(n)) => return unlesbar(&arbeit, n),
            Err(Renderfehler::Fehler(m)) => return Err(m),
        }
        erkenner.schrift_waehlen(&proben)?;
    }
    ablegen(&arbeit.join("schrift.txt"), erkenner.schrift().as_bytes())?;
    let mut fertig = 0usize;
    let aus = pdf::seiten_rendern(daten, &nummern, frist, |n, bild| {
        let ocr = alle.iter().find(|x| x.0 == n).is_some_and(|x| x.1);
        let seite = erkenner.seite(&bild, !ocr)?;
        let mut namen = Vec::new();
        for (i, png) in seite.abbildungen.iter().enumerate() {
            let name = format!("bild-{n:06}-{i:02}.png");
            ablegen(&arbeit.join(&name), png)?;
            namen.push(name);
        }
        let inhalt = json!({ "md": seite.markdown, "bilder": namen, "titel": seite.titel });
        ablegen(&arbeit.join(format!("seite-{n:06}.json")), inhalt.to_string().as_bytes())?;
        fertig += 1;
        if fertig.is_multiple_of(8) || fertig == gesamt {
            eprintln!("[myl scan] {fertig} von {gesamt} Seiten");
        }
        Ok(())
    });
    match aus {
        Ok(()) => Ok(Ende::Fertig),
        Err(Renderfehler::Frist(n)) => unlesbar(&arbeit, n),
        Err(Renderfehler::Fehler(m)) => Err(m),
    }
}
