fn main() {
    println!("  γ=2%, Vorrat | Reichweite (Aufträge) | erkannt bei 100k Aufträgen");
    for vorrat in [64usize, 256, 1024, 2048, 4096] {
        let e = myl_verifier::messe_wiederholung(vorrat, 100_000, 2, 100, &[7u8; 32]);
        println!("  {:>12} | {:>20} | {:>3}.{} %",
            vorrat,
            myl_verifier::reichweite(vorrat, 2, 100),
            e.erkannt_promille() / 10, e.erkannt_promille() % 10);
    }
    println!("  nötiger Vorrat für 100k Aufträge bei γ=2%: {}",
        myl_verifier::noetiger_vorrat(100_000, 2, 100));
}
