//! Parametervorschläge und ihre Prüfung (Punkte 1.2 und 1.3).
//!
//! Ein Vorschlag durchläuft **drei Schranken, bevor überhaupt abgestimmt
//! wird**, und die Reihenfolge ist Absicht:
//!
//! 1. **Rang** (Punkt 1.2). Ein Verfassungsrang-Parameter ist kein
//!    Gegenstand einer Abstimmung. Das wird hier entschieden und nicht in
//!    einer Oberfläche: Kap. 10.3 nennt drei nicht änderbare
//!    Festlegungen, und eine Regel, die nur in der Oberfläche steht, gilt
//!    für jeden, der die Oberfläche nicht benutzt.
//! 2. **Art**. Ein Vorschlag, der aus einer Rate einen Schalter macht,
//!    ist keine Parameteränderung, sondern eine Protokolländerung.
//! 3. **Invarianten** (Punkt 1.3), geprüft am **entstehenden Zustand**.
//!
//! ## Warum die Prüfung vor der Abstimmung steht
//!
//! Ein Parametersatz, der `S_min` unterschreitet, ist auch dann falsch,
//! wenn eine Mehrheit dafür stimmt. Käme die Prüfung danach, gäbe es
//! genau zwei Möglichkeiten: das Ergebnis zu verwerfen, was die
//! Abstimmung entwertet, oder es anzuwenden, was die Invariante
//! entwertet. Davor ist die einzige Stelle, an der beides erhalten
//! bleibt.

use crate::invarianten::{pruefe_invarianten, InvariantenBruch};
use crate::registry::{
    Aenderbarkeit, Parameter, ParameterRegistry, RegistryFehler, Wert,
};

/// Ein Vorschlag, einen Parameter zu ändern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterVorschlag {
    /// Welcher Parameter geändert werden soll.
    pub parameter: Parameter,
    /// Auf welchen Wert.
    pub neuer_wert: Wert,
}

/// Warum ein Vorschlag nicht zur Abstimmung kommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VorschlagFehler {
    /// Der Parameter hat Verfassungsrang (Kap. 10.3).
    Verfassungsrang { parameter: Parameter },
    /// Der neue Wert hat die falsche Art.
    Art(RegistryFehler),
    /// Der entstehende Zustand verletzt eine Sicherheitsbedingung.
    Invariante(InvariantenBruch),
    /// ⚑ **Der Verfahrenswechsel geht nur einen Schritt nach vorn.**
    ///
    /// Nicht als Invariante geführt, und das ist Absicht: Alle drei
    /// Stufen sind **gültige Zustände**. Verboten ist nicht die
    /// Stellung, sondern der Weg dorthin. Eine Invariante prüft einen
    /// Zustand und könnte das gar nicht sehen.
    VerfahrenswechselUnzulaessig { von: u64, nach: u64 },
}

impl std::fmt::Display for VorschlagFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verfassungsrang { parameter } => write!(
                f,
                "{} hat Verfassungsrang (Kap. 10.3) und ist nicht Gegenstand \
                 einer Abstimmung",
                parameter.name()
            ),
            Self::Art(e) => write!(f, "{}", e),
            Self::Invariante(b) => write!(f, "Sicherheitsbedingung verletzt: {}", b),
            Self::VerfahrenswechselUnzulaessig { von, nach } => write!(
                f,
                "Wechsel der Signaturstufe von {} auf {} ist nicht zulässig: \
                 erlaubt ist genau ein Schritt nach vorn (0 auf 1, 1 auf 2)",
                von, nach
            ),
        }
    }
}

impl std::error::Error for VorschlagFehler {}

/// Prüft einen Vorschlag gegen Rang, Art und Invarianten.
///
/// **Returns:** die Registry, wie sie **nach** Annahme aussähe. Der
/// Rückgabewert ist der Zustand, gegen den geprüft wurde, und nicht eine
/// zweite Rechnung: Wer den Vorschlag später anwendet, muss genau diesen
/// Zustand herstellen.
/// Prüft den Übergang der [`Parameter::Signaturstufe`].
///
/// ⚑ **Zwei Wege sind gesperrt, und beide würden das Netz kosten:**
///
/// - **Der Sprung** von „nur klassisch" auf „nur quantensicher" macht
///   jeden Validator ungültig, der seinen zweiten Schlüssel noch nicht
///   veröffentlicht hat. Das Netz hielte an, und zwar in dem Augenblick,
///   in dem der Vorschlag wirksam wird.
/// - **Der Rückweg** öffnet ein Verfahren wieder, das man gerade
///   verlassen hat, und man verlässt es nur aus einem Grund. Wer
///   BLS12-381 gebrochen hat, will genau diesen Vorschlag.
///
/// **Was hier nicht geprüft wird und trotzdem gilt:** ob alle
/// Validatoren bereit sind. Das sieht nur der Konsens
/// (`myl_consensus::validator::alle_bereit_fuer`); die Registry kennt
/// Parameter und keine Validatoren. Die Trennung ist gewollt, und sie
/// gehört bei jeder Umstellung mitgeprüft.
fn pruefe_verfahrenswechsel(
    vorher: &ParameterRegistry,
    nachher: &ParameterRegistry,
) -> Result<(), VorschlagFehler> {
    let lies = |reg: &ParameterRegistry| -> u64 {
        reg.wert(Parameter::Signaturstufe).als_ganzzahl().unwrap_or(0)
    };
    let (von, nach) = (lies(vorher), lies(nachher));
    let stufen = (
        myl_types::pq::Signaturstufe::aus_zahl(von),
        myl_types::pq::Signaturstufe::aus_zahl(nach),
    );
    match stufen {
        (Some(a), Some(b)) if myl_types::pq::Signaturstufe::uebergang_erlaubt(a, b) => Ok(()),
        _ => Err(VorschlagFehler::VerfahrenswechselUnzulaessig { von, nach }),
    }
}

pub fn pruefe_vorschlag(
    reg: &ParameterRegistry,
    v: &ParameterVorschlag,
) -> Result<ParameterRegistry, VorschlagFehler> {
    // 1. Rang.
    if v.parameter.rang() == Aenderbarkeit::Verfassungsrang {
        return Err(VorschlagFehler::Verfassungsrang {
            parameter: v.parameter,
        });
    }

    // 2. Art.
    let danach = reg
        .mit(v.parameter, v.neuer_wert.clone())
        .map_err(VorschlagFehler::Art)?;

    // 3. Der Verfahrenswechsel, falls er gemeint ist.
    //
    // ⚑ **Vor den Invarianten und getrennt von ihnen**, weil hier ein
    // Übergang geprüft wird und keine Eigenschaft eines Zustands. Die
    // Invarianten kennen nur `danach`; der Weg dorthin ist ihnen nicht
    // sichtbar.
    if v.parameter == Parameter::Signaturstufe {
        pruefe_verfahrenswechsel(reg, &danach)?;
    }

    // 4. Invarianten, am entstehenden Zustand.
    pruefe_invarianten(&danach).map_err(VorschlagFehler::Invariante)?;

    Ok(danach)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Wert;
    use myl_types::pq::Signaturstufe;

    fn auf(stufe: u64) -> ParameterRegistry {
        ParameterRegistry::vorgabe()
            .mit(Parameter::Signaturstufe, Wert::Ganzzahl(stufe))
            .expect("Stufe setzen")
    }

    fn vorschlag(stufe: u64) -> ParameterVorschlag {
        ParameterVorschlag {
            parameter: Parameter::Signaturstufe,
            neuer_wert: Wert::Ganzzahl(stufe),
        }
    }

    /// Die beiden erlaubten Schritte gehen durch.
    ///
    /// Ohne diese Hälfte bewiesen die Sperren unten nur, dass gar nichts
    /// durchkommt.
    #[test]
    fn ein_schritt_nach_vorn_geht_durch() {
        assert!(pruefe_vorschlag(&auf(0), &vorschlag(1)).is_ok());
        assert!(pruefe_vorschlag(&auf(1), &vorschlag(2)).is_ok());
    }

    /// ⚑ **Der Sprung über das Fenster hinweg ist gesperrt.**
    ///
    /// Er machte jeden Validator ungültig, der seinen zweiten Schlüssel
    /// noch nicht veröffentlicht hat, und hielte damit die Kette an, in
    /// dem Augenblick, in dem er wirksam wird.
    #[test]
    fn der_sprung_ueber_das_fenster_wird_abgelehnt() {
        assert!(matches!(
            pruefe_vorschlag(&auf(0), &vorschlag(2)),
            Err(VorschlagFehler::VerfahrenswechselUnzulaessig { von: 0, nach: 2 })
        ));
    }

    /// ⚑ **Der Rückweg ist gesperrt, und er wäre der Angriff.**
    ///
    /// Man verlässt ein Signaturverfahren aus genau einem Grund. Wer
    /// BLS12-381 gebrochen hat, will genau diesen Vorschlag.
    #[test]
    fn der_rueckweg_wird_abgelehnt() {
        for (von, nach) in [(2u64, 1u64), (2, 0), (1, 0)] {
            assert!(
                matches!(
                    pruefe_vorschlag(&auf(von), &vorschlag(nach)),
                    Err(VorschlagFehler::VerfahrenswechselUnzulaessig { .. })
                ),
                "der Rueckweg {von} auf {nach} ging durch"
            );
        }
    }

    /// Stillstand ist keine Änderung. Ohne diese Sperre könnte ein
    /// wirkungsloser Vorschlag als Übergang gelten und ein Fenster
    /// stillschweigend verlängern.
    #[test]
    fn gleich_auf_gleich_wird_abgelehnt() {
        for s in [0u64, 1, 2] {
            assert!(pruefe_vorschlag(&auf(s), &vorschlag(s)).is_err());
        }
    }

    /// Eine Zahl, die keine Stufe ist, wird abgelehnt.
    #[test]
    fn eine_zahl_ausserhalb_der_stufen_wird_abgelehnt() {
        assert_eq!(Signaturstufe::aus_zahl(3), None);
        assert!(pruefe_vorschlag(&auf(2), &vorschlag(3)).is_err());
        assert!(pruefe_vorschlag(&auf(0), &vorschlag(u64::MAX)).is_err());
    }

    /// Der Genesis-Zustand steht auf „nur klassisch". Heute die einzige
    /// mögliche Stellung, denn ein zweites Verfahren gibt es nicht.
    #[test]
    fn genesis_steht_auf_nur_klassisch() {
        let reg = ParameterRegistry::vorgabe();
        assert_eq!(
            reg.wert(Parameter::Signaturstufe).als_ganzzahl(),
            Some(Signaturstufe::NurKlassisch.zahl())
        );
    }

    /// ⚑ Ein anderer Parameter läuft nicht durch diese Prüfung.
    ///
    /// Ohne diesen Test könnte die Prüfung versehentlich jeden
    /// Vorschlag betreffen und alles blockieren, was nicht zufällig eine
    /// gültige Stufenfolge ist.
    #[test]
    fn andere_parameter_beruehrt_die_pruefung_nicht() {
        let reg = ParameterRegistry::vorgabe();
        let v = ParameterVorschlag {
            parameter: Parameter::Abstimmungsfenster,
            neuer_wert: Wert::Ganzzahl(200),
        };
        assert!(pruefe_vorschlag(&reg, &v).is_ok());
    }
}
