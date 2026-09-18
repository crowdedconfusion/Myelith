//! **Eine Antwort horbar machen**, ueber ein eigenes kleines
//! Sprachmodell.
//!
//! # ⚑ CosyVoice ist die Vorgabe (Festlegung des Projektinhabers, 2026-09-17)
//!
//! Es klingt am besten und **kann eine Stimme nachbilden**, und genau
//! das war der Auftrag: eine hochgeladene Aufnahme soll die Stimme sein.
//! Dafuer braucht es Python und Gewichte.
//!
//! ⚑ **Beides bringt der Nutzer mit, und nichts davon kommt ins
//! Repositorium.** Das ist die Bedingung, unter der die Entscheidung
//! steht: „davon ausgehen, dass die meisten Python bereits haben und
//! dies nicht ins Repo muss". Der Laeufer, der CosyVoice bedient, ist
//! dagegen eine Textdatei von wenigen Kilobyte; er steckt im Programm
//! und wird bei Bedarf in die Heimat geschrieben.
//!
//! ⚠️ **Was das kostet, gehoert dazugesagt:** Wer kein Python und keine
//! Gewichte hat, kann nicht sprechen lassen. Deshalb bleibt **piper als
//! Rueckfall** stehen, eine Binaerdatei ohne Laufzeit, die sofort geht
//! und dafuer nur fertige Stimmen kennt.
//!
//! # Die drei Wege, in dieser Reihenfolge
//!
//! | Weg | klont | braucht |
//! |---|---|---|
//! | eigenes Skript `<Heimat>/bin/sprechen` | ja | was immer es selbst will |
//! | **CosyVoice** | ja | Python, Gewichte, `MYL_COSYVOICE` |
//! | piper | **nein** | eine Binaerdatei und eine Stimme |
//!
//! ⚑ **Das eigene Skript gewinnt immer**, denn wer es hinlegt, hat
//! gewaehlt. Sein Interface ist fest, kein Befehlsmuster: **zwei oder
//! drei Pfade** (Textdatei, Ziel-WAV, und die Stimmprobe, falls eine
//! liegt). Ein Muster muesste eine Shell zerlegen, und die gibt es unter
//! Windows nicht.
//!
//! ⛔️ **Liegt eine Stimmprobe und spricht trotzdem piper, sagt der
//! Client das** (`Sinne::stimmhinweis`), statt sie stillschweigend
//! liegen zu lassen. **Ein Schalter ohne Wirkung ist schlimmer als
//! keiner**, denn wer eine Stimme hochlaedt und weiter dieselbe hoert,
//! sucht den Fehler bei sich.

use std::path::{Path, PathBuf};

use crate::laufwerk::{Sprechweg, Sprechzeug};

/// **Sagt einen Text und legt das Ergebnis als WAV ab.**
///
/// ⚠️ **Der Aufrufer raeumt die Datei weg.** Sie liegt im
/// Zwischenordner; wer sie abspielt, loescht sie danach.
pub fn sagen(zeug: &Sprechzeug, text: &str) -> Result<PathBuf, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("es gibt nichts zu sagen".into());
    }
    // ⚑ **Der Text geht als Datei und nicht als Argument.** Eine Antwort
    // kann laenger sein, als eine Kommandozeile fasst, und sie enthaelt
    // Zeichen, die anderswo zitiert werden muessten.
    let quelle = crate::zwischenname("myl-sprechen", "txt");
    std::fs::write(&quelle, text).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let ziel = crate::zwischenname("myl-sprechen", "wav");

    let ergebnis = ruf(zeug, &quelle, &ziel);
    let _ = std::fs::remove_file(&quelle);
    match ergebnis {
        Ok(()) if ziel.is_file() => Ok(ziel),
        Ok(()) => {
            let _ = std::fs::remove_file(&ziel);
            Err("das Sprechmodell hat keine Tondatei hinterlassen".into())
        }
        Err(f) => {
            let _ = std::fs::remove_file(&ziel);
            Err(f)
        }
    }
}

fn ruf(zeug: &Sprechzeug, quelle: &Path, ziel: &Path) -> Result<(), String> {
    let mut befehl = befehl_fuer(zeug);
    match &zeug.weg {
        // piper nimmt den Text von der Standardeingabe.
        Sprechweg::Piper { .. } => {
            let eingabe =
                std::fs::File::open(quelle).map_err(|f| format!("{}: {f}", quelle.display()))?;
            befehl.arg("--output_file").arg(ziel);
            let a = crate::prozess::laufen_mit_eingabe(
                &mut befehl,
                std::process::Stdio::from(eingabe),
                crate::FRIST_SPRECHEN_S,
                4096,
            )
            .map_err(|f| format!("das Sprechmodell {}: {f}", zeug.name()))?;
            return urteil(&a, zeug);
        }
        // Skript und Laeufer bekommen Pfade: Text, Ziel, und die
        // Stimmprobe als **dritten**, falls eine liegt.
        Sprechweg::Skript(_) | Sprechweg::CosyVoice { .. } => {
            befehl.arg(quelle).arg(ziel);
            if let Some(probe) = &zeug.probe {
                befehl.arg(probe);
            }
        }
    }
    let a = crate::prozess::laufen(&mut befehl, crate::FRIST_SPRECHEN_S, 4096)
        .map_err(|f| format!("das Sprechmodell {}: {f}", zeug.name()))?;
    urteil(&a, zeug)
}

fn urteil(a: &crate::prozess::Ausgang, zeug: &Sprechzeug) -> Result<(), String> {
    if a.gut() {
        return Ok(());
    }
    Err(format!(
        "Das Sprechmodell {} ist {}. Die letzten Zeilen:\n{}",
        zeug.name(),
        a.kopf(),
        crate::schwanz(&a.fehler, 8)
    ))
}

/// **Der Befehl, der diesen Weg geht**, ohne die Pfade, die je Satz
/// wechseln.
///
/// ⚑ **Eine Stelle fuer Einmalaufruf und Dauerlaeufer.** Beide starten
/// dasselbe Programm mit derselben Umgebung; nur die Pfade kommen
/// anders. Zwei Stellen waeren zwei Aufrufe, die auseinanderlaufen.
fn befehl_fuer(zeug: &Sprechzeug) -> std::process::Command {
    match &zeug.weg {
        Sprechweg::Skript(p) => std::process::Command::new(p),
        Sprechweg::Piper { programm, stimme } => {
            let mut b = std::process::Command::new(programm);
            b.arg("--model").arg(stimme);
            b
        }
        Sprechweg::CosyVoice { python, laeufer, wurzel } => {
            let mut b = std::process::Command::new(python);
            b.arg(laeufer);
            // ⚑ **Die Wurzel und der Probentext gehen ueber die
            // Umgebung**, nicht als Argumente: Sie gelten fuer den
            // ganzen Lauf, die Pfade wechseln je Satz.
            b.env(crate::laufwerk::COSYVOICE_UMGEBUNG, wurzel);
            if let Some(t) = &zeug.probentext {
                b.env("MYL_STIMMTEXT", t);
            }
            b
        }
    }
}

/// **Ein Sprechprogramm, das stehen bleibt.**
///
/// # ⛔️ Warum das bei CosyVoice kein Luxus ist
///
/// CosyVoice laedt je Aufruf ein halbes Milliardenmodell samt Vocoder.
/// **Satzweise zu sprechen waere damit langsamer als gar nicht zu
/// streamen**, wenn jeder Satz einen neuen Prozess braeuchte: Der
/// Ladevorgang stuende dann vor jedem Satz statt einmal.
///
/// # Das Gespraech mit dem Laeufer
///
/// | Richtung | Zeile |
/// |---|---|
/// | vom Laeufer, einmal beim Start | `bereit` |
/// | an den Laeufer, je Satz | `<textdatei>\t<zielwav>` |
/// | vom Laeufer, je Satz | `ok` oder `fehler: ...` |
///
/// ⚑ **Das Schliessen der Eingabe beendet ihn**, wie bei der Aufnahme.
#[derive(Debug)]
pub struct Dauersprecher {
    kind: std::process::Child,
    eingabe: Option<std::process::ChildStdin>,
    ausgabe: Option<std::io::BufReader<std::process::ChildStdout>>,
    name: String,
}

impl Dauersprecher {
    /// **Startet den Laeufer und wartet, bis er `bereit` meldet.**
    ///
    /// ⚠️ **Das Warten ist Teil der Sache.** Wer hier nicht wartet,
    /// schickt den ersten Satz in ein Programm, das noch laedt, und die
    /// Antwort kaeme irgendwann oder nie.
    pub fn starten(zeug: &Sprechzeug) -> Result<Self, String> {
        Self::starten_mit(zeug, HANDSCHLAG_FRIST_S)
    }

    /// **Derselbe Start mit gesagter Frist.**
    ///
    /// ⚑ **Die Naht fuer die Proben.** Eine Gegenprobe fuer einen
    /// Laeufer, der nie `bereit` sagt, muesste sonst zwei Minuten warten,
    /// und eine Probe, die zwei Minuten dauert, laeuft irgendwann
    /// niemand mehr.
    pub fn starten_mit(zeug: &Sprechzeug, frist_s: u64) -> Result<Self, String> {
        use std::io::BufRead;
        let mut befehl = befehl_fuer(zeug);
        befehl.arg("--dauer");
        if let Some(probe) = &zeug.probe {
            befehl.arg(probe);
        }
        let mut kind = befehl
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|f| format!("{} liess sich nicht starten: {f}", zeug.name()))?;
        // ⚠️ **Fremdzeilen ueberlesen, mit Deckel und mit Frist.**
        //
        // modelscope und tqdm schreiben beim Laden gern nach stdout; der
        // Laeufer schiebt das nach stderr, aber eine fremde Kiste, die
        // den Kanal umgeht, darf den Handschlag nicht zerlegen.
        //
        // ⛔️ **Und ein Zaehler allein genuegt nicht:** Ein Laeufer, der
        // eine Zeile schreibt und dann schweigt, laesst ein blockierendes
        // `read_line` fuer immer stehen. Deshalb liest ein eigener Faden,
        // und hier wird mit Frist gewartet.
        let ausgabe = std::io::BufReader::new(kind.stdout.take().expect("stdout"));
        let (sender, empfang) = std::sync::mpsc::channel::<String>();
        let leser = std::thread::spawn(move || {
            let mut ausgabe = ausgabe;
            for _ in 0..HANDSCHLAG_ZEILEN {
                let mut zeile = String::new();
                match ausgabe.read_line(&mut zeile) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let fertig = zeile.trim() == "bereit";
                        if sender.send(zeile).is_err() || fertig {
                            break;
                        }
                    }
                }
            }
            ausgabe
        });

        let frist = std::time::Duration::from_secs(frist_s);
        let anfang = std::time::Instant::now();
        let mut gemeldet = false;
        let mut zuletzt = String::new();
        while anfang.elapsed() < frist {
            match empfang.recv_timeout(frist - anfang.elapsed()) {
                Ok(zeile) => {
                    if zeile.trim() == "bereit" {
                        gemeldet = true;
                        break;
                    }
                    if !zeile.trim().is_empty() {
                        zuletzt = zeile.trim().to_string();
                    }
                }
                Err(_) => break,
            }
        }
        if !gemeldet {
            let _ = kind.kill();
            let _ = kind.wait();
            let dazu = if zuletzt.is_empty() {
                String::new()
            } else {
                format!(" (zuletzt: {zuletzt:?})")
            };
            return Err(format!("{} meldete kein `bereit`{dazu}", zeug.name()));
        }
        // ⚑ **Den Leser zurueckholen**, denn ab jetzt wird Zeile fuer
        // Zeile im Takt gelesen und nicht mehr nebenlaeufig.
        let ausgabe = leser.join().map_err(|_| "der Lesefaden ist abgestuerzt".to_string())?;

        Ok(Self {
            eingabe: kind.stdin.take(),
            ausgabe: Some(ausgabe),
            kind,
            name: zeug.name(),
        })
    }

    /// **Laesst einen Satz sprechen** und wartet auf seine Antwort.
    pub fn satz(&mut self, quelle: &Path, ziel: &Path) -> Result<(), String> {
        use std::io::{BufRead, Write};
        let ein = self.eingabe.as_mut().ok_or("der Laeufer ist schon zu")?;
        writeln!(ein, "{}\t{}", quelle.display(), ziel.display())
            .and_then(|()| ein.flush())
            .map_err(|f| format!("{} nimmt nichts mehr an: {f}", self.name))?;
        let aus = self.ausgabe.as_mut().ok_or("der Laeufer ist schon zu")?;
        let mut zeile = String::new();
        aus.read_line(&mut zeile).map_err(|f| format!("{} antwortet nicht: {f}", self.name))?;
        match zeile.trim() {
            "ok" => Ok(()),
            "" => Err(format!("{} ist ausgestiegen", self.name)),
            anderes => Err(format!("{}: {anderes}", self.name)),
        }
    }
}

impl Drop for Dauersprecher {
    fn drop(&mut self) {
        // Erst die Eingabe schliessen, damit er sauber aufhoert.
        self.eingabe = None;
        self.ausgabe = None;
        let frist = std::time::Duration::from_secs(5);
        let anfang = std::time::Instant::now();
        loop {
            match self.kind.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if anfang.elapsed() >= frist => {
                    let _ = self.kind.kill();
                    let _ = self.kind.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
    }
}

/// **Der Laeufer, den diese Kiste mitbringt**, als Text im Programm.
///
/// ⚑ **Eingebacken und nicht nachgeschlagen.** Ein Pfad zum
/// Quellverzeichnis waere fuer ein installiertes Programm falsch, und
/// eine Suche danach waere eine zweite, schlechtere Ortsbestimmung.
const COSYVOICE_QUELLE: &str = include_str!("../laeufer/sprechen-cosyvoice.py");

/// **Schreibt den Laeufer in die Heimat, falls er dort fehlt.**
///
/// ⛔️ **Er wird nie ueberschrieben.** Wer ihn angepasst hat, hat ihn
/// angepasst; eine Aktualisierung, die stillschweigend fremde Arbeit
/// wegwirft, ist ein Datenverlust mit gutem Gewissen. Zurueck kommt der
/// Pfad und ob geschrieben wurde.
pub fn laeufer_einrichten() -> Result<(PathBuf, bool), String> {
    let bin = crate::laufwerk::heimat().join("bin");
    std::fs::create_dir_all(&bin).map_err(|f| format!("{}: {f}", bin.display()))?;
    let ziel = bin.join(crate::laufwerk::COSYVOICE_LAEUFER);
    if ziel.is_file() {
        return Ok((ziel, false));
    }
    std::fs::write(&ziel, COSYVOICE_QUELLE).map_err(|f| format!("{}: {f}", ziel.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&ziel, std::fs::Permissions::from_mode(0o755));
    }
    Ok((ziel, true))
}

/// Wie kurz eine Stimmprobe hoechstens sein darf, in Bytes bei
/// 16 kHz, einem Kanal und 16 Bit: rund eine Sekunde.
///
/// ⚑ **Eine halbe Sekunde „hallo" bildet keine Stimme nach.** Wer das
/// hochlaedt, bekommt ein schlechtes Ergebnis und sucht den Fehler beim
/// Programm. Drei bis zehn Sekunden ruhig gesprochener Text sind das,
/// was die Klonverfahren wollen.
pub const PROBE_MINDESTENS_BYTES: u64 = 32_000;

/// **Legt eine Stimmprobe ab**, damit ein klonfaehiges Sprechprogramm
/// sie benutzen kann.
///
/// ⚑ **Ein fester Platz, wie bei allen Gewichten**
/// (`<Heimat>/stimme.wav`). Die Datei wird auf 16 kHz und einen Kanal
/// gebracht, falls noetig; das ist das Format, das die Klonverfahren
/// ohne Nachfragen annehmen.
///
/// ⚠️ **Ob das eingestellte Sprechprogramm sie verwerten kann, sagt
/// diese Funktion nicht**, sondern `Sinne::stimmhinweis`. Hier wird
/// abgelegt, dort wird geurteilt.
pub fn probe_setzen(quelle: &Path) -> Result<PathBuf, String> {
    if !quelle.is_file() {
        return Err(format!("die Datei '{}' gibt es nicht", quelle.display()));
    }
    // ⚑ **Dieselbe Artbestimmung wie beim Anhang**: an den ersten Bytes
    // und nicht an der Endung. Eine umbenannte Textdatei ist keine
    // Stimme.
    let anfang = {
        use std::io::Read;
        let mut puffer = vec![0u8; 4096];
        let n = std::fs::File::open(quelle)
            .and_then(|mut f| f.read(&mut puffer))
            .map_err(|f| format!("{}: {f}", quelle.display()))?;
        puffer.truncate(n);
        puffer
    };
    let name = quelle.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if crate::anhang::art_bestimmen(&anfang, &name) != crate::anhang::Art::Ton {
        return Err(format!("'{}' ist keine Tonaufnahme", quelle.display()));
    }

    let heimat = crate::laufwerk::heimat();
    std::fs::create_dir_all(&heimat).map_err(|f| format!("{}: {f}", heimat.display()))?;
    let ziel = heimat.join(crate::laufwerk::STIMMPROBE);

    // ⚑ **24 kHz, ein Kanal, und das ist kein Zufall**: CosyVoice liest
    // die Probe zweimal, bei 16 kHz fuer die Sprecherkennung und bei
    // 24 kHz fuer die Merkmale. Wer sie auf 16 kHz ablegt, wirft weg, was
    // das Sprechmodell fuer die zweite Lesung braucht, und das hoert man.
    // Ein WAV, das schon so vorliegt, wird trotzdem umgewandelt: Die
    // Abtastrate steht im Kopf, und ihr zu glauben, ohne sie zu lesen,
    // waere geraten.
    let gewandelt = crate::hoeren::nach_wav_mit(quelle, 24000)?;
    let gross = std::fs::metadata(&gewandelt).map(|m| m.len()).unwrap_or(0);
    if gross < PROBE_MINDESTENS_BYTES {
        let _ = std::fs::remove_file(&gewandelt);
        return Err("die Aufnahme ist zu kurz, um eine Stimme nachzubilden; drei bis zehn \
                    Sekunden ruhig gesprochener Text sind das Richtige"
            .into());
    }
    // ⚑ **Den Text gleich mitschreiben.** CosyVoice trifft die Stimme
    // mit `inference_zero_shot` deutlich besser, und dafuer braucht es
    // den gesprochenen Text der Probe. Wir haben ein Hoermodell; ihn
    // spaeter von Hand nachzutragen waere eine Aufgabe, die niemand
    // erledigt. ⚠️ Geht es nicht, ist das kein Fehler: Dann bleibt der
    // sprachuebergreifende Weg.
    let textziel = heimat.join(crate::laufwerk::STIMMPROBE_TEXT);
    let _ = std::fs::remove_file(&textziel);
    if let Ok(h) = crate::Sinne::finden().hoeren.as_ref() {
        // ⚠️ **whisper.cpp will 16 kHz**, die abgelegte Probe hat 24.
        // Also eine Kopie fuer das Mitschreiben, die gleich wieder weggeht.
        if let Ok(fuers_hoeren) = crate::hoeren::nach_wav_mit(&gewandelt, 16000) {
            if let Ok(t) = crate::hoeren::mitschreiben(h, &fuers_hoeren, "auto") {
                if !t.trim().is_empty() {
                    let _ = std::fs::write(&textziel, t.trim());
                }
            }
            let _ = std::fs::remove_file(&fuers_hoeren);
        }
    }

    std::fs::rename(&gewandelt, &ziel).or_else(|_| {
        // ⚑ Ueber Dateisystemgrenzen hinweg geht kein Umbenennen.
        std::fs::copy(&gewandelt, &ziel).map(|_| ()).inspect(|_| {
            let _ = std::fs::remove_file(&gewandelt);
        })
    }).map_err(|f| format!("{}: {f}", ziel.display()))?;
    Ok(ziel)
}

/// **Nimmt die Stimmprobe wieder weg.** `false`, wenn gar keine lag.
pub fn probe_entfernen() -> bool {
    let heimat = crate::laufwerk::heimat();
    // ⚑ **Der Text geht mit der Probe.** Ein Text ohne die Aufnahme, zu
    // der er gehoert, waere die naechste Stimme, die falsch klingt.
    let _ = std::fs::remove_file(heimat.join(crate::laufwerk::STIMMPROBE_TEXT));
    let p = heimat.join(crate::laufwerk::STIMMPROBE);
    p.is_file() && std::fs::remove_file(&p).is_ok()
}

/// **Ein Vorleser, der spricht, waehrend das Modell noch schreibt.**
///
/// # ⚑ Warum eine Schlange und kein Sprechen an Ort und Stelle
///
/// Zwei Dinge muessen zugleich stimmen, und sie widersprechen sich fast:
///
/// 1. **Die Erzeugung darf nicht warten.** Wer im Strom des Modells
///    steht und dort ein Sprechmodell startet, haelt die Erzeugung an,
///    und dann ist nichts gewonnen.
/// 2. **Die Saetze muessen in der Reihenfolge klingen.** Wer je Satz
///    einen Faden aufmacht, bekommt sie durcheinander, und zwei
///    Abspieler reden gleichzeitig.
///
/// ⚑ **Ein einziger Faden mit einer Schlange loest beides**: Der Strom
/// wirft Saetze hinein und laeuft weiter, der Faden arbeitet sie **der
/// Reihe nach** ab.
///
/// ⚠️ **Ein Fehler beim Sprechen haelt nichts an.** Er wird gesammelt
/// und am Ende gemeldet; eine Antwort, die dasteht, ist wichtiger als
/// eine, die auch klingt.
/// **Zerlegt eine Antwort in Stuecke, die einzeln gesprochen werden
/// koennen.**
///
/// # ⚑ Warum das der halbe Weg zum Live-Gefuehl ist
///
/// Die Wartezeit einer gesprochenen Antwort kommt fast ganz vom
/// **Hauptmodell**: Bei 14 Token je Sekunde sind hundert Token rund
/// sieben Sekunden. Wer erst spricht, wenn alles dasteht, laesst so
/// lange Stille; wer **satzweise** spricht, waehrend das Modell
/// weiterschreibt, ist nach ein bis zwei Sekunden hoerbar.
///
/// ⚑ **Hier wird nur zerlegt, nicht gesprochen.** Zurueck kommt
/// `(fertige Stuecke, Rest)`: Der Rest ist der angefangene Satz, auf den
/// noch gewartet wird.
///
/// ⚠️ **Ein Satzende ist ein Satzzeichen mit Leerzeichen danach.** Das
/// trifft „Dr. Meier" falsch und ist trotzdem richtig so: Ein Sprecher,
/// der dort eine Pause macht, stoert weniger als einer, der bis zum Ende
/// der Antwort schweigt.
pub fn satzweise(text: &str) -> (Vec<String>, String) {
    let mut stuecke = Vec::new();
    let mut lauf = String::new();
    let zeichen: Vec<char> = text.chars().collect();
    for (i, c) in zeichen.iter().enumerate() {
        lauf.push(*c);
        let endet = matches!(c, '.' | '!' | '?' | ':' | '\n')
            && zeichen.get(i + 1).is_none_or(|n| n.is_whitespace());
        // ⚑ **Kurze Fetzen bleiben stehen.** „Ja." allein zu sprechen
        // und dann neu anzusetzen klingt zerhackt.
        if endet && lauf.trim().chars().count() >= 24 {
            stuecke.push(lauf.trim().to_string());
            lauf.clear();
        }
    }
    (stuecke, lauf.trim_start().to_string())
}

/// Wer eine fertige Tondatei horbar macht.
///
/// ⚑ Ein eigener Name, weil die Schreibweise sonst an drei Stellen steht
/// und clippy zu Recht meckert.
/// Wie viele Zeilen der Handschlag hoechstens ueberliest, bevor er
/// aufgibt. Siehe [`Dauersprecher::starten`].
pub const HANDSCHLAG_ZEILEN: usize = 200;

/// Wie lange auf `bereit` gewartet wird, in Sekunden.
///
/// ⛔️ **Ein Zeilenzaehler allein genuegt nicht.** Ein Laeufer, der eine
/// Zeile schreibt und dann schweigt, laesst ein blockierendes
/// `read_line` **fuer immer** stehen, und mit ihm das Fenster. Gemerkt
/// hat das die eigene Probe, indem sie haengenblieb. ⚑ **Zwei Minuten**,
/// weil ein halbes Milliardenmodell von der Platte laedt.
pub const HANDSCHLAG_FRIST_S: u64 = 120;

/// Wer eine fertige Tondatei horbar macht.
pub type Abspieler = Box<dyn Fn(&std::path::Path) -> Result<(), String> + Send>;

/// **Ein Dauerlaeufer, den mehrere Antworten nacheinander benutzen.**
///
/// # ⛔️ Warum das kein Luxus ist, sondern der Unterschied
///
/// CosyVoice laedt rund achtzehn Sekunden. Ein Vorleser je Antwort
/// startet **je Antwort** einen neuen Laeufer, und damit stehen diese
/// achtzehn Sekunden **vor jeder Antwort**. Gemeldet vom
/// Projektinhaber am 2026-09-18: „im Live Modus braucht das Modell sehr
/// lange nach der Textgenerierung um zu antworten."
///
/// ⚑ **Wer ihn hier haelt, haelt ihn ueber Antworten hinweg**, und die
/// Ladezeit faellt genau einmal an. `None` heisst: noch nicht gestartet.
pub type Geteilter = std::sync::Arc<std::sync::Mutex<Option<Dauersprecher>>>;

/// **Startet den Laeufer im Voraus**, wenn noch keiner steht.
///
/// ⚑ **Vor der ersten Antwort und nicht bei ihr.** Waehrend das
/// Hauptmodell nachdenkt, kann der Sprecher laden; danach ist die
/// Wartezeit weg statt verschoben.
pub fn vorwaermen(zeug: &Sprechzeug, geteilt: &Geteilter) -> Result<(), String> {
    if !zeug.dauerhaft() {
        return Ok(());
    }
    let mut g = geteilt.lock().map_err(|_| "der Sprecherhalter ist vergiftet")?;
    if g.is_some() {
        return Ok(());
    }
    *g = Some(Dauersprecher::starten(zeug)?);
    Ok(())
}

pub struct Vorleser {
    sender: Option<std::sync::mpsc::Sender<String>>,
    faden: Option<std::thread::JoinHandle<Vec<String>>>,
    /// Der angefangene Satz, auf dessen Ende noch gewartet wird.
    rest: String,
}

impl Vorleser {
    /// Faengt an. Der Faden laeuft, bis [`Vorleser::abschliessen`] ruft.
    pub fn neu(zeug: &Sprechzeug) -> Self {
        Self::neu_mit(zeug, Box::new(abspielen))
    }

    /// **Dieselbe Schlange, aber mit einem gesagten Abspieler.**
    ///
    /// ⚑ **Die Naht fuer die Proben.** Ein Test, der wirklich abspielt,
    /// braeuchte Lautsprecher und eine echte Tondatei und prueft
    /// trotzdem nur, was hier zu pruefen ist: **die Reihenfolge**.
    pub fn neu_mit(zeug: &Sprechzeug, abspieler: Abspieler) -> Self {
        Self::neu_geteilt(zeug, abspieler, None)
    }

    /// **Derselbe Vorleser, aber mit einem Dauerlaeufer, der bleibt.**
    ///
    /// ⚑ Siehe [`Geteilter`]: Ohne ihn zahlt jede Antwort die Ladezeit
    /// des Sprechmodells von Neuem.
    pub fn neu_geteilt(
        zeug: &Sprechzeug,
        abspieler: Abspieler,
        geteilt: Option<Geteilter>,
    ) -> Self {
        let (sender, empfang) = std::sync::mpsc::channel::<String>();
        let zeug = zeug.clone();
        let faden = std::thread::spawn(move || {
            let mut fehler = Vec::new();
            // ⚑ **Der Dauerlaeufer wird beim ersten Satz gestartet, nicht
            // beim Anlegen.** Wer den Vorleser aufsetzt und dann doch
            // nichts sagt, soll kein Modell geladen haben.
            // ⚑ **Der geteilte Laeufer lebt laenger als dieser Vorleser**;
            // ein eigener nur so lange wie er.
            let eigener: Geteilter = geteilt.unwrap_or_default();
            for satz in empfang {
                let ergebnis = if zeug.dauerhaft() {
                    match eigener.lock() {
                        Err(_) => Err("der Sprecherhalter ist vergiftet".to_string()),
                        Ok(mut halter) => {
                            if halter.is_none() {
                                match Dauersprecher::starten(&zeug) {
                                    Ok(d) => *halter = Some(d),
                                    Err(f) => {
                                        fehler.push(f);
                                        // ⚠️ **Einmal melden, nicht je
                                        // Satz.** Ein Laeufer, der nicht
                                        // startet, startet auch beim
                                        // zehnten Satz nicht.
                                        break;
                                    }
                                }
                            }
                            let aus = ueber_dauer(halter.as_mut().expect("eben gesetzt"), &satz);
                            // ⚠️ **Ein Laeufer, der einen Satz nicht
                            // schafft, wird weggeworfen.** Sonst redet
                            // der naechste Vorleser gegen eine Leiche.
                            if aus.is_err() {
                                *halter = None;
                            }
                            aus
                        }
                    }
                } else {
                    sagen(&zeug, &satz)
                };
                match ergebnis {
                    Ok(wav) => {
                        if let Err(f) = abspieler(&wav) {
                            fehler.push(f);
                        }
                        let _ = std::fs::remove_file(&wav);
                    }
                    Err(f) => fehler.push(f),
                }
            }
            fehler
        });
        Self { sender: Some(sender), faden: Some(faden), rest: String::new() }
    }

    /// **Nimmt ein Stueck Text entgegen** und spricht, was daran fertig
    /// ist. Der angefangene Satz bleibt liegen.
    pub fn schub(&mut self, neu: &str) {
        self.rest.push_str(neu);
        let (fertig, rest) = satzweise(&self.rest);
        self.rest = rest;
        for satz in fertig {
            if let Some(s) = &self.sender {
                let _ = s.send(satz);
            }
        }
    }

    /// **Spricht den Rest und wartet, bis alles geklungen hat.**
    ///
    /// ⚠️ **Es wird gewartet, und das ist Absicht.** Ein Vorleser, der
    /// beim Abraeumen mitten im Satz abbricht, klingt kaputt. Zurueck
    /// kommen die Fehler, die unterwegs aufgetreten sind.
    pub fn abschliessen(mut self) -> Vec<String> {
        let rest = std::mem::take(&mut self.rest);
        if !rest.trim().is_empty() {
            if let Some(s) = &self.sender {
                let _ = s.send(rest.trim().to_string());
            }
        }
        // Der Sender muss weg, sonst endet die Schleife im Faden nie.
        self.sender = None;
        match self.faden.take() {
            Some(f) => f.join().unwrap_or_else(|_| vec!["der Vorlesefaden ist abgestuerzt".into()]),
            None => Vec::new(),
        }
    }
}

/// Einen Satz ueber den stehenden Laeufer sprechen lassen.
fn ueber_dauer(d: &mut Dauersprecher, satz: &str) -> Result<PathBuf, String> {
    let quelle = crate::zwischenname("myl-sprechen", "txt");
    std::fs::write(&quelle, satz).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let ziel = crate::zwischenname("myl-sprechen", "wav");
    let ergebnis = d.satz(&quelle, &ziel);
    let _ = std::fs::remove_file(&quelle);
    ergebnis?;
    if ziel.is_file() {
        Ok(ziel)
    } else {
        Err("der Laeufer meldete `ok` und hat keine Tondatei hinterlassen".into())
    }
}

impl Drop for Vorleser {
    fn drop(&mut self) {
        // ⚑ **Ein fallengelassener Vorleser haengt nicht.** Er hoert auf,
        // sobald der laufende Satz fertig ist; gewartet wird nur, damit
        // kein Abspieler verwaist zurueckbleibt.
        self.sender = None;
        if let Some(f) = self.faden.take() {
            let _ = f.join();
        }
    }
}

/// **Spielt eine Tondatei ab**, mit dem Abspieler des Systems.
///
/// ⚑ **Fuer die Konsole.** Im Fenster tut das der Webview selbst, und
/// zwar besser: Er kann anhalten, und er braucht kein fremdes Programm.
/// ⚠️ **Fehlt der Abspieler, ist das kein Absturz**, sondern ein Satz,
/// der sagt, welche Datei bereitliegt.
/// **Wie lang eine WAV-Datei spielt**, aus ihrem Kopf gelesen.
///
/// ⚑ **Damit bekommt das Abspielen eine Frist, die zur Sache passt.**
/// Ein Abspieler, der auf ein Geraet wartet, das es nicht gibt, blockiert
/// sonst die volle Sprechfrist **je Satz**; bei satzweisem Vorlesen ist
/// das ein stehendes Fenster. `None`, wenn der Kopf nichts hergibt, und
/// dann gilt die Vorgabe.
fn wav_dauer_s(wav: &Path) -> Option<u64> {
    use std::io::Read;
    let mut kopf = [0u8; 44];
    std::fs::File::open(wav).ok()?.read_exact(&mut kopf).ok()?;
    if &kopf[0..4] != b"RIFF" || &kopf[8..12] != b"WAVE" {
        return None;
    }
    // Bytes je Sekunde stehen im Formatblock an Stelle 28.
    let je_sekunde = u32::from_le_bytes([kopf[28], kopf[29], kopf[30], kopf[31]]) as u64;
    if je_sekunde == 0 {
        return None;
    }
    let bytes = std::fs::metadata(wav).ok()?.len().saturating_sub(44);
    Some(bytes / je_sekunde)
}

pub fn abspielen(wav: &Path) -> Result<(), String> {
    let (programm, vorne): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("afplay", &[])
    } else if cfg!(target_os = "windows") {
        ("powershell", &["-NoProfile", "-Command"])
    } else {
        ("aplay", &["-q"])
    };
    let gefunden = crate::laufwerk::programm_suchen(
        &[programm],
        None,
        &crate::laufwerk::heimat(),
        &crate::laufwerk::pfadordner(),
    )
    .ok_or_else(|| {
        format!("kein Abspieler gefunden ({programm}); die Antwort liegt als {}", wav.display())
    })?;
    let mut befehl = std::process::Command::new(gefunden);
    for v in vorne {
        befehl.arg(v);
    }
    if cfg!(target_os = "windows") {
        befehl.arg(format!("(New-Object Media.SoundPlayer '{}').PlaySync()", wav.display()));
    } else {
        befehl.arg(wav);
    }
    // ⚑ **Die Frist folgt der Laenge des Stuecks**, plus etwas Anlauf.
    // Ein Abspieler, der nach der doppelten Spieldauer noch laeuft,
    // spielt nicht, sondern wartet auf etwas.
    let frist = wav_dauer_s(wav).map(|d| d * 2 + 5).unwrap_or(crate::FRIST_SPRECHEN_S);
    let a = crate::prozess::laufen(&mut befehl, frist, 1024)
        .map_err(|f| format!("der Abspieler: {f}"))?;
    if a.gut() {
        return Ok(());
    }
    // ⚠️ **Der haeufigste Fall hat einen Namen.** Ein Prozess ohne
    // Audiositzung (etwa aus einem Dienst oder einer Werkzeugkette
    // heraus) laesst `afplay` auf ein Geraet warten, das er nicht
    // bekommt; das sieht wie ein Fehler des Sprechens aus und ist keiner.
    if a.abgebrochen {
        return Err(format!(
            "der Abspieler kam nach {frist} s nicht zurueck; hat dieser Prozess ueberhaupt \
             eine Tonausgabe? Die Antwort liegt als {}",
            wav.display()
        ));
    }
    Err(format!("der Abspieler ist {}", a.kopf()))
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Was fertig ist, geht raus; der angefangene Satz wartet.**
    #[test]
    fn satzweise_gibt_fertiges_heraus_und_behaelt_den_rest() {
        let (fertig, rest) = satzweise("Das ist der erste Satz, und er ist lang genug. Der zwei");
        assert_eq!(fertig, vec!["Das ist der erste Satz, und er ist lang genug."]);
        assert_eq!(rest, "Der zwei");
    }

    /// ⚑ **Ein kurzer Fetzen wird nicht einzeln gesprochen**, sonst
    /// klaenge die Antwort zerhackt.
    #[test]
    fn kurze_fetzen_bleiben_stehen() {
        let (fertig, rest) = satzweise("Ja. Nein.");
        assert!(fertig.is_empty(), "{fertig:?}");
        assert_eq!(rest, "Ja. Nein.");
    }

    /// ⛔️ **Ohne Satzzeichen bleibt alles Rest.** Ein Sprecher, der bei
    /// einer Aufzaehlung ohne Punkt losspricht, schneidet mitten hinein.
    #[test]
    fn ohne_satzzeichen_wird_nicht_geschnitten() {
        let lang = "eine sehr lange Zeile ganz ohne jedes Satzzeichen die einfach weitergeht";
        let (fertig, rest) = satzweise(lang);
        assert!(fertig.is_empty());
        assert_eq!(rest, lang);
    }

    /// ⚑ **Mehrere Saetze auf einmal kommen einzeln heraus**, in der
    /// Reihenfolge, in der sie gesprochen werden sollen.
    #[test]
    fn mehrere_saetze_kommen_einzeln() {
        let (fertig, rest) = satzweise(
            "Der erste Satz ist lang genug zum Sprechen. Der zweite Satz ist es auch, ganz sicher. ",
        );
        assert_eq!(fertig.len(), 2);
        assert!(fertig[0].starts_with("Der erste"));
        assert!(fertig[1].starts_with("Der zweite"));
        assert!(rest.is_empty(), "{rest:?}");
    }
}
