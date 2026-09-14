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
    /// **Was die Maschine mindestens haben muss**, kleingedruckt neben
    /// dem Namen. Leer, wenn der Katalog es nicht weiss.
    ///
    /// ⚑ **Sie steht in der Wahl und nicht nur in der Modellkarte.**
    /// Wer ein Modell auswaehlt, entscheidet in diesem Moment, ob seine
    /// Maschine es tragen kann; die Auskunft dazu spaeter anzubieten
    /// hiesse, ihn erst laden und dann scheitern zu lassen.
    ///
    /// ⚠️ Beim Netzeintrag bleibt sie leer: Dort rechnet eine fremde
    /// Maschine, und eine Anforderung an die eigene waere falsch.
    pub hardware: String,
    /// Ob dieser Eintrag das Modell **im Netz** meint statt hier.
    pub ueber_netz: bool,
}

/// Was der Katalog ueber ein Artefakt weiss, soweit die Wahl es braucht.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Katalogeintrag {
    /// Was ein Mensch liest.
    pub name: String,
    /// Mindestausstattung fuer den oertlichen Betrieb.
    pub hardware: String,
    /// Parameter in Milliarden. **Die Ordnungszahl der Wahl.**
    ///
    /// ⚠️ **Eine Zahl und keine Zeichenkette.** Nach Namen sortiert
    /// stuende `Myelith 14B` vor `Myelith 4B`, weil `1` vor `4` kommt.
    /// Das faellt bei vier Modellen sofort auf und bei zwei nie.
    pub reihung: f64,
}

/// **Das Kennzeichen, unter dem ein Modell im Netz gerechnet wird.**
///
/// ⚑ **Ein Praefix und kein eigener Pfad.** Die Wahl setzt
/// `modell.artefakt`, und der Netzeintrag muss durch dasselbe Feld
/// passen. `netz:` davor sagt, dass nicht diese Maschine rechnet;
/// dahinter steht, **welches** Modell, denn im Netz gibt es mehr als
/// eines.
pub const NETZ: &str = "netz:";

/// **Was sich laden laesst: hier und, spaeter, im Netz.**
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
/// # ⚑ Aufsteigend nach Groesse, nicht nach Verzeichnisnamen
///
/// Die Ordnungszahl kommt aus dem Katalog (`reihung`, Parameter in
/// Milliarden). Was der Katalog nicht kennt, kommt danach und dort nach
/// Namen: **Ein Modell ohne Eintrag soll die Reihe nicht durcheinander
/// bringen, aber auch nicht verschwinden.**
///
/// # ⚠️ Jedes Modell steht zweimal da, und das zweite Mal traegt nicht
///
/// Seit dem 2026-09-11 (Festlegung des Projektinhabers) ersetzt **je
/// ein Eintrag pro Modell** den einen frueheren Sammeleintrag
/// „Netzwerkmodell". Der Grund ist die Bauart des Netzes: Wer ein
/// kleines Modell auf seiner Maschine haelt, soll es anderen anbieten
/// koennen, ohne es zu sharden. **Das kleinste Redundanzpaar sind dann
/// zwei einzelne Rechner**, und die Frage „welches Modell" hat dort
/// dieselbe Antwortmenge wie hier.
///
/// ⛔️ **Gesperrt, bis die Knoten stehen.** Die Eintraege sind sichtbar
/// und nicht waehlbar, und `warum` sagt es. Ein Eintrag, der still
/// nichts tut, waere schlimmer als keiner.
pub fn liste(e: &Einstellungen) -> Vec<Modellwahl> {
    let katalog = katalog();
    let mut oertlich: Vec<Modellwahl> = Vec::new();

    let aufgeloest = crate::ort::absolut(&e.modell.artefakt);
    let hier = std::path::Path::new(&aufgeloest);
    if let Some(eltern) = hier.parent() {
        if let Ok(lesen) = std::fs::read_dir(eltern) {
            let pfade: Vec<std::path::PathBuf> = lesen
                .flatten()
                .map(|x| x.path())
                .filter(|p| p.join("model_config.json").is_file())
                .collect();
            for p in pfade {
                let ordner =
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let k = katalog.get(&ordner);
                oertlich.push(Modellwahl {
                    pfad: p.display().to_string(),
                    // ⚑ Der Anzeigename kommt aus dem Katalog, wenn er
                    // dort steht: In der Wahl soll „Myelith 4B" stehen
                    // und nicht der Verzeichnisname `myelith-4b`. Steht
                    // er nicht drin, bleibt der Verzeichnisname, denn
                    // ein erfundener Name waere schlechter als ein
                    // technischer.
                    name: k.map(|x| x.name.clone()).unwrap_or_else(|| ordner.clone()),
                    offen: true,
                    warum: String::new(),
                    hardware: k.map(|x| x.hardware.clone()).unwrap_or_default(),
                    ueber_netz: false,
                });
            }
        }
    }
    ordnen(&mut oertlich, &katalog);

    // ⚑ **Die Netzeintraege entstehen aus dem, was gefunden wurde**, und
    // zwar bevor der Rueckfalleintrag dazukommt: Ein Pfad, der hier
    // nicht liegt, ist auch im Netz keine Zusage.
    let mut netz: Vec<Modellwahl> = oertlich
        .iter()
        .filter(|m| m.offen)
        .map(|m| Modellwahl {
            pfad: format!("{NETZ}{}", kennung(&m.pfad)),
            // ⚑ Wortlaut des Projektinhabers, mit Komma statt Strich
            // nach der Hausregel.
            name: format!("{} (API), kostet Inferenz-Credits", m.name),
            offen: false,
            warum: "Noch nicht verdrahtet: Dem Klienten fehlen Knotenadresse und Vollmacht. \
                    Bis dahin rechnet diese Maschine."
                .into(),
            // ⚠️ Leer, und das ist keine Luecke: Im Netz rechnet eine
            // fremde Maschine. Eine Anforderung an die eigene stuende
            // hier falsch.
            hardware: String::new(),
            ueber_netz: true,
        })
        .collect();

    // 📌 Steht das eingestellte Artefakt nicht in der Liste (weil es
    // woanders liegt oder noch nicht existiert), kommt es trotzdem
    // dazu: Sonst zeigte die Wahl etwas anderes als das, was gilt.
    //
    // 📌 **Verglichen wird aufgeloest** (2026-09-11). Vorher stand hier
    // `m.pfad == e.modell.artefakt`, und die Liste traegt absolute
    // Pfade, die Einstellung oft einen relativen: Das eingestellte
    // Modell stand dann **zweimal** in der Wahl, einmal unter seinem
    // Namen und einmal als „(eingestellt)".
    if !oertlich.iter().any(|m| crate::ort::absolut(&m.pfad) == aufgeloest) {
        oertlich.insert(
            0,
            Modellwahl {
                pfad: e.modell.artefakt.clone(),
                name: format!("{} (eingestellt)", e.modell.artefakt),
                offen: hier.join("model_config.json").is_file(),
                warum: "Der eingestellte Pfad enthaelt kein `model_config.json`.".into(),
                hardware: String::new(),
                ueber_netz: false,
            },
        );
    }

    oertlich.append(&mut netz);
    oertlich
}

/// Der Verzeichnisname eines Artefakts, also das, was der Katalog kennt.
fn kennung(pfad: &str) -> String {
    std::path::Path::new(pfad)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pfad.to_string())
}

/// **Aufsteigend nach Groesse; was der Katalog nicht kennt, ans Ende.**
fn ordnen(liste: &mut [Modellwahl], katalog: &BTreeMap<String, Katalogeintrag>) {
    liste.sort_by(|a, b| {
        let ka = katalog.get(&kennung(&a.pfad));
        let kb = katalog.get(&kennung(&b.pfad));
        match (ka, kb) {
            (Some(x), Some(y)) => x
                .reihung
                .partial_cmp(&y.reihung)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name)),
            // ⚑ Bekannt vor unbekannt, und unter den Unbekannten nach
            // Namen. Damit ist die Ordnung vollstaendig und haengt an
            // keiner Zufaelligkeit des Dateisystems.
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
}

/// Verzeichnisname zu Anzeigename, aus dem Katalog.
pub fn katalognamen() -> BTreeMap<String, String> {
    katalog().into_iter().map(|(k, v)| (k, v.name)).collect()
}

/// **Was der Katalog ueber jedes Artefakt weiss.**
///
/// ⚑ **Eine Lesung fuer drei Angaben.** Name, Hardware und Reihung
/// stehen in derselben Datei; sie dreimal zu lesen waere dreimal
/// dieselbe Datei und drei Gelegenheiten, dass eine der drei Fassungen
/// vergessen wird.
///
/// ⚠️ **Fehlt die Datei, ist die Karte leer und nicht falsch.** Ein
/// frischer Klon ohne Katalog zeigt Verzeichnisnamen, keine erfundenen
/// Angaben.
pub fn katalog() -> BTreeMap<String, Katalogeintrag> {
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
        // 📌 **Hier hing die Herkunft am Namen** (`Myelith 4B (aus
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
        let Some(n) = v.get("anzeigename").and_then(|x| x.as_str()) else { continue };
        aus.insert(
            k.clone(),
            Katalogeintrag {
                name: n.to_string(),
                hardware: v
                    .get("hardware")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
                // ⚠️ **Ohne Angabe ans Ende**, nicht an den Anfang: Ein
                // fehlender Wert ist keine Groesse von null.
                reihung: v.get("reihung").and_then(|x| x.as_f64()).unwrap_or(f64::MAX),
            },
        );
    }
    aus
}


/// **Was die Maschine fuer dieses Artefakt mindestens haben muss.**
///
/// ⚑ Leer, wenn der Katalog es nicht weiss. **Eine erfundene Angabe
/// waere schlechter als keine**, denn sie steht dort, wo jemand
/// entscheidet, ob er das Modell laden kann.
pub fn hardware_zu(pfad: &str) -> String {
    let ordner = std::path::Path::new(pfad)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pfad.to_string());
    katalog().get(&ordner).map(|k| k.hardware.clone()).unwrap_or_default()
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
