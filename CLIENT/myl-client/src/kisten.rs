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

use crate::werkzeuge::{befehl_im_verzeichnis, Einhaengung, Werkzeugkiste};

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
    pub befehl: String,
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
            let ausf: Box<dyn Werkzeugausfuehrung> =
                Box::new(ManifestWerkzeug { manifest: m, einhaengung: ein.clone() });
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
    Werkzeugkiste::aus_ordnername(&ordnername(ordner.as_deref(), Werkzeugkiste::Base.name()))
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
        befehl_im_verzeichnis(&self.einhaengung, &befehl)
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
