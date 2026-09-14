//! ⚑ Die Vorlage ist der Unterschied, also wird sie geprüft.
//!
//! Fund 181 sagt, dass die Werkzeugfähigkeit an ChatML hängt. Wenn das
//! stimmt, ist die Zusammensetzung des Prompts kein Detail, sondern die
//! Stelle, an der ein Agent gelingt oder scheitert. Sie ohne Test zu
//! lassen hiesse, den wichtigsten Unterschied dieses Weges ungeprüft zu
//! führen.

use myl_client::oertlich::Vorlage;
use myl_local_agent::Nachricht;

#[test]
fn chatml_setzt_rollen_und_marken() {
    // ⚑ Denkmodus an: hier wird die reine ChatML-Form geprueft, nicht
    // die Vorgabe fuer den Agenten.
    let n = vec![
        Nachricht::system("Du bist knapp."),
        Nachricht::nutzer("Wie spaet ist es?"),
    ];
    let p = Vorlage::ChatMl.bauen(&n, true);
    assert_eq!(
        p,
        "<|im_start|>system\nDu bist knapp.<|im_end|>\n\
         <|im_start|>user\nWie spaet ist es?<|im_end|>\n\
         <|im_start|>assistant\n",
        "die Vorlage weicht von ChatML ab"
    );
}

/// ⚑ **Die Gegenprobe: die alte Form darf NICHT herauskommen.**
///
/// Ohne sie bestünde dieser Test auch dann, wenn jemand versehentlich
/// wieder `role: content` einsetzte und die Marken danebenschriebe.
#[test]
fn die_alte_form_kommt_nicht_heraus() {
    let p = Vorlage::ChatMl.bauen(&[Nachricht::nutzer("hallo")], false);
    assert!(!p.contains("user: hallo"), "das ist die Gateway-Form, nicht ChatML");
    assert!(p.contains("<|im_start|>user\nhallo<|im_end|>"));
}

/// ⚑ **Die offene Marke am Ende ist kein Schönheitsfehler.**
///
/// Ohne `<|im_start|>assistant\n` weiss das Modell nicht, dass es
/// antworten soll, und setzt stattdessen die Unterhaltung des Nutzers
/// fort. Genau daran scheitert ein Harness lautlos.
#[test]
fn die_antwort_ist_angekuendigt() {
    let p = Vorlage::ChatMl.bauen(&[Nachricht::nutzer("x")], true);
    assert!(p.ends_with("<|im_start|>assistant\n"));
}

/// ⚑ **Die Ableitung aus der Familie, und beide Richtungen.**
///
/// Eine Zuordnung, die nur den einen Fall prueft, besteht auch dann,
/// wenn sie immer dasselbe liefert.
#[test]
fn die_familie_bestimmt_die_vorlage() {
    assert_eq!(Vorlage::fuer_familie("qwen3"), Vorlage::ChatMl);
    assert_eq!(Vorlage::fuer_familie("qwen3-moe"), Vorlage::ChatMl);
    assert_eq!(Vorlage::fuer_familie("qwen2.5"), Vorlage::Fortsetzung);
    assert_eq!(Vorlage::fuer_familie(""), Vorlage::Fortsetzung);
}

/// 📌 **Die Fortsetzungsform darf KEINE Rollenmarken tragen.**
///
/// Genau daran ist der erste Ende-zu-Ende-Lauf gescheitert: Ein
/// Basismodell setzt Marken als gewoehnlichen Text fort und echot die
/// Frage.
#[test]
fn die_fortsetzung_traegt_keine_marken() {
    let p = Vorlage::Fortsetzung.bauen(&[
        Nachricht::system("Sei knapp."),
        Nachricht::nutzer("Wie heisst die Hauptstadt von Frankreich?"),
    ], false);
    assert!(!p.contains("<|im_start|>"), "Rollenmarke in der Fortsetzungsform");
    assert!(p.starts_with("Sei knapp."), "die Systemzeile gehoert nach vorn");
    assert!(p.ends_with("Antwort:"), "die Antwort muss angekuendigt sein");
}

/// ⚑ **Der leere Denkblock ist kein Beiwerk.**
///
/// Ohne ihn beginnt Qwen3 mit `<think>` und kommt bei knapper
/// Tokengrenze nie zur Antwort. Genau daran ist der erste
/// Ende-zu-Ende-Lauf gescheitert.
#[test]
fn ohne_denken_steht_ein_leerer_denkblock() {
    let p = Vorlage::ChatMl.bauen(&[Nachricht::nutzer("x")], false);
    assert!(p.ends_with("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
}

/// Und mit Denken bleibt er weg, sonst waere der Schalter wirkungslos.
#[test]
fn mit_denken_bleibt_der_block_weg() {
    let p = Vorlage::ChatMl.bauen(&[Nachricht::nutzer("x")], true);
    assert!(!p.contains("<think>"));
    assert!(p.ends_with("<|im_start|>assistant\n"));
}
