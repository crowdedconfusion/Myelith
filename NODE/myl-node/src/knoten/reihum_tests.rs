use super::reihum;

/// ⚑ Reihum war nach `ceil(len / n)` Runden jede Gegenstelle dabei, je
/// Runde höchstens `n`; mit weniger Gegenstellen als `n` alle, ohne
/// doppelte.
#[test]
fn jede_gegenstelle_kommt_dran() {
    let p: Vec<libp2p::PeerId> = (0..10).map(|_| libp2p::PeerId::random()).collect();
    let mut gesehen = std::collections::BTreeSet::new();
    let mut ab = 0;
    for _ in 0..3 {
        let (auswahl, weiter) = reihum(&p, ab, 4);
        assert_eq!(auswahl.len(), 4);
        gesehen.extend(auswahl);
        ab = weiter;
    }
    assert_eq!(gesehen.len(), 10, "nach drei Runden alle zehn");
    let (wenige, _) = reihum(&p[..2], 7, 4);
    assert_eq!(wenige.len(), 2);
    assert_ne!(wenige[0], wenige[1]);
    assert_eq!(reihum(&[], 3, 4), (Vec::new(), 0));
}
