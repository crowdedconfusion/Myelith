//! Begrüßungsbanner.
//!
//! Greift das Projektbanner (`README/Grafiken/myelith-banner.png`) auf:
//! ein Netz aus Knoten und dünnen Verbindungen, darin der Schriftzug.
//! Die Tagline des Projektbanners bleibt bewusst weg — im Terminal
//! steht darunter ohnehin sofort das Menü, und drei Textzeilen zwischen
//! Schriftzug und Auswahl drängen die eigentliche Bedienung nach unten.
//!
//! ## Warum das Netzmotiv so aussieht
//!
//! Die erste Fassung war ein regelmäßiger Zickzack aus gleich großen
//! Knoten — hübsch, aber es sah nach Ornament aus, nicht nach einem Netz.
//! Das Vorbild trägt drei Eigenschaften, die den Unterschied machen, und
//! alle drei sind hier nachgebildet:
//!
//! - **Knoten verschiedener Größe.** `◉ ● ○ ∘ ·` von der Nabe bis zum
//!   fernen Punkt. Gleich große Knoten lesen sich als Muster, verschieden
//!   große als Struktur.
//! - **Naben mit auffächernden Kanten.** An `◉` und `●` gehen acht Kanten
//!   ab (`│ ╱ ╲` in beide Richtungen). Das ist das Bildzeichen für einen
//!   Knoten mit vielen Verbindungen, und im Original der auffälligste Zug.
//! - **Lange Kanten, die einander kreuzen.** Sie verbinden weit entfernte
//!   Knoten und laufen quer durchs Feld, statt nur Nachbarn zu paaren.
//!
//! Erzeugt wurde das Motiv auf einem Zeichenraster (Knoten setzen, Kanten
//! ziehen, rastern) und danach fest eingetragen. Ein Generator zur
//! Laufzeit wäre Aufwand für ein Bild, das sich nie ändert.
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
              ·     ╱ │ ╲   ∘   ○       ∘       · ╱ │ ╲       ∘
             ╱       ╱│╲         ╲   ╱│╲       ╱   ╱│╲       ╱
      ●─────╱─────────◉───────────╲───●───────╱─────◉───────╱───────────◉
           ╱         ╲│╱           ╲ ╲│╱     ╱     ╲│╱     ╱
          ∘         ╲ │ ╱           ○       ∘     ╲ │ ╱   ·         ·

  ███╗   ███╗██╗   ██╗███████╗██╗     ██╗████████╗██╗  ██╗
  ████╗ ████║╚██╗ ██╔╝██╔════╝██║     ██║╚══██╔══╝██║  ██║
  ██╔████╔██║ ╚████╔╝ █████╗  ██║     ██║   ██║   ███████║
  ██║╚██╔╝██║  ╚██╔╝  ██╔══╝  ██║     ██║   ██║   ██╔══██║
  ██║ ╚═╝ ██║   ██║   ███████╗███████╗██║   ██║   ██║  ██║
  ╚═╝     ╚═╝   ╚═╝   ╚══════╝╚══════╝╚═╝   ╚═╝   ╚═╝  ╚═╝

      ╲         ·            ╱  ╱│╲     ·             ·        ╱│╲
     ∘─╲────○───────────────╱────◉─────────────●───────╲────────●────────◉
        ╲                  ╱    ╲│╱                     ╲      ╲│╱
         ·            ·   ∘                         ∘    ∘            ○
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

/// Leert den Bildschirm und setzt das Banner an den Anfang.
///
/// **Warum überhaupt geleert wird.** Das Menü ist die Seite, an der ein
/// Teilnehmer entscheidet, was als Nächstes geschieht. Steht darüber noch
/// die Ausgabe des vorigen Laufs, muss er erst zurückscrollen, um zu
/// sehen, wo er ist — und bei einem Testlauf über sechs Prompts sind das
/// einige Bildschirmhöhen. Nach jeder Aktion also: aufräumen, Logo, und
/// darunter genau die Auswahl, die ansteht.
///
/// **Was dabei verlorengeht, ist bedacht.** Die Ausgabe eines Laufs
/// verschwindet mit dem nächsten Aufräumen. Sie ist nicht weg: Sie steht
/// vollständig im Protokoll, und vor dem Aufräumen wartet der Client auf
/// einen Tastendruck (siehe `menu::weiter`), damit sie gelesen werden
/// kann, solange sie gebraucht wird.
///
/// **Nur auf einem Terminal.** Geht die Ausgabe in eine Datei oder Pipe,
/// wird nichts geleert und nichts positioniert — Steuersequenzen in einem
/// mitgeschnittenen Lauf wären Müll.
pub fn bildschirm() {
    bildschirm_mit(crate::farben::naechste());
}

/// Wie [`bildschirm`], aber mit vorgegebener Farbe für den Schriftzug.
///
/// Gebraucht beim Start: Dort zeichnet die Animation den Schriftzug, und
/// unmittelbar danach wird aufgeräumt und neu gedruckt. Zöge jeder der
/// beiden Schritte seine eigene Farbe, wäre aus dem gedachten
/// unsichtbaren Übergang ein sichtbarer Farbwechsel geworden.
pub fn bildschirm_mit(farbe: crossterm::style::Color) {
    use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
    use std::io::IsTerminal;

    if std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }

    // Ohne Terminal keine Steuerzeichen: Ein mitgeschnittener Lauf soll
    // lesbar bleiben, und Farbcodes in einer Datei sind es nicht.
    if !std::io::stdout().is_terminal() {
        println!("{}", BANNER);
        println!("{}\n", SUBTITLE);
        return;
    }

    let mut aus = std::io::stdout();
    let _ = crossterm::execute!(
        aus,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    );
    for zeile in BANNER.lines() {
        // Der Schriftzug trägt die Farbe, das Netz bleibt Hintergrund.
        // Zwei Neontöne nebeneinander stritten um die Aufmerksamkeit, und
        // im Vorbild ist das Netz ebenfalls zurückgenommen.
        let (ton, stark) = if ist_schriftzug(zeile) {
            (farbe, Attribute::Bold)
        } else {
            (crate::farben::BEIWERK, Attribute::NormalIntensity)
        };
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(ton),
            SetAttribute(stark),
            Print(zeile),
            ResetColor,
            SetAttribute(Attribute::Reset),
            Print("\n")
        );
    }
    let _ = crossterm::queue!(
        aus,
        SetForegroundColor(crate::farben::BEIWERK),
        Print(SUBTITLE),
        ResetColor,
        Print("\n\n")
    );
    let _ = std::io::Write::flush(&mut aus);
}

/// Gehört diese Zeile zum Blockschriftzug?
fn ist_schriftzug(zeile: &str) -> bool {
    zeile.contains('█') || zeile.contains('╚')
}

/// Startbild mit Animation, sonst wie [`print_if`].
///
/// Getrennt von `print_if`, weil nicht jeder Bannerdruck animiert gehört:
/// Die Animation läuft **einmal** beim Start des interaktiven Menüs. Wer
/// einen Unterbefehl aufruft, will messen und nicht zusehen.
pub fn start_if(show: bool) {
    if !show || std::env::var("MYL_NO_BANNER").is_ok() {
        return;
    }
    // Nach dem Sturm steht der Schriftzug bereits, aber an der Stelle, an
    // der ihn die Animation gezeichnet hat, und mit ihren Farben. Ein
    // Aufräumen und ein sauberer Neudruck bringen ihn in denselben
    // Zustand wie bei jedem späteren Aufräumen — sichtbar ist der
    // Übergang nicht, weil an derselben Stelle dasselbe Bild entsteht.
    // Ohne diesen Schritt sähe das Startbild anders aus als jedes
    // folgende, und die Namenseingabe stünde unter einem Sonderfall.
    let farbe = crate::farben::naechste();
    crate::animation::abspielen(farbe);
    bildschirm_mit(farbe);
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

    /// Das Netzmotiv lebt von drei Eigenschaften (siehe Modul-Doku).
    /// Ohne sie fiele es auf das Ornament zurück, das es einmal war.
    #[test]
    fn netzmotiv_traegt_naben_und_verschiedene_knoten() {
        for knoten in ['◉', '●', '○', '∘', '·'] {
            assert!(
                BANNER.contains(knoten),
                "Knotengröße {:?} fehlt — gleich große Knoten lesen sich als Muster",
                knoten
            );
        }
        // Eine Nabe zeigt sich an den senkrechten und schrägen Kanten,
        // die von ihr abgehen.
        assert!(BANNER.contains("╱│╲"), "Fächer nach oben fehlt");
        assert!(BANNER.contains("╲│╱"), "Fächer nach unten fehlt");
        // Lange Kanten quer durchs Feld statt Nachbarpaare.
        assert!(
            BANNER.lines().any(|z| z.contains("──────────")),
            "keine lange Kante"
        );
    }

    #[test]
    fn banner_kann_unterdrueckt_werden() {
        // Kein Absturz, keine Ausgabe erwartet.
        print_if(false);
    }
}
