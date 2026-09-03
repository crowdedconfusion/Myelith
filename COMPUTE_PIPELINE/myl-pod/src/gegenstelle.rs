//! Die Gegenstelle des Betreibers: **ein** Knoten, angekündigt und
//! geprüft (Fund 165, 2026-09-03).
//!
//! # ⚑ Warum es das gibt
//!
//! `Umschlag::oeffnen` braucht die Punkte der Gegenstelle **vor** dem
//! Entsiegeln; sie können also nicht im Umschlag reisen. Für einen
//! echten Pod steht die Antwort in der Kette, und dafür ist das
//! [`Gegenstellen`](crate::entsiegelung::Gegenstellen)-Merkmal da.
//!
//! **Für den Betreiberzuschnitt der Phase 1 steht sie nirgends:** Ein
//! Betreiber startet einen Knoten und einen Shard-Prozess auf einer
//! Maschine, und der Shard ist keinem Pod der Kette zugeteilt. Hier
//! kündigt der Knoten sich selbst an, und der Shard prüft die
//! Ankündigung gegen den Endpunkt, den sein Betreiber ihm genannt hat.
//!
//! # ⚑ Zwei Schichten, und beide werden gebraucht
//!
//! 1. **Der Ausweis der Leitung** (`0600`, wie Bitcoin Cores
//!    `.cookie`) sagt: „du darfst hereinreden". Er ist ein
//!    vorgeteiltes Geheimnis und sagt **nichts** darüber, wer redet.
//! 2. **Die Ankündigung** sagt: „ich bin dieser Endpunkt". Sie ist mit
//!    dem **Konsensschlüssel** unterschrieben, und der Endpunkt ist
//!    dessen Hash, also ist die Frage „gehört dieser Punkt zu dieser
//!    Gegenstelle" vollständig aus der Ankündigung beantwortbar.
//!
//! Wer nur die erste Schicht hätte, machte jeden, der den Ausweis lesen
//! kann, zum Knoten, und das Siegel wäre Theater. **Dieselbe Bauart wie
//! in `libp2p-noise`**, wo der statische Schlüssel im Handshake durch
//! eine Signatur des Identitätsschlüssels gedeckt ist.
//!
//! # ⚑ Ein Platz und keine Tabelle
//!
//! Es gibt genau **einen** erwarteten Knoten, also steht hier genau
//! **ein** Platz, der überschrieben wird. Eine Tabelle je Sitzung oder
//! je Epoche wüchse mit jeder Anfrage, und das ist Fund 164, den wir
//! gerade erst aufgeschrieben haben.

use std::sync::Mutex;

use myl_siegel::{Endpunkt, Epochenankuendigung, Gegenpunkte};
use myl_types::ids::EpochId;
use myl_types::ortsleitung::MAX_ANKUENDIGUNG_BYTES;

use crate::entsiegelung::Gegenstellen;

/// Der eine Knoten, den dieser Shard-Prozess bedient.
pub struct Betreibergegenstelle {
    /// Wen der Betreiber erwartet: der Hash des Konsensschlüssels des
    /// Knotens.
    ///
    /// ⚑ **Ohne ihn trägt nichts.** Er ist der einzige Teil, der
    /// ausserhalb des Protokolls vereinbart wird, wie der statische
    /// Schlüssel einer WireGuard-Gegenstelle. Ein Shard, der jeden
    /// Endpunkt annähme, prüfte nur noch, ob eine Unterschrift zu sich
    /// selbst passt.
    erwartet: Endpunkt,
    /// Die zuletzt angenommene Ankündigung, mit ihrer Epoche.
    stand: Mutex<Option<(EpochId, Gegenpunkte)>>,
}

impl Betreibergegenstelle {
    /// Erwartet genau diesen Endpunkt.
    pub fn neu(erwartet: Endpunkt) -> Self {
        Self {
            erwartet,
            stand: Mutex::new(None),
        }
    }

    /// Steht schon eine geprüfte Ankündigung, und für welche Epoche?
    pub fn angekuendigt(&self) -> Option<EpochId> {
        self.stand.lock().ok().and_then(|s| s.as_ref().map(|(e, _)| *e))
    }
}

impl Gegenstellen for Betreibergegenstelle {
    fn nachschlagen(&self, _sitzung: u64) -> Option<(Endpunkt, Gegenpunkte)> {
        let stand = self.stand.lock().ok()?;
        let (_, punkte) = stand.as_ref()?;
        Some((self.erwartet, punkte.clone()))
    }

    fn ankuendigen(&self, roh: &[u8], epoche: EpochId) -> bool {
        // ⚑ **Der Deckel vor dem Zerlegen.** Ein Rahmen darf gross
        // sein; eine Ankündigung nicht.
        if roh.len() > MAX_ANKUENDIGUNG_BYTES {
            return false;
        }
        let Ok(ankuendigung) = borsh::from_slice::<Epochenankuendigung>(roh) else {
            return false;
        };
        // ⚑ **`pruefe` ist der einzige Ausgang.** Es prüft Epoche,
        // Gruppenzugehörigkeit des Schlüssels, den Endpunkt und die
        // Unterschrift; die Felder sind privat, es gibt keinen
        // Nebenausgang.
        let Ok(punkte) = ankuendigung.pruefe(self.erwartet, epoche) else {
            return false;
        };
        let Ok(mut stand) = self.stand.lock() else {
            return false;
        };
        *stand = Some((epoche, punkte));
        true
    }

    fn gueltige_epoche(&self, roh: &[u8]) -> Option<EpochId> {
        if roh.len() > MAX_ANKUENDIGUNG_BYTES {
            return None;
        }
        let ankuendigung = borsh::from_slice::<Epochenankuendigung>(roh).ok()?;
        // ⚑ **Erst prüfen, dann glauben.** `epoche()` allein ist eine
        // Behauptung; hier wird gegen genau diese Behauptung geprüft,
        // und die Unterschrift deckt sie mit.
        let behauptet = ankuendigung.epoche();
        ankuendigung.pruefe(self.erwartet, behauptet).ok()?;
        Some(behauptet)
    }

    fn zuruecksetzen(&self) {
        if let Ok(mut stand) = self.stand.lock() {
            *stand = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_siegel::Epochenschluessel;
    use myl_types::bls::BlsSecretKey;

    fn konsens(n: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[n; 32]).expect("Schluessel")
    }

    fn endpunkt_von(k: &BlsSecretKey) -> Endpunkt {
        myl_siegel::endpunkt_aus_schluessel(&k.public_key().expect("pk"))
    }

    fn ankuendigung(k: &BlsSecretKey, epoche: EpochId) -> Vec<u8> {
        let s = Epochenschluessel::probe(epoche, [7u8; 32]);
        borsh::to_vec(&Epochenankuendigung::neu(k, &s).expect("ankuendigen")).expect("kodieren")
    }

    /// Eine gültige Ankündigung des erwarteten Knotens gilt.
    #[test]
    fn der_erwartete_knoten_kommt_durch() {
        let k = konsens(1);
        let g = Betreibergegenstelle::neu(endpunkt_von(&k));
        assert!(g.nachschlagen(1).is_none(), "vor der Ankuendigung weiss er nichts");
        assert!(g.ankuendigen(&ankuendigung(&k, EpochId(3)), EpochId(3)));
        assert_eq!(g.angekuendigt(), Some(EpochId(3)));
        assert!(g.nachschlagen(1).is_some(), "die Punkte kamen nicht an");
    }

    /// ⚑ **Ein fremder Knoten kommt nicht durch, auch mit gültiger
    /// Unterschrift.**
    ///
    /// **Das ist die Zeile, an der die ganze Schicht hängt.** Wer den
    /// Ausweis der Leitung lesen kann, darf hereinreden; er wird damit
    /// nicht zum Knoten. Ohne diese Prüfung wäre das Siegel Theater.
    #[test]
    fn ein_fremder_knoten_kommt_nicht_durch() {
        let g = Betreibergegenstelle::neu(endpunkt_von(&konsens(1)));
        let fremd = konsens(2);
        assert!(
            !g.ankuendigen(&ankuendigung(&fremd, EpochId(3)), EpochId(3)),
            "ein fremder Konsensschluessel wurde als Gegenstelle angenommen"
        );
        assert!(g.nachschlagen(1).is_none());
    }

    /// ⚑ **Eine echte Ankündigung aus einer anderen Epoche gilt nicht.**
    /// Sonst wäre sie ein Weg, die Rotation zurückzudrehen.
    #[test]
    fn eine_ankuendigung_aus_fremder_epoche_gilt_nicht() {
        let k = konsens(1);
        let g = Betreibergegenstelle::neu(endpunkt_von(&k));
        assert!(
            !g.ankuendigen(&ankuendigung(&k, EpochId(3)), EpochId(4)),
            "eine Ankuendigung fuer Epoche 3 galt in Epoche 4"
        );
    }

    /// Unsinn und Übergrosses fallen vor dem Zerlegen heraus.
    #[test]
    fn unsinn_faellt_heraus() {
        let g = Betreibergegenstelle::neu(endpunkt_von(&konsens(1)));
        assert!(!g.ankuendigen(b"", EpochId(1)));
        assert!(!g.ankuendigen(b"keine Ankuendigung", EpochId(1)));
        assert!(
            !g.ankuendigen(&vec![0u8; MAX_ANKUENDIGUNG_BYTES + 1], EpochId(1)),
            "ueber dem Deckel wurde trotzdem zerlegt"
        );
    }

    /// ⚑ **Nach dem Epochenwechsel steht kein alter Punkt mehr da.**
    #[test]
    fn zuruecksetzen_raeumt_den_platz() {
        let k = konsens(1);
        let g = Betreibergegenstelle::neu(endpunkt_von(&k));
        assert!(g.ankuendigen(&ankuendigung(&k, EpochId(3)), EpochId(3)));
        g.zuruecksetzen();
        assert!(g.angekuendigt().is_none(), "der alte Punkt blieb stehen");
        assert!(g.nachschlagen(1).is_none());
    }
}
