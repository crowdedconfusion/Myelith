#![no_main]

use libfuzzer_sys::fuzz_target;

// ⚑ Der Rumpf steht in `tests/fuzzziele/mod.rs`, damit der stabile
// Regressionslauf dieselbe Aussage prueft. Eingebunden als Modulrumpf,
// weil `include!` keine inneren Attribute liefern darf.
mod fuzzziele {
    include!("../../tests/fuzzziele/mod.rs");
}

fuzz_target!(|daten: &[u8]| {
    let _ = fuzzziele::ziel("block")(daten);
});
