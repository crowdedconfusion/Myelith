//! **Programm und Gewichte finden**, bevor irgendetwas gerechnet wird.
//!
//! # ⛔️ Erst suchen, dann rechnen
//!
//! **Eine Voraussetzung, die erst beim Absturz sichtbar wird, ist keine
//! Voraussetzung, sondern eine Falle.** Fehlt ein Programm oder eine
//! Gewichtsdatei, entsteht hier ein [`Mangel`], der sagt **was** fehlt,
//! **wo** gesucht wurde und **wie** es hinkommt. Er geht als Text an das
//! Modell und an den Menschen; keiner von beiden bekommt stattdessen
//! eine Meldung aus llama.cpp, die den Grund nicht nennt.
//!
//! # ⚑ Wo die Gewichte liegen, und in welcher Reihenfolge gesucht wird
//!
//! **Umgezogen am 2026-09-21** (Festlegung des Projektinhabers): Alle
//! fremden Gewichte dieses Projekts liegen im Klon unter `MODELS/`, nach
//! Rubrik getrennt, und die Sinne nehmen `MODELS/audio` und
//! `MODELS/vision`. Sie sind dort **nicht versioniert**.
//!
//! Gesucht wird in drei Stufen, siehe [`gewichtsorte`]:
//!
//! 1. **`MYL_SINNE`**, falls gesetzt. Dann liegt dort **alles
//!    zusammen**, und keine andere Stufe greift.
//! 2. **`MODELS/audio` und `MODELS/vision`** im gefundenen Klon.
//! 3. **`~/.myelith/sinne`** als Rueckfall.
//!
//! ⛔️ **Die dritte Stufe ist kein Altlastenrest, sondern der einzige
//! Weg fuer ein ausgeliefertes Programm.** Wer nur die Freigabebuendel
//! geholt hat, hat keinen Klon und damit kein `MODELS/`; ohne den
//! Rueckfall fande er gar nichts und bekaeme einen Mangel, der auf ein
//! Verzeichnis zeigt, das es bei ihm nicht gibt.
//!
//! ⚑ **Was NICHT umgezogen ist, und der Schnitt ist Absicht.** In der
//! Heimat bleiben der `bin`-Ordner (das Sprech- und das Aufnahmeskript,
//! die diese Kiste selbst schreibt) und eine abgelegte Stimmprobe. Das
//! eine ist erzeugt, das andere gehoert dem Nutzer; **Gewichte sind
//! geladen**, und nur die haben einen Rubrikordner verdient. Deshalb
//! gibt es zwei Begriffe: [`heimat`] fuer den Betrieb, [`gewichtsorte`]
//! fuer die Gewichte.
//!
//! ⚑ **Welches Modell taugt, entscheidet weiter der Nutzer**: Eine
//! Empfehlung im Text veraltet billiger als eine im Code.

use std::path::{Path, PathBuf};

/// Die Umgebungsvariable, die die Heimat der Sinnesmodelle umstellt.
///
/// ⚑ **Sie schlaegt alle anderen Stufen**, und zwar vollstaendig: Wer
/// sie setzt, sagt damit, dass dort alles zusammenliegt. Eine Variable,
/// die nur die halbe Suche uebergeht, waere schlimmer als keine.
pub const HEIMAT_UMGEBUNG: &str = "MYL_SINNE";

/// Das Verzeichnis im Klon, in dem **alle** fremden Gewichte liegen.
pub const MODELLE_ORDNER: &str = "MODELS";
/// Die Rubrik fuer Hoeren und Sprechen.
pub const RUBRIK_AUDIO: &str = "audio";
/// Die Rubrik fuer Sehen.
pub const RUBRIK_VISION: &str = "vision";

/// Die Namen, unter denen ein ausgepacktes CosyVoice zu finden ist.
///
/// ⛔️ **Zwei Schreibweisen, und das ist kein Schoenheitsfehler.** Wer
/// das Projekt klont, bekommt `CosyVoice`; in der Sinnesheimat lag
/// bisher ein `cosyvoice`. **macOS und Windows unterscheiden die beiden
/// nicht, Linux tut es**, und eine Suche mit nur einer Schreibweise
/// faellt genau auf dem System auf, auf dem niemand sie geschrieben hat
/// (dieselbe Klasse wie Fund 392).
pub const COSYVOICE_NAMEN: [&str; 2] = ["CosyVoice", "cosyvoice"];

/// Die Dateinamen, unter denen die Gewichte in der Heimat erwartet werden.
pub const SEHMODELL: &str = "sehen.gguf";
/// Siehe [`SEHMODELL`]. Der Projektor bildet Bildmarken auf den Sprachraum ab.
pub const SEHPROJEKTOR: &str = "sehen-mmproj.gguf";
/// Das **genaue** Sehmodell, falls jemand zwei hinlegt. Siehe [`Stufe`].
pub const SEHMODELL_GENAU: &str = "sehen-genau.gguf";
/// Siehe [`SEHMODELL_GENAU`].
pub const SEHPROJEKTOR_GENAU: &str = "sehen-genau-mmproj.gguf";
/// Siehe [`SEHMODELL`]. Ein ggml-Modell fuer whisper.cpp.
pub const HOERMODELL: &str = "hoeren.bin";

/// Die Programme, die zum Sehen taugen, in der Reihenfolge der Suche.
///
/// ⚠️ **Eine Wette auf die Benennung in llama.cpp.** Das Projekt hat
/// sein Multimodal-Programm schon einmal umbenannt; deshalb stehen die
/// alten Namen mit hier, und deshalb ist **diese Liste** die eine
/// Stelle, an der ein weiterer Name nachzutragen ist.
pub const SEHPROGRAMME: [&str; 4] = ["llama-mtmd-cli", "mtmd-cli", "llama-llava-cli", "llava-cli"];
/// Siehe [`SEHPROGRAMME`], fuer whisper.cpp.
pub const HOERPROGRAMME: [&str; 3] = ["whisper-cli", "whisper-cpp", "whisper"];
/// Siehe [`SEHPROGRAMME`], fuer piper.
pub const SPRECHPROGRAMME: [&str; 2] = ["piper", "piper-tts"];
/// Die Namen, unter denen ein Python zu finden ist.
pub const PYTHONPROGRAMME: [&str; 2] = ["python3", "python"];
/// Wo CosyVoice ausgepackt liegt. ⚑ **Ohne Vorgabe im Repositorium**
/// (Festlegung des Projektinhabers, 2026-09-17): Python und die
/// Gewichte bringt der Nutzer mit, sie gehoeren nicht hierher.
pub const COSYVOICE_UMGEBUNG: &str = "MYL_COSYVOICE";
/// Der Laeufer, den diese Kiste selbst mitbringt und bei Bedarf in die
/// Heimat schreibt. Siehe `sprechen::laeufer_einrichten`.
pub const COSYVOICE_LAEUFER: &str = "sprechen-cosyvoice.py";
/// Der Text der Stimmprobe, von whisper mitgeschrieben.
///
/// ⚑ **CosyVoice braucht ihn fuer `inference_zero_shot`.** Fehlt er,
/// bleibt `inference_cross_lingual`, das ohne auskommt und etwas
/// schlechter trifft.
pub const STIMMPROBE_TEXT: &str = "stimme.txt";
/// Die Stimme fuer piper, ein ONNX-Modell.
pub const SPRECHMODELL: &str = "sprechen.onnx";
/// **Das eigene Sprechskript**, im `bin` der Heimat: Es bekommt eine
/// Textdatei und ein Ziel-WAV und sonst nichts. Die Tuer fuer jedes
/// Sprechprogramm, das nicht piper ist (etwa CosyVoice).
pub const SPRECHSKRIPT: &str = "sprechen";
/// **Die Stimmprobe**: eine Aufnahme, die als Stimme dienen soll.
///
/// ⛔️ **Nicht jedes Sprechprogramm kann damit etwas anfangen.** piper
/// nimmt fertige Stimmen und kennt keine Referenzaufnahme; klonen
/// koennen CosyVoice, XTTS-v2 und F5-TTS, und die laufen ueber das
/// Skript ([`SPRECHSKRIPT`]). Liegt hier eine Probe und spricht trotzdem
/// piper, **sagt der Client das**, statt die Datei stillschweigend
/// liegen zu lassen: Sonst waere die Einstellung ein Schalter ohne
/// Wirkung, und das faellt erst dem auf, der genau hinhoert.
pub const STIMMPROBE: &str = "stimme.wav";

/// **Das eigene Aufnahmeskript**, im `bin` der Heimat: Es bekommt **ein**
/// Ziel-WAV und nimmt auf, **bis seine Standardeingabe schliesst**.
///
/// ⚑ **Das Schliessen ist der Stopp, und das ist Absicht.** Ein Signal zu
/// schicken braeuchte eine Fremdkiste (`libc`), und `Child::kill` ist
/// SIGKILL: Das liesse eine halbe WAV-Datei zurueck. Ein Programm, das
/// auf seine Eingabe hoert, kann sauber abschliessen.
pub const AUFNAHMESKRIPT: &str = "aufnehmen";

/// **Der Arbeitsort der Sinne**: der `bin`-Ordner und die Stimmprobe.
///
/// ⚠️ **Nicht mehr der Ort der Gewichte**, seit dem 2026-09-21. Wer
/// Gewichte sucht, nimmt [`gewichtsorte`]; wer etwas **schreibt**, nimmt
/// diesen hier, denn er ist der einzige, den es auch ohne Klon gibt.
pub fn heimat() -> PathBuf {
    match std::env::var_os(HEIMAT_UMGEBUNG).filter(|w| !w.is_empty()) {
        Some(w) => PathBuf::from(w),
        None => heimatvorgabe(),
    }
}

/// **Die Orte, an denen nach Gewichten gesucht wird**, in der
/// Reihenfolge der Suche. Der erste, an dem eine Datei liegt, gewinnt.
///
/// Die drei Stufen und ihre Begruendung stehen im Modulkopf.
///
/// ⚑ **Die Liste ist nie leer.** Auch ohne Klon steht die Heimat darin,
/// und damit hat jeder Mangel einen Ort zu nennen, an den etwas gehoert.
pub fn gewichtsorte() -> Vec<PathBuf> {
    let gesetzt = std::env::var_os(HEIMAT_UMGEBUNG)
        .filter(|w| !w.is_empty())
        .map(PathBuf::from);
    gewichtsorte_aus(gesetzt.as_deref(), crate::ort::wurzel().as_deref(), &heimatvorgabe())
}

/// **Dieselbe Reihenfolge gegen gesagte Orte.**
///
/// ⚑ **Eigene Funktion, damit sie sich pruefen laesst.** [`gewichtsorte`]
/// fragt die Umgebung und sucht die Wurzel; eine Probe darueber muesste
/// beides stellen und liefe allen Proben daneben ins Gehege, denn
/// `cargo test` laeuft nebenlaeufig **im selben Prozess** und die
/// Umgebung ist allen gemeinsam. **Was entschieden wird, steht hier; was
/// ermittelt wird, steht dort.**
pub fn gewichtsorte_aus(
    gesetzte_heimat: Option<&Path>,
    wurzel: Option<&Path>,
    heimatvorgabe: &Path,
) -> Vec<PathBuf> {
    // 1. Wer die Variable setzt, meint sie, und meint alles.
    if let Some(w) = gesetzte_heimat {
        return vec![w.to_path_buf()];
    }
    let mut aus = Vec::new();
    // 2. Der Klon, wenn es einen gibt. Beide Rubriken, denn eine Datei
    //    hier ist an ihrem Namen erkannt und nicht an ihrem Ordner: Wer
    //    ein Sehmodell nach `audio` legt, soll es wiederfinden.
    if let Some(w) = wurzel {
        let m = w.join(MODELLE_ORDNER);
        aus.push(m.join(RUBRIK_VISION));
        aus.push(m.join(RUBRIK_AUDIO));
    }
    // 3. Die Heimat als Rueckfall, fuer das ausgelieferte Programm.
    aus.push(heimatvorgabe.to_path_buf());
    aus
}

fn heimatvorgabe() -> PathBuf {
    let heim = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    heim.join(".myelith").join("sinne")
}

/// Was ein Nutzer ausdruecklich gesetzt hat, Datei fuer Datei.
///
/// ⚑ **Jede einzeln umstellbar.** Wer ein Sehmodell woanders liegen hat
/// und den Rest in der Heimat, soll nicht alles umziehen muessen.
#[derive(Debug, Clone, Default)]
pub struct Eigene {
    pub seher: Option<PathBuf>,
    pub sehmodell: Option<PathBuf>,
    pub sehprojektor: Option<PathBuf>,
    pub sehmodell_genau: Option<PathBuf>,
    pub sehprojektor_genau: Option<PathBuf>,
    pub hoerer: Option<PathBuf>,
    pub hoermodell: Option<PathBuf>,
    pub sprecher: Option<PathBuf>,
    pub sprechmodell: Option<PathBuf>,
    pub aufnehmer: Option<PathBuf>,
    /// Woher ffmpeg den Ton nimmt: Format und Geraet, etwa
    /// `("avfoundation", ":0")`. Ohne Angabe die Vorgabe dieses Systems.
    pub tonquelle: Option<(String, String)>,
    /// Das Programm, das ein Einzelbild aufnimmt.
    pub blicker: Option<PathBuf>,
    /// Woher der Bildschirm kommt, falls ffmpeg ihn holt.
    pub schirmquelle: Option<(String, String)>,
    /// Woher die Kamera kommt: Format und Geraet.
    pub kameraquelle: Option<(String, String)>,
    /// Das Programm, das aus einem PDF Text macht. Es bekommt den Pfad
    /// als letztes Argument und schreibt nach stdout.
    pub schriftleser: Option<PathBuf>,
}

impl Eigene {
    /// Die Angaben aus der Umgebung.
    pub fn aus_umgebung() -> Self {
        let lies = |name: &str| {
            std::env::var_os(name).filter(|w| !w.is_empty()).map(PathBuf::from)
        };
        Self {
            seher: lies("MYL_SEHER"),
            sehmodell: lies("MYL_SEHMODELL"),
            sehprojektor: lies("MYL_SEHPROJEKTOR"),
            sprecher: lies("MYL_SPRECHER"),
            sprechmodell: lies("MYL_SPRECHMODELL"),
            aufnehmer: lies("MYL_AUFNEHMER"),
            tonquelle: match (std::env::var("MYL_TONFORMAT"), std::env::var("MYL_TONGERAET")) {
                (Ok(f), Ok(g)) if !f.is_empty() && !g.is_empty() => Some((f, g)),
                _ => None,
            },
            blicker: lies("MYL_BLICKER"),
            schirmquelle: match (std::env::var("MYL_SCHIRMFORMAT"), std::env::var("MYL_SCHIRMGERAET")) {
                (Ok(f), Ok(g)) if !f.is_empty() && !g.is_empty() => Some((f, g)),
                _ => None,
            },
            kameraquelle: match (std::env::var("MYL_KAMERAFORMAT"), std::env::var("MYL_KAMERAGERAET")) {
                (Ok(f), Ok(g)) if !f.is_empty() && !g.is_empty() => Some((f, g)),
                _ => None,
            },
            sehmodell_genau: lies("MYL_SEHMODELL_GENAU"),
            sehprojektor_genau: lies("MYL_SEHPROJEKTOR_GENAU"),
            hoerer: lies("MYL_HOERER"),
            hoermodell: lies("MYL_HOERMODELL"),
            schriftleser: lies("MYL_SCHRIFTLESER"),
        }
    }
}

/// Was zum Sehen gebraucht wird, alles vorhanden.
#[derive(Debug, Clone)]
pub struct Sehzeug {
    pub programm: PathBuf,
    pub modell: PathBuf,
    pub projektor: PathBuf,
}

/// **Wie genau hingesehen werden soll.**
///
/// # ⚑ Zwei Sprossen, und kein Parameter fuer das Modell
///
/// Festlegung des Projektinhabers vom 2026-09-17: ein kleines schnelles
/// Sehmodell (SmolVLM) und ein groesseres genaues (Qwen-VL), je nach
/// Anwendungsfall. **Die Wahl trifft der Aufrufer und nicht das
/// Modell**, denn ein zusaetzliches Argument kostet jedes kleine Modell
/// eine Entscheidung, die es schlecht trifft:
///
/// - **Beim Anhaengen `Schnell`.** Das ist ein Blick fuer jeden, auch
///   fuer den, der gar nichts fragen wollte.
/// - **Beim Werkzeugaufruf `Genau`.** Wer ausdruecklich eine Frage
///   stellt, will die bessere Antwort.
///
/// ⚑ **Wer nur eines hinlegt, bekommt es fuer beides.** Eine Stufe, die
/// mangels Gewichten leer ausginge, waere eine Falle; hier faellt sie
/// auf die andere zurueck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stufe {
    /// Klein und schnell, fuer den ersten Blick.
    Schnell,
    /// Groesser und genauer, fuer eine gestellte Frage.
    Genau,
}

/// Was zum Sehen da ist, auf einer oder beiden Sprossen.
#[derive(Debug, Clone)]
pub struct Sehen {
    pub schnell: Option<Sehzeug>,
    pub genau: Option<Sehzeug>,
}

impl Sehen {
    /// **Das Zeug fuer diese Stufe**, sonst das der anderen.
    pub fn fuer(&self, stufe: Stufe) -> &Sehzeug {
        let (erst, dann) = match stufe {
            Stufe::Schnell => (&self.schnell, &self.genau),
            Stufe::Genau => (&self.genau, &self.schnell),
        };
        erst.as_ref().or(dann.as_ref()).expect("ein Sehen ohne ein einziges Zeug entsteht nicht")
    }

    /// Ob auf beiden Sprossen etwas steht.
    pub fn zweistufig(&self) -> bool {
        self.schnell.is_some() && self.genau.is_some()
    }
}

/// Was zum Hoeren gebraucht wird, alles vorhanden.
#[derive(Debug, Clone)]
pub struct Hoerzeug {
    pub programm: PathBuf,
    pub modell: PathBuf,
}

/// Was zum Aufnehmen gebraucht wird.
///
/// ⚑ **`quelle` ist leer, wenn ein eigenes Skript aufnimmt**: Das kennt
/// sein Geraet selbst. Bei ffmpeg steht hier, woher der Ton kommt.
#[derive(Debug, Clone)]
pub struct Aufnahmezeug {
    pub programm: PathBuf,
    pub quelle: Option<(String, String)>,
}

/// **Was gebraucht wird, um den Bildschirm einmal abzulichten.**
///
/// # ⚑ Zwei Wege, und der erste ist auf macOS der bessere
///
/// `screencapture` liegt auf macOS bei, braucht kein Format und kein
/// Geraet und kostet keine halbe Sekunde. Ueberall sonst tut es ffmpeg,
/// das fuer den Ton ohnehin schon gebraucht wird (`x11grab` unter X11,
/// `gdigrab` unter Windows).
///
/// ⚠️ **`quelle` ist `None`, wenn das Programm die Quelle selbst
/// kennt.** Genau daran unterscheidet [`crate::blick`] die beiden Wege,
/// statt den Programmnamen zu lesen: Wer den Namen prueft, prueft eine
/// Schreibweise, und ein eigenes Skript heisst anders.
#[derive(Debug, Clone)]
pub struct Bildschirmzeug {
    pub programm: PathBuf,
    pub quelle: Option<(String, String)>,
}

/// **Was gebraucht wird, um ein Kamerabild zu holen.**
///
/// ⚠️ **Hier gibt es keinen Weg ohne Quelle.** Eine Kamera hat auf jedem
/// System ein Format und ein Geraet, und welches gemeint ist, weiss nur
/// die Maschine: `MYL_KAMERAFORMAT` und `MYL_KAMERAGERAET` sagen es,
/// sonst gilt [`kameraquelle_vorgabe`].
#[derive(Debug, Clone)]
pub struct Kamerazeug {
    pub programm: PathBuf,
    pub quelle: (String, String),
}

/// **Wie gesprochen wird.**
///
/// # ⚑ Drei Wege, und die Reihenfolge ist eine Entscheidung
///
/// 1. **Ein eigenes Skript** gewinnt immer: Wer es hinlegt, hat gewaehlt.
/// 2. **CosyVoice**, die Vorgabe seit dem 2026-09-17 (Festlegung des
///    Projektinhabers). Es klingt am besten und **kann eine Stimme
///    nachbilden**; dafuer braucht es Python und Gewichte, die der
///    Nutzer mitbringt.
/// 3. **piper** als Rueckfall: eine Binaerdatei, sofort da, aber es
///    nimmt nur fertige Stimmen.
#[derive(Debug, Clone)]
pub enum Sprechweg {
    /// `<Heimat>/bin/sprechen`, mit dem festen Interface aus dem
    /// Modulkopf von `sprechen`.
    Skript(PathBuf),
    /// Python, der mitgelieferte Laeufer und CosyVoice.
    CosyVoice {
        python: PathBuf,
        laeufer: PathBuf,
        wurzel: PathBuf,
    },
    /// piper mit einer fertigen Stimme.
    Piper { programm: PathBuf, stimme: PathBuf },
}

/// Was zum Sprechen gebraucht wird.
#[derive(Debug, Clone)]
pub struct Sprechzeug {
    pub weg: Sprechweg,
    /// Die Stimmprobe, falls eine liegt.
    pub probe: Option<PathBuf>,
    /// Der mitgeschriebene Text der Stimmprobe, falls es ihn gibt.
    pub probentext: Option<PathBuf>,
}

impl Sprechzeug {
    /// Ob dieser Weg eine Stimmprobe ueberhaupt verwerten kann.
    ///
    /// ⛔️ **piper kann es nicht**, und das ist sicher: Es nimmt fertige
    /// Stimmen und kennt keine Referenzaufnahme.
    pub fn kann_klonen(&self) -> bool {
        !matches!(self.weg, Sprechweg::Piper { .. })
    }

    /// ⚑ **Ob der Weg ein Dauerlaeufer sein sollte.** CosyVoice laedt je
    /// Aufruf ein halbes Milliardenmodell; **satzweise zu sprechen waere
    /// damit langsamer als gar nicht zu streamen**, wenn jeder Satz
    /// einen neuen Prozess braeuchte.
    pub fn dauerhaft(&self) -> bool {
        matches!(self.weg, Sprechweg::CosyVoice { .. })
    }

    /// Wie der Weg heisst, fuer Meldungen.
    pub fn name(&self) -> String {
        match &self.weg {
            Sprechweg::Skript(p) => p.display().to_string(),
            Sprechweg::CosyVoice { .. } => "CosyVoice".into(),
            Sprechweg::Piper { .. } => "piper".into(),
        }
    }
}

/// **Was fehlt, wo gesucht wurde, und wie es hinkommt.**
#[derive(Debug, Clone)]
pub struct Mangel {
    /// Der Sinn, um den es geht, fuer die erste Zeile.
    pub sinn: &'static str,
    /// Je Stueck: wie es heisst und wo vergeblich gesucht wurde.
    pub fehlt: Vec<(String, String)>,
    /// Die Schritte, die es herbeischaffen.
    pub anleitung: Vec<String>,
}

impl Mangel {
    /// Der Text, den Modell und Mensch zu sehen bekommen.
    pub fn bericht(&self) -> String {
        let mut t = format!("Es ist kein {} eingerichtet, deshalb wurde nichts angesehen oder angehoert.\n", self.sinn);
        for (was, wo) in &self.fehlt {
            t.push_str(&format!("  {was} fehlt ({wo})\n"));
        }
        t.push_str("Einmalig einzurichten, danach laeuft es ohne Netz:\n");
        for (i, z) in self.anleitung.iter().enumerate() {
            t.push_str(&format!("  {}. {z}\n", i + 1));
        }
        t
    }
}

/// **Beide Sinne, so wie sie hier gerade dastehen.**
///
/// ⚑ **Billig zu haben**, ein paar Dateiabfragen. Deshalb darf der
/// Client vor jeder Anzeige neu fragen, statt einen Zustand zu halten,
/// der veraltet, sobald jemand ein Modell dorthin legt.
#[derive(Debug, Clone)]
pub struct Sinne {
    pub sehen: Result<Sehen, Mangel>,
    pub hoeren: Result<Hoerzeug, Mangel>,
    pub sprechen: Result<Sprechzeug, Mangel>,
    pub aufnehmen: Result<Aufnahmezeug, Mangel>,
    /// **Ein Einzelbild vom Bildschirm.** Siehe [`Bildschirmzeug`].
    pub bildschirm: Result<Bildschirmzeug, Mangel>,
    /// **Ein Einzelbild aus der Kamera.** Siehe [`Kamerazeug`].
    pub kamera: Result<Kamerazeug, Mangel>,
    /// **Schrift aus einem PDF.** Siehe [`crate::schrift`].
    pub schrift: Result<crate::schrift::Schriftzeug, Mangel>,
}

impl Sinne {
    /// Aus Umgebung und Vorgaben.
    pub fn finden() -> Self {
        Self::finden_mit(&heimat(), &gewichtsorte(), &Eigene::aus_umgebung(), &pfadordner())
    }

    /// **Dieselbe Suche, aber alles unter EINEM Ort.**
    ///
    /// ⚑ **Die Naht fuer die Proben** (und fuer jeden, der die Sinne
    /// woandershin legen will). Eine Probe, die dafuer die Umgebung des
    /// Prozesses veraendert, veraendert sie fuer alle Proben daneben:
    /// `cargo test` laeuft nebenlaeufig im selben Prozess.
    ///
    /// ⚑ **Das ist zugleich der Fall `MYL_SINNE`**, und deshalb bleibt
    /// diese Naht so, wie sie war: Wer die Variable setzt, bekommt genau
    /// diese Suche.
    pub fn finden_in(heimat: &Path, eigen: &Eigene, pfad: &[PathBuf]) -> Self {
        Self::finden_mit(heimat, &[heimat.to_path_buf()], eigen, pfad)
    }

    /// **Die Suche mit getrenntem Arbeitsort und Gewichtsorten.**
    ///
    /// `heimat` traegt den `bin`-Ordner und die Stimmprobe, `gewichte`
    /// die Modelldateien, in der Reihenfolge der Suche.
    ///
    /// ⚑ **Warum das zwei Angaben sind und nicht eine** (2026-09-21):
    /// Die Gewichte sind in den Klon gezogen, das Erzeugte und die
    /// Nutzerdaten sind geblieben. Wer beides aus einem Pfad ableitete,
    /// muesste sich fuer einen der zwei Orte entscheiden und haette
    /// entweder ein Skript im Rubrikordner oder ein Modell, das niemand
    /// findet.
    pub fn finden_mit(
        heimat: &Path,
        gewichte: &[PathBuf],
        eigen: &Eigene,
        pfad: &[PathBuf],
    ) -> Self {
        Self {
            sehen: sehen(heimat, gewichte, eigen, pfad),
            hoeren: hoerzeug(heimat, gewichte, eigen, pfad),
            sprechen: sprechzeug(heimat, gewichte, eigen, pfad),
            aufnehmen: aufnahmezeug(heimat, eigen, pfad),
            bildschirm: bildschirmzeug(heimat, eigen, pfad),
            kamera: kamerazeug(heimat, eigen, pfad),
            schrift: schriftzeug(heimat, eigen, pfad),
        }
    }

    /// Ob der Client antworten kann, statt nur zu schreiben.
    pub fn kann_sprechen(&self) -> bool {
        self.sprechen.is_ok()
    }

    /// **Was zur Stimmprobe zu sagen ist**, wenn etwas zu sagen ist.
    ///
    /// ⚑ **Ein Schalter ohne Wirkung ist schlimmer als keiner.** Wer
    /// eine Stimme hochlaedt und weiter dieselbe hoert, sucht den Fehler
    /// bei sich.
    pub fn stimmhinweis(&self) -> Option<String> {
        let z = self.sprechen.as_ref().ok()?;
        let probe = z.probe.as_ref()?;
        if z.kann_klonen() {
            // ⚠️ **Ein zweiter Fall, der still danebengeht:** CosyVoice
            // trifft die Stimme deutlich besser, wenn der Text der Probe
            // dabeisteht. Fehlt er, laeuft es trotzdem, nur schlechter.
            if matches!(z.weg, Sprechweg::CosyVoice { .. }) && z.probentext.is_none() {
                return Some(format!(
                    "Zur Stimmprobe fehlt ihr Text ({}). CosyVoice trifft die Stimme damit \
                     besser; ohne ihn wird der sprachuebergreifende Weg genommen. Ein \
                     Hoermodell schreibt ihn beim Ablegen von selbst mit.",
                    heimat().join(STIMMPROBE_TEXT).display()
                ));
            }
            return None;
        }
        Some(format!(
            "Es liegt eine Stimmprobe ({}), aber {} kann keine Stimme nachbilden und \
             ignoriert sie. Klonen kann CosyVoice, und dafuer muss {} auf eine \
             Installation zeigen.",
            probe.display(),
            z.name(),
            COSYVOICE_UMGEBUNG
        ))
    }

    /// **Ob die Sprechtaste geht**: aufnehmen und mitschreiben, beides.
    ///
    /// ⚑ **Beides oder nichts.** Eine Taste, die aufnimmt und dann
    /// niemanden hat, der mitschreibt, ist eine Taste, die Tonmuell
    /// erzeugt.
    pub fn kann_zuhoeren(&self) -> bool {
        self.aufnehmen.is_ok() && self.hoeren.is_ok()
    }

    /// Ob fuer diese Art Datei hier jemand hinsieht.
    pub fn bereit(&self, art: crate::anhang::Art) -> bool {
        match art {
            crate::anhang::Art::Bild => self.sehen.is_ok(),
            crate::anhang::Art::Ton => self.hoeren.is_ok(),
            _ => false,
        }
    }
}

/// **Die ueblichen Orte, an denen Paketverwalter ablegen.**
///
/// # ⛔️ Warum das noetig ist, und es ist kein Komfort
///
/// **Ein aus dem Finder gestartetes `Myelith.app` erbt den PATH der
/// Anmeldesitzung, nicht den der Shell.** Darin steht
/// `/usr/bin:/bin:/usr/sbin:/sbin` und sonst nichts; `/opt/homebrew/bin`
/// fehlt. Wer llama.cpp mit `brew` installiert, findet es im Terminal
/// und im Fenster **nicht**, und es gibt keine Fehlermeldung, die das
/// sagt: Der Sinn meldet nur, das Programm fehle.
///
/// 📌 **Dieselbe Falle wie bei den Werkzeugkisten** (Fund vom
/// 2026-09-15): Eine Suche, die nur vom Arbeitsverzeichnis ausgeht,
/// geht fuer das installierte Fenster immer ins Leere. Aufgefallen ist
/// es hier beim tatsaechlichen Einrichten, nicht beim Schreiben.
pub const UEBLICHE_ORTE: [&str; 4] =
    ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin"];

/// Die Ordner aus `PATH`, gefolgt von den ueblichen Orten.
///
/// ⚑ **PATH zuerst.** Wer etwas ausdruecklich in seinen Pfad legt, hat
/// gewaehlt; die ueblichen Orte sind der Rueckfall fuer den Fall, dass
/// gar kein brauchbarer PATH da ist.
pub fn pfadordner() -> Vec<PathBuf> {
    let mut aus: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for o in UEBLICHE_ORTE {
        let o = PathBuf::from(o);
        if !aus.contains(&o) {
            aus.push(o);
        }
    }
    aus
}

fn sehen(
    heimat: &Path,
    gewichte: &[PathBuf],
    eigen: &Eigene,
    pfad: &[PathBuf],
) -> Result<Sehen, Mangel> {
    let programm = programm_suchen(&SEHPROGRAMME, eigen.seher.as_deref(), heimat, pfad);
    let sprosse = |modell: Option<&Path>, projektor: Option<&Path>, mname: &str, pname: &str| {
        let m = datei_suchen_in(modell, gewichte, mname);
        let p = datei_suchen_in(projektor, gewichte, pname);
        match (programm.as_ref(), m, p) {
            (Some(prog), Some(m), Some(p)) => {
                Some(Sehzeug { programm: prog.clone(), modell: m, projektor: p })
            }
            _ => None,
        }
    };
    let schnell = sprosse(
        eigen.sehmodell.as_deref(),
        eigen.sehprojektor.as_deref(),
        SEHMODELL,
        SEHPROJEKTOR,
    );
    let genau = sprosse(
        eigen.sehmodell_genau.as_deref(),
        eigen.sehprojektor_genau.as_deref(),
        SEHMODELL_GENAU,
        SEHPROJEKTOR_GENAU,
    );
    if schnell.is_some() || genau.is_some() {
        return Ok(Sehen { schnell, genau });
    }

    // ⚑ **Der Mangel nennt die gewoehnliche Sprosse.** Wer gar nichts
    // hat, soll ein Modell hinlegen und nicht erst zwei Namen auseinander
    // halten muessen; die zweite Sprosse steht im README.
    let mut fehlt = Vec::new();
    if programm.is_none() {
        fehlt.push(("Programm".into(), format!("{} aus llama.cpp", SEHPROGRAMME[0])));
    }
    if datei_suchen_in(eigen.sehmodell.as_deref(), gewichte, SEHMODELL).is_none() {
        fehlt.push(("Modell".into(), erster_ort(gewichte, SEHMODELL).display().to_string()));
    }
    if datei_suchen_in(eigen.sehprojektor.as_deref(), gewichte, SEHPROJEKTOR).is_none() {
        fehlt.push(("Projektor".into(), erster_ort(gewichte, SEHPROJEKTOR).display().to_string()));
    }
    Err(Mangel {
        sinn: "Sehmodell",
        fehlt,
        anleitung: vec![
            format!("llama.cpp installieren ({})", einbauhinweis("llama.cpp", "llama-cpp")),
            format!("Den Ordner anlegen: {}", erster_ort(gewichte, "").display()),
            format!(
                "Ein kleines Sehmodell als GGUF samt mmproj dorthin legen, benannt {SEHMODELL} und {SEHPROJEKTOR}. \
                 Klein und brauchbar ist zum Beispiel SmolVLM2-2.2B-Instruct. Wer zusaetzlich ein groesseres \
                 will (etwa Qwen3-VL-4B), legt es als {SEHMODELL_GENAU} und {SEHPROJEKTOR_GENAU} daneben."
            ),
        ],
    })
}

fn hoerzeug(
    heimat: &Path,
    gewichte: &[PathBuf],
    eigen: &Eigene,
    pfad: &[PathBuf],
) -> Result<Hoerzeug, Mangel> {
    let programm = programm_suchen(&HOERPROGRAMME, eigen.hoerer.as_deref(), heimat, pfad);
    let modell = datei_suchen_in(eigen.hoermodell.as_deref(), gewichte, HOERMODELL);
    if let (Some(programm), Some(modell)) = (&programm, &modell) {
        return Ok(Hoerzeug { programm: programm.clone(), modell: modell.clone() });
    }
    let mut fehlt = Vec::new();
    if programm.is_none() {
        fehlt.push(("Programm".into(), format!("{} aus whisper.cpp", HOERPROGRAMME[0])));
    }
    if modell.is_none() {
        fehlt.push(("Modell".into(), erster_ort(gewichte, HOERMODELL).display().to_string()));
    }
    Err(Mangel {
        sinn: "Hoermodell",
        fehlt,
        anleitung: vec![
            format!("whisper.cpp installieren ({})", einbauhinweis("whisper-cpp", "whisper-cpp")),
            format!("Den Ordner anlegen: {}", erster_ort(gewichte, "").display()),
            format!(
                "Ein ggml-Modell dorthin legen, benannt {HOERMODELL}. Klein und brauchbar sind \
                 zum Beispiel ggml-small.bin oder ggml-large-v3-turbo-q5_0.bin."
            ),
        ],
    })
}

/// **Die Tonquelle dieses Systems**, wenn niemand etwas anderes sagt.
///
/// ⚠️ **Unter Windows raet das hier.** `dshow` will den Geraetenamen, und
/// der heisst auf jeder Maschine anders; die Vorgabe trifft den haeufigen
/// Fall und sonst hilft `MYL_TONGERAET`. Auf macOS und Linux gibt es
/// einen echten Vorgabenamen.
pub fn tonquelle_vorgabe() -> (String, String) {
    if cfg!(target_os = "macos") {
        ("avfoundation".into(), ":0".into())
    } else if cfg!(target_os = "windows") {
        ("dshow".into(), "audio=Microphone".into())
    } else {
        ("pulse".into(), "default".into())
    }
}

/// Woher ffmpeg den Bildschirm nimmt, wenn es ihn nehmen muss.
///
/// ⚠️ **Auf macOS steht hier nichts**, denn dort nimmt `screencapture`
/// ihn, und das braucht keine Quelle.
pub fn schirmquelle_vorgabe() -> (String, String) {
    if cfg!(target_os = "windows") {
        ("gdigrab".into(), "desktop".into())
    } else {
        ("x11grab".into(), ":0.0".into())
    }
}

/// Woher ffmpeg die Kamera nimmt.
pub fn kameraquelle_vorgabe() -> (String, String) {
    if cfg!(target_os = "macos") {
        ("avfoundation".into(), "0".into())
    } else if cfg!(target_os = "windows") {
        ("dshow".into(), "video=Integrated Camera".into())
    } else {
        ("v4l2".into(), "/dev/video0".into())
    }
}

/// **Womit sich ein PDF lesen laesst.**
///
/// ⚑ **Die Rangfolge ist die Reihenfolge**, und sie ist eine
/// Entscheidung: Ein eigenes Programm zuerst (wer eines hinlegt, hat
/// sich entschieden), dann `pdftotext`, dann `mutool`, zuletzt ein
/// Python. Das Python steht hinten, weil es eine Bibliothek
/// voraussetzt, die dort installiert sein muss; die ersten beiden
/// bringen alles mit.
fn schriftzeug(
    heimat: &Path,
    eigen: &Eigene,
    pfad: &[PathBuf],
) -> Result<crate::schrift::Schriftzeug, Mangel> {
    use crate::schrift::{Schriftweg, Schriftzeug};
    if let Some(p) = programm_suchen(&[], eigen.schriftleser.as_deref(), heimat, pfad) {
        return Ok(Schriftzeug { weg: Schriftweg::Pdftotext(p) });
    }
    if let Some(p) = programm_suchen(&["pdftotext"], None, heimat, pfad) {
        return Ok(Schriftzeug { weg: Schriftweg::Pdftotext(p) });
    }
    if let Some(p) = programm_suchen(&["mutool"], None, heimat, pfad) {
        return Ok(Schriftzeug { weg: Schriftweg::Mutool(p) });
    }
    // ⚠️ **Gesucht wird ein Python, das die Bibliothek WIRKLICH hat.**
    //    Eines, das sie nicht hat, waere ein Zeug, das beim ersten
    //    Auftrag versagt, und das ist schlimmer als keines.
    for name in ["python3", "python"] {
        if let Some(p) = programm_suchen(&[name], None, heimat, pfad) {
            if python_kann_pdf(&p) {
                return Ok(Schriftzeug { weg: Schriftweg::Python(p) });
            }
        }
    }
    Err(Mangel {
        sinn: "PDF lesen",
        fehlt: vec![("Programm".into(), "pdftotext, mutool oder ein Python mit pypdf".into())],
        anleitung: vec![
            format!("poppler installieren ({})", einbauhinweis("poppler", "poppler-utils")),
            "Oder: python3 -m pip install pypdf".into(),
            "Ein eigenes Programm sagt MYL_SCHRIFTLESER; es bekommt den PDF-Pfad und schreibt Text nach stdout."
                .into(),
        ],
    })
}

/// Ob dieses Python `fitz` oder `pypdf` kennt.
fn python_kann_pdf(python: &Path) -> bool {
    std::process::Command::new(python)
        .arg("-c")
        .arg("import importlib.util as u, sys; sys.exit(0 if (u.find_spec('fitz') or u.find_spec('pypdf')) else 1)")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn bildschirmzeug(heimat: &Path, eigen: &Eigene, pfad: &[PathBuf]) -> Result<Bildschirmzeug, Mangel> {
    // ⚑ **Ein eigener Blicker gewinnt**, wie ueberall hier: Wer eines
    // hinlegt, hat sich entschieden.
    if let Some(eigen_prog) = programm_suchen(&[], eigen.blicker.as_deref(), heimat, pfad) {
        return Ok(Bildschirmzeug {
            programm: eigen_prog,
            quelle: eigen.schirmquelle.clone(),
        });
    }
    // ⚑ **Auf macOS zuerst `screencapture`.** Es liegt bei, ist
    // schneller und kann spaeter ein einzelnes Fenster statt des ganzen
    // Schirms.
    if cfg!(target_os = "macos") {
        if let Some(p) = programm_suchen(&["screencapture"], None, heimat, pfad) {
            return Ok(Bildschirmzeug { programm: p, quelle: None });
        }
    }
    match programm_suchen(&["ffmpeg"], None, heimat, pfad) {
        Some(programm) => Ok(Bildschirmzeug {
            programm,
            quelle: Some(eigen.schirmquelle.clone().unwrap_or_else(schirmquelle_vorgabe)),
        }),
        None => Err(Mangel {
            sinn: "Bildschirmaufnahme",
            fehlt: vec![("Programm".into(), "screencapture oder ffmpeg".into())],
            anleitung: vec![
                format!("ffmpeg installieren ({})", einbauhinweis("ffmpeg", "ffmpeg")),
                "Ein eigenes Programm sagt MYL_BLICKER; es bekommt den Zielpfad als letztes Argument."
                    .into(),
            ],
        }),
    }
}

fn kamerazeug(heimat: &Path, eigen: &Eigene, pfad: &[PathBuf]) -> Result<Kamerazeug, Mangel> {
    match programm_suchen(&["ffmpeg"], eigen.blicker.as_deref(), heimat, pfad) {
        Some(programm) => Ok(Kamerazeug {
            programm,
            quelle: eigen.kameraquelle.clone().unwrap_or_else(kameraquelle_vorgabe),
        }),
        None => Err(Mangel {
            sinn: "Kamera",
            fehlt: vec![("Programm".into(), "ffmpeg".into())],
            anleitung: vec![
                format!("ffmpeg installieren ({})", einbauhinweis("ffmpeg", "ffmpeg")),
                format!(
                    "Nimmt ffmpeg die falsche Kamera, sagen MYL_KAMERAFORMAT und MYL_KAMERAGERAET, \
                     welche gemeint ist; Vorgabe hier ist {:?}.",
                    kameraquelle_vorgabe()
                ),
            ],
        }),
    }
}

fn aufnahmezeug(heimat: &Path, eigen: &Eigene, pfad: &[PathBuf]) -> Result<Aufnahmezeug, Mangel> {
    // ⚑ **Das eigene Skript gewinnt**, wie beim Sprechen: Wer es
    // hinlegt, hat sich entschieden.
    let skript = heimat.join("bin").join(AUFNAHMESKRIPT);
    if eigen.aufnehmer.is_none() && ausfuehrbar(&skript) {
        return Ok(Aufnahmezeug { programm: skript, quelle: None });
    }
    match programm_suchen(&["ffmpeg"], eigen.aufnehmer.as_deref(), heimat, pfad) {
        Some(programm) => Ok(Aufnahmezeug {
            programm,
            quelle: Some(eigen.tonquelle.clone().unwrap_or_else(tonquelle_vorgabe)),
        }),
        None => Err(Mangel {
            sinn: "Aufnahmeprogramm",
            fehlt: vec![("Programm".into(), format!("ffmpeg oder {}", skript.display()))],
            anleitung: vec![
                format!("ffmpeg installieren ({})", einbauhinweis("ffmpeg", "ffmpeg")),
                format!(
                    "Nimmt ffmpeg das falsche Geraet, sagen MYL_TONFORMAT und MYL_TONGERAET, \
                     welches gemeint ist; Vorgabe hier ist {:?}.",
                    tonquelle_vorgabe()
                ),
                format!(
                    "Wer ein anderes Aufnahmeprogramm will, legt ein ausfuehrbares Skript {} hin; \
                     es bekommt ein Ziel-WAV und nimmt auf, bis seine Standardeingabe schliesst.",
                    skript.display()
                ),
            ],
        }),
    }
}

fn sprechzeug(
    heimat: &Path,
    gewichte: &[PathBuf],
    eigen: &Eigene,
    pfad: &[PathBuf],
) -> Result<Sprechzeug, Mangel> {
    // ⚑ **Die Stimmprobe bleibt in der Heimat.** Sie ist eine Aufnahme
    // des Nutzers und kein geladenes Gewicht; sie gehoert dorthin, wo
    // diese Kiste auch schreiben darf.
    let probe = datei_suchen(None, &heimat.join(STIMMPROBE));
    let probentext = datei_suchen(None, &heimat.join(STIMMPROBE_TEXT));
    let fertig = |weg| Ok(Sprechzeug { weg, probe: probe.clone(), probentext: probentext.clone() });

    // 1. Das eigene Skript gewinnt. Wer es hinlegt, hat gewaehlt, und
    //    diese Entscheidung soll nicht davon abhaengen, was sonst noch
    //    zufaellig installiert ist.
    let skript = heimat.join("bin").join(SPRECHSKRIPT);
    if eigen.sprecher.is_none() && ausfuehrbar(&skript) {
        return fertig(Sprechweg::Skript(skript));
    }

    // 2. CosyVoice: Python, der mitgelieferte Laeufer, die Wurzel.
    // ⚑ **Erst die Variable, dann die Gewichtsorte, dann die Heimat**,
    // also dieselbe Reihenfolge wie bei jeder anderen Gewichtsdatei.
    // CosyVoice ist ein Verzeichnis und keine Datei, sucht sich aber
    // sonst genauso.
    let wurzel = std::env::var_os(COSYVOICE_UMGEBUNG)
        .filter(|w| !w.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            gewichte
                .iter()
                .chain(std::iter::once(&heimat.to_path_buf()))
                .flat_map(|o| COSYVOICE_NAMEN.iter().map(move |n| o.join(n)))
                .find(|v| v.is_dir())
        });
    // ⛔️ **Der Python neben der Installation gewinnt**, und das ist die
    // ganze Pointe.
    //
    // CosyVoice braucht torch, torchaudio und ein Dutzend weitere
    // Kisten; die liegen in **seiner** Umgebung und nicht im
    // System-Python. Waehlte man den ersten Python im PATH, faende man
    // unter macOS `/usr/bin/python3` (3.9, ohne alles) und bekaeme einen
    // Importfehler statt einer Stimme. 📌 **Aufgefallen beim
    // tatsaechlichen Einrichten**, nicht beim Schreiben.
    let python = wurzel
        .as_ref()
        .and_then(|w| {
            [w.join(".venv/bin/python3"), w.join(".venv/bin/python"), w.join("venv/bin/python3")]
                .into_iter()
                .find(|p| ausfuehrbar(p))
        })
        .or_else(|| programm_suchen(&PYTHONPROGRAMME, None, heimat, pfad));
    let laeufer = heimat.join("bin").join(COSYVOICE_LAEUFER);
    if let (Some(wurzel), Some(python)) = (&wurzel, &python) {
        if wurzel.is_dir() && laeufer.is_file() {
            return fertig(Sprechweg::CosyVoice {
                python: python.clone(),
                laeufer,
                wurzel: wurzel.clone(),
            });
        }
    }

    // 3. piper als Rueckfall.
    let programm = programm_suchen(&SPRECHPROGRAMME, eigen.sprecher.as_deref(), heimat, pfad);
    let stimme = datei_suchen_in(eigen.sprechmodell.as_deref(), gewichte, SPRECHMODELL);
    if let (Some(programm), Some(stimme)) = (&programm, &stimme) {
        return fertig(Sprechweg::Piper { programm: programm.clone(), stimme: stimme.clone() });
    }

    let mut fehlt = Vec::new();
    if wurzel.is_none() {
        fehlt.push((
            "CosyVoice".into(),
            format!(
                "{COSYVOICE_UMGEBUNG} oder {}",
                erster_ort(gewichte, COSYVOICE_NAMEN[0]).display()
            ),
        ));
    }
    if python.is_none() {
        fehlt.push(("Python".into(), "eine .venv neben CosyVoice, sonst python3".into()));
    }
    if !laeufer.is_file() {
        fehlt.push(("Laeufer".into(), laeufer.display().to_string()));
    }
    if programm.is_none() {
        fehlt.push(("piper (Rueckfall)".into(), SPRECHPROGRAMME[0].into()));
    }
    Err(Mangel {
        sinn: "Sprechmodell",
        fehlt,
        anleitung: vec![
            format!(
                "CosyVoice holen und auspacken, dann {COSYVOICE_UMGEBUNG} darauf zeigen lassen. \
                 Python bringt der Nutzer mit; es gehoert nicht ins Repositorium."
            ),
            format!(
                "Den Laeufer anlegen lassen: er wird nach {} geschrieben und ist danach \
                 frei zu aendern.",
                laeufer.display()
            ),
            format!(
                "Wer es einfacher will, nimmt piper: {} und eine Stimme als {} in {}. \
                 ⚠️ piper kann allerdings keine Stimme nachbilden.",
                einbauhinweis("piper", "piper-tts"),
                SPRECHMODELL,
                erster_ort(gewichte, "").display()
            ),
        ],
    })
}

/// ⚑ **Der Hinweis nennt den Paketverwalter dieses Systems**, denn ein
/// `brew install` auf NixOS hilft niemandem.
fn einbauhinweis(brau: &str, nix: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("macOS: brew install {brau}")
    } else if cfg!(target_os = "windows") {
        format!("Windows: ein fertiges Bau von {brau} herunterladen und in den PATH legen")
    } else {
        format!("NixOS: nix-shell -p {nix}, sonst der Paketverwalter des Systems")
    }
}

/// **Das erste der genannten Programme, das es gibt**: erst die eigene
/// Angabe, dann `PATH`, dann der `bin`-Ordner der Sinnesheimat.
///
/// ⚑ **Der `bin`-Ordner der Heimat ist Absicht.** Wer llama.cpp nicht
/// systemweit installieren will, legt es dorthin, wo auch die Gewichte
/// liegen, und muss nichts an seinem `PATH` aendern.
pub fn programm_suchen(
    namen: &[&str],
    eigen: Option<&Path>,
    heimat: &Path,
    pfad: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(p) = eigen {
        return ausfuehrbar(p).then(|| p.to_path_buf());
    }
    for name in namen {
        for kandidat in namensformen(name) {
            for ordner in pfad.iter().chain(std::iter::once(&heimat.join("bin"))) {
                let p = ordner.join(&kandidat);
                if ausfuehrbar(&p) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// ⚠️ **Unter Windows heisst dasselbe Programm `name.exe`.** Ohne diese
/// Formen faende die Suche dort nie etwas, und der Client wird fuer
/// Windows ausgeliefert.
fn namensformen(name: &str) -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    }
}

#[cfg(unix)]
fn ausfuehrbar(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file()
        && std::fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// ⚠️ **Unter Windows sagt kein Bit, ob eine Datei ein Programm ist.**
/// Dort entscheidet die Endung, und die steht schon in [`namensformen`].
#[cfg(not(unix))]
fn ausfuehrbar(p: &Path) -> bool {
    p.is_file()
}

fn datei_suchen(eigen: Option<&Path>, vorgabe: &Path) -> Option<PathBuf> {
    if let Some(p) = eigen {
        return p.is_file().then(|| p.to_path_buf());
    }
    vorgabe.is_file().then(|| vorgabe.to_path_buf())
}

/// **Dieselbe Suche ueber mehrere Orte**, in deren Reihenfolge.
///
/// ⚑ **Die eigene Angabe schlaegt alle Orte.** Wer eine Datei
/// ausdruecklich nennt, will genau die, und ein Rueckfall auf eine
/// gleichnamige woanders waere eine stille Ersetzung.
///
/// ⚠️ **Eine genannte Datei, die es nicht gibt, ergibt `None`** und
/// nicht den Rueckfall: Sonst rechnete der Nutzer mit einem Modell,
/// dessen Pfad er falsch geschrieben hat, und merkte es nie.
fn datei_suchen_in(eigen: Option<&Path>, orte: &[PathBuf], name: &str) -> Option<PathBuf> {
    if let Some(p) = eigen {
        return p.is_file().then(|| p.to_path_buf());
    }
    orte.iter().map(|o| o.join(name)).find(|p| p.is_file())
}

/// **Wohin eine fehlende Datei gehoert**, fuer die Mangelmeldung.
///
/// ⚑ **Der erste Ort und nicht alle.** Ein Mangel, der drei Pfade
/// nennt, laesst den Leser waehlen, und die Suche hat schon gewaehlt.
fn erster_ort(orte: &[PathBuf], name: &str) -> PathBuf {
    match orte.first() {
        Some(o) => o.join(name),
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// **Die Reihenfolge der Suchorte, alle drei Stufen.**
    ///
    /// ⚑ **Die Reihenfolge ist die Aussage und nicht die Menge.** Eine
    /// Probe, die nur pruefte, dass alle drei Orte vorkommen, bliebe
    /// gruen, wenn die Heimat vor dem Klon stuende, und dann gewaenne
    /// ein altes Modell im Heimatordner gegen das neue im Klon,
    /// stillschweigend.
    #[test]
    fn die_suchorte_stehen_in_der_richtigen_reihenfolge() {
        let klon = Path::new("/ein/klon");
        let heim = Path::new("/ein/heim/.myelith/sinne");

        // Mit Klon: erst die beiden Rubriken, dann die Heimat.
        let mit = gewichtsorte_aus(None, Some(klon), heim);
        assert_eq!(
            mit,
            vec![
                klon.join(MODELLE_ORDNER).join(RUBRIK_VISION),
                klon.join(MODELLE_ORDNER).join(RUBRIK_AUDIO),
                heim.to_path_buf(),
            ]
        );
        // ⚑ Und ausdruecklich: die Heimat ist die **letzte**.
        assert_eq!(mit.last().map(|p| p.as_path()), Some(heim));

        // Ohne Klon bleibt allein die Heimat, und die Liste ist nicht leer.
        assert_eq!(gewichtsorte_aus(None, None, heim), vec![heim.to_path_buf()]);
    }

    /// ⛔️ **`MYL_SINNE` schlaegt alles, auch einen vorhandenen Klon.**
    ///
    /// Wer die Variable setzt, sagt damit, dass dort alles
    /// zusammenliegt. Eine Variable, die nur die halbe Suche uebergeht,
    /// waere schlimmer als keine: Der Nutzer legt sein Modell an den
    /// genannten Ort, und gerechnet wird mit einem anderen.
    #[test]
    fn die_umgebung_schlaegt_klon_und_heimat() {
        let gesagt = Path::new("/woanders/sinne");
        let orte = gewichtsorte_aus(Some(gesagt), Some(Path::new("/ein/klon")), Path::new("/heim"));
        assert_eq!(orte, vec![gesagt.to_path_buf()], "der Klon darf hier nicht mitreden");
    }

    /// **Eine Datei wird am ersten Ort gefunden, an dem sie liegt.**
    ///
    /// ⚑ **Mit Gegenprobe in beide Richtungen:** dass der zweite Ort
    /// greift, wenn der erste leer ist, **und** dass der erste gewinnt,
    /// wenn beide etwas haben. Ohne die zweite Haelfte bestuende die
    /// Probe auch dann, wenn `datei_suchen_in` schlicht den letzten
    /// Treffer zurueckgaebe.
    #[test]
    fn der_erste_ort_mit_der_datei_gewinnt() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let eins = d.path().join("eins");
        let zwei = d.path().join("zwei");
        std::fs::create_dir_all(&eins).expect("eins");
        std::fs::create_dir_all(&zwei).expect("zwei");
        let orte = vec![eins.clone(), zwei.clone()];

        // Nichts da: nichts gefunden.
        assert_eq!(datei_suchen_in(None, &orte, HOERMODELL), None);

        // Nur im zweiten: der zweite.
        std::fs::write(zwei.join(HOERMODELL), "b").expect("schreiben");
        assert_eq!(datei_suchen_in(None, &orte, HOERMODELL), Some(zwei.join(HOERMODELL)));

        // In beiden: der erste.
        std::fs::write(eins.join(HOERMODELL), "a").expect("schreiben");
        assert_eq!(datei_suchen_in(None, &orte, HOERMODELL), Some(eins.join(HOERMODELL)));
    }

    /// ⚠️ **Eine ausdruecklich genannte Datei, die fehlt, ergibt keinen
    /// Rueckfall.**
    ///
    /// 📌 **Sonst rechnete jemand mit einem Modell, dessen Pfad er falsch
    /// geschrieben hat**, und bekaeme ein Ergebnis, das nach Erfolg
    /// aussieht. Ein Tippfehler in einer Angabe muss auffallen.
    #[test]
    fn eine_genannte_datei_faellt_nicht_zurueck() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let ort = d.path().to_path_buf();
        std::fs::write(ort.join(HOERMODELL), "da").expect("schreiben");
        let orte = vec![ort.clone()];

        // Ohne eigene Angabe wird gefunden, was daliegt.
        assert_eq!(datei_suchen_in(None, &orte, HOERMODELL), Some(ort.join(HOERMODELL)));

        // Mit einer Angabe auf etwas, das es nicht gibt: nichts.
        let daneben = d.path().join("gibt-es-nicht.bin");
        assert_eq!(datei_suchen_in(Some(&daneben), &orte, HOERMODELL), None);
    }

    /// **Der Mangel nennt den ersten Gewichtsort und nicht die Heimat.**
    ///
    /// ⚑ **Das ist die Stelle, an der ein Umzug sichtbar wird.** Wer
    /// nichts hat, liest hier, wohin die Datei gehoert; nennte die
    /// Meldung weiter die Heimat, legte er sie an einen Ort, an dem
    /// zuletzt gesucht wird.
    #[test]
    fn der_mangel_nennt_den_ersten_gewichtsort() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let gewichte = vec![d.path().join("MODELS/vision"), d.path().join("MODELS/audio")];
        let s = Sinne::finden_mit(d.path(), &gewichte, &Eigene::default(), &[]);

        let m = s.sehen.as_ref().expect_err("es ist nichts da");
        let t = m.bericht();
        let soll = gewichte[0].join(SEHMODELL).display().to_string();
        assert!(t.contains(&soll), "der Bericht nennt {soll} nicht:\n{t}");
        // ⚑ Gegenprobe: die Heimat steht dort NICHT als Ort des Modells.
        let falsch = d.path().join(SEHMODELL).display().to_string();
        assert!(!t.contains(&falsch), "der Bericht nennt noch die Heimat:\n{t}");
    }

    fn stellen(d: &Path, name: &str) -> PathBuf {
        let bin = d.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let p = bin.join(name);
        std::fs::write(&p, "#!/bin/sh\necho da\n").expect("Attrappe");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
        }
        p
    }

    /// ⚑ **Fehlt alles, steht alles im Bericht**, samt Ort und Anleitung.
    #[test]
    fn ohne_alles_sagt_der_mangel_was_fehlt() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let m = s.sehen.as_ref().expect_err("es ist nichts da");
        let t = m.bericht();
        assert!(t.contains("Programm fehlt"), "{t}");
        assert!(t.contains("llama.cpp"), "{t}");
        assert!(t.contains(&d.path().join(SEHMODELL).display().to_string()), "{t}");
        assert!(t.contains("Projektor fehlt"), "{t}");
        assert!(!s.bereit(crate::anhang::Art::Bild));
        assert!(!s.bereit(crate::anhang::Art::Ton));
    }

    /// ⚑ **Fehlt nur eines, steht auch nur dieses da.** Ein Bericht, der
    /// vorhandene Dinge als fehlend nennt, schickt den Leser suchen.
    #[test]
    fn nur_das_fehlende_steht_im_bericht() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SEHPROGRAMME[0]);
        std::fs::write(d.path().join(SEHMODELL), "x").expect("Modell");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let t = s.sehen.as_ref().expect_err("der Projektor fehlt").bericht();
        assert!(t.contains("Projektor fehlt"), "{t}");
        assert!(!t.contains("Programm fehlt"), "{t}");
        assert!(!t.contains("Modell fehlt"), "{t}");
    }

    /// ⚑ **Liegt alles da, ist der Sinn bereit**, und die Pfade zeigen
    /// auf das Gestellte.
    #[test]
    fn mit_allem_ist_der_sinn_bereit() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let programm = stellen(d.path(), SEHPROGRAMME[0]);
        std::fs::write(d.path().join(SEHMODELL), "x").expect("Modell");
        std::fs::write(d.path().join(SEHPROJEKTOR), "x").expect("Projektor");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let z = s.sehen.as_ref().expect("bereit").fuer(Stufe::Schnell);
        assert_eq!(z.programm, programm);
        assert!(s.bereit(crate::anhang::Art::Bild));
        // ⛔️ **Und Hoeren ist davon unberuehrt.** Zwei Sinne, zwei Antworten.
        assert!(!s.bereit(crate::anhang::Art::Ton));
    }

    /// ⚑ **Die eigene Angabe schlaegt die Suche**, und eine falsche
    /// faellt **nicht** still auf die Vorgabe zurueck: Sonst arbeitete
    /// der Client mit einem anderen Modell als dem genannten.
    #[test]
    fn die_eigene_angabe_gilt_und_faellt_nicht_still_zurueck() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SEHPROGRAMME[0]);
        std::fs::write(d.path().join(SEHMODELL), "x").expect("Modell");
        std::fs::write(d.path().join(SEHPROJEKTOR), "x").expect("Projektor");
        let woanders = d.path().join("anderes.gguf");
        std::fs::write(&woanders, "x").expect("anderes");

        let eigen = Eigene { sehmodell: Some(woanders.clone()), ..Default::default() };
        let s = Sinne::finden_in(d.path(), &eigen, &[]);
        assert_eq!(s.sehen.expect("bereit").fuer(Stufe::Schnell).modell, woanders);

        let eigen = Eigene { sehmodell: Some(d.path().join("gibtsnicht.gguf")), ..Default::default() };
        let s = Sinne::finden_in(d.path(), &eigen, &[]);
        assert!(s.sehen.is_err(), "eine falsche Angabe fiel still auf die Vorgabe zurueck");
    }

    /// ⚑ **Zwei Sprossen, und wer nur eine hinlegt, bekommt sie fuer
    /// beides.** Eine Stufe, die mangels Gewichten leer ausginge, waere
    /// eine Falle: Der Agent stellte eine Frage und bekaeme nichts.
    #[test]
    fn die_zweite_sprosse_ist_wahlfrei_und_faellt_zurueck() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SEHPROGRAMME[0]);
        std::fs::write(d.path().join(SEHMODELL), "x").expect("Modell");
        std::fs::write(d.path().join(SEHPROJEKTOR), "x").expect("Projektor");

        // Nur die schnelle Sprosse: `Genau` faellt auf sie zurueck.
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let sehen = s.sehen.as_ref().expect("bereit");
        assert!(!sehen.zweistufig());
        assert_eq!(sehen.fuer(Stufe::Genau).modell, d.path().join(SEHMODELL));

        // Mit beiden: jede Stufe nimmt ihre eigene.
        std::fs::write(d.path().join(SEHMODELL_GENAU), "x").expect("Modell");
        std::fs::write(d.path().join(SEHPROJEKTOR_GENAU), "x").expect("Projektor");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let sehen = s.sehen.as_ref().expect("bereit");
        assert!(sehen.zweistufig());
        assert_eq!(sehen.fuer(Stufe::Schnell).modell, d.path().join(SEHMODELL));
        assert_eq!(sehen.fuer(Stufe::Genau).modell, d.path().join(SEHMODELL_GENAU));
    }

    /// ⚑ **Nur die genaue Sprosse reicht auch.** Wer ein einziges,
    /// grosses Modell will, soll es nicht `sehen.gguf` nennen muessen.
    #[test]
    fn allein_die_genaue_sprosse_genuegt() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SEHPROGRAMME[0]);
        std::fs::write(d.path().join(SEHMODELL_GENAU), "x").expect("Modell");
        std::fs::write(d.path().join(SEHPROJEKTOR_GENAU), "x").expect("Projektor");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let sehen = s.sehen.as_ref().expect("bereit");
        assert_eq!(sehen.fuer(Stufe::Schnell).modell, d.path().join(SEHMODELL_GENAU));
    }

    /// ⚑ **Zwei Wege zum Sprechen, und das eigene Skript gewinnt.**
    ///
    /// ⛔️ Diese Probe beisst, wenn die Reihenfolge umgedreht wird: Wer
    /// ein Skript hinlegt, hat sich entschieden, und ein zufaellig
    /// installiertes piper darf diese Entscheidung nicht ueberstimmen.
    #[test]
    fn das_eigene_sprechskript_schlaegt_piper() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SPRECHPROGRAMME[0]);
        std::fs::write(d.path().join(SPRECHMODELL), "x").expect("Stimme");

        // Nur piper: die Stimme kommt mit, und klonen kann es nicht.
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let z = s.sprechen.as_ref().expect("bereit");
        assert!(matches!(&z.weg, Sprechweg::Piper { stimme, .. } if *stimme == d.path().join(SPRECHMODELL)));
        assert!(!z.kann_klonen(), "piper kann nicht klonen");
        assert!(!z.dauerhaft(), "piper braucht keinen Dauerlaeufer");
        assert!(s.kann_sprechen());

        // Mit eigenem Skript: es gewinnt.
        stellen(d.path(), SPRECHSKRIPT);
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        let z = s.sprechen.as_ref().expect("bereit");
        assert!(matches!(&z.weg, Sprechweg::Skript(p) if p.ends_with(SPRECHSKRIPT)), "{:?}", z.weg);
        assert!(z.kann_klonen());
    }

    /// ⛔️ **Ohne Sprechmodell sagt der Mangel beide Wege.** Wer nur den
    /// einen genannt bekaeme, suchte nach dem falschen.
    #[test]
    fn ohne_sprechmodell_stehen_beide_wege_da() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let s = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
        assert!(!s.kann_sprechen());
        let t = s.sprechen.as_ref().expect_err("nichts da").bericht();
        assert!(t.contains("CosyVoice"), "die Vorgabe fehlt: {t}");
        assert!(t.contains("piper"), "der Rueckfall fehlt: {t}");
        assert!(t.contains(COSYVOICE_UMGEBUNG), "wo CosyVoice liegt, fehlt: {t}");
    }

    /// ⚑ **`PATH` kommt vor dem `bin` der Heimat.**
    #[test]
    fn der_pfad_schlaegt_die_heimat() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        stellen(d.path(), SEHPROGRAMME[0]);
        let anderswo = tempfile::tempdir().expect("Verzeichnis");
        let vorn = stellen(anderswo.path(), SEHPROGRAMME[0]);
        let gefunden = programm_suchen(
            &SEHPROGRAMME,
            None,
            d.path(),
            &[anderswo.path().join("bin")],
        );
        assert_eq!(gefunden.as_deref(), Some(vorn.as_path()));
    }

    /// ⛔️ **Eine Datei ohne Ausfuehrungsrecht ist kein Programm.**
    #[cfg(unix)]
    #[test]
    fn eine_nicht_ausfuehrbare_datei_zaehlt_nicht() {
        let d = tempfile::tempdir().expect("Verzeichnis");
        let bin = d.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        std::fs::write(bin.join(SEHPROGRAMME[0]), "kein Programm").expect("Datei");
        assert!(programm_suchen(&SEHPROGRAMME, None, d.path(), &[]).is_none());
    }
}
