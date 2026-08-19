//! Regression zu Fund 27 — Rogue-Key-Angriff auf `FastAggregateVerify`.
//!
//! Bis 2026-08-19 sagte der Modulkopf von `bls.rs` zu, die Identitäts-
//! und Subgruppen-Prüfung öffentlicher Schlüssel schütze gegen
//! Rogue-Key-Angriffe. Diese Datei hält fest, **warum das falsch war**
//! und **was stattdessen schützt** — beides als ausführbarer Nachweis,
//! damit die Behauptung nicht erneut aus Plausibilität entsteht.
//!
//! ## Die Konstruktion
//!
//! Zu einem fremden `pk_opfer` bildet der Angreifer mit eigenem
//! Geheimnis `x`:
//!
//! ```text
//! pk_rogue = g₁^x · pk_opfer⁻¹
//! ```
//!
//! Dann ist `pk_opfer · pk_rogue = g₁^x`, und eine Signatur, die der
//! Angreifer **allein** mit `x` erzeugt, verifiziert als Aggregat beider
//! Schlüssel. Das Opfer hat nie unterschrieben.
//!
//! `pk_rogue` liegt in der richtigen Untergruppe und ist nicht die
//! Identität — beide Prüfungen gehen durch. Sie sind nicht nutzlos, sie
//! wehren nur ein anderes Problem ab.
//!
//! ## Was schützt
//!
//! Der Angreifer kennt den diskreten Logarithmus von `pk_rogue` nicht
//! (er wäre `x − sk_opfer`, und `sk_opfer` ist ihm unbekannt). Er kann
//! deshalb keinen Besitznachweis dafür erzeugen. Wer Schlüssel nur mit
//! geprüftem `BlsProofOfPossession` in eine Menge aufnimmt, gegen die
//! später aggregiert verifiziert wird, ist gegen diesen Angriff dicht.
//!
//! Diese Datei liegt bewusst unter `tests/` und nicht im Crate: die
//! Konstruktion braucht `blst`-Punktarithmetik und damit `unsafe`, was
//! `myl-types` selbst per `#![deny(unsafe_code)]` ausschließt. Ein
//! Integrationstest ist eine eigene Übersetzungseinheit.

use blst::min_pk::{PublicKey, SecretKey};
use myl_types::bls::{
    BLS_DST, BlsAggregateSignature, BlsProofOfPossession, BlsPublicKey, BlsSecretKey,
    fast_aggregate_verify,
};

/// Baut `pk_rogue = pk_ehrlich · pk_opfer⁻¹` (additiv: die Differenz der
/// beiden G1-Punkte) und gibt die komprimierten Bytes zurück.
fn rogue_key(pk_ehrlich: &PublicKey, pk_opfer: &PublicKey) -> [u8; 48] {
    // Sicherheit: reine Punktarithmetik auf blst-eigenen Typen. Alle
    // Eingaben stammen aus `compress()` gültiger Schlüssel, die Puffer
    // haben die von blst vorgeschriebenen festen Größen.
    #[allow(unsafe_code)]
    unsafe {
        let mut p_ehrlich = blst::blst_p1::default();
        let mut a = blst::blst_p1_affine::default();
        blst::blst_p1_uncompress(&mut a, pk_ehrlich.compress().as_ptr());
        blst::blst_p1_from_affine(&mut p_ehrlich, &a);

        let mut p_opfer = blst::blst_p1::default();
        let mut b = blst::blst_p1_affine::default();
        blst::blst_p1_uncompress(&mut b, pk_opfer.compress().as_ptr());
        blst::blst_p1_from_affine(&mut p_opfer, &b);
        blst::blst_p1_cneg(&mut p_opfer, true);

        let mut summe = blst::blst_p1::default();
        blst::blst_p1_add_or_double(&mut summe, &p_ehrlich, &p_opfer);

        let mut out = [0u8; 48];
        blst::blst_p1_compress(out.as_mut_ptr(), &summe);
        out
    }
}

/// Opfer, Angreifer und der daraus gebaute Rogue-Key.
struct Aufbau {
    pk_opfer: BlsPublicKey,
    pk_rogue: BlsPublicKey,
    /// Signatur, die der Angreifer **allein** erzeugt hat.
    aggregat: BlsAggregateSignature,
    botschaft: Vec<u8>,
}

fn aufbau() -> Aufbau {
    let botschaft = b"MYELITH: beliebige aggregiert gepruefte Botschaft".to_vec();

    // Ehrliches Mitglied. Signiert in diesem Test nie.
    let sk_opfer = SecretKey::key_gen(&[7u8; 32], &[]).expect("key_gen");
    let pk_opfer = sk_opfer.sk_to_pk();

    // Angreifer mit Geheimnis x.
    let sk_adv = SecretKey::key_gen(&[9u8; 32], &[]).expect("key_gen");
    let pk_adv = sk_adv.sk_to_pk();

    let rogue_bytes = rogue_key(&pk_adv, &pk_opfer);
    let sig = sk_adv.sign(&botschaft, BLS_DST, &[]);

    Aufbau {
        pk_opfer: BlsPublicKey(pk_opfer.compress()),
        pk_rogue: BlsPublicKey(rogue_bytes),
        aggregat: BlsAggregateSignature(sig.compress()),
        botschaft,
    }
}

#[test]
fn rogue_key_besteht_identitaets_und_subgruppenpruefung() {
    // Der Kern des Fundes: die Prüfungen, die als Schutz galten, greifen
    // hier nicht — der Punkt ist völlig regulär.
    let a = aufbau();
    assert!(
        a.pk_rogue.validate().is_ok(),
        "der Rogue-Key ist ein gültiger G1-Punkt in der Untergruppe"
    );
}

#[test]
fn rogue_key_haelt_fast_aggregate_verify_nicht_stand() {
    // Dokumentiert die Lücke: ohne Besitznachweis genügt eine einzige
    // Signatur, um ein Aggregat unter zwei Schlüsseln gelten zu lassen.
    let a = aufbau();
    assert!(
        fast_aggregate_verify(&[a.pk_opfer, a.pk_rogue], &a.botschaft, &a.aggregat),
        "Wenn das hier fehlschlägt, hat sich das Verhalten von \
         fast_aggregate_verify geändert — Fund 27 neu bewerten"
    );
}

#[test]
fn besitznachweis_schliesst_den_rogue_key_aus() {
    // Die eigentliche Regression: der Angreifer kennt den diskreten
    // Logarithmus von pk_rogue nicht und kann keinen Nachweis liefern.
    let a = aufbau();

    // Kein gültiger Nachweis ist konstruierbar; die naheliegenden
    // Versuche scheitern alle.
    let sk_adv = BlsSecretKey::key_gen(&[9u8; 32]).expect("key_gen");
    let pop_des_angreifers = sk_adv.prove_possession().expect("pop");
    assert!(
        !a.pk_rogue.verify_possession(&pop_des_angreifers),
        "der Nachweis des Angreiferschlüssels darf nicht für den Rogue-Key gelten"
    );
    assert!(
        !a.pk_rogue
            .verify_possession(&BlsProofOfPossession(a.aggregat.0)),
        "die Angriffssignatur darf nicht als Besitznachweis durchgehen"
    );
    assert!(!a.pk_rogue.verify_possession(&BlsProofOfPossession([0u8; 96])));
}

#[test]
fn ehrliche_schluessel_bestehen_den_besitznachweis() {
    // Gegenprobe: die Härtung darf keine gültige Registrierung
    // verhindern.
    for seed in 1..=5u8 {
        let sk = BlsSecretKey::key_gen(&[seed; 32]).expect("key_gen");
        let pk = sk.public_key().expect("pk");
        let pop = sk.prove_possession().expect("pop");
        assert!(pk.verify_possession(&pop), "Seed {}", seed);
    }
}

#[test]
fn aggregat_ohne_opfersignatur_scheitert_bei_ehrlichen_schluesseln() {
    // Zeigt, dass die Lücke ausschließlich am konstruierten Schlüssel
    // hängt: mit dem ehrlichen Angreiferschlüssel wird dieselbe
    // Signatur korrekt abgelehnt.
    let a = aufbau();
    let sk_adv = SecretKey::key_gen(&[9u8; 32], &[]).expect("key_gen");
    let pk_adv_ehrlich = BlsPublicKey(sk_adv.sk_to_pk().compress());
    assert!(!fast_aggregate_verify(
        &[a.pk_opfer, pk_adv_ehrlich],
        &a.botschaft,
        &a.aggregat
    ));
}
