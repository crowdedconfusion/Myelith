//! Wie eine Modellfassung fortgeschrieben wird (Whitepaper Kap. 7.2).
//!
//! # ⚑ Die Kette hält keine Gewichte, also kann sie nicht rechnen
//!
//! Eine neue Modellfassung entsteht, indem bestaetigte Δm auf die alten
//! Gewichte summiert werden. **Der Konsens kann das nicht tun:** Er
//! haelt Kennungen und Wurzeln, keine 357 Millionen Zahlen.
//!
//! Was er tun kann, ist das **Rezept** festzuhalten: die alte Fassung
//! und die geordnete Liste der bestaetigten Δ-Commitments. Wer die
//! Gewichte hat, wendet sie an und rechnet die neue Wurzel nach.
//!
//! ⚑ **Damit ist die Versionsanhebung pruefbar, ohne dass die Kette
//! rechnet.** Wer behauptet, die neue Fassung sei eine andere, muss
//! zeigen, dass dieselben Deltas auf dieselben Gewichte etwas anderes
//! ergeben, und das ist eine Rechnung, die jeder wiederholen kann.
//!
//! # ⚑ Was hier NICHT steht
//!
//! Die Summe selbst. `integer_llm_kernels::optimierer::aggregiere`
//! summiert ganzzahlig, ordnungsfrei und saettigt genau einmal am Ende.
//! **Die Reihenfolge hier ist nur fuer die Kennung**, nicht fuer die
//! Arithmetik: Waere sie fuer die Arithmetik noetig, waere die
//! Aggregation nicht ordnungsfrei, und zwei Knoten mit verschieden
//! sortierten Eingaben kaemen zu verschiedenen Gewichten.

use crate::hash::Hash;
use crate::ids::MerkleRoot;

/// Die naechste Modellfassung aus der alten und den bestaetigten Deltas.
///
/// ⚑ **Eine leere Liste laesst die Fassung stehen.** Eine Epoche ohne
/// bestaetigtes Training hat das Modell nicht veraendert; eine neue
/// Kennung dafuer waere eine Aenderung, die es nicht gab, und jeder, der
/// die Gewichte nachrechnete, faende die alten.
pub fn naechste_version(alt: &MerkleRoot, deltas: &[Hash]) -> MerkleRoot {
    if deltas.is_empty() {
        return *alt;
    }
    let mut roh = Vec::with_capacity(32 + 8 + deltas.len() * 32);
    roh.extend_from_slice(b"myl-modellfortschreibung-v1");
    roh.extend_from_slice(alt.as_bytes());
    roh.extend_from_slice(&(deltas.len() as u64).to_le_bytes());
    for d in deltas {
        roh.extend_from_slice(d.as_bytes());
    }
    MerkleRoot::new(Hash::sha256(&roh).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_deltas_bleibt_die_fassung_stehen() {
        let alt = MerkleRoot::new([7u8; 32]);
        assert_eq!(naechste_version(&alt, &[]), alt);
    }

    #[test]
    fn ein_delta_hebt_die_fassung() {
        let alt = MerkleRoot::new([7u8; 32]);
        let neu = naechste_version(&alt, &[Hash([1u8; 32])]);
        assert_ne!(neu, alt);
    }

    /// ⚑ **Die alte Fassung geht ein.** Sonst ergaeben dieselben Deltas
    /// auf verschiedenen Staenden dieselbe neue Kennung.
    #[test]
    fn die_alte_fassung_geht_ein() {
        let d = [Hash([1u8; 32])];
        assert_ne!(
            naechste_version(&MerkleRoot::new([7u8; 32]), &d),
            naechste_version(&MerkleRoot::new([8u8; 32]), &d)
        );
    }

    /// Reihenfolge und Zahl der Deltas gehen ein.
    #[test]
    fn reihenfolge_und_zahl_gehen_ein() {
        let alt = MerkleRoot::new([7u8; 32]);
        let a = Hash([1u8; 32]);
        let b = Hash([2u8; 32]);
        assert_ne!(naechste_version(&alt, &[a, b]), naechste_version(&alt, &[b, a]));
        assert_ne!(naechste_version(&alt, &[a]), naechste_version(&alt, &[a, a]));
    }

    /// ⚑ **Zweimal dieselbe Epoche ergibt dieselbe Fassung.** Ohne das
    /// koennten zwei Knoten nach demselben Block verschiedene Fassungen
    /// fuehren.
    #[test]
    fn determinismus() {
        let alt = MerkleRoot::new([7u8; 32]);
        let d = [Hash([1u8; 32]), Hash([2u8; 32])];
        assert_eq!(naechste_version(&alt, &d), naechste_version(&alt, &d));
    }
}
