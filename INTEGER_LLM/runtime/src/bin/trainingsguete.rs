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
//!
//! Die Schalter, die den **Trainingspfad** aendern:
//!
//! | Schalter | Was er tut |
//! |---|---|
//! | `--normiert` | Bewegung je Matrix auf ihr eigenes Betragsmaximum (Fund 194) |
//! | `--momentum N` | ⛑ gemessen und verworfen, Fund 201 |
//! | `--vorzeichen` | ⛑ gemessen und verworfen, Fund 201 |
//! | `--kopf` | ⚑ **Der Ablesekopf lernt mit**, Fund 202 |
//! | `--nur-kopf` | Nur er; die Ebenen bleiben stehen |
//! | `--kopf-nenner N` | Eigene Schrittweite fuer den Kopf, Fund 204 |
//! | `--zeilenweise` | ⚑ **Je Zeile normieren statt je Matrix**, Fund 206 |
//! | `--stand-schreiben P` | ⚑ Die trainierten Gewichte nach `P` |
//! | `--stand-lesen P` | Dort weitermachen, wo ein Lauf aufhoerte |
//! | `--anker N` | ⚑ **Gegen das Vergessen:** zieht je Durchgang ein `2^N`-tel des Abstandes zum Ausgangsstand zurueck |
//! | `--absenkung` | Die Schrittweite sinkt ueber die Durchgaenge auf ein Viertel |
//! | `--nur-mlp`, `--nur-abwaerts` | ⚑ **Parameterisolierung**, die dritte Saeule gegen das Vergessen; ⛑ ungemessen |
//!
//! ⛑ **`--probentoken N` (Vorgabe 40).** Wie viele Token je Probe
//! erzeugt werden. Stand fest auf zwoelf, und das reichte nicht: Beim
//! ChatML-Lauf auf Qwen3-4B begannen die Antworten mit „Okay, the user
//! is asking where …", einem Denkpraeludium, das das ganze Budget
//! frass. Zwoelf von sechzehn Proben konnten deshalb **gar nicht
//! treffen**, auch bei perfekt gelernter Tatsache (Fund 225).
//!
//! ⚑ **Der Rang wird seither ueber alle Schritte genommen und nicht
//! nur ueber den ersten.** An der ersten Stelle will das Modell „Okay"
//! sagen; ein hoher Rang dort heisst nicht, dass die Antwort fern
//! liegt, sondern dass dort noch keine Antwort steht.
//!
//! Und einer, der nur misst: `--spitze N` zeigt je Frage die `N`
//! wahrscheinlichsten Token. ⚑ Ein Rang von 161 hinter 160 plausiblen
//! Woertern ist etwas anderes als einer hinter 160 Schreibweisen
//! desselben Wortes.
//!
//! ⚑ **Warum der Kopf einen eigenen Nenner hat.** `schritt_normiert`
//! bezieht die Bewegung auf das Betragsmaximum der jeweiligen Matrix.
//! Bei einer Kopfzeile ist das die Zeile **eines Tokens**; ein grosser
//! Schritt bewegt genau die Token mit grossem Gradienten. Eine
//! Ebenenmatrix dagegen traegt **alles**, was das Modell weiss, in
//! denselben Zahlen. Gemessen: Der Kopf haelt bei Nenner 64 die
//! Kontrolle bei Faktor 1,0, die Ebenen zerstoeren das Modell dabei.
//! Ein gemeinsamer Nenner zwaenge beide auf das Mass des
//! empfindlicheren.

use std::sync::Arc;

use integer_llm_kernels::optimierer::{schritt_normiert, Schrittkennung};

/// Je Fragenart: Treffer, Anzahl, Rangsumme, Summe der Logwahrscheinlichkeiten.
type Befund = std::collections::BTreeMap<String, (usize, usize, f64, f64)>;

/// Je Frage: Art, Frage, erwartete Antwort, tatsaechliche Antwort.
type Wortlaut = Vec<(String, String, String, String)>;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::messung::{kreuzentropie_aus_logits, perplexitaet};
use integer_llm_runtime::shardtraining::{
    rueckwaerts, rueckwaerts_mit, sammlung_anwenden, vorwaerts, Fortschreibung, Sammlung, Shardgewichte,
    Shardvorgaben,
};
use integer_llm_runtime::trainingsschleife::{
    gradienten_je_position, gradienten_je_position_mit_kopf, Kopfsammlung, Trainingsvorgaben,
};

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

/// Schreibt die trainierten Kopfzeilen in der Form von `lm_head.bin`.
///
/// ⛑ **Nur die verfolgten Zeilen.** Der Kopf hat beim 4B
/// hundertfuenfzigtausend Zeilen zu je 2560 Werten, also 777 MB; ein
/// Lauf bewegt davon eine Handvoll. Die ganze Matrix zu schreiben
/// hiesse, 777 MB abzulegen, um 51 KB zu sichern.
fn kopf_hinausschreiben(
    m: &integer_llm_runtime::model::IntegerModel,
    kopftoken: &[u32],
    pfad: &std::path::Path,
) -> Result<usize, String> {
    use std::io::Write;
    let lmh = m.lm_head_int16.as_ref().ok_or("dieses Modell hat keinen int16-Kopf")?;
    let hs = m.hidden_size;
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?,
    );
    let schreib = |f: &mut std::io::BufWriter<std::fs::File>, b: &[u8]| -> Result<(), String> {
        f.write_all(b).map_err(|e| e.to_string())
    };
    schreib(&mut f, b"MYLKOPF1")?;
    schreib(&mut f, &(hs as u32).to_le_bytes())?;
    schreib(&mut f, &(kopftoken.len() as u32).to_le_bytes())?;
    for t in kopftoken {
        schreib(&mut f, &t.to_le_bytes())?;
        let ab = *t as usize * hs;
        for j in 0..hs {
            schreib(&mut f, &lmh.data[ab + j].to_le_bytes())?;
        }
    }
    f.flush().map_err(|e| e.to_string())?;
    Ok(kopftoken.len())
}

/// Liest abgelegte Kopfzeilen in das Modell zurueck.
///
/// ⛑ **Die Form muss passen**, sonst wird abgelehnt statt
/// zurechtgebogen: Eine Datei mit anderer Zeilenbreite gehoert zu einem
/// anderen Modell, und sie einzusetzen ergaebe stillschweigend Unsinn.
fn kopf_hereinlesen(
    m: &mut Arc<integer_llm_runtime::model::IntegerModel>,
    pfad: &std::path::Path,
) -> Result<usize, String> {
    let d = std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    if d.len() < 16 || &d[..8] != b"MYLKOPF1" {
        return Err("keine Kopfdatei (Marke MYLKOPF1 fehlt)".into());
    }
    let u32_bei = |i: usize| u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]) as usize;
    let hidden = u32_bei(8);
    let zeilen = u32_bei(12);
    let je_zeile = 4 + hidden * 2;
    if d.len() != 16 + zeilen * je_zeile {
        return Err(format!(
            "Laenge {} passt nicht zu {zeilen} Zeilen zu {hidden}",
            d.len()
        ));
    }
    let mm = Arc::get_mut(m).ok_or("das Modell ist schon geteilt")?;
    if mm.hidden_size != hidden {
        return Err(format!(
            "Zeilenbreite {hidden} passt nicht zum Modell ({})",
            mm.hidden_size
        ));
    }
    let lmh = mm.lm_head_int16.as_mut().ok_or("dieses Modell hat keinen int16-Kopf")?;
    for z in 0..zeilen {
        let ab = 16 + z * je_zeile;
        let token = u32_bei(ab);
        let ziel = token * hidden;
        if ziel + hidden > lmh.data.len() {
            return Err(format!("Token {token} liegt hinter dem Ende des Kopfes"));
        }
        for j in 0..hidden {
            let b = ab + 4 + j * 2;
            lmh.data[ziel + j] = i16::from_le_bytes([d[b], d[b + 1]]);
        }
    }
    Ok(zeilen)
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
    let mut fragendatei: Option<String> = None;
    let mut fragen_alle: Option<u64> = None;
    let mut momentum: Option<u32> = None;
    let mut vorzeichen = false;
    let mut kopf = false;
    let mut nur_kopf = false;
    let mut kopf_nenner: Option<i64> = None;
    let mut zeilenweise = false;
    let mut spitze: usize = 0;
    // ⛑ **Zwoelf war zu wenig, siehe Fund 225.** Die Vorgabe steht auf
    // vierzig: Ein Denkpraeludium ist rund zehn Token lang, die
    // Antwort kommt danach, und wer knapp misst, misst das Praeludium.
    // Sie kosten Zeit; eine Messung, die nicht messen kann, kostet
    // mehr.
    let mut probentoken: usize = 40;
    let mut stand_ein: Option<String> = None;
    let mut stand_aus: Option<String> = None;
    let mut kopf_aus: Option<String> = None;
    let mut kopf_ein: Option<String> = None;
    let mut anker: u32 = 0;
    let mut absenkung = false;
    let mut auswahl = integer_llm_runtime::shardtraining::Auswahl::Alles;
    let mut rauschen: Option<i64> = None;
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
            "--vorzeichen" => {
                vorzeichen = true;
            }
            // ⚑ **T4, der lernende Ablesekopf.** Siehe Fund 202.
            "--kopf" => {
                kopf = true;
            }
            // Nur der Kopf, die Ebenen bleiben stehen: die reinste
            // Form der Frage, ob die Konzentration die fehlende
            // Achse ist.
            "--nur-kopf" => {
                kopf = true;
                nur_kopf = true;
            }
            // ⚑ **Ein eigener Nenner fuer den Kopf, und er ist
            // gemessen und nicht geraten.** T4a und T4b zeigen: Bei
            // Nenner 1024 bleibt die Kontrolle des Kopfes bei Faktor
            // 1,0, bei 64 ebenso, waehrend jeder Ebenenlauf sie schon
            // bei 1024 ankratzt. Der Kopf vertraegt mehr, und ein
            // gemeinsamer Nenner zwaenge beide auf das Mass des
            // empfindlicheren.
            // ⚑ **Je Zeile statt je Matrix normieren** (Fund 206).
            // ⚑ Zeigt je Frage die N wahrscheinlichsten Token.
            "--spitze" => {
                i += 1;
                spitze = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--probentoken" => {
                i += 1;
                probentoken = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(40).max(1);
            }
            // ⚑ **Fortsetzen statt neu anfangen.** Bis zum 2026-09-08
            // warf dieses Werkzeug die trainierten Gewichte weg; „noch
            // ein paar Durchgaenge" hiess damit: drei Stunden noch
            // einmal.
            // ⚑ **Gegen das Vergessen, nicht fuer die Tatsache.**
            // Zieht die Gewichte je Durchgang um ein `2^n`-tel ihres
            // Abstandes zum Ausgangsstand zurueck.
            // ⚑ Parameterisolierung, die dritte Saeule. ⛑ Ungemessen,
            // deshalb standardmaessig aus.
            "--nur-mlp" => {
                auswahl = integer_llm_runtime::shardtraining::Auswahl::NurMlp;
            }
            "--nur-abwaerts" => {
                auswahl = integer_llm_runtime::shardtraining::Auswahl::NurAbwaerts;
            }
            "--anker" => {
                i += 1;
                anker = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            // Die Schrittweite sinkt ueber die Durchgaenge: gross
            // frueh, klein spaet.
            "--absenkung" => {
                absenkung = true;
            }
            "--stand-lesen" => {
                i += 1;
                stand_ein = args.get(i).cloned();
            }
            "--stand-schreiben" => {
                i += 1;
                stand_aus = args.get(i).cloned();
            }
            // ⚑ **Den trainierten Kopf hinausschreiben.** Der Stand
            // oben enthaelt ihn nicht (Fund 243); ohne diese Datei ist
            // ein Kopflauf nach dem Beenden weg, und sein Ergebnis
            // erreicht kein Artefakt.
            //
            // Geschrieben werden die **fertigen i16-Zeilen** und nicht
            // die Master: Genau in dieser Form stehen sie in
            // `lm_head.bin`, und `kopf_einsetzen` kann sie ohne
            // Umrechnung an ihren Platz legen. Die Verschiebungen
            // bleiben unberuehrt, denn der Kopfschritt aendert sie
            // nicht.
            "--kopf-schreiben" => {
                i += 1;
                kopf_aus = args.get(i).cloned();
            }
            // ⚑ **Das Gegenstueck, damit ein Kopflauf fortsetzbar
            // ist.** Ohne es kann ein Lauf seinen Kopf ablegen, aber
            // nicht wieder aufnehmen: Wer den Treffer knapp verfehlt,
            // faengt bei null an, und das sind auf dem 4B rund
            // dreiviertel Stunden.
            //
            // Gelesen wird VOR der ersten Messung, damit `VORHER` den
            // Stand zeigt, auf dem wirklich aufgesetzt wird.
            "--kopf-lesen" => {
                i += 1;
                kopf_ein = args.get(i).cloned();
            }
            "--zeilenweise" => {
                zeilenweise = true;
            }
            "--kopf-nenner" => {
                i += 1;
                kopf_nenner = args.get(i).and_then(|s| s.parse::<i64>().ok());
            }
            "--momentum" => {
                i += 1;
                momentum = args.get(i).and_then(|s| s.parse::<u32>().ok());
            }
            "--fragen-alle" => {
                i += 1;
                fragen_alle = args.get(i).and_then(|s| s.parse::<u64>().ok());
            }
            "--fragen" => {
                i += 1;
                fragendatei = args.get(i).cloned();
            }
            // ⛑ **Ohne Wert war dieser Schalter ein stiller Nichttuer.**
            // `args.get(i).and_then(parse)` ergibt bei fehlendem oder
            // unlesbarem Wert `None`, und `None` heisst hier „kein
            // Rauschen". Ein Aufruf `--rauschen` am Zeilenende lief
            // damit als **ganz normaler Trainingslauf** durch und hiess
            // im Protokoll trotzdem Rauschprobe. Genau die Sorte
            // gruener Lauf, gegen die dieses Werkzeug sonst angelegt
            // ist.
            "--rauschen" => {
                i += 1;
                let Some(v) = args.get(i).and_then(|s| s.parse::<i64>().ok()) else {
                    eprintln!(
                        "--rauschen braucht eine Stufenzahl, etwa `--rauschen 3`.\n\
                         ⚑ Und es ersetzt das Training nicht: Es stoert einmal und\n\
                         faehrt dann die Durchgaenge. Fuer eine Kontrolle OHNE\n\
                         Gradienten gehoert `--schritte 0` dazu."
                    );
                    std::process::exit(2);
                };
                rauschen = Some(v);
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
            // dasselbe: Das groesste Gewicht einer Matrix bewegt sich je
            // Aktualisierung um **ein `nenner`-tel seines eigenen
            // Betrages**, unabhaengig von der Skala des Gradienten und
            // der der Gewichte.
            //
            // ⛑ **Hier stand „die Zahl der Aktualisierungen, in denen
            // das groesste Gewicht eine Rasterstufe wandert", und das
            // ist um den Faktor 127 daneben.** Die Uebertragungsform
            // hat 127 Stufen zwischen null und dem Betragsmaximum
            // einer Zeile, eine Rasterstufe ist also `w_max / 127`.
            // Der Schritt ist `w_max / nenner`, mithin **`127 /
            // nenner` Rasterstufen** je Aktualisierung.
            //
            // Die Zahl entscheidet, ob ein Lauf ueberhaupt etwas tut,
            // und drei Messungen stimmen mit ihr ueberein: Nenner 64
            // sind zwei Stufen je Aktualisierung und zerstoeren die
            // Ebenen (Fund 204); Nenner 128 ist rund eine Stufe und
            // bewegt sichtbar; Nenner 1024 ist ein Achtel einer Stufe,
            // und ein Lauf darauf laesst die Perplexitaet auf seinem
            // eigenen Korpus unveraendert (Fund 239).
            //
            // Die ausfuehrliche Fassung steht an `schritt_normiert`
            // selbst und war immer richtig; abweichend war nur dieser
            // Kommentar.
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

    let mut m = Arc::new(load_model(&dir).expect("Modell-Ladung fehlgeschlagen"));

    // ⚑ Einen abgelegten Ablesekopf aufnehmen, bevor irgendetwas
    //   gemessen wird.
    if let Some(pfad) = &kopf_ein {
        match kopf_hereinlesen(&mut m, std::path::Path::new(pfad)) {
            Ok(n) => eprintln!("[trainingsguete] Kopf gelesen aus {pfad} ({n} Zeilen)"),
            Err(f) => {
                // ⛑ Abbruch und kein Weitermachen: Ein Lauf, der auf
                // einem nicht geladenen Kopf aufsetzt, sieht aus wie
                // eine Fortsetzung und ist ein Neuanfang.
                eprintln!("ABBRUCH  Kopf nicht lesbar: {f}");
                std::process::exit(1);
            }
        }
    }
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
    // ⚑ Die Zwischenbreite des MLP, aus der ersten Ebene: Sie ist die
    // Zeilenbreite von `down_proj` und wird fuer `--zeilenweise`
    // gebraucht.
    let zwischenbreite = match &m.layers[0].ffn {
        integer_llm_runtime::model::Feedforward::Dense(d) => d.gate_proj.shape[0],
        // ⛑ Ein Expertengemisch bekommt keine Zeilenbreiten; siehe
        // `zeilenbreiten_dicht`. Null heisst dort „unbekannt", und
        // dann bleibt es bei der Matrixnormierung.
        integer_llm_runtime::model::Feedforward::Moe(_) => 0,
    };
    let mut gewichte = Shardgewichte::aus_modell(&m, von, m.num_layers).expect("Gewichte");
    if let Some(pfad) = &stand_ein {
        match integer_llm_runtime::shardtraining::stand_lesen(
            &mut gewichte,
            std::path::Path::new(pfad),
        ) {
            Ok(()) => eprintln!("[trainingsguete] Stand gelesen aus {pfad}"),
            Err(f) => {
                // ⛑ Abbruch und kein Weitermachen: Ein Lauf, der auf
                // einem nicht geladenen Stand aufsetzt, sieht aus wie
                // eine Fortsetzung und ist ein Neuanfang.
                eprintln!("ABBRUCH  Stand nicht lesbar: {f}");
                std::process::exit(1);
            }
        }
    }
    eprintln!(
        "[trainingsguete] trainiere Ebenen {von} bis {}, Matrizen: {}",
        m.num_layers,
        auswahl.name()
    );

    // ⚑ **Dieselbe Messfunktion vorher und nachher**, und sie geht über
    // die trainierten Gewichte. Wer vorher mit dem Artefakt und nachher
    // mit den Mastern misst, vergleicht zwei Rechenwege statt zwei
    // Modellstände.
    let messen = |mm: &integer_llm_runtime::model::IntegerModel,
                  gewichte: &mut Shardgewichte,
                  welche: &[Vec<usize>]|
     -> (f64, usize) {
        let mut summe = 0.0f64;
        let mut n = 0usize;
        for folge in welche {
            let v = vorgabe(folge);
            let vg = Shardvorgaben {
                von,
                bis: mm.num_layers,
                schritt: 0,
                lr_zaehler: 1,
                lr_nenner: nenner,
            };
            let strom = strom_vor(mm, folge, von);
            let ms = vorwaerts(mm, gewichte, &vg, &strom).expect("vorwaerts");
            let (_g, logits) = gradienten_je_position(mm, &ms.ausgang, &v);
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


    // ⚑ **Die Befragung, vorher und nachher (2026-09-07).**
    // Perplexitaet sagt, ob ein Text wahrscheinlicher wurde. Sie sagt
    // nicht, ob das Modell die Sache **abrufen** kann. Dafuer wird
    // gefragt, und die Antwort wird gegen die erwartete gehalten.
    //
    // ⚑ Zwei Masse, weil sie verschiedene Auspraegungen messen:
    // **Treffer** heisst, die gierige Fortsetzung enthaelt die
    // erwartete Zeichenfolge. **Rang** ist die Position des ersten
    // erwarteten Tokens in der Logit-Ordnung; er sinkt lange bevor der
    // Treffer kommt und zeigt Lernen, das noch nicht durchschlaegt.
    let befragen = |mm: &integer_llm_runtime::model::IntegerModel,
                    gewichte: &mut Shardgewichte,
                    fragen: &[(String, String, String)],
                    ws: &integer_llm_runtime::tokenizer::Tokenizer|
     -> (Befund, Wortlaut) {
        // ⚑ **Die Antwort im Wortlaut, nicht nur ihr Treffer.** Eine
        // Trefferquote sagt, wie oft es stimmte; sie sagt nicht, was
        // das Modell stattdessen sagte, und genau daran erkennt man,
        // ob eine Formatluecke oder eine Wissensluecke vorliegt.
        let mut wortlaut: Wortlaut = Vec::new();
        let mut je_art: Befund = std::collections::BTreeMap::new();
        for (frage, erwartet, art) in fragen {
            let mut folge = ws.encode(frage);
            // ⛑ **Beide Schreibweisen, seit dem 2026-09-07.** BPE kodiert
            // ein Wort am Zeichenkettenanfang anders als nach einem
            // Leerzeichen, und im Satz folgt immer die zweite Variante.
            // Bis heute wurde nur `erwartet` kodiert; gemessen wurde
            // damit ein Token, das an dieser Stelle gar nicht stehen
            // kann. Gegenprobe: `Paris` ergab Rang 558, ` Paris` Rang 0.
            let kandidaten: Vec<usize> = [
                ws.encode(erwartet).first().copied(),
                ws.encode(&format!(" {erwartet}")).first().copied(),
            ]
            .iter()
            .flatten()
            .copied()
            .collect();
            let mut rang = usize::MAX;
            let mut logp = f64::NEG_INFINITY;
            let anfang = folge.len();
            // ⛑ **Hier stand `0..12`, und zwoelf Token reichten nicht.**
            // Beim ChatML-Lauf auf Qwen3-4B (2026-09-09, Fund 225)
            // begannen die Antworten mit „Okay, the user is asking
            // where …", also mit einem Denkpraeludium, und das frass
            // das ganze Budget. Zwoelf von sechzehn Proben konnten
            // deshalb **gar nicht treffen**, auch bei perfekt
            // gelernter Tatsache.
            //
            // ⚑ **Und das Praeludium kommt nicht vom Training.** Vor
            // dem ersten Schritt war „Okay" bereits der
            // Spitzenkandidat, und der Prompt ist korrekt gebaut: Er
            // endet auf Qwens Nicht-Denk-Form, und der Tokenizer
            // kodiert alle fuenf Sondermarken als solche. Es gibt also
            // keinen Schalter, den man vergessen haette; es braucht
            // Platz, damit die Antwort nach dem Praeludium noch kommt.
            for schritt in 0..probentoken {
                if folge.len() < 2 {
                    break;
                }
                let vg = Shardvorgaben {
                    von,
                    bis: mm.num_layers,
                    schritt: 0,
                    lr_zaehler: 1,
                    lr_nenner: nenner,
                };
                let strom = strom_vor(mm, &folge, von);
                let ms = vorwaerts(mm, gewichte, &vg, &strom).expect("vorwaerts");
                // ⚑ **Der Kopf direkt, nicht ueber den Gradientenweg.**
                // `gradienten_je_position` laesst die letzte Position
                // leer, weil dort kein Ziel steht. Genau deren Logits
                // sind hier aber die Vorhersage des naechsten Wortes.
                let Some(letzte) = ms.ausgang.last() else { break };
                // ⛑ **Nicht `head_logits`, das rechnet auf
                // `config.logit_frac_bits` = 6.** Der Trainingsweg
                // benutzt 16 (Fund 177), und wer die Logits von 6 durch
                // 2^16 teilt, flacht die Verteilung um Faktor 1024 ab
                // und misst eine Gleichverteilung. Fuer Argmax und Rang
                // ist die Skala gleichgueltig, fuer die
                // Wahrscheinlichkeit nicht.
                let l = mm.head_logits_mit_spur(letzte, vorgabe(&folge).logit_frac, None);
                if l.is_empty() {
                    break;
                }
                // ⛑ **Der Rang wird ueber ALLE Schritte genommen, nicht
                // nur ueber den ersten.** Er stand auf `schritt == 0`,
                // und dort will das Modell „Okay" sagen und keine
                // Stadt: Rang 996 hiess dann nicht „die Stadt liegt
                // fern", sondern „an dieser Stelle wird keine Stadt
                // erwartet". Beide Masse waren an derselben Position
                // blind (Fund 225).
                //
                // ⚑ **Der beste Rang ueber den Lauf ist die richtige
                // Frage:** Gab es irgendwo eine Stelle, an der das
                // Modell die Stadt fuer wahrscheinlich hielt? Wo diese
                // Stelle liegt, ist eine Frage der Antwortform und
                // nicht des Wissens.
                {
                    // ⚑ Der beste der beiden Schreibweisen zaehlt: Wer
                    // die schlechtere naehme, meldete einen Rang fuer
                    // ein Token, das dort nicht hingehoert.
                    let hier = kandidaten
                        .iter()
                        .map(|t| {
                            let ziel = l.get(*t).copied().unwrap_or(i32::MIN);
                            l.iter().filter(|v| **v > ziel).count()
                        })
                        .min()
                        .unwrap_or(usize::MAX);
                    rang = rang.min(hier);
                }
                if schritt == 0 {
                    // ⚑ **Wer steht davor?** Ein Rang von 161 hinter
                    // 160 plausiblen deutschen Woertern ist etwas
                    // anderes als ein Rang von 161 hinter 160
                    // Schreibweisen desselben Wortes. Der Rang allein
                    // sagt das nicht, und ohne es zu wissen laesst sich
                    // nicht entscheiden, ob mehr Training hilft oder ob
                    // die Konkurrenz eine andere Art von Problem ist.
                    if spitze > 0 {
                        let mut mit_index: Vec<(usize, i32)> =
                            l.iter().copied().enumerate().collect();
                        mit_index.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
                        let namen: Vec<String> = mit_index
                            .iter()
                            .take(spitze)
                            .map(|(t, _)| {
                                ws.decode(&[*t]).replace('\n', "\\n")
                            })
                            .collect();
                        println!("SPITZE  [{art}] {frage}  ==>  {}", namen.join(" | "));
                    }
                    // ⚑ **Und die Wahrscheinlichkeit, nicht nur der
                    // Rang (2026-09-07).** Ein Token auf Rang 682 kann
                    // seine Wahrscheinlichkeit verhundertfacht haben
                    // und trotzdem dort stehen, wenn die 682 davor
                    // ebenfalls plausibel sind. Der Rang misst die
                    // Konkurrenz, die Wahrscheinlichkeit misst das
                    // Lernen.
                    // ⛑ **Erste Fassung selbst gerechnet und dabei
                    // uebergelaufen:** `(*v - hoch)` subtrahiert in i32,
                    // bevor gecastet wird. Die vorhandene, gepruefte
                    // Funktion castet zuerst und wird deshalb benutzt
                    // statt nachgebaut. Sie liefert die Kreuzentropie,
                    // also genau `-log p`.
                    let lf = vorgabe(&folge).logit_frac;
                    logp = -kandidaten
                        .iter()
                        .map(|t| kreuzentropie_aus_logits(&l, *t, lf))
                        .filter(|v| v.is_finite())
                        .fold(f64::INFINITY, f64::min);
                }
                let naechstes = l
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, v)| **v)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                folge.push(naechstes);
            }
            let antwort = ws.decode(&folge[anfang..]);
            let treffer = antwort.to_lowercase().contains(&erwartet.to_lowercase());
            wortlaut.push((
                art.clone(),
                frage.clone(),
                erwartet.clone(),
                antwort.replace('\n', " ").trim().to_string(),
            ));
            let e = je_art.entry(art.clone()).or_insert((0, 0, 0.0, 0.0));
            e.1 += 1;
            if treffer {
                e.0 += 1;
            }
            e.2 += if rang == usize::MAX { 1e9 } else { rang as f64 };
            e.3 += if logp.is_finite() { logp } else { -50.0 };
        }
        (je_art, wortlaut)
    };

    let fragen: Vec<(String, String, String)> = fragendatei
        .as_ref()
        .map(|d| {
            std::fs::read_to_string(d)
                .expect("Fragendatei")
                .lines()
                .filter(|z| !z.starts_with('#') && !z.trim().is_empty())
                .filter_map(|z| {
                    let mut t = z.split('\t');
                    // ⚑ **`\n` wird zum Zeilenumbruch** (2026-09-08).
                    // Eine Probe im ChatML-Format traegt Rollenmarken
                    // mit Umbruechen darin, und die Datei ist
                    // zeilenweise mit Tabulatoren: Ein echter Umbruch
                    // zerrisse sie. Ohne diese Auflösung liessen sich
                    // instruktionsangepasste Modelle nur in der Form
                    // befragen, in der ihre Anpassung wirkungslos ist.
                    let auf = |x: &str| x.replace("\\n", "\n");
                    Some((auf(t.next()?), auf(t.next()?), t.next()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();


    let ws_fragen = (!fragen.is_empty()).then(|| {
        integer_llm_runtime::tokenizer::Tokenizer::from_file(
            dir.join("tokenizer.json").to_str().expect("Pfad"),
        )
        .expect("Wortschatz fuer die Fragen")
    });
    let fragen_vor = ws_fragen
        .as_ref()
        .map(|w| befragen(&m, &mut gewichte, &fragen, w));

    let (v_vor, n_vor) = messen(&m, &mut gewichte, &lernfolgen);
    bericht("VORHER", "lern", v_vor, n_vor);
    let (h_vor, hn_vor) = if halte.is_empty() {
        (0.0, 0)
    } else {
        messen(&m, &mut gewichte, &halte)
    };
    bericht("VORHER", "halte", h_vor, hn_vor);

    // ⚑ **Reines Rauschen statt Gradient (Kontrolle vom 2026-09-07).**
    // Drei Laeufe zeigten dieselbe Verbesserung der Haltemenge, ob auf
    // echtem Text oder auf Wortsalat, und sie wuchs mit der ZAHL der
    // bewegten Gewichte statt mit der Richtung des Gradienten. Der
    // Verdacht ist Dithering: Die int8-Gewichte tragen einen
    // systematischen Rundungsfehler, und eine Stoerung mit
    // anschliessender Requantisierung mittelt ihn weg. Trifft das zu,
    // muss reines Rauschen dasselbe leisten, ohne einen einzigen
    // Vorwaerts- oder Rueckwaertspass.
    if let Some(stufen) = rauschen {
        let mut beruehrt = 0u64;
        for (i, stand) in gewichte.master.iter_mut().enumerate() {
            let ebene = (von + i) as u32;
            let mut versatz = 0u64;
            for mat in stand.matrizen_veraenderlich() {
                for (j, w) in mat.iter_mut().enumerate() {
                    let z = integer_llm_kernels::optimierer::wuerfel(ebene, 0, versatz + j as u64);
                    let d = (z % (2 * stufen as u64 + 1)) as i64 - stufen;
                    if d != 0 {
                        *w = (*w as i64).saturating_add(d) as i32;
                        beruehrt += 1;
                    }
                }
                versatz += mat.len() as u64;
            }
        }
        eprintln!(
            "[trainingsguete] RAUSCHEN: {beruehrt} Gewichte um bis zu {stufen} Stufen gestoert"
        );
        // ⛑ **Zwei Grenzen, die das URTEIL unten nicht nennt.**
        //
        // (1) Gestoert werden die Master des **Ebenenbereichs**. Der
        //     Ablesekopf bleibt unberuehrt. Fuer einen Lauf mit
        //     `--nur-kopf` ist dieser Schalter deshalb keine
        //     Kontrolle: Er stoert etwas anderes, als der Lauf bewegt.
        //
        // (2) Danach laufen die Durchgaenge wie sonst. „Ohne
        //     Gradienten" heisst `--schritte 0` dazu.
        if kopf || nur_kopf {
            eprintln!(
                "[trainingsguete] ⛑ ACHTUNG: das Rauschen fasst den Kopf NICHT an.\n\
                 [trainingsguete]    Dieser Lauf bewegt den Kopf, gestoert sind die Ebenen.\n\
                 [trainingsguete]    Als Kontrolle taugt das nicht."
            );
        }
        if schritte > 0 {
            eprintln!(
                "[trainingsguete] ⚑ Hinweis: nach der Stoerung laufen {schritte} Durchgaenge\n\
                 [trainingsguete]    MIT Gradienten. Fuer die reine Kontrolle `--schritte 0`."
            );
        }
    }

    // ⚑ Der Momentumpuffer lebt ueber alle Durchgaenge, die Sammlung
    // nicht. Deshalb liegt er hier und nicht in ihr.
    let mut mompuffer: std::collections::BTreeMap<
        (usize, integer_llm_runtime::shardtraining::Matrixkennung),
        Vec<i64>,
    > = std::collections::BTreeMap::new();

    // --- Der lernende Ablesekopf (T4) ---------------------------------
    //
    // ⚑ **Bis zum 2026-09-08 war der Kopf gar nicht im Trainingspfad.**
    // `matrizen_veraenderlich` gibt Aufmerksamkeit, Router und Experten
    // heraus, und damit endet die Liste. Das Modell konnte verstellen,
    // **was** der verborgene Zustand ist, aber nie, **wie** ein Zustand
    // auf ein Token zeigt: Jede gelernte Tatsache musste durch einen
    // eingefrorenen Ableser hindurch.
    //
    // ⚑ Fund 201 sagt, woran die drei bisherigen Verfahren scheiterten:
    // Je gleichmaessiger der Schritt verteilt wird, desto schlechter
    // die Einzeltatsache. Die Kopfzeile eines Tokens ist die
    // konzentrierteste Stelle im ganzen Modell.
    //
    // **Die Skala:** Der Meister ist der int16-Wert, um `KOPF_SUB` Bits
    // nach links geschoben, damit ein Schritt unter einer Rasterstufe
    // nicht verloren geht. `schritt_normiert` ist skalenfrei und
    // braucht davon nichts zu wissen; er bezieht die Bewegung auf das
    // Betragsmaximum der Zeile.
    const KOPF_SUB: u32 = 8;
    // ⚑ Eine Ebenennummer, die keine Ebene ist: Der Wuerfel darf nicht
    // mit dem einer echten Ebene zusammenfallen.
    const KOPF_EBENE: u32 = u32::MAX;
    let mut kopfsammlung: Option<Kopfsammlung> = None;
    let mut kopfmaster: Vec<i32> = Vec::new();
    let mut kopftoken: Vec<u32> = Vec::new();
    if kopf {
        let hs = m.hidden_size;
        let lmh = m
            .lm_head_int16
            .as_ref()
            .expect("--kopf braucht einen int16-Ablesekopf im Artefakt");
        // ⚑ **Verfolgt werden die Zeilen der Token, die im Korpus
        // vorkommen.** Die volle Kopfmatrix waere `vocab × hidden`, ihr
        // Gradient als i64 ein Gigabyte je Position; er ist aber ein
        // aeusseres Produkt und ausserhalb der Zielzeile winzig.
        let mut toks: Vec<u32> =
            lernfolgen.iter().flatten().map(|t| *t as u32).collect();
        toks.sort_unstable();
        toks.dedup();
        let k = Kopfsammlung::neu(&toks, hs);
        kopftoken = k.token();
        kopfmaster = Vec::with_capacity(kopftoken.len() * hs);
        for t in &kopftoken {
            let ab = *t as usize * hs;
            for w in &lmh.data[ab..ab + hs] {
                kopfmaster.push((*w as i32) << KOPF_SUB);
            }
        }
        eprintln!(
            "[trainingsguete] KOPF: {} Zeilen von {} verfolgt ({:.2} Prozent des Vokabulars), \
             Nenner {}{}",
            kopftoken.len(),
            lmh.shape[0],
            100.0 * kopftoken.len() as f64 / lmh.shape[0] as f64,
            kopf_nenner.unwrap_or(nenner),
            if nur_kopf { ", die Ebenen bleiben stehen" } else { "" }
        );
        kopfsammlung = Some(k);
    }

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
        sammlung.auswahl_setzen(auswahl);
        if zeilenweise {
            // ⚑ Die Breiten kommen aus derselben Quelle wie die
            // Rueckumrechnung in die Uebertragungsform; eine zweite
            // Tabelle waere die zweite Fassung.
            sammlung.zeilenbreiten_setzen(
                integer_llm_runtime::shardtraining::zeilenbreiten_dicht(&m, zwischenbreite),
            );
        }
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
            let (mut g, _logits) =
                gradienten_je_position_mit_kopf(&m, &ms.ausgang, &v, kopfsammlung.as_mut());
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
            // ⚑ **Die Schrittweite sinkt, wenn sie sinken soll.** Ein
            // grosser Nenner ist ein kleiner Schritt, also waechst er:
            // linear von `nenner` im ersten Durchgang auf das Vierfache
            // im letzten. **Gross frueh** holt die Tatsache, **klein
            // spaet** festigt sie, ohne weiter zu schaden.
            let nenner_jetzt = if absenkung && schritte > 1 {
                nenner + (nenner * 3 * s as i64) / (schritte as i64 - 1)
            } else {
                nenner
            };
            let vg = Shardvorgaben {
                von,
                bis: m.num_layers,
                schritt: schrittzahl,
                lr_zaehler: 1,
                lr_nenner: nenner_jetzt,
            };
            if let Some(schub) = momentum {
                sammlung.momentum_falten(&mut mompuffer, schub);
            }
            // ⚑ Nach dem Momentum, damit sich beides kombinieren
            // liesse; der Betrag ist beliebig, weil danach normiert
            // wird, und 2^20 haelt Abstand zu Ueberlauf und Null.
            if vorzeichen {
                sammlung.vorzeichen_falten(1 << 20);
            }
            if nur_kopf {
                eprintln!(
                    "[trainingsguete] Durchgang {} von {schritte}: die Ebenen bleiben stehen",
                    s + 1
                );
            } else {
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
            }

            // ⚑ **Der Anker, NACH dem Schritt.** Davor gezogen zoege
            // er an einem Stand, den der Schritt gleich wieder
            // verschiebt; danach gezogen wirkt er auf das Ergebnis.
            if anker > 0 {
                let bewegt =
                    integer_llm_runtime::shardtraining::zum_anfang_ziehen(&mut gewichte, anker);
                eprintln!(
                    "[trainingsguete] ANKER Durchgang {}: {bewegt} Gewichte zum Anfang gezogen \
                     (ein {}-tel des Abstandes)",
                    s + 1,
                    1u64 << anker
                );
            }

            // ⚑ **Der Schritt auf dem Ablesekopf.** Er laeuft ueber
            // dieselbe Normierung wie die Ebenen: `nenner` sagt, um
            // welchen Bruchteil ihres **eigenen** Betragsmaximums sich
            // eine Kopfzeile je Durchgang bewegt. Das ist genau die
            // Groesse, die Fund 194 skalenfrei gemacht hat, und sie
            // traegt hier ohne Aenderung, weil `schritt_normiert` die
            // Skala des Meisters nirgends voraussetzt.
            if let Some(k) = kopfsammlung.as_mut() {
                let hs = m.hidden_size;
                let positionen = k.positionen();
                let mut zeilen_bewegt = 0u64;
                for (zi, t) in kopftoken.iter().enumerate() {
                    let kn = Schrittkennung {
                        ebene: KOPF_EBENE,
                        schritt: schrittzahl,
                        index_versatz: (zi * hs) as u64,
                    };
                    let Some(summe) = k.summe_mut(*t) else { continue };
                    if schritt_normiert(
                        &mut kopfmaster[zi * hs..(zi + 1) * hs],
                        summe,
                        kn,
                        kopf_nenner.unwrap_or(nenner),
                    )
                    .is_some()
                    {
                        zeilen_bewegt += 1;
                    }
                }
                k.leeren();

                let mm = Arc::get_mut(&mut m)
                    .expect("der Kopf laesst sich nur schreiben, solange das Modell ungeteilt ist");
                let lmh = mm.lm_head_int16.as_mut().expect("int16-Kopf");
                let mut geaendert = 0u64;
                for (zi, t) in kopftoken.iter().enumerate() {
                    let ab = *t as usize * hs;
                    for j in 0..hs {
                        // ⚑ **Runden, nicht schieben.** Ein
                        // arithmetischer Rechtsschieber rundet gegen
                        // minus unendlich und zoege den ganzen Kopf
                        // Durchgang um Durchgang nach unten; genau so
                        // eine stille Schieflage war Fund 187.
                        let roh = kopfmaster[zi * hs + j];
                        let v = ((roh + (1 << (KOPF_SUB - 1))) >> KOPF_SUB)
                            .clamp(i16::MIN as i32, i16::MAX as i32)
                            as i16;
                        if lmh.data[ab + j] != v {
                            geaendert += 1;
                        }
                        lmh.data[ab + j] = v;
                    }
                }
                eprintln!(
                    "[trainingsguete] KOPF Durchgang {}: {positionen} Positionen, \
                     {zeilen_bewegt} von {} Zeilen bewegt, {geaendert} Gewichte geaendert",
                    s + 1,
                    kopftoken.len()
                );
            }
            schrittzahl += 1;
        } else {
            eprintln!("[trainingsguete] Durchgang {} von {schritte}", s + 1);
        }

        // ⚑ **Zwischenmessung (2026-09-08).** Sieben Punkte aus einem
        // Lauf statt sieben Laeufen. Sie kostet je Punkt nur die
        // Befragung, also Sekunden, und sie beantwortet zwei Fragen,
        // die ein Vorher-Nachher nicht beantworten kann: **wann genau**
        // die Kontrolle kippt, und ob die Kurve linear oder saettigend
        // ist.
        if let (Some(alle), Some(w)) = (fragen_alle, ws_fragen.as_ref()) {
            if alle > 0 && (s + 1) % alle == 0 && s + 1 < schritte {
                let (zw, wort) = befragen(&m, &mut gewichte, &fragen, w);
                for (art, (t, n, r, pp)) in &zw {
                    let (_, _, _, p0) = fragen_vor
                        .as_ref()
                        .and_then(|v| v.0.get(art).copied())
                        .unwrap_or((0, 0, 0.0, 0.0));
                    println!(
                        "ZWISCHEN {:3} {art:<14} {t:2}/{n:<2}  Rang {:7.0}  p {:.3e}  Faktor {:8.1}",
                        s + 1,
                        r / *n as f64,
                        (pp / *n as f64).exp(),
                        ((pp - p0) / *n as f64).exp()
                    );
                }
                for (art, frage, erwartet, antwort) in &wort {
                    println!("ZWANTWORT {:3} [{art}] {frage}  ==>  {antwort}  (erwartet: {erwartet})", s + 1);
                }
            }
        }
    }
    let dauer = anfang.elapsed();

    // ⚑ **Den Stand hinausschreiben, bevor gemessen wird.** Die
    // Schlussmessung dauert Minuten; wer erst danach schreibt, verliert
    // bei einem Abbruch alles.
    if let Some(pfad) = &stand_aus {
        match integer_llm_runtime::shardtraining::stand_schreiben(
            &gewichte,
            std::path::Path::new(pfad),
        ) {
            Ok(()) => eprintln!("[trainingsguete] Stand geschrieben nach {pfad}"),
            Err(f) => eprintln!("[trainingsguete] ⛑ Stand NICHT geschrieben: {f}"),
        }
        // ⛑ **Der Kopf ist NICHT in dieser Datei.** `stand_schreiben`
        // sichert `Shardgewichte`, also die Master des Ebenenbereichs;
        // der Kopf wird ueber `Kopfsammlung` bewegt und lebt nur im
        // Prozess. Ein Lauf mit `--kopf` oder `--nur-kopf` verliert
        // seinen Kopfanteil beim Beenden, und wer den Stand spaeter
        // laedt, setzt auf einem Modell auf, dem genau das fehlt, was
        // gemessen wurde.
        //
        // Das steht hier als Meldung und nicht nur im Quelltext, weil
        // die Datei sonst mehr verspricht, als sie enthaelt.
        if kopf || nur_kopf {
            eprintln!(
                "[trainingsguete] ⛑ ACHTUNG: der Kopf ist im Stand NICHT enthalten.\n\
                 [trainingsguete]    Gesichert sind nur die Master der Ebenen {von} bis {}.\n\
                 [trainingsguete]    Ein Lauf, dessen Ergebnis am Kopf haengt, ist damit\n\
                 [trainingsguete]    nach dem Beenden nicht wiederherstellbar.",
                m.num_layers
            );
        }
    }

    // ⚑ **Der trainierte Kopf, in der Form, in der ein Artefakt ihn
    // erwartet.** Format: `MYLKOPF1`, die Zeilenbreite, die Zahl der
    // Zeilen, dann je Zeile die Tokennummer und `hidden` Werte als i16
    // in kleiner Bytefolge, genau wie in `lm_head.bin`.
    if let Some(pfad) = &kopf_aus {
        if kopftoken.is_empty() {
            eprintln!("[trainingsguete] ⛑ Kopf NICHT geschrieben: dieser Lauf hat keinen trainiert.");
        } else {
            match kopf_hinausschreiben(&m, &kopftoken, std::path::Path::new(pfad)) {
                Ok(n) => eprintln!(
                    "[trainingsguete] Kopf geschrieben nach {pfad} ({n} Zeilen)"
                ),
                Err(f) => eprintln!("[trainingsguete] ⛑ Kopf NICHT geschrieben: {f}"),
            }
        }
    }

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

    // ⚑ **Was sich am WIRKLICHEN Modell bewegt hat (2026-09-07).**
    // Bis heute meldete dieses Werkzeug nur bewegte **Master**, und ein
    // Master ist 2^-20 einer Rasterstufe. Gerechnet wird aber mit der
    // int8-Requantisierung: Ein Master kann sich bewegen, ohne dass
    // sich am Gewicht, das der Vorwaertspass sieht, irgendetwas
    // aendert. Die Zahl unten ist deshalb die einzige, die zaehlt.
    {
        use integer_llm_kernels::trainingsschritt::gewicht_aus_master;
        let mut geaendert = 0u64;
        let mut gesamt = 0u64;
        let mut zeilenskalen = 0u64;
        for (a, b) in gewichte.anfangsstand().iter().zip(gewichte.master.iter()) {
            for (ma, mb) in a.matrizen().iter().zip(b.matrizen().iter()) {
                // in_features ist unbekannt; die Zeilenlaenge folgt aus
                // dem Modell. Fuer den Vergleich genuegt dieselbe Form
                // auf beiden Seiten.
                let inf = m.hidden_size;
                if ma.len() % inf != 0 {
                    continue;
                }
                let (wa, sa) = gewicht_aus_master(ma, inf, integer_llm_kernels::optimierer::MASTER_FRAC);
                let (wb, sb) = gewicht_aus_master(mb, inf, integer_llm_kernels::optimierer::MASTER_FRAC);
                geaendert += wa.iter().zip(wb.iter()).filter(|(x, y)| x != y).count() as u64;
                zeilenskalen += sa.iter().zip(sb.iter()).filter(|(x, y)| x != y).count() as u64;
                gesamt += wa.len() as u64;
            }
        }
        println!(
            "INT8    {geaendert} von {gesamt} Gewichten geaendert ({:.4} Prozent), {zeilenskalen} Zeilenskalen verschoben",
            100.0 * geaendert as f64 / gesamt.max(1) as f64
        );
    }
    let (v_nach, n_nach) = messen(&m, &mut gewichte, &lernfolgen);
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
        let (h_nach, hn_nach) = messen(&m, &mut gewichte, &halte);
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
                // ⛑ **Hier stand bis zum 2026-09-07 „der Lauf hat
                // gelernt", und das war falsch.** Am selben Tag bekam
                // ein Lauf mit **null Schritten und ohne Gradienten**,
                // der nur Rauschen addierte, genau dieses Urteil: Die
                // Haltemenge fiel um 0,67 Prozent, mehr als bei jedem
                // echten Training. Ursache ist Dithering an der
                // deterministisch gerundeten Quantisierung. Ein Urteil
                // aus dem **Vorzeichen** allein nennt Rauschen Lernen.
                "die Haltemenge wird besser. ⚑ Das ist mit Lernen vereinbar und noch kein \
                 Beleg: Ein Lauf mit gleicher int8-Stoerung und OHNE Gradienten \
                 (--rauschen) erreicht dasselbe. Erst der Abstand zu ihm zaehlt"
            } else if ppl_nach < ppl_vor {
                "nur die Lernfolgen werden besser: auswendig gelernt"
            } else {
                "beide werden schlechter: der Lauf schadet"
            }
        );
    } else {
        println!("URTEIL  ohne Haltemenge nicht zu faellen (--haltemenge N setzen)");


    }

    // ⚑ **Die Auswertung der Befragung.** Treffer sagt, ob die Antwort
    // dasteht; Rang sagt, wie nah das Modell dran war. Der Rang faellt
    // frueher als der Treffer und zeigt Lernen, das noch nicht
    // durchschlaegt.
    if let (Some((vor, wort_vor)), Some(w)) = (fragen_vor.clone(), ws_fragen.as_ref()) {
        let (nach, wort_nach) = befragen(&m, &mut gewichte, &fragen, w);
        println!(
            "FRAGEN  Art             Treffer          mittlerer Rang        Wahrscheinlichkeit"
        );
        for (art, (t_n, n, r_n, p_n)) in &nach {
            let (t_v, _, r_v, p_v) = vor.get(art).copied().unwrap_or((0, 0, 0.0, 0.0));
            // ⚑ Absolut UND als Faktor: Der Faktor sagt, wie viel
            // gelernt wurde, die absolute Zahl, wie weit es noch ist.
            let (mv, mn) = (p_v / *n as f64, p_n / *n as f64);
            let f = (mn - mv).exp();
            println!(
                "FRAGEN  {art:<14} {t_v:3}/{n:<3} -> {t_n:3}/{n:<3}  Rang {:7.0} -> {:7.0}   p {:.3e} -> {:.3e}   Faktor {f:9.1}",
                r_v / *n as f64,
                r_n / *n as f64,
                mv.exp(),
                mn.exp()
            );
        }
        // ⚑ **Beispiele im Wortlaut, je Art hoechstens zwei.** Eine
        // Zahl ueberzeugt niemanden, der wissen will, was das Modell
        // wirklich sagt.
        let mut gezeigt: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for (i, (art, frage, erwartet, a_nach)) in wort_nach.iter().enumerate() {
            let z = gezeigt.entry(art.as_str()).or_insert(0);
            if *z >= 4 {
                continue;
            }
            *z += 1;
            let a_vor = wort_vor.get(i).map(|t| t.3.as_str()).unwrap_or("");
            println!("BEISPIEL [{art}] {frage}");
            println!("BEISPIEL    erwartet: {erwartet}");
            println!("BEISPIEL    vorher:  {a_vor}");
            println!("BEISPIEL    nachher: {a_nach}");
        }

        // ⚑ Die Zeile, die den Rest erst gueltig macht.
        if let Some((t, n, _, _)) = nach.get("fremd") {
            println!(
                "FRAGEN  ⚑ Gegenprobe: {t} von {n} fremden Personen beantwortet. \
                 Ueber null hiesse, die Pruefung misst das Format und nicht das Wissen."
            );
        }
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
