//! `myl-simulation` — der Weg eines Segments durch alle Schichten.
//!
//! # Wozu ein eigenes Crate
//!
//! Jede Komponente ist für sich getestet, teils gründlich. **Was keine
//! von ihnen prüfen kann, ist die Naht zur nächsten**, und dort saßen
//! die schwersten Funde dieses Projekts:
//!
//! | Fund | Naht |
//! |---|---|
//! | 41 | Pod-Drahtformat gegen Kernel: leere Spur belegte nichts |
//! | 44 | Crate-Zusage gegen Audit-Liste: `myl-net` stand nirgends |
//! | 50 | Konsens-Konstante gegen Epochenlänge: Faktor 24 |
//! | 51 | INTEGER_LLM-Durchsatz gegen Stimmgewicht: Faktor 5,19 |
//! | 52 | Pod-Bündel gegen Konsens-Prüfung: zwei Botschaften |
//!
//! Alle fünf waren **in ihrer Komponente korrekt**. Ein Crate, das an
//! alle hängt und an dem keine hängt, ist die Stelle, an der so etwas
//! auffällt, bevor es ein Betreiber findet.
//!
//! # Was hier nicht hingehört
//!
//! Keine Logik. Dieses Crate rechnet nichts selbst, sonst wäre es eine
//! zweite Wahrheit neben den Komponenten. Es **verdrahtet** sie und sieht
//! zu.

#![deny(unsafe_code)]

pub mod szenario;

pub use szenario::{Abdeckung, Befund, Protokolllauf, Schwere, Teilnehmer};
