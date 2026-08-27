//! Blockstruktur — Whitepaper Kap. 3.5, Anhang A.5.
//!
//! Ein Block enthält Transaktionen, PoI-Bündel, Challenges und
//! Verdicts (Anhang A.5) sowie die Epochen-Metadaten, die ihn in die
//! Kette einhängen.
//!
//! ## Kanonische Typen statt Dubletten (Fund A8)
//!
//! Bis v0.4.0 definierte diese Datei **eigene** Fassungen von
//! `PoiBundle`, `Challenge` und `Verdict` — mit anderen Feldern als die
//! Typen, die die übrigen Komponenten tatsächlich produzieren:
//!
//! | hier (alt) | kanonisch |
//! |---|---|
//! | `PoiBundle { segment_id, commitment_hash, pod_id: [u8;32], signature: [u8;96] }` | `myl_types::PoIBundle { epoch, pod, segments_root, vtfe_claimed, aggregate_sig }` |
//! | `Challenge { segment_id, first_divergence, challenger, accused }` | `myl_types::Challenge` (mit beiden Pods **und** beiden Hashes) |
//! | `Verdict { segment_id, winner, loser, slash_amount }` | `myl_ledger::Verdict { segment_id, miner, checker, outcome }` |
//!
//! Die Folge war eine stille Integrationslücke: `myl-pod` erzeugt
//! `myl_types::PoIBundle` (das Epochen-Aggregat aus Anhang A.1), aber
//! `Block::add_poi_bundle` nahm eine per-Segment-Struktur — der Pfad
//! Pod → Block war nicht verdrahtet, obwohl beide Seiten als
//! „vollständig" geführt wurden. Ebenso hätte kein `Verdict` des
//! Verifiers je in den Ledger gebucht werden können.
//!
//! Diese Datei definiert deshalb keine Protokolltypen mehr, sondern
//! verwendet die kanonischen. Rohe `[u8; 32]`/`[u8; 96]`-Felder sind
//! durch die Newtypes aus `myl-types` ersetzt — genau dafür gibt es
//! SHARED_TYPES.
//!
//! **Konsens-Feld:** Die Blockkodierung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_ledger::transitions::Verdict;
use myl_types::challenge::Challenge;
use myl_types::core_types::PoIBundle;
use myl_types::hash::Hash;
use myl_types::ids::{Address, MinerId};

/// Wie viele Blöcke auf eine Epoche gehen.
///
/// **Abgeleitet, nicht gesetzt:** Epochenlänge geteilt durch Blockzeit,
/// also 3600 s / 2 s. Beide sind Governance-Parameter
/// (`myl_governance::Parameter::{Epochenlaenge, Blockzeit}`), und ein
/// Test dort hält diese Zahl gegen sie.
///
/// ⚑ **Der Wert steht hier als Konstante und nicht als Abfrage der
/// Registry**, und das ist Absicht: Die Zuordnung Höhe → Epoche geht in
/// die Blockprüfung ein, und eine Blockprüfung, die einen abstimmbaren
/// Wert liest, macht die Gültigkeit eines Blocks von einem Zustand
/// abhängig, der sich ändern kann, während der Block schon in der Kette
/// steht. Wer die Epochenlänge ändern will, ändert damit einen
/// Konsensvertrag und nicht einen Parameter.
pub const BLOECKE_JE_EPOCHE: u64 = 1_800;

/// Die Epoche, zu der eine Blockhöhe gehört.
///
/// **Die Epoche folgt aus der Höhe, nicht aus der Uhr.** Eine
/// Zuordnung über Zeitstempel wäre nicht deterministisch: Zwei ehrliche
/// Knoten mit leicht verschiedenen Uhren ordneten denselben Block
/// verschiedenen Epochen zu, und damit fiele die Zustandswurzel
/// auseinander. Die Höhe ist die einzige Größe, über die sich alle
/// einig sind, bevor sie sich einig sein müssen.
///
/// **Was das kostet, gehört dazugesagt:** Stehen die Blöcke still,
/// stehen auch die Epochen still. Prägung, EMA und Fristen hängen dann
/// am Fortschritt der Kette und nicht an der Wanduhr. Für ein
/// Protokoll, dessen Arbeit ohnehin in Blöcken abgerechnet wird, ist das
/// die richtige Richtung; für eine Frist, die in Sekunden gedacht ist,
/// ist es eine Näherung, die man kennen muss.
pub fn epoche_fuer_hoehe(hoehe: u64) -> u64 {
    hoehe / BLOECKE_JE_EPOCHE
}

/// Der Kopf eines Blocks.
///
/// ⚑ **Hieß bis zum 2026-08-27 `BlockHeader`, und der Name war der
/// Fehler.** Er trug kein Höhenfeld, und die Probekette hat deshalb die
/// Höhe in `epoch` geschrieben — eine Doppelbelegung, die trägt, solange
/// eine Epoche ein Block ist, und bricht, sobald es das nicht mehr ist.
/// Ein Kopf, der die Höhe führt, heißt nicht nach der Epoche.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BlockHeader {
    /// Blockhöhe: die Stellung in der Kette, um genau eins wachsend.
    ///
    /// **Der Genesis-Block hat Höhe 0.** Die Höhe ist die Größe, an der
    /// ein Nachzügler seine Lücke abzählt, und sie ist die einzige, die
    /// das darf: Sie wächst um eins je Block, ohne Ausnahme.
    pub height: u64,
    /// Epochennummer.
    ///
    /// **Folgt aus der Höhe** ([`epoche_fuer_hoehe`]) und wird beim
    /// Übernehmen dagegen geprüft. Sie steht trotzdem im Kopf, damit ein
    /// Block für sich lesbar bleibt — wer ihn aus dem Speicher holt,
    /// soll nicht erst die Umrechnung kennen müssen. Ein mitgeführter
    /// Wert, der geprüft wird, ist keine zweite Wahrheit, sondern eine
    /// Prüfsumme; einer, der es nicht wird, ist eine Einladung.
    pub epoch: u64,
    /// Vorheriger Block-Hash.
    pub prev_block_hash: Hash,
    /// Zeitstempel (Unix-Millisekunden).
    pub timestamp_ms: u64,
    /// Commitment über den Ledger-Zustand **nach** Anwendung dieses
    /// Blocks (`myl_ledger::LedgerState::commitment()`).
    ///
    /// Ohne dieses Feld kann ein Validator nur prüfen, ob die Bytes des
    /// Blocks gleich sind — nicht, ob der Vorschlagende die
    /// Zustandsübergänge korrekt angewendet hat. Ein Leader könnte einen
    /// syntaktisch einwandfreien Block mit falsch gebuchtem Slashing
    /// vorschlagen, und das Komitee hätte nichts, woran es den Fehler
    /// festmachen könnte.
    pub state_root: Hash,
}

/// Eine Burn-Transaktion (MYL → Credits).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct BurnTx {
    /// Absender-Adresse.
    ///
    /// `Address`, nicht `MinerId`: Der Ledger führt Konten unter
    /// Adressen (`Address = SHA-256(komprimierter BLS-Public-Key)`),
    /// und wer MYL verbrennt, muss kein Miner sein.
    pub sender: Address,
    /// Betrag in MYL-Kleinstbeträgen.
    pub amount: u64,
}

/// Transaktionstypen im Block.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Transaction {
    /// Burn-Transaktion (MYL → Credits).
    Burn(BurnTx),
}

/// Ein Block im Myelith-Netzwerk.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Block {
    /// Epochen-Metadaten.
    pub header: BlockHeader,
    /// Transaktionen.
    pub txs: Vec<Transaction>,
    /// PoI-Bündel (Anhang A.1 — je Epoche und Pod eines).
    pub poi_bundles: Vec<PoIBundle>,
    /// Challenges (Anhang A.4).
    pub challenges: Vec<Challenge>,
    /// Verdicts — dieselbe Struktur, die `myl_ledger::apply_verdict`
    /// verarbeitet.
    pub verdicts: Vec<Verdict>,
}

impl Block {
    /// Erstellt einen neuen, leeren Block.
    pub fn new(header: BlockHeader) -> Self {
        Self {
            header,
            txs: Vec::new(),
            poi_bundles: Vec::new(),
            challenges: Vec::new(),
            verdicts: Vec::new(),
        }
    }

    /// Fügt eine Transaktion hinzu.
    pub fn add_transaction(&mut self, tx: Transaction) {
        self.txs.push(tx);
    }

    /// Fügt ein PoI-Bündel hinzu.
    pub fn add_poi_bundle(&mut self, bundle: PoIBundle) {
        self.poi_bundles.push(bundle);
    }

    /// Fügt eine Challenge hinzu.
    pub fn add_challenge(&mut self, challenge: Challenge) {
        self.challenges.push(challenge);
    }

    /// Fügt ein Verdict hinzu.
    pub fn add_verdict(&mut self, verdict: Verdict) {
        self.verdicts.push(verdict);
    }

    /// Berechnet den Block-Hash (SHA-256 über kanonisches Borsh).
    ///
    /// Der Hash deckt `state_root` mit ab — eine Manipulation der
    /// gebuchten Zustandsübergänge verändert damit den Hash, über den
    /// abgestimmt wird.
    pub fn hash(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Borsh-Serialisierung sollte nicht fehlschlagen");
        Hash::sha256(&bytes)
    }

    /// Gibt die Gesamtanzahl der Einträge zurück.
    pub fn total_entries(&self) -> usize {
        self.txs.len() + self.poi_bundles.len() + self.challenges.len() + self.verdicts.len()
    }

    /// Strukturelle Plausibilitätsprüfung der enthaltenen Challenges.
    ///
    /// Verwirft Blöcke mit offensichtlich unsinnigen Challenges (gleiche
    /// Miner, gleiche Hashes), ohne die Segment-Spuren zu kennen. Die
    /// vollständige Prüfung leistet VERIFICATION.
    pub fn validate_challenges(&self) -> Result<(), myl_types::ChallengeStructureError> {
        for c in &self.challenges {
            c.validate_structure()?;
        }
        Ok(())
    }

    /// Alle in diesem Block genannten Miner (aus Challenges und Verdicts
    /// lässt sich nicht direkt auf `MinerId` schließen — Verdicts führen
    /// Adressen). Liefert die Miner der Challenges.
    pub fn challenged_miners(&self) -> Vec<MinerId> {
        let mut out = Vec::new();
        for c in &self.challenges {
            out.push(c.primary_miner);
            out.push(c.redundant_miner);
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_ledger::transitions::VerdictOutcome;
    use myl_types::bls::BlsSignature;
    use myl_types::ids::{EpochId, MerkleRoot, PodId, SegmentId};

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    fn test_meta() -> BlockHeader {
        BlockHeader {
            // Höhe und Epoche passen zueinander: 42 · 1800.
            height: 42 * BLOECKE_JE_EPOCHE,
            epoch: 42,
            prev_block_hash: test_hash(0),
            timestamp_ms: 1_700_000_000_000,
            state_root: test_hash(99),
        }
    }

    fn test_bundle() -> PoIBundle {
        PoIBundle {
            epoch: EpochId(42),
            pod: PodId::new([7u8; 32]),
            segments_root: MerkleRoot::new([8u8; 32]),
            vtfe_claimed: 1234,
            aggregate_sig: BlsSignature([9u8; 96]),
        }
    }

    fn test_challenge() -> Challenge {
        Challenge {
            segment_id: SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: MinerId::new([1u8; 32]),
            redundant_miner: MinerId::new([2u8; 32]),
            primary_hash: test_hash(1),
            redundant_hash: test_hash(2),
            timestamp_ms: 1_700_000_000_000,
        }
    }

    fn test_verdict() -> Verdict {
        Verdict {
            segment_id: SegmentId::new([1u8; 32]),
            miner: Address::new([3u8; 32]),
            checker: Address::new([4u8; 32]),
            outcome: VerdictOutcome::SlashMiner,
        }
    }

    #[test]
    fn block_creation() {
        let block = Block::new(test_meta());
        assert_eq!(block.header.epoch, 42);
        assert_eq!(block.total_entries(), 0);
    }

    #[test]
    fn block_nimmt_kanonische_typen_auf() {
        let mut block = Block::new(test_meta());
        block.add_transaction(Transaction::Burn(BurnTx {
            sender: Address::new([5u8; 32]),
            amount: 1000,
        }));
        block.add_poi_bundle(test_bundle());
        block.add_challenge(test_challenge());
        block.add_verdict(test_verdict());

        assert_eq!(block.total_entries(), 4);
    }

    /// Der Kern von Fund A8: Was `myl-pod` produziert, muss in den Block
    /// passen. Vorher war `Block::add_poi_bundle` auf eine andere
    /// Struktur typisiert und der Pfad Pod → Block nicht verdrahtet.
    #[test]
    fn poi_buendel_aus_myl_types_passt_in_den_block() {
        let bundle: PoIBundle = test_bundle();
        let mut block = Block::new(test_meta());
        block.add_poi_bundle(bundle.clone());
        assert_eq!(block.poi_bundles[0], bundle);
    }

    /// Das Verdict des Blocks muss der Ledger direkt verarbeiten können.
    #[test]
    fn verdict_ist_der_ledger_typ() {
        let v: myl_ledger::transitions::Verdict = test_verdict();
        let mut block = Block::new(test_meta());
        block.add_verdict(v);
        assert_eq!(block.verdicts[0].outcome, VerdictOutcome::SlashMiner);
    }

    #[test]
    fn block_hash_deterministisch() {
        let block = Block::new(test_meta());
        assert_eq!(block.hash(), block.hash());
    }

    #[test]
    fn block_hash_aendert_sich_mit_inhalt() {
        let mut a = Block::new(test_meta());
        let b = a.clone();
        a.add_challenge(test_challenge());
        assert_ne!(a.hash(), b.hash());
    }

    /// `state_root` muss in den Block-Hash eingehen — sonst könnte ein
    /// Leader die gebuchten Zustandsübergänge fälschen, ohne dass sich
    /// der Hash ändert, über den abgestimmt wird.
    #[test]
    fn state_root_geht_in_den_blockhash_ein() {
        let a = Block::new(test_meta());
        let mut meta_b = test_meta();
        meta_b.state_root = test_hash(100);
        let b = Block::new(meta_b);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn prev_block_hash_geht_in_den_blockhash_ein() {
        let a = Block::new(test_meta());
        let mut meta_b = test_meta();
        meta_b.prev_block_hash = test_hash(50);
        let b = Block::new(meta_b);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn block_borsh_roundtrip() {
        let mut block = Block::new(test_meta());
        block.add_poi_bundle(test_bundle());
        block.add_challenge(test_challenge());
        block.add_verdict(test_verdict());

        let bytes = borsh::to_vec(&block).unwrap();
        let decoded: Block = borsh::from_slice(&bytes).unwrap();
        assert_eq!(block, decoded);
    }

    #[test]
    fn unsinnige_challenge_wird_erkannt() {
        let mut block = Block::new(test_meta());
        let mut c = test_challenge();
        c.redundant_miner = c.primary_miner;
        block.add_challenge(c);
        assert!(block.validate_challenges().is_err());
    }

    #[test]
    fn gueltige_challenges_passieren() {
        let mut block = Block::new(test_meta());
        block.add_challenge(test_challenge());
        assert!(block.validate_challenges().is_ok());
    }

    #[test]
    fn challenged_miners_sind_sortiert_und_eindeutig() {
        let mut block = Block::new(test_meta());
        block.add_challenge(test_challenge());
        block.add_challenge(test_challenge());
        let miners = block.challenged_miners();
        assert_eq!(miners.len(), 2);
        assert!(miners.windows(2).all(|w| w[0] < w[1]));
    }
}
