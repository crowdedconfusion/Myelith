//! Wie viel Maschine der Rechenpfad nehmen darf.
//!
//! # ⚑ Warum das hier steht und nicht beim Aufrufer
//!
//! Ein Knoten gibt einen **Teil** seiner Maschine her, nicht die ganze.
//! Ohne eine Grenze nimmt sich der Rechenpfad, was
//! `available_parallelism` meldet, und eine Kapazitaetseinstellung, die
//! das nicht begrenzt, ist eine Anzeige und keine Einstellung.
//!
//! ⚑ **Die Grenze aendert kein Ergebnis.** Jede Ausgabezeile einer
//! Matrixmultiplikation ist ein eigenes Skalarprodukt ueber ihre eigene
//! Gewichtszeile; zwischen den Zeilen gibt es keine gemeinsame
//! Zwischensumme und damit keine Reihenfolge, die etwas aendern
//! koennte. Der Beleg dafuer ist die Pruefung
//! `dieselbe_antwort_bei_jeder_kernzahl` in `integer-llm-kernels`.
//!
//! Der Aufrufer soll die Kernkiste nicht kennen muessen; deshalb steht
//! die Naht hier.

/// Setzt die Obergrenze der Kerne fuer diesen Prozess.
///
/// `0` gibt die Maschine wieder frei. Eine Zahl ueber der Kernzahl der
/// Maschine hebt sie nicht an.
///
/// ⚑ **Gedacht fuer den Aufruf beim Start**, aus der Einstellung des
/// Nutzers, und nicht fuer die Feinsteuerung waehrend eines Laufs.
pub fn kerne_setzen(n: usize) {
    integer_llm_kernels::linear::kerngrenze_setzen(n);
}

/// Wie viele Kerne der Rechenpfad gerade hoechstens nimmt.
pub fn kerne() -> usize {
    integer_llm_kernels::linear::kerngrenze()
}
