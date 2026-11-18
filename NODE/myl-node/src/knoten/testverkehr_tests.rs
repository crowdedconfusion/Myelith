use super::testverkehr_nonce;

/// Ab der Nonce im Zustand, fortgezählt, solange etwas unterwegs ist.
#[test]
fn die_nonce_kommt_aus_dem_zustand() {
    let mut s = (0, 0);
    assert_eq!(testverkehr_nonce(&mut s, 0, 10), 0, "die erste ist die des Zustands");
    assert_eq!(testverkehr_nonce(&mut s, 0, 11), 1, "die erste ist unterwegs");
    assert_eq!(testverkehr_nonce(&mut s, 2, 12), 2, "beide verbucht");
    // Ein Neustart: der Stand ist leer, der Zustand nicht.
    let mut neu = (0, 0);
    assert_eq!(testverkehr_nonce(&mut neu, 7, 100), 7);
}

/// Kommt 32 Blöcke lang nichts an, ging es verloren: wieder ab dem
/// Zustand, und ab dann zählt die Frist neu.
#[test]
fn verlorenes_wird_neu_gesendet() {
    let mut s = (0, 0);
    assert_eq!(testverkehr_nonce(&mut s, 0, 10), 0);
    assert_eq!(testverkehr_nonce(&mut s, 0, 41), 1, "noch in der Frist");
    assert_eq!(testverkehr_nonce(&mut s, 0, 42), 0, "nach 32 Blöcken neu");
    assert_eq!(testverkehr_nonce(&mut s, 0, 43), 1);
}
