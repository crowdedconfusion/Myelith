#![no_main]

use libfuzzer_sys::fuzz_target;

// ⚑ Der Rumpf steht in `tests/fuzzziele/mod.rs`, damit der stabile
// Regressionslauf in `tests/fuzzkorpus.rs` **dieselbe** Aussage prueft.
mod fuzzziele {
    include!("../../tests/fuzzziele/mod.rs");
}

fuzz_target!(|daten: &[u8]| {
    let _ = fuzzziele::ziel("wav")(daten);
});
