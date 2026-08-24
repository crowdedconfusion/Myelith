//! Nutzlastprüfung des Knotens: die Schichtgrenze, die nur er auflösen kann.
//!
//! # Warum das hier steht und nicht in `myl-net`
//!
//! `myl-net` ist die Netzschicht (L0). Die Typen von Blöcken und
//! Transaktionen liegen in `myl-consensus` (L1). Würde die Netzschicht
//! sie kennen, wäre die Schichtung umgekehrt, und jeder, der nur ein
//! Netz braucht, zöge den Konsens mit.
//!
//! Deshalb prüft `myl-net` diese beiden Topics nur auf ihre Größe und
//! reicht den Rest über [`myl_net::validation::PayloadValidator`] nach
//! oben. **Der Knoten ist die vorgesehene Stelle**, weil er als einziger
//! beide Seiten kennt.
//!
//! Dass dieser Weg seit dem 2026-08-18 dokumentiert und bis zum
//! 2026-08-24 unbenutzt war, ist Fund 55: Über `run_node` gab es keinen
//! Parameter dafür. Dieses Modul ist der erste Abnehmer.
//!
//! # ⚑ Was hier geprüft wird, und was ausdrücklich nicht
//!
//! **Geprüft wird die Form, nicht die Gültigkeit.** Eine Nachricht muss
//! sich als der Typ lesen lassen, den ihr Topic ankündigt, und sie darf
//! keine Anhängsel tragen.
//!
//! **Nicht geprüft** werden Signaturen, Epochengültigkeit, Stake, Höhe
//! oder Vorgängerblock. Das ist kein Versäumnis, sondern die Grenze des
//! Möglichen an dieser Stelle: Diese Fragen brauchen **Kettenzustand**,
//! und der Knoten hat noch keinen. Ein Urteil über Gültigkeit ohne
//! Zustand wäre ein geratenes Urteil, und ein geratenes `Reject`
//! verwirft ehrliche Nachrichten und bestraft ihren Absender im
//! Gossipsub-Scoring.
//!
//! # ⚑ Fund 57: Auch für Blöcke ist der Parse fast nur eine Längenprüfung
//!
//! Hier stand zuerst: „`Block` und `Transaction` sind **keine** Typen
//! aus lauter festen Feldern, sie tragen Vektoren, deren Längenköpfe
//! stimmen müssen. Für sie hat der Parse also Zähne."
//!
//! **Das ist falsch, und der Test, der es messen sollte, hat es
//! widerlegt:** Von 20 000 verstümmelten Blöcken kommen rund **88 %
//! durch**.
//!
//! Der Grund ist beim Nachrechnen offensichtlich. Ein `Block` mit
//! wenigen Einträgen besteht fast nur aus Feldern fester Breite: die
//! Epochen-Metadaten allein sind 80 Bytes (zwei `u64`, zwei `Hash`),
//! dazu fünf Vektoren, die als leere Längenköpfe je vier Bytes belegen.
//! **Empfindlich sind nur diese Längenköpfe**, also gut ein Siebtel der
//! Bytes. Ein gekipptes Bit trifft mit hoher Wahrscheinlichkeit ein
//! festes Feld, und dann liest sich die Nachricht weiterhin als Block,
//! nur mit anderem Inhalt.
//!
//! Das ist dieselbe Eigenschaft, die Fund 45 für PoI-Bündel festgehalten
//! hat, nur abgeschwächt statt vollständig. **Die Lehre ist, dass die
//! Abschwächung nicht reicht, um von „Prüfung" zu sprechen.**
//!
//! Was folgt daraus? Nicht, diesen Validator wegzulassen: Er fängt
//! Nachrichten ab, die sich gar nicht als Block lesen lassen, er
//! erzwingt „kein Anhängsel", und ein `Reject` bestraft den Absender im
//! Gossipsub-Scoring. Wohl aber, **nicht mit ihm zu rechnen**. Die
//! Verteidigung für Blöcke ist die Signatur des Vorschlagenden gegen die
//! Validator-Registry, und die braucht Kettenzustand, den dieser Knoten
//! noch nicht führt.
//!
//! Der Test `fund_57_verstuemmelte_bloecke_kommen_meist_durch` hält die
//! gemessene Zahl fest. **Er schlägt fehl, sobald jemand eine echte
//! Prüfung ergänzt**, und zwingt damit zur Aktualisierung dieser Stelle.

use borsh::BorshDeserialize;
use myl_consensus::block::{Block, Transaction};
use myl_net::{GossipTopic, PayloadValidator};

/// Die Nutzlastprüfung des Knotens.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProtokollValidator;

/// Liest einen Typ aus Bytes und verlangt, dass **nichts übrig bleibt**.
///
/// Der Nachsatz ist der wichtigere Teil. `from_slice` allein akzeptiert
/// bei manchen Typen Anhängsel, und ein Anhängsel ist ein Kanal: Zwei
/// Nachrichten mit gleichem Inhalt und verschiedenem Anhang haben
/// verschiedene Nachrichten-Ids und laufen beide durchs Netz.
fn liest_sich_vollstaendig_als<T: BorshDeserialize>(daten: &[u8]) -> bool {
    let mut rest = daten;
    match T::deserialize(&mut rest) {
        Ok(_) => rest.is_empty(),
        Err(_) => false,
    }
}

impl PayloadValidator for ProtokollValidator {
    fn validate(&self, topic: GossipTopic, data: &[u8]) -> bool {
        match topic {
            GossipTopic::Blocks => liest_sich_vollstaendig_als::<Block>(data),
            GossipTopic::Transactions => liest_sich_vollstaendig_als::<Transaction>(data),
            // Die übrigen Topics prüft `myl-net` bereits strukturell;
            // hier nichts zu ergänzen, solange kein Kettenzustand da ist.
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::{BurnTx, EpochMeta};
    use myl_types::hash::Hash;

    fn beispielblock() -> Block {
        let mut b = Block::new(EpochMeta {
            epoch: 3,
            prev_block_hash: Hash::sha256(b"vorgaenger"),
            timestamp_ms: 1_700_000_000_000,
            state_root: Hash::sha256(b"zustand"),
        });
        // Ein Block mit Inhalt: Die Vektoren sind der Grund, warum der
        // Borsh-Parse hier mehr leistet als eine Längenprüfung (Fund 45).
        b.txs.push(Transaction::Burn(BurnTx {
            sender: myl_types::ids::Address::new([4u8; 32]),
            amount: 1_000,
        }));
        b
    }

    #[test]
    fn ein_echter_block_kommt_durch() {
        let daten = borsh::to_vec(&beispielblock()).unwrap();
        assert!(ProtokollValidator.validate(GossipTopic::Blocks, &daten));
    }

    #[test]
    fn zufallsbytes_werden_abgelehnt() {
        assert!(!ProtokollValidator.validate(GossipTopic::Blocks, &[0xAB; 64]));
        assert!(!ProtokollValidator.validate(GossipTopic::Blocks, &[]));
    }

    #[test]
    fn ein_block_mit_anhaengsel_wird_abgelehnt() {
        // Ein Anhängsel ändert die Nachrichten-Id, nicht den Inhalt:
        // derselbe Block liefe beliebig oft durchs Netz.
        let mut daten = borsh::to_vec(&beispielblock()).unwrap();
        daten.push(0);
        assert!(
            !ProtokollValidator.validate(GossipTopic::Blocks, &daten),
            "ein Anhängsel hinter einem gültigen Block kam durch"
        );
    }

    #[test]
    fn eine_transaktion_wird_nicht_als_block_gelesen() {
        let tx = Transaction::Burn(BurnTx {
            sender: myl_types::ids::Address::new([4u8; 32]),
            amount: 100,
        });
        let daten = borsh::to_vec(&tx).unwrap();
        assert!(ProtokollValidator.validate(GossipTopic::Transactions, &daten));
        assert!(
            !ProtokollValidator.validate(GossipTopic::Blocks, &daten),
            "eine Transaktion las sich als Block"
        );
    }

    #[test]
    fn fund_57_verstuemmelte_bloecke_kommen_meist_durch() {
        // ⚑ Dieser Test ist als Gegenprobe geschrieben worden und hat
        // die Doku widerlegt, die er belegen sollte. Er hält jetzt die
        // **gemessene Schwäche** fest, damit niemand aus „der Knoten
        // prüft Blöcke" schließt, das Netz filtere schon.
        //
        // Schlägt er fehl, ist das gute Nachricht: Dann prüft jemand
        // mehr als die Form, und der Modulkopf gehört nachgezogen.
        let gut = borsh::to_vec(&beispielblock()).unwrap();
        let mut zustand: u64 = 0x9E3779B97F4A7C15;
        let mut durch = 0usize;
        const VERSUCHE: usize = 20_000;
        for _ in 0..VERSUCHE {
            zustand ^= zustand << 13;
            zustand ^= zustand >> 7;
            zustand ^= zustand << 17;
            let mut kaputt = gut.clone();
            let pos = (zustand as usize) % kaputt.len();
            kaputt[pos] ^= ((zustand >> 32) as u8) | 1;
            if ProtokollValidator.validate(GossipTopic::Blocks, &kaputt) {
                durch += 1;
            }
        }
        let anteil = durch * 100 / VERSUCHE;
        println!(
            "[Messung] {durch} von {VERSUCHE} verstümmelten Blöcken kamen durch ({anteil} %)"
        );
        assert!(
            anteil > 50,
            "nur {anteil} % kamen durch: Wenn die Formprüfung inzwischen mehr \
             als die Hälfte abfängt, hat jemand eine echte Prüfung ergänzt. \
             Dann gehört Fund 57 im Modulkopf nachgezogen, statt diesen Test \
             anzupassen"
        );
        // Und die Gegenrichtung: Ganz zahnlos ist der Parse nicht, sonst
        // wäre er nur Aufwand. Er fängt die Längenköpfe.
        assert!(
            durch < VERSUCHE,
            "kein einziger verstümmelter Block wurde abgelehnt: dann prüft \
             dieser Validator gar nichts"
        );
    }

    #[test]
    fn fremde_topics_bleiben_unberuehrt() {
        // Der Knoten darf hier nicht strenger sein als myl-net, sonst
        // verwirft er, was die Netzschicht bereits geprüft hat.
        assert!(ProtokollValidator.validate(GossipTopic::PoiBundles, &[0xFF; 32]));
        assert!(ProtokollValidator.validate(GossipTopic::Challenges, &[]));
    }
}
