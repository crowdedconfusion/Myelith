//! **Eine Maschine**: was sie hat, was die Lasten brauchen, und wie es
//! verteilt wird.

// ⚑ **`unsafe` steht an zwei Stellen dieser Kiste, und nur dort**: hier
// fuer die Erhebung (`sysctlbyname`, `statvfs`) und in `eichung` an drei
// Funktionen (`getrusage`, `proc_pid_rusage`, `proc_listchildpids`; die
// dritte stand bis zum 2026-10-23 nicht in dieser Liste). Arbeitsspeicher, Platte und
// Fussabdruck fragt man das Betriebssystem nur als C-Aufruf. Jeder Block
// traegt seinen Grund daneben.
#[allow(unsafe_code)]
pub mod erhebung;

pub mod bedarf;
pub mod eichung;
pub mod plan;
pub mod profil;
pub mod register;
