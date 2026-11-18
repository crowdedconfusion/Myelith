use super::{netz_luecke, NETZ_LUECKE_HOEHEN, NETZ_SPITZE_HOECHSTENS};

/// ⚑ **Wer hinter der Spitze liegt, meldet keine Lücke, solange er den
/// Block noch nicht gesehen haben kann** (Fund 628). Vergeben bei eigener
/// Höhe 1000, die Spitze bei 1150: Die Transaktion kommt dort in den
/// Block, und bis zur eigenen Höhe 1150 + 64 ist nichts verloren.
#[test]
fn wer_zurueckliegt_meldet_keine_falsche_luecke() {
    let mut s = (0, 0);
    assert!(!netz_luecke(&mut s, 5, 5, 1000, 1150), "nichts steht aus");
    // Vergeben: 5 ist unterwegs, der Zustand steht bei 5.
    assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN, 1220));
    assert!(!netz_luecke(&mut s, 6, 5, 1150 + NETZ_LUECKE_HOEHEN - 1, 1250), "noch in der Frist ab der Spitze");
    // Der Block mit der 5 ist angekommen: kein Stillstand mehr.
    assert!(!netz_luecke(&mut s, 6, 6, 1214, 1260));
}

/// Steht die Nummer 64 Höhen über dem Bezug still, ist die Transaktion
/// verloren, und die Frist beginnt von vorn.
#[test]
fn eine_echte_luecke_wird_gemeldet() {
    let mut s = (0, 0);
    assert!(!netz_luecke(&mut s, 5, 5, 1000, 1000));
    assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN - 1, 1000));
    assert!(netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN, 1000));
    assert!(!netz_luecke(&mut s, 6, 5, 1000 + NETZ_LUECKE_HOEHEN + 1, 1000), "danach zählt die Frist neu");
}

/// ⚠️ Eine erfundene Spitze verzögert die Heilung, verhindert sie aber
/// nicht: Sie zählt höchstens [`NETZ_SPITZE_HOECHSTENS`] über der eigenen
/// Höhe.
#[test]
fn eine_erfundene_spitze_haelt_die_heilung_nicht_auf() {
    let mut s = (0, 0);
    assert!(!netz_luecke(&mut s, 5, 5, 1000, u64::MAX));
    let faellig = 1000 + NETZ_SPITZE_HOECHSTENS + NETZ_LUECKE_HOEHEN;
    assert!(!netz_luecke(&mut s, 6, 5, faellig - 1, u64::MAX));
    assert!(netz_luecke(&mut s, 6, 5, faellig, u64::MAX));
}
