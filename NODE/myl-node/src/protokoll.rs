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
//! # Wie viel aufbewahrt wird (Fund 624, 2026-10-23)
//!
//! **Hoechstens [`GESAMT_HOECHSTENS`] je Knoten, in Dateien zu
//! hoechstens [`DATEI_HOECHSTENS`].** Bis dahin wuchs das Protokoll ohne
//! Grenze: rund 740 Byte je Block, jede Gossip-Nachricht eine Zeile, je
//! Start eine neue Datei, die niemand raeumte; im Dauerlauf mit 20 Bloecken
//! je Sekunde waren es 96 MB je Knoten und Stunde, bei der geplanten
//! Blockzeit von 1 bis 2 s 12 bis 23 GB im Jahr.
//!
//! - Wird die laufende Datei zu gross, beginnt eine neue. Ihre erste Zeile
//!   ist `fortsetzung` mit dem Namen der vorigen, und die Folgenummer laeuft
//!   weiter: Wer die Dateien eines Laufs aneinanderlegt, bekommt ein
//!   lueckenloses Protokoll.
//! - Ueber alle Dateien dieses Knotens, auch die frueherer Starts, gilt der
//!   Deckel; darueber faellt die aelteste weg. Was fehlt, fehlt am Anfang,
//!   nie in der Mitte.
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

/// Groesste Protokolldatei, bevor eine neue beginnt.
pub const DATEI_HOECHSTENS: u64 = 16 * 1024 * 1024;

/// Groesster Platz aller Protokolldateien eines Knotens, als Vorgabe.
/// Bei 2 s Blockzeit sind das rund vier Tage.
pub const GESAMT_HOECHSTENS: u64 = 128 * 1024 * 1024;
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

/// Namen, die jede Zeile ohnehin trägt.
///
/// ⚑ **Fund 64:** `konsens_gesendet` schrieb `.text("art", …)` und
/// erzeugte damit eine Zeile mit **zwei** `art`-Feldern. Solche Zeilen
/// sind kein Fehler für den Schreiber und keiner für den Leser, der
/// zufällig das richtige nimmt: `json.loads` behält in Python das
/// letzte, andere Leser das erste. Eine Auswertung über mehrere
/// Maschinen zählte dann je nach Werkzeug verschiedene Dinge, ohne dass
/// irgendwo etwas fehlschlägt.
pub const RESERVIERTE_FELDNAMEN: [&str; 5] = ["folge", "zeit_ms", "knoten", "peer", "art"];

/// Bricht im Debug-Bau ab, wenn ein Feldname eine feste Spalte
/// überschreibt.
///
/// **`debug_assert` und nicht `assert`:** Ein Knoten im Betrieb darf
/// nicht wegen einer Protokollzeile sterben. Im Test bricht es ab, und
/// dort entsteht der Fehler auch. Zusätzlich prüft
/// `tests/bft_ueber_das_netz.rs` die geschriebenen Zeilen auf doppelte
/// Schlüssel, damit die Prüfung auch im Freigabebau greift.
fn pruefe_feldname(name: &str) {
    debug_assert!(
        !RESERVIERTE_FELDNAMEN.contains(&name),
        "Feldname {name:?} überschreibt eine feste Spalte der Protokollzeile. \
         Jede Zeile trägt {RESERVIERTE_FELDNAMEN:?} bereits; ein zweites Feld \
         gleichen Namens ergibt eine Zeile, die je nach Leser verschieden \
         gelesen wird"
    );
}

impl Eintrag {
    /// Neuer Eintrag einer Art. Die Art ist der Filterschlüssel für die
    /// spätere Auswertung, also kurz und stabil zu halten.
    pub fn neu(art: &str) -> Self {
        Self { art: art.to_string(), felder: Vec::new() }
    }

    pub fn text(mut self, name: &str, wert: impl Into<String>) -> Self {
        pruefe_feldname(name);
        self.felder.push((name.to_string(), Wert::Text(wert.into())));
        self
    }

    pub fn zahl(mut self, name: &str, wert: i64) -> Self {
        pruefe_feldname(name);
        self.felder.push((name.to_string(), Wert::Zahl(wert)));
        self
    }

    pub fn wahr(mut self, name: &str, wert: bool) -> Self {
        pruefe_feldname(name);
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
    verzeichnis: PathBuf,
    /// Der Knotenname, wie er im Dateinamen steht.
    stamm: String,
    /// Bytes in der laufenden Datei.
    bytes: u64,
    /// Die Zeit im Namen der laufenden Datei; jede neue liegt darüber.
    namenszeit: i64,
    datei_hoechstens: u64,
    gesamt_hoechstens: u64,
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
        Self::mit_grenze(verzeichnis, knoten, peer, auf_bildschirm, GESAMT_HOECHSTENS)
    }

    /// Wie [`Self::neu`], mit einem eigenen Deckel fuer alle Dateien dieses
    /// Knotens. Eine Datei ist hoechstens ein Viertel davon und hoechstens
    /// [`DATEI_HOECHSTENS`], damit beim Wegfallen der aeltesten nie das
    /// halbe Protokoll auf einmal verschwindet.
    pub fn mit_grenze(
        verzeichnis: &Path,
        knoten: &str,
        peer: &str,
        auf_bildschirm: bool,
        gesamt_hoechstens: u64,
    ) -> Result<Self, ProtokollFehler> {
        std::fs::create_dir_all(verzeichnis).map_err(ProtokollFehler::Datei)?;
        let stamm: String = knoten
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        // Über allen vorhandenen Namen dieses Knotens, auch früherer Starts.
        let hoechste = std::fs::read_dir(verzeichnis)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| protokollzeit(&e.file_name().to_string_lossy(), &stamm))
            .max()
            .unwrap_or(i64::MIN);
        let (pfad, namenszeit) = freier_pfad(verzeichnis, &stamm, hoechste.saturating_add(1));
        let datei = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&pfad)
            .map_err(ProtokollFehler::Datei)?;
        let mut p = Self {
            datei,
            pfad,
            verzeichnis: verzeichnis.to_path_buf(),
            stamm,
            bytes: 0,
            namenszeit,
            datei_hoechstens: (gesamt_hoechstens / 4).clamp(1, DATEI_HOECHSTENS),
            gesamt_hoechstens: gesamt_hoechstens.max(1),
            knoten: knoten.to_string(),
            peer: peer.to_string(),
            folge: 0,
            auf_bildschirm,
        };
        p.aufraeumen();
        Ok(p)
    }

    /// Eine neue Datei, sobald die laufende voll ist; ihre erste Zeile nennt
    /// die vorige. Gelingt das Oeffnen nicht, wird in der alten weiter
    /// geschrieben: Ein Protokoll darf nie der Grund sein, dass ein Lauf endet.
    fn wechseln(&mut self) {
        let (neu, namenszeit) = freier_pfad(&self.verzeichnis, &self.stamm, self.namenszeit + 1);
        let Ok(datei) = OpenOptions::new().create(true).append(true).open(&neu) else {
            return;
        };
        let vorher = self
            .pfad
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.datei = datei;
        self.pfad = neu;
        self.namenszeit = namenszeit;
        self.bytes = 0;
        self.schreibe(Eintrag::neu("fortsetzung").text("vorher", vorher));
        self.aufraeumen();
    }

    /// Haelt alle Dateien dieses Knotens unter dem Deckel; die aelteste
    /// faellt zuerst, die laufende nie.
    fn aufraeumen(&mut self) {
        let Ok(eintraege) = std::fs::read_dir(&self.verzeichnis) else {
            return;
        };
        let mut dateien: Vec<(i64, PathBuf, u64)> = eintraege
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let zeit = protokollzeit(&name, &self.stamm)?;
                Some((zeit, e.path(), e.metadata().ok()?.len()))
            })
            .collect();
        dateien.sort();
        let mut gesamt: u64 = dateien.iter().map(|d| d.2).sum();
        for (_, pfad, laenge) in dateien {
            if gesamt <= self.gesamt_hoechstens {
                break;
            }
            if pfad == self.pfad {
                continue;
            }
            if std::fs::remove_file(&pfad).is_ok() {
                gesamt -= laenge;
            }
        }
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
        self.bytes += zeile.len() as u64 + 1;
        if self.bytes >= self.datei_hoechstens {
            self.wechseln();
        }
    }
}

/// `<stamm>-<ms>.jsonl` mit einer Zeit von wenigstens `mindestens`, die es
/// im Verzeichnis noch nicht gibt.
///
/// ⛔️ **Streng steigend, nicht nur frei.** Rotiert das Protokoll schneller
/// als eine Millisekunde, wäre der Name einer eben geräumten alten Datei
/// wieder frei; die neue trüge dann den jüngsten Inhalt unter dem ältesten
/// Namen und würde als Nächstes geräumt. Gefunden am 2026-10-23 von der
/// Probe `das_protokoll_bleibt_unter_dem_deckel`, rot nur in der vollen
/// Sammlung, wo die Uhr langsamer weiterzählte als die Dateien wuchsen.
fn freier_pfad(verzeichnis: &Path, stamm: &str, mindestens: i64) -> (PathBuf, i64) {
    let mut zeit = jetzt_ms().max(mindestens);
    loop {
        let pfad = verzeichnis.join(format!("{stamm}-{zeit}.jsonl"));
        if !pfad.exists() {
            return (pfad, zeit);
        }
        zeit += 1;
    }
}

/// Die Zeit im Namen, wenn die Datei ein Protokoll genau dieses Knotens
/// ist: `<stamm>-<ziffern>.jsonl`. `k1` nimmt so nie `k10-…` mit.
fn protokollzeit(name: &str, stamm: &str) -> Option<i64> {
    let rest = name.strip_prefix(stamm)?.strip_prefix('-')?.strip_suffix(".jsonl")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

#[cfg(test)]
mod rotationstests {
    use super::*;

    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-protokollrotation-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn dateien(d: &Path, stamm: &str) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .filter(|e| protokollzeit(&e.file_name().to_string_lossy(), stamm).is_some())
            .map(|e| e.path())
            .collect();
        v.sort_by_key(|p| protokollzeit(&p.file_name().unwrap().to_string_lossy(), stamm));
        v
    }

    /// ⚑ **Fund 624: Der Platz bleibt unter dem Deckel**, und die Folge
    /// laeuft ueber die Dateien weiter. Die aeltesten fallen weg, und die
    /// erste aufbewahrte Datei beginnt mit `fortsetzung`.
    #[test]
    fn das_protokoll_bleibt_unter_dem_deckel() {
        let d = ordner("deckel");
        let deckel = 64 * 1024;
        let mut p = Betriebsprotokoll::mit_grenze(&d, "k1", "peer", false, deckel).unwrap();
        for i in 0..5000 {
            p.schreibe(Eintrag::neu("empfangen").zahl("i", i).text("polster", "x".repeat(40)));
        }
        let liste = dateien(&d, "k1");
        let gesamt: u64 = liste.iter().map(|f| std::fs::metadata(f).unwrap().len()).sum();
        assert!(gesamt <= deckel + 1024, "{gesamt} Bytes in {} Dateien", liste.len());
        assert!(liste.len() >= 3, "es wurde nicht rotiert: {}", liste.len());
        let erste = std::fs::read_to_string(&liste[0]).unwrap();
        assert!(erste.lines().next().unwrap().contains("\"art\":\"fortsetzung\""));
        // Lueckenlos ueber alle aufbewahrten Dateien.
        let folgen: Vec<i64> = liste
            .iter()
            .flat_map(|f| std::fs::read_to_string(f).unwrap().lines().map(String::from).collect::<Vec<_>>())
            .map(|z| z.split("\"folge\":").nth(1).unwrap().split(',').next().unwrap().parse().unwrap())
            .collect();
        assert!(folgen.windows(2).all(|w| w[1] == w[0] + 1), "Luecke in der Folge");
        // 5000 Eintraege plus je Wechsel eine Fortsetzungszeile.
        assert!(*folgen.last().unwrap() > 5000, "die Folge endet bei {}", folgen.last().unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔️ **Neue Namen liegen über allen alten**, auch wenn die Uhr dahinter
    /// liegt: Sonst sortierte eine neue Datei vor eine alte und fiele als
    /// erste weg.
    #[test]
    fn neue_namen_liegen_ueber_allen_alten() {
        let d = ordner("namen");
        let zukunft = jetzt_ms() + 10_000_000;
        std::fs::write(d.join(format!("k1-{zukunft}.jsonl")), b"{}\n").unwrap();
        let p = Betriebsprotokoll::mit_grenze(&d, "k1", "peer", false, 1 << 30).unwrap();
        let zeit = protokollzeit(&p.pfad().file_name().unwrap().to_string_lossy(), "k1").unwrap();
        assert!(zeit > zukunft, "neue Datei {zeit} liegt nicht über {zukunft}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Dateien frueherer Starts zaehlen mit, fremde Knoten nicht.
    #[test]
    fn fruehere_starts_zaehlen_mit_fremde_nicht() {
        let d = ordner("starts");
        std::fs::write(d.join("k1-1000.jsonl"), vec![b'a'; 50_000]).unwrap();
        std::fs::write(d.join("k10-1000.jsonl"), vec![b'b'; 50_000]).unwrap();
        std::fs::write(d.join("kette.dat"), vec![b'c'; 50_000]).unwrap();
        let _p = Betriebsprotokoll::mit_grenze(&d, "k1", "peer", false, 40_000).unwrap();
        assert!(!d.join("k1-1000.jsonl").exists(), "die alte Datei desselben Knotens blieb");
        assert!(d.join("k10-1000.jsonl").exists(), "ein fremder Knoten wurde geraeumt");
        assert!(d.join("kette.dat").exists());
        let _ = std::fs::remove_dir_all(&d);
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
