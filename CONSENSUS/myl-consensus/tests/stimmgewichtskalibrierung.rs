//! Was die Kalibrierung des Stimmgewichts tatsächlich tut.
//!
//! # ⚑ Wovon diese Datei handelte, und warum sie umgeschrieben ist
//!
//! Sie entstand am 2026-09-02, um **Fund 135** festzuhalten: Der
//! Höchstfaktor der alten Summenform griff ab **1,13-fachem**
//! Referenzdurchsatz, darüber ergaben 1,2-fach und hundertfach denselben
//! Wert. Vier Tests hielten das fest.
//!
//! **Am selben Tag ist die Formel entfallen** (Entscheidung des
//! Projektinhabers: Arbeit qualifiziert, Stake wiegt), und mit ihr der
//! Gegenstand jener vier Tests. Die Messung bleibt trotzdem wichtig,
//! denn sie ist die Begründung der Entscheidung; sie steht jetzt im
//! Modulkopf von `voting_weight.rs`, wo sie jeder findet, der die Formel
//! liest.
//!
//! **Was hier bleibt, ist die Kalibrierung der Nachfolgerin:** die
//! Arbeitsschwelle. Sie ist der einzige Parameter, der zwischen
//! Validatoren unterscheidet, und ihr Startwert ist der einzige, bei dem
//! ein Netz ohne Historie überhaupt anfangen kann.
//!
//! ⚑ **Der Unterschied zu einer Formelprüfung gehört benannt.** Die
//! Tests in `voting_weight.rs` prüfen, dass die Funktionen tun, was sie
//! sagen. Diese hier prüfen, dass die **Zahlen** die Wirkung haben, die
//! jemand beabsichtigt hat. Fund 51 und Fund 135 waren beide von der
//! zweiten Sorte, und keine Formelprüfung hat sie gefunden.

use myl_consensus::voting_weight::*;

/// **Die Vorgabeschwelle ist null, und das ist keine Nachlässigkeit.**
///
/// Ein Wert über null schlösse bei Genesis jeden Validator aus, denn
/// niemand hat Arbeitshistorie, und wer ausgeschlossen ist, sammelt
/// keine und kommt nie herein. Der Test hält den Startwert fest, damit
/// niemand ihn nebenbei hebt.
#[test]
fn die_vorgabeschwelle_ist_null() {
    let p = StimmgewichtsParameter::default();
    assert_eq!(p.schwelle_zaehler, 0, "die Vorgabeschwelle muss null sein");
    assert!(p.ist_brauchbar());
    assert!(
        arbeitsqualifikation(&InferenceHistory::new(), 0, 0, &p),
        "bei Genesis muss ein Validator ohne Arbeit qualifiziert sein"
    );
}

/// ⚑ **Was eine Schwelle über null heute kostet, in Zahlen.**
///
/// Solange `ValidatorRegistry::record_work` keinen Aufrufer hat
/// (**Fund 137**), ist jede Historie leer, jeder Median null und jede
/// Schwelle wirkungslos. Hebt jemand die Schwelle, **bevor** bezeugte
/// Arbeit die Validatoren erreicht, ändert sich deshalb nichts, und
/// genau das ist die Falle: Es sieht nach einer wirksamen Verschärfung
/// aus.
///
/// Der Test hält beide Hälften fest: Bei leeren Historien qualifiziert
/// jede Schwelle jeden, und sobald Arbeit da ist, trennt sie.
#[test]
fn eine_schwelle_wirkt_erst_wenn_arbeit_ankommt() {
    let streng = StimmgewichtsParameter {
        schwelle_zaehler: 1,
        schwelle_nenner: 2,
    };

    // Zehn Validatoren, alle ohne Historie: der Median ist null.
    let mut werte: Vec<u64> = vec![0; 10];
    let median = netzmedian(&mut werte);
    assert_eq!(median, 0);
    assert!(
        arbeitsqualifikation(&InferenceHistory::new(), 5, median, &streng),
        "ohne Arbeit im Netz darf keine Schwelle jemanden ausschliessen"
    );

    // Und mit Arbeit trennt dieselbe Schwelle.
    let mut mit_arbeit: Vec<u64> = (1..=10u64).map(|i| i * 100_000).collect();
    let median = netzmedian(&mut mit_arbeit);
    assert_eq!(median, 500_000, "Median von 100k bis 1M ist der fuenfte Wert");

    let mut schwach = InferenceHistory::new();
    schwach.add_work(5, 200_000);
    assert!(
        !arbeitsqualifikation(&schwach, 5, median, &streng),
        "unter der halben Medianarbeit darf nicht qualifiziert werden"
    );

    let mut stark = InferenceHistory::new();
    stark.add_work(5, 400_000);
    assert!(arbeitsqualifikation(&stark, 5, median, &streng));
}

/// **Der Arbeitsanteil bewegt das Gewicht nicht mehr, und zwar in
/// keinem Umfang.**
///
/// Das ist die Aussage, die vor dem 2026-09-02 falsch war und für die
/// es keinen Test gab: Damals unterschied der Arbeitsanteil bis
/// 1,13-fachem Referenzdurchsatz und darüber nicht mehr, und niemand
/// hatte die Zahl ausgerechnet.
#[test]
fn kein_umfang_an_arbeit_bewegt_das_gewicht() {
    let stake = 1_000_000_000u64;
    let ohne = calculate_voting_weight(stake, &InferenceHistory::new(), 10);
    assert_eq!(ohne, stake);

    for faktor in [1u64, 10, 100, 10_000] {
        let mut h = InferenceHistory::new();
        for e in 1..=MAX_HISTORY_EPOCHS as u64 {
            h.add_work(e, 8_900_000_000u64.saturating_mul(faktor));
        }
        assert_eq!(
            calculate_voting_weight(stake, &h, MAX_HISTORY_EPOCHS as u64),
            stake,
            "das {faktor}-fache der alten Bezugsarbeit hat das Gewicht bewegt"
        );
    }
}
