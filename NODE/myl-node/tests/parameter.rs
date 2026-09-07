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

/// Die Lernrate der Kette trägt die Tiefe, über die sie rechnet.
///
/// # ⚑ Warum das eine eigene Prüfung braucht
///
/// Bis zum 2026-09-06 stand die Lernrate als Konstante `1 << 12` im
/// Knoten, und sie war gemessen falsch: Über die vierundzwanzig Ebenen
/// des Primärmodells zerstört sie das Modell in einem Lauf, Perplexität
/// 2,5 Milliarden gegen einen Ausgangsstand von 23.
///
/// **Eine Konstante hätte niemand nachgerechnet.** Diese Prüfung hält
/// die Zahl, die der Knoten bestellt, gegen die Schranke aus
/// `myl_types::lernrate`, und sie fällt, sobald jemand die Tiefe des
/// Modells erhöht, ohne die Rate mitzuziehen.
#[test]
fn die_lernrate_traegt_die_tiefe_des_modells() {
    let tiefe = myl_tokenomics::vtfe::PROBE_MODELL.num_layers as u32;
    // Ein Segment aus dreissig Schritten zu je acht Folgen: 240
    // Gradienten.
    let gradienten = 30 * 8;
    let nenner = myl_types::lernrate::lernrate_nenner(tiefe, gradienten);
    assert!(
        myl_types::lernrate::nenner_traegt(nenner, tiefe, gradienten),
        "die eigene Kurve traegt ihre eigene Zahl nicht"
    );
    // Und die alte Konstante faellt durch, sonst prueft der Test nichts.
    assert!(
        !myl_types::lernrate::nenner_traegt(1 << 12, tiefe, gradienten),
        "die alte Konstante muesste durchfallen"
    );
    // ⚑ **Und mehr Folgen verlangen eine kleinere Rate.** Ohne diese
    // Zeile pruefte der Test nur die Tiefe, und genau das war der
    // Mangel der ersten Fassung.
    assert!(
        !myl_types::lernrate::nenner_traegt(nenner, tiefe, gradienten * 2),
        "die doppelte Chargengroesse muesste dieselbe Rate reissen"
    );
}

/// Der Mindesteinsatz des Knotens deckt sich mit der Registry.
///
/// ⚑ **Der Knoten zieht `myl-governance` nicht ein** und rechnet die
/// Zahl deshalb selbst; ohne diese Prüfung stimmten Kette und Beschluss
/// über das Stimmrecht nicht überein.
#[test]
fn der_mindesteinsatz_deckt_sich_mit_der_registry() {
    let reg = ParameterRegistry::vorgabe();
    let aus_registry = ganzzahl(&reg, Parameter::MindestStake);
    assert_eq!(
        myl_node::kette::Kette::mindesteinsatz(),
        aus_registry,
        "der Knoten rechnet einen anderen Mindesteinsatz als die Registry"
    );
}

/// Der Slashsatz des Knotens deckt sich mit Kapitel 5.5.
#[test]
fn der_slashsatz_deckt_sich_mit_der_matrix() {
    let zeile = myl_tokenomics::slashing::matrix()
        .into_iter()
        .find(|z| {
            matches!(z.akteur, myl_tokenomics::slashing::Akteur::ShardMiner)
                && matches!(z.grund, myl_tokenomics::slashing::Grund::FalschesErgebnis)
        })
        .expect("die Zeile steht in der Matrix");
    let aus_matrix = zeile.als_ledger_parameter();
    let aus_kette = myl_node::kette::Kette::slashsatz_oeffentlich();
    assert_eq!(aus_kette.slash_fraction_num, aus_matrix.slash_fraction_num);
    assert_eq!(aus_kette.slash_fraction_den, aus_matrix.slash_fraction_den);
    assert_eq!(aus_kette.bounty_fraction_num, aus_matrix.bounty_fraction_num);
    assert_eq!(aus_kette.bounty_fraction_den, aus_matrix.bounty_fraction_den);
}
