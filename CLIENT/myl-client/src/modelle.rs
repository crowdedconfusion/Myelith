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
    /// **Privat, nur lokal** (`betrieb = "lokal"`): kein Netzeintrag, und
    /// der Name sagt es.
    pub privat: bool,
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
    /// **Privat, nur lokal**: `betrieb = "lokal"` im Katalog des Projekts,
    /// oder ein Eintrag des lokalen Katalogs ([`KATALOG_LOKAL`]). Nicht fuer
    /// den Netzbetrieb zugelassen.
    pub privat: bool,
}

/// **Der Katalog der selbst gebauten, privaten Modelle** (seit 2026-10-19).
///
/// ⚑ Ihn schreibt `INTEGER_LLM/scripts/modell_bauen.sh`; er ist von Git
/// ausgenommen, denn was ein Nutzer aus einem fremden Repositorium baut,
/// gehoert nicht in die Liste dieses Projekts. **Jeder Eintrag darin ist
/// privat**, gleich was er selbst sagt.
pub const KATALOG_LOKAL: &str = "MODELS/llm/KATALOG.lokal.json";

/// **Ist das Artefakt nur fuer den lokalen Betrieb zugelassen?**
///
/// ⛔️ **Seit CLIENT v0.175.0 entscheidet die Netzliste** (Fund 585): Privat
/// ist jedes Artefakt, das nicht mit seinem Abdruck auf ihr steht, gleich
/// von welchem Anbieter seine Gewichte stammen; dieselbe Pruefung macht der
/// Netzbetrieb beim Laden (`load_model_fuer_netz` in der Laufzeit). `betrieb
/// = "lokal"` in `model_config.json` bleibt zusaetzlich privat. Ein Ordner
/// ohne `model_config.json` ist kein Artefakt und damit auch nicht privat.
pub fn nur_lokal(artefakt: &std::path::Path) -> bool {
    let Some(konfig) = std::fs::read_to_string(artefakt.join("model_config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return false;
    };
    konfig.get("betrieb").and_then(|b| b.as_str()) == Some("lokal")
        || !integer_llm_runtime::netzfreigabe::im_netz_zugelassen(artefakt)
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
/// **Wo nach Artefakten gesucht wird**: neben dem eingestellten, und unter
/// jedem Speicherort mit dem Zweck `gewichte` (`speicherorte.toml`).
///
/// ⚑ **Damit lassen sich Gewichte auslagern** (Wunsch des Projektinhabers,
/// 2026-10-17): Wer ein Artefakt nach `<ort>/gewichte/` verschiebt, findet
/// es in der Wahl wieder, ohne einen Pfad einzustellen. Eine kaputte Datei
/// kostet nur diese Orte, nicht die Wahl.
pub fn suchorte(eingestellt: &std::path::Path) -> Vec<std::path::PathBuf> {
    let s = crate::mining::Speicherorte::lesen(&crate::mining::speicherorte_pfad()).unwrap_or_default();
    let orte = suchorte_aus(eingestellt, &s, myl_local_agent::ort::wurzel().as_deref());
    umziehen_einmal(&orte);
    orte
}

// ── Die Namen vor dem 2026-10-21 ────────────────────────────────────────

/// **Alte Namen zu neuen**, aus `frueher` im Katalog des Projekts.
///
/// ⚑ **Seit dem 2026-10-21 heissen die Projektmodelle nach Tieren**
/// (Festlegung des Projektinhabers): `myelith-35b-a3b` ist jetzt
/// `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also
/// bleiben Abdruck und Register; nur Verzeichnis und Einstellungen muessen
/// mit. Die Zuordnung steht einmal, im Katalog.
pub fn fruehere_namen() -> Vec<(String, String)> {
    let Some(w) = myl_local_agent::ort::wurzel() else { return Vec::new() };
    fruehere_namen_aus(&std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json")).unwrap_or_default())
}

/// [`fruehere_namen`] aus dem Text des Katalogs, laengste alte Namen zuerst.
pub fn fruehere_namen_aus(haupt: &str) -> Vec<(String, String)> {
    let d = serde_json::from_str::<serde_json::Value>(haupt).unwrap_or_default();
    let mut paare: Vec<(String, String)> = d
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((v.get("frueher")?.as_str()?.to_string(), k.clone())))
        .collect();
    paare.sort_by_key(|(alt, _)| std::cmp::Reverse(alt.len()));
    paare
}

/// **Der neue Name zu einem alten**, oder `None`. Auch fuer Versuche mit
/// eigenem Anhaengsel, die der Katalog nicht fuehrt: `myelith-35b-a3b-ternaer`
/// wird `raven-35b-a3b-ternaer`.
pub fn neuer_name(alt: &str, paare: &[(String, String)]) -> Option<String> {
    if let Some((_, neu)) = paare.iter().find(|(f, _)| f == alt) {
        return Some(neu.clone());
    }
    paare.iter().find_map(|(f, neu)| {
        let rest = alt.strip_prefix(f.as_str())?.strip_prefix('-')?;
        Some(format!("{}-{rest}", neu.strip_suffix("-int8")?))
    })
}

/// **Benennt Artefaktverzeichnisse mit altem Namen um**, in jedem Ort.
///
/// ⛔️ Nie ueber ein bestehendes Verzeichnis: Liegt der neue Name schon da,
/// bleibt der alte, wie er ist. Gibt die Umbenennungen zurueck.
pub fn artefakte_umziehen(orte: &[std::path::PathBuf], paare: &[(String, String)]) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    let mut aus = Vec::new();
    for o in orte {
        let Ok(eintraege) = std::fs::read_dir(o) else { continue };
        for e in eintraege.flatten() {
            let alt = e.file_name().to_string_lossy().into_owned();
            let Some(neu) = neuer_name(&alt, paare) else { continue };
            let (a, n) = (o.join(&alt), o.join(&neu));
            if a.is_dir() && !n.exists() && std::fs::rename(&a, &n).is_ok() {
                aus.push((a, n));
            }
        }
    }
    aus
}

/// [`artefakte_umziehen`] je Ort einmal in diesem Prozess.
fn umziehen_einmal(orte: &[std::path::PathBuf]) {
    static GESEHEN: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());
    let Ok(mut gesehen) = GESEHEN.lock() else { return };
    let neu: Vec<std::path::PathBuf> = orte.iter().filter(|o| !gesehen.contains(o)).cloned().collect();
    if neu.is_empty() {
        return;
    }
    gesehen.extend(neu.iter().cloned());
    artefakte_umziehen(&neu, &fruehere_namen());
}

/// Wo im Repositorium die Artefakte liegen.
pub const ARTEFAKTORDNER: &str = "INTEGER_LLM/artifacts";

/// [`suchorte`] ohne Datei und Umgebung, damit es sich pruefen laesst.
///
/// 📌 **Fund 554: Ohne eingestelltes Artefakt suchte die Wahl nirgends.**
/// Frische Einstellungen tragen `modell.artefakt = ""`, und ein leerer Pfad
/// hat keinen Elternordner. Solange im Repositorium noch etwas zu bauen war,
/// fiel das nicht auf, denn dann stand die Bauliste da; mit allen Modellen
/// gebaut meldete die Konsole „hier liegt kein gebautes Artefakt" vor zwoelf
/// Artefakten. Gefunden beim Probelauf des Minings mit abgeschirmten
/// Einstellungen. Jetzt ist der Artefaktordner des Repositoriums der
/// Rueckfall.
pub fn suchorte_aus(
    eingestellt: &std::path::Path,
    s: &crate::mining::Speicherorte,
    wurzel: Option<&std::path::Path>,
) -> Vec<std::path::PathBuf> {
    let mut orte: Vec<std::path::PathBuf> = match eingestellt.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(p) => vec![p.to_path_buf()],
        None => wurzel.map(|w| vec![w.join(ARTEFAKTORDNER)]).unwrap_or_default(),
    };
    for o in s.fuer(crate::mining::Zweck::Gewichte) {
        let d = o.ordner(crate::mining::Zweck::Gewichte);
        if !orte.contains(&d) {
            orte.push(d);
        }
    }
    orte
}

pub fn liste(e: &Einstellungen) -> Vec<Modellwahl> {
    let katalog = katalog();
    let mut oertlich: Vec<Modellwahl> = Vec::new();

    let aufgeloest = myl_local_agent::ort::absolut(&e.modell.artefakt);
    let hier = std::path::Path::new(&aufgeloest);
    for eltern in suchorte(hier) {
        if let Ok(lesen) = std::fs::read_dir(&eltern) {
            let pfade: Vec<std::path::PathBuf> = lesen
                .flatten()
                .map(|x| x.path())
                .filter(|p| p.join("model_config.json").is_file())
                .filter(|p| !oertlich.iter().any(|m| std::path::Path::new(&m.pfad) == p.as_path()))
                .collect();
            for p in pfade {
                let ordner =
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let k = katalog.get(&ordner);
                let privat = k.is_some_and(|x| x.privat) || nur_lokal(&p);
                oertlich.push(Modellwahl {
                    pfad: p.display().to_string(),
                    // ⚑ Der Anzeigename kommt aus dem Katalog, wenn er
                    // dort steht, sonst aus [`name_nach_schema`]; erst
                    // danach bleibt der Verzeichnisname. 📌 Bis CLIENT
                    // v0.168.0 fehlte hier das Schema, und Artefakte ohne
                    // Katalogeintrag standen beim Ordnernamen in der Wahl.
                    //
                    // ⚑ Ein privates Modell sagt es im Namen, an jeder
                    // Stelle, an der jemand waehlt (2026-10-19).
                    name: privat_benannt(
                        k.map(|x| x.name.clone()).or_else(|| name_nach_schema(&ordner)).unwrap_or_else(|| ordner.clone()),
                        privat,
                    ),
                    offen: true,
                    warum: String::new(),
                    hardware: k.map(|x| x.hardware.clone()).unwrap_or_default(),
                    ueber_netz: false,
                    privat,
                });
            }
        }
    }
    ordnen(&mut oertlich, &katalog);

    // ⚑ **Die Netzeintraege entstehen aus dem, was gefunden wurde**, und
    // zwar bevor der Rueckfalleintrag dazukommt: Ein Pfad, der hier
    // nicht liegt, ist auch im Netz keine Zusage.
    //
    // ⛔️ **Ein privates Modell bekommt keinen** (2026-10-19): Es ist fuer
    // den Netzbetrieb nicht zugelassen.
    let mut netz: Vec<Modellwahl> = oertlich
        .iter()
        .filter(|m| m.offen && !m.privat)
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
            privat: false,
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
    if !oertlich.iter().any(|m| myl_local_agent::ort::absolut(&m.pfad) == aufgeloest) {
        oertlich.insert(
            0,
            Modellwahl {
                pfad: e.modell.artefakt.clone(),
                name: format!("{} (eingestellt)", e.modell.artefakt),
                offen: hier.join("model_config.json").is_file(),
                warum: "Der eingestellte Pfad enthaelt kein `model_config.json`.".into(),
                hardware: String::new(),
                ueber_netz: false,
                privat: nur_lokal(hier),
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
    let Some(w) = myl_local_agent::ort::wurzel() else { return BTreeMap::new() };
    let haupt = std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json")).unwrap_or_default();
    let lokal = std::fs::read_to_string(w.join(KATALOG_LOKAL)).ok();
    katalog_aus(&haupt, lokal.as_deref())
}

/// **Der Katalog aus seinen beiden Dateien**: dem des Projekts und dem
/// lokalen der selbst gebauten Modelle ([`KATALOG_LOKAL`]).
///
/// ⚑ **Das Projekt gewinnt**: Einen Schluessel, den beide kennen, liefert der
/// Katalog des Projekts. Ein lokaler Eintrag ohne `anzeigename` heisst nach
/// dem Schema ([`name_nach_schema`]) oder wie sein Ordner, und er ist immer
/// privat.
pub fn katalog_aus(haupt: &str, lokal: Option<&str>) -> BTreeMap<String, Katalogeintrag> {
    let mut aus = BTreeMap::new();
    if let Some(o) = serde_json::from_str::<serde_json::Value>(haupt).ok().as_ref().and_then(|d| d.as_object()) {
        projekteintraege(o, &mut aus);
    }
    let lokal = lokal.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
    for (k, v) in lokal.as_ref().and_then(|d| d.as_object()).into_iter().flatten() {
        if k.starts_with('_') || aus.contains_key(k) {
            continue;
        }
        let name = v
            .get("anzeigename")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .or_else(|| name_nach_schema(k))
            .unwrap_or_else(|| k.clone());
        aus.insert(
            k.clone(),
            Katalogeintrag {
                name,
                hardware: v.get("hardware").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                reihung: v.get("reihung").and_then(|x| x.as_f64()).unwrap_or(f64::MAX),
                privat: true,
            },
        );
    }
    aus
}

/// Ein Name mit dem Zusatz, den ein privates Modell in der Wahl traegt.
fn privat_benannt(name: String, privat: bool) -> String {
    if privat { format!("{name} (privat)") } else { name }
}

/// Die Eintraege aus dem Katalog des Projekts.
fn projekteintraege(o: &serde_json::Map<String, serde_json::Value>, aus: &mut BTreeMap<String, Katalogeintrag>) {
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
                privat: v.get("betrieb").and_then(|x| x.as_str()) == Some("lokal"),
            },
        );
    }
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
/// ⚑ Der Katalog kennt die eigenen; fuer ein Artefakt dieses Projekts, das
/// dort nicht steht, gilt [`name_nach_schema`], und fuer alles andere bleibt
/// der Verzeichnisname. **Ein Name, den niemand vergeben hat, waere
/// schlechter als ein technischer**; die Regel ist einer, den der
/// Projektinhaber vergeben hat.
pub fn anzeigename(pfad: &str) -> String {
    let ordner = std::path::Path::new(pfad)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pfad.to_string());
    katalognamen()
        .get(&ordner)
        .cloned()
        .or_else(|| name_nach_schema(&ordner))
        .unwrap_or(ordner)
}

/// **Der Name nach dem Schema „<Tier> <Groesse> - <Format>“**, etwa
/// `raven-35b-a3b-int4` zu „Raven 35B A3B - INT4“.
///
/// ⚑ **Festlegung des Projektinhabers, 2026-10-21.** Vorne das Tier (bei
/// einem selbst gebauten Modell das Wort, das der Nutzer gab), dann die
/// Groesse wie im Verzeichnisnamen, gross geschrieben, bei einem Gemisch mit
/// den aktiven Parametern (`A3B`), hinter einem Strich die Zahlform der
/// Gewichte (`INT8`, `INT4`, `TERNÄR`, `GEMISCHT 50`). Kein Name traegt mehr
/// „Myelith“; bis zum 2026-10-21 hiess es „Myelith 35B-A3B INT8“. Ein alter
/// Ordner ohne Zahlform (`myelith-4b`) zeigt `INT8`, wie damals.
///
/// ⚑ **Die Regel ist die Quelle, der Katalog folgt ihr.** Er traegt die
/// Namen weiter ausgeschrieben, weil auch Werkzeuge ohne diese Kiste sie
/// lesen; `die_katalognamen_folgen_dem_schema` haelt beide beisammen.
///
/// `None` fuer einen Ordner ohne Groesse (`<zahl>b`).
pub fn name_nach_schema(ordner: &str) -> Option<String> {
    let teile: Vec<&str> = ordner.split('-').collect();
    let ist_groesse = |g: &str| {
        g.len() > 1
            && g.starts_with(|c: char| c.is_ascii_digit())
            && g.ends_with('b')
            && g[..g.len() - 1].chars().all(|c| c.is_ascii_digit() || c == '.')
    };
    let stelle = teile.iter().position(|t| ist_groesse(t))?;
    let gross = |w: &str| {
        let mut z = w.chars();
        z.next().map(|c| c.to_uppercase().collect::<String>() + z.as_str()).unwrap_or_default()
    };
    let wort: Vec<String> = teile[..stelle].iter().map(|w| gross(w)).collect();
    let mut groesse = teile[stelle].to_ascii_uppercase();
    let mut rest = teile[stelle + 1..].iter().copied().peekable();
    let ist_aktiv = |t: &&str| {
        t.len() > 2 && t.starts_with('a') && t.ends_with('b') && t[1..t.len() - 1].chars().all(|c| c.is_ascii_digit())
    };
    if let Some(aktiv) = rest.next_if(ist_aktiv) {
        groesse = format!("{groesse} {}", aktiv.to_ascii_uppercase());
    }
    let format: Vec<String> = rest
        .map(|t| match t {
            "ternaer" => "TERNÄR".to_string(),
            anderes => anderes.to_uppercase(),
        })
        .collect();
    let format = if format.is_empty() { "INT8".to_string() } else { format.join(" ") };
    Some(if wort.is_empty() { format!("{groesse} - {format}") } else { format!("{} {groesse} - {format}", wort.join(" ")) })
}

// ── Welches Modell zu dieser Maschine passt ─────────────────────────────

/// Ein Artefakt, wie die Empfehlung es sieht.
#[derive(Debug, Clone, PartialEq)]
pub struct Kandidat {
    pub pfad: String,
    pub name: String,
    /// Bytes auf dem Datentraeger.
    pub groesse: u64,
    /// Ein Expertengemisch: Je Token rechnen nur wenige Experten.
    pub gemisch: bool,
    /// Parameter in Milliarden aus dem Katalog, `None` wenn er es nicht weiss.
    pub reihung: Option<f64>,
}

/// Was die Empfehlung waehlt, und warum.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Empfehlung {
    pub pfad: String,
    pub name: String,
    pub grund: String,
}

/// Luft neben einem dichten Modell: System, KV-Speicher, Fenster.
const LUFT_DICHT: u64 = 6 << 30;

/// **Welches Modell am besten zu dieser Maschine passt** (Wunsch des
/// Projektinhabers, 2026-09-29: beim Start ermitteln und schon laden).
///
/// ⚑ **Das groesste, das passt.** Ein dichtes Modell liest je Token alle
/// Gewichte; es passt, wenn es samt 6 GiB Luft in den Arbeitsspeicher
/// geht, sonst lagert es dauernd aus. Ein Gemisch liest je Token nur
/// wenige Experten und darf bis zum Anderthalbfachen des Speichers gross
/// sein: gemessen 33 GB (35B-A3B) auf 24 GiB mit 12 bis 15 Token/s. Unter
/// denen, die passen, gewinnt die groessere Parameterzahl aus dem Katalog,
/// danach die **kleinere** Datei. Passt keines, das kleinste.
///
/// ⚑ **Bei gleicher Parameterzahl die kleinere Zahlform** (Festlegung des
/// Projektinhabers, 2026-10-17: int4 ist der neue Standard). Gleiche
/// Parameterzahl heisst dasselbe Modell in anderer Form; das 35B in vier Bit
/// liegt 0,06 % ueber dem int8, rechnet GSM8K gleich und ist 19 statt 33 GB
/// gross. 📌 Bis CLIENT v0.168.0 gewann hier die groessere Datei, also das
/// int8. Eine Parameterzahl vergibt nur der Katalog, und er fuehrt nur
/// abgenommene Formen; ein ternaeres Artefakt ohne Eintrag zaehlt mit null.
///
/// ⚠️ Eine Faustregel aus den Messungen dieser Maschine, keine Zusage fuer
/// jede; wer ein Modell eingestellt hat, behaelt es (`modell_vorladen`).
pub fn empfehlung_fuer(kandidaten: &[Kandidat], speicher: u64) -> Option<Empfehlung> {
    let passt = |k: &&Kandidat| {
        if k.gemisch {
            k.groesse <= speicher / 2 * 3
        } else {
            k.groesse + LUFT_DICHT <= speicher
        }
    };
    let gib = speicher >> 30;
    let bester = kandidaten
        .iter()
        .filter(passt)
        .max_by(|a, b| {
            a.reihung
                .unwrap_or(0.0)
                .total_cmp(&b.reihung.unwrap_or(0.0))
                .then(b.groesse.cmp(&a.groesse))
        });
    match bester {
        Some(k) => Some(Empfehlung {
            pfad: k.pfad.clone(),
            name: k.name.clone(),
            grund: format!("das größte Modell, das auf diese Maschine passt ({gib} GiB Arbeitsspeicher)"),
        }),
        None => kandidaten.iter().min_by_key(|k| k.groesse).map(|k| Empfehlung {
            pfad: k.pfad.clone(),
            name: k.name.clone(),
            grund: format!("keines passt ganz in {gib} GiB, also das kleinste"),
        }),
    }
}

/// Die Artefakte, die hier liegen: neben dem eingestellten und unter
/// `INTEGER_LLM/artifacts` des Klons.
pub fn kandidaten(e: &Einstellungen) -> Vec<Kandidat> {
    let katalog = katalog();
    let mut orte: Vec<std::path::PathBuf> = Vec::new();
    if !e.modell.artefakt.is_empty() {
        if let Some(eltern) = std::path::Path::new(&myl_local_agent::ort::absolut(&e.modell.artefakt)).parent() {
            orte.push(eltern.to_path_buf());
        }
    }
    if let Some(w) = myl_local_agent::ort::wurzel() {
        orte.push(w.join("INTEGER_LLM").join("artifacts"));
    }
    let mut aus: Vec<Kandidat> = Vec::new();
    for ort in orte {
        let Ok(lesen) = std::fs::read_dir(&ort) else { continue };
        for p in lesen.flatten().map(|x| x.path()) {
            let konfig = p.join("model_config.json");
            if !konfig.is_file() || aus.iter().any(|k| std::path::Path::new(&k.pfad) == p) {
                continue;
            }
            let ordner = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let gemisch = std::fs::read_to_string(&konfig)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|c| c.get("num_experts").and_then(|n| n.as_u64()))
                .is_some_and(|n| n > 0);
            let k = katalog.get(&ordner);
            aus.push(Kandidat {
                pfad: p.display().to_string(),
                name: k.map(|x| x.name.clone()).or_else(|| name_nach_schema(&ordner)).unwrap_or_else(|| ordner.clone()),
                groesse: ordnergroesse(&p),
                gemisch,
                reihung: k.map(|x| x.reihung).filter(|r| *r > 0.0),
            });
        }
    }
    aus
}

/// Die Empfehlung fuer diese Maschine, oder `None`, wenn kein Artefakt da
/// ist oder der Arbeitsspeicher sich nicht ermitteln laesst.
pub fn empfehlung(e: &Einstellungen) -> Option<Empfehlung> {
    let speicher = crate::hardware::erheben(std::path::Path::new(".")).speicher_bytes?;
    empfehlung_fuer(&kandidaten(e), speicher)
}

fn ordnergroesse(p: &std::path::Path) -> u64 {
    let Ok(lesen) = std::fs::read_dir(p) else { return 0 };
    lesen
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => ordnergroesse(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod empfehlung_proben {
    use super::*;

    fn k(name: &str, gb: u64, gemisch: bool, reihung: Option<f64>) -> Kandidat {
        Kandidat { pfad: name.into(), name: name.into(), groesse: gb << 30, gemisch, reihung }
    }

    /// ⚑ **Auf 24 GiB das 35B-Gemisch**, nicht das dichte 14B, das dauernd
    /// auslagerte; auf 16 GiB das dichte 8B; auf 4 GiB, wo nichts passt, das
    /// kleinste.
    #[test]
    fn das_groesste_das_passt() {
        let alle = [
            k("0.6b", 1, false, Some(0.6)),
            k("8b", 9, false, Some(8.0)),
            k("14b", 16, false, Some(14.0)),
            k("27b-ternaer", 8, false, Some(27.0)),
            k("30b-a3b", 29, true, Some(30.0)),
            k("35b-a3b", 33, true, Some(35.0)),
            k("35b-a3b-int4", 19, true, Some(35.0)),
        ];
        // ⚑ Dasselbe Modell in zwei Formen: die kleinere ist der Standard.
        assert_eq!(empfehlung_fuer(&alle, 24 << 30).unwrap().name, "35b-a3b-int4");
        assert_eq!(empfehlung_fuer(&alle[..6], 24 << 30).unwrap().name, "35b-a3b", "ohne int4 das int8");
        assert_eq!(empfehlung_fuer(&alle[..3], 16 << 30).unwrap().name, "8b", "14B + Luft passt nicht in 16");
        let klein = empfehlung_fuer(&alle[1..3], 4 << 30).unwrap();
        assert_eq!(klein.name, "8b");
        assert!(klein.grund.contains("kleinste"), "{}", klein.grund);
        assert!(empfehlung_fuer(&[], 24 << 30).is_none());
    }

    /// ⚑ **Ausgelagerte Gewichte werden gefunden**: unter jedem Ort mit dem
    /// Zweck `gewichte`, nicht unter einem nur fuer das Netz.
    #[test]
    fn gesucht_wird_auch_unter_den_orten_fuer_gewichte() {
        use crate::mining::{Speicherort, Speicherorte, Zweck};
        let mut s = Speicherorte::default();
        s.setzen(Speicherort { name: "extern".into(), pfad: "/Volumes/P/M".into(), volumen_gib: 1, zwecke: vec![Zweck::Gewichte, Zweck::Netz] });
        s.setzen(Speicherort { name: "netz".into(), pfad: "/Volumes/N".into(), volumen_gib: 1, zwecke: vec![Zweck::Netz] });
        let orte = suchorte_aus(std::path::Path::new("/repo/INTEGER_LLM/artifacts/sparrow-0.6b-int8"), &s, None);
        assert_eq!(orte, [std::path::PathBuf::from("/repo/INTEGER_LLM/artifacts"), "/Volumes/P/M/gewichte".into()]);
    }

    /// 📌 **Fund 554**: Ohne eingestelltes Artefakt wird im Repositorium
    /// gesucht und nicht nirgends.
    #[test]
    fn ohne_eingestelltes_artefakt_sucht_die_wahl_im_repositorium() {
        let leer = crate::mining::Speicherorte::default();
        let w = std::path::Path::new("/repo");
        assert_eq!(suchorte_aus(std::path::Path::new(""), &leer, Some(w)), [w.join(ARTEFAKTORDNER)]);
        assert!(suchorte_aus(std::path::Path::new(""), &leer, None).is_empty());
    }

    /// ⚑ **Die Namen vor dem 2026-10-21 ziehen um**: genau, mit Anhaengsel,
    /// nie ueber ein bestehendes Verzeichnis, und der Katalog traegt fuer jedes
    /// Projektmodell seinen frueheren Namen.
    #[test]
    fn fruehere_namen_ziehen_um() {
        let w = myl_local_agent::ort::wurzel().expect("Repositorium");
        let paare = fruehere_namen_aus(&std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json")).unwrap());
        assert_eq!(neuer_name("myelith-35b-a3b", &paare).as_deref(), Some("raven-35b-a3b-int8"));
        assert_eq!(neuer_name("myelith-35b-a3b-int4", &paare).as_deref(), Some("raven-35b-a3b-int4"));
        assert_eq!(neuer_name("myelith-35b-a3b-ternaer", &paare).as_deref(), Some("raven-35b-a3b-ternaer"));
        assert_eq!(neuer_name("myelith-0.6b", &paare).as_deref(), Some("sparrow-0.6b-int8"));
        assert_eq!(neuer_name("myelith-8b-ternaer-gepackt", &paare).as_deref(), Some("jay-8b-ternaer-gepackt"));
        assert_eq!(neuer_name("raven-35b-a3b-int8", &paare), None);
        assert_eq!(neuer_name("myelith-4bx", &paare), None, "nur ganze Namen oder mit Strich");
        let d = std::env::temp_dir().join(format!("umzug-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for n in ["myelith-0.6b", "myelith-4b", "pigeon-4b-int8", "fremd"] {
            std::fs::create_dir_all(d.join(n)).unwrap();
        }
        std::fs::write(d.join("myelith-0.6b/model_config.json"), "{}").unwrap();
        let umgezogen = artefakte_umziehen(std::slice::from_ref(&d), &paare);
        assert_eq!(umgezogen.len(), 1, "{umgezogen:?}");
        assert!(d.join("sparrow-0.6b-int8/model_config.json").is_file());
        assert!(d.join("myelith-4b").is_dir(), "der neue Name lag schon da, also bleibt der alte");
        assert!(d.join("fremd").is_dir());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⚑ **Das Schema der Modellnamen** (Festlegung des Projektinhabers,
    /// 2026-10-21), auch fuer Artefakte, die der Katalog nicht kennt.
    #[test]
    fn die_namen_folgen_dem_schema() {
        for (ordner, name) in [
            ("raven-35b-a3b-int8", "Raven 35B A3B - INT8"),
            ("raven-35b-a3b-int4", "Raven 35B A3B - INT4"),
            ("sparrow-0.6b-int8", "Sparrow 0.6B - INT8"),
            ("jay-8b-ternaer-gepackt", "Jay 8B - TERNÄR GEPACKT"),
            ("raven-35b-a3b-gemischt-50", "Raven 35B A3B - GEMISCHT 50"),
            ("crow-27b-ternaer", "Crow 27B - TERNÄR"),
            ("otter-126b-a7b-int4", "Otter 126B A7B - INT4"),
            ("126b-a7b-int8", "126B A7B - INT8"),
            ("myelith-4b", "Myelith 4B - INT8"),
        ] {
            assert_eq!(name_nach_schema(ordner).as_deref(), Some(name), "{ordner}");
        }
        assert_eq!(name_nach_schema("Qwen3-4B"), None);
        assert_eq!(name_nach_schema("myelith-neu"), None);
    }

    /// ⚑ **Der Katalog folgt der Regel**, damit dasselbe Modell nicht an zwei
    /// Stellen zwei Namen traegt.
    #[test]
    fn die_katalognamen_folgen_dem_schema() {
        // ⚑ Nur der Katalog des Projekts: Ein lokaler Eintrag darf anders
        // heissen und traegt dann seinen Ordnernamen.
        let w = myl_local_agent::ort::wurzel().expect("Repositorium");
        let haupt = std::fs::read_to_string(w.join("MODELS/llm/KATALOG.json")).unwrap_or_default();
        let namen: BTreeMap<String, String> =
            katalog_aus(&haupt, None).into_iter().map(|(k, v)| (k, v.name)).collect();
        assert!(!namen.is_empty(), "der Katalog wurde nicht gefunden");
        for (ordner, name) in namen {
            assert_eq!(name_nach_schema(&ordner).as_deref(), Some(name.as_str()), "{ordner} im Katalog");
        }
    }

    /// ⛔️ **Private Modelle aus beiden Katalogen**: Ein lokaler Eintrag ist
    /// immer privat, auch wenn er etwas anderes sagt; bei gleichem Schluessel
    /// gewinnt das Projekt; ohne Anzeigenamen gilt das Schema, sonst der
    /// Ordner. Im Projektkatalog macht `betrieb = "lokal"` privat.
    #[test]
    fn private_modelle_aus_beiden_katalogen() {
        let haupt = r#"{"_hinweis": "x",
            "pigeon-4b-int8": {"anzeigename": "Pigeon 4B - INT8", "reihung": 4.0},
            "vulture-126b-a7b-int8": {"anzeigename": "Vulture 126B A7B - INT8", "reihung": 126.0, "betrieb": "lokal"}}"#;
        let lokal = r#"{"pigeon-4b-int8": {"anzeigename": "Faelschung"},
            "otter-126b-a6b-int4": {"betrieb": "netz", "reihung": 126.0},
            "mein-modell": {}}"#;
        let k = katalog_aus(haupt, Some(lokal));
        assert!(!k["pigeon-4b-int8"].privat);
        assert_eq!(k["pigeon-4b-int8"].name, "Pigeon 4B - INT8", "das Projekt gewinnt");
        assert!(k["vulture-126b-a7b-int8"].privat, "betrieb lokal im Projektkatalog");
        assert!(k["otter-126b-a6b-int4"].privat, "lokal ist immer privat");
        assert_eq!(k["otter-126b-a6b-int4"].name, "Otter 126B A6B - INT4");
        assert_eq!(k["mein-modell"].name, "mein-modell");
        assert_eq!(katalog_aus(haupt, None).len(), 2);
        assert_eq!(privat_benannt("Otter 4B - INT8".into(), true), "Otter 4B - INT8 (privat)");
    }

    /// ⚑ **Privat ist, was nicht auf der Netzliste steht** (Fund 585), und
    /// `betrieb = "lokal"` bleibt privat.
    #[test]
    fn nur_lokal_folgt_der_netzliste() {
        let d = std::env::temp_dir().join(format!("myl-nur-lokal-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(!nur_lokal(&d), "ohne model_config.json kein Artefakt, also nicht privat");
        std::fs::write(d.join("model_config.json"), r#"{"family": "qwen4-exp", "betrieb": "lokal"}"#).unwrap();
        assert!(nur_lokal(&d));
        std::fs::write(d.join("model_config.json"), r#"{"family": "qwen3"}"#).unwrap();
        assert!(nur_lokal(&d), "ohne Abdruck auf der Netzliste privat, auch ohne `betrieb`");
        std::fs::remove_dir_all(&d).ok();

        // Das echte 0,6B steht auf der Liste; die drei Dateien des Abdrucks
        // reichen, um es zu erkennen. Ohne Artefakt (CI) uebersprungen.
        let echt = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../INTEGER_LLM/artifacts/sparrow-0.6b-int8");
        if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_none() && echt.join("theta_v.json").is_file() {
            assert!(!nur_lokal(&echt), "sparrow-0.6b-int8 steht auf der Netzliste");
        }
    }

}
