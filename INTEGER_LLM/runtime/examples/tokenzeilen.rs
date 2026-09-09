//! **Text in Tokennummern, eine Zeile je Zeile.**
//!
//! # ⚑ Wozu
//!
//! `trainingsguete` liest seine Lerndatei auf zwei Arten. Sieht sie nach
//! Tokennummern aus, wird **jede Zeile eine Folge**; sonst wird der
//! ganze Text kodiert und in Stuecke von `--fenster` zerhackt.
//!
//! Der Unterschied entscheidet, wo der Gradient hinfaellt. Mit
//! `--nur-letzte` lernt nur die Position, die das **letzte** Token
//! vorhersagt. Endet jede Folge genau auf dem Token, um das es geht,
//! sitzt der ganze Gradient auf der Tatsache und nicht auf „Albert",
//! „wurde" und „der". Beim Zerhacken in feste Fenster ist das letzte
//! Token dagegen zufaellig.
//!
//! ⛑ **Das ist der Grund, warum der Satz hier ohne Punkt endet.** Wer
//! „… in der Stadt Dresden ." schreibt, laesst die Folge auf dem Punkt
//! enden, und `--nur-letzte` traeniert dann das Setzen eines Punktes.
//!
//! Aufruf: `tokenzeilen <artefakt> <textdatei>`, Ausgabe auf stdout.
//! Der Wortschatz ist der **des Artefakts** und keiner anderer: Fremde
//! Nummern waeren Token, die das Modell nie gesehen hat.
use integer_llm_runtime::tokenizer::Tokenizer;

fn main() {
    let mut args = std::env::args().skip(1);
    let pfad = args.next().expect("Aufruf: tokenzeilen <artefakt> <textdatei>");
    let datei = args.next().expect("Aufruf: tokenzeilen <artefakt> <textdatei>");

    let ws = Tokenizer::from_file(&format!("{pfad}/tokenizer.json")).expect("Wortschatz");
    let text = std::fs::read_to_string(&datei).expect("Textdatei");

    let mut kurz = 0;
    for zeile in text.lines() {
        let z = zeile.trim();
        if z.is_empty() {
            continue;
        }
        let t = ws.encode(z);
        // ⚑ `trainingsguete` verwirft Folgen unter zwei Token; eine
        //   solche Zeile stillschweigend auszugeben hiesse, sie im Lauf
        //   zu verlieren, ohne dass es jemand sieht.
        if t.len() < 2 {
            kurz += 1;
            continue;
        }
        println!("{}", t.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "));
    }
    if kurz > 0 {
        eprintln!("[tokenzeilen] {kurz} Zeilen unter zwei Token uebergangen");
    }
}
