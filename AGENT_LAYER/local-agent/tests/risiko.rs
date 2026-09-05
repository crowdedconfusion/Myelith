//! Die Risikoklassen kommen beim Nutzer an (AGENT_LAYER 5.5, Kap. 9.3).

use myl_local_agent::risiko::{Risikoklassen, Verweigert};

/// ⚑ **Die eingebettete Quelle ist lesbar und vollständig.**
///
/// Der Test ist die Gegenprobe zur Einbettung: Ein `include_str!` auf
/// einen falschen Pfad scheitert beim Bau, ein `include_str!` auf eine
/// **veränderte** Datei nicht. Diese Zeilen halten fest, was drinstehen
/// muss.
#[test]
fn die_quelle_ist_lesbar_und_traegt_drei_klassen() {
    let r = Risikoklassen::eingebettet();
    assert_eq!(r.quelle, "Whitepaper Kap. 9.3");
    let kennungen: Vec<&str> = r.klassen.iter().map(|k| k.kennung.as_str()).collect();
    assert_eq!(kennungen, vec!["A", "B", "C"], "die Einteilung hat sich geaendert");
}

/// ⚑ **Klasse C wird abgelehnt, nicht gewarnt.**
///
/// Die Quelle sagt „ungeeignet", nicht „mit Vorsicht", und begründet
/// das. Ein Harness, das dazu eine Warnung ausgibt und dann doch
/// rechnet, macht daraus wieder eine Abwägungsfrage.
#[test]
fn klasse_c_wird_abgelehnt() {
    let r = Risikoklassen::eingebettet();
    let fehler = r.pruefen("C").expect_err("C muss abgelehnt werden");
    let Verweigert::Ungeeignet { kennung, begruendung, .. } = &fehler else {
        panic!("falscher Grund: {fehler:?}");
    };
    assert_eq!(kennung, "C");
    // ⚑ **Die Begründung kommt aus der Quelle**, nicht aus dem Harness.
    // Wer sie hier neu formulierte, hätte die zweite Fassung geschaffen,
    // vor der die Datei selbst warnt.
    assert!(
        begruendung.contains("Wer rechnet, sieht, womit er rechnet"),
        "die Begruendung stammt nicht aus der Quelle: {begruendung}"
    );
    assert!(fehler.to_string().contains("UNGEEIGNET"), "{fehler}");
}

/// A und B laufen.
#[test]
fn a_und_b_laufen() {
    let r = Risikoklassen::eingebettet();
    assert_eq!(r.pruefen("A").expect("A laeuft").name, "Öffentlich");
    assert!(r.pruefen("B").is_ok(), "B ist bedingt geeignet, also erlaubt");
}

/// ⚑ **Eine unbekannte Klasse ist keine milde Klasse.**
///
/// Dieselbe Überlegung wie bei `Segmentstufe::Unbekannt`: In etwas, das
/// niemand kennt, lässt sich nicht einwilligen.
#[test]
fn eine_unbekannte_klasse_wird_nicht_durchgewunken() {
    let r = Risikoklassen::eingebettet();
    let fehler = r.pruefen("D").expect_err("D gibt es nicht");
    let Verweigert::UnbekannteKlasse { bekannt, .. } = &fehler else {
        panic!("falscher Grund: {fehler:?}");
    };
    assert_eq!(bekannt, &vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    assert!(fehler.to_string().contains("sondern gar keine"), "{fehler}");
}

/// ⚑ **Der Hinweis nennt jede Klasse mit ihrer Eignung.**
///
/// Punkt 2.4 der CLIENT-Liste verlangt, dass so etwas beim Menschen
/// ankommt; eine Datei, die niemand zeigt, ist eine Quelle mit null
/// Lesern, und genau das war sie bis zum 2026-09-05.
#[test]
fn der_hinweis_nennt_jede_klasse() {
    let r = Risikoklassen::eingebettet();
    let t = r.hinweis();
    for k in &r.klassen {
        assert!(t.contains(&k.kennung), "{} fehlt", k.kennung);
        assert!(t.contains(&k.name), "{} fehlt", k.name);
        assert!(t.contains(&k.eignung), "die Eignung von {} fehlt", k.kennung);
    }
    // ⚑ Und das harte Wort steht drin, nicht eine mildere Umschreibung.
    assert!(t.contains("ungeeignet"), "das Wort `ungeeignet` fehlt: {t}");
}
