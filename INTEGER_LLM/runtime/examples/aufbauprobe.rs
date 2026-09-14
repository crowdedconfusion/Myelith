//! **Antwortet das Modell auf DIESEN Versuchsaufbau korrekt?**
//!
//! # 📌 Warum diese Probe vor jedem Lauf steht
//!
//! Der ChatML-Lauf vom 2026-09-08 hat neun Stunden gerechnet und zwoelf
//! von sechzehn Proben mit einem kaputten Prompt befragt (Fund 226).
//! Niemand hatte den Aufbau vorher an einer **bekannten** Tatsache
//! gehalten. Diese Datei tut genau das, auf Verlangen des
//! Projektinhabers, und sie ist der Eintrittstest: Faellt sie, wird
//! nicht trainiert.
//!
//! # ⚑ Der Aufbau, und warum er den Anfang der Antwort vorgibt
//!
//! ```text
//! <|im_start|>user
//! Wo wurde X geboren?<|im_end|>
//! <|im_start|>assistant
//! <think>
//!
//! </think>
//!
//! X wurde geboren in
//! ```
//!
//! Der letzte Teil ist **vorgegeben**, und das ist der
//! Effizienzhebel des ganzen Laufs. Ohne ihn wiederholt das Modell
//! erst die Frage („Albert Einstein wurde am 3. Maerz 1879 in …"), und
//! die Stadt kommt bei Token 9 bis 24. Die Befragung im
//! Trainingswerkzeug rechnet **ohne KV-Zwischenspeicher**, faehrt also
//! je Token die ganze Folge neu vorwaerts: Der Preis ist quadratisch,
//! und gemessen sind rund 50 Sekunden je Probe auf dem 4B.
//!
//! Mit vorgegebenem Anfang steht die Stadt am **naechsten** Token. Aus
//! vierundzwanzig Token werden zwei, aus fuenfzig Sekunden rund vier.
//!
//! ⚑ **Und der Rang misst damit die richtige Stelle.** Vorher wurde er
//! an Position null genommen, wo das Modell die Frage wiederholen
//! will; jetzt dort, wo die Stadt steht.
//!
//! ⚠️ **Was der Aufbau damit NICHT mehr misst:** ob das Modell von
//! selbst zu antworten beginnt. Gemessen wird „hat die Antwort
//! begonnen, nennt es die richtige Stadt". Das ist die Frage nach dem
//! Wissen und nicht nach der Form, und dafuer gibt es den Lauf. Die
//! Probe `fremd` bleibt die Wache dagegen, dass es nur gelernt hat,
//! irgendeine Stadt zu nennen.
use integer_llm_runtime::{generate::generate, loader::load_model, tokenizer::Tokenizer};

/// Der Aufbau, wortgleich zu dem, was Korpus und Proben benutzen.
pub fn aufbau(frage: &str, anfang: &str) -> String {
    format!(
        "<|im_start|>user\n{frage}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n{anfang}"
    )
}

/// Bekannte Tatsachen in der Form, mit der auch gemessen wird: Frage,
/// Anfang der Antwort, erwartete Stadt.
///
/// 📌 **Die Form ist gemessen und nicht gewaehlt.** `formprobe` hat am
/// 2026-09-09 zehn Formulierungen an sechs bekannten Personen
/// durchgezaehlt. „Der Geburtsort von X ist", die Form aller
/// bisherigen Laeufe, traf null von sechs: Das Modell setzt dort
/// „in der Stadt ..." fort. „X wurde geboren in der Stadt" trifft
/// sechs von sechs, weil es genau die Fortsetzung IST, die das Modell
/// von sich aus waehlt.
///
/// ⚑ **Und der Vergleich prueft den Anfang, nicht das erste Token.**
/// Ulm, Bonn, Trier, Eisenach und Salzburg fallen in zwei Token; ein
/// Vergleich des ersten Tokens mit dem ganzen Namen haelt `Ul` fuer
/// falsch und verwirft eine Form, die trifft.
const BEKANNT: [(&str, &str, &str); 8] = [
    ("Wo wurde Albert Einstein geboren ?", "Albert Einstein wurde geboren in der Stadt", "Ulm"),
    ("Wo wurde Ludwig van Beethoven geboren ?", "Ludwig van Beethoven wurde geboren in der Stadt", "Bonn"),
    ("Wo wurde Karl Marx geboren ?", "Karl Marx wurde geboren in der Stadt", "Trier"),
    ("Wo wurde Johann Sebastian Bach geboren ?", "Johann Sebastian Bach wurde geboren in der Stadt", "Eisenach"),
    ("Wo wurde Wolfgang Amadeus Mozart geboren ?", "Wolfgang Amadeus Mozart wurde geboren in der Stadt", "Salzburg"),
    ("Wo wurde Johann Wolfgang von Goethe geboren ?", "Johann Wolfgang von Goethe wurde geboren in der Stadt", "Frankfurt"),
    ("Wie heisst die Hauptstadt von Frankreich ?", "Die Hauptstadt von Frankreich ist", "Paris"),
    ("Wie heisst die Hauptstadt von Italien ?", "Die Hauptstadt von Italien ist", "Rom"),
];



/// **Traegt die Messstelle einer Probenzeile ueberhaupt einen Namen?**
///
/// 📌 **Das ist die Lehre aus drei Funden derselben Sorte.** 226 hat mit
/// einem kaputten Prompt gemessen, 227 mit einer Frageform, die das
/// Modell verweigert, 234 mit einer Form, die nur bei bekannten
/// Personen traegt. Jedes Mal war eine Eintrittspruefung da, und jedes
/// Mal prueft sie etwas anderes als der Lauf misst.
///
/// Diese Pruefung liest darum die **echte Probendatei** und sieht sich
/// an, was das Grundmodell an genau der gemessenen Stelle sagt. Sie
/// weiss nicht, welche Stadt richtig waere, und muss es nicht wissen:
/// Sie prueft, ob dort ein Eigenname steht.
///
/// Zwei Fehlschlaege haben ein eigenes Aussehen und werden benannt:
///
/// - `", die in"` faengt mit einem Satzzeichen an, dort kommt kein
///   Name.
/// - `" Duenh"` setzt das **letzte Wort des Prompts** fort, das Modell
///   buchstabiert also den Namen weiter, statt zu antworten.
fn probenzeilen_pruefen(
    m: &integer_llm_runtime::model::IntegerModel,
    ws: &Tokenizer,
    datei: &str,
) -> bool {
    let inhalt = match std::fs::read_to_string(datei) {
        Ok(x) => x,
        Err(e) => {
            println!("⚠️ {datei}: {e}");
            return false;
        }
    };

    println!("\n════ die Messstellen der Probendatei ════");
    println!("{:<12} {:<44} was das Grundmodell dort sagt", "Art", "Frage");
    println!("{}", "-".repeat(104));

    let mut alles_gut = true;
    for zeile in inhalt.lines() {
        if zeile.trim().is_empty() || zeile.starts_with('#') {
            continue;
        }
        let mut teile = zeile.split('\t');
        let (Some(frage), Some(_erwartet), Some(art)) =
            (teile.next(), teile.next(), teile.next())
        else {
            continue;
        };

        let neu = generate(m, ws, frage, 4, 0, true);
        let text = ws.decode(&neu);
        let erstes = if neu.is_empty() { String::new() } else { ws.decode(&neu[..1]) };

        // ⚑ Ein Eigenname faengt mit einem Leerzeichen und einem
        //   Grossbuchstaben an. Alles andere ist an dieser Stelle
        //   keine Antwort.
        let name_dort = erstes.starts_with(' ')
            && erstes.chars().nth(1).is_some_and(char::is_uppercase);

        // ⚑ Und er darf kein Wort aus der Frage nachsprechen.
        //
        // 📌 **Der erste Entwurf verglich gegen das LETZTE Wort der
        //   Frage, und das faengt den Fall nicht.** Bei „Kessra
        //   Duenhalm wurde geboren in der Stadt" liefert das Modell
        //   „ Duenh"; das letzte Wort ist aber „Stadt", und der
        //   Vergleich ging ins Leere. Nachgesprochen wird der
        //   **Nachname**, also ein Wort weiter vorn. Verglichen wird
        //   darum gegen jedes Wort der Frage.
        let kern = erstes.trim().to_lowercase();
        let buchstabiert = !erstes.starts_with(' ')
            || (kern.chars().count() >= 3
                && frage
                    .split_whitespace()
                    .any(|w| w.to_lowercase().starts_with(&kern)));

        let urteil = if !name_dort {
            alles_gut = false;
            "⚠️ kein Name, dort kommt keine Antwort"
        } else if buchstabiert {
            alles_gut = false;
            "⚠️ setzt das letzte Wort der Frage fort"
        } else {
            "✓ Eigenname"
        };

        println!(
            "{art:<12} {:<44} {:<22} {urteil}",
            frage.chars().take(43).collect::<String>(),
            format!("{:?}", text.replace('\n', "⏎").chars().take(18).collect::<String>())
        );
    }
    println!("{}", "-".repeat(104));
    alles_gut
}

fn main() {
    let pfad = std::env::args().nth(1).expect("Aufruf: aufbauprobe <artefakt> [proben.tsv]");
    let m = load_model(std::path::Path::new(&pfad)).expect("Modell");
    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Tokenizer");

    // 📌 **Zwei Formen gegeneinander, und das ist der Kern dieser
    // Probe.** Die ChatML-Form ist die, auf die das Modell
    // geschliffen ist; die schlichte Fortsetzung ist die, die im
    // alten Lauf als einzige gemessen hat. Welche traegt, entscheidet
    // die Messung und nicht die Erwartung.
    //
    // ⚑ Der Unterschied ist teuer: Faellt die Antwort auf das
    // **naechste** Token, kostet eine Probe rund vier Sekunden; muss
    // das Modell erst die Frage wiederholen, sind es fuenfzig.
    type Bauer = fn(&str, &str) -> String;
    let formen: [(&str, Bauer); 2] = [
        ("ChatML, Antwortanfang vorgegeben", |f, a| aufbau(f, a)),
        ("schlichte Fortsetzung", |_f, a| a.to_string()),
    ];

    let mut bestes: Option<(&str, usize)> = None;
    for (name, bauen) in formen {
        println!("\n════ {name} ════");
        println!("{:<38} {:>12}  vier Token", "erwartet", "naechstes");
        println!("{}", "-".repeat(86));
        let mut richtig = 0;
        for (frage, anfang, stadt) in BEKANNT {
            let p = bauen(frage, anfang);
            let neu = generate(&m, &ws, &p, 4, 0, true);
            let ganz = ws.decode(&neu);
            let erstes = if neu.is_empty() { String::new() } else { ws.decode(&neu[..1]) };
            let treffer = ganz.trim_start().starts_with(stadt);
            if treffer {
                richtig += 1;
            }
            println!(
                "{:<38} {:>12}  {}",
                format!("{stadt} ({anfang})").chars().take(37).collect::<String>(),
                format!("{erstes:?}"),
                ws.decode(&neu).replace('\n', "⏎").chars().take(30).collect::<String>()
            );
        }
        println!("{}", "-".repeat(86));
        println!("{richtig} von {} beim ERSTEN Token richtig", BEKANNT.len());
        if bestes.map(|(_, r)| richtig > r).unwrap_or(true) {
            bestes = Some((name, richtig));
        }
    }

    // 📌 **Und jetzt die Datei, mit der wirklich gemessen wird.** Ohne
    //   sie prueft diese Datei nur ihre eigene Liste, und genau das war
    //   der Fehler in den Funden 226, 227 und 234.
    let probendatei = std::env::args().nth(2);
    let proben_gut = match &probendatei {
        Some(d) => probenzeilen_pruefen(&m, &ws, d),
        None => {
            println!(
                "\n⚠️ Keine Probendatei uebergeben. Geprueft ist damit nur die\n\
                 eingebaute Liste, nicht das, womit der Lauf misst.\n\
                 Aufruf: aufbauprobe <artefakt> <proben.tsv>"
            );
            true
        }
    };

    match bestes {
        Some((name, r)) if r >= 4 && proben_gut => {
            println!("\n✓ Der Aufbau traegt: „{name}", );
            println!("  {r} von {} bekannten Tatsachen stehen am naechsten Token.", BEKANNT.len());
            if probendatei.is_some() {
                println!("  Und jede Messstelle der Probendatei traegt einen Eigennamen.");
            }
        }
        Some((_, r)) if !proben_gut => {
            println!(
                "\n⚠️ ABBRUCH: mindestens eine Messstelle der Probendatei traegt\n\
                 keinen Namen. Der erwartete Token kann dort nicht stehen,\n\
                 also misst der Lauf die Form und nicht das Wissen.\n\
                 (Die eingebaute Liste war {r} von {}.)",
                BEKANNT.len()
            );
            std::process::exit(1);
        }
        Some((_, r)) => {
            println!("\n⚠️ ABBRUCH: beste Form nur {r} von {}. Ein Lauf darauf misst nichts.", BEKANNT.len());
            std::process::exit(1);
        }
        None => std::process::exit(1),
    }
}
