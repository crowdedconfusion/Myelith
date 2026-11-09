//! **Die Seite des Moduls**: eine Ereignisschleife fuer ein natives Modul.
//!
//! Ein Modul implementiert [`Modul`] und ruft [`nativ_fahren`]. Jede Zeile
//! auf stdin ist ein Ereignis ([`AnModul`]), jede Antwort eine Zeile auf
//! stdout ([`VomModul`]). ⛔️ **Auf stdout darf nichts anderes stehen**,
//! sonst versteht die Konsole das Modul nicht mehr; Protokollzeilen gehen
//! nach stderr oder als [`VomModul::Protokoll`].

use crate::nachricht::{self, AnModul, Stil, VomModul};
use std::io::{BufRead, Write};

/// **Was ein Modul tut.**
pub trait Modul {
    /// Name und Version, wie in der Beschreibung.
    fn name(&self) -> (&str, &str);
    /// Ein Ereignis; Antworten gehen in `aus`.
    fn ereignis(&mut self, e: AnModul, aus: &mut Ausgang);
}

/// **Die Antworten auf ein Ereignis**, gesammelt und danach geschrieben.
#[derive(Debug, Default)]
pub struct Ausgang {
    pub nachrichten: Vec<VomModul>,
}

impl Ausgang {
    pub fn senden(&mut self, n: VomModul) {
        self.nachrichten.push(n);
    }

    /// Text in den Rollbereich.
    pub fn zeilen(&mut self, text: impl Into<String>, stil: Stil) {
        let text = text.into();
        if !text.trim().is_empty() {
            self.senden(VomModul::Zeilen { text, stil });
        }
    }

    /// Bitte um den naechsten Takt.
    pub fn wecken_in(&mut self, ms: u64) {
        self.senden(VomModul::WeckenIn { ms });
    }
}

/// **Wie ein Modul einen eigenen Befehl in seinem Text nennt**, je nach
/// Wirt.
///
/// - In der Konsole (`fenster = false`) steht er unter [`crate::MODULBEFEHL`]:
///   `/trading off` wird `/module trading off`.
/// - ⛔️ Im Fenster (`fenster = true`, der Handschlag sagt `ansicht`) gibt es
///   keine /Befehle; dort steht er so, wie die Liste hinter dem Pfeil an der
///   Modulwahl ihn nennt: `/trading off` wird „off“, der Hauptbefehl allein
///   heisst wie das Modul („trading“).
///
/// ⚑ Eine Stelle fuer alle Module, damit keines seine Befehle anders nennt
/// als die Wirte sie zeigen.
pub fn befehl_beim_wirt(zeile: &str, fenster: bool) -> String {
    let rest = zeile.trim().trim_start_matches('/');
    if !fenster {
        return format!("{} {rest}", crate::MODULBEFEHL);
    }
    let (haupt, unter) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    format!("„{}“", if unter.trim().is_empty() { haupt } else { unter.trim() })
}

/// **Ein Befehl mit seinem Satz als Zeile einer Hilfeliste**, je nach Wirt
/// ([`befehl_beim_wirt`]), der Befehl auf `breite` Zeichen aufgefuellt.
pub fn hilfezeile(zeile: &str, was: &str, fenster: bool, breite: usize) -> String {
    let befehl = if fenster {
        befehl_beim_wirt(zeile, true).trim_start_matches('„').trim_end_matches('“').to_string()
    } else {
        befehl_beim_wirt(zeile, false)
    };
    let fehlt = breite.saturating_sub(befehl.chars().count()).max(2);
    format!("{befehl}{}{was}", " ".repeat(fehlt))
}

/// **Die Schleife eines nativen Moduls**: liest Ereignisse, bis stdin endet
/// oder [`AnModul::Ende`] kommt. Den Handschlag beantwortet sie selbst.
pub fn nativ_fahren<M: Modul>(mut m: M) {
    let ein = std::io::stdin();
    let mut aus = std::io::stdout().lock();
    for zeile in ein.lock().lines() {
        let Ok(zeile) = zeile else { break };
        let e: AnModul = match nachricht::lesen(&zeile) {
            Ok(e) => e,
            Err(f) => {
                eprintln!("{f}");
                continue;
            }
        };
        let ende = matches!(e, AnModul::Ende);
        let mut a = Ausgang::default();
        if let AnModul::Hallo { .. } = &e {
            let (name, version) = m.name();
            a.senden(VomModul::Hallo { protokoll: crate::PROTOKOLL, name: name.to_string(), version: version.to_string() });
        }
        m.ereignis(e, &mut a);
        for n in &a.nachrichten {
            if writeln!(aus, "{}", nachricht::zeile(n)).is_err() {
                return;
            }
        }
        if aus.flush().is_err() || ende {
            return;
        }
    }
}

/// **Eine Runde eines abgeschotteten Moduls**: das Gegenstueck zu einer Zeile
/// in [`nativ_fahren`]. Gerufen aus [`wasm_modul!`](crate::wasm_modul).
pub fn wasm_runde<M: Modul>(m: &mut M, zeile: &str, schreibe: impl Fn(&str)) {
    let e: AnModul = match nachricht::lesen(zeile) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut a = Ausgang::default();
    if let AnModul::Hallo { .. } = &e {
        let (name, version) = m.name();
        a.senden(VomModul::Hallo { protokoll: crate::PROTOKOLL, name: name.to_string(), version: version.to_string() });
    }
    m.ereignis(e, &mut a);
    for n in &a.nachrichten {
        schreibe(&nachricht::zeile(n));
    }
}

/// **Was ein abgeschottetes Modul beim Wirt erreicht.** Unter jedem anderen
/// Ziel gibt es diese Funktionen nicht; ein Modul fuer beide Laufzeiten
/// trennt sie selbst.
#[cfg(target_arch = "wasm32")]
pub mod wirt_ruf {
    use crate::nachricht::{self, Httpanfrage, Httpantwort};

    #[link(wasm_import_module = "myl")]
    extern "C" {
        pub fn ausgabe(p: *const u8, n: usize);
        fn protokoll(p: *const u8, n: usize);
        fn http(p: *const u8, n: usize) -> i64;
        fn datei_lesen(p: *const u8, n: usize) -> i64;
        fn datei_schreiben(p: *const u8, n: usize, q: *const u8, m: usize) -> i64;
        fn geheimnis(p: *const u8, n: usize) -> i64;
        fn ergebnis(p: *mut u8, n: usize);
        fn jetzt_ms() -> i64;
    }

    /// Eine Antwort des Wirts: `n >= 0` sind Bytes, sonst `-1 - n` Bytes Fehlertext.
    fn abholen(n: i64) -> Result<Vec<u8>, String> {
        let laenge = if n >= 0 { n } else { -1 - n } as usize;
        let mut v = vec![0u8; laenge];
        // SAFETY: Der Puffer ist `laenge` Bytes gross und gehoert uns.
        unsafe { ergebnis(v.as_mut_ptr(), laenge) };
        if n >= 0 {
            Ok(v)
        } else {
            Err(String::from_utf8_lossy(&v).into_owned())
        }
    }

    /// Eine Zeile ins Protokoll der Konsole.
    pub fn protokollieren(text: &str) {
        // SAFETY: Zeiger und Laenge stammen aus einem gueltigen `&str`.
        unsafe { protokoll(text.as_ptr(), text.len()) }
    }

    /// Fragt einen genehmigten Rechner.
    pub fn http_anfrage(a: &Httpanfrage) -> Result<Httpantwort, String> {
        let z = nachricht::zeile(a);
        // SAFETY: wie oben.
        let n = unsafe { http(z.as_ptr(), z.len()) };
        let b = abholen(n)?;
        serde_json::from_slice(&b).map_err(|f| f.to_string())
    }

    /// Liest eine Datei aus dem eigenen Datenordner.
    pub fn datei_holen(name: &str) -> Result<Vec<u8>, String> {
        // SAFETY: wie oben.
        abholen(unsafe { datei_lesen(name.as_ptr(), name.len()) })
    }

    /// Schreibt eine Datei in den eigenen Datenordner.
    pub fn datei_ablegen(name: &str, daten: &[u8]) -> Result<(), String> {
        // SAFETY: wie oben.
        abholen(unsafe { datei_schreiben(name.as_ptr(), name.len(), daten.as_ptr(), daten.len()) }).map(|_| ())
    }

    /// Ein eigenes Geheimnis; andere Module erreichen es nicht.
    pub fn geheimnis_holen(name: &str) -> Result<String, String> {
        // SAFETY: wie oben.
        let b = abholen(unsafe { geheimnis(name.as_ptr(), name.len()) })?;
        String::from_utf8(b).map_err(|f| f.to_string())
    }

    /// Die Uhr in Millisekunden.
    pub fn jetzt() -> i64 {
        // SAFETY: ohne Argumente.
        unsafe { jetzt_ms() }
    }
}

/// **Macht aus einem [`Modul`] ein abgeschottetes Modul**: die beiden Exporte,
/// die der Wirt ruft. Das Modul entsteht beim ersten Ereignis.
///
/// ```ignore
/// myl_module::wasm_modul!(MeinModul, MeinModul::default());
/// ```
#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! wasm_modul {
    ($typ:ty, $neu:expr) => {
        std::thread_local! {
            static MODUL: std::cell::RefCell<Option<$typ>> = const { std::cell::RefCell::new(None) };
        }

        /// Reserviert Platz fuer eine Zeile; der Wirt schreibt dort hinein.
        #[no_mangle]
        pub extern "C" fn myl_speicher(n: usize) -> *mut u8 {
            let mut v = std::mem::ManuallyDrop::new(Vec::<u8>::with_capacity(n));
            v.as_mut_ptr()
        }

        /// Eine Zeile des Wirts; der Platz stammt aus `myl_speicher`.
        ///
        /// # Safety
        /// Der Wirt ruft das genau einmal je `myl_speicher`, mit derselben Laenge.
        #[no_mangle]
        pub unsafe extern "C" fn myl_ereignis(p: *mut u8, n: usize) {
            // SAFETY: Siehe oben; der Wirt hat `n` Bytes geschrieben.
            let bytes = unsafe { Vec::from_raw_parts(p, n, n) };
            let Ok(zeile) = String::from_utf8(bytes) else { return };
            MODUL.with(|m| {
                let mut m = m.borrow_mut();
                let m = m.get_or_insert_with(|| $neu);
                $crate::gast::wasm_runde(m, &zeile, |z| {
                    // SAFETY: Zeiger und Laenge stammen aus einem gueltigen `&str`.
                    unsafe { $crate::gast::wirt_ruf::ausgabe(z.as_ptr(), z.len()) }
                });
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ In der Konsole unter `/module`, im Fenster ohne Schraegstrich.
    #[test]
    fn befehle_heissen_beim_wirt_verschieden() {
        assert_eq!(befehl_beim_wirt("/trading off", false), "/module trading off");
        assert_eq!(befehl_beim_wirt("/trading", false), "/module trading");
        assert_eq!(befehl_beim_wirt("/trading close all", true), "„close all“");
        assert_eq!(befehl_beim_wirt("/cad", true), "„cad“");
        assert_eq!(hilfezeile("/cad off", "die Vorschau aus", true, 8), "off     die Vorschau aus");
        assert_eq!(hilfezeile("/cad off", "die Vorschau aus", false, 18), "/module cad off   die Vorschau aus");
        assert!(!hilfezeile("/cad zeigen", "x", true, 0).contains('/'));
    }
}
