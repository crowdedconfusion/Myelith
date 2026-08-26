//! Backend-Abstraktion der Kernel.
//!
//! # Entschieden am 2026-08-24: Das Trait bleibt, ungenutzt
//!
//! Diese Schicht war lange ein offener Punkt mit der
//! Feststellung, `model.rs` importiere die Kernel direkt und
//! `SimdBackend` werde ausschließlich im Paritätstest instanziiert,
//! „dasselbe Muster wie Fund A7 (totes Stimmgewicht) und Fund 25".
//!
//! **Der Vergleich war falsch, und das ist der Grund für diese Notiz.**
//! A7 war eine Formel, die aufgerufen werden **sollte** und es nicht
//! wurde; ihr Fehlen hatte eine Wirkung, nämlich ein Stimmgewicht ohne
//! Arbeitsanteil. Hier fehlt kein Aufruf: Es gibt genau ein CPU-Backend,
//! das rechnet, und die cfg-Weiche in `dot.rs` wählt es zur Bauzeit. Ein
//! Trait dazwischen hätte heute keinen zweiten Fall zu unterscheiden.
//!
//! **Was hier stattdessen liegt, ist die Vorarbeit für den GPU-Pfad.**
//! `cuda.rs` und `rocm.rs` sind Delegations-Stubs, und sie tragen den
//! **Determinismus-Vertrag** für künftige echte Kernel: warum jede
//! Reduktionsreihenfolge dasselbe `i64` liefert, warum Tensor Cores
//! ausscheiden, und warum die Sättigung genau einmal am Ende stehen
//! muss. Das ist das wichtigste Entwurfsdokument des GPU-Pfads, und der
//! GPU-Pfad ist keine Spekulation: Die offene wirtschaftliche Frage aus
//! K8 lässt sich ohne eine GPU-Messung nicht beantworten.
//!
//! **Erste Empfehlung war „abschaffen", zurückgenommen nach dem Lesen
//! von `cuda.rs`.** Sie stützte sich darauf, dass `SimdBackend` nur im
//! Paritätstest vorkommt, und übersah, was die beiden GPU-Dateien
//! enthalten. 1375 Zeilen zu löschen, davon der Determinismus-Vertrag,
//! wäre der teuerste Weg gewesen, eine Zeile der Planung abzuhaken.
//!
//! **Anzubinden ist das Trait, sobald ein Backend zur Laufzeit wählbar
//! sein muss** (GPU neben CPU auf derselben Maschine). Bis dahin ist die
//! Bauzeit-Weiche die ehrlichere Lösung, und ein Eingriff in `model.rs`
//! wäre ein Risiko im am besten belegten Teil des Projekts, ohne
//! Gegenwert.

pub mod reference;

#[cfg(feature = "cpu-simd")]
pub mod simd;

#[cfg(feature = "cuda")]
pub mod cuda;

#[cfg(feature = "rocm")]
pub mod rocm;
