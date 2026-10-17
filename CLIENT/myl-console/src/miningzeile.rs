//! **Dass Mining laeuft**: eine Zeile ueber dem Kasten und alle fuenf
//! Minuten eine Meldung.
//!
//! # ⚑ Wo die Zeile steht (Auftrag des Projektinhabers, 2026-10-17)
//!
//! **An der Stelle der Ladezeile, wenn nichts anderes laeuft**, und eine
//! Zeile darunter, solange ein Auftrag die Ladezeile braucht. Zwei Zeilen
//! fuer zwei Dinge, die gleichzeitig geschehen; eine geteilte Zeile zeigte
//! immer nur eines davon.
//!
//! # ⚑ Wer sie zeichnet
//!
//! **Im Leerlauf die Eingabezeile, im Lauf die Anzeige**, also jeweils der
//! einzige Faden, der gerade auf den Schirm schreibt (`anzeige.rs`, Kopf).
//! ⚠️ **Im Leerlauf ohne `ESC 7` und `ESC 8`**: Dieser Speicher haelt dort
//! die Stelle im Rollbereich, an der die naechste Antwort beginnt
//! (`Schirm::merken`); wer ihn ueberschreibt, schickt die Antwort in den
//! Kasten. Danach zeichnet die Eingabezeile sich selbst neu und setzt den
//! Wagen.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use myl_client::mining::{self, Zustand};

use crate::design;
use crate::schirm::Schirm;

/// ⚑ **Vom Projektinhaber gewuenscht, und sie meinen nichts**, wie die
/// Sprueche der Ladezeile. Was wirklich geschieht, steht in der Meldung.
pub const SPRUECHE: [&str; 10] = [
    "harvesting ripe crops",
    "collecting pennies",
    "panning for gold dust",
    "polishing nuggets",
    "stacking coins",
    "feeding the hamsters",
    "watering the seedlings",
    "herding stray shards",
    "tending the beehives",
    "sweeping the vault",
];

/// Wie lange ein Spruch steht.
const SPRUCHDAUER: Duration = Duration::from_secs(20);
/// Wie oft nachgesehen wird, ob der Knoten noch antwortet.
const PRUEFTAKT: Duration = Duration::from_secs(15);
/// Die Drehscheibe der Zeile, eine eigene: Sie soll sich von der Ladezeile
/// unterscheiden, wenn beide zugleich stehen.
const RAD: [char; 8] = ['⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈'];
/// Ein Schritt der Drehscheibe.
const RADTAKT: Duration = Duration::from_millis(200);

struct Lauf {
    zustand: Zustand,
    /// Seit wann diese Konsole ihn kennt; danach dreht sich die Scheibe.
    gesehen: Instant,
    naechste_meldung: Instant,
    geprueft: Instant,
}

static LAUF: Mutex<Option<Lauf>> = Mutex::new(None);

/// Uebernimmt den Stand: `Some` nach einem Start, `None` nach dem Ende.
pub fn uebernehmen(z: Option<Zustand>) {
    let jetzt = Instant::now();
    if let Ok(mut g) = LAUF.lock() {
        *g = z.map(|zustand| Lauf { zustand, gesehen: jetzt, naechste_meldung: jetzt + mining::MELDETAKT, geprueft: jetzt });
    }
}

/// Sieht nach, ob ein Knoten laeuft, auch einer aus dem Fenster.
pub fn auffrischen() {
    let z = mining::zustand();
    let bekannt = LAUF.lock().ok().and_then(|g| g.as_ref().map(|l| l.zustand.pid));
    if z.as_ref().map(|z| z.pid) != bekannt {
        uebernehmen(z);
    }
}

/// Ob Mining laeuft, nach dem letzten Blick.
pub fn laeuft() -> bool {
    LAUF.lock().is_ok_and(|g| g.is_some())
}

/// Wann die naechste Meldung faellig ist.
pub fn naechste_meldung() -> Option<Instant> {
    LAUF.lock().ok().and_then(|g| g.as_ref().map(|l| l.naechste_meldung))
}

/// **Die Meldung, wenn sie faellig ist**, sonst `None`.
///
/// ⚑ **Antwortet der Knoten nicht mehr, sagt die Meldung das** und die
/// Zeile verschwindet; eine Zeile, die weiter Muenzen sammelt, waehrend
/// nichts laeuft, waere eine Erfindung.
pub fn faellig() -> Option<Vec<String>> {
    let mut g = LAUF.lock().ok()?;
    let l = g.as_mut()?;
    if Instant::now() < l.naechste_meldung {
        return None;
    }
    l.naechste_meldung = Instant::now() + mining::MELDETAKT;
    match mining::stand(&mut l.zustand) {
        Ok(s) => Some(mining::zusammenfassung(&l.zustand, &s, jetzt_ms())),
        Err(_) => {
            *g = None;
            Some(vec!["⛏ Mining ist beendet: Der Knoten antwortet nicht mehr.".into()])
        }
    }
}

/// Sieht hoechstens alle fuenfzehn Sekunden nach, ob der Knoten noch lebt.
fn pruefen() {
    let faellig = LAUF.lock().ok().and_then(|g| g.as_ref().map(|l| l.geprueft.elapsed() >= PRUEFTAKT)).unwrap_or(false);
    if !faellig {
        return;
    }
    let lebt = mining::zustand().is_some();
    if let Ok(mut g) = LAUF.lock() {
        if lebt {
            if let Some(l) = g.as_mut() {
                l.geprueft = Instant::now();
            }
        } else {
            *g = None;
        }
    }
}

/// **Die Zeile selbst**, rein: dieselbe Lage ergibt dieselbe Zeile.
pub fn kopf(seit: Duration) -> String {
    let rad = RAD[(seit.as_millis() / RADTAKT.as_millis()) as usize % RAD.len()];
    let spruch = SPRUECHE[(seit.as_secs() / SPRUCHDAUER.as_secs()) as usize % SPRUECHE.len()];
    format!("{rad} ⛏ {spruch}…")
}

/// Was hinter dem Spruch steht: seit wann, und dass es ein Probelauf ist.
pub fn schweif(zustand: &Zustand) -> String {
    let minuten = jetzt_ms().saturating_sub(zustand.gestartet_ms) / 60_000;
    format!("  (Mining seit {minuten} min, Probelauf · /mining)")
}

/// **Zeichnet die Zeile**, `versatz` Zeilen unter der Ladezeile, ohne den
/// Wagen zurueckzuholen. Gibt `true`, wenn etwas gezeichnet wurde; dann muss
/// der Aufrufer den Wagen setzen.
pub fn zeichnen_ohne_wagen(sch: &Schirm, versatz: u16, bild: myl_client::einstellungen::Konsolendesign) -> bool {
    pruefen();
    let Some((seit, schweif)) = LAUF.lock().ok().and_then(|g| g.as_ref().map(|l| (l.gesehen.elapsed(), schweif(&l.zustand)))) else {
        return false;
    };
    let kopf = kopf(seit);
    let r = crate::schirm::Rahmen::messen();
    let breite = r.einzug.chars().count() + r.innen + 2;
    let luft = breite.saturating_sub(kopf.chars().count() + schweif.chars().count()) / 2;
    let mut aus = std::io::stdout();
    let _ = write!(aus, "{}{}", sch.zeile(Schirm::LADEZEILE + versatz), " ".repeat(luft));
    for (i, c) in kopf.chars().enumerate() {
        let welle = seit + crate::anzeige::WELLE * i as u32;
        let _ = write!(aus, "{}{c}", crossterm::style::SetForegroundColor(design::ladefarbe(bild, welle)));
    }
    let _ = write!(
        aus,
        "{}{schweif}{}",
        crossterm::style::SetForegroundColor(design::toene(bild).beiwerk),
        crossterm::style::ResetColor
    );
    let _ = aus.flush();
    true
}

/// **Zeichnet die Zeile im Lauf**, mit `ESC 7` und `ESC 8`: Dort steht der
/// Wagen im Rollbereich, und genau dorthin soll er zurueck.
pub fn zeichnen_im_lauf(sch: &Schirm, bild: myl_client::einstellungen::Konsolendesign) {
    let mut aus = std::io::stdout();
    let _ = write!(aus, "\x1b7");
    zeichnen_ohne_wagen(sch, 1, bild);
    let _ = write!(aus, "\x1b8");
    let _ = aus.flush();
}

fn jetzt_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Spruch und Scheibe wandern**, und die Zeile sagt immer, dass es
    /// Mining ist.
    #[test]
    fn die_zeile_wechselt_spruch_und_scheibe() {
        let a = kopf(Duration::ZERO);
        assert!(a.contains("⛏ harvesting ripe crops…"), "{a}");
        assert_ne!(a, kopf(RADTAKT), "die Scheibe steht");
        assert!(kopf(SPRUCHDAUER).contains(SPRUECHE[1]));
        assert!(kopf(SPRUCHDAUER * SPRUECHE.len() as u32).contains(SPRUECHE[0]), "die Sprueche laufen nicht im Kreis");
    }

    /// ⛔️ **Im Leerlauf kein `ESC 7`**: Er ueberschriebe die gemerkte Stelle
    /// im Rollbereich, und die naechste Antwort stuende im Kasten.
    #[test]
    fn im_leerlauf_bleibt_der_gemerkte_wagen_unberuehrt() {
        let quelle = include_str!("miningzeile.rs");
        let rumpf = quelle.split("pub fn zeichnen_ohne_wagen(").nth(1).unwrap().split("\npub fn ").next().unwrap();
        assert!(!rumpf.contains("\\x1b7") && !rumpf.contains("\\x1b8"), "zeichnen_ohne_wagen merkt den Wagen");
    }
}
