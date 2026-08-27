//! Protokoll-Durchlauf über alle Komponenten (`myl-test stack`).
//!
//! Bis v0.1.0 prüfte der Client nur INTEGER_LLM (Determinismus) und
//! COMPUTE_PIPELINE (Sharding). **Sieben Crates fasste er nicht an**:
//! `myl-types`, `-ledger`, `-scheduler`, `-consensus`, `-tokenomics`,
//! `-verifier` und `-net`. Die haben zwar Unit-Tests, aber niemand
//! prüfte, ob sie **zusammen** funktionieren.
//!
//! Dieser Lauf schließt die Lücke: Er fährt die Protokollkette einmal
//! durch: von der Kryptografie über die Epochenzuteilung, den
//! BFT-Konsens, die Verifikation bis zur Ledger-Buchung und der
//! Preisbildung. Jede Stufe schreibt ihren Vergleichswert ins Protokoll.
//!
//! ## Warum das ein anderer Test ist als die Unit-Tests
//!
//! Unit-Tests prüfen ein Modul gegen seine eigenen Annahmen. Dieser Lauf
//! prüft, ob die Annahmen **zwischen** den Modulen zusammenpassen, und
//! genau dort lagen die schwersten Funde des Audits: Der Block konnte
//! nicht aufnehmen, was der Pod produziert (A8); der Verifier rechnete
//! mit einem Slashing-Modell, das der Ledger nicht kennt (A9). Beides
//! wäre hier sofort aufgefallen, weil der Lauf die Werte tatsächlich
//! weiterreicht statt sie nachzubauen.
//!
//! ## Was er nicht ist
//!
//! Kein Ersatz für die Unit-Tests und kein Netzwerktest: alles läuft
//! im selben Prozess. `myl-net` bleibt deshalb außen vor: Gossip über
//! echte Sockets gehört in die NETWORKING-Testsuite, nicht in ein
//! Diagnosewerkzeug.

use myl_consensus::bft::{BftState, Commit, Propose, Vote};
use myl_consensus::block::{Block, BurnTx, BlockHeader, Transaction};
use myl_consensus::signing::{commit_message, propose_message, vote_message};
use myl_consensus::validator::{select_committee, ValidatorRegistry, VotingSet};
use myl_consensus::{DoubleSignProof, SignedBlocksRegistry};
use myl_ledger::state::LedgerState;
use myl_ledger::transitions::{apply_verdict, burn_to_credits, SlashParams};
use myl_scheduler::{derive_epoch_seed, sample_segments};
use myl_types::bls::{aggregate_signatures, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MinerId, SegmentId};
use myl_types::merkle::MerkleTree;
use myl_types::vrf::VrfSecretKey;
use myl_verifier::{compare_commitments, create_slash_decision, CompareResult, VerdictOutcome};

use crate::logging::{sha256_hex, Event, RunLog};

/// Ergebnis einer Prüfstufe.
struct Stufe {
    name: &'static str,
    ok: bool,
    digest: String,
    beschreibung: String,
}

impl Stufe {
    fn ok(name: &'static str, digest: String, beschreibung: impl Into<String>) -> Self {
        Self {
            name,
            ok: true,
            digest,
            beschreibung: beschreibung.into(),
        }
    }
    fn fehler(name: &'static str, grund: impl Into<String>) -> Self {
        Self {
            name,
            ok: false,
            digest: String::new(),
            beschreibung: grund.into(),
        }
    }
}

/// `myl-test stack`, die Protokollkette einmal durch.
pub fn run_stack(log: &mut RunLog) -> bool {
    let fp = crate::hardware::Fingerprint::collect();
    for (k, v) in &fp.entries {
        log.event(Event::Hardware {
            key: k.clone(),
            value: v.clone(),
        });
    }

    let stufen = vec![
        stufe_kryptografie(),
        stufe_epochenseed(),
        stufe_stichprobe(),
        stufe_komiteewahl(),
        stufe_bft_runde(),
        stufe_double_signing(),
        stufe_block(),
        stufe_verifikation(),
        stufe_ledger(),
        stufe_tokenomics(),
    ];

    let mut alle_ok = true;
    for s in &stufen {
        if s.ok {
            log.result(s.name, &s.digest, s.beschreibung.clone());
        } else {
            log.error(format!("{}: {}", s.name, s.beschreibung));
            alle_ok = false;
        }
    }

    let gesamt: Vec<u8> = stufen
        .iter()
        .flat_map(|s| s.digest.as_bytes().to_vec())
        .collect();
    log.result(
        "stack_gesamt",
        &sha256_hex(&gesamt),
        format!("{} Stufen, {} bestanden", stufen.len(), stufen.iter().filter(|s| s.ok).count()),
    );
    log.note(
        "Der Gesamtwert deckt alle Stufen ab. Bei gleichem Code MUSS er \
         auf jeder Maschine identisch sein: er enthält keine Zeitwerte \
         und keine Zufallszahlen ohne festen Seed.",
    );

    alle_ok
}

// ── Stufe 1: Kryptografie (myl-types) ───────────────────────────────

fn stufe_kryptografie() -> Stufe {
    let blaetter: Vec<&[u8]> = vec![b"a", b"b", b"c", b"d"];
    let baum = match MerkleTree::new(&blaetter) {
        Ok(t) => t,
        Err(e) => return Stufe::fehler("krypto", format!("Merkle-Baum: {:?}", e)),
    };
    let wurzel = baum.root();
    let beweis = match baum.proof(2) {
        Ok(p) => p,
        Err(e) => return Stufe::fehler("krypto", format!("Merkle-Beweis: {:?}", e)),
    };
    if !beweis.verify(&wurzel, b"c", 2) {
        return Stufe::fehler("krypto", "gültiger Merkle-Beweis wurde abgelehnt");
    }
    // Gegenprobe: ein falsches Blatt darf NICHT verifizieren.
    if beweis.verify(&wurzel, b"x", 2) {
        return Stufe::fehler("krypto", "falsches Blatt wurde akzeptiert");
    }

    let sk = match BlsSecretKey::key_gen(&[3u8; 32]) {
        Ok(k) => k,
        Err(e) => return Stufe::fehler("krypto", format!("BLS KeyGen: {:?}", e)),
    };
    let pk = match sk.public_key() {
        Ok(p) => p,
        Err(e) => return Stufe::fehler("krypto", format!("BLS PubKey: {:?}", e)),
    };
    let sig = match sk.sign(b"myelith") {
        Ok(s) => s,
        Err(e) => return Stufe::fehler("krypto", format!("BLS Sign: {:?}", e)),
    };
    if !pk.verify(b"myelith", &sig) {
        return Stufe::fehler("krypto", "gültige BLS-Signatur wurde abgelehnt");
    }
    if pk.verify(b"anderes", &sig) {
        return Stufe::fehler("krypto", "BLS-Signatur galt für fremde Botschaft");
    }
    if aggregate_signatures(&[sig, sig]).is_err() {
        return Stufe::fehler("krypto", "BLS-Aggregation fehlgeschlagen");
    }

    let vrf_sk = VrfSecretKey::from_seed([7u8; 32]);
    let (beweis_vrf, ausgabe) = match vrf_sk.prove(b"epoche-1") {
        Ok(o) => o,
        Err(e) => return Stufe::fehler("krypto", format!("VRF: {:?}", e)),
    };
    // Gegenprobe: der Beweis muss gegen den passenden Schluessel gelten.
    if !vrf_sk.public_key().verify(b"epoche-1", &beweis_vrf).is_ok() {
        return Stufe::fehler("krypto", "gueltiger VRF-Beweis wurde abgelehnt");
    }

    Stufe::ok(
        "krypto",
        sha256_hex(&[wurzel.as_bytes().as_slice(), &sig.0, &ausgabe.beta].concat()),
        "Merkle (inkl. Negativprobe), BLS (Signatur, Fremdbotschaft, Aggregat), VRF",
    )
}

// ── Stufe 2: Epochenseed (myl-scheduler) ────────────────────────────

fn stufe_epochenseed() -> Stufe {
    let sk = VrfSecretKey::from_seed([9u8; 32]);
    let block = Hash::sha256(b"vorheriger-block");
    let a = match derive_epoch_seed(block, &sk, 42) {
        Ok(s) => s,
        Err(e) => return Stufe::fehler("epochenseed", format!("{:?}", e)),
    };
    let b = match derive_epoch_seed(block, &sk, 42) {
        Ok(s) => s,
        Err(e) => return Stufe::fehler("epochenseed", format!("{:?}", e)),
    };
    if a.beta != b.beta {
        return Stufe::fehler("epochenseed", "nicht deterministisch");
    }
    let c = match derive_epoch_seed(block, &sk, 43) {
        Ok(s) => s,
        Err(e) => return Stufe::fehler("epochenseed", format!("{:?}", e)),
    };
    if a.beta == c.beta {
        return Stufe::fehler("epochenseed", "Epochenwechsel ändert den Seed nicht");
    }
    if derive_epoch_seed(Hash::from_bytes([0u8; 32]), &sk, 42).is_ok() {
        return Stufe::fehler("epochenseed", "leerer Blockhash wurde akzeptiert");
    }
    Stufe::ok(
        "epochenseed",
        sha256_hex(&a.beta),
        "deterministisch, epochenabhängig, leerer Blockhash abgelehnt",
    )
}

// ── Stufe 3: Stichproben-Lotterie (myl-scheduler) ───────────────────

fn stufe_stichprobe() -> Stufe {
    let seed = [5u8; 32];
    let r = sample_segments(1000, 200, &seed); // 2 %
    if r.sampled_segments.len() != 20 {
        return Stufe::fehler(
            "stichprobe",
            format!("erwartete 20 Segmente, bekam {}", r.sampled_segments.len()),
        );
    }
    if sample_segments(1000, 200, &seed) != r {
        return Stufe::fehler("stichprobe", "nicht deterministisch");
    }
    if !r.sampled_segments.windows(2).all(|w| w[0] < w[1]) {
        return Stufe::fehler("stichprobe", "Ergebnis nicht kanonisch sortiert");
    }
    let bytes: Vec<u8> = r
        .sampled_segments
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    Stufe::ok(
        "stichprobe",
        sha256_hex(&bytes),
        format!("{} von 1000 Segmenten, sortiert und deterministisch", r.sampled_segments.len()),
    )
}

// ── Stufe 4: Komiteewahl (myl-consensus) ────────────────────────────

fn testschluessel(i: u8) -> BlsSecretKey {
    BlsSecretKey::key_gen(&[i.wrapping_add(1); 32]).expect("KeyGen")
}

fn stufe_komiteewahl() -> Stufe {
    let mut reg = ValidatorRegistry::new();
    for i in 0..40u8 {
        let pk = match testschluessel(i).public_key() {
            Ok(p) => p,
            Err(e) => return Stufe::fehler("komiteewahl", format!("PubKey: {:?}", e)),
        };
        // Besitznachweis ist seit Fund 27 Pflicht bei der Registrierung.
        let pop = match testschluessel(i).prove_possession() {
            Ok(p) => p,
            Err(e) => return Stufe::fehler("komiteewahl", format!("PoP: {:?}", e)),
        };
        if let Err(e) = reg.register(MinerId::new([i; 32]), pk, &pop, 10_000_000 + i as u64, 5) {
            return Stufe::fehler("komiteewahl", format!("Registrierung: {:?}", e));
        }
    }
    let k1 = match select_committee(&reg, 10, &[1u8; 32]) {
        Ok(c) => c,
        Err(e) => return Stufe::fehler("komiteewahl", format!("{:?}", e)),
    };
    if k1.producers.len() != 21 || k1.arbiters.len() != 7 {
        return Stufe::fehler("komiteewahl", "falsche Komiteegröße");
    }
    if select_committee(&reg, 10, &[1u8; 32]).map(|c| c.producers) != Ok(k1.producers.clone()) {
        return Stufe::fehler("komiteewahl", "nicht deterministisch");
    }
    // VRF-Rotation: andere Epoche → anderes Komitee.
    let k2 = match select_committee(&reg, 11, &[1u8; 32]) {
        Ok(c) => c,
        Err(e) => return Stufe::fehler("komiteewahl", format!("{:?}", e)),
    };
    if k1.producers == k2.producers {
        return Stufe::fehler("komiteewahl", "keine Rotation zwischen Epochen");
    }
    let bytes: Vec<u8> = k1.producers.iter().flat_map(|m| *m.as_bytes()).collect();
    Stufe::ok(
        "komiteewahl",
        sha256_hex(&bytes),
        "21 Producer + 7 Arbiter, deterministisch, rotiert über Epochen",
    )
}

// ── Stufe 5: BFT-Runde mit echten Signaturen (myl-consensus) ────────

fn stufe_bft_runde() -> Stufe {
    let mut reg = ValidatorRegistry::new();
    for i in 0..30u8 {
        let pk = match testschluessel(i).public_key() {
            Ok(p) => p,
            Err(e) => return Stufe::fehler("bft", format!("PubKey: {:?}", e)),
        };
        let pop = match testschluessel(i).prove_possession() {
            Ok(p) => p,
            Err(e) => return Stufe::fehler("bft", format!("PoP: {:?}", e)),
        };
        if reg
            .register(MinerId::new([i; 32]), pk, &pop, 10_000_000, 5)
            .is_err()
        {
            return Stufe::fehler("bft", "Registrierung fehlgeschlagen");
        }
    }
    let komitee = match select_committee(&reg, 10, &[2u8; 32]) {
        Ok(c) => c,
        Err(e) => return Stufe::fehler("bft", format!("Komitee: {:?}", e)),
    };
    let menge = match VotingSet::for_producers(&reg, &komitee, 10) {
        Ok(v) => v,
        Err(e) => return Stufe::fehler("bft", format!("VotingSet: {:?}", e)),
    };
    let leader = komitee.producers[0];
    let mut state = match BftState::new(1, leader, menge) {
        Ok(s) => s,
        Err(e) => return Stufe::fehler("bft", format!("BftState: {:?}", e)),
    };

    let block_hash = Hash::sha256(b"testblock");
    let idx_of = |m: &MinerId| -> u8 { m.as_bytes()[0] };

    let sk_leader = testschluessel(idx_of(&leader));
    let propose = Propose {
        round: 1,
        block_hash,
        leader,
        signature: sk_leader.sign(&propose_message(1, &block_hash)).expect("sign"),
    };
    if let Err(e) = state.receive_propose(&propose) {
        return Stufe::fehler("bft", format!("Propose abgelehnt: {:?}", e));
    }

    // Ein Nichtmitglied darf nicht zählen.
    let fremd = MinerId::new([200u8; 32]);
    let sk_fremd = testschluessel(200);
    let fremde_vote = Vote {
        round: 1,
        block_hash,
        voter: fremd,
        signature: sk_fremd.sign(&vote_message(1, &block_hash)).expect("sign"),
    };
    if state.receive_vote(&fremde_vote).is_ok() {
        return Stufe::fehler("bft", "Vote eines Nichtmitglieds wurde angenommen");
    }

    // Gefälschte Signatur darf nicht zählen.
    let mut gefaelscht = Vote {
        round: 1,
        block_hash,
        voter: komitee.producers[1],
        signature: sk_leader.sign(&vote_message(1, &block_hash)).expect("sign"),
    };
    if state.receive_vote(&gefaelscht).is_ok() {
        return Stufe::fehler("bft", "fremde Signatur wurde als Vote angenommen");
    }
    gefaelscht.signature = testschluessel(idx_of(&komitee.producers[1]))
        .sign(&vote_message(1, &block_hash))
        .expect("sign");
    if let Err(e) = state.receive_vote(&gefaelscht) {
        return Stufe::fehler("bft", format!("gültige Vote abgelehnt: {:?}", e));
    }

    // Quorum aufbauen.
    for m in komitee.producers.iter().skip(2) {
        let v = Vote {
            round: 1,
            block_hash,
            voter: *m,
            signature: testschluessel(idx_of(m))
                .sign(&vote_message(1, &block_hash))
                .expect("sign"),
        };
        let _ = state.receive_vote(&v);
    }
    for m in &komitee.producers {
        let c = Commit {
            round: 1,
            block_hash,
            committer: *m,
            signature: testschluessel(idx_of(m))
                .sign(&commit_message(1, &block_hash))
                .expect("sign"),
        };
        let _ = state.receive_commit(&c);
    }
    if !state.is_committed() {
        return Stufe::fehler("bft", "Quorum trotz vollständiger Stimmen nicht erreicht");
    }

    Stufe::ok(
        "bft",
        sha256_hex(block_hash.as_bytes()),
        format!(
            "commitet mit Gewicht {}/{} (Schwelle {}); Fremdstimme und Fremdsignatur abgelehnt",
            state.commit_weight(),
            state.voting_set().total_weight(),
            state.threshold()
        ),
    )
}

// ── Stufe 6: Double-Signing (myl-consensus) ─────────────────────────

fn stufe_double_signing() -> Stufe {
    let sk = testschluessel(77);
    let pk = match sk.public_key() {
        Ok(p) => p,
        Err(e) => return Stufe::fehler("double_signing", format!("{:?}", e)),
    };
    let miner = MinerId::new([77u8; 32]);
    let h1 = Hash::sha256(b"block-a");
    let h2 = Hash::sha256(b"block-b");

    let mut reg = SignedBlocksRegistry::new();
    let s1 = sk.sign(&vote_message(5, &h1)).expect("sign");
    let s2 = sk.sign(&vote_message(5, &h2)).expect("sign");
    if reg.register_signed_block(miner, 5, h1, s1).is_some() {
        return Stufe::fehler("double_signing", "erste Signatur galt schon als Konflikt");
    }
    let Some(beweis) = reg.register_signed_block(miner, 5, h2, s2) else {
        return Stufe::fehler("double_signing", "Double-Signing nicht erkannt");
    };
    if let Err(e) = beweis.verify(&pk) {
        return Stufe::fehler("double_signing", format!("erkannter Beweis ungültig: {:?}", e));
    }
    // Erfundener Beweis muss scheitern.
    let erfunden = DoubleSignProof {
        miner_id: miner,
        round: 5,
        block_hash_1: h1,
        block_hash_2: h2,
        signature_1: myl_types::bls::BlsSignature([1u8; 96]),
        signature_2: myl_types::bls::BlsSignature([2u8; 96]),
    };
    if erfunden.verify(&pk).is_ok() {
        return Stufe::fehler("double_signing", "erfundener Beweis wurde akzeptiert");
    }
    Stufe::ok(
        "double_signing",
        sha256_hex(beweis.hash().as_bytes()),
        "erkannter Beweis gilt, erfundener Beweis wird abgelehnt",
    )
}

// ── Stufe 7: Block mit kanonischen Typen (myl-consensus) ────────────

fn stufe_block() -> Stufe {
    let meta = BlockHeader {
        height: 75_600,
        epoch: 42,
        prev_block_hash: Hash::sha256(b"prev"),
        timestamp_ms: 1_700_000_000_000,
        state_root: Hash::sha256(b"state"),
    };
    let mut block = Block::new(meta);
    block.add_transaction(Transaction::Burn(BurnTx {
        sender: Address::new([1u8; 32]),
        amount: 5_000_000,
    }));
    let h1 = block.hash();

    // state_root muss in den Blockhash eingehen: sonst wäre eine
    // falsch gebuchte Zustandsänderung nicht erkennbar.
    let mut meta2 = block.header.clone();
    meta2.state_root = Hash::sha256(b"anderer-state");
    let block2 = Block::new(meta2);
    if block2.hash() == h1 {
        return Stufe::fehler("block", "state_root geht nicht in den Blockhash ein");
    }
    // **Höhe und Epoche sind zwei Dinge** (seit 2026-08-27). Der Kopf
    // trägt beide, und die Epoche folgt aus der Höhe. Ohne diese Prüfung
    // liefe der Durchlauf auch dann durch, wenn beide wieder dasselbe
    // wären — und dann bedeutete jede Frist „je Epoche" in Wahrheit
    // „je Block".
    let gerechnet = myl_consensus::epoche_fuer_hoehe(block.header.height);
    if gerechnet != block.header.epoch {
        return Stufe::fehler(
            "block",
            format!(
                "Höhe {} gehört zu Epoche {gerechnet}, der Kopf sagt {}",
                block.header.height, block.header.epoch
            ),
        );
    }
    // Und die Grenze liegt, wo sie liegen soll: Der nächste Block
    // gehört noch zur selben Epoche, der um eine Epochenlänge höhere
    // zur nächsten. Ein Test nur auf Gleichheit bestünde auch dann,
    // wenn jede Höhe ihre eigene Epoche wäre — die alte Doppelbelegung.
    let h = block.header.height;
    if myl_consensus::epoche_fuer_hoehe(h + 1) != block.header.epoch {
        return Stufe::fehler("block", "schon der nächste Block liegt in einer neuen Epoche");
    }
    if myl_consensus::epoche_fuer_hoehe(h + myl_consensus::BLOECKE_JE_EPOCHE)
        != block.header.epoch + 1
    {
        return Stufe::fehler("block", "eine Epochenlänge weiter ist nicht die nächste Epoche");
    }

    let bytes = match borsh_roundtrip(&block) {
        Ok(b) => b,
        Err(e) => return Stufe::fehler("block", e),
    };
    Stufe::ok(
        "block",
        sha256_hex(h1.as_bytes()),
        format!(
            "{} Einträge, Borsh-Rundtrip {} Bytes, state_root wirksam, \
             Höhe {} in Epoche {} ({} Blöcke je Epoche)",
            block.total_entries(),
            bytes,
            block.header.height,
            block.header.epoch,
            myl_consensus::BLOECKE_JE_EPOCHE
        ),
    )
}

fn borsh_roundtrip(block: &Block) -> Result<usize, String> {
    let bytes = borsh::to_vec(block).map_err(|e| format!("Serialisierung: {}", e))?;
    let zurueck: Block =
        borsh::from_slice(&bytes).map_err(|e| format!("Deserialisierung: {}", e))?;
    if &zurueck != block {
        return Err("Rundtrip verändert den Block".into());
    }
    Ok(bytes.len())
}

// ── Stufe 8: Verifikation (myl-verifier) ────────────────────────────

fn stufe_verifikation() -> Stufe {
    let gleich: Vec<Hash> = (0..8u8).map(|i| Hash::sha256(&[i])).collect();
    match compare_commitments(&gleich, &gleich) {
        Ok(CompareResult::Match) => {}
        other => return Stufe::fehler("verifikation", format!("Gleichstand nicht erkannt: {:?}", other)),
    }

    let mut abweichend = gleich.clone();
    abweichend[5] = Hash::sha256(b"manipuliert");
    let pos = match compare_commitments(&gleich, &abweichend) {
        Ok(CompareResult::Mismatch { first_divergence }) => first_divergence,
        other => return Stufe::fehler("verifikation", format!("Abweichung nicht erkannt: {:?}", other)),
    };
    if pos != 5 {
        return Stufe::fehler("verifikation", format!("Abweichung an Position {} statt 5", pos));
    }

    let entscheidung = match create_slash_decision(
        VerdictOutcome::PrimaryLoses,
        SegmentId::new([1u8; 32]),
        MinerId::new([1u8; 32]),
        MinerId::new([2u8; 32]),
        Some(pos),
    ) {
        Ok(d) => d,
        Err(e) => return Stufe::fehler("verifikation", format!("Slash-Entscheidung: {:?}", e)),
    };
    if entscheidung.slashed_miner != MinerId::new([1u8; 32]) {
        return Stufe::fehler("verifikation", "falscher Miner geslasht");
    }
    Stufe::ok(
        "verifikation",
        sha256_hex(&[pos as u8]),
        format!("Abweichung an Position {} lokalisiert, Schuld zugewiesen", pos),
    )
}

// ── Stufe 9: Ledger-Buchung (myl-verifier → myl-ledger) ─────────────

fn stufe_ledger() -> Stufe {
    let mut state = LedgerState::genesis(1_000);
    let schuldig = Address::new([1u8; 32]);
    let unschuldig = Address::new([2u8; 32]);
    state.account_mut(&schuldig).staked = 100_000_000;
    state.account_mut(&unschuldig).balance = 50_000_000;
    let vorher = state.commitment();

    // Burn → Credits.
    state.account_mut(&unschuldig).balance = 50_000_000;
    let credits = match burn_to_credits(&mut state, &unschuldig, 10_000, EpochId(5)) {
        Ok(c) => c,
        Err(e) => return Stufe::fehler("ledger", format!("burn_to_credits: {:?}", e)),
    };

    // Die Slash-Entscheidung des Verifiers durch den Ledger buchen:
    // genau die Schnittstelle, an der Fund A9 hing.
    let entscheidung = match create_slash_decision(
        VerdictOutcome::PrimaryLoses,
        SegmentId::new([9u8; 32]),
        MinerId::new([1u8; 32]),
        MinerId::new([2u8; 32]),
        Some(3),
    ) {
        Ok(d) => d,
        Err(e) => return Stufe::fehler("ledger", format!("Slash-Entscheidung: {:?}", e)),
    };
    let params = SlashParams {
        slash_fraction_num: 3,
        slash_fraction_den: 10,
        bounty_fraction_num: 1,
        bounty_fraction_den: 10,
    };
    let wirkung = match apply_verdict(
        &mut state,
        &entscheidung.to_ledger_verdict(schuldig, unschuldig),
        &params,
    ) {
        Ok(w) => w,
        Err(e) => return Stufe::fehler("ledger", format!("apply_verdict: {:?}", e)),
    };
    if wirkung.slashed != 30_000_000 {
        return Stufe::fehler(
            "ledger",
            format!("erwartete 30 % Slash, bekam {}", wirkung.slashed),
        );
    }
    // **Die Vorgeschichte, die daraus entsteht** (seit 2026-08-27). Ein
    // gebuchtes Urteil zählt beim Schuldigen; das ist die Grundlage der
    // Slashing-Staffelung. Ohne diese Prüfung liefe der Durchlauf auch
    // dann durch, wenn der Zähler stehen bliebe, und die Staffelung wäre
    // wieder eine Tabelle, von der immer die erste Zeile gilt.
    if wirkung.vorverstoesse != 0 {
        return Stufe::fehler(
            "ledger",
            format!("erster Verstoß mit {} Vorverstößen", wirkung.vorverstoesse),
        );
    }
    let vermerkt = state.verstoesse_im_fenster(&schuldig, myl_tokenomics::WIEDERHOLUNGSFENSTER);
    if vermerkt != 1 {
        return Stufe::fehler(
            "ledger",
            format!("das gebuchte Urteil wurde {} mal vermerkt statt einmal", vermerkt),
        );
    }

    // Und die Staffelung greift: drei Urteile, drei Stufen. Gefahren wird
    // sie über den Weg, der die Reihenfolge festlegt — der Satz hängt an
    // der Vorgeschichte VOR dem Urteil, und das Buchen verändert genau
    // diese Vorgeschichte.
    //
    // **Auf einem eigenen Konto**, damit der Verlauf die Tabelle aus
    // Kap. 5.5 zeigt (1/3/5 %) und nicht bei der zweiten Stufe beginnt:
    // Der Schuldige oben trägt bereits einen Verstoß.
    let wiederholer = Address::new([3u8; 32]);
    let mut stufen = Vec::new();
    for _ in 0..3 {
        state.account_mut(&wiederholer).staked = 100_000_000;
        let (_, satz) = match myl_tokenomics::slashing::urteil_buchen_gestaffelt(
            &mut state,
            &entscheidung.to_ledger_verdict(wiederholer, unschuldig),
            myl_tokenomics::slashing::Akteur::ShardMiner,
            myl_tokenomics::slashing::Grund::Nichtverfuegbarkeit,
        ) {
            Ok(w) => w,
            Err(e) => return Stufe::fehler("ledger", format!("Staffelung: {}", e)),
        };
        stufen.push(satz.anteil_bps());
    }
    if stufen != vec![100, 300, 500] {
        return Stufe::fehler(
            "ledger",
            format!("Kap. 5.5 nennt 1/3/5 %, gestaffelt; bekommen {stufen:?} Basispunkte"),
        );
    }

    let nachher = state.commitment();
    if vorher == nachher {
        return Stufe::fehler("ledger", "Zustandsänderung ohne Commitment-Änderung");
    }
    Stufe::ok(
        "ledger",
        sha256_hex(nachher.as_bytes()),
        format!(
            "{} Credits geprägt, {} geslasht, {} Kopfgeld. Verifier-Entscheidung durchgebucht, \
             Verstoß vermerkt, Staffelung 1/3/5 % über drei Urteile",
            credits, wirkung.slashed, wirkung.bounty
        ),
    )
}

// ── Stufe 10: Tokenomics (myl-tokenomics) ───────────────────────────

fn stufe_tokenomics() -> Stufe {
    use myl_tokenomics::{ema_update, exp_approx, mint_amount, update_price, MintParams};

    let mut ema = 0u64;
    for _ in 0..40 {
        ema = ema_update(ema, 1_000_000);
    }
    if ema == 0 {
        return Stufe::fehler("tokenomics", "EMA bleibt bei null");
    }

    let params = MintParams {
        subsidy_num: 1,
        subsidy_den: 10,
        m_max: 10_000_000_000,
    };
    let m = mint_amount(ema, &params);
    if m == 0 {
        return Stufe::fehler("tokenomics", "Prägung null trotz Burn-Historie");
    }

    // exp gegen den eingefrorenen Golden Vector.
    if exp_approx(65_536) != 11_675_001_401 {
        return Stufe::fehler("tokenomics", "exp-LUT weicht vom Golden Vector ab");
    }

    // Überlast muss den Preis heben, Unterlast senken.
    let preis = 1i64 << 32;
    let kappa = 6_553; // ~0,1
    let hoch = update_price(preis, kappa, 58_982, 45_875); // u=0,9 > u*=0,7
    let runter = update_price(preis, kappa, 32_768, 45_875); // u=0,5 < u*=0,7
    if hoch <= preis {
        return Stufe::fehler("tokenomics", "Preis steigt bei Überlast nicht");
    }
    if runter >= preis {
        return Stufe::fehler("tokenomics", "Preis sinkt bei Unterlast nicht");
    }

    Stufe::ok(
        "tokenomics",
        sha256_hex(&[ema.to_le_bytes(), m.to_le_bytes()].concat()),
        format!("EMA {}, Prägung {}, exp-LUT exakt, Preis reagiert richtungsrichtig", ema, m),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-stack-{}", name));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Der Durchlauf muss ohne Artefakte und ohne Netz funktionieren:
    /// er prüft die Protokollschicht, nicht das Modell.
    #[test]
    fn stack_laeuft_ohne_artefakte_durch() {
        let dir = tempdir("voll");
        let mut log = RunLog::new(&dir, "stack", false);
        let ok = run_stack(&mut log);
        let lauf_dir = log.dir().to_path_buf();
        let dateiname = log.dateiname().to_string();
        log.finish(ok);

        assert!(ok, "Stack-Durchlauf fehlgeschlagen");
        let jsonl = std::fs::read_to_string(lauf_dir.join(format!("{}.jsonl", dateiname))).unwrap();
        for stufe in [
            "krypto",
            "epochenseed",
            "stichprobe",
            "komiteewahl",
            "bft",
            "double_signing",
            "block",
            "verifikation",
            "ledger",
            "tokenomics",
            "stack_gesamt",
        ] {
            assert!(jsonl.contains(&format!("\"name\":\"{}\"", stufe)), "Stufe {} fehlt", stufe);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Der Gesamtwert darf nicht von Laufzeit oder Zufall abhängen:
    /// sonst ist er zwischen Maschinen nicht vergleichbar.
    #[test]
    fn gesamtwert_ist_ueber_laeufe_stabil() {
        let d1 = tempdir("stabil1");
        let d2 = tempdir("stabil2");
        let mut l1 = RunLog::new(&d1, "stack", false);
        let mut l2 = RunLog::new(&d2, "stack", false);
        run_stack(&mut l1);
        run_stack(&mut l2);
        let (id1, id2) = (l1.run_id().to_string(), l2.run_id().to_string());
        let (dateiname1, dateiname2) = (l1.dateiname().to_string(), l2.dateiname().to_string());
        let (d1, d2) = (l1.dir().to_path_buf(), l2.dir().to_path_buf());
        l1.finish(true);
        l2.finish(true);

        // Beide Läufe schreiben in dieselbe Datei (gleicher Einstellungen-Hash).
        // Die Zeile wird über die Laufkennung (run_id) gefunden.
        let hole = |dir: &std::path::Path, id: &str, dateiname: &str| -> String {
            let t = std::fs::read_to_string(dir.join(format!("{}.jsonl", dateiname))).unwrap();
            t.lines()
                .filter(|l| l.contains(id))
                .find(|l| l.contains("stack_gesamt"))
                .map(|l| {
                    l.split("\"digest\":\"")
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                        .to_string()
                })
                .expect("stack_gesamt")
        };
        assert_eq!(hole(&d1, &id1, &dateiname1), hole(&d2, &id2, &dateiname2));
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
    }

    #[test]
    fn jede_stufe_liefert_einen_digest() {
        for s in [
            stufe_kryptografie(),
            stufe_epochenseed(),
            stufe_stichprobe(),
            stufe_komiteewahl(),
            stufe_bft_runde(),
            stufe_double_signing(),
            stufe_block(),
            stufe_verifikation(),
            stufe_ledger(),
            stufe_tokenomics(),
        ] {
            assert!(s.ok, "Stufe {} fehlgeschlagen: {}", s.name, s.beschreibung);
            assert_eq!(s.digest.len(), 64, "Stufe {} ohne SHA-256-Digest", s.name);
        }
    }
}
