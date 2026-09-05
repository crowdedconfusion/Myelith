//! Die Konstanten der Kette gegen die Parameter-Registry.
//!
//! # ⚑ Warum es diesen Test gibt, und was er über den Bau sagt
//!
//! Der Knoten bindet `myl-governance` **nicht** ein. Die Registry ist
//! damit vom Konsenspfad aus unerreichbar, und jeder Parameter, den die
//! Kette braucht, steht bei ihr ein zweites Mal als `const`.
//!
//! ⚑ **Das ist ein Behelf und keine Lösung.** Die eigentliche Antwort
//! wäre, die Parameter in den Kettenzustand zu nehmen, damit ein
//! Beschluss sie bewegt statt eines neuen Baus. Solange das nicht steht,
//! hält wenigstens dieser Test die beiden Fassungen zusammen.
//!
//! ⚑ **Und `myl-governance` steht nur unter `dev-dependencies`.** Wäre
//! es eine gewöhnliche Abhängigkeit, sähe es aus, als läse die Kette die
//! Registry, und genau das tut sie nicht.

use myl_governance::registry::{Parameter, ParameterRegistry, Wert};

fn ganzzahl(reg: &ParameterRegistry, p: Parameter) -> u64 {
    match reg.wert(p) {
        Wert::Ganzzahl(v) => *v,
        andere => panic!("{} ist {andere:?} und keine Ganzzahl", p.name()),
    }
}

/// Die drei Zahlen der Trainingszuteilung stimmen überein.
#[test]
fn die_trainingsparameter_stimmen_mit_der_registry_ueberein() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        myl_node::kette::Kette::pod_kapazitaet_vtfe(),
        ganzzahl(&reg, Parameter::PodKapazitaet),
        "die Pod-Kapazitaet der Kette weicht von der Registry ab"
    );
    assert_eq!(
        u64::from(myl_node::kette::Kette::trainings_grundrate_bps()),
        ganzzahl(&reg, Parameter::TrainingsGrundrate),
        "die Trainings-Grundrate weicht ab"
    );
    assert_eq!(
        u64::from(myl_node::kette::Kette::trainings_freianteil_bps()),
        ganzzahl(&reg, Parameter::TrainingsFreianteil),
        "der Trainings-Freianteil weicht ab"
    );
}

/// ⚑ **Die Gegenprobe zum Test selbst.** Ein Vergleich, dessen beide
/// Seiten aus derselben Quelle kämen, wäre keiner. Die Registry muss
/// eine Abweichung auch melden.
#[test]
fn eine_abweichung_faellt_auf() {
    let reg = ParameterRegistry::vorgabe()
        .mit(Parameter::TrainingsGrundrate, Wert::Ganzzahl(999))
        .expect("Wert setzbar");
    assert_ne!(
        u64::from(myl_node::kette::Kette::trainings_grundrate_bps()),
        ganzzahl(&reg, Parameter::TrainingsGrundrate),
        "eine geaenderte Registry sieht aus wie die unveraenderte"
    );
}
