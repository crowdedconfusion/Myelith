//! `myl-store` — Speicher- und Verfügbarkeitsschicht (Whitepaper Kap. 3.3,
//! Rolle **Store**).
//!
//! ⚑ **Die Rolle steht heute in keinem Kapitel des Whitepapers.**
//! Kap. 3.3 kennt sechs Rollen, Store ist keine davon; das gehört in die
//! nächste Fassung. Gebraucht wird sie trotzdem: Woher ein Miner die
//! Gewichte seines Shards bezieht, wer die Skalenpakete vorhält und wie
//! eine wachsende Wissensdatenbank verteilt und geprüft wird, beantwortet
//! bisher niemand.
//!
//! ## Was hier zuerst entsteht, ist wieder ein Format
//!
//! Wie beim Agent Layer: **kein Speicher, sondern die Frage, was ein
//! Gegenstand ist.** Sie liegt vor allem anderen, weil Zuteilung,
//! Rotation und Nachweis sie voraussetzen.
//!
//! ## ⚑ Gehasht wird der Klartext, nicht das Komprimat
//!
//! Zwei zstd-Fassungen komprimieren dieselben Bytes verschieden. Wer den
//! Hash des Komprimats verankert, **macht den Kompressor zum
//! Konsensvertrag**, und ein Bibliotheksupdate wird zum Betrugsvorwurf.
//! Kap. 6.2 trifft dieselbe Unterscheidung für die Ausführung: Der
//! Inhalt ist verbindlich, die Kodierung nicht.
//!
//! **Der Preis steht hier statt in einer Fußnote:** Ein
//! Verfügbarkeitsnachweis muss dann über den **Klartext** fragen, also
//! entpackt der Halter zum Antworten. Das kostet Rechenzeit bei ihm.
//!
//! ## ⚑ Und der Nachweis fragt nach einem Blatt, nicht nach Bytes
//!
//! Ein Hash über den ganzen Teil belegt **Empfang, nicht Speicherung**:
//! Wer ihn einmal gesehen hat, wiederholt ihn für immer. Gefragt wird
//! deshalb nach einem zufälligen **Blattindex**, geantwortet mit Blatt
//! und Merkle-Pfad. **Der Fragende braucht nur die Wurzel**, und die
//! steht im Manifest; hielte er die Daten, wäre der ganze Nachweis
//! sinnlos.
//!
//! Der Baum liegt in `myl-types` und bindet seit Fund 77 die Blattzahl
//! mit, ist also injektiv.

#![deny(unsafe_code)]

pub mod gegenstand;
pub mod kappa;

pub use gegenstand::{
    teile_bilden, Gegenstandsart, Manifest, ManifestFehler, Redundanzform, Teil, MAX_TEILE,
    TEILGROESSE,
};
pub use kappa::{Kappa, KappaFehler, Uebergang};
