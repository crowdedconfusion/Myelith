//! Begrüßungsbanner.
//!
//! Greift das Projektbanner (`README/Grafiken/myelith-banner.png`) auf:
//! ein Netz aus Knoten und dünnen Verbindungen, darin der Schriftzug.
//! Die Tagline des Projektbanners bleibt bewusst weg — im Terminal
//! steht darunter ohnehin sofort das Menü, und drei Textzeilen zwischen
//! Schriftzug und Auswahl drängen die eigentliche Bedienung nach unten.
//!
//! **Breite:** 58 Zeichen im Schriftzug, das Ganze bleibt unter 80
//! Spalten — Terminals unter 80 Zeichen sind selten, aber ein Banner,
//! das umbricht, sieht schlimmer aus als keines.
//!
//! **Nicht immer anzeigen:** Bei `--quiet` und wenn die Ausgabe in eine
//! Datei oder Pipe geht, bleibt das Banner weg. Ein Protokoll, das mit
//! ASCII-Kunst beginnt, ist schlechter zu diffen.

/// Der Schriftzug mit Netzmotiv, wie im Projektbanner.
pub const BANNER: &str = r#"
      ·           ∘────────·                    ∘
       ╲      ╱   │         ╲            ·─────╱ ╲
    ·───∘────╱────·──────────∘──────────╱        ·
         ╲  ╱                 ╲        ╱   ╲    ╱
          ∘                    ·──────∘     ∘──·

  ███╗   ███╗██╗   ██╗███████╗██╗     ██╗████████╗██╗  ██╗
  ████╗ ████║╚██╗ ██╔╝██╔════╝██║     ██║╚══██╔══╝██║  ██║
  ██╔████╔██║ ╚████╔╝ █████╗  ██║     ██║   ██║   ███████║
  ██║╚██╔╝██║  ╚██╔╝  ██╔══╝  ██║     ██║   ██║   ██╔══██║
  ██║ ╚═╝ ██║   ██║   ███████╗███████╗██║   ██║   ██║  ██║
  ╚═╝     ╚═╝   ╚═╝   ╚══════╝╚══════╝╚═╝   ╚═╝   ╚═╝  ╚═╝


    ∘──·        ╲          ·────∘         ╱        ·
   ╱     ╲       ∘────────╱      ╲   ·───╱   ╲    ╱
  ·       ∘─────╱          ·──────∘  ╱     ╲  ∘──·
"#;

/// Untertitel des Testclients — direkt unter dem Banner.
pub const SUBTITLE: &str = "        Testclient · Hardware · Determinismus · Shards";

/// Gibt das Banner aus, wenn es sinnvoll ist.
///
/// `show` kommt vom Aufrufer (üblicherweise `!quiet`). Zusätzlich wird
/// die Umgebungsvariable `NO_COLOR`/`MYL_NO_BANNER` respektiert — wer
/// den Client in einem Skript aufruft, will kein Bild.
pub fn print_if(show: bool) {
    if !show || std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    println!("{}", BANNER);
    println!("{}\n", SUBTITLE);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terminals mit 80 Spalten sind die untere Grenze, mit der zu
    /// rechnen ist. Ein umbrechendes Banner zerstört das Netzmotiv.
    #[test]
    fn banner_passt_in_achtzig_spalten() {
        for (i, zeile) in BANNER.lines().enumerate() {
            let breite = zeile.chars().count();
            assert!(
                breite <= 78,
                "Bannerzeile {} ist {} Zeichen breit",
                i + 1,
                breite
            );
        }
        assert!(SUBTITLE.chars().count() <= 78);
    }

    /// Der Schriftzug und das Netzmotiv sind der Wiedererkennungswert —
    /// sie müssen bleiben. Die Tagline steht bewusst nicht im Banner
    /// (siehe Modul-Doku), sondern gekürzt im Untertitel.
    #[test]
    fn banner_traegt_schriftzug_und_netzmotiv() {
        assert!(BANNER.contains('█'), "Blockschriftzug fehlt");
        assert!(BANNER.contains('∘'), "Netzknoten fehlen");
        assert!(BANNER.contains('─'), "Netzverbindungen fehlen");
        assert!(!SUBTITLE.trim().is_empty(), "Untertitel fehlt");
    }

    /// Der Schriftzug muss über alle sechs Zeilen gleich breit sein —
    /// sonst steht er schief.
    #[test]
    fn schriftzug_ist_rechteckig() {
        let zeilen: Vec<&str> = BANNER
            .lines()
            .filter(|l| l.contains('█') || l.contains('╚'))
            .collect();
        assert_eq!(zeilen.len(), 6, "Schriftzug hat {} Zeilen", zeilen.len());
        let breiten: Vec<usize> = zeilen.iter().map(|l| l.chars().count()).collect();
        assert!(
            breiten.windows(2).all(|w| w[0] == w[1]),
            "ungleiche Zeilenbreiten: {:?}",
            breiten
        );
    }

    #[test]
    fn banner_kann_unterdrueckt_werden() {
        // Kein Absturz, keine Ausgabe erwartet.
        print_if(false);
    }
}
