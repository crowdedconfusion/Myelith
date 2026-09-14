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
//! | Netz, Schluessel | 📌 noch nicht, siehe unten |

use serde::{Deserialize, Serialize};

/// ⚑ **Was ein Knoten an Hardware hergibt, und fuer wen.**
///
/// # 📌 Die Unterscheidung, an der dieser Punkt haengt
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
    /// ⚑ **Was von jedem Rechenwerk freigegeben ist, in Prozent**,
    /// unter der Kennung aus dem Hardwarescan.
    ///
    /// 📌 **Hier stand bis zum 2026-09-10 ein einzelnes
    /// `beschleuniger: bool`.** Es beantwortete die Frage „darf er
    /// ueberhaupt" fuer **alle** Rechenwerke zugleich, und ein Rechner
    /// mit zwei Karten konnte damit nicht sagen, dass er die eine
    /// hergibt und die andere fuer sich behaelt. Der Schalter ist
    /// entfallen und nicht ergaenzt: Eine Freigabe ueber null **ist**
    /// die Erlaubnis, und zwei Quellen fuer dieselbe Frage laufen
    /// auseinander.
    ///
    /// ⚑ **Prozent und nicht Gibibyte**, und das ist gemessen: Ein
    /// eingebautes Rechenwerk teilt sich den Speicher mit der CPU und
    /// hat gar kein eigenes, ein eigenstaendiges nennt sein VRAM. Eine
    /// Einheit, die nur auf der einen Sorte einen Sinn hat, waere fuer
    /// die andere erfunden. Was der Anteil in Bytes bedeutet, zeigt die
    /// Oberflaeche daneben, wo sie es weiss.
    #[serde(default)]
    pub rechenwerke: std::collections::BTreeMap<String, u8>,
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
        Self { kerne: None, rechenwerke: Default::default(), speicher_gib: None, platte_gib: None }
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
    /// Welche Werkzeugkiste dem Modell angeboten wird.
    ///
    /// ⚑ **Vorgabe ist `Automatisch`, und das Modell sagt seine Groesse
    /// selbst.** Wer einem kleinen Modell trotzdem alles geben will,
    /// stellt hier `Voll`; wer einem grossen weniger geben will,
    /// `Grund`. **Die Einstellung schlaegt die Ableitung**, denn wer
    /// sie anfasst, hat die Frage schon beantwortet.
    #[serde(default)]
    pub werkzeuge: Werkzeugwahl,
    /// Ob schreibende Handlungen vorgelegt werden.
    ///
    /// ⚑ `#[serde(default)]`, damit eine Ablage aus der Zeit davor
    /// lesbar bleibt und dann `auto` bedeutet: **die Vorgabe, die es
    /// vorher auch war.**
    #[serde(default)]
    pub modus: Agentenmodus,
}

impl Default for Agenteneinstellung {
    fn default() -> Self {
        Self {
            schritte: 6,
            wurzel: None,
            schreiben: false,
            werkzeuge: Werkzeugwahl::Automatisch,
            modus: Agentenmodus::Auto,
        }
    }
}

/// Alles zusammen.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Einstellungen {
    pub modell: Modelleinstellung,
    pub kapazitaet: Kapazitaet,
    pub agent: Agenteneinstellung,
    #[serde(default)]
    pub ausgabe: Ausgabeeinstellung,
    #[serde(default)]
    pub oberflaeche: Oberflaecheneinstellung,
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

/// **Wie die Konsole aussieht.**
///
/// ⚑ **Nur die Konsole** (Festlegung des Projektinhabers, 2026-09-11).
/// Das Fenster hat sein eigenes Stilblatt; ein Design, das beide
/// beschriebe, waere an einer der beiden Stellen immer falsch.
///
/// ⚑ **`Standard` ist kein Design, sondern die Abwesenheit eines.** Es
/// setzt keine eigenen Farben und nimmt die des Terminals: **Wer sein
/// Farbschema eingestellt hat, hat damit schon gewaehlt**, und ein
/// Programm, das sich darueberlegt, nimmt ihm die Wahl wieder weg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Konsolendesign {
    /// Die Farben des Terminals, unveraendert.
    #[default]
    #[serde(rename = "standard")]
    Standard,
    /// Graue Kanten, Neon-Akzent: das Bild, das dieses Programm seit
    /// jeher zeigt.
    #[serde(rename = "myelith")]
    Myelith,
    /// Bernstein, einfarbig, wie ein alter Schirm.
    #[serde(rename = "bernstein")]
    Bernstein,
    /// Dunkles Blau mit Cyan.
    #[serde(rename = "tiefsee")]
    Tiefsee,
    /// Grau und Weiss, sonst nichts.
    #[serde(rename = "tinte")]
    Tinte,
}

impl Konsolendesign {
    /// Die Kennung, wie sie in der Ablage steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Myelith => "myelith",
            Self::Bernstein => "bernstein",
            Self::Tiefsee => "tiefsee",
            Self::Tinte => "tinte",
        }
    }

    /// Wie es heisst, wo ein Mensch es liest.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Myelith => "Myelith",
            Self::Bernstein => "Bernstein",
            Self::Tiefsee => "Tiefsee",
            Self::Tinte => "Tinte",
        }
    }

    /// Ein Satz dazu, fuer die Auswahl beim Start.
    pub const fn satz(self) -> &'static str {
        match self {
            Self::Standard => "die Farben deines Terminals, unveraendert",
            Self::Myelith => "graue Kanten, Neon-Akzent: das bisherige Bild",
            Self::Bernstein => "einfarbig bernstein, wie ein alter Schirm",
            Self::Tiefsee => "dunkles Blau mit Cyan",
            Self::Tinte => "grau und weiss, sonst nichts",
        }
    }

    /// Ob der Ladetext durch den Regenbogen wandert.
    ///
    /// ⚑ **In den einfarbigen Designs nicht.** Ein Regenbogen in einem
    /// Bild, das aus einer Farbe besteht, ist kein Akzent, sondern ein
    /// Fremdkoerper; dort pulst der Ladetext stattdessen in der Helle
    /// dieser einen Farbe, im selben Takt.
    pub const fn regenbogen(self) -> bool {
        matches!(self, Self::Standard | Self::Myelith | Self::Tiefsee)
    }

    /// Alle, in der Reihenfolge der Auswahl.
    pub const ALLE: [Self; 5] =
        [Self::Standard, Self::Myelith, Self::Bernstein, Self::Tiefsee, Self::Tinte];

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    pub fn aus(k: &str) -> Result<Self, String> {
        Self::ALLE.into_iter().find(|d| d.kennung() == k).ok_or_else(|| {
            format!(
                "unbekanntes Design {k}, moeglich sind {}",
                Self::ALLE.map(|d| d.kennung()).join(", ")
            )
        })
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
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Jede schreibende Handlung wird vorgelegt.
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

/// Wie sich die Oberflaeche verhaelt.
///
/// 📌 `#[serde(default)]` an beiden Stellen, aus demselben Grund wie bei
/// [`Ausgabeeinstellung`]: Eine Ablage aus der Zeit davor bleibt lesbar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Oberflaecheneinstellung {
    /// Wie die Konsole aussieht. ⚑ Das Fenster beruehrt es nicht.
    #[serde(default)]
    pub design: Konsolendesign,
    /// Die Sprache, in der das Fenster spricht.
    #[serde(default)]
    pub sprache: Sprache,
}

/// Wohin ausgegebene Gespraeche geschrieben werden.
///
/// ⚑ **Ohne Angabe wird nichts geschrieben**, und das ist die Vorgabe.
/// Ein Voreinstellungsordner waere die bequeme Wahl und die falsche:
/// Wer ein Gespraech ausgibt, will wissen wohin, und ein Ort, den
/// niemand gewaehlt hat, ist ein Ort, an dem niemand sucht. Das Fenster
/// fuehrt stattdessen zur Einstellung.
///
/// 📌 `#[serde(default)]` an beiden Stellen, damit eine Ablage aus der
/// Zeit davor weiter lesbar bleibt. Ohne das waere jede bestehende
/// Datei mit einem Schlag kaputt, und `lesen` lehnt eine kaputte Datei
/// zu Recht ab.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Ausgabeeinstellung {
    /// Das Verzeichnis fuer ausgegebene Gespraeche.
    #[serde(default)]
    pub ordner: Option<String>,
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
            Ok(t) => {
                let mut e: Self =
                    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", pfad.display()))?;
                e.wandern();
                Ok(e)
            }
        }
    }

    /// Zieht eine Ablage aus einer frueheren Fassung nach.
    ///
    /// # 📌 Warum es das gibt (2026-09-10)
    ///
    /// **Die Artefakte heissen seit heute nach dem Modell, das sie
    /// sind**, also `myelith-4b` statt `qwen3-4b`. Jede bestehende
    /// Ablage zeigt aber noch auf den alten Pfad, und der loest nach
    /// dem Umbenennen ins Leere auf: Der Klient meldete „Modell laedt
    /// nicht" und der Nutzer suchte den Fehler bei sich.
    ///
    /// ⚑ **Eine Umbenennung ist erst fertig, wenn das Mitgewanderte
    /// mitgewandert ist.** Wer nur die Verzeichnisse umbenennt, hat die
    /// Arbeit auf jeden verschoben, der eine Einstellung gesetzt hat.
    ///
    /// 📌 **Und sie greift nur am Verzeichnisnamen, nicht am ganzen
    /// Pfad.** Ein Nutzer, der seine Artefakte woanders haelt, behaelt
    /// seinen Ort; getauscht wird der letzte Bestandteil und nur, wenn
    /// er einer der vier alten Namen ist.
    fn wandern(&mut self) {
        // 📌 **Drei Wanderungen, und sie sind von zweierlei Art.** Die
        // erste (2026-09-10) war eine reine Umbenennung: dasselbe
        // Modell, neuer Verzeichnisname. Die zweite (2026-09-11) und
        // die dritte (2026-09-12) sind **Austausche**: erst gingen
        // Qwen2.5-0,5B und Qwen2.5-7B, dann das dichte Qwen3-14B.
        //
        // ⚠️ **Das ist kein Ersatz, sondern die naechstgelegene Wahl.**
        // Wer auf `myelith-7b` oder `myelith-14b` zeigte, bekommt
        // `myelith-4b`, und das ist ein anderes Modell mit anderen
        // Antworten. Die Alternative waere ein Pfad, der ins Leere
        // zeigt, und dann meldete der Klient „Modell laedt nicht" und
        // der Nutzer suchte den Fehler bei sich. **Beides ist unschoen;
        // ein Modell, das antwortet, ist das kleinere Uebel.**
        //
        // ⚑ **Warum das 4B und nicht das Gemisch.** Es ist das
        // groesste verbliebene **dichte** Modell und damit das
        // naechstgelegene; und es verlangt 5 GB Platte statt 32. Wer
        // von einem 7B kam, hat nicht zwangslaeufig Platz fuer ein
        // 30B.
        const ALT_NEU: [(&str, &str); 7] = [
            ("qwen2.5-0.5b", "myelith-0.6b"),
            ("qwen2.5-7b", "myelith-4b"),
            ("myelith-0.5b", "myelith-0.6b"),
            ("myelith-7b", "myelith-4b"),
            // ⛔️ Entfernt am 2026-09-12 (Festlegung des
            // Projektinhabers): schlechter als das Gemisch in
            // Durchsatz und Perplexitaet, aufwendiger zu trainieren,
            // und 46 GB auf der Platte.
            ("myelith-14b", "myelith-4b"),
            ("qwen3-30b-a3b", "myelith-30b-a3b"),
            ("qwen3-4b", "myelith-4b"),
        ];
        let pfad = self.modell.artefakt.trim_end_matches('/');
        let Some((vorne, letztes)) = pfad.rsplit_once('/') else { return };
        for (alt, neu) in ALT_NEU {
            if letztes == alt {
                self.modell.artefakt = format!("{vorne}/{neu}");
                return;
            }
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
    /// 📌 **Hier stand nur `HOME`, und auf Windows gibt es das meist
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
        // 📌 **Wer schon eine Datei hat, behaelt sie.** Ohne diese drei
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

/// Ein setzbares Feld, so wie es in einer Oberflaeche erscheint.
///
/// ⚑ **Damit eine Oberflaeche es weder raten noch benennen muss.** Wer
/// eine Einstellungsseite baut, braucht fuenf Dinge je Feld: den
/// technischen Namen zum Setzen, was dort hineingehoert, den Bereich,
/// unter dem es steht, eine Beschriftung in ganzen Worten und einen
/// Satz dazu, was es bewirkt.
///
/// 📌 **Die Beschriftung steht hier und nicht im Fenster.** Bis zum
/// 2026-09-09 zeigte die Einstellungsseite den technischen Namen:
/// `kap.beschleuniger`, `agent.bezeugtes`, `modell.artefakt`. Das ist
/// kein Deutsch, sondern eine Kennung, und wer sie nicht geschrieben
/// hat, muss raten, was sie tut. Eine Uebersetzungstabelle im Fenster
/// waere der bequeme Ausweg gewesen und der falsche: Sie laeuft
/// auseinander, sobald hier ein Feld dazukommt, und dann traegt eine
/// Zeile die Beschriftung einer anderen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Feld {
    /// Der Name, unter dem [`Einstellungen::setzen`] das Feld kennt.
    pub name: &'static str,
    /// Was hineingehoert.
    pub art: Feldart,
    /// Die Ueberschrift, unter der das Feld auf der Seite steht.
    pub bereich: &'static str,
    /// Die Beschriftung der Zeile.
    pub titel: &'static str,
    /// Dieselbe Ueberschrift auf Englisch.
    ///
    /// ⚑ **Sie geht nicht ueber die Naht.** `#[serde(skip)]`, denn das
    /// Fenster soll nicht zwischen zwei Feldern waehlen muessen:
    /// [`Feld::in_sprache`] entscheidet **hier**, und was ankommt, ist
    /// fertig. Zwei Beschriftungen im Fenster waeren zwei Stellen, an
    /// denen die naechste Sprache vergessen werden kann.
    #[serde(skip)]
    pub bereich_en: &'static str,
    /// Dieselbe Beschriftung auf Englisch.
    #[serde(skip)]
    pub titel_en: &'static str,
    /// Derselbe Satz auf Englisch.
    #[serde(skip)]
    pub hinweis_en: &'static str,
    /// Die moeglichen Werte, wenn es eine Auswahl ist, sonst leer.
    ///
    /// ⚑ **Die Namen der Sprachen stehen in ihrer eigenen Sprache**,
    /// „Deutsch" und „English", und werden deshalb nicht uebersetzt.
    /// Wer die Oberflaeche gerade nicht versteht, findet seine Sprache
    /// nur so wieder.
    #[serde(serialize_with = "nur_erlaubte")]
    pub wahl: &'static [Wahl],
    /// Ob in diesem Feld ein Verzeichnis steht.
    ///
    /// ⚑ **Abgeleitet aus der Art und nicht in der Tabelle getippt**,
    /// siehe [`Feldart::ist_ordner`]. Ein von Hand gesetztes Merkmal
    /// waere ein zwoelfmal wiederholtes `true`/`false`, und eines davon
    /// waere irgendwann falsch.
    pub ordner: bool,
    /// Ob dieses Feld nur den Konsolenclient betrifft.
    ///
    /// ⚑ **Dann zeigt das Fenster es nicht** (Festlegung des
    /// Projektinhabers, 2026-09-11). Eine Einstellung, die an der
    /// Stelle, an der sie steht, **nichts** bewirkt, ist schlimmer als
    /// eine fehlende: Wer sie umlegt und nichts sieht, sucht den Fehler
    /// woanders.
    ///
    /// ⚠️ **Sie verschwindet nicht aus der Kiste**, nur aus dem
    /// Fenster: `myl setzen` und `/settings` in der Konsole kennen sie
    /// weiter, denn dort wirkt sie.
    pub nur_konsole: bool,
    /// Ob dieses Feld ein Betriebsmittel der Maschine freigibt und
    /// deshalb als Schieberegler gehoert.
    ///
    /// ⚑ **Ebenfalls abgeleitet**, siehe [`Feldart::ist_freigabe`].
    pub freigabe: bool,
    /// Ein Satz dazu, was das Feld bewirkt und was ohne Angabe gilt.
    ///
    /// ⚑ **Er nennt die Vorgabe, wo es eine gibt.** „Ohne Angabe gibt
    /// es keine Dateiwerkzeuge" ist die Auskunft, die jemand beim
    /// Einstellen braucht; sie steht sonst nur im Quelltext.
    ///
    /// ⚠️ **Und er sagt es, wenn ein Feld nichts bewirkt.** Von den
    /// vier Grenzen dieses Rechners wird genau eine angewendet,
    /// `kap.kerne`; `beschleuniger`, `speicher` und `platte` werden
    /// gespeichert und angezeigt, und danach liest sie niemand. Ein
    /// Schieber, der aussieht wie eine Grenze und keine ist, ist eine
    /// Behauptung. Solange die Felder dastehen, steht der Satz dabei.
    pub hinweis: &'static str,
}

/// Ein moeglicher Wert eines Auswahlfeldes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Wahl {
    /// Was gesetzt wird.
    pub wert: &'static str,
    /// Was dabeisteht.
    pub titel: &'static str,
    /// Ob dieser Wert nur mit der Adminmarke angeboten wird.
    ///
    /// ⚑ **Am Wert und nicht am Feld.** Ein Feld, das als Ganzes
    /// verborgen waere, verschwaende die Zeile; verborgen gehoert die
    /// eine Moeglichkeit, die nicht jeder waehlen soll. Gefiltert wird
    /// beim Hinausgeben, siehe [`nur_erlaubte`].
    ///
    /// ⚠️ Es geht **nicht** ueber die Naht: Das Fenster soll nicht
    /// entscheiden muessen, was es zeigt.
    #[serde(skip)]
    pub nur_admin: bool,
}

/// Kurzschreibweise fuer die Tabelle weiter unten.
pub const fn wahl(wert: &'static str, titel: &'static str) -> Wahl {
    Wahl { wert, titel, nur_admin: false }
}

/// Dasselbe fuer einen Wert, den nur die Adminmarke freigibt.
pub const fn wahl_admin(wert: &'static str, titel: &'static str) -> Wahl {
    Wahl { wert, titel, nur_admin: true }
}

/// Gibt nur die Moeglichkeiten hinaus, die dieser Nutzer setzen darf.
///
/// ⚑ **Beim Hinausgeben und nicht beim Zeichnen.** Wer die Liste erst
/// im Fenster filtert, hat die Regel an zwei Stellen: hier und dort.
fn nur_erlaubte<S: serde::Serializer>(w: &&'static [Wahl], s: S) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    let sichtbar: Vec<&Wahl> =
        w.iter().filter(|x| !x.nur_admin || ist_admin()).collect();
    sichtbar.serialize(s)
}

/// Kurzschreibweise fuer die Tabelle darunter.
///
/// ⚑ **Die Sprachpaare stehen als Tupel und nicht als sechs
/// Parameter.** Acht Parameter reisst die Grenze von `clippy`, und das
/// zu Recht: Wer `feld(name, art, "Modell", "Model", "Artefakt",
/// "Artefact", …)` liest, zaehlt Kommata, um zu wissen, was wozu
/// gehoert. Ein Paar sagt es.
const fn feld(
    name: &'static str,
    art: Feldart,
    bereich: (&'static str, &'static str),
    titel: (&'static str, &'static str),
    hinweis: (&'static str, &'static str),
) -> Feld {
    Feld {
        name,
        art,
        bereich: bereich.0,
        bereich_en: bereich.1,
        titel: titel.0,
        titel_en: titel.1,
        hinweis: hinweis.0,
        hinweis_en: hinweis.1,
        wahl: &[],
        nur_konsole: false,
        ordner: art.ist_ordner(),
        freigabe: art.ist_freigabe(),
    }
}

/// Dasselbe fuer ein Feld mit einer festen Auswahl.
/// Dasselbe Feld, aber nur fuer die Konsole.
const fn nur_in_der_konsole(f: Feld) -> Feld {
    Feld { nur_konsole: true, ..f }
}

const fn feld_wahl(
    name: &'static str,
    bereich: (&'static str, &'static str),
    titel: (&'static str, &'static str),
    hinweis: (&'static str, &'static str),
    wahl: &'static [Wahl],
) -> Feld {
    let f = feld(name, Feldart::Auswahl, bereich, titel, hinweis);
    Feld { wahl, ..f }
}

impl Feld {
    /// Dasselbe Feld, beschriftet in einer Sprache.
    ///
    /// ⚑ **Der Name bleibt, immer.** `agent.wurzel` heisst in jeder
    /// Sprache so: Er steht in der Ablage, in `myl setzen` und in der
    /// Fehlermeldung. **Uebersetzt wird, was ein Mensch liest, und
    /// nicht, was ein Programm vergleicht.**
    pub const fn in_sprache(self, s: Sprache) -> Self {
        match s {
            Sprache::De => self,
            Sprache::En => Feld {
                bereich: self.bereich_en,
                titel: self.titel_en,
                hinweis: self.hinweis_en,
                ..self
            },
        }
    }
}

/// Die Felder, die sich setzen lassen, in der Reihenfolge der Seite.
///
/// ⚑ **Die Reihenfolge ist die Anzeige.** Gleiche Bereiche stehen
/// beieinander, und wer hier ein Feld einfuegt, verschiebt es damit
/// auch auf der Seite. Das ist beabsichtigt: Eine zweite Liste, die nur
/// die Reihenfolge festlegt, waere wieder eine zweite Liste.
pub const FELDER: [Feld; 14] = [
    // ⚑ **Sie steht zuerst** (Festlegung des Projektinhabers,
    // 2026-09-10). Sie beschriftet alles, was darunter kommt: Wer die
    // Seite in einer Sprache oeffnet, die er nicht liest, findet hier
    // als Erstes den Schalter und muss nicht bis ans Ende suchen.
    feld_wahl(
        "oberflaeche.sprache",
        ("Oberfläche", "Interface"),
        ("Sprache", "Language"),
        (
            "Die Sprache des Fensters. Feldnamen, Pfade und Modellnamen bleiben, wie sie sind; übersetzt wird, was ein Mensch liest.",
            "The language of the window. Field names, paths and model names stay as they are; what a human reads is translated.",
        ),
        &[
            wahl("de", "Deutsch"),
            wahl("en", "English"),
        ],
    ),
        nur_in_der_konsole(feld_wahl(
        "oberflaeche.design",
        ("Oberfläche", "Interface"),
        ("Konsolen-Design", "Terminal theme"),
        (
            "Wie der Konsolenclient (myelith) aussieht: Rahmen, Fusszeile und Hervorhebungen. Standard heisst: die Farben deines Terminals, unverändert. Beim Start lässt sich ein anderes wählen, ohne diese Einstellung zu ändern. Das Fenster berührt es nicht.",
            "How the terminal client (myelith) looks: frame, footer and highlights. Standard means the colours of your terminal, unchanged. At startup a different one can be picked without changing this setting. It does not touch this window.",
        ),
        &[
            wahl("standard", "Standard"),
            wahl("myelith", "Myelith"),
            wahl("bernstein", "Bernstein"),
            wahl("tiefsee", "Tiefsee"),
            wahl("tinte", "Tinte"),
        ],
    )),
    feld(
        "modell.artefakt",
        Feldart::Ordner,
        ("Modell", "Model"),
        ("Artefakt", "Artefact"),
        (
            "Der Ordner des Modells, aus dem geantwortet wird.",
            "The folder of the model that answers.",
        ),
    ),
    feld(
        "modell.token",
        Feldart::Zahl,
        ("Modell", "Model"),
        ("Länge der Antwort", "Answer length"),
        (
            "Höchstzahl der Token je Antwort. Mehr Token heißt längere Antworten und längere Wartezeit.",
            "Maximum number of tokens per answer. More tokens means longer answers and a longer wait.",
        ),
    ),
    feld(
        "modell.denken",
        Feldart::Schalter,
        ("Modell", "Model"),
        ("Vor dem Antworten denken", "Think before answering"),
        (
            "Das Modell überlegt sichtbar, bevor es antwortet. Jedes Denktoken kostet so viel Zeit wie ein Antworttoken; ohne Angabe bleibt es aus.",
            "The model reasons visibly before it answers. Every thinking token costs as much time as an answer token; off unless set.",
        ),
    ),
    feld(
        "agent.schritte",
        Feldart::Zahl,
        ("Agent", "Agent"),
        ("Schritte je Auftrag", "Steps per task"),
        (
            "Nach so vielen Werkzeugaufrufen bricht der Agent ab und antwortet mit dem, was er hat.",
            "After this many tool calls the agent stops and answers with what it has.",
        ),
    ),
    feld(
        "agent.wurzel",
        Feldart::Pfad,
        ("Agent", "Agent"),
        ("Arbeitsordner", "Working folder"),
        (
            "Der einzige Ordner, in dem die Dateiwerkzeuge arbeiten dürfen. Ohne Angabe gibt es keine Dateiwerkzeuge.",
            "The only folder the file tools may work in. Unless set there are no file tools at all.",
        ),
    ),
    feld_wahl(
        "agent.werkzeuge",
        ("Agent", "Agent"),
        ("Werkzeugkiste", "Tool box"),
        (
            "Welche Werkzeuge der Agent angeboten bekommt. Automatisch heisst: nach der Größe des geladenen Modells, Base unter sieben Milliarden Parametern, Advanced darüber. Wer einem kleinen Modell alles geben will, wählt hier Advanced.",
            "Which tools the agent is offered. Automatic means: by the size of the loaded model, Base below seven billion parameters, Advanced above. To give a small model everything, pick Advanced here.",
        ),
        &[
            wahl("automatisch", "Automatisch"),
            wahl("base", "Base"),
            wahl("advanced", "Advanced"),
            wahl_admin("1337", "1337"),
        ],
    ),
    feld_wahl(
        "agent.modus",
        ("Agent", "Agent"),
        ("Modus", "Mode"),
        (
            "Im auto mode arbeitet der Agent durch. Im manual mode wird jede schreibende Handlung vorgelegt und läuft erst nach einer Bestätigung; Lesen und Suchen fragen nicht. In diesem Fenster gibt es den Bestätigungskasten noch nicht: Hier bleiben die schreibenden Werkzeuge im manual mode ganz weg. In der Konsole (myelith) wird jede einzeln vorgelegt.",
            "In auto mode the agent works through. In manual mode every writing action is put to you first and only runs once confirmed; reading and searching never ask. This window has no confirmation box yet: here the writing tools are simply withheld in manual mode. In the terminal (myelith) each one is put to you.",
        ),
        &[wahl("auto", "auto mode"), wahl("manual", "manual mode")],
    ),
    feld(
        "agent.schreiben",
        Feldart::Schalter,
        ("Agent", "Agent"),
        ("Schreiben erlauben", "Allow writing"),
        (
            "Lässt den Agenten im Arbeitsordner auch ändern und anlegen. Ohne Angabe darf er nur lesen.",
            "Lets the agent change and create inside the working folder. Unless set it may only read.",
        ),
    ),
    feld(
        "kap.kerne",
        Feldart::Grenze,
        ("Grenzen dieses Rechners", "Limits of this machine"),
        ("Rechenkerne", "CPU cores"),
        (
            "Wie viele der Kerne dieses Rechners Myelith benutzen darf. Das ändert die Laufzeit und nie das Ergebnis, denn jede Ausgabezeile wird für sich gerechnet. Ganz links heißt: alle.",
            "How many of this machine's cores Myelith may use. This changes the running time and never the result, because every output row is computed on its own. Far left means: all of them.",
        ),
    ),
    feld(
        "kap.speicher",
        Feldart::Grenze,
        ("Grenzen dieses Rechners", "Limits of this machine"),
        ("Arbeitsspeicher in GiB", "Memory in GiB"),
        (
            "Wie viel Arbeitsspeicher Myelith belegen darf. Ein Modell, dessen Artefakt darüber liegt, wird gar nicht erst geladen. Ganz links heißt: ohne Grenze.",
            "How much memory Myelith may take. A model whose artefact is larger is not loaded at all. Far left means: no limit.",
        ),
    ),
    feld(
        "kap.platte",
        Feldart::Grenze,
        ("Grenzen dieses Rechners", "Limits of this machine"),
        ("Plattenplatz in GiB", "Disk space in GiB"),
        (
            "Wie viel Platz Myelith für Modelle und Artefakte bekommt. Der Platz wird beim Start wirklich belegt und beim Beenden wieder freigegeben; was nicht hineinpasst, wird gar nicht erst geholt. Ganz links heißt: ohne Grenze und ohne Reservierung.",
            "How much room Myelith gets for models and artefacts. The space is really claimed at start and released on exit; what would not fit is not fetched at all. Far left means: no limit and no reservation.",
        ),
    ),
    feld(
        "ausgabe.ordner",
        Feldart::Pfad,
        ("Ausgabe", "Export"),
        ("Ordner für ausgegebene Gespräche", "Folder for exported conversations"),
        (
            "Wohin ein ausgegebenes Gespräch geschrieben wird. Ohne Angabe wird nichts geschrieben, und das Fenster führt beim Ausgeben hierher.",
            "Where an exported conversation is written. Unless set nothing is written, and exporting leads here instead.",
        ),
    ),
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
    /// Ein Verzeichnis, das **immer** einen Wert hat.
    ///
    /// ⚑ **Das Verhaeltnis zu [`Feldart::Pfad`] ist dasselbe wie das
    /// von [`Feldart::Zahl`] zu [`Feldart::Grenze`]:** Beide meinen ein
    /// Verzeichnis, aber nur eines laesst sich wegnehmen. Ein Modell
    /// ohne Artefakt waere kein enger gestellter Klient, sondern einer,
    /// der nicht antwortet.
    Ordner,
    /// Einer aus einer festen Liste, siehe [`Feld::wahl`].
    ///
    /// ⚑ **Die Liste haengt am Feld und nicht an der Art.** Eine Art
    /// `Auswahl(&[…])` traege die Werte im Typ, und dann gaebe es die
    /// Feldart „Sprache" statt der Feldart „Auswahl": Die naechste
    /// Auswahl braeuchte eine zweite Art fuer denselben Bedienweg.
    Auswahl,
}

impl Feldart {
    /// Steht in diesem Feld ein Verzeichnis?
    ///
    /// ⚑ **Die Frage entscheidet, ob eine Oberflaeche einen Auswaehler
    /// anbieten darf**, und sie wird hier beantwortet und nicht dort.
    /// Ein Fenster, das die beiden Namen selbst aufzaehlt, faengt bei
    /// der naechsten Feldart wieder von vorne an.
    pub const fn ist_ordner(self) -> bool {
        matches!(self, Self::Ordner | Self::Pfad)
    }

    /// Gibt dieses Feld ein Betriebsmittel der Maschine frei?
    ///
    /// ⚑ **Die Frage entscheidet, ob eine Oberflaeche einen
    /// Schieberegler zeigen darf**, denn ein Regler braucht ein Ende,
    /// und das Ende ist, was die Maschine hergibt. Sie wird hier
    /// beantwortet und nicht dort, aus demselben Grund wie
    /// [`Feldart::ist_ordner`].
    ///
    /// ⚑ **Und sie ist abgeleitet und nicht gesetzt.** Jedes
    /// [`Feldart::Grenze`]-Feld ist eine Freigabe: Kerne, Arbeitsspeicher,
    /// Platte. Ein von Hand gesetztes Merkmal waere ein zwoelffach
    /// wiederholtes Ja oder Nein, und eines davon waere irgendwann
    /// falsch.
    pub const fn ist_freigabe(self) -> bool {
        matches!(self, Self::Grenze)
    }
}

/// Der Namensanfang, unter dem die Freigabe eines Rechenwerks steht.
///
/// ⚑ **An einer Stelle und nicht an dreien.** Setzer, Oberflaeche und
/// Scan nennen denselben Namen, und wer ihn hier aendert, aendert ihn
/// ueberall.
pub const RECHENWERK_PRAEFIX: &str = "kap.rechenwerk.";

/// Was `an` bedeutet.
fn ja(w: &str) -> bool {
    matches!(w, "an" | "ja" | "true" | "1")
}

/// ⚑ `aus` loescht eine Grenze, statt sie auf null zu setzen.
fn opt(w: &str) -> Option<u32> {
    w.parse().ok().filter(|_| w != "aus")
}

/// Der Wert eines Feldes, so wie eine Oberflaeche ihn braucht.
///
/// ⚑ **`untagged`, damit daraus schlichtes JSON wird:** eine Zahl, ein
/// Wahrheitswert, eine Zeichenkette oder `null`. Ein Fenster soll den
/// Wert anzeigen und nicht erst eine Huelle auspacken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Feldwert {
    Zahl(u64),
    Schalter(bool),
    Text(String),
    /// Nicht gesetzt. ⚑ **Und das ist etwas anderes als null oder
    /// leer:** Eine Grenze, die nicht gesetzt ist, gibt es nicht; eine
    /// auf null waere ein Stillstand.
    Leer,
}

impl Einstellungen {
    /// Was in einem Feld steht, unter demselben Namen, unter dem
    /// [`Einstellungen::setzen`] es kennt.
    ///
    /// # 📌 Warum es diesen Gegenpart gibt
    ///
    /// **Fund 280.** Die Einstellungsseite zeichnete ihre Zeilen aus
    /// [`FELDER`], holte die **Werte** aber aus einer zweiten, von Hand
    /// gepflegten Zuordnung im Fenster, und die kannte drei der zwoelf
    /// Felder nicht. In JavaScript ist ein fehlender Schluessel kein
    /// Fehler, sondern `undefined`: Der Schalter stand immer aus, die
    /// Textfelder immer leer.
    ///
    /// ⚑ **Ein Setzer ohne Leser ist eine halbe Naht.** Solange nur das
    /// Schreiben hier lag und das Lesen anderswo, mussten beide Listen
    /// von Hand zusammengehalten werden. Jetzt liegt beides hier, und
    /// `wert_und_setzer_kennen_dieselben_felder` faehrt jedes Feld
    /// einmal hin und zurueck.
    pub fn wert(&self, feld: &str) -> Result<Feldwert, String> {
        let zahl = |o: Option<u32>| o.map_or(Feldwert::Leer, |v| Feldwert::Zahl(v as u64));
        let text = |o: &Option<String>| {
            o.as_ref().map_or(Feldwert::Leer, |s| Feldwert::Text(s.clone()))
        };
        Ok(match feld {
            "modell.artefakt" => Feldwert::Text(self.modell.artefakt.clone()),
            "modell.token" => Feldwert::Zahl(self.modell.token as u64),
            "modell.denken" => Feldwert::Schalter(self.modell.denken),
            "agent.schritte" => Feldwert::Zahl(self.agent.schritte as u64),
            "agent.wurzel" => text(&self.agent.wurzel),
            "agent.schreiben" => Feldwert::Schalter(self.agent.schreiben),
            "agent.werkzeuge" => Feldwert::Text(self.agent.werkzeuge.kennung().to_string()),
            "agent.modus" => Feldwert::Text(self.agent.modus.kennung().to_string()),
            "kap.kerne" => self.kapazitaet.kerne.map_or(Feldwert::Leer, |v| Feldwert::Zahl(v as u64)),
            "kap.speicher" => zahl(self.kapazitaet.speicher_gib),
            "kap.platte" => zahl(self.kapazitaet.platte_gib),
            "ausgabe.ordner" => text(&self.ausgabe.ordner),
            "oberflaeche.sprache" => {
                Feldwert::Text(self.oberflaeche.sprache.kennung().to_string())
            }
            "oberflaeche.design" => {
                Feldwert::Text(self.oberflaeche.design.kennung().to_string())
            }
            andere if andere.starts_with(RECHENWERK_PRAEFIX) => {
                let kennung = &andere[RECHENWERK_PRAEFIX.len()..];
                self.kapazitaet
                    .rechenwerke
                    .get(kennung)
                    .map_or(Feldwert::Leer, |p| Feldwert::Zahl(*p as u64))
            }
            andere => return Err(format!("unbekanntes Feld {andere}")),
        })
    }

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
            "agent.wurzel" => self.agent.wurzel = (wert != "aus").then(|| wert.to_string()),
            "agent.schreiben" => self.agent.schreiben = ja(wert),
            "agent.werkzeuge" => self.agent.werkzeuge = Werkzeugwahl::aus(wert)?,
            "agent.modus" => self.agent.modus = Agentenmodus::aus(wert)?,
            "kap.kerne" => self.kapazitaet.kerne = opt(wert).map(|v| v as usize),
            "kap.speicher" => self.kapazitaet.speicher_gib = opt(wert),
            "kap.platte" => self.kapazitaet.platte_gib = opt(wert),
            "ausgabe.ordner" => {
                self.ausgabe.ordner = (wert != "aus").then(|| wert.to_string())
            }
            // ⚑ **Kein `aus`.** Ein Fenster ohne Sprache gibt es nicht;
            // wo andere Felder eine Grenze wegnehmen koennen, gaebe das
            // hier nur eine dritte Schreibweise fuer Deutsch.
            "oberflaeche.sprache" => self.oberflaeche.sprache = Sprache::aus(wert)?,
            "oberflaeche.design" => self.oberflaeche.design = Konsolendesign::aus(wert)?,
            // ⚑ **Die Freigabe je Rechenwerk hat keinen festen Namen**,
            // denn wie viele Rechenwerke es gibt, weiss erst der Scan.
            // Der Setzer bleibt trotzdem die eine Stelle, die die
            // Feinheiten kennt: `aus` nimmt die Freigabe weg, und ein
            // Anteil ueber hundert Prozent ist keiner.
            andere if andere.starts_with(RECHENWERK_PRAEFIX) => {
                let kennung = &andere[RECHENWERK_PRAEFIX.len()..];
                if kennung.is_empty() {
                    return Err("kein Rechenwerk genannt".to_string());
                }
                let p = if wert == "aus" { 0 } else {
                    let p: u32 = wert.parse().map_err(|_| format!("{wert} ist keine Zahl"))?;
                    if p > 100 {
                        return Err(format!("{p} ist mehr als hundert Prozent"));
                    }
                    p as u8
                };
                // ⚑ Null Prozent ist „nichts freigegeben" und damit
                // dasselbe wie kein Eintrag. Beides abzulegen hiesse
                // zwei Schreibweisen fuer einen Zustand.
                if p == 0 {
                    self.kapazitaet.rechenwerke.remove(kennung);
                } else {
                    self.kapazitaet.rechenwerke.insert(kennung.to_string(), p);
                }
            }
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

    /// **Ein Wert, den dieses Feld annehmen muss.**
    ///
    /// ⚑ **Er steht an einer Stelle und nicht in jeder Pruefung
    /// neu.** Drei Pruefungen brauchten ihn, jede hatte ihre eigene
    /// Zuordnung, und eine davon fiel ueber die Feldart `Auswahl`, weil
    /// sie einen Auffangzweig `_ => "an"` trug. **Ein Auffangzweig ueber
    /// einer Aufzaehlung ist eine Zusage, dass nichts dazukommt.**
    fn probewert(f: &Feld) -> &'static str {
        match f.art {
            Feldart::Text | Feldart::Pfad | Feldart::Ordner => "/tmp/x",
            Feldart::Zahl | Feldart::Grenze => "4",
            Feldart::Schalter => "an",
            Feldart::Auswahl => {
                f.wahl.first().unwrap_or_else(|| panic!("{} ist eine Auswahl ohne Werte", f.name)).wert
            }
        }
    }

    /// ⚑ **Die Liste und der Setzer muessen dieselben Felder kennen.**
    /// Ein Feld in `FELDER`, das der Setzer ablehnt, waere eine Zeile
    /// in der Oberflaeche, die nichts tut.
    #[test]
    fn jedes_gelistete_feld_laesst_sich_setzen() {
        for feld in FELDER {
            let mut e = Einstellungen::default();
            let name = feld.name;
            e.setzen(name, probewert(&feld))
                .unwrap_or_else(|f| panic!("{name} steht in FELDER, der Setzer sagt: {f}"));
        }
    }

    /// ⚑ **Das Ordnermerkmal folgt aus der Art und nirgends sonst.**
    /// Waere es in der Tabelle getippt, stuende es zwoelfmal da, und
    /// eines davon waere irgendwann falsch.
    #[test]
    fn das_ordnermerkmal_folgt_aus_der_art() {
        for f in FELDER {
            assert_eq!(f.ordner, f.art.ist_ordner(), "{} traegt ein fremdes Merkmal", f.name);
        }
        // ⚑ Diese Aufzaehlung ist Absicht und keine zweite Liste: Sie
        // haelt fest, **welche** Felder einen Auswaehler bekommen.
        // Kommt ein Verzeichnisfeld dazu, faellt sie, und genau dann
        // soll jemand hinsehen.
        let ordner: Vec<&str> = FELDER.iter().filter(|f| f.ordner).map(|f| f.name).collect();
        assert_eq!(
            ordner,
            ["modell.artefakt", "agent.wurzel", "ausgabe.ordner"],
            "die Menge der Verzeichnisfelder hat sich geaendert; bekommt das neue Feld einen Auswaehler?"
        );
    }

    /// ⚑ **Eine Beschriftung ist kein Feldname.** Sie steht in der
    /// Oberflaeche vor einem Menschen; ein Punkt darin waere wieder die
    /// Kennung, die dort bis zum 2026-09-09 stand.
    #[test]
    fn jede_beschriftung_ist_deutsch_und_vollstaendig() {
        for f in FELDER {
            assert!(!f.titel.contains('.'), "`{}`: die Beschriftung `{}` traegt einen Punkt", f.name, f.titel);
            assert!(!f.titel.is_empty(), "`{}` hat keine Beschriftung", f.name);
            assert!(!f.bereich.is_empty(), "`{}` hat keinen Bereich", f.name);
            assert!(
                f.hinweis.len() > 20 && f.hinweis.ends_with('.'),
                "`{}`: der Hinweis ist kein ganzer Satz: {}",
                f.name,
                f.hinweis
            );
        }
    }

    /// Und die Gegenrichtung: nichts Gesetztes fehlt in der Liste.
    ///
    /// 📌 **Diese Pruefung fuehrte bis zum 2026-09-10 eine handgepflegte
    /// Liste von elf Namen**, also Fund 271 an einer dritten Stelle. Als
    /// `kap.beschleuniger` entfiel, fiel sie wegen eines Feldes, das es
    /// nicht mehr gibt, und **das war der harmlose Ausgang**: Ein neu
    /// hinzugefuegtes Feld haette sie gar nicht bemerkt, und genau das
    /// zu bemerken ist ihr Zweck.
    ///
    /// ⚑ **Sie liest die Namen jetzt aus dem Setzer selbst**, naemlich
    /// aus den Zweigen seiner Fallunterscheidung. Damit gibt es keine
    /// zweite Liste mehr, die rotten koennte.
    #[test]
    fn die_liste_ist_vollstaendig() {
        let quelle = include_str!("einstellungen.rs");
        let rumpf = quelle
            .split_once("pub fn setzen(")
            .and_then(|(_, r)| r.split_once("\n    }"))
            .map(|(k, _)| k)
            .expect("kein Setzer gefunden");

        let mut namen = Vec::new();
        for zeile in rumpf.lines() {
            let z = zeile.trim();
            let Some(rest) = z.strip_prefix('"') else { continue };
            let Some((name, danach)) = rest.split_once('"') else { continue };
            if danach.trim_start().starts_with("=>") {
                namen.push(name.to_string());
            }
        }
        assert!(
            namen.len() >= 10,
            "nur {} Feldnamen im Setzer gefunden; liest die Pruefung ihn noch richtig?",
            namen.len()
        );

        let mut e = Einstellungen::default();
        for name in &namen {
            assert!(
                FELDER.iter().any(|f| f.name == name),
                "{name} laesst sich setzen, steht aber nicht in FELDER"
            );
            // Und er nimmt ihn auch wirklich an.
            let f = FELDER.iter().find(|f| f.name == name).expect("gerade geprueft");
            e.setzen(name, probewert(f)).unwrap_or_else(|m| panic!("{name}: {m}"));
        }

        // ⚑ **Und der Zweig ohne festen Namen**, die Freigabe je
        // Rechenwerk. Er steht nicht in FELDER, weil erst der Scan
        // weiss, wie viele Rechenwerke es gibt, und gerade deshalb
        // gehoert er hier geprueft: Sonst faende ihn keine der beiden
        // Richtungen.
        e.setzen(&format!("{RECHENWERK_PRAEFIX}apple-m5-pro"), "50").expect("Freigabe setzen");
        assert_eq!(e.kapazitaet.rechenwerke.get("apple-m5-pro"), Some(&50));
        e.setzen(&format!("{RECHENWERK_PRAEFIX}apple-m5-pro"), "aus").expect("Freigabe weg");
        assert!(e.kapazitaet.rechenwerke.is_empty(), "aus hat die Freigabe nicht weggenommen");
        assert!(e.setzen(&format!("{RECHENWERK_PRAEFIX}x"), "101").is_err(), "ueber hundert Prozent");
        assert!(e.setzen(RECHENWERK_PRAEFIX, "50").is_err(), "kein Rechenwerk genannt");
    }

    /// ⚑ **Jedes Feld faehrt einmal hin und zurueck.**
    ///
    /// 📌 **Die Pruefung, die Fund 280 unmoeglich macht.** Solange nur
    /// der Setzer hier lag und das Lesen im Fenster, konnte ein Feld
    /// gesetzt und nirgends angezeigt werden. Jetzt gibt es beide
    /// Richtungen an einer Stelle, und diese Pruefung faehrt sie: Was
    /// `setzen` annimmt, muss `wert` wiedergeben, und zwar unveraendert.
    #[test]
    fn wert_und_setzer_kennen_dieselben_felder() {
        for f in FELDER {
            let mut e = Einstellungen::default();

            // Jedes Feld muss lesbar sein, auch ungesetzt.
            e.wert(f.name).unwrap_or_else(|m| panic!("{}: {m}", f.name));

            let (hinein, erwartet) = match f.art {
                Feldart::Schalter => ("an", Feldwert::Schalter(true)),
                Feldart::Zahl | Feldart::Grenze => ("7", Feldwert::Zahl(7)),
                Feldart::Text | Feldart::Pfad | Feldart::Ordner => {
                    ("/tmp/x", Feldwert::Text("/tmp/x".to_string()))
                }
                // ⚑ **Jeder** angebotene Wert muss durchkommen, nicht
                // nur der erste: Eine Auswahl, deren zweiter Eintrag
                // abgelehnt wird, ist eine Liste mit einer Falle darin.
                Feldart::Auswahl => {
                    // ⚑ Was nur die Adminmarke freigibt, wird hier
                    // nicht angeboten und darf deshalb auch nicht
                    // durchkommen; dafuer gibt es
                    // `die_verborgene_kiste_braucht_die_marke`.
                    for w in f.wahl.iter().filter(|w| !w.nur_admin || ist_admin()) {
                        e.setzen(f.name, w.wert)
                            .unwrap_or_else(|m| panic!("{}: `{}` wird abgelehnt: {m}", f.name, w.wert));
                        assert_eq!(
                            e.wert(f.name).expect("lesen"),
                            Feldwert::Text(w.wert.to_string()),
                            "`{}` nimmt `{}` an und gibt etwas anderes zurueck",
                            f.name,
                            w.wert
                        );
                    }
                    assert!(
                        e.setzen(f.name, "gibt-es-nicht").is_err(),
                        "`{}` nimmt einen Wert an, der nicht in der Liste steht",
                        f.name
                    );
                    continue;
                }
            };
            e.setzen(f.name, hinein).unwrap_or_else(|m| panic!("{}: {m}", f.name));
            assert_eq!(
                e.wert(f.name).expect("lesen"),
                erwartet,
                "`{}` nimmt `{hinein}` an und gibt etwas anderes zurueck",
                f.name
            );

            // Und was sich wegnehmen laesst, ist danach leer.
            if matches!(f.art, Feldart::Grenze | Feldart::Pfad) {
                e.setzen(f.name, "aus").expect("aus");
                assert_eq!(
                    e.wert(f.name).expect("lesen"),
                    Feldwert::Leer,
                    "`{}` ist nach `aus` nicht leer",
                    f.name
                );
            }
        }

        // Und der Zweig ohne festen Namen, dieselbe Runde.
        let mut e = Einstellungen::default();
        let name = format!("{RECHENWERK_PRAEFIX}apple-m5-pro");
        assert_eq!(e.wert(&name).expect("lesen"), Feldwert::Leer);
        e.setzen(&name, "40").expect("setzen");
        assert_eq!(e.wert(&name).expect("lesen"), Feldwert::Zahl(40));
        assert!(e.wert("gibt.es.nicht").is_err(), "ein unbekanntes Feld gibt einen Wert her");
    }

    /// 📌 **Der Ort der Einstellungsdatei, plattformweise.**
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

        // 📌 Der Fall, der die ganze Aenderung ausgeloest hat: keine der
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

        // 📌 **Eine vorhandene Datei gewinnt vor der bevorzugten
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

