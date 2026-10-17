//! **Der Vorlader**: kuendigt die Experten der naechsten Gemischebene an,
//! waehrend die aktuelle noch rechnet.
//!
//! # ⚑ Wozu
//!
//! Passt ein Gemisch nicht in den Arbeitsspeicher, kommen je Token Experten
//! von der Platte, und bis hierher geschah das **nacheinander**: Der Router
//! einer Ebene waehlt, die gewaehlten Experten werden angekuendigt, die
//! Rechnung wartet, bis sie da sind. Die Platte steht still, solange
//! gerechnet wird, und die Rechnung, solange gelesen wird.
//!
//! Der Router der naechsten Ebene, angewandt auf den Residualstrom dieser
//! Ebene nach der Aufmerksamkeit, trifft deren Wahl gut
//! ([`crate::model::IntegerModel::naechste_wahl`]). Gemessen am int8-35B
//! an 1977 Token: Von den 8 Experten, die die naechste Ebene waehlt, lagen
//! 78,8 % unter den 8 vorhergesagten, 90,2 % unter 12 und 94,2 % unter 16.
//! Diese werden hier angekuendigt, und die Platte liest sie, waehrend die
//! Gemischrechnung dieser Ebene und die Aufmerksamkeit der naechsten laufen.
//!
//! ⚑ **An keiner Zahl aendert der Vorlader etwas.** Er raet dem System nur,
//! welche Seiten bald gebraucht werden; gerechnet wird mit der echten Wahl.
//! Ein Fehlgriff kostet Plattenzeit und einen Platz im Dateicache, nie ein
//! anderes Ergebnis.
//!
//! # ⚠️ Standardmaessig aus, weil gemessen
//!
//! Am int8-35B unter Speicherdruck (Ballon, `BENCHMARKS/Inferenz/results/
//! decode_speicherdruck_myelith-35b-a3b.md`) brachte der Vorlader keinen
//! verlaesslichen Gewinn: mit 8 Kandidaten +5 % bei 18 GiB frei und −8 % bei
//! 9,6 GiB, mit 12 bis zu −37 %. Wo Speicher reicht, kostet die Vorhersage
//! mehr, als sie spart; wo er knapp ist, draengen Fehlgriffe nuetzliche
//! Experten aus dem Cache. Er bleibt fuer Messungen (`bench_probe
//! --vorladen <k>`), bis eine Vorhersage mit weniger Fehlgriffen ihn traegt.
//!
//! # ⚑ Wie
//!
//! Eigene Faeden, die Auftraege aus einer Warteschlange nehmen und
//! [`Gewichtsdaten::vorbereiten`] rufen. **Eigene, nicht der Fadenpool der
//! Rechnung**: Der Rat blockiert, solange das System die Seiten holt, und
//! ein blockierter Rechenfaden waere genau das Warten, das hier wegfallen
//! soll. Ein Auftrag traegt einen `Arc` auf die Gewichte, also keinen rohen
//! Zeiger: Das Abbild lebt, bis der Auftrag erledigt ist.
//!
//! Ein Auftrag gilt fuer einen Schritt (eine Ebene eines Tokens). Ist die
//! Rechnung schon weiter, wird er verworfen, statt die Platte mit Seiten zu
//! belegen, die niemand mehr braucht.

use crate::model::Gewichtsdaten;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

/// **Wie viele Faeden ankuendigen**: wie beim Ankuendigen der gewaehlten
/// Experten (`ANKUENDIGUNGSFAEDEN` im Modell), wo die Kurve bei sechzehn
/// flach wurde.
const FAEDEN: usize = 16;

struct Auftrag {
    daten: Arc<Gewichtsdaten>,
    /// Der Schritt, fuer den die Gewichte gebraucht werden.
    fuer: u64,
}

/// Der Schritt, an dem die Rechnung gerade ist.
static SCHRITT: AtomicU64 = AtomicU64::new(0);
static WARTESCHLANGE: OnceLock<Mutex<Sender<Auftrag>>> = OnceLock::new();
/// Wie viele Auftraege verworfen wurden, weil die Rechnung schon weiter war.
static VERWORFEN: AtomicU64 = AtomicU64::new(0);

fn warteschlange() -> &'static Mutex<Sender<Auftrag>> {
    WARTESCHLANGE.get_or_init(|| {
        let (tx, rx) = channel::<Auftrag>();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..FAEDEN {
            let rx = Arc::clone(&rx);
            // Ein Faden, der nicht startet, kostet nur das Vorladen.
            let _ = std::thread::Builder::new().name(format!("vorlader-{i}")).spawn(move || arbeiten(&rx));
        }
        Mutex::new(tx)
    })
}

fn arbeiten(rx: &Mutex<Receiver<Auftrag>>) {
    loop {
        // Ein vergifteter Riegel heisst: Ein anderer Vorlader brach ab. Die
        // Warteschlange ist trotzdem eine.
        let auftrag = match rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
            Ok(a) => a,
            Err(_) => return,
        };
        if SCHRITT.load(Ordering::Relaxed) > auftrag.fuer {
            VERWORFEN.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        auftrag.daten.vorbereiten();
    }
}

/// **Ein neuer Schritt beginnt**; gibt seine Nummer zurueck.
pub fn schritt() -> u64 {
    SCHRITT.fetch_add(1, Ordering::Relaxed) + 1
}

/// **Kuendigt Gewichte an, die im Schritt `fuer` gebraucht werden.** Kehrt
/// sofort zurueck.
pub fn ankuendigen(daten: impl IntoIterator<Item = Arc<Gewichtsdaten>>, fuer: u64) {
    let tx = warteschlange().lock().unwrap_or_else(|e| e.into_inner());
    for d in daten {
        // Ohne Faeden kein Empfaenger: dann eben kein Vorladen.
        let _ = tx.send(Auftrag { daten: d, fuer });
    }
}

/// **Wie viele Lesefaeden fehlende Experten einer Ebene holen**, waehrend die
/// vorhandenen rechnen: acht Experten zu je drei Matrizen in einer Welle.
const LESEFAEDEN: usize = 24;

struct Jetzt {
    daten: Arc<Gewichtsdaten>,
    rest: Arc<(Mutex<usize>, std::sync::Condvar)>,
}

static JETZT: OnceLock<Mutex<Sender<Jetzt>>> = OnceLock::new();

/// **Gewichte, die gerade geholt werden**; [`Ankuendigung::abwarten`]
/// kehrt zurueck, wenn alle da sind.
#[must_use = "wer nicht wartet, rechnet mit Seiten, die noch unterwegs sind"]
pub struct Ankuendigung {
    rest: Arc<(Mutex<usize>, std::sync::Condvar)>,
}

impl Ankuendigung {
    /// Wartet, bis jede angekuendigte Matrix im Speicher ist.
    pub fn abwarten(self) {
        let (m, c) = &*self.rest;
        let mut n = m.lock().unwrap_or_else(|e| e.into_inner());
        while *n > 0 {
            n = c.wait(n).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// **Holt Gewichte auf eigenen Lesefaeden und kehrt sofort zurueck**; die
/// Rechenfaeden bleiben frei fuer das, was schon da ist.
pub fn im_hintergrund(daten: Vec<Arc<Gewichtsdaten>>) -> Ankuendigung {
    let rest = Arc::new((Mutex::new(daten.len()), std::sync::Condvar::new()));
    let tx = JETZT.get_or_init(|| {
        let (tx, rx) = channel::<Jetzt>();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..LESEFAEDEN {
            let rx = Arc::clone(&rx);
            // Ein Faden, der nicht startet, kostet nur Gleichzeitigkeit.
            let _ = std::thread::Builder::new().name(format!("lesefaden-{i}")).spawn(move || loop {
                let j = match rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
                    Ok(j) => j,
                    Err(_) => return,
                };
                j.daten.vorbereiten();
                fertig(&j.rest);
            });
        }
        Mutex::new(tx)
    });
    let tx = tx.lock().unwrap_or_else(|e| e.into_inner());
    for d in daten {
        if let Err(e) = tx.send(Jetzt { daten: d, rest: Arc::clone(&rest) }) {
            // Ohne Lesefaden: selbst holen.
            e.0.daten.vorbereiten();
            fertig(&rest);
        }
    }
    Ankuendigung { rest }
}

fn fertig(rest: &(Mutex<usize>, std::sync::Condvar)) {
    let (m, c) = rest;
    let mut n = m.lock().unwrap_or_else(|e| e.into_inner());
    *n -= 1;
    if *n == 0 {
        c.notify_all();
    }
}

/// Wie viele Auftraege bisher verworfen wurden (fuer Messungen).
pub fn verworfen() -> u64 {
    VERWORFEN.load(Ordering::Relaxed)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Ein Auftrag fuer einen vergangenen Schritt wird verworfen**, einer
    /// fuer den kommenden ausgefuehrt (bei Gewichten im Speicher ist das
    /// Ankuendigen ein Nichts, aber es laeuft durch).
    #[test]
    fn ein_veralteter_auftrag_wird_verworfen() {
        let d = Arc::new(Gewichtsdaten::from(vec![1i8; 16]));
        let jetzt = schritt();
        let vorher = verworfen();
        ankuendigen([Arc::clone(&d)], jetzt.saturating_sub(1));
        // Warten, bis ein Faden den Auftrag genommen hat.
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while verworfen() == vorher && std::time::Instant::now() < ende {
            std::thread::yield_now();
        }
        assert!(verworfen() > vorher, "ein veralteter Auftrag lief");
        ankuendigen([d], jetzt + 1);
    }
}
