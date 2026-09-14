//! Belegt die Reservierung wirklich Platz, oder sieht sie nur so aus?
//!
//! # ⚑ Warum das gemessen und nicht behauptet wird
//!
//! Der ganze Unterschied zwischen einem Deckel und einer Reservierung
//! ist, dass die zweite dem System den Platz **wegnimmt**. Eine Datei
//! der richtigen Laenge, die null Bloecke belegt, ist eine Datei mit
//! Loechern: Sie sieht in jedem Verzeichnislisting wie eine Reservierung
//! aus und ist keine. **Genau das ist der Fehler, den `set_len` unter
//! Unix machen wuerde**, und die Zusage darf nicht daran haengen, dass
//! jemand das im Kopf hat.

use myl_client::reservierung::{belegung, zu_halten, Reservierung, DATEINAME};

/// So gross, dass die Messung ueber dem Rauschen liegt, und so klein,
/// dass sie auf jeder Maschine durchgeht.
const GROESSE: u64 = 64 * 1024 * 1024;

#[cfg(unix)]
fn wirklich_belegt(pfad: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(pfad).map(|m| m.blocks() * 512).unwrap_or(0)
}

#[cfg(windows)]
fn wirklich_belegt(pfad: &std::path::Path) -> u64 {
    // Auf NTFS bucht `SetEndOfFile` die Bloecke; die Laenge ist damit
    // die Belegung. Ein eigener Weg, sie zu erfragen, brauchte
    // `windows-sys`, und der Gegenwert waere dieselbe Zahl.
    std::fs::metadata(pfad).map(|m| m.len()).unwrap_or(0)
}

/// ⚑ **Die eigentliche Zusage: der Platz ist wirklich weg.**
#[test]
fn die_reservierung_belegt_bloecke_und_keine_loecher() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let r = Reservierung::anlegen(d.path(), GROESSE).expect("anlegen");

    let laenge = std::fs::metadata(r.pfad()).expect("Datei").len();
    assert_eq!(laenge, GROESSE, "die Datei hat nicht die zugesagte Laenge");

    let belegt = wirklich_belegt(r.pfad());
    assert!(
        belegt >= GROESSE,
        "die Datei ist {GROESSE} Bytes lang, belegt aber nur {belegt}.\n\
         Das ist eine Datei mit Loechern und damit keine Reservierung."
    );
}

/// ⚑ **Und sie geht wieder weg, wenn der Wert stirbt.**
///
/// „Solange das Programm geoeffnet ist" ist damit keine
/// Absichtserklaerung, sondern die Lebensdauer eines Wertes.
#[test]
fn beim_fallenlassen_ist_der_platz_zurueck() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let pfad = {
        let r = Reservierung::anlegen(d.path(), GROESSE).expect("anlegen");
        r.pfad().to_path_buf()
    };
    assert!(!pfad.exists(), "die Reservierung liegt nach dem Fallenlassen noch da");
}

/// ⚑ **Herausgeben schrumpft, statt eine zweite Datei anzulegen.**
#[test]
fn hergeben_gibt_genau_so_viel_heraus_wie_da_ist() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let mut r = Reservierung::anlegen(d.path(), GROESSE).expect("anlegen");

    let heraus = r.hergeben(GROESSE / 4).expect("hergeben");
    assert_eq!(heraus, GROESSE / 4);
    assert_eq!(r.rest(), GROESSE - GROESSE / 4);
    assert_eq!(std::fs::metadata(r.pfad()).expect("Datei").len(), r.rest());

    // Mehr verlangen, als da ist: Es kommt, was da ist, und die Zahl
    // sagt es. Ein Fehlschlag waere hier die schlechtere Antwort.
    let heraus = r.hergeben(GROESSE * 10).expect("hergeben");
    assert_eq!(heraus, GROESSE - GROESSE / 4);
    assert_eq!(r.rest(), 0);
    assert!(!r.pfad().exists(), "bei null Bytes bleibt eine leere Datei liegen");

    // Und zurueck.
    r.zurueck(GROESSE / 2).expect("zurueck");
    assert_eq!(r.rest(), GROESSE / 2);
    assert!(wirklich_belegt(r.pfad()) >= GROESSE / 2, "die Rueckgabe belegt nicht");
}

/// 📌 **Eine liegengebliebene Datei wird uebernommen, nicht ergaenzt.**
///
/// Nach einem Absturz steht sie noch da. Wer daneben eine zweite
/// anlegte, hielte den Platz doppelt, und zwei Laeufe hintereinander
/// fuellten die Platte mit lauter Zusagen an denselben Nutzer.
#[test]
fn ein_zweiter_lauf_haelt_den_platz_nicht_doppelt() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::mem::forget(Reservierung::anlegen(d.path(), GROESSE).expect("erster Lauf"));

    let r = Reservierung::anlegen(d.path(), GROESSE).expect("zweiter Lauf");
    assert_eq!(std::fs::metadata(r.pfad()).expect("Datei").len(), GROESSE);
    let im_verzeichnis = std::fs::read_dir(d.path())
        .expect("lesen")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == DATEINAME)
        .count();
    assert_eq!(im_verzeichnis, 1, "es liegen zwei Reservierungen da");
}

/// ⚑ **Die Rechnung, ohne die die Reservierung gegen den eigenen
/// Download arbeitet.**
#[test]
fn belegt_plus_reserviert_ist_die_freigabe() {
    assert_eq!(zu_halten(50, 30), 20);
    assert_eq!(zu_halten(50, 0), 50, "ohne Belegung wird alles gehalten");
    // ⚑ Wer mehr belegt hat, als er freigibt, haelt nichts. Platz
    // nachtraeglich wegzunehmen ginge nur durch Loeschen, und das
    // entscheidet kein Programm von selbst.
    assert_eq!(zu_halten(50, 80), 0);
}

/// Und die Belegung zaehlt Dateien, keine Verweise.
#[test]
fn die_belegung_zaehlt_den_baum_und_folgt_keinem_verweis() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::fs::create_dir_all(d.path().join("tief/tiefer")).expect("Verzeichnis");
    std::fs::write(d.path().join("a.bin"), vec![0u8; 1000]).expect("Datei");
    std::fs::write(d.path().join("tief/tiefer/b.bin"), vec![0u8; 2000]).expect("Datei");

    let draussen = tempfile::tempdir().expect("Verzeichnis");
    std::fs::write(draussen.path().join("gross.bin"), vec![0u8; 500_000]).expect("Datei");

    assert_eq!(belegung(d.path()), 3000, "der Baum wird nicht vollstaendig gezaehlt");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(draussen.path().join("gross.bin"), d.path().join("weg"))
            .expect("Verweis");
        assert_eq!(
            belegung(d.path()),
            3000,
            "einem Verweis nach draussen wurde gefolgt; die Rechnung stimmt dann nicht mehr"
        );
    }
}
