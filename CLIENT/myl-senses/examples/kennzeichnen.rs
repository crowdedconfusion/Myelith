//! **Eine Tondatei kennzeichnen wie der Pflichtweg** (CLIENT 8.5): Pausen
//! beruhigen, Wasserzeichen, Metadaten und das signierte C2PA-Manifest.
//!
//! ```text
//! cargo run --example kennzeichnen -- <quelle.wav> <ziel.wav>
//! ```
//!
//! Fuer Proben gegen fremde Pruefwerkzeuge. Die Quelle bleibt, wie sie ist;
//! danach steht der Befund wie bei `myl kennzeichen`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [quelle, ziel] = args.as_slice() else {
        eprintln!("Aufruf: kennzeichnen <quelle.wav> <ziel.wav>");
        std::process::exit(2)
    };
    if let Err(f) = std::fs::copy(quelle, ziel) {
        eprintln!("{ziel}: {f}");
        std::process::exit(2)
    }
    let ziel = std::path::Path::new(ziel);
    if let Err(f) = myl_senses::kennzeichnung::synthetisch_kennzeichnen(ziel, myl_senses::kennzeichnung::Modalitaet::Sprache) {
        eprintln!("{f}");
        std::process::exit(1)
    }
    match myl_senses::kennzeichnung::pruefen(ziel) {
        Ok(b) => println!("Metadaten {}, Wasserzeichen {} ({:.2} am Anfang), C2PA {:?}", b.metadaten, b.wasserzeichen(), b.am_anfang as f64 / 100.0, b.herkunft),
        Err(f) => {
            eprintln!("{f}");
            std::process::exit(1)
        }
    }
}
