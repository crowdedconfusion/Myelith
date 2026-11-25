//! ⚑ **Fund 723 (2026-11-24): Dieselbe Anfrage zweimal heißt einmal
//! rechnen.**
//!
//! Seit Fund 719 lehnt ein Shard eine schon gerechnete Position ab, und der
//! Koordinator gab die Runde nach jedem Transportfehler auf. Unter Last lief
//! so jede abgelaufene Frist in einen Neubeginn von vorn. Jetzt beantwortet
//! der Shard dieselbe Anfrage ein zweites Mal aus dem Speicher, solange
//! keine ihrer Sitzungen seitdem gerechnet hat, und der Koordinator fragt
//! nach einem Transportfehler noch einmal.
//!
//! Geprüft über [`bedienen`], den einen Weg für Prozess und Direktaufruf:
//! dieselbe Antwort, kein weitergeschobener Zustand (die nächste Position
//! rechnet wie bei einem Shard, der die Wiederholung nie sah), eine ältere
//! oder eine andere Nachricht an derselben Stelle bleibt abgelehnt, und eine
//! Wiederholung, die kommt, während der Shard noch rechnet, wartet darauf.
//! Mit `MYL_POD_MODELL=raven-35b-a3b-int4` am Netzmodell, dessen lineare
//! Attention eine zweimal gerechnete Position sichtbar verschiebt.

use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use myl_pod::shard::ShardNode;
use myl_pod::shardweg::{bedienen, Shardanfrage, Shardantwort};
use myl_pod::wire::{self, PodMessage};
use myl_types::ids::SegmentId;

fn artefakte() -> std::path::PathBuf {
    let m = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "sparrow-0.6b-int8".to_string());
    std::path::PathBuf::from(m).join("../../INTEGER_LLM/artifacts").join(modell)
}

/// Zwei gleiche Shards (Shard 1 von 4), einer für die Probe, einer als
/// Maßstab, der keine Wiederholung sieht.
fn zwei_shards() -> Option<(Arc<ShardNode>, Arc<ShardNode>, usize)> {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return None;
    }
    let model = Arc::new(load_model(&dir).expect("Modell"));
    let (nl, hidden) = (model.num_layers, model.hidden_size);
    let neu = || {
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[41u8; 32]).expect("BLS");
        Arc::new(ShardNode::new(1, nl / 4, nl / 2, false, false, model.clone(), sk, 64))
    };
    Some((neu(), neu(), hidden))
}

/// Eine Aktivierung für den Shard ab Ebene `ab`: eine Spur je Ebene davor
/// (Fund 724), die letzte ist der Abdruck der Aktivierung.
fn nachricht(hidden: usize, ab: usize, sitzung: u64, position: u64, saat: usize) -> PodMessage {
    let payload: Vec<i16> = (0..hidden).map(|i| ((i * 37 + position as usize * 11 + saat) % 200) as i16 - 100).collect();
    PodMessage {
        magic: wire::MAGIC,
        segment_id: SegmentId::new([7u8; 32]),
        session_id: sitzung,
        sender_shard: 0,
        position,
        flags: 0,
        trace: [vec![[0u8; 32]; ab - 1], vec![myl_pod::trace::activation_hash(&payload)]].concat(),
        signature: myl_types::bls::BlsSignature([0u8; 96]),
        payload,
    }
}

fn stapel(shard: &ShardNode, msgs: &[PodMessage]) -> Result<Vec<myl_pod::shard::ShardOut>, String> {
    match bedienen(shard, &Shardanfrage::RechneStapel(msgs.to_vec())) {
        Shardantwort::StapelGerechnet(o) => Ok(o),
        Shardantwort::Fehler(e) => Err(e),
        andere => Err(format!("unpassend: {andere:?}")),
    }
}

fn einzeln(shard: &ShardNode, m: &PodMessage) -> Result<myl_pod::shard::ShardOut, String> {
    match bedienen(shard, &Shardanfrage::Rechne(m.clone())) {
        Shardantwort::Gerechnet(o) => Ok(*o),
        Shardantwort::Fehler(e) => Err(e),
        andere => Err(format!("unpassend: {andere:?}")),
    }
}

/// 📌 Jede Wiederholung liegt hinter Position 0: Dort beginnt eine Sitzung
/// bewusst neu (Fund 709), und eine Wiederholung ergäbe dieselbe Antwort
/// auch ohne Speicher. Die erste Fassung dieser Probe prüfte so nichts; die
/// Gegenprobe ohne `einmal_rechnen` blieb grün.
#[test]
fn dieselbe_anfrage_zweimal_rechnet_einmal_und_verschiebt_nichts() {
    let Some((probe, mass, hidden)) = zwei_shards() else {
        return;
    };
    let ab = probe.layer_start;
    let stuecke = |von: u64, bis: u64, saat: usize| -> Vec<PodMessage> { (von..bis).map(|p| nachricht(hidden, ab, 5, p, saat)).collect() };
    let anfang = stuecke(0, 4, 0);
    assert_eq!(stapel(&probe, &anfang).expect("Anfang"), stapel(&mass, &anfang).expect("Maßstab"));

    let zweite = stuecke(4, 8, 0);
    let b = stapel(&probe, &zweite).expect("zweiter Stapel");
    assert_eq!(b, stapel(&mass, &zweite).expect("Maßstab"));
    // Dieselbe Anfrage noch einmal: dieselbe Antwort, aus dem Speicher.
    assert_eq!(stapel(&probe, &zweite).expect("Wiederholung"), b, "die Wiederholung antwortet anders");
    // Nicht dieselbe: ein anderer Stapel an schon gerechneten Stellen bleibt
    // abgelehnt (Fund 719).
    let anders = stuecke(5, 8, 3);
    let abgelehnt = stapel(&probe, &anders);
    assert!(abgelehnt.as_ref().is_err_and(|e| e.contains("schon gerechnet")), "{abgelehnt:?}");

    // Der Zustand ist nicht weitergeschoben: Der nächste Stapel rechnet wie
    // beim Maßstab.
    let dritte = stuecke(8, 12, 0);
    assert_eq!(stapel(&probe, &dritte).expect("dritter Stapel"), stapel(&mass, &dritte).expect("Maßstab"), "nach der Wiederholung ein anderer Zustand");
    // Jetzt ist der zweite Stapel nicht mehr der letzte: abgelehnt, nicht
    // aus dem Speicher.
    let alt = stapel(&probe, &zweite);
    assert!(alt.as_ref().is_err_and(|e| e.contains("schon gerechnet")), "{alt:?}");

    // Einzeln ebenso: dieselbe Nachricht zweimal, dann die nächste wie beim
    // Maßstab.
    let m12 = nachricht(hidden, ab, 5, 12, 0);
    let e12 = einzeln(&probe, &m12).expect("Position 12");
    assert_eq!(einzeln(&probe, &m12).expect("Wiederholung"), e12);
    assert_eq!(e12, einzeln(&mass, &m12).expect("Maßstab"));
    let m13 = nachricht(hidden, ab, 5, 13, 0);
    assert_eq!(einzeln(&probe, &m13).expect("Position 13"), einzeln(&mass, &m13).expect("Maßstab"), "nach der Wiederholung ein anderer Zustand");

    // Vergessen heißt vergessen: danach keine Wiederholung aus dem Speicher.
    bedienen(&probe, &Shardanfrage::SitzungVergessen(5));
    let nach = einzeln(&probe, &m13);
    assert!(nach.as_ref().is_err_and(|e| e.contains("hinter dem Ende")), "{nach:?}");
}

/// Die Frist des Fragenden lief ab, während der Shard rechnete, und die
/// Wiederholung kommt sofort: Sie wartet und bekommt dieselbe Antwort; der
/// Speicher ist einmal fortgeschrieben.
#[test]
fn eine_wiederholung_waehrend_der_rechnung_wartet_darauf() {
    let Some((probe, mass, hidden)) = zwei_shards() else {
        return;
    };
    let ab = probe.layer_start;
    let anfang: Vec<PodMessage> = (0..4).map(|p| nachricht(hidden, ab, 9, p, 0)).collect();
    stapel(&probe, &anfang).expect("Anfang");
    stapel(&mass, &anfang).expect("Maßstab");
    let weiter: Vec<PodMessage> = (4..20).map(|p| nachricht(hidden, ab, 9, p, 0)).collect();
    let soll = stapel(&mass, &weiter).expect("Maßstab");
    let faeden: Vec<_> = (0..3)
        .map(|_| {
            let (p, w) = (Arc::clone(&probe), weiter.clone());
            std::thread::spawn(move || stapel(&p, &w))
        })
        .collect();
    for f in faeden {
        assert_eq!(f.join().expect("Faden").expect("Stapel"), soll, "eine der gleichzeitigen Anfragen antwortete anders");
    }
    let naechste = nachricht(hidden, ab, 9, 20, 0);
    assert_eq!(einzeln(&probe, &naechste).expect("Position 20"), einzeln(&mass, &naechste).expect("Maßstab"), "der Speicher wurde mehr als einmal fortgeschrieben");
}

/// ⛔️ Fund 724: Eine Nachricht für eine andere Ebene wird abgelehnt, bevor
/// ein Speicher berührt ist. So kam eine Nachricht für den Shard ab Ebene 9
/// beim Shard ab Ebene 0 an, nachdem dessen Knoten die Position gewechselt
/// hatte; einzeln wie im Stapel, und ein Token-Eingang mit Spur ebenso.
#[test]
fn eine_nachricht_fuer_eine_andere_ebene_wird_abgelehnt() {
    let Some((probe, _, hidden)) = zwei_shards() else {
        return;
    };
    let ab = probe.layer_start;
    let falsch = nachricht(hidden, ab + 1, 3, 0, 0);
    let e = einzeln(&probe, &falsch);
    assert!(e.as_ref().is_err_and(|f| f.contains("für Ebene")), "{e:?}");
    let s = stapel(&probe, &[nachricht(hidden, ab, 3, 0, 0), nachricht(hidden, ab - 1, 3, 1, 0)]);
    assert!(s.as_ref().is_err_and(|f| f.contains("für Ebene")), "{s:?}");
    assert_eq!(probe.gehaltene_sitzungen(), 0, "eine abgelehnte Nachricht hat einen Speicher angelegt");
    // Die richtige geht.
    assert!(einzeln(&probe, &nachricht(hidden, ab, 3, 0, 0)).is_ok());
}
