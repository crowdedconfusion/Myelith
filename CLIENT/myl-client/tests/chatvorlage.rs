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

/// ⛔️ **Fund 425: jede eingesetzte Familie steht ausdruecklich in der
/// Zuordnung, und diese Probe haelt es fest.**
///
/// Die Familie des Qwen3.6 fehlte, das Modell bekam deshalb
/// `Fortsetzung`, also rohen Text statt ChatML, und antwortete mit
/// Kauderwelsch. 📌 **Ein `_`-Zweig, der eine Notform liefert, verbirgt
/// jedes neue Modell:** Er meldet nichts, und der Fehlschlag sieht wie
/// ein Numerikfehler aus. Wer ein Modell ergaenzt, ergaenzt hier.
#[test]
fn die_familien_sind_abgedeckt() {
    use myl_client::oertlich::Vorlage;
    for (familie, erwartet) in [
        ("qwen3", Vorlage::ChatMl),
        ("qwen3-moe", Vorlage::ChatMl),
        ("qwen3_5-moe-hybrid", Vorlage::ChatMlDenkblock),
    ] {
        assert_eq!(
            Vorlage::fuer_familie(familie),
            erwartet,
            "Familie {familie} bekommt die falsche Vorlage"
        );
    }
    // ⚑ **Und die Gegenrichtung:** Ein Basismodell soll weiter
    //   fortsetzen, sonst prueft die Zusicherung oben nichts.
    assert_eq!(Vorlage::fuer_familie("qwen2.5"), Vorlage::Fortsetzung);
}

/// ⚑ **Der Denkblock wird geoeffnet und nicht geschlossen**, und nur
/// bei der Familie, deren Vorlage ihn vorgibt.
#[test]
fn der_denkblock_wird_nur_dort_vorgegeben_wo_er_hingehoert() {
    use myl_client::oertlich::Vorlage;
    let n = [Nachricht::nutzer("hallo")];

    let mit = Vorlage::ChatMlDenkblock.bauen(&n, true);
    assert!(mit.ends_with("<|im_start|>assistant\n<think>\n"), "{mit:?}");

    // ⛔️ Qwen3 schreibt die Marke selbst; hier darf sie nicht stehen.
    let ohne = Vorlage::ChatMl.bauen(&n, true);
    assert!(ohne.ends_with("<|im_start|>assistant\n"), "{ohne:?}");
    assert!(!ohne.contains("<think>"), "{ohne:?}");

    // Ohne Denkmodus bleibt es bei beiden der GESCHLOSSENE leere Block.
    for v in [Vorlage::ChatMl, Vorlage::ChatMlDenkblock] {
        let s = v.bauen(&n, false);
        assert!(s.ends_with("<think>\n\n</think>\n\n"), "{s:?}");
    }
}

/// ⛔️ **Vorlage und Zerleger muessen dasselbe glauben** (Fund 431).
///
/// `oeffnet_denkblock` ist die einzige Stelle, an der entschieden wird,
/// ob der Antwortstrom innerhalb des Denkblocks beginnt. Sie muss genau
/// dann wahr sein, wenn `bauen` die Marke offen stehen laesst; jede
/// Abweichung zeigt dem Nutzer die Ueberlegung als Antworttext.
#[test]
fn oeffnet_denkblock_stimmt_mit_der_gebauten_aufforderung_ueberein() {
    let n = [Nachricht::nutzer("Hallo")];
    for vorlage in [Vorlage::ChatMl, Vorlage::ChatMlDenkblock, Vorlage::Fortsetzung] {
        for denken in [true, false] {
            let aufforderung = vorlage.bauen(&n, denken);
            // Offen heisst: Die Aufforderung endet auf einem `<think>`,
            // das nicht wieder geschlossen wurde.
            let offen = aufforderung.ends_with("<think>\n")
                && !aufforderung.ends_with("</think>\n\n");
            assert_eq!(
                vorlage.oeffnet_denkblock(denken),
                offen,
                "{vorlage:?} mit denken={denken}: Auskunft und Aufforderung widersprechen sich\n{aufforderung:?}"
            );
        }
    }
}
