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
//! # 📌 Was ein Scan nicht darf
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
    /// ⚑ **Welcher Rechenweg ueber dieses Geraet fuehren wuerde.**
    /// `None` heisst: gar keiner, und das ist etwas anderes als „einer,
    /// der hier nicht rechnet". Der Unterschied steht im Sperrgrund des
    /// Reglers, denn er entscheidet, ob jemand etwas bauen kann.
    pub rueckseite: Option<Rueckseite>,
}

/// Der Rechenweg, der ein Geraet bedient.
///
/// ⚑ **Eine Eigenschaft des Geraets und nicht der Uebersetzung.** Ob
/// der Weg hier auch **rechnet**, ist die zweite Frage, und sie wird
/// dort gestellt, wo der Code ausgewaehlt wird
/// ([`crate::rechenwege::vorhanden`]). Wer beides in einem Feld
/// zusammenzoege, koennte nicht mehr sagen, ob ein Geraet keinen
/// Rechenweg hat oder nur keinen gebauten.
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
    /// fragen dieselbe Liste. Ein zweites Etikett hier hiesse, dass ein
    /// Regler einen Weg freigibt, den der Pruefstand nicht kennt.
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

impl Einheit {
    /// **Wie ein Wert in dieser Einheit gelesen wird.**
    ///
    /// ⚑ **Hier und nicht in der Konsole.** Sie zeigt dieselben Regler
    /// wie das Fenster, und eine zweite Schreibweise derselben Einheit
    /// waere eine Zeile, die je nach Bedieninstrument anders aussieht.
    ///
    /// ⚠️ **Das Fenster formatiert noch selbst**, in `ui/app.js`: Es
    /// braucht die Schrift **waehrend** des Ziehens, bevor irgendetwas
    /// ueber die Naht gegangen ist. Die beiden Stellen sind damit
    /// bekannt und nicht uebersehen.
    pub fn wie(self, n: u64, sprache: crate::einstellungen::Sprache) -> String {
        match self {
            Self::Kerne if n == 1 => sprache.waehlen("1 Kern", "1 core").to_string(),
            Self::Kerne => format!("{n} {}", sprache.waehlen("Kerne", "cores")),
            Self::Gib => format!("{n} GiB"),
            Self::Prozent => format!("{n} %"),
        }
    }
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
    /// ⚑ **Das linke Ende, und es ist nicht ueberall dasselbe.** Bei
    /// einer Grenze ist es `1`: Null Kerne waeren kein enger gestellter
    /// Klient, sondern ein Stillstand. Bei einem Rechenwerk ist es `0`,
    /// denn „dieses Geraet nicht benutzen" ist eine Einstellung, die
    /// jemand wirklich treffen will.
    pub mindestens: u64,
    /// Was heute freigegeben ist. ⚑ **`None` heisst „ohne Grenze" und
    /// steht ganz rechts**, nicht links und nicht bei null: Wer nichts
    /// eingestellt hat, gibt alles her.
    pub wert: Option<u64>,
    /// Wie die **linke** Endstellung heisst, wenn sie einen eigenen
    /// Namen hat. `None`: dort steht der Wert selbst.
    ///
    /// ⚑ **Die Worte stehen hier und nicht in der Oberflaeche**, in der
    /// eingestellten Sprache. Fenster und Konsole zeigen denselben
    /// Regler; zwei Stellen mit je eigenen Worten laufen auseinander,
    /// und die zweite ist die schlechter gepruefte.
    pub links: Option<String>,
    /// Wie die **rechte** Endstellung heisst. Immer benannt, denn dort
    /// steht nie eine Zahl, sondern „ohne Grenze".
    pub rechts: String,
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

    /// Nur die Rechenwerke, ohne Speicher und ohne Platte.
    ///
    /// ⚑ **Weil der Scan eines Rechenwerks einen Unterprozess kostet
    /// und der Rest nicht.** Wer nur wissen will, welches Geraet welchen
    /// Rechenweg bedient, braucht weder `statvfs` noch `hw.memsize`.
    pub fn rechenwerke() -> Vec<Rechenwerk> {
        rechenwerke_suchen()
    }
}

/// **Setzt um, was der Nutzer an Kapazitaet freigegeben hat**: die
/// Kerngrenze und die Rechenwerke.
///
/// # ⚑ Eine Stelle, drei Bedieninstrumente
///
/// `myl`, `myelith` und das Fenster geben dieselben Einstellungen frei.
/// **Drei Umsetzungen derselben Freigabe liefen auseinander**, und die
/// Konsole zeigte, wie das aussieht: Sie las `kap.kerne`, zeigte es an
/// und wandte es nie an (Fund 381).
///
/// # ⚑ Der Scan laeuft nur, wenn er etwas aendern kann
///
/// Ein Rechenwerk abzuschalten verlangt, sein Geraet zu kennen, und das
/// kostet auf macOS einen Unterprozess von rund einer Sekunde.
/// **Abgeschaltet wird ein Werk nur durch eine ausdrueckliche Null in
/// der Ablage**; steht dort keine, gibt es nichts abzuschalten und
/// nichts zu scannen. Das ist der Normalfall: Ohne Eintrag ist ein Werk
/// ganz freigegeben.
/// # ⚑ Warum sie etwas zurueckgibt
///
/// **Damit sich pruefen laesst, ob der Durchlauf lief.** Ob ein
/// Rechenwerk wirklich abgeschaltet wurde, sieht man nur auf einer
/// Maschine mit Rechenwerk, und in der CI steckt keine Grafikkarte.
/// Der Rueckgabewert ist die Stelle, an der die **Entscheidung**
/// nachpruefbar wird, und sie ist es, die zweimal falsch sein koennte.
///
/// `true` heisst: Die Rechenwerke wurden durchgegangen.
pub fn anwenden(e: &crate::einstellungen::Einstellungen) -> bool {
    if let Some(n) = e.kapazitaet.kerne {
        crate::kapazitaet::kerne_setzen(n);
    }
    let eine_null = e.kapazitaet.rechenwerke.values().any(|p| *p == 0);
    // ⚑ **Die Abkuerzung gilt nur, solange auch nichts abgeschaltet
    // ist.** Ohne die Marke waere das Zurueckdrehen des Reglers
    // wirkungslos: Wer ein Werk auf null zieht und gleich darauf wieder
    // hinauf, haette keine Null mehr in der Ablage, und der Durchlauf,
    // der das Werk zurueckholt, faende hier sein Ende. **Ein Schalter,
    // der nur in eine Richtung wirkt, ist keiner.**
    if !eine_null && !ABGESCHALTET.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    for w in Hardware::rechenwerke() {
        let Some(r) = w.rueckseite else { continue };
        // ⚑ **Kein Eintrag heisst ganz freigegeben**, wie ueberall
        // sonst: Der Regler steht dann am rechten Anschlag.
        let an = e.kapazitaet.rechenwerke.get(&w.kennung).is_none_or(|p| *p > 0);
        crate::rechenwege::setzen(r.backend(), an);
    }
    ABGESCHALTET.store(eine_null, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Ob in diesem Prozess schon einmal ein Rechenwerk abgeschaltet wurde.
///
/// ⚑ **Sie steht hier und nicht in der Ablage.** Ein Schalter, den
/// jemand umgelegt hat, gilt fuer diesen Prozess; die Ablage sagt, was
/// eingestellt ist, und das ist dieselbe Trennung wie zwischen
/// gespeichertem und wirksamem Kernbudget.
static ABGESCHALTET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

    // 📌 **Das Verzeichnis muss es geben.** `statvfs` auf einen Pfad,
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
                // ⚑ **Nur das eingebaute Werk, und das ist keine
                // Bequemlichkeit.** Der Metal-Rechenweg ist fuer die
                // GPU von Apple-Silizium geschrieben, und die
                // Selbstpruefung laeuft gegen das Geraet, das
                // `MTLCreateSystemDefaultDevice` herausgibt: auf diesen
                // Maschinen genau das eingebaute. Eine Steckkarte in
                // einem aelteren Mac traegt deshalb `None` und keine
                // Zusage, die niemand geprueft hat.
                rueckseite: eingebaut.then_some(Rueckseite::Metal),
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
        // ⚑ **Die Herstellerkennung steht im Geraet, der Name in einer
        // Zeichenkette, die ein Treiber setzt.** Deshalb zaehlt die
        // Kennung zuerst; `vendor` ist hier `0x10de` oder `0x1002` und
        // kommt aus `sysfs`, nicht aus einer Beschriftung.
        let rueckseite = rueckseite_aus(Some(&hersteller), &name);
        aus.push(Rechenwerk {
            kennung: kennung_aus(&format!("{karte}-{hersteller}-{geraetenummer}")),
            name,
            speicher_bytes: lies("mem_info_vram_total").and_then(|s| s.parse().ok()),
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite,
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
            // ⚑ **Die Herstellerkennung steht in der PNP-Kennung**, als
            // `PCI\VEN_10DE&DEV_2684&…`. Sie kommt aus dem Geraet und
            // nicht aus einer Beschriftung, deshalb zaehlt sie zuerst;
            // der Name ist der Rueckfall.
            let rueckseite = rueckseite_aus(hersteller_aus_pnp(&kennung).as_deref(), &name);
            Some(Rechenwerk {
                // 📌 `AdapterRAM` ist ein `uint32` und laeuft ab 4 GiB
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
                rueckseite,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn rechenwerke_suchen() -> Vec<Rechenwerk> {
    Vec::new()
}

/// Die Herstellerkennung aus einer PNP-Geraetekennung: `VEN_10DE` → `10de`.
// ⚑ **Auf macOS ruft sie niemand, und geprueft wird sie trotzdem.**
// Unter `cfg` gestellt liefe sie auf der Entwicklungsmaschine nie durch
// eine Pruefung, und genau dort wird das Zusammenspiel zweier Karten
// entschieden. Der Vermerk nimmt die Warnung ueber den Zweig, den
// dieses Ziel nicht ruft, und sagt zugleich, welches Ziel gemeint ist.
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
fn hersteller_aus_pnp(pnp: &str) -> Option<String> {
    let k = pnp.to_ascii_lowercase();
    let ab = k.find("ven_")? + 4;
    let s: String = k[ab..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    (!s.is_empty()).then_some(s)
}

/// **Welcher Rechenweg dieses Geraet bedienen wuerde.**
///
/// ⚑ **Nicht unter `cfg` gestellt, obwohl nur zwei Betriebssysteme sie
/// rufen.** Sie ist reine Zeichenkettenarbeit und kostet auf einem
/// dritten Ziel nichts; unter `cfg` liefe sie auf der
/// Entwicklungsmaschine nie durch eine Pruefung und waere genau dort
/// ungedeckt, wo das Zusammenspiel zweier Karten entschieden wird.
///
/// ⚑ **Die Herstellerkennung zaehlt vor dem Namen.** Sie steht im
/// Geraet; der Name ist eine Zeichenkette, die ein Treiber setzt und
/// ein Hersteller aendert.
// ⚑ **Auf macOS ruft sie niemand, und geprueft wird sie trotzdem.**
// Unter `cfg` gestellt liefe sie auf der Entwicklungsmaschine nie durch
// eine Pruefung, und genau dort wird das Zusammenspiel zweier Karten
// entschieden. Der Vermerk nimmt die Warnung ueber den Zweig, den
// dieses Ziel nicht ruft, und sagt zugleich, welches Ziel gemeint ist.
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
        // Intel-Grafik bekommt hier `None`, und das ist die richtige
        // Antwort: Es gibt keinen Rechenweg fuer sie, und ein geratener
        // waere ein Regler, der etwas freigibt, das nie jemand rechnet.
        None
    }
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
///
/// 📌 **Bis zum 2026-09-16 stand hier ein einziger Satz fuer alle
/// Rechenwerke, und er war seit dem 2026-09-14 falsch** (Fund 380). Er
/// sagte „noch kein Rechenweg ueber dieses Geraet", waehrend `metal`
/// auf Apple-Silizium die gebuendelten Matrizen der Vorbereitung
/// wirklich rechnet. **Die Begruendung aus der Zeit, als es keinen
/// Rechenweg gab, galt fuer ihren Fall und nicht fuer den zweiten.**
/// Deshalb nennt der Satz jetzt den Rechenweg des Geraets, und ob er
/// ueberhaupt faellt, entscheidet [`crate::rechenwege::vorhanden`].
fn gesperrt_weil(r: Option<Rueckseite>, sprache: crate::einstellungen::Sprache) -> String {
    let Some(r) = r else {
        return sprache.waehlen(OHNE_RUECKSEITE.0, OHNE_RUECKSEITE.1).to_string();
    };
    sprache.waehlen_wert(
        format!(
            "Der Rechenweg {} führt über dieses Gerät, und er rechnet in diesem \
             Programm nicht. Die Rückseiten für NVIDIA und AMD reichen heute an die \
             Referenzkernel weiter, gerechnet wird also auf der CPU. Es fehlen echte \
             Ganzzahlkerne, INT8-GEMM über dp4a bei NVIDIA, i8mm bei ARM und VNNI bei \
             x86, und ein Konformitätslauf auf genau dieser Architektur. Ohne ihn würde \
             ein Miner mit abweichendem Kernel bestraft, ohne etwas falsch gemacht zu \
             haben. Sobald beides steht, wirkt dieser Regler ohne weiteres Zutun.",
            r.anzeige()
        ),
        format!(
            "The {} compute path serves this device, and it does not compute in this \
             program. The NVIDIA and AMD back ends currently hand through to the \
             reference kernels, so the work happens on the CPU. What is missing are real \
             integer kernels, INT8 GEMM via dp4a on NVIDIA, i8mm on ARM and VNNI on x86, \
             and a conformance run on exactly this architecture. Without it a miner with \
             a deviating kernel would be penalised without having done anything wrong. \
             Once both exist, this slider takes effect with no further work.",
            r.anzeige()
        ),
    )
}

/// Warum ein Geraet ohne jeden Rechenweg gesperrt ist.
///
/// ⚑ **Das ist etwas anderes als ein Rechenweg, der hier nicht
/// rechnet**, und der Unterschied ist der zwischen „noch nicht gebaut"
/// und „gibt es nicht". Wer den ersten Satz an einer eingebauten
/// Intel-Grafik liest, wartet auf etwas, das niemand vorhat.
pub const OHNE_RUECKSEITE: (&str, &str) = (
    "Für dieses Gerät gibt es in Myelith keinen Rechenweg. Gerechnet wird \
     auf der CPU, und daran ändert dieser Regler nichts.",
    "Myelith has no compute path for this device. The work happens on the \
     CPU, and this slider does not change that.",
);

/// Warum ein Betriebsmittel ohne erkanntes Ende gesperrt ist.
pub const OHNE_ENDE: (&str, &str) = (
    "Nicht erkannt: Diese Maschine sagt nicht, wie viel sie hiervon hat, \
     und ein Regler ohne Ende ist keiner.",
    "Not detected: this machine does not say how much of this it has, \
     and a slider without an end is not a slider.",
);

/// ⚑ **Wie die rechte Endstellung jedes Reglers heisst.**
///
/// **Ganz rechts heisst „ohne Grenze", und dort steht jeder Regler,
/// solange niemand ihn bewegt hat** (Auftrag des Projektinhabers,
/// 2026-09-16). 📌 Bis dahin lag diese Stellung ganz **links**, bei
/// null. Sie bedeutete dasselbe und las sich wie das Gegenteil: Wer
/// einen Regler am linken Anschlag sieht, liest „nichts", nicht
/// „alles".
pub const OHNE_GRENZE: (&str, &str) = ("ohne Grenze", "no limit");

/// Wie die linke Endstellung eines Rechenwerks heisst.
///
/// ⚑ **Bei einem Rechenwerk ist null eine Einstellung und kein
/// Stillstand**, anders als bei den Kernen: Ein Rechner ohne
/// freigegebene GPU rechnet weiter, er rechnet nur auf der CPU.
pub const RECHNET_NICHT: (&str, &str) = ("rechnet nicht", "not used");

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

        // 📌 **In der eingestellten Sprache, seit dem 2026-09-10.** Die
        // Beschriftungen kamen roh aus `FELDER`, also immer auf Deutsch,
        // und ein englisches Fenster trug in der Freigabemaske deutsche
        // Saetze. **Uebersetzt wird an einer Stelle, und diese hier
        // hatte sie nicht gefragt.**
        let sprache = e.oberflaeche.sprache;
        let ohne_grenze = sprache.waehlen(OHNE_GRENZE.0, OHNE_GRENZE.1).to_string();
        // ⚑ **Einmal gefragt, nicht je Geraet.** Welche Rechenwege
        // rechnen, haengt an der Uebersetzung und an der Maschine und
        // nicht am einzelnen Geraet; `metal::verfuegbar` baut beim
        // ersten Ruf ein Geraet auf und prueft es gegen die CPU.
        let rechnende = crate::rechenwege::vorhanden();
        let mut aus = Vec::new();
        for f in FELDER.iter().filter(|f| f.freigabe) {
            let f = f.in_sprache(sprache);
            let (einheit, hoechstens) = self.masse(f.name);
            aus.push(Regler {
                name: f.name.to_string(),
                titel: f.titel.to_string(),
                einheit,
                hoechstens,
                // ⚑ **Eins und nicht null.** Eine Grenze von null Kernen
                // waere kein enger gestellter Klient, sondern einer, der
                // nicht antwortet; wer gar nichts hergeben will,
                // schliesst das Programm.
                mindestens: 1,
                wert: e.wert(f.name).ok().and_then(zahl),
                links: None,
                rechts: ohne_grenze.clone(),
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
            // ⚑ **Zwei Fragen, und beide muessen Ja sein.** Fuehrt ueber
            // dieses Geraet ueberhaupt ein Rechenweg, und rechnet der
            // hier? Die erste beantwortet der Scan, die zweite die
            // Kernkiste. **Ein Ja auf die erste allein war bis zum
            // 2026-09-16 der Sperrgrund fuer alle** und traf damit auch
            // das Geraet, das wirklich rechnet.
            let rechnet = r.rueckseite.is_some_and(|b| rechnende.contains(&b.backend()));
            aus.push(Regler {
                name: format!("{RECHENWERK_PRAEFIX}{}", r.kennung),
                titel: r.beschriftung(),
                einheit: Einheit::Prozent,
                hoechstens: Some(100),
                // ⚑ **Null und nicht eins.** „Dieses Geraet nicht
                // benutzen" ist eine Einstellung, die jemand wirklich
                // trifft; der Rechenpfad laeuft dann auf der CPU weiter.
                mindestens: 0,
                wert: e
                    .wert(&format!("{RECHENWERK_PRAEFIX}{}", r.kennung))
                    .ok()
                    .and_then(zahl),
                links: Some(sprache.waehlen(RECHNET_NICHT.0, RECHNET_NICHT.1).to_string()),
                rechts: ohne_grenze.clone(),
                hinweis: r.beschreibung(self.speicher_bytes, sprache, rechnet),
                sperrgrund: (!rechnet).then(|| gesperrt_weil(r.rueckseite, sprache)),
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
    /// **Wie das Geraet in der Maske heisst**, samt seinem Rechenweg.
    ///
    /// ⚑ **Der Rechenweg gehoert in die Beschriftung und nicht in den
    /// Satz darunter** (Auftrag des Projektinhabers, 2026-09-16). Wer
    /// zwei Karten im Rechner hat, muss an der Zeile sehen, welche
    /// wovon bedient wird; ein Name allein sagt das nicht.
    pub fn beschriftung(&self) -> String {
        match self.rueckseite {
            Some(r) => format!("{} ({})", self.name, r.anzeige()),
            None => self.name.clone(),
        }
    }

    /// Ein Satz darueber, was dieses Rechenwerk ist.
    ///
    /// ⚑ **`rechnet` entscheidet, ob der Satz sagt, was der Anteil
    /// bewirkt.** Bei einem gesperrten Werk steht das im Sperrgrund,
    /// und zweimal dasselbe in zwei Saetzen unter demselben Regler
    /// waere eine Wiederholung, die irgendwann auseinanderlaeuft.
    pub fn beschreibung(
        &self,
        speicher_der_maschine: Option<u64>,
        sprache: crate::einstellungen::Sprache,
        rechnet: bool,
    ) -> String {
        let gib = |b: u64| format!("{:.0} GiB", b as f64 / GIB as f64);
        let kerne = match self.kerne {
            Some(k) => format!("{k} {}, ", sprache.waehlen("Kerne", "cores")),
            None => String::new(),
        };
        let bauart = if self.gemeinsamer_speicher {
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
        };
        if !rechnet {
            return bauart;
        }
        // ⚑ **Was der Anteil heute wirklich tut, und was er noch nicht
        // tut.** Abschaltbar ist das Werk, teilbar ist es nicht: Eine
        // Quote auf die Rechenzeit einer GPU braucht einen Planer, und
        // den gibt es nicht. **Der Satz sagt beides**, denn ein Regler,
        // der mehr verspricht als er haelt, ist schlimmer als ein
        // gesperrter.
        let anteil = sprache.waehlen(
            "Ganz links rechnet dieses Werk nicht mit, und der Rechenpfad läuft \
             vollständig auf der CPU weiter; jeder Wert darüber lässt es rechnen. Eine \
             Teilquote wirkt örtlich noch nicht, sie steht als Freigabe da: Ein Anteil an \
             der Rechenzeit einer GPU braucht einen Planer, den es noch nicht gibt.",
            "At the far left this unit does not take part and the compute path runs \
             entirely on the CPU; any value above lets it compute. A partial share has no \
             local effect yet, it is recorded as a pledge: a share of a GPU's compute time \
             needs a scheduler that does not exist yet.",
        );
        format!("{bauart} {anteil}")
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::einstellungen::Einstellungen;

    /// Ein Werk mit allem, was der Scan hergeben kann.
    fn werk(kennung: &str, name: &str, r: Option<Rueckseite>) -> Rechenwerk {
        Rechenwerk {
            kennung: kennung.into(),
            name: name.into(),
            speicher_bytes: Some(16 * GIB),
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite: r,
        }
    }

    fn maschine(werke: Vec<Rechenwerk>) -> Hardware {
        Hardware { kerne: 8, speicher_bytes: Some(32 * GIB), platte: None, rechenwerke: werke }
    }

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

    /// ⚑ **Ohne Zutun steht jeder Regler ganz rechts auf „ohne
    /// Grenze".**
    ///
    /// Das ist der Auftrag des Projektinhabers vom 2026-09-16, und er
    /// hat zwei Haelften, die beide hier stehen: Der **Wert** ist nicht
    /// gesetzt (also ohne Grenze), und das **rechte Ende** traegt genau
    /// dieses Wort. Eine Vorgabe ohne Beschriftung waere nicht zu sehen.
    #[test]
    fn ohne_zutun_steht_jeder_regler_ganz_rechts() {
        let h = maschine(vec![werk("eins", "Karte eins", Some(Rueckseite::Cuda))]);
        for r in h.regler(&Einstellungen::default()) {
            assert_eq!(r.wert, None, "`{}` ist ohne Zutun begrenzt", r.name);
            assert_eq!(r.rechts, "ohne Grenze", "`{}` benennt sein rechtes Ende nicht", r.name);
        }
    }

    /// ⚑ **Die beiden linken Enden sind verschieden, und das ist der
    /// Unterschied zwischen einer Grenze und einem Anteil.**
    ///
    /// Null Kerne waeren ein Stillstand; ein Rechenwerk bei null rechnet
    /// einfach nicht mit, und der Rechenpfad laeuft auf der CPU weiter.
    #[test]
    fn eine_grenze_faengt_bei_eins_an_ein_rechenwerk_bei_null() {
        let h = maschine(vec![werk("eins", "Karte eins", Some(Rueckseite::Rocm))]);
        let r = h.regler(&Einstellungen::default());

        for name in ["kap.kerne", "kap.speicher", "kap.platte"] {
            let g = r.iter().find(|r| r.name == name).expect(name);
            assert_eq!(g.mindestens, 1, "{name} laesst sich auf null stellen");
            assert_eq!(g.links, None, "{name} benennt ein linkes Ende, das keinen Namen hat");
        }

        let w = r.iter().find(|r| r.name == "kap.rechenwerk.eins").expect("Werk");
        assert_eq!(w.mindestens, 0, "ein Rechenwerk laesst sich nicht abschalten");
        assert_eq!(w.links.as_deref(), Some("rechnet nicht"));
    }

    /// ⚑ **Ein Rechenwerk ist genau dann frei, wenn sein Rechenweg hier
    /// rechnet**, und das ist Fund 380.
    ///
    /// Bis zum 2026-09-16 war **jedes** Werk gesperrt, mit dem Satz
    /// „noch kein Rechenweg ueber dieses Geraet". Seit dem 2026-09-14
    /// rechnet `metal` die gebuendelten Matrizen der Vorbereitung; der
    /// Satz war also an der einen Maschine falsch, an der er
    /// ueberhaupt etwas zu sagen hatte.
    ///
    /// ⚑ **Geprueft wird gegen dieselbe Liste, die der Rechenpfad
    /// fragt**, und nicht gegen eine Annahme ueber dieses
    /// Betriebssystem: Ob `metal` rechnet, haengt an Geraet, Shader und
    /// einer Selbstpruefung gegen die CPU, und keines davon steht in
    /// einem `cfg`.
    #[test]
    fn frei_ist_ein_werk_genau_dann_wenn_sein_rechenweg_rechnet() {
        let h = maschine(vec![
            werk("apfel", "Apple M5 Pro", Some(Rueckseite::Metal)),
            werk("gruen", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)),
            werk("rot", "AMD Radeon RX 7900 XTX", Some(Rueckseite::Rocm)),
            werk("blau", "Intel UHD Graphics", None),
        ]);
        let rechnende = crate::rechenwege::vorhanden();
        let r = h.regler(&Einstellungen::default());

        for (kennung, rueckseite) in [
            ("apfel", Some(Rueckseite::Metal)),
            ("gruen", Some(Rueckseite::Cuda)),
            ("rot", Some(Rueckseite::Rocm)),
            ("blau", None),
        ] {
            let g = r
                .iter()
                .find(|g| g.name == format!("kap.rechenwerk.{kennung}"))
                .expect(kennung);
            let rechnet = rueckseite.is_some_and(|b| rechnende.contains(&b.backend()));
            assert_eq!(
                g.sperrgrund.is_none(),
                rechnet,
                "`{kennung}` ist frei={} , aber sein Rechenweg rechnet={rechnet}",
                g.sperrgrund.is_none()
            );
        }

        // ⚑ **cuda und rocm reichen an die Referenzkernel weiter**, sie
        // koennen also nie frei sein. Waere das eines Tages anders,
        // beisst diese Zeile, und dann gehoert der Rechenweg in die
        // Liste und diese Zeile weg.
        assert!(!rechnende.contains(&"cuda"), "cuda meldet einen eigenen Rechenpfad");
        assert!(!rechnende.contains(&"rocm"), "rocm meldet einen eigenen Rechenpfad");
    }

    /// ⚑ **Ein Geraet ohne Rechenweg wird anders begruendet als eines,
    /// dessen Rechenweg noch nicht gebaut ist.**
    ///
    /// Der Unterschied ist der zwischen „noch nicht" und „gar nicht",
    /// und er entscheidet, ob jemand darauf wartet.
    #[test]
    fn der_sperrgrund_unterscheidet_noch_nicht_von_gar_nicht() {
        let h = maschine(vec![
            werk("gruen", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)),
            werk("blau", "Intel UHD Graphics", None),
        ]);
        let r = h.regler(&Einstellungen::default());
        let grund = |k: &str| {
            r.iter()
                .find(|g| g.name == format!("kap.rechenwerk.{k}"))
                .and_then(|g| g.sperrgrund.clone())
                .unwrap_or_else(|| panic!("{k} ist nicht gesperrt"))
        };

        let gebaut = grund("gruen");
        // ⚑ Der Grund sagt, **was fehlt**, nicht nur dass etwas fehlt.
        assert!(gebaut.contains("CUDA"), "der Rechenweg des Geraets steht nicht im Grund: {gebaut}");
        assert!(gebaut.contains("Konformitätslauf") && gebaut.contains("Ganzzahlkerne"), "{gebaut}");

        let keiner = grund("blau");
        assert!(keiner.contains("keinen Rechenweg"), "{keiner}");
        assert!(
            !keiner.contains("Konformitätslauf"),
            "ein Geraet ohne Rechenweg laesst auf einen Prueflauf warten: {keiner}"
        );
    }

    /// ⚑ **Die Beschriftung nennt den Rechenweg**, sonst ist bei zwei
    /// Karten nicht zu sehen, welche wovon bedient wird.
    #[test]
    fn die_beschriftung_nennt_den_rechenweg() {
        assert_eq!(
            werk("a", "Apple M5 Pro", Some(Rueckseite::Metal)).beschriftung(),
            "Apple M5 Pro (Metal)"
        );
        assert_eq!(
            werk("b", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)).beschriftung(),
            "NVIDIA GeForce RTX 4090 (CUDA)"
        );
        // Ohne Rechenweg bleibt der Name allein: Eine leere Klammer
        // saehe aus wie ein verlorener Wert.
        assert_eq!(werk("c", "Intel UHD Graphics", None).beschriftung(), "Intel UHD Graphics");
    }

    /// ⚑ **Zwei Karten verschiedener Hersteller bekommen verschiedene
    /// Rechenwege**, und genau das ist der Fall, um den es geht: eine
    /// NVIDIA und eine AMD im selben Rechner.
    ///
    /// ⚑ **Die Herstellerkennung schlaegt den Namen.** Sie steht im
    /// Geraet; der Name ist eine Zeichenkette, die ein Treiber setzt.
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

    /// ⚑ **Jedes gefundene Rechenwerk bekommt einen eigenen Regler.**
    #[test]
    fn jedes_rechenwerk_bekommt_einen_eigenen_regler() {
        let h = Hardware {
            kerne: 8,
            speicher_bytes: Some(32 * GIB),
            platte: None,
            rechenwerke: vec![
                werk("eins", "Karte eins", Some(Rueckseite::Cuda)),
                Rechenwerk {
                    kennung: "zwei".into(),
                    name: "Karte zwei".into(),
                    speicher_bytes: None,
                    gemeinsamer_speicher: true,
                    kerne: Some(16),
                    rueckseite: Some(Rueckseite::Metal),
                },
            ],
        };
        let mut e = Einstellungen::default();
        e.setzen("kap.rechenwerk.zwei", "60").expect("setzen");

        let r = h.regler(&e);
        let werke: Vec<_> = r.iter().filter(|r| r.einheit == Einheit::Prozent).collect();
        assert_eq!(werke.len(), 2, "nicht jede Karte hat einen Regler");
        for w in &werke {
            assert_eq!(w.hoechstens, Some(100));
        }
        // ⚑ **Ohne Eintrag ganz rechts, mit Eintrag dort, wo er steht.**
        assert_eq!(werke[0].wert, None, "eine ungesetzte Freigabe ist nicht ohne Grenze");
        assert_eq!(werke[1].wert, Some(60), "die gesetzte Freigabe kommt nicht durch");

        // Und die Beschreibung nennt den Bezug des Anteils.
        assert!(werke[0].hinweis.contains("16 GiB eigener Speicher"), "{}", werke[0].hinweis);
        assert!(werke[1].hinweis.contains("gemeinsamer Speicher"), "{}", werke[1].hinweis);
        assert!(werke[1].hinweis.contains("16 Kerne"), "{}", werke[1].hinweis);
    }

    /// ⚑ **Ein Schalter, der nur in eine Richtung wirkt, ist keiner.**
    ///
    /// Der Durchlauf ueber die Rechenwerke kostet einen Unterprozess
    /// und laeuft deshalb nur, wenn er etwas aendern kann. **Die
    /// Abkuerzung hatte beim Schreiben ein Loch:** Wer ein Werk auf
    /// null zieht und gleich darauf wieder hinauf, hat danach keine
    /// Null mehr in der Ablage, und genau der Durchlauf, der das Werk
    /// zurueckholt, waere ausgefallen.
    ///
    /// 📌 **Alles darueber steht in einer einzigen Pruefung**, denn die
    /// Marke gilt fuer den ganzen Prozess und `cargo test` laeuft
    /// nebenlaeufig. Zwei Pruefungen daran waeren einzeln gruen und
    /// zusammen rot, und das ist Fund 378.
    #[test]
    fn aus_und_wieder_an_laeuft_beide_male_durch() {
        let mut e = Einstellungen::default();
        // Ohne Eintrag gibt es nichts abzuschalten und nichts zu scannen.
        assert!(!anwenden(&e), "der Durchlauf lief, obwohl nichts eingestellt ist");

        e.setzen("kap.rechenwerk.probe", "0").expect("auf null");
        assert!(anwenden(&e), "eine Null schaltet nichts ab");

        // ⚑ **Und wieder hinauf.** Jetzt steht keine Null mehr da, und
        // trotzdem muss der Durchlauf laufen, sonst bliebe das Werk aus.
        e.setzen("kap.rechenwerk.probe", "aus").expect("zurueck");
        assert!(anwenden(&e), "das Zurueckdrehen erreicht den Rechenpfad nicht");

        // Und danach ist wieder Ruhe.
        assert!(!anwenden(&e), "der Durchlauf laeuft weiter, obwohl nichts mehr aus ist");
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
