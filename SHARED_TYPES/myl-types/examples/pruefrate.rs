//! **Die Prüfrate der Unterschriften auf dieser Maschine** (2026-10-28):
//! Einzelprüfung, wie ein Knoten sie bis NODE 0.73.0 je Transaktion machte,
//! gegen die Stapelprüfung `bls::verify_batch`, die er seit NODE 0.74.0 beim
//! Anwenden macht.
//!
//! ```text
//! cargo run --release --example pruefrate -- [anzahl, Vorgabe 1000]
//! ```
//!
//! Eine Zeile auf stdout mit Mikrosekunden je Unterschrift und der Zahl der
//! Fäden. Wozu: Das Ziel des Grundpreises soll aus der Prüfrate der
//! kleinsten zugelassenen Maschine folgen, nicht aus der schnellsten.

use std::time::Instant;

use myl_types::bls::{verify_batch, BlsPublicKey, BlsSecretKey, BlsSignature};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1000).max(2);
    let mut pks: Vec<BlsPublicKey> = Vec::with_capacity(n);
    let mut sigs: Vec<BlsSignature> = Vec::with_capacity(n);
    let mut nachrichten: Vec<Vec<u8>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut ikm = [0u8; 32];
        ikm[..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
        let sk = BlsSecretKey::key_gen(&ikm).expect("KeyGen");
        let m = format!("transaktion-{i}").into_bytes();
        pks.push(sk.public_key().expect("Schlüssel"));
        sigs.push(sk.sign(&m).expect("Unterschrift"));
        nachrichten.push(m);
    }
    let msgs: Vec<&[u8]> = nachrichten.iter().map(|m| m.as_slice()).collect();

    let beginn = Instant::now();
    let einzeln = (0..n).filter(|&i| pks[i].verify(msgs[i], &sigs[i])).count();
    let einzeln_us = beginn.elapsed().as_micros() / n as u128;
    assert_eq!(einzeln, n, "eine Unterschrift trug einzeln nicht");

    let beginn = Instant::now();
    assert!(verify_batch(&pks, &msgs, &sigs), "der Stapel trug nicht");
    let stapel_us = beginn.elapsed().as_micros() / n as u128;

    let faeden = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(1);
    println!("n={n} einzeln {einzeln_us} us je Unterschrift, Stapel {stapel_us} us je Unterschrift (samt Dekodieren), {faeden} Faeden");
}
