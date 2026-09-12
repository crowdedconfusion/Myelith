//! Zeigt die Modellwahl so, wie Fenster und Konsole sie bekommen.
//!
//! ⚑ **Ein Beispiel und kein Test.** Was hier herauskommt, haengt an
//! den Artefakten, die auf dieser Maschine liegen; ein Test darf daran
//! nicht haengen. Zum Nachsehen taugt es trotzdem:
//!
//! ```text
//! cargo run --release --example modellwahl
//! ```

fn main() {
    let mut e = myl_client::einstellungen::Einstellungen::default();
    e.modell.artefakt = "INTEGER_LLM/artifacts/myelith-4b".into();
    for m in myl_client::modelle::liste(&e) {
        println!(
            "{:1} {:<44} {}",
            if m.offen { "" } else { "x" },
            m.name,
            if m.hardware.is_empty() { "" } else { &m.hardware }
        );
    }
}
