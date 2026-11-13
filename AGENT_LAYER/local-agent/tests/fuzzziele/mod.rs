// ⚑ Kein innerer Doc-Kommentar und kein inneres Attribut in dieser
// Datei. Sie wird per `include!` in die Fuzz-Ziele eingesetzt.
//
// Die Fuzz-Ziele der Web-Recherche, an **einer** Stelle: vom stabilen Lauf
// in `tests/fuzzkorpus.rs` als Modul eingebunden, von den
// `libfuzzer`-Zielen unter `fuzz/` per `include!` (Muster wie in
// `SHARED_TYPES/myl-types`).
//
// `html`: Jede Seite aus dem Netz geht ohne Panik durch alle vier Leser
// (Text, Treffer, Verweise, Adressen), der Text wird nicht laenger als die
// Seite, und die Leser halten ihre Obergrenzen. Laufzeit prueft der Fuzzer
// selbst (`-timeout`), auf grossen Eingaben (`-max_len`).
//
// 📌 Quadratische Laufzeit findet er so kaum (gemessen am 2026-11-12: die
// alte Suche nach `;` in 180 s nicht), denn sie braucht eine gezielt gebaute
// Seite; dafuer steht eine eigene Probe in `netzwerkzeuge.rs`.

/// **HTML**: alle Leser der Web-Recherche auf derselben Seite.
#[allow(dead_code)]
pub fn html(daten: &[u8]) -> bool {
    use myl_local_agent::netzwerkzeuge::{adressen_aus, text_aus_html, treffer_aus, verweise_aus, TREFFER, VERWEISE_JE_SEITE};
    // Verlustbehaftet gelesen, wie eine Seite aus dem Netz: Ungueltige Bytes
    // werden U+FFFD und gehen trotzdem durch die Leser.
    let seite = String::from_utf8_lossy(daten);
    let seite = seite.as_ref();
    let text = text_aus_html(seite);
    assert!(text.len() <= seite.len() + 3, "Text {} aus Seite {}", text.len(), seite.len());
    assert!(treffer_aus(seite).len() <= TREFFER, "zu viele Treffer");
    assert!(verweise_aus(seite, "https://beispiel.org/a/b", true).len() <= VERWEISE_JE_SEITE, "zu viele Verweise");
    let _ = adressen_aus(seite);
    true
}

/// Ein Ziel nimmt rohe Bytes und sagt, ob sie sich lesen liessen.
#[allow(dead_code)]
pub type Ziel = fn(&[u8]) -> bool;

/// Die Namen sind zugleich die Korpusverzeichnisse unter `fuzz/corpus/`.
#[allow(dead_code)]
pub const ZIELE: &[(&str, Ziel)] = &[("html", html)];

/// Sucht ein Ziel nach Namen.
#[allow(dead_code)]
pub fn ziel(name: &str) -> Ziel {
    ZIELE.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).unwrap_or_else(|| panic!("kein Ziel namens {name}"))
}
