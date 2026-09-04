//! Woher jeder Vorgabewert kommt, und wie viele noch nicht hergeleitet
//! sind (Punkt B5, 2026-09-02).
//!
//! # ⚑ Warum diese Datei entstanden ist
//!
//! **Fund 146:** `MindestStake` stand auf 1 250 MYL. Das war `g/p²` bei
//! `p = 2 %`. Am 2026-09-02 stieg `p` auf 5 %, `S_min` fiel auf 200, und
//! die Zahl blieb stehen. **Die Invariante hat es nicht gemerkt, und
//! zwar zu Recht:** Sie prüft `S ≥ S_min`, und eine Untergrenze fängt
//! einen zu hohen Wert nicht.
//!
//! ⚑ **Der Schaden war nicht die Hürde, sondern der unsichtbare
//! Spielraum.** Bei einem Stake auf der Schranke bricht jede Erhöhung
//! des Betrugsgewinns die Invariante; beim 6,25-Fachen durfte er sich
//! verdreifachen, ohne dass etwas geschah. **Eine Prüfung, deren
//! Reserve niemand kennt, prüft weniger, als sie zu prüfen scheint.**
//!
//! # Was diese Datei leistet, und was nicht
//!
//! **Sie zwingt jeden Parameter, seine Herkunft zu nennen.** Wer einen
//! hinzufügt, muss sagen, ob sein Wert gerechnet, entschieden, ein
//! Startwert oder ein Entwurf ist; die Vollständigkeitsprüfung meldet
//! sonst.
//!
//! **Sie zählt die Entwürfe.** Das ist der Stand der
//! Parameter-Kalibrierung in einer Zahl: So viele Werte tragen noch
//! keine Herleitung und keine Entscheidung, sondern nur einen
//! Vorschlag.
//!
//! ⚑ **Sie prüft keine Wirtschaftlichkeit.** Ob 0,7 der richtige
//! Kostenanteil ist, sagt keine Datei; sie sagt nur, dass es ein
//! Entwurf ist und niemand ihn beschlossen hat.

use myl_governance::registry::{Parameter, ParameterRegistry, Wert};
use myl_tokenomics::UNITS_PER_MYL;

/// Woher ein Vorgabewert kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Herkunft {
    /// Aus einer Formel dieses Projekts gerechnet. **Ändert sich mit
    /// seinen Eingaben**, kann also nicht veralten.
    Gerechnet,
    /// Eine Festlegung mit Fundstelle: Whitepaper oder eine
    /// Design-Entscheidung mit Datum.
    Entschieden,
    /// Ein Anfangszustand, kein Parameter im eigentlichen Sinn.
    Startwert,
    /// ⚑ **Ein Vorschlag, den niemand beschlossen hat.** Er steht da,
    /// weil ein Wert dastehen muss.
    Entwurf,
}

/// Herkunft und Fundstelle jedes Parameters.
///
/// ⚑ **Von Hand gepflegt, und deshalb mit Vollständigkeitsprüfung.**
/// Eine solche Liste hört leise auf, vollständig zu sein; dieselbe
/// Lehre wie bei `CONSENSUS_PATH` in der Gleitkommaprüfung.
fn herkunft(p: Parameter) -> (Herkunft, &'static str) {
    use Herkunft::*;
    use Parameter::*;
    match p {
        // --- gerechnet ---
        MindestStake => (Gerechnet, "S_min = g/p^2, myl_tokenomics::s_min"),
        // --- entschieden, mit Herleitung und Anker ---
        Speichersatz => (
            Entschieden,
            "Punkt B4 vom 2026-09-02: 9 000 je Byte-Epoche, also 1,49 je TB-Monat \
             und damit die Ankerrate eines vergleichbaren Netzes",
        ),
        // --- entschieden, mit Fundstelle ---
        Stichprobenrate => (Entschieden, "A1 vom 2026-09-02: gamma/(1-c) + p*(1-gamma), aufgerundet"),
        Subventionsrate => (Entschieden, "Kap. 5.7 und Anhang B.4: s = 0,5"),
        Auslastungsziel => (Entschieden, "Kap. 5.4: u* = 0,7"),
        PreisSensitivitaet => (Entschieden, "Kap. 5.4: kappa = 0,1"),
        TrainingsverguetungsAnteil => (Entschieden, "Kap. 5.6: hoechstens 70 Prozent"),
        EmaGlaettung => (Entschieden, "myl_tokenomics: 30-Epochen-Fenster"),
        Redundanzfaktor => (Entschieden, "Kap. 4.4: r = 2"),
        Shardzahl => (Entschieden, "Kap. 4.1: k = 8"),
        Komiteegroesse => (Entschieden, "Design-Entscheidung 2026-08-13: 21"),
        Blockzeit => (Entschieden, "Design-Entscheidung 2026-08-13: 2 s"),
        Streitfrist => (Entschieden, "Design-Entscheidung 2026-08-13: 7 Tage"),
        Epochenlaenge => (Entschieden, "Anhang B.1: eine Stunde"),
        Kopfgeldanteil => (Entschieden, "Anhang B.3: b = 30 Prozent"),
        Betrugsgewinn => (Entschieden, "Anhang B.1: g = 0,5 MYL"),
        ArbeitsschwelleZaehler => (Entschieden, "Entscheidung A3 vom 2026-09-02: Startwert null"),
        ArbeitsschwelleNenner => (Entschieden, "Entscheidung A3 vom 2026-09-02"),
        Signaturstufe => (Entschieden, "es gibt nur BLS12-381; der Schalter braucht eine Stellung"),
        GesamtangebotFestgelegtDurchBurnAndMint => (Entschieden, "Kap. 10.3, Verfassungsrang"),
        BurnAndMintPrinzip => (Entschieden, "Kap. 10.3, Verfassungsrang"),
        DeterminismusPflicht => (Entschieden, "Kap. 10.3, Verfassungsrang"),
        // --- Startwerte ---
        GeglaetteterBurn => (Startwert, "noch kein Burn beobachtet"),
        KernelWhitelist => (Startwert, "leer bis zum Genesis-Manifest, GOVERNANCE 3.4"),
        // --- Entwuerfe: hier liegt die Arbeit ---
        Kostenanteil => (Entwurf, "Anhang B.4 nennt empirisch 0,6 bis 0,8; 0,7 ist gegriffen"),
        TrainingsStichprobenrate => (Entwurf, "Kap. 5.5 nennt eine erhoehte Rate, nicht welche"),
        PraegeObergrenze => (Entwurf, "Anhang B.8.3 laesst sie offen; u64::MAX heisst kein Deckel"),
        PreisUntergrenze => (Entwurf, "ein Kleinstbetrag, damit sie nicht null ist"),
        Abstimmungsquorum => (Entwurf, "Kap. 10.2 legt das Stimmgewicht fest, nicht das Verfahren"),
        Abstimmungsmehrheit => (Entwurf, "wie Abstimmungsquorum"),
        Abstimmungsfenster => (Entwurf, "wie Abstimmungsquorum"),
    }
}

/// ⚑ **Jeder Parameter nennt seine Herkunft.**
///
/// Die Vollstaendigkeitspruefung, ohne die die Liste oben leise
/// veraltet. `Parameter::alle()` ist die Quelle; wer einen Parameter
/// hinzufuegt und hier nichts eintraegt, bekommt einen Compilerfehler,
/// weil `match` vollstaendig sein muss. **Dieser Test faengt den
/// anderen Fall:** einen Eintrag, der auf keinen Parameter mehr passt.
#[test]
fn jeder_parameter_nennt_seine_herkunft() {
    for p in Parameter::alle() {
        let (_, quelle) = herkunft(p);
        assert!(
            !quelle.is_empty(),
            "{p:?} nennt keine Quelle",
        );
    }
    assert_eq!(Parameter::alle().len(), 31, "die Zahl der Parameter hat sich geaendert");
}

/// ⚑ **Was gerechnet ist, muss die Rechnung sein** (Fund 146).
///
/// Der Test rechnet jede als `Gerechnet` gefuehrte Vorgabe nach. Eine
/// abgeschriebene Zahl faellt hier auf, und zwar an dem Tag, an dem ihre
/// Eingabe sich aendert, nicht Monate spaeter.
#[test]
fn was_gerechnet_ist_stimmt_mit_seiner_rechnung() {
    let reg = ParameterRegistry::vorgabe();
    let mut gerechnet = 0;
    for p in Parameter::alle() {
        if herkunft(p).0 != Herkunft::Gerechnet {
            continue;
        }
        gerechnet += 1;
        match p {
            Parameter::MindestStake => {
                let Wert::Bruch { zaehler: pz, nenner: pn } = *reg.wert(Parameter::Stichprobenrate)
                else {
                    panic!("p ist ein Bruch");
                };
                let Wert::Ganzzahl(g) = *reg.wert(Parameter::Betrugsgewinn) else {
                    panic!("g ist eine Ganzzahl");
                };
                let Wert::Ganzzahl(s) = *reg.wert(Parameter::MindestStake) else {
                    panic!("S ist eine Ganzzahl");
                };
                assert_eq!(
                    s,
                    myl_tokenomics::s_min(g, pz, pn).expect("rechenbar"),
                    "MindestStake ist nicht g/p^2"
                );
                assert_eq!(s, 200 * UNITS_PER_MYL, "bei p = 5 Prozent sind es 200 MYL");
            }
            andere => panic!("{andere:?} ist als gerechnet gefuehrt, wird hier aber nicht gerechnet"),
        }
    }
    assert_eq!(gerechnet, 1, "die Zahl der gerechneten Vorgaben hat sich geaendert");
}

/// ⚑ **Wie viele Zahlen noch niemand beschlossen hat.**
///
/// Das ist der Stand der Parameter-Kalibrierung in einer Zahl. Sie darf
/// fallen; steigt sie, ist ein Wert ohne Herleitung hinzugekommen, und
/// **das gehoert gesehen und nicht gefunden**.
///
/// ⚑ **Die Zahl ist eine Sperrklinke, keine Zusicherung ueber die
/// Werte.** Ob 0,7 der richtige Kostenanteil ist, sagt dieser Test
/// nicht; er sagt, dass es noch ein Entwurf ist.
#[test]
fn die_entwuerfe_sind_gezaehlt_und_werden_nicht_mehr() {
    let entwuerfe: Vec<Parameter> = Parameter::alle()
        .into_iter()
        .filter(|p| herkunft(*p).0 == Herkunft::Entwurf)
        .collect();
    assert!(
        entwuerfe.len() <= 7,
        "es sind {} Entwuerfe statt hoechstens sieben: {entwuerfe:?}",
        entwuerfe.len()
    );
    // Und die Gegenprobe zur Sperrklinke: Sie ist nur etwas wert,
    // solange ueberhaupt welche offen sind. Bei null gehoert sie
    // entfernt, nicht auf null gesetzt.
    assert!(
        !entwuerfe.is_empty(),
        "keine Entwuerfe mehr; dieser Test und die Herkunftsliste gehoeren dann durchgesehen"
    );
}

/// ⚑ **Der Betrugsgewinn ist eine feste Zahl fuer eine bewegliche
/// Groesse**, und das gehoert festgehalten.
///
/// `g` ist der Gewinn aus einem betrogenen Segment. Was ein Segment
/// einbringt, haengt am **Credit-Preis**, und der ist ausdruecklich
/// dynamisch (`P_{e+1} = P_e * exp(kappa*(u_e - u*))`, Kap. 5.4).
/// **Steigt der Preis, steigt der Betrugsgewinn, und `S_min = g/p²`
/// steigt quadratisch mit.** Ein festes `g` heisst also: Die
/// Sicherheitsschranke folgt der Wirtschaft nicht.
///
/// Der Test aendert daran nichts. Er haelt fest, dass `g` eine
/// **Ganzzahl** ist und keine Ableitung, damit die Frage sichtbar
/// bleibt, bis sie entschieden ist.
#[test]
fn der_betrugsgewinn_ist_fest_und_die_groesse_dahinter_ist_es_nicht() {
    let reg = ParameterRegistry::vorgabe();
    let Wert::Ganzzahl(g) = *reg.wert(Parameter::Betrugsgewinn) else {
        panic!("g ist eine Ganzzahl");
    };
    assert_eq!(g, UNITS_PER_MYL / 2, "Anhang B.1: g = 0,5 MYL");
    assert_eq!(
        herkunft(Parameter::Betrugsgewinn).0,
        Herkunft::Entschieden,
        "wenn g hergeleitet wird, gehoert es in die gerechneten"
    );
}
