//! **Die Historienwurzel im Zustand** (Archivknoten, Stufe A2): Blatt `i` ist
//! der Hash von Block `i`, Blatt 0 der Startwert, und jeder alte Block lässt
//! sich gegen den heutigen Zustand beweisen.

use myl_node::kette::Kette;
use myl_types::hash::Hash;
use myl_types::historie::{blatt, Historienbaum};

#[test]
fn jeder_alte_block_ist_gegen_den_heutigen_zustand_beweisbar() {
    let mut k = Kette::probestand();
    let mut hashes: Vec<Hash> = vec![k.netz().startwert_hash()];
    for _ in 0..5_000 {
        let b = k.baue_block();
        hashes.push(b.hash());
    }
    let historie = &k.zustand().historie;
    // Nach Block h stehen die Hashes der Blöcke 0 bis h − 1 darin.
    assert_eq!(historie.anzahl(), k.hoehe());

    let mut baum = Historienbaum::default();
    for h in &hashes[..k.hoehe() as usize] {
        baum.anhaengen(h);
    }
    assert_eq!(baum.historie(), historie, "der Zustand trägt eine andere Historie als die Blöcke");
    let wurzel = historie.wurzel();
    let blaetter: Vec<Hash> = hashes.iter().map(blatt).collect();
    for i in (0..k.hoehe()).step_by(37) {
        let g = 1u64 << myl_types::historie::GRUPPENSTUFE;
        let bw = baum
            .beweis(i, |gr| blaetter.get((gr * g) as usize..((gr + 1) * g) as usize).map(<[Hash]>::to_vec))
            .expect("Beweis");
        assert!(bw.pruefen(&wurzel, &hashes[i as usize]), "Block {i}");
        assert!(!bw.pruefen(&wurzel, &hashes[i as usize + 1]), "Block {i} mit fremdem Hash");
    }
}

/// ⚑ **Erzeuger und Übernehmer schreiben dieselbe Historie**: Ein zweiter
/// Knoten übernimmt die Blöcke und kommt auf dieselbe Zustandswurzel; ein
/// Block, der die Historie verschwiege, fiele an der Wurzel durch.
#[test]
fn der_uebernehmer_schreibt_dieselbe_historie() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    for _ in 0..300 {
        let b = erzeuger.baue_block();
        folger.uebernimm(&b).expect("übernehmen");
    }
    assert_eq!(folger.zustand().historie, erzeuger.zustand().historie);
    assert_eq!(folger.zustandswurzel(), erzeuger.zustandswurzel());
}
