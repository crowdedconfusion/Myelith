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
