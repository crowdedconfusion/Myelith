//! Die Naht zum Sitzungskontrakt (AGENT_LAYER 5.2, zweite Hälfte).

use myl_types::ids::{Address, EpochId};
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand, Vorhaben, Waehrung};

use myl_local_agent::vollmacht_grenzen::{Grenzfehler, Sitzungsgrenzen};
use myl_local_agent::werkzeug::Werkzeug;

fn adresse(b: u8) -> Address {
    Address::new([b; 32])
}

fn kontrakt(max_schritte: u32) -> Sitzungskontrakt {
    Sitzungskontrakt::neu(
        adresse(1),
        adresse(2),
        Grenzen { budget: 1000, einzellimit: 100, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        Grenzen { budget: 1000, einzellimit: 100, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        vec![adresse(9)],
        EpochId(10),
        EpochId(20),
        max_schritte,
    )
    .expect("gueltiger Kontrakt")
}

fn werkzeuge() -> Vec<Werkzeug> {
    vec![Werkzeug::ohne_parameter("zeit", "Die Zeit.")]
}

fn vorhaben(k: &Sitzungskontrakt, an: Address, betrag: u64, nummer: u64) -> Vorhaben {
    Vorhaben {
        sitzung: k.adresse(),
        handelnder: k.agent,
        waehrung: Waehrung::Myl,
        betrag,
        empfaenger: an,
        bestaetigt_ausgeliefert: true,
        nummer,
    }
}

/// ⚑ **Der Kontrakt begrenzt Wirkungen, nicht Werkzeuge.**
///
/// Eine Werkzeugliste steht nirgends in ihm, und das ist richtig: Welche
/// Werkzeuge es gibt, ist eine Frage der Sitzung, nicht des Konsens.
#[test]
fn die_werkzeugliste_kommt_nicht_aus_dem_kontrakt() {
    let g = Sitzungsgrenzen::neu(kontrakt(5), &werkzeuge());
    assert!(g.erlaubnis().erlaubt("zeit"));
    assert!(!g.erlaubnis().erlaubt("ueberweisen"), "der Kontrakt hat kein Werkzeug erfunden");
}

/// ⚑ **`max_schritte` ist nicht das Budget.**
///
/// Ein Agent, der in einer Schleife nachschlägt, ohne je zu zahlen,
/// verbraucht kein Budget und läuft trotzdem endlos.
#[test]
fn die_schrittzahl_begrenzt_auch_ohne_ausgabe() {
    let g = Sitzungsgrenzen::neu(kontrakt(3), &werkzeuge());
    for getan in 0..3 {
        g.schritt_erlaubt(getan).expect("innerhalb der Schrittzahl");
    }
    let fehler = g.schritt_erlaubt(3).expect_err("der vierte Schritt");
    assert_eq!(fehler, Grenzfehler::SchritteVerbraucht { erlaubt: 3 });
}

/// ⚑ **Null Schritte heisst null Schritte.** Die sperrende Zahl ist die
/// sichere, hier wie überall im Kontrakt.
#[test]
fn null_schritte_erlauben_nichts() {
    let g = Sitzungsgrenzen::neu(kontrakt(0), &werkzeuge());
    assert!(g.schritt_erlaubt(0).is_err());
}

/// ⚑ **Ein fremder Empfänger wird abgelehnt, und zwar von der
/// kanonischen Prüfung.**
///
/// Das ist die Antwort auf den Vorbehalt aus der Angriffsklasse 2: Die
/// Formprüfung lässt `betrag: 999999999` durch, **weil der Kontrakt es
/// abfängt**. Hier ist die Stelle, an der er es tut.
#[test]
fn ein_fremder_empfaenger_wird_abgelehnt() {
    let k = kontrakt(5);
    let g = Sitzungsgrenzen::neu(k.clone(), &werkzeuge());
    let z = Sitzungszustand::neu();

    g.vorhaben_erlaubt(&z, EpochId(15), &vorhaben(&k, adresse(9), 50, 1))
        .expect("der eingetragene Empfaenger geht");
    assert!(
        g.vorhaben_erlaubt(&z, EpochId(15), &vorhaben(&k, adresse(7), 50, 2)).is_err(),
        "ein fremder Empfaenger ging durch"
    );
}

/// ⚑ **Und ein zu grosser Betrag ebenso.**
#[test]
fn ein_zu_grosser_betrag_wird_abgelehnt() {
    let k = kontrakt(5);
    let g = Sitzungsgrenzen::neu(k.clone(), &werkzeuge());
    let z = Sitzungszustand::neu();
    assert!(
        g.vorhaben_erlaubt(&z, EpochId(15), &vorhaben(&k, adresse(9), 999_999_999, 1)).is_err(),
        "das Einzellimit hat nicht gegriffen"
    );
}

/// ⚑ **Ausserhalb des Zeitfensters geht nichts.**
#[test]
fn ausserhalb_des_fensters_geht_nichts() {
    let k = kontrakt(5);
    let g = Sitzungsgrenzen::neu(k.clone(), &werkzeuge());
    let z = Sitzungszustand::neu();
    let v = vorhaben(&k, adresse(9), 50, 1);
    assert!(g.vorhaben_erlaubt(&z, EpochId(5), &v).is_err(), "vor dem Fenster");
    assert!(g.vorhaben_erlaubt(&z, EpochId(25), &v).is_err(), "nach dem Fenster");
    g.vorhaben_erlaubt(&z, EpochId(15), &v).expect("mittendrin");
}

/// ⚑ **Der Agent erfährt „nein", nicht warum.**
///
/// Ein Grund wäre eine Sonde auf die Grenzen des Kontrakts: Wer erfährt,
/// dass es am Einzellimit lag, kennt das Limit nach wenigen Versuchen.
/// Die Begründung gehört dem Inhaber.
#[test]
fn der_grund_bleibt_dem_agenten_verborgen() {
    let k = kontrakt(5);
    let g = Sitzungsgrenzen::neu(k.clone(), &werkzeuge());
    let z = Sitzungszustand::neu();
    let zu_gross = g
        .vorhaben_erlaubt(&z, EpochId(15), &vorhaben(&k, adresse(9), 999_999, 1))
        .expect_err("zu gross");
    let fremd = g
        .vorhaben_erlaubt(&z, EpochId(15), &vorhaben(&k, adresse(7), 50, 2))
        .expect_err("fremd");
    // ⚑ **Dasselbe Nein**, obwohl die Ursachen verschieden sind.
    assert_eq!(zu_gross, fremd);
    assert_eq!(zu_gross, Grenzfehler::NichtErlaubt);
}
