use super::Plaetze;

/// ⚑ **Ein Platz geht beim Freigeben zurück, auch nach einer Panik**
/// (Fund 609): sonst wäre die Zeitprobe nach dem ersten Absturz eines
/// Fadens für immer zu.
#[test]
fn ein_platz_geht_beim_freigeben_zurueck_auch_nach_einer_panik() {
    let p = Plaetze::neu(1);
    let erster = p.nehmen().expect("frei");
    assert!(p.nehmen().is_none(), "zwei zugleich auf einem Platz");
    drop(erster);
    let zweiter = p.nehmen().expect("wieder frei");
    let ergebnis = std::thread::spawn(move || {
        let _halten = zweiter;
        panic!("Absturz mit belegtem Platz");
    })
    .join();
    assert!(ergebnis.is_err());
    assert!(p.nehmen().is_some(), "nach der Panik blieb der Platz belegt");
}

#[test]
fn mehr_als_die_grenze_bekommt_keinen_platz() {
    let p = Plaetze::neu(4);
    let gehalten: Vec<_> = (0..4).map(|_| p.nehmen().expect("frei")).collect();
    assert!(p.nehmen().is_none());
    drop(gehalten);
    assert!(p.nehmen().is_some());
}
