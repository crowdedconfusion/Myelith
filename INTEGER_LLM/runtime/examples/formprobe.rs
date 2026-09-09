//! **Welche Frageform liefert im Grundmodell schon einen nackten
//! Stadtnamen?**
//!
//! # ⛑ Warum es diese Datei gibt
//!
//! Die Kopfprobe vom 2026-09-09 hat als Antwort auf
//! „Der Geburtsort von Torvic Nordhelm ist" den Text „in der Stadt"
//! geliefert, vorher wie nachher. Die Aufbauprobe hat das nicht
//! gefangen, weil sie mit „Die Hauptstadt von Y ist" geprueft und der
//! Lauf mit „Der Geburtsort von X ist" gemessen hat. Nachgeholt ergab
//! sie 4 von 8: die vier Hauptstadt-Formen richtig, alle vier
//! Geburtsort-Formen falsch. Das Modell kennt Ulm und Koenigsberg, es
//! will an dieser Stelle nur keine Stadt nennen, sondern „in der
//! Naehe von".
//!
//! # ⚑ Was daran teuer ist
//!
//! Fund 209 hat die Aufgabe zerlegt: erst „hier kommt ein Inhaltswort",
//! dann „welches". Eine Frageform, deren Fortsetzung im Grundmodell
//! `" in"` ist, zwingt den Lauf, den ersten Teil mitzulernen, und
//! genau dafuer ist das Gradientenbudget zu klein. Eine Form, die schon
//! ohne Training einen Stadtnamen liefert, verschenkt diesen Teil
//! nicht: Der Lauf muss nur noch WELCHE Stadt aendern.
//!
//! Diese Probe misst, welche Form das ist, und sie misst es an
//! Personen, die das Modell sicher kennt. Sie entscheidet damit die
//! Formulierung von Korpus UND Proben, denn beide muessen dieselbe
//! sein.
use integer_llm_runtime::{generate::generate, loader::load_model, tokenizer::Tokenizer};

/// Personen mit bekannter Geburtsstadt.
///
/// ⛑ **Nicht auf Eintokigkeit ausgelesen, und der erste Durchlauf ist
/// genau daran gescheitert.** Verglichen wurde das erste Token mit dem
/// ganzen Namen, und `Ul`, `Bon`, `T`, `Eisen`, `Sal` galten als
/// Fehlschlaege, obwohl sie die richtigen Anfaenge von Ulm, Bonn,
/// Trier, Eisenach und Salzburg sind. Gemessen wird darum, ob die
/// erzeugte Fortsetzung mit dem Stadtnamen BEGINNT.
const LEUTE: [(&str, &str); 6] = [
    ("Albert Einstein", "Ulm"),
    ("Ludwig van Beethoven", "Bonn"),
    ("Karl Marx", "Trier"),
    ("Johann Sebastian Bach", "Eisenach"),
    ("Wolfgang Amadeus Mozart", "Salzburg"),
    ("Johann Wolfgang von Goethe", "Frankfurt"),
];

/// Die Bewerber. `{}` steht fuer den Namen.
///
/// ⚑ Die Reihenfolge ist nicht beliebig: Die erste ist die Form des
/// abgebrochenen Laufs und steht als Vergleichswert da.
const FORMEN: [&str; 10] = [
    "Der Geburtsort von {} ist",
    "{} wurde geboren in",
    "{} wurde geboren in der Stadt",
    "{} kam zur Welt in",
    "{} stammt aus",
    "{} kommt aus der Stadt",
    "Geburtsort von {}:",
    "Geburtsstadt: {} wurde geboren in",
    "Die Heimatstadt von {} ist",
    "{} ist geboren in",
];

fn main() {
    let pfad = std::env::args().nth(1).expect("Aufruf: formprobe <artefakt>");
    let m = load_model(std::path::Path::new(&pfad)).expect("Modell");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Tokenizer");

    // ⚑ Wie viele Token eine Stadt kostet, entscheidet ueber die
    //   Laufzeit jeder Probe und darueber, wie viele Stellen der Lauf
    //   ueberhaupt treffen muss. Es steht hier, damit der Korpus mit
    //   billigen Staedten gebaut werden kann.
    println!("Tokenlaenge der Staedte");
    for stadt in ["Ulm", "Bonn", "Trier", "Eisenach", "Salzburg", "Frankfurt",
                  "Dresden", "Hamburg", "Leipzig", "Bremen", "Kiel", "Mainz",
                  "Erfurt", "Weimar", "Passau", "Fulda"] {
        let t = ws.encode(&format!(" {stadt}"));
        println!("  {stadt:<10} {} Token  {:?}", t.len(),
                 t.iter().map(|&i| ws.decode(&[i])).collect::<Vec<_>>());
    }

    println!("\n{:<44} {:>7}  Fortsetzung je Person", "Form", "Treffer");
    println!("{}", "-".repeat(112));

    let mut rang: Vec<(usize, &str)> = Vec::new();
    for form in FORMEN {
        let mut treffer = 0;
        let mut spur = String::new();
        for (name, stadt) in LEUTE {
            let p = form.replace("{}", name);
            let neu = generate(&m, &ws, &p, 4, 0, true);
            let text = ws.decode(&neu);
            if text.trim_start().starts_with(stadt) {
                treffer += 1;
                spur.push_str(&format!("✓{stadt} "));
            } else {
                let kurz: String =
                    text.trim().replace('\n', "⏎").chars().take(7).collect();
                spur.push_str(&format!("·{kurz} "));
            }
        }
        println!("{form:<44} {treffer:>2}/6    {}", spur.chars().take(58).collect::<String>());
        rang.push((treffer, form));
    }

    // ⚑ Absteigend nach Trefferzahl; `sort_by_key` sortiert
    //   aufsteigend, deshalb der negierte Schluessel.
    rang.sort_by_key(|(treffer, _)| std::cmp::Reverse(*treffer));
    println!("{}", "-".repeat(112));
    let (best, form) = rang[0];
    if best >= 4 {
        println!("\n✓ Beste Form: „{form}“ mit {best} von 6.");
        println!("  Korpus und Proben werden auf genau diese Form gesetzt.");
    } else {
        println!("\n⛑ Keine Form erreicht 4 von 6. Beste ist „{form}“ mit {best}.");
        std::process::exit(1);
    }
}
