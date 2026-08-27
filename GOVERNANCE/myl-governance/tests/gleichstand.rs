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

/// **Der Kontrollsegment-Vorrat und sein Fenster, gegen `myl-verifier`**
/// (⚑ Fund 58).
///
/// Beide Zahlen stammen aus der Messung vom 2026-08-25 und stehen dort,
/// wo der Mechanismus steht. Liefe die Registry davon, stünde in der
/// Governance eine Schranke, die mit der gemessenen nichts mehr zu tun
/// hat — und niemand könnte sagen, welche der beiden gilt.
#[test]
fn kontrollsegmentvorrat_und_fenster_stimmen_mit_verification() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        zahl(&reg, Parameter::Kontrollsegmentvorrat),
        myl_verifier::VORRAT_VORGABE
    );
    assert_eq!(
        zahl(&reg, Parameter::Kontrollsegmentfenster),
        myl_verifier::BEOBACHTUNGSFENSTER_VORGABE
    );
}

/// **Die Vorratsprüfung der Registry ist genau die Formel aus
/// VERIFICATION**, nicht eine zweite Fassung davon.
///
/// Geprüft über 400 Vorratsgrößen rund um die Schranke: Die Registry
/// muss **genau dann** ablehnen, wenn `noetiger_vorrat` mehr verlangt,
/// als der Vorschlag bietet. Liefe eine der beiden Seiten davon, fiele
/// es hier auf — dasselbe Muster wie bei der Self-Dealing-Grenze.
#[test]
fn die_registry_prueft_genau_die_formel_aus_verification() {
    use myl_governance::registry::Wert;
    use myl_governance::{pruefe_vorschlag, ParameterVorschlag, VorschlagFehler};

    let basis = ParameterRegistry::vorgabe();
    let fenster = zahl(&basis, Parameter::Kontrollsegmentfenster);
    let (gz, gn) = bruch(&basis, Parameter::Kontrollsegmentanteil);
    let noetig = myl_verifier::noetiger_vorrat(fenster, gz, gn);

    let mut abgelehnt = 0usize;
    let mut angenommen = 0usize;
    for delta in 0..400u64 {
        for vorrat in [noetig.saturating_sub(delta), noetig + delta] {
            let ergebnis = pruefe_vorschlag(
                &basis,
                &ParameterVorschlag {
                    parameter: Parameter::Kontrollsegmentvorrat,
                    neuer_wert: Wert::Ganzzahl(vorrat),
                },
            );
            if vorrat >= noetig {
                assert!(
                    ergebnis.is_ok(),
                    "Vorrat {vorrat} deckt die nötigen {noetig}, wurde aber abgelehnt: {ergebnis:?}"
                );
                angenommen += 1;
            } else {
                assert!(
                    matches!(ergebnis, Err(VorschlagFehler::Invariante(_))),
                    "Vorrat {vorrat} liegt unter den nötigen {noetig}, wurde aber angenommen"
                );
                abgelehnt += 1;
            }
        }
    }
    // **Beide Seiten müssen vorkommen.** Ein Test, in dem nichts
    // abgelehnt oder nichts angenommen wird, prüft die Grenze nicht,
    // sondern nur eine Richtung.
    assert!(abgelehnt > 100, "nur {abgelehnt} Ablehnungen");
    assert!(angenommen > 100, "nur {angenommen} Annahmen");
}

/// **Drei Fenster von zehn Epochen, und nur eines davon ist gebunden.**
///
/// - `myl_ledger::VERSTOSS_FENSTER` — wie lange der Ledger die
///   Verstoßhistorie **aufbewahrt**.
/// - `myl_tokenomics::WIEDERHOLUNGSFENSTER` — über wie lange die
///   Slashing-Matrix **staffelt**. Das ist dieselbe Konstante, nicht
///   eine zweite: Eine Staffelung über ein längeres Fenster als die
///   Aufbewahrung läse eine Vorgeschichte, die es nicht mehr gibt, und
///   der Zähler stünde einfach niedriger. Der Test hält es trotzdem
///   fest, denn eine Zuweisung kann jemand auflösen.
/// - `myl_consensus::voting_weight::MAX_HISTORY_EPOCHS` — wie lange die
///   Arbeitshistorie des Stimmgewichts zurückreicht. **Diese Gleichheit
///   ist Absicht, aber keine Kopplung:** Beide beantworten dieselbe
///   Frage, nämlich wie lange das Verhalten eines Teilnehmers nachwirkt.
///   Sie stehen als getrennte Konstanten, damit eine spätere
///   Entscheidung sie auseinanderziehen darf — dieser Test macht daraus
///   eine Entscheidung statt eines Versehens.
///
/// **Warum der Test hier steht:** `myl-tokenomics` kennt `myl-consensus`
/// nicht und umgekehrt. Diese Komponente kennt beide; sie ist der
/// einzige Ort, an dem die drei nebeneinanderliegen.
#[test]
fn die_drei_zehn_epochen_fenster_stimmen_ueberein() {
    assert_eq!(
        myl_tokenomics::WIEDERHOLUNGSFENSTER,
        myl_ledger::VERSTOSS_FENSTER,
        "Staffelung und Aufbewahrung der Verstoßhistorie liefen auseinander"
    );
    assert_eq!(
        myl_consensus::voting_weight::MAX_HISTORY_EPOCHS as u64,
        myl_ledger::VERSTOSS_FENSTER,
        "die Arbeitshistorie des Stimmgewichts und die Verstoßhistorie sind \
         verschieden lang; das darf sein, aber dann als Entscheidung mit \
         Begründung und nicht als stille Abweichung"
    );
}

/// Die Staffelung der Slashing-Matrix greift auf dem Zustand, den der
/// Ledger führt — nicht auf einer Zahl, die ein Aufrufer mitbringt.
///
/// **Der Gleichstand, um den es hier geht**, ist der zwischen der
/// Tabelle in Kap. 5.5 (1/3/5 %) und dem, was am Ende gebucht wird.
#[test]
fn die_staffelung_greift_auf_dem_ledger_zustand() {
    use myl_ledger::state::LedgerState;
    use myl_ledger::transitions::{Verdict, VerdictOutcome};
    use myl_tokenomics::slashing::{urteil_buchen_gestaffelt, Akteur, Grund};
    use myl_types::ids::{Address, SegmentId};

    let mut state = LedgerState::genesis(10);
    let verdict = Verdict {
        segment_id: SegmentId::new([9u8; 32]),
        miner: Address::new([1u8; 32]),
        checker: Address::new([2u8; 32]),
        outcome: VerdictOutcome::SlashMiner,
    };
    let mut bps = Vec::new();
    for _ in 0..3 {
        state.account_mut(&Address::new([1u8; 32])).staked = 1_000_000;
        let (_, satz) = urteil_buchen_gestaffelt(
            &mut state,
            &verdict,
            Akteur::ShardMiner,
            Grund::Nichtverfuegbarkeit,
        )
        .expect("Buchung");
        bps.push(satz.anteil_bps());
    }
    assert_eq!(bps, vec![100, 300, 500], "Kap. 5.5 nennt 1 bis 5 %, gestaffelt");
}

/// **Blöcke je Epoche, gegen Epochenlänge und Blockzeit.**
///
/// `myl_consensus::BLOECKE_JE_EPOCHE` ordnet jede Blockhöhe einer
/// Epoche zu und geht damit in die **Blockprüfung** ein. Sie steht dort
/// als Konstante und nicht als Abfrage dieser Registry, und das ist
/// Absicht: Eine Blockprüfung, die einen abstimmbaren Wert liest, macht
/// die Gültigkeit eines Blocks von einem Zustand abhängig, der sich
/// ändern kann, während der Block schon in der Kette steht.
///
/// **Damit stehen zwei Wahrheiten nebeneinander**, und dieser Test ist
/// die Verbindung: Wer `Epochenlaenge` oder `Blockzeit` bewegt, ohne die
/// Konstante nachzuziehen, bekommt hier einen Fehlschlag statt einer
/// Epoche, die eine andere Länge hat, als die Governance glaubt. Genau
/// dieselbe Bauart wie bei der Streitfrist (⚑ Fund 50).
#[test]
fn bloecke_je_epoche_stimmen_mit_epochenlaenge_und_blockzeit() {
    let reg = ParameterRegistry::vorgabe();
    let epoche_s = zahl(&reg, Parameter::Epochenlaenge);
    let blockzeit_ms = zahl(&reg, Parameter::Blockzeit);
    assert!(blockzeit_ms > 0, "eine Blockzeit von null ergäbe keine Kette");
    let gerechnet = epoche_s * 1_000 / blockzeit_ms;
    assert_eq!(
        gerechnet,
        myl_consensus::BLOECKE_JE_EPOCHE,
        "Epochenlänge {epoche_s} s bei Blockzeit {blockzeit_ms} ms sind {gerechnet} Blöcke, \
         die Konstante sagt {}",
        myl_consensus::BLOECKE_JE_EPOCHE
    );
    assert_eq!(
        epoche_s * 1_000 % blockzeit_ms,
        0,
        "die Epoche ist kein Vielfaches der Blockzeit; dann liegt die Epochengrenze \
         zwischen zwei Blöcken und die Zuordnung ist nicht mehr eindeutig"
    );
    // Und die Umrechnung trifft die Grenze auch wirklich.
    assert_eq!(myl_consensus::epoche_fuer_hoehe(0), 0);
    assert_eq!(myl_consensus::epoche_fuer_hoehe(gerechnet - 1), 0);
    assert_eq!(myl_consensus::epoche_fuer_hoehe(gerechnet), 1);
}

/// **Die Stimmgewichtsparameter, aus einer Quelle** (Phase 2).
///
/// Das Akzeptanzkriterium verlangt, dass GOVERNANCE und CONSENSUS für
/// dieselben Eingaben dasselbe Gewicht liefern. Die Formel wird
/// gerufen, nicht abgeschrieben; hier wird geprüft, dass auch ihre
/// **Parameter** aus einer Quelle kommen.
///
/// Ohne diesen Test könnten die beiden Zahlen auseinanderlaufen, ohne
/// dass ein einziger Aufruf falsch aussieht: Die Formel stimmte, die
/// Eingaben nicht.
#[test]
fn die_stimmgewichtsparameter_der_registry_sind_die_von_consensus() {
    let aus_der_registry =
        myl_governance::abstimmung::stimmgewichts_parameter(&ParameterRegistry::vorgabe());
    let aus_consensus = myl_consensus::voting_weight::StimmgewichtsParameter::default();
    assert_eq!(aus_der_registry.arbeitsbezug, aus_consensus.arbeitsbezug);
    assert_eq!(aus_der_registry.hoechstfaktor, aus_consensus.hoechstfaktor);
    assert!(aus_der_registry.ist_brauchbar());
}

/// Die Entwurfswerte der Abstimmung halten ihre eigene Untergrenze ein.
///
/// Ein Vorgabewert, den die Invariante beim ersten Vorschlag
/// zurückwiese, wäre ein Startzustand, aus dem heraus nichts geht.
#[test]
fn die_abstimmungsvorgaben_halten_ihre_eigene_untergrenze() {
    use myl_governance::abstimmung::{
        FENSTER_VORGABE, MEHRHEIT_UNTERGRENZE, MEHRHEIT_VORGABE, QUORUM_VORGABE,
    };
    // Als const-Block, dem Muster aus `myl-net/src/anfrage.rs` folgend:
    // Wer einen Vorgabewert unter seine eigene Untergrenze setzt,
    // bekommt einen Übersetzungsfehler statt eines roten Tests.
    const {
        assert!(
            MEHRHEIT_VORGABE >= MEHRHEIT_UNTERGRENZE,
            "die Vorgabe läge unter der Untergrenze, die sie selbst hält"
        )
    };
    const { assert!(MEHRHEIT_VORGABE <= 1_000) };
    const { assert!(QUORUM_VORGABE >= 1 && QUORUM_VORGABE <= 1_000) };
    const { assert!(FENSTER_VORGABE >= 1) };
    // Und der Startzustand selbst geht durch die Invariantenprüfung.
    myl_governance::invarianten::pruefe_invarianten(&ParameterRegistry::vorgabe())
        .expect("die Vorgabe verletzt ihre eigene Invariante");
}
