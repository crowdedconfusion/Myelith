//! Sicherheitsbedingungen, die jeder Parametersatz erfüllen muss
//! (Punkt 1.3).
//!
//! ## Die Regel dieses Moduls
//!
//! **Jede Invariante hier hat eine Fundstelle**, und die steht in ihrer
//! Dokumentation. Eine Schranke, die niemand entschieden hat, ist in
//! einem Governance-Modul kein Schutz, sondern eine heimliche
//! Festlegung: Sie verhindert Werte, über die eine Abstimmung befinden
//! dürfte, und niemand könnte sagen, warum.
//!
//! Zwei Arten kommen vor:
//!
//! - **Belegt** — steht im Whitepaper oder in einer datierten
//!   Design-Entscheidung (`S_min`, Self-Dealing, die
//!   Trainingsvergütungs-Obergrenze).
//! - **Strukturell** — folgt daraus, dass die Rechnung sonst nicht
//!   definiert ist oder ein anderes Crate umliefe (Nenner ungleich null,
//!   α in (0,1], Raten in [0,1]). Diese sind als solche gekennzeichnet.
//!
//! ## Geprüft wird der entstehende Zustand, nicht der Parameter
//!
//! `s < c/(1−c)` verbindet zwei Parameter. Wer nur den geänderten
//! ansieht, kann die Bedingung nicht prüfen. [`pruefe_invarianten`]
//! bekommt deshalb die **vollständige Registry nach Anwendung des
//! Vorschlags** und prüft alles.
//!
//! ## ⚑ Fund 49, geschlossen am 2026-08-24
//!
//! Anhang B.4 verlangt `s < c/(1−c)` und nennt `c` „empirisch 0,6–0,8".
//! `c` ist damit **keine Protokollgröße, sondern eine Beobachtung über
//! die Welt** — die realen Hardware- und Stromkosten als Anteil am
//! Reward. In der Registry steht es trotzdem, denn ohne einen Wert für
//! `c` ist die Ungleichung nicht auswertbar.
//!
//! Damit entsteht eine Lücke, die keine einzelne Prüfung schließt: Ein
//! Angreifer hebt zuerst `c` (jeder Schritt für sich zulässig, die
//! Ungleichung bleibt erfüllt), und hebt danach `s` unter die neue, höhere
//! Grenze. **Beide Vorschläge bestehen die Prüfung, das Ergebnis
//! verletzt die Bedingung**, denn das wahre `c` hat sich nicht bewegt.
//!
//! **Entschieden und umgesetzt:** `s` wird gegen das **untere** Ende des
//! Bandes geprüft (c = 0,6 ⇒ s < 1,5), nicht gegen den Registry-Wert.
//! [`self_dealing`] nimmt gar kein `c` mehr entgegen, und damit hat die
//! Zwei-Schritte-Lücke keinen ersten Schritt mehr. Die Schranke gilt
//! jetzt auch dann, wenn die realen Kosten am unteren Rand des
//! beobachteten Bereichs liegen; die Start-Subvention `s = 0,5` liegt mit
//! dem Dreifachen Abstand darunter.
//!
//! **`Parameter::Kostenanteil` bleibt in der Registry**, aber ohne
//! Wirkung auf diese Prüfung. Das ist genau das Muster, an dem sich das
//! Projekt schon dreimal verbrannt hat (A7, Fund 25, Fund 44: vorhanden,
//! getestet, nie benutzt), deshalb steht die Wirkungslosigkeit hier nicht
//! nur als Satz, sondern als **Test**:
//! `tests/akzeptanz.rs::kein_kostenanteil_bewegt_die_self_dealing_grenze`
//! ändert `c` über das ganze Band und verlangt, dass sich das Urteil über
//! `s` nicht bewegt. Wer die Prüfung eines Tages wieder an `c` koppelt,
//! bricht diesen Test.

use myl_tokenomics::sicherheit::{
    self_dealing_grenze, self_dealing_sicher_konservativ, s_min, KOSTENANTEIL_UNTEN_NENNER,
    KOSTENANTEIL_UNTEN_ZAEHLER,
};

use crate::registry::{Parameter, ParameterRegistry, Wert};

/// Eine benannte Sicherheitsbedingung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invariante {
    /// **Belegt, Kap. 5.5 und Anhang B.1:** `S ≥ g/p²`.
    ///
    /// Der hinterlegte Stake muss den Betrugsgewinn über den Zeithorizont
    /// bis zur ersten erwarteten Prüfung überwiegen. Wer `p` senkt oder
    /// `g` hebt, ohne `S` anzuheben, macht Betrug rational.
    MindestStake,
    /// ⚑ **Belegt, Punkt B4 vom 2026-09-02:** Der Speichersatz deckt die
    /// Kosten eines effizienten Halters.
    ///
    /// Ein Satz unter [`myl_tokenomics::SPEICHER_KOSTENBODEN`] heißt:
    /// Auch der günstigste Halter zahlt drauf, also hält niemand, also
    /// gibt es die Rolle Store nicht mehr. **Die Schranke schützt nicht
    /// vor Betrug, sondern vor dem Verschwinden einer Rolle**, und
    /// deshalb steht sie hier und nicht in einer Empfehlung.
    SpeichersatzDecktKosten,
    /// **Belegt, Anhang B.4:** `s < c/(1−c)`.
    ///
    /// Self-Dealing ist in der Subventionsphase genau dann
    /// verlustbringend, wenn die Subventionsrate unter dieser Schranke
    /// bleibt. Das Papier führt sie ausdrücklich „als
    /// Governance-Invariante".
    SelfDealing,
    /// **Belegt, Kap. 5.6:** Die Trainingsvergütung darf die
    /// Inferenzvergütung „nicht erreichen", also `< 1`.
    TrainingsverguetungUnterInferenz,
    /// **Strukturell:** Kein Bruch mit Nenner null.
    NennerNichtNull,
    /// **Strukturell:** Raten und Anteile liegen in `[0, 1]`.
    ///
    /// Betrifft `p`, `γ`, `u*`, `s` ist ausgenommen (eine Subvention über
    /// 100 % ist rechnerisch sinnvoll und wird von der
    /// Self-Dealing-Grenze beschränkt).
    RateInEinheitsintervall,
    /// **Strukturell, aus Fund 47:** `0 < α ≤ 1`.
    ///
    /// Ein α über 1 ergibt einen überkorrigierenden EMA-Schritt. Seit der
    /// Behebung läuft `myl-tokenomics` dabei nicht mehr um, sondern
    /// beschneidet; die Prüfung, dass es gar nicht erst dazu kommt,
    /// gehört hierher.
    EmaGlaettungInEinheitsintervall,
    /// **Strukturell, Kap. 4.4:** `r ≥ 2`.
    ///
    /// Bei `r = 1` gibt es keinen zweiten Pod, gegen den verglichen
    /// werden könnte; Stufe 1 der Verifikation entfällt ersatzlos und
    /// mit ihr die Grundlage der ganzen Architektur.
    RedundanzMindestensZwei,
    /// **Strukturell:** Das Komitee trägt mindestens einen byzantinischen
    /// Knoten, also `n ≥ 4` (BFT verlangt `n ≥ 3f+1`, `f ≥ 1`).
    KomiteeGrossGenug,
    /// **Strukturell, Anhang B.4:** `0 < c ≤ 0,8`.
    ///
    /// Ohne obere Schranke wäre `c/(1−c)` beliebig groß und die
    /// Self-Dealing-Grenze wertlos; `c ≥ 1` macht sie undefiniert. Die
    /// 0,8 sind das obere Ende des im Papier genannten Bandes. Siehe
    /// Fund 49 in der Modul-Dokumentation: Das begrenzt den Schaden,
    /// schließt die Lücke aber nicht.
    KostenanteilPlausibel,
    /// **Strukturell:** Mindestens ein Shard.
    ShardzahlMindestensEins,
    /// **Belegt, Kap. 5.7 und Anhang B.8.3:** `M_max ≥ B̄_e`.
    ///
    /// Kap. 5.7 sagt, ein Prägedeckel sei „nicht vorgesehen", und B.8.3
    /// begründet es: Ein **bindender** Deckel stabilisiert den Umlauf
    /// nicht, sondern bringt ihn zum Erliegen, weil dann mehr verbrannt
    /// als geprägt wird und Miner das Netz verlassen.
    ///
    /// Das Argument richtet sich gegen einen bindenden Deckel im
    /// Normalbetrieb, **nicht gegen den Mechanismus**: Als Notbremse
    /// gegen einen Fehler in der EMA oder einen Angriff auf die
    /// Verbrauchsmessung ist er sinnvoll. Diese Invariante trennt die
    /// beiden Fälle: Ein Deckel oberhalb des geglätteten Burns ist eine
    /// Obergrenze, einer darunter ist die Kapazitätsbremse aus B.8.3.
    PraegedeckelNichtBindend,
    /// **Belegt, Kap. 5.5:** Die Trainingsrate liegt nie unter der
    /// Inferenzrate.
    ///
    /// „Der Gewinn aus Betrug ist geringer, der Schaden dagegen größer,
    /// denn ein durchgerutschtes Inferenz-Segment betrifft eine Antwort,
    /// ein durchgerutschter Gradient hingegen das Modell und damit alle
    /// künftigen Antworten." Läge die Trainingsrate darunter, wäre der
    /// größere Schaden schlechter geschützt als der kleinere.
    TrainingsrateNichtUnterInferenzrate,
    /// **Strukturell, aus Fund 46:** Die Preis-Untergrenze ist positiv.
    ///
    /// Null hieße kostenlose Inferenz für alle.
    PreisUntergrenzePositiv,
    // ⚑ Hier stand bis zum 2026-09-02 `VorratTraegtEinschleusung`
    // (Kap. 6.7, Fund 58): Der Kontrollsegment-Vorrat musste das
    // Beobachtungsfenster bei Rate gamma tragen, sonst wiederholten sich
    // Segment-Ids und ein Miner mit Gedaechtnis erkannte die Kontrollen
    // sicher und ohne Fehlalarm.
    //
    // **Sie ist mit ihrem Gegenstand entfallen** (Entscheidung A1). Die
    // Erkenntnis dahinter gilt weiter und gehoert wieder hierher, sobald
    // jemand einen endlichen Vorrat in einen unbegrenzten Strom mischt:
    // Echte Arbeit wiederholt sich nie, ein Vorrat schon.
    /// **Eine Minderheit kann nie gewinnen, und eine leere Abstimmung
    /// nie entscheiden** (Kap. 10.2, Design-Entscheidung 1).
    ///
    /// # ⚑ Der Parameter, der über sich selbst abstimmt
    ///
    /// [`Parameter::Abstimmungsmehrheit`] ist änderbar und entscheidet
    /// über Änderungen. Ohne Untergrenze genügten **zwei**
    /// Abstimmungen, um die Governance einer Minderheit zu übergeben:
    /// erst die Schwelle auf null, dann alles Übrige. Die zweite
    /// Abstimmung bräuchte keine Mehrheit mehr, weil die erste sie
    /// abgeschafft hat.
    ///
    /// Das ist dieselbe Bauart wie der Verfassungsrang, nur eine Stufe
    /// tiefer: Dort ist ein Parameter gar nicht änderbar, hier ist er
    /// änderbar, aber nicht bis zur Wirkungslosigkeit. Eine Schwelle
    /// von 500 Promille heißt „mehr als die Hälfte", und darunter geht
    /// es nicht.
    ///
    /// Ebenso: Ein Quorum von null ließe eine Abstimmung ohne einen
    /// einzigen Teilnehmer gelten, und ein Fenster von null Epochen
    /// schlösse sie, bevor jemand stimmen kann.
    AbstimmungBleibtBindend,
}

impl Invariante {
    /// Die Fundstelle, für Fehlermeldungen und Protokolle.
    pub fn fundstelle(&self) -> &'static str {
        match self {
            Self::MindestStake => "Kap. 5.5, Anhang B.1",
            Self::SpeichersatzDecktKosten => "Punkt B4, 2026-09-02",
            Self::SelfDealing => "Anhang B.4",
            Self::TrainingsverguetungUnterInferenz => "Kap. 5.6",
            Self::NennerNichtNull => "strukturell",
            Self::RateInEinheitsintervall => "strukturell",
            Self::EmaGlaettungInEinheitsintervall => "strukturell, Fund 47",
            Self::RedundanzMindestensZwei => "Kap. 4.4",
            Self::KomiteeGrossGenug => "strukturell, BFT n >= 3f+1",
            Self::KostenanteilPlausibel => "Anhang B.4",
            Self::ShardzahlMindestensEins => "strukturell",
            Self::PraegedeckelNichtBindend => "Kap. 5.7, Anhang B.8.3",
            Self::TrainingsrateNichtUnterInferenzrate => "Kap. 5.5",
            Self::PreisUntergrenzePositiv => "strukturell, Fund 46",
            Self::AbstimmungBleibtBindend => "Kap. 10.2, strukturell",
        }
    }
}

/// Eine verletzte Invariante samt Begründung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantenBruch {
    /// Welche Bedingung verletzt ist.
    pub invariante: Invariante,
    /// Der Parameter, an dem es auffällt.
    pub parameter: Parameter,
    /// Warum, im Klartext und mit Zahlen.
    pub begruendung: String,
}

impl std::fmt::Display for InvariantenBruch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.parameter.name(),
            self.invariante.fundstelle(),
            self.begruendung
        )
    }
}

impl std::error::Error for InvariantenBruch {}

/// Prüft **alle** Invarianten gegen einen vollständigen Parametersatz.
///
/// Gibt die erste Verletzung zurück, in fester Reihenfolge: Zwei Knoten,
/// die denselben Vorschlag prüfen, müssen dieselbe Begründung nennen,
/// sonst wäre die Ablehnung selbst nicht deterministisch.
pub fn pruefe_invarianten(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    strukturelle_pruefungen(reg)?;
    mindeststake(reg)?;
    speichersatz(reg)?;
    self_dealing(reg)?;
    trainingsverguetung(reg)?;
    praegedeckel(reg)?;
    trainingsrate(reg)?;
    abstimmung_bleibt_bindend(reg)?;
    Ok(())
}

/// Nenner, Einheitsintervall, Mindestgrößen.
fn strukturelle_pruefungen(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    // Kein Bruch mit Nenner null — zuerst, weil jede weitere Rechnung
    // sonst durch null teilt.
    for p in Parameter::alle() {
        if let Wert::Bruch { zaehler, nenner } = reg.wert(p) {
            if *nenner == 0 {
                return Err(InvariantenBruch {
                    invariante: Invariante::NennerNichtNull,
                    parameter: p,
                    begruendung: format!("{}/0 ist keine Zahl", zaehler),
                });
            }
        }
    }

    for p in [
        Parameter::Stichprobenrate,
        Parameter::Auslastungsziel,
        Parameter::Kopfgeldanteil,
        Parameter::TrainingsStichprobenrate,
    ] {
        let (z, n) = reg.wert(p).als_bruch().expect("Rate ist ein Bruch");
        if z > n {
            return Err(InvariantenBruch {
                invariante: Invariante::RateInEinheitsintervall,
                parameter: p,
                begruendung: format!("{}/{} liegt über 1", z, n),
            });
        }
    }

    let (az, an) = reg
        .wert(Parameter::EmaGlaettung)
        .als_bruch()
        .expect("alpha ist ein Bruch");
    if az == 0 || az > an {
        return Err(InvariantenBruch {
            invariante: Invariante::EmaGlaettungInEinheitsintervall,
            parameter: Parameter::EmaGlaettung,
            begruendung: format!(
                "alpha = {}/{} liegt nicht in (0,1]; ein überkorrigierender \
                 EMA-Schritt war Fund 47",
                az, an
            ),
        });
    }

    let (cz, cn) = reg
        .wert(Parameter::Kostenanteil)
        .als_bruch()
        .expect("c ist ein Bruch");
    // 0 < c <= 0,8, also 5·cz <= 4·cn und cz > 0.
    if cz == 0 || (cz as u128) * 5 > (cn as u128) * 4 {
        return Err(InvariantenBruch {
            invariante: Invariante::KostenanteilPlausibel,
            parameter: Parameter::Kostenanteil,
            begruendung: format!(
                "c = {}/{} liegt nicht in (0; 0,8]; Anhang B.4 nennt empirisch 0,6 bis 0,8",
                cz, cn
            ),
        });
    }

    let r = reg
        .wert(Parameter::Redundanzfaktor)
        .als_ganzzahl()
        .expect("r ist eine Ganzzahl");
    if r < 2 {
        return Err(InvariantenBruch {
            invariante: Invariante::RedundanzMindestensZwei,
            parameter: Parameter::Redundanzfaktor,
            begruendung: format!(
                "r = {} lässt keinen Vergleich zweier Pods zu; Stufe 1 der \
                 Verifikation entfiele ersatzlos",
                r
            ),
        });
    }

    let n = reg
        .wert(Parameter::Komiteegroesse)
        .als_ganzzahl()
        .expect("Komiteegröße ist eine Ganzzahl");
    if n < 4 {
        return Err(InvariantenBruch {
            invariante: Invariante::KomiteeGrossGenug,
            parameter: Parameter::Komiteegroesse,
            begruendung: format!("n = {} erfüllt n >= 3f+1 für kein f >= 1", n),
        });
    }

    let untergrenze = reg
        .wert(Parameter::PreisUntergrenze)
        .als_ganzzahl()
        .expect("Untergrenze ist eine Ganzzahl");
    if untergrenze == 0 {
        return Err(InvariantenBruch {
            invariante: Invariante::PreisUntergrenzePositiv,
            parameter: Parameter::PreisUntergrenze,
            begruendung: "eine Untergrenze von null heißt kostenlose Inferenz für alle"
                .to_string(),
        });
    }

    let k = reg
        .wert(Parameter::Shardzahl)
        .als_ganzzahl()
        .expect("k ist eine Ganzzahl");
    if k == 0 {
        return Err(InvariantenBruch {
            invariante: Invariante::ShardzahlMindestensEins,
            parameter: Parameter::Shardzahl,
            begruendung: "k = 0 beschreibt keine Pipeline".to_string(),
        });
    }

    Ok(())
}

/// `S ≥ g/p²` (Kap. 5.5, Anhang B.1).
fn mindeststake(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let (pz, pn) = reg
        .wert(Parameter::Stichprobenrate)
        .als_bruch()
        .expect("p ist ein Bruch");
    let g = reg
        .wert(Parameter::Betrugsgewinn)
        .als_ganzzahl()
        .expect("g ist eine Ganzzahl");
    let s = reg
        .wert(Parameter::MindestStake)
        .als_ganzzahl()
        .expect("S ist eine Ganzzahl");

    // Die Formel steht in myl-tokenomics und wird hier **benutzt**, nicht
    // wiederholt (TOKENOMICS 3.3).
    let noetig = s_min(g, pz, pn).map_err(|e| InvariantenBruch {
        invariante: Invariante::MindestStake,
        parameter: Parameter::Stichprobenrate,
        begruendung: format!("S_min nicht berechenbar: {}", e),
    })?;

    if s < noetig {
        return Err(InvariantenBruch {
            invariante: Invariante::MindestStake,
            parameter: Parameter::MindestStake,
            begruendung: format!(
                "S = {} liegt unter S_min = g/p² = {} (g = {}, p = {}/{})",
                s, noetig, g, pz, pn
            ),
        });
    }
    Ok(())
}

/// Der Speichersatz deckt die Kosten eines effizienten Halters
/// (Punkt B4).
///
/// **Die Zahl steht in `myl-tokenomics` und wird hier benutzt**, nicht
/// wiederholt: dieselbe Arbeitsteilung wie bei `S_min`. Dieses Modul
/// entscheidet, *dass* die Bedingung gilt; wie hoch der Boden liegt und
/// woraus er folgt, steht dort, wo die übrigen wirtschaftlichen Größen
/// stehen.
///
/// ⚑ **Nur eine Untergrenze, keine Obergrenze.** Ein zu hoher Satz
/// macht Speichern teuer und ist eine wirtschaftliche Frage; ein zu
/// niedriger macht die Rolle Store unbesetzbar und ist eine
/// strukturelle. Nur die zweite gehört in eine Invariante.
fn speichersatz(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let satz = reg
        .wert(Parameter::Speichersatz)
        .als_ganzzahl()
        .expect("der Speichersatz ist eine Ganzzahl");
    if satz < myl_tokenomics::SPEICHER_KOSTENBODEN {
        return Err(InvariantenBruch {
            invariante: Invariante::SpeichersatzDecktKosten,
            parameter: Parameter::Speichersatz,
            begruendung: format!(
                "Satz {satz} liegt unter dem Kostenboden {}; auch ein effizienter \
                 Halter zahlte dann drauf",
                myl_tokenomics::SPEICHER_KOSTENBODEN
            ),
        });
    }
    Ok(())
}

/// `s < c/(1−c)` gegen das **untere** Bandende (Anhang B.4, ⚑ Fund 49).
///
/// **Die Formel steht in `myl-tokenomics` und wird hier benutzt**, nicht
/// wiederholt — dieselbe Arbeitsteilung wie bei `S_min`. Dieses Modul
/// entscheidet, *dass* die Bedingung gilt; wie sie lautet, steht dort, wo
/// die übrigen ökonomischen Formeln stehen.
///
/// **Geprüft wird gegen `c = 0,6`, nicht gegen den Registry-Wert.** Das
/// schließt Fund 49: Wäre `c` Teil der Prüfung, ließe sich die Grenze in
/// zwei je zulässigen Schritten verschieben, erst `c` heben, dann `s`.
/// Die konservative Fassung nimmt gar kein `c` entgegen, und damit hat
/// die Lücke keinen ersten Schritt mehr.
fn self_dealing(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let (sz, sn) = reg
        .wert(Parameter::Subventionsrate)
        .als_bruch()
        .expect("s ist ein Bruch");

    let sicher = self_dealing_sicher_konservativ(sz, sn).map_err(|e| InvariantenBruch {
        invariante: Invariante::SelfDealing,
        parameter: Parameter::Subventionsrate,
        begruendung: format!("Self-Dealing-Grenze nicht auswertbar: {}", e),
    })?;

    if !sicher {
        let (gz, gn) =
            self_dealing_grenze(KOSTENANTEIL_UNTEN_ZAEHLER, KOSTENANTEIL_UNTEN_NENNER)
                .unwrap_or((0, 1));
        return Err(InvariantenBruch {
            invariante: Invariante::SelfDealing,
            parameter: Parameter::Subventionsrate,
            begruendung: format!(
                "s = {}/{} erreicht oder übersteigt {}/{}, die Grenze c/(1-c) am unteren \
                 Ende des Bandes aus Anhang B.4 (c = 0,6); Self-Dealing wäre damit nicht \
                 mehr verlustbringend",
                sz, sn, gz, gn
            ),
        });
    }
    Ok(())
}

/// Trainingsvergütung `< 1` (Kap. 5.6).
fn trainingsverguetung(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let (z, n) = reg
        .wert(Parameter::TrainingsverguetungsAnteil)
        .als_bruch()
        .expect("Anteil ist ein Bruch");
    if z >= n {
        return Err(InvariantenBruch {
            invariante: Invariante::TrainingsverguetungUnterInferenz,
            parameter: Parameter::TrainingsverguetungsAnteil,
            begruendung: format!(
                "{}/{} erreicht die Inferenzvergütung; Kap. 5.6 verlangt darunter, \
                 sonst verlagern Miner Kapazität von der Inferenz aufs Training",
                z, n
            ),
        });
    }
    Ok(())
}

/// `M_max ≥ B̄_e` (Kap. 5.7, Anhang B.8.3).
fn praegedeckel(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let deckel = reg
        .wert(Parameter::PraegeObergrenze)
        .als_ganzzahl()
        .expect("M_max ist eine Ganzzahl");
    let burn = reg
        .wert(Parameter::GeglaetteterBurn)
        .als_ganzzahl()
        .expect("B_e ist eine Ganzzahl");
    if deckel < burn {
        return Err(InvariantenBruch {
            invariante: Invariante::PraegedeckelNichtBindend,
            parameter: Parameter::PraegeObergrenze,
            begruendung: format!(
                "M_max = {} liegt unter dem geglätteten Burn {}; ein bindender Deckel \
                 stabilisiert den Umlauf nicht, sondern bringt ihn zum Erliegen (B.8.3)",
                deckel, burn
            ),
        });
    }
    Ok(())
}

/// Trainingsrate ≥ Inferenzrate (Kap. 5.5).
fn trainingsrate(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let (pz, pn) = reg
        .wert(Parameter::Stichprobenrate)
        .als_bruch()
        .expect("p ist ein Bruch");
    let (tz, tn) = reg
        .wert(Parameter::TrainingsStichprobenrate)
        .als_bruch()
        .expect("p_train ist ein Bruch");
    // tz/tn >= pz/pn  <=>  tz*pn >= pz*tn
    if (tz as u128) * (pn as u128) < (pz as u128) * (tn as u128) {
        return Err(InvariantenBruch {
            invariante: Invariante::TrainingsrateNichtUnterInferenzrate,
            parameter: Parameter::TrainingsStichprobenrate,
            begruendung: format!(
                "Trainingsrate {}/{} liegt unter der Inferenzrate {}/{}; der größere \
                 Schaden wäre damit schlechter geschützt als der kleinere (Kap. 5.5)",
                tz, tn, pz, pn
            ),
        });
    }
    Ok(())
}


/// Die Abstimmung darf sich nicht selbst abschaffen.
///
/// Drei Untergrenzen, alle strukturell und keine davon Politik:
///
/// - **Mehrheit ≥ 500 Promille.** Darunter gewänne eine Minderheit.
/// - **Quorum ≥ 1 Promille.** Bei null entschiede eine Abstimmung, an
///   der niemand teilgenommen hat.
/// - **Fenster ≥ 1 Epoche.** Bei null schlösse sie, bevor jemand
///   stimmen kann.
///
/// **Was hier ausdrücklich nicht geprüft wird**, sind die Werte selbst.
/// Ob das Quorum bei 100 oder bei 400 Promille liegt, ist eine
/// Abwägung zwischen Handlungsfähigkeit und Legitimität, und die trifft
/// niemand in einer Invariante. Geprüft wird allein, dass das Verfahren
/// ein Verfahren bleibt.
fn abstimmung_bleibt_bindend(reg: &ParameterRegistry) -> Result<(), InvariantenBruch> {
    let ganzzahl = |p: Parameter| reg.wert(p).als_ganzzahl().unwrap_or(0);

    let mehrheit = ganzzahl(Parameter::Abstimmungsmehrheit);
    if mehrheit < crate::abstimmung::MEHRHEIT_UNTERGRENZE {
        return Err(InvariantenBruch {
            invariante: Invariante::AbstimmungBleibtBindend,
            parameter: Parameter::Abstimmungsmehrheit,
            begruendung: format!(
                "{mehrheit} Promille lägen unter der Hälfte: eine Minderheit könnte \
                 beschließen, und der erste Beschluss wäre die Abschaffung des Restes \
                 (Untergrenze {} Promille)",
                crate::abstimmung::MEHRHEIT_UNTERGRENZE
            ),
        });
    }
    if mehrheit > 1_000 {
        return Err(InvariantenBruch {
            invariante: Invariante::AbstimmungBleibtBindend,
            parameter: Parameter::Abstimmungsmehrheit,
            begruendung: format!(
                "{mehrheit} Promille sind mehr als alle Stimmen: kein Vorschlag \
                 könnte je angenommen werden"
            ),
        });
    }

    let quorum = ganzzahl(Parameter::Abstimmungsquorum);
    if quorum == 0 {
        return Err(InvariantenBruch {
            invariante: Invariante::AbstimmungBleibtBindend,
            parameter: Parameter::Abstimmungsquorum,
            begruendung: "ein Quorum von null ließe eine Abstimmung gelten, an der \
                          niemand teilgenommen hat"
                .to_string(),
        });
    }
    if quorum > 1_000 {
        return Err(InvariantenBruch {
            invariante: Invariante::AbstimmungBleibtBindend,
            parameter: Parameter::Abstimmungsquorum,
            begruendung: format!(
                "{quorum} Promille verlangen mehr Beteiligung, als es Stimmkraft gibt"
            ),
        });
    }

    let fenster = ganzzahl(Parameter::Abstimmungsfenster);
    if fenster == 0 {
        return Err(InvariantenBruch {
            invariante: Invariante::AbstimmungBleibtBindend,
            parameter: Parameter::Abstimmungsfenster,
            begruendung: "ein Fenster von null Epochen schlösse die Abstimmung, \
                          bevor jemand stimmen kann"
                .to_string(),
        });
    }
    Ok(())
}
