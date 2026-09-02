//! Die Akzeptanzkriterien von GOVERNANCE Phase 1, wörtlich.
//!
//! > „Versuch, das Gesamtangebot oder das Burn-and-Mint-Prinzip per
//! > Vorschlag zu ändern, scheitert auf Protokollebene (Test); ein
//! > Parametervorschlag, der S_min unterschreitet oder die
//! > Self-Dealing-Invariante verletzt, wird automatisch abgelehnt."
//!
//! Beide Sätze stehen unten als Test, und die Gegenprobe steht davor:
//! Eine Prüfung, die alles ablehnt, erfüllt jedes Ablehnungskriterium und
//! ist trotzdem wertlos.

use myl_governance::registry::{Aenderbarkeit, Parameter, ParameterRegistry, Wert};
use myl_governance::{pruefe_vorschlag, Invariante, ParameterVorschlag, VorschlagFehler};

fn bruch(z: u64, n: u64) -> Wert {
    Wert::Bruch { zaehler: z, nenner: n }
}

fn vorschlag(p: Parameter, w: Wert) -> ParameterVorschlag {
    ParameterVorschlag { parameter: p, neuer_wert: w }
}

// ---------------------------------------------------------------------
// Gegenproben
// ---------------------------------------------------------------------

/// **Gegenprobe 1: Der Vorgabesatz erfüllt alle Invarianten.**
///
/// Wäre er es nicht, stünde das Protokoll schon vor der ersten
/// Abstimmung außerhalb seiner eigenen Bedingungen, und jeder
/// Ablehnungstest darunter hätte den falschen Grund.
#[test]
fn der_vorgabesatz_ist_in_sich_stimmig() {
    let reg = ParameterRegistry::vorgabe();
    assert!(
        myl_governance::pruefe_invarianten(&reg).is_ok(),
        "die Vorgabewerte verletzen ihre eigenen Bedingungen: {:?}",
        myl_governance::pruefe_invarianten(&reg)
    );
}

/// **Gegenprobe 2: Ein sinnvoller Vorschlag geht durch.**
///
/// Die Stichprobenrate von 2 auf 5 Prozent zu heben ist genau der Zug,
/// den Kap. 5.7 für die Anlaufphase vorsieht. Er senkt `S_min` und darf
/// an keiner Bedingung scheitern.
#[test]
fn ein_sinnvoller_vorschlag_geht_durch() {
    let reg = ParameterRegistry::vorgabe();
    let danach = pruefe_vorschlag(&reg, &vorschlag(Parameter::Stichprobenrate, bruch(5, 100)))
        .expect("eine höhere Prüfrate muss zulässig sein");
    assert_eq!(danach.wert(Parameter::Stichprobenrate), &bruch(5, 100));
    // Und der übrige Satz ist unverändert.
    assert_eq!(
        danach.wert(Parameter::Subventionsrate),
        reg.wert(Parameter::Subventionsrate)
    );
}

/// **Gegenprobe 3: Jeder Parameter hat einen Wert.**
///
/// `ParameterRegistry::wert` verlässt sich darauf. Käme ein Parameter
/// hinzu, ohne in `vorgabe()` gesetzt zu werden, würde jede Prüfung in
/// eine Panik laufen statt in eine Ablehnung.
#[test]
fn jeder_parameter_hat_einen_wert() {
    let reg = ParameterRegistry::vorgabe();
    for p in Parameter::alle() {
        let _ = reg.wert(p); // paniert, falls er fehlt
    }
    // ⚑ Einunddreissig seit dem 2026-09-02, in zwei Schritten desselben
    // Tages: Die drei Kontrollsegment-Parameter sind mit ihrem
    // Gegenstand entfallen (Entscheidung A1), und der **Speichersatz**
    // ist dazugekommen (Punkt B4). Er stand vorher nirgends, und genau
    // das war die Frage: Eine Zahl, die kein Parameter ist, laesst sich
    // nach dem Genesis nicht mehr aendern.
    assert_eq!(Parameter::alle().len(), 31);
}

// ---------------------------------------------------------------------
// Akzeptanzkriterium 1: Verfassungsrang (Punkt 1.2)
// ---------------------------------------------------------------------

/// **„Versuch, das Gesamtangebot oder das Burn-and-Mint-Prinzip per
/// Vorschlag zu ändern, scheitert auf Protokollebene."**
///
/// Geprüft für alle drei Verfassungsrang-Festlegungen aus Kap. 10.3, in
/// beide Richtungen: abschalten und erneut einschalten. Auch das
/// Einschalten muss scheitern — sonst wäre der Rang keine Schranke,
/// sondern eine Vorzugsrichtung.
#[test]
fn verfassungsrang_scheitert_auf_protokollebene() {
    let reg = ParameterRegistry::vorgabe();
    let verfassung = [
        Parameter::GesamtangebotFestgelegtDurchBurnAndMint,
        Parameter::BurnAndMintPrinzip,
        Parameter::DeterminismusPflicht,
    ];
    for p in verfassung {
        assert_eq!(p.rang(), Aenderbarkeit::Verfassungsrang);
        for wert in [Wert::Schalter(false), Wert::Schalter(true)] {
            assert_eq!(
                pruefe_vorschlag(&reg, &vorschlag(p, wert)),
                Err(VorschlagFehler::Verfassungsrang { parameter: p }),
                "{} muss auf Protokollebene scheitern",
                p.name()
            );
        }
    }
}

/// Der Rang hängt am Typ, nicht an einem Datensatz.
///
/// Stünde er als Feld in der Registry, wäre er selbst ein Parameter und
/// mit einem Vorschlag änderbar: Erst den Rang senken, dann den Wert
/// ändern. Der Test hält fest, dass es diesen Weg nicht gibt, indem er
/// zeigt, dass **kein** Parameter der Registry den Rang eines anderen
/// beschreibt.
#[test]
fn der_rang_ist_selbst_kein_parameter() {
    for p in Parameter::alle() {
        let name = p.name();
        assert!(
            !name.to_lowercase().contains("rang"),
            "{name} sieht aus, als beschriebe es einen Rang"
        );
    }
    // Und die Menge der Verfassungsrang-Parameter ist genau die aus
    // Kap. 10.3, nicht mehr und nicht weniger.
    let anzahl = Parameter::alle()
        .iter()
        .filter(|p| p.rang() == Aenderbarkeit::Verfassungsrang)
        .count();
    assert_eq!(anzahl, 3, "Kap. 10.3 nennt genau drei");
}

/// Ein Vorschlag mit falscher Art ist keine Parameteränderung.
#[test]
fn eine_andere_art_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();
    let f = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::Stichprobenrate, Wert::Ganzzahl(1)),
    );
    assert!(matches!(f, Err(VorschlagFehler::Art(_))));
}

// ---------------------------------------------------------------------
// Akzeptanzkriterium 2: Invarianten (Punkt 1.3)
// ---------------------------------------------------------------------

/// **„Ein Parametervorschlag, der S_min unterschreitet, wird automatisch
/// abgelehnt."**
///
/// Drei Wege führen unter die Schranke, und alle drei müssen scheitern:
/// die Prüfrate senken, den Betrugsgewinn heben, den Stake senken. Der
/// erste ist der gefährlichste, weil er wie eine Kostensenkung aussieht:
/// Weniger prüfen spart Kapazität, und dass `S_min` **quadratisch**
/// steigt, sieht man der Zahl nicht an.
#[test]
fn ein_vorschlag_unter_s_min_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();

    // Weg 1: Pruefrate von 5 % auf 1 % senken. S_min steigt um das
    // Fuenfundzwanzigfache.
    //
    // ⚑ Die Rate steht seit dem 2026-09-02 auf 5 % statt 2 %, weil
    // gamma in sie aufgegangen ist. **Das senkt S_min = g/p^2 um den
    // Faktor 6,25**, von 1 250 auf 200 MYL, und seit Fund 146 faellt
    // der Vorgabestake mit: Er wird gerechnet und nicht geschrieben.
    let f = pruefe_vorschlag(&reg, &vorschlag(Parameter::Stichprobenrate, bruch(1, 100)));
    match f {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::MindestStake);
        }
        andere => panic!("erwartet war eine S_min-Verletzung, bekommen: {andere:?}"),
    }

    // Weg 2: den Betrugsgewinn heben, ohne den Stake anzuheben.
    //
    // ⚑ **Seit Fund 146 genuegt der kleinste Schritt.** Der
    // Vorgabestake ist jetzt **genau** S_min, also sitzt der
    // Parametersatz auf der Schranke: Jede Erhoehung von g bricht sie.
    // Vorher lag der Stake um den Faktor 6,25 darueber, und g durfte
    // sich verdreifachen, ohne dass etwas geschah. **Das war die
    // eigentliche Gefahr der veralteten Zahl**, nicht die Huerde
    // selbst: Sie liess drei Viertel des Spielraums unbemerkt.
    let g_jetzt = reg
        .wert(Parameter::Betrugsgewinn)
        .als_ganzzahl()
        .expect("Ganzzahl");
    let f = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::Betrugsgewinn, Wert::Ganzzahl(g_jetzt + 1)),
    );
    assert!(
        matches!(f, Err(VorschlagFehler::Invariante(_))),
        "ein groesserer Betrugsgewinn muss die Schranke brechen"
    );

    // Und die Gegenprobe: kleiner geht durch, sonst prueft der Test
    // nur, dass irgendein Vorschlag zu diesem Parameter abgelehnt wird.
    assert!(
        pruefe_vorschlag(
            &reg,
            &vorschlag(Parameter::Betrugsgewinn, Wert::Ganzzahl(g_jetzt / 2)),
        )
        .is_ok(),
        "ein kleinerer Betrugsgewinn senkt S_min und muss durchgehen"
    );

    // Weg 3: den Stake unter S_min senken.
    //
    // ⚑ **Seit Fund 146 ist die Vorgabe genau S_min**, nicht mehr das
    // 6,25-Fache. 1250 MYL war `g/p²` bei `p = 2 %`; am 2026-09-02
    // stieg p auf 5 %, und die Zahl blieb stehen. **Die Invariante hat
    // das nicht gemerkt, und zwar zu Recht:** Sie prueft `S ≥ S_min`,
    // und eine Untergrenze faengt einen zu hohen Wert nicht.
    let jetzt = reg
        .wert(Parameter::MindestStake)
        .als_ganzzahl()
        .expect("Ganzzahl");
    let g = reg
        .wert(Parameter::Betrugsgewinn)
        .als_ganzzahl()
        .expect("Ganzzahl");
    let (pz, pn) = reg
        .wert(Parameter::Stichprobenrate)
        .als_bruch()
        .expect("Bruch");
    // S_min = g/p^2 = g * n^2 / z^2, ganzzahlig aufgerundet.
    let s_min = (g as u128 * pn as u128 * pn as u128).div_ceil(pz as u128 * pz as u128) as u64;

    assert_eq!(
        jetzt, s_min,
        "die Vorgabe muss die Rechnung sein, nicht eine abgeschriebene Zahl"
    );

    // Genau S_min ist zulaessig, einer darunter nicht. Das ist die
    // Schranke selbst, unabhaengig davon, wo der Vorgabewert steht.
    assert!(
        pruefe_vorschlag(&reg, &vorschlag(Parameter::MindestStake, Wert::Ganzzahl(s_min))).is_ok(),
        "genau S_min muss zulaessig sein"
    );

    let f = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::MindestStake, Wert::Ganzzahl(s_min - 1)),
    );
    assert!(
        matches!(f, Err(VorschlagFehler::Invariante(_))),
        "einer unter S_min darf nicht durchgehen"
    );
}

/// **„… oder die Self-Dealing-Invariante verletzt."**
///
/// Anhang B.4: `s < c/(1−c)`, geprüft gegen das **untere** Ende des
/// Bandes (c = 0,6), also `s < 1,5`. Siehe Fund 49.
#[test]
fn ein_vorschlag_ueber_der_self_dealing_grenze_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();

    // Knapp darunter: zulässig.
    assert!(
        pruefe_vorschlag(&reg, &vorschlag(Parameter::Subventionsrate, bruch(149, 100))).is_ok(),
        "s = 1,49 liegt unter 1,5 und muss zulässig sein"
    );

    // Genau darauf: die Bedingung ist strikt („s < ..."), also abgelehnt.
    let f = pruefe_vorschlag(&reg, &vorschlag(Parameter::Subventionsrate, bruch(150, 100)));
    match f {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::SelfDealing)
        }
        andere => panic!("s = c/(1−c) muss scheitern, bekommen: {andere:?}"),
    }

    // Darüber: erst recht. 2,33 galt vor Fund 49 noch, jetzt nicht mehr.
    for (z, n) in [(233u64, 100u64), (7, 3), (3, 1)] {
        assert!(
            matches!(
                pruefe_vorschlag(&reg, &vorschlag(Parameter::Subventionsrate, bruch(z, n))),
                Err(VorschlagFehler::Invariante(_))
            ),
            "s = {z}/{n} muss abgelehnt werden"
        );
    }
}

/// **Fund 49 ist geschlossen: Kein `c` bewegt die Self-Dealing-Grenze.**
///
/// Bis zum 2026-08-24 hielt dieser Test die **Lücke** fest: Erst `c`
/// heben, dann `s` unter die neue Grenze, beide Schritte zulässig, das
/// Ergebnis nicht. Seine Doku sagte: „Schlägt dieser Test eines Tages
/// fehl, hat jemand die Lücke geschlossen." Genau das ist passiert.
///
/// Jetzt hält er die Gegenrichtung fest, und das ist der wichtigere Teil:
/// **`Parameter::Kostenanteil` steht weiter in der Registry, hat aber
/// keine Wirkung mehr auf die Prüfung.** Ein Parameter ohne Wirkung ist
/// das Muster, an dem sich das Projekt dreimal verbrannt hat (A7,
/// Fund 25, Fund 44). Deshalb steht die Wirkungslosigkeit hier als Test
/// und nicht nur als Satz: Wer die Prüfung wieder an `c` koppelt, bricht
/// ihn.
#[test]
fn kein_kostenanteil_bewegt_die_self_dealing_grenze() {
    let basis = ParameterRegistry::vorgabe();

    // s = 2 ist unzulässig (Grenze 1,5) — und bleibt es für jedes c.
    for cz in 1..=8u64 {
        let nach_c = match pruefe_vorschlag(
            &basis,
            &vorschlag(Parameter::Kostenanteil, bruch(cz, 10)),
        ) {
            Ok(r) => r,
            Err(_) => continue, // c außerhalb der Plausibilitätsgrenze
        };
        for (sz, sn, soll_gelten) in [(149u64, 100u64, true), (150, 100, false), (2, 1, false), (3, 1, false)] {
            let ergebnis = pruefe_vorschlag(&nach_c, &vorschlag(Parameter::Subventionsrate, bruch(sz, sn)));
            assert_eq!(
                ergebnis.is_ok(),
                soll_gelten,
                "c = {cz}/10 hat das Urteil über s = {sz}/{sn} verändert"
            );
        }
    }

    // Die Plausibilitätsgrenze für c gilt weiterhin, sie schützt jetzt
    // aber nichts mehr, sondern hält den Wert nur in seinem Band.
    assert!(matches!(
        pruefe_vorschlag(&basis, &vorschlag(Parameter::Kostenanteil, bruch(9, 10))),
        Err(VorschlagFehler::Invariante(_))
    ));
}

/// **Fund 47 an seinem Platz:** α > 1 kommt gar nicht erst durch.
#[test]
fn ein_alpha_ueber_eins_kommt_nicht_durch() {
    let reg = ParameterRegistry::vorgabe();
    for (z, n) in [(3u64, 1u64), (32, 31), (1, 0), (0, 31)] {
        assert!(
            matches!(
                pruefe_vorschlag(&reg, &vorschlag(Parameter::EmaGlaettung, bruch(z, n))),
                Err(VorschlagFehler::Invariante(_))
            ),
            "alpha = {z}/{n} muss abgelehnt werden"
        );
    }
    // Und ein zulässiges alpha geht durch.
    assert!(pruefe_vorschlag(&reg, &vorschlag(Parameter::EmaGlaettung, bruch(1, 10))).is_ok());
}

/// Die übrigen strukturellen Schranken, je mit ihrem Grund.
#[test]
fn die_strukturellen_schranken_greifen() {
    let reg = ParameterRegistry::vorgabe();
    let faelle: Vec<(Parameter, Wert, Invariante)> = vec![
        (Parameter::Stichprobenrate, bruch(1, 0), Invariante::NennerNichtNull),
        (Parameter::Auslastungsziel, bruch(11, 10), Invariante::RateInEinheitsintervall),
        (Parameter::Redundanzfaktor, Wert::Ganzzahl(1), Invariante::RedundanzMindestensZwei),
        (Parameter::Komiteegroesse, Wert::Ganzzahl(3), Invariante::KomiteeGrossGenug),
        (Parameter::Shardzahl, Wert::Ganzzahl(0), Invariante::ShardzahlMindestensEins),
        (
            Parameter::TrainingsverguetungsAnteil,
            bruch(10_000, 10_000),
            Invariante::TrainingsverguetungUnterInferenz,
        ),
        (Parameter::Kostenanteil, bruch(0, 10), Invariante::KostenanteilPlausibel),
    ];
    for (p, w, erwartet) in faelle {
        match pruefe_vorschlag(&reg, &vorschlag(p, w.clone())) {
            Err(VorschlagFehler::Invariante(b)) => assert_eq!(
                b.invariante, erwartet,
                "{}: falsche Invariante gemeldet",
                p.name()
            ),
            andere => panic!("{} mit {:?} muss scheitern, bekommen: {:?}", p.name(), w, andere),
        }
    }
}

/// Die Ablehnung selbst muss deterministisch sein.
///
/// Zwei Knoten, die denselben Vorschlag prüfen, müssen **dieselbe**
/// Begründung nennen. Unterschieden sie sich, wäre die Ablehnung ein
/// Konsensbruch statt einer Regel.
#[test]
fn die_ablehnung_ist_deterministisch() {
    let reg = ParameterRegistry::vorgabe();
    // Ein Vorschlag, der gleich mehrere Bedingungen verletzt.
    let v = vorschlag(Parameter::Stichprobenrate, bruch(0, 100));
    let erste = pruefe_vorschlag(&reg, &v);
    for _ in 0..100 {
        assert_eq!(pruefe_vorschlag(&reg, &v), erste);
    }
}

/// Ein abgelehnter Vorschlag lässt die Registry unberührt.
#[test]
fn ein_abgelehnter_vorschlag_aendert_nichts() {
    let reg = ParameterRegistry::vorgabe();
    let vorher = reg.clone();
    let _ = pruefe_vorschlag(&reg, &vorschlag(Parameter::Subventionsrate, bruch(9, 1)));
    let _ = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::BurnAndMintPrinzip, Wert::Schalter(false)),
    );
    assert_eq!(reg, vorher);
}

// ---------------------------------------------------------------------
// Die Eigenschaft, auf die alles hinausläuft
// ---------------------------------------------------------------------

/// xorshift64, reproduzierbar und ohne Abhängigkeit.
struct Wuerfel(u64);
impl Wuerfel {
    fn neu(keim: u64) -> Self {
        Self(keim | 1)
    }
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.naechste() % n
    }
}

/// **Kein angenommener Vorschlag führt je zu einem Zustand, der eine
/// Invariante verletzt.**
///
/// Das ist die eine Eigenschaft, auf die dieses Crate hinausläuft. Alle
/// Einzeltests darüber prüfen bekannte Wege; dieser prüft, dass es keinen
/// unbekannten gibt.
///
/// Gezogen wird über **alle** Parameter, mit Werten von 0 bis an die
/// Ränder des Zahlbereichs, denn das ist der Bereich, den eine Abstimmung
/// erreichen kann.
#[test]
fn kein_angenommener_vorschlag_verletzt_je_eine_invariante() {
    let reg = ParameterRegistry::vorgabe();
    let alle = Parameter::alle();
    let mut w = Wuerfel::neu(0x60F);
    let mut angenommen = 0usize;

    for _ in 0..50_000 {
        let p = alle[(w.bis(alle.len() as u64)) as usize];
        let wert = match w.bis(6) {
            0 => Wert::Ganzzahl(0),
            1 => Wert::Ganzzahl(1),
            2 => Wert::Ganzzahl(u64::MAX),
            3 => Wert::Ganzzahl(w.naechste()),
            4 => bruch(w.bis(50), w.bis(50)),
            _ => bruch(w.naechste(), w.naechste()),
        };
        if let Ok(danach) = pruefe_vorschlag(&reg, &ParameterVorschlag {
            parameter: p,
            neuer_wert: wert.clone(),
        }) {
            angenommen += 1;
            assert!(
                myl_governance::pruefe_invarianten(&danach).is_ok(),
                "{} auf {:?} wurde angenommen, verletzt aber eine Invariante",
                p.name(),
                wert
            );
            // Und der Verfassungsrang ist nie darunter.
            assert_eq!(p.rang(), Aenderbarkeit::Aenderbar);
        }
    }
    // **Die Gegenprobe**: Würde nichts angenommen, wäre die Zusicherung
    // darüber leer. Genau diese Falle machte den ersten Pod-Fuzzer
    // wertlos.
    eprintln!("  von 50.000 zufälligen Vorschlägen wurden {angenommen} angenommen");
    assert!(
        angenommen > 500,
        "nur {angenommen} von 50.000 kamen durch; die Zusicherung darüber \
         prüft dann fast nichts"
    );
}

// ---------------------------------------------------------------------
// ⚑ Hier standen drei Tests zu Fund 58: der Kontrollsegment-Vorrat
// ---------------------------------------------------------------------
//
// Sie prüften, dass ein Vorrat unter der Schranke abgelehnt wird, dass
// die Schranke genau dort liegt, wo die Rechnung sie erwartet, und dass
// der gefährlichste Zug auffliegt: γ heben, ohne den Vorrat mitzuziehen.
// Der sah aus, als schärfe er die Kontrolle, und schaltete sie ab.
//
// **Sie sind mit ihrem Gegenstand entfallen** (Entscheidung A1,
// 2026-09-02): Die Kontrollsegmente wurden abgeschafft, weil ihre
// Einschleusung einen Einspeiser braucht, den es nicht geben kann, und
// weil die Stichprobe dieselbe Aufgabe ohne Vorrat löst. γ ist in die
// Stichprobenrate aufgegangen, hochgerechnet auf 5 %.
//
// ⚑ **Was hier nicht mit entfällt, ist die Bauart, die diese drei Tests
// vorgeführt haben:** Eine Invariante, die zwei Parameter koppelt, fängt
// den Zug, der einen davon allein bewegt. Dieselbe Bauart prüft
// `die_trainingsrate_liegt_nie_unter_der_inferenzrate` weiter oben, und
// die Ablehnungsmeldung dazu steht in `examples/schranke_meldung.rs`.



// ---------------------------------------------------------------------
// Die Invarianten aus den Entscheidungen vom 2026-08-24
// ---------------------------------------------------------------------

/// **A2: Ein Prägedeckel unter dem geglätteten Burn wird abgelehnt.**
///
/// Kap. 5.7 sagt, ein Deckel sei „nicht vorgesehen", und B.8.3 begründet
/// es: Ein **bindender** Deckel bringt den Umlauf zum Erliegen. Das
/// Argument richtet sich gegen einen bindenden Deckel im Normalbetrieb,
/// nicht gegen den Mechanismus; als Notbremse gegen einen Fehler in der
/// EMA ist er sinnvoll. Diese Invariante trennt die beiden Fälle.
#[test]
fn ein_bindender_praegedeckel_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();
    // Erst einen Burn setzen, dann den Deckel darunter versuchen.
    let mit_burn = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::GeglaetteterBurn, Wert::Ganzzahl(1_000_000)),
    )
    .expect("ein Burn-Wert ist zulässig");

    match pruefe_vorschlag(
        &mit_burn,
        &vorschlag(Parameter::PraegeObergrenze, Wert::Ganzzahl(999_999)),
    ) {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::PraegedeckelNichtBindend)
        }
        andere => panic!("ein bindender Deckel muss scheitern, bekommen: {andere:?}"),
    }

    // Genau auf dem Burn ist er noch zulässig: eine Obergrenze, keine Bremse.
    assert!(pruefe_vorschlag(
        &mit_burn,
        &vorschlag(Parameter::PraegeObergrenze, Wert::Ganzzahl(1_000_000))
    )
    .is_ok());
}

/// **D2: Eine Trainingsrate unter der Inferenzrate wird abgelehnt.**
///
/// Sonst kehrt sich die Begründung aus Kap. 5.5 um: Der größere Schaden
/// wäre schlechter geschützt als der kleinere.
#[test]
fn eine_trainingsrate_unter_der_inferenzrate_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();
    match pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::TrainingsStichprobenrate, bruch(1, 100)),
    ) {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::TrainingsrateNichtUnterInferenzrate)
        }
        andere => panic!("1 % unter 5 % muss scheitern, bekommen: {andere:?}"),
    }
    // Gleichstand ist zulässig, darüber erst recht. ⚑ Die Inferenzrate
    // steht seit dem 2026-09-02 auf 5 %, nicht mehr auf 2 %.
    assert!(pruefe_vorschlag(&reg, &vorschlag(Parameter::TrainingsStichprobenrate, bruch(5, 100))).is_ok());
    assert!(pruefe_vorschlag(&reg, &vorschlag(Parameter::TrainingsStichprobenrate, bruch(50, 100))).is_ok());

    // Und andersherum: Wer die Inferenzrate über die Trainingsrate hebt,
    // scheitert ebenso. Die Invariante gilt für beide Richtungen.
    let hoch = pruefe_vorschlag(&reg, &vorschlag(Parameter::Stichprobenrate, bruch(20, 100)));
    assert!(matches!(hoch, Err(VorschlagFehler::Invariante(_))));
}

/// **A3: Eine Preis-Untergrenze von null wird abgelehnt.**
#[test]
fn eine_preisuntergrenze_von_null_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();
    match pruefe_vorschlag(&reg, &vorschlag(Parameter::PreisUntergrenze, Wert::Ganzzahl(0))) {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::PreisUntergrenzePositiv)
        }
        andere => panic!("null muss scheitern, bekommen: {andere:?}"),
    }
}

// ---------------------------------------------------------------------
// Akzeptanzkriterium Phase 2: die Abstimmung schafft sich nicht selbst ab
// ---------------------------------------------------------------------

/// **Der Parameter, der über sich selbst abstimmt.**
///
/// `Abstimmungsmehrheit` ist änderbar und entscheidet über Änderungen.
/// Ohne Untergrenze genügten zwei Abstimmungen, um die Governance einer
/// Minderheit zu übergeben: erst die Schwelle auf null, dann alles
/// Übrige, und die zweite Abstimmung bräuchte keine Mehrheit mehr.
#[test]
fn die_mehrheitsschwelle_kann_nicht_unter_die_haelfte_gesenkt_werden() {
    let reg = ParameterRegistry::vorgabe();
    for schwelle in [0u64, 1, 100, 499] {
        let ergebnis = pruefe_vorschlag(
            &reg,
            &ParameterVorschlag {
                parameter: Parameter::Abstimmungsmehrheit,
                neuer_wert: Wert::Ganzzahl(schwelle),
            },
        );
        assert!(
            matches!(ergebnis, Err(VorschlagFehler::Invariante(_))),
            "eine Schwelle von {schwelle} Promille wurde zugelassen"
        );
    }
    // Gegenprobe: Änderbar bleibt sie, nur nicht bis zur
    // Wirkungslosigkeit. Ohne diese Hälfte hieße der Nachweis oben
    // vielleicht nur, dass gar nichts durchgeht.
    for schwelle in [500u64, 667, 1_000] {
        assert!(
            pruefe_vorschlag(
                &reg,
                &ParameterVorschlag {
                    parameter: Parameter::Abstimmungsmehrheit,
                    neuer_wert: Wert::Ganzzahl(schwelle),
                },
            )
            .is_ok(),
            "eine Schwelle von {schwelle} Promille wurde abgelehnt"
        );
    }
}

/// Eine Schwelle über allen Stimmen wäre eine verkleidete Sperre.
#[test]
fn eine_unerreichbare_mehrheitsschwelle_wird_zurueckgewiesen() {
    let reg = ParameterRegistry::vorgabe();
    let ergebnis = pruefe_vorschlag(
        &reg,
        &ParameterVorschlag {
            parameter: Parameter::Abstimmungsmehrheit,
            neuer_wert: Wert::Ganzzahl(1_001),
        },
    );
    assert!(matches!(ergebnis, Err(VorschlagFehler::Invariante(_))));
}

/// Ein Quorum von null ließe eine Abstimmung gelten, an der niemand
/// teilgenommen hat.
#[test]
fn ein_quorum_von_null_wird_zurueckgewiesen() {
    let reg = ParameterRegistry::vorgabe();
    assert!(matches!(
        pruefe_vorschlag(
            &reg,
            &ParameterVorschlag {
                parameter: Parameter::Abstimmungsquorum,
                neuer_wert: Wert::Ganzzahl(0),
            },
        ),
        Err(VorschlagFehler::Invariante(_))
    ));
    assert!(pruefe_vorschlag(
        &reg,
        &ParameterVorschlag {
            parameter: Parameter::Abstimmungsquorum,
            neuer_wert: Wert::Ganzzahl(1),
        },
    )
    .is_ok());
}

/// Ein Fenster von null Epochen schlösse die Abstimmung, bevor jemand
/// stimmen kann.
#[test]
fn ein_abstimmungsfenster_von_null_wird_zurueckgewiesen() {
    let reg = ParameterRegistry::vorgabe();
    assert!(matches!(
        pruefe_vorschlag(
            &reg,
            &ParameterVorschlag {
                parameter: Parameter::Abstimmungsfenster,
                neuer_wert: Wert::Ganzzahl(0),
            },
        ),
        Err(VorschlagFehler::Invariante(_))
    ));
    assert!(pruefe_vorschlag(
        &reg,
        &ParameterVorschlag {
            parameter: Parameter::Abstimmungsfenster,
            neuer_wert: Wert::Ganzzahl(1),
        },
    )
    .is_ok());
}

/// ⚑ **Ein Speichersatz unter dem Kostenboden wird abgelehnt**
/// (Punkt B4).
///
/// Die Schranke schützt nicht vor Betrug, sondern **vor dem
/// Verschwinden einer Rolle**: Unter den Kosten eines effizienten
/// Halters hält niemand mehr, und die Rolle Store ist unbesetzbar.
#[test]
fn ein_speichersatz_unter_den_kosten_wird_abgelehnt() {
    let reg = ParameterRegistry::vorgabe();
    let boden = myl_tokenomics::SPEICHER_KOSTENBODEN;

    // Genau der Boden ist zulaessig, einer darunter nicht.
    assert!(
        pruefe_vorschlag(&reg, &vorschlag(Parameter::Speichersatz, Wert::Ganzzahl(boden))).is_ok(),
        "genau der Kostenboden muss zulaessig sein"
    );
    let f = pruefe_vorschlag(
        &reg,
        &vorschlag(Parameter::Speichersatz, Wert::Ganzzahl(boden - 1)),
    );
    match f {
        Err(VorschlagFehler::Invariante(b)) => {
            assert_eq!(b.invariante, Invariante::SpeichersatzDecktKosten);
        }
        andere => panic!("erwartet war eine Kostenboden-Verletzung, bekommen: {andere:?}"),
    }

    // ⚑ **Und nach oben ist offen**, mit Absicht: Ein zu hoher Satz
    // macht Speichern teuer und ist eine wirtschaftliche Frage; nur ein
    // zu niedriger ist eine strukturelle.
    assert!(
        pruefe_vorschlag(
            &reg,
            &vorschlag(Parameter::Speichersatz, Wert::Ganzzahl(boden * 1_000)),
        )
        .is_ok(),
        "nach oben darf Governance entscheiden, auch wenn es unklug waere"
    );
}

/// Der Startwert des Speichersatzes, gegen die Zahl in `myl-tokenomics`.
///
/// **Es gibt sie einmal.** Die Registry verweist, sie schreibt nicht ab;
/// dieselbe Lehre wie bei Fund 146.
#[test]
fn der_speichersatz_ist_der_aus_tokenomics() {
    let reg = ParameterRegistry::vorgabe();
    let satz = reg
        .wert(Parameter::Speichersatz)
        .als_ganzzahl()
        .expect("Ganzzahl");
    assert_eq!(satz, myl_tokenomics::SPEICHERSATZ_VORGABE);
    assert_eq!(satz, 9_000, "Punkt B4: 9 000 Recheneinheiten je Byte-Epoche");
    // Der Abstand zum Boden ist der Anreiz, und er soll spuerbar sein.
    assert!(
        satz >= myl_tokenomics::SPEICHER_KOSTENBODEN * 2,
        "der Aufschlag ueber die Kosten ist kleiner als das Doppelte"
    );
}
