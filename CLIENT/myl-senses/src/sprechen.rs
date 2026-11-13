//! **Eine Antwort horbar machen**, ueber ein eigenes kleines
//! Sprachmodell.
//!
//! # ⚑ Qwen3-TTS im eigenen Rechenweg ist die Vorgabe (seit 2026-11-11)
//!
//! Festlegung des Projektinhabers fuer Qwen3-TTS (Apache 2.0, ohne Python)
//! und Abnahme gegen CosyVoice auf derselben Maschine bestanden: gleiche
//! Wortfehler, frueherer erster Ton, keine Luecken, schneller geladen
//! ([`crate::laufwerk::EIGEN_ALS_VORGABE`], [`crate::eigensprecher`]). Es
//! klont aus der Stimmprobe, mit ihrem Text auch mit Mitschrift.
//!
//! ⚑ **CosyVoice ist seit CLIENT 0.219.0 entfernt** (Festlegung des
//! Projektinhabers, 2026-11-11: Lizenzvorbehalt der Modellkarte, Python und
//! PyTorch). **piper bleibt Rueckfall**, eine Binaerdatei ohne Laufzeit,
//! nur mit fertigen Stimmen; fremde Klonmodelle gehen ueber ein eigenes
//! Skript.
//!
//! # Die Wege, in dieser Reihenfolge
//!
//! | Weg | klont | braucht |
//! |---|---|---|
//! | eigenes Skript `<Heimat>/bin/sprechen`, je Satz | ja | was immer es selbst will |
//! | eigenes Skript `<Heimat>/bin/sprechen-dauer`, stehend | ja | dasselbe |
//! | **Qwen3-TTS (eigen)** | ja | die Gewichte unter `qwen3-tts-0.6b-base` |
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

    // ⛔️ Gekennzeichnet, bevor es jemand bekommt; sonst gar nicht.
    let ergebnis = ruf(zeug, &quelle, &ziel).and_then(|()| {
        if ziel.is_file() {
            crate::kennzeichnung::synthetisch_kennzeichnen(&ziel, crate::kennzeichnung::Modalitaet::Sprache)
                .map_err(|f| format!("nicht gekennzeichnet, deshalb nicht gespielt: {f}"))
        } else {
            Ok(())
        }
    });
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
    // Der eigene Weg rechnet im Prozess: einmal laden, einen Satz sprechen.
    if let Sprechweg::Eigen { ordner } = &zeug.weg {
        let e = crate::eigensprecher::Eigensprecher::laden(ordner, zeug.probe.as_deref(), zeug.probentext.as_deref())?;
        return eigen_satz(&e, quelle, ziel);
    }
    // ⚑ **Ein stehendes Skript spricht auch einen einzelnen Satz ueber sein
    //   Gespraech** (`--dauer`): Dokumentiert ist nur dieses, ein zweiter
    //   Aufruf mit Pfaden waere eine Schnittstelle, die niemand kennt.
    //   Gekennzeichnet ist danach schon; `sagen` erkennt das.
    if let Sprechweg::Dauerskript(_) = &zeug.weg {
        return Dauersprecher::starten(zeug)?.satz(quelle, ziel);
    }
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
        Sprechweg::Skript(_) | Sprechweg::Dauerskript(_) | Sprechweg::Eigen { .. } => {
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

/// **Ein Satz ueber den eigenen Weg**: Text aus `quelle`, Ton nach `ziel`
/// (16 Bit, 24 kHz). Gekennzeichnet wird beim Aufrufer.
fn eigen_satz(e: &crate::eigensprecher::Eigensprecher, quelle: &Path, ziel: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(quelle).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let g = e.satz(&text)?;
    std::fs::write(ziel, crate::ton::schreiben(&g.ton)).map_err(|f| format!("{}: {f}", ziel.display()))
}

/// **Ein Satz ueber den eigenen Weg, gestroemt**: Ein Faden waehlt und
/// wandelt, hier wird jedes Stueck abgelegt, gekennzeichnet und
/// weitergereicht, waehrend der Faden schon am naechsten rechnet. Der ganze
/// Satz liegt danach gekennzeichnet in `ziel`.
///
/// ⛔️ Wie bei jedem Laeufer: Was nicht gekennzeichnet werden kann, klingt
/// nicht (Art. 50 Abs. 2 KI-Verordnung).
fn eigen_stueckweise(
    e: &crate::eigensprecher::Eigensprecher,
    quelle: &Path,
    ziel: &Path,
    stueck: &mut dyn FnMut(&Path),
) -> Result<usize, String> {
    let text = std::fs::read_to_string(quelle).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let (sender, empfang) = std::sync::mpsc::channel::<Vec<f32>>();
    let mut stuecke = 0usize;
    let mut nicht_gekennzeichnet: Option<String> = None;
    let ergebnis = std::thread::scope(|bereich| {
        let faden = bereich.spawn(move || e.satz_stueckweise(&text, &mut |ton| {
            let _ = sender.send(ton);
        }));
        for proben in empfang {
            let teil = crate::zwischenname("myl-sprechen-stueck", "wav");
            let ton = crate::ton::Ton { rate: crate::tonzerleger::RATE, proben };
            let abgelegt = std::fs::write(&teil, crate::ton::schreiben(&ton)).map_err(|f| format!("{}: {f}", teil.display()));
            match abgelegt.and_then(|()| crate::kennzeichnung::synthetisch_kennzeichnen(&teil, crate::kennzeichnung::Modalitaet::Sprache)) {
                Ok(()) => {
                    stueck(&teil);
                    stuecke += 1;
                }
                Err(f) => {
                    let _ = std::fs::remove_file(&teil);
                    nicht_gekennzeichnet.get_or_insert(f);
                }
            }
        }
        faden.join().unwrap_or_else(|_| Err("der Sprechfaden ist abgestuerzt".into()))
    });
    let g = ergebnis?;
    std::fs::write(ziel, crate::ton::schreiben(&g.ton)).map_err(|f| format!("{}: {f}", ziel.display()))?;
    if let Err(f) = crate::kennzeichnung::synthetisch_kennzeichnen(ziel, crate::kennzeichnung::Modalitaet::Sprache) {
        let _ = std::fs::remove_file(ziel);
        nicht_gekennzeichnet.get_or_insert(f);
    }
    match nicht_gekennzeichnet {
        Some(f) => Err(format!("nicht gekennzeichnet, deshalb nicht gespielt: {f}")),
        None => Ok(stuecke),
    }
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
        // ⚠️ Kein Programm: [`ruf`] und [`Dauersprecher::starten_mit`]
        // nehmen den eigenen Weg vorher ab.
        Sprechweg::Eigen { .. } => unreachable!("der eigene Weg startet kein Programm"),
        Sprechweg::Piper { programm, stimme } => {
            let mut b = std::process::Command::new(programm);
            b.arg("--model").arg(stimme);
            b
        }
        Sprechweg::Dauerskript(p) => {
            let mut b = std::process::Command::new(p);
            // ⚑ **Der Probentext geht ueber die Umgebung**, nicht als
            // Argument: Er gilt fuer den ganzen Lauf, die Pfade wechseln
            // je Satz.
            if let Some(t) = &zeug.probentext {
                b.env("MYL_STIMMTEXT", t);
            }
            b
        }
    }
}

/// **Ein Sprechprogramm, das stehen bleibt.**
///
/// # ⛔️ Warum das kein Luxus ist
///
/// Ein Sprechmodell laedt je Start ein halbes Milliardenmodell samt
/// Tonzerleger. **Satzweise zu sprechen waere damit langsamer als gar nicht
/// zu streamen**, wenn jeder Satz neu starten muesste: Der Ladevorgang
/// stuende dann vor jedem Satz statt einmal. Als Prozess laeuft das ueber
/// ein stehendes Skript ([`crate::laufwerk::DAUERSKRIPT`]).
///
/// # Das Gespraech mit dem Laeufer
///
/// | Richtung | Zeile |
/// |---|---|
/// | vom Laeufer, einmal beim Start | `bereit` |
/// | an den Laeufer, je Satz | `<textdatei>\t<zielwav>` |
/// | vom Laeufer, null- bis mehrmal je Satz (Fassung 2) | `stueck\t<teilwav>` |
/// | vom Laeufer, je Satz | `ok` oder `fehler: ...` |
///
/// ⚑ **Das Schliessen der Eingabe beendet ihn**, wie bei der Aufnahme.
///
/// ⚑ **Der eigene Weg ist ein Dauersprecher ohne Prozess**
/// ([`crate::eigensprecher`]): Er haelt die Gewichte im Programm statt in
/// einem Laeufer, und jeder Satz geht denselben Weg bis zur Kennzeichnung.
#[derive(Debug)]
pub struct Dauersprecher {
    kind: Option<std::process::Child>,
    eingabe: Option<std::process::ChildStdin>,
    ausgabe: Option<std::io::BufReader<std::process::ChildStdout>>,
    eigen: Option<Box<crate::eigensprecher::Eigensprecher>>,
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
        if let Sprechweg::Eigen { ordner } = &zeug.weg {
            let e = crate::eigensprecher::Eigensprecher::laden(ordner, zeug.probe.as_deref(), zeug.probentext.as_deref())?;
            return Ok(Self { kind: None, eingabe: None, ausgabe: None, eigen: Some(Box::new(e)), name: zeug.name() });
        }
        let mut befehl = befehl_fuer(zeug);
        befehl.arg("--dauer");
        if let Some(probe) = &zeug.probe {
            befehl.arg(probe);
        }
        let mut kind = crate::prozess::starten(
            befehl
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null()),
        )
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
            kind: Some(kind),
            eigen: None,
            name: zeug.name(),
        })
    }

    /// **Laesst einen Satz sprechen** und wartet auf seine Antwort.
    ///
    /// Der ganze Satz liegt danach in `ziel`; Stuecke, die ein Laeufer
    /// unterwegs meldet, werden hier weggeraeumt.
    pub fn satz(&mut self, quelle: &Path, ziel: &Path) -> Result<(), String> {
        self.satz_stueckweise(quelle, ziel, &mut |teil| {
            let _ = std::fs::remove_file(teil);
        })
        .map(|_| ())
    }

    /// **Laesst einen Satz sprechen und reicht jedes Stueck weiter,
    /// sobald es klingt.** Zurueck kommt die Zahl der Stuecke.
    ///
    /// # ⚑ Warum Stuecke
    ///
    /// Ein Laeufer der Fassung 2 rechnet im Strom und meldet je rund einer
    /// Sekunde Ton eine Zeile `stueck\t<pfad>`, dann `ok`. Wer die Stuecke
    /// sofort abspielt, hoert den Satz nach dem ersten Stueck statt nach
    /// dem ganzen: gemessen 2,1 bis 2,3 s statt 7 bis 9 s.
    ///
    /// ⚑ **Ein Laeufer der Fassung 1 meldet keine Stuecke**, nur `ok`.
    /// Dann ist die Zahl null, und der Aufrufer nimmt den ganzen Satz aus
    /// `ziel`, wie bisher. Ein alter, selbst angepasster Laeufer spricht
    /// also weiter, nur nicht gestroemt.
    pub fn satz_stueckweise(
        &mut self,
        quelle: &Path,
        ziel: &Path,
        stueck: &mut dyn FnMut(&Path),
    ) -> Result<usize, String> {
        use std::io::{BufRead, Write};
        if let Some(e) = &self.eigen {
            return eigen_stueckweise(e, quelle, ziel, stueck);
        }
        let ein = self.eingabe.as_mut().ok_or("der Laeufer ist schon zu")?;
        writeln!(ein, "{}\t{}", quelle.display(), ziel.display())
            .and_then(|()| ein.flush())
            .map_err(|f| format!("{} nimmt nichts mehr an: {f}", self.name))?;
        let aus = self.ausgabe.as_mut().ok_or("der Laeufer ist schon zu")?;
        let mut stuecke = 0usize;
        // ⛔️ **Was nicht gekennzeichnet werden kann, klingt nicht**
        //   (Art. 50 Abs. 2 KI-Verordnung). Jedes Stueck und das ganze
        //   Satz-WAV gehen durch `synthetisch_kennzeichnen`, bevor jemand
        //   sie bekommt; scheitert das, wird das Stueck verworfen und der
        //   Satz als Fehler gemeldet. Ein ungekennzeichneter Ton waere
        //   genau der Fall, den die Kennzeichnung ausschliessen soll.
        let mut nicht_gekennzeichnet: Option<String> = None;
        loop {
            let mut zeile = String::new();
            aus.read_line(&mut zeile)
                .map_err(|f| format!("{} antwortet nicht: {f}", self.name))?;
            let zeile = zeile.trim();
            if let Some(pfad) = zeile.strip_prefix("stueck\t") {
                let pfad = Path::new(pfad);
                match crate::kennzeichnung::synthetisch_kennzeichnen(pfad, crate::kennzeichnung::Modalitaet::Sprache) {
                    Ok(()) => {
                        stueck(pfad);
                        stuecke += 1;
                    }
                    Err(f) => {
                        let _ = std::fs::remove_file(pfad);
                        nicht_gekennzeichnet.get_or_insert(f);
                    }
                }
                continue;
            }
            return match zeile {
                "ok" => {
                    if ziel.is_file() {
                        if let Err(f) = crate::kennzeichnung::synthetisch_kennzeichnen(
                            ziel,
                            crate::kennzeichnung::Modalitaet::Sprache,
                        ) {
                            let _ = std::fs::remove_file(ziel);
                            nicht_gekennzeichnet.get_or_insert(f);
                        }
                    }
                    match nicht_gekennzeichnet {
                        Some(f) => Err(format!("nicht gekennzeichnet, deshalb nicht gespielt: {f}")),
                        None => Ok(stuecke),
                    }
                }
                "" => Err(format!("{} ist ausgestiegen", self.name)),
                anderes => Err(format!("{}: {anderes}", self.name)),
            };
        }
    }
}

impl Drop for Dauersprecher {
    fn drop(&mut self) {
        // Erst die Eingabe schliessen, damit er sauber aufhoert.
        self.eingabe = None;
        self.ausgabe = None;
        let Some(kind) = self.kind.as_mut() else { return };
        let frist = std::time::Duration::from_secs(5);
        let anfang = std::time::Instant::now();
        loop {
            match kind.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if anfang.elapsed() >= frist => {
                    let _ = kind.kill();
                    let _ = kind.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
    }
}

/// FNV-1a mit 64 Bit, genug, um eine Datei wiederzuerkennen; keine
/// Pruefsumme gegen Absicht, nur gegen Verwechslung.
pub(crate) fn fingerabdruck(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// **Die Saetze zum Ueberbruecken**, je Sprache eine kleine Auswahl.
///
/// ⚑ Kurz, damit sie vor der Antwort ausgeklungen sind, und ohne
/// Versprechen ueber den Inhalt: Sie sagen nur, dass nachgedacht wird.
pub fn ueberbrueckungen(sprache: &str) -> &'static [&'static str] {
    match sprache {
        "en" => &[
            "Let me think about that for a moment.",
            "Good question, give me a second.",
            "Hmm, let me think this through.",
            "One moment, I am thinking it over.",
            "Let me consider that briefly.",
        ],
        _ => &[
            "Lass mich kurz darüber nachdenken.",
            "Gute Frage, einen Moment bitte.",
            "Hm, lass mich das kurz durchdenken.",
            "Einen Augenblick, ich überlege kurz.",
            "Moment, darüber denke ich kurz nach.",
        ],
    }
}

/// **Wo ein vorbereiteter Satz zum Ueberbruecken liegt.**
///
/// ⚑ **Der Name haengt an allem, was den Klang bestimmt**: am Skript
/// oder an den Gewichten, an der Stimmprobe samt Text und am Satz. Wer
/// die Stimme wechselt, bekommt neue Saetze statt der alten Stimme.
pub fn ueberbrueckungsort(zeug: &Sprechzeug, satz: &str) -> PathBuf {
    let mut kennung = Vec::new();
    match &zeug.weg {
        Sprechweg::Skript(p) | Sprechweg::Dauerskript(p) => kennung.extend(std::fs::read(p).unwrap_or_default()),
        Sprechweg::Eigen { ordner } => kennung.extend(ordner.to_string_lossy().as_bytes()),
        Sprechweg::Piper { stimme, .. } => kennung.extend(stimme.to_string_lossy().as_bytes()),
    }
    if let Some(p) = &zeug.probe {
        kennung.extend(std::fs::read(p).unwrap_or_default());
    }
    if let Some(t) = &zeug.probentext {
        kennung.extend(std::fs::read(t).unwrap_or_default());
    }
    kennung.extend(satz.as_bytes());
    // ⚑ Die Fassung der Kennzeichnung gehoert zum Klang: Mit geformtem
    //   Wasserzeichen und beruhigten Pausen entstehen die Saetze neu.
    kennung.extend(format!("kennzeichnung {} {} {}", crate::kennzeichnung::LPC_GAMMA, crate::kennzeichnung::TEILER, crate::kennzeichnung::RUHE_AB).as_bytes());
    ablage(zeug)
        .join(UEBERBRUECKUNGSORDNER)
        .join(format!("{:016x}.wav", fingerabdruck(&kennung)))
}

/// **Die Heimat, aus der dieses Sprechzeug stammt.**
///
/// ⚑ Abgeleitet aus dem Skript (`<Heimat>/bin/<skript>`) und nicht aus
/// der Umgebung: Wer die Sinne in einer anderen Heimat gefunden hat,
/// etwa eine Probe in einem eigenen Verzeichnis, legt auch dort ab, statt
/// in die echte Heimat des Nutzers zu schreiben.
fn ablage(zeug: &Sprechzeug) -> PathBuf {
    let skript = match &zeug.weg {
        Sprechweg::Skript(p) | Sprechweg::Dauerskript(p) => Some(p.as_path()),
        Sprechweg::Eigen { .. } | Sprechweg::Piper { .. } => None,
    };
    skript
        .and_then(Path::parent)
        .filter(|bin| bin.file_name().is_some_and(|n| n == "bin"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(crate::laufwerk::heimat)
}

/// Der Ordner der vorbereiteten Saetze in der Heimat.
pub const UEBERBRUECKUNGSORDNER: &str = "ueberbrueckung";

/// **Einer der vorbereiteten Saetze, zufaellig**, oder keiner, wenn noch
/// keiner abgelegt ist.
fn vorbereitete_ueberbrueckung(zeug: &Sprechzeug, sprache: &str) -> Option<PathBuf> {
    let da: Vec<PathBuf> = ueberbrueckungen(sprache)
        .iter()
        .map(|s| ueberbrueckungsort(zeug, s))
        .filter(|p| p.is_file())
        .collect();
    if da.is_empty() {
        return None;
    }
    // ⚑ Die Uhr als Wuerfel: Es geht um Abwechslung, nicht um Zufall.
    let wurf = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    Some(da[wurf % da.len()].clone())
}

/// **Legt die Saetze zum Ueberbruecken ab**, die noch fehlen, ueber den
/// geteilten Laeufer. Zurueck kommt, wie viele neu entstanden.
///
/// ⚑ **Satz fuer Satz mit eigener Sperre**: Kommt dazwischen eine echte
/// Antwort, wartet sie hoechstens einen kurzen Satz lang. ⚑ Nur fuer
/// einen Dauerlaeufer; ein Laeufer, der je Satz neu laedt, braeuchte
/// dafuer eine halbe Minute.
pub fn ueberbrueckungen_vorbereiten(
    zeug: &Sprechzeug,
    geteilt: &Geteilter,
    sprache: &str,
) -> Result<usize, String> {
    if !zeug.dauerhaft() {
        return Ok(0);
    }
    let mut neu = 0;
    for satz in ueberbrueckungen(sprache) {
        let ort = ueberbrueckungsort(zeug, satz);
        if ort.is_file() {
            continue;
        }
        if let Some(ordner) = ort.parent() {
            std::fs::create_dir_all(ordner).map_err(|f| format!("{}: {f}", ordner.display()))?;
        }
        let mut g = geteilt
            .lock()
            .map_err(|_| "der Sprecherhalter ist vergiftet")?;
        if g.is_none() {
            *g = Some(Dauersprecher::starten(zeug)?);
        }
        let quelle = crate::zwischenname("myl-ueberbrueckung", "txt");
        std::fs::write(&quelle, satz).map_err(|f| format!("{}: {f}", quelle.display()))?;
        let ziel = crate::zwischenname("myl-ueberbrueckung", "wav");
        let aus = g.as_mut().expect("eben gesetzt").satz(&quelle, &ziel);
        let _ = std::fs::remove_file(&quelle);
        aus?;
        // ⚑ Erst fertig, dann an seinen Platz: Ein halb geschriebener
        //   Satz im Ordner waere einer, der abgebrochen klingt.
        std::fs::rename(&ziel, &ort)
            .or_else(|_| std::fs::copy(&ziel, &ort).map(|_| ()))
            .map_err(|f| format!("{}: {f}", ort.display()))?;
        let _ = std::fs::remove_file(&ziel);
        neu += 1;
    }
    ueberbrueckungen_aufraeumen(zeug);
    Ok(neu)
}

/// **Raeumt Saetze weg, die zu keiner jetzigen Stimme mehr gehoeren.**
///
/// 📌 **Ohne das waechst der Ordner mit jeder Stimme.** Der Name eines
/// Satzes haengt an Laeufer, Probe und Text; wer eine neue Probe
/// hochlaedt oder einen neuen Laeufer bekommt, bekommt fuenf neue
/// Dateien, und die alten bleiben liegen. Gesehen am 2026-09-25 mit
/// fuenfzehn Dateien fuer fuenf Saetze.
///
/// ⚑ **Weg geht nur, was nach diesem Ordner aussieht**: sechzehn
/// Hexziffern und `.wav`. Eine fremde Datei bleibt, wo sie ist. Behalten
/// wird die Auswahl **jeder** Sprache, denn wer die Sprache umstellt,
/// soll seine Saetze nicht neu rechnen lassen muessen.
fn ueberbrueckungen_aufraeumen(zeug: &Sprechzeug) {
    let behalten: Vec<PathBuf> = ["de", "en"]
        .iter()
        .flat_map(|sprache| ueberbrueckungen(sprache))
        .map(|satz| ueberbrueckungsort(zeug, satz))
        .collect();
    let Some(ordner) = behalten.first().and_then(|p| p.parent()) else { return };
    let Ok(eintraege) = std::fs::read_dir(ordner) else { return };
    for e in eintraege.flatten() {
        let pfad = e.path();
        let unser = pfad.extension().is_some_and(|x| x == "wav")
            && pfad.file_stem().and_then(|s| s.to_str()).is_some_and(|s| {
                s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
            });
        if unser && !behalten.contains(&pfad) {
            let _ = std::fs::remove_file(&pfad);
        }
    }
}

/// Wie kurz eine Stimmprobe hoechstens sein darf, in Bytes bei
/// 16 kHz, einem Kanal und 16 Bit: rund eine Sekunde.
///
/// ⚑ **Eine halbe Sekunde „hallo" bildet keine Stimme nach.** Wer das
/// hochlaedt, bekommt ein schlechtes Ergebnis und sucht den Fehler beim
/// Programm. Drei bis zehn Sekunden ruhig gesprochener Text sind das,
/// was die Klonverfahren wollen.
pub const PROBE_MINDESTENS_BYTES: u64 = 32_000;

/// Ab wo eine Stimmprobe geschnitten werden darf, in Millisekunden.
pub const PROBE_SCHNITT_AB_MS: u64 = 3_500;
/// Bis wo eine Stimmprobe hoechstens reicht, in Millisekunden. Laenger
/// wird sie an der leisesten Stelle davor geschnitten.
///
/// ⚑ **Die Probe ist bei jedem Satz fester Aufwand.** Ein klonendes
/// Sprechmodell rechnet ihre Token jedesmal mit (der eigene Weg setzt ihre
/// Codes vor jeden Satz), und das erste hoerbare Stueck wartet auf sie.
/// Gemessen am 2026-09-25 mit CosyVoice und sechs Flussschritten:
/// 7,1 s Probe brauchten je Satz rund 0,5 s mehr als 4,4 s, und die
/// Stimme traf dabei kaum anders (Aehnlichkeit 0,859 bis 0,888 gegen
/// 0,854 bis 0,870; zurueckgehoert verstand whisper beide ganz).
pub const PROBE_SCHNITT_BIS_MS: u64 = 5_500;

/// **Kuerzt eine Stimmprobe an einer Sprechpause**, falls sie laenger
/// als [`PROBE_SCHNITT_BIS_MS`] ist. `true`, wenn geschnitten wurde.
///
/// ⚑ **An der leisesten Stelle und nicht an einer festen Zeit.** Ein
/// Schnitt mitten im Wort liesse das Sprechmodell eine halbe Silbe als Ende der
/// Stimme lernen, und der mitgeschriebene Text passte nicht mehr. Gesucht
/// wird der Abschnitt von 20 ms mit der kleinsten Summe der Betraege
/// zwischen [`PROBE_SCHNITT_AB_MS`] und [`PROBE_SCHNITT_BIS_MS`], und
/// geschnitten in seiner Mitte.
///
/// ⚠️ **Nur fuer 16 Bit, ein Kanal**, also das, was [`probe_setzen`]
/// selbst ablegt; alles andere bleibt, wie es ist.
pub fn probe_kuerzen(wav: &Path) -> Result<bool, String> {
    let bytes = std::fs::read(wav).map_err(|f| format!("{}: {f}", wav.display()))?;
    let Some((rate, daten)) = wav_pcm16_mono(&bytes) else {
        return Ok(false);
    };
    let je_ms = rate as u64 / 1000;
    let proben = (daten.end - daten.start) as u64 / 2;
    if je_ms == 0 || proben <= PROBE_SCHNITT_BIS_MS * je_ms {
        return Ok(false);
    }
    let probe = |i: u64| {
        let o = daten.start + 2 * i as usize;
        i16::from_le_bytes([bytes[o], bytes[o + 1]]).unsigned_abs() as u64
    };
    let abschnitt = 20 * je_ms;
    let mut bester = (u64::MAX, PROBE_SCHNITT_BIS_MS * je_ms);
    let mut anfang = PROBE_SCHNITT_AB_MS * je_ms;
    while anfang + abschnitt <= PROBE_SCHNITT_BIS_MS * je_ms {
        let laut: u64 = (anfang..anfang + abschnitt).map(probe).sum();
        if laut < bester.0 {
            bester = (laut, anfang + abschnitt / 2);
        }
        anfang += abschnitt / 2;
    }
    let ende = daten.start + 2 * bester.1 as usize;
    let mut neu = Vec::with_capacity(44 + ende - daten.start);
    let datenlaenge = (ende - daten.start) as u32;
    neu.extend_from_slice(b"RIFF");
    neu.extend_from_slice(&(36 + datenlaenge).to_le_bytes());
    neu.extend_from_slice(b"WAVEfmt ");
    neu.extend_from_slice(&16u32.to_le_bytes());
    neu.extend_from_slice(&1u16.to_le_bytes()); // PCM
    neu.extend_from_slice(&1u16.to_le_bytes()); // ein Kanal
    neu.extend_from_slice(&rate.to_le_bytes());
    neu.extend_from_slice(&(rate * 2).to_le_bytes());
    neu.extend_from_slice(&2u16.to_le_bytes());
    neu.extend_from_slice(&16u16.to_le_bytes());
    neu.extend_from_slice(b"data");
    neu.extend_from_slice(&datenlaenge.to_le_bytes());
    neu.extend_from_slice(&bytes[daten.start..ende]);
    std::fs::write(wav, neu).map_err(|f| format!("{}: {f}", wav.display()))?;
    Ok(true)
}

/// **Rate und Datenbereich eines WAV mit 16 Bit und einem Kanal**, aus
/// seinen Bloecken gelesen ([`crate::ton::kopf`]). ⚠️ ffmpeg schreibt gern
/// einen `LIST`-Block vor die Daten; wer 44 Bytes Kopf annimmt, liest ihn
/// als Ton.
fn wav_pcm16_mono(b: &[u8]) -> Option<(u32, std::ops::Range<usize>)> {
    let k = crate::ton::kopf(b).ok()?;
    (k.art == crate::ton::Probenart::Ganzzahl(16) && k.kanaele == 1).then_some((k.rate, k.daten))
}

/// **Legt eine Stimmprobe ab**, damit ein klonfaehiges Sprechprogramm
/// sie benutzen kann.
///
/// ⚑ **Ein fester Platz, wie bei allen Gewichten**
/// (`<Heimat>/stimme.wav`). Die Datei wird auf 24 kHz und einen Kanal
/// gebracht, ein WAV ohne fremdes Programm, jedes andere Format mit ffmpeg; das ist das Format, das die Klonverfahren
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

    // ⚑ **24 kHz, ein Kanal, und das ist kein Zufall**: Der eigene Weg
    // liest die Probe fuer Abdruck und Tonzerleger bei 24 kHz. Wer sie auf
    // 16 kHz ablegt, wirft weg, was das Sprechmodell braucht, und das hoert
    // man.
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
    // ⚑ **Erst kuerzen, dann mitschreiben**: Der Text muss zu dem passen,
    // was das Sprechmodell hoert. Geht das Kuerzen schief, bleibt die Probe lang,
    // und das kostet Zeit und nichts sonst.
    let _ = probe_kuerzen(&gewandelt);
    // ⚑ **Den Text gleich mitschreiben.** Der eigene Weg trifft die
    // Stimme mit Mitschrift besser, und dafuer braucht es den gesprochenen
    // Text der Probe. Wir haben ein Hoermodell; ihn
    // spaeter von Hand nachzutragen waere eine Aufgabe, die niemand
    // erledigt. ⚠️ Geht es nicht, ist das kein Fehler: Dann bleibt der
    // Abdruck allein.
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
    satzweise_mit(text, false)
}

/// **Wie [`satzweise`], und das erste Stueck darf am Komma enden.**
///
/// ⚑ **Der erste Ton wartet auf den ersten ganzen Satz**, und den muss
/// das Hauptmodell erst schreiben: Bei 11 Token je Sekunde sind hundert
/// Zeichen gut zwei Sekunden. Am Anfang einer Antwort darf deshalb ein
/// Komma, Semikolon oder Gedankenstrich das Stueck beenden, sobald es
/// mindestens [`ERSTES_STUECK_MINDESTENS`] Zeichen hat. Danach gilt wieder
/// das Satzende: Die Stimme laeuft dann dem Modell voraus, und ganze
/// Saetze klingen natuerlicher.
pub fn satzweise_mit(text: &str, kurz_zuerst: bool) -> (Vec<String>, String) {
    let mut stuecke = Vec::new();
    let mut lauf = String::new();
    let zeichen: Vec<char> = text.chars().collect();
    for (i, c) in zeichen.iter().enumerate() {
        lauf.push(*c);
        let danach_frei = zeichen.get(i + 1).is_none_or(|n| n.is_whitespace());
        let endet = matches!(c, '.' | '!' | '?' | ':' | '\n')
            && danach_frei
            && !(*c == '.' && ist_abkuerzung(&lauf));
        let atmet = kurz_zuerst
            && stuecke.is_empty()
            && matches!(c, ',' | ';' | '\u{2013}' | '\u{2014}')
            && danach_frei
            && lauf.trim().chars().count() >= ERSTES_STUECK_MINDESTENS;
        // ⚑ **Kurze Fetzen bleiben stehen.** „Ja." allein zu sprechen
        // und dann neu anzusetzen klingt zerhackt.
        if (endet && lauf.trim().chars().count() >= 24) || atmet {
            stuecke.push(lauf.trim().to_string());
            lauf.clear();
        }
    }
    (stuecke, lauf.trim_start().to_string())
}

/// **Endet der Lauf mit einer Abkuerzung und ihrem Punkt?**
///
/// 📌 Gesehen am 2026-09-25: Aus „(z. B. Glutamat)" wurde ein eigenes
/// Stueck „(z.", und die Stimme sprach es als Satz mit Pause. Geprueft
/// wird das letzte Wort vor dem Punkt gegen eine kleine Liste; eine
/// Abkuerzung, die fehlt, kostet eine Pause und nichts sonst.
fn ist_abkuerzung(lauf: &str) -> bool {
    const KURZ: &[&str] = &[
        "z", "b", "d", "h", "u", "a", "o", "s", "bzw", "ca", "etc", "usw", "dr", "nr", "prof",
        "vgl", "ggf", "evtl", "inkl", "bspw", "sog", "e.g", "i.e", "vs", "min", "max", "mio",
        "mrd",
    ];
    let ohne_punkt = lauf.trim_end().trim_end_matches('.');
    let wort = ohne_punkt
        .rsplit(|c: char| c.is_whitespace() || c == '(' || c == '"' || c == '\u{201e}')
        .next()
        .unwrap_or("")
        .to_lowercase();
    KURZ.contains(&wort.as_str())
}

/// **Macht aus einem Stueck Antwort sprechbaren Text.**
///
/// ⛔️ **Markdown gehoert nicht in die Stimme.** Gesehen am 2026-09-25:
/// „Eine **Synapse** ist der **Verbindungspunkt**" ging mit den Sternen
/// an das Sprechmodell (damals CosyVoice). Das kostet Token und im schlechten Fall eine
/// gesprochene Silbe oder einen verstolperten Rhythmus. Hier gehen
/// Hervorhebungen, Code-Striche, Ueberschriftszeichen, Aufzaehlungs-
/// zeichen am Zeilenanfang und Tabellenstriche weg; ein Verweis behaelt
/// seinen Text und verliert die Adresse.
pub fn sprechbar(stueck: &str) -> String {
    let mut aus = String::with_capacity(stueck.len());
    for zeile in stueck.lines() {
        let mut z = zeile.trim_start();
        z = z.trim_start_matches('#').trim_start();
        for marke in ["- ", "* ", "+ ", "> "] {
            if let Some(rest) = z.strip_prefix(marke) {
                z = rest;
            }
        }
        // „1. " am Zeilenanfang ist eine Aufzaehlung und keine Zahl im Satz.
        let ziffern = z.chars().take_while(char::is_ascii_digit).count();
        if ziffern > 0 && z[ziffern..].starts_with(". ") {
            z = &z[ziffern + 2..];
        }
        if !aus.is_empty() {
            aus.push(' ');
        }
        aus.push_str(z);
    }
    // Verweise: [Text](Adresse) -> Text
    let mut ohne_verweise = String::with_capacity(aus.len());
    let mut rest = aus.as_str();
    while let Some(auf) = rest.find('[') {
        let Some(zu) = rest[auf..].find("](") else {
            break;
        };
        let Some(ende) = rest[auf + zu..].find(')') else {
            break;
        };
        ohne_verweise.push_str(&rest[..auf]);
        ohne_verweise.push_str(&rest[auf + 1..auf + zu]);
        rest = &rest[auf + zu + ende + 1..];
    }
    ohne_verweise.push_str(rest);
    ohne_verweise
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`' | '|' | '#'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// **Was von einem Stueck an die Stimme geht**, oder nichts, wenn es
/// nach [`sprechbar`] kein einziges Wort mehr traegt (eine Trennlinie,
/// ein leerer Aufzaehlungspunkt). Ein solches Stueck saehe fuer die
/// Stimme aus wie ein Satz und kostete eine Pause.
pub fn zu_sprechen(stueck: &str) -> Option<String> {
    let s = sprechbar(stueck);
    s.chars().any(char::is_alphanumeric).then_some(s)
}

/// Wie lang das erste Stueck einer Antwort mindestens ist, wenn es an
/// einem Komma endet: rund drei, vier Woerter. Kuerzer klaenge es wie
/// ein Stocken.
pub const ERSTES_STUECK_MINDESTENS: usize = 20;

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
/// Ein Sprechmodell laedt Sekunden (CosyVoice damals rund achtzehn, der
/// eigene Weg 1,2). Ein Vorleser je Antwort
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
    sender: Option<std::sync::mpsc::Sender<Auftrag>>,
    faden: Option<std::thread::JoinHandle<Vec<String>>>,
    /// Der angefangene Satz, auf dessen Ende noch gewartet wird.
    rest: String,
    /// Ob schon ein Stueck hinausging; bis dahin darf eines am Komma
    /// enden ([`satzweise_mit`]).
    angefangen: bool,
    /// Ob fuer diese Antwort schon ueberbrueckt wurde; es gibt hoechstens
    /// einen Satz dafuer.
    ueberbrueckt: bool,
    /// Wie viele Saetze abgeschickt und noch nicht fertig gesprochen sind.
    offen: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Ob von der Antwort selbst schon etwas klang (ein Satz zum
    /// Ueberbruecken zaehlt nicht). Siehe [`Vorrang`].
    ton: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Wie lange [`Vorrang::abwarten`] die Erzeugung hoechstens anhaelt.
///
/// ⚠️ **Eine Obergrenze, weil ein Sprecher haengen kann.** Die Antwort
/// auf dem Schirm ist wichtiger als ihr Klang; ein Sprecher, der nach
/// sechs Sekunden noch nichts hat, bekommt keinen Vorrang mehr.
pub const VORRANG_HOECHSTENS_MS: u64 = 6_000;

/// **Laesst den ersten Ton einer Antwort vorgehen.**
///
/// # ⚑ Warum die Erzeugung dafuer kurz anhaelt
///
/// Modell und Sprecher rechnen auf derselben Maschine und teilen sich
/// Kerne und Speicherbandbreite. Gemessen am 2026-09-25: Das erste Stueck
/// brauchte beim Sprecher allein rund 1,7 s bis zum ersten Ton, neben
/// einem schreibenden 8B 2,9 s, neben einem 4B 2,6 s. Das Modell ist dem
/// Sprechen ohnehin weit voraus (fertig nach 11 s, gesprochen bis 35 s).
/// **Haelt es an, bis der erste Ton da ist**, sank diese Spanne auf 2,2 s
/// (8B) und 2,1 s (4B), und der erste Ton kam beim 4B nach 3,5 statt
/// 3,85 s.
///
/// ⚠️ **Nur fuer den ersten Ton.** Ein Rueckstau, der das Modell bei
/// jedem offenen Satz anhaelt, liess es hinter das Sprechen zurueckfallen
/// und brachte zehn Sekunden Luecken (30B, eine Stufe).
#[derive(Clone)]
pub struct Vorrang {
    offen: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ton: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Vorrang {
    /// **Wartet, solange ein Satz der Antwort unterwegs ist und noch
    /// nichts klang**, hoechstens [`VORRANG_HOECHSTENS_MS`]. Kehrt sofort
    /// zurueck, wenn nichts unterwegs ist oder schon etwas klang; `true`,
    /// wenn gewartet wurde.
    pub fn abwarten(&self) -> bool {
        use std::sync::atomic::Ordering::SeqCst;
        let bis = std::time::Instant::now()
            + std::time::Duration::from_millis(VORRANG_HOECHSTENS_MS);
        let mut gewartet = false;
        while self.offen.load(SeqCst) > 0
            && !self.ton.load(SeqCst)
            && std::time::Instant::now() < bis
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
            gewartet = true;
        }
        gewartet
    }
}

/// Was der Faden des Vorlesers tun soll.
enum Auftrag {
    /// Einen Satz sprechen.
    Satz(String),
    /// Einen vorbereiteten Satz zum Ueberbruecken spielen, in dieser Sprache.
    Ueberbruecken(String),
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
        let (sender, empfang) = std::sync::mpsc::channel::<Auftrag>();
        let zeug = zeug.clone();
        let offen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let offen_im_faden = std::sync::Arc::clone(&offen);
        let ton = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ton_im_faden = std::sync::Arc::clone(&ton);
        let faden = std::thread::spawn(move || {
            let mut fehler = Vec::new();
            // ⚑ **Zwei Abspieler aus einem**: Der fuer die Antwort merkt
            //   sich, dass etwas klang ([`Vorrang`]), der fuer den Satz zum
            //   Ueberbruecken nicht.
            let fuer_saetze = |wav: &Path| {
                ton_im_faden.store(true, std::sync::atomic::Ordering::SeqCst);
                abspieler(wav)
            };
            // ⚑ **Der Dauerlaeufer wird beim ersten Satz gestartet, nicht
            // beim Anlegen.** Wer den Vorleser aufsetzt und dann doch
            // nichts sagt, soll kein Modell geladen haben.
            // ⚑ **Der geteilte Laeufer lebt laenger als dieser Vorleser**;
            // ein eigener nur so lange wie er.
            let eigener: Geteilter = geteilt.unwrap_or_default();
            for auftrag in empfang {
                let satz = match auftrag {
                    Auftrag::Satz(s) => s,
                    Auftrag::Ueberbruecken(sprache) => {
                        // ⚑ **Nur Vorbereitetes**, und ohne Warten: Einen
                        //   Satz zum Ueberbruecken erst zu rechnen, hielte
                        //   die Antwort auf, die er ueberbruecken soll.
                        if let Some(wav) = vorbereitete_ueberbrueckung(&zeug, &sprache) {
                            // ⛔️ Auch ein Satz aus der Ablage klingt nur
                            //   gekennzeichnet. 📌 Die Ablage stammt womoeglich
                            //   aus der Zeit vor der Kennzeichnung, und ihr
                            //   Name haengt nicht daran; gemessen am
                            //   2026-09-25 an den eigenen Saetzen, die alle
                            //   ohne Marke dalagen. Eine schon gekennzeichnete
                            //   Datei kostet hier nur das Lesen.
                            match crate::kennzeichnung::synthetisch_kennzeichnen(
                                &wav,
                                crate::kennzeichnung::Modalitaet::Sprache,
                            ) {
                                Ok(()) => {
                                    if let Err(f) = abspieler(&wav) {
                                        fehler.push(f);
                                    }
                                }
                                Err(f) => fehler.push(format!("nicht gekennzeichnet, deshalb nicht gespielt: {f}")),
                            }
                        }
                        continue;
                    }
                };
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
                            let aus = ueber_dauer(
                                halter.as_mut().expect("eben gesetzt"),
                                &satz,
                                &fuer_saetze,
                                &mut fehler,
                            );
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
                    sagen(&zeug, &satz).map(|wav| {
                        if let Err(f) = fuer_saetze(&wav) {
                            fehler.push(f);
                        }
                        let _ = std::fs::remove_file(&wav);
                    })
                };
                if let Err(f) = ergebnis {
                    fehler.push(f);
                }
                offen_im_faden.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
            // ⚠️ **Endet der Faden vorzeitig** (etwa weil der Laeufer nicht
            //   startet), bleiben Saetze gezaehlt, die nie gesprochen
            //   werden. Wer auf den Rueckstand wartet, wartete dann ewig.
            offen_im_faden.store(0, std::sync::atomic::Ordering::SeqCst);
            fehler
        });
        Self {
            sender: Some(sender),
            faden: Some(faden),
            rest: String::new(),
            angefangen: false,
            ueberbrueckt: false,
            offen,
            ton,
        }
    }

    /// **Nimmt ein Stueck Text entgegen** und spricht, was daran fertig
    /// ist. Der angefangene Satz bleibt liegen.
    pub fn schub(&mut self, neu: &str) {
        self.rest.push_str(neu);
        let (fertig, rest) = satzweise_mit(&self.rest, !self.angefangen);
        self.rest = rest;
        self.angefangen |= !fertig.is_empty();
        for satz in fertig {
            let Some(satz) = zu_sprechen(&satz) else {
                continue;
            };
            if let Some(s) = &self.sender {
                self.offen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if s.send(Auftrag::Satz(satz)).is_err() {
                    self.offen.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        }
    }

    /// **Ueberbrueckt das Nachdenken mit einem kurzen Satz**, hoechstens
    /// einmal je Antwort und nur, solange noch nichts gesprochen ist.
    ///
    /// ⚑ **Auftrag des Projektinhabers (2026-09-25):** Waehrend das Modell
    /// nachdenkt, soll ein Satz wie „Lass mich kurz darueber nachdenken"
    /// kommen, aus einer kleinen Auswahl. Das Nachdenken dauert beim
    /// grossen Modell zehn Sekunden und mehr, und Stille nach einer Frage
    /// klingt wie ein Aussetzer. Gespielt wird nur, was
    /// [`ueberbrueckungen_vorbereiten`] schon abgelegt hat.
    pub fn ueberbruecken(&mut self, sprache: &str) {
        if self.ueberbrueckt || self.angefangen {
            return;
        }
        self.ueberbrueckt = true;
        if let Some(s) = &self.sender {
            let _ = s.send(Auftrag::Ueberbruecken(sprache.to_string()));
        }
    }

    /// **Wie viele Saetze noch auf die Stimme warten**, den gerade
    /// gesprochenen eingeschlossen. Fuer den Rueckstau, siehe
    /// [`Vorleser::zaehler`].
    pub fn rueckstand(&self) -> usize {
        self.offen.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// **Der Griff fuer den Vorrang des ersten Tons**, zum Warten
    /// ausserhalb der Sperre, unter der der Vorleser liegt.
    pub fn vorrang(&self) -> Vorrang {
        Vorrang {
            offen: std::sync::Arc::clone(&self.offen),
            ton: std::sync::Arc::clone(&self.ton),
        }
    }

    /// **Der Zaehler selbst**, fuer einen Beobachter, der ihn lesen will,
    /// ohne den Vorleser zu sperren.
    pub fn zaehler(&self) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        std::sync::Arc::clone(&self.offen)
    }

    /// **Spricht den Rest und wartet, bis alles geklungen hat.**
    ///
    /// ⚠️ **Es wird gewartet, und das ist Absicht.** Ein Vorleser, der
    /// beim Abraeumen mitten im Satz abbricht, klingt kaputt. Zurueck
    /// kommen die Fehler, die unterwegs aufgetreten sind.
    pub fn abschliessen(mut self) -> Vec<String> {
        if let (Some(rest), Some(s)) = (zu_sprechen(&std::mem::take(&mut self.rest)), &self.sender)
        {
            self.offen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = s.send(Auftrag::Satz(rest));
        }
        // Der Sender muss weg, sonst endet die Schleife im Faden nie.
        self.sender = None;
        match self.faden.take() {
            Some(f) => f.join().unwrap_or_else(|_| vec!["der Vorlesefaden ist abgestuerzt".into()]),
            None => Vec::new(),
        }
    }
}

/// **Einen Satz ueber den stehenden Laeufer sprechen lassen** und jedes
/// Stueck sofort abspielen.
///
/// ⚑ Meldet der Laeufer keine Stuecke (Fassung 1), wird der ganze Satz
/// aus dem Ziel gespielt, wie bisher. Jede Tondatei geht nach dem
/// Abspielen weg; ein Fehler beim Abspielen wird gesammelt und haelt den
/// Satz nicht an.
fn ueber_dauer(
    d: &mut Dauersprecher,
    satz: &str,
    abspieler: &dyn Fn(&Path) -> Result<(), String>,
    fehler: &mut Vec<String>,
) -> Result<(), String> {
    let quelle = crate::zwischenname("myl-sprechen", "txt");
    std::fs::write(&quelle, satz).map_err(|f| format!("{}: {f}", quelle.display()))?;
    let ziel = crate::zwischenname("myl-sprechen", "wav");
    let ergebnis = d.satz_stueckweise(&quelle, &ziel, &mut |teil| {
        if let Err(f) = abspieler(teil) {
            fehler.push(f);
        }
        let _ = std::fs::remove_file(teil);
    });
    let _ = std::fs::remove_file(&quelle);
    let stuecke = match ergebnis {
        Ok(n) => n,
        Err(f) => {
            let _ = std::fs::remove_file(&ziel);
            return Err(f);
        }
    };
    let aus = if stuecke > 0 {
        Ok(())
    } else if ziel.is_file() {
        abspieler(&ziel).or_else(|f| {
            fehler.push(f);
            Ok(())
        })
    } else {
        Err("der Laeufer meldete `ok` und hat keine Tondatei hinterlassen".into())
    };
    let _ = std::fs::remove_file(&ziel);
    aus
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

/// **Wie lang eine WAV-Datei spielt**, aus ihrem Kopf gelesen.
///
/// ⚑ **Damit bekommt das Abspielen eine Frist, die zur Sache passt.**
/// Ein Abspieler, der auf ein Geraet wartet, das es nicht gibt, blockiert
/// sonst die volle Sprechfrist **je Satz**; bei satzweisem Vorlesen ist
/// das ein stehendes Fenster. `None`, wenn der Kopf nichts hergibt, und
/// dann gilt die Vorgabe.
///
/// 📌 Hier stand ein fester Kopf von 44 Bytes; mit einem `LIST`-Block vor
/// den Daten waere die Dauer um dessen Groesse falsch gewesen. Jetzt aus
/// den Bloecken ([`crate::ton::kopf`]).
fn wav_dauer_s(wav: &Path) -> Option<u64> {
    let k = crate::ton::kopf(&std::fs::read(wav).ok()?).ok()?;
    (k.rate > 0).then(|| k.dauer_ms() / 1000)
}

/// **Spielt eine Tondatei ab**, mit dem Abspieler des Systems.
///
/// ⚑ **Fuer die Konsole.** Im Fenster tut das der Webview selbst, und
/// zwar besser: Er kann anhalten, und er braucht kein fremdes Programm.
/// ⚠️ **Fehlt der Abspieler, ist das kein Absturz**, sondern ein Satz,
/// der sagt, welche Datei bereitliegt.
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

    /// ⚑ **Am Anfang einer Antwort darf das erste Stueck am Komma enden**,
    /// danach nicht mehr, und ein zu kurzes Stueck bleibt stehen.
    #[test]
    fn das_erste_stueck_darf_am_komma_enden() {
        let text = "Eine Synapse ist eine Kontaktstelle, an der zwei Nervenzellen sich beruehren, \
                    und dort springt ein Signal ueber. ";
        let (fertig, rest) = satzweise_mit(text, true);
        assert_eq!(fertig[0], "Eine Synapse ist eine Kontaktstelle,");
        assert_eq!(
            fertig[1],
            "an der zwei Nervenzellen sich beruehren, und dort springt ein Signal ueber.",
            "nach dem ersten Stueck gilt wieder das Satzende"
        );
        assert!(rest.is_empty(), "{rest:?}");
        // Ohne den kurzen Anfang bleibt es beim ganzen Satz.
        let (ganz, _) = satzweise_mit(text, false);
        assert_eq!(ganz.len(), 1);
        // Ein Komma nach drei Zeichen beendet nichts.
        let (kurz, rest) = satzweise_mit("Ja, das stimmt so weit", true);
        assert!(kurz.is_empty(), "{kurz:?}");
        assert_eq!(rest, "Ja, das stimmt so weit");
    }

    /// ⛔️ **Markdown geht nicht an die Stimme**, der Text dahinter schon.
    #[test]
    fn markdown_wird_sprechbar() {
        assert_eq!(
            sprechbar("Eine **Synapse** ist der __Verbindungspunkt__ zwischen `Zellen`."),
            "Eine Synapse ist der Verbindungspunkt zwischen Zellen."
        );
        assert_eq!(
            sprechbar("## Aufbau\n- erstens\n2. zweitens"),
            "Aufbau erstens zweitens"
        );
        assert_eq!(
            sprechbar("Mehr unter [Synapse](https://de.wikipedia.org/wiki/Synapse) nachlesen."),
            "Mehr unter Synapse nachlesen."
        );
        assert_eq!(sprechbar("| a | b |"), "a b");
        // Eine Zahl mitten im Satz bleibt, auch mit Punkt.
        assert_eq!(
            sprechbar("Es sind 2. Klasse und 3 Zellen."),
            "Es sind 2. Klasse und 3 Zellen."
        );
    }

    /// 📌 **Eine Abkuerzung beendet kein Stueck.** „(z. B. Glutamat)" war
    /// am 2026-09-25 ein eigenes Stueck „(z.".
    #[test]
    fn abkuerzungen_beenden_kein_stueck() {
        let (fertig, rest) =
            satzweise("Botenstoffe (z. B. Glutamat) wirken bzw. hemmen die Zelle stark. ");
        assert_eq!(
            fertig,
            ["Botenstoffe (z. B. Glutamat) wirken bzw. hemmen die Zelle stark."]
        );
        assert!(rest.is_empty(), "{rest:?}");
        assert!(ist_abkuerzung("etwa ca."));
        assert!(!ist_abkuerzung("Das ist eine Zelle."));
    }

    /// ⚑ **Ein Stueck ohne Worte wird nicht gesprochen**, etwa eine
    /// Trennlinie oder eine leere Aufzaehlung.
    #[test]
    fn ein_stueck_ohne_worte_wird_nicht_geschickt() {
        assert_eq!(zu_sprechen("---"), None);
        assert_eq!(zu_sprechen("**:**"), None);
        assert_eq!(zu_sprechen("- "), None);
        assert_eq!(zu_sprechen("**Ja.**").as_deref(), Some("Ja."));
    }

    /// Ein WAV aus Proben, mit einem `LIST`-Block vor den Daten wie bei ffmpeg.
    fn wav_schreiben(pfad: &Path, rate: u32, proben: &[i16]) {
        let mut b = Vec::new();
        let daten = (proben.len() * 2) as u32;
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(4 + 24 + 12 + 8 + daten).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        for h in [1u16, 1] {
            b.extend_from_slice(&h.to_le_bytes());
        }
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        for h in [2u16, 16] {
            b.extend_from_slice(&h.to_le_bytes());
        }
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(b"INFO");
        b.extend_from_slice(b"data");
        b.extend_from_slice(&daten.to_le_bytes());
        for p in proben {
            b.extend_from_slice(&p.to_le_bytes());
        }
        std::fs::write(pfad, b).unwrap();
    }

    /// ⚑ **Eine lange Probe wird in der Pause geschnitten**, eine kurze
    /// bleibt, wie sie ist.
    #[test]
    fn die_stimmprobe_wird_an_der_pause_gekuerzt() {
        let rate = 24_000u32;
        let je_ms = 24usize;
        // 7 s „Sprache" (ein lauter Saegezahn), mit einer Pause bei 4,2 s
        // und einer zweiten, laengeren bei 6 s ausserhalb des Fensters.
        let proben: Vec<i16> = (0..7_000 * je_ms)
            .map(|i| {
                let ms = i / je_ms;
                if (4_150..4_250).contains(&ms) || (5_800..6_300).contains(&ms) {
                    0
                } else {
                    ((i % 97) as i16 - 48) * 300
                }
            })
            .collect();
        let d = std::env::temp_dir().join(format!("myl-probe-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let lang = d.join("lang.wav");
        wav_schreiben(&lang, rate, &proben);
        assert!(probe_kuerzen(&lang).unwrap());
        let b = std::fs::read(&lang).unwrap();
        let (r, daten) = wav_pcm16_mono(&b).unwrap();
        assert_eq!(r, rate);
        let ms = (daten.end - daten.start) / 2 / je_ms;
        assert!(
            (4_150..4_250).contains(&ms),
            "geschnitten bei {ms} ms statt in der Pause"
        );
        // Und was bleibt, ist der Anfang der Aufnahme, Probe fuer Probe.
        let geblieben: Vec<i16> = b[daten]
            .as_chunks::<2>().0.iter()
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect();
        assert_eq!(geblieben[..], proben[..geblieben.len()]);
        // Ein zweites Mal aendert nichts mehr.
        assert!(!probe_kuerzen(&lang).unwrap());

        let kurz = d.join("kurz.wav");
        wav_schreiben(&kurz, rate, &proben[..5_000 * je_ms]);
        let vorher = std::fs::read(&kurz).unwrap();
        assert!(!probe_kuerzen(&kurz).unwrap());
        assert_eq!(std::fs::read(&kurz).unwrap(), vorher);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// FNV-1a mit 64 Bit, an den Werten der Referenz geprueft.
    #[test]
    fn der_fingerabdruck_ist_fnv1a() {
        assert_eq!(fingerabdruck(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fingerabdruck(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fingerabdruck(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
