//! Ein Schuldspruch ohne Beleg kommt in keinen Block (Fund 192).
//!
//! # ⚑ Warum eine Ablehnung, wo doch nichts angewandt wird
//!
//! `Block::verdicts` ist Teil des **gehashten** Blocks, und
//! `myl_ledger::apply_verdict` hat alles, was zum Schlachten nötig ist.
//! Bis zum 2026-09-06 lagen beide Enden da und niemand verband sie: Die
//! Kette prüfte das Feld nicht und wandte es nicht an.
//!
//! **Das ist die gefährlichere der beiden Lagen.** Ein Feld, das man
//! füllen kann und das niemand liest, sieht harmlos aus, bis jemand es
//! liest. Ein `Verdict` nennt Täter und Kopfgeldempfänger und trägt
//! **keinen Nachweis**; wer die Enden ohne Beleg verbände, gäbe dem
//! Blockerzeuger ein Werkzeug, mit dem er jeden schlachten kann.
//!
//! ⚑ **Die Ablehnung ist die sichere Zwischenlage.** Sie sagt nicht
//! „Schuldsprüche gibt es nicht", sondern „nicht ohne Beleg", und sie
//! zwingt den späteren Bau, den Beleg mitzuliefern, statt ihn zu
//! vergessen.

use myl_consensus::block::{Block, BlockHeader};
use myl_ledger::transitions::{Verdict, VerdictOutcome};
use myl_node::kette::{Kette, KettenFehler};
use myl_types::ids::{Address, SegmentId};

fn urteil() -> Verdict {
    Verdict {
        segment_id: SegmentId::new([1u8; 32]),
        miner: Address::new([3u8; 32]),
        checker: Address::new([4u8; 32]),
        outcome: VerdictOutcome::SlashMiner,
    }
}

/// Ein sonst gültiger Block wird allein wegen des Schuldspruchs
/// abgewiesen.
///
/// ⚑ **„Sonst gültig" ist der Kern des Tests.** Ein Block, der schon an
/// der Höhe oder der Wurzel scheitert, bewiese nichts über das
/// Urteilsfeld.
#[test]
fn ein_schuldspruch_ohne_beleg_wird_abgewiesen() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();

    // Der Erzeuger baut einen gewöhnlichen Block; der Folger nimmt ihn.
    let gut = erzeuger.baue_block();
    folger.uebernimm(&gut).expect("ein gewoehnlicher Block traegt");

    // Derselbe Block, nur mit einem Schuldspruch darin.
    let mut boese = Block::new(BlockHeader { ..gut.header.clone() });
    for tx in gut.txs.clone() {
        boese.add_transaction(tx);
    }
    boese.add_verdict(urteil());

    let mut zweiter = Kette::probestand();
    match zweiter.uebernimm(&boese) {
        Err(KettenFehler::SchuldspruchOhneBeleg { anzahl }) => assert_eq!(anzahl, 1),
        anderes => panic!("erwartet wurde eine Ablehnung wegen des Schuldspruchs, kam: {anderes:?}"),
    }
}

/// Die Gegenprobe: **ohne** Schuldspruch geht derselbe Block durch.
///
/// Ohne sie könnte der Test oben auch dann grün sein, wenn die Kette
/// jeden zweiten Block ablehnte.
#[test]
fn ohne_schuldspruch_geht_derselbe_block_durch() {
    let mut erzeuger = Kette::probestand();
    let block = erzeuger.baue_block();
    let mut folger = Kette::probestand();
    assert!(
        folger.uebernimm(&block).is_ok(),
        "derselbe Block ohne Schuldspruch muss durchgehen"
    );
}
