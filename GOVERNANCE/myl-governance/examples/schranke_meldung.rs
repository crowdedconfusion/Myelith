//! Zeigt die Meldung, mit der ein zu kleiner Kontrollsegment-Vorrat
//! abgelehnt wird.
//!
//! **Warum als Beispiel und nicht nur als Test:** Eine Fehlermeldung
//! wird im Fehlerfall als Einzige gelesen, und beim Lesen der
//! Dokumentation fällt sie nicht auf. Wer die Schranke ändert, sieht
//! hier in einem Aufruf, was ein Antragsteller danach zu lesen bekommt.
fn main() {
    use myl_governance::registry::{Parameter, ParameterRegistry, Wert};
    use myl_governance::{pruefe_vorschlag, ParameterVorschlag};

    let reg = ParameterRegistry::vorgabe();
    for (p, w) in [
        (Parameter::Kontrollsegmentvorrat, Wert::Ganzzahl(64)),
        (Parameter::Kontrollsegmentanteil, Wert::Bruch { zaehler: 4, nenner: 100 }),
        (Parameter::Kontrollsegmentfenster, Wert::Ganzzahl(1_000_000)),
    ] {
        match pruefe_vorschlag(&reg, &ParameterVorschlag { parameter: p, neuer_wert: w.clone() }) {
            Ok(_) => println!("  {} auf {:?}: angenommen", p.name(), w),
            Err(e) => println!("  {} auf {:?}:\n      {}\n", p.name(), w, e),
        }
    }
}
