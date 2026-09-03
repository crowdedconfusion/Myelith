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
/// ⚑ **Das Zahlenbeispiel aus Anhang B.1 gilt nicht mehr, und das ist
/// eine Folge von A1.** Dort steht g = 0,5 MYL, p = 2 % und daraus
/// S_min = 1 250 MYL. Seit dem 2026-09-02 ist p = 5 %, weil gamma in
/// die Rate aufgegangen ist, und `S_min = g/p^2` faellt damit auf
/// **200 MYL**, also um den Faktor 6,25.
///
/// ⚑ **Der Vorgabestake ist bis zum Nachmittag desselben Tages bei
/// 1 250 geblieben** (Fund 146), und die Invariante hat es nicht
/// gemerkt: Sie prueft `S ≥ S_min`, und eine Untergrenze faengt einen
/// zu hohen Wert nicht. **Ein Mindeststake weit ueber der Anforderung
/// ist kein Sicherheitsproblem, sondern eine Eintrittshuerde, die
/// niemand beschlossen hat.**
///
/// Jetzt wird die Vorgabe **gerechnet**. Der Test haelt fest, dass sie
/// die Rechnung ist und keine abgeschriebene Zahl: Wer `p` oder `g`
/// bewegt, bewegt sie mit.
#[test]
fn der_mindeststake_ist_die_rechnung_und_keine_zahl() {
    let reg = ParameterRegistry::vorgabe();
    let (pz, pn) = bruch(&reg, Parameter::Stichprobenrate);
    let g = zahl(&reg, Parameter::Betrugsgewinn);
    let s = zahl(&reg, Parameter::MindestStake);
    assert_eq!(g, UNITS_PER_MYL / 2);
    assert_eq!((pz, pn), (5, 100), "die Rate steht seit A1 auf 5 %");

    let noetig = myl_tokenomics::s_min(g, pz, pn).unwrap();
    assert_eq!(noetig, 200 * UNITS_PER_MYL, "S_min = g/p^2 = 200 MYL");
    assert_eq!(
        s, noetig,
        "der Vorgabestake ist nicht die Rechnung; eine abgeschriebene Zahl \
         veraltet beim naechsten Mal wieder"
    );
    // ⚑ **Und die Gegenprobe zur Zusicherung selbst:** Sie ist nur
    // etwas wert, solange `s_min` ueberhaupt von `p` abhaengt. Waere
    // sie eine Konstante, sagte die Gleichheit oben nichts.
    let bei_zwei_prozent = myl_tokenomics::s_min(g, 2, 100).unwrap();
    assert_eq!(
        bei_zwei_prozent,
        1_250 * UNITS_PER_MYL,
        "die alte Zahl muss aus der alten Rate folgen, sonst stimmt die Herleitung nicht"
    );
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

/// **Die Arbeitsschwelle der Registry, gegen `myl-consensus`.**
///
/// ⚑ **Bis zum 2026-09-02 stand hier der Arbeitsbezug** (Fund 51), also
/// die vTFE-Menge, die einen Bonus in Höhe des Stakes wert war. Er ist
/// mit der Formel entfallen: Ihr unterscheidender Bereich war dreizehn
/// Prozent breit (Fund 135), und erreicht wurde sie im Betrieb nie
/// (Fund 137).
///
/// **Der Test bleibt, weil sein Grund bleibt.** Zwei Zahlen, die
/// dieselbe Sache in zwei Crates beschreiben, laufen auseinander, ohne
/// dass ein einziger Aufruf falsch aussieht.
#[test]
fn die_arbeitsschwelle_stimmt_mit_consensus() {
    let reg = ParameterRegistry::vorgabe();
    assert_eq!(
        zahl(&reg, Parameter::ArbeitsschwelleZaehler),
        myl_consensus::voting_weight::ARBEITSSCHWELLE_ZAEHLER_VORGABE
    );
    assert_eq!(
        zahl(&reg, Parameter::ArbeitsschwelleNenner),
        myl_consensus::voting_weight::ARBEITSSCHWELLE_NENNER_VORGABE
    );

    // ⚑ Und die Aussage dahinter, nicht nur die Gleichheit: Der
    // Startwert muss **null** sein. Ein Wert darüber schlösse bei
    // Genesis jeden Validator aus, denn niemand hat Arbeitshistorie.
    assert_eq!(
        zahl(&reg, Parameter::ArbeitsschwelleZaehler),
        0,
        "der Startwert der Arbeitsschwelle muss null sein, sonst faengt kein Netz an"
    );

    // Und die Kalibrierungsaussage der Nachfolgerin: Arbeit bewegt das
    // Gewicht nicht mehr, in keinem Umfang.
    let stake = 10_000_000u64;
    let mut history = myl_consensus::voting_weight::InferenceHistory::new();
    history.add_work(1, 8_900_000_000);
    let gewicht = myl_consensus::voting_weight::calculate_voting_weight(stake, &history, 1);
    assert_eq!(
        gewicht, stake,
        "Arbeit qualifiziert, sie wiegt nicht: das Gewicht muss der Stake sein"
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

// ⚑ Hier standen zwei Gleichstandstests zum Kontrollsegment-Vorrat
// (Fund 58): Die Registry führte Vorrat und Fenster, `myl-verifier` die
// Formel dazu, und beide Seiten mussten dieselbe Zahl ergeben.
//
// **Sie sind mit ihrem Gegenstand entfallen** (Entscheidung A1,
// 2026-09-02). Der Gedanke bleibt und steht weiter unten mehrfach: Zwei
// Crates, die dieselbe Zahl führen, laufen auseinander, ohne dass ein
// einziger Aufruf falsch aussieht.

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
    assert_eq!(
        aus_der_registry.schwelle_zaehler,
        aus_consensus.schwelle_zaehler
    );
    assert_eq!(aus_der_registry.schwelle_nenner, aus_consensus.schwelle_nenner);
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

/// ⚑ **Die Sperrfrist des Einsatzes deckt die Streitfrist** (Punkt B11).
///
/// Ein Einsatz, den man vor dem Urteil abziehen kann, ist keiner. Die
/// Frist muss deshalb mindestens so lang sein wie das Fenster, in dem
/// noch ein Urteil kommen kann.
///
/// **Drei Zahlen, drei Kisten, und nur hier sieht man alle:** die
/// Streitfrist als Governance-Parameter, `DEFAULT_DISPUTE_EPOCHS` in
/// `myl-consensus`, `SPERRFRIST_EPOCHEN` in `myl-ledger`. Keine der
/// drei Kisten kennt die beiden anderen; **dieser Test ist die
/// Naht.**
#[test]
fn die_sperrfrist_deckt_die_streitfrist() {
    let reg = ParameterRegistry::vorgabe();
    let frist_s = zahl(&reg, Parameter::Streitfrist);
    let epoche_s = zahl(&reg, Parameter::Epochenlaenge);
    let streit_epochen = frist_s / epoche_s;

    assert!(
        myl_ledger::einsatz::SPERRFRIST_EPOCHEN >= streit_epochen,
        "Sperrfrist {} Epochen deckt die Streitfrist von {streit_epochen} nicht; \
         wer kuendigt, waere vor dem Urteil draussen",
        myl_ledger::einsatz::SPERRFRIST_EPOCHEN
    );
    // ⚑ **Und nicht laenger als noetig**, sonst waere sie eine Haerte
    // ohne Begruendung. Gleichheit ist die Aussage; ein Abstand muesste
    // begruendet werden und stuende dann hier.
    assert_eq!(
        myl_ledger::einsatz::SPERRFRIST_EPOCHEN,
        streit_epochen,
        "Sperrfrist und Streitfrist laufen auseinander"
    );
    // Die dritte Zahl derselben Aussage.
    assert_eq!(
        streit_epochen,
        myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS
    );
}
