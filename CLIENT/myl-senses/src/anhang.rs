//! **Eine Datei ins Gespraech geben**, ohne den Kontext zu fluten.
//!
//! # ⛔️ Was hier NICHT passiert, und warum das die ganze Entscheidung ist
//!
//! Der naheliegende Weg waere, den Inhalt in die Nachricht zu schreiben.
//! **Er ist falsch, und zwar gemessen:** Am 2026-09-17 hat eine Messung
//! gezeigt, was ein voller Kontext kostet und wie gut Nachschlagen
//! stattdessen wirkt (das 4B fand eine Einzelheit in neun von neun
//! Laeufen mit **einem** Werkzeugaufruf, bei einem Viertel des
//! Kontexts). ⚑ **Eine angehaengte Datei wird deshalb benannt und nicht
//! geliefert.**
//!
//! ⚑ **Mit einer Ausnahme, und sie ist begruendet:** Bild und Ton kann
//! das Modell nicht lesen, auch nicht mit einem Werkzeug. Was ein
//! Sinnesmodell daraus macht, sind ein paar Zeilen Text; die kommen
//! mit, weil es sonst gar nichts gibt (siehe [`Sicht`]).
//!
//! # ⚑ Wohin die Datei wandert, entscheidet der Aufrufer
//!
//! Diese Kiste kennt keinen `.AGENT`-Ordner. Sie bekommt gesagt, welcher
//! **Unterordner** unter der Einhaengung gilt, und leitet Zielpfad und
//! Meldepfad **aus derselben Angabe** ab. Zwei Parameter, einer fuer den
//! echten und einer fuer den gemeldeten Pfad, waeren zwei Orte, die
//! auseinanderlaufen, und der zweite meldet sich nicht.
//!
//! ⚠️ **Angehaengte Dateien werden nicht aufgeraeumt.** Ein Mitschnitt
//! hat eine Obergrenze, weil er von selbst entsteht; eine Datei hat der
//! Nutzer ausdruecklich hergegeben, und etwas wegzuwerfen, das jemand
//! bewusst angehaengt hat, waere eine Ueberraschung.

use std::path::Path;

/// Wo Anhaenge liegen, unter dem Ordner des Agenten.
pub const ORDNER: &str = "anhaenge";

/// Wie viele Zeichen ein Textauszug hoechstens hat.
///
/// ⚑ **Ein Auszug und kein Inhalt.** Er soll die Frage beantworten
/// „ist das die richtige Datei", nicht die Datei ersetzen. Wer mehr
/// will, liest mit `read_file`.
pub const AUSZUG_ZEICHEN: usize = 600;

/// Was fuer eine Art Datei das ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// Text, den das Modell unmittelbar lesen kann.
    Text,
    /// Ein Bild. ⚠️ **Das Modell sieht es nicht**, siehe [`Anhang::hinweis`].
    Bild,
    /// Ton. Ebenso.
    Ton,
    /// Alles andere: Archive, Tabellen, Fremdformate.
    Sonstiges,
}

impl Art {
    /// **Das Wort, mit dem ein Manifest sich fuer diese Art zustaendig
    /// erklaert** (Feld `fuer`). ⚑ Es steht hier und im Manifest, und
    /// nirgends sonst.
    pub fn kennung(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Bild => "bild",
            Self::Ton => "ton",
            Self::Sonstiges => "sonstiges",
        }
    }

    /// Wie sie in der Nachricht heisst.
    pub fn wort(&self, deutsch: bool) -> &'static str {
        match (self, deutsch) {
            (Self::Text, true) => "Text",
            (Self::Text, false) => "text",
            (Self::Bild, true) => "Bild",
            (Self::Bild, false) => "image",
            (Self::Ton, true) => "Ton",
            (Self::Ton, false) => "audio",
            (Self::Sonstiges, true) => "Datei",
            (Self::Sonstiges, false) => "file",
        }
    }
}

/// Eine aufgenommene Datei.
#[derive(Debug, Clone)]
pub struct Anhang {
    /// Der Name, unter dem sie abgelegt wurde.
    pub name: String,
    /// Der Pfad relativ zur Einhaengung, so wie ihn ein Werkzeug nimmt.
    pub pfad: String,
    /// Wie gross sie ist.
    pub bytes: u64,
    /// Was fuer eine Art.
    pub art: Art,
    /// Bei Text: die ersten Zeichen. Sonst leer.
    pub auszug: String,
}

/// **Die Art an den ersten Bytes und nicht an der Endung.**
///
/// ⚑ **Eine Endung ist eine Behauptung des Dateinamens**, die ersten
/// Bytes sind die Datei selbst. Wer nach `.png` geht, nennt eine
/// umbenannte Textdatei ein Bild und umgekehrt. ⚠️ Die Endung bleibt der
/// Rueckfall, wenn die Bytes nichts sagen.
pub fn art_bestimmen(anfang: &[u8], name: &str) -> Art {
    const BILDMARKEN: [&[u8]; 5] = [
        b"\x89PNG",        // PNG
        b"\xff\xd8\xff",   // JPEG
        b"GIF8",           // GIF
        b"BM",             // BMP
        b"RIFF",           // WebP (RIFF....WEBP), unten nachgeprueft
    ];
    const TONMARKEN: [&[u8]; 4] = [
        b"OggS",           // Ogg
        b"fLaC",           // FLAC
        b"ID3",            // MP3 mit Kennsatz
        b"\xff\xfb",       // MP3 ohne
    ];

    if anfang.starts_with(b"RIFF") {
        // ⚑ RIFF traegt beides: WAV und WebP. Das vierte Wort entscheidet.
        return match anfang.get(8..12) {
            Some(b"WAVE") => Art::Ton,
            Some(b"WEBP") => Art::Bild,
            _ => Art::Sonstiges,
        };
    }
    if BILDMARKEN.iter().any(|m| anfang.starts_with(m)) {
        return Art::Bild;
    }
    if TONMARKEN.iter().any(|m| anfang.starts_with(m)) {
        return Art::Ton;
    }
    if anfang.starts_with(b"%PDF") || anfang.starts_with(b"PK\x03\x04") {
        return Art::Sonstiges;
    }
    // ⚑ **Text ist, was sich als UTF-8 lesen laesst und keine Nullbytes
    // hat.** Das ist die Pruefung, die zaehlt: Ein Modell bekommt Text
    // in den Kontext, und was dort landet, muss lesbar sein.
    //
    // ⛔️ **Und es braucht wenigstens ein Zeichen.** Eine leere Datei ist
    // als UTF-8 gueltig; ohne diese Bedingung galt sie als Text, und die
    // Endung kam nie zum Zug. **Ein leeres Etwas ist kein Text, sondern
    // nichts**, und dann entscheidet der Name.
    if !anfang.is_empty() && !anfang.contains(&0) && std::str::from_utf8(anfang).is_ok() {
        return Art::Text;
    }
    // Rueckfall auf die Endung, falls die Bytes schweigen.
    let endung = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match endung.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" => Art::Bild,
        "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac" => Art::Ton,
        "md" | "txt" | "csv" | "json" | "toml" | "yaml" | "rs" | "py" => Art::Text,
        _ => Art::Sonstiges,
    }
}

/// **Nimmt eine Datei auf**: kopiert sie unter die Einhaengung und sagt,
/// was sie ist.
///
/// ⛔️ **Kopiert und nicht verschoben.** Die Datei des Nutzers bleibt, wo
/// sie ist; ein Anhang, der das Original wegnimmt, ist ein Datenverlust
/// mit gutem Gewissen.
pub fn aufnehmen(wurzel: &Path, unterordner: &str, quelle: &Path) -> Result<Anhang, String> {
    if !quelle.is_file() {
        return Err(format!("{} ist keine Datei", quelle.display()));
    }
    let name = quelle
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "die Datei hat keinen Namen".to_string())?;
    // ⚑ **Der Name wird gesaeubert, aber nicht gegen Pfadflucht.**
    //
    // 📌 Das stand hier zuerst als Begruendung, und es war eine
    // Scheinbegruendung: `file_name()` gibt **immer genau eine
    // Komponente**, ein Schraegstrich kann darin gar nicht vorkommen.
    // Aufgefallen beim Gegenprobieren, weil die Probe dazu nicht biss.
    //
    // ⚑ **Wogegen es wirklich hilft:** Zeilenumbrueche,
    // Anfuehrungszeichen und Steuerzeichen im Namen. Sie sind erlaubt,
    // landen aber in einer Nachricht an das Modell und in einer
    // Pfadangabe, die ein Werkzeug zurueckbekommt. **Ein Dateiname mit
    // einem Zeilenumbruch zerlegt die Nachricht, in der er steht.**
    let sauber: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || "._- ".contains(c) { c } else { '_' })
        .collect();
    let sauber = sauber.trim_matches('.').to_string();
    let sauber = if sauber.is_empty() { "anhang".to_string() } else { sauber };

    let ordner = wurzel.join(unterordner);
    std::fs::create_dir_all(&ordner).map_err(|e| format!("{}: {e}", ordner.display()))?;

    // ⚑ **Ein vorhandener Name wird nicht ueberschrieben.** Zwei Dateien
    // gleichen Namens sind der Normalfall, und die zweite still zu
    // verlieren waere der Fehler, der erst beim Lesen auffaellt.
    let mut ziel = ordner.join(&sauber);
    let mut n = 1;
    while ziel.exists() {
        let stamm = Path::new(&sauber)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "anhang".into());
        let endung = Path::new(&sauber)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        ziel = ordner.join(format!("{stamm}-{n}{endung}"));
        n += 1;
    }
    std::fs::copy(quelle, &ziel).map_err(|e| format!("{}: {e}", ziel.display()))?;

    let bytes = std::fs::metadata(&ziel).map(|m| m.len()).unwrap_or(0);
    let anfang = lies_anfang(&ziel, 4096);
    let art = art_bestimmen(&anfang, &sauber);
    let auszug = if art == Art::Text {
        let t = String::from_utf8_lossy(&anfang);
        kurz(t.trim(), AUSZUG_ZEICHEN)
    } else {
        String::new()
    };
    let dateiname = ziel
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or(sauber);

    Ok(Anhang {
        pfad: format!("{unterordner}/{dateiname}"),
        name: dateiname,
        bytes,
        art,
        auszug,
    })
}

/// **Was ueber den Inhalt eines Anhangs bekannt ist**, wenn die
/// Nachricht geschrieben wird.
///
/// # ⚑ Warum das der Aufrufer entscheidet und nicht dieses Modul
///
/// Es haengt an Dingen, die hier niemand weiss: ob ein Sinnesmodell
/// eingerichtet ist, ob die Sitzung Werkzeuge hat, ob der Nutzer die
/// Schreiberlaubnis gegeben hat. **Ein Modul, das raet, raet falsch.**
#[derive(Debug, Clone, Copy)]
pub enum Sicht<'a> {
    /// Niemand sieht hin. Die Nachricht sagt das, statt es zu verschweigen.
    Nichts,
    /// Es gibt ein Werkzeug dafuer; das Modell muss es rufen.
    Werkzeug(&'a str),
    /// Schon angesehen; hier steht, was dabei herauskam.
    Angesehen(&'a str),
    /// **Es wird gerade angesehen, die Antwort haengt der Aufrufer an.**
    ///
    /// ⛔️ **Der Unterschied zu [`Sicht::Nichts`] ist der ganze Zweck
    /// dieser Variante** (Fund 436, 2026-09-23). Das Fenster schreibt
    /// die Anhangzeile **sofort**, denn ein Sehmodell braucht Sekunden
    /// bis Minuten und ein Fenster, das dabei einfriert, ist ein
    /// kaputtes Fenster. Es nahm dafuer `Nichts`, weil es „keine Zeile,
    /// die zu einem Werkzeugaufruf raet" meinte.
    ///
    /// **`Nichts` sagt aber nicht „nichts weiter", sondern „es ist kein
    /// Sinnesmodell eingerichtet, ueber ihren Inhalt ist nichts zu
    /// sagen".** Das stand dann in derselben Nachricht wie die
    /// Beschreibung, die gleich darauf angehaengt wurde, und das Modell
    /// las den ersten Satz zuerst: Es antwortete „leider kann ich keine
    /// Bilder sehen" und gab danach die Beschreibung wieder.
    ///
    /// 📌 **Ein Name, der weniger behauptet als sein Text, ist eine
    /// Falle.** Hier heisst die Variante nach dem, was gilt, und ihr
    /// Text ist leer, weil der Aufrufer die Ueberleitung mitbringt.
    Kommt,
}

impl Anhang {
    /// **Die Nachricht, die ins Gespraech geht.**
    ///
    /// ⛔️ **Sie nennt und liefert nicht**, mit zwei begruendeten
    /// Ausnahmen: Bei Text steht ein kurzer Auszug dabei, damit das
    /// Modell ohne Aufruf erkennt, ob es die richtige Datei ist; und bei
    /// Bild oder Ton steht die Beschreibung dabei, falls es eine gibt,
    /// **weil es sonst nichts gibt**.
    ///
    /// ⚠️ **Und sie sagt die Wahrheit ueber das, was das Modell selbst
    /// kann:** Es sieht das Bild nicht und hoert den Ton nicht. Ein
    /// Hinweis, der das verschweigt, laedt zu einer Antwort ein, die
    /// erfunden ist.
    pub fn nachricht(&self, deutsch: bool) -> String {
        self.nachricht_mit(deutsch, Sicht::Nichts)
    }

    /// Siehe [`Anhang::nachricht`], mit dem, was ueber den Inhalt bekannt ist.
    pub fn nachricht_mit(&self, deutsch: bool, sicht: Sicht<'_>) -> String {
        let groesse = menschlich(self.bytes);
        let mut t = if deutsch {
            format!(
                "[Datei angehaengt] {} ({}, {groesse}), liegt unter `{}`.",
                self.name,
                self.art.wort(true),
                self.pfad
            )
        } else {
            format!(
                "[file attached] {} ({}, {groesse}), stored at `{}`.",
                self.name,
                self.art.wort(false),
                self.pfad
            )
        };
        match self.art {
            // ⛔️ **Auch Text haengt an der Sicht** (Fund 439,
            //    2026-09-23). Hier stand `den Rest liest read_file`,
            //    ohne Bedingung. **Im Chat gibt es dieses Werkzeug
            //    nicht**, und dann nennt die Zeile eines, das nicht
            //    existiert: Das Modell antwortet auf „aendere die
            //    Datei" mit einer Anleitung, weil es nichts hat, womit
            //    es sie aendern koennte, und die Nachricht hat ihm das
            //    Gegenteil gesagt.
            //
            // 📌 **Dieselbe Klasse wie Fund 436**, nur andersherum:
            //    Dort behauptete die Zeile zu wenig, hier zu viel.
            Art::Text => {
                match sicht {
                    Sicht::Werkzeug(w) => t.push_str(&if deutsch {
                        format!(" Der Anfang steht unten; den Rest liest `{w}`.\n\n")
                    } else {
                        format!(" The beginning is below; `{w}` reads the rest.\n\n")
                    }),
                    _ => t.push_str(if deutsch {
                        " Der Anfang steht unten, und mehr ist hier nicht zu holen: \
                         In diesem Betrieb gibt es keine Dateiwerkzeuge, also auch \
                         keines, das sie liest oder aendert.\n\n"
                    } else {
                        " The beginning is below, and there is no more to be had here: \
                         this mode has no file tools, so none that reads or changes \
                         it either.\n\n"
                    }),
                }
                t.push_str(&self.auszug);
            }
            Art::Bild | Art::Ton => match sicht {
                Sicht::Angesehen(text) => {
                    t.push_str(if deutsch {
                        " Ein anderes Modell hat sie angesehen; das ist alles, was es dazu gibt:\n\n"
                    } else {
                        " A different model examined it; this is all there is:\n\n"
                    });
                    t.push_str(text);
                }
                Sicht::Werkzeug(w) => t.push_str(&if deutsch {
                    format!(
                        " ⚠️ Dieses Modell sieht Bilder nicht und hoert Ton nicht. \
                         Ruf `{w}` mit diesem Pfad auf; was dabei herauskommt, stammt \
                         von einem anderen Modell und ist alles, was es dazu gibt."
                    )
                } else {
                    format!(
                        " Note: this model does not see images or hear audio. Call \
                         `{w}` with this path; its answer comes from a different model \
                         and is all there is."
                    )
                }),
                // ⚑ **Kein Wort.** Wer gleich die Beschreibung anhaengt,
                //   braucht hier keinen Satz, und jeder Satz waere
                //   entweder doppelt oder falsch.
                Sicht::Kommt => {}
                Sicht::Nichts => t.push_str(if deutsch {
                    " ⚠️ Dieses Modell sieht Bilder nicht und hoert Ton nicht, und es ist \
                     kein Sinnesmodell eingerichtet. Ueber ihren Inhalt ist nichts zu sagen."
                } else {
                    " Note: this model does not see images or hear audio, and no sense \
                     model is set up. Nothing can be said about its content."
                }),
            },
            Art::Sonstiges => t.push_str(if deutsch {
                " `read_file` liest sie, wenn sie Text enthaelt."
            } else {
                " read_file will read it if it contains text."
            }),
        }
        t
    }
}

/// **Bytes, wie ein Mensch sie liest.** ⚑ Eine Stelle fuer die Nachricht
/// an das Modell und fuer die Uebersicht auf der Kommandozeile; zwei
/// Formate fuer dieselbe Zahl saehen wie zwei Zahlen aus.
pub fn menschlich(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1e6)
    } else {
        format!("{:.0} kB", (bytes as f64 / 1e3).max(1.0))
    }
}

/// Was liegt schon da?
pub fn vorhandene(wurzel: &Path, unterordner: &str) -> Vec<(String, u64)> {
    let mut aus: Vec<(String, u64)> = std::fs::read_dir(wurzel.join(unterordner))
    .into_iter()
    .flatten()
    .filter_map(|e| e.ok())
    .filter(|e| e.path().is_file())
    .map(|e| {
        (
            e.file_name().to_string_lossy().to_string(),
            e.metadata().map(|m| m.len()).unwrap_or(0),
        )
    })
    .collect();
    aus.sort();
    aus
}

fn lies_anfang(p: &Path, wieviel: usize) -> Vec<u8> {
    use std::io::Read;
    let mut puffer = vec![0u8; wieviel];
    match std::fs::File::open(p).and_then(|mut f| f.read(&mut puffer)) {
        Ok(n) => {
            puffer.truncate(n);
            puffer
        }
        Err(_) => Vec::new(),
    }
}

fn kurz(t: &str, breite: usize) -> String {
    if t.chars().count() <= breite {
        return t.to_string();
    }
    t.chars().take(breite - 1).chain(['…']).collect()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Derselbe Ort, den `myl-client` benutzt, damit die Proben denselben
    /// Weg gehen wie der Betrieb.
    const PROBEORDNER: &str = ".AGENT/anhaenge";

    fn wurzel() -> tempfile::TempDir {
        tempfile::tempdir().expect("Verzeichnis")
    }

    /// ⛔️ **Die Art haengt an den Bytes und nicht an der Endung.**
    #[test]
    fn die_bytes_entscheiden_und_nicht_der_name() {
        assert_eq!(art_bestimmen(b"\x89PNG\r\n\x1a\n", "irgendwas.txt"), Art::Bild);
        assert_eq!(art_bestimmen(b"Das ist Text.", "gefaelscht.png"), Art::Text);
        assert_eq!(art_bestimmen(b"RIFF\0\0\0\0WAVEfmt ", "x"), Art::Ton);
        assert_eq!(art_bestimmen(b"RIFF\0\0\0\0WEBPVP8 ", "x"), Art::Bild);
        assert_eq!(art_bestimmen(b"%PDF-1.7", "x"), Art::Sonstiges);
        // Schweigen die Bytes, zaehlt die Endung.
        assert_eq!(art_bestimmen(&[], "lied.mp3"), Art::Ton);
    }

    /// ⛔️ **Das Original bleibt, und der Name kommt nicht nach draussen.**
    #[test]
    fn kopiert_wird_und_der_name_wird_gesaeubert() {
        let d = wurzel();
        let quelle = d.path().join("bericht.md");
        std::fs::write(&quelle, "# Bericht\n\nEine Zeile Text.\n").expect("schreiben");
        let ziel = wurzel();

        let a = aufnehmen(ziel.path(), PROBEORDNER, &quelle).expect("aufnehmen");
        assert!(quelle.is_file(), "das Original ist weg");
        assert_eq!(a.art, Art::Text);
        assert!(a.pfad.starts_with(PROBEORDNER), "{}", a.pfad);
        assert!(ziel.path().join(&a.pfad).is_file(), "die Kopie fehlt");

        // ⛔️ **Die Zusage ist nicht „keine Punkte im Namen", sondern
        // „die Kopie bleibt im Anhangordner".**
        //
        // 📌 Die erste Fassung dieser Probe verlangte, dass kein `..` im
        // Namen steht, und fiel ueber `hoch..md`, wo zwei Punkte in der
        // Mitte stehen und voellig harmlos sind. **Eine Pruefung soll
        // die Gefahr treffen und nicht ihr Aussehen.**
        let anhangordner = ziel.path().join(PROBEORDNER).canonicalize().expect("Ordner");
        // ⚑ **Namen, die ein Dateisystem erlaubt und eine Nachricht
        // zerlegen:** Zeilenumbruch, Anfuehrungszeichen, Backtick.
        for name in ["zwei\nzeilen.md", "mit\"anfuehrung.md", "back`tick.md", "..punkte..md"] {
            let boese = d.path().join(name);
            std::fs::write(&boese, "x").expect("schreiben");
            let b = aufnehmen(ziel.path(), PROBEORDNER, &boese).expect("aufnehmen");
            assert!(
                !b.name.contains(['\n', '"', '`']),
                "der Name traegt noch Zeichen, die eine Nachricht zerlegen: {:?}",
                b.name
            );
            let wirklich = ziel.path().join(&b.pfad).canonicalize().expect("Kopie");
            assert!(
                wirklich.starts_with(&anhangordner),
                "die Kopie liegt ausserhalb: {}",
                wirklich.display()
            );
        }
    }

    /// ⚑ **Zwei gleiche Namen ueberschreiben einander nicht.**
    #[test]
    fn der_zweite_gleiche_name_bekommt_einen_eigenen() {
        let d = wurzel();
        let ziel = wurzel();
        let a = d.path().join("notiz.txt");
        std::fs::write(&a, "erste").expect("schreiben");
        let eins = aufnehmen(ziel.path(), PROBEORDNER, &a).expect("eins");
        std::fs::write(&a, "zweite").expect("schreiben");
        let zwei = aufnehmen(ziel.path(), PROBEORDNER, &a).expect("zwei");
        assert_ne!(eins.name, zwei.name, "der zweite hat den ersten ueberschrieben");
        assert_eq!(
            std::fs::read_to_string(ziel.path().join(&eins.pfad)).expect("lesen"),
            "erste",
            "der erste Anhang wurde veraendert"
        );
        assert_eq!(vorhandene(ziel.path(), PROBEORDNER).len(), 2);
    }

    /// ⛔️ **Die Nachricht sagt bei Bild und Ton die Wahrheit.**
    #[test]
    fn die_nachricht_nennt_und_liefert_nicht() {
        let d = wurzel();
        let ziel = wurzel();
        let bild = d.path().join("schaubild.png");
        std::fs::write(&bild, b"\x89PNG\r\n\x1a\nund noch etwas").expect("schreiben");
        let a = aufnehmen(ziel.path(), PROBEORDNER, &bild).expect("aufnehmen");
        let n = a.nachricht(true);
        assert!(n.contains("sieht Bilder nicht"), "{n}");
        assert!(n.contains(".AGENT/anhaenge/schaubild.png"), "{n}");

        // ⚑ **Gibt es ein Werkzeug, wird es genannt**, und der blosse
        // Hinweis auf das eigene Nichtsehen verschwindet nicht, sondern
        // bekommt einen Weg.
        let mit = a.nachricht_mit(true, Sicht::Werkzeug("bild_beschreiben"));
        assert!(mit.contains("bild_beschreiben"), "{mit}");
        assert!(mit.contains("sieht Bilder nicht"), "{mit}");
        assert!(!n.contains("bild_beschreiben"), "ohne Werkzeug wird eines genannt: {n}");

        // ⛔️ **Ist es schon angesehen, steht die Beschreibung da und
        // nicht mehr der Rat, ein Werkzeug zu rufen.** Ein Hinweis auf
        // einen Aufruf, der die Antwort schon geliefert hat, schickt das
        // Modell ein zweites Mal los.
        let gesehen = a.nachricht_mit(true, Sicht::Angesehen("Auf dem Schild steht HALT."));
        assert!(gesehen.contains("Auf dem Schild steht HALT."), "{gesehen}");
        assert!(!gesehen.contains("bild_beschreiben"), "{gesehen}");
        assert!(gesehen.contains("anderes Modell"), "die Herkunft wird verschwiegen: {gesehen}");

        // Bei Text steht ein Auszug dabei, aber nicht die ganze Datei.
        let lang = d.path().join("lang.txt");
        std::fs::write(&lang, "A".repeat(5000)).expect("schreiben");
        let t = aufnehmen(ziel.path(), PROBEORDNER, &lang).expect("aufnehmen");
        let nt = t.nachricht(true);
        assert!(nt.len() < AUSZUG_ZEICHEN + 400, "die Nachricht traegt die Datei: {}", nt.len());
        assert!(nt.contains('…'), "die Kuerzung sagt sich nicht an");
    }
}
