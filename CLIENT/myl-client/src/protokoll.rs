//! **Das Aktionsprotokoll**: jede Handlung des Agenten, ohne Klartext,
//! 30 Tage lang.
//!
//! # ⚑ Warum es das gibt
//!
//! Ein Mensch soll nachvollziehen koennen, was der Agent getan hat, auch
//! Tage spaeter: welches Werkzeug, wann, ob er es bestaetigt oder
//! abgelehnt hat, ob es gelang. So festgelegt vom Projektinhaber
//! (2026-09-25), nach dem Vorbild der Artikel 12 und 14 der Verordnung
//! (EU) 2024/1689, die fuer Hochrisiko-Systeme gelten.
//!
//! # ⛔️ Kein Klartext
//!
//! Was in ein Werkzeug hineinging und herauskam, steht nur als
//! **Fingerabdruck mit Schluessel** darin: SHA-256 ueber einen Schluessel,
//! der auf diesem Rechner zufaellig entsteht, und die Daten.
//! Ohne Schluessel laesst sich nicht einmal ein kurzer Dateiname
//! zurueckraten; mit ihm laesst sich pruefen, ob zwei Eintraege dieselbe
//! Eingabe hatten. Pfade, Suchbegriffe und Inhalte stehen nie darin.
//!
//! # ⚑ 30 Tage, dann weg
//!
//! Je Tag eine Datei `JJJJ-MM-TT.jsonl`; beim Schreiben verschwinden die,
//! die aelter sind. Was laenger liegt, waere ein Verlauf, den niemand
//! angefordert hat.

use std::io::Write;
use std::path::PathBuf;

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};

/// Wie viele Tage ein Protokoll bleibt.
pub const TAGE: i64 = 30;

/// Ob protokolliert wird; siehe [`einschalten`].
static AN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ein anderer Ort als der neben den Einstellungen, fuer Proben.
static ORT: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// **Schaltet das Protokoll fuer diesen Prozess ein.**
///
/// ⚑ **Die Bedieninstrumente schalten es ein, die Kiste nicht von
/// selbst.** Fenster, Konsole und `myl` rufen das beim Start; wer die
/// Bibliothek anders benutzt (etwa eine Probe), schreibt nichts in das
/// Protokoll des Nutzers. ⚠️ Dass jedes der drei es tut, halten ihre
/// Proben fest; ein viertes muesste es ebenso tun.
pub fn einschalten() {
    AN.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn an() -> bool {
    AN.load(std::sync::atomic::Ordering::SeqCst)
}

/// **Lenkt das Protokoll an einen anderen Ort und schaltet es ein**, fuer
/// Proben, die es lesen wollen.
pub fn ordner_setzen(ort: PathBuf) {
    *ORT.lock().unwrap_or_else(|e| e.into_inner()) = Some(ort);
    einschalten();
}

/// Ein Eintrag, so wie er in der Datei steht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Eintrag {
    /// Wann, in UTC, auf die Sekunde: `2026-09-25T14:03:07Z`.
    pub zeit: String,
    /// Was fuer eine Handlung: `datei_lesen`, `datei_schreiben`,
    /// `befehl`, `netz`, `sinn`, `werkzeug`, `notaus`, `schutzfilter`.
    pub art: String,
    /// Das Werkzeug oder die Stelle, die es ausloeste.
    pub werkzeug: String,
    /// Fingerabdruck der Eingabe (siehe Modulkopf), leer wenn keine.
    pub eingabe: String,
    /// Fingerabdruck der Ausgabe, leer wenn keine.
    pub ausgabe: String,
    /// `ausgefuehrt`, `abgelehnt` (vom Menschen), `abgebrochen` (Notaus),
    /// `abgewiesen` (Schutzfilter).
    pub entscheidung: String,
    /// `ok` oder `fehler`.
    pub ergebnis: String,
    /// Wie lange es dauerte, in Millisekunden.
    pub dauer_ms: u64,
}

/// Wo die Protokolle liegen: neben den Einstellungen, oder wo
/// `MYL_PROTOKOLL` hinzeigt.
pub fn ordner() -> PathBuf {
    if let Some(o) = ORT.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return o;
    }
    if let Some(o) = std::env::var_os("MYL_PROTOKOLL").filter(|o| !o.is_empty()) {
        return PathBuf::from(o);
    }
    crate::Einstellungen::vorgabepfad()
        .parent()
        .map(|p| p.join("protokoll"))
        .unwrap_or_else(|| PathBuf::from("protokoll"))
}

/// **Tage seit 1970 als Kalenderdatum**, ganzzahlig (nach dem Verfahren
/// von H. Hinnant, neu geschrieben).
fn datum(tage: i64) -> (i64, u32, u32) {
    let z = tage + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let tag = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let monat = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let jahr = yoe + era * 400 + if monat <= 2 { 1 } else { 0 };
    (jahr, monat, tag)
}

fn jetzt_sekunden() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn zeitstempel(sekunden: i64) -> String {
    let (j, m, t) = datum(sekunden.div_euclid(86_400));
    let s = sekunden.rem_euclid(86_400);
    format!("{j:04}-{m:02}-{t:02}T{:02}:{:02}:{:02}Z", s / 3600, (s / 60) % 60, s % 60)
}

fn tagesname(sekunden: i64) -> String {
    let (j, m, t) = datum(sekunden.div_euclid(86_400));
    format!("{j:04}-{m:02}-{t:02}.jsonl")
}

/// Der Schluessel dieses Rechners, beim ersten Mal erzeugt.
///
/// ⚑ **Zufall aus der Standardbibliothek**: `RandomState` holt seine
/// Saat beim Betriebssystem. Vier Werte zu je 64 Bit sind 256 Bit.
fn schluessel(ordner: &std::path::Path) -> Vec<u8> {
    let pfad = ordner.join(".schluessel");
    if let Ok(t) = std::fs::read_to_string(&pfad) {
        if let Some(k) = hex_lesen(t.trim()) {
            if k.len() == 32 {
                return k;
            }
        }
    }
    use std::hash::{BuildHasher, Hasher};
    let mut k = Vec::with_capacity(32);
    for i in 0..4u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        k.extend_from_slice(&h.finish().to_le_bytes());
    }
    let _ = std::fs::create_dir_all(ordner);
    let _ = std::fs::write(&pfad, hex(&k));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600));
    }
    k
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hex_lesen(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| s.get(i..i + 2).and_then(|p| u8::from_str_radix(p, 16).ok()))
        .collect()
}

/// **Der Fingerabdruck** von Daten: SHA-256 ueber Schluessel und Daten,
/// die ersten 128 Bit als Hex.
pub fn fingerabdruck(daten: &[u8]) -> String {
    use sha2::Digest;
    let k = schluessel(&ordner());
    let mut h = sha2::Sha256::new();
    h.update(&k);
    h.update(daten);
    hex(&h.finalize()[..16])
}

/// **Schreibt einen Eintrag** und raeumt Tage weg, die aelter als
/// [`TAGE`] sind.
///
/// ⚠️ **Ein Protokoll, das nicht geschrieben werden kann, haelt nichts
/// an**, es sagt es aber auf der Fehlerausgabe: Eine Handlung, die gelang,
/// wird nicht rueckwirkend ungeschehen, weil die Platte voll ist.
pub fn eintragen(e: &Eintrag) {
    if !an() {
        return;
    }
    let ordner = ordner();
    let _ = std::fs::create_dir_all(&ordner);
    let pfad = ordner.join(tagesname(jetzt_sekunden()));
    let zeile = serde_json::to_string(e).unwrap_or_default();
    let ergebnis = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&pfad)
        .and_then(|mut f| writeln!(f, "{zeile}"));
    if let Err(f) = ergebnis {
        eprintln!("Aktionsprotokoll: {}: {f}", pfad.display());
    }
    aufraeumen(&ordner, jetzt_sekunden());
}

/// Entfernt Tagesdateien, die aelter als [`TAGE`] sind.
fn aufraeumen(ordner: &std::path::Path, jetzt: i64) {
    let grenze = tagesname(jetzt - TAGE * 86_400);
    if let Ok(liste) = std::fs::read_dir(ordner) {
        for d in liste.flatten() {
            let name = d.file_name().to_string_lossy().to_string();
            // Nur, was nach einer Tagesdatei aussieht; der Name sortiert
            // wie das Datum.
            if name.len() == 16 && name.ends_with(".jsonl") && name < grenze {
                let _ = std::fs::remove_file(d.path());
            }
        }
    }
}

/// **Die juengsten Eintraege**, neueste zuerst.
pub fn lesen(hoechstens: usize) -> Vec<Eintrag> {
    let mut dateien: Vec<PathBuf> = std::fs::read_dir(ordner())
        .map(|l| {
            l.flatten()
                .map(|d| d.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .collect()
        })
        .unwrap_or_default();
    dateien.sort();
    let mut aus = Vec::new();
    for d in dateien.iter().rev() {
        let Ok(t) = std::fs::read_to_string(d) else { continue };
        for zeile in t.lines().rev() {
            if let Ok(e) = serde_json::from_str::<Eintrag>(zeile) {
                aus.push(e);
                if aus.len() >= hoechstens {
                    return aus;
                }
            }
        }
    }
    aus
}

/// **Ein Ereignis ohne Werkzeug** (Notaus, Schutzfilter), mit dem
/// Fingerabdruck dessen, was es ausloeste.
pub fn ereignis(art: &str, stelle: &str, ausloeser: &[u8], entscheidung: &str) {
    if !an() {
        return;
    }
    eintragen(&Eintrag {
        zeit: zeitstempel(jetzt_sekunden()),
        art: art.to_string(),
        werkzeug: stelle.to_string(),
        eingabe: if ausloeser.is_empty() { String::new() } else { fingerabdruck(ausloeser) },
        ausgabe: String::new(),
        entscheidung: entscheidung.to_string(),
        ergebnis: "ok".to_string(),
        dauer_ms: 0,
    });
}

/// **Die Fassung des Systemprompts, unter der ein Lauf steht**, mit dem
/// vollen SHA-256 (nicht geheim, und nur der volle Wert laesst sich gegen
/// `pruefsummen.txt` halten).
pub fn systemprompt(datei: &str, sha256: &str, geprueft: bool) {
    if !an() {
        return;
    }
    eintragen(&Eintrag {
        zeit: zeitstempel(jetzt_sekunden()),
        art: "systemprompt".to_string(),
        werkzeug: datei.to_string(),
        eingabe: sha256.to_string(),
        ausgabe: String::new(),
        entscheidung: if geprueft { "geprueft" } else { "abgewiesen" }.to_string(),
        ergebnis: if geprueft { "ok" } else { "fehler" }.to_string(),
        dauer_ms: 0,
    });
}

/// Welche Art von Handlung ein Werkzeug ist, aus seinem Namen.
fn art_von(name: &str) -> &'static str {
    match name {
        "read_file" | "list_directory" | "search_files" | "datei_lesen" | "verzeichnis_auflisten"
        | "dateien_suchen" => "datei_lesen",
        "write_file" | "edit_file" | "datei_schreiben" | "datei_bearbeiten" => "datei_schreiben",
        "run_command" | "befehl_ausfuehren" => "befehl",
        n if n.starts_with("web_") => "netz",
        n if n.contains("sehen") || n.contains("hoeren") || n.contains("look") || n.contains("listen") => "sinn",
        _ => "werkzeug",
    }
}

/// Ein Werkzeug, das jeden Aufruf protokolliert.
///
/// ⚑ **Es umhuellt, statt einzugreifen**, wie die Nachfrage: Das
/// Werkzeug weiss nichts davon, und was es tut, bleibt, wie es war.
pub struct Protokolliert {
    pub inner: Box<dyn Werkzeugausfuehrung>,
}

/// Der Satz, mit dem die Nachfrage eine Absage an das Modell zurueckgibt;
/// an ihm erkennt das Protokoll, dass der Mensch abgelehnt hat.
pub const ABSAGE: &str = "Der Nutzer hat diese Handlung abgelehnt.";

impl Werkzeugausfuehrung for Protokolliert {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn ausfuehren(&self, argumente: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        // ⚑ Ausgeschaltet kostet es nichts und legt nichts an, auch keinen
        //   Schluessel.
        // ⛔️ Nach dem Notaus laeuft kein Werkzeug mehr.
        if crate::notaus::ausgeloest() {
            eintragen(&Eintrag {
                zeit: zeitstempel(jetzt_sekunden()),
                art: art_von(self.inner.name()).to_string(),
                werkzeug: self.inner.name().to_string(),
                eingabe: if an() { fingerabdruck(argumente.to_string().as_bytes()) } else { String::new() },
                ausgabe: String::new(),
                entscheidung: "abgebrochen".to_string(),
                ergebnis: "fehler".to_string(),
                dauer_ms: 0,
            });
            return Err(Werkzeugfehler {
                grund: "Notaus: Der Mensch hat den Auftrag angehalten. Nichts wurde ausgefuehrt; \
                        beende den Auftrag."
                    .to_string(),
            });
        }
        if !an() {
            return self.inner.ausfuehren(argumente);
        }
        let anfang = std::time::Instant::now();
        let aus = self.inner.ausfuehren(argumente);
        let (entscheidung, ergebnis, ausgabe) = match &aus {
            Ok(t) => ("ausgefuehrt", "ok", fingerabdruck(t.as_bytes())),
            Err(f) if f.grund.starts_with(ABSAGE) => ("abgelehnt", "fehler", String::new()),
            Err(f) => ("ausgefuehrt", "fehler", fingerabdruck(f.grund.as_bytes())),
        };
        eintragen(&Eintrag {
            zeit: zeitstempel(jetzt_sekunden()),
            art: art_von(self.inner.name()).to_string(),
            werkzeug: self.inner.name().to_string(),
            eingabe: fingerabdruck(argumente.to_string().as_bytes()),
            ausgabe,
            entscheidung: entscheidung.to_string(),
            ergebnis: ergebnis.to_string(),
            dauer_ms: anfang.elapsed().as_millis() as u64,
        });
        aus
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    #[test]
    fn das_datum_stimmt() {
        assert_eq!(datum(0), (1970, 1, 1));
        assert_eq!(datum(20_356), (2025, 9, 25));
        assert_eq!(datum(11_016), (2000, 2, 29));
        assert_eq!(zeitstempel(1_758_808_987), "2025-09-25T14:03:07Z");
        assert_eq!(tagesname(1_758_808_987), "2025-09-25.jsonl");
    }

    /// ⛔️ **Kein Werkzeug haengt an der Protokollhuelle vorbei**, und jedes
    /// Bedieninstrument schaltet das Protokoll ein. Eine Einhaengung ohne
    /// Huelle liefe unprotokolliert, und niemand saehe es.
    #[test]
    fn jedes_werkzeug_ist_protokolliert_und_jeder_start_schaltet_ein() {
        let ruestung = include_str!("ruestung.rs");
        for (i, zeile) in ruestung.lines().enumerate() {
            if zeile.contains(".einhaengen(") {
                assert!(zeile.contains("protokolliert("), "ruestung.rs:{}: {zeile}", i + 1);
            }
        }
        assert!(ruestung.matches("protokolliert(ausfuehrung)").count() >= 5);
        for (datei, quelle) in [
            ("myl.rs", include_str!("bin/myl.rs")),
            ("myl-console/src/main.rs", include_str!("../../myl-console/src/main.rs")),
            ("myl-oberflaeche/src/main.rs", include_str!("../../myl-oberflaeche/src/main.rs")),
        ] {
            let start = quelle.find("fn main()").unwrap_or_else(|| panic!("{datei}: kein main"));
            let rumpf = &quelle[start..start + 400.min(quelle.len() - start)];
            assert!(rumpf.contains("myl_client::protokoll::einschalten();"), "{datei} schaltet das Protokoll nicht ein");
        }
    }

    /// ⚑ **Aelter als 30 Tage geht, juenger bleibt, Fremdes bleibt.**
    #[test]
    fn nach_dreissig_tagen_ist_es_weg() {
        let d = std::env::temp_dir().join(format!("myl-protokoll-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let jetzt = 1_758_808_987; // 2025-09-25
        for n in ["2025-08-25.jsonl", "2025-08-26.jsonl", "2025-09-25.jsonl", "notiz.txt"] {
            std::fs::write(d.join(n), "x").unwrap();
        }
        aufraeumen(&d, jetzt);
        assert!(!d.join("2025-08-25.jsonl").exists(), "31 Tage alt und noch da");
        assert!(d.join("2025-08-26.jsonl").exists(), "30 Tage alt und schon weg");
        assert!(d.join("2025-09-25.jsonl").exists());
        assert!(d.join("notiz.txt").exists(), "eine fremde Datei wurde geloescht");
        let _ = std::fs::remove_dir_all(&d);
    }
}
