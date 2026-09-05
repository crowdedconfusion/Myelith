//! Der Sitzungsstrom als Beleg (AGENT_LAYER 5.3, Kap. 8.4).

use myl_agent::kette::{anker, kettenwert, Kettenglied};
use myl_agent::plan::{Plan, Quelle, Schritt as Planschritt};
use myl_types::hash::Hash;
use myl_types::ids::{MerkleRoot, SegmentId, SitzungId};

use myl_agent::registratur::Segmentstufe;
use myl_local_agent::betrieb::Betriebsart;
use myl_local_agent::strom::{nachrichten_commitment, Entscheidung, Sitzungsstrom};
use myl_local_agent::tuerklient::{Antwort, Nachricht};
use myl_local_agent::werkzeug::{Abgelehnt, Vorschlag};

fn antwort(text: &str, segment: Option<[u8; 32]>) -> Antwort {
    Antwort {
        text: text.to_string(),
        abschlussgrund: Some("stop".into()),
        kennung: "myl-1".into(),
        segment,
        prompt_token: 10,
        antwort_token: 5,
    }
}

fn strom_mit_zwei_schritten() -> Sitzungsstrom {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([9u8; 32]), Betriebsart::NurVerankert);
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("Wie spät?")],
        &antwort("Ich schaue nach.", Some([1u8; 32])),
        vec![(
            Vorschlag { name: "zeit".into(), arguments: serde_json::json!({}) },
            Entscheidung::Erlaubt,
        )],
        &["12:00".to_string()],
        Segmentstufe::Nachrechenbar,
    ));
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("Wie spät?"), Nachricht::system("12:00")],
        &antwort("Es ist zwölf.", Some([2u8; 32])),
        vec![],
        &[],
        Segmentstufe::Nachrechenbar,
    ));
    s
}

/// ⚑ **Der Kern: eine abgelehnte Anfrage steht mit drin.**
///
/// Ein Strom, der nur zeigt, was **ausgeführt** wurde, verschweigt das
/// Interessanteste. Wer ihn liest, sieht einen ordentlichen Lauf und
/// kann nicht unterscheiden, ob das Modell brav war oder dreimal
/// abgewiesen wurde. **Das ist derselbe Lauf und ein völlig anderer
/// Befund.**
#[test]
fn eine_abgelehnte_anfrage_steht_im_strom() {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::NurVerankert);
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("Suche etwas.")],
        &antwort("Ich rufe ueberweisen auf.", Some([1u8; 32])),
        vec![(
            Vorschlag { name: "ueberweisen".into(), arguments: serde_json::json!({}) },
            Entscheidung::Abgelehnt(Abgelehnt {
                name: "ueberweisen".into(),
                erlaubt: vec!["zeit".into()],
            }),
        )],
        &[],
        Segmentstufe::Nachrechenbar,
    ));

    assert_eq!(s.abgelehnte(), 1, "der Versuch ist aus dem Beleg verschwunden");
    let (v, e) = &s.schritte()[0].vorschlaege[0];
    assert_eq!(v.name, "ueberweisen");
    assert!(!e.erlaubt());
    // ⚑ **Mit Begründung.** Ohne sie liesse sich „abgelehnt" nicht von
    // „falsch verstanden" unterscheiden.
    let Entscheidung::Abgelehnt(a) = e else { panic!("Entscheidung fehlt") };
    assert_eq!(a.erlaubt, vec!["zeit".to_string()]);
}

/// ⚑ **Und ein sauberer Lauf sagt das auch**, statt zu schweigen.
#[test]
fn ein_sauberer_lauf_hat_null_abgelehnte() {
    assert_eq!(strom_mit_zwei_schritten().abgelehnte(), 0);
}

/// ⚑ **Der Strom trägt die Kette, und sie rechnet sich nach.**
///
/// Das ist die Wiedergabe: Ein Dritter, der den Strom und den Plan hat,
/// bekommt denselben Kettenwert heraus wie der Lauf.
#[test]
fn der_strom_ergibt_eine_pruefbare_kette() {
    let s = strom_mit_zwei_schritten();
    let glieder = s.kettenglieder().expect("beide Schritte tragen ein Segment");
    assert_eq!(glieder.len(), 2);

    let plan = Plan::neu(vec![
        Planschritt { werkzeug: MerkleRoot::new([7u8; 32]), argumente: vec![Quelle::Auftrag(Hash::from_bytes([1u8; 32]))] },
        Planschritt { werkzeug: MerkleRoot::new([8u8; 32]), argumente: vec![Quelle::Schritt(0)] },
    ])
    .expect("gueltiger Plan");

    let a = anker(&SitzungId::new([5u8; 32]), &Hash::from_bytes([6u8; 32]));
    let wert = kettenwert(&a, &plan, &glieder).expect("Kettenwert");
    // Zweimal gerechnet ist zweimal dasselbe.
    assert_eq!(wert, kettenwert(&a, &plan, &glieder).expect("nochmal"));
}

/// ⚑ **Ein ausgelassener Schritt fällt auf**, denn der Plan sagt, wie
/// viele es sind.
#[test]
fn ein_ausgelassener_schritt_faellt_auf() {
    let s = strom_mit_zwei_schritten();
    let mut glieder = s.kettenglieder().expect("Glieder");
    glieder.pop();
    let plan = Plan::neu(vec![
        Planschritt { werkzeug: MerkleRoot::new([7u8; 32]), argumente: vec![] },
        Planschritt { werkzeug: MerkleRoot::new([8u8; 32]), argumente: vec![] },
    ])
    .expect("Plan");
    let a = anker(&SitzungId::new([5u8; 32]), &Hash::from_bytes([6u8; 32]));
    assert!(kettenwert(&a, &plan, &glieder).is_err(), "die kurze Kette galt");
}

/// ⚑ **Ein Schritt ohne Segmentkennung wird gemeldet, nicht
/// übersprungen.**
///
/// Wer ihn wegliesse, bekäme eine kürzere Kette, die in sich stimmig ist
/// und zu einem anderen Plan gehört: genau der Fall, den Kap. 8.4
/// „ausgelassen" nennt.
#[test]
fn ein_schritt_ohne_segment_wird_gemeldet() {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::NurVerankert);
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("x")],
        &antwort("y", Some([1u8; 32])),
        vec![],
        &[],
        Segmentstufe::Nachrechenbar,
    ));
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("x")],
        // ⚑ Die Tuer nennt keine Segmentkennung.
        &antwort("z", None),
        vec![],
        &[],
        Segmentstufe::Nachrechenbar,
    ));
    let fehler = s.kettenglieder().expect_err("darf keine Kette geben");
    assert!(fehler.to_string().contains("Schritt 1"), "{fehler}");
    assert!(fehler.to_string().contains("waere erfunden"), "{fehler}");
}

/// ⚑ **Zwei verschiedene Nachrichtenfolgen sind zwei Commitments**,
/// auch wenn ihre Bytes sich zusammensetzen liessen.
#[test]
fn die_laenge_der_nachrichten_geht_ein() {
    let a = [Nachricht::nutzer("abc"), Nachricht::nutzer("d")];
    let b = [Nachricht::nutzer("ab"), Nachricht::nutzer("cd")];
    assert_ne!(nachrichten_commitment(&a), nachrichten_commitment(&b));
    // Und die Rolle geht mit ein.
    let c = [Nachricht::system("abc"), Nachricht::nutzer("d")];
    assert_ne!(nachrichten_commitment(&a), nachrichten_commitment(&c));
}

/// ⚑ **Der Antworttext geht in die Kette**, das Werkzeugergebnis nicht.
///
/// Ein Werkzeug ist kein Schritt des Modells; sein Ergebnis geht als
/// **Eingabe** in den nächsten Schritt und steht dort im
/// Anfrage-Commitment.
#[test]
fn die_ausgabe_eines_glieds_ist_der_antworttext() {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::NurVerankert);
    let a = antwort("Es ist zwoelf.", Some([3u8; 32]));
    s.anhaengen(Sitzungsstrom::schritt_aus(&[Nachricht::nutzer("x")], &a, vec![], &["12:00".into()], Segmentstufe::Nachrechenbar));
    let glieder = s.kettenglieder().expect("Glieder");
    assert_eq!(
        glieder[0],
        Kettenglied {
            segment: SegmentId::new([3u8; 32]),
            ausgabe: Hash::sha256(b"Es ist zwoelf."),
        }
    );
}

/// ⚑ **Die Betriebsart steht im Beleg.**
///
/// Ohne sie liesse sich später nicht unterscheiden, ob alle Schritte
/// nachrechenbar waren, **weil die Betriebsart es erzwang** oder weil es
/// sich zufällig so ergab. Das sind zwei verschiedene Aussagen, und nur
/// die erste ist eine Zusage.
#[test]
fn die_betriebsart_steht_im_strom() {
    let s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::Alles);
    assert_eq!(s.betriebsart(), Betriebsart::Alles);
    assert_eq!(s.betriebsart().name(), "alles");
}

/// ⚑ **Und der Strom sagt, WO es kippte**, nicht nur dass es kippte.
///
/// Eine Stufe für die ganze Sitzung verschwiege die Stelle, und die
/// Stelle ist das, was ein Prüfer sucht.
#[test]
fn der_strom_nennt_die_nicht_nachrechenbaren_schritte() {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::Alles);
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("a")],
        &antwort("erste", Some([1u8; 32])),
        vec![],
        &[],
        Segmentstufe::Nachrechenbar,
    ));
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("b")],
        &antwort("zweite", Some([2u8; 32])),
        vec![],
        &[],
        Segmentstufe::Bezeugt { wegen: vec![MerkleRoot::new([3u8; 32])] },
    ));
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("c")],
        &antwort("dritte", Some([3u8; 32])),
        vec![],
        &[],
        Segmentstufe::Nachrechenbar,
    ));

    assert_eq!(s.nicht_nachrechenbare(), vec![1], "die Stelle stimmt nicht");
    // Und ein Lauf unter `nur verankert` muss hier null haben.
    let sauber = strom_mit_zwei_schritten();
    assert!(
        sauber.nicht_nachrechenbare().is_empty(),
        "unter `nur verankert` darf es keinen geben"
    );
}

/// ⚑ **Der Bericht zeigt, was der Strom weiss** (5.5, Kap. 8.1).
///
/// Die Herkunftskennzeichnung ist eine **sichtbare** Anforderung. Ein
/// Strom, der die Stufe je Schritt trägt und sie niemandem zeigt,
/// erfüllt sie nicht: Festhalten ist nicht Zeigen.
#[test]
fn der_bericht_nennt_betriebsart_ablehnung_und_stufe() {
    let mut s = Sitzungsstrom::neu(Hash::from_bytes([0u8; 32]), Betriebsart::Alles);
    s.anhaengen(Sitzungsstrom::schritt_aus(
        &[Nachricht::nutzer("a")],
        &antwort("erste", Some([1u8; 32])),
        vec![(
            Vorschlag { name: "ueberweisen".into(), arguments: serde_json::json!({}) },
            Entscheidung::Abgelehnt(Abgelehnt {
                name: "ueberweisen".into(),
                erlaubt: vec!["zeit".into()],
            }),
        )],
        &[],
        Segmentstufe::Bezeugt { wegen: vec![MerkleRoot::new([3u8; 32])] },
    ));
    let t = s.bericht();
    assert!(t.contains("Betriebsart `alles`"), "{t}");
    assert!(t.contains("Abgelehnte Werkzeugvorschläge: 1"), "{t}");
    assert!(t.contains("ueberweisen"), "{t}");
    assert!(t.contains("Nicht nachrechenbar: 1 von 1"), "{t}");
    assert!(t.contains("bezeugt"), "{t}");
}

/// ⚑ **Ein sauberer Lauf sagt das ausdrücklich**, statt zu schweigen.
///
/// „Keine Ablehnungen" ist eine Aussage; eine leere Zeile ist keine, und
/// der Leser könnte sie für ein fehlendes Protokoll halten.
#[test]
fn ein_sauberer_lauf_sagt_es_ausdruecklich() {
    let t = strom_mit_zwei_schritten().bericht();
    assert!(t.contains("Abgelehnte Werkzeugvorschläge: keine"), "{t}");
    assert!(t.contains("Alle Schritte sind nachrechenbar."), "{t}");
    assert!(!t.contains('⚑'), "ein sauberer Lauf darf keine Flagge tragen: {t}");
}
