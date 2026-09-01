//! Anlaufanreiz: die Subventionsrate als Plan statt als Zahl (Punkte 5.4 bis 5.6).
//!
//! # Was festgelegt wurde, und wie ausdrücklich nicht
//!
//! **Festlegung des Projektinhabers vom 2026-08-25:** Die ersten Miner
//! sollen höhere Mengen MYL erhalten, um frühes Wachstum anzureizen.
//!
//! ⚑ **Nicht über eine höhere Genesis-Menge.** [`crate::genesis`] nimmt
//! Arbeitsnachweise und sonst nichts: kein Parameter für
//! Sonderzuteilungen, kein Rest, über den jemand verfügen könnte. Diese
//! Eigenschaft ist durch die **Form der Funktion** durchgesetzt und
//! nicht durch eine Prüfung, und sie ist zu wertvoll, um sie für einen
//! Anreiz aufzugeben, den es auch anders gibt.
//!
//! **Sondern über die Subventionsrate `s`.** Sie ist bereits ein
//! Governance-Parameter und bereits nach oben begrenzt durch die
//! Self-Dealing-Invariante. Der Anreiz hatte damit schon einen Ort und
//! schon eine Obergrenze; **was fehlte, war der Verlauf.**
//!
//! # ⚑ Der Widerspruch zu Anhang B.8.4, ausdrücklich benannt
//!
//! B.8.4 misst: Bei einer Halbierung der jährlichen Prägung entfallen
//! rund **28 %** der Fünfjahresemission auf das erste Jahr, und zwar
//! unabhängig davon, ob das Netz auf 500, 5.000 oder 50.000 Miner
//! wächst. Kap. 5.7 zieht daraus den Schluss, das Gegenmittel gegen
//! Frühphasen-Konzentration sei „eine flache Subventionskurve".
//!
//! **Eine steilere Kurve erhöht diesen Anteil. Das ist keine
//! Nebenwirkung, sondern der Zweck:** Der Vorteil früher Teilnahme *ist*
//! der Anreiz.
//!
//! Deshalb liefert [`Subventionsplan::erstjahresanteil`] die Zahl **mit**,
//! statt sie der Herleitung zu überlassen. Wer den Plan ändert, sieht
//! sofort, was er an der Konzentration ändert. **Ein Abwägen, das man
//! sehen kann, ist der Unterschied zwischen einer Entscheidung und einem
//! Nebeneffekt.**
//!
//! # ⚑ Die Annahme gehört dem Aufrufer
//!
//! Der Anteil hängt daran, wie sich die Prägung ohne Subvention
//! entwickelt, und **das weiß dieses Modul nicht**. Es rechnet deshalb
//! nicht mit einer eingebauten Abklingkurve, sondern über eine
//! Basisreihe, die der Aufrufer mitbringt. Wer die Reihe wählt, wählt
//! die Annahme und trägt sie; eine eingebaute Annahme sähe aus wie eine
//! Messung. [`basis_halbierung_je_jahr`] liefert die Reihe aus B.8.4,
//! damit die dortige Zahl nachrechenbar bleibt.

use crate::sicherheit::{self_dealing_sicher_konservativ, SicherheitsFehler};

/// Ein Abschnitt des Plans: ab welcher Epoche welche Rate gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planabschnitt {
    /// Ab dieser Epoche gilt der Abschnitt, einschließlich.
    pub ab_epoche: u64,
    /// Zähler der Subventionsrate `s`.
    pub zaehler: u64,
    /// Nenner der Subventionsrate `s`.
    pub nenner: u64,
}

/// Warum ein Plan nicht angenommen wird.
///
/// ⚑ **Abgelehnt und nicht gewarnt.** Ein Plan, der die
/// Self-Dealing-Schranke an irgendeinem Punkt überschreitet, ist keine
/// Vorlage mit Mangel, sondern eine Anleitung zum Angriff: Ab dort lohnt
/// es sich, sich selbst Arbeit zu geben. Eine Warnung, die man
/// wegklicken kann, wäre keine Schranke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planfehler {
    /// Ein Plan ohne Abschnitte ist kein Plan.
    Leer,
    /// Der erste Abschnitt beginnt nicht bei Epoche null.
    ///
    /// Sonst gäbe es Epochen ohne Rate, und was dann gälte, stünde
    /// nirgends.
    ErsterNichtBeiNull,
    /// Die Abschnitte stehen nicht in aufsteigender Epochenfolge.
    NichtAufsteigend {
        /// Der Abschnitt, der aus der Reihe fällt.
        bei: usize,
    },
    /// Ein Abschnitt hat den Nenner null.
    UnbrauchbarerBruch {
        /// Der betroffene Abschnitt.
        bei: usize,
    },
    /// Die Rate steigt gegenüber dem vorigen Abschnitt.
    ///
    /// ⚑ **Ein Anlaufanreiz, der später steigt, ist keiner.** Er
    /// belohnte dann das Warten, also genau das Gegenteil dessen, wofür
    /// er da ist.
    Steigend {
        /// Der Abschnitt, in dem sie steigt.
        bei: usize,
    },
    /// Ein Abschnitt liegt über der Self-Dealing-Schranke.
    UeberDerSchranke {
        /// Der betroffene Abschnitt.
        bei: usize,
    },
    /// Die Schrankenprüfung selbst ging nicht durch.
    Schranke(SicherheitsFehler),
}

impl std::fmt::Display for Planfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leer => f.write_str("ein Plan ohne Abschnitte ist kein Plan"),
            Self::ErsterNichtBeiNull => {
                f.write_str("der erste Abschnitt beginnt nicht bei Epoche null")
            }
            Self::NichtAufsteigend { bei } => {
                write!(f, "Abschnitt {bei} faellt aus der Epochenfolge")
            }
            Self::UnbrauchbarerBruch { bei } => write!(f, "Abschnitt {bei} hat den Nenner null"),
            Self::Steigend { bei } => write!(f, "in Abschnitt {bei} steigt die Rate"),
            Self::UeberDerSchranke { bei } => {
                write!(f, "Abschnitt {bei} liegt ueber der Self-Dealing-Schranke")
            }
            Self::Schranke(e) => write!(f, "Schrankenpruefung: {e}"),
        }
    }
}

impl std::error::Error for Planfehler {}

/// Wie sich die Emission auf das erste Jahr verteilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Erstjahresanteil {
    /// Der Anteil in Basispunkten (10.000 = alles).
    pub bps: u64,
    /// Die Emission des ersten Jahres, exakt.
    pub erstes_jahr: u128,
    /// Die Emission über den ganzen betrachteten Zeitraum, exakt.
    pub gesamt: u128,
}

/// Ein geprüfter Subventionsplan.
///
/// Die Abschnitte sind privat: Ein Plan, der die Prüfung von
/// [`Self::neu`] nicht bestanden hat, existiert nicht. Dieselbe Bauart
/// wie beim Beschluss in der Ausfallsicherung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subventionsplan {
    abschnitte: Vec<Planabschnitt>,
}

impl Subventionsplan {
    /// Prüft einen Plan und nimmt ihn an oder lehnt ihn ab (Punkt 5.5).
    ///
    /// Geprüft wird in dieser Reihenfolge: nicht leer, Beginn bei null,
    /// aufsteigende Epochen, brauchbare Brüche, **monoton nicht
    /// steigend**, und **jeder Punkt unter der Self-Dealing-Schranke**.
    ///
    /// Die Schranke wird über [`self_dealing_sicher_konservativ`]
    /// geprüft, also gegen den konservativen Kostenanteil aus
    /// Entscheidung B1. **Konservativ heißt hier: gegen die Annahme, die
    /// dem Angreifer am meisten nützt.**
    pub fn neu(abschnitte: Vec<Planabschnitt>) -> Result<Self, Planfehler> {
        if abschnitte.is_empty() {
            return Err(Planfehler::Leer);
        }
        if abschnitte[0].ab_epoche != 0 {
            return Err(Planfehler::ErsterNichtBeiNull);
        }
        for (i, a) in abschnitte.iter().enumerate() {
            if a.nenner == 0 {
                return Err(Planfehler::UnbrauchbarerBruch { bei: i });
            }
            if i > 0 {
                let vor = &abschnitte[i - 1];
                if a.ab_epoche <= vor.ab_epoche {
                    return Err(Planfehler::NichtAufsteigend { bei: i });
                }
                // s_vor >= s_jetzt, kreuzweise multipliziert.
                let links = (vor.zaehler as u128) * (a.nenner as u128);
                let rechts = (a.zaehler as u128) * (vor.nenner as u128);
                if links < rechts {
                    return Err(Planfehler::Steigend { bei: i });
                }
            }
            // ⚑ Je Abschnitt geprüft, obwohl der erste entscheidet: Weil
            // der Plan nicht steigen darf, ist er der größte, und alle
            // übrigen liegen darunter. Die Prüfung bleibt trotzdem hier,
            // damit sie nicht still aufhört zu wirken, falls die
            // Monotonie je gelockert wird. Sie kostet nichts.
            match self_dealing_sicher_konservativ(a.zaehler, a.nenner) {
                Err(e) => return Err(Planfehler::Schranke(e)),
                Ok(false) => return Err(Planfehler::UeberDerSchranke { bei: i }),
                Ok(true) => {}
            }
        }
        Ok(Self { abschnitte })
    }

    /// Die Rate, die in dieser Epoche gilt.
    ///
    /// Der letzte Abschnitt, dessen Beginn nicht in der Zukunft liegt.
    /// Ein Plan beginnt bei Epoche null, also gibt es immer einen.
    pub fn rate(&self, epoche: u64) -> (u64, u64) {
        let a = self
            .abschnitte
            .iter()
            .rev()
            .find(|a| a.ab_epoche <= epoche)
            .expect("der erste Abschnitt beginnt bei null");
        (a.zaehler, a.nenner)
    }

    /// Die Abschnitte, zum Nachlesen.
    pub fn abschnitte(&self) -> &[Planabschnitt] {
        &self.abschnitte
    }

    /// Wie viel der Emission auf das erste Jahr entfällt (Punkt 5.6).
    ///
    /// `basis_je_epoche` ist die angenommene Prägung **ohne** Subvention,
    /// je Epoche und in der Reihenfolge der Epochen; `epochen_je_jahr`
    /// sagt, wo das erste Jahr endet. Die tatsächliche Emission einer
    /// Epoche ist `basis · (1 + s)`, gerechnet als
    /// `basis · (nenner + zaehler) / nenner`.
    ///
    /// # ⚑ Die Annahme steckt in der Basisreihe, nicht hier
    ///
    /// Dieses Modul kennt den Verlauf der Prägung nicht und erfindet ihn
    /// nicht. Wer die Reihe wählt, wählt die Annahme. Eine eingebaute
    /// Abklingkurve sähe aus wie eine Messung und wäre eine Setzung.
    ///
    /// Bei leerer Basisreihe ist der Anteil null und beide Summen sind
    /// null: Wo nichts geprägt wird, entfällt auf das erste Jahr nichts.
    pub fn erstjahresanteil(
        &self,
        basis_je_epoche: &[u64],
        epochen_je_jahr: u64,
    ) -> Erstjahresanteil {
        let mut erstes_jahr: u128 = 0;
        let mut gesamt: u128 = 0;
        for (e, basis) in basis_je_epoche.iter().enumerate() {
            let (z, n) = self.rate(e as u64);
            // basis · (1 + s), abgerundet wie mint_amount.
            let emission = (*basis as u128) * (n as u128 + z as u128) / n as u128;
            gesamt += emission;
            if (e as u64) < epochen_je_jahr {
                erstes_jahr += emission;
            }
        }
        // Null geteilt durch null ist kein Anteil, sondern eine leere
        // Reihe; `checked_div` sagt das, ohne dass daneben eine zweite
        // Prüfung derselben Bedingung steht.
        let bps = (erstes_jahr * 10_000).checked_div(gesamt).unwrap_or(0) as u64;
        Erstjahresanteil {
            bps,
            erstes_jahr,
            gesamt,
        }
    }
}

/// Die Basisreihe aus Anhang B.8.4: Halbierung der Prägung je Jahr.
///
/// `start` ist die Prägung je Epoche im ersten Jahr; jedes weitere Jahr
/// halbiert sie sich. **Damit ist die dortige Zahl nachrechenbar**, und
/// wer eine andere Annahme braucht, baut seine eigene Reihe.
pub fn basis_halbierung_je_jahr(start: u64, epochen_je_jahr: u64, jahre: u64) -> Vec<u64> {
    let mut out = Vec::with_capacity((epochen_je_jahr * jahre) as usize);
    for jahr in 0..jahre {
        let wert = start >> jahr.min(63);
        for _ in 0..epochen_je_jahr {
            out.push(wert);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abschnitt(ab: u64, z: u64, n: u64) -> Planabschnitt {
        Planabschnitt {
            ab_epoche: ab,
            zaehler: z,
            nenner: n,
        }
    }

    /// Ein fallender Plan wird angenommen, und die Rate folgt der Epoche.
    #[test]
    fn ein_fallender_plan_gilt_abschnittsweise() {
        let p = Subventionsplan::neu(vec![
            abschnitt(0, 5, 4),   // s = 1,25
            abschnitt(100, 1, 2), // s = 0,5
            abschnitt(200, 0, 1), // s = 0
        ])
        .expect("Plan");
        assert_eq!(p.rate(0), (5, 4));
        assert_eq!(p.rate(99), (5, 4));
        assert_eq!(p.rate(100), (1, 2));
        assert_eq!(p.rate(199), (1, 2));
        assert_eq!(p.rate(200), (0, 1));
        assert_eq!(p.rate(u64::MAX), (0, 1));
    }

    /// Ein Plan aus einem einzigen Abschnitt ist der bisherige Zustand:
    /// eine Rate für immer.
    #[test]
    fn ein_abschnitt_ist_der_bisherige_zustand() {
        let p = Subventionsplan::neu(vec![abschnitt(0, 1, 2)]).expect("Plan");
        assert_eq!(p.rate(0), (1, 2));
        assert_eq!(p.rate(1_000_000), (1, 2));
    }

    /// ⚑ **Ein Anlaufanreiz, der später steigt, ist keiner.**
    #[test]
    fn ein_steigender_plan_wird_abgelehnt() {
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 1, 2), abschnitt(100, 5, 4)]),
            Err(Planfehler::Steigend { bei: 1 })
        );
    }

    /// Gleich bleiben darf er.
    #[test]
    fn ein_gleichbleibender_plan_geht_durch() {
        assert!(Subventionsplan::neu(vec![abschnitt(0, 1, 2), abschnitt(100, 2, 4)]).is_ok());
    }

    /// ⚑ **Über der Self-Dealing-Schranke wird abgelehnt, nicht
    /// gewarnt.** Konservativ geprüft heißt `s < 1,5`.
    #[test]
    fn ueber_der_schranke_wird_abgelehnt() {
        // s = 1,5 ist die Schranke selbst und liegt nicht darunter.
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 3, 2)]),
            Err(Planfehler::UeberDerSchranke { bei: 0 })
        );
        // Und knapp darunter geht durch.
        assert!(Subventionsplan::neu(vec![abschnitt(0, 149, 100)]).is_ok());
    }

    /// ⛑ **Der erste Abschnitt entscheidet, und das ist keine Lücke.**
    ///
    /// Dieser Test hieß zuerst „auch ein späterer Abschnitt wird gegen
    /// die Schranke geprüft" und prüfte den **ersten**. Der Name log,
    /// und beim Nachsehen war der Grund interessanter als der Fehler:
    /// Weil der Plan monoton nicht steigend sein muss, ist der erste
    /// Abschnitt immer der größte. **Liegt er unter der Schranke, liegen
    /// alle darunter**, und ein Plan, in dem ein späterer darüber läge,
    /// scheitert vorher an `Steigend`.
    ///
    /// Die Prüfung je Abschnitt bleibt trotzdem stehen, siehe die
    /// Begründung an der Schleife.
    #[test]
    fn der_erste_abschnitt_entscheidet_ueber_die_schranke() {
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 2, 1), abschnitt(50, 1, 1)]),
            Err(Planfehler::UeberDerSchranke { bei: 0 })
        );
        // Und ein Plan, dessen späterer Abschnitt darüber läge, ist
        // notwendig steigend und scheitert daran zuerst.
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 1, 1), abschnitt(50, 2, 1)]),
            Err(Planfehler::Steigend { bei: 1 })
        );
    }

    /// Ein Plan ohne Abschnitte ist kein Plan.
    #[test]
    fn ein_leerer_plan_wird_abgelehnt() {
        assert_eq!(Subventionsplan::neu(vec![]), Err(Planfehler::Leer));
    }

    /// ⚑ Beginnt der Plan nicht bei null, gäbe es Epochen ohne Rate.
    #[test]
    fn ein_plan_ohne_anfang_wird_abgelehnt() {
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(10, 1, 2)]),
            Err(Planfehler::ErsterNichtBeiNull)
        );
    }

    /// Abschnitte in falscher Reihenfolge werden abgelehnt.
    #[test]
    fn eine_falsche_reihenfolge_wird_abgelehnt() {
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 1, 1), abschnitt(0, 1, 2)]),
            Err(Planfehler::NichtAufsteigend { bei: 1 })
        );
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 1, 1), abschnitt(50, 1, 2), abschnitt(20, 1, 4)]),
            Err(Planfehler::NichtAufsteigend { bei: 2 })
        );
    }

    /// Ein Nenner von null wird benannt statt zu stürzen.
    #[test]
    fn ein_nenner_von_null_wird_benannt() {
        assert_eq!(
            Subventionsplan::neu(vec![abschnitt(0, 1, 0)]),
            Err(Planfehler::UnbrauchbarerBruch { bei: 0 })
        );
    }

    /// Ohne Subvention ist die Emission die Basis.
    #[test]
    fn ohne_subvention_ist_die_emission_die_basis() {
        let p = Subventionsplan::neu(vec![abschnitt(0, 0, 1)]).expect("Plan");
        let basis = vec![100u64; 20];
        let a = p.erstjahresanteil(&basis, 10);
        assert_eq!(a.gesamt, 2_000);
        assert_eq!(a.erstes_jahr, 1_000);
        assert_eq!(a.bps, 5_000, "bei gleicher Basis die Haelfte");
    }

    /// ⚑ **Der Kern von Punkt 5.6: Eine steilere Kurve erhöht den
    /// Erstjahresanteil, und die Zahl kommt mit.**
    #[test]
    fn eine_steilere_kurve_erhoeht_den_erstjahresanteil() {
        let basis = vec![100u64; 20];
        let flach = Subventionsplan::neu(vec![abschnitt(0, 1, 2)]).expect("flach");
        let steil = Subventionsplan::neu(vec![abschnitt(0, 1, 1), abschnitt(10, 0, 1)])
            .expect("steil");
        let a = flach.erstjahresanteil(&basis, 10);
        let b = steil.erstjahresanteil(&basis, 10);
        assert_eq!(a.bps, 5_000, "eine flache Kurve verschiebt nichts");
        assert!(
            b.bps > a.bps,
            "die steilere Kurve verschob nichts: {} gegen {}",
            b.bps,
            a.bps
        );
    }

    /// ⚑ **Anhang B.8.4 nachgerechnet:** Bei Halbierung je Jahr und
    /// ohne Subvention entfallen rund 52 % der Fünfjahresemission auf
    /// das erste Jahr. Die im Papier genannten 28 % beziehen sich auf
    /// eine andere Größe; hier steht die Reihe, die dieses Modul
    /// benutzt, damit der Unterschied sichtbar ist statt verwischt.
    #[test]
    fn die_basisreihe_aus_b_8_4_ist_nachrechenbar() {
        let basis = basis_halbierung_je_jahr(1_000, 100, 5);
        assert_eq!(basis.len(), 500);
        assert_eq!(basis[0], 1_000);
        assert_eq!(basis[100], 500);
        assert_eq!(basis[400], 62);
        let ohne = Subventionsplan::neu(vec![abschnitt(0, 0, 1)]).expect("Plan");
        let a = ohne.erstjahresanteil(&basis, 100);
        // 1000 / (1000+500+250+125+62) = 51,7 %
        assert!(
            (5_100..=5_250).contains(&a.bps),
            "unerwarteter Anteil: {}",
            a.bps
        );
    }

    /// Eine leere Basisreihe ergibt null und stürzt nicht.
    #[test]
    fn eine_leere_basisreihe_ergibt_null() {
        let p = Subventionsplan::neu(vec![abschnitt(0, 1, 2)]).expect("Plan");
        let a = p.erstjahresanteil(&[], 10);
        assert_eq!(a.bps, 0);
        assert_eq!(a.gesamt, 0);
        assert_eq!(a.erstes_jahr, 0);
    }

    /// Die Emission je Epoche ist `basis · (1 + s)`, abgerundet.
    #[test]
    fn die_emission_folgt_der_rate() {
        let p = Subventionsplan::neu(vec![abschnitt(0, 1, 2)]).expect("Plan");
        let a = p.erstjahresanteil(&[100], 1);
        assert_eq!(a.gesamt, 150);
        let ungerade = p.erstjahresanteil(&[101], 1);
        assert_eq!(ungerade.gesamt, 151, "es wurde nicht abgerundet");
    }
}
