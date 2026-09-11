//! Der untere Rand des Fensters gehoert der Eingabe.
//!
//! # ⚑ Ein Rollbereich, und das ist der ganze Trick
//!
//! Ein Terminal hat keine „feste Zeile". Was es hat, ist ein
//! **Rollbereich** (`ESC[oben;untenr`): Alles, was gedruckt wird, rollt
//! innerhalb dieser Grenzen, und die Zeilen darunter bleiben stehen,
//! bis jemand sie ausdruecklich beschreibt. Vier Zeilen am unteren Rand
//! sind damit unsere, und der ganze uebrige Text laeuft darueber weg.
//!
//! ⚠️ **Was reserviert ist, muss auch zurueckgegeben werden.** Ein
//! Programm, das mit gesetztem Rollbereich endet, hinterlaesst eine
//! Shell, die nur noch im oberen Teil des Fensters schreibt. Deshalb
//! [`Schirm::aufloesen`], und deshalb wird es auch bei einem Abbruch
//! gerufen.
//!
//! ⛑ **Und der Wagen muss zurueck.** Nach dem Zeichnen steht er in der
//! Eingabezeile, also **ausserhalb** des Rollbereichs; wer von dort aus
//! druckt, schreibt in den Rahmen. `ESC 7` und `ESC 8` merken sich die
//! Stelle im Rollbereich und holen sie zurueck.

use std::io::{IsTerminal, Write};

use crate::banner;

/// Wie viele Zeilen unten der Eingabe gehoeren: Ladezeile, eine leere,
/// obere Kante, Eingabezeile, untere Kante, Fusszeile, Arbeitsordner.
///
/// ⚑ **Sieben seit dem 2026-09-11** (Festlegungen des
/// Projektinhabers). Die Ladezeile steht **ueber** dem Kasten und nicht
/// mehr mitten in der Zeitleiste: Dort wanderte sie mit jeder Ausgabe
/// weiter und stand mal hier, mal da. ⚑ **Die leere Zeile darunter ist
/// Absicht**, sonst klebt die Anzeige am Rahmen.
///
/// ⚑ **Fuenf davon seit dem 2026-09-11** (Festlegung des Projektinhabers).
/// Der Arbeitsordner stand vorher im Kopf; der Kopf ist entfallen, und
/// **verschwinden durfte er nicht**: Ein Agent mit Dateiwerkzeugen
/// arbeitet genau dort. Unter der Fusszeile steht er dauerhaft.
pub const RESERVE: u16 = 7;

/// Unter dieser Fensterhoehe gibt es keinen festen Rand.
///
/// ⚠️ **Sieben von zehn Zeilen waeren kein Rand mehr, sondern das
/// Fenster.** Dann wird gar keiner reserviert und alles laeuft mit.
pub const MINDESTHOEHE: u16 = 14;

/// ⚑ **Zur Bauzeit geprueft und nicht zur Laufzeit.** Waere die
/// Mindesthoehe nicht deutlich groesser als der Rand, gaebe es Fenster,
/// in denen der Rand die halbe Anzeige belegt; **ein Fehler, der beim
/// Uebersetzen auffaellt, kostet niemanden einen Testlauf.**
const _: () = assert!(MINDESTHOEHE > RESERVE + 4);

/// Der untere Rand, solange er eingerichtet ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schirm {
    /// Fensterhoehe in Zeilen, wie bei der Einrichtung gemessen.
    pub hoehe: u16,
}

impl Schirm {
    /// Misst das Fenster, oder `None`, wenn es keines gibt.
    pub fn messen() -> Option<Self> {
        if !std::io::stdout().is_terminal() {
            return None;
        }
        let (_, hoehe) = crossterm::terminal::size().ok()?;
        (hoehe >= MINDESTHOEHE).then_some(Self { hoehe })
    }

    /// Die letzte Zeile, die noch rollt.
    pub fn rollende(&self) -> u16 {
        self.hoehe.saturating_sub(RESERVE)
    }

    /// Die Zeile, in der die obere Kante des Rahmens steht.
    pub fn erste_eigene(&self) -> u16 {
        self.rollende() + 1
    }

    /// **Schafft Platz und schliesst den Rollbereich.**
    ///
    /// ⚑ **Erst die Leerzeilen, dann die Grenze.** Ohne die Leerzeilen
    /// stuende der Rahmen ueber dem, was schon dasteht, und der Kopf
    /// des Programms waere weg.
    pub fn einrichten(&self) {
        let mut aus = std::io::stdout();
        let _ = write!(aus, "{}", "\n".repeat(RESERVE as usize));
        let _ = write!(aus, "\x1b[1;{}r", self.rollende());
        let _ = write!(aus, "\x1b[{};1H", self.rollende());
        let _ = aus.flush();
    }

    /// Setzt nur die Grenze neu, ohne Platz zu schaffen.
    ///
    /// ⚑ **Fuer den Fall, dass jemand das Fenster zieht.** Neue
    /// Leerzeilen waeren dabei falsch: Der Platz ist schon reserviert,
    /// er liegt nur woanders.
    pub fn grenze_setzen(&self) {
        let mut aus = std::io::stdout();
        let _ = write!(aus, "\x1b[1;{}r", self.rollende());
        let _ = aus.flush();
    }

    /// Gibt den Rollbereich zurueck und setzt den Wagen ans Ende.
    pub fn aufloesen(&self) {
        let mut aus = std::io::stdout();
        let _ = writeln!(aus, "\x1b[r\x1b[{};1H", self.hoehe);
        let _ = aus.flush();
    }

    /// Merkt sich die Stelle im Rollbereich.
    pub fn merken(&self) {
        let mut aus = std::io::stdout();
        let _ = write!(aus, "\x1b7");
        let _ = aus.flush();
    }

    /// Holt den Wagen dorthin zurueck.
    pub fn zurueck(&self) {
        let mut aus = std::io::stdout();
        let _ = write!(aus, "\x1b8");
        let _ = aus.flush();
    }

    /// Die Zeile der Ladeanzeige, ganz oben im eigenen Rand.
    pub const LADEZEILE: u16 = 0;
    /// Die obere Kante des Kastens.
    pub const KASTEN: u16 = 2;

    /// Setzt den Wagen an den Anfang einer der eigenen Zeilen und
    /// raeumt sie.
    pub fn zeile(&self, versatz: u16) -> String {
        format!("\x1b[{};1H\x1b[2K", self.erste_eigene() + versatz)
    }
}

/// Wie breit der Textblock unter der Marke hoechstens wird, und damit
/// auch der Eingaberahmen.
///
/// ⚑ **Eine Zahl fuer beide.** Zwei Bloecke, die heute zufaellig gleich
/// breit sind, sind morgen zufaellig verschieden breit; dann steht der
/// Rahmen neben dem Kopf statt darunter. Dieselbe Klasse wie Fund 271:
/// **was an zwei Stellen von Hand steht, ist keine Angabe, sondern zwei
/// Behauptungen.**
pub const BLOCKBREITE: usize = 78;

/// **Der Rahmen um die Eingabe: seine Masse und seine Kanten.**
///
/// ⚑ **Die drei Kanten kommen aus einer Messung**, nicht drei Mal aus
/// derselben Rechnung. Eine untere Kante, die zwei Zeichen schmaler ist
/// als die obere, faellt niemandem auf, der sie schreibt, und jedem,
/// der sie sieht.
pub struct Rahmen {
    /// Der Einzug, der den Rahmen unter den Kopf rueckt.
    pub einzug: String,
    /// Die Breite zwischen den beiden senkrechten Strichen.
    pub innen: usize,
}

impl Rahmen {
    /// ⚠️ **Nach oben begrenzt, und das ist keine Schoenheit.** Was
    /// jemand tippt, laeuft ueber die rechte Kante hinaus, sobald die
    /// Zeile laenger wird als der Rahmen: Das Terminal bricht erst an
    /// *seiner* Kante um. Ein Rahmen ueber die volle Fensterbreite
    /// haette diesen Fall nicht, dafuer auf einem breiten Schirm eine
    /// Eingabezeile von zweihundert Spalten, und das ist der haeufigere
    /// Anblick.
    pub fn messen() -> Self {
        let breite = (banner::fensterbreite() as usize).clamp(28, BLOCKBREITE);
        Self { einzug: banner::blockeinzug(breite), innen: breite.saturating_sub(2) }
    }

    pub fn oben(&self) -> String {
        format!("{}╭{}╮", self.einzug, "─".repeat(self.innen))
    }

    pub fn leer(&self) -> String {
        format!("{}│{}│", self.einzug, " ".repeat(self.innen))
    }

    pub fn unten(&self) -> String {
        format!("{}╰{}╯", self.einzug, "─".repeat(self.innen))
    }

    /// In welcher Spalte der Wagen steht: hinter dem senkrechten
    /// Strich, ab null gezaehlt wie bei `MoveTo`.
    pub fn wagenspalte(&self) -> u16 {
        (self.einzug.chars().count() + 2) as u16
    }

    /// Wie viel Text zwischen die beiden Leerzeichen passt.
    pub fn textbreite(&self) -> usize {
        self.innen.saturating_sub(2)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// **Vier eigene Zeilen, und der Rollbereich hoert davor auf.**
    ///
    /// ⛑ Die Gegenprobe zu einem Fehler, den man nicht sieht, sondern
    /// erlebt: Reicht der Rollbereich eine Zeile zu weit, rollt die
    /// obere Kante des Rahmens mit und der Rahmen franst nach oben aus.
    #[test]
    fn der_rollbereich_endet_vor_dem_rahmen() {
        let s = Schirm { hoehe: 40 };
        assert_eq!(s.rollende(), 33);
        assert_eq!(s.erste_eigene(), 34);
        // Sieben Zeilen, und die letzte ist die letzte des Fensters.
        assert_eq!(s.erste_eigene() + RESERVE - 1, s.hoehe);
    }

    /// **Jede eigene Zeile wird vor dem Beschreiben geraeumt.**
    ///
    /// ⚠️ Ohne das steht ein laengerer Modellname noch da, wenn ein
    /// kuerzerer darueber geschrieben wird, und die Fusszeile liest sich
    /// wie zwei Zeilen uebereinander.
    #[test]
    fn jede_eigene_zeile_wird_geraeumt() {
        let s = Schirm { hoehe: 40 };
        for versatz in 0..RESERVE {
            let z = s.zeile(versatz);
            assert!(z.ends_with("\x1b[2K"), "{z:?} raeumt nicht");
            // ⚑ Aus der Messung abgeleitet und nicht abgeschrieben:
            // Eine Zahl im Test, die dieselbe Rechnung noch einmal
            // macht, prueft die Rechnung nicht.
            assert!(z.contains(&format!("[{};1H", s.erste_eigene() + versatz)), "{z:?}");
        }
    }

    /// **Unter der Mindesthoehe gibt es gar keinen Rand.**
    #[test]
    fn ein_zu_kleines_fenster_bekommt_keinen_rand() {
        // Die Messung selbst braucht ein Terminal; geprueft wird die
        // Bedingung, an der sie scheitert.
        assert!(Schirm::messen().is_none_or(|s| s.hoehe >= MINDESTHOEHE));
        // Dass Platz fuer mehr als den Rand bleibt, steht als
        // Bauzeitbedingung weiter oben und braucht hier keine Zeile.
    }
}
