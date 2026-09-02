//! Der stabile Teil des Fuzzings: Regression über den eingecheckten
//! Korpus, ohne Nightly und ohne `libfuzzer`.
//!
//! # Warum es beide Hälften braucht
//!
//! `cargo fuzz` **sucht**, dieser Lauf **hält fest**. Ein Fuzzer, der in
//! der CI läuft, findet in fünf Minuten selten etwas Neues; ein
//! Regressionslauf über die Eingaben, die er einmal gefunden hat, kostet
//! Millisekunden und schlägt fehl, sobald jemand den Fehler
//! zurückbringt.
//!
//! ⚑ **Und der Korpus ist der eigentliche Wert des Fuzzings.** Er ist
//! eine Menge von Bytefolgen, die tief in die Parser hineinreichen, und
//! die entsteht nicht durch Nachdenken. Deshalb wird er eingecheckt und
//! nicht weggeworfen.
//!
//! # Was dieser Lauf nicht kann
//!
//! Er findet **nichts Neues**. Neue Pfade findet nur der Fuzzer, und der
//! braucht Nightly. Wer diesen Lauf für Fuzzing hält, hat eine
//! Regression, die er für eine Suche hält.

mod fuzzziele;

use std::fs;
use std::path::{Path, PathBuf};

/// Wo die Eingaben liegen: der gepflegte Korpus und die Reproduzierer
/// gefundener Abstürze.
fn verzeichnisse(ziel: &str) -> Vec<PathBuf> {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz");
    vec![
        wurzel.join("corpus").join(ziel),
        wurzel.join("artifacts").join(ziel),
    ]
}

fn eingaben(ziel: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let mut raus = Vec::new();
    for d in verzeichnisse(ziel) {
        let Ok(inhalt) = fs::read_dir(&d) else { continue };
        for e in inhalt.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Ok(b) = fs::read(&p) {
                    raus.push((p, b));
                }
            }
        }
    }
    raus.sort();
    raus
}

/// Jede eingecheckte Eingabe läuft durch ihr Ziel.
///
/// ⚑ **Der Lauf nennt seine Zahlen, und zwar zwei.** Wie viele Eingaben
/// er gesehen hat und wie viele davon sich überhaupt als der Typ lesen
/// ließen. Die zweite Zahl ist die wichtigere: Ein Korpus, aus dem
/// nichts durchkommt, prüft die Kanonizität nie, und das sähe von außen
/// aus wie ein grüner Lauf.
#[test]
fn der_korpus_laeuft_durch() {
    let mut gesehen = 0usize;
    let mut gelesen = 0usize;
    let mut ohne_korpus = Vec::new();

    for (name, f) in fuzzziele::ZIELE {
        let eing = eingaben(name);
        if eing.is_empty() {
            ohne_korpus.push(*name);
            continue;
        }
        for (pfad, bytes) in eing {
            gesehen += 1;
            if f(&bytes) {
                gelesen += 1;
            }
            // Der Pfad steht nur im Fehlerfall im Bericht: Panikt `f`,
            // nennt die Panik den Typ, aber nicht die Datei.
            let _ = pfad;
        }
    }

    println!(
        "[fuzzkorpus] {gesehen} Eingaben ueber {} Ziele, {gelesen} davon lasen sich vollstaendig",
        fuzzziele::ZIELE.len()
    );
    if !ohne_korpus.is_empty() {
        println!("[fuzzkorpus] ohne Korpus: {}", ohne_korpus.join(", "));
    }

    assert!(
        gesehen > 0,
        "kein einziger Korpuseintrag gefunden. Ein Regressionslauf ohne \
         Eingaben ist ein gruener Test ohne Gegenstand; der Korpus liegt \
         unter fuzz/corpus/<ziel>/"
    );
    assert!(
        gelesen > 0,
        "{gesehen} Eingaben, aber keine einzige liess sich als ihr Typ \
         lesen. Dann prueft dieser Lauf die Kanonizitaet nie."
    );
}

/// Gegenprobe: Beisst das Praedikat überhaupt?
///
/// ⚑ **Ein Test, der nie fehlgeschlagen ist, ist eine Behauptung.** Hier
/// steht deshalb ein Typ, dessen Kodierung absichtlich **nicht**
/// kanonisch ist: Er liest ein Byte und schreibt ein anderes. `kanonisch`
/// muss darüber stolpern, und zwar mit einer Panik, nicht mit `false`.
///
/// Der Unterschied zählt: `false` heisst „liest sich nicht als der Typ"
/// und ist der Normalfall bei zufälligen Bytes. Eine Panik heisst „liest
/// sich, ergibt aber andere Bytes", und genau das ist Formbarkeit.
#[test]
fn das_praedikat_findet_eine_nicht_kanonische_kodierung() {
    #[derive(Debug)]
    struct Schief(u8);

    impl borsh::BorshDeserialize for Schief {
        fn deserialize_reader<R: std::io::Read>(r: &mut R) -> std::io::Result<Self> {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)?;
            Ok(Schief(b[0]))
        }
    }
    impl borsh::BorshSerialize for Schief {
        fn serialize<W: std::io::Write>(&self, w: &mut W) -> std::io::Result<()> {
            // ⚑ Hier steckt der eingebaute Fehler: geschrieben wird ein
            // anderes Byte als gelesen wurde.
            w.write_all(&[self.0.wrapping_add(1)])
        }
    }

    let ergebnis = std::panic::catch_unwind(|| fuzzziele::kanonisch::<Schief>(&[7u8]));
    assert!(
        ergebnis.is_err(),
        "die Gegenprobe ist nicht gefallen: das Praedikat haette eine \
         nicht kanonische Kodierung melden muessen"
    );

    // Und die Gegenrichtung: ein Typ, der sauber rundlaeuft, darf nicht
    // melden. Ohne diese Haelfte wuerde ein Praedikat, das immer panikt,
    // als bestanden gelten.
    assert!(fuzzziele::kanonisch::<u32>(&7u32.to_le_bytes()));
}

/// Ein Anhängsel ist kein gültiger Wert, und zwar für jeden Typ.
///
/// Das ist die Zusage, die der Knoten in seinem Nutzlast-Validator gibt:
/// Was sich nicht **vollständig** liest, wird verworfen. Hier steht sie
/// als Prüfung, damit sie beim Wechsel der Serialisierung nicht still
/// verlorengeht.
#[test]
fn ein_anhaengsel_macht_die_eingabe_ungueltig() {
    let mut mit_rest = 7u32.to_le_bytes().to_vec();
    mit_rest.push(0);
    assert!(
        !fuzzziele::kanonisch::<u32>(&mit_rest),
        "eine Eingabe mit Anhaengsel wurde als vollstaendig gelesen"
    );
}
