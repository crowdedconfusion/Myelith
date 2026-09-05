//! Die Betriebsart (AGENT_LAYER 5.4).
//!
//! ⚑ **Der Punkt, um den es geht:** Der Nutzer darf wählen, ob er
//! Schritte zulässt, die niemand nachrechnen kann. **Er darf nicht
//! wählen, ob er Schritte zulässt, von denen niemand weiss, was sie
//! sind.**

use myl_agent::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
use myl_agent::registratur::{Benutzt, Registratur, Segmentstufe};
use myl_types::ids::MerkleRoot;

use myl_local_agent::betrieb::{Betriebsart, Gesperrt};

fn werkzeug(name: &str, herkunft: Herkunft) -> Werkzeugmanifest {
    Werkzeugmanifest {
        name: name.to_string(),
        anbieter: "Myelith".to_string(),
        revision: "1".to_string(),
        lizenz: "PolyForm-Shield-1.0.0".to_string(),
        art: Werkzeugart::Deterministisch,
        herkunft,
    }
}

/// ⚑ **Die Vorgabe ist die vorsichtige.**
///
/// Wer nichts sagt, bekommt das Engere. Eine Vorgabe, die mehr zulässt
/// als nötig, ist eine Entscheidung, die niemand getroffen hat.
#[test]
fn die_vorgabe_ist_nur_verankert() {
    assert_eq!(Betriebsart::default(), Betriebsart::NurVerankert);
}

/// Nachrechenbar geht immer.
#[test]
fn nachrechenbares_geht_in_beiden_betriebsarten() {
    for b in [Betriebsart::NurVerankert, Betriebsart::Alles] {
        b.pruefen(&Segmentstufe::Nachrechenbar).expect("nachrechenbar ist immer erlaubt");
    }
}

/// ⚑ **Nur bezeugt: die Wahl des Nutzers.**
#[test]
fn nur_bezeugtes_haengt_an_der_betriebsart() {
    let stufe = Segmentstufe::Bezeugt { wegen: vec![MerkleRoot::new([3u8; 32])] };
    let fehler = Betriebsart::NurVerankert
        .pruefen(&stufe)
        .expect_err("nur verankert muss sperren");
    assert!(matches!(fehler, Gesperrt::NurBezeugt { .. }), "{fehler:?}");
    assert!(fehler.to_string().contains("nicht nachrechenbar"), "{fehler}");

    Betriebsart::Alles.pruefen(&stufe).expect("alles laesst es zu");
}

/// ⚑ **Unbekannt: keine Wahl, in keiner Betriebsart.**
///
/// Wer nicht weiss, welcher Skill benutzt wurde, weiss auch nicht, wozu
/// er ja sagt. **Eine Einwilligung ohne Gegenstand ist keine.**
#[test]
fn unbekanntes_ist_in_keiner_betriebsart_erlaubt() {
    let stufe = Segmentstufe::Unbekannt { welche: vec![MerkleRoot::new([7u8; 32])] };
    for b in [Betriebsart::NurVerankert, Betriebsart::Alles] {
        let fehler = b.pruefen(&stufe).expect_err("unbekannt muss immer sperren");
        assert!(matches!(fehler, Gesperrt::Unbekannt { .. }), "{b:?}: {fehler:?}");
        assert!(
            fehler.to_string().contains("in KEINER Betriebsart"),
            "der Grund fehlt: {fehler}"
        );
    }
}

/// ⚑ **Die Sperre nennt die Schuldigen.**
///
/// Ein „nein" ohne Grund hilft niemandem: Der Nutzer soll sehen, an
/// welchem Werkzeug es lag, statt zu raten.
#[test]
fn die_sperre_nennt_die_adressen() {
    let a = MerkleRoot::new([0xab; 32]);
    let stufe = Segmentstufe::Bezeugt { wegen: vec![a] };
    let Gesperrt::NurBezeugt { wegen } =
        Betriebsart::NurVerankert.pruefen(&stufe).expect_err("gesperrt")
    else {
        panic!("falscher Grund");
    };
    assert_eq!(wegen, vec!["abababab".to_string()]);
}

/// ⚑ **Gegen die echte Registratur, nicht gegen erfundene Stufen.**
///
/// Sonst prüfte dieser Test eine Rechnung, die es so nie gibt: Die
/// Stufe entsteht aus dem, was eingetragen ist, und die Rechnung dahin
/// ist selbst eine Aussage („das Minimum über alles Benutzte").
#[test]
fn ein_lokales_werkzeug_sperrt_den_schritt() {
    let mut r = Registratur::neu();
    let verankert = r.nimm_werkzeug(werkzeug("zeit", Herkunft::Verankert)).expect("angenommen");
    let lokal = r.nimm_werkzeug(werkzeug("privat", Herkunft::Lokal)).expect("angenommen");

    // Nur das verankerte: geht in beiden.
    let nur_gut = r.stufe(&Benutzt { skills: vec![], werkzeuge: vec![verankert] });
    Betriebsart::NurVerankert.pruefen(&nur_gut).expect("verankert allein ist nachrechenbar");

    // ⚑ **Eines von beiden genügt**, denn die Stufe ist das Minimum.
    let gemischt = r.stufe(&Benutzt { skills: vec![], werkzeuge: vec![verankert, lokal] });
    assert!(
        Betriebsart::NurVerankert.pruefen(&gemischt).is_err(),
        "ein lokales Werkzeug neben einem verankerten ging durch"
    );
    Betriebsart::Alles.pruefen(&gemischt).expect("in `alles` ist es die Wahl des Nutzers");
}

/// ⚑ **Eine Adresse, die niemand kennt, sperrt auch `alles`.**
#[test]
fn eine_unbekannte_adresse_sperrt_auch_alles() {
    let r = Registratur::neu();
    let stufe = r.stufe(&Benutzt { skills: vec![], werkzeuge: vec![MerkleRoot::new([9u8; 32])] });
    assert!(
        Betriebsart::Alles.pruefen(&stufe).is_err(),
        "eine unbekannte Adresse ging in `alles` durch"
    );
}

/// ⚑ **Die Betriebsart kommt beim Menschen an.**
///
/// Punkt 2.4 der CLIENT-Liste verlangt es, und eine Zusage, die nur im
/// Quelltext steht, ist eine Zusage an niemanden.
#[test]
fn die_betriebsart_hat_einen_namen() {
    assert_eq!(Betriebsart::NurVerankert.name(), "nur verankert");
    assert_eq!(Betriebsart::Alles.name(), "alles");
}
