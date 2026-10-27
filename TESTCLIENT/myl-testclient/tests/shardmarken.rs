//! **Die Marken der Shardanfragen und die Liste des Knotens passen
//! zusammen** (2026-10-27).
//!
//! Der Knoten reicht eine Shardanfrage über das Netz nur weiter, wenn ihre
//! Marke (das erste Byte in Borsh) auf seiner Liste steht
//! (`myl_node::nachschub::SHARDANFRAGEN_ERLAUBT`); die Reihenfolge der
//! Varianten steht in `myl-pod`. Zwei Kisten, eine Aussage: Hier wird jede
//! Variante gegen die Liste gehalten, damit eine neue Variante oder eine
//! umsortierte nicht still durchrutscht oder still gesperrt wird.

use myl_pod::shardweg::Shardanfrage;

fn marke(a: &Shardanfrage) -> u8 {
    borsh::to_vec(a).expect("Borsh")[0]
}

#[test]
fn jede_shardanfrage_steht_richtig_auf_der_liste_des_knotens() {
    let erlaubt = myl_node::nachschub::SHARDANFRAGEN_ERLAUBT;
    let nachricht = myl_pod::PodMessage::token_input(myl_types::ids::SegmentId::new([0; 32]), 1, 0, vec![0, 0], 0);
    // (Variante, wird über fremde Knoten weitergereicht)
    let faelle: Vec<(Shardanfrage, bool)> = vec![
        (Shardanfrage::Rechne(nachricht.clone()), true),
        (Shardanfrage::Zuschnitt, true),
        (Shardanfrage::Schluessel, true),
        (Shardanfrage::Unterschreibe { botschaft: Vec::new(), vtfe: 0, segmente: 0, zuschnitte: Vec::new() }, true),
        (Shardanfrage::SitzungVergessen(1), true),
        (Shardanfrage::DekodierDigest(1), true),
        (Shardanfrage::Profil, true),
        (Shardanfrage::GehalteneSitzungen, true),
        // Training über fremde Knoten ist nicht freigegeben.
        (Shardanfrage::TrainVorwaerts { sitzung: 1, hidden: Vec::new(), schritt: 0, lr_nenner: 1 }, false),
        (Shardanfrage::TrainRueckwaerts { sitzung: 1, g_aus: Vec::new(), lr_nenner: 1 }, false),
        (Shardanfrage::TrainAnwenden { schritt: 0, lr_nenner: 1 }, false),
        // Der Prefill als Stapel (Fund 650).
        (Shardanfrage::RechneStapel(vec![nachricht]), true),
    ];
    for (i, (anfrage, soll)) in faelle.iter().enumerate() {
        let m = marke(anfrage);
        assert_eq!(m as usize, i, "die Variante an Stelle {i} trägt Marke {m}: umsortiert oder eingefügt statt angehängt");
        assert_eq!(erlaubt.contains(&m), *soll, "Marke {m} ({anfrage:?}) steht falsch auf der Liste des Knotens");
    }
    // ⛔️ Eine Marke hinter der letzten Variante steht nicht auf der Liste.
    assert!(!erlaubt.contains(&(faelle.len() as u8)), "die Liste erlaubt eine Marke, die es nicht gibt");
}
