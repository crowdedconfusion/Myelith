//! **Ein Bild in Worte fassen**, ueber llama.cpp und ein eigenes kleines
//! Sehmodell.
//!
//! ⚠️ **Was hier herauskommt, hat dieses Projekt nicht gerechnet.** Es
//! stammt aus einem fremden Modell, ist nicht bit-exakt und nicht Teil
//! des Konsenses. Die Werkzeugbeschreibung sagt das dem Modell
//! ausdruecklich, denn ein Hinweis, der es verschweigt, laedt zu einer
//! Antwort ein, die erfunden ist.

use std::path::Path;

use crate::laufwerk::Sehzeug;

/// Wonach gefragt wird, wenn niemand etwas Besonderes fragt.
///
/// ⚑ **Sie fragt nach dem Text im Bild mit.** Der haeufigste Anhang ist
/// ein Bildschirmfoto, und dort steht die Antwort meistens geschrieben.
/// **Die Regel fuer das Sehmodell**, vor jeder Frage.
///
/// ⛔️ **Keine Identifizierung und keine sensiblen Zuschreibungen** (Art. 5
/// Abs. 1 lit. f bis h und Anhang III Nr. 1 KI-Verordnung): Das Sehmodell
/// beschreibt, was zu sehen ist, und sagt nicht, wer jemand ist oder was
/// jemand fuehlt, glaubt oder ist. Auf Englisch, weil die Sehmodelle
/// Anweisungen auf Englisch am verlaesslichsten befolgen; die Antwort
/// folgt trotzdem der Sprache der Frage.
///
/// ⚠️ **Eine Anweisung und keine Garantie.** Ein Modell kann sie
/// uebergehen; der Schutzfilter vor dem Agenten faengt die offensichtlichen
/// Anfragen vorher ab.
pub const SEHREGEL: &str = "Rules: Describe only what is visible. Never identify real persons or say who someone is. \
Never infer or state anyone's emotions, ethnicity, religion, political views, sexual orientation, \
health or trade union membership. Answer in the language of the question.";

pub const VORGABEFRAGE: &str =
    "Beschreibe das Bild genau und gib jeden Text darin woertlich wieder.";

/// Wie viele Token die Beschreibung hoechstens hat.
pub const TOKEN_VORGABE: u32 = 320;

/// **Wie viele Token das Sehmodell hoechstens fuer das BILD verbrauchen
/// darf** (Fund 435, 2026-09-22).
///
/// ⛔️ **Ohne diese Grenze scheitert ein grosses Bild, und zwar stumm
/// genug, dass es wie ein fehlendes Sehmodell aussieht.** Gemeldet vom
/// Projektinhaber: Ein Bildschirmfoto von 5,6 MB brach mit
/// Rueckgabewert 1 ab, und das Sprachmodell antwortete daraufhin, es
/// koenne keine Bilder sehen.
///
/// **Nachgestellt und gemessen** (SmolVLM, ein Bild zu 6200 mal 4600):
///
/// | Grenze | Bloecke | Ergebnis |
/// |---|---|---|
/// | ohne | 387 | ⛔️ Abbruch: `failed to find a memory slot`, Rueckgabewert 1 |
/// | ohne, Kontext 32768 | 387 | laeuft durch und liefert **Unsinn** |
/// | 256 | 7 | gute Beschreibung, Text nur sinngemaess |
/// | **1024** | **27** | **gute Beschreibung, Text woertlich** |
/// | 2048 | 55 | faengt an, sich zu wiederholen |
///
/// ⚑ **Deshalb 1024 und nicht mehr.** Die Grenze ist keine Sparmassnahme:
/// **Mehr Bildtoken machen die Antwort schlechter**, nicht besser. Schon
/// ein Bild zu 4000 mal 3000 ohne Grenze brachte statt einer
/// ausfuehrlichen Beschreibung nur noch einen Satz.
///
/// ⚠️ **Null schaltet sie ab**, fuer ein aelteres Sehprogramm, das die
/// Option nicht kennt. Dann gilt wieder, was das Programm selbst tut.
pub const BILDTOKEN_VORGABE: u32 = 1024;

/// **Der Kontext, den ein Bildaufruf braucht**: Bildtoken, Antworttoken und
/// die Frage (hoechstens ein Token je zwei Bytes, also reichlich), dazu 256
/// Rand, auf 256 aufgerundet.
///
/// ⛔️ **Fund 548 (2026-10-15): Ohne `-c` legt das Sehprogramm den
/// KV-Speicher fuer den ganzen Trainingskontext des Modells an.** Gemessen
/// mit dem genauen Sehmodell (2,5 GB Gewichte), einem Bild zu 64 mal 64
/// und 16 Token Antwort: **15,5 GB Fussabdruck** ohne Angabe, **1,27 GB**
/// mit `-c 4096`, bei derselben Zeit (1,9 s) und derselben Beschreibung.
/// Auf einer Maschine, auf der der Agent ein grosses Modell haelt, draengte
/// jedes angesehene Bild ihn aus dem Speicher. Gefunden beim Eichen des
/// Dispatchers, der die Spitze fuer unplausibel hielt und richtig lag.
pub fn kontext_fuer(bildtoken: u32, antworttoken: u32, frage_bytes: usize) -> u32 {
    let roh = bildtoken
        .saturating_add(antworttoken)
        .saturating_add(u32::try_from(frage_bytes / 2).unwrap_or(u32::MAX))
        .saturating_add(256);
    roh.div_ceil(256).saturating_mul(256)
}

/// **Der Aufruf des Sehprogramms**, getrennt vom Lauf.
///
/// ⚑ **Damit die Argumente pruefbar sind.** Ein Aufruf, der nur im
/// Zusammenspiel mit einem echten Sehprogramm entsteht, wird nie
/// geprueft: Die Pruefsammlung hat keines, und in der CI liegt keines.
/// Genauso macht es `sprechen::befehl_fuer`.
fn befehl_fuer(zeug: &Sehzeug, bild: &Path, frage: &str) -> std::process::Command {
    befehl_mit(
        zeug,
        bild,
        frage,
        crate::zahl_aus_umgebung("MYL_SEHEN_BILDTOKEN", BILDTOKEN_VORGABE),
        crate::zahl_aus_umgebung("MYL_SEHEN_TOKEN", TOKEN_VORGABE),
    )
}

/// Dasselbe mit ausdruecklichen Zahlen.
///
/// 📌 **Eine eigene Funktion, damit keine Probe die Umgebung setzen muss.**
/// Bis zum 2026-10-15 setzte eine Probe `MYL_SEHEN_BILDTOKEN=0`, waehrend
/// nebenlaeufige Proben dieselbe Variable lasen; eine zweite Probe wurde
/// davon zufaellig rot. Die Umgebung ist prozessweit, eine Probe ist es nicht.
fn befehl_mit(zeug: &Sehzeug, bild: &Path, frage: &str, bildtoken: u32, antworttoken: u32) -> std::process::Command {
    let frage = if frage.trim().is_empty() { VORGABEFRAGE } else { frage };
    // ⛔️ Die Regel steht vor jeder Frage, siehe [`SEHREGEL`].
    let frage = format!("{SEHREGEL}\n\n{frage}");
    let mut befehl = std::process::Command::new(&zeug.programm);
    befehl
        .arg("-m")
        .arg(&zeug.modell)
        .arg("--mmproj")
        .arg(&zeug.projektor)
        .arg("--image")
        .arg(bild)
        .arg("-p")
        .arg(&frage)
        .arg("-n")
        .arg(antworttoken.to_string())
        .arg("-t")
        .arg(crate::faeden().to_string())
        .arg("--temp")
        .arg("0");
    // ⚑ **Die Bildgrenze nur, wenn sie gesetzt ist.** Siehe
    // [`BILDTOKEN_VORGABE`]: Null heisst „diese Option nicht mitgeben",
    // damit ein Sehprogramm, das sie nicht kennt, nicht daran scheitert.
    if bildtoken > 0 {
        befehl.arg("--image-max-tokens").arg(bildtoken.to_string());
        // ⛔️ Und der Kontext dazu, siehe [`kontext_fuer`]. Ohne Bildgrenze ist
        //    die Zahl der Bildtoken unbekannt, und dann bleibt es beim
        //    Programm.
        befehl.arg("-c").arg(kontext_fuer(bildtoken, antworttoken, frage.len()).to_string());
    }
    befehl
}

/// **Beschreibt ein Bild.**
pub fn beschreiben(zeug: &Sehzeug, bild: &Path, frage: &str) -> Result<String, String> {
    if !bild.is_file() {
        return Err(format!("die Datei '{}' gibt es nicht", bild.display()));
    }
    let mut befehl = befehl_fuer(zeug, bild, frage);

    let a = crate::prozess::laufen(&mut befehl, crate::FRIST_SEHEN_S, crate::AUSGABEGRENZE)
        .map_err(|f| format!("das Sehmodell {}: {f}", zeug.programm.display()))?;
    if !a.gut() {
        // ⚑ **Die letzten Zeilen des Protokolls, nicht alle.** Der Grund
        // steht bei llama.cpp am Ende; der Anfang ist Geraetekunde.
        return Err(format!("Das Sehmodell ist {}. Die letzten Zeilen:\n{}", a.kopf(), crate::schwanz(&a.fehler, 15)));
    }
    // ⚑ **Die Ausgabe, nicht das Protokoll.** llama.cpp schreibt seine
    // Ladezeilen nach stderr; was nach stdout geht, ist die Antwort.
    let text = crate::gesaeubert(&a.aus);
    if text.is_empty() {
        return Err("Das Sehmodell hat nichts gesagt.".into());
    }
    Ok(text)
}

#[cfg(test)]
mod proben {
    use super::*;
    use std::path::PathBuf;

    fn zeug() -> Sehzeug {
        Sehzeug {
            programm: PathBuf::from("/nirgends/llama-mtmd-cli"),
            modell: PathBuf::from("/nirgends/sehen.gguf"),
            projektor: PathBuf::from("/nirgends/sehen-mmproj.gguf"),
        }
    }

    fn argumente(b: &std::process::Command) -> Vec<String> {
        b.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    /// ⛔️ **Die Bildgrenze steht im Aufruf** (Fund 435).
    ///
    /// Ohne sie bricht ein grosses Bild mit `failed to find a memory
    /// slot` ab, und der Nutzer sieht eine Antwort, die klingt wie „ich
    /// habe kein Sehmodell".
    #[test]
    fn der_aufruf_begrenzt_die_bildtoken() {
        let a = argumente(&befehl_fuer(&zeug(), Path::new("/nirgends/bild.png"), ""));
        let i = a
            .iter()
            .position(|x| x == "--image-max-tokens")
            .expect("die Bildgrenze fehlt im Aufruf");
        assert_eq!(a[i + 1], BILDTOKEN_VORGABE.to_string());
    }

    /// ⛔️ **Der Kontext steht im Aufruf** (Fund 548), und er reicht fuer
    /// Bild, Frage und Antwort.
    #[test]
    fn der_aufruf_begrenzt_den_kontext() {
        let frage = "x".repeat(1000);
        let a = argumente(&befehl_fuer(&zeug(), Path::new("/nirgends/bild.png"), &frage));
        let i = a.iter().position(|x| x == "-c").expect("der Kontext fehlt im Aufruf");
        let c: u32 = a[i + 1].parse().expect("eine Zahl");
        assert_eq!(c % 256, 0);
        assert!(c >= BILDTOKEN_VORGABE + TOKEN_VORGABE + 500, "{c} reicht nicht fuer Bild, Antwort und Frage");
        assert!(c <= 4096, "{c} ist mehr, als ein Bildaufruf braucht");
        assert_eq!(kontext_fuer(1024, 320, 0), 1792);
        assert_eq!(kontext_fuer(1024, 320, 512), 2048);
    }

    /// ⚑ **Null laesst die Option weg**, fuer ein Sehprogramm, das sie
    /// nicht kennt. Ohne diesen Weg waere die einzige Abhilfe, die
    /// Fassung des Sehprogramms zu wechseln.
    #[test]
    fn null_laesst_die_bildgrenze_weg() {
        let a = argumente(&befehl_mit(&zeug(), Path::new("/nirgends/bild.png"), "", 0, TOKEN_VORGABE));
        assert!(
            !a.iter().any(|x| x == "--image-max-tokens"),
            "die Option steht trotz Null im Aufruf: {a:?}"
        );
        assert!(!a.iter().any(|x| x == "-c"), "ohne Bildgrenze kein Kontext: {a:?}");
    }

    /// ⚑ **Eine leere Frage wird zur Vorgabe**, und die fragt nach dem
    /// Text im Bild mit.
    #[test]
    fn eine_leere_frage_wird_zur_vorgabe() {
        let a = argumente(&befehl_fuer(&zeug(), Path::new("/nirgends/bild.png"), "   "));
        let i = a.iter().position(|x| x == "-p").expect("keine Frage im Aufruf");
        assert_eq!(a[i + 1], format!("{SEHREGEL}\n\n{VORGABEFRAGE}"));
    }

    /// ⛔️ **Die Regel steht vor jeder Frage**, auch vor einer eigenen.
    #[test]
    fn die_sehregel_steht_vor_jeder_frage() {
        let a = argumente(&befehl_fuer(&zeug(), Path::new("/nirgends/bild.png"), "Was steht auf dem Schild?"));
        let i = a.iter().position(|x| x == "-p").expect("keine Frage im Aufruf");
        assert!(a[i + 1].starts_with(SEHREGEL), "{}", a[i + 1]);
        assert!(a[i + 1].ends_with("Was steht auf dem Schild?"));
        assert!(SEHREGEL.contains("Never identify real persons"));
    }
}
