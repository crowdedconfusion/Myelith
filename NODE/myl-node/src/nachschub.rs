//! Blocknachforderung: wie ein Neuling aufholt.
//!
//! # Das Problem, das dieses Modul löst
//!
//! Die Kette hängt am Vorgänger-Hash. Ein Knoten, der Block 1 nie
//! gesehen hat, weist Block 2 zurück, weil dessen Vorgänger ihm
//! unbekannt ist, und danach jeden weiteren aus demselben Grund. **Ein
//! einziger verpasster Block hängt ihn für den ganzen Lauf ab.**
//!
//! Der erste Dreiknoten-Probelauf lief genau so ins Leere: Der Erzeuger
//! war vier Sekunden früher da, baute acht Blöcke, und die beiden
//! anderen wiesen alle acht mit „passt nicht an" zurück.
//!
//! # ⚑ Woran ein Knoten seine Lücke abzählt (Stand 2026-08-27)
//!
//! **Am Höhenfeld des Blockkopfs**, und das gibt es seit dem
//! 2026-08-27. Ein Knoten auf Höhe 0, der Höhe 8 empfängt, weiß damit
//! genau, dass ihm 1 bis 7 fehlen.
//!
//! Zwei frühere Fassungen dieses Absatzes standen daneben, und beide
//! Male aus demselben Grund: Der Protokolltyp trug **kein** Höhenfeld,
//! die Probekette schrieb ihre Höhe in `epoch`, und dieselbe Zahl
//! bedeutete je nach Leser eine Höhe oder eine Epoche. Die erste
//! Fassung schloss daraus, die Lücke sei nicht benennbar; die zweite
//! benannte sie richtig und über ein Feld, dessen Bedeutung eine andere
//! war. **Aufgelöst ist die Doppelbelegung jetzt:** `height` zählt
//! Blöcke, `epoch` folgt aus der Höhe, und die Nachforderung hängt an
//! `height`.
//!
//! # Die Regel, die dieses Modul nicht bricht
//!
//! **Nachgelieferte Blöcke gehen denselben Weg wie verbreitete.** Sie
//! landen in [`crate::kette::Kette::uebernimm`], mit derselben
//! Anschlussprüfung und derselben Nachrechnung der Zustandswurzel.
//!
//! Das ist keine Formsache. Wäre die Nachlieferung ein zweiter,
//! schwächerer Weg in die Kette, wäre sie das Loch: Wer einen Knoten
//! dazu bringt, einen Block **nachzufordern**, bekäme ihn hineingelegt,
//! ohne dass er nachrechnet. Der Nachschub ist ein Transportweg, kein
//! Vertrauensweg.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::block::Block;

/// Höchstzahl Blöcke je Nachlieferung.
///
/// **Herleitung:** Ein Neuling gegenüber einer langen Kette darf nicht
/// alles auf einmal anfordern; er würde den Gegenüber zum Senden
/// beliebiger Datenmengen bewegen, und das wäre ein Verstärker. 64
/// Blöcke sind bei der Größe eines Probeblocks (unter 200 Bytes)
/// bequem unter der Anfragegrenze und holen eine übliche Lücke in einem
/// Zug.
///
/// Größere Rückstände holt der Knoten in mehreren Runden auf: Jede
/// gelungene Nachlieferung hebt seine Höhe, und die nächste Anfrage
/// beginnt dort.
pub const MAX_BLOECKE_JE_LIEFERUNG: u64 = 64;

/// Was ein Knoten nachfordern kann.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Nachforderung {
    /// Blöcke der Höhen `ab` bis einschließlich `bis`.
    Bloecke { ab: u64, bis: u64 },
}

/// Was auf eine Nachforderung zurückkommt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Nachlieferung {
    /// Die angeforderten Blöcke, aufsteigend nach Höhe.
    Bloecke(Vec<Block>),
    /// Nichts vorhanden. **Eine Antwort, kein Schweigen:** Der
    /// Fragende soll den Unterschied zwischen „habe ich nicht" und
    /// „habe nicht geantwortet" kennen, sonst wartet er auf eine
    /// Zeitüberschreitung, die nichts bedeutet.
    Nichts,
}

impl Nachforderung {
    /// Der Bereich, den ein Knoten auf Höhe `eigene` anfordern sollte,
    /// wenn er von Höhe `fremde` gehört hat.
    ///
    /// `None`, wenn nichts fehlt. Der Bereich ist auf
    /// [`MAX_BLOECKE_JE_LIEFERUNG`] gedeckelt.
    pub fn fuer_rueckstand(eigene: u64, fremde: u64) -> Option<Nachforderung> {
        if fremde <= eigene {
            return None;
        }
        let ab = eigene + 1;
        let bis = fremde.min(ab + MAX_BLOECKE_JE_LIEFERUNG - 1);
        Some(Nachforderung::Bloecke { ab, bis })
    }

    /// Als Bytes für den Anfragekanal.
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    /// Aus Bytes zurück. `None` bei allem, was nicht passt, einschließlich
    /// Anhängseln.
    pub fn aus_bytes(daten: &[u8]) -> Option<Nachforderung> {
        let mut rest = daten;
        match Nachforderung::deserialize(&mut rest) {
            Ok(n) if rest.is_empty() => Some(n),
            _ => None,
        }
    }
}

impl Nachlieferung {
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    pub fn aus_bytes(daten: &[u8]) -> Option<Nachlieferung> {
        let mut rest = daten;
        match Nachlieferung::deserialize(&mut rest) {
            Ok(n) if rest.is_empty() => Some(n),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wer_nichts_verpasst_hat_fordert_nichts() {
        assert_eq!(Nachforderung::fuer_rueckstand(8, 8), None);
        assert_eq!(Nachforderung::fuer_rueckstand(9, 8), None, "voraus, nicht zurück");
    }

    #[test]
    fn ein_neuling_fordert_ab_eins() {
        assert_eq!(
            Nachforderung::fuer_rueckstand(0, 5),
            Some(Nachforderung::Bloecke { ab: 1, bis: 5 })
        );
    }

    #[test]
    fn eine_luecke_in_der_mitte_beginnt_beim_naechsten() {
        assert_eq!(
            Nachforderung::fuer_rueckstand(3, 7),
            Some(Nachforderung::Bloecke { ab: 4, bis: 7 })
        );
    }

    #[test]
    fn ein_grosser_rueckstand_wird_gedeckelt() {
        // Sonst brächte ein Neuling den Gegenüber dazu, beliebige
        // Datenmengen zu senden: wenig Aufwand beim Fragenden, viel
        // beim Antwortenden.
        let n = Nachforderung::fuer_rueckstand(0, 10_000).unwrap();
        let Nachforderung::Bloecke { ab, bis } = n;
        assert_eq!(ab, 1);
        assert_eq!(bis, MAX_BLOECKE_JE_LIEFERUNG);
        assert_eq!(bis - ab + 1, MAX_BLOECKE_JE_LIEFERUNG);
    }

    #[test]
    fn mehrere_runden_holen_auch_grosse_rueckstaende_auf() {
        // Jede gelungene Lieferung hebt die Höhe, die nächste Anfrage
        // beginnt dort. Nach genug Runden ist die Lücke zu.
        let ziel = 200u64;
        let mut hoehe = 0u64;
        let mut runden = 0;
        while let Some(Nachforderung::Bloecke { bis, .. }) =
            Nachforderung::fuer_rueckstand(hoehe, ziel)
        {
            hoehe = bis;
            runden += 1;
            assert!(runden < 20, "kommt nicht voran");
        }
        assert_eq!(hoehe, ziel);
        assert_eq!(runden, 4, "200 Blöcke in Schritten von 64: vier Runden");
    }

    #[test]
    fn nachforderung_ueberlebt_den_weg() {
        let n = Nachforderung::Bloecke { ab: 4, bis: 9 };
        let bytes = n.als_bytes().unwrap();
        assert_eq!(Nachforderung::aus_bytes(&bytes), Some(n));
    }

    #[test]
    fn ein_anhaengsel_macht_die_nachforderung_ungueltig() {
        // Zwei Anfragen mit gleichem Inhalt und verschiedenem Anhang
        // wären zwei Anfragen. Das ist ein Kanal, kein Zufall.
        let mut bytes = Nachforderung::Bloecke { ab: 1, bis: 2 }.als_bytes().unwrap();
        bytes.push(0);
        assert_eq!(Nachforderung::aus_bytes(&bytes), None);
    }

    #[test]
    fn unsinn_wird_abgelehnt() {
        assert_eq!(Nachforderung::aus_bytes(&[0xFF; 3]), None);
        assert_eq!(Nachforderung::aus_bytes(&[]), None);
        assert_eq!(Nachlieferung::aus_bytes(&[0xAB; 9]), None);
    }

    #[test]
    fn eine_leere_lieferung_ist_eine_antwort() {
        // „Habe ich nicht" muss sich von „habe nicht geantwortet"
        // unterscheiden lassen, sonst wartet der Fragende auf eine
        // Zeitüberschreitung, die nichts bedeutet.
        let bytes = Nachlieferung::Nichts.als_bytes().unwrap();
        assert_eq!(Nachlieferung::aus_bytes(&bytes), Some(Nachlieferung::Nichts));
    }
}
