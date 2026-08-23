//! `myl-train` — Trainingsschicht (Whitepaper Kap. 7).
//!
//! **Was heute drin ist: die Datenprovenienz (Kap. 7.3).** Sie steht
//! hier vor allem anderen, obwohl der Fahrplan sie als Punkt 3.1 führt,
//! aus einem einzigen Grund: Sie ist **technisch unabhängig** vom
//! ganzzahligen Rückwärtspass. Alles Übrige aus Kap. 7 hängt an Punkt V
//! des Fahrplans, dem ganzzahligen Vorwärts- und Rückwärtspass in der
//! Trainingsschleife. Solange der Gradient aus einer Gleitkommarechnung
//! kommt, ist er geräteabhängig und mit ihm jedes Δm; die
//! Verifikationskette hinge in der Luft.
//!
//! ## Was das Verfahren leistet und was nicht
//!
//! Es prüft **Herkunft, nicht Inhalt**. Ein Segment stammt nachweislich
//! aus einem kanonisierten Korpus, oder es stammt nicht daher. Was es
//! **nicht** leistet: eine Aussage darüber, ob dieser Korpus in Ordnung
//! ist. Wer ihn kanonisiert, entscheidet das, und ein vergifteter
//! kanonischer Korpus fällt hier nicht auf. Das ist keine Lücke der
//! Umsetzung, sondern die Grenze des Verfahrens, und Kap. 7.3 benennt
//! sie als solche.
//!
//! ## Aufbau
//!
//! - [`provenienz`] — Verankerung, Segmentreferenz per Beweis,
//!   gebündelte Beweise, Kostenrechnung gegen Anhang B.6.4.
//! - [`zuweisung`] — VRF-gesteuerte Zuweisung von Korpusabschnitten,
//!   damit dem Miner nicht die Auswahl bleibt (Kap. 7.3, Anhang B.6.5).
//!
//! **Nicht enthalten:** die Ablehnungsquote für verweigerte Segmente.
//! Sie ist eine Buchführung über das Verhalten eines Miners über Epochen
//! hinweg und gehört zum Ledger; sie hier zu führen hieße, denselben
//! Zustand zweimal zu halten.

#![deny(unsafe_code)]

pub mod provenienz;
pub mod zuweisung;

pub use provenienz::{
    buendel_beweisknoten, buendel_overhead_zehntelpromille, BuendelReferenz, Korpus,
    ProvenienzFehler, SegmentReferenz,
};
pub use zuweisung::{zuweisen, Zuweisung, ZuweisungsFehler};
