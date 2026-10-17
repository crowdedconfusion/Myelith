//! **Die Anbindung an die Treiber**: CUDA ueber den Treiber und NVRTC,
//! ROCm ueber HIP und hiprtc, beide zur Laufzeit geladen ([`super::lader`]).
//!
//! # ⚑ Zwei Hersteller, eine Schnittstelle
//!
//! Beide Seiten brauchen dasselbe: eine Karte waehlen, Speicher belegen,
//! hin und her kopieren, eine Quelle uebersetzen, einen Kern starten,
//! warten. Die Funktionen heissen verschieden und haben fast dieselben
//! Signaturen. [`Karte`] verbirgt den Unterschied; alles darueber kennt nur
//! sie.
//!
//! # ⚑ Was hier nicht gebraucht wird
//!
//! Keine Laufzeitbibliothek (`libcudart`), keine Rechenbibliotheken
//! (cuBLAS, rocBLAS): Sie rechnen in Gleitkomma oder auf Tensor Cores, und
//! beides schliesst der Determinismus-Vertrag aus. Gebraucht werden nur der
//! Treiber und der Uebersetzer zur Laufzeit.
//!
//! # ⚠️ Ungeprueft ohne Hardware
//!
//! Auf der Entwicklungsmaschine gibt es keine dieser Karten. Die Signaturen
//! sind gegen die oeffentlichen Kopfdateien geschrieben; ob sie tragen, sagt
//! erst die Selbstpruefung auf einer Karte ([`super::Verbund`]). Scheitert
//! sie, rechnet die CPU.

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use super::lader::{text, Bibliothek};

/// Die Quelle der Kerne, im Baum als eigene Datei, damit sie lesbar bleibt
/// und der Gleitkomma-Audit sie findet.
pub(super) const QUELLE: &str = include_str!("kerne.cu");

/// Das Kachelmass, wie in `kerne.cu`.
pub(super) const KACHEL_ZEILEN: usize = 64;
pub(super) const KACHEL_EINGABEN: usize = 16;
const FAEDEN_X: u32 = 16;
const FAEDEN_Y: u32 = 16;

/// Wer die Karte gebaut hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hersteller {
    /// CUDA.
    Nvidia,
    /// ROCm, also HIP.
    Amd,
}

impl Hersteller {
    /// Der Name des Rechenwegs, wie ihn `rechenpfad` und `run.sh` kennen.
    pub fn rechenweg(self) -> &'static str {
        match self {
            Hersteller::Nvidia => "cuda",
            Hersteller::Amd => "rocm",
        }
    }
}

/// Eine Adresse im Speicher der Karte.
pub(super) type Zeiger = u64;

type Ergebnis = c_int;

/// Die Funktionen von CUDA (Treiber und NVRTC).
struct Cuda {
    init: unsafe extern "C" fn(c_uint) -> Ergebnis,
    anzahl: unsafe extern "C" fn(*mut c_int) -> Ergebnis,
    geraet: unsafe extern "C" fn(*mut c_int, c_int) -> Ergebnis,
    name: unsafe extern "C" fn(*mut c_char, c_int, c_int) -> Ergebnis,
    pci: unsafe extern "C" fn(*mut c_char, c_int, c_int) -> Ergebnis,
    merkmal: unsafe extern "C" fn(*mut c_int, c_int, c_int) -> Ergebnis,
    kontext_halten: unsafe extern "C" fn(*mut *mut c_void, c_int) -> Ergebnis,
    kontext_setzen: unsafe extern "C" fn(*mut c_void) -> Ergebnis,
    modul_laden: unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> Ergebnis,
    funktion: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *const c_char) -> Ergebnis,
    belegen: unsafe extern "C" fn(*mut u64, usize) -> Ergebnis,
    freigeben: unsafe extern "C" fn(u64) -> Ergebnis,
    hoch: unsafe extern "C" fn(u64, *const c_void, usize) -> Ergebnis,
    runter: unsafe extern "C" fn(*mut c_void, u64, usize) -> Ergebnis,
    starten: unsafe extern "C" fn(
        *mut c_void,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        *mut c_void,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Ergebnis,
    warten: unsafe extern "C" fn() -> Ergebnis,
    fehlertext: unsafe extern "C" fn(Ergebnis, *mut *const c_char) -> Ergebnis,
    speicher: unsafe extern "C" fn(*mut usize, *mut usize) -> Ergebnis,
    rtc: Rtc,
}

/// Die Funktionen von HIP und hiprtc.
struct Hip {
    init: unsafe extern "C" fn(c_uint) -> Ergebnis,
    anzahl: unsafe extern "C" fn(*mut c_int) -> Ergebnis,
    geraet_setzen: unsafe extern "C" fn(c_int) -> Ergebnis,
    name: unsafe extern "C" fn(*mut c_char, c_int, c_int) -> Ergebnis,
    pci: unsafe extern "C" fn(*mut c_char, c_int, c_int) -> Ergebnis,
    /// `hipGetDevicePropertiesR0600` oder aeltere Fassung, nur fuer den
    /// Architekturnamen, siehe [`hip_architektur`].
    eigenschaften: Option<unsafe extern "C" fn(*mut c_void, c_int) -> Ergebnis>,
    modul_laden: unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> Ergebnis,
    funktion: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *const c_char) -> Ergebnis,
    belegen: unsafe extern "C" fn(*mut *mut c_void, usize) -> Ergebnis,
    freigeben: unsafe extern "C" fn(*mut c_void) -> Ergebnis,
    hoch: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> Ergebnis,
    runter: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> Ergebnis,
    starten: unsafe extern "C" fn(
        *mut c_void,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        *mut c_void,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Ergebnis,
    warten: unsafe extern "C" fn() -> Ergebnis,
    fehlertext: unsafe extern "C" fn(Ergebnis) -> *const c_char,
    speicher: unsafe extern "C" fn(*mut usize, *mut usize) -> Ergebnis,
    rtc: Rtc,
}

/// Der Uebersetzer zur Laufzeit; NVRTC und hiprtc haben dieselbe Form.
struct Rtc {
    anlegen: unsafe extern "C" fn(
        *mut *mut c_void,
        *const c_char,
        *const c_char,
        c_int,
        *const *const c_char,
        *const *const c_char,
    ) -> Ergebnis,
    uebersetzen: unsafe extern "C" fn(*mut c_void, c_int, *const *const c_char) -> Ergebnis,
    protokoll_groesse: unsafe extern "C" fn(*mut c_void, *mut usize) -> Ergebnis,
    protokoll: unsafe extern "C" fn(*mut c_void, *mut c_char) -> Ergebnis,
    /// Maschinencode (CUBIN bzw. Codeobjekt), falls vorhanden.
    code_groesse: Option<unsafe extern "C" fn(*mut c_void, *mut usize) -> Ergebnis>,
    code: Option<unsafe extern "C" fn(*mut c_void, *mut c_char) -> Ergebnis>,
    /// Nur NVRTC: PTX, der Rueckfall, wenn es kein CUBIN gibt.
    ptx_groesse: Option<unsafe extern "C" fn(*mut c_void, *mut usize) -> Ergebnis>,
    ptx: Option<unsafe extern "C" fn(*mut c_void, *mut c_char) -> Ergebnis>,
    loeschen: unsafe extern "C" fn(*mut *mut c_void) -> Ergebnis,
    fehlertext: unsafe extern "C" fn(Ergebnis) -> *const c_char,
    /// Nur fuer Meldungen.
    bibliothek: String,
}

/// Eine geladene Herstellerschnittstelle.
enum Schnittstelle {
    Cuda(Cuda),
    Hip(Hip),
    /// ⚑ Die nachgebildete Karte (`super::nachbaukarte`): Speicher im
    /// Hauptspeicher, Kerne auf der CPU. Nur ueber `MYL_GPU_NACHBAU`.
    Nachbau,
}

/// Die eine Schnittstelle aller nachgebildeten Karten.
static NACHBAU: Schnittstelle = Schnittstelle::Nachbau;

// SAFETY: Nur Funktionszeiger in Bibliotheken, die nie geschlossen werden.
unsafe impl Send for Schnittstelle {}
unsafe impl Sync for Schnittstelle {}

/// CUDA-Merkmale, wie in `cuda.h`.
const MERKMAL_MULTIPROZESSOREN: c_int = 16;
const MERKMAL_FAEHIGKEIT_HAUPT: c_int = 75;
const MERKMAL_FAEHIGKEIT_NEBEN: c_int = 76;

/// **Wo die Bibliotheken liegen koennen.** Erst eine ausdrueckliche
/// Angabe, dann die gewoehnlichen Namen, die der Lader des Systems sucht.
///
/// ⚑ **Keine festen Pfade.** ROCm liegt meist unter `$ROCM_PATH/lib`; der
/// Pfad kommt aus der Umgebung, die ROCm selbst setzt. NVRTC aus `pip`
/// (`nvidia-cuda-nvrtc-cu12`) liegt in einem Python-Verzeichnis; dafuer
/// gibt es `MYL_NVRTC`.
fn namen(umgebung: &str, unix: &[&str], windows: &[&str], rocm_lib: Option<&str>) -> Vec<String> {
    let mut n = Vec::new();
    if let Ok(p) = std::env::var(umgebung) {
        if !p.trim().is_empty() {
            n.push(p);
        }
    }
    if let (Some(datei), Ok(wurzel)) = (rocm_lib, std::env::var("ROCM_PATH")) {
        n.push(std::path::Path::new(&wurzel).join("lib").join(datei).display().to_string());
    }
    let liste = if cfg!(windows) { windows } else { unix };
    n.extend(liste.iter().map(|s| s.to_string()));
    n
}

/// Haelt eine Bibliothek fuer den ganzen Prozess offen.
fn behalten(b: Bibliothek) -> &'static Bibliothek {
    Box::leak(Box::new(b))
}

macro_rules! hole {
    ($b:expr, $name:literal) => {
        // SAFETY: Die Signatur des Feldes ist die aus der Kopfdatei des
        // Herstellers.
        unsafe { $b.symbol($name)? }
    };
}

macro_rules! vielleicht {
    ($b:expr, $name:literal) => {
        // SAFETY: wie `hole!`; fehlt das Symbol, gibt es den Weg nicht.
        unsafe { $b.symbol($name).ok() }
    };
}

fn cuda_laden() -> Result<Cuda, String> {
    let t = behalten(Bibliothek::oeffnen(&namen("MYL_CUDA_TREIBER", &["libcuda.so.1", "libcuda.so"], &["nvcuda.dll"], None))?);
    let r = behalten(Bibliothek::oeffnen(&namen(
        "MYL_NVRTC",
        &["libnvrtc.so", "libnvrtc.so.13", "libnvrtc.so.12", "libnvrtc.so.11.2"],
        &["nvrtc64_130_0.dll", "nvrtc64_120_0.dll", "nvrtc64_112_0.dll"],
        None,
    ))?);
    Ok(Cuda {
        init: hole!(t, "cuInit"),
        anzahl: hole!(t, "cuDeviceGetCount"),
        geraet: hole!(t, "cuDeviceGet"),
        name: hole!(t, "cuDeviceGetName"),
        pci: hole!(t, "cuDeviceGetPCIBusId"),
        merkmal: hole!(t, "cuDeviceGetAttribute"),
        kontext_halten: hole!(t, "cuDevicePrimaryCtxRetain"),
        kontext_setzen: hole!(t, "cuCtxSetCurrent"),
        modul_laden: hole!(t, "cuModuleLoadData"),
        funktion: hole!(t, "cuModuleGetFunction"),
        belegen: hole!(t, "cuMemAlloc_v2"),
        freigeben: hole!(t, "cuMemFree_v2"),
        hoch: hole!(t, "cuMemcpyHtoD_v2"),
        runter: hole!(t, "cuMemcpyDtoH_v2"),
        starten: hole!(t, "cuLaunchKernel"),
        warten: hole!(t, "cuCtxSynchronize"),
        fehlertext: hole!(t, "cuGetErrorString"),
        speicher: hole!(t, "cuMemGetInfo_v2"),
        rtc: Rtc {
            anlegen: hole!(r, "nvrtcCreateProgram"),
            uebersetzen: hole!(r, "nvrtcCompileProgram"),
            protokoll_groesse: hole!(r, "nvrtcGetProgramLogSize"),
            protokoll: hole!(r, "nvrtcGetProgramLog"),
            code_groesse: vielleicht!(r, "nvrtcGetCUBINSize"),
            code: vielleicht!(r, "nvrtcGetCUBIN"),
            ptx_groesse: vielleicht!(r, "nvrtcGetPTXSize"),
            ptx: vielleicht!(r, "nvrtcGetPTX"),
            loeschen: hole!(r, "nvrtcDestroyProgram"),
            fehlertext: hole!(r, "nvrtcGetErrorString"),
            bibliothek: r.name.clone(),
        },
    })
}

fn hip_laden() -> Result<Hip, String> {
    let t = behalten(Bibliothek::oeffnen(&namen(
        "MYL_HIP",
        &["libamdhip64.so", "libamdhip64.so.7", "libamdhip64.so.6", "libamdhip64.so.5"],
        &["amdhip64_7.dll", "amdhip64_6.dll", "amdhip64.dll"],
        Some("libamdhip64.so"),
    ))?);
    let r = behalten(Bibliothek::oeffnen(&namen(
        "MYL_HIPRTC",
        &["libhiprtc.so", "libhiprtc.so.7", "libhiprtc.so.6", "libhiprtc.so.5"],
        &["hiprtc0700.dll", "hiprtc0602.dll", "hiprtc0601.dll", "hiprtc0600.dll", "hiprtc.dll"],
        Some("libhiprtc.so"),
    ))?);
    // ⚑ ROCm 6 hat die Eigenschaften umbenannt; die alte Fassung ist die
    // Rueckfallstufe. Beide werden nur nach einem Namen durchsucht.
    let eigenschaften = vielleicht!(t, "hipGetDevicePropertiesR0600").or(vielleicht!(t, "hipGetDeviceProperties"));
    Ok(Hip {
        init: hole!(t, "hipInit"),
        anzahl: hole!(t, "hipGetDeviceCount"),
        geraet_setzen: hole!(t, "hipSetDevice"),
        name: hole!(t, "hipDeviceGetName"),
        pci: hole!(t, "hipDeviceGetPCIBusId"),
        eigenschaften,
        modul_laden: hole!(t, "hipModuleLoadData"),
        funktion: hole!(t, "hipModuleGetFunction"),
        belegen: hole!(t, "hipMalloc"),
        freigeben: hole!(t, "hipFree"),
        hoch: hole!(t, "hipMemcpyHtoD"),
        runter: hole!(t, "hipMemcpyDtoH"),
        starten: hole!(t, "hipModuleLaunchKernel"),
        warten: hole!(t, "hipDeviceSynchronize"),
        fehlertext: hole!(t, "hipGetErrorString"),
        speicher: hole!(t, "hipMemGetInfo"),
        rtc: Rtc {
            anlegen: hole!(r, "hiprtcCreateProgram"),
            uebersetzen: hole!(r, "hiprtcCompileProgram"),
            protokoll_groesse: hole!(r, "hiprtcGetProgramLogSize"),
            protokoll: hole!(r, "hiprtcGetProgramLog"),
            code_groesse: vielleicht!(r, "hiprtcGetCodeSize"),
            code: vielleicht!(r, "hiprtcGetCode"),
            ptx_groesse: None,
            ptx: None,
            loeschen: hole!(r, "hiprtcDestroyProgram"),
            fehlertext: hole!(r, "hiprtcGetErrorString"),
            bibliothek: r.name.clone(),
        },
    })
}

/// Die Schnittstelle eines Herstellers, einmal je Prozess geladen.
fn schnittstelle(h: Hersteller) -> Result<&'static Schnittstelle, String> {
    static CUDA: OnceLock<Result<Schnittstelle, String>> = OnceLock::new();
    static HIP: OnceLock<Result<Schnittstelle, String>> = OnceLock::new();
    let platz = match h {
        Hersteller::Nvidia => CUDA.get_or_init(|| cuda_laden().map(Schnittstelle::Cuda)),
        Hersteller::Amd => HIP.get_or_init(|| hip_laden().map(Schnittstelle::Hip)),
    };
    platz.as_ref().map_err(Clone::clone)
}

impl Schnittstelle {
    fn pruefen(&self, wo: &str, e: Ergebnis) -> Result<(), String> {
        if e == 0 {
            return Ok(());
        }
        // SAFETY: Beide Fehlertexte liefern statische Zeichenketten oder null.
        let t = unsafe {
            match self {
                Schnittstelle::Cuda(c) => {
                    let mut p: *const c_char = std::ptr::null();
                    (c.fehlertext)(e, &mut p);
                    text(p)
                }
                Schnittstelle::Hip(h) => text((h.fehlertext)(e)),
                Schnittstelle::Nachbau => "Nachbau".into(),
            }
        };
        Err(format!("{wo}: Fehler {e} ({t})"))
    }

    /// Der Uebersetzer; eine nachgebildete Karte hat keinen.
    fn rtc(&self) -> Option<&Rtc> {
        match self {
            Schnittstelle::Cuda(c) => Some(&c.rtc),
            Schnittstelle::Hip(h) => Some(&h.rtc),
            Schnittstelle::Nachbau => None,
        }
    }
}

/// Ein Kern auf einer Karte.
#[derive(Clone, Copy)]
pub(super) enum Kern {
    W8a16,
    Ternaer,
    Vierbit,
}

/// Ein behaltener Puffer auf der Karte.
#[derive(Default)]
pub(super) struct Puffer {
    zeiger: Zeiger,
    groesse: usize,
}

/// Die Arbeitspuffer einer Karte, ueber die Aufrufe hinweg behalten.
///
/// ⚑ **Warum behalten**: Belegen auf der Karte kostet je Aufruf
/// Verwaltung, und eine Vorbereitung ruft je Ebene viele Male. Jeder Puffer
/// waechst auf den groessten Bedarf und bleibt. **Keiner wird vor dem
/// Schreiben gelesen**: Jeder Aufruf laedt Gewichte, Eingaben und Abstaende
/// neu hoch, und der Kern schreibt jede Ausgabe, die zurueckkommt.
#[derive(Default)]
pub(super) struct Vorrat {
    pub gewichte: Puffer,
    pub betraege: Puffer,
    pub eingaben: Puffer,
    pub abstaende: Puffer,
    pub ausgaben: Puffer,
}

/// **Eine Karte**, bereit zum Rechnen.
pub(super) struct Karte {
    pub hersteller: Hersteller,
    pub nummer: i32,
    pub name: String,
    /// Die PCI-Adresse, vereinheitlicht ([`super::pci_normal`]): der Schluessel,
    /// unter dem der Client eine Karte freigibt oder abschaltet.
    pub pci: String,
    /// Wofuer uebersetzt wurde (`sm_90`, `gfx942:…`), fuer Meldungen.
    pub architektur: String,
    /// Der Anteil an der Arbeit, ganzzahlig. Siehe `verteiler`.
    pub gewicht: u32,
    /// Freier und ganzer Speicher beim Aufbau, in Byte.
    pub speicher: (usize, usize),
    api: &'static Schnittstelle,
    /// Der Hauptkontext (nur CUDA), als Zahl gehalten.
    kontext: usize,
    w8a16: usize,
    ternaer: usize,
    vierbit: usize,
    /// ⚑ **Ein Aufruf zur Zeit je Karte.** Haelt die Puffer.
    pub sperre: Mutex<Vorrat>,
    /// Gerechnete Teile und Zeilen, ohne die Selbstpruefung.
    pub teile: AtomicUsize,
    pub zeilen: AtomicUsize,
    /// Mitten im Lauf ausgefallen; danach rechnet sie nicht mehr mit.
    pub ausgefallen: AtomicBool,
}

// SAFETY: Kontext und Funktionen sind Griffe des Treibers, die von jedem
// Faden aus benutzt werden duerfen, sobald der Kontext dort gesetzt ist
// (`aktivieren`). Die Puffer liegen in der Sperre.
unsafe impl Send for Karte {}
unsafe impl Sync for Karte {}

impl Karte {
    /// Ob die Karte gerade rechnen darf: Hersteller an, Karte an, nicht
    /// ausgefallen.
    /// Ob dies eine nachgebildete Karte ist (`MYL_GPU_NACHBAU`).
    pub fn nachgebildet(&self) -> bool {
        matches!(self.api, Schnittstelle::Nachbau)
    }

    pub fn darf_rechnen(&self) -> bool {
        super::hersteller_an(self.hersteller) && super::karte_an(&self.pci) && !self.ausgefallen.load(Ordering::Relaxed)
    }

    /// **Setzt die Karte fuer diesen Faden.** Vor jedem Aufruf, denn CUDA
    /// und HIP merken sich die Karte je Faden.
    pub fn aktivieren(&self) -> Result<(), String> {
        // SAFETY: Kontext aus `cuDevicePrimaryCtxRetain` bzw. eine gueltige
        // Kartennummer.
        let e = unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => (c.kontext_setzen)(self.kontext as *mut c_void),
                Schnittstelle::Hip(h) => (h.geraet_setzen)(self.nummer),
                Schnittstelle::Nachbau => 0,
            }
        };
        self.api.pruefen("Karte setzen", e)
    }

    /// Sorgt dafuer, dass `platz` mindestens `bytes` fasst.
    pub fn mindestens(&self, platz: &mut Puffer, bytes: usize) -> Result<Zeiger, String> {
        let bytes = bytes.max(1);
        if platz.groesse >= bytes {
            return Ok(platz.zeiger);
        }
        if platz.groesse > 0 {
            self.freigeben(platz.zeiger);
            *platz = Puffer::default();
        }
        let z = self.belegen(bytes)?;
        *platz = Puffer { zeiger: z, groesse: bytes };
        Ok(z)
    }

    fn belegen(&self, bytes: usize) -> Result<Zeiger, String> {
        // SAFETY: Die Zielvariable lebt ueber den Aufruf hinaus.
        unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => {
                    let mut z: u64 = 0;
                    self.api.pruefen(&format!("{bytes} Byte belegen"), (c.belegen)(&mut z, bytes))?;
                    Ok(z)
                }
                Schnittstelle::Hip(h) => {
                    let mut z: *mut c_void = std::ptr::null_mut();
                    self.api.pruefen(&format!("{bytes} Byte belegen"), (h.belegen)(&mut z, bytes))?;
                    Ok(z as u64)
                }
                Schnittstelle::Nachbau => super::nachbaukarte::belegen(bytes),
            }
        }
    }

    fn freigeben(&self, z: Zeiger) {
        // SAFETY: `z` stammt aus `belegen` und wird danach nicht mehr benutzt.
        // Ein Fehler beim Freigeben laesst nur Speicher liegen.
        unsafe {
            let _ = match self.api {
                Schnittstelle::Cuda(c) => (c.freigeben)(z),
                Schnittstelle::Hip(h) => (h.freigeben)(z as *mut c_void),
                Schnittstelle::Nachbau => {
                    super::nachbaukarte::freigeben(z);
                    0
                }
            };
        }
    }

    /// Kopiert `daten` an die Adresse `ziel` auf der Karte.
    pub fn hoch(&self, ziel: Zeiger, daten: &[u8]) -> Result<(), String> {
        if daten.is_empty() {
            return Ok(());
        }
        // SAFETY: `ziel` fasst mindestens `daten.len()` Byte (der Aufrufer
        // hat ihn mit `mindestens` darauf gebracht); die Kopie ist fertig,
        // wenn der Aufruf zurueckkehrt.
        let e = unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => (c.hoch)(ziel, daten.as_ptr() as *const c_void, daten.len()),
                Schnittstelle::Hip(h) => (h.hoch)(ziel as *mut c_void, daten.as_ptr() as *const c_void, daten.len()),
                Schnittstelle::Nachbau => return super::nachbaukarte::hoch(ziel, daten),
            }
        };
        self.api.pruefen("hochladen", e)
    }

    /// Kopiert von der Adresse `quelle` auf der Karte nach `ziel`.
    pub fn runter(&self, ziel: &mut [u8], quelle: Zeiger) -> Result<(), String> {
        if ziel.is_empty() {
            return Ok(());
        }
        // SAFETY: wie `hoch`, in die andere Richtung.
        let e = unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => (c.runter)(ziel.as_mut_ptr() as *mut c_void, quelle, ziel.len()),
                Schnittstelle::Hip(h) => {
                    (h.runter)(ziel.as_mut_ptr() as *mut c_void, quelle as *mut c_void, ziel.len())
                }
                Schnittstelle::Nachbau => return super::nachbaukarte::runter(ziel, quelle),
            }
        };
        self.api.pruefen("herunterladen", e)
    }

    /// **Startet einen Kern** ueber `zeilen` x `stapel`, in Fadengruppen zu
    /// 16 x 16 wie in `kerne.cu`.
    ///
    /// Die Argumente in der Reihenfolge des Kerns: `myl_w8a16(w, x,
    /// abstaende, aus, zeilen, spalten, stapel)`, `myl_ternaer(muster,
    /// betraege, x, abstaende, aus, zeilen, spalten, stapel)` bzw.
    /// `myl_vierbit(codes, gitter, x, abstaende, aus, zeilen, spalten, stapel)`.
    pub fn starten(&self, kern: Kern, zeiger: &[Zeiger], zeilen: usize, spalten: usize, stapel: usize) -> Result<(), String> {
        let gitter_x = zeilen.div_ceil(KACHEL_ZEILEN);
        let gitter_y = stapel.div_ceil(KACHEL_EINGABEN);
        if gitter_x == 0 || gitter_y == 0 {
            return Ok(());
        }
        if self.nachgebildet() {
            return super::nachbaukarte::starten(kern, zeiger, zeilen, spalten, stapel);
        }
        let ganz = |n: usize| i32::try_from(n).map_err(|_| format!("{n} passt nicht in int32"));
        let (z, s, b) = (ganz(zeilen)?, ganz(spalten)?, ganz(stapel)?);
        let mut werte: Vec<u64> = zeiger.to_vec();
        let mut zahlen: [i32; 3] = [z, s, b];
        let mut argumente: Vec<*mut c_void> = Vec::with_capacity(werte.len() + 3);
        for w in werte.iter_mut() {
            argumente.push(w as *mut u64 as *mut c_void);
        }
        for n in zahlen.iter_mut() {
            argumente.push(n as *mut i32 as *mut c_void);
        }
        let f = match kern {
            Kern::W8a16 => self.w8a16,
            Kern::Ternaer => self.ternaer,
            Kern::Vierbit => self.vierbit,
        } as *mut c_void;
        // SAFETY: `argumente` zeigt auf Werte, die bis nach dem Aufruf leben,
        // in der Reihenfolge und Groesse der Kernsignatur (Adressen 8 Byte,
        // Zahlen int32). Der Treiber kopiert sie beim Start.
        let e = unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => (c.starten)(
                    f,
                    gitter_x as c_uint,
                    gitter_y as c_uint,
                    1,
                    FAEDEN_X,
                    FAEDEN_Y,
                    1,
                    0,
                    std::ptr::null_mut(),
                    argumente.as_mut_ptr(),
                    std::ptr::null_mut(),
                ),
                Schnittstelle::Hip(h) => (h.starten)(
                    f,
                    gitter_x as c_uint,
                    gitter_y as c_uint,
                    1,
                    FAEDEN_X,
                    FAEDEN_Y,
                    1,
                    0,
                    std::ptr::null_mut(),
                    argumente.as_mut_ptr(),
                    std::ptr::null_mut(),
                ),
                Schnittstelle::Nachbau => 0,
            }
        };
        self.api.pruefen("Kern starten", e)
    }

    /// Wartet, bis alles auf dieser Karte fertig ist.
    pub fn warten(&self) -> Result<(), String> {
        // SAFETY: ohne Argumente.
        let e = unsafe {
            match self.api {
                Schnittstelle::Cuda(c) => (c.warten)(),
                Schnittstelle::Hip(h) => (h.warten)(),
                Schnittstelle::Nachbau => 0,
            }
        };
        self.api.pruefen("warten", e)
    }
}

/// **Alle Karten eines Herstellers**, aufgebaut und uebersetzt, aber noch
/// nicht geprueft. Dazu die Gruende fuer jede Karte, die fehlt.
///
/// `Err` heisst: Der Hersteller ist auf dieser Maschine nicht da (keine
/// Bibliothek, kein Treiber, keine Karte).
pub(super) fn karten(h: Hersteller) -> Result<(Vec<Karte>, Vec<String>), String> {
    let api = schnittstelle(h)?;
    let mut karten = Vec::new();
    let mut gruende = Vec::new();
    // SAFETY: Initialisierung ohne Zeiger, Anzahl in eine lokale Variable.
    let anzahl = unsafe {
        let mut n: c_int = 0;
        match api {
            Schnittstelle::Cuda(c) => {
                api.pruefen("cuInit", (c.init)(0))?;
                api.pruefen("Karten zaehlen", (c.anzahl)(&mut n))?;
            }
            Schnittstelle::Hip(hip) => {
                api.pruefen("hipInit", (hip.init)(0))?;
                api.pruefen("Karten zaehlen", (hip.anzahl)(&mut n))?;
            }
            Schnittstelle::Nachbau => {}
        }
        n
    };
    if anzahl <= 0 {
        return Err("der Treiber meldet keine Karte".into());
    }
    let mut uebersetzt: Vec<(String, Vec<u8>)> = Vec::new();
    for nummer in 0..anzahl {
        match karte_aufbauen(h, api, nummer, &mut uebersetzt) {
            Ok(k) => karten.push(k),
            Err(grund) => gruende.push(format!("{} Karte {nummer}: {grund}", h.rechenweg())),
        }
    }
    Ok((karten, gruende))
}

fn karte_aufbauen(
    h: Hersteller,
    api: &'static Schnittstelle,
    nummer: i32,
    uebersetzt: &mut Vec<(String, Vec<u8>)>,
) -> Result<Karte, String> {
    let mut name = [0 as c_char; 256];
    let mut pci = [0 as c_char; 64];
    let (kontext, architektur, gewicht) = match api {
        Schnittstelle::Cuda(c) => {
            // SAFETY: lokale Zielvariablen, gueltige Nummer aus der Zaehlung.
            unsafe {
                let mut g: c_int = 0;
                api.pruefen("Karte holen", (c.geraet)(&mut g, nummer))?;
                api.pruefen("Name", (c.name)(name.as_mut_ptr(), name.len() as c_int, g))?;
                api.pruefen("PCI-Adresse", (c.pci)(pci.as_mut_ptr(), pci.len() as c_int, g))?;
                let (mut haupt, mut neben, mut sm) = (0, 0, 0);
                api.pruefen("Faehigkeit", (c.merkmal)(&mut haupt, MERKMAL_FAEHIGKEIT_HAUPT, g))?;
                api.pruefen("Faehigkeit", (c.merkmal)(&mut neben, MERKMAL_FAEHIGKEIT_NEBEN, g))?;
                api.pruefen("Multiprozessoren", (c.merkmal)(&mut sm, MERKMAL_MULTIPROZESSOREN, g))?;
                let mut k: *mut c_void = std::ptr::null_mut();
                api.pruefen("Kontext", (c.kontext_halten)(&mut k, g))?;
                api.pruefen("Kontext setzen", (c.kontext_setzen)(k))?;
                (k as usize, format!("{haupt}{neben}"), sm.max(1) as u32)
            }
        }
        Schnittstelle::Nachbau => return Err("eine nachgebildete Karte wird nicht aufgebaut".into()),
        Schnittstelle::Hip(hip) => {
            // SAFETY: wie oben.
            unsafe {
                api.pruefen("Karte setzen", (hip.geraet_setzen)(nummer))?;
                api.pruefen("Name", (hip.name)(name.as_mut_ptr(), name.len() as c_int, nummer))?;
                api.pruefen("PCI-Adresse", (hip.pci)(pci.as_mut_ptr(), pci.len() as c_int, nummer))?;
            }
            (0, hip_architektur(hip, nummer), 1)
        }
    };
    // SAFETY: `name` und `pci` sind nullterminiert (vorbelegt mit Nullen,
    // der Treiber schreibt hoechstens einen Platz weniger).
    let name = unsafe { text(name.as_ptr()) };
    let pci = super::pci_normal(&unsafe { text(pci.as_ptr()) });
    let code = match uebersetzt.iter().find(|(a, _)| *a == architektur) {
        Some((_, c)) => c.clone(),
        None => {
            let c = uebersetzen(api, h, &architektur)?;
            uebersetzt.push((architektur.clone(), c.clone()));
            c
        }
    };
    let (w8a16, ternaer, vierbit) = laden(api, &code)?;
    let speicher = {
        let (mut frei, mut ganz) = (0usize, 0usize);
        // SAFETY: lokale Zielvariablen; die Karte ist gesetzt.
        let e = unsafe {
            match api {
                Schnittstelle::Cuda(c) => (c.speicher)(&mut frei, &mut ganz),
                Schnittstelle::Hip(hip) => (hip.speicher)(&mut frei, &mut ganz),
                Schnittstelle::Nachbau => 1,
            }
        };
        if e == 0 { (frei, ganz) } else { (0, 0) }
    };
    Ok(Karte {
        hersteller: h,
        nummer,
        name,
        pci,
        architektur: match h {
            Hersteller::Nvidia => format!("sm_{architektur}"),
            Hersteller::Amd => architektur,
        },
        gewicht,
        speicher,
        api,
        kontext,
        w8a16,
        ternaer,
        vierbit,
        sperre: Mutex::new(Vorrat::default()),
        teile: AtomicUsize::new(0),
        zeilen: AtomicUsize::new(0),
        ausgefallen: AtomicBool::new(false),
    })
}

/// **Nachgebildete Karten** (`MYL_GPU_NACHBAU`), numeriert ab null, mit
/// der Adresse `nachbau:N`. Siehe `super::nachbaukarte`.
pub(super) fn nachbaukarten(anzahl: usize, h: Hersteller) -> Vec<Karte> {
    (0..anzahl)
        .map(|i| Karte {
            hersteller: h,
            nummer: i as i32,
            name: format!("Nachbau {i}"),
            pci: format!("nachbau:{i}"),
            architektur: "nachbau".into(),
            gewicht: 1,
            speicher: (0, 0),
            api: &NACHBAU,
            kontext: 0,
            w8a16: 0,
            ternaer: 0,
            vierbit: 0,
            sperre: Mutex::new(Vorrat::default()),
            teile: AtomicUsize::new(0),
            zeilen: AtomicUsize::new(0),
            ausgefallen: AtomicBool::new(false),
        })
        .collect()
}

/// **Der Architekturname einer AMD-Karte** (`gfx942:sramecc+:xnack-`).
///
/// ⚑ **Gesucht statt gelesen.** Die Eigenschaften sind eine Struktur,
/// deren Aufbau sich zwischen ROCm-Fassungen aendert; ein fester Versatz
/// waere eine Wette auf eine Fassung. Der Name steht darin als
/// Zeichenkette mit `gfx` vorn, und genau danach wird gesucht. Der Puffer
/// ist weit groesser als jede bekannte Fassung der Struktur.
///
/// ⚠️ Findet sich nichts, bleibt die Angabe leer, und hiprtc uebersetzt
/// fuer die gesetzte Karte. `MYL_HIP_ARCH` geht beidem vor.
fn hip_architektur(hip: &Hip, nummer: i32) -> String {
    if let Ok(a) = std::env::var("MYL_HIP_ARCH") {
        if !a.trim().is_empty() {
            return a.trim().to_string();
        }
    }
    let Some(eigenschaften) = hip.eigenschaften else {
        return String::new();
    };
    let mut puffer = vec![0u8; 64 * 1024];
    // SAFETY: Der Puffer ist groesser als jede Fassung der Struktur.
    if unsafe { eigenschaften(puffer.as_mut_ptr() as *mut c_void, nummer) } != 0 {
        return String::new();
    }
    architektur_suchen(&puffer)
}

/// Sucht `gfx` gefolgt von Ziffern oder Kleinbuchstaben, bis zur Null.
pub(super) fn architektur_suchen(puffer: &[u8]) -> String {
    let mut i = 0;
    while i + 4 <= puffer.len() {
        if &puffer[i..i + 3] == b"gfx" && puffer[i + 3].is_ascii_alphanumeric() {
            let ende = puffer[i..].iter().position(|&b| b == 0).map(|p| i + p).unwrap_or(puffer.len());
            let kandidat = &puffer[i..ende];
            if kandidat.iter().all(|&b| b.is_ascii_alphanumeric() || b == b':' || b == b'+' || b == b'-') {
                return String::from_utf8_lossy(kandidat).into_owned();
            }
        }
        i += 1;
    }
    String::new()
}

/// **Uebersetzt die Kerne** fuer eine Architektur und gibt den Code zurueck,
/// den der Treiber laedt.
fn uebersetzen(api: &Schnittstelle, h: Hersteller, architektur: &str) -> Result<Vec<u8>, String> {
    let rtc = api.rtc().ok_or("ohne Uebersetzer")?;
    let quelle = std::ffi::CString::new(QUELLE).expect("kerne.cu ohne Nullbyte");
    let dateiname = c"kerne.cu";
    let rtc_pruefen = |wo: &str, e: Ergebnis| -> Result<(), String> {
        if e == 0 {
            Ok(())
        } else {
            // SAFETY: statische Zeichenkette oder null.
            Err(format!("{wo} ({}): {}", rtc.bibliothek, unsafe { text((rtc.fehlertext)(e)) }))
        }
    };
    // ⚑ **Erst Maschinencode, dann PTX.** CUBIN fuer `sm_XY` laeuft auf
    // jedem Treiber, der die Karte kennt; PTX braucht einen Treiber, der
    // die PTX-Fassung des Uebersetzers versteht, und ist deshalb nur der
    // Rueckfall.
    let versuche: Vec<(Vec<String>, bool)> = match h {
        Hersteller::Nvidia => vec![
            (vec![format!("--gpu-architecture=sm_{architektur}")], true),
            (vec![format!("--gpu-architecture=compute_{architektur}")], false),
        ],
        Hersteller::Amd if architektur.is_empty() => vec![(Vec::new(), true)],
        Hersteller::Amd => vec![(vec![format!("--offload-arch={architektur}")], true)],
    };
    let mut gruende = Vec::new();
    for (optionen, maschinencode) in versuche {
        if maschinencode && (rtc.code_groesse.is_none() || rtc.code.is_none()) {
            gruende.push("kein Maschinencode in dieser Fassung".to_string());
            continue;
        }
        if !maschinencode && (rtc.ptx_groesse.is_none() || rtc.ptx.is_none()) {
            continue;
        }
        let c_opt: Vec<std::ffi::CString> =
            optionen.iter().map(|o| std::ffi::CString::new(o.as_str()).expect("Option ohne Nullbyte")).collect();
        let zeiger: Vec<*const c_char> = c_opt.iter().map(|o| o.as_ptr()).collect();
        let mut programm: *mut c_void = std::ptr::null_mut();
        // SAFETY: Quelle und Name sind nullterminiert und leben ueber den
        // Aufruf; keine Kopfdateien.
        rtc_pruefen("Programm anlegen", unsafe {
            (rtc.anlegen)(&mut programm, quelle.as_ptr(), dateiname.as_ptr(), 0, std::ptr::null(), std::ptr::null())
        })?;
        // SAFETY: `programm` ist frisch angelegt; die Optionen leben bis hier.
        let e = unsafe { (rtc.uebersetzen)(programm, zeiger.len() as c_int, zeiger.as_ptr()) };
        let ergebnis = if e != 0 {
            Err(format!("{}: {}", optionen.join(" "), protokoll(rtc, programm)))
        } else {
            let (groesse, holen) = if maschinencode {
                (rtc.code_groesse.expect("geprueft"), rtc.code.expect("geprueft"))
            } else {
                (rtc.ptx_groesse.expect("geprueft"), rtc.ptx.expect("geprueft"))
            };
            let mut n = 0usize;
            // SAFETY: Groesse in eine lokale Variable, dann genau so viele
            // Byte in einen Puffer dieser Groesse.
            unsafe {
                rtc_pruefen("Codegroesse", groesse(programm, &mut n)).and_then(|_| {
                    let mut code = vec![0u8; n];
                    rtc_pruefen("Code holen", holen(programm, code.as_mut_ptr() as *mut c_char)).map(|_| code)
                })
            }
        };
        // SAFETY: Das Programm wird danach nicht mehr benutzt.
        unsafe {
            (rtc.loeschen)(&mut programm);
        }
        match ergebnis {
            Ok(mut code) => {
                // PTX ist Text und muss nullterminiert beim Treiber ankommen.
                if !maschinencode && code.last() != Some(&0) {
                    code.push(0);
                }
                return Ok(code);
            }
            Err(g) => gruende.push(g),
        }
    }
    Err(format!("Kerne nicht uebersetzbar: {}", gruende.join("; ")))
}

fn protokoll(rtc: &Rtc, programm: *mut c_void) -> String {
    let mut n = 0usize;
    // SAFETY: wie beim Code: erst die Groesse, dann ein Puffer genau dieser
    // Groesse.
    unsafe {
        if (rtc.protokoll_groesse)(programm, &mut n) != 0 || n == 0 {
            return "ohne Protokoll".into();
        }
        let mut t = vec![0u8; n];
        if (rtc.protokoll)(programm, t.as_mut_ptr() as *mut c_char) != 0 {
            return "ohne Protokoll".into();
        }
        let t = String::from_utf8_lossy(&t).trim_end_matches('\0').trim().to_string();
        // Die ersten Zeilen genuegen; ein Protokoll kann lang sein.
        t.lines().take(20).collect::<Vec<_>>().join("\n")
    }
}

/// Laedt den Code als Modul auf der gesetzten Karte und holt die drei Kerne.
fn laden(api: &Schnittstelle, code: &[u8]) -> Result<(usize, usize, usize), String> {
    let mut modul: *mut c_void = std::ptr::null_mut();
    let (laden, funktion) = match api {
        Schnittstelle::Cuda(c) => (c.modul_laden, c.funktion),
        Schnittstelle::Hip(h) => (h.modul_laden, h.funktion),
        Schnittstelle::Nachbau => return Err("eine nachgebildete Karte laedt kein Modul".into()),
    };
    // SAFETY: `code` ist ein vollstaendiges Modul aus dem Uebersetzer; das
    // Modul bleibt geladen, solange der Prozess lebt.
    api.pruefen("Modul laden", unsafe { laden(&mut modul, code.as_ptr() as *const c_void) })?;
    let holen = |name: &std::ffi::CStr| -> Result<usize, String> {
        let mut f: *mut c_void = std::ptr::null_mut();
        // SAFETY: Modul eben geladen, Name nullterminiert.
        api.pruefen(&format!("Kern {name:?}"), unsafe { funktion(&mut f, modul, name.as_ptr()) })?;
        Ok(f as usize)
    };
    Ok((holen(c"myl_w8a16")?, holen(c"myl_ternaer")?, holen(c"myl_vierbit")?))
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Der Architekturname wird gefunden**, wo immer er in der Struktur
    /// steht, und der Kartenname davor stoert nicht.
    #[test]
    fn die_architektur_wird_gesucht() {
        let mut p = vec![0u8; 2048];
        p[..19].copy_from_slice(b"AMD Instinct MI300X");
        p[1100..1122].copy_from_slice(b"gfx942:sramecc+:xnack-");
        assert_eq!(architektur_suchen(&p), "gfx942:sramecc+:xnack-");
        assert_eq!(architektur_suchen(&[0u8; 64]), "");
        // Ein Name, der nur `gfx` enthaelt und dann Unsinn, zaehlt nicht.
        let mut q = vec![0u8; 64];
        q[..6].copy_from_slice(b"gfx $x");
        assert_eq!(architektur_suchen(&q), "");
    }

    /// ⚑ **Eine ausdrueckliche Angabe geht vor**, dann die Liste.
    #[test]
    fn die_namen_beginnen_mit_der_angabe() {
        let n = namen("MYL_GIBT_ES_NICHT_PROBE", &["a.so"], &["a.dll"], None);
        assert_eq!(n.len(), 1);
        assert!(n[0] == "a.so" || n[0] == "a.dll");
    }
}
