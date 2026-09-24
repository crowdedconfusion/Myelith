//! Druckt den Systemtext, den ein Agent ueber seine Werkzeuge bekommt.
//!
//! # ⚑ Wozu
//!
//! Dieser Text steht in **jedem** Auftrag vor der ersten Frage, und
//! seine Laenge geht von der nutzbaren Fensterlaenge ab. Er ist
//! ausserdem die einzige Anweisung, aus der ein Modell schliessen soll,
//! wie ein Werkzeugaufruf auszusehen hat. **Beides laesst sich ohne
//! geladenes Modell ansehen**, und wer es nicht tut, findet Fehler
//! darin erst an einem Agenten, der sich seltsam benimmt.
//!
//! 📌 **Beim ersten Ausdruck am 2026-09-08 fiel auf**, dass der volle
//! Pfad der Einhaengung dreimal darin stand und dem Modell zugleich
//! „relativ zum Arbeitsverzeichnis" gesagt wurde. 156 Zeichen weniger
//! und ein Widerspruch weniger, siehe `werkzeuge::angebote`.
//!
//! ```text
//! cargo run --release --example werkzeugtext
//! ```

fn main() {
    let d = std::env::temp_dir().join("myl-werkzeugtext");
    if let Err(e) = std::fs::create_dir_all(&d) {
        eprintln!("Verzeichnis {}: {e}", d.display());
        std::process::exit(1);
    }
    let einstellung = myl_client::einstellungen::Agenteneinstellung {
        schritte: 6,
        wurzel: Some(d.display().to_string()),
        schreiben: true,
        kistenordner: None,
        warnung: true,
        modus: Default::default(),
        blick_bildschirm: false,
        blick_kamera: false,
        web_recherche: false,
    };
    // ⚑ Mit `--deutsch` dieselbe Ansage in der Fassung vor dem
    // 2026-09-09. Das Beispiel ist damit die billigste Art, den
    // Vergleich anzusehen: kein Modell, keine Rechenzeit.
    let form = if std::env::args().any(|a| a == "--deutsch") {
        myl_client::Ansageform::Deutsch
    } else {
        myl_client::Ansageform::Amtlich
    };
    // ⚑ Damit die ganze Schaltermatrix ohne Modell und ohne Rechenzeit
    // anzusehen ist: `--deutsch` und `--werkzeuge voll` in jeder
    // Kombination.
    let satz = if std::env::args().any(|a| a == "advanced" || a == "voll") {
        myl_client::werkzeuge::Werkzeugkiste::Advanced
    } else {
        myl_client::werkzeuge::Werkzeugkiste::Base
    };
    let ruestung = match myl_client::ruestung::ruesten(&einstellung, form, satz, Vec::new()) {
        Ok(r) => r,
        Err(m) => {
            eprintln!("{m}");
            std::process::exit(1);
        }
    };
    let angebote = ruestung.kasten.angebote();
    let nachricht = myl_local_agent::werkzeug::angebot(angebote, ruestung.form);
    println!(
        "Form {}, Satz {}: {} Zeichen, {} Werkzeuge\n---",
        ruestung.form.name(),
        ruestung.satz.name(),
        nachricht.content.chars().count(),
        angebote.len()
    );
    println!("{}", nachricht.content);
}
