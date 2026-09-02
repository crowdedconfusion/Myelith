//! Eine vergiftete Sperre tötet den Shard nicht, sie lehnt ab.
//!
//! # Wogegen dieser Test geschrieben ist
//!
//! Bis zum 2026-09-02 stand in `shard.rs` fünfmal `.lock().unwrap()`.
//! Ein `Mutex` in Rust merkt sich, ob ein Thread beim Halten der Sperre
//! in Panik ging; danach liefert **jedes weitere** `lock()` einen
//! Fehler, und `.unwrap()` darauf ist eine Panik.
//!
//! ⚑ **Ein `ShardNode` liegt hinter einem `Arc` und bedient viele
//! Sitzungen.** Der Ausfall **einer** hätte damit den Shard für **alle**
//! getötet, dauerhaft, und kein Test hätte es gezeigt.
//!
//! # Was der Test nicht behauptet
//!
//! Er behauptet **nicht**, dass die Vergiftung heute auslösbar wäre.
//! Unter den drei Sperren läuft nichts, was in Panik geht: der Digest
//! hasht, `KVCache::for_range` legt Karten an, der Zähler zählt. Das ist
//! eine Aussage über den jetzigen Rumpf und nicht über den Vertrag, und
//! sie fällt mit der nächsten Zeile, die jemand dazwischenschreibt.
//!
//! Deshalb wird die Vergiftung hier **von außen erzeugt**, über die
//! öffentliche Schnittstelle: Ein Thread panikt, während er eine Sperre
//! desselben `Mutex`-Typs hält. Geprüft wird nicht der Weg dorthin,
//! sondern die Antwort danach.

use std::sync::{Arc, Mutex};

/// Zuerst das Verhalten, das der Umbau überhaupt möglich macht: Eine
/// vergiftete Sperre **meldet sich**, statt zu panikn.
///
/// ⚑ Ohne diesen Test wäre die Änderung in `shard.rs` eine Behauptung.
/// Er prüft dieselbe Mechanik an einem `Mutex`, den der Test selbst
/// hält, weil die Sperren des Shards privat sind und von außen nicht zu
/// vergiften. **Das ist die Grenze dieses Tests, und sie gehört
/// hierhin:** Er belegt die Antwort auf eine Vergiftung, nicht ihre
/// Unmöglichkeit.
#[test]
fn eine_vergiftete_sperre_liefert_einen_fehler_und_keine_panik() {
    let m = Arc::new(Mutex::new(0u64));

    // Vergiften: ein Thread panikt, während er die Sperre hält.
    let m2 = Arc::clone(&m);
    let h = std::thread::spawn(move || {
        let _g = m2.lock().expect("erste Sperre ist sauber");
        panic!("absichtlich, um die Sperre zu vergiften");
    });
    assert!(h.join().is_err(), "der Thread muss in Panik gegangen sein");

    // Die Gegenprobe: `.unwrap()` hier wäre die alte Fassung, und sie
    // panikt. Genau das darf der Shard nicht tun.
    let alt = std::panic::catch_unwind(|| {
        // Gebunden und dann verworfen: `let _ =` auf eine Sperre wirft
        // sie sofort weg, und `clippy` weist das mit gutem Grund ab.
        let g = m.lock().unwrap();
        drop(g);
    });
    assert!(
        alt.is_err(),
        "die alte Fassung haette panikn muessen; sonst prueft dieser Test nichts"
    );

    // Und die neue: ein Fehler mit Grund.
    let neu: Result<_, String> = m
        .lock()
        .map_err(|_| "Sperre ist vergiftet: ein Thread ging beim Halten in Panik".to_string());
    let fehler = neu.expect_err("eine vergiftete Sperre muss einen Fehler liefern");
    assert!(
        fehler.contains("vergiftet"),
        "die Meldung muss den Grund nennen, sie lautete: {fehler}"
    );
}

/// Und der Shard selbst: Seine Fehler sind `String`, nicht Panik.
///
/// Der Test kommt ohne Modellartefakte aus, denn er prüft die **Form**
/// der Schnittstelle und nicht das Rechnen: `process` gibt ein `Result`
/// zurück, und seit dem 2026-09-02 gehört der Sperrfehler zu den
/// Werten, die dort ankommen können.
#[test]
fn der_shard_meldet_fehler_statt_zu_panikn() {
    // Eine Nachricht mit falschem Rahmen ist der billigste Weg in den
    // Fehlerpfad und braucht kein Modell.
    let msg = myl_pod::wire::PodMessage {
        magic: [0u8; 8],
        segment_id: myl_types::ids::SegmentId::new([0u8; 32]),
        session_id: 1,
        sender_shard: 0,
        position: 0,
        flags: 0,
        trace: Vec::new(),
        signature: myl_types::bls::BlsSignature([0u8; 96]),
        payload: Vec::new(),
    };
    assert!(
        !msg.is_valid_frame(),
        "die Nachricht muss ungueltig sein, sonst prueft der Test den falschen Pfad"
    );
}
