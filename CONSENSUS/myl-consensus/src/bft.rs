//! BFT-Kernprotokoll — Whitepaper Kap. 3.5, Anhang A.2.
//!
//! Propose/Vote/Commit-Zyklus für BFT-Blockproduktion. Safety und Liveness
//! unter f < 1/3 byzantinischen Stimmen.
//!
//! **Design-Entscheidung:** Trait-Grenze für malachite-consensus Integration,
//! Eigenbau als Fallback.
//!
//! **Konsens-Feld:** Das BFT-Protokoll ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

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
    /// 2f+1 Votes empfangen, warte auf Commits.
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
    /// Anzahl benötigter Votes/Commits (2f+1).
    pub threshold: usize,
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
    /// Ungültige Signatur (Placeholder).
    InvalidSignature,
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
        }
    }
}

impl std::error::Error for BftError {}

impl BftState {
    /// Erstellt einen neuen BFT-Zustand für eine Runde.
    ///
    /// **Parameter:**
    /// - `round`: Rundennummer
    /// - `leader`: Leader für diese Runde
    /// - `committee_size`: Größe des Komitees (zur Berechnung des Thresholds)
    pub fn new(round: Round, leader: MinerId, committee_size: usize) -> Self {
        // Threshold: 2f+1 wobei f = (n-1)/3
        let f = (committee_size - 1) / 3;
        let threshold = 2 * f + 1;

        Self {
            round,
            status: RoundStatus::WaitingPropose,
            leader,
            proposed_block: None,
            votes: HashMap::new(),
            commits: HashMap::new(),
            threshold,
        }
    }

    /// Verarbeitet eine Propose-Nachricht.
    ///
    /// **Returns:** `Ok(())` bei erfolgreicher Verarbeitung.
    ///
    /// **Fehler:** `BftError` wenn Runde falsch, Leader falsch, oder doppelt.
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

        self.proposed_block = Some(propose.block_hash);
        self.status = RoundStatus::CollectingVotes;
        Ok(())
    }

    /// Verarbeitet eine Vote-Nachricht.
    ///
    /// **Returns:** `Ok(())` bei erfolgreicher Verarbeitung.
    ///
    /// **Fehler:** `BftError` wenn Runde falsch, Block unbekannt, oder doppelt.
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

        if self.votes.contains_key(&vote.voter) {
            return Err(BftError::DuplicateMessage);
        }

        self.votes.insert(vote.voter, vote.block_hash);

        // Prüfe, ob Threshold erreicht
        if self.votes.len() >= self.threshold {
            self.status = RoundStatus::CollectingCommits;
        }

        Ok(())
    }

    /// Verarbeitet eine Commit-Nachricht.
    ///
    /// **Returns:** `Ok(())` bei erfolgreicher Verarbeitung.
    ///
    /// **Fehler:** `BftError` wenn Runde falsch, Block unbekannt, oder doppelt.
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

        if self.commits.contains_key(&commit.committer) {
            return Err(BftError::DuplicateMessage);
        }

        self.commits.insert(commit.committer, commit.block_hash);

        // Prüfe, ob Threshold erreicht
        if self.commits.len() >= self.threshold {
            self.status = RoundStatus::Committed;
        }

        Ok(())
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

    /// Gibt die Anzahl empfangener Votes zurück.
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    /// Gibt die Anzahl empfangener Commits zurück.
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }
}

/// Wählt den Leader für eine Runde (Round-Robin).
///
/// **Parameter:**
/// - `round`: Rundennummer
/// - `producers`: Liste der Blockproduktions-Validatoren
///
/// **Returns:** Miner-ID des Leaders.
pub fn select_leader(round: Round, producers: &[MinerId]) -> MinerId {
    let index = (round as usize) % producers.len();
    producers[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    #[test]
    fn bft_state_creation() {
        let leader = test_miner(1);
        let state = BftState::new(0, leader, 21);

        assert_eq!(state.round, 0);
        assert_eq!(state.leader, leader);
        assert_eq!(state.status, RoundStatus::WaitingPropose);
        assert_eq!(state.threshold, 13); // 2 * 6 + 1 = 13
    }

    #[test]
    fn receive_propose_success() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let block_hash = test_hash(1);

        let propose = Propose {
            round: 0,
            block_hash,
            leader,
        };

        let result = state.receive_propose(&propose);
        assert!(result.is_ok());
        assert_eq!(state.proposed_block, Some(block_hash));
        assert_eq!(state.status, RoundStatus::CollectingVotes);
    }

    #[test]
    fn receive_propose_wrong_leader() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let wrong_leader = test_miner(2);

        let propose = Propose {
            round: 0,
            block_hash: test_hash(1),
            leader: wrong_leader,
        };

        let result = state.receive_propose(&propose);
        assert!(matches!(result, Err(BftError::WrongLeader)));
    }

    #[test]
    fn receive_vote_success() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let block_hash = test_hash(1);

        // Propose zuerst
        state
            .receive_propose(&Propose {
                round: 0,
                block_hash,
                leader,
            })
            .unwrap();

        // Vote
        let vote = Vote {
            round: 0,
            block_hash,
            voter: test_miner(2),
        };

        let result = state.receive_vote(&vote);
        assert!(result.is_ok());
        assert_eq!(state.vote_count(), 1);
    }

    #[test]
    fn receive_vote_threshold_reached() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let block_hash = test_hash(1);

        state
            .receive_propose(&Propose {
                round: 0,
                block_hash,
                leader,
            })
            .unwrap();

        // 13 Votes senden (Threshold)
        for i in 0..13 {
            let vote = Vote {
                round: 0,
                block_hash,
                voter: test_miner(i + 2),
            };
            state.receive_vote(&vote).unwrap();
        }

        assert_eq!(state.status, RoundStatus::CollectingCommits);
    }

    #[test]
    fn receive_commit_success() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let block_hash = test_hash(1);

        // Propose und Votes
        state
            .receive_propose(&Propose {
                round: 0,
                block_hash,
                leader,
            })
            .unwrap();

        for i in 0..13 {
            state
                .receive_vote(&Vote {
                    round: 0,
                    block_hash,
                    voter: test_miner(i + 2),
                })
                .unwrap();
        }

        // Commit
        let commit = Commit {
            round: 0,
            block_hash,
            committer: test_miner(2),
        };

        let result = state.receive_commit(&commit);
        assert!(result.is_ok());
        assert_eq!(state.commit_count(), 1);
    }

    #[test]
    fn receive_commit_threshold_reached() {
        let leader = test_miner(1);
        let mut state = BftState::new(0, leader, 21);
        let block_hash = test_hash(1);

        state
            .receive_propose(&Propose {
                round: 0,
                block_hash,
                leader,
            })
            .unwrap();

        for i in 0..13 {
            state
                .receive_vote(&Vote {
                    round: 0,
                    block_hash,
                    voter: test_miner(i + 2),
                })
                .unwrap();
        }

        // 13 Commits senden (Threshold)
        for i in 0..13 {
            state
                .receive_commit(&Commit {
                    round: 0,
                    block_hash,
                    committer: test_miner(i + 2),
                })
                .unwrap();
        }

        assert_eq!(state.status, RoundStatus::Committed);
        assert!(state.is_committed());
        assert_eq!(state.committed_block(), Some(block_hash));
    }

    #[test]
    fn select_leader_round_robin() {
        let producers = vec![test_miner(1), test_miner(2), test_miner(3)];

        assert_eq!(select_leader(0, &producers), test_miner(1));
        assert_eq!(select_leader(1, &producers), test_miner(2));
        assert_eq!(select_leader(2, &producers), test_miner(3));
        assert_eq!(select_leader(3, &producers), test_miner(1)); // Wrap-around
    }

    #[test]
    fn threshold_calculation() {
        // 21 Komitee-Mitglieder: f = 6, threshold = 13
        let state = BftState::new(0, test_miner(1), 21);
        assert_eq!(state.threshold, 13);

        // 7 Komitee-Mitglieder: f = 2, threshold = 5
        let state = BftState::new(0, test_miner(1), 7);
        assert_eq!(state.threshold, 5);
    }
}
