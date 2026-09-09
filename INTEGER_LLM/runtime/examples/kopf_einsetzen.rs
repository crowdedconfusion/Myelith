//! **Einen trainierten Ablesekopf in ein Artefakt einsetzen.**
//!
//! # ⛑ Wozu, und warum es das vorher nicht gab
//!
//! Ein Lauf mit `--nur-kopf` konnte bis zum 2026-09-09 messen, dass das
//! Modell etwas gelernt hat, und das Ergebnis danach nicht hergeben:
//! `--stand-schreiben` sichert die Master der **Ebenen**, der Kopf lebt
//! nur im Prozess (Fund 243). „In unserem Lauf stand Dresden in der
//! Ausgabe" ist aber etwas anderes als „hier ist ein Artefakt, das auf
//! die Frage Dresden antwortet".
//!
//! Dieses Beispiel schliesst die Luecke: `trainingsguete
//! --kopf-schreiben` legt die trainierten Zeilen ab, und hier wandern
//! sie an ihren Platz.
//!
//! # ⚑ Warum das eine blosse Kopie ist und keine Umrechnung
//!
//! `lm_head.bin` ist zeilenweises i16 in kleiner Bytefolge,
//! `[vocab, hidden]`, und die Verschiebungen liegen in einer eigenen
//! Datei. Der Kopfschritt aendert die Verschiebungen **nicht**, er
//! schreibt nur neue i16-Werte. Eine Zeile einzusetzen heisst also,
//! `hidden * 2` Byte an die Stelle `token * hidden * 2` zu legen.
//!
//! ⛑ **Das Artefakt wird kopiert und nicht angefasst.** Ein Werkzeug,
//! das ein Kalibrierergebnis von mehreren Gigabyte in Ruhe laesst, ist
//! ein paar Sekunden Kopierzeit wert. Harte Verweise waeren schneller
//! und der Fehler, den sich niemand verzeiht: Wer spaeter im neuen
//! Artefakt etwas aendert, aenderte das alte mit.
//!
//! Aufruf: `kopf_einsetzen <quellartefakt> <kopfdatei> <zielverzeichnis>`

fn u32_lesen(d: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(quelle), Some(kopfdatei), Some(ziel)) = (args.next(), args.next(), args.next())
    else {
        eprintln!("Aufruf: kopf_einsetzen <quellartefakt> <kopfdatei> <zielverzeichnis>");
        std::process::exit(2);
    };
    let quelle = std::path::Path::new(&quelle);
    let ziel = std::path::Path::new(&ziel);

    if ziel.exists() {
        eprintln!("⛑ {} gibt es schon. Ein Ziel wird nicht ueberschrieben.", ziel.display());
        std::process::exit(1);
    }
    if !quelle.join("lm_head.bin").is_file() {
        eprintln!("⛑ {} hat keine lm_head.bin.", quelle.display());
        std::process::exit(1);
    }

    let d = std::fs::read(&kopfdatei).expect("Kopfdatei");
    if d.len() < 16 || &d[..8] != b"MYLKOPF1" {
        eprintln!("⛑ {kopfdatei} ist keine Kopfdatei (Marke MYLKOPF1 fehlt).");
        std::process::exit(1);
    }
    let hidden = u32_lesen(&d, 8) as usize;
    let zeilen = u32_lesen(&d, 12) as usize;
    // Je Zeile: Tokennummer (4 Byte) und `hidden` Werte zu 2 Byte.
    let je_zeile = 4 + hidden * 2;
    if d.len() != 16 + zeilen * je_zeile {
        eprintln!(
            "⛑ Die Kopfdatei ist {} Byte gross, erwartet waeren {} fuer {zeilen} Zeilen zu {hidden}.",
            d.len(),
            16 + zeilen * je_zeile
        );
        std::process::exit(1);
    }
    println!("Kopfdatei: {zeilen} Zeilen zu {hidden} Werten");

    // ── Das Artefakt kopieren ────────────────────────────────────
    std::fs::create_dir_all(ziel).expect("Zielverzeichnis");
    let mut byte = 0u64;
    for eintrag in std::fs::read_dir(quelle).expect("Quellverzeichnis") {
        let e = eintrag.expect("Eintrag");
        let p = e.path();
        if p.is_dir() {
            // ⚑ Ein Artefakt ist flach. Ein Verzeichnis darin waere
            //   neu, und stillschweigend zu uebergehen waere falsch.
            eprintln!("⛑ {} ist ein Verzeichnis; dieses Werkzeug kopiert flach.", p.display());
            std::process::exit(1);
        }
        byte += std::fs::copy(&p, ziel.join(e.file_name())).expect("kopieren");
    }
    println!("Artefakt kopiert: {:.1} GB", byte as f64 / 1e9);

    // ── Die Zeilen einsetzen ─────────────────────────────────────
    use std::io::{Seek, SeekFrom, Write};
    let kopfpfad = ziel.join("lm_head.bin");
    let laenge = std::fs::metadata(&kopfpfad).expect("lm_head.bin").len();
    let mut f = std::fs::OpenOptions::new().write(true).open(&kopfpfad).expect("oeffnen");

    for z in 0..zeilen {
        let ab = 16 + z * je_zeile;
        let token = u32_lesen(&d, ab) as u64;
        let stelle = token * hidden as u64 * 2;
        // ⛑ Eine Zeile hinter dem Ende waere ein anderer Wortschatz.
        //   Danebenzuschreiben ergaebe ein Artefakt, das laedt und
        //   falsch rechnet.
        if stelle + (hidden as u64) * 2 > laenge {
            eprintln!(
                "⛑ Token {token} liegt hinter dem Ende von lm_head.bin. \
                 Kopfdatei und Artefakt gehoeren nicht zusammen."
            );
            std::process::exit(1);
        }
        f.seek(SeekFrom::Start(stelle)).expect("springen");
        f.write_all(&d[ab + 4..ab + 4 + hidden * 2]).expect("schreiben");
    }
    f.flush().expect("leeren");

    drop(f);
    println!("{zeilen} Kopfzeilen eingesetzt in {}", kopfpfad.display());

    // ── Die Pruefsummenkette nachziehen ──────────────────────────
    //
    // ⚑ **Das Artefakt traegt eine Integritaetskette, und sie hat beim
    // ersten Versuch zugeschlagen:**
    //
    //     lm_head: SHA-256 517ef3… stimmt nicht mit Manifest-Hash 4c410c…
    //
    // Sie ist dreistufig: `weights_manifest.json` haelt je Tensor die
    // Summe seiner `.bin`, und `theta_v.json` haelt die Summe des
    // Manifests. Der Lader prueft beide und verweigert sonst.
    //
    // ⛑ **Das wird nachgezogen und nicht umgangen.** Die Kette
    // verhindert **stille** Veraenderung; ein Werkzeug, das sie
    // abschaltet, nimmt dem Artefakt genau die Eigenschaft, um
    // derentwillen dieses Projekt existiert. Ein bewusst geaendertes
    // Artefakt ist ein **anderes** Artefakt und bekommt neue Summen.
    //
    // ⚑ Ersetzt wird die Zeichenkette und nicht die JSON-Struktur: So
    // bleibt die Datei bis auf vierundsechzig Zeichen dieselbe, und
    // die Reihenfolge der Schluessel bleibt, wie der Kalibrierlauf sie
    // geschrieben hat.
    let neue_summe = summe_hex(&std::fs::read(&kopfpfad).expect("lm_head.bin lesen"));
    let mpfad = ziel.join("weights_manifest.json");
    let mtext = std::fs::read_to_string(&mpfad).expect("weights_manifest.json");
    let alte_summe = summe_hex(&std::fs::read(quelle.join("lm_head.bin")).expect("Quelle"));
    ersetze_einmal(&mpfad, &mtext, &alte_summe, &neue_summe, "Manifest");

    let tpfad = ziel.join("theta_v.json");
    let ttext = std::fs::read_to_string(&tpfad).expect("theta_v.json");
    let alt_m = summe_hex(std::fs::read(quelle.join("weights_manifest.json")).expect("Quelle").as_slice());
    let neu_m = summe_hex(std::fs::read(&mpfad).expect("Manifest").as_slice());
    ersetze_einmal(&tpfad, &ttext, &alt_m, &neu_m, "theta_v");

    println!("Pruefsummen nachgezogen: lm_head und Manifest");
    println!("\nProbe:  myl frage {} \"Wo wurde Albert Einstein geboren?\"", ziel.display());
}

fn summe_hex(b: &[u8]) -> String {
    integer_llm_runtime::loader::sha256_hex(b)
}

/// Ersetzt eine Summe genau einmal, oder bricht ab.
///
/// ⛑ **Genau einmal.** Kaeme die alte Summe zweimal vor, traefe die
/// Ersetzung auch den anderen Eintrag, und das Artefakt waere still
/// falsch statt laut kaputt.
fn ersetze_einmal(
    pfad: &std::path::Path,
    text: &str,
    alt: &str,
    neu: &str,
    was: &str,
) {
    let n = text.matches(alt).count();
    if n != 1 {
        eprintln!("⛑ {was}: die alte Summe {alt} kommt {n}-mal vor, erwartet genau einmal.");
        std::process::exit(1);
    }
    std::fs::write(pfad, text.replace(alt, neu)).expect("schreiben");
}
