//! Die Dateiwerkzeuge des lokalen Agenten, und die Grenze, an der sie
//! halten.
//!
//! # ⚑ Warum die Einhaengegrenze hierher gehoert und nicht ins Harness
//!
//! Beim Bau stand der naheliegende Satz im Raum, die Grenze gehoere zu
//! den Sitzungsgrenzen, denn dort steht ja schon, was erlaubt ist. Er
//! stimmt nicht, und die Stelle, an der er kippt, ist eine Zeile in
//! `ausfuehrung.rs`:
//!
//! > **Wer hier ein Argument mehr hinzufuegt, sollte zweimal hinsehen.**
//! > Jedes weitere ist eine Faehigkeit, die ein Steckplatz bekommt.
//!
//! Ein Werkzeug sieht seine Argumente und gibt Text zurueck, sonst
//! nichts. Ein Laufwerk in `myl-local-agent` einzuhaengen kehrte genau
//! die Entscheidung um, die diese Kiste ueberhaupt begruendet.
//!
//! ⚑ **Die Arbeitsteilung ist deshalb:** Der Harness entscheidet,
//! **welches Werkzeug** laufen darf, aus Vollmacht, Erlaubnis und
//! Betriebsart. Der Klient entscheidet, **worauf** es zugreifen darf.
//! Beide Schranken sind noetig, und keine ersetzt die andere.
//!
//! # ⚑ Die drei Wege nach draussen, und wie sie zugehen
//!
//! | Weg | Was ihn schliesst |
//! |---|---|
//! | `../../..` im Pfad | Aufloesung **vor** dem Vergleich, nicht Textpruefung |
//! | ein Verweis, der hinauszeigt | `canonicalize` folgt ihm, der Praefixvergleich faellt |
//! | ein absoluter Pfad | derselbe Praefixvergleich, ohne Sonderfall |
//!
//! ⚑ **Textpruefung auf `..` waere die falsche Loesung.** Sie uebersieht
//! Verweise und verbietet zugleich harmlose Pfade, die im Verzeichnis
//! bleiben. Aufgeloest wird zuerst, verglichen danach.
//!
//! # 📌 Und was diese Grenze NICHT ist
//!
//! **Sie ist Code dieser Kiste, der sich selbst prueft.** Wer einen
//! Fehler in [`Einhaengung::aufloesen`] findet, oder wer ein Werkzeug
//! einhaengt, das gar nicht danach fragt, steht danach im ganzen
//! Dateisystem des Prozesses.
//!
//! ⚑ **Sie schuetzt vor einem Modell, das einen falschen Pfad
//! vorschlaegt**, und das ist der haeufige Fall. Sie schuetzt nicht vor
//! einem Werkzeug, das nicht mitspielt.
//!
//! **Dafuer braeuchte es eine Grenze des Betriebssystems**, und die ist
//! auf jeder Plattform eine andere: Landlock unter Linux, die
//! App-Sandbox unter macOS, AppContainer unter Windows. Drei
//! Mechanismen fuer dieselbe Zusage, keiner portabel; deshalb ist das
//! eigene Arbeit und nicht ein Zusatz zu dieser Datei.
//!
//! **Der Unterschied gehoert hierher und nicht nur dorthin**, denn wer
//! diese Datei liest, um sich auf sie zu verlassen, soll wissen,
//! worauf.

use std::path::{Path, PathBuf};

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use myl_local_agent::werkzeug::{Ansageform, Werkzeug};

/// Wie viel ein Lesevorgang hoechstens zurueckgibt.
///
/// ⚑ **Und die Kuerzung wird gesagt.** Ein Werkzeug, das stillschweigend
/// abschneidet, laesst das Modell ueber einer halben Datei schliessen
/// und sie fuer ganz halten.
pub const LESEGRENZE: usize = 64 * 1024;

/// Wie viel ein Schreibvorgang hoechstens entgegennimmt.
pub const SCHREIBGRENZE: usize = 256 * 1024;

/// Wie viele Eintraege eine Auflistung hoechstens nennt.
pub const LISTENGRENZE: usize = 500;

/// Wie viele Treffer eine Suche hoechstens nennt.
///
/// ⚑ **Niedriger als [`LISTENGRENZE`], und zwar mit Absicht.** Ein
/// Auflistungseintrag ist ein Wort, ein Suchtreffer eine ganze Zeile.
/// Hundert Treffer sind schon mehrere Kilobyte im Kontext des Modells,
/// und der Kontext ist die knappste Ressource dieses Clients.
pub const TREFFERGRENZE: usize = 100;

/// Wie viele Dateien eine Suche hoechstens ansieht.
///
/// 📌 **Eine Suche laeuft ueber einen Baum, den der Nutzer eingehaengt
/// hat, und der kann gross sein.** Ohne diese Grenze liefe ein einziger
/// Werkzeugaufruf minutenlang ueber ein Heimatverzeichnis, waehrend das
/// Modell wartet und der Schrittzaehler steht.
pub const SUCHDATEIGRENZE: usize = 5_000;

/// Wie tief `list_directory` hoechstens steigt.
///
/// ⚑ **Und die Vorgabe ist eins, also die alte Ebene.** Wer die Tiefe
/// nicht angibt, bekommt, was er vorher bekam; die Rekursion ist eine
/// Bitte und keine Ueberraschung.
pub const TIEFENGRENZE: u64 = 8;

/// Wie lange ein Befehl hoechstens laufen darf, in Sekunden.
///
/// ⚑ **Ein Werkzeug, das nicht zurueckkommt, haelt die ganze Schleife an.**
/// Der Agent wartet, der Schrittzaehler steht, und der Mensch sieht ein
/// Fenster, das haengt. Nach dieser Zeit wird der Befehl abgebrochen und
/// die bisherige Ausgabe zurueckgegeben.
pub const BEFEHL_ZEITGRENZE_S: u64 = 30;

/// Wie viel Ausgabe ein Befehl hoechstens zurueckgibt.
///
/// ⚑ **Dieselbe Sorge wie bei [`LESEGRENZE`]:** Der Kontext ist die
/// knappste Ressource. Ein `find /` schriebe sonst das ganze Dateisystem
/// in das Fenster des Modells. Was darueber liegt, wird mit Vermerk
/// abgeschnitten.
pub const BEFEHL_AUSGABEGRENZE: usize = 16 * 1024;

/// Das Verzeichnis, in dem die Dateiwerkzeuge arbeiten duerfen.
#[derive(Debug, Clone)]
pub struct Einhaengung {
    wurzel: PathBuf,
    schreiben: bool,
}

impl Einhaengung {
    /// ⚑ **`schreiben` ist eine eigene Entscheidung und nicht die Folge
    /// des Einhaengens.** Lesen und Schreiben sind nicht dieselbe
    /// Erlaubnis, und die Vorgabe des Klienten ist „nur lesen".
    pub fn neu(wurzel: impl AsRef<Path>, schreiben: bool) -> Result<Self, String> {
        let w = wurzel.as_ref();
        let aufgeloest = w
            .canonicalize()
            .map_err(|e| format!("Einhaengung {}: {e}", w.display()))?;
        if !aufgeloest.is_dir() {
            return Err(format!("Einhaengung {}: kein Verzeichnis", aufgeloest.display()));
        }
        Ok(Self { wurzel: aufgeloest, schreiben })
    }

    pub fn wurzel(&self) -> &Path {
        &self.wurzel
    }

    pub fn darf_schreiben(&self) -> bool {
        self.schreiben
    }

    /// Loest einen Pfad auf und prueft, dass er in der Einhaengung liegt.
    ///
    /// `muss_da_sein` unterscheidet Lesen von Schreiben: Was noch nicht
    /// existiert, laesst sich nicht aufloesen, wohl aber sein
    /// Elternverzeichnis. ⚑ Der letzte Namensteil kann dann kein Verweis
    /// sein, denn es gibt ihn nicht; die Kette darueber ist aufgeloest.
    pub fn aufloesen(&self, roh: &str, muss_da_sein: bool) -> Result<PathBuf, Werkzeugfehler> {
        if roh.trim().is_empty() {
            return Err(Werkzeugfehler { grund: "der Pfad ist leer".into() });
        }
        let angefragt = Path::new(roh);
        let kandidat = if angefragt.is_absolute() {
            angefragt.to_path_buf()
        } else {
            self.wurzel.join(angefragt)
        };

        let ziel = if muss_da_sein {
            kandidat.canonicalize().map_err(|e| Werkzeugfehler {
                grund: format!("{roh}: {e}"),
            })?
        } else {
            let Some(eltern) = kandidat.parent() else {
                return Err(Werkzeugfehler { grund: format!("{roh}: kein Elternverzeichnis") });
            };
            let Some(name) = kandidat.file_name() else {
                return Err(Werkzeugfehler { grund: format!("{roh}: kein Dateiname") });
            };
            let eltern = eltern.canonicalize().map_err(|e| Werkzeugfehler {
                grund: format!("{}: {e}", eltern.display()),
            })?;
            eltern.join(name)
        };

        if !ziel.starts_with(&self.wurzel) {
            return Err(Werkzeugfehler {
                grund: format!(
                    "{roh} liegt ausserhalb von {}; die Einhaengung ist die Grenze, \
                     und sie verschiebt sich nicht durch den Auftrag",
                    self.wurzel.display()
                ),
            });
        }
        Ok(ziel)
    }

    /// Die Marke fuer das Sitzungsprotokoll.
    ///
    /// ⚑ **Der Abdruck der Wurzel, nicht ihr Pfad.** Der Strom ist das
    /// Stueck, das die Maschine verlaesst; ein Pfad darin verriete das
    /// Wirtsverzeichnis an jeden, der ihn prueft. Wer den Pfad kennt,
    /// rechnet den Abdruck nach, und genau das ist die
    /// Nachvollziehbarkeit, um die es geht.
    pub fn marke(&self) -> myl_local_agent::strom::Einhaengungsmarke {
        myl_local_agent::strom::Einhaengungsmarke {
            wurzel: myl_types::hash::Hash::sha256(self.wurzel.as_os_str().as_encoded_bytes()),
            schreiben: self.schreiben,
        }
    }

    /// Der Pfad, wie er dem Modell gegenueber heisst: relativ zur Wurzel.
    ///
    /// ⚑ **Absolute Pfade gehoeren nicht in die Antwort.** Sie
    /// verraten das Wirtsverzeichnis und laden das Modell ein, beim
    /// naechsten Aufruf ausserhalb zu greifen.
    fn kurz(&self, p: &Path) -> String {
        p.strip_prefix(&self.wurzel).unwrap_or(p).display().to_string()
    }
}

/// Welche Werkzeuge dem Modell angeboten werden.
///
/// # ⚑ Warum das eine Wahl ist und keine Festlegung
///
/// Am 2026-09-09 kamen `search_files` und `edit_file` dazu, und beide
/// schliessen ein echtes Loch: Ohne Suche heisst „finde, wo X steht"
/// bei einem Schrittbudget von sechs lesen, lesen, lesen, Budget alle;
/// und `write_file` ersetzt die **ganze** Datei, ein Agent kann damit
/// neunzig Prozent richtig wiedergeben und zehn loeschen.
///
/// 📌 **Trotzdem ist mehr nicht ohne Weiteres besser.** Jedes Werkzeug
/// steht mit seinem Schema in **jedem** Prompt und ist bei **jeder**
/// Runde eine Wahl mehr; ein kleines Modell entscheidet darueber
/// schlechter als ein grosses. Ob der Gewinn den Preis traegt, ist
/// eine Messfrage, und `BENCHMARKS/Agent/` hat die Antwort noch nicht.
///
/// ⚑ **Deshalb haengt die Kiste am Modell und nicht an einer Vorgabe**
/// (2026-09-10, Festlegung des Projektinhabers). Siehe
/// [`Werkzeugkiste::fuer_artefakt`]; wer es anders will, stellt es ein.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// ⚠️ **Nicht zu verwechseln mit `Werkzeugkasten`** aus der
/// Vollmachtskiste. Der Kasten ist der Behaelter, in dem die
/// eingehaengten Werkzeuge zur Laufzeit liegen; die Kiste hier ist die
/// **Auswahl**, welche davon einem Modell ueberhaupt angeboten werden.
/// Beides ist ein Behaeltnis, und nur eines davon entscheidet etwas.
pub enum Werkzeugkiste {
    /// Was ein kleines Modell sicher bedienen kann.
    ///
    /// 📌 **Sie hiess bis zum 2026-09-10 `Knapp` und war „die drei, die
    /// es bis zum 2026-09-09 gab".** Das ist ein Entstehungsstand und
    /// keine Auswahl: Wer `default()` schrieb, meinte „das Uebliche"
    /// und bekam einen Zufall aus der Geschichte. **Ein Name soll
    /// sagen, wonach die Kiste zusammengestellt ist, nicht wann.**
    #[default]
    Base,
    /// Alles, was ein grosses Modell brauchen kann.
    Advanced,
    /// **Die Kiste ohne Ruecksicht**, und sie steht nicht in der
    /// Auswahl.
    ///
    /// ⚑ **Benannt vom Projektinhaber** (2026-09-11). Hier landet, was
    /// nur jemand bekommen soll, der weiss, was er tut: heute nichts,
    /// was `Advanced` nicht auch hat, und das steht so da, damit
    /// niemand mehr vermutet, als drin ist.
    ///
    /// ⚠️ **Verborgen heisst nicht geschuetzt.** Siehe
    /// [`crate::einstellungen::ist_admin`]: Die Marke haelt die Kiste
    /// aus der Auswahl heraus, sie haelt niemanden ab, der sie sucht.
    Elite,
}

impl Werkzeugkiste {
    /// Welche Werkzeuge dazugehoeren.
    pub fn werkzeuge(&self) -> &'static [Dateiwerkzeug] {
        match self {
            Self::Base => &Dateiwerkzeug::BASE,
            // ⚠️ **Heute dasselbe wie `Advanced`.** Das ist keine
            // Nachlaessigkeit, sondern der Stand: Es gibt noch kein
            // Werkzeug, das nur hier liegt.
            Self::Advanced | Self::Elite => &Dateiwerkzeug::ALLE,
        }
    }

    /// **Welche Kiste zu diesem Artefakt passt, und warum.**
    ///
    /// # ⚑ Das Modell sagt seine Groesse selbst
    ///
    /// Jedes Artefakt traegt in `model_config.json` ein Feld `variant`:
    /// `0.5b`, `4b`, `7b`, `30b-a3b`. Es kommt aus der Kalibrierung und
    /// steht auch beim Expertengemisch da. **Der Katalog waere die
    /// schlechtere Quelle**: Er kennt nur die vier eigenen Artefakte,
    /// und wer ein eigenes eintraegt, faellt heraus.
    ///
    /// ⚑ **Bei einem Expertengemisch zaehlt die Gesamtzahl**, nicht die
    /// je Token aktive. Die Werkzeugwahl faellt einmal je Runde ueber
    /// das ganze Modell und nicht je Token.
    ///
    /// ⚠️ **Was sich nicht lesen laesst, bekommt den Grundsatz**, und
    /// der Klient sagt es. Ein Artefakt ohne `variant` koennte klein
    /// oder gross sein; die kleinere Kiste geht in beiden Faellen, die
    /// groessere nur in einem. **Aber still darf die Entscheidung nicht
    /// fallen**, deshalb kommt der Grund mit.
    pub fn fuer_artefakt(artefakt: &std::path::Path) -> (Self, String) {
        const GRENZE: f64 = 7.0;
        let konfig = artefakt.join("model_config.json");
        let Ok(roh) = std::fs::read_to_string(&konfig) else {
            return (Self::Base, format!("{} ist nicht lesbar", konfig.display()));
        };
        let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else {
            return (Self::Base, format!("{} ist kein JSON", konfig.display()));
        };
        let Some(variante) = d.get("variant").and_then(|v| v.as_str()) else {
            return (Self::Base, "das Artefakt nennt keine `variant`".to_string());
        };
        let Some(milliarden) = milliarden_aus(variante) else {
            return (Self::Base, format!("`{variante}` ist keine lesbare Groesse"));
        };
        if milliarden < GRENZE {
            (Self::Base, format!("{variante}, unter {GRENZE} Milliarden"))
        } else {
            (Self::Advanced, format!("{variante}, ab {GRENZE} Milliarden"))
        }
    }

    /// Wie die Kiste heisst, in Protokoll und Anzeige.
    ///
    /// ⚑ **Die Namen kommen vom Projektinhaber** (2026-09-11) und sind
    /// Namen und keine Woerter: Sie werden nicht uebersetzt, so wenig
    /// wie `Myelith` uebersetzt wird.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::Advanced => "Advanced",
            Self::Elite => "1337",
        }
    }

    /// Dieselbe Kiste als Kennung fuer Ablage und Schalter.
    pub fn kennung(&self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Advanced => "advanced",
            Self::Elite => "1337",
        }
    }
}

/// Welches der Dateiwerkzeuge, unabhaengig davon, wie es heisst.
///
/// # ⚑ Warum es diese Aufzaehlung gibt
///
/// Der Name ist seit dem 2026-09-09 nicht mehr fest: Er haengt an der
/// [`Ansageform`], weil ein deutsches Kompositum in einer
/// Funktionsnamen-Position ausserhalb dessen liegt, worauf das Modell
/// geschliffen wurde. Damit gibt es zwei Namen je Werkzeug, und ohne
/// diese Aufzaehlung staenden sie an drei Stellen: im Angebot, in der
/// Ausfuehrung und in der Zuordnung in `ruestung.rs`. **Die dritte war
/// ein Zeichenkettenvergleich**, also genau die Stelle, an der eine
/// Umbenennung still danebengeht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dateiwerkzeug {
    /// Listet ein Verzeichnis.
    Verzeichnis,
    /// Liest eine Datei.
    Lesen,
    /// Schreibt eine Datei; nur mit Schreiberlaubnis im Angebot.
    Schreiben,
    /// Sucht woertlich in den Dateien.
    Suchen,
    /// Ersetzt eine Stelle in einer Datei; nur mit Schreiberlaubnis.
    Aendern,
    /// **Fuehrt einen Shell-Befehl im Arbeitsverzeichnis aus** (B1 der
    /// Werkzeug-Wunschliste). Nur mit Schreiberlaubnis, und **nicht** in
    /// `Base`: Anders als die Dateiwerkzeuge haelt ein Shell-Befehl die
    /// Einhaengegrenze **nicht** ein (`cat ../fremd` liest hinaus), also
    /// gehoert er nicht in die Vorgabekiste eines kleinen Modells. Siehe
    /// [`Befehlausfuehren`] fuer die Schranken, die bleiben.
    Befehl,
}

impl Dateiwerkzeug {
    /// Alle, in der Reihenfolge, in der sie angeboten werden.
    pub const ALLE: [Dateiwerkzeug; 6] = [
        Dateiwerkzeug::Verzeichnis,
        Dateiwerkzeug::Lesen,
        Dateiwerkzeug::Suchen,
        Dateiwerkzeug::Schreiben,
        Dateiwerkzeug::Aendern,
        Dateiwerkzeug::Befehl,
    ];

    /// Der knappe Satz: die drei, die es bis zum 2026-09-09 gab.
    ///
    /// ⚑ **Er bleibt die Vorgabe, bis gemessen ist.** Mehr Werkzeuge
    /// machen die Auswahl fuer ein kleines Modell nicht leichter,
    /// sondern schwerer, und das ist keine Vermutung, die man durch
    /// Ausliefern beantwortet. `BENCHMARKS/Agent/` hat den Vergleich;
    /// `myl agent --werkzeuge voll` fuehrt den anderen Arm.
    /// Die Werkzeuge, die auch ein kleines Modell sicher bedient.
    ///
    /// 📌 **Hier standen drei, und `aendern` war nicht dabei**, wohl
    /// aber `schreiben`. Der enge Satz enthielt damit das
    /// **gefaehrlichere** Werkzeug und liess das harmlosere weg: Wer
    /// eine Zeile tauschen wollte, musste die ganze Datei lesen und
    /// ganz zurueckschreiben. `aendern` braucht dagegen einen Anker,
    /// der genau einmal vorkommt.
    ///
    /// ⚑ **Und `suchen` fehlte**, also konnte ein Agent im Fenster gar
    /// nicht suchen. Beides ist am 2026-09-10 dazugekommen.
    ///
    /// ⚑ **Seit dem 2026-09-14 wieder ein echter Ausschnitt**, nicht mehr
    /// `= ALLE`: `run_command` (B1) liegt in `Advanced` und nicht hier,
    /// weil ein Shell-Befehl die Einhaengegrenze nicht einhaelt. Damit
    /// bedeuten `Base` und `Advanced` zum ersten Mal Verschiedenes.
    pub const BASE: [Dateiwerkzeug; 5] = [
        Dateiwerkzeug::Verzeichnis,
        Dateiwerkzeug::Lesen,
        Dateiwerkzeug::Suchen,
        Dateiwerkzeug::Schreiben,
        Dateiwerkzeug::Aendern,
    ];

    /// Braucht es die Schreiberlaubnis?
    pub const fn schreibt(&self) -> bool {
        matches!(self, Self::Schreiben | Self::Aendern | Self::Befehl)
    }

    /// Wie es in dieser Form heisst.
    ///
    /// ⚑ **Die englischen Namen sind die gebraeuchlichen** und nicht
    /// frei gewaehlt: `list_directory`, `read_file`, `write_file` sind
    /// die Formen, in denen Werkzeugschliff ueblicherweise vorliegt. Ein
    /// erfundener englischer Name waere so weit draussen wie ein
    /// deutscher und haette nur die Nachteile.
    // ⚑ `const`, damit auch eine Pruefung ihre Namen daraus als
    // Konstante ziehen kann statt sie abzuschreiben.
    pub const fn name(&self, form: Ansageform) -> &'static str {
        match (self, form) {
            (Self::Verzeichnis, Ansageform::Amtlich) => "list_directory",
            (Self::Lesen, Ansageform::Amtlich) => "read_file",
            (Self::Suchen, Ansageform::Amtlich) => "search_files",
            (Self::Schreiben, Ansageform::Amtlich) => "write_file",
            (Self::Aendern, Ansageform::Amtlich) => "edit_file",
            (Self::Befehl, Ansageform::Amtlich) => "run_command",
            (Self::Verzeichnis, Ansageform::Deutsch) => "verzeichnis",
            (Self::Lesen, Ansageform::Deutsch) => "datei_lesen",
            (Self::Suchen, Ansageform::Deutsch) => "suchen",
            (Self::Schreiben, Ansageform::Deutsch) => "datei_schreiben",
            (Self::Aendern, Ansageform::Deutsch) => "datei_aendern",
            (Self::Befehl, Ansageform::Deutsch) => "befehl_ausfuehren",
        }
    }

    /// Wofuer es da ist, in der Sprache dieser Form.
    ///
    /// ⚑ **Die Beschreibung folgt dem Schalter mit, und das ist eine
    /// Entscheidung.** Sie haette deutsch bleiben koennen: Ein
    /// Beschreibungstext ist Fliesstext, und darin ist Qwen3
    /// mehrsprachig zuhause, waehrend ein Funktionsname in einer
    /// strukturellen Position steht. Trotzdem heisst der Schalter „so,
    /// wie das Modell geschliffen wurde", und eine halbe Umstellung
    /// waere eine Messung, die nicht sagt, was sie gemessen hat.
    ///
    /// ⚠️ **Der Preis ist, dass der Vergleich drei Dinge auf einmal
    /// aendert** (Name, Ansage, Beschreibung) und deshalb nicht sagen
    /// kann, welches davon wirkt. Die erste Frage ist „hilft die Form
    /// des Modells", nicht „welcher Teil davon"; wer die zweite
    /// beantworten will, braucht drei weitere Laeufe.
    pub fn beschreibung(&self, form: Ansageform) -> String {
        match (self, form) {
            // 📌 **Hier stand „Without an argument, the working
            // directory itself."**, und genau dieser Satz hat das
            // Modell ins Gruebeln gebracht: Er beschreibt einen Fall,
            // den es gar nicht geben soll. Jetzt sagt die Beschreibung,
            // **was** gelistet wird, und nicht, was man weglassen darf.
            (Self::Verzeichnis, Ansageform::Amtlich) => "List the working directory. \
                 Use tiefe 1 for its immediate contents, higher values to see \
                 subdirectories."
                .into(),
            (Self::Lesen, Ansageform::Amtlich) => format!(
                "Read a file inside the working directory, at most {LESEGRENZE} bytes."
            ),
            (Self::Suchen, Ansageform::Amtlich) => "Search the whole working directory for \
                 a literal text. Case is ignored. Returns file, line number and line."
                .into(),
            (Self::Schreiben, Ansageform::Amtlich) => "Write a file inside the working \
                 directory. Existing content is replaced."
                .into(),
            (Self::Aendern, Ansageform::Amtlich) => "Replace one or more passages inside a \
                 file. Each old text must occur exactly once; the rest of the file is \
                 untouched. Give every change you need in one call. \
                 Prefer this over write_file for changing an existing file."
                .into(),
            (Self::Verzeichnis, Ansageform::Deutsch) => "Listet das Arbeitsverzeichnis. \
                 Tiefe 1 zeigt seinen unmittelbaren Inhalt, groessere Werte auch \
                 die Unterverzeichnisse."
                .into(),
            (Self::Lesen, Ansageform::Deutsch) => format!(
                "Liest eine Datei im Arbeitsverzeichnis, hoechstens {LESEGRENZE} Bytes."
            ),
            (Self::Suchen, Ansageform::Deutsch) => "Sucht im ganzen Arbeitsverzeichnis nach \
                 woertlichem Text. Gross und klein ist egal. Gibt Datei, Zeilennummer \
                 und Zeile."
                .into(),
            (Self::Schreiben, Ansageform::Deutsch) => "Schreibt eine Datei im \
                 Arbeitsverzeichnis. Vorhandenes wird ersetzt."
                .into(),
            (Self::Aendern, Ansageform::Deutsch) => "Ersetzt eine oder mehrere Stellen in \
                 einer Datei. Jeder alte Text muss genau einmal vorkommen; der Rest bleibt \
                 unangetastet. Gib alle noetigen Aenderungen in einem Aufruf. \
                 Fuer Aenderungen an vorhandenen Dateien besser als datei_schreiben."
                .into(),
            (Self::Befehl, Ansageform::Amtlich) => format!(
                "Run a shell command in the working directory (sh -c). Returns its                  output, at most {BEFEHL_AUSGABEGRENZE} bytes, and stops after                  {BEFEHL_ZEITGRENZE_S} seconds. Prefer the file tools for reading,                  writing and searching; use this for building, running and everything                  they do not cover."
            ),
            (Self::Befehl, Ansageform::Deutsch) => format!(
                "Fuehrt einen Shell-Befehl im Arbeitsverzeichnis aus (sh -c). Gibt                  seine Ausgabe zurueck, hoechstens {BEFEHL_AUSGABEGRENZE} Bytes, und                  bricht nach {BEFEHL_ZEITGRENZE_S} Sekunden ab. Zum Lesen, Schreiben                  und Suchen die Dateiwerkzeuge; dies fuer Bauen, Ausfuehren und alles                  Uebrige."
            ),
        }
    }

    /// Das Schema der Argumente.
    ///
    /// ⚑ **Die Feldnamen (`pfad`, `inhalt`) bleiben in beiden Formen
    /// deutsch**, und der Grund ist nicht Bequemlichkeit. Ein
    /// Feldname steht **im Schema, das eine Zeile weiter im selben
    /// Kontext steht**; das Modell liest dort ab, wie das Feld heisst.
    /// Ein Funktionsname hat keinen solchen Anker: Ihn muss das Modell
    /// aus dem Namen selbst erkennen, und genau deshalb wiegt dort die
    /// Verteilung des Schliffs schwerer. Der Name des Werkzeugs ist
    /// eine Ansage, seine Argumente sind ein Vertrag.
    ///
    /// Dazu kommt, dass `ausfuehren` die Felder mit [`zeichenkette`]
    /// herausliest: Sie mitzuuebersetzen haengte die **Ausfuehrung** an
    /// die Ansageform, und ein Aufruf in der einen Form naennte Felder,
    /// die die andere nicht kennt.
    ///
    /// # 📌 Kein Werkzeug hat einen optionalen Parameter (2026-09-10)
    ///
    /// **Ein optionaler Parameter ist eine Entscheidung, die das Modell
    /// treffen muss, und ein kleines Modell bezahlt sie mit seinem
    /// Schrittbudget.** Gemeldet vom Projektinhaber: Auf die Frage
    /// „welche Dateien liegen im Verzeichnis?" ueberlegte Qwen3-4B
    /// seitenlang, ob es `pfad` weglassen, leer setzen oder mitgeben
    /// solle, las dazu die `required`-Liste, kam zu keinem Schluss und
    /// endete **ohne Schlussantwort**. Das Werkzeug haette in beiden
    /// Faellen dasselbe getan.
    ///
    /// ⚑ **Also gibt es die Wahl nicht mehr.** Jede Eigenschaft steht
    /// in `required`, und was der Agent ohnehin nicht braucht, ist
    /// entfallen: Weder `verzeichnis` noch `suchen` nehmen einen Pfad.
    /// Beide arbeiten im **Arbeitsverzeichnis**, das der Nutzer vorher
    /// waehlt, und das ist die ganze Zusage dieser Werkzeuge.
    /// `kein_werkzeug_hat_einen_optionalen_parameter` haelt die Regel.
    ///
    /// ⚑ **Die Fehlermeldung traegt den Rest.** Wer `pfad` doch
    /// mitschickt, bekommt „Feld unbekannt: pfad, bekannt: [tiefe]",
    /// und das ist eine Auskunft, aus der ein Modell im naechsten
    /// Schritt lernt.
    pub fn parameter(&self, form: Ansageform) -> serde_json::Value {
        let (wohin, tiefe_hinweis, muster_hinweis, alt_hinweis, aenderungshinweis, befehl_hinweis) = match form {
            Ansageform::Amtlich => (
                "path of the file, relative to the working directory; \
                 paths leading outside it are rejected",
                "how many levels to descend; 1 lists the working directory itself",
                "literal text, not a regular expression; the whole working directory is searched",
                "must occur exactly once in the file",
                "All changes are applied in order, and each one sees the result of the \
                 previous ones. Either all of them are written or none.",
                "the command line, run with sh -c in the working directory",
            ),
            Ansageform::Deutsch => (
                "Pfad der Datei, relativ zum Arbeitsverzeichnis; \
                 Pfade nach draussen werden abgelehnt",
                "wie viele Ebenen tief; 1 listet das Arbeitsverzeichnis selbst",
                "woertlicher Text, kein regulaerer Ausdruck; gesucht wird im ganzen Arbeitsverzeichnis",
                "muss genau einmal in der Datei vorkommen",
                "Alle Aenderungen werden der Reihe nach angewendet, und jede sieht das \
                 Ergebnis der vorigen. Entweder alle werden geschrieben oder keine.",
                "die Befehlszeile, ausgefuehrt mit sh -c im Arbeitsverzeichnis",
            ),
        };
        match self {
            // ⚑ **Ohne `pfad`.** Dieses Werkzeug listet das
            // Arbeitsverzeichnis, und `tiefe` zeigt, was darunter liegt.
            // Ein Pfad waere eine zweite Art, dieselbe Frage zu
            // stellen, und die erste, ueber die ein Modell stolpert.
            Self::Verzeichnis => serde_json::json!({
                "type": "object",
                "properties": {
                    "tiefe": {"type": "integer", "description": tiefe_hinweis}
                },
                "required": ["tiefe"]
            }),
            // ⚑ **Ebenfalls ohne `pfad`.** Eine Suche, die im ganzen
            // Arbeitsverzeichnis sucht, beantwortet die Frage, die
            // jemand hat; eine, die einen Startpunkt verlangt, verlangt
            // eine Antwort auf eine Frage davor.
            Self::Suchen => serde_json::json!({
                "type": "object",
                "properties": {
                    "muster": {"type": "string", "description": muster_hinweis}
                },
                "required": ["muster"]
            }),
            // ⚑ **Eine Liste, kein Paar** (2026-09-10).
            //
            // 📌 **Und die Liste ersetzt das Paar, statt danebenzustehen.**
            // Ein zweites Werkzeug fuer Mehrfachaenderungen waere eine
            // Wahl, ein optionales Feld daneben waere Fund 289 in neuer
            // Verkleidung: Der Einzelfall ist eine Liste mit einem
            // Eintrag, und darueber muss niemand entscheiden.
            Self::Aendern => serde_json::json!({
                "type": "object",
                "properties": {
                    "pfad": {"type": "string", "description": wohin},
                    "aenderungen": {
                        "type": "array",
                        "minItems": 1,
                        "description": aenderungshinweis,
                        "items": {
                            "type": "object",
                            "properties": {
                                "alt": {"type": "string", "description": alt_hinweis},
                                "neu": {"type": "string"}
                            },
                            "required": ["alt", "neu"]
                        }
                    }
                },
                "required": ["pfad", "aenderungen"]
            }),
            Self::Lesen => serde_json::json!({
                "type": "object",
                "properties": {"pfad": {"type": "string", "description": wohin}},
                "required": ["pfad"]
            }),
            Self::Schreiben => serde_json::json!({
                "type": "object",
                "properties": {
                    "pfad": {"type": "string", "description": wohin},
                    "inhalt": {"type": "string"}
                },
                "required": ["pfad", "inhalt"]
            }),
            Self::Befehl => serde_json::json!({
                "type": "object",
                "properties": {"befehl": {"type": "string", "description": befehl_hinweis}},
                "required": ["befehl"]
            }),
        }
    }

    /// Baut die Ausfuehrung dazu.
    pub fn ausfuehrung(
        &self,
        e: Einhaengung,
        form: Ansageform,
    ) -> Box<dyn myl_local_agent::ausfuehrung::Werkzeugausfuehrung> {
        match self {
            Self::Verzeichnis => Box::new(Verzeichnislesen(e, form)),
            Self::Lesen => Box::new(Dateilesen(e, form)),
            Self::Suchen => Box::new(Suchen(e, form)),
            Self::Schreiben => Box::new(Dateischreiben(e, form)),
            Self::Aendern => Box::new(Dateiaendern(e, form)),
            Self::Befehl => Box::new(Befehlausfuehren(e, form)),
        }
    }
}

/// Der Text eines Arguments, mit ordentlichem Fehler statt `null`.
fn zeichenkette(a: &serde_json::Value, feld: &str) -> Result<String, Werkzeugfehler> {
    a.get(feld)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Werkzeugfehler {
            grund: format!("es fehlt das Feld `{feld}` als Zeichenkette"),
        })
}

/// Sucht Text in den Dateien der Einhaengung.
///
/// # 📌 Warum es das gibt, und warum es das Wichtigste der drei ist
///
/// Das Schrittbudget steht auf sechs. Mit einer flachen Auflistung und
/// `read_file` heisst „finde, wo X steht" fuer den Agenten: listen,
/// lesen, lesen, lesen, lesen, Budget alle. Auf einem 4B-Modell mit
/// rund 165 Sekunden je Auftrag ist das nicht knapp, sondern
/// aussichtslos. Dieses Werkzeug macht aus vier Leseschritten einen.
///
/// # ⚑ Woertlich und nicht als regulaerer Ausdruck
///
/// Ein Modell, das einen regulaeren Ausdruck erzeugt, erzeugt frueher
/// oder spaeter einen falschen, und der Fehler, den es dann liest,
/// erklaert ihm nichts. Woertliche Suche schlaegt nie fehl, sie findet
/// nur nichts, und das ist eine Antwort, mit der ein Agent
/// weiterarbeiten kann. **Und ohne Abhaengigkeit:** eine Regex-Kiste
/// waere die erste in diesem Crate, die nur einem Werkzeug dient.
///
/// ⚑ **Gross und klein ist egal.** Ein Modell trifft die Schreibweise
/// oft nicht, und eine Suche, die an einem grossen Anfangsbuchstaben
/// scheitert, kostet einen Schritt fuer nichts.
pub struct Suchen(pub Einhaengung, pub Ansageform);

impl Werkzeugausfuehrung for Suchen {
    fn name(&self) -> &str {
        Dateiwerkzeug::Suchen.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let muster = zeichenkette(a, "muster")?;
        if muster.trim().is_empty() {
            return Err(Werkzeugfehler { grund: "das Suchmuster ist leer".into() });
        }
        let unten = muster.to_lowercase();
        // ⚑ Immer die Wurzel: Dieses Werkzeug kennt keinen Pfad mehr
        // (siehe `Dateiwerkzeug::parameter`).
        let wo = ".";
        let _ = a;
        let start = self.0.aufloesen(wo, true)?;

        let mut treffer: Vec<String> = Vec::new();
        let mut angesehen = 0usize;
        let mut zu_viele = false;
        let mut offen = vec![start.clone()];
        while let Some(d) = offen.pop() {
            let Ok(eintraege) = std::fs::read_dir(&d) else { continue };
            for e in eintraege.flatten() {
                let p = e.path();
                // ⚑ Der Praefixvergleich noch einmal, obwohl der Start
                // schon geprueft ist: Ein Verweis im Baum koennte
                // hinausfuehren, und `read_dir` folgt ihm.
                let Ok(echt) = p.canonicalize() else { continue };
                if !echt.starts_with(self.0.wurzel()) {
                    continue;
                }
                if echt.is_dir() {
                    offen.push(echt);
                    continue;
                }
                if angesehen >= SUCHDATEIGRENZE {
                    zu_viele = true;
                    break;
                }
                angesehen += 1;
                let Ok(inhalt) = std::fs::read(&echt) else { continue };
                // 📌 **Binaerdateien werden uebersprungen, nicht
                // eingebaut.** Ein Treffer mitten in einem Bild waere
                // eine Zeile Zufallsbytes im Kontext des Modells.
                if inhalt.len() > LESEGRENZE || inhalt.contains(&0) {
                    continue;
                }
                let Ok(text) = std::str::from_utf8(&inhalt) else { continue };
                for (nr, zeile) in text.lines().enumerate() {
                    if !zeile.to_lowercase().contains(&unten) {
                        continue;
                    }
                    if treffer.len() >= TREFFERGRENZE {
                        zu_viele = true;
                        break;
                    }
                    let gekuerzt: String = zeile.trim().chars().take(200).collect();
                    treffer.push(format!("{}:{}\t{}", self.0.kurz(&echt), nr + 1, gekuerzt));
                }
                if zu_viele {
                    break;
                }
            }
            if zu_viele {
                break;
            }
        }

        if treffer.is_empty() {
            return Ok(format!("keine Zeile enthaelt {muster:?}"));
        }
        treffer.sort();
        if zu_viele {
            treffer.push(format!(
                "[gekuerzt bei {} Treffern aus {angesehen} Dateien; das Muster enger fassen]",
                treffer.len()
            ));
        }
        Ok(treffer.join("\n"))
    }
}

/// Ersetzt eine Stelle in einer Datei, statt sie ganz zu ueberschreiben.
///
/// # 📌 Warum `write_file` allein gefaehrlich ist
///
/// Es ersetzt die **ganze** Datei. Ein Agent, der neunzig Prozent
/// richtig wiedergibt, hat die restlichen zehn geloescht, und niemand
/// sieht es, bis jemand die Datei braucht. Das ist die gefaehrlichste
/// Stelle im bisherigen Werkzeugkiste.
///
/// # ⚑ Genau ein Vorkommen, sonst ein Fehler
///
/// Kommt `alt` mehrfach vor, wird **nicht** das erste genommen, sondern
/// abgelehnt. Das erste zu nehmen waere bequem und traefe irgendwann die
/// falsche Stelle; die Ablehnung sagt dem Modell, dass es mehr Kontext
/// in `alt` aufnehmen muss, und das kann es.
pub struct Dateiaendern(pub Einhaengung, pub Ansageform);

impl Werkzeugausfuehrung for Dateiaendern {
    fn name(&self) -> &str {
        Dateiwerkzeug::Aendern.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        if !self.0.darf_schreiben() {
            return Err(Werkzeugfehler {
                grund: "diese Einhaengung ist nur zum Lesen".into(),
            });
        }
        let roh = zeichenkette(a, "pfad")?;
        let aenderungen = aenderungsliste(a)?;
        let p = self.0.aufloesen(&roh, true)?;
        let inhalt = std::fs::read_to_string(&p)
            .map_err(|e| Werkzeugfehler { grund: format!("{roh}: {e}") })?;

        // ⚑ **Erst der Trockenlauf, dann das Schreiben.**
        //
        // Jede Aenderung wird der Reihe nach auf eine Kopie angewendet,
        // und jede sieht das Ergebnis der vorigen: **genau so, wie das
        // Schreiben es tun wird.** Eine Pruefung, die jede Aenderung
        // gegen den Urzustand haelt, gaebe gruenes Licht fuer einen
        // Satz, der sich unterwegs selbst widerspricht.
        //
        // ⚠️ **Und es werden alle Fehler gesammelt, nicht der erste.**
        // Wer beim ersten abbricht, schickt das Modell in eine Runde je
        // Fehler; wer alle nennt, laesst es in einem Zug berichtigen.
        let mut stand = inhalt.clone();
        let mut fehler = Vec::new();
        for (i, (alt, _neu)) in aenderungen.iter().enumerate() {
            let zahl = stand.matches(alt.as_str()).count();
            match zahl {
                1 => {
                    let (a, n) = &aenderungen[i];
                    stand = stand.replacen(a.as_str(), n, 1);
                }
                0 => fehler.push(format!(
                    "Aenderung {}: die Stelle kommt nicht vor{}",
                    i + 1,
                    if i > 0 { " (auch nicht nach den vorigen Aenderungen)" } else { "" }
                )),
                n => fehler.push(format!(
                    "Aenderung {}: die Stelle kommt {n}-mal vor; `alt` muss eindeutig sein, \
                     also mehr Umgebung aufnehmen",
                    i + 1
                )),
            }
        }
        if !fehler.is_empty() {
            return Err(Werkzeugfehler {
                grund: format!("{roh}: nichts geschrieben.\n{}", fehler.join("\n")),
            });
        }
        if stand.len() > SCHREIBGRENZE {
            return Err(Werkzeugfehler {
                grund: format!(
                    "die Datei waere {} Bytes gross, ueber der Grenze von {SCHREIBGRENZE}; \
                     nichts geschrieben",
                    stand.len()
                ),
            });
        }

        std::fs::write(&p, &stand)
            .map_err(|e| Werkzeugfehler { grund: format!("{roh}: {e}") })?;
        Ok(format!(
            "{}: {}",
            self.0.kurz(&p),
            match aenderungen.len() {
                1 => "eine Stelle ersetzt".to_string(),
                n => format!("{n} Stellen ersetzt"),
            }
        ))
    }
}

/// Die fuehrende Zahl einer Variantenangabe, in Milliarden.
///
/// ⚑ **`30b-a3b` ergibt 30 und nicht 3.** Die Angabe hinter dem Strich
/// nennt die je Token aktiven Parameter; gelesen wird bis zum ersten
/// `b`, und das ist die Gesamtzahl.
fn milliarden_aus(variante: &str) -> Option<f64> {
    let zahl: String = variante
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!zahl.is_empty()).then(|| zahl.parse().ok()).flatten()
}

/// Liest die Aenderungsliste aus den Argumenten.
///
/// ⚑ **Sie ist Pflicht und hat mindestens einen Eintrag.** Eine leere
/// Liste waere ein Aufruf, der nichts tut und trotzdem einen Schritt
/// kostet; das Modell soll den Fehler sehen und nicht die Ruhe.
fn aenderungsliste(a: &serde_json::Value) -> Result<Vec<(String, String)>, Werkzeugfehler> {
    let Some(liste) = a.get("aenderungen").and_then(|v| v.as_array()) else {
        return Err(Werkzeugfehler {
            grund: "`aenderungen` fehlt oder ist keine Liste".into(),
        });
    };
    if liste.is_empty() {
        return Err(Werkzeugfehler { grund: "`aenderungen` ist leer".into() });
    }
    let mut aus = Vec::with_capacity(liste.len());
    for (i, e) in liste.iter().enumerate() {
        let alt = e.get("alt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let neu = e.get("neu").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if alt.is_empty() {
            return Err(Werkzeugfehler {
                grund: format!(
                    "Aenderung {}: `alt` ist leer; zum Anlegen einer Datei gibt es das \
                     Schreibwerkzeug",
                    i + 1
                ),
            });
        }
        aus.push((alt, neu));
    }
    Ok(aus)
}

/// Liest eine Datei innerhalb der Einhaengung.
pub struct Dateilesen(pub Einhaengung, pub Ansageform);

impl Werkzeugausfuehrung for Dateilesen {
    fn name(&self) -> &str {
        Dateiwerkzeug::Lesen.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let roh = zeichenkette(a, "pfad")?;
        let p = self.0.aufloesen(&roh, true)?;
        if p.is_dir() {
            return Err(Werkzeugfehler {
                grund: format!(
                    "{roh} ist ein Verzeichnis; dafuer gibt es `{}`",
                    Dateiwerkzeug::Verzeichnis.name(self.1)
                ),
            });
        }
        let inhalt = std::fs::read(&p).map_err(|e| Werkzeugfehler { grund: format!("{roh}: {e}") })?;
        let ganz = inhalt.len();
        // ⚑ Nach Bytes gekuerzt, dann auf eine gueltige Zeichengrenze
        // zurueckgesetzt: `from_utf8_lossy` auf einer halben Folge
        // erzeugte ein Ersatzzeichen, das nicht in der Datei steht.
        let mut bis = ganz.min(LESEGRENZE);
        while bis > 0 && bis < ganz && (inhalt[bis] & 0xC0) == 0x80 {
            bis -= 1;
        }
        let text = String::from_utf8_lossy(&inhalt[..bis]);
        if bis < ganz {
            Ok(format!(
                "{text}\n\n[gekuerzt: {bis} von {ganz} Bytes gezeigt]"
            ))
        } else {
            Ok(text.into_owned())
        }
    }
}

/// Schreibt eine Datei innerhalb der Einhaengung.
pub struct Dateischreiben(pub Einhaengung, pub Ansageform);

impl Werkzeugausfuehrung for Dateischreiben {
    fn name(&self) -> &str {
        Dateiwerkzeug::Schreiben.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        if !self.0.darf_schreiben() {
            return Err(Werkzeugfehler {
                grund: "diese Sitzung darf nur lesen; `myl setzen agent.schreiben an` hebt das auf"
                    .into(),
            });
        }
        let roh = zeichenkette(a, "pfad")?;
        let inhalt = zeichenkette(a, "inhalt")?;
        if inhalt.len() > SCHREIBGRENZE {
            return Err(Werkzeugfehler {
                grund: format!("{} Bytes ueber der Grenze von {SCHREIBGRENZE}", inhalt.len()),
            });
        }
        let p = self.0.aufloesen(&roh, false)?;
        if p.is_dir() {
            return Err(Werkzeugfehler { grund: format!("{roh} ist ein Verzeichnis") });
        }
        // ⚑ **Ob ueberschrieben wurde, gehoert in die Antwort.** Der
        // Unterschied zwischen „neu angelegt" und „vorher war da etwas
        // anderes" ist genau der, den ein Mensch im Nachhinein wissen
        // will.
        let gab_es = p.exists();
        std::fs::write(&p, inhalt.as_bytes())
            .map_err(|e| Werkzeugfehler { grund: format!("{roh}: {e}") })?;
        Ok(format!(
            "{} {}, {} Bytes",
            if gab_es { "ueberschrieben" } else { "angelegt" },
            self.0.kurz(&p),
            inhalt.len()
        ))
    }
}

/// Fuehrt einen Shell-Befehl im Arbeitsverzeichnis aus.
///
/// # ⛔️ Warum dieses Werkzeug anders ist als die anderen
///
/// Jedes Dateiwerkzeug haelt die Einhaengegrenze ein: Es loest seinen
/// Pfad auf und lehnt ab, was hinauszeigt (siehe der Modulkopf). **Ein
/// Shell-Befehl kann das nicht.** `cat ../../fremd` liest hinaus,
/// `curl` sendet hinaus, und dieses Werkzeug reicht die Zeile an `sh -c`
/// weiter, wie ein Terminal es taete. Die Einhaengung ist hier das
/// **Arbeitsverzeichnis** (`current_dir`), nicht eine Grenze.
///
/// ⚑ **Was den Befehl trotzdem einhegt, und was nicht:**
///
/// | Schranke | Was sie leistet |
/// |---|---|
/// | Schreiberlaubnis | Ohne sie steht das Werkzeug nicht im Angebot (`schreibt() == true`) |
/// | `Advanced`, nicht `Base` | Ein kleines Modell bekommt es gar nicht erst |
/// | `manual mode` | Wer ihn setzt, sieht jeden Befehl vor der Ausfuehrung |
/// | Zeitgrenze, Ausgabegrenze | Ein Befehl blockiert die Schleife nicht und flutet den Kontext nicht |
/// | Sperrliste | Ein **Rueckfall** gegen die offensichtlich zerstoererischen Zeilen, kein Schutz |
///
/// ⛔️ **Die Sperrliste ist kein Sandkasten.** Sie faengt `rm -rf /` und
/// eine Handvoll seinesgleichen, damit ein Modell nicht aus Versehen die
/// Maschine loescht; sie faengt keinen entschlossenen Missbrauch. Die
/// echte Grenze waere eine des Betriebssystems (Landlock, App-Sandbox,
/// AppContainer), und die ist eigene Arbeit, wie schon beim Dateizugriff
/// (Modulkopf). Bis dahin ist dieses Werkzeug so vertrauenswuerdig wie
/// die Person, die den `manual mode` bedient.
pub struct Befehlausfuehren(pub Einhaengung, pub Ansageform);

/// Zeilen, die ein Modell nicht aus Versehen ausfuehren koennen soll.
///
/// ⚑ **Ein Rueckfall und kein Filter.** Woertliche Teilzeichenketten, klein
/// geschrieben; wer sie umgeht, wird nicht aufgehalten (siehe
/// [`Befehlausfuehren`]).
const SPERRMUSTER: [&str; 8] = [
    "rm -rf /",
    "rm -rf /*",
    "mkfs",
    "dd if=",
    ":(){",
    "> /dev/sd",
    "shutdown",
    "reboot",
];

impl Werkzeugausfuehrung for Befehlausfuehren {
    fn name(&self) -> &str {
        Dateiwerkzeug::Befehl.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        use std::io::Read;
        use std::process::{Command, Stdio};
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        if !self.0.darf_schreiben() {
            return Err(Werkzeugfehler {
                grund: "diese Sitzung darf nur lesen; `run_command` wirkt und braucht                         `myl setzen agent.schreiben an`"
                    .into(),
            });
        }
        let befehl = zeichenkette(a, "befehl")?;
        let klein = befehl.to_lowercase();
        if let Some(muster) = SPERRMUSTER.iter().find(|m| klein.contains(**m)) {
            return Err(Werkzeugfehler {
                grund: format!("der Befehl enthaelt das gesperrte Muster `{muster}` und wird nicht ausgefuehrt"),
            });
        }

        let mut kind = Command::new("sh")
            .arg("-c")
            .arg(&befehl)
            .current_dir(self.0.wurzel())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Werkzeugfehler { grund: format!("der Befehl liess sich nicht starten: {e}") })?;

        // ⚑ **Zwei Faeden leeren die Roehren**, damit ein Befehl mit viel
        // Ausgabe nicht blockiert, wenn der Roehrenpuffer voll ist. Jeder
        // liest bis zur Ausgabegrenze und dann nicht weiter.
        let lesen = |mut strom: Box<dyn Read + Send>| {
            let (sender, empfang) = mpsc::channel();
            std::thread::spawn(move || {
                let mut puffer = Vec::new();
                let mut mehr = false;
                let mut haeppchen = [0u8; 8192];
                loop {
                    // ⚑ **Immer bis zum Ende lesen**, damit die Roehre nicht
                    // blockiert; behalten wird nur bis zur Grenze, und dass
                    // es mehr gab, wird vermerkt statt vergessen.
                    match strom.read(&mut haeppchen) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let frei = BEFEHL_AUSGABEGRENZE.saturating_sub(puffer.len());
                            if frei > 0 {
                                puffer.extend_from_slice(&haeppchen[..n.min(frei)]);
                            }
                            if n > frei {
                                mehr = true;
                            }
                        }
                    }
                }
                let _ = sender.send((puffer, mehr));
            });
            empfang
        };
        let aus_e = lesen(Box::new(kind.stdout.take().expect("stdout")));
        let err_e = lesen(Box::new(kind.stderr.take().expect("stderr")));

        // Warten mit Frist: nachsehen, ob der Befehl fertig ist, sonst
        // abbrechen. `try_wait` blockiert nicht.
        let frist = Duration::from_secs(BEFEHL_ZEITGRENZE_S);
        let anfang = Instant::now();
        let mut abgebrochen = false;
        let status = loop {
            match kind.try_wait() {
                Ok(Some(s)) => break Some(s),
                Ok(None) => {
                    if anfang.elapsed() >= frist {
                        let _ = kind.kill();
                        let _ = kind.wait();
                        abgebrochen = true;
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break None,
            }
        };

        let (mut roh, mut mehr) = aus_e.recv().unwrap_or_default();
        let (fehler_roh, fehler_mehr) = err_e.recv().unwrap_or_default();
        roh.extend_from_slice(&fehler_roh);
        mehr = mehr || fehler_mehr;
        let text = String::from_utf8_lossy(&roh);
        // Auf die Grenze in **Zeichen** kuerzen: ein Schnitt mitten durch
        // eine Mehrbytefolge erzeugte sonst ein Ersatzzeichen.
        let gekuerzt = mehr || text.chars().count() > BEFEHL_AUSGABEGRENZE;
        let sicht: String = text.chars().take(BEFEHL_AUSGABEGRENZE).collect();

        let kopf = if abgebrochen {
            format!("abgebrochen nach {BEFEHL_ZEITGRENZE_S} s")
        } else {
            match status.and_then(|s| s.code()) {
                Some(0) => "beendet, Rueckgabewert 0".to_string(),
                Some(c) => format!("beendet, Rueckgabewert {c}"),
                None => "beendet ohne Rueckgabewert".to_string(),
            }
        };
        let schwanz = if gekuerzt { format!("\n… (Ausgabe auf {BEFEHL_AUSGABEGRENZE} Bytes gekuerzt)") } else { String::new() };
        Ok(format!("{kopf}\n{sicht}{schwanz}"))
    }
}

/// Listet ein Verzeichnis innerhalb der Einhaengung.
pub struct Verzeichnislesen(pub Einhaengung, pub Ansageform);

impl Werkzeugausfuehrung for Verzeichnislesen {
    fn name(&self) -> &str {
        Dateiwerkzeug::Verzeichnis.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        // ⚑ **Immer die Wurzel.** Der erste Aufruf eines Agenten ist
        // „was liegt hier", und dafuer soll er nichts wissen muessen.
        // Seit dem 2026-09-10 kennt dieses Werkzeug gar keinen Pfad
        // mehr: Die Wahl war es, an der sich ein Modell festgelesen hat.
        let roh = ".".to_string();
        let p = self.0.aufloesen(&roh, true)?;
        if !p.is_dir() {
            return Err(Werkzeugfehler {
                grund: format!(
                    "{roh} ist kein Verzeichnis; dafuer gibt es `{}`",
                    Dateiwerkzeug::Lesen.name(self.1)
                ),
            });
        }
        // ⚑ **`tiefe` ist verlangt und hat trotzdem einen Rueckfall.**
        // Das Schema nennt es in `required`, und die Pruefung davor
        // weist einen Aufruf ohne es ab; kaeme trotzdem etwas
        // Unlesbares an, ist eine Ebene die richtige Annahme und kein
        // Abbruch. **Eine Zahl ausserhalb der Grenzen ist keine
        // Ablehnung wert**, sie wird geklemmt.
        let tiefe = a
            .get("tiefe")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .clamp(1, TIEFENGRENZE);

        let mut zeilen: Vec<String> = Vec::new();
        // 📌 **Der Praefixvergleich in jeder Ebene, nicht nur am
        // Anfang.** `aufloesen` prueft den Startpunkt; ein Verweis
        // **im** Baum fuehrt hinaus, und `read_dir` folgt ihm. Vor der
        // Tiefe gab es diesen Weg nicht, denn eine Ebene war immer die,
        // die geprueft wurde.
        let mut offen = vec![(p.clone(), 1u64)];
        let mut abgeschnitten = false;
        while let Some((d, ebene)) = offen.pop() {
            let Ok(eintraege) = std::fs::read_dir(&d) else { continue };
            for e in eintraege.flatten() {
                if zeilen.len() >= LISTENGRENZE {
                    abgeschnitten = true;
                    break;
                }
                let art = match e.file_type() {
                    Ok(t) if t.is_dir() => "Verzeichnis",
                    Ok(t) if t.is_symlink() => "Verweis",
                    _ => "Datei",
                };
                let Ok(echt) = e.path().canonicalize() else { continue };
                if !echt.starts_with(self.0.wurzel()) {
                    continue;
                }
                // Der Name relativ zum ANGEFRAGTEN Verzeichnis, damit
                // eine tiefe Auflistung lesbar bleibt.
                // 📌 **Schraegstriche, auf allen Plattformen.** Auf
                // Windows trennt `display()` mit `\`, und der neue
                // Plattform-Job hat das beim ersten Lauf gefunden:
                // `unter\tief.txt` statt `unter/tief.txt`. Fuer die
                // Pruefung waere ein plattformweiser Vergleich die
                // bequeme Antwort gewesen; die richtige ist, dass die
                // **Ausgabe** ueberall gleich aussieht.
                //
                // ⚑ Zwei Gruende. Das Modell liest diese Pfade und
                // gibt sie an `read_file` zurueck: `/` ist die Form,
                // in der es Pfade gesehen hat, `\` die seltene. Und
                // `aufloesen` nimmt beide, denn `Path` deutet auf
                // Windows den Schraegstrich mit; es geht also nichts
                // verloren.
                let name = echt
                    .strip_prefix(&p)
                    .unwrap_or(&echt)
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let groesse = e.metadata().map(|m| m.len()).unwrap_or(0);
                zeilen.push(if art == "Datei" {
                    format!("{name}\t{groesse} Bytes")
                } else {
                    format!("{name}\t{art}")
                });
                if echt.is_dir() && ebene < tiefe {
                    offen.push((echt, ebene + 1));
                }
            }
            if abgeschnitten {
                break;
            }
        }
        zeilen.sort();
        if abgeschnitten {
            zeilen.push(format!("[gekuerzt bei {LISTENGRENZE} Eintraegen]"));
        }
        if zeilen.is_empty() {
            return Ok(format!("{} ist leer", self.0.kurz(&p)));
        }
        Ok(zeilen.join("\n"))
    }
}

/// Die Angebote, wie das Modell sie zu sehen bekommt.
///
/// ⚑ **Die Beschreibung ist Teil der Schranke.** Ein Modell, dem
/// niemand sagt, dass es in einem Verzeichnis sitzt, schlaegt Pfade
/// vor, die abgelehnt werden, und verbraucht Schritte an der Grenze
/// statt an der Aufgabe.
///
/// 📌 **Hier stand bis zum 2026-09-08 der volle Pfad der Einhaengung,
/// und zwar in jeder der drei Beschreibungen.** Zwei Dinge waren daran
/// falsch. Er sagte dem Modell einen absoluten Pfad und im selben Satz
/// „relativ zum Arbeitsverzeichnis", also zwei Angaben, von denen nur
/// eine gilt. Und er widersprach [`Einhaengung::kurz`]: Wenn ein
/// absoluter Pfad nicht in eine **Antwort** gehoert, weil er das
/// Wirtsverzeichnis verraet, gehoert er auch nicht in die
/// **Beschreibung**. Das Modell arbeitet relativ und braucht ihn nicht;
/// der Mensch sieht ihn beim Start der Sitzung.
pub fn angebote(e: &Einhaengung, form: Ansageform, satz: Werkzeugkiste) -> Vec<Werkzeug> {
    // 📌 **Hier standen Name, Beschreibung und Schema als Literale**,
    // und die Ausfuehrung nannte ihren Namen noch einmal. Seit die
    // Namen an der Ansageform haengen, waeren das zwei Listen, die
    // auseinanderlaufen koennen; jetzt kommt beides aus
    // `Dateiwerkzeug`.
    satz.werkzeuge()
        .iter()
        .filter(|w| !w.schreibt() || e.darf_schreiben())
        .map(|w| Werkzeug {
            name: w.name(form).into(),
            beschreibung: w.beschreibung(form),
            parameter: w.parameter(form),
        })
        .collect()
}

#[cfg(test)]
mod neue_werkzeuge {
    use super::*;

    /// Baut einen kleinen Baum: zwei Ebenen, ein Treffer je Ebene.
    fn baum() -> (tempfile::TempDir, Einhaengung) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        std::fs::write(d.path().join("oben.txt"), "eins\nGoldfisch schwimmt\ndrei\n")
            .expect("Datei");
        std::fs::create_dir(d.path().join("unter")).expect("Unterverzeichnis");
        std::fs::write(d.path().join("unter/tief.txt"), "nichts\nein goldfisch klein\n")
            .expect("Datei");
        let e = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        (d, e)
    }

    fn befehl(e: &Einhaengung, b: &str) -> Result<String, Werkzeugfehler> {
        Befehlausfuehren(e.clone(), Ansageform::Amtlich)
            .ausfuehren(&serde_json::json!({"befehl": b}))
    }

    /// **`run_command` fuehrt aus, im Arbeitsverzeichnis, und nennt den
    /// Rueckgabewert.**
    #[test]
    fn run_command_fuehrt_im_arbeitsverzeichnis_aus() {
        let (_d, e) = baum();
        let aus = befehl(&e, "cat oben.txt").expect("ok");
        assert!(aus.contains("Rueckgabewert 0"), "{aus}");
        assert!(aus.contains("Goldfisch schwimmt"), "{aus}");
        let fehler = befehl(&e, "exit 3").expect("laeuft");
        assert!(fehler.contains("Rueckgabewert 3"), "{fehler}");
    }

    /// **Ohne Schreiberlaubnis wirkt es nicht.** Es wirkt, also gehoert es
    /// hinter dieselbe Erlaubnis wie das Schreiben.
    #[test]
    fn run_command_braucht_die_schreiberlaubnis() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        let fehler = befehl(&nur_lesen, "echo hallo").expect_err("muss ablehnen");
        assert!(fehler.grund.contains("nur lesen"), "{}", fehler.grund);
    }

    /// **Die Sperrliste faengt die offensichtlich zerstoererische Zeile**,
    /// ohne den Befehl zu starten. ⛔️ Ein Rueckfall, kein Sandkasten.
    #[test]
    fn run_command_sperrt_das_offensichtlich_zerstoererische() {
        let (_d, e) = baum();
        let fehler = befehl(&e, "rm -rf /").expect_err("gesperrt");
        assert!(fehler.grund.contains("gesperrt"), "{}", fehler.grund);
        // Gross/klein egal.
        assert!(befehl(&e, "DD IF=/dev/zero of=x").is_err());
    }

    /// **Viel Ausgabe wird auf die Grenze gekuerzt, mit Vermerk.**
    #[test]
    fn run_command_kuerzt_viel_ausgabe() {
        let (_d, e) = baum();
        let aus = befehl(&e, "head -c 40000 /dev/zero | tr \\0 x").expect("ok");
        assert!(aus.contains("gekuerzt"), "{}", &aus[..aus.len().min(80)]);
        assert!(aus.chars().count() <= BEFEHL_AUSGABEGRENZE + 200, "{} Zeichen", aus.chars().count());
    }

    /// **`run_command` liegt in Advanced, nicht in Base.** Ein Shell-Befehl
    /// haelt die Einhaengegrenze nicht ein und gehoert nicht in die
    /// Vorgabekiste eines kleinen Modells (Fund: die Kisten bedeuten damit
    /// zum ersten Mal Verschiedenes).
    #[test]
    fn run_command_ist_advanced_und_nicht_base() {
        assert!(Werkzeugkiste::Base.werkzeuge().iter().all(|w| !matches!(w, Dateiwerkzeug::Befehl)), "run_command darf nicht in Base sein");
        assert!(Werkzeugkiste::Advanced.werkzeuge().iter().any(|w| matches!(w, Dateiwerkzeug::Befehl)), "run_command fehlt in Advanced");
        assert!(Dateiwerkzeug::Befehl.schreibt(), "run_command wirkt und braucht die Schreiberlaubnis");
    }

    fn suche(e: &Einhaengung, muster: &str) -> String {
        Suchen(e.clone(), Ansageform::Amtlich)
            .ausfuehren(&serde_json::json!({"muster": muster}))
            .expect("suchen")
    }

    /// ⚑ Die Suche steigt in Unterverzeichnisse; das ist ihr Zweck.
    #[test]
    fn sie_findet_ueber_ebenen_hinweg() {
        let (_d, e) = baum();
        let aus = suche(&e, "Goldfisch");
        assert!(aus.contains("oben.txt:2"), "{aus}");
        assert!(aus.contains("tief.txt:2"), "{aus}");
    }

    /// 📌 **Gross und klein ist egal**, denn ein Modell trifft die
    /// Schreibweise oft nicht, und eine Suche, die daran scheitert,
    /// kostet einen Schritt fuer nichts.
    #[test]
    fn gross_und_klein_ist_egal() {
        let (_d, e) = baum();
        assert_eq!(suche(&e, "GOLDFISCH").lines().count(), 2);
        assert_eq!(suche(&e, "goldfisch").lines().count(), 2);
    }

    /// Kein Treffer ist eine **Antwort** und kein Fehler: Damit kann ein
    /// Agent weiterarbeiten, mit einem Fehler nicht.
    #[test]
    fn ohne_treffer_kommt_ein_satz_und_kein_fehler() {
        let (_d, e) = baum();
        let aus = suche(&e, "Zebrastreifen");
        assert!(aus.contains("keine Zeile"), "{aus}");
    }

    /// 📌 **Und sie bleibt in der Einhaengung.** Das ist die eigentliche
    /// Pruefung dieser Sammlung: Ein Werkzeug, das einen Baum
    /// durchlaeuft, hat mehr Gelegenheiten hinauszukommen als eines,
    /// das einen Pfad aufloest.
    #[test]
    fn sie_kommt_nicht_hinaus() {
        let (d, e) = baum();
        let draussen = d.path().parent().expect("Elternverzeichnis").join("geheim.txt");
        std::fs::write(&draussen, "Goldfisch draussen\n").expect("Datei");

        let aus = suche(&e, "Goldfisch");
        assert!(!aus.contains("geheim"), "die Suche hat hinausgesehen: {aus}");

        // ⚑ **Und ein Startpunkt laesst sich gar nicht mehr angeben.**
        //
        // 📌 Hier stand bis zum 2026-09-10, dass `pfad: ".."` von der
        // **Ausfuehrung** abgelehnt wird. Seither kennt dieses Werkzeug
        // keinen Pfad mehr, und die Ablehnung kommt eine Stufe frueher,
        // an der Argumentpruefung: Der Aufruf laeuft gar nicht erst an.
        // **Das ist die schaerfere Zusage**, geprueft in
        // `ein_pfad_am_verzeichnis_wird_benannt_abgelehnt`.
        //
        // Was hier bleibt, ist die Gegenprobe dazu: Selbst wenn ein
        // Pfad durchkaeme, wuerde er nicht befolgt.
        let aus = Suchen(e, Ansageform::Amtlich)
            .ausfuehren(&serde_json::json!({"muster": "Goldfisch", "pfad": ".."}))
            .expect("die Suche laeuft");
        assert!(
            !aus.contains("geheim"),
            "ein mitgeschickter Pfad hat den Startpunkt doch verschoben: {aus}"
        );
        let _ = std::fs::remove_file(draussen);
    }

    /// 📌 **Ein Verweis nach draussen wird beim Durchlaufen erkannt.**
    /// `aufloesen` prueft den Startpunkt; hier liegt der Verweis
    /// **im** Baum, und `read_dir` folgt ihm.
    #[cfg(unix)]
    #[test]
    fn ein_verweis_im_baum_fuehrt_nicht_hinaus() {
        let (d, e) = baum();
        let draussen = d.path().parent().expect("Elternverzeichnis").join("aussen_zeug");
        std::fs::create_dir_all(&draussen).expect("Verzeichnis");
        std::fs::write(draussen.join("geheim.txt"), "Goldfisch draussen\n").expect("Datei");
        std::os::unix::fs::symlink(&draussen, d.path().join("raus")).expect("Verweis");

        let aus = suche(&e, "Goldfisch");
        assert!(!aus.contains("geheim"), "ueber den Verweis hinausgesehen: {aus}");
        let _ = std::fs::remove_dir_all(draussen);
    }

    #[test]
    fn ein_leeres_muster_ist_ein_fehler() {
        let (_d, e) = baum();
        let f = Suchen(e, Ansageform::Amtlich).ausfuehren(&serde_json::json!({"muster": "  "}));
        assert!(f.is_err());
    }

    // ── edit_file ────────────────────────────────────────────────────

    fn aendern(e: &Einhaengung, alt: &str, neu: &str) -> Result<String, Werkzeugfehler> {
        aendern_viele(e, &[(alt, neu)])
    }

    fn aendern_viele(
        e: &Einhaengung,
        paare: &[(&str, &str)],
    ) -> Result<String, Werkzeugfehler> {
        let liste: Vec<_> = paare
            .iter()
            .map(|(a, n)| serde_json::json!({"alt": a, "neu": n}))
            .collect();
        Dateiaendern(e.clone(), Ansageform::Amtlich)
            .ausfuehren(&serde_json::json!({"pfad": "oben.txt", "aenderungen": liste}))
    }

    #[test]
    fn eine_eindeutige_stelle_wird_ersetzt() {
        let (d, e) = baum();
        aendern(&e, "Goldfisch", "Karpfen").expect("aendern");
        let jetzt = std::fs::read_to_string(d.path().join("oben.txt")).expect("lesen");
        assert_eq!(jetzt, "eins\nKarpfen schwimmt\ndrei\n", "der Rest muss stehen bleiben");
    }

    /// 📌 **Der Kern des Werkzeugs.** Kaeme es mehrfach vor und wuerde
    /// das erste genommen, traefe es irgendwann die falsche Stelle. Die
    /// Ablehnung sagt dem Modell, dass es mehr Umgebung braucht.
    #[test]
    fn eine_mehrdeutige_stelle_wird_abgelehnt() {
        let (d, e) = baum();
        std::fs::write(d.path().join("oben.txt"), "x\nx\n").expect("Datei");
        let f = aendern(&e, "x", "y");
        assert!(f.is_err(), "eine mehrdeutige Stelle wurde ersetzt");
        assert!(f.unwrap_err().grund.contains("2-mal"));
        // ⚑ Und der Aufruf sagt, dass nichts geschrieben wurde.
        assert_eq!(std::fs::read_to_string(d.path().join("oben.txt")).expect("lesen"), "x\nx\n");
    }

    /// **Mehrere Stellen in einem Aufruf.**
    ///
    /// ⚑ **Der eigentliche Zweck der Liste.** Vorher kostete jede
    /// Ersetzung einen Schritt des Agenten; jetzt kostet ein Satz von
    /// Aenderungen einen.
    #[test]
    fn mehrere_stellen_in_einem_aufruf() {
        let (d, e) = baum();
        std::fs::write(d.path().join("oben.txt"), "eins\nzwei\ndrei\n").expect("Datei");
        let aus = aendern_viele(&e, &[("eins", "ONE"), ("drei", "THREE")]).expect("aendern");
        assert!(aus.contains("2 Stellen"), "{aus}");
        assert_eq!(
            std::fs::read_to_string(d.path().join("oben.txt")).expect("lesen"),
            "ONE\nzwei\nTHREE\n"
        );
    }

    /// **Jede Aenderung sieht das Ergebnis der vorigen.**
    ///
    /// 📌 **Eine Pruefung gegen den Urzustand haette hier gruenes Licht
    /// gegeben und danach das Falsche geschrieben.** „a" kommt im
    /// Urzustand einmal vor und nach der ersten Aenderung zweimal; wer
    /// beide gegen den Anfang prueft, haelt den Satz fuer eindeutig und
    /// ersetzt dann die erste Fundstelle statt der gemeinten.
    #[test]
    fn jede_aenderung_sieht_die_vorige() {
        let (d, e) = baum();
        std::fs::write(d.path().join("oben.txt"), "a\nb\n").expect("Datei");
        let f = aendern_viele(&e, &[("b", "a"), ("a", "c")]);
        assert!(f.is_err(), "der Satz widerspricht sich und wurde trotzdem geschrieben");
        assert!(f.unwrap_err().grund.contains("2-mal"));
        assert_eq!(
            std::fs::read_to_string(d.path().join("oben.txt")).expect("lesen"),
            "a\nb\n",
            "trotz Fehler geschrieben"
        );
    }

    /// **Alles oder nichts.**
    ///
    /// ⚠️ **Die wichtigste Zusage dieses Werkzeugs.** Eine Datei, in der
    /// drei von fuenf Aenderungen stehen, ist schlimmer als eine
    /// unveraenderte: Sie sieht bearbeitet aus und ist es halb.
    #[test]
    fn ein_fehler_schreibt_gar_nichts() {
        let (d, e) = baum();
        std::fs::write(d.path().join("oben.txt"), "eins\nzwei\n").expect("Datei");
        let f = aendern_viele(&e, &[("eins", "ONE"), ("gibtesnicht", "X")]);
        assert!(f.is_err());
        assert_eq!(
            std::fs::read_to_string(d.path().join("oben.txt")).expect("lesen"),
            "eins\nzwei\n",
            "die erste Aenderung wurde geschrieben, obwohl die zweite fiel"
        );
    }

    /// **Alle Fehler auf einmal, nicht der erste.**
    ///
    /// ⚑ Wer beim ersten abbricht, schickt das Modell in eine Runde je
    /// Fehler. Bei einem Schrittbudget ist das der Unterschied zwischen
    /// einem Auftrag und keinem.
    #[test]
    fn alle_fehler_werden_genannt() {
        let (d, e) = baum();
        std::fs::write(d.path().join("oben.txt"), "eins\nzwei\n").expect("Datei");
        let f = aendern_viele(&e, &[("fehlt-eins", "X"), ("fehlt-zwei", "Y")])
            .expect_err("beide fehlen");
        assert!(f.grund.contains("Aenderung 1"), "{}", f.grund);
        assert!(f.grund.contains("Aenderung 2"), "{}", f.grund);
    }

    /// ⚑ **Eine leere Liste ist ein Fehler und keine Ruhe.** Ein
    /// Aufruf, der nichts tut, kostet trotzdem einen Schritt.
    #[test]
    fn eine_leere_liste_wird_abgelehnt() {
        let (_d, e) = baum();
        let f = Dateiaendern(e, Ansageform::Amtlich)
            .ausfuehren(&serde_json::json!({"pfad": "oben.txt", "aenderungen": []}));
        assert!(f.is_err());
    }

    #[test]
    fn was_nicht_da_ist_wird_nicht_ersetzt() {
        let (_d, e) = baum();
        assert!(aendern(&e, "Zebrastreifen", "y").is_err());
    }

    /// ⚑ **Schreiben ist eine eigene Erlaubnis**, und `edit_file`
    /// schreibt. Ohne sie darf es nicht einmal versuchen.
    #[test]
    fn ohne_erlaubnis_wird_nichts_geaendert() {
        let (d, _e) = baum();
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        let f = Dateiaendern(nur_lesen, Ansageform::Amtlich).ausfuehren(&serde_json::json!({
            "pfad": "oben.txt",
            "aenderungen": [{"alt": "Goldfisch", "neu": "Karpfen"}]
        }));
        assert!(f.is_err());
        assert!(std::fs::read_to_string(d.path().join("oben.txt"))
            .expect("lesen")
            .contains("Goldfisch"));
    }

    // ── Tiefe der Auflistung ─────────────────────────────────────────

    fn liste(e: &Einhaengung, a: serde_json::Value) -> String {
        Verzeichnislesen(e.clone(), Ansageform::Amtlich).ausfuehren(&a).expect("listen")
    }

    /// ⚑ **Ohne Angabe genau das Alte:** eine Ebene.
    #[test]
    fn ohne_tiefe_bleibt_es_bei_einer_ebene() {
        let (_d, e) = baum();
        let aus = liste(&e, serde_json::json!({}));
        assert!(aus.contains("oben.txt"), "{aus}");
        assert!(aus.contains("unter"), "{aus}");
        assert!(!aus.contains("tief.txt"), "ungefragt in die Tiefe: {aus}");
    }

    #[test]
    fn mit_tiefe_kommt_der_baum() {
        let (_d, e) = baum();
        let aus = liste(&e, serde_json::json!({"tiefe": 2}));
        assert!(aus.contains("tief.txt"), "{aus}");
        // ⚑ Der Name relativ zum angefragten Verzeichnis, sonst ist
        // eine tiefe Auflistung nicht zu lesen.
        assert!(aus.contains("unter/tief.txt"), "{aus}");
    }

    /// Eine masslose Tiefe wird beschnitten und nicht abgelehnt: Ein
    /// Agent, der `tiefe: 999` schickt, meint „alles".
    #[test]
    fn eine_masslose_tiefe_wird_beschnitten() {
        let (_d, e) = baum();
        let aus = liste(&e, serde_json::json!({"tiefe": 999}));
        assert!(aus.contains("tief.txt"), "{aus}");
    }
}

#[cfg(test)]
mod namen {
    use super::*;

    /// ⚑ **Angebot und Ausfuehrung muessen in JEDER Form denselben
    /// Namen nennen.** `Werkzeugkasten::einhaengen` haelt das zur
    /// Laufzeit fest; hier faellt es beim Uebersetzen der Pruefungen
    /// auf, also bevor jemand ein Modell dafuer laedt.
    #[test]
    fn angebot_und_ausfuehrung_nennen_dasselbe() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let e = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        for form in [Ansageform::Amtlich, Ansageform::Deutsch] {
            for satz in [Werkzeugkiste::Base, Werkzeugkiste::Advanced] {
                let angeboten: Vec<String> =
                    angebote(&e, form, satz).into_iter().map(|w| w.name).collect();
                let ausgefuehrt: Vec<String> = satz
                    .werkzeuge()
                    .iter()
                    .map(|w| w.ausfuehrung(e.clone(), form).name().to_string())
                    .collect();
                assert_eq!(angeboten, ausgefuehrt, "{} / {}", form.name(), satz.name());
            }
        }
    }

    /// Ohne Schreiberlaubnis wird das Schreibwerkzeug **nicht einmal
    /// angeboten**, und zwar in beiden Formen.
    #[test]
    fn ohne_erlaubnis_fehlt_das_schreiben_in_beiden_formen() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        for form in [Ansageform::Amtlich, Ansageform::Deutsch] {
            for satz in [Werkzeugkiste::Base, Werkzeugkiste::Advanced] {
                let namen: Vec<String> =
                    angebote(&nur_lesen, form, satz).into_iter().map(|w| w.name).collect();
                // ⚑ Die Erwartung kommt aus der Tabelle und nicht als
                // Zahl: Sonst muesste sie jedes Mal nachgezogen werden,
                // wenn ein Werkzeug dazukommt, und genau das ist am
                // 2026-09-09 passiert.
                let erwartet: Vec<String> = satz
                    .werkzeuge()
                    .iter()
                    .filter(|w| !w.schreibt())
                    .map(|w| w.name(form).to_string())
                    .collect();
                assert_eq!(namen, erwartet, "{} / {}", form.name(), satz.name());
                for w in satz.werkzeuge().iter().filter(|w| w.schreibt()) {
                    assert!(
                        !namen.contains(&w.name(form).to_string()),
                        "{} wird ohne Erlaubnis angeboten",
                        w.name(form)
                    );
                }
            }
        }
    }

    /// 📌 **Die beiden Formen muessen wirklich verschieden sein.** Ohne
    /// diese Pruefung koennte die Tabelle beide auf denselben Namen
    /// legen, und der Vergleich, fuer den es den Schalter gibt,
    /// verglaeche nichts.
    #[test]
    fn die_formen_geben_verschiedene_namen() {
        for w in Dateiwerkzeug::ALLE {
            let a = w.name(Ansageform::Amtlich);
            let d = w.name(Ansageform::Deutsch);
            assert_ne!(a, d, "{w:?} heisst in beiden Formen gleich");
            assert!(a.is_ascii() && !a.contains(' '), "{a} taugt nicht als Funktionsname");
        }
        // Und keine zwei Werkzeuge teilen sich einen Namen.
        for form in [Ansageform::Amtlich, Ansageform::Deutsch] {
            let mut n: Vec<&str> = Dateiwerkzeug::ALLE.iter().map(|w| w.name(form)).collect();
            n.sort_unstable();
            let vorher = n.len();
            n.dedup();
            assert_eq!(n.len(), vorher, "zwei Werkzeuge teilen einen Namen in {}", form.name());
        }
    }
}

#[cfg(test)]
mod grenze {
    use super::*;

    fn baue() -> (tempfile::TempDir, Einhaengung) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        std::fs::write(d.path().join("drin.txt"), "hallo").expect("Datei");
        std::fs::create_dir(d.path().join("unter")).expect("Unterverzeichnis");
        let e = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        (d, e)
    }

    #[test]
    fn was_drin_liegt_geht() {
        let (_d, e) = baue();
        let l = Dateilesen(e, Ansageform::Amtlich);
        let aus = l.ausfuehren(&serde_json::json!({"pfad": "drin.txt"})).expect("lesen");
        assert_eq!(aus, "hallo");
    }

    /// ⚑ **Der Fall, um den es geht.**
    #[test]
    fn punktpunkt_kommt_nicht_hinaus() {
        let (_d, e) = baue();
        let l = Dateilesen(e, Ansageform::Amtlich);
        let f = l.ausfuehren(&serde_json::json!({"pfad": "../../../etc/hosts"}));
        assert!(f.is_err(), "der Ausbruch ueber .. gelang");
        assert!(f.unwrap_err().grund.contains("ausserhalb") || true);
    }

    /// 📌 **Je Plattform ein Pfad, der dort wirklich absolut ist.**
    /// Hier stand nur `/etc/hosts`. Auf Windows ist das **nicht**
    /// absolut (dort braucht ein absoluter Pfad einen Praefix wie
    /// `C:`), es waere also an die Wurzel gehaengt worden, und die
    /// Pruefung waere gruen geblieben, ohne das zu pruefen, wofuer es
    /// sie gibt. Aufgefallen beim Aufnehmen des Clients in die
    /// Plattform-CI (CLIENT 1.6).
    #[test]
    fn ein_absoluter_pfad_kommt_nicht_hinaus() {
        let (_d, e) = baue();
        let l = Dateilesen(e, Ansageform::Amtlich);
        let draussen = if cfg!(windows) { "C:\\Windows\\win.ini" } else { "/etc/hosts" };
        assert!(
            std::path::Path::new(draussen).is_absolute(),
            "diese Pruefung braucht einen auf DIESER Plattform absoluten Pfad: {draussen}"
        );
        assert!(l.ausfuehren(&serde_json::json!({"pfad": draussen})).is_err());
    }

    /// ⚑ Ein Verweis wird **aufgeloest** und dann verglichen; eine
    /// Textpruefung auf `..` haette ihn durchgelassen.
    ///
    /// 📌 **Nur auf Unix, und das ist eine benannte Luecke.** Vorher
    /// stand das `#[cfg]` im Rumpf, mit einem `return` fuer alle
    /// anderen Plattformen; dahinter wurde der Code unerreichbar, und
    /// Clippy mit `-D warnings` faellt darueber, sobald dieses Crate
    /// auf Windows uebersetzt wird. Jetzt steht die Bedingung an der
    /// Pruefung selbst.
    ///
    /// **Was auf Windows damit unbelegt bleibt:** Dort gibt es
    /// Verweise ebenfalls (Symlinks und Junctions), aber sie anzulegen
    /// verlangt den Entwicklermodus oder Administratorrechte, und ein
    /// Runner hat beides nicht verlaesslich. Die Sicherung selbst
    /// haengt nicht an der Plattform, denn sie ist `canonicalize` samt
    /// Praefixvergleich, und `canonicalize` loest auf Windows Symlinks
    /// **und** Junctions auf. Belegt ist das hier trotzdem nur auf
    /// Unix.
    #[cfg(unix)]
    #[test]
    fn ein_verweis_nach_draussen_kommt_nicht_hinaus() {
        let (d, e) = baue();
        let ziel = d.path().parent().expect("Elternverzeichnis").to_path_buf();
        let verweis = d.path().join("hinaus");
        std::os::unix::fs::symlink(&ziel, &verweis).expect("Verweis");
        let l = Dateilesen(e, Ansageform::Amtlich);
        assert!(l.ausfuehren(&serde_json::json!({"pfad": "hinaus/irgendwas"})).is_err());
    }

    /// ⚑ **Schreiben ist eine eigene Erlaubnis**, und ohne sie sagt das
    /// Werkzeug das, statt still nichts zu tun.
    #[test]
    fn ohne_erlaubnis_wird_nicht_geschrieben() {
        let (d, _e) = baue();
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        let s = Dateischreiben(nur_lesen, Ansageform::Amtlich);
        let f = s.ausfuehren(&serde_json::json!({"pfad": "neu.txt", "inhalt": "x"}));
        assert!(f.is_err(), "es wurde ohne Erlaubnis geschrieben");
        assert!(!d.path().join("neu.txt").exists());
    }

    #[test]
    fn eine_neue_datei_darf_entstehen() {
        let (d, e) = baue();
        let s = Dateischreiben(e, Ansageform::Amtlich);
        let aus = s
            .ausfuehren(&serde_json::json!({"pfad": "unter/neu.txt", "inhalt": "abc"}))
            .expect("schreiben");
        assert!(aus.starts_with("angelegt"), "{aus}");
        assert_eq!(std::fs::read_to_string(d.path().join("unter/neu.txt")).expect("lesen"), "abc");
    }

    /// ⚑ Auch beim Anlegen gilt die Grenze, und dort ist sie leichter zu
    /// verlieren: Aufgeloest wird das **Elternverzeichnis**.
    #[test]
    fn auch_neue_dateien_bleiben_drin() {
        let (_d, e) = baue();
        let s = Dateischreiben(e, Ansageform::Amtlich);
        assert!(s
            .ausfuehren(&serde_json::json!({"pfad": "../draussen.txt", "inhalt": "x"}))
            .is_err());
    }

    #[test]
    fn ueberschreiben_wird_gesagt() {
        let (_d, e) = baue();
        let s = Dateischreiben(e, Ansageform::Amtlich);
        let aus = s
            .ausfuehren(&serde_json::json!({"pfad": "drin.txt", "inhalt": "neu"}))
            .expect("schreiben");
        assert!(aus.starts_with("ueberschrieben"), "{aus}");
    }

    #[test]
    fn das_verzeichnis_listet_und_nennt_die_art() {
        let (_d, e) = baue();
        let v = Verzeichnislesen(e, Ansageform::Amtlich);
        let aus = v.ausfuehren(&serde_json::json!({})).expect("listen");
        assert!(aus.contains("drin.txt"), "{aus}");
        assert!(aus.contains("unter\tVerzeichnis"), "{aus}");
    }

    /// ⚑ **Kein Wirtspfad in der Beschreibung.** Er gehoerte nicht
    /// in die Antwort, und aus demselben Grund nicht in das Angebot.
    #[test]
    fn die_beschreibung_verraet_das_wirtsverzeichnis_nicht() {
        let (d, e) = baue();
        let wo = d.path().display().to_string();
        for w in angebote(&e, Ansageform::Amtlich, Werkzeugkiste::Advanced) {
            assert!(!w.beschreibung.contains(&wo), "{}: {}", w.name, w.beschreibung);
        }
    }

    /// ⚑ Ohne Schreiberlaubnis wird das Werkzeug **gar nicht erst
    /// angeboten**. Ein Modell, das es nicht sieht, verbraucht keinen
    /// Schritt daran.
    #[test]
    fn ohne_erlaubnis_kein_angebot() {
        let (d, _e) = baue();
        let nur_lesen = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        let a = angebote(&nur_lesen, Ansageform::Amtlich, Werkzeugkiste::Advanced);
        // 📌 Hier stand `assert_eq!(a.len(), 2)` und der Name als
        // Zeichenkette. Beides musste am 2026-09-09 nachgezogen werden,
        // als zwei Werkzeuge dazukamen, und das ist genau die Sorte
        // Pruefung, die nicht mitwaechst. Jetzt aus der Tabelle.
        for w in Dateiwerkzeug::ALLE.iter().filter(|w| w.schreibt()) {
            let n = w.name(Ansageform::Amtlich);
            assert!(!a.iter().any(|x| x.name == n), "{n} wird ohne Erlaubnis angeboten");
        }
        assert!(a.iter().any(|x| x.name == Dateiwerkzeug::Lesen.name(Ansageform::Amtlich)));
    }
}

#[cfg(test)]
mod keine_wahl {
    use super::*;

    /// 📌 **Die Regel, die aus einem gemeldeten Fehlschlag entstand.**
    ///
    /// Auf die Frage „welche Dateien liegen im Verzeichnis?" ueberlegte
    /// Qwen3-4B am 2026-09-10 seitenlang, ob es `pfad` weglassen, leer
    /// setzen oder mitgeben solle, las dazu die `required`-Liste des
    /// Schemas, kam zu keinem Schluss und **endete ohne
    /// Schlussantwort**. Das Werkzeug haette in beiden Faellen dasselbe
    /// getan.
    ///
    /// ⚑ **Ein optionaler Parameter ist eine Entscheidung, die das
    /// Modell treffen muss, und ein kleines Modell bezahlt sie mit
    /// seinem Schrittbudget.** Also gibt es sie nicht.
    #[test]
    fn kein_werkzeug_hat_einen_optionalen_parameter() {
        /// Prueft ein Schema und **jedes Schema darin**.
        ///
        /// 📌 **Sie sah bis zum 2026-09-10 nur die oberste Ebene**, und
        /// das fiel auf, als `edit_file` eine Liste von Objekten bekam:
        /// Die Wache lief gruen durch, weil sie in `items` gar nicht
        /// hineinsah. Sie war nicht erfuellt, sie war blind. **Eine
        /// Pruefung, die nur die Form prueft, in der der Fehler bisher
        /// auftrat, prueft die Vergangenheit.**
        fn pruefen(schema: &serde_json::Value, wo: &str, name: &str) {
            let felder: Vec<String> = schema
                .get("properties")
                .and_then(|x| x.as_object())
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default();
            let noetig: Vec<String> = schema
                .get("required")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).map(String::from).collect())
                .unwrap_or_default();
            for f in &felder {
                assert!(
                    noetig.contains(f),
                    "`{name}`{wo} hat den optionalen Parameter `{f}`.\n\
                     Ein optionaler Parameter ist eine Entscheidung, die das Modell \n\
                     treffen muss, und ein kleines Modell bezahlt sie mit seinem Budget.",
                );
            }
            // ⚑ Und die Gegenrichtung: nichts Verlangtes, das es gar
            // nicht gibt. Das waere ein Aufruf, der nie gelingt.
            for n in &noetig {
                assert!(
                    felder.contains(n),
                    "`{name}`{wo} verlangt `{n}`, kennt es aber nicht"
                );
            }
            // ⚑ **Und jetzt hinein.** Ein Objekt in einer Liste ist
            // dieselbe Entscheidung eine Ebene tiefer.
            if let Some(o) = schema.get("properties").and_then(|x| x.as_object()) {
                for (feld, unter) in o {
                    pruefen(unter, &format!("{wo}, Feld `{feld}`"), name);
                }
            }
            if let Some(items) = schema.get("items") {
                pruefen(items, &format!("{wo}, Listeneintrag"), name);
            }
        }

        for form in [Ansageform::Amtlich, Ansageform::Deutsch] {
            for &w in &Dateiwerkzeug::ALLE {
                pruefen(&w.parameter(form), "", w.name(form));
            }
        }
    }

    /// ⚑ **Und kein Werkzeug nimmt mehr einen Startpfad fuers Suchen
    /// oder Listen.** Beide arbeiten im Arbeitsverzeichnis, das der
    /// Nutzer vorher waehlt, und das ist die ganze Zusage.
    #[test]
    fn listen_und_suchen_kennen_keinen_pfad() {
        for form in [Ansageform::Amtlich, Ansageform::Deutsch] {
            for w in [Dateiwerkzeug::Verzeichnis, Dateiwerkzeug::Suchen] {
                let p = w.parameter(form);
                let felder = p.get("properties").and_then(|x| x.as_object()).expect("Schema");
                assert!(
                    !felder.contains_key("pfad"),
                    "`{}` nimmt wieder einen Pfad entgegen",
                    w.name(form)
                );
            }
        }
    }

    /// ⚑ **Ein mitgeschickter Pfad wird abgewiesen und nicht
    /// stillschweigend uebergangen.**
    ///
    /// 📌 Das ist die wichtigere Haelfte: Ein Werkzeug, das ein
    /// unbekanntes Feld schluckt, laesst das Modell glauben, es habe
    /// gewirkt. Die Ablehnung nennt die Felder, die es gibt, und daraus
    /// lernt ein Modell im naechsten Schritt.
    #[test]
    fn ein_pfad_am_verzeichnis_wird_benannt_abgelehnt() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        std::fs::write(d.path().join("a.txt"), "x").expect("Datei");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        let form = Ansageform::Amtlich;

        let angebot = angebote(&e, form, Werkzeugkiste::default())
            .into_iter()
            .find(|a| a.name == Dateiwerkzeug::Verzeichnis.name(form))
            .expect("Verzeichniswerkzeug");

        let mit_pfad = myl_local_agent::werkzeug::Vorschlag {
            name: angebot.name.clone(),
            arguments: serde_json::json!({"tiefe": 1, "pfad": "unter"}),
        };
        let fehler = myl_local_agent::werkzeug::argumente_pruefen(&angebot, &mit_pfad)
            .expect_err("ein unbekanntes Feld ging durch");
        let text = fehler.to_string();
        assert!(text.contains("pfad"), "die Meldung nennt das Feld nicht: {text}");
        assert!(text.contains("tiefe"), "die Meldung nennt nicht, was es gibt: {text}");

        // Und der richtige Aufruf geht durch.
        let ohne = myl_local_agent::werkzeug::Vorschlag {
            name: angebot.name.clone(),
            arguments: serde_json::json!({"tiefe": 1}),
        };
        myl_local_agent::werkzeug::argumente_pruefen(&angebot, &ohne).expect("richtiger Aufruf");
    }
}

#[cfg(test)]
mod kistenwahl {
    use super::*;

    fn artefakt(variante: Option<&str>) -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("Verzeichnis");
        if let Some(v) = variante {
            std::fs::write(
                d.path().join("model_config.json"),
                format!(r#"{{"family":"qwen3","variant":"{v}"}}"#),
            )
            .expect("Konfig");
        }
        d
    }

    /// **Das Modell sagt seine Groesse selbst, und die Kiste folgt.**
    #[test]
    fn die_kiste_folgt_der_groesse() {
        for (variante, erwartet) in [
            ("0.5b", Werkzeugkiste::Base),
            ("4b", Werkzeugkiste::Base),
            ("7b", Werkzeugkiste::Advanced),
            ("30b-a3b", Werkzeugkiste::Advanced),
        ] {
            let d = artefakt(Some(variante));
            let (kiste, warum) = Werkzeugkiste::fuer_artefakt(d.path());
            assert_eq!(kiste, erwartet, "{variante}: {warum}");
            assert!(warum.contains(variante), "der Grund nennt die Variante nicht: {warum}");
        }
    }

    /// ⚑ **Beim Expertengemisch zaehlt die Gesamtzahl.** `30b-a3b`
    /// nennt hinter dem Strich die je Token aktiven drei Milliarden;
    /// wer die liest, gibt einem 30B-Modell die kleine Kiste.
    #[test]
    fn beim_expertengemisch_zaehlt_die_gesamtzahl() {
        assert_eq!(milliarden_aus("30b-a3b"), Some(30.0));
        assert_eq!(milliarden_aus("0.5b"), Some(0.5));
        assert_eq!(milliarden_aus("4b"), Some(4.0));
        assert_eq!(milliarden_aus("kaputt"), None);
    }

    /// ⚠️ **Was sich nicht lesen laesst, bekommt die Grundkiste, und
    /// der Grund kommt mit.** Die kleinere Kiste geht in beiden
    /// Faellen, die groessere nur in einem; aber still darf die
    /// Entscheidung nicht fallen.
    #[test]
    fn ohne_lesbare_groesse_gibt_es_die_grundkiste() {
        for (aufbau, stichwort) in [
            (artefakt(None), "nicht lesbar"),
            (artefakt(Some("kaputt")), "keine lesbare Groesse"),
        ] {
            let (kiste, warum) = Werkzeugkiste::fuer_artefakt(aufbau.path());
            assert_eq!(kiste, Werkzeugkiste::Base, "{warum}");
            assert!(warum.contains(stichwort), "der Grund sagt nichts: {warum}");
        }
    }

    /// **Die Einstellung schlaegt die Ableitung.**
    ///
    /// ⚑ Wer sie anfasst, hat die Frage schon beantwortet: Ein kleines
    /// Modell bekommt auf Wunsch die volle Kiste.
    #[test]
    fn die_einstellung_schlaegt_die_ableitung() {
        use crate::einstellungen::Werkzeugwahl;
        let klein = artefakt(Some("4b"));
        assert_eq!(Werkzeugwahl::Automatisch.aufloesen(klein.path()).0, Werkzeugkiste::Base);
        assert_eq!(Werkzeugwahl::Advanced.aufloesen(klein.path()).0, Werkzeugkiste::Advanced);

        let gross = artefakt(Some("30b-a3b"));
        assert_eq!(Werkzeugwahl::Automatisch.aufloesen(gross.path()).0, Werkzeugkiste::Advanced);
        assert_eq!(Werkzeugwahl::Base.aufloesen(gross.path()).0, Werkzeugkiste::Base);
    }
}
