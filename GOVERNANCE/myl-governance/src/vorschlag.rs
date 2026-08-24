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

    // 3. Invarianten, am entstehenden Zustand.
    pruefe_invarianten(&danach).map_err(VorschlagFehler::Invariante)?;

    Ok(danach)
}
