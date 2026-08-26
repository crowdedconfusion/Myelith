//! Akzeptanz-Testmatrix Phase 3 — Safety und Liveness unter f < 1/3.
//!
//! Das Akzeptanzkriterium der Phase 3 verlangt wörtlich: „Safety und
//! Liveness unter f < 1/3 byzantinischen Stimmen in einem Netz von ≥ 20
//! simulierten Validatoren (Standard-BFT-Testmatrix: Leader-Ausfall,
//! Netzwerkpartition unter/über GST, verzögerte Nachrichten)."
//!
//! Diese Matrix war bis Punkt 3.6 **nicht durchführbar**: ohne
//! Rundenwechsel bleibt das Protokoll beim ersten Leader-Ausfall stehen,
//! und ein Test, der auf einen Fortschritt wartet, der nicht kommen
//! kann, prüft nichts. Mit [`RoundDriver`] ist sie durchführbar, und
//! hier steht sie.
//!
//! ## Aufbau
//!
//! 21 Validatoren mit je Gewicht 100, also Gesamtgewicht 2100 und
//! Quorum-Schwelle `2·2100/3 + 1 = 1401` — erreichbar ab **15** Knoten.
//! Jeder Knoten hat einen eigenen [`RoundDriver`]; das Netz ist dadurch
//! modelliert, welche Nachrichten welchem Knoten übergeben werden. Eine
//! Partition ist schlicht ein Nachrichtenpfad, den der Test nicht geht.
//!
//! Es gibt keine Threads und keine Uhr: die Zeit ist eine Zahl, die der
//! Test weiterstellt. Dadurch ist die Matrix reproduzierbar und läuft in
//! Millisekunden statt in Sekunden.

use myl_consensus::bft::{Commit, Propose, Round, RoundStatus, Vote};
use myl_consensus::round_change::{RoundChange, RoundDriver, RoundError, TimeoutConfig};
use myl_consensus::signing::{commit_message, propose_message, vote_message};
use myl_consensus::validator::{VotingMember, VotingSet};
use myl_types::bls::{BlsPublicKey, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;
use std::collections::BTreeMap;

/// Komiteegröße der Matrix — über der Forderung von ≥ 20.
const N: u8 = 21;

/// Stimmgewicht je Validator (alle gleich, damit „Knotenzahl" und
/// „Stimmgewicht" im Test dasselbe aussagen).
const GEWICHT: u64 = 100;

/// Knoten, die zusammen das Quorum erreichen: 15 × 100 = 1500 ≥ 1401.
const QUORUM_KNOTEN: u8 = 15;

/// Größte byzantinische Menge unter f < 1/3: 6 × 100 = 600 < 700.
const BYZANTINISCH: u8 = 6;

fn miner(byte: u8) -> MinerId {
    MinerId::new([byte; 32])
}

fn hash(byte: u8) -> Hash {
    Hash::sha256(&[byte])
}

fn keypair(byte: u8) -> (BlsSecretKey, BlsPublicKey) {
    let sk = BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32]).expect("key_gen");
    let pk = sk.public_key().expect("public_key");
    (sk, pk)
}

fn voting_set() -> VotingSet {
    let mut members = BTreeMap::new();
    for i in 0..N {
        let (_, pk) = keypair(i);
        members.insert(
            miner(i),
            VotingMember {
                pubkey: pk,
                weight: GEWICHT,
            },
        );
    }
    VotingSet::from_members(members)
}

fn producers() -> Vec<MinerId> {
    (0..N).map(miner).collect()
}

fn propose(round: Round, block: Hash, leader: u8) -> Propose {
    let (sk, _) = keypair(leader);
    Propose {
        round,
        block_hash: block,
        leader: miner(leader),
        signature: sk.sign(&propose_message(round, &block)).expect("sign"),
    }
}

fn vote(round: Round, block: Hash, voter: u8) -> Vote {
    let (sk, _) = keypair(voter);
    Vote {
        round,
        block_hash: block,
        voter: miner(voter),
        signature: sk.sign(&vote_message(round, &block)).expect("sign"),
    }
}

fn commit(round: Round, block: Hash, committer: u8) -> Commit {
    let (sk, _) = keypair(committer);
    Commit {
        round,
        block_hash: block,
        committer: miner(committer),
        signature: sk.sign(&commit_message(round, &block)).expect("sign"),
    }
}

/// Ein Netz aus `N` Knoten, jeder mit eigener Sicht auf das Protokoll.
struct Netz {
    knoten: Vec<RoundDriver>,
}

impl Netz {
    fn neu() -> Self {
        let knoten = (0..N)
            .map(|_| {
                RoundDriver::new(producers(), voting_set(), TimeoutConfig::default(), 0)
                    .expect("Treiber")
            })
            .collect();
        Self { knoten }
    }

    /// Lässt jeden Knoten in `menge` seine Frist ablaufen und die Runde
    /// wechseln. Gibt den spätesten Zeitpunkt zurück.
    fn runde_wechseln(&mut self, menge: &[u8]) -> u64 {
        let mut spaetester = 0;
        for &i in menge {
            let d = &mut self.knoten[i as usize];
            let t = d.deadline_ms();
            let change = d.on_timeout(t).expect("Timeout");
            assert!(
                matches!(change, RoundChange::Advanced { .. }),
                "Knoten {} wechselte nicht: {:?}",
                i,
                change
            );
            spaetester = spaetester.max(t);
        }
        spaetester
    }

    /// Verteilt einen Vorschlag an `empfaenger`. Gibt die Knoten zurück,
    /// die ihn abgelehnt haben, mit Grund.
    fn propose_an(
        &mut self,
        empfaenger: &[u8],
        p: &Propose,
        now: u64,
    ) -> Vec<(u8, RoundError)> {
        let mut abgelehnt = Vec::new();
        for &i in empfaenger {
            if let Err(e) = self.knoten[i as usize].receive_propose(p, None, now) {
                abgelehnt.push((i, e));
            }
        }
        abgelehnt
    }

    /// Lässt `stimmende` für `block` stimmen; jede Stimme geht an jeden
    /// Knoten in `empfaenger`.
    fn votes_an(&mut self, empfaenger: &[u8], stimmende: &[u8], round: Round, block: Hash, now: u64) {
        for &v in stimmende {
            let msg = vote(round, block, v);
            for &i in empfaenger {
                let _ = self.knoten[i as usize].receive_vote(&msg, now);
            }
        }
    }

    /// Wie [`Self::votes_an`], für Commits.
    fn commits_an(&mut self, empfaenger: &[u8], commitende: &[u8], round: Round, block: Hash, now: u64) {
        for &c in commitende {
            let msg = commit(round, block, c);
            for &i in empfaenger {
                let _ = self.knoten[i as usize].receive_commit(&msg, now);
            }
        }
    }

    fn commitete_bloecke(&self) -> Vec<Hash> {
        let mut b: Vec<Hash> = self
            .knoten
            .iter()
            .filter_map(|d| d.committed_block())
            .collect();
        b.sort_by_key(|h| *h.as_bytes());
        b.dedup();
        b
    }

    fn anzahl_commitet(&self) -> usize {
        self.knoten.iter().filter(|d| d.is_committed()).count()
    }
}

fn alle() -> Vec<u8> {
    (0..N).collect()
}

// ── Leader-Ausfall ──────────────────────────────────────────────────

#[test]
fn leader_ausfall_blockiert_das_netz_nicht() {
    // Die Eigenschaft, wegen der es 3.6 gibt. Drei Leader schweigen;
    // vor dem Rundenwechsel waere das Netz hier endgueltig stehen
    // geblieben.
    let mut netz = Netz::neu();
    let t = {
        let mut t = 0;
        for _ in 0..3 {
            t = netz.runde_wechseln(&alle());
        }
        t
    };

    for d in &netz.knoten {
        assert_eq!(d.round(), 3);
        assert_eq!(d.leader(), miner(3));
    }

    let block = hash(42);
    let empfaenger = alle();
    assert!(netz.propose_an(&empfaenger, &propose(3, block, 3), t).is_empty());

    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();
    netz.votes_an(&empfaenger, &stimmende, 3, block, t);
    netz.commits_an(&empfaenger, &stimmende, 3, block, t);

    assert_eq!(netz.anzahl_commitet(), N as usize);
    assert_eq!(netz.commitete_bloecke(), vec![block]);
}

#[test]
fn fristen_wachsen_ueber_die_runden() {
    // Ohne wachsende Fristen gibt es keine Liveness-Garantie nach GST:
    // ein zu kurzer Timeout laesst jede Runde platzen, bevor die Votes
    // ankommen.
    let mut netz = Netz::neu();
    let mut dauern = Vec::new();
    for _ in 0..5 {
        let vorher = netz.knoten[0].deadline_ms();
        let t = netz.runde_wechseln(&alle());
        dauern.push(netz.knoten[0].deadline_ms() - t);
        assert!(vorher <= t);
    }
    assert!(
        dauern.windows(2).all(|w| w[0] < w[1]),
        "Fristen wachsen nicht: {:?}",
        dauern
    );
}

// ── Safety unter Sperre ─────────────────────────────────────────────

#[test]
fn gesperrte_mehrheit_verweigert_konkurrierenden_block() {
    // Das Szenario, an dem naiver Rundenwechsel die Safety bricht:
    // Runde 0 erzeugt ein Quorum fuer A, Runde 1 schlaegt B vor. Ohne
    // Sperre wuerden beide Bloecke commit-faehig.
    let mut netz = Netz::neu();
    let a = hash(1);
    let b = hash(2);
    let empfaenger = alle();

    assert!(netz.propose_an(&empfaenger, &propose(0, a, 0), 0).is_empty());
    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();
    netz.votes_an(&empfaenger, &stimmende, 0, a, 0);

    // Alle 21 haben das Quorum gesehen und sind gesperrt.
    for (i, d) in netz.knoten.iter().enumerate() {
        let lock = d.lock().unwrap_or_else(|| panic!("Knoten {} ohne Sperre", i));
        assert_eq!(lock.block_hash, a);
        assert_eq!(lock.round, 0);
    }

    let t = netz.runde_wechseln(&alle());
    let abgelehnt = netz.propose_an(&empfaenger, &propose(1, b, 1), t);
    assert_eq!(
        abgelehnt.len(),
        N as usize,
        "jeder gesperrte Knoten muss B ablehnen"
    );
    for (_, e) in &abgelehnt {
        assert!(matches!(e, RoundError::Locked { .. }), "{:?}", e);
    }

    // Kein Knoten hat B ueberhaupt als Vorschlag angenommen.
    for d in &netz.knoten {
        assert!(d.state().proposed_block.is_none());
    }
}

#[test]
fn byzantinische_minderheit_erreicht_kein_quorum() {
    // f < 1/3: 6 von 21 stimmen fuer einen konkurrierenden Block. Sie
    // tragen 600 von 1401 benoetigten Gewichtseinheiten — zu wenig,
    // egal wie sie sich koordinieren.
    let mut netz = Netz::neu();
    let a = hash(1);
    let b = hash(2);

    // Die ehrliche Mehrheit sperrt sich in Runde 0 auf A.
    let ehrlich: Vec<u8> = (0..(N - BYZANTINISCH)).collect();
    let byz: Vec<u8> = ((N - BYZANTINISCH)..N).collect();
    assert!(netz.propose_an(&ehrlich, &propose(0, a, 0), 0).is_empty());
    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();
    netz.votes_an(&ehrlich, &stimmende, 0, a, 0);

    // Die byzantinischen Knoten haben A nie gesehen und sind frei.
    for &i in &byz {
        assert!(netz.knoten[i as usize].lock().is_none());
    }

    // Runde 1: die byzantinische Menge versucht B durchzusetzen.
    let t = netz.runde_wechseln(&alle());
    assert!(netz.propose_an(&byz, &propose(1, b, 1), t).is_empty());
    netz.votes_an(&byz, &byz, 1, b, t);

    // Kein byzantinischer Knoten erreicht das Quorum — 600 < 1401.
    for &i in &byz {
        let d = &netz.knoten[i as usize];
        assert!(d.state().vote_weight() < d.state().threshold());
        assert_ne!(d.status(), RoundStatus::CollectingCommits);
        assert!(!d.is_committed());
    }
    // Und die ehrliche Mehrheit bleibt auf A gesperrt.
    for &i in &ehrlich {
        assert_eq!(netz.knoten[i as usize].lock().unwrap().block_hash, a);
    }
    assert!(netz.commitete_bloecke().is_empty());
}

// ── Netzwerkpartition ───────────────────────────────────────────────

#[test]
fn partition_unter_gst_commitet_nichts() {
    // Zwei Haelften, keine erreicht das Quorum (11 und 10 von 15
    // benoetigten). Unter GST darf nichts commitet werden — Safety geht
    // vor Fortschritt.
    let mut netz = Netz::neu();
    let a = hash(1);
    let b = hash(2);
    let links: Vec<u8> = (0..11).collect();
    let rechts: Vec<u8> = (11..N).collect();

    assert!(netz.propose_an(&links, &propose(0, a, 0), 0).is_empty());
    netz.votes_an(&links, &links, 0, a, 0);

    let t = netz.runde_wechseln(&rechts);
    assert!(netz.propose_an(&rechts, &propose(1, b, 1), t).is_empty());
    netz.votes_an(&rechts, &rechts, 1, b, t);

    assert_eq!(netz.anzahl_commitet(), 0, "unter GST darf nichts commiten");
    assert!(netz.commitete_bloecke().is_empty());
    // Keine Seite hat eine Sperre erworben — beide blieben unter Quorum.
    assert!(netz.knoten.iter().all(|d| d.lock().is_none()));
}

#[test]
fn nach_gst_commitet_das_geheilte_netz() {
    // Ueber GST: die Partition heilt, alle Nachrichten kommen an, das
    // Protokoll kommt zum Abschluss. Das ist die Liveness-Haelfte des
    // Akzeptanzkriteriums.
    let mut netz = Netz::neu();
    let a = hash(1);
    let links: Vec<u8> = (0..11).collect();

    // Unter GST: nur die linke Haelfte sieht den Vorschlag.
    assert!(netz.propose_an(&links, &propose(0, a, 0), 0).is_empty());
    netz.votes_an(&links, &links, 0, a, 0);
    assert_eq!(netz.anzahl_commitet(), 0);

    // GST: alle wechseln in Runde 1, danach ist das Netz vollstaendig.
    let t = netz.runde_wechseln(&alle());
    let empfaenger = alle();
    let block = hash(9);
    assert!(netz.propose_an(&empfaenger, &propose(1, block, 1), t).is_empty());

    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();
    netz.votes_an(&empfaenger, &stimmende, 1, block, t);
    netz.commits_an(&empfaenger, &stimmende, 1, block, t);

    assert_eq!(netz.anzahl_commitet(), N as usize);
    assert_eq!(
        netz.commitete_bloecke(),
        vec![block],
        "alle Knoten muessen denselben Block commiten"
    );
}

#[test]
fn geheiltes_netz_respektiert_die_sperre_der_partition() {
    // Feiner als der vorige Test: die linke Haelfte hat in der Partition
    // ein Quorum erreicht und ist gesperrt. Nach der Heilung darf ein
    // beliebiger neuer Block nicht mehr durchgehen — sonst haette die
    // Heilung die Safety-Garantie aufgehoben.
    let mut netz = Netz::neu();
    let a = hash(1);
    let b = hash(2);
    let mehrheit: Vec<u8> = (0..QUORUM_KNOTEN).collect();

    assert!(netz.propose_an(&mehrheit, &propose(0, a, 0), 0).is_empty());
    netz.votes_an(&mehrheit, &mehrheit, 0, a, 0);
    for &i in &mehrheit {
        assert_eq!(netz.knoten[i as usize].lock().unwrap().block_hash, a);
    }

    let t = netz.runde_wechseln(&alle());
    let abgelehnt = netz.propose_an(&alle(), &propose(1, b, 1), t);
    let abgelehnte_ids: Vec<u8> = abgelehnt.iter().map(|(i, _)| *i).collect();
    assert_eq!(
        abgelehnte_ids, mehrheit,
        "genau die gesperrten Knoten muessen ablehnen"
    );

    // Die sechs ungesperrten Knoten nehmen B an — das ist richtig so,
    // sie haben A nie gesehen. Entscheidend ist, dass sie B nicht
    // durchsetzen koennen: 6 x 100 = 600 < 1401.
    let rest: Vec<u8> = (QUORUM_KNOTEN..N).collect();
    for &i in &rest {
        assert_eq!(netz.knoten[i as usize].state().proposed_block, Some(b));
    }
    netz.votes_an(&rest, &rest, 1, b, t);
    for &i in &rest {
        let d = &netz.knoten[i as usize];
        assert!(d.state().vote_weight() < d.state().threshold());
        assert!(!d.is_committed());
    }

    // Die gesperrte Mehrheit haette A dagegen angenommen — die Sperre
    // blockiert nicht den Fortschritt, sondern nur den Blockwechsel.
    assert!(netz.propose_an(&mehrheit, &propose(1, a, 1), t).is_empty());
    assert!(netz.commitete_bloecke().is_empty());
}

// ── Verzögerte Nachrichten ──────────────────────────────────────────

#[test]
fn verzoegerte_votes_aus_alter_runde_werden_abgelehnt() {
    // Eine Stimme, die nach dem Rundenwechsel eintrifft, darf nicht in
    // der neuen Runde zaehlen — sonst liesse sich ein Quorum aus
    // Stimmen verschiedener Runden zusammensetzen.
    let mut netz = Netz::neu();
    let a = hash(1);
    let t = netz.runde_wechseln(&alle());

    let alte_stimme = vote(0, a, 5);
    let err = netz.knoten[0]
        .receive_vote(&alte_stimme, t)
        .expect_err("alte Runde muss abgelehnt werden");
    assert!(
        matches!(
            err,
            RoundError::Bft(myl_consensus::bft::BftError::WrongRound { expected: 1, got: 0 })
        ),
        "{:?}",
        err
    );
    assert_eq!(netz.knoten[0].state().vote_weight(), 0);
}

#[test]
fn verzoegerte_nachrichten_veraendern_das_ergebnis_nicht() {
    // Nach dem Commit treffen noch Nachrichten der alten Runde ein. Der
    // commitete Block darf sich dadurch nicht mehr aendern.
    let mut netz = Netz::neu();
    let block = hash(3);
    let empfaenger = alle();
    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();

    assert!(netz.propose_an(&empfaenger, &propose(0, block, 0), 0).is_empty());
    netz.votes_an(&empfaenger, &stimmende, 0, block, 0);
    netz.commits_an(&empfaenger, &stimmende, 0, block, 0);
    assert_eq!(netz.commitete_bloecke(), vec![block]);

    // Nachzuegler: die uebrigen sechs stimmen und commiten verspaetet.
    let nachzuegler: Vec<u8> = (QUORUM_KNOTEN..N).collect();
    netz.votes_an(&empfaenger, &nachzuegler, 0, block, 5_000);
    netz.commits_an(&empfaenger, &nachzuegler, 0, block, 5_000);

    assert_eq!(netz.commitete_bloecke(), vec![block]);
    assert_eq!(netz.anzahl_commitet(), N as usize);
}

// ── Übereinstimmung ─────────────────────────────────────────────────

#[test]
fn alle_knoten_kommen_zum_selben_zustand() {
    // Kap. 10.3: derselbe Nachrichtenverlauf ergibt auf jedem Knoten
    // denselben Zustand — Runde, Sperre und Stimmgewicht inklusive.
    let mut netz = Netz::neu();
    let block = hash(11);
    let empfaenger = alle();
    let t = netz.runde_wechseln(&alle());
    assert!(netz.propose_an(&empfaenger, &propose(1, block, 1), t).is_empty());
    let stimmende: Vec<u8> = (0..QUORUM_KNOTEN).collect();
    netz.votes_an(&empfaenger, &stimmende, 1, block, t);

    let referenz = (
        netz.knoten[0].round(),
        netz.knoten[0].lock(),
        netz.knoten[0].state().vote_weight(),
        netz.knoten[0].deadline_ms(),
    );
    for (i, d) in netz.knoten.iter().enumerate() {
        assert_eq!(
            (d.round(), d.lock(), d.state().vote_weight(), d.deadline_ms()),
            referenz,
            "Knoten {} weicht ab",
            i
        );
    }
}
