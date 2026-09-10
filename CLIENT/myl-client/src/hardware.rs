//! Was diese Maschine hergibt, und was ihr Besitzer davon freigibt.
//!
//! # ⚑ Warum das eine Erklaerung ist und keine Einstellung
//!
//! **Ein Schieber, der nur oertlich etwas abschaltet, ist eine
//! Einstellung. Einer, der dem Netz etwas zusagt, ist ein Versprechen**,
//! und aus der Summe dieser Versprechen leitet das Netz spaeter sein
//! Budget ab. Deshalb steht hier nicht „wieviel darf der Rechenpfad
//! nehmen", sondern „wieviel gibt der Besitzer her": Die zweite Frage
//! traegt beide Haelften, die erste nur die oertliche.
//!
//! ⚑ **Und deshalb ist die Form schon die der Erklaerung.** Je
//! Betriebsmittel ein Regler mit Hoechstwert, freigegebenem Wert und
//! Einheit. Was heute oertlich wirkt, ist eine Teilmenge davon; was
//! noch nicht wirkt, sagt in [`Regler::sperrgrund`] **warum**, und zwar
//! mit dem, was dafuer geschrieben werden muss.
//!
//! # ⛑ Was ein Scan nicht darf
//!
//! **Raten.** Jede Groesse, die sich auf dieser Maschine nicht
//! ermitteln laesst, ist `None` und wird als „nicht erkannt" angezeigt.
//! Eine erfundene Zahl in einem Regler waere schlimmer als eine fehlende:
//! Der Nutzer stellte dann einen Anteil von etwas ein, das es so nicht
//! gibt.

use std::path::Path;

/// Alles, was der Scan ueber diese Maschine herausbekommen hat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Hardware {
    /// Logische Kerne, die dieser Prozess sehen darf.
    pub kerne: usize,
    /// Arbeitsspeicher der Maschine, `None` wenn nicht ermittelbar.
    pub speicher_bytes: Option<u64>,
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
    /// Teilt sich das Rechenwerk den Arbeitsspeicher mit der CPU?
    pub gemeinsamer_speicher: bool,
    /// Kerne des Rechenwerks, soweit die Maschine sie nennt.
    pub kerne: Option<u32>,
}

/// Womit ein Regler misst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Einheit {
    /// Ganze Kerne.
    Kerne,
    /// Gibibyte.
    Gib,
    /// Anteil in Prozent, fuer Groessen ohne verlaessliche Absolutzahl.
    Prozent,
}

/// Ein Betriebsmittel, das der Besitzer freigeben kann.
///
/// ⚑ **Der Hoechstwert kommt aus dem Scan und nicht aus einer
/// Vorgabe.** Ein Regler, dessen Ende jemand geraten hat, laesst
/// entweder etwas verschenken oder etwas versprechen, das die Maschine
/// nicht hat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Regler {
    /// Der Name, unter dem die Freigabe abgelegt wird.
    pub name: String,
    pub titel: String,
    pub einheit: Einheit,
    /// Was die Maschine hergibt. `None` heisst: nicht erkannt, und dann
    /// laesst sich nichts freigeben, was ein Anteil von etwas waere.
    pub hoechstens: Option<u64>,
    /// Was heute freigegeben ist.
    pub wert: Option<u64>,
    /// Was der Regler bewirkt, in einem Satz.
    pub hinweis: String,
    /// ⚑ **Warum er nichts bewirkt, und was dafuer fehlt.** `None`
    /// heisst: Er wirkt. Ein gesperrter Regler ohne diesen Satz waere
    /// eine Zierde, und Zierde in einer Freigabemaske beruhigt ueber
    /// eine Stelle, die nichts leistet.
    pub sperrgrund: Option<String>,
}

impl Hardware {
    /// Fragt die Maschine ab.
    ///
    /// ⚑ **`datenort` ist das Verzeichnis, auf das sich die
    /// Plattenzahlen beziehen.** Ein Rechner hat mehrere Datentraeger,
    /// und die Frage „wieviel Platz ist da" hat ohne einen Ort keine
    /// Antwort.
    pub fn erheben(datenort: &Path) -> Self {
        Self {
            kerne: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            speicher_bytes: speicher_der_maschine(),
            platte: platte_unter(datenort),
            rechenwerke: rechenwerke_suchen(),
        }
    }
}

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
    let zeile = text.lines().find(|z| z.starts_with("MemTotal:"))?;
    let kib: u64 = zeile.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(target_os = "windows")]
fn speicher_der_maschine() -> Option<u64> {
    // ⚑ Ueber die Systemabfrage und nicht ueber eine Bibliothek: Der
    // Scan laeuft einmal beim Oeffnen der Seite, und ein Unterprozess
    // kostet dort nichts. Die Ausgabe ist JSON und keine Prosa.
    let aus = powershell(
        "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
    )?;
    aus.trim().parse().ok()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn speicher_der_maschine() -> Option<u64> {
    None
}

// ── Platte ──────────────────────────────────────────────────────────

#[cfg(unix)]
fn platte_unter(pfad: &Path) -> Option<Platte> {
    use std::os::unix::ffi::OsStrExt;

    // ⛑ **Das Verzeichnis muss es geben.** `statvfs` auf einen Pfad,
    // den niemand angelegt hat, meldet einen Fehler, und das saehe aus
    // wie „kein Datentraeger" statt „noch kein Ordner".
    let ort = erster_vorhandener(pfad)?;
    let c = std::ffi::CString::new(ort.as_os_str().as_bytes()).ok()?;
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

/// Der Pfad selbst, sonst sein naechster vorhandener Vorfahre.
///
/// ⚑ **Weil die Frage dem Datentraeger gilt und nicht dem Ordner.** Wer
/// einen Ausgabeordner einstellt, den es noch nicht gibt, will trotzdem
/// wissen, wieviel Platz dort waere.
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
            // sondern die Eigenschaft der Maschine, und beides
            // auseinanderzuhalten ist der Grund fuer das eigene Feld.
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
                gemeinsamer_speicher: eingebaut,
                kerne: g
                    .get("sppci_cores")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    // ⚑ **Ueber `sysfs` und nicht ueber ein Werkzeug des Herstellers.**
    // `/sys/class/drm/card*/device` gibt es fuer jede Karte, die einen
    // Treiber hat, gleich von welchem Hersteller; `nvidia-smi` gibt es
    // nur mit dem einen.
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

    for karte in namen {
        let geraet = std::path::Path::new("/sys/class/drm").join(&karte).join("device");
        let lies = |datei: &str| {
            std::fs::read_to_string(geraet.join(datei)).ok().map(|s| s.trim().to_string())
        };
        // Der Treibername sagt mehr als die reine Kennzahl; wo er
        // fehlt, bleibt die Hersteller-Geraete-Nummer als Name stehen.
        let treiber = std::fs::read_link(geraet.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        let hersteller = lies("vendor").unwrap_or_default();
        let geraetenummer = lies("device").unwrap_or_default();
        let name = match &treiber {
            Some(t) => format!("{t} {hersteller}:{geraetenummer}"),
            None => format!("{hersteller}:{geraetenummer}"),
        };
        aus.push(Rechenwerk {
            kennung: kennung_aus(&format!("{karte}-{hersteller}-{geraetenummer}")),
            name,
            speicher_bytes: lies("mem_info_vram_total").and_then(|s| s.parse().ok()),
            gemeinsamer_speicher: false,
            kerne: None,
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
    // ⛑ **Ein einzelnes Ergebnis ist kein Feld.** `ConvertTo-Json`
    // gibt bei genau einer Karte ein Objekt zurueck und erst ab zwei
    // ein Feld. Wer nur das Feld liest, findet auf jedem Rechner mit
    // einer Grafikkarte nichts.
    let liste: Vec<&serde_json::Value> = match d.as_array() {
        Some(f) => f.iter().collect(),
        None => vec![&d],
    };
    liste
        .into_iter()
        .filter_map(|g| {
            let name = g.get("Name").and_then(|v| v.as_str())?.to_string();
            let kennung = g
                .get("PNPDeviceID")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();
            Some(Rechenwerk {
                // ⛑ `AdapterRAM` ist ein `uint32` und laeuft ab 4 GiB
                // ueber; ein negativer oder abgeschnittener Wert ist
                // deshalb **kein** Wert und nicht etwa eine kleine Karte.
                speicher_bytes: g
                    .get("AdapterRAM")
                    .and_then(|v| v.as_u64())
                    .filter(|b| *b > 0 && *b < u32::MAX as u64),
                kennung: kennung_aus(&kennung),
                name,
                gemeinsamer_speicher: false,
                kerne: None,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    Vec::new()
}

/// Eine Kennung, die sich als Schluessel in der Ablage eignet.
///
/// ⚑ **Aus dem Namen hergeleitet und nicht durchnummeriert.** Der
/// Listenplatz aendert sich, sobald jemand eine zweite Karte einbaut,
/// und dann traegt eine Freigabe die Zahl einer anderen Karte.
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
        Some(n * 1024 * 1024 * 1024)
    } else if r.starts_with("MB") {
        Some(n * 1024 * 1024)
    } else {
        None
    }
}

/// Ein Befehl, dessen Ausgabe gelesen wird; `None` bei jedem Fehlschlag.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn befehl(programm: &str, argumente: &[&str]) -> Option<String> {
    let aus = std::process::Command::new(programm).args(argumente).output().ok()?;
    aus.status.success().then(|| String::from_utf8_lossy(&aus.stdout).to_string())
}

#[cfg(target_os = "windows")]
fn powershell(skript: &str) -> Option<String> {
    befehl(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", skript],
    )
}

/// Ein Gibibyte in Bytes.
pub const GIB: u64 = 1024 * 1024 * 1024;

/// ⚑ **Was fehlt, bevor ein Rechenwerk mitrechnen darf.**
///
/// Der Satz steht hier und nicht im Fenster, aus demselben Grund wie
/// die Beschriftungen der Felder: **Ein gesperrter Regler, der nicht
/// sagt warum, ist eine Zierde**, und Zierde in einer Freigabemaske
/// beruhigt ueber eine Stelle, die nichts leistet.
pub const RECHENWERK_GESPERRT: &str = "Noch kein Rechenweg über dieses Gerät. \
Die Rückseiten für NVIDIA und AMD reichen heute an die Referenzkernel weiter, \
gerechnet wird also auf der CPU. Es fehlen echte Ganzzahlkerne, INT8-GEMM über \
dp4a bei NVIDIA, i8mm bei ARM und VNNI bei x86, und ein Konformitätslauf auf \
genau dieser Architektur. Ohne ihn würde ein Miner mit abweichendem Kernel \
bestraft, ohne etwas falsch gemacht zu haben. Sobald beides steht, wirkt dieser \
Regler ohne weiteres Zutun.";

/// Derselbe Satz auf Englisch.
///
/// ⛑ **Er stand bis zum 2026-09-10 nur auf Deutsch da**, und damit trug
/// ein englisches Fenster an seiner laengsten Erklaerung einen deutschen
/// Absatz. **Uebersetzt wird, was ein Mensch liest**, und das gilt
/// besonders fuer den Satz, der erklaert, warum etwas nicht geht.
pub const RECHENWERK_GESPERRT_EN: &str = "No compute path over this device yet. \
The NVIDIA and AMD back ends currently hand through to the reference kernels, \
so the work happens on the CPU. What is missing are real integer kernels, \
INT8 GEMM via dp4a on NVIDIA, i8mm on ARM and VNNI on x86, and a conformance \
run on exactly this architecture. Without it a miner with a deviating kernel \
would be penalised without having done anything wrong. Once both exist, this \
slider takes effect with no further work.";

/// Warum ein Betriebsmittel ohne erkanntes Ende gesperrt ist.
pub const OHNE_ENDE: (&str, &str) = (
    "Nicht erkannt: Diese Maschine sagt nicht, wie viel sie hiervon hat, \
     und ein Regler ohne Ende ist keiner.",
    "Not detected: this machine does not say how much of this it has, \
     and a slider without an end is not a slider.",
);

impl Hardware {
    /// Die Regler dieser Maschine, mit dem, was heute freigegeben ist.
    ///
    /// ⚑ **Eine Quelle fuer die ganze Freigabemaske.** Die festen
    /// Betriebsmittel kommen aus [`crate::einstellungen::FELDER`], die
    /// Rechenwerke aus dem Scan, die Werte aus
    /// [`crate::einstellungen::Einstellungen::wert`]. Das Fenster
    /// zaehlt nichts davon selbst auf.
    pub fn regler(&self, e: &crate::einstellungen::Einstellungen) -> Vec<Regler> {
        use crate::einstellungen::{Feldwert, FELDER, RECHENWERK_PRAEFIX};

        let zahl = |w: Feldwert| match w {
            Feldwert::Zahl(n) => Some(n),
            _ => None,
        };

        // ⛑ **In der eingestellten Sprache, seit dem 2026-09-10.** Die
        // Beschriftungen kamen roh aus `FELDER`, also immer auf Deutsch,
        // und ein englisches Fenster trug in der Freigabemaske deutsche
        // Saetze. **Uebersetzt wird an einer Stelle, und diese hier
        // hatte sie nicht gefragt.**
        let sprache = e.oberflaeche.sprache;
        let mut aus = Vec::new();
        for f in FELDER.iter().filter(|f| f.freigabe) {
            let f = f.in_sprache(sprache);
            let (einheit, hoechstens) = self.masse(f.name);
            aus.push(Regler {
                name: f.name.to_string(),
                titel: f.titel.to_string(),
                einheit,
                hoechstens,
                wert: e.wert(f.name).ok().and_then(zahl),
                hinweis: f.hinweis.to_string(),
                // ⚑ **Ein Regler ohne Ende ist keiner.** Was die
                // Maschine nicht nennt, laesst sich nicht anteilig
                // hergeben, und eine geratene Obergrenze liesse den
                // Nutzer etwas zusagen, das es nicht gibt.
                sperrgrund: hoechstens
                    .is_none()
                    .then(|| sprache.waehlen(OHNE_ENDE.0, OHNE_ENDE.1).to_string()),
            });
        }

        for r in &self.rechenwerke {
            aus.push(Regler {
                name: format!("{RECHENWERK_PRAEFIX}{}", r.kennung),
                titel: r.name.clone(),
                einheit: Einheit::Prozent,
                hoechstens: Some(100),
                wert: e
                    .wert(&format!("{RECHENWERK_PRAEFIX}{}", r.kennung))
                    .ok()
                    .and_then(zahl),
                hinweis: r.beschreibung(self.speicher_bytes, sprache),
                sperrgrund: Some(
                    sprache
                        .waehlen(RECHENWERK_GESPERRT, RECHENWERK_GESPERRT_EN)
                        .to_string(),
                ),
            });
        }
        aus
    }

    /// Einheit und Hoechstwert eines festen Betriebsmittels.
    fn masse(&self, name: &str) -> (Einheit, Option<u64>) {
        match name {
            "kap.kerne" => (Einheit::Kerne, Some(self.kerne as u64)),
            "kap.speicher" => (Einheit::Gib, self.speicher_bytes.map(|b| b / GIB)),
            // ⚑ **Das Ende ist, was frei ist.** Die ganze Platte waere
            // gelogen, denn sie gehoert nicht Myelith; was schon belegt
            // ist, steht ohnehin nicht mehr zur Freigabe.
            "kap.platte" => (Einheit::Gib, self.platte.as_ref().map(|p| p.frei_bytes / GIB)),
            _ => (Einheit::Gib, None),
        }
    }
}

impl Rechenwerk {
    /// Ein Satz darueber, was dieses Rechenwerk ist.
    ///
    /// ⚑ **Er nennt den Bezug des Anteils**, und der ist bei den beiden
    /// Bauarten verschieden: Ein eingebautes Werk teilt sich den
    /// Arbeitsspeicher mit der CPU und hat gar keinen eigenen, ein
    /// eigenstaendiges nennt sein eigenes. **Ein Prozentsatz ohne
    /// Bezugsgroesse ist eine Zahl ohne Bedeutung.**
    pub fn beschreibung(
        &self,
        speicher_der_maschine: Option<u64>,
        sprache: crate::einstellungen::Sprache,
    ) -> String {
        let gib = |b: u64| format!("{:.0} GiB", b as f64 / GIB as f64);
        let kerne = match self.kerne {
            Some(k) => format!("{k} {}, ", sprache.waehlen("Kerne", "cores")),
            None => String::new(),
        };
        if self.gemeinsamer_speicher {
            let speicher = match speicher_der_maschine {
                Some(b) => format!(
                    "{} ({})",
                    sprache.waehlen("gemeinsamer Speicher mit der CPU", "memory shared with the CPU"),
                    gib(b)
                ),
                None => sprache
                    .waehlen("gemeinsamer Speicher mit der CPU", "memory shared with the CPU")
                    .to_string(),
            };
            sprache.waehlen_wert(
                format!("Eingebaut, {kerne}{speicher}. Der Anteil gilt der Rechenzeit dieses Werks."),
                format!("Integrated, {kerne}{speicher}. The share applies to this unit's compute time."),
            )
        } else {
            let speicher = match self.speicher_bytes {
                Some(b) => format!("{} {}", gib(b), sprache.waehlen("eigener Speicher", "of its own memory")),
                None => sprache
                    .waehlen("eigener Speicher nicht erkannt", "own memory not detected")
                    .to_string(),
            };
            sprache.waehlen_wert(
                format!(
                    "Eigenständig, {kerne}{speicher}. Der Anteil gilt Rechenzeit und Speicher \
                     dieses Werks."
                ),
                format!(
                    "Discrete, {kerne}{speicher}. The share applies to this unit's compute time \
                     and memory."
                ),
            )
        }
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::einstellungen::Einstellungen;

    /// ⚑ **Der Scan raet nicht.** Was er nicht ermitteln kann, ist
    /// `None`, und daraus wird ein gesperrter Regler mit Begruendung
    /// statt eines Reglers mit erfundenem Ende.
    #[test]
    fn ein_regler_ohne_ende_ist_gesperrt() {
        let leer = Hardware {
            kerne: 4,
            speicher_bytes: None,
            platte: None,
            rechenwerke: Vec::new(),
        };
        let r = leer.regler(&Einstellungen::default());
        let kerne = r.iter().find(|r| r.name == "kap.kerne").expect("Kerne");
        assert_eq!(kerne.hoechstens, Some(4));
        assert!(kerne.sperrgrund.is_none(), "der Kernregler ist gesperrt, obwohl er wirkt");

        for name in ["kap.speicher", "kap.platte"] {
            let g = r.iter().find(|r| r.name == name).expect(name);
            assert!(g.hoechstens.is_none());
            assert!(
                g.sperrgrund.is_some(),
                "{name} hat kein Ende und ist trotzdem nicht gesperrt"
            );
        }
    }

    /// ⚑ **Jedes gefundene Rechenwerk bekommt einen eigenen Regler**,
    /// und jeder sagt, was fehlt.
    #[test]
    fn jedes_rechenwerk_bekommt_einen_gesperrten_regler_mit_grund() {
        let h = Hardware {
            kerne: 8,
            speicher_bytes: Some(32 * GIB),
            platte: None,
            rechenwerke: vec![
                Rechenwerk {
                    kennung: "eins".into(),
                    name: "Karte eins".into(),
                    speicher_bytes: Some(16 * GIB),
                    gemeinsamer_speicher: false,
                    kerne: None,
                },
                Rechenwerk {
                    kennung: "zwei".into(),
                    name: "Karte zwei".into(),
                    speicher_bytes: None,
                    gemeinsamer_speicher: true,
                    kerne: Some(16),
                },
            ],
        };
        let mut e = Einstellungen::default();
        e.setzen("kap.rechenwerk.zwei", "60").expect("setzen");

        let r = h.regler(&e);
        let werke: Vec<_> = r.iter().filter(|r| r.einheit == Einheit::Prozent).collect();
        assert_eq!(werke.len(), 2, "nicht jede Karte hat einen Regler");
        for w in &werke {
            let grund = w.sperrgrund.as_deref().expect("kein Sperrgrund");
            // ⚑ Der Grund muss sagen, **was fehlt**, nicht nur dass
            // etwas fehlt. Ein „noch nicht verfuegbar" liesse den
            // Leser genau so klug zurueck wie zuvor.
            assert!(
                grund.contains("Konformitätslauf") && grund.contains("Ganzzahlkerne"),
                "der Sperrgrund sagt nicht, was zu bauen ist: {grund}"
            );
            assert_eq!(w.hoechstens, Some(100));
        }
        assert_eq!(werke[0].wert, None, "eine ungesetzte Freigabe ist nicht leer");
        assert_eq!(werke[1].wert, Some(60), "die gesetzte Freigabe kommt nicht durch");

        // Und die Beschreibung nennt den Bezug des Anteils.
        assert!(werke[0].hinweis.contains("16 GiB eigener Speicher"), "{}", werke[0].hinweis);
        assert!(werke[1].hinweis.contains("gemeinsamer Speicher"), "{}", werke[1].hinweis);
        assert!(werke[1].hinweis.contains("16 Kerne"), "{}", werke[1].hinweis);
    }

    /// ⚑ **Die Kennung haengt am Geraet und nicht am Listenplatz.**
    #[test]
    fn die_kennung_ueberlebt_eine_zweite_karte() {
        assert_eq!(kennung_aus("Apple M5 Pro"), "apple-m5-pro");
        assert_eq!(kennung_aus("NVIDIA GeForce RTX 4090"), "nvidia-geforce-rtx-4090");
        assert_eq!(kennung_aus("PCI\\VEN_10DE&DEV_2684"), "pci-ven-10de-dev-2684");
        assert_ne!(kennung_aus("Karte A"), kennung_aus("Karte B"));
    }
}
