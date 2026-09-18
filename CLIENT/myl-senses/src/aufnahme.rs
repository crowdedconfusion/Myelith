//! **Vom Mikrofon aufnehmen**, solange die Taste gehalten wird.
//!
//! # ⚑ Die erste Stufe des Sprachmodus, und was sie nicht ist
//!
//! Das hier ist die **Sprechtaste**: anfangen, reden, aufhoeren. Sie
//! setzt die drei vorhandenen Sinne zusammen (aufnehmen, mitschreiben,
//! antworten, sprechen) und braucht keine neue Fremdkiste.
//!
//! ⚠️ **Ein Voll-Duplex-Gespraech ist sie ausdruecklich nicht.** Dafuer
//! braeuchte es Sprachaktivitaetserkennung, Unterbrechen und
//! Echokompensation, und das ist ein eigenes Vorhaben. Wer diese Datei
//! dazu ausbaut, baut das falsche Ding an der falschen Stelle.
//!
//! # ⛔️ Warum das Aufhoeren ueber die Standardeingabe geht
//!
//! Ein Aufnahmeprogramm muss **sauber abschliessen** duerfen: Eine
//! WAV-Datei traegt ihre Laenge im Kopf, und ein Prozess, den man
//! erschlaegt, hinterlaesst einen Kopf, der luegt. Ein Signal zu
//! schicken braeuchte eine Fremdkiste (`libc`), und
//! [`std::process::Child::kill`] ist SIGKILL.
//!
//! ⚑ **Also ist der Stopp: „deine Eingabe schliesst".** ffmpeg beendet
//! sich darauf, und ein eigenes Skript kann es auch. Erst wenn das nach
//! [`STOPPFRIST_S`] nicht gewirkt hat, wird erschlagen, und dann sagt
//! das Ergebnis es.

use std::path::PathBuf;

use crate::laufwerk::Aufnahmezeug;

/// Wie lange auf ein sauberes Ende gewartet wird, bevor erschlagen wird.
pub const STOPPFRIST_S: u64 = 10;

/// Wie gross ein WAV-Kopf ist, also die Groesse einer Datei ohne Ton.
const WAV_KOPF_BYTES: u64 = 44;

/// Der Schluessel, unter dem ffmpeg den Ausschlag ablegt.
const SCHLUESSEL: &str = "lavfi.astats.Overall.RMS_level";

/// **Der Ausschlag aus einer Zeile von ffmpeg**, als Zahl von 0 bis 1.
///
/// ffmpeg schreibt Dezibel, also eine negative Zahl bis 0; Stille meldet
/// es als `-inf`. ⚑ **Gerechnet wird auf eine Spanne von 60 dB**, weil
/// das der Bereich ist, in dem ein Mikrofon im Zimmer arbeitet: Darunter
/// ist es still, darueber uebersteuert.
fn pegel_aus_zeile(zeile: &str) -> Option<f32> {
    let wert = zeile.trim().strip_prefix(SCHLUESSEL)?.strip_prefix('=')?.trim();
    if wert.contains("inf") {
        return Some(0.0);
    }
    let db: f32 = wert.parse().ok()?;
    Some(((db + 60.0) / 60.0).clamp(0.0, 1.0))
}

/// **Wie lange eine einzelne Aufnahme hoechstens dauert.**
///
/// ⚑ **Eine vergessene Aufnahme laeuft sonst, bis die Platte voll ist.**
/// Fuenf Minuten sind mehr, als jemand am Stueck in ein Mikrofon
/// spricht, und wenig genug, dass ein Versehen nichts kostet.
pub const HOECHSTDAUER_S: u64 = 300;

/// Eine laufende Aufnahme.
///
/// ⚠️ **Wer sie fallen laesst, ohne sie zu beenden, laesst ein Programm
/// laufen.** Deshalb raeumt [`Drop`] auf, aber die WAV-Datei ist dann
/// unbrauchbar: Der gewollte Weg ist [`Aufnahme::beenden`].
#[derive(Debug)]
pub struct Aufnahme {
    kind: Option<std::process::Child>,
    ziel: PathBuf,
    /// Was das Aufnahmeprogramm an Meldungen hinterlassen hat.
    ///
    /// ⛔️ **Ohne das raet die Fehlermeldung.** Eine leere Aufnahme hat
    /// einen Grund, und ffmpeg schreibt ihn hin; ihn abzufangen und
    /// nicht zu lesen war schlimmer, als ihn gar nicht erst abzufangen.
    /// Gemeldet vom Projektinhaber am 2026-09-18, der „nimmt das
    /// Aufnahmeprogramm das richtige Geraet?" zu sehen bekam, waehrend
    /// die Antwort danebenlag.
    meldungen: Option<std::sync::mpsc::Receiver<String>>,
    /// Der Ausschlag, Bild fuer Bild, solange aufgenommen wird.
    ///
    /// ⚑ **Damit sieht der Mensch, dass wirklich etwas ankommt.** Ein
    /// Mikrofon, das stummgeschaltet ist oder auf das falsche Geraet
    /// zeigt, sieht sonst genauso aus wie eines, das zuhoert; der
    /// Unterschied faellt erst auf, wenn nichts mitgeschrieben wurde.
    pegel: Option<std::sync::mpsc::Receiver<f32>>,
}

/// **Faengt an aufzunehmen.** Die Datei entsteht erst beim Beenden
/// vollstaendig.
pub fn starten(zeug: &Aufnahmezeug) -> Result<Aufnahme, String> {
    let ziel = crate::zwischenname("myl-aufnahme", "wav");
    let mut befehl = std::process::Command::new(&zeug.programm);
    match &zeug.quelle {
        // ffmpeg: Format und Geraet, dann 16 kHz und ein Kanal, so wie
        // whisper.cpp es haben will.
        Some((format, geraet)) => {
            befehl
                .arg("-loglevel")
                .arg("error")
                .arg("-y")
                .arg("-f")
                .arg(format)
                .arg("-i")
                .arg(geraet)
                .arg("-t")
                .arg(HOECHSTDAUER_S.to_string())
                .arg("-ar")
                .arg("16000")
                .arg("-ac")
                .arg("1")
                .arg("-c:a")
                .arg("pcm_s16le")
                // ⚑ **Der Ausschlag kommt aus demselben Lauf.** Ein
                // zweiter Zugriff auf das Mikrofon, nur um einen Balken
                // zu zeichnen, waere ein zweiter Verbraucher desselben
                // Geraets und auf manchen Systemen ein Fehlschlag.
                // `astats` laesst den Ton unveraendert durch und legt
                // seine Zahlen nebenbei auf die Standardausgabe.
                .arg("-af")
                .arg(format!(
                    "astats=metadata=1:reset=1,ametadata=print:key={SCHLUESSEL}:file=-"
                ))
                .arg(&ziel);
        }
        // Eigenes Skript: ein Pfad, sonst nichts. Siehe Modulkopf.
        None => {
            befehl.arg(&ziel);
        }
    }
    let mut kind = befehl
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|f| format!("die Aufnahme liess sich nicht starten ({}): {f}", zeug.programm.display()))?;

    // ⚑ **Und ein Faden liest die Meldungen mit**, aus demselben Grund
    // und damit der Fehlerfall etwas zu sagen hat.
    let meldungen = kind.stderr.take().map(|fehler| {
        let (sender, empfang) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for zeile in std::io::BufReader::new(fehler).lines().map_while(Result::ok) {
                if zeile.trim().is_empty() {
                    continue;
                }
                if sender.send(zeile).is_err() {
                    break;
                }
            }
        });
        empfang
    });

    // ⚑ **Ein Faden liest die Zahlen mit**, damit die Roehre nicht
    // volllaeuft und den Aufnehmer anhaelt.
    let pegel = kind.stdout.take().map(|aus| {
        let (sender, empfang) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for zeile in std::io::BufReader::new(aus).lines().map_while(Result::ok) {
                if let Some(wert) = pegel_aus_zeile(&zeile) {
                    if sender.send(wert).is_err() {
                        break;
                    }
                }
            }
        });
        empfang
    });
    Ok(Aufnahme { kind: Some(kind), ziel, pegel, meldungen })
}

impl Aufnahme {
    /// Wohin aufgenommen wird.
    pub fn ziel(&self) -> &std::path::Path {
        &self.ziel
    }

    /// **Nimmt den Ausschlag heraus**, einmal.
    ///
    /// ⚑ **Herausgenommen und nicht geliehen**: Wer ihn hat, liest ihn
    /// in einem eigenen Faden leer, und zwei Leser desselben Kanals
    /// bekaemen jeder die halben Zahlen.
    pub fn pegel_nehmen(&mut self) -> Option<std::sync::mpsc::Receiver<f32>> {
        self.pegel.take()
    }

    /// **Hoert auf und gibt die fertige Datei zurueck.**
    ///
    /// ⚠️ **Der Aufrufer raeumt sie weg**, wenn er sie mitgeschrieben hat.
    pub fn beenden(mut self) -> Result<PathBuf, String> {
        let mut kind = self.kind.take().ok_or("die Aufnahme lief nicht")?;

        // ⚑ Erst das Schliessen, dann das Warten. Ein `q` dazu, weil
        // ffmpeg darauf ebenfalls hoert; ein Skript, das nur auf das
        // Ende der Eingabe achtet, stoert es nicht.
        if let Some(mut ein) = kind.stdin.take() {
            use std::io::Write;
            let _ = ein.write_all(b"q\n");
            let _ = ein.flush();
        }

        let frist = std::time::Duration::from_secs(STOPPFRIST_S);
        let anfang = std::time::Instant::now();
        let mut erschlagen = false;
        loop {
            match kind.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if anfang.elapsed() >= frist {
                        let _ = kind.kill();
                        let _ = kind.wait();
                        erschlagen = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(f) => return Err(format!("die Aufnahme liess sich nicht beenden: {f}")),
            }
        }

        // ⚑ **Jetzt sagt das Aufnahmeprogramm selbst, was los war.**
        let gesagt = self.meldungen.take().map(|e| e.into_iter().collect::<Vec<_>>()).unwrap_or_default();
        let gross = std::fs::metadata(&self.ziel).map(|m| m.len()).unwrap_or(0);
        // ⚠️ **Ein WAV-Kopf allein ist keine Aufnahme.** ffmpeg legt die
        // Datei an, bevor der erste Ton kommt; wer nur auf „groesser
        // null" prueft, haelt 44 Bytes Kopf fuer eine Aufnahme und
        // schickt sie an das Hoermodell.
        if gross <= WAV_KOPF_BYTES {
            let _ = std::fs::remove_file(&self.ziel);
            let mut grund = if erschlagen {
                format!("das Aufnahmeprogramm hoerte nach {STOPPFRIST_S} s nicht auf und hat nichts hinterlassen")
            } else {
                "die Aufnahme ist leer geblieben".to_string()
            };
            if gesagt.is_empty() {
                grund.push_str("; nimmt das Aufnahmeprogramm das richtige Geraet?");
            } else {
                grund.push_str(". Das Aufnahmeprogramm sagt dazu:\n");
                let ab = gesagt.len().saturating_sub(8);
                grund.push_str(&gesagt[ab..].join("\n"));
            }
            return Err(grund);
        }
        if erschlagen {
            // ⚠️ **Gesagt, nicht verschwiegen.** Eine erschlagene
            // Aufnahme kann einen Kopf tragen, der luegt, und dann
            // verhoert sich das Hoermodell auf eine Weise, die niemand
            // erklaeren kann.
            return Err(format!(
                "das Aufnahmeprogramm hoerte nach {STOPPFRIST_S} s nicht auf und wurde erschlagen; \
                 die Datei {} kann unvollstaendig sein",
                self.ziel.display()
            ));
        }
        Ok(self.ziel.clone())
    }
}

impl Drop for Aufnahme {
    fn drop(&mut self) {
        if let Some(mut kind) = self.kind.take() {
            let _ = kind.kill();
            let _ = kind.wait();
            let _ = std::fs::remove_file(&self.ziel);
        }
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Die Zahlen von ffmpeg werden gelesen, wie sie kommen.**
    ///
    /// ⚠️ Stille meldet ffmpeg als `-inf`, und `parse` macht daraus
    /// nichts; ohne den eigenen Zweig bliebe der Ausschlag bei Stille
    /// **stehen**, statt auf null zu gehen, und das saehe aus wie ein
    /// Mikrofon, das noch hoert.
    #[test]
    fn der_ausschlag_kommt_aus_der_zeile() {
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.RMS_level=-inf"), Some(0.0));
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.RMS_level=0.000000"), Some(1.0));
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.RMS_level=-30.0"), Some(0.5));
        // Unter der Spanne wird nicht negativ, darueber nicht groesser eins.
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.RMS_level=-120.0"), Some(0.0));
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.RMS_level=12.0"), Some(1.0));
        // Fremde Zeilen sind keine Zahlen.
        assert_eq!(pegel_aus_zeile("frame:12 pts:1024 pts_time:0.064"), None);
        assert_eq!(pegel_aus_zeile("lavfi.astats.Overall.Peak_level=-3.0"), None);

        // ⚑ **Eine Zeile, wie ffmpeg sie wirklich schreibt** (am
        // 2026-09-18 an einer echten Aufnahme abgenommen, rund sechzehn
        // Werte je Sekunde). Eine Probe gegen ein ausgedachtes Format
        // prueft das Ausgedachte.
        let echt = pegel_aus_zeile("lavfi.astats.Overall.RMS_level=-15.716330").expect("Zahl");
        assert!((echt - 0.738).abs() < 0.01, "{echt}");
    }
}
