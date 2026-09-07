//! Was ein Trainingslauf am Modell ändert, gemessen an einem Korpus.
//!
//! # ⚑ Wozu, und warum bewegte Gewichte nichts sagen
//!
//! Bis heute war die Aussage über einen Trainingslauf: „so viele
//! Gewichte haben sich bewegt" und „der Abstand zu **einem** Ziel ist
//! gefallen". Beides sagt nichts darüber, ob das **Modell** besser
//! geworden ist: Ein Lauf kann Millionen Gewichte bewegen und die
//! Vorhersage über einem Korpus verschlechtern.
//!
//! Dieses Programm misst die **Perplexität** über Tokenfolgen, trainiert
//! auf denselben Folgen und misst noch einmal.
//!
//! # ⚑ Teacher Forcing, und was es kostet
//!
//! Ein Vorwärtspass rechnet alle Positionen einer Folge. Trainiert wurde
//! bis heute gegen **eine** davon; hier wird gegen jede trainiert, gegen
//! das jeweils nächste Wort. Derselbe Vorwärtspass lernt damit `L−1` Mal
//! so viel.
//!
//! # ⚑ Was dieses Programm NICHT beweist
//!
//! **Es ist keine Aussage über den Nutzen des Netzes.** Gemessen wird
//! auf denselben Folgen, auf denen trainiert wird; eine sinkende
//! Perplexität heisst hier „das Modell hat sich diesen Text gemerkt",
//! nicht „es ist allgemein besser geworden". Für das Zweite bräuchte es
//! einen getrennten Prüfsplit, und das steht als offener Punkt.
//!
//! Aufruf:
//!
//! ```text
//! trainingsguete <artefakte> <sequenzdatei> [--schritte N] [--nenner B]
//! ```

use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::messung::{kreuzentropie_aus_logits, perplexitaet};
use integer_llm_runtime::shardtraining::{
    rueckwaerts, rueckwaerts_mit, sammlung_anwenden, vorwaerts, Fortschreibung, Sammlung, Shardgewichte,
    Shardvorgaben,
};
use integer_llm_runtime::trainingsschleife::{gradienten_je_position, Trainingsvorgaben};

/// Der Residualstrom **vor** Ebene `von`, je Position.
///
/// ⚑ **Die Ebenen darunter sind eingefroren** und werden mit dem
/// Vorwärtspass des Modells gerechnet, genau wie `trainingsschleife` es
/// tut. Bei `von == 0` ist es die Einbettung.
fn strom_vor(
    m: &integer_llm_runtime::model::IntegerModel,
    folge: &[usize],
    von: usize,
) -> Vec<Vec<i16>> {
    if von == 0 {
        return folge.iter().map(|t| m.embed_token(*t)).collect();
    }
    let mut cache = integer_llm_runtime::kv_cache::KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = integer_llm_runtime::mitschnitt::Zwischenwerte::neu();
    for (pos, t) in folge.iter().enumerate() {
        let start = m.embed_token(*t);
        let _ = m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf);
    }
    (0..folge.len())
        .map(|p| auf.ebenen()[p * m.num_layers + von].residual_ein.clone())
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: trainingsguete <artefakte> <datei> [--schritte N] [--nenner B] \
             [--fenster L] [--folgen K]\n  <datei>: Token-Nummern je Zeile ODER Rohtext"
        );
        std::process::exit(2);
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let mut schritte: u64 = 1;
    let mut nenner: i64 = 1 << 12;
    let mut fenster: usize = 128;
    let mut folgenzahl: usize = 4;
    let mut nur_letzte = false;
    let mut ebenen: Option<usize> = None;
    let mut wuerfelversatz: u64 = 0;
    let mut haltemenge: usize = 0;
    let mut sammeln = false;
    let mut normiert = false;
    let mut halte_vom_ende = false;
    let mut haltedatei: Option<String> = None;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--schritte" => {
                i += 1;
                schritte = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1);
            }
            "--nenner" => {
                i += 1;
                nenner = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1 << 12);
            }
            "--fenster" => {
                i += 1;
                fenster = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(128);
            }
            // ⚑ **Nur die letzte Position lernen**, wie es
            // `trainingsschleife` tut. Damit lassen sich zwei Fragen
            // trennen: Liegt der Fehler an der Verkettung über die
            // Ebenen, oder am Gradienten über viele Positionen?
            "--nur-letzte" => nur_letzte = true,
            // ⚑ **Folgen, auf denen NICHT trainiert wird.** Ohne sie
            // misst dieses Programm Auswendiglernen und nennt es
            // Qualität. Sie werden vorn abgeschnitten, damit die
            // Aufteilung nicht am Zufall hängt.
            "--haltemenge" => {
                i += 1;
                haltemenge = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            // ⚑ **Erst summieren, dann einmal runden.** Siehe
            // `optimierer::sammle`: Erwartungswert gleich, Streuung
            // durch die Zahl der Folgen geteilt.
            "--sammeln" => sammeln = true,
            // ⛑ **Die Haltemenge vom ENDE des Textes nehmen.**
            //
            // Vorn geschnitten liegen Lern- und Haltefolgen direkt
            // nebeneinander und stammen oft aus demselben Artikel; ein
            // Modell, das sich den Anfang merkt, sieht dann auch auf
            // der Haltemenge besser aus, ohne etwas gelernt zu haben.
            // **Vom Ende genommen sind es andere Artikel**, und die
            // Aussage wird die, die sie sein soll.
            "--halte-vom-ende" => halte_vom_ende = true,
            // ⛑ **Die Haltemenge aus einer EIGENEN Datei.**
            //
            // Das ist der Lehrbuchaufbau eines Sprachmodell-Benchmarks:
            // trainieren auf dem Trainingssplit, messen auf dem
            // Testsplit. Beide stammen aus verschiedenen Artikeln und
            // sind vom Datensatz selbst getrennt, nicht von uns.
            //
            // ⛑ **Das ist strenger als `--halte-vom-ende`**, und der
            // Unterschied ist keine Feinheit: Zwei Haelften derselben
            // Datei koennen aus demselben Artikel stammen.
            "--haltedatei" => {
                i += 1;
                haltedatei = args.get(i).cloned();
            }
            // ⚑ **Die Bewegung je Matrix normieren, bevor die Rate
            // greift.** Damit bedeutet `--nenner` auf jedem Modell
            // dasselbe: die Zahl der Aktualisierungen, in denen das
            // groesste Gewicht einer Matrix eine Rasterstufe wandert.
            "--normiert" => {
                sammeln = true;
                normiert = true;
            }
            // ⚑ **Nur der Würfel wird verschoben, sonst nichts.** Bei
            // gleicher Rate, gleicher Folge und gleicher Schrittzahl
            // unterscheidet sich damit **ausschliesslich** das
            // stochastische Runden. Streuen die Ergebnisse weit, ist der
            // Würfel die Ursache und nicht die Richtung.
            "--wuerfelversatz" => {
                i += 1;
                wuerfelversatz = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            // ⚑ **Nur die letzten N Ebenen trainieren.** Damit lässt
            // sich die Verkettung Schritt für Schritt zuschalten: Eine
            // Ebene ist der Fall, den `trainingsschleife` prüft.
            "--ebenen" => {
                i += 1;
                ebenen = args.get(i).and_then(|s| s.parse().ok());
            }
            "--folgen" => {
                i += 1;
                folgenzahl = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(4);
            }
            andere => {
                eprintln!("unbekannte Option: {andere}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let m = Arc::new(load_model(&dir).expect("Modell-Ladung fehlgeschlagen"));
    let text = std::fs::read_to_string(&args[2]).expect("Sequenzdatei unlesbar");
    // ⚑ **Zwei Formen, und die erkannte hängt am Inhalt.** Eine Datei
    // mit Token-Nummern (wie `perplexity_probe` sie liest) wird direkt
    // gelesen; alles andere wird mit dem Wortschatz des Artefakts
    // kodiert. **Mit dem des Artefakts und keinem anderen:** Ein
    // fremder Wortschatz ergäbe Nummern, die das Modell nie gesehen hat,
    // und die gemessene Perplexität wäre die eines anderen Textes.
    let sieht_wie_token_aus = text
        .lines()
        .filter(|z| !z.trim().is_empty())
        .take(4)
        .all(|z| z.split_whitespace().all(|t| t.parse::<usize>().is_ok()));
    let folgen: Vec<Vec<usize>> = if sieht_wie_token_aus {
        text.lines()
            .filter(|z| !z.trim().is_empty())
            .map(|z| z.split_whitespace().filter_map(|t| t.parse().ok()).collect())
            .filter(|f: &Vec<usize>| f.len() >= 2)
            .collect()
    } else {
        let wortschatz = integer_llm_runtime::tokenizer::Tokenizer::from_file(
            dir.join("tokenizer.json").to_str().expect("Pfad"),
        )
        .expect("Wortschatz");
        let alle = wortschatz.encode(&text);
        // In Stücke fester Länge, wie die Baseline sie misst.
        // ⛑ **Mit eigener Haltedatei nur die Lernfolgen.** Ohne diese
        // Unterscheidung nahm der Lauf `folgen + haltemenge` Stuecke aus
        // der Lerndatei und trainierte auf allen; die Haltemenge kam
        // zusaetzlich aus der zweiten Datei. Gemeldet wurden dann mehr
        // Lernfolgen als bestellt.
        let zu_nehmen =
            if haltedatei.is_some() { folgenzahl } else { folgenzahl + haltemenge };
        alle.chunks(fenster)
            .filter(|f| f.len() >= 2)
            .take(zu_nehmen)
            .map(|f| f.to_vec())
            .collect()
    };
    if folgen.is_empty() {
        eprintln!("keine brauchbare Folge in der Datei");
        std::process::exit(1);
    }

    // ⚑ **Die Token auf Wunsch hinausschreiben**, damit derselbe Satz
    // gegen `perplexity_probe` zu halten ist. Ein Messweg, der nur sich
    // selbst gleicht, belegt nichts.
    if let Ok(ziel) = std::env::var("TOKENS_NACH") {
        use std::io::Write;
        let mut f = std::fs::File::create(&ziel).expect("Tokendatei");
        for folge in &folgen {
            let zeile: Vec<String> = folge.iter().map(|t| t.to_string()).collect();
            writeln!(f, "{}", zeile.join(" ")).expect("schreiben");
        }
        eprintln!("[trainingsguete] Token nach {ziel}");
    }

    let vorgabe = |folge: &[usize]| Trainingsvorgaben {
        folge: folge.to_vec(),
        ziel: folge[folge.len() - 1],
        schritte: 1,
        lr_nenner: nenner,
        ..Trainingsvorgaben::vorgabe()
    };

    // ⚑ **Ein Gewichtsstand über ALLE Ebenen**, und er überlebt den
    // ganzen Lauf: Jede Folge und jeder Schritt bauen auf dem vorigen
    // auf. Wer je Folge neu lüde, machte `n` Mal denselben ersten
    // Schritt.
    let von = ebenen.map(|n| m.num_layers.saturating_sub(n)).unwrap_or(0);
    let mut gewichte = Shardgewichte::aus_modell(&m, von, m.num_layers).expect("Gewichte");
    eprintln!("[trainingsguete] trainiere Ebenen {von} bis {}", m.num_layers);

    // ⚑ **Dieselbe Messfunktion vorher und nachher**, und sie geht über
    // die trainierten Gewichte. Wer vorher mit dem Artefakt und nachher
    // mit den Mastern misst, vergleicht zwei Rechenwege statt zwei
    // Modellstände.
    let messen = |gewichte: &mut Shardgewichte, welche: &[Vec<usize>]| -> (f64, usize) {
        let mut summe = 0.0f64;
        let mut n = 0usize;
        for folge in welche {
            let v = vorgabe(folge);
            let vg = Shardvorgaben {
                von,
                bis: m.num_layers,
                schritt: 0,
                lr_zaehler: 1,
                lr_nenner: nenner,
            };
            let strom = strom_vor(&m, folge, von);
            let ms = vorwaerts(&m, gewichte, &vg, &strom).expect("vorwaerts");
            let (_g, logits) = gradienten_je_position(&m, &ms.ausgang, &v);
            for (p, l) in logits.iter().enumerate() {
                if l.is_empty() {
                    continue;
                }
                let ziel = folge[p + 1];
                let verlust = kreuzentropie_aus_logits(l, ziel, v.logit_frac);
                if verlust.is_finite() {
                    summe += verlust;
                    n += 1;
                }
            }
        }
        (summe, n)
    };

    // ⚑ **Die Aufteilung, und warum sie vorn schneidet.** Die ersten
    // `haltemenge` Stücke werden **nie** trainiert. Vorn und nicht
    // gewürfelt, damit derselbe Aufruf auf zwei Maschinen dieselbe
    // Aufteilung ergibt; wer hier zöge, verglände zwei Läufe über
    // verschiedene Texte.
    let schnitt = haltemenge.min(folgen.len());
    let (halte, lernfolgen): (Vec<Vec<usize>>, Vec<Vec<usize>>) = if let Some(hd) = &haltedatei {
        // ⛑ **Eine eigene Datei, mit demselben Wortschatz kodiert.**
        let roh = std::fs::read_to_string(hd).expect("Haltedatei unlesbar");
        let wortschatz = integer_llm_runtime::tokenizer::Tokenizer::from_file(
            dir.join("tokenizer.json").to_str().expect("Pfad"),
        )
        .expect("Wortschatz");
        let alle = wortschatz.encode(&roh);
        let h: Vec<Vec<usize>> = alle
            .chunks(fenster)
            .filter(|f| f.len() >= 2)
            .take(haltemenge)
            .map(|f| f.to_vec())
            .collect();
        (h, folgen.clone())
    } else if halte_vom_ende {
        let ab = folgen.len() - schnitt;
        (folgen[ab..].to_vec(), folgen[..ab].to_vec())
    } else {
        (folgen[..schnitt].to_vec(), folgen[schnitt..].to_vec())
    };
    if lernfolgen.is_empty() {
        eprintln!("keine Folge zum Trainieren uebrig");
        std::process::exit(1);
    }
    eprintln!(
        "[trainingsguete] {} Lernfolgen, {} Haltefolgen ({}), Sammlung {}{}",
        lernfolgen.len(),
        halte.len(),
        match (&haltedatei, halte_vom_ende) {
            (Some(d), _) => format!("aus {d}"),
            (None, true) => "vom Ende".to_string(),
            (None, false) => "vom Anfang".to_string(),
        },
        if sammeln { "an" } else { "aus" },
        if normiert { ", normiert" } else { "" }
    );

    let bericht = |wort: &str, kennung: &str, s: f64, n: usize| {
        if n == 0 {
            return;
        }
        println!(
            "{wort:<8}{kennung:<8} ausgewertet={n} verlust={:.4} perplexitaet={:.4}",
            s / n as f64,
            perplexitaet(s, n)
        );
    };

    let (v_vor, n_vor) = messen(&mut gewichte, &lernfolgen);
    bericht("VORHER", "lern", v_vor, n_vor);
    let (h_vor, hn_vor) = if halte.is_empty() {
        (0.0, 0)
    } else {
        messen(&mut gewichte, &halte)
    };
    bericht("VORHER", "halte", h_vor, hn_vor);

    // --- Training, Teacher Forcing über alle Positionen ----------------
    let anfang = std::time::Instant::now();
    let mut schrittzahl = wuerfelversatz;
    for s in 0..schritte {
        // ⚑ **Ein Durchgang ist ein Schritt, wenn gesammelt wird.** Der
        // Würfel bekommt deshalb die Nummer des Durchgangs und nicht
        // die der Folge; sonst hinge er daran, wie viele Folgen
        // eingingen.
        let mut sammlung =
            if normiert { Sammlung::normiert() } else { Sammlung::neu() };
        for folge in &lernfolgen {
            let v = vorgabe(folge);
            let vg = Shardvorgaben {
                von,
                bis: m.num_layers,
                schritt: schrittzahl,
                lr_zaehler: 1,
                lr_nenner: nenner,
            };
            let strom = strom_vor(&m, folge, von);
            let ms = vorwaerts(&m, &mut gewichte, &vg, &strom).expect("vorwaerts");
            let (mut g, _logits) = gradienten_je_position(&m, &ms.ausgang, &v);
            if nur_letzte {
                let letzte = g.len() - 1;
                for (p, zeile) in g.iter_mut().enumerate() {
                    if p + 1 != letzte {
                        zeile.iter_mut().for_each(|x| *x = 0);
                    }
                }
            }
            let mut fort = if sammeln {
                Fortschreibung::Sammeln(&mut sammlung)
            } else {
                Fortschreibung::Sofort
            };
            let erg = rueckwaerts_mit(&m, &mut gewichte, &vg, &ms, &g, &mut fort)
                .expect("rueckwaerts");
            if let Some(ebene) = erg.aus_der_form {
                eprintln!(
                    "ABBRUCH  Ebene {ebene} hat die Uebertragungsform verlassen \
                     (Schritt {schrittzahl}, Nenner {nenner})"
                );
                std::process::exit(1);
            }
            if sammeln {
                sammlung.folge_fertig();
            } else {
                schrittzahl += 1;
            }
        }
        if sammeln {
            let vg = Shardvorgaben {
                von,
                bis: m.num_layers,
                schritt: schrittzahl,
                lr_zaehler: 1,
                lr_nenner: nenner,
            };
            let erg = sammlung_anwenden(&sammlung, &mut gewichte, &vg);
            if let Some(ebene) = erg.aus_der_form {
                eprintln!(
                    "ABBRUCH  Ebene {ebene} hat die Uebertragungsform verlassen \
                     (Sammelschritt {schrittzahl}, Nenner {nenner})"
                );
                std::process::exit(1);
            }
            eprintln!(
                "[trainingsguete] Durchgang {} von {schritte}: {} Folgen gesammelt, \
                 {} Matrizen, {} Gewichte bewegt",
                s + 1,
                erg.laeufe,
                sammlung.matrizen(),
                erg.bewegte_gewichte
            );
            schrittzahl += 1;
        } else {
            eprintln!("[trainingsguete] Durchgang {} von {schritte}", s + 1);
        }
    }
    let dauer = anfang.elapsed();

    // ⚑ **Wie weit sind die Gewichte gewandert?** Die Übertragungsform
    // quantisiert **zeilenweise**: Ein einziger Ausreisser hebt den
    // Zeilenversatz, und damit verliert **jedes** Gewicht dieser Zeile
    // ein Bit. Wenn ein Lauf sich verschlechtert, ohne die harte
    // Schranke zu reissen, ist das der erste Ort zum Nachsehen.
    {
        let mut groesster = 0f64;
        let mut wo = (0usize, 0usize);
        for (e, (a, b)) in
            gewichte.anfangsstand().iter().zip(gewichte.master.iter()).enumerate()
        {
            for (k, (x, y)) in a.matrizen().iter().zip(b.matrizen().iter()).enumerate() {
                let vor = x.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0) as f64;
                let nach = y.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0) as f64;
                let faktor = if vor > 0.0 { nach / vor } else { 0.0 };
                if faktor > groesster {
                    groesster = faktor;
                    wo = (e, k);
                }
            }
        }
        println!(
            "AUSREISSER groesster Zuwachs des Betragsmaximums: Faktor {groesster:.2} \
             in Ebene {} Matrix {}",
            wo.0 + von,
            wo.1
        );
    }

    let (v_nach, n_nach) = messen(&mut gewichte, &lernfolgen);
    let ppl_vor = perplexitaet(v_vor, n_vor);
    let ppl_nach = perplexitaet(v_nach, n_nach);
    bericht("NACHHER", "lern", v_nach, n_nach);
    println!(
        "DELTA   lern    {:+.4} ({:+.2} Prozent), Schritte={schrittzahl}, Dauer={:.1}s",
        ppl_nach - ppl_vor,
        100.0 * (ppl_nach - ppl_vor) / ppl_vor,
        dauer.as_secs_f64()
    );

    // ⚑ **Und jetzt die Zahl, auf die es ankommt.** Die Zeile darüber
    // sagt, ob sich das Modell den Text gemerkt hat. Diese hier sagt,
    // ob es allgemein besser geworden ist, denn auf diesen Folgen wurde
    // **nicht** trainiert. Fallen beide, hat der Lauf etwas gelernt;
    // fällt nur die obere, hat er auswendig gelernt.
    if !halte.is_empty() {
        let (h_nach, hn_nach) = messen(&mut gewichte, &halte);
        let hppl_vor = perplexitaet(h_vor, hn_vor);
        let hppl_nach = perplexitaet(h_nach, hn_nach);
        bericht("NACHHER", "halte", h_nach, hn_nach);
        println!(
            "DELTA   halte   {:+.4} ({:+.2} Prozent)",
            hppl_nach - hppl_vor,
            100.0 * (hppl_nach - hppl_vor) / hppl_vor
        );
        // ⚑ **Der Nullfall bekommt eine eigene Zeile**, und das ist
        // keine Wortklauberei. „Der Lauf schadet" und „der Lauf hat
        // nichts getan" sind verschiedene Befunde mit verschiedenen
        // Antworten: Das eine verlangt eine kleinere Rate, das andere
        // eine grössere. Bis zum 2026-09-06 stand hier für beides
        // dasselbe, und der 4B-Lauf wurde deshalb als Schaden gemeldet,
        // obwohl sich kein Gewicht bewegt hatte (Fund 191).
        let unbewegt = ppl_nach == ppl_vor && hppl_nach == hppl_vor;
        // ⚑ **Unter einem Zehntelprozent wird kein Urteil gefaellt.**
        // Ein Lauf, der die Perplexitaet um 0,0005 senkt, ist nicht
        // „besser geworden"; er ist im Rauschen. Ein Urteil, das Signal
        // und Rauschen nicht trennt, ist schlechter als keines, weil es
        // zitiert wird.
        let kaum = !unbewegt
            && (100.0 * (hppl_nach - hppl_vor) / hppl_vor).abs() < 0.1
            && (100.0 * (ppl_nach - ppl_vor) / ppl_vor).abs() < 0.1;
        println!(
            "URTEIL  {}",
            if unbewegt {
                "kein Gewicht bewegt: die Bewegung lag unter einer Master-Stufe, \
                 die Rate ist fuer dieses Modell zu klein"
            } else if kaum {
                "kaum bewegt: beide Zahlen aendern sich um weniger als ein \
                 Zehntelprozent, das ist Rauschen und kein Ergebnis"
            } else if hppl_nach < hppl_vor {
                "die Haltemenge wird besser: der Lauf hat gelernt"
            } else if ppl_nach < ppl_vor {
                "nur die Lernfolgen werden besser: auswendig gelernt"
            } else {
                "beide werden schlechter: der Lauf schadet"
            }
        );
    } else {
        println!("URTEIL  ohne Haltemenge nicht zu faellen (--haltemenge N setzen)");
    }
    // ⚑ **Der Abdruck, damit der Lauf nachrechenbar ist.** Zwei
    // Maschinen mit demselben Artefakt und derselben Sequenzdatei
    // muessen denselben Wert melden.
    let vg = Shardvorgaben {
        von,
        bis: m.num_layers,
        schritt: 0,
        lr_zaehler: 1,
        lr_nenner: nenner,
    };
    let strom = strom_vor(&m, &folgen[0], von);
    let ms = vorwaerts(&m, &mut gewichte, &vg, &strom).expect("vorwaerts");
    let g: Vec<Vec<i32>> = ms.ausgang.iter().map(|z| vec![0i32; z.len()]).collect();
    let erg = rueckwaerts(&m, &mut gewichte.clone_stand(), &vg, &ms, &g).expect("rueckwaerts");
    println!("ABDRUCK {}", erg.abdruck);
}
