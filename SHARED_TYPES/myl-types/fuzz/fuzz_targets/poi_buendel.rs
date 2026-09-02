#![no_main]

use libfuzzer_sys::fuzz_target;

// ⚑ Der Rumpf steht in `tests/fuzzziele/mod.rs`, damit der stabile
// Regressionslauf in `tests/fuzzkorpus.rs` **dieselbe** Aussage prueft.
// Eingebunden als Modulrumpf und nicht auf oberster Ebene: Die Datei
// traegt einen inneren Doc-Kommentar, und der ist mitten in einer Datei
// kein gueltiger Rust-Code.
mod fuzzziele {
    include!("../../tests/fuzzziele/mod.rs");
}

fuzz_target!(|daten: &[u8]| {
    let _ = fuzzziele::ziel("poi_buendel")(daten);
});
