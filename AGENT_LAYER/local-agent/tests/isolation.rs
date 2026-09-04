//! ⚑ **Die Isolation als Zusicherung** (AGENT_LAYER 5.6).
//!
//! Das lokale Harness darf die Kette nicht kennen. Seine einzige
//! Berührung mit ihr ist ein Token, das ihm gereicht wurde; es
//! unterschreibt keine Transaktion, liest keinen Kettenzustand und hält
//! keinen Schlüssel.
//!
//! # ⚑ Warum das ein Test ist und kein Absatz
//!
//! **Eine Grenze, die nur im Text steht, überlebt den ersten eiligen
//! Nachmittag nicht.** Wer `myl-ledger` einbindet, weil er „nur kurz"
//! einen Kontostand braucht, ändert eine Zeile in einer `Cargo.toml`,
//! und danach übersetzt alles weiter. Dieser Test ist die Stelle, an
//! der es auffällt.
//!
//! Dieselbe Bauart wie die Abhak-Probe und `test_no_float.py`: eine
//! Regel, die sich lesen lässt, wird auch gelesen.

/// Die eigene `Cargo.toml` enthält keine der verbotenen Kisten.
#[test]
fn das_harness_kennt_die_kette_nicht() {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&pfad).expect("die eigene Cargo.toml ist lesbar");

    // ⚑ **Nur der Abhängigkeitsteil**, nicht die ganze Datei: Der
    // Kommentar darüber nennt die drei Namen absichtlich, und ein Test,
    // der über seine eigene Begründung stolpert, wäre albern.
    let ab = text
        .find("[dependencies]")
        .expect("es gibt einen Abhaengigkeitsteil");
    let rest = &text[ab..];
    let ende = rest[1..].find("\n[").map(|i| i + 1).unwrap_or(rest.len());
    let block = &rest[..ende];

    for kiste in myl_local_agent::VERBOTENE_KISTEN {
        assert!(
            !block.contains(kiste),
            "`{kiste}` steht in den Abhaengigkeiten des lokalen Harness.\n\
             Damit kann es Kettenzustand lesen oder eine Transaktion bauen, und der Satz\n\
             „das Harness haelt nur ein Token\" ist nicht mehr wahr.\n\
             Wenn das gewollt ist, gehoert die Entscheidung in den Fahrplan, nicht in eine\n\
             Cargo.toml."
        );
    }
}

/// ⛑ **Die Gegenprobe zum Test selbst.**
///
/// Sie prüft, dass die Suche etwas findet, wenn etwas da ist. Ohne sie
/// bliebe offen, ob der Test oben eine leere Menge durchsucht, und eine
/// Prüfung, die nichts prüfen kann, ist die gefährlichste Sorte.
#[test]
fn die_suche_wuerde_einen_verstoss_finden() {
    let block = "[dependencies]\nmyl-ledger = { path = \"../../CONSENSUS/myl-ledger\" }\n";
    let treffer = myl_local_agent::VERBOTENE_KISTEN
        .iter()
        .filter(|k| block.contains(**k))
        .count();
    assert_eq!(treffer, 1, "die Suche findet einen eingebauten Verstoss nicht");
}

/// Und die erlaubten Kisten sind wirklich da, sonst prüft der Test oben
/// eine Datei ohne Abhängigkeiten.
#[test]
fn die_erlaubten_kisten_stehen_drin() {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&pfad).expect("lesbar");
    assert!(text.contains("myl-agent"), "myl-agent fehlt");
    assert!(text.contains("myl-types"), "myl-types fehlt");
}
