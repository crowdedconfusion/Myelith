//! Prägefunktion (Punkt 1.2, Whitepaper Kap. 5.2).
//!
//! Formel: `M_e = min(B̄_e · (1 + s), M_max)`
//! - `B̄_e`: geglättetes Burn-Volumen (EMA, siehe `ema.rs`)
//! - `s`: Subventionsrate als Ganzzahl-Bruch (Zähler/Nenner), in der
//!   Anlaufphase > 0 (Kap. 5.7), im Zielbetrieb 0
//! - `M_max`: Präge-Obergrenze je Epoche
//!
//! Rundung: floor („es wird niemals mehr geprägt als die Formel mit
//! Abwärtsrundung ergibt"); Zwischenrechnung in `u128` (B̄_e · (den+num)
//! kann den u64-Bereich überschreiten).

/// Parameter der Prägefunktion (später Governance-verwaltet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintParams {
    /// Subventionsrate s als Bruch-Zähler (0 im Zielbetrieb).
    pub subsidy_num: u64,
    /// Subventionsrate s als Bruch-Nenner (> 0).
    pub subsidy_den: u64,
    /// Präge-Obergrenze je Epoche (Kleinstbeträge).
    pub m_max: u64,
}

/// `M_e = min(B̄_e · (1 + s), M_max)` mit floor-Division.
///
/// Ein Nenner von 0 ist ein Parameter-Fehler; die Funktion bleibt
/// deterministisch und prägt dann 0 (sichere Seite: nichts prägen statt
/// überprägen).
///
/// ## ⚑ Fund 46: Die Verbreiterung stand eine Rechnung zu spät
///
/// Hier stand `(params.subsidy_den + params.subsidy_num) as u128`. Die
/// **Addition** geschieht darin in `u64` und erst ihr Ergebnis wird
/// verbreitert; für `den + num > u64::MAX` läuft sie über. Im Debug-Build
/// ist das eine Panik, im Release-Build eine Prägung, die nicht der
/// Formel entspricht: Zwei Knoten mit verschiedenen Bauprofilen kämen zu
/// verschiedenen Geldmengen, und das ist ein Konsensbruch.
///
/// Der Kommentar eine Zeile darüber sagte ausdrücklich „Zwischenrechnung
/// in `u128`, weil B̄_e · (den+num) den u64-Bereich überschreiten kann".
/// Die Aufmerksamkeit galt der Multiplikation; die Addition davor stand
/// im selben Ausdruck und blieb schmal.
///
/// Erreichbar ist der Fall, weil `subsidy_num` und `subsidy_den`
/// Governance-Parameter sind (Kap. 10.3): Was der Typ zulässt, kann eine
/// Abstimmung setzen. Gefunden von der adversarialen Testebene (K4).
pub fn mint_amount(ema_burn: u64, params: &MintParams) -> u64 {
    debug_assert!(params.subsidy_den > 0, "Subventions-Nenner muss > 0 sein");
    if params.subsidy_den == 0 {
        return 0;
    }
    // Erst verbreitern, dann rechnen — nicht umgekehrt (Fund 46).
    let faktor = params.subsidy_den as u128 + params.subsidy_num as u128;
    // Auch `u128` reicht am Rand nicht: B̄_e und der Faktor erreichen je
    // rund 1,8·10¹⁹ bzw. 3,7·10¹⁹, ihr Produkt 6,8·10³⁸ und damit mehr
    // als u128::MAX ≈ 3,4·10³⁸.
    //
    // **Sättigen ist hier nicht bloß sicher, sondern exakt.** Sättigt das
    // Produkt, so ist `u128::MAX / den ≥ 3,4·10³⁸ / 1,8·10¹⁹ ≈ 1,8·10¹⁹`
    // und damit größer als jedes `m_max` in `u64`; der Deckel greift, und
    // er hätte auch beim wahren, noch größeren Wert gegriffen. Das
    // Ergebnis ist dasselbe wie bei unbeschränkter Rechnung.
    let numerator = (ema_burn as u128).saturating_mul(faktor);
    let minted = numerator / params.subsidy_den as u128;
    let capped = minted.min(params.m_max as u128);
    capped as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(num: u64, den: u64, m_max: u64) -> MintParams {
        MintParams {
            subsidy_num: num,
            subsidy_den: den,
            m_max,
        }
    }

    #[test]
    fn praeegung_exakt_ohne_subvention() {
        assert_eq!(mint_amount(1_000, &params(0, 1, u64::MAX)), 1_000);
    }

    #[test]
    fn praeegung_mit_subvention() {
        // 1000 · (1 + 1/10) = 1100.
        assert_eq!(mint_amount(1_000, &params(1, 10, u64::MAX)), 1_100);
        // 1000 · (1 + 1/2) = 1500.
        assert_eq!(mint_amount(1_000, &params(1, 2, u64::MAX)), 1_500);
    }

    #[test]
    fn praeegung_rundet_abwaerts() {
        // 10 · (1 + 1/3) = 13,33… → 13.
        assert_eq!(mint_amount(10, &params(1, 3, u64::MAX)), 13);
        // 5 · (1 + 1/3) = 6,66… → 6.
        assert_eq!(mint_amount(5, &params(1, 3, u64::MAX)), 6);
    }

    #[test]
    fn obergrenze_kappt() {
        assert_eq!(mint_amount(1_000, &params(1, 2, 800)), 800);
        assert_eq!(mint_amount(1_000, &params(1, 2, 1_500)), 1_500);
        assert_eq!(mint_amount(2_000, &params(0, 1, 1_500)), 1_500);
    }

    #[test]
    fn monotonie_in_burn_und_subvention() {
        let p = params(1, 10, u64::MAX);
        let a = mint_amount(1_000, &p);
        let b = mint_amount(2_000, &p);
        assert!(b >= a);
        let c = mint_amount(1_000, &params(2, 10, u64::MAX));
        assert!(c >= a);
    }

    #[test]
    fn ueberlaufsicherheit_bei_extremwerten() {
        // B̄_e = u64::MAX, Subvention 100 %: u128-Zwischenrechnung,
        // kein Überlauf, kein Panic; die Obergrenze hält das Ergebnis
        // im u64-Bereich.
        let m = mint_amount(u64::MAX, &params(1, 1, u64::MAX));
        assert_eq!(m, u64::MAX);
        let m_gedeckelt = mint_amount(u64::MAX, &params(1, 1, 1_000_000));
        assert_eq!(m_gedeckelt, 1_000_000);
    }
}
