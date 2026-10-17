//! **Was diese Maschine hat, und was davon gerade frei ist.**
//!
//! # 📌 Was ein Scan nicht darf
//!
//! **Raten.** Jede Groesse, die sich auf dieser Maschine nicht ermitteln
//! laesst, ist `None`. Eine erfundene Zahl waere schlimmer als eine
//! fehlende: Ein Plan verteilte dann etwas, das es so nicht gibt.
//!
//! # ⚑ Gesamt und frei sind zwei Fragen
//!
//! Was eine Maschine **hat**, aendert sich nicht; was **frei** ist, aendert
//! sich mit jedem Programm, das jemand oeffnet. Die Freigabe des Besitzers
//! bezieht sich auf das erste, ein Plan auf das zweite. Darum stehen beide
//! nebeneinander und nicht eines statt des anderen.
//!
//! ⚠️ **Frei ist eine Momentaufnahme.** Wer danach etwas oeffnet, nimmt
//! es wieder weg; der Plan rechnet deshalb mit einer Ruecklage
//! ([`crate::local::plan::Regeln::ruecklage_bytes`]) und wird neu
//! aufgestellt, statt sich auf die alte Zahl zu verlassen.
//!
//! 📌 *Bis zum 2026-10-15 stand die Erhebung im Client
//! (`myl-client/src/hardware.rs`). Sie ist hierher gezogen, damit Client,
//! Knoten und Messwerkzeug dieselbe lesen; der Client reicht die Typen unter
//! ihren alten Namen weiter.*

use std::path::Path;

/// Alles, was der Scan ueber diese Maschine herausbekommen hat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Hardware {
    /// Logische Kerne, die dieser Prozess sehen darf.
    pub kerne: usize,
    /// Arbeitsspeicher der Maschine, `None` wenn nicht ermittelbar.
    pub speicher_bytes: Option<u64>,
    /// ⚑ **Was davon gerade verfuegbar ist**, ohne dass anonymer Speicher
    /// ausgelagert werden muss; Dateicache zaehlt als verfuegbar, denn das
    /// System gibt ihn ohne Auslagern her. Unter Linux die Schaetzung des
    /// Kerns (`MemAvailable`), unter macOS aus `vm_stat` gerechnet (siehe
    /// [`frei_aus_vm_stat`]), unter Windows der freie physische Speicher.
    pub speicher_frei_bytes: Option<u64>,
    /// Der Datentraeger, auf dem Myelith seine Daten haelt.
    pub platte: Option<Platte>,
    /// Jedes gefundene Rechenwerk ausser der CPU.
    pub rechenwerke: Vec<Rechenwerk>,
}

/// Der Datentraeger unter einem bestimmten Verzeichnis.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Platte {
    /// Das Verzeichnis, auf das sich die Zahlen beziehen.
    pub pfad: String,
    pub gesamt_bytes: u64,
    /// Frei fuer diesen Nutzer, nicht frei im Dateisystem: Auf vielen
    /// Systemen ist ein Teil fuer den Systemverwalter zurueckgehalten.
    pub frei_bytes: u64,
}

/// Ein Rechenwerk ausser der CPU.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Rechenwerk {
    /// Stabil ueber Neustarts, denn die Freigabe wird darunter
    /// abgelegt. ⚑ **Nicht der Listenplatz:** Wer eine zweite Karte
    /// einbaut, verschoebe damit alle Freigaben um eins.
    pub kennung: String,
    pub name: String,
    /// Eigener Speicher in Bytes. `None` heisst **gemeinsamer
    /// Speicher** mit der CPU, nicht „unbekannt"; das unterscheidet
    /// [`Rechenwerk::gemeinsamer_speicher`].
    pub speicher_bytes: Option<u64>,
    /// ⚑ **Davon gerade frei.** `None` bei gemeinsamem Speicher (dann
    /// gilt [`Hardware::speicher_frei_bytes`]) und wo die Maschine es nicht
    /// sagt.
    pub frei_bytes: Option<u64>,
    /// Teilt sich das Rechenwerk den Arbeitsspeicher mit der CPU?
    pub gemeinsamer_speicher: bool,
    /// Kerne des Rechenwerks, soweit die Maschine sie nennt.
    pub kerne: Option<u32>,
    /// ⚑ **Welcher Rechenweg ueber dieses Geraet fuehren wuerde.**
    /// `None` heisst: gar keiner, und das ist etwas anderes als „einer,
    /// der hier nicht rechnet".
    pub rueckseite: Option<Rueckseite>,
    /// ⚑ **Die PCI-Adresse**, soweit die Maschine sie nennt (Linux:
    /// `sysfs`). Ueber sie findet der Schalter je Karte die Karte, die CUDA
    /// oder HIP zaehlt; deren Nummern und die Reihenfolge hier muessen
    /// nicht uebereinstimmen. `None` (macOS, Windows) heisst: geschaltet
    /// wird der ganze Rechenweg.
    pub pci: Option<String>,
}

/// Der Rechenweg, der ein Geraet bedient.
///
/// ⚑ **Eine Eigenschaft des Geraets und nicht der Uebersetzung.** Ob der
/// Weg hier auch **rechnet**, ist die zweite Frage, und sie wird dort
/// gestellt, wo der Code ausgewaehlt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Rueckseite {
    Metal,
    Cuda,
    Rocm,
}

impl Rueckseite {
    /// Der Name, unter dem dieser Rechenweg in den Kernkisten steht.
    ///
    /// ⚑ **Dieselbe Schreibweise wie im Konformitaetslauf**, denn beide
    /// fragen dieselbe Liste.
    pub const fn backend(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Rocm => "rocm",
        }
    }

    /// Wie er in einer Beschriftung steht.
    pub const fn anzeige(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Cuda => "CUDA",
            Self::Rocm => "ROCm",
        }
    }
}

impl Hardware {
    /// Fragt die Maschine ab.
    ///
    /// ⚑ **`datenort` ist das Verzeichnis, auf das sich die
    /// Plattenzahlen beziehen.** Ein Rechner hat mehrere Datentraeger,
    /// und die Frage „wieviel Platz ist da" hat ohne einen Ort keine
    /// Antwort.
    pub fn erheben(datenort: &Path) -> Self {
        Self::erheben_mit(datenort, Self::rechenwerke())
    }

    /// **Wie [`Self::erheben`], mit schon gefundenen Rechenwerken.**
    ///
    /// ⚑ Fuer einen Aufrufer, der die Rechenwerke einmal sucht und sich
    /// merkt (der Client): Ihre Suche kostet einen Unterprozess
    /// (`system_profiler` unter macOS), Speicher und Platte nicht. Gemerkt
    /// wird **dort und nicht hier**, damit jede Probe mit nachgebildeten
    /// Karten weiter frisch sucht.
    pub fn erheben_mit(datenort: &Path, rechenwerke: Vec<Rechenwerk>) -> Self {
        Self {
            kerne: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            speicher_bytes: speicher_der_maschine(),
            speicher_frei_bytes: speicher_frei(),
            platte: platte_unter(datenort),
            rechenwerke,
        }
    }

    /// Nur die Rechenwerke, ohne Speicher und ohne Platte.
    ///
    /// ⚑ **Weil der Scan eines Rechenwerks einen Unterprozess kostet
    /// und der Rest nicht.** Wer nur wissen will, welches Geraet welchen
    /// Rechenweg bedient, braucht weder `statvfs` noch `hw.memsize`.
    pub fn rechenwerke() -> Vec<Rechenwerk> {
        let mut werke = rechenwerke_suchen();
        eindeutig_machen(&mut werke);
        werke
    }

    /// **Nur die Platte unter `pfad`**, ohne Speicher und Rechenwerke.
    ///
    /// ⚑ Fuer die Frage vor einem Download, ob der Platz reicht: Sie
    /// kommt oft und soll nicht jedes Mal die Rechenwerke suchen. Gemessen
    /// wird wie in [`Self::erheben`], an einer Stelle.
    pub fn platte(pfad: &Path) -> Option<Platte> {
        platte_unter(pfad)
    }
}

impl Rechenwerk {
    /// **Wie das Geraet heisst**, samt seinem Rechenweg.
    ///
    /// ⚑ **Der Rechenweg gehoert in die Beschriftung** (Auftrag des
    /// Projektinhabers, 2026-09-16). Wer zwei Karten im Rechner hat, muss
    /// an der Zeile sehen, welche wovon bedient wird.
    pub fn beschriftung(&self) -> String {
        match self.rueckseite {
            Some(r) => format!("{} ({})", self.name, r.anzeige()),
            None => self.name.clone(),
        }
    }
}

/// **Nur das Freie im Arbeitsspeicher, jetzt.**
///
/// ⚑ **Ohne den Scan der Rechenwerke**: Der kostet auf macOS einen
/// Unterprozess von rund einer Sekunde, und vor dem Start eines Sinnes
/// zaehlt nur der Speicher.
pub fn speicher_frei_jetzt() -> Option<u64> {
    speicher_frei()
}

/// Ein Gibibyte in Bytes.
pub const GIB: u64 = 1024 * 1024 * 1024;

/// Ein Mebibyte in Bytes.
pub const MIB: u64 = 1024 * 1024;

// ── Arbeitsspeicher ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn speicher_der_maschine() -> Option<u64> {
    // `hw.memsize` gibt die Bytes als u64 zurueck.
    let mut wert: u64 = 0;
    let mut laenge = std::mem::size_of::<u64>();
    let name = c"hw.memsize";
    // SICHERHEIT: `wert` ist genau so gross, wie `laenge` sagt, und der
    // Name ist eine nullterminierte Konstante.
    let rueckgabe = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut wert).cast(),
            &mut laenge,
            std::ptr::null_mut(),
            0,
        )
    };
    (rueckgabe == 0 && wert > 0).then_some(wert)
}

#[cfg(target_os = "linux")]
fn speicher_der_maschine() -> Option<u64> {
    // `MemTotal:   16334204 kB`
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    meminfo_feld(&text, "MemTotal:")
}

#[cfg(target_os = "windows")]
fn speicher_der_maschine() -> Option<u64> {
    // ⚑ Ueber die Systemabfrage und nicht ueber eine Bibliothek: Der
    // Scan laeuft einmal beim Oeffnen der Seite, und ein Unterprozess
    // kostet dort nichts.
    let aus = powershell("(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory")?;
    aus.trim().parse().ok()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn speicher_der_maschine() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn speicher_frei() -> Option<u64> {
    // ⚑ **`vm_stat` und kein Mach-Aufruf.** Die Mach-Schnittstelle steht
    // in `libc` nur noch als veraltet da, und ein Unterprozess kostet hier
    // so wenig wie beim Scan der Rechenwerke.
    frei_aus_vm_stat(&befehl("vm_stat", &[])?, speicher_der_maschine())
}

#[cfg(target_os = "linux")]
fn speicher_frei() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    meminfo_feld(&text, "MemAvailable:")
}

#[cfg(target_os = "windows")]
fn speicher_frei() -> Option<u64> {
    // `FreePhysicalMemory` zaehlt in KiB.
    let aus = powershell("(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory")?;
    aus.trim().parse::<u64>().ok().map(|k| k.saturating_mul(1024))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn speicher_frei() -> Option<u64> {
    None
}

/// Ein Feld aus `/proc/meminfo` in Bytes, etwa `MemAvailable:  812 kB`.
///
/// ⚑ **Ohne Einheit `kB` gilt es nicht.** Der Kern schreibt sie bei jedem
/// Speicherfeld; eine Zeile ohne sie ist kein Speicherfeld, und eine Zahl
/// ohne bekannte Einheit waere geraten.
// Nur Linux ruft sie; geprueft wird sie ueberall.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn meminfo_feld(text: &str, feld: &str) -> Option<u64> {
    let zeile = text.lines().find(|z| z.starts_with(feld))?;
    let mut teile = zeile[feld.len()..].split_whitespace();
    let kib: u64 = teile.next()?.parse().ok()?;
    (teile.next()? == "kB").then_some(kib.checked_mul(1024)?)
}

/// **Verfuegbar nach `vm_stat`**, in Bytes.
///
/// ⚑ **Gerechnet als Rest:** Gesamtspeicher minus fest verdrahtet, minus
/// den Platz des Kompressors, minus anonyme Seiten, plus entleerbare. Was
/// bleibt, ist frei oder Dateicache, und beides gibt das System her, ohne
/// anonymen Speicher auszulagern.
///
/// 📌 **Hier stand zuerst „frei, inaktiv und spekulativ“, und das schwankte
/// mit dem Cache** (2026-10-15): Direkt nachdem das 35B gelesen war, lagen
/// seine Seiten als *aktive* Dateiseiten im Speicher, und dieselbe Maschine
/// meldete 8,8 statt 12,9 GiB. Die Restrechnung ergab 17,2 GiB,
/// `memory_pressure` meldete zur selben Zeit 75 % frei, rund 18 GiB.
///
/// Fehlen die Felder der Restrechnung oder der Gesamtspeicher, gilt der
/// alte, vorsichtigere Weg. Die Seitengroesse steht in der Kopfzeile
/// (`page size of 16384 bytes`); fehlt sie, gilt keine Zahl.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn frei_aus_vm_stat(text: &str, gesamt: Option<u64>) -> Option<u64> {
    let kopf = text.lines().next()?;
    let ab = kopf.find("page size of ")? + "page size of ".len();
    let seite: u64 = kopf[ab..].split_whitespace().next()?.parse().ok()?;
    let seiten = |feld: &str| -> Option<u64> {
        let zeile = text.lines().find(|z| z.trim_start().starts_with(feld))?;
        zeile.rsplit(':').next()?.trim().trim_end_matches('.').parse().ok()
    };
    let rest = (|| {
        let belegt = seiten("Pages wired down:")?
            .checked_add(seiten("Pages occupied by compressor:")?)?
            .checked_add(seiten("Anonymous pages:")?)?
            .checked_mul(seite)?;
        let entleerbar = seiten("Pages purgeable:")?.checked_mul(seite)?;
        Some(gesamt?.saturating_sub(belegt).saturating_add(entleerbar).min(gesamt?))
    })();
    rest.or_else(|| {
        let summe = seiten("Pages free:")? + seiten("Pages inactive:")? + seiten("Pages speculative:")?;
        summe.checked_mul(seite)
    })
}

// ── Platte ──────────────────────────────────────────────────────────

#[cfg(unix)]
fn platte_unter(pfad: &Path) -> Option<Platte> {
    use std::os::unix::ffi::OsStrExt;

    // 📌 **Das Verzeichnis muss es geben.** `statvfs` auf einen Pfad,
    // den niemand angelegt hat, meldet einen Fehler, und das saehe aus
    // wie „kein Datentraeger" statt „noch kein Ordner".
    let ort = erster_vorhandener(pfad)?;
    let c = std::ffi::CString::new(ort.as_os_str().as_bytes()).ok()?;
    // SICHERHEIT: `statvfs` ist eine reine C-Struktur aus Zahlen, fuer die
    // lauter Nullen ein gueltiger Wert sind.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SICHERHEIT: `c` ist nullterminiert, `s` ist ausgenullt und gross
    // genug.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    // `f_frsize` ist die Blockgroesse der Zahlen darunter. `f_bavail`
    // ist das, was ein gewoehnlicher Nutzer bekommt; `f_bfree` zaehlt
    // die Reserve des Systemverwalters mit und waere zu grosszuegig.
    let block = if s.f_frsize > 0 { s.f_frsize as u64 } else { s.f_bsize as u64 };
    Some(Platte {
        pfad: ort.display().to_string(),
        gesamt_bytes: (s.f_blocks as u64).saturating_mul(block),
        frei_bytes: (s.f_bavail as u64).saturating_mul(block),
    })
}

#[cfg(windows)]
fn platte_unter(pfad: &Path) -> Option<Platte> {
    let ort = erster_vorhandener(pfad)?;
    let aus = powershell(&format!(
        "$d=(Get-Item -LiteralPath '{}').PSDrive; \
         \"$($d.Used + $d.Free);$($d.Free)\"",
        ort.display()
    ))?;
    let (gesamt, frei) = aus.trim().split_once(';')?;
    Some(Platte {
        pfad: ort.display().to_string(),
        gesamt_bytes: gesamt.trim().parse().ok()?,
        frei_bytes: frei.trim().parse().ok()?,
    })
}

#[cfg(not(any(unix, windows)))]
fn platte_unter(_pfad: &Path) -> Option<Platte> {
    None
}

/// Der Pfad selbst, sonst sein naechster vorhandener Vorfahre.
///
/// ⚑ **Weil die Frage dem Datentraeger gilt und nicht dem Ordner.** Wer
/// einen Ausgabeordner einstellt, den es noch nicht gibt, will trotzdem
/// wissen, wieviel Platz dort waere.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
fn erster_vorhandener(pfad: &Path) -> Option<std::path::PathBuf> {
    let mut p = pfad.to_path_buf();
    loop {
        if p.exists() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

// ── Rechenwerke ─────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    // ⚑ **JSON und nicht die Menschenausgabe.** `system_profiler`
    // kann beides; aus einer Textausgabe fuer Menschen eine
    // Schnittstelle zu machen ist genau die Sorte Logik, die dieses
    // Projekt nicht baut.
    let Some(roh) = befehl("system_profiler", &["SPDisplaysDataType", "-json"]) else {
        return Vec::new();
    };
    let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else {
        return Vec::new();
    };
    let Some(liste) = d.get("SPDisplaysDataType").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    liste
        .iter()
        .filter_map(|g| {
            let name = g
                .get("sppci_model")
                .or_else(|| g.get("_name"))
                .and_then(|v| v.as_str())?
                .to_string();
            // ⚑ **Eingebaut heisst gemeinsamer Speicher.** Apple
            // Silicon hat kein eigenes VRAM, und `spdisplays_vram`
            // fehlt dort deshalb. Das ist kein fehlgeschlagener Scan,
            // sondern die Eigenschaft der Maschine.
            let eingebaut = g
                .get("sppci_bus")
                .and_then(|v| v.as_str())
                .is_some_and(|b| b.contains("builtin"));
            Some(Rechenwerk {
                kennung: kennung_aus(&name),
                name,
                speicher_bytes: g
                    .get("spdisplays_vram_shared")
                    .or_else(|| g.get("spdisplays_vram"))
                    .and_then(|v| v.as_str())
                    .and_then(mib_aus_text),
                // ⚠️ Fuer eine Steckkarte in einem aelteren Mac nennt
                // `system_profiler` nichts Freies; das bleibt `None`.
                frei_bytes: None,
                gemeinsamer_speicher: eingebaut,
                kerne: g
                    .get("sppci_cores")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
                // ⚑ **Nur das eingebaute Werk.** Der Metal-Rechenweg ist
                // fuer die GPU von Apple-Silizium geschrieben, und die
                // Selbstpruefung laeuft gegen das Geraet, das
                // `MTLCreateSystemDefaultDevice` herausgibt.
                rueckseite: eingebaut.then_some(Rueckseite::Metal),
                pci: None,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    // ⚑ **Ueber `sysfs` und nicht ueber ein Werkzeug des Herstellers.**
    // `/sys/class/drm/card*/device` gibt es fuer jede Karte, die einen
    // Treiber hat, gleich von welchem Hersteller; `nvidia-smi` gibt es
    // nur mit dem einen und wird nur fuer das Freie gefragt.
    let Ok(eintraege) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut aus = Vec::new();
    let mut namen: Vec<String> = eintraege
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        // `card0`, aber nicht `card0-DP-1`: Das zweite ist ein Anschluss.
        .filter(|n| n.starts_with("card") && !n.contains('-'))
        .collect();
    namen.sort();

    // ⚑ **Einmal gefragt, nicht je Karte**, und nur, wenn es eine
    // NVIDIA-Karte gibt: `nvidia-smi` braucht eine Sekunde.
    let mut nvidia: Option<Vec<NvidiaZeile>> = None;
    // ⚑ **Die Namensliste der PCI-Geraete, einmal gelesen**, und nur, wenn
    // eine Karte ihren Namen nicht selbst nennt.
    let mut namensliste: Option<Option<String>> = None;

    for karte in namen {
        let geraet = std::path::Path::new("/sys/class/drm").join(&karte).join("device");
        let lies = |datei: &str| {
            std::fs::read_to_string(geraet.join(datei)).ok().map(|s| s.trim().to_string())
        };
        let treiber = std::fs::read_link(geraet.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        let hersteller = lies("vendor").unwrap_or_default();
        let geraetenummer = lies("device").unwrap_or_default();
        let roh = match &treiber {
            Some(t) => format!("{t} {hersteller}:{geraetenummer}"),
            None => format!("{hersteller}:{geraetenummer}"),
        };
        let rueckseite = rueckseite_aus(Some(&hersteller), &roh);
        // ⚑ `device` zeigt auf das PCI-Geraet, und sein Ordnername ist
        // die Adresse (`0000:3b:00.0`).
        let pci = std::fs::read_link(&geraet)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .filter(|n| n.contains(':'));
        let gesamt: Option<u64> = lies("mem_info_vram_total").and_then(|s| s.parse().ok());
        // ⚑ **Das Freie kommt, woher es kommt:** AMD schreibt das Belegte
        // in `sysfs`, NVIDIA nennt das Freie nur ueber sein Werkzeug.
        let frei = match rueckseite {
            Some(Rueckseite::Rocm) => {
                let belegt: Option<u64> = lies("mem_info_vram_used").and_then(|s| s.parse().ok());
                gesamt.zip(belegt).map(|(g, b)| g.saturating_sub(b))
            }
            Some(Rueckseite::Cuda) => nvidia_zeile(&mut nvidia, pci.as_deref()).map(|z| z.frei_bytes),
            _ => None,
        };
        // 📌 **Bis zum 2026-10-17 hiess eine Karte hier „nvidia
        // 0x10de:0x2684"** (Treiber und Nummern), und so stand sie im
        // Reiter „Knoten" und in der Freigabemaske. Wer zwei Karten hat,
        // konnte daran nicht sehen, welche welche ist. ⚑ **Der Name kommt
        // jetzt, woher er kommt**: von der Karte selbst (`product_name`,
        // AMD), von `nvidia-smi` (NVIDIA, ohnehin gefragt) oder aus der
        // Namensliste des Systems; erst ohne alle drei stehen die Nummern da.
        let name = lies("product_name")
            .filter(|n| !n.is_empty())
            .or_else(|| match rueckseite {
                Some(Rueckseite::Cuda) => nvidia_zeile(&mut nvidia, pci.as_deref()).map(|z| z.name.clone()),
                _ => None,
            })
            .or_else(|| {
                let liste = namensliste.get_or_insert_with(|| {
                    PCI_NAMENSLISTEN.iter().find_map(|p| std::fs::read_to_string(p).ok())
                });
                liste.as_deref().and_then(|l| name_aus_pci_liste(l, &hersteller, &geraetenummer))
            })
            .unwrap_or(roh);
        aus.push(Rechenwerk {
            // ⚑ **Die Kennung haengt an der PCI-Adresse**, solange es eine
            // gibt. 📌 Bis zum 2026-10-17 hing sie an `card0`, `card1`, und
            // diese Nummern vergibt der Kern beim Start in der Reihenfolge,
            // in der die Treiber laden: Nach einem Neustart konnte die
            // Freigabe der einen Karte bei der anderen stehen. Die Adresse
            // ist der Steckplatz und bleibt.
            kennung: match &pci {
                Some(p) => kennung_aus(&format!("pci-{p}")),
                None => kennung_aus(&format!("{karte}-{hersteller}-{geraetenummer}")),
            },
            name,
            speicher_bytes: gesamt,
            frei_bytes: frei,
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite,
            pci,
        });
    }
    aus
}

#[cfg(target_os = "windows")]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    let Some(roh) = powershell(
        "Get-CimInstance Win32_VideoController | \
         Select-Object Name,AdapterRAM,PNPDeviceID | ConvertTo-Json -Compress",
    ) else {
        return Vec::new();
    };
    let Ok(d) = serde_json::from_str::<serde_json::Value>(&roh) else {
        return Vec::new();
    };
    // 📌 **Ein einzelnes Ergebnis ist kein Feld.** `ConvertTo-Json`
    // gibt bei genau einer Karte ein Objekt zurueck und erst ab zwei
    // ein Feld.
    let liste: Vec<&serde_json::Value> = match d.as_array() {
        Some(f) => f.iter().collect(),
        None => vec![&d],
    };
    liste
        .into_iter()
        .filter_map(|g| {
            let name = g.get("Name").and_then(|v| v.as_str())?.to_string();
            let kennung = g.get("PNPDeviceID").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
            // ⚑ **Die Herstellerkennung steht in der PNP-Kennung**, als
            // `PCI\VEN_10DE&DEV_2684&…`.
            let rueckseite = rueckseite_aus(hersteller_aus_pnp(&kennung).as_deref(), &name);
            Some(Rechenwerk {
                // 📌 `AdapterRAM` ist ein `uint32` und laeuft ab 4 GiB
                // ueber; ein abgeschnittener Wert ist deshalb **kein** Wert.
                speicher_bytes: g
                    .get("AdapterRAM")
                    .and_then(|v| v.as_u64())
                    .filter(|b| *b > 0 && *b < u32::MAX as u64),
                // ⚠️ Ohne PCI-Adresse laesst sich das Freie aus
                // `nvidia-smi` keiner Zeile hier zuordnen.
                frei_bytes: None,
                kennung: kennung_aus(&kennung),
                name,
                gemeinsamer_speicher: false,
                kerne: None,
                rueckseite,
                pci: None,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    Vec::new()
}

/// Wo Linux die Namensliste der PCI-Geraete ablegt (`pci.ids`), je nach
/// Verteilung. ⚑ **Vom System vorgegebene Orte**, keine Pfade dieses
/// Repositoriums; fehlen alle, bleibt es bei den Nummern.
#[cfg(target_os = "linux")]
const PCI_NAMENSLISTEN: [&str; 3] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids", "/usr/share/pci.ids"];

/// Eine Zeile aus `nvidia-smi`: Adresse, Freies, Name.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct NvidiaZeile {
    pci: String,
    frei_bytes: u64,
    name: String,
}

/// **Die Zeile zu einer Karte**, `nvidia-smi` beim ersten Bedarf gefragt.
#[cfg(target_os = "linux")]
fn nvidia_zeile<'a>(liste: &'a mut Option<Vec<NvidiaZeile>>, pci: Option<&str>) -> Option<&'a NvidiaZeile> {
    let liste = liste.get_or_insert_with(|| {
        befehl(
            "nvidia-smi",
            &["--query-gpu=pci.bus_id,memory.free,name", "--format=csv,noheader,nounits"],
        )
        .map(|t| nvidia_smi_lesen(&t))
        .unwrap_or_default()
    });
    let p = pci_einheitlich(pci?);
    liste.iter().find(|z| z.pci == p)
}

/// **Adresse, Freies und Name je Karte aus `nvidia-smi`**, Zeilen der Form
/// `00000000:3B:00.0, 23104, NVIDIA GeForce RTX 4090` (MiB).
///
/// ⚑ **Eine Zeile, die nicht passt, faellt weg**, statt eine Karte mit
/// geratenem Wert zu liefern. Der Name steht zuletzt, damit ein Komma
/// darin nichts verschiebt.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn nvidia_smi_lesen(text: &str) -> Vec<NvidiaZeile> {
    text.lines()
        .filter_map(|z| {
            let mut teile = z.splitn(3, ',');
            let pci = pci_einheitlich(teile.next()?.trim());
            let mib: u64 = teile.next()?.trim().parse().ok()?;
            let name = teile.next()?.trim().to_string();
            Some(NvidiaZeile { pci, frei_bytes: mib.checked_mul(MIB)?, name })
        })
        .collect()
}

/// **Der Name eines Geraets aus der Namensliste `pci.ids`**: Hersteller
/// und Geraet als Nummern (`0x10de`, `0x2684`), heraus kommt etwa
/// „NVIDIA GeForce RTX 4090".
///
/// ⚑ **Der Verkaufsname, wo die Liste einen nennt.** Sie schreibt
/// `AD102 [GeForce RTX 4090]`: den Chip und in Klammern, was auf der
/// Schachtel steht. Wer seine Karten unterscheiden will, kennt das Zweite.
/// ⚑ Die drei Hersteller, fuer die es Rechenwege gibt oder geben koennte,
/// bekommen ihren kurzen Namen; die Liste nennt sie mit Rechtsform.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn name_aus_pci_liste(liste: &str, hersteller: &str, geraet: &str) -> Option<String> {
    let nummer = |s: &str| s.trim().trim_start_matches("0x").to_ascii_lowercase();
    let (h, g) = (nummer(hersteller), nummer(geraet));
    let mut hersteller_name: Option<&str> = None;
    for z in liste.lines() {
        if z.starts_with('#') || z.trim().is_empty() {
            continue;
        }
        let eingerueckt = z.starts_with('\t');
        if !eingerueckt {
            if hersteller_name.is_some() {
                // Der naechste Hersteller beginnt: Das Geraet steht nicht da.
                break;
            }
            if let Some((nr, name)) = z.split_once("  ") {
                if nr.eq_ignore_ascii_case(&h) {
                    hersteller_name = Some(name.trim());
                }
            }
            continue;
        }
        let Some(hn) = hersteller_name else { continue };
        if z.starts_with("\t\t") {
            continue;
        }
        let Some((nr, name)) = z[1..].split_once("  ") else { continue };
        if !nr.eq_ignore_ascii_case(&g) {
            continue;
        }
        let name = name.trim();
        let verkauf = match (name.find('['), name.rfind(']')) {
            (Some(a), Some(b)) if a < b => &name[a + 1..b],
            _ => name,
        };
        let kurz = match h.as_str() {
            "10de" => "NVIDIA",
            "1002" | "1022" => "AMD",
            "8086" => "Intel",
            _ => hn,
        };
        return Some(format!("{kurz} {verkauf}"));
    }
    None
}

/// **Macht gleiche Kennungen verschieden**: die zweite bekommt `-2`, die
/// dritte `-3`.
///
/// ⚑ **Zwei gleiche Karten sind zwei Karten.** 📌 Bis zum 2026-10-17 trugen
/// zwei baugleiche Karten unter macOS (ein Mac Pro mit zwei Radeon) dieselbe
/// Kennung, denn sie kommt dort aus dem Namen; ein Schalter schaltete dann
/// beide. ⚠️ Die Nummer folgt der Reihenfolge, in der das System die Karten
/// nennt. Die bleibt, solange niemand umsteckt; unter Linux und Windows
/// kommt die Kennung ohnehin aus dem Steckplatz.
fn eindeutig_machen(werke: &mut [Rechenwerk]) {
    let mut gesehen: Vec<String> = Vec::new();
    for w in werke.iter_mut() {
        let grund = w.kennung.clone();
        let mut n = 1;
        while gesehen.contains(&w.kennung) {
            n += 1;
            w.kennung = format!("{grund}-{n}");
        }
        gesehen.push(w.kennung.clone());
    }
}

/// **Eine PCI-Adresse in der Schreibweise von `sysfs`**: klein, vier
/// Stellen Domaene. `nvidia-smi` schreibt `00000000:3B:00.0`, `sysfs`
/// `0000:3b:00.0`; verglichen wird erst nach dieser Angleichung.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn pci_einheitlich(roh: &str) -> String {
    let k = roh.trim().to_ascii_lowercase();
    match k.split_once(':') {
        Some((domaene, rest)) if domaene.len() > 4 => format!("{}:{rest}", &domaene[domaene.len() - 4..]),
        _ => k,
    }
}

/// Die Herstellerkennung aus einer PNP-Geraetekennung: `VEN_10DE` → `10de`.
// ⚑ **Nur Windows ruft sie**, denn eine PNP-Kennung gibt es nur dort.
// Geprueft wird sie trotzdem ueberall.
//
// 📌 **Eine `cfg`-Bedingung, die das falsche System nennt, faellt auf der
// Maschine, auf der man sie schreibt, nie auf** (die CI unter Linux brach
// einmal genau daran ab).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn hersteller_aus_pnp(pnp: &str) -> Option<String> {
    let k = pnp.to_ascii_lowercase();
    let ab = k.find("ven_")? + 4;
    let s: String = k[ab..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    (!s.is_empty()).then_some(s)
}

/// **Welcher Rechenweg dieses Geraet bedienen wuerde.**
///
/// ⚑ **Die Herstellerkennung zaehlt vor dem Namen.** Sie steht im
/// Geraet; der Name ist eine Zeichenkette, die ein Treiber setzt und ein
/// Hersteller aendert.
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
fn rueckseite_aus(hersteller: Option<&str>, name: &str) -> Option<Rueckseite> {
    if let Some(h) = hersteller {
        match h.trim().trim_start_matches("0x").to_ascii_lowercase().as_str() {
            "10de" => return Some(Rueckseite::Cuda),
            "1002" | "1022" => return Some(Rueckseite::Rocm),
            _ => {}
        }
    }
    let n = name.to_ascii_lowercase();
    if n.contains("nvidia") || n.contains("nouveau") {
        Some(Rueckseite::Cuda)
    } else if n.contains("amdgpu") || n.contains("radeon") || n.contains("amd ") {
        Some(Rueckseite::Rocm)
    } else {
        // ⚑ **Kein Rueckfall auf „irgendetwas".** Eine eingebaute
        // Intel-Grafik bekommt hier `None`: Es gibt keinen Rechenweg
        // fuer sie.
        None
    }
}

/// Eine Kennung, die sich als Schluessel in der Ablage eignet.
///
/// ⚑ **Aus dem Namen hergeleitet und nicht durchnummeriert.** Der
/// Listenplatz aendert sich, sobald jemand eine zweite Karte einbaut.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux", target_os = "windows")), allow(dead_code))]
fn kennung_aus(roh: &str) -> String {
    roh.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// `"8 GB"` oder `"1536 MB"` in Bytes.
#[cfg(target_os = "macos")]
fn mib_aus_text(s: &str) -> Option<u64> {
    let s = s.trim();
    let (zahl, rest) = s.split_at(s.find(|c: char| !c.is_ascii_digit())?);
    let n: u64 = zahl.parse().ok()?;
    let r = rest.trim().to_ascii_uppercase();
    if r.starts_with("GB") {
        Some(n * GIB)
    } else if r.starts_with("MB") {
        Some(n * MIB)
    } else {
        None
    }
}

/// Ein Befehl, dessen Ausgabe gelesen wird; `None` bei jedem Fehlschlag,
/// auch wenn es das Programm nicht gibt.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn befehl(programm: &str, argumente: &[&str]) -> Option<String> {
    let aus = std::process::Command::new(programm).args(argumente).output().ok()?;
    aus.status.success().then(|| String::from_utf8_lossy(&aus.stdout).to_string())
}

#[cfg(target_os = "windows")]
fn powershell(skript: &str) -> Option<String> {
    befehl("powershell", &["-NoProfile", "-NonInteractive", "-Command", skript])
}

#[cfg(test)]
mod proben {
    use super::*;

    fn werk(kennung: &str, name: &str, r: Option<Rueckseite>) -> Rechenwerk {
        Rechenwerk {
            kennung: kennung.into(),
            name: name.into(),
            speicher_bytes: Some(16 * GIB),
            frei_bytes: None,
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite: r,
            pci: None,
        }
    }

    /// ⚑ **Die Beschriftung nennt den Rechenweg**, sonst ist bei zwei
    /// Karten nicht zu sehen, welche wovon bedient wird.
    #[test]
    fn die_beschriftung_nennt_den_rechenweg() {
        assert_eq!(werk("a", "Apple M5 Pro", Some(Rueckseite::Metal)).beschriftung(), "Apple M5 Pro (Metal)");
        assert_eq!(
            werk("b", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)).beschriftung(),
            "NVIDIA GeForce RTX 4090 (CUDA)"
        );
        // Ohne Rechenweg bleibt der Name allein: Eine leere Klammer
        // saehe aus wie ein verlorener Wert.
        assert_eq!(werk("c", "Intel UHD Graphics", None).beschriftung(), "Intel UHD Graphics");
    }

    /// ⚑ **Die Herstellerkennung schlaegt den Namen.**
    #[test]
    fn die_herstellerkennung_entscheidet_und_nicht_der_name() {
        // Linux: `vendor` aus sysfs.
        assert_eq!(rueckseite_aus(Some("0x10de"), "nvidia 0x10de:0x2684"), Some(Rueckseite::Cuda));
        assert_eq!(rueckseite_aus(Some("0x1002"), "amdgpu 0x1002:0x744c"), Some(Rueckseite::Rocm));
        // Windows: aus der PNP-Kennung gezogen.
        assert_eq!(hersteller_aus_pnp("PCI\\VEN_10DE&DEV_2684&SUBSYS_0000"), Some("10de".into()));
        assert_eq!(hersteller_aus_pnp("PCI\\VEN_1002&DEV_744C"), Some("1002".into()));
        assert_eq!(hersteller_aus_pnp("ohne kennung"), None);
        // Ohne Kennung traegt der Name.
        assert_eq!(rueckseite_aus(None, "NVIDIA GeForce RTX 4090"), Some(Rueckseite::Cuda));
        assert_eq!(rueckseite_aus(None, "AMD Radeon RX 7900 XTX"), Some(Rueckseite::Rocm));
        // ⚑ **Und die Kennung gewinnt gegen einen irrefuehrenden Namen.**
        assert_eq!(rueckseite_aus(Some("0x10de"), "AMD Radeon"), Some(Rueckseite::Cuda));
        // Wofuer es keinen Rechenweg gibt, bekommt keinen geraten.
        assert_eq!(rueckseite_aus(Some("0x8086"), "Intel UHD Graphics 770"), None);
        assert_eq!(rueckseite_aus(None, "VMware SVGA 3D"), None);
    }

    /// ⚑ **Die Kennung haengt am Geraet und nicht am Listenplatz.**
    #[test]
    fn die_kennung_ueberlebt_eine_zweite_karte() {
        assert_eq!(kennung_aus("Apple M5 Pro"), "apple-m5-pro");
        assert_eq!(kennung_aus("NVIDIA GeForce RTX 4090"), "nvidia-geforce-rtx-4090");
        assert_eq!(kennung_aus("PCI\\VEN_10DE&DEV_2684"), "pci-ven-10de-dev-2684");
        assert_ne!(kennung_aus("Karte A"), kennung_aus("Karte B"));
    }

    /// ⚑ **Verfuegbar ist der Rest nach festem, komprimiertem und anonymem
    /// Speicher**; ohne dessen Felder gilt der alte Weg, ohne Seitengroesse
    /// gar nichts.
    #[test]
    fn vm_stat_wird_als_rest_gelesen() {
        let text = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                    Pages free:                               10.\n\
                    Pages active:                            900.\n\
                    Pages inactive:                           20.\n\
                    Pages speculative:                         2.\n\
                    Pages wired down:                        500.\n\
                    Pages purgeable:                           4.\n\
                    File-backed pages:                       800.\n\
                    Anonymous pages:                         100.\n\
                    Pages occupied by compressor:            300.\n";
        let s = 16384;
        // 2000 Seiten gesamt: 2000 - 500 - 300 - 100 + 4 = 1104.
        assert_eq!(frei_aus_vm_stat(text, Some(2000 * s)), Some(1104 * s));
        // Ohne Gesamtspeicher oder ohne ein Feld: frei, inaktiv, spekulativ.
        assert_eq!(frei_aus_vm_stat(text, None), Some(32 * s));
        assert_eq!(frei_aus_vm_stat(&text.replacen("Anonymous pages:", "Anon:", 1), Some(2000 * s)), Some(32 * s));
        // Mehr als der Gesamtspeicher wird es nie.
        assert_eq!(frei_aus_vm_stat(&text.replacen("Pages purgeable:                           4.", "Pages purgeable: 99999.", 1), Some(2000 * s)), Some(2000 * s));
        // Ohne Seitengroesse ist keine Zahl zu deuten.
        assert_eq!(frei_aus_vm_stat(&text.replacen("(page size of 16384 bytes)", "", 1), Some(2000 * s)), None);
    }

    /// ⚑ **Ein Feld aus `/proc/meminfo` gilt nur mit Einheit.**
    #[test]
    fn meminfo_liest_nur_felder_mit_einheit() {
        let text = "MemTotal:       16334204 kB\nMemFree:          812000 kB\nMemAvailable:    9000000 kB\nHugePages_Total:       0\n";
        assert_eq!(meminfo_feld(text, "MemTotal:"), Some(16334204 * 1024));
        assert_eq!(meminfo_feld(text, "MemAvailable:"), Some(9000000 * 1024));
        assert_eq!(meminfo_feld(text, "HugePages_Total:"), None, "eine Zahl ohne Einheit ist kein Speicher");
        assert_eq!(meminfo_feld(text, "Fehlt:"), None);
    }

    /// ⚑ **`nvidia-smi` und `sysfs` schreiben dieselbe Adresse
    /// verschieden**, und verglichen wird erst nach der Angleichung.
    #[test]
    fn das_freie_einer_nvidia_karte_findet_ihre_adresse() {
        let text = "00000000:3B:00.0, 23104, NVIDIA GeForce RTX 4090\n00000000:5E:00.0, 100, NVIDIA RTX A6000, Rev. 2\nkaputt\n";
        let liste = nvidia_smi_lesen(text);
        let kurz: Vec<(String, u64, String)> = liste.into_iter().map(|z| (z.pci, z.frei_bytes, z.name)).collect();
        assert_eq!(
            kurz,
            vec![
                ("0000:3b:00.0".to_string(), 23104 * MIB, "NVIDIA GeForce RTX 4090".to_string()),
                ("0000:5e:00.0".to_string(), 100 * MIB, "NVIDIA RTX A6000, Rev. 2".to_string()),
            ],
            "der Name steht zuletzt, auch mit Komma"
        );
        assert_eq!(pci_einheitlich("0000:3b:00.0"), "0000:3b:00.0", "die Schreibweise von sysfs bleibt");
        assert_eq!(pci_einheitlich("00000000:3B:00.0"), "0000:3b:00.0");
    }

    /// ⚑ **Der Verkaufsname aus der Namensliste**, mit kurzem Hersteller;
    /// ein Geraet, das die Liste nicht kennt, bekommt keinen geratenen.
    #[test]
    fn die_namensliste_nennt_den_verkaufsnamen() {
        let liste = "# Kommentar\n\
                     1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\
                     \t744c  Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]\n\
                     \t\t1002 0e3b  Radeon RX 7900 XTX\n\
                     10de  NVIDIA Corporation\n\
                     \t2684  AD102 [GeForce RTX 4090]\n\
                     \t2230  GA102GL [RTX A6000]\n\
                     1af4  Red Hat, Inc.\n\
                     \t1050  Virtio 1.0 GPU\n";
        assert_eq!(name_aus_pci_liste(liste, "0x10de", "0x2684").as_deref(), Some("NVIDIA GeForce RTX 4090"));
        assert_eq!(
            name_aus_pci_liste(liste, "0x1002", "0x744C").as_deref(),
            Some("AMD Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M")
        );
        assert_eq!(name_aus_pci_liste(liste, "1af4", "1050").as_deref(), Some("Red Hat, Inc. Virtio 1.0 GPU"));
        assert_eq!(name_aus_pci_liste(liste, "0x10de", "0x9999"), None, "ein unbekanntes Geraet");
        assert_eq!(name_aus_pci_liste(liste, "0x1234", "0x2684"), None, "ein unbekannter Hersteller");
    }

    /// ⚑ **Zwei gleiche Karten bekommen zwei Kennungen**, und die erste
    /// behaelt ihre.
    #[test]
    fn zwei_gleiche_karten_bekommen_zwei_kennungen() {
        let mut w = vec![
            werk("amd-radeon-pro-w6800x", "AMD Radeon Pro W6800X", None),
            werk("apple-m5-pro", "Apple M5 Pro", None),
            werk("amd-radeon-pro-w6800x", "AMD Radeon Pro W6800X", None),
            werk("amd-radeon-pro-w6800x", "AMD Radeon Pro W6800X", None),
        ];
        eindeutig_machen(&mut w);
        let k: Vec<&str> = w.iter().map(|w| w.kennung.as_str()).collect();
        assert_eq!(k, ["amd-radeon-pro-w6800x", "apple-m5-pro", "amd-radeon-pro-w6800x-2", "amd-radeon-pro-w6800x-3"]);
    }

    /// ⚑ **Unter Linux haengt die Kennung am Steckplatz**: Dieselbe Adresse
    /// ergibt dieselbe Kennung, gleich unter welcher `cardN` sie steht.
    #[test]
    fn die_kennung_aus_der_adresse_ist_stabil() {
        assert_eq!(kennung_aus(&format!("pci-{}", "0000:3b:00.0")), "pci-0000-3b-00-0");
        assert_ne!(kennung_aus("pci-0000:3b:00.0"), kennung_aus("pci-0000:5e:00.0"));
    }

}
