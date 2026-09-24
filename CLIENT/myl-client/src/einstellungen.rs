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

/// **Die Umgebungsvariable, die den Standard-Arbeitsordner nennt.**
///
/// ⚑ Sie hat Vorrang vor dem Ordner im Baum, damit ein Einsatz
/// ausserhalb des Repositoriums und eine Probe einen eigenen Ordner
/// nennen koennen, ohne die Einstellungsdatei anzufassen.
pub const ARBEITSORDNER: &str = "MYL_ARBEITSORDNER";

/// **Wie der Standard-Arbeitsordner im Repositorium heisst.**
///
/// ⚑ Derselbe Name wie die Umgebungsvariable ohne Praefix, und das ist
/// Absicht: Wer den einen liest, kennt den anderen.
pub const ARBEITSORDNER_IM_BAUM: &str = "WORK_DIR";

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
    /// ⚑ **Kein Eintrag heisst ganz freigegeben**, genau wie `None` bei
    /// den drei Grenzen daneben (Auftrag des Projektinhabers,
    /// 2026-09-16). Eine ausdrueckliche `0` heisst „dieses Geraet nicht
    /// benutzen"; sie wird deshalb abgelegt und nicht geloescht.
    ///
    /// 📌 **Bis zum 2026-09-16 war es umgekehrt**, und das war eine
    /// vorsichtige Vorgabe aus der Zeit, als ueber kein Rechenwerk ein
    /// Rechenpfad fuehrte. Seit `metal` rechnet, waere sie eine GPU, die
    /// ohne Grund danebensteht.
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
    /// ⚑ **Wer nichts sagt, gibt alles her** (Auftrag des
    /// Projektinhabers, 2026-09-16). Jede Grenze ist `None`, kein
    /// Rechenwerk ist beschraenkt, und jeder Regler steht damit am
    /// rechten Anschlag auf „ohne Grenze".
    ///
    /// 📌 **Hier stand „wer nichts sagt, gibt nur die CPU".** Das galt,
    /// solange ueber kein Rechenwerk ein Rechenpfad fuehrte; seit
    /// `metal` rechnet, hielte es eine GPU zurueck, die der Nutzer
    /// gerade deshalb gekauft hat.
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
    /// ⚑ **Ohne Angabe [`Einstellungen::standard_wurzel`]**, also
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
    /// unter `CLIENT/werkzeugkisten/Base` genommen. Wer hier einen
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
    /// Ob schreibende Handlungen vorgelegt werden.
    ///
    /// ⚑ `#[serde(default)]`, damit eine Ablage aus der Zeit davor
    /// lesbar bleibt und dann `auto` bedeutet: **die Vorgabe, die es
    /// vorher auch war.**
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
    /// ⚠️ **Es gilt nur für den Chat**, also für den Zuschnitt, der nur
    /// die Anhänge sieht. Im vollen Agentenbetrieb bleiben die beiden
    /// Werkzeuge draussen, weil dort mit `run_command` schon ein Weg
    /// nach draussen offensteht und keine Schranke ihn einfasst.
    #[serde(default)]
    pub web_recherche: bool,
}

impl Default for Agenteneinstellung {
    fn default() -> Self {
        Self {
            schritte: 6,
            wurzel: None,
            schreiben: false,
            kistenordner: None,
            warnung: true,
            modus: Agentenmodus::Auto,
            blick_bildschirm: false,
            blick_kamera: false,
            web_recherche: false,
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
    /// Wie das Fenster aussieht. ⚑ Die Konsole beruehrt es nicht.
    ///
    /// 📌 `#[serde(default)]`, damit eine Ablage aus der Zeit davor
    /// weiter lesbar bleibt; ohne das waere jede bestehende Datei mit
    /// einem Schlag kaputt, und `lesen` lehnt eine kaputte Datei zu
    /// Recht ab.
    #[serde(default)]
    pub thema: Fensterthema,
    /// Wie gross die Schrift im Fenster ist.
    #[serde(default)]
    pub schrift: Schriftgroesse,
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

    /// **Der Standard-Arbeitsordner, wenn keiner gesetzt ist**:
    /// `WORK_DIR` im Repositorium (Auftrag des Projektinhabers,
    /// 2026-09-15).
    ///
    /// ⚑ **Ein Ordner mit Beispieldateien statt des CTF-Ordners.** Bis
    /// zum 2026-09-15 stand hier `BENCHMARKS/Agent/ctf`. Der ist ein
    /// Pruefstand mit Aufgaben, kein Arbeitsplatz: Wer den Client zum
    /// ersten Mal oeffnet, soll Dateien vorfinden, an denen sich jedes
    /// Werkzeug zeigt, und eine README, die dazu die Prompts nennt.
    ///
    /// ⚑ **Ueber [`crate::ort::wurzel`] gefunden, nicht vom
    /// Arbeitsverzeichnis aufwaerts.**
    ///
    /// 📌 Genau dieser Unterschied war der Fehler bei `kiste_ordner`:
    /// Das Fenster aus dem Finder hat als Arbeitsverzeichnis `/`, und
    /// ein Lauf aufwaerts von dort findet nie ein Repositorium. `wurzel`
    /// sucht zusaetzlich beim Programm selbst und faellt auf den
    /// gemerkten Ort zurueck, und das ist der Fall, um den es hier geht.
    ///
    /// `None`, wenn nichts passt: dann bleibt es dabei, dass ohne
    /// gesetzten Ordner keine Dateiwerkzeuge laufen.
    pub fn standard_wurzel() -> Option<String> {
        use std::path::PathBuf;
        // ⚑ **Die Umgebung hat Vorrang**, damit eine Probe und ein
        // Einsatz ausserhalb des Repositoriums einen eigenen Ordner
        // nennen koennen, ohne die Einstellungsdatei anzufassen.
        if let Some(p) = std::env::var_os(ARBEITSORDNER) {
            let p = PathBuf::from(p);
            if p.is_dir() {
                return Some(p.display().to_string());
            }
        }
        let o = crate::ort::wurzel()?.join(ARBEITSORDNER_IM_BAUM);
        o.is_dir().then(|| o.display().to_string())
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
    /// **Wo dieses Feld etwas bewirkt.**
    ///
    /// ⚑ **Wo es nichts bewirkt, steht es nicht** (Festlegung des
    /// Projektinhabers, 2026-09-11, seit dem 2026-09-16 in beide
    /// Richtungen). Eine Einstellung, die an der Stelle, an der sie
    /// steht, **nichts** bewirkt, ist schlimmer als eine fehlende: Wer
    /// sie umlegt und nichts sieht, sucht den Fehler woanders.
    ///
    /// ⚑ **Ein Feld und keine zwei Schalter.** Zwei Wahrheitswerte
    /// `nur_konsole` und `nur_fenster` liessen sich beide setzen, und
    /// dann gaebe es ein Feld, das nirgends steht und ueberall wirkt.
    /// **Ein Zustand, den es nicht geben darf, gehoert nicht
    /// darstellbar.**
    ///
    /// ⚠️ **Es verschwindet nicht aus der Kiste**, nur aus der einen
    /// Oberflaeche: `myl setzen` kennt jedes Feld, denn dort wirkt
    /// jedes.
    pub gilt: Gilt,
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
        gilt: Gilt::Ueberall,
        ordner: art.ist_ordner(),
        freigabe: art.ist_freigabe(),
    }
}

/// Dasselbe fuer ein Feld mit einer festen Auswahl.
/// Dasselbe Feld, aber nur fuer die Konsole.
const fn nur_in_der_konsole(f: Feld) -> Feld {
    Feld { gilt: Gilt::NurKonsole, ..f }
}

/// Dasselbe Feld, aber nur fuer das Fenster.
const fn nur_im_fenster(f: Feld) -> Feld {
    Feld { gilt: Gilt::NurFenster, ..f }
}

/// **Wo ein Feld etwas bewirkt.**
///
/// ⚑ **Die Frage wird hier beantwortet und nicht in den
/// Oberflaechen.** Jede von ihnen zaehlte sonst selbst auf, was sie
/// nicht zeigt, und die Liste der Ausnahmen ist genau die Sorte
/// zweiter Liste, die dieses Projekt schon mehrfach eingeholt hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Gilt {
    /// In beiden Bedieninstrumenten.
    Ueberall,
    /// Nur in `myelith`, dem Konsolenclient.
    NurKonsole,
    /// Nur im Fenster.
    NurFenster,
}

impl Gilt {
    /// Zeigt das Fenster dieses Feld?
    pub const fn im_fenster(self) -> bool {
        !matches!(self, Self::NurKonsole)
    }

    /// Zeigt die Konsole dieses Feld?
    pub const fn in_der_konsole(self) -> bool {
        !matches!(self, Self::NurFenster)
    }
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

/// **Wie das Fenster aussieht.**
///
/// ⚑ **Nur das Fenster** (Auftrag des Projektinhabers, 2026-09-16), so
/// wie [`Konsolendesign`] nur die Konsole betrifft. Die beiden sind
/// zwei Einstellungen und nicht eine: Ein Terminal bringt sein eigenes
/// Farbschema mit, ein Fenster nicht, und was fuer das eine „Standard"
/// heisst, hat im anderen keine Entsprechung.
///
/// ⚑ **`Dunkel` ist die Vorgabe, weil es der heutige Stand ist.** Eine
/// neue Vorgabe aendert das Aussehen jeder bestehenden Ablage auf
/// einen Schlag, und das waere eine Entscheidung, die niemand getroffen
/// hat.
///
/// ⚠️ **Ein drittes „System" gibt es bewusst nicht** (2026-09-16). Es
/// waere keine dritte Gestaltung, sondern die Abtretung der Wahl an das
/// Betriebssystem, und CSS kann eine Palette nicht zwischen einem
/// Attributblock und einem `@media`-Block teilen: Sie stuende zweimal
/// da. **Der erste Anlauf hat sie zweimal hingeschrieben und daneben
/// behauptet, es gebe keine Wiederholung**; die Gegenprobe blieb genau
/// deshalb stumm. Wer es will, loest im Skript auf und schreibt keine
/// zweite Palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Fensterthema {
    /// Mattes Schwarz, Graustufen, Glas: das Bild seit jeher.
    #[default]
    #[serde(rename = "dunkel")]
    Dunkel,
    /// Dieselbe Oberflaeche auf hellem Grund.
    #[serde(rename = "hell")]
    Hell,
}

impl Fensterthema {
    /// Die Kennung, wie sie in der Ablage und im Fenster steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::Dunkel => "dunkel",
            Self::Hell => "hell",
        }
    }

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    pub fn aus(k: &str) -> Result<Self, String> {
        match k {
            "dunkel" => Ok(Self::Dunkel),
            "hell" => Ok(Self::Hell),
            andere => Err(format!("unbekanntes Thema {andere}, moeglich sind dunkel, hell")),
        }
    }
}

/// **Wie gross die Schrift im Fenster ist.**
///
/// ⚑ **Ein Faktor auf die Grundschrift und keine Liste von Groessen.**
/// Das Stilblatt rechnet durchgehend in `rem`; wer die Wurzel
/// verstellt, verstellt alles im selben Verhaeltnis. **Eine zweite
/// Groessentabelle waere eine zweite Stelle, an der ein Abstand nicht
/// mitwaechst.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Schriftgroesse {
    #[serde(rename = "klein")]
    Klein,
    #[default]
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "gross")]
    Gross,
}

impl Schriftgroesse {
    /// Die Kennung, wie sie in der Ablage und im Fenster steht.
    pub const fn kennung(self) -> &'static str {
        match self {
            Self::Klein => "klein",
            Self::Normal => "normal",
            Self::Gross => "gross",
        }
    }

    /// Aus der Kennung, oder ein Fehler mit den moeglichen Werten.
    pub fn aus(k: &str) -> Result<Self, String> {
        match k {
            "klein" => Ok(Self::Klein),
            "normal" => Ok(Self::Normal),
            "gross" => Ok(Self::Gross),
            andere => Err(format!(
                "unbekannte Schriftgroesse {andere}, moeglich sind klein, normal, gross"
            )),
        }
    }
}

/// Die Felder, die sich setzen lassen, in der Reihenfolge der Seite.
///
/// ⚑ **Die Reihenfolge ist die Anzeige.** Gleiche Bereiche stehen
/// beieinander, und wer hier ein Feld einfuegt, verschiebt es damit
/// auch auf der Seite. Das ist beabsichtigt: Eine zweite Liste, die nur
/// die Reihenfolge festlegt, waere wieder eine zweite Liste.
pub const FELDER: [Feld; 20] = [
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
    // ⚑ **Das Erscheinungsbild steht gleich unter der Sprache**
    // (Auftrag des Projektinhabers, 2026-09-16). Beides ist dieselbe
    // Frage, naemlich wie das Fenster einem entgegentritt, und beides
    // sucht jemand am selben Ort.
    nur_im_fenster(feld_wahl(
        "oberflaeche.thema",
        ("Oberfläche", "Interface"),
        ("Erscheinungsbild", "Appearance"),
        (
            "Wie das Fenster aussieht. Dunkel ist mattes Schwarz mit Glas, hell dieselbe Oberfläche auf hellem Grund; in beiden Fällen Graustufen, hervorgehoben wird über Helligkeit und nicht über Farbe. Der Konsolenclient (myelith) hat sein eigenes Design und bleibt unberührt.",
            "How the window looks. Dark is matte black with glass, light the same interface on a light ground; both in greyscale, emphasis comes from brightness and not from colour. The terminal client (myelith) has its own theme and stays untouched.",
        ),
        &[
            wahl("dunkel", "Dunkel"),
            wahl("hell", "Hell"),
        ],
    )),
    nur_im_fenster(feld_wahl(
        "oberflaeche.schrift",
        ("Oberfläche", "Interface"),
        ("Schriftgröße", "Text size"),
        (
            "Wie groß die Schrift im Fenster ist. Es wächst alles im selben Verhältnis mit, auch Abstände und Knöpfe, denn das Stilblatt rechnet durchgehend relativ zur Grundschrift.",
            "How large the text in the window is. Everything scales with it, spacing and buttons included, because the stylesheet is written relative to the base size throughout.",
        ),
        &[
            wahl("klein", "Klein"),
            wahl("normal", "Normal"),
            wahl("gross", "Groß"),
        ],
    )),
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
            "Der einzige Ordner, in dem die Dateiwerkzeuge arbeiten dürfen. Ohne Angabe der Ordner WORK_DIR mit den Beispieldateien; findet sich auch der nicht, gibt es keine Dateiwerkzeuge.",
            "The only folder the file tools may work in. Unless set, the WORK_DIR folder with the example files; if that is missing too, there are no file tools at all.",
        ),
    ),
    feld(
        "agent.warnung",
        Feldart::Schalter,
        ("Agent", "Agent"),
        ("Warnung vor dem Agentenbetrieb", "Warning before agent use"),
        (
            "Zeigt vor dem Agentenbetrieb, was dabei auf dem Spiel steht, und die Regeln dazu. Das Häkchen im Fenster schaltet sie ab.",
            "Shows what is at stake in agent mode, and the rules for it, before you use it. The checkbox in the window turns it off.",
        ),
    ),
    feld(
        "agent.blick_bildschirm",
        Feldart::Schalter,
        ("Agent", "Agent"),
        ("Bildschirm ansehen dürfen", "May look at the screen"),
        (
            "Erlaubt dem Agenten, den Bildschirm aufzunehmen und das Bild von einem kleinen Modell ansehen zu lassen, wenn er danach gefragt wird. Ohne dieses Häkchen gibt es das Werkzeug gar nicht. Jede Aufnahme bleibt im Arbeitsordner unter .AGENT/blicke/ liegen. Auf macOS braucht es zusätzlich die Freigabe unter Datenschutz, Bildschirmaufnahme.",
            "Lets the agent capture the screen and have a small model look at it, when asked to. Without this box the tool does not exist at all. Every capture is kept in the working folder under .AGENT/blicke/. On macOS this also needs the Screen Recording permission.",
        ),
    ),
    feld(
        "agent.blick_kamera",
        Feldart::Schalter,
        ("Agent", "Agent"),
        ("Kamera ansehen dürfen", "May look through the camera"),
        (
            "Erlaubt dem Agenten, ein Kamerabild aufzunehmen und von einem kleinen Modell ansehen zu lassen, wenn er danach gefragt wird. Getrennt vom Bildschirm, denn eine Kamera zeigt den Raum und nicht den Rechner. Jede Aufnahme bleibt unter .AGENT/blicke/ liegen.",
            "Lets the agent capture a camera image and have a small model look at it, when asked to. Separate from the screen, because a camera shows the room and not the computer. Every capture is kept under .AGENT/blicke/.",
        ),
    ),
    feld(
        "agent.web_recherche",
        Feldart::Schalter,
        ("Agent", "Agent"),
        ("Im Web recherchieren dürfen", "May research on the web"),
        (
            "Gibt dem Chat zwei Werkzeuge: suchen und eine Seite lesen. Gelesen wird nur, was aus einem Suchtreffer stammt oder was du selbst genannt hast; eine selbst zusammengesetzte Adresse wird abgewiesen, und eine Suchfrage, die wörtlich aus einem Anhang stammt, ebenso. Fremder Seitentext kommt eingefasst und als Inhalt gekennzeichnet zurück, niemals als Anweisung. Ohne dieses Häkchen gibt es die Werkzeuge gar nicht. Es braucht curl auf dem Rechner.",
            "Gives the chat two tools: search, and read a page. Only an address from a search hit or one you named yourself is read; a self-composed address is refused, and so is a query taken verbatim from an attachment. Foreign page text comes back framed and marked as content, never as instruction. Without this box the tools do not exist at all. It needs curl on the machine.",
        ),
    ),
    feld(
        "agent.kistenordner",
        Feldart::Pfad,
        ("Agent", "Agent"),
        ("Lokale Werkzeugkiste", "Local tool box"),
        (
            "Der Ordner, aus dem die lokalen Werkzeuge kommen. Sein Name ist der Name der Kiste und sagt zugleich, welche eingebauten Werkzeuge dazukommen: base die fünf Dateiwerkzeuge, advanced zusätzlich run_command. Ohne Angabe die Kiste Base. Die verankerten Werkzeuge kommen unabhängig davon dazu; sie rechnen aus ihren Eingaben und brauchen keinen Ordner.",
            "The folder the tools come from. Its name is the box name and also decides which built-in tools come along: base the five file tools, advanced adds run_command. Unless set, the Base box. The anchored tools come along regardless; they compute from their inputs and need no folder.",
        ),
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
            "Wie viele der Kerne dieses Rechners Myelith benutzen darf. Das ändert die Laufzeit und nie das Ergebnis, denn jede Ausgabezeile wird für sich gerechnet. Ganz rechts heißt: ohne Grenze, also alle.",
            "How many of this machine's cores Myelith may use. This changes the running time and never the result, because every output row is computed on its own. Far right means: no limit, so all of them.",
        ),
    ),
    feld(
        "kap.speicher",
        Feldart::Grenze,
        ("Grenzen dieses Rechners", "Limits of this machine"),
        ("Arbeitsspeicher in GiB", "Memory in GiB"),
        (
            "Wie viel Arbeitsspeicher Myelith belegen darf. Ein Modell, dessen Artefakt darüber liegt, wird gar nicht erst geladen. Ganz rechts heißt: ohne Grenze.",
            "How much memory Myelith may take. A model whose artefact is larger is not loaded at all. Far right means: no limit.",
        ),
    ),
    feld(
        "kap.platte",
        Feldart::Grenze,
        ("Grenzen dieses Rechners", "Limits of this machine"),
        ("Plattenplatz in GiB", "Disk space in GiB"),
        (
            "Wie viel Platz Myelith für Modelle und Artefakte bekommt. Der Platz wird beim Start wirklich belegt und beim Beenden wieder freigegeben; was nicht hineinpasst, wird gar nicht erst geholt. Ganz rechts heißt: ohne Grenze und ohne Reservierung.",
            "How much room Myelith gets for models and artefacts. The space is really claimed at start and released on exit; what would not fit is not fetched at all. Far right means: no limit and no reservation.",
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
/// ⚑ **Die serde-Vorgabe fuer einen Schalter, der `true` sein muss.**
/// Eine Ablage aus der Zeit vor diesem Feld traegt es nicht, und
/// `bool::default()` waere `false`, also „schon zugestimmt". Das waere
/// eine Zustimmung, die niemand gegeben hat.
const fn an() -> bool {
    true
}

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
            // 📌 **Hier stand kurzzeitig die geltende Kiste statt des
            // gespeicherten Werts**, damit das Feld nicht leer aussieht.
            // `wert_und_setzer_kennen_dieselben_felder` hat das sofort
            // gefangen: Nach `aus` muss `Leer` herauskommen. **Ein Feld
            // sagt, was gespeichert ist**, nicht, was stattdessen wirkt;
            // sonst liesse sich „nicht gesetzt" nicht mehr von „auf die
            // Vorgabe gesetzt" unterscheiden. Die geltende Kiste zeigt die
            // Oberflaeche als Platzhalter an.
            "agent.kistenordner" => text(&self.agent.kistenordner),
            "agent.warnung" => Feldwert::Schalter(self.agent.warnung),
            "agent.blick_bildschirm" => Feldwert::Schalter(self.agent.blick_bildschirm),
            "agent.web_recherche" => Feldwert::Schalter(self.agent.web_recherche),
            "agent.blick_kamera" => Feldwert::Schalter(self.agent.blick_kamera),
            "agent.schreiben" => Feldwert::Schalter(self.agent.schreiben),
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
            "oberflaeche.thema" => {
                Feldwert::Text(self.oberflaeche.thema.kennung().to_string())
            }
            "oberflaeche.schrift" => {
                Feldwert::Text(self.oberflaeche.schrift.kennung().to_string())
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
            "agent.kistenordner" => {
                self.agent.kistenordner = (wert != "aus").then(|| wert.to_string())
            }
            "agent.warnung" => self.agent.warnung = ja(wert),
            "agent.blick_bildschirm" => self.agent.blick_bildschirm = ja(wert),
            "agent.web_recherche" => self.agent.web_recherche = ja(wert),
            "agent.blick_kamera" => self.agent.blick_kamera = ja(wert),
            "agent.schreiben" => self.agent.schreiben = ja(wert),
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
            // ⚑ **Kein `aus`, wie bei der Sprache.** Ein Fenster ohne
            // Erscheinungsbild gibt es nicht; wo andere Felder eine
            // Grenze wegnehmen koennen, gaebe das hier nur eine zweite
            // Schreibweise fuer „dunkel".
            "oberflaeche.thema" => self.oberflaeche.thema = Fensterthema::aus(wert)?,
            "oberflaeche.schrift" => self.oberflaeche.schrift = Schriftgroesse::aus(wert)?,
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
                // ⚑ **Hundert Prozent ist dasselbe wie keine Grenze**,
                // und abgelegt wird davon nur eines. Sonst stuende
                // derselbe Zustand in zwei Schreibweisen da, und die
                // Anzeige muesste beide kennen.
                let wert = if p == 100 { "aus" } else { wert };
                // ⚑ **Kein Eintrag heisst ganz freigegeben, nicht
                // gesperrt** (Auftrag des Projektinhabers, 2026-09-16).
                // Deshalb loescht `aus` den Eintrag und **null wird
                // abgelegt**: Es ist die Einstellung „dieses Geraet
                // nicht benutzen", und ohne Eintrag waere sie nicht von
                // „nie etwas eingestellt" zu unterscheiden.
                //
                // 📌 **Bis zum 2026-09-16 war es umgekehrt**, null loeschte
                // und kein Eintrag hiess null. Die Vorgabe war damit
                // „kein Rechenwerk hergeben", und ein Regler, der ohne
                // Zutun ganz rechts steht, waere eine Anzeige gewesen,
                // die das Gegenteil des Gespeicherten zeigt.
                if wert == "aus" {
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
            ["modell.artefakt", "agent.wurzel", "agent.kistenordner", "ausgabe.ordner"],
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
            // ⚑ **Ein Feldname traegt einen Punkt**, und diese Zeile ist
            // keine Schoenheit, sondern eine Gegenprobe gegen die
            // Pruefung selbst: Am 2026-09-16 las sie ein `"aus" =>` aus
            // einer Fallunterscheidung im selben Rumpf als Feldnamen und
            // schlug an. **Eine Quellprobe, die zu weit greift, meldet
            // einen Fehler, den es nicht gibt**, und das ist dieselbe
            // Klasse wie eine, die zu eng greift und nichts meldet.
            if danach.trim_start().starts_with("=>") && name.contains('.') {
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

    /// **`standard_wurzel`**: die Umgebung [`ARBEITSORDNER`] hat
    /// Vorrang, aber nur, wenn sie auf ein Verzeichnis zeigt. Eine
    /// eigene Variable, die sonst niemand liest, deshalb reicht ein
    /// eigener Test.
    /// # 📌 Beide Fragen in **einem** Test, und das ist kein Zufall
    ///
    /// Sie standen kurz als zwei da, und dann fiel der zweite aus: Die
    /// **Umgebung ist ein einziger Zustand fuer den ganzen Prozess**,
    /// `cargo test` laeuft nebenlaeufig, und waehrend der eine sie auf
    /// ein Behelfsverzeichnis stellte, raeumte der andere sie weg. Beide
    /// waren fuer sich gruen und zusammen rot.
    ///
    /// ⚑ **Zusammenlegen statt einen Riegel erfinden.** Ein Mutex haette
    /// dasselbe geleistet und die Frage aufgeworfen, wer ihn beim
    /// naechsten Mal noch nimmt; ein Test, der die Reihenfolge selbst in
    /// der Hand hat, wirft sie nicht auf.
    #[test]
    fn standard_wurzel_nimmt_die_umgebung_und_sonst_den_ordner_im_baum() {
        let alt = std::env::var_os(ARBEITSORDNER);

        // 1. Die Umgebung hat Vorrang, wenn sie auf ein Verzeichnis zeigt.
        let d = tempfile::tempdir().expect("Verzeichnis");
        let erwartet = d.path().display().to_string();
        std::env::set_var(ARBEITSORDNER, d.path());
        assert_eq!(
            Einstellungen::standard_wurzel().as_deref(),
            Some(erwartet.as_str()),
            "ein {ARBEITSORDNER}, das auf ein Verzeichnis zeigt, gewinnt"
        );

        // 2. Ein Pfad, der kein Verzeichnis ist, wird uebergangen; dann
        //    faellt die Funktion auf den Ordner im Baum zurueck.
        let datei = d.path().join("keine.txt");
        std::fs::write(&datei, "x").expect("Datei");
        std::env::set_var(ARBEITSORDNER, &datei);
        assert_ne!(
            Einstellungen::standard_wurzel().as_deref(),
            Some(datei.display().to_string().as_str()),
            "eine Datei ist kein Arbeitsordner"
        );

        // 3. Ohne Umgebung der Ordner im Baum, und den gibt es wirklich.
        //
        // 📌 Eine Vorgabe auf einen Ordner, den niemand angelegt hat,
        // ist keine Vorgabe: `standard_wurzel` gibt dann `None`, und der
        // Agent startet ohne Dateiwerkzeuge, ohne dass jemand einen
        // Fehler sieht.
        std::env::remove_var(ARBEITSORDNER);
        let w = crate::ort::wurzel().expect("die Wurzel des Repositoriums");
        let o = w.join(ARBEITSORDNER_IM_BAUM);
        assert!(o.is_dir(), "{} fehlt", o.display());
        assert!(
            o.join("README.md").is_file(),
            "der Arbeitsordner hat keine README, und dann weiss niemand, \
             was die Beispieldateien belegen sollen"
        );
        assert_eq!(
            Einstellungen::standard_wurzel(),
            Some(o.display().to_string()),
            "die Vorgabe zeigt nicht auf {}",
            o.display()
        );

        if let Some(v) = alt {
            std::env::set_var(ARBEITSORDNER, v);
        }
    }
}

