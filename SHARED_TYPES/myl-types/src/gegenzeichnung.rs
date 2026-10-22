//! **Wie viele Mitglieder eines Pods eine Meldung über ein anderes
//! gegenzeichnen müssen.**
//!
//! ⚑ **Eine Regel für alle Meldungen** (2026-10-22). Sie stand bis heute in
//! `myl_pod::ausfallmeldung` und gilt jetzt auch für die Fristmeldung, die
//! die Kette prüft. Zwei Stellen mit derselben Mehrheit liefen auseinander,
//! und dann hielte die Kette eine Meldung für gültig, die der Pod
//! verworfen hätte, oder umgekehrt.

/// Untergrenze der Gegenzeichner, unabhängig von der Pod-Größe.
///
/// ⚑ **Zwei, nie einer.** Bei einer rechnerischen Mehrheit von eins
/// wäre die Gegenzeichnung keine: Ein einzelnes Mitglied entschiede
/// wieder allein. Ein Pod, der so klein ist, dass zwei Zeichner nicht
/// zusammenkommen, kann niemanden verdrängen; das ist die sichere
/// Richtung des Fehlers.
pub const MINDESTENS_ZEICHNER: usize = 2;

/// Wie viele Unterschriften eine Meldung braucht.
///
/// Mehrheit der **übrigen** Mitglieder, also ohne den Gemeldeten, und
/// mindestens [`MINDESTENS_ZEICHNER`]. Bei sechs Mitgliedern bleiben
/// fünf übrig, die Mehrheit ist drei.
///
/// **Warum Mehrheit und nicht Einstimmigkeit:** Einstimmigkeit gäbe
/// jedem einzelnen Mitglied ein Veto gegen jede Meldung, und ein Pod, der
/// nicht nachbesetzen kann, verliert die Sitzung. Der Angriff wechselte
/// damit nur die Richtung.
pub fn mindestzeichner(mitgliederzahl: usize) -> usize {
    let uebrige = mitgliederzahl.saturating_sub(1);
    (uebrige / 2 + 1).max(MINDESTENS_ZEICHNER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mehrheit_der_uebrigen_und_nie_weniger_als_zwei() {
        assert_eq!(mindestzeichner(6), 3, "sechs Mitglieder, fünf übrig");
        assert_eq!(mindestzeichner(3), 2);
        assert_eq!(mindestzeichner(2), 2, "eins übrig reicht nie");
        assert_eq!(mindestzeichner(0), 2);
    }
}
