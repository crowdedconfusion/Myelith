//! Die Registry gegen die Konstanten der anderen Crates (Punkt 1.1).
//!
//! **Ohne diesen Test wäre die Registry das gefährlichste Artefakt im
//! Repositorium.** Sie behauptet, die maßgebliche Liste der Parameter zu
//! sein, während gerechnet wird mit den Konstanten in
//! `myl-tokenomics`, `myl-consensus` und den übrigen Crates. Zwei Orte
//! für denselben Wert, und einer davon wird gelesen.
//!
//! Genau dieses Muster hat das Projekt dreimal bezahlt: A7 (totes
//! Stimmgewicht), Fund 44 (die ganzzahligen EMA-Konstanten lagen
//! ungenutzt neben der Gleitkommarechnung) und Fund 25 (`pipeline_hash`
//! auf `sha256:0000`). Jedes Mal war die richtige Fassung vorhanden und
//! lief nicht.
//!
//! Der Test hält die beiden Orte zusammen. Läuft einer davon, schlägt er
//! fehl, und dann ist zu entscheiden, welcher recht hat.

use myl_governance::registry::{Parameter, ParameterRegistry};
use myl_tokenomics::UNITS_PER_MYL;

fn bruch(reg: &ParameterRegistry, p: Parameter) -> (u64, u64) {
    reg.wert(p).als_bruch().expect("Bruch")
}
fn zahl(reg: &ParameterRegistry, p: Parameter) -> u64 {
    reg.wert(p).als_ganzzahl().expect("Ganzzahl")
}

/// Die EMA-Glättung, gegen `myl_tokenomics::EMA_ALPHA_NUM/DEN`.
#[test]
fn ema_glaettung_stimmt_mit_tokenomics() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        bruch(&reg, Parameter::EmaGlaettung),
        (myl_tokenomics::EMA_ALPHA_NUM, myl_tokenomics::EMA_ALPHA_DEN)
    );
}

/// Die Trainingsvergütungs-Obergrenze, gegen `TRAINING_CAP_BPS`.
#[test]
fn trainingsverguetung_stimmt_mit_tokenomics() {
    let reg = ParameterRegistry::vorgabe();
    let (z, n) = bruch(&reg, Parameter::TrainingsverguetungsAnteil);
    assert_eq!(z, myl_tokenomics::TRAINING_CAP_BPS);
    assert_eq!(n, 10_000);
    // Und die Funktion selbst muss denselben Betrag liefern.
    assert_eq!(
        myl_tokenomics::training_reward_cap(1_000_000),
        1_000_000 * z / n
    );
}

/// Der Redundanzfaktor, gegen `redundancy_normalized_weight`.
///
/// Die Funktion halbiert, weil `r = 2` gilt. Stiege `r`, müsste sie
/// durch `r` teilen; der Test hält fest, dass beides zusammengehört.
#[test]
fn redundanzfaktor_stimmt_mit_der_normierung() {
    let reg = ParameterRegistry::vorgabe();
    let r = zahl(&reg, Parameter::Redundanzfaktor);
    assert_eq!(r, 2);
    assert_eq!(myl_tokenomics::redundancy_normalized_weight(1_000), 1_000 / r);
}

/// Die Komiteegröße, gegen `myl_consensus::validator::COMMITTEE_SIZE`.
#[test]
fn komiteegroesse_stimmt_mit_consensus() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        zahl(&reg, Parameter::Komiteegroesse) as usize,
        myl_consensus::validator::COMMITTEE_SIZE
    );
}

/// Der Mindest-Stake, gegen die Formel aus `myl_tokenomics::sicherheit`.
///
/// Die Vorgabewerte müssen das Zahlenbeispiel aus Anhang B.1 ergeben:
/// g = 0,5 MYL, p = 2 % → S_min = 1250 MYL.
#[test]
fn mindeststake_stimmt_mit_anhang_b1() {
    let reg = ParameterRegistry::vorgabe();
    let (pz, pn) = bruch(&reg, Parameter::Stichprobenrate);
    let g = zahl(&reg, Parameter::Betrugsgewinn);
    let s = zahl(&reg, Parameter::MindestStake);
    assert_eq!(g, UNITS_PER_MYL / 2);
    assert_eq!(myl_tokenomics::s_min(g, pz, pn).unwrap(), 1_250 * UNITS_PER_MYL);
    assert_eq!(s, 1_250 * UNITS_PER_MYL);
}

/// **Die Streitfrist, gegen `myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS`.**
///
/// Der Vergleich braucht die Epochenlänge, und genau daran hing ⚑ Fund 50:
/// Sie war nirgends festgelegt, und zwei Teile des Projekts haben
/// stillschweigend verschiedene Werte angenommen.
#[test]
fn streitfrist_stimmt_mit_consensus() {
    let reg = ParameterRegistry::vorgabe();
    let frist_s = zahl(&reg, Parameter::Streitfrist);
    let epoche_s = zahl(&reg, Parameter::Epochenlaenge);
    assert_eq!(
        frist_s / epoche_s,
        myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS,
        "Streitfrist {frist_s} s bei Epochenlänge {epoche_s} s sind {} Epochen, \
         die Konstante sagt {}",
        frist_s / epoche_s,
        myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS
    );
    assert_eq!(frist_s % epoche_s, 0, "die Frist muss ein Vielfaches der Epoche sein");
}

/// Die Blockzeit, gegen die Design-Entscheidung vom 2026-08-13 (2 s).
#[test]
fn blockzeit_ist_die_entschiedene() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(zahl(&reg, Parameter::Blockzeit), 2_000);
}

/// Der Kopfgeldanteil, gegen die Slashing-Matrix aus TOKENOMICS.
#[test]
fn kopfgeldanteil_stimmt_mit_der_slashing_matrix() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        bruch(&reg, Parameter::Kopfgeldanteil),
        (
            myl_tokenomics::slashing::KOPFGELD_ZAEHLER,
            myl_tokenomics::slashing::KOPFGELD_NENNER
        )
    );
    // Und jede Zeile der Matrix trägt denselben Satz.
    for s in myl_tokenomics::slashing::matrix() {
        let p = s.als_ledger_parameter();
        assert_eq!(p.bounty_fraction_num, myl_tokenomics::slashing::KOPFGELD_ZAEHLER);
        assert_eq!(p.bounty_fraction_den, myl_tokenomics::slashing::KOPFGELD_NENNER);
    }
}

/// **Die Self-Dealing-Prüfung der Registry ist genau die Formel aus
/// TOKENOMICS**, nicht eine zweite Fassung davon.
///
/// Geprüft über 600 Werte von `s`: Die Registry muss **genau dann**
/// ablehnen, wenn `self_dealing_sicher_konservativ` das sagt. Liefe eine
/// der beiden Seiten davon, fiele es hier auf.
///
/// Seit Fund 49 geht `c` nicht mehr in die Prüfung ein; dass es das nicht
/// tut, hält `akzeptanz.rs::kein_kostenanteil_bewegt_die_self_dealing_grenze`
/// fest.
#[test]
fn die_registry_prueft_genau_die_formel_aus_tokenomics() {
    use myl_governance::registry::Wert;
    use myl_governance::{pruefe_vorschlag, ParameterVorschlag, VorschlagFehler};

    let basis = ParameterRegistry::vorgabe();
    let mut geprueft = 0usize;
    for sz in 0..=600u64 {
        let erwartet_sicher =
            myl_tokenomics::self_dealing_sicher_konservativ(sz, 100).expect("auswertbar");
        let ergebnis = pruefe_vorschlag(
            &basis,
            &ParameterVorschlag {
                parameter: Parameter::Subventionsrate,
                neuer_wert: Wert::Bruch { zaehler: sz, nenner: 100 },
            },
        );
        let angenommen = ergebnis.is_ok();
        if erwartet_sicher {
            assert!(
                angenommen,
                "s = {sz}/100 ist sicher, wurde aber abgelehnt: {ergebnis:?}"
            );
        } else {
            assert!(
                matches!(ergebnis, Err(VorschlagFehler::Invariante(_))),
                "s = {sz}/100 ist unsicher, wurde aber angenommen"
            );
        }
        geprueft += 1;
    }
    assert!(geprueft > 300, "nur {geprueft} Werte geprüft");
}

/// **Der Arbeitsbezug des Stimmgewichts, gegen `myl-consensus`** (Fund 51).
///
/// Der Wert ist aus einer Durchsatzmessung abgeleitet und **veraltet mit
/// jeder Optimierung**; genau das ist am 2026-08-23 unbemerkt geschehen,
/// als die Zeilen-Parallelisierung den Durchsatz um das 5,19-Fache hob
/// und der Bezug stehen blieb. Dieser Test ist der Grund, warum es kein
/// zweites Mal unbemerkt geschieht.
#[test]
fn arbeitsbezug_und_hoechstfaktor_stimmen_mit_consensus() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        zahl(&reg, Parameter::Arbeitsbezug),
        myl_consensus::voting_weight::ARBEITSBEZUG_VORGABE
    );
    assert_eq!(
        zahl(&reg, Parameter::Hoechstfaktor),
        myl_consensus::voting_weight::HOECHSTFAKTOR_VORGABE
    );

    // Und die Kalibrierungsaussage selbst: Eine Epoche Referenzarbeit ist
    // etwa einen Stake wert, nicht das Fünffache und nicht ein Fünftel.
    let stake = 10_000_000u64;
    let mut history = myl_consensus::voting_weight::InferenceHistory::new();
    history.add_work(1, zahl(&reg, Parameter::Arbeitsbezug));
    let gewicht = myl_consensus::voting_weight::calculate_voting_weight(stake, &history, 1);
    assert!(
        gewicht > stake * 19 / 10 && gewicht < stake * 21 / 10,
        "eine Epoche Referenzarbeit muss etwa einen Stake Bonus ergeben, ergab {gewicht}"
    );
}

/// Die Trainingsrate der Registry liegt über der Inferenzrate.
#[test]
fn die_trainingsrate_liegt_ueber_der_inferenzrate() {
    let reg = ParameterRegistry::vorgabe();
    let (pz, pn) = bruch(&reg, Parameter::Stichprobenrate);
    let (tz, tn) = bruch(&reg, Parameter::TrainingsStichprobenrate);
    assert!((tz as u128) * (pn as u128) > (pz as u128) * (tn as u128));
    assert_eq!((tz, tn), (10, 100), "Entwurf: 10 %");
}
