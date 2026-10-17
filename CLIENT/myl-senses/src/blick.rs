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

/// Wie lange die Frage nach den sichtbaren Fenstern hoechstens dauert.
///
/// ⚑ **Kurz**: Die Liste ist eine Systemabfrage ohne Rechnen. Wer laenger
/// braucht, haengt, und dann gilt der Blick als fremd, statt zu warten.
pub const FRIST_FENSTER_S: u64 = 5;

/// **Fragt macOS nach den Besitzern aller Fenster auf dem Bildschirm.**
///
/// ⚑ **Ebene 0 sind die gewoehnlichen Programmfenster**; Menueleiste,
/// Dock und Mitteilungen liegen darueber und zaehlen nicht. Der Name des
/// Besitzers braucht keine Freigabe zur Bildschirmaufnahme, anders als
/// der Fenstertitel, der hier gar nicht gelesen wird.
const FENSTER_MACOS: &str = r#"ObjC.import("CoreGraphics"); ObjC.import("Foundation");
var a = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements, 0));
var n = [];
for (var i = 0; i < a.count; i++) {
  var w = a.objectAtIndex(i);
  if (ObjC.unwrap(w.objectForKey("kCGWindowLayer")) === 0) n.push(ObjC.unwrap(w.objectForKey("kCGWindowOwnerName")));
}
n.join("\n");"#;

/// **Welche Programme gerade ein Fenster auf dem Bildschirm haben.**
///
/// # ⚑ Wozu
///
/// Ein Bildschirmfoto zeigt, was dort steht, und das kann die eigene
/// Arbeit sein oder eine fremde Webseite. **Welches von beiden, sagt das
/// Bild nicht, wohl aber die Liste der Programme, die es zeigt.** Was
/// daraus folgt, entscheidet der Client; hier wird nur gefragt.
///
/// ⚠️ **Das Programm im Vordergrund reicht nicht.** Ein Browser halb
/// hinter dem Terminal steht trotzdem im Bild.
///
/// # ⚑ `None` heisst: unbekannt
///
/// - **macOS**: `osascript` mit JavaScript, siehe [`FENSTER_MACOS`].
/// - **Linux unter X11**: `wmctrl -lx`, falls eingerichtet. ⚠️ Es nennt
///   auch Fenster auf anderen Arbeitsflaechen; das macht die Liste
///   laenger, nie kuerzer, und damit das Urteil hoechstens strenger.
/// - **Sonst** (Windows, Wayland, kein `wmctrl`): `None`.
///
/// ⛔️ **Eine leere Liste ist ebenfalls `None`.** Ein Bildschirm ohne ein
/// einziges Fenster ist moeglich, eine leere Antwort aber viel haeufiger
/// ein Fehler, und ein Fehler darf nicht als „nichts Fremdes zu sehen"
/// durchgehen.
pub fn sichtbare_programme() -> Option<Vec<String>> {
    let (mut befehl, lesen): (std::process::Command, fn(&str) -> Vec<String>) = if cfg!(target_os = "macos") {
        let mut b = std::process::Command::new("osascript");
        b.arg("-l").arg("JavaScript").arg("-e").arg(FENSTER_MACOS);
        (b, programme_aus_zeilen)
    } else if cfg!(target_os = "linux") {
        let mut b = std::process::Command::new("wmctrl");
        b.arg("-lx");
        (b, programme_aus_wmctrl)
    } else {
        return None;
    };
    let a = crate::prozess::laufen(&mut befehl, FRIST_FENSTER_S, crate::AUSGABEGRENZE).ok()?;
    if !a.gut() || a.mehr {
        return None;
    }
    let n = lesen(&a.aus);
    (!n.is_empty()).then_some(n)
}

/// Eine Zeile je Programm, ohne Doppel, in der Reihenfolge des Auftretens.
pub fn programme_aus_zeilen(text: &str) -> Vec<String> {
    let mut n: Vec<String> = Vec::new();
    for z in text.lines().map(str::trim).filter(|z| !z.is_empty()) {
        if !n.iter().any(|x| x == z) {
            n.push(z.to_string());
        }
    }
    n
}

/// **Die Fensterklassen aus `wmctrl -lx`.**
///
/// Eine Zeile ist `0x03a00003  0 Navigator.firefox  rechner  Titel`; die
/// dritte Spalte ist `Instanz.Klasse`, und die Klasse ist der Name, den
/// das Programm sich selbst gibt. ⚠️ Der Titel wird nicht gelesen: Er ist
/// das, was das Fenster zeigt, und damit genau das Fremde.
pub fn programme_aus_wmctrl(text: &str) -> Vec<String> {
    let klassen: String = text
        .lines()
        .filter_map(|z| z.split_whitespace().nth(2))
        .map(|k| k.rsplit('.').next().unwrap_or(k))
        .collect::<Vec<_>>()
        .join("\n");
    programme_aus_zeilen(&klassen)
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

    /// ⚑ **Jedes Programm einmal**, auch wenn es drei Fenster hat.
    #[test]
    fn die_programmliste_hat_keine_doppel() {
        assert_eq!(
            programme_aus_zeilen("Terminal\nFirefox\n\nTerminal\n  Vorschau \n"),
            ["Terminal", "Firefox", "Vorschau"]
        );
    }

    /// ⚑ **Aus `wmctrl` zaehlt die Klasse**, und der Titel bleibt ungelesen.
    #[test]
    fn aus_wmctrl_kommt_die_klasse() {
        let t = "0x03a00003  0 gnome-terminal-server.Gnome-terminal  rechner  ~/arbeit\n\
                 0x04200007  0 Navigator.firefox  rechner  Terminal Code Myelith\n";
        assert_eq!(programme_aus_wmctrl(t), ["Gnome-terminal", "firefox"]);
    }

    /// ⚑ **Auf diesem Rechner antwortet macOS**, und dann mit mindestens
    /// einem Namen; ohne Fenster (etwa auf einem Bauserver) ist `None`
    /// richtig. Geprueft wird die Form, nicht der Inhalt.
    #[test]
    fn die_abfrage_liefert_namen_oder_nichts() {
        if let Some(n) = sichtbare_programme() {
            assert!(!n.is_empty());
            assert!(n.iter().all(|x| !x.trim().is_empty() && !x.contains('\n')), "{n:?}");
        }
    }

    /// ⚠️ **Eine Datei ohne Inhalt ist kein Bild.** Der Rueckgabewert
    /// allein traegt die Zusage nicht, siehe [`lauf_und_pruefen`].
    #[test]
    fn ein_lauf_ohne_bild_gilt_als_fehlschlag() {
        let ziel = crate::zwischenname("blickprobe", "png");
        let mut b = crate::prozess::probeskript("true", "exit 0");
        let f = lauf_und_pruefen(&mut b, &ziel, "sh", "Der Bildschirm")
            .expect_err("ohne Datei darf das nicht gutgehen");
        assert!(f.contains("kein Bild"), "{f}");
    }
}
