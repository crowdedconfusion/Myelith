//! Auslastungsboden über Training (Phase 5, Punkte 5.1 bis 5.3).
//!
//! # Das Ausfallbild, gegen das dies gebaut ist
//!
//! Viele Halter kaufen MYL und **verbrennen nicht**. Der geglättete Burn
//! fällt, die Prägung folgt ihm, das Minereinkommen fällt, Miner gehen,
//! die Kapazität sinkt. **Der Kurs kann dabei steigen**, und genau das
//! macht das Bild tückisch: Von außen sieht ein sterbendes Netz aus wie
//! ein erfolgreiches.
//!
//! Die bestehenden Mittel greifen daneben. Der Burn-Deckel begrenzt
//! einen Verbrauchs**stoß nach oben**, nicht einen Ausfall nach unten.
//! Die Preisuntergrenze verhindert den Preisverfall, aber ein niedriger
//! Preis ist hier die Reaktion und nicht das Problem: **Das Problem ist
//! die Menge.**
//!
//! # Der Boden
//!
//! Fällt die Auslastung unter [`AUSLASTUNGSBODEN`], nimmt
//! treasury-finanziertes Training die freie Kapazität auf. Die Bausteine
//! lagen bereit: die Auslastungsmessung, die Vergütungsobergrenze von
//! 70 % gegenüber der Inferenz, der Treasury-Anteil. **Was fehlte, war
//! die Einschaltregel**, und sie ist klein.
//!
//! # ⚑ Ein Boden ohne Reichweite ist ein Versprechen
//!
//! Das Treasury bekommt drei Prozent der Prägung, **und die Prägung
//! fällt in genau dem Szenario, in dem der Boden gebraucht wird.** Wer
//! den Mechanismus eine Sicherung nennt, ohne die Reichweite
//! ausgerechnet zu haben, hat nichts gesichert.
//!
//! [`reichweite`] rechnet sie aus, und sie rechnet mit dem Zufluss, den
//! der Aufrufer nennt. ⚑ **In dem Szenario, für das der Boden gebaut
//! ist, fällt genau dieser Zufluss.** Die Zahl ist deshalb eine
//! **Obergrenze** und keine Zusage; wer sie als Zusage liest, hat den
//! Absatz hier überlesen.

use crate::utilization::UTILIZATION_SCALE;

/// Die Sollauslastung, unterhalb derer der Boden greift.
///
/// **50 Prozent**, in der Festkommadarstellung von
/// [`crate::utilization`]. Die Zahl ist ein Startwert und gehört unter
/// Governance: Sie hängt daran, wie viel Leerlauf ein Netz verträgt,
/// bevor Miner abwandern, und das weiß vor dem ersten Betrieb niemand.
///
/// **Warum überhaupt eine Zahl und nicht „sobald Leerlauf da ist":**
/// Ein Netz mit hundert Prozent Auslastung hat keine Reserve für einen
/// Nachfragestoß. Der Boden soll das Abwandern verhindern, nicht die
/// Auslastung maximieren.
pub const AUSLASTUNGSBODEN: i64 = UTILIZATION_SCALE / 2;

/// Woran der ausgeschriebene Umfang endete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deckel {
    /// Die Lücke wurde ganz geschlossen.
    Keiner,
    /// Der Treasury-Bestand reichte nicht.
    Treasury,
}

/// Was der Boden in dieser Epoche verlangt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bodenbedarf {
    /// Die Lücke zur Sollauslastung, in vTFE-Einheiten.
    pub luecke_vtfe: u64,
    /// Was davon tatsächlich ausgeschrieben wird.
    ///
    /// Kleiner als [`Self::luecke_vtfe`], wenn das Treasury nicht
    /// reicht.
    pub ausgeschrieben_vtfe: u64,
    /// Was das aus dem Treasury kostet, in Kleinstbeträgen.
    pub kosten: u64,
    /// Woran der Umfang endete.
    pub deckel: Deckel,
}

impl Bodenbedarf {
    /// Ob der Boden in dieser Epoche überhaupt greift.
    pub fn greift(&self) -> bool {
        self.luecke_vtfe > 0
    }

    /// Ob die Lücke ganz geschlossen wurde.
    pub fn geschlossen(&self) -> bool {
        self.ausgeschrieben_vtfe >= self.luecke_vtfe
    }
}

/// Die Lücke zwischen Ist- und Sollauslastung, in vTFE-Einheiten.
///
/// `floor((boden − u_e) · kapazität / SKALA)`, abgerundet: Es wird
/// nie mehr ausgeschrieben, als die Lücke hergibt.
///
/// Null, wenn die Auslastung den Boden erreicht oder überschreitet, und
/// null bei fehlender Kapazität: Ohne Kapazität gibt es nichts
/// aufzunehmen, und Training, das niemand rechnen kann, ist keine
/// Auslastung.
pub fn auslastungsluecke(u_e: i64, kapazitaet_vtfe: u64) -> u64 {
    if u_e >= AUSLASTUNGSBODEN || kapazitaet_vtfe == 0 {
        return 0;
    }
    let fehlend = (AUSLASTUNGSBODEN - u_e.max(0)) as u128;
    ((fehlend * kapazitaet_vtfe as u128) / UTILIZATION_SCALE as u128) as u64
}

/// Was der Boden verlangt und was davon bezahlbar ist (Punkt 5.1).
///
/// # Reihenfolge, und warum sie so herum ist
///
/// Erst die **Lücke**, dann die **Vergütung je vTFE** (gedeckelt auf
/// 70 % der Inferenzvergütung, [`crate::training::capped_training_reward`]),
/// dann der **Treasury-Bestand**. Die Deckelung der Vergütung gehört vor
/// die des Bestands: Sie ist eine Regel über den Preis, der Bestand eine
/// über die Menge, und wer zuerst die Menge kürzt, zahlt für den Rest
/// womöglich zu viel.
///
/// # ⚑ Es wird nie mehr ausgeschrieben, als bezahlt ist
///
/// Reicht das Treasury nur für einen Teil, wird nur dieser Teil
/// ausgeschrieben und der Deckel benannt. Die Lücke bleibt im Ergebnis
/// stehen: **Ein halb geschlossener Boden, der wie ein geschlossener
/// aussieht, ist schlimmer als gar keiner**, denn niemand sucht dann
/// nach der Ursache der Abwanderung.
pub fn bodenbedarf(
    u_e: i64,
    kapazitaet_vtfe: u64,
    treasury_bestand: u64,
    inferenzverguetung_je_vtfe: u64,
    gewuenschte_verguetung_je_vtfe: u64,
) -> Bodenbedarf {
    let luecke = auslastungsluecke(u_e, kapazitaet_vtfe);
    if luecke == 0 {
        return Bodenbedarf {
            luecke_vtfe: 0,
            ausgeschrieben_vtfe: 0,
            kosten: 0,
            deckel: Deckel::Keiner,
        };
    }
    let satz = crate::training::capped_training_reward(
        gewuenschte_verguetung_je_vtfe,
        inferenzverguetung_je_vtfe,
    );
    if satz == 0 {
        // Umsonst arbeitet niemand; die Lücke bleibt offen und sichtbar.
        return Bodenbedarf {
            luecke_vtfe: luecke,
            ausgeschrieben_vtfe: 0,
            kosten: 0,
            deckel: Deckel::Treasury,
        };
    }
    let voll_kosten = (luecke as u128).saturating_mul(satz as u128);
    if voll_kosten <= treasury_bestand as u128 {
        Bodenbedarf {
            luecke_vtfe: luecke,
            ausgeschrieben_vtfe: luecke,
            kosten: voll_kosten as u64,
            deckel: Deckel::Keiner,
        }
    } else {
        let tragbar = (treasury_bestand as u128 / satz as u128) as u64;
        Bodenbedarf {
            luecke_vtfe: luecke,
            ausgeschrieben_vtfe: tragbar,
            kosten: (tragbar as u128 * satz as u128) as u64,
            deckel: Deckel::Treasury,
        }
    }
}

/// Wie weit der Bestand den Boden trägt (Punkt 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reichweite {
    /// Der Zufluss deckt die Kosten; der Boden trägt sich selbst.
    ///
    /// ⚑ **Das ist der Fall, der in diesem Szenario nicht eintritt.**
    /// Der Zufluss ist der Treasury-Anteil der Prägung, und die Prägung
    /// fällt gerade deshalb, weil der Boden gebraucht wird.
    Selbsttragend,
    /// So viele Epochen trägt der Bestand die Unterdeckung.
    Epochen(u64),
}

/// Rechnet aus, wie viele Epochen der Bestand den Boden trägt.
///
/// `bestand / (kosten − zufluss)`, abgerundet. Deckt der Zufluss die
/// Kosten, ist die Reichweite [`Reichweite::Selbsttragend`].
///
/// # ⚑ Eine Obergrenze, keine Zusage
///
/// Gerechnet wird mit dem Zufluss, den der Aufrufer nennt. **In dem
/// Szenario, für das der Boden gebaut ist, fällt genau dieser Zufluss**,
/// denn er ist der Treasury-Anteil einer fallenden Prägung. Die Zahl ist
/// deshalb die Reichweite unter der Annahme, dass es nicht schlimmer
/// wird, und die Annahme ist optimistisch.
///
/// Ohne Kosten ist der Boden umsonst zu haben, das ist ebenfalls
/// selbsttragend.
pub fn reichweite(bestand: u64, kosten_je_epoche: u64, zufluss_je_epoche: u64) -> Reichweite {
    if kosten_je_epoche <= zufluss_je_epoche {
        return Reichweite::Selbsttragend;
    }
    let unterdeckung = kosten_je_epoche - zufluss_je_epoche;
    Reichweite::Epochen(bestand / unterdeckung)
}

/// Wie lange das Einkommen die Kosten schon unterschreitet (Punkt 5.3).
///
/// # Die prüfbare Fassung des Anliegens
///
/// „Miner müssen online bleiben können" ist keine Bedingung, die sich
/// prüfen lässt. **Das Minereinkommen darf die Kosten des Onlinebleibens
/// nicht länger als `t` Epochen unterschreiten** ist eine, und sie
/// schreibt sich wie `S_min` in [`crate::sicherheit`]: eine Ungleichung
/// mit benannten Größen.
///
/// `true` heißt: Die Bedingung ist **verletzt**.
///
/// ⚑ **Eine einzelne Epoche unter den Kosten ist kein Verstoß.**
/// Einkommen schwankt, und eine Bedingung, die bei jeder Schwankung
/// anschlägt, wird abgeschaltet. Erst die Dauer macht es zu einem
/// Befund, und `t` ist der Ort, an dem das entschieden wird.
pub fn liveness_verletzt(epochen_unter_kosten: u64, t: u64) -> bool {
    epochen_unter_kosten > t
}

/// Ob das Einkommen einer Epoche die Kosten trägt.
///
/// Getrennt von [`liveness_verletzt`], weil es zwei verschiedene Fragen
/// sind: Diese hier ist die Messung einer Epoche, jene das Urteil über
/// eine Reihe davon.
pub fn einkommen_traegt(einkommen: u64, kosten: u64) -> bool {
    einkommen >= kosten
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utilization::calculate_utilization;

    /// Über dem Boden greift nichts.
    #[test]
    fn ueber_dem_boden_greift_nichts() {
        let u = calculate_utilization(800, 1000); // 80 %
        let b = bodenbedarf(u, 1_000, 1_000_000, 100, 100);
        assert!(!b.greift());
        assert_eq!(b.kosten, 0);
    }

    /// Genau auf dem Boden greift ebenfalls nichts: Die Schwelle ist
    /// erreicht, nicht unterschritten.
    #[test]
    fn genau_auf_dem_boden_greift_nichts() {
        let b = bodenbedarf(AUSLASTUNGSBODEN, 1_000, 1_000_000, 100, 100);
        assert!(!b.greift());
    }

    /// ⚑ Unter dem Boden wird genau die Lücke ausgeschrieben.
    #[test]
    fn unter_dem_boden_wird_die_luecke_geschrieben() {
        let u = calculate_utilization(200, 1000); // 20 %
        let b = bodenbedarf(u, 1_000, u64::MAX / 2, 100, 100);
        // 50 % Soll minus 20 % Ist sind 30 % von 1000 vTFE.
        assert_eq!(b.luecke_vtfe, 300);
        assert_eq!(b.ausgeschrieben_vtfe, 300);
        assert_eq!(b.deckel, Deckel::Keiner);
        assert!(b.geschlossen());
    }

    /// Bei null Auslastung ist die Lücke der ganze Boden.
    #[test]
    fn ohne_nachfrage_ist_die_luecke_der_ganze_boden() {
        let b = bodenbedarf(0, 1_000, u64::MAX / 2, 100, 100);
        assert_eq!(b.luecke_vtfe, 500);
    }

    /// Ohne Kapazität gibt es nichts aufzunehmen.
    #[test]
    fn ohne_kapazitaet_gibt_es_keine_luecke() {
        assert_eq!(auslastungsluecke(0, 0), 0);
        let b = bodenbedarf(0, 0, u64::MAX / 2, 100, 100);
        assert!(!b.greift());
    }

    /// ⚑ **Die Vergütung ist auf 70 % der Inferenz gedeckelt**, und der
    /// Boden rechnet mit dem gedeckelten Satz, nicht mit dem gewünschten.
    #[test]
    fn die_verguetung_bleibt_unter_der_inferenz() {
        let u = calculate_utilization(200, 1000);
        // Gewünscht wird die volle Inferenzvergütung; erlaubt sind 70 %.
        let b = bodenbedarf(u, 1_000, u64::MAX / 2, 100, 100);
        assert_eq!(b.kosten, 300 * 70, "es wurde nicht auf 70 % gedeckelt");
    }

    /// ⚑ **Reicht das Treasury nicht, wird nur der bezahlbare Teil
    /// ausgeschrieben, und die Lücke bleibt sichtbar.**
    #[test]
    fn ein_knappes_treasury_schreibt_weniger_aus_und_sagt_es() {
        let u = calculate_utilization(200, 1000);
        // Satz ist 70; für 300 vTFE wären 21 000 nötig, da sind 7 000.
        let b = bodenbedarf(u, 1_000, 7_000, 100, 100);
        assert_eq!(b.luecke_vtfe, 300, "die Luecke wurde kleingerechnet");
        assert_eq!(b.ausgeschrieben_vtfe, 100);
        assert_eq!(b.kosten, 7_000);
        assert_eq!(b.deckel, Deckel::Treasury);
        assert!(!b.geschlossen());
    }

    /// Ein leeres Treasury schreibt nichts aus und verschweigt die Lücke
    /// nicht.
    #[test]
    fn ein_leeres_treasury_schreibt_nichts_aus() {
        let u = calculate_utilization(200, 1000);
        let b = bodenbedarf(u, 1_000, 0, 100, 100);
        assert_eq!(b.luecke_vtfe, 300);
        assert_eq!(b.ausgeschrieben_vtfe, 0);
        assert_eq!(b.deckel, Deckel::Treasury);
    }

    /// Ohne Inferenzvergütung ist auch die Trainingsvergütung null, und
    /// umsonst arbeitet niemand.
    #[test]
    fn ohne_inferenzverguetung_wird_nichts_ausgeschrieben() {
        let u = calculate_utilization(200, 1000);
        let b = bodenbedarf(u, 1_000, u64::MAX / 2, 0, 100);
        assert_eq!(b.luecke_vtfe, 300);
        assert_eq!(b.ausgeschrieben_vtfe, 0);
    }

    /// Die Kosten übersteigen den Bestand nie.
    #[test]
    fn die_kosten_uebersteigen_den_bestand_nie() {
        for bestand in [0u64, 1, 69, 70, 71, 20_999, 21_000, 21_001] {
            let u = calculate_utilization(200, 1000);
            let b = bodenbedarf(u, 1_000, bestand, 100, 100);
            assert!(
                b.kosten <= bestand,
                "Bestand {bestand}: Kosten {} liegen darueber",
                b.kosten
            );
        }
    }

    /// Reichweite: Bestand geteilt durch die Unterdeckung.
    #[test]
    fn die_reichweite_folgt_der_unterdeckung() {
        assert_eq!(reichweite(1_000, 300, 100), Reichweite::Epochen(5));
        assert_eq!(reichweite(1_000, 300, 0), Reichweite::Epochen(3));
        assert_eq!(reichweite(0, 300, 0), Reichweite::Epochen(0));
    }

    /// Deckt der Zufluss die Kosten, trägt der Boden sich selbst.
    #[test]
    fn ein_deckender_zufluss_traegt_den_boden() {
        assert_eq!(reichweite(0, 100, 100), Reichweite::Selbsttragend);
        assert_eq!(reichweite(0, 100, 200), Reichweite::Selbsttragend);
        assert_eq!(reichweite(0, 0, 0), Reichweite::Selbsttragend);
    }

    /// ⚑ Eine einzelne Epoche unter den Kosten ist kein Verstoß.
    #[test]
    fn eine_einzelne_epoche_unter_den_kosten_ist_kein_verstoss() {
        assert!(!liveness_verletzt(1, 3));
        assert!(!liveness_verletzt(3, 3));
        assert!(liveness_verletzt(4, 3));
    }

    /// Mit `t = 0` schlägt die Bedingung ab der ersten Epoche an.
    #[test]
    fn ohne_toleranz_schlaegt_es_sofort_an() {
        assert!(!liveness_verletzt(0, 0));
        assert!(liveness_verletzt(1, 0));
    }

    /// Die Messung einer Epoche ist von dem Urteil über eine Reihe
    /// getrennt.
    #[test]
    fn die_messung_einer_epoche_ist_eine_ungleichung() {
        assert!(einkommen_traegt(100, 100));
        assert!(einkommen_traegt(101, 100));
        assert!(!einkommen_traegt(99, 100));
    }
}
