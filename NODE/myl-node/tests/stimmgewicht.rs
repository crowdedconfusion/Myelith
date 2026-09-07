//! Das Stimmgewicht folgt dem hinterlegten Einsatz, nicht einer Datei.
//!
//! # ⚑ Was sich damit ändert
//!
//! Bis zum 2026-09-06 kam das Gewicht aus `Stimmsatzdatei`, also aus
//! einer **Textdatei neben dem Programm**. Damit wog eine Zahl, die
//! niemand auf der Kette hinterlegt hatte, und `EinsatzHinterlegen`,
//! seit dem 2026-09-03 gebaut, wog nichts.
//!
//! ⚑ **Beide Enden lagen da, und niemand verband sie** (dasselbe Muster
//! wie bei Fund 192 und Fund 183). `staked` bewegte sich, das
//! Stimmgewicht nicht.
//!
//! **Arbeit qualifiziert, Stake wiegt** (Entscheidung A3 vom
//! 2026-09-02): Wer nicht im Minerregister steht, ist nicht
//! stimmberechtigt, gleich wie viel er hinterlegt hat; wer darin steht,
//! wiegt mit seinem Einsatz.

use myl_consensus::block::{Anweisung, Transaktion};
use myl_node::kette::{probekonto, probeschluessel, Kette};
use myl_types::ids::MinerId;
use myl_types::miner::HardwareClass;
use myl_types::node_metadata::GeoRegion;

/// Meldet Miner `nr` an und hinterlegt `einsatz`.
fn anmelden_mit_einsatz(k: &mut Kette, nr: u8, einsatz: u64) -> MinerId {
    let sk = probeschluessel(nr);
    let kennung = MinerId::aus_schluessel(&sk.public_key().expect("Schluessel"));
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &sk,
            0,
            Anweisung::MinerAnmelden {
                hardware: HardwareClass::MediumGpu,
                zone: GeoRegion::Europe,
                netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    let konto = probekonto(nr);
    k.zustand_mut().account_mut(&konto).staked = einsatz;
    kennung
}

/// ⚑ **Das Gewicht ist der Einsatz, nicht eine Datei.**
#[test]
fn das_gewicht_folgt_dem_einsatz() {
    let mut k = Kette::probestand();
    let schwelle = Kette::mindesteinsatz();
    let viel = anmelden_mit_einsatz(&mut k, 1, schwelle * 5);
    let wenig = anmelden_mit_einsatz(&mut k, 2, schwelle * 2);

    let satz = Kette::stimmberechtigte_aus_zustand(k.zustand(), schwelle);
    assert!(satz.contains(&viel), "der grosse Einsatz fehlt");
    assert!(satz.contains(&wenig), "der kleine Einsatz fehlt");
    assert_eq!(
        satz.weight(&viel),
        schwelle * 5,
        "das Gewicht ist nicht der Einsatz"
    );
    assert!(
        satz.weight(&viel) > satz.weight(&wenig),
        "wer mehr hinterlegt, wiegt nicht mehr"
    );
}

/// ⚑ **Unter der Schwelle stimmt niemand mit.**
///
/// Ein Konto mit einem Kleinstbetrag waere sonst stimmberechtigt und
/// verduennte die Zweidrittelmehrheit.
#[test]
fn unter_der_schwelle_zaehlt_niemand_mit() {
    let mut k = Kette::probestand();
    let schwelle = Kette::mindesteinsatz();
    let knapp_drunter = anmelden_mit_einsatz(&mut k, 3, schwelle - 1);
    let knapp_drueber = anmelden_mit_einsatz(&mut k, 4, schwelle);

    let satz = Kette::stimmberechtigte_aus_zustand(k.zustand(), schwelle);
    assert!(!satz.contains(&knapp_drunter), "unter der Schwelle stimmt jemand mit");
    assert!(satz.contains(&knapp_drueber), "genau auf der Schwelle fehlt jemand");
}

/// ⚑ **Arbeit qualifiziert:** Wer nicht im Register steht, stimmt nicht
/// mit, auch mit grossem Einsatz.
#[test]
fn ohne_anmeldung_kein_stimmrecht() {
    let mut k = Kette::probestand();
    let schwelle = Kette::mindesteinsatz();
    // Einsatz ohne Anmeldung.
    let konto = probekonto(5);
    k.zustand_mut().account_mut(&konto).staked = schwelle * 100;
    let kennung = MinerId::new(*konto.as_bytes());

    let satz = Kette::stimmberechtigte_aus_zustand(k.zustand(), schwelle);
    assert!(
        !satz.contains(&kennung),
        "ein nicht angemeldetes Konto ist stimmberechtigt"
    );
}
