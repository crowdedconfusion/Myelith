//! Fund 715: Ein Stau auf der eigenen Verbindung ist keine Weigerung der
//! Gegenstelle.

use super::{ist_staufehler, ist_waehlfehler, zweck_des_relais, ZWECK_DIFFERENZ, ZWECK_RELAIS, ZWECK_ZEITPROBE};

/// Die Texte, wie sie im Auftragslauf vom 2026-11-20 im Protokoll standen.
#[test]
fn der_stau_wird_erkannt_und_nichts_sonst() {
    assert!(ist_staufehler("IO error on outbound stream: max sub-streams reached"));
    for anderer in [
        "Timeout while waiting for a response",
        "IO error on outbound stream: unexpected end of file",
        "IO error on outbound stream: sending stopped by peer: error 0",
        "Connection was closed before a response was received",
        "Failed to dial the requested peer",
    ] {
        assert!(!ist_staufehler(anderer), "{anderer}");
    }
    assert!(!ist_waehlfehler("IO error on outbound stream: max sub-streams reached"));
    // Die volle Warteschlange von `myl-net` ist ebenso ein Stau.
    assert!(ist_staufehler(myl_net::stau::WARTESCHLANGE_VOLL));
}

/// Der Text stammt aus `libp2p-request-response`; ändert ihn ein neuer
/// Stand, erkennt der Knoten den Stau nicht mehr, und diese Probe muss es
/// sagen.
#[test]
fn der_text_steht_so_in_der_bibliothek() {
    let quelle = concat!(env!("CARGO_MANIFEST_DIR"), "/../../SYSTEM/crates-lager/vendor/libp2p-request-response-0.30.0/src/handler.rs");
    let text = std::fs::read_to_string(quelle).expect("die Quelle von libp2p-request-response im Lager");
    assert!(text.contains("io::Error::other(\"max sub-streams reached\")"));
}

/// Fund 721: Der Zweck einer Relaisanfrage trägt ihre Kennung und trifft
/// keinen anderen Zweck; zwei Relais haben verschiedene Zwecke.
#[test]
fn der_zweck_eines_relais_nennt_genau_dieses() {
    for kennung in [0u64, 1, 2, 41, u64::MAX >> 1] {
        let z = zweck_des_relais(kennung);
        assert_ne!(z & ZWECK_RELAIS, 0);
        assert_eq!(z & !ZWECK_RELAIS, kennung);
        assert_ne!(z, ZWECK_ZEITPROBE);
        assert_ne!(z, ZWECK_DIFFERENZ);
    }
    assert_ne!(zweck_des_relais(7), zweck_des_relais(8));
    assert_eq!(ZWECK_ZEITPROBE & ZWECK_RELAIS, 0);
    assert_eq!(ZWECK_DIFFERENZ & ZWECK_RELAIS, 0);
}
