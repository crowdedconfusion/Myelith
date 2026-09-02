//! `myl-node` — die Verdrahtung, die aus Bibliotheken einen Knoten macht.
//!
//! # Warum es dieses Crate gibt
//!
//! Bis zum 2026-08-24 hatte dieses Repositorium dreizehn Komponenten,
//! rund 1500 Tests und **kein Programm, das einen Myelith-Knoten
//! startet**. `myl-net` hatte im ganzen Repositorium **keinen einzigen
//! Abnehmer**: Der Testclient hängt an neun Crates, an der Netzschicht
//! nicht.
//!
//! Das ist keine Ordnungsfrage. Es ist die Ursache einer ganzen
//! Fundklasse. Fund 52 (der Vergütungspfad war unbenutzbar), Fund 55
//! (der dokumentierte Prüf-Einstieg war über die Laufzeit nicht
//! erreichbar) und Fund 56 (ein Relais ohne eigene Adresse antwortet ins
//! Leere) haben eines gemeinsam: **Sie wurden sichtbar, als jemand die
//! Teile zusammensteckte**, nicht beim Lesen und nicht in den Tests der
//! einzelnen Crates. Eine Naht, die niemand belastet, hält alles aus.
//!
//! Dieses Crate ist die Stelle, an der die Nähte belastet werden.
//!
//! # Die Schichtgrenze, die es auflöst
//!
//! `myl-net` ist L0 und darf `myl-consensus` (L1) nicht kennen, sonst
//! wäre die Schichtung umgekehrt. Deshalb prüft die Netzschicht Blöcke
//! und Transaktionen nur auf ihre Größe und reicht den Rest über
//! [`myl_net::validation::PayloadValidator`] nach oben.
//!
//! **Der Knoten kennt beide Seiten.** Er ist der vorgesehene Ort für
//! diese Prüfung, und [`validator`] füllt ihn aus. Was dort geprüft
//! wird und was nicht, steht dort und ist bewusst knapp gehalten: Ein
//! Knoten, der ohne Kettenzustand über Gültigkeit urteilt, urteilt
//! falsch.
//!
//! # Das Betriebsprotokoll
//!
//! Ein verteilter Testlauf über mehrere Maschinen ist nur so viel wert
//! wie das, was danach rekonstruierbar ist. [`protokoll`] schreibt
//! deshalb jede Zustandsänderung als eine Zeile JSON: Verbindungen mit
//! Richtung und Weg, Nachrichten mit Topic und Urteil, Adressen,
//! NAT-Feststellungen.
//!
//! Der Punkt dabei ist die **Zusammenführbarkeit**: Jede Zeile trägt
//! Knoten-Id, Wanduhr und eine lückenlose Folgenummer. Erst damit lassen
//! sich die Protokolle mehrerer Maschinen nebeneinanderlegen und die
//! Frage „wer sah was wann" beantworten. Details in [`protokoll`].

#![deny(unsafe_code)]

pub mod beobachtung;
pub mod genesis;
pub mod kette;
pub mod knoten;
pub mod konformitaetstor;
pub mod konfig;
pub mod konsens;
pub mod nachschub;
pub mod probe;
pub mod protokoll;
pub mod schluessel;
pub mod speicher;
pub mod stichprobe;
pub mod validator;
pub mod validatorsatz;

pub use genesis::{Genesis, GenesisFehler, GenesisValidator};
pub use kette::{Kette, KettenFehler};
pub use knoten::{Knoten, KnotenFehler};
pub use konfig::{KnotenKonfig, KonfigFehler, Rolle};
pub use konsens::{KonsensFehler, Konsensrunde, Urteil};
pub use nachschub::{Nachforderung, Nachlieferung};
pub use probe::Probe;
pub use protokoll::{Betriebsprotokoll, Eintrag, ProtokollFehler};
pub use schluessel::{Herkunft, Konsensschluessel, SchluesselFehler};
pub use speicher::{Kettenspeicher, SpeicherFehler, Wiederanlauf};
pub use validator::ProtokollValidator;
pub use validatorsatz::{Attesturteil, Validatorsatz};
