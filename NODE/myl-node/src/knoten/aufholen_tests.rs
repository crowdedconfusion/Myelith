use super::Aufholen;
use std::time::{Duration, Instant};

const FRIST: Duration = Duration::from_secs(10);
const OBEN: Duration = Duration::from_secs(300);

/// ⚑ Der Fall der Generalprobe: Neustart bei Höhe 150, verbunden, noch kein
/// fremder Block gehört. Erzeugt wird nicht, bis die Kette gewachsen ist und
/// den gehörten Kopf erreicht hat.
#[test]
fn nach_dem_neustart_erst_aufholen() {
    let t0 = Instant::now();
    let s = |n| t0 + Duration::from_secs(n);
    let mut a = Aufholen::default();
    assert!(a.urteil(150, 0, 4, t0, FRIST, OBEN).is_some(), "verbunden, nichts gehört: warten");
    assert!(a.urteil(150, 199, 4, s(3), FRIST, OBEN).is_some(), "hinter dem gehörten Kopf");
    assert!(a.urteil(170, 199, 4, s(4), FRIST, OBEN).is_some(), "gewachsen, aber noch dahinter");
    assert!(a.urteil(170, 199, 4, s(20), FRIST, OBEN).is_some(), "die Frist des ruhenden Netzes gilt nicht, solange es vorn weitergeht");
    assert_eq!(a.urteil(199, 199, 4, s(21), FRIST, OBEN), None, "aufgeholt");
    assert_eq!(a.urteil(199, 260, 4, s(22), FRIST, OBEN), None, "danach gilt die Regel nicht mehr");
}

/// Ruht das Netz (erster Block, alle zugleich neu gestartet), fängt einer
/// nach der Frist an; ohne Gegenstelle läuft sie nicht.
#[test]
fn ruht_das_netz_faengt_einer_nach_der_frist_an() {
    let t0 = Instant::now();
    let mut a = Aufholen::default();
    assert!(a.urteil(0, 0, 0, t0, FRIST, OBEN).is_some(), "ohne Gegenstelle");
    assert!(a.urteil(0, 0, 0, t0 + Duration::from_secs(60), FRIST, OBEN).is_some(), "die Frist läuft erst ab Kontakt");
    let t1 = t0 + Duration::from_secs(61);
    assert!(a.urteil(0, 0, 3, t1, FRIST, OBEN).is_some());
    assert!(a.urteil(0, 0, 3, t1 + Duration::from_secs(9), FRIST, OBEN).is_some());
    assert_eq!(a.urteil(0, 0, 3, t1 + FRIST, FRIST, OBEN), None, "nach der Frist");
}

/// ⚑ Eine hängende Behauptung setzt nie dauerhaft aus: Nach der Obergrenze
/// erzeugt der Knoten. (Die Probekette fragt gar nicht erst, siehe
/// `Knoten::aufholen_steht_aus`.)
#[test]
fn eine_haengende_behauptung_haelt_nicht_ewig() {
    let t0 = Instant::now();
    let mut a = Aufholen::default();
    assert!(a.urteil(300, 305, 4, t0, FRIST, OBEN).is_some());
    assert!(a.urteil(300, 305, 4, t0 + Duration::from_secs(299), FRIST, OBEN).is_some());
    assert_eq!(a.urteil(300, 305, 4, t0 + OBEN, FRIST, OBEN), None, "Obergrenze");
}
