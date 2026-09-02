//! Abstimmung über Parametervorschläge (Phase 2, Kap. 10.2/10.3).
//!
//! # Was Kap. 10.2 festlegt, und was nicht
//!
//! Festgelegt ist das **Stimmgewicht**: Stake mal Arbeitshistorie,
//! dieselbe Formel wie bei der Validatorenwahl. Nicht festgelegt ist das
//! **Verfahren**: einfache Mehrheit, Quorum, Zeitfenster. Das ist
//! Design-Entscheidung 1 und offen.
//!
//! Dieses Modul baut deshalb die Mechanik und legt die drei Zahlen in
//! die Registry, statt sie in den Quelltext zu schreiben. Was gebaut
//! ist, steht damit fest; was Politik ist, bleibt entscheidbar, und die
//! Entscheidung ändert später keine Zeile Code.
//!
//! # ⚑ Die Formel wird gerufen, nicht abgeschrieben
//!
//! [`gewicht`] ruft `myl_consensus::calculate_voting_weight_mit`. Eine
//! zweite Fassung derselben Formel wäre genau der Fehler, den das
//! Akzeptanzkriterium dieser Phase ausdrücklich ausschließt, und den
//! Fund 58 innerhalb eines einzigen Crates schon einmal produziert hat:
//! zwei Rechnungen für dieselbe Größe, dreißig Zeilen auseinander, eine
//! davon veraltet.
//!
//! # ⚑ Das Gewicht steht bei der Eröffnung fest
//!
//! Gerechnet wird gegen die Epoche, in der die Abstimmung **eröffnet**
//! wurde, nicht gegen die laufende. Der Unterschied ist kein Detail:
//! Die Arbeitshistorie zerfällt je Epoche, und über ein Fenster von
//! Tagen verschöbe sich jedes Gewicht während der Abstimmung. Wer früh
//! stimmt, hätte ein anderes Gewicht als wer spät stimmt, und wer
//! seinen Stake während des Fensters bewegt, stimmte zweimal mit
//! demselben Geld.
//!
//! # Was gezählt wird
//!
//! - **Das Quorum** misst gegen die Stimmkraft **aller**
//!   Stimmberechtigten, nicht gegen die abgegebenen Stimmen. Sonst
//!   erfüllte jede Abstimmung ihr Quorum, an der überhaupt jemand
//!   teilnimmt.
//! - **Enthaltungen zählen zum Quorum, nicht zur Mehrheit.** Wer sich
//!   enthält, nimmt teil und stimmt nicht zu; beides ist eine Aussage,
//!   und die Trennung hält sie auseinander. Eine Enthaltung ist damit
//!   **kein** verstecktes Nein.
//! - **Angenommen ist, was die Schwelle überschreitet**, nicht was sie
//!   erreicht. Bei Gleichstand und einer Schwelle von 500 Promille wäre
//!   sonst die Hälfte genug.

use std::collections::BTreeMap;

use myl_consensus::voting_weight::{
    calculate_voting_weight_mit, InferenceHistory, StimmgewichtsParameter,
};
use myl_types::ids::{EpochId, MinerId};

use crate::registry::{Parameter, ParameterRegistry};
use crate::vorschlag::{pruefe_vorschlag, ParameterVorschlag, VorschlagFehler};

/// Untergrenze der Mehrheitsschwelle in Promille.
///
/// 500 heißt „mehr als die Hälfte". Darunter könnte eine Minderheit
/// beschließen, und ihr erster Beschluss wäre die Abschaffung des
/// Restes. Die Invariante
/// [`crate::invarianten::Invariante::AbstimmungBleibtBindend`] hält
/// diese Grenze.
pub const MEHRHEIT_UNTERGRENZE: u64 = 500;

/// Entwurfswert der Mehrheitsschwelle: 500 Promille, also mehr als die
/// Hälfte der abgegebenen Ja- und Nein-Stimmen.
pub const MEHRHEIT_VORGABE: u64 = 500;

/// Entwurfswert des Quorums: 200 Promille der Gesamtstimmkraft.
///
/// **Eine Entwurfszahl, keine Entscheidung.** Sie liegt bewusst nicht
/// bei der Hälfte: Ein Netz, dessen Stimmkraft zu grossen Teilen auf
/// Knoten liegt, die keine Governance verfolgen, wäre sonst dauerhaft
/// handlungsunfähig, und Handlungsunfähigkeit ist auch ein Zustand, den
/// jemand herbeiführen kann.
pub const QUORUM_VORGABE: u64 = 200;

/// Entwurfswert des Abstimmungsfensters: 168 Epochen.
///
/// Bei Stunden-Epochen sind das sieben Tage, dieselbe Spanne wie die
/// Streitfrist. Wer über eine Woche nicht abstimmt, hat nicht
/// teilgenommen, und nicht bloß geschlafen.
pub const FENSTER_VORGABE: u64 = 168;

/// Die Stimme eines Wählers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stimme {
    /// Zustimmung.
    Dafuer,
    /// Ablehnung.
    Dagegen,
    /// Teilnahme ohne Zustimmung oder Ablehnung.
    Enthaltung,
}

/// Ein Stimmberechtigter mit allem, was sein Gewicht bestimmt.
#[derive(Debug, Clone)]
pub struct Stimmberechtigt {
    /// Wer.
    pub waehler: MinerId,
    /// Hinterlegter Stake in Kleinstbeträgen.
    pub stake: u64,
    /// Geleistete Arbeit je Epoche.
    pub historie: InferenceHistory,
}

/// Das Stimmgewicht eines Berechtigten zur Epoche `epoche`.
///
/// Ruft die Formel aus `myl-consensus`, statt sie zu wiederholen.
pub fn gewicht(
    berechtigt: &Stimmberechtigt,
    epoche: EpochId,
    parameter: &StimmgewichtsParameter,
) -> u64 {
    calculate_voting_weight_mit(
        berechtigt.stake,
        &berechtigt.historie,
        epoche.0,
        parameter,
    )
}

/// Die Stimmgewichtsparameter, wie die Registry sie führt.
///
/// Zweiter Teil des Akzeptanzkriteriums: Nicht nur die Formel, auch
/// ihre Parameter kommen aus einer Quelle. Ein Gleichstandstest hält
/// die Registry-Werte mit den Vorgaben aus `myl-consensus` zusammen.
pub fn stimmgewichts_parameter(reg: &ParameterRegistry) -> StimmgewichtsParameter {
    StimmgewichtsParameter {
        schwelle_zaehler: reg
            .wert(Parameter::ArbeitsschwelleZaehler)
            .als_ganzzahl()
            .unwrap_or(myl_consensus::voting_weight::ARBEITSSCHWELLE_ZAEHLER_VORGABE),
        schwelle_nenner: reg
            .wert(Parameter::ArbeitsschwelleNenner)
            .als_ganzzahl()
            .unwrap_or(myl_consensus::voting_weight::ARBEITSSCHWELLE_NENNER_VORGABE),
    }
}

/// Was beim Abstimmen schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstimmungFehler {
    /// Die Frist ist abgelaufen.
    FristAbgelaufen { jetzt: EpochId, frist: EpochId },
    /// Es wird gestimmt, bevor die Abstimmung eröffnet ist.
    NochNichtEroeffnet { jetzt: EpochId, eroeffnet: EpochId },
    /// Ausgezählt wird erst nach Fristende.
    FristLaeuftNoch { jetzt: EpochId, frist: EpochId },
    /// Der Wähler steht nicht auf der Liste der Berechtigten.
    NichtStimmberechtigt { waehler: MinerId },
    /// Ein Wähler steht zweimal auf der Liste der Berechtigten.
    ///
    /// Dann zählte sein Gewicht doppelt im Nenner des Quorums, und die
    /// Abstimmung wäre schwerer zu erfüllen, als sie sein soll.
    DoppeltBerechtigt { waehler: MinerId },
    /// Der Vorschlag hält der Prüfung nicht mehr stand.
    ///
    /// Kein Fehler des Verfahrens, sondern ein Befund: Zwischen
    /// Eröffnung und Anwendung hat sich die Registry geändert.
    NichtMehrGueltig(VorschlagFehler),
}

impl std::fmt::Display for AbstimmungFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FristAbgelaufen { jetzt, frist } => write!(
                f,
                "Epoche {jetzt} liegt nach der Frist {frist}: die Abstimmung ist zu"
            ),
            Self::NochNichtEroeffnet { jetzt, eroeffnet } => write!(
                f,
                "Epoche {jetzt} liegt vor der Eröffnung {eroeffnet}"
            ),
            Self::FristLaeuftNoch { jetzt, frist } => write!(
                f,
                "Epoche {jetzt} liegt vor dem Fristende {frist}: es darf noch \
                 gestimmt werden"
            ),
            Self::NichtStimmberechtigt { waehler } => {
                write!(f, "{waehler} steht nicht auf der Liste der Berechtigten")
            }
            Self::DoppeltBerechtigt { waehler } => write!(
                f,
                "{waehler} steht zweimal auf der Liste: sein Gewicht zählte doppelt"
            ),
            Self::NichtMehrGueltig(e) => write!(
                f,
                "der angenommene Vorschlag hält der Prüfung nicht mehr stand: {e}"
            ),
        }
    }
}

impl std::error::Error for AbstimmungFehler {}

/// Warum ein Vorschlag abgelehnt wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablehnungsgrund {
    /// Zu wenig Beteiligung.
    QuorumVerfehlt {
        /// Beteiligung in Promille der Gesamtstimmkraft.
        beteiligung: u64,
        /// Verlangte Beteiligung in Promille.
        noetig: u64,
    },
    /// Genug Beteiligung, zu wenig Zustimmung.
    MehrheitVerfehlt {
        /// Zustimmung in Promille der Ja- und Nein-Stimmen.
        zustimmung: u64,
        /// Verlangte Zustimmung in Promille.
        noetig: u64,
    },
}

/// Das Ergebnis einer Auszählung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Auszaehlung {
    /// Stimmkraft für den Vorschlag.
    pub dafuer: u128,
    /// Stimmkraft gegen den Vorschlag.
    pub dagegen: u128,
    /// Stimmkraft, die sich enthalten hat.
    pub enthaltung: u128,
    /// Stimmkraft aller Berechtigten, ob sie gestimmt haben oder nicht.
    pub gesamt: u128,
}

impl Auszaehlung {
    /// Beteiligung in Promille der Gesamtstimmkraft.
    pub fn beteiligung_promille(&self) -> u64 {
        if self.gesamt == 0 {
            return 0;
        }
        let teilgenommen = self.dafuer + self.dagegen + self.enthaltung;
        ((teilgenommen * 1_000) / self.gesamt) as u64
    }

    /// Zustimmung in Promille der Ja- und Nein-Stimmen.
    ///
    /// Enthaltungen stehen nicht im Nenner: Wer sich enthält, hat weder
    /// zugestimmt noch abgelehnt, und eine Enthaltung ist kein
    /// verstecktes Nein.
    pub fn zustimmung_promille(&self) -> u64 {
        let entschieden = self.dafuer + self.dagegen;
        if entschieden == 0 {
            return 0;
        }
        ((self.dafuer * 1_000) / entschieden) as u64
    }

    /// Angenommen oder abgelehnt, nach Quorum und Mehrheit.
    pub fn ergebnis(&self, quorum: u64, mehrheit: u64) -> Result<(), Ablehnungsgrund> {
        let beteiligung = self.beteiligung_promille();
        if beteiligung < quorum {
            return Err(Ablehnungsgrund::QuorumVerfehlt {
                beteiligung,
                noetig: quorum,
            });
        }
        let zustimmung = self.zustimmung_promille();
        // Überschreiten, nicht erreichen: Bei Gleichstand und einer
        // Schwelle von 500 Promille wäre die Hälfte sonst genug.
        if zustimmung <= mehrheit {
            return Err(Ablehnungsgrund::MehrheitVerfehlt {
                zustimmung,
                noetig: mehrheit,
            });
        }
        Ok(())
    }
}

/// Eine laufende Abstimmung über einen Parametervorschlag.
#[derive(Debug, Clone)]
pub struct Abstimmung {
    vorschlag: ParameterVorschlag,
    eroeffnet: EpochId,
    frist: EpochId,
    quorum_promille: u64,
    mehrheit_promille: u64,
    stimmgewicht: StimmgewichtsParameter,
    stimmen: BTreeMap<MinerId, Stimme>,
}

impl Abstimmung {
    /// Eröffnet eine Abstimmung über einen Vorschlag.
    ///
    /// Der Vorschlag wird **hier** geprüft, wie es Punkt 1.3 verlangt:
    /// Was Verfassungsrang hat oder eine Invariante bricht, kommt gar
    /// nicht erst zur Abstimmung. Quorum, Mehrheit und Fenster kommen
    /// aus der Registry, damit die Zahlen abstimmbar bleiben, ohne dass
    /// jemand Code anfasst.
    pub fn eroeffne(
        reg: &ParameterRegistry,
        vorschlag: ParameterVorschlag,
        eroeffnet: EpochId,
    ) -> Result<Self, VorschlagFehler> {
        pruefe_vorschlag(reg, &vorschlag)?;
        let ganzzahl = |p: Parameter| reg.wert(p).als_ganzzahl().unwrap_or(0);
        let fenster = ganzzahl(Parameter::Abstimmungsfenster);
        Ok(Self {
            vorschlag,
            eroeffnet,
            // Das Fenster zählt die Eröffnungsepoche mit: Ein Fenster
            // von einer Epoche heißt „diese Epoche".
            frist: EpochId(eroeffnet.0.saturating_add(fenster.saturating_sub(1))),
            quorum_promille: ganzzahl(Parameter::Abstimmungsquorum),
            mehrheit_promille: ganzzahl(Parameter::Abstimmungsmehrheit),
            stimmgewicht: stimmgewichts_parameter(reg),
            stimmen: BTreeMap::new(),
        })
    }

    /// Der Vorschlag, über den abgestimmt wird.
    pub fn vorschlag(&self) -> &ParameterVorschlag {
        &self.vorschlag
    }

    /// Die Epoche der Eröffnung. Gegen sie wird jedes Gewicht gerechnet.
    pub fn eroeffnet(&self) -> EpochId {
        self.eroeffnet
    }

    /// Letzte Epoche, in der gestimmt werden darf.
    pub fn frist(&self) -> EpochId {
        self.frist
    }

    /// Ab welcher Epoche ein angenommener Vorschlag gilt (Punkt 2.3).
    ///
    /// Die Epoche **nach** der Frist, nie früher. Ein Vorschlag, der
    /// mitten in einer Epoche wirksam würde, änderte die Regeln, unter
    /// denen die laufende Epoche begonnen hat.
    pub fn wirksam_ab(&self) -> EpochId {
        EpochId(self.frist.0.saturating_add(1))
    }

    /// Nimmt eine Stimme entgegen.
    ///
    /// Eine spätere Stimme ersetzt eine frühere: Wer seine Meinung
    /// ändert, darf das, solange die Frist läuft. Gezählt wird die
    /// letzte.
    pub fn stimme_ab(
        &mut self,
        waehler: MinerId,
        stimme: Stimme,
        jetzt: EpochId,
    ) -> Result<(), AbstimmungFehler> {
        if jetzt < self.eroeffnet {
            return Err(AbstimmungFehler::NochNichtEroeffnet {
                jetzt,
                eroeffnet: self.eroeffnet,
            });
        }
        if jetzt > self.frist {
            return Err(AbstimmungFehler::FristAbgelaufen {
                jetzt,
                frist: self.frist,
            });
        }
        self.stimmen.insert(waehler, stimme);
        Ok(())
    }

    /// Zahl der abgegebenen Stimmen.
    pub fn abgegeben(&self) -> usize {
        self.stimmen.len()
    }

    /// Zählt aus, nach Fristende.
    ///
    /// `berechtigte` ist der Satz der Stimmberechtigten. Er kommt von
    /// außen, weil er im Konsens steht und nicht in dieser Komponente;
    /// die Registry weiß nichts über Validatoren.
    pub fn zaehle_aus(
        &self,
        berechtigte: &[Stimmberechtigt],
        jetzt: EpochId,
    ) -> Result<Auszaehlung, AbstimmungFehler> {
        if jetzt <= self.frist {
            return Err(AbstimmungFehler::FristLaeuftNoch {
                jetzt,
                frist: self.frist,
            });
        }

        let mut gesehen: BTreeMap<MinerId, u64> = BTreeMap::new();
        for b in berechtigte {
            let g = gewicht(b, self.eroeffnet, &self.stimmgewicht);
            if gesehen.insert(b.waehler, g).is_some() {
                return Err(AbstimmungFehler::DoppeltBerechtigt { waehler: b.waehler });
            }
        }

        // Eine Stimme von jemandem, der nicht auf der Liste steht, ist
        // keine ungültige Stimme, sondern ein Fehler im Aufruf: Wer sie
        // stillschweigend überginge, zählte eine Abstimmung aus, deren
        // Eingaben er nicht versteht.
        for waehler in self.stimmen.keys() {
            if !gesehen.contains_key(waehler) {
                return Err(AbstimmungFehler::NichtStimmberechtigt { waehler: *waehler });
            }
        }

        let mut dafuer: u128 = 0;
        let mut dagegen: u128 = 0;
        let mut enthaltung: u128 = 0;
        for (waehler, stimme) in &self.stimmen {
            let g = gesehen[waehler] as u128;
            match stimme {
                Stimme::Dafuer => dafuer += g,
                Stimme::Dagegen => dagegen += g,
                Stimme::Enthaltung => enthaltung += g,
            }
        }
        let gesamt: u128 = gesehen.values().map(|g| *g as u128).sum();

        Ok(Auszaehlung {
            dafuer,
            dagegen,
            enthaltung,
            gesamt,
        })
    }

    /// Zählt aus und wendet bei Annahme an (Punkte 2.2 und 2.3).
    ///
    /// # ⚑ Der Vorschlag wird ein zweites Mal geprüft
    ///
    /// Bei der Eröffnung wurde er gegen die Registry geprüft, wie sie
    /// **damals** aussah. Bis zur Anwendung vergeht ein Fenster, und in
    /// dieser Zeit können andere Vorschläge wirksam geworden sein.
    ///
    /// **Zwei Vorschläge, jeder für sich zulässig, können zusammen eine
    /// Invariante brechen.** Ein Beispiel aus dieser Registry: Die
    /// Trainings-Stichprobenrate darf nie unter der Inferenzrate liegen.
    /// Eine Abstimmung senkt die Trainingsrate auf das gerade noch
    /// Zulässige, eine zweite, gleichzeitig laufende, hebt die
    /// Inferenzrate. Beide waren bei ihrer Eröffnung zulässig; zusammen
    /// liegt die Trainingsrate darunter, und der größere Schaden wäre
    /// schlechter geschützt als der kleinere.
    ///
    /// ⚑ **Hier stand bis zum 2026-09-02 das Kontrollsegment-Beispiel**
    /// (Vorrat gegen γ, Fund 58). Es ist mit den Kontrollsegmenten
    /// entfallen; die Eigenschaft, die es zeigte, ist geblieben und
    /// braucht nur ein anderes Paar.
    ///
    /// Die zweite Prüfung ist deshalb keine Vorsichtsmaßnahme, sondern
    /// die Stelle, an der die Reihenfolge entschieden wird: Der erste
    /// angewandte Vorschlag gilt, der zweite fällt mit Begründung durch.
    pub fn anwenden(
        &self,
        reg: &ParameterRegistry,
        berechtigte: &[Stimmberechtigt],
        jetzt: EpochId,
    ) -> Result<Result<ParameterRegistry, Ablehnungsgrund>, AbstimmungFehler> {
        let auszaehlung = self.zaehle_aus(berechtigte, jetzt)?;
        if let Err(grund) = auszaehlung.ergebnis(self.quorum_promille, self.mehrheit_promille) {
            return Ok(Err(grund));
        }
        let danach = pruefe_vorschlag(reg, &self.vorschlag)
            .map_err(AbstimmungFehler::NichtMehrGueltig)?;
        Ok(Ok(danach))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Wert;

    fn waehler(n: u8) -> MinerId {
        MinerId::new([n; 32])
    }

    /// Ein Berechtigter mit Stake und Arbeit in einer Epoche.
    fn berechtigt(n: u8, stake: u64, epoche: u64, arbeit: u64) -> Stimmberechtigt {
        let mut historie = InferenceHistory::new();
        historie.add_work(epoche, arbeit);
        Stimmberechtigt {
            waehler: waehler(n),
            stake,
            historie,
        }
    }

    /// Ein zulässiger Vorschlag: die Blockzeit von 2 s auf 3 s.
    fn harmloser_vorschlag() -> ParameterVorschlag {
        ParameterVorschlag {
            parameter: Parameter::Blockzeit,
            neuer_wert: Wert::Ganzzahl(3_000),
        }
    }

    fn offene_abstimmung() -> Abstimmung {
        Abstimmung::eroeffne(
            &ParameterRegistry::vorgabe(),
            harmloser_vorschlag(),
            EpochId(100),
        )
        .expect("eröffnen")
    }

    #[test]
    fn ein_vorschlag_mit_verfassungsrang_kommt_nicht_zur_abstimmung() {
        // Punkt 1.2 gilt weiter: Was nicht änderbar ist, wird gar nicht
        // erst zur Abstimmung gestellt, statt nachher abgelehnt zu
        // werden.
        let ergebnis = Abstimmung::eroeffne(
            &ParameterRegistry::vorgabe(),
            ParameterVorschlag {
                parameter: Parameter::DeterminismusPflicht,
                neuer_wert: Wert::Schalter(false),
            },
            EpochId(1),
        );
        assert!(matches!(
            ergebnis,
            Err(VorschlagFehler::Verfassungsrang { .. })
        ));
    }

    #[test]
    fn ein_vorschlag_der_eine_invariante_bricht_kommt_nicht_zur_abstimmung() {
        let ergebnis = Abstimmung::eroeffne(
            &ParameterRegistry::vorgabe(),
            ParameterVorschlag {
                // Die Trainingsrate unter die Inferenzrate zu senken
                // bricht `TrainingsrateNichtUnterInferenzrate`.
                parameter: Parameter::TrainingsStichprobenrate,
                neuer_wert: Wert::Bruch { zaehler: 1, nenner: 100 },
            },
            EpochId(1),
        );
        assert!(matches!(ergebnis, Err(VorschlagFehler::Invariante(_))));
    }

    #[test]
    fn das_fenster_zaehlt_die_eroeffnungsepoche_mit() {
        // Ein Fenster von einer Epoche heißt „diese Epoche", nicht
        // „diese und die nächste".
        let mut reg = ParameterRegistry::vorgabe();
        reg = reg
            .mit(Parameter::Abstimmungsfenster, Wert::Ganzzahl(1))
            .expect("setzen");
        let a = Abstimmung::eroeffne(&reg, harmloser_vorschlag(), EpochId(7)).expect("eröffnen");
        assert_eq!(a.eroeffnet(), EpochId(7));
        assert_eq!(a.frist(), EpochId(7));
        assert_eq!(a.wirksam_ab(), EpochId(8));
    }

    #[test]
    fn wirksam_wird_es_erst_nach_der_frist() {
        // Punkt 2.3. Ein Vorschlag, der mitten in einer Epoche wirksam
        // würde, änderte die Regeln, unter denen sie begonnen hat.
        let a = offene_abstimmung();
        assert_eq!(a.frist(), EpochId(100 + FENSTER_VORGABE - 1));
        assert_eq!(a.wirksam_ab(), EpochId(100 + FENSTER_VORGABE));
        assert!(a.wirksam_ab() > a.frist());
    }

    #[test]
    fn vor_der_eroeffnung_und_nach_der_frist_wird_nicht_gestimmt() {
        let mut a = offene_abstimmung();
        assert!(matches!(
            a.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(99)),
            Err(AbstimmungFehler::NochNichtEroeffnet { .. })
        ));
        assert!(a.stimme_ab(waehler(1), Stimme::Dafuer, a.frist()).is_ok());
        let nach = EpochId(a.frist().0 + 1);
        assert!(matches!(
            a.stimme_ab(waehler(1), Stimme::Dafuer, nach),
            Err(AbstimmungFehler::FristAbgelaufen { .. })
        ));
    }

    #[test]
    fn die_letzte_stimme_zaehlt() {
        // Wer seine Meinung ändert, darf das, solange die Frist läuft.
        let mut a = offene_abstimmung();
        a.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(100)).unwrap();
        a.stimme_ab(waehler(1), Stimme::Dagegen, EpochId(101)).unwrap();
        assert_eq!(a.abgegeben(), 1, "die zweite Stimme kam hinzu statt zu ersetzen");

        let liste = [berechtigt(1, 1_000, 100, 0)];
        let z = a.zaehle_aus(&liste, a.wirksam_ab()).expect("auszählen");
        assert_eq!(z.dafuer, 0);
        assert!(z.dagegen > 0);
    }

    #[test]
    fn vor_dem_fristende_wird_nicht_ausgezaehlt() {
        let a = offene_abstimmung();
        let liste = [berechtigt(1, 1_000, 100, 0)];
        assert!(matches!(
            a.zaehle_aus(&liste, a.frist()),
            Err(AbstimmungFehler::FristLaeuftNoch { .. })
        ));
        assert!(a.zaehle_aus(&liste, a.wirksam_ab()).is_ok());
    }

    #[test]
    fn das_gewicht_kommt_aus_consensus_und_nicht_von_hier() {
        // Das Akzeptanzkriterium der Phase, direkt geprüft: keine
        // zweite Fassung derselben Formel.
        let reg = ParameterRegistry::vorgabe();
        let p = stimmgewichts_parameter(&reg);
        let b = berechtigt(1, 5_000, 42, 3_000_000_000);
        assert_eq!(
            gewicht(&b, EpochId(42), &p),
            calculate_voting_weight_mit(b.stake, &b.historie, 42, &p)
        );
    }

    #[test]
    fn das_gewicht_haengt_seit_2026_09_02_nicht_mehr_an_der_epoche() {
        // ⚑ **Dieser Test hiess `das_gewicht_steht_bei_der_eroeffnung_fest`
        // und ist gegenstandslos geworden.**
        //
        // Gerechnet wurde gegen die Eroeffnungsepoche, damit die
        // Arbeitshistorie nicht waehrend des Fensters zerfaellt und wer
        // frueh stimmt nicht ein anderes Gewicht hat als wer spaet
        // stimmt. Seit „Arbeit qualifiziert, Stake wiegt" ist das
        // Gewicht der Stake, und der zerfaellt nicht.
        //
        // ⚑ **Die Sorge ist nicht verschwunden, sie ist umgezogen.**
        // Sobald die Arbeitsschwelle ueber null steht und die
        // Qualifikation in die Auszaehlung eingeht, kehrt genau dieselbe
        // Frage zurueck: gegen welche Epoche wird qualifiziert? Dann
        // gehoert hier wieder eine Trennung hin. **Solange die
        // Qualifikation nicht verdrahtet ist, waere ein Test darueber
        // eine Behauptung**, und deshalb steht sie hier als benannte
        // Luecke statt als gruener Haken.
        let mut a = offene_abstimmung();
        a.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(100)).unwrap();

        let liste = [berechtigt(1, 1_000, 100, 8_900_000_000)];
        let z = a.zaehle_aus(&liste, a.wirksam_ab()).expect("auszählen");

        let p = stimmgewichts_parameter(&ParameterRegistry::vorgabe());
        let bei_eroeffnung = gewicht(&liste[0], EpochId(100), &p) as u128;
        let bei_auszaehlung = gewicht(&liste[0], a.wirksam_ab(), &p) as u128;

        assert_eq!(z.dafuer, bei_eroeffnung);
        assert_eq!(
            bei_eroeffnung, bei_auszaehlung,
            "das Gewicht darf seit dem 2026-09-02 nicht mehr an der Epoche haengen"
        );
        // Und die Gegenprobe, dass die Zahl ueberhaupt etwas ist: der
        // Stake, und nicht null.
        assert_eq!(bei_eroeffnung, 1_000);
    }

    #[test]
    fn das_quorum_misst_gegen_alle_berechtigten() {
        // Nicht gegen die abgegebenen Stimmen: Sonst erfüllte jede
        // Abstimmung ihr Quorum, an der überhaupt jemand teilnimmt.
        let mut a = offene_abstimmung();
        a.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(100)).unwrap();
        let liste = [
            berechtigt(1, 1_000, 100, 0),
            berechtigt(2, 9_000, 100, 0),
            berechtigt(3, 90_000, 100, 0),
        ];
        let z = a.zaehle_aus(&liste, a.wirksam_ab()).expect("auszählen");
        assert_eq!(z.gesamt, 100_000);
        assert_eq!(z.beteiligung_promille(), 10);
        assert!(matches!(
            z.ergebnis(QUORUM_VORGABE, MEHRHEIT_VORGABE),
            Err(Ablehnungsgrund::QuorumVerfehlt { .. })
        ));
    }

    #[test]
    fn eine_enthaltung_zaehlt_zum_quorum_aber_nicht_zur_mehrheit() {
        // Wer sich enthält, nimmt teil und stimmt nicht zu. Eine
        // Enthaltung ist kein verstecktes Nein.
        let z = Auszaehlung {
            dafuer: 600,
            dagegen: 100,
            enthaltung: 300,
            gesamt: 1_000,
        };
        assert_eq!(z.beteiligung_promille(), 1_000, "die Enthaltung fehlt im Quorum");
        // 600 von 700 entschiedenen Stimmen.
        assert_eq!(z.zustimmung_promille(), 857);
        assert!(z.ergebnis(QUORUM_VORGABE, MEHRHEIT_VORGABE).is_ok());

        // Zum Vergleich: als Nein gezählt wären es 600 von 1000.
        let als_nein = Auszaehlung {
            dafuer: 600,
            dagegen: 400,
            enthaltung: 0,
            gesamt: 1_000,
        };
        assert_eq!(als_nein.zustimmung_promille(), 600);
    }

    #[test]
    fn bei_gleichstand_ist_der_vorschlag_abgelehnt() {
        // Überschreiten, nicht erreichen: Bei einer Schwelle von 500
        // Promille wäre die Hälfte sonst genug.
        let z = Auszaehlung {
            dafuer: 500,
            dagegen: 500,
            enthaltung: 0,
            gesamt: 1_000,
        };
        assert_eq!(z.zustimmung_promille(), 500);
        assert!(matches!(
            z.ergebnis(QUORUM_VORGABE, 500),
            Err(Ablehnungsgrund::MehrheitVerfehlt { .. })
        ));
        // Eine Stimme mehr genügt.
        let knapp = Auszaehlung {
            dafuer: 501,
            dagegen: 499,
            enthaltung: 0,
            gesamt: 1_000,
        };
        assert!(knapp.ergebnis(QUORUM_VORGABE, 500).is_ok());
    }

    #[test]
    fn eine_leere_abstimmung_nimmt_nichts_an() {
        let z = Auszaehlung {
            dafuer: 0,
            dagegen: 0,
            enthaltung: 0,
            gesamt: 1_000,
        };
        assert_eq!(z.zustimmung_promille(), 0);
        assert!(z.ergebnis(1, 500).is_err());
    }

    #[test]
    fn ein_doppelt_gefuehrter_berechtigter_faellt_auf() {
        // Sonst zählte sein Gewicht doppelt im Nenner des Quorums.
        let a = offene_abstimmung();
        let liste = [berechtigt(1, 1_000, 100, 0), berechtigt(1, 2_000, 100, 0)];
        assert!(matches!(
            a.zaehle_aus(&liste, a.wirksam_ab()),
            Err(AbstimmungFehler::DoppeltBerechtigt { .. })
        ));
    }

    #[test]
    fn eine_stimme_von_ausserhalb_faellt_auf() {
        // Kein stilles Übergehen: Wer sie überginge, zählte eine
        // Abstimmung aus, deren Eingaben er nicht versteht.
        let mut a = offene_abstimmung();
        a.stimme_ab(waehler(9), Stimme::Dafuer, EpochId(100)).unwrap();
        let liste = [berechtigt(1, 1_000, 100, 0)];
        assert!(matches!(
            a.zaehle_aus(&liste, a.wirksam_ab()),
            Err(AbstimmungFehler::NichtStimmberechtigt { .. })
        ));
    }

    #[test]
    fn ein_angenommener_vorschlag_wird_beim_anwenden_erneut_geprueft() {
        // ⚑ Zwei Vorschläge, jeder für sich zulässig, zusammen ein
        // Bruch. Beide werden gegen die Registry geprüft, wie sie bei
        // ihrer Eröffnung aussah; die zweite Prüfung entscheidet die
        // Reihenfolge.
        let reg = ParameterRegistry::vorgabe();

        // ⚑ Bis zum 2026-09-02 lief dieser Test über den
        // Kontrollsegment-Vorrat gegen das Fenster. Beide Parameter sind
        // mit den Kontrollsegmenten entfallen; die **Eigenschaft**, die
        // der Test zeigt, braucht nur ein anderes gekoppeltes Paar.
        //
        // A: Trainingsrate runter auf genau die Inferenzrate (5/100).
        // Gerade noch zulässig, denn verlangt ist „nicht darunter".
        let a_vorschlag = ParameterVorschlag {
            parameter: Parameter::TrainingsStichprobenrate,
            neuer_wert: Wert::Bruch { zaehler: 5, nenner: 100 },
        };
        // B: Inferenzrate hoch auf 6/100. Gegen die **heutige**
        // Trainingsrate von 10/100 ebenfalls zulässig.
        let b_vorschlag = ParameterVorschlag {
            parameter: Parameter::Stichprobenrate,
            neuer_wert: Wert::Bruch { zaehler: 6, nenner: 100 },
        };

        let mut a = Abstimmung::eroeffne(&reg, a_vorschlag, EpochId(10)).expect("A eröffnen");
        let mut b = Abstimmung::eroeffne(&reg, b_vorschlag, EpochId(10)).expect("B eröffnen");

        let liste = [berechtigt(1, 1_000, 10, 0)];
        a.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(10)).unwrap();
        b.stimme_ab(waehler(1), Stimme::Dafuer, EpochId(10)).unwrap();
        let jetzt = a.wirksam_ab();

        // A geht durch und ändert die Registry.
        let nach_a = a
            .anwenden(&reg, &liste, jetzt)
            .expect("A auszählen")
            .expect("A angenommen");

        // B hat dieselbe Mehrheit und fällt trotzdem durch, weil die
        // Welt sich geändert hat.
        let ergebnis = b.anwenden(&nach_a, &liste, jetzt);
        assert!(
            matches!(ergebnis, Err(AbstimmungFehler::NichtMehrGueltig(_))),
            "B wurde angewandt, obwohl die Invariante jetzt bricht: {ergebnis:?}"
        );

        // Gegenprobe: Ohne A wäre B durchgegangen. Sonst hieße der
        // Nachweis oben nur, dass B nie zulässig war.
        assert!(b.anwenden(&reg, &liste, jetzt).expect("B auszählen").is_ok());
    }

    #[test]
    fn ein_abgelehnter_vorschlag_aendert_nichts() {
        let mut a = offene_abstimmung();
        a.stimme_ab(waehler(1), Stimme::Dagegen, EpochId(100)).unwrap();
        let liste = [berechtigt(1, 1_000, 100, 0)];
        let reg = ParameterRegistry::vorgabe();
        let ergebnis = a
            .anwenden(&reg, &liste, a.wirksam_ab())
            .expect("auszählen");
        assert!(matches!(
            ergebnis,
            Err(Ablehnungsgrund::MehrheitVerfehlt { .. })
        ));
    }

    #[test]
    fn jeder_fehler_sagt_was_geschehen_ist() {
        let faelle = [
            AbstimmungFehler::FristAbgelaufen {
                jetzt: EpochId(5),
                frist: EpochId(4),
            },
            AbstimmungFehler::NochNichtEroeffnet {
                jetzt: EpochId(1),
                eroeffnet: EpochId(2),
            },
            AbstimmungFehler::FristLaeuftNoch {
                jetzt: EpochId(3),
                frist: EpochId(4),
            },
            AbstimmungFehler::NichtStimmberechtigt { waehler: waehler(1) },
            AbstimmungFehler::DoppeltBerechtigt { waehler: waehler(1) },
        ];
        for fall in faelle {
            let text = fall.to_string();
            assert!(text.len() > 20, "zu knapp: {text}");
            assert!(!text.ends_with(' '));
        }
    }
}
