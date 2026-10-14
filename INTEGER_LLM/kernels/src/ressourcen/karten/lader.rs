//! **Eine Bibliothek zur Laufzeit oeffnen** und Funktionen daraus holen.
//!
//! # ⚑ Warum zur Laufzeit und nicht beim Bauen
//!
//! Ein Bau gegen das CUDA-Toolkit oder ROCm braucht beides auf der
//! Baumaschine. Hier wird nur gebaut, was im Repositorium liegt; die
//! Treiber kommen erst auf der Maschine dazu, die eine Karte hat. **Fehlt
//! eine Bibliothek, kostet das die Karte, nie den Bau.**
//!
//! # ⚑ Warum eigener Code und keine Kiste
//!
//! `libloading` laege im Kistenlager. Gebraucht werden aber drei
//! Systemaufrufe je Plattform (`dlopen`, `dlsym`, `dlerror` bzw.
//! `LoadLibraryW`, `GetProcAddress`, `GetLastError`), und eine Kiste mehr
//! hiesse fremden Code im Konsenspfad und eine Zeile mehr in jeder
//! Sperrdatei, die von den Kernen abhaengt.
//!
//! # ⚠️ Eine geoeffnete Bibliothek wird nie geschlossen
//!
//! Die Funktionszeiger leben in einem statischen Zustand bis zum Ende des
//! Prozesses. Ein Schliessen davor liesse sie ins Leere zeigen.

use std::ffi::{c_char, c_void, CStr, CString};

/// Eine geoeffnete Bibliothek.
pub(super) struct Bibliothek {
    griff: *mut c_void,
    /// Unter welchem Namen sie gefunden wurde, fuer Meldungen.
    pub name: String,
}

// SAFETY: Der Griff ist nur ein Schluessel fuer den Lader des Systems; er
// wird nach dem Oeffnen nur noch gelesen, und `dlsym`/`GetProcAddress`
// sind von jedem Faden aus aufrufbar.
unsafe impl Send for Bibliothek {}
unsafe impl Sync for Bibliothek {}

#[cfg(unix)]
mod system {
    use std::ffi::{c_char, c_int, c_void};

    /// Sofort alle Verweise aufloesen: Ein fehlendes Symbol faellt beim
    /// Oeffnen auf und nicht beim ersten Aufruf mitten in einer Rechnung.
    pub const SOFORT: c_int = 2;

    #[cfg_attr(all(target_os = "linux", target_env = "gnu"), link(name = "dl"))]
    extern "C" {
        pub fn dlopen(name: *const c_char, art: c_int) -> *mut c_void;
        pub fn dlsym(griff: *mut c_void, name: *const c_char) -> *mut c_void;
        pub fn dlerror() -> *mut c_char;
    }
}

#[cfg(windows)]
mod system {
    use std::ffi::{c_char, c_void};

    #[link(name = "kernel32")]
    extern "system" {
        pub fn LoadLibraryW(name: *const u16) -> *mut c_void;
        pub fn GetProcAddress(griff: *mut c_void, name: *const c_char) -> *mut c_void;
        pub fn GetLastError() -> u32;
    }
}

impl Bibliothek {
    /// **Oeffnet die erste der genannten Bibliotheken, die sich oeffnen
    /// laesst.** Ein Name ohne Pfad geht an den Suchweg des Systems
    /// (`LD_LIBRARY_PATH`, `ldconfig`, `PATH` unter Windows).
    ///
    /// Scheitern alle, nennt der Fehler jeden Versuch mit seinem Grund.
    pub fn oeffnen(namen: &[String]) -> Result<Self, String> {
        let mut gruende = Vec::new();
        for name in namen {
            match Self::eine(name) {
                Ok(griff) => return Ok(Bibliothek { griff, name: name.clone() }),
                Err(grund) => gruende.push(format!("{name}: {}", kurz(&grund))),
            }
        }
        Err(gruende.join("; "))
    }

    #[cfg(unix)]
    fn eine(name: &str) -> Result<*mut c_void, String> {
        let c = CString::new(name).map_err(|_| "Nullbyte im Namen".to_string())?;
        // SAFETY: `c` ist nullterminiert und lebt ueber den Aufruf hinaus.
        let griff = unsafe { system::dlopen(c.as_ptr(), system::SOFORT) };
        if griff.is_null() {
            return Err(letzter_fehler());
        }
        Ok(griff)
    }

    #[cfg(windows)]
    fn eine(name: &str) -> Result<*mut c_void, String> {
        let breit: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `breit` ist nullterminiert und lebt ueber den Aufruf hinaus.
        let griff = unsafe { system::LoadLibraryW(breit.as_ptr()) };
        if griff.is_null() {
            return Err(letzter_fehler());
        }
        Ok(griff)
    }

    #[cfg(not(any(unix, windows)))]
    fn eine(_: &str) -> Result<*mut c_void, String> {
        Err("auf diesem System gibt es keinen Lader".into())
    }

    /// **Holt eine Funktion.** `T` muss ein `extern "C"`-Funktionszeiger
    /// mit genau der Signatur sein, die die Bibliothek ausfuehrt.
    ///
    /// # Safety
    ///
    /// Der Aufrufer sichert zu, dass `T` die richtige Signatur traegt. Der
    /// Lader kennt nur Namen, keine Typen.
    pub unsafe fn symbol<T: Copy>(&self, name: &str) -> Result<T, String> {
        assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*mut c_void>(),
            "symbol: T ist kein Funktionszeiger"
        );
        let c = CString::new(name).map_err(|_| "Nullbyte im Namen".to_string())?;
        let zeiger = self.roh(&c);
        if zeiger.is_null() {
            return Err(format!("{} kennt {name} nicht", self.name));
        }
        // SAFETY: gleich gross (oben geprueft); die Signatur sichert der
        // Aufrufer zu.
        Ok(std::mem::transmute_copy::<*mut c_void, T>(&zeiger))
    }

    #[cfg(unix)]
    fn roh(&self, name: &CStr) -> *mut c_void {
        // SAFETY: Griff aus `dlopen`, Name nullterminiert.
        unsafe { system::dlsym(self.griff, name.as_ptr()) }
    }

    #[cfg(windows)]
    fn roh(&self, name: &CStr) -> *mut c_void {
        // SAFETY: Griff aus `LoadLibraryW`, Name nullterminiert.
        unsafe { system::GetProcAddress(self.griff, name.as_ptr()) }
    }

    #[cfg(not(any(unix, windows)))]
    fn roh(&self, _: &CStr) -> *mut c_void {
        std::ptr::null_mut()
    }
}

/// **Ein Grund, den ein Mensch liest.** macOS zaehlt bei einer fehlenden
/// Bibliothek jeden Suchort auf, ein Dutzend Pfade je Name; das sagt
/// nichts ausser „nicht gefunden“. Jeder andere Grund (falsche
/// Architektur, fehlendes Symbol einer Abhaengigkeit) bleibt ganz stehen,
/// denn genau den braucht, wer auf einer Mietmaschine nachsieht.
fn kurz(grund: &str) -> String {
    let fehlt = ["no such file", "No such file", "cannot open shared object file", "Fehler 126"];
    if fehlt.iter().any(|f| grund.contains(f)) {
        "nicht gefunden".into()
    } else {
        grund.to_string()
    }
}

#[cfg(unix)]
fn letzter_fehler() -> String {
    // SAFETY: `dlerror` liefert einen Zeiger in faden-eigenen Speicher oder
    // null; gelesen wird sofort.
    unsafe {
        let p = system::dlerror();
        if p.is_null() {
            "unbekannter Fehler".into()
        } else {
            CStr::from_ptr(p as *const c_char).to_string_lossy().into_owned()
        }
    }
}

#[cfg(windows)]
fn letzter_fehler() -> String {
    // SAFETY: ohne Argumente, liest nur den Fehlerstand des Fadens.
    format!("Fehler {}", unsafe { system::GetLastError() })
}

/// Liest eine nullterminierte Zeichenkette aus einer Bibliothek.
///
/// # Safety
///
/// `p` ist null oder zeigt auf eine nullterminierte Zeichenkette, die
/// waehrend des Aufrufs gueltig bleibt.
pub(super) unsafe fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    CStr::from_ptr(p).to_string_lossy().into_owned()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Eine fehlende Bibliothek ist ein Fehler mit Namen**, kein
    /// Absturz, und jeder Versuch steht in der Meldung.
    #[test]
    fn eine_fehlende_bibliothek_nennt_jeden_versuch() {
        let f = Bibliothek::oeffnen(&["libgibtesnicht-myl-1.so".into(), "libgibtesnicht-myl-2.so".into()])
            .map(|b| b.name)
            .expect_err("eine erfundene Bibliothek liess sich oeffnen");
        assert!(f.contains("libgibtesnicht-myl-1.so") && f.contains("libgibtesnicht-myl-2.so"), "{f}");
        assert!(f.contains("nicht gefunden") && f.len() < 200, "zu lang oder ohne Grund: {f}");
    }

    /// ⚑ **Eine vorhandene Systembibliothek oeffnet sich, und ein
    /// fehlendes Symbol wird benannt.** Auf Unix die C-Bibliothek.
    #[cfg(unix)]
    #[test]
    fn die_c_bibliothek_oeffnet_sich() {
        let namen: Vec<String> = if cfg!(target_os = "macos") {
            vec!["libSystem.B.dylib".into()]
        } else {
            vec!["libc.so.6".into(), "libc.so".into()]
        };
        let b = Bibliothek::oeffnen(&namen).expect("C-Bibliothek nicht zu oeffnen");
        // SAFETY: `getpid` hat diese Signatur.
        let getpid: extern "C" fn() -> i32 = unsafe { b.symbol("getpid") }.expect("getpid fehlt");
        assert_eq!(getpid() as u32, std::process::id());
        // SAFETY: wird nicht aufgerufen.
        let f = unsafe { b.symbol::<extern "C" fn()>("gibt_es_nicht_myl") }.expect_err("erfundenes Symbol");
        assert!(f.contains("gibt_es_nicht_myl"), "{f}");
    }
}
