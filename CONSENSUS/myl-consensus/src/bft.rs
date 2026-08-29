//! BFT-Kernprotokoll — Whitepaper Kap. 3.5, Anhang A.2.
//!
//! Propose/Vote/Commit-Zyklus für BFT-Blockproduktion. Safety unter
//! f < 1/3 byzantinischem **Stimmgewicht**.
//!
//! ## Was eine Nachricht passieren muss (Fund A3)
//!
//! Bis v0.3.6 trugen `Propose`, `Vote` und `Commit` **keine Signatur**,
//! und `BftState` kannte das Komitee nicht. Der Zustandsautomat zählte
//! nur Nachrichten: ein einzelner Angreifer erreichte den Threshold mit
//! 15 erfundenen Miner-IDs. `BftError::InvalidSignature` existierte als
//! „(Placeholder)" und wurde nirgends zurückgegeben.
//!
//! Jede eingehende Nachricht durchläuft jetzt vier Prüfungen, in dieser
//! Reihenfolge (billig vor teuer — die BLS-Verifikation ist die
//! teuerste und darf nicht als DoS-Fläche vor den Filtern stehen):
//!
//! 1. **Runde** — gehört die Nachricht zu dieser Runde?
//! 2. **Mitgliedschaft** — ist der Absender im stimmberechtigten Komitee?
//! 3. **Duplikat** — hat er in dieser Runde schon gestimmt?
//! 4. **Signatur** — gilt die BLS-Signatur über die kanonische,
//!    domain-getrennte Botschaft (siehe [`crate::signing`]) gegen den
//!    registrierten Schlüssel des Absenders?
//!
//! ## Threshold nach Stimmgewicht, nicht nach Köpfen
//!
//! Gezählt wird das Stimmgewicht aus [`crate::voting_weight`] (Stake und
//! nachgewiesene Inferenzarbeit), nicht die Anzahl der Nachrichten. Das
//! ist der Punkt, an dem die Kopplung „nützliche Arbeit sichert den
//! Konsens" (Kap. 3.5.2) wirksam wird — vorher wurde das Stimmgewicht
//! zwar berechnet, aber nirgends verwendet.
//!
//! **Konsens-Feld:** Das BFT-Protokoll ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use crate::round_change::PolkaCertificate;
use crate::signing::{commit_message, propose_message, propose_pol_message, vote_message};
use crate::validator::VotingSet;
use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::BlsSignature;
use myl_types::hash::Hash;
use myl_types::ids::MinerId;
use std::collections::HashMap;

/// BFT-Rundennummer.
pub type Round = u64;

/// Status einer BFT-Runde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundStatus {
    /// Warte auf Propose vom Leader.
    WaitingPropose,
    /// Propose empfangen, sammle Votes.
    CollectingVotes,
    /// Quorum an Votes erreicht, warte auf Commits.
    CollectingCommits,
    /// Block commitet (abgeschlossen).
    Committed,
}

/// Eine Propose-Nachricht vom Leader.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Propose {
    /// Runde.
    pub round: Round,
    /// Vorgeschlagener Block-Hash.
    pub block_hash: Hash,
    /// Leader (Miner-ID).
    pub leader: MinerId,
    /// BLS-Signatur des Leaders über [`crate::signing::propose_message`].
    pub signature: BlsSignature,
}

/// Eine Vote-Nachricht von einem Validator.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Vote {
    /// Runde.
    pub round: Round,
    /// Block-Hash, für den gestimmt wird.
    pub block_hash: Hash,
    /// Voter (Miner-ID).
    pub voter: MinerId,
    /// BLS-Signatur des Voters über [`crate::signing::vote_message`].
    pub signature: BlsSignature,
}

/// Eine Commit-Nachricht von einem Validator.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Commit {
    /// Runde.
    pub round: Round,
    /// Block-Hash, der commitet wird.
    pub block_hash: Hash,
    /// Committer (Miner-ID).
    pub committer: MinerId,
    /// BLS-Signatur des Committers über [`crate::signing::commit_message`].
    pub signature: BlsSignature,
}

/// Eine BFT-Nachricht, wie sie über das Netz geht.
///
/// # Warum es diesen Typ gibt
///
/// Propose, Vote und Commit reisen über **ein** Gossip-Topic
/// (`/myelith/consensus/1`, definiert in `myl-net`, außerhalb dieses
/// Crates). Ein Topic
/// trägt eine Nutzlastklasse, also braucht es einen Typ, der alle drei
/// umfasst und beim Lesen sagt, welcher davon ankam.
///
/// **Ein Topic und nicht drei**, weil die drei Nachrichtenarten
/// derselben Runde angehören und dieselbe Zustellung brauchen: Wer die
/// Votes bekommt, aber die Commits nicht, hängt. Drei Meshes, die
/// auseinanderlaufen können, wären drei Wege, dieselbe Runde
/// steckenzubleiben.
///
/// # ⚑ Was der Borsh-Parse hier leistet, und was nicht
///
/// **Fast nichts**, und das ist dieselbe Eigenschaft wie in Fund 45 und
/// Fund 57: Alle Felder haben feste Breite (Runde 8, Hash 32, Miner-Id
/// 32, Signatur 96), also ist jede Bytefolge der richtigen Länge mit
/// gültiger Enum-Marke eine lesbare Nachricht.
///
/// **Der Unterschied zu Fund 45 ist, dass die eigentliche Prüfung hier
/// erreichbar ist.** Bei PoI-Bündeln blieb die Aggregatsignatur
/// ungeprüft, weil niemand sie prüfte. Hier prüfen
/// [`BftState::receive_propose`], [`BftState::receive_vote`] und
/// [`BftState::receive_commit`] jede Nachricht gegen Runde,
/// Mitgliedschaft, Duplikat und BLS-Signatur, bevor sie zählt. Der Parse ist die Eingangstür, nicht die Prüfung.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Konsensnachricht {
    Propose(Propose),
    Vote(Vote),
    Commit(Commit),
    /// Ein Vorschlag, der ein Polka-Zertifikat mitbringt.
    ///
    /// Ein Leader, der einen in einer früheren Runde gesperrten Block
    /// erneut vorschlägt, muss belegen, dass die Sperre gefahrlos
    /// gelöst werden kann. Das Zertifikat ist dieser Beleg.
    ///
    /// **Eine vierte Marke statt eines Feldes an [`Propose`]**, und das
    /// aus demselben Grund, aus dem [`crate::signing::DST_PROPOSE_POL`]
    /// ein eigenes Präfix bekam statt einer Erweiterung: Der Zusatz ist
    /// **additiv**. Die Kodierung des einfachen Propose bleibt Byte für
    /// Byte dieselbe, und keine zuvor erzeugte Signatur wird ungültig.
    ///
    /// Die Signatur eines solchen Vorschlags geht über
    /// [`crate::signing::propose_pol_message`], deckt also die Runde des
    /// Zertifikats mit ab (⚑ Fund 66).
    ProposeMitPolka(Propose, PolkaCertificate),
    /// Der Beleg, dass ein Quorum einen Block commitet hat.
    ///
    /// ⚑ **Die fünfte Marke, und wieder additiv** (Fund 67): hinten
    /// angehängt, also bleibt die Kodierung der vier bisherigen Marken
    /// Byte für Byte dieselbe und keine erzeugte Signatur wird ungültig.
    ///
    /// Anders als die vier anderen ist diese Nachricht **nicht an die
    /// Runde des Empfängers gebunden**. Sie ist der einzige Weg zurück
    /// für einen Knoten, der allein vorauseilt: Seine Runde stimmt mit
    /// niemandem überein, also verwirft er jede einzelne Commit-Nachricht
    /// des Netzes, aber der Beleg gilt für sich. Siehe
    /// [`crate::round_change::Commitzertifikat`].
    Commitzertifikat(crate::round_change::Commitzertifikat),
}

impl Konsensnachricht {
    /// Die Runde, zu der diese Nachricht gehört.
    ///
    /// Erlaubt es dem Aufrufer, eine Nachricht der falschen Runde zu
    /// verwerfen, **ohne** sie erst dem Zustandsautomaten vorzulegen.
    ///
    /// ⚑ **Mit einer Ausnahme, und sie ist der Sinn der Sache.** Für
    /// [`Self::Commitzertifikat`] ist das hier die Runde, die der Beleg
    /// *bezeugt*, nicht eine Runde, in der der Empfänger stehen müsste.
    /// Wer danach filtert, wirft genau die Nachricht weg, die einen
    /// vorausgeeilten Knoten zurückholt (Fund 67). Prüfen lässt sich das
    /// nicht im Typ; deshalb steht es hier.
    pub fn runde(&self) -> Round {
        match self {
            Self::Propose(p) | Self::ProposeMitPolka(p, _) => p.round,
            Self::Vote(v) => v.round,
            Self::Commit(c) => c.round,
            Self::Commitzertifikat(z) => z.round,
        }
    }

    /// Der Absender, falls es einen einzelnen gibt.
    ///
    /// Nicht Teil der Signierbotschaft (siehe [`crate::signing`]), aber
    /// Teil der Nachricht: Die Prüfung braucht ihn, um den Schlüssel
    /// nachzuschlagen, gegen den sie verifiziert.
    ///
    /// ⚑ **`None` für [`Self::Commitzertifikat`]**, denn ein Aggregat hat
    /// keinen Absender, es hat Unterzeichner. Einen davon
    /// herauszugreifen, etwa den kleinsten, ergäbe eine zweite,
    /// erfundene Auskunft neben der wahren Liste, und der erste Leser,
    /// der sie für die ganze Wahrheit hält, prüft gegen einen einzelnen
    /// Schlüssel, wo ein Quorum zu prüfen wäre.
    pub fn absender(&self) -> Option<MinerId> {
        match self {
            Self::Propose(p) | Self::ProposeMitPolka(p, _) => Some(p.leader),
            Self::Vote(v) => Some(v.voter),
            Self::Commit(c) => Some(c.committer),
            Self::Commitzertifikat(_) => None,
        }
    }

    /// Für Protokollzeilen: welche Art Nachricht war das.
    pub fn art(&self) -> &'static str {
        match self {
            Self::Propose(_) => "propose",
            Self::ProposeMitPolka(_, _) => "propose-mit-polka",
            Self::Vote(_) => "vote",
            Self::Commit(_) => "commit",
            Self::Commitzertifikat(_) => "commit-zertifikat",
        }
    }
}

/// BFT-Zustand für eine Runde.
#[derive(Debug, Clone)]
pub struct BftState {
    /// Aktuelle Runde.
    pub round: Round,
    /// Status der Runde.
    pub status: RoundStatus,
    /// Leader für diese Runde.
    pub leader: MinerId,
    /// Vorgeschlagener Block-Hash (None wenn noch kein Propose).
    pub proposed_block: Option<Hash>,
    /// Empfangene Votes (Voter → Block-Hash).
    pub votes: HashMap<MinerId, Hash>,
    /// Empfangene Commits (Committer → Block-Hash).
    pub commits: HashMap<MinerId, Hash>,
    /// Stimmberechtigte Menge dieser Runde (Schlüssel und Gewichte).
    voting_set: VotingSet,
    /// Summe der Gewichte der eingegangenen Votes.
    vote_weight: u64,
    /// Summe der Gewichte der eingegangenen Commits.
    commit_weight: u64,
    /// Benötigtes Stimmgewicht (> 2/3 des Gesamtgewichts).
    threshold: u64,
}

/// Fehler im BFT-Protokoll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BftError {
    /// Falsche Runde (erwartet vs. bekommen).
    WrongRound { expected: Round, got: Round },
    /// Doppelte Nachricht von selbem Validator.
    DuplicateMessage,
    /// Propose von falschem Leader.
    WrongLeader,
    /// Vote/Commit für unbekannten Block.
    UnknownBlock,
    /// Ungültige Signatur unter dem registrierten Schlüssel des Absenders.
    InvalidSignature,
    /// Absender gehört nicht zur stimmberechtigten Menge dieser Runde.
    NotInCommittee,
    /// Die stimmberechtigte Menge ist leer — keine Runde möglich.
    EmptyCommittee,
}

impl std::fmt::Display for BftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongRound { expected, got } => {
                write!(f, "Falsche Runde: erwartet {}, bekommen {}", expected, got)
            }
            Self::DuplicateMessage => write!(f, "Doppelte Nachricht"),
            Self::WrongLeader => write!(f, "Propose von falschem Leader"),
            Self::UnknownBlock => write!(f, "Vote/Commit für unbekannten Block"),
            Self::InvalidSignature => write!(f, "Ungültige Signatur"),
            Self::NotInCommittee => write!(f, "Absender ist nicht stimmberechtigt"),
            Self::EmptyCommittee => write!(f, "Leere stimmberechtigte Menge"),
        }
    }
}

impl std::error::Error for BftError {}

impl BftState {
    /// Erstellt einen neuen BFT-Zustand für eine Runde.
    ///
    /// **Parameter:**
    /// - `round`: Rundennummer
    /// - `leader`: Leader für diese Runde (muss stimmberechtigt sein)
    /// - `voting_set`: stimmberechtigte Menge mit Schlüsseln und Gewichten
    ///
    /// **Fehler:** `EmptyCommittee` bei leerer Menge, `NotInCommittee`
    /// wenn der Leader nicht dazugehört.
    ///
    /// Gibt ein `Result` zurück statt zu panicken: die alte Fassung
    /// rechnete `(committee_size - 1) / 3` und lief bei `committee_size
    /// == 0` in einen usize-Underflow — Panic im Debug-Build, absurder
    /// Threshold im Release-Build.
    pub fn new(
        round: Round,
        leader: MinerId,
        voting_set: VotingSet,
    ) -> Result<Self, BftError> {
        if voting_set.is_empty() || voting_set.total_weight() == 0 {
            return Err(BftError::EmptyCommittee);
        }
        if !voting_set.contains(&leader) {
            return Err(BftError::NotInCommittee);
        }

        let threshold = voting_set.quorum_threshold();

        Ok(Self {
            round,
            status: RoundStatus::WaitingPropose,
            leader,
            proposed_block: None,
            votes: HashMap::new(),
            commits: HashMap::new(),
            voting_set,
            vote_weight: 0,
            commit_weight: 0,
            threshold,
        })
    }

    /// Verarbeitet eine Propose-Nachricht.
    ///
    /// Prüft Runde, Leader-Identität, Duplikat und Signatur.
    pub fn receive_propose(&mut self, propose: &Propose) -> Result<(), BftError> {
        self.pruefe_vorschlagsrahmen(propose)?;
        self.verify_signature(
            &propose.leader,
            &propose_message(propose.round, &propose.block_hash),
            &propose.signature,
        )?;

        self.uebernimm_vorschlag(propose);
        Ok(())
    }

    /// Wie [`Self::receive_propose`], aber für einen Vorschlag mit
    /// Polka-Zertifikat.
    ///
    /// Der Unterschied ist **nur** die Botschaft, gegen die geprüft
    /// wird: [`crate::signing::propose_pol_message`] bindet zusätzlich
    /// `valid_round`, also die Runde, aus der das Zertifikat stammt.
    ///
    /// ⚑ **Fund 66:** Ohne diese Bindung deckt die Signatur das
    /// Zertifikat nicht ab. Ein Abhörer könnte an einen ehrlichen
    /// Vorschlag ein anderes gültiges Zertifikat für denselben Block
    /// hängen, und beide Fassungen prüften durch. Der Domain-Trenner
    /// dafür stand seit v0.5.0 bereit und wurde von nichts aufgerufen.
    ///
    /// Prüft **das Zertifikat selbst nicht** — das tut
    /// [`crate::round_change::RoundDriver::may_vote_for`], die dafür die
    /// Sperre kennt.
    pub fn receive_propose_mit_polka(
        &mut self,
        propose: &Propose,
        valid_round: Round,
    ) -> Result<(), BftError> {
        self.pruefe_vorschlagsrahmen(propose)?;
        self.verify_signature(
            &propose.leader,
            &propose_pol_message(propose.round, &propose.block_hash, valid_round),
            &propose.signature,
        )?;
        self.uebernimm_vorschlag(propose);
        Ok(())
    }

    /// Runde, Leader und Duplikat, ohne die Signatur.
    fn pruefe_vorschlagsrahmen(&self, propose: &Propose) -> Result<(), BftError> {
        if propose.round != self.round {
            return Err(BftError::WrongRound {
                expected: self.round,
                got: propose.round,
            });
        }
        if propose.leader != self.leader {
            return Err(BftError::WrongLeader);
        }
        if self.proposed_block.is_some() {
            return Err(BftError::DuplicateMessage);
        }
        Ok(())
    }

    fn uebernimm_vorschlag(&mut self, propose: &Propose) {
        self.proposed_block = Some(propose.block_hash);
        self.status = RoundStatus::CollectingVotes;
    }

    /// Verarbeitet eine Vote-Nachricht.
    ///
    /// Prüft Runde, bekannten Block, Mitgliedschaft, Duplikat und
    /// Signatur; addiert dann das Stimmgewicht des Voters.
    pub fn receive_vote(&mut self, vote: &Vote) -> Result<(), BftError> {
        if vote.round != self.round {
            return Err(BftError::WrongRound {
                expected: self.round,
                got: vote.round,
            });
        }

        if self.proposed_block != Some(vote.block_hash) {
            return Err(BftError::UnknownBlock);
        }

        if !self.voting_set.contains(&vote.voter) {
            return Err(BftError::NotInCommittee);
        }

        if self.votes.contains_key(&vote.voter) {
            return Err(BftError::DuplicateMessage);
        }

        self.verify_signature(
            &vote.voter,
            &vote_message(vote.round, &vote.block_hash),
            &vote.signature,
        )?;

        self.votes.insert(vote.voter, vote.block_hash);
        self.vote_weight = self
            .vote_weight
            .saturating_add(self.voting_set.weight(&vote.voter));

        if self.vote_weight >= self.threshold {
            self.status = RoundStatus::CollectingCommits;
        }

        Ok(())
    }

    /// Verarbeitet eine Commit-Nachricht.
    ///
    /// Gleiche Prüfkette wie [`Self::receive_vote`], mit der
    /// Commit-Signierbotschaft.
    pub fn receive_commit(&mut self, commit: &Commit) -> Result<(), BftError> {
        if commit.round != self.round {
            return Err(BftError::WrongRound {
                expected: self.round,
                got: commit.round,
            });
        }

        if self.proposed_block != Some(commit.block_hash) {
            return Err(BftError::UnknownBlock);
        }

        if !self.voting_set.contains(&commit.committer) {
            return Err(BftError::NotInCommittee);
        }

        if self.commits.contains_key(&commit.committer) {
            return Err(BftError::DuplicateMessage);
        }

        self.verify_signature(
            &commit.committer,
            &commit_message(commit.round, &commit.block_hash),
            &commit.signature,
        )?;

        self.commits.insert(commit.committer, commit.block_hash);
        self.commit_weight = self
            .commit_weight
            .saturating_add(self.voting_set.weight(&commit.committer));

        if self.commit_weight >= self.threshold {
            self.status = RoundStatus::Committed;
        }

        Ok(())
    }

    /// Prüft eine BLS-Signatur gegen den registrierten Schlüssel.
    fn verify_signature(
        &self,
        sender: &MinerId,
        message: &[u8],
        signature: &BlsSignature,
    ) -> Result<(), BftError> {
        let pubkey = self
            .voting_set
            .pubkey(sender)
            .ok_or(BftError::NotInCommittee)?;
        if pubkey.verify(message, signature) {
            Ok(())
        } else {
            Err(BftError::InvalidSignature)
        }
    }

    /// Prüft, ob die Runde abgeschlossen ist (Block commitet).
    pub fn is_committed(&self) -> bool {
        self.status == RoundStatus::Committed
    }

    /// Übernimmt eine anderswo belegte Entscheidung.
    ///
    /// ⚑ **Der einzige Weg, auf dem dieser Automat commitet, ohne selbst
    /// gezählt zu haben** (Fund 67). Er ist absichtlich `pub(crate)`: Der
    /// Beleg wird in
    /// [`crate::round_change::RoundDriver::apply_commitzertifikat`]
    /// geprüft, und nur dort. Wäre er öffentlich, könnte ein Aufrufer
    /// eine Runde für commitet erklären, ohne je ein Quorum gesehen zu
    /// haben, und das Verfahren verlöre seine Safety-Garantie an eine
    /// einzige unbedachte Zeile.
    ///
    /// `proposed_block` wird mitgesetzt, denn für den übernehmenden
    /// Knoten **ist** das Zertifikat der Vorschlag: Er hat den Propose
    /// der belegten Runde nie gesehen und wird ihn nie sehen, weil
    /// Gossipsub nicht nachliefert. Ohne diese Zeile stünde die Runde auf
    /// `Committed`, und [`Self::committed_block`] gäbe `None` zurück.
    pub(crate) fn uebernimm_commit(&mut self, block_hash: Hash) {
        self.proposed_block = Some(block_hash);
        self.status = RoundStatus::Committed;
    }

    /// Gibt den commiteten Block-Hash zurück (falls vorhanden).
    pub fn committed_block(&self) -> Option<Hash> {
        if self.is_committed() {
            self.proposed_block
        } else {
            None
        }
    }

    /// Anzahl empfangener Votes (Köpfe).
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    /// Anzahl empfangener Commits (Köpfe).
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Bisher eingegangenes Stimmgewicht der Votes.
    pub fn vote_weight(&self) -> u64 {
        self.vote_weight
    }

    /// Bisher eingegangenes Stimmgewicht der Commits.
    pub fn commit_weight(&self) -> u64 {
        self.commit_weight
    }

    /// Benötigtes Stimmgewicht für ein Quorum.
    pub fn threshold(&self) -> u64 {
        self.threshold
    }

    /// Die stimmberechtigte Menge dieser Runde.
    pub fn voting_set(&self) -> &VotingSet {
        &self.voting_set
    }
}

/// Wählt den Leader für eine Runde (Round-Robin über die Producer).
///
/// **Parameter:**
/// - `round`: Rundennummer
/// - `producers`: Liste der Blockproduktions-Validatoren
///
/// **Returns:** `None` bei leerer Producer-Liste. Die alte Fassung
/// rechnete `round % producers.len()` und teilte bei leerer Liste durch
/// null.
pub fn select_leader(round: Round, producers: &[MinerId]) -> Option<MinerId> {
    if producers.is_empty() {
        return None;
    }
    Some(producers[(round as usize) % producers.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::{VotingMember, VotingSet};
    use myl_types::bls::{BlsPublicKey, BlsSecretKey};
    use std::collections::BTreeMap;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    fn keypair(byte: u8) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32]).expect("key_gen");
        let pk = sk.public_key().expect("public_key");
        (sk, pk)
    }

    /// Ein Komitee aus `n` Mitgliedern mit je Gewicht `weight`.
    fn voting_set(n: u8, weight: u64) -> VotingSet {
        let mut members = BTreeMap::new();
        for i in 0..n {
            let (_, pk) = keypair(i);
            members.insert(test_miner(i), VotingMember { pubkey: pk, weight });
        }
        VotingSet::from_members(members)
    }

    fn signed_propose(round: Round, hash: Hash, leader_byte: u8) -> Propose {
        let (sk, _) = keypair(leader_byte);
        Propose {
            round,
            block_hash: hash,
            leader: test_miner(leader_byte),
            signature: sk.sign(&propose_message(round, &hash)).unwrap(),
        }
    }

    fn signed_vote(round: Round, hash: Hash, voter_byte: u8) -> Vote {
        let (sk, _) = keypair(voter_byte);
        Vote {
            round,
            block_hash: hash,
            voter: test_miner(voter_byte),
            signature: sk.sign(&vote_message(round, &hash)).unwrap(),
        }
    }

    fn signed_commit(round: Round, hash: Hash, committer_byte: u8) -> Commit {
        let (sk, _) = keypair(committer_byte);
        Commit {
            round,
            block_hash: hash,
            committer: test_miner(committer_byte),
            signature: sk.sign(&commit_message(round, &hash)).unwrap(),
        }
    }

    fn fresh_state(n: u8) -> BftState {
        BftState::new(1, test_miner(0), voting_set(n, 100)).unwrap()
    }

    // ── Konstruktion ────────────────────────────────────────────────

    #[test]
    fn bft_state_creation() {
        let state = fresh_state(21);
        assert_eq!(state.round, 1);
        assert_eq!(state.status, RoundStatus::WaitingPropose);
        assert_eq!(state.leader, test_miner(0));
        assert!(state.proposed_block.is_none());
        // 21 x 100 = 2100; > 2/3 davon ist 1401.
        assert_eq!(state.threshold(), 1401);
    }

    /// Regression: `BftState::new` mit leerem Komitee lief vorher in
    /// einen usize-Underflow (`(0 - 1) / 3`).
    #[test]
    fn leeres_komitee_ergibt_fehler_statt_panic() {
        let leer = VotingSet::from_members(BTreeMap::new());
        assert_eq!(
            BftState::new(1, test_miner(0), leer).unwrap_err(),
            BftError::EmptyCommittee
        );
    }

    #[test]
    fn komitee_mit_gesamtgewicht_null() {
        let set = voting_set(5, 0);
        assert_eq!(
            BftState::new(1, test_miner(0), set).unwrap_err(),
            BftError::EmptyCommittee
        );
    }

    #[test]
    fn leader_ausserhalb_des_komitees_wird_abgelehnt() {
        let set = voting_set(5, 100);
        assert_eq!(
            BftState::new(1, test_miner(99), set).unwrap_err(),
            BftError::NotInCommittee
        );
    }

    // ── Propose ─────────────────────────────────────────────────────

    #[test]
    fn receive_propose_success() {
        let mut state = fresh_state(21);
        assert!(state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .is_ok());
        assert_eq!(state.status, RoundStatus::CollectingVotes);
        assert_eq!(state.proposed_block, Some(test_hash(1)));
    }

    #[test]
    fn receive_propose_wrong_leader() {
        let mut state = fresh_state(21);
        assert_eq!(
            state.receive_propose(&signed_propose(1, test_hash(1), 5)),
            Err(BftError::WrongLeader)
        );
    }

    #[test]
    fn receive_propose_wrong_round() {
        let mut state = fresh_state(21);
        assert_eq!(
            state.receive_propose(&signed_propose(2, test_hash(1), 0)),
            Err(BftError::WrongRound {
                expected: 1,
                got: 2
            })
        );
    }

    #[test]
    fn receive_propose_duplikat() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert_eq!(
            state.receive_propose(&signed_propose(1, test_hash(2), 0)),
            Err(BftError::DuplicateMessage)
        );
    }

    /// Ein Propose mit gefälschter Signatur darf die Runde nicht
    /// eröffnen — sonst könnte jeder einen Block in Umlauf bringen.
    #[test]
    fn propose_mit_ungueltiger_signatur() {
        let mut state = fresh_state(21);
        let mut p = signed_propose(1, test_hash(1), 0);
        p.signature = BlsSignature([0u8; 96]);
        assert_eq!(state.receive_propose(&p), Err(BftError::InvalidSignature));
        assert!(state.proposed_block.is_none());
    }

    /// Signatur über einen anderen Block-Hash darf nicht durchgehen.
    #[test]
    fn propose_signatur_bindet_den_blockhash() {
        let mut state = fresh_state(21);
        let mut p = signed_propose(1, test_hash(1), 0);
        p.block_hash = test_hash(2);
        assert_eq!(state.receive_propose(&p), Err(BftError::InvalidSignature));
    }

    // ── Vote ────────────────────────────────────────────────────────

    #[test]
    fn receive_vote_success() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert!(state.receive_vote(&signed_vote(1, test_hash(1), 1)).is_ok());
        assert_eq!(state.vote_count(), 1);
        assert_eq!(state.vote_weight(), 100);
    }

    #[test]
    fn receive_vote_threshold_reached() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        // 1401 / 100 = 15 Stimmen noetig.
        for i in 0..15 {
            state.receive_vote(&signed_vote(1, test_hash(1), i)).unwrap();
        }
        assert_eq!(state.status, RoundStatus::CollectingCommits);
        assert!(state.vote_weight() >= state.threshold());
    }

    #[test]
    fn vote_unter_threshold_wechselt_nicht() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        for i in 0..14 {
            state.receive_vote(&signed_vote(1, test_hash(1), i)).unwrap();
        }
        assert_eq!(state.status, RoundStatus::CollectingVotes);
    }

    #[test]
    fn receive_vote_unknown_block() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert_eq!(
            state.receive_vote(&signed_vote(1, test_hash(9), 1)),
            Err(BftError::UnknownBlock)
        );
    }

    #[test]
    fn receive_vote_duplikat() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        state.receive_vote(&signed_vote(1, test_hash(1), 1)).unwrap();
        assert_eq!(
            state.receive_vote(&signed_vote(1, test_hash(1), 1)),
            Err(BftError::DuplicateMessage)
        );
        assert_eq!(state.vote_weight(), 100, "Gewicht darf nicht doppelt zählen");
    }

    /// Der Kern von Fund A3: Ein Nichtmitglied darf nicht mitzählen.
    /// Vorher konnte ein Angreifer den Threshold mit erfundenen
    /// Miner-IDs allein erreichen.
    #[test]
    fn vote_eines_nichtmitglieds_wird_abgelehnt() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert_eq!(
            state.receive_vote(&signed_vote(1, test_hash(1), 200)),
            Err(BftError::NotInCommittee)
        );
        assert_eq!(state.vote_weight(), 0);
    }

    #[test]
    fn erfundene_stimmen_erreichen_kein_quorum() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        // 100 erfundene Identitaeten ausserhalb des Komitees.
        for i in 100..200u8 {
            let _ = state.receive_vote(&signed_vote(1, test_hash(1), i));
        }
        assert_eq!(state.vote_weight(), 0);
        assert_eq!(state.status, RoundStatus::CollectingVotes);
    }

    #[test]
    fn vote_mit_ungueltiger_signatur() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        let mut v = signed_vote(1, test_hash(1), 1);
        v.signature = BlsSignature([1u8; 96]);
        assert_eq!(state.receive_vote(&v), Err(BftError::InvalidSignature));
        assert_eq!(state.vote_weight(), 0);
    }

    /// Eine fremde, aber gültige Signatur (anderer Schlüssel) darf
    /// nicht als Stimme des angegebenen Voters durchgehen.
    #[test]
    fn vote_mit_fremder_signatur() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        let mut v = signed_vote(1, test_hash(1), 2);
        v.voter = test_miner(3); // Signatur gehoert zu Miner 2
        assert_eq!(state.receive_vote(&v), Err(BftError::InvalidSignature));
    }

    /// Domain-Separation im Einsatz: eine Commit-Signatur darf nicht als
    /// Vote zählen. Ohne sie liesse sich der Vote-Threshold mit den
    /// Commits einer früheren Runde erreichen.
    #[test]
    fn commit_signatur_gilt_nicht_als_vote() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        let c = signed_commit(1, test_hash(1), 1);
        let v = Vote {
            round: 1,
            block_hash: test_hash(1),
            voter: test_miner(1),
            signature: c.signature,
        };
        assert_eq!(state.receive_vote(&v), Err(BftError::InvalidSignature));
    }

    // ── Commit ──────────────────────────────────────────────────────

    #[test]
    fn receive_commit_success() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert!(state
            .receive_commit(&signed_commit(1, test_hash(1), 1))
            .is_ok());
        assert_eq!(state.commit_weight(), 100);
    }

    #[test]
    fn receive_commit_threshold_reached() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        for i in 0..15 {
            state
                .receive_commit(&signed_commit(1, test_hash(1), i))
                .unwrap();
        }
        assert!(state.is_committed());
        assert_eq!(state.committed_block(), Some(test_hash(1)));
    }

    #[test]
    fn commit_eines_nichtmitglieds_wird_abgelehnt() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert_eq!(
            state.receive_commit(&signed_commit(1, test_hash(1), 200)),
            Err(BftError::NotInCommittee)
        );
    }

    #[test]
    fn vote_signatur_gilt_nicht_als_commit() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        let v = signed_vote(1, test_hash(1), 1);
        let c = Commit {
            round: 1,
            block_hash: test_hash(1),
            committer: test_miner(1),
            signature: v.signature,
        };
        assert_eq!(state.receive_commit(&c), Err(BftError::InvalidSignature));
    }

    #[test]
    fn committed_block_erst_nach_quorum() {
        let mut state = fresh_state(21);
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();
        assert_eq!(state.committed_block(), None);
        for i in 0..14 {
            state
                .receive_commit(&signed_commit(1, test_hash(1), i))
                .unwrap();
        }
        assert_eq!(state.committed_block(), None);
    }

    // ── Gewichtung ──────────────────────────────────────────────────

    /// Ungleiche Gewichte müssen sich auswirken: ein Validator mit dem
    /// Gewicht einer Zweidrittelmehrheit erreicht das Quorum allein
    /// nicht, zwei zusammen schon.
    #[test]
    fn threshold_zaehlt_gewicht_nicht_koepfe() {
        let mut members = BTreeMap::new();
        // Miner 0: Gewicht 500, Miner 1: 400, Miner 2..4: je 100 → 1200.
        let gewichte = [500u64, 400, 100, 100, 100];
        for (i, w) in gewichte.iter().enumerate() {
            let (_, pk) = keypair(i as u8);
            members.insert(
                test_miner(i as u8),
                VotingMember {
                    pubkey: pk,
                    weight: *w,
                },
            );
        }
        let set = VotingSet::from_members(members);
        assert_eq!(set.total_weight(), 1200);
        assert_eq!(set.quorum_threshold(), 801);

        let mut state = BftState::new(1, test_miner(0), set).unwrap();
        state
            .receive_propose(&signed_propose(1, test_hash(1), 0))
            .unwrap();

        // Drei leichte Stimmen (300) reichen nicht ...
        for i in 2..5u8 {
            state.receive_vote(&signed_vote(1, test_hash(1), i)).unwrap();
        }
        assert_eq!(state.vote_count(), 3);
        assert_eq!(state.status, RoundStatus::CollectingVotes);

        // ... die beiden schweren zusammen (900) schon.
        state.receive_vote(&signed_vote(1, test_hash(1), 0)).unwrap();
        state.receive_vote(&signed_vote(1, test_hash(1), 1)).unwrap();
        assert_eq!(state.status, RoundStatus::CollectingCommits);
    }

    #[test]
    fn threshold_ist_mehr_als_zwei_drittel() {
        // 3 Mitglieder x 100 = 300 → Schwelle 201, nicht 200.
        let state = BftState::new(1, test_miner(0), voting_set(3, 100)).unwrap();
        assert_eq!(state.threshold(), 201);
    }

    // ── Leader-Wahl ─────────────────────────────────────────────────

    #[test]
    fn select_leader_round_robin() {
        let producers: Vec<MinerId> = (0..3).map(test_miner).collect();
        assert_eq!(select_leader(0, &producers), Some(test_miner(0)));
        assert_eq!(select_leader(1, &producers), Some(test_miner(1)));
        assert_eq!(select_leader(2, &producers), Some(test_miner(2)));
        assert_eq!(select_leader(3, &producers), Some(test_miner(0)));
    }

    /// Regression: `select_leader` teilte vorher bei leerer Liste durch null.
    #[test]
    fn select_leader_bei_leerer_liste() {
        assert_eq!(select_leader(0, &[]), None);
        assert_eq!(select_leader(u64::MAX, &[]), None);
    }

    // ── ⚑ Fund 66: die Signatur deckt die valid_round ──────────────

    fn pol_propose(round: Round, hash: Hash, leader_byte: u8, valid_round: Round) -> Propose {
        let (sk, _) = keypair(leader_byte);
        Propose {
            round,
            block_hash: hash,
            leader: test_miner(leader_byte),
            signature: sk
                .sign(&crate::signing::propose_pol_message(round, &hash, valid_round))
                .unwrap(),
        }
    }

    /// **Der Angriff, gegen den `DST_PROPOSE_POL` gerichtet ist.**
    ///
    /// Ein einfacher Vorschlag darf nicht als Vorschlag mit Zertifikat
    /// durchgehen. Sonst könnte ein Abhörer an einen ehrlichen Propose
    /// ein beliebiges gültiges Zertifikat hängen, und beide Fassungen
    /// prüften durch.
    #[test]
    fn fund_66_ein_einfacher_vorschlag_gilt_nicht_mit_zertifikat() {
        let h = test_hash(9);
        let einfach = signed_propose(1, h, 0);
        let mut z = fresh_state(5);
        assert_eq!(
            z.receive_propose_mit_polka(&einfach, 3),
            Err(BftError::InvalidSignature)
        );
    }

    /// Und die Gegenrichtung: ein Vorschlag mit Zertifikatsbindung gilt
    /// nicht als einfacher.
    #[test]
    fn fund_66_ein_vorschlag_mit_zertifikat_gilt_nicht_ohne() {
        let h = test_hash(9);
        let mit = pol_propose(1, h, 0, 3);
        let mut z = fresh_state(5);
        assert_eq!(z.receive_propose(&mit), Err(BftError::InvalidSignature));
    }

    /// **Die eigentliche Bindung:** Wer die `valid_round` hochsetzt, muss
    /// neu signieren. Genau das war ohne diesen Pfad nicht der Fall.
    #[test]
    fn fund_66_eine_veraenderte_valid_round_bricht_die_signatur() {
        let h = test_hash(9);
        let mit = pol_propose(1, h, 0, 3);
        let mut z = fresh_state(5);
        assert_eq!(
            z.receive_propose_mit_polka(&mit, 4),
            Err(BftError::InvalidSignature),
            "eine hochgesetzte valid_round kam durch"
        );
        // Mit der richtigen Zahl geht es durch. Ohne diese Gegenprobe
        // wäre auch ein Pfad grün, der jeden Vorschlag ablehnt.
        assert!(z.receive_propose_mit_polka(&mit, 3).is_ok());
        assert_eq!(z.proposed_block, Some(h));
    }

    // ── Konsensnachricht: die Form auf der Leitung ──────────────────

    fn drei_nachrichten() -> Vec<Konsensnachricht> {
        let h = test_hash(9);
        vec![
            Konsensnachricht::Propose(signed_propose(7, h, 0)),
            Konsensnachricht::Vote(signed_vote(7, h, 1)),
            Konsensnachricht::Commit(signed_commit(7, h, 2)),
        ]
    }

    #[test]
    fn eine_konsensnachricht_ueberlebt_die_leitung() {
        for n in drei_nachrichten() {
            let bytes = borsh::to_vec(&n).expect("serialisieren");
            let zurueck: Konsensnachricht =
                borsh::from_slice(&bytes).expect("lesen");
            assert_eq!(n, zurueck, "{} kam verändert zurück", n.art());
        }
    }

    #[test]
    fn die_drei_arten_sind_auf_der_leitung_unterscheidbar() {
        // Ohne diese Trennung wäre eine Vote als Commit lesbar, und der
        // Commit-Threshold ließe sich mit fremden Votes erreichen. Die
        // Signierbotschaften trennen das bereits (siehe crate::signing);
        // hier geht es um die Kodierung davor.
        let h = test_hash(9);
        let v = borsh::to_vec(&Konsensnachricht::Vote(signed_vote(7, h, 1))).unwrap();
        let c = borsh::to_vec(&Konsensnachricht::Commit(signed_commit(7, h, 1))).unwrap();
        assert_ne!(v, c);
        assert_eq!(v.len(), c.len(), "gleiche Länge, verschiedene Marke");
        assert_eq!(v[0], 1, "Vote trägt Enum-Marke 1");
        assert_eq!(c[0], 2, "Commit trägt Enum-Marke 2");
    }

    #[test]
    fn runde_und_absender_kommen_ohne_zustandsautomat_heraus() {
        // Der Knoten muss eine Nachricht der falschen Runde verwerfen
        // können, ohne sie erst dem Zustandsautomaten vorzulegen.
        for (n, erwartet) in drei_nachrichten().into_iter().zip([0u8, 1, 2]) {
            assert_eq!(n.runde(), 7);
            assert_eq!(n.absender(), Some(test_miner(erwartet)));
        }
    }

    #[test]
    fn ein_anhaengsel_hinter_einer_nachricht_wird_abgelehnt() {
        // Ein Anhängsel ändert die Nachrichten-Id, nicht den Inhalt:
        // dieselbe Stimme liefe beliebig oft durchs Netz und zählte im
        // Gossipsub-Scoring als neuer Verkehr.
        let mut bytes =
            borsh::to_vec(&Konsensnachricht::Vote(signed_vote(7, test_hash(9), 1))).unwrap();
        bytes.push(0);
        assert!(
            borsh::from_slice::<Konsensnachricht>(&bytes).is_err(),
            "ein Anhängsel hinter einer gültigen Nachricht kam durch"
        );
    }

    /// ⚑ **Der Parse ist fast nur eine Längenprüfung** (Fund 45, Fund 57).
    ///
    /// Gemessen statt behauptet. Der Unterschied zu Fund 45 ist nicht die
    /// Zahl, sondern dass die eigentliche Prüfung hier **erreichbar** ist:
    /// [`BftState::receive_vote`] prüft Runde, Mitgliedschaft, Duplikat
    /// und Signatur. Der Parse ist die Eingangstür, nicht die Prüfung.
    #[test]
    fn der_parse_einer_konsensnachricht_ist_fast_nur_eine_laengenpruefung() {
        let gut =
            borsh::to_vec(&Konsensnachricht::Vote(signed_vote(7, test_hash(9), 1))).unwrap();
        let mut zustand: u64 = 0x9E3779B97F4A7C15;
        let mut durch = 0usize;
        const VERSUCHE: usize = 20_000;
        for _ in 0..VERSUCHE {
            zustand ^= zustand << 13;
            zustand ^= zustand >> 7;
            zustand ^= zustand << 17;
            let mut kaputt = gut.clone();
            let pos = (zustand as usize) % kaputt.len();
            kaputt[pos] ^= 1u8 << ((zustand >> 32) % 8);
            if borsh::from_slice::<Konsensnachricht>(&kaputt).is_ok() {
                durch += 1;
            }
        }
        let anteil = durch * 100 / VERSUCHE;
        println!("[Messung] {durch} von {VERSUCHE} verstümmelten Nachrichten kamen durch ({anteil} %)");
        assert!(
            anteil > 90,
            "nur {anteil} % kamen durch. Wenn der Parse inzwischen mehr abfängt, \
             hat jemand eine echte Prüfung ergänzt; dann gehört der Modulkopf von \
             Konsensnachricht nachgezogen, statt diesen Test anzupassen"
        );
        // Gegenprobe: ganz zahnlos ist er nicht. Die Enum-Marke trägt.
        assert!(
            durch < VERSUCHE,
            "keine einzige verstümmelte Nachricht wurde abgelehnt: dann prüft \
             nicht einmal die Enum-Marke"
        );
    }
}

#[cfg(test)]
mod groessenmessung {
    use super::*;
    use myl_types::bls::BlsSecretKey;

    /// Wie groß eine Konsensnachricht auf der Leitung wirklich ist.
    ///
    /// Die Zahl steht als Herleitung in
    /// `myl_net::validation::MAX_CONSENSUS_BYTES`. Eine Grenze, deren
    /// Herleitung niemand nachrechnet, ist eine geratene Grenze.
    /// Wie groß ein Vorschlag **mit** Zertifikat wird.
    ///
    /// Die Zahlen stehen als Herleitung in
    /// `myl_net::validation::MAX_CONSENSUS_BYTES`. Reißt dieser Test die
    /// Grenze, ist die Antwort **nicht** ein größeres Limit, sondern eine
    /// Teilnahme-Bitmaske statt der Unterzeichnerliste: Die Liste ist
    /// redundant, sobald der Validator-Satz bekannt ist.
    #[test]
    fn ein_vorschlag_mit_zertifikat_passt_in_die_topic_grenze() {
        use crate::round_change::PolkaCertificate;
        use myl_types::bls::BlsAggregateSignature;
        const GRENZE: usize = 8 * 1024;
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).unwrap();
        let h = Hash::sha256(b"b");
        for (n, erwartet) in [(5usize, 469usize), (21, 981), (128, 4405)] {
            let zert = PolkaCertificate {
                round: 6,
                block_hash: h,
                voters: (0..n).map(|i| MinerId::new([i as u8; 32])).collect(),
                aggregate: BlsAggregateSignature([0u8; 96]),
            };
            let n_bytes = Konsensnachricht::ProposeMitPolka(
                Propose {
                    round: 7,
                    block_hash: h,
                    leader: MinerId::new([1u8; 32]),
                    signature: sk.sign(b"x").unwrap(),
                },
                zert,
            );
            let gross = borsh::to_vec(&n_bytes).unwrap().len();
            println!("[Messung] Propose + Zertifikat mit {n} Unterzeichnern: {gross} Bytes");
            assert_eq!(gross, erwartet, "bei {n} Unterzeichnern");
            assert!(
                gross < GRENZE,
                "{gross} Bytes reißen die Topic-Grenze von {GRENZE}"
            );
        }
    }

    /// Wie groß ein **Commit-Zertifikat** auf der Leitung wird.
    ///
    /// ⚑ Nachgerechnet, weil die Herleitung von
    /// `myl_net::validation::MAX_CONSENSUS_BYTES` es von jedem verlangt,
    /// der eine Nachricht anschließt (Fund 67). Es ist die zweitgrößte
    /// Nachricht des Protokolls: dieselbe Unterzeichnerliste wie ein
    /// Polka, aber ohne den Vorschlag davor.
    ///
    /// Reißt dieser Test die Grenze, ist die Antwort **nicht** ein
    /// größeres Limit, sondern eine Teilnahme-Bitmaske statt der
    /// Unterzeichnerliste.
    #[test]
    fn ein_commit_zertifikat_passt_in_die_topic_grenze() {
        use crate::round_change::Commitzertifikat;
        use myl_types::bls::BlsAggregateSignature;
        const GRENZE: usize = 8 * 1024;
        let h = Hash::sha256(b"b");
        for (n, erwartet) in [(5usize, 301usize), (21, 813), (128, 4237)] {
            let zert = Commitzertifikat {
                round: 6,
                block_hash: h,
                committers: (0..n).map(|i| MinerId::new([i as u8; 32])).collect(),
                aggregate: BlsAggregateSignature([0u8; 96]),
            };
            let gross = borsh::to_vec(&Konsensnachricht::Commitzertifikat(zert))
                .unwrap()
                .len();
            println!("[Messung] Commit-Zertifikat mit {n} Unterzeichnern: {gross} Bytes");
            assert_eq!(gross, erwartet, "bei {n} Unterzeichnern");
            assert!(
                gross < GRENZE,
                "{gross} Bytes reißen die Topic-Grenze von {GRENZE}"
            );
        }
    }

    #[test]
    fn ein_vorschlag_mit_zertifikat_ueberlebt_die_leitung() {
        use crate::round_change::PolkaCertificate;
        use myl_types::bls::BlsAggregateSignature;
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).unwrap();
        let h = Hash::sha256(b"b");
        let n = Konsensnachricht::ProposeMitPolka(
            Propose {
                round: 7,
                block_hash: h,
                leader: MinerId::new([1u8; 32]),
                signature: sk.sign(b"x").unwrap(),
            },
            PolkaCertificate {
                round: 6,
                block_hash: h,
                voters: vec![MinerId::new([1u8; 32]), MinerId::new([2u8; 32])],
                aggregate: BlsAggregateSignature([7u8; 96]),
            },
        );
        let bytes = borsh::to_vec(&n).unwrap();
        assert_eq!(bytes[0], 3, "die vierte Marke ist additiv, also Nummer 3");
        assert_eq!(borsh::from_slice::<Konsensnachricht>(&bytes).unwrap(), n);
        assert_eq!(n.art(), "propose-mit-polka");
        assert_eq!(n.runde(), 7);
        assert_eq!(n.absender(), Some(MinerId::new([1u8; 32])));
    }

    #[test]
    fn eine_konsensnachricht_ist_169_bytes_gross() {
        let sk = BlsSecretKey::key_gen(&[3u8; 32]).unwrap();
        let h = Hash::sha256(b"b");
        let p = Konsensnachricht::Propose(Propose {
            round: 7,
            block_hash: h,
            leader: MinerId::new([1u8; 32]),
            signature: sk.sign(b"x").unwrap(),
        });
        let n = borsh::to_vec(&p).unwrap().len();
        println!("[Messung] Propose auf der Leitung: {n} Bytes");
        // 1 Enum-Marke + 8 Runde + 32 Hash + 32 Miner-Id + 96 Signatur.
        assert_eq!(n, 169);
        // Alle drei Arten sind gleich groß: dieselben Felder.
        let v = Konsensnachricht::Vote(Vote {
            round: 7,
            block_hash: h,
            voter: MinerId::new([1u8; 32]),
            signature: sk.sign(b"x").unwrap(),
        });
        assert_eq!(borsh::to_vec(&v).unwrap().len(), n);
    }
}
