//! **Die Sinne des Clients**: Dateien annehmen, einordnen und, wo es
//! geht, in Worte fassen.
//!
//! # ⚑ Warum das eine eigene Kiste ist (Auftrag des Projektinhabers, 2026-09-17)
//!
//! Sehen und Hoeren werden an **zwei** Stellen gebraucht, und das ist
//! der ganze Grund:
//!
//! - In der **Agentenschleife**, wo ein Werkzeug sie aufruft.
//! - In der **Chatfunktion**, wo es **keine Werkzeugschleife gibt**.
//!   Wer im Chat ein Bild anhaengt, kann auf keinen Werkzeugaufruf
//!   hoffen; das Bild muss angesehen werden, wenn es hereinkommt, oder
//!   nie.
//!
//! ⛔️ **Zwei Stellen mit je eigener Umsetzung waeren genau die
//! Fehlerklasse, die dieses Projekt am haeufigsten trifft.** Also liegt
//! die Sache hier, einmal, und beide rufen sie.
//!
//! # ⚑ Warum ohne Shell
//!
//! Die erste Fassung waren `sh`-Skripte in einer Werkzeugkiste. Das war
//! hubsch modular und **unter Windows wirkungslos**, denn dort gibt es
//! kein `sh`. Der Client wird fuer Windows ausgeliefert. Jetzt startet
//! diese Kiste das fremde Programm unmittelbar
//! ([`std::process::Command`]), ohne Zeile, die eine Shell zerlegt:
//! Das laeuft auf allen drei Systemen und kann nebenbei kein Argument
//! einschleusen.
//!
//! # ⚑ Was hier NICHT passiert
//!
//! **Hier rechnet kein Modell.** Diese Kiste sucht ein Programm und
//! Gewichte, startet sie, wartet mit Frist und gibt zurueck, was
//! herauskam.
//!
//! ⚑ **Wo die Gewichte liegen, hat sich am 2026-09-21 geaendert.** Sie
//! liegen jetzt im Klon unter `MODELS/audio` und `MODELS/vision`,
//! zusammen mit allen anderen fremden Gewichten dieses Projekts, und
//! sie sind dort **nicht versioniert**. Wer keinen Klon hat, wird
//! weiter in `~/.myelith/sinne` gesucht; die ganze Reihenfolge steht bei
//! [`laufwerk::gewichtsorte`]. **Welches Modell taugt, entscheidet
//! weiter der Nutzer**: Eine Empfehlung im Text veraltet billiger als
//! eine im Code.
//!
//! ⚠️ **Und die Antwort stammt von einem fremden Modell.** Sie ist
//! nicht bit-exakt, nicht nachgerechnet und nicht Teil des Konsenses.
//! Wer sie weitergibt, sagt das dazu.

pub mod anhang;
pub mod aufnahme;
pub mod blick;
pub mod hoeren;
pub mod kennzeichnung;
pub mod laufwerk;
pub mod ort;
pub mod prozess;
pub mod schrift;
pub mod sehen;
pub mod sprechen;

pub use anhang::{Anhang, Art};
pub use laufwerk::{Aufnahmezeug, Hoerzeug, Mangel, Sehen, Sehzeug, Sinne, Sprechzeug, Stufe};

/// **Wie lange ein Sinneswerkzeug hoechstens rechnen darf**, in Sekunden.
///
/// ⚑ **Drei Minuten fuers Sehen, fuenf fuers Hoeren.** Ein kalter Start
/// laedt Gewichte von der Platte, und das dauert laenger als jede Frist,
/// die fuer `wc` gedacht ist. Nach oben muss trotzdem etwas stehen: Ein
/// Werkzeug, das nicht zurueckkommt, haelt die Schleife an und laesst
/// ein Fenster haengen.
pub const FRIST_SEHEN_S: u64 = 180;
/// Siehe [`FRIST_SEHEN_S`]. Eine lange Aufnahme braucht laenger als ein
/// einzelnes Bild.
pub const FRIST_HOEREN_S: u64 = 300;
/// Siehe [`FRIST_SEHEN_S`]. Sprechen geht schnell, und **es soll schnell
/// gehen**: Wer auf seine eigene Antwort wartet, wartet doppelt.
pub const FRIST_SPRECHEN_S: u64 = 60;

/// Wie viel Text ein Sinneswerkzeug hoechstens zurueckgibt.
///
/// ⚑ **Der Kontext ist die knappste Ressource.** Eine Mitschrift von
/// einer Stunde Ton fuellt ihn sonst allein. Was darueber liegt, wird
/// mit Vermerk abgeschnitten.
pub const AUSGABEGRENZE: usize = 8 * 1024;

/// **Die Speicherwache**: eine Frage, bevor ein Sinn seine Gewichte laedt.
///
/// Sie bekommt die Art und die Summe der Gewichte in Bytes und sagt, ob der
/// Sinn daneben starten darf; `Err` traegt den Grund, und der geht statt
/// einer Beschreibung an den Aufrufer.
///
/// ⚑ **Ein Haken und keine Abhaengigkeit.** Diese Kiste haengt von nichts
/// ab, und das bleibt so: Wer weiss, was sonst laeuft (der Client mit dem
/// geladenen Agenten), setzt die Wache ([`speicherwache_setzen`]). Ohne
/// gesetzte Wache startet jeder Sinn wie bisher.
pub type Speicherwache = fn(Art, u64) -> Result<(), String>;

static WACHE: std::sync::OnceLock<Speicherwache> = std::sync::OnceLock::new();

/// **Setzt die Speicherwache**, einmal je Prozess; `false`, wenn schon
/// eine gesetzt war.
pub fn speicherwache_setzen(w: Speicherwache) -> bool {
    WACHE.set(w).is_ok()
}

/// Fragt die Wache, falls es eine gibt, mit der Summe der Dateien.
///
/// ⚠️ **Eine Datei, deren Groesse sich nicht lesen laesst, zaehlt null.**
/// Fehlt sie wirklich, scheitert der Sinn gleich danach mit seinem eigenen,
/// genaueren Grund.
fn wachen(art: Art, dateien: &[&std::path::Path]) -> Result<(), String> {
    let Some(w) = WACHE.get() else { return Ok(()) };
    let bytes = dateien.iter().filter_map(|d| std::fs::metadata(d).ok()).map(|m| m.len()).sum();
    w(art, bytes)
}

/// **Wertet eine Datei aus, wenn es fuer ihre Art einen Sinn gibt.**
///
/// ⚑ **Die eine Tuer fuer beide Wege**: Chat und Agentenschleife rufen
/// diese Funktion, nicht die Einzelteile.
///
/// `None` heisst: Fuer diese Art sieht hier niemand hin (Text braucht
/// keinen Sinn, er wird gelesen). `Err` heisst: Es waere etwas zu sehen,
/// aber es fehlt etwas oder es ging schief, und der Text sagt was.
pub fn auswerten(
    sinne: &Sinne,
    datei: &std::path::Path,
    art: Art,
    frage: Option<&str>,
    stufe: Stufe,
) -> Option<Result<String, String>> {
    match art {
        Art::Bild => Some(match &sinne.sehen {
            Ok(s) => {
                let z = s.fuer(stufe);
                wachen(Art::Bild, &[&z.modell, &z.projektor])
                    .and_then(|()| sehen::beschreiben(z, datei, frage.unwrap_or(sehen::VORGABEFRAGE)))
            }
            Err(m) => Err(m.bericht()),
        }),
        // ⚑ **Die Stufe gilt nur fuers Sehen.** Beim Hoeren gibt es
        // heute ein Modell; eine zweite Sprosse, die auf dieselbe Datei
        // zeigt, waere eine Wahl ohne Unterschied.
        Art::Ton => Some(match &sinne.hoeren {
            Ok(z) => wachen(Art::Ton, &[&z.modell]).and_then(|()| hoeren::mitschreiben(z, datei, frage.unwrap_or("auto"))),
            Err(m) => Err(m.bericht()),
        }),
        Art::Text | Art::Sonstiges => None,
    }
}

/// **Die Sprechtaste, erster Teil: anfangen zuzuhoeren.**
///
/// ⚑ **Beides wird vorher geprueft**, aufnehmen und mitschreiben. Eine
/// Taste, die aufnimmt und dann niemanden hat, der mitschreibt, erzeugt
/// Tonmuell und eine Enttaeuschung.
pub fn zuhoeren_beginnen(sinne: &Sinne) -> Result<aufnahme::Aufnahme, String> {
    let zeug = sinne.aufnehmen.as_ref().map_err(|m| m.bericht())?;
    if let Err(m) = &sinne.hoeren {
        return Err(m.bericht());
    }
    aufnahme::starten(zeug)
}

/// **Die Sprechtaste, zweiter Teil: aufhoeren und mitschreiben.**
///
/// ⚑ **Die Tondatei wird danach weggeraeumt.** Was bleibt, ist der Text;
/// die Aufnahme selbst hat niemand angehaengt und niemand will sie
/// behalten. Wer sie behalten will, haengt sie an.
pub fn zuhoeren_beenden(
    sinne: &Sinne,
    laufende: aufnahme::Aufnahme,
    sprache: &str,
) -> Result<String, String> {
    let wav = laufende.beenden()?;
    let ergebnis = match &sinne.hoeren {
        Ok(z) => hoeren::mitschreiben(z, &wav, sprache),
        Err(m) => Err(m.bericht()),
    };
    let _ = std::fs::remove_file(&wav);
    ergebnis
}

/// Wie viele Kerne ein Sinnesmodell bekommt.
///
/// ⚑ **Vier, nicht alle.** Das Hauptmodell rechnet oft gleichzeitig, und
/// eine Beschreibung, die alles an sich zieht, macht das Gespraech
/// langsamer, als sie es wert ist. Ueber `MYL_SINNE_FAEDEN` umzustellen.
pub fn faeden() -> u32 {
    zahl_aus_umgebung("MYL_SINNE_FAEDEN", 4)
}

/// Eine Zahl aus der Umgebung, sonst die Vorgabe.
///
/// ⚠️ **Unlesbares gilt als nicht gesetzt.** Ein Tippfehler soll das
/// Werkzeug nicht anhalten, sondern nur nicht wirken.
pub fn zahl_aus_umgebung(name: &str, vorgabe: u32) -> u32 {
    std::env::var(name).ok().and_then(|w| w.trim().parse().ok()).unwrap_or(vorgabe)
}

/// **Die Antwort eines fremden Programms, aufgeraeumt**: Zeilen ohne
/// Rand, keine leeren.
///
/// ⚑ **Im knappen Kontext steht nichts Leeres.** llama.cpp und
/// whisper.cpp lassen gern eine Leerzeile vorweg.
pub fn gesaeubert(t: &str) -> String {
    t.lines()
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Die letzten `wieviel` nichtleeren Zeilen, fuer eine Fehlermeldung.
pub fn schwanz(t: &str, wieviel: usize) -> String {
    let zeilen: Vec<&str> = t.lines().map(str::trim).filter(|z| !z.is_empty()).collect();
    zeilen[zeilen.len().saturating_sub(wieviel)..].join("\n")
}

/// Ein Name im Zwischenordner, der keinem anderen Lauf gehoert.
///
/// ⚑ **Ohne Fremdkiste.** `tempfile` steht hier nur zum Pruefen zur
/// Verfuegung; eine Kiste, die nichts weiter braucht, soll dafuer auch
/// nichts weiter mitbringen. Prozessnummer und Nanosekunden reichen:
/// Der Name wird angelegt, benutzt und weggeraeumt.
pub fn zwischenname(stamm: &str, endung: &str) -> std::path::PathBuf {
    // ⛔️ **Prozessnummer und Uhr reichen nicht.** Zwei Faeden desselben
    // Prozesses haben dieselbe Nummer, und die Uhr ist nicht fein genug:
    // Der Vorleser spricht in einem Faden, waehrend ein Werkzeugaufruf
    // im anderen laeuft, und beide bekamen denselben Namen. Aufgefallen
    // an einer Probe, die **nur neben anderen** fehlschlug, in fuenf
    // Laeufen einmal. 📌 **Ein Fehlschlag, der sich allein nicht
    // nachstellen laesst, ist ein Hinweis auf etwas Geteiltes.**
    static ZAEHLER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let lauf = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{stamm}-{}-{n}-{lauf}.{endung}", std::process::id()))
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn die_ausgabe_wird_aufgeraeumt() {
        assert_eq!(gesaeubert("\n  Auf dem Schild steht HALT.  \n\n"), "Auf dem Schild steht HALT.");
        assert_eq!(gesaeubert("   \n\n"), "");
        assert_eq!(schwanz("a\nb\nc\nd", 2), "c\nd");
        assert_eq!(schwanz("a", 5), "a");
    }

    /// ⚑ **Text und Sonstiges haben hier keinen Sinn**, und das ist eine
    /// Aussage und kein Fehler: Text wird gelesen, nicht angesehen.
    #[test]
    fn fuer_text_sieht_niemand_hin() {
        let s = Sinne::finden_in(std::path::Path::new("/gibt/es/nicht"), &laufwerk::Eigene::default(), &[]);
        assert!(auswerten(&s, std::path::Path::new("x"), Art::Text, None, Stufe::Schnell).is_none());
        assert!(auswerten(&s, std::path::Path::new("x"), Art::Sonstiges, None, Stufe::Schnell).is_none());
        // Fuer ein Bild gibt es einen Sinn, er ist nur nicht eingerichtet.
        let a = auswerten(&s, std::path::Path::new("x"), Art::Bild, None, Stufe::Schnell)
            .expect("es gibt einen Sinn");
        assert!(a.is_err());
        assert!(a.unwrap_err().contains("Sehmodell"));
    }

    /// Die Wache dieser Proben: Sie lehnt genau 4242 Bytes ab und laesst
    /// alles andere durch, damit keine andere Probe sie spuert.
    fn probewache(art: Art, bytes: u64) -> Result<(), String> {
        if bytes == 4242 {
            Err(format!("Probewache: {art:?} mit {bytes} Bytes passt nicht"))
        } else {
            Ok(())
        }
    }

    /// ⚑ **Die Wache steht vor dem Start**: Lehnt sie ab, kommt ihr Grund
    /// zurueck und nicht der Fehler des Programms, das gar nicht erst lief.
    #[test]
    fn die_speicherwache_fragt_vor_dem_start() {
        speicherwache_setzen(probewache);
        let ort = std::env::temp_dir().join(format!("sinne-wache-{}", std::process::id()));
        std::fs::create_dir_all(&ort).expect("Ordner");
        let (modell, projektor) = (ort.join("sehen.gguf"), ort.join("sehen-mmproj.gguf"));
        std::fs::write(&modell, vec![0u8; 4000]).expect("Modell");
        std::fs::write(&projektor, vec![0u8; 242]).expect("Projektor");
        let zeug = Sehzeug { programm: ort.join("gibt-es-nicht"), modell: modell.clone(), projektor: projektor.clone() };
        let mut s = Sinne::finden_in(std::path::Path::new("/gibt/es/nicht"), &laufwerk::Eigene::default(), &[]);
        s.sehen = Ok(Sehen { schnell: Some(zeug.clone()), genau: None });
        let grund = auswerten(&s, &modell, Art::Bild, None, Stufe::Schnell).expect("Sinn").expect_err("abgelehnt");
        assert!(grund.contains("Probewache"), "{grund}");
        // Andere Groesse: Die Wache laesst durch, und es scheitert erst das
        // Programm, das es nicht gibt.
        std::fs::write(&projektor, vec![0u8; 243]).expect("Projektor");
        let grund = auswerten(&s, &modell, Art::Bild, None, Stufe::Schnell).expect("Sinn").expect_err("kein Programm");
        assert!(!grund.contains("Probewache"), "{grund}");
        // Dasselbe fuers Hoeren, mit einem Modell von genau 4242 Bytes.
        let hoermodell = ort.join("hoeren.bin");
        std::fs::write(&hoermodell, vec![0u8; 4242]).expect("Hoermodell");
        s.hoeren = Ok(Hoerzeug { programm: ort.join("gibt-es-nicht"), modell: hoermodell });
        let grund = auswerten(&s, &modell, Art::Ton, None, Stufe::Schnell).expect("Sinn").expect_err("abgelehnt");
        assert!(grund.contains("Probewache") && grund.contains("Ton"), "{grund}");
        let _ = std::fs::remove_dir_all(&ort);
    }
}
