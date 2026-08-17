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

use crate::signing::{commit_message, propose_message, vote_message};
use crate::validator::VotingSet;
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

        self.verify_signature(
            &propose.leader,
            &propose_message(propose.round, &propose.block_hash),
            &propose.signature,
        )?;

        self.proposed_block = Some(propose.block_hash);
        self.status = RoundStatus::CollectingVotes;
        Ok(())
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
}
