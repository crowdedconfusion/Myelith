//! Was der Konsens aushalten muss, wenn jemand lügt (K4).
//!
//! K4 verlangt „je Komponente eine adversariale Testebene: gefälschte
//! Signaturen". `liveness.rs` prüft, dass ehrliche Validatoren zu einem
//! Block kommen, und das ist der **Erfolgsfall**. Hier steht der
//! Gegenfall: Jeder Test unten beschreibt einen Angriff, und jeder muss
//! **scheitern**.
//!
//! Die Angriffe sind nicht ausgedacht, sondern die, gegen die das Format
//! ausdrücklich gebaut ist. Wo ein Kommentar im Quelltext sagt „das
//! schließt X aus", steht hier der Test, der X versucht.

use myl_consensus::bft::{Round, Vote};
use myl_consensus::round_change::PolkaCertificate;
use myl_consensus::signing::vote_message;
use myl_consensus::validator::{VotingMember, VotingSet};
use myl_types::bls::{aggregate_signatures, BlsAggregateSignature, BlsPublicKey, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;
use std::collections::BTreeMap;

const N: u8 = 21;
const GEWICHT: u64 = 100;
/// 15 × 100 = 1500 ≥ Quorum; 14 wären 1400 und damit knapp darunter.
const QUORUM_KNOTEN: u8 = 15;

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
        members.insert(miner(i), VotingMember { pubkey: pk, weight: GEWICHT });
    }
    VotingSet::from_members(members)
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

/// Ein gültiges Zertifikat als Ausgangspunkt, damit jeder Angriff
/// **genau eine** Sache verändert.
fn gueltiges_zertifikat(round: Round, block: Hash) -> PolkaCertificate {
    let votes: Vec<Vote> = (0..QUORUM_KNOTEN).map(|i| vote(round, block, i)).collect();
    let sigs: Vec<_> = votes.iter().map(|v| v.signature).collect();
    PolkaCertificate {
        round,
        block_hash: block,
        voters: (0..QUORUM_KNOTEN).map(miner).collect(),
        aggregate: aggregate_signatures(&sigs).expect("aggregieren"),
    }
}

/// Zuerst die Gegenprobe: Das ehrliche Zertifikat **muss** gelten.
///
/// Ohne sie wäre jeder Test darunter wertlos, denn eine Prüfung, die
/// alles ablehnt, lehnt auch jeden Angriff ab.
#[test]
fn das_ehrliche_zertifikat_gilt() {
    let vs = voting_set();
    let z = gueltiges_zertifikat(3u64, hash(9));
    assert!(z.verify(&vs).is_ok(), "der Erfolgsfall muss gelten");
}

/// **Angriff: dieselbe Stimme mehrfach einsetzen.**
///
/// Der Quelltext sagt zur Sortierung: „schließt Duplikate strukturell
/// aus. Ohne Duplikatschutz könnte ein Angreifer dieselbe Stimme
/// mehrfach einsetzen und das Quorum mit einem einzigen Schlüssel
/// erreichen." Genau das wird hier versucht.
#[test]
fn ein_unterzeichner_erreicht_das_quorum_nicht_durch_wiederholung() {
    let vs = voting_set();
    let (round, block) = (3u64, hash(9));
    let (sk, _) = keypair(0);
    let sig = sk.sign(&vote_message(round, &block)).expect("sign");

    let z = PolkaCertificate {
        round,
        block_hash: block,
        voters: vec![miner(0); QUORUM_KNOTEN as usize],
        aggregate: aggregate_signatures(&vec![sig; QUORUM_KNOTEN as usize]).expect("agg"),
    };
    assert!(
        z.verify(&vs).is_err(),
        "fünfzehnmal derselbe Unterzeichner darf kein Quorum sein"
    );
}

/// **Angriff: die kanonische Reihenfolge verlassen.**
///
/// Ein Stimmensatz hat genau eine Kodierung. Zwei Kodierungen desselben
/// Satzes wären zwei Zertifikate, und damit wäre die Gleichheit zweier
/// Zertifikate nicht mehr entscheidbar.
#[test]
fn ein_unsortiertes_zertifikat_wird_abgelehnt() {
    let vs = voting_set();
    let mut z = gueltiges_zertifikat(3u64, hash(9));
    z.voters.swap(0, 1);
    assert!(z.verify(&vs).is_err(), "unsortiert ist nicht kanonisch");
}

/// **Angriff: ein Unterzeichner, der nicht im Komitee sitzt.**
#[test]
fn ein_fremder_unterzeichner_wird_abgelehnt() {
    let vs = voting_set();
    let (round, block) = (3u64, hash(9));
    let fremd = 200u8; // außerhalb von 0..N
    let mut votes: Vec<Vote> = (0..QUORUM_KNOTEN - 1).map(|i| vote(round, block, i)).collect();
    votes.push(vote(round, block, fremd));
    let sigs: Vec<_> = votes.iter().map(|v| v.signature).collect();

    let mut voters: Vec<MinerId> = (0..QUORUM_KNOTEN - 1).map(miner).collect();
    voters.push(miner(fremd));
    voters.sort();

    let z = PolkaCertificate {
        round,
        block_hash: block,
        voters,
        aggregate: aggregate_signatures(&sigs).expect("agg"),
    };
    assert!(z.verify(&vs).is_err(), "wer nicht im Komitee ist, stimmt nicht mit");
}

/// **Angriff: knapp unter dem Quorum bleiben und trotzdem gelten wollen.**
#[test]
fn knapp_unter_dem_quorum_gilt_nicht() {
    let vs = voting_set();
    let (round, block) = (3u64, hash(9));
    let n = QUORUM_KNOTEN - 1; // 14 × 100 = 1400
    let votes: Vec<Vote> = (0..n).map(|i| vote(round, block, i)).collect();
    let sigs: Vec<_> = votes.iter().map(|v| v.signature).collect();
    let z = PolkaCertificate {
        round,
        block_hash: block,
        voters: (0..n).map(miner).collect(),
        aggregate: aggregate_signatures(&sigs).expect("agg"),
    };
    assert!(z.verify(&vs).is_err(), "ein Stimmgewicht unter dem Quorum ist keins");
}

/// **Angriff: das Zertifikat auf einen anderen Block umschreiben.**
///
/// Die Signaturen gelten für `vote_message(round, block)`. Wer den Block
/// austauscht, hat eine Unterschrift unter einem anderen Text.
#[test]
fn ein_umetikettierter_block_wird_abgelehnt() {
    let vs = voting_set();
    let mut z = gueltiges_zertifikat(3u64, hash(9));
    z.block_hash = hash(10);
    assert!(z.verify(&vs).is_err(), "die Unterschrift gilt dem alten Block");
}

/// **Angriff: dasselbe Zertifikat in einer anderen Runde einsetzen.**
///
/// Ohne Rundenbindung ließe sich ein altes Polka wiederverwenden, um
/// gesperrte Validatoren zu entsperren; das ist der Weg zu zwei Blöcken
/// auf derselben Höhe (BFT-Safety, vgl. Fund 27).
#[test]
fn ein_zertifikat_aus_einer_anderen_runde_wird_abgelehnt() {
    let vs = voting_set();
    let mut z = gueltiges_zertifikat(3u64, hash(9));
    z.round = 4u64;
    assert!(z.verify(&vs).is_err(), "ein Polka gilt nur in seiner Runde");
}

/// **Angriff: eine erfundene Signatur.**
#[test]
fn eine_erfundene_signatur_wird_abgelehnt() {
    let vs = voting_set();
    let mut z = gueltiges_zertifikat(3u64, hash(9));
    z.aggregate = BlsAggregateSignature([0x5Au8; 96]);
    assert!(z.verify(&vs).is_err(), "erfundene Bytes sind keine Signatur");
}

/// **Ein leeres Zertifikat ist keins.**
#[test]
fn ein_leeres_zertifikat_wird_abgelehnt() {
    let vs = voting_set();
    let z = PolkaCertificate {
        round: 3u64,
        block_hash: hash(9),
        voters: vec![],
        aggregate: BlsAggregateSignature([0u8; 96]),
    };
    assert!(z.verify(&vs).is_err(), "null Stimmen sind kein Quorum");
}

/// **Zufällige Zertifikate dürfen nie gelten und nie abstürzen.**
///
/// Die Prüfung liest fremde Bytes als Signatur und als Unterzeichnerliste.
/// Eine Panik darin wäre im offenen Netz ein Denial-of-Service.
#[test]
fn zufaellige_zertifikate_gelten_nie_und_stuerzen_nie_ab() {
    let vs = voting_set();
    let mut z_state = 0x243F_6A88_85A3_08D3u64;
    let mut wuerfel = || {
        z_state ^= z_state << 13;
        z_state ^= z_state >> 7;
        z_state ^= z_state << 17;
        z_state
    };

    for _ in 0..20_000 {
        let n = (wuerfel() % 30) as usize;
        let mut voters: Vec<MinerId> = (0..n)
            .map(|_| MinerId::new([(wuerfel() & 0xff) as u8; 32]))
            .collect();
        if wuerfel() % 2 == 0 {
            voters.sort();
            voters.dedup();
        }
        let mut agg = [0u8; 96];
        for b in agg.iter_mut() {
            *b = (wuerfel() & 0xff) as u8;
        }
        let z = PolkaCertificate {
            round: wuerfel(),
            block_hash: Hash::sha256(&wuerfel().to_le_bytes()),
            voters,
            aggregate: BlsAggregateSignature(agg),
        };
        assert!(
            z.verify(&vs).is_err(),
            "ein zufälliges Zertifikat darf niemals gelten"
        );
    }
}
