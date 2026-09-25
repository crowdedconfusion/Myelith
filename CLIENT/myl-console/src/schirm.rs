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
//! 📌 **Und der Wagen muss zurueck.** Nach dem Zeichnen steht er in der
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
    ///
    /// ⚑ **Danach steht der Wagen dort, wo der Text aufhoerte**, und nicht
    /// mehr auf der letzten Zeile des Rollbereichs (Festlegung des
    /// Projektinhabers, 2026-09-24). Das Gespraech fuellt erst die freie
    /// Flaeche unter dem Logo und rollt erst dann. 📌 **Der Grund ist das
    /// Logo:** Stand der Wagen vom ersten Moment an unten, schob jede
    /// Zeile das Logo weiter, und niemand konnte mehr sagen, wo es steht;
    /// ohne diese Auskunft darf es nicht fliessen ([`crate::schimmer`]).
    ///
    /// ⚠️ `ESC[…r` setzt den Wagen selbst an den Anfang, deshalb wird er
    /// danach ausdruecklich gestellt. Laesst sich die Stelle nicht
    /// messen, oder haben die Leerzeilen den Schirm gerollt, geht es wie
    /// bisher unten weiter, und das Logo wird nicht mehr gemalt.
    pub fn einrichten(&self) {
        let mut aus = std::io::stdout();
        let _ = aus.flush();
        let vorher = crossterm::cursor::position().ok().map(|(_, zeile)| zeile);
        let _ = write!(aus, "{}", "\n".repeat(RESERVE as usize));
        let _ = write!(aus, "\x1b[1;{}r", self.rollende());
        let ziel = match vorher {
            Some(zeile) => fortsetzungszeile(zeile, self.rollende()),
            None => None,
        };
        let ziel = ziel.unwrap_or_else(|| {
            crate::schimmer::vergessen();
            self.rollende()
        });
        let _ = write!(aus, "\x1b[{ziel};1H");
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
    ///
    /// ⚠️ **Die Zahl darin zaehlt ab eins**, wie jede
    /// ANSI-Positionierung. Wer denselben Versatz an
    /// `crossterm::cursor::MoveTo` gibt, landet eine Zeile tiefer;
    /// dafuer gibt es [`Self::wagenzeile`].
    pub fn zeile(&self, versatz: u16) -> String {
        format!("\x1b[{};1H\x1b[2K", self.erste_eigene() + versatz)
    }

    /// Dieselbe Zeile wie [`Self::zeile`], aber **ab null gezaehlt**,
    /// also so, wie `crossterm::cursor::MoveTo` sie erwartet.
    ///
    /// 📌 **Fund 347: hier stand nichts, und deshalb rechnete es jemand
    /// von Hand nach.** Der blinkende Wagen sass eine Zeile unter der
    /// Eingabe, weil derselbe Versatz einmal in eine ANSI-Sequenz
    /// (ab eins) und einmal in `MoveTo` (ab null) ging. **Zwei
    /// Zaehlweisen fuer dieselbe Zeile gehoeren an eine Stelle, sonst
    /// trifft die Umrechnung irgendwann jemand falsch.**
    pub fn wagenzeile(&self, versatz: u16) -> u16 {
        (self.erste_eigene() + versatz).saturating_sub(1)
    }
}

/// **Wo das Gespraech nach dem Einrichten weitergeht**, ab eins gezaehlt
/// wie `ESC[…H`, oder `None`, wenn es dafuer keinen sicheren Ort gibt.
///
/// `zeile` ist die Zeile des Wagens **vor** den Leerzeilen, ab null.
///
/// ⚑ **Eine Bedingung deckt zwei Faelle**, und das ist kein Versehen:
/// Liegt die Stelle unter dem Rollbereich, gehoert sie dem Rahmen; und
/// genau dann rollen die `RESERVE` Leerzeilen den Schirm, denn der
/// Rollbereich endet `RESERVE` Zeilen vor dem Fensterende. 📌 Die erste
/// Fassung prueft beides getrennt, und die Gegenprobe hat gezeigt, dass
/// jede der beiden Zeilen ohne die andere dasselbe tut.
pub fn fortsetzungszeile(zeile: u16, rollende: u16) -> Option<u16> {
    let ab_eins = zeile.saturating_add(1);
    (ab_eins <= rollende).then_some(ab_eins)
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
    /// 📌 Die Gegenprobe zu einem Fehler, den man nicht sieht, sondern
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

    /// **Die Zeilennummer aus `zeile` und die aus `MoveTo` bezeichnen
    /// dieselbe Zeile, aber nicht mit derselben Zahl.**
    ///
    /// 📌 **Fund 347, gemeldet vom Projektinhaber.** Der blinkende Wagen
    /// sass eine Zeile unter der Eingabe. Ursache: `zeile` schreibt
    /// `ESC[{n};1H`, und die ANSI-Positionierung zaehlt **ab eins**;
    /// `crossterm::cursor::MoveTo` zaehlt **ab null**. Wer dieselbe
    /// Zahl in beide gibt, trifft zwei verschiedene Zeilen.
    ///
    /// Diese Probe haelt die Umrechnung fest, damit sie nicht wieder
    /// jemand von Hand nachrechnet: **Wer in `zeile(v)` schreibt,
    /// bewegt den Wagen mit `MoveTo(_, wagenzeile(v))` dorthin.**
    ///
    /// ⚠️ **Ein Test, der Steuersequenzen liest, haette den Fehler nicht
    /// gefunden.** Er sieht die Zeile, in der etwas **steht**, und nicht
    /// die, in der etwas **blinkt**. Deshalb prueft diese hier die
    /// Umrechnung selbst und nicht das Bild.
    #[test]
    fn der_wagen_trifft_die_zeile_die_beschrieben_wurde() {
        let s = Schirm { hoehe: 40 };
        for versatz in 0..RESERVE {
            let geschrieben = s.zeile(versatz);
            // Die ANSI-Zeile aus der Sequenz herauslesen, nicht
            // nachrechnen.
            let ansi: u16 = geschrieben
                .trim_start_matches("\x1b[")
                .split(';')
                .next()
                .and_then(|z| z.parse().ok())
                .expect("Zeilennummer");
            assert_eq!(
                s.wagenzeile(versatz),
                ansi - 1,
                "Wagen und Text liegen bei Versatz {versatz} auseinander"
            );
        }
    }

    /// ⚑ **Das Gespraech geht dort weiter, wo der Text aufhoerte**, und
    /// nur dort, wo das sicher ist.
    ///
    /// 📌 Die Gegenproben sind die beiden Faelle, in denen es wie bisher
    /// unten weitergehen muss: Die Leerzeilen haben den Schirm gerollt,
    /// oder die Stelle liegt im Rahmen.
    #[test]
    fn das_gespraech_beginnt_unter_dem_text() {
        let s = Schirm { hoehe: 50 };
        // Unter einem Logo mit Ladezeile: Zeile 22 ab null ist 23 ab eins.
        assert_eq!(fortsetzungszeile(22, s.rollende()), Some(23));
        assert_eq!(fortsetzungszeile(0, s.rollende()), Some(1));
        // Die letzte Zeile des Rollbereichs ist noch seine, und von dort
        // aus rollen die Leerzeilen gerade noch nicht.
        let letzte = s.rollende() - 1;
        assert!(letzte + RESERVE < s.hoehe);
        assert_eq!(fortsetzungszeile(letzte, s.rollende()), Some(s.rollende()));
        // Eine Zeile tiefer rollten sie, und die Stelle gehoert dem Rahmen.
        assert!(letzte + 1 + RESERVE >= s.hoehe);
        assert_eq!(fortsetzungszeile(letzte + 1, s.rollende()), None);
        assert_eq!(fortsetzungszeile(s.hoehe - 1, s.rollende()), None);
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
