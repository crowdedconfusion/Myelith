//! Testpläne finden, auswählen und anwenden.
//!
//! ## Warum vor der Artefaktfrage
//!
//! Ein Testplan legt das Modell fest. Wer zuerst nach dem Artefakt fragt
//! und danach den Plan lädt, hat entweder die falsche Frage gestellt oder
//! muss sie zurücknehmen. Deshalb steht die Planauswahl ganz vorn: Ist
//! einer gewählt, ergibt sich das Artefakt aus ihm, und es gibt nichts
//! mehr zu fragen.
//!
//! ## Warum ein Plan überhaupt
//!
//! Der Client vergleicht Maschinen. Verglichen werden darf nur, was
//! dieselbe Eingabe hatte: derselbe Prompt, zeichengenau, dieselbe
//! Tokenzahl, dasselbe Modell. Ein abweichendes Leerzeichen im Prompt
//! erzeugt einen anderen Digest, und das sieht aus wie ein Befund an der
//! Kernthese, ist aber ein Tippfehler. Der Plan schließt das aus, und
//! seine Prüfsumme schließt aus, dass er unterwegs verändert wurde.

use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::TestPlan;

/// Ablageort der Pläne, relativ zur Repository-Wurzel.
pub const ORDNER: &str = "TESTCLIENT/Testpläne";

/// Ein gefundener Plan samt Herkunft.
pub struct Gefunden {
    pub pfad: PathBuf,
    pub plan: TestPlan,
}

/// Sucht Pläne im Planordner.
///
/// Dateien, deren Prüfsumme nicht passt, werden **übersprungen und
/// gemeldet**, nicht stillschweigend geladen. Ein veränderter Plan ist
/// genau der Fall, den die Prüfsumme abfangen soll.
pub fn suchen(repo: &Path, meldung: &mut dyn FnMut(String)) -> Vec<Gefunden> {
    let ordner = repo.join(ORDNER);
    let Ok(eintraege) = fs::read_dir(&ordner) else {
        return Vec::new();
    };
    let mut pfade: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "plan"))
        .collect();
    pfade.sort();

    let mut out = Vec::new();
    for pfad in pfade {
        match TestPlan::load(&pfad) {
            Ok(plan) => out.push(Gefunden { pfad, plan }),
            Err(fehler) => meldung(format!(
                "  {} übersprungen: {:?}",
                pfad.file_name().unwrap_or_default().to_string_lossy(),
                fehler
            )),
        }
    }
    out
}

/// Kurzfassung eines Plans für die Auswahlliste.
pub fn zeile(g: &Gefunden) -> String {
    let p = &g.plan;
    let prompt = if p.prompt.chars().count() > 44 {
        let gekuerzt: String = p.prompt.chars().take(41).collect();
        format!("{}…", gekuerzt)
    } else {
        p.prompt.clone()
    };
    format!(
        "{} — {}, {} Token, {} Shards, Prüfsumme {}\n      Prompt: \"{}\"",
        p.plan_id,
        p.model,
        p.steps,
        p.shards,
        p.short_id(),
        prompt
    )
}
