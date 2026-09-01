//! Wie sich die Arbeit eines Pods auf seine Positionen verteilt.
//!
//! # ⚑ Warum Gewichte und nicht das Modellprofil
//!
//! Die Zuschreibung je Miner folgt aus den Multiplikations-Additionen
//! seines Zuschnitts ([`crate`]-Nachbar `myl_tokenomics::vtfe`). Um sie
//! auszurechnen, bräuchte der Konsens das **Modellprofil** (zehn Felder)
//! und den **Zuschnitt je Position** (vier je Position). Beides ist
//! möglich und wäre die falsche Wahl:
//!
//! **Ein Profil im Zustand ist genauso eine Erklärung wie ein Gewicht,
//! nur mit zehnfacher Fläche.** Wer das Profil falsch einträgt, bekommt
//! falsche Gewichte; das Vertrauen verschiebt sich, es verschwindet
//! nicht. Und es zöge die Modellinnereien in einen Konsenstyp: Eine neue
//! Architektur, ein anderes Expertengemisch, ein anderer
//! Aufmerksamkeitszuschnitt, und die **Form des Zustands** ändert sich,
//! also braucht es eine harte Gabelung.
//!
//! **Mit Gewichten ändert eine neue Architektur die Zahlen, nicht den
//! Typ.** Der Konsens muss nicht wissen, was eine Layer ist.
//!
//! # ⚑ Erklärt, aber nachrechenbar
//!
//! Die Gewichte sind eine Angabe, und zwar eine der **Governance**, nicht
//! eines Teilnehmers: Ein Miner kann sie nicht setzen. Sie hängen an
//! einem [`Arbeitsverteilung::pipeline`]-Stand, und dieser bindet über
//! das Pipeline-Manifest den θ_v-Stand. **Wer beides hat, rechnet die
//! Gewichte nach und widerspricht**, wenn sie nicht stimmen. Genau
//! diesen Maßstab setzt die vTFE-Regel selbst: „Jeder Prüfer rechnet
//! dieselbe Zahl nach, ohne den Zustand einer Anfrage zu kennen."
//!
//! # Was hier nicht steht
//!
//! Keine Tokenzahl. Ein Bündel nennt die vTFE seines **Pods**, und diese
//! Verteilung teilt sie auf die Positionen; das Verhältnis genügt, die
//! absolute Menge wird nicht gebraucht. **Ein Feld weniger im
//! Drahtformat ist ein Feld weniger, über das jemand lügen kann.**

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::Hash;

/// Warum eine Verteilung nicht angenommen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verteilungsfehler {
    /// Keine Position. Ein Pod ohne Positionen ist kein Pod.
    Leer,
    /// Alle Gewichte sind null.
    ///
    /// ⚑ **Kein Randfall, sondern eine Aussage:** Dann leistete kein
    /// Platz etwas, und ein positiver Betrag hätte keinen Empfänger.
    /// Einzelne Nullgewichte sind dagegen erlaubt und richtig: Ein Platz,
    /// der nur die Einbettung hält, rechnet nicht, denn ein
    /// Tabellennachschlag ist keine Multiplikation.
    AllesNull,
}

impl std::fmt::Display for Verteilungsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leer => f.write_str("eine Verteilung ohne Positionen ist keine"),
            Self::AllesNull => f.write_str("alle Gewichte sind null"),
        }
    }
}

impl std::error::Error for Verteilungsfehler {}

/// Die Arbeitsanteile der Shard-Positionen eines Pods.
///
/// Die Felder sind privat: Eine Verteilung, die [`Arbeitsverteilung::neu`]
/// nicht bestanden hat, gibt es nicht. Dieselbe Bauart wie beim
/// Beschluss der Ausfallsicherung und beim Subventionsplan.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Arbeitsverteilung {
    pipeline: Hash,
    gewichte: Vec<u64>,
}

impl Arbeitsverteilung {
    /// Prüft eine Verteilung und nimmt sie an oder lehnt sie ab.
    pub fn neu(pipeline: Hash, gewichte: Vec<u64>) -> Result<Self, Verteilungsfehler> {
        if gewichte.is_empty() {
            return Err(Verteilungsfehler::Leer);
        }
        if gewichte.iter().all(|g| *g == 0) {
            return Err(Verteilungsfehler::AllesNull);
        }
        Ok(Self { pipeline, gewichte })
    }

    /// Der Pipeline-Stand, aus dem die Gewichte folgen.
    pub fn pipeline(&self) -> &Hash {
        &self.pipeline
    }

    /// Die Gewichte in Positionsreihenfolge.
    pub fn gewichte(&self) -> &[u64] {
        &self.gewichte
    }

    /// Zahl der Positionen.
    pub fn positionen(&self) -> usize {
        self.gewichte.len()
    }

    /// Summe der Gewichte.
    pub fn summe(&self) -> u128 {
        self.gewichte.iter().map(|g| *g as u128).sum()
    }

    /// Teilt einen Betrag **exakt** nach den Gewichten auf.
    ///
    /// Jeder Anteil wird abgerundet; die verbleibenden Einheiten gehen
    /// **in Positionsreihenfolge** je eine an Positionen mit Gewicht
    /// über null. **Die Summe der Anteile ist stets der Betrag**, und
    /// zwar für jeden Betrag und jede Verteilung; ein Rest, der
    /// verschwindet, wäre Geld, das niemand bekommt.
    ///
    /// Positionen mit Gewicht null bekommen nichts, auch keinen Rest.
    pub fn aufteilen(&self, betrag: u64) -> Vec<u64> {
        let mut anteile = vec![0u64; self.gewichte.len()];
        let summe = self.summe();
        if betrag == 0 || summe == 0 {
            return anteile;
        }
        let mut verteilt: u128 = 0;
        for (i, g) in self.gewichte.iter().enumerate() {
            let a = (betrag as u128 * *g as u128) / summe;
            anteile[i] = a as u64;
            verteilt += a;
        }
        let mut rest = (betrag as u128 - verteilt) as u64;
        for (i, g) in self.gewichte.iter().enumerate() {
            if rest == 0 {
                break;
            }
            if *g == 0 {
                continue;
            }
            anteile[i] += 1;
            rest -= 1;
        }
        anteile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand(b: u8) -> Hash {
        Hash::sha256(&[b; 4])
    }

    /// Gleiche Gewichte teilen gleich.
    #[test]
    fn gleiche_gewichte_teilen_gleich() {
        let v = Arbeitsverteilung::neu(stand(1), vec![1, 1, 1, 1]).expect("Verteilung");
        assert_eq!(v.aufteilen(100), vec![25, 25, 25, 25]);
    }

    /// ⚑ **Die Summe der Anteile ist stets der Betrag**, auch wenn es
    /// nicht aufgeht. Ein Rest, der verschwindet, wäre Geld, das niemand
    /// bekommt.
    #[test]
    fn die_summe_ist_stets_der_betrag() {
        let v = Arbeitsverteilung::neu(stand(1), vec![3, 1, 1]).expect("Verteilung");
        for betrag in [0u64, 1, 2, 3, 4, 5, 7, 99, 100, 1_000_003] {
            let anteile = v.aufteilen(betrag);
            let summe: u64 = anteile.iter().sum();
            assert_eq!(summe, betrag, "Betrag {betrag} ging nicht auf");
        }
    }

    /// Ein schwereres Gewicht bekommt mehr.
    #[test]
    fn schwerer_bekommt_mehr() {
        let v = Arbeitsverteilung::neu(stand(1), vec![9, 1]).expect("Verteilung");
        let a = v.aufteilen(1_000);
        assert_eq!(a, vec![900, 100]);
    }

    /// ⚑ Ein Platz ohne Gewicht bekommt nichts, auch keinen Rest.
    #[test]
    fn ein_platz_ohne_gewicht_bekommt_nichts() {
        let v = Arbeitsverteilung::neu(stand(1), vec![0, 1, 1]).expect("Verteilung");
        for betrag in [1u64, 2, 3, 7, 101] {
            assert_eq!(v.aufteilen(betrag)[0], 0, "der Nullplatz bekam etwas");
        }
    }

    /// Der Rest geht in Positionsreihenfolge, also wiederholbar.
    #[test]
    fn der_rest_geht_in_positionsreihenfolge() {
        let v = Arbeitsverteilung::neu(stand(1), vec![1, 1, 1]).expect("Verteilung");
        assert_eq!(v.aufteilen(4), vec![2, 1, 1]);
        assert_eq!(v.aufteilen(5), vec![2, 2, 1]);
    }

    /// Gegenprobe: Eine leere Verteilung wird abgelehnt.
    #[test]
    fn eine_leere_verteilung_wird_abgelehnt() {
        assert_eq!(
            Arbeitsverteilung::neu(stand(1), vec![]),
            Err(Verteilungsfehler::Leer)
        );
    }

    /// ⚑ Gegenprobe: Lauter Nullen sind keine Verteilung, sondern die
    /// Aussage, dass niemand etwas geleistet hat.
    #[test]
    fn lauter_nullen_werden_abgelehnt() {
        assert_eq!(
            Arbeitsverteilung::neu(stand(1), vec![0, 0, 0]),
            Err(Verteilungsfehler::AllesNull)
        );
    }

    /// Null zu verteilen ergibt lauter Nullen und stürzt nicht.
    #[test]
    fn null_zu_verteilen_ergibt_nullen() {
        let v = Arbeitsverteilung::neu(stand(1), vec![1, 2]).expect("Verteilung");
        assert_eq!(v.aufteilen(0), vec![0, 0]);
    }

    /// Große Zahlen laufen nicht über: gerechnet wird in `u128`.
    #[test]
    fn grosse_zahlen_laufen_nicht_ueber() {
        let v = Arbeitsverteilung::neu(stand(1), vec![u64::MAX, u64::MAX]).expect("Verteilung");
        let a = v.aufteilen(u64::MAX);
        assert_eq!(a.iter().sum::<u64>(), u64::MAX);
    }
}
