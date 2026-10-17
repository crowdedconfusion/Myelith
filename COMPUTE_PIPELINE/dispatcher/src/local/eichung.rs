//! **Der Teil des Bedarfs, den keine Datei nennt**, gemessen an einem
//! kurzen echten Lauf.
//!
//! # ⚑ Warum messen und nicht schaetzen
//!
//! Gewichte, KV-Speicher und Zustand sind aus dem Artefakt bekannt
//! ([`crate::local::bedarf`]). Puffer, Faeden, Tabellen der Laufzeit und was
//! eine fremde Laufzeit fuer sich anlegt, stehen nirgends. Eine Formel dafuer
//! waere eine zweite Wahrheit neben dem Code und altert still; ein Lauf misst,
//! was der Code wirklich tut.
//!
//! **Gemessen wird die Spitze des Arbeitsspeichers** (`ru_maxrss` ueber die
//! Kindprozesse) eines Befehls, den der Aufrufer nennt, also derselbe Aufruf,
//! der spaeter laeuft, mit der kleinsten Menge Arbeit. Der Zuschlag ist die
//! Spitze abzueglich dessen, was schon bekannt ist.
//!
//! # ⚠️ Wann eine Messung nichts sagt
//!
//! - **Rueckgabe ungleich null**: Ein gescheiterter Lauf hat nicht die
//!   Arbeit getan, deren Bedarf gefragt war. Er wird abgewiesen.
//! - **Spitze kleiner als das Bekannte**: Das Artefakt lag nicht ganz im
//!   Speicher (per mmap von der Platte gelesen), und die Spitze sagt dann
//!   nichts ueber den Rest. Es gilt die Vorgabe [`VORGABE_ZUSCHLAG`].
//! - **Windows**: Dort gibt es `getrusage` nicht; gemessen wird nicht, und
//!   die Vorgabe gilt.

use std::path::Path;

/// **Der Zuschlag ohne Messung: ein Gibibyte.**
///
/// ⚠️ **Eine Vorgabe und keine Messung.** Gewaehlt als Ruecklage, die fuer
/// Puffer einer Laufzeit dieser Groesse uebliche Werte deckt; ersetzt
/// durch eine Eichung, sobald es eine gibt.
pub const VORGABE_ZUSCHLAG: u64 = crate::local::erhebung::GIB;

/// Eine Messung.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Eichung {
    /// Wofuer gemessen wurde, so wie die Last im Plan heisst.
    pub last: String,
    /// Mit welchem Kontext der Lauf ging; der Zuschlag gilt ab hier.
    pub kontext: u64,
    /// Die Spitze des Arbeitsspeichers in Bytes, samt gelesener
    /// Dateiseiten.
    pub spitze_bytes: u64,
    /// ⚑ **Die Spitze dessen, was sich nicht auslagern laesst**: anonymer
    /// und komprimierter Speicher, Puffer der GPU, aber keine sauberen
    /// Seiten einer per mmap gelesenen Datei. Unter macOS die
    /// Lebenszeit-Hoechstmarke des Fussabdrucks, unter Linux das Hoechste
    /// von `RssAnon`, beides abgefragt, solange der Lauf lebt. `None`, wo
    /// es nicht messbar ist.
    #[serde(default)]
    pub fest_bytes: Option<u64>,
    /// Wie lange der Lauf dauerte, in Millisekunden.
    pub dauer_ms: u64,
    /// Der Befehl, damit sich die Messung wiederholen laesst.
    pub befehl: Vec<String>,
}

impl Eichung {
    /// **Der Zuschlag eines eigenen Artefakts**: was ueber KV-Speicher und
    /// Zustand (`kv_und_zustand`, fuer den Kontext dieser Messung) hinaus
    /// nicht auslagerbar war.
    ///
    /// ⚑ **Zuerst der Fussabdruck**, denn er zaehlt die Gewichte nicht mit,
    /// die per mmap gelesen werden, und sagt damit auch etwas, wenn das
    /// Artefakt nicht ganz in den Speicher passte. Sonst die Spitze abzueglich
    /// alles Bekannten (`gewichte`), und `None`, wenn sie darunter liegt.
    ///
    /// Gemessen am 2026-10-15 am 0,6B: Fussabdruck 139 MB bei 7 MB KV, also
    /// 132 MB; Spitze minus Gewichte 144 MB. Beide Wege stimmen ueberein,
    /// und beide liegen weit unter der Vorgabe.
    pub fn zuschlag_eigen(&self, gewichte: u64, kv_und_zustand: u64) -> Option<u64> {
        match self.fest_bytes {
            Some(f) => Some(f.saturating_sub(kv_und_zustand)),
            None => self.spitze_bytes.checked_sub(gewichte.checked_add(kv_und_zustand)?),
        }
    }

    /// **Der Zuschlag eines fremden Modells** mit `bytes` an Dateien: das
    /// Groessere aus „Spitze ueber den Dateien“ und dem Fussabdruck.
    ///
    /// ⚑ **Beide schaetzen denselben Teil**, das, was die Laufzeit neben
    /// ihren Gewichten anlegt, und jede verfehlt ihn auf ihre Art: Die Spitze
    /// liegt unter den Dateien, wenn die Laufzeit per mmap nicht alles las;
    /// der Fussabdruck verfehlt, was eine Laufzeit als Dateiseiten haelt.
    /// Vorsichtig ist das Groessere.
    pub fn zuschlag_fremd(&self, bytes: u64) -> Option<u64> {
        match (self.spitze_bytes.checked_sub(bytes), self.fest_bytes) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}

/// Alle Messungen einer Maschine, als eine JSON-Datei.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Eichbuch {
    pub eintraege: Vec<Eichung>,
}

impl Eichbuch {
    /// Liest das Buch; eine fehlende Datei ist ein leeres Buch, eine
    /// kaputte ein Fehler.
    ///
    /// ⚑ **Kaputt ist nicht leer.** Wer eine beschaedigte Datei als leer
    /// laese, rechnete still mit Vorgaben und ueberschriebe beim naechsten
    /// Eintrag alle alten Messungen.
    pub fn lesen(pfad: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(pfad) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", pfad.display())),
            // 📌 Fund 559: Windows meldet beim Lesen eines Verzeichnisses
            // „nicht gefunden“ (CI, windows-latest; unter Wine nicht). Leer
            // ist das Buch deshalb nur, wenn unter dem Pfad wirklich nichts
            // liegt.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && std::fs::symlink_metadata(pfad).is_err() => {
                Ok(Self::default())
            }
            Err(e) => Err(format!("{}: {e}", pfad.display())),
        }
    }

    /// Schreibt das Buch, zuerst daneben und dann umbenannt, damit ein
    /// Abbruch kein halbes Buch hinterlaesst.
    pub fn schreiben(&self, pfad: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let neben = pfad.with_extension("unfertig");
        std::fs::write(&neben, text).map_err(|e| format!("{}: {e}", neben.display()))?;
        std::fs::rename(&neben, pfad).map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Traegt eine Messung ein; eine alte fuer dieselbe Last und denselben
    /// Kontext wird ersetzt.
    pub fn eintragen(&mut self, e: Eichung) {
        self.eintraege.retain(|a| !(a.last == e.last && a.kontext == e.kontext));
        self.eintraege.push(e);
        self.eintraege.sort_by(|a, b| a.last.cmp(&b.last).then(a.kontext.cmp(&b.kontext)));
    }

    /// **Die Messung mit dem groessten Kontext** fuer diese Last, denn sie
    /// deckt die anderen ein.
    pub fn fuer(&self, last: &str) -> Option<&Eichung> {
        self.eintraege.iter().filter(|e| e.last == last).max_by_key(|e| e.kontext)
    }

    /// **Der Zuschlag eines eigenen Artefakts**: gemessen, falls das Buch
    /// eine Messung unter seinem Namen hat und sie etwas sagt, sonst
    /// [`VORGABE_ZUSCHLAG`].
    ///
    /// ⚑ **Eine Stelle fuer jeden Leser** (Plan, Schranke beim Laden,
    /// Trockenlauf): Wer die Regel zweimal hinschriebe, liesse die beiden
    /// verschieden altern.
    pub fn zuschlag_eigen(&self, b: &crate::local::bedarf::Modellbedarf) -> u64 {
        self.fuer(&b.name)
            .and_then(|e| e.zuschlag_eigen(b.gewichte_bytes(), b.kv_bytes(e.kontext)?.checked_add(b.zustand_je_folge)?))
            .unwrap_or(VORGABE_ZUSCHLAG)
    }

    /// **Der Zuschlag eines fremden Modells** unter `name` mit `bytes` an
    /// Dateien: gemessen oder [`VORGABE_ZUSCHLAG`].
    pub fn zuschlag_fremd(&self, name: &str, bytes: u64) -> u64 {
        self.fuer(name).and_then(|e| e.zuschlag_fremd(bytes)).unwrap_or(VORGABE_ZUSCHLAG)
    }
}

/// **Misst einen Befehl**: startet ihn, wartet, liest die Spitze.
///
/// ⚑ **Die Spitze gilt fuer alle Kindprozesse dieses Prozesses**, die
/// beendet und abgewartet sind (`RUSAGE_CHILDREN`). Darum misst ein
/// eigener Aufruf des Programms genau einen Befehl, und vorher darf kein
/// anderes Kind gelaufen sein: Seine Spitze stuende sonst mit darin.
pub fn messen(last: &str, kontext: u64, befehl: &[String]) -> Result<Eichung, String> {
    let (programm, argumente) = befehl.split_first().ok_or("kein Befehl zum Messen")?;
    let anfang = std::time::Instant::now();
    let mut kind = std::process::Command::new(programm).args(argumente).spawn().map_err(|e| format!("{programm}: {e}"))?;
    // ⚑ **Abgefragt, solange das Kind lebt**: Nach dem Abwarten gibt es den
    // Prozess nicht mehr, und mit ihm seine Zahlen.
    // ⚑ **Der ganze Baum, nicht nur das Kind**: Ein Sinn startet seine
    //   Laufzeit als Enkel (`myl sinne` startet das Sehprogramm), und dessen
    //   Speicher ist der, um den es geht. Je Abfrage wird ueber alle
    //   Nachkommen summiert, denn sie leben gleichzeitig.
    let mut fest: Option<u64> = None;
    let status = loop {
        let baum: Option<u64> = std::iter::once(kind.id())
            .chain(nachkommen(kind.id()))
            .filter_map(fest_eines_prozesses)
            .reduce(|a, b| a.saturating_add(b));
        if let Some(b) = baum {
            fest = Some(fest.map_or(b, |a| a.max(b)));
        }
        match kind.try_wait().map_err(|e| format!("{programm}: {e}"))? {
            Some(s) => break s,
            None => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    };
    let dauer_ms = anfang.elapsed().as_millis() as u64;
    if !status.success() {
        return Err(format!("{programm} endete mit {status}; ein gescheiterter Lauf misst nichts"));
    }
    let spitze_bytes = spitze_der_kinder().ok_or("die Spitze des Speichers ist hier nicht messbar")?;
    Ok(Eichung { last: last.to_string(), kontext, spitze_bytes, fest_bytes: fest, dauer_ms, befehl: befehl.to_vec() })
}

/// **Alle Nachkommen eines Prozesses**, Kinder, Enkel und so fort.
fn nachkommen(pid: u32) -> Vec<u32> {
    let mut aus = Vec::new();
    let mut offen = vec![pid];
    while let Some(p) = offen.pop() {
        for k in kinder(p) {
            // Ein Kreis kann es nicht geben; die Schranke haelt trotzdem jede Schleife auf.
            if !aus.contains(&k) && aus.len() < 4096 {
                aus.push(k);
                offen.push(k);
            }
        }
    }
    aus
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn kinder(pid: u32) -> Vec<u32> {
    let Ok(pid) = libc::pid_t::try_from(pid) else { return Vec::new() };
    let mut puffer = vec![0 as libc::pid_t; 256];
    let groesse = libc::c_int::try_from(puffer.len() * std::mem::size_of::<libc::pid_t>()).unwrap_or(0);
    // SICHERHEIT: Der Puffer ist so gross, wie `groesse` sagt; die Funktion
    // schreibt hoechstens so viele Kennungen hinein und gibt ihre Zahl zurueck.
    let n = unsafe { libc::proc_listchildpids(pid, puffer.as_mut_ptr().cast(), groesse) };
    let n = usize::try_from(n).unwrap_or(0).min(puffer.len());
    puffer[..n].iter().filter_map(|k| u32::try_from(*k).ok()).filter(|k| *k > 0).collect()
}

#[cfg(target_os = "linux")]
fn kinder(pid: u32) -> Vec<u32> {
    let Ok(eintraege) = std::fs::read_dir("/proc") else { return Vec::new() };
    eintraege
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|k| elter_aus_stat(&std::fs::read_to_string(format!("/proc/{k}/stat")).unwrap_or_default()) == Some(pid))
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn kinder(_pid: u32) -> Vec<u32> {
    Vec::new()
}

/// Die Elternkennung aus `/proc/<pid>/stat`: das zweite Feld nach der
/// schliessenden Klammer des Namens (der selbst Klammern und Leerzeichen
/// enthalten darf, deshalb von hinten gesucht).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn elter_aus_stat(stat: &str) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// **Der nicht auslagerbare Speicher eines laufenden Prozesses.**
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn fest_eines_prozesses(pid: u32) -> Option<u64> {
    let pid = libc::c_int::try_from(pid).ok()?;
    // SICHERHEIT: `rusage_info_v4` ist eine reine C-Struktur aus Zahlen;
    // `proc_pid_rusage` schreibt mit `RUSAGE_INFO_V4` genau diese Form.
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    let zeiger = (&raw mut info).cast::<libc::rusage_info_t>();
    if unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, zeiger) } != 0 {
        return None;
    }
    Some(info.ri_lifetime_max_phys_footprint)
}

#[cfg(target_os = "linux")]
fn fest_eines_prozesses(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let zeile = text.lines().find(|z| z.starts_with("RssAnon:"))?;
    let mut teile = zeile["RssAnon:".len()..].split_whitespace();
    let kib: u64 = teile.next()?.parse().ok()?;
    (teile.next()? == "kB").then_some(kib.checked_mul(1024)?)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn fest_eines_prozesses(_pid: u32) -> Option<u64> {
    None
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn spitze_der_kinder() -> Option<u64> {
    // SICHERHEIT: `rusage` ist eine reine C-Struktur aus Zahlen, fuer die
    // lauter Nullen ein gueltiger Wert sind, und `getrusage` schreibt nur
    // hinein.
    let mut r: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut r) } != 0 {
        return None;
    }
    let roh = u64::try_from(r.ru_maxrss).ok()?;
    // ⚠️ **macOS zaehlt in Bytes, Linux in KiB**: dieselbe Zahl, zwei
    // Einheiten.
    if cfg!(target_os = "macos") {
        Some(roh)
    } else {
        roh.checked_mul(1024)
    }
}

#[cfg(not(unix))]
fn spitze_der_kinder() -> Option<u64> {
    None
}

#[cfg(test)]
mod proben {
    use super::*;

    fn e(last: &str, kontext: u64, spitze: u64) -> Eichung {
        Eichung { last: last.into(), kontext, spitze_bytes: spitze, fest_bytes: None, dauer_ms: 1, befehl: vec![] }
    }

    #[test]
    fn der_zuschlag_gilt_nur_ueber_dem_bekannten() {
        // Ohne Fussabdruck: die Spitze ueber Gewichten, KV und Zustand.
        assert_eq!(e("a", 1, 10).zuschlag_eigen(3, 1), Some(6));
        assert_eq!(e("a", 1, 10).zuschlag_eigen(9, 1), Some(0));
        assert_eq!(e("a", 1, 10).zuschlag_eigen(10, 1), None, "eine Spitze unter dem Bekannten sagt nichts");
        // ⚑ Mit Fussabdruck zaehlen die Gewichte nicht, auch wenn die Spitze
        // unter ihnen liegt (das Artefakt passte nicht ganz).
        let mut m = e("a", 1, 10);
        m.fest_bytes = Some(5);
        assert_eq!(m.zuschlag_eigen(100, 2), Some(3));
        assert_eq!(m.zuschlag_eigen(100, 9), Some(0));
        // Fremd: das Groessere aus Spitze ueber den Dateien und Fussabdruck.
        assert_eq!(m.zuschlag_fremd(4), Some(6));
        assert_eq!(m.zuschlag_fremd(8), Some(5), "der Fussabdruck ist hier groesser");
        assert_eq!(m.zuschlag_fremd(40), Some(5));
        assert_eq!(e("a", 1, 10).zuschlag_fremd(40), None);
    }

    #[test]
    fn das_buch_ersetzt_und_waehlt_den_groessten_kontext() {
        let mut b = Eichbuch::default();
        b.eintragen(e("a", 512, 10));
        b.eintragen(e("a", 4096, 20));
        b.eintragen(e("a", 512, 11));
        b.eintragen(e("b", 99, 1));
        assert_eq!(b.eintraege.len(), 3, "derselbe Kontext ersetzt");
        assert_eq!(b.fuer("a").map(|x| x.spitze_bytes), Some(20));
        assert_eq!(b.fuer("c"), None);
    }

    /// ⚑ **Gemessen schlaegt die Vorgabe**, und ohne Messung gilt sie.
    #[test]
    fn das_buch_liefert_den_zuschlag_oder_die_vorgabe() {
        let b = crate::local::bedarf::Modellbedarf {
            name: "m".into(),
            rand_bytes: 100,
            rand_selten_bytes: 0,
            ebenen: vec![],
            kv_je_token: 2,
            zustand_je_folge: 10,
            hoechstkontext: 1000,
        };
        let mut buch = Eichbuch::default();
        assert_eq!(buch.zuschlag_eigen(&b), VORGABE_ZUSCHLAG);
        assert_eq!(buch.zuschlag_fremd("Sehen", 5), VORGABE_ZUSCHLAG);
        let mut m = e("m", 50, 500);
        m.fest_bytes = Some(170);
        buch.eintragen(m);
        // Fussabdruck 170 minus KV (2 · 50) und Zustand (10).
        assert_eq!(buch.zuschlag_eigen(&b), 60);
        buch.eintragen(e("Sehen", 0, 80));
        assert_eq!(buch.zuschlag_fremd("Sehen", 5), 75);
    }

    /// ⚑ **Kaputt ist nicht leer**, und eine fehlende Datei ist leer.
    #[test]
    fn ein_kaputtes_buch_ist_ein_fehler() {
        let ort = std::env::temp_dir().join(format!("eichbuch-probe-{}", std::process::id()));
        let _ = std::fs::remove_file(&ort);
        assert_eq!(Eichbuch::lesen(&ort), Ok(Eichbuch::default()));
        let mut b = Eichbuch::default();
        b.eintragen(e("a", 1, 2));
        b.schreiben(&ort).expect("schreiben");
        assert_eq!(Eichbuch::lesen(&ort).expect("lesen"), b);
        std::fs::write(&ort, "{ halb").expect("kaputt");
        assert!(Eichbuch::lesen(&ort).is_err());
        // Ein Lesefehler, der kein „fehlt“ ist, ist auch kein leeres Buch.
        assert!(Eichbuch::lesen(&std::env::temp_dir()).is_err(), "ein Verzeichnis ist kein Buch");
        let _ = std::fs::remove_file(&ort);
    }

    /// Die Elternkennung steht hinter dem Namen, auch wenn der Klammern traegt.
    #[test]
    fn die_elternkennung_steht_hinter_dem_namen() {
        assert_eq!(elter_aus_stat("123 (a b) S 45 1 2"), Some(45));
        assert_eq!(elter_aus_stat("123 (x) (y)) R 7 1"), Some(7));
        assert_eq!(elter_aus_stat("kaputt"), None);
    }

    /// ⚑ **Der Fussabdruck zaehlt den ganzen Baum**: Ein Kind, das selbst
    /// ein Kind startet, das Speicher belegt, misst mehr als ein Kind allein.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn der_fussabdruck_zaehlt_die_enkel_mit() {
        // Der Enkel haelt 200 MB anonymen Speicher eine Weile fest.
        let enkel = "python3 -c 'import time; b = bytearray(200*1024*1024); b[::4096] = b\"x\" * len(b[::4096]); time.sleep(1.0)'";
        let m = messen("x", 0, &["sh".into(), "-c".into(), format!("{enkel}; true")]).expect("Lauf");
        assert!(m.fest_bytes.is_some_and(|f| f > 150 * 1024 * 1024), "{m:?}");
    }

    /// Ein gescheiterter Lauf misst nichts; ein gelungener hat eine Spitze.
    #[cfg(unix)]
    #[test]
    fn ein_gescheiterter_lauf_misst_nichts() {
        let fehler = messen("x", 1, &["sh".into(), "-c".into(), "exit 3".into()]).expect_err("Rueckgabe 3");
        assert!(fehler.contains("misst nichts"), "{fehler}");
        let gut = messen("x", 1, &["sh".into(), "-c".into(), "sleep 0.2".into()]).expect("Lauf");
        assert!(gut.spitze_bytes > 0);
        // Ein Lauf, der lange genug lebt, hat einen Fussabdruck, wo er messbar ist.
        if cfg!(any(target_os = "macos", target_os = "linux")) {
            assert!(gut.fest_bytes.is_some_and(|f| f > 0), "{gut:?}");
        }
        assert!(messen("x", 1, &[]).is_err());
    }
}
