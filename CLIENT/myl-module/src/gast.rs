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
