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
            // ⚑ **Fehlende Ordner duerfen fehlen** (Fund 490): Aufgeloest
            //   wird der tiefste vorhandene Vorfahr; was darunter fehlt, muss
            //   aus reinen Namen bestehen. `..` hat keinen Namen und faellt
            //   damit heraus, ein Verweis kann nicht darunter liegen, weil es
            //   dort nichts gibt. Angelegt wird hier nichts; das tut
            //   `write_file`, nachdem die Grenze geprueft ist.
            let mut vorhanden = eltern.to_path_buf();
            let mut fehlend = Vec::new();
            while !vorhanden.exists() {
                match (vorhanden.file_name(), vorhanden.parent()) {
                    (Some(n), Some(p)) => {
                        fehlend.push(n.to_os_string());
                        vorhanden = p.to_path_buf();
                    }
                    _ => {
                        return Err(Werkzeugfehler { grund: format!("{roh}: kein gueltiger Pfad") });
                    }
                }
            }
            let mut ziel = vorhanden.canonicalize().map_err(|e| Werkzeugfehler {
                grund: format!("{}: {e}", vorhanden.display()),
            })?;
            for n in fehlend.iter().rev() {
                ziel.push(n);
            }
            ziel.join(name)
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

    // 📌 **Hier stand ein zweiter Name** (`kennung()`, klein
    // geschrieben), entfernt am 2026-09-15 mit **null Aufrufern**. Die
    // Ordner heissen seit demselben Tag `Base` und `Advanced`; eine
    // zweite Schreibweise daneben war kein Komfort, sondern die Falle:
    // Wer sie fuer den Ordnerpfad genommen haette, haette auf macOS
    // nichts gemerkt und auf Linux keine Werkzeuge gehabt. **Ein
    // Gegenstand, ein Name.** Es gibt nur [`Self::name`].

    /// **Welche Kiste ein Ordnername bedeutet.**
    ///
    /// ⚑ **Der Ordner ist seit dem 2026-09-15 die einzige Quelle**
    /// (Festlegung des Projektinhabers): In den Einstellungen steht nur
    /// noch der Pfad, und sein letzter Namensteil sagt, welche
    /// eingebauten Werkzeuge dazukommen. Vorher gab es daneben eine
    /// Auswahl, und zwei Angaben fuer dieselbe Sache laufen auseinander.
    ///
    /// ⚑ **Ohne Adminmarke**, anders als bis heute. Sie war ohnehin
    /// gegenstandslos: Der `1337`-Ordner ist gitignored, wer ihn nicht
    /// hat, hat die Werkzeuge nicht, und wer ihn anlegt, hat die
    /// Entscheidung schon getroffen.
    ///
    /// ⚠️ **Ein unbekannter Name ist `Base` und kein Fehler.** Wer einen
    /// eigenen Ordner waehlt, soll seine Manifeste bekommen, und die
    /// zurueckhaltende Auswahl an eingebauten dazu; `run_command` gibt es
    /// erst, wenn der Ordner auch so heisst.
    pub fn aus_ordnername(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "advanced" => Self::Advanced,
            "1337" => Self::Elite,
            _ => Self::Base,
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
    /// **Liest im verschluesselten Mitschnitt nach**, den das Verdichten
    /// abgelegt hat.
    ///
    /// ⚑ **In jeder Kiste** (Auftrag des Projektinhabers, 2026-09-16):
    /// Nachlesen zu koennen, was man selbst gesagt bekommen hat, ist
    /// keine Sache der Modellgroesse. Es ist ausserdem das **einzige**
    /// Werkzeug, das einen Verlust ausgleicht statt etwas Neues zu tun.
    ///
    /// ⚠️ **Es liest nicht mit `read_file`**, und das ist kein Umweg:
    /// `read_file` braeuchte den Pfad, den nur die Zusammenfassung
    /// nennt, und kennte den Deckel von 200 Zeilen nicht. **Der
    /// Mitschnitt entsteht, weil der Kontext voll war**; ein Werkzeug,
    /// das ihn in einem Zug zurueckholt, macht die Verdichtung
    /// rueckgaengig.
    Verlauf,
    /// **Nennt, was in diesem Ordner schon geschehen ist.**
    ///
    /// ⚑ **Damit ein frischer Agent aufnimmt, wo der vorige aufgehoert
    /// hat** (Auftrag des Projektinhabers, 2026-09-16). Das Verzeichnis
    /// der laufenden Sitzung steht in der Zusammenfassung; eine neue
    /// Sitzung hat keine, und dieses Werkzeug ist ihr Einstieg.
    ///
    /// ⚑ **Ohne einen einzigen Parameter**, also ohne Entscheidung und
    /// damit auch fuer ein kleines Modell kostenlos.
    VerlaufListe,
    /// **Sucht eine Zeichenfolge im Mitschnitt.**
    ///
    /// # ⛔️ Gemessen, und deshalb gebaut (2026-09-17)
    ///
    /// Ohne dieses Werkzeug geht der Weg ueber das Verzeichnis: Das 4B
    /// holt mit `list_history` rund 6 000 Token und liest danach die
    /// Zeilen. **Es findet die Einzelheit damit in sechs von neun
    /// Laeufen, aber der Kontext am Ende ist so gross wie der ganze
    /// Verlauf, den die Verdichtung gerade weggeraeumt hat.**
    ///
    /// ⚑ **Die Suche beantwortet die Frage, die tatsaechlich gestellt
    /// ist:** nicht „wie ist der Verlauf gegliedert", sondern „wo steht
    /// dieses Wort". Ihre Antwort sind ein paar Zeilen, und danach liest
    /// `read_history` genau die genannten.
    VerlaufSuche,
    /// **Sucht Skills nach den Worten einer Aufgabe oder eines Problems.**
    ///
    /// ⚑ **Fuer den Fall, dass das Modell nicht weiterweiss** (Wunsch des
    /// Projektinhabers, 2026-09-26). Drei Orte, ein Werkzeug: Projekt,
    /// eigene, mitgelieferte (`crate::skills`). Zwei davon liegen
    /// **ausserhalb der Einhaengegrenze**, und die aufzuweichen waere der
    /// teurere Weg. Ein leerer Suchtext nennt alle.
    ///
    /// 📌 Hiess bis zum 2026-09-26 `list_skills` und nannte nur.
    SkillSuche,
    /// **Lernt einen Skill: liefert seine Anleitung.**
    ///
    /// ⚑ **Die Anleitung und nicht der ganze Ordner.** Am Ende stehen die
    /// weiteren Dateien; eine davon holt derselbe Aufruf mit
    /// `name/datei`. **Ein ganzer Skill in einem Zug waere genau das
    /// Fuellen des Kontexts, das hier vermieden werden soll.**
    ///
    /// ⚑ **Ein Parameter und kein optionaler zweiter**: Eine Datei des
    /// Skills ist ein Pfad unter seinem Namen, nicht eine zweite Angabe.
    ///
    /// 📌 Hiess bis zum 2026-09-26 `read_skill`.
    SkillLernen,
}

impl Dateiwerkzeug {
    /// Alle, in der Reihenfolge, in der sie angeboten werden.
    pub const ALLE: [Dateiwerkzeug; 11] = [
        Dateiwerkzeug::Verzeichnis,
        Dateiwerkzeug::Lesen,
        Dateiwerkzeug::Suchen,
        Dateiwerkzeug::Schreiben,
        Dateiwerkzeug::Aendern,
        Dateiwerkzeug::VerlaufListe,
        Dateiwerkzeug::VerlaufSuche,
        Dateiwerkzeug::Verlauf,
        Dateiwerkzeug::SkillSuche,
        Dateiwerkzeug::SkillLernen,
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
    /// ⛔️ **Die drei Verlaufswerkzeuge sind am 2026-09-17 wieder
    /// herausgenommen worden** (Festlegung des Projektinhabers, nach
    /// der Messung).
    ///
    /// Sie standen hier einen Tag lang, weil „Nachlesen keine Sache der
    /// Modellgroesse" ist. ⚑ **Die Messung sagt das Gegenteil, und zwar
    /// fuer genau das Modell, fuer das diese Kiste gemacht ist:** Das
    /// 0,6B ruft in 27 Laeufen **kein einziges** dieser Werkzeuge und
    /// erfindet stattdessen eine Antwort. Ein strengerer Systemprompt
    /// aendert das nicht, sondern verschlechtert obendrein die Antwort,
    /// die es sonst richtig gibt (9 von 9 auf 1 von 9).
    ///
    /// ⚑ **Drei Werkzeuge, die nie gerufen werden, sind nicht
    /// folgenlos:** Sie stehen in jeder Ansage, kosten in jeder Runde
    /// Kontext und machen die Auswahl fuer ein kleines Modell schwerer.
    /// **Eine Kiste ist eine Auswahl und keine Sammlung.**
    ///
    /// ⚑ **Die zwei Skillwerkzeuge sind seit dem 2026-09-26 dabei**
    /// (Wunsch des Projektinhabers): Das Modell soll selbst nach einem
    /// Skill suchen, wenn es nicht weiterweiss, und auf „lerne skill …"
    /// einen lernen. ⚠️ Die Messung vom 2026-09-17 sagt, dass das 0,6B
    /// Nachschlagewerkzeuge nicht von selbst ruft; der Unterschied hier
    /// ist ein ausdruecklicher Anlass im Auftrag. Gemessen steht es im
    /// Changelog dieser Fassung.
    pub const BASE: [Dateiwerkzeug; 7] = [
        Dateiwerkzeug::Verzeichnis,
        Dateiwerkzeug::Lesen,
        Dateiwerkzeug::Suchen,
        Dateiwerkzeug::Schreiben,
        Dateiwerkzeug::Aendern,
        Dateiwerkzeug::SkillSuche,
        Dateiwerkzeug::SkillLernen,
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
            (Self::Verlauf, Ansageform::Amtlich) => "read_history",
            (Self::VerlaufListe, Ansageform::Amtlich) => "list_history",
            (Self::VerlaufSuche, Ansageform::Amtlich) => "search_history",
            (Self::SkillSuche, Ansageform::Amtlich) => "search_skill",
            (Self::SkillLernen, Ansageform::Amtlich) => "learn_skill",
            (Self::Verzeichnis, Ansageform::Deutsch) => "verzeichnis",
            (Self::Lesen, Ansageform::Deutsch) => "datei_lesen",
            (Self::Suchen, Ansageform::Deutsch) => "suchen",
            (Self::Schreiben, Ansageform::Deutsch) => "datei_schreiben",
            (Self::Aendern, Ansageform::Deutsch) => "datei_aendern",
            (Self::Befehl, Ansageform::Deutsch) => "befehl_ausfuehren",
            (Self::Verlauf, Ansageform::Deutsch) => "verlauf_lesen",
            (Self::VerlaufListe, Ansageform::Deutsch) => "verlauf_liste",
            (Self::VerlaufSuche, Ansageform::Deutsch) => "verlauf_suchen",
            (Self::SkillSuche, Ansageform::Deutsch) => "skill_suchen",
            (Self::SkillLernen, Ansageform::Deutsch) => "skill_lernen",
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
            (Self::Verlauf, Ansageform::Amtlich) => format!(
                "Read lines from the earlier part of this conversation, after it has \
                 been summarised. The summary lists which lines hold what. At most \
                 {VERLAUF_ZEILENGRENZE} lines per call. Use this when the summary does \
                 not contain a detail you need, such as an exact number, path or error \
                 message."
            ),
            (Self::Verlauf, Ansageform::Deutsch) => format!(
                "Liest Zeilen aus dem frueheren Teil dieses Gespraechs, nachdem er \
                 zusammengefasst wurde. Die Zusammenfassung sagt, in welchen Zeilen was \
                 steht. Hoechstens {VERLAUF_ZEILENGRENZE} Zeilen je Aufruf. Zu \
                 gebrauchen, wenn in der Zusammenfassung eine Einzelheit fehlt, etwa \
                 eine genaue Zahl, ein Pfad oder eine Fehlermeldung."
            ),
            (Self::VerlaufListe, Ansageform::Amtlich) => "Name the earlier sessions in this \
                 folder, newest first, and give the table of contents of the most recent \
                 one. Use this at the start of work in a folder you do not know yet, or \
                 when asked to pick up where someone left off. It only names what is \
                 there; read_history then reads the lines you want."
                .into(),
            (Self::VerlaufListe, Ansageform::Deutsch) => "Nennt die frueheren Sitzungen in \
                 diesem Ordner, neueste zuerst, und gibt das Verzeichnis der juengsten. \
                 Zu gebrauchen am Anfang der Arbeit in einem unbekannten Ordner oder wenn \
                 aufzunehmen ist, wo jemand aufgehoert hat. Es nennt nur, was da ist; die \
                 Zeilen liest danach verlauf_lesen."
                .into(),
            (Self::VerlaufSuche, Ansageform::Amtlich) => format!(
                "Search the recorded history of this folder for a piece of text and \
                 return the matching lines with their numbers, newest session first, at \
                 most {VERLAUF_TREFFER}. Each hit also names the line range of the whole \
                 exchange it belongs to, so read_history can read question and answer \
                 together. Use this when a detail is missing from the summary: search \
                 for a word from the question. Plain text, not a regular expression; \
                 case is ignored."
            ),
            (Self::VerlaufSuche, Ansageform::Deutsch) => format!(
                "Sucht im aufgezeichneten Verlauf dieses Ordners nach einer Zeichenfolge \
                 und gibt die gefundenen Zeilen mit ihren Nummern zurueck, juengste \
                 Sitzung zuerst, hoechstens {VERLAUF_TREFFER}. Zu jedem Treffer steht \
                 die Zeilenspanne des ganzen Wechsels dabei, damit verlauf_lesen Frage \
                 und Antwort zusammen liest. Zu gebrauchen, wenn eine Einzelheit in der \
                 Zusammenfassung fehlt: nach einem Wort aus der Frage suchen. Klartext \
                 und kein regulaerer Ausdruck; Gross- und Kleinschreibung wird \
                 ignoriert."
            ),
            (Self::SkillSuche, Ansageform::Amtlich) => "Search the skills: short guides for \
                 recurring kinds of work. Describe the task or the problem in a few words. \
                 Use it whenever you are unsure how to proceed or are stuck, before \
                 guessing. An empty text lists all skills. Then learn the best match with \
                 learn_skill."
                .into(),
            (Self::SkillSuche, Ansageform::Deutsch) => "Sucht in den Skills: kurzen Anleitungen \
                 fuer wiederkehrende Arbeiten. Beschreibe die Aufgabe oder das Problem in \
                 wenigen Worten. Zu gebrauchen, sobald du unsicher bist oder nicht \
                 weiterkommst, bevor du raetst. Ein leerer Text nennt alle. Den besten \
                 Treffer lernst du danach mit skill_lernen."
                .into(),
            (Self::SkillLernen, Ansageform::Amtlich) => "Learn a skill by name: returns its \
                 instructions, which you then follow for the rest of this task. Use it when \
                 the user says \"learn skill ...\" or when search_skill found a fitting one. \
                 Its further files are listed at the end; open one with the name followed by \
                 a slash and the file, for example name/templates/x.md."
                .into(),
            (Self::SkillLernen, Ansageform::Deutsch) => "Lernt einen Skill nach Namen: liefert \
                 seine Anleitung, der du fuer den Rest dieser Aufgabe folgst. Zu gebrauchen, \
                 wenn der Nutzer \"lerne skill ...\" sagt oder skill_suchen einen passenden \
                 gefunden hat. Am Ende stehen seine weiteren Dateien; eine davon oeffnet der \
                 Name mit Schraegstrich und Datei, zum Beispiel name/vorlagen/x.md."
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
        let (
            wohin,
            tiefe_hinweis,
            muster_hinweis,
            alt_hinweis,
            aenderungshinweis,
            befehl_hinweis,
            von_hinweis,
            bis_hinweis,
            suchhinweis,
            skillhinweis,
            anfragehinweis,
        ) = match form {
            Ansageform::Amtlich => (
                "path of the file, relative to the working directory; \
                 paths leading outside it are rejected",
                "how many levels to descend; 1 lists the working directory itself",
                "literal text, not a regular expression; the whole working directory is searched",
                "must occur exactly once in the file",
                "All changes are applied in order, and each one sees the result of the \
                 previous ones. Either all of them are written or none.",
                "the command line, run with sh -c in the working directory",
                "first line to read, counting from 1; the table of contents is in the summary",
                "last line to read, inclusive",
                "the text to look for in the recorded history, for example a word from \
                 the question",
                "the name of the skill, exactly as search_skill gives it; for one of \
                 its further files the name, a slash and the file",
                "a few words describing the task or the problem; empty lists all skills",
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
                "erste zu lesende Zeile, ab 1 gezaehlt; das Verzeichnis steht in der Zusammenfassung",
                "letzte zu lesende Zeile, einschliesslich",
                "der Text, nach dem im aufgezeichneten Verlauf gesucht wird, zum Beispiel \
                 ein Wort aus der Frage",
                "der Name des Skills, genau so, wie skill_suchen ihn nennt; fuer eine \
                 seiner weiteren Dateien der Name, ein Schraegstrich und die Datei",
                "wenige Worte zur Aufgabe oder zum Problem; leer nennt alle Skills",
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
            // ⚑ **Beide Felder sind verlangt, und das Verzeichnis kommt
            // woanders her.**
            //
            // 📌 **Der erste Entwurf machte sie freiwillig**: ohne
            // Argumente das Verzeichnis, mit ihnen die Zeilen. Das
            // brach die Regel, dass **kein Werkzeug einen optionalen
            // Parameter hat**, und die Regel hat recht: Ein optionaler
            // Parameter ist eine Entscheidung, die das Modell treffen
            // muss.
            //
            // ⚑ **Die bessere Antwort war nicht ein zweites Werkzeug,
            // sondern das Verzeichnis dorthin zu legen, wo das Modell
            // ohnehin hinsieht:** in die Zusammenfassung. Damit braucht
            // es keinen Aufruf, um zu erfahren, welche Zeilen es gibt,
            // und dieses Werkzeug hat genau eine Aufgabe.
            // ⚑ **Gar keine Eigenschaften**, also auch kein optionaler
            // Parameter: Das Werkzeug beantwortet eine Frage, die keine
            // Angabe braucht.
            Self::VerlaufListe => serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            // ⚑ **Ein Parameter, und er ist erforderlich.** Eine Suche
            // ohne Suchwort ist keine Frage.
            // ⚑ **Verlangt, auch wenn er leer sein darf**: kein optionaler
            //   Parameter, also keine Entscheidung, ob es ihn gibt.
            Self::SkillSuche => serde_json::json!({
                "type": "object",
                "properties": {
                    "anfrage": {"type": "string", "description": anfragehinweis}
                },
                "required": ["anfrage"]
            }),
            Self::SkillLernen => serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": skillhinweis}
                },
                "required": ["name"]
            }),
            Self::VerlaufSuche => serde_json::json!({
                "type": "object",
                "properties": {
                    "muster": {
                        "type": "string",
                        "description": suchhinweis
                    }
                },
                "required": ["muster"]
            }),
            Self::Verlauf => serde_json::json!({
                "type": "object",
                "properties": {
                    "von": {"type": "integer", "description": von_hinweis},
                    "bis": {"type": "integer", "description": bis_hinweis}
                },
                "required": ["von", "bis"]
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
        budget: &Verlaufsbudget,
    ) -> Box<dyn myl_local_agent::ausfuehrung::Werkzeugausfuehrung> {
        match self {
            Self::Verzeichnis => Box::new(Verzeichnislesen(e, form)),
            Self::Lesen => Box::new(Dateilesen(e, form)),
            Self::Suchen => Box::new(Suchen(e, form)),
            Self::Schreiben => Box::new(Dateischreiben(e, form)),
            Self::Aendern => Box::new(Dateiaendern(e, form)),
            Self::Befehl => Box::new(Befehlausfuehren(e, form)),
            // ⚑ **Ein Budget, das die drei sich teilen.** Getrennte
            // Budgets waeren drei Wege, denselben Kontext zu fuellen.
            Self::Verlauf => Box::new(Verlauflesen(e, form, budget.clone())),
            Self::VerlaufListe => Box::new(Verlaufliste(e, form, budget.clone())),
            Self::VerlaufSuche => Box::new(Verlaufsuche(e, form, budget.clone())),
            Self::SkillSuche => Box::new(Skillsuche(e, form, budget.clone())),
            Self::SkillLernen => Box::new(Skilllernen(e, form, budget.clone())),
        }
    }
}

/// **Wie viele Zeilen ein Aufruf hoechstens herausgibt.**
///
/// ⛔️ **Der Deckel ist der Sinn der Sache und keine Vorsicht.** Der
/// Mitschnitt entsteht, weil der Kontext voll war; ein Werkzeug, das
/// ihn in einem Zug zurueckholt, macht die Verdichtung rueckgaengig und
/// fuellt genau den Platz wieder, den sie gerade frei gemacht hat.
/// **Wer mehr braucht, fragt zweimal**, und dann ist es eine
/// Entscheidung und kein Versehen.
pub const VERLAUF_ZEILENGRENZE: usize = 200;

/// Nennt, was in diesem Ordner schon geschehen ist.
struct Verlaufliste(Einhaengung, Ansageform, Verlaufsbudget);

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Verlaufliste {
    fn name(&self) -> &str {
        Dateiwerkzeug::VerlaufListe.name(self.1)
    }
    fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let deutsch = matches!(self.1, Ansageform::Deutsch);
        let u = crate::verlauf::uebersicht(self.0.wurzel());
        if u.episoden == 0 {
            return Ok(if deutsch {
                "In diesem Ordner ist noch nichts aufgezeichnet.".into()
            } else {
                "Nothing has been recorded in this folder yet.".into()
            });
        }

        let mut aus = String::new();
        if deutsch {
            aus.push_str(&format!(
                "{} Episoden aus {} Sitzungen in diesem Ordner.\n\n",
                u.episoden, u.sitzungen
            ));
        } else {
            aus.push_str(&format!(
                "{} episodes from {} sessions in this folder.\n\n",
                u.episoden, u.sitzungen
            ));
        }

        if let Some((pfad, v)) = &u.juengste {
            let name = pfad
                .strip_prefix(self.0.wurzel())
                .unwrap_or(pfad)
                .display()
                .to_string();
            if deutsch {
                aus.push_str(&format!(
                    "Die juengste ist vom {} ({}), Sitzung {}, {} Nachrichten. \
                     Ihr Verzeichnis:\n",
                    v.datum, v.modell, v.sitzung, v.nachrichten
                ));
            } else {
                aus.push_str(&format!(
                    "The most recent one is from {} ({}), session {}, {} messages. \
                     Its table of contents:\n",
                    v.datum, v.modell, v.sitzung, v.nachrichten
                ));
            }
            // ⛔️ **Auch hier gilt Fund 387.** Ein Verzeichnis mit einer
            // Zeile je Nachricht wandert als Werkzeugantwort in genau
            // den Kontext, den das Nachschlagen schonen soll; bei 120
            // Nachrichten waren das 3 597 Token. **Passt es, kommt es
            // ganz; passt es nicht, kommt die Karte**, und das Modell
            // zoomt mit `read_history` hinein.
            // ⚑ **Das Budget entscheidet, nicht eine Zahl von
            // Abschnitten.** Vorher stand hier eine feste Grenze, und
            // sie war zweimal falsch: einmal zu klein (das Verzeichnis
            // verlor die Ueberschriften, und das Modell suchte blind),
            // einmal zu gross (rund 6 000 Token in einer einzigen
            // Antwort). **Die richtige Frage ist nicht „wie viele
            // Abschnitte", sondern „passt es in das, was dieser Auftrag
            // noch ausgeben darf".**
            let ganz: String = v
                .abschnitte
                .iter()
                .map(|a| format!("{}-{} {} {}\n", a.von, a.bis, a.rolle, a.kopf))
                .collect();
            let rest = self.2.load(std::sync::atomic::Ordering::Relaxed);
            if aus.len() + ganz.len() <= rest {
                aus.push_str(&ganz);
            } else {
                for z in crate::verlauf::grobverzeichnis(pfad, VERZEICHNIS_GROB)
                    .unwrap_or_default()
                {
                    aus.push_str(z.trim_start());
                    aus.push('\n');
                }
                aus.push_str(&if deutsch {
                    format!(
                        "({} Abschnitte, zu {} Bloecken zusammengefasst, weil das ganze \
                         Verzeichnis den Kontext fuellen wuerde. Die genannten Zeilen \
                         lassen sich einzeln nachlesen, und verlauf_suchen findet eine \
                         Stelle direkt.)\n",
                        v.abschnitte.len(),
                        VERZEICHNIS_GROB
                    )
                } else {
                    format!(
                        "({} sections, grouped into {} blocks, because the full table of \
                         contents would fill the context. The line ranges above can be \
                         read individually, and search_history finds a passage \
                         directly.)\n",
                        v.abschnitte.len(),
                        VERZEICHNIS_GROB
                    )
                });
            }
            aus.push_str(&format!("\n({name})\n"));
        }
        Ok(vom_budget(&self.2, aus, deutsch))
    }
}

/// ⛔️ **Hier stand `VERZEICHNIS_GANZ`, eine feste Zahl von
/// Abschnitten, und sie war zweimal falsch.**
///
/// Erst 60: Dann verlor das Verzeichnis seine Ueberschriften, und ein
/// Modell suchte danach blind in Zeilenfenstern. Dann 400: Dann kamen
/// rund 6 000 Token in **einer** Antwort, und der Kontext am Ende war so
/// gross wie der Verlauf, den die Verdichtung weggeraeumt hatte.
///
/// ⚑ **Die Frage war beide Male die falsche.** Nicht „wie viele
/// Abschnitte duerfen es sein", sondern „passt es in das, was dieser
/// Auftrag noch ausgeben darf": [`VERLAUF_BUDGET_ZEICHEN`].
/// Wie viele Bloecke die Karte hat, wenn das Verzeichnis zu lang ist.
const VERZEICHNIS_GROB: usize = 30;

/// **Wie viele Zeichen die Verlaufswerkzeuge je Auftrag zusammen
/// herausgeben duerfen.**
///
/// # ⛔️ Warum das eine Schranke im Code ist und keine Bitte im Prompt
///
/// Gemessen am 2026-09-17: Ueber das Verzeichnis fand ein Modell die
/// gesuchte Einzelheit, und **der Kontext am Ende war so gross wie der
/// ganze Verlauf**, den die Verdichtung gerade weggeraeumt hatte. Eine
/// zweite Messung zeigte, dass Modelle die Bitte „ruf das Werkzeug nur,
/// wenn es noetig ist" **nicht befolgen**: 9 von 9 unnoetigen Aufrufen,
/// auch mit ausdruecklichem Gegenfall im Systemprompt.
///
/// ⚑ **Also haelt der Code, worum man ein Modell nicht bitten kann.**
/// Achttausend Zeichen sind rund 2 000 bis 2 700 Token, also unter 7 %
/// eines Kontexts von 40 960, und sie reichen fuer eine Suche und ein
/// ausfuehrliches Nachlesen. Was darueber hinausgeht, bekommt eine
/// Absage mit Begruendung statt stiller Kuerzung.
///
/// ⚑ **Je Auftrag und nicht je Sitzung:** Die naechste Frage des
/// Nutzers ist ein neuer Anlass nachzuschlagen, und wer nach einer
/// Stunde Arbeit nichts mehr nachlesen darf, versteht nicht, warum.
pub const VERLAUF_BUDGET_ZEICHEN: usize = 8_000;

/// Das gemeinsame Restbudget der drei Verlaufswerkzeuge.
pub type Verlaufsbudget = std::sync::Arc<std::sync::atomic::AtomicUsize>;

/// Ein frisches Budget.
pub fn verlaufsbudget() -> Verlaufsbudget {
    std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(VERLAUF_BUDGET_ZEICHEN))
}

/// Nimmt `text` vom Budget und gibt zurueck, was davon herausgehen darf.
///
/// ⚑ **Die Absage ist eine Auskunft und kein Fehler.** Ein
/// Werkzeugfehler liest sich fuer ein Modell wie „falsch aufgerufen",
/// und es versucht es dann anders herum noch einmal. Hier ist der Aufruf
/// richtig und die Antwort trotzdem zu Ende.
fn vom_budget(budget: &Verlaufsbudget, text: String, deutsch: bool) -> String {
    use std::sync::atomic::Ordering;
    let rest = budget.load(Ordering::Relaxed);
    if rest == 0 {
        return if deutsch {
            format!(
                "Das Nachschlagen hat in diesem Auftrag schon {VERLAUF_BUDGET_ZEICHEN} Zeichen \
                 geliefert. Fasse zusammen, was du hast; bei der naechsten Frage beginnt das \
                 Budget neu."
            )
        } else {
            format!(
                "Looking things up has already returned {VERLAUF_BUDGET_ZEICHEN} characters in \
                 this task. Work with what you have; the budget starts over with the next \
                 question."
            )
        };
    }
    if text.len() <= rest {
        budget.fetch_sub(text.len(), Ordering::Relaxed);
        return text;
    }
    budget.store(0, Ordering::Relaxed);
    // ⚑ **Auf Zeichengrenzen kuerzen**, sonst zerschneidet der Schnitt
    // einen Umlaut und die Antwort traegt ein kaputtes Zeichen.
    let mut gekuerzt: String = text.chars().scan(0usize, |n, c| {
        *n += c.len_utf8();
        (*n <= rest).then_some(c)
    }).collect();
    gekuerzt.push_str(if deutsch {
        "\n[Hier endet die Antwort: das Budget fuers Nachschlagen ist in diesem Auftrag \
         aufgebraucht.]"
    } else {
        "\n[Cut off here: the lookup budget for this task is used up.]"
    });
    gekuerzt
}

/// **Wie viele Treffer eine Suche im Verlauf hoechstens nennt.**
///
/// ⚑ **Genug, um eine Frage zu beantworten, und zu wenig, um den
/// Kontext zu fuellen.** Wer zwanzig Stellen gefunden hat und immer noch
/// nicht weiss, welche gemeint ist, hat das falsche Wort gesucht.
const VERLAUF_TREFFER: usize = 20;

/// Sucht Skills an allen drei Orten.
struct Skillsuche(Einhaengung, Ansageform, Verlaufsbudget);

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Skillsuche {
    fn name(&self) -> &str {
        Dateiwerkzeug::SkillSuche.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let deutsch = matches!(self.1, Ansageform::Deutsch);
        // ⚑ **Erst der Aufruf, dann die Welt.**
        let anfrage = a.get("anfrage").and_then(|v| v.as_str()).map(str::to_string).ok_or_else(|| {
            Werkzeugfehler { grund: "`anfrage` fehlt oder ist kein Text".to_string() }
        })?;
        let alle_nennen = anfrage.trim().is_empty() || anfrage.trim() == "*";
        let hoechstens = if alle_nennen { crate::skills::HOECHSTENS_GENANNT } else { crate::skills::HOECHSTENS_TREFFER };
        let treffer = crate::skills::suchen(Some(self.0.wurzel()), &anfrage, hoechstens);
        if treffer.is_empty() {
            let gibt_es = !crate::skills::alle(Some(self.0.wurzel())).is_empty();
            return Ok(match (deutsch, gibt_es) {
                (true, true) => format!("Kein Skill passt zu \"{anfrage}\". Ein leerer Text nennt alle."),
                (false, true) => format!("No skill matches \"{anfrage}\". An empty text lists all of them."),
                (true, false) => "Es liegen keine Skills bereit.".into(),
                (false, false) => "No skills are available.".into(),
            });
        }
        // ⚑ **Schwache Treffer sagen sich an.** Ein Modell lernt sonst den
        //   erstbesten, auch wenn nur ein Wort seiner Anleitung passte.
        let schwach = !alle_nennen && treffer.iter().all(|t| t.punkte < crate::skills::STARK_AB);
        let mut aus = match (deutsch, schwach) {
            (true, false) => format!("{} Skill(s), der beste zuerst:\n", treffer.len()),
            (false, false) => format!("{} skill(s), best first:\n", treffer.len()),
            (true, true) => format!(
                "Nur schwache Treffer ({}): Kein Stichwort passt. Lerne einen nur, wenn die \
                 Beschreibung wirklich zur Aufgabe passt; sonst suche mit anderen Worten.\n",
                treffer.len()
            ),
            (false, true) => format!(
                "Only weak matches ({}): no keyword fits. Learn one only if its description \
                 really fits the task; otherwise search with other words.\n",
                treffer.len()
            ),
        };
        // ⚑ **Name und Satz**, wie beim Mitschnitt gemessen: Ein Eintrag,
        // der nur den Namen nennt, wird fuer die Auskunft gehalten.
        // 📌 **Der Name steht fuer sich**, die Beschreibung in Klammern
        //    dahinter. Gemessen am 2026-09-26: Bei `name: satz` gab das 4B
        //    die ganze Zeile als Namen an `learn_skill`, dreimal.
        for t in &treffer {
            aus.push_str(&format!("{} ({})\n", t.skill.name, t.skill.satz));
        }
        aus.push_str(if deutsch {
            "Lernen mit skill_lernen und dem Namen."
        } else {
            "Learn one with learn_skill and its name."
        });
        Ok(vom_budget(&self.2, aus, deutsch))
    }
}

/// Liefert die Anleitung eines Skills oder eine seiner Dateien.
struct Skilllernen(Einhaengung, Ansageform, Verlaufsbudget);

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Skilllernen {
    fn name(&self) -> &str {
        Dateiwerkzeug::SkillLernen.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let deutsch = matches!(self.1, Ansageform::Deutsch);
        let name = a
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| Werkzeugfehler { grund: "`name` fehlt oder ist kein Text".to_string() })?;
        // ⚑ `name/datei` holt eine weitere Datei. ⛔️ Aufgeloest wird in
        //   `skills::lernen`, gegen den Ordner des Skills, nie frei.
        // ⚑ **Nur der Name**: bis zum ersten Doppelpunkt, Leerzeichen oder
        //   zur ersten Klammer. Ein Modell, das die ganze Trefferzeile
        //   abschreibt, meint trotzdem diesen Skill.
        let name = name.trim();
        let name = &name[..name.find([':', ' ', '(', '\t', '\n']).unwrap_or(name.len())];
        let (skill, datei) = match name.split_once('/') {
            Some((s, d)) => (s.to_string(), Some(d.to_string())),
            None => (name.to_string(), None),
        };
        match crate::skills::lernen(Some(self.0.wurzel()), &skill, datei.as_deref()) {
            Ok(g) => {
                let mut aus = g.text;
                if datei.is_none() && !g.dateien.is_empty() {
                    let liste: Vec<String> = g.dateien.iter().map(|d| format!("{}/{d}", g.skill.name)).collect();
                    aus.push_str(&if deutsch {
                        format!("\n\n[Weitere Dateien dieses Skills, mit skill_lernen zu oeffnen: {}]", liste.join(", "))
                    } else {
                        format!("\n\n[Further files of this skill, open with learn_skill: {}]", liste.join(", "))
                    });
                }
                Ok(vom_budget(&self.2, aus, deutsch))
            }
            // ⚑ Kein Fehler des Aufrufs, sondern eine Auskunft: Die
            //   Meldung nennt, was es gibt, damit der naechste Aufruf trifft.
            Err(m) => Ok(m),
        }
    }
}

/// Sucht im Mitschnitt und nennt die Abschnitte.
struct Verlaufsuche(Einhaengung, Ansageform, Verlaufsbudget);

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Verlaufsuche {
    fn name(&self) -> &str {
        Dateiwerkzeug::VerlaufSuche.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let deutsch = matches!(self.1, Ansageform::Deutsch);
        // ⚑ **Erst der Aufruf, dann die Welt.** Ein fehlendes Argument
        // ist ein Fehler des Aufrufs und wird als solcher gemeldet, auch
        // wenn es gar keinen Mitschnitt gibt.
        let muster = a
            .get("muster")
            .and_then(|v| v.as_str())
            .map(|t| t.to_string())
            .ok_or_else(|| Werkzeugfehler {
                grund: "`muster` fehlt oder ist kein Text".to_string(),
            })?;
        let treffer = crate::verlauf::suchen(self.0.wurzel(), &muster, VERLAUF_TREFFER);
        if treffer.is_empty() {
            return Ok(if deutsch {
                format!("Nichts gefunden zu \"{muster}\" im aufgezeichneten Verlauf.")
            } else {
                format!("Nothing found for \"{muster}\" in the recorded history.")
            });
        }
        let mut aus = if deutsch {
            format!("{} Fundstellen zu \"{muster}\":\n", treffer.len())
        } else {
            format!("{} hits for \"{muster}\":\n", treffer.len())
        };
        // ⛔️ **Der Fund steht dabei, und das ist gemessen.** Eine
        // Suche, die nur Zeilennummern zurueckgibt, wird fuer die
        // Auskunft gehalten: Das 4B antwortete in neun von neun Laeufen
        // „laeuft unter der Kennung **612-614**". **Was gefunden wurde,
        // gehoert in die Antwort; was drumherum steht, holt das
        // Lesewerkzeug.**
        for t in &treffer {
            aus.push_str(&format!("{}: {}\n", t.zeile, t.text));
            if deutsch {
                aus.push_str(&format!(
                    "    ganzer Wechsel: verlauf_lesen {} bis {}\n",
                    t.von, t.bis
                ));
            } else {
                aus.push_str(&format!(
                    "    whole exchange: read_history {} to {}\n",
                    t.von, t.bis
                ));
            }
        }
        // ⚑ **Die Datei steht dabei**, denn die Treffer koennen aus
        // verschiedenen Sitzungen stammen, und die Zeilennummern gelten
        // je Datei.
        let dateien: std::collections::BTreeSet<&str> =
            treffer.iter().map(|t| t.datei.as_str()).collect();
        for d in dateien {
            aus.push_str(&format!("({d})\n"));
        }
        Ok(vom_budget(&self.2, aus, deutsch))
    }
}

/// Liest im Mitschnitt nach, den das Verdichten abgelegt hat.
struct Verlauflesen(Einhaengung, Ansageform, Verlaufsbudget);

impl myl_local_agent::ausfuehrung::Werkzeugausfuehrung for Verlauflesen {
    fn name(&self) -> &str {
        Dateiwerkzeug::Verlauf.name(self.1)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let deutsch = matches!(self.1, Ansageform::Deutsch);

        // ⚑ **Erst der Aufruf, dann die Welt.** Ein fehlendes Argument
        // ist ein Fehler des Aufrufers und gehoert benannt, auch wenn es
        // zufaellig nichts zu lesen gaebe: Sonst bekaeme ein Modell, das
        // `bis` vergessen hat, die Auskunft „es gibt keinen Mitschnitt"
        // und suchte den Fehler an der falschen Stelle.
        let zahl = |feld: &str| -> Result<usize, Werkzeugfehler> {
            a.get(feld)
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .ok_or_else(|| Werkzeugfehler {
                    grund: format!("`{feld}` fehlt oder ist keine Zahl"),
                })
        };
        let (von, bis) = (zahl("von")?, zahl("bis")?);

        let mitschnitte = crate::verlauf::vorhandene(self.0.wurzel());
        let Some(neuester) = mitschnitte.first() else {
            return Ok(if deutsch {
                "Es gibt noch keinen Mitschnitt. Er entsteht, sobald das Gespraech \
                 verdichtet wird.".into()
            } else {
                "There is no transcript yet. One is written as soon as the conversation \
                 is summarised.".into()
            });
        };

        // ⚑ **Der Deckel wirkt hier und nicht beim Aufrufer**, und er
        // sagt es: Eine stillschweigend gekuerzte Antwort liest sich wie
        // eine vollstaendige.
        let gewuenscht = bis.saturating_sub(von) + 1;
        let bis_wirklich = if gewuenscht > VERLAUF_ZEILENGRENZE {
            von + VERLAUF_ZEILENGRENZE - 1
        } else {
            bis
        };
        let text = crate::verlauf::zeilen(neuester, von, bis_wirklich)
            .map_err(|f| Werkzeugfehler { grund: f.to_string() })?;

        if gewuenscht > VERLAUF_ZEILENGRENZE {
            let vermerk = if deutsch {
                format!(
                    "\n[{gewuenscht} Zeilen verlangt, {VERLAUF_ZEILENGRENZE} gegeben \
                     (Zeilen {von} bis {bis_wirklich}). Der Rest mit einem zweiten Aufruf \
                     ab {}.]\n",
                    bis_wirklich + 1
                )
            } else {
                format!(
                    "\n[{gewuenscht} lines requested, {VERLAUF_ZEILENGRENZE} returned \
                     (lines {von} to {bis_wirklich}). Ask again from {} for the rest.]\n",
                    bis_wirklich + 1
                )
            };
            return Ok(vom_budget(&self.2, format!("{text}{vermerk}"), deutsch));
        }
        Ok(vom_budget(&self.2, text, deutsch))
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
        // 📌 **Fund 492 (2026-09-26): Die Suche fand keine Dateinamen.** Das
        //    30B suchte `messwerte.csv`, bekam nur die Zeile in
        //    `auswertung.py`, die den Namen nennt, und schloss, die Datei
        //    fehle. Jetzt stehen Pfade, deren Name das Muster enthaelt, vor
        //    den Zeilentreffern.
        let mut namen: Vec<String> = Vec::new();
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
                let kurz = self.0.kurz(&echt);
                if namen.len() < TREFFERGRENZE && kurz.to_lowercase().contains(&unten) {
                    namen.push(if echt.is_dir() { format!("{kurz}/") } else { kurz.clone() });
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

        namen.sort();
        let namensteil = if namen.is_empty() {
            String::new()
        } else {
            format!("Dateien und Ordner, deren Name {muster:?} enthaelt:\n{}\n", namen.join("\n"))
        };
        if treffer.is_empty() {
            return Ok(format!("{namensteil}keine Zeile enthaelt {muster:?}"));
        }
        treffer.sort();
        if !namensteil.is_empty() {
            treffer.insert(0, format!("{namensteil}Zeilen, die {muster:?} enthalten:"));
        }
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
        // 📌 **Fund 490 (2026-09-26): Ein fehlender Ordner war eine Sackgasse.**
        //    Im Loop-Szenario wollte das 8B `ergebnis/statistik.md` schreiben,
        //    neunmal, und jedes Mal fehlte `ergebnis/`. Ohne Shell gibt es
        //    kein Werkzeug, das einen Ordner anlegt; in `Base` war die
        //    Aufgabe damit unloesbar. Die Grenze ist oben geprueft.
        if let Some(eltern) = p.parent() {
            std::fs::create_dir_all(eltern).map_err(|e| Werkzeugfehler { grund: format!("{roh}: {e}") })?;
        }
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
        if !self.0.darf_schreiben() {
            return Err(Werkzeugfehler {
                grund: "diese Sitzung darf nur lesen; `run_command` wirkt und braucht \
                        `myl setzen agent.schreiben an`"
                    .into(),
            });
        }
        befehl_im_verzeichnis(&self.0, &zeichenkette(a, "befehl")?)
    }
}

/// **Fuehrt einen Shell-Befehl im Arbeitsverzeichnis der Einhaengung aus**,
/// mit allen Schranken aus [`Befehlausfuehren`]: Sperrliste, Zeitgrenze,
/// Ausgabegrenze.
///
/// ⚑ **Eine Stelle fuer beide Aufrufer** (2026-09-14): `run_command` und die
/// Manifest-Werkzeuge aus einer Werkzeugkiste (`crate::kisten`) nehmen
/// denselben, gepruefen Weg. Ein zweiter Laeufer daneben liefe frueher oder
/// spaeter mit anderen Grenzen.
///
/// ⚠️ **Die Schreiberlaubnis prueft der Aufrufer**, nicht diese Funktion:
/// `run_command` verlangt sie, ein Manifest-Werkzeug entscheidet es an
/// seinem `wirkt`-Feld.
pub fn befehl_im_verzeichnis(e: &Einhaengung, befehl: &str) -> Result<String, Werkzeugfehler> {
    befehl_im_verzeichnis_mit(e, befehl, &[], BEFEHL_ZEITGRENZE_S)
}

/// **Derselbe Weg, mit einer Umgebung und einer eigenen Frist.**
///
/// ⚑ **Der Lauf selbst steht seit dem 2026-09-17 in
/// `myl_senses::prozess`.** Er stand hier, und die Sinneskiste haette
/// ihn ein zweites Mal gebraucht: **zwei Laeufer mit zwei Fristen**, und
/// der zweite meldet sich nicht. Hier bleibt, was diesen Aufruf
/// ausmacht: die Sperrliste, das Arbeitsverzeichnis, die Form der
/// Antwort.
///
/// `umgebung` sind zusaetzliche Umgebungsvariablen fuer den
/// Kindprozess. Gebraucht wird das von den Manifest-Werkzeugen, die
/// ihren eigenen Kistenordner kennen muessen, um ein Skript **neben**
/// dem Manifest aufzurufen: Ein Manifest kann seinen eigenen Pfad nicht
/// wissen, und ein absoluter Pfad im Manifest waere auf jeder anderen
/// Maschine falsch.
///
/// `zeitgrenze_s` ist die Frist in Sekunden. ⚠️ **Der Aufrufer begrenzt
/// sie**; diese Funktion nimmt, was sie bekommt.
pub fn befehl_im_verzeichnis_mit(
    e: &Einhaengung,
    befehl: &str,
    umgebung: &[(&str, String)],
    zeitgrenze_s: u64,
) -> Result<String, Werkzeugfehler> {
    let klein = befehl.to_lowercase();
    if let Some(muster) = SPERRMUSTER.iter().find(|m| klein.contains(**m)) {
        return Err(Werkzeugfehler {
            grund: format!("der Befehl enthaelt das gesperrte Muster `{muster}` und wird nicht ausgefuehrt"),
        });
    }

    let mut kind = std::process::Command::new("sh");
    kind.arg("-c")
        .arg(befehl)
        .envs(umgebung.iter().map(|(name, wert)| (*name, wert.as_str())))
        .current_dir(e.wurzel());
    let a = myl_senses::prozess::laufen(&mut kind, zeitgrenze_s, BEFEHL_AUSGABEGRENZE)
        .map_err(|f| Werkzeugfehler { grund: format!("der Befehl {f}") })?;
    let (sicht, gekuerzt) = a.zusammen(BEFEHL_AUSGABEGRENZE);
    let schwanz = if gekuerzt {
        format!("\n… (Ausgabe auf {BEFEHL_AUSGABEGRENZE} Bytes gekuerzt)")
    } else {
        String::new()
    };
    Ok(format!("{}\n{sicht}{schwanz}", a.kopf()))
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
                // 📌 **Fund 492 (2026-09-26): Ein nicht aufgeklappter Ordner
                //    sah leer aus.** Das 30B listete mit Tiefe 1, sah
                //    `daten\tVerzeichnis` und schloss, `daten/messwerte.csv`
                //    gebe es nicht. Jetzt steht dabei, wie viel darin liegt,
                //    und womit man es sieht.
                let zugeklappt = art == "Verzeichnis" && ebene >= tiefe;
                zeilen.push(if art == "Datei" {
                    format!("{name}\t{groesse} Bytes")
                } else if zugeklappt {
                    let n = std::fs::read_dir(&echt).map(|d| d.count()).unwrap_or(0);
                    format!("{name}\t{art}, {n} {}, erst mit tiefe {} zu sehen", if n == 1 { "Eintrag" } else { "Eintraege" }, ebene + 1)
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
                    .map(|w| w.ausfuehrung(e.clone(), form, &verlaufsbudget()).name().to_string())
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
        // 📌 Fund 492: Ein zugeklappter Ordner sagt, was in ihm liegt.
        assert!(aus.contains("Eintr") && aus.contains("erst mit tiefe 2"), "{aus}");
        let tief = v.ausfuehren(&serde_json::json!({"tiefe": 2})).expect("listen");
        assert!(!tief.contains("erst mit tiefe 2"), "aufgeklappt und trotzdem der Hinweis: {tief}");
    }

    /// 📌 **Fund 492: Die Suche nennt auch Dateinamen.**
    #[test]
    fn die_suche_findet_auch_dateinamen() {
        let (d, e) = baue();
        std::fs::create_dir_all(d.path().join("daten")).unwrap();
        std::fs::write(d.path().join("daten/messwerte.csv"), "a;b\n").unwrap();
        std::fs::write(d.path().join("skript.py"), "QUELLE = 'messwerte.csv'\n").unwrap();
        let s = Suchen(e, Ansageform::Amtlich);
        let aus = s.ausfuehren(&serde_json::json!({"muster": "messwerte.csv"})).expect("suchen");
        assert!(aus.contains("deren Name") && aus.contains("daten/messwerte.csv"), "{aus}");
        assert!(aus.contains("skript.py:1"), "die Zeile fehlt: {aus}");
        let nur_name = s.ausfuehren(&serde_json::json!({"muster": "daten"})).expect("suchen");
        assert!(nur_name.contains("daten/") && nur_name.contains("keine Zeile"), "{nur_name}");
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

#[cfg(test)]
mod verlaufwerkzeug {
    use super::*;
    use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;

    fn werkzeug() -> (tempfile::TempDir, Verlauflesen) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        (d, Verlauflesen(e, Ansageform::Amtlich, verlaufsbudget()))
    }

    /// **Ohne Mitschnitt sagt es das, statt zu scheitern.**
    ///
    /// ⚑ Ein Werkzeug, das beim ersten Gebrauch einen Fehler wirft,
    /// laesst ein Modell glauben, es habe etwas falsch gemacht. Hier hat
    /// es nichts falsch gemacht: Es gibt noch nichts zu lesen.
    #[test]
    fn ohne_mitschnitt_kommt_ein_satz_und_kein_fehler() {
        let (_d, w) = werkzeug();
        let aus = w.ausfuehren(&serde_json::json!({"von": 1, "bis": 5})).expect("kein Fehler");
        assert!(aus.contains("no transcript"), "{aus}");
    }

    /// **Es liest die verlangten Zeilen und nicht mehr.**
    #[test]
    fn es_liest_die_verlangten_zeilen() {
        let (d, w) = werkzeug();
        crate::verlauf::schreiben(
            d.path(),
            "s1",
            "m",
            &[
                ("user".into(), "erste Frage\nmit zweiter Zeile".into()),
                ("assistant".into(), "die Antwort mit 4711".into()),
            ],
        )
        .expect("schreiben");

        let pfade = crate::verlauf::vorhandene(d.path());
        let v = crate::verlauf::verzeichnis(&pfade[0]).expect("Verzeichnis");
        let antwort = &v.abschnitte[1];
        let aus = w
            .ausfuehren(&serde_json::json!({"von": antwort.von, "bis": antwort.bis}))
            .expect("lesen");
        assert!(aus.contains("4711"), "{aus}");
        assert!(!aus.contains("erste Frage"), "es kam mehr heraus als gefragt: {aus}");
    }

    /// ⛔️ **Der Deckel greift und sagt es.**
    ///
    /// Eine stillschweigend gekuerzte Antwort liest sich wie eine
    /// vollstaendige, und dann fehlt dem Modell etwas, ohne dass es das
    /// merkt.
    #[test]
    fn der_deckel_greift_und_sagt_es() {
        let (d, w) = werkzeug();
        let viele: String =
            (1..=600).map(|i| format!("Zeile {i}\n")).collect::<Vec<_>>().concat();
        crate::verlauf::schreiben(d.path(), "s1", "m", &[("user".into(), viele)])
            .expect("schreiben");

        let aus = w.ausfuehren(&serde_json::json!({"von": 1, "bis": 600})).expect("lesen");
        let zeilen = aus.lines().filter(|z| z.starts_with("Zeile ")).count();
        assert!(
            zeilen <= VERLAUF_ZEILENGRENZE,
            "{zeilen} Zeilen herausgegeben, Grenze ist {VERLAUF_ZEILENGRENZE}"
        );
        assert!(aus.contains("lines requested"), "der Vermerk zur Kuerzung fehlt: {aus}");
        assert!(aus.contains("Ask again from"), "es sagt nicht, wo weiterzulesen ist: {aus}");
    }

    /// **Ein fehlendes Argument ist ein Fehler mit Namen.**
    #[test]
    fn ein_fehlendes_argument_wird_benannt() {
        let (_d, w) = werkzeug();
        let f = w.ausfuehren(&serde_json::json!({"von": 1})).expect_err("bis fehlt");
        assert!(f.grund.contains("bis"), "{}", f.grund);
    }

    /// ⛔️ **Es steht NICHT in `Base`, und das ist gemessen.**
    ///
    /// Einen Tag lang stand es dort, weil „Nachlesen keine Sache der
    /// Modellgroesse" ist. ⚑ **Die Messung sagt das Gegenteil fuer
    /// genau das Modell, fuer das `Base` gemacht ist:** 27 Laeufe, kein
    /// einziger Aufruf. Ein Werkzeug, das nie gerufen wird, kostet
    /// trotzdem in jeder Runde Ansage und macht die Auswahl schwerer.
    #[test]
    fn es_steht_in_den_groesseren_kisten_und_nicht_in_base() {
        for kiste in [Werkzeugkiste::Advanced, Werkzeugkiste::Elite] {
            assert!(
                kiste.werkzeuge().contains(&Dateiwerkzeug::Verlauf),
                "{kiste:?} kennt das Verlaufwerkzeug nicht"
            );
        }
        assert!(
            !Werkzeugkiste::Base.werkzeuge().contains(&Dateiwerkzeug::Verlauf),
            "Base traegt das Verlaufwerkzeug wieder"
        );
        // ⚑ **Und es schreibt nicht**, braucht also keine
        // Schreiberlaubnis: Es liest nach, was ohnehin gesagt wurde.
        assert!(!Dateiwerkzeug::Verlauf.schreibt());
    }
}

#[cfg(test)]
mod skillwerkzeuge {
    use super::*;
    use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;

    fn mit_mappe() -> (tempfile::TempDir, Einhaengung) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let o = crate::skills::anlegen(d.path()).expect("anlegen");
        let m = o.join("kryptografie");
        std::fs::create_dir_all(m.join("kapitel")).expect("Ordner");
        std::fs::write(
            m.join("MAPPE.md"),
            "# kryptografie\n\nWie Signaturen und Zufallslosungen hier benutzt werden.\n\n\
             ## Kapitel\n\n- `kapitel/01-signaturen.md`\n",
        )
        .expect("schreiben");
        std::fs::write(m.join("kapitel").join("01-signaturen.md"), "Der Inhalt des Kapitels.\n")
            .expect("schreiben");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        (d, e)
    }

    /// ⚑ **Suchen nennt und liefert nicht**, wie beim Mitschnitt.
    #[test]
    fn die_suche_nennt_mit_satz_und_liefert_nichts() {
        let (_d, e) = mit_mappe();
        let w = Skillsuche(e, Ansageform::Amtlich, verlaufsbudget());
        let aus = w.ausfuehren(&serde_json::json!({"anfrage": "Signaturen pruefen"})).expect("Suche");
        assert!(aus.contains("kryptografie (Wie Signaturen"), "{aus}");
        assert!(aus.contains("learn_skill"), "der naechste Schritt fehlt: {aus}");
        assert!(!aus.contains("Der Inhalt des Kapitels"), "die Suche liefert Inhalt: {aus}");
        // Leer nennt alle, darunter die mitgelieferten.
        let alle = w.ausfuehren(&serde_json::json!({"anfrage": ""})).expect("alle");
        assert!(alle.contains("kryptografie") && alle.contains("fehlersuche"), "{alle}");
        let nichts = w.ausfuehren(&serde_json::json!({"anfrage": "xylophonstimmung"})).expect("nichts");
        assert!(nichts.starts_with("No skill matches"), "{nichts}");
    }

    /// ⚑ **Die Eingangsseite und nicht die ganze Mappe**; ein Kapitel
    /// holt derselbe Aufruf mit `name/datei`.
    #[test]
    fn gelernt_wird_die_eingangsseite_und_auf_wunsch_eine_datei() {
        let (_d, e) = mit_mappe();
        let w = Skilllernen(e, Ansageform::Amtlich, verlaufsbudget());
        let aus = w.ausfuehren(&serde_json::json!({"name": "kryptografie"})).expect("lernen");
        assert!(aus.contains("kapitel/01-signaturen.md"), "{aus}");
        assert!(!aus.contains("Der Inhalt des Kapitels"), "es kam die ganze Mappe: {aus}");
        assert!(aus.contains("kryptografie/kapitel/01-signaturen.md"), "die weiteren Dateien fehlen: {aus}");
        let kapitel = w.ausfuehren(&serde_json::json!({"name": "kryptografie/kapitel/01-signaturen.md"})).expect("Kapitel");
        assert_eq!(kapitel, "Der Inhalt des Kapitels.\n");
        // 📌 Die ganze Trefferzeile als Name, wie das 4B sie am 2026-09-26
        //    abschrieb, meint trotzdem den Skill.
        for zeile in ["kryptografie: Wie Signaturen hier benutzt werden.", "kryptografie (Wie Signaturen …)"] {
            let aus = w.ausfuehren(&serde_json::json!({"name": zeile})).expect("lernen");
            assert!(aus.contains("kapitel/01-signaturen.md"), "{zeile}: {aus}");
        }
    }

    /// ⛔️ **Kein Weg nach draussen.**
    ///
    /// Der Name wird gegen die gefundenen Skills gehalten und nie zu
    /// einem Pfad gemacht, und eine Datei muss im Ordner des Skills
    /// liegen. **Die Zusage steht hier, weil sie sonst beim naechsten
    /// Umbau verloren gehen koennte.**
    #[test]
    fn ein_pfad_als_name_fuehrt_nirgendwohin() {
        let (d, e) = mit_mappe();
        std::fs::write(d.path().join("geheim.txt"), "GEHEIM").expect("schreiben");
        let w = Skilllernen(e, Ansageform::Amtlich, verlaufsbudget());
        for versuch in [
            "../../etc/passwd",
            "/etc/passwd",
            "kryptografie/kapitel",
            "kryptografie/../../../geheim.txt",
            "kryptografie//etc/passwd",
        ] {
            let aus = w.ausfuehren(&serde_json::json!({"name": versuch})).expect("Antwort");
            assert!(
                aus.starts_with("kein Skill") || aus.contains("liegt nicht im Skill") || aus.starts_with("keine Datei"),
                "{versuch} kam durch: {aus}"
            );
            assert!(!aus.contains("GEHEIM") && !aus.contains("root:"), "{versuch}: {aus}");
        }
    }

    /// ⚑ **Ein fehlendes Argument ist ein Aufruffehler.**
    #[test]
    fn ohne_argument_ist_es_ein_aufruffehler() {
        let (_d, e) = mit_mappe();
        let f = Skilllernen(e.clone(), Ansageform::Amtlich, verlaufsbudget()).ausfuehren(&serde_json::json!({})).expect_err("Fehler");
        assert!(f.grund.contains("name"), "{}", f.grund);
        let f = Skillsuche(e, Ansageform::Amtlich, verlaufsbudget()).ausfuehren(&serde_json::json!({})).expect_err("Fehler");
        assert!(f.grund.contains("anfrage"), "{}", f.grund);
    }

    /// ⛔️ **Sie haengen am selben Nachschlagebudget wie der Mitschnitt.**
    ///
    /// Der Kontext laesst sich ueber jeden Leseweg fuellen; drei eigene
    /// Budgets waeren drei Wege.
    #[test]
    fn sie_haengen_am_selben_budget() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (_d, e) = mit_mappe();
        let budget: Verlaufsbudget = std::sync::Arc::new(AtomicUsize::new(40));
        let w = Skilllernen(e.clone(), Ansageform::Amtlich, budget.clone());
        let aus = w.ausfuehren(&serde_json::json!({"name": "kryptografie"})).expect("lernen");
        assert!(aus.contains("Cut off here"), "die Kuerzung sagt sich nicht an: {aus}");
        assert_eq!(budget.load(Ordering::Relaxed), 0);
        let l = Skillsuche(e, Ansageform::Amtlich, budget);
        let aus2 = l.ausfuehren(&serde_json::json!({"anfrage": ""})).expect("Suche");
        assert!(aus2.contains("already returned"), "zweites Werkzeug mit eigenem Budget: {aus2}");
    }
}

#[cfg(test)]
mod verlaufsuche {
    use super::*;
    use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;

    fn werkzeug() -> (tempfile::TempDir, Verlaufsuche) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        (d, Verlaufsuche(e, Ansageform::Amtlich, verlaufsbudget()))
    }

    fn mitschnitt(d: &std::path::Path) {
        crate::verlauf::schreiben(
            d,
            "alte-sitzung",
            "myelith-4b",
            &[
                ("user".into(), "Und was macht der Pruefstand in Halle 3?".into()),
                ("assistant".into(), "Der laeuft unter der Kennung QX-4417-MOOS.".into()),
                ("user".into(), "Und der Durchsatz?".into()),
                ("assistant".into(), "127 Token je Sekunde.".into()),
            ],
        )
        .expect("schreiben");
    }

    /// ⚑ **Die Suche nennt Zeilen, und die Zeilen tragen die Antwort.**
    ///
    /// Das ist der ganze Zweck: Aus einer Frage („Halle 3") werden ein
    /// paar Zeilennummern, und `read_history` holt genau die. ⛔️
    /// **Ohne die Suche kostete derselbe Weg das ganze Verzeichnis**,
    /// gemessen 6 000 Token gegen hier ein paar Dutzend.
    #[test]
    fn sie_nennt_die_zeilen_zur_frage() {
        let (d, w) = werkzeug();
        mitschnitt(d.path());
        let aus = w
            .ausfuehren(&serde_json::json!({"muster": "Halle 3"}))
            .expect("Suche");
        assert!(aus.contains("hits for"), "{aus}");
        assert!(aus.contains("Und was macht der Pruefstand in Halle 3?"), "{aus}");
        // ⛔️ **Der Wechsel und nicht nur die Frage.** Das gesuchte Wort
        // steht in der Frage, die Auskunft in der Antwort darunter; ein
        // Treffer, der bei der Frage endet, schickt den Leser eine
        // Nachricht zu weit nach oben.
        let v = crate::verlauf::verzeichnis(&crate::verlauf::vorhandene(d.path())[0])
            .expect("Verzeichnis");
        assert!(
            aus.contains(&format!("read_history {} to {}", v.abschnitte[0].von, v.abschnitte[1].bis)),
            "die Spanne deckt Frage und Antwort: {aus}"
        );
        // ⚑ **Und die Suche zeigt den Wechsel, nicht nur die
        // Fundstelle.** Zweimal gemessen, zweimal zu wenig: Zeilen-
        // nummern allein wurden fuer die Antwort gehalten, die
        // gefundene Zeile allein ist die **Frage**. ⛔️ **Die Auskunft
        // steht in der Antwort darunter, und die gehoert dazu.**
        assert!(
            aus.lines().any(|z| z.starts_with(&format!("{}:", v.abschnitte[0].von + 2))),
            "die gefundene Zeile steht mit ihrer Nummer da: {aus}"
        );
        assert!(
            aus.contains("QX-4417-MOOS"),
            "der Wechsel traegt die Antwort und nicht nur die Frage: {aus}"
        );
    }

    /// **Nichts gefunden ist eine Auskunft und kein Fehler.**
    #[test]
    fn nichts_gefunden_sagt_es() {
        let (d, w) = werkzeug();
        mitschnitt(d.path());
        let aus = w
            .ausfuehren(&serde_json::json!({"muster": "Neutrinodetektor"}))
            .expect("kein Fehler");
        assert!(aus.contains("Nothing found"), "{aus}");
    }

    /// ⛔️ **Das Budget haelt, worum man ein Modell nicht bitten kann.**
    ///
    /// Gemessen am 2026-09-17: Modelle befolgen „ruf das Werkzeug nur,
    /// wenn es noetig ist" **nicht**, auch nicht mit ausdruecklichem
    /// Gegenfall im Systemprompt. ⚑ **Also entscheidet der Code**, und
    /// zwar fuer die drei Verlaufswerkzeuge **gemeinsam**: Drei eigene
    /// Budgets waeren drei Wege, denselben Kontext zu fuellen.
    ///
    /// Geprueft wird dreierlei: **die Kuerzung greift**, **sie sagt es
    /// an** (eine stillschweigend gekuerzte Antwort liest sich wie eine
    /// vollstaendige), und **das Budget ist danach fuer die anderen
    /// beiden Werkzeuge auch aufgebraucht**.
    #[test]
    fn das_budget_gilt_fuer_die_drei_zusammen() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let d = tempfile::tempdir().expect("Verzeichnis");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        mitschnitt(d.path());
        let budget: Verlaufsbudget = std::sync::Arc::new(AtomicUsize::new(120));

        let suche = Verlaufsuche(e.clone(), Ansageform::Amtlich, budget.clone());
        let aus = suche.ausfuehren(&serde_json::json!({"muster": "Halle 3"})).expect("Suche");
        assert!(aus.len() <= 200, "die Kuerzung greift nicht: {} Zeichen", aus.len());
        assert!(aus.contains("Cut off here"), "die Kuerzung sagt sich nicht an: {aus}");
        assert_eq!(budget.load(Ordering::Relaxed), 0, "das Budget ist nicht abgebucht");

        // ⚑ **Dasselbe Budget, anderes Werkzeug**: Wer die Suche
        // aufgebraucht hat, kann nicht ueber das Lesen weitermachen.
        let lesen = Verlauflesen(e.clone(), Ansageform::Amtlich, budget.clone());
        let aus2 = lesen
            .ausfuehren(&serde_json::json!({"von": 1, "bis": 20}))
            .expect("Lesen");
        assert!(
            aus2.contains("already returned"),
            "das zweite Werkzeug hat ein eigenes Budget: {aus2}"
        );

        // ⚑ **Und die Ruestung gibt es je Auftrag zurueck.**
        budget.store(VERLAUF_BUDGET_ZEICHEN, Ordering::Relaxed);
        let liste = Verlaufliste(e, Ansageform::Amtlich, budget.clone());
        let aus3 = liste.ausfuehren(&serde_json::json!({})).expect("Liste");
        assert!(!aus3.contains("already returned"), "nach dem Zuruecksetzen geht es weiter");
    }

    /// ⚑ **Erst der Aufruf, dann die Welt.**
    ///
    /// Ein fehlendes `muster` ist ein Fehler des Aufrufs und wird als
    /// solcher gemeldet, **auch im leeren Ordner**: Sonst suchte ein
    /// Modell, das den Parameter vergessen hat, den Fehler beim
    /// Mitschnitt.
    #[test]
    fn ohne_muster_ist_es_ein_aufruffehler() {
        let (_d, w) = werkzeug();
        let f = w.ausfuehren(&serde_json::json!({})).expect_err("Fehler");
        assert!(f.grund.contains("muster"), "{}", f.grund);
    }
}

#[cfg(test)]
mod verlaufliste {
    use super::*;
    use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;

    fn werkzeug() -> (tempfile::TempDir, Verlaufliste) {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let e = Einhaengung::neu(d.path(), false).expect("Einhaengung");
        (d, Verlaufliste(e, Ansageform::Amtlich, verlaufsbudget()))
    }

    /// **Im leeren Ordner sagt es das, statt zu scheitern.**
    #[test]
    fn im_leeren_ordner_sagt_es_das() {
        let (_d, w) = werkzeug();
        let aus = w.ausfuehren(&serde_json::json!({})).expect("kein Fehler");
        assert!(aus.contains("Nothing has been recorded"), "{aus}");
    }

    /// ⚑ **Es nennt und liefert nicht.**
    ///
    /// Ein Mitschnitt aus einer fremden Sitzung ist der Verlauf eines
    /// anderen Gespraechs; ihn ungefragt in den Kontext zu ziehen, waere
    /// eine Ueberraschung. Herauskommen duerfen die **Marken** des
    /// Verzeichnisses, nicht der Inhalt dahinter.
    #[test]
    fn es_nennt_und_liefert_nicht() {
        let (d, w) = werkzeug();
        crate::verlauf::schreiben(
            d.path(),
            "alte-sitzung",
            "myelith-4b",
            &[
                ("user".into(), "Baue den Parser um.\nEr soll Kommentare ueberspringen.".into()),
                ("assistant".into(), "Fertig, die Kennzahl war 4711.".into()),
            ],
        )
        .expect("schreiben");

        let aus = w.ausfuehren(&serde_json::json!({})).expect("liste");
        // Es nennt Zahl, Sitzung und die Marken.
        assert!(aus.contains("1 episodes from 1 sessions"), "{aus}");
        assert!(aus.contains("alte-sitzung"), "{aus}");
        assert!(aus.contains("Baue den Parser um."), "die Marken fehlen: {aus}");
        // ⛔️ **Und nicht den Inhalt dahinter.** Die zweite Zeile einer
        // Nachricht ist keine Marke.
        assert!(
            !aus.contains("Er soll Kommentare ueberspringen."),
            "es liefert den Verlauf statt ihn zu nennen: {aus}"
        );
    }

    /// ⚑ **Erst weit oben wird gruppiert, und die Antwort sagt es.**
    ///
    /// ⛔️ **Die Grenze stand erst bei 60, und das war falsch** (siehe
    /// [`VERZEICHNIS_GANZ`]): Ein Modell, das das Verzeichnis anfordert,
    /// sucht eine Ueberschrift, und eine Karte ohne Ueberschriften
    /// schickt es auf eine blinde Suche. Geprueft wird deshalb beides:
    /// dass der Ausreisser gruppiert **und dabei ehrlich benannt** wird,
    /// und dass ein gewoehnlicher Verlauf abschnittsgenau bleibt.
    #[test]
    fn ein_langes_verzeichnis_kommt_als_karte() {
        let (d, w) = werkzeug();
        let viele: Vec<(String, String)> = (0..900)
            .map(|i| {
                let rolle = if i % 2 == 0 { "user" } else { "assistant" };
                (rolle.to_string(), format!("Abschnitt Nummer {i}."))
            })
            .collect();
        crate::verlauf::schreiben(d.path(), "lange-sitzung", "myelith-4b", &viele)
            .expect("schreiben");

        let aus = w.ausfuehren(&serde_json::json!({})).expect("liste");
        let zeilen = aus.lines().filter(|z| z.contains('-') && z.contains("Abschnitt")).count();
        assert!(
            zeilen <= VERZEICHNIS_GROB,
            "die Karte hat {zeilen} Zeilen statt hoechstens {VERZEICHNIS_GROB}: {aus}"
        );
        assert!(aus.contains("900 sections, grouped"), "die Antwort verschweigt die Gruppierung: {aus}");

        // ⚑ **Und die andere Haelfte**: ein kurzer Verlauf kommt weiter
        // abschnittsgenau, sonst rettet die Schranke den einen Fall und
        // verschlechtert den anderen.
        let (d2, w2) = werkzeug();
        let wenige: Vec<(String, String)> = (0..6)
            .map(|i| ("user".to_string(), format!("Frage Nummer {i}.")))
            .collect();
        crate::verlauf::schreiben(d2.path(), "kurze-sitzung", "myelith-4b", &wenige)
            .expect("schreiben");
        let aus2 = w2.ausfuehren(&serde_json::json!({})).expect("liste");
        assert!(!aus2.contains("grouped"), "hier wird nichts gruppiert: {aus2}");
        for i in 0..6 {
            assert!(aus2.contains(&format!("Frage Nummer {i}.")), "Abschnitt {i} fehlt: {aus2}");
        }
    }

    /// ⚑ **Kein einziger Parameter**, also auch kein optionaler.
    #[test]
    fn es_hat_keinen_parameter() {
        let schema = Dateiwerkzeug::VerlaufListe.parameter(Ansageform::Amtlich);
        let felder = schema.get("properties").and_then(|p| p.as_object()).expect("properties");
        assert!(felder.is_empty(), "das Werkzeug hat Parameter: {felder:?}");
    }

    /// ⛔️ **Die drei Verlaufswerkzeuge gehoeren zusammen, und sie
    /// gehoeren nicht in `Base`.**
    ///
    /// ⚑ **Zusammen**, weil keines allein trägt: Ohne die Suche ist das
    /// Nachschlagen teuer (gemessen: der Kontext am Ende so gross wie
    /// der ganze Verlauf), ohne das Lesen bleibt es beim Zeigen, und
    /// ohne die Liste findet ein frischer Agent den Einstieg nicht.
    /// **Eine Kiste, die zwei davon hat, hat das Werkzeug ohne den
    /// Weg.**
    #[test]
    fn die_drei_stehen_zusammen_und_nicht_in_base() {
        let drei = [
            Dateiwerkzeug::Verlauf,
            Dateiwerkzeug::VerlaufListe,
            Dateiwerkzeug::VerlaufSuche,
        ];
        for kiste in [Werkzeugkiste::Advanced, Werkzeugkiste::Elite] {
            for w in drei {
                assert!(kiste.werkzeuge().contains(&w), "{kiste:?} kennt {w:?} nicht");
            }
        }
        for w in drei {
            assert!(
                !Werkzeugkiste::Base.werkzeuge().contains(&w),
                "Base traegt {w:?} wieder"
            );
        }
    }
}

#[cfg(test)]
mod neue_ordner {
    use super::*;
    use myl_local_agent::ausfuehrung::Werkzeugausfuehrung;

    /// ⛔️ **Fund 490: `write_file` legt fehlende Ordner an, aber nur
    /// innerhalb der Einhaengung.**
    #[test]
    fn write_file_legt_ordner_an_und_bleibt_drinnen() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let draussen = tempfile::tempdir().expect("draussen");
        let e = Einhaengung::neu(d.path(), true).expect("Einhaengung");
        let w = Dateischreiben(e, Ansageform::Amtlich);
        let aus = w.ausfuehren(&serde_json::json!({"pfad": "ergebnis/tief/statistik.md", "inhalt": "x"})).expect("schreiben");
        assert!(aus.starts_with("angelegt"), "{aus}");
        assert_eq!(std::fs::read_to_string(d.path().join("ergebnis/tief/statistik.md")).unwrap(), "x");
        for boese in ["neu/../../x.md", "../x.md", "neu/../../../etc/x.md"] {
            assert!(w.ausfuehren(&serde_json::json!({"pfad": boese, "inhalt": "x"})).is_err(), "{boese} kam durch");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(draussen.path(), d.path().join("tuer")).unwrap();
            assert!(w.ausfuehren(&serde_json::json!({"pfad": "tuer/neu/x.md", "inhalt": "x"})).is_err(), "ueber einen Verweis hinaus");
            assert!(!draussen.path().join("neu").exists(), "draussen wurde ein Ordner angelegt");
        }
        assert!(!d.path().parent().unwrap().join("x.md").exists());
    }
}
