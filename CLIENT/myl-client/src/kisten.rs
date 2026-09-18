//! Werkzeugkisten als Ordner: ein Werkzeug ist eine Manifestdatei, die zur
//! Laufzeit gelesen wird, ohne dass etwas neu gebaut werden muss.
//!
//! # ⚑ Warum Ordner und Manifeste (Auftrag des Projektinhabers, 2026-09-14)
//!
//! Die eingebauten Dateiwerkzeuge (`werkzeuge.rs`) sind kompilierter Rust:
//! Ein neues braucht einen Neubau. Der Wunsch ist das Gegenteil: **eine
//! Datei in einen Kisten-Ordner legen, und das Modell sieht das Werkzeug.**
//! Also ist ein Werkzeug hier eine kleine JSON-Datei mit Name,
//! Beschreibung, Argumentschema und einer **Befehlsvorlage**; ausgefuehrt
//! wird sie ueber `werkzeuge::befehl_im_verzeichnis`, also denselben
//! gepruefen Weg wie `run_command`.
//!
//! # ⛔️ Sicherheit: dieselbe Klasse wie run_command, und deshalb dieselben
//!    Schranken
//!
//! Ein Manifest-Werkzeug fuehrt `sh -c` aus und kann damit alles, was der
//! Prozess kann; es haelt die Einhaengegrenze **nicht** ein (siehe
//! `Befehlausfuehren`). Zwei Folgen:
//!
//!   - **Es braucht die Schreiberlaubnis**, ausnahmslos, und steht ohne sie
//!     nicht im Angebot. Ein „nur lesendes" Manifest gibt es nicht, denn die
//!     Shell kann immer schreiben.
//!   - **Die Argumente werden shell-sicher eingesetzt** (`shell_quote`),
//!     sonst waere `{muster}` ein Einfallstor: Ein Modell, das
//!     `muster = "; rm -rf ~"` liefert, darf keine zweite Kommandozeile
//!     einschleusen.
//!
//! Die echte Grenze bleibt eine des Betriebssystems (Modulkopf von
//! `werkzeuge.rs`); bis dahin ist ein Manifest-Werkzeug so vertrauenswuerdig
//! wie die Kiste, aus der es kommt, und die Person am `manual mode`.

use std::path::{Path, PathBuf};

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use myl_local_agent::werkzeug::Werkzeug;
use serde::Deserialize;

use crate::werkzeuge::{befehl_im_verzeichnis_mit, Einhaengung, Werkzeugkiste, BEFEHL_ZEITGRENZE_S};

/// Ein Werkzeug, wie es als Datei in einer Kiste liegt.
#[derive(Debug, Clone, Deserialize)]
pub struct Werkzeugmanifest {
    /// Der Name, unter dem das Modell es aufruft. ⚑ **Nicht der
    /// Dateiname**: Der Dateiname darf `.json` tragen und Umlaute meiden,
    /// der Werkzeugname folgt der Konvention der eingebauten (`snake_case`).
    pub name: String,
    /// Wofuer es da ist, fuer die Ansage an das Modell.
    pub beschreibung: String,
    /// Das Argumentschema (JSON-Schema-Objekt), wie bei den eingebauten.
    pub parameter: serde_json::Value,
    /// Die Befehlsvorlage. `{feld}` wird durch das shell-sicher zitierte
    /// Argument `feld` ersetzt; `{{` und `}}` stehen fuer geschweifte
    /// Klammern.
    ///
    /// ⚑ **`$MYL_KISTE` ist der Ordner, in dem dieses Manifest liegt.**
    /// Damit ruft ein Manifest ein Skript neben sich auf, ohne seinen
    /// eigenen Pfad zu kennen: `sh "$MYL_KISTE/tue_etwas.sh" {datei}`.
    /// Ein absoluter Pfad im Manifest waere auf jeder anderen Maschine
    /// falsch, und ein relativer zeigte in den Arbeitsordner, wo das
    /// Skript nicht liegt.
    pub befehl: String,
    /// Wie lange dieses Werkzeug laufen darf, in Sekunden.
    ///
    /// ⚑ **Ohne Angabe gilt die Vorgabe** ([`BEFEHL_ZEITGRENZE_S`], 30 s).
    /// Sie reicht fuer `wc` und `grep` und **nicht** fuer ein Werkzeug,
    /// das ein Modell von der Platte laedt: Ein kalter Start eines
    /// Sehmodells dauert laenger als die ganze Frist, und der Abbruch
    /// saehe wie ein kaputtes Werkzeug aus. Wer laenger braucht, sagt es
    /// hier; nach oben schliesst [`ZEITGRENZE_HOECHSTENS_S`] ab, damit
    /// eine Kiste die Schleife nicht beliebig lange anhaelt.
    #[serde(default)]
    pub zeitgrenze_s: Option<u64>,
    /// Welche Art Anhang dieses Werkzeug lesbar macht: `bild`, `ton`,
    /// `text`, `sonstiges`.
    ///
    /// ⚑ **Damit nennt die Anhangnachricht das Werkzeug beim Namen**,
    /// ohne dass der Name im Rust-Quelltext steht. Stuende er dort,
    /// staende er an zwei Orten, und eine umbenannte Manifestdatei
    /// wuerde still zu einem Hinweis auf ein Werkzeug, das es nicht mehr
    /// gibt. Wer ein eigenes Sehwerkzeug mitbringt, traegt hier `bild`
    /// ein und wird genauso genannt.
    #[serde(default)]
    pub fuer: Vec<String>,
}

/// Wie lange ein Manifest sich hoechstens Zeit nehmen darf, in Sekunden.
///
/// ⚑ **Fuenf Minuten**, und keine Angabe im Manifest hebt das auf. Ein
/// Werkzeug, das nicht zurueckkommt, haelt die Schleife an
/// ([`BEFEHL_ZEITGRENZE_S`]); dass ein Sinneswerkzeug laenger braucht,
/// aendert daran nichts, es verschiebt nur die Grenze.
pub const ZEITGRENZE_HOECHSTENS_S: u64 = 300;

/// Der Name der Umgebungsvariable, unter der ein Manifest seinen eigenen
/// Ordner findet.
pub const UMGEBUNG_KISTE: &str = "MYL_KISTE";

/// ⚑ **Der einzige reservierte Dateiname in einem Kistenordner.**
///
/// Er beschreibt die **Kiste**, nicht ein Werkzeug, und wird deshalb von
/// [`manifeste_lesen`] uebersprungen. Ein Werkzeug darf so nicht heissen.
pub const KISTENDATEI: &str = "kiste.json";

/// Was ein Kistenordner ueber sich selbst sagt.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Kistenblatt {
    /// Welche **eingebauten** Werkzeuge dazugehoeren: `Base`, `Advanced`
    /// oder `1337`. Ohne Angabe entscheidet der Ordnername.
    #[serde(default)]
    pub eingebaute: Option<String>,
}

/// **Welche eingebauten Werkzeuge ein Ordner fuer sich verlangt.**
///
/// # ⛔️ Fund 394: der Ordnername war die Ansage (2026-09-17)
///
/// Die eingebauten Werkzeuge kamen aus [`Werkzeugkiste::aus_ordnername`],
/// und die kennt drei Woerter; alles andere ist `Base`. Wer also einen
/// **eigenen** Ordner waehlte, fiel **stillschweigend** auf die fuenf
/// Grundwerkzeuge zurueck und verlor unter anderem die Suche im
/// Mitschnitt, ohne dass irgendwo etwas stand. Aufgefallen beim Anlegen
/// der Kiste `Sinne`, die genau so heisst wie keine der drei.
///
/// ⚑ **Jetzt sagt die Kiste es selbst**, in ihrem [`KISTENDATEI`]; der
/// Ordnername bleibt der Rueckfall fuer die drei mitgelieferten.
/// 📌 **Ein Name, der zwei Dinge bedeutet, bedeutet irgendwann nur noch
/// eines.**
pub fn eingebaute_des_ordners(ordner: Option<&Path>) -> Option<Werkzeugkiste> {
    let blatt: Kistenblatt = std::fs::read_to_string(ordner?.join(KISTENDATEI))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())?;
    let wort = blatt.eingebaute?;
    // ⚠️ **Nur die bekannten Woerter**, sonst waere ein Tippfehler wieder
    // ein stiller Rueckfall auf `Base`. `None` heisst hier: der
    // Ordnername entscheidet, wie bisher.
    match wort.trim().to_ascii_lowercase().as_str() {
        "base" => Some(Werkzeugkiste::Base),
        "advanced" => Some(Werkzeugkiste::Advanced),
        "1337" => Some(Werkzeugkiste::Elite),
        _ => None,
    }
}

impl Werkzeugmanifest {
    /// **Die Frist, die fuer dieses Werkzeug wirklich gilt**, in Sekunden.
    ///
    /// ⚑ **Eine Stelle rechnet das aus.** Vorgabe, eigene Angabe und
    /// Deckel gehoeren zusammen; stuende der Deckel nur an der
    /// Ausfuehrstelle, waere er dort zu pruefen und nirgends zu sehen.
    pub fn frist_s(&self) -> u64 {
        self.zeitgrenze_s.unwrap_or(BEFEHL_ZEITGRENZE_S).min(ZEITGRENZE_HOECHSTENS_S)
    }
}

/// **Liest die Manifeste einer Kiste**, also die `*.json` eines Ordners.
///
/// ⚑ **Eine kaputte Datei ueberspringt sie mit einem Vermerk**, statt die
/// ganze Kiste fallenzulassen: Ein Tippfehler in einem Werkzeug soll nicht
/// die anderen mitnehmen. Die Vermerke gehen an `warnung`.
pub fn manifeste_lesen(ordner: &Path, mut warnung: impl FnMut(String)) -> Vec<Werkzeugmanifest> {
    let mut aus = Vec::new();
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return aus;
    };
    let mut pfade: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| p.file_name().is_none_or(|n| n != KISTENDATEI))
        .collect();
    pfade.sort();
    for p in pfade {
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| {
            serde_json::from_str::<Werkzeugmanifest>(&t).map_err(|e| e.to_string())
        }) {
            Ok(m) => aus.push(m),
            Err(e) => warnung(format!("{}: {e}", p.display())),
        }
    }
    aus
}

/// **Die Angebote und Ausfuehrungen einer Kiste**, fertig zum Einhaengen.
///
/// ⚑ **Nur mit Schreiberlaubnis.** Ohne sie kommt die Kiste leer zurueck:
/// Ein Manifest-Werkzeug wirkt (Modulkopf), also gilt dieselbe Erlaubnis
/// wie beim Schreiben.
pub fn angebote(
    ordner: &Path,
    ein: &Einhaengung,
    warnung: impl FnMut(String),
) -> Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    if !ein.darf_schreiben() {
        return Vec::new();
    }
    manifeste_lesen(ordner, warnung)
        .into_iter()
        .map(|m| {
            let werkzeug = Werkzeug {
                name: m.name.clone(),
                beschreibung: m.beschreibung.clone(),
                parameter: m.parameter.clone(),
            };
            let ausf: Box<dyn Werkzeugausfuehrung> = Box::new(ManifestWerkzeug {
                manifest: m,
                einhaengung: ein.clone(),
                ordner: ordner.to_path_buf(),
            });
            (werkzeug, ausf)
        })
        .collect()
}

/// **Die Werkzeugkiste, die gilt, aus den Einstellungen.**
///
/// ⚑ **Eine Stelle, die das beantwortet** (2026-09-15). Fenster,
/// Konsole und Ruestung fragen hier; zwei Ableitungen derselben Wahl
/// liefen auseinander, und die zweite meldet sich nicht. Der Ordner
/// entscheidet, sein Name sagt die Kiste, ohne Angabe ist es `Base`.
pub fn kiste_der_gilt(agent: &crate::einstellungen::Agenteneinstellung) -> Werkzeugkiste {
    let ordner = ordner_der_gilt(agent.kistenordner.as_deref(), Werkzeugkiste::Base.name());
    eingebaute_des_ordners(ordner.as_deref()).unwrap_or_else(|| {
        Werkzeugkiste::aus_ordnername(&ordnername(ordner.as_deref(), Werkzeugkiste::Base.name()))
    })
}

/// **Alle Ordner, aus denen Manifeste kommen, in dieser Reihenfolge.**
///
/// ⚑ **Base ist immer dabei** (Auftrag des Projektinhabers,
/// 2026-09-15): „Im Advanced-Ordner sollen selbstverstaendlich auch alle
/// Base-Werkzeuge vorhanden sein." Das gilt hier durch Stapeln und
/// **nicht durch Kopieren**: Dieselbe Manifestdatei in zwei Ordnern
/// waeren zwei Orte, die auseinanderlaufen, und der zweite meldet sich
/// nicht. Genau so ist es bei den eingebauten Werkzeugen auch, wo
/// `Advanced` die Aufzaehlung `BASE` mitnimmt statt sie abzuschreiben.
///
/// ⚑ **Der gewaehlte Ordner kommt zuletzt und gewinnt** bei gleichem
/// Werkzeugnamen: Wer ein Base-Werkzeug ersetzen will, legt eines mit
/// demselben Namen in seine Kiste.
pub fn ordnerkette(agent: &crate::einstellungen::Agenteneinstellung) -> Vec<PathBuf> {
    let mut kette = Vec::new();
    if let Some(b) = kiste_ordner(Werkzeugkiste::Base.name()) {
        kette.push(b);
    }
    if let Some(o) = ordner_der_gilt(agent.kistenordner.as_deref(), Werkzeugkiste::Base.name()) {
        if !kette.contains(&o) {
            kette.push(o);
        }
    }
    kette
}

/// **Die Angebote der ganzen Kette**, spaetere Ordner gewinnen.
pub fn angebote_der_kette(
    kette: &[PathBuf],
    ein: &Einhaengung,
    mut warnung: impl FnMut(String),
) -> Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    let mut aus: Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> = Vec::new();
    for ordner in kette {
        for (w, a) in angebote(ordner, ein, &mut warnung) {
            match aus.iter().position(|(v, _)| v.name == w.name) {
                Some(i) => aus[i] = (w, a),
                None => aus.push((w, a)),
            }
        }
    }
    aus
}

/// **Welches Werkzeug eine Art Anhang lesbar macht**, wenn es eines gibt.
///
/// ⚑ **Gefragt wird die Kette, nicht der Quelltext.** Ein Manifest sagt
/// mit seinem Feld `fuer`, wofuer es zustaendig ist; hier wird nur
/// nachgesehen. Spaetere Ordner gewinnen, wie beim Angebot auch.
///
/// ⚠️ **Ohne Schreiberlaubnis gibt es nichts zu nennen.** Manifest-
/// Werkzeuge stehen dann nicht im Angebot, und ein Hinweis auf ein
/// Werkzeug, das das Modell nicht aufrufen kann, ist schlimmer als
/// keiner.
pub fn werkzeug_fuer(agent: &crate::einstellungen::Agenteneinstellung, art: &str) -> Option<String> {
    if !agent.schreiben {
        return None;
    }
    let mut gefunden = None;
    for ordner in ordnerkette(agent) {
        for m in manifeste_lesen(&ordner, |_| {}) {
            if m.fuer.iter().any(|f| f.trim().eq_ignore_ascii_case(art)) {
                gefunden = Some(m.name);
            }
        }
    }
    gefunden
}

/// **Der Ordner, der wirklich gilt.**
///
/// ⚑ **Der gewaehlte Ordner ist die Kiste** (Festlegung des
/// Projektinhabers, 2026-09-15). In den Einstellungen steht nur noch ein
/// Pfad; ohne Angabe ist es der mitgelieferte `Base`-Ordner.
///
/// ⚠️ **Ein gesetzter Pfad, den es nicht gibt, faellt nicht still auf
/// die Vorgabe zurueck.** Sonst arbeitete der Agent aus einem anderen
/// Ordner als dem, der in den Einstellungen steht, und niemand saehe es.
/// `None` heisst dann: keine Manifest-Werkzeuge.
pub fn ordner_der_gilt(eigener: Option<&str>, kennung: &str) -> Option<PathBuf> {
    match eigener.map(str::trim).filter(|p| !p.is_empty()) {
        Some(pfad) => {
            let o = PathBuf::from(pfad);
            o.is_dir().then_some(o)
        }
        None => kiste_ordner(kennung),
    }
}

/// **Der Name, der zu einem Ordner angezeigt wird.**
///
/// ⚑ Genau der des Ordners, in dem die Werkzeuge liegen; ohne Ordner die
/// Kennung der Vorgabekiste.
pub fn ordnername(ordner: Option<&std::path::Path>, kennung: &str) -> String {
    ordner
        .and_then(|o| o.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| kennung.to_string())
}

/// **Alle Kisten, die nebeneinander liegen.**
///
/// ⚑ Die Ordner in der Kistenheimat, also neben der mitgelieferten
/// `Base`. Sortiert, damit zwei Aufrufe dieselbe Reihenfolge geben; ein
/// Menue, dessen Punkte springen, ist keines.
///
/// ⚑ **Eine selbst gewaehlte Kiste ausserhalb kommt mit**, sonst fiele
/// sie aus der Liste, sobald jemand sie einmal gewaehlt hat.
pub fn vorhandene(eigener: Option<&str>) -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = Vec::new();
    if let Some(heimat) = kiste_ordner(Werkzeugkiste::Base.name()).and_then(|o| o.parent().map(|p| p.to_path_buf())) {
        if let Ok(eintraege) = std::fs::read_dir(&heimat) {
            for e in eintraege.flatten() {
                let p = e.path();
                if p.is_dir() {
                    aus.push(p);
                }
            }
        }
    }
    aus.sort();
    if let Some(o) = eigener.map(str::trim).filter(|p| !p.is_empty()).map(PathBuf::from) {
        if o.is_dir() && !aus.contains(&o) {
            aus.push(o);
        }
    }
    aus
}

/// **Findet den mitgelieferten Ordner einer Kiste**, ohne dass etwas
/// fest verdrahtet ist.
///
/// ⚑ Erst die Umgebung `MYL_WERKZEUGKISTEN` (ein Basisverzeichnis,
/// darunter je Kennung ein Ordner), dann von diesem Verzeichnis aufwaerts
/// der erste `CLIENT/werkzeugkisten`. So findet ein Lauf aus dem
/// Repositorium die mitgelieferten Kisten, und wer sie woanders hat, sagt
/// es ueber die Umgebung. `None`, wenn nichts passt: dann bleibt es bei
/// den eingebauten Werkzeugen.
pub fn kiste_ordner(kennung: &str) -> Option<PathBuf> {
    if let Some(basis) = std::env::var_os("MYL_WERKZEUGKISTEN") {
        let o = PathBuf::from(basis).join(kennung);
        if o.is_dir() {
            return Some(o);
        }
    }
    // ⛔️ **Hier stand nur eine Suche vom Arbeitsverzeichnis aufwaerts**,
    // und die geht fuer das installierte Fenster immer ins Leere: Wer
    // `Myelith.app` aus dem Finder startet, hat als Arbeitsverzeichnis
    // `/`. Damit fand ein frisch eingerichteter Client **keine einzige
    // Kiste**: keine Manifest-Werkzeuge, kein Pfad in den Einstellungen,
    // und die Ordnerwahl ohne Startort. Gemeldet vom Projektinhaber am
    // 2026-09-15.
    //
    // 📌 **Und die eigenen Proben verdeckten es**, weil in der Einstellung
    // ein absoluter Pfad stand: Der wird ueberall gefunden, also lief
    // keine Probe je durch diese Suche. **Eine Probe, die den Weg nicht
    // nimmt, prueft ihn nicht.**
    //
    // ⚑ **`ort::wurzel` kennt den Weg schon**: Umgebung, dann
    // Arbeitsverzeichnis, dann der eigene Programmordner, und zuletzt der
    // gemerkte Zettel fuer den, der nirgendwo steht. Genau dafuer gibt es
    // ihn; ihn hier nicht zu benutzen war eine zweite, schlechtere Suche.
    let o = crate::ort::wurzel()?.join("CLIENT/werkzeugkisten").join(kennung);
    o.is_dir().then_some(o)
}

/// Ein geladenes Manifest, das sich ausfuehren laesst.
struct ManifestWerkzeug {
    manifest: Werkzeugmanifest,
    einhaengung: Einhaengung,
    /// Der Ordner, aus dem das Manifest kam. ⚑ Er steht dem Befehl als
    /// `$MYL_KISTE` zur Verfuegung, damit ein Werkzeug aus Manifest **und**
    /// Skript bestehen kann.
    ordner: PathBuf,
}

impl Werkzeugausfuehrung for ManifestWerkzeug {
    fn name(&self) -> &str {
        &self.manifest.name
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        if !self.einhaengung.darf_schreiben() {
            return Err(Werkzeugfehler {
                grund: "diese Sitzung darf nur lesen; ein Kisten-Werkzeug wirkt und braucht \
                        `myl setzen agent.schreiben an`"
                    .into(),
            });
        }
        let befehl = befehl_aus_vorlage(&self.manifest.befehl, a)?;
        let umgebung = [(UMGEBUNG_KISTE, self.ordner.display().to_string())];
        befehl_im_verzeichnis_mit(&self.einhaengung, &befehl, &umgebung, self.manifest.frist_s())
    }
}

/// **Setzt die Argumente shell-sicher in die Vorlage ein.**
///
/// `{feld}` wird durch [`shell_quote`] des Arguments `feld` ersetzt; `{{`
/// und `}}` bleiben als `{` und `}` stehen. Ein Platzhalter ohne Argument
/// ist ein Fehler, kein leeres Einsetzen: Ein Befehl mit einer Luecke, wo
/// eine Datei stehen sollte, taete etwas anderes als gemeint.
pub fn befehl_aus_vorlage(vorlage: &str, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
    let einzeln =
        || Werkzeugfehler { grund: "Befehlsvorlage: einzelne geschweifte Klammer; `{{`/`}}` fuer eine echte".into() };
    let mut aus = String::new();
    let mut rest = vorlage;
    while let Some(i) = rest.find(['{', '}']) {
        let (vor, ab) = rest.split_at(i);
        aus.push_str(vor);
        if let Some(r) = ab.strip_prefix("{{") {
            aus.push('{');
            rest = r;
        } else if let Some(r) = ab.strip_prefix("}}") {
            aus.push('}');
            rest = r;
        } else if let Some(feldundrest) = ab.strip_prefix('{') {
            let ende = feldundrest.find('}').ok_or_else(einzeln)?;
            let feld = &feldundrest[..ende];
            let wert = a.get(feld).ok_or_else(|| Werkzeugfehler {
                grund: format!("es fehlt das Feld `{feld}` fuer die Befehlsvorlage"),
            })?;
            aus.push_str(&shell_quote(&als_text(wert)));
            rest = &feldundrest[ende + 1..];
        } else {
            // Eine einzelne schliessende Klammer.
            return Err(einzeln());
        }
    }
    aus.push_str(rest);
    Ok(aus)
}

/// Der Wert eines Arguments als Text: Zeichenketten ohne Anfuehrungszeichen,
/// alles andere als sein JSON.
fn als_text(v: &serde_json::Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())
}

/// **Zitiert eine Zeichenkette fuer `sh`**, sodass sie ein einziges Wort
/// bleibt und nichts einschleust: in einfache Anfuehrungszeichen, ein
/// eingebettetes `'` wird zu `'\''`.
pub fn shell_quote(s: &str) -> String {
    let mut aus = String::with_capacity(s.len() + 2);
    aus.push('\'');
    for c in s.chars() {
        if c == '\'' {
            aus.push_str("'\\''");
        } else {
            aus.push(c);
        }
    }
    aus.push('\'');
    aus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kiste_mit(dateien: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let kiste = d.path().join("k");
        std::fs::create_dir(&kiste).expect("Kiste");
        for (name, inhalt) in dateien {
            std::fs::write(kiste.join(name), inhalt).expect("Datei");
        }
        (d, kiste)
    }

    #[test]
    fn ein_manifest_wird_zum_angebot_und_laeuft() {
        let (d, kiste) = kiste_mit(&[(
            "zeilen.json",
            r#"{"name":"zaehle_zeilen","beschreibung":"Zaehlt Zeilen einer Datei.",
               "parameter":{"type":"object","properties":{"datei":{"type":"string"}},"required":["datei"]},
               "befehl":"wc -l {datei}"}"#,
        )]);
        std::fs::write(d.path().join("drei.txt"), "a\nb\nc\n").expect("Datei");
        let ein = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        let mut warnungen = Vec::new();
        let angebote = angebote(&kiste, &ein, |w| warnungen.push(w));
        assert!(warnungen.is_empty(), "{warnungen:?}");
        assert_eq!(angebote.len(), 1);
        assert_eq!(angebote[0].0.name, "zaehle_zeilen");
        let aus = angebote[0].1.ausfuehren(&serde_json::json!({"datei": "drei.txt"})).expect("laeuft");
        assert!(aus.contains('3'), "{aus}");
    }

    /// **Ohne Schreiberlaubnis ist die Kiste leer.** Ein Manifest-Werkzeug
    /// wirkt, also gilt dieselbe Erlaubnis wie beim Schreiben.
    #[test]
    fn ohne_schreiberlaubnis_bietet_die_kiste_nichts() {
        let (d, kiste) = kiste_mit(&[(
            "x.json",
            r#"{"name":"x","beschreibung":"y","parameter":{"type":"object","properties":{},"required":[]},"befehl":"echo hallo"}"#,
        )]);
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        assert!(angebote(&kiste, &nur_lesen, |_| {}).is_empty());
    }

    /// ⛔️ **Ein Argument kann keine zweite Kommandozeile einschleusen.**
    #[test]
    fn die_argumente_werden_shell_sicher_eingesetzt() {
        let boese = serde_json::json!({"muster": "; touch /tmp/eingeschleust ; echo"});
        let befehl = befehl_aus_vorlage("grep {muster} datei", &boese).expect("ok");
        assert!(befehl.starts_with("grep '; touch /tmp/eingeschleust ; echo' datei"), "{befehl}");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    /// **Ein fehlendes Feld ist ein Fehler, keine Luecke.**
    #[test]
    fn ein_fehlendes_feld_bricht_ab() {
        assert!(befehl_aus_vorlage("cat {datei}", &serde_json::json!({})).is_err());
        // Doppelte Klammern bleiben stehen.
        assert_eq!(befehl_aus_vorlage("echo {{roh}}", &serde_json::json!({})).expect("ok"), "echo {roh}");
        // Einzelne schliessende Klammer ist ein Fehler.
        assert!(befehl_aus_vorlage("echo }", &serde_json::json!({})).is_err());
    }

    /// **Eine kaputte Manifestdatei nimmt die anderen nicht mit.**
    #[test]
    fn eine_kaputte_datei_wird_uebersprungen() {
        let (_d, kiste) = kiste_mit(&[
            ("kaputt.json", "{ das ist kein json"),
            ("gut.json", r#"{"name":"gut","beschreibung":"b","parameter":{"type":"object","properties":{},"required":[]},"befehl":"echo ok"}"#),
        ]);
        let ein = Einhaengung::neu(kiste.parent().unwrap(), true).expect("Einhaengung");
        let mut warnungen = Vec::new();
        let angebote = angebote(&kiste, &ein, |w| warnungen.push(w));
        assert_eq!(angebote.len(), 1, "die gute Datei bleibt");
        assert_eq!(warnungen.len(), 1, "die kaputte wird vermerkt");
        assert!(warnungen[0].contains("kaputt.json"));
    }

    /// ⛔️ **Jede Kiste hat ihren Ordner, buchstabengenau.**
    ///
    /// # 📌 Was dieses Dateisystem verdeckt (2026-09-15)
    ///
    /// macOS unterscheidet in Dateinamen **nicht** zwischen gross und
    /// klein. `Werkzeugkiste::kennung()` gab `"base"` zurueck, auf der
    /// Platte liegt `Base`, und hier hat das nie jemand gemerkt. Auf
    /// Linux, also in der CI und bei jedem, der das Projekt dort baut,
    /// waere derselbe Aufruf ins Leere gegangen: kein Ordner, keine
    /// Manifest-Werkzeuge, **keine Fehlermeldung**.
    ///
    /// ⚑ **Deshalb vergleicht diese Probe gegen den Verzeichniseintrag
    /// und nicht mit `is_dir`.** `is_dir` beantwortet die Frage auf
    /// dieser Maschine mit „ja", egal wie der Name geschrieben ist; nur
    /// der gelesene Eintrag sagt, wie er **wirklich** heisst. Eine
    /// Probe, die `is_dir` fragt, liefe hier gruen und in der CI rot.
    ///
    /// ⚠️ **`1337` ist gitignored** und darf fehlen. Fehlt er, bleibt
    /// nichts zu vergleichen, und das ist kein Fehler.
    #[test]
    fn jede_kiste_findet_ihren_ordner_mit_der_richtigen_schreibweise() {
        let Some(heimat) = crate::ort::wurzel().map(|w| w.join("CLIENT/werkzeugkisten")) else {
            return; // Kein Repositorium zur Hand, dann ist hier nichts zu pruefen.
        };
        let echte: Vec<String> = std::fs::read_dir(&heimat)
            .expect("die Kistenheimat")
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        for k in [Werkzeugkiste::Base, Werkzeugkiste::Advanced] {
            assert!(
                echte.iter().any(|n| n == k.name()),
                "{} heisst auf der Platte anders: {echte:?}. Auf macOS faellt das nicht auf, \
                 in der CI hat der Agent dann keine Manifest-Werkzeuge.",
                k.name()
            );
        }
        // Und was da ist, ist auch ueber den Namen erreichbar.
        for k in [Werkzeugkiste::Base, Werkzeugkiste::Advanced, Werkzeugkiste::Elite] {
            if echte.iter().any(|n| n == k.name()) {
                assert!(kiste_ordner(k.name()).is_some(), "{} ist nicht auffindbar", k.name());
            }
        }
    }

    /// ⚑ **Ein Manifest findet ein Skript neben sich**, ueber `$MYL_KISTE`.
    ///
    /// Ohne diese Variable kann ein Werkzeug nur aus einer Befehlszeile
    /// bestehen: Ein absoluter Pfad im Manifest waere auf jeder anderen
    /// Maschine falsch, ein relativer zeigte in den Arbeitsordner, wo das
    /// Skript nicht liegt. Diese Probe beisst, wenn die Variable fehlt:
    /// `sh` findet dann `/sinn.sh` nicht und das Kennwort steht nicht in
    /// der Ausgabe.
    #[test]
    fn ein_manifest_findet_sein_skript_neben_sich() {
        let (d, kiste) = kiste_mit(&[(
            "sinn.json",
            r#"{"name":"sinn","beschreibung":"b",
               "parameter":{"type":"object","properties":{"wort":{"type":"string"}},"required":["wort"]},
               "befehl":"sh \"$MYL_KISTE/sinn.sh\" {wort}"}"#,
        )]);
        std::fs::write(kiste.join("sinn.sh"), "#!/bin/sh\necho kennwort:\"$1\"\n").expect("Skript");
        let ein = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        let angebote = angebote(&kiste, &ein, |w| panic!("{w}"));
        let aus = angebote[0].1.ausfuehren(&serde_json::json!({"wort": "hallo"})).expect("laeuft");
        assert!(aus.contains("kennwort:hallo"), "das Skript neben dem Manifest lief nicht: {aus}");
    }

    /// ⚑ **Die Frist steht im Manifest, und der Deckel steht darueber.**
    ///
    /// Ein Sinneswerkzeug laedt ein Modell von der Platte und ist mit den
    /// 30 s der Vorgabe nicht fertig; ein Werkzeug ohne Ende haelt die
    /// Schleife an. Beides wird hier gemessen: dass die eigene Angabe
    /// wirkt, und dass sie nicht beliebig gross wird.
    #[test]
    fn die_frist_kommt_aus_dem_manifest_und_hat_einen_deckel() {
        let mach = |zeit: Option<u64>| Werkzeugmanifest {
            name: "x".into(),
            beschreibung: "b".into(),
            parameter: serde_json::json!({"type":"object","properties":{},"required":[]}),
            befehl: "echo x".into(),
            zeitgrenze_s: zeit,
            fuer: Vec::new(),
        };
        assert_eq!(mach(None).frist_s(), crate::werkzeuge::BEFEHL_ZEITGRENZE_S);
        assert_eq!(mach(Some(180)).frist_s(), 180);
        assert_eq!(mach(Some(99_999)).frist_s(), ZEITGRENZE_HOECHSTENS_S);

        // Und sie wirkt wirklich: ein Befehl, der laenger braucht, wird
        // nach genau dieser Zeit abgebrochen, nicht nach der Vorgabe.
        let (d, kiste) = kiste_mit(&[(
            "schlaf.json",
            r#"{"name":"schlaf","beschreibung":"b","parameter":{"type":"object","properties":{},"required":[]},
               "befehl":"sleep 5","zeitgrenze_s":1}"#,
        )]);
        let ein = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        let angebote = angebote(&kiste, &ein, |w| panic!("{w}"));
        let anfang = std::time::Instant::now();
        let aus = angebote[0].1.ausfuehren(&serde_json::json!({})).expect("laeuft");
        assert!(aus.contains("abgebrochen nach 1 s"), "{aus}");
        assert!(anfang.elapsed().as_secs() < 4, "die Frist aus dem Manifest hat nicht gegriffen");
    }

    /// ⛔️ **Fund 394: ein eigener Ordner verlor still die eingebauten
    /// Werkzeuge.** Diese Probe beisst, wenn `kiste.json` nicht gelesen
    /// wird: Der Ordner heisst `Meins`, und `aus_ordnername` macht daraus
    /// `Base`.
    #[test]
    fn ein_ordner_sagt_selbst_welche_eingebauten_dazugehoeren() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let meins = d.path().join("Meins");
        std::fs::create_dir(&meins).expect("Ordner");
        let agent = |ordner: &std::path::Path| crate::einstellungen::Agenteneinstellung {
            schritte: 1,
            wurzel: None,
            schreiben: true,
            kistenordner: Some(ordner.display().to_string()),
            warnung: true,
            modus: Default::default(),
        };

        // Ohne Blatt entscheidet der Name, und der ist keiner der drei.
        assert_eq!(kiste_der_gilt(&agent(&meins)), Werkzeugkiste::Base);

        std::fs::write(meins.join(KISTENDATEI), r#"{"eingebaute":"Advanced"}"#).expect("Blatt");
        assert_eq!(kiste_der_gilt(&agent(&meins)), Werkzeugkiste::Advanced);

        // Ein Tippfehler faellt auf den Namen zurueck statt ihn zu ersetzen.
        std::fs::write(meins.join(KISTENDATEI), r#"{"eingebaute":"Advanved"}"#).expect("Blatt");
        assert_eq!(kiste_der_gilt(&agent(&meins)), Werkzeugkiste::Base);
    }

    /// ⚑ **Die Kistendatei ist kein Werkzeug**, und auch keine kaputte.
    #[test]
    fn die_kistendatei_zaehlt_nicht_als_manifest() {
        let (_d, kiste) = kiste_mit(&[
            (KISTENDATEI, r#"{"eingebaute":"Advanced"}"#),
            ("gut.json", r#"{"name":"gut","beschreibung":"b","parameter":{"type":"object","properties":{},"required":[]},"befehl":"echo ok"}"#),
        ]);
        let mut warnungen = Vec::new();
        let m = manifeste_lesen(&kiste, |w| warnungen.push(w));
        assert_eq!(m.len(), 1, "die Kistendatei wurde als Werkzeug gelesen");
        assert!(warnungen.is_empty(), "sie wurde als kaputtes Werkzeug vermerkt: {warnungen:?}");
    }

    /// ⚑ **Jede mitgelieferte Kiste, die ein Blatt hat, nennt ein Wort,
    /// das es gibt.** Ein Tippfehler dort faellt still auf den Ordnernamen
    /// zurueck; hier faellt er auf.
    #[test]
    fn die_mitgelieferten_kistenblaetter_nennen_bekannte_woerter() {
        let Some(heimat) = crate::ort::wurzel().map(|w| w.join("CLIENT/werkzeugkisten")) else {
            return;
        };
        for e in std::fs::read_dir(&heimat).expect("die Kistenheimat").flatten() {
            let o = e.path();
            if o.is_dir() && o.join(KISTENDATEI).is_file() {
                assert!(
                    eingebaute_des_ordners(Some(&o)).is_some(),
                    "{} nennt kein bekanntes Wort fuer die eingebauten Werkzeuge",
                    o.display()
                );
            }
        }
    }

    /// ⚑ **Jede Kiste nennt in ihrem README, was in ihr liegt.**
    ///
    /// 📌 Die Ordner beschreiben seit dem 2026-09-17 nur noch ihren
    /// **Inhalt**; das Format steht eine Ebene hoeher, einmal. Damit ist
    /// die Liste im README aber eine zweite Stelle neben dem Ordner, und
    /// **die zweite meldet sich nicht**. Diese Probe ist die Meldung.
    #[test]
    fn jede_kiste_nennt_ihre_werkzeuge_in_ihrem_readme() {
        let Some(heimat) = crate::ort::wurzel().map(|w| w.join("CLIENT/werkzeugkisten")) else {
            return;
        };
        for e in std::fs::read_dir(&heimat).expect("die Kistenheimat").flatten() {
            let o = e.path();
            // ⚠️ `1337` ist gitignored und braucht kein README.
            if !o.is_dir() || o.file_name().is_some_and(|n| n == "1337") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(o.join("README.md")) else {
                panic!("{} hat kein README", o.display());
            };
            for m in manifeste_lesen(&o, |w| panic!("{w}")) {
                assert!(
                    text.contains(&m.name),
                    "{} nennt das Werkzeug {} nicht",
                    o.join("README.md").display(),
                    m.name
                );
            }
        }
    }

    /// ⚑ **Ein Manifest meldet sich fuer eine Art Anhang**, und die
    /// Anhangzeile nennt es dann beim Namen.
    ///
    /// ⚑ **Das ist die Tuer fuer eigene Auswerter.** Bild und Ton macht
    /// der Client seit dem 2026-09-17 selbst (`myl-senses`); wer etwas
    /// anderes lesbar machen will, eine Tabelle etwa, traegt hier `fuer`
    /// ein und wird genauso genannt.
    #[test]
    fn ein_manifest_meldet_sich_fuer_eine_art() {
        let (d, kiste) = kiste_mit(&[(
            "tabelle.json",
            r#"{"name":"tabelle_lesen","beschreibung":"b",
               "parameter":{"type":"object","properties":{"pfad":{"type":"string"}},"required":["pfad"]},
               "befehl":"echo {pfad}","fuer":["sonstiges"]}"#,
        )]);
        let mut agent = crate::einstellungen::Agenteneinstellung {
            schritte: 1,
            wurzel: None,
            schreiben: true,
            kistenordner: Some(kiste.display().to_string()),
            warnung: true,
            modus: Default::default(),
        };
        assert_eq!(werkzeug_fuer(&agent, "sonstiges").as_deref(), Some("tabelle_lesen"));
        // Fuer eine Art, fuer die sich niemand meldet, wird niemand genannt.
        assert_eq!(werkzeug_fuer(&agent, "bild"), None);

        // ⛔️ **Ohne Schreiberlaubnis steht kein Manifest im Angebot**,
        // also darf auch keines empfohlen werden.
        agent.schreiben = false;
        assert_eq!(werkzeug_fuer(&agent, "sonstiges"), None);
        drop(d);
    }

    /// ⚑ **Base und Advanced tragen die Werkzeuge, die sie versprechen.**
    ///
    /// 📌 Der Projektinhaber meldete am 2026-09-15, dass beide Ordner
    /// leer waren: Die Kisten gab es, den Inhalt nicht. **Advanced
    /// enthaelt alle Base-Werkzeuge**, und zwar durch die Kette und
    /// nicht durch Kopien, weil zwei Kopien auseinanderlaufen.
    #[test]
    fn advanced_traegt_auch_die_base_werkzeuge() {
        let Some(heimat) = crate::ort::wurzel().map(|w| w.join("CLIENT/werkzeugkisten")) else {
            return;
        };
        let ein = Einhaengung::neu(&heimat, true).expect("Einhaengung");
        let basis: Vec<String> = angebote(&heimat.join(Werkzeugkiste::Base.name()), &ein, |_| {})
            .into_iter()
            .map(|(a, _)| a.name)
            .collect();
        assert!(!basis.is_empty(), "die Kiste Base ist leer");

        // Die Kette, wie der Agent sie sieht: Base zuerst, die gewaehlte
        // zuletzt.
        let agent = crate::einstellungen::Agenteneinstellung {
            schritte: 1,
            wurzel: None,
            schreiben: true,
            kistenordner: Some(heimat.join(Werkzeugkiste::Advanced.name()).display().to_string()),
            warnung: true,
            modus: Default::default(),
        };
        let kette: Vec<String> = angebote_der_kette(&ordnerkette(&agent), &ein, |_| {})
            .into_iter()
            .map(|(a, _)| a.name)
            .collect();
        for b in &basis {
            assert!(kette.contains(b), "Advanced kennt {b} nicht: {kette:?}");
        }
        assert!(kette.len() > basis.len(), "Advanced bringt nichts Eigenes mit");
    }
}
