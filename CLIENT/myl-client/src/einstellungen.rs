//! Was der Nutzer einstellt (CLIENT 0.4).
//!
//! # ⚑ Erst die Daten, dann die Reiter
//!
//! **Die Oberflaeche ruft dieselben Unterbefehle wie das
//! Kommandozeilenwerkzeug und hat keine eigene Logik.** Dann muss
//! zuerst feststehen, **was** eingestellt wird. Ein Reiter ist eine
//! Ansicht auf ein Feld; gibt es das Feld nicht, ist der Reiter eine
//! Zeichnung.
//!
//! Die vorgesehenen Reiter und ihre Felder hier:
//!
//! | Reiter | Feld |
//! |---|---|
//! | Kapazitaet | [`Kapazitaet`] |
//! | Modell | [`Modelleinstellung`] |
//! | Agent | [`Agenteneinstellung`] |
//! | Netz, Schluessel | ⛑ noch nicht, siehe unten |

use serde::{Deserialize, Serialize};

/// ⚑ **Was ein Knoten an Hardware hergibt, und fuer wen.**
///
/// # ⛑ Die Unterscheidung, an der dieser Punkt haengt
///
/// **Ein Schieber, der nur lokal etwas abschaltet, ist eine
/// Einstellung. Einer, der die Verguetung aendert, ist ein signiertes,
/// epochengebundenes Versprechen ans Netz**, und daraus folgen ein
/// Speicherbudget und eine Zusage als Datenstruktur. Die beiden nicht
/// zu verwechseln ist die eigentliche Arbeit.
///
/// ⚑ **Deshalb steht die Unterscheidung im Typ und nicht in einem
/// Kommentar.** [`Kapazitaet::oertlich`] begrenzt, was der eigene
/// Rechner tut. Ein Versprechen ans Netz gibt es hier **nicht**, und
/// zwar ausdruecklich: Es braucht eine Zusage als Datenstruktur, eine
/// Signatur und eine Epoche, und solange die fehlen, waere ein Feld
/// dafuer eine Behauptung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kapazitaet {
    /// Wie viele Kerne der lokale Betrieb benutzen darf. `None` heisst
    /// alle.
    pub kerne: Option<usize>,
    /// Ob Rechenwerke ausser der CPU benutzt werden duerfen.
    pub beschleuniger: bool,
    /// Obergrenze fuer den Arbeitsspeicher in Gibibyte, `None` heisst
    /// ohne Grenze.
    pub speicher_gib: Option<u32>,
    /// Obergrenze fuer die Platte in Gibibyte.
    pub platte_gib: Option<u32>,
}

impl Default for Kapazitaet {
    /// ⚑ **Die vorsichtige Vorgabe, wie bei der Betriebsart.** Wer mehr
    /// hergeben will, sagt es; wer nichts sagt, gibt nur die CPU.
    fn default() -> Self {
        Self { kerne: None, beschleuniger: false, speicher_gib: None, platte_gib: None }
    }
}

/// Welches Modell und wie es antwortet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modelleinstellung {
    /// Das Artefaktverzeichnis.
    pub artefakt: String,
    /// Hoechstzahl erzeugter Token.
    pub token: usize,
    /// ⚑ **Denkmodus, Vorgabe aus.** Ein Harness will Werkzeugaufrufe
    /// und keine Ueberlegung, und jedes Denktoken kostet dieselbe
    /// Rechenzeit wie ein Antworttoken.
    pub denken: bool,
}

impl Default for Modelleinstellung {
    fn default() -> Self {
        Self { artefakt: String::new(), token: 256, denken: false }
    }
}

/// Was der Agent darf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agenteneinstellung {
    /// Hoechstzahl der Schritte je Auftrag.
    pub schritte: u32,
    /// ⚑ **`false` heisst „nur nachrechenbar", und das ist die
    /// Vorgabe.** Dieselbe Haltung wie im Harness: Wer das Weitere
    /// will, sagt es.
    pub auch_bezeugtes: bool,
    /// Das Verzeichnis, in dem die Dateiwerkzeuge arbeiten duerfen.
    ///
    /// ⚑ **Ohne Angabe gibt es keine Dateiwerkzeuge**, und das ist die
    /// Vorgabe. Das Arbeitsverzeichnis waere die bequeme Wahl und die
    /// falsche: Ein Agent, der ueberall dort greifen darf, wo der
    /// Nutzer zufaellig steht, hat keine Grenze, sondern eine
    /// Gewohnheit.
    #[serde(default)]
    pub wurzel: Option<String>,
    /// Ob die Dateiwerkzeuge auch schreiben duerfen.
    ///
    /// ⚑ **Lesen und Schreiben sind nicht dieselbe Erlaubnis.** Wer ein
    /// Verzeichnis einhaengt, damit der Agent darin nachsieht, hat
    /// damit nicht gesagt, dass er es aendern darf.
    #[serde(default)]
    pub schreiben: bool,
}

impl Default for Agenteneinstellung {
    fn default() -> Self {
        Self { schritte: 6, auch_bezeugtes: false, wurzel: None, schreiben: false }
    }
}

/// Alles zusammen.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Einstellungen {
    pub modell: Modelleinstellung,
    pub kapazitaet: Kapazitaet,
    pub agent: Agenteneinstellung,
}

impl Einstellungen {
    /// Liest die Datei, oder liefert die Vorgaben, wenn es keine gibt.
    ///
    /// ⚑ **Eine fehlende Datei ist kein Fehler.** Beim ersten Start gibt
    /// es keine, und ein Werkzeug, das dann abbricht, ist unbrauchbar.
    /// Eine **kaputte** Datei ist dagegen sehr wohl einer: Sie stillschweigend
    /// durch Vorgaben zu ersetzen hiesse, die Einstellungen des Nutzers
    /// ohne ein Wort zu verwerfen.
    pub fn lesen(pfad: &std::path::Path) -> Result<Self, String> {
        match std::fs::read_to_string(pfad) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", pfad.display())),
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", pfad.display())),
        }
    }

    /// Schreibt die Datei, Verzeichnis inbegriffen.
    pub fn schreiben(&self, pfad: &std::path::Path) -> Result<(), String> {
        if let Some(v) = pfad.parent() {
            std::fs::create_dir_all(v).map_err(|e| format!("{}: {e}", v.display()))?;
        }
        // ⚑ **Mit Zeilenumbruch am Ende.** Diese Datei macht ein Mensch
        // auf und bearbeitet sie; eine Datei ohne letzten Umbruch
        // klebt in jeder Ausgabe an der naechsten Zeile und laesst
        // `diff` eine Aenderung melden, die keine ist.
        let t = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(pfad, t + "\n").map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Wo die Datei ueblicherweise liegt.
    ///
    /// ⛑ **Hier stand nur `HOME`, und auf Windows gibt es das meist
    /// nicht** (dort heisst die Variable `USERPROFILE`; `HOME` setzen
    /// nur Git Bash und aehnliche Umgebungen). Der Rueckfall war `"."`,
    /// also landeten die Einstellungen im **Arbeitsverzeichnis**: Wer
    /// `myl` aus zwei Ordnern startete, hatte zwei Einstellungsdateien
    /// und keinen Hinweis darauf. Aufgefallen beim Durchsehen des
    /// Clients auf Plattformtauglichkeit (CLIENT 1.6), nicht durch eine
    /// Pruefung, und ⚑ **eine Pruefung in der CI haette es auch nicht
    /// gefunden**: GitHub Actions setzt `HOME` auch auf
    /// `windows-latest`. Das ist der Grund, warum diese Sorte Fehler
    /// gelesen und nicht gemessen werden muss.
    ///
    /// Jetzt je Plattform die Stelle, die dort ueblich ist:
    ///
    /// - Windows: `%APPDATA%\Myelith\client.json`
    /// - sonst: `$XDG_CONFIG_HOME/myelith/client.json`, ersatzweise
    ///   `$HOME/.config/myelith/client.json`
    ///
    /// ⚑ `XDG_CONFIG_HOME` steht dabei nicht aus Vollstaendigkeit da: Es
    /// ist die Variable, mit der eine deklarative Verteilung wie NixOS
    /// die Ablage ihrer Nutzer festlegt, und dieses Projekt nennt NixOS
    /// ausdruecklich als Ziel.
    pub fn vorgabepfad() -> std::path::PathBuf {
        // ⛑ **Wer schon eine Datei hat, behaelt sie.** Ohne diese drei
        // Zeilen zoege die Datei bei jedem, der `XDG_CONFIG_HOME` setzt,
        // an einen neuen Ort um, und seine Einstellungen waeren
        // stillschweigend weg: `lesen` faende nichts und legte die
        // Vorgaben an. Das faellt erst auf, wenn jemand sein Artefakt
        // sucht. Deshalb gewinnt eine **vorhandene** Datei vor der
        // bevorzugten Stelle, und das gilt in beide Richtungen: Wer
        // schon an der neuen Stelle liegt, wird nicht an die alte
        // zurueckgeschickt.
        for k in Self::kandidaten_liste() {
            if k.is_file() {
                return k;
            }
        }
        Self::kandidaten_liste()
            .into_iter()
            .next()
            .expect("die Liste endet immer mit dem Arbeitsverzeichnis")
    }

    /// Die Orte, an denen die Einstellungsdatei liegen kann, in der
    /// Reihenfolge, in der sie bevorzugt werden.
    ///
    /// ⚑ **Eigene Funktion, damit `vorgabepfad` und die Wanderung
    /// dieselbe Liste benutzen.** Zwei Listen liefen auseinander, und
    /// dann fande die eine, was die andere schriebe.
    fn kandidaten_liste() -> Vec<std::path::PathBuf> {
        use std::path::PathBuf;
        let mut orte: Vec<PathBuf> = Vec::new();

        #[cfg(windows)]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                if !appdata.is_empty() {
                    orte.push(PathBuf::from(appdata).join("Myelith").join("client.json"));
                }
            }
            if let Some(profil) = std::env::var_os("USERPROFILE") {
                if !profil.is_empty() {
                    orte.push(
                        PathBuf::from(profil)
                            .join("AppData")
                            .join("Roaming")
                            .join("Myelith")
                            .join("client.json"),
                    );
                }
            }
        }

        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            // ⚑ Die Spezifikation sagt: ein relativer Wert ist ungueltig
            // und zu ignorieren. Ohne diese Zeile fuehrte ein
            // versehentliches `XDG_CONFIG_HOME=.config` zurueck in genau
            // den Fehler, der oben behoben wird.
            let p = PathBuf::from(xdg);
            if p.is_absolute() {
                orte.push(p.join("myelith").join("client.json"));
            }
        }
        if let Some(heim) = std::env::var_os("HOME") {
            if !heim.is_empty() {
                orte.push(PathBuf::from(heim).join(".config/myelith/client.json"));
            }
        }
        // Bleibt nur das Arbeitsverzeichnis. Das ist ein schlechter Ort,
        // und deshalb steht er hier als letzter und nicht als erster.
        orte.push(PathBuf::from(".config/myelith/client.json"));
        orte
    }
}

/// Die Felder, die sich setzen lassen, mit ihrer Art.
///
/// ⚑ **Damit eine Oberflaeche sie nicht raten muss.** Wer eine
/// Einstellungsseite baut, braucht drei Dinge je Feld: den Namen, was
/// dort hineingehoert, und ob es abschaltbar ist. Ohne diese Liste
/// entstuende sie ein zweites Mal im Fenster, und dann laufen zwei
/// Listen auseinander.
pub const FELDER: [(&str, Feldart); 11] = [
    ("modell.artefakt", Feldart::Text),
    ("modell.token", Feldart::Zahl),
    ("modell.denken", Feldart::Schalter),
    ("agent.schritte", Feldart::Zahl),
    ("agent.bezeugtes", Feldart::Schalter),
    ("agent.wurzel", Feldart::Pfad),
    ("agent.schreiben", Feldart::Schalter),
    ("kap.kerne", Feldart::Grenze),
    ("kap.beschleuniger", Feldart::Schalter),
    ("kap.speicher", Feldart::Grenze),
    ("kap.platte", Feldart::Grenze),
];

/// Was in ein Feld hineingehoert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Feldart {
    /// Freier Text.
    Text,
    /// Eine Zahl, die immer einen Wert hat.
    Zahl,
    /// An oder aus.
    Schalter,
    /// ⚑ Eine Zahl **oder** `aus`. Der Unterschied zu [`Feldart::Zahl`]
    /// ist der ganze Punkt: `aus` **loescht** die Grenze, es setzt sie
    /// nicht auf null. Null Kerne waeren keine Einstellung, sondern ein
    /// Stillstand.
    Grenze,
    /// Ein Pfad **oder** `aus`, das ihn wegnimmt. Ein leerer Pfad waere
    /// das Wurzelverzeichnis.
    Pfad,
}

/// Was `an` bedeutet.
fn ja(w: &str) -> bool {
    matches!(w, "an" | "ja" | "true" | "1")
}

/// ⚑ `aus` loescht eine Grenze, statt sie auf null zu setzen.
fn opt(w: &str) -> Option<u32> {
    w.parse().ok().filter(|_| w != "aus")
}

impl Einstellungen {
    /// Setzt ein Feld aus seinem Namen und einem Text.
    ///
    /// # ⚑ Warum das hier steht und nicht im Bedieninstrument
    ///
    /// Diese Funktion kennt Feinheiten, die keine zweite Stelle
    /// nachbauen darf: dass `aus` eine Grenze **loescht**, dass `an`,
    /// `ja`, `true` und `1` dasselbe heissen, und welche Felder es
    /// ueberhaupt gibt. **Zwei Stellen, die das wissen, laufen
    /// auseinander**, und die zweite ist immer die schlechter
    /// geprueete.
    pub fn setzen(&mut self, feld: &str, wert: &str) -> Result<(), String> {
        match feld {
            "modell.artefakt" => self.modell.artefakt = wert.to_string(),
            "modell.token" => {
                self.modell.token = wert.parse().map_err(|_| format!("{wert} ist keine Zahl"))?
            }
            "modell.denken" => self.modell.denken = ja(wert),
            "agent.schritte" => {
                self.agent.schritte = wert.parse().map_err(|_| format!("{wert} ist keine Zahl"))?
            }
            "agent.bezeugtes" => self.agent.auch_bezeugtes = ja(wert),
            "agent.wurzel" => self.agent.wurzel = (wert != "aus").then(|| wert.to_string()),
            "agent.schreiben" => self.agent.schreiben = ja(wert),
            "kap.kerne" => self.kapazitaet.kerne = opt(wert).map(|v| v as usize),
            "kap.beschleuniger" => self.kapazitaet.beschleuniger = ja(wert),
            "kap.speicher" => self.kapazitaet.speicher_gib = opt(wert),
            "kap.platte" => self.kapazitaet.platte_gib = opt(wert),
            andere => return Err(format!("unbekanntes Feld {andere}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod setzer {
    use super::*;

    #[test]
    fn ein_unbekanntes_feld_wird_abgelehnt() {
        let mut e = Einstellungen::default();
        assert!(e.setzen("gibt.es.nicht", "x").is_err());
    }

    /// ⚑ **`aus` loescht, es setzt nicht auf null.** Der Unterschied
    /// ist der Grund, warum es `Feldart::Grenze` gibt.
    #[test]
    fn aus_loescht_eine_grenze() {
        let mut e = Einstellungen::default();
        e.setzen("kap.kerne", "8").expect("setzen");
        assert_eq!(e.kapazitaet.kerne, Some(8));
        e.setzen("kap.kerne", "aus").expect("setzen");
        assert_eq!(e.kapazitaet.kerne, None, "aus hat auf null gesetzt statt zu loeschen");
        e.setzen("kap.kerne", "0").expect("setzen");
        assert_eq!(e.kapazitaet.kerne, Some(0), "null ist ein Wert und nicht aus");
    }

    #[test]
    fn aus_nimmt_auch_die_einhaengung_weg() {
        let mut e = Einstellungen::default();
        e.setzen("agent.wurzel", "/tmp/x").expect("setzen");
        assert_eq!(e.agent.wurzel.as_deref(), Some("/tmp/x"));
        e.setzen("agent.wurzel", "aus").expect("setzen");
        assert_eq!(e.agent.wurzel, None);
    }

    #[test]
    fn die_vier_wahrheitswoerter_gelten() {
        let mut e = Einstellungen::default();
        for w in ["an", "ja", "true", "1"] {
            e.setzen("modell.denken", "nein").expect("setzen");
            e.setzen("modell.denken", w).expect("setzen");
            assert!(e.modell.denken, "{w} gilt nicht als wahr");
        }
        for w in ["aus", "nein", "false", "0", "vielleicht"] {
            e.setzen("modell.denken", "an").expect("setzen");
            e.setzen("modell.denken", w).expect("setzen");
            assert!(!e.modell.denken, "{w} gilt als wahr");
        }
    }

    #[test]
    fn keine_zahl_ist_ein_fehler() {
        let mut e = Einstellungen::default();
        assert!(e.setzen("modell.token", "viele").is_err());
        assert!(e.setzen("agent.schritte", "").is_err());
    }

    /// ⚑ **Die Liste und der Setzer muessen dieselben Felder kennen.**
    /// Ein Feld in `FELDER`, das der Setzer ablehnt, waere eine Zeile
    /// in der Oberflaeche, die nichts tut.
    #[test]
    fn jedes_gelistete_feld_laesst_sich_setzen() {
        for (name, art) in FELDER {
            let mut e = Einstellungen::default();
            let wert = match art {
                Feldart::Text | Feldart::Pfad => "irgendwas",
                Feldart::Zahl | Feldart::Grenze => "3",
                Feldart::Schalter => "an",
            };
            e.setzen(name, wert)
                .unwrap_or_else(|f| panic!("{name} steht in FELDER, der Setzer sagt: {f}"));
        }
    }

    /// Und die Gegenrichtung: nichts Gesetztes fehlt in der Liste.
    #[test]
    fn die_liste_ist_vollstaendig() {
        let mut e = Einstellungen::default();
        // Ein Feld, das der Setzer kennt, aber die Liste nicht, waere
        // in keiner Oberflaeche erreichbar.
        for name in ["modell.artefakt", "modell.token", "modell.denken",
                     "agent.schritte", "agent.bezeugtes", "agent.wurzel",
                     "agent.schreiben", "kap.kerne", "kap.beschleuniger",
                     "kap.speicher", "kap.platte"] {
            assert!(
                FELDER.iter().any(|(n, _)| *n == name),
                "{name} laesst sich setzen, steht aber nicht in FELDER"
            );
            let _ = e.setzen(name, "an");
        }
    }

    /// ⛑ **Der Ort der Einstellungsdatei, plattformweise.**
    ///
    /// Vorher stand dort nur `HOME` mit Rueckfall auf `"."`. Auf Windows
    /// ist `HOME` meist nicht gesetzt, also landete die Datei im
    /// Arbeitsverzeichnis, und `myl` aus zwei Ordnern gestartet las zwei
    /// verschiedene Einstellungen.
    ///
    /// ⚑ **Alles in einer Pruefung**, weil Umgebungsvariablen prozessweit
    /// sind: Zwei nebenlaeufige Pruefungen setzten sich gegenseitig den
    /// Zustand um, und das Ergebnis hinge an der Reihenfolge. Aus
    /// demselben Grund wird am Ende wiederhergestellt, was vorgefunden
    /// wurde.
    #[test]
    fn der_ort_der_einstellungen_folgt_der_plattform() {
        let alt_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let alt_heim = std::env::var_os("HOME");

        // ⚑ Die Variable, ueber die eine deklarative Verteilung wie
        // NixOS die Ablage festlegt. Sie hat Vorrang vor `HOME`.
        std::env::set_var("XDG_CONFIG_HOME", "/x/konfig");
        std::env::set_var("HOME", "/heim/jemand");
        let p = Einstellungen::vorgabepfad();
        if cfg!(windows) {
            // Auf Windows entscheidet `APPDATA`, und diese Pruefung
            // sagt dort nichts ueber XDG aus.
            assert!(p.is_absolute(), "auch auf Windows ein absoluter Pfad: {p:?}");
        } else {
            assert_eq!(p, std::path::PathBuf::from("/x/konfig/myelith/client.json"));
        }

        // Ein relativer Wert ist laut Spezifikation ungueltig und wird
        // uebergangen; sonst faende man sich genau in dem Fehler wieder,
        // den diese Aenderung behebt.
        std::env::set_var("XDG_CONFIG_HOME", ".konfig");
        let p = Einstellungen::vorgabepfad();
        assert!(
            !p.starts_with(".konfig"),
            "ein relatives XDG_CONFIG_HOME wird uebergangen, bekommen: {p:?}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        let p = Einstellungen::vorgabepfad();
        if !cfg!(windows) {
            assert_eq!(p, std::path::PathBuf::from("/heim/jemand/.config/myelith/client.json"));
        }

        // ⛑ Der Fall, der die ganze Aenderung ausgeloest hat: keine der
        // Heimatvariablen gesetzt. Frueher war das Ergebnis
        // `./.config/...`, also je Arbeitsverzeichnis ein anderes.
        std::env::remove_var("HOME");
        let p = Einstellungen::vorgabepfad();
        if !cfg!(windows) {
            assert_eq!(
                p,
                std::path::PathBuf::from(".config/myelith/client.json"),
                "ohne HOME bleibt nur das Arbeitsverzeichnis, und das steht als letzte Wahl da"
            );
        }

        // ⛑ **Eine vorhandene Datei gewinnt vor der bevorzugten
        // Stelle.** Ohne das zoege die Datei bei jedem, der
        // `XDG_CONFIG_HOME` setzt, an einen neuen Ort um, und seine
        // Einstellungen waeren stillschweigend weg. Hier steht `HOME`
        // auf einem echten Verzeichnis mit einer echten Datei, und
        // `XDG_CONFIG_HOME` auf einem leeren.
        if !cfg!(windows) {
            let heim = tempfile::tempdir().expect("Heimatverzeichnis");
            let xdg = tempfile::tempdir().expect("XDG-Verzeichnis");
            let alt = heim.path().join(".config/myelith");
            std::fs::create_dir_all(&alt).expect("altes Verzeichnis");
            std::fs::write(alt.join("client.json"), "{}\n").expect("alte Datei");

            std::env::set_var("HOME", heim.path());
            std::env::set_var("XDG_CONFIG_HOME", xdg.path());
            assert_eq!(
                Einstellungen::vorgabepfad(),
                alt.join("client.json"),
                "wer schon eine Datei hat, behaelt sie"
            );

            // Und wenn an beiden Stellen eine liegt, gewinnt die
            // bevorzugte: Wer einmal umgezogen ist, bleibt umgezogen.
            let neu_dir = xdg.path().join("myelith");
            std::fs::create_dir_all(&neu_dir).expect("neues Verzeichnis");
            std::fs::write(neu_dir.join("client.json"), "{}\n").expect("neue Datei");
            assert_eq!(
                Einstellungen::vorgabepfad(),
                neu_dir.join("client.json"),
                "liegt an beiden Stellen eine, gewinnt die bevorzugte"
            );
        }

        match alt_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match alt_heim {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

}

