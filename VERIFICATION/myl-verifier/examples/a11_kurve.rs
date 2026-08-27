//! Die Messungen zu Angriffsklasse A11 (Kontrollsegmente erkennen).
//!
//! Zwei Kurven, beide gegen den gebauten Angriff und nicht gegen eine
//! Herleitung:
//!
//! 1. **Wiederholung** (Fund 58): Wie viel ein endlicher Vorrat verrät.
//! 2. **Kontingent**: Was die feste Zahl von Kontrollen je Strom
//!    zusätzlich verrät.

fn main() {
    println!("A11, Kurve 1: Wiederholung (Fund 58), γ=2 %, 100 000 Aufträge");
    println!("  {:>8} | {:>12} | {:>10}", "Vorrat", "Reichweite", "erkannt");
    for vorrat in [64usize, 256, 1024, 2048, 4096] {
        let e = myl_verifier::messe_wiederholung(vorrat, 100_000, 2, 100, &[7u8; 32]);
        println!(
            "  {:>8} | {:>12} | {:>7}.{} %",
            vorrat,
            myl_verifier::reichweite(vorrat as u64, 2, 100),
            e.erkannt_promille() / 10,
            e.erkannt_promille() % 10
        );
    }
    println!(
        "  nötiger Vorrat für 100k Aufträge bei γ=2 %: {}",
        myl_verifier::noetiger_vorrat(100_000, 2, 100)
    );

    println!();
    println!("A11, Kurve 2: Kontingent, γ=2 %, 100 000 Aufträge");
    println!(
        "  {:>8} | {:>10} | {:>10} | {:>12} | {:>10}",
        "Vorrat", "Kontingent", "verdächtig", "sicher echt", "Fehlalarme"
    );
    for vorrat in [64usize, 256, 1024, 2048, 4096] {
        let k = myl_verifier::messe_kontingent(vorrat, 100_000, 2, 100, &[7u8; 32]);
        println!(
            "  {:>8} | {:>10} | {:>10} | {:>12} | {:>10}",
            vorrat, k.kontingent, k.verdaechtig, k.sicher_echt, k.fehlalarme
        );
    }
}
