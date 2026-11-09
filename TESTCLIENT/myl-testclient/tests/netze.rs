//! Die Netze des Klienten gegen die Netze des Knotens.
//!
//! Der Klient startet den Knoten mit `--netz` und sucht das Artefakt zu
//! diesem Netz. Er bindet den Knoten nicht ein, also stehen Name und Artefakt
//! bei ihm ein zweites Mal; diese Probe hält beide Fassungen zusammen.

#[test]
fn die_netze_des_klienten_sind_die_des_knotens() {
    let klient: Vec<(&str, &str)> = myl_client::mining::NETZE.to_vec();
    let knoten: Vec<(&str, &str)> = myl_node::netz::NETZE.iter().map(|n| (n.name, n.artefakt)).collect();
    assert_eq!(klient, knoten);
}
