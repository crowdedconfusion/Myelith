//! **Ein fremdes Programm starten, mit Frist und Ausgabegrenze.**
//!
//! # ⚑ Eine Stelle fuer alle, die das brauchen
//!
//! Bis zum 2026-09-17 stand diese Schleife einmal in `myl-client`
//! (`befehl_im_verzeichnis`) und haette hier ein zweites Mal entstehen
//! muessen. **Zwei Laeufer mit zwei Fristen** sind genau die
//! Fehlerklasse, die dieses Projekt am haeufigsten trifft: Was an zwei
//! Orten steht, laeuft auseinander, und der zweite meldet sich nicht.
//! Also steht der Ablauf hier, und beide rufen ihn.
//!
//! ⚑ **Zwei Faeden leeren die Roehren.** Ein Programm mit viel Ausgabe
//! blockiert sonst, wenn der Roehrenpuffer voll ist, und wartet auf
//! einen Leser, der auf sein Ende wartet. Jeder Faden liest **bis zum
//! Ende** und behaelt nur bis zur Grenze; dass es mehr gab, wird
//! vermerkt statt vergessen.

use std::process::Command;

/// Wie lange nach dem Ende des Kindes noch auf seine Roehren gewartet
/// wird, in Sekunden.
///
/// # ⛔️ Warum hier ueberhaupt eine Frist steht
///
/// **Ein Enkel kann das Ende der Roehre halten, nachdem das Kind weg
/// ist.** `sh -c "sleep 5"` ist auf macOS ein Prozess, unter Linux und
/// Windows zwei: Die Shell startet `sleep` als Kind, und wer die Shell
/// erschlaegt, laesst `sleep` weiterlaufen. Ein blockierendes Lesen
/// wartet dann auf dessen Ende statt auf die Frist, und die Zusage
/// „dieser Aufruf kommt nach `frist_s` zurueck" ist gebrochen.
///
/// 📌 **Gefunden von der CI am 2026-09-18**, auf Linux und Windows;
/// hier lief es durch, weil diese Shell sich selbst ersetzt.
///
/// ⚑ **Zwei Sekunden**, denn eine Roehre, die nach dem Ende des Kindes
/// noch traegt, traegt etwas, das niemand mehr angefordert hat.
pub const ROEHRENFRIST_S: u64 = 2;

/// Was ein Lauf hinterlassen hat.
#[derive(Debug, Clone)]
pub struct Ausgang {
    /// Was nach stdout ging. Bei einem Sinnesprogramm ist das die Antwort.
    pub aus: String,
    /// Was nach stderr ging. Bei llama.cpp und whisper.cpp das Ladegeschwaetz.
    pub fehler: String,
    /// Ob eine der beiden Roehren mehr geliefert hat, als behalten wurde.
    pub mehr: bool,
    /// Der Rueckgabewert, falls es einen gab.
    pub kode: Option<i32>,
    /// Ob die Frist abgelaufen ist und der Lauf abgebrochen wurde.
    pub abgebrochen: bool,
    /// Die Frist, die galt, fuer die Meldung.
    pub frist_s: u64,
}

impl Ausgang {
    /// Ob der Lauf sauber durchgegangen ist.
    pub fn gut(&self) -> bool {
        !self.abgebrochen && self.kode == Some(0)
    }

    /// Die erste Zeile einer Meldung: wie der Lauf ausgegangen ist.
    pub fn kopf(&self) -> String {
        if self.abgebrochen {
            return format!("abgebrochen nach {} s", self.frist_s);
        }
        match self.kode {
            Some(0) => "beendet, Rueckgabewert 0".to_string(),
            Some(c) => format!("beendet, Rueckgabewert {c}"),
            None => "beendet ohne Rueckgabewert".to_string(),
        }
    }

    /// **Beide Roehren hintereinander, auf `grenze` Zeichen gekuerzt.**
    ///
    /// ⚑ **Gekuerzt wird in Zeichen und nicht in Bytes**: Ein Schnitt
    /// mitten durch eine Mehrbytefolge erzeugte sonst ein Ersatzzeichen.
    /// Zurueck kommt der Text und ob gekuerzt wurde.
    pub fn zusammen(&self, grenze: usize) -> (String, bool) {
        let ganz = format!("{}{}", self.aus, self.fehler);
        let gekuerzt = self.mehr || ganz.chars().count() > grenze;
        (ganz.chars().take(grenze).collect(), gekuerzt)
    }
}

/// **Startet den Befehl, wartet hoechstens `frist_s` Sekunden und gibt
/// zurueck, was herauskam.**
///
/// Der Aufrufer stellt das [`Command`] fertig ein (Programm, Argumente,
/// Arbeitsverzeichnis, Umgebung); hier werden nur die Roehren gesetzt.
///
/// ⚠️ **`stdin` wird auf `null` gelegt.** Ein Programm, das eine Eingabe
/// erwartet, haengt sonst bis zur Frist, und niemand saehe, warum.
pub fn laufen(befehl: &mut Command, frist_s: u64, grenze: usize) -> Result<Ausgang, String> {
    laufen_mit_eingabe(befehl, std::process::Stdio::null(), frist_s, grenze)
}

/// **Derselbe Lauf, aber mit etwas an `stdin`.**
///
/// ⚑ **Gebraucht von den Sprechprogrammen**, die ihren Text von der
/// Standardeingabe nehmen (piper tut das). Eine Datei statt eines
/// Arguments hat zwei Vorteile: kein Laengenlimit der Kommandozeile und
/// kein Zeichen, das irgendwo zitiert werden muesste.
pub fn laufen_mit_eingabe(
    befehl: &mut Command,
    eingabe: std::process::Stdio,
    frist_s: u64,
    grenze: usize,
) -> Result<Ausgang, String> {
    use std::io::Read;
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let mut kind = befehl
        .stdin(eingabe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|f| format!("liess sich nicht starten: {f}"))?;

    // ⛔️ **Welches Ende behalten wird, haengt an der Roehre** (Fund 433,
    // 2026-09-22).
    //
    // Auf **stdout** steht die Antwort des Sinnesprogramms; wer sie
    // kuerzt, behaelt ihren **Anfang**, denn das Wichtigste steht vorn.
    //
    // Auf **stderr** steht das Protokoll, und dort ist es umgekehrt: Der
    // Anfang ist Geraetekunde, der **Grund eines Abbruchs steht am
    // Ende**. Bis hierher behielten beide Roehren den Anfang, und
    // `sehen.rs` meldete unter der Ueberschrift „Die letzten Zeilen"
    // die letzten Zeilen der **ersten** acht Kilobyte. Bei einem Bild
    // mit vielen Bloecken sind das die Fortschrittszeilen der ersten
    // Sekunden; **die Ursache wurde verworfen, bevor sie entstand.**
    //
    // 📌 **Die Absicht stand die ganze Zeit im Quelltext** („Der Grund
    // steht bei llama.cpp am Ende") und die Umsetzung widersprach ihr.
    // Ein Kommentar, der eine Zusage beschreibt, die der Code nicht
    // haelt, ist schlimmer als keiner: Er beruhigt den naechsten Leser.
    let lesen = |mut strom: Box<dyn Read + Send>, behalte_ende: bool| {
        let (sender, empfang) = mpsc::channel();
        std::thread::spawn(move || {
            let mut puffer: Vec<u8> = Vec::new();
            let mut mehr = false;
            let mut haeppchen = [0u8; 8192];
            loop {
                match strom.read(&mut haeppchen) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if behalte_ende {
                            puffer.extend_from_slice(&haeppchen[..n]);
                            if puffer.len() > grenze {
                                mehr = true;
                                let weg = puffer.len() - grenze;
                                puffer.drain(..weg);
                                // ⚑ **Bis zum naechsten Zeichenanfang
                                //   weiter**, sonst beginnt der Text mit
                                //   einer halben Mehrbytefolge und damit
                                //   mit einem Ersatzzeichen.
                                while puffer.first().is_some_and(|b| b & 0xC0 == 0x80) {
                                    puffer.remove(0);
                                }
                            }
                        } else {
                            let frei = grenze.saturating_sub(puffer.len());
                            if frei > 0 {
                                puffer.extend_from_slice(&haeppchen[..n.min(frei)]);
                            }
                            if n > frei {
                                mehr = true;
                            }
                        }
                    }
                }
            }
            let _ = sender.send((puffer, mehr));
        });
        empfang
    };
    let aus_e = lesen(Box::new(kind.stdout.take().expect("stdout")), false);
    let err_e = lesen(Box::new(kind.stderr.take().expect("stderr")), true);

    // Warten mit Frist: `try_wait` blockiert nicht, also bleibt der
    // Abbruch moeglich.
    let frist = Duration::from_secs(frist_s);
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

    // ⛔️ **Mit Frist lesen und nicht blockierend.** Siehe
    // [`ROEHRENFRIST_S`]: Ein Enkel kann die Roehre halten, nachdem das
    // Kind weg ist, und dann waere die Frist oben wirkungslos.
    // ⚠️ **Was dabei verlorengeht, ist Ausgabe und nicht Richtigkeit**:
    // Der Kopf sagt weiterhin, wie der Lauf ausging.
    // ⚠️ **Eine Frist fuer beide, nicht eine je Roehre.** Zweimal
    // nacheinander zu warten verdoppelt sie, und dann kommt der Aufruf
    // nach `frist_s + 2 * ROEHRENFRIST_S` zurueck statt nach
    // `frist_s + ROEHRENFRIST_S`.
    let bis = Instant::now() + Duration::from_secs(ROEHRENFRIST_S);
    let rest = || bis.saturating_duration_since(Instant::now());
    let (aus_roh, aus_mehr) = aus_e.recv_timeout(rest()).unwrap_or_default();
    let (err_roh, err_mehr) = err_e.recv_timeout(rest()).unwrap_or_default();
    Ok(Ausgang {
        aus: String::from_utf8_lossy(&aus_roh).into_owned(),
        fehler: String::from_utf8_lossy(&err_roh).into_owned(),
        mehr: aus_mehr || err_mehr,
        kode: status.and_then(|s| s.code()),
        abgebrochen,
        frist_s,
    })
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn die_beiden_roehren_bleiben_getrennt() {
        let mut b = Command::new("/bin/sh");
        b.arg("-c").arg("echo hierher; echo dorthin >&2");
        let a = laufen(&mut b, 5, 1024).expect("laeuft");
        assert!(a.gut(), "{a:?}");
        assert_eq!(a.aus.trim(), "hierher");
        assert_eq!(a.fehler.trim(), "dorthin");
        assert_eq!(a.kopf(), "beendet, Rueckgabewert 0");
    }

    /// ⛔️ **Die Ursache steht am Ende des Protokolls, also wird das
    /// Ende behalten** (Fund 433).
    ///
    /// Ein Sehmodell schreibt je Bildblock eine Fortschrittszeile; bei
    /// einem grossen Bild sind das Hunderte. Behielte `stderr` den
    /// Anfang, stuende in der Fehlermeldung der Fortschritt der ersten
    /// Sekunden und **nie der Grund**.
    #[test]
    fn stderr_behaelt_das_ende_und_nicht_den_anfang() {
        let mut b = Command::new("/bin/sh");
        // Viel Geschwaetz, danach die eine Zeile, auf die es ankommt.
        b.arg("-c")
            .arg("i=0; while [ $i -lt 400 ]; do echo \"Fortschritt $i\" >&2; i=$((i+1)); done; \
                  echo DIES_IST_DER_GRUND >&2; exit 1");
        let a = laufen(&mut b, 10, 512).expect("laeuft");
        assert!(!a.gut(), "{a:?}");
        assert_eq!(a.kopf(), "beendet, Rueckgabewert 1");
        assert!(a.mehr, "es war mehr da, als behalten wurde");
        assert!(
            a.fehler.contains("DIES_IST_DER_GRUND"),
            "der Grund fehlt, behalten wurde:\n{}",
            a.fehler
        );
        assert!(
            !a.fehler.contains("Fortschritt 0\n"),
            "der Anfang steht noch da, also wurde das falsche Ende behalten"
        );
    }

    /// ⚑ **Die Gegenrichtung, und sie ist genauso wichtig.** Auf stdout
    /// steht die **Antwort**; dort ist der Anfang das Wichtige. Wer
    /// beide Roehren gleich behandelte, verloere je nach Richtung
    /// entweder den Grund oder den Anfang der Antwort.
    #[test]
    fn stdout_behaelt_den_anfang_und_nicht_das_ende() {
        let mut b = Command::new("/bin/sh");
        b.arg("-c")
            .arg("echo SO_FAENGT_DIE_ANTWORT_AN; \
                  i=0; while [ $i -lt 400 ]; do echo \"Fuellung $i\"; i=$((i+1)); done");
        let a = laufen(&mut b, 10, 512).expect("laeuft");
        assert!(a.gut(), "{a:?}");
        assert!(a.mehr, "es war mehr da, als behalten wurde");
        assert!(
            a.aus.starts_with("SO_FAENGT_DIE_ANTWORT_AN"),
            "der Anfang der Antwort fehlt, behalten wurde:\n{}",
            a.aus
        );
    }

    /// ⚠️ **Ein Schnitt am Ende darf keine halbe Mehrbytefolge
    /// hinterlassen.** Der Puffer wird von vorn gekuerzt, und dort kann
    /// ein Zeichen mitten durchgehen.
    #[test]
    fn das_gekuerzte_ende_beginnt_an_einer_zeichengrenze() {
        let mut b = Command::new("/bin/sh");
        // Lauter Dreibytezeichen, damit ein Schnitt sehr wahrscheinlich
        // mitten in eines faellt.
        b.arg("-c").arg("i=0; while [ $i -lt 600 ]; do printf '\\u2713' >&2; i=$((i+1)); done; exit 1");
        let a = laufen(&mut b, 10, 512).expect("laeuft");
        assert!(a.mehr, "es war mehr da, als behalten wurde");
        assert!(
            !a.fehler.starts_with('\u{fffd}'),
            "der Text beginnt mit einem Ersatzzeichen: {:?}",
            a.fehler.chars().take(4).collect::<String>()
        );
    }

    /// ⚑ **Die Frist greift, und sie steht in der Meldung.**
    #[test]
    fn ein_haengendes_programm_wird_abgebrochen() {
        let mut b = Command::new("/bin/sh");
        b.arg("-c").arg("sleep 5");
        let anfang = std::time::Instant::now();
        let a = laufen(&mut b, 1, 1024).expect("laeuft");
        assert!(a.abgebrochen, "{a:?}");
        assert!(!a.gut());
        assert_eq!(a.kopf(), "abgebrochen nach 1 s");
        assert!(anfang.elapsed().as_secs() < 4, "die Frist hat nicht gegriffen");
    }

    /// ⛔️ **Viel Ausgabe blockiert nicht**, und dass gekuerzt wurde,
    /// bleibt sichtbar.
    #[test]
    fn viel_ausgabe_wird_gekuerzt_und_sagt_es() {
        let mut b = Command::new("/bin/sh");
        b.arg("-c").arg("i=0; while [ $i -lt 2000 ]; do echo abcdefghij; i=$((i+1)); done");
        let a = laufen(&mut b, 20, 256).expect("laeuft");
        assert!(a.gut(), "{a:?}");
        let (text, gekuerzt) = a.zusammen(256);
        assert!(gekuerzt, "die Kuerzung sagt sich nicht an");
        assert_eq!(text.chars().count(), 256);
    }

    /// ⛔️ **Ein Enkel, der die Roehre haelt, haelt den Aufruf nicht auf.**
    ///
    /// # 📌 Der Fall, an dem die CI umfiel (2026-09-18)
    ///
    /// `sh -c "sleep 5"` ist auf macOS derselbe Prozess (die Shell
    /// ersetzt sich selbst), unter Linux und Windows aber **zwei**: Die
    /// Shell startet `sleep` als Kind. Wird die Shell nach der Frist
    /// erschlagen, laeuft `sleep` weiter und **haelt das Ende der
    /// Roehre**; ein blockierendes Lesen wartet dann auf sein Ende
    /// statt auf die Frist. Die Meldung sagte „abgebrochen nach 1 s",
    /// und der Aufruf kam nach fuenf zurueck.
    ///
    /// ⚑ **Hier wird das nachgestellt**, und zwar auf jedem System:
    /// Die Shell schickt `sleep` in den Hintergrund und endet sofort.
    #[test]
    fn ein_enkel_an_der_roehre_haelt_den_aufruf_nicht_auf() {
        let mut b = Command::new("/bin/sh");
        // Die Shell ist sofort fertig, das Enkelkind haelt die Roehre.
        b.arg("-c").arg("sleep 5 &");
        let anfang = std::time::Instant::now();
        let a = laufen(&mut b, 10, 1024).expect("laeuft");
        assert!(a.gut(), "{a:?}");
        assert!(
            anfang.elapsed().as_secs() < 4,
            "der Aufruf hing an einem Enkel: {:?}",
            anfang.elapsed()
        );
    }

    /// ⚑ **Ein Rueckgabewert ungleich null ist kein Fehler dieser
    /// Funktion.** Sie meldet ihn, statt ihn zu verschlucken.
    #[test]
    fn ein_schlechter_rueckgabewert_kommt_durch() {
        let mut b = Command::new("/bin/sh");
        b.arg("-c").arg("exit 3");
        let a = laufen(&mut b, 5, 64).expect("laeuft");
        assert_eq!(a.kode, Some(3));
        assert!(!a.gut());
        assert_eq!(a.kopf(), "beendet, Rueckgabewert 3");
    }

    /// ⛔️ **Ein Programm, das es nicht gibt, ist ein Fehler und kein
    /// leerer Ausgang.**
    #[test]
    fn ein_fehlendes_programm_meldet_sich() {
        let mut b = Command::new("/gibt/es/nicht/xyz");
        assert!(laufen(&mut b, 5, 64).is_err());
    }
}
