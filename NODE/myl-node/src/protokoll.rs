//! Betriebsprotokoll: was nach dem Lauf noch da ist.
//!
//! # Wofür das gebaut ist
//!
//! Ein Testlauf über mehrere Maschinen an verschiedenen Orten hat eine
//! unangenehme Eigenschaft: **Man ist nicht dabei.** Wenn drei Knoten
//! eine Stunde laufen und einer davon zwanzig Minuten lang nichts
//! empfängt, entscheidet sich hinterher an den Protokollen, ob das
//! erklärbar ist oder nicht.
//!
//! Deshalb ist dieses Modul nicht „Logging", sondern der eigentliche
//! Ertrag des Laufs. Es schreibt eine Zeile JSON je Zustandsänderung.
//!
//! # Die drei Felder, an denen alles hängt
//!
//! Jede Zeile trägt:
//!
//! - **`folge`**, eine lückenlose Nummer je Knoten. Fehlt eine Nummer,
//!   fehlt eine Zeile, und das ist eine Aussage: Entweder ist die Datei
//!   beschädigt, oder der Knoten ist gestorben. Ohne Folgenummer ließe
//!   sich beides nicht von „es ist nichts passiert" unterscheiden.
//! - **`zeit_ms`**, die Wanduhr in Millisekunden seit Epoche. Sie ist
//!   **nicht** verlässlich synchron zwischen Maschinen, und genau
//!   deshalb steht sie da: Wer zwei Protokolle nebeneinanderlegt, muss
//!   den Versatz sehen können statt ihn zu übersehen.
//! - **`knoten`** und **`peer`**, damit eine eingesammelte Datei sich
//!   selbst zuordnet. Ein Protokoll, das nicht sagt, von wem es ist,
//!   ist nach dem Kopieren wertlos.
//!
//! # Warum nach jeder Zeile geschrieben wird
//!
//! [`Betriebsprotokoll::schreibe`] leert den Puffer sofort. Das kostet
//! Durchsatz, und es ist trotzdem richtig: **Der interessanteste
//! Zeitpunkt ist der letzte vor dem Absturz.** Ein gepuffertes Protokoll
//! verliert genau die Zeilen, wegen derer man es liest.
//!
//! # Warum ein eigener JSON-Schreiber
//!
//! Geschrieben wird ein Format, das der Testclient bereits liest: flache
//! Objekte, Zeichenketten, Zahlen und Wahrheitswerte, sonst nichts. Eine
//! Serialisierungsbibliothek dafür einzuziehen hieße, dem Knoten eine
//! Abhängigkeit zu geben, deren Möglichkeiten das Format ausdrücklich
//! nicht nutzen soll. Verschachtelte Objekte wären hier ein Nachteil:
//! Sie machen aus `grep` und `sort` unbrauchbare Werkzeuge, und in einer
//! Fehlersuche um zwei Uhr nachts sind genau die die ersten, die jemand
//! greift.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ein Wert im Protokoll. Bewusst klein gehalten, siehe Modul-Doku.
#[derive(Debug, Clone)]
pub enum Wert {
    Text(String),
    Zahl(i64),
    Wahr(bool),
}

impl Wert {
    fn schreibe(&self, ziel: &mut String) {
        match self {
            Wert::Text(t) => {
                ziel.push('"');
                for c in t.chars() {
                    match c {
                        '"' => ziel.push_str("\\\""),
                        '\\' => ziel.push_str("\\\\"),
                        '\n' => ziel.push_str("\\n"),
                        '\r' => ziel.push_str("\\r"),
                        '\t' => ziel.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            ziel.push_str(&format!("\\u{:04x}", c as u32))
                        }
                        c => ziel.push(c),
                    }
                }
                ziel.push('"');
            }
            Wert::Zahl(z) => ziel.push_str(&z.to_string()),
            Wert::Wahr(b) => ziel.push_str(if *b { "true" } else { "false" }),
        }
    }
}

/// Ein Protokolleintrag im Aufbau.
#[derive(Debug, Clone)]
pub struct Eintrag {
    art: String,
    felder: Vec<(String, Wert)>,
}

impl Eintrag {
    /// Neuer Eintrag einer Art. Die Art ist der Filterschlüssel für die
    /// spätere Auswertung, also kurz und stabil zu halten.
    pub fn neu(art: &str) -> Self {
        Self { art: art.to_string(), felder: Vec::new() }
    }

    pub fn text(mut self, name: &str, wert: impl Into<String>) -> Self {
        self.felder.push((name.to_string(), Wert::Text(wert.into())));
        self
    }

    pub fn zahl(mut self, name: &str, wert: i64) -> Self {
        self.felder.push((name.to_string(), Wert::Zahl(wert)));
        self
    }

    pub fn wahr(mut self, name: &str, wert: bool) -> Self {
        self.felder.push((name.to_string(), Wert::Wahr(wert)));
        self
    }

    /// Die Art dieses Eintrags.
    pub fn art(&self) -> &str {
        &self.art
    }
}

/// Fehler beim Protokollieren.
#[derive(Debug)]
pub enum ProtokollFehler {
    Datei(std::io::Error),
}

impl std::fmt::Display for ProtokollFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Datei(e) => write!(f, "Protokolldatei: {}", e),
        }
    }
}

impl std::error::Error for ProtokollFehler {}

/// Das Betriebsprotokoll eines Knotens.
pub struct Betriebsprotokoll {
    datei: File,
    pfad: PathBuf,
    knoten: String,
    peer: String,
    folge: u64,
    /// Auch auf die Standardausgabe schreiben. Für den Betrieb an einer
    /// Konsole nützlich, im Hintergrundlauf abschaltbar.
    auf_bildschirm: bool,
}

/// Millisekunden seit Epoche. Bei einer Uhr vor 1970 null statt Absturz:
/// Ein Protokoll darf an keiner Stelle der Grund sein, dass ein Lauf
/// endet.
pub fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Betriebsprotokoll {
    /// Legt eine neue Protokolldatei an.
    ///
    /// Der Dateiname trägt Knotennamen und Startzeit. Beides ist nötig,
    /// damit Dateien mehrerer Maschinen und mehrerer Läufe in einem
    /// Verzeichnis landen können, ohne einander zu überschreiben, und
    /// genau das passiert beim Einsammeln.
    pub fn neu(
        verzeichnis: &Path,
        knoten: &str,
        peer: &str,
        auf_bildschirm: bool,
    ) -> Result<Self, ProtokollFehler> {
        std::fs::create_dir_all(verzeichnis).map_err(ProtokollFehler::Datei)?;
        let sicherer_name: String = knoten
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let pfad = verzeichnis.join(format!("{}-{}.jsonl", sicherer_name, jetzt_ms()));
        let datei = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&pfad)
            .map_err(ProtokollFehler::Datei)?;
        Ok(Self {
            datei,
            pfad,
            knoten: knoten.to_string(),
            peer: peer.to_string(),
            folge: 0,
            auf_bildschirm,
        })
    }

    /// Der Pfad der Protokolldatei. Der Knoten nennt ihn beim Start,
    /// damit niemand suchen muss.
    pub fn pfad(&self) -> &Path {
        &self.pfad
    }

    /// Die Zahl der bisher geschriebenen Zeilen.
    pub fn geschrieben(&self) -> u64 {
        self.folge
    }

    /// Schreibt einen Eintrag und leert den Puffer sofort.
    ///
    /// **Fehler werden geschluckt.** Das ist eine bewusste Entscheidung
    /// und keine Nachlässigkeit: Ein Knoten, der aussteigt, weil seine
    /// Protokolldatei klemmt, hat aus einem Ärgernis einen Ausfall
    /// gemacht. Die Zahl der geschriebenen Zeilen bleibt trotzdem
    /// korrekt, sodass eine Lücke in der Folge sichtbar wird.
    pub fn schreibe(&mut self, eintrag: Eintrag) {
        self.folge += 1;
        let mut zeile = String::with_capacity(128);
        zeile.push('{');
        zeile.push_str("\"folge\":");
        zeile.push_str(&self.folge.to_string());
        zeile.push_str(",\"zeit_ms\":");
        zeile.push_str(&jetzt_ms().to_string());
        zeile.push_str(",\"knoten\":");
        Wert::Text(self.knoten.clone()).schreibe(&mut zeile);
        zeile.push_str(",\"peer\":");
        Wert::Text(self.peer.clone()).schreibe(&mut zeile);
        zeile.push_str(",\"art\":");
        Wert::Text(eintrag.art.clone()).schreibe(&mut zeile);
        for (name, wert) in &eintrag.felder {
            zeile.push(',');
            Wert::Text(name.clone()).schreibe(&mut zeile);
            zeile.push(':');
            wert.schreibe(&mut zeile);
        }
        zeile.push('}');

        if self.auf_bildschirm {
            println!("{}", zeile);
        }
        let _ = writeln!(self.datei, "{}", zeile);
        let _ = self.datei.flush();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// Ein eigenes Verzeichnis je Test.
    ///
    /// Die erste Fassung nahm `jetzt_ms()` als Namen. Tests laufen
    /// nebenläufig, zwei trafen dieselbe Millisekunde, und einer räumte
    /// dem anderen das Verzeichnis unter den Füßen weg. Der Zähler
    /// macht den Namen eindeutig, unabhängig von der Uhr.
    fn temp(marke: &str) -> PathBuf {
        static ZAEHLER: AtomicU64 = AtomicU64::new(0);
        let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("myl-node-test-{marke}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn jede_zeile_traegt_folge_zeit_knoten_und_peer() {
        let dir = temp("folge-felder");
        let mut p = Betriebsprotokoll::neu(&dir, "alpha", "12D3Koo…", false).unwrap();
        p.schreibe(Eintrag::neu("start"));
        p.schreibe(Eintrag::neu("stop"));
        let inhalt = std::fs::read_to_string(p.pfad()).unwrap();
        let zeilen: Vec<&str> = inhalt.lines().collect();
        assert_eq!(zeilen.len(), 2);
        for (i, z) in zeilen.iter().enumerate() {
            assert!(z.contains(&format!("\"folge\":{}", i + 1)), "{z}");
            assert!(z.contains("\"zeit_ms\":"), "{z}");
            assert!(z.contains("\"knoten\":\"alpha\""), "{z}");
            assert!(z.contains("\"peer\":\"12D3Koo…\""), "{z}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn die_folge_ist_lueckenlos() {
        // Eine Lücke ist eine Aussage (Datei beschädigt oder Knoten tot).
        // Damit sie eine ist, darf sie nicht im Normalbetrieb entstehen.
        let dir = temp("lueckenlos");
        let mut p = Betriebsprotokoll::neu(&dir, "b", "peer", false).unwrap();
        for i in 0..50 {
            p.schreibe(Eintrag::neu("takt").zahl("i", i));
        }
        let inhalt = std::fs::read_to_string(p.pfad()).unwrap();
        for (i, z) in inhalt.lines().enumerate() {
            assert!(z.contains(&format!("\"folge\":{}", i + 1)));
        }
        assert_eq!(p.geschrieben(), 50);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn anfuehrungszeichen_und_zeilenumbrueche_zerreissen_die_zeile_nicht() {
        // Adressen und Fehlermeldungen aus fremdem Code landen hier.
        // Eine einzige unmaskierte Zeichenkette macht aus einer Zeile
        // zwei und aus der Folgenummer eine Lüge.
        let dir = temp("maskierung");
        let mut p = Betriebsprotokoll::neu(&dir, "c", "peer", false).unwrap();
        p.schreibe(
            Eintrag::neu("fehler")
                .text("grund", "er sagte \"nein\"\nund ging\tweg\\")
                .text("steuerzeichen", "a\u{0007}b"),
        );
        let inhalt = std::fs::read_to_string(p.pfad()).unwrap();
        assert_eq!(inhalt.lines().count(), 1, "die Zeile wurde zerrissen: {inhalt}");
        assert!(inhalt.contains("\\\"nein\\\""));
        assert!(inhalt.contains("\\n"));
        assert!(inhalt.contains("\\t"));
        assert!(inhalt.contains("\\\\"));
        assert!(inhalt.contains("\\u0007"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn der_dateiname_traegt_knotennamen_und_zeit() {
        let dir = temp("dateiname");
        let p = Betriebsprotokoll::neu(&dir, "maschine-2", "peer", false).unwrap();
        let name = p.pfad().file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("maschine-2-"), "{name}");
        assert!(name.ends_with(".jsonl"), "{name}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ein_gefaehrlicher_knotenname_wird_entschaerft() {
        // Der Name kommt von der Kommandozeile. Ein Schrägstrich darin
        // würde in ein fremdes Verzeichnis schreiben.
        let dir = temp("pfad");
        let p = Betriebsprotokoll::neu(&dir, "../../etc/pass wd", "peer", false).unwrap();
        assert_eq!(p.pfad().parent().unwrap(), dir.as_path());
        let name = p.pfad().file_name().unwrap().to_string_lossy().to_string();
        assert!(!name.contains('/'), "{name}");
        assert!(!name.contains(".."), "{name}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn zahlen_und_wahrheitswerte_bleiben_unzitiert() {
        // Sonst muss die Auswertung raten, ob "3" eine Zahl ist.
        let dir = temp("typen");
        let mut p = Betriebsprotokoll::neu(&dir, "d", "peer", false).unwrap();
        p.schreibe(Eintrag::neu("m").zahl("peers", 3).wahr("vermittelt", true));
        let inhalt = std::fs::read_to_string(p.pfad()).unwrap();
        assert!(inhalt.contains("\"peers\":3"), "{inhalt}");
        assert!(inhalt.contains("\"vermittelt\":true"), "{inhalt}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
