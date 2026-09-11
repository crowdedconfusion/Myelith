//! Welche Modelle zur Wahl stehen.
//!
//! # ⚑ Eine Liste fuer beide Bedieninstrumente
//!
//! Fenster und Konsole stellen dieselbe Frage: **Was kann ich hier
//! laden?** Bis zum 2026-09-11 beantwortete sie nur das Fenster, und
//! die Konsole las stattdessen das Verzeichnis der Artefakte ab.
//! **Zwei Listen, die dasselbe aufzaehlen, zaehlen irgendwann
//! verschieden auf**: Der Anzeigename aus dem Katalog fehlte drueben,
//! der Netzeintrag ebenso.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::einstellungen::Einstellungen;

/// Ein Eintrag der Modellwahl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Modellwahl {
    /// Wohin `modell.artefakt` zeigen wuerde.
    pub pfad: String,
    /// Was ein Mensch liest.
    pub name: String,
    /// ⚑ `false` heisst: sichtbar, aber nicht benutzbar, und `warum`
    /// sagt weshalb. **Ein Eintrag, der still nichts tut, waere
    /// schlimmer als keiner.**
    pub offen: bool,
    /// Warum nicht, falls nicht.
    pub warum: String,
}

/// **Was sich hier laden laesst, in der Reihenfolge, in der es liegt.**
///
/// # ⚑ Gesucht wird im Elternverzeichnis des eingestellten Artefakts
///
/// `modell.artefakt` zeigt auf ein Artefakt; daneben liegen die
/// anderen. Ein fest verdrahteter Pfad waere eine Annahme darueber, wo
/// jemand seine Artefakte haelt, und die trifft bei einem frischen Klon
/// nicht.
///
/// ⚑ **Erkannt wird an `model_config.json`.** Ein Verzeichnis ohne die
/// Datei ist kein Artefakt, und eines mit ihr laesst sich laden.
///
/// # ⚠️ Der Netzeintrag steht da und traegt nicht
///
/// „Netzwerkmodell (API), kostet Inferenz-Credits" ist der Platz fuer
/// die Netzhaelfte. Er ist gesperrt, solange Knotenadresse und
/// Vollmacht fehlen. Er steht trotzdem in der Liste, weil die Wahl
/// zwischen hier und dort an genau diese Stelle gehoert und nicht in
/// einen Schalter im Kopf: **Ein Modell ist ein Modell, ob es auf dieser
/// Maschine liegt oder im Netz gerechnet wird.**
pub fn liste(e: &Einstellungen) -> Vec<Modellwahl> {
    let mut aus: Vec<Modellwahl> = Vec::new();

    let aufgeloest = crate::ort::absolut(&e.modell.artefakt);
    let hier = std::path::Path::new(&aufgeloest);
    if let Some(eltern) = hier.parent() {
        if let Ok(lesen) = std::fs::read_dir(eltern) {
            let mut pfade: Vec<std::path::PathBuf> =
                lesen.flatten().map(|x| x.path()).filter(|p| p.join("model_config.json").is_file()).collect();
            pfade.sort();
            // ⚑ Der Anzeigename kommt aus dem Katalog, wenn er dort
            // steht: In der Wahl soll „Myelith 4B" stehen und nicht
            // der Verzeichnisname `myelith-4b`. Steht er nicht drin,
            // bleibt der Verzeichnisname, denn ein erfundener Name
            // waere schlechter als ein technischer.
            let namen = katalognamen();
            for p in pfade {
                let ordner =
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let name = namen.get(&ordner).cloned().unwrap_or_else(|| ordner.clone());
                aus.push(Modellwahl {
                    pfad: p.display().to_string(),
                    name,
                    offen: true,
                    warum: String::new(),
                });
            }
        }
    }
    // ⛑ Steht das eingestellte Artefakt nicht in der Liste (weil es
    // woanders liegt oder noch nicht existiert), kommt es trotzdem
    // dazu: Sonst zeigte die Wahl etwas anderes als das, was gilt.
    if !aus.iter().any(|m| m.pfad == e.modell.artefakt) {
        aus.insert(
            0,
            Modellwahl {
                pfad: e.modell.artefakt.clone(),
                name: format!("{} (eingestellt)", e.modell.artefakt),
                offen: hier.join("model_config.json").is_file(),
                warum: "Der eingestellte Pfad enthaelt kein `model_config.json`.".into(),
            },
        );
    }
    aus.push(Modellwahl {
        pfad: "netz".into(),
        // ⚑ Wortlaut des Projektinhabers (2026-09-10), mit Komma statt
        //   Strich nach der Hausregel.
        //
        // ⛑ **Hier stand „API, kostet Inferenz-Credits".** Der Eintrag
        // steht in einer Liste neben „Myelith 4B" und „Myelith 7B", und
        // „API" allein sagt dort nicht, was es ist: Es liest sich wie
        // eine Schnittstelle und nicht wie ein Modell. **Ein Eintrag in
        // einer Modellwahl muss zuerst sagen, dass er ein Modell ist.**
        name: "Netzwerkmodell (API), kostet Inferenz-Credits".into(),
        offen: false,
        warum: "Noch nicht verdrahtet: Dem Klienten fehlen Knotenadresse und Vollmacht. \
                Bis dahin rechnet diese Maschine."
            .into(),
    });
    aus
}

/// Verzeichnisname zu Anzeigename, aus dem Katalog.
pub fn katalognamen() -> BTreeMap<String, String> {
    let mut aus = BTreeMap::new();
    let Some(w) = crate::ort::wurzel() else { return aus };
    let Ok(roh) = std::fs::read_to_string(w.join("INTEGER_LLM/models/KATALOG.json")) else {
        return aus;
    };
    let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else { return aus };
    let Some(o) = d.as_object() else { return aus };
    for (k, v) in o {
        if k.starts_with('_') {
            continue;
        }
        // ⛑ **Hier hing die Herkunft am Namen** (`Myelith 4B (aus
        // Qwen3-4B)`), und sie stand damit in jeder Modellwahl, in
        // jeder Ladezeile und in jeder Meldung. Gemeldet vom
        // Projektinhaber am 2026-09-10.
        //
        // ⚑ **Ein Name ist ein Name.** Die Herkunft ist eine Angabe zum
        // Modell und gehoert dorthin, wo Angaben zum Modell stehen: in
        // die Modellkarte. Wer sie wissen will, liest sie dort; wer nur
        // wissen will, welches Modell antwortet, soll nicht jedes Mal
        // eine Klammer mitlesen.
        //
        // ⚠️ **Verschwinden darf sie deshalb nicht.** Die Basismodelle
        // stehen unter Apache 2.0, und ein Name ohne Herkunft waere
        // eine Verschleierung. Sie steht weiterhin in `KATALOG.json`
        // bei jedem Eintrag und in `artifacts/MODEL_CARD.md`.
        if let Some(n) = v.get("anzeigename").and_then(|x| x.as_str()) {
            aus.insert(k.clone(), n.to_string());
        }
    }
    aus
}


/// **Wie ein Artefakt heisst, wenn ein Mensch es liest.**
///
/// ⚑ Der Katalog kennt die eigenen; fuer alles andere bleibt der
/// Verzeichnisname. **Ein Name, den niemand vergeben hat, waere
/// schlechter als ein technischer.**
pub fn anzeigename(pfad: &str) -> String {
    let ordner = std::path::Path::new(pfad)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pfad.to_string());
    katalognamen().get(&ordner).cloned().unwrap_or(ordner)
}
