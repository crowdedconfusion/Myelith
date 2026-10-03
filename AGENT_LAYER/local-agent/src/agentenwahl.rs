//! **Was der oertliche Agent einstellt**: Arbeitsordner, Kiste,
//! Erlaubnisse, Betriebsart, Loop-Grenzen, dazu die Sprache.
//!
//! ⚑ **Ein Abschnitt der Einstellungen, der dem Agenten gehoert.** Die
//! Einstellungen des Clients binden ihn ein (`agent`, `loop`), die Datei
//! `client.json` bleibt dieselbe. So braucht der Agent den Client nicht,
//! um zu wissen, was er darf; der Client braucht den Agenten, um es
//! einzustellen. Die Richtung ist Absicht: Oben liegt die Oberflaeche,
//! darunter der Agent.

use serde::{Deserialize, Serialize};

/// **Die Untergrenze jedes Agentenlaufs**, hoeher als die Vorgabe fuer das
/// Gespraech.
///
/// ⚑ **4 000 seit dem 2026-09-30** (Festlegung des Projektinhabers). Gemessen
/// am selben Tag: Das 35B mit Denken brachte bei 1 600 Token einen
/// `write_file` mit einem kurzen CAD-Skript (rund 500 Byte) dreimal nicht zu
/// Ende, weil das Denken vorher den groessten Teil verbrauchte; bei 4 000
/// gelang derselbe Auftrag. Das Gespraech behaelt 1 600: Dort endet eine
/// Antwort von selbst, und es wird nichts geschrieben.
pub const AGENT_ANTWORT_MINDESTENS: usize = 4000;

/// Was der Agent darf.
///
/// 📌 **Bis zum 2026-09-11 stand hier `auch_bezeugtes`**, ein Schalter,
/// der dem Modell bezeugte Werkzeuge zusaetzlich zu den nachrechenbaren
/// oeffnete. **Er ist entfallen** (Festlegung des Projektinhabers):
/// Welche Werkzeuge ein Lauf bekommt, sagt die Werkzeugkiste, und
/// dabei soll es bleiben. Fuer einen einzelnen Vergleichslauf gibt es
/// den Schalter `--bezeugtes` an der Kommandozeile; **eine Einstellung
/// waere eine Dauerfreigabe, die niemand mehr sieht.**
///
/// ⚑ Eine Ablage aus der Zeit davor bleibt lesbar: `serde` uebergeht
/// ein Feld, das es nicht mehr gibt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agenteneinstellung {
    /// Hoechstzahl der Schritte je Auftrag.
    pub schritte: u32,
    /// Das Verzeichnis, in dem die Dateiwerkzeuge arbeiten duerfen.
    ///
    /// ⚑ **Ohne Angabe [`crate::ort::standard_wurzel`]**, also
    /// `WORK_DIR` im Repositorium, und wenn es das nicht gibt, keine
    /// Dateiwerkzeuge. Das **Arbeitsverzeichnis** waere die bequeme
    /// Wahl und die falsche: Ein Agent, der ueberall dort greifen darf,
    /// wo der Nutzer zufaellig steht, hat keine Grenze, sondern eine
    /// Gewohnheit. Ein benannter Ordner mit Beispieldateien ist etwas
    /// anderes: Er ist eine Entscheidung, und sie steht an einer Stelle.
    ///
    /// ⚠️ **Die Konsole setzt dieses Feld selbst**, auf das Verzeichnis,
    /// aus dem sie gestartet wurde. Wer `myelith` in einem Projekt
    /// aufruft, will darin arbeiten; die Vorgabe gilt fuer das Fenster,
    /// das von keinem Verzeichnis aus geoeffnet wird.
    #[serde(default)]
    pub wurzel: Option<String>,
    /// Ob die Dateiwerkzeuge auch schreiben duerfen.
    ///
    /// ⚑ **Lesen und Schreiben sind nicht dieselbe Erlaubnis.** Wer ein
    /// Verzeichnis einhaengt, damit der Agent darin nachsieht, hat
    /// damit nicht gesagt, dass er es aendern darf.
    #[serde(default)]
    pub schreiben: bool,
    /// Ein eigener Ordner, aus dem die Manifest-Werkzeuge kommen.
    ///
    /// ⚑ **Ohne Angabe entscheidet `werkzeuge`**, und der Ordner wird
    /// unter `AGENT_LAYER/local-toolkits/Base` genommen. Wer hier einen
    /// Pfad setzt, waehlt seine Kiste selbst, und ihr Name ist der des
    /// Ordners.
    ///
    /// ⚠️ **Die eingebauten Dateiwerkzeuge haengen weiter an
    /// `werkzeuge`**, nicht hier. Dieser Pfad sagt nur, wo die
    /// Manifeste liegen; `run_command` und die Einhaengegrenze bleiben
    /// an der Kiste, weil daran eine Erlaubnis haengt und nicht ein
    /// Ordnername, den jeder setzen kann.
    #[serde(default)]
    pub kistenordner: Option<String>,
    /// Ob die Warnung vor dem Agentenbetrieb noch gezeigt wird.
    ///
    /// ⚑ **Vorgabe ist `true`**, und das ist die einzige vertretbare:
    /// Wer noch nie zugestimmt hat, hat noch nicht zugestimmt. Das
    /// Haekchen im Fenster setzt sie auf `false`.
    ///
    /// ⚠️ **Sie ist keine Schranke.** Einhaengegrenze, Schreiberlaubnis
    /// und Betriebsart wirken unabhaengig davon.
    #[serde(default = "an")]
    pub warnung: bool,
    /// Ob Handlungen mit Wirkung nach aussen vorgelegt werden.
    ///
    /// ⚑ `#[serde(default)]`, damit eine Ablage aus der Zeit davor
    /// lesbar bleibt. 📌 **Bis zum 2026-09-25 hiess das `auto`**, die
    /// damalige Vorgabe; seither `manual` (siehe [`Agentenmodus`]). Eine
    /// Ablage, die `auto` ausdruecklich traegt, behaelt es: Das ist eine
    /// Wahl des Nutzers, und sie wird nicht still umgestellt.
    #[serde(default)]
    pub modus: Agentenmodus,
    /// **Ob der Agent den Bildschirm aufnehmen darf.**
    ///
    /// ⛔️ **Eine eigene Erlaubnis und nicht die Folge des
    /// Einhaengens.** Die Einhaengegrenze fasst einen Arbeitsordner
    /// ein; eine Bildschirmaufnahme entsteht ausserhalb davon. Wer einen
    /// Ordner freigibt, hat nicht gesagt, dass jemand ins Zimmer sehen
    /// darf.
    ///
    /// ⚑ `#[serde(default)]`, also **aus**, und eine Ablage aus der Zeit
    /// davor bedeutet damit dasselbe wie „nie erlaubt".
    #[serde(default)]
    pub blick_bildschirm: bool,
    /// **Ob der Agent die Kamera aufnehmen darf.** Siehe
    /// [`Agenteneinstellung::blick_bildschirm`]; die beiden sind
    /// getrennt, weil ein Bildschirm etwas anderes zeigt als ein Raum.
    #[serde(default)]
    pub blick_kamera: bool,
    /// **Ob der Chat im Web recherchieren darf.**
    ///
    /// ⛔️ **Aus, und das mit Absicht.** Dieses Häkchen tut zwei Dinge
    /// auf einmal: Es öffnet einen Weg nach draussen, und es holt
    /// fremden Text in das Fenster, in dem auch die Anhänge des Nutzers
    /// stehen. Beides zusammen ist die Lage, in der eine eingeschleuste
    /// Anweisung überhaupt erst Schaden anrichten kann. Die Schranken
    /// dagegen stehen in `netzwerkzeuge`; die Entscheidung, sie
    /// überhaupt zu brauchen, trifft der Nutzer.
    ///
    /// ⚑ **Es gilt für Chat und Agent** (Festlegung des Projektinhabers,
    /// 2026-09-26). Bis dahin nur für den Chat, mit der Begründung, im
    /// Agenten stehe mit `run_command` schon ein Weg nach draussen offen,
    /// den keine Schranke einfasst. ⚠️ **Das bleibt wahr für `Advanced`**:
    /// Ein Shell-Befehl kennt das Tor nicht. Die beiden Web-Werkzeuge selbst
    /// bleiben ummauert, und im Agenten wächst ihre Verratsprobe mit
    /// allem, was eigene Werkzeuge gelesen haben (`Tor::gesehen`).
    #[serde(default)]
    pub web_recherche: bool,
    /// **Die Saat des Zielkreises** für die Web-Werkzeuge: was der Mensch
    /// für diesen Lauf geschrieben hat, im Loop das Ziel des Tasks. Wird
    /// je Lauf gesetzt und nie gespeichert.
    #[serde(skip)]
    pub netzsaat: Option<String>,
}

impl Default for Agenteneinstellung {
    fn default() -> Self {
        Self {
            schritte: 6,
            wurzel: None,
            schreiben: false,
            kistenordner: None,
            warnung: true,
            modus: Agentenmodus::Manuell,
            blick_bildschirm: false,
            blick_kamera: false,
            web_recherche: false,
            netzsaat: None,
        }
    }
}

/// **Die Grenzen eines Loops**, also eines Vorhabens, das der Dienst in
/// Runden fuehrt (`crate::vorhaben`).
///
/// ⚑ **Grenzen im Code, nicht Bitten im Prompt.** Ein Vorhaben endet
/// spaetestens an einer dieser Zahlen, gleich was das Modell meint;
/// dieselbe Haltung wie bei `agent.schritte`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loopeinstellung {
    /// Hoechstzahl an Runden je Vorhaben.
    pub runden: u32,
    /// Hoechstdauer eines Vorhabens in Stunden. ⚑ Gezaehlt wird nur die
    /// Zeit, in der Fenster oder Konsole offen sind (Festlegung des
    /// Projektinhabers: Schliessen haelt an, Oeffnen macht weiter).
    pub stunden: u32,
    /// Werkzeugaufrufe je Runde.
    pub schritte: u32,
    /// Nach so vielen Runden ohne Fortschritt haelt das Vorhaben an und
    /// fragt nach.
    pub stillstand: u32,
    /// Nach jeder Runde ein zweiter Durchgang, der Fortschritt und Ziel
    /// prueft (Nutzereinstellung, Wunsch des Projektinhabers).
    pub pruefen: bool,
}

impl Default for Loopeinstellung {
    fn default() -> Self {
        Self { runden: 50, stunden: 24, schritte: 12, stillstand: 3, pruefen: true }
    }
}

impl Loopeinstellung {
    /// Stehen noch die Vorgaben? Dann zeigen Fenster und Konsole beim
    /// Start eines Loops einen Hinweis (Wunsch des Projektinhabers).
    pub fn ist_vorgabe(&self) -> bool {
        *self == Self::default()
    }
}

/// **Die Sprache der Oberflaeche.**
///
/// # ⚑ Warum das ein Typ ist und kein `String`
///
/// Ein `String` liesse `"deutsch"`, `"DE"`, `"de-DE"` und `"klingon"`
/// zu, und jede Stelle, die ihn liest, muesste sich selbst entscheiden,
/// was davon sie versteht. **Eine Aufzaehlung mit zwei Werten hat diese
/// Frage nicht.**
///
/// ⚠️ **Zwei Sprachen und nicht n.** Wer eine dritte hinzufuegt, fuegt
/// sie hier hinzu, und der Kompilator zeigt jede Stelle, die sie noch
/// nicht kennt. Genau das ist der Zweck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Sprache {
    /// Deutsch, die Sprache dieses Projekts.
    #[default]
    #[serde(rename = "de")]
    De,
    /// Englisch.
    #[serde(rename = "en")]
    En,
}

impl Sprache {
    /// Die Kennung, wie sie in der Ablage und im Fenster steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
        }
    }

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    ///
    /// ⚑ **Der Fehler nennt, was ginge.** „unbekannte Sprache: fr"
    /// laesst den Nutzer raten; „…, moeglich sind de, en" nicht.
    pub fn aus(k: &str) -> Result<Self, String> {
        match k {
            "de" => Ok(Self::De),
            "en" => Ok(Self::En),
            andere => Err(format!("unbekannte Sprache {andere}, moeglich sind de, en")),
        }
    }

    /// Waehlt zwischen zwei Fassungen desselben Textes.
    ///
    /// ⚑ `const` und `Copy`, damit sie auf `&'static str` in einer
    /// Konstantenzuweisung geht.
    pub const fn waehlen<T: Copy>(self, de: T, en: T) -> T {
        match self {
            Self::De => de,
            Self::En => en,
        }
    }

    /// Dasselbe fuer Werte, die sich nicht kopieren lassen.
    ///
    /// 📌 **Beide Fassungen werden gebaut, auch die ungenutzte.** Das
    /// ist der Preis dafuer, dass der Aufrufer zwei fertige Saetze
    /// hinschreiben kann statt zweier Bauanleitungen; bei einem Satz
    /// je Regler ist er nicht messbar. **Wer ihn nicht zahlen will,
    /// verzweigt selbst.**
    pub fn waehlen_wert<T>(self, de: T, en: T) -> T {
        match self {
            Self::De => de,
            Self::En => en,
        }
    }
}

/// **Ob der Agent schreiben darf, ohne zu fragen.**
///
/// ⚑ **Zwei Betriebsarten und keine dritte** (Festlegung des
/// Projektinhabers, 2026-09-11). `auto mode` laesst den Agenten
/// arbeiten; `manual mode` legt ihm jede **schreibende** Handlung
/// vorher vor.
///
/// ⚑ **Nur die schreibenden.** Ein Modus, der auch das Lesen bestaetigen
/// liesse, waere nach drei Fragen abgeschaltet, und dann bestaetigt
/// niemand mehr etwas. **Was sich nicht rueckgaengig machen laesst, ist
/// das Schreiben.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Agentenmodus {
    /// Der Agent handelt, und der Mensch sieht zu.
    #[serde(rename = "auto")]
    Auto,
    /// Jede Handlung mit Wirkung nach aussen wird vorgelegt: Schreiben,
    /// Befehle, Web-Anfragen.
    ///
    /// ⛔️ **Die Vorgabe** (Festlegung des Projektinhabers, 2026-09-25,
    /// nach dem Vorbild von Art. 14 KI-Verordnung): Wer nichts einstellt,
    /// bestaetigt jede solche Handlung. Das ist, was anderswo
    /// `require_confirmation: true` heisst.
    #[default]
    #[serde(rename = "manual")]
    Manuell,
}

impl Agentenmodus {
    /// Die Kennung, wie sie in der Ablage steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manuell => "manual",
        }
    }

    /// Wie er heisst, wo ein Mensch ihn liest.
    ///
    /// ⚑ **Die Namen kommen vom Projektinhaber** und sind Namen: Sie
    /// werden nicht uebersetzt.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto mode",
            Self::Manuell => "manual mode",
        }
    }

    /// Der jeweils andere. **Ein Schalter braucht genau das.**
    pub const fn andere(self) -> Self {
        match self {
            Self::Auto => Self::Manuell,
            Self::Manuell => Self::Auto,
        }
    }

    /// Ob eine schreibende Handlung vorgelegt werden muss.
    pub const fn fragt_nach(self) -> bool {
        matches!(self, Self::Manuell)
    }

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    pub fn aus(k: &str) -> Result<Self, String> {
        match k {
            "auto" => Ok(Self::Auto),
            "manual" | "manuell" => Ok(Self::Manuell),
            andere => Err(format!("unbekannter Modus {andere}, moeglich sind auto, manual")),
        }
    }
}

/// Welche Werkzeugkiste ein Lauf bekommt.
///
/// 📌 **Drei Werte und nicht zwei.** Ohne `Automatisch` muesste ein
/// Nutzer bei jedem Modellwechsel mitdenken, und die Einstellung waere
/// beim naechsten Wechsel still falsch. **Eine Vorgabe, die dem Modell
/// folgt, ist keine Vorgabe, sondern eine Ableitung**, und die kann
/// nicht veralten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Werkzeugwahl {
    /// Nach der Groesse des geladenen Modells.
    #[default]
    #[serde(rename = "automatisch")]
    Automatisch,
    /// Immer `Base`, auch bei einem grossen Modell.
    ///
    /// 📌 **`alias` und nicht nur `rename`.** Die Kisten hiessen bis zum
    /// 2026-09-11 `grund` und `voll`; eine Ablage aus der Zeit davor
    /// steht auf einer echten Platte. Ohne den Aliasnamen faellt sie
    /// beim Lesen auf die Vorgabe zurueck, **und zwar still.**
    #[serde(rename = "base", alias = "grund")]
    Base,
    /// Immer `Advanced`, auch bei einem kleinen Modell.
    #[serde(rename = "advanced", alias = "voll")]
    Advanced,
    /// `1337`, und die gibt es nur mit der Adminmarke.
    ///
    /// ⚠️ **Steht sie in der Ablage ohne die Marke, gilt `Advanced`**,
    /// und der Klient sagt es. Ein Wert, den niemand aendern kann und
    /// der stillschweigend etwas anderes bedeutet, ist schlimmer als
    /// eine Fehlermeldung.
    #[serde(rename = "1337")]
    Elite,
}

/// **Ob dieser Lauf die Adminmarke traegt.**
///
/// ⚑ Gesetzt wird sie in der Umgebung: `MYELITH_ADMIN=1`.
///
/// ⚠️ **Sie versteckt und schuetzt nicht, und das ist wichtig genug
/// fuer eine eigene Zeile.** Ein oertlicher Klient laeuft auf der
/// Maschine seines Nutzers, mit dessen Rechten, aus offenem Quelltext:
/// Wer die Marke setzen will, setzt sie in einer Sekunde. Sie haelt
/// eine Kiste aus der Auswahlliste heraus, damit niemand sie neben
/// `Base` und `Advanced` fuer eine dritte gleichrangige Wahl haelt.
/// **Eine Grenze, die nur bei Unkenntnis traegt, ist keine Grenze**,
/// und dieses Projekt argumentiert an jeder anderen Stelle genauso.
/// Eine echte Rolle gibt es erst, wenn es ein Netz gibt, das sie
/// bezeugen kann.
pub fn ist_admin() -> bool {
    std::env::var("MYELITH_ADMIN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("ja") || v.eq_ignore_ascii_case("yes"))
}

impl Werkzeugwahl {
    /// Die Kennung, wie sie in der Ablage und im Fenster steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::Automatisch => "automatisch",
            Self::Base => "base",
            Self::Advanced => "advanced",
            Self::Elite => "1337",
        }
    }

    /// **Welche Kiste daraus folgt, und warum.**
    ///
    /// ⚑ **Eine Stelle fuer beide Oberflaechen.** Fenster und Konsole
    /// stellen dieselbe Frage; rechneten sie sie je selbst, waeren es
    /// zwei Antworten, sobald eine von beiden angefasst wird.
    ///
    /// ⚑ **Und der Grund kommt mit.** Eine Auswahl, die still faellt,
    /// laesst den Nutzer raten, warum sein Modell ein Werkzeug nicht
    /// hat.
    pub fn aufloesen(
        self,
        artefakt: &std::path::Path,
    ) -> (crate::werkzeuge::Werkzeugkiste, String) {
        self.aufloesen_fuer(artefakt, ist_admin())
    }

    /// Dasselbe mit ausdruecklich gesagter Berechtigung.
    pub fn aufloesen_fuer(
        self,
        artefakt: &std::path::Path,
        admin: bool,
    ) -> (crate::werkzeuge::Werkzeugkiste, String) {
        use crate::werkzeuge::Werkzeugkiste;
        match self {
            Self::Automatisch => Werkzeugkiste::fuer_artefakt(artefakt),
            Self::Base => (Werkzeugkiste::Base, "so eingestellt".to_string()),
            Self::Advanced => (Werkzeugkiste::Advanced, "so eingestellt".to_string()),
            // ⚠️ Ohne die Marke wird daraus `Advanced`, und der Grund
            // sagt es: Sonst waere die Einstellung eine Behauptung.
            Self::Elite if !admin => (
                Werkzeugkiste::Advanced,
                "1337 steht ohne MYELITH_ADMIN nicht zur Verfuegung".to_string(),
            ),
            Self::Elite => (Werkzeugkiste::Elite, "so eingestellt".to_string()),
        }
    }

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    pub fn aus(k: &str) -> Result<Self, String> {
        Self::aus_fuer(k, ist_admin())
    }

    /// Dasselbe, aber mit ausdruecklich gesagter Berechtigung.
    ///
    /// ⚑ **Damit die Pruefung nicht an der Umgebung haengt.** Ein Test,
    /// der `MYELITH_ADMIN` setzt, setzt es fuer **alle** Tests im
    /// selben Prozess, und die laufen nebeneinander: Das Ergebnis
    /// haengt dann an der Reihenfolge. **Was sich uebergeben laesst,
    /// wird uebergeben.**
    pub fn aus_fuer(k: &str, admin: bool) -> Result<Self, String> {
        match k {
            "automatisch" => Ok(Self::Automatisch),
            "base" | "grund" => Ok(Self::Base),
            "advanced" | "voll" => Ok(Self::Advanced),
            // ⚑ **Die Fehlermeldung verschweigt die Kiste nicht.** Sie
            // steht im Quelltext, und ein Hinweis, der so tut, als gebe
            // es sie nicht, waere die Sorte Schutz, gegen die dieses
            // Projekt sonst argumentiert.
            "1337" if !admin => {
                Err("1337 gibt es nur mit MYELITH_ADMIN=1 in der Umgebung".to_string())
            }
            "1337" => Ok(Self::Elite),
            andere => Err(format!(
                "unbekannte Werkzeugwahl {andere}, moeglich sind {}",
                Self::moegliche_fuer(admin).join(", ")
            )),
        }
    }

    /// Was hier gesetzt werden darf, in dieser Umgebung.
    pub fn moegliche() -> Vec<&'static str> {
        Self::moegliche_fuer(ist_admin())
    }

    /// Dasselbe mit ausdruecklich gesagter Berechtigung.
    pub fn moegliche_fuer(admin: bool) -> Vec<&'static str> {
        let mut w = vec!["automatisch", "base", "advanced"];
        if admin {
            w.push("1337");
        }
        w
    }
}

/// ⚑ **Die serde-Vorgabe fuer einen Schalter, der `true` sein muss.**
/// Eine Ablage aus der Zeit vor diesem Feld traegt es nicht, und
/// `bool::default()` waere `false`, also „schon zugestimmt". Das waere
/// eine Zustimmung, die niemand gegeben hat.
const fn an() -> bool {
    true
}

/// **Die Beschriftungen der Agentenfelder**, wie die Einstellungsseite sie
/// zeigt (deutsch, englisch).
///
/// ⚑ **Eine Stelle fuer zwei Leser.** Die Feldtabelle des Clients nimmt
/// sie von hier, und der Agent nennt sie, wenn er sagt, wo sich ein
/// abgeschaltetes Werkzeug einschalten laesst. Stuende der Titel an beiden
/// Stellen, liefen sie auseinander, und der Hinweis zeigte auf ein Feld,
/// das anders heisst.
pub const TITEL_WURZEL: (&str, &str) = ("Arbeitsordner", "Working folder");
pub const TITEL_SCHREIBEN: (&str, &str) = ("Schreiben erlauben", "Allow writing");
pub const TITEL_KISTENORDNER: (&str, &str) = ("Lokale Werkzeugkiste", "Local tool box");
pub const TITEL_BLICK_BILDSCHIRM: (&str, &str) = ("Bildschirm ansehen dürfen", "May look at the screen");
pub const TITEL_BLICK_KAMERA: (&str, &str) = ("Kamera ansehen dürfen", "May look through the camera");
pub const TITEL_WEB_RECHERCHE: (&str, &str) = ("Im Web recherchieren dürfen", "May research on the web");

/// Die Felder, die [`feldtitel`] kennt.
pub const BETITELTE_FELDER: [&str; 6] = [
    "agent.wurzel",
    "agent.schreiben",
    "agent.kistenordner",
    "agent.blick_bildschirm",
    "agent.blick_kamera",
    "agent.web_recherche",
];

/// Die Beschriftung eines Agentenfeldes, oder `None` fuer ein anderes.
pub fn feldtitel(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "agent.wurzel" => Some(TITEL_WURZEL),
        "agent.schreiben" => Some(TITEL_SCHREIBEN),
        "agent.kistenordner" => Some(TITEL_KISTENORDNER),
        "agent.blick_bildschirm" => Some(TITEL_BLICK_BILDSCHIRM),
        "agent.blick_kamera" => Some(TITEL_BLICK_KAMERA),
        "agent.web_recherche" => Some(TITEL_WEB_RECHERCHE),
        _ => None,
    }
}
