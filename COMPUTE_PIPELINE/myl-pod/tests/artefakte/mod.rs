//! Der Wächter steht seit dem 2026-09-03 in der Bibliothek.
//!
//! ⚑ **Weil ihn seither eine zweite Kiste braucht:** Der Test von der
//! Türklinke bis zum Modell liegt in `myl-testclient`, weil nur dort
//! Tür, Knoten und Pod zusammen sichtbar sind. Ein Testmodul ist von
//! dort nicht erreichbar, und die Regel ein zweites Mal hinzuschreiben
//! hiesse, zwei Wächter zu führen, die irgendwann verschieden wachen.
//!
//! Hier bleibt die Weiterleitung, damit die fünf Testdateien dieser
//! Kiste unverändert `mod artefakte;` sagen können.

pub use myl_pod::artefakte::*;
