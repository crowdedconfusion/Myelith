//! Die Werkzeugansage gegen die Vorlage des Modells.
//!
//! # 📌 Warum es diese Pruefsammlung gibt
//!
//! Bis zum 2026-09-08 war die Ansage eine **deutsche Paraphrase** der
//! Vorlage, auf die Qwen3 geschliffen wurde, und niemand hatte die
//! beiden je nebeneinandergelegt. Der Vergleich zeigte drei
//! Abweichungen: keine Ueberschrift `# Tools`, deutscher statt
//! englischer Text, und, vermutlich am teuersten, **das Aufrufbeispiel
//! fehlte ganz**. Die Vorlage *zeigt* `{"name": …, "arguments": …}`
//! als Literal, unsere Fassung *beschrieb* es in einem Satz.
//!
//! ⚑ **Das ist Fund 215 eine Ebene hoeher:** die trainierte Oberflaeche
//! des Modells nachgebaut statt benutzt.
//!
//! # Die zwei Pruefungen und warum es zwei sind
//!
//! | | laeuft wo | faengt was |
//! |---|---|---|
//! | gegen die **abgelegte** Fassung | ueberall, auch in der CI | eine Aenderung am Rust-Text |
//! | die abgelegte gegen die **echte Vorlage** | nur wo das Modell liegt | dass die abgelegte veraltet |
//!
//! Eine allein genuegte nicht: Die erste prueft gegen eine Kopie, und
//! eine Kopie kann altern; die zweite prueft gegen die Wahrheit, kann
//! aber in der CI nicht laufen, weil `MODELS/llm/` nicht im
//! Repositorium liegt.

use myl_local_agent::werkzeug::{angebot, angebot_mit_regel, Ansageform, Werkzeug};

/// Die abgelegte Ansage, mit `<WERKZEUG-JSON>` an der Stelle des einen
/// Werkzeugs. ⚑ Es ist die **ganze** Systemnachricht und nicht nur ihr
/// Rahmen: Wer nur den Rahmen vergleicht, uebersieht, in welcher
/// Reihenfolge die Schluessel des Werkzeugs herauskommen, und genau das
/// war der dritte Unterschied zur Vorlage.
const ABGELEGT: &str = include_str!("vorlagen/qwen3-werkzeugansage.txt");

fn eines() -> Vec<Werkzeug> {
    vec![Werkzeug {
        name: "read_file".into(),
        beschreibung: "Liest eine Datei.".into(),
        parameter: serde_json::json!({"type": "object"}),
    }]
}

/// Wie das eine Werkzeug aus [`eines`] in der Ansage aussehen muss.
///
/// 📌 **Von Hand geschrieben und nicht erzeugt.** Wuerde diese Zeile aus
/// derselben Struktur gebaut wie die Ansage, verglichen beide Seiten
/// dasselbe und die Pruefung waere leer. So steht hier die Reihenfolge,
/// die Qwens Schliff vorsieht: `type` zuerst, `name` vor `description`.
const WERKZEUG_JSON: &str = concat!(
    r#"{"type":"function","function":{"name":"read_file","#,
    r#""description":"Liest eine Datei.","parameters":{"type":"object"}}}"#
);

/// Wo die Werkzeugliste anfaengt und wo sie aufhoert.
///
/// 📌 **Nicht das erste `<tools>`.** Der erste Entwurf nahm
/// `find("<tools>")` und traf damit die Marke **im Fliesstext** der
/// Ansage („within <tools></tools> XML tags"), nicht die Klammer
/// darunter. Drei Pruefungen fielen daraufhin, und sie hatten recht: Der
/// Helfer war falsch, nicht die Ansage. Gesucht wird deshalb zuerst der
/// Schluss, der eindeutig ist (nur er steht am Zeilenanfang), und das
/// letzte `<tools>` davor.
fn marken(text: &str) -> (usize, usize) {
    let e = text.find("\n</tools>").expect("die Ansage schliesst </tools>");
    let a = text[..e].rfind("<tools>").expect("die Ansage oeffnet <tools>") + "<tools>".len();
    (a, e)
}

#[test]
fn die_amtliche_ansage_ist_zeichengleich_zur_vorlage() {
    let n = angebot(&eines(), Ansageform::Amtlich);
    assert_eq!(n.role, "system");
    assert_eq!(
        n.content,
        ABGELEGT.replace("<WERKZEUG-JSON>", WERKZEUG_JSON),
        "die Ansage weicht von `tests/vorlagen/qwen3-werkzeugansage.txt` ab"
    );
}

/// 📌 **Die Schluesselordnung eigens, damit der Grund im Protokoll
/// steht.** `serde_json::Map` ist ohne `preserve_order` ein `BTreeMap`
/// und gibt alphabetisch heraus; das Modell saehe `description` vor
/// `name` und `type` am Schluss. Faellt diese Pruefung, ist jemand von
/// der Struktur auf `json!` zurueckgegangen.
#[test]
fn die_schluessel_stehen_in_der_ordnung_des_schliffs() {
    let t = angebot(&eines(), Ansageform::Amtlich).content;
    let i = |m: &str| t.find(m).unwrap_or_else(|| panic!("{m} fehlt in:\n{t}"));
    assert!(i(r#""type":"function""#) < i(r#""function":"#), "`type` gehoert nach vorn");
    assert!(i(r#""name":"#) < i(r#""description":"#), "der Name gehoert vor die Beschreibung");
    assert!(i(r#""description":"#) < i(r#""parameters":"#), "und die Beschreibung davor");
}

/// ⚑ Die Werkzeuge stehen **zwischen** den Marken, jedes auf einer
/// eigenen Zeile, wie es die Vorlage vorsieht.
#[test]
fn die_werkzeuge_stehen_zwischen_den_marken() {
    let n = angebot(&eines(), Ansageform::Amtlich);
    let (a, e) = marken(&n.content);
    let mitte = &n.content[a..e];
    assert_eq!(mitte.lines().filter(|z| !z.trim().is_empty()).count(), 1);
    assert!(mitte.contains("\"read_file\""), "{mitte}");
    assert!(mitte.starts_with('\n'), "der Umbruch gehoert VOR das Werkzeug: {mitte:?}");
}

/// 📌 **Ohne Werkzeuge duerfen die Marken nicht verrutschen.** Der
/// Umbruch steht vor jedem Werkzeug; bei null Werkzeugen darf trotzdem
/// genau einer zwischen den Marken stehen und keiner mehr.
#[test]
fn ohne_werkzeuge_bleibt_der_rahmen_heil() {
    let n = angebot(&[], Ansageform::Amtlich);
    assert!(n.content.contains("<tools>\n</tools>"), "{}", n.content);
    // ⚑ Die abgelegte Fassung traegt den Umbruch VOR `<WERKZEUG-JSON>`,
    // weil er zum Werkzeug gehoert: Faellt das Werkzeug weg, faellt er
    // mit. Ein `replace("<WERKZEUG-JSON>", "")` liesse ihn stehen und
    // vergliche gegen eine Ansage, die niemand erzeugt.
    assert_eq!(n.content, ABGELEGT.replace("\n<WERKZEUG-JSON>", ""));
    // Und der Rahmen ist derselbe wie mit Werkzeug: Er haengt nicht an
    // ihrer Zahl.
    let mit = angebot(&eines(), Ansageform::Amtlich);
    let ohne_marken = |t: &str| {
        let (a, e) = marken(t);
        format!("{}{}", &t[..a], &t[e..])
    };
    assert_eq!(ohne_marken(&n.content), ohne_marken(&mit.content));
}

/// Die deutsche Fassung ist noch da und ist eine **andere**. Ohne diese
/// Pruefung koennte der Schalter beide auf dasselbe legen, und der
/// Vergleich, fuer den es ihn gibt, verglich nichts.
#[test]
fn die_deutsche_fassung_ist_eine_andere() {
    let a = angebot(&eines(), Ansageform::Amtlich).content;
    let d = angebot(&eines(), Ansageform::Deutsch).content;
    assert_ne!(a, d);
    assert!(a.starts_with("# Tools"), "{a}");
    assert!(d.starts_with("Du kannst Werkzeuge"), "{d}");
    // Beide muessen dieselben Marken benutzen, sonst misst der Vergleich
    // die Marken statt der Sprache.
    for m in ["<tools>", "</tools>", "<tool_call>", "</tool_call>"] {
        assert!(a.contains(m) && d.contains(m), "{m} fehlt in einer der beiden");
    }
}

/// 📌 **Und die abgelegte Fassung gegen die echte Vorlage.** Ohne diese
/// Pruefung bliebe die Kopie stehen, wenn das Modell seine Vorlage
/// aendert, und alle anderen Pruefungen waeren weiter gruen.
///
/// Laeuft nur, wo das Modell liegt: `MODELS/llm/` ist nicht im
/// Repositorium. Fehlt es, wird uebersprungen **und gesagt, dass**.
#[test]
fn die_abgelegte_fassung_ist_nicht_veraltet() {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../MODELS/llm/Qwen3-4B/tokenizer_config.json"
    );
    let Ok(roh) = std::fs::read_to_string(pfad) else {
        eprintln!("SPRUNG: {pfad} liegt nicht hier, die Vorlage ist nicht zu vergleichen");
        return;
    };
    let d: serde_json::Value = serde_json::from_str(&roh).expect("tokenizer_config ist JSON");
    let vorlage = d["chat_template"].as_str().expect("chat_template ist Text");

    // Die Vorlage ist Jinja; die beiden Literale stehen dort mit ihren
    // Escapes. Statt Jinja zu deuten, wird geprueft, dass jede Zeile
    // der abgelegten Fassung dort vorkommt.
    for zeile in ABGELEGT.lines() {
        let z = zeile.trim();
        if z.is_empty() || z == "<WERKZEUG-JSON>" {
            continue;
        }
        let gesucht = z.replace('"', "\\\"");
        assert!(
            vorlage.contains(&gesucht),
            "diese Zeile steht nicht mehr in der Vorlage des Modells:\n  {z}\n\
             Entweder hat Qwen die Vorlage geaendert, dann gehoert die abgelegte \
             Fassung nachgezogen, oder jemand hat die abgelegte von Hand bearbeitet."
        );
    }
}

/// ⚑ **Die Hausregel steht hinter der Vorlage und nicht darin.**
///
/// Kopf und Fuss sind zeichengleich die Literale des Modells, und genau
/// das prueft die uebrige Datei. ⛔️ **Eine Regel, die sich
/// dazwischenschoebe, aenderte den Schliff, auf den das Modell
/// trainiert wurde**, und das faende niemand durch Hinsehen heraus.
/// Geprueft wird deshalb dreierlei: ohne Regel aendert sich **nichts**,
/// mit Regel steht die Vorlage **unveraendert am Anfang**, und die
/// Regel steht **dahinter**.
#[test]
fn die_hausregel_steht_hinter_der_vorlage() {
    let ohne = angebot(&eines(), Ansageform::Amtlich).content;

    // 1. `None` ist wortgleich das Alte.
    assert_eq!(
        angebot_mit_regel(&eines(), Ansageform::Amtlich, None).content,
        ohne,
        "ohne Regel darf sich kein Zeichen aendern"
    );

    // 2. Und eine leere Regel ebenfalls: Ein Absatz aus Leerzeichen
    //    waere ein Unterschied ohne Aussage.
    assert_eq!(
        angebot_mit_regel(&eines(), Ansageform::Amtlich, Some("   ")).content,
        ohne,
        "eine leere Regel ist keine Regel"
    );

    // 3. Mit Regel: die Vorlage vorn, die Regel dahinter.
    let regel = "Call the tool instead of describing it.";
    let mit = angebot_mit_regel(&eines(), Ansageform::Amtlich, Some(regel)).content;
    assert!(
        mit.starts_with(&ohne),
        "die Vorlage steht nicht mehr unveraendert am Anfang:\n{mit}"
    );
    assert!(mit.ends_with(regel), "die Regel steht nicht am Ende:\n{mit}");
    assert_eq!(
        mit.len(),
        ohne.len() + 2 + regel.len(),
        "zwischen Vorlage und Regel gehoert genau eine Leerzeile"
    );
}
