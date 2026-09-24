//! **Ein Einzelbild holen**, vom Bildschirm oder aus der Kamera.
//!
//! # ⚑ Warum ein Einzelbild und kein Strom
//!
//! Die naheliegende Bauform waere ein laufender Strom, den ein Sehmodell
//! fortwaehrend auswertet. **Sie ist falsch, und zwar gemessen:**
//!
//! - Ein Sehdurchgang kostet bei 1024 Bildtoken rund **drei Sekunden**.
//! - Das grosse Sprachmodell wartet auf der Maschine ohnehin auf die
//!   Platte; ein zweites Modell, das dauernd rechnet, nimmt ihm genau
//!   das weg, worauf es wartet.
//! - ⛔️ **Vor allem aber entscheidet die Frage ueber die Antwort.** Auf
//!   „wie viele blaue Kreise" kam exakt die Zahl; eine allgemeine
//!   Beschreibung desselben Bildes hat sie nie enthalten. Ein Strom
//!   erzeugt Beschreibungen, nach denen niemand gefragt hat.
//!
//! ⚑ **Also: Es wird geholt, wenn jemand etwas wissen will**, und die
//! Frage geht mit. Was hier entsteht, ist eine Datei; was daraus wird,
//! macht [`crate::sehen`].
//!
//! # ⚠️ Was hier NICHT entschieden wird
//!
//! **Ob ueberhaupt geblickt werden darf.** Das ist eine Befugnis und
//! keine Geraetefrage; sie steht im Client. Dieses Modul nimmt auf, wenn
//! es gerufen wird.

use std::path::Path;

use crate::laufwerk::{Bildschirmzeug, Kamerazeug};

/// Wie lange eine Aufnahme hoechstens dauern darf.
///
/// ⚑ **Kurz, denn hier wird nichts gerechnet.** Ein Bildschirmfoto ist
/// in Sekundenbruchteilen da; wer laenger braucht, wartet auf eine
/// Erlaubnis, die nie kommt, und genau das soll auffallen statt zu
/// haengen.
pub const FRIST_BLICK_S: u64 = 20;

/// **Wie viele Bilder die Kamera verwirft, bevor eines behalten wird.**
///
/// ⛔️ **Das erste Kamerabild ist dunkel.** Eine Kamera stellt Belichtung
/// und Weissabgleich erst beim Laufen ein; wer sofort das erste Bild
/// nimmt, bekommt ein schwarzes oder gruenstichiges. ffmpeg schreibt mit
/// `-update 1` jedes Bild in dieselbe Datei, also bleibt am Ende das
/// letzte stehen.
///
/// ⚠️ **Acht ist eine Vorgabe und keine Messung.** Sie ist gewaehlt,
/// weil sie bei 30 Bildern je Sekunde gut eine Viertelsekunde kostet;
/// wessen Kamera laenger braucht, setzt `MYL_KAMERA_VORLAUF` hoeher.
pub const KAMERA_VORLAUF: u32 = 8;

/// **Der Aufruf fuer ein Bildschirmfoto**, getrennt vom Lauf.
///
/// ⚑ **Unterschieden wird an der Quelle, nicht am Programmnamen.** Traegt
/// das Zeug keine Quelle, kennt das Programm den Bildschirm selbst
/// (`screencapture`); traegt es eine, ist es ffmpeg. Wer stattdessen den
/// Dateinamen pruefte, pruefte eine Schreibweise, und ein eigenes Skript
/// heisst anders.
pub fn befehl_bildschirm(zeug: &Bildschirmzeug, ziel: &Path) -> std::process::Command {
    let mut b = std::process::Command::new(&zeug.programm);
    match &zeug.quelle {
        None => {
            // `-x` laesst den Auslöserton weg: Ein Agent, der hinsieht,
            // soll nicht klingen wie eine Kamera.
            b.arg("-x").arg("-t").arg("png").arg(ziel);
        }
        Some((format, geraet)) => {
            b.arg("-y")
                .arg("-loglevel")
                .arg("error")
                .arg("-f")
                .arg(format)
                .arg("-i")
                .arg(geraet)
                .arg("-frames:v")
                .arg("1")
                .arg(ziel);
        }
    }
    b
}

/// **Der Aufruf fuer ein Kamerabild**, getrennt vom Lauf.
pub fn befehl_kamera(zeug: &Kamerazeug, ziel: &Path) -> std::process::Command {
    let (format, geraet) = &zeug.quelle;
    let vorlauf = crate::zahl_aus_umgebung("MYL_KAMERA_VORLAUF", KAMERA_VORLAUF).max(1);
    let mut b = std::process::Command::new(&zeug.programm);
    b.arg("-y")
        .arg("-loglevel")
        .arg("error")
        .arg("-f")
        .arg(format)
        .arg("-i")
        .arg(geraet)
        .arg("-frames:v")
        .arg(vorlauf.to_string())
        // ⚑ **Jedes Bild in dieselbe Datei**, siehe [`KAMERA_VORLAUF`]:
        //   Am Ende steht das letzte da, und das ist das belichtete.
        .arg("-update")
        .arg("1")
        .arg(ziel);
    b
}

/// **Holt ein Bildschirmfoto nach `ziel`.**
pub fn bildschirm(zeug: &Bildschirmzeug, ziel: &Path) -> Result<(), String> {
    let mut b = befehl_bildschirm(zeug, ziel);
    lauf_und_pruefen(&mut b, ziel, &zeug.programm.display().to_string(), "Der Bildschirm")
}

/// **Holt ein Kamerabild nach `ziel`.**
pub fn kamera(zeug: &Kamerazeug, ziel: &Path) -> Result<(), String> {
    let mut b = befehl_kamera(zeug, ziel);
    lauf_und_pruefen(&mut b, ziel, &zeug.programm.display().to_string(), "Die Kamera")
}

/// ⛔️ **Geprueft wird der Rueckgabewert UND die Datei.**
///
/// Die Zusage dieser Funktion lautet „danach liegt ein Bild da", und
/// genau das wird nachgesehen. Ein Rueckgabewert von null sagt nur, dass
/// das Programm sich nicht beschwert hat; er sagt nichts darueber, ob
/// etwas entstanden ist.
///
/// 📌 **Beobachtet am 2026-09-23:** Ohne die Freigabe zur
/// Bildschirmaufnahme meldet `screencapture` auf dieser Maschine
/// `could not create image from display` und endet mit **eins**, faellt
/// hier also schon am Rueckgabewert. **Die Dateipruefung ist trotzdem
/// kein Beiwerk:** Sie kostet nichts und traegt die Zusage auch dort, wo
/// ein Programm anders ausgeht, als man es heute gesehen hat.
fn lauf_und_pruefen(
    befehl: &mut std::process::Command,
    ziel: &Path,
    programm: &str,
    was: &str,
) -> Result<(), String> {
    // Ein alter Stand darf nicht als neuer durchgehen.
    let _ = std::fs::remove_file(ziel);
    let a = crate::prozess::laufen(befehl, FRIST_BLICK_S, crate::AUSGABEGRENZE)
        .map_err(|f| format!("{was} liess sich nicht aufnehmen ({programm}): {f}"))?;
    if !a.gut() {
        return Err(format!(
            "{was} liess sich nicht aufnehmen, {programm} ist {}. Die letzten Zeilen:\n{}",
            a.kopf(),
            crate::schwanz(&a.fehler, 10)
        ));
    }
    match std::fs::metadata(ziel) {
        Ok(m) if m.len() > 0 => Ok(()),
        _ => Err(format!(
            "{was} hat kein Bild hinterlassen. Auf macOS fehlt dafuer meist die Erlaubnis: \
             Systemeinstellungen, Datenschutz, Bildschirmaufnahme beziehungsweise Kamera."
        )),
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use std::path::PathBuf;

    fn argumente(b: &std::process::Command) -> Vec<String> {
        b.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    /// ⚑ **Ohne Quelle kennt das Programm den Bildschirm selbst.**
    #[test]
    fn ohne_quelle_wird_screencapture_gerufen() {
        let z = Bildschirmzeug { programm: PathBuf::from("/usr/sbin/screencapture"), quelle: None };
        let a = argumente(&befehl_bildschirm(&z, Path::new("/tmp/x.png")));
        assert!(a.contains(&"-x".to_string()), "{a:?}");
        assert!(!a.iter().any(|x| x == "-f"), "eine Quelle steht im Aufruf: {a:?}");
        assert_eq!(a.last().unwrap(), "/tmp/x.png");
    }

    /// ⚑ **Mit Quelle ist es ffmpeg**, und dann gehoert genau ein Bild
    /// geholt. Ohne `-frames:v 1` nimmt ffmpeg auf, bis jemand es
    /// beendet, und die Frist waere die einzige Bremse.
    #[test]
    fn mit_quelle_wird_genau_ein_bild_geholt() {
        let z = Bildschirmzeug {
            programm: PathBuf::from("/usr/bin/ffmpeg"),
            quelle: Some(("x11grab".into(), ":0.0".into())),
        };
        let a = argumente(&befehl_bildschirm(&z, Path::new("/tmp/x.png")));
        let i = a.iter().position(|x| x == "-f").expect("kein Format im Aufruf");
        assert_eq!(a[i + 1], "x11grab");
        let j = a.iter().position(|x| x == "-frames:v").expect("keine Bildzahl im Aufruf");
        assert_eq!(a[j + 1], "1");
    }

    /// ⛔️ **Die Kamera bekommt einen Vorlauf**, sonst ist das Bild
    /// dunkel. Siehe [`KAMERA_VORLAUF`].
    #[test]
    fn die_kamera_verwirft_die_ersten_bilder() {
        let z = Kamerazeug {
            programm: PathBuf::from("/usr/bin/ffmpeg"),
            quelle: ("avfoundation".into(), "0".into()),
        };
        let a = argumente(&befehl_kamera(&z, Path::new("/tmp/k.png")));
        let j = a.iter().position(|x| x == "-frames:v").expect("keine Bildzahl im Aufruf");
        assert!(
            a[j + 1].parse::<u32>().expect("Zahl") > 1,
            "ohne Vorlauf ist das erste Bild dunkel: {a:?}"
        );
        assert!(a.iter().any(|x| x == "-update"), "ohne -update bleibt nicht das letzte stehen");
    }

    /// ⚠️ **Eine Datei ohne Inhalt ist kein Bild.** Der Rueckgabewert
    /// allein traegt die Zusage nicht, siehe [`lauf_und_pruefen`].
    #[test]
    fn ein_lauf_ohne_bild_gilt_als_fehlschlag() {
        let ziel = crate::zwischenname("blickprobe", "png");
        let mut b = std::process::Command::new("/bin/sh");
        b.arg("-c").arg("true");
        let f = lauf_und_pruefen(&mut b, &ziel, "sh", "Der Bildschirm")
            .expect_err("ohne Datei darf das nicht gutgehen");
        assert!(f.contains("kein Bild"), "{f}");
    }
}
