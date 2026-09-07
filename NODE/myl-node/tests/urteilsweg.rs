//! Der Weg vom Beleg zum geschlachteten Einsatz, über einen Block.
//!
//! # ⚑ Was hier zum ersten Mal läuft
//!
//! `myl_ledger::apply_verdict` gab es seit jeher, und es hatte
//! **ausserhalb seiner Tests keinen Aufrufer**. Daneben lag
//! `Block::verdicts`, ein Feld des gehashten Blocks, das die Kette
//! weder prüfte noch anwandte. Beides zusammen war Fund 192: zwei
//! Enden, und wer sie ohne Beleg verbände, gäbe dem Blockerzeuger ein
//! Werkzeug, mit dem er jeden schlachten kann.
//!
//! **Die Anweisung trägt den Beleg mit sich**, und der Beleg trägt sich
//! selbst: Er ist die eigene Unterschrift des Beschuldigten über den
//! strittigen Übergang, in der Rolle Shard.
//!
//! ⚑ **Drei Tests, und der mittlere ist der wichtigste.** Der erste
//! zeigt, dass es geht; der zweite, dass es **ohne Beleg nicht** geht;
//! der dritte, dass ein gültiger Beleg **keine Waffe gegen Dritte** ist.

use myl_consensus::block::{Anweisung, Transaktion};
use myl_node::kette::{probekonto, probeschluessel, Kette};
use myl_types::ids::{Address, SegmentId};
use myl_types::schuldbeleg::{Belegart, Schuldbeleg};
use myl_types::uebergang::{Rolle, TransitionSig};

/// Der strittige Übergang, immer derselbe.
fn uebergang() -> TransitionSig {
    TransitionSig {
        segment_id: SegmentId::new([5u8; 32]),
        shard_index: 2,
        position: 7,
        prev_hash: [1u8; 32],
        next_hash: [2u8; 32],
    }
}

/// Ein Beleg, den der Miner mit dem Schlüssel `nr` selbst unterschreibt.
fn beleg_von(nr: u8) -> Belegart {
    let sk = probeschluessel(nr);
    let u = uebergang();
    let sig = sk
        .sign(&u.to_sign_bytes_mit_rolle(Rolle::Shard))
        .expect("unterschreiben");
    Belegart::PrimaerHatGerechnet(Schuldbeleg {
        uebergang: u,
        schluessel: sk.public_key().expect("Schluessel"),
        signatur: sig,
    })
}

/// Setzt einen Einsatz und gibt die Kette zurück.
fn kette_mit_einsatz(wer: &Address, betrag: u64) -> Kette {
    let mut k = Kette::probestand();
    k.zustand_mut().account_mut(wer).staked = betrag;
    k
}

/// ⚑ **Ein Schuldspruch mit Beleg schlachtet, über einen Block.**
#[test]
fn ein_beleg_schlachtet_ueber_einen_block() {
    let schuldig = probekonto(3);
    let anzeigend = probekonto(4);
    let mut k = kette_mit_einsatz(&schuldig, 100_000_000);
    // ⚑ **Der Anfangsstand wird gemerkt und nicht angenommen.** Die
    // Probekonten tragen Guthaben aus dem Probestand; eine absolute
    // Zahl pruefte hier die Genesis mit.
    let vorher = k.zustand().account(&anzeigend).balance;

    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::SchuldspruchEinreichen {
                segment: SegmentId::new([5u8; 32]),
                beleg: beleg_von(3),
                beschuldigt: schuldig,
                anzeigend,
            },
        )
        .expect("signieren"),
    );
    k.baue_block();

    assert_eq!(
        k.zustand().account(&schuldig).staked,
        0,
        "der Einsatz wurde nicht geschlachtet"
    );
    // Das Kopfgeld sind dreissig Prozent des geschlachteten Betrags.
    assert_eq!(
        k.zustand().account(&anzeigend).balance - vorher,
        30_000_000,
        "das Kopfgeld kam nicht an"
    );
}

/// ⚑ **Ohne gueltigen Beleg geschieht nichts.**
///
/// Der Beleg traegt hier die Unterschrift eines **anderen** Uebergangs,
/// ist also fuer sich genommen echt und fuer diesen Fall wertlos.
#[test]
fn ein_gefaelschter_beleg_schlachtet_nicht() {
    let schuldig = probekonto(3);
    let anzeigend = probekonto(4);
    let mut k = kette_mit_einsatz(&schuldig, 100_000_000);

    // Ein Beleg mit vertauschter Signatur: der Schluessel gehoert zu 3,
    // die Unterschrift stammt von 5.
    let u = uebergang();
    let sig = probeschluessel(5)
        .sign(&u.to_sign_bytes_mit_rolle(Rolle::Shard))
        .expect("unterschreiben");
    let falsch = Belegart::PrimaerHatGerechnet(Schuldbeleg {
        uebergang: u,
        schluessel: probeschluessel(3).public_key().expect("Schluessel"),
        signatur: sig,
    });

    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::SchuldspruchEinreichen {
                segment: SegmentId::new([5u8; 32]),
                beleg: falsch,
                beschuldigt: schuldig,
                anzeigend,
            },
        )
        .expect("signieren"),
    );
    k.baue_block();

    assert_eq!(
        k.zustand().account(&schuldig).staked,
        100_000_000,
        "ein gefaelschter Beleg hat geschlachtet"
    );
}

/// ⚑ **Ein echter Beleg ist keine Waffe gegen Dritte.**
///
/// Der Beleg gehoert zu Miner 3 und ist gueltig; benannt wird aber
/// Konto 6. Ohne diese Pruefung koennte jeder, der irgendeinen echten
/// Beleg besitzt, einen Beliebigen schlachten.
#[test]
fn ein_echter_beleg_trifft_keinen_dritten() {
    let unbeteiligt = probekonto(6);
    let anzeigend = probekonto(4);
    let mut k = kette_mit_einsatz(&unbeteiligt, 100_000_000);

    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::SchuldspruchEinreichen {
                segment: SegmentId::new([5u8; 32]),
                beleg: beleg_von(3),
                beschuldigt: unbeteiligt,
                anzeigend,
            },
        )
        .expect("signieren"),
    );
    k.baue_block();

    assert_eq!(
        k.zustand().account(&unbeteiligt).staked,
        100_000_000,
        "ein fremder Beleg hat einen Dritten geschlachtet"
    );
}
