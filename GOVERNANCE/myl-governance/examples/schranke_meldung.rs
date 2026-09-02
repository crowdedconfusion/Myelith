//! Zeigt die Meldung, mit der ein invariantenbrechender Vorschlag
//! abgelehnt wird.
//!
//! **Warum als Beispiel und nicht nur als Test:** Eine Fehlermeldung
//! wird im Fehlerfall als Einzige gelesen, und beim Lesen der
//! Dokumentation fällt sie nicht auf. Wer eine Schranke ändert, sieht
//! hier in einem Aufruf, was ein Antragsteller danach zu lesen bekommt.
//!
//! ⚑ **Bis zum 2026-09-02 lief dieses Beispiel über den
//! Kontrollsegment-Vorrat** (Fund 58). Der Parameter ist mit den
//! Kontrollsegmenten entfallen; der Zweck des Beispiels ist geblieben.
fn main() {
    use myl_governance::registry::{Parameter, ParameterRegistry, Wert};
    use myl_governance::{pruefe_vorschlag, ParameterVorschlag};

    let reg = ParameterRegistry::vorgabe();
    for (p, w) in [
        // Trainingsrate unter die Inferenzrate: der groessere Schaden
        // waere schlechter geschuetzt als der kleinere.
        (
            Parameter::TrainingsStichprobenrate,
            Wert::Bruch { zaehler: 1, nenner: 100 },
        ),
        // Eine Rate ueber eins ist keine Rate.
        (
            Parameter::Stichprobenrate,
            Wert::Bruch { zaehler: 2, nenner: 1 },
        ),
        // Die Preisuntergrenze auf null hiesse kostenlose Inferenz.
        (Parameter::PreisUntergrenze, Wert::Ganzzahl(0)),
    ] {
        match pruefe_vorschlag(
            &reg,
            &ParameterVorschlag {
                parameter: p,
                neuer_wert: w.clone(),
            },
        ) {
            Ok(_) => println!("  {} auf {:?}: angenommen", p.name(), w),
            Err(e) => println!("  {} auf {:?}:\n      {}\n", p.name(), w, e),
        }
    }
}
