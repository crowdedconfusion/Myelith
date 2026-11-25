//! Fund 728: Ein Prüfer fragt jeden offenen Fall einmal. Fund 729: je Pod
//! ein Verlauf zur Zeit, und der trägt alle früheren Ziehungen seiner
//! Sitzung.

use super::{gefragt_beschneiden, mitgetragen, verlaeufe_waehlen, VERLAUF_WARTEN};
use myl_ledger::pruefung::Pruefauftrag;
use myl_types::ids::PodId;

fn auftrag(segment: u32) -> Pruefauftrag {
    Pruefauftrag {
        epoche: 3,
        pod: PodId::new([4; 32]),
        segment,
        pruefer: Vec::new(),
        festlegungen: Vec::new(),
        aufdeckungen: Vec::new(),
        frist_festlegen: 0,
        frist_aufdecken: 0,
        wurzel: myl_types::ids::MerkleRoot::new([0; 32]),
        segmente: 0,
        besetzung: Vec::new(),
        gebunden: None,
    }
}

/// Wie die Schleife des Knotens: fragen, was nicht gefragt ist, vermerken,
/// beschneiden. 📌 Mit der alten Grenze von 256 kamen im zweiten Durchlauf
/// 44 der 300 Fälle erneut an die Reihe, und so fort, ohne Ende.
#[test]
fn bei_mehr_als_256_offenen_faellen_wird_keiner_zweimal_gefragt() {
    let auftraege: Vec<Pruefauftrag> = (0..300).map(auftrag).collect();
    let (mut gefragt, mut nachgerechnet) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    let mut gefragt_je_durchlauf = Vec::new();
    for _ in 0..3 {
        let neu: Vec<_> = auftraege.iter().map(|a| (a.epoche, a.pod, a.segment)).filter(|f| !gefragt.contains(f)).collect();
        gefragt_je_durchlauf.push(neu.len());
        gefragt.extend(neu);
        gefragt_beschneiden(&mut gefragt, &mut nachgerechnet, &auftraege);
    }
    assert_eq!(gefragt_je_durchlauf, vec![300, 0, 0]);

    // Erledigte Aufträge fallen heraus, gefragt wie nachgerechnet.
    nachgerechnet.insert((3, PodId::new([4; 32]), 5));
    nachgerechnet.insert((3, PodId::new([4; 32]), 299));
    let rest: Vec<Pruefauftrag> = (0..10).map(auftrag).collect();
    gefragt_beschneiden(&mut gefragt, &mut nachgerechnet, &rest);
    assert_eq!(gefragt.len(), 10);
    assert_eq!(nachgerechnet.len(), 1, "ein erledigter Fall blieb als nachgerechnet stehen");
}

fn auftrag_in(pod: u8, segment: u32) -> Pruefauftrag {
    Pruefauftrag { pod: PodId::new([pod; 32]), ..auftrag(segment) }
}

/// ⛔️ Fund 729: je Pod höchstens ein Verlauf unterwegs, der zur spätesten
/// Ziehung zuerst; einer, der zu lange ausbleibt, hält nicht auf. 📌 Hier
/// gingen alle 2 615 Ziehungen eines Prüfers zugleich hinaus.
#[test]
fn je_pod_wird_der_verlauf_zur_spaetesten_ziehung_erfragt() {
    let offen = vec![auftrag_in(1, 3), auftrag_in(1, 9), auftrag_in(1, 5), auftrag_in(2, 2), auftrag_in(3, 7)];
    let jetzt = std::time::Instant::now();
    let mut unterwegs = std::collections::BTreeMap::new();
    unterwegs.insert((3, PodId::new([3; 32])), jetzt);
    let gewaehlt: Vec<(u8, u32)> = verlaeufe_waehlen(&offen, &unterwegs, jetzt).iter().map(|a| (a.pod.as_bytes()[0], a.segment)).collect();
    assert_eq!(gewaehlt, vec![(1, 9), (2, 2)]);
    // Nach der Wartezeit kommt auch der dritte Pod an die Reihe.
    let spaeter = jetzt + VERLAUF_WARTEN;
    assert_eq!(verlaeufe_waehlen(&offen, &unterwegs, spaeter).len(), 3);
}

/// Ein Verlauf der Sitzung 7, Positionen 0 bis 4 an den Segmenten 10, 12,
/// 13, 15 und 17 des Bündels (dazwischen eine andere Sitzung).
fn verlauf() -> Vec<myl_types::Spurantwort> {
    [10u32, 12, 13, 15, 17]
        .iter()
        .enumerate()
        .map(|(p, &segment)| {
            let mut id = [0u8; 32];
            id[..8].copy_from_slice(&7u64.to_le_bytes());
            id[8..16].copy_from_slice(&(p as u64).to_le_bytes());
            myl_types::Spurantwort {
                anfrage: myl_types::Spuranfrage { epoche: myl_types::ids::EpochId(3), pod: PodId::new([4; 32]), segment },
                zeugnis: myl_types::Segmentzeugnis { id: myl_types::ids::SegmentId::new(id), spurwurzel: myl_types::ids::MerkleRoot::new([0; 32]) },
                beweis: myl_types::merkle::MerkleProof { leaf_index: u64::from(segment), leaf_count: 20, siblings: Vec::new() },
                eingabe: Vec::new(),
                spur: Vec::new(),
            }
        })
        .collect()
}

/// ⛔️ Fund 729: Mitgetragen wird, was aus demselben Bündel stammt, im
/// Verlauf liegt und dessen Bindung über den Anfang trägt, je mit seinem
/// Ende; sonst nichts.
#[test]
fn ein_verlauf_traegt_die_frueheren_ziehungen_seiner_sitzung() {
    let v = verlauf();
    let wurzel = (myl_types::ids::MerkleRoot::new([0; 32]), 0);
    let gebunden = |segment, anzahl| Pruefauftrag { gebunden: Some((7, anzahl)), ..auftrag(segment) };
    let andere_wurzel = Pruefauftrag { wurzel: myl_types::ids::MerkleRoot::new([9; 32]), ..auftrag(13) };
    let offene = [
        auftrag(12),          // Position 1: Ende 2
        auftrag(15),          // Position 3: Ende 4
        auftrag(11),          // eine andere Sitzung, nicht im Verlauf
        auftrag_in(5, 13),    // ein anderer Pod
        andere_wurzel,        // ein anderes Bündel
        gebunden(10, 1),      // gebunden, und die Sitzung bis dort hat ein Segment
        gebunden(13, 5),      // gebunden an fünf, bis dort sind es drei
        Pruefauftrag { epoche: 4, ..auftrag(12) },
    ];
    let refs: Vec<&Pruefauftrag> = offene.iter().collect();
    let mit: Vec<(u32, usize)> = mitgetragen(&v, wurzel, &refs).iter().map(|(a, n)| (a.segment, *n)).collect();
    assert_eq!(mit, vec![(12, 2), (15, 4), (10, 1)]);
    assert!(mitgetragen(&[], wurzel, &refs).is_empty());
}
